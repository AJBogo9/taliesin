# Marketing site

The qmd-fast landing site, **built by qmd-fast itself** with **nothing but Markdown
+ YAML — no custom CSS**. It's the framework's own dogfood test: if this looks good
on the defaults, the framework is doing its job.

Pages: `index.qmd` (landing), `features.qmd`, `formats.qmd`, plus `demo.qmd` (the
showcase deck, embedded via `{{< embed >}}` and kept out of nav). Config is
`_quarto.yml` (native flat schema). There is intentionally no stylesheet.

> Placeholders to update before going live: `url:` and the GitHub links in
> `_quarto.yml` (currently `https://qmd-fast.dev` and `github.com/AJBogo9/qmd-fast`).

## How it's authored (all framework features)

- **Hero** — the `hero:` front-matter block (`eyebrow` / `headline` / `lead` /
  `actions`) renders the top of each page. No HTML.
- **Sections** — plain `##` headings + prose, the way any qmd-fast doc reads.
- **Card grids** — `::: {.feature-grid}` with `::: {.feature}` cards (fenced divs).
- **Screencasts** — `{{< video light.mp4 dark=dark.mp4 caption="…" >}}` (built-in
  shortcode; the clip matching the page theme plays, swapping live on toggle).
- **Live deck** — `{{< embed demo.qmd >}}` (built-in shortcode).
- **Buttons** — Pandoc attributes: `[Text](href){.btn .btn-primary .btn-lg}`.
- **Closing CTA** — a `::: {.hero}` fenced div.

The theme (serif body, sans headings, light/dark toggle) is the qmd-fast default.

## Preview

```sh
qmd-fast preview site
```

The `Guide` and `Internals` nav links resolve live: `_quarto.yml`'s `mounts:` serves
the two sibling docs books under `/docs/guide/` and `/docs/internals/` (rendered on
request, so content edits show on refresh).

## Build (single-tree: site at root, two docs books under /docs)

```sh
qmd-fast build site            --out _site
qmd-fast build docs/guide      --out _site/docs/guide
qmd-fast build docs/internals  --out _site/docs/internals
```

Deploy `_site/` to any static host with directory indexing.

## The screencasts

`assets/{live-edit,live-code}-{light,dark}.mp4` are produced by the scripted
recorder (non-destructively) from demo specs. The optional 3rd arg picks the theme
and suffixes the output, so one spec records both variants:

```sh
cd tools/record-demo
for clip in live-edit live-code; do
  for theme in light dark; do
    QMD_FAST_PYTHON=<py-with-numpy+matplotlib> node record.mjs demos/$clip.mjs $theme
  done
done
cp out/live-edit-light.mp4 out/live-edit-dark.mp4 \
   out/live-code-light.mp4 out/live-code-dark.mp4 ../../site/assets/
```

`demo.qmd` is a copy of `docs/demo.qmd`; re-copy it if the showcase deck changes.
