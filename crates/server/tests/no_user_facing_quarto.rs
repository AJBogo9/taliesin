//! Shed-Quarto invariant: no user-facing surface may name "Quarto".
//!
//! Taliesin is a standalone tool; once the migration on-ramps and comparison prose
//! were removed, "Quarto" must not reappear in the surfaces a user actually reads:
//! the CLI `--help` text, the top-level `README.md`, and the User Guide book
//! (`docs/guide`). Internal code comments, test names, `THIRD_PARTY.md`, the
//! `notes/*-QUARTO.md` planning/history files, and the Internals book (which keeps
//! accurate architectural contrast) are deliberately NOT covered by this gate.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Workspace root (this crate lives two levels down at `crates/server`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.tmd`/`.md` file under `dir`, recursively.
fn tmd_and_md(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(tmd_and_md(&p));
            } else if matches!(
                p.extension().and_then(|x| x.to_str()),
                Some("tmd") | Some("md")
            ) {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn cli_help_does_not_name_quarto() {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("--help")
        .output()
        .expect("run `taliesin --help`");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !text.to_lowercase().contains("quarto"),
        "CLI --help output names Quarto:\n{text}"
    );
}

#[test]
fn readme_and_guide_do_not_name_quarto() {
    let root = repo_root();

    // README.md is a single file; the guide is a whole book.
    let mut files = vec![root.join("README.md")];
    files.extend(tmd_and_md(&root.join("docs/guide")));

    let offenders: Vec<String> = files
        .iter()
        .filter(|f| {
            std::fs::read_to_string(f)
                .map(|txt| txt.to_lowercase().contains("quarto"))
                .unwrap_or(false)
        })
        .map(|f| f.display().to_string())
        .collect();

    assert!(
        offenders.is_empty(),
        "these user-facing files still name Quarto:\n{}",
        offenders.join("\n")
    );
}
