# Mobile audit (2026-07-26)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Lens:** the reader experience on a touch device, measured in a real browser at the recorded
three-viewport matrix plus the landscape-phone band. Proposed by the author out of real device
testing ("many of the computer shortcuts are still visible on mobile and there are other small
visual bugs"). First genuinely new lens since the twelve AP slots and four non-AP lenses closed.

**Verdict: the symptom the author reported is real, reproduces on demand, and is one root cause
wearing seven faces.** Nothing here is a rendering bug or a layout collapse — the responsive layout
is sound, there is no horizontal overflow anywhere I measured, and the console is clean on every
page tested. The defect is that **the tool never asks what kind of device it is on.**

## The root cause

**There is not one input-capability query in the entire tree.** Measured over every `.css`, `.js`
and `.rs` file in `crates/` + `web-client/`:

| Query | Occurrences |
|---|---|
| `pointer: coarse` | **0** |
| `hover: none` | **0** |
| `any-pointer` | **0** |

Every decision about whether to show a keyboard hint, a hover-revealed control, or a presenter tool
is made from **viewport width** (`max-width` 30/40/60/73rem, `min-width: 60.001rem`) or from **deck
layout mode** (`html.tali-feed`). Both are proxies, and both are wrong in the same direction: a
phone that is wide (landscape) or in stepped mode is treated as a desktop.

The browser is not withholding this information. At 390×844 and at 844×390 with touch emulation,
`matchMedia('(pointer: coarse)')` and `matchMedia('(hover: none)')` both return **true**. The tool
simply never asks.

This is not an argument for a device-detection layer. `pointer`/`hover` are ordinary CSS media
features, they need no JS, no user agent sniffing, and no new config knob — which matters given the
project's minimal-config rule. The fix shape for most of what follows is a media query.

## Method

`target/debug/taliesin preview site 4388` (the marketing site, with `docs/guide`, `docs/internals`
and the five gallery projects mounted), driven through the chrome-devtools MCP with real device
emulation (`Emulation.setDeviceMetricsOverride`, `mobile,touch`), not window resizing.

**Trap hit, recorded so the next round does not lose an hour to it:** `resize_page` floors at about
**500px** — it resizes the *window*, and Chrome will not make a window narrower than that. Two of my
early probes reported `innerWidth: 500` while I believed I was at 390, which silently moved me across
the 40rem breakpoint that half this audit is about. **Use viewport emulation, never window resize,
for anything below ~500px.** This is the same floor the recorded scroll-feature gotcha names.

Surfaces exercised: the marketing site (home, `formats`), a guide book chapter
(`docs/guide/using/formats.html`), and the demo deck (`docs/guide/demo.html`) in both orientations.

## Falsifying my own entry, first

The backlog entry I wrote for this round predicted the `⌘K` hint as the headline. **Confirmed, but
it is the least of it** — MOB-3 below is real and is the smallest-blast-radius item in the list. The
entry's framing ("the breakpoints are all width") was right and turned out to under-state the
problem: mode-gating (MOB-2) is a second proxy the entry never named, and it is the worse one,
because it changes under the reader's hand when they rotate the device.

**One prediction I got outright wrong, corrected here so it is not carried forward.** My first probe
concluded the phone slide-feed *never engages* — no feed class, no scroll, page height equal to the
viewport. That was a measurement error: the feed flag lands on `document.documentElement`
(`html.tali-feed`), not on `.tali-deck`, and the scroller is `.tali-slides`, not the document. **The
feed works correctly**: `html.tali-feed` is set, `scroll-snap-type: y mandatory`, 10,972px of scroll
across 14 slides, and a scroll lands exactly one viewport (844px). The A3 phone-feed routing is
healthy and is not in the findings below.

## Findings

### MOB-1 (HIGH) — the deck menu hands a touch reader a keyboard manual

On a 390×844 touch phone, opening the deck menu renders a **"KEYBOARD" section 125px tall listing
ten shortcuts** — `→ ↓ Space`, `← ↑`, `Home End`, `O`, `0`, `F`, `S`, `B`, `?`, `Esc` — plus hint
badges `O` / `S` / `F` on the Tools rows. On a 390×844 screen that legend is roughly a third of the
menu, and it pushes the controls that *do* work (the slide list, Present, Share, the theme segment)
out of reach.

`deck.js`'s `buildMenu()` appends `KEYS_HTML` unconditionally, and `tool()` takes a `hint` argument
that is always passed. `.tali-menu-keys` (`deck.css:762`) and `.tali-menu-hint` (`deck.css:745`)
carry no capability gate.

**This is an oversight, not a decision, and the same function proves it.** `buildMenu()` already
suppresses contextually in three other places: the theme row is dropped for an embedded deck, the
Present item is CSS-hidden outside the feed, and the Speaker item is hidden *in* the feed
(`deck.css:632-633`). The keyboard block was simply never included in that thinking. The internal
contradiction is visible on screen: in portrait, **Speaker view is correctly hidden from Tools while
the keyboard legend still advertises `S` — Speaker view**.

**Fix:** gate `.tali-menu-keys` and `.tali-menu-hint` on `@media (hover: none) and (pointer: coarse)`.
No JS, no new option.

### MOB-2 (HIGH) — rotating a phone gives it desktop affordances

The gates that *do* exist key on deck layout mode, and layout mode is chosen by orientation, so the
reader changes their own capability class by turning the device.

Measured on one emulated phone, same page, rotation only:

| | portrait 390×844 | landscape 844×390 |
|---|---|---|
| `pointer: coarse` | true | true |
| feed mode | on | off |
| Speaker view offered | hidden (correct) | **visible** |
| keyboard hint badges | O, S, F | O, S, F |
| keyboard legend | visible | visible (125px) |

Speaker view opens a second presenter window. Offering it on a phone is a dead end, and the code
already knows that — it hides it in portrait via `html.tali-feed`, a *mode*, so a rotation to
landscape hands it straight back.

**Fix:** gate presenter-only tools on input capability rather than on feed/stepped mode. The two
conditions are independent and should not be conflated.

### MOB-3 (HIGH) — the ⌘K hint appears on any touch device wider than 640px

`site.css:197` hides `.tali-search-kbd` under `@media (max-width: 40rem)`, and **its own comment
states the intent as capability**: *"The keyboard-shortcut hint is meaningless on a touch phone (no
⌘/Ctrl key)."* 40rem is 640px, so the rule misses every phone in landscape and every tablet.

Measured at 844×390 with touch: `pointer: coarse` true, `hover: none` true, and the badge renders
**"Ctrl K"**.

The platform swap compounds it. `search.js:1064` sets `IS_MAC` from
`navigator.platform || navigator.userAgent` and rewrites the server-rendered `⌘K` to `Ctrl K` on
anything non-Mac — **no touch check**. So an Android phone in landscape is told to press Ctrl+K and
an iPad is told to press ⌘K. The button's `aria-keyshortcuts` is fine and should stay; only the
visible badge is wrong.

Emitted at `site/chrome.rs:41` and `:506`.

**Fix:** replace the width gate with a capability gate.

### MOB-4 (HIGH) — the copy-code and copy-link buttons are invisible on touch

Both reveal-on-hover controls sit at `opacity: 0` and are revealed only by `:hover` or
`:focus-visible`, with no `hover: none` fallback:

- `.tali-copy` (`base.css:394-401`) — revealed by `pre:hover`. Measured `opacity: 0` on a touch
  device. **Copy-code is arguably more valuable on a phone than on a desktop** (no easy text
  selection across a scrolling `<pre>`), and it is exactly where it cannot be found.
- `.tali-anchor` (`base.css:321-332`) — revealed by `:is(h1..h6):hover`. Same measurement.

There is a sharp irony in the anchor rule. `base.css:324-327` adds a centred invisible 24×24
`::after` overlay with the comment *"WCAG 2.5.8: ... expands the click/tap target"* — the tap target
was sized **for touch**, on a control that on touch is never visible. The size problem was solved;
the visibility problem underneath it was not.

Both keep `pointer-events: auto` at `opacity: 0`, and opacity does not disable hit-testing, so the
targets stay live while invisible. *(Confidence split: the invisibility is directly measured; that a
reader can mis-tap an invisible control follows from CSS semantics — my `elementFromPoint` probe was
inconclusive because the element sat outside the viewport at measure time, so I am not claiming it as
observed.)*

**Fix:** under `@media (hover: none)`, either show these persistently or drop them entirely. Showing
them is the better call for `.tali-copy`; `.tali-anchor` is a judgment call worth the author's ruling.

### MOB-5 (MEDIUM) — the book chapter drawer does not behave like a drawer

On a 390×844 phone the Chapters drawer covers **93% of the viewport width** and the full height, over
a backdrop. Three problems, all measured:

1. **Page scroll is not locked.** With the drawer open, `window.scrollBy(0, 400)` scrolled the
   article behind it by **328px**. `body` is `position: static`, `overflow: visible`,
   `overscroll-behavior: auto`; the panel is also `overscroll-behavior: auto`, so scrolling *inside*
   it chains to the page at either end. On a phone this means a swipe meant for the chapter list
   moves the chapter, and dismissing the drawer returns the reader somewhere they did not choose.
   **There is no scroll-lock code anywhere in the client JS** — this was never implemented, rather
   than implemented and broken.
2. **It is not a dialog.** `role`, `aria-modal` and `aria-hidden` are all absent, and after opening,
   `document.activeElement` is still `.tali-book-body`. A backdrop plus 93% coverage is a modal by
   every behavioural definition; the button carries `aria-expanded="true"`, which is the disclosure
   pattern, not the dialog one. A focus trap already exists in the codebase
   (`code-enhance/04-focus-trap.js`, built for Cmd-K) and is not used here.
3. **The close control is 26×22px** — under the **24px WCAG 2.5.8 AA floor** on the height axis, and
   well under 44px. *Severity limited, and I got this wrong before checking:* tap-to-dismiss on the
   backdrop **works** (a 42%-black scrim over the remaining 7%), and Escape works, so the small
   button is not the only way out. It is still the only dismiss control a reader can *see and aim
   at*, and it is under the AA floor, but this is a size defect rather than a trap.

### MOB-6 (MEDIUM) — the marketing site tells phone readers to press keys

Content, not code, and on the two highest-traffic pages a new visitor sees:

- `site/index.tmd:121` — *"press `F` for fullscreen, or open it in its own tab"*
- `site/formats.tmd:42` — *"The deck above is live: click it, arrow through it, press `F` for
  fullscreen."*

Verified rendered and visible at 844×390 with touch. "Arrow through it" and "press F" describe a
keyboard the reader does not have, about a deck sitting directly above the sentence.

**The distinction that matters, so this is not over-applied:** a *reference table* of shortcuts in
the guide is legitimate content on any device, and I am not flagging those. This is an
**instruction** about the widget on the current page, and it is wrong for the device.

### MOB-8 (MEDIUM) — the book topbar grows instead of truncating on narrow screens

Author-reported ("the Chapters text overflows a bit on super narrow screens"), then measured. **The
row never overflows horizontally at any width I tested — it grows vertically instead**, because the
book title wraps:

| viewport | title lines | topbar height | % of viewport height |
|---|---|---|---|
| 390px | 1 | ~48px | 6% |
| 320px | 2 | 56px | 7% |
| 280px | **3** | **77px** | **12%** |
| 240px | **3** | **77px** | **13%** |

`.tali-book-brand` (`site.css:291-292`) sets `display: block` with **no `white-space: nowrap`, no
`overflow: hidden`, no `text-overflow: ellipsis`, and no `min-width: 0`**. As a flex item its default
`min-width: auto` refuses to shrink below content, so it wraps rather than truncating — and because
the topbar is **sticky**, the cost is subtracted from every screen of reading, permanently.

**This is the same failure the file already documents once.** `site.css:244-248` explains that the row
"puts all of its shrink pressure on the one item with no min-content floor", which collapsed the
hamburger to a ~4px dot; the fix was `flex: none` on the icon. That protected the icon and left the
pressure on the title, which has no floor of its own. The idiom is used correctly elsewhere in the
tree (`deck.css:761`, `.tali-menu-slide-t`, is `nowrap` + `ellipsis`).

**Hiding the "Chapters" label is right but is NOT sufficient.** Measured at 240px: the title needs
**146px** for one line; the label plus its gap is worth **66px** (27% of the row); hiding it leaves
**103px** — still short, so the title still wraps. The unbounded growth is the missing ellipsis, and
a longer book title than `docs/guide`'s would wrap at far wider viewports.

**One fact that makes the label safe to hide:** the button is already
`<button aria-label="Chapters"><svg/><span>Chapters</span></button>`, so the accessible name comes
from `aria-label`, not the span. `display: none` on the span costs screen-reader users nothing — no
`.tali-sr-only` dance needed.

**Measured threshold, if the label is dropped by width:** with the label, the title fits one line down
to ~360px and wraps at 320px; without it, one line holds to ~280px. So the label stops earning its
width between **320 and 360px** — about **22rem**. With the ellipsis fix in place this threshold is no
longer load-bearing: it only decides how much of the title survives, not whether the bar grows.

### MOB-7 (LOW) — the desktop nav is served to landscape phones

The burger appears only under 40rem, so at 844×390 a touch phone gets the **full 7-link desktop
nav** with a minimum link height of **26px**, plus the search button and settings gear. 26px clears
the 24px AA floor and misses the 44px AAA target. Same root cause as MOB-3: the burger breakpoint is
a width proxy for "small screen", and a landscape phone defeats it.

Deck menu launcher is 34×34; deck menu rows are 34px and 52px.

## Measured healthy — do not re-scope

- **The phone slide-feed (A3) works.** `html.tali-feed`, `scroll-snap-type: y mandatory`, 10,972px
  over 14 slides, one-viewport snapping, rotation re-routing live. See the correction above.
- **No horizontal overflow** on any page or viewport measured
  (`documentElement.scrollWidth <= innerWidth` throughout).
- **Console is clean** — zero errors or warnings across every page and viewport tested.
- **The `⌘K` badge is correctly hidden at 390px**, and the book topbar collapses to
  Chapters + title + icons without crowding.
- **Body typography is right on a phone**: 16px, 27.2px line height, 39 characters per line at
  390px. Narrower than the desktop ~70ch keep, but 70ch is unreachable at 390px without shrinking
  the type, and 39ch is inside normal mobile practice. Not a defect; measured so it is not re-raised.
- **Code blocks scroll horizontally rather than wrapping** (11 of 18 on one chapter,
  `white-space: pre`, worst overflow 368px). Flagged as an observation, not a finding: wrapped code
  is often worse than scrolled code, the scroll affordance is visible, and this looks deliberate. If
  the author disagrees it becomes a real item.

## Not measured (so a green result here is not mistaken for coverage)

Real iOS Safari and real Android Chrome (this was Chromium device emulation, which models viewport,
DPR and pointer capability but **not** WebKit behaviour, real touch latency, momentum scrolling, the
dynamic viewport toolbar, or safe-area insets); a real screen reader on a phone (VoiceOver /
TalkBack); tablet-sized viewports; the `--host` QR phone-preview flow, which is a first-class phone
feature and got no coverage here; the deck feed under `prefers-reduced-motion`; and every corpus
project except the three surfaces named under Method.

**Note on provenance:** the author described this as auditing functionality added by a previous
session with a weaker model. **That work is not in this repository** — at the time of the audit
`main == origin/main == d8b867c`, with no other branches, worktrees or stashes, and the mobile
surfaces examined here date to earlier commits (`8bb0a65` rename-era, `2369d80` WCAG batch,
`dc0cc58` drawer outline). Everything above is an audit of the shipped tree.
