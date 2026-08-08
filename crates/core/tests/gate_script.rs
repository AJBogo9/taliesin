//! `tools/gates.sh` is the one committed script that runs every gate, and its whole value
//! is that a green run means every gate *ran*. Two things can silently hollow it out, and
//! neither is visible from inside the script:
//!
//! 1. A new `TALIESIN_REQUIRE_*` gate is added to the Rust sources and nobody arms it in
//!    the script, so that gate skips on every run of the script that exists to stop skips.
//! 2. A canary test is renamed. The script greps for `test <name> ... ok`; a renamed
//!    canary makes that grep fail, which is loud — but only if someone runs the script.
//!    Worse, a canary *deleted* along with its coverage leaves the script asserting on a
//!    name that will never appear, and the failure reads as a tooling bug.
//!
//! So derive both lists from the tree rather than from memory: every REQUIRE variable the
//! sources read must be set in the script, and every canary the script names must still
//! exist as a function.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Every `.rs` file under `crates/`, so the scan follows the tree instead of a
/// hand-maintained file list (the shape that let `deny.toml`'s second false CI claim
/// survive its own gate).
fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                // `target/` holds generated copies of the same sources.
                if p.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo_root().join("crates"), &mut out);
    assert!(!out.is_empty(), "found no Rust sources under crates/");
    out
}

#[test]
fn the_gate_script_exists_and_is_executable() {
    let p = repo_root().join("tools/gates.sh");
    assert!(p.is_file(), "tools/gates.sh is missing");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&p).unwrap().permissions().mode();
        assert!(
            mode & 0o111 != 0,
            "tools/gates.sh is not executable (mode {mode:o}); a documented `./tools/gates.sh` \
             that needs `bash tools/gates.sh` is a first-contact papercut"
        );
    }
}

/// Every interpreter gate the Rust sources read is armed by the script. A gate the script
/// does not set is a gate that skips, which is the exact failure this script exists to
/// make impossible.
#[test]
fn the_gate_script_arms_every_require_gate_in_the_tree() {
    let script = read("tools/gates.sh");

    let mut found: Vec<String> = Vec::new();
    for path in rust_sources() {
        let text = std::fs::read_to_string(&path).unwrap();
        let mut rest = text.as_str();
        while let Some(i) = rest.find("TALIESIN_REQUIRE_") {
            let tail = &rest[i..];
            let end = tail
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(tail.len());
            let name = tail[..end].to_string();
            // Doc comments say `TALIESIN_REQUIRE_*` to mean "the family"; that bare prefix
            // is a mention, not a gate.
            if name.len() > "TALIESIN_REQUIRE_".len() && !found.contains(&name) {
                found.push(name);
            }
            rest = &tail[end..];
        }
    }
    found.sort();

    // A tree with no REQUIRE gates would make the loop below vacuous, and this test would
    // pass while proving nothing.
    assert!(
        found.len() >= 4,
        "expected at least the four known interpreter gates, found {found:?}"
    );

    for var in &found {
        assert!(
            script.contains(&format!("{var}=1")),
            "`{var}` gates a test in the tree but tools/gates.sh never sets it, so that \
             gate skips on the one run that is supposed to prove nothing skipped. \
             Gates found in the tree: {found:?}"
        );
    }
}

/// `CONTRIBUTING.md` is the only tracked file that carries the `core.hooksPath` wiring —
/// which git does not do for you, so without that line a contributor's push is gated by
/// nothing — and the only one that routes a stranger to the gate script. Both claims are
/// worth exactly as much as the things they name, so pin the pair.
#[test]
fn contributing_wires_the_hooks_path_and_points_at_the_gate_script() {
    let doc = read("CONTRIBUTING.md");

    assert!(
        doc.contains("git config core.hooksPath .githooks"),
        "CONTRIBUTING.md no longer carries the `core.hooksPath` wiring command, and no \
         other tracked file does: a fresh clone runs no gate on push and nothing says so"
    );
    assert!(
        repo_root().join(".githooks/pre-push").is_file(),
        "CONTRIBUTING.md points core.hooksPath at .githooks, but .githooks/pre-push is gone"
    );
    assert!(
        doc.contains("./tools/gates.sh"),
        "CONTRIBUTING.md no longer names the gate script, so a contributor's only \
         instruction is `cargo test`, which skips silently"
    );
    // The licensing-continuity half: the README reserves the right to relicense, which
    // only survives if inbound contributions carry a grant that permits it. A
    // CONTRIBUTING.md without one quietly ends that reservation on the first merged PR.
    assert!(
        doc.contains("relicense"),
        "CONTRIBUTING.md states no inbound relicensing grant, but README.md reserves the \
         right to relicense — the first merged contribution would make that false"
    );
}

/// Every canary the script names still exists. The script's proof that (say) the R kernel
/// ran is `grep 'test r_cells_execute_and_persist_state_across_cells ... ok'`; rename that
/// test and the proof becomes an assertion about a name nothing emits.
#[test]
fn every_canary_the_gate_script_names_still_exists() {
    let script = read("tools/gates.sh");

    let canaries: Vec<String> = script
        .lines()
        .filter_map(|l| l.strip_prefix("CANARY_"))
        .filter_map(|l| l.split_once('='))
        .map(|(_, v)| v.trim().trim_matches('"').to_string())
        .collect();

    assert_eq!(
        canaries.len(),
        5,
        "expected one canary per interpreter gate (python, R, node) plus the two \
         browser-backed capabilities that stand for chrome (the reactive client's render \
         and the print track's pagination). Every canary that has ever been dropped was \
         dropped because the ONLY thing it proved went away, never because another canary \
         was made to cover for it — a canary repointed at a surviving test would leave two \
         proving the same thing and one capability proving nothing. The `#| trace: true` \
         settrace harness was the sixth until Wave 3 cut debug mode: it was independent of \
         the plain python-kernel canary (that one proves a kernel runs, this one proved the \
         harness runs inside it), so nothing inherits it and the python gate is unchanged — \
         `CANARY_KERNEL` still fails when ipykernel is missing. `read --run`'s \
         headless-`{{js}}` observation was chrome's own canary until Wave 2 cut the \
         machine-facing verbs — the two that remain still fail when chrome is missing, so \
         that gate is unchanged too. The math hover's browser render was the eleventh until \
         Wave 4.1 cut the rasterizer, the figure lightbox's was the tenth until the visual \
         minimalism pass deleted it, and the `{{pyodide}}` runtime's plus its two \
         cargo-feature guards were the seventh through ninth until that language was \
         withdrawn, got {canaries:?}"
    );

    let sources: Vec<String> = rust_sources()
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect();

    for canary in &canaries {
        let needle = format!("fn {canary}(");
        assert!(
            sources.iter().any(|s| s.contains(&needle)),
            "tools/gates.sh asserts the canary `{canary}` reports ok, but no `{needle}` \
             exists in the tree — it was renamed or deleted, and the script now proves \
             nothing about the interpreter it stands for"
        );
    }
}
