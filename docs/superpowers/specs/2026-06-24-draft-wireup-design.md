# Design: wire up `draft:` (low) — exclude draft pages from a website build

Status: approved 2026-06-24 ("implement these"). Branch `feat/draft-wireup`. From
`backlog.md` Open/next. Core-only; zero new dependencies.

## Problem

`draft: true` in a page's front matter is a silent no-op. (Backlog said it was already in
the allowlist — it is NOT: `KNOWN_KEYS` has no `draft`, so `draft: true` currently triggers
an "unknown front-matter key" warning.) Wiring it up has three parts:

1. **Recognize it:** add `"draft"` to `crate::frontmatter::KNOWN_KEYS` so it's a valid key
   (no more unknown-key warning). This regenerates the drift-locked JSON schema, so
   re-bless `assets/schema/qmd-frontmatter.schema.json` via
   `QMD_FAST_BLESS=1 cargo test -p qmd-fast-core --lib schema`.
2. **Parse it:** add `pub(crate) draft: bool` to `site::frontmatter::FrontInfo`, set from
   `val.get("draft")` as a YAML bool (default false).
3. **Honor it:** in `website_pages` (`site/mod.rs`), `filter_map` out pages whose `fm.draft`
   is true — before they become `Page`s. Since listings derive from `self.pages`
   (`collection()` → `self.pages.iter()`) and prev/next nav iterates pages, a dropped draft
   disappears from the build output, listings, and pages-derived nav in one move.

## Scope

Websites only (the `website_pages` path). Books list chapters explicitly via `chapters:`;
a draft chapter is out of scope. `draft: false`/absent → page built as normal.

## Test (TDD, `#[cfg(test)] mod tests` in `site/mod.rs`)

Temp site with `index.qmd`, `published.qmd`, and `wip.qmd` (`draft: true`); call
`website_pages(root)` and assert the page `rel`s include `index.qmd` + `published.qmd` and
NOT `wip.qmd`. (Plus the existing `every_corpus_doc_*` validator tests stay green — `draft`
is now a known key.)

## Invariants

Core-only; no change to render/exec, the block model, or book chapters; zero new deps. The
generated schema stays drift-locked (re-blessed in the same change).

## Out of scope

Draft books/chapters; a `--drafts` build flag to include them; visually marking drafts in
preview.
