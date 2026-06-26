# Reader anchor + copy-link on hover — design

> Status: shipped (2026-06-26, `feat/reader-anchor-copy-link` → main). Hovering a
> heading or numbered float reveals a `#`; clicking it copies the canonical deep link
> to that anchor. Browser-verified (chrome-devtools): headings + figure-caption anchors
> with correct href/aria-label, hover/focus reveal, click copies the absolute `#id` URL
> + flashes ✓ + sets the hash + announces "Link copied"; no console errors.

## Problem

The selection toolbar's Share action gives a `#:~:text=` link to an arbitrary passage,
and the hover-cards (`qmdInitLinkPreview`) preview a reference's target — but there is no
way to grab the *canonical* link to a section or figure. Every docs site (Docusaurus,
Stripe, MDN) reveals a `#` on a heading so a reader can hand someone "this exact
section". qmd-fast already assigns stable anchor ids to every heading and numbered float;
this surfaces them.

## Goal

A reader-side affordance: hovering a content anchor reveals a small `#`; activating it
copies the absolute deep link (`<page-url>#<id>`) and flashes feedback. Complements the
Share action (canonical *section/figure* anchor vs arbitrary *text fragment*) and the
hover-cards (grab the link vs preview the target).

**Scope (v1):** headings (`h1`–`h6` carrying an `id` — which excludes the title block's
`<h1 class="title">`, that has none) and numbered floats (figure / table / listing — the
`#` goes inside the `<figcaption>`). **Deferred:** equations (the display-math `(n)` tag
placement is fiddly) and callouts (low value).

Reader-side, read-only: clipboard only, no source write, no `localStorage`. Idempotent
enhancer; deck-skipped (decks have their own nav).

## Invariants honored

- Reader-side, read-only, offline, additive; no block-model change; the `.qmd` is never
  written (single editing surface).
- **Decks excluded:** the enhancer returns early on `.qmd-deck`.
- **Idempotent:** guarded per element (`a.dataset` / a marker class) so it survives the
  live-preview re-mounts (`qmdEnhancers.run` on every change).
- **Pure URL builder:** `qmdAnchorUrl(id)` does no DOM/clipboard work and is deterministic
  — it sits beside `qmdBuildTextFragmentUrl`.

## Mechanism

All in `crates/core/assets/js/code-enhance.js`, as a new enhancer `qmdInitAnchorLinks(root)`
registered through `qmdEnhancers.register`.

**`qmdAnchorUrl(id)`** (new top-level helper):

```js
function qmdAnchorUrl(id) {
  var u = new URL(location.href);
  u.hash = '';                          // drop any existing #id / :~:text=
  return u.href + '#' + encodeURIComponent(id);
}
```

**`qmdInitAnchorLinks(root)`** (runs per mount, over `root || document`):

- `if (document.querySelector('.qmd-deck')) return;` (deck skip).
- For each `(root||document).querySelectorAll('h1[id],h2[id],h3[id],h4[id],h5[id],h6[id], figure.qmd-figure[id] > figcaption, ...')` — concretely: headings with an `id`, and the `<figcaption>` of a `figure[id]`/listing/table float (the float's `id` lives on the wrapper, so the link reads the ancestor `[id]`).
- Idempotency: skip if the target already has a trailing `a.qmd-anchor` (or set `el.dataset.qmdAnchored`).
- Build `<a class="qmd-anchor" href="#<id>" aria-label="Copy link to this section">#</a>`
  and append it to the heading (or the figcaption). The `href` makes it keyboard-focusable
  and a real in-page link.
- On click: do **not** `preventDefault` (let it set the URL hash — the address bar now
  shows the shareable anchor; the page is already there, so no jump). Then
  `qmdCopyText(qmdAnchorUrl(id), ok, fail)`; on ok, swap the `#` for a check glyph for
  ~1.2s, toggle a `qmd-anchor-copied` class, and announce "Link copied" via a shared
  `aria-live` region (reuse the page's, or a small one this enhancer owns). On `file://`,
  the copy still works; the link resolves once served over http/https.

**CSS** (`crates/core/assets/css/base.css`):

```css
.qmd-anchor { margin-left: .35em; color: var(--qmd-muted); text-decoration: none;
  opacity: 0; transition: opacity .12s ease; font-weight: 400; }
:is(h1,h2,h3,h4,h5,h6):hover > .qmd-anchor,
figcaption:hover .qmd-anchor,
.qmd-anchor:focus { opacity: 1; }
.qmd-anchor:hover { color: var(--qmd-accent); }
.qmd-anchor-copied { opacity: 1; color: var(--qmd-accent); }
@media print { .qmd-anchor { display: none; } }
```

(`:focus-visible` keeps it revealed for keyboard users; the `figcaption` rule scopes the
float case.)

## Verification

- **Corpus pin:** reuse `corpus/reader/long-read.qmd` (many headings) for the heading case;
  the float case is exercised by any figure-bearing corpus doc in the browser check (no new
  corpus doc).
- **Rust test** (`render/tests.rs`): `assembled_page_ships_anchor_links` — `render_html_page`
  contains `qmdInitAnchorLinks`.
- **Browser (chrome-devtools MCP):** build a doc with headings + a (mermaid) figure; hover a
  heading → the `#` appears → click → assert the clipboard / `qmdAnchorUrl` output is the
  absolute `…#<slug>` URL and the glyph flashes; keyboard-focus the `#` and confirm it shows;
  confirm a figure's figcaption also gets one; no console errors.
- **Gates:** `cargo test -p qmd-fast-core`, `clippy -D warnings`, `fmt`, `tsc`.

## Files

- `crates/core/assets/js/code-enhance.js` — `qmdAnchorUrl` + `qmdInitAnchorLinks`.
- `crates/core/assets/css/base.css` — `.qmd-anchor` rules.
- `crates/core/src/render/tests.rs` — `assembled_page_ships_anchor_links`.
