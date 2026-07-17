# Reduction audit map (Phase 1 output)

> Produced 2026-07-17 by a 5-way read-only fan-out (render / site / cite+diagnostics /
> server / client+assets), then orchestrator-verified. Spec:
> `docs/superpowers/specs/2026-07-17-reduction-and-modularity-pass-design.md`.
> Plan: `docs/superpowers/plans/2026-07-17-reduction-audit-phase1.md`.
> **Analysis only. No code was changed. Owner reviews this before Phase 2/3.**

## Headline

**The codebase is already lean.** Five auditors swept the whole Rust + client-JS +
asset surface against the 71-doc corpus and whole-repo call sites. `crates/server`
built with zero warnings and zero `TODO`/`FIXME`; the only `#[allow(dead_code)]` in the
whole server crate is two frozen-zone accessors. Every user-facing *feature* has corpus
coverage. There is no pile of dead weight to cut.

That is itself the answer to the motivating question: the "reduction" yield is small,
and it strongly validates the "present benefit only, do not build the extension system
now" decision. There is almost nothing in core today that is "not useful" and wants
extracting. The extension system's real future job is to keep *new* breadth out of
core, not to externalize current bloat, because there isn't any.

The findings sort into: a small set of safe deletions (Phase 2), two consolidations and
two genuine tangle points (Phase 3, optional), and a larger set of **coverage gaps**
that are the *opposite* of reduction (they are "add a corpus pin", not "delete").

## Bucket totals (verified)

| Bucket | Count | Net effect |
|---|---|---|
| dead (safe to delete) | 3 | small deletions |
| dead (leave: frozen/test-only) | 1 | no action |
| redundant (consolidate) | 2 | modest dedup |
| tangled (Phase 3 candidates) | 2 | decouple if painful |
| coverage gap (add a corpus/unit pin) | 7 | not reduction |
| doc drift | 1 | docs-only |
| housekeeping (out of product scope) | 1 | owner decision |

---

## Phase 2 candidates: safe deletions (verified dead)

| # | Item | Location | Evidence | Confidence |
|---|---|---|---|---|
| D1 | **`about:` block** — `Page.about` field, `AboutSpec`, `about_html()`, `parse_about()`, the `about:` doc + accepted-key entry | `site/mod.rs:69,88,1163,1450`; `site/frontmatter.rs:19,60,97,114`; `frontmatter.rs:85` | No corpus doc sets `about:`; referenced by **no test**; superseded by `hero:` (tech_blog.rs pins the old `.tali-about*` markup is gone). Also remove `about:` from the accepted-key set (like the `image:` retirement precedent) and sweep any residual `.tali-about*` CSS. | H |
| D2 | **`TAL-MEDIA` catalog row** (`"local audio not found"`) | `diagnostics/codes.rs:65` | String exists only here; `media.rs` deliberately never validates `<audio>` (documented + guard test), so nothing produces it. One-row delete. | H |
| D3 | **`search_button(full: bool)` `full=true` branch** | `site/chrome.rs:34-53` | Both prod call sites (`:110` navbar, `:262` sidebar) pass `false`; only a unit test passes `true`. Drop the param (and its test) or wire the book sidebar to actually use `full=true` as its stale doc-comment claims. | M |

## Leave (dead but not worth touching)

| # | Item | Location | Why leave |
|---|---|---|---|
| L1 | `warm_pool::is_warm()`, `capacity()` | `warm_pool.rs:630-641` | `#[allow(dead_code)]`, used only by kernel-gated tests, inside the frozen exec zone. Risk > reward. |

## Phase 3 candidates: consolidations (verified redundant)

| # | Item | Location | Evidence | Confidence |
|---|---|---|---|---|
| R1 | **`llms.rs` re-derives text extraction** (`strip_katex`, `text_content`) instead of reusing `render::indexable_text` | `site/llms.rs:196,221` vs `render/text.rs:121` | `search.rs:169` already reuses `render::indexable_text` for the same job (with an explicit "do not re-derive it here" comment). Point `llms::page_prose` at it; add a test asserting output equivalence before deleting the twins. | M |
| R2 | **Two raw-source scanners** (`chapter_heading` vs `scan_page_anchors`) | `site/book.rs:180` vs `site/xref.rs:75` | Near-identical "skip front-matter, toggle code fence, walk headings" line-scanners, with **one real divergence**: `xref` resolves `{{< include >}}` first, `chapter_heading` does not. Factor the shared skeleton into one helper; decide deliberately whether chapter-title detection should resolve includes. | M |

## Phase 3 candidates: tangle points (decouple only if painful)

| # | Item | Location | Evidence | Confidence |
|---|---|---|---|---|
| T1 | **`PageParts` 17-field struct hand-built at 3 call sites** | `render/page.rs` + `serve/mod.rs:692` + `serve_site/mod.rs:668` | The render area's actual "core edits ripple outward" signature; a documented past title-drift bug (`page.rs:380-390`) came from these 3 drifting. A field addition must be mirrored 3 ways by hand. Candidate for a single constructor / builder so a field change is one edit. | H |
| T2 | **Three site/ modules each run their own raw-source pre-scan** | `site/xref.rs`, `site/book.rs`, `site/discovery.rs` | `xref`, `book`, and `discovery` each independently re-implement a slice of the include/parse pipeline over raw `.tmd` (see R2). A recurring pattern, not a single bug. If touched, unify on one shared pre-scan. | M |

## Deliberate duplications (leave — flagged only so a future edit updates both)

- **`toc-sheet.js` drag logic** duplicates the inline block in `client.js:744-855`. Necessary: a static build ships no `client.js`. A future drag-gesture tweak must touch both.
- **`client.js scanA11y()` mirrors `diagnostics/a11y.rs`.** Intentional: the kernel-free `check` CLI approximates the live browser audit. Keep the two check sets in sync.

## Coverage gaps (NOT reduction — add a corpus/unit pin, per the "corpus leads" policy)

These are load-bearing or real code paths with no corpus regression net. Listed because
the project's own rule is "every capability ships pinned by a corpus doc."

| # | Item | Location | Gap |
|---|---|---|---|
| C1 | `{{< embed >}}` shortcode | `render/extension/mod.rs:116,349` | Zero corpus coverage **and** zero unit tests, yet load-bearing (wired into `build.rs`, `site/discovery.rs`; used across `docs/guide`). Highest-priority gap. |
| C2 | `{{< video >}}` `dark=` / `poster=` args | `render/extension/mod.rs:129` | Params never exercised by any corpus doc. |
| C3 | `theme.rs` custom-`.css` / `_extensions/theme.css` branch | `render/theme.rs` | Unit-tested only; no corpus doc uses a custom theme. |
| C4 | `head:` / `body-start:` / `body-end:` config knobs | `site/config/mod.rs:44-47` | Real code path; corpus coverage dropped to test-only (tech-blog removed its usage). |
| C5 | `mounts:` config | `site/config/mod.rs:52-55` | Real usage is the non-corpus marketing `site/` + a CLI test; no `corpus/` pin. |
| C6 | `citation_*` (Google Scholar) meta | `site/meta.rs` | Synthetic-test only; no corpus doc has `date:`+`author:` under a `url:`-bearing site. |
| C7 | `render` / `blocks` CLI subcommands | `server/query.rs:18,138` | No black-box CLI integration test (covered only indirectly). Low risk. |

Sub-gaps also noted: `TAL-FM-KEY` ("unknown prose-lint key") and
`bare_citation_key_not_rendered` are wired + unit-tested but have no corpus demo;
`TAL-CATEGORY` (from `site/categories.rs`) lacks a deliberate-typo corpus fixture.

## Doc drift (docs-only fix)

- `docs/guide/reference/cli.tmd` command table is stale vs the real CLI: missing
  `mcp`, `map`, `vocab`, `read`, and the `paper` kind on the `new` row. (AGENTS.md /
  `--help` are correct; the guide table predates them.)

## Housekeeping (out of product scope — owner decision, do NOT auto-remove)

- Six `.claude/worktrees/agent-*` git worktrees at the repo root (dates 2026-07-17
  00:27-01:14), one on a named `docs-theorem-environments` branch. These may be
  session debris **or** live worktrees from concurrent sessions. Left untouched by the
  audit. If confirmed debris: `git worktree remove` each, then delete its branch.

---

## Recommended dispositions

- **Phase 2 (safe, do now, TDD-guarded):** D1 (`about:`), D2 (`TAL-MEDIA` row), D3
  (`search_button` branch), R1 (`llms` text-extraction reuse). Each guarded by the
  corpus suite; R1 additionally by an output-equivalence test.
- **Phase 3 (optional, only if it earns present benefit):** T1 (`PageParts`
  constructor) is the strongest single decoupling win. R2/T2 (unify site raw-source
  scanners) is a real but larger refactor with a behavioral decision inside it.
- **Not this pass:** the coverage gaps (C1-C7) are a separate "add corpus pins"
  initiative; the doc-drift and worktree items are one-liners for whenever.
- **Deck:** classified must-stay-native / mid-redesign; untouched, as specified.

---

## Execution outcome (2026-07-17, scope: Phase 2 + T1)

Plan: `docs/superpowers/plans/2026-07-17-reduction-phase2-3.md`. Branch
`reduction-modularity-pass`. One commit per item; workspace tests + clippy + fmt green;
tech-blog rebuilt and artifact-verified (hero intact, `tali-about`/`tali-search-full`
gone, search button present).

- **D2 done** (`1a47fb6`): dropped the orphaned `TAL-MEDIA` audio row.
- **D3 done** (`f57e5d4`): removed the unused `search_button(full=true)` variant + its dead CSS.
- **D1 done** (`dcf0588`): removed the `about:` block end-to-end. Wider than first scoped:
  it cascaded into `schema.rs`/`vocab.rs` (both derive from the frontmatter consts) and
  three regenerated golden assets (schema, vocab, repo-root + bundled AGENTS.md). `about:`
  now warns as an unknown key (the `image:`/`csl` retirement precedent).
- **R1 DEFERRED** (`90ef09f`): the equivalence gate FAILED — `text_content` decodes
  `&#8217;`/`&nbsp;`, `render::indexable_text` does not, so reusing it would leak raw
  entities into `llms.txt`. Divergence pinned by a passing test; aligning the two (which
  also changes the search index) is a separate call, out of this pass's scope.
- **T1 done** (`59c99b8`): added `PageParts::defaults()`; the three production assemblers
  use `..PageParts::defaults()`, so a new field is one edit. Page-assembly snapshots
  byte-identical.

Untouched, as deferred: R2/T2 (site raw-source scanners), the coverage gaps C1-C7, the
`cli.tmd` doc drift, and the stray `.claude/worktrees/agent-*` (owner's call).
