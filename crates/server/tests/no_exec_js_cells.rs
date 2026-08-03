//! `--no-exec` / `TALIESIN_NO_EXEC` must stop a `{js}` cell, which is a code cell whose
//! runtime is the browser (items 79 + 118, 2026-07-28).
//!
//! Why this file exists rather than another assertion in `read_run.rs`: before this,
//! `TALIESIN_NO_EXEC` appeared in exactly one test, covering `read --run` — a command that
//! never emitted a page. The promise the guide made was about **preview**, and
//! `crates/core` contained zero references to the variable, so every browser-side channel
//! ran with the flag on.
//!
//! **What this file pins and what it cannot.** `build --stdout` is the one-shot form of the
//! exact emitter `preview` serves (both land in `render_document*`), so a `{js}` cell that renders
//! as inert source here renders as inert source there. What no automated test in this repo
//! can currently reach is the live socket: there is no `reqwest`/`TcpListener` harness for
//! the bin crate (a deliberate gap, backlog Tier 3). That half was verified by hand on
//! 2026-07-28 via chrome-devtools against two real previews of the same document: with the
//! flag the tab title stayed `JS` with 0 `script[type="application/tali-js"]` and 0
//! `.tali-js-cell`; without it, the cell's `document.title = "PWNED"` ran and the title
//! changed. Re-run that by hand if this emitter moves.
//!
//! Deliberately NOT asserted, because it is deliberately NOT implemented: raw `<script>`
//! passthrough and `include-in-header`/`css:` injection still reach the page under the flag.
//! Stripping author-written HTML is a sanitizer, ruled out 2026-07-03. The CLI reference
//! says so in the same paragraph that documents the flag; item 88's family is what keeps
//! those words honest.

use std::process::Command;

fn render(path: &str, no_exec: bool) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_taliesin"));
    // `--stdout` is the page on stdout, the one-shot form the retired `render` verb was.
    // The `--no-exec` FLAG is deliberately not passed: `TALIESIN_NO_EXEC` is what this file
    // tests, and the baseline row needs a run with neither, or it passes vacuously.
    cmd.arg("build").arg(path).arg("--stdout");
    if no_exec {
        cmd.env("TALIESIN_NO_EXEC", "1");
    }
    let out = cmd.output().expect("run taliesin build --stdout");
    assert!(
        out.status.success(),
        "the build must still succeed under the flag: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn fixture(tag: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-no-exec-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("doc.tmd");
    std::fs::write(&path, body).unwrap();
    path
}

/// The marker `tali-js.js` dispatches on: a cell is live if and only if the page carries a
/// `script` of this type. Asserting the full tag rather than the class name alone matters
/// here — every page inlines the whole JS payload, so a bare `contains("tali-js")` is true
/// on a page that renders nothing (the inlined-asset needle trap).
const LIVE_CELL_SCRIPT: &str = r#"type="application/tali-js""#;

#[test]
fn no_exec_renders_a_js_cell_as_source_instead_of_running_it() {
    let doc = fixture(
        "plain",
        "---\ntitle: JS\n---\n\n```{js}\ndocument.title = \"PWNED\";\nreturn html`<b>x</b>`;\n```\n",
    );
    let path = doc.to_str().unwrap();

    // Baseline: without the flag the cell IS live. This row is what stops the test passing
    // vacuously if `{js}` emission ever moves or is removed.
    let live = render(path, false);
    assert!(
        live.contains(LIVE_CELL_SCRIPT) && live.contains("tali-js-cell"),
        "without the flag a {{js}} cell must still be live"
    );

    let inert = render(path, true);
    assert!(
        !inert.contains(LIVE_CELL_SCRIPT),
        "under --no-exec no cell may be handed to the {{js}} runtime"
    );
    assert!(
        !inert.contains("tali-js-cell"),
        "under --no-exec no live {{js}} cell wrapper may be emitted"
    );
    // The source is still shown, highlighted — a suppressed cell is a *listing*, not a
    // hole in the document, which is how a kernel-less `{python}` cell already behaves.
    assert!(
        inert.contains("tali-hl-"),
        "the cell's source must still render, highlighted"
    );
    // The block model is untouched: click-to-source and the incremental swap key off these.
    assert!(
        inert.contains("data-block-id=") && inert.contains("data-sourcepos="),
        "a suppressed cell keeps its block id + sourcepos"
    );
}

/// The same guarantee for the second client-side language (item 153). This is the seam a
/// registry makes easy to get wrong: `--no-exec` was spelled `lang == "js"`, so a language
/// added to the registry without moving that line would keep running under the flag — the
/// flag's whole promise being that *nothing* the document carries executes in the browser.
#[test]
fn no_exec_renders_a_glsl_cell_as_source_too() {
    const LIVE_SHADER: &str = r#"type="application/tali-glsl""#;
    let doc = fixture(
        "glsl",
        "---\ntitle: Shader\n---\n\n```{glsl}\nvoid main() { gl_FragColor = vec4(1.0); }\n```\n",
    );
    let path = doc.to_str().unwrap();

    // The known-positive row: without the flag the shader IS live.
    let live = render(path, false);
    assert!(
        live.contains(LIVE_SHADER) && live.contains("tali-glsl-cell"),
        "without the flag a {{glsl}} cell must be live"
    );

    let inert = render(path, true);
    assert!(
        !inert.contains(LIVE_SHADER) && !inert.contains("tali-glsl-cell"),
        "under --no-exec no shader may be handed to the browser"
    );
    assert!(
        inert.contains("gl_FragColor"),
        "the shader source must still render"
    );
}

#[test]
fn no_exec_does_not_number_a_js_figure_it_will_not_emit() {
    // A labelled `{js}` figure materializes only because the render pass emits it. With the
    // flag it does not, so it must not burn a figure number or register `@fig-demo` —
    // otherwise `@fig-demo` points at a "Figure 1" no element carries and every later
    // figure shifts. Same rule the pass already applies to `{bash}`/`{sql}`.
    let doc = fixture(
        "figure",
        "---\ntitle: F\n---\n\n```{js}\n//| label: fig-demo\n//| fig-cap: A scene\nreturn html`<b>x</b>`;\n```\n\nSee @fig-demo.\n",
    );
    let path = doc.to_str().unwrap();

    let live = render(path, false);
    assert!(
        live.contains("Figure&nbsp;1") || live.contains("Figure 1"),
        "without the flag the js figure is numbered: {live:.0}"
    );

    let inert = render(path, true);
    assert!(
        !inert.contains(LIVE_CELL_SCRIPT),
        "the labelled cell must not be live under --no-exec"
    );
    assert!(
        !inert.contains("id=\"fig-demo\""),
        "no anchor may be registered for a figure that is never emitted"
    );
}
