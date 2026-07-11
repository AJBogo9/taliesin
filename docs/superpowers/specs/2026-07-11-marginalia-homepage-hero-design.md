# Marginalia homepage hero — design

**Date:** 2026-07-11
**Status:** design approved (brainstorm), pending spec review then implementation plan
**Scope:** the personal blog homepage (`corpus/tech-blog/index.tmd`) plus one framework
default (`hero:` gains an optional portrait). Corpus-plus-roadmap: the new capability ships
pinned by the homepage that uses it.

## Why

The blog homepage is the author's forward-facing brand and currently wears Quarto's
`about: {template: jolla}` centered-avatar template: photo, name, a flat grey rule, then a
loose bio paragraph. The audit (`notes/2026-07-11-website-design-audit.md`,
`#no-native-taliesin-hero-grammar` / `#about-header-no-subtitle-slot` / `#identity-directions`
Direction A "Marginalia" / `#accent-absent-on-blog`) calls this the weakest-designed page on
the site and the least native-Taliesin. The marketing site already opens with a confident
`hero:` block; the blog should share that primitive so both sites read as one design language,
while staying editorial ("Marginalia": the iron-gall accent is literally manuscript ink).

## Decisions already made (author, 2026-07-11)

- **Voice:** first-principles.
- **Identity direction:** A "Marginalia".
- **Architectural route:** extend `hero:` with a photo and switch the homepage from `about:`
  to `hero:` (chosen over extending `about:` with text slots, or hand-authored bespoke markup).
  Rationale: Marginalia's copy (eyebrow + POV headline + lead) is exactly the hero grammar,
  which already exists; this grows one primitive by one optional field instead of duplicating
  hero's text slots onto `about:`, and it unifies the blog with the marketing hero.
- **Headline (`<h1>`):** "Machine learning, worked out from first principles"
- **Eyebrow:** "ML · STATISTICS · ALGORITHMS"
- **Lead:** "Notes on concepts I'm working to understand, mostly statistics and the math
  underneath the models, built up from running code." (trimmed so it stops echoing the
  headline's "machine learning" / "first principles")
- **Hero actions:** none. GitHub and LinkedIn are already in the footer, CV in the nav, and the
  Recent Posts listing sits directly below as the natural next action. The hero stays clean.
- **Photo:** round, smaller, demoted to the side, with a thin `--tali-border-strong` ring so
  its saturated red background is contained and stops touching the page
  (`#avatar-red-halo-clashes`).

## Design

### 1. Homepage front matter (`corpus/tech-blog/index.tmd`)

Replace the `about:` block with a `hero:` block; keep the LCP preload and the listing.

```yaml
---
title: "Andreas Bogossian"          # stays: drives <title>, og:title, nav wordmark
hero:
  eyebrow: "ML · STATISTICS · ALGORITHMS"
  headline: "Machine learning, worked out from first principles"
  lead: "Notes on concepts I'm working to understand, mostly statistics and the math underneath the models, built up from running code."
  image: profile.webp               # NEW HeroSpec field
  image-alt: "Andreas Bogossian"
page-layout: article                # keep (editorial reading measure); verify grid fit
toc: false
include-in-header:                  # keep (LCP: preload the portrait)
  text: |
    <link rel="preload" as="image" href="profile.webp" fetchpriority="high">
listing:
  id: recent-posts
  contents: posts
  sort: "date desc"
  max-items: 2
  type: grid
---

## Recent Posts

::: {#recent-posts}
:::

[View all posts →](blog.tmd){.btn}
```

Body changes: the loose lead paragraph moves into `hero: lead:` (no longer body text). The
`[View all posts →]` link drops `.btn-outline-primary` (a Bootstrap class with no native rule)
for plain `.btn`, closing `#btn-outline-primary-bootstrapism` while the file is open.

The visible `<h1>` becomes the POV headline; the name stays in the nav wordmark, `<title>`, and
OpenGraph (the audit's "stop repeating the name").

### 2. Framework: `hero:` gains an optional portrait

**`HeroSpec`** (`crates/core/src/site/mod.rs`): add
```rust
pub image: Option<String>,      // relative to the page, emitted as-is (like about: image)
pub image_alt: Option<String>,
```

**`parse_hero`** (`crates/core/src/site/frontmatter.rs`): parse `image:` and `image-alt:`
(scalar string), mirroring how `about:` parses its `image`/`image-alt`.

**`hero_html`** (`crates/core/src/site/mod.rs`): when no image is present, emit exactly the
current markup (byte-identical, so the marketing hero and its unit test are untouched). When an
image is present, wrap the text in `.hero-body`, add the `hero-has-media` class, and append the
portrait:

```
no image (unchanged):
  <header class="hero" ...>{eyebrow}<h1>{headline}</h1>{lead}{actions}</header>

with image (new):
  <header class="hero hero-has-media" ...>
    <div class="hero-body">{eyebrow}<h1>{headline}</h1>{lead}{actions}</div>
    <img class="hero-media" src="{image}" alt="{image_alt}">
  </header>
```

`{image}`/`{image_alt}` are HTML-escaped like every other emitted attribute. The
`data-block-id="qmd-title-block"` + `data-qmd-src` invariants are preserved on the `<header>`.

### 3. CSS (`crates/core/assets/css/base.css`)

The current `.hero` is `text-align: center` (marketing). All Marginalia styling is scoped to
`.hero-has-media` so the imageless hero is unchanged in both HTML and CSS:

```css
/* A hero with a portrait (the blog homepage). Scoped to .hero-has-media so the
   imageless marketing hero is untouched. Left-aligned, two columns, ink hairline. */
.hero-has-media { text-align: left; display: grid; grid-template-columns: 1fr auto;
  gap: 2rem; align-items: center; }
.hero-has-media .hero-lead { max-width: none; margin-left: 0; margin-right: 0; }
.hero-has-media .hero-eyebrow::after { content: ""; display: block; width: 2.5rem;
  height: 2px; background: var(--tali-accent); margin-top: .55rem; }
.hero-media { width: 140px; height: 140px; border-radius: 50%; object-fit: cover;
  border: 1px solid var(--tali-border-strong); }
@media (max-width: 640px) {
  .hero-has-media { grid-template-columns: 1fr; }   /* DOM order stacks text, then photo */
  .hero-media { width: 108px; height: 108px; }
}
```

Notes:
- The eyebrow is already `color: var(--tali-link)` mono-uppercase; the `::after` adds the short
  ink hairline that replaces the `jolla` header's flat full-width grey rule (`#accent-absent-on-blog`).
- Colors use `--tali-*` tokens only (no vendor hex, no opacity-muted text): theme invariant.
- Mobile stacks text-first (message before face) via DOM order; flip with `order` only if the
  screenshot argues for a byline layout.

## Invariants respected

- **Offline / zero-CDN:** the portrait is a bundled local asset; no new external requests.
- **Single editing surface:** the preview never writes back; this is source-authored front matter.
- **HTML-only:** no new output format.
- **Minimal-config / perfect the default:** one optional field (`image:`) on an existing
  primitive, no blog-local CSS; every future Taliesin homepage can use a portrait hero.
- **Theme system:** `--tali-*` tokens only.
- **No marketing-hero regression:** the imageless path is byte-identical in HTML and CSS.

## Test impact and corpus coverage

- `crates/core/tests/tech_blog.rs` asserts `tali-about` on the homepage today; flip it to assert
  the hero (`class="hero hero-has-media"`, the eyebrow, the `<h1>` headline, `hero-media`,
  and the absence of `tali-about`). TDD: update this assertion first, watch it fail, then implement.
- `crates/core/src/site/frontmatter.rs` (`parse_hero` unit test): add an `image:`/`image-alt:`
  case; the no-image case stays as-is.
- `crates/core/src/site/mod.rs` (`hero_html` unit test): add a with-image case asserting the
  `.hero-body` wrapper + `<img class="hero-media">`; assert the no-image case is unchanged
  (byte-identical) so the marketing hero is provably untouched.
- **Corpus pin:** the homepage itself is the target corpus doc for the new `image:` capability
  (renders in the `tech_blog` net); the marketing site's imageless `hero:` pins the unchanged path.
- **`about:` coverage:** after this change, `corpus/tech-blog/index.tmd` is likely the last real
  `about:` *page* in the corpus (the `docs/guide` mentions are documentation examples, not
  rendered about pages). `about_html` keeps its `mod.rs` unit test, and `about:` remains a
  supported, documented feature. Flag for the author: whether to retire the now-unused `about:`
  primitive later is a separate item (`#jolla-about-generic-identity`), out of scope here.

## Verification plan

- `cargo test -p taliesin-core` green (corpus + unit).
- Build the site and browser-check via chrome-devtools at three viewports (mobile ~390, laptop
  landscape ~1440, laptop portrait ~900) in light and dark: eyebrow + ink hairline, POV headline,
  trimmed lead, portrait ring containing the red background, two-column on desktop, clean stack on
  mobile, no horizontal overflow, no regression to the Recent Posts grid.
- Confirm the marketing hero (`site/index.tmd`) is visually unchanged (imageless path).

## Out of scope (tracked separately in the audit file)

- Marketing-hero restyle (feature-first policy defers marketing work).
- `custom.css` deletion (`#custom-css-delete-to-zero`; needs the `.sr-only` alias first).
- The nav-prefetch stack (`#dead-nav-prefetch-stack`) and RSS feed (`#rss-feed-silently-dropped`).
- Replacing the portrait with a deliberate studio photo (`#profile-photo-casual`): the ring
  mitigates the halo; a new photo is the author's call, later.
- Retiring the `about:` primitive.

## Open / verify during implementation

- `page-layout: article` vs `full`: keep `article` (editorial measure); if the two-column hero or
  the 2-card grid reads cramped at the reading measure, revisit. Verify with the screenshot.
- Mobile stack order (text-first vs a photo byline on top): decide from the mobile screenshot.
