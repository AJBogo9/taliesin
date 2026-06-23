# Dropping Quarto backwards-compatibility — initiative backlog

> Dedicated backlog for de-Quarto-ing qmd-fast into a fully native `.qmd` tool.
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
`includes.rs`, numbering) are **intrinsic** to qmd-fast's own goals (click-to-source,
exact sourcepos, incremental block swap), not Quarto mimicry. Dropping Quarto
would not shrink them. The genuine constraint is small and specific: the deck
engine wears reveal.js's vocabulary, and the OJS runtime is a 440 KB black box.

Source migration is a non-issue: the corpus will be LLM-rewritten to any native
format. Cost concentrates in **config design + the deck/OJS rewrites + the one
extension port**, not in the documents.

---

## North star: what "fully native" looks like

When this initiative is done, qmd-fast accepts **only** its own format and owes
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

## Phase 1 — Cheap isolated deletes (low risk, do first)

The author pre-built the seams for these; downstream already reads native types.
An afternoon of mechanical work. Individually revertable, low blast radius.

- [ ] **Delete `site/config/quarto.rs`** (109 L) and the dispatch in
      `site/config/mod.rs:138-143` (`is_quarto` detection + `quarto::from_value`).
      The native `parse_native` becomes the only path.
- [ ] **Delete `render/extension/quarto.rs`** (48 L) and the `quarto::contribution`
      call in `render/extension/mod.rs:186`. Keep only the native flat-manifest path.
- [ ] **Migrate the 6 nested `_quarto.yml` + 1 nested `_extension.yml` to flat
      native shape** via LLM. Targets: `corpus/tech-blog/_quarto.yml` (the big one:
      ~20 html keys, navbar, footer, open-graph), `corpus/bayesian-book/_quarto.yml`,
      `corpus/demo-book/_quarto.yml`, `docs/guide/_quarto.yml`,
      `docs/internals/_quarto.yml`, and the liquid-glass `_extension.yml`
      (`contributes: → formats:` → flat).
- [ ] **Update test fixtures** that assert the Quarto-shaped path
      (`tests/config.rs`, `tests/extensions.rs` reference Quarto shapes).
- **Acceptance:** `cargo test -p qmd-fast-core` green; every corpus site still
  discovers its pages + renders chrome; clippy + fmt clean. A Quarto-shaped config
  now warns/falls through rather than parsing (intended).

## Phase 2 — Rename the config file (`_quarto.yml` → `_site.yml`)

Zero logic change; kills the single most visible Quarto identity. Pairs with Phase 1.

- [ ] **Change the sentinel** (the `_quarto.yml` filename literal in `site/config/mod.rs`
      `load_config` + the directory-is-a-site detection in `serve_site.rs` / `main.rs`).
- [ ] **Update the file watcher** (notify path for config hot-reload).
- [ ] **Update the 3 `data-qmd-src` chrome literals** that point click-to-source at
      the config file.
- [ ] **Rename the 7 corpus/docs config files** + fixtures.
- [ ] **Keep the leading underscore** (`_site.yml`) — it's a free "skip this page"
      signal to the page walker, no extra ignore logic needed.
- **Open question:** `_site.yml` vs `site.yml` vs `qmd.yml`. Recommend `_site.yml`
      (keeps the walker skip; reads as "the site config"). Decide before doing.
- **Acceptance:** all corpus/docs sites build + preview; click-to-source on a config
  line opens `_site.yml`, not `_quarto.yml`.

## Phase 3 — Close the config schema + cosmetic honesty

Make the native schema *strict* and strip remaining Quarto residue.

- [ ] **Audit the corpus, then close `KNOWN_KEYS`** (`frontmatter.rs:14`) from the
      74-key Quarto superset to the ~20–25 keys qmd-fast actually implements. The
      linter flips from "silently ignore unknown" to "warn on unsupported key" —
      strictly better DX. **CAUTION:** the audit caught the inventory naming
      *implemented-and-used* keys as dead — keep `page-layout`, `title-block-banner`,
      `title-block-style`, and anything the corpus relies on. Grep the corpus first;
      do not trim from a hand-written list.
- [ ] **Replace the `listings.json` sidecar** with server-side prev/next chrome that
      reuses `book_nav_html` (the book sidebar already computes ordering server-side).
- [ ] **Rename emitted `header-section-number` → `qmd-section-number`** (~3 LOC,
      class rename only).
- [ ] **Delete inert files** carried for Quarto: the 2 `_metadata.yml` that no longer
      drive anything, `corpus/bayesian-book/index_cache/` (leftover knitr cache),
      and `.quartoignore` (superseded by native ignore).
- [ ] **Drop the dead `trestles` about-template branch** if unused by the corpus.
- **Acceptance:** feeding a deliberately-misspelled config key produces a "did you
  mean" warning; corpus configs produce *zero* spurious warnings; prev/next nav
  still renders on every multi-page site.

## Phase 4 — De-reveal the deck engine (THE design-freedom prize)

The engine is already 100% own code (reveal.js is gone), but it's authored in
reveal's vocabulary so unmodified reveal *theme extensions* load. Verified surface:
`deck.css` (32 KB, 238 reveal-vocabulary hits), `deck.js` (80 KB, 100 hits, 2
`window.Reveal` calls), `reveal-extra.css` (145 lines, self-described as "the
typographic theme reveal's bundled CSS used to provide, now owned here"). The one
corpus extension (liquid-glass, the author's own repo) uses ~4 API methods + the
host class. This is where infinite hours pays off most — it unlocks design space,
not just tidiness.

- [ ] **Design the native deck contract.** Replace `.reveal/.slides/section` +
      stringly `past/present/future` classes with `.qmd-deck` / `.qmd-slide` carrying
      `data-state="past|current|next"` (or an index-based model). Decide the public
      JS surface: `window.QmdDeck` only (drop the `window.Reveal` facade alias).
- [ ] **Re-author `deck.css` at honest specificity.** Today CSS is deliberately
      low-specificity to lose specificity wars to a theme extension's `.reveal *`.
      Native ownership removes that constraint — rewrite cleanly.
- [ ] **Fold `reveal-extra.css` into `deck.css`** as an honest native theme (no
      "resupply reveal's bundled theme" framing).
- [ ] **Move slide effects to the block model.** Per-slide backgrounds, auto-animate,
      `. . .` pauses, `.r-stretch`, magic-move → emitted as `data-*` attributes
      server-side in `reveal.rs`, instead of post-hoc HTML string surgery.
- [ ] **Drop the `.reveal-viewport` shim** and any DOM scaffolding that exists only
      for the reveal contract.
- [ ] **Define a native theme-extension API** and **port liquid-glass** to it
      (`liquid-glass-revealjs` is the author's own repo — update in lockstep).
- **Gating:** do not delete the reveal contract until the native contract renders
      every corpus deck (`liquid-glass-slides/example.qmd`, `docs/guide/demo.qmd`,
      `tour.qmd`) correctly *and* the ported liquid-glass extension works live.
- **Acceptance:** all decks render + navigate (overview, speaker view, fragments,
      pauses, magic-move, auto-animate, backgrounds, PDF export, drawing all intact);
      liquid-glass loads against the native API; grep finds **zero** `reveal`/
      `.slides`/`past`/`present`/`future`/`window.Reveal` in `deck.*` + `reveal.rs`.
- **Design-freedom unlocked here:** real `data-state` transitions; per-slide effects
      as first-class block attributes; CSS at honest specificity; transitions/scaling
      not bounded by what a reveal facade can express; glass/background panels emitted
      server-side. Consider renaming `reveal.rs` → `deck.rs` once de-revealed.

## Phase 5 — Re-architect OJS interactivity (biggest asset liability, do last)

`quarto-ojs-runtime.min.js` is **440 KB** — by far the largest asset (12× the next,
`deck.js` at 80 KB) and a vendored black box you can't debug or modify. Plus ~220 LOC
of glue, `ojs-init.html`, the `class=cell`/`ojs-module-contents` wire format, and the
only dead `--quarto-hl-*`/`--bs-*` CSS vars. **Not a free drop:** 13 corpus files use
`{ojs}`/`ojs_define` (per-file cell counts up to 9), some genuinely reactive (sliders
re-running plots). This is the only place "LLM-rewrite the docs" is genuinely hard.

- [ ] **Design a native `{js}` enhancer cell** that runs against the live block DOM
      (mounts, re-runs on its own inputs, integrates with the incremental block model).
- [ ] **Decide the reactivity story.** The big loss is *automatic reactive recompute*
      (Observable's dataflow graph). Options: (a) accept manual wiring (event
      listeners) and rewrite reactive demos as explicit handlers; (b) build a small
      native reactive primitive; (c) keep a *much smaller* reactive lib than the
      440 KB fork. With unlimited hours, (b) is the "best end result" path but the
      most work — prototype against the `pca-geometry` Three.js demo + a slider post.
- [ ] **Build the Python→JS bridge** to replace `ojs_define` (13 bridge sites).
- [ ] **Rewrite the ~50 `{ojs}` cells** across the 9 interactive posts; verify each
      live (OJS needs a real server — browser-test via chrome-devtools MCP, not units).
- [ ] **Delete** `quarto-ojs-runtime.min.js`, `quarto-ojs.css`, `ojs-init.html`, the
      wire-format glue, and the dead CSS vars once parity is reached.
- **Gating:** delete nothing until the native path renders every interactive corpus
      post with equal-or-better behavior. This phase can regress posts if rushed.
- **Acceptance:** every interactive corpus post works live with no Observable runtime
      loaded; total bundled asset weight drops ~440 KB; the largest asset becomes
      `deck.js`.

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
  cross-references (`@fig-`/`@sec-`, used 25× vs 17 cites) — qmd-fast's own feature.
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

## Open design decisions (resolve before the phase that needs them)

1. **Config filename:** `_site.yml` (recommended) vs `site.yml` vs `qmd.yml`. (Phase 2)
2. **Closed-schema strictness:** warn vs hard-error on unknown config keys. Recommend
   warn (keeps single-typo resilience). (Phase 3)
3. **Deck state model:** `data-state` string vs numeric index + offset. (Phase 4)
4. **Native theme-extension API surface:** what methods/hooks an extension like
   liquid-glass targets (the old contract exposed ~4 reveal methods). (Phase 4)
5. **OJS reactivity replacement:** manual handlers vs a small native reactive
   primitive vs a lighter reactive lib. This is the single biggest design call in the
   initiative — it determines whether interactive posts keep "free" recompute. (Phase 5)
6. **Rename `reveal.rs` → `deck.rs`** once de-revealed? (cosmetic, Phase 4 tail)

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
