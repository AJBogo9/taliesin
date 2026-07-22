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

## Roll-up (filled at Task 8)
- gaps: … · friction: … · interaction-bugs: … · correctly-refused: …
