//! A `format: revealjs` document dropped loose into a website (not referenced by
//! `{{< embed >}}`) would be silently flattened into a chrome-wrapped article with
//! no slides. Discovery must warn, so the silent failure becomes an actionable hint.

use std::fs;
use std::path::PathBuf;

use qmd_fast_core::Site;

/// A throwaway site directory under the system temp dir (no `tempfile` dev-dep).
fn tmp_site(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qmd-loose-deck-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn loose_deck_in_site_is_warned_not_silently_flattened() {
    let dir = tmp_site("warn");
    fs::write(dir.join("_quarto.yml"), "title: Test Site\n").unwrap();
    fs::write(dir.join("index.qmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
    fs::write(
        dir.join("talk.qmd"),
        "---\ntitle: My Talk\nformat: revealjs\n---\n\n## Slide one\n\n## Slide two\n",
    )
    .unwrap();

    let site = Site::discover(&dir);
    let warned = site
        .warnings
        .iter()
        .any(|w| w.contains("talk.qmd") && (w.contains("embed") || w.contains("deck")));
    let warnings = site.warnings.clone();
    let _ = fs::remove_dir_all(&dir);
    assert!(
        warned,
        "expected a loose-deck warning for talk.qmd, got: {warnings:?}"
    );
}

#[test]
fn embedded_deck_in_site_is_not_warned() {
    let dir = tmp_site("ok");
    fs::write(dir.join("_quarto.yml"), "title: Test Site\n").unwrap();
    fs::write(
        dir.join("index.qmd"),
        "---\ntitle: Home\n---\n\n{{< embed talk.qmd >}}\n",
    )
    .unwrap();
    fs::write(
        dir.join("talk.qmd"),
        "---\ntitle: My Talk\nformat: revealjs\n---\n\n## Slide one\n",
    )
    .unwrap();

    let site = Site::discover(&dir);
    let warned = site.warnings.iter().any(|w| w.contains("talk.qmd"));
    let warnings = site.warnings.clone();
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !warned,
        "an embedded deck must not be flagged as loose, got: {warnings:?}"
    );
}
