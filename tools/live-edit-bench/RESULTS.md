# live-edit benchmark results (indicative)

> Indicative numbers from one run on the author's machine; absolute times vary by
> machine and build profile. Regenerate with `cargo run -p live-edit-bench`. The
> structural rows (op counts, payload ratio, DOM preservation) are the deterministic,
> gated invariants (`tools/live-edit-bench/tests/regression.rs`).

What this shows, for one keystroke-sized edit to a paragraph above the cells and the
collapsible callout in a real post: the warm server re-renders and diffs in a fraction
of the cold-start time (lazy syntax-highlight and math init are amortized), it sends a
payload roughly 83x smaller than the full page a reload would re-fetch, and the open
`<details>` callout below the edit is patched in place (a `SetMeta`, never a re-render),
so its live DOM state survives. None of these are things Quarto's cold-pass-plus-full-
reload model can match.

## live-edit benchmark: `corpus/posts/em-algorithm/index.qmd`

| metric | value |
|---|---|
| cold full render | 123994.9 us |
| warm edit (render + diff) | 28425.1 us |
| diff only | 685.6 us |
| ops emitted | 55 (insert 1, set_meta 54, update 0, remove 0) |
| full page HTML | 269693 bytes |
| warm-edit payload | 3231 bytes |
| payload shrink vs full reload | 83x smaller |
| open `<details>` survives as same DOM node | yes |
