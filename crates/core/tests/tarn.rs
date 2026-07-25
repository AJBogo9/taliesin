//! Interaction pin for the "OSS docs maintainer" demand-probe persona (corpus/tarn/).
//! Locks the feature *combinations* the single-feature corpus docs never exercise
//! together: `.panel-tabset`s that lower to ARIA tabs with every panel present in the
//! built HTML (offline-complete + search-indexable), a cross-PAGE guide->reference link
//! that survives `.tmd`->`.html` rewrite, cross-page `@sec-` refs that number by chapter,
//! and a full-text search index that spans the Guide and the Reference including
//! tabset-nested, non-default-tab content. See
//! notes/2026-07-22-corpus-demand-probe-docs-maintainer.md for the findings this produced.

mod common;
use common::corpus_dir;
use taliesin_core::Site;

fn tarn() -> Site {
    Site::discover(&corpus_dir().join("tarn"))
}

#[test]
fn install_page_has_two_tabsets_lowering_to_aria_tabs() {
    let install = tarn().render_page("install.tmd").expect("install renders");
    // Two `.panel-tabset`s on one page (package-manager + per-OS) => two tablists, and
    // three panels each. A page with more than one tabset is pinned nowhere else. Scope to
    // the tabset-specific classes so page chrome (which carries its own role="tablist") does
    // not inflate the count.
    assert!(
        install.contains("tabset-tab") && install.contains("role=\"tab\""),
        "tabsets carry ARIA tab roles: {install}"
    );
    assert_eq!(
        install.matches("class=\"tabset-tablist\"").count(),
        2,
        "two tabsets => two tablists: {install}"
    );
    assert_eq!(
        install.matches("class=\"tabset-panel\"").count(),
        6,
        "two tabsets x three tabs => six panels: {install}"
    );
}

#[test]
fn search_index_spans_guide_and_reference_including_tabset_content() {
    let idx = tarn().search_index_json;
    // The full-text index covers both a Guide page and a Reference page (cross-book search).
    for url in [
        "\"u\":\"install.html\"",
        "\"u\":\"quickstart.html\"",
        "\"u\":\"api-frame.html\"",
    ] {
        assert!(idx.contains(url), "page indexed ({url}): {idx}");
    }
    // Tabset-nested, non-default-tab content is searchable as plain text (its section body
    // carries every panel, not just the visible one): a command from a non-first tab of the
    // install tabsets and the CLI tab of the quickstart tabset all appear.
    for needle in [
        "conda install",
        "scoop install tarn",
        "tarn query sales.csv",
    ] {
        assert!(
            idx.contains(needle),
            "tabset-panel content indexed (`{needle}`): {idx}"
        );
    }
    assert!(
        !idx.contains("</script"),
        "raw </script must be neutralized: {idx}"
    );
}

#[test]
fn quickstart_links_cross_page_into_the_reference() {
    let qs = tarn()
        .render_page("quickstart.tmd")
        .expect("quickstart renders");
    // A Guide page links into Reference pages: `.tmd#anchor` rewrites to `.html#anchor`,
    // even when the link sits inside a `.code-walkthrough` step's prose.
    assert!(
        qs.contains("api-frame.html#fn-filter"),
        "walkthrough step links to the Frame.filter reference entry: {qs}"
    );
    assert!(
        qs.contains("api-query.html#fn-col"),
        "walkthrough step links to the col() reference entry: {qs}"
    );
    // The walkthrough still lowers to line-focused steps around those links.
    assert!(
        qs.contains("data-cw-lines=\"2\""),
        "the code walkthrough keeps its per-step line ranges: {qs}"
    );
}

#[test]
fn cross_page_section_refs_number_by_chapter() {
    let qs = tarn()
        .render_page("quickstart.tmd")
        .expect("quickstart renders");
    // `@sec-install` (a chapter) resolves cross-page to "Chapter 1".
    assert!(
        qs.contains("install.html#sec-install") && qs.contains("Chapter&nbsp;1"),
        "cross-page ref to ch1 resolves to Chapter 1: {qs}"
    );

    let api = tarn()
        .render_page("api-frame.tmd")
        .expect("api-frame renders");
    // `@sec-lazy` (a subsection of ch3) resolves cross-page to the scoped "Section 3.2".
    assert!(
        api.contains("concepts.html#sec-lazy") && api.contains("Section&nbsp;3.2"),
        "cross-page ref to the ch3 subsection resolves to Section 3.2: {api}"
    );

    let query = tarn()
        .render_page("api-query.tmd")
        .expect("api-query renders");
    // The deprecation callout links cross-page into the Frame reference.
    assert!(
        query.contains("api-frame.html#fn-filter"),
        "the deprecation callout links to Frame.filter across pages: {query}"
    );
}

#[test]
fn the_figure_is_hover_indexed_but_section_and_api_headings_are_not() {
    let idx = tarn().hover_index_json;
    // Definitional floats (the figure) enter the hover index; section/API-entry headings
    // do not (they are navigation, not hover-preview targets) -- same contract as demo-book.
    assert!(
        idx.contains("\"fig-dataflow\":\""),
        "the figure is hover-indexed: {idx}"
    );
    assert!(
        !idx.contains("\"fn-filter\":\""),
        "API-entry headings are not hover-indexed: {idx}"
    );
    assert!(
        !idx.contains("\"sec-lazy\":\""),
        "section headings are not hover-indexed: {idx}"
    );
    assert!(
        !idx.contains("</script"),
        "raw </script must be neutralized: {idx}"
    );
}

// --- SKIM-1 pins (the 2026-07-24 skimmability audit) ---------------------------
//
// corpus/tarn was grown to 12 numbered chapters across 3 parts + a nested part so the
// corpus finally pins the shapes the dogfood books have and the corpus did not: chapters
// with a front-matter `title:` (which DEMOTES every body heading one level), a chapter
// rooted deeper than `##`, a chapter below MIN_TOC_HEADINGS, and a part inside a part.
// Every defect below was live on 32 of 32 dogfood chapters and invisible here.

#[test]
fn a_nested_part_keeps_its_chapters_and_is_marked_nested() {
    // `{ part:, chapters: }` inside another one used to delete itself AND its chapters,
    // with `check` still exiting 0. Three chapters sit under "Going further".
    let index = tarn().render_page("index.tmd").expect("index renders");
    for part in ["Guide", "Going further", "Reference", "Appendices"] {
        assert!(
            index.contains(part),
            "part header `{part}` missing: {index}"
        );
    }
    assert!(
        index.contains("tali-book-part-nested"),
        "the nested part must be marked as nested, not flattened into its parent"
    );
    for ch in ["grouping.html", "joins.html", "performance.html"] {
        assert!(
            index.contains(ch),
            "chapter `{ch}` under the nested part must survive: {index}"
        );
    }
}

#[test]
fn a_titled_chapters_headings_number_without_a_spurious_zero() {
    // The regression, on all four chapter shapes at once. A `title:` chapter has its body
    // headings demoted one level; numbering them against a hardcoded `h2` base produced
    // "4.0.1" while the SAME heading's `@sec-` number resolved to "4.1".
    for page in [
        "loading.tmd",   // titled, rooted at `###`
        "filtering.tmd", // titled, rooted at `##`
        "joins.tmd",     // titled, carrying a body `# H1`
        "grouping.tmd",  // titled, two `{.definition}` blocks
    ] {
        let html = tarn().render_page(page).expect("chapter renders");
        let numbers: Vec<&str> = html
            .match_indices("class=\"tali-section-number\">")
            .map(|(i, m)| {
                let rest = &html[i + m.len()..];
                &rest[..rest.find('<').unwrap_or(0)]
            })
            .collect();
        assert!(!numbers.is_empty(), "{page} must emit section numbers");
        for n in &numbers {
            assert!(
                !n.contains(".0"),
                "{page} emitted a spurious-zero section number `{n}` (all: {numbers:?})"
            );
        }
    }
}

#[test]
fn the_rendered_number_the_toc_row_and_the_resolved_ref_agree() {
    // The lockstep the audit asked for: a `@sec-` link must read the number its target
    // heading visibly shows. `@sec-filter-predicate` is a subsection of chapter 5, whose
    // headings are demoted — the exact case where the three numbering sites disagreed.
    let errors = tarn().render_page("errors.tmd").expect("errors renders");
    assert!(
        errors.contains("filtering.html#sec-filter-predicate"),
        "the cross-page ref must resolve: {errors}"
    );
    let resolved = errors
        .split("filtering.html#sec-filter-predicate")
        .nth(1)
        .and_then(|s| s.split('<').next())
        .unwrap_or_default()
        .to_string();
    // Pull the number the heading itself renders on the target page.
    let filtering = tarn()
        .render_page("filtering.tmd")
        .expect("filtering renders");
    let at = filtering
        .find("id=\"sec-filter-predicate\"")
        .expect("the target heading exists");
    let heading_number = filtering[at..]
        .split("class=\"tali-section-number\">")
        .nth(1)
        .and_then(|s| s.split('<').next())
        .expect("the target heading is numbered")
        .to_string();
    assert!(
        resolved.contains(&heading_number),
        "the ref reads `{resolved}` but its target heading reads `{heading_number}`"
    );
}

#[test]
fn a_chapter_below_the_toc_gate_still_gets_the_search_index() {
    // `performance.tmd` has two headings, under MIN_TOC_HEADINGS, so it earns no TOC. The
    // Cmd-K index global used to ride inside the TOC-gated script block while the Cmd-K
    // BUTTON rendered unconditionally: the affordance was advertised and the index absent.
    let perf = tarn()
        .render_page("performance.tmd")
        .expect("performance renders");
    assert!(
        !perf.contains("id=\"TOC\""),
        "performance.tmd is meant to sit below the TOC gate: {perf}"
    );
    assert!(
        perf.contains("tali-search-btn"),
        "the Cmd-K button renders on every book page"
    );
    assert!(
        perf.contains("TALIESIN_SEARCH_URL"),
        "…so the whole-book index must ship with it, TOC or not: {perf}"
    );
}

#[test]
fn the_appendix_is_unnumbered_and_the_definitions_render() {
    let glossary = tarn()
        .render_page("glossary.tmd")
        .expect("glossary renders");
    assert!(
        !glossary.contains("class=\"tali-section-number\">13"),
        "the appendix must not take a chapter number: {glossary}"
    );
    let grouping = tarn()
        .render_page("grouping.tmd")
        .expect("grouping renders");
    // `tali-theorem-definition`, not a bare "definition" — the syntax highlighter also
    // emits `tali-hl-definition`, so a loose substring count would pass vacuously.
    assert_eq!(
        grouping.matches("tali-theorem-definition").count(),
        2,
        "grouping.tmd carries two `{{.definition}}` blocks: {grouping}"
    );
}

// --- SKIM-2 Ship A: the fields the Cmd-K outline groups + labels by ------------------
// The grouping render itself is client JS and is NOT pinned here (it is covered by the
// `web-client` jsconfig type-check plus manual browser verification). What IS pinned is the
// producer contract it keys off: every section record must carry the page url, the anchor,
// the level, the chapter number and its ancestor heading path, or the palette silently falls
// back to a flat list with no way to notice.

/// Every `{…}` object in the built index, as raw JSON text.
fn index_records(idx: &str) -> Vec<&str> {
    idx.split("},{")
        .map(|r| {
            r.trim_start_matches(['[', '{'])
                .trim_end_matches([']', '}'])
        })
        .collect()
}

#[test]
fn every_index_record_carries_the_fields_the_outline_groups_by() {
    let idx = tarn().search_index_json;
    let records = index_records(&idx);
    assert!(
        records.len() > 40,
        "a 12-chapter book should index far more than its chapters: {}",
        records.len()
    );
    for r in &records {
        // `u` groups the rows under a page, `i` is the anchor a row navigates to, `l` is the
        // indent depth. A record missing any of the three cannot be placed in the outline.
        for field in ["\"u\":", "\"i\":", "\"l\":"] {
            assert!(r.contains(field), "record lacks {field}: {r}");
        }
    }
}

#[test]
fn a_numbered_chapters_records_carry_its_number_and_the_heading_path() {
    let idx = tarn().search_index_json;
    let records = index_records(&idx);
    let grouping: Vec<&&str> = records
        .iter()
        .filter(|r| r.contains("\"u\":\"grouping.html\""))
        .collect();
    assert!(
        grouping.len() > 4,
        "grouping.tmd has three sections and two subsections: {grouping:?}"
    );
    // `c` numbers the chapter row. The page-title record carries the BARE title (the rendered
    // section numbers live on headings), so without `c` the outline's chapter rows would be
    // the only unnumbered thing in a numbered book.
    for r in &grouping {
        assert!(r.contains("\"c\":6"), "grouping.tmd is chapter 6: {r}");
    }
    // The indexed heading text carries the number the PAGE shows. Scoping the render numbers
    // floats and theorems but not headings, so this needs `number_chapter_headings` and
    // without it a reader cannot search the "6.2" they can see.
    assert!(
        grouping
            .iter()
            .any(|r| r.contains("\"t\":\"6.2 Aggregates that are not sums\"")),
        "indexed headings carry their rendered section number: {grouping:?}"
    );
    // `h` is the ancestor path: absent on a top-level section (so a flat page's index is
    // byte-identical to before), and on a nested one it names the rendered parent heading.
    let nested: Vec<&&&str> = grouping.iter().filter(|r| r.contains("\"h\":")).collect();
    assert_eq!(
        nested.len(),
        2,
        "exactly the two `###` subsections are nested: {grouping:?}"
    );
    for r in &nested {
        assert!(
            r.contains("\"h\":\"6.2 Aggregates that are not sums\""),
            "an ancestor path names the rendered (numbered) parent heading: {r}"
        );
    }
}

#[test]
fn a_websites_index_carries_no_chapter_number() {
    // `c` is emitted only for a book chapter, so a plain website's records are unchanged.
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    assert!(
        !site.search_index_json.contains("\"c\":"),
        "a website has no chapters, so no record may claim one"
    );
}
