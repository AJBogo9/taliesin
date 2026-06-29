# Split `diagnostics.rs` into `diagnostics/` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the 1121-line `crates/core/src/diagnostics.rs` into a cohesive
`diagnostics/` module directory — one file per validator family + a shared helpers
file — with zero behavior change, proving the architecture-blueprint split pattern.

**Architecture:** A behavior-preserving move-refactor (Fowler). `diagnostics.rs`
becomes `diagnostics/mod.rs`, which holds the module contract + `mod`/`pub use`
re-exports + the test module declaration. Each of the 8 validators moves to its own
file with its private helpers; 4 helpers shared across families move to `helpers.rs`.
The public surface is re-exported from `mod.rs` so every external caller
(`use qmd_fast_core::diagnostics as dx;` in `server/src/main.rs`) is unchanged.

**Tech Stack:** Rust (edition 2024). The existing 23 `#[cfg(test)]` tests plus the
corpus invariants are the characterization-test safety net — no new tests are written;
each step is gated on them staying green.

## Global Constraints

- Edition 2024, workspace resolver 3. `cargo fmt` + `clippy -D warnings` must stay clean (CI enforces).
- **No behavior change.** This is a pure move. Do not edit any function body, rename
  anything public, reorder logic, or reformat untouched lines.
- **Public surface unchanged:** `qmd_fast_core::diagnostics::{validate_duplicate_heading_ids,
  validate_internal_anchors, citations_without_bibliography, validate_local_assets,
  validate_local_media, validate_local_links, validate_js_reactive_graph, validate_a11y}`
  must all still resolve (re-exported from `mod.rs`).
- Invariant guardrails (none should be touched here, but do not break them): block
  `data-block-id`/`data-sourcepos`, read-only preview, HTML-only, warm kernel. The
  validators are read-only static analysis; keep them so.
- One logical change; commit at the end of each task.

## File Structure (target)

```
crates/core/src/diagnostics/
  mod.rs        contract doc + `mod x;` + `pub use x::*;` re-exports + `#[cfg(test)] mod tests;`
  helpers.rs    pub(crate): start_line, is_local_ref, collect_attr_values, tag_attr
  headings.rs   validate_duplicate_heading_ids + heading_id
  anchors.rs    validate_internal_anchors + same_page_manual_fragments
  bibliography.rs  citations_without_bibliography
  assets.rs     validate_local_assets + local_img_refs
  media.rs      validate_local_media + local_media_refs
  links.rs      validate_local_links + local_link_refs + link_target_exists
  reactive.rs   validate_js_reactive_graph + struct JsNode + closest_owned
  a11y.rs       validate_a11y + heading_level + tag_has_attr + strip_tags
                + has_accessible_name + struct Interactive + interactives
  tests.rs      the existing `mod tests` body (23 tests)
```

Each family file begins with the imports it needs (the compiler is the final arbiter):
- All: `use crate::render::{Block, Warning};`  (a11y also `DocFormat`)
- `assets.rs`/`media.rs`/`links.rs`: `use std::path::Path;`
- Any file using a shared helper: `use super::helpers::{…};` (only the ones it calls)

---

### Task 1: Convert file → directory and extract shared helpers

**Files:**
- Rename: `crates/core/src/diagnostics.rs` → `crates/core/src/diagnostics/mod.rs`
- Create: `crates/core/src/diagnostics/helpers.rs`
- Modify: `crates/core/src/diagnostics/mod.rs`

**Interfaces:**
- Produces: `pub(crate) fn start_line(sourcepos: &str) -> Option<u32>`,
  `pub(crate) fn is_local_ref(v: &str) -> bool`,
  `pub(crate) fn collect_attr_values<'a>(html: &'a str, needle: &str, out: &mut Vec<&'a str>)`,
  `pub(crate) fn tag_attr<'a>(tag: &'a str, attr: &str) -> Option<&'a str>` — all in `diagnostics::helpers`.

- [ ] **Step 1: Move the file into a directory**

```bash
cd crates/core/src
mkdir diagnostics
git mv diagnostics.rs diagnostics/mod.rs
```

- [ ] **Step 2: Verify it still builds unchanged**

Run: `cargo build -p qmd-fast-core`
Expected: builds clean (a file→dir move with `mod.rs` is path-transparent; `pub mod diagnostics;` in `lib.rs:30` is unchanged).

- [ ] **Step 3: Create `helpers.rs` with the four shared helpers**

Create `crates/core/src/diagnostics/helpers.rs`. Move (cut) these four functions
**verbatim** from `mod.rs` into it, and change each from `fn`/private to `pub(crate) fn`
(they are now called across module boundaries):

```rust
//! HTML / sourcepos helpers shared by more than one validator family.

/// 1-based start line from a block's `sourcepos` (`"startLine:col-..."`), if positive.
pub(crate) fn start_line(sourcepos: &str) -> Option<u32> { /* …verbatim body… */ }

/// True when `v` is a local (non-external, non-anchor, non-`data:`) reference.
pub(crate) fn is_local_ref(v: &str) -> bool { /* …verbatim body… */ }

/// Push every value following `needle` (e.g. `id="`) in `html` into `out`.
pub(crate) fn collect_attr_values<'a>(html: &'a str, needle: &str, out: &mut Vec<&'a str>) { /* …verbatim body… */ }

/// The value of `attr` (e.g. `href="`) within a single `tag`, if present.
pub(crate) fn tag_attr<'a>(tag: &'a str, attr: &str) -> Option<&'a str> { /* …verbatim body… */ }
```

(Copy the real bodies from the original `mod.rs` lines: `start_line` 12-22, `is_local_ref`
149-164, `collect_attr_values` 63-83, `tag_attr` 238-251. Do not alter the bodies.)

- [ ] **Step 4: Wire `helpers` into `mod.rs`**

In `diagnostics/mod.rs`: delete the four moved function definitions, and below the
existing `use` lines add:

```rust
mod helpers;
use helpers::{collect_attr_values, is_local_ref, start_line, tag_attr};
```

- [ ] **Step 5: Build + test (the validators still call the helpers, now via `use`)**

Run: `cargo test -p qmd-fast-core diagnostics`
Expected: PASS (23 tests). Then `cargo build -p qmd-fast-core` clean.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/diagnostics/
git commit -m "refactor(diagnostics): file->dir + extract shared helpers (no behavior change)"
```

---

### Task 2: Extract the eight validator families

Each family is the same mechanical move. For family `F` with public validator `V`,
private items `P…`, and shared helpers `H…` it calls:

1. Create `crates/core/src/diagnostics/F.rs`.
2. Add its imports at the top (see "File Structure"): `use crate::render::{Block, Warning};`
   (+`DocFormat` / `use std::path::Path;` as noted), and `use super::helpers::{H…};`
   for only the shared helpers it calls.
3. Cut `V` and its private items `P…` **verbatim** from `mod.rs` into `F.rs`. `V`
   stays `pub fn`; the `P…` stay private (`fn`/`struct`) — they're only used within `F.rs`.
4. In `mod.rs` add `mod F;` and `pub use F::V;`.

**Files (create each, modify `mod.rs`):**

- [ ] **Step 1: `headings.rs`** — move `validate_duplicate_heading_ids` (pub) + `heading_id` (priv). Imports: `use crate::render::{Block, Warning};` `use super::helpers::start_line;`. In `mod.rs`: `mod headings; pub use headings::validate_duplicate_heading_ids;`

- [ ] **Step 2: `anchors.rs`** — move `validate_internal_anchors` (pub) + `same_page_manual_fragments` (priv). Imports: `use crate::render::{Block, Warning};` `use super::helpers::{collect_attr_values, start_line};`. In `mod.rs`: `mod anchors; pub use anchors::validate_internal_anchors;`

- [ ] **Step 3: `bibliography.rs`** — move `citations_without_bibliography` (pub; no private helpers). Imports: `use crate::render::{Block, Warning};` `use super::helpers::start_line;`. In `mod.rs`: `mod bibliography; pub use bibliography::citations_without_bibliography;`

- [ ] **Step 4: `assets.rs`** — move `validate_local_assets` (pub) + `local_img_refs` (priv). Imports: `use crate::render::{Block, Warning};` `use std::path::Path;` `use super::helpers::{is_local_ref, start_line};`. In `mod.rs`: `mod assets; pub use assets::validate_local_assets;`

- [ ] **Step 5: `media.rs`** — move `validate_local_media` (pub) + `local_media_refs` (priv). Imports: `use crate::render::{Block, Warning};` `use std::path::Path;` `use super::helpers::{is_local_ref, start_line, tag_attr};`. In `mod.rs`: `mod media; pub use media::validate_local_media;`

- [ ] **Step 6: `links.rs`** — move `validate_local_links` (pub) + `local_link_refs` + `link_target_exists` (priv). Imports: `use crate::render::{Block, Warning};` `use std::path::Path;` `use super::helpers::{is_local_ref, start_line, tag_attr};`. In `mod.rs`: `mod links; pub use links::validate_local_links;`

- [ ] **Step 7: `reactive.rs`** — move `validate_js_reactive_graph` (pub) + `struct JsNode` + `closest_owned` (priv). Imports: `use crate::render::{Block, Warning};` `use super::helpers::{collect_attr_values, start_line};`. In `mod.rs`: `mod reactive; pub use reactive::validate_js_reactive_graph;`

- [ ] **Step 8: `a11y.rs`** — move `validate_a11y` (pub) + `heading_level` + `tag_has_attr` + `strip_tags` + `has_accessible_name` + `struct Interactive` + `interactives` (priv). Imports: `use crate::render::{Block, DocFormat, Warning};` `use super::helpers::{start_line, tag_attr};`. In `mod.rs`: `mod a11y; pub use a11y::validate_a11y;`

- [ ] **Step 9: Build + test after the batch**

Run: `cargo build -p qmd-fast-core` (clean), then `cargo test -p qmd-fast-core diagnostics`
Expected: PASS (23 tests). If the compiler reports an unresolved helper, that helper
was mis-assigned — move it to `helpers.rs` (`pub(crate)`) and import it.

- [ ] **Step 10: Commit**

```bash
git add crates/core/src/diagnostics/
git commit -m "refactor(diagnostics): one module per validator family (no behavior change)"
```

---

### Task 3: Move tests to a sibling, write the module contract, final verify

**Files:**
- Create: `crates/core/src/diagnostics/tests.rs`
- Modify: `crates/core/src/diagnostics/mod.rs`

- [ ] **Step 1: Move the test module body to `tests.rs`**

Cut the entire `#[cfg(test)] mod tests { … }` block from `mod.rs` into a new
`crates/core/src/diagnostics/tests.rs` containing only the **inner** body (the
contents between the outer `mod tests {` and its closing `}`), with `use super::*;`
at the top so the tests still see the re-exported validators + `Warning`/`Block`.
In `mod.rs`, replace the block with:

```rust
#[cfg(test)]
mod tests;
```

- [ ] **Step 2: Write the module contract at the top of `mod.rs`**

Replace the existing `//!` header with the three-question contract (keep it ≤ ~12 lines):

```rust
//! Static document-lint validators for `qmd-fast check` (the "check-superset").
//!
//! **What:** one read-only validator per family — headings, anchors, bibliography,
//! assets, media, links, reactive graph, a11y — each takes the rendered block model
//! (and, where needed, the doc base dir) and returns located [`Warning`]s on the same
//! click-to-source channel as render-time diagnostics, so a green `check` means the
//! document is publishable.
//!
//! **How to use:** call the re-exported `validate_*` / `citations_without_bibliography`
//! fns; `qmd-fast check` (`crates/server/src/main.rs`) runs the whole set.
//!
//! **Depends on:** [`crate::render`] for the block model + `Warning` channel, and
//! `std::path` for the asset/link existence checks. Pure static analysis; the only IO
//! is stat-ing referenced local files.
```

- [ ] **Step 3: Confirm `mod.rs` is now just contract + wiring**

`mod.rs` should contain only: the contract doc, `mod helpers; use helpers::{…};`, the
eight `mod F; pub use F::V;` lines, and `#[cfg(test)] mod tests;`. Nothing else.

- [ ] **Step 4: Full verification gate**

Run, expecting all green:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: fmt clean; clippy clean; all tests pass — in particular the diagnostics
tests (23) and the corpus invariants (`crates/core/tests/corpus.rs`). Note:
`parallel_build_determinism::sequential_and_concurrent_match_with_code_cells` has a
known load-sensitive flake (backlog); if it alone fails, re-run it in isolation to
confirm it is the pre-existing flake and not a regression.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/diagnostics/
git commit -m "refactor(diagnostics): move tests to sibling + module contract; mod.rs is now a map"
```

---

## Self-Review

- **Spec coverage:** Implements the blueprint's Tier-1 `diagnostics.rs → diagnostics/`
  split (one file per validator family + `helpers.rs` + `mod.rs` re-exports), the
  module-contract convention, and the per-split verification gate. ✓
- **Placeholders:** The `/* …verbatim body… */` markers are deliberate move-references
  (the bodies exist in the source and move unchanged), with exact source line numbers
  given — not unfinished code. No `TODO`/`TBD`.
- **Type consistency:** Validator signatures and helper signatures are copied from the
  current source (`validate_*(blocks: &[Block], …) -> Vec<Warning>`,
  `validate_a11y(blocks, format: DocFormat)`); helper visibility raised to `pub(crate)`;
  re-export names match every `dx::` call site in `main.rs`.
```
