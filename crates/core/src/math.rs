//! Server-side math rendering via KaTeX.
//!
//! The `katex` crate runs KaTeX in an embedded JS engine and reuses the JS
//! context per thread, so there is no per-render process startup — math is
//! rendered to static HTML+MathML at parse time, no client-side JS required
//! (only KaTeX's stylesheet for fonts).

/// Render a LaTeX fragment to HTML. KaTeX is configured with
/// `throw_on_error = false`, so an invalid expression renders inline (in red)
/// rather than aborting the document; engine-level failures fall back to the
/// escaped source wrapped in a `qmd-math-error` span.
pub fn render(latex: &str, display: bool) -> String {
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
        assert!(html.contains("katex-display"), "expected display markup, got: {html}");
    }

    #[test]
    fn invalid_math_does_not_panic() {
        // throw_on_error=false: KaTeX renders the error inline rather than failing.
        let _ = render("\\frac{", false);
    }
}
