//! Every cold kernel start goes through `Kernel::start_with_retry`, never the raw
//! `Kernel::start`.
//!
//! `prepare_connection` peeks free loopback ports by binding then releasing them, so two
//! kernels starting at the same moment can be handed the same port and the loser exits
//! with `zmq.error.ZMQError: Address already in use`. The re-roll that survives this used
//! to live in the *callers*, each with a private copy, which meant a caller that forgot it
//! inherited a known race with nothing to catch the omission. Three had, among them the
//! child half of `cold_kernel_self_reaps_on_ungraceful_parent_death` and the live-kernel
//! test in `kernel.rs`. The suite starts many kernels at once, so on any given run one of them
//! could lose the race — which is why the resulting flake was mis-attributed for weeks to
//! whichever test happened to lose it, and "fixed" against a theory of timing that was
//! never the cause.
//!
//! This is a source-level guard rather than a behavioural one on purpose: the failure it
//! prevents reproduces roughly 1 run in 13 under a fully parallel suite, so a behavioural
//! test for it would itself be the flake.

use std::path::{Path, PathBuf};

fn server_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every `.rs` file under `crates/server/src`, recursively.
fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_caller_reaches_the_un_retried_kernel_start() {
    let mut files = Vec::new();
    rust_sources(&server_src(), &mut files);
    assert!(!files.is_empty(), "found no server sources to scan");

    // The call is written `Kernel::start(`; the retrying wrapper is `start_with_retry(`,
    // and doc-comment references are written ``[`Kernel::start`]`` (no paren), so neither
    // is matched here.
    let mut offenders = Vec::new();
    for file in &files {
        let src = std::fs::read_to_string(file).expect("read a server source file");
        for (i, line) in src.lines().enumerate() {
            if line.contains("Kernel::start(") {
                offenders.push(format!(
                    "{}:{}: {}",
                    file.strip_prefix(server_src()).unwrap_or(file).display(),
                    i + 1,
                    line.trim()
                ));
            }
        }
    }

    // Exactly one legitimate site: the wrapper itself, which is what does the re-rolling.
    let expected = "kernel.rs";
    assert_eq!(
        offenders.len(),
        1,
        "`Kernel::start(` must be called only inside `Kernel::start_with_retry`; every \
         other caller inherits the `peek_ports` race. Offending sites:\n  {}",
        offenders.join("\n  ")
    );
    assert!(
        offenders[0].starts_with(expected),
        "the one permitted `Kernel::start(` is the one inside `start_with_retry` in \
         {expected}; found it at {}",
        offenders[0]
    );
}
