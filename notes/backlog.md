# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git + [AUDITS.md](AUDITS.md) +
> [ROADMAP.md](ROADMAP.md); delete an item when it lands, don't leave a `[x]`. The "already shipped"
> list near the bottom is the compact anti-rot guard (do not re-add / re-scope), not a changelog.

## State (2026-07-21)

v0.2.0. All four formats render + deploy; the dev loop is strong (block-level incremental updates with
DOM-state preservation, warm server + Jupyter kernel, `_freeze` cache, Alt-click + reverse cursor sync,
located diagnostics, CSS hot-swap, Cmd-K search). **Most of the backlog has already shipped** (DX, PMF,
polish, machine-facing, corpus-coverage and reduction audits are all closed). What is actually open is
small; it is ranked below by product impact.

## Standing constraints (read before working)

- **Do-NOT-touch (one freeze):** `MAX_WARM_PAGES` + the deterministic LRU eviction in
  `serve_site/exec_pool.rs` (M6a, sign-off refused 2026-07-17) and the **single-editing-surface**
  invariant (the preview is read-only; it must never write back to source). The rest of the
  exec/kernel zone is not frozen (its audit finished, M2-M5 sign-offs granted + spent).
- **Website / brand** (2026-07-11 audit, detail:
  [2026-07-11-website-design-audit.md](2026-07-11-website-design-audit.md)): the personal blog
  (`corpus/tech-blog/`) is the forward-facing brand, direction **"Marginalia"**; its 14 explicit KEEPs
  live in that file. Every change stays invariant-safe: no CDN, no preview write-back, no new output
  format, offline bundling, `--tali-*` tokens only.
- **Author policy:** feature-first (finish framework features before marketing-site work).
- **Working method:** branch per feature; brainstorm if there's a fork; spec under
  `docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
  extension harnesses); fast-forward merge locally; delete the item here. Push to `origin/main` only
  when the author asks. **Review subagents get a git worktree or you commit first** (a "read-only"
  reviewer with `Bash` still writes scratch files to your CWD; one ran `cat > Cargo.toml` in the repo
  root and destroyed the workspace manifest).
- **Tests: three gates, or the suite silently under-tests itself** (CI sets all three):
  `TALIESIN_REQUIRE_NODE=1` (JS-equivalence guard), `TALIESIN_R=R TALIESIN_REQUIRE_R=1` (R kernel),
  `TALIESIN_PYTHON=… TALIESIN_REQUIRE_KERNEL=1` (pool-booted `--jobs` path; a missing interpreter is a
  hard fail, not a skip). `cargo test` aborts the remaining binaries at the first failure, so re-run
  before trusting a total. **If an `exec` probe test fails, `--test-threads=1` before blaming your
  change** (there are two flake families: two load-sensitive *timing* tests, and two `exec::tests`
  *concurrency-race* tests, both under P3 below).
- **Git:** do not trust a SHA written in notes. Check `git log --oneline origin/main..main` for what is
  unpushed and `git reflog show origin/main` before believing any "not pushed" claim; the author pushes
  mid-session with no signal here.
- **How this file lies to you:** entries rot (the author pushes mid-session; a scoped prune leaves the
  rest looking freshly reviewed). Before picking an item, **grep its named symbol/flag in source** and
  prefer measuring the running product over reading this file. Trust an item's *symptom*, never its
  cause, line number, or stated cost (all three have rotted). Verify a fix by **mutation** (restore the
  bug, watch the named test fail), not by a green suite. Grep traps live here: a bare word matches
  prose, `grep | head` reports head's exit code, quote `--include='*.tmd'` in zsh.

## Open work (priority order: product impact)

Ranked highest user/product value first. Impact is not the same as buildability, so each item carries a
gating tag: a high-impact item can still be frozen or need a ruling.

### A. High impact (build first)

1. **DX17(b): headless `{js}` executed-output visibility** (the remaining fork; part (a), python/r via
   `read --run`, shipped 2026-07-21, see "Already shipped"). `{js}` (Observable Plot, the corpus's own
   idiom) is still never server-run, so an agent can't headlessly tell a `{js}` chart produced. Plan:
   a local headless Chrome (`chromiumoxide`) over the built page, gated + optional (degrades to
   "skipped: chrome unavailable"), observation-only (no reactive re-run, so the CUT `js-kernel-rerun`
   trap stays out). *Gating: L, net-new; own spec/plan when picked up. Design (Phase 2):*
   [2026-07-21-dx17-headless-executed-output-design.md](../docs/superpowers/specs/2026-07-21-dx17-headless-executed-output-design.md).

#### Editor DevX / language-server initiative

The daily-authoring counterpart to the machine-facing work, and the direct answer to Quarto 2's
headline pitch (*"a new Markdown parser for real-time errors, autocompletion, project-wide YAML
validation"*). Taliesin already has the parser (comrak + block model) and a deep validator suite in
Rust (`check.rs` + `taliesin_core::diagnostics`); the gap is that the VS Code companion surfaces it
**on-save and lossily**, not live and rich. Aligned with the **single-editing-surface** invariant: the
editor is the *only* authoring surface, so this is where authoring quality lives (the collaborative /
visual-editor half of Quarto 2 is out of scope — it needs multiple write paths). Full audit:
[2026-07-21-vscode-devx-audit.md](2026-07-21-vscode-devx-audit.md). Each item pins via the extension
`node:test` harness (`editor/vscode/src/test/`) + the `corpus/diagnostics/` Rust pins. Ordered by value;
pull the top open one.

  (E1 severity/code/docs_url + quick-fix, E2 on-type diagnostics, E3 column-accurate diagnostics, E4
  hover, E5 outline + go-to-definition, E6 front-matter value completion have all shipped — see "Already
  shipped" below. E7's **diagnostics slice has now shipped** too; the remaining E7 capabilities are
  additive on that harness, below.)

- **E7. `taliesin lsp` server — capability follow-ups** *(shipped so far: the stdio harness + live
  diagnostics + go-to-definition + document outline + hover, 2026-07-21; specs
  [diagnostics-slice](../docs/superpowers/specs/2026-07-21-e7-lsp-diagnostics-slice-design.md) +
  [go-to-definition](../docs/superpowers/specs/2026-07-21-e7-lsp-goto-definition.md) +
  [hover](../docs/superpowers/specs/2026-07-21-e7-lsp-hover.md)).*
  `taliesin lsp` (in `crates/server/src/lsp.rs` + `lsp_nav.rs` + `lsp_outline.rs`, `lsp-server`/`lsp-types`)
  advertises `textDocumentSync: FULL` + `definitionProvider` + `documentSymbolProvider` + `hoverProvider`,
  holds a `HashMap<Url,String>` document store, publishes live unsaved-buffer diagnostics (via
  `check::buffer_diagnostics`), answers `textDocument/definition` for `@xref`/`[@cite]`/`{{< include >}}`
  (via `lsp_nav::{classify_target, definition_site, bib_entry_site, frontmatter_bib_paths}`),
  `textDocument/documentSymbol` (the heading outline, via `lsp_outline::outline`), and `textDocument/hover`
  (xref label+number from a live-buffer render's `RenderedDoc::xref_numbers` + `vocab` labels, front-matter
  key docs from `vocab`, `[@cite]` -> brace-balanced `.bib` entry via `lsp_nav::bib_entry_text`).
  **Remaining, each additive on the same server** (the logic still lives only in the VS Code companion's
  TypeScript and must be ported to Rust to become editor-agnostic): **completion** (Rust-backed via
  `vocab`/`symbols`; port the cursor-context detection from `completions.ts`), **rename**, and
  **quick-fix code-actions** (the `suggestion` field already rides the diagnostic). Migrating the VS Code
  companion itself to a `vscode-languageclient` is a separate, later item. *Each: S–M, additive.*

### B. Medium impact

2. **One-command deck publish + presenter tools** *(needs an owner ruling)*: the deck design questions
   with real speaker value. Today the Share QR only encodes `localhost:PORT` and `build` yields a file
   the user must self-host, so: one-command deck publish? Plus a presenter laser/spotlight + auto-advance
   (reveal.js reflexes). Also thread `footer:`/`logo:` through both deck-page builders (no corpus deck
   needs one yet).

3. **Decouple focus mode from OS fullscreen** *(needs an owner ruling)*: focus mode is welded to OS
   fullscreen (`03-focus-mode.js:39-45`, "the author's ask"). Split the calm reading column from
   fullscreen?

4. **Deck engine mobile polish** (P2): mobile pinch/pan + touch gestures (they matter for the phone-feed
   deck mode; hard to verify without a device); drop `fitSlide` from the resize path (needs a lazy
   fit-on-show refactor first).

5. **Locate the site-side cross-page duplicate-label warning** (§2 #1 Part B). `site/xref.rs` +
   `site/mod.rs` push `"duplicate cross-reference label X defined on multiple pages"` onto a
   **`Vec<String>`** channel that carries no location, half-reproducing the Quarto flaw the tool
   critiques. This is **not just a channel type change**: a cross-page duplicate has **two or more
   locations** (page A's line and page B's line), so the fix must first decide what to point at (the
   second definition, both, or a per-page list). *Gating: P3, corpus-exercised (nothing ships wrong).*

6. **Reader-facing offline download** *(needs an owner ruling: scope)*: a "download this
   book/page to read offline" affordance. The built output is *already* 100% self-contained and
   network-free (framework CSS/JS/fonts/KaTeX inlined or `data:`-URI'd; `_assets/` is root-relative), so
   this is roughly 90% repackaging and 10% new: a build-time-generated **static** `<book>.zip` (a static
   link fits the "no server at read time" architecture better than an on-demand `serve_site` route), plus
   an `<a download>` in the book topbar (`site/chrome.rs` ~218-238). No archive crate is in the tree yet
   (add `zip`, or a small store/deflate writer); `build_site_async` already holds the page manifest and
   output tree. *Format settled (owner, 2026-07-21): a **zip*** (a directory is not deliverable via a
   static `<a download>`, which hands the browser a single file; the `file://` double-click gotcha does
   not apply because pages already use document-relative asset paths, `asset_href` at `build.rs:1191`;
   text compresses ~70-85% so it scales with doc size). *Ruling still needed:* whole-site vs per-book vs
   per-page, and default-on vs opt-in (minimal-config favors always-emit, no knob). On-brand: the
   reader-offline experience is
   `FEATURE-IDEAS.md`'s headline opportunity (its #14 framed this as a PWA/service-worker; a zip is the
   simpler, more explicit sibling). **Not a new output format** (a delivery wrapper around the existing
   HTML). *Gating: S/M, net-new.* Pin: build a book, assert `<book>.zip` exists plus a chrome download link.

7. **Media playback behavior** (P2; a11y + UX; two parts sharing one delivery surface): (a) **video
   hover-to-play, pause-on-leave**: `{{< video >}}` emits `autoplay muted loop playsinline` with **no
   `controls` and no pause path** (`render/extension/mod.rs:346`), a live **WCAG 2.2.2 (Pause, Stop,
   Hide)** failure on the forward-facing site (`site/index.tmd`, `site/features.tmd`). Hover-to-play with
   pause-on-leave *satisfies* 2.2.2, but the "perfect default" also needs a touch fallback (no hover on
   mobile, so tap or IntersectionObserver play-when-visible) and `prefers-reduced-motion` meaning no
   autoplay. Must drop the unconditional `autoplay` (rewrite the pin `tests.rs:2881-2918`) and coexist
   with `syncThemeVideos` (`theme.rs:190-208`) plus the lightbox (`11-lightbox.js:125-137`). (b) **single
   active player**: a delegated `play` listener (capture) pausing every other `<audio>`/`<video>` when one
   starts. Today four raw-HTML `<audio controls>` on `corpus/posts/fourier-transform/index.tmd:86-98` can
   all play at once, and there is **zero** media-coordination JS. Both parts ship in **two** client
   surfaces for preview/build parity (`web-client/client.js` plus a bundled `assets/js/` asset). *Gating:
   M, net-new, mostly client-side.* Pin: a media corpus doc (the fourier post is a ready single-player pin).

### C. Low / hardening (P3)

8. **DX16: update-available nudge** (async, boxed, `NO_UPDATE_NOTIFIER` opt-out). S, net-new. *Weigh
   against the offline invariant first (it implies a network check).*

9. **Cross-reference labels are English-only** (§2 #3): an i18n scope question, not a small defect. The
    hardcoded English const table is `cite/render.rs:15-21`, and `lang` appears **zero** times in that
    file, so there is no localization seam yet. `lang:` correctly sets `<html lang>`; the "promise" of
    translated labels was never real. **No corpus doc demands it.**

10. **Remaining design questions** *(owner ruling first, low impact)*: deck inverts the page serif/sans
    logic (`deck.css:705-711`), accept+document or unify? · add a `//| uses:` alias for the consumer
    `//| input:` (weigh vocab sprawl)? · callout kinds are namespaced but theorem kinds are bare,
    document or reconsider? · a Vite user pressing `r/o/u/c/q` or `h` gets silence now that interactivity
    moved to the browser dev menu, so one banner line pointing at the `◇` menu.

11. **ASCII-art `generator` comment** (brand/discovery nicety): a leading HTML comment (project name,
    `taliesin_core::VERSION` from `lib.rs:59`, and a URL) so a developer who opens dev tools or
    view-source can find the tool. The machine-readable half already ships (`<meta name="generator"
    content="Taliesin" />`, `page.rs:277`, pinned by `head_meta.rs`); this adds the human-readable banner.
    One insertion in the shared `assemble_html_page` `format!` (`page.rs:270-295`, placed just inside
    `<head>` to sidestep the doctype-first-byte rule) covers build and both previews; decks need a
    symmetric edit (`deck.rs:70-77`, which lacks even the generator meta today). No test churn (head tests
    use `.contains`). Minimal-config: a default, not a knob. *Gating: S, net-new, low priority.*

12. **Reliability / test-infra long tail** (P3, dev-facing):
    - **R cold-kernel orphan residual:** IRkernel has no `ParentPollerUnix` equivalent, so R cold
      kernels still orphan on ungraceful parent death; there is no clean fix (PDEATHSIG is the only
      lever and is hazardous), and R is rarely the cold single-doc path. `kernel.rs`. (The
      warm-pool, cold-Python and `/tmp`-sweep halves all landed.)
    - **`mounts:` live serve/discovery is untested** (C5's only remaining gap): everything else about
      `mounts:` is pinned; untested is the live `serve_site` `MountedSite` discovery + serving under the
      `/at/` prefix (`serve_site/mod.rs` ~139-170), incl. the "no directory" warn path. A bin-crate
      integration gap (the suite has no live-HTTP serve test). Low-value (mounts are preview-only, so
      nothing ships wrong), demand-driven.
    - **Two load-sensitive timing tests:**
      `exec::tests::pooled_kernel_serves_cells_without_a_long_warming_state` +
      `kernel::tests::kernel_executes_state_errors_and_interrupts_runaway_cell` fail under CPU load; both
      assert on timing. Fix: wait on a **state signal**, not a duration.
    - **Two `exec::tests` concurrency-race tests** (NOT timing):
      `a_successful_probe_pins_the_freeze_key_format` +
      `a_failed_interp_probe_is_not_memoized_for_the_process_lifetime`. On pristine `main` they fail
      ~2 runs in 3 in a full `--bins` run, never when filtered, and pass 3/3 under `--test-threads=1`
      (which is slower, so it refutes timing). The assertion: the freeze key's interpreter-id segment
      comes back **empty**; `probe_version` returned `None`, and since the 10s `bound` can't have fired,
      the spawn failed. Leading (unproven) hypothesis: **`ETXTBSY`** from `write_exe`'s (`exec.rs:1228`)
      write-then-exec race across tokio threads. **Do not fix from this note** (exec/kernel zone,
      unproven): the cheap first move is to make `probe_version` log *why* it returned `None`, then
      re-run the full suite until it trips.
    - **`build.rs:926` warms the pool before knowing any page needs a kernel**, even under
      `TALIESIN_NO_EXEC=1`. Hygiene, not perf (0.25s vs 0.27s on a prose-only site).
    - **Mermaid `<script>` SRI + `crossorigin`:** deferred (only live Preview lazy-loads from the CDN; a
      build inlines the vendored copy). Needs a hash pinned to the CDN build; `integrity`+`crossorigin`
      would break a non-CORS `TALIESIN_MERMAID_URL` override.
    - **Perf (low):** protocol-level op-message batching (one WS message per save, not per-op). Worst
      case: an edit near the top of a long doc where every downstream block emits a `SetMeta`
      (`diff.rs` `anchor_op`). Client + server ship together, no wire-compat constraint.
    - **Audit long-tail:** a tens-of-MB cell output blocks ZMQ receive before the cap fires
      (`kernel.rs`, do-not-touch).

### D. Gated, not actionable now (kept visible, do not spin up)

- **M6a `MAX_WARM_PAGES` / `exec_pool.rs` eviction:** the standing freeze; sign-off refused 2026-07-17.
  Eviction drops the executor and kills its kernel child processes, so this is kernel lifecycle, not a
  constant. Do not tune without a new ruling.
- **M2's hanging-interpreter sibling** *(needs its own exec/kernel ruling)*: a *hanging* (not missing)
  interpreter costs ~161s recovery, downstream of the (bounded) `interp_id` probe in the warm-pool
  forkserver READY wait + kernel-start retries. `kernel::tests::transient_start_errors_retry_but_missing_interpreter_does_not`
  shows the *missing* case is handled and the *hanging* one is not. `kernel.rs`/`warm_pool.rs`.
  *(Aside, pre-existing + load-bearing: `crates/server/Cargo.toml` doesn't list tokio's `process`
  feature though `kernel.rs`/`warm_pool.rs`/`exec.rs` use it; it compiles only via feature unification.)*
- **M4 test stand-in flake:** the M4 test's `sleep 300` stand-in kernel survives ~2 of 8 full-suite runs,
  only when the build is cold. Measured, unexplained, argued test-only (a real kernel has three reclaim
  nets where the stand-in has one). Worth an hour only if a real kernel is ever seen outliving its pool.
- **D72 bare `@key`:** declined for now (the diagnostic already ships, so nothing renders wrong
  silently, which makes it a feature question not a defect). Edits `crates/core/src/cite/`, needs
  sign-off if revived.

## Tier 3: demand-driven (band E; build only when a real user asks)

Per the PMF audit ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md)) the highest-value next move is
**real users, not more features**, so this whole band waits on demand.

- **Companion (Phase 2):** editor commands (`.tmd`-buffer text transforms only, never preview gestures);
  `editor.wordWrap` default for `[taliesin]`; grammar polish (YAML-type `#|`/`//|`/`%%|` values;
  recommend cell-language extensions via `.vscode/extensions.json`); **marketplace packaging hygiene**
  (`.vscodeignore` misses `.vscode-test/` (1.8 GB), `test-fixtures/`, `scripts/`, `out/test/`,
  `out/e2e/`; no top-level `icon`/`repository`/`license`/`keywords`; `"private": true` blocks publish);
  `symbolCache` only invalidates on save (`completions.ts`, low).
- **LaTeX hover-preview in the VS Code editor** (Companion Phase 2, a sub-case of the LSP item below):
  hover `$…$`/`$$…$$` to see a rendered preview. Math is already grammar-recognized
  (`tmd.injection.tmLanguage.json:15-37`), but the extension has **no HoverProvider** yet
  (`editor/vscode/src/`). Rendering-reuse is cheap: `math::render(latex, display)` is a pure, memoized
  function (`math.rs:57`), wrappable in a thin `taliesin math <expr>` subcommand. The **hard part is
  fidelity**: KaTeX's HTML+CSS will not survive VS Code's Hover sanitizer (no external stylesheet or
  `@font-face`), and the `katex` crate emits no image/SVG, so a legible offline hover likely needs a
  rasterization step (new dependency surface), not a reuse of the offline KaTeX path. **Spike first**
  (does the Hover sanitizer keep enough inline styling to be legible? VS Code's own Markdown extension
  does math hover, so there is precedent). Build it as a sub-case of the **LSP** item below (write-once
  for Neovim/Helix/Zed/VS Code), not a bespoke VS Code-only hack. *Gating: M, demand-driven, fidelity risk.*
- **`.tmd` format-on-save** (open question): a source pretty-printer must preserve `data-sourcepos` line
  stability for click-to-source; brainstorm reflow-vs-risk first.
- **Dogfood: migrate the FL-weather book to Taliesin** — a real Quarto to Taliesin migration +
  portability stress test; pin a reduced version under `corpus/` if it renders clean.
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic).
- **`taliesin publish` follow-ups:** optional `--init` wrapper for the one-time `wrangler` setup;
  email-allowlist (Cloudflare Access) mode. (Also the Zenodo DOI on-ramp, `CITATION.cff`/`.zenodo.json`
  to a GitHub-release DOI, belongs with Wave 5's repro/print-pdf track.)
- **Interactive/explorable numerics** (`FEATURE-IDEAS.md` #62-66; none pinned; promote with a corpus pin
  when one graduates; must NOT reintroduce a reactive VM). Highest-leverage: **#62** a bundled
  numerics/stats global for `{js}` + **#63** `animate`/play-tick + draggable-`point` `{{< input >}}`.
- **Wave 5** (`ROADMAP.md`): print-pdf track (paged render *of* the built HTML), docs-as-spec, `{glsl}`
  cell language, SEO completeness. **Fold `llms.txt`/`llms-full.txt`** in (the block model separates
  prose from code/math at `client.js:50`). *Pin: a `tech_blog.rs` assertion that `llms.txt` lists
  discovered pages + `llms-full.txt` excludes drafts.*
- **Site-level shared bibliography + hygiene** (M). `bibliography:` is per-document only
  (`cite/mod.rs:42`). Allow it in `_site.yml`, merged under each page's; add two read-only diagnostics
  ("entry never cited", "duplicate key") over the parsed registry (does NOT touch the BibTeX/CSL
  do-not-touch core). *Pin: a small site, one entry cited from two pages, one uncited.*
- **Author structure panel** (M/L). A read-only preview sidebar: the heading tree with per-section word
  count (`client.js:50-58` already counts) + a badge per node for unresolved xref / TODO / over-goal
  length. Click to scroll; move the editor cursor via cursor sync. An annotation layer on the dev panel,
  not a new component. *Pin: `corpus/layout/structure.tmd`.*
- **Session revision digest** (M). Surface the `BlockOp` stream the client already receives: a session
  word delta + a feed of the last N ops, each click-to-source. *Behavioral pin (a `tools/live-edit-bench`
  assertion), not a corpus doc.*
- **Block-level transclusion** `{{< include file.tmd#sec-id >}}` (M). Reuse a section across a series.
  Must ride **on top of** the `includes.rs` source-map pass (resolve fragment to block range, hand a
  sub-slice), never rewrite it. Hard gate: the source map must not perturb. Defer until a series needs it.
- **LSP for the language intelligence** (L). Everything an LSP needs is already in Rust (`check`,
  `vocab`, `register_xref`, bib parser, `closest()`); write-once for Neovim/Helix/Zed/VS Code, removes
  the `#| label:` completion drift. The preview stays the view (two `postMessage` shapes in
  `docs/internals/protocol.tmd:325-350`). Do NOT rebuild the preview as an LSP.
- **Image optimization** (WebP/AVIF + `srcset` + lazy-load behind a content-hashed cache) — until posts
  get image-heavy.
- **Marketing site** (deferred, feature-first; rolls into a demo-machine rebuild): `live-edit-hero-demo`
  clip; swap `site/_site.yml` placeholders; demo-led hero rebuild (3-viewport spot-check of the
  already-fixed 390px hero overflow + theme/video desync); **#12 demo video needs a pause affordance
  (WCAG 2.2.2) + reduced-motion respect** and its baked-in desktop text downscales ~3x on mobile
  (re-record or ship a mobile source); mobile embed refine; deploy.
- **`serde_yaml` fallback watch-item:** if 0.9 breaks against a future serde/edition, swap to
  `serde_yaml_ng` (v0.10), gated on a test that `Error::location().line()` still works. Fix the stale
  `Cargo.toml` comment (names the unsound `serde_yml`) when touched.
- **PMF demand-driven tail** ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md), Tier C; each waits on a
  real user asking): hover-preview extended to inline `[@key]`/footnotes (reuse `site/hover.rs`);
  reader-owned document-level show/hide-code toggle (a reader-local pref, a11y-exempt); on-page code+data
  download plus a "reproducible" affordance; scroll-synced TOC greying of passed sections;
  versioned/permanent-URL scheme for link-rot distrust; deck autoplay/kiosk loop; a docs "deck powers"
  page (the `?`/`m` shortcut menu exists; first-timers don't know it does).

## Quarto catalog (policy, not a task)

**Owner ruling 2026-07-16: no sweep. Triage an area on demand, when you next work that area.** Before
consulting it read the triage doc's "three layers" section
([2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md)): the entries are the asset
and were well-grounded on 2026-07-03, but the heading status is degenerate and the executive summary is
misleading. A skeptic verdict is evidence, never a ruling (its "drop Atom feeds" verdict was overruled;
Atom shipped with autodiscovery).

## Already shipped: do not re-add / re-scope

The bulk of this file used to be blow-by-blow `LANDED` records; that detail lives in git +
[AUDITS.md](AUDITS.md). Kept here only as the anti-rot guard (grep the named symbol before trusting any
claim that one of these is "missing"):

- **DX audit batch** DX1-DX15, DX18, DX19 shipped; **DX17(a)** shipped 2026-07-21 (below); only
  **DX16** and **DX17(b)** (headless `{js}`) remain (above).
- **Editor DevX (VS Code companion) E1-E6 shipped 2026-07-21** (audit
  [2026-07-21-vscode-devx-audit.md](2026-07-21-vscode-devx-audit.md); only E7 `taliesin lsp` remains, above;
  spec/plan `docs/superpowers/specs|plans/2026-07-21-editor-devx-e3-e5.*`):
  E1 rich diagnostics + did-you-mean quick-fix; **E2** on-type diagnostics (`taliesin check --stdin`
  lints the piped buffer, not the saved file, skipping the interpreter probe; debounced
  `onDidChangeTextDocument`; pin `stdin_buffer_is_linted_instead_of_the_on_disk_file` + `debounce.ts`
  node:tests); **E3** column-accurate diagnostics (a `[col,end_col)` span on `Warning`/`Diagnostic`,
  serialized `skip_if_none` so un-columned JSON stays byte-identical; front-matter key typos get the span
  via `block_key_span`/`nested_key_span`, xref stays whole-line — it is HTML-derived, block-line only; the
  squiggle covers the token and the quick-fix uses `fixSpan` (exact span, no edit-distance guess); pins
  `frontmatter::tests::unknown_*_column_span`, `check_json_front_matter_typo_carries_a_column_span`,
  `check.test.ts` `fixSpan`); **E4** `HoverProvider` resolving `@xref`→label / front-matter key→doc /
  `[@key]`→BibTeX entry (pure `hover.ts` + shared `backend.ts`); **E5** document outline
  (`DocumentSymbolProvider` over a pure `outline.ts` heading scan) + go-to-definition
  (`DefinitionProvider`: `{{< include >}}`→file, `@xref`→same-doc def via `definitionSite`, `[@key]`→`.bib`
  via `bibEntryOffset`; buffer+filesystem, no backend; `outline.test.ts`/`definition.test.ts`); **E6**
  front-matter value completion (`vocab` `frontmatterValues` for `format`/`theme` + a `frontmatter-value`
  `detectContext` case).
- **DX17(a) headless executed-output (python/r) shipped 2026-07-21:** `taliesin read --run` executes
  python/r via build's exec path and projects `[figure fig-x: produced, alt "…"]` / `[output: …]` /
  `[cell error: …]` (+ `--format json` per-cell). Core `classify_exec_output`; pinned by
  `corpus/agent/executed-read.tmd` + `read_run.rs`; AGENTS.md onramp documents it. Phase 2 (headless
  `{js}`) remains as item 1.
- **Click-to-source into `{{< include >}}`d files already works** (do not re-scope as "build it"): an
  Alt-click on included content already opens the *included* file at the correct line on both paths
  (plain-browser `vscode://`, and the VS Code webview via `qmd-goto`), because included blocks carry
  `data-source-file` from the `includes.rs` per-line source map and labels are kept primary-doc-relative.
  Pinned by `corpus.rs:161-219` (plus the "every `source_file` must be relative" invariant,
  `corpus.rs:124-137`) and the companion's `paths.test.ts:45-58`. **Only real gap:** `web-client/` has no
  JS tests at all, so `openSource()`'s include handling is proven by corpus attributes and inspection, not
  a JS assertion on the emitted `vscode://` URL or `qmd-goto` payload (a small P3 hardening add if wanted).
- **Deck audit** fully shipped; **B3-18** (the last item) landed 2026-07-21: a structural deck edit now
  re-mounts only the edited `<section>`s (client-side signature-keyed reconcile in `client.js`), so
  untouched slides keep their live `{js}`/WebGL/input state. Prerequisite fix: `{{< input >}}` control
  ids are name-based (`qin-<name>`), not line-based, so an input block's `data-block-id` is
  position-independent (`render/extension/mod.rs`).
- **Polish audit batch** PL1-PL20 all shipped (`git log --oneline origin/main | grep PL`).
- **PMF builds** B1 (reader "Cite this" box), B2 (book landing-page auto-TOC), B4 (deck Marginalia
  identity) shipped. B5 Zenodo DOI is demand-driven (above).
- **Corpus-coverage** C1-C7 pinned; only C5's `serve_site` mount serve-path remains (in P3 above). C3/C4
  done, C6 was never a gap.
- **Machine-facing audit** M1, M2, M3-M5, M6b shipped; **M6a is frozen**, M2's hanging-interpreter
  sibling + the M4 stand-in flake remain (gated, above).
- **AI-native packaging + guardrails** (the former Medium #2) fully shipped: `taliesin map --format json`
  (`map_cli.rs`), the citation-wired `paper` scaffold + `--json` on `new`/`init` (`corpus/scaffold/`),
  `build`/`publish` `--format json` (`structured_build_errors.rs`), the default-on placeholder-alt nudge
  (`diagnostics/a11y.rs::placeholder_alt_message`), and the distributable Claude Code skill
  (`editor/claude-code/skills/taliesin`, drift-locked by `skill_freshness.rs`).
- **R/Python stream ANSI leak fixed 2026-07-21** (the former #6): `render_outputs`' `Output::Stream` arm
  now `strip_ansi`s before escaping, matching the error arm, so R `message()`/`warning()` (and Python
  coloured stderr) no longer leak `[31m…[0m` into the page (`kernel.rs`; pinned by
  `render_outputs_strips_ansi_from_streams`, verified end-to-end against a real R kernel).
- **Live defects** §2 #1 Part A, #2, #4-#10 shipped; only Part B (P3) + #3 i18n (low) remain (above).
- **Reduction/modularity** Phase 2 + T1 + R2 (scanner unification) shipped; the codebase is already lean.
- **Ungraceful-death reaping** warm-pool forkserver + cold-Python kernel + stale-`/tmp` sweep shipped;
  only the R cold-kernel residual remains (in P3 above).
- **`assets/js/*` `tsc`/`@ts-check`** at strict-zero, CI-gated. **Interpreter selection signal +
  project-local `python:`/`r:` `_site.yml` fields** shipped. **OG-card coverage** (book chapters + decks)
  shipped.

## Decided against / do-not-re-litigate

- **2026-07-12 wishlist cut to `FEATURE-IDEAS.md`** (revive only when a corpus doc needs one):
  cross-revision diff, repro manifest, List-of-Figures/Tables/Theorems, interactive tables, line-level
  code xrefs, image `dark=`. Reader text-size/line-spacing controls declined (a11y-exempt substrate in
  `14-reader-prefs.js`). Stale `new-post`/`new-project` scaffolder skills retired (the `deploy` skill
  stays).
- **TODO / FIXME surfacing skipped** (owner ruled 2026-07-10): no `level` concept exists, so a TODO
  warning would fail `check` on every draft. If revived, design A (preview-only `Diagnostic::info` at
  `serve/mod.rs::compute_diagnostics`) beats re-plumbing a real `level`; the scan must NOT reuse
  `prose::strip_inline` (it blanks code, where TODOs live).
- **AI-native leftovers declined 2026-07-16:** `check --online` citation resolution (the only proposed
  network egress; buys a link-rot check at the cost of the offline invariant; if ever revived, check-only,
  off by default, never reachable from `build`/`publish`); numeric/quoted-claim-without-citation hint
  (its own spec rates it FP-prone); per-page text/JSON sidecar (redundant, `taliesin read` +
  `llms.txt`/`llms-full.txt` ship).
- **Refuted by measurement (do NOT re-scope):** `build` does not leak forkserver subtrees (the graceful
  path is reaped; the *ungraceful* R residual is the only gap, above); the warm pool booting Python on
  prose-only builds is hygiene, not latency; dev attributes are 0.29% of page bytes (don't strip); a
  `--version -dirty` marker is stale-by-construction (refused); the `assets/css` stale-embed claim did
  not reproduce (re-verify for `assets/js` before any touch-render workaround); the 390px `hero:`
  overflow + theme/video desync are already fixed; include symlink-loop SIGABRT does not exist (Linux
  caps at `MAXSYMLINKS=40`).
- **`_redirects`/`_headers` preserved, never generated** (`build.rs:1881` treats them as author-placed
  deploy metadata; `stale_sweep.rs` pins it). Auto-generating them is a "perfect the default vs add a
  knob" call; leave as-is unless a real deploy proves it needs one.
- **Gate the gate:** a drift test that cannot fail is worse than none. Any new drift gate must be
  mutation-checked against exactly the shape it guards.
- **Library outsourcing decided against** (each verified vs the invariants): hayagriva/biblatex,
  schemars, jsonschema, morphdom/idiomorph, similar/dissimilar, clap, owo-colors, slug, html-escape,
  lightningcss/palette, IntersectionObserver/scrollspy libs, deck micro-helpers. Keep `two_face` extras
  filling gaps only (the bundled syntect set is consulted first and must win).
- **Reading-first defaults, research-validated keeps** (do NOT "fix"): serif body for long-form screen
  reading; ~70ch measure `--tali-maxw: 46rem`; right-rail scrollspy + width-gated sidenotes; scroll (not
  pagination) book reading; ship REAL bold/italic faces, never synthesized.
- **2026-07-06 decisions:** book pager stays bottom-only; book page-TOC fix-in-place, keep both nav
  surfaces; xref graph tool removed; focus mode stays ephemeral; deck overview keeps per-slide
  backgrounds; dev-menu + `#tali-progress` + reading-progress bar stay three separate signals.
- **2026-07-18 PMF re-derivations:** the reader "Cite this" box (D70) was REVIVED and shipped as B1; the
  deck desktop "async handout" reading view stays CUT (do not re-open without a fresh ruling).

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Per the PMF audit (2026-07-18) the tool is
feature-complete for ~one real user, so the highest-leverage next move is **real users**, not more
features; the owner is publishing soon to gather feedback. When publishing, lead the copy with the
**speed moat** (warm server, block-level incremental, no per-edit rebuild), the single most-repeated
Quarto grievance and the most under-marketed asset.
