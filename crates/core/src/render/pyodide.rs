//! Delivery of the vendored Pyodide runtime for `{pyodide}` cells.
//!
//! **Why this is a directory and not a hashed blob.** Every other vendored asset is a single
//! file the build renames to `app.<hash>.css`. Pyodide cannot be: `pyodide.mjs` resolves
//! `pyodide.asm.mjs`, `pyodide.asm.wasm` and `python_stdlib.zip` by FIXED name relative to
//! its `indexURL`, and `pyodide-lock.json` names the wheel. Renaming any of them breaks the
//! runtime at load. The version therefore lives in the DIRECTORY name, which is what makes
//! it cache-safe across a Pyodide bump.

use crate::OutputMode;

/// The version-stamped directory name, shared by the preview route and the build's
/// `_assets/`. Bumping Pyodide means bumping this, which busts every reader's cache.
pub const PYODIDE_DIR_NAME: &str = "pyodide-314.0.3";

/// Same-origin path both dev servers serve the vendored runtime from. A route rather than an
/// inline blob for the same reason `PREVIEW_MERMAID_PATH` is one, only more so: the page
/// shell is re-served on every navigation, and this payload is 12.9 MB.
pub const PREVIEW_PYODIDE_DIR: &str = "/_taliesin/pyodide-314.0.3/";

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

/// Rewrite every `{pyodide}` wrapper into visible highlighted source, for the one output path
/// that cannot carry the runtime: `build <file.tmd> out.html`.
///
/// **Re-emitting the source is the whole job.** The author's code lives inside the
/// `<script type="application/tali-pyodide">`, so stripping the script (what `--bare` does)
/// would leave an empty `<div>` and silently delete the content the reader came for.
pub fn degrade_pyodide_cells(body: &str) -> String {
    let spec = match crate::render::client_lang("pyodide") {
        Some(s) => s,
        None => return body.to_string(),
    };
    let open = format!("<script type=\"{}\"", spec.mime);
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(i) = rest.find(&open) {
        let (before, tail) = rest.split_at(i);
        out.push_str(before);
        let Some(gt) = tail.find('>') else {
            out.push_str(tail);
            return out;
        };
        let after_open = &tail[gt + 1..];
        let Some(end) = after_open.find("</script>") else {
            out.push_str(tail);
            return out;
        };
        let src = after_open[..end].replace("<\\/script", "</script");
        // The same shape `emit.rs` produces for a listing, so the degraded cell is
        // indistinguishable from an ordinary ```python block: `highlight` returns the
        // token spans only, and the caller supplies the `<pre><code class=…>` frame.
        out.push_str(&format!(
            "<pre><code class=\"language-python\">{}</code></pre>",
            crate::highlight::highlight(&src, Some("python"))
        ));
        rest = &after_open[end + "</script>".len()..];
    }
    out.push_str(rest);
    out
}
