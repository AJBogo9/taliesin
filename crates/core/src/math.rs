//! Server-side math rendering via KaTeX.
//!
//! The `katex` crate runs KaTeX in an embedded JS engine and reuses the JS
//! context per thread, so there is no per-render process startup — math is
//! rendered to static HTML+MathML at parse time, no client-side JS required
//! (only KaTeX's stylesheet for fonts).
//!
//! Even so, each render is a JS evaluation (~1 ms), and the dev server re-renders
//! the *whole* document on every save — so a math-heavy page would re-render every
//! expression each keystroke (hundreds of ms). KaTeX output is a pure function of
//! `(latex, display_mode)` under our fixed options, so [`render`] memoizes results
//! in a process-global cache: after the first render, the unchanged math on a save
//! (i.e. all of it but the block being edited) is a hashmap hit. The cache persists
//! for the life of the process (and is shared across a site's pages).

use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};
use std::sync::{LazyLock, Mutex};

type Key = (String, bool);

/// A bounded `(latex, display_mode) -> rendered HTML` memo that keeps what it holds: once
/// full it takes nothing more, rather than evicting.
///
/// Evicting was a cliff. A whole-project pass (the Cmd-K index, a build) reads the project's
/// math in page order, the same order every time, so a policy that makes room for the newest
/// entry (oldest-first, or least-recently-used) evicts each expression just before the pass
/// comes round to it again: past the cap no pass hit at all, and a save of a book with 9,693
/// distinct expressions took 10.7 s where one with 6,723 took 128 ms (audit 2026-09-24, F1).
/// Keeping the first `cap` holds a repeated pass at a `cap / distinct` hit rate. What that
/// gives up is small: an expression first typed after the memo filled is typeset on every
/// render of its own page. (A full clear, before FIFO, cold-started every later save.)
#[derive(Default)]
struct MathCache {
    map: HashMap<Key, String>,
}
impl MathCache {
    /// Insert `key -> html` while fewer than `cap` entries are held. A no-op once full, and
    /// for a key already present.
    fn insert_bounded(&mut self, key: Key, html: String, cap: usize) {
        if self.map.len() < cap {
            self.map.entry(key).or_insert(html);
        }
    }
}
static CACHE: LazyLock<Mutex<MathCache>> = LazyLock::new(|| Mutex::new(MathCache::default()));
const CACHE_CAP: usize = 8192;

/// Render a LaTeX fragment to HTML (memoized). KaTeX is configured with
/// `throw_on_error = false`, so an invalid expression renders inline (in red)
/// rather than aborting the document; engine-level failures fall back to the
/// escaped source wrapped in a `tali-math-error` span.
pub fn render(latex: &str, display: bool) -> String {
    // A poisoned lock can only happen if a thread panicked *holding* it; we never
    // render (the only fallible work) under the lock, so recover the map either way.
    let key = (latex.to_string(), display);
    if let Some(hit) = CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .map
        .get(&key)
    {
        return hit.clone();
    }
    let html = render_uncached(latex, display);
    CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert_bounded(key, html.clone(), CACHE_CAP);
    html
}

/// Whether `(latex, display)` has been typeset in this process, i.e. is in the memo. Test-only:
/// the witness that a pass did or did not typeset something.
#[cfg(test)]
pub(crate) fn is_memoized(latex: &str, display: bool) -> bool {
    CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .map
        .contains_key(&(latex.to_string(), display))
}

/// One KaTeX request: the expression, its mode, and where to send the HTML back.
type Job = (String, bool, Sender<String>);

/// The single thread KaTeX ever runs on.
///
/// The `katex` crate keeps its JS context in a **thread-local**, so the ~24.7 ms QuickJS
/// boot is paid once per thread that renders math — and [`crate::render`] spawns a fresh
/// big-stack thread for *every* render, so before this existed the boot was paid again on
/// every page of a whole-project pass and on every cold document render. Measured
/// 2026-08-27, release: a cold `Site::discover` of `corpus/tech-blog` was 497 ms for 17
/// pages, dominated by exactly this.
///
/// Funnelling every miss through one long-lived worker makes it once per PROCESS instead.
/// Serialising the calls costs nothing worth measuring — a warm KaTeX render is ~0.07 ms,
/// and [`CACHE`] absorbs the repeats — and it holds one JS context rather than one per
/// render thread, which is also what makes the concurrent page loops in `site` affordable.
static KATEX: LazyLock<Option<Mutex<Sender<Job>>>> = LazyLock::new(|| {
    let (tx, rx) = channel::<Job>();
    std::thread::Builder::new()
        .name("taliesin-katex".to_string())
        .spawn(move || {
            // Ends when the last `Sender` drops, i.e. at process exit.
            for (latex, display, reply) in rx {
                // The reply channel is gone when the requester was abandoned by the render
                // watchdog; that is expected, so the send result is deliberately ignored.
                let _ = reply.send(render_on_this_thread(&latex, display));
            }
        })
        .ok()
        .map(|_| Mutex::new(tx))
});

/// Render on the KaTeX worker, falling back to the calling thread if the worker could not
/// be spawned or has died. The fallback is a correctness guarantee, not an optimization:
/// math must still render (paying its own boot) on a machine that refuses a new thread.
fn render_uncached(latex: &str, display: bool) -> String {
    let Some(worker) = KATEX.as_ref() else {
        return render_on_this_thread(latex, display);
    };
    let (reply_tx, reply_rx) = channel();
    // The lock is held only for the send: the worker processes serially anyway, and
    // holding it across the recv would stop callers from queueing behind each other.
    let sent = worker
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .send((latex.to_string(), display, reply_tx))
        .is_ok();
    if !sent {
        return render_on_this_thread(latex, display);
    }
    reply_rx
        .recv()
        .unwrap_or_else(|_| render_on_this_thread(latex, display))
}

fn render_on_this_thread(latex: &str, display: bool) -> String {
    // Which thread actually booted a JS context, so `katex_runs_on_exactly_one_thread`
    // can pin the invariant without a wall clock. Compiled out of release entirely.
    #[cfg(test)]
    RENDER_THREADS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(std::thread::current().id());
    let opts = katex::Opts::builder()
        .display_mode(display)
        .throw_on_error(false)
        .build();
    match opts {
        Ok(opts) => katex::render_with_opts(latex, &opts).unwrap_or_else(|_| fallback(latex)),
        Err(_) => fallback(latex),
    }
}

#[cfg(test)]
static RENDER_THREADS: LazyLock<Mutex<std::collections::HashSet<std::thread::ThreadId>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));

fn fallback(latex: &str) -> String {
    format!(
        "<span class=\"tali-math-error\" title=\"math render failed\">{}</span>",
        crate::render::html_escape(latex)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_inline_math_to_katex_html() {
        let html = render("x^2 + y^2", false);
        assert!(html.contains("katex"), "expected katex markup, got: {html}");
    }

    #[test]
    fn display_mode_emits_display_class() {
        let html = render("\\int_0^1 x \\, dx", true);
        assert!(
            html.contains("katex-display"),
            "expected display markup, got: {html}"
        );
    }

    #[test]
    fn invalid_math_does_not_panic() {
        // throw_on_error=false: KaTeX renders the error inline rather than failing.
        let _ = render("\\frac{", false);
    }

    /// The invariant E exists for: however many threads render math, KaTeX itself runs on
    /// exactly one, so the ~24.7 ms QuickJS boot is paid once per process rather than once
    /// per render thread. Asserted through the recorded thread ids, not a wall clock, so it
    /// cannot flake on a slow or a single-core machine.
    #[test]
    fn katex_runs_on_exactly_one_thread_however_many_threads_ask() {
        // Distinct expressions, so every one of them MISSES the memo and reaches KaTeX.
        std::thread::scope(|scope| {
            for t in 0..4 {
                scope.spawn(move || {
                    for i in 0..3 {
                        let html = render(&format!("z_{{{t}{i}}} + \\alpha"), false);
                        assert!(html.contains("katex"), "expected katex markup: {html}");
                    }
                });
            }
        });
        let threads = RENDER_THREADS.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            threads.len(),
            1,
            "KaTeX booted a JS context on {} threads; the worker must be the only one",
            threads.len()
        );
        assert_ne!(
            threads.iter().next().copied(),
            Some(std::thread::current().id()),
            "the work must land on the worker, not inline on the caller"
        );
    }

    /// A whole-project pass (the Cmd-K index, a build) reads the project's math in page
    /// order, the same order every time. Evicting the oldest entry, past the cap each
    /// expression was evicted just before it came round again: no pass ever hit, and every
    /// save re-typeset the whole project on the one KaTeX thread (audit 2026-09-24, F1).
    #[test]
    fn a_full_cache_still_hits_on_every_repeated_pass() {
        let (cap, distinct) = (8, 10);
        let mut c = MathCache::default();
        let mut hits = 0;
        for _pass in 0..3 {
            for i in 0..distinct {
                let key = (i.to_string(), false);
                if c.map.contains_key(&key) {
                    hits += 1;
                } else {
                    c.insert_bounded(key, format!("h{i}"), cap);
                }
            }
        }
        assert_eq!(c.map.len(), cap, "bounded");
        assert_eq!(
            hits,
            2 * cap,
            "each pass after the first hits all {cap} held"
        );
    }

    #[test]
    fn a_full_cache_keeps_what_it_holds_and_stays_bounded() {
        // No KaTeX needed: at cap, a new key is not taken, nothing held is dropped, and the
        // map never exceeds the cap (it was a full clear, then oldest-first eviction).
        let mut c = MathCache::default();
        for i in 0..3 {
            c.insert_bounded((i.to_string(), false), format!("h{i}"), 3);
        }
        c.insert_bounded(("3".into(), false), "h3".into(), 3);
        assert_eq!(c.map.len(), 3, "stays bounded, not cleared");
        assert!(!c.map.contains_key(&("3".to_string(), false)), "not taken");
        assert!(c.map.contains_key(&("0".to_string(), false)), "oldest kept");
        // Re-inserting an existing key is a no-op: the first render stands.
        c.insert_bounded(("2".into(), false), "dup".into(), 4);
        assert_eq!(c.map.get(&("2".to_string(), false)).unwrap(), "h2");
    }

    #[test]
    fn memoized_render_is_stable_and_mode_keyed() {
        // The cache must be transparent (same input → identical output) and key on
        // the display flag, so inline and display renders never alias.
        let inline_a = render("a^2 + b^2", false);
        let inline_b = render("a^2 + b^2", false); // served from cache
        assert_eq!(inline_a, inline_b, "memoized render must be stable");
        let display = render("a^2 + b^2", true);
        assert_ne!(
            inline_a, display,
            "display mode must be a distinct cache entry"
        );
        assert!(display.contains("katex-display"));
    }
}
