//! Per-page executor cache: a small bounded LRU of warm `Executor`s (one per page) so
//! revisiting a page reuses its warm kernel + freeze state instead of cold-starting,
//! while capping resident kernels at `MAX_WARM_PAGES`. The eviction order must stay
//! deterministic (the build relies on it); this is a verbatim relocation of that logic.
//! `use super::*` reaches Executor / WarmPool / the std types from serve_site/mod.rs.

use super::*;

/// How many pages keep a warm kernel at once. Each page's executor holds its own
/// Python/R kernel (~80-150 MB each), so an unbounded map would grow a kernel per
/// page visited and never reclaim it. We keep the most-recently-built pages warm
/// and drop the rest's kernels; an evicted page just pays a cold kernel start on
/// its next edit.
const MAX_WARM_PAGES: usize = 6;

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
    /// The one process-wide warm pool of pre-booted Python kernels, shared by every
    /// page executor so the first edit on a fresh page is near-instant instead of
    /// paying a cold boot. `None` (the `Default`, and when `TALIESIN_PYTHON` is unset
    /// / the forkserver can't boot) → every page cold-starts, exactly as before.
    warm_pool: Option<Arc<crate::warm_pool::WarmPool>>,
}

impl ExecPool {
    /// A pool whose executors persist their outputs under `freeze_dir` and draw their
    /// Python kernels from the shared `warm_pool` (when one booted).
    pub(super) fn new(
        freeze_dir: PathBuf,
        warm_pool: Option<Arc<crate::warm_pool::WarmPool>>,
    ) -> Self {
        ExecPool {
            freeze_dir,
            warm_pool,
            ..Default::default()
        }
    }

    /// A fresh executor for `rel`, cache-backed when the pool has a `_freeze/` dir,
    /// running its kernels in `work_dir` (the page's own directory), drawing Python
    /// kernels from the shared warm pool when one is wired.
    pub(super) fn make(&self, rel: &str, work_dir: &Path) -> crate::exec::Executor {
        let ex = if self.freeze_dir.as_os_str().is_empty() {
            crate::exec::Executor::new()
        } else {
            crate::exec::Executor::with_freeze(crate::freeze::page_path(&self.freeze_dir, rel))
        };
        let mut ex = ex.in_dir(work_dir);
        ex.set_warm_pool(self.warm_pool.clone());
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
                self.execs.remove(&evicted); // drops the executor -> kills its kernels
                crate::log::kernel(&format!("evicted warm kernel for {evicted}"));
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
}
