//! Cross-page **full-text** search index: every page's title + each anchored
//! heading, each carrying the plain-text body of its section so Cmd-K matches
//! prose, not just headings. Built once at discovery; written to `search-index.js`
//! and lazy-loaded by the client on first open (so it never bloats every page).
//! `use super::*` reaches Page + the render entry point.

use super::*;

/// The per-page search fragments (page `rel` → that page's JSON entries, no
/// surrounding brackets), in page order — one `{u,p,i,l,t,b}` object per page title
/// and per anchored heading (`u`rl, `p`age title, anchor `i`d, `l`evel, heading
/// `t`ext, section `b`ody text). Kept separate from [`assemble`] so the dev server can
/// refresh a single edited page's entries without re-rendering the whole site (see
/// [`super::Site::rebuild_search_index`]). Renders each page's markdown once (no
/// code execution) so the anchor ids match what the served pages emit.
pub(super) fn build_sections(
    pages: &[Page],
    book: &Option<Book>,
    targets: &HashMap<String, XrefTarget>,
    site_defaults: Option<&render::SiteDefaults>,
) -> Vec<(String, String)> {
    pages
        .iter()
        .filter_map(|p| {
            page_fragment(p, super::book::chapter_of(book, p), targets, site_defaults)
                .map(|frag| (p.rel.clone(), frag))
        })
        .collect()
}

/// Assemble the per-page fragments into the served `[…]` JSON array (dropping any
/// empty fragment so the array stays well-formed).
pub(super) fn assemble(sections: &[(String, String)]) -> String {
    let body = sections
        .iter()
        .map(|(_, frag)| frag.as_str())
        .filter(|frag| !frag.is_empty())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

/// The search-index entries for ONE page as a JSON-array **body** (comma-joined
/// `{u,p,i,l,t,b}` objects, no surrounding brackets). `None` when the page is
/// excluded from search (the author's 404 chrome page) or its source can't be read.
///
/// `chapter` is the page's book chapter number (`Site::chapter_for`), so the indexed text
/// carries the numbers the rendered page shows ("Theorem 2.1", not "Theorem 1"). Rendering
/// unscoped here made every snippet in a book contradict its own target and hid a search
/// for the number the reader can actually see.
///
/// `targets` is the xref registry, for the same reason one level out: this renders the page
/// ALONE, and a single-doc render cannot know a cross-PAGE number, so a `@fig-` to another
/// page survives as an unresolved marker reading a bare "Figure". Passing the (already
/// harvested) registry is what lets the snippet agree with its target — so a caller must
/// hand over a registry whose numbers are filled, not the empty one the source scan leaves.
pub(super) fn page_fragment(
    page: &Page,
    chapter: Option<u32>,
    targets: &HashMap<String, XrefTarget>,
    site_defaults: Option<&render::SiteDefaults>,
) -> Option<String> {
    // The author's own 404 page (output URL `404.html`) is navigation chrome, not
    // content: keep it out of the full-text index so a search never surfaces it.
    if page.url == "404.html" {
        return None;
    }
    let (src, doc) = render_finished(page, chapter, targets, site_defaults)?;
    let page_title = page
        .title
        .clone()
        .or(doc.title)
        .unwrap_or_else(|| page.url.clone());

    // `c` (the page's chapter number) and `h` (a heading's ancestor path) are what let the
    // client render the index as the BOOK's outline rather than a flat row list: `c` numbers
    // the group header (a page-title entry's `t` is the bare title — the rendered numbers
    // live on section headings, not on it), and `h` says where a section sits inside its
    // chapter. Both are omitted when empty, so a website's index is byte-identical to before.
    let chapter_field = chapter.map(|c| format!(",\"c\":{c}")).unwrap_or_default();
    let mut entries: Vec<String> = Vec::new();
    let mut push = |id: &str, level: u8, title: &str, body: &str, path: &str| {
        let path_field = if path.is_empty() {
            String::new()
        } else {
            format!(",\"h\":\"{}\"", json_str(path))
        };
        entries.push(format!(
            "{{\"u\":\"{}\",\"p\":\"{}\",\"i\":\"{}\",\"l\":{},\"t\":\"{}\",\"b\":\"{}\"{}{}}}",
            json_str(&page.url),
            json_str(&page_title),
            json_str(id),
            level,
            json_str(title),
            json_str(body),
            chapter_field,
            path_field,
        ));
    };

    let body: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
    let hs = headings_with_pos(&body);
    // A page that emits no title block and opens at `# H1` has that heading as its OWN
    // title, not a section — the same rule `ChapterNumbering` uses to decide the H1 takes
    // the bare chapter number. Indexed as a heading it is a second record for the same
    // destination as the page record, one line below it and reading the same words, and it
    // would sit in every section's ancestor path as pure noise. Fold it into the page record
    // instead (which is what the titled shape already does: a title block's text lands in
    // the page record's body too).
    let title_heading_is_first =
        !render::emits_title_block(crate::frontmatter::front_matter_block(&src).unwrap_or(""))
            && hs.first().is_some_and(|h| h.0 == 1);
    let skip = usize::from(title_heading_is_first);
    // The page itself: jump to its top; body = everything before the first real section.
    let intro_end = hs.get(skip).map(|h| h.3).unwrap_or(body.len());
    push("", 0, &page_title, &section_text(&body[..intro_end]), "");
    // Each anchored heading: body = text from its close to the next heading's open.
    // `ancestors` is the open heading stack, so the path costs one pop-loop, not a rescan.
    let mut ancestors: Vec<(u8, &str)> = Vec::new();
    for (idx, (level, id, title, _open, close_end)) in hs.iter().enumerate().skip(skip) {
        while ancestors.last().is_some_and(|(l, _)| *l >= *level) {
            ancestors.pop();
        }
        if title.is_empty() {
            continue;
        }
        let path = ancestors
            .iter()
            .map(|(_, t)| *t)
            .collect::<Vec<_>>()
            .join(" > ");
        let sec_end = hs.get(idx + 1).map(|n| n.3).unwrap_or(body.len());
        let sec_body = section_text(body.get(*close_end..sec_end).unwrap_or(""));
        push(id, *level, title, &sec_body, &path);
        ancestors.push((*level, title));
    }
    Some(entries.join(","))
}

/// Render ONE page's markdown with its post-passes finished, exactly as the served page
/// finishes them. Returns `(source, rendered)`, or `None` when the source can't be read.
///
/// **The order is the whole point of this function existing.** `Site::finish_blocks`
/// numbers, then resolves; a scoped render numbers floats and theorems but NOT headings
/// (that is `number_chapter_headings`, a separate step), and only then can the xref
/// registry fill a cross-page `@fig-` that this alone-rendered page left as a bare marker.
/// Getting the order wrong indexes text the page never shows — which is exactly the bug
/// Ship A found, where every heading was indexed unnumbered under a page reading
/// "5.2 How nulls behave". [`super::skim`] needs the identical recipe, so it is written
/// once here rather than copied and left to drift.
pub(super) fn render_finished(
    page: &Page,
    chapter: Option<u32>,
    targets: &HashMap<String, XrefTarget>,
    site_defaults: Option<&render::SiteDefaults>,
) -> Option<(String, render::RenderedDoc)> {
    let src = std::fs::read_to_string(&page.input).ok()?;
    let base = page.input.parent().unwrap_or_else(|| Path::new("."));
    let mut doc = render::render_document_scoped_with_site(&src, base, chapter, site_defaults);
    if let Some(chapter) = chapter {
        super::chapter::number_chapter_headings(&mut doc.blocks, chapter);
    }
    super::xref::resolve_blocks(&mut doc.blocks, targets, &page.url);
    Some((src, doc))
}

/// Scan rendered HTML for `<h1..6 id="…">text</hN>`, returning, per anchored
/// heading, `(level, id, text, open_byte, close_end_byte)` — the byte span lets
/// the caller slice each section's body (heading-close → next heading-open).
pub(super) fn headings_with_pos(html: &str) -> Vec<(u8, String, String, usize, usize)> {
    let mut out = Vec::new();
    let mut pos = 0; // byte offset of `rest` within `html`
    let mut rest = html;
    while let Some(p) = rest.find("<h") {
        pos += p;
        rest = &rest[p..];
        let open_start = pos;
        let level = rest
            .as_bytes()
            .get(2)
            .map(|b| b.wrapping_sub(b'0'))
            .filter(|l| (1..=6).contains(l));
        let Some(level) = level else {
            pos += 2;
            rest = &rest[2..];
            continue;
        };
        let Some(gt) = rest.find('>') else { break };
        let open_tag = &rest[..gt];
        let id = open_tag
            .split_once("id=\"")
            .and_then(|(_, a)| a.split_once('"').map(|(id, _)| id.to_string()));
        let close = format!("</h{level}>");
        let inner = &rest[gt + 1..];
        let Some(end) = inner.find(&close) else {
            pos += gt + 1;
            rest = inner;
            continue;
        };
        let close_end = pos + gt + 1 + end + close.len();
        if let Some(id) = id {
            out.push((
                level,
                id,
                section_text(&inner[..end]),
                open_start,
                close_end,
            ));
        }
        let advance = gt + 1 + end + close.len();
        pos += advance;
        rest = &rest[advance..];
    }
    out
}

/// Plain text from inner HTML. The extraction is [`render::indexable_text`], the same pass
/// `taliesin read` and the TOC/slug path use, so a snippet reads exactly like the page it
/// points at: KaTeX math indexed once (not MathML + raw TeX + glyphs), `&nbsp;` normalized
/// so a reader can search the "Theorem 2.1" they can see, and entities decoded once. Do not
/// re-derive it here.
///
/// **Uncapped, deliberately.** A 1500-character cap used to truncate the body here, which
/// took the tail off 18.7% of the Guide's section records and 25.9% of the Internals' —
/// roughly 15% of each book's prose, silently: no signal to the reader searching for a
/// phrase that is on the page, and none to the author. Uncapping grows the indexed text by
/// only ~1.17x (measured on both books), and `score()` is `indexOf` scans over that text at
/// well under a millisecond per keystroke, so the cap was never buying what it cost.
pub(super) fn section_text(html: &str) -> String {
    render::indexable_text(html)
}

/// Escape a string for a JSON value inlined inside a `<script>` (so `</script>`
/// in content can't break out, and control chars stay valid JSON). Returns the
/// escaped body without surrounding quotes.
pub(super) fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            '<' => out.push_str("\\u003c"), // neutralize a stray </script>
            // U+2028/U+2029 are valid raw in JSON but are line terminators in a pre-ES2019
            // JS string literal; the index is inlined as JS, not `JSON.parse`d, so escape them.
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_text_separates_blocks_decodes_and_collapses() {
        let html = "<p>First.</p><p>Second &amp; third.</p>";
        assert_eq!(section_text(html), "First. Second & third.");
    }

    #[test]
    fn section_text_keeps_a_long_sections_tail() {
        // The old 1500-char cap silently dropped the end of a long section, so a phrase the
        // reader can SEE on the page matched nothing. A distinctive term past the old cap
        // must survive into the index.
        let long = format!("<p>{}needle-past-the-old-cap</p>", "filler ".repeat(400));
        let text = section_text(&long);
        assert!(text.chars().count() > 1500, "no truncation: {}", text.len());
        assert!(
            text.ends_with("needle-past-the-old-cap"),
            "the tail of a long section is indexed"
        );
    }

    #[test]
    fn json_str_neutralizes_script_close_tag() {
        // Both the search and hover indices inline JSON inside a `<script>`; a literal
        // `</script>` in content must not break out. Every `<` is escaped to `<`.
        let out = json_str("</script><script>alert(1)</script>");
        assert!(!out.contains("</script"), "raw </script leaked: {out}");
        assert!(out.contains("\\u003c/script"), "expected escaped <: {out}");
    }

    #[test]
    fn json_str_escapes_line_and_paragraph_separators() {
        // The index is emitted as a JS literal (`window.TALIESIN_SEARCH_INDEX=[…]`), not
        // `JSON.parse`d, so U+2028/U+2029 in prose (valid raw in JSON but a line terminator
        // in a pre-ES2019 JS string literal) must be escaped or the whole index script fails
        // to parse. Both survive as their `\uXXXX` escape, and no raw separator leaks.
        let out = json_str("a\u{2028}b\u{2029}c");
        assert_eq!(out, "a\\u2028b\\u2029c");
        assert!(!out.contains('\u{2028}') && !out.contains('\u{2029}'));
    }

    #[test]
    fn section_text_decodes_nbsp_so_the_visible_number_is_searchable() {
        // A numbered label renders "Theorem&nbsp;2.1" (figure.rs, cell_numbered.rs,
        // cite/render.rs and render/mod.rs all emit the non-breaking space). Indexing the
        // raw entity means a reader typing the number they can SEE matches nothing.
        assert_eq!(
            section_text("<p>Theorem&nbsp;2.1 holds.</p>"),
            "Theorem 2.1 holds."
        );
    }

    #[test]
    fn section_text_indexes_math_once_and_never_leaks_latex() {
        // KaTeX emits every formula three times: the MathML semantic text, a raw-TeX
        // `<annotation>`, then the visible glyphs. Indexing all three triples the math and
        // puts LaTeX source in the index — the exact leak `strip_tags` was made to prevent.
        let html = "<p>Euler: <span class=\"katex\"><span class=\"katex-mathml\"><math>\
                    <semantics><mrow><mi>e</mi></mrow>\
                    <annotation encoding=\"application/x-tex\">e^{i\\pi}</annotation>\
                    </semantics></math></span>\
                    <span class=\"katex-html\" aria-hidden=\"true\">eiπ</span></span>.</p>";
        let text = section_text(html);
        assert!(
            !text.contains("\\pi"),
            "raw LaTeX leaked into the index: {text}"
        );
        assert_eq!(text, "Euler: eiπ .");
    }

    #[test]
    fn section_text_is_quote_aware_about_a_gt_inside_an_attribute() {
        // KaTeX ships `title` attributes containing `>`; a naive `<`/`>` toggle ends the
        // tag early and spills attribute source into the indexed prose.
        assert_eq!(section_text("<p><span title=\"a>b\">x</span></p>"), "x");
    }

    #[test]
    fn section_text_decodes_entities_exactly_once() {
        // Chained `.replace` decodes `&amp;lt;` twice (`&lt;` then `<`). Prose about markup
        // must survive as the text the page shows.
        assert_eq!(
            section_text("<p>&amp;lt; is an entity</p>"),
            "&lt; is an entity"
        );
    }

    #[test]
    fn headings_with_pos_yields_spans_for_full_text_sections() {
        let html = "<h2 id=\"a\">Alpha</h2><p>body of a</p><h3 id=\"b\">Beta</h3><p>body of b</p>";
        let hs = headings_with_pos(html);
        assert_eq!(hs.len(), 2);
        assert_eq!(
            (hs[0].0, hs[0].1.as_str(), hs[0].2.as_str()),
            (2, "a", "Alpha")
        );
        // The span between heading a's close and heading b's open is a's section.
        assert_eq!(section_text(&html[hs[0].4..hs[1].3]), "body of a");
    }
}
