//! `taliesin init` scaffolds a project that is correct on the first save: it renders and
//! `taliesin check` passes on it with no diagnostics.
//!
//! This is the behavioral pin for the whole scaffolder. It used to sit beside a byte pin
//! (`corpus/scaffold-{site,book}/`, compared const-for-const in `cli::init_template_tests`)
//! covering three templates; Wave 8 cut `init` to the one starter, so what is left is the
//! property that actually matters: the real binary writes it, the real `check` reads it
//! back over the whole project, and nothing arrives that nobody asked for.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    // A per-call sequence (not just name+pid) so two tests reusing a name don't share one dir
    // and clobber each other's scaffold when run in parallel.
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tali-init-{}-{}-{}", name, std::process::id(), seq));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin init");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn check_is_clean(path: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", path.to_str().unwrap(), "--check-only"])
        .output()
        .expect("run the lint");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The whole point: the starter must survive the tool's own preflight as a project.
#[test]
fn init_scaffolds_a_check_clean_project() {
    let dir = tmp("basic");
    let (ok, _out, err) = run(&["init", dir.to_str().unwrap()]);
    assert!(ok, "`init` should succeed; stderr: {err}");

    assert!(dir.join("_site.yml").exists(), "_site.yml written");
    assert!(dir.join("index.tmd").exists(), "index.tmd written");

    let (clean, diagnostics) = check_is_clean(&dir);
    assert!(
        clean,
        "`taliesin check` must pass on a fresh `init`, got:\n{diagnostics}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `init` writes those two files and nothing else. It shipped a `.taliesin/` dot-directory
/// (a copy of the bundled `_site.yml` schema, wired by a modeline on the config's first
/// line) into every project it created until Wave 8, and zero such directories existed
/// anywhere in this repository, including in the author's own projects.
#[test]
fn init_writes_nothing_the_author_did_not_ask_for() {
    let dir = tmp("no-extras");
    let (ok, _out, err) = run(&["init", dir.to_str().unwrap()]);
    assert!(ok, "stderr: {err}");

    let mut entries: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    assert_eq!(
        entries,
        vec!["_site.yml".to_string(), "index.tmd".to_string()],
        "init wrote something unasked for"
    );
    let cfg = std::fs::read_to_string(dir.join("_site.yml")).unwrap();
    assert!(
        !cfg.contains("yaml-language-server"),
        "no schema modeline: {cfg}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The template menu is gone: `--template` is an unknown flag, and a rejected run never
/// half-scaffolds.
#[test]
fn the_template_flag_is_rejected_and_writes_nothing() {
    let dir = tmp("template");
    let (ok, _out, err) = run(&["init", dir.to_str().unwrap(), "--template", "site"]);
    assert!(!ok, "`--template` no longer exists, so it must fail");
    assert!(err.contains("--template"), "names the flag typed: {err}");
    assert!(
        !dir.join("_site.yml").exists(),
        "no partial scaffold on a rejected flag"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The wizard is gone, so a bare `init` scaffolds rather than prompting, which is the
/// behavior CI, a pipe and an agent always got. Driven with stdin on `/dev/null` so a
/// regression that reintroduced a prompt would hang or fail here, not at a terminal.
#[test]
fn a_bare_init_scaffolds_without_prompting() {
    use std::process::Stdio;
    let dir = tmp("bare");
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["init", dir.to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .expect("run taliesin init");
    assert!(
        out.status.success(),
        "bare `init` scaffolds; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.join("_site.yml").exists() && dir.join("index.tmd").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `-y`/`--yes` opted out of that wizard. Both are gone, and neither may be mistaken for the
/// directory to scaffold into, which is what a bare `-y` would have become, since a
/// leading-dash token is otherwise just a positional.
#[test]
fn the_retired_yes_flag_is_not_read_as_a_directory() {
    let cwd = tmp("yes");
    std::fs::create_dir_all(&cwd).unwrap();
    for flag in ["-y", "--yes"] {
        let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
            .args(["init", flag])
            .current_dir(&cwd)
            .output()
            .expect("run taliesin init");
        assert!(!out.status.success(), "`init {flag}` must fail");
        assert!(
            !cwd.join(flag).exists(),
            "`init {flag}` must not scaffold into a directory called `{flag}`"
        );
    }
    let _ = std::fs::remove_dir_all(&cwd);
}
