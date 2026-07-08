# Taliesin backlog

**Scope: corpus-plus-roadmap.** "Done" = the docs under `corpus/` render correctly (the
corpus is the regression net); each new capability ships pinned by a target corpus doc.
Output stays **HTML-only**. Roadmap: `ROADMAP.md`.

> Kept small (read often). **Only open tasks live here** — delete items once landed; don't
> leave `[x]`. Completed work is in git + `ROADMAP.md` / `native-rewrite.md` / `AUDITS.md`.

## State (2026-07-07)

Local `main` runs ahead of `origin` between sessions (the author syncs `main`↔`origin`), v0.2.0. All four formats render + deploy;
the dev loop is strong (block-level incremental updates with DOM-state preservation, warm server +
Jupyter kernel, `_freeze` cache, Alt-click + reverse cursor sync, located diagnostics, CSS hot-swap,
Cmd-K search). The author pushes/syncs between sessions; agents commit + fast-forward-merge to local
main on request, never push.

**Recently shipped** (detail in git + `ROADMAP.md` / `native-rewrite.md` / `AUDITS.md`): the native
rewrite + roadmap Waves 0-4, the reader cluster, `check`/prose-lint + `{input}`/scrolly, the `--bare`
build, the reading-first redesign, deep-audit P1/P2, the Taliesin rename, the `.tmd` editor grammar, the
legacy-format clean break (`.tmd`-only input, `deck`/`define()` the only spellings), the security-P3
batch, the VS Code companion language features, F2a cross-page hover-preview, nested-theorem numbering,
the 2026-07-07 audit batches (1-4, Batch 5 in full [high-value half + remainder], 6, 7, the Batch 8
robustness trio: watcher prune + live search index + reconnect state, and Batch 9: the freeze/kernel
honesty + resource-hygiene trio), cross-reference backlinks (a quiet per-target "Referenced by"
line — the reverse of forward xref; `site/backlinks.rs`), and the Batch 8 consolidation (the
duplicated diff-then-broadcast tail hoisted into the shared `protocol::Broadcast` helper, so the
single-doc and site dev servers can't drift on the block-level incremental invariant).

**Working method:** branch per feature; brainstorm if there's a fork; spec under
`docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
extension harnesses); fast-forward merge locally; delete the item here. **Do-NOT-touch:** the
exec/kernel zone + the single-editing-surface invariant. Review subagents use read-only git.

**Author policy (feature-first):** finish framework features before marketing-site work.

## Needs your input (the blockers)

Empty — the three prior blockers were ruled on 2026-07-07 (see Priority queue below).

## Priority queue

### Tier 1 — decided, build-ready (no blocker)
- Empty — the 2026-07-07 audit implementation queue is fully landed (Batches 1-9 + the Batch 8
  consolidation). Next build-ready work = promote a "Decided 2026-07-07" dedicated-session item, or
  pull a Tier-2 hardening item forward.

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
  teardown; (b) a failed `Kernel::start` leaks its `/tmp/tali-kernel-<uuid>` dir (error paths drop the
  `PathBuf` without cleanup); (c) warm-pool `in_flight` counter can leak if a refill task panics (no
  reachable panic site today; an RAII guard would harden). Reclaimed on reboot but unbounded under
  repeated failures. Also: a boot-failure diagnostic can overwrite a cache-hit cell's output
  (`exec.rs:491`, already flagged `error`; optional freeze-restore); R stream/stderr still leaks raw
  ANSI into HTML (`kernel.rs:887-893` `Output::Stream` emits `esc(text)` with no `strip_ansi`, do-not-touch).
- **Testing / CI:** insta snapshots on `body_html()` for reactive/explorable/bayesian docs through the
  exec path (`corpus.rs:99` is structural-only); `#[serial]` the kernel-load determinism tests + assert
  a dropped output is a hard named error (the known silent-drop flake); `deny.toml` multiple-versions
  policy. **`tsc`/`@ts-check` — web-client tier DONE:** `search.js` + `toc-spy.js` + `toc-sheet.js`
  (the last already carried `@ts-check` but was never in the `include`) are now `@ts-check`'d, fixed
  (159 errors → 0), and wired into `web-client/jsconfig.json` + the CI `typecheck` job alongside the
  already-gated `client.js`. **Remaining: `assets/js/*` — its own (large) pass** (~800+ errors:
  `deck.js` alone ~400, plus `qmd-js.js`/`scrolly.js`/`tabset.js`/`walkthrough.js`/`mermaid.js` + the
  16 `code-enhance/` fragments; exclude the vendored `*.min.js`). Needs its own ambient globals + a
  config that compiles the concatenated `code-enhance/` fragments as one shared script scope.
- **Security:** injected Mermaid `<script>` SRI + `crossorigin` — deferred (only the live Preview
  lazy-loads mermaid from the CDN; a static build inlines the vendored copy). Needs a hash pinned to the
  CDN build, and both `integrity` + `crossorigin` would break a non-CORS `TALIESIN_MERMAID_URL` override.
- **Deck engine (P2, deferred):** drop `fitSlide` from the resize path (needs a lazy fit-on-show
  refactor first); mobile pinch/pan + touch gestures (hard to verify without a device); thread
  `footer:`/`logo:` through both deck-page builders (no corpus deck needs one yet).
- **Perf (low):** protocol-level op-message batching (one WS message per save, not one-per-op); lazy
  discover-time search index (`search::build_sections`, eagerly built at `site/mod.rs:304`); visited
  pages never evicted from `app.pages` (`serve_site/mod.rs:42`, unbounded growth).
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
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic).
- **`taliesin publish`: build, push HTML to a deploy branch, host auto-deploys.**
  Researched 2026-07-07 for the private research-paper-draft workflow. Command shape:
  `taliesin build <project> --out <tmp>`, then push the built `_site/` tree to a deploy branch
  (e.g. `published` / `gh-pages`) via a git worktree (or `git subtree split`); the host watches that
  branch and serves the committed HTML with no build step of its own. One-way (source, build, deploy
  branch), never writes back to source (respects the single-editing-surface invariant); the
  per-project target could live in `_site.yml`. **Brainstorm the forks first:** worktree vs
  `git subtree`; one repo-per-paper vs one branch-per-paper; whether it also scripts the initial host
  hookup. The documented manual recipe pushes `_site/` today; this only automates it.
  **Hosting picked** (password + private-repo + deploy-from-branch, since drafts must not be public):
  **Cloudflare Pages, recommended** — free, private repo, deploys from a branch, unlimited bandwidth;
  password two ways: Cloudflare Access (email allowlist, one-time-PIN/SSO, free ≤50 users, viewers need
  no account) or a Pages-Functions HTTP basic-auth env var. Rejected/second-choice: AWS Amplify (OK if
  already on AWS); Netlify (password gated behind Pro $19/mo); Vercel (avoid — free auth forces viewers
  onto your team, shared-password is a $150/mo add-on); Railway (app/container platform, a mismatch for
  static HTML); GitHub Pages (no native password without Enterprise).
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

The 2026-07-07 deep audit's build-ready fixes (decided, no blocker) were worked in
**batches sized as one branch each**. Batches 1-9 all landed, including the Batch 8 consolidation
(the duplicated diff-then-broadcast tail hoisted into the shared `protocol::Broadcast` helper so the
two dev servers can't drift on the incremental invariant). Full per-item detail (repro + fix approach)
and the ~80 low-severity long tail live in [AUDITS.md](AUDITS.md) 2026-07-07. **CONFIRMED unless
marked PLAUSIBLE.** Already-tracked items (op-batching, kernel `/tmp` [`Kernel::start`] + `in_flight`
leaks, boot-diagnostic clobber, `app.pages` LRU, lazy search index, `fitSlide`, R ANSI leak, Mermaid
SRI) stay in **Tier 2** above; the audit only sharpened their exact paths in AUDITS.md.

### Cut (philosophy gate: adopt) — cleared
The `?qmd=embed` dead-mode removal landed (drop the orphaned ternary branch + stale comments;
speaker previews became snapshot clones long ago, so nothing generated the URL). The gate KEPT two
proposed cuts as **do-not-cut:** `data-level` is a live test anchor; the two `.tali-input` CSS blocks
style two different features.

### Low-severity long tail (~80 items) → [AUDITS.md](AUDITS.md) 2026-07-07
Pick up opportunistically alongside whichever batch touches the same file. Includes: include symlink-loop
SIGABRT + lexical-only `safe_join` (`includes.rs`); diff-LIS unique-id `debug_assert!`; dead
`ts`/`typescript`/`toml` highlight aliases; an unresolved fence language degrading to plain text
silently (`highlight.rs:51` — deferred from Batch 5: needs a warnings channel threaded into the pure
`highlight` fn, invasive); `percent_decode` slice-panic on a non-ASCII path;
`click_block` terminal-escape injection; qmd-js initial pass paints in DOM order not topo order; many
citation-render edge cases; and the architecture / waste / stale-but-working-docs tail — including the
**stale-but-working `qmd-*` docs references that still have runtime aliases** (`qmd.*` cell API,
`qmd-input`/`qmd-embed`/`qmd-video`/`qmd-fnref`/`qmd-main` classes, `window.qmdEnhancers`/`QmdDeck`);
renaming those is a separate verify-each-alias pass, not a mechanical sweep.
**Do NOT "rename" these live identifiers — they are correct as-is:** `qmd-goto`/`qmd-cursor` (postMessage),
`qmd_token` (cookie), `qmd-theme` (localStorage key + `<style id>`), `qmd:themechange` (event),
`qmdFast.*` (VS Code config), `qhl-*` (highlight scope).

### Owner-gated: do NOT build without your ruling
- **Add (gate: adopt, but confirm):** shareable/deep-linkable `{{< input >}}` state via the URL fragment
  (reader-local, hydrate from `data-qmd-input`, no Rust/model change) (`qmd-js.js`); reader text-size +
  line-spacing controls (a11y-exempt per CLAUDE.md; substrate exists) (`14-reader-prefs.js`).
- **Add (deferred, need a scope/default ruling):** cross-revision block-diff "what changed" view;
  reader-facing reproducibility manifest; web-native List of Figures/Tables/Theorems; interactive data
  tables; "Cite this" export; code-line xrefs (`@lst-3:line`); theme-aware `dark=` figures.
- **Verify + close:** the 390px hero overflow + theme/video desync (listed open under Tier-3 Marketing) are
  confirmed present in code (`site.css` `.tali-site-main { box-sizing:border-box }` guards the overflow;
  `theme.rs` `syncThemeVideos()` on `qmd:themechange` swaps the clip). Just needs the 3-viewport browser
  check, then close.

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
