# Demand-probe findings, OSS docs-maintainer persona

**Date:** 2026-07-22 · **Spec:** docs/superpowers/specs/2026-07-22-corpus-demand-probe-design.md
**Plan:** docs/superpowers/plans/2026-07-22-corpus-demand-probe-docs-maintainer.md
**Persona:** a solo maintainer of a small OSS dataframe library ("Tarn"), hosting its
documentation as a Taliesin book (Guide + API Reference) with tabbed install/usage
panels and full-text Cmd-K search.

Categories: `gap` (in-scope capability missing) · `friction` (works but awkward) ·
`interaction-bug` (breaks only in combination) · `correctly-refused` (a settled non-goal).

## Findings

<!-- One entry per finding:
### F-NN, <title>  [category · Pn]
**Wanted:** … **Happened:** … **Repro:** … **Disposition:** …
-->

### F-01, `powershell` is not a recognized highlight language  [friction · P3]

**Wanted:** As a docs maintainer, put a Windows install snippet in a
```` ```powershell ```` fenced block (the near-universal convention for Windows docs) and
have it syntax-highlighted like the `bash` blocks in the other OS tabs.
**Happened:** `powershell` is not in the bundled syntect set, so the block renders as
**unstyled plain text** and `check` emits `install.tmd:NN: warning[TAL-CODE-LANG]: unknown
code language ``powershell``…`. It degrades gracefully (readable plain text, build still
succeeds), so this is friction, not breakage. `bash` highlights fine (`tali-hl-bash`
spans), which makes the missing coverage conspicuous in a per-OS tabset where the macOS/
Linux tabs are highlighted and the Windows tab is not.
**Repro:** a ```` ```powershell ```` code block anywhere; `taliesin check` warns
`TAL-CODE-LANG` and the block emits no `tali-hl-*` spans.
**Disposition:** backlog. In-scope highlight-coverage gap (`two-face` ships a PowerShell
syntax; adding it to the bundled set would close it). Workaround used in the shipped
exhibit: the Windows tab uses ```` ```bash ```` (the install commands are shell one-liners),
so the gallery doc stays `check`-clean and fully highlighted. P3 — graceful degradation +
a trivial workaround.

### F-02, the a11y heading-skip lint fires on the natural API-reference shape  [friction · P3]

**Wanted:** Author an API-reference page as a page title (`#`) followed by a flat list of
per-method entries (`###`), the way many hand-written API docs are structured.
**Happened:** `check` warns `api-frame.tmd:7: warning[TAL-A11Y-HEADING]: heading level
skips from h1 to h3 (add an intervening heading, or demote this one)` for each such page.
The linter is **correct** (a skipped heading level is a real screen-reader problem), but
it means the common "title + flat `###` entries" API-reference layout is not clean by
default; you must either demote entries to `##` or insert a grouping `##`.
**Repro:** any page whose only headings are one `#` and several `###`; `taliesin check`
warns `TAL-A11Y-HEADING`.
**Disposition:** working-as-intended, no engine change. Logged because it is a genuine
authoring-DX moment for this persona: the a11y linter shapes API-reference structure.
Resolved in the exhibit by demoting method entries to `##` (which also lists them in the
book TOC/sidebar, a navigation win). P3, no defect.

### F-03, `read` concatenates adjacent list items with no separator  [friction · P3]

**Wanted:** `taliesin read` an API-reference page and get a legible text projection of each
method's **parameter list** (one item per parameter, plus a Returns line).
**Happened:** adjacent list items run together with no space or newline between them:
`read corpus/tarn/api-frame.tmd` projects the two bullets `- **columns** — …` and
`- **Returns** — …` as one string `…column names.Returns — a new Frame…`. The item
boundary is lost, so a parameter list (ubiquitous in API docs) reads as a run-on. The same
`render::indexable_text` pass feeds the full-text search index, where the concatenation is
benign (matching still works), but for the machine-*reading* projection it is a real
readability gap.
**Repro:** `taliesin read <a doc with a bullet list>`; adjacent `<li>` text is joined with
no separator.
**Disposition:** backlog (P3). In-scope polish of the `read`/`indexable_text` projection:
insert a separator (newline or bullet) between list items in the text projection. HTML
renders the list correctly, so this is projection-only.

### Confirms pilot F-02 (`read` loses cross-reference resolution) on a second persona

Not a new number: the pilot's finding **F-02** (a book chapter's `read` projection drops
cross-reference resolution) reproduces here on a *different* ref type. `read
corpus/tarn/api-frame.tmd` projects `@sec-lazy` as the bare word "**Section**" ("see
**Section** for why nothing runs"), and an in-prose cross-page link
`[Frame.filter](api-frame.tmd#fn-filter)` projects as bare text "Frame.filter" with the
target dropped. The pilot found this for `@thm-`/`@sec-` in a course; seeing it again for
`@sec-` + `.tmd#anchor` links in a docs book confirms the finding is **persona-independent**
and strengthens the existing backlog item (book-aware `read` with resolved refs). Logged
here as corroboration, folded into the same backlog entry, not double-counted.

## Progress log (which surfaces produced findings)

- **Task 1 scaffold** clean: the book builds (6 pages + `search-index.js`), all 24 corpus
  invariants pass with `corpus/tarn/` included.
- **Task 2, ch1 (install):** two `.panel-tabset`s on one page (package-manager pip/conda/uv
  + per-OS macOS/Linux/Windows) lower correctly to ARIA tabs (2 `role="tablist"`, 6
  `role="tab"`, 6 `role="tabpanel"`). **Every panel's content — including non-default
  tabs — is present in the built HTML and in `search-index.js`** (all of `pip install
  tarn`/`conda install`/`brew install tarn`/`scoop install tarn` are indexed as plain
  text): tabset-hidden content is fully offline-complete and searchable, no lazy gap. One
  finding: **F-01** (`powershell` unhighlighted).
- **Task 4, ch3 (concepts):** authored cleanly, no findings. `@fig-dataflow` numbers
  chapter-scoped (Figure 3.1) with no config; both callout kinds (`callout-note` with a
  `title=`, bare `callout-tip`) render; the hand-authored `dataflow.svg` (named colors, no
  hex) is offline-complete; the `#sec-lazy` subsection anchor is in place for the later
  cross-page refs. A "covered" case. `check` clean.
- **Task 5, ch4/ch5 (API reference):** the reference part works: all five method/function
  anchors (`#fn-select`/`#fn-filter`/`#fn-groupby`, `#fn-col`/`#fn-lit`) render; the
  **cross-page** links resolve both directions (`api-frame`→`api-query.html#fn-col`,
  deprecation callout→`api-frame.html#fn-filter`, `.tmd#anchor`→`.html#anchor`); `@sec-lazy`
  resolves cross-page to "Section 3.2"; the `callout-warning` deprecation box renders. Two
  findings: **F-02** (a11y heading-skip lint on the flat-`###` API shape; WAI, demoted to
  `##`) surfaced by `check`; **F-03** (`read` concatenates list items) plus a
  cross-persona **confirmation of pilot F-02** surfaced by the `read` probe.
- **Task 3, ch2 (quickstart):** the headline docs interaction WORKS end to end and
  produced **no findings**. A `.code-walkthrough` (`data-cw-lines` 1-4) whose *step prose*
  carries **cross-page links into the reference** (`api-frame.html#fn-filter`,
  `api-query.html#fn-col`, `api-frame.html#fn-groupby`), a `@sec-install`→"Chapter 1" and
  `@sec-lazy`→"Section 3.2" cross-page xref, and a per-language usage `.panel-tabset`
  (Python/CLI) all render together; `check` clean. The full-text search index **spans the
  guide and the reference** (`quickstart.html` + `api-frame.html` entries) and includes the
  CLI tab's `tarn query sales.csv`. This is the demand-probe's core positive result: the
  tabset × walkthrough × guide→reference-links × cross-book-search *combination* is solid.

## Roll-up (filled at Task 8)
- gaps: … · friction: … · interaction-bugs: … · correctly-refused: …
