//! Static accessibility checks (heading-level skips, unnamed interactives, alt-less images).

use super::helpers::{heading_level, start_line, strip_tags, tag_attr};
use crate::render::{Block, Warning};

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
///
/// `pub(super)` because `links.rs` scans the same elements for its link-text collision
/// rule; one nesting-aware scan, not two.
pub(super) struct Interactive<'a> {
    /// `"link"` or `"button"`, for the message.
    pub(super) kind: &'a str,
    /// The open tag's attributes (everything inside `<…>`).
    pub(super) open: &'a str,
    /// The HTML between the open and matching close tag.
    pub(super) inner: &'a str,
}

/// Every element carrying `role="button" | "link" | "tab"` that is NOT itself a native
/// `<a>`/`<button>` (those are already covered by [`interactives`]), returned as the
/// open tag + inner HTML so the caller can test for an accessible name. This catches a
/// `<div role="button">` / `<span role="link">` / `[role="tab"]` that is interactive to
/// assistive tech but invisible to the literal-tag scan. Conservative: the matching
/// close tag is found by tag name with simple nesting depth, and an element with any
/// accessible name passes — never a false *positive*.
fn role_interactives(html: &str) -> Vec<Interactive<'_>> {
    let mut out = Vec::new();
    for (role, kind) in [
        ("role=\"button\"", "button"),
        ("role=\"link\"", "link"),
        ("role=\"tab\"", "tab"),
    ] {
        let mut i = 0;
        while let Some(pos) = html[i..].find(role) {
            let role_at = i + pos;
            i = role_at + role.len();
            // Find the enclosing tag's `<` (scan left) and `>` (scan right). The role
            // attribute lives inside one open tag, so the nearest `<` before it that is
            // not a `</` close starts the element.
            let Some(lt) = html[..role_at].rfind('<') else {
                continue;
            };
            if html[lt..].starts_with("</") {
                continue; // role text inside a close tag — not an open tag
            }
            let Some(rel_gt) = html[role_at..].find('>') else {
                continue;
            };
            let open_end = role_at + rel_gt; // index of this tag's '>'
            // Tag name: letters right after `<`.
            let name: String = html[lt + 1..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            // Native `<a>`/`<button>` are already audited by `interactives`; skip to
            // avoid a duplicate finding. A self-closing/void element has no inner name.
            if name == "a" || name == "button" || html[lt..open_end].ends_with('/') {
                i = open_end + 1;
                continue;
            }
            let open = &html[lt + 1..open_end];
            // Match the close tag for `name`, accounting for same-name nesting.
            let inner_start = open_end + 1;
            let inner = matching_inner(html, &name, inner_start);
            out.push(Interactive { kind, open, inner });
            i = inner_start;
        }
    }
    out
}

/// The HTML between `inner_start` and the close tag that matches the element of tag
/// `name` opened just before `inner_start`, accounting for same-name nesting. Falls back
/// to the rest of the string if no balanced close is found (a conservative scan).
fn matching_inner<'a>(html: &'a str, name: &str, inner_start: usize) -> &'a str {
    let open_tag = format!("<{name}");
    let close_tag = format!("</{name}");
    let mut depth = 1usize;
    let mut j = inner_start;
    while j < html.len() {
        let next_open = html[j..].find(&open_tag).map(|p| j + p);
        let next_close = html[j..].find(&close_tag).map(|p| j + p);
        match (next_open, next_close) {
            (_, None) => break, // no close: fall through to rest-of-string
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                j = o + open_tag.len();
            }
            (_, Some(c)) => {
                depth -= 1;
                if depth == 0 {
                    return &html[inner_start..c];
                }
                j = c + close_tag.len();
            }
        }
    }
    &html[inner_start..]
}

/// Every `<a href …>…</a>` and `<button …>…</button>` in `html`, returned as the open
/// tag + inner HTML so the caller can test for an accessible name. Nested same-type
/// elements are rare in content; the first close tag wins (a conservative scan, never a
/// false *positive*).
pub(super) fn interactives(html: &str) -> Vec<Interactive<'_>> {
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

/// The page title block read as a heading: it is `blocks[0]` and its `<h1 class="title">`
/// is the page's only `<h1>`, so an outline scan that skips it is not scanning the outline
/// a reader navigates.
///
/// AP7-1's second cause. [`heading_level`] requires a block's html to *start with* `<hN`,
/// and the title block starts `<header class="tali-title-block">`, so `prev` stayed `0`
/// through the first body heading — the one carrying the largest jump on the page. The
/// single most common heading skip in the corpus was the one shape the rule was
/// structurally incapable of reporting, and `check` printed "no problems found" on 37 of
/// 51 book pages.
fn title_block_level(html: &str) -> Option<u8> {
    (html.starts_with("<header class=\"tali-title-block\"") && html.contains("<h1")).then_some(1)
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
///    is flagged (never "doesn't start at h1").
/// 2. **Interactive element with no accessible name** — an `<a href>`/`<button>`, or any
///    element with `role="button"|"link"|"tab"` (e.g. a `<div role="button">`), whose
///    text is empty and which carries no `aria-label`/`title` and no labelled
///    `<img>`/`<svg>` descendant (e.g. an icon-only link).
/// 3. **`<img>` without `alt`** — a raw/passthrough `<img>` with no `alt` attribute at
///    all. (`![]()` markdown always emits an `alt`, so this catches hand-written
///    `<img>` only.)
///
/// The heading scan counts the page's title block as its `<h1>` (see
/// [`title_block_level`]), so the first body heading is compared against something.
pub fn validate_a11y(blocks: &[Block]) -> Vec<Warning> {
    let mut out = Vec::new();

    // (1) Heading-level skips.
    {
        let mut prev = 0u8;
        for b in blocks {
            let Some(lvl) = heading_level(&b.html).or_else(|| title_block_level(&b.html)) else {
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

        // (2) Interactive elements with no accessible name. Both native `<a>`/`<button>`
        // and `role="button"|"link"|"tab"` elements (e.g. a `<div role="button">`) are
        // audited; the role scan deliberately skips native `<a>`/`<button>` so they are
        // not flagged twice.
        for el in interactives(&b.html)
            .into_iter()
            .chain(role_interactives(&b.html))
        {
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

        // (4) Label in Name (WCAG 2.1 AA, 2.5.3): a control whose `aria-label` does not
        // contain its own visible text. A voice-control user says what they can read, so a
        // button reading "Save draft" but named "Submit" cannot be operated by voice at all
        // — and a screen-reader user hears one thing while a sighted colleague reads
        // another. The check is *containment*, not equality: an accessible name may add
        // context ("Search the site" for a control reading "Search"), it may not replace it.
        //
        // R9 picked this as the one axe rule that ports cleanly into the kernel-free
        // channel, and it is worth having statically for a second reason: Lighthouse
        // weights `label-content-name-mismatch` **0**, so a page can fail it while the
        // accessibility category still reads 100.
        for el in interactives(&b.html)
            .into_iter()
            .chain(role_interactives(&b.html))
        {
            // `aria-labelledby` wins over `aria-label` and resolves against ids elsewhere
            // in the document, which this block-local scan cannot see. Skip rather than
            // guess: a wrong accusation costs more than a missed one.
            if el.open.contains("aria-labelledby=") {
                continue;
            }
            let Some(label) = tag_attr(el.open, "aria-label=\"") else {
                continue;
            };
            let visible = fold_label(&visible_label(el.inner));
            // No visible text at all is the icon-only case — rule 2's business, and exactly
            // what an `aria-label` is *for*. 2.5.3 has nothing to say about it.
            if visible.is_empty() || fold_label(label).contains(&visible) {
                continue;
            }
            let w = Warning::new(format!(
                "{} is named `{}` but reads `{}`, so its accessible name disagrees with its \
                 visible text (WCAG 2.5.3: the name must contain the visible label, or voice \
                 control cannot reach it)",
                el.kind,
                label.trim(),
                visible_label(el.inner)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
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
            } else if is_img && let Some(msg) = placeholder_alt_message(tag) {
                // A non-empty but useless alt (`alt="image"`, a filename echo): it passes the
                // missing-alt check yet tells a screen-reader user nothing. A common LLM tell.
                let w = Warning::new(msg);
                out.push(match line {
                    Some(l) => w.at(b.source_file.clone(), l),
                    None => w,
                });
            }
        }
    }

    out
}

/// The text a sighted reader actually sees inside an interactive element: markup removed
/// and every `aria-hidden="true"` subtree dropped.
///
/// Dropping those subtrees is the whole subtlety of WCAG 2.5.3. A shortcut hint marked
/// `<kbd aria-hidden="true">⌘K</kbd>` is *painted* but is not part of the accessible name
/// and is not what a voice-control user says, so counting it as the visible label would
/// accuse the correct fix of being the defect. That is not hypothetical: it is exactly the
/// shape item 124 shipped on the search button.
///
/// Conservative in one direction on purpose — an unbalanced hidden subtree swallows the
/// rest of the element, which can only *lose* a warning, never invent one, because an empty
/// visible label is skipped rather than reported.
///
/// Nesting is tracked with a plain depth counter rather than with [`matching_inner`], which
/// matches a close tag by name *prefix*: there `<i>` would take `<img` for a nested `<i>`
/// and unbalance the count.
fn visible_label(inner: &str) -> String {
    /// Void elements close themselves, so inside a hidden subtree they must not increment
    /// the depth — otherwise the first `<img>` in an icon swallows the rest of the label.
    const VOID: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source",
        "track", "wbr",
    ];
    let mut out = String::new();
    let mut rest = inner;
    // How deep we are inside an `aria-hidden` subtree; 0 means the text is visible.
    let mut hidden = 0usize;
    loop {
        let Some(lt) = rest.find('<') else {
            if hidden == 0 {
                out.push_str(rest);
            }
            break;
        };
        if hidden == 0 {
            out.push_str(&rest[..lt]);
        }
        let tail = &rest[lt..];
        let Some(gt) = tail.find('>') else { break };
        let tag = &tail[..gt];
        rest = &tail[gt + 1..];
        let closing = tag.starts_with("</");
        let name = tag[1..]
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if name.is_empty() {
            continue; // a comment or a doctype, not an element
        }
        // An element that cannot contain anything never changes the depth.
        let childless = tag.ends_with('/') || VOID.contains(&name.as_str());
        if hidden > 0 {
            if closing {
                hidden -= 1;
            } else if !childless {
                hidden += 1;
            }
            continue;
        }
        if !closing && !childless && tag.contains("aria-hidden=\"true\"") {
            hidden = 1;
            continue;
        }
        // Element boundaries are word boundaries to a reader, so they must not fuse two
        // words ("Next" + "page" is not "Nextpage"). `fold_label` collapses the run.
        out.push(' ');
    }
    out
}

/// Fold a label to the form WCAG 2.5.3 compares: lowercase, punctuation and symbols
/// dropped, whitespace collapsed. 2.5.3 is about whether a voice-control user saying the
/// visible words hits the control, so case, an ellipsis and a trailing colon are all noise.
fn fold_label(s: &str) -> String {
    let mut out = String::new();
    let mut pending_gap = false;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if pending_gap && !out.is_empty() {
                out.push(' ');
            }
            pending_gap = false;
            out.extend(c.to_lowercase());
        } else {
            pending_gap = true;
        }
    }
    out
}

/// Words that name an image's *medium* rather than its content — useless as alt text.
const PLACEHOLDER_ALT_WORDS: &[&str] = &[
    "image",
    "photo",
    "photograph",
    "picture",
    "pic",
    "figure",
    "screenshot",
    "graphic",
    "graphics",
    "img",
];

/// A warning when a non-empty `alt` looks like a placeholder — a bare medium word
/// (`alt="image"`) or an echo of the image filename (`alt="scree.png"` for
/// `src="scree.png"`) — else `None`. `alt=""` (decorative) is exempt. Kept deliberately
/// narrow (exact word match + filename echo) so a descriptive alt is never accused.
fn placeholder_alt_message(tag: &str) -> Option<String> {
    let raw = tag_attr(tag, "alt=\"")?;
    let alt = raw
        .trim()
        .trim_end_matches(['.', ':', ','])
        .to_ascii_lowercase();
    if alt.is_empty() {
        return None; // alt="" is the sanctioned decorative marker.
    }
    let is_placeholder = PLACEHOLDER_ALT_WORDS.contains(&alt.as_str())
        || tag_attr(tag, "src=\"").is_some_and(|src| {
            let file = src
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(src)
                .to_ascii_lowercase();
            let stem = file.rsplit_once('.').map_or(file.as_str(), |(s, _)| s);
            alt == file || alt == stem
        });
    is_placeholder.then(|| {
        format!(
            "alt text `{}` looks like a placeholder (describe the image's content, or use \
             alt=\"\" if it is decorative)",
            raw.trim()
        )
    })
}
