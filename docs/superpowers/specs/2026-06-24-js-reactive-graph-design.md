# Design: `js-reactive-graph` (last Wave 3 feature)

Status: approved 2026-06-24. Branch `feat/js-reactive-graph`. Roadmap:
`BEYOND-QUARTO.md` Pillar III. ~70 lines of client JS in `qmd-js.js`, NO Rust, no model
change. Read-only-additive.

## Problem (verified)

A `{js}` input change fires only the *direct* listeners under that DOM input's name
(`qmd-js.js:44-47`), and the auto-registration (line 152) keys sinks under their
`data-inputs` names. So a **transitive** chain `viewof n → (name: squared, input: n) →
(input: squared)` is broken: the last cell lists `input: squared`, but `squared` is a
derived `//| name` (not a DOM input), so nothing fires its consumers and it never updates.

## Mechanism (qmd-js.js only)

- Record `defines` (`name || viewof`), `inputs`, and `container` on each cell.
- `buildGraph(r)` (called at the end of `enhance` when fresh cells mount; rebuilt as cells
  grow): a `consumers` map (`name → [cells listing it in inputs]`) + a global **topological
  order** via Kahn's over `producer.defines → consumer` edges. Leftover cells after Kahn's
  are a **cycle** → `diagnoseCycle` (console + a `qmd-js-error` `<pre>` in each cyclic cell,
  which is then excluded from scheduling).
- `downstreamInOrder(r, name)`: BFS over `consumers` following each hit cell's own `defines`
  → the transitive-downstream set, returned filtered through the topo order.
- `scheduleFrom(r, name)`: `runSequentially(downstreamInOrder(...))` — one controlled pass,
  each cell run once in dependency order (reusing `run()` + its `freshInv()` teardown). NOT
  cascading listener fires (the reactive-VM/OJS trap).
- The `registerInput` listener calls `scheduleFrom(r, name)` (then still fires any manual
  `r.listeners[name]` for the public `onInput` API). The auto-registration at line 152 is
  removed — `data-inputs` sinks now re-run via the graph (transitively).

## Scope decision (approved)

The closure governs the **input-change path** (frequent/interactive). The **define-landing
path (`bindDefines`) stays a full rebuild, unchanged** — define landing is rare (cold load /
kernel restart) and some cells read `qmd.defines.X` without declaring `//| input: X`
(fourier-transform); a closure there would regress those implicit readers. Full-rebuild is
correct and safe.

## Gate + pin

Per the roadmap, commit the corpus doc FIRST: `corpus/reactive/graph.qmd` —
`viewof n → (name: squared, input: n) → (input: squared)` (the transitive chain) + an
independent `viewof m → (input: m)` chain (proves isolation). README row.

## Verification (browser)

Drag `n`: the derived value recomputes and the transitive sink updates; capture the
independent `m`-chain output node and confirm its reference is unchanged (it did NOT
re-run). A scratch cyclic doc (not committed — a permanently-broken cell doesn't belong in
the corpus, and cycle detection is client-only so `cargo test` can't cover it) confirms the
diagnosed error. Full suite stays green (no Rust/HTML change).

## Invariants

Reads `data-name/viewof/inputs` only, never `data-block-id`; confined to `qmd-js.js`;
preserves the per-cell `invalidation` teardown; re-derives the graph after an incremental
swap; no Rust/HTML change; `deck.js` untouched.

## Out of scope (YAGNI)

A general reactive runtime/VM; define-path closure; multi-input fan-in optimizations beyond
the topo order; persisting the graph across reloads.
