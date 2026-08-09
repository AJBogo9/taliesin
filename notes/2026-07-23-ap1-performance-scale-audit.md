# Audit: performance & scale (perspective AP1)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

Date: 2026-07-23. Perspective: AP1 from the backlog "Audit perspectives" section
(performance & scale), Tier 1 "genuinely untouched, highest expected yield". Run as a
single-perspective session with a clean tree at `f49b1e0` (== `origin/main`), no worktrees,
no other preview/dev port live, so it touches no source and commits nothing. Measurement
used a freshly-built `--release` binary (`target/release/taliesin`, this tree) plus a
throwaway harness that path-depends on `taliesin-core` and times the per-keystroke
site-wide passes directly (in `scratchpad/perfbench/`, outside the repo).

No perf note existed in `notes/` before this: every prior audit used small corpus docs, so
nobody had measured where the warm incremental loop (the moat) degrades. This pass measures
it.

## Why this perspective

The load-bearing promise is "block-level incremental updates with no per-edit startup cost
(warm server + kernel)". That promise is only as good as the *server-side* rebuild latency:
the client applies a minimal DOM diff, but the server still has to produce the new block
list on every save, and any superlinear step there (or any whole-document/whole-site work
hidden in the rebuild) turns the fast loop slow exactly when a document grows. The two
never-measured questions: (1) does anything go quadratic in block count or page count, and
(2) what does one keystroke-save actually cost as a doc/site scales.

## Executive summary

**Performance is healthy and free of quadratic blowups on every path measured.** Single-doc
render, multi-page site build, and the block diff all scale linearly or sublinearly. An
8000-block / 20k-line single document renders in 647 ms; a 400-page site builds in 874 ms
(parallel across cores); the block diff is O(n log n) by construction and measured linear.
Nothing found here is a correctness bug.

**The one real degradation is in the warm-preview moat, and it is a size-dependent tax
mistaken for a constant.** Every `.tmd` save in a *site/book* preview runs **two independent
full-site sequential render passes** — `refresh_xrefs()` (once per save, gated on the changed
file, not on open tabs) and `validate_cross_page_links()` (inside `build_page`, once per open
tab) — before the edited page's own incremental diff is computed. Each pass renders *every
page of the whole site* from disk on a single thread. Both cost scales linearly with
(pages × blocks-per-page); both are annotated in source as "~27 ms" measured on a 20-page
book, treating a growing cost as fixed. This is the first place the incremental loop degrades
as a project grows, and the double pass is low-hanging fruit.

## Evidence

### 1. Single-document render scales sublinearly (no quadratic) — POSITIVE

`taliesin build doc_N.tmd out.html`, `TALIESIN_NO_CACHE=1`, best of 3, synthetic docs of N
mixed blocks (paragraphs, headings, `python` code, `$…$` math, lists). Fixed
startup+trivial-render overhead measured separately at **4 ms**, so these times are almost
entirely real render work.

| blocks | best ms | ms/block | ratio vs ½ size |
|-------:|--------:|---------:|----------------:|
|  1000  |   111   |  0.111   |    —    |
|  2000  |   185   |  0.093   |  1.67×  |
|  4000  |   338   |  0.085   |  1.83×  |
|  8000  |   647   |  0.081   |  1.91×  |

Every doubling costs **< 2.0×** and ms/block *falls* as N grows (fixed costs amortizing), so
render is sublinear-to-linear, not quadratic. A realistic large single file (a long chapter,
~1–2k blocks) rebuilds in ~110–185 ms; only a pathological multi-thousand-block single file
approaches half a second. Since the warm single-doc rebuild path (`serve/mod.rs::rebuild`)
re-renders the whole document (comrak has no incremental parse) and startup is only 4 ms,
this table also characterizes the single-doc keystroke→ops latency.

### 2. Multi-page site build scales linearly (no quadratic) — POSITIVE

`taliesin build book_N/` (a book with N cross-referenced chapters, each with labeled
sections + a figure + `@sec-`/`@fig-` refs into 5 other chapters), best of 3:

| chapters | best ms | ms/chapter |
|---------:|--------:|-----------:|
|    50    |   185   |    3.70    |
|   100    |   267   |    2.67    |
|   200    |   438   |    2.19    |
|   400    |   874   |    2.19    |

Per-chapter cost falls then flattens at ~2.19 ms; the 400/200 ratio is exactly 2.02× —
linear. The build parallelizes page renders across cores (`tokio::JoinSet` + a
`Semaphore(build_cap)`), so wall-time is well under the sequential sum.

### 3. The per-keystroke site-wide tax (the finding) — measured directly

The scratch harness times the two functions the warm loop calls per save, on the same
synthetic books, best of 3:

| pages | `refresh_xrefs` | `validate_cross_page_links` | per-keystroke (sum) | discover (boot, 1×) |
|------:|----------------:|----------------------------:|--------------------:|--------------------:|
|   50  |      9.0 ms     |           9.4 ms            |       18.4 ms       |       29.9 ms       |
|  100  |     18.4 ms     |          17.7 ms            |       36.1 ms       |       58.5 ms       |
|  200  |     38.7 ms     |          37.0 ms            |       75.7 ms       |      121.3 ms       |
|  400  |     78.2 ms     |          74.9 ms            |      153.1 ms       |      249.3 ms       |

Perfectly linear in page count (each doubling ≈2.0×) — **no quadratic**. But these synthetic
pages are *light* (~6 blocks each). The real forward-facing brand site tells the true story:

| real site        | pages | `refresh_xrefs` | `validate_cross_page_links` | per-keystroke |
|------------------|------:|----------------:|----------------------------:|--------------:|
| `corpus/tech-blog` |  17  |     30.4 ms     |          30.2 ms            |    **60.6 ms** |
| `corpus/demo-book` |   5  |      0.5 ms     |           0.5 ms            |     1.0 ms    |

`tech-blog` is content-rich (~1.8 ms/page/pass vs ~0.19 ms/page/pass synthetic), so it
already pays **~60 ms of pure whole-site re-render on every keystroke-save**, on top of the
edited page's own render/exec/diff. Extrapolating that real per-page weight: a **100**-page
content-rich book ≈ 100 × 1.8 × 2 ≈ **~360 ms/keystroke**; a **200**-page book ≈ **~700
ms/keystroke** — past the ~100 ms "feels instant" line and into "feels laggy". That is the
moat degrading, and it degrades *linearly*, so it is invisible until a project is large.

Both source sites annotate the cost as fixed:
- `serve_site/mod.rs:1430` (before `refresh_xrefs`): "It costs 27ms on the largest real book
  (`docs/guide`, 20 pages)".
- `serve_site/mod.rs:1107` (before `cross_page_diagnostics`): "`validate_cross_page_links`
  re-renders the whole site (~27 ms)".

Both are true only at 20 pages; the measured 17-page tech-blog is already ~30 ms *per pass*,
so the two run back-to-back at ~60 ms today.

### 4. Warm-preview memory is bounded — POSITIVE (bounded probe)

`taliesin preview book_200 4399`, no browser client, 30 real appends to one chapter (each
triggers a whole-site `refresh_xrefs`), 150 ms apart. RSS: **15.4 MB at boot → 18.1 MB after
30 edits (+2.7 MB)**, and the edited file itself grew by 60 lines over the run. No leak
signature in the probe. (Multi-hour drift and kernel RSS drift not chased — see Residuals.)

## Finding

### PERF-1 (medium): the warm loop does two redundant full-site render passes per keystroke

**Root cause.** On every `.tmd` save in a site/book preview, `rebuild_project`
(`serve_site/mod.rs:1441`) runs `refresh_xrefs()` — a full sequential render of every page to
rebuild the xref-number registry — and then, for each open tab, `build_page` calls
`cross_page_diagnostics` → `validate_cross_page_links()` (`site/mod.rs:745`), which renders
*every page again* to collect outgoing links and id registries. The two passes are
independent (`refresh_xrefs` builds `xref_targets`/`backlinks`; `validate_cross_page_links`
builds a per-page id set + link list) but each pays a *separate* whole-site render on the
same save. `validate_cross_page_links` additionally computes warnings for all N pages and
then `cross_page_diagnostics` throws away all but the current page's
(`preview_diag.rs:47`, `.filter(|(rel, _)| rel == page_rel)`).

**Why it matters (and why it isn't a bug).** It is not incorrect — DX1 (2026-07-18)
explicitly judged a debounced whole-site re-derive "fine" at ~27 ms and ruled incrementalizing
"unnecessary". That call was right *at corpus size*. The finding is that the cost is
size-dependent, the comments enshrine it as constant, it is *doubled* (two passes, not one),
and it is the single thing that turns the incremental loop non-incremental as a book grows —
exactly the moat AP1 was scoped to stress. It is silent because it scales linearly: no cliff,
just a slow slide as pages accumulate.

**Build-ready fixes (cheapest first):**
- **PERF-1a (halve it): share one whole-site render across the two passes within a single
  rebuild.** Both need a per-page render of the site; running them off one render pass (or one
  cached id/anchor+link registry produced once per rebuild) removes the second full traversal.
  `refresh_xrefs` already produces the anchor/number registry; `validate_cross_page_links`
  mostly needs each page's id set + outgoing links, which the same render can capture. Target:
  one pass per save instead of two.
- **PERF-1b (scope the discard): `validate_cross_page_links` renders all pages to keep only
  the current page's warnings.** The *other* pages are rendered only for their id registries
  (to resolve THIS page's outgoing links). If PERF-1a shares a registry, the second render
  disappears; if not, at minimum the warning computation can early-return the current page's
  links resolved against the shared registry rather than validating every page's links.
- **PERF-1c (only if a real >100-page book appears): debounce/coalesce.** A burst of saves
  (autosave, format-on-save) currently pays the tax per event; a short trailing debounce on
  the whole-site passes (keeping the edited page's own render immediate) caps it. Lowest
  priority — demand-driven, and the perfect-default lens says don't add machinery until a real
  large book exists.

Fix belongs entirely in `serve_site` + `site::{refresh_xrefs, validate_cross_page_links}`;
touches neither the block model, the diff, nor the Do-NOT-touch `exec_pool` LRU. A corpus pin
is awkward (the corpus has no 100-page book and shouldn't grow one just for this); verify by
the same harness pattern used here (time the passes before/after on a generated large book)
plus the existing `serve_site` behavior tests staying green.

## Verified healthy (do not re-audit)

- **Block diff** (`crates/core/src/diff.rs`): LCS→LIS via patience sorting, O(n log n) time /
  O(n) space **by construction** — the source comment explicitly rejects the textbook O(m·n)
  DP ("would allocate tens of MB on every keystroke-save once a document reaches a few
  thousand blocks"). Not a scaling risk.
- **Single-doc render**: sublinear per block (§1). No quadratic in comrak parse, block-model
  build, server-side highlight, or KaTeX render at 8000 blocks.
- **Site cold build**: linear, parallel across cores (§2). `build_site_async` shares `Site`
  via `Arc` (per-page `site.clone()` is a refcount bump, not a deep clone) and reassembles by
  index.
- **Boot/discover**: linear, one-time (~249 ms at 400 pages; 85 ms at tech-blog's 17).

## False leads (refuted — recorded per "trust the symptom, re-derive the cause")

- **"`site.clone()` inside the per-page build loop is O(pages²)"** — REFUTED. `site` is
  `Arc<Site>` (confirmed by `Arc::try_unwrap(site)` at `build.rs:1558`); the clone is a cheap
  refcount bump.
- **"The hover/xref index build is O(pages×refs) quadratic"** — REFUTED. The per-keystroke
  site passes measure perfectly linear in page count through 400 pages (§3); the cross-ref
  work is a keyed HashMap insert + lookup, not a nested scan.
- **"Single-doc render goes quadratic on very large docs"** — REFUTED. ms/block *falls* from
  0.111 to 0.081 across 1k→8k blocks (§1).

## Residuals not chased (for a future AP1-style pass)

- **Kernel RSS drift** over many `{python}`/`{r}` cell executions in a long warm session
  (needs a live kernel + a scripted execute-many loop; the warm kernel is reused across
  edits, so a per-execution leak would compound). Not measured here.
- **Multi-hour warm-preview RSS**: only a 30-edit bounded probe was run (§4); slow drift over
  hours is untested.
- **`notify` watcher at extreme directory counts**: the code already prunes
  `node_modules`/`.git` from the watch set and registers non-recursive descriptors; not
  stress-tested against a 10k-subdirectory tree.
- **Cold-build RSS peak** at 400+ pages built 16-wide (each page holds a rendered doc in
  flight): not sampled; the parallel `Semaphore(build_cap)` bounds concurrency, so peak is
  ~build_cap docs, but the number is unmeasured.

## Method notes for the next AP1-style run

- Startup overhead is only **4 ms** for a release build, so a cold `taliesin build` closely
  proxies a warm single-doc rebuild — you do not need to drive the websocket to characterize
  single-doc keystroke latency.
- For the *site* warm loop, drive the functions directly: a throwaway crate that
  path-depends on `taliesin-core` and times `Site::discover` / `refresh_xrefs` /
  `validate_cross_page_links` is faster and more precise than driving a browser, and the two
  functions are exactly what `serve_site` calls per save (confirmed by reading `build_page` +
  `rebuild_project`). Harness kept at `scratchpad/perfbench/` this session.
- Use content-rich real corpus sites (`tech-blog`), not only synthetic light pages: the
  per-page render weight differs ~10×, and the finding only bites on real content.
- `preview` writes to `_book/` for a book and `_site/` for a website — glob the right one
  when counting written pages.
