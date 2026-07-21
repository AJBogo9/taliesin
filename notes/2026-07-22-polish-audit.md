# Polish audit (2026-07-22)

A **wide, deep polish audit** to refill the backlog: every existing surface examined for
micro-craft, silent holes, inconsistency, and unintuitive implementation, with the goal of
making the tool feel *extremely* finished. Successor lens to the 2026-07-19 feature-polish
audit (whose PL1–PL20 all shipped); this one adds a large **empirical browser sweep** the
prior audit couldn't do (its chrome-devtools profile was blocked).

## Method / provenance

- **Empirical browser sweep (orchestrator, chrome-devtools MCP):** the tech-blog site
  (home / blog / publications / projects / cv / posts), the demo-book (index + chapter), the
  deck (`deck.tmd`, feed **and** stepped modes), and the feature docs (callouts, panels,
  gallery, scrolly) — across light/dark/sepia, laptop + narrow, checking console, heading
  structure, overflow, tap-targets, landmarks, and `<head>` completeness. Findings tagged
  **[browser]** were reproduced live. Lighthouse was unusable here (`NO_FCP`: the headless MCP
  window won't paint for it), so a11y was checked with hand-written DOM probes instead.
- **4 read-only code auditors (one per surface: CSS/theming · client JS behavior · CLI/DX +
  diagnostics wording · emitted-HTML semantics/a11y),** each briefed with the shipped PL1–PL20
  + open backlog + invariants so they hunt *new* ground. Findings tagged **[agent]**; the
  subset the orchestrator independently grepped to the line is tagged **[verified]**.
- **Standing law honored:** entries rot; **grep the named symbol before promising anything.**
  [agent] items cite a real `file:line` the auditor read, but were not all independently
  line-checked — verify before building, and confirm a fix by mutation.

## Excluded as already-tracked (checked against `backlog.md` before filing)

Not re-reported here even though they surfaced: **video autoplay/pause + single active player**
(backlog B7 — but see PA-A1/PA-B7 for two *new* nuances), **image lazy-load / WebP-srcset**
(deferred "Image optimization"), **cross-ref labels English-only** (parked #9), **deck
generator-meta / ASCII banner** (item 11 — but the deck **favicon 404** + **theme-color** in
PA-H1 are new), **update-notifier** (DX16), **client.js has no JS tests** (noted P3).

---

## Top picks (highest value, mostly small)

| # | Finding | Where | V/E |
|---|---------|-------|-----|
| PA-H2 | **Listing/section pages emit no `<h1>`** (blog/publications/projects start at H3/H2) | site listings | high · M |
| PA-H1 | **Deck `<head>` is bare**: no favicon (→ /favicon.ico 404 + blank tab), no theme-color, no description/OG — **favicon ✓ shipped 2026-07-22 (`dc58aa9`)**; theme-color/OG still open | `deck.rs` | high · S |
| PA-C1 | **Cite-this "Copied!" + deck active buttons fail AA** (wrong accent token, white ≈2.3:1 in dark) | `site.css:379`, `deck.css:409` | high · S |
| PA-S1 | **`site.css` never migrated to PL11 geometry/motion tokens** (0 uses vs base's 31/8) | `site.css` | med · M |
| PA-D1 | Deck controls (theme seg, share, speaker) have **no `:focus-visible` ring** | `deck.css:933` | med-high · S |
| PA-B1 | Kernel-unavailable error tells headless `build`/`read`/CI to **"click Restart kernel"** (a button that isn't there) | `exec.rs:333` | med · S |
| PA-A2 | Lightbox **gallery step is silent to screen readers** (no `aria-live`; every sibling has one) | `11-lightbox.js:81` | med · S |
| PA-P1 | **Printed links lose their URL** (no `a[href]::after` expansion) | `base.css:831` | med · S |
| PA-M1 | Card/date semantics: dates are `<span>` not `<time>`; listing is a `<div>` not a list | `mod.rs:1015`, `site/mod.rs:1285` | med · S/M |
| PA-B2 | `check` human diagnostics are **uncolored** (doctor + dev-log colorize; rustc/cargo do too) | `check.rs:451` | med · S |

---

## I. Empirical / browser findings (orchestrator, reproduced live)

### PA-H2 — Listing & section pages have no `<h1>`. [browser, verified]
`/blog.html` renders `h1=0` (outline starts at the H3 post-card titles), `/publications.html`
`h1=0` (starts at H2 "Theses"), `/projects.html` `h1=0` (starts at H3 cards). Only real content
pages (`/cv.html`, posts) and the homepage hero carry an `<h1>`. A screen-reader/heading-nav
pass drops straight into H3 cards with no page context, the visible page name lives only in the
nav + `<title>`, and it's an SEO gap. The page title is *known* (the nav label / listing block
heading), so an `<h1>` can be emitted with zero config. **Fix:** emit a page `<h1>` for
listing/`about`/section-index pages (visible, or visually-hidden if the design wants no big
title), and demote the listing section heading + cards one level so "Recent Posts" is an H2 over
H3 cards (today "Recent Posts" is an H3 sibling of the cards it contains). **high · M · [author/site].**

### PA-H1 — The built deck's `<head>` is missing everything a page has. [browser, verified]
A built deck emits only `viewport` + `referrer` meta + `<title>` + `lang`; it has **no favicon
link** (so the browser requests `/favicon.ico` → **404** in console, and the tab shows a blank
default icon), **no `<meta name="theme-color">`** (mobile browser chrome stays white against the
dark deck), **no `generator`**, **no `description`/OG**. Pages have all of these. **Fix:** share
the page's `<head>` favicon + theme-color + generator emission into `deck.rs`'s head assembly.
Folds in backlog item 11's "decks lack generator meta." **high · S.**

### PA-A3 — Anchor-copy `#` bleeds into the accessible name of headings *and* figcaptions. [browser]
The copy-link `<a aria-label="Copy link to this section">#</a>` lives *inside* the heading
(`h2` accessible name becomes "Recent Posts, Copy link to this section") and inside figcaptions
("Figure 1: No pooling.#"). The `#` is real text, not a pseudo-element, so it also shows in
`textContent`. **Fix:** make the copy-link's `#` glyph `aria-hidden` (the button keeps its
`aria-label` for its own name) and/or place it outside the heading's name computation. **low · S.**

### PA-A4 — Small tap targets at narrow width (WCAG 2.2 SC 2.5.8, 24px min). [browser]
At ~500px the footer links (`a.tali-foot-item`, ~43×17), nav brand (`a.tali-nav-brand`, ~143×16),
and heading anchor (`a.tali-anchor`, ~14×26) fall under the 24×24 minimum. Inline-text-link
latitude covers some, but the footer and nav-brand are standalone controls. **Fix:** raise the
footer/nav-brand hit area (padding) to ≥24px tall on touch widths. **low · S.**

### PA-A5 — A build ships an `<img src="">` (empty src). [browser, verify]
The media-gallery build contains one `<img src="">` in a bare `<div>` (naturalWidth 0), likely a
pre-created lightbox/scaffold image populated on open. An empty `src=""` is a known anti-pattern
(older engines re-request the page URL). **Fix (if confirmed):** create the scaffold `<img>`
without a `src` attribute, or use a 1×1 data-URI placeholder. **low · S · verify.**

### Confirmed-good (credit — reproduced clean)
Light/dark/**sepia** all render correctly (sepia = warm `#f4ecd8`, my earlier "sepia broke" was
a hash-nav artifact); **zero JS console errors** on any page/book/deck; math (90 KaTeX spans),
code, tables — **no horizontal overflow** at any width tested; deck **aspect-based mode switch**
works (portrait→feed, 16:9→stepped); tabset roving-tabindex correct; book chapter/section
numbering + "Referenced by" backlinks + prev/next + theorem boxes + mermaid all clean; callouts
(5 kinds, bundled icons, theme accents), panels, gallery grid, scrollytelling all solid.

---

## II. Semantic HTML & accessibility (emitted markup) [agent; ★ = orchestrator-verified]

- **PA-M1 ★** Visible dates are `<span>`, never `<time datetime>` — zero `<time>` emitted in the
  crate (`render/mod.rs:1015`, `site/mod.rs:1350`). Wrap in `<time datetime="{iso}">`. **med · S.**
- **PA-M2** Blog posts aren't wrapped in `<article>` though `is_article` is already computed
  (`page.rs:508`, `render/mod.rs:873`). Emit `<main><article>` when `is_article`. **low-med · M.**
- **PA-M3** Listing grid is a `<div>` of cards, not a `<ul>`/`role=list` (`site/mod.rs:1285`) —
  AT never announces "list, N items." Contrast the book chapter list + TOC, which are correct
  `<ul>`. **low-med · S.**
- **PA-M4** The whole card is one giant `<a>` (`site/mod.rs:1395`): its accessible name is
  draft-badge + date + title + description + every category, and the `<h3>` conveys nothing
  distinct on heading-nav. Point the card link at the title via `aria-labelledby`. **med · M.**
- **PA-M5** Card category badges are click-interactive `<span>`s *inside* the card `<a>*
  (`site/mod.rs:1368`, wired in `10-category-filter.js:83`): keyboard/AT can't operate them
  (nested interactive-in-link). The `.tali-cat-filter` chip row is the real a11y path — drop the
  in-card handler, or make them real `<button>`s outside the `<a>`. **med · M.** *(= JS PA-B13.)*
- **PA-M6 ★** Table header cells carry no `scope` (`emit.rs:406`, no `scope=` anywhere). Emit
  `<th scope="col">` for pipe-table header rows. **low-med · S.**
- **PA-M7** Footnotes region has `role=doc-endnotes` but no accessible name (`render/mod.rs:863`)
  — add `aria-label="Footnotes"`. **low · S.**
- **PA-M8** Footnote refs aren't `role=doc-noteref` (`emit.rs:94`); the ref link's whole name is a
  bare number. **low · S.**
- **PA-M9** Slider `<output>` has no `for=` tying it to its range input (`extension/mod.rs:311`).
  **low · S.**
- **PA-M10** Deck on-screen progress bar is an unlabeled, unhidden `<div>` (`deck.js:1863`) —
  `aria-hidden="true"` (the live "slide N of M" already covers it). **low · S.**
- **PA-M11** Deck-embed "Open ↗" `target=_blank` link gives no programmatic new-tab cue
  (`extension/mod.rs:396`) — add visually-hidden "(opens in a new tab)". **low · S.**
- **PA-M12** Mermaid (bare `{mermaid}`, non-figure) ships no text-alternative hook
  (`emit.rs:52`) — support an optional caption/`aria-label`. **low · M · lower-confidence.**
- **PA-M13** Hero/card images silently default to `alt=""` with no lint nudge, unlike body
  `<img>` (`site/mod.rs:1327/1455` vs `diagnostics/a11y.rs:304`) — warn when `image:` set but
  `image-alt:` absent. **low · S.**

---

## III. CSS / theming / print [agent; ★ = orchestrator-verified]

**Structural gap (★ verified): `site.css` was never migrated to the PL11 geometry/motion tokens
— 0 uses of `--tali-radius-*` / `--tali-shadow-*` / `--tali-dur*` vs base.css's 31 / 8 / 8.**

- **PA-S1 ★** `site.css` radii are off-scale (`10/6/7px` intermediates that exist nowhere else,
  plus `4/8px` that map exactly to `sm`/`md`). Map to `--tali-radius-*`. **med · M.**
- **PA-S2 ★** `site.css` transitions are all raw literals (`.12s`×14, `.15s`×6) — use
  `var(--tali-dur)`. **med · M.**
- **PA-S3** An unofficial third duration `.15s` (~12 uses across base+site) isn't in the 2-value
  motion scale — fold to `--tali-dur` or add `--tali-dur-med`. **low-med · S.**
- **PA-S4** Card-hover shadow `0 6px 20px` (`site.css:131`) is off the `--tali-shadow-md`
  geometry (`0 4px 18px`). **low · S.**
- **PA-C1** **Cite-this "Copied!" fails AA in dark** (`site.css:379`): uses `--tali-accent` behind
  white (≈2.3:1); every other confirmed/active control uses `--tali-accent-fill` (5.59:1). Wrong
  token on the one control a dark reader sees on success. **high · S.**
- **PA-C2** Deck speaker "Read mode" active button (`deck.css:409`) — same wrong-token bug + a
  stray `#3b6ea5` fallback; deck.css uses `--tali-accent-fill` 0 times. **med · S.**
- **PA-C3** Deck share "Copy" button is a standalone `#4b57b0` (`deck.css:721`), outside the owned
  palette — use `--tali-accent-fill`. **med · S.**
- **PA-C4** Sepia has no `<mark>` search-highlight branch (`base.css:101`); light + dark do, so a
  sepia reader on a no-Highlight-API engine gets un-tuned amber over warm paper. **med · S.**
- **PA-C5** Deck per-slide-bg accent hexes hardcoded (`deck.css:254/255/266/267`) + theme.rs
  pre-paint `BG` map (`theme.rs:103`) duplicate token values with **no drift-lock** (card.rs has
  one for the identical Rust-can't-read-CSS case). Add drift-lock tests. **med · S/M.**
- **PA-D1** Deck controls with **no `:focus-visible` ring** (`deck.css:933` rings only 3 classes;
  decks don't load base.css): theme segment, share copy/close/url, speaker buttons — and
  `deck.js:2102` calls `.focus()` onto a ringless control. **med-high · S.**
- **PA-F1** Mobile burger-menu dropdown shadow is hardcoded black `rgba(0,0,0,.12)`
  (`site.css:193`) — invisible on dark (where elevation is a light glow). Use `--tali-shadow-md`.
  **med · S.**
- **PA-F2** Three scrim alphas for one "dim behind overlay" (`.42`/`.38`/`.55` at `base.css:754`,
  `site.css:267`, `deck.css:698`) — add a `--tali-scrim` token. **low · S.**
- **PA-F3** Keyboard focus on a listing card misses the hover lift/border/title-tint
  (`site.css:130`) — add `.tali-card:focus-visible`. **low-med · S.**
- **PA-F4** Mixed px/rem breakpoints (`640px` at `base.css:274` + `site.css:177` among rem
  breakpoints) — diverge under text-zoom; express as `40rem`. **low · S.**
- **PA-P1** Printed links lose their destination — no `a[href]::after{content:attr(href)}`
  (`base.css:831`). Standard editorial-print polish for a scholarly tool. **med · S.**
- **PA-P2** `pre` scroll-shadow gradient isn't reset for print (`base.css:850` resets table +
  math but not `pre`'s 4-layer bg) — faint edge-vignettes on every printed code block. **low · S.**
- **PA-CSS-note** Deck code-block copy button has no hover-reveal (**★ confirmed visually**:
  permanently visible on every code slide, unlike the page's `opacity:0` reveal). Copy-on-a-
  presented-slide is also odd — hide it on decks or hover-reveal. **low-med · S.**

---

## IV. Front-end behavior / client JS [agent]

- **PA-A2** Lightbox **gallery step is silent to AT** (`11-lightbox.js:81`): the dialog has a
  static `aria-label`, but stepping ←/→ updates the image/caption/counter with no `aria-live`,
  while every sibling enhancer (anchor-links, category-filter, focus-mode, deck) has one.
  `aria-live="polite"` on `.tali-lb-cap`. **med · S.**
- **PA-B3** Mobile TOC sheet is a dimming modal but neither traps focus nor `inert`s the page
  (`client.js:893`, `toc-sheet.js:46`); the lightbox + Cmd-K both use `taliFocusTrap`. Reuse it.
  **low-med · M.**
- **PA-B4** Deck share dialog is `role=dialog` without `aria-modal="true"` (`deck.js:2062`) — the
  one modal missing it. **low · S.**
- **PA-B5** "Cite this" tabs lack roving `tabindex` (`cite_this.rs:332`, `17-cite-box.js:33`) —
  every format tab is its own Tab stop, unlike the correct `tabset.js` pattern. **low-med · S.**
- **PA-B6** Preview client's 3 programmatic smooth-scrolls ignore `prefers-reduced-motion`
  (`client.js:154/701/1576`); `search.js` + reading-progress already gate on it. Reuse a
  `scrollBehavior()` helper. **low-med · S** (preview is used all day).
- **PA-B7** Lightbox/link-preview open transitions not gated on reduced-motion
  (`11-lightbox.js:16`, `12-link-preview.js:16`). **low · S.**
- **PA-B8** Scrolly & walkthrough `window` scroll/resize listeners only detach on the *next*
  scroll after the container is swapped by a live diff (`scrolly.js:53`, `walkthrough.js:85`):
  editing a doc with these accumulates orphaned listeners (each retaining a detached container)
  until the reader scrolls. `qmd-js.js` gets a real `teardown` hook; these don't. **low-med · M.**
- **PA-B9** Static-build mobile TOC handle picks up the sr-only " (read)" suffix
  (`toc-sheet.js:137` reads `active.textContent`, contaminated by `toc-spy.js:42`) → visible
  handle reads "Conclusion (read)". toc-spy solves this for its own chip; flash() doesn't.
  **med · S** (visible text bug in builds).
- **PA-B10** Lightbox-enlarged video has no `controls` and no keyboard operation
  (`11-lightbox.js:50/128`) — reader can't pause/scrub the zoomed clip, only Esc. Add `controls`
  to the *zoomed* video (inline stays chrome-less). **low-med · S** (nuance of tracked B7).
- **PA-B11** A landing Python `define` re-runs cells diagnosed as cyclic (`qmd-js.js:167`), unlike
  the initial run + input-driven scheduler which exclude cyclic cells — overwrites the "cycle"
  diagnostic. Filter with `r.graph.cyclic`. **low · S.**
- **PA-B12** Overflowing link-preview card (`>50vh`) can only be scrolled after a **mouse** pin
  (`12-link-preview.js:13/226`): a keyboard user can't pin/enter it (focusout hides it). Graceful
  fallback exists (Enter navigates), so it's a gap not a dead end. **low · M.**
- **PA-B13** = PA-M5 (card category tags mouse-only; chips are the keyboard path). **low · S.**
- **PA-B14** Cmd-K result list has no Home/End (`search.js:544`) though the cite tabs + deck both
  implement it — and it's the longest list in the app. **low · S.**
- **PA-B15** Reader menu + deck control menu stay open when focus Tabs out
  (`13-reader-menu.js:68`, `deck.js:1982`): light-dismiss popovers with no `focusout`-closes.
  **low · S.**
- **PA-B16** Resume-reading pill appears with no `role=status`/live cue (`15-reading-progress.js:74`)
  and auto-removes after 8s — non-visual readers never learn it exists. **low · S.**

---

## V. CLI / help / diagnostics wording [agent; ★ = orchestrator-verified]

- **PA-CLI1** `preview --port <N>` is parsed + tested but absent from `preview --help` and
  top-level usage (`cli.rs:868/897`, `main.rs:248`) — the cure for the positional-port papercut is
  undiscoverable. Document it. **med · S.**
- **PA-CLI2** `read --run` / `read --format|--json` undocumented in `read --help` (`query.rs:84`,
  `main.rs:360`) — the agent-facing JSON mode is the *least* discoverable. Add a Flags block.
  **med · S.**
- **PA-CLI3** Six subcommands still hand-write their `usage:` line instead of routing through the
  PL15 `command_synopsis()` single source (`check.rs:686` already drops `--errors-only`/
  `--stdin`/`--explain`; also publish/preview/read/map/symbols). Route all six through it.
  **med · S.**
- **PA-B1 ★** Kernel-unavailable message tells **headless** `build`/`read --run`/CI to "click
  Restart kernel" (`exec.rs:333`, surfaced at `build.rs:656`, `query.rs:212`) — a live-preview
  dev-menu action that doesn't exist there. Drop the "click" clause from the shared message (keep
  env-var + `taliesin doctor`). **med · S.**
- **PA-B2** `check` human diagnostics are uncolored (`check.rs:451`) while `doctor` + the dev log
  colorize behind the same `NO_COLOR`/`is_terminal` gate; rustc/cargo/tsc colorize severity. Paint
  the severity word (TTY-gated, so the greppable non-TTY contract is unchanged). **med · S.**
- **PA-CLI4** `publish` prints parse errors as raw `error: …` via bare `eprintln!`
  (`publish.rs:67/212`) while every other command (and publish's own runtime errors) route the
  same shared helpers through styled `log::error`. Route publish's parser through `log::error`.
  **med · S.**
- **PA-CLI5** Diagnostic-wording inconsistencies to unify: broken in-page link leaves `#frag`
  un-backticked while sibling "not found" messages backtick their subject (`anchors.rs:63` vs
  `assets.rs:50`/`links.rs:87`); `unknown command:` carries a colon its siblings omit
  (`main.rs:104`); consequence-only messages (`bibliography.rs:25`, `headings.rs:36`) give no
  inline action while peers do — pick one register. **low · S–M.**
- **PA-CLI6** `doctor` summary mixes "python" (lowercase) with "R" in one prose sentence
  (`doctor.rs:191`); `TALIESIN_NO_CLEAR` is the one ENV entry with no gloss (`main.rs:144`).
  **low · S.**

---

## Cross-cutting patterns (the shape of the findings)

1. **The design system stopped at `base.css`.** Tokens (color + PL11 geometry/motion) are rich in
   base.css but `site.css` (0 uses) and `deck.css` (0 `--tali-accent-fill`) are still hand-literal,
   so the *chrome* around the well-tokenized *content* is a second, drifting design language. This
   is the single biggest "feels unfinished" lever: PA-S1/S2/C1/C2/C3/D1/F1.
2. **`<head>` + page-scaffold completeness is page-only.** Decks (PA-H1) and listing pages (PA-H2)
   skip scaffolding pages get for free (favicon, theme-color, `<h1>`, `<article>`, list semantics).
3. **Screen-reader announcement + focus-containment is 90% consistent, with named holes.** One
   `aria-live`/focus-trap/roving-tabindex/`aria-modal` is missing on exactly one surface each
   (PA-A2/B3/B4/B5) while all siblings have it — the same "silent hole in a diligent surface"
   pattern the prior audit named.
4. **`prefers-reduced-motion` is honored in the reader enhancers but not the preview client**
   (PA-B6/B7) — the daily-driver surface.
5. **CLI help/usage drift persists** where PL15 didn't reach (PA-CLI1/2/3).

## Design questions (owner ruling first — not build-ready)

- **Book chapter sidebar is a hamburger even at 1366px** with ~400px of free margin each side — a
  persistent left sidebar would aid book navigation; is the always-collapsed choice deliberate
  reading-first, or worth a wide-viewport persistent rail? (2026-07-06 "keep both nav surfaces"
  may already settle this.)
- **Copy button on deck code slides** — useful, or noise on a presented slide? (PA-CSS-note.)
- **Card whole-`<a>` vs title-`<a>`** (PA-M4) is a UX call: giant click target vs clean
  heading-nav / link name.

## Suggested grind order

1. **Design-system single-source pass (one CSS/token PR):** PA-S1, S2, C1, C2, C3, F1, F3, D1,
   C4 — closes cross-cutting pattern #1, all small, high "feels finished" payoff.
2. **Scaffold-completeness pass:** PA-H1 (deck head), PA-H2 (listing `<h1>` + heading levels),
   PA-M1/M2/M3 (time/article/list) — pattern #2.
3. **A11y announcement/focus holes (one JS PR):** PA-A2, B3, B4, B5, B14, B15, PA-M6/M7/M8.
4. **CLI/diagnostics sweep:** PA-B1, B2, CLI1–6.
5. **Reduced-motion + print:** PA-B6, B7, P1, P2. Fold the rest opportunistically.

Each fits the standard branch → spec → corpus-pin (where behavioral) → browser-verify loop.
