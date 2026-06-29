//! Static accessibility checks (heading-level skips, unnamed interactives, alt-less images).

use super::helpers::{start_line, tag_attr};
use crate::render::{Block, DocFormat, Warning};

/// The heading level (1..=6) of a block whose HTML opens with `<h1>`..`<h6>`, else
/// `None`. Reads only the second byte of the tag (`<hN`), the same shape the heading-id
/// check keys off.
fn heading_level(html: &str) -> Option<u8> {
    if !html.starts_with("<h") {
        return None;
    }
    let d = html.as_bytes().get(2)?;
    if d.is_ascii_digit() && (b'1'..=b'6').contains(d) {
        Some(d - b'0')
    } else {
        None
    }
}

/// Whether the tag opened at the start of `tag` (everything before the first `>`)
/// carries attribute `attr` (e.g. `"alt"`), matched as a whole word so `alt` does
/// not false-match inside another attribute name/value. Accepts ` alt=`, ` alt>`,
/// or a bare boolean ` alt` at the tag end.
fn tag_has_attr(tag: &str, attr: &str) -> bool {
    let mut i = 0;
    while let Some(pos) = tag[i..].find(attr) {
        let at = i + pos;
        i = at + attr.len();
        // Must be preceded by whitespace (an attribute boundary, not a substring).
        let prev_ws = at == 0 || tag.as_bytes()[at - 1].is_ascii_whitespace();
        if !prev_ws {
            continue;
        }
        // Must be followed by `=`, whitespace, or the tag end (a real attribute, not a prefix).
        match tag.as_bytes().get(i) {
            None => return true,
            Some(c) if *c == b'=' || c.is_ascii_whitespace() || *c == b'/' || *c == b'>' => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// The visible text content of an HTML fragment, i.e. everything outside `<...>` tags
/// with runs of whitespace collapsed. Used to decide whether an interactive element has
/// a non-empty accessible name from its text alone.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for ch in html.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether the element spanning `inner` (the HTML between an interactive element's open
/// and close tag) carries an accessible name: non-empty text, or an `alt`-bearing
/// `<img>`, or a labelled `<svg>` (`role="img"`, `aria-label`, `<title>`). Mirrors the
/// `named` check in `scanA11y`. (`aria-label`/`title` on the element itself are checked
/// by the caller off the open tag.)
fn has_accessible_name(inner: &str) -> bool {
    if !strip_tags(inner).is_empty() {
        return true;
    }
    // An <img alt="non-empty"> descendant names the control.
    let mut i = 0;
    while let Some(pos) = inner[i..].find("<img") {
        let start = i + pos;
        let Some(end) = inner[start..].find('>') else {
            break;
        };
        let tag = &inner[start..start + end];
        i = start + end + 1;
        if let Some(alt) = tag_attr(tag, "alt=\"")
            && !alt.trim().is_empty()
        {
            return true;
        }
    }
    inner.contains("aria-label=") || inner.contains("<title") || inner.contains("role=\"img\"")
}

/// One interactive element to audit for an accessible name: the `<a>`/`<button>` open
/// tag (for `aria-label`/`title`) plus the inner HTML up to its close tag.
struct Interactive<'a> {
    /// `"link"` or `"button"`, for the message.
    kind: &'a str,
    /// The open tag's attributes (everything inside `<…>`).
    open: &'a str,
    /// The HTML between the open and matching close tag.
    inner: &'a str,
}

/// Every `<a href …>…</a>` and `<button …>…</button>` in `html`, returned as the open
/// tag + inner HTML so the caller can test for an accessible name. Nested same-type
/// elements are rare in content; the first close tag wins (a conservative scan, never a
/// false *positive*).
fn interactives(html: &str) -> Vec<Interactive<'_>> {
    let mut out = Vec::new();
    for (open_pat, close_pat, kind, require_href) in [
        ("<a ", "</a>", "link", true),
        ("<button", "</button>", "button", false),
    ] {
        let mut i = 0;
        while let Some(pos) = html[i..].find(open_pat) {
            let tag_start = i + pos;
            let Some(rel_end) = html[tag_start..].find('>') else {
                break;
            };
            let open_end = tag_start + rel_end; // index of '>'
            let open = &html[tag_start + open_pat.len()..open_end];
            i = open_end + 1;
            if require_href && !tag_has_attr(open, "href") {
                continue; // a named anchor (`<a id=…>` / `<a name=…>`) is not a link target
            }
            let Some(crel) = html[i..].find(close_pat) else {
                continue;
            };
            let inner = &html[i..i + crel];
            i += crel + close_pat.len();
            out.push(Interactive { kind, open, inner });
        }
    }
    out
}

/// Static accessibility checks ported from the live preview's `scanA11y`
/// (`web-client/client.js`) into the kernel-free `check` channel, so a green `check`
/// also vouches for the statically-knowable a11y subset. Read-only — reads only block
/// HTML + sourcepos. Three rules ship; document-`lang` (the page builders default it to
/// `en`, so a built page is never lang-less) and body-text contrast (needs *computed*
/// CSS, not a static block-model fact) are intentionally left to the live audit.
///
/// 1. **Heading-level skip** — a heading that jumps `>= 2` levels deeper than the
///    previous one (e.g. `<h2>` then `<h4>`). Conservative: only a *mid-document* skip
///    is flagged (never "doesn't start at h1"), and decks are skipped entirely
///    (slides are slide-structured, not a single outline).
/// 2. **Interactive element with no accessible name** — an `<a href>`/`<button>` whose
///    text is empty and which carries no `aria-label`/`title` and no labelled
///    `<img>`/`<svg>` descendant (e.g. an icon-only link).
/// 3. **`<img>` without `alt`** — a raw/passthrough `<img>` with no `alt` attribute at
///    all. (`![]()` markdown always emits an `alt`, so this catches hand-written
///    `<img>` only.)
pub fn validate_a11y(blocks: &[Block], format: DocFormat) -> Vec<Warning> {
    let mut out = Vec::new();

    // (1) Heading-level skips — skipped wholesale for decks.
    if format != DocFormat::Reveal {
        let mut prev = 0u8;
        for b in blocks {
            let Some(lvl) = heading_level(&b.html) else {
                continue;
            };
            if prev > 0 && lvl >= prev + 2 {
                let w = Warning::new(format!(
                    "heading level skips from h{prev} to h{lvl} (add an intervening heading, or demote this one)"
                ));
                out.push(match start_line(&b.sourcepos) {
                    Some(l) => w.at(b.source_file.clone(), l),
                    None => w,
                });
            }
            prev = lvl;
        }
    }

    for b in blocks {
        let line = start_line(&b.sourcepos);

        // (2) Interactive elements with no accessible name.
        for el in interactives(&b.html) {
            let named_on_tag = el.open.contains("aria-label=\"") || el.open.contains("title=\"");
            if named_on_tag || has_accessible_name(el.inner) {
                continue;
            }
            let w = Warning::new(format!(
                "{} has no accessible name (icon-only? add aria-label or visible text)",
                el.kind
            ));
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }

        // (3) Raw `<img>` with no `alt` attribute.
        let mut i = 0;
        while let Some(pos) = b.html[i..].find("<img") {
            let start = i + pos;
            let Some(end) = b.html[start..].find('>') else {
                break;
            };
            let tag = &b.html[start..start + end];
            i = start + end + 1;
            // `<img`-prefix guard: only a real tag (`<img ` / `<img>` / `<img/>`).
            let after = tag.as_bytes().get(4).copied();
            let is_img = matches!(after, None | Some(b' ') | Some(b'/') | Some(b'\t'));
            if is_img && !tag_has_attr(tag, "alt") {
                let w = Warning::new(
                    "image is missing alt text (add alt text, or alt=\"\" if decorative)",
                );
                out.push(match line {
                    Some(l) => w.at(b.source_file.clone(), l),
                    None => w,
                });
            }
        }
    }

    out
}
