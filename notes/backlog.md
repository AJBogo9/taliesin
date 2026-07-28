# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git + [AUDITS.md](AUDITS.md) +
> [ROADMAP.md](ROADMAP.md); delete an item when it lands, don't leave a `[x]`. Method lessons that
> outlive their item go to [LESSONS.md](LESSONS.md). The "do not re-add" list at the bottom is a
> compact anti-rot guard, **one line per entry**, not a changelog.

## RESUME HERE (cold start, 2026-07-28)

**If you are a fresh session with no context, read this block, then "State", then band A. That is
enough to continue; nothing else is required.**

- **Git state as of 2026-07-28. Verify it, do not trust it** — this is the line that rots first,
  and both the author and parallel sessions push:

  ```sh
  git log --oneline origin/main..HEAD   # what is unpushed on this branch
  git branch -vv                        # what branches still exist
  ```

  At the time of writing everything is landed: `main` == `origin/main`, working tree clean, and
  **no unmerged branches** — the launch-blocker batch, the publication-readiness batch and the
  earlier audit work are all in `origin/main`, and their branches were deleted once each was
  fully contained in it. So if the two commands above show you a branch, it is **newer than this
  block**, not something this block forgot.

  **Do not re-add a SHA here.** A previous version of this bullet named one, plus two branches
  (`book-drawer-section-highlight`, `critique-pass-2026-07-27`) that had already been merged and
  deleted, and it sent the next session chasing all three. This file cannot track git; git can.
- **The audit slate is COMPLETE except R12.** Wave 1 (R1 adoption friction, R3 pre-mortem, R4 due
  diligence, R5 untrusted document) ran 2026-07-27. **Waves 2 and 3 plus the tail ran 2026-07-28**
  in one session: R14 deck exemptions, R6 ATAM, R7 FMEA, R2 first contact, R9 conformance/ACR,
  R11 external document, R8 value stream, R10 demand, R13 green software. **Only R12 (real-device
  mobile, Android) is left, and it needs the author's phone.** The premise held: every prior round
  asked *is this correct?*, and these asked whether it is detectable, holds under stress, would be
  adopted, and can be handed over. Spec:
  [audit slate](../docs/superpowers/specs/2026-07-27-audit-slate-design.md). Wave 1 method:
  [wave 1 plan](../docs/superpowers/plans/2026-07-27-audit-wave-1.md).
- **Band A held items 79-137 when the audits closed; a good many have since shipped** across
  four batches (79-83, 109, 117, 118, 120, 121, 127, 128; then 84, 89, 90, 92, 93; then the
  reader-cost trio 150, 137, 124; then the verified sweep 85, 86, 97, 98, 99, 114, 123, 130).
  **Two of those closed with no product change** — 130 was already fixed and this file had not
  noticed, and 99 was a measurement that came back clean — which is the rot tax, not progress.
  Item **150** was added 2026-07-28 from the author's own size question, and **137, 148 and 149
  were amended in place** rather than deleted, so what remains of each is visible. The earlier
  "nothing in any of
  these rounds changed a line of product code" is long dead, which is the usual way this file rots:
  **ask git, not this line.**
- **138 and 146 SHIPPED 2026-07-28** on `block-single-root-2026-07-28`, together with the
  module-path half of 143 — the multi-root block that half-mounted every op, and the three
  tree-derived prose gates. Detail in "Do not re-add / re-scope". **Two things that batch is
  worth reading for:** its filed blast radius was 3× the real one (2 pages → 1) and the bug is
  invisible to a spot check (editing the *first* of N roots looks correct even unfixed), and
  the path gate's first run reported six hits of which **five were dated records correctly
  describing the past** — which is why that gate excludes `notes/` and `docs/superpowers/`.
  **A parallel session was rewriting the LSP + VS Code companion in the shared tree
  throughout**, so this batch ran in a worktree and deliberately avoided `main.rs`/`cli.rs`;
  that is why item 144's CLI residuals were not taken.
- **The critique round's code band is nearly drained: 139, 140, 141 and 142 shipped 2026-07-28**
  on `critique-fixes-139-142` (LSP rename validation + the external-URL fragment, both TOC escaping
  defects, the Cmd-K scroll lock, and the manifest's icon/`start_url`/pin defects). Detail and the
  one deliberately-partial half in "Do not re-add / re-scope". **138 and 146 then shipped
  2026-07-28** with 143's module-path half, and **143 is now fully CLOSED (2026-07-28)**: the rest
  of the docs-vs-behaviour sweep shipped, and **three of its own filed claims were false** — read
  "Do not re-add / re-scope" before re-deriving any of them. **All that is left of that round is
  144** (CLI/diagnostic residuals — note its three LSP sub-items sit in files a later batch
  rewrote, so re-derive them).
- **The findings docs** (each finding carries its measurement and its refutation test). Wave 1:
  [adoption friction](2026-07-27-adoption-friction-audit.md) ·
  [pre-mortem](2026-07-27-premortem-audit.md) ·
  [due diligence](2026-07-27-due-diligence-audit.md) ·
  [untrusted document](2026-07-27-untrusted-document-audit.md). Waves 2-3:
  [R14 deck exemptions](2026-07-28-deck-exemption-audit.md) ·
  [R6 ATAM](2026-07-28-atam-architecture-audit.md) ·
  [R7 FMEA](2026-07-28-fmea-detection-audit.md) ·
  [R2 first contact](2026-07-28-first-contact-audit.md) ·
  [R9 conformance/ACR](2026-07-28-conformance-acr-audit.md) ·
  [R11 external document](2026-07-28-external-document-audit.md) ·
  [R8 value stream](2026-07-28-author-value-stream-audit.md) ·
  [R10 demand](2026-07-28-demand-positioning-audit.md) ·
  [R13 green software](2026-07-28-green-software-audit.md).
- **If your job is to pick the next batch, here is the state of the board.** The
  **launch-blocking set is empty**, the **publication-readiness set is done**, and the
  **reader-cost batch (150, 137, 124) shipped 2026-07-28** — see "Do not re-add / re-scope" for
  what each did and what it measured. Nothing in band A gates anything else; pick on value, not
  on order. The deck/conformance pair **112 + 125 SHIPPED 2026-07-28** as one step-the-deck
  browser harness — the first browser test of `deck.js` — together with **113** (the corpus
  deck's missing math + kernel cell) and **111** (the vacuous deck rows in the a11y walk);
  see "Do not re-add / re-scope". **It found two shipped layout defects on its first run**
  (clipped code blocks on 5 of 21 slides; the browser's focus ring around every
  vertical-stack slide), neither of them filed and neither visible to any emission test —
  which is the strongest evidence this file has that *rendered geometry* was an uncovered
  axis, not just an untested file. What is left in band A is mostly **words, not code**
  (94, 95, 96, 135, 136). **126 and 119 shipped 2026-07-28** — the ACR is published in the
  guide and the detection register is [DETECTION-DEBT.md](DETECTION-DEBT.md). The structural pair's
  other half (**146**) and the one item that touched the block model (**138**) both shipped
  2026-07-28.
  **Two items are rulings and must not be built without one: 101** (what licence a user's built
  page carries, given every page inlines AGPL JS — its CLA sub-bullet is discharged, the rest is
  open) and **122** (its filed fix reverses the PL14 ruling; the cost objection was measured
  2026-07-28 and is weaker than filed, so it turns on design, not milliseconds).
- **A rot warning with a fresh example, because it cost time this week.** Item 124 was filed as a
  code fix plus a static rule; by the time it was picked up the **code fix and its pin were
  already in the tree** (`chrome.rs`'s kbd already carried `aria-hidden`), and only the rule was
  left. Nothing in this file said so. **Grep the named symbol before you build the named fix** —
  that is the standing rule at the bottom of "Standing constraints", and it earned its place
  again.
- **Two measurement hazards worth knowing before you measure anything.**
  (1) `target/release/taliesin` is shared across sessions and may have been built from a *different*
  branch: check `taliesin --version` against your own HEAD before trusting any CLI result (the R14
  headline was re-verified after rebuilding, and held). (2) A table-shaped probe whose every cell is
  negative is a **broken probe** until proven otherwise — zsh does not word-split `$VAR` in a `for`
  loop, and an all-`NONE` inventory was caught only because one row had been measured by hand
  minutes earlier. Carry a known-positive row in every such probe.
- **THE LAUNCH BLOCKERS ARE SHIPPED AND PUSHED (2026-07-28): `origin/main` was at `a52afc7` when
  the next batch branched off it.** Nine items in one batch, each verified by **mutation** (restore
  the bug, watch the named test fail) and, where it was a browser claim, in a real browser:
  **80 + 117** (`mounts:` containment + its pin), **81** (`check` no longer spawns a
  project-supplied interpreter), **79 + 118** (`--no-exec` now covers `{js}`; words made honest),
  **109** (`check` *and* `build --strict` walk a site's decks), **127** (comma fences),
  **128** (link-extension did-you-mean), **120 + 121** (`new deck` in a site, and the warning that
  named a value the parser rejects). Also closed: **82** (re-grepped, the merge had done it) and
  **parts of 87 and 88** (each amended in place, not deleted, so what remains is visible).
  **Gates on that branch:** `cargo fmt --check` and `clippy --workspace --all-targets -D warnings`
  clean; full workspace suite with all four interpreter gates and `--test-threads=1` =
  **102 binaries, 1,690 tests, 0 failures, 0 ignored** (the 99/1,673 baseline plus 3 new test
  binaries and 17 tests — the totals reconcile exactly, and *zero ignored* is what proves the
  gates ran rather than skipped); both `tsc` gates exit 0; `node --test` 6/6; `check` exit 0 on
  **all 16** corpus/docs/site projects. Not re-run: `cargo audit` / `cargo deny` (no dependency
  changed).
- **THE LAUNCH-BLOCKING SET IS EMPTY.** Item 83 (five MIT tags) was the last one and is resolved:
  the tags are deleted, see "Do not re-add / re-scope".
- **The publication-readiness batch shipped next and is PUSHED (2026-07-28): items 84, 89, 90,
  92, 93.**
  `tools/gates.sh` (the one script that runs every gate and **refuses to be green when one
  skipped**), `CONTRIBUTING.md` with the inbound relicensing grant, the restored `ci.yml` **plus** a
  new `release.yml` (both guarded on `github.event.repository.private != true`, so they are
  **inert until publication** and need no follow-up commit), the measured install expectation and
  platform matrix, and the "Coming from Quarto" chapter generated from the vocabulary consts.
  Full detail in "Do not re-add / re-scope";
  **`git log --oneline origin/main..HEAD` rather than trusting this line.**
  **Gates: `./tools/gates.sh` itself, all nine, `PASSED` (exit 0)** — the workspace suite with all
  four interpreter gates and `--test-threads=1` at **105 binaries, 1,700 passed, 0 failed,
  0 ignored** (the 102/1,690 baseline plus 3 new test binaries and 10 tests, reconciling exactly),
  both `tsc` gates, `node --test`, the VS Code grammar test, `cargo audit` and `cargo deny check`.
  Zero skip lines in the test log and all four canaries printed `... ok`, which is what makes
  "0 ignored" mean the gates *ran*. `check` exit 0 on **all 16** projects. The **INCOMPLETE** path
  was exercised too (a deliberately broken `TALIESIN_PYTHON` + `--allow-missing` → 8 pass, 1
  skipped, **exit 2**), because a verdict nobody has seen fail is not a verdict.
- **Two traps this batch hit, both worth knowing.** (1) A mutation **survived**: the `mounts:`
  lexical containment check was fully shadowed by the canonical symlink check for any target that
  *exists*, so the test passed with the guard disabled. It took a row whose target does **not**
  exist to pin it. A guard can be dead and green. (2) A new test failed on its own fixture's
  **prose**: the pin doc for item 127 explains the defect, so `language-rust,ignore` legitimately
  appears on the page as inline code. Needle the emitted **tag**, never the bare class name.
- **Item 100 is RULED (2026-07-28) and no longer blocks anything.** The answer is **archive plus
  fresh public**, specced in
  [2026-07-28-public-flip-audit-design.md](../docs/superpowers/specs/2026-07-28-public-flip-audit-design.md):
  the history *is* published, the money/strategy docs leave every commit that held them, this remote
  becomes `taliesin-private-archive`, and a new public repo receives the rewritten history.
  **Neither phase runs without a separate instruction, and Phase 2 is irreversible.** The one thing
  it hands back to the backlog: **fix items 79, 80 and 81 before Phase 2**, so no still-open finding
  has to be judged as an exploit recipe. **DISCHARGED 2026-07-28** — all three shipped on
  `launch-blockers-2026-07-28` (unpushed), so they are now descriptions of *fixed* behaviour.
  Confirm that branch is merged rather than trusting this sentence.
- **LIVE COORDINATION NOTE (2026-07-28), and it will expire — verify before acting.** A parallel
  session is building **`crates/server/src/math_image.rs`** (untracked WIP at the time of writing:
  headless math-to-PNG). It is a **second `chromiumoxide` consumer**, and the batch below made that
  dependency an **opt-in `headless-js` feature that is OFF by default**. Measured: with the feature
  on the tree is clippy-clean; with the default off, `math_image.rs` fails
  `-D warnings` with three dead-code errors. **That file therefore needs to declare the feature**
  (`#[cfg(feature = "headless-js")]` on the module, or `main.rs`'s `mod math_image;`), exactly as
  `headless_js.rs` does. It was deliberately NOT edited from the other session's tree — it is
  uncommitted work owned by that session. Once it lands, add it to
  `crates/core/tests/headless_js_feature.rs`'s caller list.
- **Auditing is DONE.** All 14 slate rounds have run except **R12** (real-device mobile, Android),
  which needs the author's phone. **Do not open a new round** — a great many items are open (do
  not trust a count written here; count band A yourself) and an audit's
  value decays to zero if its findings never ship.

## State (2026-07-28)

- **Audit Wave 1 landed and refilled the board: four rounds, 30 items (79-108).** Spec:
  [2026-07-27-audit-slate-design.md](../docs/superpowers/specs/2026-07-27-audit-slate-design.md)
  (14 rounds, 3 families). The slate's thesis is that every prior round asked *is this correct?*
  while professional practice asks four other questions this project had never asked: is it
  **detectable**, does it **hold under scenario stress**, would a stranger **adopt** it, can it be
  **handed over**. That thesis paid: **three HIGH security findings, none of which is a correctness
  bug**, which is exactly why ~30 correctness rounds could not see them. They are defects only once
  a document arrives from someone else, and publication creates that condition.
- **The three HIGH findings — ALL THREE FIXED 2026-07-28 on `launch-blockers-2026-07-28`
  (unpushed).** Kept below as the record of what they were, because each one's *shape* is the
  reusable part. Item 80 was additionally reproduced end-to-end before the fix and re-measured
  after: `mounts: { escaped: /etc }` under `preview` answered `GET /escaped/hostname` with **200**
  and the contents of `/etc/hostname`; it now answers **404** with a diagnostic naming the
  boundary. What each said, as filed:
  `--no-exec` is documented as "preview untrusted docs safely" but `crates/core` contains **zero**
  references to `TALIESIN_NO_EXEC`, so `{js}`, raw `<script>` and header injection all still run
  (item 79). `mounts:` does `root.join(&m.path)` with no containment, and Rust's `Path::join`
  *replaces* the base on an absolute argument (item 80). `taliesin check` — the kernel-free,
  network-free pass an agent runs first on an unknown project — spawns the binary named by that
  project's `_site.yml` `python:` field (item 81).
- **The licence story is the other headline, and it is two separate problems.** Three tracked files
  still claim MIT while `Cargo.toml` says `AGPL-3.0-only`, and **tag `v0.2.0` genuinely ships an MIT
  `LICENSE`**, so cloning the tag gets MIT and the dual-licence moat leaks at a tag (items 82, 83).
  Separately, **nothing anywhere states what a user's *output* is licensed as** (grep = 0 hits)
  while every built page inlines bundled CSS/JS carrying no licence header. That second one is an
  owner ruling, not code (item 101).
- **One hypothesis was refuted by measurement, and the refutation is worth more than a finding.**
  This file has warned for weeks that the four hand-run gates were the most likely to have rotted
  because nothing runs them. **All four pass**, measured on a fresh clone: live Python 457 tests
  exit 0 (non-vacuous — a named kernel test printed `ok`), live R 3/3 in 20.4 s on a real IRkernel
  boot, both `tsc` checks exit 0 with `--listFiles` confirming 5 and 25 files, `node --test` 6/6.
  `cargo audit` and `cargo deny check` also exit 0. **The gates are healthy; what is missing is
  anything that makes an outsider's run non-vacuous** (item 84).
- **A second finding died on verification, which is the contract working.** The pre-mortem filed a
  live jsdelivr CDN URL in the binary. The string is real at `render/mod.rs:1532`, but
  `mermaid_url_for()` routes Preview to a same-origin vendored copy and a static Build inlines the
  library content-gated, so it is a never-reached fallback — and **OFF-2 already found and fixed
  exactly this** on 2026-07-22. What survives is narrow and became item 86: the no-CDN invariant is
  pinned on the `bare` surface and the reveal.js case only, never on a normal built page.
- **A `.tmd` from a stranger is `informed consent`, not `safe-by-default`, and that is the right
  call.** `SECURITY.md:38-41` already says so correctly. The deliverable is discoverability and
  honest wording (items 87, 88), plus two *enforcement* exceptions that are not "the document's code
  ran" but "the tool was steered outside the document by metadata": `mounts:` and `check`'s
  interpreter probe. **Do NOT reverse the CSP ruling** (2026-07-03 catalog): no CSP, no sanitizer,
  no cell sandbox.
- **The author-reported round is fully closed: 72-76 plus item 77's four residuals all SHIPPED
  2026-07-27.** 76 removed the book's right-rail TOC (owner ruling, reversing the 2026-07-06 "keep
  both nav surfaces" decision): `Site::page_toc` returns false for a book *ahead of* the page's own
  `toc:`, so one gate covers both builds and both previews, and the 14rem track the layout reserved on
  *every* chapter went with it at an unchanged text measure. `toc:` is inert in a book now, so
  `_site.yml` validation says so and the six book configs plus the `init` scaffold dropped the key.
  77 shipped as three code fixes and one refutation (next bullet). Item **56** remains an authoring
  judgment plus a feature proposal, not a task — and it is now the only thing in band A.
- **One of item 77's four was false, and the false one is the lesson.** Filed as "the pca-geometry
  scree plot bakes `tick_params(colors=\"white\")` into a PNG, so it is unreadable on a light page."
  Rendered through a real kernel and read in a browser: **perfectly legible.**
  `MPL_THEME_PREAMBLE`'s `_tali_recolour` overrides every `Text`, spine and tickline before each of
  the two inline PNGs, so a hardcoded foreground never reaches the page. The item's *second* claim was
  the real defect (the bars still carried the pre-72 light-on-dark palette while the neighbouring 3-D
  arrows had been retuned, so two adjacent figures named one colour in two shades). Same shape as
  DT-5: **a filed cause is a hypothesis, and this is the second in two days that did not survive being
  measured.**
- **That round's own defect is the one worth remembering.** Item 74 shipped two brand SVGs that were
  served `200 image/svg+xml`, were copied into the build correctly, and painted a **broken image** on
  the forward-facing blog: each carried a CSS comment naming a tag in angle brackets, and an SVG
  `style` element is XML, not HTML, so it is not an implicit CDATA section. Every check passed because
  "the file exists and is served" is not "the file renders" — it took a browser to see it.
  `crates/core/tests/svg_assets_render.rs` now pins both properties an `img`-loaded SVG needs.
- **The duplicate item 70 resolved itself, and not the way it first looked.** Two items were filed as
  70 on the same day, and the obvious fix was to keep the deck-letterbox one (AUDITS.md pointed at it
  as "items 70-71") and renumber the `_site.yml`-boundary one. **That would have renumbered the
  survivor.** The letterbox item was DT-5, which was **retracted the same day as false** (`5e92816`,
  on `origin/main`): the probe intersected each neighbour with the *viewport* instead of its
  *clipping ancestor*, and `.tali-deck`'s `overflow: hidden` had been clipping them all along. So
  band A's 70 no longer exists and the `_site.yml` item keeps **70** as the original claimant. (77 was
  later issued for something unrelated, and has since shipped.) **The lesson is about this file, not
  about decks:** a numbering collision can be
  the symptom of a bad filing rather than a clerical slip, so check whether both items are *real*
  before renumbering either.
- **A lens is the best opener: band A holds no code work and band B is empty.** (Band D gained
  **78** on the way out of 77 — a real tool defect, but a filed-not-scoped one with no obvious fix,
  which is why it is in D and not A.) Standing recommendation:
  **real-device mobile** — unblocked, and that round verifies rather than re-finds since batch 1
  shipped. First thing to check on real hardware is the drawer scroll lock: `overflow: hidden` on the
  root holds less completely on iOS Safari than on Chromium, and only Chromium was measured. The
  drawer also just became a book's *only* navigation surface (item 76), which raises what a
  real-device failure there would cost.
- **The author-reported round is worth a method note** ([LESSONS.md](LESSONS.md) candidate): six
  observations from *using* the product produced four real defects, one broader than reported (72) and
  one already fixed but still advertised (75). Two of the four are stale strings that every automated
  gate passes over, because no gate compares prose against behaviour.
- **Nothing is owed by the author.** The last item needing a human (the in-editor click-to-source
  round-trip) was verified 2026-07-25. **That coverage gap is permanent:** the relay harness passes
  both directions but stops at the relay and cannot see whether the editor lands the cursor, so any
  future change to the relay or the companion re-opens the same manual check.
- **Do not trust this file's freshness.** The author pushes mid-session with no signal here, and a
  scoped prune leaves the rest looking freshly reviewed. **No commit counts and no SHAs are
  recorded** — a count written *into* this file is invalidated by the commit that writes it. Ask git:
  `git log --oneline origin/main..main`.
- **A six-critic adversarial round also landed 2026-07-28, on the other branch, and its code is now
  merged here.** Method and full findings:
  [2026-07-28-launch-critique.md](2026-07-28-launch-critique.md) (1,088 lines). Six hostile critics on
  disjoint surfaces, then a defender per critic whose job was to refute; only findings the defender
  could not kill became items. **Its 12 items were renumbered 79-90 → 138-149** in this merge, because
  Wave 1 had already issued 79-108 and its numbers are referenced from nine files against three.
  That round's findings doc uses internal IDs (`BL-1`, `CJ-1`, …), not backlog numbers, so nothing in
  it needed rewriting.
- **That round's own lesson is the one to carry: a fix lands in one file and misses its sibling.** It
  happened three times in one session, twice to fixes made *during* the round — `THIRD_PARTY.md` was
  corrected to AGPL while `docs/internals/repository.tmd` still said MIT; `deny.toml`'s header lost
  its CI claim while a comment twelve lines below kept one. Both were caught only because a defender
  re-read the fixed files. **Fix the class, grep the repo for the shape, and gate on the shape — never
  on the sentence you happened to fix.**
- **Three of that round's proposed fixes were wrong in ways that would have shipped a NEW false
  sentence**, and its defender caught each. **Before applying any fix text from that findings doc,
  read its correction note.** The worst: a proposed defence-in-depth would have dropped `"tmd"` from
  `SKIP_EXT`, which `mirror_assets` uses to *exclude* sources — it would have copied every `.tmd` in
  the project into the deploy. A defender also refuted one of that round's own findings outright (the
  stale "mermaid is the sole CDN dep" sentence is in `notes/`, not in the shipped `THIRD_PARTY.md`,
  which is accurate and drift-locked). **Its findings are adjudicated, not assumed.**
- **Gates re-run on the MERGED tree (2026-07-28, `integration-2026-07-28`), which is the figure to
  trust — it is the first time both branches' code was built together:** full workspace suite with
  all three interpreter gates and `--test-threads=1` = **99 binaries, 1,673 tests, 0 failures, 0
  ignored** (zero ignored is the check that the gates were live, not skipping); `cargo fmt --check`
  and `clippy --workspace --all-targets -D warnings` clean; the **fourth** gate
  (`TALIESIN_REQUIRE_CHROME=1 --test read_run_js`) **3 passed**; **both** JS `tsc` gates exit 0;
  `node --test crates/server/src/assets/_middleware.test.mjs` **6 pass**. The totals match the
  critique branch's own pre-merge figure exactly, so **the merge introduced no regression**.
  Not re-run after the merge: `check` over the corpus/docs projects, and `cargo audit` /
  `cargo deny` (no dependency changed). Earlier context: the 76 + 77 batch (2026-07-27) measured the
  same suite at 1,671 tests with `check` clean on all 15 corpus/docs projects; the critique branch
  additionally verified the built guide + site carry **0** leaked `.tmd` sources and **0**
  unrewritten `.tmd` hrefs (both were 1 before).

## Standing constraints (read before working)

- **Do-NOT-touch (one freeze):** `MAX_WARM_PAGES` + the deterministic LRU eviction in
  `serve_site/exec_pool.rs` (M6a, sign-off refused 2026-07-17) and the **single-editing-surface**
  invariant (the preview is read-only; it must never write back to source). The rest of the
  exec/kernel zone is not frozen.
- **Website / brand** (2026-07-11 audit, detail:
  [2026-07-11-website-design-audit.md](2026-07-11-website-design-audit.md)): the personal blog
  (`corpus/tech-blog/`) is the forward-facing brand, direction **"Marginalia"**; its 14 explicit KEEPs
  live in that file. Every change stays invariant-safe: no CDN, no preview write-back, no new output
  format, offline bundling, `--tali-*` tokens only.
- **Author policy:** feature-first (finish framework features before marketing-site work).
- **Working method:** branch per feature; brainstorm if there's a fork; spec under
  `docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
  extension harnesses); fast-forward merge locally; **delete the item here when it lands.** Push to
  `origin/main` only when the author asks. **Review subagents get a git worktree or you commit
  first** (a "read-only" reviewer with `Bash` still writes scratch files to your CWD; one ran
  `cat > Cargo.toml` in the repo root and destroyed the workspace manifest).
- **Tests: three gates, or the suite silently under-tests itself:** `TALIESIN_REQUIRE_NODE=1`,
  `TALIESIN_R=R TALIESIN_REQUIRE_R=1`, `TALIESIN_PYTHON=… TALIESIN_REQUIRE_KERNEL=1` (a missing
  interpreter must be a hard fail, not a skip). `cargo test` aborts the remaining binaries at the
  first failure, so re-run before trusting a total. A **fourth** gate nothing else runs:
  `TALIESIN_REQUIRE_CHROME=1 --test read_run_js`.
- **A red `exec`/`kernel` probe is now real signal, not a coin flip.** The flake was fixed 2026-07-25
  (a port race in `prepare_connection`; the re-roll now lives on `Kernel::start_with_retry`, and
  `crates/server/tests/kernel_start_is_retried.rs` fails if any caller reaches the un-retried
  primitive). Verified 0 failures in 45 post-fix runs under the same load.
- **`corpus/tarn` is the fixture for scale-sensitive work** (12 numbered chapters, 3 parts + a nested
  part) and deliberately carries the shapes the rest of the corpus lacks. **Use it instead of minting
  a fixture.** It is a *documentation* book, not a scale fixture: do NOT grow it toward 200 pages and
  do NOT mint `corpus/longbook` (the walker renders every corpus doc on every `cargo test`).
- **Git:** do not trust a SHA written in notes. Check `git log --oneline origin/main..main` for what
  is unpushed and `git reflog show origin/main` before believing any "not pushed" claim.
- **How this file lies to you:** entries rot. Before picking an item, **grep its named symbol/flag in
  source** and prefer measuring the running product over reading this file. Trust an item's
  *symptom*, never its cause, line number or stated cost. Verify a fix by **mutation** (restore the
  bug, watch the named test fail), not by a green suite. **What would ship silently is tracked
  per class in [DETECTION-DEBT.md](DETECTION-DEBT.md)** — a live register, updated in the same
  change as the fix, not a dated findings doc. **The full trap catalogue — probes,
  instruments, cargo-mutants scoping, the coverage illusions — is in [LESSONS.md](LESSONS.md); read
  it before writing a probe or a pin.**

## Audit lenses — the menu, since the table in AUDITS.md is not one

[AUDITS.md](AUDITS.md)'s round index is a *record*: a further round needs a lens proposed first.
Ranked; take from the top. **L1, L2, L4 and L5 have run and closed.**

**Standing recommendation — real-device mobile.** The 2026-07-26 round was Chromium emulation, which
does not model WebKit, momentum scroll, the dynamic viewport toolbar or safe-area insets. Everything
it should cover is the "Not measured" list in [2026-07-26-mobile-audit.md](2026-07-26-mobile-audit.md):
real iOS Safari / Android Chrome, a phone screen reader, tablet widths, and the `--host` QR
phone-preview flow, which is a first-class phone feature that got no coverage at all.

**Never run:**

- **L3. The subsystems that post-date every lens that would own them — PARTIAL.** `headless_js.rs` was
  read; `lsp.rs` (1,922 lines, 07-21), `complete.rs` (1,157), `skim.rs` (647) and `manifest.rs` (303)
  were not, though the mutation campaign has since pinned much of what it would have looked at.
  `lsp.rs` is younger than the security (07-17), DX (07-18), mutation (07-18) and polish (07-19)
  rounds, and only AP10 has read it. The web manifest is a *phone* surface (add-to-home-screen,
  standalone display) the mobile round did not touch.
- **L6. A real external document — BLOCKED** on a repository that is not on this machine. All four
  demand probes were fixtures written for the probe; the FL-weather Quarto book (Tier 3) is the fifth
  and the only one the corpus cannot fake.

**Re-runs, ranked by age × churn measured in each round's own surface (2026-07-26):**

- **The deck audit (07-12) is the most rotted:** 2,510+/1,196- in `deck.rs` + `deck.js` + `deck.css`
  since, and the mode-model was deliberately reshaped after it (reader + PDF deleted, phone feed
  added, motion round 07-24). AUDITS.md already warns the doc describes *outgoing* behaviour. Re-run
  it **crossed with touch**, not as-is.
- **The website/brand audit (07-11):** its headline performance finding measured per-page inlining and
  is now obsolete (hashed `_assets/`), which is itself the signal. Its Lighthouse pass was
  desktop-mode only, which is how it missed the touch-target defects the mobile round found.
- **The security release audit (07-17)** should wait for the flip date it is parked on (item 25),
  **except** `headless_js.rs` and the LSP, which post-date it and spawn or expose processes.
- **Not due:** AP10 (07-23). **Closed, do not re-scope:** the mutation / vacuous-test round — every
  survivor it measured is triaged, and a re-run's only new information would be about code written
  since. Its numbers and method are in [LESSONS.md](LESSONS.md) plus
  [server half](2026-07-27-mutation-server-half-complete.md) and
  [`lsp_nav.rs`](2026-07-27-mutation-lsp-nav-complete.md).

**Unblocked by progress already made:**

- **Real iOS Safari / Android Chrome, a phone screen reader, the `--host` QR flow** — the author is
  now device-testing.
- **Deck touch gestures** (item 4) — device blocker gone, and the mobile round confirmed the feed
  itself works, so pinch/pan is testable.
- **Fuzzing the LSP + MCP request loops** (an AP2 residual). HEALTH-1 shipped, so `serve::guarded`
  wraps both dispatches (`lsp.rs:105`, `mcp.rs:127`): there is finally a survival property to assert.
- **Reader-surface work that needed section extents** — `data-section-end` shipped 07-26, so the four
  skimmability proposals blocked on "zero `<section>` extents" have substrate.
- **Still blocked:** the prune half of the release audit (gated on the public-flip date), and true
  WebKit unless the phone is an iPhone.

**Superseded 2026-07-28 by the audit slate.** "Auditing is done for now" was true of the *lens menu*
above and false of auditing: the menu was exhausted because every lens on it asked the same question.
The standing menu is now
[2026-07-27-audit-slate-design.md](../docs/superpowers/specs/2026-07-27-audit-slate-design.md) —
14 rounds in 3 families, built from consulting-practice instruments (ATAM, FMEA, pre-mortem, JTBD
four forces, technical due diligence, VPAT/ACR, value-stream mapping) rather than more code-reading.
**Wave 1 (R1, R3, R4, R5) ran 2026-07-27/28 and produced three HIGH security findings**, refilling
this file from one non-coding item to 30. Remaining waves:

- **Waves 2 and 3 and the tail all RAN on 2026-07-28** (R14, R6, R7, R2, R9, R11, R8, R10, R13) and
  produced items 109-137. Their durable artefacts, so a later round does not rebuild them: the
  **deck exemption register** (R14), the **sensitivity/tradeoff register** (R6), the **D≥8 detection
  cluster** (R7), the **draft ACR** (R9) and the **external-document shape inventory** (R11).
- **R14's premise was too generous by an order of magnitude**, which is the reusable lesson: the
  two documented `DocFormat::Reveal` exemptions turned out to be *correct* (a duplicate-heading rule
  would be 100% false positives on the `auto-animate` idiom, measured), while the real hole was that
  a deck in a site **never reaches the code those exemptions live in**. Scoping a round from the
  exemptions that are *written down* finds the wrong thing.
- **ONLY R12 REMAINS — real-device mobile (Android), and it needs the author's phone.** Still the
  only lens with a HIGH track record here; Wave 1's pre-mortem independently re-priced it as
  launch-blocking. Priority order is in the slate spec: the book drawer scroll lock first (item 76
  made the drawer a book's *only* nav surface), then the `--host` QR flow, momentum scrolling and
  the dynamic viewport toolbar, tablet widths, TalkBack. **Record explicitly that an Android round
  does not cover WebKit/iOS**, or it will later read as full mobile coverage.
- **The slate is otherwise exhausted. Do not open a new round before the open items ship.**
  AUDITS.md's own stop-auditing ruling applies with more force now than when it was written: an
  audit's value decays to zero if its findings never ship, and there are now three waves of them.

**A note on why the deck was under-audited, because the mechanism is reusable.** Nothing in this
repo forbade it. What existed was three compounding layers: code-level diagnostic exemptions (each
individually well-reasoned), **eight** "declined / retracted / do-not-re-scope" deck entries in this
file, and sessions reading that thicket as coverage. The first draft of the audit slate did exactly
that and wrote "no deck audit" without measuring anything. **A dense do-not-touch cluster is not
evidence of coverage; it is a reason to measure.**

## Open items

**Ranked for implementation, not by theme.** Band A is what a session can build today; B is buildable
but not worth a session alone; C, D and E are blocked and are listed so they are not re-scoped.
**Item numbers are stable** and referenced from the findings docs and [AUDITS.md](AUDITS.md): they are
NOT renumbered when the order changes, and a closed item's number is never reused.

**Standing rule for a batch:** branch per batch, verify each fix by *mutation*, browser-verify
anything client-side, and **delete the item from this file when it lands**.

### A. Build now

**Refilled 2026-07-28 by audit Wave 1.** Findings docs:
[adoption friction](2026-07-27-adoption-friction-audit.md) ·
[pre-mortem](2026-07-27-premortem-audit.md) ·
[due diligence](2026-07-27-due-diligence-audit.md) ·
[untrusted document](2026-07-27-untrusted-document-audit.md).
**Items 79-81 are the launch blockers.** Each finding in these docs carries the measurement that
produced it and the observation that would refute it; trust the symptom, re-derive the cause.

**Security: the three HIGH findings — ALL SHIPPED 2026-07-28** on branch
`launch-blockers-2026-07-28` (items 79, 80, 81, with their pins 117 and 118). Details in
"Do not re-add / re-scope". Item **82** is also gone: the merge closed it and a re-grep
confirms zero surviving self-MIT claims, which was the whole action it asked for.

**Licence correctness (publication blockers)**

**Licence at a tag: RESOLVED 2026-07-28 (was item 83) — see "Do not re-add / re-scope".**
The five pre-relicence tags are deleted. Their commits remain in `main`'s history, so nothing
was lost but the labels; the SHAs are recorded there in case a snapshot is ever wanted again.

**Making an outsider's run mean something: SHIPPED 2026-07-28 (items 84, 89, 90) — see
"Do not re-add / re-scope".** `tools/gates.sh`, a root `CONTRIBUTING.md`, and the restored
workflow (guarded on repository visibility so it stays inert until publication).

**Honesty of shipped words**

88. **One shipped string left of the three.** (MEDIUM.) Two fixed 2026-07-28 with item 79:
    `docs/internals/repository.tmd`'s false "`--host` auto-enables `--no-exec`" claim (verified
    false in `cli.rs` — `expose` never sets `no_exec`), and the verbatim injection of
    `include-in-header`/`include-before-body`/`include-after-body`/`css:`, now stated in the CLI
    reference's new "Documents you did not write" section. **Still open:** `SECURITY.md`'s
    symlink allowance assumes *you* placed the symlink, which is false for an untrusted archive.
    Same family as item 75: **no gate compares prose against behaviour.**
87. **Two of the three surfaces remain.** (MEDIUM.) The section itself shipped 2026-07-28 with
    item 79: `docs/guide/reference/cli.tmd` now carries **"Documents you did not write"** (what
    executes, what passes through verbatim, what `--no-exec` does and does not do, and the two
    things that are *enforced* rather than documented), and both `--help` surfaces point at it.
    **Still open:** a link from `README.md`, and the one-line first-run notice.
    `SECURITY.md:38-41` already takes the right position.

**Smaller, verified**

**Items 85 and 86 SHIPPED 2026-07-28 in the verified sweep — see "Do not re-add / re-scope".**

**Install expectation, binaries and platform matrix: SHIPPED 2026-07-28 (item 92) — see
"Do not re-add / re-scope".** *Not* done, and deliberately not re-filed as a code item: **hosted
docs and a hosted demo**, which are a deploy decision belonging with item 100's Phase 2.

**"Coming from Quarto": SHIPPED 2026-07-28 (item 93) — see "Do not re-add / re-scope".**
94. **A "Your source stays yours" paragraph, with the measured number.** (MEDIUM.) Measured across
    all 115 corpus docs (10,118 lines): at most **8.59%** of lines carry any non-CommonMark
    construct, and all six families are Pandoc/Quarto vocabulary, not invented. Three exits already
    exist (Markdown source, `read --format json`, runtime-free static HTML) and none is *named* as
    one. **The "no exit path" anxiety is refuted; the fix is to say so.**
95. **A continuity paragraph: one maintainer, pre-1.0, and what leaving costs.** (MEDIUM.)
96. **Quantify the dogfooding claim.** (LOW.)

**Items 97, 98 and 99 SHIPPED 2026-07-28 in the verified sweep — see "Do not re-add / re-scope".**
99 needed no code: it was a measurement, and it came back clean in both directions.

---

**Audit Waves 2 and 3 (2026-07-28) — items 109-137.** The slate is **complete except R12**. Eight
rounds ran in one session: [R14 deck exemptions](2026-07-28-deck-exemption-audit.md) ·
[R6 ATAM](2026-07-28-atam-architecture-audit.md) ·
[R7 FMEA](2026-07-28-fmea-detection-audit.md) ·
[R2 first contact](2026-07-28-first-contact-audit.md) ·
[R9 conformance/ACR](2026-07-28-conformance-acr-audit.md) ·
[R11 external document](2026-07-28-external-document-audit.md) ·
[R8 value stream](2026-07-28-author-value-stream-audit.md) ·
[R10 demand](2026-07-28-demand-positioning-audit.md) ·
[R13 green software](2026-07-28-green-software-audit.md).

**R7 re-ranked the launch-blocking set by severity-with-detection**, which changed Wave 1's order:
**80** (S9 D9) and **83** (S9 D10) now rank above **79**, because 83 ships a wrong licence at a tag
today and nothing anywhere would catch it. **109** joins them.

**Do the pins with their fixes.** 117 pairs with 80, 118 with 79. (R9's static-rule proposal
paired with 124; both shipped 2026-07-28.) A fix without its pin cannot be verified by mutation,
which is this file's standing rule.

**Launch blockers**

**Then**

122. **`check` says "no problems found" on a document whose code cell cannot run — BUT ITS
     PROPOSED FIX REVERSES A DATED RULING, so read this before building it.** (MEDIUM.)
     Measured cold: plain `check` prints exactly that and exits 0, while `build` on the same file
     warns twice and `doctor` names the missing package. The Environment section is shown **only**
     to a user who already passed `--require-kernel`. **Do not** make that flag the default (it
     would break the kernel-free property). Filed fix: print the Environment line unconditionally
     when the document contains a code cell, exit code unchanged.
     **The conflict, found 2026-07-28 while shipping item 81** (which touches this exact code):
     that is **PL14**, a deliberate decision with a spec
     ([2026-07-19-pl14-check-env-footer.md](../docs/superpowers/specs/2026-07-19-pl14-check-env-footer.md))
     and a test that pins it by name — `default_human_check_omits_the_environment_block`
     (`check_cli.rs`), whose comment states the reason: the footer "duplicated `doctor` on every
     keystroke/CI run". Implementing 122 as filed deletes that test. **So this is an owner ruling,
     not a task.** A shape that satisfies both is available and is the recommendation: print the
     Environment *line* unconditionally but keep **not probing** by default, so the line names the
     interpreter that would be used and says it was not spawned. Item 81 already built exactly that
     reporting shape (`runs: null` + `not_probed`), so the remaining work is only where the line is
     printed. **Cost to check first:** `collect_environment` re-renders every page of a site to find
     used languages, so putting it on the default path doubles a site `check`'s render work
     (`check <site>` was measured at 538 ms). Measure that before wiring it in.
     **MEASURED 2026-07-28 (release `b8c93bb`), and the cost objection is weaker than filed.**
     Default vs `--require-kernel` (which is the only surface that collects the environment today),
     best of three each: `docs/guide` (20 pages) **0.36 s → 0.54 s**; `corpus/tech-blog` (17)
     **0.54 s → 0.79 s**; `docs/internals` (15) **0.21 s → 0.31 s**; `site` (5) **0.11 s → 0.15 s**.
     So it is **about +50%, not a doubling**, and **+100-250 ms absolute** on the largest projects
     in the tree — and the "538 ms" in the line above did not reproduce as a *baseline*: 540 ms is
     the whole `--require-kernel` run on the slowest project. **That delta is an upper bound for the
     recommended shape**, because it includes actually spawning the interpreters, which the
     "name it, do not probe it" line would not do. The remaining open question is not cost but
     whether used-language detection can be had without the render walk; if not, the walk is what
     the +50% buys.
**Items 112, 125 and 113 SHIPPED 2026-07-28 in the deck-harness batch — see "Do not re-add /
re-scope".** The eleven deck shapes 113 listed and deliberately did *not* build (table,
footnote, citation, `{r}`, theorem envs, tabset, `@fig-` + captioned figure,
`{{< include >}}`, `{{< video >}}`, `logo:`, `theme:`, `lang:`, `css:`) are still absent from
every deck in the tree, and are still deliberately unbuilt: the walker renders every corpus
doc on every `cargo test`.
129. **Shape inventory from two real external documents — the durable half of R11.** (MEDIUM, mostly
     a record.) What real documents contain that `corpus/` has nowhere: `lang,attr` fences (734
     occurrences → item 127), ` ```console ` (209), links with a non-`.tmd` extension (128 → item
     128), a `SUMMARY.md`-driven chapter spine, **112 pages in one flat directory** (the largest
     corpus project is 14), and chapter files with **no front matter at all**. **Do NOT grow
     `corpus/` toward these** — the walker renders every corpus doc on every `cargo test`. **Pin
     only the two that earned it** (127 and 128) — **both pinned and shipped 2026-07-28**: the
     `lang,attr` fence is now a fixture in `corpus/highlight.tmd` with its own test, and the
     link-extension shape has `crates/core/tests/migrated_link_extensions.rs`. The rest are
     recorded so a later round does not re-derive them.
135. **Five verified positioning claims the project has never made.** (MEDIUM, words not code.)
     Each backed by a real user asking for what Taliesin already does: marimo
     [#3114](https://github.com/marimo-team/marimo/issues/3114) (edit in your own editor, outputs
     alongside) = the single-editing-surface architecture; marimo
     [#1379](https://github.com/marimo-team/marimo/discussions/1379) (the format is hard for humans)
     = `.tmd` is Markdown; marimo
     [#2675](https://github.com/marimo-team/marimo/issues/2675) (reload on disk change) = 90 ms,
     measured; Quarto [#4201](https://github.com/quarto-dev/quarto-cli/discussions/4201) (avoid
     kernel restarts) = the warm kernel; Quarto
     [#3674](https://github.com/orgs/quarto-dev/discussions/3674) +
     [#10429](https://github.com/orgs/quarto-dev/discussions/10429) (render only changed files /
     freeze a single cell) = the per-cell cumulative hash. **The sixth is the strongest:** Quarto's
     tracker carries *stale-output* complaints against `freeze`, and Taliesin's cumulative key makes
     a stale hit **structurally impossible**. That reproducibility guarantee is unclaimed by anyone
     and is stated nowhere in this repo.
136. **State the speed story with the measured absolutes, and no multiplier.** (MEDIUM.) Public
     Quarto threads report 1-2 s/document and ~400 s for a 376-document blog; Taliesin measures
     3.8 ms/page (112-page real book), 4 ms first paint, 90 ms warm edit. **These measure different
     work** (Pandoc + possible execution vs a cache replay) — publish the architecture and the
     absolute numbers, never a ratio.
**Low**

**Items 114, 123 and 130 SHIPPED 2026-07-28 in the verified sweep — see "Do not re-add /
re-scope".** 130 needed no product change at all: it had already been fixed and this file had
not noticed, which is the rot warning in the RESUME block earning its place a second time.

131. **The cold-build cliff: 3,981 ms vs 789 ms warm.** (LOW, and probably correct as-is.) Filed so
     it is not rediscovered as a defect. Kernel *variable* state is never cached — the property that
     makes the cache trustworthy — so a cold start genuinely cannot skip work unless the whole
     document is unchanged. **The waste is inherent to a correctness guarantee worth keeping.**

**Resolved by ruling, not work**

116. **The positional cascade vs a Python DAG — CLOSED, do not build.** R6 measured the cascade is
     unfelt at corpus scale (max 11 cells anywhere) and that a DAG is not a small change because
     kernel variable state is never cached. R10 then found the demand evidence points the other way:
     reactivity is marimo's claim and well made, while **reproducibility is unclaimed by anyone** and
     Taliesin has the stronger implementation. **Tell the cascade story properly (item 135); do not
     build the DAG.**
132. **Not a separate item — R8's value-stream pricing of 109.** A deck's defects are found by an
     *audience*, the latest and most expensive point in the stream, while every other defect class
     in this tool is caught in the 90 ms loop or by `check`. That asymmetry is the argument for
     109's priority, and it is one no correctness framing produces. Number retained (never reused).
133. **Not a separate item — R8's value-stream pricing of 127/128.** **447 of the 457** diagnostics a
     real external book produces are the tool's vocabulary gap, not the author's mistakes, so a
     migrated document costs a triage pass before any real work starts. Anxiety with a stopwatch on
     it. Number retained (never reused).

---
---

**The 2026-07-28 critique round — items 138-149** (issued as 79-90 on branch
`critique-pass-2026-07-27`, renumbered in the merge because Wave 1 already held 79-108). Six hostile
critics on disjoint surfaces, each with a defender whose job was to refute; **every item below was
conceded by a defender that tried to kill it**, and each carries a measured repro in
[2026-07-28-launch-critique.md](2026-07-28-launch-critique.md).

**That branch's 14 commits are merged here**, so the deck source leak, the LSP `languageId` gate, the
stale-prose gate (twice), the AGPL/MIT contradiction, the false CI claims, the landing page's
imaginary pen tool and ~10 other false doc claims are **already fixed — do not re-fix them.**
**Three of that round's proposed fixes were wrong** and its defender caught each: read the correction
note in the findings doc before applying any fix text from it.

**Item 138 SHIPPED 2026-07-28** on `block-single-root-2026-07-28`, with **146** and the
module-path half of **143** — see "Do not re-add / re-scope".

144. **Diagnostic and CLI residuals a first-hour user hits.** Each small, each measured.
    A timeout-killed cell is reported to the console as "raised an uncaught exception" because
    the timeout has no `NOT_RUN_` kind (`kernel.rs:866` bypasses the marker) — **and the guard
    test at `build.rs:3027` passes vacuously for the live path, so fix the test in the same
    commit**; single-doc `build` prefixes diagnostics with a bare `file_stem()` that no editor
    can open (thread a display label — `fallback` is load-bearing for the freeze path *and* the
    page title, so do not just swap it); the missing-kernel warning prints twice, the short form
    adding nothing; a missing input file reports `(os error 2)` with no did-you-mean though
    `closest` already exists; `skim` is missing from `taliesin --help` (add the `COMMANDS` ↔
    `usage()` parity gate modelled on `env_help_lists_every_runtime_env_var`); `codeAction`
    builds quick fixes from **any** provider's diagnostic and ignores the requested range; a
    message after `shutdown` exits 1 (editors read that as a crash) while a bare `exit` exits 0;
    CRLF `documentSymbol` ranges run one column long.

145. **Retired into item 137, which SHIPPED 2026-07-28** — see "Do not re-add / re-scope". The
     critique round filed this as its item 86, the unreferenced `_assets/` payload, independently
     of R13's item 137. Its mechanism analysis is what made the fix buildable: it was right that
     the predicates cannot be evaluated where `write_asset_bundle` runs, and the shipped fix took
     neither of the two routes it proposed — it votes off the **emitted href** after the pages
     exist, which is exact rather than over-inclusive. Number retained, never reused.

**Item 146 SHIPPED 2026-07-28** (all three candidates) — see "Do not re-add / re-scope".
**One residual, and it is item 144's, not this one's:** the `COMMANDS` ↔ `usage()` *command*
parity gate (`skim` is missing from `--help`). The flag half shipped here; the command half
sits in `main.rs`, which a parallel session is rewriting, so it was deliberately not raced.

56. **L5-1 residual: the manual's cross-page references.** (The `description:` half shipped
    2026-07-26: 0 of 36 tracked pages → 36 of 36.) What is left is not the authoring pass the item
    assumed, and splits two ways:
    - **Glossary, term index and float digest have no surface to feed.** `glossary`, `term-index` and
      `float-digest` grep to **zero** across `crates/core/src` + `crates/server/src`, so "they render
      empty until an authoring pass happens" describes a *feature proposal*, not authoring work.
      Writing `{.definition}` blocks today feeds only `skim.rs`, which reads them as statement heads.
    - **Backlinks ship and render nothing, and authoring genuinely could fix that.**
      `site/backlinks.rs` builds its reverse index from **cross-page** xref markers; the books' 33
      xrefs (17 guide + 16 internals) are all intra-page, so **0** "Referenced by" lines are emitted
      in either book. Real cross-chapter references would light it up, but they have to be references
      someone means — a writing judgment, not a sweep.

### B. Buildable, but low yield on its own

**Empty.** Item 77's four residuals were the last occupants and shipped 2026-07-27. The band's own
lesson held again: an item here is cheap to build and therefore easy to build *without asking whether
it should be*, and **one of the four closed on evidence rather than code** (77's scree plot was filed
as unreadable-on-a-light-page and measured perfectly readable, while the figure it never named was
the broken one). Refile here only after re-deriving the cause from source.

### C. Blocked on an owner ruling (not a task until then)

100. **RULED 2026-07-28 — the answer is "archive plus fresh public", and it is specced.** See
     [2026-07-28-public-flip-audit-design.md](../docs/superpowers/specs/2026-07-28-public-flip-audit-design.md).
     The ruling threads the needle both routes below missed: **the history IS published** (1,608
     single-author commits are the evidence a grant applicant wants), and the private planning docs
     leave *every* commit that ever held them. Mechanism: relocate the purged docs to
     `~/Documents/personal/taliesin-private/`, rewrite history, rename this remote to
     `taliesin-private-archive` (stays private, complete backup), create a **new public**
     `AJBogo9/taliesin` and push the rewritten history there. No force-push, no destructive remote
     op, and the private blobs never reach the public repo at all. Zero forks and never having been
     public is what makes it cheap. **Kept, not purged:** security audits, `.claude/`,
     `docs/superpowers/`, `AGENTS.md`, `LESSONS.md` — for the stated goal those are the exhibit.
     **Purged:** money and strategy documents only, plus ~11 commit subjects that name them.
     **Execution status: NOT STARTED and not to be started without a separate instruction.**
     Phase 1 is a read-only audit and is safe whenever wanted; **Phase 2 is irreversible** and is
     additionally gated on Phase 1's findings being signed off.
     **What still lands on this item:** the spec's own D-checks (incl. the provenance check on
     corpus documents), and its rule that any still-open finding reading as an **exploit recipe** is
     reported for individual judgement, default keep — which is exactly items **79, 80, 81**, so
     **fix those before Phase 2** rather than deciding whether to redact them. **All three are
     FIXED (2026-07-28, `launch-blockers-2026-07-28`, unpushed)**, so this clause is discharged once
     that branch is merged — verify, do not trust this line. **New input for Phase 2 from item
     83:** five local tags ship an MIT `LICENSE` and none has been pushed, so whether tags travel
     to the new public repo belongs to this spec.
     *Original framing, kept because it records why the ruling was hard:*
     `notes/STARTUP-PLAN.md:126` records a plan to publish as a **fresh repo with no history**
     ("Keep this repo private forever; the public one is a separate repo"), *not* to flip this
     repo's visibility. Those two routes resolve different findings, so the prune work cannot be
     scoped until this is ruled. Two hard facts either way:
     - **`notes/FUNDING-RESEARCH.md` and `notes/STARTUP-PLAN.md` are git-TRACKED while their own
       text says they must not be** (`FUNDING-RESEARCH.md:4` "keep this file out of";
       `STARTUP-PLAN.md:119` "remove anything private: `STARTUP-PLAN.md`"). They carry the ***REMOVED***
       analysis, a table of named funders being skipped and why, a funder's contact address, and
       "***REMOVED***". The 2026-07-17 round already filed this and
       recorded the prune as **not done**.
     - **A fresh `git init` fixes none of the tree-level findings** and discards the 1,573-commit
       process record, which is the strongest evidence an individual grant applicant has. The
       due-diligence doc's §6 proposes a third route (targeted `filter-repo`) with its honest cost.
     Supersedes the "flip-day artefact checklist" framing; **extends item 25, does not replace it.**
101. **State the licence position on what Taliesin *emits*.** (HIGH, ruling.) Measured: **zero**
     statements across README, LICENSE, THIRD_PARTY, SECURITY and both books' source about what a
     user's output is licensed as. It is a genuinely non-obvious question here because every built
     page inlines AGPL-licensed CSS/JS (`base.css`, `deck.js`, `tali-js.js` spot-checked, **none
     carries a licence header**), so a user's blog contains AGPL material. **No licence change is
     proposed** — the finding is that the position is unstated, and stating it discounts the
     licence, bus-factor and portability anxieties at once.
     **Narrowed 2026-07-28:** the other branch added `crates/core/assets/js/LICENSES.md`, which
     carries the full permission notices for the **vendored third-party** bundles. That is adjacent
     and good, but its own text says "This covers the redistributed third-party bundles only", so
     **it does not answer this item**: what a *user's built page* is licensed as, given it inlines
     Taliesin's *own* AGPL scripts, is still unstated.
     **Merged in from the critique round's item 88 (2026-07-28), which found the same question
     independently with sharper evidence and a named remedy** — that item is retired into this one:
     - **Mechanism, measured.** Taliesin's own runtime JS is `include_str!`'d into every page a user
       builds (`render/mod.rs:1658-1660` plus the `code-enhance/` fragments): a probe page measured
       **1.2 MB with 13 `taliEnhancers` hits and zero licence statements**. If that runtime is AGPL,
       arguably **every page a user publishes is an AGPL work** — an adoption tax larger than §13's,
       landing on *document authors* rather than on a hosted competitor.
     - **The standard remedy is known:** an explicit output exception (the GCC runtime-library /
       Bison-output pattern), or an MIT carve-out for the emitted runtime only.
     - **Decide before publishing anything.** The first published page fixes the answer in the wild.
     - **A separable second question the same finding raised: AGPL vs MPL-2.0.** §13 is *not*
       inapplicable here — a `--host` LAN preview is network interaction (`LICENSE:542-548` +
       `SECURITY.md:44-47`) — so the "nobody stands in that hole" framing is wrong. But the tax
       lands on exactly the adopters the project needs. MPL keeps file-level copyleft, passes most
       corporate bans, and ***REMOVED*** `deny.toml` protects.
     - ~~**Either way, the reservation at `README.md:156-158` is fiction the moment one outside PR
       merges without a CLA or DCO.**~~ **DISCHARGED 2026-07-28 by item 89:** `CONTRIBUTING.md`
       clause 3 is the inbound grant (perpetual, worldwide, irrevocable, sublicensable, explicitly
       including **relicensing**), and `gate_script.rs` fails the suite if that grant disappears.
       **This does not touch the rest of item 101**, which is about what a *user's built page* is
       licensed as, not what a contribution is.
102. **Decide what to do about constructs that render elsewhere and silently do not here.**
     (Ruling.) Detail in [adoption friction](2026-07-27-adoption-friction-audit.md).
103. **Clear the name in software classes before the flip.** (Ruling, legal not code.) Trademark
     search in the relevant classes; the name is the retained optionality per the product stance.

71. **Two deck-on-touch behaviours that are working-as-written, and may be working-as-wrong**
    (DT-3 + DT-4, detail: [2026-07-27-deck-touch-audit.md](2026-07-27-deck-touch-audit.md)).
    Neither is a bug; both are a choice someone made that the touch crossing put a number on.
    - **A slow swipe does nothing.** Measured: 200 px in ~30 ms navigates, the same 200 px over
      **750 ms** does not (`deck.js:1859`, `dt > 600`). A swipe's time bound normally separates a
      swipe from a pan/scroll — but in stepped mode there is no competing one-finger gesture to
      separate from (`deck.feed` returns at `:1798`, `deck.overview` at `:1799`, both above it, and
      the stepped stage does not scroll), and the 50 px distance floor already rejects a tap. So in
      the only mode where the bound is live it can *only* reject input the reader meant, and what it
      rejects is the slow deliberate swipe a motor-impaired reader makes. Proposed: drop `dt` in
      stepped mode, keep the distance floor. **No real user has been observed failing on it.**
    - **The share panel says "Point a phone here" — to a phone.** The QR takes most of the card and
      is the one useless half on the device reading it; Copy is the action that works and is
      secondary. Panel geometry is otherwise correct at 390 px (nothing clipped, QR legible).
      `navigator.share` was absent under emulation, **so the Web Share option was not measured and
      is not claimed**.

147. **Retired into item 101** (2026-07-28 merge). The critique round filed this as its item 88,
     "What licence governs a page a user publishes?", independently of Wave 1's item 101 and with
     sharper evidence. Its whole body now lives under **101**; the number is retained, never reused.

148. **Distribution: the binary channel now has a MECHANISM but still has no artifact; the package
    managers are untouched.** **Amended 2026-07-28 by item 92** — read this before re-filing any of
    it. What shipped: `.github/workflows/release.yml` builds Linux x86-64, macOS arm64 and macOS
    x86-64 on a `v*` tag, attaches a tarball + `.sha256` with `LICENSE` + `THIRD_PARTY.md` inside,
    and the README states the matrix (Windows explicitly unsupported). **What is still true:** no
    tag has been cut and the workflow is guarded inert until the repo is public, so `gh release
    list` is still empty and there is still nothing to download **today**; crates.io `taliesin` /
    `taliesin-core` / `taliesin-server` are all still 404 (all three names free); no Homebrew, Nix,
    or install script. The remaining work is therefore **cut a tag after the flip**, then decide
    about crates.io / brew / nix separately.
    - **Cold-build cost re-measured 2026-07-28: 2m11s, 268 crates, 2.6 GB peak RSS at `-j4`**, for
      one ~38 MB binary; the README now states this. The filed **2m59s** was a different machine or
      job count, not a regression. Either way the argument stands: the audience for a documentation
      tool is not the population that will install a Rust toolchain and wait it out, which is
      exactly why the release workflow exists.
    - **Prerequisite the critic missed and the defender found:** `cargo publish` will *reject*
      this workspace as-is. `Cargo.toml:14` declares `taliesin-core = { path = "crates/core" }`
      with **no `version`**; add `version = "0.2.0"` first.
    - Also blank on crates.io without it: no `keywords`, `categories`, `readme`, `homepage` or
      `documentation` in any manifest, so the crate pages would carry one description line and
      nothing else. Watch `crates/core` = 7.3 MiB tracked against the 10 MiB `.crate` cap.

149. **Launch presentation, all gated on the flip.** Grouped because none is actionable until the
    repo is public, and each is small once it is.
    - **The README does not lead with the speed moat**, contradicting this file's own ruling at
      `:577-579`. "Quarto" appears **zero** times in the README and in `site/*.tmd`, and
      `tools/live-edit-bench/RESULTS.md` (cold 123,994.9 µs vs warm 28,425.1 µs, diff 685.6 µs,
      83× smaller payload) is cited from nowhere. Note the ruling says *lead with the moat*; it
      does not say *name Quarto* — that inference is the critic's.
    - **The GitHub repo is a dead first impression**: description defines Taliesin in terms of
      Taliesin, `homepageUrl` empty, one topic ("rust"), zero releases, and the README's only
      image is the licence badge — while four screencasts demonstrating the moat sit committed
      in `site/assets/` and appear on no page a visitor sees. (They are MP4; a GIF conversion or
      an uploaded asset URL is needed, not a one-line embed.)
    - ~~**No platform statement anywhere**~~ **DONE 2026-07-28 (item 92).** The README carries a
      platform matrix naming the three built targets and stating Windows unsupported, and
      `release_targets.rs` pins it against the release workflow in both directions. The underlying
      fact is unchanged and still worth knowing: `/proc` is read directly in five places with
      `#[cfg(not(unix))]` fallbacks that `LESSONS.md:88` records as never executed by any test.
    - **CoC and issue templates only** — ~~CONTRIBUTING / CLA or DCO~~ **DONE 2026-07-28 (item
      89).** `CONTRIBUTING.md` exists and its clause 3 is the inbound grant, explicitly including
      relicensing, so `README.md:156-158` is no longer ended by the first merged outside PR;
      `gate_script.rs` fails the suite if that grant disappears. Still absent: a code of conduct and
      GitHub issue templates, both of which are only worth doing once the repo is public.
    - **`taliesin.dev` resolves to nothing** (registered, NS + SPF + a google-site-verification
      TXT, zero web records) and is baked into every canonical URL, `og:url`, sitemap and feed.
      `site/README.md:11-12` already flags it as a placeholder.
    - **`taliesin build site` 404s its own primary CTA** — `docs/guide/` and five `gallery/*`
      mounts are preview-only, and `site/README.md` documents an 8-command build that nothing
      runs. Worse than filed: `--strict` exits **0** on it and `check` says "no problems found",
      so both automated gates bless a deploy the tool has already warned about. A `site/build.sh`
      is the cheap fix; counting mount warnings as `--strict` problems is the durable one.
    - **The name** (surfaced, not a task): TALIESIN is a live registered mark of the Frank Lloyd
      Wright Foundation (Reg. 4150375). Software is outside the recited goods so legal risk is
      low; the cost is permanent SEO invisibility, and `github.com/taliesin` + `/taliesins` are
      both taken. Renaming twice is worse than a bad search name — if keeping it, always publish
      as "Taliesin — the `.tmd` dev server" so the disambiguator travels.

25. **Pre-public release: the flip procedure, and a contradiction to resolve first** (detail:
    [2026-07-17-security-release-audit.md](2026-07-17-security-release-audit.md) and
    [2026-07-28-launch-critique.md](2026-07-28-launch-critique.md)). All five code items shipped
    2026-07-25. **oss-4 was ruled 2026-07-25: deferred** ("I'll do it at the end of summer").

    **Author leaning, 2026-07-28 (a leaning, NOT a ruling — re-confirm before acting):** do a
    **visibility flip** with the sensitive documents removed, deliberately keeping the commit
    history public so readers can see how the work was done.

    **The one fact that decides whether that plan works.** A visibility flip exposes **every past
    commit**, and `git rm` in a new commit does not remove a file from history. Two documents are
    tracked and both instruct otherwise in their own headers —
    `notes/STARTUP-PLAN.md:3-5` ("keeping it out of any public release") and
    `notes/FUNDING-RESEARCH.md:4` ("keep this file out of git") — and they contain the ***REMOVED***
    ***REMOVED***, and "MIT
    would let a competitor or a cloud provider close it against you. ***REMOVED***."
    So **"flip + delete the files" leaves them fully readable in history**, which is the opposite
    of the intent. Only three options actually work:
    - **(a) Flip, and rewrite history first** (`git filter-repo` over those paths). Keeps the
      visible history the author wants, at the cost of rewriting every SHA — and any SHA recorded
      in `notes/` or in a findings doc stops resolving.
    - **(b) Fresh public repo** per `notes/STARTUP-PLAN.md:111-127`, which is a *dated ruling*
      ("decided 2026-06-18") prescribing exactly this: `rsync -a --exclude='.git'`, remove the
      private docs, "Keep this repo private forever; the public one is a separate repo." Clean,
      but **discards the commit history**, which is the thing the author said they wanted to keep.
    - **(c) Flip as-is and accept the exposure.** Cheapest, and the least consistent with having
      written "keep this out of git" twice.

    **Note the procedure collision, because two committed documents currently disagree:**
    `***REMOVED*** (fresh repo), while this file and
    `2026-07-17-security-release-audit.md:217-218` both sequence the `oss-*` items to "whenever
    the repo actually flips public". Whichever option is chosen, **fix the losing document in the
    same change** or the next session will follow the wrong one.

    Still open under whichever route: whether to prune `notes/` + `docs/superpowers/`. The
    deferral's stated reason — "no secret is exposed … but it is a curated bug roadmap" —
    describes the audit notes and **does not describe the two files above**, which is why they
    were never named in it. Scale, measured: `git ls-files notes/` = 63, `docs/superpowers` = 69,
    and the largest is `2026-07-03-quarto-design-decisions-catalog.md` at **1,129,387 bytes** of
    adversarial self-critique sitting under `docs/`, which a visitor reads as "the manual".

    **Correction to this item's own former text.** It claimed "**Verified NOT open, do not
    re-scope:** … the tracked `/home/bogo` paths are scrubbed." Measured 2026-07-28:
    `git grep -Il "/home/bogo"` → **11 files**. The 2026-07-17 scrub was scoped to the four paths
    under `docs/superpowers/*` and did do that; the summary generalised it to "the tracked paths",
    and one new occurrence has since accreted (`2026-07-18-shell-completion-dynamic-design.md:189`,
    dated the day after). Eight of the remaining ten are `notes/*` prose covered by the prune
    above, and two are self-references *documenting the scrub*. Low impact — the username is
    already public via git author metadata — but **a "verified NOT open" line in this file was
    measurably false**, which is the failure mode `LESSONS.md` warns about. Still correctly
    closed: `SECURITY.md` exists, PT-1 / PT-2 / NET-1 / OUT-1 / DEP-01 / DEP-02 all shipped
    2026-07-17, and `dos-yaml` + NET-3 were refuted.

### D. Blocked on a device, a real user, or working-as-intended

Kept visible so they are not re-scoped. Revive on a real signal, not on capacity.

104. **Three Wave 1 items whose own round could not verify them, filed with the measurement each
     needs.** (Do not build until measured — each says so in its findings doc.)
     - **The `.gitattributes` line that makes `.tmd` behave like `.md`** on GitHub. Needs GitHub
       linguist-override behaviour confirmed; the round could not.
     - **The Jupyter on-ramp that already exists outside the project.** Needs `nbconvert` output
       confirmed to survive the rename.
     - **The scale ceiling**, measured with a **runtime-generated fixture that never enters the
       corpus walker** — deliberately shaped to respect the standing ban on growing `corpus/tarn`
       and on minting `corpus/longbook`, whose stated reason is that the walker renders every
       corpus doc on every `cargo test`.
105. **The headless `--no-sandbox` rationale rests on an assumption this round retired.** (LOW.)
     The justification assumed only author-written documents reach the headless path; item 79's
     family says otherwise. Re-derive the rationale before changing the flag.

78. **The figure recolour has no notion of "text sitting on a data fill", so it can *cause* the
    contrast failure it exists to prevent** (P3, filed 2026-07-27 while fixing item 77's fourth
    residual; item 41's family). `MPL_THEME_PREAMBLE`'s `_tali_recolour` sets **every** `Text` in a
    figure to the reader's foreground. That is right for titles, axis labels and ticks, which sit on
    the transparent page background — and wrong for an annotation drawn *inside* a data-coloured
    mark, whose background does not change with the theme. **Measured** on
    `corpus/tech-blog/posts/pca-geometry/`'s covariance heatmap: the `1.00` cells are near-black
    `#67000d`, so in the **light** render the annotation is recoloured to near-black `#1a1a1a` on
    near-black and is effectively illegible; the dark render is fine. The author cannot fix it in the
    document — an explicit `color=` on the annotation is exactly what the recolour overrides, which
    is what makes this a tool item and not a corpus one.
    **Not obvious how to fix, which is why it is filed rather than done.** Matplotlib does not mark
    which `Text` is "on" a mark, so candidates are all heuristics: skip a `Text` whose axes-fraction
    position lands inside a filled artist; skip `Text` parented to a `QuadMesh`/`AxesImage`; or pick
    per-annotation black/white from the *underlying* fill's luminance instead of the page
    foreground (what matplotlib's own `annotate` helpers do). **Do NOT "fix" it by dropping the
    recolour** — that reinstates the baked-foreground bug the preamble exists for.

4. **Deck engine mobile polish** (P2): mobile pinch/pan + touch gestures (they matter for the
   phone-feed deck mode); drop `fitSlide` from the resize path (needs a lazy fit-on-show refactor
   first). *(The desktop trackpad half shipped 2026-07-24 — pinch / ctrl+wheel-down opens the overview
   map, with a 250 ms hysteresis.)* **The device blocker is gone.** **Partly measured 2026-07-27**
   (deck × touch round): with synthetic touch events, swipe navigation works (h 0→1→0), a two-finger
   pinch-in opens the overview, and an overview one-finger pan neither navigates nor exits (B6-31
   holds). **What is still unmeasured is the part emulation cannot reach**: a real finger, and
   overview pan while zoomed *past* fit — at fit scale `clampOv` has nothing to pan, so the probe
   proved only that pan does not misfire, not that panning works. Chromium touch emulation is still
   not evidence for a pinch on glass.

10. **Two kernel limitations with no clean fix** (P3, dev-facing):
    - **R cold kernels still orphan on ungraceful parent death.** IRkernel has no `ParentPollerUnix`
      equivalent, so there is nothing to arm; PDEATHSIG is the only other lever and is hazardous. R is
      rarely the cold single-doc path, and the warm-pool, cold-Python and `/tmp`-sweep halves all
      landed. `kernel.rs`.
    - **A tens-of-MB cell output blocks ZMQ receive before the cap fires.** `kernel.rs`. (Not
      forbidden — the old "do-not-touch" note was the completed rewrite-scoping list, not a freeze.)

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
    - **F-03 (WAI, authoring nuance):** a `{js}` "once" cell's returned node is mounted *after* the
      cell body runs, so an attachment-gated init (`if (!node.isConnected) return`) silently no-ops the
      first paint. Gate teardown on `invalidation`, not DOM attachment. Candidate: a doc line in the
      `{js}`-cell reference, or an optional post-mount hook.

41. **R graphics cannot follow the page theme; matplotlib figures can** (P3, M; detail:
    [2026-07-26-corpus-demand-probe-analyst.md](2026-07-26-corpus-demand-probe-analyst.md), AN-2b).
    Taliesin renders every inline matplotlib figure **twice** (light + dark foreground) and swaps them
    on the theme toggle (`kernel.rs`'s `MPL_THEME_PREAMBLE`); measured on `corpus/analyst/` the Python
    figure emits two genuinely different PNGs and the ggplot figure emits one, so a mixed-language
    report has half its figures track the reader's theme and half baked. **Blocked on being a feature,
    not a fix:** a real version re-renders the figure twice against two foregrounds. **Do NOT confuse
    this with AN-2a, which is fixed** — the R device no longer paints opaque white under a transparent
    figure; the *ink* is still baked at one colour, and that is what is left. The documented workaround
    (a neutral mid-grey palette) is the second instance of the convention named in item 18's F-02.
    Minor and separable: an R figure is emitted `<img alt="output">` where the Python pair is `alt=""`;
    both sit inside a captioned `<figure>`, so `alt=""` is right and `"output"` is noise read aloud.

70. **A project with no `_site.yml` declares no boundary** (P3, filed 2026-07-27 from the path-parity
    batch's "surfaced, not fixed"). `build <dir>` accepts a bare directory, so a single-document render
    of one of its pages roots at that page, and the site path's own inference can still widen to
    `.git`. Nothing can infer an undeclared boundary; the fix is for the author to declare one. Live
    instance: `corpus/posts/pca-geometry/` (the loose twin of the tech-blog page, byte-identical to it
    and pinned so by `twinned_corpus_sources_stay_byte_identical`) sits under no project marker, so
    `build` of it warns `include not resolved` — true since PT-2 shipped and **now uncovered by any
    test**, since the corpus pin moved to the tech-blog copy. Decide whether that warning is correct
    behaviour or wants a better message before writing code.

### E. Gated, not actionable now (do not spin up)

- **M6a `MAX_WARM_PAGES` / `exec_pool.rs` eviction:** the standing freeze; sign-off refused
  2026-07-17. Eviction drops the executor and kills its kernel child processes, so this is kernel
  lifecycle, not a constant. Do not tune without a new ruling.
- **M2's hanging-interpreter sibling** *(needs its own exec/kernel ruling)*: a *hanging* (not missing)
  interpreter costs ~161s recovery, downstream of the (bounded) `interp_id` probe in the warm-pool
  forkserver READY wait + kernel-start retries.
  `kernel::tests::transient_start_errors_retry_but_missing_interpreter_does_not` shows the *missing*
  case is handled and the *hanging* one is not. `kernel.rs`/`warm_pool.rs`. *(Aside, pre-existing +
  load-bearing: `crates/server/Cargo.toml` doesn't list tokio's `process` feature though
  `kernel.rs`/`warm_pool.rs`/`exec.rs` use it; it compiles only via feature unification.)*
- **M4 test stand-in flake:** the M4 test's `sleep 300` stand-in kernel survives ~2 of 8 full-suite
  runs, only when the build is cold. Measured, unexplained, argued test-only (a real kernel has three
  reclaim nets where the stand-in has one). Worth an hour only if a real kernel is ever seen outliving
  its pool.
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

- **2026-07-28 the honesty + build-cost batch (items 91, 110, 115, 119, 126, 134, 143),** on
  branch `backlog-batch-2026-07-28`. **Do not re-scope any of the following as open:**
  - **`chromiumoxide` is an opt-in `headless-js` feature** (91). The premise re-measured
    properly: it is only 12 of 268 crates, but `chromiumoxide` + `chromiumoxide_cdp` are the
    two most expensive units in the whole graph — **81 of 336 CPU-seconds, and 2m 30s → 1m 39s
    wall, 268 → 252 crates, a 44 MB → 32 MB binary** (one machine, `-j3`, clean both times).
    Off by default; `gates.sh`, `ci.yml` and `release.yml` all pass it, and
    `crates/core/tests/headless_js_feature.rs` fails if any of the three stops (3 of its 4
    assertions mutation-killed; the 4th is unreachable because that mutation stops cargo from
    parsing the manifest at all, which is recorded in the test). Without the feature
    `read --run-js` reports `skipped` naming the rebuild — verified on a real default-feature
    binary, not inferred.
  - **Not linting `draft:` pages is RULED CORRECT** (110); the defect was the silence.
    `check` now prints what it held back, and `discovery.rs` carries the ruling so it is not
    "fixed" by linting them. **First design was wrong and measurement caught it:** a second
    `Site::discover` to learn the drafts cost **+50 to +83 ms** (~20% of a check), not the
    "<10 ms" the comment claimed, so the fact is threaded out of the discovery that already
    runs and is now free.
  - **`Block::sourcepos` documents the empty-string contract** (115). Ten producers write
    `String::new()` on purpose; the client's `usableSourcepos` gate is what makes that safe,
    and inventing a plausible range silently sends the editor to line 1.
  - **`check`'s link scope needed no output change** (134) — `cli.tmd:40` already documented
    it accurately, including the above-root case. The item had rotted. What was genuinely
    undocumented was the *draft* scope, now stated in two reference pages.
  - **The docs-vs-behaviour sweep is finished** (143). ~25 claims re-derived from source and
    fixed. **Three of its own filed claims were false and were NOT "fixed":**
    `TALIESIN_MERMAID_URL` is honoured in *both* modes (not preview-only); `build --strict`
    *does* fail on `_site.yml` problems (`build.rs:1717`); and `SiteApp` exists. Genuinely
    fixed: 9 Mermaid-as-CDN claims (it is vendored — inlined in a build, same-origin in
    preview), the `format:` sub-key exemption (linted since the extension mechanism was
    found not to exist — including in `frontmatter.rs`'s own comment), `PageIncludes.resources`
    /`has_markup()`/`copy_resources` (none exist), the protocol's message count (**twelve**,
    documented as nine while listing ten — `build-state` and `cell-state` were missing), the
    loopback-Origin allowance (**`--host` drops it**), the diff mask (`data-sourcepos` is the
    *only* thing masked — `data-source-file` must match exactly), 9 stale "book sidebar"
    instances, `init`'s file count (**five** places said two; it writes 3/4/5 + `.taliesin/`),
    and ROADMAP's normative guardrails. **Left alone on purpose:** `ROADMAP.md:289`'s "mermaid
    is the sole CDN dep" and the other `qmd` tokens there sit inside dated `[x] DONE` records
    that correctly describe the past.
  - **The ACR is published** (126) at `docs/guide/reference/accessibility.tmd`, linked from
    the README. **Re-derived before publishing:** the draft's 2.5.3 "Does not support" row was
    stale — item 124 shipped, the `<kbd>` carries `aria-hidden`, so it is "Supports". The
    not-evaluated list is as long as the table on purpose.
  - **`notes/DETECTION-DEBT.md` is the live register** (119), pointed at from AUDITS.md and
    this file's standing constraints. **Re-derived, not copied:** most of R7's D≥8 rows had
    since been fixed. Only **three** remain at D≥8, and the top one is `MAX_WARM_PAGES` /
    `exec_pool.rs` at **D=10 with zero test references** (re-measured) — the standing freeze
    forbids *tuning* it, not *pinning* it.

- **2026-07-28 the block-model + docs-gate batch (items 138, 146, and the module-path half
  of 143),** on branch `block-single-root-2026-07-28`. Both fixes mutation-verified, the
  client-side one additionally reproduced and re-measured in a real browser.
  **Do not re-scope any of the following as open:**
  - **Every block now has exactly one root element** (138). `emit_html_block` injected the
    block's data attrs into the *leading* start tag, so a raw HTML literal that comrak folded
    into one block (three `{{< input >}}` controls on consecutive lines, as `corpus/descent`
    ships) put the id on root 1 of N, and the client — which mounts with
    `firstElementChild` — half-applied every op. Such a literal is now wrapped in a single
    `<div>` carrying the attrs. Fixed server-side, where `site/backlinks.rs` already asserts
    the same invariant for its own emitter.
  - **The filed blast radius was wrong in the direction that matters, and re-measuring is
    what found it.** Filed as 2 pages / 6 orphan roots; across all 161 corpus + docs
    documents there is exactly **one** id-carrying block that is not single-root. The second
    page named (`corpus/diagnostics/a11y.tmd`) is clean — its raw HTML is a lone `<img>` plus
    inline tags inside paragraphs. The sweep did turn up a separate **benign** class: 21
    blocks with no `data-block-id` at all (20 comment-only, one stray `</div>`), which nothing
    in the DOM claims and no op targets. Those are deliberately left alone — wrapping them
    would add an empty div to a page to hold content that renders nothing — and the pin
    fixes that class to exactly the comment/closing-tag shapes, so real content cannot
    quietly become unaddressable.
  - **The repro needs an edit to a LATER root, which is why this survived so long.** Editing
    the *first* control looks correct even with the bug: the untouched later roots are already
    in the DOM and unchanged. Measured in a browser against a build with the fix disabled,
    editing the *third* control leaves the source at `max="99"` and the live DOM at `max="60"`
    with the id swapped. A one-root-per-block probe is the only thing that sees this class;
    a spot check does not.
  - **Prose is gated against the tree now, not against a needle list** (146, all three
    candidates). `stale_docs.rs` gains: no shipped doc may name a source file that does not
    exist (183 path claims examined, resolution accepting suffix and `src`-elided forms so
    `server/exec.rs` and `tests/regression.rs` stay legal); no shipped doc may show a retired
    front-matter key, with the key list parsed out of `RETIRED_KEYS` so retiring another key
    extends the gate for free; every flag the CLI reference documents must exist in the CLI
    (80 mentions). Each mutation-verified against its own shape, each with a floor on how much
    it examined — an extractor that stops matching is a gate that passes forever.
  - **`notes/` and `docs/superpowers/` are excluded from that gate and must stay excluded.**
    They are dated records, and **five of the six** paths the first version reported were of
    exactly that kind (a 2026-06 spec correctly describing the tree as it was). One exemption
    exists, for prose whose *subject* is a dead path, and it asserts the sentence is still
    there so a rewrite must delete it rather than leave it shadowing the next defect.
  - **The retired-key gate closes a real hole:** `check` lints the corpus and the books' own
    front matter, never a YAML block quoted in prose — so the one surface where a reader
    learns the vocabulary was the one surface nothing checked.
  - **Nine stale module paths fixed** (143's path half), all four "file became a module
    directory" (`serve.rs`, `serve_site.rs`, `cite.rs`, `diagnostics.rs`), plus
    `code-enhance.js` → `code-enhance/` and a test table's `extensions.rs` → `theme_css.rs`.
    Four of them sat inside **mermaid diagram labels**, which are parsed client-side, so both
    books' diagrams were re-rendered in a browser (6 of 6 draw). The LAN-token sentence also
    moved from `serve/mod.rs` to `serve/security.rs`, where those functions actually live —
    a correction no path gate can make, since both spellings resolve.
- **2026-07-28 the deck-harness batch (items 112, 125, 113, 111, plus two defects the
  harness found),** on branch `deck-harness-2026-07-28`. Each pin verified by **mutation
  against `deck.js` / `deck.css` itself** (restore the bug, watch the named test fail), not
  by a green suite. **Do not re-scope any of the following as open:**
  - **`deck.js` has a browser test now** (112). `crates/server/tests/deck_browser.rs` builds
    `corpus/deck.tmd`, opens it in headless Chrome at **1280×900 — landscape is
    load-bearing, a portrait window opens the phone feed instead of the stepped deck** — and
    walks it with **real** `ArrowRight` events (CDP `Input.dispatchKeyEvent`, not a synthetic
    `KeyboardEvent`; that is the Cmd-K lesson). Two properties: at every step the `#/slug`
    resolves, *the way a reader's browser resolves it*, to the slide the reader is on; and
    the address captured mid-walk **re-opens in a fresh page onto the same slide and the same
    fragment step**. The round-trip is the half that carries the weight — mutating
    `writeHash` to emit the index instead of the id fails both, but mutating `readHash` to
    land on slide 0 fails **only** the round-trip, which is what proves it is not decoration.
    Also asserted, because the walk is worthless if it did not move: it reaches the last
    slide, `h` never goes backwards, and it steps *into* the vertical stack (`v > 0`).
  - **Deck content is auditable, and the claim is now made** (125). The same walk asserts
    both halves: at rest **1 of 21** slides is exposed (every off-camera slide is `inert`,
    which is the *correct* implementation and is pinned in that direction — a mutation that
    stops setting `inert` fails it), and the union over the stepped walk is **all 21**, each
    scanned against the two `validate_a11y` rules a live DOM can check. **0 violations across
    100% of slides**, where a page-load audit covered one slide. The scan carries its own
    vacuity control (it asserts it *examined* elements), because a selector that matches
    nothing reports clean forever.
  - **Two shipped layout defects the harness found on its FIRST run**, neither of them a
    filed item, both invisible to every emission test because both are about *rendered
    geometry*. This is the item-112 argument paying off immediately:
    - **Code blocks were clipped off the right edge of 5 of the 21 slides.**
      `.tali-deck pre` set `width: 100%` with `padding: .8em 1em` + a `1px` border while
      the global reset is `content-box`, so every code block computed ~32px wider than the
      slide's content box, ran into `.tali-deck { overflow: hidden }` and took the copy
      button with it. It also made `fitSlide` shrink every code slide to fit an overflow
      that should not have existed, so slide code rendered **smaller than the design calls
      for**. One line (`box-sizing: border-box`), pinned by
      `no_slide_content_is_clipped_by_the_slide_edge`. `.tali-slide-bg` is exempt in the
      probe and must stay exempt: a per-slide backdrop is full-bleed on purpose.
    - **The browser's focus ring painted around every slide in a vertical stack.**
      `deck.js` moves focus to the slide that becomes current (correct — the previous one
      goes `inert`), and `deck.css` suppressed the ring with
      `.tali-slides > section:focus-visible`. **A child combinator**, and a vertical
      sub-slide is a grandchild inside `.tali-stack`, so Chrome's `outline: auto 1px`
      painted a light rectangle on a projected deck from the first key press that entered
      a stack. Fixed to a descendant selector, pinned by
      `the_focused_slide_never_shows_the_browsers_focus_ring` — which is why the walk
      asserts it steps *into* the stack: on top-level slides the property already held.
    - **The probe was wrong before it was right, and the trap generalises.** The first
      overflow probe mixed `getBoundingClientRect` (rendered px) with `getComputedStyle`
      padding (unscaled CSS px). The stage is a **scaled camera**, so that invents or hides
      `padding × (scale − 1)` — it reported a phantom 7px on `<h2>`/`<p>` across most
      slides, which reconciles exactly against the harness's measured `@0.833` scale
      (`40 × 0.167 = 6.7`). **Any geometry assertion inside a scaled stage must convert;**
      the failure message carries the slide's `WxH @scale` so the next false positive is
      diagnosable instead of mysterious.
  - **The corpus deck gained math and a kernel cell** (113). Both existed only in the
    dogfood decks, which `corpus.rs` does not walk. The `{python}` pin is on the **block
    model**, not on emitted HTML: an unexecuted `{python}` cell and a plain ```python fence
    render to the same `<pre><code class="language-python">`, so an HTML needle would pass on
    a deck with no runnable cell at all. The other eleven shapes 113 listed stay unbuilt.
  - **Gates, measured in an ISOLATED WORKTREE, and that detail is the point.** A parallel
    session was writing code (not just findings) in the shared tree — `editor/vscode/`
    largely rewritten with ~2,100 deletions **staged in the index**, plus `lsp.rs`,
    `lsp_complete.rs`, `vocab.rs` and two new files — so a suite run in the main tree
    certifies *their* work and mine together and neither alone. Two runs there came back
    green at **1,728** and then **1,762** tests, the drift being their tests appearing
    mid-batch, and `cargo fmt --check` **failed on their file**. Re-run on a detached
    worktree at this batch's commit: **106 binaries, 1,730 passed, 0 failed, 0 ignored**,
    zero skip lines, all four interpreter canaries `... ok`, with `--test-threads=1` and all
    four `TALIESIN_REQUIRE_*` armed. **1,730 reconciles exactly** (1,725 baseline + 1 test
    binary + 5 tests). `cargo fmt --check`, `clippy --workspace --all-targets -D warnings`,
    both `tsc` gates and `node --test` all exit 0 **in that worktree**; `check` exit 0 on
    all 16 projects and `cargo audit` / `cargo deny check` exit 0 (main tree, and no
    dependency changed). **Not run: the VS Code companion gate** — it `npm ci`s the exact
    directory the other session was working in, and nothing in this batch touches `editor/`.
    **Lesson for the next batch: commit your own paths explicitly and certify off a
    worktree whenever another session shares the tree.**
  - **A vacuous row stopped propping up a floor** (111). `a11y_outline.rs`'s book walk
    counted `demo.tmd` and `tour.tmd` toward `pages >= 40` while `a11y.rs` exempts a deck
    from the heading-skip rule wholesale, so two of the rows proving "the walk was live" were
    empty **by construction**. Decks are now counted separately and the floor applies only to
    pages the rule can fire on. **Deliberately NOT asserted: that a deck emits no skip.**
    Measured — re-running the rule over those same blocks as `DocFormat::Html` (i.e. with the
    exemption gone) also reports **0** on both files, so such an assertion would pass for two
    independent reasons and could never fail. Replacing one vacuous row with another was the
    way to get this item wrong.
- **2026-07-28 MERGED-TREE figure, which is the one to trust** — the verified sweep and the
  critique-round batch below were built and gated **together** on
  `worktree-verified-sweep-2026-07-28` after merging `main`: `./tools/gates.sh` **PASSED, all
  nine, exit 0**; workspace suite with all four interpreter gates and `--test-threads=1` =
  **105 binaries, 1,725 passed, 0 failed, 0 ignored**, zero skip messages; `check` exit 0 on all
  16 projects. **1,725 reconciles exactly** — 1,709 baseline plus 8 test functions from each
  branch — so the merge lost nothing and introduced no regression. Both branches independently
  measured **1,717**, which is not a contradiction and is worth knowing before someone "corrects"
  one of them: each added 8 tests to the same baseline, so the same total twice was arithmetic,
  not a copied number. Only `notes/backlog.md` conflicted; every source file auto-merged.
- **2026-07-28 the verified sweep (items 85, 86, 97, 98, 99, 114, 123, 130),** each pinned by a
  test and verified by mutation. **Do not re-scope any of the following as open:**
  - **A `theme:` extension bundle is contained** (85). The `_extensions/<name>/theme.css` arm read
    `base.join(ext)` with no containment while the sibling `.css` arm went through `safe_join_in`;
    both now go through `try_join_in`, which **keeps the refusal reason**, so a theme that was
    refused is no longer reported as "not found" and the author is not sent hunting a typo that
    is not there. Two shapes escaped and both are pinned: a `../` climb, and an **absolute** name
    — `Path::join` *replaces* the base on an absolute argument, so `theme: /etc` read
    `/etc/theme.css` outright. **That is item 80's `mounts:` footgun in a second place**, which is
    the reusable part. A bare unknown name (`darkly`) is still silent, deliberately: it may be a
    legacy built-in, and turning every miss into a warning was the way to get this wrong.
  - **No built page fetches anything off-origin** (86). `no_built_page_fetches_anything_off_origin`
    walks every corpus doc in **both** shipping modes (`Build` *and* `Preview`, the larger surface)
    and scans per element, not by substring — an `<a href>`, a `rel=canonical`, `og:url`, JSON-LD's
    body and an SVG `xmlns="http://www.w3.org/2000/svg"` are all legitimately absolute, and a
    whole-page `http` grep flags all five. **Its boundary is written into the test and must stay
    honest: it reads STATIC references only** (markup attributes, CSS `url(`/`@import`), so a URL
    that inlined JS fetches at runtime is invisible — which is exactly where `MERMAID_DEFAULT`
    lives as a deliberate never-reached fallback. Measured both ways: reinstating the OFF-2
    fallback does **not** fail it, pointing one emitted `<link rel=icon>` at a CDN does.
  - **A shortcode source is a path, not a URL** (97). `{{< embed >}}` and `{{< video >}}` both
    document their positional argument as a file *relative to the page*, and an embed target is
    additionally **built** as a local file — yet a scheme-bearing token went straight into
    `<iframe src>` / `<video src>`, and `check`'s missing-local-media diagnostic cannot see one.
    A refused `src` leaves the shortcode unexpanded (the existing "keep verbatim" path, so the URL
    survives as inert text and never becomes an element); a refused `dark=`/`poster=`/`captions=`
    is **dropped and the clip still plays**, matching how a typo'd option already degrades. Also
    filtered in `embed_targets`, or `build` would be handed a target it can only fail on. **Do not
    widen it into a sanitizer** — raw HTML still passes through (2026-07-03 CSP ruling), and
    `caption=` is prose, so a colon in a sentence is not a scheme. A `C:/…` drive path and a
    `clip.mp4?v=2` query string are paths and are pinned as such.
  - **Both `jsconfig.json` include lists are globbed** (98). The hand-written lists rotted in
    **both** directions and tsc reports neither: a new file was silently unchecked, and
    `code-enhance/03-focus-mode.js` was still listed months after that file was deleted. Measured
    identical before and after (**5** and **25** project files, minified bundles still excluded by
    shape via `exclude: ["*.min.js"]` rather than by name), then verified by dropping a
    deliberately ill-typed file into each directory and watching both gates fail — which the old
    lists would not have done.
  - **The eviction log reports only kernels that existed** (114). `Executor::has_live_kernel()`
    gates the line; a page that never ran a cell is silent. **The eviction ORDER and the cap were
    not touched** — that is the standing freeze — only what is said about an eviction. The
    decision is a pure function (`eviction_line`) so it is testable without capturing stderr,
    which `crate::log::kernel` writes to directly.
  - **`init` says what it wrote that you did not ask for** (123). The item's own text was already
    half-stale: `AGENTS.md` *is* listed, as a bare path among five. What was missing is that it is
    5,049 bytes of unexplained file, so a one-line note now names it and `.taliesin/` and says both
    can be deleted. `the_onramp_note_names_every_file_init_writes_unasked` derives the list from
    `onramp_files()`, so a fourth onramp file fails the suite until the note mentions it. **Not a
    flag** — an `--onramp` knob would be a configuration answer to a documentation problem.
  - **The `qhl-` prefix (130) was ALREADY FIXED when this was picked up**, and nothing said so.
    `CLAUDE.md` says `tali-hl-`, the emitter's prefix is pinned by
    `crates/core/tests/highlight_langs.rs`, and the vacuous `contains("qhl-") ||
    contains("language-python")` disjunct in `render/tests.rs` had already been repaired. Only a
    stale present-tense sentence in `LESSONS.md` was left, and it is corrected. **Same shape as
    item 124 four days earlier: grep the named symbol before building the named fix.**
  - **The Chrome gate is not vacuous** (99), measured rather than argued, and it needed no code.
    Armed with a browser present: **3 passed, no skip line**, and `read --run` reports
    `svg 320×200` plus the page's real thrown `Error: intentional read --run test failure` —
    neither is producible without a browser having executed the page. Armed with
    `CHROME_PATH=/nonexistent`: **2 hard failures** naming the gate, not a silent skip. Both
    directions hold, so `CANARY_CHROME` printing `... ok` means the browser really ran.
- **2026-07-28 the critique-round client/LSP/manifest batch (items 139, 140, 141, 142)** on branch
  `critique-fixes-139-142`. Each defect was reproduced FIRST as a failing test and each fix is
  mutation-verified by that test having been red against the unfixed code. **Do not re-scope any of
  the following as open:**
  - **`textDocument/rename` validates the new name and refuses with a `ResponseError`** (139).
    `lsp_nav::anchor_name_error` is the single grammar, living next to the scanner that enforces
    it, and it checks TWO things: every char is an xref-id char (`my section` wrote
    `{#my section}` and rewrote every reference to match), and the kind prefix survives (renaming
    `sec-a` to `intro` would leave every `@intro` as prose, since `anchor_at` only recognises a
    known prefix). Refusal is JSON-RPC **RequestFailed (-32803)**, which the editor shows in its
    rename box; a null result reads as "nothing to rename". `resolve_prepare_rename` untouched.
  - **A rename no longer rewrites the `#fragment` of an external URL** (139). `is_anchor_site`'s
    `'#' => true` arm now requires the sigil to open a `{#id}` attribute or a bare in-document
    link destination `](#id)`. Both in-document forms still move; `[x](https://example.com/p.html#sec-a)`
    does not. **The mutation campaign's 29 mutants / 0 survivors here proved the implemented rule
    was faithfully pinned, not that it was right** — the missing piece was a fixture with an
    outbound link, which `rename_leaves_the_fragment_of_an_external_url_alone` now is.
  - **`toc_html` stopped double-escaping an explicit heading id** (141). `## R&D notes {#r&d-notes}`
    emitted `href="#r&amp;amp;d-notes"` against an anchor of `r&amp;d-notes` — **a dead link in the
    published build**, verified fixed by building the fixture and reading the emitted pair. The id
    reaches `toc_html` via `extract_attr` out of already-escaped HTML, exactly like the entry text
    three lines above whose comment already warned about this; it now uses the same
    `escape_attr_from_html`.
  - **`buildToc` builds DOM nodes instead of an HTML string** (141). It was the one place the
    client re-serialized DOM text into markup. Browser-verified in preview: all 5 TOC links
    resolve, including `#r&d-notes`, and the nesting is now `li > ul` rather than the parser-repaired
    `ul > ul` — so preview and build agree on a level-skipping page, which they did not before.
  - **The Cmd-K palette locks the background scroller** (142). Browser-verified with a REAL
    `PageDown`, both directions: 0 px moved with the palette open, **738 px with it closed**, so the
    probe is not vacuous. It restores the **saved** root `overflow`, not `''` — separately verified
    by staging a book-drawer lock, opening and closing the palette with real keys, and confirming
    the palette actually closed *and* the drawer's lock survived. (A first attempt at that check
    used synthetic `KeyboardEvent`s, which did not close the palette and made the assertion
    vacuous; the screenshot is what caught it.)
  - **The web manifest stops shipping Taliesin's brand and stops pointing at a 404** (140).
    `favicon:` is promoted to the app icon when the project supplied no `icon-192/512` pair, so an
    author who already declared a mark no longer installs *Taliesin's* logo onto their readers'
    home screens; an SVG gets `sizes:"any"` (true for a vector), a PNG gets its **real** size read
    from the IHDR chunk, and an unreadable size is **omitted rather than invented**. A remote,
    absolute, missing or `.ico` favicon falls back to the bundled set. `start_url` is `./` only
    when an `index.html` exists, else the first page — `display: standalone` removes the address
    bar, so cold-launching into a 404 had no way out. `build` now gates the bundled PNGs on
    `Icons::ships_bundled()`, so they are not written next to a manifest that never cites them.
    **Partially closed, deliberately:** the splash colour is still a single light value, because a
    manifest cannot express an OS-conditional colour and `SiteConfig` has no theme key to read
    (theme is per-document front matter). What *was* fixed is the item's other half — the pin
    asserted the wrong invariant, and `manifest_bg_tracks_the_theme_bootstrap_fallback` now
    asserts the splash tracks the bootstrap's own `BG` fallback rather than a CSS token that
    merely agrees with it. **A dark-mode phone still sees one white splash frame; the address bar
    is unaffected** (the bootstrap owns `<meta name="theme-color">`). Do not re-file that frame as
    a bug without a mechanism that does not exist in the format.
  - **Gates: `./tools/gates.sh`, all nine, PASSED (exit 0)** — workspace suite with all four
    interpreter gates and `--test-threads=1` at **105 binaries, 1,717 passed, 0 failed, 0 ignored**,
    plus both `tsc` gates, `node --test`, the VS Code grammar test, `cargo audit` and `cargo deny`.
    **The +17 against the 1,700 recorded for the publication-readiness batch does NOT reconcile to
    this branch**, which adds exactly 8 test functions and removes none: the recorded baseline had
    already rotted by 9. Measure, do not reconcile against a number in this file.

- **2026-07-28 the reader-cost batch (items 150, 137, 124),** each pinned by a test and verified
  by mutation, and browser-verified because every claim in it is about what a browser fetches.
  **Do not re-scope any of the following as open:**
  - **A site build ships the body typeface as files, not as base64 in the render-blocking sheet**
    (150). `app.css` **229,778 B raw / 137,412 B gzipped → 66,358 / 11,943** — a **125 KB gzipped
    saving on the critical path of every page**, measured on a built `docs/guide`. `deck.css` had
    a **second** copy of the same 160 KB and lost it too (199,654 → 36,234). The faces are
    content-hashed `_assets/*.woff2`; `app.css`/`deck.css` reference them as **siblings** (a
    `url()` resolves against the *stylesheet*, so a bare filename is right at every page depth and
    a `../` climb there would be a bug visible only on nested pages), while the page's
    `<link rel=preload as=font … crossorigin>` is page-relative and *does* climb.
    **Per-target, deliberately:** `build <file.tmd>` promises ONE self-contained file and still
    inlines — the pin asserts that direction too, so this cannot be "fixed" into breaking the
    single-file promise. Only the **roman** face is preloaded (the italic is a minority of a
    page's text; preloading it would fetch 64 KB on every page for text most pages lack).
    Browser-verified: both faces `status: "loaded"`, the rendered width differs from the serif
    fallback (580.5 vs 558.6, so it is the real face and not a silent fallback), the font is
    fetched **once** and starts *before* the stylesheet, zero console messages.
    **Untouched sibling, still open as filed:** `katex.css` is 361 KB of which 339 KB is 20
    inlined font `data:` URIs — same shape, but conditional (4 of 23 pages), so it ranks below.
  - **The three conditional blobs are written only when something links them** (137). Measured on
    prose-only `corpus/tarn`: `_assets/` **4,751,169 → 281,886 B, a 94% cut**. They are hashed up
    front (a page needs the href) and flushed after every HTML surface exists. The vote is read
    off the **emitted href**, not by re-deriving the render-time predicates, so any future emitter
    that links one is covered automatically. **The flush sits after the 404 page and before the
    book `.zip` on purpose** — a vote arriving after the flush is a published page pointing at a
    file that was never written, and the zip is the offline artifact that would carry the hole.
    Verified in a browser in the direction that would 404: on a page using all three, mermaid
    renders 3 SVGs with real geometry, KaTeX 4 nodes, every request 200.
  - **The Label-in-Name static rule** (124's residual). `TAL-A11Y-LABEL`, WCAG 2.1 AA 2.5.3: an
    `aria-label` that does not *contain* the control's visible text. **The emitter fix and its pin
    were already shipped when this was picked up** — `chrome.rs`'s search button already carried
    `aria-hidden='true'` on the kbd — so only R9's rule was left; this file said otherwise, which
    is the usual rot. Containment, not equality (a name may add context, never replace it), and
    `aria-hidden` subtrees are excluded, **which is the whole subtlety**: counting a hidden
    shortcut hint as the visible label would accuse the sanctioned fix of being the defect. Skips
    `aria-labelledby` (resolves against ids this block-local scan cannot see) and icon-only
    controls (rule 2's business). **Zero hits across all 16 corpus/docs/site projects**, all still
    `check` exit 0 — it fires on the defect, not on real content.
- **2026-07-28 the publication-readiness batch (items 84, 89, 90, 92, 93),** each pinned by a
  test and verified by mutation. **Do not re-scope any of the following as open:**
  - **`tools/gates.sh` is the one gate script** (84). It *preflights* every prerequisite before
    the first slow gate and **refuses to run** when one is missing, because a partial run that
    looks green is the whole defect; `--allow-missing` downgrades the verdict to **INCOMPLETE**
    (exit 2) rather than letting a skip pass as success. It reads `${PIPESTATUS[0]}` (the filed
    trap: `cmd | tee log` reports *tee*'s status), arms all four `TALIESIN_REQUIRE_*` variables,
    asserts each interpreter's **named canary test** printed `... ok`, and fails on a single
    ignored test. `crates/core/tests/gate_script.rs` derives both lists from the tree, so a new
    REQUIRE gate that nobody arms, or a renamed canary, fails the suite instead of hollowing the
    script out silently.
  - **`CONTRIBUTING.md` exists and carries the inbound licence grant** (89). It is the only
    tracked file with `git config core.hooksPath .githooks` (git does not do this for you, so a
    fresh clone is gated by nothing), and clause 3 grants a **relicensing** right — without it the
    README's reserved right dies on the first merged PR. Pinned by `gate_script.rs`.
  - **`.github/workflows/ci.yml` is restored, plus a new `release.yml`** (90, 92). **The premise
    was re-checked and holds**: standard runners are free and unlimited on *public* repos, and the
    2026-01-01 pricing revision kept that explicitly. But publication has **not happened**, so
    **every job in both files carries `if: github.event.repository.private != true`** — inert on
    this private repo, armed the instant it is public, no follow-up commit to forget. The guard
    degrades toward *running* (`null != true`) for events with no repository payload. `ci.yml` also
    gained `TALIESIN_REQUIRE_CHROME`, which did not exist when it was deleted. **Because the guard
    means CI genuinely checks nothing today, the "no CI" prose was corrected rather than reversed**
    — `stale_docs.rs` now asserts the guard and the prose *together*, across every file in
    `.github/workflows/`, so making CI live cannot silently leave the docs lying.
  - **Install expectation, binaries, platform matrix** (92). Measured, not estimated: a cold
    release build is **2m 11s / 268 crates / 2.6 GB peak** on four cores, producing one ~40 MB
    self-contained binary. `release.yml` attaches a tarball + `.sha256` per platform on a `v*`
    tag, packaging `LICENSE` + `THIRD_PARTY.md` beside the binary (AGPL: a bare executable is a
    distribution stripped of its terms). **Windows is stated as unsupported**, not omitted.
    `release_targets.rs` pins the README matrix against the workflow's matrix **in both
    directions** — an undocumented target is invisible, a documented one nothing builds is a
    broken promise.
  - **"Coming from Quarto"** (93) is `docs/guide/using/from-quarto.tmd`, chapter 2 of the User
    Guide. Its thesis is that **`taliesin check` is the migration assistant** and the page is only
    the map. `quarto_migration_page.rs` parses `NON_HTML_FORMATS`, `RETIRED_KEYS`,
    `UNSUPPORTED_KEYS` and `MIGRATED_DOC_EXTS` **out of the sources** (they are `pub(crate)`, and
    widening them for a test would be the tail wagging the dog) and requires the page to name every
    entry — so adding a format to the diagnostic and not to the page fails the suite.
- **2026-07-28 item 83 — the five MIT tags are DELETED (owner-approved).** Every tag predating
  the relicence commit (`3d474cb`, 2026-07-19) carried an MIT `LICENSE` while HEAD ships
  AGPL-3.0, so cloning any of them leaked the dual-licence moat README and `deny.toml` call the
  commercial strategy. **None had ever been pushed** (`git ls-remote --tags origin` was empty), so
  nothing leaked; the exposure would have begun at publication, and item 100's Phase 2 pushes a
  rewritten history to a *new public* repo. Deleted: `v0.2.0` (`268a18f`),
  `stable-2026-06-22` (`4fbb60b`), `stable-2026-06-25` (`d270bed`),
  `stable-2026-06-30` (`7eca3a5`), `stable-2026-07-07` (`df394c6`). **All five commits are
  reachable from `main`**, so only the labels went; re-tag any SHA above if a snapshot is wanted.
  Safe by measurement, not assumption: `~/.local/bin/taliesin-stable` is a frozen 19 MB **binary**,
  not a tag checkout, and `taliesin-promote` only ever *creates* tags. **The durable rule: never
  tag before the licence is settled, and any new release tag must be cut from a tree whose
  `LICENSE` matches `Cargo.toml`.** The surviving tag is `prune-markdown-bloat-work` (2026-07-26,
  already AGPL).
- **2026-07-28 the launch-blocker batch (items 79-82, 109, 117, 118, 120, 121, 127, 128),** all
  mutation-verified. **Do not re-scope any of the following as open:**
  - **`mounts:` is contained** (80): `Mount::resolve` refuses an absolute `path:`, a climb past the
    site root's **parent**, and a symlink whose target leaves it; enforced once in `load_config`, so
    every consumer (preview, `build`'s recipe, `map`, link validation) sees a filtered list, plus a
    belt-and-braces refusal at the preview call site that turns a path into an HTTP root. The
    boundary is the parent **on purpose** — every real mount is a sibling (`../docs/guide`,
    `../corpus/course`) and all **seven** in `site/_site.yml` still resolve. **Do not narrow it to
    "no `..` at all"**: a test asserts declared-mounts == kept-mounts on the real config.
  - **`check` does not spawn a project-supplied interpreter** (81): a `_site.yml` `python:`/`r:`
    field or the project's own `.venv` is *reported, never run*, with `runs: null` and a
    `not_probed` reason; `--require-kernel` is the opt-in. The MCP/JSON path deliberately has **no**
    opt-in. `TALIESIN_PYTHON` and a bare `python3` are the user's own choice and still probe.
  - **`--no-exec` covers `{js}`** (79): a `{js}` cell is a code cell whose runtime is the browser,
    so it renders as highlighted source, and a labelled `{js}` figure no longer burns a figure
    number it will not emit. One owner for the flag (`render::no_exec_in_force`); the server's
    `exec_disabled` delegates to it. **Deliberately NOT a sanitizer** — raw `<script>`,
    `include-*` and `css:` still pass through, and the CLI reference's new "Documents you did not
    write" section says so. Do not re-scope as "strip the HTML too" (2026-07-03 CSP ruling).
  - **A deck in a site is validated** (109): `collect_site_diagnostics` walks `site.decks` at
    `Scope::Standalone`, and `build`'s deck loop counts the deck's render + static + **cross-ref**
    diagnostics toward `--strict`. The two front doors now report the same count on the same file.
  - **A comma fence highlights on its first token** (127): ` ```rust,ignore ` → `language-rust`.
    Pinned by a fixture in `corpus/highlight.tmd`. **Do not "fix" the class to keep the attribute**
    — a comma is not a valid class token, which was the defect.
  - **A migrated link gets a did-you-mean** (128): `creators.qmd` → "did you mean `creators.tmd`?",
    on both validators, and it lifts into the structured `suggestion.replacement` an agent or an
    editor quick fix applies. **A suggestion, never a rewrite** — a `.md` link may point at a real
    shipped `.md`, and a test row pins that it is left alone.
  - **`new deck` in a site says how to use it there** (120), words not a write; and the loose-deck
    warning names **`format: deck`**, the value the author wrote, not `revealjs` (121).
- **Deck PDF export: already deleted (2026-07-12 deck audit, A2), do not re-scope "remove it."** Asked
  again 2026-07-27; pinned gone by `render/tests.rs:1950`. What survives is ~25 lines of `@media print`
  in `deck.css:522` that keep a stray Cmd/Ctrl+P legible — **that is a don't-emit-garbage guard, not
  PDF export, and it is already free, so keep it.** (The stale *marketing claims* are live work: item 75.)
- **2026-07-27 item 76 — a book has no right-rail TOC.** The gate is `Site::page_toc`, ahead of the
  page's own `toc:`, so a page-level `toc: true` cannot reinstate it and all four assemblers share one
  decision. **Do not re-scope as "give books their TOC back"** (owner ruling, reversing 2026-07-06)
  **or as "delete the rail everywhere"**: websites and single documents keep the rail, `toc-spy.js`
  and the shared `TOC_SHEET_MARKUP` (still the one copy — a book simply never reaches it). `toc:` is a
  website key now, and `validate_toc_scope` tells a book author the key is inert.
- **2026-07-27 the drawer marks which section of the open chapter you are in** (author-asked, the
  natural completion of 76: the expanded chapter row was the only section-level surface a book had
  left). `.tali-book-section-active` + `aria-current="location"` on the current chapter's panel only,
  off the same `scroll-margin-top` activation line as `toc-spy.js`. **Do not re-scope as "give the
  drawer a scrollspy"** — it is computed on each open, deliberately: the drawer locks the root
  scroller, so nothing can move while it is on screen and a scroll listener would watch a dead event.
- **2026-07-27 item 77 (the four 72-75 residuals):** shortcode arguments are linted against a closed
  vocabulary with did-you-mean, and shortcode diagnostics became the **`TAL-SHORTCODE` WARNING**
  family instead of falling through to `(TAL-CHECK, ERROR)`, where a one-letter typo blocked
  `build --strict`/`publish`. `favicon:` resolves through `chrome::site_asset_href` like `logo:`
  (site-absolute and external pass through unprefixed). A book brands on `logo:` alone; **a book with
  neither title nor logo still emits no brand link, deliberately.** The fourth was refuted — see State.
- **2026-07-27 mutation campaign (items 58-69):** every measured survivor in `crates/core`'s five
  post-07-18 files, the ten `crates/server` files and `lsp_nav.rs` is triaged and pinned; the
  unkillable ones are recorded in the two findings docs' tables. **Do not re-run it against the same
  scope.** Method in [LESSONS.md](LESSONS.md).
- **2026-07-27 item 66:** `404.html` links the shared `_assets/` bundle (355,700 → 16,185 bytes on
  `corpus/tarn`); its hrefs are root-absolute on purpose, so a project-subpath deploy degrades to
  unstyled rather than mislinking. The preview keeps the self-contained form.
- **2026-07-27 item 67** (outside the repo, `~/.local/bin/taliesin`): the launcher exits early for
  `__complete` only — 24.3 s → 0.024 s per tab press. **`completions` is deliberately NOT exempt**
  (run by hand, generates a shim from the binary's own command list, so stale is wrong there).
- **2026-07-26 deck weight + headless bounding (items 52, 55):** a site deck went 4,583,261 → 6,962
  bytes via a separate `deck.<hash>.{css,js}` pair (**a deck cannot link the page's `app.js`** —
  `search.js` would steal Cmd-K); every headless browser phase is bounded with teardown kept
  reachable. The standalone artifact stays 4.4 MB and self-contained on purpose.
- **2026-07-26 path-parity batch (items 50, 51, 57, PP-1..3):** one document now renders the same
  whichever command renders it. `render_single_doc` decides the single-document containment root once
  (nearest `_site.yml`, else the doc's own directory); `TOC_SHEET_MARKUP` is the one copy of the
  mobile-sheet chrome all four assemblers emit; the single-doc preview ships Cmd-K. **Do not re-scope
  as "give the single-file build the inferred root"** — that is a revert of `9359a2c`.
- **2026-07-26 migration UX (items 53, 54):** a pre-rename `_quarto.yml` is no longer silently
  defaulted, and retired keys carry the scope they were retired from. Both messages append to the
  classified prefix, so neither needed a new diagnostic code.
- **2026-07-26 mobile batch (items 42-49, MOB-1..8):** the tree now asks what device it is on
  (`hover`/`pointer` media features; it had none). Deck menu drops its keyboard legend + hint badges
  and gates Speaker view on capability instead of orientation; the ⌘K badge is hidden on touch at any
  width; copy-code shows and the heading anchor dims on touch; the book drawer locks page scroll and
  keeps focus through outline hydration; touch nav targets grow by overlay; the sticky book topbar
  truncates instead of wrapping.
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
- **2026-07-25 band-B batch:** AP3-3 (the kernel port re-roll), PA-M3 (listing list semantics), PA-M13
  (`image:` without `image-alt:` warns), PA-H1's residuals (deck `theme-color` + social meta).
- **Earlier, closed:** the backlink-context + resume batch, the book-wayfinding batch, the hardening
  batch, book-level `theorems:`, live-executor mounts (F-04), structure-preserving book-aware `read`,
  AP8-1's output scrub, the DET-1 reproducibility guard, the DX audit batch, `taliesin lsp`, DX17(a)+(b)
  headless executed output, the deck audit, the polish audit batch, the PMF builds, corpus-coverage, the
  machine-facing audit, AI-native packaging, the R/Python ANSI leak, ungraceful-death reaping, and the
  `assets/js` `tsc` gate.

### Decided against

- **"Adjacent slides bleed into the deck's letterbox" (DT-5, filed and RETRACTED 2026-07-27, same
  day):** **false — the letterbox is empty.** `.tali-deck` is sized to the 16:9 stage
  (`min(100%, 100vh*16/9)`) with `overflow: hidden`, and its comment already says "adjacent cells
  fall outside and are clipped (no peek)". The probe intersected each neighbour with the
  **viewport** instead of with its **clipping ancestor**, and `getBoundingClientRect` knows nothing
  about `overflow: hidden` — re-measured, the neighbour contributes **0 px** inside the clip box and
  `elementFromPoint` returns `BODY` there. **Do not re-file it from a rect measurement**; if it ever
  looks true again, the only valid evidence is a rendered pixel, not a rectangle.
- **Deck presenter tools** (one-command publish, laser/spotlight, auto-advance): declined 2026-07-22 and
  **re-declined 2026-07-26** on the same grounds — no real speaker ask has appeared. Revive only when the
  author actually presents from Taliesin. (`footer:`/`logo:` from that item did ship.)
- **WS op-message batching** (declined 2026-07-25 **on measurement, premise confirmed**): the worst case
  is 55 ops in one frame, but a warm edit is 32.2 ms of which the diff is 0.94 ms, so batching saves
  ~220 bytes on a 32,303-byte payload (0.7%), none on the critical path. Reopen only if render cost drops
  far enough that framing is measurable.
- **Item 29's reduction residuals R1 + T2** (closed 2026-07-25 without code): R1's `text_content` /
  `indexable_text` fork is deliberate and equalizing them would leak raw entities into `llms.txt`; T2's
  "three modules pre-scan" is partly rotted — the real duplication is a six-line idiom in two places, and
  the divergence that looked like a latent bug is unreachable.
- **Deck-motion, whole item** (detail: [2026-07-24-deck-motion-audit.md](2026-07-24-deck-motion-audit.md)):
  Option A + residuals shipped; **(3) no-change** ruled; **(4) Option C (shared-element FLIP) declined —
  do not re-cost it a third time**. A coverage-weighted refinement of (5) measured *worse* (15 of 25
  slides vs 23 of 25); do not re-refine without measuring.
- **A separate per-page outline artifact for the book drawer** (declined 2026-07-25 while building it):
  the index it would duplicate is already lazy-loaded on every page, so a sidecar buys ~55 KB gzipped on
  one cached subresource in exchange for a second copy of the render recipe, assembly, invalidation,
  route and build write.
- **`drawer-typeahead`** (declined 2026-07-25): Cmd-K plus the drawer's collapsible outline covers it, and
  a second search-like box beside a Search button is a discoverability smell.
- **A "~N min read" label on a book chapter** (2026-07-25): `prose::word_count` excludes fenced code and
  math, so a code-heavy chapter is understated — and reading code is *slower* than prose, so the error
  goes into a promise about the reader's time in the wrong direction, on exactly the chapters this tool
  exists for. (The dated-post estimate in `render/mod.rs` is a different surface; `is_article` is
  test-pinned, do not touch it.)
- **Flipping a book chapter's label to prefer `title:` over its `# H1`** (resolved 2026-07-25): measured
  across every book in the repo, only 3 of 48 chapters differ and in 2 the `# H1` is the *better* nav
  label. Resolved as documentation, not code.
- **CAD-as-code** (`{openscad}` / CadQuery cell → live 3-D preview; researched 2026-07-23, NOT built):
  technically feasible and legally green, killed on **demand**. **Do not bundle openscad-wasm (GPL).**
  Five named revisit triggers in [2026-07-23-cad-as-code-research.md](2026-07-23-cad-as-code-research.md).
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
  fixed; include symlink-loop SIGABRT does not exist (Linux caps at `MAXSYMLINKS=40`); **decks pass path
  parity outright** and `mounts:` differs from direct serving by 4 bytes (boot nonce + ws path).
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
