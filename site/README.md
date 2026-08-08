# Marketing site

The Taliesin landing site, **built by Taliesin itself** with **nothing but Markdown
+ YAML — no custom CSS**. It's the framework's own dogfood test: if this looks good
on the defaults, the framework is doing its job.

Pages: `index.tmd` (landing), `features.tmd`, `formats.tmd`, `showcase.tmd` and
`gallery.tmd`. Config is `_site.yml` (native flat schema). There is intentionally no
stylesheet.

> Placeholder to update before going live: the GitHub links in `_site.yml`
> (currently `github.com/AJBogo9/taliesin`). `url:` is now the registered
> `https://taliesin.sh`.

## How it's authored (all framework features)

- **Hero** — the `hero:` front-matter block (`eyebrow` / `headline` / `lead` /
  `actions`) renders the top of each page. No HTML.
- **Sections** — plain `##` headings + prose, the way any Taliesin doc reads.
- **Card grids** — `::: {.feature-grid}` with `::: {.feature}` cards (fenced divs).
- **Screencasts** — a hand-written `<video>` in a `<figure class="tali-figure">`, which
  base.css frames and captions. One clip per slot: the `{{< video >}}` shortcode and its
  theme-matched light/dark pair were retired on 2026-08-08.
- **Live graphics** — `{js}` cells (the spinnable surface on the landing page, the
  reactive plots and the Lorenz attractor on `showcase.tmd`).
- **Buttons** — Pandoc attributes: `[Text](href){.btn .btn-primary .btn-lg}`.
- **Closing CTA** — a `::: {.hero}` fenced div.

The theme (serif body, sans headings, light/dark toggle) is the Taliesin default.

## Preview

```sh
taliesin preview site
```

The `Guide`, `Internals` and gallery nav links point into projects this one does not
contain, so `preview site` shows them as links that go nowhere and `build site` reports
each as a broken link. That is true of `site/` read on its own; the composed deploy is
what makes them resolve.

## Build (single-tree: site at root, docs and gallery under it)

```sh
tools/build-site.sh              # the whole tree -> site/_site
tools/build-site.sh --check      # the gate: --no-exec, temp dir, links asserted
```

Seven projects: this one, then each sub-project into `_site/<prefix>/`. **The parent is
built first, and that order is load-bearing**: the parent build sweeps stale output,
deleting anything under the output directory it did not itself write, so a sub-project
built first would be silently swept away.

`_site.yml` used to carry a `mounts:` key that did this from inside the tool (cut
2026-08-09). Before that it was a shell script, and a bare `taliesin build site` produced
a tree whose Guide, Internals and gallery links 404'd, including the landing page's
primary call to action (item 149). The script is back, so it is wired into
`.githooks/pre-push` and it **asserts** every cross-project link against the composed
output rather than trusting that it built the right thing.

The `analyst` exhibit is the only one whose pages **execute**: it needs a python with
`ipykernel` (`TALIESIN_PYTHON`) plus `pandas`/`numpy`/`scipy`/`matplotlib`. Without them
its figures and tables build as "cell did not run" placeholders.

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
