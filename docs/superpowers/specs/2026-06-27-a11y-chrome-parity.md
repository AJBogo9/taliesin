# a11y chrome parity (Lane B)

2026-06-27 release-hardening pass. The live-preview client already has a client-side
a11y *audit* panel, but the rendered chrome has gaps. This lane closes the touch +
a11y parity holes in the chrome that the audit can't fix for itself.

## Goals

1. **Distinguishing `aria-label`s on nav landmarks.** A page can carry several
   `<nav>` landmarks (TOC, book post-nav, category filter, etc.). Each gets a
   distinct accessible name so a screen reader's landmark list is navigable.
2. **One shared `:focus-visible` ring.** A single CSS token (`--qmd-focus`) drives a
   consistent keyboard-focus outline across links, buttons, tabs, reader-menu
   controls, and deck controls, replacing the absence of any global ring (only a few
   ad-hoc per-component rings existed).
3. **Focus management audit for `role=dialog`.** Every `role=dialog` either traps +
   restores focus (modal: lightbox, Cmd-K, keyboard sheet) or is a deliberate
   light-dismiss popover (reader menu) that moves focus back to its trigger on Esc.
   The lightbox dialog was missing an accessible name; add it. Everything else
   already conforms; documented here so the invariant is explicit.
4. **Slide roles + "Slide N of M".** Deck `<section class="qmd-slide">`s get
   `role="group"` + `aria-roledescription="slide"` server-side (additive, no block-id
   shift), and deck.js labels each leaf `aria-label="Slide N of M"` + announces the
   current slide through a polite live region on navigation.
5. **forced-colors / prefers-contrast.** `@media (forced-colors: active)` and
   `prefers-contrast: more` rules keep borders, focus rings, and icon glyphs visible
   in high-contrast / Windows High Contrast mode.
6. **Static-build-vs-runtime divergence.** The skip-to-content link + a focusable
   content container exist server-side in `page.rs` now, so they work with JS off; the
   runtime `qmdInitSkipLink` becomes idempotent over the server-rendered markup.
   (Images already emit real `alt` server-side — verified, no change needed there;
   the `img:not([alt])` audit fires on genuinely alt-less images.)

## Scope / ownership

Owned files only:

- `crates/core/assets/css/{base,dark,deck,site}.css` — focus token, forced-colors,
  prefers-contrast.
- `crates/core/assets/js/deck.js` — slide `aria-label` + live announcement.
- `crates/core/assets/js/code-enhance.js` — lightbox `aria-label`; `qmdInitSkipLink`
  made idempotent over server markup.
- `crates/core/src/render/page.rs` — server-side skip-link + focusable `<main>`,
  TOC nav `aria-label` already present (`role=doc-toc`); main wrapper id/tabindex.
- `crates/core/src/render/deck.rs` — slide `role`/`aria-roledescription` on
  `<section>` (byte-stable: attributes added to the `<section>` open tag, never to an
  inner `[data-block-id]` block).
- `docs/guide/using/reading.qmd` — document the a11y / keyboard surface.
- `crates/core/tests/corpus.rs` — extend the existing nav-a11y assertion.

Explicitly NOT touched (other lanes / do-not-edit): `render/mod.rs` (TOC builder —
already `role=doc-toc`), `site/chrome.rs` (navbar/sidebar/footer — already labelled
`qmd-site-nav`/`qmd-book-sidebar`/burger aria), `emit.rs`/`figure.rs` (image alt —
already real), `qmd-js.js`, `mermaid.js`, `tabset.js`, `walkthrough.js`, `scrolly.js`.

## Load-bearing invariant guard

Slide roles are appended to the `<section class="qmd-slide">` open tag in
`render_section`, alongside the existing `id`/`data-level`/`data-background-*`. The
`<section>` carries **no** `data-block-id`; the per-block hashes live on the inner
blocks (`s.blocks`), which this change never touches. So no block id shifts and no
`data-sourcepos` moves. The corpus invariant test (`crates/core/tests/corpus.rs`)
re-runs to confirm.

## Server-side skip-link + main (page.rs)

The static/site/book layouts already wrap content in `<main>` only when a TOC exists.
Make the skip-link + a focusable `<main id="qmd-main">` server-side so they work with
JS off:

- Inject a `<a class="qmd-skip" href="#qmd-main">Skip to content</a>` as the first
  body child, and ensure the primary content container is a `<main id="qmd-main"
  tabindex="-1">`.
- The runtime `qmdInitSkipLink` already no-ops when a `.qmd-skip` / focusable main is
  present (guards added); on a live preview (`#qmd-root` mount) it still creates them.
- Decks emit their own chrome and are skipped (the deck page has no skip-link).

## Verification

- Server-side: corpus assertion (skip-link present, `<main id="qmd-main"
  tabindex="-1">`, TOC `aria-label`/`role=doc-toc`, slide `role=group` +
  `aria-roledescription="slide"`). Failing-first, then green.
- Browser (coordinator): focus-ring consistency across links/buttons/tabs/reader
  controls/deck controls; forced-colors mode; "Slide N of M" announcement; dialog
  focus open/restore (lightbox, Cmd-K, keyboard sheet) + reader-menu Esc restore;
  skip-link reachable with JS disabled. At 390 / 900 / 1440 px.
</content>
</invoke>
