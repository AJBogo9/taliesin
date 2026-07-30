# Editor authoring gestures

**Date:** 2026-07-30
**Branch:** `editor-authoring-gestures-2026-07-30`
**Promotes:** [FEATURE-IDEAS.md](../../../notes/FEATURE-IDEAS.md) Session 3 ideas **73**, **84** and
**82** onto the board, by owner ruling on 2026-07-30, in the same way items 153-174 were promoted
on 2026-07-29.

The `.tmd` file is the single editing surface. Everything in this spec is an edit the *author*
initiates in the editor, so the preview still never writes back and the single-editing-surface
freeze is untouched. That is the reason this cluster is buildable at all: the same value
(reordering, inserting, repairing) was refused as a preview gesture and is in scope as an editor
one.

Three items, three commits, one branch:

| Item | Idea | What it is | Where |
|---|---|---|---|
| A | 73 | The paste and drop cluster: six gestures, one provider pair | TS + `lsp` |
| B | 84 | Rename a file, repair its references in both directions | TS + `lsp` |
| C | 82 | `file:line:` in the dev-server log becomes clickable | TS only |

## Why these three, and not the other VS Code items

The P1 queue holds no VS Code work except item 175(d), which the board blocks behind 175(b)
output streaming. The remaining editor items sit parked in `FEATURE-IDEAS.md` Session 3 as ideas
73 to 86 (ideas 68 to 71 shipped as items 177 and 178). Three clusters were offered; the author
chose the authoring-gesture one over toolchain integration (ideas 80, 85, 82, 79) and over
Jupyter parity (175(b) plus 175(d)), on the grounds that it is the highest felt-per-day value and
that it is the only one of the three that is genuinely extension-shaped rather than mostly Rust.

Idea 82 rides along because it is XS, TS-only, and coupled to nothing else here.

## Starting state (measured 2026-07-30, not taken from a note)

| Fact | Value |
|---|---|
| Candidate APIs already registered by the companion | **none**. No paste, drop, task, terminal-link, URI, MCP, CodeLens, file-decoration provider and no `onWillRenameFiles` |
| `editor/vscode/package.json` `engines.vscode` | `^1.91.0` |
| `@types/vscode` actually resolved | **1.125.0** |
| Local VS Code | 1.130.0 |
| Image refs beside the doc vs in a subdirectory, across `corpus/` + `docs/` | **24 vs 7** (3 of the 7 are one book, 4 are cross-doc reuse of `corpus/media/`) |
| Canonical figure shape | `![caption](file.svg){#fig-label}` |
| Table aligner | `lsp_format::format_tables` ([lsp_format.rs:246](../../../crates/server/src/lsp_format.rs#L246)), `pub(crate)` |
| Project walk | `site::enclosing_site_root` ([site/mod.rs:261](../../../crates/core/src/site/mod.rs#L261)), public; `site::anchors_defined_elsewhere_in_project` ([site/xref.rs:111](../../../crates/core/src/site/xref.rs#L111)) already walks every page from disk |
| Asset containment | `includes::repo_boundary` + `inside_repo`, with **two** distinct warnings in `copy_local_assets` ([build.rs:724](../../../crates/server/src/build.rs#L724), [:732](../../../crates/server/src/build.rs#L732)): outside the doc tree, and outside the repository |
| Dataset feature (item 176) | **shipped**: `render/extension/dataset.rs`, pinned by `corpus/datasets.tmd`. Syntax `{{< dataset path.csv >}}`; the `datasets:` front-matter block is **optional** (size and checksum are read off the file) |
| Corpus CSV idiom | `pd.read_csv` for `{python}`, readr's `read_csv` for `{r}` |
| Custom-request precedent | `taliesin/sectionEdit` returns `{ edits, cursor? }` ([commands.ts:38](../../../editor/vscode/src/commands.ts#L38)) |
| Diagnostic line formats | `file:line: severity[CODE]: msg` ([check.rs:766](../../../crates/server/src/check.rs#L766)), `file: severity[CODE]: msg` ([:769](../../../crates/server/src/check.rs#L769)), `file:line: msg` ([build.rs:1097](../../../crates/server/src/build.rs#L1097)). **No column in any of them** |

## Corrections to FEATURE-IDEAS.md, recorded so they are not re-derived

The ideas file is an asset but four of its specifics are wrong, and each would have shaped the
build:

1. **`images/<slug>-01.png` is the minority convention.** Measured 24 beside-the-doc against 7 in
   a subdirectory. Ruled: write beside the doc.
2. **Idea 82's `page.tmd:12:3` has a column; the real format does not.** A pattern requiring
   `:col` would match nothing. The comment at [check.rs:752](../../../crates/server/src/check.rs#L752)
   already names the shape as the one "a problem-matcher keys off", so it was designed for and
   then mis-transcribed.
3. **Idea 84 is not gated on idea 74 (the project index), and is M rather than L.** The ideas
   file prices whole-book correctness as needing a live index. A rename is a one-shot event, so it
   needs a *walk*, and `anchors_defined_elsewhere_in_project` already does exactly that walk. The
   file's own corrected Fact 1 says so; its cost line contradicts its own fact.
4. **`onWillRenameFiles` has no "native refactor preview".** That belongs to rename-symbol
   providers. These edits apply with the rename, in one undo, with no preview step.

## Decisions taken

Inputs, not open questions. The first four are author rulings from the 2026-07-30 brainstorm; the
fifth was delegated.

1. **Batch:** authoring gestures (ideas 73, 84, 82), over toolchain integration and over Jupyter
   parity.
2. **A pasted image is written beside the doc**, matching the corpus and the containment root.
3. **A pasted image is named from the document stem with a counter** (`bayes-01.png`) and inserted
   as a snippet with tab stops on caption and label. No dialog: the gesture's whole value is that
   it is instant, and the two things only the author can write are where the cursor goes.
4. **A rename repairs both directions**, inbound and outbound.
5. **Asset renames are in, inbound half only** (delegated). Renaming `scree.png` breaks
   `![](scree.png)` in the same way and through the identical scan; an asset has no outbound refs
   of its own. Recorded here so it is a decision rather than a silent widening.

## The seam: what is Rust, what is TypeScript

`CLAUDE.md` is explicit that the companion implements no language features of its own, and
[client.ts:31](../../../editor/vscode/src/client.ts#L31) records why `sectionEdit` exists: a
heading scan in TS is a second copy of the knowledge. So:

| | TypeScript owns | Rust owns |
|---|---|---|
| A | the gesture, clipboard bytes, writing the `.png` | the filename rule, the emitted text, the bib key, the containment verdict |
| B | the `onWillRenameFiles` hook, applying the `WorkspaceEdit` | finding every reference, computing every edit |
| C | all of it | nothing (a drift gate pins the pattern to `check.rs`) |

TS never parses `.tmd`. Rust never touches the clipboard.

**Two new custom requests**, both returning `sectionEdit`'s existing `{ edits, cursor? }` shape so
the TS plumbing is already written:

- **`taliesin/insertEdit`**, tagged by `kind` (`image` | `table` | `bibtex` | `dataset`), the way
  `sectionEdit` is tagged by its four transforms. One method keeps the
  [lsp.rs:4073](../../../crates/server/src/lsp.rs#L4073) census small.
- **`taliesin/renameFileEdits`**, taking an array of `{ oldUri, newUri }` (the Explorer can rename
  a multi-selection) and returning per-file edit lists.

Both need filesystem reads. That is already true of the LSP's diagnostic path
([check.rs:293](../../../crates/server/src/check.rs#L293) walks every page from disk), so it does
not break the kernel-free and offline property, only the mistaken "the LSP makes zero fs calls"
reading of it.

## Item A: the paste and drop cluster (idea 73)

One `DocumentPasteEditProvider` plus one `DocumentDropEditProvider`, registered for the `taliesin`
language only.

### A1. Paste an image from the clipboard

Mime `image/png`, `image/jpeg`, `image/svg+xml`, `image/webp`, `image/gif`, mapped to the
extension. Rust scans the doc's directory and returns the next free `<doc-stem>-NN.<ext>`,
zero-padded to two digits. The stem is **slugged** (lowercased, non-alphanumerics collapsed to
`-`), because a document called `Chapter 1 (draft).tmd` must not produce a filename with spaces
and parentheses in it that every later reference then has to escape. A stem that slugs to nothing
falls back to `figure`. TS writes the bytes. The edit is a `SnippetString`, so the tab stops
survive the paste:

```
![${1:caption}](bayes-01.png){#fig-${2:label}}
```

### A2. Drag an image in from the Explorer

Mime `text/uri-list`, inserting a doc-relative path. The containment check calls
`includes::repo_boundary` and `inside_repo`, **the same functions `copy_local_assets` calls**, and
distinguishes the same two cases the build warns about separately (outside the doc tree, outside
the repository). Using a second rule here would let the editor bless a path the build then warns
on, which is the exact bug class this gesture exists to prevent. Outside the boundary offers a
quick-pick: copy it in beside the doc (consistent with A1), or insert the path anyway.

### A3. Paste a spreadsheet or an HTML table

`text/html` containing a real `<table>` is the **default** paste edit. `text/plain` TSV is offered
in the paste-as menu but is deliberately **not** default: plain text containing tabs is not a
table, and silently becoming one is worse than one extra keystroke. Rust builds the pipe table and
runs it through `lsp_format::format_tables`.

A cell whose text contains `|` is escaped. An unescaped pipe splits the cell; this is a trap
already recorded in `LESSONS.md` and it is the first thing to test.

### A4. Paste a URL over a selection

An absolute `http(s)` URL in `text/plain` **and** a non-empty selection produces
`[selection](url)`. An empty selection falls through to a plain paste and does nothing clever. No
setting, unlike VS Code's own `markdown.editor.pasteUrlAsFormattedLink.enabled`: minimal-config
says perfect the default instead.

### A5. Paste a BibTeX entry

Detected on `@type{key,` and parsed by the existing `cite/` bib parser. The target `.bib` is the
front matter's `bibliography:`, else the site-level `bibliography:` from `_site.yml` (item 163).
The append lands as `DocumentPasteEdit.additionalEdit`, a `WorkspaceEdit` over the `.bib` URI, so
the whole gesture is **one undo** and the file write is not a side effect behind the editor's back.

If the key already exists in that `.bib`, insert `[@key]` and append nothing: `parse_bib_warned`
lints duplicate keys, so appending one would make the gesture trip the author's own lint.

If the document has no bibliography at all, insert `[@key]` and stop. The existing
`citations_without_bibliography` diagnostic already says what is wrong. Creating the `.bib`,
editing the front matter and pasting is three coupled writes for the least common case.

### A6. Drop a `.csv`

Inserts `{{< dataset <relpath> >}}` plus a loader cell. The cell language is the language of the
**first** code cell in the document, and `{python}` when the document has no cells at all. First
rather than nearest, so the same document always yields the same answer no matter where the drop
lands; a document mixing `{python}` and `{r}` therefore gets its opening language, which is the
one the reader has already been taught to expect. The import line is emitted **only if the
document does not already import it**, which Rust can see from the buffer.

No `datasets:` front-matter scaffold. `corpus/datasets.tmd:31-33` states that nothing in the front
matter is required, because size and checksum are read off the file; the entry adds only licence,
source and description, which are facts the author has and the editor does not. Emitting empty
placeholders is noise that a lint may flag.

## Item B: rename repair in both directions (idea 84)

`workspace.onWillRenameFiles` with `event.waitUntil(...)`, so edits land atomically with the
rename and the whole operation is one undo.

**Inbound.** Find the enclosing project with `site::enclosing_site_root`, walk every page, and
rewrite:

- `{{< include … >}}` and `{{< embed … >}}` targets
- relative links **in both spellings**. Cross-page links are authored as `.html`
  (`../guide/using/formats.html`), so renaming `intro.tmd` must rewrite `intro.html`; item 128's
  migration work means `.tmd`-spelled links also exist. Handling one spelling makes the repair
  silently half-done, which is worse than not running.
- `_site.yml` entries

**Outbound.** The moved file's own relative refs (images, includes, `{{< dataset >}}` paths,
links), rebased from the old directory to the new. **Skipped entirely when the directory is
unchanged**, since an in-place rename breaks none of them.

Three constraints:

1. **`_site.yml` is edited as text, never re-serialized.** A YAML round-trip reformats the
   author's file and drops comments. Rust returns a range over the exact scalar, located by
   scanning lines for the known old path.
2. **No `_site.yml` means no inbound walk.** Item 70 rules that a project with no `_site.yml`
   declares no boundary and that inferring one is the wrong move, so a bare document gets the
   outbound half only. A cited limitation, not an oversight.
3. **No confirmation prompt.** TypeScript's `updateImportsOnFileMove.enabled` offers
   `prompt|always|never`; minimal-config prefers a better default, and one-undo makes a prompt
   unnecessary.

**Assets** (per decision 5) take the inbound half only.

## Item C: terminal links (idea 82)

`window.registerTerminalLinkProvider`, matching the three shapes the tree emits, anchored at line
start so the ANSI severity colour is never in the way, and with **no column group**:

```
file:line: severity[CODE]: message      check.rs:766
file: severity[CODE]: message           check.rs:769
file:line: message                      build.rs:1097
```

Path resolution uses the terminal's cwd where the companion owns the terminal (`runInTerminal`
already sets it), else the workspace folders. An ambiguous candidate produces **no link** rather
than a link that opens the wrong file.

**The drift gate is the point.** A hand-written regex in TS against a format string in Rust is
exactly the drift this project pins elsewhere (`release_targets.rs`, `site_build_script.rs`). A
Rust test reads the TS source, extracts the pattern literal, and asserts it matches a diagnostic
string produced by `check.rs`'s own formatter. Mutation-checkable both ways: change the format,
red; loosen the regex, red.

## Testing and gates

**Layers, because no one of them is sufficient:**

- **Rust unit tests** for every `insertEdit` kind and both `renameFileEdits` directions.
- **Rename tests run against a temp copy of `corpus/tarn`** (12 numbered chapters, 3 parts
  including a nested one: the cross-directory shape a move needs). Copied, never mutated. The
  walker renders every corpus doc on every `cargo test`, so a test that edited the corpus in place
  would poison every later assertion. This also satisfies the standing "use `corpus/tarn` instead
  of minting a fixture" rule.
- **A real Extension Host e2e test per gesture** in `editor/vscode/src/e2e/suite`. Not
  belt-and-braces: the math-delimiter work was pinned at three levels precisely because a manifest
  and a unit test cannot prove VS Code *accepted* a provider, and a paste provider registered
  against a stale `engines.vscode` fails in exactly that way.
- **Mutation verification** on each fix: restore the bug, watch the named test fail. A green suite
  is not evidence.

**Gates this batch trips, named so none is a surprise:**

- `the_internals_book_documents_every_taliesin_namespaced_method`
  ([lsp.rs:4073](../../../crates/server/src/lsp.rs#L4073)) fails unless each new method gets a row
  in `docs/internals/extending.tmd`, in the same commit.
- Both `tsc` type-checks (`web-client`, `crates/core/assets/js`) and the companion's own build.
- `./tools/gates.sh`, with all interpreter canaries named and passing.
- `docs/guide/using/writing.tmd` gains the author-facing description of the gestures.

**Known hazard, recorded rather than chased:** the companion e2e suite is load-sensitive. The two
list-continuation tests fail with "the Enter keystroke was never delivered" at load ~6-7 on `main`
as well as on a branch. If they go red, run alternating baseline/branch pairs before calling it a
regression.

**Verify before building, do not assume:**

- the exact VS Code version that finalized `registerDocumentPasteEditProvider` (believed ~1.97),
  then bump `engines.vscode` to it. The tree already declares `^1.91.0` while resolving types at
  1.125.0, so this inconsistency predates the batch and this is where it gets fixed.
- that `DocumentPasteEdit.insertText` accepts a `SnippetString` (believed yes; A1 depends on it).
- whether `TerminalLinkContext.line` arrives with ANSI stripped. Anchoring at line start makes C
  correct either way, but the test should state which is true.

## No new corpus documents

These are editor capabilities, not render capabilities: there is no `.tmd` anyone could write that
exercises a paste gesture. The corpus-plus-roadmap pin obligation is met by `corpus/tarn` serving
as the rename fixture and by the already-shipped `corpus/datasets.tmd` as the CSV-drop target.
Growing `corpus/` here would add walker cost on every `cargo test` and buy nothing.

## Out of scope

Named so a later session does not read this spec as covering them:

- Ideas **80** (task provider and problem matchers), **85** (MCP definition and LM tools), **79**
  (status bar), **83** (URI handler): the toolchain batch, not chosen. Idea 80 is the natural
  follow-up, since `check.rs`'s format is already problem-matcher shaped and item C proves the
  pattern.
- Ideas **74** to **78**: gated on the project index, which is the one item that meaningfully
  changes the LSP's architecture.
- Ideas **67** (semantic tokens, needs re-justification and a colour-theme ruling) and **72**
  (colour provider, lowest value in its cluster).
- **175(b)/(d)** and idea **86**: Jupyter parity. Item 175(d) must be built from the backlog entry,
  not from idea 86, and not from both.
- **Directory renames.** VS Code reports a directory rename as one event without enumerating its
  children, so it is a separate mechanism with its own walk and its own tests. Offered and not
  chosen.
