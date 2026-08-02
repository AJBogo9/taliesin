//! The vendored Pyodide runtime is an **opt-in build feature** (backlog item 205), and that
//! fact is written in six files that have to agree. The twin of `headless_js_feature.rs`,
//! deliberately: same failure shape, same gate shape.
//!
//! Measured 2026-08-02: the payload is 16,455,447 B, **21.8% of the 75.6 MB binary**, for a
//! capability one showcase page uses. It is also what put `taliesin-core` at 24.2 MiB against
//! crates.io's 10 MiB `.crate` cap, which a cargo feature does *not* fix; only `exclude` does.
//! Hence two independent levers in one manifest.
//!
//! Each direction of drift fails differently, and the quiet ones are the dangerous ones:
//!
//! - **Put `pyodide` in `default`, or drop the `#[cfg]` on `pyodide_payload`,** and every
//!   build silently carries 15.7 MiB again. Nothing else notices: the tree compiles and every
//!   test passes.
//! - **Drop `exclude`,** and `cargo publish` stays blocked on a 2.4x-over-cap `.crate`, which
//!   surfaces only at publish time.
//! - **Drop `--features taliesin-server/pyodide` from `gates.sh` or `ci.yml`,** and
//!   `crates/core/tests/pyodide.rs` stops being built at all (`required-features` makes cargo
//!   skip it without a word), while four more `#[cfg]`'d tests vanish from files that stay
//!   green. The suite shrinks and stays green, which is exactly the shape `gates.sh` asserting
//!   canaries BY NAME exists to catch.
//! - **Drop it from `release.yml`,** and every downloaded binary silently loses `{pyodide}`
//!   cells, the audience that did not pay the build cost and should get the complete tool.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Manifest lines with comments and blanks dropped, so a prose mention in a comment cannot
/// satisfy any assertion below.
fn manifest_code(rel: &str) -> String {
    read(rel)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_payload_feature_exists_and_is_not_on_by_default() {
    let core = manifest_code("crates/core/Cargo.toml");
    assert!(
        core.contains("pyodide = []"),
        "crates/core must declare the `pyodide` feature in [features]"
    );

    let server = manifest_code("crates/server/Cargo.toml");
    assert!(
        server.contains("pyodide = [\"taliesin-core/pyodide\"]"),
        "crates/server must re-export the feature so the workspace-root spelling matches \
         `headless-js` (`--features taliesin-server/pyodide`)"
    );
    let default = server
        .lines()
        .find(|l| l.starts_with("default = "))
        .unwrap_or_else(|| panic!("[features] must state `default` explicitly, even if empty"));
    assert!(
        !default.contains("pyodide"),
        "`pyodide` is back in the default feature set, which un-does the whole point: every \
         build carries 15.7 MiB again. Found: {default}"
    );
}

/// The `.crate` lever, which is independent of the feature and fixes a different blocker.
#[test]
fn the_vendored_payload_is_excluded_from_the_published_crate() {
    let core = manifest_code("crates/core/Cargo.toml");
    assert!(
        core.contains("exclude = [\"assets/pyodide/*\"]"),
        "crates/core must `exclude` the vendored payload from `cargo package`: a cargo \
         feature does not shrink a `.crate`, and 24.2 MiB is 2.4x over crates.io's cap"
    );
}

/// The payload must be reachable only through a `#[cfg]`'d `include_bytes!`, or the feature
/// gates nothing.
#[test]
fn the_only_include_bytes_of_the_payload_is_feature_gated() {
    let src = read("crates/core/src/render/pyodide.rs");
    // Exactly one site interpolates the payload directory into an `include_bytes!`.
    let sites = src
        .matches("include_bytes!(concat!(\"../../assets/pyodide/")
        .count();
    assert_eq!(
        sites, 1,
        "expected exactly one `include_bytes!` of the pyodide payload (the `payload_file!` \
         macro); a second one would not be covered by the single `#[cfg]` below"
    );
    assert!(
        src.contains("#[cfg(feature = \"pyodide\")]\nmacro_rules! payload_file"),
        "the `payload_file!` macro must be feature-gated, or the bytes ride in regardless"
    );
    assert!(
        src.contains("#[cfg(not(feature = \"pyodide\"))]"),
        "there must be a feature-off `pyodide_payload` returning no bytes, so every caller \
         compiles either way"
    );

    // And no other source file may `include_bytes!` from that directory.
    for dir in ["crates/core/src", "crates/server/src"] {
        let mut found = Vec::new();
        walk_rs(&repo_root().join(dir), &mut found);
        for f in found {
            if f.ends_with("render/pyodide.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap_or_default();
            assert!(
                !text.contains("assets/pyodide/"),
                "{f} references `assets/pyodide/` directly; the payload must stay behind \
                 `pyodide_payload()` so one `#[cfg]` gates all of it"
            );
        }
    }
}

fn walk_rs(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p.display().to_string());
        }
    }
}

/// The delivery tests must declare the feature they need, or they silently stop being built.
#[test]
fn the_delivery_test_target_declares_the_feature_it_needs() {
    let core = manifest_code("crates/core/Cargo.toml");
    let decl = core
        .split("[[test]]")
        .find(|s| s.contains("name = \"pyodide\""))
        .unwrap_or_else(|| {
            panic!(
                "`crates/core/tests/pyodide.rs` needs a `[[test]]` entry with \
                 `required-features = [\"pyodide\"]`: feature-off its assertions do not \
                 merely fail, several pass VACUOUSLY (there is no wrapper for \
                 `degrade_pyodide_cells` to strip)"
            )
        });
    assert!(
        decl.contains("required-features = [\"pyodide\"]"),
        "the `pyodide` test target must declare `required-features = [\"pyodide\"]`"
    );

    // The browser test needs both: a driver to drive, and a runtime to drive it against.
    let server = manifest_code("crates/server/Cargo.toml");
    let browser = server
        .split("[[test]]")
        .find(|s| s.contains("name = \"pyodide_browser\""))
        .unwrap_or_else(|| panic!("no `[[test]]` entry for `pyodide_browser`"));
    for f in ["headless-js", "pyodide"] {
        assert!(
            browser.contains(&format!("\"{f}\"")),
            "`pyodide_browser` must require `{f}`; it boots the real runtime in a real browser"
        );
    }
}

/// Every place that runs the delivery tests or ships a binary has to ask for the feature.
/// Derived from the files themselves rather than restated, so a rename fails here.
#[test]
fn every_caller_that_needs_the_runtime_asks_for_it() {
    const FLAG: &str = "taliesin-server/pyodide";

    let gates = read("tools/gates.sh");
    let gate_cmd = gates
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("cargo test --workspace"))
        .unwrap_or_else(|| panic!("gates.sh no longer runs `cargo test --workspace`"));
    assert!(
        gate_cmd.contains(FLAG),
        "gates.sh must run the workspace suite with `--features …,{FLAG}`, or cargo skips \
         the pyodide delivery target (`required-features`) and four `#[cfg]`'d tests vanish \
         from files that stay green. Found: {gate_cmd}"
    );

    let ci = read(".github/workflows/ci.yml");
    let ci_cmd = ci
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("run: cargo test --workspace"))
        .unwrap_or_else(|| panic!("ci.yml no longer runs `cargo test --workspace`"));
    assert!(
        ci_cmd.contains(FLAG),
        "ci.yml's test step must pass `{FLAG}`. Found: {ci_cmd}"
    );

    let release = read(".github/workflows/release.yml");
    let build_cmd = release
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("run: cargo build --release"))
        .unwrap_or_else(|| panic!("release.yml no longer builds with `cargo build --release`"));
    assert!(
        build_cmd.contains(FLAG),
        "release.yml must build with `{FLAG}`: a DOWNLOADED binary should be the complete \
         tool, the same policy `headless-js` follows. Found: {build_cmd}"
    );
}

/// `gates.sh` must assert the two feature canaries by name, at both gating altitudes.
#[test]
fn the_gate_script_names_a_canary_for_each_gating_altitude() {
    let gates = read("tools/gates.sh");
    for canary in [
        // whole target, via `required-features`
        "a_single_file_build_degrades_a_pyodide_cell_to_visible_source",
        // a lone `#[cfg]`'d test inside an otherwise-ungated file
        "site_build_copies_the_pyodide_runtime_and_stamps_a_page_relative_index",
    ] {
        assert!(
            gates.contains(canary),
            "gates.sh must name `{canary}` as a canary; the two gating mechanisms fail \
             differently and one canary cannot cover both"
        );
    }
}
