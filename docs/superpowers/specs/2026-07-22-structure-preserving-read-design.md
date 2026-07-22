# Structure-preserving, book-aware `taliesin read` (backlog item 19)

Date: 2026-07-22
Status: approved design, ready for a plan

## Problem

`taliesin read` projects a rendered document to deterministic plain text for an agent (or a
blind author) reading what it made without a browser. Three independent demand probes
(personas 1/2/3) each hit the same seam: the projection **fuses structured blocks**, so an
agent reads run-on text.

Measured on the running product (do not trust prose descriptions; these were reproduced):

- **Lists** (`corpus/tarn/api-query.tmd`): a `<ul>` block flattens every `<li>` into one
  string — `name — the column to reference.Returns — an Expr you can compare…`. A `<ul>` is
  one block; both list items share it. Root cause: `render/text.rs::project_block` falls a
  list through to `visible(html)`, which strips tags with no separator.
- **Scrolly / stepped divs** (`corpus/descent/index.tmd`): adjacent `.step` narrations merge
  across step boundaries — `…to the dark point in the middle.Which way is downhill.` — and a
  `.code-walkthrough`'s steps fuse the same way. Same `visible()`-flatten root cause.
- **Input controls** (`corpus/descent/index.tmd`): a `{{< input >}}` control's label and
  value fuse — `step size (η)0.12`. One block can hold several controls
  (`class="tali-input"` divs with a `.tali-input-label` + an `<output>`).
- **Book scoping** (`corpus/course/em.tmd`): `read` renders a file standalone, so a book
  chapter loses chapter-scoped numbering and cross-page resolution — `Recall Theorem from
  Section` (cross-page `@thm-`/`@sec-` unresolved), `By Theorem 1` (should be `3.1`). And
  `read <book-dir>` hard-errors via `directory_rejection`.

This consolidates backlog items **16 F-02** (book scoping), **17 F-03** (lists), **18 F-01**
(scrolly steps + input). It is the single most-repeated cross-persona finding.

## Non-goals / scope boundary

- **Not** item 16 F-03's other half — the `{{< embed >}}` iframe-chrome leak in the read
  projection. That stays a separate P3. (The `.code-walkthrough` half of 16 F-03 is fixed as
  a natural side effect of the general `.step` separation, but that is a bonus, not the
  target.)
- No new output format. `read` remains a **view**, HTML stays the only build target.
- No block-model change. All new projection arms read `block.html` exactly as the existing
  arms do; `data-block-id` / `data-sourcepos` are untouched.
- Whole-directory read is **parse-only** — executing a whole book's kernels is out of scope.

## Design

Two independent halves (buildable + reviewable separately).

### A. Structure-preserving projection — `crates/core/src/render/text.rs` (pure)

`project_block` gains three arms before the final `visible(html)` fallthrough. Each is pure
(HTML in → text out), unit-testable, and follows the existing bracket-tag convention already
used for `[note]` / `[figure N: …]` / `[image: …]` / `[output: …]` / `[js: …]`.

1. **Lists** (`leading_tag == "ul" | "ol"`): project each top-level `<li>` on its own line.
   - unordered → `- <item text>`; ordered → `1. `, `2. `, … counting in document order.
   - a nested `<ul>`/`<ol>` inside an `<li>` recurses with two-space indentation.
   - each item's text is `visible()` of the `<li>` inner HTML minus its nested list, so inline
     markup (bold, links, code) strips to text: `- **name** — the column` → `- name — the column`.

2. **Stepped divs** (a block containing one or more `<div>`s carrying the `step` class as a
   whole token — a `.scrolly`'s `scrolly-steps` container or a `.code-walkthrough`; the
   `scrolly-steps` container class must NOT itself be mistaken for a step, so match the class
   token `step`, not the substring): project each `.step`'s visible text as its own paragraph,
   blank-line separated. Non-step content in the same block (if any) projects around them.

3. **Input controls** (a block containing `class="tali-input"` divs): each control →
   `[input] <label> = <value>`, one line per control. Label = the `.tali-input-label` text;
   value = the `<output …data-qmd-out>` text (equivalently the control's `value=`).

The empty-map projection (`project`) stays byte-identical to today for any document that has
none of these constructs.

### B. Book-aware read — `crates/server/src/query.rs` (orchestration)

Reuses the exact sequence `crates/core/src/site/search.rs::page_fragment` is already proven on
(scoped render → resolve cross-refs), plus `Site::number_chapter`.

- **Single page (`read page.tmd`), auto-scope (default, no flag):**
  1. Walk up from the file's directory for an enclosing `_site.yml`.
  2. Not found → today's standalone render, unchanged.
  3. Found → `Site::discover_with(root, DraftMode::Include)` (Include so a `draft:` page is
     still found), match the `Page` whose `input` canonicalizes to the target, then:
     `render_document_with_includes_scoped(&src, base, site.chapter_for(page))` →
     `site.number_chapter(page, &mut blocks)` → `site.resolve_cross_refs(&mut blocks, &page.url)`.
  4. Project as today. Result matches the built page: `@thm-elbo`→"Theorem 3.1", cross-page
     `@thm-consistency`→"Theorem 2.1", `@sec-mle`→"Chapter 2", numbered headings.
  - Deliberately the **minimal pair** (`number_chapter` + `resolve_cross_refs`), *not* the full
    `Site::finish_blocks`: read stays content, with no injected "Referenced by" backlinks,
    "Cite this" box, listing cards, or book-TOC chrome.
  - `--run` still works on a single page and composes with scoping (exec runs after the scoped
    render, as today).

- **Whole book/site (`read <dir>`):**
  1. A directory with a discoverable `_site.yml` + pages → whole-project read; a non-site
     directory keeps today's helpful `directory_rejection` error.
  2. Iterate `site.pages` (already in chapter/nav order). For each: scoped render +
     `number_chapter` + `resolve_cross_refs`, then project.
  3. `--format human` (default): concatenate with a per-page header
     `===== <rel> (Chapter N) =====` (the `(Chapter N)` clause only when `chapter_for` is
     `Some`), blank-line separated.
  4. `--format json`: `{ "path": <dir>, "pages": [ { "path": <rel>, "title": …,
     "chapter": <n|null>, "text": <projection> } ] }`.
  5. Parse-only: kernel cells across the book project as source (warn once, as single-file read
     already warns). `--run` on a directory is an error that points to per-page `--run`.

Core needs no new API for B: scoping mutates `block.html` in place (as `search` already does);
the existing `RenderedDoc::body_text` / `body_text_with_js` then project the resolved blocks.

## Testing (the regression net)

- **Unit** (`render/text.rs`): one test per new arm — a list (ordered, unordered, nested), a
  `.step` sequence (separated), a multi-control `.tali-input` block (`label = value` per line).
- **Snapshot** (`crates/core/tests/text_projection.rs`): extend
  `corpus/reader/text-projection.tmd` with a list + `.step`s + an `{{< input >}}`; re-bless the
  golden snapshot (the ONLY snapshot that reshapes — the persona pins `descent.rs`/`course.rs`/
  `tarn.rs` don't assert on `read`) and add direct asserts (items separated, steps separated,
  `= ` present).
- **Integration** (new `crates/server/tests/read_book.rs`) over `corpus/course/`:
  - `read corpus/course/em.tmd` contains `Theorem 3.1` and the resolved cross-page refs
    (`Theorem 2.1` / `Chapter 2`), and NOT a bare standalone `Theorem 1` for `@thm-elbo`.
  - `read corpus/course` projects every chapter with its `=====` header and the resolved refs.
- **Mutation-check** each new test: restore the bug (drop the separator / skip `resolve_cross_refs`),
  watch the named test fail.

## Files

- `crates/core/src/render/text.rs` — three projection arms + unit tests.
- `crates/server/src/query.rs` — enclosing-site walk-up helper, per-page scoping in `cmd_read`,
  whole-directory read (human + json).
- `corpus/reader/text-projection.tmd` — extended fixture.
- `crates/core/tests/text_projection.rs` — re-blessed snapshot + asserts.
- `crates/server/tests/read_book.rs` — new book-aware integration test.

## Risks / watch-items

- **Auto-scope changes `read` output for every in-site file.** Intended (it is the item), and a
  standalone file is unaffected. The one pinned snapshot that moves is `text_projection.rs`,
  re-blessed with review.
- **`indexable_text` (Cmd-K search) shares `render::text` helpers but not `project_block`.** The
  new arms live in `project_block`, which search does not call, so search output must stay
  unchanged — verify no `indexable_text` byte drift.
- **Directory discovery cost** on `read <page>`: a cheap walk-up + one `Site::discover`. Only
  paid for a file inside a site; a loose `.tmd` short-circuits.
