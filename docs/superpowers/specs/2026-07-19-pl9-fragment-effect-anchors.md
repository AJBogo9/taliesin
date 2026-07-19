# PL9 — Deck fragment-effect classes escape the validated vocabulary

## Problem

`.fade-out` and `.highlight` are real, styled fragment-effect modifiers
(`deck.css`) that ride alongside `.fragment` (`::: {.fragment .fade-out}` reveals a
*change* to an already-visible block instead of revealing the block). But neither was in
`DIV_FEATURE_CLASSES` — the near-miss anchor set that powers the div-class did-you-mean —
so a typo in the effect modifier (`::: {.fragment .fade-ot}`, `.hihglight`) rendered a
plain fragment with **no diagnostic**. That's the one incomplete spot in an otherwise
complete `:::` vocabulary, and it's exactly a deck author's fiddly modifier.

The generic-div arm (`divs.rs`, where `.fragment` divs render) already calls
`validate_div_class` over every class; the effect names just weren't known anchors, so a
near-miss matched nothing and stayed silent.

## Fix

Add `"fade-out"` and `"highlight"` to `DIV_FEATURE_CLASSES` (`render/validate.rs`). That
is the complete set — `deck.css` has exactly two `.fragment.<effect>` combinators.

**Curation decision — anchor-only, not editor-offered.** They go in
`DIV_FEATURE_CLASSES` (the did-you-mean anchor) but **not** in `vocab::DIV_CLASS_NAMES`
(the editor-offered set). This matches the existing policy: `.fragment`/`.incremental`
are anchored-but-not-offered too, because the editor vocab curates *structural container*
classes, and fragment/effect modifiers are a separate deck-authoring family that only
makes sense alongside `.fragment`. The `DIV_CLASS_NAMES ⊆ DIV_FEATURE_CLASSES` subset
test stays valid (we only grew the superset), so no vocab re-bless is needed.

## Tests (TDD)

- `render::validate::fragment_effect_modifiers_are_did_you_mean_anchors`: a mistyped
  `.fade-ot`/`.hihglight` alongside `.fragment` draws the exact did-you-mean; the correct
  `.fade-out`/`.highlight` stay silent (known anchors). Mutation-checked: removing the two
  anchors makes the test fail.
- Existing `vocab::div_classes_are_a_subset_of_the_validator_vocab` +
  `vocab_matches_committed` stay green (superset-only growth).
