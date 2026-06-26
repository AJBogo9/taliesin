# Reader polish bundle (design)

Date: 2026-06-26
Status: approved (brainstorm), pre-implementation
Feature branch: `feat/reader-polish`
Cluster: reader-experience (FEATURE-IDEAS.md #16 text-wrap, #19 skip-link, #23
widow/orphan, #55 keyboard reader).

## Summary

Three small, low-risk reader/a11y/typography wins, all client-side + CSS (no Rust /
render change):

1. **Typography polish** (CSS): `text-wrap: pretty` on prose, `balance` on
   headings/captions, widow/orphan control, and figure/caption keep-together.
2. **Skip-to-content link** (enhancer + CSS): a visually-hidden-until-focused link
   that jumps keyboard/screen-reader users straight to the content.
3. **Keyboard reader** (enhancer + a one-line search export): `?` opens a shortcuts
   cheatsheet, `/` opens search, `←`/`→` move to the previous/next chapter.

Each rides the `qmdEnhancers` registry, is deck-skipped, and is idempotent. No new
runtime dependency.

## Goals

- Prose wraps prettily (no orphaned last words) and headings/captions balance; a
  figure caption never strands from its figure.
- A keyboard or screen-reader user can skip the chrome and land on the content in one
  keystroke.
- Power readers get `?` (help), `/` (search), and `←`/`→` (prev/next chapter), each
  guarded so it never fires while typing or while a modal is open.
- All offline, all reusing existing machinery (`qmdFocusTrap`, the book prev/next
  anchors, the Cmd-K search palette).

## Non-goals (v1, YAGNI)

- **No hyphenation toggle** (can hurt dyslexic readers; deferred).
- **No `forced-colors` / `prefers-contrast` pass** (deferred).
- **No `g`/index or other shortcuts** beyond `?`, `/`, `←`, `→`.
- **No Rust / render / page-template change.** The skip link is a client enhancer
  because the content wrapper varies across build/site/preview; this is one robust
  path, consistent with the all-enhancers architecture.
- **No new corpus doc** (these are global chrome behaviors; the guide book is the
  test surface).

## Invariants honoured

- **Read-only:** navigation + focus only; never writes source.
- **HTML-only, offline:** CSS + two JS files; no dependency, no network.
- **No core change:** `crates/core/assets/css/base.css`,
  `crates/core/assets/js/code-enhance.js`, `web-client/search.js` only.
- **Deck-skipped** (a `.qmd-deck` page has its own chrome + keys).
- **Reuses** `window.qmdFocusTrap` (modal), `.qmd-book-prev`/`.qmd-book-next` (nav),
  and the search palette's existing `open()`.

## 1. Typography polish (`base.css`)

Applied globally (NOT scoped to `main`/`#qmd-root`): unlike line-height, `text-wrap`
and orphan/widow control have no visible effect on chrome (nav/TOC items are short, so
`pretty` is a no-op there), and global scoping is the only thing that also covers
no-TOC build pages (which have no `<main>` wrapper). Add near the existing prose rules:

```css
  /* Reading-typography polish. Global: text-wrap/orphans are no-ops on short chrome. */
  p, li { text-wrap: pretty; orphans: 2; widows: 2; }
  h1, h2, h3, h4, h5, h6, figcaption, blockquote, .callout-title { text-wrap: balance; }
  figure, .qmd-figure { break-inside: avoid; }
  figcaption { break-before: avoid; }
  h1, h2, h3, h4, h5, h6 { break-after: avoid; }
```

`text-wrap: balance` is browser-capped to a few lines, so it is safe on long
headings; `pretty`/`balance` degrade to normal where unsupported. Pure CSS, additive,
no markup or JS. (Unlike the line-spacing controls, this needs no chrome re-pin: a
prettier last line in a nav/TOC item is invisible.)

## 2. Skip-to-content (`qmdInitSkipLink` in `code-enhance.js` + `base.css`)

An idempotent enhancer (guard `window.__qmdSkipLink`; deck-skip):

- Find the content container, in order: `document.querySelector('main')`,
  else `document.getElementById('qmd-root')`, else the first `[data-block-id]` block.
  Give it `id="qmd-main"` (only if it has none) and `tabindex="-1"`.
- Prepend `<a class="qmd-skip" href="#qmd-main">Skip to content</a>` as the first
  `<body>` child (once).
- On click/activate, also call `target.focus()` so focus (not just scroll) moves to
  the content (keyboard users continue from there).

CSS: visually hidden until focused, then pinned top-left:

```css
  .qmd-skip { position: fixed; top: -3rem; left: .5rem; z-index: 2147482000;
    background: var(--qmd-fg); color: var(--qmd-bg); padding: .5rem .8rem;
    border-radius: 6px; font: 600 .9rem var(--qmd-font-head); text-decoration: none;
    transition: top .15s ease; }
  .qmd-skip:focus { top: .5rem; }
  @media (prefers-reduced-motion: reduce) { .qmd-skip { transition: none; } }
```

## 3. Keyboard reader (`qmdInitKeyboard` in `code-enhance.js` + a `search.js` export)

### Search export (`web-client/search.js`)

Inside the IIFE, after `function open()` is defined, add:

```js
  window.qmdOpenSearch = open;
```

So any page that ships the search palette (TOC pages) exposes a programmatic opener.
No behavior change to Cmd-K.

### The enhancer (`code-enhance.js`)

An idempotent enhancer (guard `window.__qmdKeyboard`; deck-skip) with one
document-level `keydown` listener, mirroring the focus-mode guard:

```text
on keydown e:
  t = e.target
  typing = t is INPUT/TEXTAREA/SELECT or t.isContentEditable
  modal  = document.querySelector('[aria-modal="true"]')   // cheatsheet, Cmd-K, lightbox
  if e.key == '?' (and not typing, no Ctrl/Cmd/Alt):
      e.preventDefault(); toggleCheatsheet()                // allowed even if cheatsheet open (to close)
      return
  if typing or modifier held: return
  if e.key == 'Escape' and cheatsheet open: closeCheatsheet(); return
  if modal: return                                          // don't hijack keys under other modals
  if e.key == '/': if window.qmdOpenSearch { e.preventDefault(); window.qmdOpenSearch() }
  if e.key == 'ArrowRight': click '.qmd-book-next' if present
  if e.key == 'ArrowLeft':  click '.qmd-book-prev' if present
```

Notes:
- `?` is `Shift+/`; checking `e.key === '?'` handles the shift. It toggles even when
  the cheatsheet modal is open (so `?` closes it), but is still blocked while typing.
- Arrow nav follows the existing book anchors: `var a =
  document.querySelector('.qmd-book-next'); if (a) { e.preventDefault();
  window.location.assign(a.href); }`. No-op on single docs / non-book sites.
- Guard against a focused interactive control: the `typing` check covers
  INPUT/TEXTAREA/SELECT; also treat a focused link/button as "don't nav" by checking
  `t.closest('a,button,[role="tab"],input,select,textarea')` for the arrow keys, so an
  in-content slider or tablist keeps its arrows.

### The cheatsheet overlay

Built once (lazily on first `?`). A modal:

```html
<div class="qmd-keys" role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
  <div class="qmd-keys-card">
    <h2>Keyboard shortcuts</h2>
    <dl> … <kbd>?</kbd> Show this help · <kbd>/</kbd> Search ·
          <kbd>f</kbd> Focus mode · <kbd>←</kbd>/<kbd>→</kbd> Previous / next chapter ·
          <kbd>Esc</kbd> Close … </dl>
    <button class="qmd-keys-close" aria-label="Close">×</button>
  </div>
</div>
```

- Focus-trapped with `window.qmdFocusTrap(card, closeButton)` (the shipped helper);
  release on close. Esc and the × button and a backdrop click close it. Re-`?` closes.
- Styled in base.css (centered card, backdrop, dark/sepia aware via `--qmd-*`,
  reduced-motion aware). `<kbd>` styling.

## Testing

1. **Typography CSS ships** (Rust render test): a rendered page's HTML contains
   `text-wrap: pretty` (and `text-wrap: balance`), proving the bundled CSS includes
   the rules. (Add to `render/tests.rs`.)
2. **Client type-check / syntax**: `node --check` on `code-enhance.js` and
   `search.js`; keep the enhancers warning-clean.
3. **Browser (chrome-devtools)** on the live **guide book** (`preview docs/guide`,
   which has a TOC, search, and chapters with prev/next):
   - Tab from the top reveals the `Skip to content` link; activating it moves focus to
     `#qmd-main`.
   - `?` opens the cheatsheet (focus-trapped: Tab cycles inside; Esc closes; `?`
     toggles closed).
   - `/` opens the search palette.
   - `→` navigates to the next chapter; `←` back.
   - Typing `/` or `?` inside the search input does NOT trigger the shortcuts (guard).
   - 0 console errors.

## Risks & mitigations

- **Bare `←`/`→` surprising / conflicting**: guarded against typing, modals,
  modifiers, and a focused interactive element (slider/tablist keep their arrows);
  no-op outside books. Matches mdBook/Bookdown convention.
- **Prose CSS leaking into chrome** (the line-spacing lesson): prose rules are scoped
  to `main`/`#qmd-root`; `balance` on headings/captions is intended page-wide and is
  harmless on chrome headings.
- **Skip-link target varies by mode**: the fallback chain (`main` → `#qmd-root` →
  first block) always resolves; the enhancer sets the id only if absent.
- **`?` under the cheatsheet's own modal**: handled first, before the generic modal
  guard, so it can close itself.
- **JS skip link (not server markup)**: consistent with the all-enhancers
  architecture (the built page runs enhancers on load); it is the one path that works
  across build/site/preview without template surgery.

## Out of scope follow-ups (recorded, not built)

- Hyphenation reader-pref toggle (#17).
- `forced-colors` / `prefers-contrast: more` support (#20).
- Hanging punctuation / optical margins (#22).
- A `g` index shortcut, or making the cheatsheet rows adapt to which shortcuts apply.
