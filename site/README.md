# Marketing site

The qmd-fast landing site, **built by qmd-fast itself** (a website project). Three
pages — `index.qmd` (landing), `features.qmd`, `formats.qmd` — plus `demo.qmd` (the
showcase deck, embedded via `{{< embed >}}` and kept out of nav). Styling is
`marketing.css` (a "clean dev-tool" layer over the base theme); config is
`_quarto.yml` (native flat schema).

> Placeholders to update before going live: `url:` and the GitHub links in
> `_quarto.yml` / the page CTAs (currently `https://qmd-fast.dev` and
> `github.com/AJBogo9/qmd-fast`).

## Preview

```sh
qmd-fast preview site        # live, with hot reload
```

The `Docs` nav item points at `docs/`, which only exists in the built tree (see
below) — in `preview` it 404s until you also serve the docs book.

## Build (single-tree deploy: site at root, docs book at /docs)

```sh
qmd-fast build site --out _site            # marketing pages + demo deck + assets
qmd-fast build docs --out _site/docs       # the docs book under /docs
```

Then deploy `_site/` to any static host. Building the deck (and the docs' code
cells) wants a Python kernel; without one they degrade to source. Serve `_site/`
with directory indexing so `/docs/` resolves to `/docs/index.html`.

## The hero videos

`index.qmd` / `features.qmd` embed two autoplaying clips from `assets/`:

- `live-edit.mp4` — the live block-update loop
- `live-code.mp4` — editing a `{python}` cell, output re-runs in place

They are produced by the scripted recorder, non-destructively, from demo specs:

```sh
cd tools/record-demo
QMD_FAST_PYTHON=<py-with-numpy+matplotlib> node record.mjs demos/live-edit.mjs
QMD_FAST_PYTHON=<...> node record.mjs demos/live-code.mjs
cp out/live-edit.mp4 out/live-code.mp4 ../../site/assets/
```

`demo.qmd` is a copy of `docs/demo.qmd`; re-copy it if the showcase deck changes.
