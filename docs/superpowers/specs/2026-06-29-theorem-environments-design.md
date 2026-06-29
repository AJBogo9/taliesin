# Design: `theorem-environments` (revives the cut `crossref-family-and-labels`)

Status: designed 2026-06-29, not yet built. Branch `feat/theorem-environments`.
Roadmap: Pillar IV (Breadth, web-native, corpus-pinned) of `notes/BEYOND-QUARTO.md`.
This is the deliberate, in-discipline revival of the previously **CUT**
`crossref-family-and-labels` item, whose revival condition was "real demand, then
pin with `corpus/refs/theorems.qmd`": the demand is now evidenced (see Provenance) and
the pin doc is the one the cut item pre-named. Read-only-additive throughout; every
phase rides supported seams.

LaTeX-style theorem-like environments (definition / theorem / lemma / proof / ...): the
amsthm model, in a live HTML view. Numbered, cross-referenceable, semantically-labeled
blocks composed from machinery qmd-fast already owns (the `:::` div block model,
per-kind cross-ref prefixes, the post-pass numbering pattern, the callout aesthetic).

## Provenance

- Demand research (deep-research workflow, 105 agents, 2026-06-29; 22/25 claims confirmed
  by 3-vote adversarial verification). Verdict: build a focused subset. Demand is real,
  recurrent, and aligned with exactly this tool's target user (LaTeX-trained math/CS/econ
  authors publishing to HTML), though moderate in raw volume. The decisive arguments are
  **competitive parity** (Quarto ships 11 kinds, bookdown 9, Jupyter Book/MyST ~14 via
  `sphinx-proof`) and a **clean differentiator**: cross-environment **shared counters**
  are amsthm-standard in LaTeX but are NOT supported in bookdown HTML and require
  reimplementing Quarto's whole crossref counter machinery (a Quarto maintainer calls it
  hard, possibly impossible in books). qmd-fast owns its own numbering, so it can ship
  shared counters cleanly where the two biggest HTML competitors structurally cannot.
- Codebase integration probes (three qmd-explorer passes, 2026-06-29) confirmed every
  seam below against the live source.

## Decisions (locked with the author)

1. **Scope:** full vision, phased. One coherent spec; the MVP (phase 1) is independently
   shippable; later phases are clearly marked and separately corpus-pinned.
2. **Kind set:** core 8 fixed in the MVP (`theorem`, `lemma`, `corollary`, `proposition`,
   `definition`, `example`, `remark`, `proof`). Author-defined custom kinds are a later
   phase, not the MVP.
3. **Numbering default:** continuous, per-kind, document/page-wide in the MVP (Theorem 1,
   2, 3 ...; Lemma 1, 2, 3 ... independently), matching how figures/tables/listings number
   today. Section/chapter scoping ("Theorem 2.3") and shared counters are phase-2 config,
   because standalone posts have no visible section numbers (the `number-sections` feature
   is unwired) and figures/tables are continuous, so scoping is genuinely new work that is
   only coherent where section numbers are visible (books).
4. **Web-native affordances (phase 3):** hover-preview of refs, collapsible proofs,
   clickable QED + deep-link anchors. Semantic HTML + ARIA is MVP baseline, not optional.
5. **Slides (phase 4):** rich deck support (per-slide-group numbering + proofs that reveal
   on the next fragment step), built strictly additively over the deck's `registerPlugin`
   seam and existing fragment mechanism; the deck core is not touched.

## Verified seams (the facts the design rests on)

- **Dispatch:** new special divs slot into the `if/else if` chain in `build_container`
  (`crates/core/src/render/divs.rs`), exactly like `panel-tabset`/`scrolly`/`magic-move`/
  callouts. `parse_attrs` already handles `.class`, `#id`, and `title=`. No `:::`-scanner
  change.
- **Cross-refs:** `cite/render.rs::xref_label` already maps `thm -> "Theorem"` and
  `def -> "Definition"`, and `@thm-x` already parses today. Registration and resolution
  are two passes: targets are registered into `xref_registry`, then `cite::process`
  rewrites `@thm-x` runs. Broken-ref warnings (`data-qmd-xref` marker) and cross-page
  resolution (`site/xref.rs`) are automatic.
- **Numbering pattern:** `apply_table_captions` (`crates/core/src/render/mod.rs`) is the
  model post-pass: it receives the assembled `Vec<Block>` + the xref registry + warnings,
  walks blocks in document order, assigns numbers, injects label HTML, and registers
  xrefs. It runs AFTER `group_divs` and BEFORE `cite::process`. The insertion slot for a
  `number_theorems` pass between the two is free.
- **Numbering reality:** figures/equations/listings number continuously (bare integers),
  even in books; standalone posts have NO hierarchical section numbers (`sec_count` is a
  sequential counter over `{#sec-}` headings only, feeding `@sec-` resolution; headings
  show no number); only book pages get dotted "2.1" numbers, via the separate site-layer
  pass `site/chapter.rs::number_chapter_headings` using `BookEntry.number`. A post-pass can
  detect heading boundaries via `block_heading_level(&b.html)`.
- **Block model:** `build_container` builds the container `Block` with the div-span's
  `id`/`sourcepos`/`source_file` and embeds `data-block-id`/`data-sourcepos`/
  `data-source-file`; children keep their own attrs inside the concatenated HTML. The
  container is `cell: None` (never executed), correct for prose theorems.
- **Math:** KaTeX renders into child blocks during `emit`, before `build_container` sees
  them, so math inside a theorem Just Works; `cite::process` skips KaTeX `<annotation>`.
- **Validation:** `CALLOUT_KINDS` + `validate_callout_kind` in `render/validate.rs` show the
  const + dispatch pattern to mirror for a `THEOREM_KINDS` const (callouts get did-you-mean
  off the `callout-` prefix; theorems have no prefix, so no kind-typo did-you-mean — see
  Validation below).
- **Decks:** decks go through the SAME `render_internal_impl -> group_divs ->
  build_container` pipeline, so a `.theorem` arm reaches slides for free with attrs intact;
  but decks load only `deck.css` (not `base.css`); the deck has a complete fragment
  mechanism (`.fragment` + `qmd-frag-visible`) and a `QmdDeck.registerPlugin(p)` plugin
  seam with `on('ready'|'slidechanged'|'fragmentshown'|...)`.

---

## Phase 1: MVP

Independently shippable. Styled, numbered, cross-referenceable theorem blocks + a proof
environment, with continuous per-kind numbering, the cross-ref prefixes, theming,
validation, and ARIA baseline.

### Kinds, styles, prefixes

| Kind | Style | Ref prefix | Numbered |
|---|---|---|---|
| `theorem` | plain | `thm` | yes |
| `lemma` | plain | `lem` | yes |
| `corollary` | plain | `cor` | yes |
| `proposition` | plain | `prp` | yes |
| `definition` | definition | `def` | yes |
| `example` | definition | `exm` | yes |
| `remark` | remark | `rem` | yes |
| `proof` | (proof) | none | no |

The three amsthm styles: `plain` (italic body), `definition` (upright body), `remark`
(upright, lighter). `proof` is its own thing (italic "Proof." lead, upright body, an
auto-appended ∎). Prefixes match Quarto's, so existing Quarto/bookdown math docs migrate
with minimal rewrite (a happy alignment, not a compat tier).

### Authoring

```markdown
::: {.theorem #thm-pyth title="Pythagorean theorem"}
For a right triangle with legs $a, b$ and hypotenuse $c$, $a^2 + b^2 = c^2$.
:::

By @thm-pyth, the distance formula follows.

::: {.proof}
Place the triangle in two squares of side $a+b$ and compare areas.
:::
```

- Kind is the div class; `title=` is the optional name shown as "Theorem 1 (Pythagorean
  theorem)"; `#thm-` is the optional cross-ref id (prefix should match the kind).
- `proof` is unnumbered and not referenceable (matches Quarto/bookdown). Optional
  `title="Proof of the main theorem"` renames the lead.

### Emission contract (new `build_container` arm in `divs.rs`)

Dispatched when any class is in `THEOREM_KINDS` (checked alongside the callout arm). Emits
one container `Block` carrying the standard `data-block-id`/`data-sourcepos`/
`data-source-file` (the existing container pattern), shaped roughly:

```html
<div class="qmd-theorem qmd-theorem-theorem qmd-thm-style-plain"
     id="thm-pyth" data-qmd-theorem-kind="theorem"
     data-block-id="…" data-sourcepos="…">
  <p class="qmd-theorem-head">
    <span class="qmd-theorem-label">Theorem<span class="qmd-theorem-number"></span></span>
    <span class="qmd-theorem-title">(Pythagorean theorem)</span>
  </p>
  <div class="qmd-theorem-body"> …child blocks… </div>
</div>
```

The number slot (`<span class="qmd-theorem-number"></span>`) is emitted empty; the
post-pass fills it with `&nbsp;N`. The optional author `id` sits on the opening tag (the
post-pass reads it from that tag only, via `tag_end`, so a child block's `id` is never
mistaken for the theorem anchor).

`proof` emits `class="qmd-proof"`, a `qmd-proof-head` lead, the body, and a trailing
`<span class="qmd-qed" aria-hidden="true">∎</span>`.

The number is NOT assigned here (theorems are assembled after the flat-block loop). The arm
emits the head with the kind name and an empty number slot; the post-pass fills it. The
exact injection mechanism (slot marker vs. label-span surgery) follows the
`apply_table_captions` / `chapter.rs` precedent and is an implementation detail for the
plan.

### Numbering: `number_theorems` post-pass

New function in `render/mod.rs`, inserted between `apply_table_captions` and
`cite::process` (so `@thm-` resolves). Walks the assembled `Vec<Block>` in document order;
for each top-level block tagged `data-qmd-theorem-kind` of a numbered kind, increment that
kind's counter (a `HashMap<kind, u32>`), inject the number into the head's number slot, and
if the block has an `id`, `register_xref(id, n.to_string())`. `proof` is skipped (no
number, no xref).

Chosen over inline counting in the `build_container` arm because the post-pass is the
trusted, unambiguously document-ordered pattern (it is exactly how tables number). Known
limitation, identical to tables: a theorem nested INSIDE another container (e.g. a tabset)
is embedded in the parent block's HTML, not a top-level `Block`, so v1 numbers top-level
theorems. The corpus pin uses flat theorems; nested theorems are a documented edge, not a
goal.

### Cross-references

Add `lem`/`cor`/`prp`/`exm`/`rem` to `cite/render.rs::xref_label` (`thm`/`def` already
present). `proof` has no prefix. Resolution combines the prefix's label with the registered
number: `@thm-pyth -> "Theorem 1"` (linked). MVP uses the singular display name only;
plural/capitalized variants are phase 2. No change to the BibTeX parser/formatter/CSL or
`[@key]` lowering.

### Validation

`THEOREM_KINDS: &[&str]` in `render/validate.rs` is the dispatch vocabulary (single source
of truth, reused by `DivAttrs::theorem_kind` and later phases). Unlike callouts, theorems
have **no namespace prefix** (a kind is a bare class like `theorem`, not `callout-note`), so
a misspelled kind is indistinguishable from any other div class and falls through to the
generic `<div>` arm (rendering unstyled, which the author notices). The MVP therefore adds
**no kind-typo did-you-mean** (a tight edit-distance heuristic that avoids false positives
is deferred). The validation the MVP gets for free: `register_xref`'s duplicate-label
warning. An id-prefix/kind mismatch warning (e.g. `.lemma #thm-x`) is phase-2 polish.

### Accessibility (baseline)

The theorem head is real text ("Theorem 1 (Pythagorean theorem)"), read in document order
by a screen reader before the body, so the label is conveyed without any ARIA. The MVP adds
**no forced landmark role** (a theorem is primary content, not a "note", so `role="note"`
would mislabel it). The QED ∎ is `aria-hidden`. Math keeps KaTeX's existing MathML/aria.
Richer region semantics (e.g. a labeled region, if warranted) are reconciled against the
existing a11y-check tooling (`docs/superpowers/specs/2026-06-25-a11y-output-audit-design.md`
and the a11y check gate) rather than guessed at here.

### CSS / theming

A restrained left-accent box keyed by style, matching the callout aesthetic but lighter:
`--qmd-thm-{style}` accent tokens in `:root` (`assets/css/base.css`), border + label
emphasis driven by the token, light/dark derived via `color-mix` (so no separate dark
overrides, per the callout-kind-contract precedent). Per-style body typography (`plain`
italic, `definition`/`remark` upright). Structure the CSS as a self-contained block so
phase 4 can extract it to a shared `theorem.css` fragment without churn.

### Pin + tests

- **Pin: `corpus/refs/theorems.qmd`** (the roadmap-pre-named doc): all 8 kinds, two with
  `title=`, a proof with auto-QED, `@thm-`/`@lem-`/`@def-` refs resolving in prose, and
  math inside a theorem. Add a `corpus/README.md` row.
- `render/tests.rs`: emitted-contract test (each kind emits its classes +
  `data-qmd-theorem-kind`; `title=` renders the parenthetical; proof emits the QED;
  numbering is continuous per-kind).
- A cross-ref test: `@thm-pyth` resolves to "Theorem&nbsp;1" with the right anchor.
- `corpus.rs` invariants (block ids/sourcepos/order/uniqueness) hold for the new doc.

### Invariants (phase 1)

Read-only-additive: a new `build_container` arm, a new post-pass, additive `xref_label`
entries, an additive validator const, additive CSS. Untouched: the `:::` scanner contract,
existing figure/table/section numbering, `cite.rs` lowering + BibTeX/CSL, `includes.rs`,
exec/freeze/kernel, and the deck engine.

---

## Phase 2: numbering config + parity polish

Pinned by extending `corpus/refs/theorems.qmd` and adding a numbered-book exercise (a
`demo-book` chapter, or `corpus/refs/theorems-book/`).

A validated `theorems:` config block (front-matter and/or `_site.yml`), schema-generated
from the same const per the `jsonschema-for-config` pattern:

```yaml
theorems:
  number-within: chapter        # none (default) | section | chapter
  shared: [theorem, lemma, corollary, proposition]   # one shared counter
  numbered: true                # true | false | unless-unique
```

- **`number-within` scoping** makes book pages render "Theorem 2.3", wired into the
  existing `site/chapter.rs` machinery (chapter from `BookEntry.number`; within-chapter
  section number by replaying the `section_number` counter logic over headings detected via
  `block_heading_level`). On standalone posts, scoping is only coherent if section numbers
  exist, so it is gated: if there is no section numbering, warn (located) and fall back to
  continuous. The theorem numbering for the book path moves to (or is invoked from) the
  site layer where the chapter number is available.
- **Shared counters** (the differentiator): the post-pass groups the listed kinds under one
  counter key, so they draw a single sequence (Theorem 1, Lemma 2, Theorem 3 ...). This is
  the capability bookdown HTML lacks and Quarto cannot easily offer, and it is cheap here
  because qmd-fast owns its numbering.
- **Reference names**: each kind carries singular / plural / sentence-start-capitalized
  variants so cross-ref prose reads naturally ("see Theorems 2.1 and 2.3", "Lemma 4
  implies"). The `thmtools` "you must tell it the name" catch, designed in from the start.
- **`numbered`**: `false` suppresses the number (still styled); `unless-unique` numbers
  only when a kind occurs more than once.
- Phase-2 polish: warn on id-prefix/kind mismatch.

*Invariant:* additive config + schema (a schema file is documentation); the book numbering
reuses `chapter.rs` rather than rewriting it; the MVP continuous behavior is the default
when no config is present.

---

## Phase 3: web-native affordances

The live-HTML payoff. Pinned by extending `corpus/refs/theorems.qmd` (and/or a `reader/`
doc) with a referenced theorem, a collapsible proof, and a deep-linkable theorem.

- **Hover-preview of refs:** hovering a `@thm-`/`@lem-`/... link pops a card showing the
  referenced theorem's statement. Extend the existing `reader/` hover cross-ref card
  enhancer to theorem anchors (theorems are valid in-page anchors, so this is likely
  near-free; verify the card enhancer generalizes to non-figure anchors). The single most
  distinctive, no-competitor-does-this affordance: qmd-fast already has the block model +
  source map to render the referenced block on demand.
- **Collapsible proofs:** `::: {.proof collapse="true"}` renders the proof as
  `<details>`/`<summary>Proof.</summary>`, reusing the callout `<details>` collapse pattern.
  Default open; `collapse="true"` starts closed. A standing Quarto request (#5272) that
  LaTeX/PDF cannot satisfy.
- **Clickable QED + deep-link anchors:** generalize the `reader/` anchor-copy-link enhancer
  so clicking a theorem's label copies a deep link to it; the QED ∎ becomes a click target
  that collapses the proof (minor). The anchor half overlaps the existing reader enhancer.

*Invariant:* enhancers are read-only client behavior over the `qmdEnhancers` seam; the
preview never writes source; collapse reuses the existing `<details>` contract.

---

## Phase 4: rich deck support

The highest-risk phase because it extends the protected deck path; designed strictly
additively. Pinned by a purpose-built deck corpus doc (e.g. `corpus/refs/theorems-deck.qmd`)
with a theorem and a step-revealed proof.

- **Shared CSS:** extract the phase-1 theorem CSS into `assets/css/theorem.css` and
  concatenate it into BOTH the page CSS (`base.css` build) and the deck CSS (`deck.css`
  build), mirroring the `code-enhance.js`-into-fragments precedent. Single source of truth;
  fixes the "base.css absent in decks" gap.
- **Theorem blocks on slides:** automatic, since decks use the same pipeline. Numbering from
  the MVP post-pass is already baked into the HTML.
- **Proof reveal-on-step:** in `DocFormat::Reveal`, emit the proof body with
  `class="fragment"` (or honor a `. . .` pause marker in source) so it reveals on the next
  step. Uses the deck's existing fragment mechanism; NO `deck.js` change.
- **Per-slide-group numbering:** a client plugin registered via `QmdDeck.registerPlugin`
  that, on `ready`, numbers `.qmd-theorem` blocks per top-level `<section>` (slide-group)
  ancestor, overriding the continuous server numbers for the deck context. Additive; the
  plugin receives only the public facade.

*Invariant (hard guardrails):* theorem markup stays INSIDE a block's HTML (no new
`<section>` wrapper, which would shift the slide model); no phantom blocks added to
`doc.blocks` (would shift block ids and break the diff); `render_section`/`split_slides`
untouched; the plugin attaches only via `registerPlugin`. Section-HTML + block-id
invariants preserved. Browser-verify the deck (theorem styled on a slide, proof reveals on
step, current-slide + numbering survive) per the project's deck verification practice.

---

## Later: author-defined custom kinds

Deferred until a corpus doc needs it (amsthm itself predefines zero kinds, so this is the
canonical extension point). A config surface registering a custom kind -> style + reference
name + counter group, making `THEOREM_KINDS` config-extensible rather than a fixed const.
Not built until pulled in by a document.

---

## Build order

1. **Phase 1 (MVP)** — kinds + styles + proof + continuous per-kind numbering post-pass +
   cross-ref prefixes + CSS + validator + ARIA baseline; pin `corpus/refs/theorems.qmd`.
   Independently shippable; the rest is optional on top.
2. **Phase 2** — `theorems:` config: `number-within` scoping (books -> "Theorem 2.3") +
   shared counters + reference names + `numbered`.
3. **Phase 3** — affordances: hover-preview, collapsible proofs, deep-link anchors + QED
   click.
4. **Phase 4** — rich deck support: shared `theorem.css`, fragment-revealed proofs,
   per-slide-group numbering plugin.
5. **Later** — custom kinds, demand-driven.

Each phase ships pinned by a corpus document added in the same change, per
corpus-plus-roadmap. When phase 1 lands, move `crossref-family-and-labels` in
`notes/BEYOND-QUARTO.md` from CUT to an active/closed Pillar IV item referencing this spec.

## Out of scope (YAGNI)

- `\qedhere`-style exact QED placement after a display equation or list (a known LaTeX pain
  point); MVP right-floats the ∎ after the last line.
- A "List of Theorems" index/front-matter listing.
- Arbitrary per-theorem custom styling beyond the three styles + tokens.
- Numbering depth beyond chapter/section (e.g. `1.1.1` within-subsection scoping).
- PDF/print rendering of theorems: covered by the separate `print-pdf-track` (a paged
  rendering derived from the built HTML), not here.
- i18n / non-English reference names.
- Executable cells inside a theorem container (the container is `cell: None` by design).

## Cross-cutting invariants

HTML-only output; the `.qmd` is the only editing surface and the preview never writes back;
the block-model contract (`data-block-id`/`data-sourcepos`/`data-source-file`) is preserved
via the container pattern; the Do-NOT-touch machinery (the `:::` scanner, `cite.rs`
lowering + BibTeX/CSL, `includes.rs`, the existing numbering scanners, exec/freeze/kernel,
the deck core) is extended only through supported seams (a new `build_container` arm, a new
post-pass, additive `xref_label` entries, the `qmdEnhancers`/`registerPlugin` enhancer
buses, additive CSS), never rewritten. Each phase is corpus-pinned.
