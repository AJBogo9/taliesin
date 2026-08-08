//! A mistyped input path gets a "did you mean" at every front door that takes one.
//!
//! `cannot read notes.tdm: No such file or directory (os error 2)` is the first thing a new
//! user sees when they transpose two characters, and it told them nothing they did not
//! already know — while `closest` had been suggesting subcommands and front-matter keys in
//! the same binary for months.
//!
//! Kernel-free and network-free: nothing here gets far enough to execute anything.

use std::fs;
use std::process::Command;

/// Every front door whose first positional is a single `.tmd` file *and* which routes a
/// missing one through `lint::cannot_read`. Kept as a list because an integration test
/// cannot reach the bin crate's `COMMANDS`; the floor below is what stops the list quietly
/// shrinking to one.
///
/// **Two invocations of `build`, not two verbs, and that is not padding.** A writing build
/// reaches `cannot_read` from `cmd_build`'s own `read_to_string`; `--check-only` reaches it
/// from `lint::collect_diagnostics`, a different call site that the retired `check` verb used
/// to own. Wave 5 removed `render`, `blocks` and `symbols`, wave 2 removed `read` and `map`,
/// and wave 9 retired `check` into the flag below. Measured 2026-08-03, the front door that
/// takes a path and still does NOT suggest: `run` ("no such file"). That is a pre-existing
/// gap, not a regression from any cut, and it is named here so the next reader sees the
/// omission is known rather than assuming this list is exhaustive.
const FILE_COMMANDS: &[&[&str]] = &[&["build"], &["build", "--check-only"]];

#[test]
fn every_file_front_door_suggests_the_near_miss() {
    let dir = std::env::temp_dir().join(format!("tali-mis-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("intro.tmd"), "---\ntitle: T\n---\n\nHi.\n").unwrap();

    assert!(
        FILE_COMMANDS.len() >= 2,
        "the list of front doors under test must not silently shrink"
    );
    for argv in FILE_COMMANDS {
        let cmd = argv.join(" ");
        let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
            .current_dir(&dir)
            .args(*argv)
            // A transposed extension: distance 2 from the real `intro.tmd`.
            .arg("intro.tdm")
            .output()
            .expect("run taliesin");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            !out.status.success(),
            "`{cmd}` on a missing file must still fail: {combined}"
        );
        assert!(
            combined.contains("did you mean `intro.tmd`"),
            "`{cmd}` should suggest the near-miss sibling, got:\n{combined}"
        );
    }

    // A name with no near miss gets no guess: a confidently wrong suggestion is worse than
    // the bare error it replaced.
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .current_dir(&dir)
        .args(["build", "completely-unrelated.tmd", "--check-only"])
        .output()
        .expect("run taliesin");
    let combined = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        !combined.contains("did you mean"),
        "nothing is within edit distance 2, so nothing is offered:\n{combined}"
    );

    let _ = fs::remove_dir_all(&dir);
}
