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
