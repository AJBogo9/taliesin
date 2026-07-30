//! A deck inside a site must reach the static validators (item 109, 2026-07-28).
//!
//! An `{{< embed >}}`-referenced deck is built and served but deliberately removed from
//! `site.pages` so it stays out of nav and search. Every static validator walked
//! `site.pages`, so the deck reached **none** of them: measured on the fixture below,
//! `check <site>` printed "no problems found" and exited 0, `build --strict` exited 0 and
//! shipped the defects, while `check <the deck>.tmd` reported all of them. No test anywhere
//! ran a validator against a deck in a site — `check_superset.rs` and `check_cli.rs`
//! contained zero `deck`/`Reveal` — which is how three mechanisms could all stop at the same
//! boundary unnoticed.
//!
//! The asymmetry is worth more than its severity suggests: every other defect class in this
//! tool is caught in the 90 ms edit loop or by `check`, while a deck's defects were left to
//! be found by an *audience* (item 132's value-stream pricing).

use std::path::{Path, PathBuf};
use std::process::Command;

/// A site whose only deck is reached through `{{< embed >}}`. `defective` decides whether
/// the deck carries the four defect shapes or is clean; the clean row is what proves a
/// passing site check is still possible, so the walk cannot be "always fails".
fn fixture(tag: &str, defective: bool) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-deck-in-site-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("_site.yml"), "title: Deck site\n").unwrap();
    std::fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\nThe talk: {{< embed talk.tmd >}}\n",
    )
    .unwrap();
    let body = if defective {
        // Four shapes, each from a different validator family: a missing local asset, a
        // broken local link, a link with no accessible name, and math KaTeX cannot parse.
        "## Slide one\n\n![](missing-image.png)\n\n[](nowhere.html)\n\n$$ \\frac{1}{ $$\n"
    } else {
        "## Slide one\n\nJust prose, nothing broken.\n"
    };
    std::fs::write(
        dir.join("talk.tmd"),
        format!("---\ntitle: Talk\nformat: deck\n---\n\n{body}"),
    )
    .unwrap();
    dir
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// `N problems` from a human `check` tally, so the site and standalone runs can be compared
/// as counts rather than by matching message text twice.
fn problem_count(stderr: &str) -> usize {
    stderr
        .lines()
        .find_map(|l| {
            l.split_once(" problem")
                .and_then(|(n, _)| n.trim().parse().ok())
        })
        .unwrap_or(0)
}

#[test]
fn site_check_reports_an_embedded_decks_defects() {
    let dir = fixture("check", true);
    let site = dir.to_str().unwrap();
    let deck = dir.join("talk.tmd");

    let (ok, _o, stderr) = run(&["check", site]);
    assert!(
        !ok,
        "a site whose deck is broken must not pass check:\n{stderr}"
    );

    // The identity the item is about: a site check and a standalone check of the same deck
    // must find the same defects. Counting both sides means a validator added later is
    // covered without touching this test.
    let (_ok2, _o2, deck_only) = run(&["check", deck.to_str().unwrap()]);
    assert_eq!(
        problem_count(&stderr),
        problem_count(&deck_only),
        "site check must find what a standalone check of the deck finds\n\
         site:\n{stderr}\ndeck alone:\n{deck_only}"
    );
    assert!(
        problem_count(&stderr) >= 4,
        "the fixture must really be defective (4 shapes): {stderr}"
    );
    // Located, and named by the deck's path within the project like every page is — an
    // unlocated "somewhere in your site" line is not actionable. The prefix is the target as
    // typed: `check` re-roots every human path onto it so the printed path opens from the
    // directory the command ran in, and a deck (which reaches the formatter down its own
    // `strip_prefix(root)` branch, not the page walk) has to follow that rule too.
    assert!(
        stderr.contains(&format!("{site}/talk.tmd:")),
        "deck diagnostics are located and rooted on the target as typed: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn strict_site_build_fails_on_an_embedded_decks_defects() {
    let dir = fixture("strict", true);
    let out = dir.join("_out");
    let site = dir.to_str().unwrap();

    let (ok, _o, stderr) = run(&["build", site, "--out", out.to_str().unwrap(), "--strict"]);
    assert!(
        !ok,
        "--strict must not bless a build that ships a broken deck:\n{stderr}"
    );
    assert!(
        stderr.contains("missing-image.png"),
        "the failure names the deck's defect: {stderr}"
    );
    // The deck is still WRITTEN: `--strict` is a gate on the report, not a refusal to build
    // (same contract pages have).
    assert!(
        out.join("talk.html").is_file(),
        "--strict still writes the page it complains about"
    );

    // A plain build stays green: this must gate `--strict`, not break every deck build.
    let plain = dir.join("_out2");
    let (ok2, _o2, stderr2) = run(&["build", site, "--out", plain.to_str().unwrap()]);
    assert!(ok2, "a non-strict build still succeeds:\n{stderr2}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_site_with_a_clean_deck_still_passes() {
    let dir = fixture("clean", false);
    let (ok, _o, stderr) = run(&["check", dir.to_str().unwrap()]);
    assert!(
        ok && stderr.contains("no problems found"),
        "walking decks must not invent problems on a clean one:\n{stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The repo's own guide is the real-world row: it ships two decks (`demo.tmd`, `tour.tmd`)
/// that `check docs/guide` never mentioned. They are clean, so a tally cannot prove the walk
/// runs — what proves it is that the *set of files checked* includes them.
#[test]
fn the_guides_own_decks_are_part_of_what_check_walks() {
    let guide = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("docs/guide");
    assert!(
        Path::new(&guide.join("demo.tmd")).is_file(),
        "fixture moved: docs/guide/demo.tmd is gone"
    );
    let (ok, stdout, stderr) = run(&["check", guide.to_str().unwrap(), "--format", "json"]);
    assert!(ok, "the guide must check clean:\n{stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        parsed["diagnostics"].as_array().map(Vec::len),
        Some(0),
        "the guide is expected clean; fix the guide, not this test: {stdout}"
    );
}
