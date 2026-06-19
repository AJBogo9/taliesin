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
use std::sync::{LazyLock, Mutex};

/// `(latex, display_mode) -> rendered HTML`. Bounded so a very long session with
/// many distinct expressions can't grow it without limit; math sets are small and
/// stable, so the cap is rarely reached (a plain clear-on-overflow is enough — a
/// full LRU isn't worth the complexity for this access pattern).
static CACHE: LazyLock<Mutex<HashMap<(String, bool), String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const CACHE_CAP: usize = 8192;

/// Render a LaTeX fragment to HTML (memoized). KaTeX is configured with
/// `throw_on_error = false`, so an invalid expression renders inline (in red)
/// rather than aborting the document; engine-level failures fall back to the
/// escaped source wrapped in a `qmd-math-error` span.
pub fn render(latex: &str, display: bool) -> String {
    // A poisoned lock can only happen if a thread panicked *holding* it; we never
    // render (the only fallible work) under the lock, so recover the map either way.
    let key = (latex.to_string(), display);
    if let Some(hit) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return hit.clone();
    }
    let html = render_uncached(latex, display);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if cache.len() >= CACHE_CAP {
        cache.clear();
    }
    cache.insert(key, html.clone());
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
    format!("<span class=\"qmd-math-error\" title=\"math render failed\">{escaped}</span>")
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
