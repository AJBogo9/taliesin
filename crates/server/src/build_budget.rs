//! Memory-aware build concurrency cap.
//!
//! `concurrency_cap` decides how many parallel quarto/kernel processes to allow,
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
            let mem_slots = if per_kernel_mb == 0 {
                cores
            } else {
                (free_mb / per_kernel_mb) as usize
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
    use super::concurrency_cap_with;

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
