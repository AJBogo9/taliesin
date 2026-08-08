//! The retired-cell-language register, and the scan that makes it speak.
//!
//! The generic "unknown code language" lint (a ` ```pyton ` typo renders as unstyled plain
//! text) was cut on 2026-08-08: the defect is visible in the preview, which is the test the
//! surviving families are kept by. What stays is the half that is *not* visible anywhere:
//! a cell in a language Taliesin used to run and has since withdrawn.

use super::helpers::start_line;
use crate::render::{Block, Warning};

/// Cell languages that Taliesin once ran and has since withdrawn: `(lang, what to do
/// instead)`.
///
/// **Why this register has to exist.** Fence languages are an *open* vocabulary (anything
/// syntect knows is legal), so a withdrawn one draws no diagnostic at all on its own: the
/// cell simply stops executing and renders as a listing, which reads as "my kernel is
/// broken" rather than "this language is gone". This is the same job `RETIRED_KEYS` does for
/// front matter and `RETIRED_DIV_CLASSES` does for fenced divs; see this repo's CLAUDE.md on
/// why a withdrawn vocabulary item needs one.
///
/// `pyodide` ran Python in the reader's browser on a vendored 15.7 MiB CPython/WASM build.
/// It was withdrawn because the payload could only ever carry the stdlib plus NumPy (the
/// tool does no network fetch), which is exactly the workload `{js}` already covers at zero
/// marginal bytes.
/// The note is the REMEDY only. [`validate_retired_cell_langs`] prefixes the fixed lead-in
/// "is a retired cell language".
pub const RETIRED_CELL_LANGS: &[(&str, &str)] = &[
    (
        "pyodide",
        "it ran Python in the reader's browser and was removed along with its vendored \
         runtime. Use `{js}` for computation that runs in the reader's browser, or \
         `{python}` for computation that runs against a kernel at build time",
    ),
    (
        "glsl",
        "it compiled a fragment shader onto a live <canvas> and was removed on 2026-08-08. \
         Use a `{js}` cell, which can reach WebGL (or three.js) directly",
    ),
    (
        "r",
        "it executed against an IRkernel and was removed on 2026-08-08. Use `{python}` for \
         computation that runs against a kernel at build time, or a plain ```r block to \
         display R code without running it",
    ),
];

/// The retirement note for a [`RETIRED_CELL_LANGS`] entry, or `None` if `lang` was never
/// retired.
pub fn retired_cell_lang(lang: &str) -> Option<&'static str> {
    RETIRED_CELL_LANGS
        .iter()
        .find(|(l, _)| *l == lang)
        .map(|(_, note)| *note)
}

/// Warn on every cell written in a [`RETIRED_CELL_LANGS`] language, with that entry's own
/// note rather than generic spelling advice.
///
/// Cells, and only cells. Two things make this the block model's question rather than the
/// emitted HTML's:
///
/// 1. A withdrawn language is very often still a syntect-known token (`r` and `glsl` both
///    are, `pyodide` is not), and nothing in the HTML separates ` ```{r} ` from a plain
///    ` ```r ` display fence — both carry `class="language-r"`. What a retirement withdrew
///    is *execution*, so a listing that merely SHOWS R code must stay silent.
/// 2. A cell with `#| echo: false` or `#| include: false` emits no listing at all, so an
///    HTML scan reports nothing for it — which is the exact silence this register exists to
///    prevent, and measured: three of the four `{r}` cells in `corpus/single-page-report`
///    warned and the hidden one did not.
///
/// [`Block::cells`] is the one accessor that answers it, and it descends into `:::`
/// containers, which reading `b.cell` directly would miss.
pub fn validate_retired_cell_langs(blocks: &[Block]) -> Vec<Warning> {
    let mut out = Vec::new();
    for b in blocks {
        for lang in b.cells().map(|c| c.lang.as_str()) {
            if let Some(note) = retired_cell_lang(lang) {
                let w = Warning::new(format!("`{{{lang}}}` is a retired cell language: {note}"));
                out.push(match start_line(&b.sourcepos) {
                    Some(l) => w.at(b.source_file.clone(), l),
                    None => w,
                });
            }
        }
    }
    out
}
