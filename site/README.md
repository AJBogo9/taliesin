# Marketing site

The Taliesin landing site, **built by Taliesin itself** with **nothing but Markdown
+ YAML — no custom CSS**. It's the framework's own dogfood test: if this looks good
on the defaults, the framework is doing its job.

Pages: `index.tmd` (landing) and `showcase.tmd` (the live demos). Config is
`_site.yml` (native flat schema). There is intentionally no stylesheet. The gallery moved
out to its own project and domain on 2026-08-16 (`gallery/`, see below).

`features.tmd` and `formats.tmd` were cut on 2026-08-19. Between them, 11 of 16 feature
boxes restated `index.tmd`, and `formats.tmd` was `index.tmd`'s "three shapes" section
retold at 2.5x length, section for section. The three facts they uniquely held
(click-to-source, bundled-not-fetched, host-anywhere output) moved into `index.tmd`, and
the "Get started" button that only `formats.tmd` carried is now the hero's primary action.
Anything enumerative belongs one click away on guide.taliesin.sh, not on the landing site.

> Placeholder to update before going live: the GitHub links in `_site.yml`
> (currently `github.com/AJBogo9/taliesin`). `url:` is now the registered
> `https://taliesin.sh`.

## How it's authored (all framework features)

- **Hero** — the `hero:` front-matter block (`eyebrow` / `headline` / `lead` /
  `actions`) renders the top of each page. No HTML.
- **Sections** — plain `##` headings + prose, the way any Taliesin doc reads.
- **Ruled sections** — `::: {.feature-list}` with `::: {.feature}` children (fenced divs).
  Not cards: `.feature-grid` was retired with the card era and nothing styles it, so a page
  still authoring it renders a bare `<div>`. A gate in `render/tests.rs` pins that.
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

Every link on every page of this project resolves in `preview site` exactly as it does in
the deploy. The `Guide`, `Internals` and `Gallery` entries are **absolute URLs** into three
separate sites, so they leave this one rather than pointing at a prefix nothing serves.

## Build and publish

```sh
taliesin build site              # this project alone -> site/_site
tools/publish.sh site            # check, build and deploy just this one
tools/publish.sh                 # all four sites
tools/publish.sh --check         # the gate: --no-exec, temp dirs, nothing deployed
```

**Four projects, four Cloudflare Pages projects, four domains** (2026-08-16):

| Source | Pages project | URL |
| --- | --- | --- |
| `site/` | `taliesin-site` | taliesin.sh |
| `docs/guide/` | `taliesin-guide` | guide.taliesin.sh |
| `docs/internals/` | `taliesin-internals` | internals.taliesin.sh |
| `gallery/` | `taliesin-gallery` | gallery.taliesin.sh |

Cloudflare Pages has no subpath deploy: `wrangler pages deploy <dir>` uploads that
directory as the *entire* site for its project. Putting the Guide under
`taliesin.sh/docs/guide` would therefore mean assembling all four locally and re-uploading
the whole tree on every change. Separate projects cost nothing (100 per account), and each
site then builds, previews and deploys alone.

This replaced two earlier answers: a `mounts:` key inside the tool (cut 2026-08-09) and the
single-tree composition script that followed it (deleted 2026-08-16). Both existed
because a bare `taliesin build site` once produced a tree whose Guide and gallery links
404'd, including the landing page's primary call to action (item 149). Absolute URLs cannot
have that failure by construction, and
`crates/core/tests/cross_site_links.rs` resolves every one of them against the source tree
so a renamed page or a changed `url:` fails `cargo test` rather than a reader's click.

The **gallery** is a flat, self-contained project: five one-page demos plus an index,
nothing composed into its output. `report.tmd` is the only page whose cells **execute**: it
needs a python with `ipykernel` (`TALIESIN_PYTHON`) plus
`pandas`/`numpy`/`scipy`/`matplotlib`. Without them its figures and tables build as "cell
did not run" placeholders.

Deploy any `_site/` to any static host with directory indexing.

## The screencasts

**No page ships one today.** The landing page proved "the output re-runs in place"
with `assets/live-code-dark.mp4` until 2026-08-19, and a screencast is theme-locked
while the page's palette follows the reader's device, so a dark capture rendered as a
black slab for every reader whose OS says light. It was replaced by a real `{python}`
cell, which is the thing itself rather than a recording of it and costs 745 KB less.
The three files still in `assets/` are referenced by nothing and predate the rename
from qmd-fast; the recorder below is kept because the *edit loop* is the one thing a
static build genuinely cannot show, so a light-and-dark pair may earn its place again.

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
