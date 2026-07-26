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
pub(crate) use render::XREF_LABELS;
/// Whether an id's prefix names a cross-reference kind, i.e. whether `@id` can resolve.
/// Public so `taliesin symbols` can offer only anchors an author can actually write
/// after `@`, instead of reimplementing the prefix list outside `taliesin-core`.
pub use render::is_xref_anchor;
pub use render::process;
pub(crate) use render::xref_prefix_for_label;
pub use validate::{validate_xrefs, validate_xrefs_known_elsewhere};

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

    /// Every parsed entry key, for the broken-citation did-you-mean.
    pub(crate) fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

/// Largest edit distance at which a name is a plausible typo rather than a different
/// name. Matches the front-matter did-you-mean ceiling (`frontmatter::closest`).
const MAX_TYPO_DISTANCE: usize = 2;

/// Shortest name worth fuzzy-matching. Below this a distance-2 edit rewrites most of
/// the name, so `fig-a` would "suggest" `fig-b`. Same value, and same reason, as
/// `site::categories::MIN_FUZZY_LEN`.
const MIN_FUZZY_LEN: usize = 5;

/// The candidate nearest to `name` within [`MAX_TYPO_DISTANCE`], or `None` when the
/// name is short, every candidate is short, or none is near enough.
///
/// `frontmatter::closest` cannot serve here: it takes `&[&'static str]`, and both
/// vocabularies (bib keys, a page's anchors) are owned `String`s read at run time.
/// Ties break lexicographically so the suggestion is deterministic — the bib keys
/// arrive from a `HashMap`, whose iteration order is not.
pub(crate) fn nearest<'a>(
    name: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Option<&'a str> {
    if name.chars().count() < MIN_FUZZY_LEN {
        return None;
    }
    candidates
        .filter(|c| c.chars().count() >= MIN_FUZZY_LEN)
        .map(|c| (crate::frontmatter::levenshtein(name, c), c))
        .filter(|&(d, _)| d > 0 && d <= MAX_TYPO_DISTANCE)
        .min_by_key(|&(d, c)| (d, c))
        .map(|(_, c)| c)
}

/// Characters allowed in a citation key, the single source of truth shared by the
/// BibTeX entry-key parser (`parse`) and the in-prose reference scanner (`render`).
/// BibTeX keys permit far more than alphanumerics (`smith.2020`, `doe+roe`,
/// `path/key`, `smith:2020a`); if the two sides disagree, either `[@smith.2020]`
/// truncates to `smith` and falsely warns "broken citation", or the bib stores a key
/// the reference can never name. Keeping ONE predicate makes them agree by construction.
pub(crate) fn is_cite_key_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '_' | ':' | '.' | '+' | '/')
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
