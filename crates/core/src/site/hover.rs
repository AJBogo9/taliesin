//! Cross-page hover-preview snippet index: anchor → the rendered HTML of the block
//! that defines it (figure / theorem / table / equation / listing / section), with
//! relative asset URLs rebased site-root-relative. Built once at discovery, served as
//! `hover-index.js` and lazy-loaded by `12-link-preview.js` when a reader hovers a
//! cross-page `.tali-xref`. `use super::*` reaches Block + the link helpers.

use super::*;

/// One snippet is capped so a giant figure/table can't blow up the index.
const SNIPPET_CAP: usize = 8000;

/// Whether a block's leading element tag is a heading (`<h1..6`).
fn is_heading(html: &str) -> bool {
    let t = html.trim_start().as_bytes();
    t.len() >= 3 && t[0] == b'<' && t[1] == b'h' && (b'1'..=b'6').contains(&t[2])
}

/// Whether a block's leading element tag carries a real `id="…"` attribute (mirrors the
/// client's `!n.id` stop condition when gathering a heading's following blocks). Matches
/// ` id="` (space-prefixed) so the universal `data-block-id="` attribute — hyphen-prefixed
/// — is NOT mistaken for a real id (which would make every heading a bare-title snippet).
fn leading_tag_has_id(html: &str) -> bool {
    leading_tag_contains(html, " id=\"")
}

/// Truncate to at most `SNIPPET_CAP` chars on a char boundary.
fn cap(mut s: String) -> String {
    if let Some((i, _)) = s.char_indices().nth(SNIPPET_CAP) {
        s.truncate(i);
    }
    s
}

/// The rendered HTML for `anchor`'s defining block. A heading anchor also appends up
/// to two following blocks (stopping at the next heading or a block with its own id),
/// matching the same-page card's "heading + up to 2 siblings" behavior.
pub(super) fn extract_snippet(blocks: &[Block], anchor: &str) -> Option<String> {
    let bi = blocks
        .iter()
        .position(|b| block_tag_has_id(&b.html, anchor))
        .or_else(|| {
            let needle = format!("id=\"{anchor}\"");
            blocks.iter().position(|b| b.html.contains(&needle))
        })?;
    let mut out = blocks[bi].html.clone();
    if is_heading(&blocks[bi].html) {
        // Append up to 2 following blocks, stopping at the next heading or a block with
        // its own id (mirrors the same-page card's "heading + up to 2 siblings").
        for b in blocks[bi + 1..].iter().take(2) {
            if is_heading(&b.html) || leading_tag_has_id(&b.html) {
                break;
            }
            out.push_str(&b.html);
        }
    }
    Some(cap(out))
}

/// Rebase relative `src=`/`href=` values in a snippet to site-root-relative, so the
/// snippet renders correctly in a card shown on a page at any depth. `page_url` is the
/// defining page's url (e.g. `ch/methods.html`). External/absolute/data/anchor URLs are
/// left untouched; `.tmd` path components map to `.html`; a `#fragment` is preserved.
pub(super) fn rewrite_snippet_urls(html: &str, page_url: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    // Rewrite both attributes in one pass, whichever comes first.
    loop {
        let src = rest.find("src=\"").map(|p| (p, 5usize));
        let href = rest.find("href=\"").map(|p| (p, 6usize));
        let next = match (src, href) {
            (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some((pos, kw)) = next else {
            out.push_str(rest);
            break;
        };
        let val_start = pos + kw;
        out.push_str(&rest[..val_start]);
        let after = &rest[val_start..];
        let Some(end) = after.find('"') else {
            out.push_str(after);
            break;
        };
        out.push_str(&rebase_url(&after[..end], page_url));
        out.push('"');
        rest = &after[end + 1..];
    }
    out
}

/// Root-relative rebase of one attribute value; skips external/absolute/data/anchor.
fn rebase_url(val: &str, page_url: &str) -> String {
    if val.is_empty() || is_external_or_special(val) {
        return val.to_string();
    }
    let (path, frag) = match val.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (val, None),
    };
    // Site-absolute (`/x`) -> root-relative (`x`); relative -> resolved against the
    // defining page's directory. `.tmd`->`.html` on the path either way.
    let mapped = qmd_to_html(path.trim_start_matches('/'));
    let rooted = if path.starts_with('/') {
        mapped
    } else {
        join_rel(page_url, &mapped)
    };
    match frag {
        Some(f) => format!("{rooted}#{f}"),
        None => rooted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blk(html: &str) -> Block {
        Block {
            id: "b".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: html.into(),
            cell: None,
        }
    }

    #[test]
    fn extract_single_element_block() {
        let blocks = vec![
            blk("<p>before</p>"),
            blk(
                "<figure id=\"fig-x\" class=\"tali-figure\"><img src=\"p.png\"><figcaption>Figure&nbsp;3</figcaption></figure>",
            ),
            blk("<p>after</p>"),
        ];
        let s = extract_snippet(&blocks, "fig-x").unwrap();
        assert!(s.contains("id=\"fig-x\"") && s.contains("Figure&nbsp;3"));
        assert!(
            !s.contains("before") && !s.contains("after"),
            "a figure is a single-element snippet"
        );
    }

    #[test]
    fn extract_heading_takes_following_blocks_until_next_heading_or_id() {
        let blocks = vec![
            blk("<h2 id=\"sec-m\">Methods</h2>"),
            blk("<p>intro one</p>"),
            blk("<p>intro two</p>"),
            blk("<h2 id=\"sec-n\">Next</h2>"),
        ];
        let s = extract_snippet(&blocks, "sec-m").unwrap();
        assert!(s.contains("Methods") && s.contains("intro one") && s.contains("intro two"));
        assert!(!s.contains("Next"), "stops at the next heading");
    }

    #[test]
    fn extract_heading_caps_at_two_following_blocks() {
        let blocks = vec![
            blk("<h2 id=\"sec-m\">Methods</h2>"),
            blk("<p>one</p>"),
            blk("<p>two</p>"),
            blk("<p>three</p>"),
        ];
        let s = extract_snippet(&blocks, "sec-m").unwrap();
        assert!(s.contains("one") && s.contains("two") && !s.contains("three"));
    }

    #[test]
    fn extract_returns_none_for_unknown_anchor() {
        assert!(extract_snippet(&[blk("<p>x</p>")], "fig-x").is_none());
    }

    #[test]
    fn extract_caps_a_huge_snippet_at_the_limit() {
        // A giant figure/table can't blow up the index: the snippet is char-capped.
        let big = format!(
            "<figure id=\"fig-x\">{}</figure>",
            "x".repeat(SNIPPET_CAP * 2)
        );
        let s = extract_snippet(&[blk(&big)], "fig-x").unwrap();
        assert!(
            s.chars().count() <= SNIPPET_CAP,
            "snippet not capped: {} chars",
            s.chars().count()
        );
    }

    #[test]
    fn extract_heading_appends_following_blocks_that_carry_only_data_block_id() {
        // Regression: every real block carries `data-block-id="…"`; that must NOT read as a
        // real id (only a space-prefixed ` id="` does), else a heading captures nothing.
        let blocks = vec![
            blk("<h1 id=\"sec-m\" data-block-id=\"b-1\">Methods</h1>"),
            blk("<p data-block-id=\"b-2\" data-sourcepos=\"2:1-2:2\">intro one</p>"),
            blk("<div class=\"tali-theorem\" id=\"thm-x\" data-block-id=\"b-3\">stop</div>"),
        ];
        let s = extract_snippet(&blocks, "sec-m").unwrap();
        assert!(s.contains("Methods") && s.contains("intro one"), "got: {s}");
        assert!(
            !s.contains("stop"),
            "stops at the block with a real id (the theorem): {s}"
        );
    }

    #[test]
    fn rewrite_rebases_relative_asset_from_nested_page() {
        let html = "<img src=\"figs/p.png\"><a href=\"other.tmd#s\">o</a>";
        let out = rewrite_snippet_urls(html, "ch/methods.html");
        assert!(
            out.contains("src=\"ch/figs/p.png\""),
            "img rebased to root-relative: {out}"
        );
        assert!(
            out.contains("href=\"ch/other.html#s\""),
            ".tmd->.html + rebased + frag kept: {out}"
        );
    }

    #[test]
    fn rewrite_leaves_absolute_external_data_and_anchor_untouched() {
        let html = "<img src=\"https://x/y.png\"><img src=\"data:image/png;base64,AA\"><a href=\"#top\">t</a><img src=\"/root.png\">";
        let out = rewrite_snippet_urls(html, "ch/methods.html");
        assert!(out.contains("src=\"https://x/y.png\""));
        assert!(out.contains("src=\"data:image/png;base64,AA\""));
        assert!(out.contains("href=\"#top\""));
        assert!(
            out.contains("src=\"root.png\""),
            "site-absolute /x becomes root-relative x: {out}"
        );
    }

    #[test]
    fn rewrite_root_page_leaves_relative_as_is() {
        let out = rewrite_snippet_urls("<img src=\"p.png\">", "methods.html");
        assert!(out.contains("src=\"p.png\""));
    }
}
