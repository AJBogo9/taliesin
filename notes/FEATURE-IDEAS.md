# Taliesin: feature ideation & product audit

> A research-backed pool of feature ideas and product critiques that **feeds**
> `ROADMAP.md` (the committed roadmap). This file is the upstream brainstorm;
> the roadmap is the downstream, sequenced, corpus-pinned plan. An idea here is not a
> commitment. It graduates to the roadmap only when it earns a corpus pin doc.
>
> **Session 1: 2026-06-25.** Method: deep research across five tool lineages (5 parallel
> research agents), each grounded in Taliesin's actual capabilities + invariants, then
> synthesized. Lineages: (1) web-native doc/notebook tools, (2) dev servers & live-reload,
> (3) word processors & PKM/wiki, (4) PDF viewers / e-readers / read-later apps,
> (5) typography, reading science & accessibility. Interactive/explorable and
> presentation/output ground folded into the synthesis.
>
> Every idea below is filtered through the **unchanged guardrails** (see ROADMAP.md):
> HTML-only output; the `.qmd` is the single editing surface and the preview is read-only;
> lean core (power at the `qmdEnhancers` registry / build seams / diagnostics channel /
> additive block metadata); offline / self-contained. Ideas that violate a guardrail are
> marked **CONFLICTS** and kept only if they carry a strong reason worth re-litigating.

---

## The headline finding (read this first)

**Taliesin's roadmap is overwhelmingly author-side and dev-loop-side; the READER's
experience of the built output is the biggest under-invested, highest-leverage, most
on-brand opportunity.** Validation, live-edit, the editor companion, the schema, the
benchmark, the craft passes, all aimed at the person writing. Yet the artifact is *read*
far more than it is written, often by a technical audience, on a phone, possibly offline,
and the reading surface today is: a TOC, scrollspy, Cmd-K search, a light/dark toggle, a
lightbox. That is thinner than a 2010 e-reader.

Two structural facts make the reader-side the right bet, not just an available one:

1. **Taliesin holds a unique substrate no static-site generator has: a stable
   content-hash `data-block-id` on every block, a total `data-sourcepos`, and a full
   site-wide numbering / cross-ref / search model.** Every reader feature that other
   tools fake with brittle fuzzy-text anchoring (Hypothesis, Readwise) becomes *exact and
   durable* here: highlights, reading-position memory, bookmarks, deep links, "referenced
   by" backlinks, hover-cards that pull the *actual target block*. The moat that makes
   authoring fast also makes reading robust. **No competitor can copy this without the
   block model.**

2. **The warm, source-mapped, block-modeled process unlocks author-side features a batch
   compiler cannot reach either** (a live doc inspector, provenance dots, predictive
   co-edit, hover-cards across a whole site). So the same architecture pays off twice.

The strategic re-framing: Taliesin is positioned as a live, reactive dev-server for computational documents. Its *defensible*
identity is **"a document you read better than a PDF and write faster than a notebook,
that is also one offline HTML file."** The reader-experience cluster is what turns the
existing moat into something a reader feels, not just a benchmark the author cites.

A second, smaller finding: **several capabilities are already built but not surfaced.**
Word-count + reading-time exist but only in the dev preview control bar (never in the
built page). Click-to-source is framed as a dev affordance, not a reader/author affordance
on the served site. The block diff is internal, never shown to the author. Cashing these
is nearly free.

---

## Personas, how they use the tool, needs, and pain points

### Persona A, the Author (the primary user today)
A technical writer / researcher / developer authoring blog posts, slide decks, an
internals book, and data-science posts with executed Python/R. Works in VS Code with the
preview open beside the editor; values fast iteration, click-to-source, beautiful
self-contained output, executed code that stays fresh, and not fighting the tool. Dogfoods
the docs as two books.

- **Needs:** zero-latency edit→see; never lose place/state on save; trust that outputs
  match code; see the *shape* of a long document; catch mistakes (broken refs, weak prose,
  dead links) while writing; ship output that looks finished; restructure long-form safely.
- **Pain points:** no whole-document structure/outline view (only a flat TOC); no
  reverse-reference ("what links here?"); prose gets no feedback (config is rigorously
  linted, prose not at all); restructuring a long doc is manual text-shuffling since
  drag-reorder was (correctly) removed; "what did that edit actually do / which cells
  ran?" is console-only; the reader experience of their own output is unpolished, which
  undersells the work.

### Persona B, the Reader (the under-served user)
A technical reader consuming a post / book / reference in a browser, desktop or phone,
sometimes offline, sometimes with assistive tech (screen reader on a math-heavy doc).

- **Needs:** legible, comfortable text at *their* size/contrast/lighting; never lose their
  place across sessions; know how much is left; look up a term without leaving; mark and
  find the one line that matters; quote correctly; search and actually *see* the hit;
  listen hands-free; reach the content with a keyboard/screen reader.
- **Pain points:** "I lost my place" (browser dumps you at the top of a 40-minute
  chapter); "how much is left?" (no progress, the min-read estimate is author-only);
  "I can't mark anything" (read-only glass); "what's this term/symbol?" (selection does
  nothing); "text too small on phone / lines too long on a 27-inch monitor" (fixed
  measure, no reader controls); "I can't read at night/outside" (binary light/dark, no
  sepia/contrast); "found it in search, now where is it?" (no in-page hit highlight);
  math is invisible to screen readers (KaTeX emits visual spans only); no read-aloud.

### Persona C, the Deck Audience (secondary)
Someone watching a presentation built as a Taliesin deck, wanting to follow on their phone,
get the slides afterward, and (if hearing-impaired) read along.

- **Needs/pain:** follow-along on a personal device; captions/speaker-notes after the talk;
  a shareable link to a specific slide; the deck readable on a small screen (mobile/touch
  is a known deferred gap).

---

## Feature ideas

Format per idea: **Name** — what it does — value — **size** {S<1d / M / L} — **where it
lives** {reader / build / diagnostics / editor-command / dev-preview / CSS} — **fit**
{fits / needs-care / CONFLICTS}. Where an idea aligns with an already-planned roadmap item
it is tagged *(roadmap: …)*. Where a corpus pin is obvious it is named.

### Category 1 — Reader experience of the output (the headline opportunity)

All reader-state ideas store in the **reader's own** `localStorage`/`IndexedDB`, keyed by
`data-block-id` (durable across re-renders), and export/import as a file. This is **not
collaboration** (no backend, no shared server) and is explicitly in-scope.

1. **Resume-where-you-left-off** — persist the topmost visible `block-id` per page/chapter;
   on reopen, a quiet "Resume reading (78%) →" pill scrolls there. — the #1 e-reader
   feature, finally exact on the web. — **S** — reader — fits. Pin: `corpus/demo-book`.
2. **Reading-progress bar + "N min left in this chapter"** — a thin scroll-tied bar plus
   per-section time-remaining from the word count below the cursor. **The logic already
   exists** (preview-only); just promote it into the built page. — removes the bottomless
   feeling. — **S** — reader/build — fits.
3. **Reader preferences panel** — font-size A−/A+, line-height, measure (narrow/normal/wide),
   theme incl. **sepia** + high-contrast, serif/sans, optional comfortable-spacing
   (`letter-spacing .04em / word-spacing .08em / line-height 1.8`, the one evidence-backed
   dyslexia aid), optional hyphenation. Writes `--qmd-*` overrides to localStorage (same
   pipeline as the dark toggle). — the core e-reader comfort suite; satisfies WCAG 1.4.12
   overridability. — **M** — reader/CSS — fits. *The flagship reader feature.*
4. **Reader highlights (block-anchored)** — select text → pick a color → persists keyed by
   `{block-id, offset, length}`. Survives reloads and edits to *other* blocks; an edit to
   *that* block marks it "moved" (Kindle-style) instead of losing it. — mark what matters,
   durably, on read-only glass. — **M** — reader — fits.
5. **Margin annotations / reader notes** — attach a note to a highlight/block, rendered in
   the existing sidenote gutter on desktop, a drawer on mobile. — think *in* the document,
   privately. — **M** — reader — fits (reuses sidenote layout).
6. **Annotation/highlight export + import (Markdown + JSON)** — export highlights/notes/
   bookmarks as Markdown (quote + note + section + deep link) or JSON; re-import on another
   device by file. — Readwise-style "take my notes with me," zero backend. — **S** — reader
   — fits (explicitly not collab).
7. **Selection toolbar** — on selection, a small floating bar: Copy, **Copy as citation**
   (clean quote + page title + nearest heading + a `#:~:text=` text-fragment deep link),
   Highlight, **Define**, Share-link. — frictionless correct quoting into notes/Zotero;
   exact passage sharing. — **M** — reader — fits (leverages the numbering/xref model).
8. **Dictionary / Wikipedia lookup on selection** — "Define" opens the existing link-preview
   popover with a definition (bundled wordlist offline, or Wiktionary/Wikipedia fetch
   online). — resolve jargon without leaving the page. — **M** offline / **S** online —
   reader — fits (reuses `qmdInitLinkPreview`).
9. **Read-aloud with highlight-follows-voice** — ✅ SHIPPED 2026-06-26 (+ moonshot 3). Web
   Speech API + the reader's OS voices (offline), block-by-block from the block in view;
   prose spoken **sentence-by-sentence** (per-sentence utterances, not flaky `boundary`
   events) with the sentence highlighted (CSS Custom Highlight API) + auto-scrolled; code
   **announced + line-stepped** (line Ranges, no code text spoken — author's pick over
   reading source verbatim); figures/equations/tables announced. Floating mini-player
   (play/pause, prev/next block, speed) + reader-menu Listen (speed + voice); rate/voice
   persisted, position ephemeral; `window.__qmdSpeakImpl` seam for headless test. Pinned
   `corpus/reader/read-aloud.qmd`. — **L** — reader — fits (walks the block DOM; no media
   files, no new format).
10. **Bookmarks rail** — star any block; a "My bookmarks" panel lists them with heading +
    snippet, click to jump. — pin the lemma / the API call you keep returning to. — **S** —
    reader — fits.
11. **Reader-side code folding / peek** — let the *reader* collapse long code blocks to a
    one-line summary, expand on click, remembered per block-id, independent of author fold
    settings. — read prose without scrolling past 80-line listings. — **S** — reader — fits.
12. **Distraction-free / focus reading mode** — one key hides nav/TOC/chrome, widens to a
    single column, optionally dims all but the block near the cursor. — Instapaper-grade
    calm for long technical reads. — **S** — reader/CSS — fits.
13. **Document minimap / thumbnail rail** — a thin right-edge map (sections as bands,
    figures/code as marks, your highlights/bookmarks + viewport shown), click to jump. —
    spatial overview + "jump to that figure I saw." — **M** — reader — fits (derived from
    the block/numbering model).
14. **"Save offline" / installable PWA** — a minimal service worker + manifest so a
    site/book installs and reads fully offline + app-like on phone. The output is *already*
    self-contained, so this is mostly packaging. — read on a plane; keep it forever. — **M**
    — build — fits (delivery wrapper, **not** a new output format).
15. **"On this page" mini-TOC with per-section min-read + read-state checkmarks** — sections
    you've scrolled past get a check; each shows its own time estimate. — triage a long
    reference page. — **S** — reader — fits (extends the shipped TOC).

### Category 2 — Typography & accessibility craft (CSS-cheap, mostly invariant-free)

16. ✅ SHIPPED 2026-06-26 (reader polish bundle). **`text-wrap: pretty` on prose + `balance` on display text** — `pretty` on `p, li`;
    `balance` on `h1-h6, figcaption, blockquote, .callout-title`. — kills orphaned last
    words and lopsided headings; zero markup. — **S** — CSS — fits. *Highest
    value-to-effort win in the whole audit.*
17. **Hyphenation pass** — `hyphens: auto; hyphenate-limit-chars: 6 3 2` on body, off for
    code/headings, gated so it can default off (hyphenation can hurt dyslexic readers). —
    smooth the ragged edge of the narrow serif measure. — **S** — CSS — fits.
18. **Math accessibility layer (MathML/ARIA)** — render KaTeX with `output: "htmlAndMathml"`
    (or attach a `render-a11y-string` `aria-label`, `aria-hidden` the visual spans). —
    screen readers finally voice equations: **the single biggest a11y hole for a tool whose
    own corpus is math-heavy.** — **M** — build (`math.rs`) — fits (KaTeX already bundled).
    Add an a11y-panel check to dogfood it.
19. ✅ SHIPPED 2026-06-26 (reader polish bundle). **Skip-to-content link + focus management** — visually-hidden-until-focused skip link as
    the first body child; focusable `<main>`. — keyboard/SR users reach prose in one
    keystroke. — **S** — build/CSS — fits.
20. **`forced-colors` + `prefers-contrast: more` support** — `@media (forced-colors: active)`
    using system colors; a higher-contrast `--qmd-*` set for `prefers-contrast: more`. —
    Windows High Contrast / low-vision readers get a working page, not a collapsed palette.
    — **S/M** — CSS — fits.
21. **Reading ruler / focus line (enhancer)** — opt-in translucent band following the
    caret/pointer line, reader-tinted, honoring reduced-motion. Evidence-backed gains for
    dyslexia/ADHD (ACM CHI 2023). — **M** — reader — fits. Off by default.
22. **Hanging punctuation / optical margins** — `hanging-punctuation: first last` (Safari)
    + an `@supports` text-indent fallback for leading quotes/bullets. — flush, intentional
    left edge (Bringhurst). — **S/M** — CSS — fits.
23. ✅ SHIPPED 2026-06-26 (reader polish bundle). **Widow/orphan + figure-caption keep** — `orphans: 2; widows: 2` and
    `break-after: avoid` so a caption never separates from its figure (screen *and* print).
    — **S** — CSS — fits.
24. **Vendored typeface track (the enabler)** — one subset variable serif + sans (woff2,
    inlined/copied), strictly **opt-in via `theme:`**, system stack stays default. — unlocks
    oldstyle figures, true small-caps, real fractions, consistent cross-machine rendering. —
    **L** — build/CSS — needs-care (lean-core + payload: one face, opt-in, never
    auto-loaded; stays offline because vendored/subset).
25. **OpenType prose polish** *(after #24)* — `oldstyle-nums proportional-nums` on prose
    (keep `tabular-nums` on code/tables/math), a `small-caps` acronym utility, diagonal
    fractions. — numerals blend into lowercase; acronyms stop shouting. — **S** — CSS —
    fits (no-op on system fonts, degrades safely). Plus **opt-in drop cap** on lead
    paragraphs.

### Category 3 — Authoring intelligence & navigation (single-surface-respecting)

Author ergonomics live on the *editor* side (the VS Code companion), as *diagnostics*, or
as *read-only* preview/build affordances, never as preview-pane edits.

26. **Structure panel / outline** — a read-only preview sidebar: heading tree with nesting,
    per-section word count, a badge for "has unresolved xref / has TODO"; click a node →
    scroll preview (+ move the editor cursor via the companion's cursor-sync). — the missing
    "shape of the whole doc," the #1 word-processor/PKM staple Taliesin lacks. — **L** —
    reader/editor — fits. Pin: `corpus/layout/structure.qmd`.
27. **"Referenced by" backlinks** — under each `{#sec-}`/`{#fig-}`/`{#eq-}` anchor, emit a
    read-only "Referenced by: §2.1, Fig 3" from the existing project xref scan (it already
    records every forward ref). — Obsidian-grade reverse navigation + revision safety. —
    **M** — build — fits (reuses `xref.rs`; no `cite.rs` change).
28. **Block-level transclusion** — extend `{{< include file.qmd#sec-id >}}` to pull a single
    anchored block/section, preserving the line source map so click-to-source still lands in
    the origin. — single-source reuse (Roam/Logseq) without copy-paste drift. — **M** —
    build — needs-care (additive parse of the `#frag` suffix only; `includes.rs` is
    Do-NOT-touch).
29. **Prose-lint diagnostics** — ✅ SHIPPED 2026-06-26. A native, offline, **opt-in**
    (`prose-lint: true | { banned: [...] }`) prose pass (`crate::prose`): doubled words, weasel
    words, custom banned terms — emitting **located** click-to-source warnings into the
    *existing* diagnostics channel, markdown-aware (skips code/math/links/HTML/fences). The
    Vale/Hemingway loop without a second tool or a network call. **Passive voice deferred**
    (its is/was+-ed heuristic is too noisy). Pinned `corpus/diagnostics/prose.qmd`. — the most
    natural growth of the validation moat. — **L** — diagnostics.
30. **Link-health / orphan report** — a build+diagnostics sweep: broken `[@cite]`, dangling
    `@xref`, includes pointing at missing files/anchors, headings nothing links to, images
    with no `alt`, oversized images, total payload. One consolidated "doc health" panel,
    each row located. — the pre-publish confidence pass every docs team builds by hand,
    live. — **M** — diagnostics/build — fits. Pin: `corpus/diagnostics/links.qmd`.
31. **Companion: "Move section / promote / demote heading"** — VS Code commands that perform
    pure `.qmd`-buffer text transforms (cut a heading + its body to the next sibling
    boundary, reinsert), author-confirmed. — Scrivener-style restructuring, the *legal*
    replacement for the removed drag-reorder. — **M** — editor-command — fits. *(roadmap:
    companion Phase 2.)*
32. **Companion: "Rename label" (include-aware refactor)** — rename a `#sec-`/`#fig-`/citation
    key and update every reference across the project (following includes), shown as a
    preview-diff the author accepts. — the project-wide rename plain find/replace can't do
    safely. — **M** — editor-command — fits. *(roadmap: companion Phase 2.)*
33. **Companion: outline tree + snippets + focus/typewriter mode** — a `DocumentSymbol`
    outline (jump to heading), shipped snippets for Taliesin constructs (callout, figure,
    tabset, include, cite), and an iA-Writer focus mode (dim non-current lines, center the
    caret). — editor-side authoring muscle-memory. — **S** each — editor-command — fits.
34. **Word-count & reading-time HUD with goals** — promote the counts into the built page
    (#2) *and* the preview, plus front-matter `goal: 1500w` / `5min` → an under/over
    **diagnostic** + a build-time badge. — Ulysses/iA pacing + "is this section bloated?". —
    **S/M** — diagnostics/build — fits.
35. **Tag index pages** — front-matter `tags:` generate read-only build-time tag-index pages
    + per-page chips + a preview filter. — cross-cutting organization beyond the folder tree.
    — **M** — build — fits (rides the listing machinery). *Note: distinct from the
    `categories`/RSS non-goal; tags are a navigational index, not a feed.*

### Category 4 — The dev loop (cash the warm, source-mapped process)

36. **Change inspector ("what changed" log)** — a dev-menu panel listing the last N block
    ops with type + a one-line summary (`update §"Method" · cell re-ran 0.4s · 3 blocks
    shifted`), each click-to-source. — turns the moat's diff stream into a visible activity
    feed; answers "what did my edit do?". — **M** — dev-preview — fits.
37. **Cell exec timeline** — a per-edit strip: each code cell as ran / replayed-from-freeze /
    cached / errored, with wall-time + a freeze-key tooltip, click → source. — makes the
    freeze cache + warm-kernel behavior legible (currently console-only); the headline
    differentiator vs the cold full-rebuild of batch compilers, *made visible*. — **M** — dev-preview/diagnostics
    — fits.
38. **Pre-publish health panel ("Lighthouse for your doc")** — one panel rolling up #30 +
    heading-skips (extends the a11y audit) + payload + perf, each located. — brings the loved
    coverage/Lighthouse/link-check trio into the live loop where it's actually run. — **M/L**
    (ship link/asset existence first) — diagnostics — fits.
39. **`taliesin check <file|dir>`** — ✅ SHIPPED 2026-06-26 (v1). A static, kernel-free CLI
    gate that renders in memory and emits every located diagnostic from the warning channel
    (schema/front-matter/`_site.yml`/cell-option/container validation with did-you-mean, broken
    `@xref`, unknown shortcodes, missing bibliography, opt-in prose-lint) as `path:line: message`
    or `--format json` (`[{file,line,message}]`); exits non-zero on any finding for CI. Pure
    `crates/server` addition reusing `render_document_with_includes` + `cite::validate_xrefs` +
    `Site::render_page_doc_warned`; no core change. **Deferred:** `--format sarif` (needs a
    `rule`/ruleId field on `Warning`), a11y/dead-link checks, and an `--exec` mode for runtime
    cell errors. — **S/M** — CLI.
40. **Inline render-frame in the error overlay** — on a render panic, show the offending
    source frame (file + lines, caret on the column) in the overlay (the located-warning
    struct already carries file/line). — cuts the overlay→editor→find-line round-trip (Next
    16's overhauled errors). — **S** — dev-preview — fits.
41. **Make the change-flash legible** — color/label the swap pulse by op kind (content-edit
    vs cell-re-ran vs position-only `SetMeta` shift) so the flash *tells you what happened*,
    not just where. — **S** — dev-preview/CSS — fits.
42. **Block spotlight (Storybook-for-prose)** — Alt-Shift-click (or an editor command)
    isolates one block onto a centered stage, surroundings dimmed; Esc returns. — work on a
    figure/callout/cell without the rest of the doc fighting for attention. — **S/M** —
    dev-preview/CSS — fits.
43. **Live effect pin** — when the editor cursor sits on a *definition* (a `{python}` cell, a
    CSS var, a cross-ref target), the preview floats a thumbnail of the *downstream* block
    that consumes it, even offscreen (reuses cursor-sync + the reactive-graph consumer map).
    — kills "scroll 2000px to see the effect." — **M** — dev-preview — fits.
44. **Two-device follow mode** — extend `--host`: an opt-in toggle so the phone mirrors the
    laptop's scroll (one-way, viewer follows author). — demo on a phone while driving from
    the desk; check responsive layout live. — **M** — dev-preview — fits (one-directional
    keeps single-surface clean; broadcasts scroll %, never writes source).
45. **Per-block "inspect" popover** — hover-Alt a block → its block-id, sourcepos, source
    file, and (cells) freeze-key + last run time. The DevTools "inspect element" for the
    block model; great for the internals dogfooding. — **S** — dev-preview — fits.

### Category 5 — Interactivity & explorable documents (genuinely-novel)

46. **`::: {.scrolly}` sticky-figure scrollytelling** — ✅ SHIPPED 2026-06-26. A sticky visual
    stage (the non-`.step` inner blocks) beside a scrolling `.step` column carrying `state=`
    (→ `data-state`). The active step (a new `scrolly.js`, reusing the walkthrough
    IntersectionObserver band) sets `data-scrolly-state` on the root for pure-CSS effects AND —
    with `name=` — drives a hidden `data-qmd-input` so a sticky `{js}` cell reacts via
    `//| input:`. **Reuses the shipped `{input}` registration (#47) — no bespoke `qmd-scrolly`
    event, no `qmd-js.js` change**: scrollytelling = a reactive input driven by scroll position.
    **Generalizes the shipped `code-walkthrough` machine.** Pinned `corpus/explorable/scrolly.qmd`;
    browser-verified. — Distill/Idyll explorable explanations with zero bespoke JS.
47. **Reader input vocabulary `{input}` bound to the reactive graph** — ✅ SHIPPED 2026-06-26.
    Authored as a built-in **shortcode** `{{< input name="k" type="slider" min=1 max=10 >}}`
    (NOT a `:::` div — a bodyless div is dropped by `group_divs` and emitting empty containers
    would touch the Do-NOT-touch div machine). Five types (slider/number/checkbox/text/select);
    a static, keyboard-accessible labeled control tagged `data-qmd-input` that the shipped
    runtime registers (reusing `registerInput`/`scheduleFrom`), so a `//| input:` cell re-runs
    transitively. Slider gets a live `<output>`; `validate_input` gives located diagnostics.
    Pinned `corpus/reactive/inputs.qmd`. — Marimo/Observable "drag the slider, the chart
    updates" as a one-liner, fully client-side and offline.
48. **Provenance dots on executed-cell output** — each output gets a tiny affordance: green =
    matches current code (freeze hit), amber = re-running; Alt-click → the source cell. —
    Marimo's "no hidden stale state" made *visible to the reader*; a trust signal unique to a
    warm+frozen architecture. — **S** — build/reader — fits.
49. **Named, reusable output objects** — `#| label: tbl-summary` makes a cell's output
    addressable so `@tbl-summary` (and the hover card) shows the *actual rendered output*;
    opt-in `{{< output tbl-summary >}}` re-embeds it. — Curvenote's reusable-figure model;
    one computation appears in multiple places without re-running. — **M** — build —
    needs-care (must reuse the freeze cache, not re-execute; keep out of `cite.rs`).
50. **DuckDB-WASM `{sql}` cells** — a `{sql}` cell over a client-loaded CSV/Parquet/Arrow,
    output a table/Plot, feeding the reactive graph. — Hex/Deepnote/Observable SQL-block love,
    offline + HTML-only. — **L** — build/reader — needs-care (DuckDB-WASM is heavy; opt-in so
    default bundles stay lean; vendor for offline). *(roadmap: cell-language-registry, cut
    until a corpus doc needs it.)*

*New (2026-07-01, dogfooding — building an interactive PML/Bayesian-ML study site on the shipped
`{input}` (#47) + `{js}` graph): the reactive substrate ships; what math/ML explorables still need is
a numerics story plus two controls. All stay HTML-only / offline and must **not** reintroduce a
reactive VM (ROADMAP's stated top design risk). Highest-leverage: #62 + #63.*

62. **Bundled numerics/stats global for `{js}` cells** — ship a small curated numerics namespace as a
    drawing-global beside `Plot`/`d3`: distribution pdf/cdf (gaussian/gamma/beta/poisson/exp), summary
    stats (mean/var), a **seeded** PRNG, and small dense linear algebra (matmul, Cholesky, 2×2 eig/inv).
    Removes the #1 friction of scientific explorables — hand-rolling pdfs/quadrature in every cell — and
    lands as another global with **no reactive-graph change**. — numpy / `scipy.stats` in a Jupyter cell;
    jStat. — **S–M** — build/reader — fits (lean-core: a bundled global; keep it small/curated +
    offline-vendored, resist growing into a numeric VM). Pin `corpus/reactive/numerics.qmd`.
63. **Two ML-explorable `{{< input >}}` types: `animate`/play tick + draggable `point`** — extend the
    shipped input vocabulary (#47) with (a) a **play/pause/step/reset** control publishing a monotonic
    tick node so iterative demos advance a frame (EM sweeps, CAVI, gradient descent), and (b) a
    **drag/click 2-D point** control publishing an `{x,y}` (or point-set) node for "place a data point"
    demos (mixtures, factor analysis). Both reuse `registerInput`/`scheduleFrom` — like scrolly (#46), an
    input driven by something other than a slider. — ipywidgets `Play`; Observable interactive canvases.
    — **M** — build/reader — needs-care: the tick must schedule **one** downstream pass per frame via the
    existing scheduler + `invalidation`, **not** a continuous dataflow loop (the reactive-VM trap). Pin
    `corpus/reactive/animate.qmd` + `.../point.qmd`.
64. **`qmd.state` — a blessed cross-re-run state store** — a small keyed store that survives scheduled
    re-runs so an iterative demo accumulates (EM parameters across ticks) instead of recomputing from
    scratch each frame; cleared on cell edit; deck-skip; never writes back to source. Formalizes what
    `invalidation` today only tears down. Pairs with #63's tick. — a Jupyter kernel holding a model between
    runs; Observable `mutable`. — **S–M** — build/reader — needs-care: scope to a per-name store with an
    explicit lifecycle; must NOT become general mutable dataflow (reactive-VM trap).
65. **Richer `{js}` output helpers: KaTeX value + mini table** — convenience builders over the existing
    DOM-return contract: typeset a returned number/array/matrix as KaTeX math (reuse the bundled KaTeX —
    e.g. echo a posterior precision typeset, not as plain text), and a minimal table renderer for
    arrays/records. Closes the rich-display gap vs Jupyter's MIME protocol with no new machinery. —
    Jupyter rich display (LaTeX / DataFrame); Observable `tex` / `Inputs.table`. — **S** — build/reader —
    fits.
66. **Opt-in Pyodide `{python}` cell (numpy/scipy in-browser, no kernel)** — a client-side `{python}`
    execution mode backed by Pyodide, feeding the reactive graph like any cell; the general "match
    Jupyter's Python" answer (this is exactly JupyterLite). Closes the fully-general gap — a
    scientific-Python stack inside an interactive, static, offline page. — JupyterLite / Pyodide. — **L**
    — build/reader — needs-care: **bundle guard** (Pyodide ~10 MB+, opt-in per page, vendored offline);
    sibling to the DuckDB-WASM `{sql}` idea (#50). Caveats: **no torch in Pyodide** (Bayes-by-Backprop
    won't run), cold-start cost. *(roadmap: cell-language-registry graduate; cut until a corpus doc needs
    it.)*

### Category 6 — Hover, cross-reference & navigation cluster (cheap, high-delight)

These share one small enhancer + the existing resolved-ref/numbering data. Bundling them is
the single most delightful low-risk upgrade to *reading*.

51. **Hover cross-reference cards** — hovering `@fig-`/`@sec-`/`@eq-`/`@tbl-`/`[@cite]` (and
    footnotes) pops a card rendering the *target block's* HTML (figure thumbnail, equation,
    bib entry, section + first lines), keyed by `data-block-id`. — eliminates
    jump-and-lose-place, the single most-loved MyST feature; reuses data Taliesin already
    computes. — **M** — reader — fits.
52. **Cross-page hover references (site/book-wide)** — extend #51 across pages from a
    site-wide label→block index built at render. — MyST's cross-project transclusion-on-hover,
    uniquely cheap given the site already renders every page's block model. — **M** — build/
    reader — needs-care (watch build cost; reuse `xref.rs`).
53. **Anchor-on-hover + copy-deep-link for every heading/figure/equation/callout** —
    hovering reveals a `#`; click copies a canonical deep link (anchor or `#:~:text=`). —
    Docusaurus/Stripe table-stakes Taliesin lacks; pairs with hover-cards. — **S** — reader/
    CSS — fits.
54. **Definition popovers / glossary** — `[term]{.gloss}` or a `glossary:` block; uses show a
    hover card with the definition; "go to definition" in Cmd-K. — readers of dense prose stop
    context-switching to look up jargon. — **M** — build/reader — fits.
55. ✅ SHIPPED 2026-06-26 (reader polish bundle). **Keyboard reader: `?` cheatsheet + arrow chapter nav** — `←/→` prev/next chapter, `/`
    focus search, `g` index, `?` overlay; the deck already has this vocabulary, port it to
    the long-form reader. — mdBook/Bookdown power-reader ergonomics. — **S** — reader — fits.
56. **"Edit in editor" on the served site** — hovering any block on a *served* site shows a
    subtle pencil; click opens the source at that line via the `qmd-goto` bridge (works under
    the companion; degrades to copy-path). — turns Docusaurus's one-way "Edit this page" into
    Taliesin's two-way click-to-source as a first-class affordance. — **S** — reader/build —
    fits (navigates, never writes).

### Category 7 — Print/paged track substance, and output-as-product

57. **Real paged print track (running heads, folios, page cross-refs)** — give the planned
    print-pdf-track *typographic substance*: `@page` running chapter/section heads
    (`string-set` + `running()`), real page numbers, `@fig-`/`@sec-` cross-refs that become
    "Figure 3 (p. 12)" via `target-counter()`, auto list-of-figures/index, widow/orphan +
    optical hyphenation; paged.js (vendored) where native paged media is absent. — a
    print/PDF that reads like a typeset book, not a screenshot; closes the one thing people
    drop to LaTeX for, *from the same HTML*. — **L** — build/CSS — fits *(roadmap:
    print-pdf-track, Wave 5; this is its substance)*. Pin: `corpus/print/paged.qmd`.
58. **Auto social-card image** — at build with `url:` set, render a per-page OG card
    (title + section + a mark) by screenshotting a card template (headless Chrome, already a
    print-track dependency) → `og:image`. — link unfurls look finished; the build-seo work
    gains a visual. — **M** — build — needs-care *(roadmap-adjacent: build-seo-completeness)*.
59. **Copy-embed snippet + oEmbed** — a "share / embed" affordance emitting an iframe snippet
    (and an oEmbed endpoint at publish) so a deck or figure embeds in another site. — the
    `{{< embed >}}` machine already isolates decks in iframes; expose it outward. — **M** —
    build/reader — fits.
60. **Deck → shareable handout / video poster** — a "download the deck as a scrollable
    handout" (the existing scroll mode, self-contained) and a poster-frame export for `{js}`/
    video slides. — the audience gets the slides after; aligns with the hero-demo recorder. —
    **S/M** — build — fits (a rendering of the HTML, not a new format).
61. **Vendor Mermaid (close the offline gap)** — Mermaid is currently the **sole CDN
    dependency**; a built page with a diagram is not truly offline. Vendor a subset or gate it.
    — honest offline story; matches the `third-party-truth` roadmap item. — **M** — build —
    fits *(roadmap-adjacent: third-party-truth)*.

---

## Improvements to existing features (the product-as-a-whole critique)

The product is deliberately lean and in genuinely strong shape; these sharpen what's there.

- **Word-count / reading-time is built but wasted** (preview-only). Promote it into the
  built page (#2, #34). Near-free, high reader value.
- **Cmd-K search jumps but is context-blind.** Add per-result context snippets, in-destination
  hit highlighting + ↑/↓ match cycling (CSS Custom Highlight API), scoped search ("this
  chapter" vs "whole book"), and structure-aware ranking (headings/labels first). The block
  model already holds the spans; only the index payload + landing behavior change.
- **Themes carry one bit where readers expect a comfort suite.** Add sepia + high-contrast as
  first-class `data-theme` values and expose the reader preferences panel (#3). The no-flash
  localStorage pipeline is already the right substrate.
- **The a11y audit is author-only and stops short.** Add checks for: equation has no text
  alternative (drives #18), no skip-to-content link (#19), color-is-the-only-signal, and
  survives-200%-zoom/1.5×-line-height reflow. Keep them author-facing + source-jumping. Then
  also give the *reader* controls (#3), since the audit currently checks contrast for the
  author while the reader can't change anything.
- **Math has no accessibility layer** (#18) — the biggest single gap for a math-heavy tool's
  own use case.
- **Sidenotes float without a visible tie to their call-site.** Add an optional numbered
  superscript marker + matching margin number (CSS counters), a `max-height`/overflow guard,
  and keep the marker on the <73rem in-flow fallback so the pairing survives.
- **The type scale is hard-coded.** Re-express `h1-h6` against a single `--qmd-scale` +
  `--qmd-font-size` root so the reader panel can rescale the whole document proportionally;
  add a gentle `clamp()` on body size for very small/large viewports (the hero already does
  this; body doesn't).
- **Click-to-source is framed as a dev affordance.** Surface it on the *served* site too (#56)
  — it's the moat's most distinctive property, near-free to extend.
- **Citations are IEEE-only; `csl:` is recognized-but-inert.** Either honor a minimal CSL
  subset or document the limitation clearly (today it silently does nothing, which the schema
  validator's own philosophy says is the worst failure mode). Reuse the cite registry to add
  "cited where" backlinks + diagnose unused/duplicate bib entries (read-only; don't touch the
  Do-NOT-touch formatter).
- **The freeze/kernel warmth is invisible.** A persistent "kernel: warm · 12 cells frozen ·
  last edit re-ran 1" line in the dev menu (seed of #37) makes the architecture self-evident.

## Removal / simplification candidates & things to watch

The tool is lean by design, so there is little to *remove*; the discipline is mostly about
not *adding* wrong.

- **Prune the suppress-only dead keys** `title-block-banner` + `site-url` (zero consumers) —
  already flagged in ROADMAP.md's `prune-and-fix-stale-docs`. Let them warn or keep with
  a justifying comment.
- **Watch the deck engine's breadth.** It is the most complete subsystem (blackout, pen,
  minimap, speaker, scroll, print, overview…) and the visual-audit "top-tier wow," so it earns
  its keep — but it is also where future effort has the *lowest* marginal reader value relative
  to the empty reader-side. Resist adding more deck modes before the reader-experience cluster
  exists. (Mobile/touch is the one deck gap with real audience value.)
- **Guard the bundle.** The vendored-font track (#24), DuckDB (#50), and paged.js (#57) are
  where "wider too" can balloon the payload. Each must be strictly opt-in. Mermaid (#61) is the
  current offline leak.
- **Resist the reactive-VM trap** (ROADMAP.md's stated highest design risk): `{input}`
  (#47) and `{sql}` (#50) must stay declarative `//|`-edge consumers of the shipped ~80-line
  scheduler, never a regrown dataflow VM.

## Moonshots (reinvent writing & reading)

1. **"My Copy" — a durable, portable reader layer over any Taliesin doc.** Because every block
   carries a content-hash id, Taliesin can ship what Hypothesis/Readwise only approximate with
   brittle anchoring: a first-class, block-anchored personal layer (highlights, notes,
   bookmarks, reading position, fold state, per-section read-state) that is *exact*, survives
   re-renders, gracefully marks only changed blocks as "moved," and exports/imports as one
   portable file with **zero backend**. The doc stays read-only and authoritative; the reader
   *owns a reading of it* that travels by file, not server. Reframes a Taliesin book from "a web
   page you read" into "a document you own a reading of" — inside HTML-only, single-surface, and
   no-collab the whole way.
2. **The transcluding hypertext document.** Combine hover cross-reference cards (#51),
   cross-page transclusion (#52), provenance dots (#48), and definition popovers (#54) into a
   reading mode where *every* labeled object (figure, theorem, citation, term, executed output,
   even a block in another chapter) is a live, previewable, source-mapped node. Taliesin already
   holds the labeled block model + numbering + resolved refs + freeze provenance for the whole
   site, so the document stops being a linear page and becomes a navigable knowledge graph you
   explore without leaving your place. MyST's hover dream + Distill's interactivity +
   Curvenote's reusable objects, but offline, self-contained, and click-to-source all the way
   down. No competitor has all four substrates in one warm, source-mapped process.
3. **Listen-and-follow study mode.** Read-aloud (#9) + highlight-follows-voice + the block
   model: prose is spoken with the current sentence highlighted and auto-scrolled; on a code
   block it pauses and line-steps (reusing the `.qhl-ln` contract from the walkthrough + deck);
   equations announce "Equation 3" and linger; figures announce their caption. A technical
   document becomes something between an audiobook and a screencast, generated entirely
   client-side from structure Taliesin already emits, fully offline.
4. **A genuinely typeset book from the same HTML.** Push the paged track (#57) to its
   conclusion: running chapter heads, real folios, auto index + list-of-figures with true page
   numbers, "see Figure 3 on p. 12," optical hyphenation/justification, gutter sidenotes. Because
   it renders *from* the built HTML it never violates HTML-only — yet it closes the one thing
   people still drop to LaTeX for. "Wider web-native capability" extended to the page.
5. **The doc inspector (author-side moonshot).** A DevTools-class panel where the document *is*
   the inspectable object: a live tree of blocks with hashes, the cell dependency graph
   (`js-reactive-graph` + Python upstream/downstream) drawn as a DAG, the diff stream as an
   event log, freeze/kernel state, all click-to-source. Editing anywhere lights up exactly which
   nodes recompute and why. Reframes authoring from "write, reload, hunt" to "edit a node in a
   running system and watch the graph settle" — the thing no static-site generator can build.

---

## Prioritization (a starting cut, not a commitment)

**Quick wins — high value, low effort, invariant-clean (do-first candidates):**
`#16` text-wrap, `#2` reading progress (logic exists), `#3`/`#6` sepia + reader prefs (partial),
`#53` anchor-on-hover copy-link, `#51` hover cross-ref cards, `#19` skip-link, `#1` resume
position, `#12` focus mode, `#55` keyboard reader, `#34` word-count promotion, `#40` error frame,
`#41` legible change-flash, `#61` vendor Mermaid (integrity).

**Big bets — open a new loved-feature category:**
`#3` reader preferences panel (flagship reader feature), the "My Copy" reader layer
(`#4`/`#5`/`#10` + moonshot 1), `#9` read-aloud, `#18` math a11y, `#26` structure panel,
`#29` prose-lint, `#57` real paged print track, `#46` scrollytelling primitive, the doc
inspector (moonshot 5), `#24` vendored-font/OpenType craft tier.

**Theme of the session:** the cheapest, most differentiated, most under-served direction is the
**reader experience of the built output** (Categories 1, 2, 6), unlocked by the content-hash
block model. The author/dev-loop ideas (Categories 3, 4) are strong but extend an already-rich
side. Recommended next brainstorm-to-spec: pick one reader-cluster entry point — the
**Reader preferences panel (#3)** or the **hover cross-reference cards (#51)** — as the first
corpus-pinned feature, since each is small, delightful, on-brand, and proves the reader-side
thesis.

---

## Parked from the backlog (2026-07-12)

Cut from `backlog.md` during a slim-down (owner ruled: these are super-polish, not priority;
revive any one here when a corpus doc needs it). Status verified against the code at cut time:

- **Cross-revision block-diff "what changed" view** — compare two saved revisions, show
  block-level changes. Not built; overlaps the "session revision digest" idea (also unbuilt);
  both trade on the diff moat.
- **Reader reproducibility manifest** — a reader-facing "how this was computed" panel
  (interpreter, versions, cell hashes). Not built.
- **Web-native List of Figures / Tables / Theorems** — an aggregated index page. Not built,
  but the numbering + anchor primitives already exist (`@fig-`, `@tbl-`, `@thm-`, `@lnx`).
- **Interactive data tables** (client-side sort/filter) — not built; adds reader JS surface.
- **"Cite this" export** — copy-BibTeX / formatted-citation affordance on posts. Not built.
- **Line-level code xrefs** (`@lst-3:line`) — whole-listing xrefs already ship
  (`label: lnx` → `@lnx`); only per-line granularity is missing.
- **Theme-aware `dark=` for static images** — `{{< video dark= >}}` already swaps a dark clip
  by theme; extending the same `dark=` to images/figures is the unbuilt part.

---

## Provenance

Session 2026-06-25. Five parallel deep-research agents (web-native doc/notebook tools; dev
servers & live-reload; word processors & PKM/wiki; PDF viewers / e-readers / read-later;
typography, reading science & accessibility), each grounded in Taliesin's capabilities +
invariants, then synthesized with the author's framing (think about the reader; pain points with
the tool *and* its output; go beyond traditional tools to reinvent writing/reading; size doesn't
matter, only whether it strengthens the tool). One sixth agent (a duplicate reader-side run) hit a
session limit and was re-run. Interactive/explorable and presentation/output ground folded into the
synthesis. All ideas respect the ROADMAP.md guardrails; ideas in tension are marked
needs-care or CONFLICTS. This file feeds the roadmap; nothing here is committed until it earns a
corpus pin.
