# Audit: cache-correctness / adversarial freeze (perspective AP4)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

Date: 2026-07-22. Perspective: AP4 from the backlog "Audit perspectives" section
(cache-correctness / adversarial freeze). Run as a single-perspective session on
`main` (`a121df2`), alongside a wrapping-up feature session in an isolated worktree
(`worktree-corpus-demand-probe`), so the two share no source and no `_freeze/` dir.
This pass covered BOTH halves: the "read half" (enumerate every input the cumulative
key folds in, then find a change the key can't see) AND the empirical half (construct
that change and prove a stale hit against the `target/debug/taliesin` binary + a real
`ipykernel` 7.3.0 kernel, in a throwaway scratchpad dir so the repo's caches are untouched).

## Why this perspective

The `_freeze/` cache is the load-bearing promise of the whole tool: *"a change to a
cell or anything upstream busts it and everything downstream, with no stale hits and
nothing to clear by hand"* (CLAUDE.md), and in code, *"a stale hit is impossible... This
is the **lone** by-design stale-hit path"* (`freeze.rs`, of the installed-packages axis).
One stale hit is a credibility bug for the core design. Nobody had tried to break the
promise; this pass does.

## Executive summary

The freeze cache is well-built and the cumulative-hash design is sound. The key
(`hash(interp -> code0 -> ... -> codei)`) correctly busts on cell code, any upstream
code, and interpreter identity; every output-shaping cell option is either folded into
the key (`fig-export`) or re-applied at render *after* the cache (`figure`/`table`
captions, `include`), so no cell option is a stale-hit vector; `eval`/`output`/`warning`
are parsed-and-ignored, so they can't desync output either; the on-disk write is atomic
(temp + rename), and a corrupt or version-mismatched file is tolerated by starting empty.
The "no stale hits" promise holds on every axis the key is *supposed* to see.

But the promise is worded as near-absolute ("**lone** by-design stale-hit path =
packages"), and that overclaims. There is a **class** of inputs the key structurally
cannot see, and one member of it is a genuine, reproducible **stale hit on a cold
`build`** that the tool's own docs actively walk a user into:

- **AP4-1 (MEDIUM, verified) — FIXED 2026-07-22 (see "Update" below):** a cacheable cell
  **downstream of a `#| cache: false` cell** restored a stale output, even on a cold
  build. `cache: false` re-runs the marked cell with fresh (nondeterministic) kernel
  state, but its dependent's cumulative key is unchanged, so `plan()` restored the
  dependent from `_freeze/`. Reproduced: the `cache: false` cell printed `A: 890903` while
  its dependent printed the stale `B: 859248` for the *same variable*, in one rendered
  document, on a cold build.
- **AP4-2 (LOW, mostly doc):** the key can't see *any* out-of-band input a cell reads
  (data files, env vars, network, wall-clock), not just packages. Same mechanism as
  AP4-1; the docs' own `include: false` example reads `pd.read_csv("data.csv")` without
  `cache: false`, so its downstream users go stale when `data.csv` changes.
- **AP4-3 (LOW, verified):** `is_uncacheable` matches the output-truncation marker as a
  bare substring, so a cell that merely *prints* that text is never cached (false
  "uncacheable"; re-runs every build). Safe direction, but inconsistent with the
  deliberately class-scoped `tali-error` check right beside it.
- **AP4-4 (LOW, AP3-adjacent):** the freeze save writes to a shared `<page>.json.tmp`
  name (not pid/uuid-unique), so two processes building the *same* page concurrently can
  race the temp write + rename. Not a stale hit (a corrupt read starts empty), but a lost
  cache generation. Rare (per-page execution is serialized); flagged for AP3.

Also noted (very low, no item): `probe_interp_id` memoizes the `--version` answer per
`(lang, path)` for the whole process, so an in-place interpreter upgrade at the *same
path* during a long-running preview isn't seen (keys don't move) until a restart. A narrow
extension of the documented package gap; "Restart kernel" / restart resolves it.

## AP4-1: `cache: false` does not propagate to dependents (stale hit on cold build)

### The mechanism

`plan()` (`exec.rs`) computes `first_uncacheable` = the index of the first
`#| cache: false` cell, and uses it only to **cap the warm prefix** (`shared =
lcp.min(first_uncacheable)`). It does **not** cap the disk-tail restore. So on a run
where a `cache: false` cell sits before a cacheable, disk-cached cell:

```
cells:   [ A (#| cache: false) , B (cacheable, on disk) ]
plan:    shared   = 0        (A is uncacheable, excluded from the warm prefix)
         run_end  = 1        (A is "unknown" -> runs; B is "known" -> not extended)
result:  run [0,1) = {A}     A re-executes (fresh nondeterministic state)
         tail [1,2) = {B}    B is RESTORED from _freeze, not re-run
```

B's cumulative key is `hash(interp, A.code, B.code)`. None of those bytes changed between
builds, so B is a cache hit, and it is served **an output computed from A's *previous*
run** while the kernel now holds A's *current* state. The tail-restore's stated
assumption, *"their kernel state is never needed (nothing after them runs)"*
(`exec.rs`), is false here: A ran, so B's inherited state *did* change, but B's key can't
see it.

The existing test `plan_cache_false_cell_always_reruns` only exercises `cache: false`
with an **empty** disk cache (`&[]`), where the dependent is missing from disk and
therefore re-runs; the disk-cached-dependent case is untested.

### The reproduction (cold build, real kernel)

`repro.tmd`:

```
```{python}
#| cache: false
import random
v = random.randint(100000, 999999)
print("A:", v)
```

```{python}
print("B:", v)
```
```

```
BUILD 1 (empty _freeze):   A: 859248   B: 859248     # consistent; B cached
BUILD 2 (fresh process):   A: 890903   B: 859248     # A re-ran, B STALE
```

On build 2 the single rendered HTML shows `A: 890903` and `B: 859248` for the same
`v` — an internally inconsistent, stale document, produced by a plain cold `build`. This
refutes the standing reassurance (in the Quarto design-decisions catalog) that *"the cold
build... is the source of truth and will catch it"*: for this pattern the cold build
*is* the stale artifact.

### Why it matters / how bad

A user who follows the documented rule (mark a nondeterministic cell `#| cache: false`
"so a stale cached result would be wrong", `docs/guide/reference/cell-options.tmd`) still
gets a stale, inconsistent result in every cell that *depends on* that cell — silently,
on the artifact that ships. It contradicts three separate statements of the invariant:
the `freeze.rs` "lone by-design stale-hit path" comment, the user-facing cache guidance,
and the catalog's "cold build catches it".

### Recommended fix (ranked; escalate only as far as needed)

1. **Doc + guard (ship this).** State plainly that `cache: false` is per-cell and does
   **not** propagate: a cell that consumes a `cache: false` cell's runtime state must
   also be `cache: false`. Correct the `freeze.rs` "lone by-design stale-hit path"
   comment to name the class (below). Add a test that **pins** the current
   restore-the-dependent behavior so it's a conscious decision, not an accident.
2. **Correctness-first (optional; has a real perf cost).** In `plan()`, forbid the
   disk-tail restore from reaching any cell at/after `first_uncacheable` (i.e. once a
   `cache: false` cell is in the run range, extend `run_end` to `len()`), so every cell
   downstream of nondeterminism re-runs. This is the only *fully* correct behavior, but
   it kills the cache benefit for the entire tail after any `cache: false` cell — even
   for dependents that are genuinely independent of it (the tool can't tell which,
   because it deliberately does no dependency analysis).

Given the minimal-config, "perfect the default" ethos and that the failing pattern is
arguably misuse, option 1 is the right immediate ship; option 2 is documented here as
considered-and-costed, to revisit only if the stale hit is observed in practice.

### Update (2026-07-22): shipped option 2 (the correctness fix)

Reversed the recommendation above and shipped **option 2** on branch
`ap4-1-cache-false-downstream`. `plan()` now extends `run_end` to the document end
whenever any `#| cache: false` cell exists, so every cell after nondeterminism re-runs
instead of restoring a disk hit the key can't tell is stale. Rationale for overriding the
audit's own option-1 lean:

- The stale hit *was* observed in practice here (reproduced on a cold build), which is the
  exact trigger the option-2 caveat named ("revisit only if observed in practice").
- The tool's headline promise is "no stale hits, **nothing to clear by hand**". Option 1
  leaves the invariant false and pushes a "remember to also mark every downstream cell"
  burden onto the user — i.e. clearing by hand. Option 2 makes the invariant actually
  hold, which is more in the "perfect the default before adding a knob" spirit than
  documenting an exception.
- "Arguably misuse" is weak when the docs walk the user *into* the pattern (the
  cell-options guidance says mark nondeterministic cells `cache: false` with no downstream
  warning).
- The perf cost is real but scoped: it only bites documents that *use* `cache: false` (an
  opt-in, rare feature for nondeterministic cells), and re-running those dependents is the
  semantically correct behavior — a cell that consumes nondeterministic state is itself
  nondeterministic. Authors who want a cached tail can keep `cache: false` cells at the
  end of the document.

Verified: new pinning test `plan_cache_false_forces_downstream_disk_hits_to_rerun` (failed
before, passes after), the empirical repro's build-2 now prints `A == B` (stale hit gone),
full `taliesin-server` + `taliesin-core` suites green, clippy clean. The `freeze.rs`
comment / docs corrections (AP4-2) and AP4-3/AP4-4 remain as separate follow-ups.

## AP4-2: the key can't see out-of-band inputs (not just packages)

`freeze.rs` names installed packages as *"the lone by-design stale-hit path."* It is not
lone. The key folds in only `interp + code`, so **any input a cell reads out of band**
is invisible: a data file (`pd.read_csv("data.csv")`), an environment variable, a network
resource, the wall clock. Edit the data without editing the code and a cold build
restores the old output — same mechanism as AP4-1, and equally silent on the shipped
artifact. This is acknowledged as inherent/shared-with-Quarto in the design catalog, but
(a) it is not in the user docs, and (b) the docs' `include: false` example literally
reads `data.csv` in a cell that is **not** `cache: false`, so downstream cells that use
`df` will go stale. Fix: correct the `freeze.rs` comment to describe the class, and add a
one-paragraph docs note ("the cache keys on code + upstream + interpreter, **not** on the
files/env/network a cell reads; mark such cells `cache: false`"). No code change — hashing
the filesystem is out of scope and against the design.

## AP4-3: `is_uncacheable` truncation match is a bare substring

```rust
fn is_uncacheable(output: &str) -> bool {
    output.contains("class=\"tali-error\"") || output.contains("taliesin: output truncated")
}
```

The `tali-error` half was deliberately hardened to match the class attribute, not a bare
substring, *"so a successful cell whose output merely prints the text 'tali-error' still
caches"* (its own comment). The truncation half is the bare substring it warns against:
`print("taliesin: output truncated")` makes the cell perpetually "uncacheable", so it
re-runs on every build. Safe direction (never a stale hit), but a needless re-run and an
inconsistency with the sibling check. Fix: match the fuller emitted form (the kernel emits
`[taliesin: output truncated at N items]` / `... at N KB]`), e.g. `"[taliesin: output
truncated at "`, mirroring the class-scoped match.

## AP4-4: shared `<page>.json.tmp` temp name (concurrency, AP3-adjacent)

`save()` writes to `path.with_extension("json.tmp")` — a fixed name per page, not
pid/uuid-unique (unlike the kernel/warm-pool runtime dirs, which already use
`<pid>_<uuid>`). Two processes building the *same* page concurrently would write the same
temp and race the rename; the rename is atomic, but the shared temp can interleave one
process's partial write with the other's rename. Worst case is a corrupt temp that the
loader tolerates by starting empty (a lost cache generation), **not** a stale hit. Rare,
because per-page execution is serialized through the exec pool. Fix: unique temp name.
Cross-references AP3 (concurrency).

## False leads / things that turned out fine (recorded honestly)

- **"Options-stripped code means output-affecting options aren't hashed -> stale hit."**
  The initial hypothesis. Refuted: every output-shaping option is re-applied at render
  *after* the cache boundary (`figure`/`table` captions and anchors in `output_block`,
  `include` gating emission in `run`), `fig-export` is explicitly folded *into* the hashed
  code (`exec.rs`), and `eval`/`output`/`warning` are parsed-and-ignored
  (`cell_extract.rs`, "not yet honoured"). Toggling any of these does the right thing. The
  option-stripping is safe *today* — but note that if `eval`/`output`/`warning` ever start
  being honoured, each must be folded into the key like `fig-export`, or it becomes a new
  AP4-1-class stale hit.
- **FNV-1a / cumulative-chain ambiguity.** The chain feeds a fixed-width 16-hex digest +
  `\n` + code into each step, so the split is unambiguous and reordering two identical-code
  cells yields identical keys (and identical outputs). No practical collision or
  reorder-staleness.
- **Interpreter-swap invalidation.** Works as documented across processes: a different
  path or a different reported `--version` changes the id and busts every key. The only
  gap is the intra-process memoization noted in the summary (same path upgraded in place).
- **Atomic write / crash safety.** Temp + rename; a crash mid-write can't corrupt an
  existing cache, and a corrupt/older-format file starts empty. Solid.

## Build-ready items to fold into backlog "Open work"

(Deferred from `backlog.md` in this session to avoid an item-number collision with the
unmerged `worktree-corpus-demand-probe` branch, which already claims the next integer;
fold after that branch lands. Proposed as one item:)

- **AP4 cache-correctness follow-ups** (`notes/2026-07-22-cache-correctness-audit.md`):
  - **AP4-1 (P2) — DONE 2026-07-22 (branch `ap4-1-cache-false-downstream`):** `cache:
    false` didn't propagate — a cacheable dependent of a `cache: false` cell restored
    stale on cold build. Shipped option 2 (plan() forces downstream re-run) + pinning
    test; the stale hit is gone. Fold nothing here; already fixed.
  - **AP4-2 (P3, doc):** correct the `freeze.rs` "lone by-design stale-hit path" comment
    to name the out-of-band-input class; add a docs note; fix the `data.csv` example.
  - **AP4-3 (P3):** make `is_uncacheable`'s truncation check match the bracketed marker,
    not a bare substring.
  - **AP4-4 (P3, AP3):** give the freeze temp file a pid/uuid-unique name.
