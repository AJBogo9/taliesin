# Per-chapter sidebar-label override for books

## Problem

A book chapter's sidebar label is derived from its first `# H1` (then front-matter
`title:`, then the file stem). A dual-use doc (one that is also stand-alone-capable,
so it carries a front-matter `title:` plus flat `#` sections) is therefore labelled
by its first *section*, not its title. There was no way to set a chapter's sidebar
label without editing the chapter's content.

## Decision

Author's choice (over "prefer front-matter `title:`" and "keep Quarto first-H1 parity"):
**add a per-chapter override in `_site.yml`**. A `chapters:` entry may now be either:

- a bare path string — `- intro.qmd` (unchanged), or
- a `{ file:, text: }` mapping — `- file: intro.qmd` / `  text: "Introduction"`,
  where `text:` overrides the sidebar label.

The override form also works inside a `{ part:, chapters: }` group's inner list.
When `text:` is absent, the label falls back to first-H1 → front-matter `title:` →
file stem exactly as before (no behavior change for existing books).

This keeps the chapter content untouched (single-editing-surface friendly) and is a
strict superset of the prior schema.

## Implementation

- `crates/core/src/site/book.rs`: `build_book` now routes each `chapters:` entry
  through `push_chapter_entry` (string or `{file,text}`); a non-chapter mapping is a
  `{part,chapters}` group whose inner list reuses the same helper. `push_chapter`
  gained a `label: Option<&str>` that wins over the H1/title fallback chain.
- `crates/core/src/schema.rs`: `chapters` gained a typed sub-schema
  (`string | {file,text} | {part,chapters}`) so the editor's YAML language server
  autocompletes/validates the override; the committed `qmd-site.schema.json` was
  re-blessed.

## Corpus pin

`corpus/demo-book/_site.yml` now labels `methods.qmd` "Methodology" (nested in the
"Core" part) and `summary.qmd` "Wrap-up" (top level) via the override form; the
`book_discovers_chapters_with_parts_numbering_and_chrome` test asserts those labels,
pinning both the top-level and nested override paths.
