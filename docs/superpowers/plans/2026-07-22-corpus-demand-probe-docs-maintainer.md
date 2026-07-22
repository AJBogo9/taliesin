# Corpus demand-probe — OSS docs-maintainer persona Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a realistic in-scope "OSS docs maintainer" project (`corpus/tarn/`, the documentation site for a small illustrative dataframe library) that stacks the feature interactions the current corpus never combines — tabsets × Cmd-K full-text search × an API-reference part × guide→reference cross-page links — while mining and logging every point of resistance, then pinning what works and exhibiting it as the second marketing-site gallery card.

**Architecture:** `corpus/tarn/` is a book project (`_site.yml` with `chapters:` + two `part:`s: Guide + Reference) of a preface + five chapters. Authoring is the probe: each resistance point becomes a categorized finding in a dated notes doc. The parts that render cleanly are locked by a new `crates/core/tests/tarn.rs` pin test (modeled on `course.rs` + the existing mod.rs search-index tests) and mounted into `site/` under `/gallery/tarn` as a second `site/gallery.tmd` card. **No engine/crate source changes** — this pass authors documents, writes a test, edits site config, and records findings. Stacks on the pilot branch so the gallery accumulates.

**Tech Stack:** Taliesin `.tmd` (comrak-flavored Markdown + `:::` divs incl. `.panel-tabset` + `@ref` crossrefs + `.tmd#anchor` link rewrite), the `taliesin` CLI (`build`/`check`/`preview`/`read`), Rust integration tests (`cargo test -p taliesin-core`), chrome-devtools MCP via the `preview` skill. **No `{python}`/`{r}`/`{js}` executable cells** — static highlighted examples only (keeps this persona distinct from the analyst persona and sidesteps the open F-04 mount-exec gap).

## Global Constraints

- **Authoring + findings + test + site-config ONLY. No changes to `crates/` source** (the only permitted `crates/` file is the new `crates/core/tests/tarn.rs`). If a finding tempts an engine fix, STOP, log the finding, do NOT fix it here.
- **Do-NOT-touch (untouched by construction):** `MAX_WARM_PAGES` + LRU eviction in `serve_site/exec_pool.rs`; the single-editing-surface invariant.
- **Respect the identity:** every element is in-scope for an HTML-only, single-author, computational-document tool. A wall that is a settled non-goal (LaTeX/Word/ePub export, versioned-docs switcher, multi-language i18n, collaboration) is logged `correctly-refused`, never a gap.
- **Offline / no-CDN / `--tali-*` tokens only** for any `site/` touch, and for the `.svg` figure use named colors only (no hex — the banned-hex corpus test).
- **Corpus guards stay green:** the new docs pass `every_corpus_doc_has_clean_front_matter` and the clean-vocabulary guard (only Taliesin's own vocabulary).
- **Branch:** all work lands on `feat/corpus-demand-probe-docs` (cut off the pilot branch `worktree-corpus-demand-probe`, in worktree `.claude/worktrees/corpus-demand-probe-docs`). Do NOT move `main`. Do NOT push (the author pushes).
- **Illustrative library:** "Tarn" is a coined, fictional dataframe/query library used to demonstrate documentation features; it must not impersonate a real product (generic name, generic API, no real org/branding). The docs body reads as genuine library docs; the corpus README + findings doc record that it is purpose-built.
- **Findings doc:** `notes/2026-07-22-corpus-demand-probe-docs-maintainer.md`. Every finding: title · category ∈ {`gap`,`friction`,`interaction-bug`,`correctly-refused`} · severity P1–P3 · *wanted* → *happened* · minimal repro · disposition.
- **Fixed book structure (so pin-test numbers/anchors are deterministic):** `index.tmd` = unnumbered preface; `install.tmd` = **Ch 1**; `quickstart.tmd` = **Ch 2**; `concepts.tmd` = **Ch 3**; `api-frame.tmd` = **Ch 4**; `api-query.tmd` = **Ch 5**. Anchors: concepts `#sec-lazy` (a subsection); reference entries `#fn-select`,`#fn-filter`,`#fn-groupby` (Ch 4) and `#fn-col`,`#fn-lit` (Ch 5). Cross-page: quickstart links `[Frame.filter](api-frame.tmd#fn-filter)` and refs `@sec-lazy`; api-query's deprecation callout links `(api-frame.tmd#fn-filter)`.
- **Verification commands** (the binary is `target/debug/taliesin`; no kernel needed):
  - Invariants + pin: `cargo test -p taliesin-core`
  - Build standalone: `./target/debug/taliesin build corpus/tarn --out /tmp/tarn-out`
  - Static lint: `./target/debug/taliesin check corpus/tarn`
  - Machine view (retro refinement — run every persona): `./target/debug/taliesin read corpus/tarn/api-frame.tmd`
  - Live/visual: the `preview` skill (chrome-devtools) at three viewports (≈390×844, ≈1440×900, ≈900×1440), light **and** dark, **including the mounted `/gallery/tarn` exhibit** (retro refinement).

## File structure

- Create `corpus/tarn/_site.yml` — book manifest (`chapters:` with Guide + Reference parts, `toc: true`).
- Create `corpus/tarn/index.tmd` — unnumbered preface / overview.
- Create `corpus/tarn/install.tmd` — Ch 1: two tabsets (package-manager + per-OS CLI).
- Create `corpus/tarn/quickstart.tmd` — Ch 2: tutorial + code-walkthrough + per-language usage tabs + guide→reference cross-links + `@sec-lazy` ref.
- Create `corpus/tarn/concepts.tmd` — Ch 3: data-model page + SVG diagram + version-note callouts + `#sec-lazy` anchor.
- Create `corpus/tarn/api-frame.tmd` — Ch 4: `Frame` API entries (`#fn-select/#fn-filter/#fn-groupby`).
- Create `corpus/tarn/api-query.tmd` — Ch 5: query functions (`#fn-col/#fn-lit`) + a deprecation callout linking cross-page to `#fn-filter`.
- Create `corpus/tarn/dataflow.svg` — one tiny hand-written SVG (named colors, no external refs).
- Create `crates/core/tests/tarn.rs` — the interaction pin test.
- Modify `site/_site.yml` — add a second `mounts:` entry (`gallery/tarn`).
- Modify `site/gallery.tmd` — add the second exhibit card.
- Create `notes/2026-07-22-corpus-demand-probe-docs-maintainer.md` — findings doc.
- Modify `corpus/README.md` — add the `tarn/` row.
- Modify `notes/backlog.md` — fold actionable findings (Task 8).

---

### Task 1: Scaffold the Tarn book skeleton + findings doc (green baseline)

**Files:** Create all `corpus/tarn/*.tmd` as stubs + `_site.yml` + the findings-doc skeleton.

**Interfaces:** Produces the `corpus/tarn/` book that `Site::discover(&corpus_dir().join("tarn"))` (Task 6) and the `mounts:` entry (Task 7) depend on.

- [ ] **Step 1: Book manifest** `corpus/tarn/_site.yml`:

```yaml
title: "Tarn: the tabular query library"
toc: true

chapters:                 # presence makes this a book
  - index.tmd
  - part: "Guide"
    chapters:
      - install.tmd
      - quickstart.tmd
      - concepts.tmd
  - part: "Reference"
    chapters:
      - api-frame.tmd
      - api-query.tmd
```

- [ ] **Step 2: Stub chapters** (one heading + one sentence each) so the book renders end-to-end before real content. `index.tmd` = `# Tarn {.unnumbered}`; `install.tmd` = `# Installation {#sec-install}`; `quickstart.tmd` = `# Quickstart {#sec-quickstart}`; `concepts.tmd` = `# Concepts {#sec-concepts}`; `api-frame.tmd` = `# The Frame type {#sec-api-frame}`; `api-query.tmd` = `# Query functions {#sec-api-query}`.

- [ ] **Step 3: Findings-doc skeleton** `notes/2026-07-22-corpus-demand-probe-docs-maintainer.md` (same header shape as the pilot's findings doc; Persona = "solo maintainer of a small OSS dataframe library, hosting its docs as a Taliesin book").

- [ ] **Step 4: Verify skeleton builds + stays green.** `./target/debug/taliesin build corpus/tarn --out /tmp/tarn-out && cargo test -p taliesin-core`. Expected: build succeeds; corpus suite PASSES (front-matter + block invariants now include `corpus/tarn/`). If front matter warns, a key is wrong — fix the manifest, don't add a legacy key.

- [ ] **Step 5: Commit.** `git add corpus/tarn notes/2026-07-22-corpus-demand-probe-docs-maintainer.md && git commit -m "test(corpus): scaffold tarn docs-site book skeleton + findings doc"`

---

### Task 2: Ch 1 — Installation (two tabsets: package-manager + per-OS)

**Files:** Modify `corpus/tarn/install.tmd`.

**Interfaces:** Produces the tabset markup + distinctive install strings the search-index + tabset pins (Task 6) read.

- [ ] **Step 1: Author the page** with TWO `.panel-tabset` blocks. Keep distinctive, greppable commands in EACH tab (so the pin can prove non-default-tab content is present + indexed). Skeleton:

```markdown
# Installation {#sec-install}

<!-- one paragraph: Tarn ships as a Python library and a standalone CLI. -->

## The library

::: {.panel-tabset}

## pip

```bash
pip install tarn
```

## conda

```bash
conda install -c conda-forge tarn
```

## uv

```bash
uv add tarn
```

:::

## The command-line tool

::: {.panel-tabset}

## macOS

```bash
brew install tarn
```

## Linux

```bash
curl -LsSf https://example.invalid/tarn/install.sh | sh
```

## Windows

```powershell
scoop install tarn
```

:::

::: {.callout-note}
Tarn needs Python 3.10 or newer. The CLI bundles its own runtime.
:::
```

- [ ] **Step 2: Build + lint + browser-verify, logging findings.** `./target/debug/taliesin build corpus/tarn --out /tmp/tarn-out && ./target/debug/taliesin check corpus/tarn`. Preview via the `preview` skill: confirm both tabsets render as ARIA tabs (`role="tab"`/`role="tabpanel"`), clicking a tab swaps panels, default tab shows, all light+dark, three viewports. Confirm **two tabsets on one page** get distinct tab/panel ids (no collision). Log any resistance (e.g. `curl` URL flagged by a link checker, tab id collision, a11y gap) as `F-NN`.

- [ ] **Step 3: Probe the search index (interaction).** After the build, inspect the emitted index for the non-default tab content: `grep -o 'conda install[^"]*' /tmp/tarn-out/search-index.js | head` (or read `corpus/tarn`'s `Site.search_index_json` mentally via the Task-6 test). Record whether tabset-nested, non-default-tab content is searchable — `works` or a `gap`/`friction` finding either way.

- [ ] **Step 4: Commit.** `git add corpus/tarn/install.tmd notes/2026-07-22-corpus-demand-probe-docs-maintainer.md && git commit -m "feat(corpus): tarn ch1 (install) dual tabsets + probe findings"`

---

### Task 3: Ch 2 — Quickstart (walkthrough + usage tabs + guide→reference cross-links)

**Files:** Modify `corpus/tarn/quickstart.tmd`.

**Interfaces:** Consumes `#sec-lazy` (Task 4) + `#fn-filter`/`#fn-select` (Task 5). Produces the cross-page link + `@sec-` ref the pin (Task 6) reads.

- [ ] **Step 1: Author the tutorial** — a short "load → filter → aggregate" story with (a) a `.code-walkthrough` of the core query, (b) a per-language usage `.panel-tabset` (Python / CLI), (c) a cross-page markdown link into the API reference, (d) a `@sec-lazy` cross-page xref into Concepts. Skeleton:

```markdown
# Quickstart {#sec-quickstart}

<!-- one paragraph framing the worked example: a CSV of sales rows. -->

The core of Tarn is a lazy query you build up and then run. Here it is, line by line:

::: {.code-walkthrough}
```python
frame = tarn.read_csv("sales.csv")   # a lazy Frame, nothing read yet
recent = frame.filter(col("year") >= 2020)   # a predicate, still lazy
totals = recent.groupby("region").sum("revenue")   # the aggregation
result = totals.collect()            # run it: this is where work happens
```

::: {.step lines="1"}
`read_csv` returns a lazy [`Frame`](api-frame.tmd#sec-api-frame) — no file is touched yet.
:::

::: {.step lines="2"}
`filter` narrows rows with a predicate built from [`col`](api-query.tmd#fn-col); see [`Frame.filter`](api-frame.tmd#fn-filter).
:::

::: {.step lines="3"}
`groupby(...).sum(...)` is the aggregation; see [`Frame.groupby`](api-frame.tmd#fn-groupby).
:::

::: {.step lines="4"}
Nothing runs until `collect()`. Why laziness matters is explained in @sec-lazy.
:::
:::

The same query from the command line:

::: {.panel-tabset}

## Python

```python
import tarn
from tarn import col

totals = (tarn.read_csv("sales.csv")
          .filter(col("year") >= 2020)
          .groupby("region").sum("revenue")
          .collect())
```

## CLI

```bash
tarn query sales.csv \
  --filter 'year >= 2020' \
  --groupby region --sum revenue
```

:::
```

- [ ] **Step 2: Build + browser-verify.** Build + `check`. Confirm: the walkthrough panel sticks and steps focus their lines; the usage tabset works; `[Frame.filter](api-frame.tmd#fn-filter)` renders as `href="api-frame.html#fn-filter"` (`.tmd`→`.html` rewrite, anchor preserved); `@sec-lazy` resolves cross-page (record the exact rendered text — "Section 3.x" or similar). Log findings. **This is the headline docs interaction** (a guide page linking into a reference page + a walkthrough whose prose links out) — log any breakage as `interaction-bug`.

- [ ] **Step 3: Commit.** `git add corpus/tarn/quickstart.tmd notes/2026-07-22-corpus-demand-probe-docs-maintainer.md && git commit -m "feat(corpus): tarn ch2 (quickstart) walkthrough + usage tabs + guide→reference links"`

---

### Task 4: Ch 3 — Concepts (data model + SVG + version callouts + the ref target)

**Files:** Modify `corpus/tarn/concepts.tmd`; create `corpus/tarn/dataflow.svg`.

**Interfaces:** Consumes nothing. Produces `#sec-lazy` (referenced cross-page by Task 3).

- [ ] **Step 1: Author the page** — the data model (Frame / Column / lazy query), a figure, a version-note callout, and the `## Lazy evaluation {#sec-lazy}` subsection quickstart points at. Skeleton:

```markdown
# Concepts {#sec-concepts}

<!-- 2-3 paragraphs: a Frame is a set of named Columns; queries are lazy plans. -->

![How a Tarn query flows from source to collected result.](dataflow.svg){#fig-dataflow}

## The Frame {#sec-frame-concept}

<!-- prose on frames + columns. -->

::: {.callout-note title="Since v0.4"}
Frames are backed by Arrow arrays; zero-copy slices share memory with their parent.
:::

## Lazy evaluation {#sec-lazy}

<!-- prose: queries build a plan; `collect()` runs it. Reference @fig-dataflow. -->

::: {.callout-tip}
Chaining stays lazy: only `collect()` (or `write_csv()`) triggers execution.
:::
```

- [ ] **Step 2: Create `corpus/tarn/dataflow.svg`** — a tiny self-contained flow diagram (source → filter → groupby → collect), named colors only (e.g. `steelblue`/`slategray`/`seagreen`), `viewBox`, under ~35 lines, legible on dark.

- [ ] **Step 3: Build + browser-verify** (three viewports, light+dark): figure renders and is legible on dark; `@fig-dataflow` → "Figure 3.1"; both callouts render with the right icon/appearance; `#sec-lazy` exists as a heading anchor. Log findings.

- [ ] **Step 4: Commit.** `git add corpus/tarn/concepts.tmd corpus/tarn/dataflow.svg notes/2026-07-22-corpus-demand-probe-docs-maintainer.md && git commit -m "feat(corpus): tarn ch3 (concepts) data model + SVG + version callouts"`

---

### Task 5: Ch 4 + Ch 5 — API reference (anchored entries + cross-page deprecation)

**Files:** Modify `corpus/tarn/api-frame.tmd` + `corpus/tarn/api-query.tmd`.

**Interfaces:** Consumes `#sec-lazy` (Task 4). Produces `#fn-select/#fn-filter/#fn-groupby` (Ch 4) + `#fn-col/#fn-lit` (Ch 5), the cross-link + hover-index targets Task 6 reads.

- [ ] **Step 1: Author `api-frame.tmd`** — the `Frame` type reference. Each method is an anchored `###` entry with a signature, a parameter list, a returns line, and (where natural) a cross-ref. Skeleton:

```markdown
# The Frame type {#sec-api-frame}

A `Frame` is a lazy table. Methods return a new `Frame` (or a scalar) and never mutate in place; see @sec-lazy.

### `Frame.select` {#fn-select}

```python
Frame.select(*columns: str) -> Frame
```

Keep only the named columns.

- **columns** — one or more column names.
- **Returns** — a new `Frame` with just those columns.

### `Frame.filter` {#fn-filter}

```python
Frame.filter(predicate: Expr) -> Frame
```

Keep rows where `predicate` is true. Build predicates with [`col`](api-query.tmd#fn-col).

- **predicate** — a boolean `Expr`.
- **Returns** — a new `Frame` with the matching rows.

### `Frame.groupby` {#fn-groupby}

```python
Frame.groupby(*keys: str) -> GroupBy
```

Group rows by the key columns; follow with an aggregation such as `.sum(...)`.

- **keys** — the grouping columns.
- **Returns** — a `GroupBy` handle.
```

- [ ] **Step 2: Author `api-query.tmd`** — the expression helpers + a **deprecation callout** that links cross-page into `api-frame`. Skeleton:

```markdown
# Query functions {#sec-api-query}

Expressions (`Expr`) are the predicates and derived columns you pass to [`Frame.filter`](api-frame.tmd#fn-filter).

### `col` {#fn-col}

```python
col(name: str) -> Expr
```

Reference a column by name inside a query.

- **name** — the column to reference.
- **Returns** — an `Expr` you can compare and combine.

### `lit` {#fn-lit}

```python
lit(value) -> Expr
```

Wrap a Python scalar as an `Expr`.

::: {.callout-warning title="Deprecated since v0.5"}
`Frame.where()` is deprecated and will be removed in v1.0. Use [`Frame.filter`](api-frame.tmd#fn-filter) instead — it takes the same predicate.
:::
```

- [ ] **Step 3: Build + lint + machine-view probe.** Build + `check` (confirm ALL cross-page anchors — `#fn-filter`, `#fn-col` — validate; a broken one should be a `check` error). Then **run the `read` probe** (retro refinement): `./target/debug/taliesin read corpus/tarn/api-frame.tmd` and `./target/debug/taliesin read corpus/tarn/api-query.tmd`. Record how `read` projects tabsets (Task 2/3 pages), code signatures, param lists, and the deprecation callout — any lossy/noisy projection is a `gap`/`friction` finding (mirrors the pilot's F-03). Browser-verify the deprecation callout + cross-page link resolve.

- [ ] **Step 4: Commit.** `git add corpus/tarn/api-frame.tmd corpus/tarn/api-query.tmd notes/2026-07-22-corpus-demand-probe-docs-maintainer.md && git commit -m "feat(corpus): tarn ch4/ch5 API reference + cross-page deprecation + read probe"`

---

### Task 6: Pin test — `crates/core/tests/tarn.rs`

**Files:** Create `crates/core/tests/tarn.rs`.

**Interfaces:** Consumes `taliesin_core::Site` (`Site::discover`, `render_page`, `search_index_json`, `hover_index_json`), exactly as `corpus.rs`/`course.rs`/mod.rs tests use them.

- [ ] **Step 1: Write the pin test.** Assert the *new* interactions (tabset lowering, all-panels-present, search spanning pages incl. tabset-nested content, guide→reference cross-page link). Adjust exact strings to actual rendered output while keeping the structural checks. Starting content:

```rust
//! Interaction pin for the "OSS docs maintainer" demand-probe persona (corpus/tarn/).
//! Locks combinations no single-feature corpus doc exercises together: tabsets that
//! lower to ARIA tabs with every panel present (offline-complete + search-indexable),
//! a cross-PAGE guide→reference link that survives .tmd→.html rewrite, and a full-text
//! search index that spans guide + reference pages including tabset-nested content.
//! See notes/2026-07-22-corpus-demand-probe-docs-maintainer.md for the findings.

mod common;
use common::corpus_dir;
use taliesin_core::Site;

fn tarn() -> Site {
    Site::discover(&corpus_dir().join("tarn"))
}

#[test]
fn install_tabsets_lower_to_aria_tabs_with_every_panel_present() {
    let install = tarn().render_page("install.tmd").expect("install renders");
    assert!(
        install.contains("role=\"tablist\"") && install.contains("role=\"tabpanel\""),
        "tabsets lower to ARIA tabs: {install}"
    );
    // Non-default tab content is in the built HTML (offline-complete): a command from a
    // second tab of each tabset is present even though its panel starts hidden.
    for needle in ["pip install tarn", "conda install", "brew install tarn", "scoop install tarn"] {
        assert!(install.contains(needle), "panel content `{needle}` present: {install}");
    }
}

#[test]
fn quickstart_links_cross_page_into_the_reference() {
    let qs = tarn().render_page("quickstart.tmd").expect("quickstart renders");
    // The guide links into a reference page: `.tmd#anchor` rewrites to `.html#anchor`.
    assert!(
        qs.contains("api-frame.html#fn-filter"),
        "guide→reference link rewrites .tmd→.html keeping the anchor: {qs}"
    );
}

#[test]
fn search_index_spans_guide_and_reference_including_tabset_content() {
    let idx = tarn().search_index_json;
    // The index covers both a guide page and a reference page.
    assert!(idx.contains("\"u\":\"quickstart.html\""), "guide page indexed: {idx}");
    assert!(idx.contains("\"u\":\"api-frame.html\""), "reference page indexed: {idx}");
    // Tabset-nested, non-default-tab content is searchable (its text is in a section body).
    assert!(idx.contains("conda install"), "tabset-panel content is indexed: {idx}");
    assert!(!idx.contains("</script"), "raw </script must be neutralized: {idx}");
}

#[test]
fn api_entries_are_hover_indexed_by_anchor() {
    let idx = tarn().hover_index_json;
    // API entries carry stable anchors used by cross-page links + hover previews.
    assert!(idx.contains("\"fn-filter\":\"") , "Frame.filter is hover-indexed: {idx}");
    assert!(idx.contains("\"fn-col\":\""), "col() is hover-indexed: {idx}");
}
```

- [ ] **Step 2: Run the pin test.** `cargo test -p taliesin-core --test tarn`. If a `contains` string differs from the real output (an href form, an nbsp variant, a hover-index key shape, whether the tab-label text is a heading), **print the page/index in the failing assertion, read the ACTUAL output, and adjust the assertion to match reality** — keep the structural intent. If a genuinely-wrong behavior blocks a green pin (e.g. tabset content is NOT indexed), that is a finding: log it and weaken that one assertion to a `// known gap: F-NN` documented check rather than asserting broken behavior.

- [ ] **Step 3: Run the whole core suite.** `cargo test -p taliesin-core`. Expected: PASS (generic invariants already cover `corpus/tarn/`; `tarn.rs` adds the interaction assertions).

- [ ] **Step 4: Commit.** `git add crates/core/tests/tarn.rs && git commit -m "test(corpus): tarn.rs pins tabsets × search-index × guide→reference cross-links"`

---

### Task 7: Gallery integration (second card + mount)

**Files:** Modify `site/_site.yml` + `site/gallery.tmd`.

**Interfaces:** Consumes the built `corpus/tarn/` book.

- [ ] **Step 1: Mount the docs site.** Edit `site/_site.yml`: add `gallery/tarn: ../corpus/tarn` to the existing `mounts:` block (below `gallery/course`). No nav change needed (the "Gallery" item already exists from the pilot).

- [ ] **Step 2: Add the second exhibit card** to `site/gallery.tmd` (below the course card), in-repo vocabulary + relative links, no CDN:

```markdown
## Tarn: a library documentation site

The docs for a small dataframe library. A numbered **book** split into a Guide and an
API **Reference**, with **tabbed** install and usage panels (per package-manager and
per-OS), a line-by-line **code walkthrough** of the core query, and full-text **Cmd-K
search** that spans every page — the whole shape of a real open-source docs site.

[Open the docs &rarr;](gallery/tarn/)
```

- [ ] **Step 3: Verify in preview** (mounts are native in `preview`). Preview `site`: confirm the Gallery page now shows TWO cards; "Open the docs" resolves to the mounted book at `/gallery/tarn/`; navigating in shows Guide/Reference parts, tabsets work, and Cmd-K search finds content across pages. Three viewports, light+dark. Log any mount findings (this is a probe of the `mounts:` path for a docs-book shape).

- [ ] **Step 4: Confirm the static-build story.** `./target/debug/taliesin build site --out /tmp/site-out && ./target/debug/taliesin build corpus/tarn --out /tmp/site-out/gallery/tarn`. Expected: both succeed; `/tmp/site-out/gallery/tarn/index.html` exists, is self-contained (offline), and its `search-index.js` is present. Note whether top-level `build site` auto-builds mounts (it mirrors the docs books; if not, that matches the pilot's observation, not a new finding).

- [ ] **Step 5: Commit.** `git add site/_site.yml site/gallery.tmd notes/2026-07-22-corpus-demand-probe-docs-maintainer.md && git commit -m "feat(site): gallery second card + mount the tarn docs at /gallery/tarn"`

---

### Task 8: Findings roll-up, backlog fold, README row, retro + final gate

**Files:** Modify `notes/2026-07-22-corpus-demand-probe-docs-maintainer.md` + `notes/backlog.md` + `corpus/README.md`.

- [ ] **Step 1: Fill the findings roll-up** (count per category; disposition each actionable one).

- [ ] **Step 2: Fold actionable findings into `notes/backlog.md`** as a new item (follow the file's item style; do not re-add shipped work). Leave `correctly-refused` in the findings doc only.

- [ ] **Step 3: Add the corpus README row** to the Documents table:

```markdown
| `tarn/` | Realistic library docs (demand-probe #2) | the documentation site for a small illustrative dataframe library: a **book** with Guide + API **Reference** parts, dual `.panel-tabset`s (package-manager + per-OS install, per-language usage), a `.code-walkthrough`, version/deprecation callouts, and **cross-page** guide→reference links — the first corpus doc to stack tabsets × full-text search × an API reference. Pinned by `tarn.rs`; the second marketing-site **gallery** exhibit (`/gallery/tarn`). See `notes/2026-07-22-corpus-demand-probe-docs-maintainer.md` | (purpose-built, demand-probe #2) |
```

- [ ] **Step 4: Write the persona retro** at the end of the findings doc: (a) did the recipe hold on a second, different persona? (b) overlap vs. fresh findings vs. the course; (c) go/no-go for persona 3 (interactive-explainer); (d) any slate adjustment.

- [ ] **Step 5: Final green gate.** `cargo test -p taliesin-core && ./target/debug/taliesin build corpus/tarn --out /tmp/tarn-out && ./target/debug/taliesin check corpus/tarn`. Then confirm **no `crates/` source changed** beyond the new test: `git diff worktree-corpus-demand-probe...HEAD --stat -- 'crates/**'` shows only `crates/core/tests/tarn.rs`.

- [ ] **Step 6: Commit.** `git add notes/2026-07-22-corpus-demand-probe-docs-maintainer.md notes/backlog.md corpus/README.md && git commit -m "docs(corpus): tarn persona findings roll-up, backlog fold, README row, retro"`

---

## Self-review notes

- **Spec coverage:** recipe §3 → Tasks 2–7 (author→build→check→read→browser→log) + Task 8 (roll-up/retro); persona cluster §4 (book + tabsets + search + mounts + walkthroughs + API-reference) → Tasks 1–5,7; automated pins §7 → Task 6 (tabset/cross-page/search assertions) + free invariants from Task 1; gallery §6 → Task 7; findings §8 → findings doc from Task 1 on; guardrails §9 → Global Constraints + Task 8 Step 5 no-`crates`-source gate; scaling §11 → this is persona 2 on its own branch.
- **No engine changes:** the only `crates/` file is the new integration test; Task 8 Step 5 asserts it via `git diff … -- 'crates/**'`.
- **Determinism:** chapter order + ids fixed in Global Constraints (install=1 … api-query=5); Task 6 Step 2 covers exact-string drift by matching actual rendered output for hrefs/keys while keeping structural checks.
- **Probe honesty:** Tasks 2/3/5/7 include explicit probe steps (search-index-of-tabset-content, guide→reference link, `read` projection of API/tabset blocks, mount of a docs book) expected to *generate* findings, not just pass.
- **Distinct from the course:** no executable cells; no theorems; the new stressors are tabsets (two per page), full-text search spanning pages, an API-reference part, and `.tmd#anchor` guide→reference links — none stacked together anywhere in the current corpus.
