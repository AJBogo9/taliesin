# Taliesin audit records

The current deep audit + its active detail. The build-ready queue lives in
[backlog.md](backlog.md); older audit rounds (pre-2026-07-07) are archived in
[AUDITS-archive.md](AUDITS-archive.md).

**Subsystem audits (own detail files):** the **slide-deck** feature was deep-audited 2026-07-12 →
[2026-07-12-deck-audit.md](2026-07-12-deck-audit.md) (43 bugs + a keep/cut/fix/add feature verdict +
a mobile-feed spec + a grind order). Also queued as **section F** in [backlog.md](backlog.md). Note:
the deck mode-model is being reshaped (delete reader + PDF; add a mobile slide-feed) — read the file
before touching deck code so you remove rather than "fix" the outgoing behavior.

The **developer experience** was deep-audited 2026-07-18 →
[2026-07-18-dx-audit.md](2026-07-18-dx-audit.md) (DX/productivity research + discoverability-pattern
catalog + full DX-surface map + error/feedback-loop audit + 4 persona workflow simulations). Headline:
the tool's DX is well above median; **one finding dominates** — the excellent located "did-you-mean"
validators (broken links/images/media, dup ids, dangling xrefs) run in `build`/`check` but **not in
live preview**, so the fast loop is silent about the errors the author most needs while writing (every
persona shipped a broken doc because of it). Most recommendations are *surfacing an existing capability*,
not net-new. Prioritized feature queue in the file.

**DX1 LANDED 2026-07-18** (the dominant finding). Live static validation now runs on both serve paths:
a new `crates/server/src/preview_diag.rs` bridge converts the `check`-superset validators
(`check::page_static_diagnostics`, `Site::validate_cross_page_links`, `Site::warnings`) into
`protocol::Diagnostic`s; `serve::rebuild` runs the static set (Standalone) on pre-exec blocks, and
`serve_site::build_page` reaches parity (static InSite + cross-page filtered to the current page +
located `_site.yml` warnings, previously console-only). Spec/plan:
`docs/superpowers/specs|plans/2026-07-18-dx1-live-preview-validation*`. **The scope collapsed on
grounding — the exact backlog rot the audit itself warns about:** the "add a red-dot audit badge" work
**already existed** (`client.js` shows an amber count + red-on-error dot on the collapsed `◇</>` button),
and single-doc `serve` **already** surfaced xrefs + render warnings; the real gap was `serve_site`
parity, not "wire both paths + build a badge." The audit's "make cross-page checking incremental" was
also unnecessary (~27 ms whole-site re-derive; a debounced full run is fine). Browser-verified on both
paths (single-doc badge=3; site index badge=2; a clean sibling page shows only the site-global config
warning, no phantom cross-page). Method note: the helpers are unit-tested in-crate (the bin crate has no
lib target, so `tests/*.rs` can't see `pub(crate)` items); the live-socket wiring is verified via
chrome-devtools, not a unit test. Cheap follow-ups deferred: DX5 (unknown `:::`-class "did you mean")
and line-locating `_site.yml` warnings (`check` doesn't locate them either).

**DX2 LANDED 2026-07-18** (Tier 1 discoverability — highest discoverability-per-line). A one-time,
dismissible, localStorage-gated (`tali-hint-seen`) callout tethered above the collapsed `◇</>` dev
button surfaces the flagship Alt-click-to-source gesture + (where live) the `?` shortcuts menu — the
gesture previously self-advertised only *inside* the collapsed panel, so the blogger + speaker personas
"would have shipped never knowing it existed." All in `web-client/client.js` (built in `buildDevMenu`,
mounted into the existing `#tali-controls` host) + a CSS block appended to the shared server-side
`STATUS_CSS` const (`serve/mod.rs`), which both serve paths already inject. Preview-only by construction
(client.js is never in `build` — verified by grepping built output for `tali-hint-nudge` → 0). Four
dismissals, all persisting: Got it / opening the `◇` menu / the first *resolving* Alt-click / Esc.
**Per-line liveness** (`askLive`) omits the `?` line where it is a dead key — on a deck (reader menu is
`.tali-deck`-skipped) and when a reader has turned shortcuts off — mirroring `07-keyboard.js`'s existing
"don't advertise dead keys" discipline. Storage failures **fail closed** (treat as seen → never show):
an un-dismissable nag is worse than a missed hint, the opposite of `taliShortcutsOn`'s fail-open. Spec/
plan: `docs/superpowers/specs|plans/2026-07-18-dx2-first-run-preview-hint*`. **Grounding notes:** the
audit called this pure `[surface]`, but the "Alt-click a block" text existed *only* inside the collapsed
panel — the surfaced first-run nudge is genuinely small net-new chrome, not pure wiring. Like the dev
menu itself (and like DX1), it ships **no corpus pin** — the corpus is rendered *output*, and the dev
client is never in output; verification is a `STATUS_CSS`-contains-`.tali-hint-nudge` mutation-checked
Rust pin + `tsc` + a chrome-devtools loop across single-doc/site/deck + the mobile/laptop/portrait
matrix. A layout gotcha surfaced in-browser: `#tali-controls` shrink-wraps to the ~60px toggle, so the
absolute callout needed a fixed `width` (14rem), not just `max-width`, or it collapsed to a sliver (the
sibling `.tali-dev-panel` avoids this with `min-width:13rem`).

**DX3 LANDED 2026-07-18** (Tier 1 discoverability — "the config-authoring equivalent of shell
completion"). `taliesin init` now produces a project whose `_site.yml` autocompletes + red-squiggles in
any editor with a YAML language server, zero manual step: it emits the two bundled schemas into a
`.taliesin/` dot-dir and prepends `# yaml-language-server: $schema=.taliesin/tali-site.schema.json` to
the scaffolded `_site.yml`. One-file change (`cli.rs`: `INIT_SITE_YML` gains the modeline; `scaffold_init`
gains the two schema entries + per-file parent-dir creation; the all-or-nothing overwrite guard + written
list now cover them). **DRY:** reuses `taliesin_core::schema::{SITE_SCHEMA, FRONTMATTER_SCHEMA}` (the same
constants `taliesin schema` emits), so init's schemas can't drift from the validator — a test pins that
the modeline path resolves to a real file whose body **==** `SITE_SCHEMA` (mutation-checked). **Grounding
notes:** all three site walkers already skip `.`/`_`-prefixed dirs (page discovery `discovery.rs:117`,
`mirror_assets`, referenced-source deploy), so `.taliesin/` is neither a phantom page nor shipped into
`_site/` — integration-verified (built output has no `.taliesin/`; the emitted files are byte-identical to
`taliesin schema`; the modeline is an inert YAML comment, so `check`/`build` report no config warning).
Only the **site** schema is modeline-wired (into `_site.yml`, a real YAML doc); the front-matter schema is
emitted for the companion but not wired into `.tmd` files (a `.tmd` isn't a YAML doc a language server
processes). `init` is the sole `_site.yml` producer, so `new`/paper/post are untouched (DX10 covers those).

**DX10 MOSTLY LANDED 2026-07-18** (Tier 2 — "scaffolds that teach"; 3 of 4 sub-parts). The audit's
headline was that the single most-delightful discovery — Quarto's `#| label:`/`#| fig-cap:` cell options
**work verbatim** — was invisible, because no scaffold showed a runnable figure. Shipped: (1) `paper` now
scaffolds a worked `{python}` matplotlib figure (`#| label: fig-demo` + `#| fig-cap:`), a `$$` display-math
block, a `## Methods {#sec-methods}` section, and `@fig-demo`/`@sec-methods` cross-refs; (2) `init`'s
`index.tmd` "Next steps" points at `taliesin new`; (3) `new post --draft` — a `NewOpts`-threaded flag that
splices `draft: true` into the front matter. All in `cli.rs` (the pure `new_files` + thin `write_new`/
`cmd_new`), plus the extended `new_cli.rs` assertions and a regenerated `corpus/scaffold/posts/my-paper/`
mirror. **Grounding / gotchas:** (a) measured that `taliesin check` reports kernel/ipykernel status only as
an *informational* "Environment" block, never a counted diagnostic — so a `{python}` figure cell keeps a
scaffold check-clean with no kernel (exit 0), and `#| label: fig-x` resolves `@fig-x` **statically** (the
core corpus net renders without executing cells, yet fig-labelled corpus docs pass). (b) The scaffold has
BOTH a check-clean integration pin (`new_cli.rs` runs the real binary + `check`) **and** a byte-exact unit
pin (`every_scaffold_matches_its_corpus_pin`, fixed date `2026-07-10`) against `corpus/scaffold/` — the
paper mirror had to be regenerated with that fixed date, not today's, or the byte-pin fails. (c) `--draft`
defaults off, so every existing scaffold + the mirror stay byte-identical. **Deferred: `new deck --tour`**
(→ DX10-followup in backlog): a teaching deck's columns must use native `layout-ncol` (reveal's `.columns`
silently degrades — the pending **DX5**), so a column demo would teach a shaky idiom until DX5 lands; the
`NewOpts`/`NEW_FLAGS` plumbing is already in place for it.

-----------------------------------------------------------------------------

# Vacuous-test / mutation audit (2026-07-18)

**Why this lens.** Every prior round was source-driven (eye-driven browser passes, the
machine-facing surfaces, the reduction/modularity sweep whose headline was "the codebase is
already lean", the exec/kernel M-audit). Those saturated. The one lens never run as a
*deliberate* sweep is the codebase's own most-repeated, hardest-won finding — **"the tests
certify the defects"** (a green test that doesn't actually constrain the behavior it names).
As the source gets leaner, the surviving bugs are exactly the ones a vacuous test would let
through, so this lens gains value precisely where the others lose it. It also hardens the
regression net, which is the load-bearing asset.

**Method.** 4 read-only discovery agents (output-correctness / xref+citation / block-model+diff /
validation+freeze-keying), each proposing candidate vacuous tests with a concrete one-token
mutation + a SURVIVES/CAUGHT prediction. A `cargo-mutants` run (`taliesin-core`, `--lib`, the four
OG/SEO output files) as a mechanical backstop. **Every candidate was then verified by real
mutation** — apply the mutation, run the named test, watch it stay green — the "mutate the fix,
watch the named test fail" discipline this repo keeps re-deriving by hand. 13 agent candidates +
1 the agents missed but cargo-mutants caught (`sameAs`); all 14 confirmed, zero agent misfires.

**Landed the same day (test hardening; no production behavior change except one dead-code
removal).** Each new/strengthened assertion was mutation-checked (mutate → the NEW test fails →
revert → passes). Full workspace green, `cargo fmt` + `clippy -D warnings` clean.

| # | The hole (a green test that constrained nothing) | Fix |
|---|---|---|
| C4 | `is_safe_data_image` excludes `data:image/svg+xml` (SVG-XSS) with **no test** | added the svg+xml rejection case to `dangerous_url_schemes_are_neutralized` |
| C1 | the dedicated block-id test checks uniqueness+stability, never content-derivation (only 4 snapshot docs pinned it incidentally) | assert two different docs get different ids |
| C2 | tabset ARIA test asserts the attributes *appear*, not that tab↔panel pair | round-trip: a tab controls a panel that points back at it |
| C3 | diff had **no** 2-inserts-in-one-gap test, so `after_id` chaining was uncovered | `old=[a,d]→[a,b,c,d]` asserts the second insert chains off the first |
| A1 | the only real-math `llms.txt` test asserts length>2000 + a name, so `strip_katex` dropping inline math is invisible | assert inline KaTeX is stripped, not garbled into the text |
| A2 | OG-card pad-box test omits the `lead` field | added a long-`lead` case (mutation now escapes at x=1195 > 1128) |
| A3 | `og:type=website` (undated pages) had no test; only the dated `article` branch was pinned | assert the undated home page is `og:type=website` |
| A4 | feed `<title>` site-title fallback never exercised (every fixture sets a host title) | a title-less listing host → `<title>` = site title, not "Feed" |
| D1 | reading time only checked `contains(" min read")`, never the number | a 400-word doc must read "2 min read", not a constant |
| B1 | duplicate-xref-label test checks the warning fires, never the resolved number (the D53 flaw itself) | resolve `@sec-dup`, assert it keeps the first definition's number |
| B2 | duplicate-bib-key "uses the last definition" — never rendered to confirm which wins | format the dup entry, assert the last (Second/2002) wins |
| B3 | bracketed `[@fig-x]` cross-ref path had zero coverage | assert `[@fig-fit]` resolves to the figure link, not a bogus citation |
| D2 | `check` diagnostic assembly has no count assertion (any-exists only) | assert the broken-xref diagnostic appears exactly once |
| E1 | JSON-LD `sameAs` "tested" by a page-contains check the footer chrome also satisfies | assert `sameAs` is in the Person JSON-LD; **plus** a real latent: removed the dead `vocab` `about` description + added a bidirectional gate (D3) |

**Lessons that generalize (and match this repo's own log).** The sharp, targeted human-mutation
approach beat the mechanical sweep for *relevance* (agents ranked what matters), and the mechanical
sweep beat the humans for *completeness* (`cargo-mutants` found `sameAs`, which four agents missed) —
run both. Two apparent CAUGHTs were not agent errors: C1's behavior is only *incidentally* pinned by
snapshot docs (its named test is still vacuous), and D2 was a harness artifact (`taliesin-server` is a
bin crate, so `--lib` had no target). Process note for next time: when the test and the mutated code
live in the **same file**, `git checkout <file>` to revert a mutation also eats the new test — back the
file up and restore from the backup instead.

-----------------------------------------------------------------------------

# Taliesin: full multi-surface deep audit (2026-07-07)

**Method.** One multi-agent workflow (87 agents, ~6.9M tokens): 24 surface×lens finder
cells (parser/block-model, render, decks, exec+kernel, freeze/warm-pool, site/books,
web-client, diagnostics/check, CLI/build, docs, deps/licensing, accessibility, reader
craft, architecture/waste, feature-scouting), each finding adversarially verified
(refute-by-default), then per-surface dedup + novelty-tagging against the existing
backlog / AUDITS / FEATURE-IDEAS, a philosophy gate on every cut/feature candidate, a
deep-dive pass on the five hottest surfaces, and a final synthesis. 134 findings
survived verification: 0 critical, 1 high code bug, 7 high total. One cluster synthesis
(CLI/build/first-run) hit the structured-output retry cap; its verified findings are
recovered in the appendix below. Read-only audit, no code changed.

**The build-ready, batched implementation queue derived from this report lives in
[backlog.md](backlog.md) ("Audit 2026-07-07 implementation queue").** This section is the
detail reference behind it: verdict, top-leverage fixes, findings by theme, cut/keep/add,
and the low-severity long tail.

## Status — mostly landed (updated 2026-07-08)

**This is the original 2026-07-07 snapshot, not a live checklist.** The batched queue and
the top-leverage fixes have since shipped, so most findings below are already fixed —
**verify against current code before acting on any of them** (this bit an earlier grind:
the `image:`-URL bug, the theming/`#qmd-root`/schema doc-drift, and the deck `{#sec-x}`
anchor all read as open here but are fixed). Remaining open work is tracked in
[backlog.md](backlog.md) (Tier-2/Tier-3), not here.

Landed, by theme (batch → the "Findings by theme" section it clears):

- Batch 2 (`0b466c4`) → **Documentation drift (rename)**, the functionally-broken cluster.
- Batch 3 (`2369d80`) → **Accessibility** (Cmd-K contrast/ARIA, reduced-motion, slide bg,
  keyboard scroll, dialog).
- Batch 4 (`1132df3`) → **Cross-reference / section numbering** + consumed-anchor.
- Batch 5 (`561ff24` + `a6cf810`) → **Silent failure** (unclosed fence, figure `width=`,
  `draft: yes`, single-doc-build YAML, `_site.yml` typos).
- Batch 6 (`92dc677`) → **BibTeX layer** + the TOC/tabset double-escape.
- Batch 7 (`19022b7`) → `image:` URL, multi-page stale-file sweep, embed `--strict`,
  **deck title-slide hot-update**.
- Batch 8 (`41313f9`) → watcher prune, live site search index, reconnect state-preserve.
- Batch 9 (`1eb3238`) → **freeze cache + kernel** honesty + resource hygiene (partial;
  the remaining exec leaks stay in backlog Tier-2).
- Also: top-leverage #1 offline deck build (`478cdc1`), the `?qmd=embed` CUT (`679b76b`),
  the diff-then-broadcast consolidation (`e09744a`), the 2026-07-08 hardening fixes
  (byte-safe `percent_decode`; active-nav highlight on `#fragment`/`?query`), and
  top-leverage #7 (`e488abb`, the same-page link-preview source-attr strip — a shared
  `stripSourceAttrs` now neutralizes both the same-page and cross-page cards).
- Confirmed already-fixed by the audit's own "What held up" and now closed: the 390px hero
  overflow, the theme/video desync, and the heading `{#id}` dedup gap.

Notable **still-open** low-tail items the sweep did *not* touch (so they don't read as
done): `app.pages` unbounded ws-key growth, the deck `. . .`/`"Title Slide"` collisions,
several CLI/build appendix items, and the stale-but-working `qmd-*` alias docs. See backlog
Tier-2/Tier-3 for the tracked set. (`block_tag_has_id` substring match [`cbb4ee3`] and
`json_str` U+2028/2029 [`595c6fe`] have since landed.)

## Executive verdict

Taliesin came through a 134-finding, 24-surface deep audit with no critical defects and a single high-severity code bug. The load-bearing invariants the whole design rests on, unique block ids, total sourcepos, block-level incremental diffing, the freeze cache's no-stale-hit promise, and the read-only single-editing-surface, all held up under direct adversarial attack (see "What held up" below). The findings are not decay; they are under-enforcement and over-claiming.

Three themes dominate:

1. **Silent failure is the default.** The largest single cluster (roughly two dozen findings) is features that no-op or render wrong with no diagnostic, in direct tension with the project's own "surface bad news early" directive. Examples span every layer: an unterminated `:::` fence is silently dropped, quoted figure `width=` values are corrupted by smart-punctuation, `draft: yes` silently publishes a draft, the `ts`/`typescript` highlight alias is dead, and the single-doc `build` (the artifact-producing path) never surfaces malformed front-matter YAML that `check` and both preview servers catch.

2. **Accessibility advertised but shallowly delivered.** The a11y foundation is real (per-theme accessible color tokens, an SR-only convention, a `prefers-reduced-motion` CSS gate, emitted ARIA), but several flagship surfaces bypass it: the Cmd-K palette fails WCAG AA in every theme, deck auto-animate/magic-move route around the reduced-motion gate, and three surfaces emit ARIA (`aria-haspopup="dialog"`, `aria-haspopup="menu"`, a named combobox) whose promised behavior they never implement.

3. **Documentation drift from the rename.** The qmd->tali / qmd-fast->Taliesin rename never fully reached the dogfooded books; three onboarding recipes are functionally broken (custom-theme CSS vars, LSP schema filenames, the protocol element id have no back-compat alias) and the User Guide teaches the theme default as the exact opposite of the runtime.

Nearly every fix is a mechanical reconnection of an existing mechanism, not new infrastructure. Confirmation quality was high: of the findings reported here, all but a couple are marked CONFIRMED against source; the few PLAUSIBLE ones are flagged inline.

## Top highest-leverage fixes

1. **Offline-build breach for decks (HIGH, CONFIRMED).** `deck_page_from_doc` takes no `OutputMode` and hardcodes `code_scripts()` = Preview (`crates/core/src/render/deck.rs:94-108`), so a `build`-ed deck containing a Mermaid diagram ships a `cdn.jsdelivr.net` dependency instead of the inlined offline library the HTML page path correctly emits, and also ships every Preview-only enhancer as dead bloat. Thread `OutputMode` through and pin with a corpus test asserting no `cdn.jsdelivr.net` in a built deck.

2. **One located-diagnostic channel for the silent no-ops.** The math and front-matter paths already turn failures into click-to-source `Warning`s; extending that channel to unclosed fences (`divs.rs:143`), unresolved fence languages (`highlight.rs:51`), non-boolean boolean keys (`site/frontmatter.rs:56`), non-`.bib` bibliographies (`render/mod.rs:773`), and unknown `_site.yml` keys (`config/mod.rs:206`) retires most of the largest cluster in one coherent piece of work.

3. **AA token sweep.** Swap raw `var(--tali-accent)` for the existing `var(--tali-accent-fill)`/`--tali-on-accent` in the Cmd-K palette (`web-client/search.js`, the one HIGH a11y offender) and darken the sub-AA syntax comment token per theme (`base.css:338`, sepia `:583`, `deck.css:760`). The accessible tokens already exist; add a contrast assertion to stop regressions.

4. **Unify section numbering.** The flat `sec_count` (`render/mod.rs:390`) is the shared root cause of three-to-four `@sec-` cross-reference bugs. Registering the hierarchical `section_number` when a chapter is present collapses all of them.

5. **Freeze / warm-kernel honesty pass.** Scope the "stale hit impossible by construction / nothing to clear by hand" wording (`freeze.rs:11`) to code + interpreter version (a library upgrade is a real stale-hit path), and fix the mid-run kernel death that poisons the warm-prefix `ran` and wedges the preview (`exec.rs:610`).

6. **Docs rename drift.** Sweep `--qmd-*` -> `--tali-*` and `qmd-*.schema.json` -> `tali-*.schema.json` in the guide, fix the `#qmd-root` -> `#tali-root` protocol id, and correct the inverted theme-default statement (`theming.tmd`).

7. **Same-page link-preview card leaks the source-map surface.** Apply the cross-page attribute-strip to the same-page path too (`12-link-preview.js`), so a read-only preview card stops being a live Alt-click click-to-source target and stops seeding duplicate block ids.

## Findings by theme

### Silent failure (surface-a-diagnostic instead)

- **Authoring surface.** Unterminated `:::` fence silently dropped, content unwrapped, no warning (`render/divs.rs:143`, medium). `::: .callout-note` (leading dot, braces forgotten) renders as literal text; bare `::: classname .extra #id` silently drops all but the first token (`divs.rs:111`, low). Quoted figure `width=`/`height=`/`fig-align=` corrupted by smart-punctuation so the feature silently no-ops, already live and non-functional at `bayesian-website/subsections/_data-modeling.tmd:4` (`figure.rs:55`, medium).
- **Config/validation.** `draft: yes`/`on` silently publishes the draft (YAML-1.2 coerces to the string, then to false); same class mis-reads `toc: yes` and `execute: {echo: no}` (`site/frontmatter.rs:56`, medium). Single-doc `build` never runs `yaml_error()`, so malformed front-matter YAML builds clean and passes `--strict` (`frontmatter.rs:107`, medium). `_site.yml` nested nav/footer/mount typos degrade silently and even top-level site warnings ship unlocated (`config/mod.rs:206`, medium). Non-`.bib` bibliography path ignored with no diagnostic (`render/mod.rs:773`, low). Non-HTML `format:` values (pdf/typst/docx...) pass `check` clean and are silently rendered as HTML (`frontmatter.rs`, low).
- **Highlighting.** `ts`/`typescript` alias is dead (syntect ships no TypeScript), so TS blocks render unhighlighted with no signal; `toml` likewise absent (`highlight.rs:36`, low). Any unresolved fence language degrades to plain text silently (`highlight.rs:51`, low).
- **Runtime.** The file watcher registers a recursive inotify watch over the whole tree including `node_modules`/`.git`, so a large project exhausts `max_user_watches` and silently kills hot reload (`serve/mod.rs:877`, medium). The site Cmd-K index freezes after a content edit in preview while single-doc search stays live (`serve_site/mod.rs:1035`, medium). Mermaid render errors are swallowed by an empty `catch`, and a successful re-render after an offline failure leaves the stale banner (`mermaid.js`, low).

### Cross-reference / citation correctness

- **Section numbering (shared root cause).** Same-page `@sec-x` in a book shows a flat number contradicting the heading's hierarchical number (`render/mod.rs:390`, medium). Cross-page `@sec-x` on a non-book website is mislabeled "Chapter N" (`site/xref.rs:215`, medium). The hover-preview card for a book section heading drops its number (`site/mod.rs:759`, low). All three collapse behind the `section_number` helper.
- **BibTeX layer.** `@inproceedings`/`@conference` silently drop `booktitle` and `pages`, the single most common citation type in the CS/ML audience (`cite/format.rs:22`, medium). Parenthesis-delimited entries cascade-drop every following reference (`cite/parse.rs:32`, medium). Manager exports (JabRef) commonly emit both forms.
- **Citation rendering (deep dive).** Pandoc-style prefix text is silently deleted from a bracket group (`[see @doe2020]` -> `[1]`) (`cite/render.rs`, low). A bib key beginning with an xref prefix (`rem-`, `fig-`) is uncitable and emits a spurious broken-xref warning (low). A locator on a bracketed cross-reference is dropped (low). `transform_html`'s tag scanner treats the first `>` as the tag end, so a `>` inside an HTML comment leaks citation processing into non-text context (low).
- **DOM ids.** Duplicate `fig-`/`eq-`/`tbl-`/`lst-` labels emit invalid duplicate DOM ids (only warned, not deduped); the heading half of this was already fixed, this is the non-heading remainder (`render/mod.rs:455`, low). A heading consumed as a callout title discards its `#id` while its `@sec-` number was already registered, leaving a resolving ref pointing at a missing anchor (`divs.rs:395`, medium). A manual mid-document References heading is left detached from its list (`cite/render.rs:81`, low).

### Load-bearing invariants under-enforced

- **Block model.** The diff's LIS silently assumes globally-unique block ids with no assertion in the post-exec hot path (`diff.rs:185`, low, trivial `debug_assert!`). A block straddling an include boundary emits a mixed-file sourcepos (start file + end line from a different file), a hole totality-only checks miss (`render/mod.rs:286`, low).
- **Incremental payload.** Ops are broadcast one-message-per-op onto a 256-slot ring; a large structural edit self-overflows it and forces a full re-render, discarding the ~83x incremental win (`serve/mod.rs:1069`, medium, already-tracked as op-batching but with new overflow detail). The diff-then-broadcast core is copy-pasted across the two dev servers, giving the payload-shape contract two owners (`serve/mod.rs:992`, medium). A websocket reconnect wholesale-remounts and destroys all live block state (WebGL/{js}/video/open-details) even when the document is byte-identical, on any sleep or wifi blip (`web-client/client.js`, medium).
- **Read-only preview leaks.** Same-page link-preview card keeps cloned `data-block-id`/`data-sourcepos` (`12-link-preview.js`, low). `qmd-cursor` reverse-sync accepts postMessage from any origin/frame (verified read-only, so no write-back breach) (`client.js`, low). `click_block` prints client-controlled strings to the author's terminal unsanitized, a terminal-escape injection beyond the documented worst case (`serve/mod.rs:756`, low).

### Freeze cache + kernel zone (Do-NOT-touch, read-only audit)

- **Over-claims.** Freeze key captures interpreter version but no package fingerprint, so a same-interpreter library upgrade is a realistic stale-hit path the docs say is "impossible by construction" (`freeze.rs:11`, medium; doc-scope fix, no knob). Pooled forkserver kernels pre-populate `sys.modules`, so warm vs cold renders can differ despite the "identical" claim (`exec.rs:253`, low, doc fix).
- **Wedge / correctness.** Mid-run kernel death poisons the in-memory warm-prefix `ran`, so a later code-unchanged rebuild never respawns and serves KERNEL_DIED placeholders indefinitely (`exec.rs:610`, medium). `is_uncacheable` false-positives on legitimate outputs containing the sentinel strings, defeating caching for the self-referential docs (`exec.rs:894`, low, PLAUSIBLE). `interp_id` memoizes an empty version string on a transient `--version` failure (`exec.rs:903`, low).
- **Resource hygiene.** `adopt_forked` leaks both the `/tmp` dir and the forked kernel process on a handshake/bind timeout (`kernel.rs`, medium). Forked-kernel liveness/SIGINT/SIGKILL keys on a bare recyclable PID, defeating KernelDied fast-fail under PID reuse (`kernel.rs`, low). (The failed-`Kernel::start` `/tmp` leak, the boot-diagnostic clobber, the stream-ANSI leak, and the `in_flight` slot leak sharpen existing Tier-2 items with exact paths rather than adding new ones.)

### Accessibility (advertised but shallow)

- **Contrast.** Cmd-K palette selected row + match highlights use raw `--tali-accent`, failing AA in every theme (`search.js`, high). Syntax comment token below 4.5:1 in light/sepia/light-deck (`base.css:338`/`:583`, `deck.css:760`, medium/low). Deck `.tali-menu-slide-n` numerals sub-AA (low).
- **ARIA overpromise.** Book chapter drawer advertises `aria-haspopup="dialog"` and dims the page but has no `role=dialog`/`aria-modal`/focus-trap; same for the mobile TOC sheet (`site/chrome.rs`, medium). Deck control menu declares `aria-haspopup="menu"` over a plain button group with no roving focus (`deck.js:1466`, low). Cmd-K combobox splits `role`/`aria-expanded`/`aria-activedescendant` across wrapper and input, leaves the listbox unnamed, and never clears a stale activedescendant on empty results (`search.js:142`, medium).
- **Motion.** Deck auto-animate FLIP and magic-move morph set inline transitions that bypass the `prefers-reduced-motion` CSS gate (`deck.js:389`, medium). JS-initiated smooth scrolls hardcode `behavior:'smooth'` with no reduced-motion guard across search, reading-progress, and client navigations, and these ship in the static build (`search.js:553` and others, medium).
- **Keyboard / AT.** Overflowing `<pre>` and wide tables are horizontal scroll containers but not keyboard-scrollable, a hard WCAG 2.1.1 failure on the most common content type (`base.css`, medium). Lightbox decoration turns `pre.mermaid` into a `role=button` leaf, hiding the diagram's SVG content from AT, and forces decorative `alt=""` images into focusable tab stops (`11-lightbox.js:178`, medium). Non-hex per-slide background colors are assumed dark, flipping heading/body text to invisible white on light named backgrounds (`deck.js:337`, medium). Deck fragment reveals are not announced to screen readers (`deck.js:449`, low). Lightbox + link-preview lack the `.tali-deck` guard, so deck nav double-handles Arrow/Esc over an open zoomed figure (`11-lightbox.js`, low). Theme/Focus segmented controls use `aria-pressed` toggles for a single-select choice with no arrow-key nav (`14-reader-prefs.js:24`, low). The Resume-reading pill auto-dismisses after 8s with no announcement (WCAG 2.2.1) (`15-reading-progress.js:88`, low). Mobile TOC handle chip leaks the SR-only "(read)" suffix (`toc-sheet.js`, low).

### Reader experience, theming, visual craft

- Floated sidenote/margin-note has no `has-toc` guard, so above 73rem it collides with the sticky TOC (`base.css:554`, medium). `--tali-flash` unthemed for sepia, so live-edit pulses render blue on the warm page (`base.css:35`, low). Sepia comment token 3.47:1 (`base.css:583`, low). Two conflicting `.tali-input` rule blocks (`base.css:779`, low, see CUT). TOC entries and `.panel-tabset` labels double-escape `&`/`<`/`>` because `html_escape` is layered on already-safe `strip_tags` output (`render/mod.rs:1608`, `divs.rs:528`, medium).

### Deck engine (Rust + client, beyond the above)

- Deck title slide is injected as a raw string outside `doc.blocks`, so front-matter title/subtitle edits produce an empty diff and never hot-update the preview (`deck.rs:206-225`, medium). Explicit `{#sec-x}` on a slide heading is dropped, so `@sec-` renders a dead link (`deck.rs`, medium). A code block whose only content is `. . .` is swallowed as a pause marker (`deck.rs`, low). A slide titled "Title Slide" collides with the hardcoded `id="title-slide"` (`deck.rs`, low). Two `{{< input >}}` on one line collide on `qin-{line_no}` (`extension/mod.rs:231`, low). Theme-switched `{{< video dark= >}}` downloads both clips (`extension/mod.rs:311`, low, PLAUSIBLE). Overview touch swipe double-fires (pans + advances index, tile highlight desyncs) (`deck.js:1398`, low). Speaker Current/Next preview is blank for `<canvas>`-rendered cells because `cloneNode` skips the bitmap (`deck.js:983`, low). Internal `DocFormat::Reveal`/`is_reveal_doc` naming survives across ~73 sites after the engine was removed (`model.rs:119`, low).

### Site, protocol, watcher, security

- Absolute `image:` URL is mangled into a broken relative path, breaking og:image + listing social cards (`site/discovery.rs:26`, medium). Single-mapping nav/footer items and href-less bare strings are silently dropped (`config/mod.rs:222`, low). `block_tag_has_id` matches id as a substring, so a listing id can bind to `data-block-id` (`links.rs:119`, low). Active-nav highlight lost on a `#fragment`/`?query` href (`links.rs:51`, low). `percent_decode` slice-panics on a raw non-ASCII request path (`serve/mod.rs:420`, low). `app.pages` grows unbounded from bogus ws `?page=` keys, each preallocating a broadcast ring (`serve_site/mod.rs:630`, low). Front-matter theme flip between built-in light and dark is not hot-swapped (`serve/mod.rs:1043`, low).

### Cmd-K search (beyond staleness + combobox)

- `json_str` leaves U+2028/U+2029 unescaped; a paragraph/line separator in prose can break the whole inlined-JS index on pre-ES2019 engines (`search.rs:159`, low). Single-doc DOM index omits h5/h6 that the server index includes (`search.js:80`, low). The site index is re-mapped and re-lowercased on every open despite a "memoized once" comment (`search.js:70`, low). Fuzzy matcher scans every word of every section per keystroke for a 4+ char non-substring term (`search.js:232`, low, situational).

### Architecture / waste

- The two dev servers duplicate the load-bearing diff-then-broadcast contract, a real drift risk (`serve/mod.rs:992`, medium). Site discover renders every page 2-4x with no shared `RenderedDoc`, and `harvest_xref_numbers` lacks the empty-guard its sibling `build_hover_index` has, so a plain blog pays a full discarded render per page (`site/mod.rs:703`, low; trivial quick-win + medium consolidation). `render_internal_impl` (~500 lines) and `compute_outputs` (~260 lines, in the protected zone) are single dense functions producing the load-bearing block model / freeze reuse (`render/mod.rs:177`, `exec.rs:376`, low). Warning->Diagnostic conversion is copy-pasted four times (`serve/mod.rs:1006`, trivial). Cell-error scanning duplicated across build paths (`build.rs:239`, low). ~90 lines of dev-menu CSS embedded as a Rust string literal (`serve/mod.rs:450`, low). `render/mod.rs` also mixes ~180 lines of asset/script plumbing and attribute post-processing with the orchestrator (low).

### Documentation drift (rename)

- **Functionally broken (no runtime alias).** Theming chapter documents `--qmd-*` CSS vars the runtime never reads (`theming.tmd:153`, high). Schema on-ramp references `qmd-*.schema.json` but the tool emits `tali-*.schema.json` (`frontmatter.tmd:261`, high). Protocol book documents `#qmd-root`; the element id is `#tali-root` (`protocol.tmd:59`, medium).
- **Inverted / wrong guidance.** The guide teaches the theme default as "settles to dark, never follows the OS", the exact opposite of the resolver (auto = follow OS, light fallback), and contradicts the Internals book (`theming.tmd:27-35`, high). Front-matter refs say page-level `image-alt` is ignored and cards emit empty alt; code emits it (test-pinned), so the a11y guidance is inverted (`frontmatter.tmd:55`, medium). getting-started/CLI claim a shipped `~/.local/bin/Taliesin` launcher that does not exist, with wrong casing (`getting-started.tmd:16`, medium). Troubleshooting says the companion defaults to `taliesin`; the extension defaults to the dead `qmd-fast` binary (`troubleshooting.tmd:118`, medium). Internals execution chapter documents `qmd-*` output classes and a persistence guard on `class="qmd-error"` the runtime never emits (`execution.tmd`, medium).
- **Stale-but-working (aliases exist).** Guide's `viewof` example and ~10 corpus posts apply a dead `qmd-input` class (`code.tmd:169`, low). Guide teaches `qmd.*` cell API and `qmd-embed`/`qmd-video`/`qmd-fnref`/`qmd-main` classes as canonical (`code.tmd`, low). Internals teach `window.qmdEnhancers`/`QmdDeck` inconsistently (low). `IRkernel::installspec()` is a no-op Taliesin never uses (`getting-started.tmd:35`, low). README has no License section despite an MIT LICENSE (`README.md`, low). THIRD_PARTY.md claims cargo-deny CI wiring is deferred when it is done, references the deleted `code-enhance.js`, and omits `scrolly.js` (`THIRD_PARTY.md:56`, low). Preview Mermaid CDN pin and the vendored build copy can silently diverge with no provenance guard (`render/mod.rs:877`, low).

## CUT / KEEP / ADD

Driven by the philosophy-gate verdicts. Taliesin's north star (HTML-only, minimal-config, single-editing-surface, no per-edit startup cost) is the arbiter.

### CUT

- **`?qmd=embed` deck mode (adopt).** Verified unreachable dead code: speaker previews now use snapshot clones, and `{{< embed >}}` drives embedding through `window.taliDeckEmbedded`, not a URL mode. Drop the ternary branch and refresh the stale comments (`deck.js:1607`). Shrinks the deck's public state surface, no behavior change.

### KEEP (proposed cuts that should not be cut)

- **`data-level` deck attribute (defer, lean keep).** Not dead: it is a semantic heading-level marker and the count anchor for `corpus.rs:214`. Costs nothing at runtime. Keep and document it as a deliberate test/anchor hook rather than remove it and make the corpus count more fragile.
- **Duplicate `.tali-input` CSS blocks (defer, not a clean cut).** The two blocks style two different features sharing one wrapper class (`{{< input >}}` controls vs `//| viewof:` js-cell inputs). A naive merge would change the `{{< input >}}` layout. Decide the fix (unify deliberately, or split the wrapper classes) first; "cleanup with no user-facing change" is inaccurate.

### ADD (philosophy-gated new capability)

Only two clear adopts; the rest are aligned-but-deferred pending a design call.

- **Shareable/deep-linkable `{{< input >}}` state via the URL fragment (ADOPT).** The textbook realization of "wider = richer browser behavior in a live HTML view": reader-local URL/fragment state hydrated from the existing `data-qmd-input` registry, ~50-80 JS lines, no Rust/model change, no config knob, no write-back. Guardrail: must stay pure reader-local URL state (never a persisted server session) and must coexist with the existing deck (`#/h/v`) and block-anchor fragment routing.
- **Reader text-size and line-spacing controls (ADOPT, explicitly sanctioned).** CLAUDE.md names theme, text size, and spacing as first-class, a11y-exempt reader rights, yet only Theme ships. The whole substrate exists (`window.taliReaderMenu.addSection`, the pre-paint pref script, the segmented widget, `--tali-*` type vars). Add `--tali-reader-scale` and a line-spacing segment, persisted reader-local like theme (`14-reader-prefs.js`). This is the sanctioned exception to "better default over a knob".
- **Deferred but aligned (need a scoping/default ruling before building):** cross-revision block diff ("what changed in this document", the single most on-brand capability durable block identity enables); a reader-facing reproducibility manifest surfacing the freeze/warm-kernel provenance (the strategic wedge vs Jupyter/Quarto); a web-native List of Figures/Tables/Theorems for books; opt-in interactive data tables (sort/filter); a document-level "Cite this" export; code-line cross-references (`@lst-3:line`); theme-aware figures (a `dark=` image variant mirroring the shipped video `dark=`); copy-as-TeX on equations; cell-output export (save PNG / download CSV); estimated reading time; reader-local text highlighting. Each reuses shipped substrate, stays HTML-only and read-only, but collides with either minimal-config (opt-in vs better-default) or an open addressing/scope decision, so none is a clean drop-in.

## What held up under attack (negative space)

The audit is most reassuring in what it did **not** find:

- **The single-editing-surface invariant holds.** The full inbound message surface was traced: `click_block` logging, `restart_kernel`, `qmd-goto` navigation, and reverse cursor-sync are the only paths, and every one is read-only to source. No preview gesture writes back to the `.tmd`. The two "leaks" found (the same-page preview card's cloned ids, the un-origin-checked `qmd-cursor` listener) are source-map-hygiene and defense-in-depth, not write-back breaches.
- **Sourcepos emission is total.** The only hole is the rare include-boundary straddle; the corpus totality test's premise otherwise holds across every block type.
- **The heading `{#id}` dedup gap is already fixed** (`render/mod.rs:402-413` routes explicit ids through `dedup_with_suffix` + warns); only the non-heading anchors remain.
- **The offline invariant holds for HTML pages.** `build`/`render` inline every vendored asset and never touch the network; the deck path is the lone regression, and it is mechanical.
- **The exec/kernel Do-NOT-touch zone was respected and is fundamentally correct.** Every kernel-zone finding is resource hygiene or a doc over-claim; execution semantics and the freeze keying are sound. No stale-hit-by-construction bug was found except the honestly-scopeable dependency-upgrade axis.
- **The deck output contract is clean.** No reveal.js vocabulary leaks into the DOM (`.tali-deck`/`.tali-slide`/`window.TaliesinDeck`); the residue is purely internal type naming.
- **Two "open" visual bugs are already fixed** (390px hero overflow via `box-sizing:border-box`; theme/video desync via `data-theme` driving, zero `prefers-color-scheme` left in bundled assets), so the backlog/AUDITS entries for them can be closed.
- **No critical or high-severity security finding.** Every security item is LOW and consistent with the single-trusted-author, loopback threat model.

### Appendix, CLI / build / first-run (cluster synthesis errored; recovered from the verify journal)

These seven were confirmed by the adversarial verifier but missed the final synthesis
(their cluster agent hit the retry cap):

- **Multi-page `_site` build never sweeps stale files** (net-new, medium): `build_site_async`
  only writes/adds, so renamed or deleted pages persist in `_site/` across rebuilds
  (`crates/server/src/build.rs`). A stale-file sweep (or clean-then-write) would fix it.
- **Embed warnings don't count toward `problems`** (medium): the embed warning loop
  (`build.rs:330-336`) calls `log::warn` but never increments `problems`, unlike
  `doc.warnings`/xrefs, so `--strict` and the exit code under-count them.
- **Positional-target build has no parent-dir creation** (low): `build.rs:205` is a bare
  `fs::write` while the `--out` path calls `create_dir_all` (`build.rs:372`), so
  `build doc.tmd out/sub/x.html` fails when `out/sub` is absent.
- **`--jobs` ignored for single-file builds** (low): parsed into `BuildArgs.jobs` but only
  passed to `build_site` (`build.rs:154`); the single-file branch (156+) drops it.
- **No-kernel build warning doesn't name the language** (low): `build_one_page`
  (`build.rs:714`) reduces the diagnostic to a bool and `PageOutcome` carries no language.
- **Single-doc `serve` swallows a file-read error** (low): `render_doc` (`serve/mod.rs:287`)
  drops the read error via `.ok()?`; `serve()` handles `Ok(None)` silently, so an
  unreadable file serves nothing with no console message.
- **`log::info` reuses the green `built` tag** (low): `log.rs:148-150` routes through
  `Style::Built`, the same tag a real file write uses (`log.rs:30`), making console output
  ambiguous.

-----------------------------------------------------------------------------

**Older audit rounds (2026-06-19 through 2026-06-30) are archived in [AUDITS-archive.md](AUDITS-archive.md).**

-----------------------------------------------------------------------------

# Closed backlog sections + landed records (moved here 2026-07-16)

**Not a task list. Nothing here is open.** This is the rot evidence for work that is
finished, moved out of [backlog.md](backlog.md) so that file can obey its own rule ("only
open tasks live here"). It is kept **verbatim rather than deleted** on purpose: three
sections (B, D, G) were re-scoped by later sessions precisely because the "why this is
closed" reasoning lived only in git. If an entry here looks open, it is not — it is a
record of why it is dead. Open work is in [backlog.md](backlog.md).

The lettered sections A-G were dissolved on 2026-07-16 when only E and F still had
anything open; the letters no longer mean anything, and the surviving open items are a
flat priority list in backlog.md.

## A. Blog identity + de-Quarto — CLOSED 2026-07-16

*(Section A is empty: #7 draft-aware preview LANDED 2026-07-16 — preview shows drafts inline
(listing badge + page banner + dev-menu count/list), build/publish exclude them and report
"N drafts not published: …", book chapters are draftable. Spec:
[2026-07-16-draft-aware-preview-design.md](../docs/superpowers/specs/2026-07-16-draft-aware-preview-design.md).
Dropped 2026-07-12: #12 chronological post prev/next — for a 7-post topic-diverse blog the
ordering is meaningless and over-promises; the reading-first listing is the right hub, and
sequential nav already exists via books. A category-driven "related posts" strip could revisit
this, but only after a richer corpus makes "related" meaningful.)*

## B. Publish / build hardening — CLOSED 2026-07-16 (was rot)

*(Section B, publish/build hardening — `publish --public`, strict-by-default + `--no-strict`,
built-site shared asset bundle — was already SHIPPED by the author; the entries were backlog
rot, verified against source + removed 2026-07-16. See [[backlog-entries-rot]].)*

## C. Theme colour-system a11y follow-ups (2026-07-09 audit) — CLOSED 2026-07-16

*(Section C is closed, 2026-07-16. Six items built: the single-key-shortcut reader toggle
(WCAG 2.1.4, gating `f`/`?`/`/`, not just `f`, which the audit under-scoped), settings-popover
focus-on-open, category chips' `aria-pressed` + live count, keyboard-reachable link previews,
the forced-colors nav marker, and settings-panel reflow at 200%. Spec:
[2026-07-16-section-c-a11y-batch-design.md](../docs/superpowers/specs/2026-07-16-section-c-a11y-batch-design.md);
plan: [2026-07-16-section-c-a11y-batch.md](../docs/superpowers/plans/2026-07-16-section-c-a11y-batch.md).
Two items were NOT built: both had rotted, closed by §F's deck theming/a11y step, verified
against source before deletion. "Embedded deck ignores a sepia host" was already fixed at its
own named anchor (`render/deck.rs:164` reads `(t==='sepia' ? 'light' : null)`, the recommended
fix verbatim). "Deck slide-number chip not restyled per-slide" was fixed by removing the
premise: the chip is now one dark-glass surface in both themes (`deck.css:352-361`), so the
`html.tali-deck-dark`-scoped restyle the bug described no longer exists. See
[[backlog-entries-rot]].*

*Known + accepted (not a bug to re-file): at 200% the 4-button Theme seg wraps, and the first
button of the wrapped line keeps its 1px `border-left`, doubling against the container's own
border. Measured, judged cosmetic (same colour, contiguous, invisible unmeasured). The fix
(gap-dividers + `flex:1 0 auto`) was prototyped in-browser and REJECTED: it stretches the
wrapped button to full width, a bigger visual change than the hairline it removes.)*

Owner-calls kept as-is (one-line changes if ever wanted): table cells use the 1.28:1 hairline
(`base.css:436` — border-strong on every cell heavies every table); callout `tip`/`important`
collapse under protanopia (icon + title already carry meaning, hue never the sole cue); deck has no
sepia palette (document decks as light/dark-only, or add + teach the reader/scroll path).

## D. Reading-first identity polish — CLOSED 2026-07-16 (was rot)

*(Section D is closed. It was **backlog rot**, and the "direction ruling" it had been blocked on
turned out to be a question about a fork that does not exist. Direction **"Marginalia"** (iron-gall
manuscript ink) is fully landed: theme/colour 2026-07-09, type ~2026-07-12, layout already shipped.)*

*The **type** pointer was rot first (the old "type → item 13" named a `#13` that exists nowhere;
§A's numbering died when §A closed). The owned Newsreader body face is wired at `base.css:35`,
applied at `:216`, both variable faces bundled at `font-weight: 200 800` and inlined as `data:` URIs.*

*Then the three "re-verified, NOT rot" layout targets were re-checked on 2026-07-16, **in a browser
rather than by reading the file**, and two of the three dissolved:*

1. ***"Hero as typeset, not a marketing slab" was ALREADY SHIPPED.** The entry quoted `base.css`'s
   `.hero { text-align: center }` as proof, but that is the **full-width landing** branch; the
   override ~10 lines below (`.tali-site-main:not(.tali-wide) .hero`) is what a reading-measure page
   gets, and its own comment calls it "an editorial masthead ... e.g. a personal homepage". Measured
   live on the blog index: `text-align: left`, lead `max-width: none`, and the iron-gall eyebrow
   hairline rendering at 40x2px in `rgb(56,65,101)`. This is the "trust the symptom, never the line
   number" trap in its purest form: the quoted line was real, and irrelevant.*
2. ***"Drop bordered feature-card grids" is MARKETING-ONLY.** The blog never authors them. Exact-match
   grep: `.feature-grid`/`.feature` appear in `site/features.tmd` + `site/index.tmd` only, **zero**
   hits in `corpus/tech-blog/`. So reshaping them is deferred marketing work, not blog work.*
3. ***The `--space-1..6` scale is genuinely absent** (verified, grep exit 1), but with 1 and 2 gone it
   is a pure refactor: no visible change, regression risk across `base.css`. Owner ruled **drop it**;
   if spacing ever actually hurts, it returns as a real item.*

***The fork was false twice over*** *(and this is the reusable lesson): (a) `page-layout: full` →
`.tali-wide` **already partitions** the deferred marketing site from the blog, and `base.css` already
exploits that partition for `.hero`, so "cannot be scoped to the blog by CSS alone" was simply wrong;
(b) the blog does not author the contested component at all. **Do not re-open D**: the identity work
is done, and the only thing left in those primitives belongs to the marketing rebuild.*

## E. Catalog-derived work — the SWEEP is closed (2026-07-16); some items stayed open

**Owner ruling 2026-07-16: stop the sweep, triage an area on demand.** Wave 1 triaged the 4
highest-leverage areas (34/165: crossref, citations, slides, config) and measured the base:
**12 of 34 (35%) outright stale or superseded, 20 of 34 (59%) contain at least one false statement
about today's source.** Triaging the remaining 131 against that base is not worth a session, and the
staleness only grows as more ships. Full results, per-entry verdicts, and the caveats:
[2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md). *(The trust caveats and
the surviving open items moved to [backlog.md](backlog.md); this is the closure record.)*

### Landed 2026-07-16 (recorded so they are not re-scoped)

- ***Cmd-K index chapter scoping.** `build_sections` was the last site path rendering unscoped, so a
  book's search index said "Theorem 1" / "Figure 1" while the page said "2.1". D49's tail (scoped
  floats never reached `search.rs`), which the theorem flip widened to theorems. The chapter lookup
  existed twice (`Site::chapter_for` + an inline copy in `scan_xref_targets`) and search needed a
  third, so it is now one `book::chapter_of` all three call. The dev server's off-lock split is
  preserved: it reads the chapter under the same brief lock as the page clone, render stays off-lock.
  Two pre-existing defects it surfaced were filed, not fixed (raw `&nbsp;` in the index text; a
  cross-page `@fig-` in a snippet renders a bare "Figure").*
- ***Theorem/float numbering agreement** (was the `[ruling]` entry). Owner ruled **flip theorems to
  auto-scope**, then ruled **delete `number-within:` with it** — because once theorems scope
  automatically the key does exactly nothing, and a recognized-but-inert key is the very bug D67
  (`csl:`) just shipped a diagnostic for. Theorems now call `float_number`, the same helper floats
  use, so a chapter cannot show "Figure 2.3" beside "Theorem 5"; measured before (Figure 2.1 beside
  Theorem 1) and after (both 2.1) on a real build. **The entry's "breaks 2 pins" was itself stale**:
  the named lines pointed at unrelated tests, and the corpus pins that assert "Theorem 2.1" **passed
  unchanged**, because `methods.tmd` is chapter 2 and now scopes without config. The one real pin was
  `site/mod.rs`'s cross-page theorem test, whose sibling *figure* test already asserted "Figure 2.1"
  for the identical book — its own comment ("a flat Figure 1 would collide with chapter 1's own first
  figure") is the argument for the flip, written before the flip. Removal swept 12 sites incl. the
  drift-locked schema + vocab JSON (regenerated with `TALIESIN_BLESS=1`, diff = the key only) and
  `methods.tmd`'s whole front matter. Migration is loud, not silent: a leftover `number-within:` now
  warns `unknown theorems key` **with a line number**.*
- ***Cross-page `@fig-` to a CELL-labelled figure** (was live defect #2, the largest one). Shipped.
  The entry's cause was right but pointed at the wrong layer: teaching `scan_page_anchors` to parse
  fences would have duplicated the renderer's "which fences are cells" rule in a second parser.
  `Site::harvest_xref_numbers` **already renders every page and already iterates the renderer's own
  registry** (`doc.xref_numbers`), which contains cell labels — it was simply `get_mut` enrich-only,
  so it looked straight at `fig-x` and dropped it. Fix = insert-if-absent there, one source of truth,
  no new parser. This also fixed **backlinks** and **`taliesin map --format json`** for cell figures
  for free (both key off `xref_targets`). Scale was understated: the corpus has **26 cell-labelled
  `fig-` anchors vs 17 brace ids**, so the broken shape was the majority of the test net's figures.
  Pinned in `corpus/demo-book` (results.tmd defines `fig-stages` with a `{mermaid}` cell; summary.tmd
  refs it cross-chapter → "Figure 3.1"); verified in a real browser (click → `results.html#fig-stages`,
  target in viewport, no console errors) and on a non-book website (flat "Figure 1").
  **Two review catches worth remembering:** (a) the insert path had to re-apply `is_ref_anchor` —
  the render registry is LOOSER than the scan (the table-caption path registers *any* id), so
  `: cap {#my-table}` leaked into `map`'s xref_targets as a phantom resolvable target. Measured on
  both sides: `main` → `{}`, first-cut branch → `{"my-table": …}`. (b) A mixed-form duplicate took
  the *loser's* number ("Figure 2" on a link to a page where it reads "Figure 1"); the enrich arm now
  only accepts a number from the page the url points at. `docs/internals/sites.tmd` corrected: the
  xref design is **three** passes (scan → render-harvest → rewrite), not two, and its prefix list was
  missing 5 of the 12 real ones.*
- ***D49 chapter-scoped float numbering.** Shipped: figures/tables/equations/listings scope to the
  chapter in a numbered book ("Figure 2.1"), flat everywhere else. The number is built ONCE by the
  renderer that knows the chapter and carried as a `String` (`render::float_number`), mirroring the
  `section_number`/theorem precedent, so the executor prints it verbatim. **It never needed the
  citation zone** (`register_xref` already took a `String`, since theorems push "2.1" through it).
  Blocked instead on the **exec zone**, for 3 integer literals in exec.rs's own `#[cfg(test)]`
  module; owner approved that narrow edit and nothing else. Verified in a real build: intro
  "Figure 1.1", methods "Figure 2.1", cross-chapter ref → `intro.html#fig-structure` "Figure 1.1",
  standalone post still flat. `demo-book` had **zero** numbered floats, so intro + methods gained one
  labelled figure each (+2 small authored SVGs) to pin it.*
- ***D67 `csl:` recognized-but-unsupported.** Shipped, and it **never needed the citation zone**. It
  was **five** surfaces, not four: `AGENTS.md` also taught the key (both it and the vocab JSON are
  *derived* from `vocab::vocab()`, so one filter fixed both). Proved inert by rendering
  `corpus/bayesian-website` with and without the key: byte-identical (980300 bytes). The `css`
  did-you-mean hazard is now **mechanically pinned** (`csl_stays_recognized_because_dropping_it_would_mis_suggest_css`
  builds `KNOWN_KEYS` without `csl` and asserts the suggestion becomes `css`), so a future cleanup
  cannot re-introduce it. The rule finally lives in `frontmatter::validate_unsupported_keys`, on the
  **render path**, not in `diagnostics/` as this entry originally instructed: `diagnostics/` is
  check-only (it appears once in the whole server crate), so the first cut left the **preview**
  silent, which is the surface the author actually reads. Orphaned `ieee.csl` (17KB) deleted with it.*
- ***D74 footnote reverse-sync.** Shipped, and the symptom was **worse** than this entry said: the
  section hardcodes `data-block-id="qmd-footnotes"`, so `closest()` DID resolve, to a block with no
  sourcepos, leaving `openSource()` on its `line = "1"` default. Every footnote silently jumped to
  **line 1**; it was never a no-op. Fixed per-`<li>` (nested positions, the pattern `:::` divs already
  use); the block-level empty sourcepos is **deliberately kept** (a block-level range would break
  `corpus.rs:151`'s monotonic-source-order assert and make reverse-sync swallow the document). **No
  exemption existed to remove:** the checks skip on `sourcepos.is_empty()` *generically*, which is
  exactly how the hole hid.*
- ***D107 deck fragment effects.** Shipped as `::: {.fragment .fade-out}` / `{.fragment .highlight}`
  (a second class on the existing fenced div, so no new authoring form). **CSS-only** (`deck.css`), no
  Rust/JS: the effects reuse the `.tali-frag-visible` marker deck.js already toggles. Declines held
  (no `incremental:` knob, no `data-fragment-index`).*
- *Also landed 2026-07-16: the **deck key sheet** (it advertised "↑ ↓ Vertical slides" while
  `up()`/`down()` call `moveTopic`; the pin now reads the binding and the sheet together so they
  cannot drift apart again); **`author: [A, B]`** (a YAML sequence read via `.as_str()` gave `None`,
  so both consumers fell through `.or(config.title)` and a multi-author site published its own
  **title** as the author in the Atom feed and JSON-LD; `SiteConfig.authors` now reuses the same
  `frontmatter::string_list` a page's `author:` always used, and the deliberate RFC-4287 title
  fallback is pinned to fire only when there is genuinely no author); and the phantom
  **`number-sections`** doc comment (the key existed nowhere in the source but the comment claiming
  it; numbering is really decided by `chapter_for`). Note the 2026-06-29 theorem spec still reasons
  about "the `number-sections` feature" as though it shipped: it is a dated record, left as written.*

## F. Deck rework (2026-07-12 slides audit) — LANDED except B3-18

**Detail: [2026-07-12-deck-audit.md](2026-07-12-deck-audit.md)** (43 confirmed bugs + a
keep/cut/fix/add feature verdict + a mobile-feed spec + a grind order). Owner-decided shape change
(REMOVE, don't fix the old behavior): a deck opens **as a deck** (desktop = stepped slides;
phone/portrait = a TikTok-style scroll-snap **slide feed**, keyed on aspect not width); **delete
reader/scroll mode**; **delete print/PDF** (the critical dark-deck-blank-PDF bug is resolved by
removal); trim the overview flourishes (minimap/LOD/threads/filter/pen/van-Wijk zoom).

**Progress (2026-07-16): the ENTIRE audit is landed except one deliberately-deferred item.**
Steps 1-7 all done (front door + feed + correctness + flourish trim + theming/a11y/perf + docs
+ the C-ADD share-link/QR, live-input deep-link, feed notes-narration, wake-lock adds). See the
audit file's top-of-doc **Status** block for the per-item tracker. **B3-18 remains open and is
tracked in [backlog.md](backlog.md).**

## G. AI-native authoring (2026-07-12 audit) — CLOSED 2026-07-16 (was rot)

*(Section G is closed as a grind chunk, 2026-07-16. It was **backlog rot**: the whole browser-free
loop (the three items this entry called "the recommended first bets") shipped 2026-07-13, along
with 5 more. Verified against source + all 30 named pins run green before deleting the entries. See
[[backlog-entries-rot]].*

*Shipped (item → anchor → pin): #1 AGENTS.md onramp → `core/agents.rs:42` → `agents_md_cli.rs`;
#2 `taliesin read` → `render/text.rs` + `model.rs:283 body_text()` → `text_projection.rs` +
`read_cli.rs`; #3 agent-grade diagnostics → `server/check.rs:23` (`{code, severity, file, line,
message, suggestion?}`) + `core/diagnostics/codes.rs` → `check_cli.rs`; #4 Claude Code skill →
`editor/claude-code/skills/taliesin/SKILL.md` → `skill_freshness.rs`; #5 `map` → `map_cli.rs`;
#6 `taliesin-mcp` → `server/mcp.rs` → `mcp_stdio.rs`; #7 scaffolds + `paper` kind → `new_cli.rs`;
#10 structured build/publish errors → `structured_build_errors.rs`. Plus **#8(b)** placeholder-alt
(`a11y.rs:337`) and **#9(B)** ScholarlyArticle (`meta.rs:150`, author-free trigger) + **#9(C)**
per-page cited-refs sidecar (`build.rs:355,1560`) → `citations_sidecar.rs`.)*

*The three ruling-gated leftovers (#8a `check --online`, #8c numeric-claim hint, #9A per-page text
sidecar) were ruled **decline** by the owner on 2026-07-16; reasoning is recorded under "Decided
against" in [backlog.md](backlog.md). **Nothing in section G is open.***
