//! Run a per-page pass across the machine's cores, in input order.
//!
//! Three whole-project passes render EVERY page: [`super::Site::harvest_xref_numbers`]
//! (cross-page float numbers), [`super::search::build_sections`] (the Cmd-K index) — both
//! of which `Site::discover` runs back to back — and `harvest_xref_numbers` again on every
//! save via [`super::Site::refresh_xrefs`]. They were sequential `for page in &self.pages`
//! loops, so a cold discover cost the sum of every page's render on one core while the
//! other fifteen sat idle: measured 2026-08-27, `corpus/tech-blog` took 337 ms for 17
//! pages before this module existed.
//!
//! Each page's render is independent — it reads its own source and returns owned data —
//! so this is plain data parallelism, not coordination. Deliberately `std::thread::scope`
//! rather than a dependency: the work is one bounded map with no nesting and no
//! work-stealing subtleties, which is the shape `scope` already covers.

use std::sync::atomic::{AtomicUsize, Ordering};

/// How many workers to run `items` across: never more threads than there is work, and
/// never more than the machine (or the cgroup CPU quota, which `available_parallelism`
/// honours) actually offers. One worker means the caller's own thread and no spawn at all.
fn worker_count(items: usize) -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(items)
}

/// Map `f` over `items` concurrently, returning the results in **input order**.
///
/// Order is load-bearing, not a nicety: `harvest_xref_numbers` resolves a duplicate
/// cross-reference label by "first definition wins", so a result set in completion order
/// would hand the anchor to whichever page happened to render fastest and make the built
/// site depend on scheduling. Workers pull the next index off a shared counter (so one slow
/// page does not strand a whole static chunk) and the results are sorted back afterwards.
///
/// **Panics propagate**, exactly as the sequential loop's did. `Site::refresh_xrefs` wraps
/// the harvest in `catch_unwind` to keep its all-or-nothing registry contract, and that
/// contract depends on a panicking page still reaching the caller — `std::thread::scope`
/// re-raises a scoped thread's panic when it joins, so the contract is unchanged here.
pub(super) fn map_ordered<I, O, F>(items: &[I], f: F) -> Vec<O>
where
    I: Sync,
    O: Send,
    F: Fn(&I) -> O + Sync,
{
    let workers = worker_count(items.len());
    if workers <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let (tx, rx) = std::sync::mpsc::channel::<(usize, O)>();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let tx = tx.clone();
            let (next, f) = (&next, &f);
            scope.spawn(move || {
                loop {
                    // The index MUST come from `fetch_add`'s own return value. Reading the
                    // counter back afterwards looks equivalent and is not: another worker
                    // can increment in between, so the result gets filed under someone
                    // else's index and two pages swap places in the output. `Relaxed` is
                    // enough — the counter is the only shared mutable state, and it hands
                    // each index to exactly one worker regardless of ordering.
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else { break };
                    if tx.send((i, f(item))).is_err() {
                        break;
                    }
                }
            });
        }
        // The collector holds the last `Sender`; dropping it here is what lets `rx`
        // terminate once every worker has finished and dropped its clone.
        drop(tx);
        let mut out: Vec<(usize, O)> = rx.iter().collect();
        out.sort_by_key(|(i, _)| *i);
        out.into_iter().map(|(_, o)| o).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex;

    #[test]
    fn results_come_back_in_input_order_however_they_finish() {
        // Reverse-graded work: item 0 sleeps longest, so completion order is the exact
        // reverse of input order and a collection-order bug cannot hide.
        let items: Vec<u64> = (0..8).collect();
        let out = map_ordered(&items, |i| {
            std::thread::sleep(std::time::Duration::from_millis((8 - i) * 4));
            i * 10
        });
        assert_eq!(out, vec![0, 10, 20, 30, 40, 50, 60, 70]);
    }

    #[test]
    fn the_work_is_actually_spread_across_threads() {
        let items: Vec<usize> = (0..64).collect();
        let seen: Mutex<HashSet<std::thread::ThreadId>> = Mutex::new(HashSet::new());
        let out = map_ordered(&items, |i| {
            seen.lock().unwrap().insert(std::thread::current().id());
            // Enough work that the pull-based counter hands items to several workers
            // rather than one racing through the whole list first.
            std::thread::sleep(std::time::Duration::from_millis(2));
            *i
        });
        assert_eq!(out, items, "order still preserved");
        let threads = seen.into_inner().unwrap().len();
        // Only assert plurality, never a specific count: a single-core machine (or a
        // one-CPU cgroup) legitimately answers 1, and this must not fail there.
        if std::thread::available_parallelism().is_ok_and(|n| n.get() > 1) {
            assert!(
                threads > 1,
                "expected fan-out, all work ran on {threads} thread"
            );
        }
    }

    #[test]
    fn an_empty_or_single_item_slice_needs_no_threads() {
        let empty: Vec<u8> = Vec::new();
        assert!(map_ordered(&empty, |_| 1u8).is_empty());
        let one = vec![7u8];
        let caller = std::thread::current().id();
        let out = map_ordered(&one, |i| (*i, std::thread::current().id()));
        assert_eq!(
            out,
            vec![(7, caller)],
            "a single item runs inline, no spawn"
        );
    }

    /// `Site::refresh_xrefs` wraps the harvest in `catch_unwind` and restores the previous
    /// registry on a panic. That contract only holds if a panicking page still reaches the
    /// caller as a panic rather than being swallowed by a worker thread.
    #[test]
    fn a_panicking_item_propagates_to_the_caller() {
        let items: Vec<usize> = (0..8).collect();
        let hit = std::panic::catch_unwind(|| {
            map_ordered(&items, |i| {
                assert_ne!(*i, 5, "boom");
                *i
            })
        });
        assert!(hit.is_err(), "a worker's panic must reach the caller");
    }
}
