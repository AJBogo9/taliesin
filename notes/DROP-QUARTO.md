# Dropping Quarto backwards-compatibility — initiative backlog

> **COMPLETE (2026-06-24). Successor initiative: `BEYOND-QUARTO.md`.** DROP-QUARTO
> removed every backwards-compat shim and closed every schema; Beyond Quarto cashes
> those closures into capability and grows the tool deliberately past Quarto. The
> guardrails are unchanged (block-model invariants, single editing surface, HTML-only,
> the Do-NOT-touch list). Scope evolves from "the corpus is the spec" to
> corpus-plus-roadmap (every new feature pins a target corpus doc). This file is kept
> as the historical record of the drop.

> Dedicated backlog for de-Quarto-ing Taliesin into a fully native `.qmd` tool.
> Created 2026-06-23 from a 49-agent verified audit (inventory per subsystem →
> corpus reality-check → adversarial verification → synthesis). Companion to
> `backlog.md` (open tasks, read-often) and `AUDITS.md` (audit records).

## Framing (the operating assumption for this file)

**Unlimited dev hours. Infinite iteration. The only thing that matters is the
end result: the best possible tool.** That changes the calculus the audit ran
under. The audit asked "is the effort worth it"; here effort is *free*, so the
question becomes "does this make the end result better, at all?" If yes, it's in
scope, however long it takes.

But "infinite time" does **not** mean "do everything." Two things still gate work:

1. **Zero/negative payoff.** Rewriting machinery that would come out *identical*
   gains nothing and risks regressing load-bearing invariants (sourcepos,
   click-to-source, incremental swap). Infinite hours don't make a lateral move
   worth the invariant risk. These live in **§ Do NOT touch** with their reasons.
2. **Dependency order.** Some drops must be gated behind a working native
   replacement before the old path is deleted (deck engine, OJS), or they break
   the corpus. Sequencing matters even when time doesn't.

**The single most important finding:** the feeling that "Quarto compat holds me
back" is ~70% misattributed. The big complex files (`cite.rs`, `divs.rs`,
`includes.rs`, numbering) are **intrinsic** to Taliesin's own goals (click-to-source,
exact sourcepos, incremental block swap), not Quarto mimicry. Dropping Quarto
would not shrink them. The genuine constraint is small and specific: the deck
engine wears reveal.js's vocabulary, and the OJS runtime is a 440 KB black box.

Source migration is a non-issue: the corpus will be LLM-rewritten to any native
format. Cost concentrates in **config design + the deck/OJS rewrites + the one
extension port**, not in the documents.

---

## North star: what "fully native" looks like

When this initiative is done, Taliesin accepts **only** its own format and owes
nothing to Quarto's spelling:

- Config file is `_site.yml`, a **closed** native schema that *warns on any
  unknown key* (today it silently tolerates a 74-key Quarto superset).
- No `project:`/`website:`/`book:`/`format:` nesting accepted; no `_extension.yml`
  `contributes:` shape accepted. The flat native paths are the only paths.
- The deck engine emits a **semantic** contract (`.qmd-deck` / `.qmd-slide[data-state]`),
  authored at honest CSS specificity, with no `window.Reveal` facade and no
  `.reveal-viewport` shim. Slide effects (backgrounds, auto-animate, pauses) are
  **block-model attributes emitted server-side**, not post-hoc HTML string surgery.
- Interactivity runs through a native, debuggable `{js}` enhancer path against the
  live block DOM — no 440 KB vendored reactive VM.
- No file is named after the tool we left; no dead `--quarto-hl-*` / `--bs-*` CSS
  vars; no Quarto class names emitted for compat.
- The `.qmd` *content dialect* (`:::` divs, `#|` cell options, `@fig-`/`[@key]`,
  `{{< >}}` directives) is **kept on purpose** — it's a good design we'd choose
  anyway, and re-spelling it gains nothing. "Native" is about the project
  contracts (config, extensions, deck, runtime), not re-inventing markdown.

---

## Per-subsystem verdict (verified)

Adversarial verification cut the raw inventory's "delete N lines" claims 2–5×.
These numbers are the corrected ones.

| Subsystem | Verdict | Real Δ | Why |
|---|---|---|---|
| **Deck engine (reveal contract)** | **DE-REVEAL** | ~30–60 removed / ~550 freed | The one genuine design-freedom prize. Own ~1600 LOC engine speaking reveal's vocabulary only so reveal themes load. |
| **OJS subsystem** | **RE-ARCHITECT** | ~220 LOC + 440 KB asset | Corpus has 13 files w/ `{ojs}`/`ojs_define`. Can't delete — rebuild interactivity natively. |
| **Site config shim** (`site/config/quarto.rs`, 109 L) | **DELETE** | ~117 | Self-documented as deletable; native flat schema already the real model. |
| **Extension shim** (`render/extension/quarto.rs`, 48 L) | **DELETE** | ~50 | One manifest user (liquid-glass); native flat path exists. |
| **`_quarto.yml` filename** | **RENAME → `_site.yml`** | ~35 (no logic) | Most visible Quarto identity; click-to-source on config literally points at `_quarto.yml`. |
| **`KNOWN_KEYS` allow-list** | **CLOSE** (74 → ~25) | ~12–20 | Strict native schema = warn-on-unknown DX. Corpus-audit first (keep `page-layout`/`title-block-*`). |
| **`listings.json` sidecar** | **RESHAPE** | ~42 | Replace with server-side prev/next chrome (reuse `book_nav_html`). |
| **Parse→emit string surgery** | **RESHAPE (partial)** | ~75 | Real fragility; but `strip_trailing_hardbreak` + `parse_heading_attr` are intrinsic. |
| `:::` three-pass machine | **KEEP** | — | Sourcepos-preserving; syntax stays; rewrite is lateral + invariant risk. |
| Citations / BibTeX / CSL | **KEEP** | ~8 (dead `csl:`) | `.bib` is universal; formatter intrinsic to any bibliography. |
| `{{< include >}}` source-map | **KEEP** | <25 | Own embed/video share the `{{< >}}` delimiter; LineOrigin map is intrinsic. |
| Leading-underscore convention | **KEEP** | ~2 actual | Replacement is *more* code + an ordering dependency. |
| Book numbering scanners | **KEEP** | ~3 (class rename) | `section_number` already shared; TOC inherits it. |
| exec / freeze / kernel | **KEEP** | ~0 | Most Quarto-*independent* area; format is our own, built *against* Quarto. |
| Theming + page assembly | **KEEP** | ~0 | `--qmd-*` vars already native; only dead vars live in OJS CSS. |

---

## Phases 1-3 (DONE, 2026-06-23, on `main`)

Completed and verified (cargo test + clippy -D warnings + fmt green; corpus sites
build). One divergence from the original wording is recorded below.

- **Phase 1.** Deleted both Quarto-compat shims (`site/config/quarto.rs`,
  `render/extension/quarto.rs`); the flat native schema is the only path. Migrated the
  two Quarto-shaped corpus configs (`tech-blog`, `bayesian-book`) and the liquid-glass
  `_extension.yml` to native flat shape (the rest were already native). A Quarto-shaped
  config now warns / falls through rather than parsing.
- **Phase 2.** Renamed the project config `_quarto.yml` to `_site.yml` (sentinel,
  watcher, the 3 `data-qmd-src` chrome anchors, the includes project-root heuristic, all
  6 config files, docs / READMEs / comments). Leading underscore kept (free build-skip
  and page-walker skip).
- **Phase 3.** Closed `KNOWN_KEYS` from 74 to 27 keys (every corpus/docs-used key plus
  every key the code reads; kept `page-layout` / `title-block-*`); unimplemented keys now
  warn. Renamed emitted `header-section-number` to `qmd-section-number`. Deleted inert
  leftovers (`.quartoignore`, 2x `_metadata.yml`, `bayesian-book/index_cache/`). The
  "trestles about-template branch" was only a doc comment (no code branch), now reworded.
  **Divergence:** rather than "replace `listings.json` with server-side prev/next
  chrome," website posts were made **plain pages** (no prev/next), consistent with the
  earlier de-specialize-posts refactor whose corpus test forbids post-nav on website
  posts. The `listings.json` sidecar, the client `post-nav.js`, and its `_site.yml`
  wiring were deleted outright. Books keep their server-side `qmd-book-postnav`
  (unchanged).
- **Follow-up (out of Phase-3 scope, noted for later).** The corpus author stylesheets
  (`corpus/tech-blog/custom.css`, `theme.scss`) still carry dead Quarto-targeting
  selectors (`.quarto-grid-item`, `#quarto-margin-sidebar`, `--bs-*`, ...) that Taliesin
  never emits: a corpus-cleanup pass, not a core change.

## Phase 4 — De-reveal the deck engine (DONE, 2026-06-23)

The engine was already 100% own code (reveal.js gone) but wore reveal's vocabulary
so unmodified reveal *theme extensions* could load. **Scope change:** the author
decided liquid-glass does NOT need to keep working against Taliesin as-is (it will be
ported to the finished contract later), which removed the entire reason for the
compat layer. So this was a clean full de-reveal, not a lockstep port.

What shipped:

- **Native DOM contract.** `.reveal` → `.qmd-deck`, `.slides` → `.qmd-slides`;
  slides stay `<section>` but `class="slide level2"` → `class="qmd-slide"
  data-level="2"` (id slug + `.center` kept); `quarto-title-block` → `qmd-title-slide`
  (`#title-slide` id kept). Emitted server-side by `deck.rs`.
- **Dropped the dead state vocabulary.** `.past`/`.present`/`.future` were write-only
  (no CSS or JS read them); removed entirely (visibility is the camera transform).
  `setVisible` was dead code; removed. Event detail keys `indexh/indexv` → `h/v`.
- **`window.Reveal` facade dropped** (the alias + the `.reveal-viewport` shim). The
  sole public surface is `window.QmdDeck`. `client.js`/`serve.rs` use it; the
  static deck calls `QmdDeck.initialize`. `QMD_FORMAT` flag value `"reveal"` → `"deck"`.
- **Folded `reveal-extra.css` into `deck.css`** (one native deck theme).
- **Renamed** `render/reveal.rs` → `render/deck.rs`; the API `RevealParts` →
  `DeckParts`, `assemble_reveal_page` → `assemble_deck_page`, `reveal_page_from_doc`
  → `deck_page_from_doc`, `reveal_client_script` → `deck_client_script`.
- **Kept the author `.qmd` dialect:** `format: revealjs`, `.fragment`/`.incremental`,
  `:::{.notes}`/`:::{.magic-move}`, `{auto-animate=true}`, `{background-color=…}`,
  `. . .` pauses. (`DocFormat::Reveal` enum variant kept internally.)
- Rewrote the ~10 tests asserting the literal reveal vocabulary; updated docs
  (`deck-engine.qmd`, `architecture.qmd`, CLAUDE.md) + `globals.d.ts`.

**Acceptance met:** grep finds zero `reveal`/`.slides`/`past`/`present`/`future`/
`window.Reveal` in `deck.css`/`deck.js`/`deck.rs`; `cargo test` + clippy + fmt green;
decks verified live (nav, fragments, pauses, magic-move, auto-animate, backgrounds,
overview, speaker, PDF, drawing).

**Deferred (separate, invariant-touching step):** move the per-slide-effect *string
surgery* (`take_bg_attrs` byte-scan in `deck.rs` that hoists `data-background-*`/
`data-auto-animate` from a heading onto its `<section>`) into a typed `Block` field.
It touches `model.rs` + the diff/sourcepos invariants the corpus tests enforce, so
it's better done on its own. Not required for the de-reveal (the attributes are
already emitted server-side; only the *mechanism* is a string scan).

**liquid-glass:** NOT ported here. Its CSS/JS still target the old reveal vocabulary,
so `corpus/liquid-glass-slides/example.qmd` renders as a deck but unstyled by the
extension until the author updates `liquid-glass-revealjs` against the new
`.qmd-deck`/`window.QmdDeck` contract.

## Phase 5 — Re-architect OJS interactivity (biggest asset liability) — ✅ DONE (2026-06-24)

`quarto-ojs-runtime.min.js` was **440 KB** — by far the largest asset (12× the next,
`deck.js`) and a vendored black box. Replaced by a tiny native `{js}` cell enhancer.

**Decisions taken** (the audit flagged a, b, c for reactivity):
- **Reactivity = manual handlers (option a).** A corpus audit found the *entire*
  reactive surface is single-input fan-out (one input → a few sink cells; intermediate
  helpers are pure) — only ~6 chains. A dataflow engine (option b) was over-provisioned,
  so there is none: inputs are plain DOM elements, and a sink re-runs when a named input
  fires. Cross-cell values use `//| name:` helpers stored in a shared scope and read via
  `qmd.get()`; cells run sequentially (document order, awaited) so a helper resolves
  before a dependent reads it.
- **Charts = vendor offline.** d3 v7.9.0 (`d3.min.js`) + Observable Plot v0.6.16
  (`plot.umd.min.js`) are vendored as UMD globals (`window.d3`/`window.Plot`), shipped
  in `<head>` only when a page/deck has `{js}` cells. Three.js is `import()`ed by the
  cell. Net asset weight dropped ~440 KB and re-spread across two debuggable libs.

**What shipped:**
- `qmd-js.js` enhancer (registered through the same `qmdEnhancers` registry as mermaid):
  per-cell scope `{get, set, value, defines, onInput, container, invalidation}`; cell
  kinds from `//| viewof:` / `//| name:` / `//| input:`; per-run `invalidation` so cells
  tear down Three.js renderers / RAF loops on re-run.
- Wire format: `<div class="cell qmd-js-cell"><div class="qmd-js-out" id="qmd-js-…">` +
  `<script type="application/qmd-js" data-target/name/viewof/inputs>` carrying the
  source verbatim (only `</script` escaped — readable in devtools, no base64).
- Python→JS bridge: `ojs_define()` (author API name kept) now emits
  `<script type="qmd-define">`; the enhancer ingests it and re-runs dependent cells when
  a define lands after first paint (cold load / kernel restart).
- All `{ojs}` cells across the corpus + `docs/guide` + `samples/deck.qmd` ported to `{js}`.
- **Deleted:** `quarto-ojs-runtime.min.js`, `quarto-ojs.css`, `ojs-init.html`, the
  `ojs_head`/`ojs_init`/`has_ojs` glue, the `PageParts.has_ojs`/`DeckParts.has_ojs`/
  `PageCtx.ojs` fields, the `nodetype="declaration"` classifier, `client.js`'s
  `afterOjsMutation`/`qmdRunOJS`, and the `window.qmdRunOJS`/`qmdBindOjsDefines` globals.

**Verified:** all 7 interactive units (em-algorithm, pca-geometry/three-scene, fourier,
a-star, evidence-lower-bound, Kruskal-Wallis, docs `code.qmd`) render live with **zero
console errors and no Observable runtime loaded**. Full `cargo test --workspace` + clippy
(`-D warnings`) + `cargo fmt --check` + client `tsc` all green.

---

## Do NOT touch (out of scope even with infinite hours)

These are not deferred-for-cost. They are **negative-ROI regardless of time**: a
rewrite produces identical-or-worse code while risking the project's core
invariants. The audit's adversarial pass specifically reclassified these out of the
"prize" column.

- **The `:::` three-pass machine** (`divs.rs`). Half is intrinsic container/attribute
  emission that survives *any* container syntax. The rest is the blank-line/regroup
  dance that keeps sourcepos exact — load-bearing for click-to-source + incremental
  swap. comrak's `alerts` extension only covers callouts (6 of the corpus's divs); the
  rest (`.incremental`/`.notes`/`.sidenote`/`.panel-tabset`/`.magic-move`/`layout-ncol`/
  `#id` wrappers) still need the full machine — adopting alerts *adds* a second path.
  The source syntax stays identical, so this is a like-for-like rewrite with invariant
  risk and zero payoff.
- **BibTeX parser + formatter + CSL** (`cite.rs`). `.bib` is universal academic
  format (every reference manager exports it), tagged quarto-specific=FALSE. The
  ~250-line formatter is intrinsic to rendering *any* bibliography. Only ~8 LOC (the
  dead `csl:` field) is honest tax — drop just that. You can change citation house-style
  **today** without touching compat.
- **`[@key]` parse-lowering.** `transform_html`/`rewrite_text` are *shared* with
  cross-references (`@fig-`/`@sec-`, used 25× vs 17 cites) — Taliesin's own feature.
  The HTML walk stays for xrefs; only ~40 LOC is citation-only.
- **`{{< include >}}` source-map pass** (`includes.rs`). Your own `{{< embed >}}`/
  `{{< video >}}` use the identical `{{< >}}` delimiter, so fence-tracking + parsing
  stay regardless. The `LineOrigin` source map is intrinsic to includes-with-
  click-to-source. A YAML `includes:` list would *lose* mid-prose positional mapping.
- **Leading-underscore "not a page" convention.** Literally 2 lines (a `starts_with('_')`
  guard in two walkers). The reference-based replacement is strictly *more* code plus a
  cross-page ordering dependency.
- **Numbering scanners.** Already factored — `section_number` (`mod.rs:746`) is one
  shared function; the TOC inherits the mutated block number. Only ~3 LOC (the class
  rename, already in Phase 3) is real.
- **exec / freeze / kernel.** The most Quarto-independent area. The `_freeze` format is
  our own (not Quarto's notebook structure); the cumulative-hash cache was *explicitly*
  designed against Quarto's fragile per-cell cache. ~0 LOC attributable to Quarto.

> If a future idea proposes touching one of these, the bar is: *does the native
> result differ from the current one, in a way that's better?* If it'd come out the
> same, it stays.

---

## Design decisions (all RESOLVED — initiative complete 2026-06-24)

1. **Config filename:** → `_site.yml`. (Phase 2)
2. **Closed-schema strictness:** → warn on unknown config keys (keeps single-typo
   resilience). (Phase 3)
3. **Deck state model:** → numeric index in `window.QmdDeck`. (Phase 4)
4. **Native theme-extension API surface:** → `window.QmdDeck`; liquid-glass deferred
   (author updates the extension against the new contract). (Phase 4)
5. **OJS reactivity replacement:** → manual handlers. The corpus's whole reactive
   surface is single-input fan-out (~6 chains), so a dataflow engine was over-provisioned;
   inputs are DOM elements and sinks re-run on a named input firing. (Phase 5)
6. **Rename `reveal.rs` → `deck.rs`:** → done (plus `.reveal` → `.qmd-deck`). (Phase 4)

## Risks / regrets to accept knowingly

- Lose drop-in rendering of existing Quarto docs (acceptable — own corpus is
  LLM-migrated; but forecloses cheaply ingesting future Quarto docs/community extensions).
- De-revealing breaks the **entire** reveal theme-extension ecosystem, not just
  liquid-glass — any future third-party reveal theme becomes a manual port.
- The OJS rewrite may **degrade** genuinely-reactive posts (a slider re-running a plot
  for free), not just re-spell them, unless decision #5 rebuilds reactivity.
- Round-tripping back to Quarto (to use a Quarto-only feature, or collaborate with a
  Quarto user) becomes a manual reverse-migration.
- Over-closing `KNOWN_KEYS` can regress your own valid docs into warning spam — audit
  the corpus, don't trim from a list.

## Provenance

Full audit output: workflow `quarto-compat-drop-audit` (49 agents, ~2.6 M tokens,
2026-06-23). Memory: `quarto-compat-drop-audit`. Verified facts: 440 KB OJS runtime;
74 `KNOWN_KEYS`; 145-line `reveal-extra.css`; `deck.css` 238 / `deck.js` 100 reveal
hits + 2 `window.Reveal` calls; both shim dispatches are one line each; 13 corpus
files use OJS.
