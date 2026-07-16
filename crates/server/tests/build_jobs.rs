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
//!
//! M1's sibling (the owner's `--jobs` semantics ruling, 2026-07-17) is pinned by
//! `explicit_jobs_is_honored_when_a_warm_pool_boots` below: `--jobs N` means N parallel
//! PAGES, so under an explicit `--jobs` the warm pool is ADDITIVE and must not dock the
//! user's number. That case needs a real interpreter, so it is kernel-gated — which is
//! precisely why M1's fix could land without noticing it was only half the bug.

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

/// Run a build and return its stderr (where `log::info` goes). `python` picks the
/// configuration under test: `None` removes `TALIESIN_PYTHON` (bare default →
/// `should_warm` false → no pool boots), `Some(p)` names a concrete interpreter
/// (provenance `Env` → the pool boots).
fn build_stderr_with(root: &Path, out: &Path, jobs: &str, python: Option<&str>) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_taliesin"));
    cmd.args([
        "build",
        root.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--jobs",
        jobs,
    ]);
    match python {
        Some(p) => cmd.env("TALIESIN_PYTHON", p),
        None => cmd.env_remove("TALIESIN_PYTHON"),
    };
    let output = cmd.output().expect("run build");
    assert!(
        output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The default configuration is the one M1 broke: with no concrete interpreter,
/// `should_warm` is false and the pool never boots.
fn build_stderr(root: &Path, out: &Path, jobs: &str) -> String {
    build_stderr_with(root, out, jobs, None)
}

/// A concrete interpreter with a bootable forkserver, or `None` to skip.
///
/// Gated on `TALIESIN_PYTHON` like the rest of the exec tests; `TALIESIN_REQUIRE_KERNEL=1`
/// (set by CI's kernel job) makes a missing interpreter a HARD FAIL, so this coverage
/// cannot quietly die the way the pre-`TALIESIN_REQUIRE_KERNEL` exec tests could. The
/// pool-booted arithmetic is exactly the half M1 could not see, so it must not be allowed
/// to silently reach zero coverage a second time.
fn concrete_python() -> Option<String> {
    match std::env::var("TALIESIN_PYTHON") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL=1 but TALIESIN_PYTHON is unset: the pool-booted \
                 --jobs path would silently skip"
            );
            eprintln!(
                "skipping warm-pool --jobs test: TALIESIN_PYTHON unset \
                 (set TALIESIN_REQUIRE_KERNEL=1 to enforce)"
            );
            None
        }
    }
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

/// The owner's ruling (2026-07-17): `--jobs N` means N parallel PAGES, so a booted warm
/// pool is ADDITIVE and must not dock N. This is M1's other half — the case M1 left open
/// pending the ruling, and the one its no-pool test could not reach.
///
/// Measured before the fix, with a real interpreter and a live forkserver:
///   `--jobs 3` → "building with up to 1 parallel page(s); pre-warming 2 kernel(s)"
/// i.e. the CLI's own promise ("max parallel pages") off by 3x, silently.
#[test]
fn explicit_jobs_is_honored_when_a_warm_pool_boots() {
    let Some(python) = concrete_python() else {
        return;
    };
    let base = tmp_dir("warm");
    let root = base.join("src");
    // Prose pages: the pool boots off the resolved interpreter regardless of cells, so
    // this exercises the split without paying for any execution.
    write_site(&root, 6);

    // The pool boots (concrete interpreter), and `--jobs 3` still means three pages.
    let err = build_stderr_with(&root, &base.join("out3"), "3", Some(&python));
    assert!(
        err.contains("up to 3 parallel page(s)"),
        "--jobs 3 must build 3 pages in parallel even with a pool booted:\n{err}"
    );
    // The pool is additive, not a tax: it still pre-warms its full target.
    assert!(
        err.contains("pre-warming 2 kernel(s)"),
        "a booted pool must still report its warm kernels:\n{err}"
    );

    // `--jobs 1` is the sharpest case: `budget_split`'s "cap 1 → no warm pool" rule was a
    // SHARED-budget rule. Under an explicit --jobs the two budgets are separate, so the
    // user gets their 1 sequential page AND a full warm pool (the same deal a serially
    // building preview already gets from `preview_warm_pool_size`).
    let err = build_stderr_with(&root, &base.join("out1"), "1", Some(&python));
    assert!(
        err.contains("up to 1 parallel page(s)"),
        "--jobs 1 stays sequential:\n{err}"
    );
    assert!(
        err.contains("pre-warming 2 kernel(s)"),
        "--jobs 1 must still pre-warm: the pool no longer shares the build's budget:\n{err}"
    );

    let _ = std::fs::remove_dir_all(&base);
}

/// Parse the page count out of "building with up to N parallel page(s)".
fn parallel_pages(err: &str) -> usize {
    let tail = err
        .split("building with up to ")
        .nth(1)
        .unwrap_or_else(|| panic!("no build log line:\n{err}"));
    tail.split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap_or_else(|_| panic!("unparsable page count:\n{err}"))
}

/// Parse the warm count out of "pre-warming N kernel(s)"; `0` when the line is absent
/// (no pool booted, which the log deliberately says nothing about).
fn pre_warmed(err: &str) -> usize {
    match err.split("pre-warming ").nth(1) {
        None => 0,
        Some(tail) => tail
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap_or_else(|_| panic!("unparsable warm count:\n{err}")),
    }
}

/// Auto mode (no `--jobs`) is the path the ruling deliberately does NOT change: there the
/// cap is ours to spend, the memory budget is real, and `warm_pool + build_kernels <= cap`
/// must keep holding — while M1's rule still applies, so a pool that never boots hands its
/// slots back rather than forfeiting them.
///
/// Asserted as a *relationship* between two runs on the same machine, not against a literal
/// number, so it is independent of core count and free RAM. (Measured here: 16 vs 14+2.)
#[test]
fn auto_mode_shares_one_budget_and_reclaims_an_unbooted_pool() {
    let Some(python) = concrete_python() else {
        return;
    };
    let base = tmp_dir("auto");
    let root = base.join("src");
    write_site(&root, 6);

    // No pool can boot (bare default): every slot the pool would have taken goes back to
    // the build. This is M1's fix on the DEFAULT path — no flag at all — which M1's own
    // test could not see, since it only ever passed an explicit --jobs.
    let free = build_stderr_with(&root, &base.join("outfree"), "0", None);
    assert!(
        !free.contains("pre-warming"),
        "must not report pre-warming a pool that cannot boot:\n{free}"
    );
    let without_pool = parallel_pages(&free);

    // The pool boots and, in auto mode, is funded FROM that same cap.
    let warm = build_stderr_with(&root, &base.join("outwarm"), "0", Some(&python));
    let with_pool = parallel_pages(&warm);
    let warmed = pre_warmed(&warm);

    // The auto-mode invariant. Derived, not hardcoded: on a core- or RAM-starved machine
    // `budget_split` legitimately warms 1 or 0, so asserting a literal 2 here would pin
    // this box rather than the rule.
    assert_eq!(
        with_pool + warmed,
        without_pool,
        "auto mode must keep warm_pool + build_kernels <= cap: {with_pool} + {warmed} != \
         {without_pool}\n--- without pool ---\n{free}\n--- with pool ---\n{warm}"
    );

    // On any machine with room to share (>= 3 slots), the pool really does claim its
    // target — so the assert above is comparing two different numbers, not passing
    // vacuously through 0. (Measured on the dev box: 14 + 2 == 16.)
    if without_pool >= 3 {
        assert_eq!(
            warmed, 2,
            "auto mode with room must pre-warm the full target:\n{warm}"
        );
    }

    let _ = std::fs::remove_dir_all(&base);
}
