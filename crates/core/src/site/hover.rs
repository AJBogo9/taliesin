//! Cross-page hover-preview snippet index: anchor → the rendered HTML of the block
//! that defines it (figure / theorem / table / equation / listing), with relative asset
//! URLs rebased site-root-relative. Built once at discovery, served as `hover-index.js`
//! and lazy-loaded by `12-link-preview.js` when a reader hovers a cross-page `.tali-xref`.
//! Section headings are deliberately excluded (they get no hover preview), so a heading
//! anchor is never indexed. `use super::*` reaches Block + the link helpers.

use super::*;

/// One snippet is capped so a giant figure/table can't blow up the index.
const SNIPPET_CAP: usize = 8000;

/// Whether a block's leading element tag is a heading (`<h1..6`).
fn is_heading(html: &str) -> bool {
    let t = html.trim_start().as_bytes();
    t.len() >= 3 && t[0] == b'<' && t[1] == b'h' && (b'1'..=b'6').contains(&t[2])
}

/// Truncate to at most `SNIPPET_CAP` chars on a char boundary.
fn cap(mut s: String) -> String {
    if let Some((i, _)) = s.char_indices().nth(SNIPPET_CAP) {
        s.truncate(i);
    }
    s
}

/// The rendered HTML for `anchor`'s defining block — a figure, theorem, table, equation,
/// or listing. Section headings get no hover preview, so a heading anchor returns `None`
/// and never enters the index.
pub(super) fn extract_snippet(blocks: &[Block], anchor: &str) -> Option<String> {
    let bi = blocks
        .iter()
        .position(|b| block_tag_has_id(&b.html, anchor))
        .or_else(|| {
            let needle = format!("id=\"{anchor}\"");
            blocks.iter().position(|b| b.html.contains(&needle))
        })?;
    if is_heading(&blocks[bi].html) {
        return None;
    }
    Some(cap(blocks[bi].html.clone()))
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
    fn extract_returns_none_for_a_heading_anchor() {
        // Section headings get no hover preview: a heading anchor is never indexed, even
        // when it carries the universal `data-block-id` (which must not read as a real id).
        let blocks = vec![
            blk("<h2 id=\"sec-m\" data-block-id=\"b-1\">Methods</h2>"),
            blk("<p>intro one</p>"),
        ];
        assert!(
            extract_snippet(&blocks, "sec-m").is_none(),
            "a section heading must not be previewed"
        );
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
