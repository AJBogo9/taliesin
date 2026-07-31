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

/// Declared BEFORE the polyfill loads, because paged.js reads `window.PagedConfig` as it
/// loads: declared after, it is never seen. `crates/core/tests/print_page.rs` pins the order.
///
/// **`auto: false` is a deliberate choice, but NOT the load-bearing one this once claimed.**
/// It was documented as necessary because paginate-on-load "never finishes chunking a
/// document containing an image that actually loads", measured at a 10 s and a 60 s budget.
/// That was the `loading="lazy"` deadlock (see [`eager_media`]) wearing a different hat:
/// paged.js's own chunker also awaits image load, so a lazy image that is never requested
/// wedges it exactly as it wedged [`PAGED_START`]. Re-measured once images load eagerly,
/// `auto: true` with an `after:` stamp paginates `corpus/print/paged.tmd` to output
/// byte-identical to this path — same page count, same resolved `(p. N)` on every reference.
///
/// What the explicit start still buys is ordering that `auto: true` does not offer: chunking
/// begins only after `document.fonts.ready`, so page breaks are never computed against
/// fallback metrics and then shifted by a font swap. That is a conservative choice, not a
/// measured necessity — the corpus pin's fonts are bundled, so it would not detect the
/// difference either way. Keep it, but do not cite a hang for it.
///
/// **No `after:` hook here, deliberately.** paged.js runs
/// `if (config.auto !== false) { done = await previewer.preview(…) } if (config.after) {
/// await config.after(done) }` — so with `auto: false` an `after` hook fires *immediately*,
/// before any pagination. Stamping completion from it produced a page that captured
/// half-rendered: content present, but every `target-counter()` unresolved, so cross-refs
/// lost their page numbers and the margin boxes came out empty. The stamp must come from
/// the `preview()` promise, which is the only signal that actually means "paginated".
const PAGED_CONFIG: &str = "window.PagedConfig = { auto: false };";

/// Start pagination once the things that determine layout have settled — web fonts (a font
/// swap re-flows every line) and every image (see [`PAGED_CONFIG`]) — then stamp completion
/// from `preview()`'s own promise.
///
/// `i.onerror` is wired beside `i.onload` on purpose: a missing image must not wedge the
/// run, which is the same failure mode inverted.
///
/// **Neither event covers an image the browser never REQUESTS**, and that is the hole this
/// wait fell through for a whole session. A `loading="lazy"` image far from the viewport is
/// never fetched, so it stays `complete === false` and fires nothing at all — the wait
/// deadlocks before the chunker starts. [`eager_media`] closes it at the source; do not
/// weaken that and assume `onerror` is a backstop here, because it is not.
const PAGED_START: &str = "window.addEventListener('load', function () { \
     var pending = Array.prototype.slice.call(document.images) \
       .filter(function (i) { return !i.complete; }) \
       .map(function (i) { return new Promise(function (r) { i.onload = i.onerror = r; }); }); \
     Promise.all([document.fonts ? document.fonts.ready : Promise.resolve()].concat(pending)) \
       .then(function () { return window.PagedPolyfill.preview(); }) \
       .then(function () { document.documentElement.dataset.taliPaged = 'done'; }); \
   });";

/// Make every deferred `<img>`/`<iframe>` load eagerly.
///
/// `loading="lazy"` is a **scrolling** optimization: the browser starts the fetch only when
/// the element nears the viewport. A paginated rendering never scrolls, so anything far
/// enough down the document is never requested at all — and that is fatal twice over.
///
/// It hangs: [`PAGED_START`] waits for every image before it lets the chunker run, and a
/// lazy image that was never requested has `complete === false` with neither `load` nor
/// `error` ever firing, so the wait never settles and pagination never begins. Measured in
/// the headless driver: zero `.pagedjs_page` boxes, polyfill loaded, fonts settled.
///
/// And even if it did not hang, it would paginate wrongly: an unfetched image has no
/// intrinsic size, so the chunker would flow the document around a figure of zero height.
///
/// The screen renderer is right to emit it (`render/image_meta.rs` — lazy is a real win
/// below the fold), which is exactly why the correction belongs here, on the print page,
/// rather than in the shared emitter.
fn eager_media(html: &str) -> String {
    html.replace(" loading=\"lazy\"", " loading=\"eager\"")
}

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

    /// The tallest a figure's media is allowed to render.
    ///
    /// **This was once documented as a paged.js hang fix. That was wrong** — and the rule it
    /// feeds was never actually in `print.css`, so the claim was never even in effect
    /// (`git log -S __TALI_MAX_FLOAT_H__` on the stylesheet finds nothing). The hang it
    /// described was the `loading="lazy"` deadlock that [`eager_media`] now fixes; capping
    /// image height only ever *appeared* to cure it, by shrinking the document enough to pull
    /// the lazy image inside Chrome's load threshold.
    ///
    /// What the cap is really for is plainer. `print.css` sets `break-inside: avoid` on
    /// `figure`, so a figure taller than one page's content box can neither be broken nor
    /// fit: measured on a 600x3000 image, it bled past both page margins, lost its caption,
    /// and stranded two near-blank pages ahead of it.
    ///
    /// Values are the page height minus the 44 mm of vertical margin `print.css` sets, minus
    /// roughly a quarter for the caption and surrounding text, so a figure and its caption
    /// always fit together on one page.
    pub fn max_float_height(self) -> &'static str {
        match self {
            // 297 − 44 = 253 mm of content box.
            Paper::A4 => "190mm",
            // 279.4 − 44 = 235 mm.
            Paper::Letter => "175mm",
            // 210 − 44 = 166 mm.
            Paper::A5 => "125mm",
        }
    }
}

/// Build a list of figures from the assembled body, in document order.
///
/// Returns `""` when the document has no figures — an empty "List of Figures" heading is a
/// defect, not a degenerate case.
///
/// **This is a GENERATED block.** It exists only in the transient print page, so it is
/// structurally absent from `taliesin read`/`skim`, the search index and `llms-full.txt`.
/// `crates/core/tests/print_page.rs` pins that rather than trusting it: the same assumption
/// leaked four times in the reader-affordances batch.
fn list_of_figures(body: &str) -> String {
    let mut items = String::new();
    let mut rest = body;
    while let Some(start) = rest.find("<figure") {
        let after = &rest[start..];
        let Some(end) = after.find("</figure>") else {
            break;
        };
        let block = &after[..end];
        rest = &after[end..];

        // The leading SPACE is load-bearing. A figure opens
        // `<figure data-block-id="b-…" data-sourcepos="…" id="fig-x" …>`, and
        // `data-block-id="` itself contains the substring `id="` — matching without the
        // space would list every figure under its block hash instead of its anchor.
        let Some(id) = attr_value(block, " id=\"") else {
            continue;
        };
        if !id.starts_with("fig-") {
            continue;
        }
        let caption = block
            .find("<figcaption")
            .and_then(|c| block[c..].find('>').map(|g| c + g + 1))
            .and_then(|open| {
                block[open..]
                    .find("</figcaption>")
                    .map(|close| super::strip_tags(&block[open..open + close]))
            })
            .unwrap_or_default();
        let caption = caption.trim();
        if caption.is_empty() {
            continue;
        }
        items.push_str(&format!("<li><a href=\"#{id}\">{caption}</a></li>"));
    }
    if items.is_empty() {
        return String::new();
    }
    // `role="doc-loft"` is the DPUB-ARIA role for a list of figures.
    format!(
        "<nav class=\"tali-lof\" role=\"doc-loft\" aria-label=\"List of figures\">\
         <h2>List of Figures</h2><ol>{items}</ol></nav>"
    )
}

/// The value of the attribute opening with `name` (quote included), or `None`.
fn attr_value<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    let i = block.find(name)? + name.len();
    let j = block[i..].find('"')? + i;
    Some(&block[i..j])
}

/// Assemble the transient paginated page for `doc`.
///
/// The caller is expected to have run the document's cells already (the `pdf` command
/// mirrors `build`'s sequence, so python/r figures are present rather than empty).
pub fn print_page_from_doc(
    doc: &RenderedDoc,
    fallback_title: &str,
    paper: Paper,
    base_dir: &std::path::Path,
) -> String {
    let css = PRINT_CSS
        .replace("__TALI_PAPER__", paper.css_size())
        .replace("__TALI_MAX_FLOAT_H__", paper.max_float_height());
    // The print page is written to a TEMP directory, so every relative URL in the document
    // — images above all — would resolve against the wrong root and silently 404, leaving
    // alt text where a figure should be. A `<base href>` at the document's own directory
    // fixes them all at once, without copying assets around.
    //
    // It must come FIRST in the head: `<base>` only affects URLs that follow it.
    let base = format!(
        "<base href=\"file://{}/\">\n",
        base_dir
            .canonicalize()
            .unwrap_or_else(|_| base_dir.to_path_buf())
            .display()
    );
    let head = format!(
        "{base}<style>{css}</style>\n\
         <script>{PAGED_CONFIG}</script>\n\
         <script>{PAGED_JS}</script>\n\
         <script>{PAGED_START}</script>\n"
    );

    // Through page.rs's own policy, not a fourth copy of it (see `resolve_title`).
    let title = super::page::resolve_title(doc, fallback_title, false);
    let mut escaped = String::new();
    super::escape_html(&title, &mut escaped);

    let content = eager_media(&doc.body_html());
    // The LoF leads the document, ahead of the content it indexes.
    let body = format!("{}{content}", list_of_figures(&content));

    assemble_html_page(&PageParts {
        mode: OutputMode::Build,
        title: &escaped,
        lang: doc.lang.as_deref().unwrap_or("en"),
        // Paper is white and ink is dark: force the light palette rather than inheriting
        // whatever the reader last chose on screen.
        theme_default: "light",
        theme_css: &doc.theme_css,
        ship_katex: content.contains("class=\"katex"),
        extra_head: &head,
        body: &body,
        ..PageParts::defaults()
    })
}
