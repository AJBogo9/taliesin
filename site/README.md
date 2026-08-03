# Marketing site

The Taliesin landing site, **built by Taliesin itself** with **nothing but Markdown
+ YAML — no custom CSS**. It's the framework's own dogfood test: if this looks good
on the defaults, the framework is doing its job.

Pages: `index.tmd` (landing), `features.tmd`, `formats.tmd`, plus `demo.tmd` (the
showcase deck, embedded via `{{< embed >}}` and kept out of nav). Config is
`_site.yml` (native flat schema). There is intentionally no stylesheet.

> Placeholders to update before going live: `url:` and the GitHub links in
> `_site.yml` (currently `https://taliesin.dev` and `github.com/AJBogo9/taliesin`).

## How it's authored (all framework features)

- **Hero** — the `hero:` front-matter block (`eyebrow` / `headline` / `lead` /
  `actions`) renders the top of each page. No HTML.
- **Sections** — plain `##` headings + prose, the way any Taliesin doc reads.
- **Card grids** — `::: {.feature-grid}` with `::: {.feature}` cards (fenced divs).
- **Screencasts** — `{{< video light.mp4 dark=dark.mp4 caption="…" >}}` (built-in
  shortcode; the clip matching the page theme plays, swapping live on toggle).
- **Live deck** — `{{< embed demo.tmd >}}` (built-in shortcode).
- **Buttons** — Pandoc attributes: `[Text](href){.btn .btn-primary .btn-lg}`.
- **Closing CTA** — a `::: {.hero}` fenced div.

The theme (serif body, sans headings, light/dark toggle) is the Taliesin default.

## Preview

```sh
taliesin preview site
```

The `Guide` and `Internals` nav links resolve live: `_site.yml`'s `mounts:` serves
the two sibling docs books under `/docs/guide/` and `/docs/internals/` (rendered on
request, so content edits show on refresh).

## Build (single-tree: site at root, two docs books under /docs)

```sh
taliesin build site --out _site   # the whole tree: this project + all 7 mounts
```

One command, eight projects: the parent, then one per `mounts:` entry into `_site/<at>/`.
There is nothing else to run and nothing to keep in step with `_site.yml`.

This used to need a shell script beside this file, because `mounts:` was a *preview*
feature and a bare `taliesin build site` produced a tree whose Guide, Internals and gallery
links 404'd — including the landing page's primary call to action (item 149). `build` walks
the mounts itself now; the script and its `TAL-MOUNT-PREVIEW` diagnostic are both gone.

**The parent is built before the mounts, and that order is load-bearing** — the parent build
sweeps stale output, deleting anything under the output directory it did not itself write,
so a mount built first would be silently swept away. `build` does this in the right order and
`crates/server/tests/mount_static_build.rs` pins it by building twice into one directory.

The `analyst` exhibit is the only one whose pages **execute**, and in two languages: it
needs a python with `ipykernel` (`TALIESIN_PYTHON`) plus `pandas`/`matplotlib`, and an R
with `IRkernel` (`TALIESIN_R`) plus `readr`/`dplyr`/`broom`/`ggplot2`/`patchwork`/`knitr`.
Without them its figures and tables build as "cell did not run" placeholders.

Deploy `_site/` to any static host with directory indexing.

## The screencasts

`assets/{live-edit,live-code}-{light,dark}.mp4` are produced by the scripted
recorder (non-destructively) from demo specs. The optional 3rd arg picks the theme
and suffixes the output, so one spec records both variants:

```sh
cd tools/record-demo
for clip in live-edit live-code; do
  for theme in light dark; do
    TALIESIN_PYTHON=<py-with-numpy+matplotlib> node record.mjs demos/$clip.mjs $theme
  done
done
cp out/live-edit-light.mp4 out/live-edit-dark.mp4 \
   out/live-code-light.mp4 out/live-code-dark.mp4 ../../site/assets/
```

`demo.tmd` is a copy of `docs/guide/demo.tmd`; re-copy it if the showcase deck changes
(`cp ../docs/guide/demo.tmd demo.tmd`). The stale `docs/demo.tmd` path this line used to
name does not exist, which is how the two copies drifted: the guide's copy was corrected
and the marketing copy went on advertising a PDF export, a "reader" mode and a pen tool
the engine has never had. `crates/core/tests/stale_docs.rs` now gates both copies.
