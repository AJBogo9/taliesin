# Bare build output (`--bare`) + content-gated enhancer JS

**Date:** 2026-06-27
**Backlog item:** "Bare / plain build output (`--bare`) + default enhancer-JS gating"
**Status:** shipped (Phase 1 + Phase 2); audit-qmd reviewed (5 low/med findings, all
test/doc-coverage gaps, pinned).

## Goal

Give `build` two improvements, sequenced:

1. **Phase 1 — content-gate the separate enhancer scripts in the build path** (no
   flag, helps every build). A static build ships an enhancer's `<script>` only when
   the page actually contains the DOM it targets.
2. **Phase 2 — `--bare` build flag** (single-doc build only): a guaranteed
   zero-`<script>`, zero-CDN, CSS-only-theme HTML page. For sending a rough draft,
   archiving a doc, or feeding a future print pipeline the smallest, most robust,
   dependency-free HTML.

## Decision that diverges from the original backlog note

The backlog note assumed `code-enhance.js` (88 KB) was just "copy buttons, lightbox,
link-preview, category-filter" and proposed coarse-gating it off pure-prose pages.
**Verified false:** `code-enhance.js` carries the *entire reader experience and a11y
layer* — reader menu, theme toggle, text-size/width controls, reading progress,
highlights, bookmarks, read-aloud, **skip-link, and keyboard navigation**
(`qmdEnhancers.register(...)` at code-enhance.js:816-830).

Coarse-gating it off prose would silently strip the theme toggle, reading progress,
skip-link, and keyboard nav from the *default* `qmd-fast build` of a blog post — an
a11y/UX regression and the opposite of Phase 1's "no regression" promise.

**Author decision (2026-06-27): keep `code-enhance.js` on every build.** Phase 1
content-gates only the cleanly DOM-tied *separate* scripts. The aggressive
"~0 KB on prose" stripping is `--bare`'s job (Phase 2), where it is opt-in and the
loss is the explicit contract.

## Design seam

`enum OutputMode { Preview, Build, Bare }` (default `Preview`), carried on
`PageParts` (`render/page.rs`) and threaded from the build CLI:

```
build CLI (--bare?) → build_page_executing(mode)
                    → render_doc_to_page(doc, title, mode)
                    → page_from_doc(doc, title, mode)
                    → html_page_inner(doc, title, site, mode)
                    → assemble_html_page(PageParts { mode, .. })
```

- **Preview is untouched.** The live servers (`serve.rs`, `serve_site.rs`) build
  `PageParts` directly with `mode: Preview` → identical bytes to today. The live loop
  carries no risk.
- `render_html_page` / `render_html_page_with_includes` (the in-process full-page
  API used by tests) keep their signatures and pass `Preview` internally → no test
  churn, ship-everything behaviour preserved.
- `html_page_from_doc_in_site` hardcodes `Build` internally (every caller — site
  build, the 404 page, mounted-page static serving, `check`'s discard — is
  build/static-equivalent; the live site preview does not go through it). So **site
  builds get Phase-1 gating too** ("helps every build").
- `render` (stdout) and the site embedded-deck build pass `Build`. Decks dispatch to
  `deck_page_from_doc` regardless of mode, so mode is moot for them.

## Phase 1 — `code_scripts_for(body, mode)`

Replaces the unconditional `code_scripts()` call in `assemble_html_page`:

- **Preview**: ship every enhancer unconditionally (a doc can gain any construct on
  an edit — same reasoning as KaTeX/d3 always-on in preview). Byte-identical to today.
- **Build**: always ship `code-enhance.js`; ship each *separate* enhancer only when
  its body marker is present:
  - `mermaid.js`   ↔ `class="mermaid"`
  - `qmd-js.js`    ↔ `application/qmd-js` (`has_js_cells`)
  - `walkthrough.js` ↔ `code-walkthrough`
  - `tabset.js`    ↔ `panel-tabset`
  - `scrolly.js`   ↔ `qmd-scrolly`
  The vendored d3 + Plot in `<head>` (`js_cell_head`) already gate on `has_js_cells`.
- **Bare**: empty.

`code_scripts()` stays (delegates to `code_scripts_for("", Preview)`) for API
stability.

## Phase 2 — `--bare` (`OutputMode::Bare`, single-doc build only)

Guaranteed contract: **zero `<script>` tags, zero CDN/network refs.**

- `assemble_html_page` in Bare mode suppresses every script source it controls: no
  `theme_head` bootstrap, `code_scripts_for` → "", no `js_cell_head`, and it blanks
  the passed-in `scripts_pre`/`scripts_post` (belt-and-suspenders for the contract).
- `html_page_inner` in Bare mode passes empty `scripts_pre`/`scripts_post` (no static
  click-logger, no `qmdEnhanceCode` call, no TOC-spy/search) and **strips
  `<script type="application/qmd-js">…</script>` blocks from the body** (a `{js}`
  cell is a qmd-fast construct that is meaningless without the runtime — drop it,
  leaving its empty output container). This is what keeps the zero-`<script>`
  guarantee true even with `{js}` cells in the source.
- **Theming goes CSS-only** (native, no flash, no script). `bare_theme_css(mode)`,
  appended after `{base}{dark}{site}` (with `dark` dropped from the main style in
  Bare since the JS-set `[data-theme]` attribute never appears):
  - forced **dark** → `DARK_CSS` with `html[data-theme="dark"]` rewritten to `:root`,
    emitted unconditionally (hardcoded dark).
  - forced **light** → nothing (`base.css :root` is already light).
  - **auto** (no `theme:`) → the same rewritten `DARK_CSS` wrapped in
    `@media (prefers-color-scheme: dark) { … }` (OS-following). `dark.css` is
    uniformly `html[data-theme="dark"]`-prefixed, so the rewrite is total and clean.
  - Niche base.css `[data-theme="dark"]` rules (theme-matched `{{< video >}}`/figure
    swaps, reader-highlight bg) are not carried into Bare; the light variant shows.
    Acceptable for a draft; noted.
- Math still works (KaTeX is server-rendered CSS + bundled fonts; the stylesheet
  ships when math is present). Favicon + extension `theme:` CSS stay (not JS).

### Loud exclusions in `--bare` (warn, never silently degrade)

Emitted by the build CLI (where logging lives):
- **Deck format**: refuse with a clear message (navigation is JS). `build_page_executing`
  returns `BuildResult::Refused(msg)`; the CLI logs it and exits non-zero, writing
  nothing.
- **`{js}` cells**: warn they are inert/dropped (count from `doc.blocks`).
- **Mermaid**: warn the diagram is shown as source (the `<pre class="mermaid">`
  already carries the source text; no renderer ships).
- **`--bare` on a directory (site)**: refuse (a site's nav/search need JS); single-doc
  only for now (a bare site is coherent but deferred, YAGNI).

## Tests

**Phase 1** (`render/tests.rs`, unit on `code_scripts_for`):
- prose Build ships `code-enhance.js` (reader menu marker) but **not**
  mermaid/walkthrough/tabset/scrolly/qmd-js.
- a `panel-tabset` body in Build ships `tabset.js`.
- prose Preview ships mermaid.js (everything) — gating is Build-only.
- Bare → `code_scripts_for` is empty.

**Phase 2** (`corpus/bare-draft.qmd` + `corpus.rs`):
- `corpus/bare-draft.qmd`: prose + inline math + a server-highlighted code block +
  an image + a `{js}` cell (proves stripping) + a mermaid block (proves degrade).
- Build it in Bare mode (`render_doc_to_page(.., Bare)`) and assert:
  - **no `<script`** anywhere (the contract),
  - `class="katex"` present (math renders),
  - **no `application/qmd-js`** (the `{js}` block was stripped),
  - the auto-theme `@media (prefers-color-scheme: dark)` + `:root .qhl-` dark block
    is present.
- Server test (`main.rs`): a deck source built with `Bare` yields
  `BuildResult::Refused`.

## Out of scope / deferred

- Splitting `code-enhance.js` so each feature gates independently (the design's
  "deferred refinement"; not needed now that code-enhance.js always ships in Build).
- A bare *site* build.
- Moving the skip-link / focusable `<main>` server-side into `page.rs` (a separate
  open backlog item; bare mode is JS-free so those are simply absent in `--bare`).
