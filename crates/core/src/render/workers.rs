//! The threads every render runs on: long-lived, each with the big stack a render needs, parked
//! between renders instead of spawned per render.
//!
//! [`super::render_internal`] runs each render off the caller's thread for two reasons it
//! documents (the stack, and a watchdog that can abandon a quadratic render). Until 2026-09-24
//! that thread was spawned afresh for every render and exited after it. A whole-project pass
//! renders every page, so a save of a 500-page book spawned 518 threads: glibc does not cache a
//! 256 MB stack, so each one paid an `mmap`, an `mprotect` and a `munmap`, the pass spent more
//! samples creating and tearing down threads than rendering, and the churn spread allocations
//! over fresh malloc arenas, so RSS climbed from 87 to 272 MB over 150 heading edits and stayed
//! there (audit 2026-09-24, F4).
//!
//! A worker that finishes a render parks itself here and takes the next one. An abandoned
//! render (the watchdog gave up on it) simply keeps its worker until it finishes, and the next
//! render takes another. At most [`Workers::max_idle`] workers stay parked; one finishing past
//! that exits, so the pool never holds more idle threads than the machine has cores.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{LazyLock, Mutex};

/// The stack every render runs on. Deeply nested input recurses in the parser and in block
/// emission; see [`super::render_internal`] and [`super::MAX_NESTING_DEPTH`].
const STACK_BYTES: usize = 256 * 1024 * 1024;

type Job = Box<dyn FnOnce() + Send>;

/// Parked workers, each waiting on its own channel for the next job.
pub(super) struct Workers {
    idle: Mutex<Vec<Sender<Job>>>,
    max_idle: usize,
}

/// The workers renders run on.
pub(super) static RENDER: LazyLock<Workers> =
    LazyLock::new(|| Workers::new(std::thread::available_parallelism().map_or(1, |n| n.get())));

/// Workers spawned so far, for the tests that pin reuse. Test-only.
#[cfg(test)]
pub(super) static SPAWNED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl Workers {
    pub(super) fn new(max_idle: usize) -> Self {
        Self {
            idle: Mutex::new(Vec::new()),
            max_idle,
        }
    }

    /// Run `job` on a parked worker, or on a new one when none is parked. `false` when no
    /// worker could be spawned (a strict `ulimit -v` refuses a 256 MB stack); the job is then
    /// dropped unrun and the caller renders on its own thread.
    pub(super) fn run(&'static self, mut job: Job) -> bool {
        loop {
            let parked = self.idle.lock().unwrap_or_else(|e| e.into_inner()).pop();
            let Some(worker) = parked else { break };
            match worker.send(job) {
                Ok(()) => return true,
                // A worker never exits while parked, so this is unreachable in practice; try
                // the next one rather than lose the job.
                Err(back) => job = back.0,
            }
        }
        let (me, jobs) = channel::<Job>();
        let _ = me.send(job);
        #[cfg(test)]
        SPAWNED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::thread::Builder::new()
            .name("taliesin-render".to_string())
            .stack_size(STACK_BYTES)
            .spawn(move || self.work(me, jobs))
            .is_ok()
    }

    /// A worker's life: run a job, park, repeat, until the pool already holds enough idle
    /// workers. A job must not unwind (the render's catches its own panic), but one that did
    /// would only end this thread: its sender is not parked while it runs.
    fn work(&self, me: Sender<Job>, jobs: Receiver<Job>) {
        while let Ok(job) = jobs.recv() {
            job();
            let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
            if idle.len() >= self.max_idle {
                return;
            }
            idle.push(me.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::mpsc::sync_channel;
    use std::thread::ThreadId;
    use std::time::Duration;

    fn private(max_idle: usize) -> &'static Workers {
        Box::leak(Box::new(Workers::new(max_idle)))
    }

    /// Run a job on `pool` and wait for it, returning the thread it ran on.
    fn run_and_wait(pool: &'static Workers) -> ThreadId {
        let (tx, rx) = sync_channel(1);
        assert!(pool.run(Box::new(move || {
            let _ = tx.send(std::thread::current().id());
        })));
        rx.recv().expect("the job ran")
    }

    #[test]
    fn sequential_jobs_share_one_worker() {
        let pool = private(4);
        let threads: HashSet<ThreadId> = (0..20).map(|_| run_and_wait(pool)).collect();
        // A job can hand its reply back a moment before its worker parks, so the next job
        // may find none parked and spawn a second. Twenty spawns is a thread per job.
        assert!(threads.len() <= 2, "{} workers for 20 jobs", threads.len());
        assert!(!threads.contains(&std::thread::current().id()));
    }

    /// The watchdog abandons a render by no longer waiting for it; the worker it holds must
    /// not stall the renders after it.
    #[test]
    fn a_busy_worker_does_not_block_the_next_job() {
        let pool = private(4);
        let (release, held) = channel::<()>();
        let (started_tx, started) = sync_channel(1);
        assert!(pool.run(Box::new(move || {
            let _ = started_tx.send(());
            let _ = held.recv();
        })));
        started.recv().unwrap();
        let (tx, rx) = sync_channel(1);
        assert!(pool.run(Box::new(move || {
            let _ = tx.send(());
        })));
        assert!(
            rx.recv_timeout(Duration::from_secs(10)).is_ok(),
            "the second job waited behind the first"
        );
        drop(release);
    }

    /// Parked threads are bounded: a burst of concurrent jobs spawns one worker each, and only
    /// `max_idle` of them stay.
    #[test]
    fn at_most_max_idle_workers_stay_parked() {
        let pool = private(2);
        let gate = std::sync::Arc::new(std::sync::Barrier::new(6));
        let (done_tx, done) = channel();
        for _ in 0..5 {
            let (gate, done_tx) = (gate.clone(), done_tx.clone());
            assert!(pool.run(Box::new(move || {
                gate.wait();
                let _ = done_tx.send(());
            })));
        }
        gate.wait();
        for _ in 0..5 {
            done.recv().unwrap();
        }
        // The last worker parks a moment after its reply; wait for the pool to settle.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while pool.idle.lock().unwrap().len() < 2 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(pool.idle.lock().unwrap().len(), 2);
    }
}
