//! The client-side cell-language registry: the languages whose "kernel" is the reader's
//! browser rather than a Jupyter kernel.
//!
//! Before this existed, `{js}` was spelled `lang == "js"` in six places (the figure
//! materialization gate, the figure emitter's match, the plain-cell arm, the `--no-exec`
//! fallback, the asset gates, the reactive-graph diagnostic). Adding a second such
//! language meant finding all six, and missing one was silent — `reactive.rs`'s
//! `runtime_defines` check is the proof: it read "any cell that is not `js`" as "a cell
//! that could publish names at runtime", which a second browser-run language would have
//! quietly broken.
//!
//! Every registered language emits **the same wrapper contract**: an output target
//! `<div>` plus a sibling `<script type="{mime}">` carrying the author's source verbatim
//! with the `//|` options as `data-*` attributes. That is what lets one enhancer registry
//! on the client ([`tali-js.js`'s `registerLanguage`](../../assets/js/tali-js.js)) run
//! them all, and what keeps teardown, the reactive graph and click-to-source language-
//! agnostic.
//!
//! **This never touches exec/freeze/kernel.** A registered language is *not* executable in
//! `executes_to_kernel`'s sense; the two sets are disjoint by construction and
//! `client_langs_never_reach_a_kernel` pins that.

/// One client-side cell language.
pub struct ClientLang {
    /// The fence language: ` ```{js} ` -> `"js"`.
    pub lang: &'static str,
    /// The `<script type>` its source rides in, and the key the client registry looks up.
    pub mime: &'static str,
    /// The wrapper `<div>`'s class (after the shared `cell`).
    pub class: &'static str,
}

/// The registered client-side cell languages.
///
/// **One entry, and the registry is kept anyway.** The alternative is spelling `lang ==
/// "js"` back into the six places listed above, which is the shape this module's own
/// history records as silently wrong once. Every entry must earn its bytes:
/// `{sql}`/DuckDB and `{ts}`/esbuild stay cut until a corpus document needs one (each is a
/// multi-MB vendored payload and its own licence question). Two entries were withdrawn:
/// `{pyodide}`, which paid the multi-MB price for a CPython WASM build, and `{glsl}`, whose
/// shader enhancer cost no vendored bytes but served one purpose-built corpus page and
/// nothing a person wrote to be read. Neither name is diagnosed any more: a fence in an
/// unknown language renders as a listing, which is what a display fence has always done.
pub(crate) const CLIENT_LANGS: &[ClientLang] = &[ClientLang {
    lang: "js",
    mime: "application/tali-js",
    class: "tali-js-cell",
}];

/// The registry entry for a fence language, or `None` for a kernel/highlight-only one.
pub fn client_lang(lang: &str) -> Option<&'static ClientLang> {
    CLIENT_LANGS.iter().find(|c| c.lang == lang)
}

/// False for a registered client language whose runtime is unavailable in this build.
///
/// Every registered language runs on browser APIs alone today, so this is unconditionally
/// true. It is kept as a named seam rather than inlined because it is the gate the emitter
/// ANDs into "should this cell become live markup", and the alternative to a compiled-out
/// runtime emitting a live wrapper is an empty husk that loads nothing. A future language
/// with an optional payload wires in here and nowhere else.
pub fn client_lang_runnable(lang: &str) -> bool {
    client_lang(lang).is_some()
}

/// True if a rendered body carries a cell of any client-side language. Gates the shared
/// `tali-js.js` runtime, which every registered language's enhancer registers into.
pub fn has_client_cells(body: &str) -> bool {
    CLIENT_LANGS.iter().any(|c| body.contains(c.mime))
}

/// True if a rendered body carries a cell of one named client-side language. Gates that
/// language's own payload (d3 + Plot for `{js}`), so a page carrying only some other
/// registered language does not ship half a megabyte of plotting library.
pub fn has_client_cells_of(body: &str, lang: &str) -> bool {
    client_lang(lang).is_some_and(|c| body.contains(c.mime))
}
