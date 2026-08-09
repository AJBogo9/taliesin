# AP7: deep accessibility of the output (2026-07-25)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Perspective:** AP7 from `backlog.md`'s "Audit perspectives". Ruled the pick by the owner on
2026-07-25 after the backlink-context batch emptied the last buildable band-C bullets.
**Run solo** against the tip of `backlog/backlink-context-and-resume` (`befcb6c`), release binary,
real built output plus a live preview. **Nothing was changed**: this round is findings only, per
the ruling ("do not pick a feature item").

## Headline

**The rendered *document* is in good shape; the rendered *application* is not.**

Every static, one-shot surface an AT consumes came back sound, and several came back better than
the entry claimed: the deck hides non-current slides with `inert`, KaTeX ships MathML with the
visual twin `aria-hidden`, tabsets implement the full APG pattern, the drawer is `display:none`
when closed, and after a correctly-instrumented measurement there is **not one invisible focus stop
on a whole chapter**.

The defects are all in the same place: **content that changes without the reader operating a
control is never announced, and the one static rule that should have caught the outline defect is
structurally blind to it.** Four of the five findings are variations on that.

Two of them touch the moat directly (the reactive `{js}` graph and the incremental block swap),
which is exactly where the entry predicted the yield would be.

## The entry's own premises, re-measured first

Per the standing method ("every audit's first job is to falsify its own entry"). The three premises
the backlog had already flagged as refuted all hold as refuted, and I refuted a fourth:

| Premise | Verdict |
|---|---|
| "no live-region announcement on slide change" | **Refuted** (already recorded). `deck.js:1651` builds a polite region; `announce()` fires on slide, fragment step and overview move. |
| "no `aria` on the deck" | **Refuted** (already recorded). `render/deck.rs:487` emits `role="group" aria-roledescription="slide"`; `updateSlideLabels` adds "Slide N of M". |
| "KaTeX a11y: the visible twin may be read twice" | **Refuted, and the open question is answered: it is not.** The emitted span is `<span class="katex-html" aria-hidden="true">` beside a real `<math>`. Measured in `guide/using/writing.html`. Nothing to do. |
| *(new)* "off-screen deck slides leak into the AT buffer", my own hypothesis | **Refuted by measurement.** Non-current slides carry `inert`, so the full a11y tree contains **3** slide nodes, all "Slide 1 of 19". The deck is correct here. |

The do-not-re-find list also holds: `TAL-A11Y-HEADING`/`-NAME`/`-ALT` exist (`diagnostics/codes.rs:127-130`),
`scanA11y` is live in `web-client/client.js:215`, and the item-11 interaction set is in place
(the settings menu toggles `aria-expanded` and returns focus to its trigger on Escape, verified).

## Verified findings, ranked

### AP7-1 (medium-high): 37 of 51 book pages emit a skipped heading level, and the project's own heading rule cannot see it

**Symptom.** `taliesin check docs/guide` prints `no problems found`, while the built pages emit:

| page | emitted outline |
|---|---|
| `guide/using/formats.html` | `h1 h3 h4 h4 … h3 h3` |
| `guide/reference/cli.html` | `h1 h4 h4 h4 h4 h3 h3 …` |
| `tarn/grouping.html` | `h1 h3 h3 h4 h4 h3` |

Measured across `docs/guide` + `docs/internals` + `corpus/tarn`: **37 of 51 pages** with an outline
skip a level (35 × `h1→h3`, 2 × `h1→h4`). `h2` is empty on essentially every chapter in both
dogfood books.

**Two independent causes, both re-derived from source (not assumed):**

1. **Demotion is an absolute `+1`.** `render/mod.rs:2490 demote_heading_html` maps `hN → h(N+1)`
   whenever a title block is emitted. That is right for a `#`-rooted chapter (`# → h2`) and wrong
   for every other rooting: the house style for both dogfood books is `##`-rooted (a `#` would
   restate the front-matter `title:`), so the first body heading lands at `h3`; `cli.tmd` opens at
   `###` and lands at `h4`. The build's *TOC* already does the right thing, taking two levels
   below the **shallowest heading present**, so the two disagree.
2. **The rule never sees the page's `<h1>`.** `diagnostics/a11y.rs:211-227` walks the block stream
   with `prev = 0` and only reports `lvl >= prev + 2`. The title block *is* `blocks[0]`
   (`render/mod.rs:1133`), but its html is `<header class="tali-title-block">…<h1>`, and
   `helpers.rs:47 heading_level` requires the html to **start with** `<hN`. `"<header".as_bytes()[2]`
   is `e`, not a digit, so it returns `None`. The `<h1>` is skipped, `prev` stays `0`, and the
   first body heading (the one carrying the largest jump on the page) is never compared to anything.

So the single most common heading skip in the entire corpus is precisely the one shape the rule is
structurally incapable of reporting.

**Why it matters.** Heading-level navigation is the primary way a screen-reader user moves through
a long chapter. Today "jump to next level-2 heading" finds nothing on 37 of 51 pages, and the
outline reads as though every section is nested two or three levels under something that is not
there. WCAG 1.3.1 / technique G141.

**Note before fixing:** changing demotion changes emitted levels, which `site/chapter.rs` numbers
*post*-demotion. The SKIM work threaded a per-site base through `ChapterNumbering` for exactly this
reason, so the relative-demotion fix and the section-number slot have to move together or `@sec-`
refs will drift. Fixing only cause (2) (teach `heading_level` about the title block, or seed
`prev = 1` when a title block is emitted) makes `check` *report* the problem on 37 pages without
fixing it, which may be the right first step but is not a silent change.

### AP7-2 (medium): the reactive `{js}` graph rewrites the document silently

**Measured**, on the built `corpus/reactive/inputs.tmd`, driving the slider **from the keyboard**
(focus the control, five × `ArrowRight`):

```
before -> outs: "k doubled (transitively) = 6" … "k=3 n=20 on=true"
        live: (EMPTY)
after  -> outs: "k doubled (transitively) = 16" … "k=8 n=20 on=true"
        live: (EMPTY)
```

Six output regions rewrote themselves; **every** `[aria-live]` / `[role=status]` / `[role=alert]`
region on the page stayed empty, and no `.tali-js-out` node carries `aria-live` or `role`
(7 of 7 report `{live: null, role: null}`). Confirmed by source: `tali-js.js` contains no
`aria-live` anywhere.

The control itself is fine: `{{< input >}}` emits a real `<label for>`/id pair
(`render/extension/mod.rs:321`) and is fully keyboard-operable. A screen-reader user therefore
hears the slider value change and is told nothing about the six regions of the document that just
changed with it. This is the "explorable explanation" feature, the most web-native thing the tool
does, and it is silent.

### AP7-3 (medium): `.scrolly` and `.code-walkthrough` carry no accessibility semantics at all

**Measured** on built `corpus/explorable/scrolly.tmd` and `corpus/narrate/walkthrough.tmd`:

| | steps | focusable steps | steps with `aria`/`role` | live regions inside | root `role` / `aria-*` |
|---|---|---|---|---|---|
| `.tali-scrolly` | 3 | **0** | **0** | **0** | `null` / `[]` |
| `.code-walkthrough` | 4 | **0** | **0** | **0** | `null` / `[]` |

Corroborated by source: `scrolly.js` and `walkthrough.js` contain no `keydown`, `tabindex`, `role`
or `aria-*` (walkthrough's `focusLines` is a *visual* line highlight, unrelated to DOM focus), and
`divs.rs` emits neither for either construct.

Both are driven purely by scroll position through an IntersectionObserver trigger line. The step
prose is readable linearly, so a screen-reader user gets the words. What they never get is the
**stage**: the sticky visual (or the highlighted code lines) that each step is talking about, whose
state advances only as a consequence of *visual scrolling* and is never named, never announced, and
never associated with the step that drives it (no `aria-controls`, no `aria-describedby`).

*Scope honestly stated:* I did not succeed in driving a state transition from the headless harness
(`scrollIntoView` did not cross the trigger line, the known scroll-feature testing gotcha), so I am
reporting the semantics, which are unambiguous and were measured twice, not a claim about when the
stage flips.

### AP7-4 (low-medium, author-facing): an incremental block swap strands keyboard focus and announces nothing

**Measured** against a live `preview` by editing the source file underneath the browser:

```
A. focus inside the EDITED block   : a:"link one"  ->  <body> (focus lost)
B. focus in an UNRELATED block     : a:"link two"  ->  a:"link two"   (survives)
   live regions in both cases      : (nothing about the change)
```

`client.js:1276` does `keepScroll(() => el.replaceWith(node))` (and `el.remove()` at :1312) with no
focus handling. The blast radius is already minimal, since the block-level diff means an unrelated
block keeps focus, which is the design paying off. But when the reader *is* in the block that
changed, focus drops to `<body>` and the next Tab restarts from the top of the document. Nothing is
announced either way.

This is **preview-only** (a built page has no websocket and no swap), so it costs an author who
works keyboard-first or with AT, not a reader. That is why it ranks below AP7-2/3 despite touching
the moat.

### AP7-5 (low): the in-page TOC is the second-to-last thing a keyboard user can reach

A full tab walk of `guide/using/formats.html` at 1440×900 is **62 stops**, in this order: 6 chrome
stops, then 48 content stops (every heading anchor and every code copy button), then **the 4 TOC
entries at stops 56-59**, then prev/next. The TOC is a *sticky sidebar, visible the whole time*, but
a keyboard user has to traverse the entire 10,000 px chapter to put focus in it.

Screen-reader users are fine: `<nav id="TOC" role="doc-toc" aria-label="Table of contents">` is
exposed as a `doc-toc` landmark (verified in the full a11y tree) and is reachable from the landmark
rotor. This lands specifically on **keyboard-only users who are not running AT** (motor impairment,
switch access, power users). The existing skip link goes to `#tali-main` only.

## Verified sound, do not re-audit these

Recorded so a later round does not spend itself re-deriving good news.

- **Deck**: non-current slides are `inert` (out of the a11y tree *and* the focus order); the polite
  live region says `Slide 2 of 19: What decks are` / `Step 1 of 1`; deck chrome is 3 clean labelled
  tab stops (`Previous slide` / `Next slide` / `Menu`).
- **KaTeX**: `<math>` MathML plus `aria-hidden="true"` on the visual twin. Settled.
- **Tabsets** (`divs.rs:609-631`): `role=tablist` / `tab` / `tabpanel`, `aria-controls`,
  `aria-labelledby`, `aria-selected`, roving `tabindex`, arrow keys, and `hidden="until-found"` so
  find-in-page still reaches inactive panels. Full APG pattern.
- **Book drawer**: emitted with a bare `hidden`, and `.tali-book-drawer[hidden] { display: none }`
  (`site.css:273`), so its 19 chapter links are **not** phantom tab stops.
- **Settings / reader-prefs menu**: `aria-expanded` toggles `false → true → false`,
  `aria-controls="tali-rmenu-panel"`, and Escape returns focus to the trigger button.
- **`{{< input >}}` controls**: real `<label for>` association, keyboard-operable.
- **Focus indicators**: `0` invisible focus stops, `0` zero-size stops, `0` unnamed stops across all
  62 stops on a chapter (see the false lead below before doubting this).
- **Landmarks**: `banner`, `main`, `navigation "Pagination"`, `doc-toc`, plus named `region`s for
  scrollable code blocks. `banner`/`main` are unnamed, which is correct for singletons.

## False leads, mine rather than the project's

All three were my own instrumentation, and all three would have shipped as confident wrong findings.
Recorded because the method generalises.

1. **"34 invisible focus stops on one chapter"** (would have been the headline). Artifact of reading
   `getComputedStyle(el).opacity` immediately after `Tab`. `.tali-copy` and `.tali-anchor` are
   `opacity: 0` with `:focus-visible { opacity: 1 }` **and** a `transition: opacity var(--tali-dur)`,
   and `--tali-dur` is `.12s`, so a synchronous read returns the *interpolated* value, near 0. With a
   220 ms settle the count is **0**. **Any probe that judges visibility, opacity or transform must
   settle past `--tali-dur` first.**
2. **"headings emit two `id` attributes"**. Artifact of a regex alternation `(?:id|class|…)="`
   matching *inside* `data-block-id="…"`. The emitted tag is correct.
3. **"the a11y tree carries only 1 heading"**. Artifact of `accessibility.snapshot({interestingOnly: true})`.
   The full tree carries all **17**.

## Not chased (scope, recorded so it is not mistaken for a clean bill)

- **A real screen reader.** Everything here is Chrome's accessibility tree plus computed style plus
  keyboard driving. NVDA/Orca verbosity and the actual *wording* an AT speaks (e.g. whether the deck
  live region is chatty in practice) are untested.
- **Colour contrast**, deliberately: it is `check`'s documented non-goal (needs computed CSS) and the
  polish rounds cover it.
- **Callouts, theorem boxes and `<details>` proofs as composite widgets**, inspected only in
  emitted HTML, not driven.
- **The mobile-TOC sheet**, since a site preview emits no sheet chrome at all (recorded in item 11's
  notes and re-confirmed here); it is reachable only in a single-doc preview.
- **Reduced-motion** across the scroll-driven features.

## Method

Release binary at `befcb6c`; `docs/guide`, `docs/internals`, `corpus/tarn` built to a scratch dir and
served statically; `corpus/deck.tmd`, `corpus/explorable/scrolly.tmd`, `corpus/narrate/walkthrough.tmd`
and `corpus/reactive/inputs.tmd` built standalone; one live `preview` for the swap probe. Browser
driving via the project's own `puppeteer-core` (`tools/ui-audit/node_modules`) with a private
`userDataDir`, because **the chrome-devtools MCP was unusable: a parallel session held
`~/.cache/chrome-devtools-mcp/chrome-profile`**, which is the documented fallback. Seven probe
scripts, kept in the session scratchpad. No repo file was modified; the tree was verified clean
after the run.
