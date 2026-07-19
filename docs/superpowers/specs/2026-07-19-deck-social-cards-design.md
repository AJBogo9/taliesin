# Deck social cards (PMF C-PUB-1 deck residual)

**Date:** 2026-07-19
**Status:** approved (owner ruled decks should get their own social card, 2026-07-19)

## Problem

The PMF audit's C-PUB-1 concern — "the amateur tell is one site-wide card" — is
satisfied for pages: a website page and a book chapter each get their own branded
1200×630 OpenGraph card, pinned this session
(`meta.rs::a_book_chapter_gets_its_own_distinct_og_card_not_one_site_wide`).

A **deck is the exception.** An embedded deck (referenced by `{{< embed deck.tmd >}}`)
is built to its own standalone `.html` via `render::deck_page_from_doc`, whose template
emits only `<title>` + favicon + theme + `doc.includes.in_header` — **no OG/Twitter meta
at all** (unlike the HTML standalone path `html_page_from_doc`, which emits
`social_meta_head`). So a deck URL shared on social renders a bare link, while every page
beside it renders a rich card.

## Scope (owner-ruled)

Give an **embedded deck in a site build with `url:` set** the same rich social treatment a
page gets: `og:title`, `og:description`, `og:type=website`, `og:url`, `og:image` (its own
branded card), and `twitter:card=summary_large_image` (+ `twitter:title/description/image`).

Explicitly **out of scope** (unchanged):
- **Live preview.** A preview is not scraped, and a deck's on-demand `/og/<hash>` would not
  resolve (the deck is not in `site.pages`, so `serve_site`'s `og_card` hash lookup can't
  find its spec). Emitting a deck `og:image` in preview would 404.
- **Single-doc `build deck.tmd`.** No site, no `url:`, so no absolute card URL is possible.
  It keeps today's behaviour (a deck template with no social meta).
- Decks with no `url:` in their site: byte-identical to today (no card, no meta).

## Design (Approach A — enrich `in_header` in the build loop, one shared emitter)

The site build's deck loop (`crates/server/src/build.rs`) already renders the deck `doc`
and holds `&site`, so it is the natural place to compute + inject the meta. The deck
template already emits its `{in_header}` slot, so **no change to `deck.rs` or the
`render_doc_to_page` API** is needed.

### 1. `crates/core/src/site/card.rs`

Add a deck-specific card spec (a deck is not a `Page`, so it can't use `card_spec`):

```rust
/// A branded card spec for a deck: its title is the headline, its subtitle/description
/// the lead, and "Slides" the eyebrow (so a shared talk card reads as a presentation at a
/// glance — the deck analogue of a post's category kicker). Footer + domain are the site's,
/// exactly like a page card.
pub fn deck_card_spec(site: &Site, title: Option<&str>, lead: Option<&str>) -> CardSpec
```

`render_card` / `card_rel_path` are reused unchanged (they take a `CardSpec`). Re-export
`deck_card_spec` from `site/mod.rs` (next to `card_spec`).

### 2. `crates/core/src/site/meta.rs`

Split the OG/Twitter tag **emission** out of `social_head` into a shared private helper so a
page and a deck emit identical tag shapes by construction (no drift — the same "the correct
helper already existed next door" trap the backlog repeatedly names):

```rust
// Fields → tags. Shared by the page path (social_head) and the deck path.
fn emit_social(site_title: Option<&str>, title: &str, desc: Option<&str>,
               page_url: Option<&str>, image: Option<&str>, og_type: &str) -> String
```

`social_head(site, page)` computes its fields from the `Page` (as today) and calls
`emit_social`. Add:

```rust
/// The social/SEO meta block for an embedded deck (built off-`Page`, so it needs its own
/// entry point). `deck_url` is the site-root-relative output URL (e.g. `talk.html`).
pub(crate) fn deck_social_head(site: &Site, deck_url: &str, title: Option<&str>,
                               lead: Option<&str>) -> String
```

It computes `page_url = <site.url>/<deck_url>` and `image = <site.url>/<deck card rel>`,
both url-gated (Some only when `url:` is set), `og_type = "website"`, then calls
`emit_social`. Exposed to the build crate as a thin **`Site` method**
`Site::deck_social_head(&self, deck_url, title, lead) -> String` (delegating to the
`meta::deck_social_head` free fn), mirroring how `render_page` fronts `meta::social_head`.

The card image URL uses `deck_card_spec` + `card_rel_path`, so the meta's `og:image` and the
PNG the build writes agree by construction (the same URL/file discipline `card_url` keeps).

### 3. `crates/server/src/build.rs` (deck loop, ~line 1543)

After rendering the deck `doc`, when `site.config.url.is_some()`:
- Build `spec = deck_card_spec(&site, doc.title.as_deref(), lead)` where
  `lead = doc.subtitle.or(doc.description)`.
- Write `out/og/<card_rel_path(&spec)>.png` = `render_card(&spec)` (deterministic bytes;
  a hash collision with a page card just rewrites identical bytes — harmless).
- `doc.includes.in_header.push_str(&site.deck_social_head(&deck.url, doc.title.as_deref(), lead))`
  **before** `render_doc_to_page`.
- Push the PNG rel path onto `card_paths` (so the deploy-tree sweep keeps it).
- Emit the same `uncovered_glyphs` font warning pages get.

When `url:` is `None`, none of this runs — the deck is byte-identical to today.

## Corpus pin (corpus-leads)

`corpus/embed/` already has a page (`index.tmd`) embedding a deck (`talk.tmd`) but no `url:`.
Add `url:` to `corpus/embed/_site.yml` and a `title:`/`subtitle:` to `talk.tmd`. The existing
`embed_site_build.rs` assertions (iframe resolves, deck built, kept out of nav) are unaffected
(they don't assert on OG tags).

New `crates/server/tests/deck_social_card.rs` builds `corpus/embed` and asserts on
`talk.html`:
- `property="og:image" content="https://…/og/<hash>.png"` present, and **distinct** from the
  embedding page's card (`index.html`) — the "not one site-wide card" property;
- `name="twitter:card" content="summary_large_image"`;
- `property="og:url"` present and absolute;
- the deck's card PNG exists on disk under `og/`.

Mutation-checked: neutralising the `in_header` injection (or the url gate) drops the deck
`og:image` and the test fails.

## Testing

- **Core unit** (`card.rs`): `deck_card_spec` sets headline=title, lead=subtitle, eyebrow="Slides",
  footer=site title, domain=host; falls back headline→site title when the deck is untitled.
- **Core unit** (`meta.rs`): `deck_social_head` emits `summary_large_image` + `og:image` +
  `og:url` when `url:` is set, and degrades (no image, `summary`) when it is not; shares the
  emitter with `social_head` (a shared-emitter drift guard if cheap).
- **Integration** (`deck_social_card.rs`): the end-to-end build pin above.

## Non-goals / invariants preserved

- No CDN, no preview write-back, no new output format (`--tali-*`/offline only).
- The deck card is composited by the same offline `render_card` (no headless browser).
- Single-editing-surface + warm-page eviction untouched.
- `render_doc_to_page` / `deck_page_from_doc` signatures unchanged (the injection is data on
  the `doc`, not an API change).
