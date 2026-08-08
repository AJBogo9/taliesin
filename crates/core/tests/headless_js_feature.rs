//! The browser driver is a **test-only build feature**, and that fact is written in four
//! files that have to agree. Measured on a clean release build (2026-07-28, `-j3`):
//! `chromiumoxide` + `chromiumoxide_cdp` are the two most expensive units in the entire
//! 344-unit graph, and everything reachable only through them costs **81 of 336
//! CPU-seconds — 24% of the build**.
//!
//! **As of 2026-08-08 nothing a released binary can run touches it.** `read --run-js` went
//! with the machine-facing verbs in Wave 2, `pdf` went with the print track in Wave 4, and
//! `deck_browser` went with the slide-deck engine in Wave 5, so `reactive_browser` is the
//! single consumer left. That is why `release.yml` no longer asks for the feature and this
//! file no longer checks that it does.
//!
//! Each direction of drift fails differently, and the quiet one is the dangerous one:
//!
//! - **Un-optionalise the dependency, or put `headless-js` back in `default`,** and every
//!   build silently pays that 24% again. Nothing else would notice: the tree still
//!   compiles and every test still passes.
//! - **Drop `--features taliesin-server/headless-js` from `gates.sh` or `ci.yml`,** and
//!   the browser test binary stops being built at all — `required-features` makes
//!   cargo skip them without a word. `TALIESIN_REQUIRE_CHROME` cannot catch that, because
//!   the tests it guards no longer exist to be skipped. The suite shrinks and stays green.
//!
//! This is the "a fix lands in one file and misses its sibling" failure this repo has hit
//! three times, so it is gated on the shape rather than on the sentence.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// The manifest lines that matter, with comments and blank lines dropped so a prose
/// mention of `optional` in a comment cannot satisfy any assertion below.
fn server_manifest_code() -> String {
    read("crates/server/Cargo.toml")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Note when verifying this one by mutation: deleting *only* `optional = true` does not
/// reach the assertion, because `headless-js = ["dep:chromiumoxide"]` then refers to a
/// non-optional dependency and cargo refuses to parse the workspace manifest at all. The
/// assertion earns its place on the mutation that *does* parse — dropping the feature's
/// `dep:` entry and the `optional` flag together.
#[test]
fn the_browser_driver_is_an_optional_dependency() {
    let m = server_manifest_code();
    let decl = m
        .lines()
        .find(|l| l.starts_with("chromiumoxide "))
        .unwrap_or_else(|| panic!("no `chromiumoxide` dependency line in the server manifest"));
    assert!(
        decl.contains("optional = true"),
        "`chromiumoxide` must stay `optional = true`, or every build pays 24% of its time \
         for a browser driver most users never invoke. Found: {decl}"
    );
}

#[test]
fn the_headless_js_feature_is_not_on_by_default() {
    let m = server_manifest_code();
    assert!(
        m.contains("headless-js = [\"dep:chromiumoxide\"]"),
        "the `headless-js` feature must gate the driver: expected \
         `headless-js = [\"dep:chromiumoxide\"]` in [features]"
    );
    let default = m
        .lines()
        .find(|l| l.starts_with("default = "))
        .unwrap_or_else(|| panic!("[features] must state `default` explicitly, even if empty"));
    assert!(
        !default.contains("headless-js"),
        "`headless-js` is back in the default feature set, which un-does the whole point: \
         every build pays for the driver again. Found: {default}"
    );
}

#[test]
fn every_browser_test_binary_declares_the_feature_it_needs() {
    // One binary left as of Wave 5; written as a list so a second one is a row, not a rewrite.
    const BROWSER_TEST_BINARIES: &[&str] = &["reactive_browser"];
    let m = server_manifest_code();
    for name in BROWSER_TEST_BINARIES {
        let decl = m
            .split("[[test]]")
            .find(|s| s.contains(&format!("name = \"{name}\"")))
            .unwrap_or_else(|| {
                panic!(
                    "`{name}` needs a `[[test]]` entry with \
                     `required-features = [\"headless-js\"]` — without one it fails to \
                     compile in a default build, because it names `chromiumoxide` directly"
                )
            });
        assert!(
            decl.contains("required-features = [\"headless-js\"]"),
            "`{name}` must declare `required-features = [\"headless-js\"]`"
        );
    }
}

/// Every place that runs the browser tests has to ask for the feature. Derived from the
/// files themselves rather than restated, so a renamed feature fails here instead of going
/// quiet. `release.yml` is deliberately NOT in this list — see the module doc.
#[test]
fn every_caller_that_needs_the_driver_asks_for_it() {
    const FLAG: &str = "--features taliesin-server/headless-js";

    // (a) The one script that runs every gate. Its chrome canary is asserted BY NAME, so
    //     dropping the flag here would report as a missing canary — but only if the flag
    //     is on the `cargo test` line and not merely mentioned in a comment.
    let gates = read("tools/gates.sh");
    let gate_cmd = gates
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("cargo test --workspace"))
        .unwrap_or_else(|| panic!("gates.sh no longer runs `cargo test --workspace`"));
    assert!(
        gate_cmd.contains(FLAG),
        "gates.sh must run the workspace suite with `{FLAG}`, or cargo skips both browser \
         test binaries (`required-features`) and the chrome gate guards nothing. Found: \
         {gate_cmd}"
    );

    // (b) CI's own test step — the same trap, on the surface a stranger reads as proof.
    let ci = read(".github/workflows/ci.yml");
    let ci_cmd = ci
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("run: cargo test --workspace"))
        .unwrap_or_else(|| panic!("ci.yml no longer runs `cargo test --workspace`"));
    assert!(
        ci_cmd.contains(FLAG),
        "ci.yml's test step must pass `{FLAG}`; it sets TALIESIN_REQUIRE_CHROME, which is \
         inert when the tests it guards were never built. Found: {ci_cmd}"
    );

    // (c) The inverse, on the release build: asking for the feature there costs 24% of
    //     every cross-build for a driver no shipped code path can reach. It is not a
    //     capability loss, so it must not creep back in as one.
    let release = read(".github/workflows/release.yml");
    let build_cmd = release
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("run: cargo build --release"))
        .unwrap_or_else(|| panic!("release.yml no longer builds with `cargo build --release`"));
    assert!(
        !build_cmd.contains(FLAG),
        "release.yml asks for `{FLAG}`, but no runtime code path uses the driver since \
         `pdf` was cut on 2026-08-08 — every cross-build would pay 24% for a dependency \
         the binary cannot reach. Found: {build_cmd}"
    );
}
