# live-edit benchmark results (indicative)

> Indicative numbers from the author's machine, re-measured **2026-08-18**; absolute times
> vary by machine and build profile (release build). Regenerate with
> `cargo run --release -p live-edit-bench`, which also rewrites the committed
> `RESULTS.json`. The structural rows (op counts, payload bytes, payload ratio, DOM
> preservation) are deterministic and gated
> (`tools/live-edit-bench/tests/regression.rs`); only the timing rows drift between runs.
>
> **Best-of-twelve is now the binary's job, not the reader's.** This note used to claim
> best of twelve while `main` measured exactly once, so a plain `cargo run` published a
> best-of-one under a best-of-twelve label. `BEST_OF` is a constant in `main.rs` now. The
> cold-render row is deliberately exempt: only the first render in a process is cold (the
> syntax set and the other lazy statics are built on first use), so best-of-twelve there
> would have published ~13 ms as a "cold render" against a true ~113 ms.

What this shows, for one keystroke-sized edit to a paragraph above the cells in a real
post: the warm server re-renders and diffs in a fraction of the cold-start time (lazy
syntax-highlight and math init are amortized), it sends a payload roughly 9x smaller
than the full page a reload would re-fetch, and 53 of the 55 emitted ops are `SetMeta`,
which patches a block's `data-sourcepos` in place without touching its DOM node, so the
live state of every one of those blocks survives the edit. None of these are things a
batch compiler's cold-pass-plus-full-reload model (Jupyter/nbconvert, R Markdown/knitr,
Quarto, MyST) can match.

## live-edit benchmark: `corpus/tech-blog/posts/em-algorithm/index.tmd`

| metric | value |
|---|---|
| cold full render | 113223.3 us |
| warm edit (render + diff) | 12137.6 us |
| diff only | 354.5 us |
| ops emitted | 55 (insert 1, set_meta 53, update 1, remove 0) |
| full page HTML | 287286 bytes |
| warm-edit payload | 31930 bytes |
| payload shrink vs full reload | 9x smaller |
| open `<details>` survives as same DOM node | yes |

## project-scale save: `Site::refresh_xrefs`

**Read this before quoting the warm-edit row as "the cost of a save".** The rows above
measure one document through the render+diff seam. A save inside a *site* preview also
runs `Site::refresh_xrefs` first, and its harvest renders **every page in the project** to
full HTML to recover the cross-page float numbers. So a site save costs the warm edit
*plus* a pass whose size is the project's, not the edit's.

| project | pages | refresh_xrefs | per page |
|---|---|---|---|
| `docs/guide` | 16 | 47.6 ms | 2.98 ms |
| `docs/internals` | 6 | 12.5 ms | 2.08 ms |
| `corpus/tech-blog` | 17 | 56.7 ms | 3.34 ms |

Measured 2026-08-18, best of three per project, release build. The rate is flat in page
count — a synthetic project of copies of the em-algorithm post (a heavy ~20 KB page with
math and cells) measured 12.5 ms/page at 20, 50, 100 and 200 pages, i.e. **2.5 s** for a
200-page save. At the sizes any real project here reaches, ~50 ms, this is comfortably
inside a save; it is recorded because the extrapolation is the thing a reader would
otherwise get wrong from the 12 ms row, not because it is currently a problem.

Deliberately **not gated**: this is a wall clock, and wall clocks measure the machine, so
by this project's own rule they carry a date and get re-measured before a release rather
than pinned by a test that fails on a slower laptop.

## Where the payload goes, and why the ratio is 9x and not 83x

Read this before quoting the ratio anywhere.

The single `Update` is **29,081 of the 32,303 payload bytes (90%)**. It is one block:
the `::: {.callout-note collapse="true"}` fenced div, the only block of the document's
59 whose html carries more than one `data-sourcepos`. The other 54 ops (53 `SetMeta`
plus the one `Insert` for the newly typed paragraph) together account for the remaining
~3.2 KB.

That one `Update` is deliberate. Until 2026-06-30 a line-shifting edit above a fenced
div emitted a cheap `SetMeta` for it too, and the payload was ~3.2 KB against a 270 KB
page: the **83x** this file used to publish. `6cdbc218` then made a block whose html
holds more than one `data-sourcepos` fall through to a full `Update`, because `SetMeta`
patches only the *outer* element's `data-sourcepos` and would leave the div's inner
blocks pointing at stale lines, silently sending Ctrl-click and reverse cursor-sync
inside the div to the wrong place. See
`diff::nested_div_sourcepos_shift_is_a_full_update_not_setmeta`.

So the 83x was a correct measurement of a design that no longer exists. It was never
regenerated after the hardening (the two later commits to this file touched only prose
and paths), and `RESULTS.json` was gitignored, so nothing caught the drift for five
weeks. Both are fixed: the JSON is committed, and `regression.rs` now pins the op shape
exactly, so the next change to this contract fails a test instead of quietly
invalidating a published number.

**Known consequence, not yet addressed.** Because the client applies `Update` with
`el.replaceWith(node)` (`web-client/client.js:1712`) after `teardownJs`, a `:::`
collapse callout that the reader has opened *does* close when an edit lands above it,
and any `{js}` cell inside such a div is torn down and re-mounted. The DOM-preservation
row above is still `yes` and honestly so: it tracks the single-`data-sourcepos`
stateful blocks, which are 58 of this document's 59 and include the `{python}` cell's
`<details>` output disclosure. But "live DOM state survives an edit" holds for every
block shape *except* a fenced div, and that exception should be stated whenever the
claim is made.
