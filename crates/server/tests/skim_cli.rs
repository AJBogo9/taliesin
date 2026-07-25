//! `taliesin skim`: the layer-cake projection, pinned against `corpus/tarn`.
//!
//! The projection is the *measuring instrument* the structural lints are calibrated
//! against, so these tests are mostly about it not lying: the first sentence is the one the
//! author wrote (not a heading welded to a paragraph, not a shell command read as prose),
//! and a section with nothing in it is annotated rather than dropped.

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_taliesin")
}

fn skim(args: &[&str]) -> (String, String, bool) {
    let out = Command::new(bin())
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run taliesin skim");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

/// `corpus/` sits two levels up from `crates/server`.
const TARN: &str = "../../corpus/tarn";

#[test]
fn the_human_stream_carries_headings_numbers_and_opening_sentences() {
    let (out, _, ok) = skim(&["skim", TARN]);
    assert!(ok, "skim should succeed on corpus/tarn");
    // Numbered headings, as the rendered page shows them (not a bare markdown scan).
    assert!(
        out.contains("5.2 How nulls behave"),
        "expected the rendered section number in the stream:\n{out}"
    );
    // The opening sentence of that section, verbatim.
    assert!(
        out.contains("A comparison against null is null, not false"),
        "expected the section's own opening sentence:\n{out}"
    );
}

#[test]
fn a_chapters_opening_sentence_excludes_its_own_heading() {
    // Regression: the intro was read as flattened text over the whole pre-section slice, so
    // it began with the chapter heading — and a heading carries no terminator, so it ran
    // straight into the first paragraph and the projection reported "1 Installation Tarn
    // ships two ways: …" as the opening sentence. Not a sentence anyone wrote; the
    // instrument must not invent prose.
    //
    // Held by the first-`<p>` rule in `first_prose_sentence`, NOT by the slice bounds: a
    // branch that skipped past the title heading was tried, and deleting it left the whole
    // projection byte-identical. Mutate `first_prose_sentence`, not the bounds, to see this
    // test bite.
    let (out, _, _) = skim(&["skim", TARN]);
    assert!(
        out.contains("▸ Tarn ships two ways"),
        "expected the chapter's opening sentence to start at the paragraph:\n{out}"
    );
    assert!(
        !out.contains("▸ 1 Installation Tarn ships"),
        "the chapter heading must not be welded onto its opening sentence:\n{out}"
    );
}

#[test]
fn code_and_tab_labels_are_not_read_as_prose() {
    // Regression: reading the whole section with `indexable_text` flattened a tabset's
    // labels and its shell commands into the sentence stream, which has no terminators, so
    // the "opening sentence" ran on into "macOS Linux Windows brew install tarn curl …".
    let (out, _, _) = skim(&["skim", TARN]);
    assert!(
        out.contains("The tarn binary is self-contained"),
        "expected the real opening sentence:\n{out}"
    );
    assert!(
        !out.contains("brew install tarn"),
        "shell commands must not appear in the projected prose:\n{out}"
    );
    assert!(
        !out.contains("scoop install"),
        "tab-panel content must not appear in the projected prose:\n{out}"
    );
}

#[test]
fn a_missing_opening_is_annotated_not_omitted() {
    // The instrument's one hard rule: a judgement is visible beside the text, never a
    // suppression. `grouping.tmd` opens straight onto its first section heading, and that
    // must read differently from a chapter the projection simply failed on.
    let (out, _, _) = skim(&["skim", TARN]);
    assert!(
        out.contains("(no opening prose)"),
        "a chapter with no opening paragraph should say so:\n{out}"
    );
}

#[test]
fn standalone_layers_are_projected_with_their_kind() {
    let (out, _, _) = skim(&["skim", TARN]);
    assert!(
        out.contains("[callout]"),
        "expected a callout title layer:\n{out}"
    );
    assert!(
        out.contains("[theorem] Definition 6.1"),
        "expected a NUMBERED theorem statement layer:\n{out}"
    );
}

#[test]
fn nested_sections_indent_by_depth_measured_per_page() {
    // Depth is measured against the page's own shallowest heading, never the absolute
    // level, so a `###`-rooted chapter cannot indent deeper than a `##`-rooted neighbour.
    let (out, _, _) = skim(&["skim", TARN, "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out).expect("skim --json is valid JSON");
    let pages = v["pages"].as_array().expect("pages array");
    for p in pages {
        let depths: Vec<u64> = p["sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["depth"].as_u64().unwrap())
            .collect();
        if let Some(first) = depths.first() {
            assert_eq!(
                *first, 0,
                "every page's first section sits at depth 0; {} did not",
                p["url"]
            );
        }
    }
    let grouping = pages
        .iter()
        .find(|p| p["url"] == "grouping.html")
        .expect("grouping.html");
    let nested: Vec<(u64, &str)> = grouping["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["depth"].as_u64().unwrap(), s["title"].as_str().unwrap()))
        .collect();
    assert!(
        nested.contains(&(1, "6.2.1 Counting distinct values")),
        "a `###` subsection should sit one step in: {nested:?}"
    );
}

#[test]
fn json_reports_no_prose_as_null_rather_than_omitting_the_field() {
    // A consumer must be able to tell "this section has no prose" from "this projection
    // does not carry sentences" — the machine form of the no-suppression rule.
    let (out, _, _) = skim(&["skim", TARN, "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let any_section = v["pages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["sections"].as_array().unwrap())
        .next()
        .expect("at least one section");
    assert!(
        any_section.get("first_sentence").is_some(),
        "first_sentence must always be present, null included: {any_section}"
    );
}

#[test]
fn word_counts_agree_with_the_map_projection() {
    // `skim`, `map` and the LSP outline all count via `prose::word_count`. If they ever
    // disagree, one of them has grown its own counter.
    let (skim_out, _, _) = skim(&["skim", TARN, "--json"]);
    let (map_out, _, _) = skim(&["map", TARN, "--json"]);
    let s: serde_json::Value = serde_json::from_str(&skim_out).unwrap();
    let m: serde_json::Value = serde_json::from_str(&map_out).unwrap();
    for page in s["pages"].as_array().unwrap() {
        let url = page["url"].as_str().unwrap();
        let mapped = m["pages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["url"] == url)
            .unwrap_or_else(|| panic!("{url} missing from map"));
        assert_eq!(
            page["words"], mapped["words"],
            "skim and map disagree on {url}'s word count"
        );
    }
}

#[test]
fn map_headings_carry_the_rendered_numbers() {
    // The reason these come from the projection and not a markdown scan: a markdown scan
    // reports every heading of a numbered book unnumbered.
    let (out, _, _) = skim(&["map", TARN, "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let grouping = v["pages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["url"] == "grouping.html")
        .expect("grouping.html");
    let texts: Vec<&str> = grouping["headings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["text"].as_str().unwrap())
        .collect();
    assert!(
        texts.iter().any(|t| t.starts_with("6.1 ")),
        "expected rendered chapter-scoped numbers, got {texts:?}"
    );
    assert!(
        grouping["words"].as_u64().unwrap() > 0,
        "a real chapter should report a nonzero word count"
    );
}

#[test]
fn a_website_projects_without_chapter_numbers() {
    // The `c`-less shape: a non-book must not grow numbers it does not have.
    let (out, _, ok) = skim(&["skim", "../../corpus/scaffold-site", "--json"]);
    assert!(ok, "skim should succeed on a plain website:\n{out}");
    let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(v["is_book"], false, "corpus/scaffold-site is not a book");
    for p in v["pages"].as_array().unwrap() {
        assert!(
            p["chapter"].is_null(),
            "a website page must carry no chapter number: {}",
            p["url"]
        );
    }
}

#[test]
fn a_file_is_rejected_with_a_helpful_message() {
    let (_, err, ok) = skim(&["skim", "../../corpus/tarn/grouping.tmd"]);
    assert!(!ok, "a single file is not a project");
    assert!(
        err.contains("symbols") || err.contains("read"),
        "the error should point at the single-file commands: {err}"
    );
}

#[test]
fn an_unknown_flag_is_rejected_with_a_did_you_mean() {
    let (_, err, ok) = skim(&["skim", TARN, "--formt", "json"]);
    assert!(!ok, "an unknown flag is an error");
    assert!(
        err.contains("--format"),
        "expected a did-you-mean naming --format: {err}"
    );
}

#[test]
fn an_unknown_format_value_is_rejected() {
    let (_, err, ok) = skim(&["skim", TARN, "--format", "yaml"]);
    assert!(!ok, "an unknown format is an error");
    assert!(!err.is_empty(), "expected a diagnostic");
}
