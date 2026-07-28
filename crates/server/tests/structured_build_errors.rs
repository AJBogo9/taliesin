//! `build`/`publish --format json` emit the build's static-lint diagnostics as
//! `{diagnostics:[{code,severity,file,line,message,suggestion?}]}` to stdout (for an
//! agent/CI), reusing `check`'s exact per-diagnostic shape so the two channels can't drift.
//! The build set is a *superset* of `check`'s (it adds embed + cell-error outputs), so
//! every diagnostic `check` reports must also appear in the build's JSON.

use std::collections::HashSet;
use std::fs;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-sbe-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn taliesin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_taliesin"))
}

fn stdout_json(cmd: &mut Command) -> serde_json::Value {
    let out = cmd.output().expect("run taliesin");
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not valid json ({e}):\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn messages(v: &serde_json::Value) -> HashSet<String> {
    v["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .map(|d| d["message"].as_str().unwrap_or("").to_string())
        .collect()
}

#[test]
fn single_doc_build_json_emits_structured_diagnostics() {
    let dir = tmp_dir("single");
    // A dup heading id + a missing image: both static (kernel-free), in check's standalone set.
    let doc = dir.join("doc.tmd");
    fs::write(
        &doc,
        "---\ntitle: T\n---\n\n## A {#dup}\n\n## B {#dup}\n\n![a missing chart](missing.png)\n",
    )
    .unwrap();

    let build = stdout_json(
        taliesin()
            .arg("build")
            .arg(&doc)
            .arg(dir.join("out.html"))
            .arg("--strict")
            .args(["--format", "json"]),
    );
    let diags = build["diagnostics"].as_array().expect("array");
    assert!(!diags.is_empty(), "build reports diagnostics: {build}");
    for d in diags {
        assert!(
            d["code"].as_str().is_some_and(|c| c.starts_with("TAL-")),
            "each carries a code: {d}"
        );
        assert!(d["file"].as_str().is_some(), "each carries a file: {d}");
    }

    // check's diagnostics for the same doc are a SUBSET of the build's (build is a superset).
    let check = stdout_json(taliesin().arg("check").arg(&doc).args(["--format", "json"]));
    let (build_msgs, check_msgs) = (messages(&build), messages(&check));
    assert!(
        check_msgs.is_subset(&build_msgs),
        "check diagnostics must be a subset of build's.\ncheck-only: {:?}",
        check_msgs.difference(&build_msgs).collect::<Vec<_>>()
    );
}

#[test]
fn site_publish_dry_run_json_emits_structured_diagnostics() {
    let dir = tmp_dir("site");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n## A {#dup}\n\n## B {#dup}\n\n![a missing chart](nope.png)\n",
    )
    .unwrap();

    let publish = stdout_json(
        taliesin()
            .arg("publish")
            .arg(&dir)
            .arg("--dry-run")
            .args(["--format", "json"]),
    );
    let diags = publish["diagnostics"].as_array().expect("array");
    assert!(
        diags.iter().any(|d| d["message"]
            .as_str()
            .unwrap_or("")
            .contains("duplicate heading id")),
        "publish --dry-run --format json reports the site's diagnostics: {publish}"
    );
    assert!(
        diags
            .iter()
            .all(|d| d["file"].as_str() == Some("index.tmd")),
        "diagnostics are located to their page: {publish}"
    );
}

#[test]
fn build_rejects_a_bad_format_value() {
    let dir = tmp_dir("badfmt");
    let doc = dir.join("doc.tmd");
    fs::write(&doc, "---\ntitle: T\n---\n\nHi.\n").unwrap();
    let out = taliesin()
        .arg("build")
        .arg(&doc)
        .args(["--format", "yaml"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "an unknown --format value must fail");
}

/// A single-doc build locates its diagnostics to a path a tool can OPEN, in both channels.
///
/// It used to prefix them with `file_stem()`: `doc:5: duplicate heading id`. That is not a
/// path — no editor's "open at line", no `vim +5`, no CI annotation resolves it, and the
/// information needed to build one (the argument the user typed) was right there at the call
/// site. The site build never had this defect, so nothing compared the two.
///
/// Asserted from a DIFFERENT working directory than the document's, so a bare filename
/// cannot pass by accident: the label has to carry the directory the user actually named.
#[test]
fn single_doc_diagnostics_are_located_to_an_openable_path() {
    let dir = tmp_dir("label");
    let sub = dir.join("chapters");
    fs::create_dir_all(&sub).unwrap();
    let doc = sub.join("intro.tmd");
    fs::write(&doc, "---\ntitle: T\n---\n\n## A {#dup}\n\n## B {#dup}\n").unwrap();
    // The path exactly as a user would type it from `dir`, directory component included.
    let typed = "chapters/intro.tmd";

    let out = taliesin()
        .current_dir(&dir)
        .arg("build")
        .arg(typed)
        .args(["--format", "json"])
        .output()
        .expect("run taliesin");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let files: HashSet<String> = json["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .map(|d| d["file"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        !files.is_empty(),
        "the dup heading must be reported: {json}"
    );
    for f in &files {
        assert_eq!(
            f, typed,
            "every diagnostic names the path the user gave, not its stem: {json}"
        );
        assert!(
            dir.join(f).exists(),
            "the located file resolves from the invocation directory: {f}"
        );
    }

    // The human channel is the same label, so the two cannot drift apart again.
    let human = taliesin()
        .current_dir(&dir)
        .arg("build")
        .arg(typed)
        .output()
        .expect("run taliesin");
    let stderr = String::from_utf8_lossy(&human.stderr).to_string();
    assert!(
        stderr.contains(&format!("{typed}:")),
        "the console prefixes the openable path too:\n{stderr}"
    );
}
