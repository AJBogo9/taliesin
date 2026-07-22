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
