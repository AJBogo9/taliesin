//! Build concurrency planning: how many pages to render at once, and how many kernels
//! to keep pre-warmed alongside them.
//!
//! [`build_plan`] is the entry point, and the split it makes turns on **who chose the
//! number**:
//!
//! - **Explicit `--jobs N`** is the user's stated *page* concurrency (the CLI says "max
//!   parallel pages", in those words, and that is the only wording a user reads). It is
//!   honored exactly, and the warm pool is **additive** on top of it. Memory is not
//!   consulted, consistent with [`concurrency_cap_with`]'s own `Some(n)` arm, which has
//!   never looked at `free_mb`: an explicit `--jobs 8` already outruns what RAM can hold,
//!   so docking 2 slots off it for the pool was a token gesture that bought no safety and
//!   cost the flag its meaning.
//! - **Auto** (`None` / `Some(0)`) is ours to spend, so the memory budget is real: the
//!   cap comes from [`concurrency_cap`] (memory- and core-aware) and [`budget_split`]
//!   shares it, keeping `warm_pool + build_kernels <= cap`.
//!
//! Note a page render is often not a kernel at all — a prose page boots none — which is
//! why "the resident-kernel budget" was never a coherent meaning for `--jobs`, and how
//! the M1 defect hid for so long.

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

/// `MemAvailable` in MiB from `/proc/meminfo`'s contents. Pure, for testing.
fn mem_available_mb_from(meminfo: &str) -> Option<u64> {
    for line in meminfo.lines() {
        if line.starts_with("MemAvailable:") {
            // Format: "MemAvailable:   12345678 kB"
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

/// Headroom in MiB at ONE cgroup-v2 level, from its `memory.max` + `memory.current`.
/// `None` when that level sets no limit (`max`) or the pair does not parse.
///
/// Pure, for testing: the real files live under `/sys/fs/cgroup/<path>/`.
fn cgroup_level_free_mb(max: &str, current: &str) -> Option<u64> {
    // "max" is cgroup-v2 for "no limit at this level".
    let limit: u64 = max.trim().parse().ok()?;
    let used: u64 = current.trim().parse().ok()?;
    Some(limit.saturating_sub(used) / (1024 * 1024))
}

/// The tightest cgroup memory headroom that applies to us, in MiB, walking `self_cgroup`
/// (the `0::<path>` line of `/proc/self/cgroup`) from its own level up to the root.
///
/// **The walk is the point.** A limit may be set on any ANCESTOR, not just our own leaf:
/// Docker's `-m`, a Kubernetes pod limit and a systemd slice all commonly land above the
/// process's own cgroup. Reading only our level would still fail open in exactly the
/// containers this exists to survive. So every level is read and the MINIMUM wins.
///
/// `read` is injected so this is testable without a container (on the dev box every level
/// reports `max`, so the real filesystem cannot exercise it).
fn cgroup_free_mb_with<F>(self_cgroup: &str, read: F) -> Option<u64>
where
    F: Fn(&str) -> Option<String>,
{
    let mut best: Option<u64> = None;
    let mut path = self_cgroup.trim().to_string();
    loop {
        let base = format!("/sys/fs/cgroup{}", path.trim_end_matches('/'));
        if let (Some(max), Some(cur)) = (
            read(&format!("{base}/memory.max")),
            read(&format!("{base}/memory.current")),
        ) && let Some(free) = cgroup_level_free_mb(&max, &cur)
        {
            best = Some(best.map_or(free, |b: u64| b.min(free)));
        }
        if path == "/" || path.is_empty() {
            break;
        }
        path = match path.rfind('/') {
            Some(0) => "/".to_string(),
            Some(i) => path[..i].to_string(),
            None => break,
        };
    }
    best
}

/// The cgroup-v2 path for this process, from `/proc/self/cgroup`'s contents. Pure.
fn self_cgroup_from(proc_self_cgroup: &str) -> Option<String> {
    // v2 unified: a single `0::<path>` line. (v1's `N:controller:<path>` lines are not
    // handled; see `probe_free_mb`.)
    proc_self_cgroup
        .lines()
        .find_map(|l| l.strip_prefix("0::"))
        .map(|p| p.trim().to_string())
}

/// Free memory in MiB: what this process may actually use, not what the host has.
///
/// The host's `MemAvailable` alone **fails open in a container** — it reports the whole
/// machine while the cgroup is what the OOM killer enforces. That was wrong in one breath
/// with being right: `available_parallelism` (in [`concurrency_cap`]) already honours the
/// cgroup CPU quota, so the budget was CPU-correct and RAM-wrong at the same time. On a
/// 128 GB host with `--cpus 16 -m 2G`, the cap came out 16, i.e. 14 kernels x ~150 MB
/// against a 2 GB ceiling: the OOM killer, courtesy of the module written to prevent it.
///
/// So: the host figure, capped by the tightest cgroup limit that applies (see
/// [`cgroup_free_mb_with`] for why the ancestor walk matters). Both are best-effort; when
/// neither reads, the caller falls back to [`FALLBACK_FREE_MB`].
///
/// Only cgroup **v2** (the unified hierarchy) is read. v1 is not handled, so a v1 container
/// still fails open exactly as before; this is a strict improvement, not a complete one.
#[cfg(target_os = "linux")]
fn probe_free_mb() -> Option<u64> {
    let read = |p: &str| std::fs::read_to_string(p).ok();
    let host = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .as_deref()
        .and_then(mem_available_mb_from);
    let cgroup = std::fs::read_to_string("/proc/self/cgroup")
        .ok()
        .as_deref()
        .and_then(self_cgroup_from)
        .and_then(|p| cgroup_free_mb_with(&p, read));
    match (host, cgroup) {
        (Some(h), Some(c)) => Some(h.min(c)),
        (Some(h), None) => Some(h),
        (None, c) => c,
    }
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

/// A build's concurrency plan: how many pages to render at once, plus how many kernels
/// to pre-warm beside them. Produced by [`build_plan`]; the two fields share one memory
/// budget in auto mode and are independent under an explicit `--jobs` (see the module
/// header).
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BudgetSplit {
    /// Kernels to keep pre-warmed in the [`crate::warm_pool::WarmPool`].
    pub warm_pool: usize,
    /// Pages the build may render at once (the semaphore cap). Named for kernels
    /// because in auto mode it is charged as one: the memory budget must assume the
    /// worst case where every concurrent page boots one.
    pub build_kernels: usize,
    /// Whether `warm_pool` was funded out of the same budget as `build_kernels` (auto
    /// mode). Decides who owns pool slots that never boot — see [`Self::build_cap`].
    pub shares_budget: bool,
}

impl BudgetSplit {
    /// The build semaphore size, once the caller knows how many pool kernels **actually**
    /// booted (`warmed`; see `is_warm()` — a pool that declined or failed to boot holds no
    /// kernels and costs no RAM).
    ///
    /// This is M1's rule, scoped to where it applies. In **auto** mode the pool's slots
    /// came out of the build's own budget, so slots that never booted return to it: that
    /// is what makes a default build on a bare `python3` use the whole memory cap instead
    /// of silently forfeiting 2. Under an **explicit** `--jobs` there is nothing to
    /// return: the pool was additive and never held a build slot, so the user's N stands
    /// whether the pool booted or not.
    pub fn build_cap(self, warmed: usize) -> usize {
        if self.shares_budget {
            self.build_kernels + self.warm_pool.saturating_sub(warmed)
        } else {
            self.build_kernels
        }
    }
}

/// Split an **auto-mode** resident-kernel `cap` (from [`concurrency_cap`]) between the
/// eager warm pool and the concurrent build. Here the two genuinely share one budget:
/// both are real resident interpreters costing ~`per_kernel_mb` each and *we* picked the
/// number against free RAM, so `warm_pool + build_kernels <= cap` must hold.
///
/// Not for an explicit `--jobs` — that number is the user's page count, not a kernel
/// budget to divide, and [`build_plan`] gives it the additive treatment instead.
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
pub(crate) fn budget_split(cap: usize) -> BudgetSplit {
    if cap == 0 {
        return BudgetSplit {
            warm_pool: 0,
            build_kernels: 0,
            shares_budget: true,
        };
    }
    // Build is guaranteed at least one slot; the warm pool may take up to
    // WARM_POOL_TARGET of whatever remains above that floor.
    let warm_pool = WARM_POOL_TARGET.min(cap - 1);
    let build_kernels = cap - warm_pool;
    BudgetSplit {
        warm_pool,
        build_kernels,
        shares_budget: true,
    }
}

/// Pure inner function for testing (mirrors [`concurrency_cap_with`]'s style: the same
/// arithmetic with `cores`/`free_mb` injected instead of probed).
pub(crate) fn build_plan_with(
    jobs: Option<usize>,
    per_kernel_mb: u64,
    cores: usize,
    free_mb: u64,
) -> BudgetSplit {
    match jobs {
        // Explicit: N is the page count the user asked for, honored exactly. The pool is
        // additive — it is not funded out of the user's number.
        //
        // The pool stays at the full WARM_POOL_TARGET even for `--jobs 1`. It is tempting
        // to scale it down to `n`, but `budget_split`'s "cap 1 → no warm pool" rule exists
        // only because warm and build SHARE a budget there; under an explicit --jobs they
        // do not, so that rationale does not carry. The precedent is `preview_warm_pool_size`:
        // a preview builds pages strictly serially and still gets a 2-kernel pool, for the
        // reason WARM_POOL_TARGET's own doc gives — the page being worked on, plus one ahead.
        Some(n) if n >= 1 => BudgetSplit {
            warm_pool: WARM_POOL_TARGET,
            build_kernels: n,
            shares_budget: false,
        },
        // Auto (`None` / `Some(0)`): the cap is ours to spend, so the memory budget is
        // real and the shared-budget split applies unchanged.
        _ => budget_split(concurrency_cap_with(None, per_kernel_mb, cores, free_mb).max(1)),
    }
}

/// Public API: plan a site build's concurrency from the `--jobs` flag.
///
/// `jobs` mirrors the `--jobs` CLI flag (`None` / `Some(0)` = auto). See the module
/// header for why explicit and auto are planned differently.
pub(crate) fn build_plan(jobs: Option<usize>, per_kernel_mb: u64) -> BudgetSplit {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let free_mb = probe_free_mb().unwrap_or(FALLBACK_FREE_MB);
    build_plan_with(jobs, per_kernel_mb, cores, free_mb)
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
    use super::{
        PER_KERNEL_MB, WARM_POOL_TARGET, budget_split, build_plan_with, cgroup_free_mb_with,
        cgroup_level_free_mb, concurrency_cap_with, mem_available_mb_from, self_cgroup_from,
    };

    // --- M6b: the RAM probe must not fail open in a container (2026-07-17).
    //
    // `probe_free_mb` had ZERO tests, which is how it stayed host-wide while
    // `available_parallelism` next door honoured the cgroup CPU quota. It reads real files,
    // so the parsing is pure and the hierarchy walk takes an injected reader: on this dev
    // box every cgroup level reports `max`, so the real filesystem cannot exercise the bug.

    #[test]
    fn mem_available_is_parsed_from_meminfo() {
        let s = "MemTotal:       32000000 kB\nMemFree:         1000000 kB\nMemAvailable:    2097152 kB\n";
        assert_eq!(mem_available_mb_from(s), Some(2048));
        assert_eq!(mem_available_mb_from("MemTotal: 1 kB\n"), None);
    }

    #[test]
    fn a_cgroup_level_reports_headroom_and_max_means_no_limit() {
        // 2 GiB limit, 1 GiB used -> 1024 MiB of headroom.
        assert_eq!(cgroup_level_free_mb("2147483648", "1073741824"), Some(1024));
        // "max" is v2 for "no limit here": not a number, so no opinion.
        assert_eq!(cgroup_level_free_mb("max", "1073741824"), None);
        // Already over the limit: saturate at 0 rather than underflow.
        assert_eq!(cgroup_level_free_mb("1048576", "2097152"), Some(0));
    }

    #[test]
    fn self_cgroup_reads_the_v2_unified_line() {
        assert_eq!(
            self_cgroup_from("1:net_cls:/\n0::/user.slice/app.scope\n").as_deref(),
            Some("/user.slice/app.scope")
        );
        // v1-only (no unified line): unhandled, and says so by returning None.
        assert_eq!(self_cgroup_from("4:memory:/docker/abc\n"), None);
    }

    #[test]
    fn a_limit_on_an_ancestor_cgroup_is_found_and_the_tightest_wins() {
        // The case that makes the walk load-bearing: our own leaf sets no limit, but a
        // parent does (Docker `-m`, a k8s pod limit, a systemd slice all land above the
        // process's own cgroup). Reading only our level would still fail open in exactly
        // the container this exists to survive.
        let read = |p: &str| -> Option<String> {
            match p {
                "/sys/fs/cgroup/a/b/memory.max" => Some("max".into()),
                "/sys/fs/cgroup/a/b/memory.current" => Some("0".into()),
                // Parent: 2 GiB limit, 1 GiB used -> 1024 MiB free. The tightest.
                "/sys/fs/cgroup/a/memory.max" => Some("2147483648".into()),
                "/sys/fs/cgroup/a/memory.current" => Some("1073741824".into()),
                // Root: 8 GiB limit, 0 used -> 8192 MiB free. Looser, must NOT win.
                "/sys/fs/cgroup/memory.max" => Some("8589934592".into()),
                "/sys/fs/cgroup/memory.current" => Some("0".into()),
                _ => None,
            }
        };
        assert_eq!(cgroup_free_mb_with("/a/b", read), Some(1024));
    }

    #[test]
    fn no_cgroup_limit_anywhere_means_no_opinion() {
        let read = |p: &str| -> Option<String> {
            if p.ends_with("memory.max") {
                Some("max".into())
            } else if p.ends_with("memory.current") {
                Some("123".into())
            } else {
                None
            }
        };
        // Every level unlimited (this dev box): the host figure must stand alone.
        assert_eq!(cgroup_free_mb_with("/user.slice/app.scope", read), None);
        // Nothing readable at all: no opinion, never a panic.
        assert_eq!(cgroup_free_mb_with("/a/b", |_| None), None);
    }

    #[test]
    fn the_container_that_failed_open_is_now_capped_by_ram_not_cores() {
        // The audit's exact scenario: `--cpus 16 -m 2G` on a 128 GB host.
        let host_memavailable_mb = 128 * 1024; // the host, which the cgroup does not govern
        let cgroup_free_mb = 2 * 1024; // what we may ACTUALLY use
        let effective = host_memavailable_mb.min(cgroup_free_mb);

        // Before: the host figure fed the cap, so the cores won and RAM was ignored.
        assert_eq!(
            concurrency_cap_with(None, PER_KERNEL_MB, 16, host_memavailable_mb),
            16,
            "pre-fix: 16 kernels x 150 MB against a 2 GB ceiling, i.e. the OOM killer"
        );
        // After: memory is the limiter, as it always should have been.
        assert_eq!(
            concurrency_cap_with(None, PER_KERNEL_MB, 16, effective),
            13,
            "2048 MiB / 150 MiB = 13 kernels, under the 16 cores"
        );
    }

    // --- build_plan: explicit `--jobs N` means N parallel PAGES (owner ruling 2026-07-17).
    //
    // These pin the arithmetic; they cannot pin the composition that actually broke (the
    // build docking N before knowing whether a pool boots), which is why the real gate for
    // this ruling is `tests/build_jobs.rs`. See that file's module doc.

    #[test]
    fn explicit_jobs_is_the_page_count_and_the_pool_is_additive() {
        // --jobs 3 = 3 pages. The pool does NOT come out of the user's number.
        let p = build_plan_with(Some(3), 150, 16, 32_000);
        assert_eq!(p.build_kernels, 3);
        assert_eq!(p.warm_pool, WARM_POOL_TARGET);
        assert!(!p.shares_budget);
    }

    #[test]
    fn explicit_jobs_one_is_sequential_but_still_pre_warms() {
        // `budget_split`'s "cap 1 -> no warm pool" is a SHARED-budget rule; under an
        // explicit --jobs the two don't share, so the rationale doesn't carry. Precedent:
        // a serially-building preview still gets a full pool (`preview_warm_pool_size`).
        let p = build_plan_with(Some(1), 150, 16, 32_000);
        assert_eq!(p.build_kernels, 1);
        assert_eq!(p.warm_pool, WARM_POOL_TARGET);
    }

    #[test]
    fn explicit_jobs_ignores_memory_pressure() {
        // Consistent with `concurrency_cap_with`'s Some(n) arm, which has never consulted
        // free_mb: the user's stated concurrency is honored, RAM or no RAM. (Memory is
        // only ours to reason about in auto mode.)
        let starved = build_plan_with(Some(8), 150, 16, 10);
        assert_eq!(starved.build_kernels, 8);
    }

    #[test]
    fn auto_is_unaffected_and_still_shares_one_budget() {
        // cores 16, free 32000/150 = 213 -> cap 16 -> 2 warm + 14 build, summing to 16.
        let p = build_plan_with(None, 150, 16, 32_000);
        assert_eq!(p.warm_pool, 2);
        assert_eq!(p.build_kernels, 14);
        assert!(p.shares_budget);
        assert_eq!(p.warm_pool + p.build_kernels, 16);
    }

    #[test]
    fn auto_matches_the_old_composed_behavior_across_the_range() {
        // The ruling must not move auto mode at all: it stays exactly
        // `budget_split(concurrency_cap(None, ..).max(1))`.
        for cores in 1..=16 {
            for free_mb in [10u64, 300, 1_000, 32_000] {
                let expect = budget_split(concurrency_cap_with(None, 150, cores, free_mb).max(1));
                assert_eq!(
                    build_plan_with(None, 150, cores, free_mb),
                    expect,
                    "auto drifted at cores={cores} free_mb={free_mb}"
                );
                // `Some(0)` is auto too.
                assert_eq!(build_plan_with(Some(0), 150, cores, free_mb), expect);
            }
        }
    }

    // --- build_cap: who owns pool slots that never booted (M1's rule, scoped).

    #[test]
    fn auto_returns_unbooted_pool_slots_to_the_build() {
        // M1: on a bare `python3` no pool boots, so the default build must use the whole
        // memory cap (16 here), not silently forfeit the 2 it reserved.
        let p = build_plan_with(None, 150, 16, 32_000);
        assert_eq!(p.build_cap(0), 16);
        // A pool that did boot keeps its slots: 14 build + 2 warm <= 16.
        assert_eq!(p.build_cap(2), 14);
    }

    #[test]
    fn explicit_jobs_holds_at_n_whether_or_not_the_pool_boots() {
        // Nothing to hand back: the pool was never funded from the user's N.
        let p = build_plan_with(Some(3), 150, 16, 32_000);
        assert_eq!(p.build_cap(0), 3);
        assert_eq!(p.build_cap(2), 3);
    }

    #[test]
    fn auto_never_exceeds_its_budget_however_the_pool_lands() {
        // The auto-mode invariant, across the range and across every possible pool
        // outcome: resident kernels (build + warmed) never exceed the memory cap.
        for cores in 1..=16 {
            for free_mb in [10u64, 300, 1_000, 32_000] {
                let cap = concurrency_cap_with(None, 150, cores, free_mb).max(1);
                let p = build_plan_with(None, 150, cores, free_mb);
                for warmed in 0..=p.warm_pool {
                    assert!(
                        p.build_cap(warmed) + warmed <= cap,
                        "auto overspends at cores={cores} free_mb={free_mb} warmed={warmed}: \
                         {} + {warmed} > {cap}",
                        p.build_cap(warmed)
                    );
                }
            }
        }
    }

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
