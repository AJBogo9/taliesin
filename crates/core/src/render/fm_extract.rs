//! The renderer's read of a document's front matter: ONE serde_yaml parse of the block
//! [`crate::frontmatter::front_matter_block`] splits off, and typed accessors over it.
//!
//! This used to be a family of line scans (`extract_field`, `detect_toc`,
//! `detect_title_block_hidden`, `detect_execute_cache`) running beside the YAML parse the
//! site layer and `author:` already used. The two readers disagreed on ordinary YAML: the
//! scan trimmed every quote character from both ends and knew nothing of comments, escapes,
//! block scalars or a value wrapped onto a second line, so one page published `It''s here`
//! in its `<h1>` and `It's here` in og:title, `toc: false  # no rail` showed the TOC and
//! `execute: cache: false  # live data` kept the cache on, all under a green `--strict`.
//! A front matter that is not valid YAML reads as empty here: `frontmatter::yaml_error`
//! reports it, located, and the build fails on it.

use crate::frontmatter::{front_matter_value, parse_front_matter_block, value_bool};

/// A document's parsed front matter (YAML `null` when it has none, or none that parses).
#[derive(Default)]
pub(crate) struct DocFront(serde_yaml::Value);

impl DocFront {
    /// The front matter of the document `src`.
    pub(super) fn of(src: &str) -> Self {
        Self(front_matter_value(src).unwrap_or_default())
    }

    /// A top-level key's raw value, for the readers that take a YAML value (`author:`).
    pub(super) fn get(&self, key: &str) -> Option<&serde_yaml::Value> {
        self.0.get(key)
    }

    /// A top-level scalar as display text: a string as the YAML parser decoded it, a number
    /// or bool in its YAML spelling (`crate::site::scalar`, the site layer's own reader, so
    /// og:title and the `<h1>` cannot disagree). `None` when absent, null, blank or not a
    /// scalar.
    pub(super) fn text(&self, key: &str) -> Option<String> {
        crate::site::scalar(self.get(key)).filter(|s| !s.trim().is_empty())
    }

    /// The top-level `toc:` setting as a tri-state: `Some` when the page sets it, `None`
    /// when absent. `Option` lets a site tell an explicit `toc: false` (which overrides the
    /// site default) from an unset toc (which inherits it). Catches the YAML-1.1 words serde
    /// reads as strings (`toc: yes`), so they take effect instead of silently no-oping.
    pub(super) fn toc(&self) -> Option<bool> {
        value_bool(self.get("toc")?)
    }

    /// `title-block-style: none` suppresses the visible title-block header while keeping
    /// the `title` metadata. Used by nav landing pages (Blog/Projects/Publications) where a
    /// big `<h1>` repeats the navbar.
    pub(super) fn title_block_hidden(&self) -> bool {
        self.text("title-block-style").as_deref() == Some("none")
    }

    /// Whether a render of this document emits a visible title block, and therefore
    /// demotes every body heading one level so the page keeps a single `<h1>`.
    pub(super) fn emits_title_block(&self) -> bool {
        !self.title_block_hidden() && self.text("title").is_some()
    }

    /// The document-level `execute: cache:` default (`true` unless a recognized false
    /// word); a cell's own `#| cache:` overrides it.
    ///
    /// `echo:` and `include:` used to live here too and were retired on 2026-08-02. They
    /// were document-wide defaults for something every real document states per cell
    /// (`#| echo:`), and a default that silently suppresses every listing in a file reads
    /// worse than saying it on the cells you mean. `cache:` stays because it is genuinely a
    /// whole-document property: it is about the freeze cache, not about how any one cell
    /// reads.
    pub(super) fn exec_cache(&self) -> bool {
        self.get("execute")
            .and_then(|e| e.get("cache"))
            .and_then(value_bool)
            != Some(false)
    }

    /// The `bibliography:` value as a list of paths: a scalar (a quoted path with spaces
    /// included) or a sequence.
    pub(super) fn bibliography(&self) -> Vec<String> {
        match self.get("bibliography") {
            Some(serde_yaml::Value::Sequence(seq)) => seq
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            Some(serde_yaml::Value::String(s)) => vec![s.clone()],
            _ => Vec::new(),
        }
    }
}

/// [`DocFront::emits_title_block`] for a front-matter BLOCK (fences already split off).
///
/// `pub(crate)` because the site's *source-side* section numbering (`site/xref.rs`) has to
/// answer the same question without rendering: a demoted chapter numbers its sections from
/// one level deeper, so a scan that guessed differently would resolve `@sec-x` to a number
/// the heading does not show.
pub(crate) fn emits_title_block(front_matter: &str) -> bool {
    DocFront(parse_front_matter_block(front_matter).unwrap_or_default()).emits_title_block()
}

/// [`DocFront::bibliography`] for a front-matter BLOCK (fences already split off). Test-only:
/// the render path reads [`DocFront`] directly.
#[cfg(test)]
pub(crate) fn bibliography_paths(front_matter: &str) -> Vec<String> {
    DocFront(parse_front_matter_block(front_matter).unwrap_or_default()).bibliography()
}
