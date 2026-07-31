//! Delivery of the vendored Pyodide payload (backlog item 158).
//!
//! **Read this before adding an assertion here.** Every Taliesin page inlines the whole CSS
//! and JS payload, so a whole-page `contains("pyodide")` is a claim about the BUNDLE as much
//! as about the document — it passes on a page that renders no Python at all, and it fails on
//! a page whose bundled CSS merely mentions the word. Every needle below is therefore a full
//! emitted tag, never a bare word. That trap has now fired in both directions on this repo.

use taliesin_core::OutputMode;
use taliesin_core::render::{code_scripts_for, degrade_pyodide_cells, has_client_cells_of};

fn render(src: &str) -> taliesin_core::RenderedDoc {
    taliesin_core::render_document_with_includes(src, std::path::Path::new("."))
}

const PY: &str = "```{pyodide}\nimport numpy as np\nnp.arange(3).tolist()\n```\n";
const JS: &str = "```{js}\nreturn document.createElement(\"p\");\n```\n";

/// Two gates, not one: a Python page must not drag in d3 + Plot, and a chart page must not
/// ship the Pyodide enhancer. This is the assertion that would have shipped 490 KB of
/// plotting library to a page that only computes.
#[test]
fn the_pyodide_and_js_asset_gates_are_independent() {
    let py = render(PY).body_html();
    let js = render(JS).body_html();

    assert!(has_client_cells_of(&py, "pyodide"));
    assert!(
        !has_client_cells_of(&py, "js"),
        "no d3/Plot for a compute-only page"
    );
    assert!(has_client_cells_of(&js, "js"));
    assert!(
        !has_client_cells_of(&js, "pyodide"),
        "no Pyodide enhancer for a chart page"
    );
}

/// A `{pyodide}`-only page in a static Build ships the shared runtime AND the language
/// enhancer. `code_scripts_for` opens every gate in Preview, so only the Build arm can catch
/// a dead cell.
#[test]
fn a_build_of_a_python_page_ships_the_runtime_and_the_enhancer() {
    let scripts = code_scripts_for(&render(PY).body_html(), OutputMode::Build);
    assert!(
        scripts.contains("application/tali-pyodide"),
        "pyodide.js (which registers that mime) must ship"
    );
    assert!(
        scripts.contains("tali-js cell error:"),
        "the shared runtime must ship for a python-only page"
    );

    let prose = code_scripts_for(&render("Just prose.\n").body_html(), OutputMode::Build);
    assert!(
        !prose.contains("application/tali-pyodide"),
        "a prose page must ship neither"
    );
}

/// The single-file build is the one output path that cannot carry a 12.9 MB directory. The
/// cell degrades to VISIBLE SOURCE rather than to an empty div, which is what stripping the
/// script alone would leave: the author's code is in the `<script>`, so removing it without
/// re-emitting it silently deletes the content.
#[test]
fn a_single_file_build_degrades_a_pyodide_cell_to_visible_source() {
    let body = render(PY).body_html();
    let out = degrade_pyodide_cells(&body);
    assert!(
        !out.contains("<script type=\"application/tali-pyodide\""),
        "the runnable wrapper must be gone: {out}"
    );
    // `arange`, NOT `np.arange(3)`. Server-side highlighting splits the source into
    // `<span>`-wrapped tokens, so the multi-token literal never appears contiguously in
    // correct output — asserting it would fail on a correctly-degraded page rather than on
    // the regression this row exists to catch. Measured: `contains("np.arange(3)")` is
    // false and `contains("arange")` is true for this exact source.
    assert!(
        out.contains("arange"),
        "the author's source must remain VISIBLE, not just deleted: {out}"
    );
    assert!(
        out.contains("<pre><code class=\"language-python\">"),
        "and it must be marked up as a python listing, the same shape emit.rs uses: {out}"
    );
}

/// The degradation must leave every OTHER client language alone — it is keyed on one mime,
/// and a `{js}` cell in the same document still runs in a single-file build.
#[test]
fn the_degradation_leaves_js_cells_running() {
    let body = render(&format!("{PY}\n{JS}")).body_html();
    let out = degrade_pyodide_cells(&body);
    assert!(
        out.contains("<script type=\"application/tali-js\""),
        "a `{{js}}` cell must survive a single-file build untouched: {out}"
    );
}
