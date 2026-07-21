# B3-18: deck subtree re-mount (preserve live widget state across a structural deck edit)

Date: 2026-07-21
Status: design approved, ready for plan
Backlog item: A.1 (B3-18), the last open DOM-state-preservation gap in a shipping live view.

## Problem

A structural deck edit (adding, removing, reordering, or retitling a slide, or inserting
a `---` / `. . .`) currently re-mounts the **whole** deck. The server detects the
structural change (`deck_structural`, `crates/server/src/serve/mod.rs:1312`) and broadcasts
a `full_render` with `remount: true`. The client
(`web-client/client.js`, the `full_render` case) then runs `resetJs()` +
`root.innerHTML = msg.body_html`, tearing down **every** `{js}`/WebGL widget on **every**
slide, not just the one the edit touched.

That breaks the load-bearing DOM-state-preservation invariant: a running Three.js scene, a
playing video, an author's slider value, or any `{js}` cell state on an *untouched* slide is
destroyed whenever the author edits any *other* slide's structure. Non-deck docs already
preserve this via incremental block ops (`update`/`insert`/`remove`/`set_meta`); the deck is
the one place the invariant still breaks.

Observable today with the existing corpus deck: `corpus/deck.tmd`'s "Ask what if?" slide has
a `{{< input >}}` slider + `{js}` cell. Drag the slider off its default (8), then add a slide
heading anywhere else in the source. The whole deck re-mounts and the slider snaps back to 8.

## Goal / non-goals

**Goal.** On a structural deck edit, re-mount only the `<section>` subtrees that actually
changed. Every unchanged slide keeps its live DOM node (and thus its `{js}`/WebGL/video/input
state), while click-to-source stays exact on the preserved-but-shifted slides.

**Non-goals (v1).**
- No server change. The server keeps detecting structural edits and sending `full_render`
  exactly as today; the fix is entirely in the client re-mount path (where the audit locates
  it).
- Vertical stacks are reconciled at the top level only (owner ruling 2026-07-21): a stack
  wrapper (`<section>` holding an h1 lead + nested h2 sub-slides) is matched as one unit. If
  any sub-slide inside a stack changed, the whole stack rebuilds. Horizontal slides — the
  common case — are preserved individually. Per-sub-slide preservation is a possible later
  follow-up, not this change.
- No new headless-browser CI harness. Verification is the mandated chrome-devtools MCP loop
  plus the CI-gated `tsc` type-check (see Testing).

## Approach: client-side section reconciliation

Chosen over a server-side section-ops approach because the `<section>` grouping is a
render-time *projection* (`slides_html`), not part of the flat block model, so server-side
section diffing would reinvent diffing at a new granularity and touch the carefully-built
structural-detection path. The server already ships the complete new slide body in the
`full_render`, so the client has everything it needs to reconcile.

### Where it hooks in

In `client.js`'s `full_render` handler, the current `!skipMount` branch runs:

```js
resetJs();
keepScroll(() => { root.innerHTML = msg.body_html; });
```

Replace this branch with: if the doc is a deck (`window.TALIESIN_FORMAT === "deck"`) **and**
`root` already holds a live deck (`root.querySelector(":scope > section")` is non-empty)
**and** the incoming `body_html` parses to a section list, run `reconcileDeckSections(root,
msg.body_html)`. Otherwise fall back to the existing `resetJs()` + `innerHTML` path
(first mount, non-deck, or an unrecognizable body).

Note: for a deck, `root` is the `.tali-slides` container (`id="tali-root"`), and `body_html`
is exactly its `<section>` children (`body_html()`, `serve/mod.rs:86`). Reconciliation
therefore operates on `root`'s direct `<section>` children, and preserving `root` itself
means `.tali-deck`'s controls/listeners survive too (a strict improvement over today's
wholesale swap, which rebuilt them).

### The reconciliation algorithm

Each real slide `<section>` contains blocks that each carry a **content-hash**
`data-block-id` (position-independent: a slide that only shifted down the file keeps identical
block-ids; only its `data-sourcepos` changes). The *content signature* of a section is the
in-order join of its descendants' `data-block-id` values. Two sections with the same
signature are, by construction, byte-identical in block content and differ at most in
`data-sourcepos`.

1. Parse `body_html` into an ordered list of incoming `<section>` nodes (a `<template>`).
2. Index the **old** sections that have a **non-empty** signature into a map
   `sig -> queue of old nodes` (a queue, so duplicate signatures — e.g. repeated
   auto-animate titles — are consumed positionally). Sections with an empty signature (only
   the front-matter title slide, which is built outside the block model) are never reused.
3. Walk incoming sections in order, building the new ordered child list:
   - Compute the incoming section's signature.
   - If it is non-empty and the map has an unconsumed old node for it: **reuse** that old
     node (dequeue it). Patch click-to-source: for each descendant `[data-block-id]` in the
     reused node, copy `data-sourcepos` (and `data-source-file`, add/remove) from the
     matching incoming descendant. This is `set_meta` semantics applied within a preserved
     section, and is what keeps Alt-click exact on a preserved-but-shifted slide.
   - Else: **build fresh** from the incoming section's HTML (`fragment(...)`). This covers
     new slides, changed slides (a within-slide edit changed a block-id → new signature),
     and the title slide (empty signature → always rebuilt, which also correctly applies a
     title/subtitle edit; it holds no live state worth preserving).
4. Any old section node **not** reused → `teardownJs(node)` so its `{js}`/WebGL cells release
   (resolves each cell's `invalidation`, runs author cleanup, unregisters its inputs). This
   is the same per-element teardown the incremental `update`/`remove` ops already use; do
   **not** call the global `resetJs()`, which would kill preserved cells.
5. Replace `root`'s children with the new ordered list, inside `keepScroll(...)`. Reusing
   nodes means a reorder moves live DOM rather than rebuilding it.
6. Fall through to the existing `scheduleAfterChange()` → `afterChange()` →
   `syncDeck()`, which calls `TaliesinDeck.sync()` + `layout()` to re-read the new slide set
   and preserves the current slide + overview (as it already does across the wholesale swap).
   `enhance()` binds only the newly-built cells (`:not([data-qmd-bound])`); preserved cells
   are already bound and are skipped, so no duplicate mounts.

### Edge cases

- **Title slide** (no `data-block-id` descendants): empty signature → always rebuilt. Cleanly
  handles a `deck_meta_changed` title/subtitle edit without special casing.
- **Duplicate signatures** (two content-identical slides, e.g. an auto-animate title pair):
  the per-signature queue consumes positionally. If a mismatch occurred it would swap two
  byte-identical nodes — harmless. Documented, acceptable for v1.
- **Vertical stack**: the wrapper `<section>`'s signature spans all its sub-slides' block-ids.
  Editing one sub-slide changes the wrapper signature → the whole stack rebuilds (per the
  owner ruling). Horizontal slides around it are still preserved.
- **Unrecognizable / empty incoming body**: fall back to `resetJs()` + `innerHTML` (safe
  superset of today's behavior).
- **Error recovery / reconnect** that reaches this branch on a deck: reconciliation is
  strictly better (more state preserved) and correct, so it needs no special trigger; only
  the `deck` + existing-sections guard gates it.

## Invariants preserved

- **Single editing surface**: read-only; no write-back to source. Only DOM is touched.
- **Block-model invariants**: every block keeps its `data-block-id`; `data-sourcepos` /
  `data-source-file` are refreshed on preserved sections, so click-to-source and reverse
  cursor-sync stay exact.
- **`{js}` lifecycle**: per-section `teardownJs` mirrors the proven incremental-op path; no
  leaked WebGL contexts or RAF loops, no duplicate cell registration.
- **Do-NOT-touch**: no change to `MAX_WARM_PAGES` / `exec_pool.rs`, kernel, or the server
  structural-detection predicate.

## Testing / verification

1. **`tsc` type-check** (CI-gated, `cd web-client && npx -y -p typescript tsc -p
   jsconfig.json`): guards that the new client code type-checks.
2. **chrome-devtools MCP end-to-end** (the mandated UI loop), bug-first then fixed, using
   `corpus/deck.tmd`:
   - Preview the deck; navigate to the "Ask what if?" slide; drag the slider to a non-default
     value (the `{js}` cell recomputes its text).
   - Structurally edit the source elsewhere (e.g. insert a `## New slide` heading above), which
     triggers `deck_structural` → `full_render`.
   - Assert: the "Ask what if?" section's slider keeps its dragged value and computed text
     (preserved), and the previously-current slide stays current.
   - Assert click-to-source: Alt-click a block on a preserved-but-shifted slide navigates to
     its **new** (shifted) source line, not the stale one.
   - Verify at the three-viewport matrix per project convention (mobile ~390, laptop
     landscape ~1440, laptop portrait ~900-tall).
3. **Regression sweep**: `cargo test -p taliesin-core` + the server suite stay green (no
   server change, so this only confirms nothing regressed). Confirm the existing
   `deck_structural`/`is_slide_structural` unit tests are untouched.

There is no new automated CI regression pin for the runtime DOM behavior: the fix is
client-side DOM reconciliation, which the repo's server-side `live-edit-bench` (it measures
the BlockOp stream) cannot exercise, and the project has no headless-browser CI harness.
Adding one is out of scope for v1; the chrome-devtools MCP scenario above is the verification.

## Out of scope (v1)

- Per-sub-slide preservation inside vertical stacks (top-level ruling).
- Any server-side section-ops protocol.
- A headless-browser CI test harness.
- The other open deck-audit items (they have already landed; B3-18 is the last).

## Execution addendum (2026-07-21)

Implementation surfaced a dependency the design missed, so the "no render change" non-goal
above was **superseded** by an owner-approved scope expansion:

- The `{{< input >}}` control rendered with a **line-based** DOM id (`qin-<line>`), which was
  baked into the block's content-hash `data-block-id`. So an input block's id was **not**
  position-independent: any edit above shifted it, changing the section's signature and
  forcing that slide to rebuild — defeating the reconcile for input-bearing slides (and, more
  broadly, making an input lose DOM/JS state on any edit-above even in a normal doc, an
  `Update` where a `SetMeta` was expected). The corpus deck's only stateful slide is exactly
  such an input slide.
- Fixed at the source: the control id is now derived from the reactive **name**
  (`qin-<name>`, deduped; line-based fallback only for an anonymous control), restoring the
  `data-block-id = content hash` invariant. Commit `fix(render): derive {{< input >}} control
  id from name, not source line`; pins `input_control_id_is_position_independent` +
  `duplicate_input_names_get_deduped_control_ids`; `reactive_inputs` snapshot updated.

Verification result (chrome-devtools MCP on `corpus/deck.tmd`): a structural edit now
preserves the exact live DOM nodes of every untouched slide **including the slider slide**
(confirmed by a JS-expando that cannot come from server HTML), with `data-sourcepos` patched
(119→123 across a 4-line insert). Exercised across add-above/add-below/remove/retitle/reorder,
the vertical stack (rebuilds per the top-level ruling; unrelated slides preserved), the
auto-animate duplicate-title pair, and viewports down to the ~500px browser floor — console
clean throughout. Full `cargo test` (`-core` + server) + `tsc` + `clippy` green.
