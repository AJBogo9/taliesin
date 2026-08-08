//! Naming + startup sweep for the runtime `/tmp` dirs that leak on ungraceful taliesin
//! death.
//!
//! `Kernel` removes its `/tmp` connection dir in `Drop`, but a SIGKILL / crash / closed
//! terminal skips `Drop`, orphaning the dir. (The kernel process itself self-reaps and
//! dies with taliesin, but removing the *dir* was always `Drop`'s job, and that never
//! ran.) We
//! tag each dir with the server's pid, so a later start can identify and reclaim the
//! ones whose owner is dead. Strictly pid-based: a live process's dir is never touched.
//!
//! Design: docs/superpowers/specs/2026-07-19-stale-runtime-dir-sweep-design.md.

use std::path::{Path, PathBuf};

const KERNEL_PREFIX: &str = "tali-kernel-";

/// A fresh cold-kernel connection dir, tagged `tali-kernel-<pid>_<uuid>` so a later run
/// can reclaim it if we die ungracefully. Replaces the old `tali-kernel-<uuid>`.
pub(crate) fn kernel_conn_dir() -> PathBuf {
    tagged(KERNEL_PREFIX)
}

/// `<prefix><pid>_<uuid>`, where `<pid>` is THIS server's — the process whose death
/// orphans the dir. The separator is `_` (never present in a decimal pid or a uuid) so
/// a legacy `<prefix><uuid>` (no `_`) is unambiguously "no owner pid": a uuid's first 8
/// hex chars are all-decimal ~2.3% of the time, so a `-` separator would misread some
/// legacy dirs as pid-tagged.
fn tagged(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

/// The owner pid encoded in a runtime dir NAME, or `None` for the legacy uuid-only
/// format (no `_`) — which the sweep must never touch. `name` is a file name; `prefix`
/// is the runtime prefix it must carry.
fn owner_pid(name: &str, prefix: &str) -> Option<u32> {
    let rest = name.strip_prefix(prefix)?;
    let (pid, _uuid) = rest.split_once('_')?;
    pid.parse::<u32>().ok()
}

/// Best-effort: remove leaked kernel dirs under the system temp dir whose
/// owning server pid is dead. Called once at the start of the kernel-spawning commands
/// (preview/serve + build).
pub(crate) fn sweep_stale_runtime_dirs() {
    sweep_in(&std::env::temp_dir());
}

/// The sweep, with `base` injected so tests never scan the real `/tmp` (where a
/// concurrent session's live kernels sit). Removes a `<prefix><pid>_<uuid>` dir iff its
/// pid is not our own and is provably dead; it skips legacy (no-pid) dirs, our own pid,
/// and any live pid. A recycled pid only ever reads as alive (the dir is kept), never
/// as a wrong delete — so no path removes a dir owned by a live process.
fn sweep_in(base: &Path) {
    let own = std::process::id();
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(pid) = owner_pid(name, KERNEL_PREFIX) else {
            continue;
        };
        if pid == own || pid_alive(pid) {
            continue;
        }
        // Owner is gone: reclaim the orphan. Best-effort — a race with another sweeper
        // or a permissions quirk is not worth failing a server start over.
        let _ = std::fs::remove_dir_all(entry.path());
    }
}

/// Whether `pid` is a live process. `kill(pid, 0)` probes without signalling: `ESRCH`
/// means gone; `0` or `EPERM` (unreachable for a same-user pid) means alive. Any other
/// error reads as alive, keeping the sweep conservative — a false "alive" only leaves a
/// dir uncleaned, it never deletes a live process's dir.
#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    // Safety: signal 0 performs permission/existence checking only and delivers no
    // signal, so a stale or recycled pid is at worst probed, never disturbed.
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if rc == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    // No cheap cross-process liveness probe; stay conservative and sweep nothing.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_pid_reads_the_pid_tag_and_skips_legacy() {
        // New pid-tagged format → the owner pid.
        assert_eq!(
            owner_pid(
                "tali-kernel-3698019_cc8652b8-6ab2-4f1f-b362-7e4a23592628",
                KERNEL_PREFIX
            ),
            Some(3698019)
        );
        // Legacy uuid-only (no `_`) → None: never swept.
        assert_eq!(
            owner_pid(
                "tali-kernel-cc8652b8-6ab2-4f1f-b362-7e4a23592628",
                KERNEL_PREFIX
            ),
            None
        );
        // The whole reason the separator is `_`: a legacy uuid whose first 8 hex chars
        // are ALL decimal must still read as legacy, not as a pid `12345678`.
        assert_eq!(
            owner_pid(
                "tali-kernel-12345678-6ab2-4f1f-b362-7e4a23592628",
                KERNEL_PREFIX
            ),
            None
        );
        // Prefix mismatch → None.
        assert_eq!(owner_pid("tali-conn-99_x", KERNEL_PREFIX), None);
        assert_eq!(owner_pid("tali-interp-python3", KERNEL_PREFIX), None);
    }

    /// The producer and the consumer were only ever tested apart: `owner_pid` against
    /// hand-written names, `sweep_in` against hand-built dirs. So nothing checked that the names
    /// `tagged` actually *produces* are the ones `owner_pid` can read back, and every producer
    /// survived being replaced by `PathBuf::default()` in the 2026-07-27 mutation run.
    /// If the tag format ever drifted, the sweep would silently stop matching real dirs while
    /// every test above stayed green. Close the loop.
    #[test]
    fn a_produced_dir_name_round_trips_through_the_owner_pid_reader() {
        let own = std::process::id();
        let path = kernel_conn_dir();
        assert_eq!(
            path.parent(),
            Some(std::env::temp_dir().as_path()),
            "a runtime dir belongs under the system temp dir, got {path:?}"
        );
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with(KERNEL_PREFIX),
            "{name:?} must carry {KERNEL_PREFIX:?}"
        );
        assert_eq!(
            owner_pid(name, KERNEL_PREFIX),
            Some(own),
            "the sweep must read this process's pid back out of {name:?}"
        );
        // The uuid half keeps two dirs from the same server apart, so a second kernel never
        // reuses the first one's connection dir.
        assert_ne!(kernel_conn_dir(), kernel_conn_dir());
    }

    /// `sweep_stale_runtime_dirs` is the entry point `main` actually calls, and replacing its
    /// whole body with `()` survived: every test here calls `sweep_in` directly with an isolated
    /// base, so the delegation itself — and the choice of the *system temp dir* as the base — was
    /// never exercised. Plant one dead-owner orphan in the real temp dir and watch the real entry
    /// point reclaim it.
    ///
    /// Safe by construction: the only thing this can remove is a `tali-kernel-`
    /// dir whose owner pid is provably dead, which is exactly what the function exists to remove.
    /// The sibling test's fixtures live inside its own subdirectory, so a top-level scan cannot
    /// see them.
    #[test]
    #[cfg(unix)]
    fn the_public_sweep_reclaims_an_orphan_under_the_real_temp_dir() {
        // A genuinely-dead pid: spawn, kill, reap, so the pid is free.
        let mut dead_child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let dead_pid = dead_child.id();
        dead_child.kill().unwrap();
        dead_child.wait().unwrap();

        let orphan = std::env::temp_dir().join(format!(
            "{KERNEL_PREFIX}{dead_pid}_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&orphan).unwrap();

        sweep_stale_runtime_dirs();

        let reclaimed = !orphan.exists();
        let _ = std::fs::remove_dir_all(&orphan); // a no-op when the sweep did its job
        assert!(
            reclaimed,
            "the public sweep must reclaim the orphan it was given: {orphan:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn sweep_removes_dead_owner_dirs_and_keeps_the_rest() {
        // Isolated base dir so the sweep never touches the real /tmp (where a parallel
        // session's live kernels sit).
        let base = std::env::temp_dir().join(format!(
            "tali-sweeptest-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&base).unwrap();

        // A genuinely-dead pid: spawn a child, then kill + reap it so its pid is free.
        let mut dead_child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let dead_pid = dead_child.id();
        dead_child.kill().unwrap();
        dead_child.wait().unwrap();

        // A LIVE non-own pid: a child kept running across the sweep. Its dir must be
        // kept — this is the case `pid_alive` exists for (drop the liveness check and
        // this assertion fails).
        let mut live_child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let live_pid = live_child.id();

        let own_pid = std::process::id();
        let dead_kernel = base.join(format!("tali-kernel-{dead_pid}_{}", uuid::Uuid::new_v4()));
        let dead_pool = base.join(format!("tali-kernel-{dead_pid}_{}", uuid::Uuid::new_v4()));
        let own = base.join(format!("tali-kernel-{own_pid}_{}", uuid::Uuid::new_v4()));
        let live = base.join(format!("tali-kernel-{live_pid}_{}", uuid::Uuid::new_v4()));
        let legacy = base.join(format!("tali-kernel-{}", uuid::Uuid::new_v4()));
        for d in [&dead_kernel, &dead_pool, &own, &live, &legacy] {
            std::fs::create_dir_all(d).unwrap();
        }

        sweep_in(&base);

        let dead_kernel_gone = !dead_kernel.exists();
        let dead_pool_gone = !dead_pool.exists();
        let own_kept = own.exists();
        let live_kept = live.exists();
        let legacy_kept = legacy.exists();

        // Tear the live child down before asserting, so a failure can't leak it.
        live_child.kill().unwrap();
        live_child.wait().unwrap();
        let _ = std::fs::remove_dir_all(&base);

        assert!(dead_kernel_gone, "a dead-owner kernel dir must be swept");
        assert!(dead_pool_gone, "a second dead-owner dir must be swept too");
        assert!(own_kept, "our own live pid's dir must be kept");
        assert!(live_kept, "a live non-own pid's dir must be kept");
        assert!(legacy_kept, "a legacy (no-pid) dir must be left untouched");
    }
}
