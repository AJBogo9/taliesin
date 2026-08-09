# Deck motion audit: the overview and every deck animation

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

2026-07-24. Triggered by three complaints from the author while testing the overview:

1. "When moving from one topic to another, there is the flash of moving from one row to another."
2. "I don't like the animation when zooming in and out of a slide."
3. "That animation too is a bit crowded."

Method: six independent review lenses over `deck.js` / `deck.css` / `render/deck.rs`, one
adversarial refuter per finding (77 raised, 56 survived, 21 refuted), plus direct
measurement in Chrome via the devtools MCP on `corpus/deck.tmd` (18 slides) and a
synthetic 106-slide deck, both at 1440x900.

Every number below is measured in the browser unless marked *derived*.

---

## Root cause, in one sentence

The deck has **one motion primitive** (a static CSS `transition` on one `transform`,
`deck.css:42`) doing **four jobs at four different scales**: a one-cell step, an N-cell
topic change, an arbitrary jump, and a 5.7x zoom that also carries a layout change. A
declarative transition cannot take distance as an input, so all three complaints follow
from that one fact.

---

## Complaint 1: the row-change flash

### Measured

Stepping from slide 15 into the topic (`corpus/deck.tmd`, h=14 -> the stack row):

| | measured |
|---|---|
| camera travel | 20,176 screen px = **14.0 screen-widths** |
| duration | 500 ms |
| mean velocity | 28 screen-widths / second |
| velocity in the first 20 ms | **~200 screen-widths / second** |
| slides swept through the stage | 14, at ~9 ms each (*derived*) |

On the 106-slide deck (5 `#` topics x 20 slides) the same step is **20 screen-widths**.

### Mechanism

`gridRows()` (`deck.js:111-128`) puts every run of consecutive top-level slides into
**one row**, and only re-wraps it `if (deck.overview)` (`pushRun`, `deck.js:103`). So in
present mode `corpus/deck.tmd` is a 15-column row 0 plus a 3-cell stack row 1.
`cameraTarget()` (`deck.js:186`) returns the destination cell centre and `setCamera()`
takes a **boolean**, not a distance, so a linear step of 1 becomes a geometric jump of
`(-13440, +540)` design px on a flat 500 ms clock.

Two aggravating factors:

- **The easing is front-loaded.** `cubic-bezier(.2,.8,.2,1)` has an initial slope of 4;
  peak/mean velocity is 4.06. Half the distance lands in the first ~65 ms.
- **The path crosses empty world.** cx and cy are components of one interpolated
  transform, so the camera flies a straight diagonal. Row 1 is only 3 cells wide, so
  world columns 3..14 of row 1 do not exist. Sections declare no background
  (`deck.css:58-67`), so the stage paints the bare html canvas. Roughly 79% of the stage
  is content-free for ~130 ms (*derived*), corroborated by a mid-flight capture.

So the artefact is not one thing: it is a strobe of 14 half-frame slides, followed by a
mostly empty stage, followed by arrival.

---

## Complaint 2: the overview zoom

### Measured

Tracking the current slide's own bounding rect while the overview opens from h=14:

| t | left edge | width |
|---|---|---|
| 9 ms | 0 (fills the stage) | 1440 |
| **70 ms** | **-3170** | 792 |
| 153 ms | -1416 | 420 |
| 321 ms | +404 | 253 |
| 565 ms | +733 (landed) | 228 |

**The slide being read is thrown 2.2 screen-widths off the left edge and comes back,
inside 500 ms.** The path is non-monotonic. The endpoints (0 -> 733) are unremarkable,
which is why this was never noticed as a bug.

### Mechanism

Three things compose:

1. **The zoom has no fixed point.** `fitOverview()` (`deck.js:758-766`) unconditionally
   sets `cx/cy` to the **grid centroid**, and `setOverview` calls it on every entry
   (`:868`). The camera changes scale *and* re-centres.
2. **The current slide's cell moves under it.** `pushRun`'s wrap is overview-gated, so
   the 15-run re-wraps to `ceil(sqrt(15)) = 4` columns and h=14 moves from (r0,c14) to
   (r3,c2): a world displacement of `(-11520, +1620)` px, running **counter** to the
   camera.
3. **Both are tweened independently and linearly.** `setOverview` deliberately arms
   `.tali-cam-anim` *before* `positionGrid()` (`deck.js:875-876`), and `deck.css:40-41`
   transitions every `> section`. The tile translate sits *inside* the camera scale, so
   the on-screen path is the **product** of two linear interpolations, which is
   quadratic. Hence the overshoot.

Secondary: CSS interpolates `scale` linearly while perceived zoom rate is `d(log s)/dt`,
so a 1.5 -> 0.264 ramp is inert at the wide end and violent at the narrow end, and enter
and exit are **not inverses**.

---

## Complaint 3: crowded

One class flip starts every clock in the file in the same frame. Measured with
`document.getAnimations()` 130 ms into the overview open:

| deck | concurrent animations |
|---|---|
| `corpus/deck.tmd` (18 slides) | **28** |
| synthetic 106-slide deck | **108** |

On `corpus/deck.tmd` that is 1 camera transform + **20 per-tile transform tweens** + 6
fragment opacity tweens + 1-2 magic-move crossfades + 1 hint keyframe: **four clocks
(250 / 400 / 500 / 4500 ms) and two easings**, none resolving together.

Alongside them, roughly **110 property changes step in a single frame with no transition
at all**: 6 card properties on each of 18 tiles (`deck.css:141-149`), the accent ring
(`:150-153`), and `display: none !important` on the progress bar and controls (`:704`).
The card chrome is the one layer that would give the moving tiles readable structure, and
it pops at frame 0 on entry and is deleted at frame 0 on exit.

---

## Findings the lenses did not raise (found by direct measurement)

### F1. Vertical stacks are never wrapped, in either mode

`gridRows()` calls `pushRun` only for runs. A stack is pushed straight in at
`deck.js:120` (`rows.push(vertsOf(T[h]).map(...))`), bypassing the wrap entirely. So a
`#` topic with 20 slides stays a 21-cell row forever.

Measured on the 106-slide deck (the authoring style `docs/guide/using/formats.tmd`
recommends):

| | value |
|---|---|
| grid in overview | **21 x 6** |
| fit scale | **0.063** |
| tile size on screen | **60 x 34 px** |
| stage area unused | **~72%** (grid is 3.5:1 against a 16:9 stage) |

The overview does not scale for `#`-topic decks at all. This matters for the redesign:
deleting the `deck.overview &&` gate on `pushRun` fixes runs but leaves stacks unwrapped.

### F2. Arrow-down does not descend into a stack, but two docs say it does

Measured on `corpus/deck.tmd` at the stack lead (h=15, v=0):

- `ArrowDown` -> h15 v0 (**no-op**)
- `ArrowRight` -> h15 v1 "First sub-point" (this is what descends)
- `ArrowUp` -> h1 v0

`down()` is `moveTopic(1)` (`deck.js:722`), which moves a **row**; a stack's sub-slides
are laid out **across** its row (`deck.js:167`), so there is no row below to reach.

Both of these are wrong:

- `corpus/deck.tmd:137` "Press down (not right) to descend into a vertical stack"
- `docs/guide/using/formats.tmd:155` "the audience presses down to go deeper and right to
  move on"

`crates/core/tests/deck_key_sheet.rs` pins the in-product key sheet ("Jump topic"), which
is correct, but does not cover these two prose claims. Separate from the motion work;
listed so it is not lost.

---

## Current-state inventory: 30 animations

Six durations (120 delay / 250 / 300 / 400 / 450 / 500 / 4500 ms) on two unrelated
scales; only 5 read a token. `--tali-dur` (.12s) is **never used in deck.css**.
Three JS timers are hand-synchronised to CSS literals with no shared constant and no test.

| # | Animation | Where | Duration | Easing | Token | RM | Verdict |
|---|---|---|---|---|---|---|---|
| 1 | Camera pan, adjacent step | `deck.css:42` | 500 ms | `cubic-bezier(.2,.8,.2,1)` | No | Yes | Retune |
| 2 | Camera pan, row/topic change | same | 500 ms | same | No | Yes | **Broken** (complaint 1) |
| 3 | Camera pan, menu jump / hash / click-to-source | same | 500 ms | same | No | Yes | **Broken** |
| 4 | Camera zoom, overview enter/exit | same | 500 ms | same, linear in scale | No | Yes | **Broken** (complaint 2) |
| 5 | Per-`<section>` transform tween | `deck.css:40-41` | 500 ms | same | No | Yes | **Delete the selector** |
| 6 | Overview re-wrap flight | `deck.js:103`, `:876` | 500 ms | same | No | Yes | **Remove from motion** |
| 7 | Overview gutter `scale(.9)` | `deck.js:148` | 500 ms | same | No | Yes | Keep, camera clock only |
| 8 | Tile card chrome | `deck.css:141-149` | **0 ms** | none | n/a | n/a | **Broken** (pops) |
| 9 | Current-tile ring `3px` | `deck.css:150-153` | **0 ms** | none | n/a | n/a | **Broken**: renders at **0.95 px** (0.48 px at 100 slides) |
| 10 | Presenter chrome hide | `deck.css:704` | **0 ms** | `display:none` | n/a | n/a | **Broken**: defeats the fade at `:643` |
| 11 | Overview hint pill | `deck.css:183-185` | 4500 ms | `ease`, no fade-in | No | Yes | Retime |
| 12 | Overview fragment flip | `deck.css:295-297` | 250 ms | `ease` | Yes | Yes | **Must be instant** |
| 13 | Overview magic-move flip | `deck.css:203-204` | 400 ms | `ease` | No | **No** | **Must be instant** |
| 14 | Fragment reveal | `deck.css:288-292` | 250 ms | `ease` | Yes | Yes | Keep, gate during flight |
| 15 | Fragment `.fade-out` | `deck.css:314-320` | 250 ms | `ease` | Yes | Yes | Correct as written |
| 16 | Fragment `.highlight` | `deck.css:335-337` | 300 ms | `ease` | No | Yes | Keep, retokenise |
| 17 | Code line-step highlight | `deck.css:360-362` | 300 ms | `ease` | No | Yes | Keep, retokenise |
| 18 | Magic-move crossfade | `deck.css:196` | 400 ms | `ease` | No | **No** | Resync |
| 19 | Magic-move line glide | `deck.js:545` | 450 ms | `cubic-bezier(.2,.8,.2,1)` | No | Yes | Resync |
| 20 | Magic-move new-line fade | `deck.js:550` | 400 ms + 120 delay | `ease` | No | Yes | Resync |
| 21 | Magic-move cleanup timers | `deck.js:547`, `:551` | 480 / 560 ms | n/a | No | n/a | Make derived + cancellable |
| 22 | Auto-animate FLIP | `deck.js:398` | 500 ms | `cubic-bezier(.2,.8,.2,1)` | No | Yes | Keep, retune |
| 23 | Auto-animate overlap write (meant to be instant) | `deck.js:421` | **500 ms via #5** | inherited | No | Yes | **Bug**: 960 px phantom slide-in |
| 24 | Auto-animate outgoing hide | `deck.js:424` | **0 ms** | none | n/a | n/a | **Broken**: hard cut inside a morph. `.tali-aa` (`:422`) has no CSS rule anywhere: dead |
| 25 | Auto-animate settle timer | `deck.js:441` | 520 ms | n/a | No | n/a | Derive |
| 26 | Progress-bar fill | `deck.css:627` | 250 ms | `ease` on `width` | Yes | **No** | Gate |
| 27 | Controls idle fade | `deck.css:643` | 250 ms | `ease` | Yes | **No** | Route through the gate |
| 28 | Feed smooth scroll | `deck.js:1550` | UA | UA | n/a | **No** | Gate |
| 29 | Menu / share open-close | `deck.css:663`, `:713` | **0 ms** | none | n/a | n/a | Inconsistent, pick one rule |
| 30 | Tile hover, copy confirm, theme segment | `deck.css:155-159` etc | **0 ms** | none | n/a | n/a | Acceptable |

---

## The motion language the deck should have

1. **One gesture, one moving thing, one clock.** While the camera moves, nothing else
   animates. Content state resolves instantly before the camera starts or after it lands.
2. **Duration derives from distance; on-screen velocity is the constant.** Budget:
   sustained travel <= 2.2 screen-widths/s, peak/mean easing ratio <= 2.5. Today: 28
   screen-widths/s and a ratio of 4.06.
3. **Above ~1.25 screen-widths, cut.** Constant-scale panning is honest to about 1.25
   screen-widths. A van Wijk arc extends that to about 3.5. Beyond that the arc's
   mid-zoom is illegible (at D=14 body text renders at ~3 px), so it buys orientation you
   cannot read.
4. **The camera never frames world space that has no cells.**
5. **A zoom has a fixed point, and it is the slide you are on.** Scale interpolates in
   **log space**, so enter and exit are exact inverses.
6. **Layout changes are never animated; they happen in a frame where they are invisible.**
   One cell exactly fills the 16:9 stage at present scale (`deck.css:15-21` +
   `deck.js:184`), so re-wrapping the grid *and* re-aiming the camera at the same tile in
   the same transition-free frame is a provable no-op on screen.
7. **Anything whose job is to be readable is drawn in screen space**, counter-scaled by
   the already-published `--tali-deck-scale`.
8. **Interrupt retargets; it never teleports.** Stripping the transition class mid-flight
   (`deck.js:195`) snaps to the specified value.
9. **`prefers-reduced-motion` is one gate, not eleven.**
10. **Timings live in tokens; timers derive from them.**

---

## Options

### A. Cut, don't fly (effort M) - recommended by the audit

Keep the one-camera architecture. Make the camera distance-aware, make the overview
toggle a two-phase gesture with the reflow hidden in a transition-free frame, gate every
content clock.

- `applyCam` computes `D` in cells and writes the transition **inline**: `D <= 1.25` ->
  360 ms `cubic-bezier(.33,.7,.3,1)`; `D > 1.25` -> **no camera transition**, plus a
  140 ms opacity fade-up on the arriving leaf.
- **Delete `> section` and `> section > section` from `deck.css:39-42`.** Single
  highest-leverage line: closes #5, #6 and #23 at once.
- `setOverview` becomes Phase 0 (one `.tali-nofx` frame: flip the class, reflow, resolve
  content, re-aim the camera at the tile's new cell at present scale) then Phase 1 (one
  camera-only 420 ms tween). Provably invisible, per principle 6.
- `fitOverview` anchors on the current tile instead of the grid centroid.
- Chrome, ring counter-scale, hint retime, token tier, derived timers, one RM gate.

*Leaves unfixed on purpose:* the middle band. A 2.24-screen-width topic change in
`corpus/deck-marginalia.tmd` currently pans legibly and would now cut.

### B. One world, one clock (effort L)

Everything in A, plus the grid becomes mode-invariant and serpentine.

- `pushRun` wraps in **both** modes (delete the `deck.overview &&` gate). **Amendment
  from F1: it must also wrap stacks**, or `#`-topic decks keep 21-wide rows.
- Odd rows lay out **right to left**, so the last cell of a row sits directly above the
  first cell of the next. Every linear step, including a topic change, becomes exactly
  one cell.
- `moveTopic` stops meaning "row above/below, same column" and means **previous/next
  topic**, which also kills its 12-cell clamp whip and its non-invertibility.
- A rAF camera driver replaces the CSS transition: straight pan below 1.25, van Wijk arc
  (rho=1.42) from 1.25 to 3.5, cut above. Log-space scale. Interrupt retargets.
- The overview needs **no reflow at all**, so A's Phase 0 becomes unnecessary.

*Costs:* present mode becomes a 2-D layout; serpentine reading order needs an affordance
(slide numbers on tiles) or the map is confusing; `gridRows`/`posOf`/`positionGrid`/
`moveTopic`/`moveHighlight` and the arrow-key semantics all move together as one coupled
change; the rAF driver takes over compositor-thread interpolation and needs a perf check
at 100+ slides.

### C. Two views, one shared element (effort L)

Present mode keeps A's policy; the overview stops being a camera state and becomes a
screen-space CSS grid with its own scroll, entered by a shared-element FLIP of the current
slide.

Structurally eliminates ~12 overview findings rather than patching them (sub-pixel ring,
illegible tiles at scale, the `deltaMode` wheel bug, pan-into-void, the 475 px arrow-key
lurch, fit reset on every save, Tab order walking every off-screen slide).

*Costs:* contradicts the design note the engine is built on (`deck.js:82-88`,
`deck.css:133-140`); tile content must stay live under `sync()` and incremental block
ops; largest diff by far. And it fixes overview *usability*, which is a different
question from the three complaints raised.

---

## Decision (2026-07-24)

Author delegated the call. **Option A, plus pinch / ctrl+wheel to enter the overview.**

Deciding evidence, from the measured step table:

```
corpus/deck.tmd, steps in screen-widths:
strip:  [1,1,1,1,1,1,1,1,1,1,1,1,1,1, 14.01, 1,1]
                                        ^^^^^ the only step over threshold
```

In the strip layout exactly one step exceeds 1.25 screen-widths, and it is precisely the
topic change; the same holds on the 106-slide deck (the only long steps are the 5 topic
changes). So A's cut lands at the scene change and nowhere else. That is a film cut, not a
fallback. Compact-left-to-right was rejected because its cuts land at arbitrary positions
(every `cols` slides, mid-topic); serpentine was rejected because the prototype showed the
overview reading `10 9 8 7 6` across row 2 and "Second sub-point" before "First
sub-point", a real legibility cost paid on every use of the map to fix a step that A fixes
for free.

The overview work is identical under A and B, so nothing here is wasted if B is revisited.

## Shipped (2026-07-24)

Implemented in `crates/core/assets/js/deck.js` + `assets/css/deck.css`, plus the F2 doc
fix in `corpus/deck.tmd` and `docs/guide/using/formats.tmd`.

**The camera now has three primitives instead of one** (`CAM = {pan, cut, zoom}`,
published to CSS as `--tali-deck-*` so the stylesheet runs off the same numbers):

| move | before | after |
|---|---|---|
| step (<= 1.25 screen-widths) | 500ms `cubic-bezier(.2,.8,.2,1)` | 360ms `cubic-bezier(.33,.7,.3,1)` pan |
| topic change (14 screen-widths) | the same 500ms whip | **cut** + a 140ms fade-up |
| overview enter/exit | the same 500ms whip, carrying a reflow | rAF tween, camera only, log-space scale |

**The grid only ever changes while the change is invisible.** That single reordering is the
whole overview fix. In step mode the camera frames one cell, so re-laying the grid (the
wrap, the gutter) moves nothing anyone can see: it happens before the zoom on the way in
and after it on the way out. The gutter comes along by zooming the camera an answering
`1/GUTTER`. What is left is a zoom that moves the camera and nothing else.

`.tali-cam-anim > section` and `> section > section` are gone from the stylesheet. Tiles
have no transform transition at all now.

### Measured, same harness as the audit

| | before | after |
|---|---|---|
| current slide's excursion on overview open | 2.2 screen-widths, path reverses | **0.166**, residual arc 0.034 |
| grid change visible in phase 1 | the cascade | **0px shift, 0px size change** (pixel-exact) |
| intermediate frames on a topic change | 14 slides swept, ~9ms each | **0** (one camera position, whole tween) |
| world pitch between tiles during the zoom | 4 different offsets | **one value, every frame** (rigid) |
| concurrent animations, 111-slide deck | 108 | **4** |
| tile size, 111-slide deck | 60x34px | **272x153px** |
| "you are here" ring | 0.95px (0.48px at 100 slides) | **2.6-3.0px at any zoom** (counter-scaled) |

Frame timing on the 111-slide zoom is unchanged: repeated runs of old and new both sit at
32 frames/500ms with occasional dropped-frame runs. An early single-sample reading
suggested a regression; it did not survive repetition.

### Also shipped

- **Stacks wrap too.** `pushRun` now applies to a `#`-section's sub-slides, and
  `positionGrid` places each sub-slide from its own grid cell rather than `v` cells across
  the wrapper. A 20-slide section was previously one 20-wide strip of specks (F1).
- **Readability floor** (`MIN_TILE_PX = 150`). If the map cannot fit at a readable size it
  fits the *width* and opens on the slide you were looking at: one pan axis, not two.
  `deck.ov.fit` stays the true fit-everything scale so the wheel can still pull all the way
  out.
- **Slide numbers on tiles** (`data-tali-n`, top-right to match `.tali-slide-number` and
  because bottom-left sat on body text).
- **Pick-then-exit.** Clicking a tile aims before leaving the map, so the zoom-in flies
  into the tile you picked instead of zooming into the slide you came from and panning.
- **Zoom out to open the map** (trackpad pinch / ctrl+wheel down, two-finger contract on
  touch), with a 250ms-pause hysteresis. Accelerator only; ctrl+wheel *in* is left to the
  browser.
- Presenter chrome fades on the zoom's clock instead of `display: none`; the map hint waits
  for the zoom to land.

### Follow-up (2026-07-24, same day)

- **↑↓ topic-jump removed** (author call). From a slide in the main run, `moveTopic` "kept
  the column" and could land you on the *second* sub-slide of a section, skipping the first
  — genuinely confusing. `moveTopic` is deleted; `down()`/`up()` are now `next()`/`prev()`,
  so ↓/→/Space all advance and ↑/← retreat, matching PowerPoint / Keynote / Google Slides.
  The overview map keeps ↑↓←→ as its 2-D selection cursor (`moveHighlight`, untouched). Key
  sheet + `deck_key_sheet.rs` + the `.tmd` docs (formats, demo×2, samples, corpus) updated;
  a `!js.contains("moveTopic")` guard keeps the jump from silently returning.
- **MEDIUM bug fixed** (found by the rust-reviewer): re-entering the overview while an exit
  zoom was still mid-flight popped the camera — the invisible-reflow re-pin assumed the grid
  was at rest/unwrapped, but mid-exit it is still wrapped and `from` is a live interpolated
  camera, so the instant reframe (at `from.scale/GUTTER`, an over-zoom toward ~1.85×) was
  visible. Fixed by gating the re-pin on `wasWrapped`: if the grid was already wrapped we
  just reverse the zoom from wherever the camera is. Verified: mid-exit re-enter now peaks at
  0.35× (was heading to ~1.85×), biggest per-frame scale delta 0.007, lands clean.
- **LOW nit fixed:** `onTouchMove` now has the `deck.feed` early-return its siblings carry.
- Two LOW tradeoffs left as-is (flagged to author, not defects): ctrl+wheel-*down* claims
  browser page-zoom-out over the deck to open the map (the approved zoom-out gesture), and it
  also fires inside an embedded deck on a scrollable page.

## Open decisions

1. ~~**Between topics: cut, or move?**~~ Resolved: cut. See Decision above.
2. ~~**Is the overview allowed to be a different layout from present mode?**~~ Resolved:
   yes, and the divergence is safe now that it is only ever applied while invisible.
3. **Should an out-of-order arrival look different from a step?** Menu picks, deep links,
   back/forward and click-to-source landings now cut when they are far, which is most of
   this, but they are not *distinguished* from a step.
4. **Is the overview a real navigator for 100+ slide decks (then C, eventually), or a
   glance at a 20-slide talk?** The readability floor closes most of the gap C was for.
5. **Wrap width on very large decks.** Each run wraps to `ceil(sqrt(n))`, which is right
   per run but stacks five topic blocks into a tall narrow column. Fitting the width
   compensates; choosing the column count from the viewport would do better.

## Verification note

Corpus decks render **identical HTML** under all three options, so `cargo test` covers
structure and not motion. Verified in-browser at 1440x900, 1717x1233 and 900x1440
(narrow-tall) on `corpus/deck.tmd` and a 111-slide stress deck: step + topic-change,
overview open/close from several tiles, wheel/pinch gesture (including that a plain wheel
and ctrl+wheel-in are *not* claimed), reduced motion, console clean. 638 core + 372 server
tests pass, clippy and `cargo fmt` clean.

## Refuted (21, not defects)

`duration-zoo`, `raster-cost-during-whip`, `ov-exit-double-camera-write`,
`ov-double-scale-tween`, `ov-no-compositing-hints`, `ov-no-cancel-no-exit`,
`ov-dark-tuned-tile-styling`, `ov-hint-toast`, `ov-fit-margin-wastes-non-binding-axis`,
`ov-grid-recomputed-per-input-event`, `forced-layout-thrash-at-morph-start`,
`auto-animate-animates-font-size`, `no-single-motion-language`,
`non-compositable-props-under-camera`, `crowded-concurrent-tracks`,
`reduced-motion-is-a-hard-cut`, `per-tile-gutter-scale`,
`overview-mass-promotion-and-shadow-paint`, `root-custom-property-per-camera-frame`,
`announce-flood-and-inert-live-region`, `feed-smooth-scroll-and-observer-scan`
