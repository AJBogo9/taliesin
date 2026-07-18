# DX5 — "Did you mean" on the last silent-degradation paths

Date: 2026-07-18. Backlog item **DX5** (§6 DX audit batch, Tier 1). Branch
`dx5-div-class-did-you-mean`. Detail source: `notes/2026-07-18-dx-audit.md`.

> **Autonomy note:** author asked me to continue without the interactive gate. Decisions below
> are documented defaults.

## Goal

Kill the two remaining *silent-degradation* traps in `:::` fenced divs — the class of bug that put
Lena's before/after slide on a projector in front of 200 people:

- **A `::: {.columns}` (reveal muscle-memory) silently stacks** instead of laying out side-by-side
  (it falls through to a plain div; the native syntax is `{layout-ncol=N}`). Fix: **accept
  `.columns`/`.column` as a `layout-ncol` alias** so it just works.
- **A misspelled feature/theorem class silently becomes a plain div, no warning.** Fix: a **"did
  you mean"** on a near-miss, extending the existing `validate_callout_kind` pattern — but only for
  near-misses, since div classes are an *open* vocabulary (custom classes are legitimate).

## Ground truth (grepped + read against source 2026-07-18)

- **The dispatch is a long if/else in `build_container`** ([`divs.rs:382`](../../../crates/core/src/render/divs.rs)):
  callout → `layout-ncol` grid → `magic-move` → `code-walkthrough` → `step` → `panel-tabset` →
  `scrolly` → theorem → **generic `else`** (`divs.rs:~648`: `class = attrs.classes.join(" ")` →
  `<div class="{class}">`). The `else` is the silent path; it already has `warnings`, `file`,
  `open_line` in scope.
- **The maintainers already flagged this exact gap.** [`validate.rs:41-42`](../../../crates/core/src/render/validate.rs):
  *"A misspelled [theorem] kind has no prefix to anchor a did-you-mean, so it falls through to a
  plain div (see the design doc)."* DX5 is that design doc.
- **`closest(key, cands)`** ([`frontmatter.rs:577`](../../../crates/core/src/frontmatter.rs)) =
  the nearest candidate within **edit distance ≤ 2**, else `None`. The exact tool the callout /
  front-matter / command did-you-means already use.
- **Canonical vocab consts:** `CALLOUT_KINDS`, `THEOREM_KINDS`
  ([`validate.rs:36,43`](../../../crates/core/src/render/validate.rs)); the structural div classes
  are enumerated in `vocab::div_classes()` ([`vocab.rs:170`](../../../crates/core/src/vocab.rs))
  but have "no single Rust const" yet.
- **`callout_kind()` matches any `callout-` prefix** ([`mod.rs:2088`](../../../crates/core/src/render/mod.rs)),
  so unknown *callout* kinds already warn via `validate_callout_kind`; only prefix-less typos
  (`.calout-note`) fall through — out of scope (rare; no anchor).
- **`theorem_kind()`** returns `Some` only for a `THEOREM_KINDS` member, so a typo'd theorem kind
  falls through — the primary target of the did-you-mean.

## Resolved decisions (autonomous, documented)

1. **Part A — `.columns` alias.** A div whose classes contain `columns` (and no explicit
   `layout-ncol`) renders as the existing `tali-layout` grid with `ncol` = the count of direct
   `.column` children (fallback **2**). Reuses the exact grid HTML the `layout-ncol` arm emits. The
   `.column` children stay generic divs (grid cells). **Silent** (a sanctioned alias — the whole
   point is that muscle-memory *works*, not that it nags).
2. **Part B — near-miss "did you mean".** In the generic `else`, for each class **not** already a
   recognized feature and **not** exactly in the known set, run `closest(class, KNOWN)`; on a hit,
   push one located (click-to-source) warning `unknown div class \`X\` (did you mean \`Y\`?)`. The
   div still renders (purely diagnostic), exactly like `validate_callout_kind`.
   - **KNOWN = a new `DIV_FEATURE_CLASSES` const ∪ `THEOREM_KINDS`.** `DIV_FEATURE_CLASSES` lives in
     `validate.rs` (beside `CALLOUT_KINDS`/`THEOREM_KINDS`) = the structural + deck feature classes:
     `panel-tabset, code-walkthrough, scrolly, magic-move, step, column-margin, aside, sidenote,
     marginnote, fragment, incremental, notes, columns, column`. A drift test keeps
     `vocab::div_classes()`'s names ⊆ this const.
   - **Open-vocabulary safety:** a class exactly in KNOWN (a legit generic class like `aside`,
     `fragment`) → no warning; a class > 2 edits from every KNOWN name (a genuine custom class) →
     no warning; only a 1–2-edit near-miss warns. **Accepted tradeoff:** a custom class that
     happens to sit within 2 edits of a feature name (e.g. `.roof` vs `proof`, `.side` vs `aside`)
     draws a spurious *warning* (never an error; renders fine; click-to-source; suggests the fix).
     This is the same tradeoff the closed-vocab did-you-means already make; the upside (catching the
     on-projector class of bug, incl. the theorem typos the code comment calls out) dominates, and
     `DIV_FEATURE_CLASSES` is one const to tune if it proves noisy.

## Changes

### `crates/core/src/render/validate.rs`
- Add `pub(crate) const DIV_FEATURE_CLASSES: &[&str]` (list above).
- Add `pub(crate) fn validate_div_class(classes: &[String], line: usize, file: Option<String>) -> Option<Warning>`:
  for the first class that is not in `DIV_FEATURE_CLASSES ∪ THEOREM_KINDS` yet has a `closest()`
  hit in that union, return `unknown_key_message("div class", class, &union)`-style warning `.at`.
  (One warning per div — the first offending class — to avoid a pile-up.)
- Re-export `DIV_FEATURE_CLASSES` from `render/mod.rs` if `vocab.rs` needs it for the drift test.

### `crates/core/src/render/divs.rs`
- **Part A:** add a `columns` arm **before** the generic `else` (after the `layout-ncol` arm is
  fine; a dedicated arm keeps intent legible): if `attrs.classes` contains `columns` and no
  `layout-ncol`, compute `ncol = max(2, count of inner blocks whose html starts with
  `<div class="column"`)` … actually `ncol = count of .column children if ≥1 else 2` — and emit the
  same `tali-layout` grid string the `layout-ncol` arm uses.
- **Part B:** in the generic `else`, before building the plain div, call `validate_div_class(&attrs.classes, open_line, file.clone())` and push any warning.

### `crates/core/src/vocab.rs`
- Point `div_classes()` at (or add a subset-of) `DIV_FEATURE_CLASSES` so the editor vocab and the
  validator share one source (or add the drift test), keeping the existing per-class descriptions.

## Testing (TDD)

- **`validate.rs` unit tests:** `validate_div_class(["fragmnet"])` → warns "did you mean `fragment`";
  `["theorm"]` → "did you mean `theorem`"; `["aside"]` (known) → `None`; `["my-widget"]` (far) →
  `None`; `["columns"]` → `None` (handled by Part A, and it's known anyway).
- **`divs.rs` unit tests:** a `::: {.columns}` with two `.column` children renders a
  `tali-layout` grid with `repeat(2,minmax(0,1fr))`; three children → `repeat(3,…)`; and the
  fall-through still emits `<div class="myclass">` for a genuine custom class (no regression).
- **Corpus pins:** (a) a `.columns` example proving the alias renders a grid (capability pin —
  place in a general corpus doc, decided in planning so it doesn't collide with the deck-redesign
  direction); (b) a misspelled feature class in `corpus/diagnostics/` with its warning asserted
  (diagnostics are exempt from the clean-corpus rule; `corpus.rs:45,70`).
- **Full gate:** `cargo test -p taliesin-core -p taliesin-server`, `cargo fmt --check`,
  `cargo clippy -- -D warnings`. Mutation-check each new pin.

## Non-goals

- **Prefix-less callout typos** (`.calout-note`) — no anchor; rare; skip.
- **Structural width fidelity** for `.columns` (reveal `width="50%"` on `.column`) — the alias lays
  out equal columns; widths are ignored (the on-projector fix is *side-by-side*, not pixel parity).
- **Warning on every unknown class** — explicitly rejected (open vocabulary; would nag legit custom
  classes). Only near-misses warn.

## Invariant safety

Render-time diagnostics + one new div arm only. No output-format change, no CDN, no preview
write-back. `data-block-id`/`data-sourcepos` on every emitted div preserved; `MAX_WARM_PAGES` /
`exec_pool.rs` freeze untouched. Part A adds a grid wrapper (same contract as `layout-ncol`); Part B
only *adds a warning* and never changes what renders.
