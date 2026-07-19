//! An embedded deck built inside a site with `url:` set must carry its OWN branded
//! OpenGraph card, the same rich social treatment every page beside it gets (PMF C-PUB-1
//! deck residual). Pinned end-to-end against `corpus/embed/` (a page embeds `talk.tmd`):
//! a deck is built off-`Page` via the context-free deck template, so without this wiring
//! it shipped with no `og:image`/`twitter:card` at all — a bare link when shared, the
//! "amateur tell" the audit named, while `index.html` rendered a full card.
//!
//! The pin asserts the deck's card is DISTINCT from the embedding page's (not one
//! site-wide image), and that the card PNG is actually written to the deploy tree.

use std::path::PathBuf;
use std::process::Command;

fn corpus_embed_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("corpus/embed")
}

fn out_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-deck-card-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Pull the `og:image` URL out of a rendered page's head.
fn og_image(html: &str) -> Option<String> {
    let key = r#"property="og:image" content=""#;
    let i = html.find(key)? + key.len();
    let rest = &html[i..];
    Some(rest[..rest.find('"')?].to_string())
}

#[test]
fn an_embedded_deck_gets_its_own_distinct_social_card() {
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

    let talk = std::fs::read_to_string(out.join("talk.html")).expect("deck built");
    let index = std::fs::read_to_string(out.join("index.html")).expect("page built");

    // The deck carries the full rich social meta a page does.
    let deck_img = og_image(&talk).unwrap_or_else(|| panic!("deck must emit og:image:\n{talk}"));
    assert!(
        deck_img.starts_with("https://embed.example.com/og/") && deck_img.ends_with(".png"),
        "deck og:image must be the branded absolute card, got {deck_img}"
    );
    assert!(
        talk.contains(r#"name="twitter:card" content="summary_large_image""#),
        "a deck with a card is summary_large_image, not summary:\n{talk}"
    );
    assert!(
        talk.contains(r#"property="og:url" content="https://embed.example.com/talk.html""#),
        "the deck must emit its absolute og:url:\n{talk}"
    );

    // Not one site-wide card: the deck's card differs from the embedding page's.
    let page_img = og_image(&index).expect("the embedding page has its own card");
    assert_ne!(
        deck_img, page_img,
        "the deck must get its OWN card (distinct hash), not the page's"
    );

    // The card PNG is actually on disk in the deploy tree (og:image doesn't 404).
    let rel = deck_img.trim_start_matches("https://embed.example.com/");
    assert!(
        out.join(rel).exists(),
        "the deck's card PNG must be written to the build output: {rel}"
    );

    let _ = std::fs::remove_dir_all(&out);
}
