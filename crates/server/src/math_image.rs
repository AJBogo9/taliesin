//! A real render of the math under the cursor, for the editor hover.
//!
//! **Why an image, when the engine is right here.** Both of the obvious routes are closed.
//! KaTeX cannot hand us a picture — its `OutputType` is `Html | Mathml | HtmlAndMathml` and
//! nothing else. And the MathML it *does* emit, which Chromium renders natively, is deleted
//! before it reaches the screen: VS Code sanitizes hover markdown against a tag allowlist
//! (`basicMarkupHtmlTags`) that contains no `<math>`, `<mrow>` or `<mi>`. What that same
//! sanitizer *does* allow is an `<img>` whose `src` is a `data:` URI — the workbench's
//! `getSanitizerOptions` passes `allowedMediaProtocols: [http, https, data, file, …]`, and
//! markdown's own `![](…)` syntax produces exactly that tag. So the only way to put real
//! typeset math in a hover is to rasterize it and inline it.
//!
//! **Why this cannot disagree with the document.** The page rendered here is the same KaTeX
//! HTML that [`taliesin_core::math`] gives a reader's page, styled with the same
//! fonts-inlined stylesheet from [`taliesin_core::render::katex_css`]. Reaching instead for a
//! LaTeX→SVG typesetter would have been less work and would have quietly rendered macros
//! KaTeX rejects: a hover that lies about what you will get is worse than one that admits to
//! approximating. That is also why every failure here returns `None` rather than a
//! best-effort drawing — the caller falls back to [`taliesin_core::math_preview`], which is
//! honest about being an approximation.
//!
//! **Cost, and how it degrades.** Rasterizing needs a browser, so this sits behind the same
//! `headless-js` feature as `{js}` observation (off by default — it is 24% of a clean release
//! build) and the same "no system Chrome, no problem" rule at runtime. Renders are cached on
//! disk by content hash, so the browser is launched only for an expression this machine has
//! never drawn before; everything else is a file read. Without the feature, without Chrome,
//! or on any error, this returns `None` and the hover is exactly what it was before.

use std::path::PathBuf;
#[cfg(feature = "headless-js")]
use std::time::Duration;

/// Bump when the rendered page changes in a way that makes every cached PNG wrong (the
/// stylesheet, the colours, the padding, the font size). It is part of the cache key, so a
/// bump orphans the old entries rather than serving them.
const RECIPE: u32 = 1;

/// How long one expression gets before we give up and let the Unicode preview answer. A
/// hover is a glance: waiting longer than this is worse than approximating.
#[cfg(feature = "headless-js")]
fn render_timeout() -> Duration {
    Duration::from_secs(
        std::env::var("TALIESIN_MATH_IMAGE_TIMEOUT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10),
    )
}

/// The rendered PNG for `latex`, as a `data:` URI ready to drop straight into hover markdown.
///
/// `None` means "no faithful image is available here" — a build without the feature, a host
/// without Chrome, a timeout, or math KaTeX could not parse. Every one of those is a fallback
/// to the Unicode preview, never an error the author is shown.
pub(crate) fn data_uri(latex: &str, display: bool, dark: bool) -> Option<String> {
    let key = cache_key(latex, display, dark);
    if let Some(png) = std::fs::read(cache_path(key))
        .ok()
        .filter(|b| !b.is_empty())
    {
        return Some(encode(&png));
    }
    let png = render_png(latex, display, dark)?;
    if png.is_empty() {
        return None;
    }
    // A cache write that fails costs a re-render next time and nothing else, so it is never
    // allowed to fail the hover.
    let path = cache_path(key);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, &png);
    Some(encode(&png))
}

/// Content hash of everything that changes the pixels: the expression, whether it is display
/// math, the colour scheme it was drawn for, and the recipe version.
fn cache_key(latex: &str, display: bool, dark: bool) -> u64 {
    taliesin_core::hash::fnv1a(&format!("{RECIPE}\u{1}{display}\u{1}{dark}\u{1}{latex}"))
}

/// Where a rendered expression lives between sessions. A user-level cache rather than the
/// project's `_freeze/`: a hover is about the author's machine, not the document's build, and
/// the same `\alpha` should not be re-drawn once per project.
fn cache_path(key: u64) -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("taliesin")
        .join("math")
        .join(format!("{key:016x}.png"))
}

/// Base64, so the PNG can be inlined. Same encoder as `crates/core/build.rs` uses to inline
/// the KaTeX fonts; hand-rolled to keep a dependency out of the tree for 20 lines.
fn encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4 + 22);
    s.push_str("data:image/png;base64,");
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        s.push(T[(n >> 18 & 63) as usize] as char);
        s.push(T[(n >> 12 & 63) as usize] as char);
        s.push(if chunk.len() > 1 {
            T[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        s.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    s
}

/// The self-contained page a browser is pointed at: the real KaTeX markup, the real
/// fonts-inlined stylesheet, and nothing else. No network, no sidecar files.
///
/// The background stays transparent so the hover's own colour shows through, which means the
/// foreground has to be chosen for the reader's scheme — an image cannot inherit the editor's
/// text colour the way the Unicode preview did.
#[cfg(any(feature = "headless-js", test))]
fn page_html(latex: &str, display: bool, dark: bool) -> String {
    let math = taliesin_core::math::render(latex, display);
    let fg = if dark { "#e4e4e4" } else { "#1a1a1a" };
    format!(
        "<!doctype html><meta charset=\"utf-8\"><style>{css}\n\
         html,body{{margin:0;padding:0;background:transparent}}\n\
         #tali-math{{display:inline-block;padding:6px 8px;color:{fg};font-size:17px;\
         background:transparent}}\n\
         #tali-math .katex{{color:{fg}}}\n\
         </style><div id=\"tali-math\">{math}</div>",
        css = taliesin_core::render::katex_css(),
    )
}

/// KaTeX could not parse it, so there is nothing faithful to draw. Detected the same way
/// `math_preview` detects it: a failed parse yields a `katex-error` span carrying no `<math>`
/// element at all.
#[cfg(any(feature = "headless-js", test))]
fn unparsed(latex: &str, display: bool) -> bool {
    !taliesin_core::math::render(latex, display).contains("<math")
}

#[cfg(not(feature = "headless-js"))]
fn render_png(_latex: &str, _display: bool, _dark: bool) -> Option<Vec<u8>> {
    // Built without a browser driver. `read --run-js` degrades the same way.
    None
}

#[cfg(feature = "headless-js")]
fn render_png(latex: &str, display: bool, dark: bool) -> Option<Vec<u8>> {
    if unparsed(latex, display) {
        return None;
    }
    let dir = std::env::temp_dir().join(format!(
        "tali-math-{}-{:016x}",
        std::process::id(),
        cache_key(latex, display, dark)
    ));
    std::fs::create_dir_all(&dir).ok()?;
    let page = dir.join("math.html");
    let wrote = std::fs::write(&page, page_html(latex, display, dark)).is_ok();
    let png = if wrote {
        let tag = cache_key(latex, display, dark);
        runtime()?.block_on(async {
            tokio::time::timeout(render_timeout(), shoot(&page, tag))
                .await
                .ok()
                .flatten()
        })
    } else {
        None
    };
    let _ = std::fs::remove_dir_all(&dir);
    png
}

/// One runtime for every render, built lazily. The LSP main loop is blocking (`lsp-server`
/// owns the thread), so the async driver has to be entered per call rather than wrapping it.
#[cfg(feature = "headless-js")]
fn runtime() -> Option<&'static tokio::runtime::Runtime> {
    use std::sync::OnceLock;
    static RT: OnceLock<Option<tokio::runtime::Runtime>> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().ok())
        .as_ref()
}

/// Launch a throwaway headless Chrome, screenshot the one element, tear it all down.
///
/// The launch flags mirror [`crate::headless_js`] deliberately, including `--no-sandbox` and
/// the private profile: the page is a `file://` document this process just wrote from the
/// author's own source, with no network and no third-party origin, and the browser is killed
/// at the end of the call.
#[cfg(feature = "headless-js")]
async fn shoot(page_path: &std::path::Path, tag: u64) -> Option<Vec<u8>> {
    use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotFormat, Viewport};
    use chromiumoxide::page::ScreenshotParams;
    use chromiumoxide::{Browser, BrowserConfig};
    use futures::StreamExt;

    let exe = crate::headless_js::chrome_path()?;
    // Per-render, not per-process: two browsers sharing one profile directory is a launch
    // failure, and nothing here guarantees renders never overlap.
    let profile = std::env::temp_dir().join(format!(
        "tali-math-profile-{}-{tag:016x}",
        std::process::id()
    ));
    let config = BrowserConfig::builder()
        .chrome_executable(&exe)
        .new_headless_mode()
        .no_sandbox()
        .user_data_dir(&profile)
        .window_size(1200, 400)
        .args(vec![
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--hide-scrollbars",
            "--disable-extensions",
        ])
        .build()
        .ok()?;
    let (mut browser, mut handler) = Browser::launch(config).await.ok()?;
    let driver = tokio::spawn(async move { while handler.next().await.is_some() {} });

    let shot = async {
        let page = browser
            .new_page(format!("file://{}", page_path.display()))
            .await
            .ok()?;
        page.wait_for_navigation().await.ok()?;
        // The clip has to come from the laid-out element: KaTeX sizes itself, and a fixed
        // viewport would either crop a wide integral or pad a lone `\alpha` with dead space
        // that reads as a broken image in the hover.
        let rect: (f64, f64, f64, f64) = page
            .evaluate(
                "(() => { const r = document.getElementById('tali-math')\
                 .getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; })()",
            )
            .await
            .ok()?
            .into_value()
            .ok()?;
        if rect.2 <= 0.0 || rect.3 <= 0.0 {
            return None;
        }
        page.screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .clip(Viewport {
                    x: rect.0,
                    y: rect.1,
                    width: rect.2,
                    height: rect.3,
                    scale: 1.0,
                })
                // The hover paints its own background; ours must not cover it.
                .omit_background(true)
                .capture_beyond_viewport(true)
                .build(),
        )
        .await
        .ok()
    }
    .await;

    let _ = browser.close().await;
    let _ = browser.wait().await;
    driver.abort();
    let _ = std::fs::remove_dir_all(&profile);
    shot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cache_key_separates_everything_that_changes_the_pixels() {
        // A key that ignored any of these would serve a light-theme image into a dark hover,
        // or an inline render where display math was asked for.
        let base = cache_key("x^2", false, true);
        assert_ne!(base, cache_key("x^3", false, true), "the expression");
        assert_ne!(base, cache_key("x^2", true, true), "display vs inline");
        assert_ne!(base, cache_key("x^2", false, false), "the colour scheme");
        assert_eq!(base, cache_key("x^2", false, true), "and is deterministic");
    }

    #[test]
    fn the_data_uri_is_a_png_the_markdown_sanitizer_will_keep() {
        // `data:image/png;base64,` is the exact shape VS Code's `allowedMediaProtocols`
        // admits; a bare base64 blob or an `image/svg+xml` would be dropped silently.
        let uri = encode(&[0x89, b'P', b'N', b'G']);
        assert!(uri.starts_with("data:image/png;base64,"), "{uri}");
        assert_eq!(uri, "data:image/png;base64,iVBORw==");
    }

    #[test]
    fn the_page_carries_the_real_stylesheet_and_the_real_markup() {
        // The whole claim of this module is that the hover shows what the DOCUMENT shows, so
        // the page must be built from the engine's own output, not a lookalike.
        let html = page_html("\\frac{a}{b}", false, true);
        assert!(html.contains("katex"), "the real KaTeX markup");
        assert!(
            html.contains("data:font/woff2;base64,"),
            "fonts inlined, no network"
        );
        assert!(
            html.contains("background:transparent"),
            "the hover paints the background"
        );
    }

    /// The canary for the whole feature: a real browser, a real KaTeX page, a real PNG.
    ///
    /// Named so `tools/gates.sh` can assert it printed `... ok` — every other test here
    /// checks a string, and would stay green if rasterizing had stopped working entirely.
    #[cfg(feature = "headless-js")]
    #[test]
    fn a_real_browser_rasterizes_real_katex_into_a_data_uri() {
        if crate::headless_js::chrome_path().is_none() {
            assert!(
                std::env::var("TALIESIN_REQUIRE_CHROME").is_err(),
                "TALIESIN_REQUIRE_CHROME is set but no system Chrome was found"
            );
            eprintln!("no system Chrome: skipping");
            return;
        }
        // Not cached: a unique expression per run would defeat the cache, so instead the
        // cached copy is removed first — otherwise this passes on a file another run wrote
        // and proves nothing about the browser.
        let (latex, display, dark) = ("\\frac{a+1}{b}", false, true);
        let _ = std::fs::remove_file(cache_path(cache_key(latex, display, dark)));

        let uri = data_uri(latex, display, dark).expect("a browser render");
        assert!(
            uri.starts_with("data:image/png;base64,"),
            "shape: {}",
            &uri[..40]
        );
        // A PNG's first bytes are \x89PNG, which base64-encodes to `iVBORw0KGgo`. Checking
        // the payload rather than the prefix is what separates "we produced an image" from
        // "we produced an empty string with the right label".
        let payload = uri.trim_start_matches("data:image/png;base64,");
        assert!(
            payload.starts_with("iVBORw0KGgo"),
            "not a PNG: {}",
            &payload[..20]
        );
        assert!(
            payload.len() > 500,
            "suspiciously small render: {} b64 chars",
            payload.len()
        );

        // And the second call must come from disk, not a second browser launch.
        assert!(
            cache_path(cache_key(latex, display, dark)).exists(),
            "the render was cached"
        );
    }

    #[test]
    fn math_katex_cannot_parse_is_declined_rather_than_drawn() {
        // A broken expression is already squiggled as a diagnostic. Drawing KaTeX's error
        // span would contradict it with a picture.
        assert!(unparsed("\\frac{", false));
        assert!(!unparsed("\\frac{a}{b}", false));
    }
}
