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
9. **Read-aloud with highlight-follows-voice** — ❌ **NOT SHIPPED. The "✅ SHIPPED 2026-06-26
   (+ moonshot 3)" mark this entry carried was false** and was corrected 2026-07-25:
   `speechSynthesis` occurs **zero** times in `crates/` and `web-client/`, and the pin it
   claimed (`corpus/reader/read-aloud.qmd`) does not exist. The "Big bets" list at the
   bottom of this file always listed `#9` as unshipped, so the file contradicted itself;
   that list was the correct half. Verdict recorded in backlog item 24: **out on cost,
   not on principle.** Design below kept as the spec if it is ever built. Web
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
14. ⚠️ PARTLY SHIPPED 2026-07-24 (install half only). **"Save offline" / installable PWA** — a minimal service worker + manifest so a
    site/book installs and reads fully offline + app-like on phone. The output is *already*
    self-contained, so this is mostly packaging. — read on a plane; keep it forever. — **M**
    — build — fits (delivery wrapper, **not** a new output format).
    **Shipped:** `manifest.webmanifest` + app icons per site build, so a site/book installs
    from Chrome/Edge, iOS "Add to Home Screen" and Safari's "Add to Dock"
    (`crates/core/src/site/manifest.rs`, spec
    `docs/superpowers/specs/2026-07-24-webmanifest-install-design.md`).
    **Deliberately NOT shipped:** the service worker. It is a one-way door (it lives in the
    reader's browser independent of the pages, so a bug outlives the fix and un-shipping
    needs a self-unregistering replacement plus every reader returning), and the offline
    value it would buy is already delivered by the book `<book>.zip`, which the reader owns
    outright. Reopen only if the zip proves insufficient in practice.
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

## Parked pending market demand (2026-07-23)

- **CAD-as-code cell (`{openscad}` / `{python}` build123d → interactive 3D preview)** — write CAD
  as code in a `.tmd` block, render an interactive 3D model in the browser, version-control the
  model as text. **Status: PARKED, no market demand** (owner's rule: no demand → don't build).
  Feasible and a clean fit (reuses the `{js}`/kernel machinery, not a new output format, mesh is
  display-only so read-only-preview holds); commercialization is legally clean via an arm's-length
  subprocess to a user-installed `openscad` (never bundle GPL openscad-wasm). But CAD workers are
  the wrong audience, code-CAD is a niche-within-the-hobbyist-niche, and Taliesin's peer group
  (Quarto/Jupyter Book/mdBook) shows zero traction for embedded CAD. **Revive when:** you actually
  want to write a doc with a live parametric model (a 3D-printing build log / parametric explainer,
  the legitimate author-pull reason, name the pin doc); or peer-group / notebook-CAD demand
  materializes; or a concrete external ask lands. Full feasibility + licensing + market record,
  and the pre-decided implementation path if revived, in
  [2026-07-23-cad-as-code-research.md](2026-07-23-cad-as-code-research.md).

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

---

## Session 2: 2026-07-12 — AI-native authoring (the "developer writing with Claude Code" persona)

> Method: single-session owner-directed **audit + brainstorm** (not a multi-agent research run),
> grounded in a read of Taliesin's actual machine-facing surfaces (`check`/`vocab`/`schema`/
> `symbols`/`new`/`init`/`blocks`, the `data-block-id`/`data-sourcepos` block model, `llms.rs`/
> `seo.rs`/`feed.rs`) against the ROADMAP guardrails. Prompted by the owner's goal: make Taliesin
> an **AI-native** writing + publishing framework whose *first-class customer is a developer
> authoring with an LLM (Claude Code / Codex)* — without demoting the manual writer. Same rules
> as Session 1: HTML-only; the `.tmd` is the single editing surface and the preview is read-only;
> offline/self-contained; power at the seams. Feeds the roadmap; nothing committed until it earns
> a pin (here, usually a **CLI/JSON snapshot test**, occasionally a corpus doc).

### The headline finding (read this first)

**Taliesin is already one of the most AI-legible publishing tools in existence — but every
AI-legible primitive it has was built for the VS Code companion or for CI, and none is *framed*,
*discoverable*, or *packaged* for a coding agent.** The raw material is excellent; the gaps are the
**protocol** (how an agent learns the loop), the **closed loop** (how an agent *reads what it
rendered* without a browser), and the **grain** (diagnostics an agent can match on and auto-fix).

Three strategic facts drive this session:

1. **The authoring moat is also an *agent* moat.** The content-hash block model, total
   `data-sourcepos`, deterministic freeze cache, and static kernel-free `check` are exactly what a
   coding agent needs — deterministic, addressable, verifiable, browser-free. A batch compiler
   cannot give an agent this.
2. **The linter is the antidote to how LLMs fail.** LLMs hallucinate citations, forget alt text,
   invent cross-reference labels, and write dead links. `taliesin check` *already* catches every
   one (xrefs, `citations_without_bibliography`, duplicate ids, a11y/alt, dead links, reactive
   graph). This is an unclaimed positioning gift: *"Taliesin catches the exact mistakes a language
   model makes."*
3. **The dialect is one LLMs already know.** Keeping Pandoc/Quarto syntax (`:::`, `#|`, `{{< >}}`,
   `[@key]`, `@fig-`) keeps the model inside its training distribution. The corollary is a
   recurring tax: *every place Taliesin "beats a Quarto decision," the model's priors are now
   wrong* — which is exactly why the onramp surfaces below matter.

### Current-state scorecard

| Surface | Exists? | AI-ready? | Gap |
|---|---|---|---|
| `check --format json` → `{diagnostics, environment}` | yes | mostly | no stable error **code**, no **severity**, no structured **suggestion/fix** — "did you mean X" is buried in prose |
| `vocab` / `schema` / `symbols --json` | yes | yes | framed "for the companion," not discoverable by an agent |
| `new` / `init` scaffolds | yes | partial | one doc / bare site; no `AGENTS.md`, no `--json`, thin templates |
| `data-block-id` + `data-sourcepos` | yes | yes | internal; not surfaced as an agent-readable map |
| `llms.txt` / `llms-full.txt` / `seo` / `feed` | yes | yes | reader-side (other people's LLMs); solid |
| **agent onramp doc / protocol** | no | — | no `AGENTS.md`, no skill, no "here's the loop" |
| **text projection of the built page** | no | — | an agent can't cheaply *read what it rendered* |
| **MCP server** | no | — | agent must shell out + parse |
| **project map (`map --json`)** | no | — | no one-shot outline for planning a multi-page edit |

The pattern: **the inputs an agent needs are excellent; the agent has no way to discover them and
no cheap way to see its output.**

### "Where it lives" — new values for this session

These ideas mostly live on surfaces Session 1's taxonomy didn't name. Extend it with:
**cli** (a subcommand / flag), **agent-surface** (a protocol doc / MCP server / skill an agent
consumes), **scaffold** (`new`/`init` output).

### Cluster 1 — Close the loop + the onramp (Tier 1, recommended first)

Format as Session 1: **Name** — what — value {low/med/high/transformational} — **size** {S/M/L} —
**where it lives** — **fit** {fits/needs-care/CONFLICTS}.

1. **Generated `AGENTS.md` (the agent onramp)** — `init`/`new` write an `AGENTS.md` into the
   project (and the repo ships one) that teaches any agent the whole protocol in one file: the
   `.tmd` dialect and its divergences from Quarto, the `check --format json` gate, `symbols`/
   `vocab`/`map` for discovery, **"edit the `.tmd`, never the preview,"** and the build/publish
   commands — value **transformational** — size **S–M** — *scaffold / agent-surface* — **fits**.
   *The cheapest, highest-leverage move: turns every existing primitive into a discoverable
   protocol.* Pin: a test that `taliesin new` output contains a valid `AGENTS.md` whose dialect
   section is generated from `vocab` (so it cannot drift).
2. **`taliesin read <page>` — a text projection of the *built* page** (a.k.a. `render --format
   text`) — a deterministic, screen-reader-like text rendering: resolved xref numbers, figure
   captions + alt text, callout kinds, code with its language, math as TeX/alt, listings — so an
   agent (or a blind author) can **read what it produced** and diff it, with no browser and no HTML
   noise. Just another block-model emitter — value **transformational** — size **M** — *cli /
   build* — **fits**. *Closes the see-what-you-made loop that today only a human-with-browser gets.*
   Pin: a `corpus/…` doc whose text projection is snapshot-tested to list every heading/figure/xref
   number.
3. **Agent-grade diagnostics** — promote each diagnostic from `{file, line, message}` to
   `{code, severity, file, line, column, message, suggestion}`: stable codes (`TAL-XREF-UNDEF`,
   `TAL-A11Y-ALT`) an agent can match on, and today's "did you mean X" moved from prose into a
   structured `suggestion`/`replacement`. `--format human` unchanged — value **high** — size **M**
   — *diagnostics / cli* — **fits** (additive to `Warning`/`Diagnostic`). Pin: a `check --format
   json` snapshot asserting codes + suggestions on a doc with a typo'd key and a bad xref.

> **1 + 2 + 3 compose the full AI-native loop:** *scaffold → the agent knows the protocol (1) →
> edit `.tmd` → `check --json` with codes + fixes (3) → `read` the result (2) → repeat* — browser-
> free, entirely on machinery that already exists.

### Cluster 2 — Packaging & distribution (Tier 2)

4. **A Taliesin Claude Code skill/plugin** — ship a `taliesin` skill (and/or plugin) so *any*
   Claude Code user gets the authoring + verify loop, the dialect crib, and the "source not
   preview" rule without a per-repo `AGENTS.md`. In-repo `AGENTS.md` (3) and in-agent skill
   reinforce each other — value **high** — size **M** — *agent-surface* — **fits**.
5. **`taliesin map --format json`** — one call returns the project outline: pages, titles, nav
   order, drafts, the xref graph, assets, mounts — so an agent can plan a multi-page edit without
   walking the tree. Builds on the site registry + `symbols` — value **high** — size **M** — *cli*
   — **fits**. Pin: a JSON snapshot over the docs book.
6. **MCP server (`taliesin-mcp`)** — wrap `check` / `symbols` / `vocab` / `map` / `read` / `build`
   as MCP tools + resources for hosts that prefer MCP over shelling out. **Keep it read/validate/
   build only** — the write path stays the `.tmd` the agent edits directly; there is never an
   "edit the preview" tool. Local stdio, offline — value **high** — size **M–L** — *agent-surface*
   — **needs-care** (guardrail: no write-back tool; no phone-home).
7. **Correct-by-construction scaffolds + `--json` on `new`/`init`** — richer check-clean templates
   (research-report, paper-with-citations, book) with `[@key]` + `references.bib` pre-wired, and
   machine-readable output so an agent knows exactly what it created and where — value **med** —
   size **S–M** — *scaffold / cli* — **fits**.

### Cluster 3 — The linter as LLM-guardrails (positioning + features)

8. **Position + sharpen `check` as the LLM-mistake catcher** — the linter already catches the four
   things LLMs get wrong (hallucinated citations, missing alt text, invented xref labels, dead
   links). Cash it two ways: **(i) positioning** — make this the marketing + docs spine ("Taliesin
   catches the exact mistakes a language model makes"); **(ii) features** — an optional citation-
   existence/DOI sanity check (opt-in, the one sanctioned network call, off by default), an "alt
   text is empty / looks auto-generated" nudge, and optionally a "numeric/quoted claim with no
   nearby citation" *soft* hint — value **high** (positioning) / **med** (features) — size **S–M**
   — *diagnostics* — **fits** / **needs-care** (the DOI check is the offline exception; the
   claim-without-citation hint has false-positive risk, keep it a soft nudge).

### Cluster 4 — Published-artifact AI-legibility (reader-side LLMs)

9. **Strengthen published-artifact AI-legibility** — `llms.txt`/`llms-full.txt` already ship; add
   per-page machine-readable content (a clean `.txt`/JSON sibling per page — reuses idea 2's text
   projection), schema.org `ScholarlyArticle`/`Dataset` structured data for research posts, and a
   machine-readable BibTeX/CSL export of a page's own references — value **med** — size **M** —
   *build* — **fits**. *Reader-side, not author-side; lower priority per the owner's focus, but
   on-brand for "publish research."* Overlaps the parked **"Cite this" export**.
10. **`build`/`publish` structured errors (`--format json`)** — a failing build in an agent/CI
    context should be parseable, not a human log; mirror `check`'s `{diagnostics}` shape on the
    build path — value **med** — size **S** — *cli* — **fits**.

### Guardrail notes specific to AI-native

- **Single editing surface holds and *helps*.** The agent edits the `.tmd`; MCP/CLI never edits the
  rendered view. A CLI autofix that rewrites `.tmd` *source* is fine — the read-only rule governs
  the *preview*, not the source file.
- **Offline holds.** No render/diagnostic feature phones home. The DOI check (8) is opt-in, off by
  default — the single sanctioned exception.
- **HTML-only holds.** The `text` projection (2) is an agent/diagnostic *view*, not a reader output
  format — name it so it never reads as a new compiler target.
- **The Quarto-divergence tax is real and recurring.** Every improvement over Quarto is a place the
  model's priors silently break; `AGENTS.md` (1) + `vocab` + the skill (4) are the mitigation and
  must enumerate the divergences. Budget for it.

### Recommended first bets

**1 + 2 + 3** — the `AGENTS.md` generator, the text projection, and agent-grade diagnostics. They
compose the complete AI-native authoring loop with no browser, built entirely on the existing block
model. **Idea 1 alone** is a near-free, order-of-magnitude improvement in how well any agent drives
Taliesin today.

### Provenance

Session 2026-07-12. Single-session owner-directed audit + brainstorm (via the `brainstorming`
skill), grounded in a direct read of the machine-facing CLI surfaces and the block model rather
than parallel research agents. Owner selected all four clusters as in-scope and asked for the pool
to be persisted here (the Session 1 → roadmap workflow). Respects the ROADMAP.md guardrails; ideas
in tension are marked needs-care. Nothing here is committed until it earns a pin.

**Graduated 2026-07-12:** all 10 ideas were grounded into code-anchored, adversarially-verified backlog
entries (a 20-agent workflow: per-idea research + anchor verification) and queued in `backlog.md` §G
(Tier-1 trio) + Tier 2/3. Detail file with every verified anchor + pin + first step + open ruling:
[2026-07-12-ai-native-backlog.md](2026-07-12-ai-native-backlog.md).

## Decided against (built, or considered, and deliberately removed)

Recorded so they are not rediscovered as fresh ideas. A verdict here is not "bad idea" —
it is "tried or costed, and the answer was no." Each names the commit that settles it.

- **Section hover previews** — built and then **deleted at `318f22f`**, 13 days before the
  2026-07-24 skimmability audit. Three tests pin the removal. Cross-reference hover cards
  (figures, equations, bibliography entries) were kept; it is specifically *headings*
  previewing their first lines that went. Do not re-scope it from the audit's wish list.
- **Ask-AI hand-off** — shipped at `5e2e8cb`, then **fully reverted at `079a30d`**
  (2026-07-23): a backendless "select a passage → hand it to your own AI" composer. Browser
  extensions do this better, and the provider surface it depended on is unstable (Claude's
  `?q=` prefill was removed around Oct 2025; ChatGPT's is prefill-only). Reviving it means
  re-litigating that, not just reverting the revert.
- **Drag-to-reorder slides** — removed 2026-06-20. It broke the single-editing-surface
  invariant: a second write path fights click-to-source over who owns the file. The in-scope
  way to make a source edit ergonomic is an editor command, never a preview gesture.
- **Read-aloud (`#9` above)** — never built; its SHIPPED mark was an error corrected
  2026-07-25. Out on cost, not on principle.

---

## Session 3: 2026-07-29 — VS Code extension ergonomics (the developer-experience surface)

> **Method.** Author-raised direction: "do a deep dive into the VS Code extension API and
> brainstorm genuinely novel features; I want the best developer experience currently possible."
> Read the full extension API surface (all 38 `contributes.*` points, every `vscode.*` namespace)
> via context7 + `code.visualstudio.com/api`, then filtered every idea through Taliesin's actual
> code, not against a wish list. **Entry format differs from Sessions 1-2 on purpose** (metadata in
> a trailing parenthesis rather than dash-separated) to keep this file free of em dashes going
> forward; the fields are the same: value, size, where it lives, fit.
>
> **Ideas 68-71 shipped 2026-07-30** (backlog items 178 + 177); **67 and 72 stay parked** with
> their reasons intact. Everything in Clusters B-F is still deliberately parked here so it is not
> lost. Nothing below is a commitment.

### Ground truth established this session (verified in source; do not re-derive)

These six facts are what make the estimates below trustworthy, and several contradict the obvious
guess. **Re-check them against source before building, per the standing anti-rot rule**, but they
were read directly, not inferred from a prior note.

- **Fact 1. The LSP's *request* handlers are document-local, but its *diagnostic* path is already
  project-wide. This was recorded wrongly on first pass and corrected the same session; read the
  corrected version.** True half: `lsp_nav`, `lsp_links`, `lsp_complete`, `lsp_format`,
  `lsp_outline`, `lsp_pos`, `lsp_cells` contain **zero** filesystem calls, and `lsp.rs`'s
  hover/definition/completion paths resolve only *relative to the open document's directory*
  (front-matter `.bib` paths, `{{< include >}}` targets). **False half:** "no `_site.yml`
  handling and no cross-page index anywhere". `publish` → `check::buffer_diagnostics` →
  `collect_file_diagnostics_from_src` (`check.rs:293`) calls
  `taliesin_core::site::anchors_defined_elsewhere_in_project` (`site/xref.rs:111`), which finds
  the enclosing `_site.yml` root (`enclosing_site_root`), walks **every page in the project**,
  reads each from disk, resolves its includes, and collects anchor ids. **So `_site.yml`
  discovery and a project-wide anchor set already exist and already run.**
  *Consequence: Cluster C is cheaper than priced below. Re-cost 75 and 76 against this before
  building; the substrate is partly there.*

- **Fact 7. Unresolved xrefs already squiggle, so "semantic tokens make dangling refs red" is
  REDUNDANT.** `cite/validate.rs` flags unresolved `data-tali-xref` markers,
  `validate_xrefs_known_elsewhere` (`check.rs:311`) accepts anchors defined on other pages, and
  the result is published as LSP diagnostics on every change. **This kills the headline pitch
  idea 67 was originally written with.** See the rewritten 67 for what value actually remains.

- **Fact 8. There is no debounce: the whole-book walk runs on every keystroke.** `didChange` →
  `publish` → `buffer_diagnostics` synchronously (`lsp.rs:273-283`, no timer, no coalescing), and
  that path does a full `render_single_doc` **plus** the `anchors_defined_elsewhere_in_project`
  walk. For a 60-page book that is a full re-render plus ~60 file reads and include resolutions
  **per keypress**. Filed as its own concern; it makes the render memo in idea 67's substrate more
  valuable than first thought (it can memoize the anchor scan too), though **debouncing is
  probably the larger and simpler win**. Not yet measured, measure before fixing.
- **Fact 2. Cross-file refs are an acknowledged gap, not an oversight.** `lsp.rs:434` documents
  go-to-definition returning `None` for "an undefined xref, **a cross-file ref**, a missing
  include/bib". Everything project-wide in Cluster C is gated on closing this.
- **Fact 3. `RenderedDoc.xref_numbers` already exists** (`render/model.rs:266`), and `lsp.rs:1323`
  already reads it (`xref_number()`) to answer hover. **The resolved figure/section number is
  already computed**, so the inlay hint that sounds hardest is nearly free.
- **Fact 4. `render_buffer()` is not memoized** (`lsp.rs:1311`): every call re-runs
  `taliesin_core::render_single_doc` under the `serve::guarded` panic guard. Fine for hover
  (user-initiated), **not** fine for inlay hints or semantic tokens (fire on edit / scroll). A
  shared memo keyed on document text is the one piece of new substrate idea 67 needs.
- **Fact 5. `lsp_nav::classify_target` is a point query** ("the token under the cursor"), hand-rolled, no
  `regex`. Semantic tokens and document highlight both need a **full-document** scan, so
  generalizing it to a `scan_all` is shared substrate, not per-feature work.
- **Fact 6. `documentSymbol` already computes whole-section ranges** (`lsp.rs:1453`: `range` is the whole
  section, `selection_range` the heading line). Folding is therefore a re-projection of an
  existing tree, not new analysis.

### Cluster A — The doc-local semantic layer (pure Rust, every editor benefits)

Everything here is a pure function of the open buffer plus its directory, so none of it needs the
Cluster C index. This is the slice chosen for the 2026-07-29 spec.

> **Ranking revised mid-session by Facts 7 and 8.** The original order led with 67 (semantic
> tokens). It does not survive contact with the diagnostics that already ship. **Build order is
> now 68 → 69 → 70/71 → 67 (re-justify or cut) → 72**, and the Fact 8 debounce/memo work should
> be priced first because it is a prerequisite for 68 being pleasant and is a live defect on its
> own terms.
>
> **SHIPPED 2026-07-30: 68, 69, 70 and 71, plus the Fact 8 debounce.** Backlog items 178 + 177,
> against `docs/superpowers/plans/2026-07-29-lsp-editor-ergonomics.md`. **67 and 72 remain parked
> with their reasons below, unchanged and un-rejustified.** Three notes worth carrying, because
> each contradicts an estimate above:
>
> - **Fact 5 was wrong about what was needed.** No `scan_all` was built. Inlay hints turned out to
>   be range-scoped, and document highlight reused `lsp_nav::anchor_occurrences`, which already
>   existed with the exact signature, so idea 70's "falls out of 67's `scan_all`" is now "needed
>   nothing from 67 at all". **67 is not a prerequisite for anything.**
> - **Fact 8's cost was over-estimated, and it was measured rather than argued.** One `publish` on
>   the largest page of the 25-page guide is 33 ms in a debug build, against a 120 ms debounce
>   window. Debouncing alone sufficed; the anchor scan got no memo. `render_buffer` is memoized
>   on `(uri, text)` for the request path.
> - **Idea 67's parenthetical fix for flat delimiters named the wrong mechanism.** It suggested
>   `contributes.semanticTokenScopes` or a bundled theme. The actual fix needed neither: one
>   `contributes.configurationDefaults` → `editor.tokenColorCustomizations` rule on the two
>   `.tmd`-suffixed delimiter scopes, no theme and no semantic tokens. Verified in a real
>   Extension Host. So the math-visibility half of 67 is **done and gone**; whatever case 67 has
>   left does not include it.

67. **Semantic tokens provider (`registerDocumentSemanticTokensProvider`).** **Rewritten after Fact
    7 killed its original justification. The first draft of this entry claimed the value was
    "a dangling ref goes red as you type"; diagnostics already do exactly that. Do not restore
    that framing.** What actually remains, and it is a weaker case:
    - **Distinguish states that are all VALID and therefore have no diagnostic.** The real one is
      *locally defined* vs *defined on another page*: both are correct, neither warns, and only
      one is reachable by go-to-definition (Fact 2). Seeing which is which at a glance is
      information nothing else in the editor gives.
    - **Distinguish kinds where TextMate is approximate rather than wrong**, mainly around
      shortcode and cell-option boundaries.
    - *Not* error surfacing. Diagnostics own that and own it correctly.
    (Value: **medium**, revised down from high. Size: **M**. Where: `lsp` + new
    `lsp_nav::scan_all`. Fit: **fits**.) **Re-justify before building**: on this reduced case it
    may not deserve a slot ahead of 68/69, and the honest option of cutting it is live.
    *This also subsumes the author's "should the LaTeX dollar signs be highlighted" note. The
    lexical half is **already built**: `tmd.injection.tmLanguage.json` scopes
    `punctuation.definition.math.begin/end.tmd`, `markup.math.inline/display.tmd`,
    `meta.embedded.math.tmd`, plus native `#math_body` colouring with no external LaTeX extension.
    If delimiters look flat, that is a theme not painting the punctuation scope, fixable with
    `contributes.semanticTokenScopes` or a bundled theme, not new grammar work.*
    **Open design question, unresolved:** whether Taliesin ships its own VS Code colour theme (the
    project owns a strict `--tali-*` palette with 9 test-banned vendor hexes) or maps its tokens
    onto standard scopes and inherits the user's theme. Decide before implementing.

68. **SHIPPED 2026-07-30. Inlay hints (`registerInlayHintsProvider`).** The most under-rated API for this format,
    because a `.tmd` is full of symbols whose meaning lives elsewhere: `@fig-scree` shows
    `⟨Figure 3⟩` (free, see fact 3); `[@knuth1984]` shows `⟨Knuth 1984⟩` (the bib is already read);
    `{{< include intro.tmd >}}` shows `⟨42 lines⟩`; headings show their computed section number.
    (Value: **high**. Size: **S-M**. Where: `lsp`. Fit: **fits**.)
    **Withdrawn sub-idea, do not re-scope:** a `⟨4.2s · cached⟩` hint on `{python}` fences.
    Cache/timing state lives in the preview server's executor and **the LSP is deliberately
    kernel-free and offline**. It would have to come from TS talking to a live preview, which is a
    much weaker proposition. See idea 79 for where cell state does belong.

69. **SHIPPED 2026-07-30. Folding ranges (`FoldingRangeProvider`).** There is no `folding` key in
    `language-configuration.json` and no LSP folding provider, so **folding is indentation-based
    today**, which is simply wrong for a Markdown-derived format. Fold by heading level, fold a
    `:::` div, fold front matter, fold a cell. (Value: **medium-high**, felt every day. Size: **S**,
    see fact 6. Where: `lsp`. Fit: **fits**.)

70. **SHIPPED 2026-07-30. Document highlight (`DocumentHighlightProvider`).** Cursor on `fig-scree`, every occurrence
    in the file lights up. ~~Falls out of idea 67's `scan_all` for almost nothing.~~ It needed
    nothing from 67: `lsp_nav::anchor_occurrences` already existed with the exact semantics, and a
    second scanner would have been free to disagree with rename about what an anchor is.
    (Value: **medium**. Size: **XS**, and it did not depend on 67. Where: `lsp`. Fit: **fits**.)

71. **SHIPPED 2026-07-30. Selection ranges (`SelectionRangeProvider`), smart expand.** Ctrl+Shift+Right grows word →
    inline math → sentence → paragraph → `:::` div → section. Genuinely pleasant in prose and
    almost never implemented in Markdown tooling. (Value: **medium**. Size: **S**. Where: `lsp`.
    Fit: **fits**.)

72. **Document colour provider (`registerColorProvider`)** for `--tali-*` values in `_site.yml` and
    front matter: native swatches and picker. (Value: **low**. Size: **S**. Where: `lsp`. Fit:
    **fits**.) *Lowest-value item in the cluster; listed for completeness.*

> **Already got, verify rather than build:** sticky scroll for headings derives from
> `documentSymbol`, which the LSP already provides. Confirm it works before speccing anything.

### Cluster B — Authoring gestures (TS-side, the most *felt* per day)

73. **The paste/drop cluster (`DocumentPasteEditProvider` + `DocumentDropEditProvider`).** One
    provider pair, six gestures. Almost nobody implements these well, and they **prevent a bug
    class the build currently only warns about**: `copy_local_assets` warns and skips assets
    outside the doc tree (`build.rs:769`), so the editor can stop that at authoring time.
    (Value: **high**. Size: **M**. Where: `editor/vscode` TS. Fit: **fits** — these are
    author-initiated edits in the editor, not the preview writing back, so single-editing-surface
    is untouched.)
    - **Paste an image from the clipboard** → write `images/<slug>-01.png` beside the doc, insert a
      figure block with a caption placeholder. *The single most-missed feature in every Markdown
      editor.*
    - **Drag an image in from the Explorer** → insert a path relative to the doc; if the source is
      outside the doc tree, offer to copy it in first.
    - **Paste a spreadsheet or HTML table** → a pipe table, then run the existing `lsp_format.rs`
      aligner on it.
    - **Paste a URL over a selection** → `[selection](url)`.
    - **Paste a BibTeX entry** → append to the front-matter `.bib`, insert `[@key]` at the cursor.
    - **Drop a `.csv`** → insert the dataset card plus a loader cell. **Blocked on backlog item
      176** (dataset provenance); build that first or this gesture has nothing to emit.

### Cluster C — Project-level surfaces (all gated on a book index)

**Every item here requires closing fact 1 and fact 2 first.** That substrate is the cost driver,
not the surfaces, and it introduces indexing, invalidation and file watching into a component whose
current statelessness is why it is reliable. Do not cherry-pick a surface from this cluster without
pricing the index.

74. **A project index in `taliesin lsp`** (workspace folders, `_site.yml`, a cross-page xref
    table). Substrate, no user-visible feature of its own. (Value: **enabling**. Size: **L**.
    Where: `lsp`. Fit: **needs-care** — it is the one item in this session that meaningfully
    changes the LSP's architecture.)
75. **Cross-file xref resolution**, closing the `lsp.rs:434` gap. (Value: **high**. Size: **M**
    after 74. Where: `lsp`. Fit: **fits**.)
76. **Workspace symbols (`workspaceSymbolProvider`).** Ctrl+T to any heading, figure or section
    **across the whole book**. Today `documentSymbol` is per-file only, so for a 60-page book this
    is a real daily cost. (Value: **high**. Size: **S** after 74. Where: `lsp`. Fit: **fits**.)
77. **A Taliesin sidebar (`viewsContainers` + `views` + `TreeView`).** Whole-book outline; a "what
    links here" cross-reference view with dangling refs grouped; a figure/table/equation index; a
    bibliography view splitting cited from uncited; a kernel panel (which kernels are warm, what is
    cached, clear-freeze-for-this-page). (Value: **medium-high**. Size: **L**. Where:
    `editor/vscode` TS + 74. Fit: **fits**, read-only navigation only.)
    *Explicitly NOT drag-to-reorder chapters. That is the removed slide-reorder mistake in a new
    costume; see "Decided against" above.*
78. **File decorations (`registerFileDecorationProvider`).** Badge `.tmd` files in the Explorer: a
    warning dot for pages failing `check`, `⚡` for fully cached, a dot for pages with
    never-executed cells. Project health at a glance with zero interaction. (Value: **medium**.
    Size: **M**. Where: TS + 74. Fit: **fits**.)
79. **Status bar item** for kernel state, last build time, cache hit ratio; click to open the
    preview or restart the kernel. (Value: **low-medium**. Size: **S**. Where: TS. Fit: **fits**.)
    *This, not idea 68, is where live cell state belongs, because it can talk to a running preview.*

### Cluster D — Toolchain integration (removes a trip to the shell)

80. **Task provider + `contributes.problemMatchers`.** Auto-discovered `taliesin build` / `check` /
    `build --out` tasks, with `check` diagnostics landing in the **Problems panel for files that
    are not open**. Today project-wide health requires a terminal. (Value: **medium-high**. Size:
    **S-M**. Where: TS + `package.json`. Fit: **fits**.)
81. **Testing API (`tests.createTestController`).** The speculative-but-interesting one. Model each
    page's `check` rules as test items, so a 60-page book gives a green/red tree of which chapters
    still render clean, with gutter icons and per-item re-run, instead of a flat diagnostic list. A
    second reading models **cell execution** as tests, which buys cancellation, run profiles and
    output panes for free. (Value: **medium**, **high** if the cell reading works. Size: **M-L**.
    Where: TS. Fit: **needs-care**, it is an unusual use of the API.)
82. **Terminal link provider.** Make `page.tmd:12:3` in the dev-server log clickable. (Value:
    **low-medium**. Size: **XS**. Where: TS. Fit: **fits**.)
83. **URI handler (`registerUriHandler`).** `vscode://taliesin.taliesin-companion/open?file=…&line=…`
    closes click-to-source for the **standalone browser preview**, which today only bridges back
    inside the webview relay. (Value: **medium**. Size: **S**. Where: TS + `web-client`. Fit:
    **fits**.) *Weigh against the standing note that click-to-source has no automated end-to-end
    coverage: the harness stops at the relay, so this needs a manual check.*
84. **`onWillRenameFiles`.** Rename `intro.tmd` and every `{{< include >}}`, relative link and
    `_site.yml` reference updates via a `WorkspaceEdit`, with VS Code's native refactor preview.
    **Table stakes in TypeScript-land and absent from every Markdown tool I am aware of.** (Value:
    **high**. Size: **M**, and **L** if it must be correct across a whole book, which needs 74.
    Where: TS + `lsp`. Fit: **fits**.)

### Cluster E — AI-native editor surface

85. **`lm.registerMcpServerDefinitionProvider` + `contributes.languageModelTools`.** `taliesin mcp`
    **already exists**; this lets the extension *advertise* it to VS Code automatically instead of
    the user hand-editing config, and register render/check/resolve-xref/vocab as native LM tools.
    Very cheap given the server is built. (Value: **medium-high**. Size: **S**. Where: TS +
    `package.json`. Fit: **fits**.)
    *This does **not** contradict the reverted Ask-AI hand-off. That was rejected because AI
    belongs in the browser extension rather than in the published document. This is AI in the
    **editor**, via the platform's own surface, which is the same principle applied consistently.*

### Cluster F — Editor execution controls

86. **CodeLens on cell fences: `▶ Run cell · ⟲ Run below · ⏹ Interrupt · ⚡ cached (4.2s)`.**
    Where Jupyter parity actually lands, and the editor is the right home precisely because the
    `.tmd` is the editing surface: execution controls belong next to the code being edited, and
    keeping them out of the preview avoids growing a second control surface in the browser.
    (Value: **high**. Size: **M**. Where: TS + preview-server protocol. Fit: **fits**.)
    **Filed as backlog item 175(d) and depends on 175(b), output streaming.** Do not build it from
    both entries.

### Ruled out this session (with the reason, so it is not re-proposed)

- **`registerNotebookSerializer` (open `.tmd` in VS Code's Notebook editor).** Tempting: a mature
  cell UI with real per-cell run buttons, for free. **But it is a second editing surface with a
  serializer that writes back**, and it would round-trip prose through a cell model. It breaks the
  same invariant drag-to-reorder-slides was removed for. Idea 86 gets most of the benefit with
  none of the conflict.
- **`registerCustomEditorProvider` for `.tmd`.** Same objection, same verdict.
- **A `@taliesin` chat participant that edits the document.** Overlaps the settled Ask-AI decision
  and re-opens the write-back question. Idea 85 is the in-scope version.
