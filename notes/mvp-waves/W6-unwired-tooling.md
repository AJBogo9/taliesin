# W6 — NOT TAKEN. See `notes/2026-08-10-mvp-publish-session.md` for the ship-path outcome

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

---

> ## ⚠ CORRECTIONS — READ FIRST, THEY OVERRIDE THE SECTION BELOW
>
> A skeptic pass over the assembled plan found 21 defects **in the plan itself**. The ones
> affecting this wave are below. Where a correction contradicts the section body, the
> correction wins. Line numbers throughout were true at `1a82f2ef` on 2026-08-10, and every
> wave that lands before yours shifts them — **grep, do not trust**.
>
> - **Step 9's stop condition is unachievable today.** `git status --porcelain` shows `?? tools/__pycache__/` and nothing ignores it — any session that ran the census (which W1 step 1 requires) has it. Either take W10 P4 step 1 first (`__pycache__/` into `.gitignore`) or relax the check to `git status --porcelain -- ':!tools/__pycache__'`.
> - **Your verification runs the census and expects a literal `82 documents, 7156 lines`.** That is correct today and correct after this wave (`samples/` is not under `corpus/`), **but W1 or W7 landing in between makes your line wrong for a reason that is not your fault.** Prefer "the census output is unchanged from the branch point".
> - **W10 P4 currently protects `tools/ui-audit/.work` as a legitimate fixture.** You `rm -rf` the whole directory. Whichever lands second must not re-derive this: after you, only `editor/vscode/.vscode-test/` is protected.
> - **Land before W9** (you close the `notes/CUT-PROGRESS.md:135` open item that W9's inventory would otherwise still count) and **before W5 → W2** on `crates/core/tests/stale_docs.rs` (you remove two entries and shrink the walk 38 → 37; the floor at `:56` is `> 25`, so there is headroom, but re-measure rather than trusting the `~38` in the comment).
> - **Rollback:** step 2's `rm -rf` destroys 382 MB of untracked working tree. `node_modules/` comes back with `npm i`; **`.work/`'s captured screenshots do not.** Say so before running it.
> - Counts confirmed exact: 16 files / 3,026 lines under `tools/ui-audit`, 729 lines across `tools/record-demo` + `samples`, one non-`notes` reference each (`retired_names.rs:40`, `site/README.md:74`).

---

### P1-1 — Delete the unwired tooling (ui-audit, record-demo, samples)

**Branch:** `cut/unwired-tooling` · **Kind:** deletion · **Size:** ~3,780 lines · **Blocked by:** none

**Why this ships before release.** Three directories totalling **3,755 tracked lines and 382 MB on disk** are in the repo, and **no gate, hook, workflow or `Cargo.toml` member reads any of them**. They are not free: waves 11, 12 and R6-1 each had to hand-edit `tools/ui-audit/` — a harness that runs nowhere — and `notes/CUT-PROGRESS.md:135` carries the standing warning that it "IS NOT CLEAN BY DEFAULT AND NO GATE READS IT." `tools/ui-audit/` also carries a **fifth, ungated corpus register**: four sites that name corpus documents by path and one that parses `corpus/README.md`'s table, so the corpus has a consumer no test knows about. This wave has the highest lines-per-risk ratio in the plan: it deletes no product code, adds no register entry, and closes one open item outright.

**Verified state (checked 2026-08-10, HEAD `1a82f2ef`).**

*Subject 1 — `tools/ui-audit/`*
- 16 tracked files, **3,026 lines** (`git ls-files tools/ui-audit | xargs wc -l`); **382 MB** on disk (`du -sh` — the rest is gitignored `node_modules` + `.work/`).
- `git grep -n "ui-audit" -- . ':!tools/ui-audit/' ':!notes/'` returns **exactly one line**: `crates/core/tests/retired_names.rs:40`, a comment above the `".work"` entry in `SKIP_DIR_NAMES` (:32-48).
- Not in `Cargo.toml` members, `tools/gates.sh`, `.githooks/pre-push`, `.github/workflows/`, `CLAUDE.md`, `CONTRIBUTING.md` or `README.md`. **DISPROVEN:** the brief says CLAUDE.md records the open item — it does not (`git grep -c ui-audit CLAUDE.md` = 0). The open item is `notes/CUT-PROGRESS.md:135`.
- The ungated corpus register, **four sites not five**: `probe-run.mjs:33` (`corpus/media/gallery.tmd`), `probe-run.mjs:52` (`corpus/analyst`), `make-sweep-index.mjs:37-38` (`corpus/demo-book/appendix.tmd`, `corpus/tech-blog/posts/draft-example/index.tmd`), `make-sweep-index.mjs:50` (reads and parses `corpus/README.md`). **DISPROVEN:** `capture-run.mjs` does not pin `corpus/descent`; its only occurrence is a `--only` usage example in a header comment at :20.
- `TALIESIN_BIN` is read by four files, all inside `tools/ui-audit/` (`probe-run.mjs`, `README.md`, `build-sweep.mjs`, `capture-run.mjs`). No Rust reads it, so it needs no `RETIRED_FLAGS` entry — it was never a CLI flag.

*Subject 2 — `tools/record-demo/`*
- 11 tracked files, **471 lines**, 56 KB on disk. Three demo `.tmd` files under `demos/` that `corpus.rs`'s walk never reaches (it walks `corpus/` only).
- Exactly one reference outside itself: `site/README.md:74` (`cd tools/record-demo`), inside a ```` ```sh ```` fence at :73-82. `stale_docs.rs`'s `backticked()` at :100-124 strips fenced blocks before extracting, so **nothing can see it**.
- **The section is already false.** `site/README.md:69` claims `assets/{live-edit,live-code}-{light,dark}.mp4` (four files); `git ls-files site/assets` shows **two**: `live-code-dark.mp4`, `live-edit-dark.mp4` (plus `og-card.png`). The `cp` at :80-81 names four files, two of which have never existed.
- **The two mp4s are live hero content and must NOT be deleted**: `site/index.tmd:77` and `site/features.tmd:15` embed `live-edit-dark.mp4`, `site/index.tmd:121` embeds `live-code-dark.mp4`, all as `<video src=…>`.

*Subject 3 — `samples/`*
- 4 tracked files, **258 lines** (`README.md` 48, `paper.tmd` 150, `references.bib` 33, `assets/backdrop.svg` 27).
- `git grep -n "samples/" -- . ':!samples/' ':!notes/'` returns **exactly two lines**: `crates/core/tests/stale_docs.rs:44` (`shipped_docs()`) and `:311` (`is_repo_path_claim`'s `ROOTS`). No shipped doc, script, hook or workflow names it.
- Built by nothing: `tools/build-site.sh:38-44` composes `docs/guide`, `docs/internals`, `corpus/tarn`, `corpus/descent`, `corpus/analyst` — five sub-projects, no `samples`.
- `samples/README.md` is dead on arrival: :3 says "One comprehensive document per Taliesin HTML **format**" (`DocFormat` went in wave 5; HTML is the only target) and :48 says "not from a **mounted** view" (`mounts:` cut in wave 11).
- **`samples/assets/backdrop.svg` is an orphan.** `paper.tmd` references no image; `git grep -n backdrop` finds no reference to it anywhere.

*Floors and gates, measured*
- `stale_docs.rs` walk floor `out.len() > 25` (:55-59): measured **38** today, **37** after removing `samples/README.md`. Headroom 12.
- `stale_docs.rs` path-claim floor `checked >= 60` (:406-410): measured **135** today; `samples/README.md` contributes **7**, leaving **128**. (**DISPROVEN:** the comment at :405 says "122 survive" — drifted, but harmless. Do not "fix" it in this wave.)
- `retired_names.rs` walker floor `files.len() > 100` (:169-173): the repo has thousands of files; removing 31 cannot approach it.
- `svg_assets_render.rs:25-30` floors at `>= 10` SVGs across `corpus`/`site`/`docs`/`crates/core/assets` — `samples/` is not one of its roots, so deleting `backdrop.svg` does not touch it.
- `gate_script.rs`'s `every_pre_push_command_is_also_run_by_the_gate_script` compares `tools/gates.sh` against `.githooks/pre-push`. Neither names anything this wave deletes; both files are untouched.

**Files**
- Delete: `tools/ui-audit/` (16 files), `tools/record-demo/` (11 files), `samples/` (4 files)
- Modify: `crates/core/tests/retired_names.rs` (drop the dead `".work"` skip entry + its comment), `crates/core/tests/stale_docs.rs` (drop `"samples/README.md"` from `shipped_docs()` and `"samples/"` from `ROOTS`), `site/README.md` (rewrite "The screencasts"), `notes/CUT-PROGRESS.md` (close the open item), `notes/DO-NOT-REBUILD.md` (one line), `notes/DETECTION-DEBT.md` (re-point rows 43-44)

**Steps**

- [ ] 1. Branch: `git switch -c cut/unwired-tooling` off `main`. Record the pre-delete sha for the recovery note: `git rev-parse --short HEAD` (expected `1a82f2ef` if nothing has landed since).
- [ ] 2. `git rm -r tools/ui-audit tools/record-demo samples`. Then **`rm -rf tools/ui-audit tools/record-demo`** — `git rm` leaves the gitignored `node_modules/` and `.work*/` behind, which is where 382 of the 382 MB live. Confirm with `du -sh tools/` and `find . -maxdepth 4 -name .work -not -path '*/node_modules/*'` (must print nothing).
- [ ] 3. `crates/core/tests/retired_names.rs`: delete the two lines at :40-41 — the comment `// The ui-audit harness's gitignored scratch build (`tools/ui-audit/.work/`).` and the `".work",` entry beneath it. Leave `".venv-audit"`, `".venv"` and everything else in `SKIP_DIR_NAMES` alone. This is a skip list consulted at :127, not an assertion, so removing an entry cannot make anything vacuous — it can only widen the walk, and step 2 proved there is nothing left for it to walk into.
- [ ] 4. `crates/core/tests/stale_docs.rs`: delete line 44 (`"samples/README.md",`) from `shipped_docs()`'s array, and delete line 311 (`"samples/",`) from `is_repo_path_claim`'s `ROOTS`. Leave `"tools/"` in `ROOTS` — it stays and becomes a self-enforcing tombstone: any future doc that backticks `tools/ui-audit/...` now fails `shipped_docs_do_not_name_a_file_that_does_not_exist`. Do **not** touch the two floor constants (`> 25` at :56, `>= 60` at :407) or their comments.
- [ ] 5. `site/README.md`: replace lines 67-82 (the whole `## The screencasts` section, heading through closing fence) with a short truthful section. It must (a) name the two files that actually exist, `assets/live-edit-dark.mp4` and `assets/live-code-dark.mp4`; (b) say they are committed artifacts embedded by `index.tmd` and `features.tmd`; (c) say the scripted recorder was deleted on 2026-08-10 and give the recovery command `git show <sha-from-step-1>:tools/record-demo/README.md`. Do **not** write the four-file `{light,dark}` claim again, and do **not** delete the mp4s.
- [ ] 6. `notes/DO-NOT-REBUILD.md`: add **one line** (the file's stated convention, :9) recording that `tools/record-demo/` was deleted on 2026-08-10, that the two committed screencasts are its only surviving output, and the `git show <sha>:tools/record-demo/` recovery path. One line — the deliberation belongs in the commit message.
- [ ] 7. `notes/DETECTION-DEBT.md`: rows 43 and 44 both name "a browser probe in `tools/ui-audit`" as the fix for the revision-digest and live-cell-output classes. That file declares itself "a live file, not a dated findings doc" (:14), so it is not covered by the `notes/` exemption. Edit the "what would change this" cell of both rows in place to say the harness was deleted on 2026-08-10 and any such probe would have to be built fresh. Do not change either `D` score — detection did not change; only the proposed remedy did.
- [ ] 8. `notes/CUT-PROGRESS.md`: the open item at :135-141 (`**tools/ui-audit/ IS NOT CLEAN BY DEFAULT AND NO GATE READS IT.**`) is closed by this deletion. Replace the bullet with a one-line closure naming the date and this branch. Append the wave to the Log tail in the file's existing format.
- [ ] 9. `git status --porcelain` must show only the intended deletions and the six modified files. Then run the gate (see Verification) and commit once.

**Decision on `samples/paper.tmd`: DELETE, declining the judge's relocate recommendation.**

The recommendation's premise — that relocating under `corpus/` adds coverage — is true. What I checked and it does not survive is that the coverage is **new**. Verified construct by construct against the tree today:

| `paper.tmd` construct | already pinned in corpus at |
|---|---|
| `{#eq-}` numbered display equations | `corpus/analyst/index.tmd`, `corpus/demo-book/{intro,methods}.tmd`, `corpus/tech-blog/posts/{fourier-transform,Kruskal-Wallis-test}/index.tmd` |
| `: caption {#tbl-}` authored table | `corpus/analyst/index.tmd`, `corpus/analyst/methods.tmd` (and `corpus/README.md` names this as analyst's job) |
| `.callout-warning collapse="true"` | `corpus/tech-blog/posts/{em-algorithm,Kruskal-Wallis-test,evidence-lower-bound}/index.tmd` |
| `::: {.column-margin}` | `corpus/layout/escapes.tmd:10` |
| `bibliography:` + `[@key]` + generated References | `corpus/single-page-report/`, `corpus/posts/cite-coverage/`, `corpus/shared-bib/` |
| `#| label: fig-` matplotlib cell | `corpus/analyst/`, `corpus/single-page-report/` |
| `@sec-` cross-refs, footnote, `toc: true` | throughout |

Beyond that: `corpus/single-page-report/` is **already the same genus** (a single-page research report with a `.bib`, `toc: true`, figures, tables, cross-refs and a Discussion), so the "only single-page research-paper exemplar" claim does not hold inside corpus. And `paper.tmd`'s own abstract (:16-18) says the document "doubles as a single-page exercise of every feature taliesin renders" — that is the self-declared feature-fixture construction the keep rule in `corpus/README.md:12-20` was written to retire, and after 13 waves the "every feature" claim is also simply false. With no unique coverage, a duplicate genus exemplar, and the standing directive breaking close calls toward cutting, it goes. Recovery is one command: `git show 1a82f2ef:samples/paper.tmd`.

**Sequencing against P0-1: none, and that is a consequence of this decision.** `tools/portability-census.py:96` defaults its root to `corpus`, so relocating the paper would move the figures P0-1 is correcting. Deleting instead leaves the census untouched. Measured today, so P0-1 can use these directly:

```
82 documents, 7156 lines under corpus/
498 lines (7.0%) carry a construct beyond CommonMark
```

against the currently-published `133 documents / 11,534 lines / 7.1%` at `README.md:28-29` and `docs/guide/using/choosing.tmd:16`. **Had the paper been relocated the numbers would be 83 / 7,307 / 7.2%** (the census counts `paper.tmd` as 151 lines, 31 of them beyond CommonMark) — recorded here only so P0-1 is not blind if the author overrides.

*Only if the author explicitly overrides and wants the paper kept:* create `corpus/paper/` holding `paper.tmd` + `references.bib` (a sibling `.bib` beside its document, the shape `corpus/posts/cite-coverage/` already uses); delete `samples/assets/backdrop.svg` regardless (it is an orphan and `svg_assets_render.rs` would then start walking it); delete the abstract's "doubles as a single-page exercise of every feature taliesin renders" sentence at :16-18; add one row to `corpus/README.md`'s two-column table; and hand P0-1 the 83 / 7,307 / 7.2% figures above. Its front matter is already clean against `KNOWN_KEYS` (`title`, `subtitle`, `author`, `date`, `bibliography`, `toc`), its cell options against `CELL_OPTION_KEYS` (`label`, `fig-cap`, `echo`), its callout kinds against `CALLOUT_KINDS`, its div class against `DIV_FEATURE_CLASSES`, and all four bib keys resolve — so `every_corpus_doc_has_clean_front_matter` and `every_corpus_doc_emits_no_unknown_key_warnings` would pass unchanged.

**Traps**

- **The ordering rule does not fire here, and the reflex to invoke it is wrong.** `tools/ui-audit/` *names* corpus documents; it does not guard them. Deleting it removes a consumer, not a pin. No corpus document is deleted by this wave (under the DELETE decision, `samples/` was never under `corpus/`), so `corpus/README.md` needs **no row edit** and `crates/core/tests/corpus.rs` sweeps exactly what it swept before.
- **`git rm` will not reclaim the 382 MB.** `node_modules/` and `.work*/` are untracked. Step 2's `rm -rf` is the load-bearing half; skipping it leaves the disk cost and an orphaned directory that later greps will keep finding.
- **Do not delete `site/assets/live-*.mp4`.** Three `<video src=>` tags depend on them (`site/index.tmd:77`, `site/index.tmd:121`, `site/features.tmd:15`). Deleting the recorder is the point; deleting its output breaks the landing page.
- **Nothing will tell you if step 5's replacement prose is wrong.** The stale section is inside a ```` ``` ```` fence, which `stale_docs.rs:100-124` strips before extraction, and line 69's token contains `{`, which `is_repo_path_claim` (:324) rejects outright. Both gates are structurally blind here. Verify by hand with the grep in Verification.
- **`retired_names.rs`'s `SKIP_DIR_NAMES` does not go vacuous, and `stale_docs.rs`'s floors do not go near their limits** — both measured above. What *would* be a mistake is lowering either floor "to be safe": they are anti-vacuity floors, and `notes/CUT-PROGRESS.md:128-134` records that moving one without measuring is how two previous waves hard-failed.
- **`notes/` is exempt from the stale gates, but `notes/DETECTION-DEBT.md` is not exempt from being true** — it declares itself live at :14 and forbids a second copy. Step 7 is not optional tidiness; without it the file's remedy column points at a directory that does not exist.
- **Adjacent, deliberately out of scope:** `tools/build-site.sh:2` says "the four gallery exhibits" while the `subprojects` array at :38-44 carries three. Real, ungated (a shell comment reaches no gate), and not this wave's job. Surface it, do not fix it here.
- **A red herring you will hit if you grep with `--hidden --no-ignore`:** `editor/vscode/out/e2e/suite/integration.test.js:139` mentions `tools/ui-audit`. `editor/vscode/.gitignore:2` ignores `out/`; it is stale compiled output and no `.ts` under `editor/vscode/src/` carries the string. Leave it.
- **`rg -r` is the replace flag, not a recursion flag.** Writing `rg -rn "ui-audit" .` silently rewrites every match to `n` in the output and will make you believe the tree says something it does not. Use `git grep -n` for the reference sweeps below.

**Verification**

```sh
# 1. Nothing tracked names any deleted directory.
git grep -n "ui-audit"     -- . ':!notes/'   # expect: no output
git grep -n "record-demo"  -- . ':!notes/'   # expect: no output
git grep -n "samples/"     -- . ':!notes/'   # expect: no output
git grep -n "TALIESIN_BIN" -- .              # expect: no output

# 2. The directories and their gitignored bulk are gone.
ls tools/                                      # no ui-audit, no record-demo
du -sh tools/                                  # was 382M+, expect < 1M
find . -maxdepth 4 -name .work -not -path '*/node_modules/*'   # expect: no output

# 3. The screencasts still exist and are still embedded.
git ls-files site/assets                       # expect live-code-dark.mp4, live-edit-dark.mp4, og-card.png
grep -rn 'live-.*-dark\.mp4' site/*.tmd        # expect 3 hits: index.tmd x2, features.tmd x1
grep -n 'light,dark' site/README.md            # expect: no output (the false claim is gone)

# 4. The census is unchanged, so P0-1 is unaffected.
python3 tools/portability-census.py            # expect: 82 documents, 7156 lines, 498 (7.0%)

# 5. The gate. ~25 min, 10 gates; bare gates.sh exits 2 at preflight on this machine.
TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh
```

Gate 5 must print its 10-gate count with every gate RUN (no `SKIPPED`) and exit 0. The specific tests that must be green and are the ones this wave can break: `retired_names::the_retired_brand_stays_retired`, `stale_docs::shipped_docs_do_not_name_a_file_that_does_not_exist`, `stale_docs::shipped_docs_do_not_use_a_retired_front_matter_key`, `stale_docs::documented_cli_flags_exist_in_the_cli`, `gate_script::every_pre_push_command_is_also_run_by_the_gate_script`.

**Done when** `git grep -n -e ui-audit -e record-demo -e 'samples/' -- . ':!notes/'` prints nothing, `du -sh tools/` is under 1 MB, the three `<video>` tags still resolve to two committed mp4s, `python3 tools/portability-census.py` still prints `82 documents, 7156 lines`, and `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh` exits 0 with all 10 gates run — in one commit on `cut/unwired-tooling`.