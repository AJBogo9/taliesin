# Taliesin audit records

The current deep audit + its active detail. The build-ready queue lives in
[backlog.md](backlog.md); older audit rounds (pre-2026-07-07) are archived in
[AUDITS-archive.md](AUDITS-archive.md).

## Round index — every dated findings doc in `notes/`

**Check here before starting an audit.** This file carries narrative entries for the recent deep
rounds (below); this table is the complete list, so a session can tell at a glance whether a lens has
already been run. Nineteen rounds had no ledger line at all until 2026-07-25, which is exactly the gap
that let *completed* rounds (AP2, AP4) read as "recommended first" in the backlog. Add a row the day
you run one.

| Round (doc) | What it was | Where its work went |
|---|---|---|
| [2026-07-08-ui-audit-findings](2026-07-08-ui-audit-findings.md) | UI-audit sweep | landed; see section E |
| [2026-07-09-polish-audit-findings](2026-07-09-polish-audit-findings.md) | polish / productivity | landed; see section E |
| [2026-07-09-ui-audit-findings](2026-07-09-ui-audit-findings.md) | UI-audit sweep | landed; see section E |
| [2026-07-11-website-design-audit](2026-07-11-website-design-audit.md) | website/brand ("Marginalia"), 99 findings | standing constraint in backlog.md; many landed |
| [2026-07-12-ai-native-backlog](2026-07-12-ai-native-backlog.md) | AI-native authoring | **closed**, section G |
| [2026-07-13-companion-check-bug](2026-07-13-companion-check-unexpected-output-bug.md) | companion JSON version-skew bug | **closed** (`b40ec0e`, verified in the installed bundle 2026-07-25); release-hygiene residuals in Tier 3 |
| [2026-07-16-machine-facing-audit](2026-07-16-machine-facing-audit.md) | machine-facing output | landed |
| [2026-07-17-backlog-truth-sweep](2026-07-17-backlog-truth-sweep.md) | backlog-vs-source truth sweep | method, not findings; source of the "entries rot" law |
| [2026-07-17-reduction-audit-map](2026-07-17-reduction-audit-map.md) | simplification / reduction | Phase 2 + T1 + R2 landed; R1 + T2 = item **29** |
| [2026-07-17-security-release-audit](2026-07-17-security-release-audit.md) | pre-open-source security + supply chain | 4 findings landed same day; the deferred set shipped 2026-07-25; `oss-4` = item **25** |
| [2026-07-18-pmf-audit](2026-07-18-pmf-audit.md) | product-market fit | the Tier 3 band ("real users, not more features") |
| [2026-07-21-vscode-devx-audit](2026-07-21-vscode-devx-audit.md) | VS Code companion DevX | Tier 3 Companion (Phase 2) |
| [2026-07-22-ap2-robustness-fuzzing-audit](2026-07-22-ap2-robustness-fuzzing-audit.md) | **AP2** fuzzing / hostile input | AP2-1/2/3 all shipped 2026-07-25 |
| [2026-07-22-cache-correctness-audit](2026-07-22-cache-correctness-audit.md) | **AP4** adversarial freeze | AP4-1 shipped 07-22; AP4-2/3/4 shipped 07-25 |
| [2026-07-22-demand-probe-course-author](2026-07-22-corpus-demand-probe-course-author.md) | demand probe, persona 1 | item **16**, closed — this row claimed "F-03 open" after item 16 had already been deleted from the backlog. Re-measured 2026-07-26: `read` projects the embed as `[embed lecture.html: Embedded slide deck]` (not iframe chrome) and separates walkthrough steps as `[lines N]` blocks, which is exactly what F-03 asked for |
| [2026-07-22-demand-probe-docs-maintainer](2026-07-22-corpus-demand-probe-docs-maintainer.md) | demand probe, persona 2 | item **17**, **closed 2026-07-26**: F-01 shipped as a vendored MIT `.sublime-syntax`. Its recorded fix was wrong three times over (the licence file is `LICENSE` not `LICENSE.txt` and is plain MIT; `plist-load` is both already enabled and irrelevant; syntect loads `.sublime-syntax` only, so the filed `.tmLanguage` was unusable). F-02 was never a defect |
| [2026-07-22-demand-probe-interactive-explainer](2026-07-22-corpus-demand-probe-interactive-explainer.md) | demand probe, persona 3 | item **18** (F-02, F-03 open) |
| [2026-07-26-demand-probe-analyst](2026-07-26-corpus-demand-probe-analyst.md) | demand probe, persona 4 — **closes the slate 4/4**; the un-probed shape was *two languages in one document*, not volume, and both defects were "the R arm of a two-arm facility was never built" | AN-1 + AN-2a shipped 07-26; AN-3/4/5/6 (items **40** + **39**) shipped later the same day; only AN-2b (item **41**) is open |
| [2026-07-23-cad-as-code-research](2026-07-23-cad-as-code-research.md) | CAD-as-code feasibility + market | **decided against** (feasible + legally green, no demand); 5 revisit triggers in the doc |
| [2026-07-24-deck-motion-audit](2026-07-24-deck-motion-audit.md) | deck overview + every animation | Option A + both residuals shipped; 3 owner decisions = item **28** |
| [2026-07-25-ap7-accessibility-audit](2026-07-25-ap7-accessibility-audit.md) | **AP7** deep a11y of the output | was item **34**; **all five findings shipped 2026-07-25** (see below). Static surfaces came back sound; the defects were all "content changes without an announcement" |
| [2026-07-25-ap3-concurrency-audit](2026-07-25-ap3-concurrency-audit.md) | **AP3** concurrency / race conditions | was item **35**; every predicted race refuted (single builder task, task-owned pool, atomic freeze writes), the real cost was head-of-line blocking — **AP3-1 shipped 2026-07-25**; AP3-3 (a 1-in-13 kernel-test flake) is the only residual, folded into item 10 |
| [2026-07-25-ap11-chaos-audit](2026-07-25-ap11-chaos-audit.md) | **AP11** chaos / failure injection | was item **36**; degradation paths are well-built (corrupt cache self-heals, unwritable output exits 1), the defect was wording — **AP11-1 shipped 2026-07-25** |
| [2026-07-25-ap6-cross-browser-audit](2026-07-25-ap6-cross-browser-audit.md) | **AP6** cross-browser (the last AP slot) | **no findings**: Firefox and Chromium byte-identical on every measured axis, 0 console errors. Coverage gaps (WebKit, mobile, the preview path, Windows/macOS) listed in the doc |
| [2026-07-25-diagnostics-and-docs-drift](2026-07-25-diagnostics-and-docs-drift-audit.md) | two non-AP lenses: diagnostic-message quality + docs drift | was items **37** + **38**; **both shipped 2026-07-25**. The fall-through count was 6 in the doc and **8** in fact — the zero-GENERIC test found `.code-walkthrough` and, separately, the two build-only execution diagnostics the check-side sweep structurally could not see |
| [2026-07-26-lenses-l2-l5-audit](2026-07-26-lenses-l2-l5-audit.md) | **L2 + L3(partial) + L4 + L5** off the menu, one session | **five findings, items 52-56.** L2: ordinary pages are fast even at 4× CPU (every LCP inside the 2,500 ms band); the one outlier is that a **deck in a site build ignores `_assets/`** and re-inlines everything, so mermaid ships twice in one tree and the deck needs 94 s over Slow 3G. L4: a pre-rename **`_quarto.yml` is invisible** (`check` says "no problems found" while the config is silently defaulted), and removed keys (`about:`, `number-within:`) are reported identically to typos with no retired-key registry anywhere. L3: `headless_js.rs` is well-built but its browser wait is unbounded end to end. L5: **3 of 37** dogfood pages set `description:` against 12 of 19 in the blog corpus. **Instrument trap:** raw CDP `Network.emulateNetworkConditions` silently no-ops — a "throttled" number that is not slower than the unthrottled one is a broken probe, not a fast page |
| [2026-07-26-path-parity-audit](2026-07-26-path-parity-audit.md) | **L1** path parity (feature × emission path), the first lens off the new menu | **three findings, items 50 + 51 + 57 — all SHIPPED 2026-07-26.** The preview is not a faithful view of the built page, and each preview path is unfaithful differently: Cmd-K is missing from the single-doc preview, the mobile TOC sheet from the site preview. One root cause: page assembly is hand-wired at three sites (`page.rs`, `serve/mod.rs`, `serve_site/mod.rs`) with no shared owner, so every divergence is a line present in two of the three. **PP-3 (item 57) was added later the same day** and is the first of this round where the *content*
differs, not the chrome: a **single-file** build drops a `{{< include ../../… >}}` that the **site**
build resolves, because the containment root documented at `includes.rs:350` (nearest ancestor with
`.git`/`_site.yml`) is only passed by one caller. Its test is green through the library API the CLI
does not use, and depends on `.git` existing — which is what **blocks the mutation re-run**, since
cargo-mutants' scratch copy carries no VCS metadata and its baseline goes red. Validated the lens
itself: DX1, AP7 and DIAG-1 had each tripped over one instance and stopped. **The other two assemblers came back clean**: decks are identical across all four deck paths (same 20-method facade, 18 slides, runtime `theme-color`, same slide after `ArrowRight`), `mounts:` differs from direct serving by 4 bytes (boot nonce + ws path) with 0 failed requests, the `{{< embed >}}` iframe matches in build and preview, and `--bare` refuses a deck outright. A static grep called deck `theme-color` missing on all four paths, a false regression: `deck.rs:240` creates that meta at runtime, so the only valid needle is the rendered result |
| [2026-07-26-mobile-audit](2026-07-26-mobile-audit.md) | **mobile / touch reader experience** — author-proposed from real device testing; the first new lens after the AP slate closed | **eight findings; seven of them share one root cause**: the tree has **zero** input-capability queries (`pointer: coarse` / `hover: none` / `any-pointer`), so every keyboard hint, hover-reveal and presenter tool is gated on viewport *width* or deck *layout mode*. Filed as items **42-49** (band A); **all eight SHIPPED the same day** (`f9c7724` + `5b3921b`), items deleted. The phone slide-feed, page overflow, console and mobile typography all measured healthy. Two traps recorded: `resize_page` floors at ~500px (use viewport emulation), and the deck feed flag lives on `html`, not `.tali-deck` — probing the wrong element made a working feed look dead |
| [2026-07-26-ap1-residual-and-docs-behaviour](2026-07-26-ap1-residual-and-docs-behaviour-audit.md) | the last two unstarted lenses: **AP1's residuals** + the **behavioural** half of the docs lens | **both shipped 2026-07-26**. Each entry was wrong about where its own defect was: the kernel does NOT leak (it saturates over 1,000 executions) — the unbounded growth was Taliesin's own freeze cache, capped by entry *count* and never by *bytes*; and the docs gates covered flag/env *existence* while the front-matter vocabulary had drifted totally (`about:` removed 2026-07-17, documented for nine more days) |

Rounds with their own narrative entry below (and so already in the ledger): AP1, AP5, AP8, AP9, AP10,
AP12, the 2026-07-19 polish audit, the 2026-07-18 vacuous-test audit, the 2026-07-24 skimmability
audit, and the 2026-07-07 multi-surface deep audit.

**All twelve AP slots are run**, and as of 2026-07-26 **so is every proposed non-AP lens.** Both of the
two that were still unstarted — AP1's unchased residuals and the *behavioural* half of the docs lens —
ran on 2026-07-26 and both of their findings shipped the same day. With the analyst probe's own
reporting-surface findings (AN-3/4/5/6) shipped that afternoon too, bands A and B of
[backlog.md](backlog.md) are empty and **that file now has no code in it at all**. The table above is
therefore a complete record, **not a menu**: a further round needs a *new* lens proposed first, not
one taken off a list.

**One such lens was proposed and RUN on 2026-07-26: the mobile audit** (row above; detail
[2026-07-26-mobile-audit.md](2026-07-26-mobile-audit.md)). The author reported it from real device
testing, and it refilled band A with eight findings after that band had been empty. Its result is one
root cause behind every symptom: **there is not a single input-capability query in the tree** — zero
`pointer: coarse`, zero `hover: none`, zero `any-pointer` across every `.css`/`.js`/`.rs` file in
`crates/` + `web-client/` — so keyboard hints, hover-reveals and presenter tools are all gated on
viewport *width* or deck *layout mode*, two proxies that both fail by treating a wide or stepped phone
as a desktop. It was the right shape on the evidence: a **crossed dimension** rather than a stacked
feature, and three earlier rounds (AP6, AP7, backlog item 4) had each named mobile as *their* explicit
out-of-scope gap rather than measuring it.

**The next lens is already named by that round's own "Not measured" section:** real iOS Safari and
Android Chrome (the mobile round was Chromium emulation, which models viewport/DPR/pointer but not
WebKit, momentum scrolling, the dynamic viewport toolbar or safe-area insets), a phone screen reader,
tablet widths, and the `--host` QR phone-preview flow — a first-class phone feature with no coverage
at all. **Run it AFTER the mobile batch ships** — it shipped 2026-07-26, so this is now UNBLOCKED and verifies rather than re-finds. Start with the drawer scroll lock: a root `overflow: hidden` is known to hold less completely on iOS Safari than on Chromium, and only Chromium was measured.

**Stop-auditing ruling, 2026-07-26.** Five rounds landed in one day and band A now holds 16 items.
**The four fresh lenses run that day (L2, L3, L4, L5) produced zero HIGH findings**; every HIGH on the
board came from the mobile round, which was not a lens at all but the author using the tool on a real
phone. The code-reading lenses now return "healthy" more often than not, and this day's positives were
substantial (decks pass path parity outright, `mounts:` differs by 4 bytes, the LSP shares `check`'s
one diagnostic path, ordinary pages are fast at 4× CPU, the web manifest's suspicious white is
deliberate and pinned). **The recommendation recorded here is to build, not to audit**: the remaining
menu entries are the weak ones, and an audit's value decays to zero if its findings never ship. The
two exceptions, both earned rather than proposed: the **mutation re-run** (mechanical yield;
item 57 shipped 2026-07-26, and a `.git`-free copy of the tree now runs 49 core binaries green, so
the baseline that blocked it is measured clear) and **real-device mobile** (the only lens with a track record of HIGHs here),
each *after* the batch it depends on.

**A standing lens menu now lives in [backlog.md](backlog.md) under "Proposed audit lenses"** (added
2026-07-26, so this table can stay a record): six never-run lenses **L1-L6**, four re-runs ranked by
age against churn measured in each round's *own* surface, and four directions that recent work has
unblocked (a real device, HEALTH-1's panic boundary, `data-section-end`). Each entry carries the
measurement that justified it, so a session picking one does not re-derive it. **L1 (path parity) ran and shipped
2026-07-26** → [2026-07-26-path-parity-audit.md](2026-07-26-path-parity-audit.md).

What those two rounds left explicitly unmeasured, so it is not mistaken for a clean result: whether the
freeze cache's per-edit rewrite costs warm-loop latency (the probe's 200 ms poll floors the
measurement), multi-hour wall-clock drift as distinct from execution *volume*, R kernels, cold-build RSS
peak at 400+ pages, `notify` at extreme directory counts, and the docs lens's own remaining half —
behavioural claims that are prose rather than a default value or a key.

### The 2026-07-25 band-B batch (AP3-3 + PA-M3 + PA-M13 + PA-H1's residuals)

Band B emptied too, which leaves **no code in Open work at all**. Two of the three items closed on
evidence rather than on code, and the one lesson worth carrying forward is about *how a defect gets
recorded*:

- **A flake entry named the wrong test AND the wrong cause, and survived a "fix" because of it.**
  The record said `kernel_executes_..._runaway_cell` flaked ~1 run in 13 on a timing edge in the
  interrupt path. Looping the real condition (the full `--bin` binary, not the one test) reproduced
  it 3 times in 37 runs — the recorded *rate* was right — and captured three **different** tests
  failing: `cold_kernel_self_reaps_...` on `Address already in use`, the runaway-cell test on
  `ConnectionReset` **at `Kernel::start`, not at its interrupt assertion**, and the pooled-warm test
  on a missed 10 s poll bound. One cause: `prepare_connection` peeks ports by binding-then-releasing,
  and the re-roll that survives that race lived in the *callers*, so the three test-side callers of
  the raw primitive inherited it. Which test lost the race on a given run was chance, so the entry
  had faithfully recorded one sample as if it were the phenomenon. **The method that worked was to
  loop the real condition and capture the failure, not to reason from the symptom** — and the entry's
  own instruction ("the assertion text has never been captured") was the correct one all along.
  Also falsified in passing: the note that the pooled-warm test "asserts on no elapsed time at all"
  — a bounded poll *is* a wall-clock assertion.
- **A first-draft pin passed against a build with its fix deliberately removed.** The deck
  theme-color test asserted `contains("theme-color")`, which the *mutation marker comment* satisfied.
  This is the [inlined-asset needle trap] one level in: needle the mechanism
  (`setAttribute('name', 'theme-color')`), never the phrase. Mutation testing is what caught it, and
  only because the mutation was run before the fix was trusted.
- **Calibrating a lint before writing it changed its implementation, not just its threshold.** The
  proposed `image:`-without-`image-alt:` rule fires on 6 pages by line-scan but only 4 by parsed
  front matter: two `docs/` hits are `image:` inside a YAML *example* in prose. The measurement is
  what forced it to read parsed front matter, and the four genuine hits were fixed rather than
  excused.
- **Declining on measurement needs the measurement, not the intuition.** WS op-batching was declined
  with its worst case *confirmed* (55 ops, 53 `SetMeta`, one frame each) and the reason recorded
  next to it: warm edit is 32.2 ms against a 0.94 ms diff, so framing is 0.7% of a payload and never
  the critical path. T2 was declined after finding its "three modules" premise partly rotted.

### The 2026-07-25 band-A batch (AP7 + AP3-1 + AP11-1 + DIAG-1 + DOCS-1)

Everything the five rounds above filed as build-ready, landed in one branch. What is worth carrying
forward rather than re-deriving:

- **Two of the eight diagnostic fall-throughs were invisible to the audit's own method.** DIAG-1
  measured `check --format json` over 23 targets and found six. `check` never executes a cell, so the
  two execution diagnostics (`TAL-CELL-ERROR`, `TAL-KERNEL`) exist only on the `build`/`publish` path
  and no check-side sweep could have seen them. **A message-catalogue audit has to enumerate the
  emitters, not the emissions of one command.**
- **AP7-1's two causes had to move together, and fixing them exposed real content defects.** With the
  heading rule able to see the title block at last, two genuine authoring skips surfaced that the blind
  rule had been hiding: `docs/guide/reference/cli.tmd` opened at `###` with no `##` above it, and the
  `new paper` scaffold put `# References` after `## Methods`. 37 of 51 book pages → 0.
- **AP11-1's fix could not read the diagnostic's own prose.** The obvious implementation (extract the
  reason from the block HTML) fails on a `#| label:` cell, whose output is wrapped in a `<figure>` —
  `classify_exec_output` then reports a figure, not an error. The marker carries a *kind*, not text.
- **AP3-1's measurement needs the cell's own body edited.** The first version of the probe edited the
  slow page's prose, which leaves the cumulative hash intact, so the cell replayed from cache and the
  probe measured 0.09 s both times — a false all-clear on an unfixed build. Mutation-checked properly
  afterwards: forcing every build back onto the exec lane reproduces 10.30 s.
- **A `{js}`-heavy page is cell-free** for routing purposes, which is most of what makes the bypass
  lane worth having: the explorable-explanation pages are exactly the ones with no kernel cells.

**Skimmability audit (reader experience at book scale), 2026-07-24** →
[2026-07-24-skimmability-audit.md](2026-07-24-skimmability-audit.md). Author-prompted, not an AP slot: "how do I
make a tool that helps writers create documents that are easily skimmable, and what makes Taliesin better for
large books". Method: 8 web-research agents (reading science, structured-writing methodology, docs-tooling
competitive sweep, e-reader/annotation prior art, typography, search/findability, accessibility, build-time
derivation) + 5 codebase-inventory agents, feeding 8 ideation lenses; 110 raw candidates consolidated to 41,
then each attacked by 3 independent adversarial verifiers (already-shipped, invariant, efficacy) with permission
to kill it; 5 killed, 36 survived; two hostile critiques on the draft raised 37 issues. Headline: **the problem
is not missing features, it is that the heading layer, the only surface a skimmer actually reads, is defective
in six small verified ways** (spurious `4.0.1` section numbers on 31 of 32 numbered dogfood chapters from a
demotion/counter disagreement; whole-book Cmd-K search absent below `MIN_TOC_HEADINGS` while the button still
renders; a scrollspy that measures a class books never emit; `h5`/`h6` dimmer than body text; a printed TOC
showing 2 of 8 entries; 18% of guide sections truncated out of the search index by `BODY_CAP`). A seventh, worse
and unrelated to skimming: **a nested `{part:, chapters:}` group silently deletes itself and every chapter under
it, and `check` exits 0** (`site/book.rs:84-86` discards the inner loop's "not a chapter" signal). All seven
re-verified by the main session at `5c25d00` via a fresh build plus a targeted repro. Above the defects, one
structural gap dominates (**no whole-book outline below chapter granularity on any reader surface**, though 161
section records already sit in the built index) and one substrate gap blocks four proposals (**zero `<section>`
extents in the emitted HTML**, though `lsp_outline.rs` already computes per-section `end_line`). Second-order
finding, evidence-backed: the leverage is on the **builder** side, not a reader annotation layer (Ponce/Mayer
2022: author-provided highlighting helps comprehension, learner-generated does not). Third-order, and outside
code: **roughly half the problem is content** (0 of 37 dogfood pages set `description:`, 8 xref links across 19
chapters, 0 backlinks, 60,208 words of internals with zero `{.definition}` blocks), so glossary/term-index/float
digest all render empty until an authoring pass happens. Build-ready work folded into Open-work items **22
(band A), 23 (band B), 24 (band C)**. **Item 24 closed 2026-07-26** when the author ruled on both of its
questions: `section-extents` took the audit's own recommendation, **option (b)**, and shipped that day as a
`data-section-end` marker on every heading block (extents nest, are inclusive of the heading, and stop before
the generated References/footnotes furniture; decks are excluded because `deck.rs` already emits a real
`<section>` per slide). Option (a), the wrapper — and with it `content-visibility: auto` and sticky section
headings — stays deferred on the diff-shape risk the audit named. `book-breadcrumb` was ruled **no**, on the
audit's own dwell-time evidence: D114 stands. Explicitly ruled out and recorded so they are not rediscovered: section
hover previews (built and deleted at `318f22f`), a TOC entry budget, margin footnotes, `taliesin split`, a
reading-density fold, the `:~:text=` half of deep links, and anything LLM-generated (byte-identical build output
is test-pinned). Also caught: `FEATURE-IDEAS.md` #9 is **falsely marked SHIPPED** (read-aloud; `speechSynthesis`
greps to zero), a rot instance the audit tripped over mid-run.

**Perspective audit AP10 (internal codebase health), 2026-07-23** →
[2026-07-23-ap10-codebase-health-audit.md](2026-07-23-ap10-codebase-health-audit.md). The pure code-read,
fan-out-safe lens; run **alongside a live parallel session** (the `ask-ai-handoff` feature session) precisely
because it binds no port/kernel/browser and edits no source, written up in an isolated worktree off
`origin/main`. Asks the AP10 question the reduction + vacuous-test rounds did not: **which of the ~708 non-test
panic sites are reachable from user input, and which sit behind a recovery boundary?** Headline: the codebase
is healthy — **dead code is essentially nil** (2 `#[allow(dead_code)]` in the whole non-test tree, corroborating
the reduction audit) and the panic surface is dominated by guarded/structural sites. The one real finding,
**HEALTH-1 (medium):** the two *persistent stdio servers* — `lsp` (editor) and `mcp` (agent) — render/project
user documents in their request loop with **no per-request `catch_unwind`**, unlike the `serve`/`build` paths
AP2 verified and unlike the LSP's own `render_buffer` (which already wraps its render in `serve::guarded` with
the comment "so a malformed buffer yields `None` rather than crashing the request loop"). `lsp::main_loop`
(`lsp.rs:93`) dispatches with `?` (propagates `Result`, not panics) and `publish()`→`check::buffer_diagnostics`
renders the buffer unguarded **every keystroke**; `mcp::cmd_mcp` (`mcp.rs:105`) calls `handle` unguarded. A
catchable panic there kills the server for the whole session (LSP: all editor intelligence dies silently; MCP:
every subsequent tool call fails). The guard is applied to hover/completion but not the every-keystroke
diagnostics path — an inconsistency AP2's census (scoped to build/serve) structurally missed. Fix: wrap the
per-message dispatch in the existing `serve::guarded`, + a "malformed buffer keeps the server alive" pin.
**AP10 also raises AP2-1/AP2-2's priority:** the deep-`>` stack-overflow abort and the O(n²) nested-bracket
hang (neither `catch_unwind`-fixable) degrade to a recovered 500 on `serve` but **kill a persistent server** on
`lsp`/`mcp`. Build-ready HEALTH-1 folded into Open-work item 21. Refuted: LSP position math panics (`lsp_pos.rs`
is defensive + tested — the AP5 divergence is a wrong-offset correctness issue, not a panic); dead-code/module
sprawl (lean). Residuals: full per-site reachability census, coupling metric, the check/`to_lsp`/projection
layer's own fuzz coverage (an AP2-followup).

**Perspective audit AP1 (performance & scale), 2026-07-23** →
[2026-07-23-ap1-performance-scale-audit.md](2026-07-23-ap1-performance-scale-audit.md). Tier-1 "genuinely
untouched, highest expected yield"; first perf note in `notes/`. Run solo on a clean tree with a fresh
`--release` binary + a throwaway harness that path-depends on `taliesin-core` (`scratchpad/perfbench/`),
touching no source. Headline: **performance is healthy and free of quadratic blowups on every path measured**
— single-doc render is sublinear per block (8000 blocks / 20k lines in 647 ms; ms/block *falls* 0.111→0.081
across 1k→8k), site cold build is linear + parallel (400 pages in 874 ms), and the block diff is O(n log n)
by construction (source-verified LCS→LIS, textbook O(m·n) DP explicitly avoided). The one real degradation is
in the warm-preview **moat**: every `.tmd` save in a site/book preview runs **two independent full-site
sequential render passes** — `refresh_xrefs()` (per save) then `validate_cross_page_links()` (per open tab,
inside `build_page`) — each rendering every page from disk, each annotated in source as a fixed "~27 ms" that
is actually a size-dependent tax. It is linear in (pages × blocks-per-page), so it is invisible until a
project is large: the real `corpus/tech-blog` (17 content-rich pages) already pays **~60 ms/keystroke** of
pure whole-site re-render (on top of the edited page), extrapolating to ~360 ms at 100 pages and ~700 ms at
200. Not a bug (DX1 rightly judged the debounced re-derive fine at corpus size); the finding is that the cost
doubles unnecessarily (two passes could share one render) and is the first thing to slide as a book grows.
Build-ready **PERF-1** (a: share one whole-site render across the two passes → ~halve it; b: scope
`validate_cross_page_links`'s all-pages-then-filter discard; c: debounce only if a real >100-page book
appears) folded into Open-work item 20. Warm-preview RSS bounded in a 30-edit probe (+2.7 MB, no leak).
Refuted false leads: `site.clone()` per page O(pages²) (it is `Arc`), hover/xref index quadratic (measured
linear), single-doc render quadratic (sublinear). Residuals not chased: kernel RSS drift, multi-hour warm
RSS, `notify` at extreme dir counts.

**Perspective audit AP5 (i18n / Unicode / multibyte offsets), 2026-07-22** →
[2026-07-22-i18n-unicode-sourcepos-audit.md](2026-07-22-i18n-unicode-sourcepos-audit.md). First of the
"Audit perspectives" series in [backlog.md](backlog.md) (each a single unexplored lens, one per session, run
around live parallel sessions so it built nothing and touched no source). Headline: the tool has three
disagreeing column conventions at the editor boundary and none is the UTF-16 that both `vscode://file:line:col`
and the LSP protocol expect (comrak `data-sourcepos` is byte-based, proven by a render probe; the stdio LSP +
diagnostics `to_lsp` are Unicode-scalar-based and never negotiate `positionEncoding`; the TS companion is
UTF-16). Scalar equals UTF-16 across the whole BMP, so all realistic natural-language text is correct; the
defect surface is astral characters (emoji, math letters) on the same line before a token, with LSP rename (a
write path) the sharpest edge. Just as valuable: the core scanners, text truncation (`char_indices`), and block
identity were verified multibyte-safe already, so a later robustness/fuzzing pass can skip them. Build-ready
pieces (I18N-1..5) folded into Open-work item 12.

**Perspective audit AP12 (offline-guarantee verification), 2026-07-22** →
[2026-07-22-offline-guarantee-audit.md](2026-07-22-offline-guarantee-audit.md). Headline: the tool's OWN assets
are genuinely offline (local woff2 fonts, server-rendered KaTeX, vendored d3/Plot, mermaid inlined into static
builds, a reveal/jsdelivr guard), but a `build ... --out` "portable" folder silently keeps any external
reference the author wrote (a remote image, a remote `{js}` `import()`) with no diagnostic. Proven by a
frozen-binary build probe: a doc with an `esm.sh` import and an `example.com` image builds to "0 assets"
(exit 0, no warning) yet still fetches both hosts at view time. Preview additionally lazy-loads mermaid from a
CDN despite the vendored copy. The fix that fits is a diagnostic, not a downloader (auto-fetching is correctly
avoided and tested today). Build-ready OFF-1/OFF-2 folded into Open-work item 13.

**Perspective audit AP9 (semantic-HTML / document-model validity), 2026-07-22** →
[2026-07-22-semantic-html-audit.md](2026-07-22-semantic-html-audit.md). A strong positive bill of health:
across 84 corpus renders plus a full website build, the emitted HTML is structurally valid (a raw stack
tokenizer and `html5lib` found zero invalid nesting, zero per-page duplicate ids, well-formed figures with one
`<figcaption>` each, labelled deck slide `<section>`s, valid list/table/definition-list structure); the only
nesting hit was the intentional `corpus/diagnostics/a11y.tmd` fixture. One finding, HTML-1 (medium): titled
docs emit multiple `<h1>` (the title-block h1 plus every author `#`, which the corpus uses for sections), so a
built site index carries 12 `<h1>` in one `<main>` and the single-root outline the tool's own PA-H2 logic
assumes is broken. Fix is the gated "heading-demotion" idea (2026-07-11 website-design audit), now evidenced;
anchor ids are level-independent so it is safe, but decks must be exempt (levels drive slide grouping). Folded
into Open-work item 14 (owner-gated).

**Perspective audit AP8 (determinism / reproducibility), 2026-07-22** →
[2026-07-22-determinism-audit.md](2026-07-22-determinism-audit.md). Covered both the read hunt and the
stateful rebuild-twice check (frozen binary). Result: a positive bill of health. Single-doc renders and a full
`corpus/bayesian-website` build are byte-identical across SEPARATE processes (fresh HashMap seeds each), and
the property holds by construction: page discovery, listings, and the hover index are explicitly sorted (the
hover index even carries a "deterministic across builds" comment), parallel page builds reassemble by index not
completion order, `xref` iteration is a keyed insert, and no time/random/pid reaches output (only temp-dir
scratch names). Reproducible cross-machine too (ordering by path/anchor/date). One low finding, DET-1: the
property has no explicit end-to-end regression guard (only single-run body-HTML snapshots + the hash unit
test), so a future unsorted HashMap-to-output could regress it silently. Folded into Open-work item 15.
NOTE: the audit session independently ran AP8 in parallel (`58db11d`,
[2026-07-22-ap8-determinism-audit.md](2026-07-22-ap8-determinism-audit.md)), a concurrent-choice collision.
Their round is the fuller record (121 docs x3 processes + 9 site builds, plus the KERNEL path this static pass
skipped) and found the one real defect, **AP8-1**: executed-cell stderr embeds the non-deterministic
`/tmp/ipykernel_<PID>/…py` path (`kernel.rs:994`), a reproducibility break and a local-path leak that the AP12
round missed. Item 15 merges both rounds (AP8-1 + this pass's complementary DET-1 regression guard).

**Subsystem audits (own detail files):** the **slide-deck** feature was deep-audited 2026-07-12 →
[2026-07-12-deck-audit.md](2026-07-12-deck-audit.md) (43 bugs + a keep/cut/fix/add feature verdict +
a mobile-feed spec + a grind order). Also queued as **section F** in [backlog.md](backlog.md). Note:
the deck mode-model is being reshaped (delete reader + PDF; add a mobile slide-feed) — read the file
before touching deck code so you remove rather than "fix" the outgoing behavior.

The **developer experience** was deep-audited 2026-07-18 →
[2026-07-18-dx-audit.md](2026-07-18-dx-audit.md) (DX/productivity research + discoverability-pattern
catalog + full DX-surface map + error/feedback-loop audit + 4 persona workflow simulations). Headline:
the tool's DX is well above median; **one finding dominates** — the excellent located "did-you-mean"
validators (broken links/images/media, dup ids, dangling xrefs) run in `build`/`check` but **not in
live preview**, so the fast loop is silent about the errors the author most needs while writing (every
persona shipped a broken doc because of it). Most recommendations are *surfacing an existing capability*,
not net-new. Prioritized feature queue in the file.

**The feature *polish* was audited 2026-07-19** → [2026-07-19-polish-audit.md](2026-07-19-polish-audit.md)
(4 read-only auditors — CLI/DX · authoring · live view · theming — + online research on how mature tools do these
+ per-finding source verification). Lens, distinct from the DX/PMF/deck rounds: "can an existing feature be
*simplified with the same power*, or is it *implemented unintuitively*?" Headline: the tool validated + located
almost everything, yet a handful of features offered co-equal spellings with no canonical, or silently dropped
input its own vocabulary invited — closing those "silent holes in a fully-diagnosed surface" was the highest lever
on "feels well-thought-out".

**Polish audit batch (PL1–PL20) LANDED 2026-07-19** — all twenty items shipped, one commit each on `origin/main`,
in two batches. **Batch 1** (`86d404e`…`06fa302`): PL1 surface the `check` code/severity/`--explain` in human
output · PL2 warn on empty feature divs + drop the colliding `.tali-input` CSS · PL3 warn on `.column width=`, add
`.columns ncol=`, fix empty-div grouping (the `group_divs` fix) · PL7 warn on a `|` in a `.step lines=` spec · PL8
dynamic `theme-color` + `generator` meta · PL9 anchor `.fade-out`/`.highlight` for the div did-you-mean · PL13
deck 3-state Auto/Light/Dark theme segment · PL14 probe the environment only when degraded · PL15 derive CLI usage
synopses + document `--draft`/`--tour` · PL16 group the 16-command help by purpose. **Batch 2** (`8eeb91a`…`76185be`):
PL4 single-source the owned `--tali-*` palette across page + deck · PL11 tokenize the geometry + motion scale · PL12
derive exec/error boxes from the callout tokens · PL10 stop leaking a raw JS stack trace to readers in built output ·
PL17 adopt a leading heading as the theorem title (like callouts) · PL5 accept `--json` ⇄ `--format json` everywhere
· PL18 one shared bad-format error + one dir flag per semantic · PL19 name `.column-margin` the canonical
margin-note · PL20 deck/reader micro-polish bundle (cold-open hint, reduced-motion on slide-jumps, key-sheet
Home/End/`0`, `og:type` gating). **Still open in §7** ([backlog.md](backlog.md)): only the four owner-ruling design
questions (deck serif/sans, focus-mode↔fullscreen, `//| uses:` alias, theorem-kind namespacing). Full per-item
findings + evidence + credit in the dated file above.

**A second, wider polish round was audited 2026-07-22** → [2026-07-22-polish-audit.md](2026-07-22-polish-audit.md)
— an **empirical browser sweep** (chrome-devtools MCP over the tech-blog site, the demo-book, the deck in feed
**and** stepped modes, and the feature docs, across light/dark/sepia + laptop/narrow) that the 2026-07-19 round
couldn't run (its chrome profile was blocked), **plus 4 read-only code auditors** (CSS/theming · client JS · CLI +
diagnostics · emitted-HTML a11y), each briefed with the shipped PL1–PL20 + open backlog so they hunt *new* ground.
~55 net-new findings (`PA-*`). Dominant pattern: **the design system stopped at `base.css`** — `site.css` (0 token
uses) and `deck.css` never took the PL4/PL11 colour+geometry+motion tokens, so the chrome around the tokenized
content is a second, drifting design language. Other clusters: page-scaffold completeness is page-only (deck +
listing pages skip favicon / `<h1>` / list semantics), one-of-a-kind a11y holes (a single missing
`aria-live`/focus-trap per surface), reduced-motion honoured in the reader enhancers but not the preview client, and
residual CLI `--help` drift. Grind order = 5 passes (design-system token PR · scaffold · a11y holes ·
CLI/diagnostics · reduced-motion+print) in the file. **PA-H1 LANDED 2026-07-22 (`dc58aa9`)** as the first item: the
standalone deck build shipped no `<link rel="icon">` (a `/favicon.ico` 404 + a blank tab), so it now falls back to
the same bundled mark a page does (pinned by `deck_offline_build::built_deck_carries_a_favicon`). **Pass (a),
design-system single-source, LANDED 2026-07-22** (branch `polish/design-system-single-source`): `site.css` +
`deck.css` now route their radii/durations/hover-shadows through the PL11 tokens, and the cite-this "Copied!" + deck
speaker/share active buttons take `--tali-accent-fill` (dark **5.59:1**, was ≈2.3:1 — the round's one true WCAG-AA
failure), with `:focus-visible` rings on every deck control, keyboard-focus parity on listing cards, and a sepia
search-`<mark>` (PA-C1/C2/C3/D1/F1/F3/S1/S2/S4/C4; four `render::tests` pins). The rest are queued
as item 11 in [backlog.md](backlog.md).

**DX1 LANDED 2026-07-18** (the dominant finding). Live static validation now runs on both serve paths:
a new `crates/server/src/preview_diag.rs` bridge converts the `check`-superset validators
(`check::page_static_diagnostics`, `Site::validate_cross_page_links`, `Site::warnings`) into
`protocol::Diagnostic`s; `serve::rebuild` runs the static set (Standalone) on pre-exec blocks, and
`serve_site::build_page` reaches parity (static InSite + cross-page filtered to the current page +
located `_site.yml` warnings, previously console-only). Spec/plan:
`docs/superpowers/specs|plans/2026-07-18-dx1-live-preview-validation*`. **The scope collapsed on
grounding — the exact backlog rot the audit itself warns about:** the "add a red-dot audit badge" work
**already existed** (`client.js` shows an amber count + red-on-error dot on the collapsed `◇</>` button),
and single-doc `serve` **already** surfaced xrefs + render warnings; the real gap was `serve_site`
parity, not "wire both paths + build a badge." The audit's "make cross-page checking incremental" was
also unnecessary (~27 ms whole-site re-derive; a debounced full run is fine). Browser-verified on both
paths (single-doc badge=3; site index badge=2; a clean sibling page shows only the site-global config
warning, no phantom cross-page). Method note: the helpers are unit-tested in-crate (the bin crate has no
lib target, so `tests/*.rs` can't see `pub(crate)` items); the live-socket wiring is verified via
chrome-devtools, not a unit test. Cheap follow-ups deferred: DX5 (unknown `:::`-class "did you mean")
and line-locating `_site.yml` warnings (`check` doesn't locate them either).

**DX2 LANDED 2026-07-18** (Tier 1 discoverability — highest discoverability-per-line). A one-time,
dismissible, localStorage-gated (`tali-hint-seen`) callout tethered above the collapsed `◇</>` dev
button surfaces the flagship Alt-click-to-source gesture + (where live) the `?` shortcuts menu — the
gesture previously self-advertised only *inside* the collapsed panel, so the blogger + speaker personas
"would have shipped never knowing it existed." All in `web-client/client.js` (built in `buildDevMenu`,
mounted into the existing `#tali-controls` host) + a CSS block appended to the shared server-side
`STATUS_CSS` const (`serve/mod.rs`), which both serve paths already inject. Preview-only by construction
(client.js is never in `build` — verified by grepping built output for `tali-hint-nudge` → 0). Four
dismissals, all persisting: Got it / opening the `◇` menu / the first *resolving* Alt-click / Esc.
**Per-line liveness** (`askLive`) omits the `?` line where it is a dead key — on a deck (reader menu is
`.tali-deck`-skipped) and when a reader has turned shortcuts off — mirroring `07-keyboard.js`'s existing
"don't advertise dead keys" discipline. Storage failures **fail closed** (treat as seen → never show):
an un-dismissable nag is worse than a missed hint, the opposite of `taliShortcutsOn`'s fail-open. Spec/
plan: `docs/superpowers/specs|plans/2026-07-18-dx2-first-run-preview-hint*`. **Grounding notes:** the
audit called this pure `[surface]`, but the "Alt-click a block" text existed *only* inside the collapsed
panel — the surfaced first-run nudge is genuinely small net-new chrome, not pure wiring. Like the dev
menu itself (and like DX1), it ships **no corpus pin** — the corpus is rendered *output*, and the dev
client is never in output; verification is a `STATUS_CSS`-contains-`.tali-hint-nudge` mutation-checked
Rust pin + `tsc` + a chrome-devtools loop across single-doc/site/deck + the mobile/laptop/portrait
matrix. A layout gotcha surfaced in-browser: `#tali-controls` shrink-wraps to the ~60px toggle, so the
absolute callout needed a fixed `width` (14rem), not just `max-width`, or it collapsed to a sliver (the
sibling `.tali-dev-panel` avoids this with `min-width:13rem`).

**DX3 LANDED 2026-07-18** (Tier 1 discoverability — "the config-authoring equivalent of shell
completion"). `taliesin init` now produces a project whose `_site.yml` autocompletes + red-squiggles in
any editor with a YAML language server, zero manual step: it emits the two bundled schemas into a
`.taliesin/` dot-dir and prepends `# yaml-language-server: $schema=.taliesin/tali-site.schema.json` to
the scaffolded `_site.yml`. One-file change (`cli.rs`: `INIT_SITE_YML` gains the modeline; `scaffold_init`
gains the two schema entries + per-file parent-dir creation; the all-or-nothing overwrite guard + written
list now cover them). **DRY:** reuses `taliesin_core::schema::{SITE_SCHEMA, FRONTMATTER_SCHEMA}` (the same
constants `taliesin schema` emits), so init's schemas can't drift from the validator — a test pins that
the modeline path resolves to a real file whose body **==** `SITE_SCHEMA` (mutation-checked). **Grounding
notes:** all three site walkers already skip `.`/`_`-prefixed dirs (page discovery `discovery.rs:117`,
`mirror_assets`, referenced-source deploy), so `.taliesin/` is neither a phantom page nor shipped into
`_site/` — integration-verified (built output has no `.taliesin/`; the emitted files are byte-identical to
`taliesin schema`; the modeline is an inert YAML comment, so `check`/`build` report no config warning).
Only the **site** schema is modeline-wired (into `_site.yml`, a real YAML doc); the front-matter schema is
emitted for the companion but not wired into `.tmd` files (a `.tmd` isn't a YAML doc a language server
processes). `init` is the sole `_site.yml` producer, so `new`/paper/post are untouched (DX10 covers those).

**DX10 MOSTLY LANDED 2026-07-18** (Tier 2 — "scaffolds that teach"; 3 of 4 sub-parts). The audit's
headline was that the single most-delightful discovery — Quarto's `#| label:`/`#| fig-cap:` cell options
**work verbatim** — was invisible, because no scaffold showed a runnable figure. Shipped: (1) `paper` now
scaffolds a worked `{python}` matplotlib figure (`#| label: fig-demo` + `#| fig-cap:`), a `$$` display-math
block, a `## Methods {#sec-methods}` section, and `@fig-demo`/`@sec-methods` cross-refs; (2) `init`'s
`index.tmd` "Next steps" points at `taliesin new`; (3) `new post --draft` — a `NewOpts`-threaded flag that
splices `draft: true` into the front matter. All in `cli.rs` (the pure `new_files` + thin `write_new`/
`cmd_new`), plus the extended `new_cli.rs` assertions and a regenerated `corpus/scaffold/posts/my-paper/`
mirror. **Grounding / gotchas:** (a) measured that `taliesin check` reports kernel/ipykernel status only as
an *informational* "Environment" block, never a counted diagnostic — so a `{python}` figure cell keeps a
scaffold check-clean with no kernel (exit 0), and `#| label: fig-x` resolves `@fig-x` **statically** (the
core corpus net renders without executing cells, yet fig-labelled corpus docs pass). (b) The scaffold has
BOTH a check-clean integration pin (`new_cli.rs` runs the real binary + `check`) **and** a byte-exact unit
pin (`every_scaffold_matches_its_corpus_pin`, fixed date `2026-07-10`) against `corpus/scaffold/` — the
paper mirror had to be regenerated with that fixed date, not today's, or the byte-pin fails. (c) `--draft`
defaults off, so every existing scaffold + the mirror stay byte-identical. **Deferred: `new deck --tour`**
(→ DX10-followup in backlog): a teaching deck's columns must use native `layout-ncol` (reveal's `.columns`
silently degrades — the pending **DX5**), so a column demo would teach a shaky idiom until DX5 lands; the
`NewOpts`/`NEW_FLAGS` plumbing is already in place for it.

**DX5 LANDED 2026-07-18** (Tier 1 — the last two silent-degradation traps). **Part A — `.columns` alias:**
`::: {.columns}` with `.column` children now aliases to the native `layout-ncol` grid (a new arm in
`build_container`, `ncol = max(2, count of .column children)`, reusing the exact `tali-layout` grid HTML),
so reveal.js muscle-memory lays out **side-by-side** (responsive, like `layout-ncol`) instead of silently
stacking — the on-projector disaster from the speaker persona. Sanctioned alias → silent (no warning).
**Part B — near-miss "did you mean":** a new `validate_div_class` (beside `validate_callout_kind`) fires
from the generic-div fall-through: for a class that is a near-miss (`closest` ≤ 2) of
`DIV_FEATURE_CLASSES ∪ THEOREM_KINDS` it pushes a located (click-to-source) `unknown div class \`X\` (did
you mean \`Y\`?)`. **Open-vocabulary design (the crux):** unlike callout/front-matter (closed vocab), div
classes allow arbitrary custom classes, so an exactly-known class (`.aside`, `.fragment`) and a class > 2
edits from every feature name (a genuine custom class) both stay **silent**; only 1–2-edit near-misses
warn. This is the case the `validate.rs:41` comment explicitly flagged (a misspelled theorem kind has no
prefix to anchor a suggestion). **Accepted tradeoff:** a custom class within 2 edits of a feature name
(`.roof`↔`proof`, `.side`↔`aside`) draws a spurious *warning* (never an error; renders fine); the upside
(catching the on-projector class of bug) dominates, and `DIV_FEATURE_CLASSES` is one const to tune.
**Verification:** unit tests (`validate_div_class` near-miss-only; `columns_div_aliases_to_the_layout_grid`),
a `vocab.rs` drift test pinning `div_classes()` ⊆ `DIV_FEATURE_CLASSES`, a mutation-checked `typos.tmd`
diagnostics pin, and a browser check (columns → 2×360px grid at 1440px; `.fragmnet` → "did you mean
`fragment`?" located in the preview dev menu). **Unblocks the DX10-followup `new deck --tour`** (columns
now work via the alias). Spec/plan: `docs/superpowers/specs|plans/2026-07-18-dx5-div-class-did-you-mean*`.
Clippy gotcha: the `DIV_FEATURE_CLASSES` re-export is read only by the cfg(test) drift test, so it is
`#[cfg(test)]`-gated to avoid an unused-import error in the lib build.

**DX11 LANDED 2026-07-18** (Tier 1 — the second silent-failure trap, after DX5's silent-degradation pair).
**The trap:** `taliesin build methods.tmd methods.pdf` wrote HTML bytes into `methods.pdf`, logged `built
methods.pdf`, and **exited 0** (`file methods.pdf` → "HTML document") — the academic persona (🎓 Priya)
opens a "PDF" full of `<!DOCTYPE html>`, concludes the tool is broken, and abandons (the audit's single
worst moment). Root cause: the second positional `out_html` was handed verbatim to `std::fs::write`
(`build.rs:231-234`), no extension check. **The fix:** a `const NON_HTML_OUTPUT_EXTS` (`pdf, docx, doc,
odt, rtf, tex, latex, typ, epub, pptx, ppt, md, markdown`) + a pure `non_html_output_error(out_html)`
helper, wired into `parse_build_args` (beside the value-less-`--out` / unknown-flag / bad-`--jobs`
guards): a denylisted extension is a hard `Err` (exit 1, **nothing written**) with a friendly message
naming the extension, the concrete `.html` fix (`Path::with_extension`, so `dist/x.pdf` → `dist/x.html`),
the browser Print-to-PDF escape hatch, and the planned print track (ROADMAP Pillar IV / Wave 5).
**Denylist not allowlist (the design crux):** an extensionless / `.html` / `.htm` / `.txt` / unusual-named
target is the author's deliberate choice (HTML content in the file they asked for), *not* a
format-expectation trap, so it stays permitted; only format-implying extensions (the pandoc/Quarto
refugee's muscle memory) are rejected. **Accepted tradeoff:** a format-implying extension not in the
const slips through (writes HTML, exit 0, as before) — low-frequency, low-harm, one line to extend. It is
the CLI analog of `frontmatter::NON_HTML_FORMATS` (which does the same for a carried-over `format:` field
*name*); the two lists stay separate consts (format names vs. output-path file extensions) in their own
crates. **Verification:** two pure unit tests (`non_html_output_error` denylist + case-insensitivity +
`.html`/extensionless/`.txt` pass-through + dir-preserving suggestion; `parse_build_args` Err on `.pdf`,
Ok on `.html`), an end-to-end binary pin (`strict_robustness.rs::build_into_pdf_is_rejected`: exit≠0, no
`.pdf` created, stderr names the extension + HTML-only + ROADMAP + Print), full `cargo test -p
taliesin-core -p taliesin-server` green, fmt + clippy clean, and a real-binary check (`.pdf`/`.docx`/`.md`
rejected; `.html` still builds). No corpus pin: this is CLI arg-validation, not a rendering capability.
Spec/plan: `docs/superpowers/specs|plans/2026-07-18-dx11-friendly-pdf-rejection*`. **Unblocks nothing new;
next per the suggested order is the DX10-followup `new deck --tour`.**

**DX10-followup LANDED 2026-07-18** (the 4th DX10 sub-part, "scaffolds that teach"; unblocked by DX5).
**Before:** `taliesin new deck <slug>` scaffolded a bare 2-slide deck, so the single most-delightful deck
capabilities (fragments, incremental reveal, side-by-side columns, live magic-move, speaker notes) were
invisible until the author read the reference. **The fix:** `new deck --tour` scaffolds a *guided*,
check-clean deck: seven slides, one per feature, each demonstrating **and** explaining it in a line
(scaffold-as-teacher) — navigation basics (`##`=slide, arrows/`?`/`s`), a `. . .` pause + `::: {.fragment}`,
`::: {.incremental}`, `::: {.columns}`/`.column` (the DX5-unblocked idiom), `::: {.magic-move}` over two
code blocks, `::: {.notes}`, and a "make it yours" closer. **Plumbing:** `NewOpts.tour` + `--tour` in
`NEW_FLAGS` + a `--tour` arm in `cmd_new` + a `--tour` branch in the pure `new_files` `Deck` arm; the tour
body is a raw-string const `TOUR_SLIDES` (so its many `::: {.class}` braces need no format escaping),
appended to interpolated front matter. **Deck-only (the design crux):** `--tour` on any other kind is a
friendly hard error (`--tour scaffolds a guided deck; use it with \`new deck <slug>\``), not a silent no-op
— on-theme with the DX batch's anti-silent-degradation stance; `--tour` is the first kind-specific `new`
flag (`--draft` stays universal). The **default `new deck` output is byte-unchanged** (new branch is
`--tour`-gated; the existing `corpus/scaffold/my-talk.tmd` pin + "default unchanged" tests stay green).
**Dependency-free + check-clean by design:** no images/citations/xrefs/`{js}`/executed cells; every `:::`
class is a known deck feature (DX5 `DIV_FEATURE_CLASSES`), so no did-you-mean fires; plain ` ```python `
blocks are highlighted, not executed (no kernel needed). **Pinned:** a `corpus/scaffold/deck-tour.tmd`
fixture (generated from the binary → rendered + linted by the corpus net = the capability pin) + three
`new_cli.rs` tests (features present + check-clean; non-deck rejection writes nothing; byte-for-byte drift
guard between the CLI output and the fixture). **Browser-verified** (chrome-devtools, the DX5 payoff is the
headline): the columns slide lays out **side by side** (`grid-template-columns: 432px 432px`, two cols at
equal `y`, different `x` — not stacked), fragments (2: pause + aside) / incremental (3 items) / magic-move
(2 code blocks) all render, and `::: {.notes}` is `display:none` in the audience view; console clean. Full
`cargo test -p taliesin-core -p taliesin-server` green, fmt + clippy clean. Spec/plan:
`docs/superpowers/specs|plans/2026-07-18-dx10-followup-deck-tour*`. **With this, all 4 DX10 sub-parts are
done; next per the suggested order is DX4/DX6/DX8.**

**DX4 LANDED 2026-07-18** (Tier 2, "the heavier discoverability tier"). **The gap:** a first-run user's
first code cell fails on kernel wiring and they want a `flutter doctor`/`quarto check`. Taliesin *had* the
probe logic (`interpreter::probe`: `<bin> --version` + an `import ipykernel`/`library(IRkernel)`), but it
was **buried inside `check` and only ran for languages a document already used** (`check.rs`
`used_languages` → `env_entry`): circular, since you can't diagnose a Python setup before you have a
working-enough Python doc. **The fix:** `taliesin doctor [dir]` surfaces it as an **unconditional**
standalone audit of *both* Python and R: resolve the interpreter (honouring `_site.yml python:/r:`, a
`.venv`, `TALIESIN_PYTHON/R`, then the PATH default, via `interpreter::resolve_*`) + probe it, plus **active
conda/virtualenv detection** (NET-NEW: `VIRTUAL_ENV`/`CONDA_PREFIX`/`CONDA_DEFAULT_ENV` were read nowhere
before) and `_site.yml` sanity (reusing `site::is_malformed_config_warning`). Prints a per-item ✓/⚠/✗ line
with a concrete fix command and a readiness summary; `--format json` (`{ok, checks:[{name,status,detail,
fix?}]}`) for agents. **Severity model (the design crux):** ✓ = interpreter runs *and* its kernel package
imports; ⚠ = runs but the package is missing (→ a `<that-python> -m pip install ipykernel` fix) OR the
*default* interpreter is simply absent (you don't have it; not a misconfiguration); ✗ = a *configured*
interpreter (`Provenance::Field`/`Env`/`Venv` — a `TALIESIN_PYTHON`, an `_site.yml` field, a `.venv`) that
does not run at all. **Exit non-zero iff any ✗** (a pointed-at-and-broken interpreter is unambiguously
wrong and scriptable; CI kernel-readiness *warning*-gating is the separate DX18 `--require-kernel`). **New
module `crates/server/src/doctor.rs`** with a **pure, testable core** (`interpreter_check`/`active_env_check`
/`overall_ok` — probe + env injected, unit-tested without spawning, mirroring `interpreter.rs`'s discipline)
+ a thin `cmd_doctor` I/O wrapper; reuses `crate::interpreter` verbatim and never executes the user's
document. **Registration is guard-tested across FIVE coupled places** (found by the test net, not guessed):
dispatch arm ⟺ `COMMANDS` ⟺ `subcommand_help` ⟺ `usage()` ⟺ **`complete::command_desc`** (the shell-
completion brain — `every_command_has_a_description` caught the missing 5th). **Verification:** 6 unit tests
(each severity branch + the fix strings; conda/venv/none; `overall_ok`), 3 `doctor_cli.rs` integration pins
(sections present, JSON shape, broken-`TALIESIN_PYTHON` exits non-zero), full `cargo test -p taliesin-core
-p taliesin-server` green, fmt + clippy clean, and a real-binary sweep (⚠ ipykernel-missing / ✗ broken-
python-exit-1 / conda-env-named / valid+malformed `_site.yml` config). No corpus pin: a CLI diagnostic, not
a rendering capability. Spec/plan: `docs/superpowers/specs|plans/2026-07-18-dx4-doctor*`. **Next per the
suggested order: DX6 (`check --explain <code>` + `docs_url`), then DX8 (Cmd-K command palette).**

**DX6 LANDED 2026-07-18** (a flag on `check`, not a subcommand). **The gap:** `check --format json` already
stamps every diagnostic with a stable `TAL-*` code, but a code was only a *label* — nothing expanded
`TAL-XREF-UNREF` into "why did this fire, what's the one fix?" (rustc solved exactly this with
`rustc --explain E0502`). **The fix, two deliverables:** (1) `taliesin check --explain <CODE>` prints
title/cause/canonical-fix (offline, needs no file); honours `--format json` (`{code,title,cause,fix,
docs_url}`); **bare `--explain` lists every code** (an index — deviates from rustc deliberately, the code
set is small/closed/enumerable); an unknown code is a `closest()` did-you-mean + non-zero exit (a
`{"error":…}` envelope under json). (2) **a per-diagnostic `docs_url`** now rides on *every* `check
--format json` diagnostic (and the shared `build`/`publish` `diagnostics_json`), **computed** from the code
(`docs_url(code) = {base}#{code.to_lowercase()}`) so it can't drift; human output stays byte-identical (no
code/url leak — the codes-work invariant). **The resolved design forks (the crux):** *(a) flag not
subcommand* — codes are surfaced BY `check`, so `check --explain` is the discoverable follow-up, and a flag
dodges the 5-place subcommand guard (only the `flag_table_covers_help` drift gate applies; `--explain`
added to `flags_for("check")`). *(b) prose home* — an `EXPLANATIONS` table (`{code,title,cause,fix}`, one
per distinct code incl. `GENERIC`) sits next to the `TABLE` it explains in
`crates/core/src/diagnostics/codes.rs`, **drift-locked** by `every_code_has_a_nonempty_explanation` +
`no_orphan_explanations` (the DX5 vocab-guard pattern — a new family with no explanation fails the build).
*(c) docs_url must resolve* — the tool ships no production docs domain, so `docs/DIAGNOSTICS.md` is a
committed catalog **generated** from `EXPLANATIONS` (`diagnostics_markdown()`) and **blessed**
(`TALIESIN_BLESS=1 cargo test -p taliesin-core --lib codes`, mirroring `schema.rs::bless_or_assert`); GitHub
renders its `## TAL-FM-KEY` headings as `#tal-fm-key` anchors, so the computed url resolves for real. *(d)
completion* — `check --explain <TAB>` enumerates the drift-locked `all_codes()` set (static vocabulary,
distinct from DX7's dynamic-from-document completion). **Prose accuracy:** each cause/fix was grounded by
grepping the *real* validator message (e.g. `TAL-XREF-UNREF` = `include: false` drops a labeled cell's
output OR a theorem id missing its `thm-` prefix; `TAL-A11Y-ALT` = missing OR placeholder alt), not guessed.
**Surface:** `codes.rs` (+`Explanation`/`EXPLANATIONS`/`explain`/`all_codes`/`DIAGNOSTICS_DOC_URL`/`docs_url`
/`diagnostics_markdown`), `check.rs` (`docs_url` on `Diagnostic`, `explain_output`, the `--explain` parse
arm + branch before the path check, `CHECK_FLAGS`), `complete.rs` (flag + `--explain` value completion),
`main.rs` (`usage()` + `subcommand_help` blocks), NEW `docs/DIAGNOSTICS.md`. **Verification:** full-workspace
`cargo test` green (0 failures), fmt + clippy clean, and a real-binary sweep of all five shapes (human/json
explain, unknown→did-you-mean human+json envelope, bare index, `docs_url` on a live `check`, `--explain`
`<TAB>` completion). Spec/plan: `docs/superpowers/specs|plans/2026-07-18-dx6-check-explain*`. **Next per the
suggested order: DX8 (Cmd-K command palette — UI, needs a chrome-devtools check), then DX7 / DX17–19.**

**DX8 LANDED 2026-07-19** (the command palette; the UI-heaviest DX item). **The gap:** ✍️🎤 "reached for
Cmd-K to *do* things, got search only." **The fix:** the Cmd-K palette (`web-client/search.js`) now runs
commands too. **Shape (design fork, resolved):** a *unified* list, not a `>`-prefix mode (which teaches a
hidden syntax and loses immediacy) — an empty query lists the available actions first (a discoverable
menu), a query sorts matching actions (scored over title + keyword synonyms via the existing `score()`)
above content results. **Action set, each self-gating on a capability global (presence, not env sniffing):**
Toggle light/dark theme (always — `window.taliToggleTheme`, defined in `theme.rs`'s `theme_head` which ships
on every page); Restart kernel + Open source in editor (**live-preview only** — `window.taliRestartKernel` /
`window.taliOpenPageSource`, defined in `client.js`, which is `include_str!`'d ONLY in `serve/mod.rs`, so
they're absent from a static `build` and those actions simply don't appear there — a published site shows
exactly one action, theme). **DRY:** each action *invokes* the module that already owns the behavior, never
reimplements it — the theme toggle was extracted from the dev-menu button's inline handler into
`taliToggleTheme` and both now share it; kernel restart reuses the `{type:"restart_kernel"}` ws message;
open-source calls the existing `gotoSource(null,1)`. **Excluded on principle (documented in the spec):**
new-post/draft — a browser→server *write* path that fights the single-editing-surface / read-only-preview
invariant (the in-scope path is `taliesin new` / an editor command; also meaningless with no server); and
slide-jump — decks own their chrome and `search.js` no-ops on `.tali-deck`. **Reach:** the palette rides
wherever Cmd-K already lives (TOC pages: books, papers-with-toc, sites), which `toc_scripts()` gates on;
single docs without a TOC don't ship `search.js`, so DX8 is correctly scoped to "extend Cmd-K where it
exists," not "add Cmd-K everywhere." **Surface:** `search.js` (action registry + capability gates +
`availableActions()` + unified `render()` prepend + the `item.action` branch in `go()`/`itemEl()` + the
"Search or run a command…" placeholder + an `.tali-s-action` tag style), `theme.rs` (+`taliToggleTheme`,
button reuses it), `client.js` (+2 preview globals), `globals.d.ts` (3 decls). **Verification (the load-
bearing half is browser JS):** Rust drift pins on the three `include_str!`'d assets (search.js registry +
placeholder, `theme_head`'s `taliToggleTheme`, `client.js`'s two hooks) + full `cargo test`/fmt/clippy +
both JS `tsc` type-checks, then **chrome-devtools at 3 viewports** (1440×900 / 390×844 / 900×1440): empty
Cmd-K shows the 3 actions as a menu, "theme"→Enter flips dark→light + persists (`qmd-theme`) + closes the
palette, "kernel"/"editor"/"source" filter to their actions, "fourier" shows content only (no action
pollution), console clean; and a **static `build`** opened via `file://` shows only the theme action
(`taliRestartKernel`/`taliOpenPageSource` `undefined`). Spec/plan:
`docs/superpowers/specs|plans/2026-07-19-dx8-command-palette*`. **Next per the suggested order: DX17–19
(DX18 cheap — `check` exit-gating).**

**DX7 LANDED 2026-07-19** (dynamic value completion). **The rot (found by grepping the named surfaces
before trusting the entry — the file's own rule):** DX7's flagship — `@`-xref completion *with descriptions*
— was **already shipped** in the companion (`completions.ts` `xref` case merges buffer `{#id}` anchors + the
`taliesin symbols` registry as `Figure N`/`Section N`); and "page/deck names, post slugs" in the *shell* are
`.tmd` *paths*, which `complete.rs` already path-completes (there is no CLI slot that takes an xref or bare
slug). So DX7 was **not** built as worded; it was scoped to the two genuine, unshipped gaps. **Gap 1 (shell
install one-liner):** `taliesin completions --install` writes the script into the shell's conventional
completion dir instead of making the user hand-run a `> ~/.local/share/…` redirect. Shape: a pure
`install_plan(shell, &InstallEnv{home,xdg_data,xdg_config})` (unit-tested, no I/O) returns
`Write{path,manual}` for bash/zsh/fish (XDG-aware paths; zsh carries an `fpath` follow-up the write can't do)
or `Manual{command}` for powershell (`$PROFILE` can't be resolved from outside pwsh); the shell is detected
from `$SHELL` (basename → `canonical_shell`) or named explicitly (`completions zsh --install`). A thin
wrapper does the `create_dir_all` + `write`; exits non-zero only on unknown/undetectable shell, no `$HOME`,
or an I/O error. `flags_for("completions")` gains `--install` (so `completions --<TAB>` offers it and
`flag_table_covers_help` stays green once the help text mentions it), and the shell-kind positional now fires
even after `--install` interleaves. **Gap 2 (editor shortcode targets = "page/deck names, post slugs" where
they're actually values in a doc):** the companion completes the file argument of `{{< embed … >}}` /
`{{< include … >}}` — a new `detectContext` context (`\{\{<\s*(embed|include)\s+([^\s>]*)$`, first-arg only)
+ a pure `shortcodePathCandidates(entries, typed, fileDetail)` (`.tmd` files + descendable subdirs, ignore-
dirs hidden, dir-prefix preserved) rendered with a replace range over the typed path (folders re-trigger
suggest to keep descending; `/` added as a trigger char). **Deliberately not built (noted in the spec):** per-
candidate front-matter reads to label deck-vs-page (I/O in the completion hot path — uniform "deck / page"
detail is honest); internal `[](page.tmd)` link-target completion (a larger, separate context). **Verification:**
`cargo test -p taliesin-server` (257 unit + all integration + `flag_table_covers_help`) + clippy `-D warnings`
clean + `cargo test -p taliesin-core` (corpus net) green; **end-to-end** `completions --install` under a
throwaway `HOME`/`$SHELL` lands the byte-identical script for bash/zsh/fish, powershell prints the manual
command (exit 0), unknown/undetectable shell errors (exit 1); `__complete` brain e2e for the new cases; the
companion's `npm test` (75 node:test, 5 new) + `tsc --noEmit` clean + esbuild bundle; the guide's
`shell-completion.tmd` gains `--install` + the shortcode note and stays `check`-clean. **Not verified:** in-
editor click-through of the shortcode completion (needs a vsix repackage+reinstall; the standard companion
caveat — left to a deliberate step). Spec: `docs/superpowers/specs/2026-07-19-dx7-dynamic-completion.md`.
**Next per the suggested order: DX18 (cheap — `check` exit-gating), then DX12 (build exit-0 warning
summary).**

**DX18 LANDED 2026-07-19** (`check` exit-gating). **The gap:** 🤖 severity + kernel-readiness are already
computed, but the exit conflated them — *any* diagnostic (error or warning) → exit 1, and a missing kernel
*never* gated. **The fix:** two default-off flags on `check` (the default run stays byte-identical).
`--errors-only` runs the reported set through `at_severity_floor` (drops `severity != error`), so warnings
leave BOTH the output and the exit decision — a warning-only doc now passes, an error still fails. It filters
`--format json` too (an agent that wants all diagnostics simply omits the flag). `--require-kernel` runs the
already-collected `environment` through `kernel_gate_fails` (any used language whose interpreter is
absent/broken or whose `ipykernel`/`IRkernel` isn't importable) and, in human mode, prints a
`--require-kernel: no runnable kernel for <langs>` note so a 0-diagnostic run that still exits 1 is legible.
**Scope call (minimal-config):** `--min-severity` folded into `--errors-only` — there are exactly two
severities today (error/warning, uncatalogued → error), so a general `--min-severity` would have one
non-default value; a better default than a knob. Noted for a future third severity. **Surface:** `check.rs`
(flag parse + two pure helpers + the filtered display/exit + the human note; `CHECK_FLAGS` +2), `complete.rs`
(`flags_for("check")` +2, so `check --<TAB>` offers them), `main.rs` (help + usage). **Verification:** pure
unit tests for both helpers (severity filter incl. the empty-when-warning-only case; kernel gate off-by-
default / needs-a-used-language / interpreter-vs-pkg) + CLI exit-code integration tests in `check_cli.rs`
(errors-only filters warnings from JSON yet still fails on an error; a warning-only temp doc flips exit 1→0;
`--require-kernel` + a broken `TALIESIN_PYTHON` flips exit 0→1 deterministically) + the `flag_table_covers_
help` drift gate green with the two new flags + full `cargo test -p taliesin-server` (259 unit + integration)
+ clippy `-D warnings` clean. **Next: DX12 (build exit-0 warning summary — cheap), then DX17 / DX19.**

**DX12 LANDED 2026-07-19** (the non-strict silent-failure trap). **The gap:** ✍️🎓 a default `build`
(no `--strict`) still ships when it hits problems — a missing image, a dead link, a broken cross-ref —
and exits 0 with no closing signal. The per-problem `warn` lines already scrolled past above the `built`
line, so the last thing on screen is a green success and the exit code agrees; the degradation is
invisible unless you re-read the whole log. `--strict` already prints a failure tally, but nobody runs a
first build with it. **The fix:** a shared `warn_nonstrict_problems(problems)` prints one closing line —
`built with N problem(s) (run with --strict to fail the build)` — after the `built` line, on BOTH build
paths, a no-op when the build was clean. Single-doc: the two success exits now route through
`finalize_build(wrote, strict, problems)` (replacing `strict_exit`; `build_dir` returns `bool` so a
create/write error skips both summaries — it already failed and reported itself). Site: the existing
`strict_fail` branch gains an `else` arm. **Scope call:** the audit's second half (a `rebuilding…`
save-start line "for symmetry with `update N blocks`") was dropped — `build` is one-shot, not a watch
loop, and the elapsed-time suffix (`· 412ms`) already answers "was that slow"; a start line would fire on
every instant build as pure noise, against the perfect-default lens. The load-bearing win is the tally.
**Surface:** `build.rs` only (`finalize_build` + `warn_nonstrict_problems` + `build_dir: ExitCode→bool` +
the site path's `else`). **Verification:** two end-to-end tests in `strict_robustness.rs`
(`nonstrict_build_summarizes_problems`: a broken `@fig-nope` xref — a kernel-free located warning —
writes at exit 0 and prints `1 problem` + `--strict`; `nonstrict_site_build_summarizes_problems`: a
malformed `_site.yml` degrades the site but ships green with the same tally); full `strict_robustness`
(15) + `parallel_build_determinism` (5, the tally is emitted once after the deterministic replay so
`--jobs N` logs stay byte-identical) + `publish` + `build_jobs` + `embed_site_build` + `stale_sweep` +
`taliesin-core` green; manually confirmed the ordering (per-warning lines → `built` → tally, exit 0) and
that a clean build prints no extra line. **Next: DX17 (headless executed-output visibility — L, forked,
overlaps ROADMAP; brainstorm first) / DX19 (CSV→figure recipes in vocab/AGENTS.md — M).**

**DX19 LANDED 2026-07-19** (the data-figure recipe). **The gap:** 🤖 `vocab`/`schema`/`symbols` describe
*structure* (which front-matter keys, cell options, div classes, xref prefixes exist), but the one thing an
agent must currently learn from prose is a *composition*: turning a data file into a numbered,
cross-referenceable figure. `vocab` is closed-set structural only, so it can't hold it. **The fix:** the
generated `AGENTS.md` onramp gains a `## Recipes` section carrying the CSV→figure idiom (a `{python}` cell
that `pd.read_csv`s a file, plots it, and labels the output `#| label: fig-sales` so `@fig-sales` resolves;
plus the one-line `{r}`+readr swap). **"Generated from real corpus examples so it can't drift":** the recipe
ships pinned by a real corpus document `corpus/recipes/csv-figure.tmd` (+ `data.csv`) added in the same
change (the corpus-leads rule), and a `recipe_matches_the_corpus_example` test asserts the embedded cell is
**byte-identical** to that doc's cell — so if the corpus idiom changes, the test fails until the const is
updated and the asset re-blessed. The recipe cell is a Rust const in `agents.rs`, embedded in `agents_md()`
inside a `~~~~` fence (so the inner ```` ``` ```` cell renders literally), golden-locked exactly like the
vocab-sourced dialect section. **Scope call:** put it in `AGENTS.md`, NOT `vocab` (the finding named both) —
a worked composition isn't a closed set, and the audit itself flags `vocab` as "closed-set structural only";
adding prose to the structural JSON would blur its contract, against the perfect-default/minimal-surface
lens. **Surface:** `crates/core/src/agents.rs` (the `CSV_FIGURE_CELL` const + the `## Recipes` block +
`recipe_matches_the_corpus_example` + a `## Recipes`/`read_csv` assertion in `agents_md_teaches_the_protocol`),
re-blessed `crates/core/assets/agents/AGENTS.md`, synced repo-root `AGENTS.md`, and the new corpus doc +
data file. **Verification:** the recipe doc is `check`-clean with no kernel (the `@fig-sales` target is
registered from the label statically) and projects `@fig-sales` as "Figure 1" in `taliesin read` (the
kernel-free agent view); full `taliesin-core` (incl. all corpus invariants over the new doc) + the golden-,
repo-root-sync-, and drift-lock tests + `taliesin-server` (26 binaries) green; clippy clean. **Next: DX17
(headless executed-output visibility) is the last DX-batch item but is L, net-new, forked, and overlaps
ROADMAP agent work — brainstorm before building.**

-----------------------------------------------------------------------------

# Vacuous-test / mutation audit (2026-07-18)

**Why this lens.** Every prior round was source-driven (eye-driven browser passes, the
machine-facing surfaces, the reduction/modularity sweep whose headline was "the codebase is
already lean", the exec/kernel M-audit). Those saturated. The one lens never run as a
*deliberate* sweep is the codebase's own most-repeated, hardest-won finding — **"the tests
certify the defects"** (a green test that doesn't actually constrain the behavior it names).
As the source gets leaner, the surviving bugs are exactly the ones a vacuous test would let
through, so this lens gains value precisely where the others lose it. It also hardens the
regression net, which is the load-bearing asset.

**Method.** 4 read-only discovery agents (output-correctness / xref+citation / block-model+diff /
validation+freeze-keying), each proposing candidate vacuous tests with a concrete one-token
mutation + a SURVIVES/CAUGHT prediction. A `cargo-mutants` run (`taliesin-core`, `--lib`, the four
OG/SEO output files) as a mechanical backstop. **Every candidate was then verified by real
mutation** — apply the mutation, run the named test, watch it stay green — the "mutate the fix,
watch the named test fail" discipline this repo keeps re-deriving by hand. 13 agent candidates +
1 the agents missed but cargo-mutants caught (`sameAs`); all 14 confirmed, zero agent misfires.

**Landed the same day (test hardening; no production behavior change except one dead-code
removal).** Each new/strengthened assertion was mutation-checked (mutate → the NEW test fails →
revert → passes). Full workspace green, `cargo fmt` + `clippy -D warnings` clean.

| # | The hole (a green test that constrained nothing) | Fix |
|---|---|---|
| C4 | `is_safe_data_image` excludes `data:image/svg+xml` (SVG-XSS) with **no test** | added the svg+xml rejection case to `dangerous_url_schemes_are_neutralized` |
| C1 | the dedicated block-id test checks uniqueness+stability, never content-derivation (only 4 snapshot docs pinned it incidentally) | assert two different docs get different ids |
| C2 | tabset ARIA test asserts the attributes *appear*, not that tab↔panel pair | round-trip: a tab controls a panel that points back at it |
| C3 | diff had **no** 2-inserts-in-one-gap test, so `after_id` chaining was uncovered | `old=[a,d]→[a,b,c,d]` asserts the second insert chains off the first |
| A1 | the only real-math `llms.txt` test asserts length>2000 + a name, so `strip_katex` dropping inline math is invisible | assert inline KaTeX is stripped, not garbled into the text |
| A2 | OG-card pad-box test omits the `lead` field | added a long-`lead` case (mutation now escapes at x=1195 > 1128) |
| A3 | `og:type=website` (undated pages) had no test; only the dated `article` branch was pinned | assert the undated home page is `og:type=website` |
| A4 | feed `<title>` site-title fallback never exercised (every fixture sets a host title) | a title-less listing host → `<title>` = site title, not "Feed" |
| D1 | reading time only checked `contains(" min read")`, never the number | a 400-word doc must read "2 min read", not a constant |
| B1 | duplicate-xref-label test checks the warning fires, never the resolved number (the D53 flaw itself) | resolve `@sec-dup`, assert it keeps the first definition's number |
| B2 | duplicate-bib-key "uses the last definition" — never rendered to confirm which wins | format the dup entry, assert the last (Second/2002) wins |
| B3 | bracketed `[@fig-x]` cross-ref path had zero coverage | assert `[@fig-fit]` resolves to the figure link, not a bogus citation |
| D2 | `check` diagnostic assembly has no count assertion (any-exists only) | assert the broken-xref diagnostic appears exactly once |
| E1 | JSON-LD `sameAs` "tested" by a page-contains check the footer chrome also satisfies | assert `sameAs` is in the Person JSON-LD; **plus** a real latent: removed the dead `vocab` `about` description + added a bidirectional gate (D3) |

**Lessons that generalize (and match this repo's own log).** The sharp, targeted human-mutation
approach beat the mechanical sweep for *relevance* (agents ranked what matters), and the mechanical
sweep beat the humans for *completeness* (`cargo-mutants` found `sameAs`, which four agents missed) —
run both. Two apparent CAUGHTs were not agent errors: C1's behavior is only *incidentally* pinned by
snapshot docs (its named test is still vacuous), and D2 was a harness artifact (`taliesin-server` is a
bin crate, so `--lib` had no target). Process note for next time: when the test and the mutated code
live in the **same file**, `git checkout <file>` to revert a mutation also eats the new test — back the
file up and restore from the backup instead.

-----------------------------------------------------------------------------

# Taliesin: full multi-surface deep audit (2026-07-07)

**Method.** One multi-agent workflow (87 agents, ~6.9M tokens): 24 surface×lens finder
cells (parser/block-model, render, decks, exec+kernel, freeze/warm-pool, site/books,
web-client, diagnostics/check, CLI/build, docs, deps/licensing, accessibility, reader
craft, architecture/waste, feature-scouting), each finding adversarially verified
(refute-by-default), then per-surface dedup + novelty-tagging against the existing
backlog / AUDITS / FEATURE-IDEAS, a philosophy gate on every cut/feature candidate, a
deep-dive pass on the five hottest surfaces, and a final synthesis. 134 findings
survived verification: 0 critical, 1 high code bug, 7 high total. One cluster synthesis
(CLI/build/first-run) hit the structured-output retry cap; its verified findings are
recovered in the appendix below. Read-only audit, no code changed.

**The build-ready, batched implementation queue derived from this report lives in
[backlog.md](backlog.md) ("Audit 2026-07-07 implementation queue").** This section is the
detail reference behind it: verdict, top-leverage fixes, findings by theme, cut/keep/add,
and the low-severity long tail.

## Status — mostly landed (updated 2026-07-08)

**This is the original 2026-07-07 snapshot, not a live checklist.** The batched queue and
the top-leverage fixes have since shipped, so most findings below are already fixed —
**verify against current code before acting on any of them** (this bit an earlier grind:
the `image:`-URL bug, the theming/`#qmd-root`/schema doc-drift, and the deck `{#sec-x}`
anchor all read as open here but are fixed). Remaining open work is tracked in
[backlog.md](backlog.md) (Tier-2/Tier-3), not here.

Landed, by theme (batch → the "Findings by theme" section it clears):

- Batch 2 (`0b466c4`) → **Documentation drift (rename)**, the functionally-broken cluster.
- Batch 3 (`2369d80`) → **Accessibility** (Cmd-K contrast/ARIA, reduced-motion, slide bg,
  keyboard scroll, dialog).
- Batch 4 (`1132df3`) → **Cross-reference / section numbering** + consumed-anchor.
- Batch 5 (`561ff24` + `a6cf810`) → **Silent failure** (unclosed fence, figure `width=`,
  `draft: yes`, single-doc-build YAML, `_site.yml` typos).
- Batch 6 (`92dc677`) → **BibTeX layer** + the TOC/tabset double-escape.
- Batch 7 (`19022b7`) → `image:` URL, multi-page stale-file sweep, embed `--strict`,
  **deck title-slide hot-update**.
- Batch 8 (`41313f9`) → watcher prune, live site search index, reconnect state-preserve.
- Batch 9 (`1eb3238`) → **freeze cache + kernel** honesty + resource hygiene (partial;
  the remaining exec leaks stay in backlog Tier-2).
- Also: top-leverage #1 offline deck build (`478cdc1`), the `?qmd=embed` CUT (`679b76b`),
  the diff-then-broadcast consolidation (`e09744a`), the 2026-07-08 hardening fixes
  (byte-safe `percent_decode`; active-nav highlight on `#fragment`/`?query`), and
  top-leverage #7 (`e488abb`, the same-page link-preview source-attr strip — a shared
  `stripSourceAttrs` now neutralizes both the same-page and cross-page cards).
- Confirmed already-fixed by the audit's own "What held up" and now closed: the 390px hero
  overflow, the theme/video desync, and the heading `{#id}` dedup gap.

Notable **still-open** low-tail items the sweep did *not* touch (so they don't read as
done): `app.pages` unbounded ws-key growth, the deck `. . .`/`"Title Slide"` collisions,
several CLI/build appendix items, and the stale-but-working `qmd-*` alias docs. See backlog
Tier-2/Tier-3 for the tracked set. (`block_tag_has_id` substring match [`cbb4ee3`] and
`json_str` U+2028/2029 [`595c6fe`] have since landed.)

## Executive verdict

Taliesin came through a 134-finding, 24-surface deep audit with no critical defects and a single high-severity code bug. The load-bearing invariants the whole design rests on, unique block ids, total sourcepos, block-level incremental diffing, the freeze cache's no-stale-hit promise, and the read-only single-editing-surface, all held up under direct adversarial attack (see "What held up" below). The findings are not decay; they are under-enforcement and over-claiming.

Three themes dominate:

1. **Silent failure is the default.** The largest single cluster (roughly two dozen findings) is features that no-op or render wrong with no diagnostic, in direct tension with the project's own "surface bad news early" directive. Examples span every layer: an unterminated `:::` fence is silently dropped, quoted figure `width=` values are corrupted by smart-punctuation, `draft: yes` silently publishes a draft, the `ts`/`typescript` highlight alias is dead, and the single-doc `build` (the artifact-producing path) never surfaces malformed front-matter YAML that `check` and both preview servers catch.

2. **Accessibility advertised but shallowly delivered.** The a11y foundation is real (per-theme accessible color tokens, an SR-only convention, a `prefers-reduced-motion` CSS gate, emitted ARIA), but several flagship surfaces bypass it: the Cmd-K palette fails WCAG AA in every theme, deck auto-animate/magic-move route around the reduced-motion gate, and three surfaces emit ARIA (`aria-haspopup="dialog"`, `aria-haspopup="menu"`, a named combobox) whose promised behavior they never implement.

3. **Documentation drift from the rename.** The qmd->tali / qmd-fast->Taliesin rename never fully reached the dogfooded books; three onboarding recipes are functionally broken (custom-theme CSS vars, LSP schema filenames, the protocol element id have no back-compat alias) and the User Guide teaches the theme default as the exact opposite of the runtime.

Nearly every fix is a mechanical reconnection of an existing mechanism, not new infrastructure. Confirmation quality was high: of the findings reported here, all but a couple are marked CONFIRMED against source; the few PLAUSIBLE ones are flagged inline.

## Top highest-leverage fixes

1. **Offline-build breach for decks (HIGH, CONFIRMED).** `deck_page_from_doc` takes no `OutputMode` and hardcodes `code_scripts()` = Preview (`crates/core/src/render/deck.rs:94-108`), so a `build`-ed deck containing a Mermaid diagram ships a `cdn.jsdelivr.net` dependency instead of the inlined offline library the HTML page path correctly emits, and also ships every Preview-only enhancer as dead bloat. Thread `OutputMode` through and pin with a corpus test asserting no `cdn.jsdelivr.net` in a built deck.

2. **One located-diagnostic channel for the silent no-ops.** The math and front-matter paths already turn failures into click-to-source `Warning`s; extending that channel to unclosed fences (`divs.rs:143`), unresolved fence languages (`highlight.rs:51`), non-boolean boolean keys (`site/frontmatter.rs:56`), non-`.bib` bibliographies (`render/mod.rs:773`), and unknown `_site.yml` keys (`config/mod.rs:206`) retires most of the largest cluster in one coherent piece of work.

3. **AA token sweep.** Swap raw `var(--tali-accent)` for the existing `var(--tali-accent-fill)`/`--tali-on-accent` in the Cmd-K palette (`web-client/search.js`, the one HIGH a11y offender) and darken the sub-AA syntax comment token per theme (`base.css:338`, sepia `:583`, `deck.css:760`). The accessible tokens already exist; add a contrast assertion to stop regressions.

4. **Unify section numbering.** The flat `sec_count` (`render/mod.rs:390`) is the shared root cause of three-to-four `@sec-` cross-reference bugs. Registering the hierarchical `section_number` when a chapter is present collapses all of them.

5. **Freeze / warm-kernel honesty pass.** Scope the "stale hit impossible by construction / nothing to clear by hand" wording (`freeze.rs:11`) to code + interpreter version (a library upgrade is a real stale-hit path), and fix the mid-run kernel death that poisons the warm-prefix `ran` and wedges the preview (`exec.rs:610`).

6. **Docs rename drift.** Sweep `--qmd-*` -> `--tali-*` and `qmd-*.schema.json` -> `tali-*.schema.json` in the guide, fix the `#qmd-root` -> `#tali-root` protocol id, and correct the inverted theme-default statement (`theming.tmd`).

7. **Same-page link-preview card leaks the source-map surface.** Apply the cross-page attribute-strip to the same-page path too (`12-link-preview.js`), so a read-only preview card stops being a live Alt-click click-to-source target and stops seeding duplicate block ids.

## Findings by theme

### Silent failure (surface-a-diagnostic instead)

- **Authoring surface.** Unterminated `:::` fence silently dropped, content unwrapped, no warning (`render/divs.rs:143`, medium). `::: .callout-note` (leading dot, braces forgotten) renders as literal text; bare `::: classname .extra #id` silently drops all but the first token (`divs.rs:111`, low). Quoted figure `width=`/`height=`/`fig-align=` corrupted by smart-punctuation so the feature silently no-ops, already live and non-functional at `bayesian-website/subsections/_data-modeling.tmd:4` (`figure.rs:55`, medium).
- **Config/validation.** `draft: yes`/`on` silently publishes the draft (YAML-1.2 coerces to the string, then to false); same class mis-reads `toc: yes` and `execute: {echo: no}` (`site/frontmatter.rs:56`, medium). Single-doc `build` never runs `yaml_error()`, so malformed front-matter YAML builds clean and passes `--strict` (`frontmatter.rs:107`, medium). `_site.yml` nested nav/footer/mount typos degrade silently and even top-level site warnings ship unlocated (`config/mod.rs:206`, medium). Non-`.bib` bibliography path ignored with no diagnostic (`render/mod.rs:773`, low). Non-HTML `format:` values (pdf/typst/docx...) pass `check` clean and are silently rendered as HTML (`frontmatter.rs`, low).
- **Highlighting.** `ts`/`typescript` alias is dead (syntect ships no TypeScript), so TS blocks render unhighlighted with no signal; `toml` likewise absent (`highlight.rs:36`, low). Any unresolved fence language degrades to plain text silently (`highlight.rs:51`, low).
- **Runtime.** The file watcher registers a recursive inotify watch over the whole tree including `node_modules`/`.git`, so a large project exhausts `max_user_watches` and silently kills hot reload (`serve/mod.rs:877`, medium). The site Cmd-K index freezes after a content edit in preview while single-doc search stays live (`serve_site/mod.rs:1035`, medium). Mermaid render errors are swallowed by an empty `catch`, and a successful re-render after an offline failure leaves the stale banner (`mermaid.js`, low).

### Cross-reference / citation correctness

- **Section numbering (shared root cause).** Same-page `@sec-x` in a book shows a flat number contradicting the heading's hierarchical number (`render/mod.rs:390`, medium). Cross-page `@sec-x` on a non-book website is mislabeled "Chapter N" (`site/xref.rs:215`, medium). The hover-preview card for a book section heading drops its number (`site/mod.rs:759`, low). All three collapse behind the `section_number` helper.
- **BibTeX layer.** `@inproceedings`/`@conference` silently drop `booktitle` and `pages`, the single most common citation type in the CS/ML audience (`cite/format.rs:22`, medium). Parenthesis-delimited entries cascade-drop every following reference (`cite/parse.rs:32`, medium). Manager exports (JabRef) commonly emit both forms.
- **Citation rendering (deep dive).** Pandoc-style prefix text is silently deleted from a bracket group (`[see @doe2020]` -> `[1]`) (`cite/render.rs`, low). A bib key beginning with an xref prefix (`rem-`, `fig-`) is uncitable and emits a spurious broken-xref warning (low). A locator on a bracketed cross-reference is dropped (low). `transform_html`'s tag scanner treats the first `>` as the tag end, so a `>` inside an HTML comment leaks citation processing into non-text context (low).
- **DOM ids.** Duplicate `fig-`/`eq-`/`tbl-`/`lst-` labels emit invalid duplicate DOM ids (only warned, not deduped); the heading half of this was already fixed, this is the non-heading remainder (`render/mod.rs:455`, low). A heading consumed as a callout title discards its `#id` while its `@sec-` number was already registered, leaving a resolving ref pointing at a missing anchor (`divs.rs:395`, medium). A manual mid-document References heading is left detached from its list (`cite/render.rs:81`, low).

### Load-bearing invariants under-enforced

- **Block model.** The diff's LIS silently assumes globally-unique block ids with no assertion in the post-exec hot path (`diff.rs:185`, low, trivial `debug_assert!`). A block straddling an include boundary emits a mixed-file sourcepos (start file + end line from a different file), a hole totality-only checks miss (`render/mod.rs:286`, low).
- **Incremental payload.** Ops are broadcast one-message-per-op onto a 256-slot ring; a large structural edit self-overflows it and forces a full re-render, discarding the ~83x incremental win (`serve/mod.rs:1069`, medium, already-tracked as op-batching but with new overflow detail). The diff-then-broadcast core is copy-pasted across the two dev servers, giving the payload-shape contract two owners (`serve/mod.rs:992`, medium). A websocket reconnect wholesale-remounts and destroys all live block state (WebGL/{js}/video/open-details) even when the document is byte-identical, on any sleep or wifi blip (`web-client/client.js`, medium).
- **Read-only preview leaks.** Same-page link-preview card keeps cloned `data-block-id`/`data-sourcepos` (`12-link-preview.js`, low). `qmd-cursor` reverse-sync accepts postMessage from any origin/frame (verified read-only, so no write-back breach) (`client.js`, low). `click_block` prints client-controlled strings to the author's terminal unsanitized, a terminal-escape injection beyond the documented worst case (`serve/mod.rs:756`, low).

### Freeze cache + kernel zone (Do-NOT-touch, read-only audit)

- **Over-claims.** Freeze key captures interpreter version but no package fingerprint, so a same-interpreter library upgrade is a realistic stale-hit path the docs say is "impossible by construction" (`freeze.rs:11`, medium; doc-scope fix, no knob). Pooled forkserver kernels pre-populate `sys.modules`, so warm vs cold renders can differ despite the "identical" claim (`exec.rs:253`, low, doc fix).
- **Wedge / correctness.** Mid-run kernel death poisons the in-memory warm-prefix `ran`, so a later code-unchanged rebuild never respawns and serves KERNEL_DIED placeholders indefinitely (`exec.rs:610`, medium). `is_uncacheable` false-positives on legitimate outputs containing the sentinel strings, defeating caching for the self-referential docs (`exec.rs:894`, low, PLAUSIBLE). `interp_id` memoizes an empty version string on a transient `--version` failure (`exec.rs:903`, low).
- **Resource hygiene.** `adopt_forked` leaks both the `/tmp` dir and the forked kernel process on a handshake/bind timeout (`kernel.rs`, medium). Forked-kernel liveness/SIGINT/SIGKILL keys on a bare recyclable PID, defeating KernelDied fast-fail under PID reuse (`kernel.rs`, low). (The failed-`Kernel::start` `/tmp` leak, the boot-diagnostic clobber, the stream-ANSI leak, and the `in_flight` slot leak sharpen existing Tier-2 items with exact paths rather than adding new ones.)

### Accessibility (advertised but shallow)

- **Contrast.** Cmd-K palette selected row + match highlights use raw `--tali-accent`, failing AA in every theme (`search.js`, high). Syntax comment token below 4.5:1 in light/sepia/light-deck (`base.css:338`/`:583`, `deck.css:760`, medium/low). Deck `.tali-menu-slide-n` numerals sub-AA (low).
- **ARIA overpromise.** Book chapter drawer advertises `aria-haspopup="dialog"` and dims the page but has no `role=dialog`/`aria-modal`/focus-trap; same for the mobile TOC sheet (`site/chrome.rs`, medium). Deck control menu declares `aria-haspopup="menu"` over a plain button group with no roving focus (`deck.js:1466`, low). Cmd-K combobox splits `role`/`aria-expanded`/`aria-activedescendant` across wrapper and input, leaves the listbox unnamed, and never clears a stale activedescendant on empty results (`search.js:142`, medium).
- **Motion.** Deck auto-animate FLIP and magic-move morph set inline transitions that bypass the `prefers-reduced-motion` CSS gate (`deck.js:389`, medium). JS-initiated smooth scrolls hardcode `behavior:'smooth'` with no reduced-motion guard across search, reading-progress, and client navigations, and these ship in the static build (`search.js:553` and others, medium).
- **Keyboard / AT.** Overflowing `<pre>` and wide tables are horizontal scroll containers but not keyboard-scrollable, a hard WCAG 2.1.1 failure on the most common content type (`base.css`, medium). Lightbox decoration turns `pre.mermaid` into a `role=button` leaf, hiding the diagram's SVG content from AT, and forces decorative `alt=""` images into focusable tab stops (`11-lightbox.js:178`, medium). Non-hex per-slide background colors are assumed dark, flipping heading/body text to invisible white on light named backgrounds (`deck.js:337`, medium). Deck fragment reveals are not announced to screen readers (`deck.js:449`, low). Lightbox + link-preview lack the `.tali-deck` guard, so deck nav double-handles Arrow/Esc over an open zoomed figure (`11-lightbox.js`, low). Theme/Focus segmented controls use `aria-pressed` toggles for a single-select choice with no arrow-key nav (`14-reader-prefs.js:24`, low). The Resume-reading pill auto-dismisses after 8s with no announcement (WCAG 2.2.1) (`15-reading-progress.js:88`, low). Mobile TOC handle chip leaks the SR-only "(read)" suffix (`toc-sheet.js`, low).

### Reader experience, theming, visual craft

- Floated sidenote/margin-note has no `has-toc` guard, so above 73rem it collides with the sticky TOC (`base.css:554`, medium). `--tali-flash` unthemed for sepia, so live-edit pulses render blue on the warm page (`base.css:35`, low). Sepia comment token 3.47:1 (`base.css:583`, low). Two conflicting `.tali-input` rule blocks (`base.css:779`, low, see CUT). TOC entries and `.panel-tabset` labels double-escape `&`/`<`/`>` because `html_escape` is layered on already-safe `strip_tags` output (`render/mod.rs:1608`, `divs.rs:528`, medium).

### Deck engine (Rust + client, beyond the above)

- Deck title slide is injected as a raw string outside `doc.blocks`, so front-matter title/subtitle edits produce an empty diff and never hot-update the preview (`deck.rs:206-225`, medium). Explicit `{#sec-x}` on a slide heading is dropped, so `@sec-` renders a dead link (`deck.rs`, medium). A code block whose only content is `. . .` is swallowed as a pause marker (`deck.rs`, low). A slide titled "Title Slide" collides with the hardcoded `id="title-slide"` (`deck.rs`, low). Two `{{< input >}}` on one line collide on `qin-{line_no}` (`extension/mod.rs:231`, low). Theme-switched `{{< video dark= >}}` downloads both clips (`extension/mod.rs:311`, low, PLAUSIBLE). Overview touch swipe double-fires (pans + advances index, tile highlight desyncs) (`deck.js:1398`, low). Speaker Current/Next preview is blank for `<canvas>`-rendered cells because `cloneNode` skips the bitmap (`deck.js:983`, low). Internal `DocFormat::Reveal`/`is_reveal_doc` naming survives across ~73 sites after the engine was removed (`model.rs:119`, low).

### Site, protocol, watcher, security

- Absolute `image:` URL is mangled into a broken relative path, breaking og:image + listing social cards (`site/discovery.rs:26`, medium). Single-mapping nav/footer items and href-less bare strings are silently dropped (`config/mod.rs:222`, low). `block_tag_has_id` matches id as a substring, so a listing id can bind to `data-block-id` (`links.rs:119`, low). Active-nav highlight lost on a `#fragment`/`?query` href (`links.rs:51`, low). `percent_decode` slice-panics on a raw non-ASCII request path (`serve/mod.rs:420`, low). `app.pages` grows unbounded from bogus ws `?page=` keys, each preallocating a broadcast ring (`serve_site/mod.rs:630`, low). Front-matter theme flip between built-in light and dark is not hot-swapped (`serve/mod.rs:1043`, low).

### Cmd-K search (beyond staleness + combobox)

- `json_str` leaves U+2028/U+2029 unescaped; a paragraph/line separator in prose can break the whole inlined-JS index on pre-ES2019 engines (`search.rs:159`, low). Single-doc DOM index omits h5/h6 that the server index includes (`search.js:80`, low). The site index is re-mapped and re-lowercased on every open despite a "memoized once" comment (`search.js:70`, low). Fuzzy matcher scans every word of every section per keystroke for a 4+ char non-substring term (`search.js:232`, low, situational).

### Architecture / waste

- The two dev servers duplicate the load-bearing diff-then-broadcast contract, a real drift risk (`serve/mod.rs:992`, medium). Site discover renders every page 2-4x with no shared `RenderedDoc`, and `harvest_xref_numbers` lacks the empty-guard its sibling `build_hover_index` has, so a plain blog pays a full discarded render per page (`site/mod.rs:703`, low; trivial quick-win + medium consolidation). `render_internal_impl` (~500 lines) and `compute_outputs` (~260 lines, in the protected zone) are single dense functions producing the load-bearing block model / freeze reuse (`render/mod.rs:177`, `exec.rs:376`, low). Warning->Diagnostic conversion is copy-pasted four times (`serve/mod.rs:1006`, trivial). Cell-error scanning duplicated across build paths (`build.rs:239`, low). ~90 lines of dev-menu CSS embedded as a Rust string literal (`serve/mod.rs:450`, low). `render/mod.rs` also mixes ~180 lines of asset/script plumbing and attribute post-processing with the orchestrator (low).

### Documentation drift (rename)

- **Functionally broken (no runtime alias).** Theming chapter documents `--qmd-*` CSS vars the runtime never reads (`theming.tmd:153`, high). Schema on-ramp references `qmd-*.schema.json` but the tool emits `tali-*.schema.json` (`frontmatter.tmd:261`, high). Protocol book documents `#qmd-root`; the element id is `#tali-root` (`protocol.tmd:59`, medium).
- **Inverted / wrong guidance.** The guide teaches the theme default as "settles to dark, never follows the OS", the exact opposite of the resolver (auto = follow OS, light fallback), and contradicts the Internals book (`theming.tmd:27-35`, high). Front-matter refs say page-level `image-alt` is ignored and cards emit empty alt; code emits it (test-pinned), so the a11y guidance is inverted (`frontmatter.tmd:55`, medium). getting-started/CLI claim a shipped `~/.local/bin/Taliesin` launcher that does not exist, with wrong casing (`getting-started.tmd:16`, medium). Troubleshooting says the companion defaults to `taliesin`; the extension defaults to the dead `qmd-fast` binary (`troubleshooting.tmd:118`, medium). Internals execution chapter documents `qmd-*` output classes and a persistence guard on `class="qmd-error"` the runtime never emits (`execution.tmd`, medium).
- **Stale-but-working (aliases exist).** Guide's `viewof` example and ~10 corpus posts apply a dead `qmd-input` class (`code.tmd:169`, low). Guide teaches `qmd.*` cell API and `qmd-embed`/`qmd-video`/`qmd-fnref`/`qmd-main` classes as canonical (`code.tmd`, low). Internals teach `window.qmdEnhancers`/`QmdDeck` inconsistently (low). `IRkernel::installspec()` is a no-op Taliesin never uses (`getting-started.tmd:35`, low). README has no License section despite an MIT LICENSE (`README.md`, low). THIRD_PARTY.md claims cargo-deny CI wiring is deferred when it is done, references the deleted `code-enhance.js`, and omits `scrolly.js` (`THIRD_PARTY.md:56`, low). Preview Mermaid CDN pin and the vendored build copy can silently diverge with no provenance guard (`render/mod.rs:877`, low).

## CUT / KEEP / ADD

Driven by the philosophy-gate verdicts. Taliesin's north star (HTML-only, minimal-config, single-editing-surface, no per-edit startup cost) is the arbiter.

### CUT

- **`?qmd=embed` deck mode (adopt).** Verified unreachable dead code: speaker previews now use snapshot clones, and `{{< embed >}}` drives embedding through `window.taliDeckEmbedded`, not a URL mode. Drop the ternary branch and refresh the stale comments (`deck.js:1607`). Shrinks the deck's public state surface, no behavior change.

### KEEP (proposed cuts that should not be cut)

- **`data-level` deck attribute (defer, lean keep).** Not dead: it is a semantic heading-level marker and the count anchor for `corpus.rs:214`. Costs nothing at runtime. Keep and document it as a deliberate test/anchor hook rather than remove it and make the corpus count more fragile.
- **Duplicate `.tali-input` CSS blocks (defer, not a clean cut).** The two blocks style two different features sharing one wrapper class (`{{< input >}}` controls vs `//| viewof:` js-cell inputs). A naive merge would change the `{{< input >}}` layout. Decide the fix (unify deliberately, or split the wrapper classes) first; "cleanup with no user-facing change" is inaccurate.

### ADD (philosophy-gated new capability)

Only two clear adopts; the rest are aligned-but-deferred pending a design call.

- **Shareable/deep-linkable `{{< input >}}` state via the URL fragment (ADOPT).** The textbook realization of "wider = richer browser behavior in a live HTML view": reader-local URL/fragment state hydrated from the existing `data-qmd-input` registry, ~50-80 JS lines, no Rust/model change, no config knob, no write-back. Guardrail: must stay pure reader-local URL state (never a persisted server session) and must coexist with the existing deck (`#/h/v`) and block-anchor fragment routing.
- **Reader text-size and line-spacing controls (ADOPT, explicitly sanctioned).** CLAUDE.md names theme, text size, and spacing as first-class, a11y-exempt reader rights, yet only Theme ships. The whole substrate exists (`window.taliReaderMenu.addSection`, the pre-paint pref script, the segmented widget, `--tali-*` type vars). Add `--tali-reader-scale` and a line-spacing segment, persisted reader-local like theme (`14-reader-prefs.js`). This is the sanctioned exception to "better default over a knob".
- **Deferred but aligned (need a scoping/default ruling before building):** cross-revision block diff ("what changed in this document", the single most on-brand capability durable block identity enables); a reader-facing reproducibility manifest surfacing the freeze/warm-kernel provenance (the strategic wedge vs Jupyter/Quarto); a web-native List of Figures/Tables/Theorems for books; opt-in interactive data tables (sort/filter); a document-level "Cite this" export; code-line cross-references (`@lst-3:line`); theme-aware figures (a `dark=` image variant mirroring the shipped video `dark=`); copy-as-TeX on equations; cell-output export (save PNG / download CSV); estimated reading time; reader-local text highlighting. Each reuses shipped substrate, stays HTML-only and read-only, but collides with either minimal-config (opt-in vs better-default) or an open addressing/scope decision, so none is a clean drop-in.

## What held up under attack (negative space)

The audit is most reassuring in what it did **not** find:

- **The single-editing-surface invariant holds.** The full inbound message surface was traced: `click_block` logging, `restart_kernel`, `qmd-goto` navigation, and reverse cursor-sync are the only paths, and every one is read-only to source. No preview gesture writes back to the `.tmd`. The two "leaks" found (the same-page preview card's cloned ids, the un-origin-checked `qmd-cursor` listener) are source-map-hygiene and defense-in-depth, not write-back breaches.
- **Sourcepos emission is total.** The only hole is the rare include-boundary straddle; the corpus totality test's premise otherwise holds across every block type.
- **The heading `{#id}` dedup gap is already fixed** (`render/mod.rs:402-413` routes explicit ids through `dedup_with_suffix` + warns); only the non-heading anchors remain.
- **The offline invariant holds for HTML pages.** `build`/`render` inline every vendored asset and never touch the network; the deck path is the lone regression, and it is mechanical.
- **The exec/kernel Do-NOT-touch zone was respected and is fundamentally correct.** Every kernel-zone finding is resource hygiene or a doc over-claim; execution semantics and the freeze keying are sound. No stale-hit-by-construction bug was found except the honestly-scopeable dependency-upgrade axis.
- **The deck output contract is clean.** No reveal.js vocabulary leaks into the DOM (`.tali-deck`/`.tali-slide`/`window.TaliesinDeck`); the residue is purely internal type naming.
- **Two "open" visual bugs are already fixed** (390px hero overflow via `box-sizing:border-box`; theme/video desync via `data-theme` driving, zero `prefers-color-scheme` left in bundled assets), so the backlog/AUDITS entries for them can be closed.
- **No critical or high-severity security finding.** Every security item is LOW and consistent with the single-trusted-author, loopback threat model.

### Appendix, CLI / build / first-run (cluster synthesis errored; recovered from the verify journal)

These seven were confirmed by the adversarial verifier but missed the final synthesis
(their cluster agent hit the retry cap):

- **Multi-page `_site` build never sweeps stale files** (net-new, medium): `build_site_async`
  only writes/adds, so renamed or deleted pages persist in `_site/` across rebuilds
  (`crates/server/src/build.rs`). A stale-file sweep (or clean-then-write) would fix it.
- **Embed warnings don't count toward `problems`** (medium): the embed warning loop
  (`build.rs:330-336`) calls `log::warn` but never increments `problems`, unlike
  `doc.warnings`/xrefs, so `--strict` and the exit code under-count them.
- **Positional-target build has no parent-dir creation** (low): `build.rs:205` is a bare
  `fs::write` while the `--out` path calls `create_dir_all` (`build.rs:372`), so
  `build doc.tmd out/sub/x.html` fails when `out/sub` is absent.
- **`--jobs` ignored for single-file builds** (low): parsed into `BuildArgs.jobs` but only
  passed to `build_site` (`build.rs:154`); the single-file branch (156+) drops it.
- **No-kernel build warning doesn't name the language** (low): `build_one_page`
  (`build.rs:714`) reduces the diagnostic to a bool and `PageOutcome` carries no language.
- **Single-doc `serve` swallows a file-read error** (low): `render_doc` (`serve/mod.rs:287`)
  drops the read error via `.ok()?`; `serve()` handles `Ok(None)` silently, so an
  unreadable file serves nothing with no console message.
- **`log::info` reuses the green `built` tag** (low): `log.rs:148-150` routes through
  `Style::Built`, the same tag a real file write uses (`log.rs:30`), making console output
  ambiguous.

-----------------------------------------------------------------------------

**Older audit rounds (2026-06-19 through 2026-06-30) are archived in [AUDITS-archive.md](AUDITS-archive.md).**

-----------------------------------------------------------------------------

# Closed backlog sections + landed records (moved here 2026-07-16)

**Not a task list. Nothing here is open.** This is the rot evidence for work that is
finished, moved out of [backlog.md](backlog.md) so that file can obey its own rule ("only
open tasks live here"). It is kept **verbatim rather than deleted** on purpose: three
sections (B, D, G) were re-scoped by later sessions precisely because the "why this is
closed" reasoning lived only in git. If an entry here looks open, it is not — it is a
record of why it is dead. Open work is in [backlog.md](backlog.md).

The lettered sections A-G were dissolved on 2026-07-16 when only E and F still had
anything open; the letters no longer mean anything, and the surviving open items are a
flat priority list in backlog.md.

## A. Blog identity + de-Quarto — CLOSED 2026-07-16

*(Section A is empty: #7 draft-aware preview LANDED 2026-07-16 — preview shows drafts inline
(listing badge + page banner + dev-menu count/list), build/publish exclude them and report
"N drafts not published: …", book chapters are draftable. Spec:
[2026-07-16-draft-aware-preview-design.md](../docs/superpowers/specs/2026-07-16-draft-aware-preview-design.md).
Dropped 2026-07-12: #12 chronological post prev/next — for a 7-post topic-diverse blog the
ordering is meaningless and over-promises; the reading-first listing is the right hub, and
sequential nav already exists via books. A category-driven "related posts" strip could revisit
this, but only after a richer corpus makes "related" meaningful.)*

## B. Publish / build hardening — CLOSED 2026-07-16 (was rot)

*(Section B, publish/build hardening — `publish --public`, strict-by-default + `--no-strict`,
built-site shared asset bundle — was already SHIPPED by the author; the entries were backlog
rot, verified against source + removed 2026-07-16. See [[backlog-entries-rot]].)*

## C. Theme colour-system a11y follow-ups (2026-07-09 audit) — CLOSED 2026-07-16

*(Section C is closed, 2026-07-16. Six items built: the single-key-shortcut reader toggle
(WCAG 2.1.4, gating `f`/`?`/`/`, not just `f`, which the audit under-scoped), settings-popover
focus-on-open, category chips' `aria-pressed` + live count, keyboard-reachable link previews,
the forced-colors nav marker, and settings-panel reflow at 200%. Spec:
[2026-07-16-section-c-a11y-batch-design.md](../docs/superpowers/specs/2026-07-16-section-c-a11y-batch-design.md);
plan: [2026-07-16-section-c-a11y-batch.md](../docs/superpowers/plans/2026-07-16-section-c-a11y-batch.md).
Two items were NOT built: both had rotted, closed by §F's deck theming/a11y step, verified
against source before deletion. "Embedded deck ignores a sepia host" was already fixed at its
own named anchor (`render/deck.rs:164` reads `(t==='sepia' ? 'light' : null)`, the recommended
fix verbatim). "Deck slide-number chip not restyled per-slide" was fixed by removing the
premise: the chip is now one dark-glass surface in both themes (`deck.css:352-361`), so the
`html.tali-deck-dark`-scoped restyle the bug described no longer exists. See
[[backlog-entries-rot]].*

*Known + accepted (not a bug to re-file): at 200% the 4-button Theme seg wraps, and the first
button of the wrapped line keeps its 1px `border-left`, doubling against the container's own
border. Measured, judged cosmetic (same colour, contiguous, invisible unmeasured). The fix
(gap-dividers + `flex:1 0 auto`) was prototyped in-browser and REJECTED: it stretches the
wrapped button to full width, a bigger visual change than the hairline it removes.)*

Owner-calls kept as-is (one-line changes if ever wanted): table cells use the 1.28:1 hairline
(`base.css:436` — border-strong on every cell heavies every table); callout `tip`/`important`
collapse under protanopia (icon + title already carry meaning, hue never the sole cue); deck has no
sepia palette (document decks as light/dark-only, or add + teach the reader/scroll path).

## D. Reading-first identity polish — CLOSED 2026-07-16 (was rot)

*(Section D is closed. It was **backlog rot**, and the "direction ruling" it had been blocked on
turned out to be a question about a fork that does not exist. Direction **"Marginalia"** (iron-gall
manuscript ink) is fully landed: theme/colour 2026-07-09, type ~2026-07-12, layout already shipped.)*

*The **type** pointer was rot first (the old "type → item 13" named a `#13` that exists nowhere;
§A's numbering died when §A closed). The owned Newsreader body face is wired at `base.css:35`,
applied at `:216`, both variable faces bundled at `font-weight: 200 800` and inlined as `data:` URIs.*

*Then the three "re-verified, NOT rot" layout targets were re-checked on 2026-07-16, **in a browser
rather than by reading the file**, and two of the three dissolved:*

1. ***"Hero as typeset, not a marketing slab" was ALREADY SHIPPED.** The entry quoted `base.css`'s
   `.hero { text-align: center }` as proof, but that is the **full-width landing** branch; the
   override ~10 lines below (`.tali-site-main:not(.tali-wide) .hero`) is what a reading-measure page
   gets, and its own comment calls it "an editorial masthead ... e.g. a personal homepage". Measured
   live on the blog index: `text-align: left`, lead `max-width: none`, and the iron-gall eyebrow
   hairline rendering at 40x2px in `rgb(56,65,101)`. This is the "trust the symptom, never the line
   number" trap in its purest form: the quoted line was real, and irrelevant.*
2. ***"Drop bordered feature-card grids" is MARKETING-ONLY.** The blog never authors them. Exact-match
   grep: `.feature-grid`/`.feature` appear in `site/features.tmd` + `site/index.tmd` only, **zero**
   hits in `corpus/tech-blog/`. So reshaping them is deferred marketing work, not blog work.*
3. ***The `--space-1..6` scale is genuinely absent** (verified, grep exit 1), but with 1 and 2 gone it
   is a pure refactor: no visible change, regression risk across `base.css`. Owner ruled **drop it**;
   if spacing ever actually hurts, it returns as a real item.*

***The fork was false twice over*** *(and this is the reusable lesson): (a) `page-layout: full` →
`.tali-wide` **already partitions** the deferred marketing site from the blog, and `base.css` already
exploits that partition for `.hero`, so "cannot be scoped to the blog by CSS alone" was simply wrong;
(b) the blog does not author the contested component at all. **Do not re-open D**: the identity work
is done, and the only thing left in those primitives belongs to the marketing rebuild.*

## E. Catalog-derived work — the SWEEP is closed (2026-07-16); some items stayed open

**Owner ruling 2026-07-16: stop the sweep, triage an area on demand.** Wave 1 triaged the 4
highest-leverage areas (34/165: crossref, citations, slides, config) and measured the base:
**12 of 34 (35%) outright stale or superseded, 20 of 34 (59%) contain at least one false statement
about today's source.** Triaging the remaining 131 against that base is not worth a session, and the
staleness only grows as more ships. Full results, per-entry verdicts, and the caveats:
[2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md). *(The trust caveats and
the surviving open items moved to [backlog.md](backlog.md); this is the closure record.)*

### Landed 2026-07-16 (recorded so they are not re-scoped)

- ***Cmd-K index chapter scoping.** `build_sections` was the last site path rendering unscoped, so a
  book's search index said "Theorem 1" / "Figure 1" while the page said "2.1". D49's tail (scoped
  floats never reached `search.rs`), which the theorem flip widened to theorems. The chapter lookup
  existed twice (`Site::chapter_for` + an inline copy in `scan_xref_targets`) and search needed a
  third, so it is now one `book::chapter_of` all three call. The dev server's off-lock split is
  preserved: it reads the chapter under the same brief lock as the page clone, render stays off-lock.
  Two pre-existing defects it surfaced were filed, not fixed (raw `&nbsp;` in the index text; a
  cross-page `@fig-` in a snippet renders a bare "Figure").*
- ***Theorem/float numbering agreement** (was the `[ruling]` entry). Owner ruled **flip theorems to
  auto-scope**, then ruled **delete `number-within:` with it** — because once theorems scope
  automatically the key does exactly nothing, and a recognized-but-inert key is the very bug D67
  (`csl:`) just shipped a diagnostic for. Theorems now call `float_number`, the same helper floats
  use, so a chapter cannot show "Figure 2.3" beside "Theorem 5"; measured before (Figure 2.1 beside
  Theorem 1) and after (both 2.1) on a real build. **The entry's "breaks 2 pins" was itself stale**:
  the named lines pointed at unrelated tests, and the corpus pins that assert "Theorem 2.1" **passed
  unchanged**, because `methods.tmd` is chapter 2 and now scopes without config. The one real pin was
  `site/mod.rs`'s cross-page theorem test, whose sibling *figure* test already asserted "Figure 2.1"
  for the identical book — its own comment ("a flat Figure 1 would collide with chapter 1's own first
  figure") is the argument for the flip, written before the flip. Removal swept 12 sites incl. the
  drift-locked schema + vocab JSON (regenerated with `TALIESIN_BLESS=1`, diff = the key only) and
  `methods.tmd`'s whole front matter. Migration is loud, not silent: a leftover `number-within:` now
  warns `unknown theorems key` **with a line number**.*
- ***Cross-page `@fig-` to a CELL-labelled figure** (was live defect #2, the largest one). Shipped.
  The entry's cause was right but pointed at the wrong layer: teaching `scan_page_anchors` to parse
  fences would have duplicated the renderer's "which fences are cells" rule in a second parser.
  `Site::harvest_xref_numbers` **already renders every page and already iterates the renderer's own
  registry** (`doc.xref_numbers`), which contains cell labels — it was simply `get_mut` enrich-only,
  so it looked straight at `fig-x` and dropped it. Fix = insert-if-absent there, one source of truth,
  no new parser. This also fixed **backlinks** and **`taliesin map --format json`** for cell figures
  for free (both key off `xref_targets`). Scale was understated: the corpus has **26 cell-labelled
  `fig-` anchors vs 17 brace ids**, so the broken shape was the majority of the test net's figures.
  Pinned in `corpus/demo-book` (results.tmd defines `fig-stages` with a `{mermaid}` cell; summary.tmd
  refs it cross-chapter → "Figure 3.1"); verified in a real browser (click → `results.html#fig-stages`,
  target in viewport, no console errors) and on a non-book website (flat "Figure 1").
  **Two review catches worth remembering:** (a) the insert path had to re-apply `is_ref_anchor` —
  the render registry is LOOSER than the scan (the table-caption path registers *any* id), so
  `: cap {#my-table}` leaked into `map`'s xref_targets as a phantom resolvable target. Measured on
  both sides: `main` → `{}`, first-cut branch → `{"my-table": …}`. (b) A mixed-form duplicate took
  the *loser's* number ("Figure 2" on a link to a page where it reads "Figure 1"); the enrich arm now
  only accepts a number from the page the url points at. `docs/internals/sites.tmd` corrected: the
  xref design is **three** passes (scan → render-harvest → rewrite), not two, and its prefix list was
  missing 5 of the 12 real ones.*
- ***D49 chapter-scoped float numbering.** Shipped: figures/tables/equations/listings scope to the
  chapter in a numbered book ("Figure 2.1"), flat everywhere else. The number is built ONCE by the
  renderer that knows the chapter and carried as a `String` (`render::float_number`), mirroring the
  `section_number`/theorem precedent, so the executor prints it verbatim. **It never needed the
  citation zone** (`register_xref` already took a `String`, since theorems push "2.1" through it).
  Blocked instead on the **exec zone**, for 3 integer literals in exec.rs's own `#[cfg(test)]`
  module; owner approved that narrow edit and nothing else. Verified in a real build: intro
  "Figure 1.1", methods "Figure 2.1", cross-chapter ref → `intro.html#fig-structure` "Figure 1.1",
  standalone post still flat. `demo-book` had **zero** numbered floats, so intro + methods gained one
  labelled figure each (+2 small authored SVGs) to pin it.*
- ***D67 `csl:` recognized-but-unsupported.** Shipped, and it **never needed the citation zone**. It
  was **five** surfaces, not four: `AGENTS.md` also taught the key (both it and the vocab JSON are
  *derived* from `vocab::vocab()`, so one filter fixed both). Proved inert by rendering
  `corpus/bayesian-website` with and without the key: byte-identical (980300 bytes). The `css`
  did-you-mean hazard is now **mechanically pinned** (`csl_stays_recognized_because_dropping_it_would_mis_suggest_css`
  builds `KNOWN_KEYS` without `csl` and asserts the suggestion becomes `css`), so a future cleanup
  cannot re-introduce it. The rule finally lives in `frontmatter::validate_unsupported_keys`, on the
  **render path**, not in `diagnostics/` as this entry originally instructed: `diagnostics/` is
  check-only (it appears once in the whole server crate), so the first cut left the **preview**
  silent, which is the surface the author actually reads. Orphaned `ieee.csl` (17KB) deleted with it.*
- ***D74 footnote reverse-sync.** Shipped, and the symptom was **worse** than this entry said: the
  section hardcodes `data-block-id="qmd-footnotes"`, so `closest()` DID resolve, to a block with no
  sourcepos, leaving `openSource()` on its `line = "1"` default. Every footnote silently jumped to
  **line 1**; it was never a no-op. Fixed per-`<li>` (nested positions, the pattern `:::` divs already
  use); the block-level empty sourcepos is **deliberately kept** (a block-level range would break
  `corpus.rs:151`'s monotonic-source-order assert and make reverse-sync swallow the document). **No
  exemption existed to remove:** the checks skip on `sourcepos.is_empty()` *generically*, which is
  exactly how the hole hid.*
- ***D107 deck fragment effects.** Shipped as `::: {.fragment .fade-out}` / `{.fragment .highlight}`
  (a second class on the existing fenced div, so no new authoring form). **CSS-only** (`deck.css`), no
  Rust/JS: the effects reuse the `.tali-frag-visible` marker deck.js already toggles. Declines held
  (no `incremental:` knob, no `data-fragment-index`).*
- *Also landed 2026-07-16: the **deck key sheet** (it advertised "↑ ↓ Vertical slides" while
  `up()`/`down()` call `moveTopic`; the pin now reads the binding and the sheet together so they
  cannot drift apart again); **`author: [A, B]`** (a YAML sequence read via `.as_str()` gave `None`,
  so both consumers fell through `.or(config.title)` and a multi-author site published its own
  **title** as the author in the Atom feed and JSON-LD; `SiteConfig.authors` now reuses the same
  `frontmatter::string_list` a page's `author:` always used, and the deliberate RFC-4287 title
  fallback is pinned to fire only when there is genuinely no author); and the phantom
  **`number-sections`** doc comment (the key existed nowhere in the source but the comment claiming
  it; numbering is really decided by `chapter_for`). Note the 2026-06-29 theorem spec still reasons
  about "the `number-sections` feature" as though it shipped: it is a dated record, left as written.*

## F. Deck rework (2026-07-12 slides audit) — LANDED except B3-18

**Detail: [2026-07-12-deck-audit.md](2026-07-12-deck-audit.md)** (43 confirmed bugs + a
keep/cut/fix/add feature verdict + a mobile-feed spec + a grind order). Owner-decided shape change
(REMOVE, don't fix the old behavior): a deck opens **as a deck** (desktop = stepped slides;
phone/portrait = a TikTok-style scroll-snap **slide feed**, keyed on aspect not width); **delete
reader/scroll mode**; **delete print/PDF** (the critical dark-deck-blank-PDF bug is resolved by
removal); trim the overview flourishes (minimap/LOD/threads/filter/pen/van-Wijk zoom).

**Progress (2026-07-16): the ENTIRE audit is landed except one deliberately-deferred item.**
Steps 1-7 all done (front door + feed + correctness + flourish trim + theming/a11y/perf + docs
+ the C-ADD share-link/QR, live-input deep-link, feed notes-narration, wake-lock adds). See the
audit file's top-of-doc **Status** block for the per-item tracker. **B3-18 remains open and is
tracked in [backlog.md](backlog.md).**

## G. AI-native authoring (2026-07-12 audit) — CLOSED 2026-07-16 (was rot)

*(Section G is closed as a grind chunk, 2026-07-16. It was **backlog rot**: the whole browser-free
loop (the three items this entry called "the recommended first bets") shipped 2026-07-13, along
with 5 more. Verified against source + all 30 named pins run green before deleting the entries. See
[[backlog-entries-rot]].*

*Shipped (item → anchor → pin): #1 AGENTS.md onramp → `core/agents.rs:42` → `agents_md_cli.rs`;
#2 `taliesin read` → `render/text.rs` + `model.rs:283 body_text()` → `text_projection.rs` +
`read_cli.rs`; #3 agent-grade diagnostics → `server/check.rs:23` (`{code, severity, file, line,
message, suggestion?}`) + `core/diagnostics/codes.rs` → `check_cli.rs`; #4 Claude Code skill →
`editor/claude-code/skills/taliesin/SKILL.md` → `skill_freshness.rs`; #5 `map` → `map_cli.rs`;
#6 `taliesin-mcp` → `server/mcp.rs` → `mcp_stdio.rs`; #7 scaffolds + `paper` kind → `new_cli.rs`;
#10 structured build/publish errors → `structured_build_errors.rs`. Plus **#8(b)** placeholder-alt
(`a11y.rs:337`) and **#9(B)** ScholarlyArticle (`meta.rs:150`, author-free trigger) + **#9(C)**
per-page cited-refs sidecar (`build.rs:355,1560`) → `citations_sidecar.rs`.)*

*The three ruling-gated leftovers (#8a `check --online`, #8c numeric-claim hint, #9A per-page text
sidecar) were ruled **decline** by the owner on 2026-07-16; reasoning is recorded under "Decided
against" in [backlog.md](backlog.md). **Nothing in section G is open.***
