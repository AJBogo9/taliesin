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

Each clears once you decide. Design calls carry a recommended default.

- **Quarto design-decisions catalog (the big one).** Branch `quarto-decisions-catalog`, commit
  `535b4e1`: 165 decisions adversarially verified, awaiting your ruling on each ("beat every Quarto
  design decision"). A dedicated triage session, not a quick call. (When ready: fan the 165 into
  batches, each with a recommended verdict + Quarto-vs-Taliesin evidence, so you rule, not derive.)
- **Reading-first identity polish + theme design-quality pass** (design judgment; its OWN session;
  overlaps deferred marketing — confirm direction before building). Hero-as-typeset not a marketing
  slab; drop bordered feature-card grids; quieter near-monochrome accent; `--space-1..6` scale;
  light/dark/sepia cohesion (WCAG-AA already tuned — RE-verify, don't redo; preserve sepia's deliberate
  low-contrast). The "templated" diagnosis is UNVERIFIED — start by pulling competitors up live +
  screenshotting Taliesin at the 3 viewports, bring a before/after to approve before building.
- **Cross-reference backlinks list — build the cheap tier, or skip?** *Recommended: build the
  xref-anchor tier.* No reverse index exists today, but the cheap tier (fig/sec/tbl/eq/lst/thm anchors)
  piggybacks the render-free scan already run at discovery (`scan_outgoing`/`collect_xref_refs`): retain
  the anchor instead of discarding it at `graph.rs:136`, aggregate anchor→referring-pages, surface a
  per-target "Referenced by" affordance. ~a few dozen lines, works in preview + build. Citations are the
  expensive tier (needs a site-wide bibliography-merge decision first) — leave out. This is the
  lightweight replacement for the discovery value the (now-removed) xref graph tool provided.

## Priority queue

### Tier 1 — decided, build-ready (no blocker)
- *(empty — the last item, nested-theorem numbering, shipped 2026-07-07.)* Next build-ready work
  comes from promoting a **Needs-your-input** blocker or pulling a Tier-2 hardening item forward.

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
- **CLI / docs microcopy:** reconcile repo-URL placeholders; README `check` mentions; reconcile
  the no-kernel-build wording (`CLAUDE.md:122`, `getting-started`, `build.rs:232`).
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
