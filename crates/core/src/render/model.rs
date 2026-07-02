//! The render block model: the data types the parser emits and the dev server
//! consumes (cells, blocks, the rendered doc, page includes). Split out of
//! mod.rs so the data model is separate from the render pipeline + emission.

/// An executable Quarto code cell (```` ```{lang} ````), exposed so the dev
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
    /// `#| fig-export: figures/x.pdf` (comma-separated for several files) also writes
    /// the cell's figure to those files with print-clean styling (black-on-white,
    /// no web theming), for inclusion in a LaTeX/print document. Vector `.pdf`/`.svg`
    /// stay resolution-independent; `.png` is saved at a print DPI. Paths resolve
    /// relative to where taliesin runs (normally the document's directory). The
    /// export itself is performed by the Python kernel preamble at display time.
    pub fig_export: Option<String>,
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
}

/// Metadata for wrapping a code cell's executed output as a numbered figure.
#[derive(Debug, Clone)]
pub struct CellFigure {
    /// `#fig-…` anchor (when labelled), so `@fig-x` cross-references resolve.
    pub anchor: Option<String>,
    pub caption: Option<String>,
    pub number: usize,
}

/// Metadata for captioning/numbering a code cell's executed `<table>` output.
#[derive(Debug, Clone)]
pub struct CellTable {
    /// `#tbl-…` anchor (when labelled), so `@tbl-x` cross-references resolve.
    pub anchor: Option<String>,
    pub caption: Option<String>,
    /// Assigned in document order (alongside Markdown tables) so the "Table N" the
    /// caption shows matches what `@tbl-x` resolves to.
    pub number: u32,
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
    pub sourcepos: String,
    /// Origin file when the block came from an `{{< include >}}`d file
    /// (relative to the primary document's directory); `None` for the primary
    /// document. Drives click-to-source across files.
    pub source_file: Option<String>,
    /// Rendered HTML for this block, root element carrying the data attributes.
    pub html: String,
    /// Present when this block is an executable code cell.
    pub cell: Option<Cell>,
}

/// The output format the document targets, taken from its front matter
/// `format:` key. Drives which page scaffold (and live client) is emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocFormat {
    /// A standard HTML page (blog post, book): the default.
    #[default]
    Html,
    /// A slide deck, rendered by taliesin's OWN native engine (reveal.js was removed);
    /// selected by `format: revealjs` / `*-revealjs`.
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
}

impl Warning {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            file: None,
            line: None,
        }
    }

    /// Attach a click-to-source location.
    pub fn at(mut self, file: Option<String>, line: u32) -> Self {
        self.file = file;
        self.line = Some(line);
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
    /// Front-matter `lang:` (a BCP-47 tag like `en`/`fr`), emitted as `<html lang>`
    /// by the page builders. `None` falls back to `en`.
    pub lang: Option<String>,
    /// Front-matter `description`, used for the SEO/OpenGraph meta on a standalone
    /// page (site pages get richer per-page meta from their `Page`).
    pub description: Option<String>,
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
        let w = Warning::new("broken cross-reference: @fig-x").at(Some("intro.qmd".into()), 12);
        assert_eq!(w.file.as_deref(), Some("intro.qmd"));
        assert_eq!(w.line, Some(12));
    }
}
