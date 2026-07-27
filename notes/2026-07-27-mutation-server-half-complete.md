# Mutation re-run, `crates/server` half — COMPLETE (the ten files after `lsp_nav.rs`)

**Status: finished 2026-07-27, 707 of 707 mutants.** This closes the sweep that
[2026-07-26-mutation-server-half-partial.md](2026-07-26-mutation-server-half-partial.md) left owed.
That file still stands on its own for `lsp_nav.rs`; this one covers everything else.

## What was run

```sh
# against a `git archive HEAD` snapshot outside the repo, so the working tree stayed free
cargo mutants -f crates/server/src/{lsp_complete,complete,lsp,headless_js,lsp_outline,doctor,\
runtime_dirs,interactive,zip,lsp_pos}.rs -j 4 --minimum-test-timeout=120 --output <outside-the-tree>
```

Snapshot commit: `9e590e6`. Output kept at
`~/.local/share/taliesin-mutants/2026-07-27-server/mutants.out` — **a persistent path, not a session
scratch dir.** That is the whole fix for how the previous run was lost: `missed.txt`/`caught.txt`/
`timeout.txt` are appended as the run goes, so an interrupted sweep stays readable.

**Scoping verified, not assumed.** cargo-mutants ran `cargo test --package=taliesin-server@0.2.0`
(40 test binaries), and the baseline was green (`55s build + 26s test`). The partial run's claim that
no workspace recheck is owed here was *checked* this time: the only two `crates/core` tests that
spawn a subprocess (`deck_qr_golden.rs`, `reactive_live_region.rs`) spawn `node`, not the taliesin
binary, and `taliesin-core` does not depend on `taliesin-server`. No core test can reach server code,
so a MISSED here is real — there is no repeat of the core half's 53%-false-MISSED disaster.

## Measured

| outcome  | count |
|----------|-------|
| caught   |   497 |
| missed   |   156 |
| timeout  |    16 |
| unviable |    38 |
| **run**  | **707 of 707** |

**Wall clock: 1 h 32 min** (08:29:26 → 10:01:26) at `-j 4` = **7.8 mutants/min**.

**The recorded cost model was 3.4× too pessimistic and should not be reused.** The backlog budgeted
"roughly three hours" from the `lsp_nav.rs`-derived rate of 2.3 mutants/min. The true rate across ten
files is 7.8/min. The 2.3 figure was an artefact of `lsp_nav.rs`'s own slow tests, not a per-server-
mutant constant: most mutants are *caught*, and a caught mutant aborts its test run early instead of
running the suite out. A file's rate therefore tracks its survivor density, and only a bad file is slow.

## Survivor density by file

| file | missed / mutants | | file | missed / mutants |
|---|---|---|---|---|
| `doctor.rs` | 14 / 33 (42%) | | `complete.rs` | 30 / 149 (20%) |
| `interactive.rs` | 5 / 12 (41%) | | `zip.rs` | 2 / 10 (20%) |
| `lsp.rs` | 33 / 98 (33%) | | `lsp_complete.rs` | 56 / 294 (19%) |
| `runtime_dirs.rs` | 5 / 15 (33%) | | `headless_js.rs` | 7 / 49 (14%) |
| | | | `lsp_outline.rs` | 4 / 38 (10%) |
| | | | **`lsp_pos.rs`** | **0 / 9 — the only clean file** |

## The 156 survivors are THREE shapes, not one

This is the substantive difference from `lsp_nav.rs`, where all 36 collapsed to a single shape.

### 1. Twenty-five whole-function replacements — the function has no behavioural test at all

The mutant replaces the entire body with a constant and nothing notices. Not an unpinned edge: an
unpinned *function*. Clustered by subsystem:

- **`runtime_dirs.rs` — 5 of 5 survivors, the whole file.** `kernel_conn_dir`, `warmpool_dir` and
  `tagged` can all return `PathBuf::default()`; `sweep_stale_runtime_dirs` can become `()`; `pid_alive`
  can return `false`. This is the ungraceful-death reaping subsystem shipped at `bccb210` — **the
  startup sweep could be a no-op in production and no test would fail.**
- **`lsp.rs::server_capabilities` — 11 survivors on one function.** `Default::default()` survives, so
  nothing asserts the LSP handshake advertises completion, definition or symbol support. Given that
  click-to-source is a load-bearing goal, this is the highest-value cheap fix in the list.
- **`complete.rs`** — `cmd_completions` and `install_completions` bodies replaceable with
  `Default::default()`, `command_desc` with `"xyzzy"`.
- **`headless_js.rs`** — `chrome_available` can return `true`, `settle_timeout` can return
  `Duration::default()`. Note this file gained two tests on 2026-07-26 (item 55); those tests pin the
  *bounding* behaviour, not these.
- **`interactive.rs` — 5 of 5** (`is_interactive`, `select`, `input`), and **`doctor.rs`**'s
  `colored`/`paint`. See "deliberate skips".

### 2. The remaining 131 are operator, boundary and negation mutations inside line scanners

Same shape as every prior round: `<`→`<=`, `&&`→`||`, `+`→`*`, `delete !` inside a function that walks
a cursor along a line. Grouped by function, densest first: `harvest_bib_keys` **20**,
`harvest_anchor_ids` **10**, `is_div_class_context` 8, `detect_shortcode_path` 6, `positionals_seen` 6,
`dir_contains_tmd` 6, `resolve_completion` 10, `nested_parent` 4, `flags_for` 4, `complete_line` 4,
`frontmatter_value` 3, `complete_paths` 3, `clean_title` 3, `resolve_definition` 3.

**This is the finding that changes the plan.** These are the same construct as `lsp_nav.rs`'s
classifiers, so **the table-driven cursor-walk test already prescribed for `lsp_nav.rs` is the same
test for `lsp_complete.rs`** — take a fixture line per construct, walk the cursor across every byte,
assert the result at each offset. One test *pattern* over two files covers roughly **85 of the 192
survivors across both halves of the server sweep**, plus it explains 31 of the 32 timeouts.

### 3. All 16 timeouts are scan-cursor arithmetic — detections, not gaps (third confirmation)

Every one is `+=`→`*=` or `-=`→`/=` on a loop cursor: `harvest_bib_keys` (4), `detect_shortcode_path`
(3), `frontmatter_value` (3), `harvest_anchor_ids` (2), `is_div_class_context` (2), `detect_xref` (1),
`positionals_seen` (1). A cursor that stops advancing spins instead of returning a wrong answer, so
the suite hanging **is** the kill. Do not write tests for these. That is now 7 + 16 + 16 = 39 timeouts
across the campaign, all the same shape, with zero exceptions — treat the rule as settled.

## Ranked next moves

1. **The cursor-walk table test, written once and applied to both `lsp_nav.rs` and
   `lsp_complete.rs`.** ~85 survivors, and it is the item the partial run already prescribed.
2. **One assertion on `server_capabilities`.** 11 survivors for a few lines, guarding the LSP
   handshake that click-to-source depends on.
3. **`runtime_dirs.rs`, 5 survivors, currently zero tests.** `pid_alive` and the stale-dir sweep are
   the kind of thing that fails silently in production, which is exactly why the mutants survive.
4. **`complete.rs`, 30 survivors** — shell completion output has no behavioural pin.
5. **`doctor.rs`, 14** — but see below; a good chunk is cosmetic.

## Deliberate skips, with the reason recorded so they are not re-litigated

- **`interactive.rs` (5 of 5).** `is_interactive`, `select` and `input` are the TTY wizard layer;
  pinning them needs a PTY harness. The *non*-TTY path is already pinned by
  `crates/server/tests/wizard_gate.rs`, which is the path that actually runs in CI-like conditions.
  Poor cost/benefit; skip knowingly.
- **`doctor.rs::colored` / `::paint`** (4 of its 14). Terminal colour selection. Cosmetic.

## Post-pin re-measure (same day): items 58, 59 and 61 landed

The pins were written and then **re-measured against the recorded survivor list**, 183 mutants over
`lsp_nav.rs` + `lsp_complete.rs` in 29 min. That re-measure is the reason this section can state
numbers instead of intentions, and it changed the work twice.

**The first cursor-walk pass killed only half.** It caught 18 of `lsp_nav`'s 36 and 38 of
`lsp_complete`'s 56, leaving **42 alive**. The split is the lesson:

> **A cursor walk over well-formed fixtures pins SPANS. Every survivor was a guard that REJECTS.**
> Weakening one does not move a boundary — it makes the classifier accept nonsense: `j > key_start`
> → `>=` invents a citation with an empty key, `j > id_start` invents an xref with an empty id,
> `kw == "include"` → `!=` matches every shortcode *except* include, and any scan bound → `<=` reads
> one character past the end of the line. **Malformed input is a separate axis from cursor
> position**, and no amount of walking a well-formed fixture reaches it.

Closing it took ~30 more fixtures, each malformed in one specific way. Final state: **39 of the 42
killed, each verified by hand** (restore the mutant, watch the named test fail, restore), and the
remaining 3 are provably equivalent.

**Two "the test looked like coverage" holes, which is the shape worth hunting elsewhere:**

- `lsp.rs`: every LSP test performs the handshake and **discards the `InitializeResult`**
  (`handshake()` does `let _ = recv()`). A server advertising nothing passed the whole suite. No
  line-coverage tool reports this, because every line ran.
- `lsp_complete.rs`: the candidates test already had a `.hidden` fixture, but `.hidden` is not a
  `.tmd`, so it never reaches the output whether the dotfile filter ran or not. A dotfile that *is*
  a `.tmd` is what makes the rule observable.

**Four unkillable mutants, recorded so no future session burns time on them.** A mutation *score*
cannot tell these from real gaps, which is the whole argument for reading the survivor list:

| mutant | why no test can kill it |
|---|---|
| `is_div_class_context` `j < 2` → `j <= 2` | at `j == 2` the real code falls through to `k = j - 2 = 0` and fails `k >= 3`, returning `false` exactly as the mutant does — and `j == 2` can never be a real div context, since `:::` needs three characters before the `{.` |
| `harvest_anchor_ids` `i = j + 1` → `i = j` | `chars[j]` is always the closing `}`, which cannot begin a `{#` |
| `harvest_anchor_ids` `i = j + 1` → `i = j - 1` | `chars[j - 1]` is the last id character; rescanning it finds nothing new, and results land in a `BTreeSet` |
| `runtime_dirs.rs:103` `pid_alive -> false` | the `#[cfg(not(unix))]` arm, dead code here. cargo-mutants parses **without evaluating `cfg`**, so **this reappears on every future run of that file** |

**The timeout rule held a fourth time:** both timeouts in the re-measure (`harvest_bib_keys` 315:23,
320:27) are `+=` → `*=` on a scan cursor. That is 41 of 41 across the campaign.

**Three process failures worth more than the pins**, all the same vacuous-green shape this round
exists to remove:

- **`git checkout HEAD -- <file>` to undo a mutation deleted the uncommitted tests**, so two
  verifications ran with only the pre-existing tests and were vacuously green. Revert a mutation by
  **inverse edit**, never through git. (Recorded before, in `notes/backlog.md`'s standing
  constraints, and it still bit.)
- **A `pkill -f '<pattern>'` matched the invoking shell's own command line and killed it** (exit
  144). Kill by PID.
- **A mutation anchor string that matched twice SKIPPED its check** rather than running it — the
  12-space `while j < n` line is a substring of the 16-space one. A harness that reports "skip" as
  anything other than a failure is a green light for work that never happened.

## Still owed after this

Nothing in scope: `lsp_nav.rs` is banked as a partial (338 of 444, findings complete enough to act on)
and these ten files are complete. The **106 untested `lsp_nav.rs` mutants** are the only unmeasured
remainder of the server half, and re-running that file is confirmation work, best done *after* the
cursor-walk test lands.
