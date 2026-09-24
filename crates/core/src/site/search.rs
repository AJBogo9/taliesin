//! Cross-page **full-text** search index: every page's title + each anchored
//! heading, each carrying the plain-text body of its section so Cmd-K matches
//! prose, not just headings. Built once at discovery; written to `search-index.js`
//! and lazy-loaded by the client on first open (so it never bloats every page).
//! `use super::*` reaches Page + the render entry point.

use super::*;

/// The per-page search fragments (page `rel` → that page's JSON entries, no
/// surrounding brackets), in page order — one `{u,p,i,l,t,b}` object per page title
/// and per anchored heading (`u`rl, `p`age title, anchor `i`d, `l`evel, heading
/// `t`ext, section `b`ody text), for [`assemble`] to join. Renders each page's markdown
/// once (no code execution) so the anchor ids match what the served pages emit.
pub(super) fn build_sections(
    pages: &[Page],
    book: &Option<Book>,
    targets: &HashMap<String, XrefTarget>,
    site_defaults: Option<&render::SiteDefaults>,
) -> Vec<(String, String)> {
    // Renders every page, so it fans out across cores the same way the xref harvest does.
    // Order is kept because the index is served as one concatenated JSON array whose page
    // order is otherwise scheduling-dependent — and a search index that reshuffles between
    // identical builds makes every `_site/search-index.js` diff noise.
    super::fanout::map_ordered(pages, |p| {
        page_fragment(p, super::book::chapter_of(book, p), targets, site_defaults)
            .map(|frag| (p.rel.clone(), frag))
    })
    .into_iter()
    .flatten()
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

impl Site {
    /// The whole Cmd-K index inlined as the script body of `page`, for a page that ships
    /// with no `search-index.js` beside it: `build <file.tmd>`, one self-contained file,
    /// whose project is the one [`Site::discover_document`] builds for the preview, so both
    /// verbs search the same index. It names the page too (`TALIESIN_PAGE_URL`), so a hit
    /// scrolls in place instead of navigating to a url the build may have written under
    /// another name. Empty when the index is.
    pub fn inline_search_index(&self, page: &Page) -> String {
        if self.search_index_json.is_empty() || self.search_index_json == "[]" {
            return String::new();
        }
        format!(
            "window.TALIESIN_PAGE_URL=\"{}\";window.TALIESIN_SEARCH_INDEX={};",
            json_str(&super::feed::percent_encode_path(&page.url)),
            self.search_index_json
        )
    }
}

/// The search-index entries for ONE page as a JSON-array **body** (comma-joined
/// `{u,p,i,l,t,b}` objects, no surrounding brackets). `None` when the page is
/// excluded from search (the author's 404 chrome page) or its source can't be read.
///
/// `chapter` is the page's book chapter number (`Site::chapter_for`), so the indexed text
/// carries the numbers the rendered page shows ("Figure 2.1", not "Figure 1"). Rendering
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
            json_str(&super::feed::percent_encode_path(&page.url)),
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
/// **The order is the whole point of this function existing.** A scoped render numbers the
/// sections, floats and theorems, and only then can the xref registry fill a cross-page
/// `@fig-` that this alone-rendered page left as a bare marker, as `Site::finish_blocks`
/// does. Getting the order wrong indexes text the page never shows: Ship A found every
/// heading indexed unnumbered under a page reading "5.2 How nulls behave", when the
/// numbering was a separate step this skipped.
///
/// The one thing the served page has that a render cannot is what its cells print, since
/// nothing here runs them. Each executed figure's and table's numbered caption is added
/// after its cell ([`executed_captions`]), before the registry resolves refs in it.
pub(super) fn render_finished(
    page: &Page,
    chapter: Option<u32>,
    targets: &HashMap<String, XrefTarget>,
    site_defaults: Option<&render::SiteDefaults>,
) -> Option<(String, render::RenderedDoc)> {
    let src = crate::includes::read_source(&page.input).ok()?;
    let base = page.input.parent().unwrap_or_else(|| Path::new("."));
    let mut doc = render::render_document_scoped_with_site(&src, base, chapter, site_defaults);
    if !render::no_exec_in_force() {
        for b in &mut doc.blocks {
            let captions = executed_captions(b);
            b.html.push_str(&captions);
        }
    }
    super::xref::resolve_blocks(&mut doc.blocks, targets, &page.url);
    Some((src, doc))
}

/// The numbered captions the executor puts under `block`'s cells' output ("Figure 5.2:
/// Variance explained…"), built as `exec.rs` builds them: core's one caption function, then
/// its cross-references marked for the registry to resolve. A caption is written in the
/// source and its number is reserved at render, so it is known without running the cell;
/// the output itself is not, and is not indexed. Only a cell whose output the page keeps
/// carries a `figure`/`table` (the render leaves both unset for `include: false`), and
/// `--no-exec` shows none of them.
fn executed_captions(block: &render::Block) -> String {
    block
        .cells()
        .filter_map(|c| {
            let (label, number, caption) = match (&c.figure, &c.table) {
                (Some(f), _) => ("Figure", &f.number, &f.caption),
                (None, Some(t)) => ("Table", &t.number, &t.caption),
                (None, None) => return None,
            };
            Some(format!(
                "<figcaption>{}</figcaption>",
                crate::cite::link_xrefs_in_fragment(&render::numbered_caption(
                    label,
                    number,
                    caption.as_deref(),
                ))
            ))
        })
        .collect()
}

/// Scan rendered HTML for `<h1..6 id="…">text</hN>`, returning, per anchored
/// heading, `(level, id, text, open_byte, close_end_byte)` — the byte span lets
/// the caller slice each section's body (heading-close → next heading-open).
///
/// The headings are the ones the one walker ([`render::tags`]) finds, so a heading is an
/// element on the page: markup inside a `<!-- comment -->` or a `<script>` body is not
/// one. A bare `find("<h")` took both for headings, and the palette offered results
/// pointing at ids the page does not carry. The id is read through the walker too:
/// quote-aware, matched as a whole NAME, and decoded, so the index carries the id the
/// browser resolves (`r&d-notes`, not the `r&amp;d-notes` a needle cut out of the markup).
pub(super) fn headings_with_pos(html: &str) -> Vec<(u8, String, String, usize, usize)> {
    let mut out = Vec::new();
    // Where the last heading closed: a tag before this sits inside that heading.
    let mut done = 0;
    for open in render::tags(html) {
        let level = match open.name.as_bytes() {
            [b'h' | b'H', l @ b'1'..=b'6'] => l - b'0',
            _ => continue,
        };
        if open.at < done {
            continue;
        }
        let inner_start = open.at + open.text.len();
        let close = format!("</h{level}>");
        let Some(end) = html[inner_start..].find(&close) else {
            continue;
        };
        let close_end = inner_start + end + close.len();
        done = close_end;
        if let Some(id) = render::attr_value(&open, "id") {
            out.push((
                level,
                id.into_owned(),
                render::heading_text(&html[inner_start..inner_start + end]),
                open.at,
                close_end,
            ));
        }
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
/// only ~1.17x (measured on both books), so the cap was never buying what it cost. What a
/// keystroke costs is the typo tier's pass over each record's words, not the `indexOf`
/// scans: `score()` over the Guide's 130 records took about 4 ms per keystroke re-splitting
/// every body, and about 2 ms once the words are split once per load (node, 2026-09-24).
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
        assert_eq!(text, "Euler: eiπ.");
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

    /// Code is searchable as the reader sees it. A space went in at every tag, and syntax
    /// highlighting wraps each token in a `<span>`, so the index held `matplotlib . pyplot`
    /// and `np . linspace ( 0 , 10 )`: typing `plt.show()` or `np.linspace` found nothing
    /// on any page. Inline code in prose read `( exec.rs )` the same way.
    #[test]
    fn code_is_indexed_with_its_tokens_joined() {
        let doc = crate::render::render_document(
            "Run it (`exec.rs`) now.\n\n```python\nimport matplotlib.pyplot as plt\n\
             bins = np.linspace(0, 10, 12)\nplt.show()\n```\n\nAfter.\n",
        );
        let html: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
        assert!(
            html.contains("<span"),
            "sanity: the fence is highlighted: {html}"
        );
        let text = section_text(&html);
        for needle in [
            "(exec.rs) now.",
            "import matplotlib.pyplot as plt",
            "np.linspace(0, 10, 12)",
            "plt.show()",
        ] {
            assert!(
                text.contains(needle),
                "{needle:?} is not searchable: {text}"
            );
        }
        // A code block is still its own block: its first and last words do not weld onto
        // the prose around it.
        assert!(text.contains("now. import") && text.ends_with("plt.show() After."));
    }

    /// A result's title is the heading's text as the TOC shows it, one extractor for both:
    /// `(exec.rs)`, not `( exec.rs )`, and inline math once, as its glyphs, unsplit.
    #[test]
    fn a_result_title_reads_like_its_toc_entry() {
        let doc = crate::render::render_document("## The executor (`exec.rs`) under $H_0$ {#ex}\n");
        let html: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
        let hs = headings_with_pos(&html);
        assert_eq!(hs.len(), 1, "{html}");
        // KaTeX closes the glyphs with an invisible U+200B, which the TOC carries too.
        assert_eq!(
            hs[0].2.trim_end_matches('\u{200b}'),
            "The executor (exec.rs) under H0"
        );
    }

    /// A diagram's source is not on the page: mermaid.js replaces the `<pre>` with the
    /// drawing. Indexed, it put `flowchart LR BR["Browser preview<br/>…` into snippets.
    /// The caption is the diagram's text the reader sees, so it stays searchable.
    #[test]
    fn mermaid_source_is_not_indexed_but_its_caption_is() {
        let doc = crate::render::render_document(
            "Before.\n\n```{mermaid}\nflowchart LR\n  Alpha --> Beta\n```\n\n\
             ```{mermaid}\n%%| label: fig-flow\n%%| fig-cap: The flowcaption.\n\
             sequenceDiagram\n  A->>B: hi\n```\n\nAfter.\n",
        );
        let html: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
        assert!(html.matches("class=\"mermaid\"").count() == 2, "{html}");
        let text = section_text(&html);
        for gone in ["flowchart", "Alpha", "Beta", "sequenceDiagram", "hi"] {
            assert!(
                !text.contains(gone),
                "diagram source {gone:?} indexed: {text}"
            );
        }
        assert!(
            text.starts_with("Before. Figure 1") && text.ends_with("The flowcaption. After."),
            "{text}"
        );
    }

    /// A commented-out heading is not on the page, so it is not a result. It was found by a
    /// bare `find("<h")`, which cannot tell a comment from markup: the palette offered
    /// "Old section title" pointing at an id no element carries, and it took the visible
    /// prose after the comment away from the real section. An apostrophe in a comment
    /// used to empty the rest of its section the same way.
    #[test]
    fn a_commented_out_heading_is_not_a_result() {
        let html = "<h2 id=\"a\">A</h2><!-- TODO: don't forget -->\
                    <p>Kept.</p><!--\n<h2 id=\"old\">Old section title</h2>\n\
                    <p>Old paragraph.</p>\n--><p>Current prose.</p>\
                    <h2 id=\"b\">B</h2><p>b</p>";
        let hs = headings_with_pos(html);
        let ids: Vec<&str> = hs.iter().map(|h| h.1.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
        assert_eq!(
            section_text(&html[hs[0].4..hs[1].3]),
            "Kept. Current prose."
        );
    }

    /// Heading markup inside a `<script>` body is a JavaScript string, not a heading. Read
    /// as one, it filed a phantom result under an id the static page lacks, gave it the
    /// script's own source as its text, and took the prose after the script with it.
    #[test]
    fn heading_markup_inside_a_script_is_not_a_result() {
        let html = "<h2 id=\"real\">Real</h2><p>Real prose.</p><div id=\"app\"></div>\
                    <script>const tpl = '<h2 id=\"phantom\">Phantom</h2><p>body</p>';\
                    document.getElementById(\"app\").innerHTML = tpl;</script>\
                    <p>After the script.</p><h2 id=\"second\">Second</h2><p>2</p>";
        let hs = headings_with_pos(html);
        let ids: Vec<&str> = hs.iter().map(|h| h.1.as_str()).collect();
        assert_eq!(ids, ["real", "second"]);
        assert_eq!(
            section_text(&html[hs[0].4..hs[1].3]),
            "Real prose. After the script."
        );
    }

    /// The id a hit navigates to is the id the heading carries, read the way the browser
    /// reads it. It was cut out of the open tag with `split_once("id=\"")`, so an explicit
    /// `{#r&d-notes}` went into the index as `r&amp;d-notes` (a hit landed nowhere, the
    /// palette just closed), and a `data-block-id` written before the real `id` was read in
    /// its place.
    #[test]
    fn a_heading_id_is_read_decoded_and_as_a_whole_attribute_name() {
        let html = "<h2 id=\"r&amp;d-notes\">R&amp;D notes</h2><p>zebra</p>\
                    <h2 class=\"x\" data-block-id=\"no\" id=\"yes\">Other</h2><p>y</p>";
        let ids: Vec<String> = headings_with_pos(html).into_iter().map(|h| h.1).collect();
        assert_eq!(ids, vec!["r&d-notes", "yes"]);
    }
}
