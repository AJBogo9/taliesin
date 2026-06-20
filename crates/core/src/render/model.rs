//! The render block model: the data types the parser emits and the dev server
//! consumes (cells, blocks, the rendered doc, page includes). Split out of
//! mod.rs so the data model is separate from the render pipeline + emission.

use std::path::PathBuf;

/// An executable Quarto code cell (```` ```{lang} ````), exposed so the dev
/// server can run it against a kernel.
#[derive(Debug, Clone)]
pub struct Cell {
    pub lang: String,
    pub code: String,
    /// When this cell's output should be a numbered `<figure>` (`#| label: fig-x`
    /// + `#| fig-cap:`), the executor wraps the rendered output accordingly.
    pub figure: Option<CellFigure>,
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
    /// relative to where qmd-fast runs (normally the document's directory). The
    /// export itself is performed by the Python kernel preamble at display time.
    pub fig_export: Option<String>,
}

/// Metadata for wrapping a code cell's executed output as a numbered figure.
#[derive(Debug, Clone)]
pub struct CellFigure {
    /// `#fig-…` anchor (when labelled), so `@fig-x` cross-references resolve.
    pub anchor: Option<String>,
    pub caption: Option<String>,
    pub number: usize,
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
    /// A reveal.js slide deck (`format: revealjs` / `*-revealjs`).
    Reveal,
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
    /// Whether a custom theme owns this doc's colours — a `theme:` CSS/extension,
    /// or a `format: <ext>-revealjs` / `extensions:` that contributes markup. Decks
    /// use this to skip the built-in light/dark management when an extension theme
    /// (e.g. liquid-glass) is in charge.
    pub theme_is_custom: bool,
    /// Resolved `include-in-header`/`include-before-body`/`include-after-body` +
    /// `css` from the doc's front matter, injected into the page template.
    pub includes: PageIncludes,
    /// Non-fatal render warnings (a missing `bibliography:`/`theme:` file, …): the
    /// core can't return a `Result`, so it reports these for the server to log +
    /// surface in the dev menu. Front-matter typo warnings are separate (the
    /// `frontmatter` linter runs off the source).
    pub warnings: Vec<String>,
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
    /// Files a format extension contributes via `format-resources` (e.g. a reveal
    /// plugin's `.js`). Absolute source paths; the build copies each next to the
    /// output page (by file name) so the deck's `<script src="...">` resolves, and
    /// the preview serves them from the `_extensions/` tree.
    pub resources: Vec<PathBuf>,
}

impl PageIncludes {
    /// Whether this contributes any head/body markup (ignores `resources`). Used to
    /// tell whether an extension contributed a theme/plugin to a deck.
    pub fn has_markup(&self) -> bool {
        !self.in_header.is_empty() || !self.before_body.is_empty() || !self.after_body.is_empty()
    }
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
        self.resources.extend(other.resources.iter().cloned());
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
