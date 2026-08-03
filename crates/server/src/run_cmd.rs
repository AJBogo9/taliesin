//! `taliesin run`: execute a document's code cells from the terminal and print what they
//! produced, with no browser anywhere in the loop.
//!
//! # What this is for
//!
//! Iterating on Python/ML code inside a `.tmd` should feel like a REPL, not like
//! publishing. You edit a cell, press Run, and read the result in the terminal. The HTML
//! is a *later* concern, and by the time you want it there is nothing left to compute:
//! the run already wrote `_freeze/`, so `taliesin build` replays it without booting a
//! kernel. That is the property Quarto structurally cannot offer (its `freeze` is
//! whole-document, and its editor "Run Cell" output is discarded, so rendering
//! re-executes) and the one Jupyter buys by storing outputs inside the source file.
//!
//! # Why it is a client, not a runner
//!
//! It executes nothing itself. `_freeze` caches cell *outputs*, never kernel *variable*
//! state, so a process that booted its own kernel per invocation would have to re-run
//! every upstream cell every time — your 4 GB load, on every keypress-to-run cycle. The
//! warm prefix living in memory *between* invocations is the entire point, so this
//! attaches to a long-lived session ([`crate::session`]) and asks it to run.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::runspec::RunScope;

/// How long to wait for a freshly spawned session to answer. Generous: it boots a
/// forkserver and pre-imports numpy/matplotlib/torch, which on a cold page cache is
/// seconds, and a client that gives up early would spawn a *second* session — the exact
/// two-kernel, two-cache-writer state this design exists to prevent.
const SESSION_READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

/// How long to wait for the TCP connect + response head from a session we believe is up.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

pub fn cmd_run(args: &[String]) -> ExitCode {
    let opts = match Opts::parse(&args[2..]) {
        Ok(o) => o,
        Err(e) => {
            crate::log::error(&e);
            return ExitCode::FAILURE;
        }
    };
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            crate::log::error(&format!("cannot start async runtime: {e}"));
            return ExitCode::FAILURE;
        }
    };
    rt.block_on(run(opts))
}

/// Parsed `taliesin run` arguments.
#[derive(Debug)]
struct Opts {
    file: PathBuf,
    scope: RunScope,
    /// `--quiet`: only errors and the summary, for scripts and CI.
    quiet: bool,
    /// `--interrupt`: stop this document's run in flight instead of starting one.
    interrupt: bool,
}

impl Opts {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut file: Option<PathBuf> = None;
        let mut cell: Option<usize> = None;
        let mut line: Option<u32> = None;
        let mut all = false;
        let mut quiet = false;
        let mut interrupt = false;
        let mut it = args.iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--cell" => {
                    cell = Some(num_arg(it.next(), "--cell")?);
                }
                "--line" => {
                    line = Some(num_arg(it.next(), "--line")?);
                }
                "--all" => all = true,
                "--quiet" | "-q" => quiet = true,
                "--interrupt" => interrupt = true,
                s if s.starts_with("--") => {
                    return Err(crate::serve::unknown_flag_error(
                        s,
                        &["--cell", "--line", "--all", "--quiet", "--interrupt"],
                    ));
                }
                s if file.is_none() => file = Some(PathBuf::from(s)),
                s => return Err(format!("unexpected argument: {s}")),
            }
        }
        let Some(file) = file else {
            return Err(crate::usage_line("run"));
        };
        if !file.exists() {
            return Err(format!("no such file: {}", file.display()));
        }
        // `--cell` and `--line` name the same thing two ways. Accepting both silently
        // would mean one of them was ignored, and the author would not know which.
        if cell.is_some() && line.is_some() {
            return Err("--cell and --line are alternatives; pass one".into());
        }
        if all && (cell.is_some() || line.is_some()) {
            return Err("--all runs the whole document; drop --cell/--line".into());
        }
        // `--interrupt` stops a run rather than starting one, so a scope is not a narrower
        // request, it is a contradiction. Refused rather than ignored, in the same style as
        // the three above: a caller who wrote both has not decided what they want.
        if interrupt && (all || cell.is_some() || line.is_some()) {
            return Err("--interrupt stops the running cells; drop --cell/--line/--all".into());
        }
        let scope = match (cell, line) {
            (Some(n), _) => {
                if n == 0 {
                    return Err("--cell is 1-based; there is no cell 0".into());
                }
                RunScope::ThroughCell(n)
            }
            (None, Some(l)) => RunScope::ThroughLine(l),
            (None, None) => RunScope::All,
        };
        Ok(Self {
            file,
            scope,
            quiet,
            interrupt,
        })
    }
}

/// Parse a numeric flag argument, naming the flag when it is missing or malformed.
fn num_arg<T: std::str::FromStr>(v: Option<&String>, flag: &str) -> Result<T, String> {
    let Some(v) = v else {
        return Err(format!("{flag} needs a number"));
    };
    v.parse()
        .map_err(|_| format!("{flag} expects a number, got `{v}`"))
}

async fn run(opts: Opts) -> ExitCode {
    let file = std::fs::canonicalize(&opts.file).unwrap_or_else(|_| opts.file.clone());
    let root = crate::session::project_root_for(&file);

    // The session is keyed on the project when there is one, and on the document when
    // there is not — matching which server would serve it, so `run` and `preview` of the
    // same thing land on ONE session rather than two. Derived by `session_key_for` rather
    // than spelled out here: this call site once carried its own version of the rule, and
    // a change on the server side left the two answering different things.
    let key = crate::session::session_key_for(&file);

    // `--interrupt` never starts a session. No session means nothing is running, which is the
    // answer; booting a kernel in order to interrupt it would take the full
    // `SESSION_READY_TIMEOUT` to accomplish nothing.
    if opts.interrupt {
        return interrupt(&key, &file, opts.quiet).await;
    }

    let port = match attach_or_start(&key, opts.quiet).await {
        Ok(p) => p,
        Err(e) => {
            crate::log::error(&e);
            return ExitCode::FAILURE;
        }
    };

    let body = serde_json::json!({
        "file": file.to_string_lossy(),
        "cell": match opts.scope { RunScope::ThroughCell(n) => Some(n), _ => None },
        "line": match opts.scope { RunScope::ThroughLine(l) => Some(l), _ => None },
    })
    .to_string();

    let mut resp =
        match crate::http1::post_json(port, crate::serve::RUN_PATH, &body, CONNECT_TIMEOUT).await {
            Ok(r) => r,
            Err(e) => {
                crate::log::error(&format!("cannot reach the session on port {port}: {e}"));
                return ExitCode::FAILURE;
            }
        };
    if resp.status != 200 {
        let detail = resp.text().await.unwrap_or_default();
        crate::log::error(&format!(
            "session refused the run ({}): {}",
            resp.status,
            detail.trim()
        ));
        return ExitCode::FAILURE;
    }

    // Ctrl-C stops the RUN, not just this process.
    //
    // Without this the signal killed only the client: `runspec::event_stream` saw the hangup,
    // broke its forwarding loop, and the session executed the rest of the document with
    // nobody watching. The author's only remaining brake was "Restart kernel", which discards
    // the warm state the whole design exists to keep.
    //
    // The first Ctrl-C asks the session to stop and **keeps reading**, so the
    // `KeyboardInterrupt` traceback and the run's own summary still arrive: exiting here would
    // hide the very output that confirms the stop worked. A second Ctrl-C leaves immediately,
    // for a session that is wedged or gone.
    let mut stopping = false;

    let mut printer = crate::run_print::Printer::new(opts.quiet, &root);
    loop {
        let line = tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c(), if !stopping => {
                stopping = true;
                if !opts.quiet {
                    crate::log::info("interrupting the run (Ctrl-C again to stop waiting)");
                }
                let body = serde_json::json!({ "file": file.to_string_lossy() }).to_string();
                // Best-effort: if the session cannot be reached the run is already over or
                // the session is gone, and the stream below will end on its own.
                let _ = crate::http1::post_json(
                    port,
                    crate::serve::INTERRUPT_PATH,
                    &body,
                    CONNECT_TIMEOUT,
                )
                .await;
                continue;
            }
            _ = tokio::signal::ctrl_c(), if stopping => {
                crate::log::error("gave up waiting for the session; the run may still be stopping");
                return ExitCode::FAILURE;
            }
            line = resp.next_line() => line,
        };
        match line {
            Ok(Some(line)) if line.trim().is_empty() => {}
            Ok(Some(line)) => {
                if printer.consume(&line) {
                    break;
                }
            }
            Ok(None) => {
                // The stream ended without a terminal message: the session died mid-run
                // (a kernel took it down, someone killed it). Say so rather than exit 0 on
                // a run whose result nobody saw.
                printer.truncated();
                return ExitCode::FAILURE;
            }
            Err(e) => {
                crate::log::error(&format!("lost the session stream: {e}"));
                return ExitCode::FAILURE;
            }
        }
    }
    printer.finish()
}

/// `taliesin run --interrupt <file>`: stop that document's run, keeping the warm kernel.
///
/// Exits 0 whether or not anything was running. "Nothing is running" is a successful
/// interrupt, not a failure: the author who hits this a second after the last cell finished
/// wanted the run stopped, and it is.
async fn interrupt(key: &Path, file: &Path, quiet: bool) -> ExitCode {
    let Some(port) = crate::session::hinted_port(key) else {
        if !quiet {
            crate::log::info("no session is running for this project; nothing to interrupt");
        }
        return ExitCode::SUCCESS;
    };
    // The hint is a hint. Prove the responder is really this project's session before
    // sending it anything, exactly as a run does — a recycled port could be anyone.
    if !crate::serve::session_owns(port, key).await {
        if !quiet {
            crate::log::info("no session is running for this project; nothing to interrupt");
        }
        return ExitCode::SUCCESS;
    }

    let body = serde_json::json!({ "file": file.to_string_lossy() }).to_string();
    let mut resp =
        match crate::http1::post_json(port, crate::serve::INTERRUPT_PATH, &body, CONNECT_TIMEOUT)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                crate::log::error(&format!("cannot reach the session on port {port}: {e}"));
                return ExitCode::FAILURE;
            }
        };
    let text = resp.text().await.unwrap_or_default();
    if resp.status != 200 {
        crate::log::error(&format!(
            "session refused the interrupt ({}): {}",
            resp.status,
            text.trim()
        ));
        return ExitCode::FAILURE;
    }
    if !quiet {
        let stopped = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("lang")
                    .and_then(|l| l.as_str())
                    .map(|s| s.to_string())
            });
        match stopped {
            Some(lang) => crate::log::info(&format!(
                "interrupted the running {lang} cell; the kernel and earlier cells are intact"
            )),
            None => crate::log::info("nothing was running"),
        }
    }
    ExitCode::SUCCESS
}

/// The port of this project's session, starting one if none is live.
///
/// A recorded port is a hint and nothing more, so it is proved with the identity
/// handshake the preview already serves before anything is sent to it.
async fn attach_or_start(key: &Path, quiet: bool) -> Result<u16, String> {
    if let Some(port) = crate::session::hinted_port(key)
        && crate::serve::session_owns(port, key).await
    {
        return Ok(port);
    }
    if !quiet {
        crate::log::info(&format!(
            "starting a session for {} (first run warms the kernel)",
            key.display()
        ));
    }
    spawn_session(key).await
}

/// Start a headless session for `key` and wait for it to answer the handshake.
///
/// Spawned detached and left running: the next `taliesin run` attaches to it, which is
/// what makes the second run instant. It exits with the terminal that owns it, or on an
/// explicit `taliesin run --stop`-style shutdown; nothing here reaps it.
async fn spawn_session(key: &Path) -> Result<u16, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("cannot find the taliesin binary to start a session: {e}"))?;
    let cwd = if key.is_dir() {
        key.to_path_buf()
    } else {
        key.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    std::process::Command::new(&exe)
        .arg("preview")
        .arg(key)
        .arg(crate::cli::SESSION_FLAG)
        .current_dir(&cwd)
        // Inherited stderr would interleave the server's own console output with this
        // run's, which is the output the author is actually reading.
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start a session: {e}"))?;

    let deadline = std::time::Instant::now() + SESSION_READY_TIMEOUT;
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        if let Some(port) = crate::session::hinted_port(key)
            && crate::serve::session_owns(port, key).await
        {
            return Ok(port);
        }
    }
    Err(format!(
        "the session did not come up within {}s; try `taliesin preview {}` to see why",
        SESSION_READY_TIMEOUT.as_secs(),
        key.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// A real file, since `Opts::parse` refuses a path that does not exist (a typo'd
    /// filename must fail here, not as a confusing 404 from the session).
    fn a_file() -> PathBuf {
        let p = std::env::temp_dir().join(format!("tali-run-opts-{}.tmd", std::process::id()));
        std::fs::write(&p, "# t\n").unwrap();
        p
    }

    #[test]
    fn defaults_to_the_whole_document() {
        let f = a_file();
        let o = Opts::parse(&args(&[&f.to_string_lossy()])).unwrap();
        assert_eq!(o.scope, RunScope::All);
        assert!(!o.quiet);
    }

    #[test]
    fn cell_and_line_are_parsed_as_scopes() {
        let f = a_file();
        let fs = f.to_string_lossy().to_string();
        assert_eq!(
            Opts::parse(&args(&[&fs, "--cell", "3"])).unwrap().scope,
            RunScope::ThroughCell(3)
        );
        assert_eq!(
            Opts::parse(&args(&[&fs, "--line", "42"])).unwrap().scope,
            RunScope::ThroughLine(42)
        );
    }

    #[test]
    fn contradictory_scopes_are_refused_rather_than_silently_resolved() {
        // Picking one and ignoring the other is the bad outcome: the author would not
        // know which cell ran.
        let f = a_file();
        let fs = f.to_string_lossy().to_string();
        assert!(Opts::parse(&args(&[&fs, "--cell", "1", "--line", "9"])).is_err());
        assert!(Opts::parse(&args(&[&fs, "--all", "--cell", "1"])).is_err());
    }

    #[test]
    fn a_zero_cell_ordinal_is_refused() {
        let f = a_file();
        let fs = f.to_string_lossy().to_string();
        let e = Opts::parse(&args(&[&fs, "--cell", "0"])).unwrap_err();
        assert!(e.contains("1-based"), "unhelpful message: {e}");
    }

    #[test]
    fn a_missing_file_fails_here_not_at_the_session() {
        let e = Opts::parse(&args(&["/nope/does-not-exist.tmd"])).unwrap_err();
        assert!(e.contains("no such file"), "unhelpful message: {e}");
    }

    #[test]
    fn a_non_numeric_flag_argument_names_the_flag() {
        let f = a_file();
        let fs = f.to_string_lossy().to_string();
        let e = Opts::parse(&args(&[&fs, "--cell", "three"])).unwrap_err();
        assert!(e.contains("--cell"), "must name the flag: {e}");
        let e = Opts::parse(&args(&[&fs, "--line"])).unwrap_err();
        assert!(e.contains("--line"), "must name the flag: {e}");
    }

    #[test]
    fn interrupt_parses_and_carries_no_scope() {
        let f = a_file();
        let o = Opts::parse(&args(&[&f.to_string_lossy(), "--interrupt"])).unwrap();
        assert!(o.interrupt);
        assert_eq!(
            o.scope,
            RunScope::All,
            "the scope is unused but must be sane"
        );
    }

    #[test]
    fn interrupt_refuses_a_scope_rather_than_ignoring_it() {
        // `--interrupt --cell 3` has no meaning: you cannot stop "through cell 3". Picking
        // one and dropping the other is the bad outcome, exactly as for --cell/--line.
        let f = a_file();
        let fs = f.to_string_lossy().to_string();
        for extra in [vec!["--cell", "3"], vec!["--line", "9"], vec!["--all"]] {
            let mut a = vec![fs.as_str(), "--interrupt"];
            a.extend(extra.iter());
            let e = Opts::parse(&args(&a)).unwrap_err();
            assert!(
                e.contains("--interrupt"),
                "must name the flag that conflicts, got: {e}"
            );
        }
    }

    #[tokio::test]
    async fn interrupting_with_no_session_succeeds_without_starting_one() {
        // Two claims, and the timing one is the load-bearing half. Booting a session in
        // order to interrupt it would be absurd, and the way that regression would arrive is
        // someone routing this through `attach_or_start` for symmetry with a real run — at
        // which point this waits out SESSION_READY_TIMEOUT (45s) to accomplish nothing.
        // Asserting only the exit code would not notice.
        let dir = std::env::temp_dir().join(format!("tali-interrupt-none-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("doc.tmd");
        std::fs::write(&file, "# t\n").unwrap();

        let t0 = std::time::Instant::now();
        let code = interrupt(&dir, &file, true).await;
        let elapsed = t0.elapsed();

        assert_eq!(
            format!("{code:?}"),
            format!("{:?}", ExitCode::SUCCESS),
            "nothing to interrupt is a success, not a failure"
        );
        assert!(
            elapsed < SESSION_READY_TIMEOUT / 4,
            "took {elapsed:?}; this must not be waiting for a session to start"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_flag_suggests_the_real_ones() {
        let f = a_file();
        let fs = f.to_string_lossy().to_string();
        let e = Opts::parse(&args(&[&fs, "--cells", "1"])).unwrap_err();
        assert!(e.contains("--cell"), "expected a suggestion, got: {e}");
    }
}
