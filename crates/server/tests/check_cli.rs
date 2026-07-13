//! `taliesin check --format json` is agent-grade: every diagnostic carries a stable
//! `code` and `severity`, and a "did you mean" typo carries a structured
//! `suggestion.replacement` an agent can apply. The `--format human` output is unchanged
//! (no codes leak into the linter-style lines).

use std::process::Command;

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin check");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn check_json_diagnostics_carry_codes_and_suggestions() {
    let (_ok, stdout, _stderr) = run(&[
        "check",
        &corpus("diagnostics/typos.tmd"),
        "--format",
        "json",
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let diags = parsed["diagnostics"].as_array().expect("diagnostics array");
    assert!(!diags.is_empty(), "typos.tmd trips diagnostics: {stdout}");

    // Every diagnostic carries a stable, non-empty code and a severity.
    for d in diags {
        let code = d["code"].as_str().unwrap_or("");
        assert!(code.starts_with("TAL-"), "stable code, got: {d}");
        let sev = d["severity"].as_str().unwrap_or("");
        assert!(sev == "error" || sev == "warning", "severity, got: {d}");
    }

    // The `treme` front-matter typo carries the structured replacement.
    let treme = diags
        .iter()
        .find(|d| d["message"].as_str().unwrap_or("").contains("`treme`"))
        .expect("treme diagnostic");
    assert_eq!(treme["code"], "TAL-FM-KEY");
    assert_eq!(treme["suggestion"]["replacement"], "theme");

    // The broken cross-reference carries its label suggestion + the xref code.
    let xref = diags
        .iter()
        .find(|d| {
            d["message"]
                .as_str()
                .unwrap_or("")
                .contains("broken cross-reference")
        })
        .expect("xref diagnostic");
    assert_eq!(xref["code"], "TAL-XREF-UNDEF");
    assert_eq!(xref["suggestion"]["replacement"], "@fig-results");
}

#[test]
fn check_human_output_is_unchanged_by_codes() {
    // Human format is the greppable `file: message` linter line on stderr; codes/severity
    // must NOT leak into it (byte-identical to before this feature).
    let (_ok, _stdout, stderr) = run(&["check", &corpus("diagnostics/typos.tmd")]);
    assert!(
        stderr.contains("unknown front-matter key `treme` (did you mean `theme`?)"),
        "human message unchanged: {stderr}"
    );
    assert!(
        !stderr.contains("TAL-FM-KEY"),
        "codes must not leak into human output: {stderr}"
    );
}
