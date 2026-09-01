//! `build`/`publish --format json` emit the build's static-lint diagnostics as
//! `{diagnostics:[{code,severity,file,line,message,suggestion?}]}` to stdout (for an
//! agent/CI), reusing the lint's exact per-diagnostic shape so the two channels can't drift.
//! The build set is a *superset* of the lint's (it adds cell-error outputs), so every
//! diagnostic `build --check-only` reports must also appear in a writing build's JSON.

use std::collections::HashSet;
use std::fs;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-sbe-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn taliesin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_taliesin"))
}

fn stdout_json(cmd: &mut Command) -> serde_json::Value {
    let out = cmd.output().expect("run taliesin");
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not valid json ({e}):\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn messages(v: &serde_json::Value) -> HashSet<String> {
    v["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .map(|d| d["message"].as_str().unwrap_or("").to_string())
        .collect()
}

#[test]
fn single_doc_build_json_emits_structured_diagnostics() {
    let dir = tmp_dir("single");
    // A dup heading id + a missing image: both static (kernel-free), in the standalone set.
    let doc = dir.join("doc.tmd");
    fs::write(
        &doc,
        "---\ntitle: T\n---\n\n## A {#dup}\n\n## B {#dup}\n\n![a missing chart](missing.png)\n",
    )
    .unwrap();

    let build = stdout_json(
        taliesin()
            .arg("build")
            .arg(&doc)
            .arg(dir.join("out.html"))
            .arg("--strict")
            .args(["--format", "json"]),
    );
    let diags = build["diagnostics"].as_array().expect("array");
    assert!(!diags.is_empty(), "build reports diagnostics: {build}");
    for d in diags {
        // Severity + file + message: the three fields an agent triages on. The `TAL-*` code
        // and its `docs_url` were here until 2026-08-08; both went with the catalogue, so a
        // consumer keying on `code` would break loudly rather than reading a stale token.
        assert!(
            matches!(
                d["severity"].as_str(),
                Some("error" | "warning" | "suggestion")
            ),
            "each carries a severity: {d}"
        );
        assert!(d["code"].is_null(), "no code survives the catalogue: {d}");
        assert!(d["docs_url"].is_null(), "nor a docs_url: {d}");
        assert!(d["file"].as_str().is_some(), "each carries a file: {d}");
    }

    // The lint's diagnostics for the same doc are a SUBSET of the build's (build is a
    // superset: it also runs the cells).
    let check =
        stdout_json(
            taliesin()
                .arg("build")
                .arg(&doc)
                .args(["--check-only", "--format", "json"]),
        );
    let (build_msgs, check_msgs) = (messages(&build), messages(&check));
    assert!(
        check_msgs.is_subset(&build_msgs),
        "check diagnostics must be a subset of build's.\ncheck-only: {:?}",
        check_msgs.difference(&build_msgs).collect::<Vec<_>>()
    );
}

/// The SITE build's structured diagnostics, located to their page. Distinct from the
/// single-document case above: a site build fans out over pages and folds the results back,
/// which is where a per-page `file` can be lost. It drove `publish --dry-run --format json`
/// until wave 4 cut that verb on 2026-08-08 — `build --format json` was always the same
/// code path, and is now the only caller of it.
#[test]
fn site_build_json_emits_structured_diagnostics_located_to_their_page() {
    let dir = tmp_dir("site");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n## A {#dup}\n\n## B {#dup}\n\n![a missing chart](nope.png)\n",
    )
    .unwrap();

    let build = stdout_json(
        taliesin()
            .arg("build")
            .arg(&dir)
            .arg("--no-exec")
            .args(["--format", "json"]),
    );
    let diags = build["diagnostics"].as_array().expect("array");
    assert!(
        diags.iter().any(|d| d["message"]
            .as_str()
            .unwrap_or("")
            .contains("duplicate heading id")),
        "build --format json reports the site's diagnostics: {build}"
    );
    assert!(
        diags
            .iter()
            .all(|d| d["file"].as_str() == Some("index.tmd")),
        "diagnostics are located to their page: {build}"
    );
}

#[test]
fn build_rejects_a_bad_format_value() {
    let dir = tmp_dir("badfmt");
    let doc = dir.join("doc.tmd");
    fs::write(&doc, "---\ntitle: T\n---\n\nHi.\n").unwrap();
    let out = taliesin()
        .arg("build")
        .arg(&doc)
        .args(["--format", "yaml"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "an unknown --format value must fail");
}

/// A single-doc build locates its diagnostics to a path a tool can OPEN, in both channels.
///
/// It used to prefix them with `file_stem()`: `doc:5: duplicate heading id`. That is not a
/// path — no editor's "open at line", no `vim +5`, no CI annotation resolves it, and the
/// information needed to build one (the argument the user typed) was right there at the call
/// site. The site build never had this defect, so nothing compared the two.
///
/// Asserted from a DIFFERENT working directory than the document's, so a bare filename
/// cannot pass by accident: the label has to carry the directory the user actually named.
#[test]
fn single_doc_diagnostics_are_located_to_an_openable_path() {
    let dir = tmp_dir("label");
    let sub = dir.join("chapters");
    fs::create_dir_all(&sub).unwrap();
    let doc = sub.join("intro.tmd");
    fs::write(&doc, "---\ntitle: T\n---\n\n## A {#dup}\n\n## B {#dup}\n").unwrap();
    // The path exactly as a user would type it from `dir`, directory component included.
    let typed = "chapters/intro.tmd";

    let out = taliesin()
        .current_dir(&dir)
        .arg("build")
        .arg(typed)
        .args(["--format", "json"])
        .output()
        .expect("run taliesin");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let files: HashSet<String> = json["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .map(|d| d["file"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        !files.is_empty(),
        "the dup heading must be reported: {json}"
    );
    for f in &files {
        assert_eq!(
            f, typed,
            "every diagnostic names the path the user gave, not its stem: {json}"
        );
        assert!(
            dir.join(f).exists(),
            "the located file resolves from the invocation directory: {f}"
        );
    }

    // The human channel is the same label, so the two cannot drift apart again.
    let human = taliesin()
        .current_dir(&dir)
        .arg("build")
        .arg(typed)
        .output()
        .expect("run taliesin");
    let stderr = String::from_utf8_lossy(&human.stderr).to_string();
    assert!(
        stderr.contains(&format!("{typed}:")),
        "the console prefixes the openable path too:\n{stderr}"
    );
}

/// The `--strict` carve-out for the offline-guarantee nudge, pinned in both directions:
/// an external reference the author kept is deliberately NOT a `--strict` failure (it can
/// be intentional, and the tool does not download URLs at build time), but the SAME run's
/// `--format json` must carry the diagnostic. JSON is the machine surface, and a channel
/// that hides a warning the console printed is the thing to fix, not the exit code.
#[test]
fn external_reference_warning_is_exempt_from_strict_but_present_in_json() {
    let dir = tmp_dir("extref");
    let doc = dir.join("doc.tmd");
    fs::write(
        &doc,
        "---\ntitle: T\n---\n\nHello.\n\n<script src=\"https://cdn.test/x.js\"></script>\n",
    )
    .unwrap();

    let out = taliesin()
        .arg("build")
        .arg(&doc)
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--strict")
        .args(["--format", "json"])
        .output()
        .expect("run taliesin");
    assert!(
        out.status.success(),
        "an external reference alone must not fail --strict:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json on stdout");
    let diags = json["diagnostics"].as_array().expect("diagnostics array");
    let ext: Vec<_> = diags
        .iter()
        .filter(|d| {
            d["message"]
                .as_str()
                .unwrap_or("")
                .contains("external reference not bundled")
        })
        .collect();
    assert_eq!(ext.len(), 1, "the warning rides the json channel: {json}");
    assert_eq!(ext[0]["severity"].as_str(), Some("warning"), "{json}");
    assert!(
        ext[0]["message"]
            .as_str()
            .unwrap()
            .contains("https://cdn.test/x.js"),
        "{json}"
    );
    assert!(
        ext[0]["line"].as_u64().is_some(),
        "located to the block that keeps the reference: {json}"
    );
}

#[test]
fn a_site_pages_external_reference_reaches_json_and_stays_exempt_under_strict() {
    // T9's carve-out, on the arm tools/publish.sh actually ships: an external ref on a
    // site page never fails --strict, and the same run's json still carries it.
    let dir = tmp_dir("site-ext");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n<script src=\"https://cdn.example.com/x.js\"></script>\n\nhello\n",
    )
    .unwrap();
    let out = taliesin()
        .arg("build")
        .arg(&dir)
        .arg("--no-exec")
        .arg("--strict")
        .args(["--format", "json"])
        .output()
        .expect("run taliesin");
    assert!(
        out.status.success(),
        "external refs never fail --strict (the documented carve-out): {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json stdout");
    assert!(
        messages(&v)
            .iter()
            .any(|m| m.contains("external reference not bundled")),
        "the site arm's json carries the carve-out diagnostic: {v}"
    );
}

#[test]
fn an_exec_only_defect_reaches_the_writing_builds_json() {
    // Live-kernel: skipped without an interpreter; the gate script arms
    // TALIESIN_REQUIRE_KERNEL so a silent skip cannot pass for coverage there. The tmp
    // dir is wiped each run, so the freeze cache starts empty and the cell executes.
    if std::env::var_os("TALIESIN_PYTHON").is_none() {
        assert!(
            std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
            "TALIESIN_REQUIRE_KERNEL is set but TALIESIN_PYTHON is unset: this test \
             needs an interpreter with ipykernel"
        );
        return;
    }
    // A labelled figure cell that runs but emits nothing: the one execution-only
    // diagnostic class. The warn line always reached the terminal; the machine surface
    // must see what the console sees.
    let dir = tmp_dir("exec-json");
    fs::write(
        dir.join("doc.tmd"),
        "---\ntitle: T\n---\n\nSee @fig-empty.\n\n```{python}\n#| label: fig-empty\n#| fig-cap: \"empty\"\nx = 1\n```\n",
    )
    .unwrap();
    let v = stdout_json(
        taliesin()
            .arg("build")
            .arg(dir.join("doc.tmd"))
            .args(["--format", "json"]),
    );
    assert!(
        messages(&v)
            .iter()
            .any(|m| m.contains("produced no output")),
        "the empty-labelled-float warning reaches --format json: {v}"
    );
}
