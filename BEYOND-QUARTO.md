# Beyond Quarto: successor initiative roadmap

> The successor to `DROP-QUARTO.md` (complete 2026-06-24). DROP-QUARTO removed
> every backwards-compat shim and closed every schema. **Beyond Quarto cashes
> those closures into capability** and grows the tool deliberately past Quarto.
> Created 2026-06-24 from a 9-agent design workflow (Quarto gap map → 5 pillar
> designs → 2 adversarial critiques [invariant-guardian + scope-skeptic] →
> synthesis). Companion to `backlog.md` (active items, read-often) and
> `DROP-QUARTO.md` / `AUDITS.md` (history).

## Thesis

qmd-fast goes past Quarto **not by matching its feature checklist, but by being a
different kind of tool: a warm, source-mapped, block-modeled live *process*, not a
batch compiler.** Quarto renders an HTML artifact and stops; every edit is a cold
full-document pass + page reload. qmd-fast keeps a content-hash block model, a
per-keystroke block diff, a warm server, a warm Jupyter kernel, and a cumulative-hash
freeze cache alive between edits. That combination is the moat, state-preserving
incremental update, click-to-source, zero per-edit startup cost, and no static-site
generator can copy it, because it is architecture, not a feature.

**"Wider too" is real but disciplined: wider in web-native capability inside a live
HTML view**, surfaced through the same block model that makes the moat pay off. It is
**not** new static output targets bolted onto a second compiler path, and it never
licenses preview write-back or rewriting the Do-NOT-touch machinery. (One sanctioned
exception, decided 2026-06-24: a **print/PDF track derived from the built HTML**, see
Pillar IV + Wave 5, where HTML stays the single source of truth and PDF is a
paged-media *rendering* of it, not a parallel format.)

## Operating philosophy: corpus-plus-roadmap

"The corpus is the spec" evolves into **corpus-plus-roadmap.** The relaxation is
precise and the discipline is preserved by one mechanism: **every new feature ships
pinned by a target corpus document added in the same change.** A proposal that names
its pin doc (`corpus/diagnostics/typos.qmd`, `corpus/layout/panels.qmd`,
`corpus/narrate/walkthrough.qmd`, …) is a real roadmap item; a proposal that cannot
name one is spec-by-wishful-thinking and waits until a document pulls it in. Scope
grows on purpose; the regression net grows with it, in lockstep. The corpus is still
the arbiter of "done", it just now includes documents that *lead* implementation as
well as record it.

## Unchanged guardrails (apply to every item below)

- **Block-model invariants** (corpus-enforced): every emitted block carries
  `data-block-id` (content hash) + `data-sourcepos`; included blocks carry
  `data-source-file`. Source mapping, incremental swap, and live-state preservation
  all key off this. Anything mutating the block model / diff / sourcepos is flagged
  and gated.
- **Single editing surface:** the `.qmd` is the only editing surface; the preview is
  read-only and never writes back. Click-to-source navigates, never writes.
  Source-edit ergonomics are *editor* commands, never preview gestures (the
  drag-to-reorder resolution).
- **HTML-only output** (PDF is a derived print rendering of the HTML, not a new
  compiler target).
- **Do NOT touch** (negative-ROI rewrites that risk invariants): the `:::` three-pass
  div machine (`divs.rs`), the BibTeX parser/formatter/CSL + `[@key]` lowering
  (`cite.rs`), the `{{< include >}}` source-map pass (`includes.rs`), the
  leading-underscore "not a page" convention, the numbering scanners, exec/freeze/
  kernel. New capability rides the *supported* seams, new `build_container` arms, the
  `qmdEnhancers` registry, additive block metadata, the diagnostics channel, never a
  rewrite. The `.qmd` content dialect (`:::`, `#|`, `@fig-`/`[@key]`, `{{< >}}`) is
  kept on purpose.

---

## Pillars

Tags: **value** {low/med/high/transformational} · **effort** {small/med/large/epic} ·
**risk** {none/low/med/high}. Each item ends with a one-line invariant note.

### Pillar I, Authoring intelligence (cash the closed schema)

DROP-QUARTO closed every schema but enforces only the top level; push the existing
`validate_keys`/`closest()` machinery all the way down, surfaced as click-to-source
diagnostics, trustworthiness Quarto's open keyspace structurally cannot match.

- [x] **`locate-render-warnings` (high / med / none). DONE @ `4c900fa`.** SUBSTRATE, build first.
  Enrich the render-warning channel from bare `String` to a located struct (message +
  optional file/line) so broken-citation / xref / bibliography warnings jump to source
  like front-matter errors already do (`serve.rs:764` `.at().with_frame()`). Changes
  only the warning return type. *Invariant: read-only; reinforces click-to-source for
  diagnostics; does not touch `cite.rs` lowering or the BibTeX formatter.*
- [x] **`nested-schema-validation` (high / med / none). DONE, merged @ `bdeebe5`
  (2026-06-24).** THE validation epic; absorbed four duplicate proposals into one
  workstream. Closed key sets + `closest()` did-you-mean for: `#|` / `//|` / `%%|` cell
  options (`render/validate.rs` `CELL_OPTION_KEYS`), callout kinds (`CALLOUT_KINDS`), and
  nested config blocks (`execute:` / `listing:` / `about:` / `hero:` children +
  front-matter top-level, `frontmatter::validate_front_matter`). **Clean-break decision
  (author directive 2026-06-24): the "unknown vs recognized-but-not-honored" distinction
  was DROPPED in favor of a single closed vocabulary per surface; any key outside qmd-fast's
  own vocabulary (typo OR Quarto term) is flagged, and the corpus was purged of all
  Quarto-only keys (a verified visible-HTML no-op).** Built on `locate-render-warnings` so
  every diagnostic is click-to-source. Closed the now-moot backlog P2. **Pinned:
  `corpus/diagnostics/typos.qmd`** + `nested_validation.rs` asserting the exact warnings +
  a corpus-wide clean-vocabulary guard. *Invariant held: read-only; no block-model/diff
  change; `:::` scanner, cite, includes, numbering, exec untouched.*
- [x] **`jsonschema-for-config` (high / med / none). DONE, merged @ `8fcea33`
  (2026-06-24).** Generated Draft-2020-12 schemas from the SAME consts the validator uses
  (`KNOWN_KEYS` + nested sets in `frontmatter.rs`, `NATIVE_KEYS` in `site/config`) via a
  `#[cfg(test)]` generator in `crates/core/src/schema.rs`; committed + bundled
  `assets/schema/qmd-frontmatter.schema.json` + `qmd-site.schema.json`, drift-locked
  by a golden-file test (`QMD_FAST_BLESS=1` regenerates). `qmd-fast schema [--out <dir>]`
  emits them; `configuration.qmd` documents the `# yaml-language-server: $schema=` on-ramp
  (front-matter caveat noted). `serde_json` added as a core DEV-dependency only (no new
  runtime dep). *Invariant held: additive only, a schema file is documentation not an output
  format; HTML-only intact.* **This completes Wave 1.**

### Pillar II, Live-edit supremacy (measure, market, close the loop)

The moat was bet-on but never measured or marketed; turn architecture into evidence +
a regression gate, then close the loop with the editor companion the client protocol
already waits for.

- [x] **`live-edit-benchmark-harness` (high / med / none). DONE, merged @ `b1b00b1`
  (2026-06-24).** = backlog #7's measurement half. A committed `tools/live-edit-bench`
  crate measures the moat through the real `render_document_with_includes → diff_blocks`
  seam. **Design decision (author): CI-safe core + live browser proof.** Kernel-free +
  deterministic: cold full render, warm edit-above render+diff, emitted `BlockOp` payload
  vs full HTML, and DOM preservation asserted at the DIFF level (the open-`<details>`
  block below the edit gets a `SetMeta`, never an `Update`), not via a committed browser
  harness. Headline on `corpus/posts/em-algorithm/index.qmd`: cold ~124 ms vs warm ~28 ms
  (warm amortizes lazy syntax/KaTeX init), payload 3.2 KB vs 270 KB page (**83x smaller**),
  54 `SetMeta` / 0 `Update`, DOM survives. Bin emits markdown + `RESULTS.json`; CI-safe
  regression gate asserts the invariants (not timings). The live browser proof + recording
  roll into `live-edit-hero-demo` (the `tools/record-demo` Playwright recorder already
  exists). *Invariant held: pure measurement; edits an in-memory copy, never source; reads
  only id/sourcepos/html; no change to crates/core or crates/server.*
- [ ] **`live-edit-hero-demo` (high / small / none).** = backlog #7's deliverable. A
  showcase doc (running `{js}` animation, open `<details>`, playing video, heavy code
  block low on the page) + a scripted read-only `tools/record-demo` walkthrough editing
  a paragraph *above* them: only the edited block flashes, everything else survives.
  Split-screen against the same edit in Quarto (full reload destroys all of it). Build
  after the benchmark so it cites real numbers. *Invariant: edits in the editor pane;
  preview shown as the read-only view it is, demonstrates single-editing-surface.*
- [ ] **`reverse-sync-coverage-audit` (high / small / none, strengthens invariants).**
  Make source-mapping total in BOTH directions. Forward `locatable()` accepts any
  `[data-block-id]`; reverse `highlightAtLine` needs a strict
  `^(\d+):\d+-(\d+):\d+$` sourcepos, so any block with a degenerate sourcepos is
  invisible to cursor sync. Audit + fix cell outputs, math, figures, title block,
  footnotes, includes. Add a corpus test asserting *non-empty* sourcepos matches the
  regex (NOT "every block", the References/footnotes blocks at `corpus.rs:94/252` are
  legitimately empty and stay exempt). *Invariant: strengthens the corpus-enforced
  block-model contract; fixes live at the attr-injection seam, never in numbering/
  figure/cite.*
- [ ] **`vscode-editor-companion` (transformational / large / none).** Phase 1 only:
  host + cursor loop. A thin extension that launches `qmd-fast preview`, hosts it in a
  webview (the `inWebview` relay already exists), reveals source on `qmd-goto`, and
  posts `qmd-cursor {file,line}` driving the already-built `highlightAtLine`
  reverse-sync (incl. deck-slide jump). The protocol is half-built and waiting for a
  host (consumer exists, no producer). Coordinate local-server launch with **backlog
  #1d** (LAN token). Land with/after `reverse-sync-coverage-audit`. **Phase 2 (capped,
  deferred): editor commands** (insert block / reorder slide) specced strictly as
  `.qmd`-buffer text transforms in the editor, never preview gestures. *Invariant:
  preview stays read-only; cursor sync highlights/scrolls, goto navigates.*

### Pillar III, New web-native capabilities (the genuinely-past-Quarto bets)

Invest where warm + source-mapped + block-modeled unlocks behavior a batch compiler
cannot reach, while resisting the reactive-VM trap.

- [ ] **`js-reactive-graph` (high / small / none).** A *minimal* transitive-downstream
  scheduler over the `//| name` / `viewof` / `inputs` edges the model already carries
  (`model.rs:47-52`); today any define re-runs *every* cell (`qmd-js.js:75`). Build a
  name→consumers map at `enhance()`, topo-sort, re-run only the downstream closure;
  keep the per-run `invalidation` teardown; detect + diagnose cycles. **~80 lines of
  client JS, NO Rust, NO model field, NO reactive VM** (the OJS-over-provisioning
  lesson). "No inputs" once-cells stay as-is. **GATE: commit a corpus reactive doc
  pinning the ~6 chains BEFORE writing the scheduler.** *Invariant: reads
  `data-name/viewof/inputs` only, never `data-block-id`; confined to `qmd-js.js`;
  re-derives the graph after an incremental swap.*
- [x] **`narrated-code-walkthrough` (high / med / none). DONE (2026-06-24, branch
  `feat/code-walkthrough`).** One `::: {.code-walkthrough}` div: a sticky code panel +
  prose `.step` divs (`lines="3-5"`); scrolling drives line-range focus (reuses the
  `.qhl-ln` spans + the deck's `.qhl-ln-hl`/`.qhl-lines-active` CSS class contract). Two
  new `build_container` arms (`divs.rs`): `.code-walkthrough` splits the first code block
  (the panel, `wrap_pre_lines`'d) from the steps column; `.step` carries `lines=` as
  `data-cw-lines` (the generic arm would drop it). One idempotent IntersectionObserver
  enhancer (`assets/js/walkthrough.js`, bundled in `code_scripts()`) focuses the step
  nearest viewport-centre; `deck.js` untouched (not loaded on pages), so the ~8-line
  line-spec parse lives in the enhancer. Grid prose-left/code-right, collapsing to a
  sticky-top single column on mobile (`base.css`). Located warning when a walkthrough has
  no code block (`validate.rs`). **Pinned: `corpus/narrate/walkthrough.qmd`** + a
  `render/tests.rs` emitted-contract test + a `validate.rs` warning test;
  browser-verified desktop + 390px (focus tracks scroll across all 4 steps, 0 console
  errors). Strong hero-demo content for #7. *Invariant held: inner blocks keep ids/
  sourcepos via `group_divs`; enhancer is read-only/scroll-only; `:::` scanner, cite,
  includes, numbering, exec, deck engine all untouched.*
- [ ] **`cell-language-registry` (high / med / low).** Ship the registry refactor +
  `{glsl}` only. Generalize the hardcoded `lang=="js"` gate (`mod.rs:1763`) into a
  registry of client-side cell languages over the existing `qmdEnhancers` seam, each
  emitting the same wrapper-div contract. Ship `{glsl}` (shader → live canvas, tiny
  vendor, strong hero demo). `{sql}`/DuckDB + `{ts}`/esbuild are CUT until a corpus doc
  needs one; then add exactly that one, ship-only-if-used, gated on **backlog #5**
  (license/size sign-off). *Invariant: same wrapper-carries-data-attrs contract;
  idempotent enhancer; never touches exec/freeze/kernel.*

### Pillar IV, Breadth (wider than Quarto, web-native, corpus-pinned)

Close the genuinely-web-native breadth gaps where qmd-fast is narrower than Quarto,
each pinned by an added corpus document so breadth never outruns the regression net.

- [ ] **`panel-tabset-margin` (high / med / none).** Add `.panel-tabset` (child
  headings → ARIA-tabbed panels, keyboard switch) and `.column-margin` / `.aside`
  (CSS-grid margin rail) as new `build_container` arms. The moat applies: an open tab +
  a running `{js}` widget inside it survive an edit elsewhere, Quarto's reload can't.
  **Pin: `corpus/layout/panels.qmd`** with a `@fig-` inside a tab (proves cross-ref
  resolves). Reduced-motion handling tracked under the a11y backlog item. *Invariant:
  inner blocks retain ids/sourcepos via `group_divs`; switch enhancer toggles
  `aria-hidden` only.*
- [ ] **`image-lightbox` (med / small / none).** Ship the lightbox half only:
  click-to-zoom overlay enhancer for figures / `.lightbox` images (`figure.rs` already
  has the aspirational comment). Zero-dep, read-only. The WebP/AVIF transcode +
  content-hash asset cache is DEFERRED to the backlog "Image optimization" item,
  triggered when a corpus doc goes image-heavy. **Pin: `corpus/media/gallery.qmd`.**
  *Invariant: image blocks keep ids/sourcepos; zoom never writes source; transcode
  cache (when built) is separate from `_freeze`.*
- [ ] **`callout-kind-contract` (med / small / none).** Make `callout_kind` a closed
  enum {note, tip, warning, important, caution} with bundled inline-SVG icons +
  `--qmd-callout-*` tokens + `appearance` (default/simple/minimal) + `icon=false`. The
  kind enum is defined ONCE and shared with `nested-schema-validation` (the validator
  reads it; unknown kinds warn + fall back to note). Color/spacing craft folds into the
  typography pass, not styled twice. Emission-only change in `build_container`'s callout
  arm. **Pin: `corpus/callouts/kinds.qmd`.** *Invariant: the `:::` scanner contract
  untouched; icons bundled offline.*
- [ ] **`print-pdf-track` (med / large / low), DEFERRED to Wave 5 (decided
  2026-06-24).** Produce a print/PDF *derived from the built HTML*, HTML staying the
  single source of truth: a paged-media pass (print CSS `@page` rules + headless Chrome,
  or `paged.js`) over `build` output, NOT a second compiler/format path. Page breaks,
  running heads, figure/table placement honored via CSS; `{js}`/video degrade to a
  poster/static frame. **Pin: `corpus/print/paged.qmd`.** *Invariant: HTML-only identity
  preserved (PDF is a rendering of the HTML, not a parallel format); no preview
  write-back; reuses the existing build artifact.*

### Pillar V, Identity, craft & OSS readiness

Make qmd-fast feel finished and public-ready: honest supply chain, premium typography,
books as versioned spec, integrity debt paid.

- [ ] **`prune-and-fix-stale-docs` (high / small / none).** LAND FIRST. Prune the
  suppress-only dead keys `title-block-banner` + `site-url` (zero consumers; let them
  warn or keep with a justifying comment). Fix docs that still claim Quarto-shaped
  config works (`docs/guide/reference/configuration.qmd:7,102-106`;
  `docs/internals/sites.qmd:34-48`), contradicted by
  `quarto_shaped_config_is_no_longer_parsed_and_warns`. Fix the stale `site/feed.rs` /
  RSS reference in CLAUDE.md's file map (verify against the post-de-specialization
  reality). Foundation for the schema + spec items. *Invariant: docs + a const list; no
  protected machinery.*
- [ ] **`third-party-truth` (high / small / none).** = backlog #5 deliverable. Rewrite
  `THIRD_PARTY.md`: it still lists deleted reveal.js + highlight.js and omits the
  vendored d3 + Observable Plot (ISC) that actually ship; mermaid is the sole CDN dep.
  Full inventory + `cargo-deny` + a grep test over `assets/js` so it cannot silently
  rot. A public repo with a wrong `THIRD_PARTY.md` is a real liability. *Invariant: docs
  + CI only.*
- [ ] **`typography-craft-pass` (high / med / none).** = backlog #6. `base.css` is flat
  17px/1.7. Add a `--qmd-scale` type ratio, `font-feature-settings` (ligatures,
  tabular-nums), align KaTeX with the body text, font-smoothing. Absorb the callout
  color/spacing craft so callouts aren't styled twice. CSS-only, zero web fonts
  (offline / FOUC-free is itself a past-Quarto default). Verify via chrome-devtools.
  *Invariant: CSS-only; block model unchanged.*
- [ ] **`version-stamp` (med / small / none).** `Cargo.toml` is `0.0.0`; add `--version`
  + a build colophon. Gates any launch. *Invariant: trivial; no machinery.*
- [ ] **`docs-as-spec` (med / large / none).** Lower priority; start after the
  validation epic stabilizes (so the spec describes settled behavior). An RFC-2119
  `.qmd`-dialect reference + a WebSocket protocol reference, promoting the dogfooded
  books to a versioned normative spec. (The `configuration.qmd`/`sites.qmd` fixes are
  handled once, by `prune-and-fix-stale-docs`.) *Invariant: authoring + version stamp;
  editor-only.*
- [ ] **`build-seo-completeness` (low / small / none).** The concrete spec of the
  existing backlog "built-site production quality" fold-in: at publish time, when `url:`
  is set, emit `sitemap.xml` + `robots.txt` + `Article`/`WebSite` JSON-LD, reusing the
  nav `_`/`.`/`draft:` exclusion. Build-time metadata files, HTML-only intact. Sequence
  after the `draft:` wire-up. *Invariant: build-time only; HTML-only.*

---

## Sequencing (waves)

Honors the invariant-guardian's hazard ordering (`prune → locate-render-warnings →
validation → jsonschema`) and the scope-skeptic's "land integrity debt first."

- **Wave 0, Integrity & foundation** (quick, zero-risk): `prune-and-fix-stale-docs` →
  `third-party-truth` (#5) → `version-stamp`. Cheap correctness debt from DROP-QUARTO;
  a public repo with a wrong `THIRD_PARTY.md` and docs describing a deleted shim is a
  liability.
- **Wave 1, Cash the schema** (cleanest direct payoff of DROP-QUARTO Phase 3):
  `locate-render-warnings` (substrate) → `nested-schema-validation` epic (pinned by
  `corpus/diagnostics/typos.qmd`) → `jsonschema-for-config` (generated from the
  now-correct consts).
- **Wave 2, Prove the moat (#7):** `live-edit-benchmark-harness` →
  `live-edit-hero-demo`. Turns the architectural bet into evidence + an automated
  state-preservation regression gate; quantifies the first-edit-after-restart penalty
  (which decides whether cold-start-prefix-warming ever returns).
- **Wave 3, Craft + breadth** (parallelizable, all read-only-additive, each
  corpus-pinned): `typography-craft-pass` (#6) · `callout-kind-contract` (shares the
  enum from Wave 1) · `panel-tabset-margin` · `image-lightbox` ·
  `narrated-code-walkthrough` · `js-reactive-graph` (after its corpus reactive doc
  lands).
- **Wave 4, Close the loop** (the deepest real past-Quarto move):
  `reverse-sync-coverage-audit` → `vscode-editor-companion` Phase 1 (coordinate with
  #1d). Phase 2 editor commands capped and deferred.
- **Wave 5, Print/PDF track** (deferred, decided 2026-06-24): `print-pdf-track`
  derived from the built HTML, pinned by `corpus/print/paged.qmd`. Scheduled when the
  HTML output is stable enough to be worth pinning a paged rendering to it.
- **Later / demand-driven** (not scheduled): `docs-as-spec` (after validation settles)
  · `{glsl}` registry (when a demo doc lands) · `build-seo-completeness` (at publish
  time) · everything in CUT/DEFER, revived only when a corpus doc or a measured penalty
  pulls it in.

**Backlog cross-refs (integrate, don't duplicate):** #1d → Wave 4 (companion LAN
token). #4 → optional branch of the Wave 2 benchmark + the gate for any crossref-family
work. #5 → Wave 0 (`third-party-truth`). #6 → Wave 3 (`typography-craft-pass`). #7 →
Wave 2 (benchmark + hero demo).

**Quick foundational wins:** all of Wave 0, plus `reverse-sync-coverage-audit` and
`image-lightbox`. **Epics:** `vscode-editor-companion`, `docs-as-spec`,
`print-pdf-track`.

---

## CUT / DEFERRED / PARKED (ratified 2026-06-24)

- **CUT, `js-kernel-rerun`** (live input drives a kernel re-run): the biggest
  scope-and-invariant hazard. Touches `exec.rs` + both servers + freeze keying
  (freeze-poisoning is the trap), needs a kernel variable-injection primitive + a new ws
  message, for a loop *no corpus doc exercises*. Building a hosted-notebook product, not
  this tool. Revisit only if a roadmap doc demands it; if ever pursued: feature-gated,
  preview-only, off in `--no-exec`, depends on `js-reactive-graph`, hard merge-gate = "a
  slider re-run leaves `_freeze` byte-identical."
- **CUT, `cold-start-prefix-warming`:** backlog already judged "not worth it until it
  bites"; touches Do-NOT-touch exec/freeze/kernel; a background warm racing a real edit
  risks a partial `ran` record that corrupts `plan()`. Revisit only if the Wave 2
  benchmark proves the penalty hurts; then behind `QMD_FAST_WARM_PREFIX`, default off.
- **CUT, `deck-structural-incremental-swap`:** optimizes the rarest edit (add/remove a
  slide heading) on a fallback that already works, at the highest risk in its pillar.
  The full-render fallback is the permanent answer.
- **CUT, `crossref-family-and-labels`** (`lem-`/`cor-`/`prp-`/`exm-`/`exr-` + label
  overrides): speculative breadth for math docs not in the corpus. Revisit if backlog
  #4's testbed sweep shows real demand, then pin with `corpus/refs/theorems.qmd`.
- **DEFER, `deck-typed-slide-effects`** (`take_bg_attrs` string surgery → typed `Block`
  field): high-invariant-risk `model.rs` refactor (must emit byte-identical `<section>`
  HTML or block ids shift) whose win is mostly internal tidiness. Defer until a NEW slide
  effect actually needs to be added safely; then phase it, the typed-field refactor
  ALONE first, merge-gated on section-HTML byte-equality, features (transitions, footer/
  logo, mobile/touch) only after it lands green. Footer/logo + mobile/touch stay separate
  demand-driven backlog items.
- **DEFER, `cross-doc-live-embed`** (live source-mapped transclusion): genuine
  past-Quarto idea, but large; needs a host→source registry in `serve_site.rs` that
  compounds the known "visited pages never evicted" bug, and no corpus doc needs it.
  Revisit when a doc wants it; design the registry WITH eviction (fix the bug, don't
  worsen it); first cut = a single labeled block.
- **DEFER, image transcode / `srcset`** (split from `image-lightbox`): the backlog
  "Image optimization" item, demand-triggered.
- **PARKED, the rename** (`qmd-fast` → "quoin"): deferred per the author; keep the
  `.qmd` extension either way. Not an active pillar.

---

## Risks & non-goals

**What could go wrong**

- **The reactive trap (highest design risk).** `js-reactive-graph` must stay a ~80-line
  closure scheduler over the ~6 declared corpus chains, pinned by a committed reactive
  doc *before* coding. The OJS lesson is explicit: do not regrow a dataflow VM. Reject
  any expansion beyond declared `//|` edges.
- **Validation cluster re-fragmenting.** The four-into-one merge is the single biggest
  waste-avoidance. Keep it one epic on the `locate-render-warnings` substrate.
- **The four hazard-items creeping back.** `js-kernel-rerun`,
  `cold-start-prefix-warming`, `deck-structural-incremental-swap`,
  `deck-typed-slide-effects` are the only proposals that would mutate block model / diff
  / freeze / exec. If any is revived it ships feature-gated with an explicit invariant
  test as a hard merge gate: freeze-byte-identical (kernel-rerun), section-HTML
  byte-identical (typed-effects), a chrome-devtools assertion that a `{js}` slide widget
  + current slide survive (structural-swap), atomic-`ran`-write (prefix-warming). No
  exception.
- **Vendored-bundle / licensing balloon.** `cell-language-registry` is where "wider too"
  can bloat. Ship the registry + `{glsl}` only; each additional engine is a separate
  ship-only-if-used decision gated on #5.
- **The companion's editor commands metastasizing into WYSIWYG.** Phase 2's "reorder
  slide" must remain a `.qmd`-buffer text transform in the editor, the exact thing the
  removed drag-to-reorder feature got wrong. Cap the command set.
- **Print track scope-creep into a real format.** `print-pdf-track` must stay a paged
  rendering *of the built HTML*. The moment it forks into a separate Pandoc/Typst/LaTeX
  path it has violated HTML-only, that is the line.

**Explicitly OUT (non-goals)**

- New static output *formats*, LaTeX / Typst / Word / ePub, or any PDF path that is not
  a paged rendering of the built HTML. HTML is the identity.
- Preview write-back / WYSIWYG / drag gestures. The preview is read-only forever.
- Rewriting any Do-NOT-touch machinery. New capability rides the supported seams.
- i18n / RTL, RSS / categories, Julia / knitr engines, drag-to-reorder, deliberate
  non-gaps for a solo English author with this corpus; not to be re-filed as breadth.
- The rename, parked, not a pillar.

## Provenance

Design workflow `beyond-quarto-roadmap` (9 agents: 1 Quarto-gap map, 5 pillar
designers, 2 adversarial critics [invariant-guardian + scope-skeptic], 1 synthesis;
~0.92 M tokens, 2026-06-24). Seeded by the post-DROP-QUARTO "what did dropping Quarto
unlock?" research (5 agents, 2026-06-24). Net of 28 designed proposals: ~15 survive as
distinct work (several merged from duplicates), 3 cut, ~4 deferred, 1 parked; the four
invariant-hazard items are off the critical path.
