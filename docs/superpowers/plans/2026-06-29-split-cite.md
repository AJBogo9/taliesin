# Split `cite.rs` into `cite/` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`).

**Goal:** Split the 1563-line `crates/core/src/cite.rs` into a cohesive `cite/`
module directory along its six orthogonal responsibilities, zero behavior change.

**Architecture:** Behavior-preserving move-refactor. `cite.rs → cite/mod.rs` holding
the contract + shared types (`Bibliography`/`Entry`/`Fields`) + the one shared
sourcepos helper + `pub use` re-exports. Six sibling modules. Every external caller
(`crate::cite::{Bibliography, parse_bib_warned, process, validate_xrefs}` in
render/mod.rs, site/mod.rs, server main.rs, serve.rs) is unchanged.

**Tech Stack:** Rust edition 2024. The existing 19 cite tests + corpus are the
characterization-test safety net; no new tests.

## Global Constraints

- `cargo fmt` + `clippy -D warnings` clean. No behavior change: a pure move; do not
  edit any function body, public name, or logic, or reformat untouched lines.
- **Public surface unchanged** (re-exported from `mod.rs`): `cite::Bibliography`
  (+ `::default()` via derive, `::is_empty`), `cite::parse_bib`,
  `cite::parse_bib_warned`, `cite::process`, `cite::validate_xrefs`.
- Invariant: `process` only transforms plain-text runs (never tags/code/math), so
  block sourcepos is untouched; the only structural change is appending a References
  block. Preserve exactly. `cite` is NOT in the exec/kernel Do-NOT-touch zone.

## Target layout (`cite/`)

```
mod.rs        contract + `pub struct Bibliography` (#[derive(Default)]) + `struct Entry`
              (#[derive(Default)]) + `type Fields` + `impl Bibliography { pub fn is_empty }`
              + `pub(crate) fn sourcepos_start_line` (shared by render+validate)
              + mod decls + pub use re-exports + `#[cfg(test)] mod tests`
clean.rs      LaTeX/BibTeX cleaning: clean (pub(crate)) + latex_accents, read_accent_arg,
              compose, accent_diacritic, special_letter, precomposed
author.rs     IEEE author names: format_authors (pub(crate)) + format_one_author,
              initials, join_authors
format.rs     IEEE formatting: `impl Bibliography { pub(crate) fn format }` + fmt_article,
              title_with_segs, fmt_book, fmt_inbook, fmt_misc, quoted_title, append_url,
              clean_pages, ordinal
parse.rs      BibTeX parse: parse_bib (pub), parse_bib_warned (pub) + take_while, skip_ws,
              skip_entry, read_value, normalize_ws
render.rs     citation/xref HTML processing: process (pub) + xref_label,
              is_manual_references_heading, transform_html, rewrite_text, is_cite_key_char,
              xref_anchor_link, xref_link, parse_xref, render_citation_group
              + `#[cfg(test)] mod tests` for the rewrite_text test (a render private)
validate.rs   validate_xrefs (pub)
tests.rs      the other 18 cite tests (public-API + clean() via pub(crate))
```

## Visibility bumps (the only non-verbatim change; necessary for cross-module calls)

- `clean` → `pub(crate)` (called by format.rs, author.rs, tests.rs)
- `format_authors` → `pub(crate)` (called by format.rs's `Bibliography::format`)
- `Bibliography::format` → `pub(crate)` (called by render.rs's `process`, tests.rs)
- `sourcepos_start_line` → `pub(crate)` (in mod.rs; called by render.rs + validate.rs)

All others keep their current visibility. `Entry`/`Fields` stay private in `mod.rs`
(descendant modules read their private fields legally).

## Intra-`cite` dependency graph (one direction, no cycles)

```
clean.rs   ← (nothing)            author.rs ← clean
format.rs  ← clean, author, mod   parse.rs  ← mod
render.rs  ← mod (format via the pub(crate) method)   validate.rs ← mod
```

## Per-module `use` headers

- clean.rs: none (self-contained; `precomposed` keeps its inline `use unicode_normalization::UnicodeNormalization;`)
- author.rs: `use super::clean::clean;` `use crate::render::escape_attr as esc;`
- format.rs: `use super::author::format_authors;` `use super::clean::clean;` `use super::{Bibliography, Fields};` `use crate::render::escape_attr as esc;`
- parse.rs: `use super::{Bibliography, Entry};` `use std::collections::HashMap;`
- render.rs: `use super::{Bibliography, sourcepos_start_line};` `use crate::render::{Block, Warning, escape_attr as esc};` `use std::collections::HashMap;`
- validate.rs: `use super::sourcepos_start_line;` `use crate::render::{Block, Warning};`
- mod.rs: `use std::collections::HashMap;`
- tests.rs: `use super::*;` `use super::clean::clean;` `use crate::render::Block;` `use std::collections::HashMap;`

## Re-exports in `mod.rs`

```rust
pub use parse::{parse_bib, parse_bib_warned};
pub use render::process;
pub use validate::validate_xrefs;
// Bibliography is defined in mod.rs (already `cite::Bibliography`).
```

## Tasks

- [ ] **Task 1:** `git mv cite.rs cite/mod.rs`; `cargo build -p qmd-fast-core` (path-transparent, clean).
- [ ] **Task 2:** Create clean.rs, author.rs, format.rs, parse.rs, validate.rs (move funcs verbatim + headers + the visibility bumps). The `impl Bibliography { format }` goes to format.rs as its own `impl` block.
- [ ] **Task 3:** Create render.rs (move funcs verbatim + header) with its own `#[cfg(test)] mod tests` holding `rewrite_text_leaves_unmatched_and_non_citation_brackets_literal`.
- [ ] **Task 4:** Rewrite mod.rs to contract + types + `is_empty` impl + `sourcepos_start_line` (pub(crate)) + mod decls + re-exports + `#[cfg(test)] mod tests`. Create tests.rs with the remaining 18 tests + the imports above.
- [ ] **Task 5:** Verify: `cargo test -p qmd-fast-core --lib cite::` (expect 18 in cite::tests + 1 in cite::render::tests = 19), then `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (430/0; the parallel_build_determinism flake is the known load-sensitive one — re-run in isolation if it alone fails). Spot-check `qmd-fast check` on a corpus doc with citations. Commit as one refactor.

## Self-Review

- Spec coverage: implements the blueprint's `cite.rs → cite/{parse,format,clean,author,render,validate}` Tier-1 split + the contract convention + per-split verification. ✓
- Placeholders: none.
- Type consistency: re-export names match all `crate::cite::` call sites; visibility bumps are the minimal set the dependency graph requires; `process`/`validate_xrefs`/`parse_bib_warned` signatures unchanged.
