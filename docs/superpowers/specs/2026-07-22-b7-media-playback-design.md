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

### Part (a) — video: user-initiated play, one unified primitive

Playback is never automatic. One play/pause primitive, driven by three inputs so desktop,
keyboard, and touch share a single code path (no IntersectionObserver):

- **Hover** (pointer): `pointerenter` plays, `pointerleave` pauses — unless *pinned* (below).
- **Focus** (keyboard): the `<video>` is `tabindex="0"` with an `aria-label`; `focus` plays,
  `blur` pauses — keyboard parity with hover.
- **Click / tap / Enter / Space**: toggles a **pinned** play state stored on the `<figure>`.
  Pinned-playing persists through leave/blur; this is the touch fallback *and* the explicit
  WCAG pause mechanism. Tap again to pin-pause.
- **`prefers-reduced-motion: reduce`**: hover/focus never auto-play; only an explicit
  tap/Enter plays (honors the motion preference, still user-overridable).

State lives on the `<figure>` (not the `<video>`) so a light/dark **pair** shares one pinned
state and it carries across a theme switch.

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
- **Browser** (chrome-devtools, 3 viewports — mobile ~390, laptop ~1440, portrait ~900):
  hover plays / leave pauses; tab-focus plays; tap pins; reduced-motion suppresses auto-play;
  starting one clip pauses the fourier audios and vice versa.

## Scope / invariants

Read-only preview preserved (no write-back). No new output format. No CDN / offline-safe
(pure inline JS/CSS). Block model untouched (video is passthrough raw HTML). Fragment
concat-order guard keeps the bundle honest.
