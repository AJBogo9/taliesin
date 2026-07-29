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
///
/// A page's bibliography can be **two layers**: a project-wide `bibliography:` from
/// `_site.yml` with the page's own `bibliography:` laid over it ([`Bibliography::overlay`]).
/// The layers matter to exactly one thing — the "declared but never cited" lint, which is
/// scoped to what the *page* declared, since a shared entry cited by some other page is not
/// an author mistake. Everything else (formatting, broken-citation checks) sees one merged
/// database and cannot tell the layers apart.
#[derive(Default)]
pub struct Bibliography {
    entries: HashMap<String, Entry>,
    /// Keys the page's own `bibliography:` contributed. Equal to `entries`' keys for a
    /// single-layer bibliography, which is every case that predates `_site.yml`'s key.
    local: std::collections::HashSet<String>,
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

    /// Lay a page's own bibliography over `self` (a project-wide layer read from
    /// `_site.yml`): a key defined in both resolves to the **page's** entry, so a post can
    /// correct or extend a shared reference without editing the shared file.
    ///
    /// A collision across the two layers is deliberately silent — it is the documented way
    /// to override — unlike a duplicate key *within* one file, which stays a warning
    /// ([`parse_bib_warned`]). After this, only `page`'s keys count as page-local.
    pub fn overlay(&mut self, page: Bibliography) {
        self.local = page.entries.keys().cloned().collect();
        self.entries.extend(page.entries);
    }

    /// The page's own declared keys that `cited` does not contain, sorted. Empty for a
    /// page whose `bibliography:` is fully cited, and — by construction — never naming an
    /// entry that came from the project-wide layer.
    pub(crate) fn uncited_local(&self, cited: &[String]) -> Vec<&str> {
        let mut out: Vec<&str> = self
            .local
            .iter()
            .map(String::as_str)
            .filter(|k| !cited.iter().any(|c| c == k))
            .collect();
        out.sort_unstable();
        out
    }

    /// Every key in the merged database that `cited` does not contain, sorted. The
    /// project-wide counterpart of [`uncited_local`](Self::uncited_local): its caller
    /// passes the keys cited by *every* page, so a shared entry is only unused when the
    /// whole site leaves it unused.
    pub fn uncited(&self, cited: &[String]) -> Vec<&str> {
        let mut out: Vec<&str> = self
            .entries
            .keys()
            .map(String::as_str)
            .filter(|k| !cited.iter().any(|c| c == k))
            .collect();
        out.sort_unstable();
        out
    }
}

/// How many uncited keys a diagnostic names before it summarizes the rest. A shared
/// `.bib` can leave dozens unused at once, and a diagnostic line that lists forty keys is
/// unreadable in an editor gutter; naming the first few is enough to start deleting.
const UNCITED_KEYS_SHOWN: usize = 5;

/// The "declared but never cited" message for `keys` (assumed non-empty and sorted). One
/// wording for both scopes — the page's own `bibliography:` and the project-wide one — so
/// [`crate::diagnostics::codes`] classifies them as one family and they cannot drift apart.
pub(crate) fn uncited_message(keys: &[&str]) -> String {
    let shown = keys
        .iter()
        .take(UNCITED_KEYS_SHOWN)
        .map(|k| format!("`@{k}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let rest = keys.len().saturating_sub(UNCITED_KEYS_SHOWN);
    let tail = if rest > 0 {
        format!(", and {rest} more")
    } else {
        String::new()
    };
    match keys.len() {
        1 => format!("bibliography entry {shown} is declared but never cited"),
        n => format!("{n} bibliography entries are declared but never cited: {shown}{tail}"),
    }
}

/// The citation keys `src` cites, scanned from **source** text: every `@key` inside a
/// `[ … ]` span (the Pandoc bracketed-citation form), deduped in first-seen order.
/// Bracketed only, so a narrative `@fig-x` cross-reference or a `@decorator` in a code cell
/// is never mistaken for a citation; a key that is itself a cross-reference anchor is
/// excluded too.
///
/// A source scan, not a render, so a `[@key]` written inside a fenced code block counts as
/// a citation. That direction is deliberate for the callers that lint on it: over-counting
/// citations makes "never cited" quieter, never wrong-in-the-loud-direction. Callers that
/// need the exact set (the reference list itself) get it from [`process`], which runs over
/// rendered HTML.
pub fn cited_keys_in_source(src: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'['
            && let Some(rel_close) = src[i + 1..].find(']')
        {
            let span = &src[i + 1..i + 1 + rel_close];
            let mut rest = span;
            while let Some(at) = rest.find('@') {
                let after = &rest[at + 1..];
                let end = after
                    .find(|c: char| !is_cite_key_char(c))
                    .unwrap_or(after.len());
                let key = after[..end].trim_end_matches(['.', ':', '-']);
                if !key.is_empty() && !is_xref_anchor(key) && !out.iter().any(|k| k == key) {
                    out.push(key.to_string());
                }
                rest = &after[end..];
            }
            i += 1 + rel_close + 1;
            continue;
        }
        i += 1;
    }
    out
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
