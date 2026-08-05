//! Citation + cross-reference HTML processing: walk block HTML, rewrite `[@key]`
//! citation groups and `@fig-`/`@sec-`/… cross-references, and append a References
//! section. Transforms only plain-text runs (never tags/code/math), so block
//! sourcepos is untouched.

use super::{Bibliography, sourcepos_start_line};
use crate::render::{Block, Warning, escape_attr as esc};
use std::collections::HashMap;

/// Cross-reference kind prefixes -> display label, in canonical order. The single source
/// of truth for both `xref_label` (the lookup) and the editor `vocab` dump, so the two
/// cannot drift. The parallel bare-prefix list in `site::xref::is_ref_anchor` is guarded
/// against this one by a unit test there.
///
/// `prp` (Proposition), `exm` (Example) and `rem` (Remark) OUTLIVE the theorem kinds
/// that retired on 2026-08-03 (visual minimalism pass, task 14; see
/// `render::validate::THEOREM_KINDS`) **on purpose — do not "tidy" them away.** The
/// first cut deleted all three tuples here, and that was wrong: with the prefix gone,
/// `parse_xref` never recognizes `@exm-x` as an xref at all, so it degrades silently to
/// literal text — no link, no `TAL-XREF-UNDEF`, nothing. Keeping the prefix (with no
/// div class left to ever satisfy it) means every `@exm-x`/`@prp-x`/`@rem-x` is
/// necessarily now a *dangling* reference, which `validate_xrefs` already reports as
/// the ordinary "broken cross-reference" error — the exact right message, for free,
/// through the existing unresolved-anchor path, no new register needed. This is also
/// why `vocab()`'s `xrefPrefixes` (12 entries: 5 float-ish + the 7 that were ever
/// theorem-shaped — `thm`/`lem`/`cor`/`def` plus the 3 kept here) and `theoremKinds`
/// (5 entries) now disagree: `prp`/`exm`/`rem` are cross-reference prefixes with no
/// matching theorem kind any more. That asymmetry is intentional, not drift.
pub(crate) const XREF_LABELS: &[(&str, &str)] = &[
    ("fig", "Figure"),
    ("tbl", "Table"),
    ("sec", "Section"),
    ("eq", "Equation"),
    ("lst", "Listing"),
    ("thm", "Theorem"),
    ("lem", "Lemma"),
    ("cor", "Corollary"),
    ("prp", "Proposition"),
    ("def", "Definition"),
    ("exm", "Example"),
    ("rem", "Remark"),
];

/// Cross-reference kind prefixes -> display label.
fn xref_label(prefix: &str) -> Option<&'static str> {
    XREF_LABELS
        .iter()
        .find(|(p, _)| *p == prefix)
        .map(|(_, l)| *l)
}

/// The cross-reference prefix whose label is `label` (the inverse of [`xref_label`]),
/// e.g. "Theorem" -> "thm". Shares [`XREF_LABELS`] so a theorem kind's suggested
/// prefix can't drift from the label lookup. `None` for a label with no xref kind.
pub(crate) fn xref_prefix_for_label(label: &str) -> Option<&'static str> {
    XREF_LABELS
        .iter()
        .find(|(_, l)| *l == label)
        .map(|(p, _)| *p)
}

/// Whether `id` is a cross-reference anchor (`sec-…`, `fig-…`, …) that `@ref`
/// resolves — its prefix before the first `-` is a known xref kind. Shares
/// [`XREF_LABELS`] so it can't drift from the label lookup.
pub fn is_xref_anchor(id: &str) -> bool {
    id.split_once('-')
        .is_some_and(|(prefix, _)| xref_label(prefix).is_some())
}

/// Resolve citations + cross-references across `blocks`, appending a References
/// block when citations were found and the bibliography could format them.
/// `xrefs` maps a cross-reference anchor (e.g. `fig-scree`) to its resolved
/// number, so `@fig-scree` renders as a linked "Figure 3".
/// Returns one warning per citation key not in the (non-empty) bibliography, for
/// the dev server's diagnostics. Empty when every citation resolves (or there's no
/// bibliography at all, in which case the missing-file case is reported elsewhere).
///
/// Also reports the **mirror** of that check — an entry the page declared and never cited —
/// which belongs here because this is the one place that holds both the cited set and the
/// bibliography. `bib_line` locates both families on the front-matter `bibliography:` line
/// (a `.bib` entry has no position in the `.tmd`), matching `render::load_bibliography`.
pub fn process(
    blocks: &mut Vec<Block>,
    bib: &Bibliography,
    xrefs: &HashMap<String, String>,
    bib_line: Option<u32>,
) -> Vec<Warning> {
    let mut order: Vec<String> = Vec::new();
    let mut number: HashMap<String, usize> = HashMap::new();
    // Track the block location where each cite key is first seen, for located warnings.
    // (file, line) pair per key; the RefCell lets the closure capture it alongside order/number.
    type KeyLocMap = HashMap<String, (Option<String>, Option<u32>)>;
    let key_loc: std::cell::RefCell<KeyLocMap> = std::cell::RefCell::new(HashMap::new());
    let cur_loc: std::cell::RefCell<(Option<String>, Option<u32>)> =
        std::cell::RefCell::new((None, None));
    let mut cite_key = |key: &str| -> usize {
        let n = *number.entry(key.to_string()).or_insert_with(|| {
            order.push(key.to_string());
            order.len()
        });
        // Record the block location the first time this key appears.
        key_loc
            .borrow_mut()
            .entry(key.to_string())
            .or_insert_with(|| cur_loc.borrow().clone());
        n
    };

    for b in blocks.iter_mut() {
        *cur_loc.borrow_mut() = (b.source_file.clone(), sourcepos_start_line(&b.sourcepos));
        b.html = transform_html(&b.html, &mut cite_key, xrefs, CiteMode::Resolve);
    }
    let key_loc = key_loc.into_inner();

    // The dead-weight lint, reported whether or not the page cites anything: a page that
    // declares a `bibliography:` and cites none of it is the loudest instance, not an
    // exemption. One warning for the whole set rather than one per key — every one of them
    // would point at the same `bibliography:` line, so N warnings would be N copies of one
    // click-to-source target.
    let mut warnings: Vec<Warning> = Vec::new();
    let uncited = bib.uncited_local(&order);
    if !uncited.is_empty() {
        let w = Warning::new(super::uncited_message(&uncited));
        warnings.push(match bib_line {
            Some(l) => w.at(None, l),
            None => w,
        });
    }

    if order.is_empty() {
        return warnings;
    }
    // If the author already wrote a `# References` / `# Bibliography` heading, render
    // the reference list under it instead of emitting a second "References" heading,
    // and, since that heading is where they asked for the list, put the list THERE
    // rather than at the end of the document (see the insert below). The FIRST such
    // heading wins: a second one is a duplicate the author can see and fix, and
    // silently preferring a later one would be no more principled.
    let manual_heading = blocks
        .iter()
        .position(|b| is_manual_references_heading(&b.html));
    let mut list =
        String::from("<section class=\"tali-references\" data-block-id=\"tali-references\">");
    if manual_heading.is_none() {
        list.push_str("<h2>References</h2>");
    }
    for (idx, key) in order.iter().enumerate() {
        let formatted = match bib.format(key) {
            Some(f) => f,
            None => {
                // A cited key with no entry is a broken citation — but only flag it
                // when a bibliography exists (else every cite would warn before one
                // is set up; the missing-file case is its own warning).
                if !bib.is_empty() {
                    let (file, line) = key_loc.get(key).cloned().unwrap_or((None, None));
                    // A near-miss key is the commonest way an author breaks a citation:
                    // point at the entry they meant instead of only naming the one they
                    // typed. The bibliography is in scope here, so no plumbing is needed.
                    let w = Warning::new(match super::nearest(key, bib.keys()) {
                        Some(near) => format!("broken citation: @{key} (did you mean `@{near}`?)"),
                        None => format!("broken citation: @{key} (not in the bibliography)"),
                    });
                    warnings.push(match line {
                        Some(l) => w.at(file, l),
                        None => w,
                    });
                }
                format!("<code>{}</code>", esc(key))
            }
        };
        list.push_str(&format!(
            "<div id=\"ref-{}\" class=\"csl-entry\">[{}] {}</div>",
            esc(key),
            idx + 1,
            formatted
        ));
    }
    list.push_str("</section>");
    // A GATHERED block: it lists entries cited from lines scattered all over the
    // document, so it has no single honest source range. The empty sourcepos is that
    // claim, and client.js `locatable()` reads it as "not click-to-source" rather than
    // guessing. Do NOT fill it in to make the list clickable: an entry's real position
    // is its `.bib` record, not the `.tmd`, and the `[@key]` citation site is one of
    // several, so any value here would be a guess wearing navigation's clothes.
    let block = Block {
        id: "tali-references".to_string(),
        sourcepos: String::new(),
        source_file: None,
        html: list,
        cell: None,
    };
    // Land the list under the author's own `# References` heading when they wrote one.
    // Appending unconditionally was right by luck while every document ended with that
    // heading, and wrong the moment one kept going: an appendix after `# References`
    // pushed the list past it, orphaning the heading and filing the references under
    // the appendix. With no manual heading there is nothing to anchor to, so the
    // implicit "at the end, under an auto <h2>" placement stands unchanged.
    match manual_heading {
        Some(i) => blocks.insert(i + 1, block),
        None => blocks.push(block),
    }
    warnings
}

/// Whether a block is a manual heading (`<h1>`…`<h6>`) whose visible text is exactly
/// "References" or "Bibliography" (case-insensitive). Such a heading means the author
/// is placing the reference list themselves, so the auto section drops its own
/// `<h2>References</h2>` to avoid a duplicate heading. Matches only a heading block
/// (not, say, a paragraph that merely mentions "references").
fn is_manual_references_heading(html: &str) -> bool {
    let t = html.trim_start();
    // Must open with an <h1>..<h6> tag.
    let bytes = t.as_bytes();
    if bytes.len() < 4
        || bytes[0] != b'<'
        || (bytes[1] | 0x20) != b'h'
        || !bytes[2].is_ascii_digit()
    {
        return false;
    }
    // Strip every tag, leaving the text content; then compare case-insensitively.
    let mut text = String::new();
    let mut in_tag = false;
    for c in t.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = text.trim().to_ascii_lowercase();
    text == "references" || text == "bibliography"
}

/// Link the bare `@fig-…`/`@tbl-…`/`@sec-…` cross-references in an HTML fragment that
/// is produced *after* [`process`] has already walked the document's blocks — an
/// executed cell's caption, which the server builds only once the kernel returns.
/// Unknown anchors (all of them here, since no local registry is passed) become
/// `data-tali-xref` markers, which the site's `resolve_cross_refs` then turns into the
/// numbered link, so a caption reference reads exactly like the same reference in prose.
///
/// Citations are deliberately left alone: numbering one now would have to append to a
/// References list that was built before this fragment existed, so `[@key]` in a caption
/// stays literal rather than silently claiming a number nothing lists.
pub fn link_xrefs_in_fragment(html: &str) -> String {
    let empty = HashMap::new();
    transform_html(html, &mut |_| 0, &empty, CiteMode::Skip)
}

/// Whether [`transform_html`] may rewrite `[@key]` citation groups. A fragment
/// transformed after the References block exists must not (see
/// [`link_xrefs_in_fragment`]).
#[derive(Clone, Copy, PartialEq)]
enum CiteMode {
    Resolve,
    Skip,
}

/// Walk HTML, transforming only plain-text runs (never inside tags or inside
/// `pre`/`code`/`script`/`style`/`annotation` elements).
fn transform_html(
    html: &str,
    cite_key: &mut impl FnMut(&str) -> usize,
    xrefs: &HashMap<String, String>,
    cites: CiteMode,
) -> String {
    const SKIP: [&str; 5] = ["pre", "code", "script", "style", "annotation"];
    let mut out = String::with_capacity(html.len());
    let mut skip_depth = 0usize;
    let mut rest = html;
    while !rest.is_empty() {
        if rest.starts_with('<') {
            let end = rest.find('>').map(|e| e + 1).unwrap_or(rest.len());
            let tag = &rest[..end];
            let name: String = tag
                .trim_start_matches(['<', '/'])
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            if SKIP.contains(&name.as_str()) {
                if tag.starts_with("</") {
                    skip_depth = skip_depth.saturating_sub(1);
                } else if !tag.ends_with("/>") {
                    skip_depth += 1;
                }
            }
            out.push_str(tag);
            rest = &rest[end..];
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let text = &rest[..end];
            if skip_depth == 0 {
                out.push_str(&rewrite_text(text, cite_key, xrefs, cites));
            } else {
                out.push_str(text);
            }
            rest = &rest[end..];
        }
    }
    out
}

/// Rewrite citations and cross-references within a plain-text run.
fn rewrite_text(
    text: &str,
    cite_key: &mut impl FnMut(&str) -> usize,
    xrefs: &HashMap<String, String>,
    cites: CiteMode,
) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    // Once a scan from some `[` finds no `]` to its right, no later `[` can either
    // (the remaining text only shrinks), so stop re-scanning to the end. Without
    // this, a run of N unmatched `[` is O(N^2) (one full scan per `[`).
    let mut no_close = false;
    while i < chars.len() {
        if chars[i] == '[' && !no_close && cites == CiteMode::Resolve {
            match chars[i + 1..].iter().position(|&c| c == ']') {
                Some(close) => {
                    let inner = &chars[i + 1..i + 1 + close];
                    if inner.contains(&'@') {
                        let inner: String = inner.iter().collect();
                        out.push_str(&render_citation_group(&inner, cite_key, xrefs));
                        i += close + 2;
                        continue;
                    }
                }
                None => no_close = true,
            }
        } else if chars[i] == '@'
            && at_word_boundary(&chars, i)
            && let Some((label, anchor, len)) = parse_xref(&chars[i..])
        {
            // A locally-resolved number renders "Figure&nbsp;3". An anchor not in
            // this document's registry may live on another page: emit it with a
            // `data-tali-xref` marker so a site can resolve it to that page (and its
            // number); if nothing resolves it, it degrades to a bare-label link.
            out.push_str(&xref_anchor_link(&anchor, label, xrefs));
            i += len;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

use super::is_cite_key_char;

/// A cross-reference link to `anchor`, labelled by kind. A locally-resolved number
/// renders "Figure&nbsp;3"; an anchor unknown to this document's registry emits a
/// `data-tali-xref` marker (so a site can resolve it cross-page) and degrades to a
/// bare-label link. Shared by the bracketed (`[@fig-x]`) and bare (`@fig-x`) paths.
fn xref_anchor_link(anchor: &str, label: &str, xrefs: &HashMap<String, String>) -> String {
    let (text, marker) = match xrefs.get(anchor) {
        // A registered anchor with no number (an unnumbered theorem) resolves to a bare
        // label, not a broken-ref marker; a numbered one renders "Figure&nbsp;3".
        Some(n) if n.is_empty() => (label.to_string(), String::new()),
        Some(n) => (format!("{label}&nbsp;{n}"), String::new()),
        None => (
            label.to_string(),
            format!(" data-tali-xref=\"{}\"", esc(anchor)),
        ),
    };
    format!(
        "<a href=\"#{}\" class=\"tali-xref\"{marker}>{text}</a>",
        esc(anchor)
    )
}

/// If `key` is a cross-reference key (`fig-x`, `tbl-x`, …), render it as a cross-ref
/// link (so `[@fig-x]` is a cross-ref, not a citation). `None` for ordinary keys.
fn xref_link(key: &str, xrefs: &HashMap<String, String>) -> Option<String> {
    let (prefix, ident) = key.split_once('-')?;
    let label = xref_label(prefix)?;
    if ident.is_empty() {
        return None;
    }
    Some(xref_anchor_link(key, label, xrefs))
}

/// Whether a bare `@` at `chars[i]` begins a word (so `@fig-x` is an xref, but the
/// `@` in `bob@rem-server.com` or an `@handle` glued to a word/`.`/`-`/`_`/`/` is not).
/// The char immediately before must be start-of-string or whitespace/opening
/// punctuation — never alphanumeric or `.` `-` `_` `/` (which are token-internal, and
/// `-`/`_`/`/` are also valid cross-reference-anchor chars). Mirrors the bracketed
/// path, where the `[` already provides the boundary.
fn at_word_boundary(chars: &[char], i: usize) -> bool {
    match i.checked_sub(1).map(|p| chars[p]) {
        None => true,
        Some(c) => !(c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | '/')),
    }
}

/// `@fig-x` -> ("Figure", "fig-x", consumed_len).
fn parse_xref(chars: &[char]) -> Option<(&'static str, String, usize)> {
    // chars[0] == '@'
    let rest: String = chars[1..].iter().collect();
    let prefix: String = rest
        .chars()
        .take_while(|c| c.is_ascii_lowercase())
        .collect();
    let label = xref_label(&prefix)?;
    let after = &rest[prefix.len()..];
    if !after.starts_with('-') {
        return None;
    }
    let ident: String = after[1..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if ident.is_empty() {
        return None;
    }
    let anchor = format!("{prefix}-{ident}");
    let consumed = 1 + prefix.len() + 1 + ident.len();
    Some((label, anchor, consumed))
}

/// Render `@a; @b, p. 5` style citation group content into `[1, 2, p. 5]`. A
/// cross-reference key inside the brackets (`[@fig-x]`) renders as a cross-ref link,
/// not a citation.
fn render_citation_group(
    inner: &str,
    cite_key: &mut impl FnMut(&str) -> usize,
    xrefs: &HashMap<String, String>,
) -> String {
    let mut rendered: Vec<String> = Vec::new();
    for item in inner.split(';') {
        let item = item.trim().trim_start_matches('-'); // `-@key` suppresses author (n/a for numeric)
        let Some(at) = item.find('@') else { continue };
        let after = &item[at + 1..];
        let key: String = after.chars().take_while(|&c| is_cite_key_char(c)).collect();
        if key.is_empty() {
            continue;
        }
        // A cross-reference key (`fig-`, `tbl-`, …) is a cross-ref, not a citation.
        if let Some(link) = xref_link(&key, xrefs) {
            rendered.push(link);
            continue;
        }
        let locator = after[key.len()..].trim().trim_start_matches(',').trim();
        let n = cite_key(&key);
        let mut piece = format!("<a href=\"#ref-{}\">{}</a>", esc(&key), n);
        if !locator.is_empty() {
            piece.push_str(&format!(", {}", esc(locator)));
        }
        rendered.push(piece);
    }
    if rendered.is_empty() {
        format!("[{}]", esc(inner))
    } else {
        format!("[{}]", rendered.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_text_leaves_unmatched_and_non_citation_brackets_literal() {
        let xrefs = HashMap::new();
        let mut key = |_: &str| 1usize;
        // A run of '[' with no closing ']' is emitted verbatim (this is also the
        // O(n^2)-pathological input the scan must not choke on).
        assert_eq!(
            rewrite_text("[[[[ no close here", &mut key, &xrefs, CiteMode::Resolve),
            "[[[[ no close here"
        );
        // A bracket group without '@' is not a citation; the brackets stay.
        assert_eq!(
            rewrite_text("see [ref 12] here", &mut key, &xrefs, CiteMode::Resolve),
            "see [ref 12] here"
        );
        // A real citation is still rewritten.
        let out = rewrite_text(
            "see [@bishop2006pattern]",
            &mut key,
            &xrefs,
            CiteMode::Resolve,
        );
        assert!(
            out.contains("<a") && !out.contains("[@"),
            "citation not rewritten: {out}"
        );
    }

    #[test]
    fn bare_xref_requires_a_word_boundary_before_at() {
        let mut xrefs = HashMap::new();
        xrefs.insert("fig-x".to_string(), "3".to_string());
        let mut key = |_: &str| 1usize;

        // A mid-word `@` (an email / @-mention glued to a word) is NOT an xref: the
        // `rem-` after the `@` looks like a `rem-` (Remark) anchor, but the preceding
        // `b` of `bob` is a word char, so it's left verbatim — no link, no diagnostic.
        let out = rewrite_text(
            "mail bob@rem-server.com today",
            &mut key,
            &xrefs,
            CiteMode::Resolve,
        );
        assert_eq!(out, "mail bob@rem-server.com today");

        // The same anchor still resolves when `@` starts a word.
        let out = rewrite_text("see @fig-x for this", &mut key, &xrefs, CiteMode::Resolve);
        assert!(
            out.contains("href=\"#fig-x\"") && out.contains("Figure"),
            "@fig-x at a word boundary must still resolve: {out}"
        );

        // Boundary forms that must keep working: start-of-string, after `(`, and a
        // trailing `.` after the anchor.
        let after_paren = rewrite_text("(@fig-x)", &mut key, &xrefs, CiteMode::Resolve);
        assert!(after_paren.contains("href=\"#fig-x\""), "{after_paren}");
        let at_start = rewrite_text("@fig-x.", &mut key, &xrefs, CiteMode::Resolve);
        assert!(
            at_start.contains("href=\"#fig-x\""),
            "start-of-string @fig-x must resolve: {at_start}"
        );
    }
}
