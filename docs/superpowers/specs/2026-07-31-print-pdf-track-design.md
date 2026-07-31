# Print/PDF track: a paged rendering of the built HTML

Backlog item **159**, the top of the P1 queue. Branch `print-pdf-track-2026-07-31`.
Upstream framing: `notes/ROADMAP.md` Pillar IV / Wave 5 (`print-pdf-track`); substance:
`notes/FEATURE-IDEAS.md` #57. Pin: `corpus/print/paged.tmd`.

The **index** half of idea #57 is **out of scope** here and stays filed: an index needs
index-term markup in the `.tmd` itself, which is a new dialect surface with its own
validator, schema and docs. It is a content-authoring feature that happens to render in
print, and folding it in would put a dialect addition on the critical path of a typography
feature.

## The line, restated

This is the item most likely to cross the project's identity boundary, so it is restated
before anything else. **The moment this forks into a separate Pandoc/Typst/LaTeX path it has
violated HTML-only.** What ships here is a *paged rendering of the build artifact*, produced
by a browser from the same HTML the preview serves. There is no second compiler, no second
document model, and no format-specific emitter.

## The problem

Taliesin already prints better than most static-site generators. `assets/css/base.css:1091-1127`
forces a light palette on paper, lifts and un-collapses the TOC, spells out external link URLs,
and sets `break-inside: avoid` on floats, pre, tables, blockquotes and callouts.

What it cannot do is *paginate*. There are no running heads, no folios, no page numbers on
cross-references, and no list of figures. Those are the things that make a printed document
read as typeset rather than as a long screenshot, and they are the reason authors still drop
to LaTeX for a paper or a book.

## Measured ground truth

Everything below was verified on this machine (Chrome 150.0.7871.186, 2026-07-31) rather than
assumed, because the whole design turns on which engine can do what. Each probe carried a
known-positive row, so a negative result is a real negative and not a broken probe.

### Chrome's native paged media is half a solution

| Capability | CSS used | Chrome 150 |
|---|---|---|
| Folios | `@bottom-center { content: counter(page) }` | **works** |
| Total page count | `counter(pages)` | **works** |
| Running heads | `string-set` + `content: string(...)` | **no output** |
| Page cross-refs | `target-counter(attr(href url), page)` | **no output** |
| List-of-figures page numbers | `target-counter()` in a list | **no output** |

`@page` margin boxes themselves *are* supported, which is what makes folios work and is the
positive control proving the probe was sound. The two failures were each re-tested with three
syntax variants (`string(x)` / `string(x, first)`, and `target-counter` with `attr(href url)`,
`attr(href)` and a literal `"#id"` selector); all six produced nothing.

**Consequence:** Chrome alone delivers folios, break control, widows/orphans and hyphenation,
and none of the three items that were the point of the feature.

### paged.js supplies exactly the missing three

paged.js **0.4.3**, **MIT**, `paged.polyfill.min.js` = **503 KB**. Re-running the same probe
with the polyfill loaded produced a real running head in `@top-center` ("Chapter One") and real
resolved page numbers from `target-counter()` (a list-of-figures entry rendered
`Figure 1: alpha .... 2`).

### paged.js cannot be driven from the Chrome CLI

This is the finding that decides the architecture.

- `--headless --print-to-pdf` truncates **deterministically at 2 pages**, identical at
  `--virtual-time-budget` of 5 s, 20 s, 60 s and 120 s. It is not a timeout; Chrome prints at a
  fixed point and paged.js's chunking is still running.
- `--headless --dump-dom` captures the page **mid-initialization**: paged.js's styles are
  injected (the `--pagedjs-*` custom properties are present and the A5 size has been picked up)
  but the content has not been chunked, so the DOM contains the `pagedjs_pages` container and
  **zero page boxes**. Printing that dump yields one blank page.

  *Method note:* an initial count of "17 page boxes" in this dump was wrong — the grep was
  matching paged.js's injected **CSS rule text**, not DOM elements. Counting rendered elements,
  not stylesheet mentions, is what turned this from a false positive into the real result.

**Consequence: CDP is required.** The driver must navigate, wait for paged.js's completion
signal, and only then call `Page.printToPDF`.

### The CDP driver is already here and already enabled

`chromiumoxide` is gated behind the `headless-js` feature because it costs 24% of a clean
release build. That gate does not bite this feature, because every path that matters already
turns it on:

- `.github/workflows/release.yml:66` — released binaries
- `.github/workflows/ci.yml:71` — the test suite
- `~/.local/bin/taliesin:44` — the author's own launcher

Only a bare `cargo build` from source lacks it. That is a developer scenario, and it already
degrades gracefully for `read --run-js`; `taliesin pdf` takes the same path with the same
kind of message.

### The completion signal

paged.js exposes a sanctioned hook: the polyfill reads `window.PagedConfig` and awaits
`config.after(done)` once rendering finishes (`paged.polyfill.js`, the `previewer.preview`
block). Internally that point is where the chunker sets `rendered = true`, sets
`--pagedjs-page-count`, and emits `"rendered"`.

The print page therefore declares:

```js
window.PagedConfig = {
  after: () => { document.documentElement.dataset.taliPaged = 'done'; }
};
```

and the driver waits for `data-tali-paged="done"`. **This is deliberately the same idiom the
codebase already uses**: `headless_js.rs` waits on `data-tali-done` to know a `{js}` cell
finished. One waiting convention, two consumers.

## Architecture

```
  .tmd ──► existing pipeline ──► RenderedDoc ──┬──► page.rs   ──► normal page   (unchanged)
                                               │
                                               └──► print.rs  ──► print HTML  (new, transient)
                                                                      │
                                                          temp dir, file:// ─┐
                                                                             ▼
                                              pdf.rs ── CDP ── wait data-tali-paged ── printToPDF ──► .pdf
```

`print.rs` is a **sibling** of `page.rs`, never a modification of it. That is what keeps every
existing page byte-identical, so no snapshot, no `body_html_snapshots` entry and no drift gate
moves as a result of this feature.

### Components

| # | Path | Purpose |
|---|---|---|
| 1 | `crates/core/src/render/print.rs` | **New.** `RenderedDoc` → standalone paginated HTML: inlines `print.css`, the polyfill, the `PagedConfig` hook, and the generated list-of-figures. |
| 2 | `crates/core/assets/css/print.css` | **New.** `@page` size/margins/margin-boxes, `string-set` running heads, `target-counter` xref suffixes, LoF leaders, widows/orphans, `hyphens: auto`. |
| 3 | `crates/core/assets/vendor/paged.polyfill.min.js` | **New, vendored.** 0.4.3, MIT, 503 KB, + `THIRD_PARTY.md` entry. |
| 4 | `crates/server/src/pdf.rs` | **New.** The `pdf` subcommand; reuses `chrome_path()` and the launch/timeout policy from `headless_js.rs`. |
| 5 | `corpus/print/paged.tmd` | **New. The pin.** Figures, cross-refs, and enough prose to force real page breaks. |

Plus wiring: the `pdf` arm in `main.rs`, its `subcommand_help` page, `COMMANDS`, and shell
completions.

### Command surface

```
taliesin pdf <file.tmd> [-o out.pdf] [--paper a4|letter|a5] [--keep-html]
```

- Default output is `<name>.pdf` beside the source, matching `build`'s convention.
- **`--paper` defaults to `a4`.** Paper size is an *invocation* choice, not document config, so
  it is a CLI flag and adds **no front-matter key** — this is the minimal-config rule
  ("perfect the default before adding a knob") applied honestly rather than dodged.
- `--keep-html` retains the intermediate paginated HTML for inspection. Without it `pdf.rs`
  removes its own temp directory. (Note: `runtime_dirs.rs` is *not* the seam for this — it owns
  kernel connection and warmpool dirs only, so the print path creates and drops its own.)

**Without the `headless-js` feature** (a bare `cargo build` from source), `taliesin pdf` exits
with a message naming the rebuild, exactly as `read --run-js` reports `skipped (chrome
unavailable)` today. Same degradation for a missing system Chrome. Never a panic, never a
silent empty PDF.

## What v1 delivers

1. **Running heads** — current chapter/section in the `@page` margin box via `string-set`.
2. **Real folios** — `counter(page)` / `counter(pages)`.
3. **Page cross-references** — `@fig-`/`@sec-`/`@thm-` refs render as "Figure 3 (p. 12)".
4. **Auto list-of-figures** — with true page numbers and leader dots.
5. **Widow/orphan control** — plus the break rules `base.css` already has.
6. **Optical hyphenation** — `hyphens: auto`, driven by the document's `lang`.

**Item 3 costs zero Rust.** Cross-references already emit
`<a href="#fig-x" class="tali-xref">Figure&nbsp;1</a>` (`crates/core/src/cite/render.rs:328`),
so the page-number suffix is a single CSS rule against a class that already exists. The
highest-value item on the list is the cheapest one.

### Media degradation

- **`{js}` cells print live.** CDP genuinely executes the page, so Plot/GLSL/numerics cells
  have painted real `<svg>`/`<canvas>` before the print call. This is *better* than the
  "degrade to a poster frame" the item filed, and it costs nothing extra.
- **Video degrades to its poster frame**, which Chrome does natively for `<video>` in print.
- **Mermaid** renders client-side like `{js}` and is subject to the same wait.

## Invariants

- **HTML-only holds.** The PDF is produced by a browser from the built HTML. No second
  compiler path, no format emitter, no new document model.
- **Single editing surface holds.** The whole path is read-only. Nothing writes back to source.
- **Block model: explicitly out of contract, and that is safe.** paged.js clones and splits
  nodes across page boundaries, which **duplicates `data-block-id`**. This is acceptable only
  because the print artifact is *terminal output*: never served by preview, never diffed, never
  source-mapped, never reloaded incrementally. **Hard boundary: the print assembly must not be
  reachable from `preview`**, and a test asserts it.
- **Do-NOT-touch respected.** `MAX_WARM_PAGES` / the `exec_pool.rs` LRU is untouched. So are
  `divs.rs`, `cite.rs`, `includes.rs`, the numbering scanners and exec/freeze/kernel. This
  feature is purely additive: a new render sibling, a new asset, a new subcommand.
- **Offline holds.** The polyfill is vendored, not fetched. The browser is local and
  system-provided. No network call is added.

## Testing

| Layer | Gate | What it proves |
|---|---|---|
| Print-HTML assembly | plain `cargo test` | Pure function over `RenderedDoc`: the polyfill and `PagedConfig` hook are inlined, the LoF lists every figure in order, `@page` size tracks `--paper`. No browser needed. |
| Existing pages unchanged | plain `cargo test` | The existing `crates/core/tests/body_html_snapshots.rs` must stay green with **no re-bless**. Because `print.rs` is a sibling of `page.rs` and `print.css`/the polyfill are inlined only by the print assembler, adding this feature must move zero normal-page bytes. A required re-bless means the sibling boundary leaked. |
| Live paginate → PDF | `TALIESIN_REQUIRE_CHROME=1` | The real CDP loop: pages > 1, running head present, a `(p. N)` suffix resolved to a non-zero number. Joins `read_run_js` in `tools/gates.sh`. |
| Projection exclusion | plain `cargo test` | The generated LoF appears in **none** of the four text projections (`read`/`skim`, the search index, `llms-full.txt`). |
| Vendored provenance | existing `third_party.rs` | paged.js 0.4.3 / MIT is declared and drift-locked. |

Two project-specific traps this test plan is written against:

- **The inlined-asset needle trap.** Every Taliesin page inlines the CSS/JS payload whole, so a
  whole-page `contains()` proves nothing about the *document*. Assertions here needle the full
  emitted tag, and the negative assertions (LoF absent from projections) are checked against a
  built artifact, not a substring of a bundle.
- **A new generated block owes the projection sweep.** The reader-affordances batch found four
  projections in three modules leaking a generated block. The LoF is generated, so it owes the
  same sweep even though living only in the print artifact should exclude it structurally.

### Verification by mutation

Per the standing rule, each fix is verified by restoring the bug and watching the *named* test
fail — not by a green suite. Specifically: remove the `PagedConfig` hook and the live gate must
go red (not hang), and drop the `target-counter` rule and the `(p. N)` assertion must go red.

## Cost

- **Binary:** +503 KB against a 5.6 MB asset tree that already carries a 3.5 MB mermaid — ~9%
  of assets, ~1% of the 48.8 MB release binary.
- **Dependencies:** **zero new.** `chromiumoxide` is already present and already enabled
  everywhere that matters.
- **Effort:** Large, and it stays Large. The risk is concentrated in the polyfill integration
  and the CDP wait, not in the CSS.

## Explicitly out of scope

- **The index** (needs `.tmd` dialect; stays filed as part of idea #57).
- **Whole-book concatenation.** v1 is single-document, matching the named pin. Chapter-scoped
  running heads, continuous folios across chapters and a book-level LoF are a follow-on.
- **Publishing a PDF beside a built site.** The artifact is on-demand only; `_site/` is
  unchanged and no shipped page carries the polyfill. A downloadable PDF is a separate call
  once the typography is proven.
- **Decks.** Print/PDF for decks was deliberately deleted (`notes/2026-07-12-deck-audit.md`)
  and is not revived here.
- **Any non-browser rendering path.** See "The line", above.
