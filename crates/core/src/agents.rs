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

/// The committed onramp, bundled so `init` can scaffold it without runtime generation,
/// exactly as `vocab.rs` bundles the vocab JSON. This is the **only** committed copy: Wave
/// 1.2 deleted the byte-identical repo-root duplicate (and the server-crate gate that kept
/// it in sync by hand), so a vocabulary change now reconciles here and nowhere else.
pub const AGENTS_MD: &str = include_str!("../assets/agents/AGENTS.md");

/// The worked CSV→figure cell embedded in the `## Recipes` section (DX19). Kept
/// byte-identical to the real, `check`-clean corpus document `corpus/recipes/csv-figure.tmd`
/// by the `recipe_matches_the_corpus_example` test, so the one data idiom `vocab` can't
/// express as a closed set is taught from a proven example that can't rot.
const CSV_FIGURE_CELL: &str = r#"```{python}
#| label: fig-sales
#| fig-cap: "Monthly sales from `data.csv`."
import pandas as pd
import matplotlib.pyplot as plt

data = pd.read_csv("data.csv")
fig, ax = plt.subplots()
ax.plot(data["month"], data["sales"], marker="o")
ax.set_xlabel("month")
ax.set_ylabel("sales")
```"#;

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
    s.push_str(
        "Add `--run` to also execute the `{python}`/`{r}` cells and report what each \
         produced (`[figure fig-x: produced, alt \"…\"]`, `[output: …]`, \
         `[cell error: …]`), so you can confirm a computed figure actually baked without \
         opening a browser; `taliesin read --run <file> --format json` gives the same \
         per-cell result structured.\n\n",
    );
    s.push_str(
        "A `{js}`/Observable-Plot cell runs in the browser, so with `--run` Taliesin also \
         drives a local headless Chrome over the built page and reports whether each \
         `{js}` chart painted (`[js: produced, <svg W×H>]`, or `[js error: …]` when it \
         threw). With no local Chrome available it degrades to `[js: skipped (chrome \
         unavailable)]`, never a failure.\n\n",
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

    // Recipes: a worked idiom the closed-set `vocab` can't express, pinned to a real
    // corpus document so it stays runnable (DX19). `vocab`/`schema` cover *structure*
    // (which keys/options exist); this covers the one *composition* an agent otherwise
    // has to learn from prose: turning a data file into a numbered, referenceable figure.
    s.push_str("\n## Recipes\n\n");
    s.push_str(
        "Worked idioms the closed-set `vocab` can't express. Each is kept byte-identical to \
         a real, `check`-clean corpus document, so it stays runnable.\n\n",
    );
    s.push_str(
        "**A figure from a CSV** (the one data idiom worth learning from an example): read a \
         data file, plot it, and give the cell a `fig-`-prefixed `#| label:` so its output \
         becomes a numbered, `@fig-`-referenceable figure. Keep the data beside the `.tmd`:\n\n",
    );
    let _ = writeln!(s, "~~~~\n{CSV_FIGURE_CELL}\n~~~~");
    s.push_str(
        "\nThen reference it in prose: `@fig-sales shows the trend.` For the R kernel, swap \
         `{python}` + pandas for `{r}` + readr (`read_csv(\"data.csv\")`); the `#| label:` and \
         `@fig-` reference are identical.\n",
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
        // DX19: the CSV→figure recipe (the one data idiom `vocab` can't express).
        assert!(
            md.contains("## Recipes") && md.contains("read_csv"),
            "missing the CSV->figure recipe section"
        );
    }

    /// Isolate the first ```` ```{python} ```` fenced cell from a `.tmd` source, verbatim
    /// (opening fence through closing fence). The recipe cell has no nested triple-backtick,
    /// so the first `\n```` closes it.
    fn extract_python_cell(doc: &str) -> Option<String> {
        let start = doc.find("```{python}")?;
        let rest = &doc[start + 3..]; // past the opening fence, so it can't self-match
        let close = rest.find("\n```")?;
        let end = start + 3 + close + "\n```".len();
        Some(doc[start..end].to_string())
    }

    /// DX19's "generated from real corpus examples so it can't drift" contract: the onramp's
    /// CSV→figure recipe must stay byte-identical to the real, `check`-clean corpus document
    /// it teaches, so an agent copies a proven idiom, not a rotting snippet. If the corpus
    /// example changes, this fails until `CSV_FIGURE_CELL` is updated to match (then re-bless
    /// AGENTS.md).
    #[test]
    fn recipe_matches_the_corpus_example() {
        let path = format!(
            "{}/../../corpus/recipes/csv-figure.tmd",
            env!("CARGO_MANIFEST_DIR")
        );
        let doc = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let cell = extract_python_cell(&doc).expect("the recipe corpus doc has a {python} cell");
        assert_eq!(
            cell, CSV_FIGURE_CELL,
            "the AGENTS.md CSV->figure recipe drifted from {path}; update CSV_FIGURE_CELL to \
             match and re-bless AGENTS.md"
        );
        assert!(
            agents_md().contains(CSV_FIGURE_CELL),
            "the generated onramp does not embed the CSV->figure recipe cell"
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
