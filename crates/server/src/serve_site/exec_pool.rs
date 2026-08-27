//! Per-page executor cache: a small bounded LRU of warm `Executor`s (one per page) so
//! revisiting a page reuses its warm kernel + freeze state instead of cold-starting,
//! while capping resident kernels at `MAX_WARM_PAGES`. The eviction order must stay
//! deterministic (the build relies on it); this is a verbatim relocation of that logic.
//! `use super::*` reaches Executor / the std types from serve_site/mod.rs.

use super::*;

/// How many pages keep a warm kernel at once. Each page's executor holds its own
/// Python kernel (~80-150 MB each), so an unbounded map would grow a kernel per
/// page visited and never reclaim it. We keep the most-recently-built pages warm
/// and drop the rest's kernels; an evicted page just pays a cold kernel start on
/// its next edit.
const MAX_WARM_PAGES: usize = 6;

/// What an eviction says on the kernel log channel, or `None` when nothing worth
/// reporting died.
///
/// An `Executor` boots no kernel until a cell actually runs, and an unbuilt page routes
/// to the exec lane by default — so the unconditional line claimed a kernel death on
/// every eviction, including on documents that contain no code at all. Previewing
/// `corpus/tarn` (14 chapters, **zero** code cells) was enough to produce it: browse past
/// six chapters and the console announces warm kernels being evicted that were never
/// booted, on a channel whose whole purpose is to report kernel lifecycle.
///
/// A pure function of the decision, not of the I/O, so the choice is testable without
/// capturing stderr — `crate::log::kernel` writes straight to it.
fn eviction_line(page: &str, had_kernel: bool) -> Option<String> {
    had_kernel.then(|| format!("evicted warm kernel for {page}"))
}

/// The per-page executors, bounded to [`MAX_WARM_PAGES`] by least-recently-built.
/// Dropping an executor (on eviction) kills its kernel child processes.
#[derive(Default)]
pub(super) struct ExecPool {
    execs: HashMap<String, crate::exec::Executor>,
    /// Page rel-paths, most-recently-built first; kept in sync with `execs`' keys.
    mru: Vec<String>,
    /// `_freeze/` directory for the project; each page's executor caches its outputs
    /// under it. Empty (the `Default`) disables caching — used by the unit tests.
    freeze_dir: PathBuf,
    /// The resolved Python interpreter (from `_site.yml` python: / .venv / env /
    /// default), applied to every page executor so the pool and the executors agree on
    /// which interpreter runs. `None` (the unit-test `Default`) leaves each executor on
    /// the env/default that `Executor::build` computes, i.e. no override.
    python: Option<crate::interpreter::Resolved>,
    /// Shared with [`super::SiteApp::interrupt`]: the pid of the cell currently executing
    /// anywhere in this pool, or 0. Handed to every executor this pool makes, so the
    /// websocket task can SIGINT a running cell without waiting for the serial builder to
    /// come back to it. `None` (the unit-test `Default`) publishes nothing.
    interrupt: Option<Arc<std::sync::atomic::AtomicU32>>,
}

impl ExecPool {
    /// A pool whose executors persist their outputs under `freeze_dir`, publishing the
    /// running cell's pid on `interrupt`.
    pub(super) fn new(
        freeze_dir: PathBuf,
        python: crate::interpreter::Resolved,
        interrupt: Arc<std::sync::atomic::AtomicU32>,
    ) -> Self {
        ExecPool {
            freeze_dir,
            python: Some(python),
            interrupt: Some(interrupt),
            ..Default::default()
        }
    }

    /// A fresh executor for `rel`, cache-backed when the pool has a `_freeze/` dir,
    /// running its kernels in `work_dir` (the page's own directory).
    pub(super) fn make(&self, rel: &str, work_dir: &Path) -> crate::exec::Executor {
        let ex = if self.freeze_dir.as_os_str().is_empty() {
            crate::exec::Executor::new()
        } else {
            crate::exec::Executor::with_freeze(crate::freeze::page_path(&self.freeze_dir, rel))
        };
        let mut ex = ex.in_dir(work_dir);
        if let Some(py) = &self.python {
            ex.set_interpreters(py.clone());
        }
        if let Some(h) = &self.interrupt {
            ex.set_interrupt_handle(h.clone());
        }
        ex
    }

    /// The executor for `rel` (created if absent), marked most-recently-used. If
    /// that pushes the live set past the cap, the least-recently-built page's
    /// executor is dropped (killing its kernels). `work_dir` is the page's own
    /// directory, used only when the executor is first created.
    pub(super) fn get(&mut self, rel: &str, work_dir: &Path) -> &mut crate::exec::Executor {
        self.mru.retain(|r| r != rel);
        self.mru.insert(0, rel.to_string());
        if !self.execs.contains_key(rel) {
            let ex = self.make(rel, work_dir);
            self.execs.insert(rel.to_string(), ex);
        }
        while self.mru.len() > MAX_WARM_PAGES {
            if let Some(evicted) = self.mru.pop() {
                // The eviction ORDER and the cap are the standing freeze; only what gets
                // SAID about an eviction changes here.
                let had_kernel = self
                    .execs
                    .remove(&evicted) // drops the executor -> kills its kernels
                    .is_some_and(|ex| ex.has_live_kernel());
                if let Some(msg) = eviction_line(&evicted, had_kernel) {
                    crate::log::kernel(&msg);
                }
            }
        }
        // Present: just inserted, and `rel` is at the MRU front so it's never the
        // one evicted (cap >= 1).
        self.execs.get_mut(rel).unwrap()
    }

    /// Restart `rel`'s kernel if it currently has one (the dev-menu action).
    pub(super) fn restart(&mut self, rel: &str) {
        if let Some(ex) = self.execs.get_mut(rel) {
            ex.restart_kernel();
        }
    }
}

#[cfg(test)]
mod tests {
    //! The pool bounds how many pages keep a warm kernel, so a long browse of a
    //! big site doesn't leak a kernel per page. `Executor::new()` doesn't spawn a
    //! kernel (that's lazy), so this exercises the eviction logic kernel-free.
    use super::*;

    #[test]
    fn evicts_least_recently_built_beyond_cap() {
        let mut pool = ExecPool::default();
        for i in 0..MAX_WARM_PAGES + 3 {
            pool.get(&format!("p{i}"), Path::new("."));
        }
        assert_eq!(pool.execs.len(), MAX_WARM_PAGES, "live set must be capped");
        assert_eq!(
            pool.mru.len(),
            MAX_WARM_PAGES,
            "mru stays in sync with execs"
        );
        assert!(!pool.execs.contains_key("p0"), "oldest page was evicted");
        assert!(
            pool.execs.contains_key(&format!("p{}", MAX_WARM_PAGES + 2)),
            "newest page is warm"
        );
    }

    #[test]
    fn touching_a_page_keeps_it_warm() {
        let mut pool = ExecPool::default();
        for i in 0..MAX_WARM_PAGES {
            pool.get(&format!("p{i}"), Path::new("."));
        }
        // Re-build the oldest page: it becomes most-recent and must survive the
        // next eviction instead of being dropped.
        pool.get("p0", Path::new("."));
        pool.get("newer", Path::new("."));
        assert!(pool.execs.contains_key("p0"), "re-touched page survived");
        assert!(
            !pool.execs.contains_key("p1"),
            "the now-oldest page was evicted"
        );
        assert_eq!(pool.execs.len(), MAX_WARM_PAGES);
    }

    #[test]
    fn an_eviction_announces_a_kernel_only_when_one_was_actually_booted() {
        // Item 114. The log line was unconditional, so evicting a page that had never
        // run a cell still reported "evicted warm kernel for …" — false on any
        // prose-only project, and on a channel that exists to report kernel lifecycle.
        assert_eq!(
            eviction_line("ch7.tmd", false),
            None,
            "a page that never booted a kernel must say nothing"
        );
        assert_eq!(
            eviction_line("ch7.tmd", true).as_deref(),
            Some("evicted warm kernel for ch7.tmd"),
            "and a real kernel death keeps the line it always had"
        );

        // The other half of the claim: the boolean fed in above is read off the
        // executor, and a pool executor that has run nothing owns no kernel. This is
        // what makes the whole browse of a code-free site silent, which is the observed
        // symptom (`corpus/tarn`, 14 chapters, zero code cells).
        let mut pool = ExecPool::default();
        assert!(
            !pool.get("prose.tmd", Path::new(".")).has_live_kernel(),
            "a freshly-made executor has booted no kernel"
        );
    }
}
