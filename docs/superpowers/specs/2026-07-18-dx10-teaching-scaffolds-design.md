# DX10 — Scaffolds that teach

Date: 2026-07-18. Backlog item **DX10** (§6 DX audit batch, Tier 2 workflow-smoothers).
Branch `dx10-teaching-scaffolds`. Detail source: `notes/2026-07-18-dx-audit.md`.

> **Autonomy note:** author is away and asked me to proceed without the interactive gate.
> Decisions below are documented defaults. **Scope call:** implement three of the four sub-parts
> the audit lists — the paper worked example, the `init`→`new` pointer, and `new post --draft` —
> and **defer `new deck --tour`** (see Non-goals for why). Each shipped sub-part is `check`-clean-
> verifiable without a browser.

## Goal

Make Taliesin's scaffolds *teach the format* instead of emitting near-blank stubs. The audit's
headline: the single most-delightful discovery — that Quarto's `#| label:` / `#| fig-cap:` cell
options **work verbatim** — is invisible, because no scaffold shows a runnable figure. A worked
starter teaches better than docs (time-to-first-success research, #10 in the audit).

## Ground truth (grepped + measured 2026-07-18)

- **`new_files(kind, slug, today) -> Vec<(PathBuf, String)>`** ([`cli.rs:248`](../../../crates/server/src/cli.rs))
  is a pure function; `write_new` + `cmd_new` are thin wrappers. Scaffolds are **check-clean**-
  pinned by `crates/server/tests/new_cli.rs` (runs the real binary + `taliesin check`) and their
  bytes are **mirrored** into `corpus/scaffold/`, which the corpus regression net renders + lints
  (`crates/core/tests/corpus.rs` walks `corpus/` recursively). There is **no automated byte-diff**
  between `new_files` and `corpus/scaffold/` — the mirror is hand-kept, so a template change must
  update the matching `corpus/scaffold/` file too.
- **A `{python}` cell keeps a doc `check`-clean with no kernel.** Measured: `taliesin check` on a
  `{python}` doc reports interpreter/ipykernel status only in an **informational** "Environment"
  block, never as a counted diagnostic (the sandbox has no ipykernel; the only problem on
  `pca-geometry` was an unrelated cross-project include). Exit code is driven by the problem count.
- **`#| label: fig-x` resolves `@fig-x` statically.** `cargo test -p taliesin-core` renders without
  executing cells, yet corpus docs with `{python}` fig cells + `@fig-` refs pass — so the label is
  collected at parse time. Section refs `@sec-x` resolve to a `{#sec-x}` heading id.
- **`draft:` is a valid front-matter key** (`frontmatter.rs:38`, `vocab.rs:57`, schema
  `"draft": {}`), so `draft: true` is `check`-clean.
- **`init` scaffolds `index.tmd`** from `INIT_INDEX_TMD` ([`cli.rs:24`](../../../crates/server/src/cli.rs)),
  whose "Next steps" list is the natural place to point at `taliesin new`.

## Changes

### Sub-part 1 — `paper` worked example (the headline)

Extend the `paper` `index.tmd` (`new_files`, the `NewKind::Paper` branch) to a *worked* research
starter that stays `check`-clean:

- Keep the citation + `references.bib` (unchanged).
- Add a `## Methods {#sec-methods}` section and reference it as `@sec-methods` from the intro.
- Add a runnable `{python}` matplotlib figure cell with `#| label: fig-demo` + `#| fig-cap:`, and
  reference it as `@fig-demo`. (Matplotlib is the academic persona's default; the caption says
  "replace this with your result.")
- Add one `$$ … $$` display-math block.

All literal `{`/`}` in the template are `format!`-escaped (`{{python}}`, `{{#sec-methods}}`), as the
existing paper already does for `{{#fig-x}}`.

Mirror the new bytes into `corpus/scaffold/posts/my-paper/index.tmd` so the corpus net renders +
lints it. Extend `new_cli.rs` (`a_paper_ships_its_bibliography_…`) to assert the doc now contains
`{python}`, `#| label: fig-`, `@fig-`, and `$$`.

### Sub-part 2 — `init`'s `index.tmd` points at `taliesin new`

Add a "Next steps" bullet to `INIT_INDEX_TMD`:
`- Start a post with \`taliesin new post my-first-post\` (add \`--draft\` to hold it back).`
Verified by the existing init serve/build smoke (the index still parses + previews).

### Sub-part 3 — `new post --draft`

Thread a minimal option into the pure scaffolder:

- `struct NewOpts { draft: bool }` (Copy). `new_files(kind, slug, today, opts)` and
  `write_new(root, kind, slug, opts)` gain the param; `cmd_new` parses `--draft`.
- `--draft` interpolates a `draft: true\n` line into the scaffolded front matter (a `{draft}` slot
  right after the `title:` line). Applied uniformly (any kind — `draft:` universally means
  "unpublished"; no kind-compat matrix). Default off → byte-identical to today's scaffolds, so the
  `corpus/scaffold/` mirror and every existing test stay valid.
- `--draft` added to `NEW_FLAGS` (so the unknown-flag did-you-mean still fires for typos).
- `new_cli.rs`: `new post x --draft` → the doc contains `draft: true` and is `check`-clean.

## Testability (TDD)

- **`new_cli.rs`** (integration, runs the real binary): (a) paper contains `{python}` + `#| label:
  fig-` + `@fig-` + `$$` and `check` is clean; (b) `new post --draft` contains `draft: true` and is
  `check`-clean; (c) a no-flag `new post` is byte-unchanged (no `draft:`), guarding the default.
- **`corpus/scaffold/posts/my-paper/index.tmd`** updated to the new bytes → `cargo test -p
  taliesin-core` renders + lints it (front-matter clean, no unknown keys, xrefs resolve, invariants
  hold). This is the render-level pin.
- **Full gate:** `cargo test -p taliesin-core -p taliesin-server`, `cargo fmt --check`,
  `cargo clippy -p taliesin-server --all-targets -- -D warnings`.
- **Integration smoke:** `new paper` + `taliesin check` on the result → 0 problems; `new post
  --draft` → `draft: true` present + check-clean; `serve` a scaffolded `init` dir to confirm the new
  index bullet renders.

## Non-goals

- **`new deck --tour` is deferred** to a documented DX10-followup. A teaching scaffold must be
  exemplary, and the columns-on-a-slide idiom (`layout-ncol` for prose, not reveal's `.columns`
  which silently degrades) is unverified in a *deck* context and intersects the still-pending **DX5**
  (accept `.columns` as a `layout-ncol` alias). Landing a shaky column demo would teach the wrong
  thing; it will land cleanly once DX5 settles the columns story. Fragments/incremental/magic-move/
  notes are already demonstrated in `corpus/deck.tmd`, which the docs reference.
- No change to `check`/render behavior — DX10 is scaffold *content* + one option flag.
- No kernel requirement introduced: the paper's `{python}` cell renders as source and stays
  `check`-clean without one (it runs, and teaches, once the user has a kernel).

## Invariant safety

Scaffold-content + one flag only. HTML-only, block model, single-editing-surface, and the
`MAX_WARM_PAGES`/`exec_pool.rs` freeze are untouched. Default-off `--draft` keeps every existing
scaffold and the `corpus/scaffold/` mirror byte-identical.
