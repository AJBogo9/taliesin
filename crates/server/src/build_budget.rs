//! Build concurrency planning: how many pages to render at once.
//!
//! [`concurrency_cap`] is the entry point, and it turns on **who chose the number**:
//! an explicit `--jobs N` is the user's stated *page* concurrency (the CLI says "max
//! parallel pages", in those words, and that is the only wording a user reads), honored
//! exactly and without consulting memory. Auto (`None` / `Some(0)`) is ours to spend, so
//! the memory budget is real: `min(cores, free_mb / per_kernel_mb)`.
//!
//! Note a page render is often not a kernel at all — a prose page boots none — which is
//! why the cap is a worst-case bound rather than a resident-kernel count.

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

/// Estimated peak RSS of one warm kernel, in MiB. Used by the memory-aware concurrency
/// cap so a build doesn't run more concurrent pages than free RAM can hold a kernel for.
/// Conservative on the high side; pages with no code cells never boot a kernel.
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

#[cfg(test)]
mod tests {
    use super::{
        PER_KERNEL_MB, cgroup_free_mb_with, cgroup_level_free_mb, concurrency_cap_with,
        mem_available_mb_from, self_cgroup_from,
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
}
