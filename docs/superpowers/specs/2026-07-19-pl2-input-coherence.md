# PL2 — Make the `{{< input >}}` reactive control coherent

The reactive input control is the one interactive feature that's a `{{< >}}` shortcode rather
than a `:::` div, so an author who reaches for `::: {.input name="k"}` (matching `.scrolly` /
`.panel-tabset` / `//| viewof`) gets **silently nothing**, while the CSS has a leftover
collision that mutes the shortcode's own label. Two independent fixes.

## (a) Warn on a dropped empty feature div (the silent hole)

An empty `:::` div (no blocks between its fences) is dropped in `group_divs` before
`build_container` runs — so **none** of the feature validators (`validate_input`,
`validate_tabset`, `validate_div_class`) ever fire for it. An empty `::: {.input name="k"}`,
`::: {.callout-note}`, `::: {.panel-tabset}` renders nothing, silently.

- `render::validate::validate_empty_feature_div(classes, line, file)` — warns (located) when an
  empty div names a real feature (`input`, `callout-*`, any `DIV_FEATURE_CLASSES` member, any
  theorem kind), with a pointed hint for `.input`: *the reactive input control is the
  `{{< input >}}` shortcode, not a `:::` div.* A plain/custom empty div stays silent
  (open vocabulary → `None`).
- Wired as a **position-independent scan** over all spans in `group_divs` (a span is empty when
  no flat block falls between its fences), so trailing/standalone empty feature divs are caught
  too — not just the ones the monotonic mid-loop skip would reach.
- New `TAL-EMPTY-DIV` (WARNING) code + EXPLANATIONS row (`--explain` teaches the fix);
  `DIAGNOSTICS.md` regenerated.

## (b) Delete the dead/colliding legacy `.tali-input` CSS

`base.css` had a second `.tali-input` block (the old `//| viewof` structure) at a **later
source order + same/higher specificity** than the shortcode's block, so it overrode it:
`.tali-input { color: var(--tali-muted) }` cascaded onto `.tali-input-label` (which sets only
`font-weight`), **muting the shortcode's label**, and `.tali-input input[type=range]` (0,2,1)
beat `.tali-input-control[type=range]` (0,2,0). Confirmed the legacy block is dead: the only
emitter of `.tali-input` markup is the shortcode (`extension/mod.rs`) — `//| viewof` controls
are raw returned DOM, never `.tali-input`-wrapped.

Deleted the whole legacy block (its misleading `//| viewof` comment + the 4 rules); the live
`.tali-js-error` block below it (its own comment) is untouched. The shortcode's own
`.tali-input` / `.tali-input-control` / `.tali-input-label` rules now govern cleanly; the
select reverts to native styling, consistent with the block's stated "native controls" intent.

## Tests / verification

- `render::validate::validate_empty_feature_div_warns_by_class_and_points_input_at_the_shortcode`
  (pure) + a `::: {.input name="k"}` tripwire in `corpus/diagnostics/typos.tmd` pinned by
  `nested_validation.rs`. Mutation-checked (neutering the `group_divs` scan fails the corpus pin).
- End-to-end: `check` flags empty `.input`/`.callout-note`/`.panel-tabset`, custom empty div
  stays silent, `--explain TAL-EMPTY-DIV` resolves.
- **Browser-verified** (isolated puppeteer): the `.tali-input-label` computed colour now equals
  `--tali-fg` (not `--tali-muted`) in light + dark, weight 600; the select keeps a native border;
  screenshots across mobile/laptop/portrait × light/dark are clean.
