# AP2 — Robustness / adversarial-input (fuzzing) audit

**Date:** 2026-07-22
**Perspective:** AP2 from the "Audit perspectives" section of `backlog.md` (the doc's #1
recommended first round). *Stateful, solo.* Run in an isolated worktree off `origin/main`
(`58a8bd7`) so it could not collide with the two concurrent sessions (one writing features on
`polish/a11y-holes`, one auditing).
**Method:** a subprocess-isolated harness (`notes/ap2-fuzz-harness/fuzz_render.rs`, an
audit-only example that reads one case from stdin and calls a chosen render entry point) driven
per-case with a `timeout`, so a *panic* (exit 101), a *stack-overflow abort* (SIGABRT/SIGSEGV),
and a *hang* (killed at the timeout) are all observable without wedging the run. Three input
sources: (1) 133 hand-crafted hostile `.tmd` cases (`gen_cases.py`), (2) size-scaling sweeps to
separate linear from super-linear cost, (3) a deterministic generative mutation fuzzer
(`fuzz_loop.py`: token-soup + random-bytes + corpus-splice + byte-flip + repeat) over 7,500
iterations across the single-doc **and** full-page paths. Plus targeted probes of the includes,
KaTeX, site-config, deck, `check`, and include-path surfaces through the real `taliesin` binary.

---

## Headline

**The parse→render pipeline is genuinely hardened — far more than the perspective's framing
assumed — and AP2's stated premise is overstated and should be corrected.** The premise was "~700
panic sites, every panic 500s the dev server, zero fuzz coverage." In practice:

- **Every server/CLI render entry already wraps rendering in `catch_unwind`** and keeps the
  preview alive with a diagnostic: single-doc `serve` init + rebuild
  (`serve/mod.rs:166`, `:1415`), per-page site build (`serve_site/mod.rs:900`, comment: *"one bad
  page cannot take down the server for every page"*), cross-ref refresh (`:1300`), and CLI `build`
  (`build.rs:238`, per-page `:1510`). A render **panic** does **not** crash the server.
- **The core render already runs on a 256 MB worker-thread stack** (`render/mod.rs:207`)
  specifically to absorb deep-nesting recursion that would otherwise abort at ~3000 levels.
- **7,500 generative mutations + 133 hand-crafted hostile inputs produced zero unexpected panics.**
  Include cycles are guarded, `safe_join` shrugs off 16 path attacks, garbage `_site.yml` (16
  variants) degrades gracefully, KaTeX survives macro-recursion bombs, and the deck engine eats
  50k-slide / 100k-separator inputs.

So the honest finding is a *positive*: for **trusted single-author input** (the tool's actual
threat model, per the `lib.rs` trust note) the renderer is robust. **The only two real gaps both
bypass the armor above, and both reduce to one root cause: there is no size / nesting-depth / time
bound around an otherwise-well-guarded renderer.** `catch_unwind` cannot catch a stack-overflow
`abort()`, and it cannot interrupt a hang. Those are exactly the two holes.

Both findings are **P3**: they need pathological input an author would not hand-type, so under the
single-author trust model this is a *resilience / DX-promise* issue ("the warm loop survives your
mistakes and shows a located diagnostic"), not a security one. But each **defeats an existing,
explicitly-relied-upon safeguard**, so they are worth closing.

---

## Verified findings (ranked by corrected severity)

### AP2-1 (P3) — Deep recursive nesting overflows the 256 MB render stack → uncatchable `abort()`, taking down the whole server / whole site build

A document with deeply-nested block structure (cheapest: a run of `>` blockquote markers)
overflows even the 256 MB guard stack and the Rust runtime **aborts the process**
(`thread has overflowed its stack` → SIGABRT). Because an abort is **not unwindable**, it sails
through *every* `catch_unwind` guard, including the per-page site guard whose whole job is to keep
one bad page from killing the others.

**Reproduced through every rendering command:**
- `render_document` (harness): debug ceiling between 65k (OK, 0.24 s) and 70k (abort) `>` levels.
- `taliesin build <file.tmd>`: 90k `>` → SIGABRT (`overflowed its stack`).
- `taliesin build <site-dir>`: a single 90k-`>` **page** → SIGABRT — **aborts the entire
  multi-page build**, every other page lost, defeating the per-page `catch_unwind` isolation.
- `taliesin check <file.tmd>`: 90k `>` → SIGABRT.

**Release threshold (the shipped `taliesin` launcher builds release):** ~600k `>` OK, ~900k
aborts — i.e. a **~900 KB single line of `>`**. Higher than debug (smaller frames) but finite and
reachable. Nested `:::` divs are cheaper per level and did not abort even at 1M levels; blockquotes
are the worst case.

**Root cause:** recursive descent over nested block structure (comrak parse + our AST→block walk)
is proportional to nesting depth; the mitigation is a *bigger stack*, which only moves the cliff.
The comment at `render/mod.rs:211` ("A big stack absorbs any realistic nesting") is optimistic:
the cliff is reachable, and past it the outcome is maximal (whole-process abort, uncatchable).

**Fix direction:** bound nesting depth *before* it overflows and emit a **located diagnostic**
("document nested deeper than N levels at line L") instead of relying on stack size. A cheap
pre-parse guard (count max consecutive leading `>` / open-fence depth, and total input size) is
enough to convert an abort into a graceful, located error and is consistent with the tool's
"survive the author's mistakes" DX. Correct the `render/mod.rs:211` comment either way.

**Repro:** `python3 -c "print('>'*900000+' x')" > deep.tmd && taliesin build deep.tmd` (release);
`… '>'*70000 …` for a debug build.

### AP2-2 (P3) — Balanced nested brackets in prose → comrak inline O(n²) render hang (no render timeout)

A paragraph of `N` open brackets, a character, then `N` close brackets renders in **O(n²)**. There
is **no timeout on pure rendering** (only cell *execution* has `TALIESIN_CELL_TIMEOUT`), so the
render thread wedges: the preview freezes with no crash and no diagnostic, and `catch_unwind`
cannot help because it is not a panic — just unbounded CPU.

**Scaling (confirmed quadratic, each doubling ≈ 3.7×):**

| brackets N | debug   | release |
|-----------:|--------:|--------:|
| 8,000      | 0.26 s  | 0.02 s  |
| 32,000     | 3.55 s  | 0.29 s  |
| 128,000    | (proj. ~57 s) | 4.27 s |
| ~500,000   | minutes | ~60 s+  |

At N=100k (a 200 KB line) debug already exceeds 25 s; release reaches minutes by ~500k (a 1 MB
line). "Slow," not "infinite," but from the author's seat it is an effective hang of the warm loop.

**Root cause — pinned:** it is comrak's (0.52.0) **inline reference-link matcher**, not our emit.
Proof: 40k brackets in a *paragraph* = 5.8 s, but the identical 40k brackets in a *code fence* or
*indented code block* (not inline-parsed) = 0.005 s, and 80k *open-only* brackets (no closing) =
0.04 s. Each `]` scans back through the stack of unmatched `[` openers looking for a link/image
match → quadratic when balanced brackets nest with no actual links. This is upstream comrak
behavior; a comrak upgrade *might* help but can't be relied on.

**Fix direction:** a library-agnostic guard — (a) a pre-parse input-size / bracket-run cap with a
located diagnostic, and/or (b) a **render watchdog**: render already runs on a spawned thread
(`render_internal`); `join`-with-timeout and, on timeout, return a diagnostic while abandoning the
thread (leak-and-recover) turns a multi-minute freeze into a located "render exceeded Ns — is this
document pathological?" error. A watchdog also caps AP2-1's cost and any future super-linear class,
so it is the higher-leverage of the two.

**Repro:** `python3 -c "print('['*500000+'x'+']'*500000)" > brk.tmd && taliesin build brk.tmd`
(release; use 100k for debug).

### AP2-3 (P3, proposal) — Close the "zero fuzz coverage" hole with a small regression harness

AP2's one true premise — *zero fuzz coverage* — still stands. This round built a reusable,
subprocess-isolated harness that catches panics **and** aborts **and** hangs (which an in-process
`proptest`/`cargo-fuzz` cannot distinguish — an abort kills the fuzzer too). The artifacts are
archived under `notes/ap2-fuzz-harness/`. Landing a trimmed version guards AP2-1/AP2-2 and any
future regression:

- A `#[test]` (or `#[ignore]`d, run in CI nightly) that feeds the 133-case battery + the two
  minimized repros through `render_document` / `render_html_page` under a bounded per-case budget
  and asserts "graceful (returns or located diagnostic), never abort/hang."
- Once AP2-1/AP2-2 fixes land, this pins them: the deep-nest doc must yield a diagnostic (not an
  abort) and the bracket doc must return within the watchdog budget.

No nightly toolchain is installed in this environment, so `cargo-fuzz` was not run; the subprocess
mutation fuzzer is the better fit here anyway (it survives the aborts/hangs a libfuzzer run would
die on).

---

## False leads (grep-flagged, code-verified clean — recorded per "re-derive the cause")

The panic-site grep flagged ~300 `unwrap`/`expect`/`panic!` in `crates/core` (most in `#[cfg(test)]`).
The reachable-from-input ones I could construct a trigger for all proved **correct code**:

- **`prose.rs:177`** `line[i..].chars().next().unwrap()` — *not* a multibyte-boundary panic. Every
  branch of the masking loop advances `i` to a char boundary (`.find()` returns byte offsets at
  boundaries; the ASCII `+1/+2` branches start from ASCII bytes; else `+= ch.len_utf8()`), and the
  loop guard guarantees a remaining char. Multibyte inputs (`café`, CJK, emoji, combining marks,
  through the `prose-lint` path) all rendered OK.
- **`render/mod.rs:2088`** `…min().unwrap()` in `toc_html` — guarded by an `if items.is_empty()
  { return }` immediately above; the sibling `toc_items` uses `let Some(base) = … else return`.
- **`render/emit.rs:265-287`** `lines.last_mut().unwrap()` (code line-wrapping) — `lines` is seeded
  with one element and never emptied; long-line / tab / unicode code inputs all OK.
- **`render/divs.rs:250,332`**, **`includes.rs`** cycle guard, **KaTeX** macro expansion, **site
  config** parse, **`safe_join`** path handling — all exercised adversarially, all graceful.

## Surfaces confirmed robust (so a future round need not re-audit them)

`render_document` and `render_html_page` (133 hostile + 7,500 generative, 0 crashes); include
cycles (A↔B and self); include targets / `safe_join` (traversal, absolute, symlink-escape, binary,
huge, null, unicode — 16 attacks); garbage `_site.yml` (scalar/list/unparseable/wrong-typed/huge/
deep/billion-laughs/unknown-flood — 16 variants); KaTeX (`\def` recursion, `\newcommand` loops,
deep `\frac`, huge matrix, macro fan-out); the deck engine (50k slides, 50k fragments, 100k empty
separators, huge notes); `taliesin check`; truncated front-matter / mid-fence / mid-math / mid-code.

## Out of AP2 scope (belongs to other perspectives)

The block **diff** going quadratic (needs two doc versions) → AP1/AP3. `_freeze` cache correctness
→ AP4. Kernel-death mid-render → AP11. Multibyte **sourcepos correctness** of Alt-click (a silent
*misfire*, not a panic) → AP5, though this round confirms the multibyte paths do not *panic*.

---

## Build-ready items to file into `backlog.md`

> Additive block; paste under the open-work section. Prefix `AP2-`.

- **AP2-1 (P3):** Bound nesting depth before the 256 MB stack overflows. A pre-parse guard on max
  consecutive `>` / open-fence depth (+ total input size) that emits a **located** diagnostic, so a
  pathological doc yields "nested too deeply at line L" instead of a whole-process `abort()` that
  defeats every `catch_unwind` (incl. per-page site isolation). Correct the `render/mod.rs:211`
  "absorbs any realistic nesting" comment. Pin with a test that the deep-nest doc renders a
  diagnostic, not an abort.
- **AP2-2 (P3):** Add a **render watchdog** — `join`-with-timeout on the existing `render_internal`
  worker thread; on timeout surface "render exceeded Ns (pathological document?)" and abandon the
  thread. Caps the comrak nested-bracket O(n²) hang and any future super-linear class in one place.
  (Optionally also a bracket-run/input-size pre-parse cap.) Pin with the 500k-bracket doc returning
  within budget.
- **AP2-3 (P3):** Land a trimmed fuzz/property regression harness (from `notes/ap2-fuzz-harness/`)
  that asserts the pipeline is graceful (never abort/hang) over the hostile battery + the two
  minimized repros — closing AP2's "zero fuzz coverage" and pinning AP2-1/AP2-2 after they land.

## AUDITS.md ledger line

> **AP2 robustness / adversarial-input (fuzzing) audit — 2026-07-22.** Subprocess-isolated harness
> (panic + abort + hang observable) + 133 hand-crafted hostile `.tmd` + 7,500 generative mutations
> (doc + page) + targeted include/KaTeX/site/deck/check probes through the real binary. Headline:
> the parse→render pipeline is **hardened** (every render entry is `catch_unwind`-guarded + a 256 MB
> render stack; 0 unexpected panics), so AP2's "every panic 500s the server" premise is overstated.
> Two real P3 gaps, both bypassing that armor via one root cause (no size/depth/time bound): AP2-1
> deep nesting → uncatchable stack-overflow `abort()` (defeats per-page isolation, whole-site);
> AP2-2 balanced nested brackets → comrak-0.52 inline O(n²) render hang (no render timeout). Plus
> AP2-3: close the zero-fuzz-coverage hole. Detail: `notes/2026-07-22-ap2-robustness-fuzzing-audit.md`.
