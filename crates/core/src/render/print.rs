//! The print/PDF track's page assembler (backlog 159).
//!
//! A **sibling** of [`super::page`], never a modification of it. That is what keeps every
//! normal page byte-identical: `crates/core/tests/body_html_snapshots.rs` must stay green
//! with no re-bless, and a required re-bless means this boundary leaked.
//!
//! **What this produces is terminal output.** paged.js clones and splits nodes across page
//! boundaries, which duplicates `data-block-id`. That is safe only because this artifact is
//! never served by preview, never diffed and never source-mapped. Do not wire it into
//! `serve`/`serve_site`.
//!
//! **`OutputMode::Bare` looks like the right mode and is not.** Its doc comment even says
//! "or a future print pipeline", but `Bare` emits zero `<script>` — and paged.js is itself a
//! script, as are the `{js}` cells whose output should appear in the PDF. `Build` is correct.

use super::model::{OutputMode, RenderedDoc};
use super::page::{PageParts, assemble_html_page};

/// The vendored polyfill, inlined so the print page is self-contained and offline.
///
/// It lives in `assets/js/` beside the other vendored bundles so it rides the existing
/// `vendored_js_is_attributed` gate — but unlike them it is inlined *here only*, never onto
/// a built page. See `THIRD_PARTY.md` and `assets/js/LICENSES.md`.
const PAGED_JS: &str = include_str!("../../assets/js/paged.polyfill.min.js");
const PRINT_CSS: &str = include_str!("../../assets/css/print.css");

/// The attribute `PagedConfig.after` stamps on `<html>` when pagination finishes; the CDP
/// driver in `taliesin-server`'s `pdf.rs` polls for it.
///
/// Deliberately the same idiom as `headless_js.rs`'s `data-tali-done`: one waiting
/// convention, two consumers. Polling a stamped attribute is what the Chrome CLI cannot do,
/// and is the reason this track needs CDP at all.
pub const PAGED_DONE_ATTR: &str = "data-tali-paged";

/// Paper size for a print render.
///
/// An **invocation** choice (a CLI flag), never document config — so this adds no
/// front-matter key. Minimal config: the default is chosen well rather than delegated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Paper {
    #[default]
    A4,
    Letter,
    A5,
}

impl Paper {
    /// Parse a `--paper` value. Case-insensitive; `None` for anything unrecognized so the
    /// caller can report the supported set rather than silently defaulting.
    pub fn parse(s: &str) -> Option<Paper> {
        match s.to_ascii_lowercase().as_str() {
            "a4" => Some(Paper::A4),
            "letter" => Some(Paper::Letter),
            "a5" => Some(Paper::A5),
            _ => None,
        }
    }

    /// The CSS `@page { size: … }` value.
    pub fn css_size(self) -> &'static str {
        match self {
            Paper::A4 => "210mm 297mm",
            Paper::Letter => "8.5in 11in",
            Paper::A5 => "148mm 210mm",
        }
    }

    /// The name accepted on the command line, for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Paper::A4 => "a4",
            Paper::Letter => "letter",
            Paper::A5 => "a5",
        }
    }
}

/// Assemble the transient paginated page for `doc`.
///
/// The caller is expected to have run the document's cells already (the `pdf` command
/// mirrors `build`'s sequence, so python/r figures are present rather than empty).
pub fn print_page_from_doc(doc: &RenderedDoc, fallback_title: &str, paper: Paper) -> String {
    let css = PRINT_CSS.replace("__TALI_PAPER__", paper.css_size());
    // Ordering is load-bearing: paged.js reads `window.PagedConfig` as it loads, so the
    // config must be declared FIRST. Declared after, it is never seen and the driver waits
    // for a stamp that never lands — a hang rather than an error.
    // `auto: true` keeps paged.js's own "paginate on load" behaviour; `after` is its
    // sanctioned completion hook.
    let head = format!(
        "<style>{css}</style>\n\
         <script>window.PagedConfig = {{ auto: true, after: function () {{ \
         document.documentElement.dataset.taliPaged = 'done'; }} }};</script>\n\
         <script>{PAGED_JS}</script>\n"
    );

    // Through page.rs's own policy, not a fourth copy of it (see `resolve_title`).
    let title = super::page::resolve_title(doc, fallback_title, false);
    let mut escaped = String::new();
    super::escape_html(&title, &mut escaped);

    let body = doc.body_html();

    assemble_html_page(&PageParts {
        mode: OutputMode::Build,
        title: &escaped,
        lang: doc.lang.as_deref().unwrap_or("en"),
        // Paper is white and ink is dark: force the light palette rather than inheriting
        // whatever the reader last chose on screen.
        theme_default: "light",
        theme_css: &doc.theme_css,
        ship_katex: body.contains("class=\"katex"),
        extra_head: &head,
        body: &body,
        ..PageParts::defaults()
    })
}
