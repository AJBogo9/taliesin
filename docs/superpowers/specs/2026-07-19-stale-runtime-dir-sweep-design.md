# Stale runtime-`/tmp`-dir sweep (ungraceful-death, sub-part 3)

**Date:** 2026-07-19
**Status:** approved, implementing
**Context:** the third and final sub-part of the backlog "Ungraceful-death path" item.
The warm-pool (`0c04f07`) and cold-kernel (`ac1da85`) process halves have landed —
both kernel families now self-reap when taliesin dies ungracefully. What still leaks
is the **connection dir**: on SIGKILL/crash/closed-terminal, `Drop`/`ConnDirGuard`
never run, so the `/tmp` dir the dead server created is orphaned. (The kernel now
self-exits via its poller, but that kills the process, not the dir — dir removal was
always taliesin's `Drop` job.)

## Scope decision (owner-ruled)

The ~4199 stray `/tmp/tali-*` dirs measured in the wild are **two separate problems**:

1. **Runtime leak** — `tali-kernel-<uuid>` + `tali-warmpool-<uuid>`, UUID-named with
   **no owner pid**. Leak on ungraceful death. *This is the minority, and the target of
   this task.*
2. **Test debris** — `tali-omit`/`tali-check`/`tali-sbe`/… — already pid-tagged, piled
   up because the test suite doesn't self-clean. *The bulk of the 4199, but a distinct
   test-hygiene concern. Out of scope here (owner ruled "runtime leak only").*

Legacy uuid-only dirs already sitting in `/tmp` (from before this ships) are **left
untouched** (owner ruled): the sweep stays strictly pid-based with zero raciness; the
one-time residue clears with a manual `rm` if desired.

## Design

A new module **`crates/server/src/runtime_dirs.rs`** owns both the dir *naming* and the
*sweep*, so the name format and its parser are defined once and can't drift.

### Naming

```
tali-kernel-<pid>_<uuid>        (was tali-kernel-<uuid>)
tali-warmpool-<pid>_<uuid>      (was tali-warmpool-<uuid>)
```

`<pid>` is the **server's** `std::process::id()` — the process whose ungraceful death
orphans the dir. The uuid keeps per-instance uniqueness.

**The separator between pid and uuid is `_`, not `-`, deliberately.** A uuid's first
8 hex chars are all-decimal ~2.3% of the time, so a `-` separator would misparse some
legacy `tali-kernel-<uuid>` dirs as pid-tagged. A pid (decimal) and a uuid (hex +
`-`) never contain `_`, so `split_once('_')` gives an unambiguous rule: **no `_` →
legacy → skip.**

Two factories replace the inline `temp_dir().join(...)` sites
(`kernel.rs:470`, `warm_pool.rs:347`):
`kernel_conn_dir() -> PathBuf`, `warmpool_dir() -> PathBuf`.

### Sweep

```
sweep_stale_runtime_dirs()  ->  sweep_in(&std::env::temp_dir())
sweep_in(base: &Path)       // base injected so tests never scan the real /tmp
```

`sweep_in` scans `base` for entries whose name starts with either prefix, parses the
owner pid, and `remove_dir_all`s a dir **iff** `Some(pid) && pid != own_pid &&
!pid_alive(pid)`. Best-effort and non-fatal (a failed remove is logged and ignored).

- `pid_alive(pid)` = `libc::kill(pid, 0)` — `0`/`EPERM` ⇒ alive (conservative),
  `ESRCH` ⇒ dead. Same-user processes never hit `EPERM`.
- **Parallel-session safe by construction:** a live preview's pid answers alive → its
  dirs are skipped; an old-binary preview writes legacy format → skipped. A recycled
  pid can only cause a false *alive* (we keep a dir we could have cleaned — harmless),
  **never** a false delete. No code path removes a dir owned by a live process.

### Wiring

One `sweep_stale_runtime_dirs()` call at the start of the kernel-spawning entry points
(preview/serve + build). Exact call sites confirmed against `main.rs`/serve entry
during implementation (`main.rs` is clear of the live PL worktree footprint).

## Testing (each mutation-checked)

- **parser** (`owner_pid`): `tali-kernel-3698019_<uuid>` → `Some(3698019)`; legacy
  `tali-kernel-<uuid>` → `None`; wrong prefix → `None`; a legacy uuid whose first
  segment is all-decimal → `None` (proves the `_` disambiguation).
- **sweep** (`sweep_in` against an isolated base dir): a dead-pid dir (spawn `sleep`,
  kill + reap → a known-dead pid) is removed; a self-pid dir, a live-pid dir, and a
  legacy no-`_` dir are all kept.

## Non-goals

- Test-debris cleanup (separate test-hygiene task).
- Age-based sweeping (racy; rejected).
- Sweeping `tali-interp-<name>` (a name-keyed reused cache, not an accumulator) or the
  pid-tagged `tali-resdeps-<pid>` core dir (different crate, cleaned on its own path).
