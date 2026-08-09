# UI-audit findings (2026-07-09)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

Raw output from the `tools/ui-audit` harness over the full corpus (89 pages / 534
cells, 6-cell matrix at scale 0.5). Pipeline: capture (Puppeteer, free) -> probe
(free) -> analyze / dedup / verify / report workflow (148 agents, Sonnet, ~9M
tokens). The full `--parallel` capture deadlocked near the end and the manifest was
salvaged from on-disk metas, so 15 cells on 3 heavy pages are unaudited (section 7).

## Triage status (2026-07-09, hand-verified against source + browser)

**FIXED + browser-verified:** #1, #3, #4, #5, #6, #8, #10 (the engine/CSS half) and
**#2** (the stale-`_freeze` figure bug). Commits `d70da77` + `d0b1ffa`.

**Two diagnoses below are WRONG as written; the text is kept verbatim as the raw
audit output. Read these corrections first:**

- **#2's stated root cause ("the freeze cache key has no dependency on renderer /
  output-format version") is false.** `freeze.rs` already carries a `FORMAT_VERSION`
  whose doc comment says to bump it exactly when "the *bundled output format* of a
  cached cell changes". It had simply never been bumped since introduction: the
  rename commit `8bb0a65` edited `freeze.rs` without touching the constant. The fix
  was a one-line bump to 3, which discards every pre-v3 entry and self-heals on the
  next build. The blast radius was also 3 live pages (KL-divergence,
  fourier-transform, pca-geometry), not 1, and two of them mixed both class
  generations inside a single cached page.
- **#8's stated root cause ("has `overflow-x: auto` but never got the edge-shadow
  affordance") understates it.** `.katex-display` never scrolled at all (`scrollLeft`
  pinned to 0), because KaTeX makes the inner `.katex` a full-width block whose
  overflow never grows the scroll area. The equation was clipped and *unreachable*,
  not merely un-signposted; a scroll shadow alone would have been inert.
- Minor: **#5's suggested selector** (`td :not(pre) > code`) cannot match `<td><code>`
  (it requires an intermediate element) and is a silent no-op. Shipped as `th code, td code`.

**Re-classified, NOT fixed:** #7 + #12 are **design choices** that contradict the stated
intent at `base.css:365-367`, not defects (owner ruling pending; see `backlog.md`).

**FIXED 2026-07-09 (content batch):** all four of #9, #11, #13 and #14 were content fixes in
`.tmd` sources, not engine defects. Each was re-diagnosed against current source (not the
audit's screenshots) and browser-verified with the pre-fix state re-injected as a
counterfactual:

- **#9** confirmed by parsing both label forms with the page's own bundled mermaid
  (11.4.1): the unquoted `(` is lexed as the shape-open token `PS` and throws
  `Parse error on line 2`; the quoted form parses and still renders `<br/>` as a real line
  break (`htmlLabels` is on, since `taliMermaidConfig` never overrides it).
- **#13** confirmed *mechanically*, so the stale-`_freeze` confound (#2) is irrelevant:
  `kernel.rs` `_qmd_recolour` recolours only `Text`, spines, ticklines and gridlines, makes
  axes backgrounds transparent, and leaves **data artists untouched** ("Data colours are
  untouched", `kernel.rs:126-142`). An explicit `color="white"` on a `Line2D` therefore
  survives into the light variant. Measured in the built PNG: 1236 opaque pure-white pixels
  in the bottom panel, composited onto `--tali-bg: #ffffff` at **contrast 1.00:1**. The
  "Sum of all three notes (what you actually hear)" panel therefore rendered *completely empty*
  on the light theme. Fixed with `#8b5cf6` (4.23:1 light / 4.19:1 dark / 3.60:1 sepia; the
  theoretical optimum for a colour that must clear 3:1 on both grounds is L≈0.2022, and
  `#8b5cf6` measures L=0.198). The HTML legend swatch (`color:white`, "The chord") had the
  same bug and was changed to match.
- **#14** confirmed: `base.css:374` (`canvas, svg, video, iframe { max-width: 100% }`) clamps
  the box only. With `width=520 height=630` and no `viewBox`, user units stay 1:1 px, so the
  drawing is clipped rather than scaled. (`base.css:368` already pairs `max-width` with
  `height:auto` for mermaid, the correct pattern.) Verified at 390/900/1440: aspect ratio
  preserved, no parent or document overflow.
- **#11** confirmed and **worse than triaged**, not merely "plausible". `Plot.tickX` given no
  `y` channel spans the **full frame height**, so the "rug" is really full-height rules that
  cross the μ/σ labels, which are filled with the same per-component colours. Dark is
  genuinely illegible. Fixed with a `var(--tali-bg)` halo (`paint-order: stroke`), which
  leaves `fill`, the colour encoding, untouched, and stays correct across a live theme flip.

**NEW, found by a collateral sweep (not in the original audit): `pca-geometry` had the exact
same bug as #13.** `ax2.plot(cumulative_var * 100, color="white", …)` made the scree plot's
cumulative-variance line invisible on light, leaving a labelled right-hand axis
("Cumulative variance (%)") measuring a series that was never drawn. The audit missed it
because pca-geometry was one of the three pages serving stale `_freeze` output (#2); fixing
the cache is what exposed it. Fixed with the same `#8b5cf6`, plus `alpha` 0.7 → 0.9 (at 0.7
even violet only reaches 2.64:1, below the 3:1 non-text threshold).

Cleared by the same sweep, **deliberately not changed** (each composited onto the real light
background and inspected): `Kruskal-Wallis` `axvline(color="white")` + `hist(edgecolor=…)`,
`em-algorithm` `hist(edgecolor="white")`, `Kruskal-Wallis` `Plot.dot(stroke:"white")`. The
median dashes still read where they cross a coloured bar, and white bar-edges act as
separators on white. `pca-geometry`'s three remaining `color="white"` sites are `Text`
artists, which the engine recolours. `KL-divergence`'s white pixels are a legend frame.

---

# Taliesin UI Audit Bug Report

## 1. Summary

| Metric | Count |
|---|---|
| Pages audited | 89 (534 cells) |
| Raw findings (pre-verification) | 43 |
| Confirmed visual/layout bugs | 14 groups (34 affected page instances) |
| Console/network error groups | 9 (7 console, 2 network) |
| Build failures | 0 |
| Interaction probes | 6/6 passed |

## 2. Confirmed visual/layout bugs

Ranked most severe first (4 high, 10 medium).

### 1. Mobile/portrait "on this page" TOC pull-up sheet overlaps or hides body content
**Severity:** high | **Affected pages:** 13 instances
**Examples:** `bayesian-website` `/index.html` @ mobile+portrait, light/dark; `docs-guide` `/using/preview.html` @ portrait, light/dark; `docs-guide` `/using/recipes.html` @ mobile+portrait, light/dark; `docs-guide` `/using/writing.html` @ mobile+portrait, light/dark; `docs-guide` `/reference/configuration.html` @ portrait (milder dead-gap variant)
**Root cause:** `site.css` sets an unconditional `top` on `#TOC` (`body.tali-site #TOC`, `body.tali-book-body #TOC`) with the same specificity as `base.css`'s mobile bottom-sheet rule (`@media max-width:60rem`, `body.tali-toc-sheet #TOC { position:fixed; inset:auto 0 0 0 }`), and comes later in source order, so it wins. This breaks the `translateY(100%)` bottom-anchor math: the closed sheet leaves ~140 to 313px poking into the viewport over content, and the open sheet falls short of the viewport bottom, leaving a gap of dimmed content beneath it.
**Source:** `crates/core/assets/css/site.css` (`body.tali-site #TOC` ~938, `body.tali-book-body #TOC` ~181) vs `crates/core/assets/css/base.css` (~657-779, `.tali-toc-sheet #TOC`, `@media max-width:60rem`)
**Verifier reasoning:** Reproduced live (non-stitched, real viewport) headless-Chrome screenshots plus `getComputedStyle`/`getBoundingClientRect` at real 390x844 and 900x1440 against the actual served build; `top:64px` instead of `auto` confirmed on multiple pages. One sub-claim (a JS toggle-timing "flash open" bug) was checked and did not hold; the CSS cascade cause is the real, independently reproduced defect.

### 2. Dual-theme matplotlib figure renders both light and dark variants stacked instead of one
**Severity:** high | **Affected pages:** 2 instances
**Examples:** `tech-blog` `/posts/KL-divergence/index.html` @ laptop/mobile/portrait, dark (illegible top copy); same page @ light (washed-out duplicate)
**Root cause:** A stale `_freeze` cache entry predates the `qmd-*` to `tali-*` CSS class rename and still emits `qmd-fig-light`/`qmd-fig-dark` markup; current CSS only toggles visibility for `tali-fig-light`/`tali-fig-dark`, so neither old-class variant is hidden and both render stacked. The freeze cache key (a cumulative content hash) has no dependency on renderer/output-format version, so a renderer-side rename silently orphans matching cached HTML.
**Source:** `corpus/tech-blog/_freeze/posts/KL-divergence/index.json` (stale, predates rename commit `8bb0a65`); `crates/core/assets/css/base.css:574-577` (current toggle rules); `crates/server/src/kernel.rs:211-212` (current emission); `crates/server/src/freeze.rs` (cache-key design)
**Verifier reasoning:** Confirmed the built HTML backing the screenshot contains the old `qmd-fig-*` classes while current source emits `tali-fig-*`; cache file mtime predates the rename commit; the tech-blog deploy script does not clear `_freeze` before building, so this would ship live as-is.

### 3. Deck slide headings with "flip-to-light" styling are invisible (white-on-white) in light-theme reader view
**Severity:** high | **Affected pages:** 2 instances
**Examples:** `docs-guide` `/demo.html` @ laptop/mobile/portrait, light ("A slide with a backdrop", "Gradient" headings); `site` `/demo.html` @ light (same headings)
**Root cause:** `deck.css`'s `html.tali-scroll` "un-flip" override restores color for `section`, `strong`, `a`, `.subtitle`, `li::marker` but omits `h1`-`h4`, so headings on a colored-backdrop slide stay hard-coded white (from the base `.tali-dark-bg` flip rule) even though the reader/scroll view never paints a dark backdrop.
**Source:** `crates/core/assets/css/deck.css:579-595` (missing h1-h4 in un-flip override), `:364-369` (base flip rule); content at `docs/guide/demo.tmd:70,75`
**Verifier reasoning:** Pixel inspection of the light-theme screenshot found a heading-sized run of pure white pixels where the dark-theme capture shows readable text at the identical position; the CSS selector list omission was confirmed by direct source read.

### 4. "Back to Blog" link misplaced to the top of the article at portrait (~900px) width
**Severity:** high | **Affected pages:** 1 instance
**Example:** `tech-blog` `/posts/evidence-lower-bound/index.html` @ portrait, light/dark
**Root cause:** Two breakpoints disagree. `site.css`'s `.tali-site-main.has-toc` grid only collapses to one column at `max-width:640px`, but `toc-sheet.js`/`base.css` switch `#TOC` to `position:fixed` (removing it from grid flow) at `max-width:60rem` (960px). In the 641 to 960px gap the grid is still two columns but `#TOC` is out of flow, so CSS Grid auto-placement puts the next sibling (`.tali-listing-backnav`) into the now-vacant top-right cell instead of below the article.
**Source:** `crates/core/assets/css/site.css:170` (640px grid collapse) vs `base.css:659-679` + `web-client/toc-sheet.js:29-32` (960px sheet breakpoint)
**Verifier reasoning:** Reproduced on a fresh build at a real 900x1440 viewport; computed styles confirmed `tali-toc-sheet` active, grid still two columns (604px/224px), `#TOC` `position:fixed`, and `.tali-listing-backnav`'s bounding rect sitting at the top next to the title.

### 5. `TALIESIN_*` identifiers in tables break mid-word instead of at underscores
**Severity:** medium | **Affected pages:** 3 instances
**Examples:** `docs-guide` `/reference/cli.html` @ all viewports, both themes; `docs-guide` `/using/getting-started.html` @ all viewports, both themes; `docs-internals` `/block-model.html` @ all viewports, both themes
**Root cause:** `:not(pre) > code { overflow-wrap: anywhere; }` applies unconditionally inside table cells too; combined with auto table-layout (`table { display:block; width:max-content }`), the browser's min-content width calculation treats every character as a break point (underscores get no special line-breaking treatment), so narrow columns squeeze identifiers and split at arbitrary characters.
**Source:** `crates/core/assets/css/base.css:347-348`, `:374`
**Verifier reasoning:** Reproduced live on the running preview; a counterfactual CSS override (`overflow-wrap:normal`) fixed the wrapping with no change in table `scrollWidth`, confirming ample width budget already existed and the mid-word split was not from insufficient space.

### 6. Fixed top-right settings/gear icon collides with full-width mobile page titles
**Severity:** medium | **Affected pages:** 2 instances
**Examples:** `diagnostics__check-superset` `/index.html` @ mobile, light/dark; `refs__theorems-shared` `/index.html` @ mobile, light/dark
**Root cause:** `.tali-rmenu-toggle` is `position:fixed`, removed from flow, and reserves no space; no padding/max-width is applied to the title block for clearance, and the mobile media query doesn't account for the icon's footprint. When a title's line runs into the icon's x-band at 390px, the opaque icon occludes trailing glyphs.
**Source:** `crates/core/assets/css/base.css:48-53` (`.tali-rmenu-toggle`), title-block rules ~303-314, 715-719
**Verifier reasoning:** Re-ran the harness's own build+capture pipeline and measured DOM rects directly: gear button (x=335.6-374, y=16-54.4) overlaps the h1 title (x=13.6-376.4, y=32-63.28) in both themes; confirmed layout-boundary-dependent (a second, unrelated title happened not to collide).

### 7. Horizontal multi-box flow/mermaid diagrams scale down to illegible text on mobile instead of reflowing
**Severity:** medium | **Affected pages:** 2 instances
**Examples:** `docs-internals` `/execution.html` @ mobile, light/dark (Fig 1 three-zone diagram, forkserver diagram); `docs-internals` `/server.html` @ mobile, light/dark (11.1 save/preview loop diagram)
**Root cause:** `pre.mermaid svg { max-width:100%; height:auto; }` is the only sizing rule at any width. Mermaid emits viewBox-scaled SVGs with `foreignObject` text labels, so shrinking the box shrinks label text proportionally like a raster image, with no breakpoint or minimum-scale guard.
**Source:** `crates/core/assets/css/base.css:361-362`
**Verifier reasoning:** Headless-Chrome measurement found effective label text as small as 5.8px (scale 0.36) at 390px versus 16px for a diagram that fits unscaled; also degraded at 1440px (~11.4px) when the intrinsic viewBox exceeds the column, so it's not phone-only. Contrasted with `table`'s deliberate scroll-not-shrink treatment in the same file.

### 8. Wide KaTeX display equations clip at the right edge on mobile with no scroll affordance
**Severity:** medium | **Affected pages:** 2 instances
**Examples:** `posts__em-algorithm__index` `/index.html` @ mobile, light/dark; `tech-blog` `/posts/em-algorithm/index.html` @ mobile, light/dark
**Root cause:** `.katex-display` has `overflow-x:auto` but, unlike `pre` and `table` in the same file, never got the edge-shadow/gradient-mask affordance signaling scrollable content, so a clipped equation reads as missing content rather than scrollable.
**Source:** `crates/core/assets/css/base.css:626` (contrast with `pre` at 316-330, `table` at 369-383)
**Verifier reasoning:** Git blame showed the `overflow-x` rule was added to fix page-level overflow but never got the scroll-shadow treatment shipped for `pre`/`table`; DOM flags confirmed KaTeX spans reaching past the viewport edge with no page-level overflow (contained but visually truncated).

### 9. Mermaid diagram fails to render, shows library error bomb icon
**Severity:** medium | **Affected pages:** 1 instance
**Example:** `docs-internals` `/architecture.html` @ all viewports, both themes (Figure 4)
**Root cause:** Invalid mermaid syntax in the doc source, not a rendering-pipeline defect: an unquoted pipe-label edge (`|output blocks<br/>(data flow, not a call)|`) has a literal `(` right after `<br/>`, which mermaid's unquoted-label lexer rejects. Taliesin passes mermaid source through verbatim by design (no server-side validation), so the client library's own error placeholder renders as intended.
**Source:** `docs/internals/architecture.tmd:196` (content typo); pipeline `crates/core/src/render/mod.rs` (~502-510), `figure.rs::emit_mermaid_figure` (~110-126)
**Verifier reasoning:** Fed the exact stripped diagram text to an independent mermaid validator; it failed identically at the same `(` token, matching the "Syntax error, mermaid version 11.4.1" bomb icon in the screenshot. Verifier downgraded severity from the candidate's "high" to medium: one figure on one page, trivial one-line content fix, not an engine defect.

### 10. "Chapters" hamburger icon collapses to an illegible dot at mobile width
**Severity:** medium | **Affected pages:** 1 instance
**Example:** `docs-internals` `/sites.html` @ mobile, light/dark
**Root cause:** Not font-size-relative sizing as originally guessed. `.tali-book-topbar-inner` is a flex row that outgrows the 390px viewport; the drawer button's `<span>Chapters</span>` has a min-content floor it can't shrink below, but the `<svg>` icon has no `flex-shrink:0`/min-width floor, so all shrink pressure lands on the icon, crushing it from 16x16 to roughly 3.9x16.
**Source:** `crates/core/src/site/chrome.rs:236-241` (button+SVG markup); `crates/core/assets/css/site.css:186-198`
**Verifier reasoning:** Live `getComputedStyle` at real 1x/2x device-pixel-ratios (not just the harness's 0.5x capture scale) confirmed the SVG shrinks to ~3.9x16px at 390px versus a clean 16x16px at widths >=500px, ruling out a capture artifact.

### 11. Interactive EM-visualizer chart annotation labels are low-contrast against underlying rug marks
**Severity:** medium | **Affected pages:** 1 instance
**Example:** `posts__em-algorithm__index` `/index.html` @ all viewports, both themes (worst in dark)
**Root cause:** `Plot.text()` annotation calls hardcode label position to each Gaussian component's own mean (its densest tick region) with fill color matching that component's rug-tick color scale, with no offset, halo, or contrast check.
**Source:** `corpus/posts/em-algorithm/index.tmd:336-368`
**Verifier reasoning:** Confirmed via screenshot crops (label visually fuses with the same-hued dense tick band, worst in dark theme) and direct source read showing no contrast-aware placement logic exists.

### 12. Embedded demo video scales down to illegible text at mobile width
**Severity:** medium | **Affected pages:** 1 instance
**Example:** `site` `/features.html` @ mobile, light/dark
**Root cause:** `.tali-video video` and the generic `video{max-width:100%}` rule scale the video purely proportionally with no minimum legible scale or mobile-specific source; the source clip is a 1100x720 desktop screen recording with text baked in at desktop scale, downscaled roughly 3x at mobile content width.
**Source:** `crates/core/assets/css/base.css:246`, `:368`; `crates/core/src/render/extension/mod.rs::video_html()` (~302-333); content `site/features.tmd:14`
**Verifier reasoning:** Visually confirmed via cropped mobile screenshots (baked-in text reduced to unreadable blur while surrounding prose stays legible); corroborated by the project's own `notes/backlog.md`, which already flags "mobile embed refine" as an outstanding TODO for this embed.

### 13. Matplotlib chart trace hardcoded to white is invisible in the auto-generated light-theme variant
**Severity:** medium | **Affected pages:** 1 instance
**Example:** `tech-blog` `/posts/fourier-transform/index.html` @ laptop/mobile, light (4th panel, "Sum of all three notes")
**Root cause:** The dual-theme matplotlib recoloring preamble deliberately never touches `Line2D` plot-artist colors (data colors preserved across themes by design), but the cell hardcodes `axes[3].plot(..., color="white")`; with the axes background set transparent, that line is invisible against the light page background.
**Source:** `corpus/tech-blog/posts/fourier-transform/index.tmd:120` (content); `crates/server/src/kernel.rs:69-245` (`_qmd_recolour`); `crates/core/assets/css/base.css:574-577`
**Verifier reasoning:** Extracted the raw dual-theme PNGs and composited the light variant onto the real light page background, confirming the trace is genuinely blank; the dark variant composited onto the dark background shows the line clearly; other panels with non-white hardcoded colors remain visible in both themes.

### 14. Interactive "wound around a circle" plot is off-center at mobile width
**Severity:** medium | **Affected pages:** 1 instance
**Example:** `tech-blog` `/posts/fourier-transform/index.html` @ mobile, light/dark
**Root cause:** Not the originally suspected JS measurement race (the layout width is a static constant, no race exists). The `{js}` cell creates the SVG with fixed width/height and no `viewBox`; the site-wide `canvas,svg,video,iframe {max-width:100%}` rule clamps only the on-screen width with no paired `height:auto` (unlike the adjacent mermaid SVG rule), so the browser can't rescale internal absolute-pixel coordinates, and content positioned at a fixed-space coordinate renders right of the box's true visual center.
**Source:** `corpus/tech-blog/posts/fourier-transform/index.tmd:417-418` (SVG width/height without viewBox); `crates/core/assets/css/base.css:368`
**Verifier reasoning:** Measured live DOM at mobile (390px) versus laptop (1440px): mobile SVG box 358x630 with the winding-circle center 81px right of the true box center (~23% of box width) and spectrum bars beyond k~13 clipped off-screen; correctly centered and fully visible at laptop width.

## 3. Console / JS errors

Grouped by message, with occurrence count across captures.

| Message | Count | Unit(s) |
|---|---|---|
| `Failed to load resource: the server responded with a status of 404 (Not Found)` | 23 | `diagnostics__a11y` (`/index.html`, all viewports/themes) |
| `qmd-js: dependency cycle involving ping` | 6 | `diagnostics__links` (`/index.html`, all viewports/themes) |
| `qmd-js: dependency cycle involving pong` | 6 | `diagnostics__links` (`/index.html`, all viewports/themes) |
| `qmd-js cell error: ReferenceError: undefined_name is not defined` | 6 | `diagnostics__links` (`/index.html`, all viewports/themes) |
| `Object` (console dump) | 6 | `docs-internals` `/architecture.html` (all viewports/themes) |
| `qmd-js cell error: Error: intentional: pins the .tali-js-error box for light + dark themes` | 6 | `reactive__js-error` (`/index.html`, all viewports/themes) |
| `Failed to load resource: net::ERR_NAME_NOT_RESOLVED` | 3 | `render-fixes__index` (`/index.html`, laptop/light + mobile/dark + portrait/dark) |

Note: most of these units (`diagnostics__a11y`, `diagnostics__links`, `reactive__js-error`, `render-fixes__index`) are corpus fixtures that deliberately exercise error/diagnostic UI (e.g. `reactive__js-error`'s own message says it "pins the .tali-js-error box"). Treat as expected demo output unless the diagnostics panel itself mishandles them; not re-verified as bugs in this pass. The `docs-internals` `/architecture.html` "Object" console dump likely correlates with confirmed bug #9 (mermaid parse-error logging).

## 4. Failed network requests

| Request | Count | Unit(s) |
|---|---|---|
| `404 http://127.0.0.1:37185/logo.png` | 23 | `diagnostics__a11y` (`/index.html`, all viewports/themes) |
| `net::ERR_NAME_NOT_RESOLVED https://media.example.com/clip.mp4?token=demo123` | 3 | `render-fixes__index` (`/index.html`, laptop/light + mobile/dark + portrait/dark) |

Both pair with the console groups above (missing-logo fixture, unresolvable placeholder media URL); neither was independently re-verified as an unintended bug in this pass.

## 5. Interaction probe results

| Feature | Result | Target |
|---|---|---|
| Deck navigation (ArrowRight advances slide) | Pass | `corpus/deck.tmd` |
| Lightbox (click opens, arrow navigates gallery) | Pass | `corpus/media/gallery.tmd` |
| Click-to-source (Alt-click emits `click_block` frame) | Pass | `corpus/media/gallery.tmd` |
| Search (query returns results) | Pass | `docs/internals` |
| TOC scrollspy (scroll marks active entry) | Pass | `docs/internals` |
| Hover-preview (xref hover opens populated card) | Pass | `corpus/demo-book` |

6/6 passed. No probed feature failed; no features were skipped ("not run").

## 6. Build failures

None. 0 build failures reported across the audit run.

## 7. Coverage gaps (unaudited cells)

This audit ran on a **salvaged** manifest: the full `--parallel 3` capture deadlocked
near the end (a hard-timeout that abandons a wedged cell leaks its Chrome tab; enough
leaks starve the event loop and stall the run), and the manifest was reconstructed from
the per-cell metas already on disk. 519 of 534 cells have screenshots; **15 shots are
missing on 3 heavy-JS/canvas pages** whose renderer never settled within the 60s
watchdog ("Target closed" mid-screenshot). A visual audit cannot see a screenshot that
does not exist, so these cells were **not** audited and are absent from the findings above:

| Page | Missing | Note |
|---|---|---|
| `tech-blog` `/posts/a-star/index.html` | 6/6 (all) | Completely unaudited; the page never settled in any viewport/theme. |
| `site` `/showcase.html` | 5/6 | Only mobile/light captured. |
| `reactive__graph` `/index.html` | 4/6 | Only mobile/light + portrait/dark captured. |

**This is itself a candidate finding, not just a data gap:** a page whose renderer
consistently fails to reach a settled state within 60s (heavy `{js}`/canvas/graph work
that never idles, or a runaway animation loop) is a real reader-facing risk worth
investigating directly. Re-capture these three in isolation (`node capture-run.mjs --only
'a-star' --only 'showcase' --only 'reactive__graph'`, single browser, no `--parallel`)
and, if they still never settle, treat the non-settling as the bug.

### RESOLVED 2026-07-09: the non-settling was the *harness*, not the pages

The hypothesis above is **refuted**. Driven in isolation, all three pages screenshot fine
(503 KB / 331 KB / 77 KB in 58-264 ms). None of them has a runaway loop:

| Page | Reality |
|---|---|
| `reactive__graph` | Zero loop primitives. Fully idle after its cells run. |
| `a-star` | Its `setTimeout(animTick)` chain is user-gated (`btnAnimate.onclick`), idle on load, torn down on `invalidation`. |
| `showcase` | A deliberate `requestAnimationFrame` WebGL orbit, gated on `IntersectionObserver` + `prefers-reduced-motion` and disposed via `invalidation`. |

The real cause was a **false negative in the harness's `settle()` predicate**. `jsOk`
required every `.tali-js-cell` to have a `.tali-js-out` with `childElementCount > 0`, but
`qmd-js.js` only fills that div `if (node instanceof Node)`. A `//| name:` value publisher
returns a Number, and an `//| input:` effect returns `undefined`, so those cells stay empty
forever and `jsOk` can never become true. Each of the three pages has exactly one such
cell, and each had *already run* (its `<script>` was stamped). So `settle()` always burned
its full 6 s timeout, then screenshotted anyway. The 15 missing shots were collateral of the
`--parallel` tab-leak deadlock, not of any per-page wedge.

The obvious fix, gate `jsOk` on `data-qmd-ran`, is **wrong**: that attribute is stamped at
*registration*, before `runSequentially` executes any cell body, so it would trade a false
negative for premature capture. `qmd-js.js` now stamps `data-qmd-done` in a `finally` when a
cell's `run()` resolves, and `jsOk` gates on that (falling back to the old child-count rule
for pages built by an older binary, and for cyclic cells, which are never run but do paint a
diagnostic). All three pages now report `settled: true` in 66-213 ms instead of timing out.

Re-captured and audited at 390/900/1440 × light/dark: **no reader-facing bugs**, no console
errors, no horizontal overflow. One capture artifact worth knowing: `showcase`'s 3D canvas is
absent from a no-scroll full-page shot at 390px, because its `IntersectionObserver` never
fires when the host is below the fold. A reader who scrolls does get it (verified: 358×460),
and emulating `prefers-reduced-motion: reduce` builds it synchronously, the cheapest way to
make the harness capture it.
