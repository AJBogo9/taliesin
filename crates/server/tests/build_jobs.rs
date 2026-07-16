//! `--jobs N` must actually build N pages in parallel.
//!
//! The regression this pins (M1, machine-facing audit 2026-07-16): the build docked 2
//! concurrency slots to the warm pool *before* resolving the interpreter, so it never
//! learned that `should_warm(Provenance::Default)` is false and no pool would boot. The
//! slots bought nothing, `--jobs 3` built ONE page at a time, and the only human-visible
//! signal — the log line — reported the loss as a purchase ("pre-warming 2 kernel(s)").
//! It fired on every build in the default configuration.
//!
//! `budget_split`'s own unit tests could not see it: they assert the arithmetic in
//! isolation, where it is correct. The defect is in the composition, so the test has to
//! be here, at the level the user's flag actually means something.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tali-jobs-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("temp dir");
    d
}

/// A small prose site: no code cells, so no kernel is ever needed and the run is fast.
fn write_site(root: &Path, pages: usize) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(root.join("_site.yml"), "title: Jobs probe\n").unwrap();
    std::fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
    for i in 0..pages {
        std::fs::write(
            root.join(format!("p{i}.tmd")),
            format!("---\ntitle: Page {i}\n---\n\nBody {i}.\n"),
        )
        .unwrap();
    }
}

/// Run a build and return its stderr (where `log::info` goes).
fn build_stderr(root: &Path, out: &Path, jobs: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args([
            "build",
            root.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--jobs",
            jobs,
        ])
        // The default configuration is the one that was broken: with no concrete
        // interpreter, `should_warm` is false and the pool never boots.
        .env_remove("TALIESIN_PYTHON")
        .output()
        .expect("run build");
    assert!(
        output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn explicit_jobs_is_not_docked_by_a_warm_pool_that_never_boots() {
    let base = tmp_dir("explicit");
    let root = base.join("src");
    write_site(&root, 6);

    // `--jobs 3` means three parallel pages (`main.rs`: "--jobs <N>  max parallel pages").
    // It used to mean one.
    let err = build_stderr(&root, &base.join("out3"), "3");
    assert!(
        err.contains("up to 3 parallel page(s)"),
        "--jobs 3 must build 3 pages in parallel, not fewer:\n{err}"
    );
    // And it must not claim to have spent the difference on kernels it never warmed.
    assert!(
        !err.contains("pre-warming 2 kernel(s)"),
        "must not report pre-warming a pool that cannot boot:\n{err}"
    );

    // The boundary the old arithmetic collapsed hardest: 2 -> 1 (fully sequential).
    let err = build_stderr(&root, &base.join("out2"), "2");
    assert!(
        err.contains("up to 2 parallel page(s)"),
        "--jobs 2 must not collapse to sequential:\n{err}"
    );

    // `--jobs 1` is sequential by definition and must stay so.
    let err = build_stderr(&root, &base.join("out1"), "1");
    assert!(
        err.contains("up to 1 parallel page(s)"),
        "--jobs 1 stays sequential:\n{err}"
    );

    let _ = std::fs::remove_dir_all(&base);
}
