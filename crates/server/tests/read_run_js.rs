//! `taliesin read --run` also observes `{js}` cells headlessly (DX17b): a `{js}` cell runs
//! in the browser, so `read --run` drives a local headless Chrome over the built page and
//! reports whether an `<svg>`/`<canvas>` painted, the real error when it threw, or a skip
//! when no Chrome is available.
//!
//! The no-Chrome degradation runs unconditionally (it forces a missing `CHROME_PATH`). The
//! live cases are gated on a system Chrome; `TALIESIN_REQUIRE_CHROME=1` turns the skip into
//! a hard failure so this coverage can't silently regress to zero (mirrors
//! `TALIESIN_REQUIRE_KERNEL`).

use std::path::PathBuf;
use std::process::Command;

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

const DOC: &str = "agent/executed-read-js.tmd";

/// The first system Chrome (mirrors `headless_js::chrome_path`): `$CHROME_PATH` if it
/// exists, else a known binary on `$PATH`.
fn which_chrome() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CHROME_PATH") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    for name in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
    ] {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// `true` when a live Chrome case should run. `false` (skip) unless
/// `TALIESIN_REQUIRE_CHROME=1`, which makes a missing Chrome a hard failure.
fn have_chrome() -> bool {
    if which_chrome().is_some() {
        return true;
    }
    assert!(
        std::env::var_os("TALIESIN_REQUIRE_CHROME").is_none(),
        "TALIESIN_REQUIRE_CHROME=1 but no system Chrome found: read --run {{js}} would go untested"
    );
    eprintln!("skipping: no system Chrome (set CHROME_PATH or install google-chrome/chromium)");
    false
}

#[test]
fn read_run_js_without_chrome_skips_and_exits_zero() {
    // Forcing a missing CHROME_PATH must degrade every `{js}` cell to a skip, never a hard
    // failure — and never depend on the host actually having (or lacking) Chrome.
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["read", "--run", "--format", "json", &corpus(DOC)])
        .env("CHROME_PATH", "/nonexistent/definitely-not-chrome")
        .output()
        .expect("run taliesin");
    assert!(
        out.status.success(),
        "no-Chrome read --run must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    let cells = v["cells"].as_array().expect("cells array");
    assert_eq!(cells.len(), 2, "both {{js}} cells listed: {v}");
    assert!(
        cells
            .iter()
            .all(|c| c["kind"] == "skipped" && c["detail"] == "chrome unavailable"),
        "every js cell skips without Chrome: {v}"
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("[js: skipped (chrome unavailable)]"),
        "text projection carries the skip line: {v}"
    );
}

#[test]
fn read_run_js_reports_svg_produced_and_error_kinds() {
    if !have_chrome() {
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["read", "--run", "--format", "json", &corpus(DOC)])
        .output()
        .expect("run taliesin");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(v["executed"], true);
    let cells = v["cells"].as_array().expect("cells array");

    let produced = cells
        .iter()
        .find(|c| c["kind"] == "js")
        .expect("the Plot cell reports kind=js");
    assert_eq!(produced["produced"], true, "Plot cell produced: {v}");
    assert!(
        produced["detail"]
            .as_str()
            .is_some_and(|d| d.starts_with("svg")),
        "Plot cell detail names an svg with dims: {v}"
    );

    let errored = cells
        .iter()
        .find(|c| c["kind"] == "js-error")
        .expect("the throwing cell reports kind=js-error");
    assert_eq!(errored["produced"], false, "errored cell not produced: {v}");
    assert!(
        errored["error"]
            .as_str()
            .is_some_and(|e| e.contains("intentional read --run test failure")),
        "the real browser error surfaces (not the terse reader message): {v}"
    );
}

#[test]
fn read_run_js_text_projects_produced_and_error() {
    if !have_chrome() {
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["read", "--run", &corpus(DOC)])
        .output()
        .expect("run taliesin");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("[js: produced, <svg"),
        "produced svg line missing: {text}"
    );
    assert!(
        text.contains("[js error: "),
        "js error line missing: {text}"
    );
}
