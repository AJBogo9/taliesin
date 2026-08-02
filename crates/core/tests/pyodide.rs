//! Delivery of the vendored Pyodide payload (backlog item 158).
//!
//! **Read this before adding an assertion here.** Every Taliesin page inlines the whole CSS
//! and JS payload, so a whole-page `contains("pyodide")` is a claim about the BUNDLE as much
//! as about the document — it passes on a page that renders no Python at all, and it fails on
//! a page whose bundled CSS merely mentions the word. Every needle below is therefore a full
//! emitted tag, never a bare word. That trap has now fired in both directions on this repo.

use taliesin_core::OutputMode;
use taliesin_core::render::{
    PREVIEW_PYODIDE_DIR, PYODIDE_DIR_NAME, code_scripts_for, degrade_pyodide_cells,
    has_client_cells_of, pyodide_index_meta,
};

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

/// The single-file build is the one output path that cannot carry a 15.7 MiB directory. The
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
        out.contains("<code class=\"language-python\">"),
        "and it must be marked up as a python listing, the same shape emit.rs uses: {out}"
    );
}

/// Finding 1 (task-4 review): the old degradation left the wrapper `<div class="cell
/// tali-pyodide-cell">` and its now-dead `<div class="tali-js-out">` output target
/// standing around the new `<pre>`, and dropped the wrapper's `data-block-id`/
/// `data-sourcepos` on the floor instead of moving them — a real regression, since every
/// emitted block must carry those two attrs (`corpus.rs`). This pins the fix: the WHOLE
/// wrapper is gone, and its block attrs ride on the surviving `<pre>`.
#[test]
fn a_degraded_pyodide_cell_carries_its_block_attrs_and_sheds_the_dead_wrapper() {
    let body = render(PY).body_html();
    // Pull the wrapper `<div>`'s own attrs (a content-hash id + sourcepos span, so they
    // can't be hardcoded) out of the PRE-degrade body, then require the exact same
    // attribute strings to survive onto the degraded `<pre>` — not just "some
    // data-block-id somewhere", but THIS block's id and span.
    let block_id_attr = extract_attr(&body, "data-block-id");
    let sourcepos_attr = extract_attr(&body, "data-sourcepos");

    let out = degrade_pyodide_cells(&body);

    assert!(
        out.contains(&block_id_attr),
        "the wrapper div's {block_id_attr} must move onto the surviving <pre>, not be \
         dropped: {out}"
    );
    assert!(
        out.contains(&sourcepos_attr),
        "the wrapper div's {sourcepos_attr} must move onto the surviving <pre>, not be \
         dropped: {out}"
    );
    assert!(
        out.contains("data-tali-cell=\"pyodide\""),
        "the reader's show/hide-code control needs something to target: {out}"
    );
    assert!(
        !out.contains("tali-js-out"),
        "the emptied output div must not survive degradation: {out}"
    );
    assert!(
        !out.contains("tali-pyodide-cell"),
        "the now-pointless wrapper div must not survive degradation: {out}"
    );
}

/// Finding 2 (task-4 review): `degrade_pyodide_cells` reverses `emit_client_cell`'s
/// `</script` -> `<\/script` escape by a blind string replace, which also un-escapes a
/// `<\/script` the AUTHOR typed literally — silently eating their backslash. That can't be
/// fixed after the fact (the HTML alone can't tell the two cases apart), so instead a
/// render-time warning fires while the real source is still available. This pins the
/// warning, on its message content, not merely on `warnings` being non-empty.
#[test]
fn a_pyodide_cell_with_a_literal_escaped_close_script_warns_at_render_time() {
    let src = "```{pyodide}\nx = \"literal <\\/script> marker\"\n```\n";
    let doc = render(src);

    let msg = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("<\\/script"))
        .unwrap_or_else(|| {
            panic!(
                "expected a warning naming the literal `<\\/script` sequence; got: {:?}",
                doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
            )
        });
    assert!(
        msg.message.contains("pyodide"),
        "the warning should name the cell kind it's about: {}",
        msg.message
    );
    assert!(
        msg.message.contains("backslash") || msg.message.contains("silently"),
        "the warning should say what goes wrong (the backslash is lost silently), not just \
         that the sequence exists: {}",
        msg.message
    );

    // A ```{js}``` cell (whose script content is never reversed) must not trip the same
    // warning — it is specific to the `{pyodide}` degrade path.
    let js_src = "```{js}\nx = \"literal <\\/script> marker\"\n```\n";
    let js_doc = render(js_src);
    assert!(
        !js_doc
            .warnings
            .iter()
            .any(|w| w.message.contains("<\\/script")),
        "a {{js}} cell is never round-tripped through degrade_pyodide_cells, so it must not \
         warn: {:?}",
        js_doc
            .warnings
            .iter()
            .map(|w| &w.message)
            .collect::<Vec<_>>()
    );
}

/// Extract `name="value"` from `html` as the exact attribute string, so a caller can
/// assert the SAME attribute (not merely the same attribute name) survived a transform.
/// Panics on a missing attribute: that is a broken test fixture, not the behavior under
/// test.
fn extract_attr(html: &str, name: &str) -> String {
    let needle = format!("{name}=\"");
    let start = html
        .find(&needle)
        .unwrap_or_else(|| panic!("no {name} attr in {html}"))
        + needle.len();
    let end = start + html[start..].find('"').expect("unterminated attr value");
    format!("{name}=\"{}\"", &html[start..end])
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

/// The index `<meta>` is the ONLY thing that tells the client enhancer where the 15.7 MiB
/// runtime lives, and it resolves three different ways. Nothing tested it: the branch shipped
/// with no assertion on `pyodide_index_meta` at all, while `pyodide_browser.rs`'s header
/// claimed a sibling test covered it. A wrong URL here is invisible to every server-side test
/// and fails only in the reader's browser, as a module-load error with no obvious cause.
///
/// The empty Build+Inline arm is not an omission, it is the signal that the page must degrade
/// (`degrade_pyodide_cells`), so it is asserted as a value rather than skipped.
#[test]
fn the_index_meta_resolves_per_mode_and_is_absent_without_pyodide_cells() {
    let body = render(PY).body_html();

    assert_eq!(
        pyodide_index_meta(&body, OutputMode::Preview, None),
        format!("<meta name=\"tali-pyodide-index\" content=\"{PREVIEW_PYODIDE_DIR}\">"),
        "preview serves the runtime from its own same-origin route"
    );
    assert_eq!(
        pyodide_index_meta(&body, OutputMode::Build, None),
        "",
        "a single self-contained file carries no runtime, and the empty string is what tells \
         the build to degrade the cell instead of shipping a wrapper that cannot boot"
    );
    // External is the site build and the portable folder: the prefix is the page-relative one
    // the assembler already computed for every other asset, so a nested chapter resolves too.
    assert_eq!(
        pyodide_index_meta(&body, OutputMode::Build, Some("../")),
        format!("<meta name=\"tali-pyodide-index\" content=\"../_assets/{PYODIDE_DIR_NAME}/\">"),
        "a nested page reaches _assets/ through its own relative prefix"
    );

    // The control: every arm above must be empty for a page with no `{pyodide}` cells, or the
    // gate is not a gate. A `{js}` page is the near miss that matters.
    let js = render(JS).body_html();
    for (mode, base) in [
        (OutputMode::Preview, None),
        (OutputMode::Build, None),
        (OutputMode::Build, Some("../")),
    ] {
        assert_eq!(
            pyodide_index_meta(&js, mode, base),
            "",
            "a page with no `{{pyodide}}` cell must never stamp the index meta"
        );
    }
}

/// `--bare`'s contract is zero `<script>`, and a `{pyodide}` cell's Python source lives INSIDE
/// its `<script>`. Stripping first therefore deleted the source outright: the artifact carried
/// two empty `<div class="cell tali-pyodide-cell">` husks, the author's code appeared nowhere,
/// and `warn_bare_exclusions` said nothing because it counts only `{js}` cells. Measured on
/// `corpus/reactive/pyodide.tmd` before the fix: 2 husks, 0 listings, `arange` 0 times.
#[test]
fn bare_output_keeps_a_pyodide_cells_source_as_a_listing_not_an_empty_husk() {
    let doc = render(PY);
    let page = taliesin_core::render_doc_to_page(&doc, "t", OutputMode::Bare);

    assert!(
        page.contains("<code class=\"language-python\">") && page.contains("arange"),
        "the author's Python must survive `--bare` as a visible listing: {page:.400}"
    );
    assert!(
        !page.contains("<script type=\"application/tali-pyodide\""),
        "and bare output still ships no client-cell script"
    );
}

/// Item 190c: the vendored version was typed as a literal TWICE — in `PYODIDE_DIR_NAME` and
/// again inside `PREVIEW_PYODIDE_DIR` — with nothing tying either to the bytes they name.
///
/// Both failure modes are silent. Bumping the payload without bumping the constants leaves a
/// *stale directory name* on a payload that changed, and the build serves that directory with
/// `Cache-Control: immutable`, so a reader who visited before the bump keeps the old runtime
/// forever. Bumping one constant and not the other splits the preview route from the build's
/// `_assets/` path. Nothing else in the suite reads upstream's version, so a wrong name here
/// Item 190a: a deck assembles its own page, and that shell never stamped the runtime index.
///
/// The consequence was narrow and total: a `{pyodide}` cell on a slide rendered, its enhancer
/// loaded, and `boot()` then rejected with "this page carries no runtime index" — in live
/// preview, where the route serving that runtime was already up and answering. No server-side
/// test saw it, because every existing assertion went through the *page* assembler.
#[test]
fn a_deck_stamps_the_pyodide_runtime_index_in_every_mode_that_can_carry_it() {
    let deck = render(&format!(
        "---\ntitle: D\nformat: deck\n---\n\n## Slide\n\n{PY}"
    ));
    assert_eq!(
        deck.format,
        taliesin_core::DocFormat::Reveal,
        "the fixture must actually be a deck, or this asserts nothing about the deck shell"
    );

    let preview = taliesin_core::render_doc_to_page(&deck, "d", OutputMode::Preview);
    assert!(
        preview.contains(&format!(
            "<meta name=\"tali-pyodide-index\" content=\"{PREVIEW_PYODIDE_DIR}\">"
        )),
        "a live deck must resolve the runtime against the route the server already serves"
    );

    // A deck in a SITE build links the shared `_assets/`. The base is recovered from
    // `deck_js`, not `app_js`: a deck deliberately links no page bundle, so `app_js` is `""`
    // here and the page assembler's trick would yield a root-relative path that 404s for any
    // deck below the site root. `sub/talk.html` is exactly that case.
    let external = taliesin_core::render_deck_to_page_external(
        &deck,
        "d",
        taliesin_core::ExternalAssets {
            app_css: "",
            app_js: "",
            katex_css: "../_assets/katex.css",
            mermaid_js: "../_assets/mermaid.js",
            jslibs_js: "../_assets/jslibs.js",
            deck_css: "../_assets/deck.abc.css",
            deck_js: "../_assets/deck.abc.js",
            font_preload: "",
        },
    );
    assert!(
        external.contains(&format!(
            "<meta name=\"tali-pyodide-index\" content=\"../_assets/{PYODIDE_DIR_NAME}/\">"
        )),
        "a nested site deck must climb to `_assets/` with the same prefix its deck.js used: \
         {external:.600}"
    );
    // The meta alone is not enough: the External arm gated the shared client-cell runtime on
    // `has_js_cells` (i.e. `{js}` only), so a deck whose only cells were `{pyodide}` shipped
    // an index pointing at a runtime nothing would ever load.
    assert!(
        external.contains("window.taliJs.registerLanguage(\"application/tali-pyodide\""),
        "a site deck with a `{{pyodide}}` cell must ship the pyodide enhancer"
    );

    // The control: a deck with no `{pyodide}` cell must stay clean in both modes, or the gate
    // is not a gate.
    let plain = render("---\ntitle: D\nformat: deck\n---\n\n## Slide\n\nProse only.\n");
    for mode in [OutputMode::Preview, OutputMode::Build] {
        assert!(
            !taliesin_core::render_doc_to_page(&plain, "d", mode)
                .contains("<meta name=\"tali-pyodide-index\""),
            "a deck with no `{{pyodide}}` cell must never stamp the index meta"
        );
    }
}

/// Item 190b: `build doc.tmd --out <dir>` produces a portable FOLDER, which has room for the
/// runtime beside `index.html` exactly as a site build does — but it inlines its assets, so it
/// took the single-file path and degraded a cell that could have run.
///
/// The needle here is why this is not `has_client_cells_of`: `assets/js/pyodide.js` contains
/// the bare mime string (it calls `registerLanguage` with it) and every page inlines its whole
/// JS payload, so on a finished page the bare needle is true whenever the enhancer shipped.
#[test]
fn attaching_the_runtime_index_is_precise_about_what_counts_as_a_cell() {
    let page = taliesin_core::render_doc_to_page(&render(PY), "t", OutputMode::Build);
    assert!(
        !page.contains("<meta name=\"tali-pyodide-index\""),
        "the inline assembler must still emit no index — the folder path adds it afterwards"
    );

    let attached = taliesin_core::attach_pyodide_index(&page, "");
    assert!(
        attached.contains(&format!(
            "<meta name=\"tali-pyodide-index\" content=\"_assets/{PYODIDE_DIR_NAME}/\">"
        )),
        "a page at a folder's root reaches `_assets/` with no climb"
    );
    assert!(
        attached.contains("</head>")
            && attached.find("tali-pyodide-index") < attached.find("</head>"),
        "the meta must land inside <head>, where the enhancer's querySelector looks"
    );
    assert!(
        attached.contains("<script type=\"application/tali-pyodide\""),
        "and the cell must NOT be degraded on this path — carrying the runtime is the point"
    );

    // The control that the mime-string trap would fail: a `{js}` page still inlines
    // `pyodide.js` in Preview mode (every gate is open there), so a bare-mime needle would
    // call this a pyodide page and stamp an index for a runtime it will never load.
    let js_preview = taliesin_core::render_doc_to_page(&render(JS), "t", OutputMode::Preview);
    assert!(
        js_preview.contains("\"application/tali-pyodide\""),
        "precondition: the preview page does inline the enhancer's mime string, which is the \
         whole reason this test exists"
    );
    assert_eq!(
        taliesin_core::attach_pyodide_index(&js_preview, ""),
        js_preview,
        "a page with no `{{pyodide}}` CELL must come back byte-identical"
    );
}
