# UI-audit findings (2026-07-08)

First findings from the `tools/ui-audit` harness (a validation run over a 3-unit
slice: `corpus/deck.tmd`, `corpus/media/gallery.tmd`, `corpus/reactive/js-error.tmd`).
Each was investigated to root cause and reproduced (or shown not to reproduce) in
a real browser viewport before acting.

## Finding 1: `.tali-js-error` box clips long stack-trace lines — REAL, FIXED

**Symptom.** In the `{js}` runtime-error box, long stack-trace lines (especially
URLs) are cut off at the right edge with no wrapping and no visible scrollbar, so
they read as silently truncated. Worst on mobile; on laptop/portrait only the
single longest line clips.

**Root cause.** `.tali-js-error` is a `<pre>` (set in
`crates/core/assets/js/qmd-js.js:156,269`). A bare `<pre>` has
`white-space: pre` (no wrap), so long lines overflow. Its rule at
`crates/core/assets/css/base.css:790` also sets `background: #fff0f0` (the
shorthand), which clears the `background-image` scroll-shadow affordance that the
generic `pre` rule (`base.css:316`) uses to signal "there is more to scroll".
Net result: overflow with no wrap, no shadow cue, and no scrollbar in a static
render.

**Fix.** Add `white-space: pre-wrap; overflow-wrap: anywhere;` to
`.tali-js-error` so long lines (and long URLs) wrap inside the box. No horizontal
overflow remains, so the missing scroll-shadow no longer matters.

**Verification.** Rebuilt, re-rendered `corpus/reactive/js-error.tmd`, confirmed
in a real browser that the stack trace now wraps within the box in light + dark.

## Finding 2: deck reader-mode menu button over body text — MOSTLY A CAPTURE ARTIFACT, minor real residual, NOT fixed

**As reported by the audit.** The fixed corner menu button overlaps and clips
body text (e.g. clipping "sees" in the last paragraph) on the mobile deck
viewport, in both themes.

**What the investigation actually found.** The reported form does **not**
reproduce in a real 390px viewport: scrolled to the end of the document, the
menu button (`.tali-controls`, `position: fixed; right:14px; bottom:14px`, 34px)
sits cleanly in the reading column's `3rem` bottom padding with zero text
overlap. The audit's screenshot came from a **full-page** capture, and Chrome
renders `position: fixed` elements at the *document* bottom in full-page mode, so
the button was painted over the last paragraph in the image only, not in a real
viewport.

A scroll sweep of a long deck (`docs/guide/demo.tmd`) did surface a **minor real
residual**: at one mid-scroll position a full-width line
("(current + next slide, notes, timer)") was covered by ~22x19px of the button
(a few characters). This is intermittent (depends on where a full-width line
lands relative to the bottom-right corner), the button is semi-opaque with a
blur, and it fades after 3s idle.

**Why not "fixed" here.** The reading column is full-width on mobile (1rem
gutters), so a fixed bottom-right control inherently floats over the column's
bottom-right corner. Removing that overlap cleanly is a design decision about the
deck's reader chrome (relocate/restyle/shrink the control, or reserve an
asymmetric safe area), not a mechanical patch. Flagged for the author rather than
changed speculatively.

## Harness limitation this exposed (mitigated)

Full-page screenshots misplace `position: fixed`/`sticky` elements (nav bars,
floating buttons, cookie banners) to the document bottom, which can make the
analysis agent report a false "element overlaps bottom content" bug. The audit
workflow's analyze prompt now warns about this so such apparent overlaps are
treated as capture artifacts unless confirmed in a real viewport. See
`tools/ui-audit/audit.workflow.js`.
