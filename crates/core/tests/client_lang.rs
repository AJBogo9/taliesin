//! The client-side cell-language registry (backlog item 153), pinned at the seams a
//! second language actually crosses.
//!
//! **Why these and not "does `{glsl}` render".** The registry's whole claim is that adding
//! a language is a *registration* rather than surgery, which means the interesting failures
//! are all of the form "one of the six places that used to say `lang == "js"` did not move".
//! Each test below is one of those places, and each was confirmed to fail by putting the
//! `js`-only spelling back:
//!
//! | seam                          | what breaks if it stays `js`-only                    |
//! |-------------------------------|------------------------------------------------------|
//! | the plain-cell arm            | a `{glsl}` cell renders as highlighted source         |
//! | the figure gate               | `label: fig-` on a shader burns no number, warns      |
//! | `--no-exec`                   | a shader still emits its script under `--no-exec`     |
//! | the runtime asset gate        | a `{glsl}`-only page ships no runtime at all          |
//! | the `{js}` asset gate         | a shader page ships 490 KB of d3 + Plot for nothing   |
//! | `reactive.rs::runtime_defines`| every dangling-input warning on the page disappears   |
//! | `strip_client_scripts`        | `--bare`'s zero-`<script>` contract breaks silently   |

use taliesin_core::OutputMode;
use taliesin_core::render::{
    client_lang, code_scripts_for, executes_to_kernel, has_client_cells, has_client_cells_of,
    has_js_cells,
};

fn render(src: &str) -> taliesin_core::RenderedDoc {
    taliesin_core::render_document_with_includes(src, std::path::Path::new("."))
}

const SHADER: &str = "```{glsl}\nvoid main() { gl_FragColor = vec4(1.0); }\n```\n";

// ---------------------------------------------------------------------------
// the registry itself
// ---------------------------------------------------------------------------

/// The two halves of the registry must agree on the MIME, because it is the only thing
/// they share: Rust writes `<script type=…>` and `tali-js.js` looks the language up by
/// exactly that string. A typo on either side is a cell that silently never mounts.
#[test]
fn every_registered_mime_is_looked_up_by_the_client_runtime() {
    let runtime = include_str!("../assets/js/tali-js.js");
    let glsl = include_str!("../assets/js/glsl.js");
    for lang in ["js", "glsl"] {
        let spec = client_lang(lang).expect("registered");
        assert!(
            runtime.contains(spec.mime) || glsl.contains(spec.mime),
            "`{}` is registered server-side as `{}` but no client file registers that mime",
            spec.lang,
            spec.mime
        );
    }
}

/// A client-side language's kernel is the browser, so it must never be in the set the
/// executor will try to run. The two sets being disjoint is what keeps `{glsl}` out of
/// `exec.rs` without `exec.rs` having to know it exists.
#[test]
fn client_langs_never_reach_a_kernel() {
    for lang in ["js", "glsl"] {
        assert!(client_lang(lang).is_some(), "{lang} should be registered");
        assert!(
            !executes_to_kernel(lang),
            "`{lang}` is a client-side language and must not be in the executable set"
        );
    }
    for lang in ["python", "r"] {
        assert!(
            client_lang(lang).is_none(),
            "`{lang}` runs against a kernel and must not be in the client registry"
        );
    }
}

// ---------------------------------------------------------------------------
// emission
// ---------------------------------------------------------------------------

#[test]
fn a_glsl_cell_emits_the_shared_wrapper_contract() {
    let h = render(SHADER).body_html();
    assert!(
        h.contains("<script type=\"application/tali-glsl\""),
        "the shader's own mime: {h}"
    );
    assert!(
        h.contains("class=\"cell tali-glsl-cell\"") && h.contains("class=\"tali-js-out\""),
        "the SAME wrapper shape a `{{js}}` cell uses: {h}"
    );
    assert!(
        h.contains("gl_FragColor"),
        "the author source rides verbatim: {h}"
    );
}

/// `//` is GLSL's comment marker too, so the shared `//|` directive parser already reads a
/// shader's reactive options — the graph does not care which language publishes a name.
#[test]
fn a_glsl_cell_takes_the_same_reactive_options() {
    let h = render("```{glsl}\n//| name: shade\n//| input: k\nvoid main() {}\n```\n").body_html();
    assert!(h.contains("data-name=\"shade\""), "published name: {h}");
    assert!(h.contains("data-inputs=\"k\""), "consumed inputs: {h}");
}

#[test]
fn a_labelled_glsl_cell_becomes_a_numbered_figure() {
    let doc = render(
        "```{glsl}\n//| label: fig-shader\n//| fig-cap: A shader.\nvoid main() {}\n```\n\nSee @fig-shader.\n",
    );
    let h = doc.body_html();
    assert!(h.contains("<figure"), "wrapped as a float: {h}");
    assert!(h.contains("id=\"fig-shader\""), "anchored: {h}");
    assert!(
        h.contains("Figure&nbsp;1") || h.contains("Figure&nbsp;1:"),
        "numbered: {h}"
    );
    assert!(
        doc.warnings
            .iter()
            .all(|w| !w.message.contains("fig-shader")),
        "a materializing float must not warn: {:?}",
        doc.warnings
    );
}

// The `--no-exec` half of the registry lives in `crates/server/tests/no_exec_js_cells.rs`,
// beside the `{js}` case it generalizes. `no_exec_in_force` reads a process-wide env var,
// so it has to be driven through a subprocess: setting it in-process would leak into every
// other test sharing this binary and make their results depend on scheduling order.

// ---------------------------------------------------------------------------
// asset gates
// ---------------------------------------------------------------------------

/// The point of two gates rather than one: a shader page needs the runtime and must NOT
/// pay for the plotting libraries, and a chart page must not pay for the WebGL enhancer.
#[test]
fn the_two_asset_gates_are_independent() {
    let shader = render(SHADER).body_html();
    let chart = render("```{js}\nreturn document.createElement(\"p\");\n```\n").body_html();

    assert!(has_client_cells(&shader), "a shader page needs the runtime");
    assert!(
        !has_js_cells(&shader),
        "a shader page must NOT drag in d3 + Plot"
    );
    assert!(has_client_cells_of(&shader, "glsl"));

    assert!(has_client_cells(&chart) && has_js_cells(&chart));
    assert!(
        !has_client_cells_of(&chart, "glsl"),
        "a chart page must NOT ship the WebGL enhancer"
    );
}

/// A `{glsl}`-only page in a static Build still ships the runtime AND the shader enhancer.
/// This is the gate that would have shipped a dead canvas: `code_scripts_for` opens every
/// gate in Preview, so only the Build arm can catch it.
#[test]
fn a_build_of_a_shader_page_ships_the_runtime_and_the_enhancer() {
    let body = render(SHADER).body_html();
    let scripts = code_scripts_for(&body, OutputMode::Build);
    assert!(
        scripts.contains("tali-js cell error:"),
        "the shared runtime must ship for a shader-only page"
    );
    assert!(
        scripts.contains("application/tali-glsl"),
        "glsl.js (which registers that mime) must ship"
    );

    let prose = code_scripts_for(&render("Just prose.\n").body_html(), OutputMode::Build);
    assert!(
        !prose.contains("application/tali-glsl"),
        "a prose page must ship neither"
    );
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
    for (global, marker) in [
        ("d3", "d3.min.js"),
        ("Plot", "@observablehq/plot"),
        ("num", "window.taliNum"),
    ] {
        assert!(
            external.contains(marker),
            "`{global}` is a `{{js}}` drawing global on the inline path but is missing from \
             the External jslibs bundle — cells would work in preview and be undefined in \
             the built site"
        );
    }
}

// ---------------------------------------------------------------------------
// `--bare`
// ---------------------------------------------------------------------------

/// `--bare`'s contract is **zero** `<script>`, and a client-side cell's source rides in
/// one. The strip was written against the `application/tali-js` literal, so registering a
/// second language would have broken that contract silently — bare output is exactly the
/// mode nobody looks at, because it exists to be pasted into someone else's page.
/// Driven off the registry now, and this is the test that says so.
#[test]
fn bare_output_strips_every_client_language_not_just_js() {
    let page = |src: &str| {
        taliesin_core::render_doc_to_page(
            &taliesin_core::render_document(src),
            "t",
            OutputMode::Bare,
        )
    };

    // The known-positive rows: both languages ARE live in a normal render.
    let live = render(SHADER).body_html();
    assert!(
        live.contains("<script"),
        "baseline: a shader emits a script"
    );

    for (label, src) in [
        ("glsl", SHADER),
        (
            "js",
            "```{js}\nreturn document.createElement(\"p\");\n```\n",
        ),
    ] {
        let out = page(src);
        assert!(
            !out.contains("<script"),
            "--bare must emit zero <script>, but the {label} cell left one: {out}"
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

/// `reactive.rs` suppressed the dangling-input check whenever ANY cell was `lang != "js"`,
/// reading that as "a kernel cell could publish names at runtime". A `{glsl}` cell is not
/// `js` and publishes nothing at runtime, so under the old spelling its mere presence
/// silently switched the whole check off for the page.
#[test]
fn a_glsl_cell_does_not_suppress_the_dangling_input_check() {
    let msgs =
        dangling("```{glsl}\nvoid main() {}\n```\n\n```{js}\n//| input: nope\nreturn 1;\n```\n");
    assert!(
        msgs.iter().any(|m| m.contains("`nope`")),
        "a shader on the page must not switch the check off: {msgs:?}"
    );
}

/// The other half: a shader is a node in the same graph, so its own `//| input:` is
/// checked like any cell's.
#[test]
fn a_glsl_cells_own_dangling_input_is_reported() {
    let msgs = dangling("```{glsl}\n//| input: missing\nvoid main() {}\n```\n");
    assert!(
        msgs.iter().any(|m| m.contains("`missing`")),
        "expected the shader's dangling input to be reported: {msgs:?}"
    );
}

/// A real kernel cell still suppresses it, because a Python `ojs_define` genuinely can
/// publish a name this static pass cannot enumerate. The conservatism is the point; the
/// bug was applying it to a language that has no runtime at all.
#[test]
fn a_python_cell_still_suppresses_the_dangling_input_check() {
    let msgs = dangling("```{python}\nx = 1\n```\n\n```{js}\n//| input: nope\nreturn 1;\n```\n");
    assert!(
        msgs.is_empty(),
        "a kernel cell must keep the check suppressed: {msgs:?}"
    );
}

/// A `{glsl}` cell publishing a `//| name:` satisfies a `{js}` cell that consumes it —
/// the registry's real claim: ONE graph, not one graph per language.
#[test]
fn a_name_published_by_a_shader_satisfies_a_js_consumer() {
    let msgs = dangling(
        "```{glsl}\n//| name: shade\nvoid main() {}\n```\n\n```{js}\n//| input: shade\nreturn 1;\n```\n",
    );
    assert!(
        msgs.is_empty(),
        "a cross-language edge should resolve: {msgs:?}"
    );
}
