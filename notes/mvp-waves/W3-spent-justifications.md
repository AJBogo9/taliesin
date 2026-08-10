# W3 — NOT TAKEN. See `notes/2026-08-10-mvp-publish-session.md` for the ship-path outcome

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
> - **STRIKE step 10 entirely (`third_party.rs`'s `OWN_JS`). W5 owns the whole `OWN_JS` + `THIRD_PARTY.md:50` + `CLAUDE.md:277` triple**, because W5 also adds the self-cleaning existence guard that stops the pattern recurring. **This removes two of your four sanctioned non-comment exceptions**, leaving `CLAUDE.md:417` and `corpus/diagnostics/widgets.tmd:8` — a strictly cleaner prose-only invariant. Update the diff-purity check's expected exception list.
> - **DROP step 7's `lsp.rs:3989` clause if W7 has landed** — W7 deletes `:3977-4137` wholesale, so you would be repairing a doc comment on a deleted test. If W7 is deferred past release, keep it.
> - **W7 first for `lsp.rs`.** W7 deletes `:1151-1155` and `:1172`, which shifts your `:1162` footgun comment. Re-locate it by grep, not by line number.
> - **Step 8 touches the one standing freeze file** (`serve_site/exec_pool.rs:10`). That is sanctioned **because it is comment-only, and it is the single exception in this plan**. Prove it: `git diff crates/server/src/serve_site/exec_pool.rs` must show one comment hunk and nothing near `MAX_WARM_PAGES` or the LRU order.
> - **Verification #3's grep is scoped wrong:** it expects `editor/vscode/**/kernelfail*` hits from a search over `crates/` only. Those hits can never appear. Widen the path or drop them from the expectation.
> - **Superseded-if-W8-is-taken (W8 is declined, so keep them):** step 5 (`crates/core/src/author.rs:4`) and step 7's `appendix_html` doc at `render/mod.rs:1513` are both deleted outright by W8. Harmless duplication if W8 is ever revived.

---

### R7 — The spent-justification sweep

**Branch:** `fix/r7-spent-justifications` · **Kind:** repair (prose) · **Size:** ~260 lines · **Blocked by:** none

> **PROSE ONLY. Zero behaviour change, and it must not be mixed into a behaviour diff.** That is the
> rule wave R4 exists to enforce, and this wave inherits it. One branch, one commit, and the diff
> must contain nothing but comment lines, doc comments, one three-string test const, one CLAUDE.md
> line and one corpus paragraph. A verification command below proves that mechanically.

**Why this ships before release.** These sentences are the direct mechanism of this project's
most-recurring failure. Wave 12 found eight spent justifications in one wave; wave R6 found a ninth
that was "two greps from being honoured again" (the code lens defended by a `runcell.ts` that had
already been deleted). A comment that names a module, a verb, a file or a capability that no longer
exists does not fail a gate: it is read by the next session as fact, and the next session acts on it.
Measured today, **34 distinct spent claims across 41 files**, of which 22 name an identifier that
resolves nowhere in the tree. One of them is in `CLAUDE.md` itself, the file every session reads
before anything else.

**Census method (re-runnable, state it in the commit message).** Three passes, all cheap:

1. **Unresolvable `a::b` paths in comments.** Extract every backticked `ident::ident` token from
   lines beginning `///`, `//!`, `//` or `*` across `crates/`, `web-client/`, `editor/vscode/src/`,
   then check the final segment has a `fn|struct|const|enum|mod|static|type|trait|macro_rules!`
   definition anywhere in the tree. 23 candidates today, 22 real after discarding `std` items.
2. **Unresolvable file paths in comments.** Same line filter, token
   `` `path.(rs|ts|js|tmd|yml|json|css|md|sh|toml|bib|html)` ``, basename checked against a walk of
   the repo. 86 candidates, most are illustrative example paths (`a.tmd`, `x.html`); ~12 are real.
3. **Dead vocabulary by register.** `RETIRED_COMMANDS` (`crates/server/src/main.rs:112-169`, 17
   verbs), `RETIRED_FLAGS` (`crates/server/src/serve/mod.rs:628-639`), `RETIRED_KEYS`,
   `RETIRED_DIV_CLASSES` (`crates/core/src/render/validate.rs:147+`), `RETIRED_XREF_PREFIXES`,
   `RETIRED_CELL_LANGS`, `RETIRED_NEW_KINDS` (`crates/server/src/cli.rs:175-192`). Grep each dead
   name and keep only the hits that read as **live**, not as history.

The keep/fix line, and it is the only judgment call in the wave: **a sentence that says a thing is
gone is correct and stays; a sentence that names a gone thing as present is the defect.** See the
disproven list for ten sentences that look dead and are actually fine.

**Verified state (checked 2026-08-10).** Every line number below was read today.

*Group A: the identifier resolves nowhere (22 claims).*

- **The `codes::` phantom, 13 sites, not 11.** No `codes` module exists (`crates/core/src/` listing;
  `codes.rs` died in `5d92a218`, wave 9). Cited at `crates/core/src/frontmatter.rs:78, :639, :643,
  :822, :988, :997, :1130`; `crates/core/src/render/validate.rs:155, :460`;
  `crates/core/src/render/extension/mod.rs:110`; `crates/core/src/render/tests.rs:453`; plus
  **`CLAUDE.md:417`** and **`corpus/diagnostics/widgets.tmd:8`**.
- **`TAL-*` codes cited as live, 9 sites.** Nothing in the tree emits one (`rg '"TAL-' crates/ -g
  '*.rs'` returns nothing). `crates/core/src/cite/render.rs:21, :50`;
  `crates/core/src/frontmatter.rs:514, :640, :823, :998`; `crates/core/src/vocab.rs:313`;
  `crates/core/tests/retired_names.rs:773`; `crates/server/src/lint.rs:781`. (`lint.rs:707, :892,
  :982, :1089` already say the catalogue went. Leave them.)
- **`render::deck::deck_overlay_html`**, two copies: `crates/core/src/site/config/mod.rs:49` and
  `crates/core/src/site/chrome.rs:605`. The deck engine went in wave 5 (`e5eabbf5`).
- **`Executor::run_through`**, three copies: `crates/core/src/render/model.rs:133`,
  `crates/core/src/render/divs.rs:507`, `crates/server/tests/nested_cell_executes.rs:6`. The method
  is **`Executor::run`**, `crates/server/src/exec.rs:580`.
- **`diagnostics::csl_recognized_but_unsupported`**, three copies:
  `crates/core/src/frontmatter.rs:59, :1174`, `crates/core/src/vocab.rs:350`. It moved into
  `frontmatter` (`crates/core/src/diagnostics/bibliography.rs:50` says so) and no function of that
  name exists.
- **`extension::dataset::DATASET_KEYS`** at `crates/core/src/render/mod.rs:60`, justifying the
  `pub(crate)` on `mod extension` (`:64`). `crates/core/src/render/extension/` contains **only
  `mod.rs`**; the `datasets:` shortcode went, and R4 already rewrote the front-matter note that
  pointed at it.
- **`frontmatter::NON_HTML_FORMATS`** at `crates/server/src/build.rs:92`. One occurrence
  workspace-wide, the comment itself. `format:` is gone; HTML is the only output.
- **`render::detect_execute_defaults`** at `crates/core/src/frontmatter.rs:345`. One occurrence.
- **`validate::validate_column_width`** at `crates/core/src/vocab.rs:177`. One occurrence, and the
  same sentence also names the retired `check` verb on `:178`.
- **`PageDoc::needs_kernel`** at `crates/server/src/serve_site/mod.rs:76`. One occurrence; it is the
  routing invariant's only named anchor, so a reader trying to verify the two-lane argument finds
  nothing.
- **`card::card_url`** at `crates/core/src/site/config/mod.rs:117`. Present tense ("took that over
  entirely"); the social-card rasterizer went in wave 4 (`d9fe8a6a`).
- **`super::skim`** at `crates/core/src/site/search.rs:158` (no `skim` module under `site/`), and
  **`skim.rs`** at `crates/core/src/render/mod.rs:1227`. Both inside the same claim as the next item.
- **"all four text projections"**, two copies: `crates/core/src/render/mod.rs:1226-1227` and
  `crates/core/src/render/tests.rs:2374`. Named: `taliesin read` (wave 2), `skim.rs` (wave 2), the
  search index, `llms-full.txt` (wave 4). **Three of the four are gone**, and R6-11 took
  `RenderedDoc::body_text` too. One projection survives: the Cmd-K search index.
- **`scope_note` in `check.rs`** at `crates/core/src/site/discovery.rs:39-40`. `scope_note` has one
  occurrence workspace-wide: that comment.
- **`crates/server/src/check.rs`** at `crates/core/src/site/config/mod.rs:174` ("Keep it stable (see
  …)"). The file is `crates/server/src/lint.rs`.
- **`crates/server/src/query.rs`** at `crates/core/src/site/mod.rs:154`, and **`query.rs`** at
  `crates/server/src/build.rs:297`. Deleted with `read` in wave 2.
- **`render/print.rs`** at `crates/core/src/render/page.rs:486`, given as the *reason* `resolve_title`
  is `pub(super)`. The print track went in wave 4.
- **`site/meta.rs` as a JSON-LD `Person`/`affiliation` consumer** at **`crates/core/src/author.rs:4`**
  (not `site/author.rs`, which does not exist). `rg -i author crates/core/src/site/meta.rs` returns
  nothing; `meta.rs:1-15` records that the JSON-LD `@graph` went on 2026-08-08.
- **`check::LSP_SOURCE`** at `editor/vscode/src/client.ts:30`. It is
  `crates/server/src/lint.rs:60`, `lint::LSP_SOURCE`.
- **"A Rust port of the companion's `outline.ts`"** at `crates/server/src/lsp_outline.rs:3`, present
  tense. `outline.ts` is gone. Its sibling `lsp_complete.rs:9-11` handles the identical fact
  correctly ("which is gone as of 2026-07-28") and is the model to copy.
- **"The `serve.rs` producers are covered by a sibling test"** at
  `crates/server/src/serve_site/mod.rs:1686`. Wave 1.1 deleted the single-document server; there is
  no sibling test, so this points a reader at coverage that does not exist.
- **`corpus/transclude.tmd`**, two copies: `crates/core/tests/corpus.rs:370` and `:563`. The file does
  not exist and there is no block-level transclusion in `crates/core/src/includes.rs`. At `:370` it is
  the stated ground for a **weakened** assertion, which is the worst shape of all.

*Group B: the sentence promises current behaviour that is false (12 claims).*

- `crates/core/src/render/mod.rs:1513` — `appendix_html`'s doc: "**Author Contributions**,
  **Acknowledgments**, and the DOI, in that order" and "Empty string when the page declares none of
  the three". The body (`:1527-1554`) emits **only Author Contributions**.
- `crates/server/src/interpreter.rs:264` — "A `python:` / `r:` field". `{r}` went in wave 6.
- `crates/core/src/frontmatter.rs:148` — a `RETIRED_KEYS` note ends "a `.code-walkthrough` marks lines
  from its own `.step lines=` and needs no cell option". Both classes were withdrawn in wave 7; an
  author following the note gets an unknown-div-class diagnostic. Same defect class R4 fixed for
  `datasets:`. **The register rule applies: one sentence, the date then the successor or an explicit
  "nothing".**
- `crates/server/src/lsp.rs:209-212` and `crates/server/src/lsp_project.rs:1-3` — both name "workspace
  symbols" and "the sidebar's two views" (outline + references) as live consumers.
  `workspaceSymbolProvider` and `referencesProvider` are both in the retired list pinned at
  `crates/server/src/lsp.rs:3513-3525`.
- `crates/server/src/lsp.rs:3989` — the doc on `a_cancelled_request_is_answered_rather_than_run` says
  "This is the Ctrl+T case: `workspace/symbol` is a whole-project walk". The test now sends
  `DocumentSymbolRequest` (`:4006`). (`lsp.rs:402` states the same history *correctly*; copy its
  wording.)
- `editor/vscode/src/client.ts:36-38` — `languageClient()` is "Exported for … the structural
  transforms [that] ask `taliesin/sectionEdit`". `sectionEdit` went in wave 10; its only caller today
  is `editor/vscode/src/map.ts:21`, which uses `taliesin/siteMap`.
- `crates/server/src/lsp.rs:1162` — the `layout-ncol` footgun is explained with "on a `.step` or
  `.panel-tabset`". Both withdrawn in wave 7 and both registered in `RETIRED_DIV_CLASSES`.
- `crates/core/src/math_vocab.rs:2, :15, :431` — "the symbol picker" (gone 2026-08-09 with
  `taliesin/mathCommands`, see `crates/server/src/lsp.rs:3622`) and "for `taliesin vocab`" (a verb in
  `RETIRED_COMMANDS`).
- `editor/vscode/src/extension.ts:29` — the companion's own two-halves comment still lists "code
  lenses" among what lives in `taliesin lsp`. `codeLensProvider` was cut in `32cd69ff` and is now in
  the never-advertise-again list.
- `crates/server/src/lsp_cells.rs:14` — "Whether a kernel actually runs this fence: `{python}`/`{r}`".
  `crates/server/src/exec.rs:159-164` returns `Some` for `"python"` only. (`:7` listing `r` as a
  language *spelling* is fine, and the tests at `:120-181` deliberately pin `{r}` as **not**
  executable. Touch neither.)
- **The two-kernel design, 7 sites.** `crates/server/src/kernel.rs:452-453` ("differ between Python
  (ipykernel) and R (IRkernel)"), `:461` ("IRkernel takes `--args <conn>`"), `:464` ("**nothing for R
  yet**", which promises a future the wave-6 ruling closed), `:732`, `:750`, `:1078`; plus
  `crates/server/src/serve_site/exec_pool.rs:10` ("Python/R kernel (~80-150 MB each)") and
  `crates/server/src/freeze.rs:109` ("a `{python}` chain never collides with an `{r}` one").
  `KernelSpec::python` is the only constructor.
- **`check` spelled as a command you can type, 11 sites.** `crates/server/src/lint.rs:719, :781,
  :1277, :1560`; `crates/core/src/cite/validate.rs:23`; `crates/core/src/site/xref.rs:96, :98`;
  `crates/core/src/site/discovery.rs:33-34`; `crates/core/src/site/bibliography.rs:62`;
  `crates/core/tests/shared_bibliography.rs:79`; `crates/server/tests/init_cli.rs:2, :7`. The verb
  is refused by the binary (`RETIRED_COMMANDS`, `crates/server/src/main.rs:152`).

**Files**

- Modify (comments and doc comments only): the 39 Rust and TypeScript files listed under Steps.
- Modify (one const): `crates/core/tests/third_party.rs:8-14`.
- Modify (one line): `CLAUDE.md:417`.
- Modify (one paragraph): `corpus/diagnostics/widgets.tmd:8`.
- Delete: nothing. Re-point: nothing.

**Steps**

- [ ] **1. `codes::` (13 sites).** In every one, the *rule* survives and only its *mechanism* is
      dead. **Do not invent a replacement symbol and do not delete the rule.** The rule is
      "no retirement note may be phrased as a did-you-mean"; the true reason is that a did-you-mean
      tells a reader to rename, and none of these retirements is a rename. Rewrite each sentence to
      say that, and drop the `codes::` clause. Sites: `crates/core/src/frontmatter.rs:78, :639, :643,
      :822, :988, :997, :1130`; `crates/core/src/render/validate.rs:155, :460`;
      `crates/core/src/render/extension/mod.rs:110`; `crates/core/src/render/tests.rs:453`;
      `CLAUDE.md:417`; `corpus/diagnostics/widgets.tmd:8`. **`CLAUDE.md:417` is the single
      highest-value line in this wave; do it first.**
- [ ] **2. `TAL-*` (9 sites).** Severity is a field on `render::Warning`. Replace each code name with
      the thing it stood for ("the broken-cross-reference error", "the unknown-key warning", …) or
      cut the clause. Sites listed above. Leave `crates/server/src/lint.rs:707, :892, :982, :1089`
      and both `editor/vscode/**/kernelfail*` files: they already state it as history.
- [ ] **3. Dead symbol names, Group A items 3 to 12.** One edit each:
      `crates/core/src/site/config/mod.rs:49` and `crates/core/src/site/chrome.rs:605`
      (`deck_overlay_html`, drop the parenthetical); `crates/core/src/render/model.rs:133`,
      `crates/core/src/render/divs.rs:507`, `crates/server/tests/nested_cell_executes.rs:6`
      (`run_through` → `run`, and confirm against `crates/server/src/exec.rs:580` before writing);
      `crates/core/src/frontmatter.rs:59, :1174` and `crates/core/src/vocab.rs:350`
      (`diagnostics::csl_recognized_but_unsupported` → name the `frontmatter` warning);
      `crates/core/src/render/mod.rs:60` (`DATASET_KEYS`: the module is `extension` alone now, so
      state the surviving reason for `pub(crate)` or delete the sentence);
      `crates/server/src/build.rs:92` (`NON_HTML_FORMATS`: HTML is the only output, so the analogy
      has no other half); `crates/core/src/frontmatter.rs:345`
      (`render::detect_execute_defaults`); `crates/core/src/vocab.rs:177-178`
      (`validate::validate_column_width` **and** the `check` in the same sentence);
      `crates/server/src/serve_site/mod.rs:76` (`PageDoc::needs_kernel`: name the field that actually
      carries the flag, or restate the invariant without a symbol);
      `crates/core/src/site/config/mod.rs:117` (`card::card_url`: the card generator went in wave 4,
      so say the key is inert and why it was dropped).
- [ ] **4. The text projections (2 sites).** `crates/core/src/render/mod.rs:1226-1227` and
      `crates/core/src/render/tests.rs:2374`. "All four" is now one: the Cmd-K search index. The
      no-double-render argument still holds for that one; keep the argument, fix the count, drop
      `taliesin read` / `skim.rs` / `llms-full.txt`. Also fixes the `super::skim` reference at
      `crates/core/src/site/search.rs:158` (that recipe-sharing sentence has no second party any
      more, so delete the clause).
- [ ] **5. Dead file paths (5 sites).** `crates/core/src/site/discovery.rs:39-40` (`scope_note` in
      `check.rs`); `crates/core/src/site/config/mod.rs:174` (`crates/server/src/check.rs` →
      `crates/server/src/lint.rs`); `crates/core/src/site/mod.rs:154` and
      `crates/server/src/build.rs:297` (`query.rs`); `crates/core/src/render/page.rs:486`
      (`render/print.rs`: state the surviving reason for `pub(super)`, or say the visibility is now
      wider than it needs to be, but **do not narrow it in this wave**);
      `crates/server/src/serve_site/mod.rs:1686` (`serve.rs`: the sibling test does not exist);
      `crates/server/src/lsp_outline.rs:3` (`outline.ts`: copy the wording of
      `crates/server/src/lsp_complete.rs:9-11`); `crates/core/src/author.rs:4` (`site/meta.rs` is not
      a consumer; the surviving consumers are the byline in `render/mod.rs` and
      `site/feed.rs`'s `<author>`, verified at `crates/core/src/site/feed.rs:158-201`).
- [ ] **6. `corpus/transclude.tmd` (2 sites).** `crates/core/tests/corpus.rs:370` and `:563`. At `:370`
      the missing document is the stated ground for a weakened per-file sourcepos assertion. **Do not
      re-tighten the assertion in this wave** (that is behaviour). Say the motivating document is
      gone and that re-tightening is an open question, so the next reader knows there is a decision
      here rather than a settled fact.
- [ ] **7. Group B behaviour promises, items 1 to 10.** `crates/core/src/render/mod.rs:1513`
      (appendix: one section, not three); `crates/server/src/interpreter.rs:264` (drop `r:`);
      `crates/core/src/frontmatter.rs:148` (one sentence, register style: date then successor or
      "nothing"); `crates/server/src/lsp.rs:209-212` and `crates/server/src/lsp_project.rs:1-3` (the
      surviving past-the-buffer consumers are cross-file definition and the `siteMap`/`cellRegions`
      extensions; check the live list against `crates/server/src/lsp.rs:3513-3525` before writing);
      `crates/server/src/lsp.rs:3989` (the test's subject is `documentSymbol`; keep the history in the
      shape `lsp.rs:402` uses); `editor/vscode/src/client.ts:36-38` (the surviving caller is
      `map.ts` over `taliesin/siteMap`) and `:30` (`lint::LSP_SOURCE`);
      `crates/server/src/lsp.rs:1162` (pick two classes that exist: the width escapes in
      `crates/core/src/render/validate.rs:52`, or a callout kind); `crates/core/src/math_vocab.rs:2,
      :15, :431` (the consumer is LSP completion inside `$…$`, nothing else; keep `category` described
      as grouping, since `crates/core/src/vocab.rs:370-381` still emits it);
      `editor/vscode/src/extension.ts:29` (drop "code lenses").
- [ ] **8. The two-kernel design (8 sites).** `crates/server/src/lsp_cells.rs:14`;
      `crates/server/src/kernel.rs:452-453, :461, :464, :732, :750, :1078`;
      `crates/server/src/freeze.rs:109`; `crates/server/src/serve_site/exec_pool.rs:10`. For
      `freeze.rs:109` and any other comment whose job is to justify a **map with one key**, re-point
      it at the wave-6 prohibition recorded in `CLAUDE.md` (`Executor::langs` and
      `FreezeCache::packages` must stay maps) instead of at `{r}`. `kernel.rs:464`'s "nothing for R
      yet" must lose the "yet".
      **`exec_pool.rs` is the one standing freeze: edit line 10's comment text and nothing else. Do
      not touch `MAX_WARM_PAGES` on `:14` or the LRU ordering anywhere in that file.**
- [ ] **9. `check` as a typeable command (11 sites).** Replace with `build … --check-only` in
      `crates/server/src/lint.rs:719, :781, :1277, :1560`; `crates/core/src/cite/validate.rs:23`;
      `crates/core/src/site/xref.rs:96, :98`; `crates/core/src/site/discovery.rs:33-34`;
      `crates/core/src/site/bibliography.rs:62`; `crates/core/tests/shared_bibliography.rs:79`;
      `crates/server/tests/init_cli.rs:2, :7`. **Scope limit, and hold it:** only the ~11 places
      where `check` is followed by a path or a `.` and therefore reads as an invocation. The other
      ~75 bare `` `check` `` mentions (the noun) stay; see Traps.
- [ ] **10. `crates/core/tests/third_party.rs:8-14` — the one non-comment edit.** `OWN_JS` still lists
      `walkthrough.js`, `tabset.js` and `scrolly.js`, deleted in wave 7. This is the second and third
      of the three copies wave R6-t2 recorded, of which R4 fixed one (CLAUDE.md's fenced map). Delete
      the three strings. It cannot change a test outcome: `OWN_JS` is an exclusion list consulted
      inside a `read_dir` over `crates/core/assets/js`, and `read_dir` cannot yield a file that does
      not exist. Say exactly that in the commit message so the next reader does not have to re-derive
      it.
- [ ] **11. Re-run the census.** All three passes from the method section, on the finished tree.
      Anything still reported must be either fixed or listed in the commit message as deliberate
      history. Record the final count.
- [ ] **12. Commit:** `docs: retire the justifications the cut already spent`

**Traps**

- **The R4 overlap is real and cuts both ways.** R4 (`169b1ca7`) claimed a retired-verb sweep over
  eight named files and its commit message says "Twelve retired-verb corrections in all". Two of the
  eight it named were **not** fixed (`site/bibliography.rs:62`, `lsp_cells.rs:14`) and six were.
  Before editing anything R4 touched, read the line. Ten sentences that look dead are already correct
  history: they are enumerated in this section's disproven list, and re-writing them would destroy a
  true record for no gain.
- **`exec_pool.rs` is the one standing freeze.** Step 8 edits a comment inside it. Nothing else in
  that file may move.
- **`stdout` is the LSP's JSON-RPC wire.** Steps 7 and 8 touch `lsp.rs`, `lsp_project.rs`,
  `lsp_cells.rs` and `lsp_outline.rs`. Comments only, so no `println!` can appear, but the diff check
  below is what proves it.
- **The register rule bites in step 7.** `crates/core/src/frontmatter.rs:148` is a `RETIRED_KEYS`
  entry, not a free comment. Its replacement must be **one sentence, the date then the successor or
  an explicit "nothing"**, and **must not be phrased as a did-you-mean**. Adding a tombstone test for
  it is forbidden: the register is derived.
- **The ordering rule does not fire, and confirm that.** This wave deletes no feature, so no corpus
  pin or docs page moves with it. `corpus/diagnostics/widgets.tmd` is edited in prose only; the
  document keeps rendering and keeps guarding the widget validators.
- **Do not rewrite `notes/`.** Out of scope by ruling, twice. The 64 dated audits already carry a
  banner telling readers to check the tree first.
- **Adjacent findings this wave must NOT fix, recorded so they are not lost.** Each is a code or test
  change and belongs in a behaviour wave (R6-12 is the natural home for the first two):
  - `crates/server/src/lsp.rs:1153` and `:1172` read `vocab["theoremKinds"]`. **That key does not
    exist**: `crates/core/src/vocab.rs:364-381` emits no `theoremKinds`, so `from_named` gets `Null`
    and the two branches are dead. Theorem environments went in wave 7.
  - `crates/core/src/render/tests.rs:2951-2958` asserts `!page.contains("Tabbed panels: the
    interaction")` and `!page.contains("Scrollytelling: scroll-driven")`. Both needles occur **only
    in that test file**, so both assertions are vacuously true forever.
  - `crates/core/tests/head_meta.rs:39-51`, `deck_head_carries_generator_meta_and_banner`, renders
    `format: deck`. `format:` is retired; the document renders as an ordinary page and the test name
    now lies about its subject.
  - `crates/core/tests/tarn.rs:3` says the fixture locks "`.panel-tabset`s that lower to ARIA tabs".
    `.panel-tabset` went in wave 7; the surviving assertion at `:38` indexes ordinary content.
  - `crates/core/src/render/repro.rs:43` says "The language as authored (`python`, `r`, `js`, …)".
    Defensible as literal (an author *can* still type `r`), so it is left out of step 8 deliberately.
    Decide it with `repro.rs`, not here.
- **The residual `check`-as-noun mentions are deliberately out of scope.** `rg -o '\`check\`' crates/
  -g '*.rs' | wc -l` reports ~75 after step 9. They read as the *concept* ("the check validators",
  "`check` stays offline"), which survives as `build --check-only`, and R4 already took the twelve
  that named it as a live verb. Sweeping them is a bigger, lower-value wave, and half of them are
  correct history. Say so in the commit message rather than leaving the count unexplained.

**Verification**

1. **The diff is prose.** On the staged tree, this must print only the four sanctioned exceptions
   (three `OWN_JS` strings in `crates/core/tests/third_party.rs`, one `CLAUDE.md` line, and the
   `corpus/diagnostics/widgets.tmd` paragraph):
   ```sh
   git diff --cached -U0 -- '*.rs' '*.ts' \
     | grep -E '^[+-]' | grep -vE '^(\+\+\+|---)' \
     | grep -vE '^[+-][[:space:]]*(///|//!|//|\*|/\*|\*/)'
   ```
   Any Rust or TypeScript line that is not a comment means the wave leaked into behaviour: undo it.
2. **Nothing new is dead.** Re-run census passes 1 and 2 and confirm the residual list contains only
   the ten history sentences named in the disproven list plus illustrative example paths.
3. **The registers still answer.** `rg -n 'codes::' crates/ CLAUDE.md corpus/` returns nothing.
   `rg -n 'TAL-[A-Z]' crates/` returns only the four `lint.rs` history lines and the two
   `editor/vscode/**/kernelfail*` history lines.
4. **The gate.** `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh` on the tree you are about
   to commit. It must print **10 gates** and 0 failed, 0 ignored. A prose wave cannot make it red, so
   a red gate means step 10 or a stray edit changed code: bisect the diff, do not "fix" the test.
5. **The binary still says what the comments now say.** `taliesin help` and `taliesin help doctor`:
   no `r:`, no `IRkernel`, no `check`, no `run`. (R4 verified this; re-confirm it costs seconds and
   this wave rewrote the sentences that describe it.)

**Done when** every one of the 34 verified claims above is corrected or deleted, the diff contains no
non-comment Rust or TypeScript line outside the four sanctioned exceptions, the re-run census reports
only the ten known history sentences, and `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh`
prints 10 gates with 0 failed and 0 ignored.
