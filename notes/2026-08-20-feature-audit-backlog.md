# 2026-08-20 Final feature audit: implementation backlog

Source: the 2026-08-20 final feature audit (64-agent workflow: register pass, nine area
inventories, two opposing judges per area reconciled stricter-wins, an adversarial
defender for every CUT verdict, a completeness critic, and paired skeptics for every
proposed addition). Full ranked ledger (283 rows) with per-feature rationale:
https://claude.ai/code/artifact/032725c3-befe-4de4-8e00-8544a3eee109

Headline result: 282 features scored; 23 CORE, 193 EARNS, 51 WATCH (keep frozen),
14 CUT, 1 RULED (fails the bar but protected by a standing decline; author-only call).
Plus two repairs and one accepted addition.

## How to use this file (read first)

- **Line numbers are the audit's reads of the tree as of 2026-08-19/20.** Re-locate by
  symbol or string if they have drifted; do not trust a number blindly. The two headline
  claims (T1's missing `--strict` at tools/publish.sh:88 and R1's stale claim at
  CLAUDE.md:233) were re-verified directly against the working tree on 2026-08-20.
- **The ordering rule is absolute**: a feature's code, its tests/pins, its docs rows, and
  its corpus fixtures are deleted in the SAME commit, never across commits. A corpus
  document deleted ahead of its code leaves the code silently unguarded.
- **The parser-side pin rule**: withdrawing a construct means deleting the READ, not just
  the vocabulary entry. Where a task says "add a parser-side pin", that means a test
  asserting the read is gone (the key now draws the generic unknown-key diagnostic and
  changes nothing in the render).
- **No retirement registers, no compatibility notes, no "did you mean another tool's
  key"** (standing ruling 2026-08-17). Cuts are silent removals plus the generic paths.
- **Do not touch**: `MAX_WARM_PAGES` + the ExecPool LRU (the one standing freeze), the
  7-item Do-NOT-touch list in notes/native-rewrite.md, and anything in
  notes/DO-NOT-REBUILD.md.
- **Do not extend any WATCH feature.** The audit froze 51 features at their current
  shape; extending one is a new feature question that must clear the bar on its own.
- Environment for verification: `export TALIESIN_PYTHON="$PWD/.venv/bin/python"` before
  gates or any push (the pre-push hook runs the kernel suite and hangs without it, until
  T2 lands). Never run two workspace test suites concurrently (they deadlock). Run
  `cargo fmt --all` LAST after all .rs edits (the PostToolUse rustfmt hook and cargo fmt
  can disagree mid-stream).
- Per-commit verification floor: `cargo test -p taliesin-core` plus the tests named in
  the task. Editor tasks: the companion's `node --test` (gates.sh runs it as
  `editor/vscode` tests). Before any push: `./tools/gates.sh`, and take the gate count
  from its own verdict line.

## Recommended order

| Phase | Tasks | Risk |
|-------|-------|------|
| 0. Repairs | T1 (--strict), T2 (pre-push preflight) | low, do first |
| 1. Prose + dead-code sweep | R1..R9 | near-zero |
| 2. Vocabulary cuts (independent commits) | C1 lang:, C2 csl:, C3 page-layout, C4 link attrs | low |
| 3. Lint cuts | C5 video lint, C6 uncited lint | low/medium |
| 4. Editor cuts (one coordinated commit) | C7 (five features) | medium (manifest gates interlock) |
| 5. Spot-check, then maybe cut | C8 author metadata + appendix | needs verification first |
| Never (without the author's written override) | X1 shared bibliography | ruled |

---

## Phase 0: repairs

### T1. Add `--strict` to the deploy-path build in tools/publish.sh  [one flag]

**Why.** A crashed cell bakes its traceback into the page HTML, the build prints
"built with N problems", exits 0, and wrangler deploys it. Verified 2026-08-20:
tools/publish.sh:88 is `$TALIESIN build "$src" --out "$out"` with no `--strict`; the
pre-gate on line 87 is `--check-only --no-exec`, structurally blind to execution.
finalize_build (crates/server/src/build.rs, ~485-514) fails unconditionally only on
unparseable YAML, kernel failure, and page IO; a crashed cell is counted into `problems`
deliberately. The cut `publish` verb was strict by default and the cut playbook pinned
that property (notes/2026-08-08-cut-playbook.md:867, :892); the script replacement
silently lost it.

**Why it is safe.** `lint::blocking` excludes Suggestion severity, and plain
`--check-only` already fails all non-advice static findings, so `--strict` on the
executing build marginally gates only exec-produced problems. It cannot re-introduce
advice-blocks-deploy.

**Change.**
1. tools/publish.sh:88: append `--strict` to the deploy-path build invocation.
2. Optional but recommended: one drift pin in the gate_script.rs family
   (crates/core/tests/gate_script.rs; the needle pattern exists around :175) asserting
   publish.sh carries `--strict` on its deploy build. Per the gate-the-gate rule, a new
   drift gate must be mutation-checked: temporarily remove the flag and confirm the test
   fails, then restore.

**Caveat.** The first strict deploy may refuse if any site currently builds with a
standing non-advice warning. Run `./tools/publish.sh --check`, then a real
`taliesin build <site> --strict --out /tmp/x` per site to see what surfaces, before the
first deploy with the flag.

**Verify.** `./tools/publish.sh --check` green; if the pin was added, the mutation check
above.

### T2. Pre-push environment preflight (the one addition that passed the bar)

**Why.** Three dated incidents in one week (recorded in the author's memory notes): a
bare push whose kernel test hung >10 min until GitHub closed the SSH connection; a
concurrent-suite deadlock (40+ min silent); a poisoned stale TALIESIN_PYTHON pointing at
a deleted foreign venv. .githooks/pre-push runs `cargo test --workspace` with zero
environment preflight, while tools/gates.sh (~:130-157) already hard-exits 2 with a named
message on the identical missing prerequisite, and the tool's own resolution order
already prefers an ancestor .venv (crates/server/src/interpreter.rs ~:258, test ~:590;
serve_site/mod.rs ~:1059). The hook's suite is the last consumer left guessing.

**Scope guard.** Script-side bash only. This must NOT touch exec/kernel recovery
machinery: "hanging-interpreter recovery" is a do-not-rebuild entry needing its own
ruling (notes/DO-NOT-REBUILD.md ~:145-148). Route around it exactly.

**Change** (~10-20 lines in .githooks/pre-push, before the cargo test invocation, only in
the main-push branch that runs the suite):
1. Self-arm: if TALIESIN_PYTHON is unset and `$PWD/.venv/bin/python` is executable,
   export it (this is the tool's own committed default applied to its last consumer, not
   a new policy).
2. Poisoned-env guard: if TALIESIN_PYTHON is set but not executable, fail immediately,
   naming the path and the fix (unset it, or point it at ./.venv/bin/python).
3. Bounded probe: mirror gates.sh's ipykernel preflight (a short-timeout
   `"$TALIESIN_PYTHON" -c 'import ipykernel'`), fail fast with the named missing piece.
4. **Diagnose before adding more**: the skeptics flagged that the exact hang mechanism
   (ungated kernel-start integration tests vs the shared server test binary vs the cargo
   target/ lock) is unproven. Spend 30 minutes reproducing which one hangs before writing
   any lock-warning half, and skip the lock warning if cargo's own "Blocking waiting for
   file lock" line proves sufficient (it likely does).

**Gates.** crates/core/tests/gate_script.rs pins only the substring needles
`--check-only` and `publish.sh` in the hook, so this edit likely trips no test. Confirm
by running that test.

**Verify.** With .venv present and TALIESIN_PYTHON unset: a main push reaches the suite
with the var armed. With TALIESIN_PYTHON pointing at a nonexistent path: the hook fails
in seconds with the named message. `./tools/gates.sh` still green.

---

## Phase 1: stale prose and dead code (R1..R9, can be one commit)

These are rot the audit surfaced, not features. Near-zero risk, but verify each claim
in place before editing.

- **R1** CLAUDE.md:233: the parenthetical "(A live `preview` hot-swaps CSS, so this bites
  the build-and-inspect loop, not the dev loop.)" is stale: the `style` websocket message
  was removed 2026-08-17 (commit fe67b306; client.js's message switch has no style case;
  crates/server/src/protocol.rs ~:281 carries the cut note). Reword to the current truth:
  bundled CSS/JS changes need a `cargo build` in BOTH loops; after a binary rebuild the
  preview's boot-id mismatch forces a full reload, which fetches the fresh bundle.
  Verify the boot-id behavior in web-client/client.js (~:941-953) before wording it.
- **R2** crates/server/src/serve/mod.rs ~:787: test-module comment still names `style`
  among produced messages; crates/server/src/serve_site/mod.rs ~:243: comment still
  describes front-matter include-*/css merging. Fix both comments.
- **R3** crates/core/src/includes.rs ~:246-253: `resource_dependencies` still lists
  `css` / `include-*` keys whose page-injection reads were retired (dead watcher weight:
  the dev server watches files nothing parses). Delete those entries and their test
  coverage lines. KEEP `bibliography` (live). The `csl` entry goes with C2, not here, if
  you land C2 in the same session; otherwise note the dependency.
- **R4** corpus/README.md:66: "the three width escapes" is a stale count; there are two
  (DIV_CLASS_NAMES / DIV_FEATURE_CLASSES are the same 2). One-word fix.
- **R5** crates/core/src/vocab.rs ~:118: still carries a `code-line-numbers` description
  for a cell option CELL_OPTION_KEYS does not emit and nothing implements (cut wave 5).
  Delete the line; run vocab's descriptions_present test.
- **R6** ~/.local/bin/taliesin launcher, lines ~28-31: dead `__complete` fast path for a
  verb cut in wave 8. Four-line trim. NOTE: this file lives OUTSIDE the repo; no gate
  sees it (its own comment records a 5-day stale-binary incident for exactly that
  reason). Edit it directly on disk.
- **R7** Stale comments narrating cut consumers: crates/server/src/lsp_outline.rs ~:118
  (names the cut lsp_edits); editor/vscode/src/client.ts ~:38 (stale sectionEdit
  comment); crates/core/src/frontmatter.rs ~:365-366 (cites `codes::classify` and
  TAL-FM-UNSUPPORTED, both cut in wave 9); crates/core/src/render/extension/mod.rs ~:110
  (same stale codes reference). The "(a Quarto leftover?)" phrasing at frontmatter.rs
  ~:397 dies with C3 anyway.
- **R8** (optional, judgment call) crates/core/src/site/chrome.rs ~:592-620: four of the
  seven bundled social-icon glyphs (x/twitter, mastodon, bluesky, email) are used by no
  project. A few path strings of dead weight. If cut, check whether any validator or the
  crate schema enumerates icon names, and update in the same commit. Skipping this is
  fine; it is WATCH-adjacent, not a defect.
- **R9** (note, no change now) tools/subset-fonts.sh: the fontTools pin has never been
  re-verified against the on-disk woff2 bytes. Before the NEXT font bump, regenerate and
  diff; do nothing today.

---

## Phase 2: vocabulary cuts (one commit each)

### C1. Cut the `lang:` front-matter key

**Evidence.** Zero uses ever, including all git history (`git grep` across all revisions
found no non-en `lang:`); the only occurrence is the reference page's own example line
restating the default. page.rs defaults every page to `en` (const ~:224, fallback ~:719),
so removal changes no built byte. Bonus: cutting the key makes preview/build lang parity
STRUCTURAL (one const), deleting the FA16 drift axis entirely.

**Removal surface (one commit):**
- The read: crates/core/src/render/mod.rs ~:696 (`extract_field(fm, "lang")`).
- crates/core/src/render/model.rs ~:346-348: `RenderedDoc.lang` field.
- crates/core/src/render/page.rs ~:719 read (keep the `"en"` const at ~:224; `<html
  lang="en">` survives as the baseline).
- serve_site plumbing: `PageState.lang` at crates/server/src/serve_site/mod.rs
  ~:234-242, :702, :729, :841, :1369, and the FA16 test
  `a_page_previews_with_the_lang_it_builds_with` (~:2440-2495).
- KNOWN_KEYS entry (crates/core/src/frontmatter.rs ~:28), vocab.rs ~:49, the render test
  at crates/core/src/render/tests.rs ~:1396-1407.
- Docs: docs/guide/reference/frontmatter.tmd row ~:53 and the example line ~:309.
- Add the parser-side pin: a test asserting a document with `lang: fi` renders
  `<html lang="en">` and draws the generic unknown-key diagnostic.

**Verify.** `cargo test --workspace`;
`the_reference_page_documents_every_known_key` and vocab's `descriptions_present` pass
(both shrink by one row). `build --check-only` over docs/ still green.

### C2. Cut the `csl:` recognized-but-inert warning

**Evidence.** A compatibility note wearing a key's clothes ("a real thing you brought
from another tool"), foreclosed by the 2026-08-17 own-vocabulary ruling. Its original
keep-ground is asserted dead by its own test
(`csl_stays_recognized_because_dropping_it_would_mis_suggest_css`, frontmatter.rs ~:833:
`css` left KNOWN_KEYS 2026-08-02 and closest() no longer mis-suggests). The generic
located unknown-key lint takes over with no code added. The dev server currently watches
a .csl file whose content nothing parses (a residual read).

**Removal surface (one commit):**
- UNSUPPORTED_KEYS `["csl"]` (frontmatter.rs ~:67), validate_unsupported_keys
  (~:370-384), the vocab.rs exclusion (~:318-321), and the warning emission
  (~:362-364).
- Four dedicated tests: frontmatter.rs ~:778, ~:796, ~:810-825, ~:833, and
  crates/core/tests/nested_validation.rs ~:46-55.
- The watcher entry: includes.rs ~:248 `csl` in resource_dependencies + its test ~:675.
- Docs: docs/guide/reference/frontmatter.tmd ~:85 row and the prose at ~:283-286
  (reword: IEEE-only stays stated; the "brought from another tool" framing goes).
- corpus/diagnostics/typos.tmd: EDIT the csl lines out, do NOT delete the file (it also
  pins langg/cach/max-itemz/theorems and the div/callout/cell-option typo paths).
- The stale codes::classify comment (R7 covers it if not already done).

**Verify.** `cargo test --workspace`; a doc with `csl: x.csl` now draws the generic
"unknown front-matter key" with no wrong suggestion (assert in a test: the parser-side
pin).

### C3. Cut `page-layout: full`

**Evidence.** A rendered no-op: no CSS rule targets `.tali-wide` anywhere (the only
occurrence in assets is the comment at crates/core/assets/css/site.css ~:124-128 saying
the class "no longer changes the measure"; the width rules went with the card grid in
commit 6a30b565, 2026-08-15). page.rs ~:141 appends a class nothing styles. The three
pages that set it render pixel-identically without it, and the docs actively claim it
widens (false).

**Removal surface (one commit):**
- The read + threading: crates/core/src/render/page.rs ~:80-81, :140-141 (SiteCtx.wide);
  crates/core/src/site/frontmatter.rs ~:18, :63; crates/core/src/site/mod.rs ~:60-61,
  :555.
- KNOWN_KEYS (frontmatter.rs ~:33), the value validator + tests (~:352-399, :710-726,
  :894), vocab.rs ~:56.
- The pinning render test `the_site_shell_wraps_a_book_and_a_website_differently`
  (render/tests.rs ~:6839-6853): remove the `.tali-wide` assertion and the stale doc
  comment (~:6818-6819); KEEP the book-vs-website shell assertions if the test covers
  both (read it first).
- Front-matter lines: site/index.tmd:4, site/showcase.tmd:4,
  corpus/tech-blog/projects.tmd:7.
- Docs, all four rows: docs/guide/reference/frontmatter.tmd ~:60 and ~:313,
  docs/guide/reference/cheatsheet.tmd ~:23, docs/guide/using/recipes.tmd ~:92-99.
- Stale comments: page.rs ~:80, site/mod.rs ~:60, and the site.css ~:124-128 comment
  block.
- Parser-side pin: a test that a page with `page-layout: full` draws the unknown-key
  diagnostic and its shell carries no `.tali-wide`.

**Verify.** `cargo test --workspace`; build site/ and diff the three affected pages'
HTML against pre-change output (should differ only by the absent class attribute).
`./tools/publish.sh --check`.

### C4. Cut link attribute blocks (`[text](url){.class}`)

**Evidence.** Raw HTML passthrough is the exact substitute under the trust model
(`<a href="x" class="btn btn-outline-secondary btn-sm">` renders identically and picks up
the same bundled a.btn styles). The only users are five `.btn` links in corpus/tech-blog.
No `{#id}` link-id usage exists anywhere, so no anchor or xref depends on the id half.
render/tests.rs ~:3787-3792 is a test AGAINST the spelling (asserts site/index.tmd has
zero `{.btn`), not a feature test.

**Removal surface (one commit):**
- crates/core/src/render/mod.rs: apply_link_attrs (~:2386-2409), its call site (~:1174),
  and inject_attrs_into_last_tag (~:2450, no other caller). Do NOT touch
  parse_pandoc_attrs (owned by divs.rs; figures and divs keep it).
- Rewrite the five links as raw `<a>` tags in the same commit: four
  corpus/tech-blog/projects (index.tmd pages) plus
  corpus/tech-blog/_includes/publications.md.
- KEEP the a.btn CSS (crates/core/assets/css/base.css ~:119-130): hero_html emits
  `class="btn btn-primary btn-lg"` directly (site/mod.rs ~:1448-1450).
- Fix site/README.md:35, which still teaches the attribute spelling.
- KEEP the tests.rs ~:3788 zero-occurrences assertion (still passes, still meaningful).

**Verify.** `cargo test --workspace`; render corpus/tech-blog and confirm the five
buttons still render styled (the corpus sweep plus one eyeball).

---

## Phase 3: lint cuts

### C5. Cut the missing-local-video lint

**Evidence.** Zero `<video>` elements exist in any real document: the last site
screencast was replaced by a live cell in ad6e4de0 (2026-08-19); the only occurrence is
the lint's own fixture. The guarded failure cannot occur in the current corpus. media.rs
is also a hand-rolled `<`-scanning HTML walker, the code class behind FA11-FA13. Cheap to
re-add (~83 LOC) the week a screencast actually lands.

**Removal surface (one commit):**
- crates/core/src/diagnostics/media.rs (whole file) + its `pub use`
  (diagnostics/mod.rs ~:44) + its call site (crates/server/src/lint.rs ~:253).
- Three unit tests: crates/core/src/diagnostics/tests.rs ~:101-148.
- The video assertions inside the multi-rule server test: crates/server/src/lint.rs
  ~:1369 and ~:1425-1446 (trim, keep the rest of the test).
- corpus/diagnostics/links.tmd: EDIT out the video section (~:18-22) and its
  title/intro mentions (~:2, :6); the file's link rules stay.
- Docs: docs/guide/reference/cli.tmd row ~:67 whole, and the `<video>` half of row
  ~:145.
- Optional: corpus/render-fixes/clip.mp4 is now an orphan (its {{< video >}} section
  went 2026-08-08); delete if nothing references it (grep first).
- Do NOT touch the portable-folder copier: copy_local_assets harvests video src/poster
  independently (crates/server/src/build.rs ~:2507-2525 + its test) and stays.

**Verify.** `cargo test --workspace`; `build corpus/diagnostics/links.tmd --check-only`
still reports the remaining families.

### C6. Cut the uncited-entry lint (page scope AND site scope)

**Evidence.** An uncited .bib entry produces zero rendered-page defect, ever (References
lists only cited keys; .bib files are unpublished source). Zero recorded catches in its
lifetime versus two defects caused by its own machinery (the shadowed-warnings bug in
fa8f0743's own message; the false "looked uncited" on inherited bibliographies). It is
Suggestion severity at both report sites and pre-push deliberately excludes advice, so it
never gates anything.

**Important scoping.** This cut is INDEPENDENT of the shared-bibliography question (X1)
and must stay that way: cut only the lint, leave overlay's page-over-shared merge and the
duplicate-key half of validate_shared_bibliography untouched.

**Removal surface (one commit):**
- Report sites: crates/core/src/cite/render.rs ~:112-119 and
  crates/core/src/site/bibliography.rs ~:156-161.
- Machinery: uncited_local / uncited / uncited_message / UNCITED_KEYS_SHOWN in
  crates/core/src/cite/mod.rs ~:96-151, and the `Bibliography.local` HashSet (written
  ~:92, read ~:101, verified sole consumer) plus its one line in overlay.
- Tests: cite/tests.rs ~:820-905 (the uncited ones), the uncited assertions in
  site/bibliography.rs tests and crates/core/tests/shared_bibliography.rs (trim, do not
  delete unrelated assertions).
- Docs: the suggestion-severity example rows in docs/guide/reference/cli.tmd (~:89,
  :102, :121, :148): reword the severity table so it does not cite a lint that no longer
  exists.

**Follow-on scope (name it, decide separately).** This lint is the SOLE producer of
Severity::Suggestion in non-test code. After the cut, the advice tier and the
`--strict`-widens-to-advice distinction have no producer. The machinery is ~40 LOC and
harmless to keep dormant; deleting the tier is a separate scope question for the author.
Record whichever way it goes; do not fold it silently into this commit.

**Verify.** `cargo test --workspace`; `build docs/guide --check-only --strict` behaves
identically before/after (the lint never blocked anything).

---

## Phase 4: the editor cuts (C7, one coordinated commit)

Five companion features fail the bar. They interlock through the manifest gates
(editor/vscode/src/test/manifest.test.ts asserts cross-references between commands,
walkthrough steps, and contributions; three generic gates at ~:201-218, :220-240,
:490-523 fail on partial removal), so land them as ONE commit, or in an order that keeps
`node --test` green at every step.

**C7a. First-kernel-failure doctor hint.** Dead code since 2026-08-09: it listens on
onDidChangeDiagnostics for KERNEL_MESSAGES strings whose only producer is
build.rs::cell_error_message, and no live channel carries those into VS Code diagnostics
(the LSP is kernel-free; the problem matchers cannot parse the build's two-space-indented
log lines; the extension creates no DiagnosticCollection). Delete
editor/vscode/src/doctorhint.ts, editor/vscode/src/kernelfail.ts, its drift test
editor/vscode/src/test/kernelfail.test.ts (a standing tax on every rewording of a live
engine error, protecting a consumer that cannot fire), and the registration pair in
extension.ts (~:87).

**C7b. Build/check tasks + Problems-panel matchers.** The workflow is terminal-first
(launcher, gates.sh, publish.sh, pre-push), and the companion's kept terminal-link
feature (termlinks.ts, own drift gate, KEEP) already makes `--check-only` output
clickable, including for unopened files. The matchers on the build tasks are provably
inert against the log format (pinned by tasks.test.ts ~:218). Delete
editor/vscode/src/tasks.ts, taskspecs.ts, test/tasks.test.ts, the manifest
`taskDefinitions` + `problemMatchers` blocks, wiring in extension.ts (~:18, :78), and the
docs section docs/guide/using/preview.tmd ~:104-111.

**C7c. Diagnose Setup command.** editor/vscode/src/commands.ts (the command is the whole
33-line file) sendTexts `taliesin doctor` into a terminal; typing it is the identical
audit. Its only two consumers are C7a and C7d. Delete the file, the contributes.commands
row, and its wiring.

**C7d. Get Started walkthrough.** Onboards a hypothetical first user (its own test names
"the one user who has never seen the tool before" as beneficiary, which the bar
excludes); VS Code never resurfaces it. Delete the manifest walkthrough contribution
(~package.json:53, steps ~:60-65 including the taliesin.setup step whose command link
and onCommand completion event point at C7c), the three walkthrough markdown pages, and
the manifest.test.ts walkthrough block (~:486-523; it asserts walkthroughs.length > 0,
so it must go in the same commit).

**C7e. Bundled _site.yml JSON schema copy.** Only activates under the Red Hat YAML
extension, which is absent from the author's 43 installed extensions: it has never
validated anything on the only machine that matters. It is CLAUDE.md's named fourth
drift gate, the only one invisible to `cargo test --workspace`, and it went stale once
already (wave 4). Delete editor/vscode/schema/tali-site.schema.json, the
contributes.yamlValidation block (~package.json:90-95), and its node gates
(manifest.test.ts ~:419-482). The crate's own schema survives golden-locked
(crates/core/assets/schema/tali-site.schema.json, pinned by site_schema_matches_committed
in crates/core/src/schema.rs). Reword docs/guide/reference/frontmatter.tmd ~:456-459,
which claims the companion "wires it up for you". Do NOT add yaml-language-server
modelines to the four _site.yml files (equally inert without the YAML extension; the
tree deliberately carries zero modelines).

**KEEP (explicitly not cut):** taliesin.restartServer, taliesin.showServerLog, both
default keybindings (ctrl+shift+k, ctrl+alt+j), termlinks.ts, the preview webview,
embedded completion, the grammar, snippets (WATCH, frozen at nine).

**Also update:** the CLAUDE.md paragraph listing "FOUR drift gates" for a new
front-matter key (the schema-copy gate is gone: three remain); check
docs/guide/reference/cli.tmd for rows mentioning tasks or the walkthrough.

**Verify.** `cd editor/vscode && npm test` (or the exact command gates.sh runs) green at
the end state; `./tools/gates.sh` green; open VS Code once and confirm the companion
activates, preview opens, diagnostics squiggle.

---

## Phase 5: spot-check first, then decide

### C8. Structured author metadata (affiliation/url/equal/contribution) + the Author Contributions appendix

**Status: CUT verdicts from the audit's completeness pass, but these two were recovered
AFTER the adversarial-defense stage ran, so they were never defended. Spot-check before
acting; treat a surviving doubt as a keep.**

**Evidence so far.** The only document using the sub-keys is
corpus/structured-authors/paper.tmd, a self-described worked witness (circular pinning);
all four deploys and every real post use the scalar `author:` spelling; the author's
active paper is on IEEEtran outside Taliesin. The module comment in author.rs still names
JSON-LD (site/meta.rs) as a consumer: stale since wave 4 cut JSON-LD. The appendix's own
doc comment promises "Author Contributions, Acknowledgments, and the DOI, in that order"
but only contributions render (the rest went 2026-08-03).

**Spot-check (do these, in order):**
1. `git grep -n "affiliation:\|equal:\|contribution:"` across corpus/ docs/ site/
   gallery/: confirm the single witness.
2. Confirm the scalar and list-of-names `author:` spellings render bylines with NO code
   from the sub-key paths (read crates/core/src/author.rs and the byline emission at
   render/mod.rs ~:1482-1607).
3. Ask the author one question if in doubt: is a Taliesin-native paper with affiliations
   planned before EUSIPCO? If yes, keep both, mark WATCH-frozen instead.

**If confirmed, removal surface (one commit):** the sub-key halves of
crates/core/src/author.rs (AUTHOR_KEYS shrinks to the name form; ~285 lines including
~100 of tests), affiliations_html/byline sub-key emission + appendix emission
(render/mod.rs ~:1324-1336, ~:1482-1607 the sub-key parts, appendix_html ~:1545-1573),
docs/guide/reference/frontmatter.tmd ~:103-142 and ~:144-169, and the witness project
corpus/structured-authors/ plus its corpus/README.md row, all in the same commit.
Parser-side pin: a list-form author with `affiliation:` now draws the sub-key
unknown-key warning path or the generic diagnostic (pick whichever the surviving
validator naturally gives; assert it).

---

## X1. RULED: shared site-wide bibliography (DO NOT IMPLEMENT)

The audit scored it CUT twice independently (zero real adopters; the per-page
`bibliography:` line is the lived substitute the author's own docs use; ~323 LOC +
seams). But the defense found this exact cut is **W8, DECLINED 2026-08-10** after skeptic
review (notes/mvp-waves/W8-author-and-shared-bib-DECLINED.md: "do not take R7 before
publishing. Take it after real users exist, if they ever ask"), ratified in
notes/2026-08-10-mvp-publish-session.md, and the anti-rot register's charter forbids
re-filing closed rulings.

**Disposition: frozen. Only the author's explicit written override reopens it.** If that
override is ever issued: W8's plan is the playbook, amended for FD2 (retirement registers
no longer exist, so delete the read outright; `bibliography:` in _site.yml becomes an
ordinary unknown key), plus: re-point the three unrelated tests that use
corpus/shared-bib as a generic fixture (crates/core/tests/standalone_document_chrome.rs
~:26, :44; crates/core/tests/project_required.rs ~:107) BEFORE deleting the project.
Note C6 (the uncited lint) is not protected by W8 and lands either way.

---

## Standing outcomes that are guidance, not tasks

- **51 WATCH features are kept frozen.** Do not extend any of them; each names its freeze
  line in the ledger. Notable conditions: the blog-shaped features (drafts, Atom feeds,
  logo:, categories:) are kept on the imminent personal-blog deploy and should be
  re-audited if it slips indefinitely; the companion-dependent features (cursor sync,
  folding, snippets, path/math completion) are frozen until the companion passes the
  author's F5 acceptance; `build --strict` on a writing build is exercised by no
  committed pipeline today and becomes real via T1.
- **The 23 CORE features** are the identity: click-to-source, block-level incremental
  updates, warm no-per-edit-cost execution, and their carriers. Any change touching them
  needs the full invariant checklist in CLAUDE.md.
- **Refuted additions, do not build:** a shipped-page-weight census (exact goldens
  degenerate into a bless ritual at this repo's churn; the incidents it cites are
  resolved history) and sweep-names-deleted-URLs (no inbound readership exists pre-flip;
  adding it later costs the same). If either is ever revisited, start from the refutation
  rationales in the audit record, not from scratch.

## Suggested prompt for the implementing session

> Read notes/2026-08-20-feature-audit-backlog.md in full, including the "How to use this
> file" section. Implement Phase 0, then Phase 1, then Phases 2-3, as separate commits
> per task, verifying each as the task specifies. Stop before Phase 4 and report; Phase 4
> is one coordinated commit and Phase 5 requires a spot-check verdict first. Do not touch
> X1. Line numbers may have drifted: re-locate by symbol, and verify every claim in the
> file against the code before acting on it.
