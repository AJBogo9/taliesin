//! Fenced-code language validation.
//!
//! A fence whose language token resolves to no syntax degrades to escaped plain
//! text. That is silent: the block still renders, just unstyled, so a typo
//! (` ```pyton `) reads as "highlighting is broken" rather than "you misspelled
//! python". This surfaces it on the same located channel as the rest of the family.

use super::helpers::start_line;
use crate::highlight::known_language;
use crate::render::{Block, Warning};

/// The language token of every highlighted fence in `html`.
///
/// `emit.rs` writes the *raw* fence label into `class="language-{l}"`, on both
/// static fences and executable cells. `{mermaid}` cells never reach here: they
/// emit a bare `<pre class="mermaid">` with no `<code>` element. The literal
/// `class="language-` cannot appear in code *content*, because a block's text is
/// HTML-escaped (`class=&quot;language-`) before it is embedded.
///
/// **Known limitation.** Raw-HTML passthrough (an `HtmlBlock`, or a `{=html}` block)
/// is emitted verbatim, so an author who hand-writes `<code class="language-xyz">`
/// with a token no syntax matches is warned about a block this renderer never
/// highlighted in the first place. Nothing in the block model distinguishes emitted
/// code from passthrough HTML, and no corpus or docs page hits it, so the scan is
/// left simple rather than made structural. If it ever bites, that is the fix.
fn fence_languages(html: &str) -> Vec<&str> {
    scan_attr(html, "class=\"language-")
}

/// Every `"`-terminated value following `attr` in `html`.
fn scan_attr<'a>(html: &'a str, attr: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find(attr) {
        let start = i + pos + attr.len();
        let Some(len) = html[start..].find('"') else {
            break;
        };
        out.push(&html[start..start + len]);
        i = start + len;
    }
    out
}

/// Cell languages that Taliesin once ran and has since withdrawn: `(lang, what to do
/// instead)`.
///
/// **Why this register has to exist.** Fence languages are an *open* vocabulary (anything
/// syntect knows is legal), so a withdrawn one does not get a did-you-mean; it falls to the
/// generic "unknown code language" arm below. That message is actively misleading here: it
/// says "check the spelling", when the spelling was right and the *capability* is gone. This
/// is the same job `RETIRED_KEYS` does for front matter and `RETIRED_DIV_CLASSES` does for
/// fenced divs; see this repo's CLAUDE.md on why a withdrawn vocabulary item needs one.
///
/// `pyodide` ran Python in the reader's browser on a vendored 15.7 MiB CPython/WASM build.
/// It was withdrawn because the payload could only ever carry the stdlib plus NumPy (the
/// tool does no network fetch), which is exactly the workload `{js}` already covers at zero
/// marginal bytes.
/// The note is the REMEDY only. `validate_code_languages` prefixes the fixed lead-in
/// "is a retired cell language", which is what `diagnostics::codes` needles to classify the
/// family: without a stable phrase every entry here would have to be added to that table by
/// hand, and one that was forgotten would fall through to `(GENERIC, ERROR)` and fail
/// `check`/`build --strict` on a document whose only sin is being out of date.
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

/// Warn on any fenced code block whose language will not be highlighted.
///
/// Tokens that render plain on purpose (`text`, `console`, `output`, …) are
/// accepted by [`known_language`] and never warn. A *withdrawn* language is warned
/// about too, but with its own note rather than the generic spelling advice: see
/// [`RETIRED_CELL_LANGS`].
pub fn validate_code_languages(blocks: &[Block]) -> Vec<Warning> {
    let mut out = Vec::new();
    for b in blocks {
        // Retired CELLS first, and only cells. Two things make this the block model's
        // question rather than the emitted HTML's:
        //
        // 1. A withdrawn language is very often still a syntect-known token (`r` and
        //    `glsl` both are, `pyodide` is not), and nothing in the HTML separates
        //    ` ```{r} ` from a plain ` ```r ` display fence — both carry
        //    `class="language-r"`. What a retirement withdrew is *execution*, so a
        //    listing that merely SHOWS R code must stay silent.
        // 2. A cell with `#| echo: false` or `#| include: false` emits no listing at
        //    all, so an HTML scan reports nothing for it — which is the exact silence
        //    this register exists to prevent, and measured: three of the four `{r}`
        //    cells in `corpus/single-page-report` warned and the hidden one did not.
        //
        // `Block::cells` is the one accessor that answers it, and it descends into `:::`
        // containers, which reading `b.cell` directly would miss.
        let mut retired = Vec::new();
        for lang in b.cells().map(|c| c.lang.as_str()) {
            if let Some(note) = retired_cell_lang(lang) {
                retired.push(lang);
                let w = Warning::new(format!("`{{{lang}}}` is a retired cell language: {note}"));
                out.push(match start_line(&b.sourcepos) {
                    Some(l) => w.at(b.source_file.clone(), l),
                    None => w,
                });
            }
        }
        for lang in fence_languages(&b.html) {
            // Already reported as a retired cell on this block; the generic "check the
            // spelling" arm would otherwise double up on a language syntect never knew.
            if retired.contains(&lang) || known_language(lang) {
                continue;
            }
            let w = Warning::new(format!(
                "unknown code language `{lang}`: this block renders as plain text \
                 (check the spelling, or use `text` if that is intended)"
            ));
            out.push(match start_line(&b.sourcepos) {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}
