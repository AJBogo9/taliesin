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

**1-3 SHIPPED 2026-07-27** — see the post-pin section below for what they cost and what they taught.

1. ~~The cursor-walk table test over `lsp_nav.rs` + `lsp_complete.rs`~~ — **done**, but it took two
   passes; the reason is the axis note below.
2. ~~One assertion on `server_capabilities`~~ — **done**, all 12.
3. ~~`runtime_dirs.rs`~~ — **done**, 4 of 5. **Correction to the paragraph above: this file did *not*
   have zero tests, and `pid_alive` was *not* unpinned.** It had two good tests covering the sweep
   logic (dead owner, live owner, own pid, legacy dir). The real gaps were narrower — the producer and
   consumer were tested apart, and the public entry point was never called — and the fifth survivor
   is `cfg`-dead, not a gap.
4. **`complete.rs`, 30 survivors** — shell completion output has no behavioural pin. (Item 63.)
5. **`doctor.rs`, 14** — but see below; a good chunk is cosmetic. (Item 64.)

Exact locations for everything still owed are in the appendix at the end of this file.

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

**Five unkillable mutants, recorded so no future session burns time on them.** A mutation *score*
cannot tell these from real gaps, which is the whole argument for reading the survivor list:

| mutant | why no test can kill it |
|---|---|
| `is_div_class_context` `j < 2` → `j <= 2` | at `j == 2` the real code falls through to `k = j - 2 = 0` and fails `k >= 3`, returning `false` exactly as the mutant does — and `j == 2` can never be a real div context, since `:::` needs three characters before the `{.` |
| `harvest_anchor_ids` `i = j + 1` → `i = j` | `chars[j]` is always the closing `}`, which cannot begin a `{#` |
| `harvest_anchor_ids` `i = j + 1` → `i = j - 1` | `chars[j - 1]` is the last id character; rescanning it finds nothing new, and results land in a `BTreeSet` |
| `runtime_dirs.rs:103` `pid_alive -> false` | the `#[cfg(not(unix))]` arm, dead code here. cargo-mutants parses **without evaluating `cfg`**, so **this reappears on every future run of that file** |
| `lsp.rs:672` `&typed[..s + 1]` → `&typed[..s * 1]` (item 60) | `dir_part` is consumed only by `doc_dir.join(dir_part)`, and `join("sub/")` and `join("sub")` name the same directory — so for **every relative path** the two list identical entries. They diverge only at `s == 0`, i.e. a `typed` beginning with `/`: `join("/")` is the filesystem root, `join("")` is the document's own directory. But `includes::try_join_in` **refuses** any absolute path (`Refused::OutsideRoot`), so pinning that case would fix as a contract the offering of paths the shortcode cannot accept. Its `+` → `-` sibling **is** killed |

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

## Appendix: the exact survivor locations still owed

Banked **inside the repo** so the remaining items do not depend on a `mutants.out` that lives
outside it. Copy a block into a `-F` regex to re-measure just that file after pinning it.
`interactive.rs`'s five are deliberately absent: knowing skip, reason above.

### `lsp.rs` — item 60 (33 survivors) — **CLOSED 2026-07-27: 32 killed, 1 equivalent**

The 12 `server_capabilities` ones fell to item 59; the remaining 21 (the *request* surface) fell to
item 60, each verified by restoring the mutant and watching a named test fail. The one survivor is
`672:39 + → *`, proven equivalent above. **What the round is worth remembering for** is where the
holes were, not the count:

- **Three of them were the reject axis of a rule whose accept axis was already covered**, exactly as
  item 58 predicted for this kind of code: `!abs.exists()` (only real include paths were ever
  asked about), the typed-prefix filters (every completion test completes from an *empty* token,
  where `||` short-circuits), and `words > 0` (every outline fixture is well past zero).
- **`frontmatter_key_doc` was the second "test that looks like coverage" in this file.** The hover
  test asserted `md.contains("title")` — satisfied by the `` `title:` `` header the hover prints
  *above* the description — so the lookup could return any other key's prose, an empty string, or a
  constant. Pinned now by quoting the vocab entry itself, plus an undocumented key that must hover
  with nothing.
- **`cmd_lsp` needed a new test binary** (`tests/lsp_stdio.rs`): the in-process tests drive `run()`
  over `Connection::memory()` and never touch the command that wraps it, so its whole body could
  become "exit 0" — an editor would be told the server shut down cleanly rather than crashed.

```
lsp.rs:22:5: replace server_capabilities -> ServerCapabilities with Default::default()
lsp.rs:26:9: delete field position_encoding from struct ServerCapabilities expression in server_capabilities
lsp.rs:27:9: delete field text_document_sync from struct ServerCapabilities expression in server_capabilities
lsp.rs:29:17: delete field open_close from struct TextDocumentSyncOptions expression in server_capabilities
lsp.rs:30:17: delete field change from struct TextDocumentSyncOptions expression in server_capabilities
lsp.rs:34:9: delete field definition_provider from struct ServerCapabilities expression in server_capabilities
lsp.rs:35:9: delete field document_symbol_provider from struct ServerCapabilities expression in server_capabilities
lsp.rs:36:9: delete field hover_provider from struct ServerCapabilities expression in server_capabilities
lsp.rs:37:9: delete field completion_provider from struct ServerCapabilities expression in server_capabilities
lsp.rs:40:13: delete field trigger_characters from struct CompletionOptions expression in server_capabilities
lsp.rs:48:9: delete field code_action_provider from struct ServerCapabilities expression in server_capabilities
lsp.rs:49:9: delete field rename_provider from struct ServerCapabilities expression in server_capabilities
lsp.rs:62:5: replace cmd_lsp -> ExitCode with Default::default()
lsp.rs:262:23: delete - in handle_request
lsp.rs:303:16: delete ! in resolve_definition
lsp.rs:313:44: replace + with - in resolve_definition
lsp.rs:313:44: replace + with * in resolve_definition
lsp.rs:447:13: delete field diagnostics from struct CodeAction expression in resolve_code_actions
lsp.rs:574:9: delete field kind from struct CompletionItem expression in resolve_completion
lsp.rs:608:47: replace || with && in resolve_completion
lsp.rs:647:37: replace || with && in resolve_completion
lsp.rs:672:39: replace + with - in resolve_completion
lsp.rs:672:39: replace + with * in resolve_completion
lsp.rs:705:25: delete field label from struct CompletionItem expression in resolve_completion
lsp.rs:706:25: delete field kind from struct CompletionItem expression in resolve_completion
lsp.rs:711:25: delete field detail from struct CompletionItem expression in resolve_completion
lsp.rs:712:25: delete field filter_text from struct CompletionItem expression in resolve_completion
lsp.rs:713:25: delete field text_edit from struct CompletionItem expression in resolve_completion
lsp.rs:755:32: replace match guard !number.is_empty() with true in merged_xref_targets
lsp.rs:798:5: replace frontmatter_key_doc -> Option<String> with Some(String::new())
lsp.rs:798:5: replace frontmatter_key_doc -> Option<String> with Some("xyzzy".into())
lsp.rs:805:38: replace == with != in frontmatter_key_doc
lsp.rs:859:25: replace > with >= in to_document_symbol
```

### `headless_js.rs` — item 62 (7 survivors) — **CLOSED 2026-07-27: 7 of 7 killed**

Every one verified by restoring the mutant and watching a named test fail. The shape here is
different from `lsp.rs`'s and worth carrying forward: **the existing tests checked that each browser
phase *has* a bound, and none checked what the bound does when it fires.**

- **`every_browser_await_is_bounded` is a real guard and it still left the whole teardown open.**
  It enumerates awaits and checks each is wrapped; the *decision* around the wrappers
  (`closed && waited` → kill) is invisible to it, and both operators in it could be flipped —
  leaking a Chrome process and its profile per run — with the scan green.
- **The wedged-launch test asserted `why.contains("launch")`, which BOTH launch failures satisfy.**
  The outer bound sits deliberately above the configured `launch_timeout` so the *library's* error,
  which carries the browser's stderr, is what the author reads. Inverting that ordering keeps the
  test green and silently costs the diagnostic. Measured: today's reason is
  `"chrome launch failed: Timeout while resolving websocket URL…"`; the mutant's is
  `"chrome launch timed out"`.
- **`eval_timeout` was extracted** (4 lines) so the relationship it exists for is assertable: the
  in-page script counts its own budget down, so a wrapper at or below that budget fires first and
  reports `timed out` for a page that was about to answer. The only behavioural route to it needs a
  real Chrome *and* a cell that settles between the two bounds — a 6 s test in the live-Chrome gate
  nothing runs.
- **Two survivors are pinned structurally, on purpose.** `331:25` (`&&` → `||`) and `332:8`
  (`delete !`) are teardown decisions reachable only by a browser that speaks CDP and then lies; no
  fake binary gets past the launch handshake, and a real Chrome exits cleanly, so neither mutant is
  observable end to end. `a_browser_that_does_not_exit_is_killed` is a source-level guard in the
  same style as its neighbour, mutation-checked against exactly those two operators. **It pins the
  spelling, not the behaviour** — an equivalent rewrite of the teardown would fail it, which is the
  cost of the trade.

```
headless_js.rs:200:5: replace chrome_available -> bool with true
headless_js.rs:215:5: replace settle_timeout -> Duration with Default::default()
headless_js.rs:218:24: replace > with >= in settle_timeout
headless_js.rs:311:47: replace + with - in observe_inner
headless_js.rs:331:25: replace && with || in observe_inner
headless_js.rs:332:8: delete ! in observe_inner
headless_js.rs:373:16: replace + with - in observe_page
```

### `complete.rs` — item 63 (30 survivors)

```
complete.rs:15:5: replace cmd_completions -> ExitCode with Default::default()
complete.rs:16:41: replace == with != in cmd_completions
complete.rs:21:19: delete ! in cmd_completions
complete.rs:68:70: delete ! in InstallEnv::from_env
complete.rs:135:5: replace install_completions -> ExitCode with Default::default()
complete.rs:329:5: replace command_desc -> &'static str with "xyzzy"
complete.rs:418:9: delete match arm "schema" in flags_for
complete.rs:419:9: delete match arm "symbols" in flags_for
complete.rs:423:9: delete match arm "map" in flags_for
complete.rs:427:9: delete match arm "skim" in flags_for
complete.rs:495:16: delete ! in positionals_seen
complete.rs:496:17: replace && with || in positionals_seen
complete.rs:498:45: replace == with != in positionals_seen
complete.rs:498:52: replace && with || in positionals_seen
complete.rs:500:19: replace += with -= in positionals_seen
complete.rs:500:19: replace += with *= in positionals_seen
complete.rs:533:9: delete match arm "render" | "read" | "blocks" | "symbols" in positional_kind
complete.rs:544:40: replace + with - in complete_paths
complete.rs:544:40: replace + with * in complete_paths
complete.rs:565:37: delete ! in complete_paths
complete.rs:623:13: replace && with || in dir_contains_tmd
complete.rs:623:22: replace > with >= in dir_contains_tmd
complete.rs:624:13: replace && with || in dir_contains_tmd
complete.rs:625:13: replace && with || in dir_contains_tmd
complete.rs:630:60: replace - with + in dir_contains_tmd
complete.rs:630:60: replace - with / in dir_contains_tmd
complete.rs:683:41: replace && with || in complete_line
complete.rs:688:26: replace == with != in complete_line
complete.rs:688:42: replace && with || in complete_line
complete.rs:688:60: replace == with != in complete_line
```

### `doctor.rs` — item 64 (14 survivors)

```
doctor.rs:26:9: replace Status::glyph -> char with Default::default()
doctor.rs:33:9: replace Status::color -> &'static str with ""
doctor.rs:33:9: replace Status::color -> &'static str with "xyzzy"
doctor.rs:40:9: replace Status::json -> &'static str with ""
doctor.rs:40:9: replace Status::json -> &'static str with "xyzzy"
doctor.rs:142:25: delete ! in active_env_check
doctor.rs:173:5: replace colored -> bool with true
doctor.rs:173:5: replace colored -> bool with false
doctor.rs:173:44: replace && with || in colored
doctor.rs:176:5: replace paint -> String with String::new()
doctor.rs:176:5: replace paint -> String with "xyzzy".into()
doctor.rs:187:70: replace == with != in summary
doctor.rs:188:13: delete match arm Some(Status::Ok) in summary
doctor.rs:244:18: replace match guard s.starts_with("--") with false in cmd_doctor
```

### `lsp_outline.rs` — item 64 (4 survivors)

```
lsp_outline.rs:31:40: replace && with || in clean_title
lsp_outline.rs:31:51: replace + with - in clean_title
lsp_outline.rs:31:51: replace + with * in clean_title
lsp_outline.rs:85:27: replace + with * in headings
```

### `zip.rs` — item 64 (2 survivors)

```
zip.rs:48:89: replace < with <= in build_zip
zip.rs:104:36: replace - with + in build_zip
```

