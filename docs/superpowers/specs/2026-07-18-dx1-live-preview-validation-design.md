# DX1 — Live validation on both preview paths

Date: 2026-07-18. Backlog item **DX1** (§6 DX audit batch, the dominant finding). Branch
`dx1-live-preview-validation`. Detail source: `notes/2026-07-18-dx-audit.md`.

## Goal

Make the located, "did-you-mean"-carrying static validators run in the **live preview**, the
surface where authoring actually happens. Today they run in `build`/`check`/`publish` but not
on either serve path, so every persona in the DX audit shipped a broken doc (dead `@fig-`
ref, wrong image filename, wrong post slug) that looked fine in preview and only surfaced at
`publish --strict` "minutes after they thought they were done." This closes the
preview→publish validation cliff.

## Ground truth (grepped against source 2026-07-18, before pricing)

Per the backlog's own law — *grep the named symbol first; the audit's S/M/L are guesses* —
the queued framing was materially stale. What the code actually shows:

- **Single-doc `serve`** ([`serve/mod.rs::rebuild`, ~L1229-1258](../../../crates/server/src/serve/mod.rs))
  already pushes all render-pass warnings **and** `validate_xrefs(&blocks)` into the dev menu,
  clickable. It is missing only `page_static_diagnostics` (broken local links, missing
  images/assets/media, dup heading ids, dangling anchors, a11y, math, code-langs,
  citation-without-bib).
- **The red-dot "audit" badge already exists.** [`client.js:91-101`](../../../web-client/client.js)
  reflects a live issue **count** (amber) on the collapsed `◇</>` button;
  [`client.js:40-42`](../../../web-client/client.js) reddens it for error-level diagnostics; the
  in-menu list is already click-to-source. New diagnostics flow into it with **no client change**.
- **Cross-page link checking is cheap.** DX-audit item #2 measured a whole-site re-derive at
  **~27 ms** on the largest book. A debounced full re-run needs no incremental machinery.
- **Site `serve_site`** ([`serve_site/mod.rs::rebuild_page`, ~L933-1003](../../../crates/server/src/serve_site/mod.rs))
  surfaces `finish_blocks` warnings + kernel/front-matter only. It runs **neither**
  `page_static_diagnostics(InSite)` **nor** `validate_cross_page_links`. This is the real gap.
- **`_site.yml` warnings** are console-only + unlocated today
  ([`serve_site/mod.rs:135-137`](../../../crates/server/src/serve_site/mod.rs) → `log::warn`).

Net: DX1 is **not** "build validation + a badge." It is *converge the two serve paths onto
the validator set that `check.rs` already defines*, feeding a badge that already exists. Effort
is S–M, dominated by the site path.

## Guiding principle

`page_static_diagnostics` ([`check.rs:112`](../../../crates/server/src/check.rs)) is, by its own
doc comment, "the single definition of the superset, so `check`, `build --strict` and `publish`
cannot drift on what counts as a defect." DX1 makes the **serve paths call the same
definitions**, so preview stops being the one surface that drifts. **No new validators are
written** — pure wiring plus two thin, testable helpers. The site wiring mirrors
`check.rs::collect_site_diagnostics`; the single-doc wiring mirrors `collect_file_diagnostics`.

## Resolved decisions (owner, 2026-07-18)

1. **Scope = minimal real gap.** Wire the static set onto both serve paths; add cross-page link
   checking + located `_site.yml` warnings on the site path; **reuse the existing badge as-is**.
2. **Cross-page links: current page only.** Each page rebuild runs the whole-site validation but
   refreshes only *that* page's cross-page diagnostics (filter results to its `rel`). Known,
   accepted limitation: breaking page B's *incoming* link by editing page A refreshes when B
   next rebuilds; `build`/`check`/`publish` remain the backstop. Covers the dominant persona
   failure (a page's own *outgoing* links) exactly.
3. **`_site.yml` warnings appear on every page's dev menu** (they are site-global; each page's
   menu is the only place to surface them).

## Changes

### A. Single-doc `serve` (`crates/server/src/serve/mod.rs`)

In `rebuild`, compute static diagnostics on **pre-execution** `doc.blocks` — *before*
`let blocks = executor.run(doc.blocks).await` consumes it. (Static lints must run pre-exec, as
`check` does: a cell-spliced matplotlib figure linted for alt-text would report a defect the
author cannot fix in source.) Convert `Warning → Diagnostic` with the existing `.at(file, line)`
mapping already used for the xref loop, and push into `diags`.

Two validators in the set (`citations_without_bibliography`, `bare_citation_key_not_rendered`)
need the raw source string. `render_doc` reads the file internally and does not currently return
`src`, so the wiring must obtain it — either read the file once in `rebuild` (as
`collect_file_diagnostics` does) or thread `src` out of the render step. `base` is
`app.path.parent()`. Resolving which is a planning detail; both are cheap.

### B. Site `serve_site` (`crates/server/src/serve_site/mod.rs`)

In `rebuild_page`, mirror `check.rs::collect_site_diagnostics`. Three additions:

- **Static set:** `page_static_diagnostics(&src, &doc.blocks, base, doc.format, Scope::InSite)`
  on pre-exec blocks (before L929 `exec.run`). `InSite` correctly omits `validate_local_links`
  (cross-page handles those) and we do **not** re-run `validate_xrefs` — `finish_blocks` already
  emits in-site broken-ref warnings, so adding it would double-count.
- **Cross-page links:** `app.site.lock().validate_cross_page_links()` (whole site, ~27 ms)
  **filtered to the current page's `rel`**, merged into that page's diagnostics. Reuses the
  existing site-lock scope already taken for `finish_blocks`.
- **`_site.yml` warnings:** read `site.warnings`, filter the missing-config advisory (via
  `taliesin_core::site::is_missing_config_warning`, exactly as `collect_site_diagnostics` does),
  surface as `_site.yml`-attributed diagnostics (file, no line — matching `check`) on each page's
  rebuild.

### C. Client / badge — no change

The collapsed `◇</>` button already shows amber + a live count for warnings and reddens for
errors, with an in-menu click-to-source list. New diagnostics use the existing
`protocol::diagnostics` channel unchanged.

## Testability & helpers

The server is a bin crate but carries `#[cfg(test)]` unit tests (e.g.
`serve_site/mod.rs` L1560). To make the assembly testable off the live socket, extract two
small pure helpers the `rebuild`/`rebuild_page` fns call:

- `preview_static_diagnostics(src, blocks, base, format, scope) -> Vec<Diagnostic>` — wraps
  `page_static_diagnostics` + the `Warning → Diagnostic` conversion.
- `preview_cross_page_diagnostics(&site, page_rel) -> Vec<Diagnostic>` — wraps
  `validate_cross_page_links()` filtered to `page_rel`.

These are the only structural additions; the serve fns become one call each.

## Testing (TDD, write the failing test first)

- **Negative fixtures under `crates/server/tests/`** (not `corpus/` — a deliberately-broken doc
  would break the "corpus renders clean" invariant): (1) a single doc with a missing image +
  dangling `@fig-`, (2) a mini-site with a broken cross-page link and an unknown `_site.yml`
  key. Assert each helper flags them, and that a clean doc/site yields **none**.
- **Positive corpus pin:** the capability ships pinned by a clean corpus doc/site exercising the
  path (corpus-plus-roadmap rule). Prefer extending an existing multi-page corpus site if a
  suitable one exists; decide during planning.
- **Full verification:** `cargo test -p taliesin-core -p taliesin-server`, `cargo fmt --check`,
  `clippy --all-targets -- -D warnings`, and a browser check via chrome-devtools that a typo'd
  image/ref lights the badge + a clickable dev-menu row on both `preview <file>` and
  `preview <dir>`. Mutation-check each new test (mutate the wired call → watch the named test
  fail → revert).

## Non-goals (explicitly out of scope)

- **DX5** — unknown `:::` div/theorem-class "did you mean" (separate backlog item).
- **Incremental** cross-page infra — the 27 ms full re-run is fine.
- **Line-locating** `_site.yml` warnings — `check` itself does not; we match it (file, no line).
- **Broadcast-to-all-pages** cross-page refresh — superseded by decision #2 (current page only).

## Invariant safety

No new output format, no CDN, no preview write-back, `--tali-*`/existing channels only. The
`data-block-id` + `data-sourcepos` block model and the `MAX_WARM_PAGES` + `exec_pool.rs`
eviction freeze are untouched — this only reads already-rendered blocks and pushes onto the
existing `protocol::diagnostics` transport.
