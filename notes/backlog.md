# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git + [AUDITS.md](AUDITS.md) +
> [ROADMAP.md](ROADMAP.md); delete an item when it lands, don't leave a `[x]`. The "do not re-add"
> list near the bottom is a compact anti-rot guard, **one line per entry**, not a changelog — if an
> entry there needs a paragraph, the paragraph belongs in its dated findings doc.

## State (2026-07-26)

**Band A holds 16 items (4 HIGH, 8 MEDIUM, 3 LOW, plus one authoring pass), filed 2026-07-26 by five
rounds in one day:** the mobile audit (42-49), path parity (50, 51, 57), and the L2/L3/L4/L5 lens
batch (52-56). Band B is empty; band C holds only item **25**, parked on a public-release *date*
rather than on a decision; the rest is blocked on a device or a real user (band D) or gated (band E).

**They are four or five batches, not sixteen sessions** — most share a root cause. The suggested
order is at the top of band A; start with the mobile batch (it holds every HIGH), then path parity
(item **57** unblocks the mutation re-run). Read the band-A preamble before starting, the two traps
under "The mobile audit RAN" before opening a browser, and the 2026-07-26 probe traps under Standing
constraints before writing any probe.

**A lens menu now exists** ("Proposed audit lenses", below): six never-run lenses (L1-L6), four
re-runs ranked by age × measured churn in each round's own surface, and four directions that the last
weeks' work has *unblocked*. As of 2026-07-26, **L1, L2, L4 and L5 have run** (items 50-51 and 52-56);
**L3 is partial** (only `headless_js.rs`; `lsp.rs`, `complete.rs`, `skim.rs`, `manifest.rs` unread);
**L6 is blocked** on a repository that is not on this machine; and **none of the four re-runs has been
done** — the mutation re-run is the one worth scheduling, since it is a long compute job rather than a
read.

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
  - **cargo-mutants:** scoping its test command to `--lib` reports MISSED for everything an
    integration test covers (measured: 102 MISSED / 0 CAUGHT, all artefact). It also writes
    `mutants.out/` into the repo root, which is **not in `.gitignore`** — pass `--output` somewhere
    outside the tree. And its scratch copy carries no `.git`, which is why the baseline is red (item
    **57**).
- **Calibrate a new lint against real output before writing it.** Measuring the proposed
  `TAL-SHAPE-*` rules over all 14 site projects killed four of their own prescriptions, including
  the most valuable one (it fired on 11.8% of the corpus, essentially all false positives) and one
  whose stated justification did not exist in the tree.

## Open work (priority order: take from the top)

### The mobile audit RAN on 2026-07-26 — its findings are in band A

Detail: [2026-07-26-mobile-audit.md](2026-07-26-mobile-audit.md). The author's reported symptom
reproduces, and **seven of its eight findings share one root cause: the tool never asks what kind of device it is
on.** Measured over every `.css`, `.js` and `.rs` file in `crates/` + `web-client/`: **zero**
`pointer: coarse`, **zero** `hover: none`, **zero** `any-pointer`. Every keyboard hint, hover-reveal
and presenter tool is gated on viewport *width* or on deck *layout mode* — two proxies that both fail
the same way, by treating a wide or stepped phone as a desktop.

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
  exist only on the `build`/`publish` path). Needs no device, no kernel and no network. **RUNNING
  2026-07-26** → [2026-07-26-path-parity-audit.md](2026-07-26-path-parity-audit.md).
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
- **The mutation / vacuous-test round (07-18) is the best value per token:** 41,149+/5,715- across
  `crates/` + `web-client/` since, including 26 new test files and 5 new subsystems, and mutation
  testing is the one lens that decays mechanically with new code. **Scope it cheaply:** `cargo-mutants`
  restricted to files added or changed since 07-18.
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

**Suggested order (2026-07-26), so a fresh session does not re-derive it.** The 16 items here are
four or five *batches*, not sixteen sessions, because most share a root cause:

1. **Mobile / touch (42-49).** Every HIGH on the board, one root cause, and four of them are the same
   edit at four sites. It came from real device use, which is the only source that produced a HIGH
   this month. **Do this first.**
2. **Path parity (50, 51, 57).** One root cause: page assembly is hand-wired at three sites with no
   shared owner. **57 is the keystone** — fixing the containment root also unblocks the mutation
   re-run, which currently cannot start at all.
3. **The mutation re-run**, which becomes possible once 57 lands. Mechanical yield on code that has
   never been mutation-checked; scope it to files changed since 2026-07-18 and heed the cargo-mutants
   traps above.
4. **Migration UX (53, 54)** and **deck weight (52)** and **hygiene (55)**: small, self-contained, any
   order.
5. **56 is an authoring pass, not code**, and can run in parallel with any of the above.

**Auditing is done for now.** Four fresh lenses on 2026-07-26 produced zero HIGH findings, while the
one round that produced four came from the author using the tool on a phone. The remaining menu
entries are the weak ones; the next *audit* worth running is real-device mobile **after** batch 1
ships, so it verifies rather than re-finds.


**The 2026-07-26 mobile audit's eight findings** (detail:
[2026-07-26-mobile-audit.md](2026-07-26-mobile-audit.md)). They share one root cause and are cheapest
built as **one batch**, because 1, 2, 3 and 4 are the same edit applied at four sites: replace a
width/mode proxy with `@media (hover: none) and (pointer: coarse)`. **Verify in a browser with real
viewport emulation, not window resize** (see the trap above), and pin what you can server-side —
remember the inlined-asset needle trap when writing any assertion.

42. **MOB-1 (HIGH): the deck menu hands a touch reader a keyboard manual.** On a 390×844 phone the
    menu renders a **125px "KEYBOARD" block listing ten shortcuts** (`→ ↓ Space`, `Home End`, `O`,
    `F`, `S`, `B`, `?`, `Esc`) plus `O`/`S`/`F` hint badges — about a third of the menu, pushing the
    controls that work out of reach. `deck.js`'s `buildMenu()` appends `KEYS_HTML` unconditionally
    and always passes `tool()`'s `hint`. **Oversight, not decision, and the same function proves
    it:** it already drops the theme row when embedded, hides Present outside the feed and hides
    Speaker *in* the feed — so in portrait, Speaker view is correctly hidden from Tools **while the
    legend still advertises `S` — Speaker view**. Fix: gate `.tali-menu-keys` (`deck.css:762`) and
    `.tali-menu-hint` (`deck.css:745`) on capability.

43. **MOB-2 (HIGH): rotating a phone gives it desktop affordances.** The gates that exist key on deck
    layout *mode*, and mode is chosen by orientation. Measured on one emulated phone, rotation only:
    portrait hides Speaker view, landscape (844×390, `pointer: coarse` still true) **shows it**, along
    with the badges and the legend. Speaker view opens a presenter window — a dead end on a phone, and
    `deck.css:632-633` already knows it. Fix: gate presenter-only tools on input capability, which is
    independent of feed/stepped.

44. **MOB-3 (HIGH): the ⌘K hint appears on any touch device wider than 640px.** `site.css:197` hides
    `.tali-search-kbd` under `max-width: 40rem`, and **its own comment states the intent as
    capability** ("meaningless on a touch phone (no ⌘/Ctrl key)"). Measured at 844×390 touch: the badge
    renders **"Ctrl K"**. `search.js:1064` compounds it — `IS_MAC` reads `navigator.platform`/
    `userAgent` with **no touch check**, so an Android phone is told to press Ctrl+K and an iPad ⌘K.
    Emitted at `site/chrome.rs:41` + `:506`. Keep `aria-keyshortcuts`; only the visible badge is wrong.

45. **MOB-4 (HIGH): copy-code and copy-link are invisible on touch.** `.tali-copy`
    (`base.css:394-401`) and `.tali-anchor` (`base.css:321-332`) sit at `opacity: 0`, revealed only by
    `:hover`/`:focus-visible`, with no `hover: none` fallback. **Copy-code matters more on a phone**
    (no easy selection across a scrolling `<pre>`) and is exactly where it cannot be found. Note the
    irony to avoid re-deriving it: `base.css:324-327` adds a 24×24 `::after` tap target citing WCAG
    2.5.8 — the size was solved *for touch* on a control that on touch never appears. Fix under
    `@media (hover: none)`: show `.tali-copy` persistently; `.tali-anchor` wants an author ruling
    (show, or drop).

46. **MOB-5 (MEDIUM): the book chapter drawer does not behave like a drawer.** At 390×844 it covers
    **93% of the viewport** over a backdrop, and: **(a)** page scroll is not locked —
    `scrollBy(0,400)` moved the article behind it by **328px**; `body` is `position: static` /
    `overflow: visible`, and the panel's `overscroll-behavior: auto` chains at either end. **There is
    no scroll-lock code anywhere in the client JS** — never implemented, not broken. **(b)** It is not
    a dialog: `role`/`aria-modal`/`aria-hidden` all absent and focus stays on `.tali-book-body` after
    opening, though a focus trap already exists (`code-enhance/04-focus-trap.js`, built for Cmd-K).
    **(c)** The close control is **26×22px**, under the 24px WCAG 2.5.8 AA floor on height. *Bounded:*
    backdrop tap-to-dismiss and Escape both work (verified), so this is a size defect, not a trap —
    (a) and (b) are the ones worth a session.

47. **MOB-6 (MEDIUM): the marketing site tells phone readers to press keys.** `site/index.tmd:121`
    ("press `F` for fullscreen") and `site/formats.tmd:42` ("click it, arrow through it, press `F`")
    — verified rendered at 844×390 touch, describing a keyboard the reader lacks about a deck directly
    above the sentence. Content fix. **Scope it precisely:** a *reference table* of shortcuts in the
    guide is legitimate on any device and is NOT in scope; this is an *instruction* about the widget
    on the page.

49. **MOB-8 (MEDIUM): the book topbar grows instead of truncating on narrow screens.**
    Author-reported, then measured: **the row never overflows horizontally at any width — it grows
    vertically**, because the title wraps. 390px → 1 line / ~48px; 320px → 2 lines / 56px; **280px and
    240px → 3 lines / 77px, 12-13% of the viewport**, and the topbar is *sticky*, so that is taken off
    every screen of reading. `.tali-book-brand` (`site.css:291-292`) is `display: block` with no
    `nowrap`, no `overflow: hidden`, no `text-overflow: ellipsis` and no `min-width: 0` — as a flex
    item its default `min-width: auto` refuses to shrink below content, so it wraps instead of
    truncating. **The same failure is already documented at `site.css:244-248`** (shrink pressure
    landing on the item with no min-content floor; that fix protected the icon and left the title
    exposed), and the correct idiom is already used at `deck.css:761`.
    **Fix = two parts, and the label alone is not enough.** Measured at 240px: the title needs
    **146px** for one line, the label + gap is worth **66px**, hiding it leaves **103px** — still
    wrapping. So: **(a)** give the brand `min-width: 0; white-space: nowrap; overflow: hidden;
    text-overflow: ellipsis` — this is the real fix and bounds the bar at one line for *any* book
    title at *any* width; **(b)** hide the `Chapters` label below ~**22rem** (measured: with the label
    the title holds one line to ~360px and wraps at 320px; without it, to ~280px). **Safe to hide:**
    the button is `<button aria-label="Chapters"><svg/><span>Chapters</span></button>`, so the
    accessible name survives `display: none` on the span — no `.tali-sr-only` needed.

48. **MOB-7 (LOW): the desktop nav is served to landscape phones.** The burger appears only under
    40rem, so at 844×390 a touch phone gets the full **7-link desktop nav at 26px** minimum link
    height (clears the 24px AA floor, misses 44px AAA), plus search and settings. Deck menu launcher
    is 34×34. Same root cause as MOB-3.

**Measured healthy in the same round — do not re-scope:** the phone slide-feed (A3) works
(`html.tali-feed`, `scroll-snap-type: y mandatory`, 10,972px over 14 slides, one-viewport snapping,
live rotation re-routing); zero horizontal overflow on any page or viewport; zero console
errors/warnings; the ⌘K badge *is* correctly hidden at 390px; body typography on a phone is right
(16px, 39 characters per line — the desktop ~70ch keep is unreachable at 390px and 39ch is normal
mobile practice). **Observation, not a finding:** 11 of 18 code blocks scroll horizontally rather
than wrap (`white-space: pre`, worst 368px); scrolled code is often better than wrapped code and the
affordance is visible, so this looks deliberate — it becomes an item only if the author disagrees.

**The 2026-07-26 path-parity round's two findings** (L1; detail:
[2026-07-26-path-parity-audit.md](2026-07-26-path-parity-audit.md)). Same shape as the mobile batch,
one layer up: page assembly is hand-wired at **three** sites with no shared owner (`render/page.rs`
for both static builds, `serve/mod.rs` for the single-doc preview, `serve_site/mod.rs` for the site
preview), and each finding is a line present in two of the three and absent from the third. Neither
breaks a built page; both break the *preview* being a faithful view of it, which is the loop the tool
is built around. **Cheapest built as one batch, and the batch should end with one helper the three
assemblers call rather than a third copy.**

50. **PP-1 (MEDIUM): the Cmd-K palette does not exist in a single-doc preview.** Browser-measured on
    one document across four paths: Ctrl+K opens the palette in the site preview, the standalone build
    and the site build, and does **nothing** under `preview <file.tmd>`. `search.js:1035-1044` binds
    Cmd/Ctrl-K on `document` unconditionally, so wherever the runtime ships the palette works, button
    or not (a standalone build has a *working, invisible* palette). Grepping all of
    `crates/server/src/serve/` for `SEARCH_JS` returns nothing, while `serve_site/mod.rs:856` injects
    it. **The comment stating the rule is now false:** `page.rs:507-512` justifies not gating Cmd-K on
    the TOC with "invisible to the author, because the preview injects both unconditionally" — the
    build-side half of that bug was fixed and the preview-side premise was never re-checked. Fix:
    inject `SEARCH_JS` beside `TOC_SPY_JS` in the single-doc shell.

51. **PP-2 (MEDIUM): the mobile TOC sheet does not exist in a site preview.** At an emulated 390×844
    phone, `body.tali-toc-sheet` is set in the single-doc preview, the standalone build and the site
    build, and **not** in `preview <dir>`; the handle is correspondingly invisible there and the TOC
    stays a desktop sidebar at phone width. The chrome (`<button id="tali-toc-handle">` + backdrop) is
    emitted at two hand-copied sites, `render/page.rs:353` and `serve/mod.rs:946`; `serve_site` has no
    copy. Two different runtimes drive it (`toc-sheet.js` in a build, `client.js:888-1001` in the
    single-doc preview), which is why it survived: each looks complete alone. This is AP7's recorded
    "not chased" note, now measured and attributed — and it hits the **book-authoring** path, so a
    book's phone reading experience is the one thing its author cannot see while writing it.

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

57. **PP-3 (MEDIUM): the two build paths disagree about `{{< include >}}`, and the test that covers it
    exercises neither.** `corpus/tech-blog/posts/pca-geometry/index.tmd` includes
    `../../_includes/three-scene.tmd`. The **site** build resolves it (no warning, `function
    makeScene3D` present). The **single-file** build of the same page drops it with `include not
    resolved (path escapes the project root)`, so the artifact ships without its 3D scene. Mechanism:
    `includes.rs:350` documents the containment root as "the nearest ancestor holding `.git` or
    `_site.yml`, else `base_dir` itself"; the site build passes the site root, the single-file path
    lets it collapse to the document's own directory, so any `../` climb escapes. **Two riders:**
    (a) `crates/core/tests/corpus.rs::includes_are_resolved_with_origin_files` is green because it
    calls `render_document_with_includes` directly, a path `build <file>` does not use — an assertion
    true of the library and false of the product; (b) that test's result **depends on `.git` being
    present**, since root inference walks for it, so the same test fails in any export, vendored copy
    or `docker COPY` without VCS metadata (verified by leaking a copy: inputs byte-identical, fixture
    present, test still red). **This is what blocks the mutation re-run** — cargo-mutants will not
    test mutants while the unmutated baseline is red, and it is red only because its scratch copy has
    no `.git`. Fix: give the single-file build the same inferred root, and make the test exercise the
    CLI path or pass an explicit root.

**The 2026-07-26 L2/L3/L4/L5 round** (detail:
[2026-07-26-lenses-l2-l5-audit.md](2026-07-26-lenses-l2-l5-audit.md)). Four lenses in one session
after L1 closed. Ordinary-page performance came back healthy on a throttled phone (every LCP inside
the 2,500 ms band), so the items below are the outliers, not a general problem.

52. **L2-1 (MEDIUM): a deck in a site build ignores `_assets/` and re-inlines the whole framework.**
    Measured on a site whose only deck draws one mermaid diagram: `talk.html` is **4,583,261 bytes**
    (1,375,317 gzipped) and links `_assets/` **zero** times, while the ordinary page beside it is
    24,718 bytes and links the shared, content-hashed assets — so **mermaid ships twice in one output
    tree**, and a second deck would ship a third copy. The fixed per-deck duplicate is ~1 MB raw /
    ~390 KB gzipped (measured by removing the mermaid block: 1,011,028 / 396,685). Over Slow 3G + 4×
    CPU the deck takes **94.0 s** to load against 10.7 s for a 365 KB page. **Name the trade-off:** a
    *standalone* deck should stay self-contained (that is the artifact you hand someone, and
    `site/mod.rs` deliberately builds an embedded deck as a standalone document). The narrow fix is to
    let a deck page inside `build <dir>` take `AssetMode::External` like every other page in that
    build, leaving the standalone path untouched.

53. **L4-1 (MEDIUM): a pre-rename `_quarto.yml` is invisible and `check` reports "no problems
    found".** The config file renamed on 2026-06-24 is not looked for, so a project still carrying it
    builds with its configuration silently defaulted (the file's `title:` is dropped; the page keeps
    only its own). Everything downstream is healthy — rename it to `_site.yml` and the linter
    immediately errors `unknown config key 'project'` — so the whole gap is a missing existence check:
    "found `_quarto.yml`; the project config is now `_site.yml`". **Where:** the config load is
    `crates/core/src/site/config/mod.rs:178` (`root.join("_site.yml")`); the sibling check belongs
    beside it, next to the existing `is_malformed_config_warning` / missing-config distinction that
    `malformed_site_yml_pushes_tagged_warning_distinct_from_missing` already pins.

54. **L4-2 (LOW/MEDIUM): removed vocabulary is indistinguishable from a typo.** `about:` (removed
    2026-07-17, superseded by `hero:`) and `number-within:` both report `unknown front-matter key`,
    the same message a misspelling gets, with no pointer to the replacement. There is **no retired-key
    registry in the tree** (`RETIRED`/`REMOVED_KEYS` grep to zero); the removals live in comments and
    tests. Conspicuous because the did-you-mean culture is otherwise thorough. Fix: a
    `RETIRED_KEYS: &[(&str, Option<&str>)]` consulted before the unknown-key warning. **Where:**
    `crates/core/src/frontmatter.rs` — `KNOWN_KEYS` is the vocabulary at `:19` and the sibling consts
    (`UNSUPPORTED_KEYS:78`, `EXECUTE_KEYS:82`, …) are the pattern to copy; the two known retirees are
    named in comments at `:725` (`number-within`) and `:817`/`:1280` (`about`, removed at `dcf0588`).

55. **L3-1 (LOW/MEDIUM): the headless `{js}` observation is not bounded end to end.** `tokio::time::
    timeout` bounds the eval (`headless_js.rs:312`), but `Browser::launch`, navigation,
    `browser.close()` and `browser.wait()` are unbounded, and the only call site is a bare
    `rt.block_on(...)` (`query.rs:371`). The module's contract covers a launch/navigation/eval
    *failure*, not a **hang**, so a wedged Chrome hangs `taliesin read --run-js` with no diagnostic.
    The pattern already exists (`TALIESIN_CELL_TIMEOUT`). Fold in **L3-2 (LOW)**: `.no_sandbox()` at
    `headless_js.rs:260` is unconditional with no recorded justification (probably correct, but every
    comparable decision here carries its reasoning).

56. **L5-1: the tool's own manual ships without the metadata it derives reader surfaces from.**
    Measured 2026-07-26: **3 of 37** dogfood pages set `description:` (`docs/guide` 3 of 22,
    `docs/internals` 0 of 15) against **12 of 19** in `corpus/tech-blog`; 31 xrefs across 37 chapters
    and **one** `{.definition}` block in ~65,000 words. The books are what a prospective user reads
    and they are the pages with no meta description and the weakest search text. **Authoring pass, not
    code.** The skimmability round recorded "0 of 37"; today it measures 3, so re-measure rather than
    trusting either figure.

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
