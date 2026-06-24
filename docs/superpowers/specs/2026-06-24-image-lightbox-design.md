# Design: `image-lightbox` (Wave 3 feature, thin completion)

Status: approved 2026-06-24. Branch `feat/image-lightbox`. Roadmap: `BEYOND-QUARTO.md`
Pillar IV. Enhancer-only; read-only; no Rust, no HTML/block-model change.

## Finding (scope)

The lightbox is **already shipped**: `qmdInitLightbox` (code-enhance.js) makes every
`figure img`, `pre.mermaid`, and `.qmd-video video` click-to-zoom with its caption. Every
real document image (captioned or `#fig-` labeled) is a `<figure>`, so already zoomable.
Re-shipping zoom would be redundant. The valuable increment is **gallery navigation**.

## Changes (`crates/core/assets/js/code-enhance.js`, `qmdInitLightbox`)

1. **Gallery nav.** On open, collect the ordered list of zoomable images on the page
   (`figure img, img.lightbox`) and remember the opened index. **←/→** step prev/next
   (wrapping), swapping the lightbox `<img>` + caption in place; Esc still closes. When the
   gallery has >1 image, append a `(n / N)` counter to the caption. Mermaid/video stay
   single-open (galleries are images only); arrows are ignored unless an image is shown.
2. **`.lightbox` opt-in (truthfulness).** Extend the click target + zoom-in cursor +
   dblclick guard to also match `img.lightbox`, so a non-figure image can opt in (figures
   keep working unchanged).

## Pin + verify

- `corpus/media/gallery.qmd` — a `::: {layout-ncol=3}` grid of 3 labeled `@fig-` figures
  + one `{.lightbox}` image; README row.
- Verify in-browser: clicking a figure opens the lightbox; ←/→ steps through all images
  with the counter; Esc closes; an `img.lightbox` opens too.

## Invariants

Enhancer-only, read-only (never writes source); set up once via the existing document-level
delegation, so it survives block swaps; no block-model/HTML/CSS-layout change; `deck.css`
untouched. No new dependency.

## Out of scope (YAGNI)

WebP/AVIF transcode + responsive `srcset` (already deferred to the backlog "Image
optimization" item); swipe/touch gestures; a dedicated gallery container syntax
(`layout-ncol` + figures already compose one).
