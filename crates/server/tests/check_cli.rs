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
        assert!(
            sev == "error" || sev == "warning" || sev == "suggestion",
            "severity, got: {d}"
        );
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
fn default_human_check_names_the_interpreter_without_spawning_it() {
    // Item 122, superseding PL14's `default_human_check_omits_the_environment_block`.
    //
    // PL14 was right that `check` must not SPAWN on every keystroke, and wrong to conclude it
    // must therefore say NOTHING: a document whose only code cell cannot run reported "no
    // problems found", exit 0, while `build` on the same file warned twice. The ruled shape
    // separates the two halves PL14 conflated — NAME the interpreter unconditionally, PROBE it
    // only on request.
    //
    // A BROKEN interpreter keeps this deterministic and pins both halves at once: the path must
    // be NAMED (so the silence is gone) and must NOT be reported as broken (so nothing spawned
    // it). A mutant that reinstates the probe fails on the second assert, not the first.
    let doc = tmp_doc(
        "static-check",
        "---\ntitle: S\n---\n\n```{python}\n1 + 1\n```\n",
    );
    let path = doc.to_str().unwrap();
    let broken = [("TALIESIN_PYTHON", "/nonexistent/definitely-not-python")];
    let (ok, _o, stderr) = run_env(&["check", path], &broken);
    assert!(
        ok,
        "naming an interpreter never changes the exit code: {stderr}"
    );
    assert!(
        stderr.contains("Environment (not probed)"),
        "a doc with a code cell names the environment it would use: {stderr}"
    );
    assert!(
        stderr.contains("/nonexistent/definitely-not-python"),
        "the line names the interpreter that WOULD be used: {stderr}"
    );
    // The probe-skip, stated positively: nothing spawned that binary, so `check` must not
    // claim to know it is broken. This is the assert PL14's concern actually needed.
    assert!(
        !stderr.contains("interpreter not found or failed to run")
            && !stderr.contains("MISSING")
            && !stderr.contains("kernels not ready"),
        "check never reports a verdict on a binary it did not run: {stderr}"
    );
}

#[test]
fn a_document_with_no_code_cell_prints_no_environment_line() {
    // The other half of item 122's contract, and the one that keeps PL14's real win: a prose
    // document must stay silent. Without this, "name it unconditionally" would print a python
    // line on every README-shaped page in the tree.
    let doc = tmp_doc("prose-only", "---\ntitle: P\n---\n\nJust prose.\n");
    let (ok, _o, stderr) = run_env(&["check", doc.to_str().unwrap()], &[]);
    assert!(ok, "clean prose passes: {stderr}");
    assert!(
        !stderr.contains("Environment"),
        "no code cell, no environment line: {stderr}"
    );
}

#[test]
fn require_kernel_still_reports_a_probed_verdict() {
    // The `--require-kernel` path is unchanged by item 122: it is the opt-in that SPAWNS, so it
    // is the only surface allowed to say a kernel is not ready. Pins that the new unconditional
    // line did not swallow the probed one — with a broken interpreter this must fail AND name
    // the failure, where the default run above passes and names nothing.
    let doc = tmp_doc(
        "require-kernel",
        "---\ntitle: R\n---\n\n```{python}\n1 + 1\n```\n",
    );
    let broken = [("TALIESIN_PYTHON", "/nonexistent/definitely-not-python")];
    let (ok, _o, stderr) = run_env(
        &["check", doc.to_str().unwrap(), "--require-kernel"],
        &broken,
    );
    assert!(
        !ok,
        "--require-kernel fails on an unrunnable kernel: {stderr}"
    );
    assert!(
        stderr.contains("Environment (kernels not ready)"),
        "the probed path keeps its degraded block: {stderr}"
    );
    assert!(
        !stderr.contains("Environment (not probed)"),
        "a probed run does not also claim it declined to probe: {stderr}"
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

#[test]
fn check_json_front_matter_typo_carries_a_column_span() {
    let (_ok, stdout, _e) = run(&[
        "check",
        &corpus("diagnostics/typos.tmd"),
        "--format",
        "json",
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let diags = parsed["diagnostics"].as_array().expect("diagnostics array");
    // The `treme` front-matter key typo squiggles exactly its 5-char key at column 1.
    let treme = diags
        .iter()
        .find(|d| d["message"].as_str().unwrap_or("").contains("`treme`"))
        .expect("treme diagnostic present");
    assert_eq!(treme["col"], 1);
    assert_eq!(treme["end_col"], 6);
    // An un-columned finding (the broken xref is HTML-derived, block-line only) must NOT
    // carry the keys, so its JSON stays byte-identical to before E3.
    let xref = diags
        .iter()
        .find(|d| d["code"] == "TAL-XREF-UNDEF")
        .expect("xref diagnostic present");
    assert!(
        xref.get("col").is_none(),
        "un-columned diag omits col: {xref}"
    );
    assert!(
        xref.get("end_col").is_none(),
        "un-columned diag omits end_col: {xref}"
    );
}

// --- the three-state severity floor (SKIM-3a) ---------------------------------------
// `check` used to exit non-zero on ANY diagnostic, which made an advice-shaped rule
// impossible: a style suggestion was reported as an ERROR and failed `check`,
// `build --strict` and `publish` (strict by default). The floor is now three-state, so
// advice is printed and gates only when asked for.
//
// These drove `corpus/diagnostics/prose.tmd` until the prose linter was retired on
// 2026-08-02. The advice source is now the shape lint (`TAL-SHAPE-DUP`, two headings that
// read the same), which needs no opt-in — a better fixture for this floor than a rule a
// document had to ask for.

/// A document whose only findings are advice.
fn advice_only_doc(name: &str) -> std::path::PathBuf {
    tmp_doc(name, "---\ntitle: T\n---\n\n## Same\n\na\n\n## Same\n\nb\n")
}

#[test]
fn advice_is_reported_at_severity_suggestion_and_passes_the_default_gate() {
    let doc = advice_only_doc("advice-default-gate");
    let (ok, stdout, _e) = run(&["check", doc.to_str().unwrap(), "--format", "json"]);
    assert!(
        ok,
        "a document whose only findings are advice passes: {stdout}"
    );
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let diags = v["diagnostics"].as_array().expect("diagnostics array");
    assert!(!diags.is_empty(), "the shape lint still fires: {stdout}");
    // Reported, not swallowed — and every one of them is advice, so nothing gates.
    assert!(
        diags.iter().all(|d| d["severity"] == "suggestion"),
        "every finding here is advice: {stdout}"
    );
    // Each carries its own family code, not the generic fallback.
    assert!(
        diags.iter().all(|d| d["code"]
            .as_str()
            .is_some_and(|c| c.starts_with("TAL-SHAPE-"))),
        "shape findings carry their own codes: {stdout}"
    );
}

#[test]
fn strict_gates_on_advice_and_errors_only_hides_it() {
    let doc = advice_only_doc("advice-strict-vs-errors-only");
    let path = doc.to_str().unwrap();
    // --strict: the same advice now fails the run (the opt-in strictest gate).
    let (ok_strict, _o, _e) = run(&["check", path, "--strict"]);
    assert!(!ok_strict, "--strict fails on advice");
    // --errors-only: advice is below the floor, so it is neither shown nor gated.
    let (ok_eo, stdout, _e2) = run(&["check", path, "--errors-only", "--format", "json"]);
    assert!(ok_eo, "--errors-only passes an advice-only document");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert!(
        v["diagnostics"].as_array().expect("array").is_empty(),
        "--errors-only reports nothing here: {stdout}"
    );
}

#[test]
fn the_human_summary_does_not_call_advice_a_problem() {
    let doc = advice_only_doc("advice-human-summary");
    let (ok, _o, stderr) = run(&["check", doc.to_str().unwrap()]);
    assert!(ok, "advice-only passes");
    assert!(
        stderr.contains("suggestion") && stderr.contains("nothing here fails the run"),
        "the summary explains the exit code: {stderr}"
    );
    assert!(
        !stderr.contains("problem"),
        "advice is not reported as a problem beside an exit 0: {stderr}"
    );
}

#[test]
fn build_strict_ships_a_document_whose_only_findings_are_advice() {
    // The failure this whole floor exists to prevent: `publish` is strict by default, so an
    // ERROR-severity style suggestion blocked releasing a document over a word choice.
    let doc = advice_only_doc("advice-build-strict");
    let out = std::env::temp_dir().join(format!("tali-advice-build-{}.html", std::process::id()));
    let (ok, _o, stderr) = run(&[
        "build",
        doc.to_str().unwrap(),
        out.to_str().unwrap(),
        "--strict",
    ]);
    assert!(ok, "--strict must not fail on advice: {stderr}");
    assert!(out.exists(), "the page was written: {stderr}");
    let _ = std::fs::remove_file(&out);
}

#[test]
fn a_suggestion_only_document_passes_until_strict() {
    // The third state of 24a's severity floor: two headings reading the same is a
    // SUGGESTION, so it is advice everywhere except under `--strict`. This asserts the whole
    // exit-code ladder on one document, where the tests above take one rung each.
    let doc = tmp_doc(
        "suggestion-only",
        "---\ntitle: T\n---\n\n## Same\n\na\n\n## Same\n\nb\n",
    );
    let path = doc.to_str().unwrap();

    let (ok_default, stdout, _e) = run(&["check", path]);
    assert!(
        ok_default,
        "advice alone does not fail the default gate (exit 0): {stdout}"
    );
    let (ok_errors_only, _o, _e) = run(&["check", path, "--errors-only"]);
    assert!(ok_errors_only, "advice alone passes --errors-only too");
    let (ok_strict, _o, stderr) = run(&["check", path, "--strict"]);
    assert!(
        !ok_strict,
        "--strict is the one floor that gates on advice: {stderr}"
    );

    // And it really is the shape lint that is being counted, at suggestion severity.
    let (_ok, json, _e) = run(&["check", path, "--format", "json"]);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let ds = parsed["diagnostics"].as_array().expect("diagnostics array");
    assert_eq!(ds.len(), 1, "exactly one diagnostic: {json}");
    assert_eq!(ds[0]["code"], "TAL-SHAPE-DUP");
    assert_eq!(ds[0]["severity"], "suggestion");
}

/// Item 81 (2026-07-28). `check` is the kernel-free, network-free pass an agent runs first
/// on a project it has not read, and `_site.yml`'s `python:` field is a string that
/// project's author wrote. Before this, `check --format json` (the shape the MCP `check`
/// tool returns, described to the agent only as "Validate") ran
/// `Command::new(<that string>).arg("--version")`.
///
/// The interpreter here is a shell script that *records having been run*, so the assertion
/// is about a real spawn rather than about output wording. The `--require-kernel` half is
/// what keeps the test non-vacuous: if the probe were removed outright instead of gated,
/// the marker would be absent in both halves and only this row would notice.
#[cfg(unix)]
#[test]
fn check_does_not_spawn_a_project_supplied_interpreter_without_an_opt_in() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!(
        "tali-check-field-interp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("it-ran");
    let fake = dir.join("fake-python.sh");
    std::fs::write(
        &fake,
        format!(
            "#!/bin/sh\ntouch '{}'\necho 'Python 9.9.9'\n",
            marker.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(
        dir.join("_site.yml"),
        format!("title: Field\npython: {}\n", fake.display()),
    )
    .unwrap();
    std::fs::write(
        dir.join("index.tmd"),
        "---\ntitle: F\n---\n\n```{python}\n1 + 1\n```\n",
    )
    .unwrap();
    let path = dir.to_str().unwrap();

    // 1. The JSON path resolves and reports the interpreter, and does not run it.
    let (_ok, stdout, _e) = run(&["check", path, "--format", "json"]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let env = parsed["environment"]
        .as_array()
        .expect("environment array present");
    assert_eq!(env.len(), 1, "one entry for the python cell: {stdout}");
    assert!(
        !marker.exists(),
        "check must not spawn a `_site.yml`-supplied interpreter: {stdout}"
    );
    assert!(
        env[0]["path"]
            .as_str()
            .unwrap_or("")
            .contains("fake-python"),
        "the entry still names which interpreter would be used: {stdout}"
    );
    assert!(
        env[0]["runs"].is_null() && env[0]["not_probed"].is_string(),
        "runnability is reported as unknown, with a reason: {stdout}"
    );

    // 2. `--require-kernel` is the opt-in, and it really does probe — without this the
    //    assertion above would also pass if the probe had been deleted entirely.
    let (_ok2, _o2, _e2) = run(&["check", path, "--require-kernel"]);
    assert!(
        marker.exists(),
        "--require-kernel must still probe the project-supplied interpreter"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
