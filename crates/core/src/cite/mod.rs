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
pub use render::link_xrefs_in_fragment;
pub use render::process;

pub use validate::{validate_xrefs, validate_xrefs_known_elsewhere};

/// A parsed BibTeX database.
///
/// A page's bibliography can be **two layers**: a project-wide `bibliography:` from
/// `_site.yml` with the page's own `bibliography:` laid over it ([`Bibliography::overlay`]).
/// The layers exist so a page can override or extend a shared entry; they collapse into one
/// merged database the moment `overlay` returns, and nothing downstream can tell them apart.
///
/// A `local` set tracked which keys the page itself contributed, for the "declared but never
/// cited" lint. That lint was cut on 2026-08-20 and the set went with it — it was the one
/// thing that needed to distinguish the layers after the merge.
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

    /// Lay a page's own bibliography over `self` (a project-wide layer read from
    /// `_site.yml`): a key defined in both resolves to the **page's** entry, so a post can
    /// correct or extend a shared reference without editing the shared file.
    ///
    /// A collision across the two layers is deliberately silent — it is the documented way
    /// to override — unlike a duplicate key *within* one file, which stays a warning
    /// ([`parse_bib_warned`]).
    pub fn overlay(&mut self, page: Bibliography) {
        self.entries.extend(page.entries);
    }
}

/// The citation keys `src` cites, scanned from **source** text: every `@key` inside a
/// `[ … ]` span (the Pandoc bracketed-citation form), deduped in first-seen order.
/// Bracketed only, so a narrative `@fig-x` cross-reference or a `@decorator` in a code cell
/// is never mistaken for a citation; a key that is itself a cross-reference anchor is
/// excluded too.
///
/// A source scan, not a render, so a `[@key]` written inside a fenced code block counts as
/// a citation. That direction was deliberate for the lint that read this: over-counting
/// citations made "never cited" quieter rather than wrong-in-the-loud-direction. That lint
/// was cut on 2026-08-20 and this now has no caller outside its own test. Callers that need
/// the exact set (the reference list itself) get it from [`process`], over rendered HTML.
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
/// the name, so `fig-a` would "suggest" `fig-b`.
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

/// Parse the 1-based END line out of a `startLine:col-endLine:col` sourcepos, so a scan
/// that wants the block's text has its whole extent and not just where it starts.
pub(crate) fn sourcepos_end_line(sourcepos: &str) -> Option<u32> {
    sourcepos
        .split_once('-')?
        .1
        .split(':')
        .next()?
        .parse::<u32>()
        .ok()
        .filter(|&l| l > 0)
}

/// Where `token` (an `@anchor` or `@citekey`) sits in `src`, searched over the source lines
/// `[start, end]`, as `(line, col, end_col)` with 1-based Unicode-scalar columns.
///
/// **Why the xref and citation validators need this** (Fable audit FA30). Both recover
/// their subject from the RENDERED HTML, after the source is gone, so all they can say is
/// which block it was in: they filed a whole-line warning and the editor drew a squiggle
/// across the whole line rather than under `@fig-reslts`. The plumbing for a precise range
/// existed end to end and was used by the front-matter linter only. The compounding cost is
/// the one-click fix: `lint.rs` attaches the "Change to `@fig-results`" payload ONLY when a
/// suggestion has a precise span, so the did-you-mean these messages already compute could
/// never become a code action.
///
/// Returns `None` when the token is not in that range, and the caller keeps its whole-line
/// location. That is the honest fallback: an id can be *produced* rather than written (a
/// generated block, a ref inside an included file the caller does not hold), and a guessed
/// span would be a wrong fix an agent applies mechanically.
///
/// The match must be delimited, or `@fig-a` would be located inside `@fig-abc`, and the
/// delimiter is the caller's `boundary`: the two vocabularies genuinely differ. An anchor
/// runs on `[A-Za-z0-9_-]` (`parse_xref`), so the `.` ending "see @fig-reslts." is a
/// sentence period; a cite key also takes `.`, `:` and `/` (`is_cite_key_char`). Reusing one
/// predicate for both put the sentence period inside the anchor and found nothing.
pub(crate) fn token_span(
    src: &str,
    start: u32,
    end: u32,
    token: &str,
    boundary: fn(char) -> bool,
) -> Option<(u32, u32, u32)> {
    for (idx, line) in src.lines().enumerate() {
        let no = idx as u32 + 1;
        if no < start || no > end {
            continue;
        }
        let mut from = 0usize;
        while let Some(rel) = line[from..].find(token) {
            let at = from + rel;
            from = at + token.len();
            // A following id character means this is a longer name that merely starts the
            // same way. A preceding one means the `@` is not the start of a reference.
            let after_ok = line[at + token.len()..]
                .chars()
                .next()
                .is_none_or(|c| !boundary(c));
            // The renderer only reads an `@` that starts a word, so `bob@rem-server.com` is
            // an address and not a `rem-` anchor. Same rule here, or the span would point at
            // a token the renderer never treated as a reference.
            let before_ok = line[..at]
                .chars()
                .next_back()
                .is_none_or(|c| !(c.is_alphanumeric() || boundary(c) || c == '@'));
            if after_ok && before_ok {
                let col = line[..at].chars().count() as u32 + 1;
                return Some((no, col, col + token.chars().count() as u32));
            }
        }
    }
    None
}
