# Image optimization (backlog item 169)

**Date:** 2026-07-30
**Item:** P1 #169, "WebP/AVIF transcode + responsive `srcset` + lazy-load behind a
content-hashed asset cache". Flagged publish-critical by the author on 2026-07-29.

## The problem, measured

28 tracked raster images, **1.47 MB** total. The two worst are `site/assets/og-card.png`
at 294,692 B and `corpus/tech-blog/projects/supercollider-mcp/thumbnail.png` at 257,012 B.
No page emits `srcset`, `<picture>`, `width`/`height`, or (outside listing cards)
`loading="lazy"`. The author is already hand-converting: six `.webp` twins exist and are
referenced directly (`image: "thumbnail.webp"`), which is exactly the toil to remove.

## Codec ruling: AVIF only, not "WebP/AVIF"

The item's title assumes both are available. Measured, they are not:

- **`image-webp` (pure Rust, +4 crates) encodes lossless only** — its own README says so.
  Lossless WebP on a photo is larger than the JPEG it replaces.
- **Lossy WebP requires `libwebp` (C)** via the `webp` crate (+9 crates). It vendors C
  source and needs a C toolchain on every build machine.
- **AVIF encodes pure-Rust** through `image`'s `avif` feature (+45 crates, rav1e).

Cost of the pure-Rust route, measured rather than assumed: a cold `--release` build of
`image` + AVIF encode is **21.8 s at `-j4`** (verified a 7.6 MB `librav1e.rlib` was
genuinely produced, not cache-served). That is ~17% on top of the 2m11s cold build item
148 records — real, but far below the "rav1e takes minutes" assumption that would have
decided this the other way.

**Trap found while measuring, and it decides the seam.** Depending on `ravif` *directly*
turns on rav1e's `asm` feature, which fails the build with `NASM build failed. Make sure
you have nasm installed`. That would break `cargo install` and the macOS release runners.
`image`'s own `avif` feature configures rav1e without asm, which is why the 21.8 s build
succeeded on a machine with no nasm. **Go through `image`; never depend on `ravif`
directly.**

Compression on the real corpus files (`image`'s `AvifEncoder`, quality 72):

| image | original | speed 4 | speed 8 | speed 10 |
|---|---|---|---|---|
| `og-card.png` 1200×630 | 294,692 | **21,651 B / 922 ms** | 27,468 / 210 ms | 42,003 / 84 ms |
| `supercollider/thumbnail.png` 1188×746 | 257,012 | **31,946 B / 970 ms** | 35,814 / 348 ms | 47,136 / 113 ms |
| `a-star/astar.png` 637×368 | 28,089 | **7,172 B / 606 ms** | 9,440 / 159 ms | 19,556 / 61 ms |

Decode is ~3.6 ms and irrelevant. **Speed 4, quality 72** is chosen: it is ~7.3% of the
original PNG, and at ~1 s per image it is only viable *behind a cache*, which is what the
item asks for anyway. Speed 10 is nearly 2× the bytes for the same pixels — not worth it.

**This measurement also kills an architecture.** ~1 s per image means the preview cannot
transcode on demand: a six-image page would cost ~6 s on first load. Any design where the
preview produces derivatives is dead.

## Architecture: split by what each side can know

Three approaches were considered.

- **(A) Build-time post-process only.** The build rewrites `<img>` → `<picture>` and core
  is untouched. Rejected: the preview then shows an image with no `width`/`height`, so
  **layout shift becomes invisible until you build**, and the preview stops predicting the
  page. That is the property this tool exists for.
- **(B) Core emits everything, preview transcodes on demand.** Exact parity, dead on the
  ~1 s/image measurement above.
- **(C) Split — chosen.** Core emits what a *render* can know; the build adds what only a
  *build* can cache.

Under (C):

- **Core** reads the image **header only** (`image::image_dimensions`, microseconds — no
  decode) and emits intrinsic `width`/`height` plus `loading`/`decoding` on every local
  raster `<img>`.
- **The build** additionally encodes AVIF rungs into a content-hashed cache and *wraps* the
  byte-identical `<img>` in `<picture><source type="image/avif" srcset=… sizes=…>`.

The parity story is exact and checkable: **the `<img>` tag is byte-identical in preview and
build**, and `<picture>` is a non-rendering wrapper, so both lay out identically. The only
difference is which bytes the browser chooses to fetch — a strictly additive difference
that cannot move a pixel.

## Where core hooks in

`render/mod.rs:1126` already establishes the pattern: `html = shift_heading_html(&html, …)`,
a post-emission transform on a block's HTML before it becomes a `Block`, with `base_dir` in
scope. The annotation goes beside it.

This is deliberately **not** done in the two emitters (`emit.rs:118` inline,
`figure.rs:104` figure). Those are free functions with no `base_dir`, threading it through
`emit_node`/`emit_children` would touch every recursive call site, and one post-emission
pass covers both paths uniformly with the same scanner shape the build's `local_refs`
already uses.

`data-block-id` is a content hash of the **source**, so annotating emitted HTML leaves it
stable — the block model, the diff and click-to-source are untouched.

### What is annotated

A local, relative, existing raster file (`.png`, `.jpg`/`.jpeg`, `.gif`, `.webp`, `.avif`)
resolved against `base_dir`. Skipped: absolute URLs, `data:` URIs (which is what executed
`{python}`/`{r}` figure output is), `.svg` (no reliable intrinsic pixel size), anything
already carrying a `width=` attribute, and any path that does not resolve to a file.

### The LCP exception

`loading="lazy"` on an above-the-fold image *delays* LCP — it is an anti-pattern on the
first image, not an optimization. **The first local raster image in a document is emitted
eager with `fetchpriority="high"`; every later one is `loading="lazy" decoding="async"`.**
The walk is already sequential over blocks, so this costs one counter.

## CSS prerequisite (found by reading, would have shipped as a visible bug)

`base.css:510` is `img { max-width: 100%; }` with **no `height: auto`**. Adding `width`
and `height` attributes to an image whose CSS constrains only its width makes the browser
honour the attribute height against a shrunken width — **every inline image narrower than
its intrinsic size would render distorted**. The rule gains `height: auto`.

`figure.tali-figure img` (`base.css:848`) already has it, and `.tali-output img`
(`base.css:792`) is `object-fit: contain` over `data:` URIs the annotator skips, so both
were already safe. Only the bare rule is wrong.

## Width rungs, and the upscaling trap

Rungs are **480, 960, and the source's native width**, deduped, with **every rung at or
above the native width dropped except the native one**.

That rule comes from a measurement, not from taste: re-encoding `astar.png` (native 637 px)
at an 800 px rung produced 12,274 B against 4,798 B at native — **44% larger than the
original file it was meant to shrink**. Never emit a rung above the source width.

`sizes` is `(max-width: 46rem) 100vw, 736px`, matching `--tali-maxw: 46rem`.

## The cache

`_freeze/img/<key>.avif`, reusing the existing gitignored persistent-cache convention that
`crates/server/src/freeze.rs` established for cell outputs.

The key is a hash of **source bytes + target width + quality + speed + an encoder-version
tag**. Source bytes rather than mtime, so a `git checkout` does not invalidate; the encoder
tag so a codec upgrade re-encodes rather than serving stale bytes. This is the same
"cumulative content hash, no stale hits, nothing to clear by hand" property the freeze cache
already guarantees.

## Corpus pin

`corpus/images/` — one document exercising a captioned `<figure>`, a bare inline image, an
image **below the smallest rung** (the upscaling trap), and an executed-cell `data:` figure
that must be left alone.

The corpus walker renders every corpus doc on every `cargo test`, so the pin must not make
the suite transcode: it will not, because **transcoding is build-only**. The walker pays
only the header read. Build-path tests use the smallest fixture that proves the behaviour.

## Testing

- Core annotation: unit tests per skip-rule, plus the LCP first-image exception.
- The CSS rule: a token/contract-style assertion that the bare `img` rule carries
  `height: auto`, since its absence is silent and visual.
- Cache: same key → no re-encode; changed source bytes / width / encoder tag → re-encode.
- Rungs: a below-rung source emits exactly one rung and never upscales.
- Build: `<picture>` wraps a byte-identical `<img>`.
- **Verified by mutation** (restore the bug, watch the named test fail), per the standing
  rule, and browser-verified for the layout half.

**Needle the full emitted tag, never a whole-page `contains()`.** Every Taliesin page
inlines `base.css`, which will now contain the string `height: auto` — a page-level
`contains("height: auto")` passes on a page with no image at all. This trap is already in
LESSONS.md and has cost this project a debugging round twice.

## Out of scope

- WebP output (no pure-Rust lossy encoder; see the codec ruling).
- Re-encoding to a *smaller original* — the fallback `<img src>` always stays the author's
  file, so a browser without AVIF gets exactly what it gets today.
- SVG optimization.
- The preview producing derivatives (dead on measurement).
