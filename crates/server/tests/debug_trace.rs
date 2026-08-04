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

fn extract_trace(html: &str) -> Option<String> {
    let open = "<script type=\"application/json\" class=\"tali-debug-trace\">";
    let start = html.find(open)? + open.len();
    let end = html[start..].find("</script>")? + start;
    Some(html[start..end].replace("\\u003c", "<"))
}
