# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git + [AUDITS.md](AUDITS.md) +
> [ROADMAP.md](ROADMAP.md); delete an item when it lands, don't leave a `[x]`. The "already shipped"
> list near the bottom is the compact anti-rot guard (do not re-add / re-scope), not a changelog.

## State (2026-07-25)

**Branch `backlog/backlink-context-and-resume`: 15 commits, NOT pushed**, stacked on
`backlog/book-outline-drawer` (5 commits), itself off `origin/main` at `994bcba`. **Do not trust
that SHA** (see Git under "Standing constraints"); verify with `git log --oneline origin/main..HEAD`.

Five code batches landed 2026-07-25 (the hardening set, SKIM-1/2/3a, the P3 residual batch, the
book-wayfinding batch, the backlink-context + resume batch). What they shipped is in git and
[AUDITS.md](AUDITS.md); the lessons worth carrying forward are folded into "Standing constraints"
below. Gates at the last code landing, **re-run before trusting them**: 1481 tests / 0 fail across
88 binaries with all three gates and `--test-threads=1`; `cargo fmt --check`,
`clippy --workspace --all-targets -D warnings` and both JS `tsc` gates clean; `check` clean on
`corpus/tarn`, `docs/guide`, `docs/internals` and `site`.

Three audits then ran, **findings only, no code**: **AP7** (accessibility) = item **34**, **AP3**
(concurrency) = item **35**, **AP11** (chaos) = item **36**.

**Build-ready now, both measured and unblocked:** **AP7-1** (37 of 51 book pages emit a skipped
heading level while `check` prints "no problems found") and **AP3-1** (a page with no code cells
hot-reloads in 0.11s alone, 12.15s when an unrelated page is executing). Each item records its own
fix-shape constraint; read it before starting.

**Owed by the author, not by a session:** the in-editor click-to-source round-trip check from the
naming purge (Task 8 Step 5). The companion was repackaged and reinstalled and the relay harness
passes both directions, but nothing automated covers the real editor round-trip.

**One audit perspective is left: AP6 (cross-browser)**, stateful/solo. Three non-AP lenses are also
proposed and unrun (see "Audit perspectives"), and they are probably the better use of a session:
AP11 returned only one low-medium finding because the failure paths are genuinely well-built.
Working method is in "Audit perspectives": a dated findings doc, a row in
[AUDITS.md](AUDITS.md)'s round index, and the build-ready findings filed into "Open work" under
their own prefix. No ruling requires the next session to take one.

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
  before trusting a total. **Flake status, re-measured by AP3 over 13 full `--bins` runs
  (2026-07-25):** the two `exec::tests` "concurrency-race" tests failed **0 times**, so the recorded
  "~2 runs in 3" is wrong and `ETXTBSY` was never reproduced; what still flakes is
  `kernel_executes_..._runaway_cell` at **1 run in 13**, despite item 10 recording it as fixed and
  deterministic. So a red `exec`/`kernel` probe is worth one re-run, but is no longer a reason to
  reach for `--no-verify` by default. See item 35.
- **Git:** do not trust a SHA written in notes. Check `git log --oneline origin/main..main` for what is
  unpushed and `git reflog show origin/main` before believing any "not pushed" claim; the author pushes
  mid-session with no signal here.
- **How this file lies to you:** entries rot (the author pushes mid-session; a scoped prune leaves the
  rest looking freshly reviewed). Before picking an item, **grep its named symbol/flag in source** and
  prefer measuring the running product over reading this file. Trust an item's *symptom*, never its
  cause, line number, or stated cost (all three have rotted). Verify a fix by **mutation** (restore the
  bug, watch the named test fail), not by a green suite. Grep traps live here: a bare word matches
  prose, `grep | head` reports head's exit code, quote `--include='*.tmd'` in zsh.
  **Commit before mutation-testing:** `git checkout -- <file>` on an *uncommitted* file restores from
  HEAD and destroys the working implementation (it did, twice).
- **What the test net structurally cannot see.** The dogfood books (`docs/guide`, `docs/internals`)
  are NOT in the regression net, so any shape only they have is invisible to a green suite. Three
  gaps were measured (enumerated, not grepped) and each one hid a real bug: **(1)** every corpus book
  chapter opened `# Title` with no front-matter `title:`, so heading demotion went unexercised while
  32 of 32 dogfood chapters use it; **(2)** no book in the repo has an **include-built chapter**, so
  any rule reading a chapter's *source* (word counts, `skim`, prose lints) passes vacuously;
  **(3)** no corpus book keeps a chapter in a **subdirectory**, so depth-relative emission
  (`{up}` hrefs, `../index.html`) is the empty string everywhere the suite can look. (2) and (3) are
  now minted in temp dirs by `site/skim.rs` and `tests/book_landing_toc.rs`. **When a defect is
  reported on a dogfood page, first ask whether the corpus has that shape at all.**
- **`corpus/tarn` is the fixture for scale-sensitive work** (12 numbered chapters, 3 parts + a nested
  part) and deliberately carries the shapes the rest of the corpus lacks: a titled chapter, a
  `###`-rooted one, one with a body `# H1`, one below `MIN_TOC_HEADINGS`, an over-cap section whose
  distinctive term sits in its last paragraph, two `{.definition}` blocks, an unnumbered appendix.
  **Use it instead of minting a fixture.** It is a *documentation* book, not a scale fixture: do NOT
  grow it toward 200 pages and do NOT mint `corpus/longbook` (the walker renders every corpus doc on
  every `cargo test`).
- **The inlined-asset needle trap** (bit three times in one batch): every page inlines the whole
  CSS + enhancer-JS payload into its `<head>`, so **any new class name, `data-` attribute or
  user-facing string is present in the HTML of every page whether or not that page renders the
  feature.** A whole-page `contains("…")` is satisfied by a page rendering none of it. **Needle the
  full emitted tag, or slice the block out first.**
- **Calibrate a new lint against real output before writing it.** Measuring the proposed
  `TAL-SHAPE-*` rules over all 14 site projects killed four of their own prescriptions, including
  the most valuable one (it fired on 11.8% of the corpus, essentially all false positives) and one
  whose stated justification did not exist in the tree.

## Open work (priority order: product impact)

Ranked highest user/product value first. Impact is not the same as buildability, so each item carries a
gating tag: a high-impact item can still be frozen or need a ruling.

**Three audits on 2026-07-25 refilled this list: items 34 (AP7), 35 (AP3) and 36 (AP11).** Before them it was
genuinely empty of buildable, unruled work, which is why a session was ruled to an audit: A's single
item is an owner decision ruled deferred; both of B's need a device or a demand signal; and C was
down to **24**'s two owner calls, item **30** (writing, not code), and P3 residuals that each carry
their own blocker (**17**'s F-01 needs a vendoring decision, **17**'s F-02 and **18**'s F-03 are WAI,
**12** is demand-driven, **29**'s T2 is "only if you are already in there", **11**'s Semantics bullet
needs a CSS-grid + filter-JS restructure).

**The two picks are AP7-1 and AP3-1**, the only unblocked, measured-defect work in the list. Both
have a fix-shape constraint recorded in their item; read it before starting.

### A. High impact (build first)

25. **Pre-public release checklist: one owner decision left** (detail:
    [2026-07-17-security-release-audit.md](2026-07-17-security-release-audit.md)). The five code
    items shipped 2026-07-25 (`dos-pages`: a ws `?page=` the site cannot resolve no longer allocates
    a never-evicted `PageState`; **DEP-03**: mermaid vendored at 11.16.0 with an explicit
    `securityLevel: 'strict'`, `THIRD_PARTY.md` updated and now drift-locked by a test that reads the
    version out of the bundle itself; `dos-rich`: an 8 MB cap on rich-output bytes, the axis the
    stream-byte and output-count caps both missed; `dos-ws-size`: `max_message_size` on both ws
    upgrades; **CMD-01**: the warm pool logs its resolved interpreter like the cold path already did).
    **What remains is not a task:**
    - **oss-4 — RULED 2026-07-25: deferred, and the public flip with it.** The owner is not
      going public yet ("I'll do it at the end of summer; before that I want to hone the tool
      to its final form"). So this is not a task and not a blocker: nothing here gates any
      other work. Re-ask when a flip date is actually set. The question when it is: whether to
      prune `notes/` + `docs/superpowers/`. No secret is exposed (the `--host` token design doc
      discloses only a per-session UUID mechanism), but it is a curated bug roadmap.
    **Verified NOT open, do not re-scope:** `SECURITY.md` exists, the tracked `/home/bogo` paths are
    scrubbed, and PT-1 / PT-2 / NET-1 / OUT-1 / DEP-01 / DEP-02 all shipped 2026-07-17. Refuted by the
    audit and not worth revisiting: `dos-yaml` (libyaml rejects the alias bomb in ~30 ms — the guard is
    in the C library, so grepping our source for it correctly finds nothing) and NET-3
    (non-constant-time token compare).

### B. Medium impact

4. **Deck engine mobile polish** (P2): mobile pinch/pan + touch gestures (they matter for the phone-feed
   deck mode; hard to verify without a device); drop `fitSlide` from the resize path (needs a lazy
   fit-on-show refactor first). *(The desktop trackpad half shipped 2026-07-24 — pinch / ctrl+wheel-down
   opens the overview map, with a 250 ms hysteresis. What that left behind is all shipped or ruled —
   see "Deck-motion: the whole item is closed" under "Decided against", formerly item 28.)*

2. **Deck presenter tools** *(owner deferred 2026-07-22 — NOT selected this round)*: one-command deck
   publish (Share QR still encodes `localhost:PORT`), a presenter laser/spotlight, auto-advance. The
   `footer:`/`logo:` threading from this item **shipped** (see "Already shipped"); the presenter pieces
   were considered and left for later. Revive only on a real speaker ask.

### C. Low / hardening (P3)

36. **AP11 chaos finding** (detail:
    [2026-07-25-ap11-chaos-audit.md](2026-07-25-ap11-chaos-audit.md)). One finding; the round's
    main result is a **positive bill of health** on failure handling, listed in the doc so it is not
    re-audited (corrupt `_freeze` self-heals in all three corruption shapes; an unwritable `_freeze`
    warns and completes; an **unwritable output dir exits 1 with nothing half-written**; a missing
    interpreter renders cells as source with a precise page diagnostic and **fails under `--strict`**;
    a killed server leaves the client in a visible `reconnecting…` state with a boot-id-forced
    re-mount). **PA-B1 was already fixed**, so AP11's only seed is closed.
    - **AP11-1 (low-medium, S): a missing interpreter is reported to the console as an author code
      exception.** With a bogus `TALIESIN_PYTHON`, the build logs `cell error … code cell raised an
      uncaught exception; its traceback is baked into the output`. Both claims are false: the kernel
      never launched and no traceback exists. **Cause:** `build.rs:380 is_cell_error_output`
      classifies a crashed cell purely by HTML shape (`<div class="tali-output"` containing
      `class="tali-error"`), and the kernel-unavailable diagnostic is emitted with exactly that
      shape, so `cell_error_message` (`build.rs:478`) asserts an exception unconditionally. Reaches
      `--format json` too, via `cell_error_diagnostics` (`build.rs:493`). The rendered **page is
      correct**; only the console and the structured diagnostics are wrong. Matters because a wrong
      interpreter path is plausibly the most common setup failure. **Fix shape:** distinguish at the
      source of truth (a distinct class on the unavailable diagnostic, or an executor-set marker),
      not by HTML shape.

35. **AP3 concurrency findings** (detail:
    [2026-07-25-ap3-concurrency-audit.md](2026-07-25-ap3-concurrency-audit.md)). The round **refuted
    every race it went looking for** (see the perspective entry); what it found is a queueing
    property. Read the "Verified sound" list before touching any of this.
    - **AP3-1 (medium-high, M): one slow cell anywhere stalls hot reload everywhere.** Measured on a
      two-page preview with a warm pool: a **cell-free** page's trivial prose edit lands in **0.11s**
      alone and **12.15s** (110x) when an unrelated page is 1.2s into a 12s `{python}` cell.
      `spawn_builder` (`serve_site/mod.rs:1006-1053`) is one task consuming one channel for the whole
      server, **root and every mount alike**, awaiting each `build_page_guarded` to completion. It
      serializes on the wrong predicate: a page with no cells needs no kernel and is queued behind
      kernel work it will never use. Matters concretely because the marketing site `mounts:` both
      dogfood books and the corpus has genuinely slow cells. **Do not "fix" by simply parallelising
      the builder:** serialization is what makes the shared warm pool and the task-owned `ExecPool`
      race-free (and `ExecPool` is under the M6a freeze), so the safe shape is a bypass for
      cell-free/no-exec rebuilds, not concurrent executors. Preview-only; degrades latency, never
      correctness.
    - **AP3-2 (low, observation not defect): the build queue has no dedupe.** `build_tx` is a bare
      `UnboundedSender` with no in-flight tracking, so every 80ms debounce window enqueues another
      build for an open page. **Measured before filing it as a bug, and the visible cost is nil:** 5
      distinct edits during one 12s build produced **1** `update` line and the correct final state,
      because builds 2..5 render byte-identical HTML and the block diff emits no ops. The residual is
      wasted CPU (per AP1, two full-site passes each), which was **not** measured. Only becomes real
      if AP3-1 is fixed by parallelising.
    - **AP3-3 (low): `kernel::tests::kernel_executes_state_errors_and_interrupts_runaway_cell` still
      flakes 1 run in 13.** Item 10 records it as fixed 2026-07-25 and deterministic (per-kernel
      `cell_cap` replacing `OnceLock` memoization of `cell_timeout()`). The rate is clearly far lower
      than before but is not zero, so either the cap has a second order-dependence or the interrupt
      path has an unrelated timing edge. **The assertion text was not captured** (the failing run was
      under a summary-only harness); loop the detail-capturing harness to catch it.
    - **Correction to item 10, verified:** its "two `exec::tests` concurrency-race tests fail ~2 runs
      in 3 in a full `--bins` run" is **0 failures in 13 full runs** with all three gates at full
      parallelism. The `ETXTBSY` hypothesis was never reproduced, so do not spend a session on it;
      note also that `probe_interp_id` memoizes only an *answer*, so a failed ask is genuinely
      retried and `interp_id_settled`'s 5s loop already absorbs a transient exec refusal.

34. **AP7 accessibility findings** (detail:
    [2026-07-25-ap7-accessibility-audit.md](2026-07-25-ap7-accessibility-audit.md)). Five findings; the
    doc also records what came back **sound** (deck `inert`, KaTeX MathML, tabsets, focus rings) and
    three false leads, so re-derive from it before doubting any of this. **AP7-1 is the only one that
    is a defect on shipped reader-facing pages.**
    - **AP7-1 (medium-high, S+M): 37 of 51 book pages emit a skipped heading level while
      `check` prints "no problems found".** Measured across `docs/guide` + `docs/internals` +
      `corpus/tarn`: 35 pages `h1→h3`, 2 pages `h1→h4`; `h2` is empty on essentially every chapter of
      both dogfood books. **Two independent causes, both re-derived from source:**
      (1) `render/mod.rs:2490 demote_heading_html` is an absolute `+1`, right for a `#`-rooted chapter
      and wrong for the `##`-rooted house style, while the build's TOC already windows relative to
      the *shallowest heading present*, so the two disagree; (2) `diagnostics/a11y.rs:211` starts
      `prev = 0` and `helpers.rs:47 heading_level` needs the block html to **start with** `<hN`, but
      the title block is `blocks[0]` as `<header class="tali-title-block">…<h1>` (`render/mod.rs:1133`),
      so the page's only `<h1>` is skipped and the largest jump on the page is never compared.
      **Sequencing matters:** fixing (2) alone is cheap but turns 37 green pages red rather than
      fixing them; fixing (1) changes emitted levels, which `site/chapter.rs` numbers *post*-demotion,
      so the relative-demotion fix and `ChapterNumbering`'s per-site base must move together or
      `@sec-` refs drift. Needs a minted pin: there is no `crates/core/tests/a11y*.rs`.
    - **AP7-2 (medium, S): the reactive `{js}` graph rewrites the document silently.** Keyboard-driving
      a `{{< input >}}` slider on built `corpus/reactive/inputs.tmd` changed six output regions
      (`k=3 n=20` → `k=8 n=20`) with **every** live region empty; no `.tali-js-out` carries `aria-live`
      or `role` (7 of 7), and `tali-js.js` has no `aria-live` at all. The control itself is correct
      (real `<label for>`, keyboard-operable); only the consequence is unannounced.
    - **AP7-3 (medium, M): `.scrolly` and `.code-walkthrough` carry no a11y semantics at all.**
      Measured: 0 focusable steps, 0 steps with `aria`/`role`, 0 live regions, `null` root role, for
      both. `scrolly.js`/`walkthrough.js` contain no `keydown`/`tabindex`/`role`/`aria`. The step
      prose reads fine linearly; what is never conveyed is the **stage** each step drives (no
      `aria-controls`/`aria-describedby`), and its state advances only as a consequence of visual
      scrolling. *The audit did not manage to drive a state transition headlessly (the known
      scroll-testing gotcha), so it reports the semantics, not the flip timing.*
    - **AP7-4 (low-medium, S): a preview block swap strands keyboard focus.** Measured against a
      live preview: focus **inside** the edited block → `<body>` (next Tab restarts at the top of the
      document); focus in an **unrelated** block survives, so the block-level diff is already doing
      its job. Nothing announced either way. `client.js:1276` `replaceWith` / `:1312` `remove` have no
      focus handling. **Preview-only** (a built page has no swap), so this costs an author who works
      keyboard-first or with AT, not a reader.
    - **AP7-5 (low, S): the in-page TOC is tab stop 56 of 62** on a chapter, after all 48 content
      stops, though it is a sticky sidebar visible the whole time. Screen-reader users are unaffected
      (`role="doc-toc"` is exposed as a landmark, verified in the full a11y tree); this lands on
      keyboard-only users not running AT. The skip link goes to `#tali-main` only.

24. **SKIM-3 residue: two owner calls, no build-ready task** (P3; detail:
    [2026-07-24-skimmability-audit.md](2026-07-24-skimmability-audit.md)). The severity floor,
    `taliesin skim`, the `TAL-SHAPE-*` lints and four of the five independent-medium items all
    shipped 2026-07-25. What is left is not code:
    - **`book-breadcrumb`, a static "Part, Chapter" ribbon: OWNER CALL.** It adds a fourth
      persistent top element, and the dwell-time evidence says the first viewport is the screening
      surface. The audit itself downgraded it to "cheap and mildly orienting" and notes it must be
      argued as a reversal of D114's "no breadcrumbs", not as an unexamined gap.
    - **`section-extents`: OWNER RULING.** The DOM has no section boundaries (zero `<section>`
      wrapping content headings on 17 of 19 built guide pages; repo-wide `<section>` comes only from
      `render/deck.rs` and the footnotes block), which blocks four proposals.
      **Recommendation: option (b), a `data-section-end` marker computed from the walk
      `lsp_outline.rs` already does** (purely additive, invisible to the diff and the corpus
      invariants). Option (a), a real wrapper, would also unlock `content-visibility: auto` and
      sticky section headings, but it changes the parent/child shape the incremental diff mounts,
      which is a design question. Pin: `corpus/layout/structure.tmd` (named by `FEATURE-IDEAS` #26,
      still does not exist).
    **Invariants for anything in this area:** the finding lands in the CLI or the editor and the
    **author** edits the `.tmd` (no preview gesture, no auto-fix, no write-back); no LLM anywhere
    (byte-identical build output is actively pinned, and `include_str!` cannot carry model weights);
    zero new YAML keys.
    **Deferred, do not schedule:** a reading-density fold (three unbuilt prerequisites, premise
    measurably overstated); `content-visibility: auto` (needs option (a)); the `:~:text=` half of
    deep links (669 of 876 dogfood paragraphs contain inline code, so fragments miss exactly the
    identifier queries they exist for; the `?h=` half ships alone); `changed-since`; read-aloud
    (out on cost, not principle).
    **Killed by verification, do not re-scope:** section hover previews (built and deleted at
    `318f22f`, pinned by three tests), a TOC entry budget (the depth window is already relative),
    margin footnotes (two real footnotes exist in the whole repo), and `taliesin split` (it would
    repair 0 references on the chapter it was designed for).
    **Note for the author, no code in it:** roughly half the measured problem is *content*. Zero of
    37 dogfood pages set `description:`, 8 xref links exist across 19 chapters, 0 backlink lines
    render, and `docs/internals` is 60,208 words with zero `{.definition}` blocks. A glossary, a term
    index and a float digest all produce near-empty output until an authoring pass happens.


17. **Demand-probe (OSS docs-maintainer, persona #2) findings** (P3, in-scope; detail:
    [2026-07-22-corpus-demand-probe-docs-maintainer.md](2026-07-22-corpus-demand-probe-docs-maintainer.md)).
    A realistic library documentation site (`corpus/tarn/`, corpus-pinned by `tarn.rs` + a `/gallery/tarn`
    marketing-site exhibit) probed the tabsets × full-text-search × API-reference cluster. The *stacked*
    interactions (book × Guide/Reference parts × two `.panel-tabset`s per page × `.code-walkthrough` ×
    guide→reference `.tmd#anchor` cross-page links × chapter-scoped `@sec-` refs × Cmd-K search spanning the
    book incl. tabset-hidden content × version/deprecation callouts × mount) ALL work — 0 interaction-bugs.
    Four P3 findings, all on secondary surfaces. **Highest-placed of the P3 demand-probe set because F-01 is
    the only one a reader sees on the page:**
    - **F-01 (friction, P3) — SYMPTOM REAL, RECORDED FIX WRONG (re-derived 2026-07-25).** The symptom
      stands: `powershell` and `ps1` both render as unstyled plain text with a `TAL-CODE-LANG` warning
      (`bash` highlights fine). But the filed one-liner cannot work: **`two-face` has no PowerShell
      syntax at all.** Enumerated, not grepped — its set is 199 syntaxes and PowerShell is not among
      them, and no feature flag adds one. (The "ordering trap" the old entry warned about is moot too:
      `resolve()` already consults the bundled set first and falls back to the extras, so a syntax in
      either set would already resolve.)
      **A real fix means vendoring a grammar**, which is a decision, not a drive-by: the upstream
      PowerShell/EditorSyntax grammar is a 43 KB `.tmLanguage` plist (needs syntect's `plist-load`
      feature, which is not enabled) and its `LICENSE.txt` 404s, so its terms need establishing before
      anything is vendored — particularly with the repo about to go public (item 25). Left to the
      author with that groundwork done. A cheap alias to another language is NOT an option: it would
      mean confidently wrong highlighting instead of honestly absent highlighting.
    - **F-02 (WAI, no action):** the a11y heading-skip lint fires on a `#` title + flat `###` API entries;
      the linter is correct (demote entries to `##`). Recorded as an authoring-DX nuance, not a defect.

10. **Reliability / test-infra long tail** (P3, dev-facing):
    - **R cold-kernel orphan residual:** IRkernel has no `ParentPollerUnix` equivalent, so R cold
      kernels still orphan on ungraceful parent death; there is no clean fix (PDEATHSIG is the only
      lever and is hazardous), and R is rarely the cold single-doc path. `kernel.rs`. (The
      warm-pool, cold-Python and `/tmp`-sweep halves all landed.)
    - **`mounts:` live serve/discovery: only an automated live-HTTP test is missing** (the live-executor-mounts
      branch LANDED): the F-04 work reworked `serve_site` mount discovery/serving and unit-pins
      the pure `match_mount`/`resolve_project`/`classify_change` helpers, and live mount serving is
      browser-verified. What remains is only the bin-crate gap of an end-to-end live-HTTP serve test (no
      `reqwest`/`TcpListener` harness). Low-value (mounts are preview-only), demand-driven.
    - **Test flakes, re-measured by AP3 over 13 full `--bins` runs (2026-07-25). The numbers in this
      bullet's previous version were wrong:** the two `exec::tests`
      (`a_successful_probe_pins_the_freeze_key_format`,
      `a_failed_interp_probe_is_not_memoized_for_the_process_lifetime`) were recorded as failing
      "~2 runs in 3"; they failed **0 of 13**. The `ETXTBSY` write-then-exec hypothesis was never
      reproduced, so **do not spend a session on it**; note also that `probe_interp_id` memoizes only
      an *answer*, so a failed ask is genuinely retried and `interp_id_settled`'s 5 s loop already
      absorbs a transient exec refusal. What **does** still flake is
      `kernel::tests::kernel_executes_state_errors_and_interrupts_runaway_cell`, **1 run in 13**,
      even though it was fixed 2026-07-25 (a per-kernel `cell_cap` replacing `OnceLock` memoization
      of `cell_timeout()`, which also dropped the `--bin` suite 155 s → 49 s). So either the cap has
      a second order-dependence or the interrupt path has an unrelated timing edge. **The assertion
      text has never been captured**; loop a detail-capturing harness to get it. Tracked as AP3-3 in
      item 35. `exec::tests::pooled_kernel_serves_cells_without_a_long_warming_state` asserts on no
      elapsed time at all (it polls `pool.ready_len()`, bounded at 10 s): nothing to fix there.
    - **Mermaid `<script>` SRI + `crossorigin`: now moot by construction.** Nothing fetches mermaid
      from a CDN any more — build inlines the vendored copy and preview serves it from a same-origin
      route (OFF-2) — so there is no cross-origin subresource left to pin. It would only come back if
      someone points `TALIESIN_MERMAID_URL` at a CDN, which is an explicit opt-out.
    - **Perf (low):** protocol-level op-message batching (one WS message per save, not per-op). Worst
      case: an edit near the top of a long doc where every downstream block emits a `SetMeta`
      (`diff.rs` `anchor_op`). Client + server ship together, no wire-compat constraint.
    - **Audit long-tail:** a tens-of-MB cell output blocks ZMQ receive before the cap fires
      (`kernel.rs`, do-not-touch).

11. **2026-07-22 polish-audit residuals** (P3 hardening + a11y + "feels finished"; detail:
    [2026-07-22-polish-audit.md](2026-07-22-polish-audit.md); [AUDITS.md](AUDITS.md) records the round).
    **Passes (a)-(f) all shipped** (design-system single-source, scaffold `<h1>`/`<time>`, `<article>`
    landmark, announce/focus holes, CLI/diagnostics, reduced-motion+print, emitted-markup a11y; see
    "Already shipped"). **The tokens, a11y-interaction and CLI-docs bullets all shipped 2026-07-25**
    (see below). One bullet is left:
    - **Semantics (M3/M13, H1):** `<ul>`/`role=list` (needs a CSS-grid + category-filter-JS restructure +
      browser verify), hero/card image-alt lint nudge, deck `theme-color`/OG (PA-H1 residual).
      Owner design-Qs (deck copy-button, card whole-`<a>`) are parked in the doc, not build-ready.

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

29. **Reduction-audit residuals** (P3, dev-facing; detail:
    [2026-07-17-reduction-audit-map.md](2026-07-17-reduction-audit-map.md)). Phase 2 + T1 + R2 shipped and
    the codebase is lean; two items were explicitly deferred and never filed here. Both re-verified open:
    - **R1 — two divergent text extractors.** *Half closed 2026-07-25:* the Cmd-K side no longer has
      an extractor of its own — `search::section_text` was `indexable_text` **plus a 1500-char cap**,
      and with the cap gone it is exactly `render::indexable_text`. What remains is the original
      divergence: `text_content` (which feeds `llms.txt`) decodes `&#8217;`/`&nbsp;`,
      `render::indexable_text` does not, so naively reusing one would leak raw entities into
      `llms.txt`. That fork is pinned by a passing test, so it is conscious, not a bug. Its stated
      sequencing hook is spent (item 23 has shipped); revisit only if a consumer needs them equal.
    - **T2 — three site modules each run their own raw-source pre-scan** (`site/xref.rs`, `site/book.rs`,
      `site/discovery.rs` each `read_to_string` the page and re-implement a slice of the include/parse
      pipeline). A recurring pattern rather than a single bug; unify on one shared pre-scan **if you are
      already in there**, not as a standalone refactor. Overlaps item 20, which wants exactly one shared
      whole-site pass.

30. **Demand-probe persona 4 (analyst) artifact** (P3, M, mostly writing; spec
    `docs/superpowers/specs/2026-07-22-corpus-demand-probe-design.md` §4). The four-persona demand-probe
    program ships each persona as one artifact in three roles — a green corpus pin, a findings doc, and a
    `/gallery/<name>` exhibit. Personas 1-3 landed (`corpus/course/`, `corpus/tarn/`, `corpus/descent/`, all
    pushed) and their findings are items 16-18. **Persona 4's `corpus/analyst/` was never authored**
    (confirmed absent), so the program is 3 of 4 and its slate is the only remaining un-probed shape.
    Diminishing returns are real and should set the priority: personas 1-3 each stacked the interactions the
    corpus had never combined and found **0 interaction-bugs** between them, only P3 friction on secondary
    surfaces. Worth finishing for corpus coverage, not because a fourth probe is likely to find a defect.

12. **i18n / Unicode multibyte correctness: DONE bar a demand-driven residual.** The LSP UTF-16 encoding
    fix shipped 2026-07-22 (folded from AP5; detail:
    [2026-07-22-i18n-unicode-sourcepos-audit.md](2026-07-22-i18n-unicode-sourcepos-audit.md)): the stdio
    LSP advertises `positionEncoding: utf-16` and converts at every boundary (I18N-2/3/4/5); I18N-1 was
    resolved as documentation (block start columns are always ASCII-prefixed, so the client conversion was
    unreachable). *Residual (not build-ready, demand-driven, do not spin up without a real ask): RTL
    layout, CJK line-breaking, non-ASCII heading-slug collisions.*

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

## Audit perspectives (pick ONE per session)

Proactive, findings-generating angles the ordinary rounds structurally cannot see: they need the
tool *run hard*, fed *hostile input*, or reasoned about as a *concurrent system*. **Working method:**
one perspective per session, solo; write a dated findings doc in `notes/`; add a row to
[AUDITS.md](AUDITS.md)'s round index; file the build-ready findings into "Open work" under their own
prefix. **Every audit's first job is to falsify its own entry:** the last four rounds each found
their entry overstated, misnamed, or already-shipped, so budget the first hour for that.

**Eleven of twelve are RUN. One remains: AP6**, stateful/solo.

| Round | Result | Work went to |
|---|---|---|
| [AP1 perf/scale](2026-07-23-ap1-performance-scale-audit.md) | no quadratic anywhere; the one tax is two full-site passes per warm save | PERF-1, shipped |
| [AP2 fuzzing](2026-07-22-ap2-robustness-fuzzing-audit.md) | zero unexpected panics; two input-bound gaps (uncatchable abort, comrak O(n²) hang) | item 26, shipped |
| [AP3 concurrency](2026-07-25-ap3-concurrency-audit.md) | every predicted race refuted; the cost is head-of-line blocking (0.11s → 12.15s) | item **35** |
| [AP4 cache/freeze](2026-07-22-cache-correctness-audit.md) | design sound; one real cold-build stale hit | AP4-1 shipped, rest shipped |
| [AP5 i18n/sourcepos](2026-07-22-i18n-unicode-sourcepos-audit.md) | premise mostly refuted; the real find is three position encodings in the LSP | item 12 |
| [AP7 a11y](2026-07-25-ap7-accessibility-audit.md) | document sound, application not; defects are all "content changes silently" | item **34** |
| [AP8 determinism](2026-07-22-determinism-audit.md) | positive bill of health; byte-identical across processes | closed |
| [AP9 semantic HTML](2026-07-22-semantic-html-audit.md) | strong positive; its one finding was a stale-artifact false lead | closed |
| [AP10 codebase health](2026-07-23-ap10-codebase-health-audit.md) | healthy; dead code ~nil; lsp/mcp lacked a panic boundary | item 21, shipped |
| [AP11 chaos](2026-07-25-ap11-chaos-audit.md) | failure paths well-built; the defect is wording (a missing interpreter reported as an author exception) | item **36** |
| [AP12 offline](2026-07-22-offline-guarantee-audit.md) | own assets genuinely offline; gap is author-introduced external refs | item 13 |

**AP6: Cross-browser / cross-platform.** CLAUDE.md mandates the chrome-devtools MCP and development
is Linux-only, so Safari, Firefox and mobile browsers are effectively untested, as are macOS/Windows
path handling, file-watch semantics and kernel spawning. The vanilla-JS client and the deck engine
are where these bugs hide. Start: drive the client through Firefox + WebKit (Playwright headless);
grep the Rust for `\`-vs-`/` path assumptions and Linux-only syscalls. **Setup cost is real:**
`/usr/bin/firefox` exists but only Chromium is cached for Playwright and `tools/ui-audit` has no
Playwright dependency. **Concrete exposure named by AP7:** `hidden="until-found"` on tabset panels,
`inert` on deck slides, `:focus-visible` opacity reveals, IntersectionObserver trigger lines.
*Stateful, solo.* Highest risk before the public flip (~end of summer), least certain yield.

**Lenses that were never AP-shaped** (proposed 2026-07-25, none run): **diagnostics-message quality**
(27+ `check` families with `title`/`fix` fields; nobody has audited whether each message names the
real fix and points at the right line; precedent: the generic CLI gate found 9 undocumented flags
where an audit had filed 2, drifting in both directions); **docs-vs-behaviour drift** (the dogfood
books *are* the manual and `check` validates their links but never their claims); **AP1's unchased
residuals** (kernel RSS drift, multi-hour warm RSS).

## Tier 3: demand-driven (band E; build only when a real user asks)

**Waits on demand, not on capacity.** The PMF audit's verdict is that what is missing is
**real users, not more features**, so nothing here is scheduled. One line each; the reasoning lives
in the linked audits.

- **Companion (Phase 2):** editor commands (`.tmd`-buffer text transforms only, never preview gestures);
- **LaTeX hover-preview in the VS Code editor** (Companion Phase 2, a sub-case of the LSP item below):
- **`.tmd` format-on-save** (open question): a source pretty-printer must preserve `data-sourcepos` line
- **Dogfood: migrate the FL-weather book to Taliesin** — a real Quarto to Taliesin migration +
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic).
- **`taliesin publish` follow-ups:** optional `--init` wrapper for the one-time `wrangler` setup;
- **Interactive/explorable numerics** (`FEATURE-IDEAS.md` #62-66; none pinned; promote with a corpus pin
- **Wave 5** (`ROADMAP.md`): print-pdf track (paged render *of* the built HTML), docs-as-spec, `{glsl}`
- **Site-level shared bibliography + hygiene** (M). `bibliography:` is per-document only
- **Author structure panel** (M/L). A read-only preview sidebar: the heading tree with per-section word
- **Session revision digest** (M). Surface the `BlockOp` stream the client already receives: a session
- **Block-level transclusion** `{{< include file.tmd#sec-id >}}` (M). Reuse a section across a series.
- **LSP for the language intelligence** (L). Everything an LSP needs is already in Rust (`check`,
- **Image optimization** (WebP/AVIF + `srcset` + lazy-load behind a content-hashed cache) — until posts
- **Marketing site** (deferred, feature-first; rolls into a demo-machine rebuild): `live-edit-hero-demo`
- **`serde_yaml` fallback watch-item:** if 0.9 breaks against a future serde/edition, swap to
- **PMF demand-driven tail** ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md), Tier C; each waits on a

## Quarto catalog (policy, not a task)

**Owner ruling 2026-07-16: no sweep. Triage an area on demand, when you next work that area.** Before
consulting it read the triage doc's "three layers" section
([2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md)): the entries are the asset
and were well-grounded on 2026-07-03, but the heading status is degenerate and the executive summary is
misleading. A skeptic verdict is evidence, never a ruling (its "drop Atom feeds" verdict was overruled;
Atom shipped with autodiscovery).

## Already shipped: do not re-add / re-scope

A compact **do not re-add / re-scope** guard, not a changelog: each line names work that is
finished. The detail is in git and in [AUDITS.md](AUDITS.md); if you need it, look there rather
than re-expanding this list.

- **The 2026-07-25 backlink-context + resume batch**
- **The 2026-07-25 book-wayfinding batch**
- **The 2026-07-25 hardening batch**
- **Book-level `theorems:`**
- **Live-executor mounts (F-04 full fix)**
- **Structure-preserving, book-aware `read`**
- **2026-07-22 (late) backlog-clearing pass**
- **AP8-1 executed-output path scrub**
- **DET-1 reproducibility guard**
- **DX audit batch**
- **E7 `taliesin lsp` (editor-agnostic language server over stdio) shipped 2026-07-22, all capabilities**
- **DX17(a) headless executed-output (python/r) shipped 2026-07-21:**
- **Click-to-source into `{{< include >}}`d files already works**
- **Deck audit**
- **Polish audit batch**
- **PMF builds**
- **Corpus-coverage**
- **Machine-facing audit**
- **AI-native packaging + guardrails**
- **R/Python stream ANSI leak fixed 2026-07-21**
- **Live defects**
- **Reduction/modularity**
- **Ungraceful-death reaping**
- **`assets/js/*` `tsc`/`@ts-check`**

## Decided against / do-not-re-litigate

- **Deck-motion: the whole item is closed** (was Open-work item 28, lifted out 2026-07-25 because it
  had no code left in it and "only open tasks live here"; detail:
  [2026-07-24-deck-motion-audit.md](2026-07-24-deck-motion-audit.md)). Option A shipped 2026-07-24;
  its two residuals (instant overview content flips via a `.tali-nofx` frame, magic-move resynced
  onto `CAM.morph`/`morphFade`/`morphFadeDelay`) and **(5)** (one viewport-driven wrap count for the
  whole overview map) shipped 2026-07-25. The owner delegated the remaining calls and they were
  **ruled, not deferred**: **(3) no-change** — an out-of-order arrival stays visually identical to a
  step, and distinguishing them buys a cue the reader has no vocabulary for; **(4) Option C (the
  shared-element FLIP rewrite) is declined** — the overview is a glance at a ~20-slide talk, not a
  navigator for 100+, and the readability floor closed most of the gap it existed for. **Do not
  re-cost Option C a third time.** A coverage-weighted refinement of (5) was tried and measured
  *worse* (15 of 25 slides against 23 of 25); the comment in the source says so — do not re-refine
  without measuring. Two LOW tradeoffs were flagged to the author and left as-is, not defects:
  ctrl+wheel-*down* claims browser page-zoom-out over the deck (that is the approved gesture), and
  it also fires inside an embedded deck on a scrollable page. *(Option B, the mode-invariant
  serpentine grid, is costed in the audit and was not chosen; the overview work is identical under
  A and B, so nothing shipped is wasted if B is ever revisited.)*
- **A separate per-page outline artifact for the book drawer** (`book-outline-artifact` Ship B's
  own spec, declined 2026-07-25 while building it). Measured rather than argued: the search index
  the sidecar would duplicate is **172 KB raw / 60 KB gzipped** on `docs/internals` (146 KB / 50 KB
  on `docs/guide`), it is already lazy-loaded on every page via `TALIESIN_SEARCH_URL`, and Cmd-K
  fetches it anyway — so a ~13x-smaller sidecar buys ~55 KB gzipped on one cached subresource in
  exchange for a second copy of `search::page_fragment`'s render-then-number-then-resolve recipe, a
  second whole-project assembly, a second `refresh_*_for_page` invalidation, a second serve route
  and a second build write. The drawer reads the same index through the same loader
  (`window.taliLoadSearchIndex`). Revisit only if the index ever grows past the point where loading
  it on a drawer open is felt — and measure again before believing it has.
- **`drawer-typeahead`, a filter box in the chapter drawer** (declined 2026-07-25 with the above).
  The audit named the cheaper alternative itself: Cmd-K plus the drawer's collapsible outline
  covers the need, and a second search-like box beside a Search button is a discoverability smell.
- **A "~N min read" label on a book chapter** (decided 2026-07-25 while shipping the cost signal).
  `prose::word_count` excludes fenced code and math, so a code-heavy chapter is understated — and
  reading code is *slower* than reading prose, so a minutes label carries that error into a promise
  about the reader's time, in the wrong direction, on exactly the chapters this tool exists for. The
  drawer and Contents print words. (The dated-post reading-time estimate in `render/mod.rs` is a
  different surface and is unchanged; `is_article` is test-pinned, do not touch it.)
- **Flipping a book chapter's label to prefer `title:` over its `# H1`** (raised + resolved
  2026-07-25 while building 23's Ship A). The symptom is real — a chapter's drawer / Contents /
  Cmd-K label can differ from its `<title>` — but **measured across every book in the repo only 3
  of 48 chapters differ, and in 2 of them the `# H1` is the BETTER nav label** (`docs/guide`'s
  preface is `title: Taliesin` opening `# Why Taliesin`; `docs/internals`' is `title: Taliesin
  Internals` opening `# How Taliesin works`). Flipping the precedence would relabel those to the
  duller name to fix a divergence the author created on purpose. **The evidence I first filed was
  overstated**: the "all these surfaces agree" claim I cited is a comment on a *website*-page test
  and is true in the case it describes; a book chapter simply has a nav label distinct from its
  page title. Resolved as documentation (a note in `docs/internals/sites.tmd` + a corrected comment
  at `site/mod.rs`'s website-title test), not code. Nothing is searchable-only-by-one-name: the
  page record's body carries the rendered title block, so both names find the page in Cmd-K.

- **CAD-as-code (`{openscad}` / CadQuery cell → live 3-D preview): researched 2026-07-23, NOT built**
  (detail: [2026-07-23-cad-as-code-research.md](2026-07-23-cad-as-code-research.md); two background research
  passes, feasibility + market). Technically **feasible and a clean fit** (user-installed `openscad`
  subprocess → STL → bundled MIT three.js, the same shape as the shipped `graphics3d` viewer) and
  commercially **legally green** (an arm's-length CLI call is FSF "mere aggregation"; the models are the
  user's own). Killed on **demand**: wrong audience, tiny niche, and the peer group (Quarto, Jupyter Book,
  mdBook) ships nothing like it with zero requests for it. **Do not bundle openscad-wasm (GPL).** Five
  named revisit triggers, any one of which reopens it: (1) *author-pull* — you actually want to write a
  `.tmd` that is better with a live parametric model (a 3-D-printing build log, a mechanism tutorial); under
  corpus-plus-roadmap that alone is sufficient, just name the pin doc; (2) the peer group ships embedded
  CAD; (3) notebook-CAD usage multiplies materially; (4) text-to-CAD becomes reliable *and* moves
  in-document; (5) a concrete external ask (course, client, grant scope). The implementation path is
  pre-decided in the doc so a revival needs no re-research.
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
- **Refuted by measurement (do NOT re-scope):** **heading-demotion (AP9's HTML-1 / former item 14) already
  ships** (`7e60f6c`, 2026-07-12): a titled HTML doc demotes every body heading one level under its
  `<h1 class="title">` (`demote_heading_html`, gated Html+titled+`!hide_title_block`, decks/books excluded by
  construction), so a fresh render/build of a titled page emits exactly one `<h1>`. AP9's "12 `<h1>`" measured
  a stale gitignored `corpus/bayesian-website/_site/index.html` (a pre-fix build artifact). The only corpus
  docs with multiple `<h1>` are decks (`deck.tmd`/`deck-marginalia.tmd`/`embed/talk.tmd`), which are exempt by
  design. `build` does not leak forkserver subtrees (the graceful
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
