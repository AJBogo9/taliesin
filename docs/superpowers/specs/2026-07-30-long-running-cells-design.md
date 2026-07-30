# Long-running cells: liveness caps and streaming output

Backlog item **175**, parts **(a)** and **(b)**. Branch `long-running-cells-2026-07-30`.

Parts (c) `#| checkpoint:` and (d) per-cell run/interrupt are **out of scope** here. (d) depends
on (b) and is filed as `FEATURE-IDEAS.md` idea 86 / item 175(d).

## The problem

Jupyter's daily-driver property for expensive work is "watch it run, then re-run only what you
choose". Taliesin has neither half. Two of the four gaps are addressed here:

- **(a)** A cell dies at 120 s of wall-clock. A 40-minute training cell is unusable until the
  author discovers `TALIESIN_CELL_TIMEOUT`.
- **(b)** A long cell shows a `⏳` elapsed badge and nothing else: no tqdm bar, no epoch log, no
  partial figure, until it completes.

## Correction to the item's filed text

The backlog says the silence tracker at `kernel.rs` "only warns with it" and asks that its role be
verified before speccing. Verified, and the filed description is wrong in one respect and
understates the machinery in another:

- It does **not** only warn. `kernel.rs:967-973` pushes an `Output::timeout` and `break`s, which is
  a real terminal cap.
- Its actual limitation is reachability: the `budget` match at `kernel.rs:921-928` consults
  `deadline` first and falls through to the silence branch **only when the wall-clock cap is
  disabled** (`TALIESIN_CELL_TIMEOUT=0`).

So (a) is not "build a silence cap". It is "swap which of two already-written caps is the default",
plus close the protection that swap would otherwise lose (below).

## Part (a): the cap rules

A pure silence cap would lose a protection the current code calls out by name: *"Total wall-clock
cap (not per-message, so a streaming runaway cell is still caught)"* (`kernel.rs:908`). A
`while True: print(x)` loop never goes silent, so silence alone never fires on it. Three rules keep
every protection that exists today and still remove the wall:

1. **Silence cap, always on, default 600 s.** No iopub message for ten minutes means the cell is
   wedged. A cell printing an epoch line every 30 s is never touched. Override:
   `TALIESIN_CELL_SILENCE` seconds, `0` disables.
2. **Runaway cap.** The loop already tracks `capped` (the `MAX_OUTPUTS` / `MAX_STREAM_BYTES` limits
   at `kernel.rs:903-907`). A cell that has blown its output budget and is *still* going is a
   runaway by definition, so a 120 s wall-clock deadline starts from that moment. This is what
   catches the infinite printer. In practice the existing `capped` branch already interrupts
   immediately, so this rule is the backstop for a kernel that ignores the SIGINT.
3. **`TALIESIN_CELL_TIMEOUT` keeps its exact current meaning** (hard wall-clock seconds) and
   changes only its default, from `120` to off. Setting `TALIESIN_CELL_TIMEOUT=120` reproduces
   today's behavior exactly, which keeps the change bisectable and gives the escape hatch a home.

This is one changed default and no new knob on the common path, per the minimal-config convention.

## Part (b): streaming output

### Transport

`ProgressSink` (`exec.rs:118`) is already `Option<Arc<dyn Fn(String) + Send + Sync>>`, and
`protocol.rs` holds typed message constructors with their own tests. A new message joins
`cell-state`:

```
{ "type": "cell-output-append", "page": ?string, "cell_id": string,
  "op": "append" | "replace_last", "html": string }
```

- **Kernel:** `execute_streaming(&mut self, code, on_output: impl FnMut(&Output))`; today's
  `execute(code)` becomes `execute_streaming(code, |_| {})`. Rather than calling back at each of
  the seven scattered `outputs.push` sites, the loop keeps a `streamed: usize` watermark and
  flushes `outputs[streamed..]` once at the end of each iteration and once after the loop. One
  rule, and it cannot miss a future push site.
- **Executor:** `exec_cell` passes a closure that renders the output with
  `render_outputs(std::slice::from_ref(&out))` (it already takes a slice) and emits the message.
- **Client:** a `case "cell-output-append"` appending into a live container inside the
  `{cell_id}-out` block.

### Carriage returns, and why they are in scope

A tqdm bar is not newline-delimited output. It is a chunk beginning with `\r` that overwrites the
current line. `render_outputs` emits one `<pre>` per `Output::Stream`, so a 100-update bar renders
as 100 stacked bars. **This is already true of the final render today**, so streaming it naively
would ship a live view that disagrees with the block that replaces it.

Fix both with one pure function, `collapse_carriage_returns(&[Output]) -> Vec<Output>`, applying
terminal semantics: within a run of consecutive same-stream `Output::Stream` values, a chunk
beginning with `\r` overwrites the current line. `render_outputs` calls it, and the live path
applies the same rule incrementally:

- merging **modified the last element** → `op: "replace_last"`
- merging **appended** → `op: "append"`

**The invariant that makes this safe is testable:** feeding a synthetic output sequence through the
streaming path and concatenating what the client would build must equal
`render_outputs(&collapse(raw))`. That is the pin, not a screenshot.

### Why this cannot corrupt anything

The streamed HTML is transient. The authoritative output still arrives as a normal
`BlockOp::Update` through the diff, and the freeze cache still stores only the final
`render_outputs`. Streaming touches neither the block model nor the cumulative-hash cache keying,
so `data-block-id` / `data-sourcepos` and freeze correctness are untouched. It is preview-only:
`build` has no websocket and emits nothing new.

## Corpus pin

The walker renders every corpus doc on every `cargo test`, so the pin must exercise streaming and
the caps **without being slow**: a `{python}` cell emitting about five lines at 50 ms sleeps plus
one `\r` progress bar, roughly 300 ms. The caps are unit-tested against tiny budgets via env var,
never through the corpus.

## Testing

| Claim | How it is pinned |
|---|---|
| Silence cap fires; wall-clock does not by default | Unit test on the budget computation with tiny budgets |
| `TALIESIN_CELL_TIMEOUT=120` reproduces today | Unit test asserting the wall-clock branch is taken |
| `\r` collapsing matches terminal semantics | Unit tests on `collapse_carriage_returns` |
| Live stream and final render agree | Invariant test: replayed appends == `render_outputs(&collapse(raw))` |
| A real kernel streams before completion | `TALIESIN_REQUIRE_KERNEL` test asserting an append arrives before the terminal `cell-state` |
| Message shape | `protocol.rs` unit test beside `cell_state_*` |

Every fix is verified by **mutation** (restore the bug, watch the named test fail), per the
standing rule.

## Known detection debt

`client.js` is deliberately one IIFE with no unit harness, so the append/replace DOM path is
**browser-verified by hand**, not asserted, the same gap items 161 and 162 hit. This earns a
DETECTION-DEBT row naming the harness change it would need, rather than a test that cannot fail.

Trap to avoid, from LESSONS: a whole-page `contains()` for `tali-stream` passes on any page,
because `base.css` is inlined into every page and contains that string. Needle the full emitted tag.

## Out of scope

- (c) `#| checkpoint:` and (d) per-cell run/interrupt.
- Any change to freeze keying, the block model, or build output.
- A reader-facing control for stream verbosity. Perfect the default first.
