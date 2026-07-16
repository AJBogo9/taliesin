# Taliesin backlog

**Scope: corpus-plus-roadmap.** "Done" = the docs under `corpus/` render correctly (the corpus is
the regression net); each new capability ships pinned by a target corpus doc. Output stays
**HTML-only**. Roadmap: `ROADMAP.md`.

> Kept small (read often). **Only open tasks live here** — delete items once landed; don't leave
> `[x]`. Completed work is in git + `ROADMAP.md` / `native-rewrite.md` / `AUDITS.md`.

## State (2026-07-16)

v0.2.0. All four formats render + deploy; the dev loop is strong (block-level incremental updates
with DOM-state preservation, warm server + Jupyter kernel, `_freeze` cache, Alt-click + reverse
cursor sync, located diagnostics, CSS hot-swap, Cmd-K search). `origin/main == local main ==
a4a96bc` (draft-aware preview merged + pushed; nothing unpushed). **Tier 1 is empty.**

**Sections A, B and F are now closed.** A (blog identity) finished with #7 draft-aware preview
(2026-07-16). B (publish hardening) was **backlog rot** — all three items were already shipped by
the author; entries deleted with evidence (see the note in section A). F (the deck audit) is fully
landed except the deliberately-deferred B3-18. **→ The next open work is section C (theme/a11y
follow-ups), then D, then E.**

**Before picking any item: grep its named symbol/flag in source first.** The author pushes work
mid-session, so an entry can go stale with no signal in this file (that is exactly how section B
rotted). Trust an item's described *symptom*, never its cause or line number.

**Working method:** branch per feature; brainstorm if there's a fork; spec under
`docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
extension harnesses); fast-forward merge locally; delete the item here. Agents commit + ff-merge to
local `main` on request; push to `origin/main` only when the author asks. **Do-NOT-touch:** the
exec/kernel zone + the single-editing-surface invariant. Review subagents use read-only git.
**Author policy (feature-first):** finish framework features before marketing-site work.

## Now — the grind queue (priority order)

The 2026-07-11 website audit (99 findings; detail:
[2026-07-11-website-design-audit.md](2026-07-11-website-design-audit.md)) makes the **personal blog**
(`corpus/tech-blog/`) the priority — it's the forward-facing brand. Direction **"Marginalia"**
(iron-gall manuscript ink). 14 explicit **KEEPs** (serif/sans pairing, offline bundling, `meta.rs` OG
head, live-figure thumbnails) live in the detail file — protect them. Every fix stays invariant-safe
(no CDN, no preview write-back, no new output format, `--tali-*` tokens only).

### A. Blog identity + de-Quarto (build-ready; quick wins first)

*(Section A is empty: #7 draft-aware preview LANDED 2026-07-16 — preview shows drafts inline
(listing badge + page banner + dev-menu count/list), build/publish exclude them and report
"N drafts not published: …", book chapters are draftable. Spec:
[2026-07-16-draft-aware-preview-design.md](../docs/superpowers/specs/2026-07-16-draft-aware-preview-design.md).
Dropped 2026-07-12: #12 chronological post prev/next — for a 7-post topic-diverse blog the
ordering is meaningless and over-promises; the reading-first listing is the right hub, and
sequential nav already exists via books. A category-driven "related posts" strip could revisit
this, but only after a richer corpus makes "related" meaningful.)*

*(Section B, publish/build hardening — `publish --public`, strict-by-default + `--no-strict`,
built-site shared asset bundle — was already SHIPPED by the author; the entries were backlog
rot, verified against source + removed 2026-07-16. See [[backlog-entries-rot]].)*

### C. Theme colour-system a11y follow-ups (2026-07-09 audit; verified, unbuilt)

Real a11y bugs (WCAG/APCA/OKLCH/CVD harness evidence), each survived adversarial verification:

- **Bare `f` forces fullscreen with no opt-out** (`03-focus-mode.js:80`, med). Keep
  `requestFullscreen` on an explicit menu action; add a reader toggle to disable single-key shortcuts
  (WCAG 2.1.4).
- **Settings popover never takes focus on open** (`13-reader-menu.js:60`, med). Focus its first
  control on open (Esc already restores focus to the launcher — the asymmetry is the bug).
- **Category-filter chips expose state only visually** (`10-category-filter.js:27`, med). Mirror the
  active class with `aria-pressed`, render it on the server's initial "All" chip, announce "Showing 4
  of 12 posts" via a visually-hidden `aria-live=polite` node.
- **Embedded deck ignores a sepia host** (`render/deck.rs:164`, med). `hostTheme()` accepts only
  light/dark; map `sepia → light` so an `{{< embed deck.tmd >}}` matches the host lightness.
- **Citation/xref link preview is hover-only** (`12-link-preview.js:159-163`, low). Only
  `mouseover`/`mouseout`; no `focusin`. Bind `focusin`/`focusout` too, set `aria-describedby` while
  open. (The recent heading-link a11y merge is a separate change; this one is still open.)
- **`forced-color-adjust: none` hides the current nav item** (`site.css:293` + `base.css:780`, low).
  Pins fg with no bg; under an opposite-polarity High-Contrast OS theme the "you are here" marker
  vanishes. Only the reader-seg pressed button (pins a bg+fg pair) needs the opt-out.
- **Deck slide-number chip not restyled per-slide** (`deck.css:455`, low). Dark restyle scoped to
  whole-deck `html.tali-deck-dark`; on a `.tali-dark-bg` slide the chip reads ~2.8-3.0:1.
- **Settings panel doesn't reflow at 200% text.** Content-loss half is fixed; at 200% the seg buttons
  + shortcut list still h-scroll. Needs a real reflow (stack the rows), not a token change.

Owner-calls kept as-is (one-line changes if ever wanted): table cells use the 1.28:1 hairline
(`base.css:436` — border-strong on every cell heavies every table); callout `tip`/`important`
collapse under protanopia (icon + title already carry meaning, hue never the sole cue); deck has no
sepia palette (document decks as light/dark-only, or add + teach the reader/scroll path).

### D. Reading-first identity polish (design; layout half)

The theme/colour half landed 2026-07-09; type → item 13. Remaining is **layout**: hero-as-typeset
(not a marketing slab), drop bordered feature-card grids, a `--space-1..6` scale. Confirm direction
before building (overlaps the deferred marketing rebuild).

### E. Quarto design-decisions catalog triage (own session)

Branch `quarto-decisions-catalog` @ `535b4e1`: 165 adversarially-verified decisions. Rule each by "is
this the right design for Taliesin" (the 2026-07-07 repositioning retired Quarto as the reference).
Fan into batches, each with a recommended verdict + evidence.

### F. Deck rework (2026-07-12 slides audit → [2026-07-12-deck-audit.md](2026-07-12-deck-audit.md))

**Start in [2026-07-12-deck-audit.md](2026-07-12-deck-audit.md)** — the wide slide-deck audit: 43
confirmed bugs + a keep/cut/fix/add feature verdict + a mobile-feed spec + a grind order. Owner-decided
shape change this session (REMOVE, don't fix the old behavior): a deck opens **as a deck** (desktop =
stepped slides; phone/portrait = a new TikTok-style scroll-snap **slide feed**, keyed on aspect not
width); **delete reader/scroll mode**; **delete print/PDF** (the critical dark-deck-blank-PDF bug is
resolved by removal); trim the overview flourishes (minimap/LOD/threads/filter/pen/van-Wijk zoom). The
file's grind order: (1) pin kept features in `corpus/deck.tmd` first (net); (2) flip the front door +
delete reader/PDF (kills whole bug families); (3) crashes/correctness (`. . .`-before-plain-code wedges
nav; readHash anchor/digit misroute → slide 0; live `---`/`. . .` not structural; "Title Slide" id
collision); (4) build the mobile feed; (5) trim flourishes; (6) theming/a11y/perf; (7) share-link +
live-input deep-link + wake-lock adds.

**Progress (2026-07-16): the ENTIRE audit is landed except one deliberately-deferred item.**
Steps 1-7 all done (front door + feed + correctness + flourish trim + theming/a11y/perf + docs
+ the C-ADD share-link/QR, live-input deep-link, feed notes-narration, wake-lock adds). See the
audit file's top-of-doc **Status** block for the per-item tracker. **Only remaining: B3-18** — a
structural deck edit re-mounts the *whole* deck, nuking every `{js}`/WebGL widget's state;
re-mount only the edited `<section>` subtree. Deferred on purpose (touches the client's re-mount
path; bigger blast radius). Nothing else in section F is open.

### G. AI-native authoring (2026-07-12 audit → [2026-07-12-ai-native-backlog.md](2026-07-12-ai-native-backlog.md))

Make a developer authoring with an LLM (Claude Code / Codex) the first-class customer without demoting
the manual writer. Primitives already exist (`check --format json`, `vocab`/`schema`/`symbols`, the block
model); the gaps are the **protocol**, the **closed loop**, and the diagnostic **grain**. Ideas + framing:
[FEATURE-IDEAS.md](FEATURE-IDEAS.md) Session 2. **Every code anchor in the detail file was opened +
adversarially verified** (5 confirmed / 5 anchor-corrected, 0 unbuildable). Priority vs A–F is owner's
call; the three below are the recommended first bets (they compose the whole browser-free loop). Start in
the detail file — each carries verified anchors, the pin, the first step, and the rulings still needed.

1. **Generated `AGENTS.md` onramp** (S–M) *[ruling: `new` too?/default-on?/pin-home]*. `core/agents.rs::agents_md()` (dialect section generated from `vocab()` so it can't drift), golden-locked, written by `cmd_init`. No new subcommand — rides `cli.rs:58` scaffold. Pin: `agents_md_matches_committed` + `agents_md_cli.rs`.
2. **`taliesin read <page>` — text projection of the built page** (M) *[ruling: `read` vs `render --format text`; parse-only?]*. New block-model emitter (`render/text.rs` + `RenderedDoc::body_text()` beside `model.rs:253`) projecting resolved xref numbers / alt / callouts / TeX to plain text, so an agent reads what it rendered with no browser. A VIEW, not an output format. Pin: `tests/text_projection.rs` snapshot over `corpus/reader/hovercards.tmd`.
3. **Agent-grade diagnostics** (M) *[ruling: code scheme/severity]*. Promote every `check --format json` diagnostic to `{code, severity, file, line, column?, message, suggestion}` (structured "did you mean"); `--format human` stays byte-identical. Additive to `Warning` (`model.rs:146`) + `Diagnostic` (`check.rs:19`). Pin: `tests/check_cli.rs` over `corpus/diagnostics/typos.tmd`. (Grounding rated this Tier-2/foundational; kept in the trio because it completes the loop.)

## Tier 2 — hardening (P3)

- **Execution-cache leaks — remainder** (exec/kernel Do-NOT-touch, careful):
  - **Ungraceful-death path (S/M):** no defense vs SIGKILL / closed terminal / crash. Absent:
    `PR_SET_PDEATHSIG` on the warm-pool helper (it has its own process group, so cheap), and a
    startup sweep of stale `/tmp/tali-warmpool-*` / `/tmp/tali-kernel-*` dirs whose owner pid is dead.
    (Measured: `kill -9` on a preview orphaned 8 procs / 451 MB + 123 `/tmp/tali-*` dirs.)
  - **Flaky timing tests** (LOAD-sensitive):
    `exec::tests::pooled_kernel_serves_cells_without_a_long_warming_state` +
    `kernel::tests::kernel_executes_state_errors_and_interrupts_runaway_cell` fail under CPU load;
    both assert on **timing**. Fix: wait on a **state signal**, not a duration.
  - `build.rs:926` warms the pool before knowing any page needs a kernel, even under
    `TALIESIN_NO_EXEC=1`. Hygiene, not perf (0.25 s vs 0.27 s on a prose-only site).
  - `fork_kernel` cross-call edge (low): a timed-out-but-queued fork mis-pairs the next `SPAWNED
    <pid>`; poison the daemon on any fork timeout so later `take`s cold-start.
  - R stream/stderr leaks raw ANSI into HTML (`kernel.rs` `Output::Stream` emits `esc(text)` with no
    `strip_ansi`, do-not-touch).
- **Interpreter selection is silent + has no project-local override (DX; S+M).** Resolved once at
  `exec.rs:217` (`TALIESIN_PYTHON` else `python3`; `TALIESIN_R` else `R`). Two gaps bit a real user
  (2026-07-11: a global `TALIESIN_PYTHON` in `~/.zshrc` errored a whole book's ~35 cells):
  - **No "which python?" signal (S, highest-leverage).** A dep-less interpreter is indistinguishable
    from a code error. Log `executing cells with <abs path>` at build start, and/or a `taliesin
    check` reporting interpreter + `ipykernel` presence (like `quarto check`). Lives in the
    build/serve entry, not the Do-NOT-touch core.
  - **No project-local declaration (M).** Add a `python:` / `r:` field in `_site.yml` (parsed in
    `schema.rs`/`frontmatter.rs`, threaded into `Executor::build`), and/or auto-detect a sibling
    `.venv/bin/python` when the env var is unset. Env var stays the override; the field wins for
    reproducibility. (Downstream `invertible-speech-disentanglement` BUG-002.)
- **`assets/js/*` `tsc`/`@ts-check` pass** (own large session). The web-client tier is done + in CI;
  remaining is `crates/core/assets/js` (measured 812 errors on a throwaway strict jsconfig; `deck.js`
  402). Needs ambient globals + a config compiling the concatenated `code-enhance/` fragments as one
  shared scope (isolated compile adds 12 spurious `TS2304`s).
- **Mermaid `<script>` SRI + `crossorigin`** — deferred (only live Preview lazy-loads from the CDN; a
  build inlines the vendored copy). Needs a hash pinned to the CDN build; `integrity`+`crossorigin`
  would break a non-CORS `TALIESIN_MERMAID_URL` override.
- **Deck engine (P2):** drop `fitSlide` from the resize path (needs a lazy fit-on-show refactor
  first); mobile pinch/pan + touch gestures (hard to verify without a device); thread
  `footer:`/`logo:` through both deck-page builders (no corpus deck needs one yet).
- **Perf (low):** protocol-level op-message batching (one WS message per save, not per-op). Worst
  case: an edit near the top of a long doc where every downstream block emits a `SetMeta` (`diff.rs`
  `anchor_op`). Client + server ship together, no wire-compat constraint.
- **Audit long-tail** (`AUDITS.md`): a tens-of-MB cell output blocks ZMQ receive before the cap fires
  (`kernel.rs`, Do-NOT-touch).

- **AI-native authoring — packaging + guardrails** (detail: [2026-07-12-ai-native-backlog.md](2026-07-12-ai-native-backlog.md); anchors verified). Tier-2 slice of the §G initiative:
  - **`taliesin map --format json`** (M) — one-call project outline (pages/nav/drafts/xref-graph/mounts) for agent planning; mirror `cmd_symbols` (`query.rs:232`), reuse `Site::discover`. Pin: `tests/map_cli.rs` over `corpus/demo-book`.
  - **Correct-by-construction scaffolds + `--json` on `new`/`init`** (S–M) — a citation-wired `paper` kind (`bibliography:` + `[@key]` + shipped `references.bib`) and machine-readable create output; seam `cli.rs:178` `new_files()`. Pin: byte-pin `corpus/scaffold/posts/my-paper/` via `cli.rs:658`.
  - **Sharpen `check` as the LLM-mistake catcher** (L, sliced) — default-on placeholder-alt nudge (do first; `a11y.rs:284` + `helpers::tag_attr`), opt-in numeric-claim-without-citation hint (`prose.rs:55`), opt-in `check --online` DOI check (the sole sanctioned egress; needs a small additive read-only accessor on `cite::Bibliography`). Pin: `corpus/diagnostics/llm-mistakes.tmd`.
  - **`build`/`publish` structured errors (`--format json`)** (M) — retain the already-computed `page_static_diagnostics` as structured `Diagnostic`s (reuse `check.rs` shape) instead of logging+dropping; coupled edit across `build.rs` + `publish.rs` (`run_site_build:868`). Pin: `tests/structured_build_errors.rs`.
  - **Taliesin Claude Code skill/plugin** (S–M, soft dep §G#1) — a distributable `taliesin` skill (loop + dialect crib + source-not-preview rule) driving the CLI, pinned against the live binary (`tests/skill_freshness.rs`) so it can't rot like the retired external scaffolder.

## Tier 3 — deferred / demand-driven

- **Companion (Phase 2):** editor commands (`.tmd`-buffer text transforms only, never preview
  gestures); `editor.wordWrap` default for `[taliesin]`; grammar polish (YAML-type `#|`/`//|`/`%%|`
  values; recommend cell-language extensions via `.vscode/extensions.json`); **marketplace packaging
  hygiene** (`.vscodeignore` misses `.vscode-test/` (1.8 GB), `test-fixtures/`, `scripts/`,
  `out/test/`, `out/e2e/`; no top-level `icon`/`repository`/`license`/`keywords`; `"private": true`
  blocks publish). `symbolCache` only invalidates on save (`completions.ts`, low) — an out-of-band
  change lags until the next save; bounded + graceful, noted so it isn't re-discovered.
- **`.tmd` format-on-save** (open question): a source pretty-printer must preserve `data-sourcepos`
  line stability for click-to-source — brainstorm reflow-vs-risk first.
- **Dogfood: migrate the FL-weather book to Taliesin** — a real Quarto→Taliesin migration +
  portability stress test; pin a reduced version under `corpus/` if it renders clean.
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic).
- **`taliesin publish` follow-ups:** optional `--init` wrapper for the one-time `wrangler` setup;
  email-allowlist (Cloudflare Access) mode.
- **Interactive/explorable numerics** (`FEATURE-IDEAS.md` #62-66; none pinned — promote with a corpus
  pin when one graduates; must NOT reintroduce a reactive VM). Highest-leverage: **#62** a bundled
  numerics/stats global for `{js}` + **#63** `animate`/play-tick + draggable-`point` `{{< input >}}`.
- **Wave 5** (`ROADMAP.md`): print-pdf track (paged render *of* the built HTML), docs-as-spec,
  `{glsl}` cell language, SEO completeness. **Fold `llms.txt`/`llms-full.txt`** in (the block model
  separates clean prose from code/math at `client.js:50`, so it'd be more accurate than the old
  scraper). *Pin: a `tech_blog.rs` assertion that `llms.txt` lists discovered pages + `llms-full.txt`
  excludes drafts.*
- **Site-level shared bibliography + hygiene** (M). `bibliography:` is per-document only
  (`cite/mod.rs:42`). Allow it in `_site.yml`, merged under each page's; add two read-only diagnostics
  ("entry never cited", "duplicate key") over the parsed registry (does NOT touch the BibTeX/CSL
  Do-NOT-touch core). *Pin: a small site, one entry cited from two pages, one uncited.*
- **Author structure panel** (M/L). A read-only preview sidebar: the heading tree with per-section
  word count (`client.js:50-58` already counts) + a badge per node for unresolved xref / TODO /
  over-goal length. Click to scroll; move the editor cursor via cursor sync under the companion. An
  annotation layer on the dev panel, not a new component. *Pin: `corpus/layout/structure.tmd`.*
- **Session revision digest** (M). Surface the `BlockOp` stream the client already receives: a
  session word delta + a feed of the last N ops, each click-to-source. (Also the home for the cut
  "cross-revision what-changed" idea if it's ever revived.) Behavioral pin (a `tools/live-edit-bench`
  assertion), not a corpus doc.
- **Block-level transclusion** `{{< include file.tmd#sec-id >}}` (M). Reuse a section across a series.
  Must ride **on top of** the `includes.rs` source-map pass (resolve fragment → block range, hand a
  sub-slice), never rewrite it. Hard gate: the source map must not perturb. Defer until a series needs
  it.
- **LSP for the language intelligence** (L). Everything an LSP needs is already in Rust (`check`,
  `vocab`, `register_xref`, bib parser, `closest()`); write-once for Neovim/Helix/Zed/VS Code, removes
  the `#| label:` completion drift. The preview stays the view (editor-agnostic; two `postMessage`
  shapes in `docs/internals/protocol.tmd:325-350`). Do NOT rebuild the preview as an LSP.
- **Image optimization** (WebP/AVIF + `srcset` + lazy-load behind a content-hashed cache) — until
  posts get image-heavy.
- **Marketing site** (deferred, feature-first; rolls into a demo-machine rebuild):
  `live-edit-hero-demo` clip; swap `site/_site.yml` placeholders; demo-led hero rebuild (with a
  3-viewport spot-check of the already-fixed 390px hero overflow + theme/video desync, plus any
  leftover em dashes); **#12 demo video needs a pause affordance (WCAG 2.2.2) + reduced-motion
  respect** and its baked-in desktop text downscales ~3x on mobile (re-record or ship a mobile
  source); mobile embed refine; deploy.
- **`serde_yaml` fallback watch-item:** if 0.9 breaks against a future serde/edition, swap to
  `serde_yaml_ng` (v0.10), gated on a test that `Error::location().line()` still works. Fix the stale
  `Cargo.toml` comment (names the unsound `serde_yml`) when touched.

- **AI-native authoring — demand-driven** (detail: [2026-07-12-ai-native-backlog.md](2026-07-12-ai-native-backlog.md)). Tier-3 slice of the §G initiative:
  - **`taliesin-mcp` MCP server** (M–L, soft deps §G#2 + Tier-2 `map`) — a local offline stdio JSON-RPC server wrapping check/symbols/vocab/map/read/build as MCP tools. **Read/validate/build ONLY** — no write/edit tool (the single-editing-surface pin: `tools/list` must expose none). Recommended seam: a `taliesin mcp` subcommand over `crates/server/src/mcp.rs` reusing the existing collect fns (hand-rolled JSON-RPC, zero new deps). Pin: `tests/mcp_stdio.rs`.
  - **Published-artifact AI-legibility** (M–L, dep §G#2) — per-page text/JSON sidecars (reuse the text projection; `page_prose` fallback), schema.org `ScholarlyArticle` JSON-LD (upgrade `meta.rs:133`), per-page cited-refs BibTeX/CSL export (surface `cite::process`'s `order` vec). Overlaps the parked "Cite this" export. **BLOCKING ruling:** the ScholarlyArticle trigger — no corpus post sets `author:`, so "dated+authored" fires on none; pick an author-free trigger before writing the `tech_blog.rs` pin.

## Decided against / do-not-re-litigate

**2026-07-12 rulings (don't re-open):** the feature-idea wishlist (cross-revision diff, repro
manifest, List-of-Figures/Tables/Theorems, interactive tables, "Cite this", line-level code xrefs,
image `dark=`) → **cut to `FEATURE-IDEAS.md`** (revive only when a corpus doc needs one). Reader
text-size/line-spacing controls → **declined for now** (a11y-exempt substrate exists in
`14-reader-prefs.js`; revisit if requested). Twinned `fourier-transform` post dirs git-tracking
anomaly → **left as-is**. Stale `new-post`/`new-project` scaffolder skills → **retired** (done this
session; the `deploy` skill stays).

**TODO / FIXME surfacing — owner ruled skip (2026-07-10).** No `level` concept exists
(`render::Warning` / `check::Diagnostic` / `protocol::Diagnostic` know only warning|error, and the
warning channel is a hard gate), so a TODO warning would fail `check` on every draft. If ever
revived: design A (preview-only `Diagnostic::info` at `serve/mod.rs::compute_diagnostics`, cannot
reach the gate) beats design B (re-plumb a real `level` through the whole gate). The scan must NOT
reuse `prose::strip_inline` (blanks code, where TODOs live); pin any fixture in `corpus/diagnostics/`.

**Refuted by measurement — do NOT re-scope:** `build` does not leak forkserver subtrees (graceful
path reaped 2026-07-08; the gap is the *ungraceful* path, Tier 2); the warm pool booting Python on
prose-only builds is hygiene, not latency (0.25 vs 0.27 s); dev attributes are 0.29% of page bytes
(don't strip); a `--version -dirty` marker computed in `build.rs` is stale-by-construction (refused);
the `assets/css` stale-embed claim did not reproduce (re-verify for `assets/js` before the
touch-render workaround); the 390px `hero:` overflow + theme/video desync are already fixed in code;
include symlink-loop SIGABRT does not exist (Linux caps at `MAXSYMLINKS=40`; includes are
author-local).

**Gate the gate:** a drift test that cannot fail is worse than none. Two of three Batch-F drift gates
couldn't fail on first draft. Any new drift gate must be mutation-checked against exactly the shape it
guards.

**Library outsourcing — decided against** (each verified vs the invariants): hayagriva/biblatex,
schemars, jsonschema, morphdom/idiomorph, similar/dissimilar, clap, owo-colors, slug, html-escape,
lightningcss/palette, IntersectionObserver/scrollspy libs, deck micro-helpers. Keep `two_face` extras
filling gaps only (the bundled syntect set is consulted first and must win — `extra_newlines()` is
bat's own curated set, different scope spans, NOT a superset).

**Reading-first defaults — research-validated keeps** (do NOT "fix"): serif body for long-form screen
reading; ~70ch measure `--tali-maxw: 46rem`; right-rail scrollspy + width-gated sidenotes; scroll
(not pagination) book reading; if a serif webfont is bundled, ship REAL bold/italic faces (see item
13), never synthesized. *Caveat:* the competitor framing (Stripe/Linear/Mintlify/…) is unverified
judgment.

**2026-07-06 decisions:** book pager stays bottom-only; book page-TOC fix-in-place, keep both nav
surfaces; xref graph tool removed; focus mode stays ephemeral; deck overview keeps per-slide
backgrounds; dev-menu + `#tali-progress` + reading-progress bar stay three separate signals
(`#tali-progress` is the exec chip, not a reading chip).

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Open-source the repo + publish the site when
ready; the security token gate is shipped.
