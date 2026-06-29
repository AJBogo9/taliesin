# Architecture: four zoom levels for a legible codebase

**Date:** 2026-06-29
**Status:** design (blueprint for an incremental, file-by-file restructure)
**Goal:** make qmd-fast readable at *different levels of abstraction*, so one human
can hold the project in their head by reading at the right altitude instead of
drowning in 2,000-line files.

## Why

The project already has a clean **top** (three crates, a clear pipeline) and clean
**leaves** (small, well-named functions). What it lacks is the **middle**: when you
open a subsystem like `render` you want to land on a one-screen map and then choose
where to descend; instead you hit a 2,000-line `mod.rs` wall. There is an L0 and an
L3 but no navigable L1 -> L2. That missing middle is exactly what makes the codebase
feel un-processable.

A code-health audit (2026-06-29, 37 agents, each finding adversarially verified)
confirmed the shape of the work: **zero high-severity defects, load-bearing
invariants intact and corpus-enforced.** The only real debt is *breadth-of-file* —
ten files in the 1.1k-2.6k line range that have outgrown single-module cohesion.
This is a polish-and-layer pass, not a rescue.

## Non-goals

- **Not** a behavior change. Every split is a *move*, verified byte-stable where the
  output is observable (corpus render tests, `protocol_contract`, determinism tests).
- **Not** one-function-per-file. That is the same legibility problem inverted: you
  trade scrolling one file for hopping thirty, and you sever cohesive logic. The unit
  of a module is a **responsibility**, not a function.
- **Not** a refactor of the Do-NOT-touch zone (`exec.rs`, `kernel.rs`, `freeze.rs`).
  Their cache-key chains, warm-prefix sync, and kernel-death handling are load-bearing
  invariants; they are deferred (see "Out of scope").
- **Not** reformatting untouched lines or "improving" code while moving it.

## The four zoom levels

```
L0  WHOLE SYSTEM     .qmd ──▶ [ core: pure transform ] ──▶ HTML
    (3 crates)       …watched, executed & served by [ server ] ──▶ viewed by [ web-client ]

L1  SUBSYSTEMS       core:   render (parse→block-model→emit) · diff · cite · math · highlight ·
    (the "chapters")         includes · frontmatter · site · diagnostics · schema · prose
                     server: cli* · check* · build* · query* · serve · serve_site ·
                             exec · kernel · freeze · warm_pool       (*emerge from the main.rs split)

L2  MODULES          render/{ frontmatter · includes · xref · cells · math · toc · attrs · html · scripts }
    (inside a        cite/{ parse · format · clean · author · render · validate }
     chapter)        diagnostics/{ headings · anchors · assets · bibliography · media · links · reactive · a11y }

L3  FUNCTIONS        the leaves
```

You should be able to **stop at any level and trust it without descending**. That is
the whole point: L0 tells you the system is a pure transform wrapped by a runtime and
a view; L1 tells you which chapter owns a concern; L2 tells you which module inside
that chapter; only then do you read functions.

## What makes a level real: the module contract

File boundaries alone do not create abstraction levels. The mechanism is a **contract
at the top of every L1/L2 module** — a short `//!` doc comment answering three
questions:

1. **What does it do?** (one sentence)
2. **How do you use it?** (the public surface — the few `pub`/`pub(crate)` items a
   caller touches)
3. **What does it depend on?** (which sibling modules / crates it leans on)

That contract is the promise that lets a reader remain at this altitude. A module
whose contract you cannot write in three lines is doing too much and wants splitting.

## The four rules

1. **A module is a responsibility, not a function.** Group the functions that change
   together for one reason and share private types/helpers. `render/divs.rs` (the
   whole `:::`-fenced-div machine) and the proposed `cite/parse.rs` are the model.
2. **Soft size triggers, never split for size alone.** ~400-600 lines = "look at me,
   have I accreted a second responsibility?"; ~800+ = "split unless there's a reason
   not to." But a tightly-coupled 700-line orchestrator (`render_internal_impl`,
   `exec.rs`'s cache-key chain) stays whole — splitting it would sever invariants.
3. **Dependencies point one way: client → server → core; core does no IO.** That
   directionality is itself an abstraction level — you can understand `core` without
   ever opening `server`. No new edge may reverse it.
4. **Use the module-directory pattern the project already uses.** A `mod.rs` holding
   the contract + shared types + re-exports, plus sibling files; submodules
   `use super::*` to see the parent's privates, so cohesion-based extraction costs
   almost no boilerplate. `render/` and `site/` already work this way.

## Target L2 layout per large file

Each is a **module directory** replacing today's single file; the public surface is
re-exported from `mod.rs` so external callers are unchanged. Risk and Do-NOT-touch
notes come from the verified audit.

### Tier 1 — low risk, mechanical (do first)

**`core/src/diagnostics.rs` (1121) → `diagnostics/`** — one file per validator family,
all re-exported from `mod.rs` so `render`/`site` callers are unchanged.
`headings · anchors · assets · bibliography · media · links · reactive · a11y` +
`helpers.rs` (shared sourcepos/ref-classification) + `mod.rs` (contract + re-exports +
the integration tests). Validators are self-contained, share only stateless helpers,
outside the Do-NOT-touch zone, tested through public fns. **Lowest-risk opener.**

**`core/src/cite.rs` (1563) → `cite/`** — six orthogonal responsibilities:
`parse.rs` (BibTeX + `@string`), `format.rs` (IEEE per-type formatters),
`clean.rs` (LaTeX accent/macro resolution), `author.rs` (name formatting),
`render.rs` (HTML walk + citation/xref rendering, keep the `RefCell` location
tracking internal here), `validate.rs` (`validate_xrefs`), `mod.rs` (types +
re-exports). Watch the `format.rs → clean.rs` dependency. Rich existing test suite.

### Tier 2 — medium risk, sensitive invariants (verify carefully)

**`server/src/main.rs` (2604) → `cli/` + `check/` + `build/` + `query/`** —
`cli/{mod,init,serve}`, `check/{mod,format}`, `build/{mod,bare,assets,js_imports,site,mirror}`,
`query/{mod,preview}`. The `cli`/`check`/`query` extractions are genuinely low-risk;
**`build/site.rs` is do-not-touch-internals**: relocate verbatim — the `PageOutcome`
deferred-warning order, semaphore/`JoinSet` permit lifecycle, warm-pool budget split,
and `Arc` drop timing are load-bearing. `mirror_assets` symlink-cycle dedup moves
intact. Re-run `parallel_build_determinism.rs` after.

**`core/src/render/mod.rs` (2046) → extract leaf helper groups, keep the orchestrator** —
new siblings `frontmatter · includes · xref · attributes · cells · math · toc ·
script · html · render_cells`; **`render_internal_impl` stays the orchestrator in
`mod.rs`.** Do NOT move the central loop or `FlatBlock` emission order; pass
`heading_slugs`/`xref_registry`/`id_counts`/`origins` as args rather than sharing them
across a module boundary. Safety net: `reverse_sync_sourcepos_is_total` + the
data-block-id/sourcepos format tests after every extraction. This is the project's
most sensitive module — go slow.

**`core/src/site/mod.rs` (1403) → `listings · pages · chapter · links · discovery`** —
keep `Site`/`Page` types, `discover()`, `page_chrome()`, `render_page_doc()` in
`mod.rs`. **`links.rs` (`rewrite_qmd_links`, PUBLIC — the server calls it) is the
high-risk piece**: an href bug breaks cross-page links and source preservation. Keep
`chapter::section_number()` `pub(crate)` for `xref.rs`. Do after `render/` (both touch
xref machinery).

**`server/src/serve.rs` (1534) → `serve/{mod,http,page,ws,security,watch}`** —
**`watch.rs` is the only place orchestrating `executor.run() → diff → broadcast`**:
preserve the exact lock sequencing (`rebuild_guarded` under `app.doc.lock()`,
diagnostics inside the lock), keep the `catch_unwind`/`AssertUnwindSafe` panic guard
cohesive, do not alter the `restart_kernel`/kick channel flow. Consumes `exec`, must
not change `exec` semantics.

**`server/src/serve_site.rs` (1210) → `serve_site/{mod,types,http,render,websocket,messages,exec_pool,watch}`** —
`ExecPool` LRU/MRU eviction + warm-pool binding stay deterministic; `build_page`
preserves the exact broadcast order (diff → style → diags); the ~80ms debounce in the
watcher is timing-sensitive. The `protocol_contract` test pins `op_json` wire shape
against the client `@typedef` — move `messages.rs` carefully and keep that test.

### Tier 3 — browser-verified JS (one focused session each)

**`assets/js/code-enhance.js` (1884) → one file per reader feature** over a shared
`code-enhance-core.js` (registry + clipboard/text-fragment/cite utils): `anchors ·
focus · keyboard · reader-menu · reader-prefs · reading-progress · highlights ·
highlight-index · bookmarks · category-filter · copy-buttons · lightbox · link-preview ·
read-aloud`. No ES modules — globals stay namespace-isolated; **preserve script load
order** so the registry + reader-menu init before dependents. chrome-devtools verify:
focus mode, read-aloud, lightbox, highlights still mount.

**`web-client/client.js` (1152) → `client/` modules by message/feature** —
`ws · dom-utils · diagnostics · error-overlay · dev-menu · progress · toc · deck ·
click-to-source · reverse-sync · lifecycle · mod`. **Ordering-sensitive**: the
`renderOk() → DOM mutation (keepScroll, teardownJs) → afterChange()` sequence must be
preserved; `{js}` teardown-before-replace prevents WebGL leaks; the build-error latch
must not flip to idle early; click-to-source must keep filtering dev-menu controls.
Vanilla JS (`// @ts-check`): verify with `tsc` AND a chrome-devtools round-trip
(alt-click nav, incremental update, deck mode). **Lowest priority** — browser-only,
best done in one focused session.

### Deferred — Do-NOT-touch

**`server/src/exec.rs` (1694), `kernel.rs` (1052), `freeze.rs`** — leave whole. If ever
revisited, the only candidates are the pure leaves (`exec/progress.rs` observation,
`exec/output_block.rs` formatting), and only *after* property tests for cache-key
stability exist. The cumulative-hash keys, warm-prefix `ran` sync, error-suppression,
and mid-run kernel-death handling are invariants; splitting risks stale reads and cache
corruption. **No runtime-behavior change permitted.**

(`assets/js/deck.js` (1592) is also large; its modularization analysis did not complete
in the audit, so it is left unscheduled pending its own pass.)

## Sequencing & verification

**One file per session, lowest-risk-first, each fully verified before the next:**

1. `diagnostics.rs` → `diagnostics/`  *(Tier 1; proof-of-pattern)*
2. `cite.rs` → `cite/`  *(Tier 1)*
3. `main.rs` → `cli/ check/ build/ query/`  *(Tier 2; build/site.rs verbatim)*
4. `render/mod.rs` → leaf-helper siblings  *(Tier 2; most sensitive)*
5. `site/mod.rs` → `listings/pages/chapter/links/discovery`  *(Tier 2)*
6. `serve.rs` → `serve/`  *(Tier 2)*
7. `serve_site.rs` → `serve_site/`  *(Tier 2)*
8. `code-enhance.js` → per-feature files  *(Tier 3; browser-verify)*
9. `client.js` → `client/`  *(Tier 3; browser-verify)*
10. **Capstone:** distill this map into `docs/internals/architecture.qmd` (the
    dogfooded book) once the L2 structure is real, so the human-facing doc describes
    the actual post-split shape rather than an aspiration.

**Verification gate per split** (the tree must stay known-good):
- `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -D warnings`.
- `cargo test --workspace` green — in particular the corpus invariants
  (`crates/core/tests/corpus.rs`: data-block-id + data-sourcepos totality,
  `reverse_sync_sourcepos_is_total`), `protocol_contract`, and
  `parallel_build_determinism` (note: this one has a known load-sensitive flake —
  see backlog — so a failure must be triaged, not assumed new).
- For JS splits: `tsc` clean + a chrome-devtools round-trip.
- Each split is a **single logical change**; the public surface (re-exported from
  `mod.rs`) stays identical so callers don't move in the same commit.

## Decomposition

Each numbered split is its **own sub-project** (its own implementation plan, change,
and verification). This document is the umbrella blueprint and the definition of
"done" for the structure. We start with #1 (`diagnostics.rs`) to prove the pattern,
then proceed in order, reassessing after each.

## Guardrails (the invariants no split may break)

- Every emitted block keeps `data-block-id` + `data-sourcepos` (and `data-source-file`
  for includes). Source mapping, incremental re-render, and live-state preservation
  all key off this.
- The `.qmd` file stays the single editing surface; the preview never writes back.
- Output stays HTML-only.
- Warm server + warm kernel: no per-edit startup cost; no change to kernel/exec/freeze
  runtime behavior.
- The `:::`-div machine (`render/divs.rs group_divs`) and the block model are sensitive
  — move, don't restructure.
