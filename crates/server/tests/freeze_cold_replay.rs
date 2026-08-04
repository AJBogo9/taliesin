//! A cold `build` of an unchanged document replays cell output from `_freeze/` **without
//! spawning a kernel**.
//!
//! This is the end-to-end half of the freeze cache, and until now nothing pinned it. The
//! nine unit tests in `freeze.rs` pin the *key* — that a cumulative hash busts on the cell,
//! on anything upstream of it, and on the interpreter — plus eviction, the save/load round
//! trip and the version-mismatch reset. All of them exercise `FreezeCache` directly. None
//! of them drives `taliesin build` twice and asks whether the second run actually skipped
//! the work, which is the property the tool is *sold* on: "an unchanged doc replays from
//! disk on the next build without booting the kernel".
//!
//! **Why the gap matters more than its size.** A break here is silent in the worst way. If
//! the restore path stops being consulted, every build still produces *correct* output — it
//! just re-executes, so the only symptom is that builds got slower, which no assertion
//! anywhere notices. And a stale hit is worse: it returns yesterday's output for today's
//! code, and the unit tests cannot see it because they never let a real build choose between
//! the cache and the kernel.
//!
//! **How "no kernel was spawned" is observed.** `TALIESIN_PYTHON` points at a wrapper script
//! that `exec`s the real interpreter and appends a line to a log **only when the arguments
//! contain `ipykernel_launcher`** — the shape `kernel.rs` actually spawns. Probing
//! (`<bin> --version`, which seeds the interpreter id) therefore does not pollute the
//! signal, and the wrapper path is identical across both builds, so the interpreter id is
//! stable and the cache key does not bust for the wrong reason.
//!
//! Gated on `TALIESIN_PYTHON` like the rest of the exec tests, with the
//! `TALIESIN_REQUIRE_KERNEL` escalation so the coverage cannot silently regress to zero.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("tali-freeze-replay-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The python to test against, or `None` to skip — unless the CI canary is armed, in which
/// case a missing interpreter is a hard failure so the gap cannot hide.
fn python_or_skip() -> Option<String> {
    match std::env::var("TALIESIN_PYTHON") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL=1 but TALIESIN_PYTHON is unset: the freeze cold-replay \
                 pin would silently skip. Point TALIESIN_PYTHON at a python with ipykernel."
            );
            eprintln!("skipping: TALIESIN_PYTHON not set (no kernel)");
            None
        }
    }
}

/// A `python` that records kernel launches. `exec` (not a subshell) so signals, the
/// connection file and stdio all behave exactly as they would for the real interpreter —
/// this must not perturb the thing it measures.
fn recording_python(dir: &Path, real: &str, log: &Path) -> PathBuf {
    let wrapper = dir.join("python-recording");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\n\
             # Record only a real kernel launch. `--version` probes seed the interpreter id\n\
             # on every build, so counting them would report a launch that never happened.\n\
             for a in \"$@\"; do\n\
             \x20 case \"$a\" in ipykernel_launcher) echo launch >> {log} ;; esac\n\
             done\n\
             exec {real} \"$@\"\n",
            log = shell_quote(log.to_str().unwrap()),
            real = shell_quote(real),
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    }
    wrapper
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// One `taliesin build <src> <dest>`, cache **enabled**.
///
/// `TALIESIN_NO_CACHE` is removed rather than merely left unset: the whole point of this
/// test is the cache, and inheriting that variable from the ambient environment would make
/// it pass vacuously by never writing `_freeze/` and never claiming a hit.
fn build(src: &Path, dest: &Path, py: &Path) -> Vec<u8> {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(src)
        .arg(dest)
        .env("TALIESIN_PYTHON", py)
        .env_remove("TALIESIN_NO_CACHE")
        .output()
        .expect("run build");
    assert!(
        out.status.success(),
        "build failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    fs::read(dest).expect("built html")
}

fn launches(log: &Path) -> usize {
    fs::read_to_string(log)
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

#[test]
fn an_unchanged_document_rebuilds_from_freeze_without_spawning_a_kernel() {
    let Some(real_py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("unchanged");
    let log = dir.join("launches.log");
    let py = recording_python(&dir, &real_py, &log);

    // A deterministic cell whose output could not appear by accident: if the second build
    // silently rendered the cell as source instead of replaying it, `1729` would be absent
    // from the output rather than merely stale.
    let src = dir.join("doc.tmd");
    fs::write(
        &src,
        "---\ntitle: Replay\n---\n\n```{python}\nprint(1729)\n```\n",
    )
    .unwrap();

    // Build 1: cold. Must really execute.
    let first = build(&src, &dir.join("out1.html"), &py);
    assert!(
        String::from_utf8_lossy(&first).contains("1729"),
        "first build did not execute the cell"
    );
    let after_first = launches(&log);
    assert!(
        after_first >= 1,
        "the wrapper observed no kernel launch on a cold build, so it is not measuring what \
         it claims — a later zero would be meaningless"
    );
    assert!(
        dir.join("_freeze").exists(),
        "no _freeze/ written, so there is nothing for the second build to replay from"
    );

    // Build 2: nothing changed. Must replay, not re-execute.
    let second = build(&src, &dir.join("out2.html"), &py);
    assert_eq!(
        launches(&log),
        after_first,
        "the second build spawned a kernel for an unchanged document: the freeze cache was \
         not consulted, or its key busted for a reason that is not a content change"
    );
    assert_eq!(
        first, second,
        "replayed output differs from executed output"
    );
}

#[test]
fn editing_the_cell_spawns_a_kernel_again() {
    let Some(real_py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("edited");
    let log = dir.join("launches.log");
    let py = recording_python(&dir, &real_py, &log);
    let src = dir.join("doc.tmd");

    fs::write(
        &src,
        "---\ntitle: E\n---\n\n```{python}\nprint(1729)\n```\n",
    )
    .unwrap();
    build(&src, &dir.join("a.html"), &py);
    let after_first = launches(&log);

    // The negative control. Without it, a cache that returned a hit for EVERYTHING would
    // pass the test above — and that is the failure mode that ships wrong output rather
    // than merely slow builds.
    fs::write(
        &src,
        "---\ntitle: E\n---\n\n```{python}\nprint(4104)\n```\n",
    )
    .unwrap();
    let edited = build(&src, &dir.join("b.html"), &py);
    assert!(
        launches(&log) > after_first,
        "an edited cell replayed from cache instead of re-executing: the key is not tracking \
         cell content, and the cache is serving stale output"
    );
    let edited = String::from_utf8_lossy(&edited);
    assert!(
        edited.contains("4104") && !edited.contains("1729"),
        "edited cell did not produce its new output, or the stale one survived alongside it"
    );
}
