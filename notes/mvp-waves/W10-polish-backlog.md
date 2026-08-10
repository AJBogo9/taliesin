# W10 — NOT TAKEN. See `notes/2026-08-10-mvp-publish-session.md` for the ship-path outcome

> ## ⚠ NOT TAKEN, AND STALE. READ `notes/2026-08-10-mvp-publish-session.md` FIRST.
>
> This wave was **not executed**. It was written on 2026-08-10 against `1a82f2ef`, and
> **every line number, and some premises, are stale**: nine commits have landed since
> (through `315d67db`), five of them prose sweeps over the same files this wave names.
> **Grep, do not trust.** If a step's premise is false, drop the step and say so in the
> commit message — that is a success, not a failure.
>
> The plan file these waves referred to (`notes/2026-08-10-mvp-publish-plan.md`) is gone
> with the waves that were taken; the session record above replaces it, and carries the
> ship-path outcome, the surfaced-not-fixed list, and what remains before a tag.
>
> **P1–P5 ARE DONE** (`37cb9e0b`, `727859c9`, `1ffd2aae`, `781b0a1c`, `0178e403`). Only
> **P6–P11** remain, and their measurements were taken against a binary that has since
> changed under all five: `new post` now refuses a rootless CWD, an error path writes
> nothing to stdout, `--check-only`'s clean verdict names the cells it did not run, and
> `doctor` has one fewer row (which also changed `--format json`'s `checks` array).
> **Re-measure before honouring any step that quotes output.**
>
> **The gate count is now 11, not 10.** W1 landed the census stanza. This file's own
> corrections block tells you to re-count and warns that three waves each want to call
> their gate "the eleventh" — that warning still stands and is now sharper: P6 step 5 and
> P8 step 4 each add a stanza, so if both land the true count is **13**. Take the number
> from the script's own verdict line and write it into `tools/gates.sh:16`, its verdict
> comment, and the `CLAUDE.md` paragraph. **Never increment by hand.**

---

> ## ⚠ CORRECTIONS — READ FIRST, THEY OVERRIDE THE SECTION BELOW
>
> A skeptic pass over the assembled plan found 21 defects **in the plan itself**. The ones
> affecting this wave are below. Where a correction contradicts the section body, the
> correction wins. Line numbers throughout were true at `1a82f2ef` on 2026-08-10, and every
> wave that lands before yours shifts them — **grep, do not trust**.
>
> - **Gate count.** P6 step 5 and P8 step 4 each add a `gates.sh` stanza and each writes "11", and W1 does too. **Re-count with `rg -c 'PASSED\+=' tools/gates.sh` and write that number**; do not increment what is there. Every other wave's "10 gates" verification line becomes "the printed count matches `tools/gates.sh:16`".
> - **P4:** if W6 has landed, `tools/ui-audit/` is **gone** — only `editor/vscode/.vscode-test/` is a protected fixture. P4's `du -sh tools/` expectations assume `.work/` exists. **P4 owns the `_book` word in `three_scene_theme.rs:74`**; P7 step 5 must not also do it.
> - **P4 step 2 deletes every `_freeze/`.** That forces a full cold re-execution of the corpus on the next `gates.sh` and is unrecoverable if no kernel is available. Confirm a working `TALIESIN_PYTHON` **before** running it, or skip the step.
> - **P3 and P5 are blocked by W4** — both edit rows in `docs/guide/reference/cli.tmd`, and P5's `doctor` row does not exist until W4 adds it.
> - **P8 before P6.** P8 adds a gate; P6 invalidates `crates/server/src/build.rs:3151`'s "app.js ships verbatim" assertion. One diff should not carry both a new gate and the change retiring an old one.
> - **P10 after W7** (or re-grep `page.rs`).
> - Anchors confirmed exact and not to be re-derived: `cli.rs:43,122,322`, `main.rs:79-88,273`, `doctor.rs:165,189`, `base.css:783,797,917,233-242`, `log.rs:93`, `serve_site/mod.rs:423`, `gates.sh:16,335,338,360-362`, `stale_docs.rs:44,56,237,311,407,606`, `gate_script.rs:162,166`.

---

### Wave P — The polish backlog: eleven ranked rough edges

**Branch:** one per item, named below · **Kind:** polish · **Size:** ~380 lines across 11 commits · **Blocked by:** none

> **This is a RANKED BACKLOG, not a wave.** Pull one item, one branch, one commit, gates green either side.
> Every item below stands alone and names its own files. Items are ordered by (impact on a first-time
> reader or author) ÷ (effort). **P1–P5 are release-blocking. P6–P9 should ship. P10–P11 are optional.**
>
> **Everything here was measured on 2026-08-10 against `target/release/taliesin`** (mtime `2026-08-09
> 22:38`). That binary stamps `3dfd15b5` in `--version` but was built from the working tree that already
> contained `1a82f2ef`'s `--out` mermaid fix — verified by observing the fix's behaviour. **This is not a
> version-string bug**; it is a binary built before its commit. Do not open a finding on it.

---

#### Five claims in the brief were wrong. Read this before planning.

- **DISPROVEN — "keep the three `readPixels` tests" (item 2).** There are none. `readPixels` appears
  **once in the entire tree**, in a doc comment at `crates/core/tests/three_scene_theme.rs:7`, recording a
  manual browser measurement. The file has **4** tests and none of them touch a canvas.
- **DISPROVEN — "the copy-comparison test is vacuous" (item 2).** Half true. See P7.
- **DISPROVEN — "~393 lines of JavaScript live inside `.tmd` files" (item 3).** Measured **2,347 lines**
  across **18** files, all inside ` ```{js} ` fences; **zero** raw `<script>` blocks in any `.tmd`. The
  claim understates the exposure by 6×. Largest single file: `corpus/tech-blog/posts/a-star/index.tmd`
  at **504** lines.
- **DISPROVEN — "stale build output committed" (item 4).** Nothing is committed.
  `git ls-files | rg '_book/|_site/|_freeze/'` returns two source files whose path contains the substring
  `serve_site/`. All **21** build-output directories are gitignored. The un-ignored residue is a *different*
  file — see P4.
- **DISPROVEN — `CLAUDE.md`'s preview-vs-build TOC sentence.** *"`preview` auto-detects and renders one,
  `build` does not."* Wrong in both directions. With `toc: true`, `build page.tmd` and `preview page.tmd`
  render the identical `<nav id="TOC">` in the identical place (screenshotted both). With no `toc:` key, a
  4-heading page gets no TOC from either verb: `Site::page_toc`'s `MIN_TOC_HEADINGS` auto-gate
  (`crates/core/src/site/mod.rs:197`) is reached only from `serve_site/mod.rs:603` and `:1215`, and returns
  false for a synthesized single-page project. Fix the sentence in whichever item you take (P9 touches this
  area).
- **CLEAN, checked not assumed — R4's docs sweep.** A backticked-path scan over `CLAUDE.md`, `README.md`,
  `docs/guide/**`, `docs/internals/**` and `site/**` produced 30 candidate dead paths; **all 30 resolve** as
  placeholders (`hello.tmd`, `post.tmd`), deliberate negatives (`_metadata.yml` is named 3 times, always as
  *"there is no `_metadata.yml` cascade"*), globs, or retrospective history prose inside `CLAUDE.md` itself.
  **No live shipped page names a file that is gone.** Do not re-run this sweep.

---

### P1 — `taliesin new post` writes outside the project, silently, exit 0

**Branch:** `fix/p1-new-post-project-aware` · **Size:** ~40 lines · **RELEASE-BLOCKING**

**Why this ships before release.** This is the second command a new user types, and following the tool's own
printed instructions puts the post in the wrong place with no warning and a success exit code. Reproduced
verbatim: `taliesin init myblog` prints *"Scaffolded a Taliesin site. Preview it: `taliesin preview
myblog`"* (`cli.rs:122`), leaving you in the parent directory. The scaffolded homepage's first bullet then
says *"Scaffold a dated post with `taliesin new post my-first-post`"* (`cli.rs:43`), with no `cd` and no
`--dir`. Typed there, it writes `./posts/my-first-post/index.tmd` **beside** `myblog/`, in a directory with
no `_site.yml`, and prints `built ./posts/my-first-post/index.tmd` and exit 0. The post is invisible to the
site, absent from the listing R5-1 just wired up, and the only way to notice is to look at the filesystem.

**Verified state (checked 2026-08-10).**
- `crates/server/src/cli.rs:322` — `let mut root = ".".to_string();`. `--dir` is the only override.
- `crates/server/src/cli.rs:434` — `write_new(root, ...)` → `write_scaffold` at `:145`, which checks only
  that the target does not already exist. **Zero project awareness anywhere in `cmd_new`.**
  `rg '_site\.yml' crates/server/src/cli.rs` returns only `init`'s writer and its tests.
- `crates/server/src/cli.rs:122` — the `init` next-step string that leaves you in the parent.
- `crates/server/src/cli.rs:43` — the homepage bullet with no `--dir`.
- Reproduced end to end in a scratch dir: two `posts/my-first-post/index.tmd` files, one orphaned.
- The overwrite guard *does* work: a second `new post` with the same slug prints
  `error   ./posts/my-first-post/index.tmd already exists; refusing to overwrite`, exit 1. Keep it.

**Files** — Modify: `crates/server/src/cli.rs` · Test: `crates/server/tests/new_cli.rs`

**Steps**
- [ ] 1. Write the failing test in `crates/server/tests/new_cli.rs`: `init` into a tempdir, then run
      `new post p` with the **tempdir's parent** as CWD, and assert it does not silently create
      `<parent>/posts/`.
- [ ] 2. Run it and watch it fail (it currently creates the directory and exits 0).
- [ ] 3. In `cmd_new`, before `write_new`, resolve the enclosing project with the walker that already
      exists: `taliesin_core::site::enclosing_site_root` (`crates/core/src/site/mod.rs:202`, public, used by
      `preview` for exactly this job). **Prefer the smallest honest behaviour, per the standing directive:**
      when `--dir` was not given and `enclosing_site_root(cwd)` returns `None`, refuse with the error shape
      `build` already uses for the same mistake (measured: *"`./posts` has no `_site.yml`, so it is not a
      project."* + two indented `to …:` lines). Do **not** add a `--force`; that is a knob, and the
      near-perfect-default rule forbids it.
- [ ] 4. Run the test — expected PASS. Confirm `new post` still works from inside a project and still works
      with an explicit `--dir`.
- [ ] 5. Fix `INIT_INDEX_TMD`'s first bullet (`cli.rs:43`) and `init`'s printed next step (`cli.rs:122`) so
      the two-command sequence they teach actually composes. Cheapest correct spelling: make `init`'s hint
      `cd myblog && taliesin preview .`, or make the bullet `taliesin new post my-first-post --dir <project>`.
      **Four lines of string content, no code.**

**Traps**
- `crates/server/tests/init_cli.rs` asserts the built `index.html` links the post (R5-1's pin). Changing
  `INIT_INDEX_TMD` risks it. Read it before editing the constant.
- `main.rs` has a drift gate over `--help` text; if you change `new`'s synopsis, grep
  `crates/server/tests/help_cli.rs` and `stale_docs.rs` first.
- **`docs/guide/reference/cli.tmd`'s table is the FIFTH, ungated registration site** for a verb
  (`CLAUDE.md` says so, and wave 13 shipped a stale `run` row through it). If `new`'s behaviour changes,
  grep that page by hand.

**Verification**
- `cargo test -p taliesin-server --test new_cli` — the new test passes.
- By hand, exactly as a new user: `taliesin init myblog && taliesin new post p` from the parent must now
  refuse with guidance rather than write an orphan.
- `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh` — 10 gates.

**Done when** running `new post` from outside any project refuses with the same shape of error `build <dir>`
already gives, and the sequence `init`'s own output teaches produces a post the homepage links.

---

### P2 — Every command error prints 56 lines of help to **stdout** while the error goes to stderr

**Branch:** `fix/p2-error-streams` · **Size:** ~15 lines · **RELEASE-BLOCKING**

**Why this ships before release.** `taliesin buidl . 2>/dev/null` prints 56 lines of help and **loses the
error entirely**. `taliesin buidl . | head` shows help with no error. The one line that answers the question
scrolls off the top of any terminal shorter than 57 rows. The tool already gets this right one function
away: an unknown *flag* prints 1 line to stderr and 0 to stdout.

**Verified state (checked 2026-08-10), by measurement, not reading.**
- `taliesin buidl .` → **stdout 56 lines, stderr 1 line**, exit 1.
- `taliesin build . --check-onl` → **stdout 0 lines, stderr 1 line**, exit 1. The two paths disagree.
- `crates/server/src/main.rs:85-88` — the `Some(other)` arm: `log::error(...)` (stderr, `log.rs:267`) then
  `usage()` (line 87).
- `crates/server/src/main.rs:273` — `fn usage()` is built entirely from `println!` / `print!`, i.e. stdout.
  Correct for `taliesin help` (`main.rs:79-81`, exit 0); wrong for the error arm.
- The same 56-line dump follows a **retired** verb: `taliesin run .` prints the one-sentence retirement note
  to stderr and then the whole help to stdout.
- Two error prefixes coexist: `  error   unknown command: …` (styled, `log::error`) vs `error: unknown flag
  …` (raw `eprintln!`, no indent). `serve/mod.rs:661` documents the raw spelling as intentional.

**Files** — Modify: `crates/server/src/main.rs` · Test: `crates/server/tests/help_cli.rs`

**Steps**
- [ ] 1. Write the failing test in `crates/server/tests/help_cli.rs`: run the binary with an unknown command
      and assert **stdout is empty** and stderr carries the did-you-mean. Assert the same for a retired verb
      from `RETIRED_COMMANDS` (`main.rs:112`).
- [ ] 2. Run it and watch it fail on stdout being 56 lines.
- [ ] 3. **Prefer the smaller fix.** Replace `usage()` at `main.rs:87` with a single stderr line —
      `eprintln!("run `taliesin help` for the full list of commands");` — rather than re-plumbing `usage()`
      to take a writer. A did-you-mean plus a pointer is the whole useful content; the other 55 lines are
      noise on an error path. If you would rather keep the full dump, it must go to **stderr**, which means
      `usage()` grows a `to: &mut dyn Write` parameter and both call sites change; that is the larger diff
      and the standing directive says take the smaller one.
- [ ] 4. Run the test — expected PASS. Confirm `taliesin help`, `taliesin --help` and `taliesin` with no
      args still print the full help to **stdout** at exit 0 (`main.rs:79-81`) — that path must not move.

**Traps**
- `crates/server/tests/help_cli.rs` and `main.rs`'s `dispatch_tests` both read help text. R4 already
  anchored some needles there; re-read `help_cli.rs:43-47` before adding assertions so you do not
  reintroduce an unanchored substring match.
- `main.rs`'s `env_help_lists_every_runtime_env_var` and `commands_in_dispatch` gates read `COMMANDS_HELP`
  and `ENV_HELP` as raw strings. Do not restructure those constants; only change where they are written.
- Do **not** change the two error prefixes in this commit. That is a separate cosmetic call and mixing it in
  makes the stream fix unreviewable.

**Verification**
- `cargo test -p taliesin-server --test help_cli`
- `./target/release/taliesin buidl . 2>/dev/null | wc -l` → **0**
- `./target/release/taliesin buidl . 2>&1 >/dev/null | wc -l` → **≥1**
- `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh`

**Done when** no error path writes to stdout, and `taliesin <typo> 2>/dev/null` prints nothing.

---

### P3 — `build --check-only` says "no problems found" on a project whose `build` exits 1

**Branch:** `fix/p3-check-only-honesty` · **Size:** ~25 lines · **RELEASE-BLOCKING**

**Why this ships before release.** `CLAUDE.md` and the User Guide both call this command **"THE PRE-PUBLISH
GATE"**. Reproduced on a fresh `init` project with one `{python}` cell and no `ipykernel`:

```
build .                        exit=1   "no python kernel available, but this document has python cells"
build . --check-only           exit=0   "no problems found"
build . --check-only --strict  exit=0   "no problems found"
```

An author runs the gate, is told the project is clean, and the publish build fails. Worse, `build`'s own
`--help` promises *"`--strict` exits non-zero on a cell error"* — a promise `--check-only` structurally
cannot keep, because wave 9 removed the interpreter probe from `check` on purpose.

**Verified state (checked 2026-08-10).** All four exit codes above measured directly. The `build .` error is
otherwise excellent (five numbered interpreter-resolution steps, the `.venv` search trail, and a `--no-exec`
escape) — **do not touch it**. `crates/server/src/lint.rs`'s `cmd_check_only` is the ~40-line front door;
`page_static_diagnostics` is the static-only superset by design.

**Files** — Modify: `crates/server/src/lint.rs`, `crates/server/src/main.rs` (`build`'s `--help` prose) ·
Test: `crates/server/tests/` (extend whichever file already drives `--check-only`)

**Steps**
- [ ] 1. Write the failing test: run `--check-only` on a project containing an executable cell and assert
      the human output says the check was **static**.
- [ ] 2. **Do NOT add an interpreter probe.** Wave 9 removed it deliberately and re-adding it is scope this
      plan is closed to. Fix the *message*, which is where the lie is.
- [ ] 3. In `cmd_check_only`'s success path, when the project contains at least one executable cell, replace
      the bare `no problems found` with a one-line qualifier naming what was not checked and the command
      that does check it — e.g. `no static problems found  ·  N code cells not run (build without
      --check-only to execute them)`. **Reuse the existing cell count; do not add a new walk.** With zero
      executable cells the output must stay exactly `no problems found`, unchanged, so a prose-only project
      sees no new noise.
- [ ] 4. Correct `--strict`'s `--help` clause in `main.rs`. It currently promises *"exits non-zero on a cell
      error or located warning"*; under `--check-only` only the second half can happen. One sentence.
- [ ] 5. Check `docs/guide/reference/cli.tmd`'s `--check-only` and `--strict` rows by hand — **that page is
      ungated in the CLI→docs direction** (`stale_docs.rs:552` gates only docs→CLI).

**Traps**
- `--format json` must not change shape. Measured today it emits `{"diagnostics": []}`; an agent parses
  that. The qualifier is human-output only.
- `.githooks/pre-push` step 4 and `tools/gates.sh` gate 9 both run `build docs/guide --check-only
  --no-exec`. If the new line lands on stdout in a way those scripts grep, they break. Read both.
- Do not let this become "add a knob". The near-perfect-default rule applies: one better sentence, no flag.

**Verification**
- On a scratch project with a `{python}` cell and no kernel: `--check-only` output now names the cells it
  did not run; `build .` still exits 1 with its full guidance intact.
- `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh` — 10 gates, including both document gates.

**Done when** the gate's success message never claims more than it checked.

---

### P4 — Stale build residue, and the one directory nothing ignores

**Branch:** `fix/p4-residue` · **Size:** ~5 lines + a documented cleanup command · **RELEASE-BLOCKING (cheap)**

**Why this ships before release.** `docs/guide/_book/` still contains `deck.3443e895a21a29e8.js` (93,655 B)
and `deck.49275a616a96ea25.css` (36,277 B) from the slide-deck engine **cut in wave 5**, plus
`manifest.webmanifest` from the PWA manifest **cut in wave 4**, and `_book/using/agents.html` documents a
`taliesin new --json` flow for a page that no longer exists. All of it is gitignored, which is **worse, not
better**: one audit agent read it as a witness for a live feature. It is a 6.7 MB fossil of the tool as it
was on 2026-08-08, sitting in the directory whose name says "the docs".

**Verified state (checked 2026-08-10).**
- `docs/guide/_book/_assets/` — `deck.3443e895a21a29e8.js`, `deck.49275a616a96ea25.css`, both mtime
  `2026-08-08 13:52`. `docs/guide/_book/manifest.webmanifest` exists. `rg -l '\-\-json'
  docs/guide/_book/using/agents.html` matches.
- **21 gitignored build directories**, 22,700 KB total: `docs/guide/_book` 6.7 M, `docs/internals/_book`
  5.3 M, `corpus/tech-blog/_site` 5.3 M, `corpus/agent/_site`, `corpus/shared-bib/_site`, and 16 `_freeze/`
  dirs. `.gitignore` covers `_site/`, `_book/`, `_freeze/`.
- **Nothing is committed.** `git ls-files | rg '_book/|_site/|_freeze/'` returns only
  `crates/server/src/serve_site/{mod,exec_pool}.rs` — a substring match on `serve_site/`. **DISPROVEN**: the
  brief's "committed" wording.
- **The real un-ignored residue is `tools/__pycache__/`** (`portability-census.cpython-312.pyc`).
  `git status --short tools/` prints `?? tools/__pycache__/`; `git check-ignore -v` matches nothing; the
  author's machine-global ignore (`~/.config/git/ignore`) contains only
  `**/.claude/settings.local.json`. So the tree is dirty right now, and will be dirty in any clone that runs
  `tools/portability-census.py`.
- Two more ignored heavyweights, both **legitimate fixtures, do not delete**: `editor/vscode/.vscode-test`
  (3.1 GB — wave R6 already learned this is the offline grammar gate's fixture) and `tools/ui-audit/.work`
  (340 MB, ignored by `tools/ui-audit/.gitignore:1`).

**Files** — Modify: `.gitignore` · Modify: `tools/gates.sh` (one line, see step 3)

**Steps**
- [ ] 1. Add `__pycache__/` to `.gitignore`, beside the existing Node/Rust blocks. **One line. This is the
      only tracked-tree change in the item.**
- [ ] 2. Delete the residue by hand, once, and do not commit anything for it (it is all ignored):
      `rm -rf docs/guide/_book docs/internals/_book corpus/*/_site corpus/*/_freeze corpus/tech-blog/posts/_freeze docs/*/_freeze site/_freeze corpus/_freeze`.
      **Do not touch `editor/vscode/.vscode-test` or `tools/ui-audit/.work`.**
- [ ] 3. **Answer the brief's question: should a tool refuse to read them?** The measurement says the
      exclusions already exist and are consistent — `serve/mod.rs:518` `SKIP_DIRS`, `stale_docs.rs:317-319`,
      `retired_names.rs:35-37`, `svg_assets_render.rs:47`, `parallel_build_determinism.rs:38` all list
      `_site`/`_book`/`_freeze`. **One walker is short**: `crates/core/tests/three_scene_theme.rs:74` skips
      `_freeze` and `_site` but **not `_book`**. It is harmless today only because `_book` holds no `.tmd`.
      Add `_book` to that condition — one word — so the walk cannot start counting a built copy if a future
      build ever emits sources.
- [ ] 4. Add a `gates.sh` preflight line that fails if `git status --porcelain` reports an untracked path
      under `tools/`. **Optional and only if it is genuinely one line** — `gates.sh` already refuses to be
      green unless every gate ran, and a dirty-tree check fits that contract. If it is not one line, skip it
      and say so in the commit body.

**Traps**
- **The ordering rule does not apply here** (nothing deleted is a pin), but re-read it before deleting
  anything under `corpus/`: only the ignored `_site`/`_freeze` subdirectories go, never a `.tmd`.
- Deleting `_freeze/` means the next `gates.sh` run re-executes every cell in the corpus. Budget for it:
  the run is ~25 min warm and will be materially longer cold.
- `.githooks/pre-push` step 5 runs `tools/build-site.sh --check` into a temp dir; it does not read
  `docs/*/_book`, so deleting those cannot break it. Verified by reading the script.

**Verification**
- `git status --short` → empty.
- `find . -type d \( -name _book -o -name _site \) -not -path './target/*'` → empty (the `_freeze` dirs
  regenerate).
- `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh` — 10 gates green from cold.

**Done when** `git status` is clean on a fresh clone that has run the census script, and no directory in the
tree describes the tool as it was two waves ago.

---

### P5 — `doctor`'s `env` row is a green ✓ that can never be anything else

**Branch:** `fix/p5-doctor-env-row` · **Size:** ~20 lines · **RELEASE-BLOCKING (cheap)**

**Why this ships before release.** Measured on a machine with no venv and no `ipykernel`:

```
  ⚠  python  python3 (python3)
             Python 3.12.3  ·  ipykernel MISSING
  ✓  env     no active virtual/conda env (using the system PATH)
```

A green tick against the sentence *"no active virtual/conda env"*. The glyph says "fine", the text says
"nothing is configured", and the row is structurally incapable of ever saying anything else. A first user
reads three rows, sees two ticks, and concludes two of three things are right.

**Verified state (checked 2026-08-10).**
- `crates/server/src/doctor.rs:189` — `status: Status::Ok`, hard-coded, the only return in
  `active_env_check` (`:165`). Its own doc comment at `:163` says *"Informational (always ✓)"*, so this is
  deliberate, not an omission.
- `crates/server/src/doctor.rs:197` — `overall_ok` fails only on `Status::Error`, so this row can never
  reach it.
- The two sibling checks **can** vary: `interpreter_check` returns Ok / Warn / Error, and the `config` check
  at `:325`/`:332` returns Ok or Warn.
- **The brief's protective note is CORRECT and must be honoured.** `doctor.rs:104-122` is the
  working-but-unselected branch: `let chosen = !matches!(r.provenance, Provenance::Default);` →
  `Status::Warn` with *"`ipykernel` present, but nothing in this project selected this interpreter"*. That
  is the only place in the tool distinguishing "you are ready" from a `ModuleNotFoundError` in every cell.
  **Do not touch it.** It is a different function from the one this item changes.
- Call site: `doctor.rs:312`. Unit tests at `:498, :504, :509, :514, :523, :529`.

**Files** — Modify: `crates/server/src/doctor.rs`

**Steps**
- [ ] 1. Decide, and per the standing directive prefer the cut. **Option A (preferred): delete the `env`
      check.** Its whole content is already in the `python` row, which prints `<path> (<provenance>)` plus
      the `.venv` search trail plus the "nothing selected this interpreter" warning. A row that repeats a
      neighbour and can never fail is decoration.
      **Option B: keep it and drop its glyph** — render informational rows with a neutral marker (`·`)
      instead of `✓`, so a green tick always means a check that could have failed and didn't.
      **Shipping a permanently-green ✓ is not an option either way.**
- [ ] 2. If you take A, delete `active_env_check`, its call at `:312`, the six unit tests at `:498-:529`,
      and the `VIRTUAL_ENV`/`CONDA_PREFIX`/`CONDA_DEFAULT_ENV` reads that feed it. Let
      `clippy -D warnings` enumerate the cascade — that is the method R6-11 and R6-2 both proved.
- [ ] 3. Update `doctor`'s `--help` line in `main.rs`. Measured today it promises *"(the Python interpreter,
      ipykernel, active conda/venv)"*; under A the third clause is false. **This is the class of doc-drift
      the audit found seventeen of.**
- [ ] 4. Update `docs/guide/reference/cli.tmd`'s `doctor` row **by hand** — ungated in this direction.

**Traps**
- `doctor --format json` emits a `checks` array an agent may parse. Removing a row changes that shape.
  Note it in the commit body. (R6-8 proposes deleting the JSON surface entirely; if R6-8 is taken first,
  this trap evaporates.)
- **No register entry is owed.** `env` is a display row, not a vocabulary name.
- Do not confuse `active_env_check` with `interpreter_check`. Different functions, ~80 lines apart, and only
  one of them is the problem.

**Verification**
- `NO_COLOR=1 ./target/release/taliesin doctor <scratch>` — no row shows a status that cannot vary.
- `cargo test -p taliesin-server doctor`
- `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh`

**Done when** every ✓ in `doctor`'s output is a check that could have printed something else.

---

### P6 — Minify the shipped JavaScript at build time (the `minify_js` question, re-measured)

**Branch:** `fix/p6-minify-shipped-js` · **Size:** ~60 lines · **SHOULD SHIP** · **Blocked by:** P8

**Why this ships before release.** Wave 4 deleted `minify_js` and recorded the cost as *+16,493 gzipped
bytes per site* on `corpus/tech-blog`, naming the honest fix as *"`esbuild --minify` at build time, not
re-deriving the tokenizer"*. **Re-measured 2026-08-10 with `editor/vscode/node_modules/.bin/esbuild`, which
is already vendored in this repo, the number holds and the per-page half was never counted:**

| what | today | `esbuild --minify` | delta |
|---|---|---|---|
| shared `app.<hash>.js` | 77,290 B / **25,428 gz** | 28,667 B / **9,668 gz** | **−15,760 gz, once per site** |
| one built site page (`index.html`) | 15,631 B / **6,353 gz** | 9,653 B / **3,897 gz** | **−2,456 gz, per page** |
| `01-registry.js` (inline, every page) | 2,726 B | 859 B | −1,867 B |
| theme bootstrap (inline, every page) | 6,833 B | 2,747 B | −4,086 B |

The wave-4 figure (16,493) was the shared bundle alone and is still accurate to within 733 bytes. **The
per-page inline half is new.** Every built page carries **10,358 B of inline JS of which 4,370 B (42%) are
source comments**, and `01-registry.js` ships **twice** on every page — once inline in `<head>`, once inside
`app.js` (deliberate and documented at `render/mod.rs:2041-2045` and `page.rs:266-277`; the IIFE is
idempotent). On the 26-page docs deploy the total is **−15,760 − (26 × 2,456) ≈ −79 KB gzipped**.

**The trade, stated so it is a decision.** *For:* ~79 KB gz off the deploy, and it makes the deliberate
registry double-include cost 859 B instead of 2,726 B. *Against:* it puts Node and `esbuild` on the critical
path of `taliesin build`, which is exactly the objection that killed the `dot`-binary alternative in the
mermaid ruling. **The only defensible shape is a build-time asset pre-pass in the repo's own tooling
(`tools/`), producing minified constants the binary `include_str!`s — not a runtime `esbuild` invocation
from `build.rs`.** If that pre-pass cannot be made to run in `gates.sh` (so it cannot silently rot), **skip
this item.** A minified constant nobody regenerates is worse than an unminified one.

**Verified state (checked 2026-08-10).**
- `crates/core/src/render/mod.rs:2097` — `core_enhance_js()` = `[CODE_ENHANCE_JS, TOC_SPY_JS,
  SEARCH_JS].join("\n;\n")`, emitted verbatim as `app.<hash>.js` at `crates/server/src/build.rs:1416`.
- `crates/server/src/build.rs:3151-3152` — the assertion `read(&bundle.app_js) ==
  taliesin_core::core_enhance_js()` with the message *"app.js should ship verbatim now that minify_js is
  gone"*. **This is the test that changes.** Its neighbour at `:3155` is the deliberate control proving CSS
  *is* still minified; keep that shape.
- Comment volume in the shipped JS sources, measured: 31,569 of 77,210 bytes = **40.9%**.
  `search.js` alone is 48,550 B (39.5% comments) = **62.9% of the raw bundle** — the brief's 48,602/77 KB
  figures confirmed.
- `esbuild` at `editor/vscode/node_modules/.bin/esbuild`; system `terser` at `/usr/bin/terser`;
  `node v24.18.0`.

**Files** — Modify: `crates/server/src/build.rs` (the assertion), `tools/gates.sh` (a regeneration check),
plus whatever pre-pass you add under `tools/`. **Do not** re-add `minify_js` to any crate.

**Steps**
- [ ] 1. Decide the shape first and write it in the commit body. If it is not a `tools/` pre-pass gated by
      `gates.sh`, **stop and close this item as declined**, with the measured numbers recorded.
- [ ] 2. Write the failing test: assert the shipped `app.js` contains no `/*` and no line beginning `//`.
- [ ] 3. Build the pre-pass. Regenerate the minified assets and commit them beside their sources.
- [ ] 4. Rewrite `build.rs:3151`'s assertion and its message so it pins the new truth. Leave the CSS control
      at `:3155` alone.
- [ ] 5. Add the regeneration check to `gates.sh` as gate 11, following the file's own `PASSED+=`/`FAILED+=`
      convention exactly, and update the count the script prints. **`gate_script.rs`'s
      `every_pre_push_command_is_also_run_by_the_gate_script` compares the two lists on every run** — read it
      before touching either.
- [ ] 6. Re-measure and record before/after gzipped bytes for the docs deploy in the commit body. The
      campaign's convention is measured, not asserted.

**Traps**
- **Do not restore `minify_js`.** Wave 4 deleted it deliberately and R5-2 restated the prohibition.
- `crates/core/assets/*` are `include_str!`-compiled: a regenerated asset needs `cargo build` before any
  rebuilt site shows it.
- The `code-enhance/` fragments and `web-client/` are both type-checked by `tsc` against
  `jsconfig.json`. Minified output must not enter either `tsc` target.
- **This must land after P8.** P8 adds a JS gate; landing both together makes one diff carry a new gate and
  the change that invalidates an old one.

**Verification**
- `gzip -c _site/_assets/app.*.js | wc -c` on a rebuilt `docs/guide` — record before and after.
- `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh` — now 11 gates, and `gate_script.rs` green.

**Done when** the shipped JS carries no source comments and a gate fails if the minified assets drift from
their sources — or the item is closed as declined with the numbers on record.

---

### P7 — `three_scene_theme.rs`'s copy comparison is half-dead, and its header names a test that never existed

**Branch:** `fix/p7-three-scene-pin` · **Size:** ~15 lines · **SHOULD SHIP**

**Why this ships before release.** Not a reader-facing defect — a truth defect in the regression net, which
is the same genus as everything the campaign spent thirteen waves on.

**Verified state (checked 2026-08-10).**
- **Exactly two copies exist**: `site/_includes/three-scene.tmd` (extended variant) and
  `corpus/tech-blog/_includes/three-scene.tmd` (base variant). One per variant, so
  `three_scene_theme.rs:213`'s `for (path, src) in group.iter().skip(1)` iterates **zero** times for both
  groups. **The brief is right about the loops.**
- **The brief is WRONG that the test is vacuous.** Two live assertions survive and would fail loudly:
  `helper_copies()`'s `copies.len() >= 2` floor at `:56`, and the merge-guard `!extended.is_empty() &&
  !base.is_empty()` at `:205`. The file's own doc comment at `:180-185` already says exactly this, in
  detail, including that "one copy cannot drift, so the assertion below simply has nothing to compare".
  **This was written down when the duplicate cull landed. Nobody is being misled.**
- **DISPROVEN — "keep the three `readPixels` tests".** `rg readPixels` over the whole tree returns
  **one hit**, `three_scene_theme.rs:7`, inside the module doc comment. It records a manual browser
  measurement, not a test. The three tests worth keeping are the ones that actually exist:
  `every_three_scene_helper_builds_a_transparent_canvas`,
  `no_document_asks_a_three_scene_for_a_background_colour`, and
  `the_three_scene_fullscreen_button_is_token_driven`. **All three are live and all three walk both copies.**

**Files** — Modify: `crates/core/tests/three_scene_theme.rs`

**Steps**
- [ ] 1. **Do not delete the test.** Its merge-guard is the half that still fires, and its doc comment says
      so. The only thing wrong is that a reader (and one audit agent) can mistake the dead inner loop for
      the whole test.
- [ ] 2. Rename `same_variant_three_scene_copies_stay_byte_identical` →
      `the_two_three_scene_variants_stay_distinct_and_same_variant_copies_stay_identical`, so the name
      states what still runs.
- [ ] 3. Trim the doc comment at `:180-185` to the current truth in two sentences: one copy per variant
      today, so the byte comparison is dormant and re-arms the moment a second copy of either appears; the
      merge-guard is what fires now. **Delete the historical paragraph about `corpus/graphics3d/` and the
      pca-geometry copy** — that is a dated record and belongs in git, per the same rule R6-1 applied to
      `notes/`.
- [ ] 4. Fix the module header at `:7-8`. It presents `readPixels` numbers as though a test takes them. Say
      they were measured by hand in a browser on the dates given.
- [ ] 5. Add `_book` to the walk exclusion at `:74` — **or leave it to P4 step 3, whichever lands first.**
      Do not do it twice.

**Traps**
- The `copies.len() >= 2` floor at `:56` is load-bearing and its comment explains why 2 is the number.
  Leave it.
- `all_docs()`'s `out.len() > 40` floor at `:92` guards against a rename emptying the walk. Leave it.

**Verification**
- `cargo test -p taliesin-core --test three_scene_theme` — 4 tests pass.
- `mv site/_includes/three-scene.tmd /tmp/ && cargo test -p taliesin-core --test three_scene_theme` must
  **fail** on the merge-guard. Move it back. This is the proof the test is not vacuous.

**Done when** the test's name and comment describe what it checks today, and moving one copy aside still
fails the suite.

---

### P8 — 2,347 lines of JavaScript in `.tmd` files, seen by neither `tsc` gate

**Branch:** `fix/p8-tmd-js-gate` · **Size:** ~50 lines · **SHOULD SHIP**

**Why this ships before release.** A syntax error in a `{js}` cell in a shipped document is caught by
nothing until a reader loads the page and the console throws. `docs/guide/using/code.tmd` (140 lines of
`{js}`), `site/showcase.tmd` (130) and `site/index.tmd` (45) are all pages the marketing deploy serves.

**Verified state (checked 2026-08-10).**
- **2,347 lines** of `{js}` across **18** `.tmd` files, measured by fence extraction. **Zero** raw
  `<script>` blocks in any `.tmd`. **DISPROVEN**: the brief's "~393 lines".
- Top offenders: `corpus/tech-blog/posts/a-star/index.tmd` **504**, `corpus/descent/index.tmd` **277**,
  `corpus/tech-blog/posts/fourier-transform/index.tmd` **258**, `site/_includes/three-scene.tmd` **237**,
  `corpus/tech-blog/posts/pca-geometry/index.tmd` **196**, `corpus/tech-blog/_includes/three-scene.tmd`
  **152**, `docs/guide/using/code.tmd` **140**, `corpus/tech-blog/posts/em-algorithm/index.tmd` **136**,
  `site/showcase.tmd` **130**.
- The two `tsc` gates cover `web-client/` and `crates/core/assets/js/` only (`CLAUDE.md`'s Commands block;
  both run in `gates.sh`). Neither sees a `.tmd`.
- `crates/core/src/diagnostics/reactive.rs` validates the **reactive graph** (which cell reads which
  `{{< input >}}`), not JavaScript syntax. Confirmed: no parser, no tokenizer.
- `corpus/reactive/js-error.tmd` exists (1 line of `{js}`) — a deliberate error fixture. **Any gate must
  exclude it or it will fail by design.**

**Files** — Modify: `tools/gates.sh` · New: one extraction script under `tools/`

**Steps**
- [ ] 1. Write the failing check first: a script that extracts every ` ```{js} ` fence from every `.tmd`
      under `corpus/`, `site/`, `docs/guide/` and `docs/internals/` and parses each with
      `node --check` (zero new dependencies — `node v24.18.0` is already a `gates.sh` prerequisite via
      `TALIESIN_REQUIRE_NODE`).
- [ ] 2. Run it against the tree and record what it finds. **If it finds nothing, say so and keep the
      gate anyway** — the value is preventing the next one.
- [ ] 3. Exclude `corpus/reactive/js-error.tmd` by name, with the reason in a comment beside the exclusion.
      Do **not** exclude by glob; a named exclusion cannot silently grow.
- [ ] 4. Add it to `tools/gates.sh` as a gate, following the `PASSED+=`/`FAILED+=` convention exactly, and
      update the count the script prints in its header and its `════ gates ════` summary.
- [ ] 5. **Read `crates/core/tests/gate_script.rs` before committing.** Its
      `every_pre_push_command_is_also_run_by_the_gate_script` compares `gates.sh` against
      `.githooks/pre-push`; adding to one and not the other is what R1 existed to fix.

**Traps**
- `node --check` is syntax only. It will not catch `d3` being undefined. Say so in the gate's own comment so
  the next session does not over-trust it.
- Cells inside `_includes/` are shared by several documents; a parse error there fails once, not per
  consumer. Fine, but the error message must name the include file.
- `{js}` fences can carry cell options (`//| label:`); the extractor must not choke on them.

**Verification**
- `./tools/gates.sh` prints the new count and names the gate in `PASSED`.
- Introduce a deliberate `{` into a `{js}` cell in `site/showcase.tmd`; the gate must fail. Revert.

**Done when** a syntax error in a shipped `{js}` cell fails a gate instead of reaching a reader.

---

### P9 — The collapsed table of contents lands above the page title, unlabeled

**Branch:** `fix/p9-toc-collapse` · **Size:** ~10 lines of CSS · **SHOULD SHIP**

**Why this ships before release.** Below 960 px, a `toc: true` page renders its heading list **above the
`<h1>`**, with no label, no container and an active-item accent bar — it reads as stray site navigation, not
as a table of contents. Screenshotted at 901 px on both the built single-file page and the live preview:
identical. At 1280 px and 1600 px it is a correct right rail.

**Verified state (checked 2026-08-10).**
- `crates/core/assets/css/base.css:783` — `@media (max-width: 60rem)`.
- `crates/core/assets/css/base.css:797` — `body.has-toc > #TOC { order: -1; position: static; … }`. The
  comment above it says *"Stack the TOC above the content"*, so the stacking is deliberate; the missing
  label is not.
- `crates/core/src/render/page.rs:565` — the nav ships `aria-label="Table of contents"`, so screen readers
  get the label and sighted readers do not.
- **`crates/core/assets/css/base.css:917` resets a pseudo-element nothing defines**:
  `#TOC::before { display: none !important; }` inside the print block. `rg 'TOC::before'
  crates/core/assets/css/` returns that one line. Dead CSS, and strong evidence a visible "Contents" label
  was intended and lost.
- **DISPROVEN — `CLAUDE.md`'s preview-vs-build TOC sentence.** See the disproven-claims block at the top.
  Fix that sentence in this commit.

**Files** — Modify: `crates/core/assets/css/base.css`, `CLAUDE.md`

**Steps**
- [ ] 1. Inside the `max-width: 60rem` block, give `body.has-toc > #TOC` a visible label via `::before`
      (`content: "Contents"`, token-driven colour, small caps or the existing label scale) — the print reset
      at `:917` already anticipates exactly this and becomes live rather than dead.
- [ ] 2. Verify at 901 px that the label reads and the rule below it still separates TOC from title.
- [ ] 3. **`cargo build` before rebuilding any site** — `base.css` is `include_str!`-compiled, so a rebuilt
      site re-emits the old CSS and you will measure a stale page. A live `preview` hot-swaps CSS, so this
      bites the build-and-inspect loop only.
- [ ] 4. Correct `CLAUDE.md`'s single-page-chrome paragraph. Replace the preview/build TOC disagreement
      sentence with the measured truth: neither verb auto-detects on the single-page path, and both honour
      an explicit `toc: true` identically.

**Traps**
- The `@media print` block at `:911-921` overrides `#TOC` layout; check the print rendering after adding the
  `::before`, since `:917`'s `!important` reset now has a real subject.
- `crates/core/tests/a11y_outline.rs` and `book_has_no_rail_toc.rs` both assert TOC structure. Read both.
- Books deliberately have **no** rail TOC (`book_has_no_rail_toc.rs:96,:100`). Do not make the label appear
  in a book.
- **Do not add a knob.** The near-perfect-default rule: one better default, no config key.

**Verification**
- Screenshot at 901 px and 1600 px via the chrome-devtools MCP, both `build page.tmd` output and
  `preview page.tmd`, light and dark.
- `cargo test -p taliesin-core` — the a11y and book-TOC suites green.

**Done when** the collapsed TOC announces itself as a table of contents, and `base.css:917` resets something
that exists.

---

### P10 — `--out <dir>` still base64-inlines 164 KB of fonts into every page

**Branch:** `fix/p10-out-dir-fonts` · **Size:** ~25 lines · **NICE TO HAVE**

**Why this ships.** `1a82f2ef` established that `--out <dir>`'s contract permits sibling assets, and used it
to take a diagram page from 3,803,493 B to 238,323 B. The same argument applies to the fonts and nobody has
made it. **Measured: 164,040 of a 238,287-byte `--out` page (68.8%) is two base64 `data:` URIs.** Shipped as
sibling `.woff2` files they are 122,604 B — base64 costs a flat +33% — and a multi-page `--out` deploy would
cache them once instead of paying per page.

**Verified state (checked 2026-08-10).** `build diag/d.tmd --out diag/out` produced `index.html` (238,287 B,
`data:` URIs 164,040 B) plus `mermaid.min.js` (3,565,102 B) and **no font files**. `script srcs: []` and the
only `<link href>` is a base64 SVG favicon; mermaid loads via a dynamic `s.src = 'mermaid.min.js'`. The site
path already does this right: `build <dir>` emits `newsreader-latin-wght-{normal,italic}.<hash>.woff2` as
shared assets.

**Files** — Modify: `crates/core/src/render/page.rs` (the `AssetMode` arm for `--out`),
`crates/server/src/build.rs` (the sibling writer)

**Steps**
- [ ] 1. Write the failing test asserting an `--out <dir>` build writes the two `.woff2` siblings and that
      the page contains no font `data:` URI.
- [ ] 2. Reuse the sibling-asset path `1a82f2ef` added for mermaid. **Do not write a second one.**
- [ ] 3. **Leave the true single-file `build page.tmd` spelling alone** — self-contained is its contract,
      exactly as the mermaid wave concluded.
- [ ] 4. Measure and record before/after in the commit body.

**Traps**
- The favicon is also a base64 `data:` URI and is 367 B. Leave it; a sibling file for 367 B is worse.
- `crates/server/tests/build_reproducibility.rs` and `parallel_build_determinism.rs` compare emitted asset
  sets. Read both before changing what `--out` writes.
- The `AssetMode::Inline` arm is shared with `--stdout`, which has no directory to write siblings into.
  Confirm which mode you are branching on.

**Verification**
- `build page.tmd --out d && ls d` shows two `.woff2`; `grep -c 'data:font' d/index.html` → 0.
- `build page.tmd` (single file) still contains them inline.

**Done when** `--out <dir>` ships its fonts as files and the single-file spelling still ships one file.

---

### P11 — Three small honesty edges, one commit

**Branch:** `fix/p11-small-edges` · **Size:** ~15 lines · **NICE TO HAVE**

**Verified state (checked 2026-08-10).**
- **`init` and `new` log with the word "built".** `  built   myblog/_site.yml`. Nothing was built; two files
  were written. The same `log::built` helper is correct for `build`. A `log::wrote` (or reusing the
  scaffold's own verb) costs one function.
- **`build <dir>` reports "0 assets" while writing four files into `_assets/`.** Measured on a fresh `init`:
  `2 pages  ·  0 assets  ·  search-index.js  ·  404.html`, and `_site/_assets/` then contains `app.<h>.js`,
  `app.<h>.css` and two `.woff2`. "Assets" means *copied local assets*; a first user counts files. One word
  ("0 local assets") fixes it.
- **`taliesin` with no arguments exits 0.** `main.rs:79-81` groups the no-args case with the explicit help
  request. Defensible (git does the same), listed so the next session does not re-discover it as a bug.
- **`new post` dates in UTC.** `cli.rs:259` `today_utc()`; reproduced a post dated `2026-08-09` created at
  `2026-08-10 00:27 EEST`. **This is documented and deliberate** (`cli.rs:256-258`: *"Taliesin has no date
  dependency and does not want one … the date is front matter they can edit"*). **Do not change it.**
  Recorded here so it is not re-opened.
- **The scaffolded homepage keeps its instructions after publish.** `INIT_INDEX_TMD` (`cli.rs:38-46`)'s
  "Next steps" list ships into the built site above the live listing. A starter you edit — arguably correct.
  Recorded, not proposed.

**Steps**
- [ ] 1. Add `log::wrote` (or rename the scaffold call site) so `init`/`new` stop claiming a build.
- [ ] 2. Change `build`'s summary to "N local assets".
- [ ] 3. Do **nothing** about UTC dates, the no-args exit code, or the starter homepage. Record the
      reasoning in the commit body so the next audit does not re-file them.

**Traps** `crates/server/tests/init_cli.rs` and `new_cli.rs` may assert on the `built` prefix — grep before
renaming. `build`'s summary line is read by `tools/build-site.sh`; check it before changing the wording.

**Verification** `cargo test -p taliesin-server --test init_cli --test new_cli`, then
`TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh`.

---

### The shipped weight, per page shape (item 5, measured — do NOT re-litigate mermaid)

**Mermaid is a ruled KEEP (2026-08-09, by the author, on a re-measurement). Nothing below reopens it.** The
`--out` residual was paid by `1a82f2ef` and is verified working: the page links a sibling
`mermaid.min.js` via `s.src = 'mermaid.min.js'`, taking a 2-node diagram page from **3,803,493 B → 238,287 B
(−93.7%)**.

**Verified: mermaid is conditional on the site path, checked rather than trusted.** On a 4-page site with one
diagram page, only `diagram.html` carries `<script src="_assets/mermaid.<hash>.js" defer>`. The other three
pages match the string "mermaid" only inside `app.js` source comments.

| shape | command | on disk | gzipped |
|---|---|---|---|
| prose page, single file | `build post.tmd` | **231,337** | **144,153** |
| — inline CSS | | 200,471 (86.7%) | — |
| — of which base64 `woff2` | | **164,040 (70.9% of the page)** | — |
| — inline JS | | 33,248 (14.4%), **43.3% comments** | — |
| diagram page, single file | `build d.tmd` | **3,803,493** | **1,122,314** |
| diagram page, `--out <dir>` | `build d.tmd --out o` | 238,287 + 3,565,102 sibling | ~145 K + 971,040 |
| site page (prose) | `build <dir>` | 15,631 | 6,353 |
| site shared CSS | | 47,570 | 9,015 |
| site shared JS (`app.<h>.js`) | | 77,290 | 25,428 |
| — of which `search.js` | | **48,550 (62.9% of the bundle)** | — |
| site fonts (2 × `woff2`) | | 122,604 | **122,374** |
| site mermaid (diagram pages only) | | 3,572,392 | ~971,040 |
| **first load, prose site page** | | **262,095** | **163,170** |

**The finding nobody has looked at.** On a prose site page's first load, the **two web fonts are 75% of the
gzipped weight (122,374 of 163,170)** — five times the CSS and larger than the entire JS bundle including
Cmd-K search. `mermaid` is conditional and `search.js` compresses to under 10 KB inside `app.js`. **If a
weight conversation is wanted after release, it is a font conversation, not a mermaid or a search
conversation.** No action proposed here: scope is closed, and the fonts are a design decision, not residue.

---

### Traps that apply to the whole backlog

- **The ordering rule.** No item here deletes a feature, so no corpus pin moves. If you find yourself
  deleting a `.tmd`, you have left this section.
- **`docs/guide/reference/cli.tmd` is the fifth, ungated registration site for every verb.** P1, P3 and P5
  all touch CLI surface. `stale_docs.rs:552` gates docs→CLI only. **Grep, do not trust.**
- **`tools/gates.sh` and `.githooks/pre-push` are compared on every test run** by
  `gate_script.rs`'s `every_pre_push_command_is_also_run_by_the_gate_script`. P4, P6 and P8 all touch
  `gates.sh`. Read the test before editing the script.
- **`crates/core/assets/*` are `include_str!`-compiled.** P6 and P9 both edit assets: `cargo build` before
  measuring any rebuilt site.
- **Do not touch the one standing freeze** (`MAX_WARM_PAGES` + the LRU in `serve_site/exec_pool.rs`).
  Nothing here goes near it.
- **Gate wall clock is ~25 min warm, materially longer after P4 deletes `_freeze/`.** Budget for it.
- **The scratch walkthrough is reproducible.** Every observation above came from
  `taliesin init myblog` → `new post` → `build` → `build --check-only` → `preview` → `doctor` against
  `target/release/taliesin`. Nothing here is inferred from source alone, and nothing is invented.
