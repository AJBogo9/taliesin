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

#[test]
fn check_json_diagnostics_carry_a_docs_url() {
    let (_ok, stdout, _stderr) = run(&[
        "check",
        &corpus("diagnostics/typos.tmd"),
        "--format",
        "json",
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let diags = parsed["diagnostics"].as_array().expect("diagnostics array");
    assert!(!diags.is_empty());
    for d in diags {
        let url = d["docs_url"].as_str().unwrap_or("");
        let code = d["code"].as_str().unwrap_or("");
        assert!(
            url.starts_with("https://github.com/AJBogo9/taliesin")
                && url.ends_with(&code.to_ascii_lowercase()),
            "each diagnostic carries a code-anchored docs_url: {d}"
        );
    }
}

#[test]
fn explain_human_prints_cause_and_fix() {
    // `--explain <CODE>` needs no file; it prints the code's cause + fix + a docs url to
    // stdout and exits 0.
    let (ok, stdout, _stderr) = run(&["check", "--explain", "TAL-XREF-UNREF"]);
    assert!(ok, "known code exits 0");
    assert!(
        stdout.starts_with("TAL-XREF-UNREF:"),
        "titled block: {stdout}"
    );
    assert!(stdout.contains("To fix:"), "has a fix: {stdout}");
    assert!(
        stdout.contains("Learn more: https://"),
        "has a url: {stdout}"
    );
}

#[test]
fn explain_json_is_structured() {
    let (ok, stdout, _stderr) = run(&["check", "--explain", "TAL-FM-KEY", "--format", "json"]);
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(v["code"], "TAL-FM-KEY");
    for k in ["title", "cause", "fix", "docs_url"] {
        assert!(v[k].is_string(), "{k} present: {stdout}");
    }
}

#[test]
fn explain_unknown_code_exits_nonzero() {
    // Human: a friendly did-you-mean on stderr, non-zero exit.
    let (ok, _stdout, stderr) = run(&["check", "--explain", "TAL-XREF-UNDEFF"]);
    assert!(!ok, "unknown code exits non-zero");
    assert!(
        stderr.contains("unknown diagnostic code"),
        "friendly error: {stderr}"
    );
    // JSON: a valid {"error": ...} envelope on stdout, so a `| jq` stays parseable.
    let (ok2, stdout2, _e) = run(&["check", "--explain", "NOPE", "--format", "json"]);
    assert!(!ok2);
    let v: serde_json::Value =
        serde_json::from_str(&stdout2).expect("error envelope is valid json");
    assert!(
        v["error"].as_str().unwrap_or("").contains("NOPE"),
        "error names the code: {stdout2}"
    );
}

#[test]
fn explain_with_no_code_lists_the_index() {
    // Bare `--explain` lists every code (human) and exits 0; `--explain --format json` is the
    // index in JSON (code = None, not "code = --format").
    let (ok, stdout, _stderr) = run(&["check", "--explain"]);
    assert!(ok);
    assert!(
        stdout.contains("TAL-FM-KEY") && stdout.contains("TAL-XREF-UNREF"),
        "index: {stdout}"
    );
    let (ok2, stdout2, _e) = run(&["check", "--explain", "--format", "json"]);
    assert!(ok2);
    let v: serde_json::Value = serde_json::from_str(&stdout2).expect("valid json");
    assert!(
        v["codes"].is_array(),
        "json index is an array under `codes`: {stdout2}"
    );
}
