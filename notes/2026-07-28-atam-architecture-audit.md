# R6 — ATAM: architecture tradeoff analysis

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Date:** 2026-07-28
**Round:** Wave 2 / R6 of the [audit slate](../docs/superpowers/specs/2026-07-27-audit-slate-design.md).
**Question.** Where does the architecture bend, and which of the three load-bearing goals gives way
first?

**Instrument.** The SEI ATAM protocol, compressed: state the driving quality attributes, build a
scenario set with a concrete stimulus and a **measurable** response, analyse each against the
architectural decisions that serve it, and classify the output into **risks**, **non-risks**,
**sensitivity points** and **tradeoff points**.

**The non-risk register is a first-class output, not filler.** Recording that a decision is safe
under its scenarios is what stops it being re-litigated, and this round's dominant result is that the
three load-bearing goals are architecturally sound. New items are numbered from **114**.

---

## The driving quality attributes

Three are stated in `CLAUDE.md` as load-bearing; three more are stated as invariants and behave as
quality attributes in every scenario below.

| # | Attribute | Stated where |
|---|---|---|
| QA1 | **Click-to-source** — the preview navigates back to the editor cursor | `CLAUDE.md`, goal 1 |
| QA2 | **Block-level incremental updates** — an edit re-renders a block, not a page | `CLAUDE.md`, goal 2 |
| QA3 | **No per-edit startup cost** — warm server + warm kernel | `CLAUDE.md`, goal 3 |
| QA4 | **Offline guarantee** — no CDN, no network egress | scope guard |
| QA5 | **Single editing surface** — the preview never writes back | `CLAUDE.md` + `[[single-editing-surface]]` |
| QA6 | **HTML-only** — one output target | scope guard |

---

## Scenario set and measured responses

Each row is stimulus / environment / measured response. A response with no number is marked as such
rather than asserted.

### QA1 — click-to-source

**S1. An author clicks any rendered element on a content page and expects their editor cursor.**
Measured on eight real documents (`taliesin render`, counting `data-block-id` against
`data-sourcepos="<positive line>:"`):

| Document | Blocks | Navigable | % |
|---|---|---|---|
| `corpus/tarn/concepts.tmd` | 12 | 11 | 91 |
| `corpus/tarn/filtering.tmd` | 20 | 18 | 90 |
| `corpus/tarn/api-query.tmd` | 13 | 12 | 92 |
| `docs/internals/execution.tmd` | 71 | 69 | 97 |
| `docs/internals/rendering.tmd` | 28 | 26 | 92 |
| `corpus/tech-blog/posts/em-algorithm/index.tmd` | 58 | 55 | 94 |
| `corpus/tech-blog/posts/fourier-transform/index.tmd` | 29 | 27 | 93 |
| `corpus/bayesian-website/index.tmd` | 99 | 96 | 96 |

**90-97% across the board, and the shortfall is correct.** The sourcepos-less blocks are
tool-generated chrome with no source line to navigate to: backlinks (`site/backlinks.rs:211`),
cite-this (`cite_this.rs:375`), the book TOC (`book_toc.rs:120/154`), rendered bibliography
(`cite/render.rs:152`) and listing cards (`site/mod.rs:1785/1802`). Each writes
`sourcepos: String::new()` deliberately.

**Classification: NON-RISK.** The invariant is enforced by `crates/core/tests/corpus.rs` over every
corpus document, and the one architectural decision that serves it (every block carries
`data-block-id` + `data-sourcepos`, emitted at one place) has no competing pressure anywhere in the
scenario set.

**S2. An author clicks content that arrived through `{{< include >}}`.** Included blocks additionally
carry `data-source-file`, and `includes.rs` maintains a per-file source map. **NON-RISK**, pinned by
`include_relative_base.rs` and `include_root_parity.rs`.

**S3. An author clicks a figure produced by a code cell.** The output block's id derives from its
cell's id (`exec.rs`, module docstring) so it swaps in place, and the cell's own sourcepos is what a
click resolves to. This is correct and is the only sensible answer: a matplotlib PNG has no source
line of its own. **NON-RISK**, and it is why `page_static_diagnostics` deliberately runs *before*
execution (`check.rs:192-194`).

**The permanent gap is recorded elsewhere and is not re-filed here:** the click-to-source harness
stops at the relay and cannot observe whether the editor lands the cursor
(`[[qmd-purge-completed]]`). That is a *test* gap, not an architecture risk.

### QA2 — block-level incremental updates

**S4. An author edits one word in one paragraph of a 99-block page.** The diff
(`crates/core/src/diff.rs`) emits one `Update` op. **NON-RISK.**

**S5. An author edits one word on a slide.** A deck diffs the *slide-transformed* projection rather
than raw blocks (`serve/mod.rs:1614`), so a within-slide edit still ships one op.
**NON-RISK, and a well-made decision** — the naive version would have shipped raw HTML that stripped
the block's `.fragment` class.

**S6. An author inserts a `---` slide boundary, or retitles a slide.** The whole deck re-mounts
(`serve/mod.rs:1605`, `:1237`). QA2 is **deliberately abandoned** here.

**Classification: TRADEOFF POINT, correctly resolved.** `<section>`-grouped slides cannot be
restructured by flat block ops, and the comment says so. The cost is bounded by deck size (56 blocks
on `corpus/deck.tmd`) and the engine preserves the current slide and overview state across the swap,
so the user-visible cost is near zero. Recorded so it is not mistaken for a defect.

**S7. An author edits a cell marked `#| cache: false`.** `plan()` (`exec.rs:1000-1030`) extends the
re-run range **to the end of the document**, because a non-deterministic cell may leave different
kernel state behind and the cumulative hash cannot see that.

**Classification: TRADEOFF POINT — correctness bought with incrementality, correctly priced.** The
comment states the reasoning and `plan_cache_false_cell_always_reruns` pins it. The cost is bounded
in practice: the largest cell count in the corpus is **11**
(`corpus/bayesian-website/subsections/_data-description.tmd`), and the branch is inert in a document
with no `cache: false` cell (`first_uncacheable == len`).

**S8. A document has 200 cells and the author edits cell 1.** Not measured, and not measurable
against the current corpus, whose maximum is 11. The positional cascade means the response is linear
in downstream cell count with no upper bound. **This is the one QA2 scenario with no evidence**, and
it is where the `{js}` side already has the better answer: `diagnostics/reactive.rs` gives `{js}`
cells a real dependency graph with cycle detection while Python and R get a positional cascade. See
item 116.

### QA3 — no per-edit startup cost

**S9. An author previews a 14-chapter prose book and visits chapters 1 through 7.**
`MAX_WARM_PAGES = 6` (`serve_site/exec_pool.rs:14`), so the 7th visit evicts the 1st.

**Measured, and the scenario resolves benignly** — for a reason worth recording because it is
non-obvious. AP3-1's bypass lane routes cell-free pages to a lane that owns no pool
(`serve_site/mod.rs:1210-1213`), and `ExecPool::make` constructs an `Executor` **without** booting a
kernel (`Kernel::start_with_retry` is reached only from the run path, `exec.rs:906`). So a prose book
consumes no kernels at all. Measured cell counts: `corpus/tarn` **0 of 14** pages have a code cell;
`docs/internals` 2 of 15; `docs/guide` 4 of 22.

**Classification: NON-RISK**, with one caveat that is its own item — see 114. `MAX_WARM_PAGES` remains
the project's one standing freeze and this round proposes no change to it; the scenario is recorded
because the freeze has never had a stated scenario, only a stated constant.

**S10. An author edits across 7+ pages that each *do* run cells.** Eviction is real here and the 7th
page pays a cold kernel start on its next edit. Bounded by design (~80-150 MB per resident kernel),
and the shared warm pool blunts the first edit on a fresh page. **SENSITIVITY POINT, known, frozen,
and correctly reasoned in its own docstring.**

**S11. A cold `build` of an unchanged document.** Every cell hits `_freeze/` and the kernel never
boots. Measured build throughput on real projects:

| Project | Pages | Wall clock |
|---|---|---|
| `corpus/tarn` | 14 | 175 ms |
| `docs/internals` | 15 | 265 ms |
| `docs/guide` | 22 | 482 ms |
| `corpus/tech-blog` | 19 | 600 ms |

12-32 ms per page including assets and a search index. **NON-RISK.**

### QA4 — offline guarantee

**S12. A built page is opened with no network.** Everything is inlined or bundled under `_assets/`.
The one live CDN string in the binary (`render/mod.rs:1532`) is a never-reached fallback, already
established by Wave 1's pre-mortem. **NON-RISK in behaviour; the *coverage* residual is Wave 1's item
86 and is not re-filed here.**

### QA5 — single editing surface

**S13. Any preview interaction attempts to change the source.** There is no write path: the preview's
only channel back is `openSource`, which navigates. This was tested architecturally once, by removing
the drag-to-reorder feature that violated it. **NON-RISK, and the strongest invariant in the tree**
because it is enforced by absence rather than by a check.

### QA6 — HTML-only

**S14. A user asks for PDF.** `--bare` already refuses a deck outright (`build.rs:620`) with a reason.
The attribute holds. **NON-RISK architecturally**; its *adoption* cost is Wave 1's item 94, which
measured that at most 8.59% of corpus lines carry any non-CommonMark construct, so the exit path is
better than it reads.

---

## The register

### Risks

**R-1. QA2 has no evidence above 11 cells, and the two languages that need it most have the weaker
machinery.** (→ item 116.) Every other quality attribute in this round has a number attached at
realistic scale. This one does not, and the architecture (positional cascade) is the kind that
degrades linearly and silently.

**R-2. Nothing in the architecture defends QA1 against a *new* block source.** Six call sites write
`sourcepos: String::new()` today, all correctly. There is no rule, test or type that makes the next
generated-block author think about it, and `corpus.rs` checks the blocks that *exist*, not the
producers. (→ item 115.)

### Non-risks (do not re-litigate)

- QA1 at 90-97% on every real document, shortfall entirely generated chrome (S1, S2, S3).
- QA2 for ordinary prose edits and for within-slide deck edits (S4, S5).
- QA3 for prose books of any length: cell-free pages consume no kernel (S9).
- QA3 for cold builds of unchanged documents: 12-32 ms/page (S11).
- QA4 offline behaviour (S12); QA5 by construction (S13); QA6 (S14).

### Sensitivity points

The durable artefact: lines where a small change causes a large quality impact.

| Where | Attribute at risk | Why |
|---|---|---|
| `serve_site/exec_pool.rs:14` + the LRU order | QA3 | **The one standing freeze.** Already known; now it has a stated scenario (S10) rather than only a constant |
| `crates/core/src/diff.rs` block-id derivation | QA1 **and** QA2 | Both goals key off one content-hash id. A change to id derivation silently breaks live-state preservation and source mapping together |
| `serve/mod.rs:1605` `deck_op_is_structural` | QA2 | Widening it converts ordinary deck edits into full re-mounts; narrowing it ships ops a `<section>` layout cannot apply |
| `exec.rs:1015` `shared = lcp.min(first_uncacheable)` | QA3 **and** correctness | One `min` is the entire boundary between "warm reuse" and "stale output" |
| `serve_site/mod.rs:104` `unwrap_or(false)` | QA3 | The lane-routing default. `true` here would send an unbuilt page with cells to the lane that cannot execute it |

### Tradeoff points

| Decision | Helps | Hurts | Verdict |
|---|---|---|---|
| Deck re-mounts on structural change (S6) | correctness of `<section>` layout | QA2 | correct; bounded and state-preserving |
| `cache: false` extends the run to document end (S7) | correctness | QA3 | correct; inert without such a cell |
| Positional cascade instead of a DAG | simplicity, notebook semantics | QA3 at scale | **unresolved — the only one that is** (item 116) |
| Bypass lane for cell-free pages | QA3 | one wasted render when a page gains its first cell | correct; the cost is stated in its own docstring |

---

## Items

### 114. The eviction log reports a kernel that was never booted. (LOW)

`exec_pool.rs:88` logs `"evicted warm kernel for {evicted}"` unconditionally on eviction. But an
`Executor` boots no kernel until a cell actually runs (`exec.rs:906`), and an unbuilt page routes to
the exec lane by default (`serve_site/mod.rs:104`, `unwrap_or(false)`) — so **every** page consumes a
pool slot on its first build, cell-free or not.

**Consequence.** Previewing a prose book with more than six chapters prints "evicted warm kernel for
…" for pages that never had one. Measured substrate: `corpus/tarn` is 14 chapters with **zero** code
cells, so a normal browse of that book is enough to produce the false line.

This project treats diagnostic honesty as a defect class in its own right (DIAG-1, AP11-1 were both
wording findings), which is why a LOW cosmetic issue is filed rather than dropped.

**Fix.** Log only when the dropped executor actually held a kernel, or reword to "released page
executor". **Refuted if** an `Executor` boots a kernel at construction.

**Not measured:** the log line was derived from source, not observed in a running preview.

### 115. Nothing makes the next author of a generated block think about click-to-source. (LOW, structural)

Six sites write `sourcepos: String::new()` (`backlinks.rs:211`, `cite_this.rs:375`,
`book_toc.rs:120`, `:154`, `cite/render.rs:152`, `site/mod.rs:1785`, `:1802`, plus three in
`render/mod.rs`). All are correct: generated chrome has no source line.

The risk is that this is a convention with no enforcement. `corpus.rs` asserts the invariant over
rendered corpus output, which catches a *regression on existing content* but not a new generated
block that quietly opts out — and QA1 is the goal with the least automated coverage in the project
(its end-to-end path is permanently manual).

**Proposal, deliberately small:** a doc comment on the `Block` type naming the two legitimate reasons
to write an empty sourcepos, so the choice is made rather than copied. Not a new type, not a lint —
this is a LOW-severity structural note, and the "prefer a better default over a knob" rule applies to
process too.

### 116. The positional cascade is the one unresolved tradeoff, and the tree already contains the better answer for one language. (MEDIUM, positioning before engineering)

`exec.rs` (module docstring) runs Python and R cells as a positional cascade: editing cell *i* re-runs
*i* and everything after it. `crates/core/src/diagnostics/reactive.rs` gives `{js}` cells a real
dependency graph with cycle detection.

**Measured:** no document in the corpus exceeds 11 cells, so the cascade's cost has never been felt
here and there is **no evidence either way** about its behaviour at 50 or 200 cells. That absence is
the finding.

**Do not treat this as a build item.** The slate flags the same thread under R10 as a *positioning*
question first: cumulative-hash reproducibility may be the stronger claim to tell properly rather than
a DAG to build. This item exists so R10 arrives with the architectural half already measured:

- the cascade is correct and cheap at corpus scale;
- kernel *variable* state is never cached, which is precisely what makes a per-cell DAG hard here and
  is stated in `exec.rs`'s own docstring;
- so a Python DAG is not a small change, and "the cascade is a feature" is a defensible position that
  has never been written down.

**Refuted if** a corpus or dogfood document exceeds ~30 cells, which would make this measurable
rather than speculative.

---

## Not measured

- **No scenario was run against a live preview.** Every response above is from source, from
  `taliesin render`/`build`, or from the CLI. S9 and S10 in particular deserve a real preview session.
- **Cold-build RSS at 400+ pages** stays unmeasured, as AUDITS.md already records. `corpus/tarn` (14
  pages) is the largest fixture and must not be grown.
- **Multi-hour preview drift** (as distinct from execution volume) stays unmeasured.
- **`notify` at extreme directory counts** stays unmeasured.

## Round bookkeeping

This round wrote only this file. `backlog.md`, `AUDITS.md` and `LESSONS.md` are the coordinating
session's to update. Item numbers 114-116 follow R14's 109-113; see that round's note on the 79-90
collision between the two live branches.
