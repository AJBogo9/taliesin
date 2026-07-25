//! A figure labelled by a CELL directive (`%%| label:` / `#| label:`) must be a
//! cross-PAGE reference target, exactly like a `{#fig-x}` brace id.
//!
//! The two anchor forms reach the registry by different routes: a brace id is found
//! by the lightweight source scan (`site::xref::scan_page_anchors`), while a cell
//! label only exists after a render, so it arrives via `Site::harvest_xref_numbers`.
//! That harvest used to *enrich* numbers only, so a cell label was never inserted and
//! a cross-page `@fig-` to one silently rendered a bare "Figure" pointing at a dead
//! same-page `#fig-x`. Same-page refs resolved, which is why nothing caught it.
//!
//! Exercised through the real `demo-book`: `results.tmd` (chapter 3) defines
//! `fig-stages` with a `{mermaid}` cell and refers to it itself; `summary.tmd`
//! (chapter 4) refers to it across the page boundary.

use taliesin_core::Site;

mod common;
use common::{TempProj, corpus_dir};

#[test]
fn cross_page_ref_to_a_cell_labelled_figure_resolves_to_its_page_and_number() {
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let summary = site.render_page("summary.tmd").expect("summary renders");
    assert!(
        summary
            .contains(r#"<a href="results.html#fig-stages" class="tali-xref">Figure&nbsp;3.1</a>"#),
        "a cross-page @fig- to a cell-labelled figure should link to its defining page \
         with its chapter-scoped number; got:\n{}",
        summary
            .lines()
            .filter(|l| l.contains("fig-stages"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn a_resolved_cell_label_leaves_no_broken_xref_marker() {
    // `cite` emits `data-tali-xref` for a target it can't see on the current page; the
    // site rewrite consumes the marker. A leftover marker means the ref stayed broken.
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let summary = site.render_page("summary.tmd").expect("summary renders");
    assert!(
        !summary.contains(r#"data-tali-xref="fig-stages""#),
        "fig-stages still carries the unresolved cross-page marker"
    );
}

#[test]
fn the_same_cell_label_on_two_pages_warns_like_a_duplicate_brace_id() {
    // Parity with the source-scan's own duplicate check. Making cell labels resolve
    // must not make a duplicated one resolve *silently*: before they resolved at all,
    // a duplicate at least surfaced as "broken cross-reference", so staying quiet here
    // would trade one wrong answer for no answer. First definition wins, as for a
    // brace id, and the loser is reported.
    let proj = TempProj::new();
    proj.file("_site.yml", "title: \"Dup\"\n")
        .file(
            "a.tmd",
            "---\ntitle: \"A\"\n---\n```{mermaid}\n%%| label: fig-dup\n%%| fig-cap: \"first\"\ngraph TD\n  A --> B\n```\n",
        )
        .file(
            "b.tmd",
            "---\ntitle: \"B\"\n---\n```{mermaid}\n%%| label: fig-dup\n%%| fig-cap: \"second\"\ngraph TD\n  C --> D\n```\n",
        );
    let site = Site::discover(&proj.0);
    assert!(
        site.warnings
            .iter()
            .any(|w| w.contains("duplicate cross-reference label")
                && w.contains("fig-dup")
                && w.contains("a.html")),
        "a cell label defined on two pages should warn and keep the first, got: {:?}",
        site.warnings
    );
}

#[test]
fn a_source_anchor_duplicated_across_pages_warns_with_a_located_line() {
    // The source-scan duplicate used to carry no location (backlog item 5). It is now a
    // `file:line:` linter line at the redefining anchor, and still names the winning page.
    let proj = TempProj::new();
    proj.file("_site.yml", "title: \"Dup\"\n")
        .file("a.tmd", "---\ntitle: \"A\"\n---\n\n## First {#sec-dup}\n")
        .file(
            "b.tmd",
            "---\ntitle: \"B\"\n---\n\nintro\n\n## Second {#sec-dup}\n",
        );
    let site = Site::discover(&proj.0);
    let w = site
        .warnings
        .iter()
        .find(|w| w.contains("duplicate cross-reference label") && w.contains("sec-dup"))
        .unwrap_or_else(|| panic!("expected a dup warning, got: {:?}", site.warnings));
    // A `file.tmd:line:` located prefix (the fix), plus the winning page named for context.
    let (file, rest) = w.split_once(':').expect("located file:line: prefix");
    assert!(file.ends_with(".tmd"), "located at a source file: {w}");
    assert!(
        rest.chars()
            .take_while(|c| *c != ':')
            .all(|c| c.is_ascii_digit()),
        "a line number follows the file: {w}"
    );
    assert!(w.contains(".html"), "names the winning page: {w}");
}

#[test]
fn an_id_that_is_not_a_ref_anchor_never_becomes_a_cross_page_target() {
    // The render registry is looser than the source scan: the Markdown-table caption
    // path registers ANY id (`render/mod.rs`, unlike the figure path's `fig-` filter),
    // so `: caption {#my-table}` lands in `xref_numbers`. The scan filtered such ids out
    // with `is_ref_anchor`; the insert path must too, or a `{#my-table}` that `@`-refs
    // can never reach (cite rejects an unknown prefix) would still be advertised as a
    // resolvable target by `taliesin map --format json` and given a hover card.
    let proj = TempProj::new();
    proj.file("_site.yml", "title: \"Leak\"\n").file(
        "a.tmd",
        "---\ntitle: \"A\"\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n: My caption {#my-table}\n",
    );
    let site = Site::discover(&proj.0);
    assert!(
        !site.xref_targets.contains_key("my-table"),
        "`my-table` is not a cross-reference anchor and must not be a target; got: {:?}",
        site.xref_targets.keys().collect::<Vec<_>>()
    );
}

#[test]
fn a_table_labelled_tbl_is_still_a_cross_page_target() {
    // The guard above must not cost the legitimate case: `{#tbl-x}` IS a ref anchor.
    let proj = TempProj::new();
    proj.file("_site.yml", "title: \"Tbl\"\n").file(
        "a.tmd",
        "---\ntitle: \"A\"\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n: My caption {#tbl-x}\n",
    );
    let site = Site::discover(&proj.0);
    assert!(
        site.xref_targets.contains_key("tbl-x"),
        "`tbl-x` is a ref anchor and must stay a target; got: {:?}",
        site.xref_targets.keys().collect::<Vec<_>>()
    );
}

#[test]
fn a_duplicate_label_is_reported_exactly_once_for_either_anchor_shape() {
    // The harvest's dedup string-matches the scan's warning text. If that text is ever
    // reworded, the dedup silently stops matching and every brace-id duplicate warns
    // TWICE (scan + harvest). `.any()` cannot see that; only a count can.
    let proj = TempProj::new();
    proj.file("_site.yml", "title: \"Dup\"\n")
        // brace-id duplicate: the SCAN reports it; the harvest must stay quiet.
        .file("d.tmd", "---\ntitle: \"D\"\n---\n![x](f.svg){#fig-bdup}\n")
        .file("e.tmd", "---\ntitle: \"E\"\n---\n![y](f.svg){#fig-bdup}\n")
        // cell-label duplicate: the scan cannot see it; the harvest must report it.
        .file(
            "a.tmd",
            "---\ntitle: \"A\"\n---\n```{mermaid}\n%%| label: fig-cdup\n%%| fig-cap: \"first\"\ngraph TD\n  A --> B\n```\n",
        )
        .file(
            "b.tmd",
            "---\ntitle: \"B\"\n---\n```{mermaid}\n%%| label: fig-cdup\n%%| fig-cap: \"second\"\ngraph TD\n  C --> D\n```\n",
        );
    let site = Site::discover(&proj.0);
    for anchor in ["fig-bdup", "fig-cdup"] {
        let quoted = format!("\u{201c}{anchor}\u{201d}");
        let n = site
            .warnings
            .iter()
            .filter(|w| w.contains("duplicate cross-reference label") && w.contains(&quoted))
            .count();
        assert_eq!(
            n, 1,
            "{anchor} should be reported exactly once, got {n}: {:?}",
            site.warnings
        );
    }
}

#[test]
fn a_duplicates_number_comes_from_the_page_the_link_points_at() {
    // Mixed-form duplicate: page `a` labels `fig-x` by cell, page `b` by brace id. The
    // scan sees only b's, so the target url is b.html. The number must then come from b
    // too — harvesting a's would render "Figure 2" on a link to b.html#fig-x, where the
    // figure is captioned "Figure 1", and would contradict the warning's own "using
    // b.html". a's fig-x is its SECOND figure, so the two numbers differ observably.
    let proj = TempProj::new();
    proj.file("_site.yml", "title: \"Mixed\"\n")
        .file(
            "a.tmd",
            "---\ntitle: \"A\"\n---\n![first](f.svg){#fig-other}\n\n```{mermaid}\n%%| label: fig-x\n%%| fig-cap: \"second on this page\"\ngraph TD\n  A --> B\n```\n",
        )
        .file("b.tmd", "---\ntitle: \"B\"\n---\n![only](f.svg){#fig-x}\n")
        .file("c.tmd", "---\ntitle: \"C\"\n---\nSee @fig-x.\n");
    let site = Site::discover(&proj.0);
    let t = site.xref_targets.get("fig-x").expect("fig-x is a target");
    assert_eq!(
        (t.url.as_str(), t.number.as_str()),
        ("b.html", "1"),
        "the number must come from the page the url points at, not the losing duplicate"
    );
}

#[test]
fn a_cell_labelled_figure_is_still_a_same_page_ref() {
    // The defining page keeps a plain same-page link (no rewrite to itself), so the
    // fix must not turn a same-page ref into a cross-page one.
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let results = site.render_page("results.tmd").expect("results renders");
    assert!(
        results.contains(r##"href="#fig-stages""##),
        "the defining page should link to fig-stages on itself"
    );
    assert!(
        !results.contains(r#"href="results.html#fig-stages""#),
        "a same-page ref must not be rewritten to a cross-page link"
    );
}
