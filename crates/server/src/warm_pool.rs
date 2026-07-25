//! An eager **warm pool** of Python kernels so the first code-cell edit is
//! near-instant instead of paying a multi-second interpreter + heavy-import boot
//! (Problem 3, taken one step further than the single warm kernel in `kernel.rs`).
//!
//! # Mechanism: forkserver preload (with a guaranteed safe fallback)
//!
//! The pool fronts a long-lived **forkserver daemon** (an embedded Python helper,
//! [`FORKSERVER_HELPER`]). The daemon boots a `multiprocessing` *forkserver* once
//! and `set_forkserver_preload([...])`s the heavy libraries (`numpy`,
//! `matplotlib`, and `torch` *only if importable*). Every kernel the pool hands
//! out is then produced by **forking** that already-warm process image: because of
//! copy-on-write the child inherits the preloaded modules for free, so the
//! expensive `import numpy` / `import matplotlib` is paid **once at server start**
//! rather than on the first edit.
//!
//! ## The warmth/exec pitfall (handled)
//!
//! A forkserver child only keeps the preloaded warmth if it runs ipykernel
//! **in-process**. If it `exec`'d `python -m ipykernel_launcher` the process image
//! would be replaced and every preloaded module lost. So the forked child starts
//! ipykernel programmatically — `IPKernelApp.instance().initialize([...]);
//! app.start()` — against a connection file *taliesin* (not Python) created with
//! [`crate::kernel::prepare_connection`]. taliesin then connects over ZMQ with the
//! same handshake `Kernel::start` uses ([`Kernel::adopt_forked`]).
//!
//! ## Process model
//!
//! A forked kernel is **not** taliesin's direct child (the forkserver server reaps
//! it); the daemon reports the kernel's PID back over stdout and taliesin drives
//! SIGINT / liveness / teardown through that PID with the same `kill(pid, …)`
//! primitives the owned-child path already uses. Each handed-out [`Kernel`] holds
//! an `Arc<ForkserverDaemon>` so the forkserver stays warm for its lifetime.
//!
//! ## Fallback (behaviour never worse than today)
//!
//! If the forkserver can't boot (no `TALIESIN_PYTHON`, import error, non-Linux, an
//! R-only doc) the pool's daemon is simply absent and [`WarmPool::take`] returns
//! `None`. The caller treats `None` as "cold start" and calls `Kernel::start`
//! directly, exactly as today. Any failure on the warm path degrades to the cold
//! path; it is never a hard error.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant, timeout};

use crate::kernel::{Kernel, KernelSpec, prepare_connection};

/// Default pool size cap. The pool pre-warms `min(POOL_CAP, requested)` kernels;
/// each warm Python kernel costs roughly one idle interpreter's RAM, so we keep
/// this small (the build RAM budget is reconciled by `build_budget::budget_split`).
pub const POOL_CAP: usize = 2;

/// How long to wait for the forkserver daemon to report `READY` before declaring
/// it unavailable and falling back to cold starts.
const DAEMON_READY_TIMEOUT: Duration = Duration::from_secs(30);

/// How long to wait for a single fork request (`SPAWNED <pid>`). Forking an
/// already-warm image is fast; a longer-than-this stall means something is wrong,
/// so we give up on this kernel and let the caller fall back.
const FORK_TIMEOUT: Duration = Duration::from_secs(20);

/// How many times a single pre-warm retries a recoverable fork failure before giving
/// up and letting the caller cold-start. Mirrors `START_ATTEMPTS` on the cold path in
/// `exec.rs`, which retries `Kernel::start` for the same transient reasons.
const FORK_ATTEMPTS: usize = 4;

/// The libraries the forkserver tries to preload. Each is included **only if
/// importable** in the target interpreter (the helper filters with
/// `importlib.util.find_spec`), so a missing `torch` simply isn't preloaded rather
/// than breaking the daemon.
const PRELOAD_CANDIDATES: &[&str] = &["numpy", "matplotlib", "torch"];

/// The embedded forkserver daemon. Boots a `multiprocessing` forkserver, preloads
/// the importable heavy libs, prints `READY <json-list>`, then for each `<id>
/// <connection-file>` request on stdin forks a child that runs `IPKernelApp`
/// **in-process** and prints `SPAWNED <id> <pid>`. See the module docs for the
/// warmth/exec rationale, and [`ForkserverDaemon::fork_kernel_within`] for why every
/// reply echoes the request's id.
const FORKSERVER_HELPER: &str = r#"
import sys, os, json, signal, importlib.util, multiprocessing as mp

def _reap_group():
    # taliesin (our parent) closed our stdin -> it has EXITED (SIGKILL, a crash, or a
    # closed terminal), so its Drop-based teardown never ran. Reap the forkserver server
    # + every kernel it forked before we follow it out: they share OUR process group (we
    # lead it via `process_group(0)` on the Rust side), and taliesin sits in its own
    # group, so this SIGKILL cannot reach it. This is the same group `kill_process_group`
    # reaps gracefully from Rust, triggered here from inside instead. Chosen over
    # PR_SET_PDEATHSIG: that signals only the helper (orphaning the server + kernels) and,
    # in taliesin's multithreaded tokio runtime, can fire on a parent *thread* exit; stdin
    # EOF is parent *process* death and reaps the whole subtree.
    try:
        os.killpg(os.getpgrp(), signal.SIGKILL)
    except Exception:
        pass

def _detect_preload(requested):
    out = []
    for m in requested:
        try:
            if importlib.util.find_spec(m) is not None:
                out.append(m)
        except Exception:
            pass
    return out

def _noop():
    pass

def _child_entry(conn_file):
    # FORKED CHILD. Heavy libs are already resident (copy-on-write), so we must
    # NOT exec a new interpreter — that would replace the image and lose every
    # preloaded module. Start ipykernel in-process instead.
    try:
        # Detach fd 1 from the daemon's control pipe BEFORE ipykernel starts: it
        # prints a startup NOTE ("Ctrl-C will not work") to stdout, which would
        # otherwise interleave with the parent's "SPAWNED <pid>" line and corrupt the
        # fork protocol taliesin reads on the daemon stdout. The kernel talks over ZMQ,
        # never this stdout, so /dev/null is safe (ipykernel re-captures fd 1 into its
        # own IOPub pipe during init anyway).
        _null = os.open(os.devnull, os.O_WRONLY)
        os.dup2(_null, 1)
        os.close(_null)
        from ipykernel.kernelapp import IPKernelApp
        app = IPKernelApp.instance()
        app.initialize(["-f", conn_file])
        app.start()
    except Exception as e:
        sys.stderr.write("tali-warm: child kernel failed: %r\n" % (e,)); sys.stderr.flush()
        os._exit(1)

def _serve(ctx):
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        # A request is "<id> <connection-file>". The id is echoed back in the reply so
        # a client that gave up on an earlier request (its deadline passed) can tell
        # that request's late reply from its own, instead of adopting a kernel that
        # belongs to nobody. Split once: the id never contains a space, the path may.
        rid, _, conn = line.partition(" ")
        if not conn:
            sys.stdout.write("ERROR %s\n" % (rid,)); sys.stdout.flush()
            continue
        try:
            p = ctx.Process(target=_child_entry, args=(conn,))
            p.start()
            sys.stdout.write("SPAWNED %s %d\n" % (rid, p.pid)); sys.stdout.flush()
        except Exception as e:
            # Report the failure and KEEP SERVING: a fork that fails is this request's
            # problem, not the daemon's, so the next request is still answerable. The
            # Rust client retries on the strength of exactly this promise.
            sys.stderr.write("tali-warm: fork failed: %r\n" % (e,)); sys.stderr.flush()
            sys.stdout.write("ERROR %s\n" % (rid,)); sys.stdout.flush()

def main():
    requested = json.loads(sys.argv[1]) if len(sys.argv) > 1 else []
    preload = _detect_preload(requested)
    try:
        ctx = mp.get_context("forkserver")
        ctx.set_forkserver_preload(preload)
        # Force the forkserver server to boot + preload now (a throwaway noop fork),
        # so the first real take() doesn't pay the import cost.
        w = ctx.Process(target=_noop); w.start(); w.join()
    except Exception as e:
        sys.stderr.write("tali-warm: forkserver unavailable: %r\n" % (e,)); sys.stderr.flush()
        os._exit(2)
    sys.stdout.write("READY " + json.dumps(preload) + "\n"); sys.stdout.flush()
    # The serve loop ends when stdin hits EOF — which, in the ungraceful case (taliesin
    # SIGKILLed / crashed / terminal closed), is the ONLY notice we get that our parent
    # is gone. On any exit from it, reap the forkserver subtree we lead so it can't
    # outlive taliesin (~100 MB per kernel). The graceful path SIGKILLs us first, so this
    # runs only when Drop didn't.
    try:
        _serve(ctx)
    finally:
        _reap_group()

if __name__ == "__main__":
    main()
"#;

/// A live forkserver daemon process: the warm Python image every pooled kernel is
/// forked from. Fork requests must be serialised (write a connection-file path,
/// read the `SPAWNED <pid>` reply), so its pipes live behind a single mutex.
pub struct ForkserverDaemon {
    /// Kept so the daemon process is killed when the last `Arc` drops.
    _child: Child,
    io: Mutex<DaemonIo>,
    /// The libs the daemon actually preloaded (for logging/diagnostics).
    preloaded: Vec<String>,
    /// The 0700 temp dir holding the helper script, removed on drop. The helper
    /// must be a real file (not `python -c`): `multiprocessing`'s forkserver
    /// re-imports the target's `__main__` by name to locate the fork entry point,
    /// which fails for a `-c` string module.
    helper_dir: PathBuf,
    /// The helper's pid, captured at boot (when `child.id()` is reliably `Some`).
    /// Because the helper leads its own process group (`process_group(0)`), this is
    /// also the pgid of the whole forkserver subtree — the target for the teardown
    /// group-kill. Stored rather than re-read from `_child.id()` at `Drop` time so
    /// teardown never depends on the child handle still being unreaped.
    helper_pid: Option<u32>,
}

impl Drop for ForkserverDaemon {
    fn drop(&mut self) {
        // Tear down the whole forkserver subtree, not just the helper. Killing only
        // `_child` (via `kill_on_drop`) leaves the multiprocessing *forkserver server*
        // the helper booted — and every kernel that server forked — orphaned: they are
        // NOT taliesin's direct children (systemd reaps them), the server ignores
        // SIGINT, and each is ~100 MB. The helper leads its own process group
        // (`process_group(0)` in `boot`) and the server + kernels inherit it, so one
        // group SIGKILL reclaims the whole subtree. Safe here because the daemon only
        // drops once its last `Arc` is gone, i.e. after every handed-out `Kernel`
        // (which already SIGKILLs its own pid on drop) has been released.
        if let Some(pid) = self.helper_pid {
            kill_process_group(pid);
        }
        // `kill_on_drop` still SIGKILLs + reaps the helper itself; clean up its dir.
        let _ = std::fs::remove_dir_all(&self.helper_dir);
    }
}

/// SIGKILL every process in the group led by `helper_pid`. The warm-pool helper is
/// spawned into its OWN process group (`process_group(0)`, so its pgid equals its
/// pid), which the forkserver server it boots — and every kernel that server forks —
/// inherit. Signalling the negated pid targets that whole group at once; SIGKILL is
/// uncatchable, so the forkserver's SIGINT mask can't dodge it, and a group that has
/// already exited returns `ESRCH`, which we ignore. No-op off Unix (no forkserver
/// there — the warm pool is inert and this is never reached).
#[cfg(unix)]
fn kill_process_group(helper_pid: u32) {
    // Safety: a negative argument signals the process group `helper_pid`; because the
    // helper leads its own group the target is exactly the forkserver subtree we
    // created, never an unrelated process. Errors (e.g. `ESRCH`) are ignored.
    unsafe {
        libc::kill(-(helper_pid as libc::pid_t), libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_process_group(_helper_pid: u32) {}

struct DaemonIo {
    /// The helper's stdin (request pipe). `Option` only so the ungraceful-death test can
    /// drop it (closing the OS fd → EOF at the helper) to stand in for a parent death;
    /// in production it is `Some` for the daemon's whole life.
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    /// The correlation id for the next fork request. Monotonic and never reused, so an
    /// abandoned request's id can never collide with a live one. Lives beside the pipes
    /// (under the same mutex) because allocating an id and writing the request it names
    /// must be one atomic step.
    next_id: u64,
}

/// One line of the daemon's fork protocol, parsed. Split out from the read loop so the
/// wire format is pinned by a unit test with no daemon and no kernel in sight.
#[derive(Debug, PartialEq, Eq)]
enum ForkReply {
    /// `SPAWNED <id> <pid>`: the request `id` produced a kernel at `pid`.
    Spawned { id: u64, pid: u32 },
    /// `ERROR <id>`: the request `id` could not be forked. The daemon is still alive
    /// and still serving, so this is per-request, not terminal.
    Error { id: u64 },
    /// Anything else: a stray line that is not part of the protocol.
    Stray,
}

/// Parse one reply line from the daemon. Unknown shapes are [`ForkReply::Stray`] rather
/// than an error: a lone banner that slipped onto the pipe must not fail a good fork.
fn parse_fork_reply(line: &str) -> ForkReply {
    if let Some(rest) = line.strip_prefix("SPAWNED ") {
        let mut parts = rest.split_whitespace();
        if let (Some(id), Some(pid), None) = (parts.next(), parts.next(), parts.next())
            && let (Ok(id), Ok(pid)) = (id.parse::<u64>(), pid.parse::<u32>())
        {
            return ForkReply::Spawned { id, pid };
        }
        return ForkReply::Stray;
    }
    if let Some(rest) = line.strip_prefix("ERROR ")
        && let Ok(id) = rest.trim().parse::<u64>()
    {
        return ForkReply::Error { id };
    }
    ForkReply::Stray
}

/// Whether a failed pre-warm is worth another attempt against the same daemon.
///
/// The daemon answers a fork it could not perform with `ERROR <id>` and then goes
/// straight back to reading stdin: it is telling us this request failed but the *next*
/// one can still be served. That promise is what makes a retry meaningful, and it is
/// the promise the client used to throw away by treating `ERROR` as terminal. A
/// timed-out request is likewise retryable now that replies carry ids (before, a retry
/// was the very thing that mis-paired the next pid).
///
/// The exceptions are the failures where the daemon itself is gone: retrying a closed
/// pipe just burns the caller's time to reach the same cold start. Everything else
/// defers to [`crate::kernel::start_error_is_transient`], which already classifies the
/// port-race and missing-interpreter failures the cold path retries, since
/// `prepare_connection` + `adopt_forked` here are the same steps failing the same ways.
fn fork_error_is_retryable(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    if m.contains("closed during fork") || m.contains("broken pipe") {
        return false;
    }
    crate::kernel::start_error_is_transient(msg)
}

/// SIGKILL a single forked kernel PID that no [`Kernel`] owns. Used to reclaim the
/// kernel behind a late `SPAWNED` reply: the request that asked for it has already
/// given up, so nothing will ever connect to it, and it would sit on ~100 MB and five
/// bound ports until the whole pool is dropped. Not our direct child (the forkserver
/// reaps it), so we signal by pid — the same primitive `ForkedCleanup`'s `Drop` uses
/// for the same orphan-a-fork hazard one step later in the handshake. A pid that has
/// already exited returns `ESRCH`, which we ignore. No-op off Unix (no forkserver
/// there, so this is never reached).
#[cfg(unix)]
fn kill_forked_pid(pid: u32) {
    // Safety: `pid` was reported by our own daemon as a kernel it forked moments ago,
    // and every kernel it forks is in our process group. Errors are ignored.
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_forked_pid(_pid: u32) {}

impl ForkserverDaemon {
    /// Boot the daemon with `python`, preloading the importable heavy libs. Errors
    /// (no interpreter, no `multiprocessing` forkserver, no `READY`) mean "no warm
    /// pool"; the caller falls back to cold starts.
    async fn boot(python: &std::path::Path) -> std::io::Result<ForkserverDaemon> {
        // The helper must be a real script file, not `python -c`: the forkserver
        // re-imports the target function's `__main__` by name, which fails for a
        // `-c` string module ("Can't get attribute '_child_entry' on __main__").
        // Write it to a private 0700 dir, removed when the daemon drops.
        // Pid-tagged so a later run can reclaim it if we die ungracefully (`Drop` won't
        // run to remove it). See `runtime_dirs`.
        let helper_dir = crate::runtime_dirs::warmpool_dir();
        {
            let mut b = std::fs::DirBuilder::new();
            b.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                b.mode(0o700);
            }
            b.create(&helper_dir)?;
        }
        let helper_path = helper_dir.join("tali_forkserver.py");
        std::fs::write(&helper_path, FORKSERVER_HELPER)?;

        let preload_json = serde_json::to_string(PRELOAD_CANDIDATES).unwrap();
        let mut cmd = Command::new(python);
        cmd.arg(&helper_path)
            .arg(preload_json)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        // Put the helper (and the forkserver server + kernels it spawns, which inherit
        // its group) into a fresh process group, so teardown can SIGKILL the whole
        // subtree by group without ever touching taliesin's own group. See
        // `kill_process_group` + `Drop for ForkserverDaemon`.
        #[cfg(unix)]
        cmd.process_group(0);
        let mut child = cmd.spawn().map_err(|e| {
            let _ = std::fs::remove_dir_all(&helper_dir);
            std::io::Error::other(format!(
                "cannot launch warm-pool forkserver `{}`: {e}",
                python.display()
            ))
        })?;
        // Capture the pid now, while `child.id()` is reliably `Some` (the child was
        // just spawned and hasn't been reaped). With `process_group(0)` this is the
        // pgid of the whole subtree; teardown uses it instead of re-reading the handle.
        let helper_pid = child.id();
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("forkserver: no stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("forkserver: no stdout"))?;
        let mut reader = BufReader::new(stdout);

        // Wait for the READY line (the forkserver has booted + preloaded). On any
        // failure clean up the helper dir before bailing (the Drop that normally
        // removes it only runs once the daemon value is constructed below).
        let mut line = String::new();
        let ready: std::io::Result<Vec<String>> = async {
            let n = timeout(DAEMON_READY_TIMEOUT, reader.read_line(&mut line))
                .await
                .map_err(|_| std::io::Error::other("forkserver: timed out waiting for READY"))??;
            if n == 0 || !line.starts_with("READY") {
                return Err(std::io::Error::other(format!(
                    "forkserver: did not report READY (got {line:?})"
                )));
            }
            Ok(line
                .strip_prefix("READY ")
                .and_then(|j| serde_json::from_str(j.trim()).ok())
                .unwrap_or_default())
        }
        .await;
        let preloaded = match ready {
            Ok(p) => p,
            Err(e) => {
                // The forkserver server may already have booted (a mid-boot READY
                // timeout), so tear down the whole subtree — not just the helper —
                // before bailing, mirroring the `Drop` teardown (`kill_process_group`
                // is a no-op off Unix, so no `cfg` gate is needed here).
                if let Some(pid) = helper_pid {
                    kill_process_group(pid);
                }
                let _ = std::fs::remove_dir_all(&helper_dir);
                return Err(e);
            }
        };

        Ok(ForkserverDaemon {
            _child: child,
            io: Mutex::new(DaemonIo {
                stdin: Some(stdin),
                stdout: reader,
                next_id: 1,
            }),
            preloaded,
            helper_dir,
            helper_pid,
        })
    }

    /// Ask the daemon to fork a kernel bound to `conn_file`; returns the kernel's
    /// PID. Serialised across callers by the io mutex.
    async fn fork_kernel(&self, conn_file: &std::path::Path) -> std::io::Result<u32> {
        self.fork_kernel_within(conn_file, FORK_TIMEOUT).await
    }

    /// Drop *our* end of the helper's stdin so its `for line in sys.stdin` loop sees EOF
    /// — exactly what an ungraceful taliesin death (SIGKILL/crash/closed terminal) does to
    /// the pipe when the process's fds close. `take()` drops the `ChildStdin`, which
    /// closes the OS write fd; `shutdown()` alone does not close a pipe, so the helper
    /// would never see EOF. Used by the ungraceful-death test to trigger the helper's
    /// self-reap WITHOUT running `Drop for ForkserverDaemon` (whose group-kill would mask
    /// whether the helper reaps itself).
    #[cfg(test)]
    pub(crate) async fn close_stdin_for_test(&self) {
        let mut io = self.io.lock().await;
        io.stdin.take();
    }

    /// [`Self::fork_kernel`] with an explicit deadline, so a test can drive the
    /// timeout path in milliseconds instead of waiting out [`FORK_TIMEOUT`].
    ///
    /// # Why every request carries an id
    ///
    /// The reply is read off a shared pipe, and the mutex is only held for the
    /// duration of *this* call. When a request times out we return `Err` and drop the
    /// mutex, but the daemon does not know that: it is single-threaded, so it finishes
    /// the fork at its own pace and writes the reply regardless. That reply then sits
    /// in the pipe with the next caller's read in front of it. With a bare `SPAWNED
    /// <pid>` there was nothing to tell the two apart, so the next fork returned the
    /// *previous* request's pid: a kernel bound to a connection file it had never seen,
    /// whose ports it could not reach and whose PID it would later SIGKILL out from
    /// under nobody. Echoing the request id makes a late reply self-identifying: we
    /// skip it, reclaim the orphaned kernel behind it, and keep waiting for our own.
    async fn fork_kernel_within(
        &self,
        conn_file: &std::path::Path,
        budget: Duration,
    ) -> std::io::Result<u32> {
        let mut io = self.io.lock().await;
        let id = io.next_id;
        io.next_id += 1;
        let req = format!("{id} {}\n", conn_file.display());
        let stdin = io
            .stdin
            .as_mut()
            .ok_or_else(|| std::io::Error::other("forkserver: stdin closed"))?;
        stdin.write_all(req.as_bytes()).await?;
        stdin.flush().await?;
        // Read until *our* reply. `_child_entry` detaches the kernel's stdout from this
        // pipe, so the reply is normally clean; but skip any stray non-protocol line as
        // belt-and-suspenders — a lone banner that slipped onto the pipe must not fail
        // an otherwise-good fork. One overall deadline bounds the whole read, so a
        // silent daemon still times out rather than looping forever.
        let deadline = Instant::now() + budget;
        let mut warned_stray = false;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(std::io::Error::other("forkserver: fork request timed out"));
            }
            let mut line = String::new();
            let n = timeout(left, io.stdout.read_line(&mut line))
                .await
                .map_err(|_| std::io::Error::other("forkserver: fork request timed out"))??;
            if n == 0 {
                return Err(std::io::Error::other("forkserver: closed during fork"));
            }
            let line = line.trim();
            match parse_fork_reply(line) {
                ForkReply::Spawned { id: rid, pid } if rid == id => return Ok(pid),
                ForkReply::Error { id: rid } if rid == id => {
                    return Err(std::io::Error::other(
                        "forkserver: fork failed (daemon reported ERROR)",
                    ));
                }
                // A reply to a request we already gave up on. The daemon answers in
                // order, so this can only be an *older* request whose deadline passed.
                // Its kernel is real and running but unreachable — the caller took the
                // cold path and its connection dir is already gone — so reclaim it here
                // rather than let it hold RAM + ports until the pool drops.
                ForkReply::Spawned { id: rid, pid } => {
                    crate::log::warn(&format!(
                        "forkserver: reclaiming kernel {pid} from abandoned fork request {rid}"
                    ));
                    kill_forked_pid(pid);
                }
                ForkReply::Error { .. } => {}
                ForkReply::Stray => {
                    if !line.is_empty() && !warned_stray {
                        // A stray whole line before the reply (rare now that the child
                        // detaches its stdout). Warn once — not per line — so a flood
                        // can't spam the log; keep reading up to the deadline regardless.
                        warned_stray = true;
                        crate::log::warn(&format!(
                            "forkserver: ignoring unexpected pipe line(s), first: {line:?}"
                        ));
                    }
                }
            }
        }
    }
}

/// An eager pool of pre-warmed Python kernels. Construct with [`WarmPool::new`]
/// (which boots the forkserver and starts background pre-warming); call
/// [`WarmPool::take`] to claim a ready kernel, or `None` to fall back to a cold
/// `Kernel::start`.
pub struct WarmPool {
    inner: Arc<PoolInner>,
}

struct PoolInner {
    /// The forkserver, or `None` when it could not boot (-> always fall back).
    daemon: Option<Arc<ForkserverDaemon>>,
    /// The interpreter the pool warms kernels for (Python only).
    python: PathBuf,
    /// How many kernels to keep ready.
    cap: usize,
    /// Ready, fully-connected kernels waiting to be handed out.
    ready: Mutex<VecDeque<Kernel>>,
    /// Number of kernels currently being forked (in-flight) but not yet pushed
    /// into `ready`. The total pool size is `ready.len() + *in_flight`; both
    /// terms are counted separately so that a successful fork decrements
    /// `in_flight` when it pushes into `ready`, keeping the invariant exact.
    ///
    /// A `std::sync::Mutex` (not tokio's), so [`SlotReservation`]'s `Drop` can
    /// release a slot without an `await` — the reservation is held across
    /// `warm_one`, and a sync `Drop` guarantees the slot is returned even if that
    /// fork panics. It is only ever locked for a non-blocking counter tweak (never
    /// across an `await`), so it can't stall the runtime.
    in_flight: std::sync::Mutex<usize>,
}

/// RAII reservation of one in-flight fork slot. Held for the lifetime of a single
/// `warm_one` fork; its `Drop` decrements `in_flight` whether the fork succeeded (the
/// kernel has by then been pushed into `ready`, so occupancy is unchanged), failed,
/// or **panicked** mid-`await`. Without it a panic between reserving and releasing a
/// slot would leak occupancy permanently and wedge the refill loop below `cap`; there
/// is no reachable panic site today, so this is defence-in-depth on the
/// `ready + in_flight <= cap` accounting.
struct SlotReservation {
    inner: Arc<PoolInner>,
}

impl Drop for SlotReservation {
    fn drop(&mut self) {
        // `unwrap_or_else(into_inner)` rather than `unwrap`: a poisoned lock (some
        // other holder panicked) must not turn a slot-release into a second panic —
        // that would abort during unwind. `saturating_sub` floors at 0.
        let mut n = self
            .inner
            .in_flight
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *n = n.saturating_sub(1);
    }
}

impl WarmPool {
    /// Build a warm pool for `python`, keeping up to `min(POOL_CAP, cap)` kernels
    /// ready. Boots the forkserver daemon and kicks off background pre-warming; if
    /// the daemon can't boot, the pool is inert (every `take` returns `None`) and
    /// the caller falls back to cold starts — never worse than today.
    ///
    /// Pass `cap == 0` to disable warm pooling entirely (inert pool).
    pub async fn new(python: &std::path::Path, cap: usize) -> WarmPool {
        let cap = cap.min(POOL_CAP);
        let daemon = if cap == 0 {
            None
        } else {
            match ForkserverDaemon::boot(python).await {
                Ok(d) => {
                    crate::log::kernel(&format!(
                        "warm-pool: forkserver ready (preloaded: {})",
                        if d.preloaded.is_empty() {
                            "none".to_string()
                        } else {
                            d.preloaded.join(", ")
                        }
                    ));
                    Some(Arc::new(d))
                }
                Err(e) => {
                    crate::log::warn(&format!(
                        "warm-pool: forkserver unavailable ({e}); using cold kernel starts"
                    ));
                    None
                }
            }
        };
        let inner = Arc::new(PoolInner {
            daemon,
            python: python.to_path_buf(),
            cap,
            ready: Mutex::new(VecDeque::new()),
            in_flight: std::sync::Mutex::new(0),
        });
        if inner.daemon.is_some() {
            PoolInner::refill(Arc::clone(&inner));
        }
        WarmPool { inner }
    }

    /// Claim a ready warm kernel, or `None` to signal "fall back to a cold
    /// `Kernel::start`". Every take triggers a background refill so the pool re-warms
    /// toward `cap` without blocking the caller.
    pub async fn take(&self) -> Option<Kernel> {
        // Inert pool (no forkserver): signal the caller to cold-start.
        self.inner.daemon.as_ref()?;
        let kernel = {
            let mut ready = self.inner.ready.lock().await;
            ready.pop_front()
        };
        // Refill on a MISS too, not just a hit. A miss is precisely the state that
        // needs re-warming: the queue is empty, so if the last refill pass died on a
        // fork failure there is nothing left to restart it, and gating the trigger on
        // a hit made "empty" a state the pool could never leave. Every later take
        // missed, every later take skipped the refill, and the pool advertised warmth
        // while silently cold-starting forever. The trigger is cheap when it is
        // pointless: a pool already at `cap` reserves no slot and the task exits at
        // once. Do NOT touch `in_flight` here: a kernel sitting in `ready` already had
        // its in_flight slot released by the refill fork that produced it.
        // Decrementing here would under-count occupancy and let the next refill spawn
        // an extra fork, transiently pushing resident kernels to cap+1 and
        // overshooting the RAM budget.
        PoolInner::refill(Arc::clone(&self.inner));
        kernel
    }

    /// Whether this pool is backed by a live forkserver (vs. inert/fallback-only).
    /// A diagnostic accessor currently exercised only by the kernel-gated tests, so
    /// it reads as dead code in a plain (no-kernel) build.
    #[allow(dead_code)]
    pub fn is_warm(&self) -> bool {
        self.inner.daemon.is_some()
    }

    /// The number of kernels this pool keeps pre-warmed (its effective `cap`).
    /// Diagnostics/tests; equals `min(requested, POOL_CAP)`, `0` for an inert pool.
    /// Only the kernel-gated tests call it today, so allow the otherwise-dead surface.
    #[allow(dead_code)]
    pub fn capacity(&self) -> usize {
        self.inner.cap
    }

    /// How many kernels are currently sitting ready in the queue. Test-only: lets a
    /// test wait for the background pre-warm to land at least one kernel so a `take`
    /// is a deterministic *hit* rather than a race that could legitimately fall back.
    #[cfg(test)]
    pub(crate) async fn ready_len(&self) -> usize {
        self.inner.ready.lock().await.len()
    }

    /// The forkserver helper's pid == the process-group id of the whole subtree
    /// (`process_group(0)`). Test-only, for the teardown regression test.
    #[cfg(test)]
    fn helper_pid(&self) -> Option<u32> {
        self.inner.daemon.as_ref().and_then(|d| d.helper_pid)
    }
}

/// Whether the warm pool should boot for an interpreter of this provenance. A
/// concretely-chosen interpreter (a `_site.yml` field, a project `.venv`, or
/// `TALIESIN_PYTHON`) is worth pre-warming; the bare `python3` default is not (we
/// never speculatively boot a forkserver against a possibly-absent `python3`). Pure,
/// unit-tested without a live kernel (mirrors [`try_reserve_slot`]'s pure-core style).
fn should_warm(prov: crate::interpreter::Provenance) -> bool {
    !matches!(prov, crate::interpreter::Provenance::Default)
}

/// Build the one warm pool a **preview server** owns, warming the resolved `python`.
/// Returns `None` (so every page cold-starts, exactly as before) when the interpreter
/// is the bare `python3` default; otherwise boots the forkserver. If that boot fails
/// the returned pool is inert and the caller still cold-starts (no regression).
///
/// The preview builder runs pages serially (one build kernel at a time), so the
/// resident set during a build is `warm_pool + 1`; we ask for the budget-split warm
/// size against the same memory cap the build uses, then let `WarmPool::new` clamp it
/// to [`POOL_CAP`]. Wrapped in an `Arc` so it's shared by every page executor.
pub async fn warm_pool_for_preview(python: &crate::interpreter::Resolved) -> Option<Arc<WarmPool>> {
    let want = crate::build_budget::preview_warm_pool_size();
    boot_pool(want, python).await
}

/// Build the one warm pool a **site build** owns, asking for `size` pre-warmed kernels
/// of the resolved `python` (already reconciled against the build's memory budget by
/// `budget_split`, so `warm_pool + build_kernels <= cap`). Returns `None` (every page
/// cold-starts, exactly as before) when `size == 0` or the interpreter is the bare
/// `python3` default. Dropped at the end of the build, killing the daemon + idle kernels.
pub async fn warm_pool_for_build(
    size: usize,
    python: &crate::interpreter::Resolved,
) -> Option<Arc<WarmPool>> {
    if size == 0 {
        return None;
    }
    boot_pool(size, python).await
}

/// Boot a warm pool of `want` kernels of the resolved `python`, or `None` when the
/// interpreter wasn't concretely chosen (the bare default), so we never speculatively
/// boot a forkserver against a possibly-absent `python3`; the caller then cold-starts
/// as before. A boot *failure* isn't `None` here: `WarmPool::new` degrades to an inert
/// pool that returns `None` from `take`, which the executor treats as a cold start.
async fn boot_pool(want: usize, python: &crate::interpreter::Resolved) -> Option<Arc<WarmPool>> {
    if !should_warm(python.provenance) {
        return None;
    }
    // CMD-01: name the fully-resolved interpreter, exactly as the cold kernel-start path
    // does ("starting python (…)"). The pool is the one place an interpreter gets launched
    // without ever being printed, so a `python:`/`r:` injected by a `_site.yml` was silently
    // the thing running the author's code. Naming what you are about to execute against is
    // not an escalation over executing against it — it just stops the choice being invisible.
    crate::log::kernel(&format!(
        "warming {want} python ({})",
        python.path.display()
    ));
    Some(Arc::new(WarmPool::new(&python.path, want).await))
}

/// Decide whether the refill loop may reserve one more fork slot, given the
/// kernels already `ready` and the forks already `in_flight`. The warm pool's core
/// invariant is `ready + in_flight <= cap`: a slot may be reserved only while the
/// current occupancy is **strictly** below `cap`, so the pool fills to exactly
/// `cap` and never overshoots to `cap + 1` (each surplus kernel costs a whole
/// interpreter's RAM against the build budget). Returns the new `in_flight` count
/// to commit when a reservation is allowed, or `None` when the pool is already full.
///
/// This is the pure arithmetic the three accounting fixes (fill-to-cap and the
/// no-decrement-on-take rule) all turned on; it is unit-tested without a live
/// kernel so a fourth regression is caught by CI rather than by RAM.
fn try_reserve_slot(ready_len: usize, in_flight: usize, cap: usize) -> Option<usize> {
    if ready_len + in_flight >= cap {
        None
    } else {
        Some(in_flight + 1)
    }
}

impl PoolInner {
    /// Spawn background tasks to bring the pool up to `cap` ready kernels. Each
    /// task forks one kernel off the warm daemon, connects it, and pushes it onto
    /// the ready queue. A failed warm-up is logged once and dropped (the next
    /// `take` simply falls back), so it never breaks the caller.
    fn refill(inner: Arc<PoolInner>) {
        let Some(daemon) = inner.daemon.clone() else {
            return;
        };
        tokio::spawn(async move {
            loop {
                // Reserve a slot up to `cap`, counting ready + in-flight kernels.
                // `try_reserve_slot` is the pure, unit-tested arithmetic guarding
                // the `ready + in_flight <= cap` invariant. Holding the `ready`
                // (tokio) lock across the `in_flight` (std) tweak serialises the
                // whole decide-then-increment against other reservers *and* against
                // a concurrent `SlotReservation` drop (both take `in_flight`), so no
                // update is lost. Lock order is always `ready` → `in_flight`.
                let reservation = {
                    let ready = inner.ready.lock().await;
                    let mut n = inner.in_flight.lock().unwrap_or_else(|e| e.into_inner());
                    match try_reserve_slot(ready.len(), *n, inner.cap) {
                        Some(next) => *n = next,
                        None => return,
                    }
                    SlotReservation {
                        inner: Arc::clone(&inner),
                    }
                };
                match Self::warm_one(&inner.python, &daemon).await {
                    Ok(kernel) => {
                        // Push into `ready` *before* releasing the slot: for the
                        // brief overlap the kernel counts in both terms (harmless
                        // over-count), never in neither (which would let a racing
                        // reserver overshoot `cap`).
                        inner.ready.lock().await.push_back(kernel);
                        drop(reservation);
                    }
                    Err(e) => {
                        // `reservation` drops here, returning the slot. End this refill
                        // *pass* rather than spinning on a daemon that just failed
                        // `FORK_ATTEMPTS` times: the next `take` re-arms the refill (see
                        // `WarmPool::take`), so stopping here costs one cold start, not
                        // the pool. Looping instead would hot-spin against a broken
                        // daemon with nothing to pace it.
                        drop(reservation);
                        crate::log::warn(&format!(
                            "warm-pool: pre-warm failed ({e}); kernel will cold-start on demand"
                        ));
                        return;
                    }
                }
            }
        });
    }

    /// Fork one warm kernel, retrying a recoverable failure before giving up.
    ///
    /// The cold path already does this: `ensure_kernel` re-rolls `Kernel::start` up to
    /// four times because a lost `peek_ports` race is a coin flip, not a verdict. The
    /// warm path forks against the *same* `prepare_connection` ports and the same
    /// `adopt_forked` handshake, so it loses the same coin flips — and it had zero
    /// retries, spending each one straight out of the pool. Mirror the cold path's
    /// shape (attempt-scaled backoff, bail at once on a permanent failure so an honest
    /// error is not delayed) so a recoverable hiccup costs a retry rather than the
    /// pool's warmth.
    async fn warm_one(
        python: &std::path::Path,
        daemon: &Arc<ForkserverDaemon>,
    ) -> std::io::Result<Kernel> {
        let spec = KernelSpec::python(python);
        let mut attempt = 1;
        loop {
            let err = match Self::warm_one_attempt(&spec, daemon).await {
                Ok(kernel) => return Ok(kernel),
                Err(e) => e,
            };
            if attempt >= FORK_ATTEMPTS || !fork_error_is_retryable(&err.to_string()) {
                return Err(err);
            }
            crate::log::warn(&format!(
                "warm-pool: pre-warm hit a recoverable failure ({err}); \
                 retrying ({attempt}/{FORK_ATTEMPTS})"
            ));
            tokio::time::sleep(Duration::from_millis(40 * attempt as u64)).await;
            attempt += 1;
        }
    }

    /// One fork attempt: create the connection, ask the daemon to fork an in-process
    /// ipykernel bound to it, and complete the ZMQ handshake + preambles. The
    /// connection file is created by taliesin (same locked-down 0600/0700 path as
    /// `Kernel::start`), and `Kernel::adopt_forked` connects.
    async fn warm_one_attempt(
        spec: &KernelSpec,
        daemon: &Arc<ForkserverDaemon>,
    ) -> std::io::Result<Kernel> {
        let (info, conn_dir, conn_file) = prepare_connection(spec.kernel_name()).await?;
        // Between `prepare_connection` and `adopt_forked` the 0700 dir — holding a 0600
        // `connection.json` with the kernel's HMAC key — has no owner: `Kernel::start`
        // guards this window with `ConnDirGuard` and `adopt_forked` guards the next one
        // with `ForkedCleanup`, but the fork *between* them was left unguarded, so every
        // failed fork orphaned a dir and its key for the life of the machine. Same
        // hazard, same guard: arm it over the fork, and disarm the instant `adopt_forked`
        // takes over (it arms `ForkedCleanup` before its first fallible step).
        let dir_guard = crate::kernel::ConnDirGuard::arm(conn_dir);
        let pid = daemon.fork_kernel(&conn_file).await?;
        let conn_dir = dir_guard.disarm();
        Kernel::adopt_forked(pid, info, conn_dir, Arc::clone(daemon), spec).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::render_outputs;

    fn python() -> Option<PathBuf> {
        std::env::var_os("TALIESIN_PYTHON").map(PathBuf::from)
    }

    /// A stand-in forkserver daemon: an executable script that speaks the fork
    /// protocol on stdin/stdout, so a test can drive the failure modes a real daemon
    /// only produces under rare load (an `ERROR` reply, a reply that outruns the
    /// client's deadline). `ForkserverDaemon::boot` runs it as the "python" program and
    /// passes it the real helper path + preload JSON, which the stand-in ignores.
    struct FakeDaemon {
        dir: PathBuf,
        script: PathBuf,
    }

    impl Drop for FakeDaemon {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn fake_daemon(py: &std::path::Path, body: &str) -> FakeDaemon {
        let dir = std::env::temp_dir().join(format!("tali-faked-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake_forkserver.py");
        // Route the shebang through `/usr/bin/env` so it works whether `py` is an
        // absolute path (local dev) or a bare name like `python` (CI's TALIESIN_PYTHON):
        // the kernel does not PATH-resolve a shebang interpreter, but `env` does, and it
        // execs an absolute path just as happily. A bare-name shebang is exec'd directly
        // by `ForkserverDaemon::boot` and fails with ENOENT.
        std::fs::write(&script, format!("#!/usr/bin/env {}\n{body}", py.display())).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        FakeDaemon { dir, script }
    }

    /// A failed fork must not leak the `/tmp/tali-kernel-<uuid>` dir it created. The
    /// dir is 0700 and holds a 0600 `connection.json` carrying the kernel's HMAC key,
    /// and nothing ever swept it: `prepare_connection` created it, the fork failed, the
    /// `?` returned, and the dir outlived the process. Measured before the fix: one
    /// orphaned dir + key per failed fork. Drives the failure with a stand-in daemon
    /// that always answers `ERROR` and records the exact paths it was asked to fork, so
    /// the assertion names those dirs rather than globbing `/tmp` (which would race
    /// every other test in this binary).
    #[test]
    fn failed_fork_does_not_leak_its_connection_dir() {
        let Some(py) = python() else {
            eprintln!("SKIPPED: no TALIESIN_PYTHON");
            return;
        };
        let fake = fake_daemon(
            &py,
            r#"
import sys, os
HERE = os.path.dirname(os.path.abspath(__file__))
log = open(os.path.join(HERE, "requests.log"), "a")
sys.stdout.write("READY []\n"); sys.stdout.flush()
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    rid, _, conn = line.partition(" ")
    if not conn:
        rid, conn = "", line
    log.write(conn + "\n"); log.flush()
    sys.stdout.write(("ERROR %s\n" % rid) if rid else "ERROR\n"); sys.stdout.flush()
"#,
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let d = Arc::new(ForkserverDaemon::boot(&fake.script).await.unwrap());
            assert!(
                PoolInner::warm_one(&py, &d).await.is_err(),
                "an always-ERROR daemon must fail the pre-warm"
            );
            let log = std::fs::read_to_string(fake.dir.join("requests.log")).unwrap();
            let asked: Vec<&str> = log.lines().filter(|l| !l.is_empty()).collect();
            // Every retry creates its own connection dir; each one must be swept.
            assert_eq!(
                asked.len(),
                FORK_ATTEMPTS,
                "an `ERROR` reply is retryable, so the pre-warm should have used every attempt"
            );
            for conn in asked {
                let dir = std::path::Path::new(conn).parent().unwrap();
                assert!(
                    !dir.exists(),
                    "a failed fork must not leak its connection dir: {}",
                    dir.display()
                );
            }
        });
    }

    /// The pool must re-warm after a fork failure instead of going dark for good.
    ///
    /// The refill task ends its pass on a failed fork, and a `take` is what re-arms it
    /// — but the trigger used to be gated on the take being a *hit*, and a take can
    /// only hit when the queue is non-empty, which is exactly what a dead refill
    /// prevents. So one fork hiccup retired the pool permanently while `is_warm()` kept
    /// answering `true`. Measured before the fix: `ready_len` stayed 0 for the full 15s
    /// poll below.
    ///
    /// The stand-in daemon fails `FORK_ATTEMPTS` requests (exhausting one whole refill
    /// pass, retries included) and then serves real kernels, so the only way the pool
    /// recovers is a take re-arming the refill.
    #[test]
    fn pool_rewarms_after_a_fork_failure() {
        let Some(py) = python() else {
            eprintln!("SKIPPED: no TALIESIN_PYTHON");
            return;
        };
        // Fails every attempt of the first refill pass, then serves real ipykernels.
        let fake = fake_daemon(
            &py,
            &format!(
                r#"
import sys, os, subprocess
FAIL_FIRST = {FORK_ATTEMPTS}
sys.stdout.write("READY []\n"); sys.stdout.flush()
n = 0
devnull = open(os.devnull, "wb")
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    rid, _, conn = line.partition(" ")
    n += 1
    if n <= FAIL_FIRST:
        sys.stdout.write("ERROR %s\n" % rid); sys.stdout.flush()
        continue
    p = subprocess.Popen([sys.executable, "-m", "ipykernel_launcher", "-f", conn],
                         stdout=devnull, stderr=devnull)
    sys.stdout.write("SPAWNED %s %d\n" % (rid, p.pid)); sys.stdout.flush()
"#
            ),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let pool = WarmPool::new(&fake.script, POOL_CAP).await;
            assert!(pool.is_warm(), "the stand-in daemon should report READY");
            // Let the boot-time refill pass burn its attempts on the scripted failures
            // and die (4 `ERROR`s + ~240ms of backoff), so what follows tests the
            // re-arm and not a pass that was still running anyway.
            tokio::time::sleep(Duration::from_secs(2)).await;
            assert_eq!(
                pool.ready_len().await,
                0,
                "the first refill pass should have failed"
            );
            // The daemon serves kernels again. A take is the pool's only way back:
            // it misses (nothing ready) and must still re-arm the refill.
            assert!(pool.take().await.is_none(), "nothing ready yet -> a miss");
            let mut refilled = false;
            for _ in 0..150 {
                if pool.ready_len().await > 0 {
                    refilled = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            assert!(
                refilled,
                "the pool must re-warm after a fork failure, not stay dark forever"
            );
            // And the re-warmed kernel is real: a pool that recovers on paper but hands
            // out a dead kernel has not recovered.
            let mut kernel = pool.take().await.expect("the re-warmed kernel is takeable");
            let html = render_outputs(&kernel.execute("print(1 + 1)").await.unwrap());
            assert!(
                html.contains('2'),
                "re-warmed kernel did not return 2: {html}"
            );
        });
    }

    /// A fork request that outran its deadline must not poison the next one.
    ///
    /// The daemon is single-threaded and does not know a client gave up: it finishes
    /// the fork and writes the reply anyway, leaving it in the pipe ahead of the next
    /// caller's read. Before the reply carried its request's id there was nothing to
    /// tell them apart, and the next fork returned the previous request's pid —
    /// measured: request two got `Ok(111111)`, request one's kernel. The pool would
    /// then hand out a `Kernel` whose ports belong to a different connection file.
    ///
    /// Also pins the reclaim: the abandoned request's process is real and running, and
    /// nothing else will ever kill it, so the late reply must be a chance to reap it.
    /// The stand-in daemon spawns a plain long-lived process per request (no kernel
    /// needed to prove pid bookkeeping) and logs the ids + pids it handed out.
    #[test]
    #[cfg(target_os = "linux")]
    fn a_timed_out_fork_does_not_mispair_a_later_reply() {
        let Some(py) = python() else {
            eprintln!("SKIPPED (no live kernel): set TALIESIN_PYTHON to test fork correlation.");
            return;
        };
        let fake = fake_daemon(
            &py,
            r#"
import sys, os, time, subprocess
HERE = os.path.dirname(os.path.abspath(__file__))
log = open(os.path.join(HERE, "requests.log"), "a")
sys.stdout.write("READY []\n"); sys.stdout.flush()
n = 0
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    rid, _, conn = line.partition(" ")
    n += 1
    # A stand-in for the forked kernel: a real process with a real pid, in the
    # daemon's process group, that outlives the reply.
    p = subprocess.Popen(["sleep", "300"])
    log.write("%s %d\n" % (rid, p.pid)); log.flush()
    if n == 1:
        time.sleep(2.0)   # outrun the client's deadline, then answer anyway
    sys.stdout.write("SPAWNED %s %d\n" % (rid, p.pid)); sys.stdout.flush()
"#,
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let d = ForkserverDaemon::boot(&fake.script).await.unwrap();
            let group = d.helper_pid.expect("a booted daemon has a helper pid");
            let first = d
                .fork_kernel_within(
                    std::path::Path::new("/tmp/tali-test-req-one/connection.json"),
                    Duration::from_millis(300),
                )
                .await;
            assert!(
                first.is_err(),
                "the stalled request must time out, not wait forever: {first:?}"
            );
            let second = d
                .fork_kernel_within(
                    std::path::Path::new("/tmp/tali-test-req-two/connection.json"),
                    Duration::from_secs(5),
                )
                .await
                .expect("the second request must be answered");

            // What the daemon actually handed out, in order. Poll: the daemon logs a
            // request as it serves it, and a client that returned the WRONG pid can get
            // here before the daemon has even logged the request that pid belongs to.
            let handed_out = || -> Vec<u32> {
                std::fs::read_to_string(fake.dir.join("requests.log"))
                    .unwrap_or_default()
                    .lines()
                    .filter_map(|l| l.split_whitespace().nth(1))
                    .filter_map(|p| p.parse().ok())
                    .collect()
            };
            let mut handed = handed_out();
            for _ in 0..50 {
                if handed.len() >= 2 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
                handed = handed_out();
            }
            assert_eq!(handed.len(), 2, "the daemon should have served two requests");
            let (abandoned, mine) = (handed[0], handed[1]);
            assert_eq!(
                second, mine,
                "the second request must get ITS OWN pid ({mine}), not the abandoned request's ({abandoned})"
            );

            // The abandoned request's process is nobody's: it must have been reclaimed
            // off the late reply, while the live request's is left alone.
            let mut reclaimed = false;
            for _ in 0..50 {
                if !live_group_members(group).contains(&abandoned) {
                    reclaimed = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            assert!(
                reclaimed,
                "the abandoned request's kernel must be reclaimed, not left holding RAM + ports"
            );
            assert!(
                live_group_members(group).contains(&mine),
                "the live request's kernel must survive the reclaim"
            );
        });
    }

    /// The fork protocol's wire format, pinned without a daemon or a kernel: a reply
    /// must be attributable to exactly one request, and anything unrecognized must be
    /// a skippable stray rather than a failure (a banner on the pipe must not break an
    /// otherwise-good fork).
    #[test]
    fn fork_replies_parse_to_their_request() {
        assert_eq!(
            parse_fork_reply("SPAWNED 7 4242"),
            ForkReply::Spawned { id: 7, pid: 4242 }
        );
        assert_eq!(parse_fork_reply("ERROR 7"), ForkReply::Error { id: 7 });
        // The pre-id shapes are not replies to anything: reading one as "my pid" is
        // exactly the desync this protocol fixed, so it must be a stray, not a pid.
        assert_eq!(parse_fork_reply("SPAWNED 4242"), ForkReply::Stray);
        assert_eq!(parse_fork_reply("ERROR"), ForkReply::Stray);
        // Junk stays skippable rather than fatal.
        assert_eq!(parse_fork_reply(""), ForkReply::Stray);
        assert_eq!(
            parse_fork_reply("NOTE: Ctrl-C will not work"),
            ForkReply::Stray
        );
        assert_eq!(parse_fork_reply("SPAWNED x 1"), ForkReply::Stray);
        assert_eq!(parse_fork_reply("SPAWNED 1 -3"), ForkReply::Stray);
        assert_eq!(parse_fork_reply("SPAWNED 1 2 3"), ForkReply::Stray);
    }

    /// A pre-warm retries what the daemon says it can still serve, and gives up at once
    /// on what it cannot. The `ERROR` reply is the whole point: the daemon reports a
    /// failed fork and goes back to reading stdin, so the next attempt is worth making
    /// — the client used to treat that as terminal. Pure, no daemon.
    #[test]
    fn only_recoverable_fork_failures_are_retried() {
        // The daemon said "this one failed", not "I am done".
        assert!(fork_error_is_retryable(
            "forkserver: fork failed (daemon reported ERROR)"
        ));
        // Now that replies carry ids, a retry after a timeout is safe (it was the
        // mis-pairing trigger before) and worth making.
        assert!(fork_error_is_retryable(
            "forkserver: fork request timed out"
        ));
        // The same port race the cold path re-rolls, reached through `prepare_connection`.
        assert!(fork_error_is_retryable("address already in use"));
        // The daemon is gone: every retry would fail identically, just later.
        assert!(!fork_error_is_retryable("forkserver: closed during fork"));
        assert!(!fork_error_is_retryable(
            "Broken pipe (os error 32)".to_ascii_lowercase().as_str()
        ));
        // Permanent by the cold path's own classification: don't delay an honest error.
        assert!(!fork_error_is_retryable(
            "`python3` exited at startup: No module named ipykernel"
        ));
    }

    /// The refill loop reserves slots one at a time until occupancy reaches `cap`,
    /// then stops — so a pool with nothing yet connected (`ready == 0`) fills to
    /// exactly `cap` outstanding forks and not one more. Pins the "fills to cap"
    /// accounting fix; deterministic, no live kernel.
    #[test]
    fn reserve_fills_to_exactly_cap_and_never_overshoots() {
        let cap = POOL_CAP; // 2
        let ready = 0; // nothing connected yet; every reservation is still in-flight
        let mut in_flight = 0;
        let mut reservations = 0;
        // Mirror refill's loop: keep reserving until `try_reserve_slot` bails.
        while let Some(next) = try_reserve_slot(ready, in_flight, cap) {
            in_flight = next;
            reservations += 1;
            assert!(
                ready + in_flight <= cap,
                "occupancy overshot cap ({} + {} > {})",
                ready,
                in_flight,
                cap
            );
            assert!(
                reservations <= cap,
                "reserve loop did not terminate at cap (runaway forks)"
            );
        }
        assert_eq!(in_flight, cap, "pool should reserve exactly cap slots");
    }

    /// After a `take`, the popped kernel leaves `ready` but `in_flight` is left
    /// untouched (the no-decrement-on-take rule). The refill the take triggers may
    /// then reserve exactly one replacement slot — never two — because an already
    /// in-flight fork still counts toward occupancy. Pins the cap+1 regression that
    /// a decrement-on-take would reintroduce; deterministic, no live kernel.
    #[test]
    fn take_keeps_in_flight_so_refill_holds_cap() {
        let cap = POOL_CAP; // 2
        // State at cap: one kernel ready, one fork still in flight.
        let ready_before = 1;
        let in_flight = 1;
        assert!(
            try_reserve_slot(ready_before, in_flight, cap).is_none(),
            "at cap, no slot may be reserved"
        );
        // A take pops the ready kernel and (correctly) leaves in_flight alone.
        let ready_after = ready_before - 1; // 0
        let in_flight_after = in_flight; // UNCHANGED — the no-decrement rule
        // The refill take triggers may reserve exactly one replacement...
        let first = try_reserve_slot(ready_after, in_flight_after, cap);
        assert_eq!(
            first,
            Some(2),
            "may reserve one replacement for the taken kernel"
        );
        // ...and then no more: a second reservation would push forks to cap+1.
        assert!(
            try_reserve_slot(ready_after, first.unwrap(), cap).is_none(),
            "must not reserve a (cap+1)th slot after the replacement"
        );
    }

    /// An inert/zero-cap pool never reserves a slot. Guards the `cap == 0`
    /// (warm-pooling disabled) path so a disabled pool stays at zero forks.
    #[test]
    fn zero_cap_reserves_nothing() {
        assert!(try_reserve_slot(0, 0, 0).is_none());
    }

    /// The pool boots for a concretely-chosen interpreter (field/.venv/env) and stays
    /// inert on the bare `python3` default, preserving "don't speculatively boot a
    /// possibly-absent python3" while now warming a project's `.venv`/pin. Pure gate,
    /// no live kernel.
    #[test]
    fn should_warm_only_for_a_concrete_interpreter() {
        use crate::interpreter::Provenance;
        assert!(should_warm(Provenance::Field));
        assert!(should_warm(Provenance::Venv));
        assert!(should_warm(Provenance::Env));
        assert!(
            !should_warm(Provenance::Default),
            "bare python3 must stay inert"
        );
    }

    /// Take a kernel from the warm pool and prove it is a live, usable kernel by
    /// running a trivial cell (`1 + 1` -> 2). Kernel-gated: without
    /// `TALIESIN_PYTHON` it reports ok without exercising a kernel.
    #[test]
    fn warm_pool_take_yields_a_live_kernel() {
        let Some(py) = python() else {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with \
                 ipykernel to exercise the warm pool; this run did not."
            );
            return;
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let pool = WarmPool::new(&py, POOL_CAP).await;
            assert!(
                pool.is_warm(),
                "forkserver should boot with a real python (preload skips missing libs)"
            );
            // The pool pre-warms in the background; poll briefly for a ready kernel.
            let mut taken = None;
            for _ in 0..100 {
                if let Some(k) = pool.take().await {
                    taken = Some(k);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let mut kernel = taken.expect("warm pool should hand out a kernel within 10s");
            assert!(kernel.is_alive(), "forked kernel should be alive");
            let html = render_outputs(&kernel.execute("print(1 + 1)").await.unwrap());
            assert!(html.contains('2'), "warm kernel did not return 2: {html}");
            // Warmth: heavy libs were preloaded in the forkserver and inherited COW.
            let warm = render_outputs(
                &kernel
                    .execute("import sys; print('numpy' in sys.modules)")
                    .await
                    .unwrap(),
            );
            assert!(
                warm.contains("True"),
                "numpy should be preloaded (COW from forkserver): {warm}"
            );
        });
    }

    /// Forcing forkserver init to fail (a bogus interpreter) must leave the pool
    /// inert so `take` returns `None`; the caller then cold-starts a working
    /// kernel via `Kernel::start`. Proves behaviour is never worse than today.
    #[test]
    fn warm_pool_falls_back_when_forkserver_init_fails() {
        let Some(py) = python() else {
            eprintln!("SKIPPED (no live kernel): set TALIESIN_PYTHON to test the fallback path.");
            return;
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            // A non-existent interpreter cannot boot a forkserver -> inert pool.
            let bogus = PathBuf::from("/nonexistent/definitely-not-python");
            let pool = WarmPool::new(&bogus, POOL_CAP).await;
            assert!(
                !pool.is_warm(),
                "bogus interpreter must not produce a forkserver"
            );
            assert!(
                pool.take().await.is_none(),
                "inert pool must return None so the caller falls back"
            );
            // The fallback the caller uses (a cold start with a real python) still
            // produces a working kernel — behaviour is no worse than before. Via
            // `start_with_retry`, which is the fallback the caller actually takes, so
            // this does not lose the `peek_ports` race against the rest of the suite.
            let mut k = Kernel::start_with_retry(&KernelSpec::python(&py), None)
                .await
                .expect("cold fallback kernel should start");
            let html = render_outputs(&k.execute("print(1 + 1)").await.unwrap());
            assert!(
                html.contains('2'),
                "fallback kernel did not return 2: {html}"
            );
        });
    }

    /// A `SlotReservation` returns its in-flight slot on drop — including when the
    /// fork it guards panics mid-`await`. Pins the accounting-leak hardening: a leaked
    /// slot would wedge the refill loop permanently below `cap`. Deterministic, no
    /// live kernel.
    #[test]
    fn slot_reservation_releases_in_flight_on_drop_and_on_panic() {
        let inner = Arc::new(PoolInner {
            daemon: None,
            python: PathBuf::from("python3"),
            cap: POOL_CAP,
            ready: Mutex::new(VecDeque::new()),
            in_flight: std::sync::Mutex::new(1),
        });
        let read = |i: &PoolInner| *i.in_flight.lock().unwrap();

        // A normal drop releases exactly one slot.
        {
            let _r = SlotReservation {
                inner: Arc::clone(&inner),
            };
            assert_eq!(
                read(&inner),
                1,
                "slot still counted while the guard is held"
            );
        }
        assert_eq!(read(&inner), 0, "drop must release the slot");

        // A panic while the reservation is held still releases it (the whole point).
        *inner.in_flight.lock().unwrap() = 1;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _r = SlotReservation {
                inner: Arc::clone(&inner),
            };
            panic!("fork blew up mid-await");
        }));
        assert!(caught.is_err(), "the panic should propagate");
        assert_eq!(read(&inner), 0, "a panic must not leak the in-flight slot");

        // Release floors at zero and never underflows.
        {
            let _r = SlotReservation {
                inner: Arc::clone(&inner),
            };
        }
        assert_eq!(read(&inner), 0, "release saturates at zero");
    }

    /// Dropping the pool tears down the ENTIRE forkserver subtree — the server the
    /// helper booted and the kernels it forked — not just the helper. Regression
    /// guard for the orphaned-forkserver leak (~100 MB each): without the
    /// process-group teardown the server survived its helper's death and reparented
    /// to init. Kernel-gated + Linux-only (reads `/proc`).
    #[test]
    #[cfg(target_os = "linux")]
    fn dropping_pool_reaps_the_whole_forkserver_group() {
        let Some(py) = python() else {
            eprintln!("SKIPPED (no live kernel): set TALIESIN_PYTHON to test forkserver teardown.");
            return;
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let pool = WarmPool::new(&py, POOL_CAP).await;
            // `is_warm` means the READY handshake completed, i.e. the forkserver
            // server booted — so the helper + server are already in the group.
            assert!(pool.is_warm(), "forkserver should boot with a real python");
            // Best-effort: also pull a forked kernel into the group, to prove kernels
            // are reaped too. The pre-warm fork can be flaky for reasons unrelated to
            // teardown (ipykernel's stdout NOTE racing the SPAWNED protocol), so this
            // is not required — the helper + server subtree is exercised regardless.
            let mut taken = None;
            for _ in 0..50 {
                if let Some(k) = pool.take().await {
                    taken = Some(k);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let pgid = pool.helper_pid().expect("a warm pool has a helper pid");

            // The isolated group is populated before teardown: the helper leads it
            // (pgid == its pid) and the server (+ any kernel) inherit it.
            let before = live_group_members(pgid);
            assert!(
                before.len() >= 2,
                "expected at least the helper + forkserver server in the group, saw {before:?}"
            );

            // Drop everything pinning the daemon `Arc`: the handed-out kernel (if any),
            // then the pool. The last `Arc` drop runs `Drop for ForkserverDaemon`,
            // which SIGKILLs the whole group.
            drop(taken);
            drop(pool);

            // Poll until no live (non-zombie) member of the group remains. The helper is
            // our child (reaped by tokio's kill_on_drop reaper); the server + kernels
            // are systemd's, reaped by the OS on death. Budget generously (> FORK_TIMEOUT):
            // the `take` above triggers a background refill that pins the daemon `Arc`
            // while it forks, deferring the group-kill until that fork returns.
            let mut cleared = false;
            for _ in 0..150 {
                if live_group_members(pgid).is_empty() {
                    cleared = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            assert!(
                cleared,
                "the whole forkserver group must be reaped on pool drop; survivors: {:?}",
                live_group_members(pgid)
            );
        });
    }

    /// An *ungraceful* taliesin death (SIGKILL, crash, closed terminal) never runs
    /// `Drop for ForkserverDaemon`, so the Rust-side group teardown never fires — yet
    /// the forkserver subtree (~100 MB per kernel) must still be reaped, or it orphans
    /// and reparents to init. The only signal the helper gets is EOF on its stdin when
    /// taliesin's fds close; the helper must turn that into the same process-group kill
    /// the graceful path performs. This drives that exact path: it closes our end of the
    /// helper's stdin (the faithful stand-in for the parent's death — a real death closes
    /// the identical write fd) WITHOUT dropping the daemon, then proves the whole group
    /// self-reaps. Measured before the fix: the forkserver server outlived the helper's
    /// EOF exit and survived. Kernel-gated + Linux-only (reads `/proc`).
    #[test]
    #[cfg(target_os = "linux")]
    fn ungraceful_parent_death_reaps_the_forkserver_subtree() {
        let Some(py) = python() else {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to test ungraceful-death reaping."
            );
            return;
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let d = ForkserverDaemon::boot(&py).await.unwrap();
            let pgid = d.helper_pid.expect("a booted daemon has a helper pid");
            // Best-effort: pull a real forked kernel into the group so the reap is proven
            // to reach kernels, not just the helper + server. Not required — the helper +
            // forkserver server are in the group regardless of whether this fork lands.
            let d = Arc::new(d);
            let _kernel = PoolInner::warm_one(&py, &d).await.ok();

            let before = live_group_members(pgid);
            assert!(
                before.len() >= 2,
                "expected at least the helper + forkserver server in the group, saw {before:?}"
            );

            // Simulate the ungraceful death: taliesin's stdin write end closes, the
            // helper's `for line in sys.stdin` hits EOF, and its teardown must reap the
            // group it leads. We do NOT drop `d` — its `Drop` would group-kill and hide
            // whether the helper reaps itself.
            d.close_stdin_for_test().await;

            let mut cleared = false;
            for _ in 0..100 {
                if live_group_members(pgid).is_empty() {
                    cleared = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            // Drop the daemon only AFTER the assertion window: by now the helper has
            // either reaped the group (fix present) or not (bug). `Drop`'s redundant
            // group-kill is then a harmless ESRCH no-op on an already-empty group.
            drop(d);
            assert!(
                cleared,
                "the helper must reap its forkserver subtree on parent death (stdin EOF); \
                 survivors: {:?}",
                live_group_members(pgid)
            );
        });
    }

    /// PIDs in process group `pgid` that are alive and not zombies, read from `/proc`.
    /// Backs the forkserver-teardown test: it proves the whole subtree is gone.
    #[cfg(target_os = "linux")]
    fn live_group_members(pgid: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return out;
        };
        for e in entries.flatten() {
            let name = e.file_name();
            let Some(name) = name.to_str() else { continue };
            let Ok(pid) = name.parse::<u32>() else {
                continue;
            };
            let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
                continue;
            };
            // `pid (comm) state ppid pgrp ...` — `comm` can contain spaces/parens, so
            // parse from the LAST ')': the fields after it are state, ppid, pgrp, ...
            let Some(rest) = stat.rsplit_once(')').map(|(_, r)| r.trim_start()) else {
                continue;
            };
            let mut fields = rest.split_whitespace();
            let state = fields.next().unwrap_or("");
            // skip ppid, take pgrp
            let pgrp = fields.nth(1).and_then(|s| s.parse::<u32>().ok());
            if pgrp == Some(pgid) && state != "Z" {
                out.push(pid);
            }
        }
        out
    }
}
