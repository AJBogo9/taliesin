# live-edit-benchmark-harness: design

> Beyond Quarto, Wave 2 (Pillar II), the measurement half of old priority #7. Successor
> context in `BEYOND-QUARTO.md`. Approved 2026-06-24.

## Goal

Turn qmd-fast's live-edit moat from an unmeasured architectural bet into committed
evidence and a regression gate. Measure the deterministic differentiator (incremental
render to diff to DOM preservation) on a real corpus doc, through the exact core seam the
dev server uses, with no Jupyter kernel and no browser required in CI.

## Decisions (locked with the author 2026-06-24)

- **CI-safe core + live browser proof.** The committed bench measures the deterministic
  part (render, diff, payload, and DOM preservation at the diff level). The end-to-end
  browser proof (an open `<details>` survives an edit above it as the same DOM node) is
  captured live via chrome-devtools / the existing `tools/record-demo`, not as a committed
  automated test.
- No kernel exec path, no committed browser harness, no Quarto-on-PATH run. Those stay
  deferred per the roadmap.

## What it measures

The seam is `qmd_fast_core::render_document_with_includes(src, base)` to
`qmd_fast_core::diff_blocks(old, new)`, both pure core (no exec, no server, no kernel):

1. **Cold full render**: time to render the whole doc once (parse to block model to HTML).
   On a warm server this is also the per-edit render cost, since qmd-fast re-renders the
   whole doc and then diffs (it does not cache the parse).
2. **Warm edit**: render the edited source plus diff, after a deterministic edit to a
   paragraph near the top (above the cells and the collapsible callout).
3. **Emitted payload**: the serialized `BlockOp` set (the wire payload the server would
   send) in bytes, versus the full page body HTML in bytes. The incremental-update win is
   a tiny op set, not a whole page.
4. **DOM preservation at the diff level**: the collapsible (`<details>`) / cell blocks
   below the edit get a `SetMeta` op (sourcepos patched in place), never an `Update`
   (which would replace the element). `SetMeta` is exactly why the live node survives.

Honesty note baked into the report: cold and warm *render* time are similar (no parse
cache), so the win is not a render speedup. The win is no per-edit startup (warm server +
kernel), a roughly 1000x smaller payload, and preserved DOM state, none of which Quarto's
cold-pass-plus-full-reload model can match.

## Architecture

A new `tools/live-edit-bench` Rust workspace crate, so no bench code ships in the
`qmd-fast` binary. It parallels the existing node `tools/record-demo` recorder.

### `tools/live-edit-bench/src/lib.rs` (the one source of truth)

```rust
pub struct LiveEditMetrics {
    pub doc: String,
    pub cold_render_ns: u128,
    pub warm_edit_ns: u128,        // render(edited) + diff
    pub diff_ns: u128,
    pub op_count: usize,
    pub set_meta_count: usize,     // DOM-preserving ops
    pub update_count: usize,       // DOM-replacing ops
    pub insert_count: usize,
    pub remove_count: usize,
    pub full_html_bytes: usize,    // doc.body_html().len(): what a reload re-sends
    pub edit_payload_bytes: usize, // the BlockOp wire payload
    pub dom_preserved: bool,       // a <details>/cell block below the edit is a SetMeta
}

/// Measure one live edit: render `src` cold, apply `edit` (a deterministic source
/// transform), render the edited source, diff. Pure measurement: never writes the
/// source file; reads only block ids / sourcepos / html.
pub fn measure_live_edit(
    src: &str,
    base: &std::path::Path,
    edit: impl Fn(&str) -> String,
) -> LiveEditMetrics;

/// The wire-payload size of one BlockOp (a faithful proxy of the server's JSON
/// message: the html plus ids/sourcepos plus a small envelope).
fn op_payload_bytes(op: &qmd_fast_core::BlockOp) -> usize;
```

The deterministic edit for the headline run inserts a new paragraph near the top of the
doc (above the collapsible callout), so every block below shifts its sourcepos and the
diff yields one `Insert` plus a run of `SetMeta`s. `dom_preserved` is true when the
collapsible / cell blocks below appear in `SetMeta` ops (not `Update`).

### `tools/live-edit-bench/src/main.rs` (emit artifacts)

Runs `measure_live_edit` on a real corpus doc (`corpus/posts/em-algorithm/index.qmd`,
which has `{python}` cells and a `collapse="true"` callout that renders as `<details>`)
with the standard edit, prints a markdown table to stdout, and writes `RESULTS.json`
(the raw metrics) next to the crate. The markdown and JSON are what the hero demo cites;
both are marked machine-indicative (absolute times vary by machine).

### `tools/live-edit-bench/tests/` (the CI-safe regression gate)

Asserts deterministic *invariants*, never absolute times:
- On a small synthetic doc (a paragraph above a `collapse` callout), a paragraph-above
  edit yields a `SetMeta` (not `Update`) for the callout block, so `dom_preserved` is
  true.
- The op set is small (a single content op plus position patches), not a whole-doc
  re-render.
- `edit_payload_bytes` is far smaller than `full_html_bytes` (assert a conservative
  ratio, e.g. payload < full_html / 10, with generous headroom).

## Relationship to `tools/record-demo`

`tools/record-demo` (node + Playwright) already records the live-edit beat to MP4/GIF and
already has a `live-edit` demo. This crate is the missing *measurement* half: it produces
the numbers, and the live browser proof reuses `record-demo` / chrome-devtools. The two
are complementary; this work does not touch `record-demo`.

## Out of scope (YAGNI / follow-ups)

- The Jupyter-kernel single-cell re-execution timing (deferred; would gate the bench on a
  kernel).
- A committed headless-browser assertion (the diff-level `SetMeta` assertion is the
  committed gate; the browser proof is captured live).
- A Quarto-on-PATH comparison run (the contrast is documented qualitatively, not executed;
  this is backlog #4's job, deliberately not rebuilt here).
- The hero demo itself (`live-edit-hero-demo`, the next Wave 2 item) consumes these numbers.

## Invariant safety

Pure measurement. The bench edits an in-memory copy of the source, never the file on disk;
it reads only `data-block-id` / `sourcepos` / `html`; nothing in the render / diff / exec
path changes. The new crate is a workspace member with no effect on the shipped binary.

## Testing

- The regression gate (the `tests/` above) is the deterministic core: it proves the
  DOM-preserving `SetMeta` behavior and the payload ratio hold, independent of timing.
- A unit test for `op_payload_bytes` confirms it sums the html plus a bounded envelope.
- The binary is smoke-run during the build (`cargo run -p live-edit-bench`) to confirm it
  emits a sane markdown table + valid `RESULTS.json` for the real corpus doc.
