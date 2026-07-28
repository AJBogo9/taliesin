//! The load-bearing `{{< embed >}}` path, pinned end-to-end against the corpus site
//! `corpus/embed/`: a page embeds a deck, and a SITE build must discover the embed
//! target and build the deck as a standalone `.html` beside the page (so the iframe
//! doesn't 404), while keeping that deck OUT of the site nav (it's a component of the
//! embedding page, not an independently published page). The embedding page's iframe
//! must resolve to the built deck.
//!
//! The single-doc counterpart (an embed in a one-file build *warns* because it can't
//! build the target) lives in `embed_warning.rs`; this is the site half.

use std::path::PathBuf;
use std::process::Command;

fn corpus_embed_dir() -> PathBuf {
    // crates/server/tests/ -> repo root -> corpus/embed
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("corpus/embed")
}

fn out_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-embed-site-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn site_build_builds_embedded_deck_beside_the_page_and_keeps_it_out_of_nav() {
    let out = out_dir();
    let result = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(corpus_embed_dir())
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run site build");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "site build failed:\n{stderr}");
    // The embed must resolve: no "embedded deck not found" warning.
    assert!(
        !stderr.contains("embedded deck not found"),
        "the embed target should resolve in a site build, stderr:\n{stderr}"
    );

    let index = std::fs::read_to_string(out.join("index.html")).expect("index.html built");
    let talk = std::fs::read_to_string(out.join("talk.html"))
        .expect("the embedded deck must be built beside the page (else the iframe 404s)");
    let _ = std::fs::remove_dir_all(&out);

    // 1. The embedding page carries the iframe, pointing at the BUILT deck (`.tmd`->`.html`).
    assert!(
        index.contains("class=\"tali-embed-frame\" src=\"talk.html\""),
        "the page's iframe must resolve to the built deck, index.html:\n{index}"
    );
    // 2. The target is built as a standalone DECK...
    assert!(
        talk.contains("class=\"tali-deck\""),
        "the embed target must be built as a deck, talk.html:\n{talk}"
    );
    // 3. ...WITHOUT the site nav chrome (it's a component, not a nav page), while the
    //    embedding page DOES carry the nav. That is what "kept out of nav" means on disk.
    assert!(
        index.contains("tali-site-nav"),
        "a normal page carries the site nav"
    );
    assert!(
        !talk.contains("tali-site-nav"),
        "the embedded deck must be a standalone deck, not a chaptered nav page, talk.html:\n{talk}"
    );
}

/// A deck's body links are author `.tmd` references and must be rewritten to `.html`
/// exactly like every other page's, because the build treats a surviving `.tmd` href as
/// a request to *publish that source file*: `.tmd` is in `SKIP_EXT`, so
/// `deploy_referenced_sources` copies any referenced one into the deploy tree. An
/// unrewritten link therefore turns "link to my page" into "publish my markdown", and it
/// does so even for a `draft:` page that the HTML build correctly refused to emit.
///
/// The deck path is the one page path that never entered `Site::render_page_doc_*`, where
/// the rewrite lives, so it inherited nothing. A live `preview` masks the whole thing: it
/// serves the *rendered* page for `/<page>.tmd`, so the link looks fine right up until the
/// static build turns it into a raw-source download.
#[test]
fn an_embedded_decks_tmd_links_are_rewritten_so_no_page_source_is_published() {
    let dir = std::env::temp_dir().join(format!("tali-deck-links-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("fixture dir");
    std::fs::write(dir.join("_site.yml"), "title: \"Deck links\"\n").unwrap();
    std::fs::write(
        dir.join("index.tmd"),
        "---\ntitle: \"Home\"\n---\n\nHome. {{< embed slides.tmd >}}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("unpublished.tmd"),
        "---\ntitle: \"Unpublished\"\ndraft: true\n---\n\nCONFIDENTIAL_MARKER_XYZ\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("slides.tmd"),
        "---\ntitle: \"Slides\"\nformat: deck\n---\n\n## One\n\nSee [the draft](unpublished.tmd).\n",
    )
    .unwrap();

    let out = dir.join("_out");
    let result = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run site build");
    assert!(
        result.status.success(),
        "site build failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let slides = std::fs::read_to_string(out.join("slides.html")).expect("deck built");
    let leaked: Vec<_> = std::fs::read_dir(&out)
        .expect("read out dir")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|n| n.ends_with(".tmd"))
        .collect();
    let draft_html = out.join("unpublished.html").exists();
    let _ = std::fs::remove_dir_all(&dir);

    // The deck's author link is rewritten, like every other page's.
    assert!(
        !slides.contains(".tmd\""),
        "a deck's `.tmd` hrefs must be rewritten to `.html`, slides.html:\n{slides}"
    );
    // The draft stays unpublished in BOTH forms: no rendered page, and no source.
    assert!(
        !draft_html,
        "a `draft:` page must not be built (that part already worked)"
    );
    assert!(
        leaked.is_empty(),
        "no page source may be published into the deploy tree, found: {leaked:?}"
    );
}
