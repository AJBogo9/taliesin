# record-demo

Record a qmd-fast **live preview** to an optimized **MP4 + GIF**, fully scripted —
no manual screen capture or editing. It starts `qmd-fast preview`, drives a real
Google Chrome through a demo you define (Playwright `recordVideo` → a smooth
`.webm`), then ffmpeg-encodes the deliverables.

```sh
cd tools/record-demo
npm install                       # once: pulls playwright-core (no browser download)
node record.mjs demos/sample.mjs  # → out/sample.mp4 + out/sample.gif
```

## How it works

- **Recording** is Playwright's built-in `recordVideo` driving the **system Google
  Chrome** (`channel: "chrome"`), so there is no Chromium download and WebGL/OJS
  content renders for real.
- **Encoding** is ffmpeg: an H.264 **MP4** (small, crisp) and a palette-optimized
  **GIF**.
- It's **non-destructive**: a demo that edits its `.tmd` (to show the live-update
  beat) restores the file afterward.

Requirements: Node 18+, `ffmpeg`, Google Chrome, and a built `qmd-fast` binary
(found automatically at `target/release|debug/qmd-fast`, or set `QMD_FAST=<path>`).
Pass `QMD_FAST_PYTHON` / `QMD_FAST_R` through the env if the demo doc runs cells.

## Writing a demo

A demo is a small ES module under `demos/` (see [`demos/sample.mjs`](demos/sample.mjs)):

```js
export default {
  name: "sample",                          // output basename
  doc: "demos/sample.tmd",                 // .tmd or a site dir, relative to this folder
  viewport: { width: 1000, height: 720 },
  theme: "dark",                           // "dark" | "light"
  gif: { fps: 14, width: 760, clip: [13.5, 17.5] }, // omit to skip the GIF
  async steps(page, { sleep, smoothScroll, editDoc }) {
    await sleep(1200);
    await smoothScroll(0.55, 5000);        // eased scroll to 55% over 5s
    await editDoc((src) => src.replace("…", "… ✨")); // triggers a live block update
    await sleep(2200);
  },
};
```

`steps` gets the Playwright `page` plus helpers: `sleep(ms)`,
`smoothScroll(fraction, ms)`, and `editDoc(src => newSrc)` (writes the doc so the
preview hot-reloads — the headline feature). Anything else is the full Playwright
API (`page.click`, `page.hover`, …).

## MP4 vs GIF (pick the right one)

- **MP4 → the web/docs.** Tiny and sharp. Embed it autoplaying and silent:
  ```html
  <video src="demo.mp4" autoplay muted loop playsinline></video>
  ```
- **GIF → GitHub READMEs** and places that can't embed video. GIFs balloon on long
  scrolling, so keep them **short**: set `gif.clip: [start, end]` (seconds) to trim
  the GIF to the one moment that matters while the MP4 keeps the whole demo. The
  encoder warns if a GIF exceeds ~5 MB.

Output lands in `out/` (git-ignored). Copy the artifact where you need it, e.g.
`docs/assets/`, and reference it from a page.
