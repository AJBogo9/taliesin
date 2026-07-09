//! `taliesin check` superset validators: each renders a real doc and asserts the
//! static lint surfaces (or stays silent on) the right located warning.

use std::path::Path;
use taliesin_core::diagnostics;

#[test]
fn duplicate_explicit_heading_id_is_deduped_at_render_time() {
    // The renderer now routes explicit `{#id}`s through the same dedup as auto-slugs
    // (fix(core) "explicit heading id dedup"): two `{#dup}` headings emit distinct DOM
    // ids (`dup`, `dup-1`) so anchors/TOC/xrefs resolve, and the render emits its own
    // located "duplicate heading id" warning. The post-hoc DOM scan
    // (`validate_duplicate_heading_ids`) therefore finds no surviving collision — the
    // bug is fixed upstream, not merely detected. (That validator stays as a belt-and-
    // suspenders guard for any future id source that bypasses the renderer's dedup.)
    let src = "---\ntitle: T\n---\n\n## First {#dup}\n\nText.\n\n## Second {#dup}\n\nMore.\n";
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));

    // Distinct ids in the rendered DOM.
    let ids: Vec<&str> = doc
        .blocks
        .iter()
        .filter_map(|b| {
            let h = &b.html;
            (h.starts_with("<h") && h.as_bytes().get(2).is_some_and(u8::is_ascii_digit))
                .then(|| {
                    let head = &h[..h.find('>').unwrap_or(h.len())];
                    let i = head.find(" id=\"")? + 5;
                    let rest = &head[i..];
                    Some(&rest[..rest.find('"')?])
                })
                .flatten()
        })
        .collect();
    assert_eq!(
        ids,
        vec!["dup", "dup-1"],
        "explicit ids must be deduped: {ids:?}"
    );

    // The renderer emits a located duplicate-id warning.
    let render_warn = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("duplicate heading id"))
        .expect("render emits a duplicate-id warning");
    assert!(
        render_warn.message.contains("dup"),
        "names the id: {render_warn:?}"
    );
    assert!(render_warn.line.is_some(), "located: {render_warn:?}");

    // The DOM scan sees no surviving duplicate (the renderer already resolved it).
    assert!(
        diagnostics::validate_duplicate_heading_ids(&doc.blocks).is_empty(),
        "render-time dedup leaves no duplicate id for the DOM scan to find"
    );
}

#[test]
fn unique_heading_ids_are_clean() {
    let src = "---\ntitle: T\n---\n\n## First {#a}\n\n## Second {#b}\n\n## Auto heading here\n";
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));
    assert!(
        diagnostics::validate_duplicate_heading_ids(&doc.blocks).is_empty(),
        "distinct ids must not warn"
    );
}

#[test]
fn missing_local_image_is_flagged_existing_is_clean() {
    let dir = std::env::temp_dir().join("tali-check-assets-missing-img");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("real.png"), b"\x89PNG").unwrap();
    let src = "---\ntitle: T\n---\n\n![ok](real.png)\n\n![missing](gone.png)\n";
    let doc = taliesin_core::render_document_with_includes(src, &dir);
    let warns = diagnostics::validate_local_assets(&doc.blocks, &dir);
    assert_eq!(warns.len(), 1, "only the missing asset warns: {warns:?}");
    assert!(
        warns[0].message.contains("gone.png"),
        "names the missing path: {:?}",
        warns[0]
    );
    assert!(warns[0].line.is_some(), "located: {:?}", warns[0]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn citation_without_bibliography_is_flagged() {
    let src = "---\ntitle: T\n---\n\nAs shown [@smith2020], it works.\n";
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));
    let warns = diagnostics::citations_without_bibliography(src, &doc.blocks);
    assert_eq!(warns.len(), 1, "cited with no bibliography: {warns:?}");
    assert!(
        warns[0].message.contains("bibliography"),
        "names the cause: {:?}",
        warns[0]
    );
}

#[test]
fn citation_with_bibliography_key_declared_is_clean() {
    // The `bibliography:` key is present (a missing FILE is a separate warning).
    let src = "---\ntitle: T\nbibliography: refs.bib\n---\n\nAs shown [@smith2020].\n";
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));
    assert!(
        diagnostics::citations_without_bibliography(src, &doc.blocks).is_empty(),
        "a declared bibliography must not trip this check"
    );
}

#[test]
fn no_citations_is_clean_for_bibliography_check() {
    let src = "---\ntitle: T\n---\n\nNo citations here at all.\n";
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));
    assert!(
        diagnostics::citations_without_bibliography(src, &doc.blocks).is_empty(),
        "no citations -> no warning"
    );
}

#[test]
fn broken_internal_anchor_is_flagged() {
    let src = "---\ntitle: T\n---\n\n## Section One {#sec-one}\n\nJump to [good](#sec-one) or [bad](#nope).\n";
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));
    let warns = diagnostics::validate_internal_anchors(&doc.blocks);
    assert_eq!(warns.len(), 1, "only the broken anchor warns: {warns:?}");
    assert!(
        warns[0].message.contains("nope"),
        "names the fragment: {:?}",
        warns[0]
    );
}

#[test]
fn valid_internal_anchor_and_cross_page_href_are_clean() {
    // #sec-one resolves; the cross-page `other.html#x` is out of scope for this check.
    let src = "---\ntitle: T\n---\n\n## Section One {#sec-one}\n\n[ok](#sec-one) and [page](other.html#x).\n";
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));
    assert!(
        diagnostics::validate_internal_anchors(&doc.blocks).is_empty(),
        "valid + cross-page anchors must not warn"
    );
}

#[test]
fn manual_anchor_is_not_flagged_when_doc_has_executable_cells() {
    // A {python} cell can emit `id="results"` at runtime (e.g. HTML('<div id="results">')).
    // Static check never runs cells, so a doc with executable cells must not flag manual
    // anchors — the id may exist in the built/served page. (No false positive.)
    let src = "---\ntitle: T\n---\n\nSee [results](#results) below.\n\n```{python}\nfrom IPython.display import HTML\nHTML('<div id=\"results\">ok</div>')\n```\n";
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));
    assert!(
        diagnostics::validate_internal_anchors(&doc.blocks).is_empty(),
        "docs with executable cells must not flag manual anchors (ids may be cell-emitted)"
    );
}

#[test]
fn xref_placeholder_anchor_is_not_flagged_as_broken_internal_link() {
    // `@sec-elsewhere` lowers to href="#sec-elsewhere" data-qmd-xref="sec-elsewhere"; it is
    // an xref (validate_xrefs' job + resolved cross-page by the site layer), not a manual
    // in-page link, so the anchor check must skip it — no double-flag.
    let src = "---\ntitle: T\n---\n\nSee @sec-elsewhere for details.\n";
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));
    assert!(
        diagnostics::validate_internal_anchors(&doc.blocks).is_empty(),
        "xref placeholder anchors must not be flagged as broken internal links"
    );
}

#[test]
fn corpus_check_superset_doc_trips_each_validator() {
    // The canonical corpus pin: one diagnostics doc that fires every new static check.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics");
    let src = std::fs::read_to_string(dir.join("check-superset.tmd")).unwrap();
    let doc = taliesin_core::render_document_with_includes(&src, &dir);
    // The duplicate `{#dup}` heading id is now resolved at render time (explicit ids go
    // through the same dedup as auto-slugs), so the diagnostic arrives on the render
    // `warnings` channel (which `taliesin check` already aggregates) rather than from
    // the post-hoc DOM scan. Coverage is unchanged: the duplicate is still reported.
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("duplicate heading id")),
        "duplicate {{#dup}} heading id (render-time warning): {:?}",
        doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
    let anchors = diagnostics::validate_internal_anchors(&doc.blocks);
    assert!(
        anchors.iter().any(|w| w.message.contains("no-such-anchor")),
        "broken in-page anchor: {anchors:?}"
    );
    let assets = diagnostics::validate_local_assets(&doc.blocks, &dir);
    assert!(
        assets
            .iter()
            .any(|w| w.message.contains("no-such-image.png")),
        "missing image: {assets:?}"
    );
    assert_eq!(
        diagnostics::citations_without_bibliography(&src, &doc.blocks).len(),
        1,
        "citation with no bibliography"
    );
    let math = diagnostics::validate_math(&doc.blocks);
    assert!(
        math.iter()
            .any(|w| w.message.contains("math failed to render")),
        "unparseable inline math (server-side KaTeX render diagnostic): {math:?}"
    );
    let langs = diagnostics::validate_code_languages(&doc.blocks);
    assert!(
        langs
            .iter()
            .any(|w| w.message.contains("unknown code language `pyton`")),
        "typo'd fence language: {langs:?}"
    );
}

#[test]
fn audio_video_src_is_skipped_only_img_is_checked() {
    // Load-bearing scoping: <audio>/<video>/<source> refs are frequently code-generated or
    // streamed (the corpus has fourier-transform's cell-written .wav, supercollider's .mp3),
    // so a static check must skip them; only a missing <img> is flagged.
    let dir = std::env::temp_dir().join("tali-check-assets-av");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = "---\ntitle: T\n---\n\n```{=html}\n<audio><source src=\"gone.wav\"></audio>\n<video src=\"gone.mp4\"></video>\n<img src=\"gone.png\">\n```\n";
    let doc = taliesin_core::render_document_with_includes(src, &dir);
    let warns = diagnostics::validate_local_assets(&doc.blocks, &dir);
    assert_eq!(
        warns.len(),
        1,
        "only the <img> is checked, not audio/video: {warns:?}"
    );
    assert!(
        warns[0].message.contains("gone.png"),
        "the image, not the av: {:?}",
        warns[0]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn external_and_anchor_refs_are_not_treated_as_local_assets() {
    let dir = std::env::temp_dir().join("tali-check-assets-external");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = "---\ntitle: T\n---\n\n![remote](https://example.com/x.png)\n\n[jump](#sec)\n\n[mail](mailto:a@b.c)\n";
    let doc = taliesin_core::render_document_with_includes(src, &dir);
    assert!(
        diagnostics::validate_local_assets(&doc.blocks, &dir).is_empty(),
        "external/anchor/mailto refs must not be checked for local existence"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
