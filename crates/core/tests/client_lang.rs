//! The client-side cell-language registry (backlog item 153), pinned at the seams a
//! second language actually crosses.
//!
//! **Why the registry is still tested with one language in it.** `{glsl}` was withdrawn on
//! 2026-08-08, so `CLIENT_LANGS` has a single entry — but the registry's whole claim is
//! that adding a language is a *registration* rather than surgery, and the interesting
//! failures are all of the form "one of the six places that used to say `lang == "js"` did
//! not move". Each seam below is one of those places. The rows that could only be driven
//! from a shader went with `{glsl}`; what is left is the language-blind half, which is what
//! a future second language would land on.
//!
//! | seam                          | what it pins                                         |
//! |-------------------------------|------------------------------------------------------|
//! | the mime handshake            | Rust's `<script type>` is what `tali-js.js` looks up  |
//! | the disjointness rule         | a client language never reaches `exec.rs`             |
//! | the `{js}` asset gate         | a prose page ships neither runtime nor libraries      |
//! | inline vs External globals    | a global present in preview and absent in the build   |
//! | `reactive.rs::runtime_defines`| the dangling-input warning is not suppressed wholesale |

use taliesin_core::OutputMode;
use taliesin_core::render::{
    client_lang, code_scripts_for, executes_to_kernel, has_client_cells, has_client_cells_of,
    has_js_cells,
};

fn render(src: &str) -> taliesin_core::RenderedDoc {
    taliesin_core::render_document_with_includes(src, std::path::Path::new("."))
}

const CHART: &str = "```{js}\nreturn document.createElement(\"p\");\n```\n";

// ---------------------------------------------------------------------------
// the registry itself
// ---------------------------------------------------------------------------

/// The two halves of the registry must agree on the MIME, because it is the only thing
/// they share: Rust writes `<script type=…>` and `tali-js.js` looks the language up by
/// exactly that string. A typo on either side is a cell that silently never mounts.
#[test]
fn every_registered_mime_is_looked_up_by_the_client_runtime() {
    let runtime = include_str!("../assets/js/tali-js.js");
    // One statement per registered language; the registry has one entry today.
    let spec = client_lang("js").expect("registered");
    assert!(
        runtime.contains(spec.mime),
        "`{}` is registered server-side as `{}` but the client runtime never looks that \
         mime up",
        spec.lang,
        spec.mime
    );
}

/// A client-side language's kernel is the browser, so it must never be in the set the
/// executor will try to run. The two sets being disjoint is what keeps the registry out of
/// `exec.rs` without `exec.rs` having to know it exists.
#[test]
fn client_langs_never_reach_a_kernel() {
    assert!(client_lang("js").is_some(), "`js` should be registered");
    assert!(
        !executes_to_kernel("js"),
        "`js` is a client-side language and must not be in the executable set"
    );
    assert!(
        client_lang("python").is_none(),
        "`python` runs against a kernel and must not be in the client registry"
    );
}

// The `--no-exec` half of the registry lives in `crates/server/tests/no_exec_js_cells.rs`,
// beside the `{js}` case it generalizes. `no_exec_in_force` reads a process-wide env var,
// so it has to be driven through a subprocess: setting it in-process would leak into every
// other test sharing this binary and make their results depend on scheduling order.

// ---------------------------------------------------------------------------
// asset gates
// ---------------------------------------------------------------------------

/// Two gates rather than one, kept apart even at one language: `has_client_cells` gates the
/// shared runtime and `has_js_cells` gates the ~490 KB of d3 + Plot that only `{js}` draws
/// with. Collapsing them would make a future language without drawing libraries pay for
/// them, which is the shape the registry exists to prevent.
#[test]
fn the_runtime_and_the_drawing_libraries_are_gated_separately() {
    let chart = render(CHART).body_html();
    assert!(has_client_cells(&chart) && has_js_cells(&chart));
    assert!(has_client_cells_of(&chart, "js"));

    let prose = render("Just prose.\n").body_html();
    assert!(!has_client_cells(&prose) && !has_js_cells(&prose));
}

/// A `{js}` page in a static Build ships the shared runtime; a prose page ships none of it.
/// `code_scripts_for` opens every gate in Preview, so only the Build arm can catch a gate
/// that has stopped closing.
#[test]
fn a_build_of_a_cell_page_ships_the_runtime_and_a_prose_page_does_not() {
    let scripts = code_scripts_for(&render(CHART).body_html(), OutputMode::Build);
    assert!(
        scripts.contains("tali-js cell error:"),
        "the shared runtime must ship for a page with client cells"
    );

    let prose = code_scripts_for(&render("Just prose.\n").body_html(), OutputMode::Build);
    assert!(!prose.contains("tali-js cell error:"));
}

/// `js_cell_head` (inline) and `js_cell_libs_js` (External/site) are the same set of
/// drawing globals reached two ways. A global added to one and not the other is a cell
/// that works in preview and is `undefined` in the built site — which nothing else here
/// would notice, since both paths render identical BODY html.
#[test]
fn the_inline_and_external_js_globals_agree() {
    let external = taliesin_core::js_cell_libs_js();
    // One marker per global the cell scope hands a `{js}` cell, each a literal that exists
    // only in that library's own source (never in a comment about it).
    for (global, marker) in [("d3", "d3.min.js"), ("Plot", "@observablehq/plot")] {
        assert!(
            external.contains(marker),
            "`{global}` is a `{{js}}` drawing global on the inline path but is missing from \
             the External jslibs bundle — cells would work in preview and be undefined in \
             the built site"
        );
    }
}

// ---------------------------------------------------------------------------
// the diagnostic
// ---------------------------------------------------------------------------

/// The reactive graph check is a `check`-time diagnostic over the block model, not a
/// render warning, so it is driven directly here.
fn dangling(src: &str) -> Vec<String> {
    let doc = render(src);
    taliesin_core::diagnostics::validate_js_reactive_graph(&doc.blocks)
        .into_iter()
        .map(|w| w.message)
        .filter(|m| m.contains("unknown reactive input"))
        .collect()
}

/// The baseline row, without which every assertion below could pass on a diagnostic that
/// had simply stopped working. A table-shaped probe whose every cell is negative is a
/// broken probe until a known-positive row proves otherwise.
#[test]
fn a_js_only_page_still_reports_its_dangling_input() {
    assert_eq!(
        dangling("```{js}\n//| input: nope\nreturn 1;\n```\n").len(),
        1,
        "the known-positive row: the check itself must work"
    );
}

/// A kernel cell suppresses the check only when it CALLS `define(`, narrowed from "any
/// kernel cell" on 2026-08-03. The conservatism is right where the bridge is really used
/// and wrong everywhere else: spelled `lang != "js"` it went silent on any page carrying a
/// second client language, and spelled "any kernel cell" it went silent on every real blog
/// post in the corpus, which is precisely where a typo'd input hides best.
#[test]
fn only_a_python_cell_that_calls_define_suppresses_the_dangling_input_check() {
    let suppressed =
        dangling("```{python}\ndefine(x=1)\n```\n\n```{js}\n//| input: nope\nreturn 1;\n```\n");
    assert!(
        suppressed.is_empty(),
        "a cell using the bridge must keep the check suppressed: {suppressed:?}"
    );

    let reported =
        dangling("```{python}\nx = 1\n```\n\n```{js}\n//| input: nope\nreturn 1;\n```\n");
    assert!(
        reported.iter().any(|m| m.contains("`nope`")),
        "a cell that defines nothing must not suppress it: {reported:?}"
    );
}
