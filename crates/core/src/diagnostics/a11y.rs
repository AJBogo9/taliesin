//! Static accessibility checks (heading-level skips, alt-less and placeholder-alt images).

use super::helpers::{heading_level, start_line};
use crate::render::{Block, Tag, Warning, attr_value, tags};

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

        // (3) Raw `<img>` with no `alt` attribute. Walked, not scanned: the hand-rolled
        // version ended each tag at the first `>`, so `<img alt="width > height" …>` was
        // truncated before its `src` and the sibling asset check went silent on it; it also
        // read the `<img src="${e}">` fragments inside the mermaid and Plot bundles every
        // page inlines, which are script text and not images at all.
        for tag in tags(&b.html) {
            if !tag.name.eq_ignore_ascii_case("img") {
                continue;
            }
            // A valueless `alt` reads as present-and-empty, i.e. decorative, exactly as the
            // whole-word attribute test it replaces had it.
            let w = match attr_value(&tag, "alt") {
                None => Some(Warning::new(
                    "image is missing alt text (add alt text, or alt=\"\" if decorative)",
                )),
                // A non-empty but useless alt (`alt="image"`, a filename echo): it passes
                // the missing-alt check yet tells a screen-reader user nothing. A common
                // LLM tell.
                Some(_) => placeholder_alt_message(&tag).map(Warning::new),
            };
            if let Some(w) = w {
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
fn placeholder_alt_message(tag: &Tag<'_>) -> Option<String> {
    let raw = attr_value(tag, "alt")?;
    let alt = raw
        .trim()
        .trim_end_matches(['.', ':', ','])
        .to_ascii_lowercase();
    if alt.is_empty() {
        return None; // alt="" is the sanctioned decorative marker.
    }
    let is_placeholder = PLACEHOLDER_ALT_WORDS.contains(&alt.as_str())
        || attr_value(tag, "src").is_some_and(|src| {
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
