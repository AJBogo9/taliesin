# MVP audit backlog, 2026-08-13

Durable state for the post-cut defect sweep. **This file is the handoff.** A fresh session
needs only this file: every item carries its own anchor, its own reproduction command and
its own done-condition, so none of them requires the conversation that produced it.

Produced by a 48-agent audit at `aceb566b` (5 scope lenses + 8 subsystem bug finders, one
adversarial refuter per finding). 34 candidate defects, **2 refuted** (recorded in
[DO-NOT-REBUILD.md](DO-NOT-REBUILD.md), do not re-file), 32 confirmed, **29 distinct**
after removing three cross-area duplicates. Plus 36 scope findings, of which 16 survived
synthesis as real work.

## The verdict this came with

**The scope is right. The shipping surface has not caught up.** The seven-verb CLI covers
the whole advertised journey with no hole, all three load-bearing goals were exercised live
and are intact, and an independent scan found the surviving document vocabulary **fully
witnessed** (every offered name in real use; the residual unused tail is two names, not the
ten on record). There is no wave 14 hiding in the feature list. **Do not cut another
feature.** What is left to cut is the layer that was never gated: the README's phantom
bullets, three unread directories, and `notes/` itself.

The single sentence that carries it: **`README.md:157-165` advertises four features the
tool deleted, and `crates/core/tests/retired_names.rs:294` is named
`the_lightbox_is_gone_from_the_client_bundle`.** The test suite asserts the absence of
features the README advertises, and every gate is green.

## Baseline at `aceb566b`

| | |
|---|---|
| `cargo test --workspace` | **81 suites, 1,352 passed, 0 failed, 0 ignored, exit 0** (measured 2026-08-13, with `TALIESIN_PYTHON="$PWD/.venv/bin/python"`) |
| `./tools/gates.sh` | **NOT RUN for this audit.** It was held back so the eleven gates would not take the cargo lock while agents worked. Run it before the first batch to establish the real baseline. |
| Working tree | clean, untouched by the audit |

**Every defect below exists in a tree where those 1,352 tests pass.** That is the finding
behind the findings: the cut removed features and their tests together, correctly, but did
not add coverage where the *remaining* surface became load-bearing.

## Rules for working this file

1. **One batch per session, one branch, one commit.** The batches below are drawn so each
   is a coherent unit that touches related code.
2. `./tools/gates.sh` green before and after. It needs
   `TALIESIN_PYTHON="$PWD/.venv/bin/python"` or it exits 2 at preflight and certifies
   nothing. **Take the gate count from the script's own verdict line**, never from prose.
3. **Delete an item from this file when it lands.** Never a `[x]`, never a strikethrough.
   That is this project's standing rule and it is why its notes rot.
4. **Trust an item's symptom, never its cause, line number or cost.** Every anchor here was
   correct on 2026-08-13 and line numbers move. Grep the named symbol before pricing work.
5. A retirement costs **one register entry and nothing else**. Do not write a tombstone
   test for it.
6. A pin and its docs page are deleted in the **same commit** as their feature, never
   before.

### Hazards that apply to several batches

- **Any change that adds or removes a `corpus/**/*.tmd` re-arms the census gate.** Gate 11
  (`tools/portability-census.py --verify`) asserts that `README.md` and
  `docs/guide/using/choosing.tmd` still publish the document count, line count,
  beyond-CommonMark count, percentage, complement and all six per-family pairs. Re-run
  `python3 tools/portability-census.py`, copy the figures into both pages, then `--verify`,
  in the same commit. This is mechanical but not optional.
- **Editing `crates/core/assets/css/*` or `assets/js/*` needs a `cargo build` before the
  change shows up.** They are `include_str!`-compiled, so rebuilding only the site re-emits
  the old bundled CSS/JS and you will measure a stale page.
- **`target/release/taliesin` is shared across sessions.** Check `taliesin help`'s version
  line against your own HEAD before trusting any CLI measurement. At `aceb566b` it reports
  `0.2.0 (0178e403)`; `git diff 0178e403..HEAD -- crates/ web-client/` is empty, so it is
  behaviourally current and only the version string lags.
- **`notes/backlog.md`'s "Standing constraints" section is itself stale** and will mislead a
  session that reads it: it names `taliesin features` (cut wave 2), "four gates" (there are
  eleven), "FIVE drift gates / EIGHT for a retired key" (now four and one), and owes a
  four-projection sweep to `taliesin read`, `skim.rs` and `llms-full.txt`, all three cut.
  Filed as **S18**.

## Confidence key

- **[V]** reproduced by the orchestrator directly, command and output in hand.
- **[A]** reproduced by an agent that quoted its command and output, then survived an
  adversarial refuter briefed to kill it.

Where a refuter narrowed or widened a finding, the corrected statement is what is written
here, not the finder's original.

---

# BATCH 3: the freeze cache can publish a wrong number

Highest consequence in the tree, because the page looks fine. Read
`notes/CUT-PROGRESS.md`'s wave 11 entry before touching `exec.rs`: a flaky test there once
accused `kernel.rs`'s SIGINT path, which was innocent.

## A6 [A] HIGH: deleting a `#| cache: false` line permanently freezes a self-contradictory page

Root cause is the persist loop at `crates/server/src/exec.rs:931`, **not** `plan`'s
`run_end` extension at `:1283` (the finder anchored the mask, the refuter found the write).

`strip_cell_options` (`crates/core/src/render/cell_extract.rs:138`) drops `#|` lines before
hashing, so `cache:` is a field and never part of the hashed code. Cells downstream of a
`cache: false` cell are therefore persisted under a cumulative key asserting "this output
follows from this upstream code" while the upstream value was never hashed. Those entries
are already false when written; `plan`'s extension is only a runtime mask over them, valid
exactly as long as the directive is present.

**Reproduced:** cell A is `#| cache: false` + `x = random.randint(...)` + print; cell B
prints `x`. Build twice, self-consistent both times. Delete only the `#| cache: false` line,
no code change. Build again: A re-runs (never persisted) while B restores a disk entry from
a different upstream value. The page reads `upstream x = 451379` and
`downstream sees x = 837111`, and stays that way across rebuilds.

**Fix direction:** the `cache` flag must participate in the cumulative key of every
downstream cell, or downstream outputs of an uncacheable cell must not be persisted at all.
The second is simpler and matches the existing "errors are never persisted" rule.

**Done when:** the reproduction above produces a self-consistent page after the directive is
deleted, pinned by a test in the `freeze_cold_replay.rs` family.

## A7 [A] HIGH: the package-change warning can never fire on a mixed run

`crates/server/src/exec.rs:938` calls `stamp_packages` (which writes the current digest)
before `:997` calls `warn_if_packages_moved` (which reads it), so
`packages::crossed(recorded, now)` compares the digest against itself. `packages::manifest`
is memoized process-wide, so the two strings are identical by construction.

The recorded digest is the **one axis the freeze key cannot see**, which is the entire
reason `packages.rs` exists. A replay that crossed a `pip install --upgrade` says nothing.

**Trigger is narrower than "any mixed run":** the disk-restored tail must lie after the last
executed cell, which in practice needs an upstream cell whose output was never persisted (an
error, or a truncated output via `is_uncacheable` at `exec.rs:1311`) sitting before at least
one disk-cached cell. A `#| cache: false` cell does **not** trigger it, because
`exec.rs:1283-1285` forces `run_end = hashes.len()`.

**Fix:** move the stamp after the comparison, or pass the pre-stamp digest to the warning.

---

# BATCH 4: source mapping, load-bearing goal #1

## A8 [A] HIGH: render warnings carry the expanded-buffer line, not the file's own

`crates/core/src/render/mod.rs:761`. The per-block tuple binds `buf_start = sp.start.line`
(post-include buffer coordinates) while binding `source_file` to the **mapped** origin file.
Both coordinate systems are computed correctly at `:683-684` and used correctly for
`data-sourcepos` at `:685-688`; only the warning tuple mixes them. Ten warning sites then
emit `.at(source_file, buf_start)`, including duplicate heading id (`:841`), duplicate
cross-reference label, and `register_xref` (`:1039`).

A 7-line partial reports `_dup.tmd:49`. Ctrl-click on that diagnostic, and the LSP squiggle
it drives, land 44 lines past the end of the file.

**Wider than the finder said:** any include shifts every later buffer line, so the **parent**
document's own warnings after an include are mislocated too. There `source_file` is `None`,
so the diagnostic carries the real, openable path with a wrong line, and the author lands 8
lines off in the file they are editing with nothing signalling it. That is the worse half.

## A9 [A] MEDIUM: a block spanning an include boundary emits an inverted `data-sourcepos`

`crates/core/src/render/mod.rs:684`. `map_origin` is applied independently to `sp.start.line`
and `sp.end.line`, and only the start's file is kept for `data-source-file`, so a paragraph
that straddles the boundary (included file's last line non-blank, parent's next line
non-blank, so comrak merges them) emits a range mixing two files' line numbers.

Measured: `data-sourcepos="39:1-6:25" data-source-file="x.tmd"`. End line 6 precedes start
line 39, violating `crates/core/tests/corpus.rs:411`'s own `sl <= el` assertion. No corpus
document happens to trip it (swept all 82, zero hits). Where it does not invert it merely
runs past the included file's EOF.

Downstream, `web-client/client.js`'s `highlightAtLine` skips such a block.

## A10 [A] MEDIUM: duplicate explicit anchors emit duplicate HTML element ids

`crates/core/src/render/figure.rs:81` writes `fig.attrs.id` straight into the element, while
the heading path at `render/mod.rs:836-846` routes an explicit `{#id}` through
`dedup_with_suffix`.

One partial included twice (the documented use: `shortcodes.tmd:44` advertises includes as
the way to keep one shared figure in one file) produces `<h2 id="sec-shared">` and
`<h2 id="sec-shared-1">` correctly, but `<figure id="fig-shared">` twice.

**Not figure-specific.** The same behaviour is at `figure.rs:142`, `cell_numbered.rs:111`
and `:130`, `divs.rs:609`, and `mod.rs:2539`: every explicit author-written id is emitted
verbatim, and only headings dedupe, because only headings get an id the author never asked
for. The condition **is** diagnosed at error severity with a location, and `--check-only`
catches it; plain `build` exits 0 while emitting invalid HTML.

**Decide before fixing:** deduping an author's explicit id silently breaks the anchor they
wrote. Refusing the build may be the better answer than renaming.

---

# BATCH 5: the browser client, which has no automated test net at all

Wave 6 deleted `chromiumoxide`, the `headless-js` feature and `reactive_browser.rs`
together. Nothing in this batch has any test coverage. `notes/CUT-PROGRESS.md`'s open hedge
"decide whether to keep one browser smoke test" is the standing question; A11 is the
argument for yes.

**Note:** the specific hedge on record, "nothing tests that a `{js}` cell's teardown runs on
a block diff", was **exercised and is correct today** (mounts 1 to 2, teardowns 0 to 1, clean
console). The gap is coverage, not behaviour. A11 is a different leak in the same file.

## A11 [A] HIGH: `tali.onInput` callbacks are never removed on cell teardown

`crates/core/assets/js/tali-js.js:386`. `teardownIn` splices the cell out of `r.cells`,
calls `dispose()` and deletes its `r.inputs` entry, but never touches `r.listeners`, the
Set-of-callbacks map that the public `tali.onInput` API writes into at `:201-205` and that
`registerInput`'s DOM handler fans out to at `:150-151`.

Measured in Chrome: after three edits to a `{js}` cell, `__talijs.cells.length` is correctly
2 but `__talijs.listeners.k.size === 4`. One slider event now runs the callback four times,
three of them against detached DOM the closure keeps alive (`container.isConnected === false`).
Every edit multiplies the work done per input event, permanently, for the session.

**Fix:** have `teardownIn` remove the torn-down cell's callbacks. This needs the listener
registration to record which cell owns each callback.

## A12 [A] MEDIUM: every block op resets horizontal scroll to 0

`web-client/client.js:1025`. `keepScroll` saves only `window.scrollY` and restores with
`scrollTo({ top: y, left: 0 })`. `left: 0` is unconditional across all four callers
(`full_render` :1198, `update` :1223, `insert` :1245, `remove` :1262).

This contradicts the client's own "scroll position survives edits" contract. Reaching the
state needs a document that scrolls horizontally, which Taliesin's own CSS clamps, so in
practice it is raw HTML with an explicit oversized width, a width-escape div, or a narrow or
zoomed viewport. One-line fix.

## A13 [A] MEDIUM: preview and build render different tables of contents

`web-client/client.js:954`. `buildToc` collects with
`root.querySelectorAll("h1[id],...,h6[id])`, which descends into `:::` container blocks,
while the server's `render::toc_items` (`crates/core/src/render/mod.rs:2579`) iterates only
top-level blocks. A heading inside a non-callout fenced div is therefore in the preview's TOC
and absent from the build's. The two also compute the level window from different sets, so
indentation can differ as well.

**Callout divs do not trigger it** (a callout consumes its headings into the title and they
never get an id). No page in the repo hits it today.

## A14 [V] MEDIUM: `build <file.tmd>` drops the auto-TOC that `preview <file.tmd>` renders

The single-file build is the only page path that never constructs a `Site`, so
`Site::page_toc`'s auto-gate never runs and `render/mod.rs:1318` leaves
`toc: toc_explicit.unwrap_or(false)`. `crates/server/src/build.rs` never calls `page_toc` at
all.

```sh
# same 3-heading paper.tmd, same binary
build paper.tmd --stdout  -> 0 TOC nodes
preview paper.tmd (curl)  -> 2 TOC nodes
paper.tmd + toc: true     -> 1
```

**And the manual states the opposite.** `docs/guide/reference/frontmatter.tmd:72`: "Leave it
out and it is **automatic**: an article page with enough headings gets one."

Inside a project the divergence is wider: `preview p3.tmd` resolves to the enclosing project
and renders the site nav; `build p3.tmd` renders neither nav nor TOC; `build .` renders both.
So `CLAUDE.md:164-165`'s claim that the two verbs "now agree on page chrome" holds only for a
file with no ancestor `_site.yml`.

**Two fixes, pick one:** route `build.rs:749` through `Site::discover_single` (about 5 lines,
makes the tool match the manual) or amend the manual sentence (1 line). Wave 13 aligned the
two verbs on navbar and footer and did not route the build side the same way; finishing that
alignment is the in-doctrine answer. **Update `CLAUDE.md` either way.**

---

# BATCH 6: dev-server plumbing

## A15 [A] MEDIUM: a page whose name contains `&`, `+`, `#` or `%` never hot-reloads

`crates/server/src/serve_site/mod.rs:817`. `encode_query` percent-encodes U+0020 only, and
its doc comment ("spaces only; `/` and `-` are query-safe") states an alphabet that is
wrong. The page ships `window.TALIESIN_WS_PATH = "/ws?page=q&a.tmd"`; axum splits on `&`, so
`page` arrives as `"q"`, `resolve_page_rel` returns None, and `client_conn` refuses and
closes. `+` is worse: it decodes to a space silently. `client.js:1345` then reconnects every
second forever.

The page renders fine at 200 and the status pill stays green, so the failure is silent.
Preview only; `build` ships no ws path.

## A16 [A] MEDIUM: a project under an ancestor named `_site`/`_book`/`_freeze`/`.git`/`node_modules` never hot-reloads

`crates/server/src/serve/mod.rs:529`. `relevant_path` computes `in_skip_dir` with
`p.components().any(...)` over the **entire** path, while `spawn_watcher`
(`serve_site/mod.rs:1413`) hands it the notify event path rooted at the canonicalized project
root. Watches are registered and events do arrive; they are then silently vetoed by the
ancestor-component scan.

**Exotic but total and silent.** Inside a normally-located project the check is redundant,
because `watch_tree` already prunes those directories so no inotify watch exists inside them.
Fix by making the scan relative to the project root.

## A17 [A] MEDIUM: `restart_kernel` from one page SIGINTs a cell running on another page

`crates/server/src/serve_site/mod.rs:909`. `SiteApp::interrupt` is one pool-wide
`Arc<AtomicU32>` set on every executor (`exec_pool.rs:82-84`); its own doc says it holds "the
pid of the cell currently executing anywhere in this pool". The ws arm SIGINTs whatever pid
it holds, then sends `BuildMsg::Restart(rel)`, which restarts only the requesting page.

Reproduced live: page A's 45s cell died with `KeyboardInterrupt` about 1s after page B sent
`restart_kernel`; B's kernel was the one restarted, so A was never re-run and its output
block was left holding the traceback.

**A page-equality check is the wrong fix.** The refuter established that the global read is
deliberate and load-bearing: the exec lane is serial, so when page A's runaway cell wedges
the queue, page B's own Restart is queued behind that same build and only the server-wide
SIGINT can unwedge it. Scoping the interrupt to the requesting page would restore the
wedge for every page except the one that owns the runaway cell. **Design the fix; do not
patch it.** Only applies to multi-page projects (`preview <file.tmd>` synthesises a one-page
project, where the pool-wide pid is by construction the page's own).

## A18 [A] LOW: both IPv6 arms of `origin_allowed` are dead code

`crates/server/src/serve/security.rs:26`. `authority.split(':').next()` can never return a
string containing a colon, so the `"::1"` and `"[::1]"` arms at `:27` are unreachable for
**every** possible input, not only IPv6 ones. `"[::1]:9999"` yields `host_only == "["`.

**No shipping component hits it:** the web client's socket is same-origin and the VS Code
companion addresses 127.0.0.1. So this is either a two-line fix or a two-line deletion, and
the standing directive argues for deletion unless a local peer on `[::1]` is a case worth
keeping.

---

# BATCH 7: the LSP

## A19 [A] MEDIUM: a lone `\r` desyncs every LSP line index

`crates/server/src/lsp_pos.rs:19` and six other sites split on `'\n'` only
(`lsp_nav::classify_target:170`, `lsp_diag::diagnose_file:49`, `lsp_outline::headings:78`,
`lsp_cells::cell_regions:30`, `lsp.rs:1082`), while comrak treats a bare CR as a line ending
per CommonMark. Confirmed independently of the LSP: `para one\rpara two` emits
`data-sourcepos="5:1-6:8"`, two lines, while `text.split('\n')` sees one.

`Diagnostic::to_lsp` (`crates/server/src/lint.rs:87-107`) mixes both models in one function:
the line number comes from comrak, the line *text* used to size the range comes from the
`\n` split. After one lone CR (pasted terminal output is the realistic source) F12 lands on
an empty line, hover answers about a neighbour, and whole-line squiggles collapse to zero
width.

## A20 [A] MEDIUM: the every-keystroke diagnostic publish runs outside the panic guard

`crates/server/src/lsp.rs:241`. Commit `5f2fc9fc` moved the didChange publish out of
`handle_notification` (wrapped in `crate::serve::guarded` at `:352-364`) into `main_loop`'s
`Batch::Timeout` arm, where it calls `publish(...)?` unwrapped. The identical call arriving
via `didOpen` at `:523` is caught and logged.

A panic under `render_single_doc` (`lint.rs:318`) or a validator on a half-typed buffer would
now take the whole language server down mid-session, and an `Err` is fatal here
(`?` to `ExitCode::FAILURE`) but merely logged on the `didOpen` path.

**Latent hardening regression plus a stale test comment**, not a live crash: no panic is
known. The comment at `lsp.rs:2149-2151` asserts coverage that no longer exists, so fix the
comment in the same commit.

## A21 [A] LOW: a transport read error exits 1 with nothing on stderr

`crates/server/src/lsp.rs:67`. `if io_threads.join().is_err() { return ExitCode::FAILURE }`
discards the `io::Error`. It is the only error path in `cmd_lsp` that does not call
`crate::log::error` (contrast `:63-65`), and it contradicts the function's own doc comment
at `:58-59` ("logs any error to stderr").

Triggered by a header not terminated with `\r\n`, a non-UTF-8 or non-JSON body, or a
truncated body from a miscounted Content-Length. The author sees the server die repeatedly
with no message, and `lsp.rs:274-283` already reasons that a non-zero exit counts toward VS
Code's "server crashed 5 times" cutoff. Diagnostic-only. One-line fix.

---

# BATCH 8: diagnostics that lie

The project's stated standard is that a diagnostic names a defect the author cannot see in
the rendered page. These four say something false.

## A22 [V] MEDIUM: the retired `listing: sort:` key is still honored

`crates/core/src/site/frontmatter.rs:177`. The tool prints:

> unknown listing key `sort`: it was removed on 2026-08-02: newest first is the only order
> now, so delete the key

and the key is live. Measured: with `sort: "date asc"` the cards render Oldest then Newest;
delete the key as instructed and they flip to Newest then Oldest, with no diagnostic on
either side. `docs/guide/reference/frontmatter.tmd:190` also states the key does not exist.

**Narrower than "fully live":** the only effect is one boolean, `!value.contains("asc")`, so
it can only reverse the always-by-`date` order. `sort: "date desc"` is indistinguishable from
absence, and any value containing the substring "asc" (`"title ascending"` was verified)
flips it.

**Wider in one respect:** the same `Site::collection` is called by `site/feed.rs:100`, so the
Atom feed comes out oldest-first too, which is wrong for a feed independent of any doc claim.

**Fix (follows register doctrine):** delete `frontmatter.rs:177` and the `sort_desc` field
(`site/mod.rs:110`, set only at `frontmatter.rs:185`), and make `site/mod.rs:1290` an
unconditional `items.reverse()`. Optionally add a parser-side pin mirroring
`parse_hero_ignores_the_retired_image_keys`. No corpus or docs document uses `sort:`.

## A23 [A] MEDIUM: the `code-line-numbers` retirement note recommends a construct retired the same day

`crates/core/src/frontmatter.rs:147-148`. The note ends "a `.code-walkthrough` marks lines
from its own `.step lines=` and needs no cell option"; both `.code-walkthrough` and `.step`
are in `RETIRED_DIV_CLASSES` (`render/validate.rs:208`, `:218`), removed 2026-08-08. One
lint run emits the recommendation and the refusal together.

Introduced in cut wave 5, orphaned by cut wave 7 later the same day.

**Two aggravating claims do not hold:** the advice is not mechanically applicable
(`codes::extract_suggestion` at `diagnostics/helpers.rs:77` keys strictly on the literal
"did you mean `", which this note lacks), and it is one long sentence, not a paragraph.

**Fix:** replace the clause with the advice the `.code-walkthrough` entry already gives.
**The durable gap is that the derived tombstone test never validates that a note's
recommended construct is live**; consider closing that, since it is the same genus as A24.

## A24 [A] MINOR: `fig-export` was retired with no register entry, so it gets silence

Absent from `RETIRED_KEYS` and every other register, so a leftover `#| fig-export:` draws a
bare "unknown cell option" with no date and no successor. Every other retirement probed
(`run`, `check`, `publish`, `pdf`, `new page`, `--host`, `.theorem`, `titel`) behaves
correctly, so this is the outlier. One register line.

**Pair with S1**, which deletes the README bullet that still advertises it.

## A25 [V] MEDIUM: the tool warns about its own feed autodiscovery tag, falsely

`crates/server/src/build.rs:2274` treats any `href=` on a `<link>` as a view-time fetch, but
`crates/core/src/site/meta.rs:88` (`feed_head`) emits
`<link rel="alternate" type="application/atom+xml" href="<absolute feed url>">`.

Measured on `corpus/tech-blog`: 50 warn lines on a 17-page build, **34 of them** (2 per page,
including `404.tmd`) reading "external reference not bundled: ... the build will fetch it at
view time, so the output is not self-contained (offline viewing fails)". The claim is false
(a browser does not fetch a `rel="alternate"` feed link at view time, and the output is
self-contained), the author cannot change the generated tag, and the warning names source
files that contain nothing of the kind.

**It does not touch the pre-publish gate** (`--check-only` prints zero offline warnings), so
the cost lands on the deploy log, where it buries the 16 genuine warnings 2:1. Fires only
once `url:` is set, which is exactly the step an author takes to publish.

**Fix:** skip self-emitted `rel="alternate"` (and check `rel="canonical"`) in `external_refs`.

---

# BATCH 9: CLI and correctness miscellany

## A26 [V] MEDIUM: a single-dash token becomes the output path

`crates/server/src/build.rs:183`, guard at `:177`. `parse_build_args` rejects unknown flags
only when they start with `--`, so a single-dash token falls through to
`positionals.push(s)`.

```sh
$ taliesin build index.tmd -o out.html
  built   -o  ·  2ms
EXIT=0
$ ls
index.tmd  -o          # out.html silently discarded
```

`-o` is the output flag in essentially every other renderer, so it is a likely typo, and the
resulting dash-named file resists `rm`/`cat` without `--`.

`notes/CUT-PROGRESS.md:253-256` states this rule explicitly and it was walked into anyway.

**One finder claim is false:** a dash-prefixed path *can* be built via `./-weird.tmd`; only
the `--` sentinel is unsupported.

## A27 [A] MEDIUM: `doctor`'s `config` row prints "`_site.yml` is valid" on a file `--check-only` rejects

`crates/server/src/doctor.rs:286` filters `site.warnings` with `is_malformed_config_warning`
only (a YAML parse failure), so unknown or typo'd keys and the scheme-less `url:` warning all
leave a green tick.

`titel: My Site` plus `navbar:` gives `✓ config _site.yml is valid`, exit 0, while
`build . --check-only` on the same file reports two errors and exits 1. The site title
silently falls back to a default.

**The wave P5 `env`-row analogy is imprecise:** that row was hard-coded `Status::Ok` and could
never vary. This row *can* vary. The defect is an overclaiming detail string, not a frozen
tick.

## A28 [A] MEDIUM: a `.tmd?query` link publishes the raw source, leaking draft content

`crates/core/src/site/links.rs:39`. `tmd_href` splits on `#` only, never on `?`, so
`strip_source_ext("post.tmd?v=2")` returns None and the link round-trips unrewritten. The
surviving `.tmd` href then drives `deploy_referenced_sources` (`build.rs:857-888`) to copy
the raw markdown into the deploy, because `.tmd` is in `SKIP_EXT`.

`--check-only --strict` exits 0, because `manual_local_links` (`links.rs:270`) deliberately
strips the query and resolves to the real page.

**Raised severity by the refuter:** this leaks `draft:` content. Proven with
`[secret](secret.tmd?v=2)` where `secret.tmd` has `draft: true`: the build writes no
`secret.html` but does write `secret.tmd`, containing the unpublished text.

## A29 [A] MEDIUM: `expand_shortcodes` tracks fences with a boolean toggle

`crates/core/src/render/extension/mod.rs:30` flips `in_code` on any line starting with
` ``` ` or `~~~`, with no fence character and no run length. An inner fence inside a longer
outer fence closes the region early, so a shortcode documented inside a nested code sample is
expanded into live markup and draws a spurious diagnostic.

**Second, worse, silent failure mode the finder missed:** the desync runs both ways. When the
mis-classified region has an odd number of inner fence lines, `in_code` sticks **true** for
the rest of the document and a genuine shortcode is silently not expanded.

**The correct helper already exists twice in the same crate** and is used by the two other
line-scanning passes over the same buffer: `includes.rs:281` `next_code_state` (with
`code_fence` at `:266` capturing `(char, usize)`) and a copy at `render/divs.rs:155`.
`extension/mod.rs` is the one pass that did not adopt it. **Reuse, do not write a third.**

## A30 [A] MEDIUM: the VS Code `input` snippet writes the type positionally

`editor/vscode/snippets/tmd.json:66` emits `{{< input number name=x ... >}}` instead of
`type=number`. `input_shortcode` reads only `shortcode_named(args, "type")`
(`render/extension/mod.rs:185`) and falls back to `"slider"`, and unknown positional args
draw no diagnostic.

Picking `number`, `checkbox`, `text` or `select` from the snippet's choice list yields a
range slider, silently. Wave 6 hand-edited this very line to drop `range` from the choices
without noticing the syntax.

**Narrower than stated:** the default (first) choice is `slider`, which is correct, so only
an author who actively picks another is bitten, and the preview visibly shows the wrong
control (the *diagnostic* is what is silent). No source in the repo is affected.

**Fix:** `type=${1|slider,number,checkbox,text,select|}`.
`editor/vscode/src/test/manifest.test.ts` gates snippet callout kinds, div classes, cell
options and xref prefixes against the Rust consts but never `INPUT_TYPES` or the argument
form; extending it is the durable fix.

---

# BATCH 10: the shipping surface

Under 30 lines of edits, and it clears every product-facing incoherence the audit found.
Nothing here is a code change. **No gate reads any of it**, which is why it drifted:
`stale_docs.rs`'s `documented_cli_flags_exist_in_the_cli` is deliberately scoped to
`docs/guide/reference/cli.tmd`, and `shipped_docs_do_not_use_a_retired_front_matter_key`
matches a retired key only at column 0.

## S1 [V] BLOCKING: `README.md:157-165` advertises four removed features

| Advertised | Reality |
|---|---|
| `@thm-` cross-references (`:158`) | in `RETIRED_XREF_PREFIXES` (`cite/render.rs:54`); no target can be defined |
| `#| fig-export: x.pdf`, a headline bullet with two lines of prose (`:160-162`) | retired; the only mention in `crates/` is a test comment calling it retired |
| "a figure lightbox" (`:163`) | `crates/core/tests/retired_names.rs:294` is `the_lightbox_is_gone_from_the_client_bundle` |
| "mobile TOC pull-up sheet" (`:164-165`) | zero hits; `render/tests.rs:3515` records both it and the lightbox as deleted |

`.github/workflows/release.yml` copies `README.md` into every release tarball, so this ships
with the binary. Delete about 9 lines.

## S2 [V] MAJOR: `README.md:5-6` promises a `tali` alias no build produces

`crates/server/Cargo.toml:10-12` declares one `[[bin]] name = "taliesin"`; `release.yml`
packages only that; nothing creates a symlink. The only `tali` on this machine is an
uncommitted hand-made symlink from 2026-07-02. A reader who types the shorter spelling the
README just taught them gets command-not-found on their first command. Delete 1 line, or
ship the alias.

## S3 [V] MAJOR: three shipped pages advertise R cells

`site/features.tmd:77` ("Python and R cells run against the warm kernel"),
`site/index.tmd:91` ("executable Python/R cells"), `docs/guide/index.tmd:98` ("Python/R
cells"). `{r}` was cut in wave 6 and is in `RETIRED_CELL_LANGS`; a reader who follows the
copy gets a warning and an unexecuted block. About 3 lines.

## S4 [A] MAJOR: the guide promises prebuilt tarballs unconditionally

`docs/guide/using/getting-started.tmd:10-13` opens Install with "Every tagged release
attaches a prebuilt `.tar.gz` ... Unpack it, put the `taliesin` binary on your `PATH`, and
skip the rest of this section." No tag has ever existed and `release.yml` has never run on
any repo state. `README.md:73-77` was fixed in W1 to carry the caveat; the guide's copy was
not, and the guide is where the README's "Getting started" link lands. About 2 lines.

## S5 [A] MEDIUM: `docs/guide/reference/cli.tmd` still teaches the retired `prose-lint:` family

Four mentions, not three: `:63`, `:102` (the headline example of the `suggestion` severity),
`:141` (a dedicated row in the "everything the lint looks at" table), and `:109`'s sample
output ("weasel word `simply`"). Retired 2026-08-02; the binary now warns and
`--check-only` exits 1. The stale-doc gate cannot see it because all mentions are
inline-backticked inside prose and table cells.

## S6 [A] MINOR: `init`'s scaffold omits `url:`

The scaffold writes `_site.yml` = `title: My site` and an `index.tmd` carrying `listing:`
with dated posts, i.e. explicitly a blog, which is the one shape that wants a feed. Feeds,
`sitemap.xml` and `robots.txt` are all gated on `canonical_base()`, so the entire
publish-adjacent surface is off by default and the build summary simply omits it.

One commented line (`url: "https://example.com"  # set this to publish a feed + sitemap`).
No new knob. **Fix A25 first or the author's experience of the feed is silence when off and
false alarms when on.**

## S7 [A] MAJOR: the shipped screencast teaches the wrong click-to-source gesture

`site/assets/live-edit-dark.mp4` is embedded on `site/index.tmd:77` and
`site/features.tmd:15`. A frame reads "Double-click any block in the preview and the editor
jumps to the exact line it came from". Double-click does nothing:
`web-client/client.js:1485` returns unless `ctrlKey || metaKey`. The gesture changed
2026-07-28 (`0ef2f509`); the videos are dated Jun 19.

Every *text* surface is correct (`README.md:12`, `site/features.tmd:40`,
`docs/guide/using/preview.tmd:29`, `editor/vscode/README.md:7`), so this is the one wrong
instruction, on the shop window, for load-bearing goal #1, in a binary asset no gate can
grep.

**Re-recording reproduces the bug:** `tools/record-demo/demos/live-edit.tmd:40` and
`sample.tmd:43` both still say "Double-click". Fix those in the same commit, or cut the
embeds (about 4 lines). **Interacts with S9**, which proposes deleting `tools/record-demo`.

## S8 [A] DECIDE: click-to-source is hard-coded to `vscode://`

`web-client/client.js:1415` and `:1461` do
`window.location.href = "vscode://file" + ...` unconditionally, with no config key, no env
var and no fallback. Meanwhile `taliesin lsp --help` sells "any LSP editor (Neovim, Helix,
Zed, VS Code)", `docs/guide/reference/cli.tmd:410` ships a Helix `languages.toml` snippet,
and the companion is not on the Marketplace.

So the day-one story for a non-VS-Code user is: the LSP works, the preview works, and the
feature the README numbers `1.` fails silently.

**The recommendation is one documentation sentence in `docs/guide/using/preview.tmd:29`, not
a knob.** "Minimal config: perfect the default before adding a knob" plus zero users decides
it. Recorded here because it is the sharpest MVP-boundary tension the audit found and the
author may disagree.

## S9 [A] NOTE: `build` prints an error-severity diagnostic as `warn` and exits 0

Unparseable YAML front matter is `error:` + exit 1 under `--check-only` and `warn` + exit 0
under plain `build`, which still writes the HTML. A first user who never learns about
`--check-only` publishes a page whose `title:`, `bibliography:` and `listing:` were silently
dropped, having seen "built" and a zero exit. The gate catches it, which is why this is a
note. Leave, or align the severity.

---

# BATCH 11: finish the subtraction

The cut stopped at the code's edge. These are the author's own untaken waves, re-verified at
`aceb566b`: not a line has moved.

## S10 [V] MAJOR: `tools/ui-audit` + `tools/record-demo` + `samples`, 3,755 tracked lines read by nothing

`grep -n 'ui-audit\|record-demo\|samples' tools/gates.sh .githooks/pre-push tools/build-site.sh .github/workflows/*.yml`
exits 1. Measured at HEAD: `tools/ui-audit` 16 files / 3,026 lines, `tools/record-demo` 11
files / 471 lines, `samples` 4 files / 258 lines.

**Correction to the source wave (W6):** it prices this as "382 MB". **Only 212 KB is
tracked**; the 382 MB is gitignored capture output. Do not publish the 382 MB figure.

**`samples/` is not a free delete:** `crates/core/tests/stale_docs.rs:44` and `:479` both
read it, so removing it is a gate edit in the same commit.

Three other tracked references, all trivial: `crates/core/tests/retired_names.rs:40` (a
comment above a `.work` skip entry), `site/README.md:74`, and one more.
**Interacts with S7**: if the screencasts are re-recorded rather than cut, `record-demo`
stays.

## S11 [A] MAJOR: the W7 dead-code sweep is 100% unspent, about 1,050 lines

Re-verified at HEAD, five subjects:

1. `crates/core/tests/retired_names.rs` is 855 lines / 21 tests, of which **18 tests / 562
   lines** are hand-written UI tombstones with no register behind them, which
   `CLAUDE.md`'s own register rule says not to write. The file's charter is lines 1-293.
2. `crates/core/src/schema.rs`, 146 lines, `SITE_SCHEMA` read only inside its own
   `#[cfg(test)]` module (`grep -rn 'schema::' --include='*.rs' crates | grep -v src/schema.rs`
   exits 1), and `crates/core/assets/schema/tali-site.schema.json` is **byte-identical** to
   `editor/vscode/schema/tali-site.schema.json` (`cmp` exits 0).
3. `$/cancelRequest` batching, which its own doc comment retires.
4. `render/model.rs:372` `after_body` (`doc_includes.rs:22` confirms only `in_header` is
   populated).
5. `lsp.rs:1153` and `:1172` index a `vocab` key that does not exist.

**Related:** `crates/core/src/render/mod.rs:2078` `pub fn base_css()` and `:2084`
`pub fn site_css()` are `pub` in `taliesin-core` with **every caller a test**, most of them
in the tombstones above. They go with them.

## S12 [A] MAJOR: `notes/` is 36,315 lines across 104 files, larger than `crates/core/src`

Nothing gates it (`retired_names.rs:61` `SKIP_PATHS` names `"notes"`; `stale_docs.rs`'s
`ROOTS` excludes it; `gate_script.rs:51` walks only `crates/`), and it is the first thing
`CLAUDE.md` tells a fresh session to read. It grew 7 files / 2,274 lines since 2026-08-10.

Deletable outright, about 1,683 lines:
- `notes/retired/diagnostics-explanations.rs`, 1,222 lines, **byte-identical** to
  `git show pre-cut:crates/core/src/diagnostics/codes.rs`, and carrying **no header saying
  it is dead**. Its module doc opens by describing a verb cut in wave 9. Every drift guard is
  blanket-exempt from `notes/`, so this unmarked copy of the pre-cut vocabulary is invisible
  to all of them (`grep -c` inside it: scrolly 8, panel-tabset 4, theorem 20, publish 11,
  prose-lint 5).
- `notes/ap2-fuzz-harness` + `notes/ap8-determinism-harness`, 461 lines.

Two banner edits, both worse than inert:
- `notes/ROADMAP.md:3-7`'s pause banner is expired and **now reads as permission to grow**.
- `notes/FEATURE-IDEAS.md:3-6` (1,181 lines) still tells its reader an idea "graduates to
  the roadmap only when it earns a corpus pin doc", a rule `CLAUDE.md` explicitly retired as
  circular.

**This file is part of the problem it describes.** Delete it when it is empty.

## S13 [A] MINOR: `init --json` and `new post --json` falsify the manual

`docs/guide/reference/cli.tmd:78` states that `build --check-only --format json` "is the
tool's one machine-readable surface". There are four: `build`, `doctor`, `init`, `new`.
`grep -rn '\-\-json' docs --include='*.tmd'` returns nothing, so neither flag appears
anywhere in the manual, including its own row in cli.tmd's command table. Each also accepts
`--format human|json` where `human` is a pure no-op, and each carries its own
`bad_format_error` branch.

About 90 lines across `cli.rs:63,70-120,338-360,430-449`. **Deleting them makes the manual
true**, which is the cheaper direction.

## S14 [A] MINOR: cut-wave residue in production source comments

`stale_docs.rs` walks only `.md`/`.tmd`, so a source comment can name a cut feature forever
with every gate green. Live examples: `TALIESIN_R` presented as a current fix in
`build.rs:528` and `exec.rs:41,555` (and in two READMEs and
`.claude/agents/corpus-verifier.md:28`); `render/mod.rs:99` says the text projection is
"reached via `RenderedDoc::body_text()`" with **no such function anywhere in the tree**, and
labels the module "Text projection (`taliesin read`)" after that verb was cut.

## S15 [A] DECIDE: the retired vocabulary is now the same size as the live vocabulary

Live closed-set vocabulary across every validator const: about **90 names in about 61 lines**.
Retirement registers: `RETIRED_KEYS` 40 + `RETIRED_DIV_CLASSES` 25 + `RETIRED_COMMANDS` 18 +
`RETIRED_XREF_PREFIXES` 7 + `RETIRED_CELL_LANGS` 3 + `RETIRED_NEW_KINDS` 3 + `RETIRED_FLAGS`
2 = **94 entries in 481 lines**, on top of 2,768 lines of dedicated drift/tombstone test
files.

For a tool with zero published users, every one of those 94 entries answers an author who
wrote a spelling that only ever existed in this repository. **The counter-argument is real
and is why this is DECIDE, not CUT:** the registers are the single strongest piece of
evidence that the surviving surface is designed rather than residual, because every retired
name answers with its successor instead of a did-you-mean. Cutting them before publishing
trades the tool's best first-contact property for lines nobody is paying for. **Recommend
keeping through 1.0 and revisiting when real users exist.**

---

# BATCH 12: publish-path decisions (the author's, not code)

## S16 [A] BLOCKING: there is no publish path for the manual

`gh repo view` says private; `gh release list` is empty; `git tag` holds no `v*`. So every
README link (clone URL, releases page, the `Docs:` URL `taliesin help` prints) 404s.
`.github/workflows/ci.yml` has six jobs and **no deploy step**; `tools/build-site.sh`
composes the deploy but is run only by hand and by `--check`; `site/_site.yml:8` declares
`url: "https://taliesin.sh"`, which **has no DNS record** and which `seo.rs`/`meta.rs` bake
into `sitemap.xml`, `robots.txt` and `og:url`.

`notes/2026-08-10-mvp-publish-session.md:74-88` lists four steps to a tag and does not
mention hosting at all. This is a decision, not lines.

## S17 [A] BLOCKING: the purge set versus "make the repository public"

`.githooks/pre-push:36-41` defines a register of files that must never be published, and its
own comment explains it matches only `--diff-filter=A` "because the purge set is still
tracked today, so a check against the whole tree would refuse every push". **All seven are
tracked at HEAD**, 3,061 lines: `notes/STARTUP-PLAN.md`, `notes/FUNDING-RESEARCH.md`,
`2026-07-18-pmf-audit.md`, `2026-07-27-due-diligence-audit.md`,
`2026-07-28-demand-positioning-audit.md`, `2026-07-28-launch-critique.md`,
`2026-07-27-adoption-friction-audit.md`. Two instruct their own removal in their own text.

Meanwhile `notes/2026-08-10-mvp-publish-session.md:77-80` lists "make the repository public"
as step 2, and **flipping visibility publishes the whole history, not HEAD**.
`notes/STARTUP-PLAN.md:111-127` records a contrary already-made decision (a fresh
no-history repo). **Two live documents disagree on the publish step, and getting it wrong is
irreversible.** Also: `git grep -Il "/home/bogo"` matches 16 files.

## S18 [V] MINOR: `notes/backlog.md`'s standing constraints are stale

It names `taliesin features` ("exists, so do not re-derive an adoption table by grep") which
wave 2 cut; "four gates" and `TALIESIN_REQUIRE_..._R`/`_CHROME` when there are eleven gates
and two runtimes; "FIVE drift gates; a RETIRED one trips EIGHT" when `CLAUDE.md` now says
four and one; and owes "the four-projection sweep" to `taliesin read`, `skim.rs` and
`llms-full.txt`, all three cut. A session that reads it for orientation is misdirected on
every count.

---

# Refuted: do not re-file

Both survived a finder and were killed by a refuter. Recorded in
[DO-NOT-REBUILD.md](DO-NOT-REBUILD.md) as well.

1. **"A block containing a `{{< input >}}` reports an end column past the end of its own
   source line"** (`crates/core/src/render/extension/mod.rs`). Refuted.
2. **"`codeAction` quick fix builds a buffer-rewriting `WorkspaceEdit` from client-supplied
   data with no cross-check"** (`crates/server/src/lsp.rs`). Refuted: the "Change to `X`"
   quick fix returning a `WorkspaceEdit` is the standard LSP contract, is user-invoked rather
   than server-initiated, and is pinned by its own test. It does not breach the
   single-editing-surface rule.

# Affirmations worth not re-litigating

Measured during this audit. Each cost real work and each closes a question a future session
would otherwise reopen.

- **Click-to-source is fully intact**, verified live through an include, a fenced div, an
  executed-cell image, and correctly silent on a gathered reference block.
- **Block-level incremental updates and live-state preservation are intact.** Editing one
  paragraph inside an included file produced `update 1 block` with the page's `{js}` runtime
  state fully preserved (slider 9, mount counter 2, teardown counter 1, `window` sentinel).
- **"No per-edit startup cost" is still literally true.** Wave 11 cut the warm *pool*, not
  the per-page warm kernel; `exec.rs:1084` returns before any boot when the page's kernel is
  alive. The cost lands on a cold or evicted page: measured 1.55 / 1.62 / 1.63 s.
- **The surviving document vocabulary is fully witnessed.** Every offered name in
  `CELL_OPTION_KEYS`, `CALLOUT_KINDS`, `INPUT_TYPES`, `DIV_FEATURE_CLASSES`, `LISTING_KEYS`,
  `HERO_KEYS` has real use in the shipped read set (excluding `docs/guide/reference/` so a
  reference table cannot vouch for itself). **The recorded "10 unused offered names" tail is
  stale by eight**: `vocab.rs:351-355` filters `UNSUPPORTED_KEYS` (removing `csl`) and
  `:316-324` filters `RETIRED_XREF_PREFIXES` (the seven theorem prefixes). The real tail is
  **two**: `_site.yml`'s `head:` and `python:`.
- **`corpus/tarn` is NOT cuttable** despite looking like a synthetic persona fixture.
  `tools/build-site.sh:41-43` composes it into the marketing deploy, so deleting it 404s a
  published page and trips gate 5, and its 16 tests are the only golden for cross-page
  `@sec-` numbering by chapter.
- **The code/surface ratio is defensible.** The alarming file sizes are mostly test modules:
  `lsp.rs` is 1,742 production of 4,138; `lint.rs` 670 of 1,906; `lsp_complete.rs` 987 of
  1,911. Whole-workspace production Rust is 35,386 lines against 42,707 test lines, with
  **zero traits and two one-variant enums**. The outlier is `notes/`, not the compiler.
- **The error paths are the best part of the tool.** Every retired verb names its successor
  rather than guessing; typos get edit-distance suggestions; the missing-kernel path prints a
  five-step numbered resolution order showing which rule won; a directory with no `_site.yml`
  is refused with two concrete next commands.

# Method note

The finder/refuter split earned its cost. Refuters killed 2 findings outright and **corrected
the severity, trigger, root cause or anchor of at least 14 more**: A5's blamed file was wrong,
A6's root cause was the persist loop rather than the plan mask, A17's obvious fix would have
reintroduced a wedge, A28's severity went *up* (draft leak), A22's blast radius widened to the
Atom feed, and A8's trigger widened from included files to any document containing an include.
**Every anchor in this file is the corrected one.** Where a finder and a refuter disagreed on
severity, the refuter's is recorded.
