//! The agent onramp: a generated `AGENTS.md` that teaches a coding agent the whole
//! Taliesin authoring loop in one file.
//!
//! An agent meeting Taliesin for the first time guesses from stale Quarto priors. This
//! file replaces the guessing with a protocol: the four pillars (edit the `.tmd` never
//! the preview; the `check --format json` gate; `symbols`/`vocab`/`schema` for
//! discovery; the build/publish commands) plus a **dialect section** whose vocabulary is
//! generated from [`crate::vocab::vocab`], so it can never drift from what `check`
//! enforces. Golden-file-locked exactly like `vocab.rs`/`schema.rs`: regenerate ONLY via
//! `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib agents`, never hand-edit the
//! committed asset.

use serde_json::Value;
use std::fmt::Write as _;

/// The committed onramp, bundled so `init` can scaffold it and the repo can ship a
/// verbatim copy without runtime generation, exactly as `vocab.rs` bundles the vocab JSON.
pub const AGENTS_MD: &str = include_str!("../assets/agents/AGENTS.md");

/// Pull the `name` field out of each `{ "name", ... }` entry in a `vocab()` array.
fn names(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|e| e["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// A comma-joined inline-code list (`` `a`, `b`, `c` ``) for the dialect section.
fn code_list(items: &[String]) -> String {
    items
        .iter()
        .map(|s| format!("`{s}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Generate the `AGENTS.md` onramp. Deterministic (walks [`crate::vocab::vocab`]), so the
/// committed asset is golden-locked against it.
pub fn agents_md() -> String {
    let vocab = crate::vocab::vocab();

    let callout_kinds = names(&vocab["calloutKinds"]);
    let cell_options = names(&vocab["cellOptions"]);
    let div_classes = names(&vocab["divClasses"]);
    let fm_keys = names(&vocab["frontmatter"]["keys"]);
    let xref_prefixes: Vec<String> = vocab["xrefPrefixes"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|e| e["prefix"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let a_callout = callout_kinds
        .first()
        .cloned()
        .unwrap_or_else(|| "note".to_string());

    let mut s = String::new();

    // Header + the one framing sentence.
    s.push_str("# AGENTS.md\n\n");
    s.push_str(
        "Guidance for AI coding agents authoring a [Taliesin](https://github.com/AJBogo9/taliesin) \
         project. Taliesin renders `.tmd` files to HTML (blog posts, slide decks, books, \
         multi-page sites). This file is generated from the live validator vocabulary, so its \
         dialect list cannot drift from what `taliesin check` enforces.\n\n",
    );

    // Pillar 1 — the single editing surface.
    s.push_str("## Edit the `.tmd`, never the preview\n\n");
    s.push_str(
        "The `.tmd` file is the single editing surface. The browser preview is a read-only view: \
         edits flow one way, source -> preview, and the preview must never be treated as writable. \
         Make every change in the `.tmd` source; **never the preview**.\n\n",
    );

    // Pillar 2 — the check gate.
    s.push_str("## Gate every change on `check`\n\n");
    s.push_str(
        "After editing, validate with `taliesin check <file-or-dir> --format json`. It prints an \
         object `{ \"diagnostics\": [...], \"environment\": {...} }`; a non-empty `diagnostics` \
         array (and a non-zero exit code) means the document has problems to fix. This is the \
         machine-readable gate an agent drives instead of opening a browser:\n\n",
    );
    s.push_str("```sh\ntaliesin check index.tmd --format json\n```\n\n");
    s.push_str(
        "To *read what you made* without a browser, `taliesin read <file>` projects the \
         rendered document to plain text (headings, resolved \"Figure N\"/cross-reference \
         numbers, callouts, fenced code, math as TeX) — the agent's substitute for looking \
         at the preview.\n\n",
    );

    // Pillar 3 — discovery surfaces.
    s.push_str("## Discover the surface\n\n");
    s.push_str(
        "Three read-only commands describe what Taliesin accepts, so an agent never has to guess:\n\n",
    );
    s.push_str(
        "- `taliesin vocab` -> every closed-set construct (front-matter keys, cell options, \
         callout/theorem kinds, div classes, cross-reference prefixes) as JSON.\n",
    );
    s.push_str("- `taliesin schema` -> the JSON Schema for front matter and `_site.yml`.\n");
    s.push_str(
        "- `taliesin symbols <file>` -> the headings, figures, and cross-reference targets in \
         a document.\n\n",
    );

    // Pillar 4 — build/publish.
    s.push_str("## Build and publish\n\n");
    s.push_str(
        "- `taliesin build <file-or-dir>` -> self-contained HTML (a single file, or a `_site/` \
         folder for a multi-page project).\n",
    );
    s.push_str(
        "- `taliesin preview <file-or-dir>` -> a live-reloading dev server (for a human; an agent \
         uses `check` + `build`).\n\n",
    );

    // Dialect — generated from vocab().
    s.push_str("## Dialect\n\n");
    s.push_str(
        "Taliesin's Markdown is Pandoc-flavored. The closed sets below come straight from the \
         validator (run `taliesin vocab` for the full list with descriptions):\n\n",
    );
    let _ = writeln!(
        s,
        "- **Callouts:** a fenced div `::: {{.callout-{a_callout}}}` opens a callout. Kinds: {}.",
        code_list(&callout_kinds)
    );
    let _ = writeln!(
        s,
        "- **Code cells:** a fenced ` ```{{python}} ` (or `{{r}}`, `{{js}}`) block runs live. \
         In-cell options use a `#| key: value` comment, e.g. `#| label: fig-scree` or \
         `#| echo: false`. Options: {}.",
        code_list(&cell_options)
    );
    let _ = writeln!(
        s,
        "- **Cross-references:** cite a labelled target with `@`-prefixes ({}); e.g. `@fig-scree` \
         renders as \"Figure 3\".",
        code_list(&xref_prefixes)
    );
    let _ = writeln!(
        s,
        "- **Citations:** `[@key]` cites a `.bib` entry declared in the `bibliography:` front matter."
    );
    let _ = writeln!(
        s,
        "- **Structural divs:** `::: {{.class}} ... :::` blocks. Classes: {}.",
        code_list(&div_classes)
    );
    let _ = writeln!(
        s,
        "- **Front matter:** a leading `---` YAML block. Keys: {}.",
        code_list(&fm_keys)
    );

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The onramp teaches the four pillars and the vocab-sourced dialect. These substrings
    /// are the contract the `agents_md_cli.rs` scaffold test also checks.
    #[test]
    fn agents_md_teaches_the_protocol() {
        let md = agents_md();
        // Pillar 1: single editing surface.
        assert!(
            md.contains("never the preview"),
            "missing edit-the-source rule"
        );
        // Pillar 2: the check gate.
        assert!(
            md.contains("check <file-or-dir> --format json")
                || md.contains("check index.tmd --format json"),
            "missing the check --format json gate"
        );
        // Dialect terms sourced from vocab():
        assert!(md.contains("[@key]"), "missing citation dialect");
        assert!(md.contains("#| label:"), "missing cell-option dialect");
        // A callout kind, straight from vocab()["calloutKinds"].
        let kinds = names(&crate::vocab::vocab()["calloutKinds"]);
        let first = kinds.first().expect("at least one callout kind");
        assert!(
            md.contains(first),
            "missing callout kind `{first}` from vocab"
        );
    }

    /// Assert the generated markdown equals the committed asset, OR (under `TALIESIN_BLESS=1`)
    /// rewrite the committed file from the generator. Mirrors `vocab.rs`/`schema.rs`.
    #[test]
    fn agents_md_matches_committed() {
        let generated = agents_md();
        if std::env::var("TALIESIN_BLESS").is_ok() {
            let path = format!("{}/assets/agents/AGENTS.md", env!("CARGO_MANIFEST_DIR"));
            std::fs::write(&path, &generated).unwrap_or_else(|e| panic!("write {path}: {e}"));
            eprintln!("blessed assets/agents/AGENTS.md");
        } else {
            assert_eq!(
                generated, AGENTS_MD,
                "AGENTS.md drift; regenerate with `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib agents`"
            );
        }
    }
}
