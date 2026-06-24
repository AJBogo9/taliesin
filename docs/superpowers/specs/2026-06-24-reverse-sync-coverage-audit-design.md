# Design: `reverse-sync-coverage-audit` (Wave 4)

Status: approved 2026-06-24. Branch `feat/reverse-sync-coverage-audit`. Roadmap:
`BEYOND-QUARTO.md` Pillar II. Strengthens the block-model contract ahead of the editor
companion. Test + comment only.

## Audit finding (empirical)

`highlightAtLine` (web-client/client.js) drives reverse cursor-sync by scanning every
`[data-sourcepos]` element and matching the strict regex `^(\d+):\d+-(\d+):\d+$`; any
non-matching sourcepos is skipped (cursor-invisible). A discovery sweep over EVERY
`data-sourcepos` in EVERY corpus doc's rendered HTML found **zero** offenders — the
emission seam (`map_origin → "{open}:1-{close}:3"`) is uniformly correct, and the
executed-cell path (`exec.rs::output_block`) carries the source cell's sourcepos verbatim.
So there is **nothing to fix**; the deliverable is the regression net + verification that
converts an un-enforced invariant into a corpus-enforced one.

## Deliverables

1. **Permanent corpus test** `reverse_sync_sourcepos_is_total` (`crates/core/tests/corpus.rs`,
   replacing the temporary discovery test): for every corpus doc, every NON-EMPTY
   `data-sourcepos` in the rendered HTML matches `^(\d+):\d+-(\d+):\d+$`. Empty sourcepos
   (generated References/footnotes blocks) is exempt (matching forward/reverse symmetry —
   those blocks have neither `data-block-id` nor `data-sourcepos`).
2. **Cell-output coverage**: a regex assertion in the existing `exec.rs` `output_block`
   unit test (the no-kernel corpus test can't produce executed outputs), pinning that a
   spliced output block's sourcepos is reverse-sync-valid.
3. **Contract comment** at the sourcepos-emission seam (the block `attrs` builder in
   `render/mod.rs` and `divs.rs`) noting `L:C-L:C` is the reverse-sync contract, corpus-
   enforced — so future emitters keep the format.
4. **Browser verification** (no companion exists yet → drive directly): post
   `{type:"qmd-cursor", file, line}` for representative lines and assert the right block
   gets `.qmd-hl` — a heading, a paragraph, a block nested inside a callout, and a deck
   slide jump.

## Invariants

Strengthens the corpus-enforced block-model contract; no production code change (audit
found none needed); fixes (if ever needed) live at the attr-injection seam, never in
numbering/figure/cite. `highlightAtLine` is the consumer; this verifies it works ahead of
building the producer (`vscode-editor-companion`, the next Wave 4 item).

## Out of scope (YAGNI)

Live-kernel executed-output audit (the `output_block` code + its unit test cover it);
changing the regex or the forward `locatable()` contract; the editor companion itself.
