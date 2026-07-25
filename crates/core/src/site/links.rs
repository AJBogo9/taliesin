//! Link rewriting + cross-file link validation: map `.tmd` hrefs to their built `.html`
//! URLs, resolve depth-relative hrefs, and the manual-local-link scan used by cross-page
//! validation. `rewrite_tmd_links` is the public entry the dev server applies to each
//! page's body. `use super::*` reaches Page/Block/esc.

use super::*;

/// Map a source rel-path to its built `.html` URL (`x.tmd` → `x.html`); a non-source
/// path round-trips unchanged.
pub(super) fn tmd_to_html(rel: &str) -> String {
    match crate::ext::strip_source_ext(rel) {
        Some(stem) => format!("{stem}.html"),
        None => rel.to_string(),
    }
}

/// Resolve a config/author href for emission from a page at `up` depth: leave
/// external/absolute/anchor links alone, map intra-site `.tmd` to `.html`, and
/// prefix in-tree relative links with the page's `../` depth.
pub(super) fn resolve_href(href: &str, up: &str) -> String {
    if href.starts_with('#')
        || href.starts_with("//")
        || href.contains("://")
        || href.starts_with("mailto:")
        || href.starts_with("tel:")
    {
        return href.to_string();
    }
    // Site-absolute (`/blog.tmd`): tmd→html, keep absolute.
    if let Some(rest) = href.strip_prefix('/') {
        return format!("/{}", tmd_href(rest));
    }
    // Relative: tmd→html, prefix with the page's depth.
    format!("{up}{}", tmd_href(href))
}

/// `.tmd`→`.html` on an href, preserving any `#fragment`.
pub(super) fn tmd_href(href: &str) -> String {
    let (path, frag) = match href.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (href, None),
    };
    let mapped = tmd_to_html(path);
    match frag {
        Some(f) => format!("{mapped}#{f}"),
        None => mapped,
    }
}

/// Whether a navbar `href` points at `page` (so the item renders active).
///
/// A nav href may carry a `#fragment` or `?query` (an in-page anchor, a cache-bust);
/// the active page is decided by the path alone, so both are stripped before matching
/// (otherwise `tmd_to_html` can't map the `.tmd` and the highlight is silently lost).
pub(super) fn href_matches_page(href: &str, page: &Page) -> bool {
    let h = href.trim_start_matches('/');
    let h = h.split(['#', '?']).next().unwrap_or(h);
    let target = tmd_to_html(h);
    target == page.url || h == page.rel
}

/// Rewrite every intra-site `.tmd` link in rendered HTML to its `.html` target,
/// preserving the author's relative/absolute prefix and `#fragment`. External
/// links, data URIs, and non-`.tmd` paths are untouched.
pub fn rewrite_tmd_links(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find("href=\"") {
        let val_start = pos + 6;
        out.push_str(&rest[..val_start]);
        let after = &rest[val_start..];
        let Some(end) = after.find('"') else {
            rest = after;
            break;
        };
        let val = &after[..end];
        out.push_str(&rewrite_one_href(val));
        out.push('"');
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Whether a link value must be left untouched by the site rewriters: an in-page anchor
/// (`#`), protocol-relative (`//`), absolute-scheme (`://`), or a non-http special scheme
/// (`data:`/`mailto:`/`tel:`/`vscode:`). Shared by `rewrite_one_href` and `hover::rebase_url`
/// so the two URL rewriters can't silently diverge when a new scheme is added.
pub(super) fn is_external_or_special(val: &str) -> bool {
    val.starts_with('#')
        || val.starts_with("//")
        || val.contains("://")
        || val.starts_with("data:")
        || val.starts_with("mailto:")
        || val.starts_with("tel:")
        || val.starts_with("vscode:")
}

pub(super) fn rewrite_one_href(val: &str) -> String {
    // Only touch in-site links (skip external/anchor/data); rewrite the `.tmd`
    // path component, keeping prefix + fragment intact.
    if is_external_or_special(val) {
        return val.to_string();
    }
    // `.tmd`→`.html` on the path component, fragment preserved (a non-`.tmd` path
    // round-trips unchanged through `tmd_to_html`).
    tmd_href(val)
}

/// Whether a block's *leading element tag* carries `id="x"` (so a `::: {#x}`
/// placeholder matches, but a code sample or prose that merely contains the text
/// `id="x"` in its body does not).
/// Whether `needle` appears within a block's *leading element tag*. Quote-aware tag end,
/// so a raw-HTML placeholder whose leading tag has a `>` inside an attribute value (e.g.
/// `<div title="a > b" id="x">`) is handled. Shared by `block_tag_has_id` (specific id) and
/// `hover::leading_tag_has_id` (any ` id="`).
pub(super) fn leading_tag_contains(html: &str, needle: &str) -> bool {
    match crate::render::tag_end(html) {
        Some(gt) => html[..gt].contains(needle),
        None => html.contains(needle),
    }
}

pub(super) fn block_tag_has_id(html: &str, id: &str) -> bool {
    // Leading space so a real `id="x"` attribute matches but the `id="x"` *suffix* of
    // `data-block-id="x"` does not (attributes are space-separated); mirrors
    // `hover::leading_tag_has_id`.
    leading_tag_contains(html, &format!(" id=\"{id}\""))
}

/// Resolve `target` (a path relative to the file at `from_rel`) to a site-root-
/// relative path: e.g. (`posts/em/index.tmd`, `thumbnail.webp`) → `posts/em/thumbnail.webp`.
pub(super) fn join_rel(from_rel: &str, target: &str) -> String {
    if target.starts_with('/') {
        return target.trim_start_matches('/').to_string();
    }
    let dir = match from_rel.rsplit_once('/') {
        Some((d, _)) => d,
        None => "",
    };
    let mut parts: Vec<&str> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// Like [`join_rel`] but returns `None` when `target` climbs *above* the file's directory
/// (`../` past the site root). A root-escaping link points at a sibling project/mount the
/// single-site registry can't see, so the cross-page link checker skips it rather than
/// false-flag a legitimate cross-book link.
pub(super) fn join_rel_in_root(from_rel: &str, target: &str) -> Option<String> {
    if target.starts_with('/') {
        return Some(target.trim_start_matches('/').to_string());
    }
    let dir = from_rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut parts: Vec<&str> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?; // None when the link climbs above the site root
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

/// `.html`→source-extension candidates on a url path (`x.html` → `x.tmd`, one per
/// [`crate::ext::ACCEPTED_SOURCE_EXTS`]), so the checker can test whether a link target is
/// backed by a source file on disk, in any accepted spelling. A non-`.html` path yields no
/// candidates (there is nothing to probe).
pub(super) fn html_to_tmd(url: &str) -> Vec<String> {
    match url.strip_suffix(".html") {
        Some(stem) => crate::ext::ACCEPTED_SOURCE_EXTS
            .iter()
            .map(|ext| format!("{stem}.{ext}"))
            .collect(),
        None => Vec::new(),
    }
}

/// The 1-based start line from a block's `sourcepos` (`"startLine:col-…"`), if positive.
/// A local copy of `diagnostics::start_line` (that one is private to its module); used to
/// locate cross-page link warnings to their source line.
pub(super) fn sourcepos_start_line(sourcepos: &str) -> Option<u32> {
    sourcepos
        .split(':')
        .next()?
        .parse::<u32>()
        .ok()
        .filter(|&l| l > 0)
}

/// Every `id="…"` value in a block's HTML, added to `out` (the page's anchor set for the
/// cross-page link check). Plain substring scan, matching how `search`/`diagnostics` read ids.
pub(super) fn collect_html_ids(html: &str, out: &mut std::collections::HashSet<String>) {
    let needle = "id=\"";
    let mut i = 0;
    while let Some(pos) = html[i..].find(needle) {
        let start = i + pos + needle.len();
        let Some(len) = html[start..].find('"') else {
            break;
        };
        out.insert(html[start..start + len].to_string());
        i = start + len;
    }
}

/// Manual relative `<a href>` links in a block's HTML, as `(path, Option<fragment>)`.
/// External (`http(s)://`, `//`, `mailto:`, `tel:`), data-URI, empty, bare in-page
/// `#frag`, and cross-reference (`tali-xref`) links are skipped — the cross-page checker
/// only resolves intra-site file links (anchors handled per target page). The path keeps
/// its authored form (`other.tmd`, `../sec/page.html`); the fragment is split off.
pub(super) fn manual_local_links(html: &str) -> Vec<(&str, Option<&str>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find("<a ") {
        let tag_start = i + pos;
        let Some(rel_end) = html[tag_start..].find('>') else {
            break;
        };
        let tag = &html[tag_start..tag_start + rel_end];
        i = tag_start + rel_end + 1;
        if tag.contains("tali-xref") {
            continue;
        }
        let Some(hpos) = tag.find("href=\"") else {
            continue;
        };
        let vstart = hpos + "href=\"".len();
        let Some(vlen) = tag[vstart..].find('"') else {
            continue;
        };
        let val = &tag[vstart..vstart + vlen];
        // Skip external / non-file / bare-anchor links.
        if val.is_empty()
            || val.starts_with('#')
            || val.starts_with("//")
            || val.contains("://")
            || val.starts_with("data:")
            || val.starts_with("mailto:")
            || val.starts_with("tel:")
            || val.starts_with("vscode:")
        {
            continue;
        }
        let (path, frag) = match val.split_once('#') {
            Some((p, f)) => (p, Some(f)),
            None => (val, None),
        };
        // Strip a `?query` so a cache-busting / signed link (`page.tmd?v=2`) still
        // resolves to its page instead of false-flagging — mirrors the single-doc
        // checker (`diagnostics::validate_local_links`).
        let path = &path[..path.find('?').unwrap_or(path.len())];
        if !path.is_empty() {
            out.push((path, frag));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmd_urls_map_to_html_and_the_retired_ext_is_not_a_source_link() {
        assert_eq!(tmd_to_html("blog.tmd"), "blog.html");
        assert_eq!(tmd_to_html("index.tmd"), "index.html");
        assert_eq!(
            tmd_to_html("posts/intro/index.tmd"),
            "posts/intro/index.html"
        );
        assert_eq!(tmd_to_html("style.css"), "style.css");
        // `.qmd` is no longer an accepted source extension: it round-trips unchanged,
        // it does NOT map to `.html`.
        assert_eq!(tmd_to_html("blog.qmd"), "blog.qmd");
        assert_eq!(
            tmd_to_html("posts/em-algorithm/index.qmd"),
            "posts/em-algorithm/index.qmd"
        );
    }

    #[test]
    fn link_rewrite_preserves_prefix_and_fragment() {
        let html = r##"<a href="blog.tmd">b</a> <a href="../KL-divergence/index.tmd#sec-x">k</a> <a href="/projects.tmd">p</a> <a href="talk.qmd">t</a> <a href="https://x.com/a.qmd">ext</a> <a href="#local">l</a>"##;
        let out = rewrite_tmd_links(html);
        assert!(out.contains("href=\"blog.html\""));
        assert!(out.contains("href=\"../KL-divergence/index.html#sec-x\""));
        assert!(out.contains("href=\"/projects.html\""));
        assert!(
            out.contains("href=\"talk.qmd\""),
            "a .qmd href is no longer a source link: it must NOT rewrite to .html"
        );
        assert!(
            out.contains("href=\"https://x.com/a.qmd\""),
            "external untouched"
        );
        assert!(out.contains("href=\"#local\""), "anchor untouched");
    }

    #[test]
    fn resolve_href_handles_depth_and_externals() {
        assert_eq!(resolve_href("blog.tmd", "../../"), "../../blog.html");
        assert_eq!(resolve_href("/blog.tmd", "../"), "/blog.html");
        assert_eq!(resolve_href("https://x.com", "../"), "https://x.com");
        assert_eq!(resolve_href("#top", "../"), "#top");
        // `.qmd` is no longer an accepted source extension: a `.qmd` href round-trips
        // unchanged (still depth-prefixed / absolute-marked), it does NOT map to `.html`.
        assert_eq!(resolve_href("blog.qmd", "../../"), "../../blog.qmd");
        assert_eq!(resolve_href("/blog.qmd", "../"), "/blog.qmd");
    }

    #[test]
    fn join_rel_in_root_resolves_and_rejects_escapes() {
        // In-site sibling + nested resolve to a site-root-relative url.
        assert_eq!(
            join_rel_in_root("posts/x/index.html", "../y/index.html").as_deref(),
            Some("posts/y/index.html")
        );
        assert_eq!(
            join_rel_in_root("index.html", "about.html").as_deref(),
            Some("about.html")
        );
        assert_eq!(
            join_rel_in_root("index.html", "/abs.html").as_deref(),
            Some("abs.html")
        );
        // A link climbing ABOVE the site root (a sibling book / mount) is rejected, so the
        // cross-page checker skips it rather than false-flag a legitimate cross-book link.
        assert_eq!(
            join_rel_in_root("index.html", "../internals/index.html"),
            None
        );
        assert_eq!(
            join_rel_in_root("guide/index.html", "../../escape.html"),
            None
        );
    }

    #[test]
    fn manual_local_links_skips_external_anchor_and_xref() {
        let html = r##"<a href="other.tmd">o</a> <a href="page.html#sec">p</a> <a href="https://x.com">e</a> <a href="#top">t</a> <a href="x.html" class="tali-xref">r</a>"##;
        let links = manual_local_links(html);
        assert_eq!(links, vec![("other.tmd", None), ("page.html", Some("sec"))]);
    }

    #[test]
    fn html_to_tmd_yields_the_tmd_candidate_only() {
        // A `.html` link target is backed by its `.tmd` source only — `.tmd` is now the
        // only accepted source extension, so `.qmd` is no longer a candidate.
        assert_eq!(html_to_tmd("blog.html"), vec!["blog.tmd".to_string()]);
        assert_eq!(html_to_tmd("style.css"), Vec::<String>::new());
    }

    #[test]
    fn manual_local_links_strips_query_string_keeps_fragment() {
        // A cache-busting / signed link must resolve to its page, not false-flag.
        let html = r##"<a href="report.tmd?v=2">q</a> <a href="dash.html?token=abc#sec">d</a>"##;
        let links = manual_local_links(html);
        assert_eq!(
            links,
            vec![("report.tmd", None), ("dash.html", Some("sec"))]
        );
    }

    #[test]
    fn block_tag_has_id_requires_a_real_id_attribute_not_a_data_block_id_suffix() {
        // A listing target `::: {#intro}` is found by scanning each block's leading tag for
        // the author id. `data-block-id="…"` ends in `id="…"`, so a bare `id="X"` needle
        // substring-matched it — a block whose content-hash id happened to equal the listing
        // id would falsely bind. A real `id` attribute is always space-separated, so match on
        // the word boundary (mirrors `hover::leading_tag_has_id`).
        assert!(block_tag_has_id(
            r#"<div id="intro" class="x">c</div>"#,
            "intro"
        ));
        assert!(block_tag_has_id(
            r#"<figure data-block-id="b-abc" data-sourcepos="1:1-2:3" id="intro">c</figure>"#,
            "intro"
        ));
        // The false positive: the searched id equals a block's data-block-id value.
        assert!(!block_tag_has_id(
            r#"<div data-block-id="intro" data-sourcepos="1:1-1:2">c</div>"#,
            "intro"
        ));
        // Body text that merely contains `id="intro"` outside the leading tag never matches.
        assert!(!block_tag_has_id(
            r#"<p>a code sample: <code>id="intro"</code></p>"#,
            "intro"
        ));
    }

    #[test]
    fn active_nav_href_matches_page_ignoring_fragment_and_query() {
        // `href_matches_page` only reads `rel`/`url`; a minimal fixture is enough.
        fn page_fixture(rel: &str, url: &str) -> Page {
            Page {
                input: std::path::PathBuf::from(rel),
                rel: rel.to_string(),
                url: url.to_string(),
                title: None,
                date: None,
                description: None,
                authors: Vec::new(),
                card_image: None,
                card_image_alt: None,
                categories: Vec::new(),
                listings: Vec::new(),
                hero: None,
                page_layout: None,
                has_bibliography: false,
                draft: false,
            }
        }
        let blog = page_fixture("blog.tmd", "blog.html");
        // Plain hrefs still match (regression guard).
        assert!(href_matches_page("blog.tmd", &blog));
        assert!(href_matches_page("/blog.tmd", &blog));
        assert!(href_matches_page("blog.html", &blog));
        // A `#fragment` on the nav href must not lose the active highlight — the active
        // page is decided by the path alone (a fragment is a within-page location).
        assert!(href_matches_page("blog.tmd#recent", &blog));
        assert!(href_matches_page("blog.html#recent", &blog));
        // A `?query` (cache-bust / signed link) likewise.
        assert!(href_matches_page("blog.html?v=2", &blog));
        assert!(href_matches_page("blog.tmd?utm=x#recent", &blog));
        // A different page must NOT match, fragment or no.
        assert!(!href_matches_page("about.tmd", &blog));
        assert!(!href_matches_page("about.tmd#recent", &blog));
    }
}
