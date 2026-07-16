# Taliesin: machine-facing output audit (2026-07-16)

**Lens: audit the surfaces with no viewer.** Every prior audit round was eye-driven (browser
screenshots at three viewports, the design audit, the deck audit, the UI audits, "does the corpus
render correctly"). That method covers everything a human eye lands on and structurally misses
everything no eye lands on. This round targets the machine-facing half: output read by browsers,
crawlers, agents, and the preview client, never by a person.

Scope: `protocol.rs`, `minify.rs`, `card.rs`, `llms.rs`, `mcp.rs`, `seo.rs`, `meta.rs`,
`warm_pool.rs`, `build_budget.rs`, `serve_site/exec_pool.rs`, plus `exec.rs::interp_id`.
Not exhaustive: `interpreter.rs` and `backlinks.rs` got only light coverage, and the lens
generalizes further than this round took it.

Method: four read-only parallel agents, each required to mark every finding CONFIRMED or PLAUSIBLE
with a concrete failure scenario, and to re-derive every cause from source (this project's recorded
failure mode is confidently-wrong root causes: see [[backlog-entries-rot]] and the 4x wrong-cause
record in the UI-audit harness). Findings marked "measured" were proven by execution, not reading.

-----------------------------------------------------------------------------

## Executive verdict

The lens paid. ~35 findings, of which six are **firing today** and the rest are latent-but-reachable.
No finding contradicts a load-bearing invariant: the single-editing-surface holds, sourcepos
emission stays total, the freeze cache's content-addressed keying is still sound, and the two dev
servers do **not** drift at the message-shape level (the abstraction in `protocol.rs` is doing its
job; the stated "two owners" worry is refuted, see negative space).

Three themes, and the third is the real one:

1. **The correct implementation already exists next door.** Three times, a helper was written
   correctly for a surface with a viewer and reimplemented weaker for a surface without one:
   - `search.rs:147` pushes a separator space ("so text from adjacent blocks/inlines stays
     word-separated"). `llms.rs:221` reimplemented it without. **Garbling production today.**
   - `search.rs:190` escapes every `<` to `<`. `meta.rs:212` reimplemented it weaker (`</`
     only). **Blanks the whole page on `<!--<script>`.**
   - `minify.rs:239` guards comment-removal token fusion in the **JS** branch. `minify.rs:58`, the
     **CSS branch of the same file**, does not.
   This is not a knowledge gap. It is an attention gradient.

2. **The observable lies in the reassuring direction.** `--jobs 3` builds one page at a time and
   logs "pre-warming 2 kernel(s)" for a pool that never boots. A tab running stale `client.js`
   shows a green "live" pill. A card silently truncates its headline into a complete-looking but
   wrong one. Where a human *does* look, the signal says fine.

3. **The tests certify the defects.** `explicit_jobs_is_respected` passes while the composed path
   halves `--jobs`. The refill test re-implements the loop it pins **with no error arm**, so it
   cannot express the bug. `exec_pool`'s tests carry a comment admitting they cannot test the
   reclamation the type exists for. `render_card_survives_overlong_text` asserts dimensions only and
   passes while truncating. `set_meta` is 54 of 55 ops in a real edit and has **zero** wire-shape
   coverage. The suite asserts *shape*, not *behavior*, because shape is what you can check without
   running the thing. The blind spot reproduced itself in the test suite.

-----------------------------------------------------------------------------

## Top highest-leverage fixes

1. **Share the three helpers instead of re-patching three call sites.** `llms.rs`/`meta.rs` should
   consume `search.rs`'s separator + escape rather than own weaker copies; `minify.rs`'s CSS branch
   should use the JS branch's guard. One coherent change retires theme 1 and stops it recurring.
2. **`--jobs N` collapse (measured, fires on every build, default config).** Docked slots are paid
   to a warm pool before learning whether one boots. *In the Do-NOT-touch exec/kernel zone: needs an
   owner ruling.*
3. **Contain MCP `read`.** Canonicalize + bind to a project root; `cmd_mcp` currently discards its
   args, so no root exists even in principle.
4. **One acorn-based behavior guard** (token stream + AST equality across `core_enhance_js()`,
   using Node's *bundled* acorn: offline, no new dependency). Catches minifier findings 1, 2, 3 and
   10, all of which `node --check` misses.
5. **A `set_meta` wire-shape test.** It is the click-to-source mechanism (a load-bearing goal) and
   the single most-emitted op, with no contract test.

-----------------------------------------------------------------------------

## Findings by theme

### Live now (firing in the current tree)

- **`--jobs N` silently collapses build concurrency** (CONFIRMED, measured). `build.rs:1345-1351`
  hands 2 slots to `budget_split`'s warm pool *before* `warm_pool_for_build` (`:1366`) learns
  whether a pool boots; with no `TALIESIN_PYTHON`/`.venv` (the default) `should_warm` is false and
  zero kernels are pre-warmed, so the slots buy nothing. Measured on 16 cores / 21 GB free:
  `--jobs 2 -> 1 page`, `--jobs 3 -> 1 page`, `--jobs 4 -> 2`, `--jobs 8 -> 6`. The CLI documents
  `--jobs <N>` as "max parallel pages" (`main.rs:230`). The log line reports the loss as a purchase.
  **Do-NOT-touch zone.**
- **`llms-full.txt` fuses adjacent text** (CONFIRMED, measured on production). `llms.rs:221-242`
  strips tags with no separator. Every post on `corpus/tech-blog`: `KL DivergenceHow to measure...
  alignment.17 March 20263 min read`. Title fuses into description, date into reading time, minting
  `20263 min read`. The one file whose entire purpose is machine legibility.
- **The websocket clobbers the SSR `<title>`** (CONFIRMED, measured). `client.js:938` assigns
  `document.title = msg.title || "Taliesin"` *before* the `skipMount` guard, from a `full_render`
  carrying the raw front-matter title (`serve_site/mod.rs:785-793`), while SSR applied
  `title_with_site_suffix` (`:561-563`). `/blog.html` -> `Blog` (suffix dropped). Worse for a
  titleless chapter: `PageDoc.title` is front-matter-only with no H1 fallback (unlike
  `discovery.rs:48`), so **5 of 6 `corpus/demo-book` chapters preview with a tab reading literally
  "Taliesin"**.
- **A front-matter `title:`-only edit broadcasts nothing** (CONFIRMED, measured). The title lives in
  chrome, outside `doc.blocks`, so the diff is empty and `Broadcast::messages` returns `vec![]`
  (`serve_site/mod.rs:947-952`, `protocol.rs:248-268`). The server *does* rebuild (a fresh fetch
  shows the new title); the live tab never hears. **The deck path already fixes exactly this**
  (`serve/mod.rs:1280`, `deck_meta_changed` folded into `remount`), gated on `DocFormat::Reveal`, so
  HTML pages keep the hole and `serve_site` has no equivalent for any format.
- **MCP `read` is an unrestricted arbitrary-file read** (CONFIRMED, executed). `mcp.rs:154` ->
  `query.rs:275-285`: no root, no canonicalization, no extension gate; `cmd_mcp(_args: &[String])`
  (`mcp.rs:69`) discards its args. Verified over real stdio JSON-RPC: `read {"path":"/etc/passwd"}`
  -> 3273 chars; `read {"path":"../../../../../../etc/hostname"}` -> the hostname. Severity is
  threat-model-dependent (an agent with filesystem access gains nothing), but the module documents a
  containment it does not implement, and a host allowlisting this as "document tools only" is
  relying on a guarantee the source never makes.
- **A restarted server leaves tabs running stale `client.js` under a green "live" pill**
  (CONFIRMED). No protocol version anywhere; `boot_id` (`protocol.rs:108-118`) detects the restart
  and is used *only* to force a re-mount. `ws.onclose` auto-reconnects in 1s (`client.js:1094`).
  `CLIENT_JS` is `include_str!`-compiled (`serve/mod.rs:24`), so the tab keeps the old client. The
  `reload()` lever already exists (`protocol.rs:128`, wired at `client.js:1059`) and is not used.
  Same trap CLAUDE.md warns about for assets, except here the server *knows* it restarted.

### Latent but reachable (real triggers, not firing today)

- **Regex literal after `=>`, `)`, `]` read as division** (CONFIRMED, executed). `minify.rs:102-110`
  (`regex_context`) omits those three. Harmless until the regex body contains a quote, which opens a
  phantom string, flips quote parity for the rest of the file, and drops real string bodies into
  `Normal` where `//` and `/* */` are stripped. `["a","b\"c"].filter(s => /["]/.test(s))` followed by
  a string literal truncates it -> **SyntaxError kills all of `app.js` site-wide** (search, TOC,
  lightbox, copy buttons, reading progress). Trigger `s => /['"]/.test(x)` is an ordinary escaping
  idiom.
- **Nested template literals silently rewritten** (CONFIRMED, executed on real mermaid). `minify.rs:
  252-283` tracks templates to the next backtick with no `${}` depth. Proven at mermaid token
  476206: `` a.join(`\n\n`) `` -> `` a.join(`\n`) ``. **Token count identical**, parses clean.
  (Real mermaid bypasses the minifier; this proves the pattern, and finding below removes the
  bypass's only protection.)
- **Nothing enforces the vendored-lib bypass** (CONFIRMED). `build.rs:1102-1104`: the bypass is a
  *comment*, no test. A one-line edit routes `mermaid_bundle_js()` through `minify_js` and corrupts
  it silently. Doc rot at `minify.rs:8`: "vendored `*.min.js` bypass it entirely" is JS-only;
  vendored `katex.min.css` **is** routed through the CSS minifier (`build.rs:1095`).
- **CSS comment removal fuses tokens** (CONFIRMED, css-tree AST diff). `minify.rs:58-66` `continue`s
  without touching `last_was_space`: `margin: 0/* reset */auto` -> `margin: 0auto`, an invalid
  declaration browsers silently drop. The JS branch guards this at `:239-245`.
- **CSS `url()` first-`)` scan desyncs string state** (CONFIRMED). `minify.rs:40-57`.
  `url("a)b.png")` + a later `content:"A/* B */C"` -> `content:"AC"`. **The comment's justification
  is rot**: it claims our CSS urls are base64 data URIs with no `)`, but 40 of 60 `url(` in the real
  generated KaTeX CSS are not data URIs (`.woff`/`.ttf` fallbacks). Behavior still correct today;
  the invariant a maintainer would trust is false.
- **`<!--<script>` in a `description` blanks the entire page** (CONFIRMED, browser-verified).
  `meta.rs:212` escapes only `</`, not `<`, driving the tokenizer into script-data-double-escaped
  state so the emitted `</script>` no longer closes. Measured: ld+json swallows 9163 chars through
  `</body>`, `JSON.parse` throws, `document.body.textContent.length === 0`. Build reports success;
  `check` reports "no problems found".
- **Non-Latin and math glyphs render as tofu** (CONFIRMED, rendered). `card.rs:18`/`:200-215`:
  bundled Newsreader has **658 glyphs** (Latin/Latin-ext/Vietnamese). Missing and drawn as `.notdef`
  boxes: Greek, Cyrillic, CJK, kana, Hangul, Arabic, Hebrew, emoji, **and math symbols (∑ ∫ ∞)**.
  `.notdef` has a non-zero advance, so layout "succeeds" and nothing errors. For this author's
  subject matter (`Deriving ∑ log p(x)`) the trigger is a matter of time. 0/153 live fields today.
- **Headline drops overflow lines with no ellipsis** (CONFIRMED, rendered). `card.rs:326-331`
  `hlines.truncate(3)`, no ellipsis, producing a complete-looking but wrong headline. The *lead*
  ellipsizes correctly via `wrap_clamp` -> `truncate_line` (`:172-183`). Same file, inconsistent.
- **Eyebrow / wordmark / domain get no wrap, truncate, or width check** (CONFIRMED, rendered).
  `card.rs:358-361`, `:399-409`, `:411-414`. A long site title overlaps the wordmark into the
  right-aligned domain ("Learnindgsbogossian.com"); a long domain drives x negative and clips left.
- **Over-long single word overflows the canvas** (CONFIRMED). `card.rs:134-154`: `wrap`'s
  `cur.is_empty()` guard accepts any first word; the shrink at `:327-330` only fires above 3 lines,
  which one long word never reaches. `NullPointerExceptionHandlerFactory` clips at x=1199 (pad edge
  1128). The test `wrap_keeps_an_overlong_word_on_its_own_line` **asserts this** without checking fit.
- **`<lastmod>` emitted verbatim** (CONFIRMED, executed). `seo.rs:24-26`, no W3C-Datetime check.
  `date: "May 15, 2026"` -> `<lastmod>May 15, 2026</lastmod>`; `2026-5-5` -> unpadded. Both invalid;
  crawlers discard silently. `check` -> "no problems found", exit 0. `feed.rs` *does* enforce
  RFC-3339. 11/11 valid today: a latent trap that fires the first time a date is typed the human way.
- **`<loc>` entity-escaped but never URL-escaped** (CONFIRMED, executed). `seo.rs:23`,
  `feed.rs:24-28`. `posts/two words/` -> `<loc>https://ex.com/posts/two words/</loc>` (raw space).
  The same URL goes into `llms.txt` as a Markdown destination, where `[T](https://x/two words/)` is
  not a link under CommonMark.
- **Scheme-less `url:` silently emits an invalid sitemap, robots.txt and llms map** (CONFIRMED,
  executed). `feed.rs:13-19` (`canonical_base`) only trims a trailing `/`. `url: ex.com` builds
  clean and emits `<loc>ex.com/</loc>`, `Sitemap: ex.com/sitemap.xml`. `check` -> "no problems
  found", exit 0. Every machine consumer rejects the lot; no human is told.
- **JSON-LD types an organisation as a `Person`** (CONFIRMED). `meta.rs:140-144`/`:159`. Dormant
  here (site title == author name). Related: `"headline":""` reachable for a dated titleless post
  (`meta.rs:173`); `dateModified` hardcoded equal to `datePublished`.
- **Multi-line block comment drops an ASI-significant LineTerminator** (CONFIRMED).
  `minify.rs:229-251`. `return /* note\nspanning */ 42;` returns `undefined` before, `42` after. The
  code's own GUARD note calls this unreachable; verified true (zero multi-line block comments across
  all 21 sources), so plausibility is genuinely low. The note is honest.
- **`MAX_WARM_PAGES = 6` sits entirely outside the budget system built to bound it** (CONFIRMED).
  `exec_pool.rs:14` vs `build_budget.rs:129-132`. On a starved box `preview_warm_pool_size()`
  correctly computes 0, then `ExecPool` next door holds 6 executors x 2 kernels + the ~1 GB
  torch-preloaded daemon. Eviction fires only from `get()`, never on idle or memory pressure: a
  preview left open overnight holds all of them. **Do-NOT-touch zone.**
- **RAM probe fails OPEN in a container** (CONFIRMED by construction). `build_budget.rs:36-46`
  reads `/proc/meminfo` (host-wide); no cgroup read exists anywhere. `available_parallelism()`
  (`:68`) *does* honour cgroup CPU quota, so the budget is CPU-correct and RAM-wrong in one breath.
  cpu-quota 16 + `memory.max=2G` on a 128 GB host -> `cap=16` -> 14 kernels x 150 MB -> OOM killer,
  from the module whose purpose is preventing it. `probe_free_mb` has zero tests. Also single-shot:
  a momentary RAM dip pins a 10-minute build to `cap=2` for its duration. **Do-NOT-touch zone.**
- **`warm_one` leaks `/tmp/tali-kernel-<uuid>/` forever on the fork-failure path** (CONFIRMED).
  `warm_pool.rs:651-652` holds `conn_dir` as a plain `PathBuf` across a fallible `fork_kernel`.
  Commit `7e711fe` added `ConnDirGuard` to `Kernel::start` and `ForkedCleanup` to `adopt_forked` and
  missed the sibling between them. Leaves a 0700 dir with a 0600 `connection.json` (and its HMAC
  key) orphaned per failure. Additional graceful-path source for the 21 leftover dirs / 77 MB the
  07-09 note attributed entirely to ungraceful death. **Do-NOT-touch zone.**
- **`PER_KERNEL_MB = 150` does not model the daemon** (PLAUSIBLE). `build_budget.rs:61` prices the
  pool as `n x 150 MB`; the real cost is `daemon(~1 GB with torch) + n x COW-delta`. Wrong in both
  directions; the daemon is budgeted at zero. Nothing measures actual RSS.

### The silent-latch class (one transient failure -> permanent silent degradation)

This is the shape the lens exists to find: a failure that produces no symptom anyone would notice,
so nothing ever latches back.

- **`interp_id` hangs the whole rebuild pipeline, forever** (CONFIRMED, **reproduced**).
  `exec.rs:971` runs blocking `std::process::Command::new(program).arg("--version").output()` with
  no timeout, no `spawn_blocking`, no `kill_on_drop`, called from `compute_outputs` (`:409`)
  **before** `ensure_kernel` (`:466`). Every timeout you built lives downstream of the thing that
  hangs. Reproduced with a fake interpreter that sleeps on `--version`: `build` was killed by an
  external 20s timeout while `TALIESIN_CELL_TIMEOUT=5` never fired. Both rebuild loops are single
  sequential consumers (`serve/mod.rs:1072-1088`, `serve_site/mod.rs:823-839`), so this wedges that
  document's/site's entire pipeline for every future edit; only a process restart recovers.
  Realistic triggers: a broken conda/pyenv shim, a wrapper waiting on stdin, a stalled network
  mount. **Do-NOT-touch zone.**
- **`interp_id` memoizes an empty version on a transient failure** (CONFIRMED). Same function:
  `.ok()` + `.unwrap_or_default()` caches `""` in a `OnceLock` for the process lifetime. That string
  feeds the freeze key, so two interpreters that both failed `--version` produce identical keys: a
  narrow but real hole in the "stale hit impossible by construction" claim. **Do-NOT-touch zone.**
- **The warm pool goes permanently dark after one fork hiccup** (CONFIRMED). `refill()` has exactly
  two callers (`warm_pool.rs:461` in `new()`, `:483` in `take()`); `take()` only refills
  `if kernel.is_some()`; the `Err` arm logs one warning and `return`s (`:627-636`). Once a transient
  failure empties `ready`, refill is never called again. The comment says it stops "for now (the
  next `take` falls back to a cold start)": an intent the control flow does not deliver.
  Three amplifiers: (i) the **Python daemon has an explicit retry protocol the Rust client refuses
  to use** (`:128-138` catches the fork exception, writes `ERROR`, keeps looping, ready for the next
  request; Rust reads `ERROR` at `:348`, returns `Err`, never asks again: the two halves of one
  protocol disagree about whether a fork failure is terminal); (ii) **the cold path already retries
  this exact failure** (`exec.rs:790-804`, `START_ATTEMPTS = 4` + `start_error_is_transient`,
  written because `peek_ports` can hand two kernels the same port under concurrent builds) while
  `warm_one` calls the same `prepare_connection` with **zero** retries; (iii) the project's own test
  documents the trigger (`warm_pool.rs:897-901`: "the pre-warm fork can be flaky... ipykernel's
  stdout NOTE racing the SPAWNED protocol"). **Do-NOT-touch zone.**
- **`fork_kernel` desyncs permanently on timeout** (CONFIRMED code path, rare trigger, total
  consequence). `warm_pool.rs:317-363`: on `FORK_TIMEOUT` it returns `Err` without draining the
  daemon's eventual `SPAWNED <pid>`, which is then consumed as the *next* request's reply,
  permanently off-by-one. Every subsequent kernel carries the previous kernel's PID, so `is_alive()`
  probes the wrong process (self-healing never fires; every cell burns the full 120s timeout), SIGINT
  hits an innocent process while the real kernel stays wedged, and `Drop` SIGKILLs the wrong pid
  while the real one leaks. **Do-NOT-touch zone.**

**Sequencing warning (load-bearing).** The refill-dark bug, the fork desync, and the `warm_one`
`/tmp` leak share **one trigger: a failed fork**, and must be fixed as **one change**. Today the
refill `return` is what kills the task that would keep producing mis-paired kernels, so the bug
limits its own sequel's blast radius. Fixing refill by "just keep retrying" **without draining the
pipe** converts the desync from one bad kernel into every kernel.

### Test shape: why 1001 commits missed all of it

- `set_meta` has **zero** wire-shape coverage (`protocol.rs:196-203` producer,
  `client.js:1026-1039` consumer). Both `protocol_contract` modules assert `update`/`insert`/
  `remove` and skip it. Renaming the `"sourcepos"` literal compiles, passes all 178 tests, passes
  `tsc`, and silently degrades Alt-click to "opens at line 1" for every line-shifted block, plus
  wrong-file attribution for included blocks, plus dead reverse-sync. `live-edit-bench/RESULTS.md:23`
  measures a real edit as **55 ops: 54 of them `set_meta`**. `// @ts-check` gives false comfort: it
  validates `client.js` against its own typedef, which knows nothing of the Rust side.
- The refill tests **re-implement the loop they claim to pin** (`warm_pool.rs:677`), with no error
  arm, so they structurally cannot express the bug. `refill()` itself has zero coverage.
- The fallback test forces the wrong branch (`:808` uses `/nonexistent/...` -> `daemon = None`,
  testing daemon-absent). The daemon-present + fork-fails branch has no test.
- `exec_pool`'s tests admit they cannot test their own claim (`:106-108`: "`Executor::new()` doesn't
  spawn a kernel... so this exercises the eviction logic kernel-free"), so the doc claim at `:17`
  ("dropping an executor kills its kernel child processes") is asserted by comment and tested by
  nothing.
- `explicit_jobs_is_respected` (`build_budget.rs:144`) **certifies** the `--jobs` defect;
  `split_never_exceeds_budget_and_build_is_at_least_one` (`:206`) asserts only `>= 1`, the weakest
  possible floor, which the `3 -> 1` collapse satisfies.
- No test parses, executes, or behavior-compares minified output; all 17 are `contains`/`count`
  shape checks. The strongest (`minify.rs:586`) reads only `code-enhance/`, 1 of the **7** sources
  `core_enhance_js()` concatenates, leaving **`search.js`, the most regex-heavy file, unguarded**.
- No emitted PNG is ever decoded or golden-compared. `png_dims` reads raw IHDR bytes at fixed
  offsets; the determinism test compares two in-process renders (both wrong identically). A change
  rendering every card blank, all-tofu, or colliding passes all 13 tests.
- **R has zero live CI coverage** (CONFIRMED). `ci.yml` installs only Python; no `setup-r`, no
  `TALIESIN_R`-gated test, no assertion on `KernelSpec::r()`'s argv. Meanwhile the Python job sets
  `TALIESIN_REQUIRE_KERNEL=1` and the canary (`kernel.rs:1156-1160`) **hard-fails** if the
  interpreter goes missing, a guard built precisely to stop coverage silently regressing to zero.
  README advertises `{r}` cells and `TALIESIN_R` as first-class. The guard exists for the language
  the author looks at.

-----------------------------------------------------------------------------

## ROT flags (verified; do not re-scope)

- **The `in_flight` slot leak is FIXED.** Commit `7e711fe` added `SlotReservation`
  (`warm_pool.rs:403-419`). Re-derived: the increment -> guard-construction window contains no
  `await` and no fallible call; the `None` arm returns before incrementing; `drop` fires on both
  arms; `saturating_sub` floors at 0; a poisoned lock is absorbed. Cancellation drops the guard.
  Panic-tested at `:836`. **It cannot leak or mis-count today.** Any live claim otherwise is stale.
- **`TALIESIN_NO_EXEC` is still not consulted by `build.rs`/`warm_pool.rs`: the 07-09 claim is
  upheld, NOT rot.** Verified by execution (`--jobs` runs under `TALIESIN_NO_EXEC=1` still logged
  "pre-warming 2 kernel(s)"). Its cited line (`build.rs:926`) *has* rotted; the boot is now
  `build.rs:1366`. Symptom right, line number wrong, exactly as the anti-rot discipline predicts.
- **`minify.rs:41`'s `url()` justification is rot** (40 of 60 real `url(` are not data URIs).
- **`card.rs:308`'s ">40px clearance" is rot**: measured worst case is **13px** (lowest ink y=527
  vs rule y=540). Safe today; the margin justifying the 0.76 baseline approximation is 3x thinner
  than documented.

## What held up under attack (negative space)

- **The two servers do NOT drift at the message-shape level.** The stated "two owners" worry is
  **refuted**: both route every server->client message through `protocol.rs`; `Broadcast::messages`
  genuinely centralizes the load-bearing body->style->diagnostics ordering; all `protocol::` call
  sites diffed with no field or variant asymmetry. The real asymmetries are semantic (`remount`
  triggers, `full_render.title` provenance), not structural.
- **Serde hazards: the category does not apply.** `protocol.rs` has no `#[serde(rename)]`/`tag`/
  `untagged`; it hand-builds JSON via `json!` with string literals, so a Rust field rename is a
  *compile error*, not silent wire drift. The risk is inverted from the hypothesis.
- **No draft or private-page leak anywhere.** The finding most expected to break. A `draft: true`
  post, a `_private/` dir and a `.hidden/` dir all built with unique markers; `grep -rl` across every
  emitted file (sitemap, llms.txt, llms-full.txt, search-index.js, feeds, HTML) -> **zero hits**.
  Verified per generator: `DraftMode::Exclude` drops drafts at `discovery.rs:28` (and `book.rs:148`)
  *before* `self.pages` exists, so the machine-facing generators cannot leak one by construction.
- **The `search.rs:159` U+2028 bug does NOT replicate in JSON-LD.** Browser-verified: raw
  U+2028/U+2029 in a description parse fine, because JSON-LD is consumed by a JSON parser (where
  they are legal raw), not eval'd as a JS literal. Correct call, not a latent copy.
- **MCP protocol handling is genuinely robust.** Malformed JSON -> `-32700`, loop survives; unknown
  method -> `-32601`; unknown tool -> `-32602`; `ping` answered after all of it; exit 0; stdout stays
  a pure JSON-RPC stream; every tool wrapped in `serve::guarded`. Errors echo only caller-supplied
  paths.
- **`card.rs` cannot panic.** 16 hostile inputs fuzzed (empty, control chars, ZWSP, U+202E, stacked
  combining marks, 500 emoji, a 20,000-char word, 5,000 words, `ﷺ`): zero panics, every PNG decoded
  clean at 1200x630. `wrap_clamp`'s `pop().unwrap()` correctly guarded; casts saturate; `blend`
  bounds-checks. It uses the `png` crate, not a hand-rolled encoder (a premise of the brief, refuted).
- **The minifier's damage is bounded by construction.** It only ever removes comments and
  whitespace, so a misclassified state usually degrades to identity. Damage needs a state that
  wrongly *includes* `Normal`. Verified correct: `a / b / c` chains, `return /x/`, division after a
  regex, keyword-before-regex, `<!--`/`-->` in JS, ASI `return`\n`1`, `//` in strings, `/[/]/`,
  one-level `${}`, escaped `\)`, comment-inside-string, `content:"{;}"`, `@media`/`!important`.
- **`budget_split` has no underflow/overflow/div-by-zero** on any core/RAM combination.
  `exec_pool` has no off-by-one and no concurrency hazard (owned by the single builder task, reached
  only through `&mut`).
- **`boot_id` exceeding `Number.MAX_SAFE_INTEGER` is not a bug.** Chased expecting a false match: both
  sides serialize the same u64 to the same decimal digits, JS parses both to the same double, so the
  comparison is preserved, and restarts are seconds apart against 256ns rounding.
- **The shipping bundles are clean today.** Original-vs-minified diffed through acorn (token stream
  + AST) and css-tree: **token-identical and AST-identical**. Every minifier finding is latent.
- **`taliesin build` output is deterministic and the shipping cards are fine**: all 153 real card-text
  fields scanned against actual glyph coverage -> 0 hits.

-----------------------------------------------------------------------------

## Provenance

Four read-only parallel agents (protocol / minify+card / llms+mcp+seo / resource managers), plus
direct verification in the main session: the `--jobs` collapse reproduced on the release binary, the
`interp_id` wedge reproduced with a hanging fake interpreter, the MCP read executed over real stdio
JSON-RPC, the `llms-full.txt` fusion confirmed on production content, and the title-clobber
mechanism read at all three sites. Two agent findings corrected the session's own earlier claims:
`node --check` is not evidence of minifier correctness (proven: identical token count, changed token
value), and `card.rs` does not hand-roll PNG encoding.
