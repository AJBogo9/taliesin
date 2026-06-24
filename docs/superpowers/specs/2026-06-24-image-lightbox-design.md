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
2. **`img.lightbox` selector (forward-compat).** The click target + zoom-in cursor +
   dblclick guard also match `img.lightbox`. NOTE (verified empirically): a *bare*
   `![](x){.lightbox}` does NOT currently carry the class — the lone-decorative-image path
   emits `<img>` and leaks the `{.lightbox}` text. Making bare images opt in needs a
   server-side change to apply attr classes to a captionless image; no corpus doc needs it,
   so it's **deferred**. Captioned/labeled images are already `<figure>`s (already zoom).
   The selector is kept as harmless forward-compat for when that server change lands.

## Pin + verify

- `corpus/media/gallery.qmd` — a `::: {layout-ncol=3}` grid of 3 labeled `@fig-` figures
  (self-contained images under `corpus/media/`); README row.
- Verify in-browser: clicking a figure opens the lightbox; ←/→ steps through all three
  with the `(n / N)` counter; Esc closes.

## Invariants

Enhancer-only, read-only (never writes source); set up once via the existing document-level
delegation, so it survives block swaps; no block-model/HTML/CSS-layout change; `deck.css`
untouched. No new dependency.

## Out of scope (YAGNI)

WebP/AVIF transcode + responsive `srcset` (already deferred to the backlog "Image
optimization" item); swipe/touch gestures; a dedicated gallery container syntax
(`layout-ncol` + figures already compose one).
