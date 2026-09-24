//! The `{js}` cell: the one cell language whose "kernel" is the reader's browser rather than
//! a Jupyter kernel.
//!
//! Before one predicate named it, `{js}` was spelled `lang == "js"` in six places (the
//! figure materialization gate, the figure emitter's match, the plain-cell arm, the
//! `--no-exec` fallback, the asset gates, the reactive-graph diagnostic), and missing one
//! was silent: `reactive.rs`'s `runtime_defines` check read "any cell that is not `js`" as
//! "a cell that could publish names at runtime". Every one of those places asks
//! [`is_client_lang`] now. It was a registry of such languages until `{glsl}` and
//! `{pyodide}` were withdrawn; its one entry is the two consts below.
//!
//! A `{js}` cell emits an output target `<div>` plus a sibling `<script type=
//! "{JS_CELL_MIME}">` carrying the author's source verbatim with the `//|` options as
//! `data-*` attributes, which [`tali-js.js`](../../assets/js/tali-js.js) looks up to run it.
//!
//! **This never touches exec/freeze/kernel.** A `{js}` cell is not executable in
//! `executes_to_kernel`'s sense; `client_langs_never_reach_a_kernel` pins that.

/// The `<script type>` a `{js}` cell's source rides in, and the type `tali-js.js` runs.
pub const JS_CELL_MIME: &str = "application/tali-js";

/// The parameters a `{js}` cell body receives, in order: `tali-js.js` compiles each cell as
/// its own `AsyncFunction` over these names and the source.
pub const JS_CELL_PARAMS: &[&str] = &["tali", "Plot", "d3", "container", "invalidation"];

/// The `{js}` cell wrapper `<div>`'s class (after the shared `cell`).
pub(super) const JS_CELL_CLASS: &str = "tali-js-cell";

/// Whether a fence language runs in the reader's browser (`{js}`) rather than in a kernel
/// or not at all.
pub fn is_client_lang(lang: &str) -> bool {
    lang == "js"
}

/// True if a rendered body carries a `{js}` cell: an element with `type="{JS_CELL_MIME}"`,
/// read through the one walker. Gates the cell runtime and the d3 + Plot libraries. A
/// substring `contains(mime)` answered for prose that merely names the type, and shipped
/// d3, Plot and the cell runtime to a page with no cell on it.
pub fn has_js_cells(body: &str) -> bool {
    super::attr_values(body, "type").any(|t| t == JS_CELL_MIME)
}
