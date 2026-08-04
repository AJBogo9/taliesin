# Algorithm debug mode: `::: {.debug}`

Date: 2026-08-04. Status: design approved, not yet implemented.

## Problem

Taliesin's stated differentiator is interactive, follow-along code visualization. It does
not currently deliver it. The tool lowers the barrier for *charts* (Plot/d3/`num` plus
reactive inputs) and for *narrated* code (`.code-walkthrough`), but a stepped algorithm
view is built from scratch every time, and the algorithm itself has to be rewritten to
support it.

### Measured, in this repo

Two documents already implement this architecture by hand, with two incompatible APIs,
and neither shows which line is executing.

| Document | Cost | What it hand-rolls |
| --- | --- | --- |
| `corpus/graphics3d/sorting.tmd` | 210 lines | bespoke `rec.cmp/swap/set/touch` recorder, op trace, `stepOnce()` replay, select/range/checkbox/button bar, rAF loop |
| `corpus/tech-blog/posts/a-star/index.tmd` | 495 lines (453 in one cell) | manual `initSearch`/`stepOnce` state machine, `setTimeout` loop, a second button bar with different labels |

Specific failures that a native feature fixes:

1. **Neither has a line cursor.** No correspondence between the algorithm's source and
   the picture, which is the reader's central question ("where are we now?").
2. **`sorting.tmd` is forward-only.** No step-back, no scrub. Once a comparison passes,
   the reader cannot go back and look at it.
3. **`sorting.tmd` hardcodes colors** (`#0b0f1a`, `#9aa4b2`, `hsl(...)`), so it ignores
   the `--tali-*` theme system entirely.
4. **A\* could not be written as A\*.** It is rewritten as a hand-rolled continuation
   (`initSearch`/`stepOnce`/`s.done`, lines 222-283) because a visualization must pause
   mid-algorithm. That rewrite, not the drawing, is the real barrier.
5. Both rebuild the same transport controls with different labels and different
   behavior.

### What exists that this builds on

- `::: {.code-walkthrough}` (`divs.rs:627`): sticky code panel plus narration steps, line
  highlighting through `data-cw-lines`. The lines are hand-declared prose driven by
  scroll position. Nothing executes.
- `scrolly.js:28-31`: the active step pushes its state into a hidden `[data-tali-input]`,
  so a `{js}` cell re-runs through `//| input:`. This is the reusable spine.
- `{{< input >}}` (slider/number/checkbox/text/select), `{js}` cells with `//| input:`,
  `//| name:`, `tali.state`, plus the `Plot`/`d3`/`num` globals.
- `deck.js:2416-2420`: a guarded Fullscreen API toggle.

Note: an `animate` input type (play/pause/step/reset) was built and retired on
2026-08-03 (`validate.rs:76-84`) because it "had no use outside its own fixture". The
lesson is binding on this design: a frame pump must live inside a feature that uses it,
never as a free-standing control.

## Research basis

Two architectures dominate the field, and both replace the manual state machine:

1. **Debugger-hook trace.** Python Tutor and Runestone Codelens run code under
   `sys.settrace`, record `{line, event, globals, heap, stack_to_render}` per step, ship
   the trace as JSON, and the frontend is a scrubber over it. The author writes plain,
   uninstrumented code.
2. **Generator `yield` snapshots.** The JavaScript community standard: write `function*`,
   `yield` a snapshot at each interesting moment, the driver calls `.next()`. `yield*` in
   recursion gives step-into behavior for free.

Hundhausen's meta-study of 24 experiments found that *how* learners use a visualization
matters more than what it shows, and that more active involvement produced better
performance. That argues for reader-driven stepping over autoplay animation, and for the
JS adapter's re-runnable inputs.

LeetCode's consensus teaching order (Two Pointers, Sliding Window, Binary Search, BFS/DFS,
Backtracking, DP) maps onto exactly four data views: numeric array as bars, array or
string as boxes, 2-D array as a grid, and an integer that indexes a visible array as a
pointer caret. Nothing beyond those four is needed to cover the whole set.

## Design

### 1. Authoring surface

A `::: {.debug}` fenced div holding one traced code cell and, optionally, one `{js}` view
cell.

The zero-JS case produces the complete debugger with no author JavaScript at all:

    ::: {.debug name="sort"}
    ```{python}
    #| trace: true
    def bubble(a):
        for i in range(len(a)):
            for j in range(len(a) - i - 1):
                if a[j] > a[j + 1]:
                    a[j], a[j + 1] = a[j + 1], a[j]
        return a

    bubble([5, 2, 9, 1, 7, 3])
    ```
    :::

Taking over the picture adds a view cell and changes nothing else:

    ```{js}
    //| input: sort
    const f = tali.frame("sort");
    return myBars(f.locals.a, f.changed.a.writes);
    ```

JS capture runs client-side, so the reader can re-run with a different input. It sits
inside the same `::: {.debug}` div, replacing the Python cell:

    ::: {.debug name="sort"}
    ```{js}
    //| trace: true
    //| input: n
    function* bubble(a) {
      for (let i = 0; i < a.length; i++)
        for (let j = 0; j < a.length - i - 1; j++) {
          yield { a, i, j };
          if (a[j] > a[j+1]) [a[j], a[j+1]] = [a[j+1], a[j]];
        }
    }
    return bubble(shuffle(tali.value("n")));
    ```
    :::

`name=` is optional. Without it the block still renders and steps; it is required only to
address the block from a `{js}` view cell (same rule as `.scrolly`).

Exactly one traced cell per `.debug` div. The diagnostics are listed in section 9.

### 2. The frame contract

Both capture adapters produce the same array of frames. This is the single shared type,
and the reason a second adapter is cheap:

    Frame = {
      line:    number,   // 1-based, into the displayed cell source
      event:   "line" | "call" | "return" | "exception",
      depth:   number,   // call depth, drives the stack view
      func:    string,   // current function name
      locals:  { [name]: Value },
      changed: { [name]: ChangeInfo },
      stack:   [{ func, line }],   // innermost last
      stdout:  string    // cumulative
    }

    ChangeInfo (array)  = { reads: [index...], writes: [index...] }
    ChangeInfo (scalar) = { from: Value, to: Value }

`changed` is the field every hand-rolled version reinvented (`hi = {a, b}` in
`sorting.tmd`, `s.current` in the A\* post), and it is what makes compared bars glow and
DP cells flash without the author writing a line of code. The two halves are obtained
differently, and this distinction is load-bearing for the implementation:

- **`writes` come from diffing** consecutive locals snapshots. Purely mechanical, exact.
- **`reads` cannot be observed by line-granularity tracing at all.** They are derived by
  static analysis: the harness pre-parses each source line once, collects its `Subscript`
  nodes over names (`a[j]`, `dp[i][j]`), and at trace time resolves those indices from the
  current locals. When an index is not resolvable from a simple name or constant, that
  read is omitted rather than guessed. For the JS generator adapter `reads` is always
  empty unless the author yields it explicitly.

Frame ordering follows standard debugger semantics, matching Python Tutor: a `line` event
fires *before* that line executes, so `frame.line` is the line about to run and
`frame.changed` describes what the *previous* line did. The implementation must not invert
this.

### 3. Capture adapters

**Python, at build time, in the warm kernel.** A `sys.settrace` harness scoped to the
cell's own code object, so library frames are never recorded. Each line/call/return event
snapshots the current frame's locals plus the stack, serialized through a capped value
encoder. The result reaches the server on the kernel's output stream like any other cell
output, so it is cached by the existing `_freeze` cumulative hash with no change to
`freeze.rs`: an unchanged document replays its trace without booting a kernel.

Caps are constants, not configuration:

| Cap | Value | On exceeding |
| --- | --- | --- |
| frames | 5,000 | truncate, block shows "trace truncated at 5,000 steps" |
| items per container | 200 | elide, render as `[... 200 of 4096]` |
| nesting depth | 4 | render deeper values as a type summary |
| chars per value | 2,000 | truncate with an ellipsis |

**JavaScript, client-side, at cell run.** The cell returns a generator; `debug.js` drains
it into frames under the same caps. No server work and no kernel.

Line numbers for the JS adapter come from a build-time Rust scan that rewrites `yield X`
into `yield __at(N, X)`, skipping string literals, template literals, and comments.

**This scanner is the one genuinely risky piece of this design.** Mitigation: when the
scan cannot complete confidently it emits no line mapping at all, so the cursor simply
does not move and a located warning is raised. It never rewrites text it is unsure about,
so a mis-scan cannot corrupt the cell. Python is unaffected either way. If the scanner
proves troublesome in implementation, the accepted fallback is a Python-only line cursor
with JS frames carrying `line: null`; everything else in the feature is unchanged.

### 4. What the chrome provides

One new bundled asset pair, `assets/js/debug.js` and `assets/css/debug.css`.

- **Transport bar**: first, back, forward, last, play/pause, speed, a scrub range input,
  and a "step 47 / 312" readout. Keyboard when the block has focus: left/right to step,
  space to play/pause, Home/End to jump.
- **Line cursor** in the code panel, reusing the `.tali-hl-ln` / `.tali-hl-ln-hl` contract
  from `walkthrough.js` (already styled in `base.css`), plus a gutter marker.
- **Variables panel**, with changed entries flashing.
- **Four auto data views**, a closed set:
  1. numeric 1-D array becomes bars with index labels and read/write flashes,
  2. any other 1-D array or a string becomes boxes with values,
  3. 2-D array becomes a grid (DP tables, matrices, boards),
  4. an integer local whose value is a valid index into a visible array becomes a labelled
     pointer caret under that slot. This is what makes two-pointer and sliding-window
     problems legible.

  Anything else falls through to compact text in the variables panel.
- **Call stack**, shown only when the trace's maximum depth exceeds 1.
- **stdout pane**, shown only when the trace produced output.

Panels appear because the trace warrants them, never because a knob was set. Everything
is styled with `--tali-*` tokens, so unlike the hand-rolled sorting exhibit it follows the
theme.

Accessibility: `role="group"` with a label, transport buttons individually labelled, the
scrub is a real `<input type="range">` carrying `aria-valuetext="step 47 of 312"`, an
`aria-live="polite"` status line announces the current line, and
`prefers-reduced-motion` disables autoplay and transitions.

### 5. Width and full screen

The reading column (roughly 70ch) is too narrow for this block. Three levels, no
configuration:

1. **Default**: the block renders at `.column-page` width (`base.css:773-780`), escaping
   the prose measure with no author markup.
2. **Expand**: a transport-bar control promotes the block to the full viewport through the
   Fullscreen API, reusing the guarded pattern in `deck.js:2416-2420`, with a
   fixed-position overlay fallback where the API is unavailable. Esc exits. The reactive
   wiring survives because it is the same DOM element, promoted rather than recreated.
3. **Reflow**: panels stack below roughly 900px (code above, visual below) and sit side by
   side above it. In full screen the code panel takes a fixed left column and the visual
   takes the rest. Pure CSS; no JavaScript layout.

### 6. Integration points

The step index publishes into a hidden `[data-tali-input]` exactly as `.scrolly` does
(`scrolly.js:28-31`). Stepping is a reactive input driven by a step counter instead of by
scroll position, so **no new reactive machinery is introduced**. This is the same move
that let scrollytelling ship without touching the graph.

| Site | Change |
| --- | --- |
| `render/validate.rs:61` | `.debug` joins `DIV_FEATURE_CLASSES` (did-you-mean anchor) |
| `render/validate.rs:18` | `trace` joins `CELL_OPTION_KEYS` |
| `render/divs.rs` | the `.debug` emission arm |
| `render/text.rs` | a `.debug` arm: the code plus a first/last frame summary, so reading-form and the search index are not empty |
| `assets/js/tali-js.js` | `tali.frame(name)` beside `tali.value`/`tali.get`/`tali.state` |
| `crates/server/src/exec.rs` | a traced variant of `exec_cell` |
| `crates/server/src/trace_py.rs` (new) | the Python tracer harness source |
| `crates/core/src/vocab.rs` | `descriptions_present` for the new cell option |
| `crates/core/src/features.rs` | catalogue entry (read from the validator consts, automatic) |
| docs (guide + internals) | the feature's pages |

Adding a cell option key trips the documented drift gates. `./tools/gates.sh` is the
gate that catches the two living outside `taliesin-core`.

### 7. Diagnostics

Every one is located (file, line) and follows the existing warning machinery.

| Condition | Message |
| --- | --- |
| `.debug` holds no traced cell | names the missing `#| trace: true` / `//| trace: true` |
| `.debug` holds more than one traced cell | names the second one; only the first is traced |
| `#| trace: true` outside a `.debug` div | the option has no effect here |
| `trace:` with a non-boolean value | expected `true` or `false` |
| `{js}` view cell uses `//| input:` naming a `.debug` with no `name=` | names the unaddressable block |
| trace exceeded a cap | rendered in the block itself, not as a build warning: the reader needs to know the run was truncated |

The JS scanner's refuse-to-rewrite path also warns, as stated in section 3.

### 8. Other output paths

- **Decks**: `.debug` is a page feature. On a deck it renders as a plain highlighted code
  block. The deck engine is not extended (decks are frozen, per the 2026-08-02 scope
  ruling).
- **Print**: degrades to the code block plus the final frame's data view, so a printed
  page still shows the algorithm and its result.
- **Reading form and search**: the `render/text.rs` arm from section 6 supplies the code
  plus a first/last frame summary, so neither surface is empty.

### 9. Corpus and marketing

Per the corpus-plus-roadmap rule, each capability ships pinned by a corpus document added
in the same change.

| Document | Pins |
| --- | --- |
| `corpus/debug/sorting.tmd` | Python capture, auto bars, pointer carets, line cursor, zero-JS default |
| `corpus/debug/leetcode.tmd` | binary search (lo/mid/hi), sliding window over a string, two-pointer palindrome: boxes, string view, carets, dict rendering |
| `corpus/debug/dp.tmd` | 2-D grid auto view plus the cells each step reads (edit distance) |
| `corpus/debug/custom-view.tmd` | the JS generator adapter, `tali.frame`, the author-override path |
| `corpus/diagnostics/` addition | every row of the section 7 table |

Marketing: a new `## Step through an algorithm` section in `site/showcase.tmd`, following
that page's existing Result/Source pattern. Binary search is the default choice: the
shortest source that still produces a legible picture (three carets, a shrinking live
range), so both the Result and the Source halves fit without scrolling. Bubble sort is the
fallback if the browser check at 1440x900 shows it reads better.

Rewriting `corpus/graphics3d/sorting.tmd` onto the feature would delete roughly 170
hand-rolled lines and is the strongest possible proof, but it is held out of this change
to keep the diff reviewable. It is recorded as the immediate follow-up.

### 10. Testing

- Rust unit tests in `divs.rs` and `validate.rs` for emission and warnings.
- `crates/core/tests/corpus.rs` invariants apply automatically to the new documents
  (`data-block-id`, `data-sourcepos`).
- A trace-harness test running a known Python function and asserting the frame sequence.
  It needs a kernel, so it sits behind `TALIESIN_REQUIRE_KERNEL` and is covered by
  `./tools/gates.sh`, not by a bare `cargo test`.
- The JS yield scanner gets dedicated Rust unit tests covering strings, template
  literals, comments, and the refuse-to-rewrite path.
- `tsc` type-check for `debug.js` through the assets `jsconfig.json`.
- Browser verification through the chrome-devtools MCP at three viewports (390x844,
  1440x900, 900x1440) plus full screen, per the project's viewport matrix.

## Deliberately not building

- **Heap and pointer diagrams** (Python Tutor's boxes-and-arrows view). A different
  product with substantially more layout machinery, and weaker than bars and grids for the
  array-shaped problems this targets.
- **Reader-editable code.** Requires a server at read time and breaks the
  single-editing-surface invariant: the `.tmd` is the only editing surface and the preview
  never writes back.
- **R tracing.** No corpus document demands it. Add it when one does.
- **Per-panel configuration.** Panels are driven by what the trace contains.

## Open risk

The JS yield-to-line scanner, as described in section 3, with its stated fallback.
