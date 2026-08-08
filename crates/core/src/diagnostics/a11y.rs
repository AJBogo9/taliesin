//! Static accessibility checks (heading-level skips, alt-less and placeholder-alt images).

use super::helpers::{heading_level, start_line, tag_attr};
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

/// Static accessibility checks over the rendered block model. Read-only — reads only block
/// HTML + sourcepos. Two rules ship; document-`lang` (the page builders default it to `en`,
/// so a built page is never lang-less) and body-text contrast (needs *computed* CSS, not a
/// static block-model fact) were never in this channel.
///
/// 1. **Heading-level skip** — a heading that jumps `>= 2` levels deeper than the
///    previous one (e.g. `<h2>` then `<h4>`). Conservative: only a *mid-document* skip
///    is flagged (never "doesn't start at h1").
/// 2. **`<img>` alt text**: a raw/passthrough `<img>` with no `alt` attribute at all
///    (`![]()` markdown always emits one, so this catches hand-written `<img>`), or an
///    `alt` that names the medium rather than the content (see
///    [`placeholder_alt_message`]).
///
/// The heading scan counts the page's title block as its `<h1>` (see
/// [`title_block_level`]), so the first body heading is compared against something.
///
/// The accessible-name rules (an icon-only `<a>`/`<button>` with no `aria-label`, and WCAG
/// 2.5.3's label-in-name mismatch) were cut on 2026-08-08 with the rest of the diagnostics
/// contraction. They needed a nesting-aware interactive-element scan (the same scan the
/// link-text collision lint shared), and this tool's pages are prose, where the author
/// writes the link text and reads it back in the preview.
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
