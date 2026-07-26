# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git + [AUDITS.md](AUDITS.md) +
> [ROADMAP.md](ROADMAP.md); delete an item when it lands, don't leave a `[x]`. The "already shipped"
> list near the bottom is the compact anti-rot guard (do not re-add / re-scope), not a changelog.

## State (2026-07-26)

**Branch `backlog/band-a-diagnostics`**, stacked on `backlog/backlink-context-and-resume`, which is
itself stacked on `origin/main`. **Nothing is pushed** — the author pushes on request only.

**No commit counts are recorded here, deliberately.** They were wrong twice in one session: the
first was written one commit stale, and the correction then mis-added the two branches' totals —
because any count written *into* this file is invalidated by the commit that writes it. Same rule
as a SHA (see Git under "Standing constraints"). Count them instead:

```sh
git log --oneline origin/main..HEAD                              # everything unpushed
git log --oneline backlog/backlink-context-and-resume..HEAD      # just this branch
```

**The demand-probe programme is finished, 4 of 4** (2026-07-26, second batch of the day). Persona 4
shipped as `corpus/analyst/` + `analyst.rs` + `/gallery/analyst` and closed item **30**. Its lesson is
about *slate design*, not probe count: personas 1-3 each stacked features the corpus had never
combined and found **0** interaction bugs, because the features compose. Persona 4 crossed a
*dimension* the corpus had never crossed — **two languages executing in one document** — and found
**two** real defects, both the same shape: *the R arm of a two-arm facility was never built.*
`figure_wrap` had a fallback and `table_wrap` did not (AN-1: a labelled `tbl-` cell whose output is
not a table emitted a dangling cross-reference, silently, with `check` clean). `KernelSpec::python`
carried two startup preambles and `KernelSpec::r` carried an empty list (AN-2a: **every** R figure
rasterised onto opaque white, on a page whose default theme is dark — found only in the browser, the
markup looked right). Neither is visible from inside a single-language document, and every corpus
document was single-language. **A fifth persona is not indicated; a fifth un-crossed dimension might
be, and none is currently known.** Both defects are fixed and shipped; the remaining four findings
are items **39** (band A), **40** (band B) and **41** (band D).

**Bands A and B were both empty before that**, as of the 2026-07-25 band-B batch. It closed items **11**
(PA-M3 list semantics, PA-M13 image-alt lint, PA-H1's deck theme-color + social meta), **29**
(R1 + T2, both closed on evidence rather than built), and emptied item **10** of everything
actionable (AP3-3 fixed; the rest split between won't-fix — which stays as item 10 in band D —
demand-driven, and declined-on-measurement). Nothing in "Open work" is buildable today; the next
entries come from the next audit round.

**Band A had been empty; item 39 is the only thing in it, and it came from the probe above, not from
an audit round.** The 2026-07-25 band-A batch closed items **34** (AP7, all five findings),
**35** (AP3-1), **36** (AP11-1), **37** (DIAG-1) and **38** (DOCS-1); what is worth carrying forward
is in [AUDITS.md](AUDITS.md) under "The 2026-07-25 band-A batch", not repeated here. Two findings
turned out larger than filed: DIAG-1's six fall-through diagnostics were **eight** (the zero-GENERIC
test found a seventh, and the two build-only execution diagnostics are invisible to any `check`-side
sweep), and AP7-1's fix surfaced two real authoring skips the blind rule had been hiding.

Five earlier code batches landed the same day (the hardening set, SKIM-1/2/3a, the P3 residual batch,
the book-wayfinding batch, the backlink-context + resume batch).

**Gates at the last code landing, re-run before trusting them:** full workspace suite with all three
gates and `--test-threads=1`; `cargo fmt --check`, `clippy --workspace --all-targets` and both JS
`tsc` gates clean; `check` clean on `corpus/tarn`, `docs/guide`, `docs/internals` and `site`. The
band-A batch additionally browser-verified every client-side change (the chrome-devtools MCP profile
was held by a parallel session, so via the project's own `puppeteer-core` harness, as AP7 itself did).

**Nothing is owed by the author.** The one item that was — the in-editor click-to-source round-trip
from the naming purge (Task 8 Step 5) — was **verified working by the author on 2026-07-25**, which
closes the naming purge outright. It needed a human because nothing automated covers the real editor
round-trip: the companion was repackaged and reinstalled and the relay harness passes both
directions, but the harness stops at the relay and cannot see whether the editor actually lands the
cursor. That gap is still there, so a future change to the relay or the companion re-opens the same
manual check.

**Every AP slot is run, and as of 2026-07-26 so is every proposed non-AP lens.** The last two —
**AP1's unchased residuals** and the **behavioural half of the docs lens** — ran on 2026-07-26 and
both of their findings shipped the same day (the 2026-07-26 audit batch, below). **There is no
queued audit angle left.** A further round now needs a *new* lens argued for first, not one taken
off a list; what those two rounds deliberately did not measure is recorded in
[AUDITS.md](AUDITS.md) rather than left looking like an open task.

Both entries were wrong about where their own defect was, in opposite directions, which is the
fourth and fifth time running that an audit's first job (falsify its own entry) paid for itself.
AP1's residual predicted a *kernel* leak; over 1,000 real executions the kernel **saturates**, and
the unbounded growth was Taliesin's own freeze cache. The docs lens's existing gates covered flag
and env-var *existence* thoroughly; the vocabulary they did not cover had drifted **totally**.
Working method is in "Audit perspectives": a dated findings doc, a row in
[AUDITS.md](AUDITS.md)'s round index, and the build-ready findings filed into "Open work" under
their own prefix.

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
  before trusting a total. **Flake status: FIXED 2026-07-25, and the entry that described it was
  wrong in both its test and its cause.** Looping the `--bin` binary reproduced the flake 3 times in
  37 runs (matching the recorded ~1 in 13) and captured what had never been captured: **three
  different tests** failing, from **one** root cause. `prepare_connection` peeks free ports by
  binding then releasing them, so concurrent starts can be handed the same port and the loser exits
  at startup — `Address already in use`, or `ConnectionReset`, or (for the pooled-warm test) a
  missed 10 s poll bound. The re-roll that survives this lived in the *callers*, so the three
  test-side callers of the raw `Kernel::start` inherited it; which test failed on a given run was
  chance, which is why it was mis-attributed to `kernel_executes_..._runaway_cell` and "fixed"
  against a theory of interrupt timing that was never the cause. The re-roll now lives on
  `Kernel::start_with_retry` and `crates/server/tests/kernel_start_is_retried.rs` fails if any
  caller reaches the un-retried primitive again. **Verified 0 failures in 45 post-fix runs** under
  the same load. A red `exec`/`kernel` probe is now a real signal, not a coin flip.
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
- **To measure anything about cell execution, edit the CELL BODY, not the page.** A cell's freeze key
  is its own code plus all upstream same-language code, so editing a page's *prose* leaves every cell
  hash intact and nothing re-runs. AP3-1's first probe did exactly this and reported 0.09 s with and
  without the fix — a false all-clear on an unfixed build. The same trap makes any "is the kernel
  busy?" setup silently no-op.
- **A message-catalogue sweep must enumerate the EMITTERS, not one command's output.** DIAG-1 measured
  `check --format json` over 23 targets and found six uncatalogued diagnostics; there were eight. The
  two it could not see are emitted only by `build`/`publish` (a crashed cell, a cell that never ran),
  and `check` never executes a cell, so no amount of `check` coverage would have reached them.
- **Calibrate a new lint against real output before writing it.** Measuring the proposed
  `TAL-SHAPE-*` rules over all 14 site projects killed four of their own prescriptions, including
  the most valuable one (it fired on 11.8% of the corpus, essentially all false positives) and one
  whose stated justification did not exist in the tree.

## Open work (priority order: take from the top)

**Ranked for implementation, not by theme.** Band A is what a session can build today and B is
buildable but not worth a session alone; **both are currently empty**, so there is no code here to
take — the next session either runs an audit (see "Audit perspectives") or takes an owner ruling
from C. C and D are blocked and are listed so they are not re-scoped. **Item numbers are stable**
and referenced from the findings docs and
[AUDITS.md](AUDITS.md), so they are NOT renumbered when the order changes, and a closed item's number
is never reused.

**Standing rule for a batch:** branch per batch, verify each fix by *mutation* (restore the bug,
watch the named test fail), browser-verify anything client-side, and **delete the item from this
file when it lands**. Read "Standing constraints" first; several of these have a recorded trap.

### A. Build now: measured, unblocked, and each one has its fix shape recorded

39. **Cross-page references are misreported, in opposite directions** (P3, S; detail:
    [2026-07-26-corpus-demand-probe-analyst.md](2026-07-26-corpus-demand-probe-analyst.md), AN-5 +
    AN-6). Both found on one page of `corpus/analyst/`, both about a *valid* cross-page ref:
    - **AN-5 — a cross-page `@sec-` renders as the bare word "Section"** (no number, no title), so
      the sentence reads "…as set out in Section." The same ref on its own page reads "Section 3".
      **Do NOT "fix" this by harvesting the number:** `site/mod.rs`'s `harvest_xref_numbers`
      excludes `sec-` deliberately and its comment gives the failure mode — a website target filled
      with a flat "1" is then mislabelled **"Chapter 1"** by `rewrite_one_xref`. The open part is
      the *label*: carry the heading title in `XrefTarget` (already in hand in `scan_page_anchors`)
      and use it when the number is empty. Watch `XrefTarget: PartialEq` — it drives the dev
      server's "did a target move" check, so a new field makes a heading edit re-render referring
      pages (correct, but intend it). Cross-page `@fig-`/`@tbl-` are **fine** (right page + right
      number, harvested from the render) — do not re-scope those.
    - **AN-6 — the editor flags valid cross-page refs as `TAL-XREF-UNDEF` errors.** The LSP has no
      `Site::discover` and is per-document, but the project is a site, so `check <dir>` is clean and
      the built page resolves every ref while the author sees red squiggles on correct content.
      Candidates: resolve the enclosing `_site.yml` in the LSP, or downgrade an unresolved xref to a
      hint when the document sits inside a site project.

**One residual is deliberately NOT closed, and is recorded where it belongs rather than left
looking open:**
- **AP7's "not chased" list** (a real screen reader, colour contrast, callouts/theorems as composite
  widgets, the mobile TOC sheet, reduced-motion across the scroll features) is scope the round
  declared out, not work it left undone. It is in
  [2026-07-25-ap7-accessibility-audit.md](2026-07-25-ap7-accessibility-audit.md).

*(**AP3-3 is closed**, 2026-07-25: it was neither the test nor the cause on file. See the flake
paragraph under "Standing constraints" — the entry is kept there because the *method* it teaches,
loop the real condition and capture the failure rather than theorize from the symptom, is what the
recorded version got wrong.)*

### B. Buildable, but low yield on its own

40. **Two authoring traps in the executed-table path, both worth one documented line** (P3, XS;
    detail: [2026-07-26-corpus-demand-probe-analyst.md](2026-07-26-corpus-demand-probe-analyst.md),
    AN-3 + AN-4). Neither is engine work; both were hit while authoring `corpus/analyst/`, which now
    demonstrates the right idiom with a comment saying why.
    - **AN-3:** `knitr::kable(format = "html")` returns a *string*, so a bare R cell prints its own
      markup and the reader sees escaped `&lt;table&gt;` in a `<pre>`. It works under
      knitr/rmarkdown (knitr splices it), which is what makes it a trap. The wrapper is
      `IRdisplay::display_html(as.character(kable(...)))`. `docs/guide/using/code.tmd` documents
      `#| tbl-cap:` without saying an R cell must *publish* HTML rather than print it. **This is
      also what exposed AN-1** (now fixed): the dangling anchor was only visible because the output
      was not a table.
    - **AN-4:** a bare pandas `display(df)` emits `<table border="1" class="dataframe">`, a row-index
      column, and a `<style scoped>` block — `scoped` is a **removed** HTML attribute no current
      browser implements, so that style element applies page-wide. `to_html(index=False, border=0)`
      gives markup the page's own table styling reaches.

**The band was empty before this.** The 2026-07-25 band-B batch cleared the last three: items **11**
and **29** are closed and deleted, and item **10** is reduced to its two no-clean-fix kernel
limitations and moved to band D.
Two of the three closed
on *evidence* rather than on code, which is the outcome this band is most likely to produce and is
worth stating plainly: an item here is cheap to build and therefore easy to build without asking
whether it should be. See "Decided against" for what was declined and on what measurement.

### C. Blocked on an owner ruling or decision (not a task until then)

Do not start these as code. Each needs a call from the author first; the item records what the
question is and what the evidence says.

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

2. **Deck presenter tools** *(owner deferred 2026-07-22 — NOT selected this round)*: one-command deck
   publish (Share QR still encodes `localhost:PORT`), a presenter laser/spotlight, auto-advance. The
   `footer:`/`logo:` threading from this item **shipped** (see "Already shipped"); the presenter pieces
   were considered and left for later. Revive only on a real speaker ask.

### D. Blocked on a device, a real user, or is working-as-intended

Kept visible so they are not re-scoped. Revive on a real signal, not on capacity.

10. **Two kernel limitations with no clean fix** (P3, dev-facing; the rest of the old
    "reliability / test-infra long tail" is closed — AP3-3 fixed 2026-07-25, the mermaid-SRI
    bullet was moot by construction, the `mounts:` live-HTTP test moved to Tier 3, and the
    op-batching bullet is in "Decided against" with its measurement):
    - **R cold kernels still orphan on ungraceful parent death.** IRkernel has no
      `ParentPollerUnix` equivalent, so there is nothing to arm; PDEATHSIG is the only other
      lever and is hazardous. R is rarely the cold single-doc path, and the warm-pool,
      cold-Python and `/tmp`-sweep halves all landed. `kernel.rs`.
    - **A tens-of-MB cell output blocks ZMQ receive before the cap fires.** `kernel.rs`.
      (The old note called this file do-not-touch; that was the completed rewrite-scoping
      list, not a freeze — see CLAUDE.md. It is still unfixed, just not forbidden.)

4. **Deck engine mobile polish** (P2): mobile pinch/pan + touch gestures (they matter for the phone-feed
   deck mode; hard to verify without a device); drop `fitSlide` from the resize path (needs a lazy
   fit-on-show refactor first). *(The desktop trackpad half shipped 2026-07-24 — pinch / ctrl+wheel-down
   opens the overview map, with a 250 ms hysteresis. What that left behind is all shipped or ruled —
   see "Deck-motion: the whole item is closed" under "Decided against", formerly item 28.)*

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

12. **i18n / Unicode multibyte correctness: DONE bar a demand-driven residual.** The LSP UTF-16 encoding
    fix shipped 2026-07-22 (folded from AP5; detail:
    [2026-07-22-i18n-unicode-sourcepos-audit.md](2026-07-22-i18n-unicode-sourcepos-audit.md)): the stdio
    LSP advertises `positionEncoding: utf-16` and converts at every boundary (I18N-2/3/4/5); I18N-1 was
    resolved as documentation (block start columns are always ASCII-prefixed, so the client conversion was
    unreachable). *Residual (not build-ready, demand-driven, do not spin up without a real ask): RTL
    layout, CJK line-breaking, non-ASCII heading-slug collisions.*

41. **R graphics cannot follow the page theme; matplotlib figures can** (P3, M; detail:
    [2026-07-26-corpus-demand-probe-analyst.md](2026-07-26-corpus-demand-probe-analyst.md), AN-2b).
    Taliesin renders every inline matplotlib figure **twice** (light + dark foreground) and swaps
    them on the theme toggle (`kernel.rs`'s `MPL_THEME_PREAMBLE`); measured on `corpus/analyst/` the
    Python figure emits two genuinely different PNGs and the ggplot figure emits one, so a
    mixed-language report has half its figures track the reader's theme and half baked. **Blocked on
    being a feature, not a fix:** a real version re-renders the figure twice against two
    foregrounds — real design, and only worth it on a real ask. **Do NOT confuse this with AN-2a,
    which is fixed:** the R device no longer paints opaque white under a transparent figure
    (`KernelSpec::r` now carries `options(repr.plot.bg = "transparent")`). Transparency lets the page
    show through; the *ink* is still baked at one colour, and that is what is left here. The
    documented workaround (a neutral mid-grey palette) is in the corpus doc and is the second
    instance of the "neutral-palette convention" option named in item 18's F-02. Minor and separable:
    an R figure is emitted `<img alt="output">` where the Python pair is `alt=""`; both sit inside a
    captioned `<figure>`, so `alt=""` is right and `"output"` is noise read aloud.

### E. Gated, not actionable now (kept visible, do not spin up)

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

**All twelve are RUN** (AP6 closed the set on 2026-07-25), **and so are all three non-AP lenses**
(AP1's residuals closed the set on 2026-07-26). Nothing here is left to take: the table below is now
a complete record, not a menu.

| Round | Result | Work went to |
|---|---|---|
| [AP1 perf/scale](2026-07-23-ap1-performance-scale-audit.md) | no quadratic anywhere; the one tax is two full-site passes per warm save | PERF-1, shipped |
| [AP2 fuzzing](2026-07-22-ap2-robustness-fuzzing-audit.md) | zero unexpected panics; two input-bound gaps (uncatchable abort, comrak O(n²) hang) | item 26, shipped |
| [AP3 concurrency](2026-07-25-ap3-concurrency-audit.md) | every predicted race refuted; the cost is head-of-line blocking (0.11s → 12.15s) | AP3-1 shipped; AP3-3 fixed 2026-07-25 (wrong test AND wrong cause on file) |
| [AP4 cache/freeze](2026-07-22-cache-correctness-audit.md) | design sound; one real cold-build stale hit | AP4-1 shipped, rest shipped |
| [AP5 i18n/sourcepos](2026-07-22-i18n-unicode-sourcepos-audit.md) | premise mostly refuted; the real find is three position encodings in the LSP | item 12 |
| [AP7 a11y](2026-07-25-ap7-accessibility-audit.md) | document sound, application not; defects are all "content changes silently" | all five shipped |
| [AP8 determinism](2026-07-22-determinism-audit.md) | positive bill of health; byte-identical across processes | closed |
| [AP9 semantic HTML](2026-07-22-semantic-html-audit.md) | strong positive; its one finding was a stale-artifact false lead | closed |
| [AP10 codebase health](2026-07-23-ap10-codebase-health-audit.md) | healthy; dead code ~nil; lsp/mcp lacked a panic boundary | item 21, shipped |
| [AP6 cross-browser](2026-07-25-ap6-cross-browser-audit.md) | **no findings**: Firefox == Chromium on every measured axis, 0 console errors | closed |
| [AP11 chaos](2026-07-25-ap11-chaos-audit.md) | failure paths well-built; the defect is wording (a missing interpreter reported as an author exception) | shipped |
| [AP12 offline](2026-07-22-offline-guarantee-audit.md) | own assets genuinely offline; gap is author-introduced external refs | item 13 |
| [AP1-residual + docs-behaviour](2026-07-26-ap1-residual-and-docs-behaviour-audit.md) | kernel does NOT leak (saturates over 1,000 execs) — the freeze cache was count-capped, never byte-capped; and the guide documented `about:` for 9 days after its removal | AP1-R1 + DOCS-2/3/4/5, all shipped 2026-07-26 |

**Lenses that were never AP-shaped** (proposed 2026-07-25) — **all three have now run.** Two ran on
2026-07-25 (*diagnostics-message quality*, *docs-vs-behaviour drift*, findings in
[2026-07-25-diagnostics-and-docs-drift-audit.md](2026-07-25-diagnostics-and-docs-drift-audit.md));
the third (**AP1's unchased residuals**) ran on 2026-07-26 together with the *behavioural* half the
docs lens had left untouched, findings in
[2026-07-26-ap1-residual-and-docs-behaviour-audit.md](2026-07-26-ap1-residual-and-docs-behaviour-audit.md).
Everything they filed has shipped.


## Tier 3: demand-driven (below every band above; build only when a real user asks)

**Waits on demand, not on capacity.** The PMF audit's verdict is that what is missing is
**real users, not more features**, so nothing here is scheduled. One line each; the reasoning lives
in the linked audits.

- **An end-to-end live-HTTP test for `mounts:` serving** (was item 10's second bullet). The F-04 work
  landed and unit-pins the pure `match_mount`/`resolve_project`/`classify_change` helpers, and live
  mount serving is browser-verified; what is missing is only the bin-crate gap of a real
  `reqwest`/`TcpListener` harness. Mounts are preview-only, so this waits for a reason to exist.
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

- **Demand probe #4, the computational-report analyst** (2026-07-26; detail:
  [2026-07-26-corpus-demand-probe-analyst.md](2026-07-26-corpus-demand-probe-analyst.md)).
  `corpus/analyst/` is a two-page quarterly latency readout and **the only corpus project that runs
  two languages in one document** (`{python}` pandas/matplotlib cleans + charts, `{r}`
  broom/ggplot2/patchwork fits + diagnoses, both over one committed `data/latency.csv`). Pinned by
  `crates/core/tests/analyst.rs` (render-time only, no kernel gated) and exhibited at
  `/gallery/analyst`. **AN-1 fixed:** `exec::table_wrap` returned a labelled `tbl-` cell's output
  unchanged when it held no `<table>`, so the spent number and the already-rewritten `@tbl-` link
  pointed at an id nothing emitted — now it falls back to `table_figure_wrap`, the same degradation
  `figure_wrap` always had. **AN-2a fixed:** `KernelSpec::r`'s `preambles` was empty where Python
  has two, so R's inline device kept its default opaque-white background and every R figure came out
  as a white slab on the dark theme even when the author made the plot's own backgrounds
  transparent; it now carries `options(repr.plot.bg = "transparent")`. That is **additive, and
  measured to be** — a default `ggplot` and base-R graphics still rasterise as alpha-less RGB, which
  is what keeps it from turning every existing R figure dark-on-dark; `tests/r_kernel.rs` asserts
  both halves. **Measured healthy, do not re-scope:** cross-language freeze isolation
  (editing one language's cell re-runs 0 cells of the other, both directions, 123 of 140 samples);
  the `MAX_BYTES` cap binding on a real page (140 edits, linear to ~16.75 MB then a plateau with the
  entry count falling); one table counter spanning the authored and executed paths in document
  order; one figure counter spanning both languages; cross-page `@tbl-`/`@fig-` to cell-produced
  floats carrying the right page and number.
- **The 2026-07-26 audit batch** (the last two lenses, run and shipped the same day; detail:
  [2026-07-26-ap1-residual-and-docs-behaviour-audit.md](2026-07-26-ap1-residual-and-docs-behaviour-audit.md)).
  **AP1-R1**: the freeze cache was capped by entry *count* (1024) and never by *bytes*, and an entry is a
  whole rendered cell output — 150 edits to one matplotlib cell wrote a 6.71 MB `_freeze/<page>.json`,
  linear, zero evictions. A 16 MB `MAX_BYTES` budget now bounds it (verified end-to-end: 450 edits
  plateau at 16.77 MB with the entry count *falling*, where before it grew without limit). **The
  kernel does NOT leak** — that was the residual's premise and it is refuted, so do not re-scope it.
  **DOCS-2**: `about:` was removed at `dcf0588` (2026-07-17) and the guide kept documenting it in 28
  places across 6 pages, including three whole sections and a nonexistent `ABOUT_KEYS`; all purged onto
  `hero:`, and `frontmatter::guide_vocabulary_gate` (three tests) is the recurrence guard, the third
  link in the front-matter chain beside the flag and env-var gates. **DOCS-3/4/5**: `footer:`/`logo:`
  added to the reference page, the wrong `theme` default in `configuration.tmd` corrected (unset is
  *auto*, not light), and the `image-alt` prose that contradicted PA-M13's own lint rewritten.
- **The 2026-07-25 band-B batch** — the last three low-yield items, two of them closed on evidence
  rather than code. **AP3-3** (item 10): the flake was neither the test nor the cause on file; the
  port re-roll moved off the callers onto `Kernel::start_with_retry`, with a source-level guard
  against a caller reaching the raw `Kernel::start` again. **PA-M3** (item 11): a listing is a `<ul>`
  of `<li>`-wrapped cards with an explicit `role="list"` (WebKit drops list semantics under
  `list-style: none`); the filter hides the item, not the card. **PA-M13**: `image:` without
  `image-alt:` warns, reading parsed front matter so a YAML example in prose cannot trip it — and the
  four real corpus omissions it found are fixed with described alt text. **PA-H1's residuals**: a
  deck keeps `<meta name="theme-color">` with its canvas, and a standalone built deck emits the same
  social block a standalone page does (a site deck's richer block still wins). **Items 29 and 10's
  remaining bullets** were closed without code — see "Decided against".
- **The 2026-07-25 band-A batch** — every build-ready audit finding: AP7-1 (relative heading
  demotion + a title-block-aware heading rule), AP7-2 (a reactive `{js}` sink announces its output
  when that output is text, and stays silent when it is a chart), AP7-3 (`.scrolly` /
  `.code-walkthrough` steps are labelled groups pointing at the stage they drive), AP7-4 (a preview
  block swap keeps keyboard focus), AP7-5 (a skip link to the in-page TOC), AP3-1 (a bypass lane for
  cell-free rebuilds), AP11-1 (`TAL-KERNEL`: a cell that never ran is no longer reported as an author
  exception), DIAG-1 (eight diagnostics catalogued + a zero-`GENERIC` gate), DOCS-1 (two env knobs
  documented + a gate tying `--help` to the guide). Do not re-derive any of these from the findings
  docs: the docs describe the *defects*, which no longer exist.
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

- **WS op-message batching** (was item 10's perf bullet; declined 2026-07-25 **on measurement, and
  the premise was right**). `tools/live-edit-bench` on `corpus/posts/em-algorithm/index.tmd` confirms
  the worst case exactly as filed: an insert near the top emits **55 ops, 53 of them `SetMeta`**, one
  WebSocket frame each. It is still not worth the protocol churn, because the same run says where the
  time actually goes: **warm edit 32.2 ms, of which the diff is 0.94 ms**. Batching would save ~4
  bytes of framing per message (~220 bytes against a 32,303-byte payload, 0.7%) and 54 client
  handler dispatches, none of it on the critical path. Reopen only if the render cost drops far
  enough that framing is measurable, or if a profile shows the client's per-message work dominating.
- **Item 29's two reduction-audit residuals, R1 and T2** (closed 2026-07-25 without code). **R1** —
  the remaining fork between `text_content` (which decodes `&#8217;`/`&nbsp;` for `llms.txt`) and
  `render::indexable_text` (which does not) is deliberate, pinned by a passing test, and its
  sequencing hook is spent; equalizing them would leak raw entities into `llms.txt`. It is a
  documented decision, not an open task. **T2** — "three site modules each run their own raw-source
  pre-scan" is *partly rotted*: `site/book.rs` does not want a resolved source at all (it reads the
  chapter's own leading `# H1`) and its own comment records that it already reads each file once.
  The real duplication is a six-line read-then-`includes::resolve` idiom in **two** places
  (`site/xref.rs`, `site/discovery.rs`), and the divergence that looked like a latent bug — xref
  falling back to `Path::new(".")` where discovery falls back to `root` — is unreachable, because
  `page.input` is always `root.join(rel)` and so always has a parent. That is below the bar the item
  set for itself ("not as a standalone refactor").
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
