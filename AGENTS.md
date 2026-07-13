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
- **Front matter:** a leading `---` YAML block. Keys: `title`, `subtitle`, `author`, `date`, `description`, `lang`, `categories`, `image`, `image-alt`, `format`, `theme`, `css`, `page-layout`, `draft`, `title-block-style`, `include-in-header`, `include-before-body`, `include-after-body`, `toc`, `bibliography`, `csl`, `execute`, `listing`, `about`, `hero`, `prose-lint`, `theorems`.
