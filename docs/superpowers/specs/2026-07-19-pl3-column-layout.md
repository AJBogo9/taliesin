# PL3 — Unify column layout; stop silently discarding `.column width=`

Three spellings make a column grid — `::: {layout-ncol=3}` (the only structural feature
dispatched on a bare `key=value`), `::: {.columns}`, and `.column` children — and the
`.columns` arm silently equalized a `::: {.column width="70%"}` (a reveal/Quarto habit) with
no warning.

## Design fork: warn, don't honour `width=`

The audit sanctions "honour OR warn." **Warn** was chosen: the grid is
`grid-template-columns: repeat(N, minmax(0,1fr))` (pinned by tests asserting `repeat(2,`), and
honouring per-column widths cleanly needs either a grid→flex switch (breaks those pins + subtly
changes a corpus-covered layout) or fragile HTML-string parsing of the pre-rendered children in
the container arm. Equal columns is a fine default ("perfect the default before a knob"); a
`.column width=` is a niche reveal feature not worth that risk in a polish pass. Variable-width
columns are recorded as a possible future enhancement.

## Changes

- **Canonical naming.** `.columns` (dot-consistent with `.scrolly` / `.panel-tabset`) is now
  documented as the canonical column grid; `layout-ncol` is the bare-attribute alias. `.columns`
  gains an optional **`ncol=`** override (parity with `layout-ncol`), so `::: {.columns ncol=3}`
  works; otherwise the count is the number of `.column` children (fallback 2).
- **Warn on `.column width=`.** `render::validate::validate_column_width` warns (located) when a
  `.column` carries a non-empty `width=`, echoing the width + naming the equal-width behaviour +
  the fixed-count knobs. New `TAL-COLUMN-WIDTH` (WARNING) code + `--explain` entry;
  `DIAGNOSTICS.md` regenerated.

## Bundled: a pre-existing `group_divs` grouping bug (found here, fixed)

The PL3 nested-column tripwire (`.columns` after an empty `.input` div, PL2) exposed a real
pre-existing bug: in `group_divs` the "skip degenerate/empty spans" loop ran **after** the
"open containing spans" loop. Spans are open-sorted, so an empty div sitting at `span_idx` has
`close < buf_start` (not a container) — the open loop stopped on it and never reached the
following block's own container, silently dropping that block out of its div (a `.column`, a
`.callout`, …). Fix: run the skip loop **first**. Not silent — called out here and in the commit;
pinned by `a_block_after_an_empty_div_stays_inside_its_own_container` (mutation-checked: reverting
the swap fails it). Full core suite (the corpus grouping invariants) stays green.

## Tests

- `render::tests::columns_ncol_overrides_the_child_count`,
  `render::tests::column_width_warns_because_columns_are_equal_width`,
  `render::tests::a_block_after_an_empty_div_stays_inside_its_own_container`.
- `render::validate::validate_column_width_warns_only_on_a_column_with_a_width` (pure).
- Corpus: a `.columns` / `.column width=` tripwire in `corpus/diagnostics/typos.tmd` pinned by
  `nested_validation.rs`; `gallery.tmd` prose reframed. Mutation-checked; verified end-to-end.
