# live-edit benchmark results (indicative)

> Indicative numbers from one run on the author's machine; absolute times vary by
> machine and build profile (these are a release build, best of twelve). Regenerate
> with `cargo run --release -p live-edit-bench`, which also rewrites the committed
> `RESULTS.json`. The structural rows (op counts, payload bytes, payload ratio, DOM
> preservation) are deterministic and gated
> (`tools/live-edit-bench/tests/regression.rs`); only the three timing rows drift
> between runs.

What this shows, for one keystroke-sized edit to a paragraph above the cells in a real
post: the warm server re-renders and diffs in a fraction of the cold-start time (lazy
syntax-highlight and math init are amortized), it sends a payload roughly 9x smaller
than the full page a reload would re-fetch, and 53 of the 55 emitted ops are `SetMeta`,
which patches a block's `data-sourcepos` in place without touching its DOM node, so the
live state of every one of those blocks survives the edit. None of these are things a
batch compiler's cold-pass-plus-full-reload model (Jupyter/nbconvert, R Markdown/knitr,
Quarto, MyST) can match.

## live-edit benchmark: `corpus/posts/em-algorithm/index.tmd`

| metric | value |
|---|---|
| cold full render | 135010.4 us |
| warm edit (render + diff) | 13403.9 us |
| diff only | 751.7 us |
| ops emitted | 55 (insert 1, set_meta 53, update 1, remove 0) |
| full page HTML | 291691 bytes |
| warm-edit payload | 32303 bytes |
| payload shrink vs full reload | 9x smaller |
| open `<details>` survives as same DOM node | yes |

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
