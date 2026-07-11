# OpenGraph card generation — design

**Date:** 2026-07-11
**Status:** approved (brainstorm), pending implementation plan
**Backlog item:** "★ NEXT UP — Social preview / OpenGraph card quality" (`notes/backlog.md`)
**Related:** `2026-07-11-auto-seo-artifacts-design.md` (the url-gated build-sidecar pattern this reuses)

## Context

The social/SEO **plumbing is already complete**. `crates/core/src/site/meta.rs::social_head`
emits full OpenGraph + Twitter-card tags on every page — `summary_large_image`, absolute
URLs (`url:`-gated), per-post `og:type=article` with the post's own title/description, and a
correct default **1200×630** card. `jsonld_head` emits a `BlogPosting`/`WebSite` image too.
The build already has a clean url-gated aux-file zone (`crates/server/src/build.rs`, ~L1142)
where the SEO sidecars (feeds, sitemap, robots, llms.txt) are written.

The **gap is purely the image.** Today `og:image` resolves to each page's `image:`
front-matter — which for posts is the raw generated figure. Those figures are the wrong
shape for a 1200×630 card: `em-algorithm` is 580×299 and `KL-divergence` 917×542 (upscale
soft under the 1200px a large card wants), `fourier-transform` is **276×274 square** (cropped
badly), all with tiny baked-in axis text and no title/branding. Separately, the homepage's
git-tracked `corpus/tech-blog/og-image.webp` (1200×630, no source in the repo) has the **old**
tagline "Notes on challenging technical things" baked into the pixels, while `_site.yml`'s
`description:` is now "Machine learning and statistics, worked out from first principles" — so
a shared homepage shows the new line as text and the stale line in the image.

## Goal

At build time, **auto-generate a branded 1200×630 card per content page** from the page's own
text and the site design, and point `og:image` / `twitter:image` / JSON-LD `image` at it. Make
it a **default** (url-gated, auto-on, no toggle) so no page ever ships a soft or oddly-cropped
preview. This subsumes the homepage fix for free: the home card regenerates from the hero's
*new* tagline, so the stale `og-image.webp` is deleted.

### Invariants honored

- **Offline / zero-CDN / deterministic.** No headless Chrome, no OG-image service, no CDN. No C
  dependencies (no libwebp). Same inputs → byte-identical output (reproducible build; the
  stale-sweep and cache stay stable).
- **`--tali-*` tokens only** for the palette. No new output format (a `.webp` sidecar is the
  same category as `sitemap.xml`, not a document target).
- **Single editing surface / preview-is-read-only** untouched (cards are a build/serve artifact,
  never a source write-back).

## Non-goals (scope boundary)

- No `og-card:` toggle or custom-per-page card override (YAGNI / perfect-the-default; a future
  escape hatch, explicitly deferred).
- No light-mode card variant (the card is a single static dark asset).
- **No body-typeface commitment.** The bundled font is scoped to cards; the "owned body webfont"
  backlog item stays its own design session.
- Cards for **content pages only** — not the 404 page or `{{< embed >}}`-referenced decks
  (aligns with the sitemap/feed page set).

## Decisions (resolved in brainstorm)

1. **Rendering stack: hand-composited raster, not SVG.** Social scrapers reject SVG, so
   `og:image` must be a raster (PNG/WebP). We draw the card by hand into an RGBA8 buffer. This
   matches the framework's homegrown-rendering DNA and the minimal-dependency bar. Chosen over a
   `resvg`/`usvg`/`tiny-skia`/`fontdb` SVG-template stack (~15-20 extra crates whose main win —
   design iteration — is undercut because resvg still needs text pre-wrapped by hand).
   - Refinement on the brainstorm option: we can go lighter than "the `image` crate" — use
     `ab_glyph` (glyph rasterization + metrics) + `image-webp` (pure-Rust lossless WebP encode)
     directly, compositing into our own `Vec<u8>` canvas. No full `image` crate.
2. **Bundled font: Newsreader (OFL).** Editorial/literary text serif; reads as a wordmark and
   small, fits the "Marginalia / iron-gall manuscript" identity, not a display-serif cliche.
   Bundle **Regular + Bold** static TTFs via `include_bytes!`. Card-scoped.
3. **Generated card drives the *social* image only.** `og:image` / `twitter:image` / JSON-LD
   `image` switch to the generated card. The page's `image:` front-matter is **untouched** and
   stays the in-page/listing thumbnail. Clean split: branded card = social preview; figure =
   content thumbnail.
4. **Auto-on, url-gated, no toggle.** Generated whenever `_site.yml` has `url:` set (same gate
   as the SEO sidecars and JSON-LD).

## Architecture

### New module `crates/core/src/site/card.rs`

Lives in `taliesin-core` so both the build (`taliesin-server::build`) and the preview
(`taliesin-server::serve_site`) call the same generator.

```rust
pub struct CardSpec {
    pub eyebrow: Option<String>,     // small-caps top line
    pub headline: String,            // large title (never empty; site title fallback)
    pub lead: Option<String>,        // description / lead paragraph
    pub footer_wordmark: String,     // constant branding, e.g. site title
    pub domain: Option<String>,      // e.g. "andreasbogossian.com", from `url:`
}

/// Deterministic. Renders `spec` onto a 1200x630 dark card and returns the encoded
/// image bytes (lossless WebP, or PNG fallback — see "Encoding" below). Infallible on
/// valid text; empty/over-long text is handled by wrap + truncate.
pub fn render_card(spec: &CardSpec) -> Vec<u8>;
```

Pure, no I/O. `CARD_DESIGN_VERSION: u32` const in this module (bumped when the template
changes, to cache-bust every card).

### Card visual (fixed dark palette)

1200×630, background `--tali-bg` dark `#16181d`, ~72px padding. Always dark (a single static
asset; matches today's card). Palette from the dark `--tali-*` tokens:

```
┌──────────────────────────────────────────────────────────┐
│  EYEBROW  (small caps, letter-spaced, #9aa0aa muted)       │  home: hero.eyebrow
│                                                            │  post: primary category | date
│  Headline — Newsreader Bold, #e6e6e6 fg,                   │  home: hero.headline
│  wrapped to 2-3 lines, auto-shrink one step if it          │  else: page.title
│  overflows 3 lines                                         │
│                                                            │
│  Lead / description — Newsreader Regular, #9aa0aa muted,   │  home: hero.lead
│  1-2 lines, truncated with an ellipsis                     │  post: page.description
│  ── hairline rule, #363a44 border ─────────────────────    │
│  ∿ mark   Andreas Bogossian            andreasbogossian.com │  curve tinted #9aa8dc accent
└──────────────────────────────────────────────────────────┘
```

- **Bell-curve mark** drawn **procedurally** — an antialiased Gaussian stroke (`y = exp(-x²)`),
  thematically on-brand, no SVG path needed. Tinted `--tali-accent` `#9aa8dc` (a restrained
  touch of the iron-gall ink).
- **Text layout** by hand: greedy word-wrap using `ab_glyph` scaled advance widths; glyph
  coverage composited into the canvas with the run's color (straight alpha over the dark bg).
  No shaping/kerning (ab_glyph is glyph-per-advance) — negligible at 1200px for a wordmark and
  short title. Headline auto-shrinks one font-size step if it still overflows 3 lines at the
  base size; lead truncates to fit its box with `…`.

### Per-page CardSpec derivation

Computed in `card.rs` from `Site` + `Page` (a helper `card_spec(site, page) -> CardSpec`):

| Field           | Home (`index.html` with `hero:`) | Post (`date:` present)          | Other page                 |
|-----------------|----------------------------------|---------------------------------|----------------------------|
| eyebrow         | `hero.eyebrow`                   | primary category, else the date | none                       |
| headline        | `hero.headline` ∥ site title     | `page.title` ∥ site title       | `page.title` ∥ site title  |
| lead            | `hero.lead` ∥ site description    | `page.description`              | `page.description`         |
| footer_wordmark | site title                       | site title                      | site title                 |
| domain          | host of `url:`                   | host of `url:`                  | host of `url:`             |

(∥ = fallback.) `headline` is never empty. This makes the **home card carry the new tagline**
via `hero.lead`/`hero.headline`, fixing the stale-image problem.

### Deterministic URL

`card.rs::card_url(site, page) -> Option<String>`:
- `None` when `site.config.url` is unset (matches the SEO/JSON-LD gate).
- else `Some(format!("/og/{:016x}.webp", fnv1a(&key)))`, where `key` concatenates
  `CARD_DESIGN_VERSION` + every `CardSpec` field + a font-id tag, using the shared
  `taliesin_core::hash::fnv1a` (the same content-hash primitive block ids use — do **not** swap
  the algorithm). Identical inputs dedupe to one file; any text or template change cache-busts.

Absolute `og:image` URL is formed the existing way — `card_url` is a site-root-relative path,
joined onto `base` by `meta.rs` exactly as a relative `image:` is today.

### Build wiring (`crates/server/src/build.rs`)

In the url-gated aux zone (beside the SEO sidecars): for each written content page, compute
`card_spec` → `render_card` → write `out/og/<hash>.webp`. **Dedupe** by hash (many pages could
share a card only if identical; in practice one per page). Add every written card path to the
stale-sweep `keep` set (~build.rs L1186) so the mirror-sweep does not delete them. On encode
error: `log::warn` and skip that one card (the page still ships; `og:image` is simply absent —
never abort the build).

### Preview wiring (`crates/server/src/serve_site.rs`)

`social_head` emits the `/og/<hash>.webp` URL in preview too (the real blog sets `url:`), so
preview must serve it or the tag is a dead link. Add a `GET /og/<hash>.webp` route that finds
the page whose `card_url` hash matches, generates on demand, and caches the bytes in memory.
Lets the author eyeball the real card at `localhost`.

**Phasing:** build path first (the deliverable), preview-serve second. If preview hash-matching
proves fiddly it does not block the core; worst case preview's `og:image` 404s locally (never
scraped), which is acceptable interim.

### meta.rs / jsonld_head wiring

`social_head` and `jsonld_head` swap their image source from `page.card_image` /
`cfg.card_image` to `card::card_url(site, page)`. `twitter:card` stays `summary_large_image`
whenever a card URL exists. Everything else (title/description/canonical/citation meta) is
unchanged. `image:` front-matter no longer feeds the social image at all.

### Config cleanup

- Delete `corpus/tech-blog/og-image.webp` and its `image: og-image.webp` line in
  `corpus/tech-blog/_site.yml` (superseded; verify no other reference first).
- Confirm site-level `image:` removal is safe for listing thumbnails (site-level `image:` is the
  default social card, not a per-post listing thumbnail — per-post `image:` drives those).

### Dependencies & licensing

- Workspace + `crates/core/Cargo.toml`: `ab_glyph`, `image-webp` (both pure-Rust, no C).
- Bundle `crates/core/assets/fonts/Newsreader-Regular.ttf` + `Newsreader-Bold.ttf` (OFL).
- `THIRD_PARTY.md`: add Newsreader (OFL), `ab_glyph`, `image-webp` (+ `png` if used as fallback).
- Optional (deferred): subset the TTFs to Latin to shrink the binary.

### Encoding

Target **lossless WebP** via `image-webp`'s encoder into RGBA8 (matches today's `.webp`
convention, smaller than PNG on a flat card). **First implementation step is a spike** confirming
`image-webp` exposes a usable RGBA lossless encode API; if not, fall back to the tiny pure-Rust
`png` crate and emit `/og/<hash>.png` (og:image accepts PNG; still zero-C). The chosen extension
is reflected in `card_url`.

## Error handling

- Font load: a bundled-font parse failure is a hard error (packaging bug; the font is always
  present).
- Encode: warn + skip the single card, never abort the build.
- Over-long / empty text: handled inside `render_card` by wrap + truncate; `headline` falls back
  to the site title so it is never empty.

## Testing / corpus pins

- **Unit (`card.rs`):** `render_card` yields a valid 1200×630 image (decode header + assert
  dims); byte-identical on a repeat call with the same spec (determinism); no panic on empty
  text and on a 300-char headline.
- **`meta.rs`:** with `url:` set, `og:image`, `twitter:image`, and JSON-LD `image` all resolve
  to the `/og/<hash>.webp` card URL, **not** to `image:`; all absent without `url:`.
- **Corpus (`crates/core/tests/tech_blog.rs`, or the build-level test):** every built page's
  `og:image` is a `/og/*.webp` that exists in the output tree; the stale-sweep keeps the cards;
  `og-image.webp` is gone from the output.
- **Honest caveat (backlog-style):** the pins cover the card **inputs** (home spec derives from
  `hero:`, post spec from `page.title`), artifact **existence**, **dims**, and **determinism** —
  **not** pixel-level "the title text is legible in the raster." Asserting rasterized glyphs is
  impractical, so the plumbing is pinned, not the pixels.

## Implementation order (for the plan)

1. **Spike:** confirm `image-webp` RGBA lossless encode (else PNG fallback). Add deps + bundle
   Newsreader TTFs.
2. **`card.rs` core:** `CardSpec`, `render_card` (canvas + rects + procedural curve + text
   wrap/blit + encode), `CARD_DESIGN_VERSION`, `card_spec`, `card_url`. TDD the unit pins first.
3. **Wire `meta.rs` + `jsonld_head`** to `card_url`; update the meta.rs tests.
4. **Build emit** in `build.rs` aux zone + stale-sweep `keep`; corpus/build test.
5. **Preview serve** route in `serve_site.rs`.
6. **Cleanup:** delete `og-image.webp` + the `image:` line; `THIRD_PARTY.md`; verify no dangling
   reference.
7. **Verify:** `cargo test -p taliesin-core`, build `corpus/tech-blog`, and browser-check a
   rendered card (chrome-devtools MCP: load the `.webp`, confirm the new tagline on home + a post
   title on a post card).
