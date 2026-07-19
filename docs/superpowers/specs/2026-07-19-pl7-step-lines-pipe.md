# PL7 — `.step lines=` silently no-ops on a deck `|` habit

## Problem

Two line-highlight grammars ship with two delimiters:

- **Decks / listings**: `code-line-numbers="1|2-3"` — `|` separates reveal **steps**, `,`
  separates ranges within a step (`emit.rs`).
- **Walkthrough / scrolly `.step`**: `lines="6-8"` — a single step's focus, parsed on `,`
  only (`walkthrough.js` `parseLineSpec`; `scrolly.js`).

A deck-trained author who writes `::: {.step lines="1|2-3"}` gets **zero** highlighted
lines: `"1|2-3"` matches neither the range regex nor `^\d+$`, so `parseLineSpec` adds
nothing — silently. It's a silent hole in an otherwise fully-diagnosed `:::` surface.

## Fix — warn, do NOT align the grammars

The grammars are **semantically different**, so aligning them would be wrong: a `.step`
*is already one step*, so the step boundary is expressed by having separate `.step`
blocks, not by a `|` inside one step's `lines=`. A `|` in a `.step lines=` is a category
error, not a valid multi-step spec to honour. So the right fix is a **located warning**
that teaches the correct shape, not making `parseLineSpec` split on `|`.

- `render::validate::validate_step_lines(spec, line, file)` — warns (click-to-source) when
  a `.step lines=` value contains `|`, echoing the spec and pointing at the fix (comma
  ranges within a step; separate `.step` blocks per pipe group). Purely diagnostic — the
  step still renders.
- Wired into the `.step` arm of `render/divs.rs::build_container` (where `data-cw-lines` is
  emitted), pushing into the render warning channel so `check`/`build --strict`/`preview`
  all surface it.
- A dedicated diagnostic code **`TAL-STEP-LINES`** (WARNING) in
  `diagnostics/codes.rs` (TABLE needle `"step separator"` + an EXPLANATIONS row), so it is
  agent-matchable and `check --explain TAL-STEP-LINES` teaches the fix (DX6/PL1 spirit).
  `docs/DIAGNOSTICS.md` regenerated (blessed).

## Tests (TDD, corpus-led)

- `corpus/diagnostics/typos.tmd` gains a `::: {.step lines="1|2-3"}` tripwire (its whole
  purpose is deliberate misuse), pinned by `nested_validation.rs` (the warning fires and is
  located). The doc's "unknown "-key count is untouched (this message isn't an unknown-key).
- `render::validate::validate_step_lines_warns_only_on_the_pipe_step_separator`: `|` warns
  with the spec echoed + located; the valid grammars (`3-5,8`, `6-8`, `all`) stay silent.
- `diagnostics::codes` completeness + `diagnostics_md_matches_committed` cover the new code.
- Mutation-checked: neutering the `|` guard fails both the unit + corpus tests. Verified
  end-to-end against the binary (`check` flags it `warning[TAL-STEP-LINES]`, `--explain`
  resolves).
