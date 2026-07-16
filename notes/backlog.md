# Taliesin backlog

**Scope: corpus-plus-roadmap.** "Done" = the docs under `corpus/` render correctly (the corpus is
the regression net); each new capability ships pinned by a target corpus doc. Output stays
**HTML-only**. Roadmap: `ROADMAP.md`.

> Kept small (read often). **Only open tasks live here** — delete items once landed; don't leave
> `[x]`. Completed work is in git + `ROADMAP.md` / `native-rewrite.md` / `AUDITS.md`.

## State (2026-07-16)

v0.2.0. All four formats render + deploy; the dev loop is strong (block-level incremental updates
with DOM-state preservation, warm server + Jupyter kernel, `_freeze` cache, Alt-click + reverse
cursor sync, located diagnostics, CSS hot-swap, Cmd-K search). **Tier 1 is empty.**

**The old A-G sections are gone.** Every one closed, so on 2026-07-16 the lettering was dissolved
and the survivors flattened into the priority list below — the letters had stopped carrying
meaning, and four of the seven sections existed only to say "closed". The closure records (incl.
the rot evidence for B, D and G) moved **verbatim** to [AUDITS.md](AUDITS.md); they were kept
rather than deleted because those three were re-scoped by later sessions precisely when the
reasoning lived only in git. **Do not re-open them; do not re-scope from them.**

**Before picking any item: grep its named symbol/flag in source first, and prefer measuring the
running product over reading this file.** The author pushes work mid-session, so an entry can go
stale with no signal here (that is how B, D and G all rotted). Trust an item's described *symptom*,
never its cause or line number. **An entry marked "verified against source" is not enough**: §D's
layout targets carried exactly that label and were quoting a real CSS line that a rule ten lines
below already overrode. A browser measurement dissolved two of them in minutes. **An entry's stated
*cost* rots too**: the theorem ruling's "breaks 2 pins" named two unrelated tests, and the pins it
feared passed untouched. Price a change by making it and reading the failures.

**Working method:** branch per feature; brainstorm if there's a fork; spec under
`docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
extension harnesses); fast-forward merge locally; delete the item here. Agents commit + ff-merge to
local `main` on request; push to `origin/main` only when the author asks. **Do-NOT-touch:** the
exec/kernel zone + the single-editing-surface invariant. Review subagents use read-only git.
**Author policy (feature-first):** finish framework features before marketing-site work.

**Standing constraints on any change** (from the 2026-07-11 website audit, 99 findings; detail:
[2026-07-11-website-design-audit.md](2026-07-11-website-design-audit.md)): the **personal blog**
(`corpus/tech-blog/`) is the forward-facing brand, direction **"Marginalia"** (iron-gall manuscript
ink), and its 14 explicit **KEEPs** (serif/sans pairing, offline bundling, `meta.rs` OG head,
live-figure thumbnails) live in that detail file — protect them. Every fix stays invariant-safe: no
CDN, no preview write-back, no new output format, `--tali-*` tokens only.

## Next session: start here

**Git.** Do not trust a SHA written here; any commit that records one falsifies it — and on
2026-07-16 an agent wrote a SHA into this file that **did not exist at all**. **Check, do not
read:** `git log --oneline origin/main..main` for what is unpushed (the author pushes, not the
agent), and `git reflog show origin/main` before believing ANY "not pushed" claim, including one in
a session handoff. The author pushes mid-session with no signal in this file, and a handoff has now
been wrong about this **five** times: on 2026-07-16 one said "+6 unpushed" (all six were already
pushed), he then pushed twice more mid-session, and later that day a handoff said "the last backlog
commit is local" while it was pushed, before he pushed four more in-flight commits. Re-run the
checks too, do not assume: `cargo test -p taliesin-core` + `-p taliesin-server`, `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`.

**What is left is a flat list; none of it is a grind chunk.** One item is build-ready with no
ruling (**D37**); the rest is small defects, two owner rulings, two sign-off-gated citation items,
and one deliberate deferral. Everything else is Tier 2/3 (demand-driven).

**Method note that paid off three times on 2026-07-16, use it:** when an entry names a file:line,
open the *running product* before the file. §D's layout targets were labelled "re-verified against
source" and still dissolved under a browser measurement, because the quoted CSS line was real but
overridden ten lines below. A true cause can also name the wrong *layer*: the cell-label xref entry
correctly blamed `scan_page_anchors`, but the fix belonged in `harvest_xref_numbers`, which already
had the data — **ask "who already knows this?" before "where does the entry point?"**. Grep traps
are live here too: a bare word matches prose (`feature` matched the English word), `grep | head`
reports **head's** exit code (so `|| echo "absent"` never fires and `$?` lies), and zsh needs
`--include='*.tmd'` quoted. See [[grep-verification-traps]].

**Decided 2026-07-16, do not re-litigate:** §G's three leftovers (`check --online`, the
numeric-claim hint, the per-page text sidecar) are **declined** (see "Decided against"). Catalog
work is **triage on demand**, not a sweep. The **References** click-to-source bug is **logged, not
fixed**, pending a design answer. The **`--space-1..6` scale is dropped** (pure refactor, no
payoff). Theorem numbering was ruled **auto-scope + delete `number-within`** and has landed.

## Open work (flat, priority order)

### 1. Build-ready now (no ruling needed)

- **D37 — lint `format:` sub-keys** (ADOPT; catalog D-number is the detail pointer). The honored
  `format: deck:` key set is empty, so whitelisting `transition` would validate no-ops as supported.
  This adds a **diagnostic, not a knob**, following the from-quarto value-lint precedent (`69c228b`)
  and the `csl:` precedent (a key that reads as honored and does nothing is the bug). Cleanest build
  on the list.

### 2. Live defects (small, independent; count them, don't trust a number written here)

1. **The References section repeats the footnote bug** (found while fixing D74, 2026-07-16).
   `cite/render.rs:102` hardcodes `data-block-id="qmd-references"` with an empty sourcepos, so
   Alt-clicking any reference silently lands on **line 1** (same mechanism as D74). **In the
   Do-NOT-touch citation zone**, and unlike footnotes there is **no clean per-`<li>` fix**: a CSL
   entry's real position is in the `.bib` file, not the `.tmd`. *Owner ruled 2026-07-16: log it, do
   not fix it yet.* **It needs a design answer before code:** where should a reference's
   click-to-source land (the `.bib` entry in another file? the `[@key]` citation site? nowhere)?
   Related, deliberately left: clicking the footnote section's own chrome (the `<hr>`/padding) still
   resolves to line 1; closing that needs `locatable()` to require a *usable* sourcepos, a client change.
2. **Duplicate-label warnings are unlocated** (`render/mod.rs:1538`, `site/xref.rs:56` emit no
   file/line), half-reproducing the exact Quarto flaw D53 critiques. *(The harvest's own duplicate
   warning, added 2026-07-16, is unlocated for the same reason and would be fixed by the same work.)*
3. **`{.python code-line-numbers=...}` is routed to the executable path** though it is authored as
   display-only in `corpus/deck.tmd:46` and two docs pages; `code_lang` splits naively. Invisible to
   the kernel-free corpus. *Unverified against a live kernel.*
4. **The xref registry goes stale on a warm content edit** (`serve_site/mod.rs:1148-1199` refreshes
   only the Cmd-K search fragment).
5. **`lang: fr` promises French, delivers English** cross-ref labels (`render/page.rs:239`).
6. **The Cmd-K index stores raw `&nbsp;` entities in its "plain text"** (found 2026-07-16 while
   fixing the index's chapter scoping; pre-existing and independent of it). The indexed body reads
   `Theorem &nbsp;2.1` / `Figure&nbsp;2.1`, so a reader typing the number they can SEE ("Theorem
   2.1") matches nothing — the text extraction never decodes entities. Scoping the index made the
   numbers agree with the page but cannot make them findable. Same pass: a cross-page `@fig-` inside
   an indexed snippet renders as a bare "Figure" (search renders a page alone, so the site-level
   xref rewrite never runs over it), e.g. methods' "refines the chapter overview from Figure into
   the steps". Both live in `site/search.rs`'s text extraction.
7. **Theorem environments are undocumented in the User Guide** (surfaced 2026-07-16 while deleting
   `number-within`, not caused by it). They shipped 2026-06-29 (8 kinds, `shared:`/`numbered:`,
   hover-preview, collapsible proofs, cross-page refs) and the **corpus exercises them in 6 docs**
   (`corpus/refs/theorems*.tmd`, `demo-book/methods.tmd`, `diagnostics/check-superset.tmd`), but
   `grep -rli theorem --include='*.tmd' docs/` matches **only** `docs/internals/sites.tmd` (and only
   as an xref prefix). So a shipped, corpus-pinned feature is unusable from the docs alone, which is
   the docs' stated purpose. Small: one `using/` page, and the numbering rule to teach is now simply
   "a numbered book chapter scopes it; nothing to configure".
8. **A labelled `include: false` python/R cell registers an anchor that never exists** (found by the
   adversarial review of the cell-label fix, 2026-07-16). `register_xref` runs *before* the lang
   match (`render/mod.rs:~523`), so `#| label: fig-x` + `#| include: false` registers `fig-x` with a
   number, while `exec.rs:379` (`!cell.include → continue`) drops the output block, so no `id="fig-x"`
   is ever emitted. `@fig-x` then renders a confident numbered link to a fragment that exists nowhere.
   **Pre-existing on the same page** (main has the identical dead link for a same-page `@fig-x`); the
   cell-label fix **widened it to cross-page** and, in doing so, silenced the "broken cross-reference"
   warning that used to fire there — the one diagnostic that flagged it. Only affects python/R:
   mermaid/`{js}` emit their figure at render time, so their anchor is real regardless of `include`.
   The fix is lang-dependent (do not register when the figure is known to never materialize, or warn
   that a labelled `include: false` cell is unreferenceable, mirroring the theorem-prefix warning at
   `render/mod.rs:1699`) and belongs in the render/exec seam, so it wants its own change.
   *Source-verified; **unverified against a live kernel** (this sandbox has no `ipykernel`).*

### 3. Needs an owner ruling (not builds)

- **D34 project defaults** (OWNER-RULING). `bibliography`/`csl`/`execute`/`theme` are absent from the
  19-key `NATIVE_KEYS`, but no corpus doc repeats them across pages, so it fails minimal-config today.
  Recommendation: **subtract before adding**, delete the dead `image:`/`SiteConfig.card_image` field
  (zero readers; its own doc comment concedes it) and defer the defaults until a corpus doc hurts.
- **D70 "Cite this" card** (OWNER-RULING). Its machine-readable half already shipped
  (`.citations.json` + ScholarlyArticle JSON-LD). A card would render **author-free for every current
  post** (0 of 8 tech-blog posts set `author:`).

### 4. Needs Do-NOT-touch sign-off (citation zone)

- **D72/D69 citations** (ADOPT, but **both edit `crates/core/src/cite/`, and need explicit
  sign-off**). D72: support bare `@key` at all? (The *diagnostic* shipped 2026-07-16, `8a45d59`, so
  the failure is now caught; the engine question is separate.) D69: the reference list is `push`ed at
  the end, so an appendix after `# References` orphans the heading. *(Two entries that named this
  zone — D49 and D67 — turned out not to need it at all. Check before assuming a third does.)*

### 5. Deliberately deferred

- **B3-18** (the last deck-audit item; detail:
  [2026-07-12-deck-audit.md](2026-07-12-deck-audit.md)): a structural deck edit re-mounts the *whole*
  deck, nuking every `{js}`/WebGL widget's state; re-mount only the edited `<section>` subtree.
  Deferred on purpose (touches the client's re-mount path; bigger blast radius). Everything else in
  that audit has landed — see [AUDITS.md](AUDITS.md).

### Consulting the Quarto catalog (policy, not a task)

**Owner ruling 2026-07-16: no sweep. Triage an area on demand, when you next work that area.** Wave
1 measured the base: **12 of 34 (35%) outright stale or superseded, 20 of 34 (59%) carry at least
one false statement about today's source**.
**Before consulting it, read the triage doc's "three layers" section** —
[2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md).
In short: the entries are the asset and were well-grounded on 2026-07-03; the **heading status is
degenerate** (162/165 skeptic verdicts are `revise`, so 94 read "Proposed (revised)" regardless of
conclusion); and the **executive summary is misleading** (it describes a per-entry tag scheme that
does not exist, miscounts, and its "rule on these first" list mixes open questions with
already-shipped work). A skeptic verdict is evidence, never a ruling: D135's skeptic insisted on
dropping Atom feeds as "a documented non-goal" and Atom shipped anyway, with autodiscovery.

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

**AI-native leftovers, owner ruled decline 2026-07-16 (don't re-open).** The other 8.5 of §G's 10
items shipped 2026-07-13; these three were the ruling-gated remainder, declined on the evidence:
- **`check --online` citation resolution** (§G#8a) → **declined.** It is the only proposed network
  egress in the tool: the workspace carries no HTTP client dep and `CHECK_FLAGS` is `["--format"]`
  only, so this buys a link-rot check at the cost of the offline invariant. Revive only if a real
  workflow demands it, and then check-only, off by default, never reachable from `build`/`publish`
  (`build.rs` shares `page_static_diagnostics`, so a network call there would make builds phone home).
- **Numeric/quoted-claim-without-citation hint** (§G#8c) → **declined.** Its own spec rates it
  FP-prone and recommends default-off. A linter that cries wolf gets switched off, taking the good
  rules with it.
- **Per-page text/JSON sidecar** (§G#9A) → **declined as redundant.** It was specified to reuse §G#2's
  projection; #2 shipped as `taliesin read`, and site-level `llms.txt`/`llms-full.txt` already ship.
  Revive only when a named consumer asks for a per-page file.

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
