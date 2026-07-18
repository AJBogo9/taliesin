# B4 — Deck visual-identity pass (Direction A: "Marginalia", serif titles)

Date: 2026-07-18. Backlog item **B4** (PMF audit). Branch `b4-deck-marginalia`.
Owner direction ruling 2026-07-18: **Direction A — Marginalia (serif titles)**.

## Goal

Give the deck the "Marginalia" editorial identity the website + book got in the 2026-07-11
audit, so a deck reads as *the same tool* as the blog. Today the deck is generic system
sans, no serif, no accent system, no section-divider treatment. This is a **visual** pass
only — no interaction/behavior change (the 2026-07-12 deck audit already owns bugs/reshape).

## Direction A (chosen)

Serif **Newsreader** titles (the blog's owned voice) + clean sans body (legibility for
code-heavy slides); iron-gall accent rule under the title; section dividers get a large
serif numeral + accent rule. The mono eyebrow shown in the mockup is **deferred** (it would
need a new `eyebrow:`/`date:` front-matter field threaded through `deck.rs`; minimal-config
says perfect the default first — the serif title + accent rule + subtitle is already a
complete designed title slide). Clean follow-up if wanted.

## Why this is (almost) pure CSS — the two free levers

Both identity levers already ship on every deck page and are simply unused by `deck.css`:
- **Newsreader** (the blog serif) is already inlined into every deck's `<style>` via
  `FONTS_CSS` (`deck.rs:75`), but `deck.css` never references it. Using it = pure CSS.
- The **iron-gall accent** is already `--deck-accent: #3a4673` (numerically the same hue as
  the site's `--tali-accent`), just re-declared.
- The **section-divider hook** already exists structurally: an h1-led slide emits
  `section.tali-slide[data-level="1"]` (`deck.rs` `group_slides`), today **completely
  unstyled**. Styling it needs no engine change.

So B4 is a `deck.css`-only change (plus a corpus exemplar + a regression pin). No `deck.rs`,
no new font, no new plumbing.

## Changes (`crates/core/assets/css/deck.css`)

1. **Serif head-font token.** Add `--deck-font-head: "Newsreader", ui-serif, Georgia,
   "Times New Roman", serif;` to the `.tali-deck` token block, and set
   `font-family: var(--deck-font-head)` on the heading rule (`h1..h4` + `h1.title`). A
   future theme overrides by redefining the token. Body/lists/code stay `--deck-font` (sans).
   Tune serif tracking (less negative than the sans `-0.02em`; ~`-0.01em`).

2. **Title slide.** `h1.title` inherits the serif; keep it large + centered (don't fight the
   existing `.center` flex layout). Add an **iron-gall accent rule** beneath it via a
   pseudo-element: `section.tali-title-slide h1.title::after { content:""; display:block;
   width: 2.2em; height: 3px; background: var(--deck-accent); margin: .4em auto 0; }`.
   Refine the subtitle (spacing/tracking), keep it muted sans.

3. **Section divider** (`section.tali-slide[data-level="1"]`, an h1-led slide). A distinct,
   centered treatment: vertical+horizontal centering (mirror the `.center` flex), a large
   **serif numeral** from a CSS counter (`counter-reset: tali-section` on `.tali-slides`;
   `counter-increment` on each `[data-level="1"]`; `::before` shows
   `counter(tali-section, decimal-leading-zero)` big + accent/muted), the serif h1, and an
   accent rule. Pure CSS; the counter is document-order so numbering is deterministic.

4. **Accent system.** Keep the existing accent list markers; ensure the accent rule/numeral
   read in **both** light and dark (the dark deck already mirrors the accent as `#9aa8dc`,
   `html.tali-deck-dark`). Add a `forced-colors` fallback for the new rules.

## Pin (corpus) + verification

- **Themed exemplar** `corpus/deck-marginalia.tmd`: a small `format: deck` with a
  title+subtitle title slide, a couple of content slides (bullets + a short code block), and
  **two** section dividers (h1) to prove the divider treatment + numeral sequence. No `{js}`
  (keeps it out of `body_html_snapshots`). This is the persistent identity showcase +
  browser-verify target. `corpus/deck.tmd` stays unchanged (it already exercises the title
  slide, a section divider, code, columns — it is the regression net; avoid snapshot drift).
- **Regression pin** (render unit test): assert `DECK_CSS` carries the identity — the
  `Newsreader`/`--deck-font-head` serif on headings, the `[data-level="1"]` section-divider
  rule, and the title accent rule — so a future edit can't silently strip the identity.
- **Browser verify** (chrome-devtools, the project's UI loop): title slide, a content slide,
  and a section divider, in **light + dark**, at presentation aspect (1280×720). Confirm the
  serif renders (Newsreader, not a fallback), the accent rule + section numeral show, legible
  contrast, no console errors. Then show the owner for refinement.

## Out of scope / deferred

- **Mono eyebrow** on the title slide — deferred (needs a small `deck.rs` front-matter
  field). A clean follow-up.
- No interaction/behavior/layout-engine change; no reveal vocabulary; the mobile slide-feed
  (`calc(100vw/960*40)`) inherits the new type identity automatically (same `.tali-deck`).
- Ship as a **rewrite of the default `deck.css` in place** (there is one deck identity and no
  `theme:` in use) — not an opt-in theme file. Minimal-config: perfect the default.

## Invariants honored

All offline (Newsreader already inlined); `--deck-*`/accent tokens only; no engine/DOM
change; deterministic output; the deck's frozen runtime names (`qmd-deck-theme`,
`window.TaliesinDeck`) untouched.
