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

/// One `taliesin build <dir> --out <dest>`: the whole project, cache enabled.
fn build_project(dir: &Path, dest: &Path, py: &Path) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(dir)
        .arg("--out")
        .arg(dest)
        .env("TALIESIN_PYTHON", py)
        .env_remove("TALIESIN_NO_CACHE")
        .output()
        .expect("run project build");
    assert!(
        out.status.success(),
        "project build failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// Addressing ONE page of a project by its path replays that project's cache.
///
/// **The defect (audit finding 03).** `cmd_build` branched only on `is_dir()`, so the
/// single-file path rooted the freeze cache at the FILE's own parent with no project
/// resolution at all. Meanwhile `preview <file.tmd>` resolves the file to its enclosing
/// `_site.yml` project (wave 1.1). The two disagreed about what document this even is:
///
///   build <project>              -> <project>/_freeze/posts/p.json
///   build <project>/posts/p.tmd  -> <project>/posts/_freeze/p.json   (a SECOND cache)
///
/// So it re-executed every time, and dropped a stray `_freeze/` in a project subdirectory
/// that nothing sweeps. That also made wave 13's `run` retirement note false where it
/// promises "a later `build` still replays without one" — the note the campaign shipped as
/// its justification for cutting the verb.
///
/// Kernel launches are the observable, exactly as in the tests above: a replay spawns none.
#[test]
fn building_one_page_of_a_project_by_path_replays_the_projects_cache() {
    let Some(real_py) = python_or_skip() else {
        return;
    };
    let tmp = tmp_dir("single-in-project");
    let log = tmp.join("launches.log");
    let py = recording_python(&tmp, &real_py, &log);

    // The project sits BESIDE the recording wrapper, not around it: `build <dir>` sweeps
    // its output directory and copies loose files as assets, and neither should be asked
    // to reason about the instrument measuring it.
    let proj = tmp.join("proj");
    fs::create_dir_all(proj.join("posts")).unwrap();
    fs::write(proj.join("_site.yml"), "title: P\n").unwrap();
    fs::write(proj.join("index.tmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
    let page = proj.join("posts/p.tmd");
    fs::write(
        &page,
        "---\ntitle: Post\n---\n\n```{python}\nprint(1729)\n```\n",
    )
    .unwrap();

    // Build 1: the whole project. Executes, and writes the cache at the project root.
    build_project(&proj, &tmp.join("site-out"), &py);
    let after_project = launches(&log);
    assert!(
        after_project >= 1,
        "the project build spawned no kernel, so this test cannot observe a later replay"
    );
    assert!(
        proj.join("_freeze/posts/p.json").is_file(),
        "the project build did not write <project>/_freeze/posts/p.json, so there is \
         nothing for the single-page build to replay from"
    );

    // Build 2: the SAME page, addressed by path. Must replay rather than re-execute.
    let html = build(&page, &tmp.join("p.html"), &py);
    assert!(
        String::from_utf8_lossy(&html).contains("1729"),
        "the single-page build lost the cell output entirely"
    );
    assert_eq!(
        launches(&log),
        after_project,
        "`build <project>/posts/p.tmd` spawned a kernel for a page the project build had \
         already cached: the single-file path is not rooting the freeze cache at the \
         enclosing project"
    );
    assert!(
        !proj.join("posts/_freeze").exists(),
        "a SECOND freeze cache was written inside the project at posts/_freeze/, which no \
         sweep removes and no later build reads"
    );
}

/// Editing an **upstream** cell busts the cells below it, whose own source never changed.
///
/// This is the property the cumulative hash exists for, and the one a per-cell cache would
/// get wrong: cell 2's code is byte-identical across both builds, so a key over that code
/// alone hits, replays yesterday's number, and ships a document whose two cells disagree.
/// `freeze.rs`'s unit tests pin it on `FreezeCache` directly; nothing drove it through a
/// real build until Wave 2, which cut `read --run` — the third and last caller of the
/// executor + `_freeze` pair from outside `build`/`serve_site`. Sixty lines here in exchange
/// for that suite is the trade the cut's own dissent asked for.
#[test]
fn editing_an_upstream_cell_re_executes_the_cells_below_it() {
    let Some(real_py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("upstream");
    let log = dir.join("launches.log");
    let py = recording_python(&dir, &real_py, &log);
    let src = dir.join("doc.tmd");

    // Two cells: the second prints a value the first defines and never mentions the literal
    // itself, so its output can only be right if it really re-ran against the new upstream.
    let doc = |seed: u32| {
        format!(
            "---\ntitle: U\n---\n\n```{{python}}\nx = {seed}\n```\n\n```{{python}}\nprint(x * 2)\n```\n"
        )
    };

    fs::write(&src, doc(1729)).unwrap();
    let first = build(&src, &dir.join("a.html"), &py);
    assert!(
        String::from_utf8_lossy(&first).contains("3458"),
        "the downstream cell did not execute on the cold build, so this test cannot observe \
         it going stale"
    );
    let after_first = launches(&log);
    assert!(after_first >= 1, "no kernel launch on the cold build");

    // Only the FIRST cell's source changes. The second is byte-identical.
    fs::write(&src, doc(4104)).unwrap();
    let edited = build(&src, &dir.join("b.html"), &py);
    assert!(
        launches(&log) > after_first,
        "an edited upstream cell replayed from cache instead of re-executing"
    );
    let edited = String::from_utf8_lossy(&edited);
    assert!(
        edited.contains("8208"),
        "the downstream cell replayed a hit keyed on its own unchanged source: the cumulative \
         hash is not folding in the cells above it, and the build shipped stale output"
    );
    assert!(
        !edited.contains("3458"),
        "the stale downstream output survived alongside the new one"
    );
}

/// One `taliesin build <src> <dest>` returning the whole `Output`, so a test can read
/// stderr (where the human log lives) and tolerate a document containing a cell error.
fn build_capturing(src: &Path, dest: &Path, py: &Path) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_taliesin"));
    cmd.arg("build").arg(src).arg(dest);
    cmd.env("TALIESIN_PYTHON", py);
    cmd.env_remove("TALIESIN_NO_CACHE");
    cmd.output().expect("run build")
}

/// The one `_freeze/*.json` under `dir`.
fn freeze_file(dir: &Path) -> PathBuf {
    let freeze = dir.join("_freeze");
    let mut found: Vec<PathBuf> = fs::read_dir(&freeze)
        .unwrap_or_else(|e| panic!("read {}: {e}", freeze.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    assert_eq!(found.len(), 1, "expected one freeze file, found {found:?}");
    found.pop().unwrap()
}

/// Deleting a `#| cache: false` line must not leave a page that contradicts itself for ever.
///
/// **The defect (audit A6).** `strip_cell_options` drops `#|` lines before hashing, so
/// `cache:` is a field and never part of the cumulative key. Cells *downstream* of a
/// `cache: false` cell were therefore persisted under a key asserting "this output follows
/// from this upstream code" — about a cell whose entire point is that its output does not
/// follow from its code. Those entries were already false when written; `plan` forcing
/// `run_end = len` while the directive is present was only a runtime mask over them.
///
/// Remove the directive and the mask goes with it: the upstream cell has no entry (it was
/// never cacheable) so it re-runs, while the downstream cell restores a disk entry computed
/// from a *different* upstream value. The page then reads `upstream x = 3` above
/// `downstream sees x = 2`, and stays that way across every later rebuild.
///
/// The upstream cell counts a file rather than rolling a die, so a failure is deterministic
/// rather than one collision away from passing.
#[test]
fn deleting_a_cache_false_directive_leaves_a_self_consistent_page() {
    let Some(real_py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("cache-false-deleted");
    let log = dir.join("launches.log");
    let py = recording_python(&dir, &real_py, &log);
    let counter = dir.join("counter.txt");
    let src = dir.join("doc.tmd");

    // Cell A is uncacheable and yields a different value every run; cell B reads A's
    // variable, so a correct page always shows the same number twice.
    let doc = |directive: &str| {
        format!(
            "---\ntitle: C\n---\n\n\
             ```{{python}}\n{directive}from pathlib import Path\n\
             p = Path(r\"{counter}\")\n\
             x = int(p.read_text()) + 1 if p.exists() else 1\n\
             p.write_text(str(x))\n\
             print(f\"upstream x = {{x}}\")\n```\n\n\
             ```{{python}}\nprint(f\"downstream sees x = {{x}}\")\n```\n",
            counter = counter.display()
        )
    };
    // `rfind`, not `find`: the page echoes each cell's SOURCE above its output, so the
    // first match is the `print(f"upstream x = {x}")` literal, whose next character is `{`.
    fn reading(html: &str, label: &str) -> String {
        let at = html
            .rfind(label)
            .unwrap_or_else(|| panic!("`{label}` missing from the page:\n{html}"));
        html[at + label.len()..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect()
    }
    fn both(html: &[u8]) -> (String, String) {
        let html = String::from_utf8_lossy(html).into_owned();
        (
            reading(&html, "upstream x = "),
            reading(&html, "downstream sees x = "),
        )
    }

    // Two builds WITH the directive. Both are self-consistent today; they are the control,
    // and they are also what writes the false downstream entry.
    fs::write(&src, doc("#| cache: false\n")).unwrap();
    let (up1, down1) = both(&build(&src, &dir.join("a.html"), &py));
    assert_eq!(up1, down1, "first build disagrees with itself");
    let (up2, down2) = both(&build(&src, &dir.join("b.html"), &py));
    assert_eq!(up2, down2, "second build disagrees with itself");
    assert_ne!(
        up1, up2,
        "the uncacheable cell replayed instead of re-running, so this test is not \
         exercising non-determinism at all"
    );

    // Delete ONLY the directive. No code changes, so no cumulative key changes.
    fs::write(&src, doc("")).unwrap();
    let (up3, down3) = both(&build(&src, &dir.join("c.html"), &py));
    assert_eq!(
        up3, down3,
        "the page contradicts itself: the downstream cell restored an output computed \
         from a different upstream value"
    );
    assert_ne!(up2, up3, "the formerly-uncacheable cell did not re-run");

    // And the cache still works once the directive is gone: nothing re-runs, and the page
    // is unchanged. (A fix that merely stopped persisting would pass everything above.)
    let after = launches(&log);
    let (up4, down4) = both(&build(&src, &dir.join("d.html"), &py));
    assert_eq!((up3, down3), (up4, down4), "the replay changed the page");
    assert_eq!(
        launches(&log),
        after,
        "an all-cacheable document re-executed instead of replaying from disk"
    );
}

/// A replay that crossed a package change says so.
///
/// **The defect (audit A7).** `stamp_packages` wrote the current digest into the cache
/// *before* `warn_if_packages_moved` read it back, and `packages::manifest` is memoized
/// process-wide, so `crossed(recorded, now)` compared the digest against itself and was
/// false by construction. The one axis the cumulative key structurally cannot see had a
/// warning that could never fire.
///
/// The package set is moved by rewriting the digest the cache recorded rather than by
/// installing anything: that is exactly the state a `pip install --upgrade` leaves behind
/// (same interpreter, same `--version`, same keys, different packages), and it does not
/// require the test to mutate the machine it runs on.
///
/// The warning needs something restored from DISK, which is the ordinary full replay: both
/// cells succeed and are persisted on the first build, and the second build restores both
/// without starting a kernel. (It used to make cell 1 fail so its key stayed unknown and
/// cell 2 came back as a restored tail. That fixture stopped existing on 2026-08-17, when
/// the persist range learned to stop at the first errored cell: nothing downstream of a
/// failure is written any more, so there is no such tail to restore.)
#[test]
fn a_replay_that_crossed_a_package_change_is_announced() {
    let Some(real_py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("packages-moved");
    let log = dir.join("launches.log");
    let py = recording_python(&dir, &real_py, &log);
    let src = dir.join("doc.tmd");
    fs::write(
        &src,
        "---\ntitle: P\n---\n\n```{python}\nprint(1618)\n```\n\n\
         ```{python}\nprint(2718)\n```\n",
    )
    .unwrap();
    const ANNOUNCEMENT: &str = "produced under a different package set";

    // Build 1: both cells run, both are persisted, and the run stamps its package digest.
    let first = build_capturing(&src, &dir.join("a.html"), &py);
    assert!(
        first.status.success(),
        "the first build should be clean: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    // The negative control, and it is the half that matters: an unmoved package set must
    // stay silent, or an always-firing warning would pass the assertion below.
    let quiet = build_capturing(&src, &dir.join("b.html"), &py);
    let quiet_err = String::from_utf8_lossy(&quiet.stderr);
    assert!(
        !quiet_err.contains(ANNOUNCEMENT),
        "nothing moved, so nothing should be announced:\n{quiet_err}"
    );

    // Move it: the outputs on disk were produced under a package set that is not the one
    // installed now.
    let file = freeze_file(&dir);
    let text = fs::read_to_string(&file).expect("freeze json");
    let mut cache: serde_json::Value = serde_json::from_str(&text).expect("freeze json parses");
    let packages = cache
        .get_mut("packages")
        .and_then(|p| p.as_object_mut())
        .expect("the cache recorded a package digest");
    assert!(
        packages.contains_key("python"),
        "no python digest recorded, so this test would assert nothing: {text}"
    );
    packages.insert(
        "python".to_string(),
        serde_json::Value::String("0000000000000000".to_string()),
    );
    fs::write(&file, serde_json::to_string(&cache).unwrap()).unwrap();

    let moved = build_capturing(&src, &dir.join("c.html"), &py);
    let moved_err = String::from_utf8_lossy(&moved.stderr);
    assert!(
        moved_err.contains(ANNOUNCEMENT),
        "a restore that crossed a package change said nothing:\n{moved_err}"
    );
}

/// Nothing computed after a failed cell is persisted.
///
/// **The defect (Fable audit FA7).** The persist loop refused an errored cell's own output
/// and the `#| cache: false` tail, and nothing else. A cell that ran AFTER an upstream cell
/// raised executed against half-mutated kernel state (the assignments below the raise never
/// happened), and its output went to disk under a cumulative key that asserts it follows
/// from a complete upstream run. The failing cell is never persisted, so it re-runs for
/// ever; the moment its failure stops reproducing, the downstream cell restores the output
/// it produced while the failure was still there.
#[test]
fn a_cell_after_a_failed_one_is_not_persisted() {
    let Some(real_py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("downstream-of-error");
    let log = dir.join("launches.log");
    let py = recording_python(&dir, &real_py, &log);
    let src = dir.join("doc.tmd");
    // Cell B does not read A's state, so it succeeds. That is the point: a successful
    // output is exactly what would be written, and its correctness is not the question.
    fs::write(
        &src,
        "---\ntitle: E\n---\n\n```{python}\nraise ValueError(\"boom\")\n```\n\n\
         ```{python}\nprint(2718)\n```\n",
    )
    .unwrap();

    let out = build_capturing(&src, &dir.join("a.html"), &py);
    assert!(
        out.status.success(),
        "a cell error is baked into the page, not a build failure: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let page = fs::read_to_string(dir.join("a.html")).expect("the page was written");
    assert!(page.contains("2718"), "the downstream cell did run: {page}");

    let text = fs::read_to_string(freeze_file(&dir)).expect("freeze json");
    let cache: serde_json::Value = serde_json::from_str(&text).expect("freeze json parses");
    let entries = cache
        .get("entries")
        .and_then(|e| e.as_array())
        .unwrap_or_else(|| panic!("no entries list in the freeze file: {text}"));
    assert!(
        entries.is_empty(),
        "an output computed after a failed cell was persisted: {text}"
    );

    // The negative control: with the failure removed, both cells persist as normal, so the
    // assertion above is about the failure and not about persistence being broken.
    fs::write(
        &src,
        "---\ntitle: E\n---\n\n```{python}\nprint(1618)\n```\n\n\
         ```{python}\nprint(2718)\n```\n",
    )
    .unwrap();
    build_capturing(&src, &dir.join("b.html"), &py);
    let text = fs::read_to_string(freeze_file(&dir)).expect("freeze json");
    let cache: serde_json::Value = serde_json::from_str(&text).expect("freeze json parses");
    let entries = cache
        .get("entries")
        .and_then(|e| e.as_array())
        .unwrap_or_else(|| panic!("no entries list in the freeze file: {text}"));
    assert_eq!(
        entries.len(),
        2,
        "a clean two-cell document must persist both: {text}"
    );
}
