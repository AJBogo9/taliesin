# The pre-publication audit slate

**Date:** 2026-07-27

Fourteen dispatchable audit rounds (thirteen core plus one optional), each a self-contained brief.
Round numbers are stable identifiers, not an ordering: R14 appears before the optional R13 because
R13 is the tail. The
slate exists because [backlog.md](../../../notes/backlog.md) has drained to one non-coding item in
band A and an empty band B, while [AUDITS.md](../../../notes/AUDITS.md) records that all twelve AP
slots and every proposed L-lens have run. A session can take any single round below and execute it
without reading the others.

## Why this slate exists

Every audit Taliesin has run asks one question: **is this correct?** Correctness, load-bearing
invariants, races, fuzzing, determinism, cache behaviour, chaos, accessibility, mobile, mutation
coverage. Roughly thirty rounds, all variants of the same question, and the question is now
answered: four fresh lenses on 2026-07-26 produced **zero HIGH findings**.

Professional audit practice asks four other questions. Taliesin has never asked any of them.

| Question | Instrument | Rounds run here |
|---|---|---|
| Is this correct? | code review, fuzzing, mutation, chaos | ~30 |
| Is this **detectable**? | FMEA detection axis, internal-control audit | **0** |
| Does this **hold under scenario stress**? | ATAM (SEI) | **0** |
| Would a stranger **adopt** this? | JTBD four forces, commercial due diligence | **0** |
| Can this be **handed over**? | technical due diligence, continuity audit | **0** |

That table is the thesis. The lens menu did not run out of value; it ran out of one column. The
remaining yield is in the other four, and it is reachable only with instruments the project has not
used.

A second framing that shaped the slate: **the tool is about to meet strangers.** The author intends
to publish and collect real feedback. Every round below is ordered by how much it changes the
quality of that first contact.

## Evidence already on the board

Measured 2026-07-27 while scoping this slate, so no round needs to re-derive it. These are inputs,
not findings, and each still needs its round to decide what (if anything) to do.

- **No `.github/` directory exists.** Zero issue templates, zero CI workflows.
- **No `CONTRIBUTING.md`, no `CODE_OF_CONDUCT.md`.** `SECURITY.md`, `LICENSE`, `CHANGELOG.md`,
  `THIRD_PARTY.md` and `README.md` are present.
- **Bus factor is 1**: 1,581 commits, one author.
- **Licence is `AGPL-3.0-only`**, the single most-cited adoption-anxiety trigger in the technical
  due-diligence literature. This is a deliberate choice, not a defect; the round's job is to decide
  what to *say* about it, not to change it.
- **The only gate is `.githooks/pre-push`**, wired through `core.hooksPath`, so it is local to the
  author's machine. A contributor's pull request runs nothing.
- **Kernel cells use a positional cascade**, not a dependency DAG: [`exec.rs:13`](../../../crates/server/src/exec.rs)
  says "only the changed cell + downstream re-run (notebook semantics)". Meanwhile
  [`diagnostics/reactive.rs`](../../../crates/core/src/diagnostics/reactive.rs) gives `{js}` cells a
  real reactive graph with cycle detection. **Taliesin already ships the reactive DAG marimo is
  famous for, but only on the JS side.**
- **[`a11y.rs`](../../../crates/core/src/diagnostics/a11y.rs) ships exactly three static rules** and
  says so in its own docstring, deferring contrast and document `lang` to a live audit that has
  never been run with a real conformance tool.
- **Decks are exempted from two diagnostic families.** `validate_document_shape` early-returns on
  `DocFormat::Reveal` ([`shape.rs:97`](../../../crates/core/src/diagnostics/shape.rs)), so no
  `TAL-SHAPE-*` warning can ever fire on a slide; and `validate_a11y` skips heading-level checks for
  decks ([`a11y.rs:228`](../../../crates/core/src/diagnostics/a11y.rs)). Meanwhile `deck.js` is 2,690
  lines, the largest hand-written JS file in the tree. **The largest hand-written client subsystem
  has the fewest automated checks.** This is R14.
- **The page `<title>` is already WCAG-correct**: [`page.rs:95`](../../../crates/core/src/render/page.rs)
  composes `"{page} · {site}"`, page name first. This is the order Quarto gets wrong. Do not refile
  it as a defect.

## Shared contract

Every brief inherits all of this. A round that skips it produces findings the project has learned
not to trust.

1. **A filed cause is a hypothesis.** Two of the last several HIGH findings were retracted on
   measurement (DT-5's letterbox, item 77's scree plot). Every finding states the measurement that
   produced it and what observation would refute it. An inference is not a finding.
2. **Trust the symptom, never the stated cause, line number or cost.** All three have rotted before.
   Re-derive from source.
3. **A finding that names one instance has not enumerated the shape.** Enumerate before filing.
4. **An audit's stated fix can be a revert.** Read the code a proposal would change, and check it
   against the "Do not re-add / re-scope" list in [backlog.md](../../../notes/backlog.md) before
   filing anything.
5. **Hard scope guard.** HTML is the only output target. No CDN or network egress. The preview never
   writes back to source. Prefer a better default over a new configuration knob. A finding that
   requires violating one of these is not a finding, it is a scope error.
6. **The one standing freeze** is `MAX_WARM_PAGES` plus the deterministic LRU in
   `serve_site/exec_pool.rs`. Do not propose tuning it without a fresh ruling.
7. **Read [LESSONS.md](../../../notes/LESSONS.md) before writing any probe.** Three traps bite this
   slate specifically:
   - A whole-page `contains()` passes vacuously, because every page inlines the full CSS and JS
     payload. Needle the emitted tag, not the page.
   - `getBoundingClientRect` reports geometry, not visibility. A box can span the screen and paint
     nothing.
   - "Served 200 OK" is not "renders." An SVG shipped as a broken image while every check passed.
8. **Rebuild the binary before measuring built output.** `assets/css` and `assets/js` are
   `include_str!`-compiled, so rebuilding only the site re-emits stale bundles.
9. **Verify a fix by mutation** (restore the bug, watch the named test fail), never by a green suite.

## Guardrails

A second session is working the same tree concurrently, and heavy cargo runs are charged to the
VS Code cgroup, which has previously taken down the desktop.

- **Worktree-isolate any round that has Bash.** A "read-only" reviewer once ran `cat > Cargo.toml` in
  the repo root and destroyed the workspace manifest.
- **Never `pkill -f taliesin`.** It kills the other session's servers and the running shell. Kill by
  PID only.
- **Use a non-default preview port** so two sessions do not collide.
- **Each round writes only its own dated file under `notes/`.** No round edits
  `notes/backlog.md`, `notes/AUDITS.md` or `notes/LESSONS.md`. Merging into the backlog happens once,
  at the end, by the coordinating session.
- **Bound `CARGO_BUILD_JOBS`** on anything that compiles, or wrap it in
  `systemd-run --user --scope -p MemoryMax=`.
- **Read-only git for subagents.** No commits, no branch switching, no `git checkout`.

## Output conventions

- Each round writes `notes/2026-07-27-<slug>-audit.md`.
- Proposed backlog items are numbered from **79** upward (78 is the highest live item). Numbers are
  stable and never reused. Reserve a block per round at dispatch time so two rounds cannot collide.
- Every finding carries: the measurement, a `file:line` where one applies, a severity, and the
  refutation test.
- **A confirmation is a valid result.** "Measured, healthy" is worth recording, and several rounds
  below are expected to return mostly that.

---

# Family I. Contact with strangers

Launch-critical. All five are novel instruments here. These change the quality of the feedback the
project receives after publication, which makes them the highest-leverage rounds on the slate.

## R1. Adoption friction: audit for anxiety, not appeal

**Question.** What stops someone from switching to Taliesin, given that they like it?

**Why novel here.** The four demand probes (course-author, docs-maintainer, interactive-explainer,
analyst) all measured *pull*: can a persona finish a task. The Jobs-to-be-Done literature's central
result is that teams over-invest in pull while **anxiety** and **habit** are what actually block
adoption. The switching equation is `Push + Pull > Anxiety + Habit`. Taliesin has measured one term
of four.

**Method.** Build the four-forces model for three switcher profiles (a Quarto user, a Jupyter or
marimo user, a plain-Markdown or mdBook user). For each, enumerate push, pull, anxiety and habit
from evidence, not imagination: real forum and issue-tracker language where available, and the
repository's own artefacts where not. Then rank anxiety-reducers by cost against blocking power.

**Targets.** README, the two dogfooded books' opening pages, the marketing site copy, `taliesin
init` output, the licence, and the absence of an exit path.

**Known threads to test, not to assume.** AGPL-3.0-only; bus factor 1 with no CONTRIBUTING; no PDF
or other escape hatch, which is a deliberate invariant but reads as lock-in to an evaluator; `.tmd`
as a novel extension; no visible answer to "who else uses this"; no import path from an existing
corpus.

**Deliverable.** A ranked anxiety register, each entry with the specific artefact that would reduce
it (a paragraph, a command, a page), plus an explicit list of anxieties the project should accept
rather than fix, with reasoning.

**Traps.** Do not convert this into a feature wishlist; the output is mostly words and affordances,
not code. Do not propose changing the licence or adding an output format. Both are rulings the owner
has already made, and this round's job is to name the cost, not reverse the decision.

## R2. First contact and cognitive walkthrough

**Question.** What does a stranger hit in the first ten minutes, and where does the interface fail
its own users?

**Why novel here.** Every round to date began from the author's machine, the author's knowledge and
a warm environment. No round has started from zero. Nielsen's ten heuristics and the cognitive
walkthrough are formal instruments that have never been applied to the authoring experience, as
distinct from the technical accessibility of the output.

**Method.** Two passes.
1. **Cold start.** Clean environment: no `TALIESIN_PYTHON`, no launcher on PATH, no Jupyter kernel,
   no prior config. Install, `taliesin init`, first `preview`, first deliberate error, first deck.
   Record every point of confusion, every message that assumes prior knowledge, and time to first
   rendered page.
2. **Heuristic evaluation + cognitive walkthrough.** Nielsen's ten heuristics against the authoring
   loop, then a scenario-driven walkthrough of two goals: "author publishes their first document"
   and "reader finds one fact in a book." At each step ask the four walkthrough questions: will the
   user try to achieve this, will they notice the control, will they recognise it as the right one,
   and will they understand the feedback.

**Targets.** CLI help and error text, `init` scaffold, diagnostics as a first-time reader sees them,
the preview's own affordances, the docs' entry points, and any prose that contradicts behaviour.

**Deliverable.** A ranked usability register keyed to the violated heuristic, plus the cold-start
timeline with the specific step where a stranger would give up. Absorbs the prose-versus-behaviour
lens: any documentation claim that misleads a first-time user is a finding here.

**Traps.** Install friction and documentation gaps are the two most-cited abandonment causes for new
CLI tools, so resist the urge to explain away friction with "the docs cover that." If the walkthrough
had to consult the docs, that is the finding.

## R3. Pre-mortem

**Question.** It is mid-2027. Taliesin was published and it failed. Why?

**Why novel here.** Every round asks what is wrong now. This asks what will kill the project later,
which is a different cognitive stance and reliably surfaces different material. Prospective hindsight
improves risk-identification accuracy by roughly 30%, and the technique costs almost nothing.

**Method.** Klein's protocol, adapted for a small team.
1. Assert the failure as fact, not possibility. No hedging.
2. **Independent silent generation first.** Each perspective writes every reason for the failure
   before any discussion. This is the step that carries the value; skipping it collapses the output
   to consensus.
3. Round-robin the reasons into one list without debate.
4. Only then cluster, rank by plausibility against impact, and identify mitigations.

Run at least three distinct failure framings so the output is not one story: nobody adopted it; people
adopted it and left; it succeeded and became unmaintainable.

**Deliverable.** A clustered failure register with mitigations, and an explicit shortlist of the
failures that are cheap to insure against now and expensive to fix later.

**Traps.** The documented failure mode of this technique is running it with an authority present,
which collapses candour. Applied here: do not pre-filter reasons against the owner's known rulings
during generation. Filter in step 4, never in step 2.

## R4. Technical due diligence and contributor readiness

**Question.** What does a serious contributor, an acquirer or a grant committee find, and can anyone
but the author land a change?

**Why novel here.** The 2026-07-17 security round covered supply chain, secrets and the `--host`
token design. It did not cover continuity, handover or contributor mechanics. `FUNDING-RESEARCH.md`
exists, so an external technical review is a live possibility rather than a hypothetical.

**Method.** Run a standard technical due-diligence checklist against the repository: architecture and
documentation quality, technical-debt quantification and whether deferred maintenance shows a
pattern, licence compatibility across the dependency tree with an SBOM, secrets scan, reproducible
build from a clean clone by someone who has never built it, and key-person risk.

Then the concrete test: **starting from a clean clone and the README alone, can a stranger build,
test, and land a correct pull request?** Follow the documented path exactly and record where it
breaks.

**Targets.** `Cargo.toml` and the workspace, `THIRD_PARTY.md`, `SECURITY.md`, `CHANGELOG.md`, the
absent `.github/`, `.githooks/pre-push` and its `core.hooksPath` wiring, the release and versioning
story, and the four hand-run gates named in `CLAUDE.md` that no automation covers.

**Deliverable.** A due-diligence report with a red-flag register, plus a minimal contributor-readiness
set scoped to what actually matters before publication rather than a generic OSS checklist.

**Traps.** Do not reflexively recommend restoring CI. It was deliberately deleted on 2026-07-26 for
billing reasons on a private repository, and a public repository changes that calculation, so the
round must re-derive the decision rather than assume either answer. Note also that the four hand-run
gates (live Python and R kernels, both `tsc` checks, the middleware test, `cargo audit` and
`cargo deny`) are the ones most likely to have rotted, since nothing runs them.

## R5. Trust and safety: the document as attack vector

**Question.** What happens when a user opens a document they did not write?

**Why novel here.** `.tmd` executes code by design. Before publication, every document was the
author's own, so this threat model did not exist. Publication creates it. The security round audited
Taliesin as a *program*; this audits `.tmd` as a *format that arrives from elsewhere*.

**Method.** Threat-model the untrusted document. Walk the paths a hostile or merely careless `.tmd`
can reach: code cells against the warm kernel, `{{< include >}}` path traversal, shortcode arguments,
front-matter, `{js}` cells in the reader's browser, `mounts:`, asset paths, and the headless browser
path. For each, establish whether an existing control stops it, and what the user is told.

**Targets.** `includes.rs` containment, `exec.rs` and `kernel.rs`, the extension and shortcode
expansion path, `headless_js.rs`, and the preview server's exposure under `--host`.

**Deliverable.** A threat model with the controls that exist, the ones that do not, and a
recommendation on what to *document* versus what to *enforce*. Explicit stance on the central
question: is executing an untrusted `.tmd` meant to be safe, or is it meant to be understood as
running someone's code, in which case the deliverable is a clear warning rather than a sandbox.

**Traps.** Do not design a sandbox. Executing code is the product, and the likely correct answer is
documentation plus a consent affordance, not containment. Check `SECURITY.md` first; it may already
take a position, in which case the finding is whether that position is discoverable.

---

# Family II. Structural

Novel instruments on surfaces that have been looked at before with other tools. These produce
architectural and risk-level findings rather than defects.

## R6. ATAM: architecture tradeoff analysis

**Question.** Where does the architecture bend, and which of the three load-bearing goals gives way
first?

**Why novel here.** AP10 assessed codebase health. That is not a scenario-based architecture
evaluation. Taliesin has three explicitly stated load-bearing goals (click-to-source, block-level
incremental updates, no per-edit startup cost) which are textbook quality attributes, and they have
never been stress-tested by scenario. The `MAX_WARM_PAGES` freeze is a sensitivity point that was
discovered by accident; ATAM finds the rest on purpose.

**Method.** The SEI protocol, compressed.
1. State the architecture and the driving quality attributes.
2. Build a **quality-attribute scenario set**: concrete stimulus, environment, and measurable
   response. Cover the three load-bearing goals plus offline guarantee, single-editing-surface, and
   HTML-only.
3. Analyse each scenario against the architectural decisions that serve it.
4. Classify the output into **risks**, **non-risks**, **sensitivity points** (a small change causes a
   large quality impact) and **tradeoff points** (a decision that helps one attribute and hurts
   another).

**Deliverable.** A risk and non-risk register, plus the sensitivity and tradeoff points named
explicitly. The sensitivity list is the durable artefact: it tells future sessions which lines are
load-bearing beyond the one already-frozen constant.

**Traps.** Non-risks are a real output, not filler; recording that a decision is safe under its
scenarios prevents it being re-litigated. Do not let this become a refactoring proposal. ATAM
produces a risk register; what to do about it is a separate ruling.

## R7. FMEA with a detection axis

**Question.** Which failures would nobody find out about?

**Why novel here.** AP11 injected failures and watched degradation. It did not score **detection**.
That is the novel column and the reason this round exists: with no CI, no telemetry by design, and
the corpus as the regression net, Taliesin's detection scores are systematically weak, and the
undetectable failures are exactly the ones that hurt after publication.

**Method.** Standard FMEA. Enumerate failure modes across the failure surface. Score each on three
1-to-10 scales: **Severity** (impact when it happens), **Occurrence** (likelihood), and **Detection**
(likelihood of catching it before a user does, where 10 means it would not be caught). Compute
`RPN = S × O × D` and rank.

The scoring rubric must be written down before scoring, or the numbers are not comparable across
modes.

**Targets.** The whole surface, but the detection column should pay special attention to the classes
`LESSONS.md` already names as structurally invisible: shapes the corpus does not contain, dogfood-only
shapes outside the regression net, prose claims no gate compares against behaviour, and the four
hand-run test gates.

**Deliverable.** A ranked risk register with the RPN table intact, and separately the **high-detection
-score cluster**: the failures that would ship silently regardless of severity. That cluster is the
round's real product.

**Traps.** RPN is a ranking aid, not a truth. A severity-10 with a low RPN can still deserve action,
so present severity separately rather than only the product. Do not score detection optimistically
because a test exists; ask whether the test would actually fail, which is the mutation question.

## R8. Author value stream mapping

**Question.** Where does an author's time actually go, end to end?

**Why novel here.** This is the sharpest round on the slate, because it tests the moat where it pays.
AP1 and L2 measured *tool* time: render milliseconds, LCP, throttled load. Nobody has measured the
**author's** cycle time across a real document lifecycle. A tool can be fast at rendering and slow at
authoring, and the entire product thesis rests on the second claim while only the first has ever been
measured.

**Method.** Lean value stream mapping. Take one realistic document end to end (idea, draft, figure,
citation, cross-reference, revision, check, publish). For each step record the time, and classify it
as value-adding, necessary non-value-adding, or waste. Look specifically for waiting, rework,
context-switching, and steps the author performs that the tool could perform.

Then compute the ratio of value-adding time to total elapsed time. That single number is the honest
version of the speed claim.

**Targets.** The full lifecycle including the parts outside the render loop: getting a figure right,
fixing a citation, resolving a diagnostic, and the deploy step.

**Deliverable.** The value stream map with times, the value-add ratio, and the ranked waste list.
Anything on the waste list that the tool could absorb becomes a feature proposal with a measured
justification rather than a guess.

**Traps.** Measure a real document, not a fixture written for the measurement; that error is exactly
what limited the four demand probes. The freeze cache's per-edit rewrite cost is explicitly listed in
AUDITS.md as never measured because the probe's 200 ms poll floors it, so use a finer instrument or
record it as unmeasured.

---

# Family III. Conformance and direction

Six rounds: four carried over from the earlier slate and sharpened, plus R14 (the deck) and the
optional R13. These produce shippable items and one publishable artefact.

## R9. Conformance oracle and a publishable VPAT/ACR

**Question.** Does the output pass a real conformance tool, and can that be turned into a credential?

**Why novel here.** AP7 was a hand audit that produced findings. No conformance *tool* has ever run
against built output, and `a11y.rs` ships three static rules while deferring contrast and `lang` by
its own admission. Separately, the deliverable is new: an **Accessibility Conformance Report** is a
procurement artefact, not a bug list.

**Method.** Run axe-core and Lighthouse against `taliesin build` output (not the preview, and rebuild
the binary first). Cover every corpus project, both dogfooded books, a deck, and the blog, in **both
themes**, since contrast is theme-dependent and static analysis cannot see computed CSS. Then map
results onto WCAG 2.1 AA success criteria in VPAT/ACR form.

**Deliverable.** Two artefacts. The defect list, and a draft ACR. Also a scoped proposal for which
axe rules could become static rules in the kernel-free `check` channel and which genuinely require a
browser, since that boundary is the interesting engineering question.

**Why the ACR matters.** The ADA Title II rule requires WCAG 2.1 AA for public institutions, roughly
70% of edtech tools now publish a VPAT, and accessibility documentation is used to screen vendors
during procurement. For a tool authored at a university, this converts accessibility work that is
already done into an adoption credential.

**Traps.** The `<title>` order is already correct; do not refile it. Competitor bug lists are
hypotheses about Taliesin, never findings. Decks are the richest target and the one where peer tools
are weakest. Do not overclaim in the ACR: an honest "partially supports" is the point of the format,
and an inflated report is worse than none.

## R10. Demand and positioning

**Question.** What do users of web-native document tools want, and what does Taliesin already deliver
without saying so?

**Why novel here.** The PMF audit reasoned internally; the demand probes used invented personas. This
substitutes real voices, and it surveys the space Taliesin actually landed in (Slidev, Observable
Framework, marimo, VitePress, mdBook) rather than the one it left.

**Method.** Survey real user language in issue trackers, discussions and forums. Rank by frequency
against recency. Filter hard to HTML behaviour: requests for PDF, Typst or Word output are
permanently out of scope and are discarded without filing. Verify every candidate against Taliesin's
own engine before filing, per the `<title>` lesson.

**Deliverable.** Deliberately **two** lists.
1. Real gaps worth building.
2. **Shipped but unmarketed strengths.** Early evidence suggests this list is the longer one:
   reproducibility through cumulative content hashing is a stronger claim than a reactive DAG and is
   never stated; the LSP plus companion plus click-to-source directly answers the single most-cited
   complaint about marimo; and the `{js}` reactive graph already exists.

**Sharpest thread.** Whether Python and R cells should get the `{js}` DAG treatment, or whether the
cumulative-hash cascade is the better story told properly. This is a positioning question before it
is an engineering one.

**Traps.** Most requests in this space are for output formats and must be discarded, not filed. A
competitor's weakness is not automatically a Taliesin strength; measure before claiming.

## R11. Real external document

**Question.** What breaks on a document the project did not write?

**Why novel here.** All four demand probes used fixtures written for the probe. `LESSONS.md` already
documents three corpus shape gaps (no include-built chapter, no chapter in a subdirectory, no
front-matter `title:` on a corpus book chapter) and **each one hid a real bug**. A real document
carries shapes nobody thought to invent.

**Method.** Source a substantial real-world document project from a public repository. It need not be
Quarto; a stranger will arrive with whatever they have, so an mdBook, a Docusaurus tree or a folder of
loose Markdown are all valid. Clone, run `check`, run `build`, and read the result in a browser.

**Deliverable.** The defect list, and separately the **shape inventory**: what the document contains
that `corpus/` does not. The inventory is the durable artefact and may outlast the defects.

**Traps.** Do not grow `corpus/` toward the external document wholesale; the walker renders every
corpus doc on every `cargo test`. Pin only reduced shapes that earned their place. `corpus/tarn` is
the existing fixture for scale-sensitive work and must not be grown toward 200 pages.

## R12. Real-device mobile, Android Chrome

**Question.** Does the mobile batch hold on real hardware?

**Why it stays.** It is the only lens with a proven track record of HIGH findings here: the 2026-07-26
round was the author using a real phone and it refilled an empty band A with eight items.

**Method.** Real Android Chrome, not emulation. The mobile batch shipped, so this **verifies rather
than re-finds**, and a confirmation is a valid recorded result.

**Priority order.**
1. The book drawer scroll lock. A root `overflow: hidden` holds less completely off Chromium, and
   item 76 made the drawer a book's *only* navigation surface, which raises what a failure costs.
2. The `--host` QR phone-preview flow. A first-class phone feature with **zero** coverage to date.
3. Momentum scrolling and the dynamic viewport toolbar against the sticky book topbar.
4. Tablet widths.
5. TalkBack on a chapter and on a deck.

**Deliverable.** Findings plus an explicit **remaining-gap statement**: WebKit and iOS Safari are not
covered by an Android round and must be recorded as still unmeasured, or the round will later read as
full mobile coverage.

**Traps.** `resize_page` floors at roughly 500px, so use viewport emulation for width work. The deck
feed flag lives on `html`, not `.tali-deck`. Both traps previously produced false results.

## R14. The deck: the subsystem the checks cannot see

**Question.** What is wrong with the deck engine, given that it is exempted from two of the project's
diagnostic families and that the backlog has deflected deck work eight separate times?

**Why novel here.** This is **not** a re-run of the 2026-07-12 deck audit or the 2026-07-27 touch
crossing. Both audited deck *behaviour*. This audits the deck's **exemption from the project's own
quality machinery**, which no round has ever looked at.

Measured 2026-07-27 while scoping this slate:

- [`shape.rs:97`](../../../crates/core/src/diagnostics/shape.rs) — `validate_document_shape`
  early-returns on `DocFormat::Reveal`. **The entire `TAL-SHAPE-*` family never runs on a deck**
  (`TAL-SHAPE-DUP`, `-EMPTY`, `-HOLLOW`, `-ECHO`, `-CAPTION`).
- [`a11y.rs:228`](../../../crates/core/src/diagnostics/a11y.rs) — heading-level skip detection is
  skipped wholesale for decks.
- `deck.js` is **2,690 lines**, the largest hand-written JavaScript file in the tree (only the
  vendored `mermaid.min.js` is bigger). `deck.rs` plus `deck.js` plus `deck.css` total 4,505 lines.
- Roughly 25 `DocFormat::Reveal` / `is_reveal_doc` branches exist across 9 files.

So **the largest hand-written client subsystem in the project is the one with the fewest automated
checks.** Each individual exemption carries a documented and defensible rationale. The aggregate does
not.

**The second mechanism is this file's own ancestors.** `backlog.md` carries eight deck-related
"declined / do-not-re-scope / retracted / blocked" entries. Every one is a reasonable ruling in
isolation. Together they read as a wall, and a session approaching the deck backs off rather than
measuring. The first draft of *this spec* did exactly that, which is the mechanism observed live.

**Method.**

1. **Build the exemption register.** Enumerate every branch in the tree that treats a deck
   differently, starting from the `DocFormat::Reveal` and `is_reveal_doc` references. For each,
   record: what is skipped, the stated rationale, whether that rationale still holds, and **what
   replacement check exists**. An exemption with no replacement is a hole, and the register makes
   holes countable.
2. **Design the deck-appropriate equivalent for each hole.** `shape.rs` is *correct* that two slides
   sharing a title is the `{auto-animate=true}` idiom rather than a duplicate. That argues for a
   deck-aware duplicate rule, not for no rule at all. The same question applies to every other
   exemption.
3. **Audit `deck.js` as code**, at its true size, against the standard the rest of the tree is held
   to. It has never been read at audit depth as a 2,690-line module.
4. **Cross with corpus coverage.** Which deck shapes does `corpus/deck.tmd` not contain? Per
   `LESSONS.md`, a shape the corpus lacks is invisible to a green suite, and that rule has already
   hidden three real bugs elsewhere.

**Deliverable.** The **exemption register** (every skip, its rationale, whether it holds, and the
replacement check or the hole), plus the defect list, plus a corpus shape-gap list. The register is
the durable artefact: it converts an invisible policy into a reviewable table.

**Traps.**

- **Most findings will not be "remove the exemption."** The exemptions are mostly right. The finding
  is usually "the exemption is correct and the replacement check was never written."
- **Do not re-file DT-5 from a rectangle measurement.** The letterbox is empty;
  `getBoundingClientRect` knows nothing about `overflow: hidden`. Only a rendered pixel counts.
- **Do not re-scope PDF export or presenter tools.** Both are ruled, presenter tools twice. The
  `@media print` block at `deck.css:522` is a don't-emit-garbage guard, not PDF export, and it stays.
- **Do not re-cost deck-motion Option C** (shared-element FLIP). Declined twice already.
- The deck feed flag lives on `html`, not `.tali-deck`. Probing the wrong element previously made a
  working feed look dead.

**Cross-round note.** R7 (FMEA) should score every deck failure mode's **Detection** axis against
this register, because a subsystem exempted from two diagnostic families is by construction the
highest-detection-score cluster in the tree. R9 should treat decks as its primary target rather than
one surface among several.

## R13 (optional). Green software and carbon intensity

**Question.** What is the measured efficiency story, and is it a differentiator worth stating?

**Why novel here.** Never run, cheap, and Taliesin has a genuine case: no CDN, fully offline, a warm
process instead of per-edit startup, incremental rebuilds, and a shared `_assets/` bundle that took
one page from 355,700 to 16,185 bytes.

**Method.** Apply the Software Carbon Intensity specification (ISO/IEC 21031:2024), which measures
carbon per functional unit rather than a total. Two functional units are meaningful here: per
document build, and per page view. Compare the warm incremental path against a cold full rebuild.

**Deliverable.** The measured SCI figures and an honest statement of whether they are strong enough to
claim publicly. A weak result is a valid outcome and should be recorded rather than buried.

**Traps.** Do not add telemetry or network egress to measure this; both violate the offline-first
invariant that produces the good result in the first place. Keep the measurement local.

---

# Wave plan

The whole slate should not run at once. Concurrency contention has previously killed background
agents silently in this project, and a second session is live on the same tree.

| Wave | Rounds | Load | Notes |
|---|---|---|---|
| **1. Launch-critical** | R1, R3, R4, R5 | Low. Reasoning and reading; almost no build. | Safe to run alongside another session. Highest leverage before publication. |
| **2. Structural** | R6, R7, **R14** | Low to moderate. Analysis-heavy. | R14's exemption register is a read of the tree, so it pairs naturally with R7's detection scoring and should run *before* it. |
| **3. Measurement** | R2, R8, R9, R11 | High. Real builds, browser, clean environments. | Wants the tree to itself. R2's cold start needs an isolated environment, not just a worktree. R8 needs the author's attention rather than CPU. |
| **Parallel, anytime** | R12 | Author's phone. | Independent of the tree entirely. |
| **Optional tail** | R13, R10 | Low. | R10 is research-only and writes nothing into the repo. |

Between waves, the coordinating session merges accepted findings into `notes/backlog.md`, updates the
round index in `notes/AUDITS.md`, and files method lessons in `notes/LESSONS.md`. Only that session
touches those three files.

# What this slate deliberately does not do

- **No competitor feature diff.** The owner's position is that Taliesin diverged from and surpassed
  Quarto, so a feature diff measures a race the project has left. R10 surveys demand instead of
  competitors.
- **No re-run of any correctness lens.** All twelve AP slots and every L-lens are recorded as run.
  The one exception is R12, which verifies a shipped batch on hardware that has never been used.
- **No licence, output-format or freeze changes proposed.** These are settled rulings. Rounds may name
  their cost; they may not reverse them.

**Retracted from this section on 2026-07-27:** an earlier draft of this spec said "no deck audit," on
the grounds that the touch crossing had covered it. That was wrong, it was written without
measurement, and it is itself an instance of the pattern R14 exists to break. See R14.
