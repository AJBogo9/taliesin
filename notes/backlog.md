# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git + [AUDITS.md](AUDITS.md) +
> [ROADMAP.md](ROADMAP.md); delete an item when it lands, don't leave a `[x]`. The "already shipped"
> list near the bottom is the compact anti-rot guard (do not re-add / re-scope), not a changelog.

## State (2026-07-22)

v0.2.0. All four formats render + deploy; the dev loop is strong (block-level incremental updates with
DOM-state preservation, warm server + Jupyter kernel, `_freeze` cache, Alt-click + reverse cursor sync,
located diagnostics, CSS hot-swap, Cmd-K search). The editor language intelligence (diagnostics,
go-to-definition, outline, hover, completion, quick-fix code actions, rename) now ships editor-agnostically
as the `taliesin lsp` stdio server: the **E1-E7 editor-DevX initiative is complete** (see "Already
shipped"). **Most of the backlog has already shipped.** Through item 19 everything is pushed (`origin/main`
at `cc45af4`); the live-executor-mounts F-04 fix landed to **local `main` after that (unpushed, the author
pushes)**. A large **2026-07-22 (late) backlog-clearing pass**
shipped: focus-mode/fullscreen split (was item 3); a Vite-user
hint banner (item 9); deck `footer:`/`logo:` (item 2); a per-book offline `<book>.zip` (item 6); the
cross-page duplicate-label warning is now located (item 5); DX16 update-nudge ruled **skip**; item 8 i18n
labels **assessed → defer**; and all six item-11 polish passes (a)-(f). **DX17b headless `{js}` also shipped
2026-07-22** (the last high-impact feature); the AP8 determinism guards (was item 15) are complete and
that item is now removed. **The machine-facing `read` projection (was item 19) shipped + pushed 2026-07-22**
(structure-preserving lists/steps/inputs + book-aware chapter/cross-page scoping + whole-book `read <dir>`;
see "Already shipped"). **The live-executor-mounts F-04 fix also landed** (local `main`, unpushed). What
remains open is smaller and mostly P3. Ranked below by product impact.

## Next session: start here

Tree is green across all gates. Both branch features landed to local `main`: structure-preserving `read`
(item 19, also pushed) and the live-executor-mounts F-04 fix (unpushed, the author pushes). No open branches
remain. Pick in priority order:

1. **Item 14 heading-demotion**: real and evidence-backed, and **owner-gated** (reshapes every corpus
   snapshot; needs a model ruling before building).
2. Then the medium/low band: deck mobile polish (item 4), OFF-2 mermaid-offline-preview (item 13), the
   small persona findings (items 16-18), and the P3 test-infra + polish residuals (items 10, 11).

- **Or run one of the eight remaining *audit perspectives* ("Audit perspectives" section below):**
  proactive, findings-generating angles the prior rounds structurally could not see (perf, fuzzing,
  concurrency, cache-correctness, i18n/sourcepos, cross-browser, a11y, determinism, semantic HTML,
  codebase health, chaos, offline-proof). Each is a fresh session that writes a dated findings doc and
  feeds build-ready items back here; the author has credits queued for exactly this. Recommended next:
  AP2 (fuzzing) + AP4 (freeze cache), both stateful/solo; the pure code-read AP10 is fan-out-safe. (AP5,
  AP8, AP9, AP12 already run.)

Working method is in "Standing constraints": branch per feature, verify by mutation, browser-verify,
ff-merge locally, delete the item here on landing.

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

Currently clear: the last high-yield item (structure-preserving, book-aware `read`) shipped 2026-07-22
(branch `structure-preserving-read`, see "Already shipped"). Next-highest is item 14 (heading-demotion,
owner-gated) in band C.

### B. Medium impact

2. **Deck presenter tools** *(owner deferred 2026-07-22 — NOT selected this round)*: one-command deck
   publish (Share QR still encodes `localhost:PORT`), a presenter laser/spotlight, auto-advance. The
   `footer:`/`logo:` threading from this item **shipped** (see "Already shipped"); the presenter pieces
   were considered and left for later. Revive only on a real speaker ask.

4. **Deck engine mobile polish** (P2): mobile pinch/pan + touch gestures (they matter for the phone-feed
   deck mode; hard to verify without a device); drop `fitSlide` from the resize path (needs a lazy
   fit-on-show refactor first).

### C. Low / hardening (P3)

10. **Reliability / test-infra long tail** (P3, dev-facing):
    - **R cold-kernel orphan residual:** IRkernel has no `ParentPollerUnix` equivalent, so R cold
      kernels still orphan on ungraceful parent death; there is no clean fix (PDEATHSIG is the only
      lever and is hazardous), and R is rarely the cold single-doc path. `kernel.rs`. (The
      warm-pool, cold-Python and `/tmp`-sweep halves all landed.)
    - **`mounts:` live serve/discovery: only an automated live-HTTP test is missing** (the live-executor-mounts
      branch LANDED to local `main`): the F-04 work reworked `serve_site` mount discovery/serving and unit-pins
      the pure `match_mount`/`resolve_project`/`classify_change` helpers, and live mount serving is
      browser-verified. What remains is only the bin-crate gap of an end-to-end live-HTTP serve test (no
      `reqwest`/`TcpListener` harness). Low-value (mounts are preview-only), demand-driven.
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

11. **2026-07-22 polish-audit residuals** (P3 hardening + a11y + "feels finished"; detail:
    [2026-07-22-polish-audit.md](2026-07-22-polish-audit.md); [AUDITS.md](AUDITS.md) records the round).
    **Passes (a)-(f) all shipped** (design-system single-source, scaffold `<h1>`/`<time>`, `<article>`
    landmark, announce/focus holes, CLI/diagnostics, reduced-motion+print, emitted-markup a11y; see
    "Already shipped"). Only low/opportunistic leftovers remain:
    - **Tokens (F2/C5/F4/S3):** a `--tali-scrim` token folding 3 divergent overlay alphas (PA-F2),
      per-slide-bg hex drift-lock (PA-C5), px/rem breakpoints (PA-F4), base.css's own `.15s` uses (PA-S3).
    - **a11y interaction (B3/B5/B14/B15):** mobile-TOC focus-trap, cite-tabs roving-tabindex, Cmd-K
      Home/End, menu tab-out-close.
    - **Semantics (M3/M13, H1):** `<ul>`/`role=list` (needs a CSS-grid + category-filter-JS restructure +
      browser verify), hero/card image-alt lint nudge, deck `theme-color`/OG (PA-H1 residual).
    - **CLI docs (CLI1/2/3):** `--help` drift (undocumented `preview --port` / `read --run` / hand-written
      usage). Owner design-Qs (deck copy-button, card whole-`<a>`) are parked in the doc, not build-ready.

12. **i18n / Unicode multibyte correctness: DONE bar a demand-driven residual.** The LSP UTF-16 encoding
    fix shipped 2026-07-22 (folded from AP5; detail:
    [2026-07-22-i18n-unicode-sourcepos-audit.md](2026-07-22-i18n-unicode-sourcepos-audit.md)): the stdio
    LSP advertises `positionEncoding: utf-16` and converts at every boundary (I18N-2/3/4/5); I18N-1 was
    resolved as documentation (block start columns are always ASCII-prefixed, so the client conversion was
    unreachable). *Residual (not build-ready, demand-driven, do not spin up without a real ask): RTL
    layout, CJK line-breaking, non-ASCII heading-slug collisions.*

13. **Offline-guarantee: OFF-2 mermaid preview** (P3; detail:
    [2026-07-22-offline-guarantee-audit.md](2026-07-22-offline-guarantee-audit.md), AP12). **OFF-1 shipped
    2026-07-22**: `build` and the site build now emit one located, informational warning per view-time
    external reference left in a `--out`/site output (never fails the build; see "Already shipped";
    deferred follow-ups: CSS `url()`/`@import` hosts, surfacing in `check` / `--format json`). **OFF-2
    (S-M, open):** live preview lazy-loads mermaid from a CDN despite the vendored copy, so inline the
    vendored library on mermaid pages (gated like the build path) or surface the network load. Overlaps
    item 10. `render/mod.rs:1292-1311`.
    *Verified offline (do not re-audit): fonts, KaTeX, d3/Plot, mermaid-in-build, the reveal/jsdelivr guard.*

14. **HTML-1: heading-demotion for a single-root document outline** (P3 semantic/a11y, OWNER-GATED; detail:
    [2026-07-22-semantic-html-audit.md](2026-07-22-semantic-html-audit.md), perspective AP9;
    [AUDITS.md](AUDITS.md) records the round). A titled document emits a title-block `<h1 class="title">` AND
    renders every author `#` heading as `<h1>` (`emit.rs:15`), so titled multi-section docs emit many sibling
    `<h1>` (proven: the built `corpus/bayesian-website` index has 12 `<h1>` in one `<main>`; 20 corpus docs emit
    2 to 12 each). The visual render is fine, but the semantic outline is a flat list of competing roots, which
    contradicts the tool's own single-h1 intent (PA-H2 injects a hidden `<h1>` only when the body has none).
    This is the "heading-demotion" idea gated in the 2026-07-11 website-design audit; AP9 adds the evidence.
    Fix: when a title-block `<h1>` is present, demote author heading levels by one for the HTML document view
    (`#` becomes `<h2>`, ...). Verified safe/scoped: heading ids come from `slugify(text)` not the level
    (`mod.rs:1496`, `520-534`), so anchors/xrefs survive; **decks must be exempt** (the deck engine groups
    slides BY heading level, `deck.rs`); a no-title doc keeps `#` as `<h1>`. `crates/core/src/render`; reshapes
    most corpus render snapshots, so it needs an owner ruling on the model before building (why it was gated).
    Size: M + a wide mechanical snapshot update.
    *Verified valid across 84 renders + a site build (do not re-audit): zero invalid nesting, zero per-page
    duplicate ids, well-formed figures (one `<figcaption>` each), labelled deck sections, valid list/table/dl,
    `<header>`/`<main>` landmarks present. The render pipeline's HTML structure is sound; only the h1 outline
    is off.*

16. **Demand-probe (course pilot) findings** (P2/P3, in-scope; detail:
    [2026-07-22-corpus-demand-probe-course-author.md](2026-07-22-corpus-demand-probe-course-author.md)).
    A realistic lecturer's course (`corpus/course/`, corpus-pinned by `course.rs` + a `/gallery/course`
    marketing-site exhibit) was authored to probe where a book-length computational project meets friction.
    The *stacked* HTML interactions (book × shared-theorem-counter × chapter-scope × cross-page refs ×
    deck-embed-in-chapter × code-walkthrough × `{python}` cell × draft-appendix) ALL work — 0 interaction-bugs.
    The remaining findings sit on secondary surfaces (F-02 book-scoped `read` shipped 2026-07-22, see "Already
    shipped"):
    - **F-01 (friction, P3):** `theorems:` is not a book-level (`_site.yml`) key — it warns/errors and is ignored;
      a book-wide theorem policy must be repeated per chapter. Candidate: recognize `theorems:` at book level.
    - **F-03 (friction, P3):** the `read` text projection of `{{< embed >}}` (leaks iframe UI chrome) and
      `.code-walkthrough` (steps + code concatenate) is lossy.

17. **Demand-probe (OSS docs-maintainer, persona #2) findings** (P3, in-scope; detail:
    [2026-07-22-corpus-demand-probe-docs-maintainer.md](2026-07-22-corpus-demand-probe-docs-maintainer.md)).
    A realistic library documentation site (`corpus/tarn/`, corpus-pinned by `tarn.rs` + a `/gallery/tarn`
    marketing-site exhibit) probed the tabsets × full-text-search × API-reference cluster. The *stacked*
    interactions (book × Guide/Reference parts × two `.panel-tabset`s per page × `.code-walkthrough` ×
    guide→reference `.tmd#anchor` cross-page links × chapter-scoped `@sec-` refs × Cmd-K search spanning the
    book incl. tabset-hidden content × version/deprecation callouts × mount) ALL work — 0 interaction-bugs.
    Four P3 findings, all on secondary surfaces:
    - **F-01 (friction, P3):** `powershell` is not in the bundled syntect set, so a Windows install snippet
      renders as unstyled plain text + a `TAL-CODE-LANG` warning (`bash` highlights fine). `two-face` ships a
      PowerShell syntax; adding it to the bundled set closes it. Edits `crates/core/src/highlight.rs`.
    - **F-04 (friction, P3):** single-file `check` (the editor companion) false-positives a `site/gallery.tmd`
      card's `mounts:` link as broken, because single-file mode lacks site/mount context; whole-site
      `taliesin check site` is clean and the build is unaffected. Candidate: treat an unknown-prefix link
      matching an enclosing site's `mounts:` entry as valid in single-file mode. Related to item 10's
      "`mounts:` live serve untested" + item 16's F-04.
    - **F-02 (WAI, no action):** the a11y heading-skip lint fires on a `#` title + flat `###` API entries;
      the linter is correct (demote entries to `##`). Recorded as an authoring-DX nuance, not a defect.

18. **Demand-probe (interactive-explainer, persona #3) findings** (P3, in-scope; detail:
    [2026-07-22-corpus-demand-probe-interactive-explainer.md](2026-07-22-corpus-demand-probe-interactive-explainer.md)).
    A single-page explorable explanation (`corpus/descent/`, gradient descent, pinned by `descent.rs` +
    a `/gallery/descent` exhibit) stacked the interactive cluster the corpus never combined on one page —
    `{{< input >}}` sliders × a **draggable** `{js}` graphic × a `.scrolly` sticky `{js}` graphic × a
    reactive Plot cell × math × two numbered SVG figures — and it ALL works, standalone and mounted, 0
    console errors. Two remaining P3 findings (F-01 read-projection fusion shipped 2026-07-22, see "Already
    shipped"):
    - **F-02 (gap, P3):** an authored numbered figure is emitted as `<img src="fig.svg">`, and an
      `<img>`-embedded SVG is style-isolated: it can't see `--tali-*` or the `qmd-theme` toggle, only the
      **OS** `prefers-color-scheme`. So a reader who forces the page theme opposite their OS gets the
      figure in the wrong palette (light-palette labels, weak contrast, on a dark page). Inline `{js}`/SVG
      graphics on the same page track the toggle fine (they use `--tali-*`). Candidates: an inline-SVG
      figure path so `![](x.svg)` inherits page vars, or document a neutral-palette convention. Edits
      would touch `crates/core/src/render/figure.rs` (figure emission).
    - **F-03 (WAI, authoring nuance):** a `{js}` "once" cell's returned node is mounted *after* the cell
      body runs, so an attachment-gated init (`if (!node.isConnected) return`) silently no-ops the first
      paint. Gate teardown on `invalidation`, not DOM attachment. WAI but a sharp edge — candidate: a doc
      line in the `{js}`-cell reference, or an optional post-mount hook.

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

## Audit perspectives (unexplored angles: pick ONE per session)

Brainstormed 2026-07-22 against the [AUDITS.md](AUDITS.md) ledger, which already covers UI, feature polish
(x3), website/marketing design, machine-facing/AI-native, the deck subsystem, DX, PMF, the VS Code
companion, simplification/reduction, and a pre-open-source security + supply-chain pass. The twelve items
below are the dimensions those rounds could **not** see: they need the tool *run hard*, fed *hostile
input*, or reasoned about as a *concurrent system*, rather than "look at rendered output" or "read code for
feature quality." They are the moat's crown jewels (source-mapped, warm, incremental, offline) which no
round has yet stress-tested.

**These are perspectives, not tasks.** Point a fresh session at one item. It produces a dated findings doc
in `notes/` (same shape as the existing audit files: headline, verified findings ranked by corrected
severity, and false leads recorded honestly per the project's "trust the symptom, re-derive the cause"
rule), records the round in [AUDITS.md](AUDITS.md), and files the build-ready findings back into "Open
work" above with their own prefix. Every session inherits the **Standing constraints** at the top of this
file (Do-NOT-touch freeze, verify-by-mutation, entries rot so re-derive from source). Two facts that set
the priority: non-test code carries ~700 `unwrap()`/`expect()`/`panic!`/`unreachable!` sites, and
`data-sourcepos` (the load-bearing invariant) is byte-offset based.

**Run one perspective per session** (context isolation + token budget: the author's stated preference).
The *stateful* ones (AP1, AP2, AP3, AP4, AP5, AP6, AP11) each build, fuzz, run the server, bind ports,
drive a browser, or spawn kernels, so they corrupt each other if run at once: keep them solo. Only the
pure code-read ones (AP9, AP10, AP12, and the read half of AP8) are safe to fan out together in one
Workflow. Recommended first three, by yield for effort, each striking a load-bearing invariant no one has
attacked: **AP2 (fuzzing), AP4 (freeze cache), AP5 (multibyte sourcepos).** AP5 is RUN (2026-07-22, see its
entry below); AP2 and AP4 remain, but both are *stateful/solo* and collide with a live feature session that
owns the exec/serve/build surface, so a pure code-read pick (AP9/AP10/AP12) is the safer next while that
session runs.

### Tier 1: genuinely untouched, highest expected yield

- **AP1: Performance & scale.** No perf note exists in `notes/`; every prior audit used small corpus docs.
  Hunt: cold-build time on a ~200-page site, a `.tmd` with ~10k blocks, RSS growth over a multi-hour warm
  preview, whether the block diff (`crates/core/src/diff.rs`) goes quadratic anywhere, kernel RSS drift.
  The warm incremental loop *is* the moat; nobody has measured where it degrades. Start: generate synthetic
  large docs, trace build/rebuild latency + RSS, flamegraph the diff. *Stateful, solo.*
- **AP2: Robustness / adversarial input (fuzzing).** ~700 panic sites, zero fuzz coverage. Feed the
  parse to render pipeline malformed `.tmd`: unbalanced `:::` fences, thousands-deep nesting, circular
  `{{< include >}}`, garbage YAML front-matter, pathological Unicode, truncated files. Every panic (which
  500s the dev server) or hang is a finding. Start: `cargo-fuzz`, or `proptest` + `arbitrary` over
  parse+render. *Stateful, solo. Recommended first.*
- **AP3: Concurrency / race conditions.** The server multiplexes a `notify` file watcher, websocket
  handlers, a warm ZMQ kernel, the exec pool, the `MAX_WARM_PAGES` LRU, and `_freeze/` writes across N
  browser clients. Rust stops data races, not logic races: save-while-executing, file-change-mid-build,
  two clients on one preview, concurrent freeze writes, eviction interleaving. Start: a stress driver plus
  a code read of shared-state ordering in `serve_site/exec_pool.rs` (respect the M6a freeze: observe, do
  not retune). *Stateful, solo.*
- **AP4: Cache-correctness (adversarial freeze).** The `_freeze/` cache promises "no stale hits, nothing to
  clear by hand." Nobody has tried to break that promise. Attack it: interpreter swap mid-session, partial
  write or crash during a freeze write, clock skew, an upstream cell edited then reverted, `#| cache: false`
  boundaries. One stale hit is a credibility bug for the core design. Start: enumerate the cumulative-hash
  inputs in `crates/server/src/freeze.rs` and construct a change that is NOT reflected in the key. *Stateful,
  solo. Recommended first.*
- **AP5: i18n / Unicode / multibyte sourcepos. RUN 2026-07-22** (findings:
  [2026-07-22-i18n-unicode-sourcepos-audit.md](2026-07-22-i18n-unicode-sourcepos-audit.md); folded into Open-work
  item 12). The starting hypothesis (byte-based sourcepos breaks Alt-click on any CJK/accent doc) was mostly
  *refuted*: the primary Alt-click locator uses the line only, block start columns are ~1, and all BMP text is
  correct. The real find was the editor LSP: it speaks Unicode scalars, comrak emits byte columns, and the TS
  companion is UTF-16 (three conventions, none of them UTF-16), diverging on astral characters (rename is a write path).
  A follow-up, done as a code read rather than the browser sweep first planned. Residual not yet chased: RTL
  layout, CJK line-breaking, non-ASCII heading-slug collisions.
- **AP6: Cross-browser / cross-platform.** CLAUDE.md mandates chrome-devtools MCP and development is
  Linux-only, so Safari, Firefox, and mobile browsers are effectively untested, as are macOS/Windows path
  handling, file-watch semantics, and kernel spawning. The vanilla-JS client and the deck engine are where
  these bugs hide. Start: drive the client through Firefox + WebKit (Playwright headless); grep the Rust for
  `\`-vs-`/` path assumptions and Linux-only syscalls. *Stateful, solo.*

### Tier 2: partially touched; a dedicated deep pass still pays

- **AP7: Deep accessibility of the output.** The polish rounds found chrome-level a11y holes; nobody has
  done a real screen-reader + keyboard pass over rendered docs, and especially the **deck as an interactive
  application** (focus management, `aria`, KaTeX a11y, live-region announcement on slide change). Overlaps
  item 11 pass (c) but goes deeper than one-hole-per-surface. *Stateful, solo.*
- **AP8: Determinism / reproducibility. RUN 2026-07-22, findings shipped + closed** (findings:
  [2026-07-22-determinism-audit.md](2026-07-22-determinism-audit.md); was Open-work item 15, now complete +
  removed). Covered
  BOTH halves (the read hunt AND the stateful rebuild-twice check, via the frozen binary). Result: a positive
  bill of health. Single-doc renders and a full multi-page site build are byte-identical across separate
  processes with fresh HashMap seeds, and determinism holds by construction (sorted discovery/listings/hover
  index, index-placed parallel builds, no time/random in output, cross-machine reproducible). One low finding,
  DET-1: no explicit end-to-end regression guard, so the manually-maintained property could silently regress.
- **AP9: Semantic-HTML / document-model correctness. RUN 2026-07-22** (findings:
  [2026-07-22-semantic-html-audit.md](2026-07-22-semantic-html-audit.md); folded into Open-work item 14).
  Result: a strong positive bill of health. Across 84 corpus renders + a site build the emitted HTML is
  structurally valid (no invalid nesting, no per-page duplicate ids, well-formed figures/tables/lists,
  labelled deck sections). The one finding is HTML-1: titled docs emit many sibling `<h1>` (title block +
  every `#`), breaking the single-root outline (the gated heading-demotion idea, now with evidence). Done as a
  render-probe + offline HTML-parse audit, no browser drive needed.
- **AP10: Internal codebase health.** Distinct from the feature-reduction audit: the ~700-panic surface
  (which `unwrap`s are reachable from user input?), module coupling, dead code, and a *coverage-hole* map
  (behaviors with zero test), which is different from the vacuous-test *quality* audit already done.
  *Code-read, fan-out-safe.*
- **AP11: Chaos / failure-injection UX.** Kill the kernel mid-cell, fill the disk during a build, drop the
  websocket, SIGKILL the server: how graceful is each degradation and what does the author actually see? DX
  touched error loops; nobody has injected real failures. (Note PA-B1 in item 11: the kernel-unavailable
  message already tells headless callers to click a Restart button that is not there.) *Stateful, solo.*
- **AP12: Offline-guarantee verification. RUN 2026-07-22** (findings:
  [2026-07-22-offline-guarantee-audit.md](2026-07-22-offline-guarantee-audit.md); folded into Open-work item
  13). The tool's own assets proved genuinely offline; the gap is author-introduced external references, which a
  `--out` build keeps with no diagnostic (proven by a build probe), plus preview lazy-loading mermaid from a CDN
  despite the vendored copy. Done as a code read + a frozen-binary build probe (no network capture needed). Not
  chased: whether built HTML leaks absolute local paths or author identity (the second sub-question here).

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

- **Live-executor mounts (F-04 full fix)** (was item 16 F-04; shipped 2026-07-22, landed to local `main`,
  unpushed): a mounted sub-project now serves through the **same live per-page path** as the root, so its
  `{python}`/`{r}` cells execute live in the host `preview` (not only in the static `build`). Engine is all
  in `serve_site/mod.rs`: `Project`/`MountPoint`/`ProjectKey` + pure `match_mount`/`resolve_project`/
  `classify_change` (unit-pinned) + **one `ExecPool` per project** (the frozen `exec_pool.rs` byte-unchanged,
  used once per project); a mount shares the warm pool only when its interpreter matches root, else cold-start.
  Each project owns its `_freeze` + websocket + hot-reload. Browser-verified on `/gallery/course/em.html`.
  Spec/plan: `docs/superpowers/{specs,plans}/2026-07-22-live-executor-mounts*`. Remaining (item 10, low): an
  automated live-HTTP serve test (the bin crate has no `reqwest`/`TcpListener` harness).
- **Structure-preserving, book-aware `read`** (was item 19; shipped + pushed 2026-07-22): the recurring
  cross-persona `read`-projection seam
  (folded items 16 F-02 + 17 F-03 + 18 F-01). Three pure arms in `render/text.rs::project_block`
  (`project_list` one line per `<li>` incl. ordered/nested; `project_steps` each `.scrolly`/`.step`
  narration its own paragraph; `project_inputs` `[input] label = value`), pinned by unit tests +
  `corpus/reader/text-projection.tmd` snapshot. Book-aware `read` in `query.rs`: `scoped_site_doc`
  auto-detects an enclosing `_site.yml` (walk-up, `.git`-bounded) and renders a page as the site does
  (`render_document_with_includes_scoped` + `Site::number_chapter` + `resolve_cross_refs`), so
  `@thm-elbo`→"Theorem 3.1", cross-page `@thm-consistency`→"Theorem 2.1"/"Chapter 2"; `read <dir>`
  projects a whole book (`===== rel (Chapter N) =====` headers, human + `--json`), `--run` on a dir
  rejected. Pinned by `crates/server/tests/read_book.rs`; `indexable_text` (Cmd-K) unchanged
  (arms live in `project_block`, which search doesn't call). Still open: item 16 F-03 (embed iframe-chrome
  leak in `read`) is a SEPARATE finding, NOT folded here.
- **2026-07-22 (late) backlog-clearing pass** (shipped 2026-07-22, on origin/main): **focus mode split from OS fullscreen**
  (`f` = calm column, `F`/menu = fullscreen; `03-focus-mode.js`); **Vite-user hint banner** (`log::keys_hint`,
  TTY-gated, points at the `◇` dev menu); **deck `footer:`/`logo:`** (`render::deck_overlay_html` +
  `DeckParts.deck_overlay`, corpus-pinned in `deck.tmd`); **per-book offline `<book>.zip`** (`server::zip`
  hand-rolled DEFLATE over the already-present flate2/crc32fast, topbar `<a download>` gated to the build via
  `page_chrome(downloads)`, `Site::archive_name`); **cross-page dup-label warning located** (`file:line:` at
  the redefining anchor via `content_lines_numbered`); **item 11 passes (b)-(e)** (see item 11). Owner
  rulings: DX16 skip, i18n defer, item-9 design-Qs documented (see "Decided against").
- **AP8-1 executed-output path scrub** (item 15, AP8) **shipped 2026-07-22** (branch
  `worktree-ap8-1-ipykernel-path-scrub`, now on origin/main): a cell's stream (matplotlib's Agg `UserWarning`, any
  `warnings.warn`, a `print(__file__)`) cited the kernel's per-process temp file
  `<tmpdir>/ipykernel_<PID>/<HASH>.py`, making builds non-reproducible + leaking a local absolute path into
  published HTML. Fix: a hand-rolled `scrub_kernel_paths` (no new dep, mirrors `strip_ansi`) normalizes that
  path — and the legacy `<ipython-input-…>` form — to a stable `<cell>` marker in the `Output::Stream` arm of
  `render_outputs` (`crates/server/src/kernel.rs`), before escaping; the `:<line>:` suffix is deterministic and
  kept. Language-agnostic (R warnings carry no such path — verified). Pinned by pure unit tests
  (`scrub_kernel_paths_normalizes_cell_source_paths`, `render_outputs_scrubs_nondeterministic_kernel_paths`) +
  a kernel-gated end-to-end `crates/server/tests/executed_output_reproducible.rs` (build the same warning doc
  twice under `TALIESIN_NO_CACHE=1` → byte-identical, no `ipykernel_` path); mutation-checked both ways.
  Completes item 15 alongside DET-1.
- **DET-1 reproducibility guard** (item 15, AP8) **shipped 2026-07-22** (branch
  `worktree-det1-determinism-guard`, now on origin/main): `crates/server/tests/build_reproducibility.rs` builds a
  feature-rich **kernel-free** site (listing + categories + Atom feed, cross-page `@thm-`/`@def-` xrefs,
  8 hover targets, site `url:` → sitemap + OG cards) twice in **separate processes at separate paths**
  (⇒ different HashMap seeds *and* `read_dir` order) and asserts **every** emitted file is byte-identical
  (not just `.html`, unlike `parallel_build_determinism.rs`), plus a non-vacuity test that the guarded
  aggregates (`search-index.js`/`hover-index.js`/`index.xml`/`sitemap.xml`/`llms.txt`/`og/*.png`) are
  populated. Mutation-checked: deleting the `entries.sort_by` in `Site::build_hover_index` diverges
  `hover-index.js` and fails it. Lands alongside AP8-1, completing item 15.
- **DX audit batch** DX1-DX15, DX18, DX19 shipped; **DX17(a)** shipped 2026-07-21 (below); **DX16 ruled
  skip** (Decided against); **DX17(b)** (headless `{js}`) **shipped 2026-07-22** — `read --run` drives a
  local headless Chrome (`chromiumoxide` 0.9, `default-features = false` so no fetcher/openssl; tokio
  1.52/edition-2024 clean) over the built page and projects each `{js}` cell's outcome. Pure
  `classify_js_node`/`JsOutcome` (`headless_js.rs`), core interleave `body_text_with_js`
  (`render/text.rs::project_with_js`), a `detail` field on `read --format json`'s cells (skip-if-none, so
  python/r JSON stays byte-identical), gated + optional (no Chrome → `[js: skipped (chrome unavailable)]`,
  exit 0), observation-only (no reactive re-run, no `{js}` freeze write). Pinned by
  `corpus/agent/executed-read-js.tmd` + the Chrome-gated `read_run_js.rs` (`TALIESIN_REQUIRE_CHROME`
  canary) + pure unit tests; `TALIESIN_JS_TIMEOUT` (default 10s) settle budget. **The whole DX audit is now
  complete.**
- **Editor DevX (VS Code companion) E1-E6 shipped 2026-07-21; E7 (`taliesin lsp`) shipped 2026-07-22 —
  the whole initiative is complete** (audit
  [2026-07-21-vscode-devx-audit.md](2026-07-21-vscode-devx-audit.md);
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
- **E7 `taliesin lsp` (editor-agnostic language server over stdio) shipped 2026-07-22, all capabilities**
  (`crates/server/src/lsp.rs` + `lsp_nav.rs` + `lsp_outline.rs` + `lsp_complete.rs`, `lsp-server`/`lsp-types`;
  specs `docs/superpowers/specs/2026-07-2{1,2}-e7-lsp-*.md`). `textDocumentSync: FULL` + a `HashMap<Url,String>`
  store; **live diagnostics** (`check::buffer_diagnostics` → `to_lsp`); **definition** (`@xref`/`[@cite]`/`{{<
  include >}}` via `lsp_nav`); **documentSymbol** (heading outline via `lsp_outline::outline`); **hover** (xref
  label+number from a live-buffer render's `xref_numbers`, key docs + `.bib` entry via `bib_entry_text`);
  **completion** (7 cursor contexts via `lsp_complete::detect_context` + `vocab` + `render_buffer`);
  **codeAction** (one-click quick-fix from a diagnostic's precise `data.replacement`); **rename** + prepare
  (`lsp_nav::{anchor_at, anchor_occurrences}` rewrite an xref anchor's definition + all `@`-refs in one
  `WorkspaceEdit`, gated to `is_xref_anchor` ids). Porting the companion itself to `vscode-languageclient` is
  a separate, still-open, later item (not scoped here).
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

- **2026-07-22 rulings** (owner, this session): **DX16 update-nudge = SKIP** — a version check is network
  egress that undercuts the offline-first identity; drop it (was item 7). **Cross-ref labels i18n = DEFER** —
  no corpus doc demands it and full i18n is a real scope question; minimal-config says don't add speculative
  config (was item 8; revive with a corpus pin + a real ask). **Item 9 design-Qs documented as-is** (owner
  chose only the Vite banner, which shipped): the deck serif/sans inversion (`deck.css`), no `//| uses:`
  alias (vocab sprawl), and the callout-namespaced/theorem-bare asymmetry all stay as intentional, not
  bugs. **Deck presenter tools** (one-command publish / laser-spotlight / auto-advance) considered and NOT
  selected — revive on a real speaker ask.
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
