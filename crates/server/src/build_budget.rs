//! Memory-aware build concurrency cap.
//!
//! `concurrency_cap` decides how many parallel kernel processes to allow,
//! respecting an explicit `--jobs` override and capping against available memory
//! and CPU core count when running in auto mode.

/// Pure inner function for testing.
///
/// Semantics:
/// - `jobs == Some(1)`      → 1 (sequential)
/// - `jobs == Some(n > 1)`  → n (explicit)
/// - `jobs == None | Some(0)` → `min(cores, max(1, free_mb / per_kernel_mb))`
pub(crate) fn concurrency_cap_with(
    jobs: Option<usize>,
    per_kernel_mb: u64,
    cores: usize,
    free_mb: u64,
) -> usize {
    match jobs {
        Some(0) | None => {
            // `per_kernel_mb == 0` means memory isn't the limiter (checked_div returns
            // None on a zero divisor), so fall back to the core count.
            let mem_slots = match free_mb.checked_div(per_kernel_mb) {
                Some(slots) => slots as usize,
                None => cores,
            };
            cores.min(mem_slots.max(1))
        }
        Some(n) => n.max(1),
    }
}

/// Probe free memory from `/proc/meminfo` (Linux).
/// Returns `MemAvailable` in MiB, or `None` if the file cannot be parsed.
#[cfg(target_os = "linux")]
fn probe_free_mb() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if line.starts_with("MemAvailable:") {
            // Format: "MemAvailable:   12345678 kB"
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

/// Conservative fallback free-memory estimate (2 GiB) for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
fn probe_free_mb() -> Option<u64> {
    None
}

/// Conservative fallback when `/proc/meminfo` is unavailable (2 GiB).
const FALLBACK_FREE_MB: u64 = 2_048;

/// Estimated peak RSS of one warm kernel (Python/R), in MiB. Used by the memory-aware
/// concurrency cap (and the warm-pool budget split) so a build doesn't keep more
/// resident kernels than free RAM can hold. Conservative on the high side; pages with
/// no code cells never boot a kernel.
pub(crate) const PER_KERNEL_MB: u64 = 150;

/// Public API: returns the number of parallel build jobs to run.
///
/// `jobs` mirrors `--jobs` CLI flag (`None` / `Some(0)` = auto).
/// `per_kernel_mb` is the estimated peak RSS per kernel process in MiB.
pub(crate) fn concurrency_cap(jobs: Option<usize>, per_kernel_mb: u64) -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let free_mb = probe_free_mb().unwrap_or(FALLBACK_FREE_MB);
    concurrency_cap_with(jobs, per_kernel_mb, cores, free_mb)
}

/// Preferred warm-pool size: how many kernels we'd *like* to keep pre-warmed so the
/// first edit is near-instant. Kept small (`2`) — enough to cover the page the user
/// is editing plus one ahead — so it speeds first-edit without crowding out the
/// parallel build. Always reconciled against the live `cap` by [`budget_split`].
pub(crate) const WARM_POOL_TARGET: usize = 2;

/// How the resident-kernel budget (`cap`, from [`concurrency_cap`]) splits between
/// the eager warm pool and the concurrent build. Both kinds of kernel are real
/// resident interpreters costing ~`per_kernel_mb` each, so they must share **one**
/// budget: `warm_pool + build_kernels <= cap`. This pure helper makes that split.
///
/// Policy (conservative, build-first):
///   - `build_kernels` is never below 1, so a tiny budget still builds (it just
///     doesn't pre-warm) — the warm pool is an accelerator, never a gate.
///   - `warm_pool = min(WARM_POOL_TARGET, cap - 1)`, i.e. the warm pool only claims
///     a slot once the build is guaranteed at least one, and never more than the
///     target. So `cap == 1` → no warm pool (all RAM goes to the single build
///     kernel); `cap == 2` → 1 warm + 1 build; `cap >= 3` → 2 warm + the rest build.
///   - With `cap == 0` (shouldn't happen — callers `.max(1)`) both are 0.
///
/// The invariant `warm_pool + build_kernels <= cap` holds for every `cap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BudgetSplit {
    /// Kernels to keep pre-warmed in the [`crate::warm_pool::WarmPool`].
    pub warm_pool: usize,
    /// Kernels the concurrent build may run at once (the semaphore cap).
    pub build_kernels: usize,
}

/// Split a resident-kernel `cap` into a warm-pool size + a build-concurrency size
/// that together fit the budget. See [`BudgetSplit`] for the policy.
pub(crate) fn budget_split(cap: usize) -> BudgetSplit {
    if cap == 0 {
        return BudgetSplit {
            warm_pool: 0,
            build_kernels: 0,
        };
    }
    // Build is guaranteed at least one slot; the warm pool may take up to
    // WARM_POOL_TARGET of whatever remains above that floor.
    let warm_pool = WARM_POOL_TARGET.min(cap - 1);
    let build_kernels = cap - warm_pool;
    BudgetSplit {
        warm_pool,
        build_kernels,
    }
}

/// How many kernels the **preview** server should pre-warm, reconciled with the same
/// memory budget the parallel build uses. The preview builder runs pages serially
/// (one build kernel at a time), so the warm-pool slot count is just the
/// [`budget_split`] of the auto memory/core cap — i.e. up to [`WARM_POOL_TARGET`],
/// less on a memory-starved machine. `WarmPool::new` clamps it again to its own
/// `POOL_CAP`, so this never over-commits RAM.
pub(crate) fn preview_warm_pool_size() -> usize {
    let cap = concurrency_cap(None, PER_KERNEL_MB).max(1);
    budget_split(cap).warm_pool
}

#[cfg(test)]
mod tests {
    use super::{WARM_POOL_TARGET, budget_split, concurrency_cap_with};

    #[test]
    fn explicit_jobs_one_is_sequential() {
        assert_eq!(concurrency_cap_with(Some(1), 150, 16, 32_000), 1);
    }

    #[test]
    fn explicit_jobs_is_respected() {
        assert_eq!(concurrency_cap_with(Some(4), 150, 16, 32_000), 4);
    }

    #[test]
    fn auto_caps_by_memory() {
        // free_mb / per_kernel_mb = 4000 / 1000 = 4, cores = 16 → min(16, 4) = 4
        assert_eq!(concurrency_cap_with(None, 1000, 16, 4_000), 4);
    }

    #[test]
    fn auto_caps_by_cores() {
        // free_mb / per_kernel_mb = 64000 / 100 = 640, cores = 8 → min(8, 640) = 8
        assert_eq!(concurrency_cap_with(None, 100, 8, 64_000), 8);
    }

    #[test]
    fn auto_never_zero() {
        // free_mb / per_kernel_mb = 10 / 100000 = 0, floor to 1
        assert_eq!(concurrency_cap_with(None, 100_000, 16, 10), 1);
    }

    #[test]
    fn some_zero_is_auto() {
        // Some(0) treated as auto: same as None
        assert_eq!(
            concurrency_cap_with(Some(0), 1000, 16, 4_000),
            concurrency_cap_with(None, 1000, 16, 4_000)
        );
    }

    #[test]
    fn split_zero_budget_is_empty() {
        let s = budget_split(0);
        assert_eq!(s.warm_pool, 0);
        assert_eq!(s.build_kernels, 0);
    }

    #[test]
    fn split_cap_one_keeps_no_warm_pool() {
        // The single slot must go to the build; a warm pool here would starve it.
        let s = budget_split(1);
        assert_eq!(s.warm_pool, 0);
        assert_eq!(s.build_kernels, 1);
    }

    #[test]
    fn split_cap_two_is_one_warm_one_build() {
        let s = budget_split(2);
        assert_eq!(s.warm_pool, 1);
        assert_eq!(s.build_kernels, 1);
    }

    #[test]
    fn split_large_cap_caps_warm_at_target_rest_builds() {
        // cap 8 → 2 warm (the target), 6 build.
        let s = budget_split(8);
        assert_eq!(s.warm_pool, WARM_POOL_TARGET);
        assert_eq!(s.build_kernels, 8 - WARM_POOL_TARGET);
    }

    #[test]
    fn split_never_exceeds_budget_and_build_is_at_least_one() {
        // The core invariant across the whole sensible range: warm + build <= cap,
        // build >= 1 for every nonzero cap, and warm is never above the target.
        for cap in 1..=64 {
            let s = budget_split(cap);
            assert!(
                s.warm_pool + s.build_kernels <= cap,
                "split for cap {cap} exceeds budget: {s:?}"
            );
            assert!(s.build_kernels >= 1, "build starved at cap {cap}: {s:?}");
            assert!(
                s.warm_pool <= WARM_POOL_TARGET,
                "warm over target at cap {cap}"
            );
        }
    }
}
