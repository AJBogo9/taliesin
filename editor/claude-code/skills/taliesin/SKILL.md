---
name: taliesin
description: Author Taliesin `.tmd` documents (blog posts, slide decks, books, multi-page sites) from a coding agent. Use when editing or creating `.tmd` files, or when the user mentions Taliesin, `taliesin build`, `taliesin check`, or a `.tmd` document. Teaches the edit → check → fix → build loop, the Markdown dialect, and the read-only-preview rule.
---

# Authoring Taliesin documents

Taliesin renders `.tmd` files to HTML (blog posts, slide decks, books, multi-page
sites). Drive it entirely from the CLI: you never need a browser. This skill is the
whole loop.

## The one rule: edit the `.tmd`, never the preview

The `.tmd` file is the single editing surface. The live preview (`taliesin preview`) is a
**read-only** view: edits flow one way, source → preview. Make every change in the `.tmd`
source; never treat the preview as writable.

## The loop

1. **Edit** the `.tmd` source.
2. **Gate** with `taliesin check <file-or-dir> --format json`. It prints
   `{ "diagnostics": [ … ], "environment": [ … ] }`. A non-empty `diagnostics` array (and a
   non-zero exit) means there are problems to fix. Each diagnostic carries a stable `code`
   (e.g. `TAL-XREF-UNDEF`, `TAL-FM-KEY`, `TAL-A11Y-ALT`), a `severity`, a `file`/`line`, a
   `message`, and — for a "did you mean" typo — a structured `suggestion.replacement` you
   can apply directly.
3. **Fix** and re-check until `diagnostics` is empty.
4. **Read** what you made without a browser: `taliesin read <file>` projects the rendered
   document to plain text (headings, resolved "Figure N"/cross-reference numbers, callouts,
   fenced code, math as TeX).
5. **Build** the deliverable: `taliesin build <file-or-dir>` (a directory builds the whole
   site to `_site/`). `taliesin build <file-or-dir> --strict --format json` fails the build
   on any problem and emits the same structured `{diagnostics:[…]}` for CI.

## Discover the surface (never guess)

- `taliesin vocab` — every closed-set construct (front-matter keys, cell options,
  callout/theorem kinds, div classes, cross-reference prefixes) as JSON.
- `taliesin schema` — the JSON Schema for front matter and `_site.yml`.
- `taliesin symbols <file>` — the headings + cross-reference targets in one document.
- `taliesin map <dir>` — the whole-project outline: pages in order, nav, mounts, and the
  cross-reference graph.

## Scaffold

- `taliesin init <dir>` — a minimal previewable site (also writes an `AGENTS.md` onramp).
- `taliesin new <post|page|deck|paper> <slug> --json` — one document, correct on its first
  save; `--json` prints `{kind, slug, created, preview}` so you know exactly what you made.
  `paper` ships a `references.bib` wired to a real `[@key]`.

## The Markdown dialect

Taliesin's Markdown is Pandoc-flavored. The constructs below are the ones a plain-Markdown
agent would miss — run `taliesin vocab` for the authoritative list with descriptions:

- **Front matter** — a leading `---` YAML block (`title`, `date`, `description`,
  `categories`, `bibliography`, …).
- **Callouts** — a fenced div `::: {.callout-note}` … `:::` (kinds: `note`, `tip`,
  `warning`, `important`, `caution`).
- **Code cells** — a fenced ` ```{python} ` (or `{r}`, `{js}`) block runs live. In-cell
  options are a `#| key: value` comment, e.g. `#| label: fig-scree` or `#| echo: false`.
- **Cross-references** — cite a labelled target with an `@`-prefix (`@fig-`, `@sec-`,
  `@tbl-`, `@eq-`, `@thm-`, …); `@fig-scree` renders as "Figure 3".
- **Citations** — `[@key]` cites a `.bib` entry declared in the `bibliography:` front
  matter. A `[@key]` with no `bibliography:` is a check error.
- **Structural divs** — `::: {.panel-tabset}`, `::: {.scrolly}`, `::: {.column-margin}`, …
- **Images** — `![descriptive alt text](path.png){#fig-x}`. Alt text must describe the
  image's content; a bare `alt="image"` or a filename echo is a check warning. Use
  `alt=""` only for a decorative image.

## A11y and correctness the checker enforces

`taliesin check` catches what an LLM co-author tends to get wrong: broken cross-references
and citations, duplicate heading ids, missing local assets, heading-level skips, and
placeholder/filename-echo alt text. A green `check` means the document is publishable, so
always finish on a clean `check`.
