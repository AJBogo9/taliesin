# Section C a11y batch: design

**Date:** 2026-07-16
**Backlog:** §C theme colour-system a11y follow-ups (`notes/backlog.md`), from the
2026-07-09 audit.
**Owner decisions this session:**
1. **`f` keeps its fullscreen coupling**; the new shortcuts toggle is the opt-out.
2. **The settings popover moves focus** to its first control on open.

## Pre-work: the section was re-verified, 2 of 8 items had rotted

The backlog warns that entries rot with no signal (§B rotted entirely). Every §C item was
re-checked against source before this design. **Two are already fixed** and must be deleted
from the backlog, not built:

- *"Embedded deck ignores a sepia host"*: **fixed at the named anchor.**
  [`render/deck.rs:164`](../../../crates/core/src/render/deck.rs#L164) already reads
  `(t==='sepia' ? 'light' : null)`, which is the audit's recommended fix verbatim.
- *"Deck slide-number chip not restyled per-slide"*: **fixed by removing the premise.**
  The chip is now one opaque dark-glass surface in both themes
  ([`deck.css:352-361`](../../../crates/core/assets/css/deck.css#L352-L361)); the comment at
  [`deck.css:525-526`](../../../crates/core/assets/css/deck.css#L525-L526) records that it
  therefore needs no dark override. The bug was scoped to a `html.tali-deck-dark`-only
  restyle that no longer exists.

Both were closed by §F's step 6 (deck theming/a11y/perf). The remaining **six** items were
each confirmed still-live in source; their line numbers had drifted and are corrected below.

## The six items

### 1. Bare `f` forces fullscreen with no opt-out (med)

**Symptom.** [`03-focus-mode.js:80`](../../../crates/core/assets/js/code-enhance/03-focus-mode.js#L80)
fires on a bare `f` with no mechanism to turn it off. WCAG 2.1.4 (Character Key Shortcuts,
Level A) requires one of: turn off, remap, or active-only-on-focus.

**Scope correction.** The audit names only `f`, but the reading surface has **three**
printable single-key shortcuts, and 2.1.4 covers every character key:

| Key | Site | Action |
|-----|------|--------|
| `f` | [`03-focus-mode.js:80`](../../../crates/core/assets/js/code-enhance/03-focus-mode.js#L80) | focus mode + fullscreen |
| `?` | [`07-keyboard.js:31`](../../../crates/core/assets/js/code-enhance/07-keyboard.js#L31) | open settings |
| `/` | [`07-keyboard.js:38`](../../../crates/core/assets/js/code-enhance/07-keyboard.js#L38) | open search |

A fix covering only `f` would leave 2.1.4 unmet. Esc and the arrow keys are non-printable,
so they are out of scope and stay unconditional.

**Approach.** WCAG's own sanctioned remedy is the turn-off mechanism, so the good default
survives intact: `f` keeps doing focus mode **and** fullscreen, and a reader who never opens
settings sees no change at all. This is why the owner rejected decoupling fullscreen from
the `f` key: it would have made one feature behave two different ways depending on trigger.

- **`taliShortcutsOn()`**, a new shared accessor in
  [`01-registry.js`](../../../crates/core/assets/js/code-enhance/01-registry.js), beside the
  existing cross-fragment helpers (`taliCopyText`, `taliAnchorUrl`, `taliCloneStripped`).
  Defaults to **on**, and returns `true` on any storage throw (a blocked-storage reader must
  not silently lose their shortcuts).
- **Storage key: `tali-shortcuts`.** This deliberately does *not* match its two siblings,
  `qmd-theme` ([`theme.rs:94`](../../../crates/core/src/render/theme.rs#L94)) and
  `qmd-deck-theme`, which are the only localStorage keys in the codebase and both still carry
  the retired `qmd-` prefix. Those are **frozen**: a storage key has no aliasing mechanism, so
  renaming one would silently reset every existing reader's saved preference. A brand-new key
  carries no such burden and should not adopt a dead prefix. Record this reasoning in a
  comment at the accessor, or the mismatch reads as an oversight and gets "fixed" later.
  - *Why the registry, not `14-reader-prefs.js`:* `01` is concatenated first and is also
    inlined standalone as `REGISTRY_JS` at
    [`page.rs:231`](../../../crates/core/src/render/page.rs#L231). Function declarations
    hoist across the concatenated shared scope, so `03` and `07` can call it at keydown time
    regardless of fragment order.
- **The toggle UI** goes in
  [`14-reader-prefs.js`](../../../crates/core/assets/js/code-enhance/14-reader-prefs.js),
  reusing that file's existing `seg()` helper (its `role="group"` + `aria-label` +
  `aria-pressed` sync) so the row renders identically to the Theme row.
- **Consumers** gate on `taliShortcutsOn()` at keydown: the `f` branch
  (`03-focus-mode.js:80`) and the `?` / `/` branches (`07-keyboard.js:31,38`).
- **The cheatsheet follows the state.** `addSection` already returns a `{setVisible}` handle
  ([`13-reader-menu.js:98`](../../../crates/core/assets/js/code-enhance/13-reader-menu.js#L98)),
  so `07`'s "Keyboard shortcuts" list hides when shortcuts are off rather than advertising
  dead keys.

**No lockout.** With shortcuts off, `?` no longer summons settings, but the gear launcher is
always visible (docked in the navbar, or floating on a chrome-less doc), so the toggle is
always reachable to switch back on.

**Minimal-config clearance.** This is a reader-local a11y preference, which CLAUDE.md
exempts by name ("Reader-local a11y preferences (theme, text size, spacing) are exempt, they
are personal, not document config"). It adds no document config and no `_site.yml` knob.

### 2. Settings popover never takes focus on open (med)

**Symptom.** [`openMenu()`](../../../crates/core/assets/js/code-enhance/13-reader-menu.js#L60)
sets `panel.hidden = false` and moves nothing; Esc *does* restore focus to the launcher
([:76-79](../../../crates/core/assets/js/code-enhance/13-reader-menu.js#L76)). The asymmetry
is the bug.

**Why the existing comment is wrong.** The file twice records a deliberate refusal to move
focus ([:38-39](../../../crates/core/assets/js/code-enhance/13-reader-menu.js#L38),
[:56-58](../../../crates/core/assets/js/code-enhance/13-reader-menu.js#L56)): "A DISCLOSURE,
not a dialog... does NOT trap or move focus." That rule is correct for a disclosure, but it
assumes the panel **follows its trigger in DOM order** so a reader can simply Tab into it.
That assumption is false here: the panel is appended to the end of `<body>`
([:46](../../../crates/core/assets/js/code-enhance/13-reader-menu.js#L46)) while the gear
sits in the navbar, so a keyboard user must Tab through the entire page to reach what they
just opened.

The comment's other worry stays valid and is **not** contradicted: it objects to a full
`taliFocusTrap` fighting the light-dismiss, not to a one-time focus move. No trap is added.

**Approach.** `openMenu()` focuses the panel's first focusable control. Esc-restore already
exists and closes the loop. The stale comment is rewritten to record *why* the disclosure
rule does not apply here, so this is not "corrected" back later. If
[`04-focus-trap.js`](../../../crates/core/assets/js/code-enhance/04-focus-trap.js) already
exports a focusable-element selector, reuse it rather than writing a second one.

### 3. Category-filter chips expose state only visually (med)

**Symptom.** [`10-category-filter.js:29`](../../../crates/core/assets/js/code-enhance/10-category-filter.js#L29)
toggles `tali-cat-active` and nothing else; the file contains no `aria-pressed` and no
`aria-live`. The server's initial chips carry no `aria-pressed` either
([`site/mod.rs:1146`](../../../crates/core/src/site/mod.rs#L1146) for All,
[:1150](../../../crates/core/src/site/mod.rs#L1150) for the rest).

**Approach.** Three coupled edits, server and client shipping together:
- Server emits `aria-pressed="true"` on the All chip and `"false"` on each category chip, so
  the initial paint is correct before any JS runs.
- The client mirrors `aria-pressed` beside the existing class toggle at `:29`.
- A visually-hidden `aria-live="polite"` node announces the result count ("Showing 4 of 12
  posts"), reusing the `tali-sr-only` class that `03-focus-mode.js:11` already uses for the
  same purpose.

### 4. Citation/xref link preview is hover-only (low)

**Symptom.** [`12-link-preview.js:159`](../../../crates/core/assets/js/code-enhance/12-link-preview.js#L159)
and [:163](../../../crates/core/assets/js/code-enhance/12-link-preview.js#L163) bind only
`mouseover`/`mouseout`; there is no `focusin` anywhere in the file, so a keyboard reader can
never surface a preview card.

**Approach.** Bind `focusin`/`focusout` beside the existing mouse pair (same delegated
handler, same show/hide path), and set `aria-describedby` on the link while its card is open
so the card is announced rather than merely painted. The existing Esc `forceHide`
([:191](../../../crates/core/assets/js/code-enhance/12-link-preview.js#L191)) already covers
dismissal.

### 5. `forced-color-adjust: none` hides the current nav item (low)

**Symptom.** Under Windows High Contrast, two rules pin a foreground with no background on
the active nav item, so an opposite-polarity OS theme can paint it invisible:
- [`base.css:868`](../../../crates/core/assets/css/base.css#L868):
  `.tali-reader-seg button[aria-pressed="true"], .tali-nav-active, a[aria-current="page"] { forced-color-adjust: none; }`
- [`site.css:312`](../../../crates/core/assets/css/site.css#L312): same opt-out on
  `.tali-nav-active, .tali-book-active, a[aria-current="page"]`.

(The audit cited `base.css:780` / `site.css:293`; the line numbers drifted, the rules are
unchanged.)

**Approach.** Drop the nav selectors from both opt-outs. Keep
`.tali-reader-seg button[aria-pressed="true"]`, which legitimately pins a **bg+fg pair** and
so is safe under forced colors. The "you are here" signal survives without the opt-out:
site.css's rule also sets `text-decoration: underline; text-underline-offset: 3px`, which
forced colors preserves.

### 6. Settings panel doesn't reflow at 200% text

**Symptom.** The content-loss half is already fixed (the `box-sizing`/`width` work recorded
at [`base.css:85-95`](../../../crates/core/assets/css/base.css#L85)). What remains: at 200%
the rows still scroll horizontally inside the panel's `overflow: auto`.
[`base.css:102`](../../../crates/core/assets/css/base.css#L102)
(`.tali-reader-row { display: flex; justify-content: space-between; }`),
[:105](../../../crates/core/assets/css/base.css#L105)
(`.tali-reader-seg { display: inline-flex; }`), and
[:254](../../../crates/core/assets/css/base.css#L254) (`.tali-keys-list > div { display: flex; }`)
all lack `flex-wrap`, so a doubled-size label plus a 4-button Theme seg cannot break onto a
second line.

**Approach: `flex-wrap`, not a breakpoint.** Wrapping responds to actual available space, so
there is no breakpoint to tune and it handles full-page zoom and text-only zoom alike (an
em-based media query would work but needs a tuned threshold per row). `space-between`
survives wrapping: each flex line distributes independently, so an unwrapped row keeps
today's look exactly and a wrapped one stacks left-aligned.

**Known cosmetic risk to check in-browser.** `.tali-reader-seg` sets `overflow: hidden` +
`border-radius: 7px` while its buttons use `border-left` as dividers
([:109-113](../../../crates/core/assets/css/base.css#L109)). A seg wrapped onto two lines
will show a stray leading divider on line 2. Verify and, if it reads badly, suppress the
divider on the first button of each line.

## Testing

**The server chip is the only automated-testable half, and it gets a failing test first.** A
Rust assertion that the rendered listing carries `aria-pressed` on the initial chips
(extending the existing `tali-cat` assertion at
[`tests/tech_blog.rs:299`](../../../crates/core/tests/tech_blog.rs#L299)). Mutation-check it
against the exact shape it guards before implementing, per the backlog's **gate the gate**
rule: a drift test that cannot fail is worse than none.

Everything else is CSS and DOM behavior with no harness (`crates/core/assets/js` is still
outside the `tsc` pass, itself an open Tier-2 item), so the sanctioned check is the
chrome-devtools browser loop:

| Item | Check |
|------|-------|
| 1 | Toggle off → `f`, `?`, `/` all inert; cheatsheet section hidden; gear still opens; pref survives reload; toggle on → all three work again |
| 2 | Open via gear **and** via `?` → `document.activeElement` is inside the panel; Esc → focus back on the launcher; click-away still dismisses |
| 3 | Chip click → `aria-pressed` flips; live region announces the count |
| 4 | Tab to a citation link → card shows; `aria-describedby` set while open; blur hides |
| 5 | forced-colors emulation → active nav item visible under both polarities; reader-seg pressed button still legible |
| 6 | 200% text → no horizontal scroll in the panel; rows stack; no stray seg divider |

All UI checks run the three-viewport matrix (390x844 mobile, 1440x900 laptop landscape,
900x1440 the narrow-tall band).

**Rebuild note.** `assets/css` and `assets/js` are `include_str!`-compiled, so the binary must
be rebuilt before a built site shows these changes. A live `preview` hot-swaps CSS.

## Scope guard

- Every new control is a **reader-local a11y preference** (CLAUDE.md-exempt); no document
  config, no `_site.yml` knob.
- No preview write-back; the `.tmd` file stays the single editing surface.
- No CDN, no new output format, `--tali-*` tokens only.
- The exec/kernel zone is untouched.
- Decks are untouched, and both deck items in this section turned out to be rot. The four
  fragments carrying reader chrome (`03`, `07`, `13`, `14`) each already early-return on
  `.tali-deck`, so the shortcuts toggle and the menu-focus change cannot reach a deck.
  `10-category-filter.js` and `12-link-preview.js` have **no** deck guard, which is correct
  and pre-existing (a deck has no category listing; link previews are wanted on slides), and
  neither edit here changes that reachability.

## Backlog outcome

Delete all eight §C bullets on landing: six built, two deleted as rot with the evidence
recorded above. That closes section C, leaving D (needs a direction ruling), E (own session),
and G (needs a priority ruling) as the open sections.
