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

### Carriage returns and stream chunking, and why they are in scope

A tqdm bar is not newline-delimited output. It is a chunk beginning with `\r` that overwrites the
current line. `render_outputs` emits one `<pre>` per `Output::Stream`, so a 100-update bar renders
as 100 stacked bars. **This is already true of the final render today**, so streaming it naively
would ship a live view that disagrees with the block that replaces it.

`LiveOutputs` is the single definition of the rule, and `collapse_carriage_returns` is a fold over
it, so the streamed view and the authoritative block cannot drift apart. Two parts:

1. **Consecutive chunks of the same stream merge into one output.** Where the kernel cut stdout
   into messages is an artefact of buffering and timing, not a property of the document, so a
   `<pre>` per message made the emitted HTML depend on that chunking and rendered a printing loop
   as a stack of boxes. stdout and stderr never merge with each other, and a rich output breaks
   the run.
2. **Terminal `\r` semantics apply within the merged text:** `\r` returns the cursor to column 0,
   so what follows replaces the current line; a line committed by `\n` is untouched.

The live path emits `op: "replace_last"` when a chunk merged into the previous output and
`op: "append"` when it started a new one.

**As-built note.** Part 1 was not in the original design, which specified `\r`-only collapsing and
an identity guarantee for everything else. It was taken after the browser check showed a streamed
log rendering as a stack of boxes, and only after **measuring** that merging drifts nothing:
across the whole corpus and every snapshot test, the sole failure was the identity guard written
for the narrower rule. The guard was replaced by a test of the rule that actually holds.

**Two testing notes that cost a round each.** The live-vs-final invariant compares two callers of
the *same* code, so it pins drift but not semantics: a mutant survived it and was killed only by a
written-out expectation. And "is it streaming" cannot be asserted by message order, because `done`
is emitted after `exec_cell` returns and a single end-of-cell flush still precedes it; the
assertion is that appends are **spread out in time** (202 µs under a batching mutant, seconds when
streaming).

### Why this cannot corrupt anything

The streamed HTML is transient. The authoritative output still arrives as a normal
`BlockOp::Update` through the diff, and the freeze cache still stores only the final
`render_outputs`. Streaming touches neither the block model nor the cumulative-hash cache keying,
so `data-block-id` / `data-sourcepos` and freeze correctness are untouched. It is preview-only:
`build` has no websocket and emits nothing new.

## Pins, and why none of them is a corpus document

**As built, no corpus document was added, and that is the right answer rather than a shortcut.**
The walker renders every corpus doc on every `cargo test` but does **not execute cells**, so a
corpus pin would pay the render cost while exercising none of this. Execution-dependent pins live
in `crates/server/tests/` against a temp-dir fixture, which is the pattern
`executed_output_reproducible.rs` already established; `progress_bar_collapses.rs` follows it and
builds a real document through a real kernel. The caps are unit-tested against tiny budgets, and
proved against a live kernel with a 2 s budget rather than the shipped 600 s one.

## Testing

| Claim | How it is pinned |
|---|---|
| Silence cap fires; wall-clock does not by default | `the_default_cap_is_silence_not_wall_clock` and siblings, on the pure `cell_budget` |
| `TALIESIN_CELL_TIMEOUT=120` reproduces today | `setting_the_wall_clock_cap_reproduces_the_old_default_exactly` |
| The budget resets on output, against a live kernel | `a_chatty_cell_outlives_the_silence_budget_and_a_quiet_one_does_not` (2 s budget) |
| Merging and `\r` semantics | `consecutive_chunks_of_one_stream_become_one_output`, `a_carriage_return_overwrites_the_current_line`, `a_progress_bar_arriving_in_chunks_collapses_across_outputs` |
| Live stream and final render agree | `the_live_stream_and_the_final_render_agree` (drift only, **not** semantics: both sides call the same code) |
| A real kernel streams *during* the cell | `a_running_cell_streams_its_output_before_it_finishes`, asserting appends are spread in time |
| A bar collapses through a real build | `crates/server/tests/progress_bar_collapses.rs` |
| Message shape | `cell_output_append_carries_the_op_and_the_rendered_fragment` |
| The client DOM path | **Nothing.** Browser-verified by hand; see the DETECTION-DEBT row |

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
