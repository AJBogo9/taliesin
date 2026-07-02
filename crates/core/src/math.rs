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

use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex};

type Key = (String, bool);

/// A bounded `(latex, display_mode) -> rendered HTML` memo. On overflow it evicts the
/// OLDEST-inserted entries (FIFO via `order`) one at a time rather than clearing the
/// whole map, so a burst of distinct expressions past the cap doesn't drop the entire
/// warm set (which the previous full-clear did, cold-starting every subsequent save).
#[derive(Default)]
struct MathCache {
    map: HashMap<Key, String>,
    order: VecDeque<Key>,
}
impl MathCache {
    /// Insert `key -> html`, keeping at most `cap` entries by evicting the oldest-
    /// inserted first (FIFO). A no-op if `key` is already present (so `order` never
    /// holds duplicates and a re-render doesn't disturb the eviction order).
    fn insert_bounded(&mut self, key: Key, html: String, cap: usize) {
        if self.map.contains_key(&key) {
            return;
        }
        while self.map.len() >= cap {
            match self.order.pop_front() {
                Some(old) => {
                    self.map.remove(&old);
                }
                None => break,
            }
        }
        self.order.push_back(key.clone());
        self.map.insert(key, html);
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

fn render_uncached(latex: &str, display: bool) -> String {
    let opts = katex::Opts::builder()
        .display_mode(display)
        .throw_on_error(false)
        .build();
    match opts {
        Ok(opts) => katex::render_with_opts(latex, &opts).unwrap_or_else(|_| fallback(latex)),
        Err(_) => fallback(latex),
    }
}

fn fallback(latex: &str) -> String {
    let mut escaped = String::new();
    for ch in latex.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(ch),
        }
    }
    format!("<span class=\"tali-math-error\" title=\"math render failed\">{escaped}</span>")
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

    #[test]
    fn cache_evicts_oldest_first_and_stays_bounded() {
        // FIFO eviction (no KaTeX needed): at cap, the oldest-inserted key is dropped,
        // recent keys survive, and the map never exceeds the cap (was a full clear).
        let mut c = MathCache::default();
        for i in 0..3 {
            c.insert_bounded((i.to_string(), false), format!("h{i}"), 3);
        }
        assert_eq!(c.map.len(), 3);
        c.insert_bounded(("3".into(), false), "h3".into(), 3); // over cap: evict "0"
        assert_eq!(c.map.len(), 3, "stays bounded, not cleared");
        assert!(
            !c.map.contains_key(&("0".to_string(), false)),
            "oldest evicted"
        );
        assert!(c.map.contains_key(&("3".to_string(), false)), "newest kept");
        assert!(c.map.contains_key(&("2".to_string(), false)), "recent kept");
        // Re-inserting an existing key is a no-op (doesn't reorder or grow).
        c.insert_bounded(("2".into(), false), "dup".into(), 3);
        assert_eq!(c.map.get(&("2".to_string(), false)).unwrap(), "h2");
        assert_eq!(c.order.len(), 3, "no duplicate in the eviction queue");
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
