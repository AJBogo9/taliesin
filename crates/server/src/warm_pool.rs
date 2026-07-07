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
use tokio::time::{Duration, timeout};

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

/// The libraries the forkserver tries to preload. Each is included **only if
/// importable** in the target interpreter (the helper filters with
/// `importlib.util.find_spec`), so a missing `torch` simply isn't preloaded rather
/// than breaking the daemon.
const PRELOAD_CANDIDATES: &[&str] = &["numpy", "matplotlib", "torch"];

/// The embedded forkserver daemon. Boots a `multiprocessing` forkserver, preloads
/// the importable heavy libs, prints `READY <json-list>`, then for each connection
/// file path on stdin forks a child that runs `IPKernelApp` **in-process** and
/// prints `SPAWNED <pid>`. See the module docs for the warmth/exec rationale.
const FORKSERVER_HELPER: &str = r#"
import sys, os, json, importlib.util, multiprocessing as mp

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
        from ipykernel.kernelapp import IPKernelApp
        app = IPKernelApp.instance()
        app.initialize(["-f", conn_file])
        app.start()
    except Exception as e:
        sys.stderr.write("tali-warm: child kernel failed: %r\n" % (e,)); sys.stderr.flush()
        os._exit(1)

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
    for line in sys.stdin:
        conn = line.strip()
        if not conn:
            continue
        try:
            p = ctx.Process(target=_child_entry, args=(conn,))
            p.start()
            sys.stdout.write("SPAWNED %d\n" % (p.pid,)); sys.stdout.flush()
        except Exception as e:
            sys.stderr.write("tali-warm: fork failed: %r\n" % (e,)); sys.stderr.flush()
            sys.stdout.write("ERROR\n"); sys.stdout.flush()

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
}

impl Drop for ForkserverDaemon {
    fn drop(&mut self) {
        // `kill_on_drop` handles the process; clean up the helper script dir.
        let _ = std::fs::remove_dir_all(&self.helper_dir);
    }
}

struct DaemonIo {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl ForkserverDaemon {
    /// Boot the daemon with `python`, preloading the importable heavy libs. Errors
    /// (no interpreter, no `multiprocessing` forkserver, no `READY`) mean "no warm
    /// pool"; the caller falls back to cold starts.
    async fn boot(python: &std::path::Path) -> std::io::Result<ForkserverDaemon> {
        // The helper must be a real script file, not `python -c`: the forkserver
        // re-imports the target function's `__main__` by name, which fails for a
        // `-c` string module ("Can't get attribute '_child_entry' on __main__").
        // Write it to a private 0700 dir, removed when the daemon drops.
        let helper_dir =
            std::env::temp_dir().join(format!("tali-warmpool-{}", uuid::Uuid::new_v4()));
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
        let helper_path = helper_dir.join("qmd_forkserver.py");
        std::fs::write(&helper_path, FORKSERVER_HELPER)?;

        let preload_json = serde_json::to_string(PRELOAD_CANDIDATES).unwrap();
        let mut child = Command::new(python)
            .arg(&helper_path)
            .arg(preload_json)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                let _ = std::fs::remove_dir_all(&helper_dir);
                std::io::Error::other(format!(
                    "cannot launch warm-pool forkserver `{}`: {e}",
                    python.display()
                ))
            })?;
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
                let _ = std::fs::remove_dir_all(&helper_dir);
                return Err(e);
            }
        };

        Ok(ForkserverDaemon {
            _child: child,
            io: Mutex::new(DaemonIo {
                stdin,
                stdout: reader,
            }),
            preloaded,
            helper_dir,
        })
    }

    /// Ask the daemon to fork a kernel bound to `conn_file`; returns the kernel's
    /// PID. Serialised across callers by the io mutex.
    async fn fork_kernel(&self, conn_file: &std::path::Path) -> std::io::Result<u32> {
        let mut io = self.io.lock().await;
        let req = format!("{}\n", conn_file.display());
        io.stdin.write_all(req.as_bytes()).await?;
        io.stdin.flush().await?;
        let mut line = String::new();
        let n = timeout(FORK_TIMEOUT, io.stdout.read_line(&mut line))
            .await
            .map_err(|_| std::io::Error::other("forkserver: fork request timed out"))??;
        if n == 0 {
            return Err(std::io::Error::other("forkserver: closed during fork"));
        }
        let line = line.trim();
        let pid = line
            .strip_prefix("SPAWNED ")
            .and_then(|p| p.parse::<u32>().ok())
            .ok_or_else(|| std::io::Error::other(format!("forkserver: bad fork reply {line:?}")))?;
        Ok(pid)
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
    in_flight: Mutex<usize>,
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
            in_flight: Mutex::new(0),
        });
        if inner.daemon.is_some() {
            PoolInner::refill(Arc::clone(&inner));
        }
        WarmPool { inner }
    }

    /// Claim a ready warm kernel, or `None` to signal "fall back to a cold
    /// `Kernel::start`". Taking one triggers a background refill so the pool
    /// re-warms toward `cap` without blocking the caller.
    pub async fn take(&self) -> Option<Kernel> {
        // Inert pool (no forkserver): signal the caller to cold-start.
        self.inner.daemon.as_ref()?;
        let kernel = {
            let mut ready = self.inner.ready.lock().await;
            ready.pop_front()
        };
        if kernel.is_some() {
            // Trigger a background refill toward `cap`.  Do NOT touch
            // `in_flight`: a kernel sitting in `ready` already had its
            // in_flight slot released by the refill fork that produced it.
            // Decrementing here would under-count occupancy and let the next
            // refill spawn an extra fork, transiently pushing resident kernels
            // to cap+1 and overshooting the RAM budget.
            PoolInner::refill(Arc::clone(&self.inner));
        }
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
}

/// Build the one warm pool a **preview server** owns, sized within the same budget
/// the parallel build respects. Returns `None` (so every page cold-starts, exactly
/// as before) when `TALIESIN_PYTHON` is unset: we don't speculatively boot a
/// forkserver against a possibly-absent `python3`. When it *is* set, the pool boots
/// the forkserver; if that fails the returned pool is inert and the caller still
/// cold-starts (no regression).
///
/// The preview builder runs pages serially (one build kernel at a time), so the
/// resident set during a build is `warm_pool + 1`; we ask for the budget-split warm
/// size against the same memory cap the build uses, then let `WarmPool::new` clamp it
/// to [`POOL_CAP`]. Wrapped in an `Arc` so it's shared by every page executor.
pub async fn warm_pool_for_preview() -> Option<Arc<WarmPool>> {
    let want = crate::build_budget::preview_warm_pool_size();
    boot_pool(want).await
}

/// Build the one warm pool a **site build** owns, asking for `size` pre-warmed
/// kernels (already reconciled against the build's memory budget by
/// `budget_split`, so `warm_pool + build_kernels <= cap`). Returns `None` (every
/// page cold-starts, exactly as before) when `size == 0` or `TALIESIN_PYTHON` is
/// unset. Dropped at the end of the build, killing the daemon + idle kernels.
pub async fn warm_pool_for_build(size: usize) -> Option<Arc<WarmPool>> {
    if size == 0 {
        return None;
    }
    boot_pool(size).await
}

/// Resolve `TALIESIN_PYTHON` and boot a warm pool of `want` kernels, or `None` when
/// the interpreter isn't configured (so we never speculatively boot a forkserver
/// against a possibly-absent `python3`; the caller then cold-starts as before). A
/// boot failure isn't `None` here — `WarmPool::new` already degrades to an inert pool
/// that returns `None` from `take`, which the executor treats as a cold start.
async fn boot_pool(want: usize) -> Option<Arc<WarmPool>> {
    let python = PathBuf::from(std::env::var_os("TALIESIN_PYTHON")?);
    Some(Arc::new(WarmPool::new(&python, want).await))
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
                // the `ready + in_flight <= cap` invariant.
                {
                    let ready = inner.ready.lock().await;
                    let mut n = inner.in_flight.lock().await;
                    match try_reserve_slot(ready.len(), *n, inner.cap) {
                        Some(next) => *n = next,
                        None => return,
                    }
                }
                match Self::warm_one(&inner.python, &daemon).await {
                    Ok(kernel) => {
                        let mut ready = inner.ready.lock().await;
                        ready.push_back(kernel);
                        // Kernel is now in `ready`; release the in-flight slot so
                        // `ready.len() + *in_flight` stays accurate.
                        let mut n = inner.in_flight.lock().await;
                        *n = n.saturating_sub(1);
                    }
                    Err(e) => {
                        // Give the slot back and stop refilling for now; the next
                        // `take` falls back to a cold start.
                        let mut n = inner.in_flight.lock().await;
                        *n = n.saturating_sub(1);
                        crate::log::warn(&format!(
                            "warm-pool: pre-warm failed ({e}); kernel will cold-start on demand"
                        ));
                        return;
                    }
                }
            }
        });
    }

    /// Fork one warm kernel and complete its ZMQ handshake + preambles. The
    /// connection file is created by taliesin (same locked-down 0600/0700 path as
    /// `Kernel::start`), the daemon forks an in-process ipykernel bound to it, and
    /// `Kernel::adopt_forked` connects.
    async fn warm_one(
        python: &std::path::Path,
        daemon: &Arc<ForkserverDaemon>,
    ) -> std::io::Result<Kernel> {
        let spec = KernelSpec::python(python);
        let (info, conn_dir, conn_file) = prepare_connection(spec.kernel_name()).await?;
        let pid = daemon.fork_kernel(&conn_file).await?;
        Kernel::adopt_forked(pid, info, conn_dir, Arc::clone(daemon), &spec).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::render_outputs;

    fn python() -> Option<PathBuf> {
        std::env::var_os("TALIESIN_PYTHON").map(PathBuf::from)
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
            // The fallback the caller uses (Kernel::start with a real python) still
            // produces a working kernel — behaviour is no worse than before.
            let mut k = Kernel::start(&KernelSpec::python(&py), None)
                .await
                .expect("cold fallback kernel should start");
            let html = render_outputs(&k.execute("print(1 + 1)").await.unwrap());
            assert!(
                html.contains('2'),
                "fallback kernel did not return 2: {html}"
            );
        });
    }
}
