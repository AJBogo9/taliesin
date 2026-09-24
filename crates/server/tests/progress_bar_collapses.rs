//! Item 175b: a `\r` progress bar renders as ONE line, not one line per frame.
//!
//! A tqdm-style writer redraws its bar in place by emitting `\r<frame>` on every
//! update. Each of those arrives as its own iopub `stream` message, so before this
//! fix a 100-update bar rendered as 100 stacked `<pre>` blocks in the built page.
//!
//! This is the end-to-end pin for the collapsing rule. The unit tests in `kernel.rs`
//! cover the semantics; this proves the rule survives a real kernel, a real build,
//! and the freeze cache, which is where a rule applied at the wrong layer (say, at
//! capture time rather than at render time) would quietly stop applying.
//!
//! **No corpus document is added for this.** The corpus walker renders every doc on
//! every `cargo test` but does not execute cells, so a corpus pin would pay the
//! render cost while exercising none of the behavior. Execution-dependent pins live
//! here, against a temp-dir fixture, exactly as `executed_output_reproducible.rs` does.
//!
//! Gated on `TALIESIN_PYTHON`; `TALIESIN_REQUIRE_KERNEL=1` turns the skip into a hard
//! failure so the coverage cannot silently regress to zero.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-175b-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn python_or_skip() -> Option<String> {
    match std::env::var("TALIESIN_PYTHON") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL=1 but TALIESIN_PYTHON is unset: the progress-bar \
                 collapsing pin would silently skip. Point TALIESIN_PYTHON at a python \
                 with ipykernel."
            );
            eprintln!("skipping: TALIESIN_PYTHON not set (no kernel)");
            None
        }
    }
}

fn build(src: &Path, dest: &Path, py: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(src)
        .arg(dest)
        .env("TALIESIN_PYTHON", py)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("run build");
    assert!(
        out.status.success(),
        "build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8(fs::read(dest).expect("read built html")).expect("html is utf-8")
}

#[test]
fn a_carriage_return_progress_bar_builds_as_a_single_line() {
    let Some(py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("bar");
    let src = dir.join("bar.tmd");
    // Five frames of a bar, then a committed line. `end=""` and the leading `\r` are
    // what any progress-bar writer does; `flush=True` makes each frame its own
    // message rather than letting Python's buffering merge them (which would hide
    // the very thing under test).
    fs::write(
        &src,
        "---\ntitle: 175b pin\n---\n\n\
         ```{python}\n\
         import sys\n\
         for i in range(5):\n    \
             print(f\"\\rprogress {i * 25}%\", end=\"\", flush=True)\n\
         print(\"\\rprogress 100%\")\n\
         print(\"after the bar\")\n\
         ```\n",
    )
    .unwrap();

    let html = build(&src, &dir.join("bar.html"), &py);

    // Not vacuous: the cell ran and its output reached the page.
    assert!(
        html.contains("progress 100%"),
        "the cell produced no output — the exec path did not run:\n{html}"
    );
    // The bar collapsed: only the final frame survives. Before the fix every
    // intermediate frame appeared as its own line.
    for frame in [
        "progress 0%",
        "progress 25%",
        "progress 50%",
        "progress 75%",
    ] {
        assert!(
            !html.contains(frame),
            "intermediate bar frame {frame:?} survived, so the bar rendered as a stack \
             of frames instead of redrawing in place"
        );
    }
    // A line committed with `\n` is never eaten by a later redraw.
    assert!(
        html.contains("after the bar"),
        "collapsing swallowed a committed line, which means `\\n` is not ending the \
         current line:\n{html}"
    );
    // And the carriage return itself never reaches the page as a literal.
    assert!(
        !html.contains('\r'),
        "a raw carriage return leaked into the built HTML"
    );
}

/// E2: a progress bar that redraws more often than the flood caps allow still runs to
/// completion. The caps counted RAW iopub messages (4096) and raw stream bytes (512 KB),
/// while the page shows the `\r`-collapsed stream, which for a bar is one short line. So
/// a tqdm loop was SIGINTed at its 4096th redraw (about seven minutes at tqdm's default
/// rate), the page showed a KeyboardInterrupt, and the next cell ran on partial state:
/// the opposite of "a long job that reports progress runs to completion". The caps now
/// apply to what is retained after collapsing, so this bar is one item of a few bytes.
#[test]
fn a_progress_bar_longer_than_the_flood_caps_runs_to_completion() {
    let Some(py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("long-bar");
    let src = dir.join("long.tmd");
    // 6000 flushed redraws of ~100 bytes: past the old item cap and, at 600 KB of raw
    // text, past the old byte cap too.
    fs::write(
        &src,
        "---\ntitle: E2 pin\n---\n\n\
         ```{python}\n\
         steps = 0\n\
         for i in range(6000):\n    \
             steps += 1\n    \
             print(f\"\\r{steps:>5}/6000 \" + \"#\" * 80, end=\"\", flush=True)\n\
         print()\n\
         ```\n\n\
         ```{python}\n\
         print(\"steps completed:\", steps)\n\
         ```\n",
    )
    .unwrap();

    let html = build(&src, &dir.join("long.html"), &py);

    assert!(
        html.contains("steps completed: 6000"),
        "the bar did not run to completion, so the next cell saw partial state:\n{html}"
    );
    for leak in ["output truncated", "KeyboardInterrupt"] {
        assert!(
            !html.contains(leak),
            "a redrawing bar tripped a flood cap ({leak:?}) although it retains one line"
        );
    }
}
