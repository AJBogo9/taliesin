//! Citations present but no `bibliography:` declared.

use crate::render::{Block, Warning};

/// Citations are present (`cite::process` appended the `qmd-references` section) but the
/// front matter declares no `bibliography:`, so every reference renders as a raw key with
/// no diagnostic today. (A declared-but-missing bibliography file is a separate warning.)
pub fn citations_without_bibliography(src: &str, blocks: &[Block]) -> Vec<Warning> {
    let has_citations = blocks.iter().any(|b| b.id == "qmd-references");
    if !has_citations {
        return Vec::new();
    }
    let declares_bib = crate::frontmatter::front_matter_block(src)
        .and_then(|fm| serde_yaml::from_str::<serde_yaml::Value>(fm).ok())
        .and_then(|v| v.as_mapping().map(|m| m.get("bibliography").is_some()))
        .unwrap_or(false);
    if declares_bib {
        return Vec::new();
    }
    vec![Warning::new(
        "citations are present but no `bibliography:` is declared, so every reference renders as a raw key",
    )]
}
