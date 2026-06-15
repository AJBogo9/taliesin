# Execution Plan

Working name: **`qmd-fast`**

This is the solution and build plan for the problems described in `PROBLEM.md`.
Read that first.

## Guiding principles

- **Scope is the corpus, not Quarto's feature set.** "Done" means the author's
  real books, posts, and slides render correctly. The ~30 real documents are the
  spec. They are finite and inspectable, which is what makes test-driven,
  largely-autonomous iteration viable here (unlike a general rewrite).
- **The server is editor-agnostic; VS Code is the primary client.** All
  intelligence lives in the Rust server behind a stable websocket protocol. Thin
  clients (VS Code extension, plain browser tab) consume that protocol. Adding
  another editor later is a new thin client, not a rewrite.
- **Block-level granularity everywhere.** Source mapping, incremental re-render,
  and live-component state preservation all key off the same block + source
  position model. Build that model once, early.
- **HTML only. No PDF.** No format matrix.
- **Drive the interactive/UX parts by hand; let the pipeline be ground out
  against the corpus.** The novel, fiddly work (source mapping, block-swap,
  live-state preservation) needs human judgment. The rendering pipeline can be
  iterated against the fixed corpus with visual + structural diffing.

## Architecture at a glance

```
┌─────────────┐   websocket    ┌──────────────────────┐
│ VS Code ext │ ◄────────────► │   Rust dev server    │
│ (webview +  │   JSON proto   │                      │
│  cursor)    │                │  ┌────────────────┐  │
└─────────────┘                │  │ parser (comrak)│  │
                               │  │  + sourcepos   │  │
┌─────────────┐   same proto   │  ├────────────────┤  │
│ browser tab │ ◄────────────► │  │ block model +  │  │
│  (preview)  │                │  │ incremental    │  │
└─────────────┘                │  │ diff/render    │  │
                               │  ├────────────────┤  │
                               │  │ kernel pool    │  │
                               │  │ (Jupyter/ZMQ)  │  │
                               │  └────────────────┘  │
                               └──────────────────────┘
```

The Rust server owns all logic. VS Code is the primary client; the browser tab
is a near-free secondary client speaking the same protocol minus the editor
glue.

## The protocol (define this first — it is the contract)

A small, **versioned** set of websocket messages.

**Server → client**

- `full_render { html, css, blocks: [{ id, sourcepos }] }`
- `block_update { id, html, sourcepos }`
- `block_remove { id }`
- `block_insert { id, html, sourcepos, after_id }`
- `kernel_status { state }`
- `error { block_id?, message }`

**Client → server**

- `open { path }`
- `source_changed { path, full_text }` — simplest contract; the server diffs
  internally.
- `execute { block_id }`
- `click_block { block_id }` — the browser preview asking "where is this?"; the
  server replies and the client opens the editor (or a `vscode://` URI).

**Editor-specific glue (VS Code only)**

- The webview posts `{ type: "goto", sourcepos }` to the extension host, which
  calls `revealRange` + sets the selection. This is the **only** per-editor code.

## Two decisions to pin down before Phase 1

1. **Block-ID strategy.** Content-hash vs. path+ordinal. Content-hash (with a
   positional tiebreak) survives reordering better and is the recommended
   default. This decision ripples through diffing, state preservation, and source
   mapping, so make it first.
2. **OJS / live-component handling.** OJS has its own reactive runtime. Default
   for v1: keep emitting the same `<script>` plumbing Quarto does and treat OJS
   and Three.js blocks as **opaque live blocks that are never swapped unless
   their source changed**. Do not try to do something custom in v1.

---

## Phase 0 — Repo skeleton & corpus

**Goal:** New repo; real documents available as the test corpus; nothing renders
yet.

- New repository. Cargo workspace:
  - `crates/core` — parser + block model + render
  - `crates/server` — dev server, websocket, file watcher, kernel pool
  - `extension/` — VS Code extension (TypeScript)
  - `web-client/` — browser preview client (vanilla JS)
- Copy 3–4 representative real documents into `corpus/`: one prose-heavy blog
  post, one with KaTeX + a Three.js/OJS demo, one reveal.js deck, one multi-file
  book stub.
- Decide the block-ID strategy now (see above).
- Snapshot Quarto's current HTML output for each corpus doc into
  `corpus/expected/` as a **reference**, not a byte-exact oracle. Cosmetic diffs
  (whitespace, attribute order, class names) are expected and must not be treated
  as failures.

**Exit:** `cargo build` works; corpus + reference outputs committed.

## Phase 1 — Parse → HTML with source positions (one blog post, no execution)

**Goal:** The load-bearing pipeline. Static markdown → HTML, with every block
carrying a `data-sourcepos` attribute.

- Parse with **comrak** with `sourcepos` enabled (comrak's AST nodes carry
  line/column ranges natively — this is the reason to pick it over
  pulldown-cmark for this project).
- Walk the AST and emit **your own** HTML — do not use comrak's built-in HTML
  emitter, because you need control to inject attributes now and to swap blocks
  later. Treat top-level AST nodes as "blocks."
- KaTeX: render math server-side. Either a Rust KaTeX port or a single
  persistent Node KaTeX process (cheap, warm). Handle inline `$...$` and display
  `$$...$$`.
- Emit `data-block-id` and `data-sourcepos` on each block's root element.
- One-shot CLI mode: `qmd-fast render post.qmd > out.html`. Open and compare
  visually against the Quarto reference.

This is where corpus-driven iteration is most effective: render → diff structure
against the reference → fix → repeat across the prose posts.

**Exit:** Prose blog posts render correctly (inspection is the judge), with
source-position attributes present on every block.

## Phase 2 — Dev server + block-swap (browser preview, no editor yet)

**Goal:** Save the file → only changed blocks update in the browser, scroll
preserved.

- Long-running server (e.g. axum), websocket, file watcher (e.g. the `notify`
  crate).
- Block model + diff: on `source_changed`, re-parse, diff the block list against
  the previous one by block-ID, and emit `block_update` / `block_remove` /
  `block_insert`.
- Browser preview client (~150 lines of vanilla JS): on `block_update`, find the
  element by `data-block-id` and replace its `outerHTML`. Preserve scroll.
- **State-preservation rule (the fiddly, attention-worthy part):** if a block's
  source is unchanged, never touch its DOM. Because blocks are diffed by ID +
  source position, unchanged blocks are already identified. This is what protects
  Three.js canvases and OJS cells from reinitializing.

**Exit:** Editing prose and saving updates one block in the browser tab with no
full reload and no scroll jump.

## Phase 3 — VS Code extension (primary client)

**Goal:** Preview lives in VS Code; click-to-source works.

- Extension hosts a webview panel that loads the same preview client, pointed at
  the server's websocket.
- Click handler in the webview reads `data-sourcepos` and posts `goto` to the
  extension host, which calls `revealRange` + sets the selection in the real
  editor. **This delivers Problem 1's fix (click-to-source).**
- Optional reverse direction: editor cursor movement highlights the corresponding
  block in the preview (scroll-sync). Defer if it adds complexity.
- Save-triggered block-swap flows into the webview via the same protocol.

**Exit:** The daily loop works — edit in VS Code, see in-place updates in the
webview, double-click the preview to jump to source.

## Phase 4 — Live execution via warm Jupyter kernel

**Goal:** Code cells execute against a persistent kernel; outputs render and
participate in block-swap.

- Kernel pool: start a Jupyter kernel (e.g. via `runtimelib` / a Jupyter client
  over ZMQ) and keep it warm. One kernel per language used (likely just Python).
- Map code blocks → execute requests; capture rich outputs (text, images, HTML,
  `application/json`). Render each output as its own block whose ID is tied to
  the source cell.
- **Execution granularity:** on save, re-execute only cells whose source changed,
  plus downstream cells. Start with simple notebook semantics — re-run a changed
  cell and everything after it. Add real dependency tracking later only if it is
  too slow.
- Source mapping for computed output is block-level (cell → output), which is
  expected and acceptable.
- The warm kernel resolves Problem 3 (startup speed) fully.

**Exit:** Editing a code cell and saving re-executes only that cell (and
dependents), swaps the output block in place, and never cold-starts the kernel.

## Phase 5 — reveal.js output

**Goal:** Same pipeline, slide template.

- Slide-splitting logic (`---` and heading-level rules → nested `<section>`s).
- Emit reveal.js boilerplate + init script; wire the theme.
- Block-swap during editing should still work within the current slide.
- Skip slide PDF export.

**Exit:** Decks render and live-update.

## Phase 6 — Books (defer until actually wanted)

**Goal:** Multi-file projects, cross-references, numbering, TOC.

- Project model: multiple `.qmd` files → a linked output with shared nav/TOC.
- Cross-references + auto-numbering (figures, sections, "see Chapter 4") — the
  real bookkeeping. Implement a reference-resolution pass over the whole project.
- Optional search index.

**Exit:** A real book from the corpus builds.

---

## Mapping of phases to the original problems

| Problem | Resolved in |
|---|---|
| 1 — No source mapping | Phase 1 (sourcepos attributes) + Phase 3 (editor jump) |
| 2 — Full-page re-render | Phase 2 (block-swap) + state-preservation rule |
| 3 — Slow startup | Phase 2 (long-running server) + Phase 4 (warm kernel) |

## Where to spend human attention vs. let it grind

- **Grind against the corpus (low human attention):** the rendering pipeline in
  Phase 1, edge cases in markdown → HTML, KaTeX handling.
- **Drive by hand (high human attention):** the protocol design, the block-swap
  + live-state preservation logic, click-to-source UX, and the kernel
  integration. These have no reference output to diff against; "correct" is a UX
  feel, and the integration is fiddly across Rust + websocket + browser.

## Suggested first commits

1. Cargo workspace skeleton + empty crates (`core`, `server`).
2. `corpus/` with real documents + `corpus/expected/` Quarto snapshots.
3. Protocol types as Rust structs and matching TypeScript types (the contract).
4. comrak parse → custom HTML emitter with `data-sourcepos` (Phase 1 core).
5. One-shot CLI `render` command.
