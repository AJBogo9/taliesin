# B7 — Media playback behavior (WCAG 2.2.2 + single active player)

Date: 2026-07-22. Backlog item B7 (P2, a11y + UX). Pin: `corpus/posts/fourier-transform/`.

## Problem

Two defects on the same delivery surface:

1. **`{{< video >}}` autoplays with no pause path.** `video_html` (`render/extension/mod.rs`)
   emits `autoplay muted loop playsinline` with **no `controls`**. A looping screencast that
   auto-starts, runs > 5 s, and sits beside body text is a live **WCAG 2.2.2 (Pause, Stop,
   Hide)** failure on the forward-facing site (`site/index.tmd`, `site/features.tmd`).

2. **No single-active-player coordination.** Four raw `<audio controls>` in
   `corpus/posts/fourier-transform/index.tmd` can all play at once; there is zero
   media-coordination JS anywhere.

## Design

### Part (a) — video: user-initiated play across every input mode

Playback is never automatic. **Correction during implementation:** the initial plan added a
click/tap *pin* toggle on the inline video, but the pre-existing lightbox (`11-lightbox.js`)
already intercepts every click on `.tali-video video` at document-capture (`stopPropagation`
+ `openVideo`) to open an enlarged, playing copy. A pin toggle both fought that and was
redundant — the lightbox *is* the tap-to-play / "watch it properly" affordance. The shipped
model instead lets the lightbox own click/tap and keeps the inline behavior to preview only:

- **Hover** (pointer): `pointerenter` plays a transient inline preview, `pointerleave` pauses.
- **Keyboard focus**: the `<video>` is `tabindex="0"` with an `aria-label`; `focusin` plays,
  `focusout` pauses — so a keyboard user can play it inline (parity with hover). Guarded by a
  shared `pointering` flag so a mouse click's focus churn doesn't flicker the inline clip as
  the lightbox opens.
- **Click / tap**: opens the video in the lightbox (enlarged, playing) via the existing
  capture-phase delegation. This is the **touch play path** and the **explicit** play path;
  this fragment deliberately binds *no* click/pointerup handler.
- **`prefers-reduced-motion: reduce`**: hover/focus never auto-play; an explicit click still
  plays (via the lightbox) — a user-requested motion.

No pin state is needed. `data-playing` on the `<figure>` drives the paused play-glyph.

Emission / render changes:

- `video_html` drops the unconditional `autoplay`; keeps `muted loop playsinline`; adds
  `preload="metadata"` (first frame renders as a still while paused), `tabindex="0"`, and an
  `aria-label` (default "Screencast", or the caption when present). Marker class on the
  `<figure>` (`.tali-video` already present) is enough for the enhancer to find it.
- `theme.rs` `syncThemeVideos` stops force-playing on load. It now only (i) promotes
  `data-src`→`src` on the theme-visible variant and (ii) pauses the hidden variant. All
  play decisions belong to the enhancer, which re-applies the figure's pinned state to the
  now-visible clip on `qmd:themechange`.

Discoverability: a **subtle CSS play-glyph overlay** (decorative, `aria-hidden`) shown when
paused, hidden when playing, so the video reads as interactive without a full control bar.
Added to `base.css` under the existing `.tali-video` block; the glyph is a pseudo-element on
`.tali-video` gated by a `data-playing` attribute the enhancer toggles.

### Part (b) — single active player (global, cross-type)

One document-level **capture-phase** `play` listener, installed exactly once (idempotence
guard), that pauses every *other* `<audio>` / `<video>` element when any one starts. Global
and cross-type: starting a video pauses all audio and vice versa → at most one media element
plays anywhere.

### Home: one code-enhance fragment

All JS lives in a new `crates/core/assets/js/code-enhance/18-media.js`, appended to the
`CODE_ENHANCE_JS` `concat!` in `render/mod.rs`. Because `code-enhance.js` ships on every
non-bare page (build **and** preview) and its registry re-runs enhancers on each block
re-mount, this one fragment covers both surfaces — no `web-client/client.js` edit. The
single-player listener self-installs once (top-level IIFE, guarded); the per-video wiring is a
registered enhancer (idempotent, re-run on mount). The
`code_enhance_bundle_matches_fragments_in_order` guard test gains the new entry.

### Interactions preserved

- **Lightbox** (`11-lightbox.js` `openVideo`): unchanged; the enlarged copy plays via the
  same `play` event, so the single-player coordinator pauses the inline clip when the
  lightbox opens — desirable.
- **Theme pair lazy fetch**: still exactly one clip downloads (src-promotion stays in
  `syncThemeVideos`).

## Testing

- **Rust pins** (`render/tests.rs`): rewrite `video_shortcode_emits_a_framed_autoplaying_screencast`
  → asserts **no** unconditional `autoplay`, presence of `muted loop playsinline`,
  `preload="metadata"`, `tabindex="0"`, and an `aria-label`. `video_dark_and_poster_and_caption_args_are_exercised`
  keeps the lazy-`data-src` assertions, adds the new markers.
- **Fragment guard**: `code_enhance_bundle_matches_fragments_in_order` includes `18-media.js`.
- **Corpus invariants**: `cargo test -p taliesin-core` (block-id/sourcepos untouched; video is
  raw-HTML passthrough).
- **Browser** (headless puppeteer-core — the chrome-devtools MCP profile was held by a
  parallel session — real page + generated media, 3 viewports mobile ~390 / laptop ~1440 /
  portrait ~900): no `autoplay` + paused on load; hover plays / leave pauses; keyboard focus
  plays inline / blur pauses; click opens the lightbox (playing) which pauses the inline clip;
  audio↔audio and video↔audio single-player; reduced-motion suppresses hover-play but an
  explicit click still plays via the lightbox. Badge visible when paused, gone when playing.
  (16/16 assertions green.)

## Scope / invariants

Read-only preview preserved (no write-back). No new output format. No CDN / offline-safe
(pure inline JS/CSS). Block model untouched (video is passthrough raw HTML). Fragment
concat-order guard keeps the bundle honest.
