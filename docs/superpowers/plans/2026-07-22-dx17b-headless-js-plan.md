# DX17(b): headless `{js}` executed-output — implementation plan

Date: 2026-07-22
Status: **ready to build** (design approved; owner signed off on the new dep, "Build it").
Design source: `docs/superpowers/specs/2026-07-21-dx17-headless-executed-output-design.md`
(Phase 2 sketch, lines 190-211). Phase 1 (`read --run` for python/r) already shipped.

## Why this is its own session

Phase 2 adds `chromiumoxide` (a CDP browser-automation crate) + `futures` to an offline-minimal
tool, and its verification needs a real headless Chrome run. That is a heavy, one-time
architectural commitment plus a live-browser test loop — worth doing carefully in a focused
session, not bolted onto the end of a broad backlog sweep. Everything below is concrete enough
to execute directly.

## Scope (from the approved design)

`read --run` additionally drives a **local headless Chrome** to run `{js}` cells the way they
actually run — in a browser — and reports back per cell, **observation-only** (no input
feedback, no server re-run, no `_freeze` write for `{js}`; the CUT `js-kernel-rerun` reactive-VM
trap stays out). Gated + optional: no Chrome → `[js: skipped (chrome unavailable)]`, never a hard
failure. Offline holds (local `file://` page + local browser, no network).

## Steps

1. **Deps** (`crates/server/Cargo.toml`): `chromiumoxide = { version = "0.7", default-features =
   false, features = ["tokio-runtime"] }` + `futures = "0.3"`. Confirm it resolves against the
   workspace tokio/edition-2024 before writing code (`cargo build -p taliesin-server`); if it
   drags in an incompatible tungstenite/tokio, pin or reconsider. **This is the risk gate** —
   settle the dependency graph first.

2. **New module** `crates/server/src/headless_js.rs` (~150-200 lines), all behind a
   `chrome_available()` probe:
   - `chrome_path()` → the first of `$CHROME_PATH`, `google-chrome`, `chromium`, `chromium-browser`
     on `PATH` (mirror `tools/ui-audit/lib/browser.mjs`'s `DEFAULT_CHROME`). `None` → skipped.
   - `async fn observe_js_cells(page_html_path: &Path, cell_ids: &[String]) -> Vec<JsResult>`:
     launch headless Chrome (`--headless --no-sandbox --disable-gpu --disable-dev-shm-usage`,
     temp user-data-dir so it never collides with a dev/MCP Chrome), open the built page over
     `file://`, **wait on the existing settle signal `data-qmd-done`** (NOT `data-qmd-ran` — see
     the ui-audit harness lesson), then `evaluate` a small JS snippet per `{js}` cell node that
     returns `{ produced: bool, kind: "svg"|"canvas"|"error"|"empty", detail, w, h, error }`.
     Classify: an `<svg>`/`<canvas>` child → produced (`[js: produced, <svg 640×400>]`); a
     captured `.tali-js-error`/`qmd-js-error` → `[js error: <message>]`; nothing → empty.
   - Timeout policy: a per-page settle timeout (~10s, reuse `TALIESIN_CELL_TIMEOUT`-style env);
     on timeout report `[js: skipped (timed out)]`, never hang.

3. **Wire into `read --run`** (`crates/server/src/query.rs` / the `read.rs` structs from Phase 1):
   after the python/r exec + projection, if the doc has `{js}` cells AND `--run` was given,
   build the page HTML in memory (reuse the Phase-1 rendered `doc` → `render_doc_to_page`, or the
   on-disk build artifact), write it to a temp file, `block_on(observe_js_cells(...))` on the
   tokio runtime `--run` already spins, and fold the results into both the text projection
   (a `[js: …]` line after each `{js}` cell's source, mirroring the python/r arm in `text.rs`)
   and the JSON `cells` array (`kind: "js" | "js-error"`, `produced`, `detail`).

4. **Gating**: no Chrome / launch failure / timeout → each `{js}` cell reports
   `produced: false, kind: "skipped", detail: "chrome unavailable"` and the text line is
   `[js: skipped (chrome unavailable)]`. Python/r-only users never touch this path. A single
   startup probe, cached, so a Chrome-less run pays ~nothing.

5. **Corpus pin** `corpus/agent/executed-read-js.tmd`: a `{js}` Observable-Plot cell that
   produces an `<svg>`, plus a deliberately-throwing `{js}` cell. The corpus suite renders it
   parse-only (regression net); `corpus/README.md` gets a one-line entry.

6. **Kernel-free unit tests** (the real coverage, can't rot): the classifier
   (`classify_js_node`) is pure over a small DOM-shape struct — assert svg/canvas → produced,
   error node → `js error`, empty → empty. No Chrome needed to test the mapping.

7. **Chrome-gated integration test** `crates/server/tests/read_run_js.rs`: run
   `read --run --format json` over the corpus doc; assert the `{js}` cell reports `produced:
   true, kind: "js"` and the erroring one `kind: "js-error"`. **Skips without a system Chrome**;
   optionally hard-fail under a `TALIESIN_REQUIRE_CHROME` canary (mirror `TALIESIN_REQUIRE_KERNEL`)
   so a CI regression can't silently green it.

8. **Docs**: `docs/guide/reference/` (the `read`/agent surface) + scaffolded `AGENTS.md` gain a
   line: `read --run` now also reports whether a `{js}`/Observable-Plot chart produced, when a
   local Chrome is available.

## Invariants held

Read-only (never writes source). Observation-only (no reactive re-run, no `{js}` freeze write —
the reactive-VM trap stays out). Offline (local page + local browser, no network; the built page
already inlines all assets). `--run` opt-in; a Chrome-less environment degrades to a skip, never
a failure — so nothing about the existing contract changes for anyone without Chrome.

## Verification (this environment)

A system `google-chrome` is present (`/usr/bin/google-chrome`, `/opt/google/chrome/chrome`), and
`chromiumoxide` launches its own temp-profile instance, so it won't collide with a held
dev/MCP Chrome. Verify end-to-end: `read --run --format json corpus/agent/executed-read-js.tmd`
reports the Plot cell `produced: true`; then a `CHROME_PATH=/nonexistent` run reports
`skipped (chrome unavailable)` and still exits 0.
