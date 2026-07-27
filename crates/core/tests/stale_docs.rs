use std::path::Path;

fn read(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// These phrases describe machinery deleted in the native rewrite and must not return.
#[test]
fn docs_do_not_claim_quarto_config_still_works() {
    let cfg = read("docs/guide/reference/configuration.tmd");
    assert!(
        !cfg.contains("still works"),
        "configuration.tmd still claims a Quarto config works"
    );
    assert!(
        !cfg.contains("Coming from a Quarto config?"),
        "configuration.tmd still has the stale Quarto-config callout"
    );
}

#[test]
fn internals_do_not_describe_the_deleted_shim() {
    let sites = read("docs/internals/sites.tmd");
    assert!(
        !sites.contains("site/config/quarto.rs"),
        "sites.tmd still describes the deleted quarto.rs shim"
    );
}

/// The GitHub Actions workflow was deleted on 2026-07-26 (it billed Actions minutes on
/// this private repo). `.githooks/pre-push` still gates fmt + clippy + `test --workspace`,
/// but the jobs only CI ran (live kernels, the JS type-checks, the VS Code grammar test,
/// `cargo audit`, `cargo deny`) are manual now. A doc that still credits "CI" for those is
/// worse than silence: it tells the next reader (or agent) a push is checked for them in
/// ways it is not.
#[test]
fn docs_do_not_promise_a_ci_that_enforces_gates() {
    assert!(
        !Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.github")
            .exists(),
        ".github/ is back: re-enabling CI means these docs need updating, not this test"
    );
    for rel in [
        "CLAUDE.md",
        "README.md",
        "docs/internals/extending.tmd",
        "docs/internals/repository.tmd",
    ] {
        let text = read(rel);
        assert!(
            !text.contains("CI enforces") && !text.contains("CI-gated"),
            "{rel} still promises a CI gate, but the workflow is gone and the check is manual"
        );
    }
}

#[test]
fn claude_md_does_not_list_feed_rs() {
    let claude = read("CLAUDE.md");
    assert!(
        !claude.contains("feed.rs"),
        "CLAUDE.md still lists the deleted feed.rs"
    );
}

/// The 2026-07-12 deck audit (A1/A2) deleted reader/scroll mode, drawing mode and
/// PDF-export mode, and `render::tests::deck_opens_as_a_deck_without_reader_or_pdf_export`
/// pins the machinery gone at the bundle level. Nothing pinned the *prose*: the two
/// marketing pages went on selling PDF export, and `samples/README.md` listed reader mode
/// and drawing on top of it. That audit's own findings doc even claims the stale claims
/// "were all stale and are now corrected" — the sweep missed all three files. fmt, clippy,
/// the suite and `check` every one of them pass over a false sentence, so gate the prose
/// against the machinery instead of against a memory of it.
#[test]
fn shipped_prose_does_not_advertise_deleted_deck_modes() {
    let deck_js = read("crates/core/assets/js/deck.js");
    for machinery in ["enterPrint", "enterScroll", "drawMode"] {
        assert!(
            !deck_js.contains(machinery),
            "{machinery} is back in deck.js — revive the prose deliberately rather than \
             deleting this test"
        );
    }

    // These three sell the deck engine and have no other reason to name a deleted mode,
    // so a bare mention is the defect. (`docs/guide/using/formats.tmd` deliberately says
    // there is *no* PDF export, which is why it is not on this list.)
    for rel in ["site/index.tmd", "site/formats.tmd", "samples/README.md"] {
        let text = read(rel);
        for claim in ["PDF export", "PDF-export", "reader mode", "Reader mode"] {
            assert!(
                !text.contains(claim),
                "{rel} advertises {claim:?}, deleted from the deck engine on 2026-07-12"
            );
        }
    }
}
