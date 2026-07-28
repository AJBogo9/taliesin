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

/// The workflow was restored on 2026-07-28, but **every job is guarded on repository
/// visibility** so it stays inert until this repo is public. That means the false claim
/// this test was built to catch is still false: nothing in CI checks a push today. A doc
/// that credits "CI" for a gate is worse than silence — it tells the next reader (or
/// agent) a push is checked for them in ways it is not.
///
/// The two halves are asserted together on purpose. Making the workflow live is one
/// deletion (the guard) and it must not be possible to do that half without noticing the
/// prose it makes stale, in either direction.
#[test]
fn docs_do_not_promise_a_ci_that_enforces_gates() {
    // Walk the directory rather than naming ci.yml, so a workflow added later cannot
    // start billing a private repo just by not being on a hand-written list.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect(".github/workflows is missing: the workflow was restored on 2026-07-28")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yml" || x == "yaml"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no workflows found under {}",
        dir.display()
    );

    let mut total_jobs = 0;
    for f in &files {
        let workflow = std::fs::read_to_string(f).unwrap();
        // Everything below `jobs:`, so the `on:` keys above it are not counted as jobs
        // and the guard named in a header comment is not counted as a guard.
        let (_, body) = workflow
            .split_once("\njobs:\n")
            .unwrap_or_else(|| panic!("{} has no jobs: block", f.display()));
        let jobs = body
            .lines()
            .filter(|l| {
                l.strip_prefix("  ").is_some_and(|k| {
                    !k.starts_with(' ')
                        && k.trim_end().ends_with(':')
                        && k.starts_with(|c: char| c.is_ascii_lowercase())
                })
            })
            .count();
        let guards = body
            .matches("if: github.event.repository.private != true")
            .count();
        assert!(
            jobs > 0 && guards == jobs,
            "{guards} of {jobs} jobs in {} carry the repository-visibility guard. If the \
             repo is public now, dropping the guard is right — but then these docs have to \
             start crediting CI, so update them (and this test) rather than only the YAML.",
            f.display()
        );
        total_jobs += jobs;
    }
    assert!(
        total_jobs >= 7,
        "only {total_jobs} guarded jobs across {} workflow file(s): the restored gate set \
         had seven, so something was deleted rather than un-guarded",
        files.len()
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
        // Match the shapes that actually shipped, not one canonical phrasing. `deny.toml`
        // carried TWO independent claims and the first pass at this gate caught only one:
        // the header said "wired into CI" and a comment twelve lines below still called
        // cargo-audit "the other CI job". A gate that knows one spelling of a false claim
        // leaves its siblings in the same file.
        for needle in ["CI enforces", "CI-gated", "wired into CI", "CI job"] {
            assert!(
                !text.contains(needle),
                "{rel} still promises a CI gate ({needle:?}), but the workflow is gone \
                 and the check is manual"
            );
        }
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
