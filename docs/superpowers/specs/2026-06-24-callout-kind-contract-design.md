# Design: `callout-kind-contract` (Wave 3 feature)

Status: approved 2026-06-24. Branch `feat/callout-kind-contract`. Roadmap:
`BEYOND-QUARTO.md` Pillar IV. Emission-only presentation contract on top of the
already-closed callout-kind enum (Wave 1's `CALLOUT_KINDS`). Read-only-additive.

## Decisions (approved)

1. `appearance` values: `default` (current boxed look), `simple` (no tinted title bar,
   border + icon only), `minimal` (no left accent, muted).
2. Introduce `--qmd-callout-{kind}` accent tokens; derive the title-bar tint via
   `color-mix(... var(--qmd-callout-{kind}) 12%, transparent)` so light + dark work from
   one definition → drop the 5 hardcoded `dark.css` callout-title overrides.
3. Icons: simple monochrome line icons (Lucide/Octicon-style), `fill="currentColor"` so
   they take the kind accent. One per kind, bundled inline (offline).

## Authoring

```markdown
::: {.callout-tip}
Default look, now with an icon.
:::
::: {.callout-warning appearance="simple"}
No tinted title bar, just the accent border + icon.
:::
::: {.callout-note icon="false"}
Icon suppressed.
:::
```

## Server (`crates/core/src/render/divs.rs`, callout arm)

- New `callout_icon(kind: &str) -> &'static str` helper keyed by the same kinds as
  `CALLOUT_KINDS`; returns an inline `<svg class="callout-icon" …>` or `""` for an
  unknown kind.
- Read `attrs.get("icon")` (default on; `"false"` suppresses) and `attrs.get("appearance")`
  (default/simple/minimal; unknown → default).
- Title becomes `{icon}{title}` inside `.callout-title`/`<summary>`. Wrapper class becomes
  `callout callout-{kind}` plus `callout-{appearance}` when not default.
- Both the plain and the `collapse` (`<details>`) variants get the icon.

## CSS (`crates/core/assets/css/base.css` + `dark.css`)

- Define `--qmd-callout-note/tip/warning/important/caution` accent tokens (`:root`).
- `.callout-{kind} { border-left-color: var(--qmd-callout-{kind}); }`;
  `.callout-{kind} .callout-title { background: color-mix(in srgb, var(--qmd-callout-{kind}) 12%, transparent); }`.
- `.callout-icon` (inline size, vertical-align, accent color via the kind border color).
- `.callout-simple` (transparent title bar, border only); `.callout-minimal` (no left
  accent, muted title).
- Remove the 5 `html[data-theme="dark"] .callout-{kind} .callout-title` overrides from
  `dark.css` (now derived from tokens). Keep dark token values if the accent needs to
  differ (it doesn't — `color-mix` with transparent adapts to the theme background).

## Tests + pin

- Pin: `corpus/callouts/kinds.qmd` — all 5 kinds + an `appearance="simple"` + an
  `icon="false"`; README row.
- Unit tests (`render/tests.rs`): each kind emits `<svg class="callout-icon"`; `icon="false"`
  suppresses it; `appearance="simple"` adds `callout-simple`; existing callout title /
  collapse / default-title tests stay green.

## Invariants

Emission-only; the `:::` scanner contract, block model, sourcepos, validation untouched;
icons bundled offline; `deck.js`/`deck.css` untouched. Fine color/spacing polish is left
to `typography-craft-pass` (so callouts aren't styled twice).

## Out of scope (YAGNI)

Custom per-callout icons (`icon="<name>"`); a `title-less` collapsed default; callout
folding animation changes.
