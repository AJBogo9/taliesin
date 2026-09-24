# live-edit benchmark results (indicative)

> Indicative numbers from the author's machine, re-measured **2026-08-27**; absolute times
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
> would have published ~13 ms as a "cold render" against a true ~105 ms.
>
> **The structural rows were regenerated 2026-09-24; the timing rows were not.** A line
> shift above a `:::` container became a `SetMeta` that carries the container's inner
> positions (it was a full `Update`), which changed the op counts and the payload. The
> timing rows above and below are still the 2026-08-27 release-build measurement; they
> were not re-measured with the fix because the machine was loaded that day, and a wall
> clock taken under load would publish the load.
>
> **The warm rows dropped ~4.5x on 2026-08-27** and the `refresh_xrefs` rows ~11x, from three
> changes in `crates/core`: `highlight::highlight` gained the memo `math::render` already had
> (it was 10.7 ms of a 12.6 ms warm render, re-deriving identical HTML every keystroke),
> KaTeX moved onto one long-lived worker thread (its QuickJS context is a thread-local and
> every render runs on a fresh thread, so the ~24.7 ms boot was being paid per page), and the
> two whole-project render passes now fan out across cores (`site/fanout.rs`). The cold-render
> row is unchanged and expected to be: a single cold document render is serial work none of
> those three touch.

What this shows, for one keystroke-sized edit to a paragraph above the cells in a real
post: the warm server re-renders and diffs in a fraction of the cold-start time (lazy
syntax-highlight and math init are amortized), it sends a payload roughly 89x smaller
than the full page a reload would re-fetch, and 54 of the 55 emitted ops are `SetMeta`,
which patches a block's `data-sourcepos` in place without touching its DOM node, so the
live state of every one of those blocks survives the edit. None of these are things a
batch compiler's cold-pass-plus-full-reload model (Jupyter/nbconvert, R Markdown/knitr,
Quarto, MyST) can match.

## live-edit benchmark: `corpus/tech-blog/posts/em-algorithm/index.tmd`

| metric | value |
|---|---|
| cold full render | 104112.2 us |
| warm edit (render + diff) | 2570.8 us |
| diff only | 211.8 us |
| ops emitted | 55 (insert 1, set_meta 54, update 0, remove 0) |
| full page HTML | 287755 bytes |
| warm-edit payload | 3241 bytes |
| payload shrink vs full reload | 89x smaller |
| open `<details>` survives as same DOM node | yes |

## project-scale save: `Site::refresh_xrefs`

**Read this before quoting the warm-edit row as "the cost of a save".** The rows above
measure one document through the render+diff seam. A save inside a *site* preview also
runs `Site::refresh_xrefs` first, and its harvest renders **every page in the project** to
full HTML to recover the cross-page float numbers. So a site save costs the warm edit
*plus* a pass whose size is the project's, not the edit's.

| project | pages | refresh_xrefs | per page |
|---|---|---|---|
| `docs/guide` | 16 | 3.2 ms | 0.20 ms |
| `docs/internals` | 6 | 1.6 ms | 0.26 ms |
| `corpus/tech-blog` | 17 | 4.6 ms | 0.27 ms |

Measured 2026-08-27, best of three per project, release build. **Still O(pages) per save,
but the constant is ~12x smaller and the work now uses every core**: the harvest renders
pages through `site::fanout::map_ordered`, so the per-page rate above is wall-clock across
`available_parallelism()` workers, not per-core cost. Re-extrapolating the 200-page synthetic
project from the same page shape: ~2.5 s before, ~0.2 s now on a 16-thread machine, and the
curve is still linear — a machine with two cores gets the memo win (the larger of the two)
but not the fan-out.

The remaining O(pages) term is a known, deliberately-unbuilt optimization: content-hashing
each page's harvest so a save re-renders only the pages that changed would make it flat.
It was costed on 2026-08-27 and **cut**, because after the memo and the fan-out it is worth
~3 ms on the largest project here, against a cache field on `Site` and a key that has to
cover source, includes, chapter number and site defaults or it silently serves stale float
numbers. Revisit it when a project exists that can feel it.

Deliberately **not gated**: this is a wall clock, and wall clocks measure the machine, so
by this project's own rule they carry a date and get re-measured before a release rather
than pinned by a test that fails on a slower laptop.

## Where the payload goes

All 55 ops together weigh 3,241 bytes: 54 `SetMeta` patches and the one `Insert` for the
newly typed paragraph. The document's `::: {.callout-note collapse="true"}` fenced div, the
only block whose html carries more than one `data-sourcepos`, is one of the 54: its patch
carries the new position of every inner block as well, and the client writes them onto the
div's own descendants in order, so Ctrl-click inside it stays exact and an opened callout
stays open.

From 2026-06-30 (`6cdbc218`) until 2026-09-24 that div took a full `Update` instead,
because `SetMeta` could then patch only the outer element and would have left the inner
positions stale. That one op was 90% of a 32 KB payload, this file published 9x, and the
re-render closed an opened callout (and re-mounted any `{js}` cell inside a fenced div) on
every edit above it. The 83x published before 2026-06-30 had today's shape but patched the
outer position only. `regression.rs` pins the op shape exactly, so the next change to this
contract fails a test instead of quietly invalidating a published number.
