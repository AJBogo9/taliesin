# Path-parity audit (L1) — 2026-07-26

**Lens:** feature × emission path. A document can reach a reader through five assemblers, and no round
has ever crossed the feature list against them. Three rounds each tripped over exactly one divergence
and stopped there: **DX1** (the located validators ran in `build`/`check` but not preview), **AP7**
(the mobile TOC sheet "is reachable only in a single-doc preview"), **DIAG-1** (two execution
diagnostics exist only on the `build`/`publish` path). Three independent instances of one shape is the
signal that the shape itself is worth auditing.

**Headline: the preview is not a faithful view of the built page, and each preview path is unfaithful
in a *different* way.** Two features that ship in a build are missing from exactly one preview path
each: the **Cmd-K palette is absent from the single-doc preview**, and the **mobile TOC sheet is absent
from the site preview**. Neither breaks a built page, so no reader is affected; what is damaged is the
loop the whole tool is built around (warm server, block-level incremental, no per-edit rebuild). An
author cannot see, test, or trust a feature in the surface where they author it.

**Root cause, one line:** page assembly is hand-wired at three sites with no shared owner —
`render/page.rs` (both static builds), `serve/mod.rs` (single-doc preview) and `serve_site/mod.rs`
(site preview) each decide independently which runtimes and which chrome to emit. Every divergence
below is a line present in two of those three files and absent from the third. This is the same shape
as the mobile round's single root cause (a capability nobody queries), one layer up: a decision nobody
centralises.

## Method

Release binary at `e534c73`. One probe document carrying every content-gated construct (mermaid, a
`{js}` cell, `.code-walkthrough`, `.panel-tabset`, `.scrolly`, math, a numbered figure, a callout, a
theorem, `toc: true`) plus a two-page site wrapping the same file, so every path renders **the same
source**. Six paths measured:

| | path | command | `OutputMode` | assets |
|---|---|---|---|---|
| P1 | single-doc preview | `preview probe.tmd` | `Preview` | Inline |
| P2 | site preview | `preview site` | `Preview` | Inline |
| P3 | standalone build | `build probe.tmd` | `Build` | Inline |
| P4 | site build | `build site --out` | `Build` | External (`_assets/`) |
| P5 | one-shot render | `render probe.tmd` | `Build` | Inline |
| P6 | bare build | `build --bare` | `Bare` | Inline |

Static paths compared by emitted bytes; the two preview paths fetched over HTTP from live servers.
Behaviour confirmed in a real browser via the project's own `puppeteer-core` harness
(`tools/ui-audit/lib/browser.mjs`) because the chrome-devtools MCP profile was held by a parallel
session, the documented fallback. Probe scripts in the session scratchpad. No repo file was modified.

## The matrix

Runtime reachable from the page (for P4, the linked `_assets/*` are concatenated first, or a page-only
grep under-reports every shared script):

| runtime | P1 doc-pv | P2 site-pv | P3 stand | P4 site-b | P5 render | P6 bare |
|---|---|---|---|---|---|---|
| `code-enhance.js` | yes | yes | yes | yes | yes | . |
| mermaid loader | yes | yes | yes | yes | yes | . |
| mermaid lib (offline) | . | . | yes | yes | yes | . |
| `tali-js.js` (`{js}` cells) | yes | yes | yes | yes | yes | . |
| d3 + Plot | yes | yes | yes | yes | yes | . |
| `walkthrough.js` | yes | yes | yes | yes | yes | . |
| `tabset.js` | yes | yes | yes | yes | yes | . |
| `scrolly.js` | yes | yes | yes | yes | yes | . |
| `toc-spy.js` | yes | yes | yes | yes | yes | . |
| **`toc-sheet.js`** | (client.js) | **none** | yes | yes | yes | . |
| **`search.js`** | **none** | yes | yes | yes | yes | . |
| `client.js` (dev) | yes | yes | . | . | . | . |

Every "." except the two in bold is deliberate and verified so: `Bare` ships zero `<script>` by
contract, the 3.5 MB mermaid library is inlined only by a static build (preview keeps the lean lazy
loader), and `client.js` is preview-only by construction.

## PP-1 (MEDIUM, author-facing): the Cmd-K palette does not exist in a single-doc preview

Browser-measured on the identical document, Ctrl+K pressed on a settled page:

```
P1 single-doc preview   opens-on-Ctrl+K=false  visible-button=false  console-errors=0
P2 site preview         opens-on-Ctrl+K=true   visible-button=true   console-errors=0
P3 standalone build     opens-on-Ctrl+K=true   visible-button=false  console-errors=0
P4 site build           opens-on-Ctrl+K=true   visible-button=true   console-errors=0
```

`search.js` binds Cmd/Ctrl-K on `document` in the capture phase unconditionally
(`web-client/search.js:1035-1044`), so wherever the runtime ships the palette opens, button or not.
That is why the standalone build has a *working, invisible* palette (P3: opens, no button) and the
single-doc preview has none at all.

**Why:** `render/page.rs:513` ships the palette when the page has a TOC **or** is a site page, and
`serve_site/mod.rs:856` injects `SEARCH_JS` for the site preview. The single-doc preview injects
`TOC_SPY_JS` and `code_scripts()` and nothing else (`serve/mod.rs:978`, `:1027-1031`); grepping the
whole of `crates/server/src/serve/` for `SEARCH_JS` returns **nothing**.

**The comment that documents the rule is now false.** `page.rs:507-512` justifies not gating Cmd-K on
the TOC with "invisible to the author, because the preview injects both unconditionally". The site
preview does inject both; the **single-doc preview injects neither the palette nor its index**. The
build-side half of that bug was fixed and the preview-side premise was never re-checked.

**Fix:** inject `SEARCH_JS` in the single-doc preview shell beside `TOC_SPY_JS`, matching
`serve_site`. Pin it as a "same document, same affordances" assertion rather than a per-path
`contains`, or the next assembler drifts the same way.

## PP-2 (MEDIUM, author-facing): the mobile TOC sheet does not exist in a site preview

At a 390×844 emulated phone (viewport **emulation**, never window resize, per the mobile round's
recorded floor at ~500px), same document:

```
P1 single-doc preview   body.tali-toc-sheet=true   handle-visible=true
P2 site preview         body.tali-toc-sheet=false  handle-visible=false
P3 standalone build     body.tali-toc-sheet=true   handle-visible=true
P4 site build           body.tali-toc-sheet=true   handle-visible=true
```

**Why:** the sheet chrome is a `<button id="tali-toc-handle">` plus a backdrop, emitted at exactly two
hand-copied sites — `render/page.rs:353` (both static builds) and `serve/mod.rs:946` (single-doc
preview). `serve_site` has no copy, so a site preview never enters sheet mode and its TOC stays a
desktop sidebar at phone width. Two runtimes drive the same chrome (`toc-sheet.js` in a static build,
`client.js:888-1001` in the single-doc preview), which is why the divergence survived: each
implementation looks complete on its own.

This is AP7's recorded "not chased" note, now measured across paths and attributed. It is also the
**book-authoring** path: `preview <dir>` is how the dogfooded books are written, so the phone reading
experience of a book is the one thing its author cannot see while writing it.

**Fix:** emit the handle + backdrop from `serve_site` too (a third copy is the wrong answer; the two
existing copies should become one helper the three assemblers call).

## Measured healthy — do not re-scope

- **The content gates match their emitters.** Every marker `code_scripts_for` looks for in a static
  build is exactly what the emitter writes: `class="mermaid"` (`emit.rs:55`), `application/tali-js`,
  `code-walkthrough` (`divs.rs:640`), `panel-tabset` (`divs.rs:693`), `tali-scrolly` (`divs.rs:782`).
  A `.scrolly` without `name=` was the sharpest suspect (its only other marker, the hidden reactive
  input, is `name=`-gated) and it is fine: the root div carries the class unconditionally.
- **The load-bearing invariants hold on all six paths**: `data-block-id` and `data-sourcepos` on every
  block, `data-section-end` present 5×, figure numbering identical, `<html lang>`, favicon, generator
  meta.
- **`render` is byte-identical to `build <file>`** (4,898,542 bytes both), so P5 is not a distinct path.
- **The `Bare` contract holds**: zero `<script>`, KaTeX CSS still inlined.
- **Site-build externalisation is correct**: every shared runtime is in `_assets/`, reachable, not
  duplicated per page.
- **Zero console errors on all four live paths.**

## PP-3 (MEDIUM), found later the same day: the two build paths disagree about `{{< include >}}`

Found while trying to run the mutation re-run, not by the parity sweep itself, and it is the first
finding of this round where **the emitted content differs**, not just the chrome.

`corpus/tech-blog/posts/pca-geometry/index.tmd` carries
`{{< include ../../_includes/three-scene.tmd >}}`. Built two ways:

| command | include | warning | `function makeScene3D` in output | blocks |
|---|---|---|---|---|
| `build corpus/tech-blog --out …` (site) | resolved | none | **1** | 50 |
| `build …/pca-geometry/index.tmd` (single file) | **dropped** | `include not resolved (path escapes the project root …)` | **0** | 51 |

So a single-file build of a page that lives in a site ships **without its 3D scene**. It warns, so it
is not silent, but the two paths produce different documents from one source.

**Mechanism.** `includes.rs:350` documents the containment root as "the nearest ancestor `base_dir`
holding a `.git` or `_site.yml`, else `base_dir` itself". The site build passes the site root, so
`../../` stays inside it. The single-file path does not, so the root collapses to the document's own
directory and any `../` climb escapes it. The rule is right; one caller does not use it.

**Two consequences worth separating:**

1. **The corpus test that covers this passes through a path the CLI does not use.**
   `crates/core/tests/corpus.rs::includes_are_resolved_with_origin_files` calls
   `render_document_with_includes(&src, &dir)` directly, which *does* infer the root, so it is green
   while `build <file>` drops the include. A test asserting an include resolves, next to a command
   that drops it, is the vacuous-test shape one level up: the assertion is true of the library and
   false of the product.
2. **That test's outcome depends on `.git` being present.** Root inference walks for `.git` or
   `_site.yml`; in a tree with neither (an export, a vendored copy, a `docker COPY` without VCS
   metadata, or a `cargo-mutants` scratch copy) the same test fails. Verified by leaking the copy:
   every input byte-identical, `corpus/_includes/three-scene.tmd` present, and the test still fails.
   **This is what blocked the mutation re-run** — cargo-mutants refuses to test mutants when the
   unmutated baseline is red, and the baseline is red only because the copy has no `.git`.

**Fix direction:** give the single-file build the same inferred root the library uses (one call site),
and make the corpus test exercise the CLI path or pass an explicit root, so it stops depending on
repository metadata.

## Second half: decks, mounts and the embed path — all clean

Run after the page findings, same day, same method. **Both remaining assemblers pass**, which is worth
recording precisely because the page half did not.

**Decks (`render/deck.rs`, a fourth assembler) are identical on all four paths** — single-doc preview,
standalone build, deck inside a site build, deck in a site preview. Measured at runtime: `window.
TaliesinDeck` present with the **same 20-method facade**, **18 slides** each, a runtime-injected
`theme-color` of `#16181d` each, and `ArrowRight` advancing to the **same** slide (`#/what-decks-are`)
on all four. Statically the invariants match too (56-57 `data-block-id`, 55 `data-sourcepos`, favicon
on every path, `client.js` preview-only, the mermaid library inlined only by the two build paths).

**`mounts:` (the fifth path) is equivalent to serving the project directly.** The mounted page and the
directly-served page differ by **4 bytes**: the boot nonce and `TALIESIN_WS_PATH`
(`/ws?page=index.tmd` → `/ws?page=sub/index.tmd`). In the browser both give the same Cmd-K palette,
the same 3 TOC links, the same relative hrefs, **0 failed requests and 0 console errors**.

**The `{{< embed >}}` iframe behaves identically in a built site and a previewed one**: one iframe,
`src=talk.html`, the same 704×396 box, and inside it 18 live slides with the deck API present.

**`build --bare` on a deck is refused, not degraded**: `--bare cannot build a slide deck: deck
navigation needs JavaScript. Build it without --bare.` Exactly the DX11-style hard error the project
prefers over a silently broken artifact.

**A false finding this half nearly produced.** A static grep reported `meta[name="theme-color"]`
missing from **all four** deck paths, which reads as a regression of the band-B batch that shipped it.
It is not: `deck.rs:240` *creates* the meta at runtime (`createElement` + `setAttribute('name',
'theme-color')`), so no static needle can see it. This is the band-B lesson ("needle the mechanism,
never the phrase") turned around: when the mechanism is runtime DOM construction, the only valid
needle is the **rendered result**. The browser probe found `#16181d` on all four.

**Also not a defect:** every deck path logged one 404 for `media/fit-a.png`. The fixture copied
`corpus/deck.tmd` away from its `media/` directory; the build had already reported it (`built with 1
problem`). Recorded so the next reader of these logs does not re-file it.

## Not measured, so a green result here is not mistaken for coverage

Executed-cell output spliced *after* gating (a cell whose output introduces a gated construct);
`publish`; the `--host` LAN path; the diagnostics surface (DIAG-1 already owns it); hot-reload
propagation *through* a mount (static + first-paint equivalence only); and any page below
`MIN_TOC_HEADINGS`, where the TOC-gating branches differ again.

## Traps this round paid for, recorded so the next one does not

1. **An unset shell variable makes `grep -qF ""` match every file.** Two rows of the first matrix came
   back "yes" everywhere, including a 629 KB `--bare` page allegedly containing a 3.5 MB library. The
   keys had escaped parens, the lookup returned empty, and an empty needle matches everything. **A
   parity row that is uniformly positive is a bug in the probe until proven otherwise.**
2. **The inlined-asset needle trap, again, at one remove.** `tali-search-btn` and `tali-toc-sheet`
   appear 16-19 times in pages that render neither, because every page inlines the whole CSS and those
   are *selectors*. Counting instead of testing presence is what exposed it; anchoring the needle to
   `<button[^>]*tali-search-btn` is what fixed it.
3. **`#tali-toc-handle` is an id, not a class.** A `.tali-toc-handle` selector reported the handle
   missing on all four paths, which would have filed a fictitious finding on top of a real one.
4. **Page-only greps under-report the site build.** Its runtimes are in linked `_assets/`, so the
   comparison must concatenate the page with what it links or every shared script reads as absent.
