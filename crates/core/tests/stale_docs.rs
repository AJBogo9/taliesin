use std::path::Path;

fn read(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// These phrases describe machinery deleted in DROP-QUARTO and must not return.
#[test]
fn docs_do_not_claim_quarto_config_still_works() {
    let cfg = read("docs/guide/reference/configuration.qmd");
    assert!(
        !cfg.contains("still works"),
        "configuration.qmd still claims a Quarto config works"
    );
    assert!(
        !cfg.contains("Coming from a Quarto config?"),
        "configuration.qmd still has the stale Quarto-config callout"
    );
}

#[test]
fn internals_do_not_describe_the_deleted_shim() {
    let sites = read("docs/internals/sites.qmd");
    assert!(
        !sites.contains("site/config/quarto.rs"),
        "sites.qmd still describes the deleted quarto.rs shim"
    );
}

#[test]
fn claude_md_does_not_list_feed_rs() {
    let claude = read("CLAUDE.md");
    assert!(
        !claude.contains("feed.rs"),
        "CLAUDE.md still lists the deleted feed.rs"
    );
}
