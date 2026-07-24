# App icons

The bundled Taliesin mark, used for `manifest.webmanifest` when a project supplies no
`icon-192.png` + `icon-512.png` of its own. Rasterized once and committed, so no rasterizer
dependency enters the build.

Regenerate after changing `web-client/favicon.svg` (needs ImageMagick), from the repo root:

    BG=$(grep -o 'rx="14" fill="#[0-9a-fA-F]*"' web-client/favicon.svg | grep -o '#[0-9a-fA-F]*')
    convert -background none -density 1152 web-client/favicon.svg -resize 192x192 \
      -background "$BG" -flatten -depth 8 crates/core/assets/icons/icon-192.png
    convert -background none -density 1152 web-client/favicon.svg -resize 512x512 \
      -background "$BG" -flatten -depth 8 crates/core/assets/icons/icon-512.png
    convert -background none -density 1152 web-client/favicon.svg -resize 410x410 \
      -background "$BG" -gravity center -extent 512x512 -depth 8 \
      crates/core/assets/icons/icon-maskable-512.png

Then look at the three files: the mark must be centred and legible, and the maskable one
must have visible padding on all four sides.

Why each flag:

- `$BG` is read out of the SVG rather than retyped, so the icon background can never drift
  from the favicon's own rounded-rect fill.
- `-density 1152` rasterizes the 64-unit viewBox far above the target size, so the
  downscale is clean.
- `-flatten` fills the rounded corners with the same background. Transparent corners render
  black on an iOS home screen, and `apple-touch-icon` is never masked by the OS.
- The maskable variant renders the mark at 410px inside a 512px canvas, because Android
  crops an adaptive icon to a circle 80% of the icon's width.

`inkscape` would also work, and reads more naturally, but the snap build on this machine
fails with a glibc symbol error.
