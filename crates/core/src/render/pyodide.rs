//! Delivery of the vendored Pyodide runtime for `{pyodide}` cells.
//!
//! **Why this is a directory and not a hashed blob.** Every other vendored asset is a single
//! file the build renames to `app.<hash>.css`. Pyodide cannot be: `pyodide.mjs` resolves
//! `pyodide.asm.mjs`, `pyodide.asm.wasm` and `python_stdlib.zip` by FIXED name relative to
//! its `indexURL`, and `pyodide-lock.json` names the wheel. Renaming any of them breaks the
//! runtime at load. The version therefore lives in the DIRECTORY name, which is what makes
//! it cache-safe across a Pyodide bump.

use crate::OutputMode;

/// The vendored Pyodide version, as ONE literal.
///
/// A macro rather than a `const` because both public paths below are `const` and there is no
/// stable const string concatenation: `concat!` takes literals, so the macro is what lets the
/// version be written once and expanded into each. Before this, the version was typed twice
/// and a re-vendor that updated only one left the preview route serving
/// `/_taliesin/pyodide-<old>/` while the build wrote `_assets/pyodide-<new>/` — under
/// `Cache-Control: immutable`, which is the part that makes it expensive rather than merely
/// wrong. `the_vendored_pyodide_version_is_locked_to_the_payload` ties this literal to
/// upstream's own `package.json`, so bumping the payload without bumping this goes red.
macro_rules! pyodide_version {
    () => {
        "314.0.3"
    };
}

/// The version-stamped directory name, shared by the preview route and the build's
/// `_assets/`. Bumping Pyodide means bumping [`pyodide_version!`], which busts every
/// reader's cache.
pub const PYODIDE_DIR_NAME: &str = concat!("pyodide-", pyodide_version!());

/// Same-origin path both dev servers serve the vendored runtime from. A route rather than an
/// inline blob for the same reason `PREVIEW_MERMAID_PATH` is one, only more so: the page
/// shell is re-served on every navigation, and this payload is 12.9 MB.
pub const PREVIEW_PYODIDE_DIR: &str = concat!("/_taliesin/pyodide-", pyodide_version!(), "/");

macro_rules! payload_file {
    ($name:literal) => {
        (
            $name,
            include_bytes!(concat!("../../assets/pyodide/", $name)) as &[u8],
        )
    };
}

/// The vendored payload as (filename, bytes), for the dev servers to route and the build to
/// copy. `LICENSE` rides along: MPL-2.0 §3.4 forbids removing notices, so the licence travels
/// with the bytes into every built site, not just the source tree.
pub fn pyodide_payload() -> &'static [(&'static str, &'static [u8])] {
    &[
        payload_file!("pyodide.mjs"),
        payload_file!("pyodide.asm.mjs"),
        payload_file!("pyodide.asm.wasm"),
        payload_file!("python_stdlib.zip"),
        payload_file!("pyodide-lock.json"),
        payload_file!("numpy-2.4.3-cp314-cp314-pyemscripten_2026_0_wasm32.whl"),
        payload_file!("LICENSE"),
    ]
}

/// The `<meta>` the enhancer reads its `indexURL` from, or `""` when the page has no
/// `{pyodide}` cells. One tag serves all three asset modes, so `pyodide.js` needs no
/// knowledge of how the page was built.
///
/// `base` is `Some(rel)` only in `AssetMode::External`, where it is the page-relative prefix
/// the build already computes for every other asset (`asset_href`). An empty return in Build
/// + Inline is the single-file path, and is the signal to [`degrade_pyodide_cells`].
pub fn pyodide_index_meta(body: &str, mode: OutputMode, base: Option<&str>) -> String {
    if !crate::render::has_client_cells_of(body, "pyodide") {
        return String::new();
    }
    let url = match base {
        Some(rel) => format!("{rel}_assets/{PYODIDE_DIR_NAME}/"),
        None if mode == OutputMode::Preview => PREVIEW_PYODIDE_DIR.to_string(),
        None => return String::new(),
    };
    format!("<meta name=\"tali-pyodide-index\" content=\"{url}\">")
}

/// True if `html` carries a live `{pyodide}` **cell**, as opposed to merely mentioning the
/// mime type.
///
/// **Why not [`crate::render::has_client_cells_of`] here.** That helper needles the bare mime
/// string, which is correct against a rendered *body* but wrong against a finished *page*:
/// `assets/js/pyodide.js` calls `registerLanguage("application/tali-pyodide", …)`, and every
/// Taliesin page inlines its whole JS payload — so on a full page the bare needle is true
/// whenever the enhancer shipped, which in `Preview` is unconditional. The `<script type=`
/// prefix appears only in an emitted cell wrapper, so it distinguishes the two.
fn has_pyodide_cell_markup(html: &str) -> bool {
    html.contains("<script type=\"application/tali-pyodide\"")
}

/// Stamp the runtime index `<meta>` into an already-assembled page, for the one build path
/// that renders inline but still has somewhere to put a 12.9 MB directory: `build
/// <file.tmd> --out <dir>`.
///
/// **Why this is a post-pass and not an asset mode.** A portable folder inlines its CSS/JS on
/// purpose — being one directory you can move is the point of it — so it is `AssetMode::Inline`
/// like every other single-document output, and the inline assembler correctly emits no index
/// (a single `.html` file cannot carry the runtime). Only the *caller* knows a directory came
/// with it. Rather than thread a fourth asset mode through the whole page assembler for one
/// output path, this mirrors [`degrade_pyodide_cells`] — the other post-pass that runs over
/// finished page HTML at exactly the same point in `build.rs` — and the build picks one of the
/// two.
///
/// `base` is the page-relative prefix to `_assets/`; `""` for the folder's root `index.html`.
/// A no-op (byte-identical) when the page has no `{pyodide}` cell, so every other build is
/// unaffected. Inserts before the first `</head>`, which both the page shell and the deck
/// shell emit.
pub fn attach_pyodide_index(html: &str, base: &str) -> String {
    if !has_pyodide_cell_markup(html) {
        return html.to_string();
    }
    let meta = format!(
        "<meta name=\"tali-pyodide-index\" content=\"{base}_assets/{PYODIDE_DIR_NAME}/\">\n"
    );
    match html.find("</head>") {
        Some(i) => format!("{}{meta}{}", &html[..i], &html[i..]),
        // No `</head>` to anchor to: the shape has drifted from both assemblers. Leave the
        // page alone rather than guess at a position — a meta in the wrong place is worse
        // than the degraded listing the caller would otherwise have produced.
        None => html.to_string(),
    }
}

/// Rewrite every `{pyodide}` wrapper into a single highlighted `<pre>` of its source, for
/// the one output path that cannot carry the runtime: `build <file.tmd> out.html`.
///
/// **The whole wrapper is replaced, not just the `<script>`.** The author's code lives
/// inside the `<script type="application/tali-pyodide">`, so stripping just the script
/// (what `--bare` does) would leave the surrounding `<div class="cell tali-pyodide-cell">`
/// and its now-dead `<div class="tali-js-out">` output target behind: an empty husk with
/// nothing left to fill it. The surviving `<pre>` instead carries the original wrapper
/// `<div>`'s block attrs (`data-block-id`/`data-sourcepos`/…, moved rather than dropped —
/// every emitted block must carry them, see `corpus.rs`) plus `data-tali-cell="pyodide"`,
/// the same shape `emit.rs` produces for an ordinary ```python fence, so the reader's
/// show/hide-code control still targets it.
///
/// **One ambiguity survives, by inherent limit, not oversight.** `emit_client_cell`
/// escapes a literal `</script` in the author's source to `<\/script` so it survives
/// inside the `<script>` element; this function reverses that to recover the source. But
/// the forward escape is lossy: an author who typed the literal sequence `<\/script`
/// themselves (backslash and all) produces the exact same `<\/script` in the HTML that a
/// real `</script` would, and by the time this function runs the two are
/// indistinguishable — the original `.tmd` source isn't reachable here
/// (`BuildResult::Page` carries only the rendered `html: String`). Reversing blindly would
/// silently eat that author's backslash, and this function has no way to detect or fix
/// that from HTML alone. What keeps it from being silent is a render-time warning (see the
/// `{pyodide}`-cell scan in `render/mod.rs`, where the real source is still available)
/// whenever a `{pyodide}` cell's source contains a literal `<\/script`.
pub fn degrade_pyodide_cells(body: &str) -> String {
    let spec = match crate::render::client_lang("pyodide") {
        Some(s) => s,
        None => return body.to_string(),
    };
    // The fixed tail of the wrapper `<div>`'s opening tag that `emit_client_cell` emits:
    // `<div{block_attrs} class="cell tali-pyodide-cell">`. Anchoring on it (rather than on
    // the `<script>` alone, as a prior version of this function did) is what lets the whole
    // wrapper be replaced: everything between `<div` and this marker is `block_attrs`,
    // carried onto the surviving `<pre>` below.
    let class_marker = format!(" class=\"cell {}\">", spec.class);
    let open_script = format!("<script type=\"{}\"", spec.mime);
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(class_i) = rest.find(&class_marker) {
        let Some(div_i) = rest[..class_i].rfind("<div") else {
            // No enclosing `<div` — not actually a wrapper this function understands.
            // Leave everything as-is rather than guess at a boundary.
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..div_i]);
        let attrs = &rest[div_i + "<div".len()..class_i];
        let after_open_tag = &rest[class_i + class_marker.len()..];
        let Some(script_i) = after_open_tag.find(&open_script) else {
            out.push_str(&rest[div_i..class_i + class_marker.len()]);
            rest = after_open_tag;
            continue;
        };
        let tail = &after_open_tag[script_i..];
        let Some(gt) = tail.find('>') else {
            out.push_str(&rest[div_i..]);
            return out;
        };
        let after_script_open = &tail[gt + 1..];
        let Some(end) = after_script_open.find("</script>") else {
            out.push_str(&rest[div_i..]);
            return out;
        };
        let src = after_script_open[..end].replace("<\\/script", "</script");
        let after_close_script = &after_script_open[end + "</script>".len()..];
        // The outer wrapper's own closing tag, immediately adjacent per
        // `emit_client_cell`'s fixed shape. If it isn't there, the emitted shape has
        // drifted from what this function expects: bail rather than guess.
        let Some(after_wrapper) = after_close_script.strip_prefix("</div>") else {
            out.push_str(&rest[div_i..]);
            return out;
        };
        // The same shape `emit.rs` produces for a listing: `highlight` returns the token
        // spans only, and the frame + `data-tali-cell` mark it as a cell's source, same as
        // an ordinary ```python fence's `<pre>` would carry.
        out.push_str(&format!(
            "<pre{attrs} data-tali-cell=\"pyodide\"><code class=\"language-python\">{}</code></pre>",
            crate::highlight::highlight(&src, Some("python"))
        ));
        rest = after_wrapper;
    }
    out.push_str(rest);
    out
}
