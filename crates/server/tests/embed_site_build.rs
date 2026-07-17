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
