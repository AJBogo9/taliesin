# Website "back to listing" link

**Status:** approved 2026-07-06. Tier-1 (build-ready). Branch `back-to-listing-link`.

## Goal

On a website post page, render a single "back" affordance ("← Blog") that returns
the reader to the listing page the post belongs to. Auto-derived, no config key.
Books are untouched (they already use the chapter prev/next pager in the same slot).

## The rule (with one refinement over the original backlog note)

The original note said: *link only when exactly one listing page's `contents:` covers
the post, else skip as ambiguous.* That rule produces **zero** backlinks on the
canonical `corpus/tech-blog/`, because two pages both draw from `contents: posts`:

- `index.tmd` — `listing: { contents: posts, max-items: 2 }` (a recent-posts **preview**)
- `blog.tmd` — `listing: { contents: posts }` (the **full** list)

Every post is covered by both → always ambiguous → never links.

**Refinement:** a `max-items`-capped listing is a *preview*, not the post's home, so it
does **not** confer ownership. Only **un-capped** listing specs count as ownership
claims. The owner then resolves uniquely:

- every `posts/*` → owned only by `blog.tmd` → **"← Blog"**
- every `projects/*` → owned only by `projects.tmd` → **"← Projects"**
- Home / cv / publications / the listing pages themselves → covered by nothing → no link
- two **un-capped** listings on the same dir → still genuinely ambiguous → skip
- zero un-capped owners → skip

## Algorithm

New method `Site::listing_owner(&self, page: &Page) -> Option<&Page>`:

1. For each candidate host `H` in `self.pages`, for each `spec` in `H.listings` where
   `spec.max_items.is_none()`: compute `prefix = listing_prefix(H, spec)`. `H` is a
   candidate owner of `page` iff `page.rel != H.rel && page.rel.starts_with(&prefix)`.
2. Dedupe candidates by `rel`. Return `Some(H)` iff exactly one distinct host qualifies,
   else `None`.

`listing_prefix(host, spec) -> String` is **extracted from the existing `collection()`**
(the `join_rel` + `trim_end_matches('/')` + `contents: .` empty-dir rule), so both
`collection()` and `listing_owner()` share one definition of "what a listing covers".
This is the only refactor; it is DRY-neutral in behavior (collection keeps calling it).

Titleless pages never appear in a listing, so they never resolve an owner (consistent
with `collection()` dropping them); no special-casing needed.

## Seam, markup, placement

`page_chrome()` at `site/mod.rs` non-book branch currently sets
`post_nav_html: String::new()`. Replace with `self.listing_backlink_html(page, depth)`,
which returns `""` when `listing_owner` is `None` (so nothing changes for pages with no
owner). Books keep `book_nav_html`.

Markup (mirrors the book pager's muted style + `tali-back-glyph`):

```html
<nav class="tali-postnav tali-listing-backnav" aria-label="Back to listing">
  <a class="tali-back-link" href="{up}{owner.url}">
    <span class="tali-back-glyph" aria-hidden="true">←</span> {esc(owner.title)}
  </a>
</nav>
```

- `up = "../".repeat(depth)` (same depth math as `book_nav_html`/`navbar_html`).
- `aria-hidden` on the glyph so a screen reader reads the title, with the `<nav>` label
  conveying direction (a small a11y improvement over the book pager).
- Renders in the existing non-book `post_nav` slot (`render/page.rs`), i.e. the bottom of
  the article inside `.tali-site-main`.

## CSS (`assets/css/site.css`)

A small block next to the existing `.tali-book-postnav` rules:

```css
.tali-listing-backnav { margin-top: 2.5rem; padding-top: 1.2rem;
  border-top: 1px solid var(--tali-border); }
.tali-back-link { color: var(--tali-muted); text-decoration: none; font-size: .95rem; }
.tali-back-link:hover { color: var(--tali-link); }
```

## Testing / corpus pin

`corpus/tech-blog/` already contains the scenario, so it is the pin. Add assertions:

- `tests/tech_blog.rs`: a rendered post page contains `← Blog` linking to `blog.html`;
  a project page contains `← Projects`; Home (`index`), `blog.tmd`, `projects.tmd`,
  `cv.tmd`, `publications.tmd` render **no** `tali-listing-backnav`.
- `site/mod.rs` unit tests: (a) unique un-capped owner → link; (b) two un-capped listings
  on same dir → `None`; (c) a capped preview + a full list → the full list owns;
  (d) capped-only (no full list) → `None`.

Depth: a post at `posts/a-star/index.html` (depth 2) links `../../blog.html`.

## Out of scope

No config key. No top-of-page duplicate (bottom slot only, matching the book pager).
No change to book navigation, to `collection()` behavior, or to citation-tier backlinks
(the separate, deferred "Referenced by" blocker). Not shown on listing/about/hero pages.
