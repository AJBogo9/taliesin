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
shipped"). **Most of the backlog has already shipped** (DX, PMF, polish, machine-facing, corpus-coverage
and reduction audits are all closed; a second **2026-07-22 polish round** — a browser sweep + 4 code auditors —
reopened a small P3 hardening/a11y tail, item 11, whose **pass (a) — design-system single-source + the
dark-mode WCAG-AA fix on filled chrome controls — shipped 2026-07-22**). What is actually open is small; it is
ranked below by product impact.

## Next session: start here

Tree is green across all gates; `origin/main` is current. Three clean entry points, pick by appetite:

- **Continue the 2026-07-22 polish audit (item 11) — small + safe.** Pass (a) shipped; the next
  highest-value slice is **pass (b) scaffold-completeness, PA-H2** — the audit's one "high" finding:
  listing/section pages (`/blog`, `/publications`, `/projects`) emit **no `<h1>`** and open at H2/H3 (SEO +
  heading-nav). It demotes heading levels, so it **touches the body-HTML snapshot tests** — a bigger, more
  visible change than pass (a). Then (c) a11y announce/focus holes, (d) CLI/diagnostics, (e)
  reduced-motion+print. Detail: [2026-07-22-polish-audit.md](2026-07-22-polish-audit.md).
- **Or the one High-impact feature, DX17(b) headless `{js}` (item 1) — large, needs a ruling first:** it
  adds a headless-Chrome dependency (`chromiumoxide`) to the offline tool, so it wants its own spec/plan and
  an owner sign-off on the new dep before coding. Design is already drafted (see item 1).
- **Or run one of the twelve queued *audit perspectives* (new "Audit perspectives" section below):**
  proactive, findings-generating angles the prior rounds structurally could not see (perf, fuzzing,
  concurrency, cache-correctness, i18n/sourcepos, cross-browser, a11y, determinism, semantic HTML,
  codebase health, chaos, offline-proof). Each is a fresh session that writes a dated findings doc and
  feeds build-ready items back here; the author has credits queued for exactly this. Recommended first
  three: AP2 (fuzzing), AP4 (freeze cache), AP5 (multibyte sourcepos).

Everything else open is P3/gated (items 5–10) or demand-driven (Tier 3). Working method is in "Standing
constraints": branch per feature, verify by mutation, browser-verify, ff-merge locally.

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

### C. Low / hardening (P3)

7. **DX16: update-available nudge** (async, boxed, `NO_UPDATE_NOTIFIER` opt-out). S, net-new. *Weigh
   against the offline invariant first (it implies a network check).*

8. **Cross-reference labels are English-only** (§2 #3): an i18n scope question, not a small defect. The
    hardcoded English const table is `cite/render.rs:15-21`, and `lang` appears **zero** times in that
    file, so there is no localization seam yet. `lang:` correctly sets `<html lang>`; the "promise" of
    translated labels was never real. **No corpus doc demands it.**

9. **Remaining design questions** *(owner ruling first, low impact)*: deck inverts the page serif/sans
    logic (`deck.css:705-711`), accept+document or unify? · add a `//| uses:` alias for the consumer
    `//| input:` (weigh vocab sprawl)? · callout kinds are namespaced but theorem kinds are bare,
    document or reconsider? · a Vite user pressing `r/o/u/c/q` or `h` gets silence now that interactivity
    moved to the browser dev menu, so one banner line pointing at the `◇` menu.

10. **Reliability / test-infra long tail** (P3, dev-facing):
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

11. **2026-07-22 polish-audit follow-ups** (P3 hardening + a11y + "feels finished"; detail:
    [2026-07-22-polish-audit.md](2026-07-22-polish-audit.md) — ~55 `PA-*` findings from an empirical browser
    sweep + 4 read-only code auditors; [AUDITS.md](AUDITS.md) records the round). **PA-H1 (standalone deck build
    shipped no favicon → a `/favicon.ico` 404 + blank tab) already landed** 2026-07-22 (`dc58aa9`, pinned by
    `deck_offline_build::built_deck_carries_a_favicon`). The rest grind as **5 passes**, each a branch → corpus-pin
    (where behavioral) → browser-verify: **(a) design-system single-source — SHIPPED 2026-07-22** (branch
    `polish/design-system-single-source`): `site.css` radii/durations/hover-shadows now route through
    `--tali-radius-*` / `var(--tali-dur)` / `--tali-shadow-md`, the cite-this "Copied!" + deck speaker/share active
    buttons take `--tali-accent-fill` (dark **5.59:1**, was ≈2.3:1 — clears WCAG AA), every deck control gets a
    `:focus-visible` ring, listing cards get keyboard-focus parity with hover, and sepia gets its own search-`<mark>`
    (PA-C1/C2/C3/D1/F1/F3/S1/S2/S4/C4). Pinned by `render::tests::{filled_chrome_controls_use_the_aa_accent_fill_not_raw_accent,
    every_interactive_deck_control_gets_a_focus_visible_ring, sepia_search_mark_keeps_body_text_readable,
    listing_card_gets_a_focus_visible_affordance}` + browser-verified light/dark/sepia. **Residual (low, deferred):**
    PA-F2 (a `--tali-scrim` token folding 3 divergent overlay alphas), PA-C5 (per-slide-bg hex drift-lock),
    PA-F4 (px↔rem breakpoints), PA-S3 (base.css's own `.15s` uses). **(b) scaffold completeness**
    — listing/section pages (`/blog`,`/publications`,`/projects`) emit **no `<h1>`** and start at H2/H3 (SEO +
    heading-nav; PA-H2, M), dates are `<span>` not `<time>` and a listing is a `<div>` not a list (PA-M1). **(c) a11y
    announce/focus holes** — one missing `aria-live`/focus-trap/roving-tabindex per surface (lightbox gallery step
    silent to AT, PA-A2; etc.). **(d) CLI/diagnostics** — the kernel-unavailable error tells headless `build`/`read`/CI
    to "click Restart kernel" (a button that isn't there; PA-B1, `exec.rs:333`), `check` human diagnostics are
    uncoloured (PA-B2), residual `--help` drift. **(e) reduced-motion + print** — honoured in the reader enhancers
    but **not the preview client** (PA-B6/B7), and printed links lose their URL (no `a[href]::after`; PA-P1). *Gating:
    mostly S/P3; a couple M (PA-H2 listing `<h1>` + heading-level demotion touches snapshots; the token pass). Verify
    each against source (entries rot) and by mutation. Owner design-Qs (deck copy-button, card whole-`<a>`) parked in
    the doc, not build-ready.*

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
attacked: **AP2 (fuzzing), AP4 (freeze cache), AP5 (multibyte sourcepos).**

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
- **AP5: i18n / Unicode / multibyte sourcepos.** Untouched, and aimed straight at the invariant: sourcepos
  is byte-based, so any char-offset assumption means Alt-click-to-source silently misfires on any doc with
  CJK, accented Latin, or emoji. Plus RTL (Arabic/Hebrew) layout, CJK line-breaking, non-ASCII heading-slug
  generation. Start: a corpus doc mixing CJK + emoji + combining marks, then Alt-click every block and check
  the editor cursor lands on the right character. *Stateful (browser), solo. Recommended first.*
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
- **AP8: Determinism / reproducibility.** Does one `.tmd` produce byte-identical HTML twice? (the
  `body_html_snapshots` drift is a symptom.) Hunt hashmap-iteration order, timestamps, random ids, plot
  float noise. Matters for caching, diffs, and any future content-addressed story. *Read half is
  fan-out-safe; the rebuild-twice check is stateful.*
- **AP9: Semantic-HTML / document-model correctness.** Beyond "does it look right": heading hierarchy,
  sectioning, figure/caption association, table semantics, W3C-validator conformance. Is the document
  *model* correct, not just the pixels? Overlaps PA-H2 (item 11 pass b). *Code-read, fan-out-safe.*
- **AP10: Internal codebase health.** Distinct from the feature-reduction audit: the ~700-panic surface
  (which `unwrap`s are reachable from user input?), module coupling, dead code, and a *coverage-hole* map
  (behaviors with zero test), which is different from the vacuous-test *quality* audit already done.
  *Code-read, fan-out-safe.*
- **AP11: Chaos / failure-injection UX.** Kill the kernel mid-cell, fill the disk during a build, drop the
  websocket, SIGKILL the server: how graceful is each degradation and what does the author actually see? DX
  touched error loops; nobody has injected real failures. (Note PA-B1 in item 11: the kernel-unavailable
  message already tells headless callers to click a Restart button that is not there.) *Stateful, solo.*
- **AP12: Offline-guarantee verification.** The tool *claims* fully offline (bundled KaTeX/fonts/JS). Prove
  it: does any built page or live preview make an external request, and does built HTML leak absolute local
  paths or author identity? The security pass touched network egress; this is the positive proof, not an
  assumption. *Code-read + a network-capture check; mostly fan-out-safe.*

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
