//! Live-kernel test for the `#| trace: true` harness. Gated the same way the other kernel
//! suites are: without a Python kernel this is a vacuous pass, so `./tools/gates.sh` arms
//! `TALIESIN_REQUIRE_KERNEL` and asserts this canary by name.

use std::fs;
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
    assert!(
        out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8_lossy(&out.stdout).into_owned();

    let json = extract_trace(&html).expect("a traced cell must embed a trace blob");
    let t: serde_json::Value = serde_json::from_str(&json).expect("the blob must be valid JSON");
    let frames = t["frames"].as_array().expect("frames array");

    assert!(
        frames.len() >= 4,
        "one frame per executed line, got {}",
        frames.len()
    );
    assert_eq!(
        frames[0]["line"], 1,
        "the first frame is the line ABOUT to run"
    );
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
        .find(|f| {
            f["changed"]["a"]["writes"]
                .as_array()
                .is_some_and(|w| !w.is_empty())
        })
        .expect("the swap must surface as a write to `a`");
    let writes = swapped["changed"]["a"]["writes"].as_array().unwrap();
    assert_eq!(writes.len(), 2, "a swap writes two slots: {writes:?}");

    // `reads` cannot come from settrace at all; they come from the per-line Subscript
    // scan. Line 3 is `if a[i] > a[i+1]:` with i == 0, so the derived read set is
    // exactly {0, 1}. This is the assertion that proves the static derivation runs:
    // without it the whole `reads` half of the frame contract is untested.
    let compare = frames
        .iter()
        .find(|f| f["line"] == 3 && f["locals"]["i"] == 0)
        .expect("a frame sitting on the comparison line with i == 0");
    let mut reads: Vec<i64> = compare["changed"]["a"]["reads"]
        .as_array()
        .expect("the comparison line must report derived reads on `a`")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    reads.sort();
    assert_eq!(
        reads,
        vec![0, 1],
        "`a[i] > a[i+1]` with i == 0 reads slots 0 and 1"
    );

    assert_eq!(t["truncated"], false);
}

/// Regression pin for `snapshot()`'s aliasing bug: a ONE-LEVEL copy of a nested list
/// (`dp[i][j] = x` on a list-of-lists) still shares its ROW objects with the previous
/// frame's snapshot, so mutating a row in place retroactively mutates what the
/// "previous" snapshot looks like too, and `diff()` compares a row against itself,
/// never reporting a write. Found while building Task 4's grid view (which reads
/// exactly this field to know which DP cell to highlight): without the fix below, a
/// grid's write highlighting is silently dead for every nested structure, forever.
/// Reproduced by hand outside the kernel first (see the task's report), then pinned
/// here against the real harness.
#[test]
fn a_nested_list_write_surfaces_as_a_row_level_write_not_a_silent_no_op() {
    let Some(py) = python_or_skip() else { return };

    let dir = std::env::temp_dir().join(format!("tali-debug-grid-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tmd");
    fs::write(
        &src,
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\n\
         dp = [[0, 0], [0, 0]]\n\
         dp[1][0] = 9\n```\n:::\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", src.to_str().unwrap(), "--stdout"])
        .env("TALIESIN_PYTHON", &py)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("build must run");
    assert!(
        out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8_lossy(&out.stdout).into_owned();
    let json = extract_trace(&html).expect("a traced cell must embed a trace blob");
    let t: serde_json::Value = serde_json::from_str(&json).expect("the blob must be valid JSON");
    let frames = t["frames"].as_array().expect("frames array");

    let wrote = frames
        .iter()
        .find(|f| {
            f["changed"]["dp"]["writes"]
                .as_array()
                .is_some_and(|w| !w.is_empty())
        })
        .expect(
            "the row-1 mutation must surface as a write to `dp`, not silently vanish \
             because the previous snapshot's row object was mutated in place",
        );
    let writes = wrote["changed"]["dp"]["writes"].as_array().unwrap();
    assert_eq!(
        writes,
        &vec![serde_json::json!(1)],
        "only row 1 changed: {writes:?}"
    );

    // The frame BEFORE the write must still show row 1 at its OLD value: if `snapshot`
    // aliased the row, this frame's own recorded `locals.dp` (encoded independently via
    // `enc`, not reused from `prev`) would still be right even when `diff` was wrong, so
    // this assertion alone would not have caught the bug -- it exists to document that
    // the frame data itself was never the problem, only the write diff was.
    let before = frames
        .iter()
        .find(|f| f["locals"]["dp"] == serde_json::json!([[0, 0], [0, 0]]))
        .expect("a frame must show dp before the mutation");
    // Line numbers count the STRIPPED cell source (`#| trace: true` is removed before
    // this line-numbers-from-1 code is compiled, same convention the other test in this
    // file pins at `frames[0]["line"] == 1`): line 1 is the assignment, line 2 the
    // mutation, so "about to run line 2" is exactly the frame sitting on the old value.
    assert_eq!(before["line"], 2, "the pre-mutation frame sits on line 2");
}

/// Regression pin for the ctx-blind `Subscript` scan: an assignment TARGET is a
/// `Subscript` node too, and without checking `node.ctx` is `ast.Load` it counted as a
/// "read" of the slot it is about to overwrite. A temp-variable swap makes this concrete:
/// `a[1] = tmp` (the third line) never reads array `a` at all (`tmp` is a plain scalar),
/// yet the unfiltered scanner reported `a[1]` as read on the frame sitting just before
/// that line runs -- one step ahead of the write it is actually about to become. Found
/// while investigating Task 4's deferred "does a swap flash both read and write" defect:
/// it turned out the literal tuple-swap example given for that defect is NOT buggy (its
/// RHS genuinely reads both slots via their own separate `Load`-context Subscript nodes,
/// unaffected by this filter either way, see the next test), but this DIFFERENT code
/// shape is a real, independent instance of the same "target counts as a read" root
/// cause, discovered by the same investigation.
#[test]
fn an_assignment_target_no_longer_counts_as_a_read_of_the_slot_it_is_about_to_overwrite() {
    let Some(py) = python_or_skip() else { return };

    let dir = std::env::temp_dir().join(format!("tali-debug-ctx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tmd");
    fs::write(
        &src,
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\n\
         a = [2, 1]\n\
         tmp = a[0]\n\
         a[0] = a[1]\n\
         a[1] = tmp\n```\n:::\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", src.to_str().unwrap(), "--stdout"])
        .env("TALIESIN_PYTHON", &py)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("build must run");
    assert!(
        out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8_lossy(&out.stdout).into_owned();
    let json = extract_trace(&html).expect("a traced cell must embed a trace blob");
    let t: serde_json::Value = serde_json::from_str(&json).expect("the blob must be valid JSON");
    let frames = t["frames"].as_array().expect("frames array");

    // Line 3 (`a[0] = a[1]`) genuinely reads index 1 (the RHS is a real `Load`): that
    // read must still show up on the frame sitting on line 3.
    let line3 = frames
        .iter()
        .find(|f| f["line"] == 3)
        .expect("a frame must sit on line 3");
    let reads3: Vec<i64> = line3["changed"]["a"]["reads"]
        .as_array()
        .expect("line 3 has a genuine read of a[1]")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(reads3, vec![1], "`a[0] = a[1]` reads slot 1: {line3:?}");

    // Line 4 (`a[1] = tmp`) reads NOTHING from `a` -- `tmp` is a plain scalar, not a
    // subscript -- so the frame sitting on line 4 must show no read on `a` at all,
    // regardless of whatever `a` diff (from line 3's write) also landed on this frame.
    let line4 = frames
        .iter()
        .find(|f| f["line"] == 4)
        .expect("a frame must sit on line 4");
    let a4 = &line4["changed"]["a"];
    assert!(
        a4["reads"].as_array().is_none_or(|r| r.is_empty()),
        "`a[1] = tmp` must not report a phantom read of `a`, since tmp is not a \
         subscript: {line4:?}"
    );
}

/// Companion to the fix above: the ctx filter must NOT suppress a genuinely dual
/// read-and-write on the SAME slot. A tuple-swap one-liner's RHS (`a[j+1], a[j]` in
/// `a[j], a[j+1] = a[j+1], a[j]`) is its own pair of `Subscript(ctx=Load)` nodes at the
/// SAME indices as the assignment targets, so those reads are real, not a target
/// masquerading as one, and must survive the fix unchanged. This only coincides on one
/// frame when the swap is the LAST statement the tracer sees (a `return` event at that
/// exact line); mid-trace, the write always lands on the NEXT frame instead (verified
/// separately, see the task report), so this test pins the return-event edge case where
/// the coincidence is real and observable.
#[test]
fn a_tuple_swaps_genuine_dual_read_and_write_survives_the_ctx_filter() {
    let Some(py) = python_or_skip() else { return };

    let dir = std::env::temp_dir().join(format!("tali-debug-tupleswap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tmd");
    fs::write(
        &src,
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\n\
         a = [2, 1]\n\
         a[0], a[1] = a[1], a[0]\n```\n:::\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", src.to_str().unwrap(), "--stdout"])
        .env("TALIESIN_PYTHON", &py)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("build must run");
    assert!(
        out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8_lossy(&out.stdout).into_owned();
    let json = extract_trace(&html).expect("a traced cell must embed a trace blob");
    let t: serde_json::Value = serde_json::from_str(&json).expect("the blob must be valid JSON");
    let frames = t["frames"].as_array().expect("frames array");

    let last = frames.last().expect("at least one frame");
    assert_eq!(
        last["event"], "return",
        "the swap is the last statement traced"
    );
    let mut writes: Vec<i64> = last["changed"]["a"]["writes"]
        .as_array()
        .expect("the return-event frame must show the swap's writes")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    writes.sort();
    let mut reads: Vec<i64> = last["changed"]["a"]["reads"]
        .as_array()
        .expect("the return-event frame must ALSO show the swap's genuine reads")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    reads.sort();
    reads.dedup();
    assert_eq!(writes, vec![0, 1], "both slots were written: {last:?}");
    assert_eq!(
        reads,
        vec![0, 1],
        "both slots were ALSO genuinely read (the RHS reads the old values before the \
         LHS overwrites them), so this is not the ctx-blind bug: {last:?}"
    );
}

/// Regression pin for a second edge the ctx=Load filter must NOT break: `a[0] += 5` is
/// `AugAssign(target=Subscript(ctx=Store))`, so the plain-Store exclusion that fixed the
/// temp-variable swap would ALSO wrongly drop this read if applied uniformly, and it
/// would be wrong to drop it: an augmented assignment genuinely reads its target before
/// writing it (`a[0] += 5` behaves like `a[0] = a[0] + 5`), and `counts[x] += 1` is one
/// of the most common idioms in exactly the kind of algorithm this feature exists to
/// visualize (frequency counting, histogram building). Found by review, reproduced
/// against the harness extracted to a standalone script before touching the fix.
#[test]
fn an_augmented_assignment_target_still_counts_as_a_read() {
    let Some(py) = python_or_skip() else { return };

    let dir = std::env::temp_dir().join(format!("tali-debug-augassign-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tmd");
    fs::write(
        &src,
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\n\
         a = [1, 2]\n\
         a[0] += 5\n```\n:::\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", src.to_str().unwrap(), "--stdout"])
        .env("TALIESIN_PYTHON", &py)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("build must run");
    assert!(
        out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8_lossy(&out.stdout).into_owned();
    let json = extract_trace(&html).expect("a traced cell must embed a trace blob");
    let t: serde_json::Value = serde_json::from_str(&json).expect("the blob must be valid JSON");
    let frames = t["frames"].as_array().expect("frames array");

    // Line 2 (`a[0] += 5`) is the frame sitting just before that line runs: this is
    // where the read is derived (`reads_for` uses the CURRENT frame's own upcoming
    // line), so it must show a read of index 0, not an empty read set.
    let line2 = frames
        .iter()
        .find(|f| f["line"] == 2)
        .expect("a frame must sit on line 2");
    let reads: Vec<i64> = line2["changed"]["a"]["reads"]
        .as_array()
        .expect("`a[0] += 5` must report a genuine read of a[0]")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(reads, vec![0], "`a[0] += 5` reads slot 0: {line2:?}");

    // And the write must still show up on the following frame, same as any other write.
    let wrote = frames
        .iter()
        .find(|f| {
            f["changed"]["a"]["writes"]
                .as_array()
                .is_some_and(|w| !w.is_empty())
        })
        .expect("the augmented assignment must surface as a write to `a`");
    let writes = wrote["changed"]["a"]["writes"].as_array().unwrap();
    assert_eq!(writes, &vec![serde_json::json!(0)], "{wrote:?}");
}

/// Regression pin for the freeze cache blind-spotting `#| trace: true`. `compute_outputs`
/// used to hash `cell.code` alone; `#| trace: true` is a directive line
/// `strip_cell_options` already strips out of `cell.code` before it is ever hashed or
/// executed, so toggling the flag left the cumulative hash unchanged. A cell already
/// cached UNTRACED then reported a cache hit on the traced re-run and silently replayed
/// the old, trace-free output: a `.debug` block that finds no trace, with no error, in
/// the primary authoring workflow (edit a cell, add `#| trace: true`, expect a trace).
///
/// Runs with the freeze cache ACTIVE (no `TALIESIN_NO_CACHE`) end to end through the real
/// `build` CLI, twice, against the SAME file path and the SAME `_freeze/` directory, so
/// this proves the on-disk cache key itself discriminates rather than only in-memory
/// per-process state.
#[test]
fn toggling_trace_on_a_cached_cell_busts_the_key_instead_of_replaying_the_old_output() {
    let Some(py) = python_or_skip() else { return };

    let dir = std::env::temp_dir().join(format!("tali-debug-cachebust-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tmd");

    let untraced = "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\nx = 1\nprint(x)\n```\n:::\n";
    fs::write(&src, untraced).unwrap();

    // Run 1: no trace flag, freeze cache ACTIVE (no TALIESIN_NO_CACHE). This is the run
    // that must populate `_freeze/t.json`.
    let out1 = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", src.to_str().unwrap(), "--stdout"])
        .env("TALIESIN_PYTHON", &py)
        .output()
        .expect("build must run");
    assert!(
        out1.status.success(),
        "first (untraced) build failed: {}",
        String::from_utf8_lossy(&out1.stderr)
    );
    let html1 = String::from_utf8_lossy(&out1.stdout).into_owned();
    assert!(
        extract_trace(&html1).is_none(),
        "the untraced run must not emit a trace blob"
    );

    // Confirm the cache was actually populated by run 1, not just assumed: read
    // `_freeze/t.json` back and check it holds a real entry, and that the cached value
    // itself is the untraced output (no trace blob baked into what got persisted).
    let freeze_path = dir.join("_freeze").join("t.json");
    let freeze_bytes = fs::read(&freeze_path)
        .unwrap_or_else(|e| panic!("run 1 must have written {}: {e}", freeze_path.display()));
    let on_disk: serde_json::Value =
        serde_json::from_slice(&freeze_bytes).expect("_freeze/t.json must be valid JSON");
    let entries = on_disk["entries"]
        .as_array()
        .expect("_freeze/t.json must have an entries array");
    assert!(
        !entries.is_empty(),
        "run 1 must have cached at least one cell, got {} entries",
        entries.len()
    );
    assert!(
        entries.iter().all(|e| !e["v"]
            .as_str()
            .unwrap_or_default()
            .contains("tali-debug-trace")),
        "the entry cached by the untraced run must not itself hold a trace blob"
    );

    // Run 2: SAME file, SAME freeze dir, only `#| trace: true` added. `strip_cell_options`
    // removes that directive line before it reaches the hashed/executed code, so the
    // cell's `code` text is byte-identical between the two runs; only `traced` differs.
    let traced = "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\nx = 1\nprint(x)\n```\n:::\n";
    fs::write(&src, traced).unwrap();

    let out2 = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", src.to_str().unwrap(), "--stdout"])
        .env("TALIESIN_PYTHON", &py)
        .output()
        .expect("build must run");
    assert!(
        out2.status.success(),
        "second (traced) build failed: {}",
        String::from_utf8_lossy(&out2.stderr)
    );
    let html2 = String::from_utf8_lossy(&out2.stdout).into_owned();
    assert!(
        extract_trace(&html2).is_some(),
        "toggling `#| trace: true` on a cell already cached untraced must still produce \
         a trace blob instead of replaying the cached untraced output"
    );
}

/// A traced cell that RAISES must behave like any other failing cell: a `tali-error`
/// output on the page, a non-zero `build --strict`, and nothing persisted to `_freeze/`.
///
/// The first harness caught the exception into an "exception" frame and returned normally,
/// so the kernel reported a clean success. Reproduced before fixing: `build --strict` on a
/// `.debug` block whose body is `b = a[5]` exited **0** with zero warnings, and
/// `_freeze/t.json` cached the errored output, against the documented invariant that an
/// error is never persisted. The same cell WITHOUT `trace:` warned and was never cached,
/// which is the whole point: `trace:` is not supposed to change what a cell IS.
///
/// The trace itself must survive the fix, so this also pins that the frames recorded
/// before the raise still reach the page: a stepper that shows the reader exactly where
/// the algorithm blew up is the most useful thing a debugger can do with a crash.
#[test]
fn a_traced_cell_that_raises_surfaces_the_error_and_is_never_cached() {
    let Some(py) = python_or_skip() else { return };

    let dir = std::env::temp_dir().join(format!("tali-debug-raise-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tmd");
    fs::write(
        &src,
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\n\
         a = [1, 2]\n\
         b = a[5]\n```\n:::\n",
    )
    .unwrap();

    // Freeze cache ACTIVE on purpose (no `TALIESIN_NO_CACHE`): the never-persist-an-error
    // rule is the half that regressed, and it only exists when the cache is live.
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", src.to_str().unwrap(), "--stdout"])
        .env("TALIESIN_PYTHON", &py)
        .output()
        .expect("build must run");
    let html = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        html.contains("class=\"tali-error\""),
        "an IndexError inside a traced cell must reach the page as a `tali-error` output, \
         exactly like an untraced cell's does: {html}"
    );
    assert!(
        html.contains("IndexError"),
        "the reader must get the interpreter's own message, not a silent widget: {html}"
    );

    // The frames recorded before the raise still ship.
    let json = extract_trace(&html).expect("the frames recorded before the raise must ship");
    let t: serde_json::Value = serde_json::from_str(&json).expect("the blob must be valid JSON");
    let frames = t["frames"].as_array().expect("frames array");
    assert!(
        frames.len() >= 2,
        "line 1 ran before line 2 raised, so at least two frames were recorded: {frames:?}"
    );

    // Nothing errored is persisted. `_freeze/t.json` may or may not be written at all
    // (that depends on whether any cell was cacheable); either way it must hold no entry
    // for this cell.
    let freeze_path = dir.join("_freeze").join("t.json");
    if let Ok(bytes) = fs::read(&freeze_path) {
        let on_disk: serde_json::Value =
            serde_json::from_slice(&bytes).expect("_freeze/t.json must be valid JSON");
        let entries = on_disk["entries"].as_array().cloned().unwrap_or_default();
        assert!(
            entries.is_empty(),
            "an errored traced cell must never be persisted, got {entries:?}"
        );
    }

    // And `--strict` (the CI gate) must fail on it.
    let strict = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", src.to_str().unwrap(), "--stdout", "--strict"])
        .env("TALIESIN_PYTHON", &py)
        .output()
        .expect("build must run");
    assert!(
        !strict.status.success(),
        "`build --strict` must exit non-zero on a traced cell that raised; stderr: {}",
        String::from_utf8_lossy(&strict.stderr)
    );
}

/// A traced cell shares the document's namespace: it can read what an earlier, UNTRACED
/// cell bound, and what it binds is there for the cells after it.
///
/// The first harness ran the author's code in a private `ns = {}`, so `data.sort()` after
/// an upstream `data = [3, 1, 2]` raised `NameError` *inside* the harness, was swallowed
/// by the same `except` the test above covers, and rendered as a widget with three empty
/// frames and no explanation at all.
#[test]
fn a_traced_cell_sees_the_namespace_an_earlier_cell_built() {
    let Some(py) = python_or_skip() else { return };

    let dir = std::env::temp_dir().join(format!("tali-debug-ns-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.tmd");
    fs::write(
        &src,
        "---\ntitle: T\n---\n\n\
         ```{python}\ndata = [3, 1, 2]\n```\n\n\
         ::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\n\
         data.sort()\n```\n:::\n\n\
         ```{python}\nprint(\"after\", data)\n```\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", src.to_str().unwrap(), "--stdout"])
        .env("TALIESIN_PYTHON", &py)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("build must run");
    assert!(
        out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        !html.contains("NameError"),
        "the traced cell must see `data` from the cell above it: {html}"
    );

    let json = extract_trace(&html).expect("a traced cell must embed a trace blob");
    let t: serde_json::Value = serde_json::from_str(&json).expect("the blob must be valid JSON");
    let frames = t["frames"].as_array().expect("frames array");
    assert!(
        frames
            .iter()
            .any(|f| f["locals"]["data"] == serde_json::json!([3, 1, 2])),
        "a frame must show `data` at the value the EARLIER cell bound: {frames:?}"
    );
    assert!(
        frames
            .iter()
            .any(|f| f["locals"]["data"] == serde_json::json!([1, 2, 3])),
        "and a later frame must show it sorted, so the mutation happened for real: {frames:?}"
    );

    // The mutation is visible DOWNSTREAM too: the cell after the `.debug` div prints the
    // sorted list, which is only possible if the traced cell wrote to the shared namespace
    // rather than to a private dict.
    assert!(
        html.contains("after [1, 2, 3]"),
        "a cell after the traced one must see the mutation: {html}"
    );

    // The kernel's own bookkeeping (`In`, `Out`, `get_ipython`, the harness function
    // itself) must NOT flood the variables panel now that the namespace is shared.
    let noise: Vec<&str> = frames
        .iter()
        .filter_map(|f| f["locals"].as_object())
        .flat_map(|o| o.keys())
        .map(String::as_str)
        .filter(|k| *k != "data")
        .collect();
    assert!(
        noise.is_empty(),
        "only the names the traced source mentions belong in a frame's locals, got {noise:?}"
    );
}

fn extract_trace(html: &str) -> Option<String> {
    let open = "<script type=\"application/json\" class=\"tali-debug-trace\">";
    let start = html.find(open)? + open.len();
    let end = html[start..].find("</script>")? + start;
    Some(html[start..end].replace("\\u003c", "<"))
}
