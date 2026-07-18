# DX2 — First-run in-preview hint (surface click-to-source + `?`)

Date: 2026-07-18. Backlog item **DX2** (§6 DX audit batch, Tier 1 discoverability family).
Branch `dx2-first-run-preview-hint`. Detail source: `notes/2026-07-18-dx-audit.md`.

## Goal

Surface Taliesin's flagship gesture — **Alt-click any block to jump to its source** — in the
live preview, where the author actually is. Today the gesture works and even carries a hint, but
that hint lives *inside the collapsed `◇</>` dev panel* ([`client.js:411-413`](../../../web-client/client.js)),
so both the blogger and speaker personas in the audit "would have shipped never knowing
click-to-source existed." DX2 adds a one-time, dismissible, `localStorage`-gated callout tethered
above the `◇</>` button that names click-to-source and the `?` shortcuts menu, then gets out of
the way forever. Highest discoverability-per-line item in the audit.

## Ground truth (grepped + read against source 2026-07-18, before pricing)

Per the backlog's own law — *grep the named symbol first; the audit's S/M/L are guesses; trust the
symptom, not the cause.* What the code actually shows:

- **The dev-preview client is inherently preview-only.** [`web-client/client.js`](../../../web-client/client.js)
  is injected on both serve paths (single-doc [`serve/mod.rs:691`](../../../crates/server/src/serve/mod.rs),
  deck [`serve/mod.rs:737`](../../../crates/server/src/serve/mod.rs), site
  [`serve_site/mod.rs`](../../../crates/server/src/serve_site/mod.rs)) and **never** in `build`.
  A hint built here can never leak into built output. ✓ correct surface.
- **The `◇</>` dev button is always visible** bottom-left in preview, but the "Alt-click a block"
  text sits inside the collapsed panel ([`client.js:411-413`](../../../web-client/client.js)),
  hidden until the user clicks `◇`. **Symptom confirmed.** No surfaced first-run nudge exists →
  DX2 is a real gap, and it's genuinely small net-new chrome, not pure wiring like DX1.
- **Alt-click already works and self-advertises while held** (dashed outline, pointer cursor) via
  the document-level handler at [`client.js:1225-1237`](../../../web-client/client.js). DX2 does
  **not** touch this behavior; it only points at it.
- **`?` opens the reader Settings menu** (which lists the shortcuts) via
  [`07-keyboard.js`](../../../crates/core/assets/js/code-enhance/07-keyboard.js). It works on
  single-doc *and* site previews (floating-gear fallback in
  [`13-reader-menu.js`](../../../crates/core/assets/js/code-enhance/13-reader-menu.js)), but is
  **skipped on decks** (`.tali-deck` early-return in both files) and is gated by
  `taliShortcutsOn()` (WCAG 2.1.4 turn-off). So the `?` line is a **dead key on a deck / when
  shortcuts are off** and must be omitted there — matching the existing "don't advertise dead
  keys" discipline (`hasSearch`/`hasChapters` in `07-keyboard.js`).
- **Deck previews DO mount the dev client + `#tali-controls`** ([`serve/mod.rs:764`](../../../crates/server/src/serve/mod.rs)),
  so the callout can anchor there on decks too; only the `?` line is dropped.
- **Dev-menu CSS lives in one shared server-side const**, `STATUS_CSS`
  ([`serve/mod.rs:528`](../../../crates/server/src/serve/mod.rs)), injected verbatim by *both*
  serve paths (`serve/mod.rs:674,720`; `serve_site/mod.rs:639`). Hint CSS added there ships on
  every preview surface with no duplication.
- **`localStorage` convention:** new keys use the owned `tali-` prefix
  ([`01-registry.js:95-99`](../../../crates/core/assets/js/code-enhance/01-registry.js)); the
  `qmd-*` keys are frozen and must not be imitated.

Net: DX2 is a small, self-contained addition to `client.js` + a CSS block in `STATUS_CSS`. No new
server routes, no protocol change, no build-path change.

## Resolved decisions (owner, 2026-07-18)

1. **Form: a callout tethered to the `◇</>` button.** Anchored just above the collapsed button
   with a downward tail pointing at it (Astro dev-toolbar pattern). Ties the discovery to the
   persistent affordance, so after dismissal the author knows where the dev tools live. (Chosen
   over a pulse-ring tag and a top-center banner.)
2. **Deck behavior: show, live lines only.** On a deck the callout shows the Alt-click line alone
   (the `?` line is omitted as a dead key). Everywhere else both lines show. Keeps click-to-source
   discoverable for the speaker persona without advertising a dead key.
3. **Dismissal (persists on any):** clicking "Got it"; the first Alt-click that resolves to a
   block (they discovered the gesture); opening the `◇` menu (they found the tools); or Esc.
4. **One-time, per-browser,** via `localStorage`. Storage failures **fail closed** (treat as seen
   → never show), so a private-mode reader can never get an un-dismissable nag.

## Changes

### A. `web-client/client.js` — build + wire the nudge

All additions live inside the existing top-level IIFE (the file is one deliberate IIFE and stays
whole — see the JS-modularization note; do **not** split it).

- **Storage helpers** (module-local, try/catch-wrapped, fail-closed):
  - `hintSeen()` → `true` if `localStorage.getItem('tali-hint-seen')` is set **or** storage throws.
  - `markHintSeen()` → best-effort `setItem('tali-hint-seen','1')`, swallow throws.
- **Build** inside `buildDevMenu()` (has the `#tali-controls` `host` in scope), only if
  `!hintSeen()`:
  - A `<div class="tali-hint-nudge" role="status" aria-live="polite" hidden>` appended to `host`
    (sibling of `toggle`/`panel`, so the shared `position:absolute` anchor lands it above the
    button).
  - Line 1 (always): `Alt`-click any block to open its source, with a `<kbd>Alt</kbd>` (title
    notes "Option on Mac" to match the existing `srcHint` copy).
  - Line 2 (only if `askLive`): "Press `<kbd>?</kbd>` for keyboard shortcuts", where
    `askLive = !document.querySelector('.tali-deck') && (typeof window.taliShortcutsOn !== 'function' || window.taliShortcutsOn())`.
  - A `<button type="button" class="tali-hint-dismiss">Got it</button>`.
  - **Reveal** after a short `setTimeout` (≈400 ms so it eases in after first paint; content is
    already server-rendered, so no need to await a `full_render`): unset `hidden`.
- **A single `dismiss()`** closure: hide + remove the node, call `markHintSeen()`, idempotent
  (guarded so the three dismissal hooks can't double-fire or throw after removal).
- **Wire the four dismissal triggers:**
  - "Got it" button → `dismiss()`.
  - The `◇` toggle's existing click handler ([`client.js:392`](../../../web-client/client.js)) →
    also `dismiss()` (only affects a still-showing nudge).
  - The Alt-click handler ([`client.js:1225`](../../../web-client/client.js)) → after a
    successful `locatable()` resolve (i.e. right where it already calls `openSource(el)`), call
    `dismiss()`.
  - A `keydown` for `Escape` while the nudge is showing → `dismiss()`.
- **Reduced motion:** the ease-in is a CSS transition gated by
  `@media (prefers-reduced-motion: reduce)` (no animation there); JS just toggles `hidden`.

Because `buildDevMenu()` runs once at client init and the nudge is created only when `!hintSeen()`,
there is nothing to tear down on reconnect and no per-render cost.

### B. `crates/server/src/serve/mod.rs` — `STATUS_CSS`

Append a `.tali-hint-nudge` block to the existing `STATUS_CSS` const:

- `.tali-hint-nudge` — `position:absolute; bottom:calc(100% + .45rem); left:0;` (the same anchor
  as `.tali-dev-panel`), small card matching the panel's `--tali-bg`/`--tali-border`/shadow/radius,
  `max-width:15rem`, comfortable `line-height`, `font:12px` matching the dev menu.
- A downward tail (`::after`, a bordered triangle) pointing at the `◇` button.
- `.tali-hint-nudge[hidden]{display:none}` and a `transition: opacity/transform` for the ease-in,
  disabled under `@media (prefers-reduced-motion: reduce)`.
- `.tali-hint-nudge kbd` — the small key-cap styling (borrow the reader `.tali-keys-list kbd`
  look; inline it, since `STATUS_CSS` is self-contained).
- `.tali-hint-dismiss` — a small text button (reuse `.tali-dev-ctl` tokens, right-aligned).

No change to `serve_site/mod.rs` (it already injects the same `STATUS_CSS`).

### C. Client / protocol — no change

No websocket message, no server render of the nudge, no build-path emission. The nudge is
constructed entirely client-side and only ever *navigates* (via the existing Alt-click), never
writes source.

## Testability & verification

DX2 is preview **chrome**, not a rendered-doc capability. Like the dev menu itself — and like DX1,
which added no corpus doc — it does **not** ship a corpus pin (the corpus is rendered *output*; the
dev client is never in output). Verification is three-legged:

- **Rust regression pin (unit test in `serve/mod.rs` `#[cfg(test)]`):** assert `STATUS_CSS`
  contains `".tali-hint-nudge"` (the CSS ships), and that a rendered single-doc preview page and
  the site body both contain the `#tali-controls` host the nudge mounts into. **Mutation-check:**
  delete the appended CSS block → the named test fails → revert. (Guards against a future edit
  silently dropping the style; the existing `index_html`/deck tests at `serve/mod.rs` ~L1646 and
  the `serve_site` body test are the models.)
- **Client type-check:** `cd web-client && npx -y -p typescript tsc -p jsconfig.json` stays clean
  (`// @ts-check`).
- **Browser check (the sanctioned loop for `client.js` UI, per CLAUDE.md):** chrome-devtools MCP
  across the viewport matrix (mobile ~390×844, laptop ~1440×900, portrait ~900×1440) on **three**
  surfaces — single-doc (`preview corpus/<doc>.tmd`), site (`preview <dir>`), and **deck**
  (`preview corpus/deck.tmd`). Confirm: (1) the callout appears once above `◇` with the correct
  lines per surface (both off-deck, Alt-click-only on the deck); (2) each of the four dismissals
  hides it and persists; (3) a reload after dismissal stays quiet; (4) clearing
  `localStorage['tali-hint-seen']` brings it back; (5) no console errors; (6) it never appears in
  `build` output (grep the built HTML for `tali-hint-nudge` → absent).

## Non-goals (explicitly out of scope)

- **No change** to the `?`/reader-menu or Alt-click behavior themselves — DX2 only surfaces them.
- **No general onboarding tour / product-tour** — a single first-run callout, nothing more.
- **No server-rendered or build-emitted** nudge — preview-only, JS-built.
- **DX3 / DX10** (schema auto-wire, teaching scaffolds) are separate items.

## Invariant safety

Preview-only (never in `build`); no preview write-back (the nudge only navigates via the existing
Alt-click — the single-editing-surface invariant holds); no new output format, no CDN; `--tali-*`
variables + existing DOM/CSS only. The `data-block-id` + `data-sourcepos` block model and the
`MAX_WARM_PAGES` + `exec_pool.rs` eviction freeze are untouched — DX2 adds one sibling node to
`#tali-controls` and one CSS block to `STATUS_CSS`.
