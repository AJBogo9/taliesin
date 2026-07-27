# Deck audit re-run, crossed with touch — 2026-07-27

The 07-12 deck round is closed (everything but the deliberately-deferred B3-18), so this is not a
re-check of its 43 findings. It is the re-run [backlog.md](backlog.md) asked for: the deck surface
has churned **2,608+/1,213-** across `deck.rs` + `deck.js` + `deck.css` in 49 commits since that
round, the mode-model was deliberately reshaped underneath it (reader + PDF deleted, phone feed
added, motion round 07-24), and the 07-26 mobile round put touch back at the top of the board
without ever entering a deck.

**Method.** `corpus/deck.tmd` (19 leaf slides) driven live in Chrome via the devtools MCP, at three
emulated devices — landscape phone `844x390` and portrait phone `390x844`, both `mobile,touch`
(`pointer: coarse` + `hover: none` confirmed live), plus `1440x900` desktop as the control. Every
number below is browser-measured on the running product; the one source-level hypothesis is marked
as such and is **not** counted as a finding. The final defect was re-measured on the **built**
`file://` artifact as well as the preview, so it is not a preview artefact.

**Result: two real defects (both SHIPPED), one consistency gap against a standard the tree adopted
12 hours earlier (SHIPPED), and two behaviours worth an owner ruling.** The deck's touch story is
otherwise in good shape, and the "measured healthy" list at the bottom is longer than the findings
list — that section is the point of the round as much as the defects are.

| | finding | status |
|---|---|---|
| **DT-1** | code-copy button invisible on every deck, on every touch device | **fixed** |
| **DT-2** | deck never got MOB-7's 44px tap-target floor | **fixed** |
| **DT-5** | adjacent slides bleed into the letterbox at any aspect ≠ 16:9 | **filed** (design ruling) |
| DT-3 | a slow swipe is rejected for no reason in stepped mode | filed (ruling) |
| DT-4 | the share panel is desktop-framed on a phone | filed (judgment) |

**DT-5 was found by accident and is the round's most consequential finding.** It surfaced as an
unexplained glyph in a screenshot corner that `elementFromPoint` reported as `BODY`; chasing it
instead of dismissing it is what turned it into a measurement. The first probe missed it because it
filtered elements by their **top** coordinate rather than testing **intersection** — a full-height
neighbour has `y = 0` and was silently excluded. *A rect filter that tests one edge is not a
visibility test.*

---

## DT-1 — the code-copy button is unreachable on a deck, on every touch device · **the real one**

**Measured.** Portrait phone `390x844`, `hover: none` + `pointer: coarse` live:

| | |
|---|---|
| `.tali-copy` computed opacity, on a deck | **0** |
| its box | 20.5 × 19.1 px (**under the 24×24 WCAG 2.5.8 AA floor**) |
| same, in the **built** `file://` artifact | **0** — ships to every reader, not a preview artefact |

Isolated to the cause with a probe that removes every other variable — same page, same stylesheet,
same media conditions, two synthetic `.tali-copy` buttons differing **only** in their ancestor:

```
opacity outside .tali-deck : 1
opacity inside  .tali-deck : 0
```

**Cause (verified, not assumed).** `deck.css:970` declares `.tali-deck .tali-copy { opacity: 0 }` at
specificity **(0,2,0)**. The MOB-4 fix is `base.css:420`, `@media (hover: none) { .tali-copy {
opacity: 1 } }` at **(0,1,0)**. A media query contributes **no specificity**, so the deck's rule wins
outright and *source order is irrelevant* — which matters, because source order is exactly what
MOB-4 was about and exactly what its test guards. The only two reveals a deck offers are
`pre:hover` (impossible on a touch device) and `:focus-visible` (needs a keyboard). So on a phone
the button exists in the DOM, occupies a sub-AA-sized box, and can never be seen or used.

**This is MOB-4's third instance.** The 07-26 round found the hover-reveal shape, fixed it for the
page/book surface, and never crossed into the deck. The backlog's own lesson from the path-parity
round — *a finding that names one instance has not enumerated the shape* — applies to it verbatim.
MOB-4's stated justification applies **harder** here than where it was fixed: "it matters MORE on a
phone, where there is no easy text selection across a scrolling `<pre>`", and on the feed that
`<pre>` sits inside a `scroll-snap-type: y mandatory` container, which is the worst place in the
tool to attempt a manual selection.

**Why no test caught it.** `hover_revealed_copy_controls_stay_reachable_without_a_hover` asserts the
**source order** of the two declarations *within `base.css`*. It cannot see a higher-specificity
override in a different file, so it passes while the defect ships. A file-scoped order assertion
cannot express a cross-file specificity fact — the guard is sound for what it guards and
structurally blind to this.

**Fixed**, and the placement is the interesting part. The obvious home for the fix is the capability
block deck.css *already has* — but that block is at `:777`, **above** the `opacity: 0` at `:970`, so
putting it there would have matched (0,2,0) and still lost, silently, exactly the way MOB-4 first
did one file over. **The fix is therefore a second capability block placed directly below the
declaration it overrides**, with the constraint written next to it. The tap target grows by a
centred 24 px `::after` overlay rather than by inflating the chip — `.tali-anchor::after`'s existing
idiom — because a 44 px chip would blanket a short code block, and 24 is the AA floor for an
in-content affordance.

**Verified in the browser on the rebuilt artifact, not just by the test:** `opacity` 0 → **1**;
hit-testing at the chip's centre returns the button; and hit-testing **2 px above the visible chip**
*also* returns the button, which is what proves the overlay actually extends the tap area past the
20.5 × 19.1 box rather than merely existing in the stylesheet.

---

## DT-2 — the deck never got MOB-7's tap-target standard · consistency gap

Every interactive control in the deck, measured on a coarse pointer:

| control | measured | AA (24) | adopted floor (44) |
|---|---|---|---|
| `.tali-ctl` prev / next / menu | 34 × 34, **6 px apart** | pass | miss |
| `.tali-menu-slide` (jump list) | 30.6 tall | pass | miss |
| `.tali-menu-item` | 34.2 | pass | miss |
| theme segment (Auto/Light/Dark) | 30.9 | pass | miss |
| `.tali-share-close` | 30 × 30 | pass | miss |
| `.tali-share-copy` | 60.6 × 34.9 | pass | miss |
| `.tali-copy` (code) | 20.5 × 19.1 | **FAIL** | miss |

`deck.css`'s only capability block (`:777`) *hides* things — key hints and the Speaker item — and
never **sizes** anything. Meanwhile `site.css:359` adopted, under the identical media query and on
2026-07-26, `min-width: 44px; min-height: 44px` for `.tali-book-drawer-close`, with its own comment
recording the reasoning ("the floor is unconditional; touch gets the same 44px the nav burger
already chose"). So this is not a new standard being invented for the deck — it is the tree's own
standard, applied everywhere except here.

**The deck's case is strictly easier than the one MOB-7 solved.** MOB-7 needed an *overlay* rather
than `min-height` because growing a **sticky** bar costs permanent reading height (measured then:
52 px → 75 px, 19.2% of a landscape phone's viewport). The deck's controls float over a fixed
960×540 stage with 14 px of edge clearance — growing them costs no reading height at all, so the
plain `min-width`/`min-height` form is available here.

Three 34 px targets with 6 px gaps is also a mis-tap hazard in its own right, and the neighbour of
"next" is "menu": the cost of missing is a popover over the slide mid-presentation.

**Fixed** by extending the existing capability block with `min-width`/`min-height`. `min-*` rather
than `width`/`height` is load-bearing and is why *this* half could live in the block at `:777` while
the DT-1 half could not: `min-width` beats an explicit `width` by the definition of the used value,
so it is not a cascade contest and its position does not matter.

**Verified in the browser after rebuilding:** prev/next/menu **34 × 34 → 44 × 44**; all 26 menu rows
at a 44 px minimum, **0 under 44**. Re-checked at the tightest viewport (844 × 390) because growing
rows inside a `max-height` popover is exactly where this could go wrong: the menu still measures
304.2 px, still scrolls, is **not clipped** top or bottom, and all three controls remain on screen.
The controls are 11.3% of a landscape phone's height but float over the stage rather than sitting in
a sticky bar, so no reading height is lost — which is the distinction MOB-7's overlay existed to
protect.

*(Also noted, not filed: the preview-only `.tali-dev-toggle` is 56.7 × 25. It is dev chrome, but the
author is a phone user of it whenever the `--host` QR flow is used.)*

---

## DT-3 — a slow swipe is rejected, and in stepped mode nothing else wants the gesture · ruling

**Measured**, landscape phone, synthetic one-finger touch:

| gesture | result |
|---|---|
| 200 px horizontal in ~30 ms | navigates (h 0 → 1, and back) |
| 200 px horizontal over **750 ms** | **nothing happens** |
| 40 px horizontal, fast | nothing (correct — the 50 px distance floor) |

`deck.js:1859` is `if (dt > 600 || Math.max(Math.abs(dx), Math.abs(dy)) < 50) return;`. A time bound
on a swipe normally exists to tell a *swipe* from a *pan/scroll* — but in stepped mode there is no
competing one-finger gesture to disambiguate from: `deck.feed` returns at `:1798` (native scroll
owns the axis) and `deck.overview` returns at `:1799` (the map owns pan/pinch), both **above** this
line, and the stepped stage does not scroll. The distance floor already rejects a tap. So in the
one mode where this bound is live, it can only reject input the reader meant — and the input it
rejects is the slow, deliberate swipe, which is what a reader with a motor impairment makes.

**Not a confirmed defect:** no real user has been observed failing on it. It is a measured behaviour
plus an argument that the guard has no job in the mode it runs in. Recommend dropping `dt` in
stepped mode (keeping the 50 px floor) — but it is a ruling, not a bug fix.

---

## DT-4 — the share panel is desktop-framed on the device it is being read on · judgment

On a phone the panel renders "**Point a phone here**" above a QR that occupies most of the card. The
reader *is* the phone; a QR cannot be scanned off the screen displaying it. The action that works —
Copy — is the secondary control, at 34.9 px tall.

The panel is otherwise well-built here: 320 px card inside a 390 px viewport, nothing clipped or
overflowing, QR legible at that size (screenshot in this round's evidence). So this is framing, not
breakage. **`navigator.share` was absent under emulation, so the Web Share option could not be
measured and is not claimed.**

---

## DT-5 — the slides either side of the current one paint into the letterbox · **filed, needs a ruling**

The stage is a fixed 960×540 (16:9) cell scaled by the camera; slides live at their own coordinates
in one world; `.tali-slides` is **`overflow: visible`** and a `section` declares **no background**.
So whenever the viewport aspect is not exactly 16:9, the fit leaves slack on one axis, and whatever
happens to sit next to the current cell in world space paints into that slack. At rest, not mid-pan.

**Measured on the built artifact:**

| viewport | aspect | slack | what bleeds in |
|---|---|---|---|
| 844 × 390 (phone, landscape) | 2.164 | 151 px horizontal | **both** neighbours, 75.3 px left + 75.7 px right = **17.9% of the viewport** |
| 1440 × 900 (16:10 laptop), top-row slide | 1.6 | 90 px vertical | nothing (no row above at that column) |
| 1440 × 900, slide inside the stack row | 1.6 | 90 px vertical | the slide above, 45 px × full width = **5% of the viewport** |
| exactly 16:9 | 1.778 | 0 | nothing — the only aspect that is clean |

Reading the left edge of a landscape-phone screenshot shows the ends of the previous slide's bullet
lines, in the deck's own ink, next to the current slide's title.

**Why this is the round's most consequential finding.** It is not a touch defect at all — touch is
just how it was found, because a phone in landscape is the widest-aspect device anyone owns and
therefore shows it worst. It lands on the deck's *primary* use: **16:10 is the most common laptop
aspect there is** (every MacBook), so a presenter on the stack row shows the audience a 45-px strip
of the previous slide across the full width of the projection.

**Why it needs a ruling rather than a patch.** The three candidate fixes are not equivalent:

1. **Letterbox mask** — paint opaque bars over the slack in present mode only. Smallest change,
   keeps the camera pan exactly as the 07-24 motion round tuned it.
2. **Hide non-current slides in present mode** — also fixes it, but it *changes the pan*, which
   sweeps through intermediate slides deliberately; the motion round measured that sweep ("a strobe
   of 14 half-frame slides") and reshaped it rather than removing it. This would silently undo part
   of that work.
3. **Clip the stage** — cannot be unconditional: the overview *needs* `overflow: visible` to show
   the whole map.

Option 1 is the recommendation, but it is the author's call because option 2 is the one that also
changes how the deck *feels*, and that was deliberately tuned nine days ago.

---

## An explicitly unverified hypothesis, for the real-device round

`deck.js` contains **zero** references to `visualViewport` (verified: 0 occurrences in `deck.js` and
in `web-client/client.js`). B5-1 deliberately restored pinch-zoom by dropping `user-scalable=no`
(confirmed live: `width=device-width, initial-scale=1`). Nothing in the swipe handler asks whether
the visual viewport is zoomed, so *on paper* a one-finger pan across a pinch-zoomed slide — more
than 50 px, under 600 ms — reaches `onTouchEnd` and navigates away from the slide the reader zoomed
in to read.

**This was NOT reproduced.** The harness cannot set page scale (`Emulation.setPageScaleFactor` is
not exposed through the MCP), and the real behaviour depends on whether the browser fires
`touchcancel` when it takes the gesture over for panning — in which case `onTouchCancel` (`:1866`)
resets `ovTouch`/`ovDragged`/`pinchStart` but pointedly **not** `touch.x`/`touch.y`, leaving a stale
swipe origin that the `dt > 600` bound then usually (not always) discards. Both halves are real
code; the interaction is untested. **This is the single most valuable thing for the standing
real-device lens to check first**, because it can only be answered on hardware.

---

## Measured healthy — do not re-scope

- **The front door holds.** A deck opens as a deck; a portrait phone routes to the feed
  (`html.tali-feed`), a landscape phone stays stepped. Aspect, not width.
- **The feed is correct in every respect measured.** `scroll-snap-type: y mandatory`,
  `overflow-y: auto`, 19 slides at exactly 390 × 844, and `font-size: 16.25px` = 390/960 × 40 to the
  digit, which is the A3 spec's font-size-not-transform rule reproducing the 960 stage exactly.
- **Feed position tracking is exact.** Scrolling drives both chip and hash together: 1/19 → 5/19 →
  13/19 → 19/19, with `#/title-slide` → `#/reveal-one-point-at-a-time` →
  `#/a-chart-that-fills-the-slide` → `#/sec-second-point`.
- **No content is hidden on the feed.** One `<pre>` at opacity 0 across all 19 slides, and it is the
  *first* block of a magic-move whose *final* block is visible — which is exactly what A3 specifies.
- **The capability gating works.** Speaker view is correctly absent on a coarse pointer (0 × 0);
  Present / Overview / Share / Fullscreen are retained; key hints are gone.
- **Gestures work.** Swipe navigates (h 0 → 1 → 0); two-finger pinch-in opens the overview; in
  overview a one-finger pan neither navigates nor exits (**B6-31 holds**).
- **The menu fits the worst viewport.** On a 390 px-tall landscape phone: `max-height: 304.2px`,
  `overflow-y: auto`, 27 items, nothing clipped top or bottom.
- **B5-1 holds.** `width=device-width, initial-scale=1` — pinch-zoom is unblocked.
- **Zero console errors** on every path exercised, in preview and in the built artifact.

## Not measured (name it, so it is not mistaken for coverage)

- **Real iOS Safari / Android Chrome.** This is Chromium emulation: no WebKit, no momentum scroll,
  no dynamic viewport toolbar, no safe-area insets.
- **An embedded `{{< embed >}}` deck on touch** — needs a host page; not built this round.
- **Overview pan while zoomed past fit.** At fit scale `clampOv` has nothing to pan, so the probe
  proved only that pan does not navigate or exit, not that panning itself works on touch.
- **A phone screen reader**, and the `--host` QR phone-preview flow.
