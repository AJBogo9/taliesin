# Taliesin backlog

**Scope: corpus-plus-roadmap.** "Done" = the docs under `corpus/` render correctly (the
corpus is the regression net); each new capability ships pinned by a target corpus doc.
Output stays **HTML-only**. Roadmap: `ROADMAP.md`.

> Kept small (read often). **Only open tasks live here** — delete items once landed; don't
> leave `[x]`. Completed work is in git + `ROADMAP.md` / `native-rewrite.md` / `AUDITS.md`.

## State (2026-07-07)

Local `main` carries this session's reader/deck polish batch (the author syncs `main`↔`origin`
between sessions, so origin may be a commit behind local at any moment), v0.2.0. All four formats render + deploy;
the dev loop is strong (block-level incremental updates with DOM-state preservation, warm server +
Jupyter kernel, `_freeze` cache, Alt-click + reverse cursor sync, located diagnostics, CSS hot-swap,
Cmd-K search). The author pushes/syncs between sessions; agents commit + fast-forward-merge to local
main on request, never push.

**Recently shipped** (detail in git + the history docs): the native rewrite, the roadmap's
Waves 0-4, the reader cluster, `check`/prose-lint + `{input}`/scrolly, the `--bare` build, the
reading-first redesign, deep-audit P1+P2, the Tier-1 priority queue, the **Taliesin rename** +
**`.tmd` editor grammar** (F5-accepted), the **legacy-format clean break** (`.tmd`-only input,
`deck`/`define()` the only spellings, no migration on-ramps, no user-facing legacy branding), the
**security-P3 batch**, the **VS Code companion language features** (check-findings diagnostics +
drift-proof completions; preview/cursor-sync F5-accepted 2026-07-06), and the **reader/deck polish
batch** (2026-07-06, browser-verified): book column no longer jumps between chapters + the focus-mode
pager re-centres; the SSR-vs-first-render race is healed with a render-generation marker
(`TALIESIN_SSR_GEN`) so a client that server-rendered pre-exec re-mounts its cell outputs; deck speaker
previews are scaled snapshot clones (no live embed iframes / re-execution); the "Resume reading" pill
clears the dev menu in preview. **F2a cross-page hover-preview** (2026-07-06): hovering a cross-page
`.tali-xref` now previews its target from a served `hover-index.js` (anchor→rendered-block-HTML index
built in `Site::discover` via `site/hover.rs`, asset URLs rebased root-relative + resolved client-side
via `TALIESIN_SITE_ROOT`); `file://`-safe `<script>` load like search; same-page path untouched.
**Nested-theorem numbering** (2026-07-07): `number_theorems` now scans each block's full HTML for every
theorem div (via a `theorem_divs` helper), not just its opening tag, so a `::: {.theorem}` nested inside
another fenced div (e.g. a `.column-margin` aside) is numbered in document order and resolves as a ref
target; pinned by `corpus/refs/theorems.tmd` + a unit test, browser-verified.

**Working method:** branch per feature; brainstorm if there's a fork; spec under
`docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
extension harnesses); fast-forward merge locally; delete the item here. **Do-NOT-touch:** the
exec/kernel zone + the single-editing-surface invariant. Review subagents use read-only git.

**Author policy (feature-first):** finish framework features before marketing-site work.

## Needs your input (the blockers)

Empty — the three prior blockers were ruled on 2026-07-07 (see Priority queue below).

## Priority queue

### Tier 1 — decided, build-ready (no blocker)
- **Cross-reference backlinks (xref-anchor tier).** Decided 2026-07-07: build it. No reverse index
  exists today; the cheap tier (fig/sec/tbl/eq/lst/thm anchors) piggybacks the render-free scan already
  run at discovery (`scan_outgoing`/`collect_xref_refs`): retain the anchor instead of discarding it at
  `graph.rs:136`, aggregate anchor→referring-pages, surface a per-target "Referenced by" affordance.
  ~a few dozen lines, works in preview + build. Citations stay out (the expensive tier — needs a
  site-wide bibliography-merge decision first). Lightweight replacement for the discovery value the
  (now-removed) xref graph tool provided.
- Audit 2026-07-07 implementation queue also lives here — see
  **[the batched queue below](#audit-2026-07-07-implementation-queue-build-ready)** (Batch 5 remainder
  + Batch 8 next; Batches 1-4, the high-value half of Batch 5, Batch 6, and Batch 7 landed 2026-07-07).

### Decided 2026-07-07 — each needs its own dedicated session
- **Quarto design-decisions catalog triage, reframed.** Branch `quarto-decisions-catalog`, commit
  `535b4e1`: 165 decisions, adversarially verified. Rule on each by "is this the right design for
  Taliesin", not "does it beat Quarto" — the same-day repositioning commit (`de3de37`) retired Quarto
  as the defining reference, so drop that framing even though the fact-checked Quarto evidence is
  still useful input. Fan the 165 into batches, each with a recommended verdict + evidence, so you
  rule, not derive.
- **Reading-first identity polish + theme design-quality pass** (design judgment; overlaps deferred
  marketing — confirm direction before building). Start with the competitor scan + before/after
  screenshots (3 viewports) — the "templated" diagnosis is still UNVERIFIED — before any rework. Then:
  hero-as-typeset not a marketing slab; drop bordered feature-card grids; quieter near-monochrome
  accent; `--space-1..6` scale; light/dark/sepia cohesion (WCAG-AA already tuned — RE-verify, don't
  redo; preserve sepia's deliberate low-contrast).

### Tier 2 — hardening (P3)
- **Execution-cache leaks** (exec/kernel Do-NOT-touch, careful): (a) ~30 orphaned
  `multiprocessing.forkserver` daemons (~100 MB each) survive a completed `build` — kill the daemon on
  teardown; (b) a failed `Kernel::start` leaks its `/tmp/qmd-kernel-<uuid>` dir (error paths drop the
  `PathBuf` without cleanup); (c) warm-pool `in_flight` counter can leak if a refill task panics (no
  reachable panic site today; an RAII guard would harden). Reclaimed on reboot but unbounded under
  repeated failures. Also: a boot-failure diagnostic can overwrite a cache-hit cell's output
  (`exec.rs:491`, already flagged `error`; optional freeze-restore); R stream/stderr still leaks raw
  ANSI into HTML (`kernel.rs:672`, do-not-touch).
- **Testing / CI:** insta snapshots on `body_html()` for reactive/explorable/bayesian docs through the
  exec path (`corpus.rs:99` is structural-only); `#[serial]` the kernel-load determinism tests + assert
  a dropped output is a hard named error (the known silent-drop flake); `deny.toml` multiple-versions
  policy; extend `tsc`/`@ts-check` to `search.js`/`toc-spy.js`/`assets/js/*` (surfaces a pre-existing
  error backlog — its own pass; client.js is already gated).
- **Security:** injected Mermaid `<script>` SRI + `crossorigin` — deferred (only the live Preview
  lazy-loads mermaid from the CDN; a static build inlines the vendored copy). Needs a hash pinned to the
  CDN build, and both `integrity` + `crossorigin` would break a non-CORS `TALIESIN_MERMAID_URL` override.
- **Deck engine (P2, deferred):** drop `fitSlide` from the resize path (needs a lazy fit-on-show
  refactor first); mobile pinch/pan + touch gestures (hard to verify without a device); thread
  `footer:`/`logo:` through both deck-page builders (no corpus deck needs one yet).
- **Perf (low):** protocol-level op-message batching (one WS message per save, not one-per-op); lazy
  discover-time search index (`search.rs:30`); `updateWordCount` deep-clones `#tali-root` per op
  (`client.js`); visited pages never evicted from `app.pages` (`serve_site.rs`, unbounded growth).
- **CLI / docs microcopy:** reconcile the no-kernel-build wording across `CLAUDE.md`,
  `getting-started`, and `build.rs` (each is substantively correct — a no-kernel build/preview
  falls back to source non-fatally — but phrased differently; optional polish, no defect).
- **Audit long-tail** (`AUDITS.md`): a combined content+theme edit drops the hot-swap until reload
  (`serve.rs`); the initial synchronous render isn't panic-guarded; mounted sub-sites don't route
  embedded decks (mount miss → bare 404); a tens-of-MB cell output blocks ZMQ receive before the cap
  fires (`kernel.rs`).

### Tier 3 — deferred / demand-driven
- **Companion:** manifest rebrand (`Taliesin-companion` → Taliesin identity + `qmdFast.*` ids); Phase 2
  editor commands (`.tmd`-buffer text transforms only, never preview gestures); `editor.wordWrap`
  default for `[taliesin]` (respect the global setting until prose overflow is a real complaint, then
  ship `"on"`); grammar polish (YAML-type the `#|`/`//|`/`%%|` option value; recommend the cell-language
  extensions via `.vscode/extensions.json`).
- **`.tmd` format-on-save** (open question): a source pretty-printer writing the editor buffer must
  preserve `data-sourcepos` line stability for click-to-source — brainstorm reflow-vs-risk before work.
- **Dogfood: migrate the FL-weather book to Taliesin** — a real-world Quarto→Taliesin migration +
  portability stress test; pin a reduced version under `corpus/` if it renders clean.
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic). **Thin
  `taliesin publish`** (push `_site/` to `gh-pages`; the documented manual recipe covers it today).
- **Interactive/explorable numerics** (`FEATURE-IDEAS.md` #62-66; none spec'd/pinned — promote with a
  corpus pin when one graduates; must NOT reintroduce a reactive VM). Highest-leverage: **#62** a
  bundled numerics/stats global for `{js}` (distributions, seeded PRNG, small dense linalg) + **#63**
  `animate`/play-tick + draggable-`point` `{{< input >}}` types. Then #64 `qmd.state` cross-re-run store,
  #65 richer `{js}` output helpers (KaTeX-typeset returns + mini table), #66 opt-in Pyodide `{python}`
  (~10 MB, no torch).
- **Wave 5** (`ROADMAP.md`): print-pdf track (paged render *of* the built HTML), docs-as-spec,
  `{glsl}` cell language, SEO completeness (sitemap/robots/JSON-LD at publish with `url:`).
- **Image optimization** (WebP/AVIF + `srcset` + lazy-load behind a content-hashed cache) — until posts
  get image-heavy.
- **Marketing site** (deferred, feature-first; rolls into a demo-machine rebuild): `live-edit-hero-demo`
  clip; swap `site/_site.yml` placeholders; demo-led hero rebuild folding in the open visual bugs (390px
  prose overflow on `page-layout: full` + `hero:`, theme/video desync, leftover em dashes); mobile embed
  refine; deploy (Cloudflare / GitHub Pages).
- **`serde_yaml` fallback watch-item:** if 0.9 ever breaks against a future serde/edition, swap to
  `serde_yaml_ng` (v0.10), gated on a test that `Error::location().line()` still works. Fix the stale
  `Cargo.toml` comment (it names the unsound `serde_yml`) when touched.

## Audit 2026-07-07 implementation queue (build-ready)

The 2026-07-07 deep audit's build-ready fixes (decided, no blocker), grouped into
**batches sized as one branch each and listed in recommended order** (Batch 1 first).
Full per-item detail (repro + fix approach) and the ~80 low-severity long tail live in
[AUDITS.md](AUDITS.md) 2026-07-07. **CONFIRMED unless marked PLAUSIBLE.**

> **How to work this:** one batch = one branch; brainstorm only if a fork appears; TDD;
> verify (cargo + browser via chrome-devtools); fast-forward-merge to local main; then
> delete the landed batch from here. **Do-NOT-touch:** exec/kernel execution semantics +
> the single-editing-surface invariant. Batch 9 enters the kernel zone: read the zone
> rules first. Already-tracked items (op-batching, kernel `/tmp` + `in_flight` leaks,
> boot-diagnostic clobber, `app.pages` LRU, lazy search index, `fitSlide`, R ANSI leak,
> Mermaid SRI) stay in **Tier 2** above; the audit only sharpened their exact paths in AUDITS.md.

### Batch 5 remainder: Silent-failure diagnostics channel [medium]
The high-value half landed 2026-07-07 (unterminated `:::` fence now warns located; quoted figure
`width=`/`height=` smart-punctuation **fixed** — curly quotes stripped in `unquote_value`, was live at
`bayesian-website/subsections/_data-modeling.tmd`; `draft: yes`/`on` now coerced fail-safe + warned;
single-doc `build` now runs `yaml_error()` so malformed front-matter fails `--strict`). Still open (lower
value / more invasive, each its own small change):
- `toc: yes` / `execute: {echo: no}` YAML-1.1 booleans in the CELL-option + render-frontmatter paths
  (a different code path from the site `draft:` one already fixed; `cell_extract.rs` uses `v != "false"`).
- `_site.yml` nested nav/footer/mount typos degrade silently + top-level warnings ship unlocated (`config/mod.rs:206`).
- Block-sequence `bibliography:` dropped ONLY when serde_yaml fails to parse the rest of the front matter
  (the `extract_field` fallback can't read a block sequence) — edge case (`fm_extract.rs:114`).
- Low-tail siblings: non-`.bib` bibliography, unresolved fence language (needs a warnings channel threaded
  into the pure `highlight` fn — invasive), non-HTML `format:`.

### Batch 8: Dev-server / watcher / incremental robustness [mixed; one large] 
- File watcher recursively watches the whole tree (incl. `node_modules`/`.git`); inotify exhaustion
  silently kills hot reload (`serve/mod.rs:877`).
- Site/book Cmd-K index freezes after a content edit in preview (single-doc search stays live) (`serve_site/mod.rs:1035`).
- ws reconnect wholesale-remounts + destroys live block state on a byte-identical doc (any sleep/wifi blip) (`client.js`).
- Two dev servers duplicate the diff-then-broadcast contract (drift risk to the incremental invariant):
  hoist into one shared helper (`serve/mod.rs:992`). **[large: schedule as its own branch.]**

### Batch 9: Freeze / kernel honesty + resource hygiene (Do-NOT-touch zone: careful) [small]
Read the exec/kernel zone rules first; these are diagnostics/docs/leak fixes, not execution-semantics changes.
- Mid-run kernel death poisons the warm-prefix `ran`, wedging the preview into replaying KERNEL_DIED
  placeholders (`exec.rs:610`).
- Freeze key has no package fingerprint; scope the "stale hit impossible / nothing to clear" doc wording,
  a same-interpreter library upgrade is a real stale-hit path, no knob (`freeze.rs:11`).
- `adopt_forked` leaks the `/tmp` dir + forked kernel on a handshake/bind timeout (`kernel.rs`).

### Cut (philosophy gate: adopt) [trivial]
- `?qmd=embed` deck mode is dead unreachable code: drop the ternary branch + stale comments (`deck.js:1607`).
  *(Gate KEPT two proposed cuts: `data-level` is a live test anchor; the two `.tali-input` CSS blocks style
  two different features, decide before merging.)*

### Low-severity long tail (~80 items) → [AUDITS.md](AUDITS.md) 2026-07-07
Pick up opportunistically alongside whichever batch touches the same file. Includes: include symlink-loop
SIGABRT + lexical-only `safe_join` (`includes.rs`); diff-LIS unique-id `debug_assert!`; dead
`ts`/`typescript`/`toml` highlight aliases; `percent_decode` slice-panic on a non-ASCII path; `app.pages`
unbounded growth; `click_block` terminal-escape injection; qmd-js initial pass paints in DOM order not topo
order; many citation-render edge cases; and the architecture / waste / stale-but-working-docs tail. The
last now specifically includes the **stale-but-working `qmd-*` docs references that still have runtime
aliases** (the `qmd.*` cell API, `qmd-input`/`qmd-embed`/`qmd-video`/`qmd-fnref`/`qmd-main` classes,
`window.qmdEnhancers`/`QmdDeck`): Batch 2 swept only the *functionally-broken, no-alias* drift (CSS vars,
schema filenames, `#tali-root`, output classes, theme-default, launcher, image-alt, companion default);
renaming the aliased references is a separate verify-each-alias pass, not a mechanical sweep. The live
identifiers `qmd-goto`/`qmd-cursor` (postMessage), `qmd_token` (cookie), `qmd-theme` (localStorage key +
`<style id>`), `qmd:themechange` (event), `qmdFast.*` (VS Code config), `qhl-*` (highlight scope) are
**correct as-is** — do not "rename" them.

### Owner-gated: do NOT build without your ruling
- **Add (gate: adopt, but confirm):** shareable/deep-linkable `{{< input >}}` state via the URL fragment
  (reader-local, hydrate from `data-qmd-input`, no Rust/model change) (`qmd-js.js`); reader text-size +
  line-spacing controls (a11y-exempt per CLAUDE.md; substrate exists) (`14-reader-prefs.js`).
- **Add (deferred, need a scope/default ruling):** cross-revision block-diff "what changed" view;
  reader-facing reproducibility manifest; web-native List of Figures/Tables/Theorems; interactive data
  tables; "Cite this" export; code-line xrefs (`@lst-3:line`); theme-aware `dark=` figures.
- **Verify + close:** the 390px hero overflow + theme/video desync (listed open under Tier-3 Marketing) are
  reported already fixed (`box-sizing:border-box`; `data-theme`-driven). Re-verify at the 3 viewports, then close.

## Decided against / do-not-re-litigate

**This session (2026-07-06):** book pager stays **bottom-only** (a top pager fights the calm column;
the Chapters drawer already gives random access). Book page-TOC: **fix in place, keep both nav
surfaces** — do NOT fold the rail into the chapter drawer (loses the always-visible scrollspy; the
"rarely used" claim is unverified). Xref graph tool: **removed** (interaction not good enough).
Focus mode stays **ephemeral** (no persistence across chapters): `requestFullscreen()` needs a user
gesture, so persistence could only restore CSS chrome-hiding and would silently drop fullscreen on nav —
a half-broken mode. Deck overview **keeps per-slide backgrounds** (documented recognizability
"fingerprint", no contrast bug today; hiding is a taste-only change — revisit only if a real deck's
overview clashes). Dev-menu + `#tali-progress` + reading-progress bar stay **three separate signals**
(orthogonal: author diagnostics / build-exec status / reader scroll-position; different corners) — and
`#tali-progress` is the exec chip, NOT a reading-progress chip (the ask's label was a misnomer); the
only real issue was the resume-pill/dev-menu overlap, now a Tier-1 fix.

**Reading-first defaults — research-validated keeps** (do NOT "fix"): serif body for long-form screen
reading (don't switch to sans); ~70ch measure `--tali-maxw: 46rem` (don't narrow); right-rail scrollspy
+ width-gated sidenotes (keep both); scroll (not pagination) book reading; system-font-only (if a serif
webfont is ever bundled, ship REAL bold/italic faces, never synthesized). *Caveat:* the competitor
framing (Stripe/Linear/Mintlify/Docusaurus/GitBook, "Bootstrap/Quarto looks dated") is unverified
judgment, not evidence.

**Library outsourcing — decided against** (each adversarially verified vs the invariants):
hayagriva/biblatex (heavy deps, only IEEE used); schemars (reopens schema↔validator drift); jsonschema
(loses source-line diagnostics); morphdom/idiomorph (reverse the 83x live-edit payload win);
similar/dissimilar (give up the block-id→LIS reduction); clap; owo-colors; slug (transliterates
non-ASCII → breaks anchors); html-escape (breaks the anti-double-escape contract); lightningcss/palette
(CSS uses native `color-mix`); IntersectionObserver/scrollspy libs; deck micro-helpers (force an offline
bundle onto every deck). The reader menu is intentionally an untrapped popover. `contents: .` has no
corpus PAGE yet (add a fixture if pinning is wanted).

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Open-source the repo + publish the site when
ready; the GitHub/install CTAs become real then. The security token gate is shipped.
