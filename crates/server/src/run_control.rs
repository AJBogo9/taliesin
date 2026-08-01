//! Stopping a run that is already executing: the shared handle an interrupt request uses to
//! reach a run it cannot borrow.
//!
//! # Why a side channel exists at all
//!
//! [`crate::exec::Executor`] owns its kernels, and `execute_streaming` holds one mutably for
//! the whole of a cell. An interrupt arriving on another request therefore has no way to
//! obtain `&mut Kernel`, and restructuring that ownership so it could would be a large change
//! to the execution core for one feature. It does not need to: an interrupt is a `SIGINT` to a
//! PID, and a PID is a number that can be published before the borrow starts.
//!
//! # Why an interrupt is two things, not one
//!
//! Signalling the running cell's PID ends **that cell**. A run is a *sequence* of cells, so a
//! signal alone stops cell 4 of 10 and then calmly executes the remaining six — the opposite
//! of what Ctrl-C means. So [`RunControl::cancel`] does both: it signals the PID *and* raises
//! a flag the run loop checks between cells. Neither half is sufficient:
//!
//! - the flag without the signal leaves the current (possibly very long) cell running;
//! - the signal without the flag stops one cell out of ten.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The cell a run is executing right now, if any.
#[derive(Debug, Clone, Copy)]
struct Running {
    /// The kernel language, so an interrupt can report what it stopped.
    lang: &'static str,
    /// The kernel process to signal. `None` when the kernel has no PID (it never booted, or
    /// a platform without one), which is not an error: there is simply nothing to signal.
    pid: Option<u32>,
}

/// One page's run state: how many cancels have happened, and what is executing.
///
/// # Why a counter and not a boolean
///
/// A boolean was the obvious design and it is wrong, in a way only an end-to-end test finds.
/// Runs **queue**: `taliesin run` starts a session, that session immediately does its own
/// execution pass, and the client's run waits behind it. So Ctrl-C during the first pass has
/// to stop a run that is executing *and* a run that has been asked for but has not started.
/// A boolean cleared at the start of each run stops only the first: the queued run then began
/// with a clean flag and executed the rest of the document, which is exactly the "the signal
/// stopped one cell, not the run" failure one level up.
///
/// So a run carries the epoch it was **requested** at, and stops the moment the live epoch
/// differs. A cancel invalidates every run already in flight or queued, and nothing else: a
/// run requested afterwards reads the new epoch and is unaffected.
#[derive(Default)]
pub(crate) struct RunControl {
    epoch: AtomicU64,
    running: Mutex<Option<Running>>,
}

impl RunControl {
    /// The current epoch. A caller records this when a run is **requested** and hands it back
    /// to the executor, which stops as soon as the two differ.
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }

    /// Publish the cell about to execute, so an interrupt can find its kernel.
    pub(crate) fn begin_cell(&self, lang: &'static str, pid: Option<u32>) {
        *self.running.lock().unwrap() = Some(Running { lang, pid });
    }

    /// The cell finished. Clears the published PID so a later interrupt cannot signal a
    /// kernel that is now idle, which would raise `KeyboardInterrupt` in whatever the author
    /// runs next.
    pub(crate) fn end_cell(&self) {
        *self.running.lock().unwrap() = None;
    }

    /// The language of the cell executing right now, if any.
    ///
    /// Only an observer, and `cfg(test)` because that is its only caller: it lets a test wait
    /// until a run is genuinely mid-cell instead of sleeping a guessed interval, which on a
    /// cold kernel boot is the difference between testing the interrupt and testing nothing.
    /// (A future `run --status` would want this in production; it can drop the gate then.)
    #[cfg(test)]
    pub(crate) fn running_lang(&self) -> Option<&'static str> {
        self.running.lock().unwrap().map(|r| r.lang)
    }

    /// Request a stop: raise the flag and signal whatever is executing.
    ///
    /// Returns the language of the cell that was interrupted, or `None` when nothing was
    /// running. `None` is a normal answer, not a failure: interrupting an idle page is what
    /// happens when the author is a second late, and it must be a quiet no-op.
    ///
    /// The epoch is bumped **even when nothing is running**, deliberately, and that covers
    /// two cases at once: a run sitting between cells has published no PID but is very much
    /// alive, and a run that has been requested but not started has no PID either.
    pub(crate) fn cancel(&self) -> Option<&'static str> {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        let running = *self.running.lock().unwrap();
        let running = running?;
        if let Some(pid) = running.pid {
            crate::kernel::interrupt_pid(pid);
        }
        Some(running.lang)
    }
}

/// Every page's [`RunControl`], so a request handler can reach a run owned by the build task.
///
/// Keyed by page key: the site rel for a project, the document path for a single doc. Cheap
/// to clone (an `Arc` inside), because both servers hand a clone to the exec path and keep
/// one for the interrupt endpoint.
#[derive(Default, Clone)]
pub(crate) struct RunRegistry(Arc<Mutex<HashMap<String, Arc<RunControl>>>>);

impl RunRegistry {
    /// `page`'s control, creating it if this is the first run for that page.
    pub(crate) fn control(&self, page: &str) -> Arc<RunControl> {
        self.0
            .lock()
            .unwrap()
            .entry(page.to_string())
            .or_default()
            .clone()
    }

    /// `page`'s control if one exists, without creating one.
    ///
    /// The interrupt endpoint uses this: a page that has never run has nothing to interrupt,
    /// and creating an entry to answer "no" would grow the map on request — the same
    /// unbounded-growth-by-request shape the site server's unknown-`?page=` key was fixed for.
    pub(crate) fn existing(&self, page: &str) -> Option<Arc<RunControl>> {
        self.0.lock().unwrap().get(page).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupting_an_idle_page_is_a_quiet_no_op() {
        let c = RunControl::default();
        assert_eq!(
            c.cancel(),
            None,
            "nothing was running, so nothing was named"
        );
    }

    #[test]
    fn a_cancel_between_cells_still_stops_the_run() {
        // A run sitting between cells has published no PID, so `cancel` has nothing to
        // signal, but it is very much alive and about to start cell 5. If the epoch only
        // moved when a PID was present, Ctrl-C in that window would be swallowed and every
        // remaining cell would execute.
        let c = RunControl::default();
        let run = c.epoch();
        c.begin_cell("python", Some(4711));
        c.end_cell();
        assert_eq!(c.cancel(), None, "nothing running at this instant");
        assert_ne!(c.epoch(), run, "the run must still be invalidated");
    }

    #[test]
    fn a_cancel_names_the_language_it_stopped() {
        let c = RunControl::default();
        // pid `None`: a kernel that never booted has nothing to signal, and that must not
        // stop the cancel from being recorded and reported.
        c.begin_cell("r", None);
        assert_eq!(c.cancel(), Some("r"));
    }

    #[test]
    fn a_cancel_also_invalidates_a_run_that_was_only_queued() {
        // The bug an end-to-end run found, which a boolean flag could not express.
        //
        // `taliesin run` starts a session; that session immediately does its own execution
        // pass; the client's run queues behind it. Ctrl-C during the first pass must stop
        // BOTH. With a boolean cleared at each run's start, the queued run began clean and
        // executed the rest of the document, i.e. the stopped run's own downstream cells,
        // moments after the author asked for everything to stop.
        let c = RunControl::default();
        let queued = c.epoch(); // the client's run, requested before the cancel
        c.begin_cell("python", Some(4711)); // the session's own pass, in flight
        assert_eq!(c.cancel(), Some("python"));
        c.end_cell();
        assert_ne!(
            c.epoch(),
            queued,
            "a run requested before the cancel must not survive it"
        );
    }

    #[test]
    fn a_run_requested_after_a_cancel_is_unaffected() {
        // The other half, and the reason this is an epoch rather than a latch: the author
        // hits Ctrl-C, then immediately runs again. That second run must execute normally.
        let c = RunControl::default();
        c.cancel();
        let fresh = c.epoch();
        c.begin_cell("python", Some(4711));
        c.end_cell();
        assert_eq!(
            c.epoch(),
            fresh,
            "nothing has invalidated a run requested after the cancel"
        );
    }

    #[test]
    fn ending_a_cell_stops_a_later_interrupt_from_signalling_an_idle_kernel() {
        // Without `end_cell` clearing the PID, an interrupt arriving after the run finished
        // would SIGINT a kernel sitting idle, raising KeyboardInterrupt inside whatever the
        // author ran next.
        let c = RunControl::default();
        c.begin_cell("python", Some(4711));
        c.end_cell();
        assert_eq!(
            c.cancel(),
            None,
            "must not name (or signal) a finished cell"
        );
    }

    #[test]
    fn the_registry_does_not_grow_on_a_lookup() {
        let reg = RunRegistry::default();
        assert!(reg.existing("never-run.tmd").is_none());
        assert_eq!(reg.0.lock().unwrap().len(), 0, "a lookup must not insert");

        let a = reg.control("page.tmd");
        assert_eq!(reg.0.lock().unwrap().len(), 1);
        // Same page, same control: the exec path and the endpoint must share one flag, or
        // the endpoint raises a cancel on an object the run never reads.
        assert!(Arc::ptr_eq(&a, &reg.control("page.tmd")));
        assert!(Arc::ptr_eq(&a, &reg.existing("page.tmd").unwrap()));
    }
}
