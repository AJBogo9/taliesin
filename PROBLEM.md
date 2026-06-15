# Problem Description

## Summary

Quarto is the current tool used for authoring books, blog posts, and reveal.js
slide decks from `.qmd` files. It works, but its architecture imposes friction
that a single-purpose, performance-oriented tool could remove. This document
describes the problems with the status quo and the constraints the replacement
must satisfy. It deliberately does **not** propose a solution — see
`PLAN.md` for that.

## Scope of use

This tool is built for one author and one workflow. It is **not** a general
Quarto replacement. The only input format is `.qmd`, and the only three
deliverables are:

1. **Blog posts** — single-file, prose-heavy, with KaTeX math, code execution,
   and interactive JS demos (Three.js, Observable JS / OJS, D3).
2. **reveal.js slides** — same authoring model, slide-structured HTML output.
3. **Books** — multi-file projects with cross-references, numbering, and a
   table of contents.

Output is **HTML only**. PDF output is explicitly out of scope.

## Why not just keep using Quarto

Quarto's value for the general public comes from its breadth: it glues together
Pandoc (AST parsing and the full format-conversion matrix), knitr/Jupyter (code
execution), and a Deno/TypeScript orchestration layer. That breadth is the
source of three concrete problems for this narrow use case.

### Problem 1 — No source mapping (preview ↔ source)

There is no way to double-click a rendered element in the preview and jump to the
exact location in the `.qmd` source that produced it. This is structurally
impossible in Quarto's pipeline: Pandoc parses markdown into an AST and emits
HTML, discarding source-position provenance along the way. By the time output
exists, the link back to source line/column is gone.

For an author who iterates heavily on long documents, the absence of
click-to-source is a constant, low-grade tax on navigation.

### Problem 2 — Full-page re-render on every change

On each change-and-rebuild cycle, the entire page is re-rendered and the preview
reloads wholesale. This loses scroll position and reinitializes every
client-side component on the page. Interactive demos (Three.js scenes, OJS
reactive cells) lose their runtime state and visibly flicker, even when the edit
was unrelated to them.

The desired behavior is in-place, block-level updates: when a part of the
document changes and is re-rendered, only the corresponding region of the
preview should update, leaving scroll position and unrelated live components
untouched.

### Problem 3 — Slow startup

Each render shells out to short-lived processes (Deno, Pandoc, a freshly spawned
execution kernel). The dominant cost in the perceived slowness is process
startup and inter-process communication, not the actual rendering work. Cold
kernel start is a recurring annoyance in the edit loop.

## Constraints and non-goals

- **Single-author scope.** Correctness is defined against the author's own
  corpus of real documents, not against arbitrary documents in the wild. The
  corpus *is* the specification.
- **HTML output only.** No PDF, no LaTeX, no docx, no format-conversion matrix.
- **Editor-agnostic core, VS Code as the primary client.** The author develops
  in VS Code daily. The core renderer/server must not be coupled to any single
  editor, but VS Code is the first and primary consumer, with a plain browser
  preview as a near-free secondary client.
- **Live code execution required.** Code cells must execute against a persistent,
  warm execution kernel (Jupyter protocol). Cached-only output is not sufficient
  for v1.
- **Performance is a first-class feature**, not an afterthought. Fast incremental
  rebuilds and no per-edit process startup are core requirements.
- **Not a Pandoc reimplementation.** Reimplementing the general markdown →
  any-format conversion matrix is explicitly out of scope and is the reason a
  full rewrite would fail. Existing, well-tested libraries should be used for
  parsing rather than hand-rolling a parser.

## Definition of "done"

The tool is successful when the author's real blog posts, slide decks, and books
render correctly (judged by direct inspection of output), and the three problems
above are resolved:

1. Double-clicking a rendered element jumps to its source location.
2. Saving a change updates only the affected block(s) in place, preserving scroll
   position and the runtime state of unrelated live components.
3. The edit loop has no per-edit process-startup cost; the execution kernel stays
   warm.
