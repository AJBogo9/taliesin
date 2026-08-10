# W7 — NOT TAKEN. See `notes/2026-08-10-mvp-publish-session.md` for the ship-path outcome

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
> **⚠ THIS WAVE NOW HAS A HARD DEPENDENCY IT WAS NOT WRITTEN WITH.** It deletes three corpus
> documents, and since `17bb5f93` the census is **gate 11**
> (`python3 tools/portability-census.py --verify`): `README.md` and
> `docs/guide/using/choosing.tmd` must publish exactly the document count, line count,
> beyond-CommonMark count, percentage, complement and six `| n | share |` pairs the
> instrument measures. **Deleting a corpus document without re-running the census and
> rewriting both pages IN THE SAME COMMIT turns `gates.sh` red.** This was cross-wave hazard
> 1; skipping this wave dissolved it, and landing W1 armed it.

---

> ## ⚠ CORRECTIONS — READ FIRST, THEY OVERRIDE THE SECTION BELOW
>
> A skeptic pass over the assembled plan found 21 defects **in the plan itself**. The ones
> affecting this wave are below. Where a correction contradicts the section body, the
> correction wins. Line numbers throughout were true at `1a82f2ef` on 2026-08-10, and every
> wave that lands before yours shifts them — **grep, do not trust**.
>
> - **THE CORPUS COUNT IN STEP 4c IS WRONG, AND ITS OWN VERIFICATION COMMAND CONFIRMS THE WRONG NUMBER.** The section says the count is 79 today and 76 after. `git ls-files 'corpus/**/*.tmd'` returns 79 **because that glob misses the three top-level `corpus/*.tmd` files**. The true count is **82 today, 79 after this wave**. Use `git ls-files 'corpus/*.tmd' 'corpus/**/*.tmd' | wc -l` in both the claim and the verification, and write **79** into `CLAUDE.md:22`. This is the exact self-confirming-falsehood shape this plan exists to prevent.
> - **ADD A MANDATORY STEP BEFORE COMMIT: re-run the census and republish.** You delete `corpus/render-fixes/` and `corpus/reader/` — 3 documents, 147 lines — which moves eight of W1's sixteen census assertions (82→79 documents, 7,156→7,009 lines, 498→491 beyond-CommonMark, and five of the six construct rows). **If W1's census gate has already landed, this wave turns `gates.sh` red without it.** Re-run `python3 tools/portability-census.py`, update `README.md:28-29` and `docs/guide/using/choosing.tmd`, confirm `--verify` exits 0. **Recommended order is W7 before W1**, which makes this a one-line confirmation instead of a republish.
> - **`corpus/callouts/` is KEPT and out of scope.** The section defers it to "a wave that cuts callout `appearance=`/`icon=`" — **no such wave exists in this plan.** Say so plainly, or a session will invent the wave or delete the fixture and violate the ordering rule.
> - **Step 3d's `$/progress` claim is false as written.** `grep -n '\$/progress' crates/server/src/lsp.rs` is **not** empty today: `lsp.rs:3984` carries the test-section banner. It becomes empty only after step 3c. Restate as "no `$/progress` implementation exists; the only occurrence is the banner at `:3984`, which step 3c removes".
> - **Step 2a deletes `crates/core/assets/schema/tali-site.schema.json`, which W8 step B9 regenerates.** W8 is declined; if it is ever taken it must run **first**, or B9 names a deleted file.
> - **Rollback for steps 3a-3b (the `lsp.rs` `main_loop` rewrite) — the only subject here that can go red for a real reason:** if `cargo test -p taliesin-server lsp` is red, `git checkout -- crates/server/src/lsp.rs` and split the cancel-batching out to its own branch. Keep `a_stream_of_requests_does_not_starve_a_pending_publish` green as the tripwire.
> - **Step 1a's four deletion ranges (`830-855`, `724-764`, `620-668`, `290-590`) are exactly right on today's 855-line file. Do not "adjust" them** — but W6 deletes `retired_names.rs:40-41` first, which shifts them by 2, so re-derive by test name as the section already mandates.
> - Amend the trap paragraph: `render/tests.rs:2951-2958` is **owned by W5**, not "deliberately not fixed here", or the next reader files it a third time.

---

### R7 — The dead-code sweep: five verified-dead subjects, two rejected

**Branch:** `cut/r7-dead-code-sweep` · **Kind:** deletion · **Size:** ~1,050 lines · **Blocked by:** none

> Absorbs the remediation plan's **R6-4** (tombstones), **R6-5** (the `_site.yml` schema) and four
> items of **R6-12**. It **rejects** two items the brief carried; see *Disproven* below and act on the
> rejections, they are load-bearing.

**Why this ships before release.** Fifteen tests in `retired_names.rs` assert that strings whose
emitters were deleted in waves 3/5/7 are still absent — they cannot fail, and a future session
reading them will believe the file is coverage rather than sediment. `lsp.rs` carries 275 lines of
`$/cancelRequest` machinery whose own doc comment retires it ("The measured example was
`workspace/symbol` … that method went on 2026-08-08"), and CLAUDE.md teaches that machinery as
doctrine. `vocab["theoremKinds"]` indexes a key `vocab()` stopped emitting, so two LSP completion
arms silently contribute nothing. None of this is product; all of it is a future session's wrong turn.

**Verified state (checked 2026-08-10).**

*Subject 1 — tombstones.* `crates/core/tests/retired_names.rs` is **855** lines, **21** tests.
Exactly **15** are string-absence tombstones against `render::code_scripts()` / `base_css()` /
`site_css()` / `TOC_SPY_JS` / a rendered page, totalling **417** lines. **Six must survive**, and a
careless range takes four of them:
`the_retired_brand_stays_retired` (:162-212), `the_guard_detects_a_reintroduction` (:214-261),
`no_q_prefixed_identifier_ships_in_emitted_markup` (:263-288) — the `q`-brand guard CLAUDE.md:101
names; `forward_xrefs_survive_the_backlink_deletion` (:591-618) — renders `corpus/demo-book` and
asserts the forward `@fig-pipeline` xref resolves; `the_cut_theorem_kinds_keep_their_xref_prefixes…`
(:669-722); and `a_leftover_pyodide_cell_is_told_it_was_withdrawn_not_that_it_is_a_typo` (:765-829),
which is a live diagnostic assertion (severity field, location, both named replacements), not a
tombstone.

*Subject 2 — the schema.* `crates/core/src/schema.rs` is 146 lines; `SITE_SCHEMA` (:19) is read only
at :92, :116, :144 — all inside its own `#[cfg(test)] mod tests`. `grep -rn "schema::" --include="*.rs" crates`
returns **nothing** outside the file, and `crates/core/src/lib.rs:43` is its only declaration. The two
committed JSON files (`crates/core/assets/schema/tali-site.schema.json`,
`editor/vscode/schema/tali-site.schema.json`) are **86 lines each and byte-identical**.
`editor/vscode/package.json:93` wires the companion copy through `yamlValidation`.

*Subject 3 — cancel batching.* `crates/server/src/lsp.rs` is 4,138 lines.
`read_batch` (:409-455) + its doc (:395-407, whose own text retires it), `is_shutdown` (:457-462),
`cancel_target` (:464-480), `Batch::Messages` (:382-386), the `inbox`/`cancelled` decls (:222-227)
and the `-32800` reply block (:251-271). Tests: :3983-4137 (the `$/cancelRequest` banner + two
tests) and the orphan banner at :3977-3982 for a pull-model section with **zero tests under it**.
`symbol_fixture` (:1919) has exactly one caller, :4001 — it becomes dead. **`Batch::Timeout`,
`Batch::Closed` and `PendingPublishes` (:159-193) are the 120 ms `didChange` coalescing and stay.**
`a_stream_of_requests_does_not_starve_a_pending_publish` (:3750) drives a live connection, not
`read_batch`, and survives unchanged — removing the drain makes its property *stronger*, since
`pending.wait()` is then consulted on every iteration rather than only when `inbox` empties.

*Subject 4 — corpus.* `corpus/render-fixes/` (index.tmd 28 lines + `diagram.png` + an orphan
`clip.mp4` no source references since `{{< video >}}` went on 2026-08-08) and `corpus/reader/`
(`preferences.tmd` 51 + `long-read.tmd` 65 + 2 pngs). Zero references outside `corpus/` and `notes/`
(`grep -rn "render-fixes\|corpus/reader" --include=*.rs --include=*.sh --include=*.ts .`);
`tools/build-site.sh:41-43` names only `corpus/tarn`, `corpus/descent`, `corpus/analyst`. Both are
reached only by `crates/core/tests/corpus.rs`'s generic `collect_tmd` sweeps. Their constructs are
covered elsewhere: figure `height=` at `crates/core/src/render/tests.rs:1737-1755` and `:3796`;
mermaid at `corpus/demo-book/results.tmd` and `docs/guide/`.

*Subject 5 — dead code.* `highlight::known_language` + `INTENTIONALLY_PLAIN`
(`crates/core/src/highlight.rs:73-85`, tests :202-213 and :219) — referenced by nothing but its own
tests, workspace-wide, `.rs`/`.ts`/`.js`; `notes/CUT-PROGRESS.md`'s log tail already records it as
owed. `PageIncludes::after_body` — **zero writes**, sites listed under *Disproven*.
`editor/vscode/src/client.ts:32 TALIESIN_SOURCE` — exported, imported nowhere.
`vocab["theoremKinds"]` at `crates/server/src/lsp.rs:1153` and `:1172` — `vocab()`
(`crates/core/src/vocab.rs:341-380`) emits no such key, so both arms index `Value::Null` and
contribute nothing. Duplicate watcher: `client.ts:71-73` `synchronize.fileEvents` registers
`**/{*.tmd,_site.yml,*.bib}` while `lsp.rs:114 register_file_watchers` dynamically registers the same
three globs — VS Code fires both. `vscode-languageclient` declares
`didChangeWatchedFiles.dynamicRegistration = true` **unconditionally**
(`node_modules/vscode-languageclient/lib/common/fileSystemWatcher.js:27`, feature registered at
`client.js:1591`), so dropping the TS half leaves the Rust registration in charge for every editor.

**Disproven — read these before writing a line.**

1. **The cgroup-v2 walk is NOT cut.** `notes/2026-08-08-cut-playbook.md:985` already adjudicated it
   into a *Must survive* list, with reasoning. `crates/server/src/build_budget.rs` is untouched by
   this wave.
2. **The migrated-link extension suggestions are NOT cut.** Three live production call sites
   (`diagnostics/links.rs:89`, `site/mod.rs:815`, `:818`); live behaviour; not dead code.
   `crates/core/src/ext.rs` and `crates/core/tests/migrated_link_extensions.rs` are untouched.
3. **`corpus/callouts/` is deferred** to whichever wave cuts the callout `appearance=` / `icon=`
   attributes; that wave owns `kinds.tmd:19-31` under the ordering rule.
4. **`SITE_SCHEMA` is dead but the schema is not.** This wave collapses two copies to one; it does
   not delete the generator. See step 2.
5. Line/count corrections: 417 (not ~482) tombstone lines; corpus is 79 `.tmd` today (CLAUDE.md:22
   says 83); `corpus/reader/_freeze/text-projection.json` is untracked.

**Files**

- Modify: `crates/core/tests/retired_names.rs`, `crates/core/src/render/tests.rs`,
  `crates/core/src/schema.rs`, `crates/core/src/lib.rs`, `editor/vscode/src/test/manifest.test.ts`,
  `editor/vscode/src/client.ts`, `crates/server/src/lsp.rs`, `crates/core/src/highlight.rs`,
  `crates/core/src/render/{model.rs,page.rs,doc_includes.rs}`, `crates/server/src/serve_site/mod.rs`,
  `crates/server/tests/asset_bundle.rs`, `corpus/README.md`, `CLAUDE.md`,
  `docs/internals/{architecture.tmd,extending.tmd}`, `docs/guide/reference/configuration.tmd`,
  `notes/DO-NOT-REBUILD.md`, `notes/2026-08-09-remediation-plan.md`
- Delete: `crates/core/assets/schema/tali-site.schema.json`, `corpus/render-fixes/` (3 tracked files),
  `corpus/reader/` (4 tracked files)
- Re-point: `crates/core/src/schema.rs`'s golden target → `editor/vscode/schema/tali-site.schema.json`

**Steps**

- [ ] **1a. Delete the 15 tombstones from `crates/core/tests/retired_names.rs`, by these four ranges,
  bottom-up so the numbers hold:** `830-855`, `724-764`, `620-668`, `290-590`. That is 417 lines; the
  file lands at 438 with 6 tests. Re-run `rg -n '#\[test\]' crates/core/tests/retired_names.rs`
  afterwards and confirm exactly six, named: `the_retired_brand_stays_retired`,
  `the_guard_detects_a_reintroduction`, `no_q_prefixed_identifier_ships_in_emitted_markup`,
  `forward_xrefs_survive_the_backlink_deletion`,
  `the_cut_theorem_kinds_keep_their_xref_prefixes_so_a_stray_reference_errors_loudly`,
  `a_leftover_pyodide_cell_is_told_it_was_withdrawn_not_that_it_is_a_typo`.
- [ ] **1b.** Leave the helpers (`retired`, `repo_root`, `walk`, `line_offends`,
  `occurrence_is_exempt`, `SKIP_DIR_NAMES`, `SKIP_PATHS`) and both imports — the brand guard still
  uses every one of them, and `common::corpus_dir` is used by `forward_xrefs_…`. Add two sentences to
  the module doc (:1-10) saying the file is the brand guard **plus three live pins the retirement
  registers cannot derive** (a forward xref, three surviving xref prefixes, one retired-cell-language
  diagnostic), so the next reader does not delete them as leftovers.
- [ ] **1c.** In `crates/core/src/render/tests.rs`, inside
  `theme_head_separates_the_reader_choice_from_the_resolved_mode` (the `head` assertions at :1793-1801),
  add `assert!(head.contains("taliSetTheme"), "the head script must expose the setter the picker calls");`.
  This is the one positive needle the deleted `the_code_visibility_pre_paint_api_is_gone_and_theme_survives`
  was carrying alone. It is an assertion on live behaviour, not a tombstone.
- [ ] **2a. Collapse the `_site.yml` schema to one committed copy.** `git rm
  crates/core/assets/schema/tali-site.schema.json`. In `crates/core/src/schema.rs`: delete the
  `SITE_SCHEMA` const (:19-20) and rewrite `bless_or_assert` to take a path relative to the **repo
  root** (`Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")`, the same idiom
  `retired_names.rs:23` uses), reading the committed file from disk rather than through `include_str!`.
  Point `site_schema_matches_committed` and `the_schema_is_structurally_sane`'s final JSON-parse check
  at `editor/vscode/schema/tali-site.schema.json`. Keep `mod generate` and `NATIVE_KEYS` coupling
  intact — that is the whole gate.
- [ ] **2b.** In `crates/core/src/lib.rs:43`, change `pub mod schema;` to `#[cfg(test)]\nmod schema;`
  (nothing outside the file references it; verified by `grep -rn "schema::" --include="*.rs" crates`).
  Rewrite the module doc (:1-17) so it says there is now **one** committed schema, living in the
  companion because `contributes.yamlValidation` needs a file inside the extension, generated from
  `site::NATIVE_KEYS` and blessed with
  `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib schema`.
- [ ] **2c.** In `editor/vscode/src/test/manifest.test.ts`, replace the body of
  `test("the bundled _site.yml schema matches the one the binary emits", …)` (:396-418) with an
  existence-only check on each `yamlValidation` entry (the file exists and parses as JSON), and
  rewrite the comment above it (:390-395): the schema is no longer a copy, and the drift gate is now
  `schema.rs`'s golden inside `cargo test`. Rename the test to
  `"the _site.yml schema the manifest points at exists and parses"`. Leave
  `"yamlValidation matches only the config filename the loader reads"` (:422) and
  `"the schema directory is not excluded from the .vsix"` (:443) untouched.
- [ ] **2d. Docs and doctrine, same commit.** `docs/internals/architecture.tmd:267` — the `schema.rs`
  row says "shipped in a copy by the VS Code companion"; it is now the one copy, generated and
  golden-locked. `docs/internals/extending.tmd:202-205` — drop "pointing at a copy of the schema …
  `crates/core/assets/schema/`". `docs/guide/reference/configuration.tmd:177-178` — "ships a copy of
  that schema" becomes "ships that schema"; check :167 and :182 still read true.
  **`CLAUDE.md:379-386`**: the `_site.yml` entry in the FOUR-drift-gates paragraph must stop calling
  the file "a bundled COPY of the crate's schema gated only by the companion's own `node --test`" and
  stop saying `cargo test --workspace` can be green while it is stale — after this step
  `cargo test -p taliesin-core` is exactly what catches it. It is still four gates.
- [ ] **3a. Reduce `read_batch` to a single-message read** in `crates/server/src/lsp.rs`. Replace
  `Batch::Messages(VecDeque<Message>, HashSet<RequestId>)` with `Batch::One(Message)`; rename
  `read_batch` → `next_message` and reduce it to the `recv` / `recv_timeout` match at :410-421,
  returning `Batch::One(m)` / `Batch::Timeout` / `Batch::Closed`. Delete the drain loop (:423-435),
  the cancel partition (:436-454), `is_shutdown` (:457-462) and `cancel_target` (:464-480). Rewrite
  the doc comment (:395-407) to say what survives: this is the **120 ms `didChange` coalescing
  window**, nothing else.
- [ ] **3b.** In `main_loop`, delete the `inbox`/`cancelled` declarations (:222-227), the
  `if inbox.is_empty()` wrapper and `inbox.pop_front()` (:228-249, :250-252), and the
  `RequestCancelled` reply block (:253-271). The loop body becomes `let msg = match
  next_message(connection, pending.wait()) { Batch::One(m) => m, Batch::Timeout => { …publish
  pending…; continue } Batch::Closed => break };` followed by the existing `match msg`. **Do not
  touch `PendingPublishes` (:159-193) or the `Batch::Timeout` publish arm** — they are the coalescing.
- [ ] **3c.** Delete the test block `3977-4137`: the orphan pull-model banner (:3977-3982, zero tests
  under it), the `$/cancelRequest` banner (:3983-3985) and both tests
  (`a_cancelled_request_is_answered_rather_than_run`, `a_cancel_only_reaches_a_request_in_its_own_batch`).
  Then delete `symbol_fixture` (:1919-1934), whose only caller was :4001 — or let clippy name it
  (see step 6). **Leave `a_stream_of_requests_does_not_starve_a_pending_publish` (:3750) exactly as
  it is: it is this refactor's guard.**
- [ ] **3d. `CLAUDE.md:221-225`**: delete the "**`$/cancelRequest` is batch-scoped**" sentence
  through "…belongs to `handle_shutdown`". The `didChange` coalescing sentence immediately above it
  (:217-218) stays and is now the whole story. **`notes/DO-NOT-REBUILD.md:168-172`**: annotate item
  **223** the way item **217** is annotated at :154 — "**CUT WHOLE 2026-08-10 (wave R7); do not
  rebuild it**", naming the reason the code's own comment gave (the only measured beneficiary,
  `workspace/symbol`, went on 2026-08-08; of the six surviving providers only completion is
  per-keystroke and it is a single-buffer scan; ignoring `$/cancelRequest` is spec-legal). Note while
  there that item 223's `$/progress` half is **not in the tree** — `grep -n '\$/progress'
  crates/server/src/lsp.rs` is empty — so the entry was already half-false.
- [ ] **4a. `git rm -r corpus/render-fixes corpus/reader`** (7 tracked files). Then `rm -rf
  corpus/render-fixes/_freeze corpus/reader/_freeze` locally — untracked, no diff, but
  `corpus/reader/_freeze/text-projection.json` is R6-11 residue whose `.tmd` is already gone and
  leaving it invites the next reader to look for the document.
- [ ] **4b. `corpus/README.md`, same commit (ordering rule).** Delete the `reader/` row (:48). **Edit,
  do not delete, :52** — it is a shared row (`` `recipes/`, `render-fixes/` ``); it becomes a
  `recipes/`-only row naming the data-to-figure recipe. Leave the `callouts/kinds.tmd` row (:45) and
  the `structured-authors/` row (:42) alone.
- [ ] **4c. `CLAUDE.md:22`**: "The corpus is 83 documents" → **76**. It was already wrong before this
  wave (`git ls-files 'corpus/**/*.tmd' | wc -l` = 79 on the branch point); state the true
  post-deletion number, do not subtract from the stale one.
- [ ] **5a.** `crates/core/src/highlight.rs`: delete `INTENTIONALLY_PLAIN` + its doc (:73-76) and
  `known_language` + its doc (:78-85); delete the test
  `known_language_accepts_real_and_intentionally_plain_tokens` (:202-213); in
  `intentionally_plain_tokens_still_render_plain` (:215-220) delete **only** line :219
  (`assert!(known_language("text"))`) and reword the doc line :215 — the `highlight("a < b",
  Some("text"))` assertion at :218 is live coverage and stays.
- [ ] **5b.** Delete `PageIncludes::after_body` and its template slot.
  `crates/core/src/render/model.rs`: field :372, the merge tuple entry :402, and rewrite the struct
  doc :360-363, which still names four front-matter keys retired on 2026-08-02 and a `format:` key
  that never existed. `crates/core/src/render/page.rs`: `include_after_body` at :158, :185, the
  `{include_after_body}` template line :396 **including its newline**, the format arg :417, and the
  three redundant `include_after_body: ""` initializers at :803, :871, :913. `render/page.rs:716` and
  `crates/server/src/serve_site/mod.rs:810` (`include_after_body: &includes.after_body`) go with them.
  `crates/core/src/render/doc_includes.rs:22-24`: the comment must now say only `before_body` remains
  an unconfigured slot, written by the site chrome's draft banner. **Keep `before_body` — it is
  written at `crates/core/src/site/mod.rs:501`.**
- [ ] **5c.** `crates/server/tests/asset_bundle.rs`: rename
  `external_inlines_enhancer_registry_before_include_after_body` (:252) to
  `external_inlines_enhancer_registry_before_the_deferred_app_bundle` and fix the two comments that
  name the retired slot (:250, and the "proves the include-after-body wiring reached the page" line
  ~:294). The test injects through `head:` and its assertions do not change.
- [ ] **5d.** `crates/server/src/lsp.rs`: delete the `vocab["theoremKinds"]` completion arm
  (:1151-1155, the `out.extend(from_named(&vocab["theoremKinds"], …))`) and the
  `|| names_in("theoremKinds", c)` clause (:1172). Both index a key `vocab()` does not emit.
- [ ] **5e.** `editor/vscode/src/client.ts`: delete `TALIESIN_SOURCE` and its doc block (:27-32,
  imported nowhere), and the duplicate watcher — the comment and `synchronize` block at **:69-73**.
  The Rust `register_file_watchers` (`lsp.rs:114`) then serves every editor, VS Code included.
- [ ] **6. Let clippy enumerate the cascade, not you.** After the deletions above, run
  `cargo clippy --workspace --all-targets -- -D warnings` and delete what it names (this is R6-11's
  and R6-2's recorded method; it found 23 items once and `lsp_memo` entire another time). Two known
  blind spots to sweep by hand: an item reached only from a `#[cfg(test)]` module is **not** dead to
  `--all-targets`, so step 3c's tests must be gone before clippy can see `symbol_fixture`; and a
  `pub` item in `taliesin-core` is invisible to clippy entirely, so anything you suspect needs
  `grep -rn "<name>" --include=*.rs --include=*.ts --include=*.js . | grep -v '^./target'`.
- [ ] **7. Record the landing** in `notes/2026-08-09-remediation-plan.md`, following the
  `### R6-1 — LANDED …` / `### R6-11 — LANDED …` precedent at :690 and :720: what landed, the two
  subjects **rejected with their evidence** (the playbook's Must-survive ruling on the cgroup walk;
  the three live call sites behind the migrated-link suggestion), and the deferral of
  `corpus/callouts/` to the callout-attribute wave. Strike through the R6-4 and R6-5 rows (:680,
  :681) and mark the four R6-12 sub-items this wave took.

**Traps**

- **The four ranges in step 1a must be applied bottom-up and verified by test name, not by eye.**
  `290-590` is 301 consecutive lines of tombstone with **no keeper inside it**, and the very next
  test (:591) is live positive coverage that renders `corpus/demo-book`. One line of drift eats it.
- **Do not delete `crates/server/src/build_budget.rs`'s cgroup walk** (see *Disproven* 1) and **do not
  delete `crates/core/src/ext.rs`'s `migrated_source_candidates`** (see *Disproven* 2). If a sibling
  section in this plan tells you otherwise, this one has the file:line evidence.
- **`corpus/callouts/kinds.tmd` belongs to another wave.** Touching it here creates the ordering-rule
  violation both waves are trying to avoid.
- **`corpus/render-fixes/index.tmd` was edited one commit ago** (`1a82f2ef`, the mermaid `--out` truth
  fix; see `notes/CUT-PROGRESS.md`'s log tail). That was prose accuracy, not evidence of value: its
  two constructs are covered at `render/tests.rs:1737-1755` and by `corpus/demo-book/results.tmd`.
  Delete it, but say so in the step-7 note so the next reader does not think it was an accident.
- **Step 5b changes rendered page bytes by one newline** (`{scripts_post}\n{include_after_body}\n</body>`
  loses a blank line). Verified safe: `crates/core/tests/snapshots/` holds only three **`body_html()`**
  snapshots (`body_html_snapshots.rs:1`), not full pages, and `crates/server/tests/build_reproducibility.rs`
  compares two builds of the same tree. Nothing asserts an exact page tail
  (`grep -rn '</body>' --include=*.rs crates` → one hit, the template itself). Still: if a byte
  comparison fails, this is the cause.
- **Step 2 is the one with a sibling copy.** The `_site.yml` schema is named in three `.tmd` pages
  and one CLAUDE.md paragraph. Grep before declaring it done: `grep -rn "tali-site.schema\|assets/schema"
  --include=*.tmd --include=*.md --include=*.ts --include=*.json . | grep -v '^./target\|^./notes'`.
- **`tools/gates.sh`'s canaries are untouched.** They are `CANARY_KERNEL="kernel_executes_state_errors_and_interrupts_runaway_cell"`
  and `CANARY_NODE="only_a_textual_sink_becomes_a_live_region"` (gates.sh:74-75), and
  `crates/core/tests/gate_script.rs:195` asserts each still exists. Neither is in this wave's deletion
  set — but if you delete a test in step 6's cascade, re-check that list.
- **`stdout` is the LSP's JSON-RPC wire.** Step 3 rewrites the loop; no `println!`.
- **Adjacent, deliberately NOT fixed here:** `docs/internals/architecture.tmd:272` still lists
  `run` among the subcommands (cut in wave 13), and `THIRD_PARTY.md:50`'s stale
  `scrolly.js`/`tabset.js`/`walkthrough.js` line plus `third_party.rs`'s `OWN_JS` are still owed from
  the last wave's own note. Both are prose subjects with their own registers; surfacing them, not
  mixing them into a deletion diff.

**Verification**

- `rg -n '#\[test\]' crates/core/tests/retired_names.rs | wc -l` → **6**, and `wc -l` → **438**.
- `rg -n 'cancelRequest|read_batch|is_shutdown|cancel_target|Batch::Messages' crates/server/src/lsp.rs`
  → **no hits**. `rg -n 'cancelRequest' CLAUDE.md` → **no hits**.
- `rg -n 'known_language|INTENTIONALLY_PLAIN|after_body|theoremKinds|TALIESIN_SOURCE'
  --glob '!target' --glob '!notes' .` → only `before_body` / `include_before_body` survive, plus
  `docs/guide/reference/frontmatter.tmd:62`'s retirement sentence, which is prose about a 2026-08-02
  retirement and stays.
- `git ls-files corpus/render-fixes corpus/reader` → empty. `git ls-files 'corpus/**/*.tmd' | wc -l` → **76**.
- `ls crates/core/assets/schema` → gone; `editor/vscode/schema/tali-site.schema.json` still 86 lines.
- `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib schema` rewrites the **companion** file and
  leaves it byte-identical (`git diff --stat` empty) — proof the generator is still wired to
  `NATIVE_KEYS`.
- `cargo clippy --workspace --all-targets -- -D warnings` clean (this is also step 6's instrument).
- `cd editor/vscode && npm test` — the manifest suite green with the rewritten schema test.
- **`TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh`** on the tree you are about to commit:
  **10 gates, all reported run, zero ignored, zero `SKIPPED`.** Bare `gates.sh` exits 2 at preflight
  on this machine. ~25 min wall clock.

**Done when** `gates.sh` reports all ten gates green on a tree where `retired_names.rs` holds six
tests, `lsp.rs` has no `$/cancelRequest` machinery, `crates/core/assets/schema/` and the two orphan
corpus projects are gone, and `grep` for the five dead symbols returns nothing — with the cgroup walk,
`ext.rs` and `corpus/callouts/` provably untouched by `git diff --stat`.