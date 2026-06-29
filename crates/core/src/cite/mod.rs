//! Pragmatic citations and cross-references.
//!
//! **What:** citations (`[@key]`, `[@key, locator]`, `[@a; @b]`) become numbered
//! links to an auto-generated References section, formatted from a parsed BibTeX
//! file; cross-references (`@fig-x`, `@sec-x`, …) become anchor links labelled by
//! kind and, when the number is known, carrying it ("Figure 3"). Not a full CSL
//! engine — numbering for *computed* figures arrives with execution.
//!
//! **How to use:** [`parse_bib`]/[`parse_bib_warned`] build a [`Bibliography`];
//! [`process`] rewrites a doc's blocks (and appends the References block);
//! [`validate_xrefs`] flags unresolved cross-references.
//!
//! **Modules (one responsibility each):** `parse` (BibTeX), `format` (IEEE
//! formatting), `clean` (LaTeX/accent cleaning), `author` (name lists), `render`
//! (citation/xref HTML processing), `validate` (xref checking). Processing runs over
//! already-rendered block HTML, transforming only plain-text runs (never tags, code,
//! or math), so block sourcepos is untouched; the only structural change is the
//! appended References block.
//!
//! **Depends on:** [`crate::render`] for the block model + `Warning`/`escape_attr`.

use std::collections::HashMap;

mod author;
mod clean;
mod format;
mod parse;
mod render;
mod validate;

#[cfg(test)]
mod tests;

pub use parse::{parse_bib, parse_bib_warned};
pub use render::process;
pub use validate::validate_xrefs;

/// A parsed BibTeX database.
#[derive(Default)]
pub struct Bibliography {
    entries: HashMap<String, Entry>,
}

/// One parsed BibTeX entry: its `@type` (lowercased, e.g. `article`/`book`/
/// `misc`) plus field values. The type drives IEEE per-type formatting.
#[derive(Default)]
struct Entry {
    kind: String,
    fields: HashMap<String, String>,
}

type Fields = HashMap<String, String>;

impl Bibliography {
    /// Whether no entries were parsed (no `bibliography:` set, or an empty file).
    /// Used to suppress "broken citation" warnings when there's no bibliography at
    /// all (the missing-file case is reported separately).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Parse the 1-based start line out of a `startLine:col-endLine:col` sourcepos.
/// Returns `None` for a generated block (empty sourcepos) or a malformed value.
/// Shared by `render::process` and `validate::validate_xrefs`.
pub(crate) fn sourcepos_start_line(sourcepos: &str) -> Option<u32> {
    sourcepos
        .split(':')
        .next()?
        .parse::<u32>()
        .ok()
        .filter(|&l| l > 0)
}
