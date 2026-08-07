//! The render block model: the data types the parser emits and the dev server
//! consumes (cells, blocks, the rendered doc, page includes). Split out of
//! mod.rs so the data model is separate from the render pipeline + emission.

/// An executable code cell (```` ```{lang} ````), exposed so the dev
/// server can run it against a kernel.
#[derive(Debug, Clone)]
pub struct Cell {
    pub lang: String,
    pub code: String,
    /// When this cell's output should be a numbered `<figure>` (`#| label: fig-x`
    /// + `#| fig-cap:`), the executor wraps the rendered output accordingly.
    pub figure: Option<CellFigure>,
    /// When this cell's output is a numbered, captioned table (from `#| label:
    /// tbl-x` / `#| tbl-cap:`), the executor injects the `#tbl-x` id and a "Table N"
    /// caption into the executed `<table>` so `@tbl-x` cross-references resolve.
    pub table: Option<CellTable>,
    /// `#| echo: false` hides the source (the cell still runs, output still shows).
    pub echo: bool,
    /// `#| include: false` hides both source and output, but the cell still runs
    /// (so downstream cells see its kernel state).
    pub include: bool,
    /// `#| cache: false` opts this cell out of the persistent execution cache: it
    /// always re-executes and its output is never written to `_freeze/`. The escape
    /// hatch for non-deterministic cells (RNG/time/network) whose output shouldn't
    /// be replayed. Defaults to `true` (cacheable).
    pub cache: bool,
    /// Native `{js}` cell options (`//| name:`/`//| viewof:`/`//| input:`). Empty
    /// for every other language; drives how the cell wires into the `{js}` runtime.
    pub js: JsOpts,
}

/// Options for a native interactive `{js}` cell (the Observable-runtime
/// replacement). `name`: publish the cell's return value into the shared scope
/// under this name (a helper other cells read). `viewof`: the cell returns a DOM
/// input registered under this name. `inputs`: re-run this cell when any of these
/// named inputs (or Python `ojs_define` values) change.
#[derive(Debug, Clone, Default)]
pub struct JsOpts {
    pub name: Option<String>,
    pub viewof: Option<String>,
    pub inputs: Vec<String>,
    /// `//| trace: true`: a `{js}` cell inside `::: {.debug}` whose generator is
    /// captured client-side instead of mounted. Read at the branch that decides how
    /// the cell is emitted (`mod.rs`), because that decision has to be made before
    /// the emitted HTML carries the `data-tali-trace="1"` attribute `divs.rs` reads
    /// for everything else (the two languages' traced cells are detected the same
    /// way once rendered, but a `{js}` one needs a DIFFERENT render path to get there:
    /// source display, not the live target-div-plus-script wrapper), so this field
    /// exists only to make that upstream decision. `emit.rs` re-derives the same fact
    /// from the raw literal rather than trusting this field, so there is still one
    /// source of truth for the attribute itself.
    pub trace: bool,
}

/// Metadata for wrapping a code cell's executed output as a numbered figure.
#[derive(Debug, Clone)]
pub struct CellFigure {
    /// `#fig-…` anchor (when labelled), so `@fig-x` cross-references resolve.
    pub anchor: Option<String>,
    pub caption: Option<String>,
    /// The number as displayed: chapter-scoped ("2.3") in a numbered book chapter, else
    /// a flat count ("3"). Rendered verbatim, so the scoping decision stays with the
    /// renderer that knows the chapter.
    pub number: String,
}

/// Metadata for captioning/numbering a code cell's executed `<table>` output.
#[derive(Debug, Clone)]
pub struct CellTable {
    /// `#tbl-…` anchor (when labelled), so `@tbl-x` cross-references resolve.
    pub anchor: Option<String>,
    pub caption: Option<String>,
    /// Assigned in document order (alongside Markdown tables) so the "Table N" the
    /// caption shows matches what `@tbl-x` resolves to. Chapter-scoped ("2.3") in a
    /// numbered book chapter, else a flat count ("3"); rendered verbatim.
    pub number: String,
}

/// A code cell's cross-reference role from its `label`/`*-cap` options.
pub(crate) enum CellRole {
    /// `label: fig-x` / `fig-cap` -> a numbered figure (the cell's output).
    Figure {
        anchor: Option<String>,
        caption: Option<String>,
    },
    /// `label: lst-x` / `lst-cap` -> a numbered listing (the cell's source);
    /// `fold` carries `code-fold` (start-open, summary) so a folded listing works.
    Listing {
        anchor: Option<String>,
        caption: Option<String>,
        fold: Option<(bool, String)>,
    },
    /// `label: tbl-x` / `tbl-cap` -> a numbered table (the cell's executed output).
    Table {
        anchor: Option<String>,
        caption: Option<String>,
    },
}

/// One top-level block: a stable id, its source position, and its HTML.
#[derive(Debug, Clone)]
pub struct Block {
    /// Content-hash id (`b-<hex>`), with a positional tiebreak (`-N`) for duplicates.
    pub id: String,
    /// Sourcepos as `startLine:startCol-endLine:endCol`, relative to `source_file`.
    ///
    /// **Empty is a value, not a gap — read this before writing a new producer.** Nine
    /// call sites construct a `Block` that no line of `.tmd` produced (the References
    /// section, the footnotes section, a book's generated TOC, "cite this"),
    /// and every one of them writes `String::new()` on purpose. It means "my content is
    /// gathered from lines scattered all over the document, so no single range is
    /// honest" — which is a different claim from "I forgot to thread the position
    /// through", and the reader gets a better answer from the first.
    ///
    /// The web client honours that distinction: `usableSourcepos` in
    /// `web-client/client.js` accepts only `L:C…` with a 1-based line, and Ctrl-click
    /// walks *past* an unusable block to the nearest ancestor that has one. So an empty
    /// sourcepos costs nothing — click-to-source simply resolves to the enclosing thing
    /// that does know where it came from, and a nested unit that knows its own line (a
    /// footnote `<li>` carries its definition's) still wins.
    ///
    /// **Do not invent a plausible-looking range to fill this in.** `openSource()`
    /// defaults an unparseable sourcepos to line 1, so a fabricated position does not
    /// fail visibly — it jumps the editor to the top of the file and looks deliberate.
    /// That was a real defect: every entry in the References list, and the footnotes
    /// section's own rule, navigated to line 1. Landing nowhere is the honest answer.
    /// `0:…` is rejected for the same reason (editors are 1-based, so line 0 is not a
    /// place).
    pub sourcepos: String,
    /// Origin file when the block came from an `{{< include >}}`d file
    /// (relative to the primary document's directory); `None` for the primary
    /// document. Drives click-to-source across files.
    pub source_file: Option<String>,
    /// Rendered HTML for this block, root element carrying the data attributes.
    pub html: String,
    /// Present when this block is an executable code cell.
    pub cell: Option<Cell>,
    /// The executable cells this block folded away, in document order. Non-empty only for
    /// a `:::` container (see [`super::divs`]); every other block leaves it empty.
    ///
    /// A container concatenates its children into ONE `html` string, so a nested cell's
    /// own `Block` — and with it the `cell` the executor looks for — stops existing at
    /// that point. `Executor::run_through` scans only top-level blocks, so the cell
    /// rendered and never ran: a `{python}` cell in a `.callout-note` or a
    /// `.panel-tabset` was dead source (backlog item 210). The folded child blocks are
    /// kept here instead, and the container leaves an empty
    /// [`super::CELL_OUT_SLOT_ATTR`] slot after each one in `html`, so the executor
    /// puts the output back INSIDE the container rather than after it — which is the
    /// difference between a tab's output appearing in its own panel and every tab's
    /// output stacked below the tabset, hidden ones included.
    ///
    /// Entries are the child blocks themselves (same id, sourcepos, source_file and
    /// html), so the executor can ask a nested cell exactly the questions it asks a
    /// top-level one. They are already flattened when a container folds another
    /// container, so an entry's own `nested` is always empty.
    pub nested: Vec<Block>,
}

impl Block {
    /// Every block carrying a code cell that this one contributes, in document order: the
    /// cells a `:::` container folded away (they are *inside* it, so they come first), then
    /// this block itself when it is a cell. Yields nothing for ordinary prose.
    ///
    /// **ONE definition, and reading `self.cell` directly instead is the bug.** "Which cells
    /// does this document have, in what order" is asked from at least seven places — the
    /// executor, the editor's freeze-cache lens, `--cell N` resolution, the preview's
    /// kernel-free bypass lane, `check`'s used-languages report, the reproduce block, and
    /// the static anchor lint — and every one of them that reads `cell` alone silently
    /// forgets each cell inside a callout or a tabset. That is the exact shape of the defect
    /// [`Block::nested`] exists to close, so it is worth one method rather than seven
    /// chances to reintroduce it.
    pub fn cell_blocks(&self) -> impl Iterator<Item = &Block> {
        self.nested
            .iter()
            .chain(std::iter::once(self).filter(|b| b.cell.is_some()))
    }

    /// [`Block::cell_blocks`] for a caller that needs the cells but not their block
    /// identity (a language census, "does this page have any cells at all").
    pub fn cells(&self) -> impl Iterator<Item = &Cell> {
        self.cell_blocks().filter_map(|b| b.cell.as_ref())
    }
}

/// The output format the document targets, taken from its front matter
/// `format:` key. Drives which page scaffold (and live client) is emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocFormat {
    /// A standard HTML page (blog post, book): the default.
    #[default]
    Html,
    /// A slide deck, rendered by taliesin's OWN native engine (reveal.js was removed);
    /// selected by `format: deck` / `*-deck`.
    Reveal,
}

/// How a page is being emitted, which decides how much optional machinery ships.
/// Threaded from the build CLI through the page builders onto [`PageParts`]; the
/// live preview always uses [`OutputMode::Preview`] so the dev loop is untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// Live dev preview: ship every enhancer unconditionally (a doc can gain any
    /// construct on an edit, so gating would race the live mount). The default.
    #[default]
    Preview,
    /// A static `build`/`render`: content-gate the DOM-specific enhancer scripts to
    /// what the page actually contains. `code-enhance.js` (reader menu + a11y) still
    /// ships on every page.
    Build,
    /// A bare single-doc build: zero `<script>`, zero CDN, CSS-only theming. For a
    /// rough draft, an archive, or a future print pipeline.
    Bare,
}

/// A non-fatal render warning, optionally carrying a click-to-source location.
/// When `line` is `Some`, the dev server renders it as a clickable diagnostic
/// (jump-to-source); `file` is doc-base-relative (matching `Block::source_file`)
/// or `None` for "the document being previewed". `line: None` is an unlocated
/// warning (logged + shown, not clickable), the same behavior bare strings had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    /// 1-based start column (Unicode-scalar count on the line); `None` = whole-line.
    pub col: Option<u32>,
    /// 1-based, exclusive end column; set together with `col`.
    pub end_col: Option<u32>,
}

impl Warning {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            file: None,
            line: None,
            col: None,
            end_col: None,
        }
    }

    /// Attach a click-to-source location.
    pub fn at(mut self, file: Option<String>, line: u32) -> Self {
        self.file = file;
        self.line = Some(line);
        self
    }

    /// Attach a `[col, end_col)` character span on the located line (1-based, exclusive end).
    pub fn span(mut self, col: u32, end_col: u32) -> Self {
        self.col = Some(col);
        self.end_col = Some(end_col);
        self
    }
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// A rendered document: front-matter metadata plus ordered blocks.
#[derive(Debug, Clone)]
pub struct RenderedDoc {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    /// Deck chrome (front-matter `footer:`/`logo:`), rendered as a persistent overlay
    /// on every slide by the deck builders. `footer` is plain text; `logo` is an image
    /// URL/path (resolved like any slide image). Ignored on non-deck documents.
    pub footer: Option<String>,
    pub logo: Option<String>,
    /// Front-matter `lang:` (a BCP-47 tag like `en`/`fr`), emitted as `<html lang>`
    /// by the page builders. `None` falls back to `en`.
    pub lang: Option<String>,
    /// Front-matter `description`, used for the SEO/OpenGraph meta on a standalone
    /// page (site pages get richer per-page meta from their `Page`).
    pub description: Option<String>,
    /// Whether this is a *dated* document (a post/article), the same gate the reading-time
    /// estimate uses. Drives the standalone `og:type` (`article` vs `website`) so a generic
    /// undated page isn't mislabelled an article to crawlers.
    pub is_article: bool,
    pub format: DocFormat,
    /// Whether this doc shows a table of contents. For a standalone render this
    /// is the front-matter `toc:` (default off); inside a site it is recomputed
    /// from `Site::page_toc` so the site default can apply.
    pub toc: bool,
    /// The page's explicit front-matter `toc:` as a tri-state: `Some(true/false)`
    /// when set, `None` when absent. The site uses this so an explicit `toc: false`
    /// overrides the site-wide default (a plain `bool` can't tell "off" from "unset").
    pub toc_explicit: Option<bool>,
    /// Resolved custom theme CSS (`.css`/`_extensions/`), empty for the built-in
    /// light/dark themes. Inlined after the base stylesheet.
    pub theme_css: String,
    /// Default theme mode for the resolver script: `"dark"`/`"light"` force it,
    /// `"auto"` follows the OS `prefers-color-scheme`.
    pub theme_default: String,
    /// Whether a custom `theme:` (a CSS file or bundle) owns this doc's colours.
    /// Decks use this to skip the built-in light/dark management when a custom theme
    /// is in charge.
    pub theme_is_custom: bool,
    /// Resolved `include-in-header`/`include-before-body`/`include-after-body` +
    /// `css` from the doc's front matter, injected into the page template.
    pub includes: PageIncludes,
    /// Non-fatal render warnings (a missing `bibliography:`/`theme:` file, …): the
    /// core can't return a `Result`, so it reports these for the server to log +
    /// surface in the dev menu. Front-matter typo warnings are separate (the
    /// `frontmatter` linter runs off the source).
    pub warnings: Vec<Warning>,
    /// Cross-reference anchor → its rendered number (`fig-x`→"3", a book `sec-x`→"2.1",
    /// `thm-x`, `tbl-x`, `eq-x`, `lst-x`). Only the RENDER knows figure/equation/theorem
    /// numbers (they're assigned during emission), so a site build harvests this map per
    /// page to give CROSS-PAGE `@ref`s their number — the source-scan xref pass can't.
    pub xref_numbers: std::collections::HashMap<String, String>,
    pub blocks: Vec<Block>,
}

/// How a page's framework CSS/JS is delivered. `Inline` bakes every blob into the page
/// (the portable single-file build, `--bare`, and live preview). `External` links to
/// content-hashed shared files under `_assets/` (the multi-page `build <dir>` path).
pub enum AssetMode<'a> {
    Inline,
    External(ExternalAssets<'a>),
}

/// Depth-adjusted hrefs for the shared `_assets/` files, supplied per page by the build.
///
/// `deck_css`/`deck_js` are the deck engine's own pair (a deck's stylesheet is `deck.css`,
/// not the page's base + site chrome, so it cannot share `app_css`). They are written only
/// when the build has a deck to link them, and are `""` otherwise — an ordinary page never
/// reads them.
pub struct ExternalAssets<'a> {
    pub app_css: &'a str,
    pub katex_css: &'a str,
    pub app_js: &'a str,
    pub mermaid_js: &'a str,
    pub jslibs_js: &'a str,
    pub deck_css: &'a str,
    pub deck_js: &'a str,
    /// The roman body face, for a `<link rel="preload" as="font">` ahead of the stylesheet
    /// (item 150). Unlike the `url()` refs *inside* the sheet, this href is resolved against
    /// the **page**, so it carries the depth climb.
    ///
    /// Only the roman face: preload is an eager fetch, and the italic is a minority of a
    /// page's text, so preloading it would pull 64 KB on every page for text most pages do
    /// not have. `""` disables the link (nothing to preload).
    pub font_preload: &'a str,
}

/// Ready-to-inject markup from the `include-in-header` / `include-before-body` /
/// `include-after-body` / `css` front-matter (and site `format: html:`) keys.
/// Each string is already resolved (inline `text:` or a referenced file's
/// contents; `css` files wrapped in `<style>`), so the template just drops it in.
///
/// These strings are injected **verbatim, unescaped**: the author is trusted
/// (see the crate-level "Trust model" doc). Don't populate them from any
/// untrusted source without sanitizing first.
#[derive(Debug, Clone, Default)]
pub struct PageIncludes {
    pub in_header: String,
    pub before_body: String,
    pub after_body: String,
}

/// What a page inherits from its project's `_site.yml`. One value rather than a parameter
/// per key, so adding the next project-wide policy does not widen six render signatures
/// again. `None` at a render entry point means "no project": a single `.tmd` invoked
/// directly, which is byte-identical to the pre-existing behaviour.
///
/// `bibliography` is inherited as a *layer*, not a fallback: a page's own `bibliography:`
/// is merged on top of it, so a post can cite a shared key and still add or correct entries
/// locally (`cite::Bibliography::overlay`).
///
/// It is the only member since the book-wide `theorems:` policy was retired on 2026-08-02
/// (`shared:` is a per-chapter statement and needs no project-level fallback). The struct
/// stays rather than collapsing to a bare `Vec<PathBuf>` for the reason above: the next
/// project-wide key is then one field, not six widened signatures.
#[derive(Debug, Clone, Default)]
pub struct SiteDefaults {
    /// Readable absolute paths to the project-wide `.bib` file(s), already resolved
    /// against the site root by `Site::discover` — so the render pass neither re-derives
    /// "relative to what?" nor repeats a bad-path diagnostic once per page.
    pub bibliography: Vec<std::path::PathBuf>,
}

impl PageIncludes {
    /// Append `other` after `self` (site-level first, then the page's own).
    pub fn merge(&mut self, other: &PageIncludes) {
        for (dst, src) in [
            (&mut self.in_header, &other.in_header),
            (&mut self.before_body, &other.before_body),
            (&mut self.after_body, &other.after_body),
        ] {
            if !src.is_empty() {
                dst.push_str(src);
            }
        }
    }
}

impl RenderedDoc {
    /// Concatenated block HTML, one block per line.
    pub fn body_html(&self) -> String {
        let mut s = String::new();
        for b in &self.blocks {
            s.push_str(&b.html);
            s.push('\n');
        }
        s
    }

    /// A deterministic, screen-reader-like plain-text projection of the document (the
    /// `taliesin read` view): headings, resolved "Figure N"/xref numbers, callout kinds,
    /// fenced code, display math as raw TeX. A VIEW, not an output format. See
    /// [`super::text`].
    pub fn body_text(&self) -> String {
        super::text::project(&self.blocks)
    }

    /// Like [`body_text`], but appends the server's headless `{js}` observation line
    /// (DX17b) after each matching `{js}` cell. `js_lines` maps a `{js}` cell's block id to
    /// its preformatted `[js: …]` line (the server computes it — observing a browser-run
    /// cell needs Chrome); core only interleaves it. An empty map yields exactly
    /// [`body_text`].
    pub fn body_text_with_js(
        &self,
        js_lines: &std::collections::HashMap<String, String>,
    ) -> String {
        super::text::project_with_js(&self.blocks, js_lines)
    }
}

#[cfg(test)]
mod warning_tests {
    use super::Warning;

    #[test]
    fn warning_new_is_unlocated_and_displays_its_message() {
        let w = Warning::new("broken citation: @x");
        assert_eq!(w.message, "broken citation: @x");
        assert_eq!(w.file, None);
        assert_eq!(w.line, None);
        assert_eq!(w.to_string(), "broken citation: @x");
    }

    #[test]
    fn warning_at_attaches_file_and_line() {
        let w = Warning::new("broken cross-reference: @fig-x").at(Some("intro.tmd".into()), 12);
        assert_eq!(w.file.as_deref(), Some("intro.tmd"));
        assert_eq!(w.line, Some(12));
    }
}
