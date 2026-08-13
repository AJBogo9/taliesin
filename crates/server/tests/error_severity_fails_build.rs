//! A plain `build` must fail on an **error**-severity diagnostic, and only on that.
//!
//! Before 2026-08-13 the severity a validator set was honoured by `--check-only` and
//! discarded by `build`: unparseable front matter printed as `warn` and exited 0, having
//! written an HTML page whose `title:`, `bibliography:` and `listing:` were all silently
//! dropped. A first author who never learns `--check-only` publishes that page having seen
//! `built` and a zero exit, which is the one outcome CI reads as "fine".
//!
//! The line is severity, not verb: `error` means the document is wrong, so it fails
//! everywhere. `warning` still ships (that is what `--strict` is for), which is what keeps
//! this from becoming "any diagnostic fails" — the rule the tool deliberately does not have.
//!
//! Exit codes are `std::process::ExitCode`, opaque to a unit test, so these go through
//! `CARGO_BIN_EXE_taliesin`.

use std::fs;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-errsev-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn taliesin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_taliesin"))
}

#[test]
fn unparseable_front_matter_fails_a_plain_build_and_still_writes_the_page() {
    let dir = tmp_dir("frontmatter");
    // Unterminated double-quoted scalar: serde_yaml cannot parse it, so every key in the
    // block is dropped -- including the `title:` the page is named by.
    fs::write(
        dir.join("bad.tmd"),
        "---\ntitle: \"unterminated\nbibliography: refs.bib\n---\n\n# Hello\n\nSome text.\n",
    )
    .unwrap();
    let out = dir.join("bad.html");

    let built = taliesin()
        .arg("build")
        .arg(dir.join("bad.tmd"))
        .arg(&out)
        .arg("--no-exec")
        .output()
        .expect("run build");
    let err = String::from_utf8_lossy(&built.stderr);

    assert!(
        !built.status.success(),
        "an error-severity diagnostic fails a plain build, with no --strict: {err}"
    );
    // The severity word itself, not just the failure: printing an `error` as `warn` is the
    // half of this defect a non-zero exit alone would leave in place.
    assert!(
        err.contains("error") && err.contains("not valid YAML"),
        "the front-matter parse error prints at error severity: {err}"
    );
    // Same shape as `--strict` and as the kernel-failure path: the page is written, then
    // the run fails. Withholding the output would make the failure harder to diagnose.
    assert!(
        out.exists(),
        "the page is still written; only the exit code changes"
    );
}

#[test]
fn a_warning_only_document_still_builds_green() {
    let dir = tmp_dir("warnonly");
    // An unknown front-matter key is severity `warning`: something will silently not work,
    // but the document is not wrong. This is the case `--strict` exists for, so a plain
    // build must still exit 0 or the alignment above has swallowed the distinction.
    fs::write(
        dir.join("warn.tmd"),
        "---\ntitle: Fine\nnotakey: nope\n---\n\n# Hello\n\nSome text.\n",
    )
    .unwrap();
    let out = dir.join("warn.html");

    let built = taliesin()
        .arg("build")
        .arg(dir.join("warn.tmd"))
        .arg(&out)
        .arg("--no-exec")
        .output()
        .expect("run build");
    let err = String::from_utf8_lossy(&built.stderr);

    assert!(
        built.status.success(),
        "a warning-severity diagnostic still builds green: {err}"
    );
    assert!(
        err.contains("notakey"),
        "the unknown key is still reported: {err}"
    );

    // ...and `--strict` is still the flag that turns it into a failure.
    let strict = taliesin()
        .arg("build")
        .arg(dir.join("warn.tmd"))
        .arg(&out)
        .arg("--no-exec")
        .arg("--strict")
        .output()
        .expect("run strict build");
    assert!(
        !strict.status.success(),
        "--strict still fails on a warning: {}",
        String::from_utf8_lossy(&strict.stderr)
    );
}
