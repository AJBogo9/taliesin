//! Live `{r}` cell execution against a real IRkernel.
//!
//! Why this file exists: `{r}` cells and `TALIESIN_R` are advertised as first-class in
//! the README, but **nothing exercised the R path**. CI installs only Python; there was
//! no `setup-r` step, no `TALIESIN_R`-gated test anywhere, and no assertion on
//! `KernelSpec::r()`'s argv. Meanwhile the Python side has a canary that HARD-FAILS if
//! its interpreter goes missing (`TALIESIN_REQUIRE_KERNEL`), built precisely so that
//! coverage could never silently regress to zero. The guard existed for the language the
//! author looks at; this is the same guard for the one he doesn't.
//!
//! Gated on `TALIESIN_R` like the rest of the exec tests. `TALIESIN_REQUIRE_R=1` (set by
//! the CI `kernel-r` job) turns the skip into a hard failure.

use std::path::PathBuf;
use std::process::Command;

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tali-r-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("temp dir");
    d
}

/// `Some(program)` when an R interpreter is configured, `None` to skip — unless
/// `TALIESIN_REQUIRE_R=1`, which makes a missing interpreter a hard failure so this
/// coverage cannot quietly die the way the pre-`TALIESIN_REQUIRE_KERNEL` exec tests could.
fn r_program() -> Option<String> {
    match std::env::var("TALIESIN_R") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_R").is_none(),
                "TALIESIN_REQUIRE_R=1 but TALIESIN_R is unset: the R execution path would \
                 silently go untested, which is exactly what this gate exists to prevent"
            );
            eprintln!("skipping: TALIESIN_R not set (no R kernel)");
            None
        }
    }
}

/// A real `{r}` cell executes and its stdout is spliced into the built page. This is the
/// R twin of the Python canary: it proves the IRkernel argv, the ZMQ handshake, stdout
/// capture, and warm state across cells all work, not just that R is installed.
#[test]
fn r_cells_execute_and_persist_state_across_cells() {
    let Some(program) = r_program() else { return };
    let dir = tmp_dir("exec");
    let doc = dir.join("doc.tmd");
    std::fs::write(
        &doc,
        "---\ntitle: R probe\n---\n\n\
         ```{r}\nx <- 6 * 7\ncat(\"answer\", x, \"\\n\")\n```\n\n\
         ```{r}\n# the warm kernel must still hold `x` from the cell above\ncat(\"still\", x, \"\\n\")\n```\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", doc.to_str().unwrap()])
        .env("TALIESIN_R", &program)
        .env("TALIESIN_NO_CACHE", "1") // never let a freeze hit stand in for real execution
        .output()
        .expect("run build");
    assert!(
        out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let html = std::fs::read_to_string(dir.join("doc.html")).expect("built page");
    assert!(
        html.contains("answer 42"),
        "the {{r}} cell must actually execute; stderr was: {}\n",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        html.contains("still 42"),
        "the warm R kernel must keep `x` across cells (state persistence)"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// An R error surfaces as a diagnostic instead of failing silently or wedging the build.
#[test]
fn an_r_error_is_reported_not_swallowed() {
    let Some(program) = r_program() else { return };
    let dir = tmp_dir("err");
    let doc = dir.join("doc.tmd");
    std::fs::write(
        &doc,
        "---\ntitle: R error probe\n---\n\n```{r}\nstop(\"deliberate failure\")\n```\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", doc.to_str().unwrap()])
        .env("TALIESIN_R", &program)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("run build");
    let html = std::fs::read_to_string(dir.join("doc.html")).unwrap_or_default();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        html.contains("deliberate failure") || stderr.contains("deliberate failure"),
        "an R error must surface (page or console), not vanish.\nstderr: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
