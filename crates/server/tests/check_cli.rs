//! `taliesin check --format json` is agent-grade: every diagnostic carries a stable
//! `code` and `severity`, and a "did you mean" typo carries a structured
//! `suggestion.replacement` an agent can apply. The `--format human` output is unchanged
//! (no codes leak into the linter-style lines).

use std::process::Command;

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> (bool, String, String) {
    run_env(args, &[])
}

fn run_env(args: &[&str], envs: &[(&str, &str)]) -> (bool, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_taliesin"));
    cmd.args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("run taliesin check");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Write `body` to a fresh temp `.tmd` file, returning its path.
fn tmp_doc(tag: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-check-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("doc.tmd");
    std::fs::write(&path, body).unwrap();
    path
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
fn check_human_output_surfaces_codes_and_an_explain_footer() {
    // PL1: the greppable linter line keeps its `file:line:` prefix + the message, and now also
    // surfaces the `severity[CODE]:` the JSON path always carried, plus a rustc-style
    // `--explain` footer — so the DX6 catalog is reachable from the output 99% of runs read.
    // The `docs_url` stays JSON-only (the code + footer are the human path back to the catalog).
    let (_ok, _stdout, stderr) = run(&["check", &corpus("diagnostics/typos.tmd")]);
    assert!(
        stderr.contains(
            "warning[TAL-FM-KEY]: unknown front-matter key `treme` (did you mean `theme`?)"
        ),
        "human line carries severity + code before the message: {stderr}"
    );
    assert!(
        stderr.contains("taliesin check --explain <CODE>"),
        "human output teaches --explain: {stderr}"
    );
    assert!(
        !stderr.contains("http"),
        "the docs_url stays JSON-only, never in human output: {stderr}"
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

#[test]
fn errors_only_filters_warnings_but_still_gates_on_errors() {
    // typos.tmd trips both an error (broken xref) and a warning (front-matter typo).
    let (ok, stdout, _e) = run(&[
        "check",
        &corpus("diagnostics/typos.tmd"),
        "--errors-only",
        "--format",
        "json",
    ]);
    assert!(!ok, "an error is present, so --errors-only still fails");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let diags = v["diagnostics"].as_array().expect("diagnostics array");
    assert!(!diags.is_empty(), "at least one error survives: {stdout}");
    assert!(
        diags.iter().all(|d| d["severity"] == "error"),
        "warnings are filtered out under --errors-only: {stdout}"
    );
}

#[test]
fn errors_only_passes_a_warning_only_document() {
    // `titel` is an unknown front-matter key (a WARNING); nothing here is an error.
    let doc = tmp_doc("warn-only", "---\ntitel: Oops\n---\n\n# Hi\n");
    let path = doc.to_str().unwrap();
    let (ok_default, _o, _e) = run(&["check", path]);
    assert!(!ok_default, "a warning fails the default gate (exit 1)");
    let (ok_errors_only, _o2, stderr) = run(&["check", path, "--errors-only"]);
    assert!(
        ok_errors_only,
        "with --errors-only a warning-only doc passes (exit 0): {stderr}"
    );
}

#[test]
fn require_kernel_gates_a_missing_interpreter() {
    // A doc with a {python} cell + a deliberately broken interpreter: `check` is static so
    // it finds no diagnostics (exit 0), but --require-kernel promotes the unrunnable kernel
    // to a failure. TALIESIN_PYTHON points at a path that cannot run, so this is
    // deterministic regardless of what's installed on the machine.
    let doc = tmp_doc(
        "needs-kernel",
        "---\ntitle: K\n---\n\n```{python}\n1 + 1\n```\n",
    );
    let path = doc.to_str().unwrap();
    let broken = [("TALIESIN_PYTHON", "/nonexistent/definitely-not-python")];
    let (ok_default, _o, _e) = run_env(&["check", path], &broken);
    assert!(
        ok_default,
        "static check ignores an unrunnable kernel by default (exit 0)"
    );
    let (ok_required, _o2, stderr) = run_env(&["check", path, "--require-kernel"], &broken);
    assert!(
        !ok_required,
        "--require-kernel fails when the kernel can't run"
    );
    assert!(
        stderr.contains("--require-kernel") && stderr.contains("python"),
        "the human note names the gate + the language: {stderr}"
    );
    // PL14: under --require-kernel the degraded environment block appears and points at doctor.
    assert!(
        stderr.contains("Environment (kernels not ready)") && stderr.contains("taliesin doctor"),
        "the degraded env block + a doctor pointer show under --require-kernel: {stderr}"
    );
}

#[test]
fn default_human_check_omits_the_environment_block() {
    // PL14: `check` is a static linter, so a default human run does NOT spawn interpreters or
    // print the Environment footer (it duplicated `doctor` on every keystroke/CI run). Forcing a
    // BROKEN interpreter makes this deterministic AND pins the probe-skip: if the default path
    // still probed, a broken interpreter would print a degraded "Environment …" block.
    let doc = tmp_doc(
        "static-check",
        "---\ntitle: S\n---\n\n```{python}\n1 + 1\n```\n",
    );
    let path = doc.to_str().unwrap();
    let broken = [("TALIESIN_PYTHON", "/nonexistent/definitely-not-python")];
    let (ok, _o, stderr) = run_env(&["check", path], &broken);
    assert!(
        ok,
        "static check passes without probing, even with a broken interpreter: {stderr}"
    );
    assert!(
        !stderr.contains("Environment"),
        "default human check omits the Environment block (it never probed): {stderr}"
    );
}

#[test]
fn json_check_still_carries_the_environment_probe() {
    // PL14 keeps `--format json` always-on: agents want the full interpreter/kernel probe, so the
    // `environment` array is present for a doc that uses a language (regardless of --require-kernel).
    let doc = tmp_doc(
        "json-env",
        "---\ntitle: J\n---\n\n```{python}\n1 + 1\n```\n",
    );
    let (_ok, stdout, _e) = run(&["check", doc.to_str().unwrap(), "--format", "json"]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let env = parsed["environment"]
        .as_array()
        .expect("environment array present");
    assert_eq!(env.len(), 1, "one entry for the python cell: {stdout}");
    assert_eq!(env[0]["lang"], "python");
}
