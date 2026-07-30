# AGENTS.md

Guidance for AI coding agents authoring a [Taliesin](https://github.com/AJBogo9/taliesin) project. Taliesin renders `.tmd` files to HTML (blog posts, slide decks, books, multi-page sites). This file is generated from the live validator vocabulary, so its dialect list cannot drift from what `taliesin check` enforces.

## Edit the `.tmd`, never the preview

The `.tmd` file is the single editing surface. The browser preview is a read-only view: edits flow one way, source -> preview, and the preview must never be treated as writable. Make every change in the `.tmd` source; **never the preview**.

## Gate every change on `check`

After editing, validate with `taliesin check <file-or-dir> --format json`. It prints an object `{ "diagnostics": [...], "environment": {...} }`; a non-empty `diagnostics` array (and a non-zero exit code) means the document has problems to fix. This is the machine-readable gate an agent drives instead of opening a browser:

```sh
taliesin check index.tmd --format json
```

To *read what you made* without a browser, `taliesin read <file>` projects the rendered document to plain text (headings, resolved "Figure N"/cross-reference numbers, callouts, fenced code, math as TeX) — the agent's substitute for looking at the preview.

Add `--run` to also execute the `{python}`/`{r}` cells and report what each produced (`[figure fig-x: produced, alt "…"]`, `[output: …]`, `[cell error: …]`), so you can confirm a computed figure actually baked without opening a browser; `taliesin read --run <file> --format json` gives the same per-cell result structured.

A `{js}`/Observable-Plot cell runs in the browser, so with `--run` Taliesin also drives a local headless Chrome over the built page and reports whether each `{js}` chart painted (`[js: produced, <svg W×H>]`, or `[js error: …]` when it threw). With no local Chrome available it degrades to `[js: skipped (chrome unavailable)]`, never a failure.

## Discover the surface

Three read-only commands describe what Taliesin accepts, so an agent never has to guess:

- `taliesin vocab` -> every closed-set construct (front-matter keys, cell options, callout/theorem kinds, div classes, cross-reference prefixes) as JSON.
- `taliesin schema` -> the JSON Schema for front matter and `_site.yml`.
- `taliesin symbols <file>` -> the headings, figures, and cross-reference targets in a document.

## Build and publish

- `taliesin build <file-or-dir>` -> self-contained HTML (a single file, or a `_site/` folder for a multi-page project).
- `taliesin preview <file-or-dir>` -> a live-reloading dev server (for a human; an agent uses `check` + `build`).

## Dialect

Taliesin's Markdown is Pandoc-flavored. The closed sets below come straight from the validator (run `taliesin vocab` for the full list with descriptions):

- **Callouts:** a fenced div `::: {.callout-note}` opens a callout. Kinds: `note`, `tip`, `warning`, `important`, `caution`.
- **Code cells:** a fenced ` ```{python} ` (or `{r}`, `{js}`) block runs live. In-cell options use a `#| key: value` comment, e.g. `#| label: fig-scree` or `#| echo: false`. Options: `echo`, `include`, `cache`, `label`, `fig-cap`, `lst-cap`, `tbl-cap`, `fig-export`, `code-fold`, `code-summary`, `code-line-numbers`, `name`, `viewof`, `input`.
- **Cross-references:** cite a labelled target with `@`-prefixes (`fig`, `tbl`, `sec`, `eq`, `lst`, `thm`, `lem`, `cor`, `prp`, `def`, `exm`, `rem`); e.g. `@fig-scree` renders as "Figure 3".
- **Citations:** `[@key]` cites a `.bib` entry declared in the `bibliography:` front matter.
- **Structural divs:** `::: {.class} ... :::` blocks. Classes: `panel-tabset`, `code-walkthrough`, `scrolly`, `magic-move`, `step`, `column-margin`, `aside`, `sidenote`, `marginnote`.
- **Front matter:** a leading `---` YAML block. Keys: `title`, `subtitle`, `author`, `date`, `description`, `lang`, `categories`, `image`, `image-alt`, `footer`, `logo`, `format`, `theme`, `css`, `page-layout`, `draft`, `title-block-style`, `include-in-header`, `include-before-body`, `include-after-body`, `toc`, `bibliography`, `execute`, `listing`, `hero`, `prose-lint`, `theorems`, `datasets`.

## Recipes

Worked idioms the closed-set `vocab` can't express. Each is kept byte-identical to a real, `check`-clean corpus document, so it stays runnable.

**A figure from a CSV** (the one data idiom worth learning from an example): read a data file, plot it, and give the cell a `fig-`-prefixed `#| label:` so its output becomes a numbered, `@fig-`-referenceable figure. Keep the data beside the `.tmd`:

~~~~
```{python}
#| label: fig-sales
#| fig-cap: "Monthly sales from `data.csv`."
import pandas as pd
import matplotlib.pyplot as plt

data = pd.read_csv("data.csv")
fig, ax = plt.subplots()
ax.plot(data["month"], data["sales"], marker="o")
ax.set_xlabel("month")
ax.set_ylabel("sales")
```
~~~~

Then reference it in prose: `@fig-sales shows the trend.` For the R kernel, swap `{python}` + pandas for `{r}` + readr (`read_csv("data.csv")`); the `#| label:` and `@fig-` reference are identical.
