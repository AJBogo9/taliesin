# Deck audit — 2026-07-12

A wide, multi-perspective audit of the slide-deck feature (the format the owner has
spent least time on). Three inputs, merged and de-duplicated here:

1. **Live browser testing** (chrome-devtools, viewport matrix) of a purpose-built
   stress deck — the source of the screenshot-verified findings.
2. **Bug fan-out** (8 self-verifying lenses): 43 confirmed/plausible defects.
3. **Feature rationalization** (keep/cut/simplify/redesign/add + a mobile-feed spec).

Everything below respects the load-bearing constraints: HTML-only output, minimal-config
(perfect the default before a knob), single editing surface (preview never writes source),
and block-id / data-sourcepos invariants.

---

## TL;DR — the shape change (owner-decided this session)

The engine core is genuinely good and right-sized (the single-camera model where a pan
*is* the transition and zoom-out *is* the overview). It is **over-built in one place** (a
cluster of overview flourishes) and **under-built in two** (the phone story; the front
door). The four decisions taken this session:

1. **A deck opens AS a deck.** Desktop/landscape → stepped slides. Phone/portrait → a
   vertical **slide-feed** (Reels/TikTok snap-scroll). No more "opens as a document."
2. **Delete reader/scroll mode** (the reflowed-prose document). Its two jobs split cleanly:
   *browse all* = overview; *read through on a phone* = the feed.
3. **Delete print/PDF export.** HTML is the shareable artifact; no PDF. (It is also
   currently broken, see B4-CRIT.)
4. **Trim the overview flourishes** (minimap, LOD cards, storyline threads, the third
   search box, the drawing pen, the van Wijk-Nuij zoom math).

Counts: **43 bugs** (4 high, 25 medium, 14 low) + **6 cuts, 2 simplifications, 11
redesigns, 16 adds** from the feature pass. Do **Part A first** — it deletes whole classes
of bug (the reader-contrast and dark-PDF families) before you ever fix them.

---

## Status — grind-order progress (updated 2026-07-13)

The per-item checklists in Parts A/B/C below are the **original audit findings**, left
as-is as the spec. This block is the **live tracker** against the Part D grind order.

1. **A4 / C-ADD-1 — corpus + browser net** · ✅ **done** (`47ce4b8`). `corpus/deck.tmd`
   expanded to pin fragments · incremental · pauses · code-line-numbers · magic-move ·
   auto-animate · per-slide-bg · `.notes` · vertical stack · `.tali-stretch`; guarded by
   `loose_deck.rs` + `deck_offline_build.rs`.
2. **A1 + A2 — flip the front door; delete reader/scroll + print/PDF** · ✅ **done**
   (`a0110ae`). A deck opens as a deck; reader/scroll + PDF export gone; a minimal
   `@media print` fallback kept (resolves B4-CRIT by removal).
3. **B0 crashes + B1/B2/B3 correctness** · ⏳ **mostly done**. Landed: B0-1/2/3, B1-4/5/7/8,
   B2-9/10/11/12, B3-13/14/16 (`c0a9036`, `87c6008`, `dd7c1bf`, `86c63f6`, `30d3f4c`).
   **B3-17** (open speaker window went stale on a live edit — `sync()` never called
   `updateSpeakerUI`) · ✅ **done** (`3e69d6f`); `sync()` now mirrors
   commit()/applyRemote()'s speaker-mode early-return (browser-verified: a note edit +
   `sync()` repaints the Notes pane). **Still open: B1-6** (speaker previews don't reflect
   fragment/canvas state), **B3-15** (front-matter title/subtitle don't hot-update),
   **B3-18** (a structural edit re-mounts the whole deck — deferred).
4. **A3 — mobile slide-feed** · ✅ **done** (`f97c01f`, first-frame `f8c2898`); pinch-zoom
   unblocked (**B5-1**). **C-ADD-4 (speaker notes as feed narration) NOT done** — the feed
   doesn't surface `::: {.notes}` yet.
5. **C-CUT-2..6 + C-SIMP-1..2 — trim the overview flourishes** · ✅ **done this session**:
   storyline threads · minimap · LOD cards · drawing pen · overview filter box cut; van
   Wijk smooth-zoom + hash `history` knob simplified; `leafAt` kept for the speaker view.
   −526/+33 lines; corpus + 3-viewport browser smoke green; adversarial review clean. Docs
   (`formats.tmd` · `deck-engine.tmd` · `demo.tmd`) purged of the cut features.
6. **B4 theming + B5 a11y + B6 perf** · ⏳ **started** (`3e69d6f`): ✅ **B4-24** (`isDarkColor`
   memoised by colour string — was a forced style/layout flush per colour-bg slide every
   `layout()`) · ✅ **B5-25** (the control menu is a light-dismiss popover, not an ARIA menu:
   launcher now `aria-haspopup="dialog"`, panel `role="dialog"` + aria-label). **Pending:**
   B4-19 chip-contrast · B4-20 dark-bg code · B4-21 custom-theme embed mis-detect · B4-22
   sepia-host embed · B4-23 missing-bg scrim · B5-26 fragment-announce · B5-27
   menu-focus-return · B5-28 overview-announce · B5-29 blackout-announce · B6-30 layout
   re-fit perf · B6-31 overview touch double-fire · B6-32 flat-deck overview wrap.
7. **C-ADD-2/3/5 — share-link + QR · live-input deep-link · wake-lock** · ⬜ **pending**.

**B7 docs drift** · partial — the flourish + pen/annotate mentions are cleaned (this session)
and reader/PDF went with A1/A2; the naming-drift items (`QmdDeck`→`TaliesinDeck`,
`.r-stretch`→`.tali-stretch`, `qhl-`→`tali-hl-`, the `?qmd=embed` passive-mode doc, the
`data-level` note) are not yet audited.

**Next up:** the Step-3 correctness leftovers (B1-6, B3-15), then the rest of Step 6
(B4-19/20 contrast, the B5 announce/focus a11y set, B6 perf), unless the owner reprioritises.

---

## Part A — The redesign (do first; it removes bugs and reshapes the rest)

### A1 — Flip the front door + delete reader/scroll mode  ·  **high**
- [ ] Replace the `syncScroll` block in `deck.js` (~1686-1705). Today the live preview and
  every direct/standalone open default to the reflowed reader, so the author's own
  build-and-inspect loop, and every share link, meets a **prose column, not the deck**
  (live-confirmed: a plain deck URL renders `html.tali-scroll`). Route instead by **aspect**:
  `feed = qmd==='feed' || (standalone && portrait && qmd!=='present')`; landscape / `?qmd=present`
  boots straight into stepped slides (the existing `apply()/layout()` path). Keep `?qmd=present`
  and `?qmd=feed` as transient escape hatches (no config knob). Embedded (`{{< embed >}}`)
  decks never enter the feed.
- [ ] Delete `enterScroll`/`exitScroll` + the ~130 lines of `html.tali-scroll` CSS
  (`deck.css:549-607`) + the "Reader mode" menu button.

### A2 — Delete print/PDF export  ·  **high**
- [ ] Remove `enterPrint`/`exitPrint`, the `beforeprint`/`afterprint` hooks, `?qmd=print`
  mode (`deck.js:1080-1104`), the `html.tali-print` + `@page` CSS (`deck.css:515-547`), the
  "Export PDF" menu item + its `⌘P` key-sheet row.
- [ ] Keep a **minimal `@media print`** fallback so a stray browser Cmd/Ctrl+P prints the
  current slide legibly instead of the raw transformed stage. (This also moots B4-CRIT.)

### A3 — Mobile slide-feed (new; the phone experience)  ·  **high, effort M**
Full-fidelity vertical feed of sticky slides; reuses the identical slide DOM (no wrapper,
no clone) so block-ids, click-to-source, and live `{js}` state survive. Distilled spec:
- [ ] **Layout by font-size, not transform.** Gate on a new `html.tali-feed` class.
  `.tali-slides` becomes a static `overflow-y:auto; scroll-snap-type:y mandatory;
  overscroll-behavior-y:contain` container; each leaf `> section` a `width:100vw;
  min-height:100dvh; scroll-snap-align:center; scroll-snap-stop:always` page, scaled with
  **`font-size: calc(100vw/960 * 40px)`** (all deck content is em-based off the 40px design
  base, so this reproduces the 960-wide stage exactly with zero DOM change). Keep
  `.tali-slide-bg` full-bleed and keep the `.tali-dark-bg`/`.tali-light-bg` contrast flip
  (the explicit difference from the deleted reader, which cleared both).
- [ ] **Fragments: all final.** On feed enter, reveal every `.fragment`/`.incremental`,
  `highlightLines(pre,'all')`, activate the last magic-move block; add a `!important` CSS
  fallback (clone the existing `.overview` override) to kill a first-paint flash. Auto-animate
  pairs render as two normal feed slides. On feed exit, `apply()` re-derives resting state.
- [ ] **Gestures: native snap owns everything.** Guard `onTouchStart/End` with a `deck.feed`
  early-return (reuse the `deck.scroll` guard shape); do **not** attach the deck `keydown`
  handler in feed. Two scroll authorities on one axis is the classic jank bug — delete one.
- [ ] **Chrome: minimal.** Keep only the progress bar + `c/t` counter, driven by an
  `IntersectionObserver` (threshold ~0.6) that reverse-maps the centered section to a flat
  index and calls `updateNumber()/updateChrome()` + rAF-throttled `writeHash()` (so `#/5`
  deep-links work both ways). Feed menu keeps: **Present** (escape hatch to stepped slides),
  **Overview**, **Dark mode**, **Fullscreen**. Drop presenter-only tools.
- [ ] **Invariants.** Add `deck.feed` to `syncInert`'s `showAll` set; branch the facade
  `slide(i)` to `scrollToSlide(i)` in feed (so Alt-click from the editor still highlights the
  block); split a `syncFeedLayout()` off `sync()` that re-flattens + refreshes the IO targets
  **without** calling the camera `apply()` (preserving live `{js}` widget identity).
- [ ] **Legibility.** Do **not** run `fitSlide` in the feed (add `|| deck.feed` to its
  early-return). Fit to width at natural sizes; a dense slide grows into a taller card
  (`min-height:100dvh; height:auto`) rather than micro-type. Floor code at `max(0.5em,11px)`
  + `overflow-x:auto`. A wall-of-text slide becomes a tall scroll-within-a-page — honest
  authoring feedback to split the slide. (Live-confirmed the *need*: a portrait 834px tablet
  today stays stepped at **42% screen coverage** with 321px black bars; the width<600 trigger
  is wrong — key on portrait aspect.)
- [ ] **Pair with the notes-as-narration add (C-ADD-4)** so a followed/after-the-talk feed
  is intelligible.

### A4 — Corpus + browser coverage (prerequisite to cutting anything)  ·  **high, effort M**
- [ ] `corpus/deck.tmd` is 4 slides exercising almost nothing (no magic-move / auto-animate /
  per-slide-bg / vertical-stacks / code-line-numbers). Every "keep" verdict and every cut
  below rests on runtime-untested code. Expand the corpus deck to pin each kept rich feature's
  render output, and fold the ui-audit deck probe into `cargo test` as one browser smoke
  (fragments → code-step → magic-move → overview → speaker). Makes the cuts safe.

---

## Part B — Bug backlog (ranked; deduped across the 3 sources)

### B0 — Crashes / navigation-wedging  ·  **HIGH**

- [ ] **`. . .` pause before a plain code block throws and wedges nav** (high) —
  `deck.js:459`. `fragsOf` runs `node.getAttribute('data-code-lines').split('|')` on every
  `<pre>`; a plain ```` ```python ```` after a `. . .` pause has no `data-code-lines`, so
  `.split` is called on `null` → uncaught TypeError; the camera never moves and chrome
  wedges. Fix: guard `var raw = node.getAttribute('data-code-lines'); if (raw) {…}`.
- [ ] **End key throws on a zero-slide deck** (low, but a crash) — `deck.js:1383`.
  `tops()[-1]` is `undefined`; `isStack(undefined)` reads `.children`. Fix: `if (!T.length) break;`.
- [ ] **Unbounded `code-line-numbers` range OOM-freezes the tab** (medium) — `deck.js:549`.
  `code-line-numbers="10-100000000"` (a typo) makes `parseLineSpec` materialize a giant Set.
  Fix: clamp endpoints to the rendered line count.

### B1 — Steps & animations  ·  correctness

- [ ] **`. . .` pause before `::: {.magic-move}` leaves it permanently hidden** (medium) —
  `deck.js:448`. The magic-move branch never gives the container its reveal step. Fix: mirror
  the PRE branch — `if (node.classList.contains('fragment')) steps.push({frag:node})`.
- [ ] **Overview shows blank magic-move blocks for unvisited slides** (medium) —
  `deck.css:407`. Magic-move `<pre>`s start `opacity:0` until stepped; overview never steps
  them. Fix: `.tali-deck.overview .magic-move > pre:last-of-type { opacity:1 }`.
- [ ] **Speaker previews don't reflect fragment/code state + blank magic-move + blank
  `<canvas>`** (medium) — `deck.js:1018, 987-1017`. `cloneNode` copies base fragment state
  (all hidden) and skips canvas bitmaps, so Current shows nothing revealed and any `{js}`/
  canvas viz previews blank; Next only ever previews the next *slide*, never the next *step*.
  Fix: reveal fragments to `deck.frag` (Current) / `deck.frag+1` (Next) before snapshot; copy
  canvases via `drawImage`/`toDataURL`. (Live-confirmed speaker view otherwise works: math +
  tables + per-slide bg render correctly.)
- [ ] **Auto-animate morph skipped on cross-window (speaker-driven) or hash nav** (medium) —
  `deck.js:953`. `applyRemote`/`onHashChange` call `apply()` (plain pan), bypassing the
  `autoAnimateTo` branch that only `commit()` has. Fix: factor that branch into a helper both
  call. (Live-confirmed auto-animate works on direct forward nav.)
- [ ] **FLIP cleanup on fixed 520ms timers races on rapid navigation** (medium) —
  `deck.js:411,416,430,523,527`. A second click before the timeout strips transforms
  mid-transition (elements jump, old slide flashes, camera restores wrong). Fix: generation
  counter + no-op stale callbacks + `clearTimeout` in-flight; prefer `transitionend`. The
  camera fly already guards this way (`cancelAnimationFrame(deck.flyRAF)`); the FLIP paths
  were left naked.

### B2 — Hash routing, deep-links & ids  ·  correctness

- [ ] **Harden `readHash` (one rewrite fixes four findings)** (medium/high) — `deck.js:1252-1280`.
  Today: (a) a slug starting with a digit (`3 ways`→`3-ways`) or a purely numeric heading
  (`## 2024`→`2024`) is parsed as a numeric *index* and clamps to the wrong slide
  (live-confirmed); (b) any in-slide anchor — a footnote `#fn-…`, a `@fig-`/`@sec-` cross-ref,
  a manual `[x](#id)` — reduces to a bare id (the strip regex makes the leading slash
  optional) and, resolving to a non-`<section>` element, **snaps the whole deck to slide 0**;
  (c) an explicit `{#a/b}` slash-id can't round-trip. Fix: try `getElementById(parts[0])`
  first regardless of shape, climb to `.closest('.tali-slides section')`, and **bail (return
  false) when there's no containing slide** so off-deck/inner anchors are left untouched; gate
  the numeric-index fallback on `/^\d+$/`.
- [ ] **"Title Slide"-titled slide collides with the injected `id="title-slide"`** (medium) —
  `deck.rs` (`slides_html` hardcodes the front-matter title slide's id; `dedup_slug` never
  registers it). Live-confirmed: **two `#title-slide` in the DOM**, so `getElementById` and
  menu-jump/#hash target the wrong section. Fix: register the injected id in the dedup map (or
  namespace it, e.g. `tali-title-slide`).
- [ ] **Explicit `{#id}` on a slide heading is slugified → dead `@ref`/`#hash`** (medium) —
  `deck.rs:303`. `## Two {#sec-My_Two}` emits `<section id="sec-my-two">` but `@sec-My_Two`
  renders `href="#sec-My_Two"` → dead anchor (uppercase/underscore are legal ids the HTML path
  preserves). Fix: route an author-chosen `data-slide-anchor` id verbatim through the dedup
  suffixer; only slugify the heading-text fallback.
- [ ] **Echo-suppression swallows a genuine re-nav to the current slide while fragments shown**
  (low) — `deck.js:1288`. Fix: `((target==null && pf===0) || target===pf)`.

### B3 — Live edit / incremental swap  ·  correctness

- [ ] **Live-inserting `---` or `. . .` isn't treated as structural** (high; 3 lenses
  converged) — `serve/mod.rs:1199` (+ `:844`). The `deck_structural` predicate only checks
  h1/h2 headings, so typing a `---` splices a stray `<hr>` *inside* the current slide (no
  split) and a `. . .` never becomes a pause. Fix: extend the predicate to a thematic-break
  (`<hr`) or pause paragraph → force re-mount.
- [ ] **Editing a post-`. . .` block strips its `.fragment` class (becomes permanently
  visible)** (medium) — `deck.rs:381`. The Update op carries raw block html, not the
  slide-transformed html, so the re-run `add_fragment_class` is lost. Fix: re-run the
  per-slide transform for a within-slide edit before emitting the Update.
- [ ] **Front-matter title/subtitle edits don't hot-update** (medium) — the title slide is a
  raw string outside `doc.blocks`, so the edit produces an empty diff. Fix: map it to a block
  or treat title/subtitle change as a re-mount.
- [ ] **Retitling a slide leaves its `<section id>` anchor stale** (medium) — `deck.rs:348`.
  The `<h2>` updates in place but the section keeps the old slug, breaking `#hash`/`@ref` to
  the new title (and the annotation `drawKey`). Fix: treat heading-text edits as structural.
- [ ] **Open speaker window's panes go stale on a live edit** (low) — `deck.js:1717`. `sync()`
  doesn't call `updateSpeakerUI()`. Fix: call it when `deck.mode==='speaker'`.
- [ ] **A structural deck edit re-mounts the whole deck, nuking every `{js}`/WebGL widget's
  state** (low) — `client.js:946`. Not just the edited slide's. Fix (later): re-mount only the
  affected `<section>` subtree.

### B4 — Theming & contrast

- [ ] **CRIT (mooted by A2): dark deck exports a blank white PDF** — `enterPrint` never removes
  `html.tali-deck-dark`, so print CSS forces white section backgrounds while `--deck-ink`
  stays `#e6e6e6` → **~1.14:1 light-on-white, invisible** (live-confirmed; screenshotted).
  Because `theme:auto` follows the OS, any dark-mode user hitting Cmd+P gets a blank PDF.
  **Resolution: delete print/PDF (A2).** If print were kept, the one-line fix is removing the
  class in `enterPrint`.
- [ ] **Slide-number chip invisible over a light per-slide background in a dark deck** (medium)
  — `deck.css:622`. `rgba(255,255,255,.62)` on `rgba(255,255,255,.12)` (live-confirmed on a
  `lightyellow` slide). Fix: give the chip the same opaque dark-glass token the controls use,
  or derive its ink from the slide's `.tali-dark-bg`/`.tali-light-bg` signal.
- [ ] **Inline `code` / untokenized code text invisible on a `.tali-dark-bg` slide in a light
  deck** (medium) — `deck.css:366`. The dark-bg rule flips text white but code keeps its light
  `#f5f5f5` chip. Fix: pin `.tali-dark-bg pre/code` back to the deck ink.
- [ ] **Custom/extension-themed embedded deck is mis-detected as standalone** (medium) —
  `deck.rs:155`. `deck_theme_head` emits nothing for a custom theme, but that script also sets
  `window.taliDeckEmbedded`, so a custom-themed `{{< embed >}}` takes the standalone path. Fix:
  emit the embed-detection flag unconditionally; keep only the light/dark color resolution
  behind the theme gate.
- [ ] **Embedded deck ignores a sepia host → dark deck in a cream page** (medium) —
  `deck.rs:164`. `hostTheme()` maps only dark/light. Fix: map `sepia → light`.
- [ ] **A failed/missing `background-image` leaves forced-white text on the bare canvas**
  (medium, plausible) — `deck.js:316`. Text is flipped white (assume-dark) but no image paints.
  Fix: give the bg layer a neutral dark scrim so assumed-white text always has a backdrop.
- [ ] **`isDarkColor` forces a sync style/layout flush per color-bg slide every `layout()`**
  (low, perf) — `deck.js:352`. Fix: memoize by color string / stash the boolean on the section.

### B5 — Accessibility

- [ ] **Viewport `user-scalable=no, maximum-scale=1.0` blocks pinch-zoom on every deck view**
  (high, WCAG 1.4.4/1.4.10) — `deck.rs:73`. Especially wrong for the mobile **feed**, which is
  a *reading* surface. Fix: drop `maximum-scale`/`user-scalable` (keep `width=device-width,
  initial-scale=1`).
- [ ] **`aria-haspopup="menu"` over a plain button group with no menu semantics** (medium) —
  `deck.js:1487`. Fix: change the token to `"dialog"`/`"true"` to match the shipped light-dismiss
  popover (avoids the larger cost of full menu roles + roving tabindex).
- [ ] **Fragment/incremental reveals never announced to AT** (medium, WCAG 4.1.3) —
  `deck.js:557`. Fix: in `fragChanged`, write the revealed fragment's text (or "Step k of n")
  to the existing `.tali-deck-live` region.
- [ ] **Closing the control menu drops focus to `<body>`** (medium, WCAG 2.4.3) — `deck.js:1584`.
  Fix: return focus to `deck.menuBtn` if focus was inside the menu.
- [ ] **Overview highlight nav moves no focus / announces nothing** (low, plausible) —
  `deck.js:779`. Fix: announce the highlighted "Slide N of M: title" via the live region.
- [ ] **Blackout not announced; underlying controls stay in the AT tree** (low, plausible) —
  `deck.js:1400`. Fix: announce "Screen blanked"/"Resumed".
- [ ] *(Mooted by cut)* Draw-toolbar colour buttons unlabeled (`deck.js:1156`) — the pen is
  being cut (C-CUT-5); moot unless kept.

### B6 — Performance / robustness

- [ ] **Every `layout()` re-fits all slides + rebuilds the minimap** (medium) — `deck.js:242`.
  O(N) forced reflows per resize frame and per live-edit; janks a 100-200 slide deck. Fix:
  skip per-slide re-fit on a pure viewport change; rebuild minimap tiles only on slide-count
  change (and the minimap is being cut anyway, C-CUT-3). Pairs with the autofit-staleness fix
  (attach a rAF-debounced `ResizeObserver` to the current slide so late `{js}`/`<img>`/KaTeX
  content re-fits — `fitSlide` only measuring at layout/resize is its own medium defect).
- [ ] **Overview touch double-fires** (high) — `deck.js:1419-1431`. A one-finger swipe in
  overview pans the map (pointer handlers) *and* fires `right()/left()/down()/up()` (touch
  handlers) → you exit overview onto a slide you never chose; also there's no pinch-zoom on
  touch. Fix: early-return `onTouchStart/End` when `deck.overview`; add two-finger
  pinch-zoom-to-centroid reusing `onOverviewWheel`'s math.
- [ ] **Overview of a flat (all-`##`) deck is one thin row of specks** (medium) — `deck.js:74-100`.
  Live-confirmed: 17 slides → a 1×17 strip of ~85px tiles in a band with huge empty space;
  up/down inert. Fix: reflow a long single-topic run into a wrapped near-square grid for the
  overview map only (preserve the straight-line present-mode pan).

### B7 — Docs drift (all in `docs/`)

- [ ] `?qmd=embed` "passive" mode is documented (`deck-engine.tmd:134,166`) but never
  implemented (runtime reads only speaker/print/normal; `embed_html` emits no `?qmd`). Rewrite.
- [ ] Speaker previews are described as `?qmd=embed` iframes (`:170`); they are DOM-clone
  snapshots. Rewrite.
- [ ] Line-stepping classes named with the pre-rename `qhl-` prefix (`:96,99,100,106`); runtime
  is `tali-hl-`. Replace.
- [ ] Public API called `window.QmdDeck` (`:12`); canonical is `window.TaliesinDeck` (QmdDeck is
  a back-compat alias). Fix.
- [ ] Guide teaches deprecated `.r-stretch` (`formats.tmd:158,165`); canonical is `.tali-stretch`.
- [ ] "reader mode is automatic on a portrait screen" (`formats.tmd:209`) — trigger is width-only
  today (and changing to aspect per A1/A3).
- [ ] Reader/PDF removal doc-map: purge reader + PDF + scroll-as-default from `formats.tmd`,
  `demo.tmd`, `deck-engine.tmd` when A1/A2 land; document the feed.
- [ ] `data-level` section attribute is emitted (`deck.rs:357`) but consumed by no runtime code
  and documented nowhere — it's the corpus-invariant count anchor. Add a one-line note or drop it.

---

## Part C — Feature verdict (keep / cut / simplify / add)

Verdict: *"Trim the flourishes, flip the front door."* Full detail in the feature-pass
output; the decisions:

### Cut (remove)
- [ ] **C-CUT-1 Reader + Print fallout** (high) — the largest dead-code block once A1/A2 land.
- [ ] **C-CUT-2 Storyline threads** (high, S) — pure `aria-hidden` decoration rebuilt every
  layout; the grid already conveys the grouping. `deck.js drawThreads` + CSS.
- [ ] **C-CUT-3 Minimap** (high, M) — only appears zoomed-in *past fit inside* overview; a
  10-30 slide deck never reaches it. "Press 0 to fit" + clicking tiles covers it.
- [ ] **C-CUT-4 LOD title cards** (medium, M) — a per-slide DOM node every layout for a
  far-zoom state realistic decks never hit. (If you keep exactly one overview extra, keep this.)
- [ ] **C-CUT-5 Drawing/annotation pen** (medium, M) — ~90 lines of bespoke canvas engine for
  the lowest-frequency, deliberately-ephemeral feature. Replace with a transient laser only if
  live emphasis is ever wanted.
- [ ] **C-CUT-6 Overview filter box** (medium, M) — a *third* find-a-slide-by-title surface
  (menu jump-list + Cmd-K already exist).

### Simplify
- [ ] **C-SIMP-1 van Wijk-Nuij smooth-zoom** (medium, M) — ~55 lines of perceptual math whose
  only effect is fancier easing on overview-enter/jumps. Route every move through `applyCam` +
  the existing CSS transition.
- [ ] **C-SIMP-2 Hash `history:true` knob** (low, S) — undocumented, unset. Keep `replaceState`
  deep-linking; drop the alternative path.

### Add (high bar; only real wins)
- [ ] **C-ADD-1 Corpus + browser coverage** (high) — = A4. Prerequisite.
- [ ] **C-ADD-2 Share-this-slide: copy-link + offline QR** (high, S) — `writeHash` already
  deep-links every position; just surface it. Live: QR the chart slide, the room lands on that
  exact slide+fragment. After: a link reopens the exact state. ~1-2KB client encoder, works on
  `file://`.
- [ ] **C-ADD-3 Live `viewof`/`input` state in the deck deep-link** (high, L) — turns the
  tool's real differentiator into a shareable artifact no competitor can match (drag churn to
  8%, share, recipient lands with churn=8% computed). Also fixes a **latent bug**: the
  page-level `{{< input >}}` hash writer and the deck's hash writer clobber each other inside a
  deck today. Reconcile the two writers (deck owns `#/slide/v/frag`, input-state a suffix).
- [ ] **C-ADD-4 Speaker notes as reader "narration" on the feed** (high, M) — reuses
  `::: {.notes}` authors already write, making a followed/after-the-talk phone feed
  intelligible. Reader-visible on the feed/read surface only, never in present/overview (ratify
  this contract shift). Ships with A3.
- [ ] **C-ADD-5 Screen Wake Lock while presenting** (medium, S) — ~15 lines; stop the display
  dimming mid-sentence.

### Rejected as out-of-scope (considered, declined)
- Configurable transition picker (the camera-pan *is* the identity; minimal-config anti-pattern).
- Presenter webcam recording / video export (a de-facto new output format).
- Live audience polls / Q&A (needs a backend, breaks static-HTML identity; the reactive `{js}`
  what-if widget is the constraint-respecting substitute).

### Keep as-is (earn their place)
Stepped nav + grid camera · autofit (`fitSlide`) · fragments/incremental/pauses · code
line-stepping · magic-move · auto-animate (weakest keeper) · overview *map* (shed its extras) ·
per-slide backgrounds + contrast-flip · `.tali-stretch` · speaker view (live-confirmed working)
· postMessage sync · blackout (add it to the menu — it's the one present-action that's
key-sheet-only) · fullscreen · control menu (reconcile its tool list) · progress bar + arrows ·
slide-number chip (fix contrast) · dark-mode + host-follow · `{{< embed >}}` iframe.

---

## Part D — Recommended grind order

1. **A4 / C-ADD-1 — pin the kept features in the corpus + one browser smoke.** Everything else
   is safer once the net exists.
2. **A1 + A2 — flip the front door; delete reader + print.** Removes the whole reader-contrast
   and dark-PDF bug families and the standalone-default astonishment in one sweep. Then B7 docs.
3. **B0 crashes + B1/B2/B3 correctness.** The `. . .`-pause-before-plain-code crash, the
   `readHash` hardening, and the live `---`/`. . .` structural miss are the highest
   user-visible bugs that survive the removals.
4. **A3 — the mobile feed** (+ C-ADD-4 narration). The big new capability.
5. **C-CUT-2..6 + C-SIMP-1..2 — trim the flourishes.** Now safe (corpus net exists). Big code
   reduction, lower maintenance.
6. **B4 theming + B5 a11y + B6 perf/robustness + remaining redesigns** (chip contrast, dark-bg
   code, overview touch double-fire, overview flat-deck wrap, FLIP race, speaker fragment/canvas
   state).
7. **C-ADD-2/3/5 — share-link + QR, live-input deep-link, wake lock.** The "richer browser
   behavior in a live HTML view" wins that make the shared HTML the reason no PDF is needed.

---
*Screenshot evidence for the live-confirmed items (dark-PDF blank page, the light-bg chip, the
portrait-tablet letterbox, the reflowed mobile document, the overview thin-row, speaker view) was
captured this session in the scratchpad. Feature-pass full detail and the mobile-feed long-form
spec are in the workflow output.*
