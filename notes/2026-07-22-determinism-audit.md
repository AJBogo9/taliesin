# Audit: determinism / reproducibility (perspective AP8)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

Date: 2026-07-22. Perspective: AP8 from the backlog "Audit perspectives" section
(determinism / reproducibility). Run as a single-perspective session alongside two live
sessions (feature on DX17b, audit on AP2 fuzzing, both in isolated worktrees), so it
touches no source and builds nothing from the working tree. This pass covered BOTH the
"read half" (a static hunt for non-determinism sources) and the empirical rebuild-twice
check the backlog flagged as stateful, using the frozen `taliesin-stable` binary
(`/home/bogo/.local/bin/taliesin-stable`, Jul 7). No `cargo build`, no contention.

> **Cross-check (added after the fact):** the audit session independently ran AP8 at the same time
> (commit `58db11d`, `notes/2026-07-22-ap8-determinism-audit.md`), a concurrent-choice collision
> neither session foresaw. Their round is the fuller record: it corroborates this static conclusion
> (they got 121 docs x3 processes + 9 site builds byte-identical) AND additionally ran the KERNEL
> path, which this pass deliberately skipped, catching one real defect this doc missed: **AP8-1**, a
> `{python}`/`{r}` cell's stderr splices the non-deterministic `/tmp/ipykernel_<PID>/…py` path into
> built HTML (root cause `kernel.rs:994`, `Output::Stream` arm). That is both a reproducibility break
> and the local-absolute-path leak my AP12 offline round explicitly deferred. Treat their doc as the
> primary AP8 record; DET-1 below (a broader regression guard) is complementary and not in theirs.
> Both items are merged into backlog Open-work item 15.

## Why this perspective

`data-block-id` is a content hash and the incremental block-diff assumes stable output, so
any non-determinism reaching the emitted bytes (HashMap iteration order, unsorted
`read_dir`, time or randomness in output, parallel results collected in completion order)
would silently break incremental updates and produce noisy diffs when a built site is
re-generated or committed. Nobody had verified the tool is deterministic; this pass proves
it, and checks that the property is guarded against future regression.

## Executive summary

Determinism holds, and it holds BY CONSTRUCTION, not by luck. Rendering the same document
and building the same multi-page site twice in SEPARATE processes (so each run gets a fresh
HashMap seed) produced byte-identical output every time, and the code paths that could
introduce order-dependence are each deliberately made stable. The output is reproducible
cross-machine as well, because every order-sensitive step keys on a path, anchor, or date,
not on filesystem or thread-completion order.

The one gap is not a defect in behavior but a hole in the safety net: this carefully
maintained property has no explicit end-to-end regression guard, so a future unsorted
HashMap-to-output (especially a new site-level artifact like the search or hover index)
could reintroduce non-determinism with nothing to catch it.

## Evidence: byte-identical across fresh seeds

Separate-process runs (each a fresh HashMap `RandomState` seed):

```
render native-tmd.tmd  x2  -> IDENTICAL
render highlight.tmd   x2  -> IDENTICAL
render deck.tmd        x2  -> IDENTICAL
build corpus/bayesian-website (multi-page: nav, listings, search index, xref
      registry, hover index)  x2  ->  diff -rq: IDENTICAL
```

If any order-sensitive output depended on HashMap iteration, the multi-page site (which has
multiple pages, cross-references, and a generated search/hover index) would have diverged
between the two seeds. It did not.

## Why it holds (deterministic by construction)

- **Page discovery is sorted.** `collect_pages` (`site/discovery.rs:110`) pushes entries in
  raw `read_dir` order, but its caller `website_pages` immediately does `inputs.sort()`
  (`discovery.rs:17`), and `pages.sort_by(rel)` (`discovery.rs:68`). Filesystem order is
  discarded, so page/nav order is stable cross-machine.
- **Listings are sorted with a tiebreak.** `items.sort_by(a.date.cmp(b.date).then(a.rel.cmp(b.rel)))`
  (`site/mod.rs:1307`), so equal dates still order deterministically.
- **The hover index is explicitly sorted.** `entries.sort_by(anchor)` at `site/mod.rs:1165`
  carries the comment "Stable order so the index is deterministic across builds," even
  though its `by_page` grouping (`site/mod.rs:1133`) walks a `HashMap`. The developer already
  reasoned about this.
- **Parallel page builds reassemble by index, not completion order.** The build spawns one
  task per page returning `(idx, outcome)` and writes `outcomes[idx] = Some(outcome)`
  (`build.rs:1502-1509`), so tokio scheduling never affects the result.
- **`xref_numbers` iteration is a keyed insert** (`site/mod.rs:1040`), order-independent, and
  its consumers emit sorted.
- **No time / randomness / pid reaches output.** Every `SystemTime::now()` / `process::id()`
  in the core is a temp-directory scratch name (`includes.rs`, `site/config`, isolated file
  I/O), never a rendered byte. The block-id hash is FNV-1a, pinned deterministic
  (`hash.rs`).

## Finding

### DET-1 (low): no explicit end-to-end determinism regression guard

The reproducibility above is real but is held together by individual `.sort()` calls spread
across `discovery.rs`, `site/mod.rs`, and the build. The only tests near this property are
indirect: single-run body-HTML snapshots (`crates/core/tests/body_html_snapshots.rs`, which
would merely FLAKE if output were seed-dependent, and only cover snapshotted body HTML, not
the search index / hover index / sitemap / full page) and the FNV-1a determinism unit test
(`hash.rs`), plus a `twinned_corpus_sources_stay_byte_identical` test (`corpus.rs:1260`)
that checks duplicated corpus SOURCES, not render output. Nothing asserts the direct
property: that building the same input twice yields identical bytes. So if someone adds a
new site-level output that iterates a `HashMap` (or a `HashSet`, or an unsorted `read_dir`)
straight into the emitted bytes and forgets the sort, determinism regresses silently and no
gate fails.

Recommendation: add one regression guard that builds a representative multi-page site
(one with cross-references, a listing, and the search + hover index, e.g.
`corpus/bayesian-website` or a purpose-built fixture) TWICE into two output dirs and asserts
they are byte-identical (`diff -rq` equivalent). Running it twice in-process is weaker
(a single process can share a seed); the strong form re-seeds, so either use two child
`build` invocations or force a fresh `RandomState` between builds. This pins the property
the code comments already care about, in the same spirit as the corpus-invariant guards.
Size: S. File: a new test under `crates/server/tests/` (the build lives in the server bin
crate).

## Verified deterministic (do not re-audit)

Single-doc render and full multi-page site build are byte-identical across fresh HashMap
seeds; page discovery, listings, and the hover index are explicitly sorted; parallel page
builds reassemble by index; no time/random/pid reaches output; block-id hashing is pinned
deterministic. The load-bearing block-diff / content-hash invariant is safe from
non-determinism, and builds are reproducible across machines.

## Method notes for the next AP8-style run

- The decisive test is cheap: `taliesin-stable render <doc>` (or `build <site> --out`) twice
  in separate processes and `diff`. Separate processes are essential, because a fresh
  HashMap seed per process is what exposes iteration-order dependence; two renders inside one
  process can share a seed and hide it.
- If a divergence ever appears, the likely culprits, in order: a new `HashMap`/`HashSet`
  iterated into output without a sort, an unsorted `read_dir`, or a parallel collection that
  keeps completion order instead of re-indexing.
