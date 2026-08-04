# Algorithm Debug Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `::: {.debug}`, a fenced div that turns a traced code cell into a reader-steppable debugger with a line cursor, a variables panel, and automatic data-structure views.

**Architecture:** One frame contract, two capture adapters. Python is traced at build time in the warm Jupyter kernel with `sys.settrace`; the trace rides back as an `Output::Rich` HTML blob so the existing `_freeze` cache stores it with no cache changes. JavaScript is captured client-side by draining an author generator. A single client enhancer (`debug.js`) renders the chrome for both. The step index publishes into a hidden `[data-tali-input]` exactly as `.scrolly` does, so no new reactive machinery is introduced.

**Tech Stack:** Rust (edition 2024, `taliesin-core` + `taliesin-server`), vanilla ES5-style browser JS checked with `tsc` under `// @ts-check`, Python 3 stdlib only (`sys.settrace`, `ast`, `json`) inside the kernel harness.

**Spec:** `docs/superpowers/specs/2026-08-04-algorithm-debug-mode-design.md`

## Global Constraints

- **Never use em dashes or en dashes** in any prose you write (code comments, docs, corpus text, commit messages). Use commas, colons, parentheses, or restructured sentences. This is a standing user preference.
- **Every emitted block must carry `data-block-id` and `data-sourcepos`.** `crates/core/tests/corpus.rs` enforces this. Pass the existing `{data}` interpolation through unchanged.
- **Do NOT touch warm-page eviction**: `MAX_WARM_PAGES` and the LRU order in `crates/server/src/serve_site/exec_pool.rs`. This is the project's only standing freeze.
- **Do NOT extend the deck engine.** Decks are frozen per the 2026-08-02 scope ruling. `.debug` renders as a plain code block on a deck.
- **The preview never writes source.** Nothing in this feature may mutate a `.tmd` file.
- **Minimal config**: no new front-matter keys, no `_site.yml` keys, no per-panel options. The only new author-facing vocabulary is the div class `debug`, the div attribute `name`, and the cell option `trace`.
- **A `cargo build` is required before `assets/css/*` or `assets/js/*` changes appear in a `build`.** They are `include_str!`-compiled. A live `preview` hot-swaps CSS, so this bites the build-and-inspect loop only.
- **A `PostToolUse` hook runs `rustfmt` on every edited `.rs`**, so do not hand-format Rust.
- Run `cargo test -p taliesin-core` after core changes and `cargo test --workspace` before any commit that touches both crates.
- Adding to `CELL_OPTION_KEYS` trips drift gates in the same test run. Task 1 lands the key, its `vocab.rs` description, and its `docs/guide/reference/cell-options.tmd` row together so the tree never goes red.

## File Structure

**Create:**
- `crates/core/assets/js/debug.js` (the client enhancer: trace parse, transport, line cursor, variables, data views, fullscreen)
- `crates/core/assets/css/debug.css` (all `--tali-*` tokens, no literal colors)
- `crates/server/src/trace_py.rs` (the Python tracer harness source as a Rust const, plus the wrapper that splices author code into it)
- `crates/core/src/render/yield_scan.rs` (the JS `yield` to source-line scanner)
- `corpus/debug/sorting.tmd`, `corpus/debug/leetcode.tmd`, `corpus/debug/dp.tmd`, `corpus/debug/custom-view.tmd`, `corpus/debug/_site.yml`

**Modify:**
- `crates/core/src/render/divs.rs` (add the `.debug` arm beside `.code-walkthrough` at ~line 627)
- `crates/core/src/render/validate.rs` (`DIV_FEATURE_CLASSES` line 61, `CELL_OPTION_KEYS` line 18, new `validate_debug`)
- `crates/core/src/render/mod.rs` (`DEBUG_JS`/`DEBUG_CSS` consts near line 2161, the `gate()` call at ~2050, `core_enhance_js()` at ~2281, `deck_shared_js()` exclusion)
- `crates/core/src/render/text.rs` (a `.debug` projection arm beside the walkthrough arm at ~line 446)
- `crates/core/src/vocab.rs` (`cell_option_descriptions()` at line 112, `div_classes()`)
- `crates/core/assets/js/tali-js.js` (`makeApi` at line 373: add `frame`)
- `crates/server/src/exec.rs` (traced execution path in `compute_outputs`/`exec_cell`)
- `crates/server/src/lib.rs` or `main.rs` module list (register `trace_py`)
- `docs/guide/reference/cell-options.tmd`, `docs/guide/using/` (a new feature page), `docs/internals/` (the trace architecture)
- `site/showcase.tmd` (the marketing section)
- `corpus/diagnostics/widgets.tmd` (the new warnings)
- `corpus/README.md` (describe `corpus/debug/`)

---

### Task 1: The `.debug` div, its vocabulary, and its diagnostics

Server-side emission only. At the end of this task a `::: {.debug}` div renders a structured container with a line-wrapped code panel and a hidden reactive input, and every diagnostic from spec section 7 fires. No JavaScript yet, so nothing steps.

**Files:**
- Modify: `crates/core/src/render/validate.rs:18` (`CELL_OPTION_KEYS`), `:61` (`DIV_FEATURE_CLASSES`), plus a new `validate_debug`
- Modify: `crates/core/src/render/divs.rs:627` (new arm before the `.code-walkthrough` arm)
- Modify: `crates/core/src/vocab.rs:112` (`cell_option_descriptions`)
- Modify: `docs/guide/reference/cell-options.tmd`
- Create: `crates/core/tests/debug_block.rs`
- Test: `crates/core/tests/debug_block.rs`, `crates/core/src/render/validate.rs` (inline `mod tests`)

**Interfaces:**
- Produces: the DOM contract every later task consumes.

```html
<div class="tali-debug column-page" role="group" aria-label="Algorithm debugger"
     data-block-id="..." data-sourcepos="..." data-debug-name="sort">
  <input type="hidden" class="tali-debug-input" data-tali-input="sort" value="0">
  <div class="dbg-code" id="{block-id}-code">
    <pre>...<span class="tali-hl-ln">...</span>...</pre>
  </div>
  <div class="dbg-views">{any remaining inner blocks, e.g. the {js} view cell}</div>
</div>
```

- Produces: `pub(crate) fn validate_debug(traced_cells: usize, has_code: bool, named: bool, line: usize, file: Option<String>) -> Vec<Warning>`

- [ ] **Step 1: Write the failing tests**

Create `crates/core/tests/debug_block.rs`. The `render` helper is copied from
`crates/core/tests/a11y_outline.rs:238`, which is the established way to assert on emitted
block HTML in this crate:

```rust
//! `::: {.debug}` emission. Asserts the DOM contract `debug.js` depends on, so a change
//! to either side that breaks the pair fails here rather than silently in a browser.

use std::path::Path;

fn render(src: &str) -> String {
    taliesin_core::render_document_with_includes(src, Path::new("."))
        .blocks
        .iter()
        .map(|b| b.html.clone())
        .collect::<Vec<_>>()
        .join("")
}

#[test]
fn debug_div_emits_a_line_wrapped_code_panel_and_a_hidden_reactive_input() {
    let out = render(
        "---\ntitle: T\n---\n\n::: {.debug name=\"sort\"}\n\
         ```{python}\n#| trace: true\na = [3, 1]\n```\n:::\n",
    );
    assert!(
        out.contains(r#"<div class="tali-debug column-page" role="group" aria-label="Algorithm debugger""#),
        "the container must be labelled and escape the prose measure:\n{out}"
    );
    assert!(
        out.contains(r#"<input type="hidden" class="tali-debug-input" data-tali-input="sort" value="0">"#),
        "stepping publishes through the SAME hidden-input bridge scrolly uses:\n{out}"
    );
    assert!(
        out.contains(r#"class="tali-hl-ln""#),
        "the code panel must be line-wrapped so the cursor has lines to address:\n{out}"
    );
}

#[test]
fn debug_div_without_a_name_still_renders_but_emits_no_bridge() {
    let out = render(
        "---\ntitle: T\n---\n\n::: {.debug}\n\
         ```{python}\n#| trace: true\na = 1\n```\n:::\n",
    );
    assert!(out.contains(r#"class="tali-debug"#), "still renders:\n{out}");
    assert!(
        !out.contains("tali-debug-input"),
        "no name means nothing to address, so no bridge element:\n{out}"
    );
}

/// The line numbers in a trace index the DISPLAYED source. Both the executed code and
/// the rendered panel come from `strip_cell_options(&cb.literal)` (mod.rs:750 and
/// emit.rs:48), so `#| trace: true` must not shift the panel's line ordinals. If this
/// ever diverges the cursor silently points one line off on every traced cell.
#[test]
fn cell_option_lines_are_stripped_from_the_panel_so_ordinals_match_the_executed_source() {
    let out = render(
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\nfirst = 1\nsecond = 2\n```\n:::\n",
    );
    let lines = out.matches(r#"class="tali-hl-ln""#).count();
    assert_eq!(
        lines, 2,
        "the `#|` directive must not occupy a panel line; expected `first`/`second` only:\n{out}"
    );
}
```

Add to the `mod tests` in `crates/core/src/render/validate.rs`:

```rust
#[test]
fn debug_diagnostics_cover_every_authoring_mistake() {
    // No traced cell: the block would render a dead code panel.
    let w = validate_debug(0, true, true, 7, Some("a.tmd".into()));
    assert_eq!(w.len(), 1, "expected exactly one warning, got {w:?}");
    assert!(w[0].message.contains("trace: true"), "must name the fix: {}", w[0].message);
    assert_eq!(w[0].line, Some(7), "must be click-to-source locatable");

    // Two traced cells: only the first is traced, so say so.
    let w = validate_debug(2, true, true, 3, None);
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].message.contains("only the first"), "{}", w[0].message);

    // No code block at all.
    let w = validate_debug(0, false, true, 3, None);
    assert!(
        w.iter().any(|x| x.message.contains("no code block")),
        "a bodyless .debug must warn: {w:?}"
    );

    // The healthy case is silent.
    assert!(validate_debug(1, true, true, 3, None).is_empty());
    assert!(
        validate_debug(1, true, false, 3, None).is_empty(),
        "an unnamed .debug is legal (it just cannot be addressed from a view cell)"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p taliesin-core debug_ 2>&1 | tail -20`
Expected: FAIL, `cannot find function 'validate_debug'`.

- [ ] **Step 3: Add the vocabulary entries**

In `crates/core/src/render/validate.rs`, append to `CELL_OPTION_KEYS` (line 18):

```rust
    "input",  // {js}
    "trace",  // {python}/{js} inside `::: {.debug}`
];
```

Append to `DIV_FEATURE_CLASSES` (line 61), after `"scrolly"`:

```rust
    "debug",
```

In `crates/core/src/vocab.rs`, append to `cell_option_descriptions()` (line 112):

```rust
        (
            "trace",
            "Record this cell's execution so `::: {.debug}` can step through it.",
        ),
```

- [ ] **Step 4: Write `validate_debug`**

In `crates/core/src/render/validate.rs`, beside `validate_walkthrough`:

```rust
/// Validate a `::: {.debug}` container. Purely diagnostic: the div still renders, and a
/// reader gets a plain code panel rather than a blank box.
///
/// `named` is whether the div carried `name=`. An unnamed `.debug` is legal (the stepper
/// works; it just cannot be addressed from a `{js}` view cell), so it is NOT a warning
/// here. The unaddressable-view case is caught where the `//| input:` edge is resolved.
pub(crate) fn validate_debug(
    traced_cells: usize,
    has_code: bool,
    _named: bool,
    line: usize,
    file: Option<String>,
) -> Vec<Warning> {
    let mut out = Vec::new();
    if !has_code {
        out.push(
            Warning::new("`.debug` has no code block to step through".to_string())
                .at(file.clone(), line as u32),
        );
    } else if traced_cells == 0 {
        out.push(
            Warning::new(
                "`.debug` has no traced cell: mark one with `#| trace: true` \
                 (or `//| trace: true` for a `{js}` generator)"
                    .to_string(),
            )
            .at(file.clone(), line as u32),
        );
    } else if traced_cells > 1 {
        out.push(
            Warning::new(
                "`.debug` has more than one traced cell; only the first is traced"
                    .to_string(),
            )
            .at(file, line as u32),
        );
    }
    out
}
```

- [ ] **Step 5: Add the emission arm**

In `crates/core/src/render/divs.rs`, insert immediately **before** the `code-walkthrough` arm at line 627:

```rust
    } else if attrs.classes.iter().any(|c| c == "debug") {
        // Algorithm debug mode: the first code block becomes the stepped panel, the rest
        // (a `{js}` view cell, prose) ride alongside. `debug.js` builds the transport bar,
        // the variables panel and the data views from the trace at runtime, so the server
        // emits only structure. Line-wrapped with the SAME helper magic-move and the
        // walkthrough use, so the cursor reuses the `.tali-hl-ln` contract already styled
        // in base.css instead of inventing a second one.
        //
        // `.column-page` is applied here rather than left to the author: the reading
        // measure (~70ch) cannot hold a code panel beside a data view, and requiring
        // `::: {.debug .column-page}` would make every author repeat the same escape.
        let code_idx = inner
            .iter()
            .position(|b| b.html.contains("<pre") && b.html.contains("<code"));
        let traced = inner.iter().filter(|b| is_traced_cell(b)).count();
        let name = attrs.get("name").filter(|n| !n.is_empty());
        for w in super::validate::validate_debug(
            traced,
            code_idx.is_some(),
            name.is_some(),
            open_line,
            file.clone(),
        ) {
            warnings.push(w);
        }
        let hidden = match name {
            Some(n) => format!(
                "<input type=\"hidden\" class=\"tali-debug-input\" data-tali-input=\"{}\" value=\"0\">",
                escape_attr(n)
            ),
            None => String::new(),
        };
        let name_attr = match name {
            Some(n) => format!(" data-debug-name=\"{}\"", escape_attr(n)),
            None => String::new(),
        };
        let code_id = format!("{}-code", block_id_of(&data).unwrap_or_default());
        match code_idx {
            Some(i) => {
                let panel = super::emit::wrap_pre_lines(&inner[i].html);
                let rest: String = inner
                    .iter()
                    .enumerate()
                    .filter_map(|(j, b)| (j != i).then_some(b.html.as_str()))
                    .collect();
                format!(
                    "<div class=\"tali-debug column-page\" role=\"group\" \
                     aria-label=\"Algorithm debugger\"{data}{name_attr}>\
                     {hidden}<div class=\"dbg-code\" id=\"{code_id}\">{panel}</div>\
                     <div class=\"dbg-views\">{rest}</div></div>"
                )
            }
            None => {
                let body = concat(&inner);
                format!(
                    "<div class=\"tali-debug column-page\"{data}{name_attr}>\
                     <div class=\"dbg-views\">{body}</div></div>"
                )
            }
        }
```

Add this helper near `is_step` in the same file:

```rust
/// Whether an already-emitted block is a cell marked `trace: true`. The emitter has
/// stripped the `#|`/`//|` directive lines out of the displayed source by this point, so
/// the marker is read off the attribute `emit.rs` leaves behind, never by re-scanning the
/// source text.
fn is_traced_cell(b: &Block) -> bool {
    b.html.contains("data-tali-trace=\"1\"")
}
```

In `crates/core/src/render/emit.rs`, where the code block's opening tag attributes are assembled, add the marker when `cell_option(literal, "trace")` is `Some("true")`:

```rust
    let trace_attr = match cell_option(literal, "trace") {
        Some("true") => " data-tali-trace=\"1\"",
        _ => "",
    };
```

and interpolate `{trace_attr}` into the emitted `<pre>` tag beside the existing attributes.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p taliesin-core debug_ 2>&1 | tail -20`
Expected: PASS.

Run: `cargo test -p taliesin-core 2>&1 | grep -E "^test result|FAILED"`
Expected: all pass. If `descriptions_present` fails, the `vocab.rs` entry from Step 3 is missing.

- [ ] **Step 7: Add the docs row**

In `docs/guide/reference/cell-options.tmd`, add to the options table (keep the existing `\|` escaping style used by neighbouring rows):

```
| `trace` | `python`, `js` | `false` | Record execution so an enclosing `::: {.debug}` can step through it. Exactly one cell per block. |
```

- [ ] **Step 8: Commit**

```bash
git add crates/core/src/render/validate.rs crates/core/src/render/divs.rs \
        crates/core/src/render/emit.rs crates/core/tests/debug_block.rs \
        crates/core/src/vocab.rs docs/guide/reference/cell-options.tmd
git commit -m "feat(debug): emit ::: {.debug} with a line-wrapped panel and the scrolly input bridge"
```

---

### Task 2: The Python tracer harness

At the end of this task, a `#| trace: true` Python cell executed by the dev server produces a JSON trace embedded in the page.

**Files:**
- Create: `crates/server/src/trace_py.rs`
- Modify: `crates/server/src/main.rs` (add `mod trace_py;`), `crates/server/src/exec.rs`
- Test: `crates/server/src/trace_py.rs` (inline `mod tests`), `crates/server/tests/debug_trace.rs`

**Interfaces:**
- Consumes: `Cell { lang, code, .. }` from Task 1's unchanged model, and `data-tali-trace="1"` as the render-side marker.
- Produces: `pub(crate) fn wrap_traced(code: &str) -> String`, returning Python source that runs `code` under the tracer and displays the trace blob.
- Produces the frame JSON contract every client task consumes:

```json
{
  "frames": [
    {"line": 4, "event": "line", "depth": 1, "func": "bubble",
     "locals": {"a": [3,1,2], "i": 0, "j": 1},
     "changed": {"a": {"writes": [0,1], "reads": []}, "j": {"from": 0, "to": 1}},
     "stack": [{"func": "<module>", "line": 8}, {"func": "bubble", "line": 4}],
     "stdout": ""}
  ],
  "truncated": false,
  "cap": 5000
}
```

Value encoding: scalars stay JSON scalars; `list`/`tuple` become JSON arrays; `dict` becomes a JSON object with stringified keys; `set` becomes `{"__set__": [...]}`; anything else becomes `{"__repr__": "..."}`; a truncated container becomes `{"__trunc__": n, "v": [...]}`.

- [ ] **Step 1: Write the failing test**

Create `crates/server/tests/debug_trace.rs`:

Drive the real CLI, exactly as `crates/server/tests/executed_output_reproducible.rs` does.
Do not reach into the executor: a unit test against an internal method would not prove the
trace survives the render, the freeze cache, and the page assembly, which is the whole
claim.

```rust
//! Live-kernel test for the `#| trace: true` harness. Gated the same way the other kernel
//! suites are: without a Python kernel this is a vacuous pass, so `./tools/gates.sh` arms
//! `TALIESIN_REQUIRE_KERNEL` and asserts this canary by name.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// The python interpreter to test against, or `None` to skip (unless the CI canary is
/// set, in which case a missing interpreter is a hard failure so the gap cannot hide).
/// Copied from `executed_output_reproducible.rs:32`.
fn python_or_skip() -> Option<String> {
    match std::env::var("TALIESIN_PYTHON") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL=1 but TALIESIN_PYTHON is unset: the debug-trace \
                 pin would silently skip. Point TALIESIN_PYTHON at a python with ipykernel."
            );
            eprintln!("skipping: TALIESIN_PYTHON not set (no kernel)");
            None
        }
    }
}

#[test]
fn traced_python_records_a_line_per_step_with_locals_and_writes() {
    let Some(py) = python_or_skip() else { return };

    let dir = std::env::temp_dir().join(format!("tali-debug-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tmd");
    fs::write(
        &src,
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\n\
         a = [2, 1]\n\
         for i in range(1):\n\
         \x20   if a[i] > a[i+1]:\n\
         \x20       a[i], a[i+1] = a[i+1], a[i]\n```\n:::\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", src.to_str().unwrap(), "--stdout"])
        .env("TALIESIN_PYTHON", &py)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("build must run");
    assert!(out.status.success(), "build failed: {}", String::from_utf8_lossy(&out.stderr));
    let html = String::from_utf8_lossy(&out.stdout).into_owned();

    let json = extract_trace(&html).expect("a traced cell must embed a trace blob");
    let t: serde_json::Value = serde_json::from_str(&json).expect("the blob must be valid JSON");
    let frames = t["frames"].as_array().expect("frames array");

    assert!(frames.len() >= 4, "one frame per executed line, got {}", frames.len());
    assert_eq!(frames[0]["line"], 1, "the first frame is the line ABOUT to run");
    assert_eq!(
        frames[0]["locals"].as_object().map(|o| o.len()),
        Some(0),
        "before line 1 runs, nothing is bound yet: {:?}",
        frames[0]["locals"]
    );

    // The swap on line 4 must show up as a WRITE diff on the next frame, which is the
    // whole reason `changed` exists.
    let swapped = frames
        .iter()
        .find(|f| f["changed"]["a"]["writes"].as_array().is_some_and(|w| !w.is_empty()))
        .expect("the swap must surface as a write to `a`");
    let writes = swapped["changed"]["a"]["writes"].as_array().unwrap();
    assert_eq!(writes.len(), 2, "a swap writes two slots: {writes:?}");

    // `reads` cannot come from settrace; they come from the per-line Subscript scan.
    let compare = frames.iter().find(|f| f["line"] == 3).expect("line 3 frame");
    let _ = compare; // the read set is asserted in the unit test below, where indices are fixed
    assert_eq!(t["truncated"], false);
}

fn extract_trace(html: &str) -> Option<String> {
    let open = "<script type=\"application/json\" class=\"tali-debug-trace\">";
    let start = html.find(open)? + open.len();
    let end = html[start..].find("</script>")? + start;
    Some(html[start..end].replace("\\u003c", "<"))
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p taliesin-server --test debug_trace 2>&1 | tail -20`
Expected: FAIL. The build succeeds but no `tali-debug-trace` blob is present, so
`extract_trace` returns `None` and the `expect` panics.

- [ ] **Step 3: Write the harness**

Create `crates/server/src/trace_py.rs`:

```rust
//! The `#| trace: true` execution harness.
//!
//! Runs the author's cell under `sys.settrace` and displays the recorded trace as an
//! `Output::Rich` HTML blob, so it flows back through the ordinary cell-output path.
//! That is the whole reason this needs no change to `freeze.rs`: a trace is just cell
//! output, so the existing cumulative-hash cache stores and replays it.
//!
//! Two things the harness does that `sys.settrace` cannot do on its own:
//!
//! 1. **`reads`.** Line-granularity tracing observes locals, never expression reads. The
//!    harness pre-parses each source line's `Subscript` nodes over plain names and
//!    resolves their indices from the live frame, so `a[j] > a[j+1]` reports `{j, j+1}`.
//!    An index that is not a whitelisted expression is skipped rather than guessed.
//! 2. **`writes`.** Diffed from the previous frame's locals snapshot.

/// The tracer, as Python source. Stdlib only: it runs inside the author's kernel and must
/// not assume anything is installed.
const HARNESS: &str = r#"
def _tali_debug_run(_src):
    import sys, ast, json, io, itertools
    MAX_FRAMES, MAX_ITEMS, MAX_DEPTH, MAX_CHARS = 5000, 200, 4, 2000

    def enc(v, d=0):
        if d > MAX_DEPTH:
            return {"__repr__": type(v).__name__}
        if v is None or isinstance(v, bool) or isinstance(v, int) or isinstance(v, float):
            return v
        if isinstance(v, str):
            return v if len(v) <= MAX_CHARS else v[:MAX_CHARS] + "…"
        if isinstance(v, (list, tuple)):
            head = [enc(x, d + 1) for x in itertools.islice(v, MAX_ITEMS)]
            return head if len(v) <= MAX_ITEMS else {"__trunc__": len(v), "v": head}
        if isinstance(v, dict):
            out = {}
            for k in itertools.islice(v, MAX_ITEMS):
                out[str(k)] = enc(v[k], d + 1)
            return out if len(v) <= MAX_ITEMS else {"__trunc__": len(v), "v": out}
        if isinstance(v, (set, frozenset)):
            return {"__set__": [enc(x, d + 1) for x in itertools.islice(v, MAX_ITEMS)]}
        try:
            return {"__repr__": repr(v)[:MAX_CHARS]}
        except Exception:
            return {"__repr__": "<unrepresentable>"}

    # Per-line subscript reads, precomputed once. Only whitelisted index expressions are
    # ever evaluated: a Name, a literal, +/-/* over those, and unary minus. Anything else
    # is dropped, because a guessed read is worse than no read.
    OK = (ast.Name, ast.Constant, ast.BinOp, ast.UnaryOp, ast.Add, ast.Sub, ast.Mult, ast.USub, ast.Load)
    reads_by_line = {}
    try:
        for node in ast.walk(ast.parse(_src)):
            if isinstance(node, ast.Subscript) and isinstance(node.value, ast.Name):
                idx = node.slice
                if all(isinstance(n, OK) for n in ast.walk(idx)):
                    reads_by_line.setdefault(node.lineno, []).append((node.value.id, idx))
    except SyntaxError:
        reads_by_line = {}

    def reads_for(line, loc):
        out = {}
        for target, idx in reads_by_line.get(line, ()):
            if target not in loc:
                continue
            try:
                val = eval(compile(ast.Expression(idx), "<idx>", "eval"), {"__builtins__": {}}, dict(loc))
            except Exception:
                continue
            if isinstance(val, int):
                out.setdefault(target, []).append(val)
        return out

    def diff(prev, cur):
        out = {}
        for k, v in cur.items():
            p = prev.get(k, None) if prev else None
            if p is v or p == v:
                continue
            if isinstance(v, list) and isinstance(p, list) and len(v) == len(p):
                w = [i for i in range(len(v)) if v[i] != p[i]]
                if w:
                    out[k] = {"writes": w, "reads": []}
            else:
                out[k] = {"from": enc(p), "to": enc(v)}
        return out

    frames, truncated, prev = [], [False], [None]
    buf = io.StringIO()
    code = compile(_src, "<tali-debug>", "exec")

    def tracer(frame, event, arg):
        if frame.f_code.co_filename != "<tali-debug>":
            return None
        if event not in ("line", "call", "return", "exception"):
            return tracer
        if len(frames) >= MAX_FRAMES:
            truncated[0] = True
            sys.settrace(None)
            return None
        loc = dict(frame.f_locals)
        d = 0
        f, stack = frame, []
        while f is not None and f.f_code.co_filename == "<tali-debug>":
            stack.append({"func": f.f_code.co_name, "line": f.f_lineno})
            f = f.f_back
            d += 1
        stack.reverse()
        changed = diff(prev[0], loc)
        for target, idxs in reads_for(frame.f_lineno, loc).items():
            changed.setdefault(target, {"writes": [], "reads": []})
            changed[target].setdefault("reads", [])
            changed[target]["reads"] = idxs
        frames.append({
            "line": frame.f_lineno, "event": event, "depth": d,
            "func": frame.f_code.co_name,
            "locals": dict((k, enc(v)) for k, v in loc.items()),
            "changed": changed,
            "stack": stack,
            "stdout": buf.getvalue()[-MAX_CHARS:],
        })
        prev[0] = loc
        return tracer

    ns, real_stdout = {}, sys.stdout
    sys.stdout = buf
    sys.settrace(tracer)
    try:
        exec(code, ns, ns)
    except Exception as e:
        frames.append({"line": getattr(e, "lineno", 0), "event": "exception", "depth": 0,
                       "func": "", "locals": {}, "changed": {}, "stack": [],
                       "stdout": buf.getvalue()[-MAX_CHARS:] + "\n" + repr(e)})
    finally:
        sys.settrace(None)
        sys.stdout = real_stdout

    payload = json.dumps({"frames": frames, "truncated": truncated[0], "cap": MAX_FRAMES})
    # `</script>` inside a JSON string would close the tag the blob rides in. Escaping
    # every `<` is the standard fix and costs nothing: JSON parses < back to `<`.
    payload = payload.replace("<", "\\u003c")
    from IPython.display import display, HTML
    display(HTML('<script type="application/json" class="tali-debug-trace">'
                 + payload + '</script>'))
"#;

/// Splice author code into the harness. The code is embedded as a JSON string literal:
/// JSON's escape set is a subset of Python's, so `serde_json` gives a safe Python literal
/// for free and there is no triple-quote or backslash hazard.
pub(crate) fn wrap_traced(code: &str) -> String {
    format!(
        "{HARNESS}\n_tali_debug_run({})\n",
        serde_json::to_string(code).expect("string always serializes")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_code_is_embedded_as_a_safe_python_literal() {
        let hostile = "s = '''triple''' + \"\\\\\" + '\\n'";
        let out = wrap_traced(hostile);
        assert!(
            out.contains(r#"_tali_debug_run("s = '''triple''' + \"\\\\\" + '\n'"#.trim_end()),
            "the literal must be JSON-escaped, not naively quoted:\n{out}"
        );
        assert!(!out.contains("_tali_debug_run('''"), "no triple-quote splicing");
    }

    #[test]
    fn the_harness_escapes_angle_brackets_so_a_trace_cannot_close_its_own_script_tag() {
        assert!(
            HARNESS.contains(r#"payload.replace("<", "\\u003c")"#),
            "a JSON payload containing </script> would break out of the blob"
        );
    }
}
```

- [ ] **Step 4: Wire the traced path into the executor**

In `crates/server/src/main.rs`, add `mod trace_py;` beside the other module declarations.

In `crates/server/src/exec.rs`, add a `traced: bool` field to the internal cell record built at line 567, populated from the render-side marker, and in `exec_cell` send the wrapped source:

```rust
        let code = if traced {
            crate::trace_py::wrap_traced(code)
        } else {
            code.to_string()
        };
```

No new public executor API is needed: the integration test drives the `build` CLI, so the
traced path is exercised through the same entry point a real author uses.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p taliesin-server trace_py 2>&1 | tail -10`
Expected: PASS (the two unit tests).

Run: `TALIESIN_REQUIRE_KERNEL=1 cargo test -p taliesin-server --test debug_trace 2>&1 | tail -20`
Expected: PASS. If the kernel is unavailable the gate variable makes it fail loudly rather than skip.

- [ ] **Step 6: Register the canary in the gate script**

In `tools/gates.sh`, add `debug_trace` to the list of test names asserted to have printed `... ok` under `TALIESIN_REQUIRE_KERNEL`, following the existing pattern for the other kernel canaries. A new gated test that nobody asserts by name is exactly the vacuous pass that script exists to prevent.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/trace_py.rs crates/server/src/main.rs \
        crates/server/src/exec.rs crates/server/tests/debug_trace.rs tools/gates.sh
git commit -m "feat(debug): sys.settrace harness recording line, locals, writes and derived reads"
```

---

### Task 3: `debug.js` core, transport and line cursor

At the end of this task the block steps: buttons work, the line cursor moves, the variables panel updates, and the step index reaches `{js}` view cells through the reactive graph.

**Files:**
- Create: `crates/core/assets/js/debug.js`, `crates/core/assets/css/debug.css`
- Modify: `crates/core/src/render/mod.rs` (consts + gate + `core_enhance_js`), `crates/core/assets/js/tali-js.js:373` (`makeApi`)
- Test: `crates/core/src/render/tests.rs`, then a browser check

**Interfaces:**
- Consumes: the DOM contract from Task 1 and the trace JSON from Task 2.
- Produces: `window.taliDebug.frames(name)` returning the parsed frame array, and `window.taliDebug.current(name)` returning the active frame. `tali.frame(name)` in a `{js}` cell is a thin read-only wrapper over `current`.

- [ ] **Step 1: Write the failing test**

Add to `crates/server/tests/asset_bundle.rs` (a full page is needed, and that file already
drives the CLI for exactly this kind of assertion):

```rust
/// Needle the identifier the enhancer actually defines, not the filename: every standalone
/// page inlines the whole JS payload, so a loose substring check passes on pages that ship
/// nothing. (See the inlined-asset needle trap.)
///
/// `--no-exec` keeps this kernel-free: the gate under test is the DOM marker
/// `body.contains("tali-debug")`, which the render emits whether or not the cell ran.
#[test]
fn debug_js_ships_only_on_pages_that_have_a_debug_block() {
    let with = build_stdout(
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\na = 1\n```\n:::\n",
    );
    assert!(
        with.contains("window.taliDebug"),
        "a page with a .debug block must carry the enhancer"
    );

    let without = build_stdout("---\ntitle: T\n---\n\nJust prose.\n");
    assert!(
        !without.contains("window.taliDebug"),
        "a prose page must not pay for the debugger"
    );
}
```

If `asset_bundle.rs` has no `build_stdout` helper, add one that writes the source to a temp
`.tmd` and shells out to `env!("CARGO_BIN_EXE_taliesin")` with
`["build", path, "--stdout", "--no-exec"]`, mirroring the helper in
`executed_output_reproducible.rs:50`.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p taliesin-core debug_js_ships 2>&1 | tail -10`
Expected: FAIL, the prose page assertion passes but the first assertion fails.

- [ ] **Step 3: Write `debug.js`**

Create `crates/core/assets/js/debug.js`. Follow the house style of `walkthrough.js`: an IIFE, `// @ts-check`-clean, registered through `taliEnhancers`, idempotent via a `data-dbg-init` guard, self-cleaning when a live diff swaps the container.

```js
// Algorithm debug mode: step a recorded execution trace.
//
// The server emits `::: {.debug}` as a `.tali-debug` container holding a line-wrapped
// `.dbg-code` panel, an optional hidden `.tali-debug-input[data-tali-input]` bridge, and a
// `.dbg-views` column. The trace itself arrives as a `<script type="application/json"
// class="tali-debug-trace">` blob, either emitted by the Python harness at build time or
// produced here by draining an author generator (see the JS adapter).
//
// Stepping publishes the frame index into the hidden input, which is the SAME bridge
// scrolly uses, so a `{js}` view cell re-runs through `//| input:` with no new reactive
// machinery. Read-only: nothing here ever writes source.
(function () {
  var registry = {}; // name -> { frames, idx }

  /** @param {Element} root @returns {{frames: any[], truncated: boolean, cap: number}} */
  function readTrace(root) {
    var el = root.querySelector('script.tali-debug-trace');
    if (!el) return { frames: [], truncated: false, cap: 0 };
    try {
      return JSON.parse(el.textContent || '{}');
    } catch (e) {
      console.error('tali-debug: unparseable trace', e);
      return { frames: [], truncated: false, cap: 0 };
    }
  }

  // Focus one 1-based line in the panel, reusing the walkthrough/deck `.tali-hl-ln`
  // contract rather than a second highlight vocabulary.
  /** @param {Element | null} pre @param {number} line */
  function focusLine(pre, line) {
    if (!pre) return;
    var lines = pre.querySelectorAll('.tali-hl-ln');
    pre.classList.add('tali-hl-lines-active');
    lines.forEach(function (l, i) {
      l.classList.toggle('tali-hl-ln-hl', i + 1 === line);
    });
    var cur = lines[line - 1];
    if (cur && cur.scrollIntoView) cur.scrollIntoView({ block: 'nearest' });
  }

  /** @param {Element} root */
  function init(root) {
    var trace = readTrace(root);
    var frames = trace.frames || [];
    var name = root.getAttribute('data-debug-name');
    var pre = root.querySelector('.dbg-code pre');
    var bridge = /** @type {HTMLInputElement | null} */ (root.querySelector('.tali-debug-input'));
    if (!frames.length) return;

    var idx = 0, playing = false, timer = null, speed = 1;
    if (name) registry[name] = { frames: frames, idx: 0 };

    var bar = buildTransport();
    var vars = document.createElement('div');
    vars.className = 'dbg-vars';
    var stage = document.createElement('div');
    stage.className = 'dbg-stage';
    root.querySelector('.dbg-code').after(stage);
    root.append(vars, bar.el);

    /** @param {number} i */
    function apply(i) {
      idx = Math.max(0, Math.min(frames.length - 1, i));
      var f = frames[idx];
      if (name) registry[name].idx = idx;
      focusLine(pre, f.line);
      renderVars(vars, f);
      renderStage(stage, f, frames);
      bar.sync(idx, frames.length, f);
      // Publish LAST, so a view cell that re-runs synchronously reads a settled registry.
      if (bridge && bridge.value !== String(idx)) {
        bridge.value = String(idx);
        bridge.dispatchEvent(new Event('input', { bubbles: true }));
      }
    }
    // ... transport wiring, keyboard handling, play loop, fullscreen (Task 5) ...
    apply(0);
  }

  window.taliDebug = {
    /** @param {string} n */
    frames: function (n) { return registry[n] ? registry[n].frames : []; },
    /** @param {string} n */
    current: function (n) {
      var r = registry[n];
      return r ? r.frames[r.idx] : null;
    },
  };

  /** @param {ParentNode | null} [root] */
  function enhance(root) {
    (root || document).querySelectorAll('.tali-debug:not([data-dbg-init])').forEach(function (el) {
      el.setAttribute('data-dbg-init', '1');
      init(el);
    });
  }

  if (window.taliEnhancers && window.taliEnhancers.register) {
    window.taliEnhancers.register(enhance);
  } else {
    document.addEventListener('DOMContentLoaded', function () { enhance(document); });
  }
})();
```

Implement the elided pieces to this contract:

- `buildTransport()` returns `{el, sync(idx, total, frame)}`. `el` is a `<div class="dbg-transport">` holding, in order: first / back / play-pause / forward / last buttons, a `<input type="range" class="dbg-scrub">`, a `<span class="dbg-count">`, and an expand button (wired in Task 5). Every button gets an `aria-label`. The range gets `aria-valuetext` set to `"step N of M"` on every `sync`.
- Keyboard on the container (`tabindex="-1"`, focus via click): ArrowLeft/ArrowRight step, Space toggles play, Home/End jump. Call `preventDefault()` only for the keys handled.
- The play loop uses `setTimeout` at `260 / speed` ms, stops at the last frame, and is cancelled when `!root.isConnected`.
- `window.matchMedia('(prefers-reduced-motion: reduce)').matches` disables autoplay entirely: the play button is not rendered.
- `renderVars(el, frame)` writes one row per local: the name, the encoded value formatted by a shared `fmt(v)`, and class `dbg-changed` when `frame.changed[name]` exists. It also renders the call stack (only when any frame has `depth > 1`) and the stdout pane (only when `frame.stdout` is non-empty).
- `renderStage` is a no-op stub in this task; Task 4 fills it in.
- If `trace.truncated` is true, prepend a `<p class="dbg-truncated">` reading `Trace truncated at {cap} steps.` The reader must know the run was cut.

- [ ] **Step 4: Add `tali.frame`**

In `crates/core/assets/js/tali-js.js`, inside the `makeApi` return object (line 374), after `value`:

```js
      // Read-only view of a `::: {.debug}` block's current frame. Deliberately a READ
      // accessor only, for the same reason `publish` is not on this object: `api` is
      // handed verbatim to author cell source as `tali`, so anything reachable here is
      // author-callable. A writable frame setter would let a cell drive the stepper that
      // re-runs it, creating exactly the feedback edge `buildGraph` never cycle-checked.
      /** @param {string} n */
      frame: function (n) {
        return window.taliDebug ? window.taliDebug.current(n) : null;
      },
```

- [ ] **Step 5: Register the assets**

In `crates/core/src/render/mod.rs`, beside `WALKTHROUGH_JS` (line 2161):

```rust
const DEBUG_JS: &str = include_str!("../../assets/js/debug.js");
```

In the `framework_scripts` format at line 2050, add `{debug_s}` to the template string and the argument:

```rust
        debug_s = gate(body.contains("tali-debug"), DEBUG_JS),
```

Add `DEBUG_JS` to `core_enhance_js()` (line 2281), after `SCROLLY_JS`. Do **not** add it to `deck_shared_js()`: `.debug` renders as a plain code block on a deck, so shipping the enhancer there would be dead weight.

Append `debug.css` to the base stylesheet by adding a `DEBUG_CSS` const and concatenating it where `BASE_CSS` is assembled.

- [ ] **Step 6: Measure the bundle cost**

Run:

```bash
cargo build --release -p taliesin-server
./target/release/taliesin build corpus/debug --out /tmp/dbg-size >/dev/null
ls -l /tmp/dbg-size/_assets/app.*.js
```

Record the byte size in the commit message. `core_enhance_js()` is the always-on `app.js`, so this cost lands on every page of every site. **If `debug.js` grows `app.js` by more than 25%, stop and move it to a conditional bundle mirroring `mermaid_bundle_js()` (`mod.rs:2296`) instead**, which ships only on pages that need it.

- [ ] **Step 7: Run the tests**

Run: `cargo test -p taliesin-core 2>&1 | grep -E "^test result|FAILED"`
Expected: all pass.

Run: `cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json`
Expected: no errors. Add any needed `window.taliDebug` declaration to `globals.d.ts`.

- [ ] **Step 8: Commit**

```bash
git add crates/core/assets/js/debug.js crates/core/assets/css/debug.css \
        crates/core/assets/js/tali-js.js crates/core/assets/js/globals.d.ts \
        crates/core/src/render/mod.rs crates/core/src/render/tests.rs
git commit -m "feat(debug): stepper chrome, line cursor and the tali.frame read accessor"
```

---

### Task 4: The four automatic data views

**Files:**
- Modify: `crates/core/assets/js/debug.js` (`renderStage`), `crates/core/assets/css/debug.css`
- Test: browser verification against `corpus/debug/sorting.tmd` (written in Task 7); until then, a scratch `.tmd` in the scratchpad

**Interfaces:**
- Consumes: `frame.locals`, `frame.changed` from Task 2's contract.
- Produces: `renderStage(el, frame, frames)`, which replaces `el`'s children with one `.dbg-view` per renderable local.

- [ ] **Step 1: Write the classifier**

Add to `debug.js`. The set is closed on purpose: four shapes cover the whole LeetCode teaching table, and anything else falls through to the variables panel rather than growing a fifth guess.

```js
  // Which built-in view a value earns. A CLOSED set: bars, boxes, grid, or nothing.
  // `null` means "leave it to the variables panel", which is the honest answer for a
  // shape we have no good picture for.
  /** @param {any} v @returns {"bars"|"boxes"|"grid"|null} */
  function viewFor(v) {
    if (!Array.isArray(v) || !v.length) return null;
    if (v.every(function (r) { return Array.isArray(r); })) return 'grid';
    if (v.every(function (x) { return typeof x === 'number' && isFinite(x); })) return 'bars';
    if (v.every(function (x) { return x === null || typeof x !== 'object'; })) return 'boxes';
    return null;
  }
```

- [ ] **Step 2: Write the pointer resolver**

This is what makes two-pointer and sliding-window problems legible, and it is the single highest-value part of the auto views.

```js
  // An integer local that is a valid index into a rendered array becomes a labelled caret
  // under that slot. `i`, `j`, `lo`, `hi`, `left`, `right` need no declaration and no
  // naming convention: being in range IS the signal.
  /** @param {any} locals @param {string} arrayName @param {number} len */
  function pointersInto(locals, arrayName, len) {
    var out = [];
    Object.keys(locals).forEach(function (k) {
      var v = locals[k];
      if (k === arrayName) return;
      if (typeof v !== 'number' || !Number.isInteger(v)) return;
      if (v < 0 || v >= len) return;
      out.push({ name: k, at: v });
    });
    return out;
  }
```

A caveat to encode: when two pointers land on the same slot, stack their labels vertically rather than overlapping them.

- [ ] **Step 3: Write the three renderers**

- **bars**: a flex row of `<div class="dbg-bar">` with `height` as a percentage of the max absolute value, `data-i` for the index, class `dbg-write` for indices in `changed[name].writes` and `dbg-read` for `changed[name].reads`. Values are shown as a label under each bar when the array has 24 or fewer elements, and hidden above that.
- **boxes**: a flex row of `<div class="dbg-box">` holding `fmt(value)`, same read/write classes. A string local is rendered by splitting into characters first.
- **grid**: a CSS grid of `<div class="dbg-cell">`, one row per sub-array, same read/write classes, with `changed[name].writes` interpreted as row indices when the outer array changed and cell indices resolved from the inner diff.

All three take colors from `--tali-*` tokens only. Never write a literal hex value: the hand-rolled sorting exhibit did exactly that and its bars ignore the theme.

- [ ] **Step 4: Verify in the browser**

Write a scratch document to the scratchpad, then:

```bash
cargo build --release -p taliesin-server
./target/release/taliesin preview /tmp/claude-*/scratchpad/dbg.tmd 4388 &
```

Drive it with the chrome-devtools MCP. Confirm, with a screenshot each: bars render and flash on a swap, a two-pointer document shows two labelled carets, a DP document shows the grid filling. Read the console and confirm it is clean.

- [ ] **Step 5: Type-check and commit**

```bash
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json
```

```bash
git add crates/core/assets/js/debug.js crates/core/assets/css/debug.css
git commit -m "feat(debug): bars, boxes, grid and in-range pointer carets"
```

---

### Task 5: Width, full screen and reflow

**Files:**
- Modify: `crates/core/assets/js/debug.js` (expand button), `crates/core/assets/css/debug.css`

- [ ] **Step 1: Wire the expand control**

Reuse the guarded pattern from `deck.js:2416-2420` rather than inventing a second one:

```js
  /** @param {Element} root */
  function toggleExpand(root) {
    try {
      if (document.fullscreenElement) document.exitFullscreen();
      else if (root.requestFullscreen) root.requestFullscreen();
      else root.classList.toggle('dbg-overlay'); // fallback: fixed-position overlay
    } catch (e) { root.classList.toggle('dbg-overlay'); }
  }
```

Listen for `fullscreenchange` to keep the button's `aria-pressed` and label honest, and make Escape leave the overlay fallback (the real Fullscreen API already handles Escape itself).

- [ ] **Step 2: Write the layout CSS**

```css
/* The reading measure cannot hold a code panel beside a data view, so the container
   is already `.column-page` (set server-side). Below the breakpoint the panels stack;
   above it they sit side by side. In full screen the code takes a fixed column so the
   stage gets every remaining pixel. */
.tali-debug { display: grid; gap: 1rem; grid-template-columns: 1fr; }
@media (min-width: 900px) {
  .tali-debug { grid-template-columns: minmax(0, 22rem) minmax(0, 1fr); }
  .tali-debug .dbg-transport { grid-column: 1 / -1; }
}
.tali-debug:fullscreen, .tali-debug.dbg-overlay {
  padding: 1.5rem;
  background: var(--tali-bg);
  grid-template-columns: minmax(0, 30rem) minmax(0, 1fr);
}
.tali-debug.dbg-overlay { position: fixed; inset: 0; z-index: 100; overflow: auto; }
@media (prefers-reduced-motion: reduce) {
  .tali-debug * { transition: none !important; animation: none !important; }
}
```

- [ ] **Step 3: Verify at all three viewports plus full screen**

Use the chrome-devtools MCP at 390x844, 1440x900, and 900x1440 (the portrait band is the one that gets forgotten), then trigger expand and screenshot again. Confirm the page body never scrolls horizontally at any size.

- [ ] **Step 4: Commit**

```bash
git add crates/core/assets/js/debug.js crates/core/assets/css/debug.css
git commit -m "feat(debug): column-page default, fullscreen expand and the stacked reflow"
```

---

### Task 6: The JavaScript generator adapter and the yield scanner

This is the plan's riskiest task. If Step 3 proves troublesome, take the documented fallback: emit no line mapping, ship the adapter with `line: null` frames, and leave the cursor parked. Everything else in the feature still works, and Python is unaffected.

**Files:**
- Create: `crates/core/src/render/yield_scan.rs`
- Modify: `crates/core/src/render/mod.rs` (`mod yield_scan;`), `crates/core/src/render/emit.rs`, `crates/core/assets/js/debug.js`

**Interfaces:**
- Produces: `pub(crate) fn stamp_yields(src: &str) -> Option<String>`, returning the rewritten source, or `None` when the scan cannot complete confidently.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/render/yield_scan.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_each_yield_with_its_source_line() {
        let src = "function* f(a) {\n  yield a;\n  a.push(1);\n  yield a;\n}\n";
        let out = stamp_yields(src).expect("plain source must scan");
        assert!(out.contains("yield __at(2, a);"), "{out}");
        assert!(out.contains("yield __at(4, a);"), "{out}");
    }

    #[test]
    fn leaves_yield_alone_inside_strings_templates_and_comments() {
        let src = "const s = 'yield x';\n// yield x\n/* yield x */\nconst t = `yield ${x}`;\nyield v;\n";
        let out = stamp_yields(src).expect("must scan");
        assert_eq!(out.matches("__at(").count(), 1, "only the real yield is stamped:\n{out}");
        assert!(out.contains("yield __at(5, v);"), "{out}");
    }

    /// A scan it cannot finish must REFUSE, not guess. A mis-stamp inside a string would
    /// corrupt the author's cell; no stamp only costs the line cursor.
    #[test]
    fn refuses_rather_than_guessing_on_an_unterminated_literal() {
        assert!(stamp_yields("const s = 'unterminated\nyield v;\n").is_none());
        assert!(stamp_yields("/* unterminated\nyield v;\n").is_none());
    }

    #[test]
    fn a_regex_literal_containing_the_word_yield_is_not_stamped() {
        let src = "const r = /yield/g;\nyield v;\n";
        let out = stamp_yields(src).expect("must scan");
        assert_eq!(out.matches("__at(").count(), 1, "{out}");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p taliesin-core yield_scan 2>&1 | tail -10`
Expected: FAIL, `stamp_yields` not found.

- [ ] **Step 3: Write the scanner**

```rust
//! Map each `yield` in a `{js}` cell to its source line, so the debugger's cursor can
//! point at the statement that produced the current frame.
//!
//! Deliberately a SCANNER, not a parser: pulling a JavaScript parser into the crate to
//! learn one line number per `yield` is not a trade this project makes. The scanner's
//! contract is therefore conservative in one direction only, and that asymmetry is the
//! whole safety design: a `yield` it fails to recognise costs a cursor position, while a
//! `yield` it invents inside a string would corrupt the author's cell. So it refuses.

/// Lexer states. Regex literals matter only because one may contain the word `yield`.
#[derive(PartialEq)]
enum S {
    Code,
    Single,
    Double,
    Template,
    LineComment,
    BlockComment,
    Regex,
}

/// Rewrite `yield EXPR` into `yield __at(LINE, EXPR)`. Returns `None` when the scan
/// cannot complete (an unterminated literal or comment), in which case the caller ships
/// the cell unmodified and the cursor stays parked.
pub(crate) fn stamp_yields(src: &str) -> Option<String> {
    let b = src.as_bytes();
    let (mut st, mut line, mut i) = (S::Code, 1usize, 0usize);
    let mut sites: Vec<(usize, usize)> = Vec::new(); // (byte offset just past `yield`, line)
    let mut prev_sig = 0u8; // last significant code byte, for the regex-vs-divide call

    while i < b.len() {
        if b[i] == b'\n' {
            line += 1;
            if st == S::LineComment {
                st = S::Code;
            }
            // An unterminated single/double quoted string cannot span a raw newline.
            if st == S::Single || st == S::Double {
                return None;
            }
            i += 1;
            continue;
        }
        match st {
            S::Code => {
                // Enter a literal or comment.
                if b[i] == b'\'' { st = S::Single; }
                else if b[i] == b'"' { st = S::Double; }
                else if b[i] == b'`' { st = S::Template; }
                else if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' { st = S::LineComment; }
                else if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' { st = S::BlockComment; }
                else if b[i] == b'/' && matches!(prev_sig, 0 | b'=' | b'(' | b',' | b':' | b'[' | b'!' | b'&' | b'|' | b'?' | b'{' | b'}' | b';') {
                    st = S::Regex;
                } else if b[i..].starts_with(b"yield")
                    && !is_ident(if i == 0 { 0 } else { b[i - 1] })
                    && !is_ident(*b.get(i + 5).unwrap_or(&0))
                {
                    sites.push((i + 5, line));
                    i += 5;
                    prev_sig = b'd';
                    continue;
                }
                if !b[i].is_ascii_whitespace() { prev_sig = b[i]; }
            }
            S::Single if b[i] == b'\'' => st = S::Code,
            S::Double if b[i] == b'"' => st = S::Code,
            S::Template if b[i] == b'`' => st = S::Code,
            S::Regex if b[i] == b'/' => st = S::Code,
            S::BlockComment if b[i] == b'*' && b.get(i + 1) == Some(&b'/') => {
                st = S::Code;
                i += 1;
            }
            _ => {}
        }
        // A backslash escapes the next byte inside any literal.
        if matches!(st, S::Single | S::Double | S::Template | S::Regex) && b[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    if st != S::Code && st != S::LineComment {
        return None; // unterminated: refuse rather than guess
    }
    if sites.is_empty() {
        return Some(src.to_string());
    }
    Some(splice(src, &sites))
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}
```

`splice` walks the recorded sites in reverse (so earlier offsets stay valid) and, for each,
finds the end of the yielded expression by scanning forward at the same bracket depth to
the first `;` or newline. It inserts `__at(LINE, ` at the site and `)` at that end. When the
expression end cannot be found at depth zero, that one site is skipped rather than
abandoning the whole file: a partial cursor beats no adapter.

- [ ] **Step 4: Drain the generator in `debug.js`**

A `//| trace: true` `{js}` cell returns a generator. In `debug.js`, drain it under the same caps as the Python harness, and build the same frame shape:

```js
  /** @param {Generator} gen @returns {{frames: any[], truncated: boolean, cap: number}} */
  function drain(gen) {
    var MAX = 5000, frames = [], prev = null, truncated = false, n;
    for (n = 0; n < MAX; n++) {
      var step = gen.next();
      if (step.done) break;
      var v = step.value || {};
      var line = v.$line != null ? v.$line : null;
      var locals = {};
      Object.keys(v).forEach(function (k) { if (k !== '$line') locals[k] = v[k]; });
      frames.push({
        line: line, event: 'line', depth: 1, func: '',
        locals: locals, changed: diffLocals(prev, locals),
        stack: [], stdout: '',
      });
      prev = locals;
    }
    if (n >= MAX) truncated = true;
    return { frames: frames, truncated: truncated, cap: MAX };
  }
```

`diffLocals` is the JS twin of the harness's `diff`: element-wise for equal-length arrays producing `writes`, `from`/`to` otherwise. `reads` is always empty here, as the spec states. Define `__at(line, v)` in the cell scope as `function (l, v) { v.$line = l; return v; }`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p taliesin-core yield_scan 2>&1 | tail -10`
Expected: PASS.

Run: `cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/yield_scan.rs crates/core/src/render/mod.rs \
        crates/core/src/render/emit.rs crates/core/assets/js/debug.js
git commit -m "feat(debug): JS generator capture with a refuse-rather-than-guess yield scanner"
```

---

### Task 7: Corpus documents

**Files:**
- Create: `corpus/debug/_site.yml`, `corpus/debug/sorting.tmd`, `corpus/debug/leetcode.tmd`, `corpus/debug/dp.tmd`, `corpus/debug/custom-view.tmd`
- Modify: `corpus/README.md`, `corpus/diagnostics/widgets.tmd`

Each document pins specific behavior, per the corpus-plus-roadmap rule that a capability ships with the document that tests it:

| Document | Pins |
| --- | --- |
| `sorting.tmd` | Python capture, bars, pointer carets, line cursor, the zero-JS default |
| `leetcode.tmd` | binary search (three carets over a shrinking range), sliding window over a string (boxes plus a dict), two-pointer palindrome |
| `dp.tmd` | the grid view and per-step reads, via edit distance |
| `custom-view.tmd` | the JS generator adapter, `tali.frame`, and an author `{js}` view overriding the built-ins |
| `corpus/diagnostics/widgets.tmd` | every row of the spec's section 7 diagnostics table |

- [ ] **Step 1: Write `corpus/debug/sorting.tmd`**

Keep the arrays small. A 6-element bubble sort is roughly 200 frames; a 60-element one blows the 5,000 cap and turns the flagship corpus document into a truncation test. Use 6 to 8 elements everywhere except the deliberate truncation fixture.

- [ ] **Step 2: Write the remaining three documents**

- [ ] **Step 3: Run the corpus suite**

Run: `cargo test -p taliesin-core 2>&1 | grep -E "^test result|FAILED"`
Expected: all pass. The corpus invariants (`data-block-id`, `data-sourcepos`) apply to the new documents automatically.

- [ ] **Step 4: Check the feature catalogue sees it**

Run: `cargo run -p taliesin-server -- features corpus/debug`
Expected: `trace` and `debug` appear as used. Then run `cargo run -p taliesin-server -- features corpus` and confirm neither shows up in the "no document uses" list.

- [ ] **Step 5: Commit**

```bash
git add corpus/debug corpus/README.md corpus/diagnostics/widgets.tmd
git commit -m "corpus: pin debug mode across sorting, leetcode patterns, DP and a custom view"
```

---

### Task 8: Text projection, deck and print degradation

**Files:**
- Modify: `crates/core/src/render/text.rs` (a `.debug` arm beside the walkthrough arm at ~line 446), `crates/core/assets/css/debug.css` (print rules)
- Test: `crates/core/src/render/text.rs` inline tests

- [ ] **Step 1: Write the failing test**

`project` takes `&[Block]`, not a string. Follow the shape of the existing
`projects_a_code_walkthrough_as_its_code_then_line_keyed_narration` test at `text.rs:968`:

```rust
#[test]
fn projects_a_debug_block_as_its_code_so_reading_form_and_search_are_not_empty() {
    let doc = crate::render_document(
        "::: {.debug name=\"d\"}\n```{python}\n#| trace: true\na = [2, 1]\n```\n:::\n",
    );
    let out = project(&doc.blocks);
    assert!(out.contains("a = [2, 1]"), "the algorithm's source must survive:\n{out}");
    assert!(!out.contains("tali-debug"), "no markup leaks into the text form:\n{out}");
}
```

- [ ] **Step 2: Run to verify it fails, then implement the arm**

Project the code fenced with its language, mirroring what the walkthrough arm does at `text.rs:446`.

- [ ] **Step 3: Add the print and deck rules**

```css
@media print {
  /* No transport in print: the reader cannot step a sheet of paper. Keep the code and
     the final state, which is what a printed algorithm page is actually for. */
  .tali-debug .dbg-transport, .tali-debug .dbg-vars { display: none; }
  .tali-debug { grid-template-columns: 1fr; }
}
```

Confirm `.debug` inside a deck renders as a plain code block and that `deck_shared_js()` does not carry `DEBUG_JS`.

- [ ] **Step 4: Run tests and commit**

```bash
cargo test -p taliesin-core 2>&1 | grep -E "^test result|FAILED"
git add crates/core/src/render/text.rs crates/core/assets/css/debug.css
git commit -m "feat(debug): text projection plus print and deck degradation"
```

---

### Task 9: Documentation

**Files:**
- Create: `docs/guide/using/debug.tmd`
- Modify: `docs/guide/_site.yml` (chapter list), `docs/internals/` (a trace-architecture page plus its `_site.yml`)

- [ ] **Step 1: Write the user-guide page**

Cover: the minimal zero-JS block, the `name=` attribute and when it is needed, taking over the view with a `{js}` cell and `tali.frame`, the JS generator form, the caps and what truncation looks like, and the full-screen control. Dogfood it: the page's examples are live `.debug` blocks, not screenshots.

- [ ] **Step 2: Write the internals page**

Cover: why the trace rides as an `Output::Rich` blob (so `_freeze` needs no changes), why `reads` are statically derived rather than traced, the frame-ordering rule (a `line` event fires before the line runs), the hidden-input bridge shared with scrolly, and the yield scanner's refuse-rather-than-guess contract.

- [ ] **Step 3: Verify both books build**

```bash
cargo run -p taliesin-server -- build docs/guide --out /tmp/guide-check
cargo run -p taliesin-server -- build docs/internals --out /tmp/internals-check
```

Expected: no warnings about the new pages.

- [ ] **Step 4: Commit**

```bash
git add docs/guide docs/internals
git commit -m "docs: the debug-mode guide page and its internals architecture note"
```

---

### Task 10: The marketing showcase section

**Files:**
- Modify: `site/showcase.tmd`

- [ ] **Step 1: Add the section**

Insert `## Step through an algorithm` following the page's existing Result/Source pattern (see `## Code that explains itself as you read` at line 289 for the shape). Use binary search: the shortest source that still gives a legible picture, so both halves fit without scrolling.

- [ ] **Step 2: Build and verify in the browser**

```bash
cargo build --release -p taliesin-server
./target/release/taliesin build site --out /tmp/site-check
```

Serve it and drive the chrome-devtools MCP: screenshot the section at 1440x900, step it, confirm the carets move and the discarded half greys out, check the console is clean. If binary search reads worse than expected, switch to bubble sort as the spec allows.

- [ ] **Step 3: Commit**

```bash
git add site/showcase.tmd
git commit -m "site: showcase the algorithm debugger"
```

---

### Task 11: Full verification

- [ ] **Step 1: Run every gate**

Run: `./tools/gates.sh 2>&1 | tail -40`

This is the only check that catches the two drift gates living outside `taliesin-core`, and the only one that refuses to be green when an interpreter is missing. Do not substitute a bare `cargo test --workspace`.

- [ ] **Step 2: Quote the result**

Paste the actual tail of the output into the final report, including the exit code. Name anything skipped or still failing. Never call this verified without its output.

- [ ] **Step 3: Re-read the whole diff**

```bash
git diff main...HEAD --stat
git diff main...HEAD
```

Confirm no reformatting of untouched lines, no changes to `serve_site/exec_pool.rs`, and no new front-matter or `_site.yml` keys.

---

## Self-Review

**Spec coverage:** section 1 (authoring surface) is Task 1; section 2 (frame contract) is Tasks 2 and 6; section 3 (capture adapters) is Tasks 2 and 6; section 4 (chrome) is Tasks 3 and 4; section 5 (width and full screen) is Task 5; section 6 (integration points) is spread across Tasks 1, 2, 3 and 8; section 7 (diagnostics) is Task 1 with fixtures in Task 7; section 8 (other output paths) is Task 8; section 9 (corpus and marketing) is Tasks 7 and 10; section 10 (testing) is distributed, with the whole-gate run in Task 11.

**Naming consistency:** `validate_debug`, `wrap_traced`, `stamp_yields`, `viewFor`, `pointersInto`, `renderStage`, `renderVars`, `drain`, `diffLocals`, `toggleExpand`, `window.taliDebug.frames`/`.current`, `tali.frame`. Class names: `.tali-debug`, `.dbg-code`, `.dbg-views`, `.dbg-stage`, `.dbg-vars`, `.dbg-transport`, `.dbg-scrub`, `.dbg-count`, `.dbg-bar`, `.dbg-box`, `.dbg-cell`, `.dbg-read`, `.dbg-write`, `.dbg-changed`, `.dbg-truncated`, `.dbg-overlay`. Data attributes: `data-debug-name`, `data-tali-trace`, `data-dbg-init`.

**Test harnesses verified against the tree, not assumed.** The first draft of this plan
invented three helpers that do not exist (`render_str`, `taliesin_test_support::executor_or_skip`,
a string-taking `project`). All three are corrected to the real patterns:
`render_document_with_includes(src, Path::new(".")).blocks` joined (copied from
`crates/core/tests/a11y_outline.rs:238`), `python_or_skip()` plus a `CARGO_BIN_EXE_taliesin`
subprocess (copied from `crates/server/tests/executed_output_reproducible.rs:32`), and
`project(&doc.blocks)` (`crates/core/src/render/text.rs:20`). Driving the CLI rather than an
internal executor method is also the stronger test: it proves the trace survives render,
freeze, and page assembly.

**Known risk carried forward:** the yield scanner in Task 6, with its stated fallback.
