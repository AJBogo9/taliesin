# UI-Audit Harness — Design Spec

**Date:** 2026-07-08
**Status:** approved (Approach A), implementing
**Location of the tool:** `tools/ui-audit/` (own Node project, outside the cargo workspace)

## Goal

A re-runnable harness that finds UI bugs across Taliesin's *entire* rendered
surface (~214 `.tmd` → many HTML pages) at **3 viewports × 2 themes**, catching
three bug classes: **visual/layout**, **console/network errors**, and
**interaction** breakage. Accessibility is explicitly out of scope for this pass.

Deliverable for this session is the **harness itself**, verified end-to-end on a
small slice. The full 214-page burn is triggered by the author later.

## The reframe (why this shape)

The browser is a serial bottleneck: one chrome-devtools MCP server drives one
shared Chrome, and subagents inherit that single connection, so fanning out
browser-driving agents just makes them contend over one tab set. Bug-finding has
three phases and only the middle one is browser-bound, so we split along that grain:

1. **Capture** — serial but *scripted and cheap* (a headless Puppeteer loop, no
   agents). This is where "wide" comes from: one pass dumps artifacts for every
   page × viewport × theme.
2. **Analyze** — massively parallel, *no browser* (agents read the captured
   screenshots + logs). This is the real fan-out.
3. **Verify** — adversarial, per unique finding, killing plausible-but-wrong ones.

Capture once, cheaply; throw as many eyes at the artifacts as we like; then
confirm. This beats N parallel browsers.

## Architecture / pipeline

```
taliesin build (per unit)  ──►  static _site/_book trees   [deterministic, no agents]
        │
   enumerate pages (glob *.html → map back to .tmd)
        │
   serve over http://127.0.0.1  ──►  Puppeteer capture     [serial, scripted, headless]
        │                              screenshot + console + network + dom-flags
        ▼
   .work/artifacts/**  +  manifest.json
        │
   audit.workflow.js (Workflow tool):  analyze ─► dedup ─► verify ─► report   [parallel agents]
        ▼
   report.md  (+ optional Artifact gallery)

taliesin preview (representative units)  ──►  Puppeteer interaction probe       [serial, live server]
        ▼                                       deck / search / lightbox / hover / toc / click-to-source
   .work/probe-results.json
```

Only the two Puppeteer steps spend the browser; only the Workflow spends agents.

## Stage 1 — Enumerate + build the surface

**Build units** (each built with `--out` into a scratch dir so nothing writes into
the tracked tree beyond the already-gitignored `_freeze/`):

Six site-project units:

| unit | command | format |
|---|---|---|
| website | `taliesin build site --out .work/build/site` | website |
| guide book | `taliesin build docs/guide --out .work/build/docs-guide` | book |
| internals book | `taliesin build docs/internals --out .work/build/docs-internals` | book |
| bayesian website | `taliesin build corpus/bayesian-website --out .work/build/bayesian-website` | website |
| demo book | `taliesin build corpus/demo-book --out .work/build/demo-book` | book |
| tech-blog | `taliesin build corpus/tech-blog --out .work/build/tech-blog` | website |

Standalone single-doc units — every other `.tmd` under `corpus/` outside those
roots and outside the exclude list, each `taliesin build <file> --out
.work/build/<slug>` (writes `<slug>/index.html`):

```
corpus/bare-draft.tmd  corpus/deck.tmd  corpus/native-tmd.tmd
corpus/posts/born-machines.tmd  corpus/posts/em-algorithm/index.tmd
corpus/posts/pca-geometry/index.tmd  corpus/posts/fourier-transform/index.tmd
corpus/posts/cite-coverage/index.tmd
corpus/callouts/kinds.tmd  corpus/layout/panels.tmd  corpus/media/gallery.tmd
corpus/reactive/graph.tmd  corpus/reactive/inputs.tmd  corpus/reactive/js-error.tmd
corpus/explorable/scrolly.tmd  corpus/narrate/walkthrough.tmd
corpus/refs/theorems.tmd  corpus/refs/theorems-shared.tmd
corpus/refs/theorems-unnumbered.tmd  corpus/refs/theorems-interactive.tmd
corpus/reader/hovercards.tmd  corpus/reader/long-read.tmd  corpus/reader/preferences.tmd
corpus/diagnostics/a11y.tmd  corpus/diagnostics/check-superset.tmd
corpus/diagnostics/links.tmd  corpus/diagnostics/prose.tmd  corpus/diagnostics/typos.tmd
corpus/render-fixes/index.tmd
```

**Exclude list** (include-only partials + embed-only decks that must not be built
as standalone pages; the embed decks are still captured because the *site* build
emits them as `tour.html`/`demo.html` inside the output tree):

- `corpus/_includes/*`, `corpus/tech-blog/_includes/*`,
  `corpus/bayesian-website/subsections/_*.tmd`, `site/_includes/*`
- embed-only: `docs/guide/tour.tmd`, `docs/guide/demo.tmd`, `site/demo.tmd`

**Route↔source mapping** is a literal, structure-preserving extension swap (no
slugification anywhere in the pipeline). So: build each unit, glob
`.work/build/<slug>/**/*.html`, and swap `.html`→`.tmd` onto the unit's source
root to recover `sourceFile`. `format` = `book` if the unit's `_site.yml` has
`chapters:`, else `website`; the three embed decks are special-cased to `deck`;
standalone `format` comes from the file's own front matter.

**Gotchas (resolved):**
- **`mounts:` are preview-only.** `taliesin build site` does *not* pull in
  `docs/guide`/`docs/internals` (it only warns). So building the units above yields
  no route overlap. We must *not* crawl `taliesin preview site` alongside the docs
  builds, or we'd double-capture the docs under `/docs/guide/*`.
- **No kernel needed for a green build**; missing `TALIESIN_PYTHON`/`_R` degrades
  code cells to source-with-warning. Executed `{js}`/reactive output will be
  absent without a kernel — acceptable, flagged in the report.
- **`_freeze/` is a content-hashed cache** next to each source root; repeat runs
  replay it. Set `TALIESIN_NO_CACHE=1` for guaranteed-fresh execution.
- **`{js}` cells need real HTTP** (relative `import()` blocked under `file://`),
  hence the static server rather than `file://`.

## Stage 2 — Capture (Puppeteer, headless, no agents)

- **Chrome:** `puppeteer-core` launched against `/usr/bin/google-chrome`
  (`executablePath`, `headless: 'new'`, `--no-sandbox`). No Chromium download.
- **Serve:** a tiny static server rooted at each unit's build dir, one unit at a
  time on a fixed port (per-unit serving sidesteps cross-project relative-path
  concerns — links inside a `_site/` tree are relative, so serving that tree at `/`
  is correct).
- **Matrix:** viewports `{390×844 (mobile), 1440×900 (laptop), 900×1440
  (portrait)}` × themes `{light, dark}`.
- **Theme forcing** (dominant lever, seeded before load via `evaluateOnNewDocument`):
  ```js
  localStorage.setItem('qmd-theme', mode);        // single-doc + site/book pages
  localStorage.setItem('qmd-deck-theme', mode);   // standalone decks (separate key)
  ```
  plus `emulateMediaFeatures([{name:'prefers-color-scheme', value: mode}])` as a
  belt-and-braces default. localStorage wins over front-matter `theme:` and OS, so
  this forces both directions deterministically. (Requires the http origin, hence
  the static server.)
- **Settle** before every screenshot (KaTeX + highlighting are SSR, so no wait
  there; web fonts, images, mermaid, `{js}` cells, deck layout do need waiting):
  ```js
  await page.goto(url, { waitUntil: 'networkidle0' });
  await page.evaluate(() => document.fonts.ready);
  await page.waitForFunction(() => {
    const imgsOk    = [...document.images].every(i => i.complete);
    const mermaidOk = !document.querySelector('pre.mermaid:not([data-processed])');
    const jsOk      = [...document.querySelectorAll('.tali-js-cell')]
                        .every(c => { const o = c.querySelector('.tali-js-out');
                                      return !o || o.childElementCount > 0; });
    const deck      = document.querySelector('.tali-deck');
    const deckOk    = !deck || (window.TaliesinDeck?.isReady?.()) ||
                        deck.classList.contains('tali-ready');
    return imgsOk && mermaidOk && jsOk && deckOk;
  }, { timeout: 8000, polling: 100 });
  ```
  On timeout: log + screenshot anyway (never hang the run).
- **Per (page, viewport, theme) artifact** under
  `.work/artifacts/<unit>/<route-slug>/<viewport>__<theme>.*`:
  - `screenshot.png` (full-page).
  - `console.json` — collected `page.on('console')` + `page.on('pageerror')`
    (type, text, location).
  - `network.json` — `page.on('requestfailed')` + responses with `status >= 400`
    (url, status, resourceType).
  - `dom-flags.json` — cheap heuristics that *pre-flag* pages for the vision
    agent (they are hints, not verdicts): horizontal overflow
    (`documentElement.scrollWidth > innerWidth + 1`), elements past the right edge,
    broken images (`complete && naturalWidth === 0`), plus page `<title>` and
    counts of console errors / failed requests.
- Writes `.work/artifacts/manifest.json`: one record per captured cell with the
  paths above + `{unit, route, sourceFile, format, viewport, theme, errorCount,
  failedRequests, domFlagCount}` for triage/prioritisation.

Decks are captured in their **default page (scroll/reader) view** — that's what a
reader sees; slide navigation is an *interaction*, handled in Stage 3.

## Stage 3 — Interaction probe (Puppeteer against a live `taliesin preview`)

Runs on **representative pages per feature** (these behaviours are shared client
JS; testing them on all 214 pages finds nothing 214 times). Uses `taliesin
preview <target>` because click-to-source needs the live server's
`window.TALIESIN_DOC` + websocket, which a static build lacks. Each probe asserts
a concrete outcome and records pass/fail + evidence into
`.work/probe-results.json`.

| feature | representative | trigger | assertion |
|---|---|---|---|
| deck nav | `corpus/deck.tmd` | load `?qmd=present` (forces step mode; scroll mode no-ops keys), press `ArrowRight` | active leaf `section:not([inert])` changes; `TaliesinDeck.getIndices()` advances |
| search | `docs/internals` (book) | `window.taliOpenSearch()`, type into `.tali-s-input`, dispatch `input` | `#tali-search` visible, `#tali-s-results li.tali-s-item` non-empty for a known term |
| lightbox | `corpus/media/gallery.tmd` | click `figure img`, press `ArrowRight` | `#tali-lightbox.open`, `img.src` changes, caption shows `(n / N)` |
| hover-preview | `corpus/demo-book` (`results.tmd`) | mouseover an `a.tali-xref` | `#tali-link-preview.open` populates from `TALIESIN_HOVER_INDEX`; **and** an Alt-click *inside* the card does NOT emit `click_block` (source attrs stripped) |
| toc scrollspy | `docs/internals/architecture` | scroll | active `#TOC a` gains `.tali-toc-active` |
| click-to-source | any preview page | Alt-hover a `[data-block-id]`, then Alt-click | `.tali-alt` on `<html>` + `.tali-src-hover` on block; Alt-click sends WS `{type:"click_block",…}` (capture via CDP `Network.webSocketFrameSent`) and attempts `vscode://file…` nav (capture via request interception + abort) |

## Stage 4 — Analyze → dedup → verify → report (`audit.workflow.js`, Workflow tool)

Reads `manifest.json` + `probe-results.json`. Bug classes are treated by their
epistemic type:

- **console/network findings are *facts*** (the log says 404) → no adversarial
  refutation, just JS-level exact-dedup by normalized message + source attribution.
- **visual findings are *judgments*** (agent thinks spacing is broken) → the
  adversarial-verify budget concentrates here.
- **interaction findings are *assertions*** from the probe → fact if the probe is
  correct; reported directly with the probe evidence.

Pipeline:

1. **Analyze** (`pipeline`, parallel): batch pages by unit/group so one agent sees
   *all six matrix cells of a page together* (needed to judge responsive/theme
   bugs). Agent reads screenshots (Read renders PNGs) + the JSON logs and returns
   schema-forced findings: `{unit, route, viewport, theme, bugClass, severity,
   title, description, evidence, suspectedSelectorOrSource}`.
2. **Dedup** (barrier): JS pre-groups exact-duplicate console/network errors; then
   a clustering agent groups the *visual* findings by root cause across pages (the
   same shared-CSS bug on 40 pages → one finding with a 40-instance list).
3. **Verify** (`parallel` over unique visual/interaction findings): adversarial
   verifier per finding, prompted to *refute*, working from the screenshot re-read
   + a source trace (grep `emit.rs`/`base.css`/`client.js`/theme CSS). Confirmed
   only if it survives. Console/network facts skip this and go straight to report.
4. **Report** (synthesis agent): ranked `report.md` — each finding with severity,
   affected-page count + instance list, repro, screenshot path, suspected source
   location. Optional Artifact gallery (top findings, thumbnails inlined as data
   URIs) offered separately since a full-run gallery would be huge.

## Harness layout

```
tools/ui-audit/
  package.json           # { type: module, deps: puppeteer-core }
  README.md              # how to run + flags
  .gitignore             # .work/
  lib/
    units.mjs            # build-unit list + exclude list + enumeration
    build.mjs            # run taliesin build per unit → page records
    serve.mjs            # minimal static http server
    browser.mjs          # puppeteer-core launch (system chrome) + theme/settle helpers
    capture.mjs          # the matrix capture loop → artifacts + manifest
    probe.mjs            # interaction probe against `taliesin preview`
  capture-run.mjs        # CLI: build + serve + capture   (flags below)
  probe-run.mjs          # CLI: interaction probe
  audit.workflow.js      # Workflow: analyze → dedup → verify → report
```

**Flags** (`capture-run.mjs`): `--only <glob>` (subset of units — this is what
makes "verify on a slice" and "run everything" the same code path), `--viewports`,
`--themes`, `--out <dir>`, `--bin <taliesin>`, `--no-build` (reuse existing build),
`--jobs <n>`.

## Verification plan (acceptance for "harness only")

Run the *whole* pipeline on a ~4-unit slice spanning every format:
`corpus/posts/born-machines.tmd` (post), `corpus/deck.tmd` (deck),
`corpus/media/gallery.tmd` (figure gallery), `corpus/demo-book` (book, 1–2 pages).
Success =
1. build + serve succeed; the slice enumerates the expected pages;
2. every matrix cell yields a valid screenshot + the three JSON logs;
3. the interaction probe passes its assertions on the representatives;
4. `audit.workflow.js` runs all four phases and emits a coherent `report.md`;
5. it catches ≥1 **real** issue (confirmed against ground truth, not just "it ran").

Then hand off the full-run command + `--only`/flags.

## Risks / open items (resolved during honing)

- Theme-forcing: resolved — localStorage seed dominates; media-emulation alone is
  insufficient for pages with explicit front-matter `theme:`.
- Settle: resolved — SSR math/highlight need no wait; predicate above covers
  fonts/images/mermaid/`{js}`/deck.
- click-to-source only works under `preview` (not static build): resolved — probe
  runs against `preview`.
- puppeteer-core vs Chrome 150 CDP compat: low risk for navigate/screenshot/
  evaluate; validated during slice run.
- Stale CLAUDE.md deck naming (`.qmd-deck`/`QmdDeck`): note for separate cleanup,
  out of scope here; harness targets the live `tali-*`/`TaliesinDeck` names.

## Out of scope

Accessibility auditing; fixing any bugs found (this pass only *finds* + reports);
the full 214-page run (author triggers); adding an audit command to the Rust
server (rejected on scope — keeps browser automation out of the doc-rendering
crate).
