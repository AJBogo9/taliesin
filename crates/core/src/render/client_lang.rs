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
/// **Deliberately short, and every entry must earn its bytes.** `{sql}`/DuckDB and
/// `{ts}`/esbuild stay cut until a corpus document needs one (each is a multi-MB vendored
/// payload and its own licence question); `{glsl}` earned its place by needing neither,
/// because WebGL is a browser API, so the whole language costs one small enhancer and no
/// vendored bytes. `{pyodide}` was the one entry that DID pay the multi-MB price (a CPython
/// WASM build) and it was **withdrawn**: see [`crate::diagnostics::RETIRED_CELL_LANGS`] for
/// what an author who still has one gets told, and `notes/DO-NOT-REBUILD.md` for the ruling.
pub(crate) const CLIENT_LANGS: &[ClientLang] = &[
    ClientLang {
        lang: "js",
        mime: "application/tali-js",
        class: "tali-js-cell",
    },
    ClientLang {
        lang: "glsl",
        mime: "application/tali-glsl",
        class: "tali-glsl-cell",
    },
];

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
/// language's own payload (d3 + Plot for `{js}`, `glsl.js` for `{glsl}`), so a shader page
/// does not ship half a megabyte of plotting library and a chart page does not ship a
/// WebGL enhancer.
pub fn has_client_cells_of(body: &str, lang: &str) -> bool {
    client_lang(lang).is_some_and(|c| body.contains(c.mime))
}
