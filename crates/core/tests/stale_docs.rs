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
    // THIRD_PARTY.md and deny.toml were the two that actually carried a false claim past
    // this gate: both asserted "CI enforces" the licence policy while `cargo deny` runs
    // nowhere but by hand. A gate whose file list omits the files that drift is not a gate.
    for rel in [
        "CLAUDE.md",
        "README.md",
        "THIRD_PARTY.md",
        "deny.toml",
        ".claude/hooks/cargo-fmt.sh",
        ".claude/agents/corpus-verifier.md",
        "docs/internals/extending.tmd",
        "docs/internals/repository.tmd",
    ] {
        let text = read(rel);
        assert!(
            !text.contains("CI enforces")
                && !text.contains("CI-gated")
                && !text.contains("wired into CI"),
            "{rel} still promises a CI gate, but the workflow is gone and the check is manual"
        );
    }
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

    // These sell the deck engine and have no other reason to name a deleted mode, so a
    // bare mention is the defect. (`docs/guide/using/formats.tmd` deliberately says there
    // is *no* PDF export, which is why it is not on this list.)
    //
    // `demo.tmd` is on the list because the first version of this gate omitted it and the
    // omission cost exactly what the gate exists to prevent: `site/demo.tmd` is embedded
    // INTO `site/index.tmd` and `site/formats.tmd` via `{{< embed >}}`, so checking only
    // the two embedding pages proves nothing about what the landing page renders. It went
    // on advertising a one-slide-per-page PDF, a "scrollable reader", and a <kbd>D</kbd>
    // pen tool that never existed in any version of the engine.
    //
    // The needles are the *shapes that actually shipped*, not the vocabulary the deleted
    // features were named after: the stale prose said "one-slide-per-page PDF" and
    // "scrollable **reader**", neither of which contains "PDF export" or "reader mode".
    for rel in [
        "site/index.tmd",
        "site/formats.tmd",
        "site/demo.tmd",
        "docs/guide/demo.tmd",
        "samples/README.md",
    ] {
        let text = read(rel);
        for claim in [
            "PDF export",
            "PDF-export",
            "per-page PDF",
            "reader mode",
            "Reader mode",
            "scrollable **reader**",
            "a **pen**",
            "to annotate",
        ] {
            assert!(
                !text.contains(claim),
                "{rel} advertises {claim:?}, which the deck engine does not do \
                 (reader/PDF modes were deleted 2026-07-12; the pen never existed)"
            );
        }
    }
}
