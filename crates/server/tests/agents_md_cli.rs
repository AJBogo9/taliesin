//! `taliesin init` scaffolds an `AGENTS.md` onramp so a coding agent driving a fresh
//! project learns the whole loop (edit the `.tmd`, gate on `check --format json`, the
//! dialect) on first contact instead of guessing from stale Quarto priors.
//!
//! The dialect terms are generated from `taliesin_core::vocab::vocab()` and golden-locked
//! in core (`agents::agents_md_matches_committed`); here we pin the *scaffold write* and
//! the repo-root shipped copy so neither can silently rot.

use std::process::Command;

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-agents-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// `init` writes an AGENTS.md that teaches the four pillars and carries vocab-sourced
/// dialect terms.
#[test]
fn init_scaffolds_the_agents_onramp() {
    let dir = tmp("init");
    let (ok, _stdout, stderr) = run(&["init", dir.to_str().unwrap()]);
    assert!(ok, "`init` should succeed; stderr: {stderr}");

    let agents = dir.join("AGENTS.md");
    assert!(agents.exists(), "`init` writes AGENTS.md");
    let body = std::fs::read_to_string(&agents).unwrap();

    // Pillar 1: the single editing surface.
    assert!(
        body.contains("never the preview"),
        "AGENTS.md must state edit-the-source-never-the-preview; got:\n{body}"
    );
    // Pillar 2: the machine-readable check gate.
    assert!(
        body.contains("check") && body.contains("--format json"),
        "AGENTS.md must document the `check --format json` gate; got:\n{body}"
    );
    // Dialect, sourced from vocab():
    assert!(
        body.contains("[@key]"),
        "AGENTS.md must show the citation dialect"
    );
    assert!(
        body.contains("#| label:"),
        "AGENTS.md must show the cell-option dialect"
    );
    // A callout kind straight from the validator vocabulary.
    let kind = taliesin_core::vocab::vocab()["calloutKinds"][0]["name"]
        .as_str()
        .expect("at least one callout kind")
        .to_string();
    assert!(
        body.contains(&kind),
        "AGENTS.md must list the `{kind}` callout kind from vocab; got:\n{body}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The scaffolded file is byte-identical to the golden-locked generator output, so `init`
/// can never emit a stale onramp.
#[test]
fn scaffolded_agents_md_matches_the_generator() {
    let dir = tmp("bytes");
    let (ok, ..) = run(&["init", dir.to_str().unwrap()]);
    assert!(ok);
    let written = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
    assert_eq!(
        written,
        taliesin_core::agents::AGENTS_MD,
        "the scaffolded AGENTS.md must equal the bundled golden asset"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
