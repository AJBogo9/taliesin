# Finishing the editor scope (2026-07-30)

**Status:** approved by the author 2026-07-30. Supersedes the parked entries for ideas 67, 72,
74-81, 83 and 85 in [FEATURE-IDEAS.md](../../../notes/FEATURE-IDEAS.md) Session 3.

**Goal.** Close the VS Code / LSP surface opened by Session 3, so "the editor scope" is finished
rather than perpetually one cluster short. Eleven ideas remained. This spec builds eight, cuts
three on evidence, and records why, so none of the three is re-proposed.

**Out of scope by author ruling (2026-07-30):**

- **Idea 86 (CodeLens run/interrupt on cell fences)** stays filed as backlog **175(d)**. It is
  blocked on 175(b), output streaming, which is a server/protocol change, not an editor one.
  Do not build it from this spec.
- **Idea 81 (Testing API)** is dropped. Idea 80 delivers the actual need (project-wide health
  without a terminal) at a fraction of the size, and a test tree would be a second surface over
  the same `check` data.

## 1. Ground truth (measured this session, not recalled)

Every estimate below rests on these. The standing anti-rot rule applies: re-check before building.

| # | Fact | Evidence |
|---|------|----------|
| G1 | None of 67, 72, 74-81, 83, 85 exist today. | `server_capabilities()` advertises 11 capabilities; the TS side registers exactly three providers (paste, drop, terminal links). |
| G2 | The project walk already computes locations and throws them away. | `scan_page_anchors` yields `ScannedAnchor { id, number, title, line }` (`crates/core/src/site/xref.rs:227`); `anchors_defined_elsewhere_in_project` (`:111`) walks every page in the project and keeps only `id`. |
| G3 | A project-wide `check` is cheap enough to run in the background. | `check docs/guide` (25 pages) = **369 ms**, release binary verified at HEAD `2f9a901`. |
| G4 | `check --format json` already emits the shape a decoration provider needs. | `{diagnostics:[{code, docs_url, severity, file, line, message, suggestion?}], environment:[...]}`. |
| G5 | Heading segmentation is already reusable. | `lsp_outline::sections`, `lsp_outline::atx_heading`, `lsp_outline::fence_marker` are `pub(crate)`. |
| G6 | **`@types/vscode@1.101.0` is the first release carrying `registerMcpServerDefinitionProvider`.** | Packed both: 1.100.0 → 0 occurrences, 1.101.0 → 3 (`registerMcpServerDefinitionProvider`, `McpStdioServerDefinition`). |
| G7 | `LanguageModelTool` is present at the current floor. | 43 occurrences in the pinned `@types/vscode@1.97.0`. |
| G8 | The webview relay is deliberately narrow. | `UP = ["tali-goto", "tali-page"]`, `DOWN = ["tali-cursor", "tali-navigate"]` (`editor/vscode/src/webview.ts:16`). |
| G9 | The standalone browser preview already reaches the editor. | `client.js:1833` and `:1879` navigate to `vscode://file<abs>:<line>:<col>`. |
| G10 | The document selector is `.tmd` only. | `client.ts:62`: `{ scheme: "file", language: "taliesin" }` plus the `untitled` twin. |

## 2. The three cuts

### 2.1 Idea 67 (semantic tokens): cut, because idea 75 removes its last justification

Session 3's Fact 7 already killed 67's headline pitch (unresolved refs squiggle via diagnostics
already). What the rewritten entry kept was one case: distinguishing *locally defined* from
*defined on another page*, on the grounds that "both are correct, neither warns, and **only one is
reachable by go-to-definition**".

Idea 75 makes both reachable. Once F12 works across pages, which side of a file boundary a target
sits on is no longer information the author needs painted into the buffer. The residual case
("kinds where TextMate is approximate") does not carry an M-sized provider on its own, and 67's
unresolved theme-versus-scopes design question would have to be answered first.

**Recorded consequence:** 67 is cut, not deferred. Its parenthetical about math delimiters is
already obsolete (that shipped as one `contributes.configurationDefaults` rule).

### 2.2 Idea 74 (project index): dissolve into a walk, do not build

74 is priced **L** and flagged **needs-care** as "the one item in this session that meaningfully
changes the LSP's architecture", because it introduces indexing, invalidation and file watching
into a component whose statelessness is why it is reliable.

It is not needed. Every surface gated on it (75, 76, 77, 78) fires on a **user gesture**: F12,
Ctrl+T, opening a view, the Explorer asking for a decoration. None fires per keystroke. This is
exactly the re-costing the ideas file already demands after item 84: *"Anything else in Cluster C
that fires once per gesture rather than once per keystroke should be re-costed the same way before
being priced against 74."*

**Replacement:** one module, `crates/server/src/lsp_project.rs`, holding a project walk behind a
**stat-validated single-entry memo**. Validation is `(path, mtime, len)` for every page in the
project, compared against the previous walk; a mismatch re-walks, a match returns the cached
result. No file watcher, no invalidation protocol, no background thread. A `stat` per page is
orders of magnitude below the read-plus-parse it guards, and correctness degrades to "re-walk more
often than necessary", never to "serve stale data".

The per-keystroke path (`didChange` → `publish` → diagnostics) is untouched and keeps using
`anchors_defined_elsewhere_in_project` behind its existing 120 ms coalescing window.

### 2.3 Idea 83 (URI handler): cut, its premise is rot

83 claims a custom `vscode://taliesin.taliesin-companion/open?file=…&line=…` handler "closes
click-to-source for the **standalone browser preview**, which today only bridges back inside the
webview relay". Per G9 that is false: the browser path already navigates to `vscode://file`,
VS Code's built-in handler, with line and column. The feature ships; only the mechanism differs
from the one 83 imagined. A custom handler would add extension-specific routing over a path that
already works, for no behaviour the author would notice.

## 3. Architecture

Two rules constrain every choice below and neither is negotiable.

- **Editor intelligence lives in Rust.** The companion implements no language features of its own.
  Anything that could be expressed as an LSP request is an LSP request; TS holds only what LSP has
  no concept of (a TreeView, a status bar item, a task provider, a decoration).
- **Single editing surface.** Every surface here is read-only navigation or a diagnostic. Nothing
  writes back to the document except through the author's own explicit gesture, and nothing here
  adds such a gesture.

### 3.1 The substrate: `crates/server/src/lsp_project.rs`

One public entry point returning a borrowed snapshot of the enclosing project:

```
ProjectScan {
    root: PathBuf,                       // the enclosing `_site.yml` directory
    anchors: Vec<ProjectAnchor>,         // id, path, line, title, number, kind
    headings: Vec<ProjectHeading>,       // path, line, level, text, section extent
    uses:     Vec<ProjectUse>,           // path, line, col, referenced id
}
```

Built from pieces that already exist, so no second scanner can disagree with the first:

- project root and page list: `site::enclosing_site_root` + `site::collect_pages`, the latter
  currently `pub(super)` in `discovery.rs:128` and needing promotion,
- anchors: the existing `scan_page_anchors`, exposed from `site/xref.rs` with its `line` intact
  (G2). This is the one new `pub` in core.
- headings: `lsp_outline::sections` per page (G5),
- uses: see 3.1.2 below.

#### 3.1.1 One targeted cleanup, in the path of the work

`enclosing_site_root` exists **twice**: `site/mod.rs:261` (`pub`) and `site/xref.rs:139`
(private, used by `anchors_defined_elsewhere_in_project`). Project-root discovery is precisely what
`lsp_project.rs` needs, and adding a third caller against a duplicated definition is how the two
copies drift apart. Consolidate to the `mod.rs` owner as part of the substrate task, verifying the
two implementations agree first. If they do **not** agree, that difference is a finding: stop and
record which behaviour is correct before deleting either.

#### 3.1.2 `uses` needs a scan-all sibling, and it is the honest part of the old "scan_all"

`lsp_nav::anchor_occurrences(text, id)` is a **targeted** search: it finds occurrences of one known
id. That answers "what links to this target" but cannot find a **dangling** reference, whose id is
by definition not in the anchor set, and the References view has to group those.

Add `lsp_nav::xref_occurrences(text) -> Vec<(id, line, col)>`, a full-document scan sharing
`anchor_occurrences`'s existing character-level predicate rather than re-deciding what a reference
looks like. Session 3's Fact 5 predicted a general `scan_all` and items 70/71 proved it unnecessary
for them; this is the one consumer that genuinely needs it, scoped to xref occurrences and nothing
else. **The two functions must be pinned against each other**: for any id present in the document,
`xref_occurrences` filtered to that id must equal `anchor_occurrences` for it. A second scanner
free to disagree with rename about what an anchor is, is the exact trap item 70 avoided.

Include resolution mirrors what the existing walk does (`includes::resolve` before scanning), so an
anchor authored in an `_includes/` partial belongs to whichever page includes it.

For a document with no enclosing `_site.yml`, the scan is empty and every consumer degrades to
today's document-local behaviour. That is the standalone-document case and it must stay silent,
not error.

### 3.2 Rust-side consumers

**Idea 75, cross-file xref resolution.** `textDocument/definition` and hover consult `ProjectScan`
when the document-local lookup misses, closing the gap documented at `lsp.rs:434`. Hover on a
cross-page `@sec-` names the target's page as well as its number, which `XrefTarget` already
carries (`url`, `number`, `title`).

**Idea 76, workspace symbols.** A new `workspace/symbol` handler over `ProjectScan.headings` plus
`ProjectScan.anchors`, so Ctrl+T reaches any heading, figure, table or equation in the book.
Matching is a case-insensitive substring on the symbol name; VS Code does its own fuzzy ranking on
top, and inventing a second ranking here would fight it.

**Idea 72, document colours.** A `documentColor` / `colorPresentation` provider for `--tali-*`
values. Requires widening the client's document selector (G10) to include `_site.yml`, because
that is where the tokens are authored; front matter in a `.tmd` is already covered. The provider
must ignore any custom property whose value is not a colour, and must not offer a presentation
that would rewrite a token to a format the theme system does not accept.

**Two custom requests for the sidebar** (idea 77), following the `taliesin/cellRegions` precedent
of keeping the knowledge in Rust:

- `taliesin/projectOutline` → the whole-book heading tree plus the figure/table/equation index,
- `taliesin/projectRefs` → uses grouped by target, with unresolved targets flagged.

### 3.3 TS-side consumers

**Idea 77, the sidebar.** A `viewsContainers` + `views` contribution with **three** TreeViews,
each a thin projection of one of the requests above:

1. **Outline**: the whole book, chapters and sections, click to reveal.
2. **References**: what links here, grouped by target, with dangling references grouped separately.
3. **Figures & tables**: the numbered-float index.

Two of the idea's five views are **cut**: the bibliography view (splitting cited from uncited) is
low value given `check` already reports unresolved citations, and the kernel panel belongs to idea
79. Explicitly **not** drag-to-reorder: that is the removed slide-reorder mistake in a new costume.

**Idea 80, tasks and problem matchers.** A `TaskProvider` auto-discovering `taliesin check`,
`taliesin build` and `taliesin build --out` for the active project, plus a
`contributes.problemMatchers` entry keyed on the existing human format,
`path:line: severity[CODE]: message`. **There is no column in that format**; a pattern requiring
`:col` matches nothing. This puts project-wide diagnostics in the Problems panel for files that are
not open, which today needs a terminal.

**Idea 78, file decorations.** A `FileDecorationProvider` badging `.tmd` files by worst `check`
severity, fed by a background `check --format json` over the project (G3, G4), refreshed on save
and on task completion. **Scoped to the check-status dot only.** The idea's `⚡ fully cached` and
never-executed-cells badges need freeze-key machinery that lives in `exec`, and the LSP is
deliberately kernel-free; those are deferred with the reason recorded, not silently dropped.

**Idea 79, status bar.** One item showing whether a preview is running for the active document's
project (with its port) and the project's problem count, click to open or focus the preview. **Live
kernel and cache state is deferred**: per G8 the relay carries four message types on purpose, and
widening it is a protocol decision that should be made on its own merits, not as a side effect of a
status bar.

**Idea 85, the AI-native surface.** Two halves with different floors:

- `contributes.languageModelTools` registering the existing `taliesin mcp` tools (`check`, `read`,
  `symbols`, `map`, `vocab`, `build`) as native LM tools. Ships at the current floor (G7).
- `lm.registerMcpServerDefinitionProvider`, advertising `taliesin mcp` to VS Code so the user never
  hand-edits config. **Requires raising `engines.vscode` to `^1.101.0` and re-pinning
  `@types/vscode` to exactly `1.101.0`** (G6).

### 3.4 The engines floor

The last batch established that `engines.vscode` is a promise the compiler checks **only** if the
types are pinned to the same number: `^1.97.0` resolves to the latest types and re-opens the gap it
was meant to close. So the pin stays exact and both fields move together, from
`^1.97.0` / `1.97.0` to `^1.101.0` / `1.101.0`. 1.101 is roughly thirteen months behind current
stable, so the compatibility cost is negligible, and `manifest.test.ts` pins the pair.

## 4. Testing

**Editor features do not render, so the corpus is not their pin.** Their equivalents are:

- **`the_internals_capability_table_names_every_capability_the_server_advertises`** already fails
  when a capability is advertised with no documentation row. Every new LSP capability here
  (`workspaceSymbolProvider`, `colorProvider`) therefore forces a row in
  `docs/guide/using/preview.tmd`, and the gate is mutation-verified in both directions.
- **`manifest.test.ts`** pins `engines.vscode` against the exact `@types/vscode` version, and gains
  rows for the new contribution points.
- **Rust unit tests** on `lsp_project.rs` against a temporary project fixture, covering: the
  standalone-document empty case, include-resolved anchors, a duplicate label across two pages, and
  **the memo's invalidation** (walk, touch a file's mtime, walk again, assert the second walk saw
  the change). A memo whose invalidation is untested is the failure mode this design chose over
  74's file watcher, so it is the one thing that must not be vacuously pinned.
- **The scanner-agreement pin from 3.1.2**: `xref_occurrences` filtered to a given id must equal
  `anchor_occurrences` for that id, over a fixture carrying a definition, several references and a
  citation (which must be excluded from both). `anchor_occurrences` already has such a fixture at
  `lsp_nav.rs:1027`; extend it rather than mint a second one that could drift.
- **`node --test` / mocha unit tests** for the problem-matcher regex against real `check` output,
  and for the decoration provider's severity mapping.
- **e2e (`npm run test:e2e`)** for the surfaces only a real Extension Host can confirm: that VS
  Code accepts the TreeView contributions, the task provider, and the LM tool registrations. A unit
  test does not prove the host accepted a provider. Note the suite is **load-sensitive**: two
  list-continuation tests fail at load ~6-7 on `main` as well as on a branch. Alternate
  baseline/branch runs before calling anything a regression.
- **Mutation verification** per the standing rule: restore each bug, watch the named test fail.
  The last four batches each had mutation find defects a green suite did not.

**Detection debt to file** in [DETECTION-DEBT.md](../../../notes/DETECTION-DEBT.md): the
`⚡ cached` and never-executed decoration badges are unobservable without freeze-key access from a
kernel-free component, and live kernel state is unobservable without widening the relay.

## 5. Build order

Dependencies first, then cheapest-per-value, with the floor bump last so a failure there cannot
block anything else.

1. `lsp_project.rs` substrate (+ the one new `pub` in `site/xref.rs`)
2. **75** cross-file xref resolution
3. **76** workspace symbols
4. **77** sidebar, three views (needs the two custom requests)
5. **80** task provider + problem matchers
6. **78** file decorations (check-status only)
7. **79** status bar
8. **72** document colour provider (+ selector widening)
9. **85a** `languageModelTools` at the current floor
10. **85b** MCP server definition provider, behind the `^1.101.0` / `1.101.0` bump

## 6. What this closes

On landing, Session 3's Cluster A is finished (68-71 shipped, 67 and 72 resolved), Cluster C is
finished (74 dissolved, 75-79 shipped), Cluster D is finished (80 shipped, 81 and 83 cut, 82 and 84
already shipped), and Cluster E is finished (85 shipped). Cluster F's single item, 86, remains as
backlog 175(d) behind output streaming, and that is the only editor-surface idea left anywhere.
