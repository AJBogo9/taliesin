//! Static accessibility checks (heading-level skips, alt-less and placeholder-alt images).

use super::helpers::heading_level;
use crate::render::sourcepos_start_line as start_line;
use crate::render::{Block, Tag, Warning, attr_value, tags};

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
/// The heading scan walks every heading ELEMENT in document order, so the title block's
/// `<h1 class="title">` counts as the page's `<h1>` and the first body heading is compared
/// against something (AP7-1: asking only about each block's root element skipped the
/// title block, and 37 of 51 book pages skipped a level under "no problems found"), and
/// a heading inside a `:::` container is part of the outline too.
///
/// The accessible-name rules (an icon-only `<a>`/`<button>` with no `aria-label`, and WCAG
/// 2.5.3's label-in-name mismatch) were cut on 2026-08-08 with the rest of the diagnostics
/// contraction. They needed a nesting-aware interactive-element scan (the same scan the
/// link-text collision lint shared), and this tool's pages are prose, where the author
/// writes the link text and reads it back in the preview.
pub fn validate_a11y(blocks: &[Block]) -> Vec<Warning> {
    let mut out = Vec::new();

    // (1) Heading-level skips, over every heading element through the walker. A container
    // block's html carries its children, and asking only about a block's ROOT element
    // missed a skip inside `::: {.foo}` and, worse, took the h3 in a `.column-margin` for
    // absent, so the h4 after it was a false skip.
    {
        let mut prev = 0u8;
        for b in blocks {
            for tag in tags(&b.html) {
                let Some(lvl) = heading_level(tag.text) else {
                    continue;
                };
                if prev > 0 && lvl >= prev + 2 {
                    let w = Warning::new(format!(
                        "heading level skips from h{prev} to h{lvl} (add an intervening heading, or demote this one)"
                    ));
                    // Located at the heading itself: its own `data-sourcepos` with its own
                    // `data-source-file`, read off ONE tag so the file and line stay a
                    // matched pair; the block's pair when the heading carries none.
                    let (file, line) = match attr_value(&tag, "data-sourcepos") {
                        Some(pos) => (
                            attr_value(&tag, "data-source-file").map(|f| f.into_owned()),
                            start_line(&pos),
                        ),
                        None => (b.source_file.clone(), start_line(&b.sourcepos)),
                    };
                    out.push(match line {
                        Some(l) => w.at(file, l),
                        None => w,
                    });
                }
                prev = lvl;
            }
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
            // The file's own name: `my%20pic.png` is the file `my pic.png`.
            let src = crate::render::asset_fs_path(&src);
            let file = src
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&src)
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
