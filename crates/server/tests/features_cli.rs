//! `taliesin features` — the adoption report (backlog item 202).
//!
//! The pin lives here rather than in `corpus/` on purpose: `features` emits no HTML, and the
//! corpus walker renders every corpus document on every `cargo test` without ever running a
//! CLI, so a corpus pin would pay the render cost and exercise none of this. Same reasoning
//! as `executed_output_reproducible.rs`.
//!
//! **Every assertion here carries its opposite.** The failure mode of an adoption report is
//! not a wrong number, it is a table that is uniformly empty (a scanner that found nothing)
//! or uniformly full (a scanner that matched everything), and either reads as plausible
//! output. So each test that asserts a feature IS used also asserts a sibling feature is
//! NOT, and the zero rows are checked to be present-and-empty rather than absent.

use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "tali-features-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> (String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin features");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        out.status.success(),
    )
}

fn json(args: &[&str]) -> serde_json::Value {
    let (stdout, ok) = run(args);
    assert!(ok, "features failed for {args:?}");
    serde_json::from_str(&stdout).expect("features emits valid json")
}

/// Find one feature's entry in a group.
fn feature<'a>(v: &'a serde_json::Value, slug: &str, name: &str) -> &'a serde_json::Value {
    v["groups"]
        .as_array()
        .expect("groups")
        .iter()
        .find(|g| g["slug"] == slug)
        .unwrap_or_else(|| panic!("no group `{slug}`"))["features"]
        .as_array()
        .expect("features")
        .iter()
        .find(|f| f["name"] == name)
        .unwrap_or_else(|| panic!("no feature `{name}` in group `{slug}`"))
}

fn docs_of(v: &serde_json::Value, slug: &str, name: &str) -> Vec<String> {
    feature(v, slug, name)["documents"]
        .as_array()
        .expect("documents is always an array, even when empty")
        .iter()
        .map(|d| d.as_str().unwrap().to_string())
        .collect()
}

/// A document that writes a known set of constructs, and pointedly does NOT write their
/// nearest neighbours, so the report cannot pass by matching everything.
const USES: &str = r#"---
title: Uses
theorems:
  shared: [theorem, lemma]
---

::: {.scrolly}
::: {.step}
A step.
:::
:::

::: {.callout-warning}
Careful.
:::

```{python}
#| label: fig-one
print(1)
```

{{< video clip.mp4 >}}

See @fig-one.
"#;

/// A second document sharing exactly one construct with the first, so "used by 2" and
/// "used by 1" are both exercised and cannot be the same code path.
const SHARES_ONE: &str = "---\ntitle: Shares\n---\n\n::: {.scrolly}\nJust a scrolly.\n:::\n";

fn fixture_dir(name: &str) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("uses.tmd"), USES).unwrap();
    std::fs::write(d.join("shares.tmd"), SHARES_ONE).unwrap();
    d
}

#[test]
fn a_bare_directory_reports_who_uses_what() {
    let d = fixture_dir("dir");
    let v = json(&["features", d.to_str().unwrap(), "--json"]);
    assert_eq!(v["documents"], 2);

    // Used by both, used by one, and used by neither — three different outcomes from one
    // scan. Without all three a scanner that returns "every document uses everything" or
    // "nothing uses anything" would satisfy the test.
    assert_eq!(
        docs_of(&v, "div-classes", "scrolly"),
        ["shares.tmd", "uses.tmd"]
    );
    assert_eq!(docs_of(&v, "div-classes", "step"), ["uses.tmd"]);
    assert!(
        docs_of(&v, "div-classes", "magic-move").is_empty(),
        "no fixture writes .magic-move"
    );

    assert_eq!(docs_of(&v, "callout-kinds", "warning"), ["uses.tmd"]);
    assert!(
        docs_of(&v, "callout-kinds", "tip").is_empty(),
        "no fixture writes a tip callout"
    );
    assert_eq!(docs_of(&v, "cell-languages", "python"), ["uses.tmd"]);
    assert!(docs_of(&v, "cell-languages", "r").is_empty());
    assert_eq!(docs_of(&v, "shortcodes", "video"), ["uses.tmd"]);
    assert!(docs_of(&v, "shortcodes", "embed").is_empty());
    assert_eq!(docs_of(&v, "xref-kinds", "fig"), ["uses.tmd"]);
    assert!(docs_of(&v, "xref-kinds", "thm").is_empty());
    assert_eq!(
        docs_of(&v, "frontmatter-subkeys", "theorems.shared"),
        ["uses.tmd"]
    );
    assert!(docs_of(&v, "frontmatter-subkeys", "hero.headline").is_empty());
    _ = std::fs::remove_dir_all(&d);
}

/// The report's reason for existing: an unused feature is a ROW, never an omission. If it
/// were omitted, "nothing uses this" and "this is not a feature" would be the same output,
/// and the corpus-plus-roadmap gap it exists to surface would be invisible.
#[test]
fn an_unused_feature_is_a_row_not_an_omission() {
    let d = fixture_dir("zero");
    let v = json(&["features", d.to_str().unwrap(), "--json"]);
    let magic = feature(&v, "div-classes", "magic-move");
    assert!(
        magic["documents"].is_array() && magic["documents"].as_array().unwrap().is_empty(),
        "an unused feature carries an empty array, not a missing key: {magic}"
    );
    let g = v["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["slug"] == "div-classes")
        .unwrap();
    assert_eq!(
        g["known"].as_u64().unwrap(),
        g["used"].as_u64().unwrap() + g["unused"].as_u64().unwrap(),
        "known must be used + unused, or the denominator is lying"
    );
    assert!(
        g["unused"].as_u64().unwrap() > 0,
        "the fixture leaves classes unused"
    );
    assert!(g["used"].as_u64().unwrap() > 0, "and uses some: {g}");
    _ = std::fs::remove_dir_all(&d);
}

/// **The corpus-plus-roadmap policy, enforced against the real corpus:** every catalogued
/// construct is pinned by at least one corpus document. This is the report checking the
/// policy that governs the project, which is the whole point of building it, and it doubles
/// as a positive control on a real tree rather than a synthetic one.
///
/// This test used to assert the opposite — it named `logo:` and `csl:` as *known* gaps
/// (backlog item 207) and failed if they ever got pinned. Both were closed on 2026-08-07
/// along with the rest: `logo:` by the deck corner mark on `corpus/deck.tmd`, `csl:` by
/// `corpus/diagnostics/typos.tmd` (it is recognized-but-inert, so the doc that pins it is
/// the one that pins its diagnostic). Item 207 originally listed four keys; the other three
/// (`include-in-header`, `include-before-body`, `include-after-body`) were discharged on
/// 2026-08-02 by *removal*, and `acknowledgements:` on 2026-08-03 by retirement — the two
/// other ways a documented-but-unpinned key can be closed.
///
/// A NEW feature that lands without a corpus document fails here, which is the gate the
/// policy asks for. Closing it means adding the pin doc, not editing this list.
#[test]
fn every_catalogued_feature_is_pinned_by_a_corpus_document() {
    let corpus = format!("{}/../../corpus", env!("CARGO_MANIFEST_DIR"));
    let v = json(&["features", &corpus, "--json"]);
    assert!(
        v["documents"].as_u64().unwrap() > 100,
        "the corpus walk must find the whole corpus, not one directory"
    );

    let mut unpinned: Vec<String> = Vec::new();
    for g in v["groups"].as_array().expect("groups") {
        let slug = g["slug"].as_str().unwrap_or("?");
        assert_eq!(
            g["known"].as_u64().unwrap(),
            g["used"].as_u64().unwrap() + g["unused"].as_u64().unwrap(),
            "`{slug}`: known must be used + unused, or the denominator is lying"
        );
        for f in g["features"].as_array().into_iter().flatten() {
            if f["documents"].as_array().is_some_and(|d| d.is_empty()) {
                unpinned.push(format!("{slug}.{}", f["name"].as_str().unwrap_or("?")));
            }
        }
    }
    assert!(
        unpinned.is_empty(),
        "every catalogued feature must be pinned by a corpus document \
         (corpus-plus-roadmap: a capability ships WITH its pin doc). Unpinned: {unpinned:#?}"
    );

    // The control: keys the corpus obviously DOES set, so an all-empty table cannot pass.
    assert!(docs_of(&v, "frontmatter-keys", "title").len() > 50);
    assert!(!docs_of(&v, "frontmatter-keys", "bibliography").is_empty());
}

/// A single file reports document-first, but the JSON keeps the one adoption shape so a
/// consumer never branches on which target it was handed.
#[test]
fn a_single_file_is_a_one_document_project() {
    let d = scratch("file");
    let f = d.join("one.tmd");
    std::fs::write(&f, USES).unwrap();
    let v = json(&["features", f.to_str().unwrap(), "--json"]);
    assert_eq!(v["documents"], 1);
    assert_eq!(docs_of(&v, "div-classes", "scrolly").len(), 1);
    assert!(docs_of(&v, "div-classes", "magic-move").is_empty());

    let (human, ok) = run(&["features", f.to_str().unwrap()]);
    assert!(ok);
    assert!(
        human.contains("scrolly"),
        "the document view names what it uses: {human}"
    );
    assert!(
        !human.contains("magic-move"),
        "the document view omits what it does not use, unlike the table: {human}"
    );
    assert!(human.contains("feature(s) used"), "{human}");
    _ = std::fs::remove_dir_all(&d);
}

/// The directory view is the inverse: it names the unused, because that is the audit.
#[test]
fn the_directory_view_shows_the_unused_tail() {
    let d = fixture_dir("human");
    let (human, ok) = run(&["features", d.to_str().unwrap()]);
    assert!(ok);
    assert!(
        human.contains("(no document)"),
        "zero rows are visible: {human}"
    );
    assert!(
        human.contains("features are used by no document"),
        "and totalled: {human}"
    );
    // Three or fewer users are named inline, so the low-adoption tail needs no second
    // command. Above three, the count alone.
    assert!(
        human.contains("uses.tmd"),
        "a one-document feature names its document: {human}"
    );
    _ = std::fs::remove_dir_all(&d);
}

/// A project uses its own page order rather than the path walk, so the report and a build
/// agree on which documents exist and in what sequence.
#[test]
fn a_project_reports_in_its_own_page_order() {
    let d = scratch("project");
    std::fs::write(
        d.join("_site.yml"),
        "title: P\nchapters:\n  - second.tmd\n  - first.tmd\n",
    )
    .unwrap();
    std::fs::write(
        d.join("first.tmd"),
        "---\ntitle: A\n---\n\n::: {.scrolly}\nx\n:::\n",
    )
    .unwrap();
    std::fs::write(
        d.join("second.tmd"),
        "---\ntitle: B\n---\n\n::: {.scrolly}\ny\n:::\n",
    )
    .unwrap();
    let v = json(&["features", d.to_str().unwrap(), "--json"]);
    assert_eq!(
        docs_of(&v, "div-classes", "scrolly"),
        ["second.tmd", "first.tmd"],
        "chapters: order, not alphabetical path order"
    );
    _ = std::fs::remove_dir_all(&d);
}

/// Build output holds generated copies of the documents being counted. Walking it would
/// double every number in the report, which is the kind of wrong that still looks right.
#[test]
fn build_output_is_not_counted_twice() {
    let d = fixture_dir("skip");
    for dir in ["_site", "_freeze"] {
        let sub = d.join(dir);
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("copy.tmd"), USES).unwrap();
    }
    let v = json(&["features", d.to_str().unwrap(), "--json"]);
    assert_eq!(v["documents"], 2, "_site/ and _freeze/ are not documents");
    assert_eq!(docs_of(&v, "div-classes", "step"), ["uses.tmd"]);
    _ = std::fs::remove_dir_all(&d);
}

/// It is a report, not a gate. Nothing it can find is an error exit; only being unable to
/// read the target is.
#[test]
fn it_never_fails_on_what_it_finds() {
    let d = fixture_dir("exit");
    assert!(run(&["features", d.to_str().unwrap()]).1);
    // An empty directory has nothing to report, which IS a usage error: the user named the
    // wrong path, and a silent empty table would hide that.
    let empty = scratch("empty");
    assert!(!run(&["features", empty.to_str().unwrap()]).1);
    assert!(!run(&["features", &format!("{}/nope", d.display())]).1);
    // An unknown flag is rejected rather than ignored, like every sibling command.
    assert!(!run(&["features", d.to_str().unwrap(), "--unused"]).1);
    _ = std::fs::remove_dir_all(&d);
    _ = std::fs::remove_dir_all(&empty);
}

/// A construct shown as an example must not be counted as a use, or the guide's own
/// reference pages become the heaviest users in the repo and every number is inflated.
#[test]
fn documentation_of_a_feature_is_not_use_of_it() {
    let d = scratch("docs");
    std::fs::write(
        d.join("reference.tmd"),
        "---\ntitle: Reference\n---\n\nThe `logo:` key, and `{{< embed x.tmd >}}`:\n\n\
         ````\n::: {.magic-move}\n:::\n\n{{< video a.mp4 >}}\n````\n",
    )
    .unwrap();
    let v = json(&["features", d.to_str().unwrap(), "--json"]);
    for (slug, name) in [
        ("frontmatter-keys", "logo"),
        ("shortcodes", "embed"),
        ("shortcodes", "video"),
        ("div-classes", "magic-move"),
    ] {
        assert!(
            docs_of(&v, slug, name).is_empty(),
            "`{name}` is documented here, not used here"
        );
    }
    // The control: the page's own real front matter still counts, so this is not just
    // "the scanner found nothing".
    assert_eq!(docs_of(&v, "frontmatter-keys", "title"), ["reference.tmd"]);
    _ = std::fs::remove_dir_all(&d);
}

/// `features` must never execute anything: it is parse-only, and an editor or an agent can
/// call it on any tree. A cell that would fail loudly if run must leave no trace.
#[test]
fn it_executes_nothing() {
    let d = scratch("noexec");
    let canary = d.join("canary.txt");
    std::fs::write(
        d.join("danger.tmd"),
        format!(
            "---\ntitle: D\n---\n\n```{{python}}\nopen({:?}, 'w').write('executed')\n```\n",
            canary.to_str().unwrap()
        ),
    )
    .unwrap();
    let v = json(&["features", d.to_str().unwrap(), "--json"]);
    assert_eq!(docs_of(&v, "cell-languages", "python"), ["danger.tmd"]);
    assert!(
        !Path::new(&canary).exists(),
        "features ran the cell; it must be parse-only"
    );
    _ = std::fs::remove_dir_all(&d);
}
