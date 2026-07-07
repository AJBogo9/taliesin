# Cross-reference backlinks (xref-anchor tier) — design

Date: 2026-07-07
Status: approved, ready to implement
Branch: `xref-backlinks`

## Motivation

Taliesin resolves *forward* cross-references project-wide: `@fig-x` / `@sec-x` /
`@thm-x` on one page links to the target's page and number. There is no *reverse*
view: standing on a figure/section/theorem, a reader can't see which other pages
point at it. The interactive xref *graph* tool once surfaced that relationship and
was removed (interaction not good enough). This is its lightweight, static,
reading-first replacement: a per-target **"Referenced by"** affordance.

Backlog Tier-1, decided 2026-07-07 ("build it; cheap tier only — fig/sec/tbl/eq/
lst/thm anchors; citations stay out as the expensive tier").

## Scope

- **In:** the anchor tier — every `is_ref_anchor` prefix (`sec- fig- tbl- eq- lst-
  thm- lem- cor- prp- def- exm- rem-`).
- **Cross-page only.** A same-page reference is footnote-shaped noise and, by
  construction, cite never marks it (`data-qmd-xref` is emitted *only* when the
  anchor is unknown to the current document's registry, i.e. defined elsewhere —
  `cite/render.rs:265`). So "cross-page only" needs no extra filtering.
- **Site-only.** A single-`.tmd` build/preview has no other pages; the feature is
  inert there, exactly like cross-page forward xref.
- **No config.** Always on when a target has cross-page referrers (perfect the
  default; no knob).
- **Out (expensive tier, not this change):** citation backlinks (needs a site-wide
  bibliography-merge decision first); precise within-page referrer locations
  (page-level only, matching the forward index); books-vs-websites differences.

## Design

### 1. Reverse index (data)

Add to `Site`:

```rust
/// anchor → referring page urls (deduped, in site/page order). Cross-page only.
pub backlinks: HashMap<String, Vec<String>>,
```

Built during `discover`, riding the **all-pages render loop that
`harvest_xref_numbers` already runs** (no new render pass). For each page, in
`self.pages` order:

1. Render it once (the loop already does, for `doc.xref_numbers`).
2. Collect the set of `data-qmd-xref="A"` marker anchors in `doc.blocks` (a new pure
   helper `xref_markers_in(html) -> Vec<&str>` in `xref.rs`, dedup per page).
3. For each unique anchor `A` that is a **known target** (`self.xref_targets`
   contains `A`), push this page's `url` onto `backlinks[A]`.

Because pages are visited in order and each contributes at most once per anchor, the
referrer vectors are already deduped and in document order. Dangling refs never match
a known target, so they're excluded for free.

`harvest_xref_numbers` gains this second responsibility (it already owns the
all-pages render); its doc comment is updated to say it also builds the reverse
index. No third traversal.

### 2. Surface (rendering)

New method `attach_backlinks(&self, blocks: &mut [Block], current_url: &str)`, called
in **both** page-render paths right next to `resolve_cross_refs` (static build +
live preview), so build and preview render identically.

```
up = "../".repeat(current_url.matches('/').count())
for each anchor A where xref_targets[A].url == current_url and backlinks[A] non-empty:
    find the block whose html contains id="A"
    inject a muted "Referenced by" line listing backlinks[A]
```

The line:

```html
<div class="tali-backrefs">↳ Referenced by
  <a href="{up}{url}" class="tali-backref">{page title}</a> ·
  <a href="{up}{url2}" class="tali-backref">{title2}</a></div>
```

- **Referrer label** = the referring page's display title
  (`page.title.as_deref().unwrap_or(&page.rel)`, the existing listing-card fallback
  — always the chapter title for a book).
- **Link** = page-level (`{up}{page.url}`, no fragment).
- **Injection point:** for a `<figure>` target the line goes *inside* the
  `<figcaption>` (rides with the caption); for every other target (heading, theorem
  div, equation, table, listing) it is appended to the end of the defining block's
  HTML. Verify the heading case reads acceptably in the browser; if a line directly
  under a heading is intrusive, that's the one placement to revisit (no design
  change, just injection point).

### 3. Styling

`site.css`: `.tali-backrefs` is small, near-monochrome (muted foreground var),
tight top margin, so it reads as scholarly apparatus, not chrome. `.tali-backref`
links inherit the quiet xref link treatment. No new color tokens.

## Data flow

```
discover
 ├─ scan_xref_targets  → xref_targets (anchor → {url, number})      [source scan]
 ├─ harvest_xref_numbers (all-pages render):
 │     ├─ fill xref_targets[A].number  (existing)
 │     └─ build backlinks[A] ← pages carrying data-qmd-xref="A"     [NEW]
 └─ build_hover_index  (definer-pages render)                       [existing]

page render (build + serve_site):
 ├─ resolve_cross_refs(blocks, url)   rewrite forward markers        [existing]
 └─ attach_backlinks(blocks, url)     inject "Referenced by" lines    [NEW]
```

## Testing

- **Unit (`xref.rs`):** `xref_markers_in` returns every marker anchor on a line /
  block, ignores non-marker links, is quote-safe.
- **Unit (site):** a two-page fixture where page B references an anchor defined on
  page A builds `backlinks = {A: [B.url]}`; a dangling ref contributes nothing; a
  same-page ref contributes nothing.
- **Unit (injection):** `attach_backlinks` appends exactly one `.tali-backrefs` line
  to A's defining block, links to B, and is a no-op for a page with no referred
  targets.
- **Corpus pin:** `corpus/demo-book` already exercises it — `results.tmd`
  cross-references `@sec-methods`, `@sec-setup`, `@thm-kl`, all defined on
  `methods.tmd`. A corpus test asserts the built `methods.html` shows
  "Referenced by … Results" on all three targets, and that `results.html` (which
  defines no referred-to anchors) has no `.tali-backrefs`.
- **Browser (chrome-devtools):** preview `demo-book`, screenshot `methods.html` at
  3 viewports; confirm the muted line renders under the three targets, links
  navigate to `results.html`, 0 console errors.

## Non-goals / invariants preserved

- Read-only view: the line is navigation only; the preview never writes source
  (single-editing-surface invariant untouched).
- No exec/kernel contact (pure render/discover-time).
- Block-model invariants (`data-block-id` / `data-sourcepos`) untouched — injection
  edits a block's inner HTML string, not its identity, exactly like
  `resolve_cross_refs`.
