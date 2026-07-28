# R8 — author value stream mapping

**Date:** 2026-07-28
**Round:** Wave 3 / R8 of the [audit slate](../docs/superpowers/specs/2026-07-27-audit-slate-design.md).
**Question.** Where does an author's time actually go, end to end?

**Why the slate called this the sharpest round.** AP1 and L2 measured *tool* time: render milliseconds,
LCP, throttled load. Nobody had measured the **author's** cycle time. A tool can be fast at rendering
and slow at authoring, and **the entire product thesis rests on the second claim while only the first
had ever been measured.**

**Subject: a real document, not a fixture** — `corpus/tech-blog/posts/em-algorithm/index.tmd`, 439
lines, 3 `{python}` cells, 59 rendered blocks. Measuring a fixture written for the measurement is the
error that limited the four demand probes, and the slate names it explicitly.

**This round is half-complete by design, and says which half.** The measurable half (tool time) is
below. The half that needs the author (thinking, drafting, getting a figure right) is stated as
questions at the end rather than guessed at. New items are numbered from **131**.

---

## Headline

**The warm loop is 90-152 ms end to end on a real document, and that number retires the speed
question rather than answering it.**

Measured in a browser with a `MutationObserver` on `#tali-root`, timestamped against the file write:

| Author action | First DOM response | Fully settled | What the server did |
|---|---|---|---|
| **Prose edit** (one paragraph) | **90 ms** | 90 ms | `restored 3 cached cells · 0 re-ran`, `update 1 block` |
| **Code-cell edit** (real kernel re-run) | **90 ms** | **152 ms** | `restored 2 cached cells · 1 re-ran`, `update 9 blocks` |

Two properties worth naming because neither is accidental:

1. **A prose edit ships exactly one block op and re-runs zero cells** on a document with three live
   Python cells. Goals 2 and 3 doing exactly what they claim.
2. **A cell edit responds in 90 ms and completes in 152 ms.** The first 90 ms is the *markdown* update
   (the author sees their own typing land), and the output block swaps at 152 ms once the kernel
   returns. **The author is never waiting on the kernel to see their edit.**

**The honest version of the speed claim.** At a realistic editing cadence of ~200 saves in an hour of
writing, tool time totals **200 × 0.15 s = 30 seconds, or 0.8% of the hour.** The warm loop is not the
bottleneck and cannot become one. **Further optimisation of the warm edit loop has no value to an
author**, and that is a more useful result than a faster number would have been.

---

## The value stream, measured

`TALIESIN_PYTHON` pointed at the real kernel venv (ipykernel 7.3.0). Every figure is wall clock on the
real document or its project.

| Step | Time | Classification |
|---|---|---|
| `check <file.tmd>` | **87 ms** | necessary non-value-adding |
| `render <file.tmd>` (no exec) | **86 ms** | necessary non-value-adding |
| **warm preview edit, prose** | **90 ms** | necessary non-value-adding |
| **warm preview edit, cell re-run** | **152 ms** | necessary non-value-adding |
| `check <site>` (19 pages) | **538 ms** | necessary non-value-adding |
| `build <site>`, freeze warm | **789-801 ms** | necessary non-value-adding |
| `build <site>`, **freeze cold, real kernel** | **3,981 ms** | **waste** (see item 131) |
| freeze per-edit rewrite, largest real page | **2.97 ms median**, 6.13 ms p95, 10.04 ms max (451,827 bytes) | necessary non-value-adding |

**The freeze rewrite measurement closes a gap AUDITS.md explicitly recorded as unmeasured**
("the probe's 200 ms poll floors the measurement"). Measured directly at file level rather than
through a poll: **2.97 ms median**, against a ~90 ms warm loop. It is **~3% of the loop**, not a
concern, and the question can be closed.

**The freeze cache earns its keep, quantified:** 3,981 ms cold against 789 ms warm on the same
project. **80% of a cold build is work the cache removes.**

---

## The waste list

Lean's question is where time goes that produces nothing the reader values. Four items, each traced to
a measurement rather than an intuition. **Three of the four were found by other rounds; the value
stream is what prices them.**

### 131. The cold-build cliff is the only measured waiting in the tool. (LOW, and probably correct)

3,981 ms against 789 ms warm. It is paid on a fresh clone, after a cache bust, and in any CI-shaped
context. It is **not** paid in the edit loop, which is the case that matters.

**Filed as LOW and probably not worth fixing.** Kernel *variable* state is never cached — that is the
design decision `exec.rs` states in its own docstring and the thing that makes the cache trustworthy —
so a cold start genuinely cannot skip work unless the whole document is unchanged. **The waste is
inherent to a correctness property the project should keep.** Recorded so the 3,981 ms is not
rediscovered and mistaken for a defect.

### 132. Defects in a deck are found by a viewer, not by the tool: the worst-priced waste in the stream. (already open as item 109; this round prices it)

Value-stream framing of R14's finding. In Lean terms a defect's cost scales with how late it is
found, and this is the latest possible: `check` says clean, `--strict` says clean, the preview shows no
diagnostic, and the defect surfaces **in front of an audience.**

Every other defect class in this tool is caught in the 90 ms loop or in `check`. Deck defects are
caught by a human, after publication. **That asymmetry is the argument for item 109's priority**, and
it is an argument no correctness framing produces.

### 133. A migrated document costs a triage pass the tool could have avoided. (already open as items 127/128; this round prices it)

R11 measured 457 problems on a real external book, of which **447 are two shapes the tool could
recognise**: 329 correct-by-convention code fences reported as spelling errors, and 118 links whose
correct target the page registry already knows.

**In value-stream terms this is rework on the author's first contact**: the author must read 457
diagnostics, determine that ~98% are the tool's vocabulary gap rather than their mistakes, and hand-fix
what remains. Wave 1's adoption round ranked anxiety above pull; this is anxiety with a stopwatch on
it.

### 134. `check <site>` costs 6× `check <file>` and the author has no reason to know which to run. (LOW)

538 ms against 87 ms. Both are fast enough that the time is not the issue; the issue is that the two
commands **answer different questions** (R14 measured that a site check silently skips decks and
drafts, and `check.rs:196` documents that `Scope::InSite` omits `validate_local_links`) and nothing
tells the author which one they just asked.

**No fix proposed here.** It is folded into item 109's deliverable: once a site check covers decks, the
two commands differ only in link scope, and that difference can be one line of output.

---

## What needs the author, and the exact questions

The slate says this round "needs the author's attention rather than CPU". Half of it does, and
guessing would produce a fiction. **These are the questions; each is answerable in one real writing
session with a stopwatch.**

1. **What fraction of an hour is spent writing prose versus fighting the tool?** The tool contributes
   0.8% (measured above). The remaining 99.2% splits between real authoring and everything else, and
   only the author knows the split.
2. **How long does getting one figure right actually take?** The slate names this explicitly. The
   measured cell re-run is 152 ms; the number that matters is how many re-runs a figure takes before
   it looks right, which is an authoring loop, not an execution loop. **If that number is large, it is
   the single biggest target in the whole product** — and it is invisible to every round run so far.
3. **How much time goes to citations and cross-references?** `cite.rs` and `xref.rs` are mature, but
   nobody has timed the author-side loop of adding a reference and getting it to resolve.
4. **How long does the deploy step take, and how often does it fail?** `publish` was not exercised in
   this round.
5. **How often does the author leave the editor?** Every context switch to a browser, a terminal or a
   file manager is classic value-stream waste, and click-to-source exists precisely to reduce it. Its
   effectiveness has never been observed.

**Recommendation: do not run this half as an audit.** It wants one instrumented real writing session,
which is a diary, not a round.

---

## Measured healthy

- **The warm loop is not a bottleneck and cannot become one** (0.8% of a writing hour).
- **Incrementality is real on a real document**: a prose edit on a 59-block page with 3 live cells
  produces one block op and zero kernel executions.
- **The author never waits on the kernel to see their own text** (90 ms first response regardless of
  whether a cell re-runs).
- **The freeze rewrite is ~3% of the loop**, closing a named unmeasured gap.
- **The freeze cache removes 80% of a cold build.**
- **First paint on a warm preview: 4 ms.**

---

## Not measured

- **Every author-side number**, per the five questions above. This round measured the tool's half of
  the stream and none of the human's.
- **A value-add ratio was deliberately not computed.** It requires total elapsed time, and any figure
  produced here would be the tool-time denominator dressed up as a lifecycle. **The slate asked for
  that ratio and this round declines to fabricate it.**
- **`publish` / deploy** was not run.
- **One document, one language.** No R-cell loop, no book-scale preview, no multi-hour session.
- **The editor half of the loop** (companion, LSP, click-to-source round trip) was not timed.

## Round bookkeeping

This round wrote only this file. Items 131-134 follow R11's 127-130. See
[R14](2026-07-28-deck-exemption-audit.md) on the 79-90 numbering collision between the two live
branches.

**Remaining slate:** R10 (demand and positioning), R13 (green software, optional), and R12
(real-device mobile), which needs the author's phone.
