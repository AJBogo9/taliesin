# Developer-tooling frontier audit — 2026-08-07

> **Trigger (author):** "the developer tooling feels close to mature — get it to the
> absolute front of developer tooling." Establish the frontier from primary sources,
> establish what is actually built, then find the gap.
>
> **Scope:** the *editor and machine* surfaces — `taliesin lsp` (14 advertised
> capabilities), the VS Code companion, `taliesin mcp`, the CLI's diagnostic surface,
> and the contributor inner loop. Not the browser preview (six prior rounds own it) and
> not the `.tmd` dialect.
>
> **This is the round [2026-08-01-feature-value-audit](2026-08-01-feature-value-audit.md)
> named as its biggest blind spot** ("the ~11,300-LOC `lsp*.rs` surface, which history
> structurally cannot see and which deserves its own round") and that
> [2026-08-04-fv5](2026-08-04-fv5-instrument-and-portfolio-recheck.md) re-priced at
> **15,592 LOC including tests, largest feature by 2.8x, still the only unmeasured one.**
>
> **Method.** Every capability claim below was measured by driving the **built release
> binary** over real stdio with a Python LSP client, not read out of the source. Two
> probe scripts, both carrying a known-positive control row per this file's own
> broken-probe rule. Frontier established from the LSP 3.17 metaModel, the MCP
> 2025-11-25 server spec, and the Zed language-extension docs (context7); DX ranking
> grounded in the DevEx framework and the CHI 2020 notebook pain-point taxonomy.

> **STATUS 2026-08-07, same day: tiers 0-2 are shipped.** Findings 1, 2 and 10 landed first
> (site-aware buffer lint, `didChangeWatchedFiles`, the trace armed). Everything else in the
> table below landed in one batch except finding 5's *highlighting* half, which stays ruled out
> (semantic tokens do not reach Helix and ship `off` in Zed; a tree-sitter grammar is a third
> definition of the dialect) — its docs half is backlog item 221 and is still owed. Finding 9 was
> deliberately no-action and remains so, though the new work took its advice: four new modules
> (`lsp_refs`, `lsp_select`, `lsp_lens`, `lsp_diag`) rather than more of `lsp.rs`.
>
> **Do not re-derive this round's findings as new work.** What is left of it is item 221 and
> nothing else.

## Verdict

**The tooling is not immature; it is asymmetric.** Taliesin ships fourteen LSP
capabilities, a stdio MCP server, a drift-gated agent vocabulary, 47 catalogued
diagnostic codes with `--explain` and structured `suggestion.replacement`, shell
completion, `doctor`, and a companion that deliberately implements zero language
features of its own. Almost nothing on that list is table stakes; most of it is ahead
of the batch document compilers it competes with, and the `lmtools.ts` decision (delete
five hand-maintained LM tools, keep the MCP provider) is the correct instinct applied
early.

The gap is **not breadth of capability. It is that the editor — the surface the
project's own load-bearing invariant declares to be the only place authoring happens —
is the *weakest* of the three consumers of the same validator engine.** The browser
preview, which the invariant declares read-only, gets a strictly better diagnostic than
the editor does. That inversion is the through-line of finding 1, 2 and 3, and all
three fixes call code that already exists.

Second through-line: **five of the six LSP methods that best fit a cross-referenced
document format are unimplemented** — `references`, `codeLens`, `semanticTokens`, and
both halves of the 3.17 pull-diagnostic model — while the data each would need is
already computed and already exposed on a *proprietary* `taliesin/` method that only
the VS Code companion can call. The surface is 15.6k LOC wide and one editor deep.

---

## The measurement

Release binary `0.2.0 (45fab028)`, driven over stdio. Probe scripts under
`scratchpad/lsp_probe.py` + `lsp_crossfile.py` (reproduced in "Reproducing" below).

### What `taliesin lsp` answers

| advertised (14) | measured |
|---|---|
| `hover` · `completion` · `documentSymbol` · `foldingRange` · `inlayHint` · `documentLink` · `documentHighlight` · `formatting` · `workspaceSymbol` · `definition` · `codeAction` · `rename` (+prepare) | all answer |

| probed, **not** implemented | JSON-RPC |
|---|---|
| `textDocument/references` | `-32601` |
| `textDocument/diagnostic` (3.17 pull) | `-32601` |
| `workspace/diagnostic` (3.17 pull) | `-32601` |
| `textDocument/semanticTokens/full` | `-32601` |
| `textDocument/codeLens` | `-32601` |
| `textDocument/selectionRange` | `-32601` |
| `signatureHelp` · `typeDefinition` · `declaration` · `linkedEditingRange` · `prepareCallHierarchy` · `inlineCompletion` · `onTypeFormatting` · `documentColor` | `-32601` |

`$/cancelRequest`, `$/progress`, `workspace/didChangeWatchedFiles` and
`textDocument/didSave`: **zero occurrences in `lsp.rs`.** The server's `workspace`
capability block serializes to `null`.

### Latency, measured

| path | measured |
|---|---|
| `didChange` → `publishDiagnostics`, 486-line page in a 22-page project | median **144 ms** (n=6), of which 120 ms is the deliberate debounce → **~24 ms of work** |
| `workspace/symbol` (Ctrl-T) | **167 ms** |
| `hover` / `inlayHint` | 19 / 24 ms |
| `documentSymbol` / `foldingRange` / `formatting` | < 1 ms |
| binary startup (`--help`) | < 10 ms |
| `check docs/guide` (22 pages, cold) | 0.36 s |

Everything except `workspace/symbol` is comfortably inside the perceptual budget.
`workspace/symbol` is the outlier and it is the *one* request a user types into
character-by-character: each keystroke queues another whole-project walk with a `stat`
per page, on a single-threaded loop, **with no cancellation**, so the superseded walks
all run to completion. This is a latent scaling cliff, not a present defect (measured
on the largest project in the tree).

---

## Finding 1 — the editor gets a strictly weaker diagnostic than the read-only preview

**Severity: high. Effort: small. The fix is one scope argument and one call.**

Measured on a two-page site (`a.tmd` links to `b.html#target`; `b.tmd`'s heading is
renamed). Positive control carried: five of six diagnostics match exactly.

| diagnostic | `taliesin check <dir>` | `taliesin lsp` on the open buffer |
|---|---|---|
| `TAL-FM-KEY` unknown front-matter key | ✓ | ✓ |
| `TAL-CALLOUT-KIND` | ✓ | ✓ |
| `TAL-XREF-UNDEF` | ✓ | ✓ |
| `TAL-ASSET` | ✓ | ✓ |
| `TAL-LINK` | ✓ *"resolves to `nope.html`, which is no page in this site"* | ✓ *"no such file under the document directory"* — **the standalone phrasing** |
| **`TAL-LINK-ANCHOR`** broken cross-page anchor | ✓ | **absent** |

**Root cause, and it is one word.** `check.rs:312` —
`page_static_diagnostics(src, &doc.blocks, base, doc.format, Scope::Standalone)` — on
the `buffer_diagnostics` path the LSP calls. Every editor buffer is linted as a
standalone document even when it sits inside a site, so `Scope::InSite`'s site-aware
counterpart never runs and `validate_cross_page_links_for` is never called.

**The preview already does it correctly**: `serve_site/mod.rs:1805` passes
`Scope::InSite`, and `:1852` calls `preview_diag::cross_page_diagnostics(&site, rel)`.
**And the expensive part is already solved.** `preview_diag.rs:37-45` documents that
this was deliberately re-scoped away from a whole-site render: *"It renders the page
plus the pages it links to instead, which is the same answer for a fraction of the
work — and, unlike the old version, work that does not grow with the size of the
book."* The incremental cross-page validator exists, is proven cheap, and the editor
does not call it.

**The docs claim otherwise.** `docs/guide/reference/cli.tmd:625-627`: *"any LSP editor
… gets live `.tmd` diagnostics as you type, **the same validators as `check`**, run on
the unsaved buffer."* Measured false for any document inside a site — which is every
book chapter and every blog post in the corpus.

**Why this matters more than its size.** The 2026-07-18 DX audit's #1 finding, the one
that "dominates every persona and both codebase audits", was this exact shape: the
validation cliff between the fast loop and `check`. It was fixed for the preview (DX1,
`preview_diag.rs`) and left open for the editor. The project's own invariant says *the
`.tmd` file is the single editing surface; the browser is a read-only view.* The
validation cliff was closed on the read-only view and left standing on the editing
surface.

## Finding 2 — the companion pays for a file-watch signal the server throws away

**Severity: high. Effort: small.**

`editor/vscode/src/client.ts:65` wires
`synchronize.fileEvents: createFileSystemWatcher("**/{*.tmd,_site.yml,*.bib}")`, so
`vscode-languageclient` sends `workspace/didChangeWatchedFiles` on every external
change to a page, the config, or a bibliography. **The server does not handle that
method** (zero occurrences) and does not advertise the capability.

Measured: rename a heading in `b.tmd` on disk → send `didChangeWatchedFiles` → the
server publishes **nothing**. Editing `b.tmd` *in the editor* also publishes nothing
for `a.tmd`; `publish()` (`lsp.rs:2093`) is per-URI and is only ever called for the
document that changed.

Consequences in ordinary use, none of them exotic:

- `git checkout` / `git pull` / an agent editing a sibling page → every open buffer
  keeps diagnostics computed against a tree that no longer exists.
- Edit `refs.bib` → `[@key]` diagnostics in the open chapter never refresh.
- Edit `_site.yml` → nothing re-lints.

There is at most a handful of open `.tmd` buffers at any time, so the fix is: handle
the notification, re-publish every open doc. It composes with finding 1 — once the
buffer lint is site-aware, cross-file invalidation is what keeps it *true*.

## Finding 3 — diagnostics are push-only, so the Problems panel can only ever show open files

**Severity: medium-high. Effort: medium.**

`textDocument/diagnostic` and `workspace/diagnostic` both `-32601`. The push model
publishes only for documents the editor has opened, so a 25-chapter book shows problems
for the two chapters you have open and silence for the other 23 — while `check <dir>`
on the same tree answers the whole question in 0.36 s.

`workspace/diagnostic` (LSP 3.17) is the primitive built for exactly this: the client
pulls project-wide results, and the server can invalidate with
`workspace/diagnostic/refresh`. The engine already computes this answer; only the
transport is missing. This is also the honest fix for finding 2's invalidation
problem — pull inverts ownership so the client asks rather than the server guessing.

## Finding 4 — the five LSP methods that best fit *this* format are the missing ones

**Severity: medium-high. Effort: medium (each is small; the set is medium).**

Taliesin's dialect is unusually reference-dense: `@fig-`/`@sec-`/`@thm-` cross-refs,
`[@key]` citations, `{{< include >}}`/`{{< embed >}}` transclusion, `//| name` reactive
edges. That shape maps onto specific LSP methods, and those are precisely the ones
absent.

- **`textDocument/references`.** "Where is this label used?" is *the* question in a
  cross-referenced book. `documentHighlight` answers it within one file only.
  `lsp_nav.rs` already resolves definitions across the project and the server already
  exposes a proprietary **`taliesin/projectRefs`** — so the answer is computed, and
  published on a method only the VS Code companion can call. Moving it onto the
  standard method is close to free and multiplies the audience.
- **`textDocument/codeLens`.** The highest-leverage single missing capability. A code
  lens above each `{python}`/`{r}`/`{js}` fence — *Run cell · Run above · ⚡ cached* —
  puts the execution loop in the editor, in **every** LSP client, from Rust, with zero
  TypeScript. Today `runcell.ts` + `taliesin/cellRegions` deliver this to VS Code
  alone. It is also the natural home for the 2026-07-18 audit's item #9 ("make caching
  legible"), which is still open and whose data `freeze.rs` already has.
- **`textDocument/semanticTokens`.** See finding 5.
- **`textDocument/selectionRange`.** Expand-selection by document structure (word →
  sentence → paragraph → callout → section). `lsp_outline.rs` already computes the
  section extents this needs; the editor default (expand-by-brackets) is meaningless in
  prose.
- **`$/progress` + `$/cancelRequest`.** No long operation is visible in the editor, and
  no superseded request can be abandoned (see the `workspace/symbol` measurement).

Correctly absent, and worth recording so they are not re-proposed: `signatureHelp`,
`callHierarchy`, `typeHierarchy`, `declaration`, `typeDefinition`, `documentColor`,
`linkedEditingRange`, `inlineCompletion`. None has a referent in a prose format.

**Doc inaccuracy found while checking:** `CLAUDE.md` lists "selection ranges" among the
LSP's implemented features. There is no `selectionRangeProvider`; the only
`selection_range` in the tree is the `DocumentSymbol.selection_range` *field*
(`lsp.rs:2069`).

## Finding 5 — "any LSP editor" ships unhighlighted grey text everywhere but VS Code

**Severity: medium. Effort: medium-large. Requires an author ruling, not just work.**

`cli.tmd:27` and `:625` advertise **Neovim, Helix, Zed, VS Code**; `lsp.rs:2086`'s
comment names "the Neovim / Helix / Zed setups the CLI reference documents". Measured:

- **No tree-sitter grammar exists anywhere in the tree** (zero hits for
  `tree-sitter`/`treesitter` across every `.rs`/`.toml`/`.json`/`.md`).
- **No `textDocument/semanticTokens`** (`-32601`).
- The only grammar is `editor/vscode/syntaxes/tmd.tmLanguage.json` — 16 scopes, VS Code
  only.
- The documented setup is **one Neovim snippet**. No Helix `languages.toml`, no Zed
  `extension.toml`.

So a Helix or Zed user gets diagnostics, hover and completion over **unhighlighted grey
text** — no fence colouring, no `:::` structure, no `#|` cell options, no front matter.
That is a worse first impression than the tool deserves and it is currently oversold.

**The tradeoff is real and I verified it rather than assuming it** (this is where the
obvious answer is wrong):

| route | Neovim | Helix | Zed | cost |
|---|---|---|---|---|
| LSP semantic tokens | ✅ built-in since 0.9 | ❌ **not supported** — tree-sitter is the only highlighter | ⚠️ supported but `semantic_tokens` **defaults to `"off"`** | one Rust producer, reuses the existing parser + `highlight.rs`, **no second dialect definition** |
| tree-sitter grammar | ✅ | ✅ | ✅ (a Zed language extension registers a grammar) | a **third** definition of the dialect after the Rust parser and the TextMate grammar |

Semantic tokens are the architecturally correct move for this project — one source of
truth in Rust is the exact principle
[2026-07-28-vscode-companion-audit](2026-07-28-vscode-companion-audit.md) was written to
enforce — and they do not reach Helix or default-Zed. A grammar reaches everyone and
violates the principle.

**Three honest options, author's call:**

1. **Narrow the claim.** Change `cli.tmd` to say Neovim/Helix/Zed get *diagnostics and
   intelligence, not highlighting*, and ship the Helix + Zed config snippets. Smallest,
   honest, zero new surface — and it is owed regardless of which route is chosen.
2. **Semantic tokens** (recommended increment). Lights up Neovim and opt-in Zed from the
   parser that already exists, and lets the TextMate grammar stop growing. Also the only
   route that can ever highlight `{python}` *inside* a fence consistently with how
   Taliesin actually renders it.
3. **Tree-sitter grammar.** Universal, and a genuine third source of truth. Only worth
   it if a non-VS-Code editor becomes a real target rather than a documented one.

Option 1 is owed now. Option 2 is the frontier move. Option 3 needs demand.

## Finding 6 — the MCP server is tools-only

**Severity: medium. Effort: small-medium.**

`mcp.rs:143` declares `"capabilities": { "tools": {} }`. Six tools
(`check`/`read`/`symbols`/`map`/`vocab`/`build`). The MCP 2025-11-25 server spec also
defines **`resources`** (+ `resources/templates`), **`prompts`**, **`completions`** and
**`logging`**, and Taliesin already holds resource-shaped data it is currently forcing
through a tool call:

- the two JSON Schemas (`taliesin schema`) → **resources**, the canonical MCP shape for
  "here is a document you may read".
- the 47-code diagnostic catalogue with cause + fix (`check --explain`) → a **resource
  template** (`taliesin://diagnostic/{code}`), so an agent resolves a code without
  spawning a process per lookup.
- `AGENTS.md` + `vocab` → a resource, not a tool call.
- **`prompts`** is the un-taken one: "start a new post in this project's idiom",
  "convert this Quarto document" — the project's own scaffolds already encode these
  answers (`new post|page|deck|paper`), and a prompt is how a host offers them without
  the agent reverse-engineering the dialect from docs.

This is additive, rides data that already exists, and is the difference between "an MCP
server exists" and "an agent can work in this project without being told how".

## Finding 7 — long-running cells report nothing outside the browser

**Severity: medium. Effort: medium.** *Grounded in research, not in a persona.*

The CHI 2020 notebook study (n=20 interviews, n=156 survey) identifies four
**high-impact** activities — median importance ≥4 *and* median difficulty ≥4 — and
"long-running tasks" is one of them (76% rate it important, 55% difficult). The
specific complaint: *"Running long-running computations in notebooks provides no
feedback on progress; while the computation is running, data scientists lose all
interactivity"*, and the stated want is *"when the process is done, it automatically
creates a notification."*

Taliesin has the machinery — `exec.rs:113` `ProgressSink`, `set_progress`, `build-state`
messages — **wired to the browser only**. `taliesin run` in the terminal and every
editor path get nothing. The silence-budget design (`TALIESIN_CELL_SILENCE`, reset on
every printed line) is genuinely better thinking than a wall-clock timeout, and it is
invisible to a user who is not looking at the browser.

Two small increments: stream the existing `ProgressSink` to the terminal for
`taliesin run`, and to `$/progress` for the LSP (finding 4). A completion notification
is the third and is cheapest via the companion.

## Finding 8 — the reproducibility hole is documented honestly and still open

**Severity: medium. Effort: medium. Not a defect — a named limit worth closing.**

`freeze.rs:19-29` states it plainly: the cumulative key *"folds in code and interpreter
identity only"*, so *"upgrading a library in place (`pip install --upgrade …`) is the
same interpreter reporting the same `--version`, so every key is unchanged."*

That is exactly CHI 2020's **"Reproduce and Reuse"** pain point (*"the only way another
person can run the notebook is if they're able to match all the environment settings"*).
`doctor` audits interpreter **presence**, not package **versions** — it reports
"ipykernel MISSING" but never "which pandas".

Cheapest honest close: have `doctor --format json` emit a package manifest, and let
`_freeze/` record the manifest digest it was produced under, so a restore that crosses
an environment change *says so* rather than silently serving output from a library
version that is gone. Folding the digest into the key itself is the stronger version and
the more disruptive one (every `pip install` busts the whole cache), so it belongs behind
a flag if at all.

## Finding 9 — contributor loop: 19 s per edit, and the two biggest files are the two least splittable

**Severity: low-medium. Effort: medium.**

| measured | |
|---|---|
| warm rebuild after touching `lsp.rs` alone | **19.1 s** |
| Rust `#[test]` functions | **2,222** |
| integration test files | **116** |
| `crates/server/src/lsp.rs` | **5,019 lines** (was 2,363 at the 2026-07-27 L3 round — **2.1x in six weeks**) |
| `crates/server/src/build.rs` | 4,008 lines |

19 s is inside the DevEx framework's tolerable band but outside the ~10 s where an edit
still feels connected to its result, and it is paid on every LSP change. `lsp.rs`
doubling in six weeks is the number worth watching: the L3 round's own lesson was *"an
audit lens that names a line count is quoting the day it was written"*, and this one has
outrun its own footnote. The file is not one concern — dispatch, capabilities, symbols,
publish, and ~2,900 lines of tests share it.

**No action proposed here**, deliberately: a split is a refactor with no user-visible
win, and this project's rule is that those wait for a change that needs them. Recording
the trajectory so the next LSP feature can carry the split rather than pay for it.

## Finding 10 — the FV-5 instrument shipped three days ago and has never been armed

**Severity: low. Effort: trivial. Highest evidence-per-minute item in this file.**

`lsp_trace.rs` was built on 2026-08-04 precisely to answer "which of the fourteen
capabilities actually fire during real writing", after three portfolio rounds priced
them at zero evidence. **No trace file exists anywhere on this machine** — it has never
been armed, so the largest feature in the tool is still unmeasured for adoption.

Set `TALIESIN_LSP_TRACE` in the companion's server-spawn environment (or the author's
own settings) and the week of data starts accruing at zero cost. Every prioritisation in
this file that touches *which* capability to build next would be better decided with
that tally than without it — including finding 4's ranking.

---

## Prioritised

Legend: **[wire]** = call code that already exists · **[new]** = net-new engine work.

### Tier 0 — do first; each is small and each closes a false claim

| # | Item | Effort | Type |
|---|---|---|---|
| 1 | **Site-aware diagnostics on the LSP buffer path** — `Scope::InSite` + `cross_page_diagnostics` in `buffer_diagnostics`. Closes the `cli.tmd:625` claim. | S | [wire] |
| 2 | **Handle `workspace/didChangeWatchedFiles`** — re-publish every open doc. The client already sends it. | S | [wire] |
| 10 | **Arm `TALIESIN_LSP_TRACE`.** One env var; makes every later decision evidence-backed. | XS | [wire] |
| 5.1 | **Narrow the Neovim/Helix/Zed claim** in `cli.tmd` + ship the Helix/Zed config snippets. Owed regardless of route. | S | docs |

### Tier 1 — the frontier moves

| # | Item | Effort | Type |
|---|---|---|---|
| 4a | **`textDocument/references`** — re-publish `taliesin/projectRefs` on the standard method. | S | [wire] |
| 4b | **`textDocument/codeLens`** — *Run cell · Run above · ⚡ cached* above every executable fence, in every editor. Absorbs the still-open "make caching legible" item. | M | [new] |
| 3 | **`workspace/diagnostic` pull model** — project-wide Problems panel; also the principled answer to finding 2. | M | [new] |
| 5.2 | **`textDocument/semanticTokens`** — highlighting from the Rust parser; stops the TextMate grammar growing. Reaches Neovim + opt-in Zed, **not** Helix. | M–L | [new] |

### Tier 2 — depth

| # | Item | Effort | Type |
|---|---|---|---|
| 4c | `$/cancelRequest` + `$/progress`; revisit `workspace/symbol`'s 167 ms. | M | [new] |
| 6 | **MCP `resources` + `prompts`** — schemas, the diagnostic catalogue as a resource template, `AGENTS.md`, and scaffold-shaped prompts. | M | [wire] |
| 7 | Terminal + LSP progress for long cells; completion notification. | M | [wire] |
| 8 | `doctor --format json` package manifest; `_freeze/` records the digest it was produced under. | M | [new] |
| 4d | `textDocument/selectionRange` from `lsp_outline`'s section extents. | S | [wire] |

**Not recommended, recorded so they are not re-derived:** a tree-sitter grammar (a third
dialect definition; needs demand first, see finding 5); the VS Code Notebook API (a
second write path into the document — the single-editing-surface invariant forbids it,
and `taliesin/cellRegions` + code lens is the correct shape); `signatureHelp` /
`callHierarchy` / `typeHierarchy` / `documentColor` / `linkedEditingRange` /
`inlineCompletion` (no referent in a prose format); splitting `lsp.rs` as its own task
(finding 9).

---

## Sources

**Frontier (primary).** LSP 3.17 specification + metaModel (`ServerCapabilities`,
`DiagnosticClientCapabilities`, `SemanticTokensWorkspaceClientCapabilities`); Model
Context Protocol server spec 2025-11-25 (`resources`, `resources/templates`, `prompts`,
`completions`, `logging` capability declarations); Zed language-extension docs
(`[grammars.*]` / `[language_servers.*]` in `extension.toml`; `semantic_tokens` setting
`off`/`combined`/`full`, default `off`); Helix — semantic tokens not supported,
tree-sitter is the only highlighter (helix-editor/helix #814, #5589, PR #6102); VS Code
extension API (`lm.registerMcpServerDefinitionProvider`, language-model tools).

**DX research.** Noda, Storey, Forsgren & Greiler, *DevEx: What Actually Drives
Productivity*, ACM Queue 21(2), 2023 — 25 sociotechnical factors reduced to **feedback
loops, cognitive load, flow state**; findings 1/2/3/9 are feedback-loop items, 5/6 are
cognitive-load items. Chattopadhyay, Prasad, Henley, Sarma & Barik, *What's Wrong with
Computational Notebooks? Pain Points, Needs, and Design Opportunities*, CHI 2020 (n=20
interviews, n=156 survey) — nine pain points; the four high-impact activities (median
importance ≥4 **and** difficulty ≥4) are **refactor code, deploy to production, explore
history, long-running tasks**; directly grounds findings 7 and 8, and its "coding
assistance in notebooks is almost non-existent… the only way to debug in most notebooks
is through the use of `print` statements" is the gap Taliesin's debug mode and LSP
already answer better than the field. Green & Petre, *Cognitive Dimensions of
Notations* — the right lens for a markup dialect; not applied in this round, see below.
Becker, Denny et al., *Compiler Error Messages Considered Unhelpful* (ITiCSE-WGR 2019)
and Barik et al., *Do Developers Read Compiler Error Messages?* (ICSE 2017, eye-tracking:
reading an error message is as hard as reading source) — Taliesin's message *wording*
already satisfies this literature; every deficit found here is about **where** the good
messages run, which reproduces the 2026-07-18 audit's conclusion on a new surface.
Rein et al., *Exploratory and Live, Programming and Coding* (arXiv 1807.08578, 212
publications) for the liveness framing. clig.dev for CLI conventions.

**Skeptic note carried forward.** The 2026-07-18 audit flagged the widely-repeated "50%
faster with instant feedback" / "82% of productive time lost to interruptions" /
"progressive disclosure 20–40% faster" numbers as folklore with no locatable primary.
Still true; still not cited here. Every quantitative claim in this file is either a
measurement taken in this round or a figure read off a named paper's own reported data.

---

## Not measured

- **`lsp_complete.rs` (1,700 lines) and `lsp_format.rs` (900) as behaviour.** Both were
  driven only far enough to confirm they answer. Completion *quality* (are the offered
  items the right ones, in the right order?) is its own round, and it is the capability
  a user touches most.
- **The `.tmd` dialect under Cognitive Dimensions.** Viscosity, premature commitment,
  hidden dependencies, error-proneness — the correct academic lens for a notation, and
  the one this round did not have room for. It would be the natural successor round and
  it targets a surface (the dialect) no audit has evaluated as a *notation*.
- **Multi-root / monorepo LSP behaviour.** Probed single-root only. `mounts:` projects
  and the two sibling docs books are exactly the shape that would break a root
  assumption.
- **Windows.** The kernel layer is documented Unix-only; the LSP is not, and was not
  probed there.
- **Real-editor round trip.** Everything here was driven over raw stdio. Nothing was
  confirmed inside a running Neovim/Helix/Zed, which is precisely how finding 5's
  "advertised but unhighlighted" state survived.

## Reproducing

Both probes are committed beside this file, in
[`2026-08-07-devtooling-harness/`](2026-08-07-devtooling-harness/), following the
`ap2-fuzz-harness` / `ap8-determinism-harness` convention. From the repo root:

```sh
python3 notes/2026-08-07-devtooling-harness/lsp_probe.py \
    target/release/taliesin docs/guide/reference/cli.tmd docs/guide
python3 notes/2026-08-07-devtooling-harness/lsp_crossfile.py \
    target/release/taliesin        # builds its own 2-page site in a tempdir
```

Both carry a known-positive control row. `lsp_crossfile.py` asserts against
`taliesin check --format json` on the same tree as ground truth, which is what turned
finding 1 from "the LSP seems quiet" into a named missing code.

## Traps hit

- **A `cd` inside one `Bash` call persisted into the next**, so two greps returned "no
  such file or directory" and would have read as "the symbol does not exist" if the
  error text had been less obvious. Fourth recurrence of this class in the audit record;
  use absolute paths in probe commands.
- **The first cross-file probe returned an all-negative table**, which this file's own
  rule says is a broken probe until proven otherwise. Adding the positive control
  (five diagnostics that *do* arrive) is what made the one absence a finding instead of
  a harness bug.
- **`WebFetch` cannot read a PDF**; it returns a plausible-sounding refusal. The CHI
  2020 paper was only readable because the fetcher saves the binary and `Read`'s `pages`
  parameter extracts it. Do not conclude "the source is unavailable" from that refusal.
