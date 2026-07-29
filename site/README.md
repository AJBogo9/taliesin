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
./site/build.sh          # -> _site/ at the repo root
./site/build.sh /tmp/out # or an explicit output directory
```

**Use the script, not a bare `taliesin build site`.** `mounts:` is a *preview* feature: the
static build renders this project's own pages and nothing else, so a plain build produces a
tree whose Guide, Internals and every gallery link 404 — including the landing page's
primary call to action. `build` warns about this (and, since item 149, `--strict` fails on
it and `check` reports `TAL-MOUNT-PREVIEW`), but only the script produces a complete tree.

It runs eight builds: the parent, then one per `mounts:` entry into `_site/<at>/`. **That
order is load-bearing** — the parent build sweeps stale output, deleting anything under the
output directory it did not itself write, so a mount built *first* is silently swept away.
For the same reason, re-running `taliesin build site` on its own afterwards puts you back to
the broken tree. `crates/server/tests/site_build_script.rs` fails the suite if the script
and `_site.yml` ever disagree about the mount list.

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
