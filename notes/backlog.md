# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git + [AUDITS.md](AUDITS.md) +
> [ROADMAP.md](ROADMAP.md); delete an item when it lands, don't leave a `[x]`. The "do not re-add"
> list near the bottom is a compact anti-rot guard, **one line per entry**, not a changelog — if an
> entry there needs a paragraph, the paragraph belongs in its dated findings doc.

## State (2026-07-27)

**The mutation campaign is FINISHED.** It ran this file from 2026-07-18 and closed 2026-07-27 with
item 69: measured end to end (`crates/core`'s five post-07-18 files, then the ten `crates/server`
files, then `lsp_nav.rs` whole), every survivor triaged, and items 58-65 + 68-69 shipped as pins.
No compute is owed and no survivor list is outstanding.

**Start here: band A holds only item 56, which is an authoring judgment and a feature proposal
rather than a task — so this session opens with WHICH LENS TO RUN, not which item to take.** The
menu is under "Proposed audit lenses"; **the standing recommendation is real-device mobile**, now
unblocked (batch 1 shipped, so that round verifies rather than re-finds, and the first thing to
check on real hardware is the drawer scroll lock — `overflow: hidden` on the root holds less
completely on iOS Safari than on Chromium, and only Chromium was measured). L3 is the other live
one: it is still only partial (`headless_js.rs` read; `lsp.rs`, `complete.rs`, `skim.rs`,
`manifest.rs` unread), though the campaign has since pinned much of what it would have looked at.

**What the campaign is worth reading for now is its method, not its results** — the four pin rules
in the band-A preamble and the lessons attached to items 58-65 and 68-69 are the transferable part,
and three of them corrected a *stated* rule in this file rather than finding a new one.

Five batches shipped on 2026-07-26: mobile (42-49, every HIGH on the board), path parity
(50, 51, 57), migration UX (53, 54), the metadata half of 56, and **deck weight + headless-JS
bounding (52, 55)**. The mutation re-run ran its `crates/core` half the same day. Band B is empty;
band C holds only item **25**, parked on a public-release *date* rather than on a decision; the rest
is blocked on a device or a real user (band D) or gated (band E).

**Verified 2026-07-27 at the last landing**: full workspace suite with all
three gates and `--test-threads=1` is **95 binaries, 1,624 tests, 0 failures**;
`cargo fmt --check` and `clippy --workspace --all-targets -D warnings` clean. The two JS `tsc` gates
were **not** re-run — this batch touched no JS — so treat them as last verified 2026-07-26. The
live-Chrome suite
(`TALIESIN_REQUIRE_CHROME=1 --test read_run_js`) is a **fourth** gate nothing else runs, and 55 is
the reason to remember it exists.

Read the 2026-07-26 probe traps under Standing constraints before writing any probe
— plus the four the mobile batch added (a scroll lock cannot be tested with `scrollBy`; a capability
rule can be discarded by the cascade; a tap target on a sticky bar must grow by overlay; a stale
*cause* can sit under a real *symptom*) and the four the path-parity batch added (an audit's stated
fix can be a revert; a documented reason can be true of a sibling path and false of yours; a finding
that names one instance has not enumerated the shape; a hidden overlay still renders a stale list).

**A lens menu now exists** ("Proposed audit lenses", below): six never-run lenses (L1-L6), four
re-runs ranked by age × measured churn in each round's own surface, and four directions that the last
weeks' work has *unblocked*. As of 2026-07-27, **L1, L2, L4 and L5 have run** (items 50-51 and 52-56);
**L3 is partial** (only `headless_js.rs`; `lsp.rs`, `complete.rs`, `skim.rs`, `manifest.rs` unread);
**L6 is blocked** on a repository that is not on this machine; **the mutation re-run has run to
completion and is closed**; and the other three re-runs are un-run, the deck audit being the most
rotted of them.

**Do not trust this file's freshness.** The author pushes mid-session with no signal here, and a
scoped prune leaves the rest looking freshly reviewed. **No commit counts and no SHAs are recorded**
— any count written *into* this file is invalidated by the commit that writes it (it was wrong twice
in one session). Ask git instead:

```sh
git log --oneline origin/main..main    # what is unpushed, right now
```

**Gates at the last code landing, re-run before trusting them:** full workspace suite with all three
gates and `--test-threads=1`; `cargo fmt --check`, `clippy --workspace --all-targets` and both JS
`tsc` gates clean; `check` clean on `corpus/tarn`, `docs/guide`, `docs/internals` and `site`.

**Nothing is owed by the author.** The last outstanding item — the in-editor click-to-source
round-trip from the naming purge — was verified working by the author on 2026-07-25. It needed a
human because nothing automated covers the real editor round-trip: the relay harness passes both
directions but stops at the relay and cannot see whether the editor lands the cursor. **That gap is
still there, so a future change to the relay or the companion re-opens the same manual check.**

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
  before trusting a total.
- **The kernel-test flake is FIXED (2026-07-25), and the entry that described it was wrong in both
  its test and its cause.** Looping the real binary reproduced it 3 times in 37 runs and caught what
  theorising never did: **three different tests** failing from **one** root cause. `prepare_connection`
  peeks free ports by binding then releasing them, so concurrent starts can be handed the same port
  and the loser dies at startup (`Address already in use`, or `ConnectionReset`, or a missed 10 s poll
  bound). The surviving re-roll lived in the *callers*, so the three test-side callers of the raw
  `Kernel::start` inherited the race; which test failed was chance, which is why it was mis-attributed
  to `kernel_executes_..._runaway_cell` and "fixed" against an interrupt-timing theory that was never
  the cause. The re-roll now lives on `Kernel::start_with_retry`, and
  `crates/server/tests/kernel_start_is_retried.rs` fails if any caller reaches the un-retried
  primitive again. **Verified 0 failures in 45 post-fix runs** under the same load: a red
  `exec`/`kernel` probe is now a real signal, not a coin flip.
- **Git:** do not trust a SHA written in notes. Check `git log --oneline origin/main..main` for what is
  unpushed and `git reflog show origin/main` before believing any "not pushed" claim.
- **How this file lies to you:** entries rot. Before picking an item, **grep its named symbol/flag in
  source** and prefer measuring the running product over reading this file. Trust an item's *symptom*,
  never its cause, line number, or stated cost (all three have rotted). Verify a fix by **mutation**
  (restore the bug, watch the named test fail), not by a green suite. Grep traps: a bare word matches
  prose, `grep | head` reports head's exit code, quote `--include='*.tmd'` in zsh. **Commit before
  mutation-testing:** `git checkout -- <file>` on an *uncommitted* file restores from HEAD and
  destroys the working implementation (it did, twice).
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
- **Probe traps from the 2026-07-26 audit day (each one produced a false result first).**
  - **zsh does not word-split an unquoted variable.** `files="a.html b.html"; for f in $files` passes
    all names to `grep` as ONE argument. Write the list literally in the loop, or use an array.
  - **An empty needle matches everything.** An unset shell variable makes `grep -qF -- "$n"` true on
    every file. **A parity/coverage row that is uniformly positive is a broken probe until proven
    otherwise**, exactly like a uniformly negative one.
  - **A runtime-injected DOM node is invisible to a static grep.** Deck `theme-color` is created by
    `deck.rs:240` (`createElement` + `setAttribute`), so grepping built HTML reports it missing on all
    four deck paths — a false regression of shipped work. When the mechanism is runtime construction,
    the only valid needle is the rendered result in a browser.
  - **`#tali-toc-handle` is an id, not a class.** A `.tali-toc-handle` selector reports the sheet
    handle missing everywhere.
  - **Raw CDP `Network.emulateNetworkConditions` silently no-ops**, with or without `Network.enable`.
    Use puppeteer's `page.emulateNetworkConditions(...)`. A "throttled" number that is not slower than
    the unthrottled one is a broken instrument, not a fast page. (`Emulation.setCPUThrottlingRate`
    does work over raw CDP.)
  - **cargo-mutants: any test-command narrowing fabricates MISSED, and `--lib` is only the
    loudest case.** Scoping to `--lib` reports MISSED for everything an integration test covers
    (measured: 102 MISSED / 0 CAUGHT, all artefact). **Scoping to a PACKAGE does the same thing and
    looks far more reasonable** (measured 2026-07-26): `-p taliesin-core` is cargo-mutants' *default*
    for a core mutant, and it cannot run `crates/server/tests/*`, where several core subsystems are
    actually pinned. Re-testing that run's 96 survivors with `--test-workspace=true` flipped **51 of
    96 (53%) to CAUGHT**. For a `crates/core` file, `--test-workspace=true` is not optional; for a
    `crates/server` file the package default is sound, because core tests cannot reach server code.
    The cost is real (each core mutant relinks ~50 server test binaries, ~1.7 mutants/min at `-j 4`),
    so budget for it rather than trading it away.
  - **Being called is not being tested — the trap that makes a mutation run worth the compute.**
    An end-to-end test that drives a subsystem *calls* every helper in it, so replacing a whole
    function is caught instantly and coverage looks fine. What survives is the inside: token-boundary
    tests, nesting-depth loops, cursor arithmetic. Measured on `skim.rs`, whose only integration
    coverage is `skim_cli`: `LayerKind::tag -> ""` and `first_prose_sentence -> None` are both caught,
    while **35 finer-grained mutants inside those same functions survived the full workspace suite**.
    Sampling whole-function mutants and generalising from them is how this gets missed (it did, here).
  - **cargo-mutants housekeeping:** its scratch copy carries no `.git`, which is why the baseline was
    red (item **57**, fixed). Still pass `--output` outside the tree so a run is never mistaken for
    working state — but note `mutants.out/` **is** already in `.gitignore` (line 9); the earlier claim
    that it is not was wrong. Run it from a `git archive` snapshot rather than the live tree, so the
    working tree stays free for other work during a multi-hour run.
  - **Equivalent mutants already triaged, do not re-triage:** `diagnostics/shape.rs` `is_content`
    (`:81` both conjuncts, and `:156` `i + 1` → `i`). At its only call site the slice runs between two
    consecutive heading indices, so no block in it can be a heading; and no `Block` anywhere is built
    with empty `html` (every block carries `data-block-id` by invariant, which `corpus.rs` enforces).
    Both conjuncts are therefore unreachable-false and no test can kill these without hand-building a
    `Block` the renderer cannot emit. **Writing that test would be the vacuous-test defect this very
    round exists to remove.**
- **Calibrate a new lint against real output before writing it.** Measuring the proposed
  `TAL-SHAPE-*` rules over all 14 site projects killed four of their own prescriptions, including
  the most valuable one (it fired on 11.8% of the corpus, essentially all false positives) and one
  whose stated justification did not exist in the tree.

## Open work (priority order: take from the top)

### The mobile audit RAN on 2026-07-26, and its eight findings SHIPPED the same day

Detail: [2026-07-26-mobile-audit.md](2026-07-26-mobile-audit.md). The author's reported symptom
reproduced, and **seven of its eight findings shared one root cause: the tool never asked what kind of
device it was on.** Measured then over every `.css`, `.js` and `.rs` file in `crates/` +
`web-client/`: **zero** `pointer: coarse`, **zero** `hover: none`, **zero** `any-pointer`. Every
keyboard hint, hover-reveal and presenter tool was gated on viewport *width* or on deck *layout mode*
— two proxies that both failed the same way, by treating a wide or stepped phone as a desktop.
**Fixed 2026-07-26** (items 42-49 deleted; the four method lessons the build paid for are recorded
where those items were, further down this section).

**Two traps this round paid for, recorded so the next one doesn't:**
- **`resize_page` floors at ~500px.** It resizes the *window*, and Chrome will not go narrower. Two
  probes reported `innerWidth: 500` while I believed I was at 390 — silently across the 40rem
  breakpoint that half the audit is about. **Use viewport emulation, never window resize, below
  ~500px.**
- **The deck feed flag is on `document.documentElement` (`html.tali-feed`) and its scroller is
  `.tali-slides`, not the document.** Probing `.tali-deck` and `window.scrollY` made a working feed
  look completely dead, and I filed that wrong before catching it.

**Still un-run, and worth naming as the next lens:** everything under "Not measured" in the findings
doc — real iOS Safari / Android Chrome (this was Chromium emulation, which does not model WebKit,
momentum scroll, the dynamic viewport toolbar or safe-area insets), a phone screen reader, tablet
widths, and the `--host` QR phone-preview flow, which is a first-class phone feature that got no
coverage at all.

### Proposed audit lenses (2026-07-26) — the menu, since the table in AUDITS.md is not one

[AUDITS.md](AUDITS.md)'s round index is a *record*: a further round needs a lens proposed first. These
were proposed on 2026-07-26 by crossing what has run against what the tree now contains, and each
carries the measurement that justifies it so none has to be re-derived. **Ranked; take from the top.**

**New lenses, never run:**

- **L1. Path parity** (feature × emission path). Five paths emit a document: single-doc `preview`,
  site `preview`, standalone `build`, site `build`, and a `mounts:`-served project. Three rounds each
  tripped over exactly one divergence and none swept the matrix: **DX1** (the located validators ran
  in `build`/`check` but not preview), **AP7** (the mobile TOC sheet exists only in a single-doc
  preview, so a site preview emits no sheet chrome at all), **DIAG-1** (two execution diagnostics
  exist only on the `build`/`publish` path). Needs no device, no kernel and no network. **RAN and its
  three findings SHIPPED 2026-07-26** →
  [2026-07-26-path-parity-audit.md](2026-07-26-path-parity-audit.md).
- **L2. Reader-side runtime performance** (crosses AP1 × AP6 × mobile). AP1 measured the *server*
  (8,000 blocks in 647 ms, 400 pages in 874 ms); AP6 measured browser *parity*, not speed. The only
  successful Lighthouse run is 2026-07-11, desktop mode, on the website, and it predates the switch
  from per-page inlining to hashed `_assets/`; the 07-22 round tried and got `NO_FCP`. Measured
  2026-07-26 on a release build: a standalone `corpus/deck.tmd` is **4,583,261 bytes** (1,375,317
  gzipped), and a `tech-blog` site build ships `_assets/mermaid.*.js` 3,572,004 · `app.css` 229,204 ·
  `app.js` 91,066 · `search-index.js` 118,726 · `hover-index.js` 54,690. Conditional loading works (a
  no-mermaid doc builds to 869,748), so this is an unmeasured surface, not a bug list: **INP, LCP and
  scroll cost under a 4× CPU throttle**, on the device the mobile round says the readers are on.
- **L3. The subsystems that post-date every lens that would own them.** First-commit dates against
  audit dates: `lsp.rs` (1,922 lines, 07-21) is younger than the security (07-17), DX (07-18),
  mutation (07-18) and polish (07-19) rounds, and only AP10 has read it; `headless_js.rs` (615 lines,
  07-22) **spawns an external browser** and the security round is five days older than it;
  `complete.rs` (1,157, 07-18/07-25), `skim.rs` (647, 07-25) and `manifest.rs` (303, 07-24) likewise.
  The web manifest is a *phone* surface (add-to-home-screen, standalone display) the mobile round did
  not touch.
- **L4. Deprecation / migration UX** (crosses time × the author's existing files). `about:` was removed
  07-17 and the *docs* drifted nine days (DOCS-2..5). The same question about a **user's project** has
  never been asked: FORMAT_VERSION 4, the `_freeze/` schema, the `.taliesin/` schemas, the retired
  `q`-prefix names. What does the tool say to a document written against last month's build?
- **L5. The content half of the skimmability round**, which that round named and deliberately left
  undone: 0 of 37 dogfood pages set `description:`, 8 xref links across 19 chapters, 0 backlinks, 0
  `{.definition}` blocks in 60,208 words of internals. Glossary, term index and float digest render
  empty until an authoring pass happens. Not code.
- **L6. A real external document.** All four demand probes were fixtures written for the probe. The
  FL-weather Quarto book (Tier 3) is the fifth probe and the only one the corpus cannot fake.

**Re-runs, ranked by age × churn measured in each round's own surface (2026-07-26):**

- **The deck audit (07-12) is the most rotted:** 2,510+/1,196- in `deck.rs` + `deck.js` + `deck.css`
  since, and the mode-model was deliberately reshaped after it (reader + PDF deleted, phone feed
  added, motion round 07-24). AUDITS.md already warns the doc describes *outgoing* behaviour. Re-run
  it **crossed with touch**, not as-is: MOB-1 and MOB-2 just put the deck back at the top of band A.
- **The mutation / vacuous-test round (07-18) RAN TO COMPLETION 2026-07-26/27 and is CLOSED.** Do
  not re-scope it; **do not re-run it against the same scope** — every survivor it measured is
  triaged, and a re-run's only new information would be about code written since. Scope and
  results — **the core half has no findings doc of its own, so these are its only numbers**: the 5
  post-07-18 core files (`skim.rs`, `shape.rs`, `cite_this.rs`, `manifest.rs`, `book_toc.rs`) =
  298 mutants → 187 caught, 96 missed, 7 timeout, 8 unviable, and re-testing the 96 against the
  full workspace suite flipped **51** to CAUGHT, leaving 44 real survivors (residual = item 65).
  The other two are banked in-repo:
  [server half](2026-07-27-mutation-server-half-complete.md) (ten files: 707 mutants → 156
  survivors, at **7.8/min**, 92 minutes) and
  [`lsp_nav.rs` whole](2026-07-27-mutation-lsp-nav-complete.md) (444 mutants → 24 survivors, at
  **9.0/min**, 48 minutes). Pins landed as items 58-65 and 68-69. **Four durable rules came out of
  it, and each corrected something this file previously stated:**
  - **Scoping the test command fabricates MISSED** (the `--lib` and `-p` traps, recorded in full
    under Standing constraints). For a `crates/core` file `--test-workspace=true` is not optional.
  - **A caught mutant aborts its test run early, so a file's mutants/min tracks its survivor
    density** — a *slow* file is a *bad* file. The 2.3/min figure that argued for a 3.5 h budget
    was an artefact of one such file, and cost two sessions of deferral.
  - **A partial run's shape-conclusion describes the part it measured, not the file** (item 68:
    "all 36 survivors are one shape" was true of 338 mutants and wrong about `lsp_nav.rs`).
  - **A hang IS the detection — 65 of 65, no counter-example — but only once a fixture enters the
    loop.** A `+= → *=` cursor mutant sitting in the *missed* column means nothing executes that
    loop at all, which is the worse finding of the two (item 69).
- **The website/brand audit (07-11):** its headline performance finding measured per-page inlining and
  is now obsolete (hashed `_assets/`), which is itself the signal. Its Lighthouse pass was desktop-mode
  only, which is how it missed the touch-target defects the mobile round found.
- **The security release audit (07-17)** should wait for the flip date it is already parked on (item
  25), **except** `headless_js.rs` and the LSP, which post-date it and spawn or expose processes.
- **Not due:** AP10 (07-23). Of its 19,337 touched lines in `crates/`, roughly half are vendored
  mermaid, the PowerShell grammar and the reverted ask-ai feature.

**Unblocked by progress already made (was blocked, is not any more):**

- **Real iOS Safari / Android Chrome, a phone screen reader, and the `--host` QR flow** — blocked on a
  device; the author is now device-testing. This is the mobile round's own "Not measured" list.
- **Deck touch gestures (band D item 4)** — the device blocker is gone and the mobile round confirmed
  the feed itself works, so pinch/pan is testable now.
- **Fuzzing the LSP + MCP request loops**, filed as an AP2 residual. HEALTH-1 shipped, so
  `serve::guarded` now wraps both dispatches (`lsp.rs:105`, `mcp.rs:127`): there is finally a survival
  property to assert. Before it, a fuzz finding could only restate "there is no boundary".
- **Reader-surface work that needed section extents** — `data-section-end` shipped 07-26, so the four
  skimmability proposals blocked on "zero `<section>` extents" have substrate.
- **Still blocked:** the prune half of the release audit (gated on the public-flip date), and true
  WebKit unless the phone is an iPhone.

### The bands

**Ranked for implementation, not by theme.** Band A is what a session can build today and B is
buildable but not worth a session alone. C, D and E are blocked and are listed so they are not
re-scoped. **Item numbers are stable** and referenced from the findings docs and
[AUDITS.md](AUDITS.md), so they are NOT renumbered when the order changes, and a closed item's number
is never reused.

**Standing rule for a batch:** branch per batch, verify each fix by *mutation* (restore the bug,
watch the named test fail), browser-verify anything client-side, and **delete the item from this
file when it lands**.

#### A. Build now

**Band A holds one item: 56, and it is an authoring judgment plus a feature proposal rather than a
task.** Items 58-69 all shipped 2026-07-27 and are kept below as *lessons*, not work — each one's
entry records what its own scoping got wrong, which is the part that transfers. **The next thing
to take is a lens, not an item** (see "Proposed audit lenses"; standing recommendation is
real-device mobile).

**Four rules that governed every pin item below** (58/59/61 paid for all four on 2026-07-27), kept
because the next test-writing batch of any kind will need them:

1. **Verify each pin by *mutation*:** restore the mutant by hand, watch the *named* test fail, then
   restore the fix. A pin never seen to fail is not a pin. **Revert by inverse edit, never
   `git checkout`** — it restores from HEAD and silently deleted these very tests once, making two
   verifications vacuously green. And assert your mutation anchor matches **exactly once**: one that
   matched twice reported "skip" and tested nothing.
2. **Cover two axes, not one.** A sweep over *well-formed* input pins **spans**; but most survivors
   are guards that **reject**, and weakening one does not move a boundary — it makes the code
   *accept nonsense* (an empty key, an empty id, a construct read one character past the line end).
   **Malformed input is a separate axis**, and the first pass at 58 skipped it and killed only half.
3. **Suspect a test that looks like coverage.** Two holes here were fully line-covered: a helper that
   drove the real path and *discarded its result* (`let _ = recv()` on the LSP handshake), and a
   fixture that *could not reach the assertion* (a `.hidden` dotfile in a filter that only emits
   `.tmd`).
4. **Never write a test for a timeout**, and expect some survivors to be **unkillable**: 65 of 65
   campaign timeouts are cursor arithmetic in a scan loop (mostly `+=`→`*=`, plus a few `-=`→`/=`
   and `+=`→`-=`), where a stalled loop spins instead of returning a wrong answer, so the hang *is*
   the detection. **But that rule assumes a fixture reaches the
   loop** — a `*=` cursor mutant that shows up as MISSED rather than TIMEOUT means no test executes
   that loop at all (item 69: two did, and the fixture that killed the boundary mutants next to
   them turned both into hangs). Four mutants are proven equivalent or `cfg`-dead
   (listed in the 07-27 findings doc); `cargo-mutants` does not evaluate `cfg`, so the `cfg`-dead one
   **reappears on every run**. Prove, record, move on — do not chase a score.

58. **SHIPPED 2026-07-27** (`a4f1600`, `ba0b3f7`, `31a30fe`, `9f7a4b5`, `8ecf607`) — the
    cursor-walk pins for `lsp_nav.rs` + `lsp_complete.rs`. **92 survivor locations, 89 killed, 3
    provably equivalent**, each verified by restoring the mutant and watching a named test fail.
    Post-pin measurement and the four unkillable mutants:
    [2026-07-27-mutation-server-half-complete.md](2026-07-27-mutation-server-half-complete.md).
    **The lesson cost a second pass, so read it before writing the next pin batch: a cursor walk
    over well-formed fixtures pins SPANS, but every survivor was a guard that REJECTS.** Weakening
    one does not move a boundary, it makes the classifier accept nonsense — an empty citation key, an
    empty xref id, a construct read one character past the end of the line. **Malformed input is a
    separate axis from cursor position**, and the first pass, which walked only well-formed
    fixtures, killed just half. Items 60 and 62-64 are the same kind of code, so budget the
    malformed-input axis into them from the start.
59. **SHIPPED 2026-07-27** (`6015c13`) — the `server_capabilities` assertion, all 12 killed. **The
    lesson is the cause, not the fix:** the tests all *performed* the handshake and threw the result
    away, because `handshake()` does `let _ = client.receiver.recv()` on the `InitializeResult`. A
    server advertising **nothing** passed the entire suite. Look for that shape elsewhere — a helper
    that drives the real path and discards what it returns is a coverage hole no line-coverage tool
    reports, since every line ran.
60. **SHIPPED 2026-07-27** (`3e62b91`) — the rest of `lsp.rs`'s request surface. **21 survivors, 20
    killed, 1 proven equivalent**, each verified by restoring the mutant and watching a named test
    fail. `lsp.rs` is now fully closed (33 of 33 triaged). **Three lessons, in the order they cost
    time:** (a) the axis rule from 58 held again — *three* holes were the reject side of a rule
    whose accept side was covered (an include path that does *not* exist, a completion filter with a
    *non-empty* typed prefix, a section with *zero* words); (b) `frontmatter_key_doc` was a second
    instance of 59's shape, a test that looked like coverage — `md.contains("title")` was satisfied
    by the `` `title:` `` header the hover prints *above* the description, so the lookup could return
    any other key's prose and pass; (c) `cmd_lsp` needed a **new test binary**
    (`crates/server/tests/lsp_stdio.rs`) because the in-process tests drive `run()` over
    `Connection::memory()` and never touch the command wrapping it. **The equivalent one is
    recorded** in the findings doc's now-five-row table: `672:39 + → *` changes `"sub/"` to `"sub"`,
    and `Path::join` gives the same directory for both.
61. **SHIPPED 2026-07-27** (`02a35da`) — `runtime_dirs.rs`, 4 of its 5 killed. **This item was
    written wrong and the correction is the useful part.** It claimed the file had zero tests and
    that `pid_alive` was unpinned; both were false. The file has two good tests, the sweep logic is
    well covered (dead owner, live owner, own pid, legacy dir), and `pid_alive` is pinned by the
    live-owner case. The real gaps were narrower: **the producer and consumer were tested apart** —
    `owner_pid` against hand-*written* names, `sweep_in` against hand-*built* dirs — so nothing
    checked that the names `tagged` actually produces parse back, and the public
    `sweep_stale_runtime_dirs` was never called at all (every test used `sweep_in` with an isolated
    base). The 5th is a **false survivor**: `pid_alive -> false` at line 103 is the
    `#[cfg(not(unix))]` arm, dead code here, and cargo-mutants parses without evaluating `cfg`, so
    **it will reappear on every future run of this file.**
62. **SHIPPED 2026-07-27** (`4db6a68`) — `headless_js.rs`, **7 of 7 killed**, each verified by
    mutation. **The lesson generalises past this file: the existing tests checked that each browser
    phase *has* a bound, and nothing checked what a bound does when it fires.**
    `every_browser_await_is_bounded` enumerates awaits and wraps — and the teardown *decision*
    around those wrappers (`closed && waited` → kill) was invisible to it, so both its operators
    could flip, leaking a Chrome process per run, with the scan green. Likewise the wedged-launch
    test asserted `why.contains("launch")`, which **both** launch failures satisfy: the outer bound
    sits above the configured `launch_timeout` so the *library's* error (it carries the browser's
    stderr) is what the author reads, and inverting that ordering stayed green while costing the
    diagnostic. Two survivors are pinned **structurally on purpose** (a browser that speaks CDP and
    then lies is not reproducible); `eval_timeout` was extracted so the one relationship worth
    asserting became assertable. Detail:
    [2026-07-27-mutation-server-half-complete.md](2026-07-27-mutation-server-half-complete.md).
63. **SHIPPED 2026-07-27** (`5e1303b`) — `complete.rs`, **30 of 30 killed**, each verified by
    mutation. **This item's own ranking note was half wrong, which is the part worth keeping:** "a
    wrong shell completion is visible immediately and breaks nothing" is true of the completion
    brain and false of the five survivors in `completions --install`, **the only path in this tool
    that creates a file outside the project**. `install_plan` was unit-tested against a hand-built
    environment, so the step that reads the *real* one and performs the write had never run — and
    one survivor (`== "--install"` → `!=`) makes `completions zsh` **install** when the author asked
    to print. Also closed: the `.tmd` walk's depth budget (it must reach *and* stop) and its
    symlink rule, which was a source comment and is the only thing making the walk cycle-proof.
    **Two shapes recurred from item 58** — every existing path test completes at the *top level*,
    so the directory-prefix split was unreachable, and the dotfile rule again needed a dotfile that
    *is* a `.tmd`. Detail:
    [2026-07-27-mutation-server-half-complete.md](2026-07-27-mutation-server-half-complete.md).
64. **SHIPPED 2026-07-27** (`da4bf55`) — the tail. **20 survivors, 15 killed, 5 recorded as
    unkillable** (3 need a TTY, 2 are provably equivalent, 1 effectively so). **This item's own
    dismissal was the thing worth checking:** "minus the 4 cosmetic `colored`/`paint` mutants"
    was wrong — colour is not cosmetic when the question is whether it appears *at all*, and two
    of those make a **piped** run emit ANSI escapes, which a subprocess sees perfectly well. Only
    three of the fourteen truly need a terminal. Two real defect-shaped holes came out of it: the
    readiness **summary** looked its verdict up by name over a list where picking the wrong entry
    still emits a plausible sentence, and an **empty** `CONDA_PREFIX`/`VIRTUAL_ENV` counted as an
    active env (shells export them empty on deactivate). `zip.rs` now reads its archive back the
    way an extractor does — its three existing tests read **fixed byte offsets**, which stay right
    even when the central directory does not. Detail:
    [2026-07-27-mutation-server-half-complete.md](2026-07-27-mutation-server-half-complete.md).
65. **SHIPPED 2026-07-27** (`1f9e1f6`) — the `crates/core` half's residual. **18 mutants at the 14
    recorded locations: 15 killed, 3 proven equivalent.** **This item's own equivalence guess was
    wrong on the one that mattered, and the way it was wrong is the lesson:** `page_skim`'s
    `&&`→`||` was filed as needing "a page that emits a title block *and* opens with an `<h1>`",
    which reads only **one side** of the conjunction — the other side is trivially reachable (a page
    with **no** title block whose first heading is a `##`), and under the mutant that page's first
    section vanishes from the projection. **When an item calls a boolean mutant equivalent, check
    both sides before believing it.** Its second guess (`first_prose_sentence`'s `>`→`>=`) held.
    Two survivors are killed by **hanging**, which is the campaign's timeout rule again: both are
    cursor rewinds in the class scanner, and a scanner that fails to advance spins instead of
    answering wrongly. `sentence_at` turned out to have **no test at all** — it is reached through
    the backlink line, so a wrong answer is a plausible quotation of the wrong sentence. The three
    equivalents are in the findings doc's now-eight-row table.
66. **SHIPPED 2026-07-27** (`df84db2`) — `404.html` links the shared `_assets/` bundle.
    Measured on `corpus/tarn`: **355,700 → 16,185 bytes** (the site's own index is 26,463), and
    what is left is the theme-bootstrap + enhancer-registry pair *every* page inlines, so the 404
    is no longer a special case. **Its hrefs are root-absolute, unlike every other page**, for the
    same reason its home link already was: the host serves this one file for any unknown path, so
    `../_assets/…` resolves against whatever directory the reader guessed at. Browser-verified at
    `/404.html` and at `/deep/deeper/oops.html`. **The cost is recorded in the doc comment rather
    than hidden:** this makes the page's existing root-deploy assumption load-bearing for styling
    too, so a project-subpath deploy now degrades to *unstyled* rather than merely mislinking —
    acceptable because the tool's own publish path (Cloudflare Pages) is a root deploy and the
    scoped `<style>` stays inline, so the layout survives regardless. The preview keeps the
    self-contained form (it has no `_assets/` to link).
67. **SHIPPED 2026-07-27** (outside the repo: `~/.local/bin/taliesin`, so there is no commit for
    it — the change is the guard described here). The author lifted the deferral. **Measured
    before: 24.3 s for a single `taliesin __complete preview ""` on a stale binary; after:
    0.024 s** — a thousandfold, and the ~15 s in the old entry was an understatement. Cause was
    exactly as recorded: the launcher shells out to `cargo build --release` whenever any source is
    newer than the binary, and the shell shim calls it on every tab press. The guard exits early
    for `__complete` only, running whatever binary is there and exiting **0 with no output** when
    there is none (an error message would be rendered by the shell as a completion candidate).
    **`completions` is deliberately NOT exempt**, against what the old entry proposed: it is run by
    hand once to generate or install the shim, it generates that shim from the binary's own command
    list, and a stale binary would install a script that has drifted from the tool. Fast-and-stale
    is right for something that fires on a keystroke; slow-and-correct is right for a one-off.
    Verified with three controls: a real command still rebuilds, `completions` still rebuilds, and
    a missing binary produces zero bytes on both streams.
68. **SHIPPED 2026-07-27** — `lsp_nav.rs` measured end to end, **443 of 444** (detail:
    [2026-07-27-mutation-lsp-nav-complete.md](2026-07-27-mutation-lsp-nav-complete.md)):
    394 caught, **24 missed**, 21 timeout, 4 unviable, at **9.0 mutants/min** — 48 minutes, against
    the 3 hours the old 2.3/min estimate would have argued for. **It answered both its questions.**
    *Confirmation:* item 58's pins killed **35 of the 36** pre-pin survivors; seven of the eight
    functions that held them are now at zero. *Discovery, and the reason running the tail was worth
    it:* the partial run's conclusion that **"all 36 survivors are one shape"** was true of the 338
    it measured and **wrong as a description of the file** — 23 of the 24 remaining survivors are in
    the tail it never reached, and the biggest hole there is not a position classifier at all.
    **Banked as the transferable rule: a partial run's shape-conclusion describes the part it
    measured, not the file.** The 24 survivors are item **69**.

69. **SHIPPED 2026-07-27** — `lsp_nav.rs`'s 24 measured survivors plus the one unmeasured mutant.
    **25 of 25 detected: 22 killed by a named assertion, 3 by hanging**, each verified by
    restoring the mutant and watching the named test fail. Five tests, the largest killing 17.
    **The campaign ends here.** Detail:
    [2026-07-27-mutation-lsp-nav-complete.md](2026-07-27-mutation-lsp-nav-complete.md). **Two
    lessons, and the second one corrects a rule this file has been repeating since 07-26:**
    (a) this item's "`bib_entry_offset` has NO test at all" was **wrong in the same way item 61's
    was** — it counted occurrences of the *name* in the test module, and the function is reached
    by two existing tests through its two wrappers. **A private helper's name count measures
    whether it is called directly, never whether it is tested.** What was actually missing was a
    fixture *shape*: every one is a canonical complete `@type{key,`, so neither whitespace loop
    ran a single iteration and no bounds check was ever evaluated at the end of the buffer (11 of
    the 17 need a truncated `.bib`, item 58's malformed axis for the third batch running).
    (b) **A `+= → *=` cursor mutant in the MISSED column is not a counter-example to the
    timeout rule — it means nothing enters the loop.** `401:23` and `406:27` looked like the
    first counter-examples to "62 of 62 hangs are detections"; they survived because a loop with
    zero iterations cannot spin, and adding the fixture flipped both to hangs. Tally is now
    **65 of 65**. The rule holds, but it *presupposes a fixture that enters the loop*, and a
    missed `*=` is the strictly worse finding of the two.

**A knowing skip, recorded so it is not re-litigated:** `interactive.rs` (5 of 5 survivors) is the TTY
wizard layer — `is_interactive`, `select`, `input`. Pinning it needs a PTY harness, and the *non*-TTY
path is already pinned by `crates/server/tests/wizard_gate.rs`. Poor cost/benefit; skip deliberately.

**Deck weight (52) and headless-JS bounding (55) SHIPPED 2026-07-26.** Both were correctly sized in
the preamble that used to sit here — 52 really did need external-asset awareness threaded through
`assemble_deck_page` rather than a flag, and 55 really did need every phase bounded with teardown
kept reachable. Measured: a site deck went **4,583,261 → 6,962 bytes** (the standalone artifact stays
4.4 MB and self-contained on purpose). **Four method lessons, since each changed the work:**

- **Read the dependency's source before believing an item's "unbounded" claim.** 55 listed
  `Browser::launch`, `new_page` and `goto` as unbounded; chromiumoxide bounds all three (a silent
  20 s `launch_timeout`, a 30 s `request_timeout`). The real gaps were the websocket connect that
  `launch_timeout` does *not* cover, and `close()`/`wait()`, which have no bound at all — `wait()`
  being the sharp one, since a browser can accept `Browser.close` and then simply not exit. The
  symptom was real and every stated cause but one was wrong.
- **"No automated reproduction" can be false.** A wedged browser is reproducible without a wedged
  browser: point `CHROME_PATH` at a program that launches and then sleeps, which is exactly what the
  launch path blocks on reading. 20.00 s before, 7 s after — the assertion is on the clock.
- **That test passed vacuously in 0.02 s** whenever it raced the other `CHROME_PATH` test: it read
  that test's `/nonexistent/…`, skipped every cell instantly as "chrome unavailable", and satisfied
  its own elapsed-time assertion by never launching anything. **A test whose subject is an env var
  needs a lock, and an assertion on *why* it skipped**, or the fast path is a green light.
- **A deck cannot link the page's `app.js`.** The obvious "let a deck take `AssetMode::External`"
  shares the page bundle — which carries `search.js`, binding a capture-phase Cmd/Ctrl-K on
  `document` and `preventDefault()`ing it. That would hand decks a palette they have never had and
  take a key from the deck's own handling. A separate `deck.<hash>.{css,js}` pair, written only when
  the build has a deck, keeps behaviour identical; the duplicated `code-enhance.js` is the price.

**Adjacent, surfaced by this batch and now filed as item 66** (do not re-file it a third time):
`404.html` still builds through the *inline* renderer, so a site of ~19 KB pages ships a 356 KB 404.

**Auditing is done for now.** Four fresh lenses on 2026-07-26 produced zero HIGH findings, while the
one round that produced four came from the author using the tool on a phone. The remaining menu
entries are the weak ones; **the next *audit* worth running is real-device mobile, and it is now
unblocked** — batch 1 has shipped, so that round verifies rather than re-finds. Everything it should
check is the "Not measured" list in the mobile findings doc, and the *first* thing to confirm on real
hardware is the drawer scroll lock: `overflow: hidden` on the root is known to hold less completely
on iOS Safari than on Chromium, and only Chromium was measured.


**The 2026-07-26 mobile audit's eight findings SHIPPED** (MOB-1..8, detail:
[2026-07-26-mobile-audit.md](2026-07-26-mobile-audit.md)). One root cause, one batch, as predicted:
the tree now has input-capability queries and the width/mode proxies are gone from the decisions that
were never about width. Items 42-49 are deleted. **Four things the round got wrong, kept here because
they are method lessons, not history:**

- **`window.scrollBy` cannot test a scroll lock.** `overflow: hidden` blocks USER scrolling and
  deliberately still permits programmatic scrolling, so MOB-5's 328px reading is the same with and
  without the fix. Use real key/gesture events: measured with PageDown, drawer closed the page moves
  707px, drawer open it does not move at all while the panel scrolls instead.
- **MOB-5's dialog half was already fixed** (`role="dialog"` since 2369d80; `taliFocusTrap` already
  wired, and the trap owns `aria-modal`'s full lifecycle). A static `aria-modal` was tried and
  reverted — the trap's release strips it. But **the symptom was real**: focus enters the panel and
  leaves ~300ms later, because `19-book-outline.js` re-parents the focused chapter link during
  hydration. Fixed there. This is the backlog's own "trust the symptom, never the stated cause".
- **The obvious MOB-7 fix is a regression.** `min-height: 44px` on a nav link grew the *sticky* bar
  from 52px to 75px at 844x390 — 19.2% of a landscape phone's viewport, i.e. MOB-8's defect
  reintroduced while fixing MOB-7. Tap targets on a sticky bar must grow by overlay, not by height.
- **A capability rule can be silently discarded by the cascade.** MOB-4's block first landed above
  `.tali-copy`'s own `opacity: 0` at equal specificity; copy stayed invisible while the anchor half
  worked, and a "selector is inside the block" test passed throughout. Assert source ORDER.

MOB-6 also had a **third** instance the round did not enumerate (`docs/guide/index.tmd`), found by
grep once the two named ones were known.

**The 2026-07-26 path-parity round's three findings SHIPPED the same day** (L1; detail:
[2026-07-26-path-parity-audit.md](2026-07-26-path-parity-audit.md)). Same shape as the mobile batch,
one layer up: page assembly was hand-wired at **three** sites with no shared owner (`render/page.rs`
for both static builds, `serve/mod.rs` for the single-doc preview, `serve_site/mod.rs` for the site
preview), and each finding was a line present in two of the three and absent from the third. It did
end with shared owners rather than another copy, which is what the round asked for.

**Items 50, 51 and 57 SHIPPED 2026-07-26** (PP-1, PP-2, PP-3), as one batch, ending with the shared
owners the round asked for: `render_single_doc` decides the single-document containment root once
(it was `Some(base)` hand-written at twelve call sites, and the hand-rooted entry point is deleted
rather than left as a thirteenth), and `TOC_SHEET_MARKUP` is the one copy of the sheet chrome all
four assemblers emit. **Four method lessons this round paid for:**

- **An audit's stated fix can be a revert.** PP-3's "give the single-file build the same inferred
  root" would have re-opened PT-2 (`9359a2c`): the inferred walk stops at `.git` **or** `_site.yml`,
  and widening to a *checkout* is exactly the escape that release-audit item closed. Separating the
  two markers satisfies both — `_site.yml` is an author declaring a project boundary and is the root
  the site build already passes, so honouring it *is* parity; `.git` never widens a single invoked
  document again. **Read the code the item proposes to change before trusting the change.**
- **A documented reason can be true of a sibling path and false of yours.** PP-1 was held back by
  "the client doesn't re-index on a live edit, so the index would go stale" — true of a site's
  cross-page index, false of a single doc, where `search.js` builds from the DOM and rebuilds on
  every open. Measured in the browser (append a heading live → the reopened palette goes 11 → 12
  items) rather than argued.
- **The `.git`-dependence was two tests, not the one the audit named.** `include_relative_base.rs`
  failed the same way. A finding that names one instance of a shape has not enumerated it.
- **A hidden overlay still renders.** Setting the palette input and reading its list gave a
  confident "No matches" while the overlay was closed the whole time — the list was stale, not
  empty. **Assert the surface is open before believing what it says**, and settle a transition
  before measuring geometry (the sheet reads as off-screen at y=844 synchronously and y=688 once
  settled).

**Adjacent, surfaced not fixed:** a site with **no `_site.yml`** (`build <dir>` accepts a bare
directory) declares no boundary, so a single-document render of one of its pages still roots at that
page, and the site path's own inference can still widen to `.git`. Nothing can infer an undeclared
boundary; the fix is to declare one. Also `corpus/posts/pca-geometry/` (the loose twin of the
tech-blog page, byte-identical to it and pinned so by `twinned_corpus_sources_stay_byte_identical`)
sits under no project marker, so `build` of it warns `include not resolved` — unchanged by this
batch, true since PT-2 shipped, and now uncovered by any test since the corpus pin moved to the
tech-blog copy.

**Measured healthy in the same round — do not re-scope:** **decks pass path parity outright** (all
four deck paths give the same 20-method `TaliesinDeck` facade, 18 slides, a runtime-injected
`theme-color`, and the same slide after `ArrowRight`); **`mounts:` differs from direct serving by 4
bytes** (boot nonce + ws path) with 0 failed requests and 0 console errors; the `{{< embed >}}` iframe
matches in build and preview; `--bare` refuses a deck with a real error instead of degrading. Also:
every content gate in `code_scripts_for`
matches its emitter exactly (including a `.scrolly` without `name=`, the sharpest suspect); the
load-bearing invariants (`data-block-id`, `data-sourcepos`, `data-section-end`), figure numbering,
favicon, `<html lang>` and generator meta are identical on all six paths; `render` is byte-identical
to `build <file>`; the `--bare` zero-`<script>` contract holds; site-build externalisation into
`_assets/` is correct and un-duplicated; zero console errors on all four live paths.

**The 2026-07-26 L2/L3/L4/L5 round** (detail:
[2026-07-26-lenses-l2-l5-audit.md](2026-07-26-lenses-l2-l5-audit.md)). Four lenses in one session
after L1 closed. Ordinary-page performance came back healthy on a throttled phone (every LCP inside
the 2,500 ms band), so the items below are the outliers, not a general problem.

**Migration UX (53, 54) SHIPPED 2026-07-26.** Both were "the tool says nothing useful to a document
written against last month's build", and both fixes were smaller than the items assumed. **Three
method lessons, since each one changed the fix:**

- **A missing signal and a mis-shaped signal look identical from the outside.** 53's `check` really
  did say "no problems found", but the warning was being pushed the whole time — as the
  *missing-config advisory*, which `check` discards on purpose because a bare directory of pages is a
  legitimate project. The fix was not "add a warning", it was "stop reporting a different situation".
  Before adding a diagnostic, check whether one is already being emitted and filtered.
- **An item's proposed data shape can contradict a decision the tree already made.** 54 proposed
  `RETIRED_KEYS: &[(&str, Option<&str>)]` carrying the replacement, which invites "did you mean
  `hero`?" — and `frontmatter.rs:487` had already ruled that phrasing out, because
  `codes::extract_suggestion` lifts it into a structured fix an agent applies mechanically. `about:`
  → `hero:` is a rewrite (different sub-keys), so the mechanical rename would have traded a warning
  for a document broken in a new way. **Read the neighbouring decision before copying an item's
  proposed signature.**
- **The item flattened a scope.** `number-within` was a `theorems:` **sub-key**, never a top-level
  one, so the registry entries carry the scope label they were retired from. A retired-key table
  without scopes would recognize it in the wrong map.

Also kept: the messages append to the classified prefix rather than replacing it, because
`codes::classify` resolves TAL-FM-KEY off the `unknown <scope> key` substring — so neither fix needed
a new diagnostic code, explanation, or regenerated `docs/DIAGNOSTICS.md`.

56. **L5-1 residual: the manual's cross-page references, not its metadata.** The `description:`
    half **SHIPPED 2026-07-26**: 0 of 36 tracked pages → 36 of 36. Both figures the item carried
    were artefacts, and the re-measure it asked for is what caught them — the "3" counted
    `description:` lines inside fenced examples that *document* the key, and the "37th" page is
    `docs/guide/_book/index.tmd`, a build-output copy under a directory discovery already skips.
    **Two things this half taught, since neither is history:**
    - **A front-matter key can render as visible prose.** `description:` is not only metadata: it
      emits a lede under the H1 (`render/mod.rs:1312`) as well as `<meta>`/og, the book landing's
      Contents annotation (`site/book_toc.rs`) and search text. 13 descriptions drafted *from* each
      page's opening paragraph therefore printed directly above that paragraph. **The browser showed
      it; no grep would have.** Check what a metadata key *renders* before writing 36 of them.
    - **Grepping the manual for a front-matter key hits the manual's own documentation of it.**
      Any coverage figure over `docs/` must parse the leading front-matter block, not match a line.

    **What is left is not the authoring pass the item assumed, and splits two ways:**
    - **Glossary, term index and float digest have no surface to feed.** `glossary`, `term-index`
      and `float-digest` grep to **zero** across `crates/core/src` + `crates/server/src`, so
      "they render empty until an authoring pass happens" describes a *feature proposal*, not
      authoring work. Writing `{.definition}` blocks today feeds only `skim.rs`, which reads them
      as statement heads.
    - **Backlinks ship and render nothing, and authoring genuinely could fix that.**
      `site/backlinks.rs` builds its reverse index from **cross-page** xref markers; the books' 33
      xrefs (17 guide + 16 internals) are all intra-page, so **0** "Referenced by" lines are emitted
      in either book. Real cross-chapter references would light it up, but they have to be
      references someone means, which is a writing judgment rather than a sweep.

#### B. Buildable, but low yield on its own — **empty**

An item here is cheap to build and therefore easy to build *without asking whether it should be*. Two
of the last three closed on **evidence rather than code**, which is the outcome this band is most
likely to produce.

#### C. Blocked on an owner ruling (not a task until then)

25. **Pre-public release: one decision, parked on a date** (detail:
    [2026-07-17-security-release-audit.md](2026-07-17-security-release-audit.md)). All five code items
    shipped 2026-07-25. **oss-4 — ruled 2026-07-25: deferred, and the public flip with it.** The owner
    is not going public yet ("I'll do it at the end of summer; before that I want to hone the tool to
    its final form"), so this gates no other work. Re-ask when a flip date is set; the question then is
    whether to prune `notes/` + `docs/superpowers/` (no secret is exposed — the `--host` token design
    doc discloses only a per-session UUID mechanism — but it is a curated bug roadmap).
    **Verified NOT open, do not re-scope:** `SECURITY.md` exists, the tracked `/home/bogo` paths are
    scrubbed, and PT-1 / PT-2 / NET-1 / OUT-1 / DEP-01 / DEP-02 all shipped 2026-07-17. Refuted by the
    audit: `dos-yaml` (libyaml rejects the alias bomb in ~30 ms — the guard is in the C library, so
    grepping our source for it correctly finds nothing) and NET-3 (non-constant-time token compare).

#### D. Blocked on a device, a real user, or working-as-intended

Kept visible so they are not re-scoped. Revive on a real signal, not on capacity.

4. **Deck engine mobile polish** (P2): mobile pinch/pan + touch gestures (they matter for the phone-feed
   deck mode); drop `fitSlide` from the resize path (needs a lazy fit-on-show refactor first). *(The
   desktop trackpad half shipped 2026-07-24 — pinch / ctrl+wheel-down opens the overview map, with a
   250 ms hysteresis.)* **The device blocker is gone** (the author is testing on a phone) but this
   item is **still not measured**: the 2026-07-26 mobile audit covered chrome, affordances and the
   feed's scroll mechanics, and deliberately did not exercise pinch/pan or touch gestures. It stays
   in band D until someone measures the gestures on real hardware — Chromium touch emulation is not
   evidence for a pinch.

10. **Two kernel limitations with no clean fix** (P3, dev-facing):
    - **R cold kernels still orphan on ungraceful parent death.** IRkernel has no
      `ParentPollerUnix` equivalent, so there is nothing to arm; PDEATHSIG is the only other
      lever and is hazardous. R is rarely the cold single-doc path, and the warm-pool,
      cold-Python and `/tmp`-sweep halves all landed. `kernel.rs`.
    - **A tens-of-MB cell output blocks ZMQ receive before the cap fires.** `kernel.rs`.
      (The old note called this file do-not-touch; that was the completed rewrite-scoping
      list, not a freeze — see CLAUDE.md. It is still unfixed, just not forbidden.)

12. **i18n / Unicode: done bar a demand-driven residual.** The LSP UTF-16 fix shipped 2026-07-22
    (detail: [2026-07-22-i18n-unicode-sourcepos-audit.md](2026-07-22-i18n-unicode-sourcepos-audit.md)).
    *Residual, do not spin up without a real ask: RTL layout, CJK line-breaking, non-ASCII heading-slug
    collisions.*

18. **Demand-probe (interactive-explainer) residuals** (P3; detail:
    [2026-07-22-corpus-demand-probe-interactive-explainer.md](2026-07-22-corpus-demand-probe-interactive-explainer.md)):
    - **F-02 (gap, P3):** an authored numbered figure is emitted as `<img src="fig.svg">`, and an
      `<img>`-embedded SVG is style-isolated: it can't see `--tali-*` or the theme toggle, only the
      **OS** `prefers-color-scheme`. So a reader who forces the page theme opposite their OS gets the
      figure in the wrong palette. Inline `{js}`/SVG graphics on the same page track the toggle fine.
      Candidates: an inline-SVG figure path so `![](x.svg)` inherits page vars, or a documented
      neutral-palette convention. Edits `crates/core/src/render/figure.rs`.
    - **F-03 (WAI, authoring nuance):** a `{js}` "once" cell's returned node is mounted *after* the cell
      body runs, so an attachment-gated init (`if (!node.isConnected) return`) silently no-ops the first
      paint. Gate teardown on `invalidation`, not DOM attachment. Candidate: a doc line in the `{js}`-cell
      reference, or an optional post-mount hook.

41. **R graphics cannot follow the page theme; matplotlib figures can** (P3, M; detail:
    [2026-07-26-corpus-demand-probe-analyst.md](2026-07-26-corpus-demand-probe-analyst.md), AN-2b).
    Taliesin renders every inline matplotlib figure **twice** (light + dark foreground) and swaps them
    on the theme toggle (`kernel.rs`'s `MPL_THEME_PREAMBLE`); measured on `corpus/analyst/` the Python
    figure emits two genuinely different PNGs and the ggplot figure emits one, so a mixed-language
    report has half its figures track the reader's theme and half baked. **Blocked on being a feature,
    not a fix:** a real version re-renders the figure twice against two foregrounds. **Do NOT confuse
    this with AN-2a, which is fixed** — the R device no longer paints opaque white under a transparent
    figure; transparency lets the page show through, but the *ink* is still baked at one colour, and
    that is what is left here. The documented workaround (a neutral mid-grey palette) is the second
    instance of the convention named in item 18's F-02. Minor and separable: an R figure is emitted
    `<img alt="output">` where the Python pair is `alt=""`; both sit inside a captioned `<figure>`, so
    `alt=""` is right and `"output"` is noise read aloud.

#### E. Gated, not actionable now (do not spin up)

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

## Tier 3: demand-driven (below every band above; build only when a real user asks)

**Waits on demand, not on capacity.** The PMF audit's verdict is that what is missing is **real users,
not more features**, so nothing here is scheduled. One line each; the reasoning lives in the linked
audits.

- **An end-to-end live-HTTP test for `mounts:` serving.** The F-04 work unit-pins the pure
  `match_mount`/`resolve_project`/`classify_change` helpers and live mount serving is browser-verified;
  what is missing is only the bin-crate gap of a real `reqwest`/`TcpListener` harness. Mounts are
  preview-only, so this waits for a reason to exist.
- **Companion (Phase 2):** editor commands (insert block / reorder slide) — strictly `.tmd`-buffer text
  transforms in the editor, never preview gestures.
- **LaTeX hover-preview in the VS Code editor** — a sub-case of the LSP item below: a `HoverProvider`
  resolving `@fig-2` to "Figure 2", a front-matter key's doc, or a `[@key]` reference, over data
  `vocab`/`symbols` already carry.
- **`.tmd` format-on-save** (open question). A source pretty-printer would write the editor *buffer*
  (the allowed surface) but must preserve `data-sourcepos` line stability for click-to-source.
  Brainstorm whether the reflow is worth the click-to-source risk before any work.
- **Dogfood: migrate the FL-weather book to Taliesin** — a real Quarto to Taliesin migration +
  portability stress test (exercises `book.rs`, includes, the freeze cache, file-mode portability). If
  it renders clean, consider pinning a reduced version under `corpus/`.
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic, kernel-free and
  network-free).
- **`taliesin publish` follow-ups:** an optional `--init` wrapper for the one-time `wrangler` setup.
- **Interactive/explorable numerics** (`FEATURE-IDEAS.md` #62-66; none pinned; promote one only with a
  corpus pin).
- **Wave 5** (`ROADMAP.md`): print-pdf track (paged render *of* the built HTML), docs-as-spec (RFC-2119
  dialect + protocol reference), `{glsl}` cell-language registry, SEO completeness (sitemap/robots/JSON-LD
  at publish with `url:`).
- **Site-level shared bibliography + hygiene** (M). `bibliography:` is per-document only, so a growing
  blog retypes keys per post and nothing reports an unused or duplicate entry. Allow `bibliography:` in
  `_site.yml` merged under each page's own, plus two **read-only** diagnostics ("entry never cited",
  "duplicate key"). Explicitly does not touch the BibTeX parser / CSL formatter.
- **Author structure panel** (M/L). A read-only preview sidebar: the heading tree with per-section word
  count and a badge per node for unresolved xref / TODO / over-goal length; click to scroll. This is the
  *revision* view, not the reader TOC. Scope it as an annotation layer on the dev panel, or it grows to L.
- **Session revision digest** (M). Surface the `BlockOp` stream the client already receives: a session
  word delta (`+340 / -180`) plus a feed of the last N ops, each click-to-source. Honest caveat: the pin
  is behavioural (a `tools/live-edit-bench` assertion), not a corpus doc.
- **Block-level transclusion** `{{< include file.tmd#sec-id >}}` (M). Reuse a section across a series
  without copy-paste drift. Must ride **on top of** the `includes.rs` source-map pass (resolve the
  fragment to a block range, hand the existing machinery a sub-slice), never rewrite it. Hard merge
  gate: the source map must not perturb.
- **LSP for the language intelligence, browser stays the view** (L). Everything an LSP needs is already
  in Rust (`check`, `vocab`, `register_xref`, the bib parser, `closest()`); it is write-once for
  Neovim/Helix/Zed/VS Code and removes the drift that causes the `#| label:` completion gap (JS regexes
  reimplementing Rust knowledge). An LSP cannot render the preview and does not need to.
- **Image optimization** (large): WebP/AVIF transcode + responsive `srcset` + lazy-load behind a
  content-hashed asset cache. Deferred until posts get image-heavy.
- **Marketing site** (deferred, feature-first; rolls into a demo-machine rebuild): `live-edit-hero-demo`
  clip; swap `site/_site.yml` placeholders; demo-led hero rebuild; mobile embed refine; deploy.
- **`serde_yaml` fallback watch-item:** the `Cargo.toml` workspace comment names `serde_yml`, which
  carries RUSTSEC-2025-0068 (unsound + unmaintained); `serde_norway` is 1+ yr stale. The maintained
  continuation is **`serde_yaml_ng`** (v0.10). No urgency (trusted local config; 0.9 still builds). If
  0.9 ever breaks against a future serde/edition, swap, gated on a test that `Error::location().line()`
  still works. Fix the stale comment when touched.
- **PMF demand-driven tail** ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md), Tier C): a
  document-level reader show/hide-code toggle, a reader code+data download affordance, instant
  client-side navigation polish. Each waits on a real ask.

## Quarto catalog (policy, not a task)

**Owner ruling 2026-07-16: no sweep. Triage an area on demand, when you next work that area.** Before
consulting it read the triage doc's "three layers" section
([2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md)): the entries are the asset
and were well-grounded on 2026-07-03, but the heading status is degenerate and the executive summary is
misleading. A skeptic verdict is evidence, never a ruling (its "drop Atom feeds" verdict was overruled;
Atom shipped with autodiscovery).

## Do not re-add / re-scope

**One line per entry.** The detail is in git, in [AUDITS.md](AUDITS.md), and in the dated findings
docs; look there rather than re-expanding this list.

### Shipped

- **2026-07-26 path-parity batch (items 50, 51, 57, PP-1..3):** one document now renders the same
  whichever command renders it. The single-document containment root is decided once by
  `render_single_doc` (nearest `_site.yml`, else the doc's own directory), so `build <page>` and
  `build <site>` emit the same document while PT-2's climb-out-of-a-checkout escape stays closed;
  `TOC_SHEET_MARKUP` is the single copy of the mobile-sheet chrome all four assemblers emit; and the
  single-doc preview ships Cmd-K on the same terms a build of that document ships it. **Do not
  re-scope as "give the single-file build the inferred root"** — that is a revert of `9359a2c`.
- **2026-07-26 mobile batch (items 42-49, MOB-1..8):** the tree now asks what device it is on
  (`hover`/`pointer` media features; it had none). Deck menu drops its keyboard legend + hint badges
  and gates Speaker view on capability instead of orientation; the ⌘K badge is hidden on touch at any
  width; copy-code shows and the heading anchor dims on touch; the book drawer locks page scroll and
  keeps focus through outline hydration; touch nav targets grow by overlay; the sticky book topbar
  truncates instead of wrapping. Three pages stopped instructing a keyboard about the deck above them.
- **2026-07-26 owner rulings (items 24, 17, 2):** `section-extents` shipped as option (b) —
  `data-section-end` on every heading block, extents nesting, heading-inclusive, stopping before
  generated furniture, decks excluded; `book-breadcrumb` ruled **no** (D114 stands); a vendored MIT
  PowerShell `.sublime-syntax` consulted last; deck presenter tools declined again.
- **2026-07-26 reporting surfaces (items 39, 40):** AN-5 (an unnumbered cross-page `@sec-` now names its
  target instead of rendering the bare word "Section"), AN-6 (per-document validation no longer reports
  valid cross-page refs as `TAL-XREF-UNDEF`; scope, not severity), AN-3 + AN-4 documented.
- **2026-07-26 demand probe #4, the analyst** (`corpus/analyst/`, the only corpus project running two
  languages in one document): AN-1 (a labelled `tbl-` cell with no `<table>` no longer emits a dangling
  xref) and AN-2a (`KernelSpec::r` carries `options(repr.plot.bg = "transparent")`) fixed.
- **2026-07-26 audit batch:** AP1-R1 (the freeze cache was capped by entry *count*, never by bytes; a
  16 MB `MAX_BYTES` budget now bounds it) and DOCS-2/3/4/5 (`about:` purged from 28 places across 6
  guide pages nine days after its removal, plus three smaller drifts).
- **2026-07-25 band-A batch:** AP7-1..5 (a11y), AP3-1 (a bypass lane for cell-free rebuilds), AP11-1
  (`TAL-KERNEL`), DIAG-1 (eight diagnostics catalogued + a zero-`GENERIC` gate), DOCS-1.
- **2026-07-25 band-B batch:** AP3-3 (the port re-roll, above), PA-M3 (listing list semantics), PA-M13
  (`image:` without `image-alt:` warns), PA-H1's residuals (deck `theme-color` + social meta).
- **Earlier, closed:** the backlink-context + resume batch, the book-wayfinding batch, the hardening
  batch, book-level `theorems:`, live-executor mounts (F-04), structure-preserving book-aware `read`,
  AP8-1's output scrub, the DET-1 reproducibility guard, the DX audit batch, `taliesin lsp`, DX17(a)+(b)
  headless executed output, the deck audit, the polish audit batch, the PMF builds, corpus-coverage, the
  machine-facing audit, AI-native packaging, the R/Python ANSI leak, ungraceful-death reaping, and the
  `assets/js` `tsc` gate.

### Decided against

- **Deck presenter tools** (one-command publish, laser/spotlight, auto-advance): declined 2026-07-22 and
  **re-declined 2026-07-26** on the same grounds — no real speaker ask has appeared. Revive only when the
  author actually presents from Taliesin. (`footer:`/`logo:` from that item did ship.)
- **WS op-message batching** (declined 2026-07-25 **on measurement, premise confirmed**): the worst case
  is 55 ops / 53 `SetMeta` in one frame each, but a warm edit is 32.2 ms of which the diff is 0.94 ms, so
  batching saves ~220 bytes on a 32,303-byte payload (0.7%) and 54 handler dispatches, none on the
  critical path. Reopen only if render cost drops far enough that framing is measurable.
- **Item 29's reduction residuals R1 + T2** (closed 2026-07-25 without code): R1's `text_content` /
  `indexable_text` fork is deliberate and equalizing them would leak raw entities into `llms.txt`; T2's
  "three modules pre-scan" is partly rotted — the real duplication is a six-line idiom in two places, and
  the divergence that looked like a latent bug is unreachable.
- **Deck-motion, whole item** (detail: [2026-07-24-deck-motion-audit.md](2026-07-24-deck-motion-audit.md)):
  Option A + residuals shipped; **(3) no-change** ruled; **(4) Option C (shared-element FLIP) declined —
  do not re-cost it a third time**. A coverage-weighted refinement of (5) measured *worse* (15 of 25
  slides vs 23 of 25); do not re-refine without measuring.
- **A separate per-page outline artifact for the book drawer** (declined 2026-07-25 while building it):
  the index it would duplicate is 172 KB raw / 60 KB gzipped on `docs/internals` and is already
  lazy-loaded on every page, so a sidecar buys ~55 KB gzipped on one cached subresource in exchange for a
  second copy of the render recipe, assembly, invalidation, route and build write.
- **`drawer-typeahead`** (declined 2026-07-25): Cmd-K plus the drawer's collapsible outline covers it, and
  a second search-like box beside a Search button is a discoverability smell.
- **A "~N min read" label on a book chapter** (2026-07-25): `prose::word_count` excludes fenced code and
  math, so a code-heavy chapter is understated — and reading code is *slower* than prose, so the error
  goes into a promise about the reader's time in the wrong direction, on exactly the chapters this tool
  exists for. (The dated-post estimate in `render/mod.rs` is a different surface; `is_article` is
  test-pinned, do not touch it.)
- **Flipping a book chapter's label to prefer `title:` over its `# H1`** (resolved 2026-07-25): measured
  across every book in the repo, only 3 of 48 chapters differ and in 2 the `# H1` is the *better* nav
  label. Resolved as documentation, not code; nothing is searchable by only one name.
- **CAD-as-code** (`{openscad}` / CadQuery cell → live 3-D preview; researched 2026-07-23, NOT built):
  technically feasible and legally green, killed on **demand**. **Do not bundle openscad-wasm (GPL).**
  Five named revisit triggers in [2026-07-23-cad-as-code-research.md](2026-07-23-cad-as-code-research.md),
  the first of which is simply author-pull with a named pin doc.
- **2026-07-22 rulings:** DX16 update-nudge = **skip** (a version check is network egress that undercuts
  the offline-first identity); cross-ref label i18n = **defer** (no corpus doc demands it); item 9's
  design questions documented as intentional (the deck serif/sans inversion, no `//| uses:` alias, the
  callout-namespaced / theorem-bare asymmetry).
- **2026-07-12 wishlist cut to `FEATURE-IDEAS.md`** (revive only when a corpus doc needs one):
  cross-revision diff, repro manifest, List-of-Figures/Tables/Theorems, interactive tables, line-level
  code xrefs, image `dark=`. Reader text-size/line-spacing controls declined (a11y-exempt substrate in
  `14-reader-prefs.js`).
- **TODO / FIXME surfacing** (owner ruled 2026-07-10): no `level` concept exists, so a TODO warning would
  fail `check` on every draft. If revived, a preview-only `Diagnostic::info` beats re-plumbing a real
  `level`, and the scan must NOT reuse `prose::strip_inline` (it blanks code, where TODOs live).
- **AI-native leftovers declined 2026-07-16:** `check --online` citation resolution (the only proposed
  network egress; check-only and off by default if ever revived), the numeric-claim-without-citation hint
  (its own spec rates it FP-prone), and a per-page text/JSON sidecar (redundant).
- **Refuted by measurement (do NOT re-scope):** heading demotion **already ships** (AP9's "12 `<h1>`"
  measured a stale gitignored build artifact; the only multi-`<h1>` corpus docs are decks, exempt by
  design); `build` does not leak forkserver subtrees; the warm pool booting Python on prose-only builds
  is hygiene, not latency; dev attributes are 0.29% of page bytes (don't strip); a `--version -dirty`
  marker is stale-by-construction; the `assets/css` stale-embed claim did not reproduce (re-verify for
  `assets/js` before any touch-render workaround); the 390px `hero:` overflow + theme/video desync are
  fixed; include symlink-loop SIGABRT does not exist (Linux caps at `MAXSYMLINKS=40`).
- **`_redirects`/`_headers` preserved, never generated** (`build.rs` treats them as author-placed deploy
  metadata; `stale_sweep.rs` pins it).
- **Gate the gate:** a drift test that cannot fail is worse than none. Any new drift gate must be
  mutation-checked against exactly the shape it guards.
- **Library outsourcing decided against** (each verified vs the invariants): hayagriva/biblatex, schemars,
  jsonschema, morphdom/idiomorph, similar/dissimilar, clap, owo-colors, slug, html-escape,
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
features. When publishing, lead the copy with the **speed moat** (warm server, block-level incremental,
no per-edit rebuild), the single most-repeated Quarto grievance and the most under-marketed asset.
