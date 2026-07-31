//! `taliesin pdf`: a paged rendering of the built HTML (backlog 159).
//!
//! HTML stays the single source of truth. This drives a local headless Chrome over the
//! transient page `taliesin_core::render::print` assembles — it is a *rendering of* the build
//! artifact, never a second compiler target. The moment this forks into a Pandoc/Typst/LaTeX
//! path it has violated HTML-only; that is the line.
//!
//! **Why CDP and not `chrome --headless --print-to-pdf`.** Measured 2026-07-31 against Chrome
//! 150: the CLI truncates a paged.js document *deterministically at 2 pages* at every
//! `--virtual-time-budget` from 5 s to 120 s, and `--dump-dom` captures the page
//! mid-initialization — styles injected, zero page boxes. Neither flag waits for paged.js's
//! async chunking, and both fail by producing a plausible-looking short document rather than
//! an error. The completion signal has to be observed, which needs CDP.
//!
//! **Why the executor runs first.** `taliesin render` deliberately skips execution; a PDF is
//! an artifact you hand to someone, so shipping one with empty python/r figures would be a
//! far worse defect than it is for a one-shot stdout dump. This mirrors `build`'s single-file
//! sequence, which also means an already-built document replays from `_freeze` and never
//! boots a kernel.
#![cfg_attr(not(feature = "headless-js"), allow(unused_imports))]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use taliesin_core::render::print::Paper;

pub(crate) fn cmd_pdf(args: &[String]) -> ExitCode {
    let mut src: Option<String> = None;
    let mut out: Option<String> = None;
    let mut paper = Paper::default();
    let mut keep_html = false;

    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--out" => match it.next() {
                Some(v) => out = Some(v.clone()),
                None => {
                    crate::log::error("`--out` needs a path");
                    return ExitCode::FAILURE;
                }
            },
            "--paper" => match it.next() {
                Some(v) => match Paper::parse(v) {
                    Some(p) => paper = p,
                    None => {
                        crate::log::error(&format!(
                            "unknown paper size `{v}` (expected a4, letter or a5)"
                        ));
                        return ExitCode::FAILURE;
                    }
                },
                None => {
                    crate::log::error("`--paper` needs a size (a4, letter or a5)");
                    return ExitCode::FAILURE;
                }
            },
            "--keep-html" => keep_html = true,
            s if s.starts_with('-') => {
                crate::log::error(&format!("unknown flag `{s}`"));
                return ExitCode::FAILURE;
            }
            s if src.is_none() => src = Some(s.to_string()),
            s => {
                crate::log::error(&format!("unexpected extra argument `{s}`"));
                return ExitCode::FAILURE;
            }
        }
    }

    let Some(src) = src else {
        return crate::usage_error("pdf");
    };
    run(Path::new(&src), out.map(PathBuf::from), paper, keep_html)
}

/// Without the browser driver there is nothing to render with. Degrade the same way
/// `read --run-js` does: a named, actionable message, never a panic and never an empty file.
#[cfg(not(feature = "headless-js"))]
fn run(_src: &Path, _out: Option<PathBuf>, _paper: Paper, _keep_html: bool) -> ExitCode {
    crate::log::error(
        "`taliesin pdf` needs the browser driver, which this binary was built without. \
         Rebuild with `--features headless-js`. (Released binaries and the `taliesin` \
         launcher already enable it.)",
    );
    ExitCode::FAILURE
}

#[cfg(feature = "headless-js")]
fn run(src: &Path, out: Option<PathBuf>, paper: Paper, keep_html: bool) -> ExitCode {
    use taliesin_core::render::print::print_page_from_doc;

    if src.is_dir() {
        crate::log::error("pdf renders a single .tmd file; whole-book output is not supported yet");
        return ExitCode::FAILURE;
    }
    if crate::headless_js::chrome_path().is_none() {
        crate::log::error(
            "`taliesin pdf` needs a local Chrome to lay out the pages, and none was found. \
             Install Chrome/Chromium, or set CHROME_PATH to its binary.",
        );
        return ExitCode::FAILURE;
    }
    let Ok(source) = std::fs::read_to_string(src) else {
        crate::log::error(&format!("cannot read {}", src.display()));
        return ExitCode::FAILURE;
    };
    let out = out.unwrap_or_else(|| src.with_extension("pdf"));
    let base = src.parent().unwrap_or_else(|| Path::new("."));
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            crate::log::error(&format!("runtime: {e}"));
            return ExitCode::FAILURE;
        }
    };

    // Mirror `build`'s single-file sequence (build.rs), not `render`'s: cells must have run
    // or every python/r figure in the PDF is empty.
    let doc = rt.block_on(async {
        let mut doc = taliesin_core::render_single_doc(&source, base);
        let mut ex = crate::exec::Executor::with_freeze(crate::freeze::page_path(
            &base.join("_freeze"),
            stem,
        ))
        .in_dir(base);
        ex.set_interpreters(
            crate::interpreter::resolve_python(None, base),
            crate::interpreter::resolve_r(None, base),
        );
        doc.blocks = ex.run(std::mem::take(&mut doc.blocks)).await;
        doc
    });

    let html = print_page_from_doc(&doc, stem, paper);

    // The paginated page is transient: it is not an output of this tool, it is the thing the
    // browser reads. `_site/` and the source tree are untouched.
    let dir = std::env::temp_dir().join(format!("tali-print-{}", std::process::id()));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        crate::log::error(&format!("temp dir: {e}"));
        return ExitCode::FAILURE;
    }
    let page = dir.join("print.html");
    if let Err(e) = std::fs::write(&page, &html) {
        crate::log::error(&format!("write print page: {e}"));
        let _ = std::fs::remove_dir_all(&dir);
        return ExitCode::FAILURE;
    }

    let result = rt.block_on(paginate_to_pdf(&page));

    if keep_html {
        crate::log::info(&format!("kept the paginated HTML at {}", page.display()));
    } else {
        let _ = std::fs::remove_dir_all(&dir);
    }

    match result {
        Ok(bytes) => match std::fs::write(&out, &bytes) {
            Ok(()) => {
                crate::log::info(&format!(
                    "wrote {} ({} KB)",
                    out.display(),
                    bytes.len() / 1024
                ));
                ExitCode::SUCCESS
            }
            Err(e) => {
                crate::log::error(&format!("write pdf: {e}"));
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            crate::log::error(&format!("pdf: {e}"));
            ExitCode::FAILURE
        }
    }
}

/// Navigate the paginated page, wait for paged.js to finish, then capture.
///
/// The body lives in [`capture_pdf`] rather than inline, mirroring `observe_inner`: it keeps
/// this a single brace-free expression, which is what lets
/// `headless_js::tests::every_browser_await_is_bounded` see that the `.await` below is on
/// `with_browser` (bounded by construction) rather than an anonymous unbounded await.
#[cfg(feature = "headless-js")]
async fn paginate_to_pdf(page_path: &Path) -> Result<Vec<u8>, String> {
    crate::headless_js::with_browser(async |b, p| capture_pdf(b, p, page_path).await).await
}

/// Drive one already-launched browser: navigate, wait for pagination, capture. Every phase
/// carries its own bound, which is the contract `with_browser` relies on.
#[cfg(feature = "headless-js")]
async fn capture_pdf(
    browser: &chromiumoxide::Browser,
    phase: std::time::Duration,
    page_path: &Path,
) -> Result<Vec<u8>, String> {
    use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;

    {
        let url = format!("file://{}", page_path.display());
        let page = tokio::time::timeout(phase, browser.new_page("about:blank"))
            .await
            .map_err(|_| "new page timed out".to_string())?
            .map_err(|e| format!("new page: {e}"))?;
        tokio::time::timeout(phase, page.goto(&url))
            .await
            .map_err(|_| "navigate timed out".to_string())?
            .map_err(|e| format!("navigate: {e}"))?;

        // Poll for the stamp rather than sleeping: chunking time scales with document
        // length, so any constant is either flaky on long documents or slow on short ones.
        let budget = crate::headless_js::settle_timeout();
        let done: bool = tokio::time::timeout(
            crate::headless_js::eval_timeout(budget),
            page.evaluate_function(wait_script(budget.as_millis() as u64)),
        )
        .await
        .map_err(|_| "pagination wait timed out".to_string())?
        .map_err(|e| format!("evaluate: {e}"))?
        .into_value()
        .map_err(|e| format!("decode: {e}"))?;
        if !done {
            return Err(format!(
                "paged.js did not finish within {} s (TALIESIN_JS_TIMEOUT raises it)",
                budget.as_secs()
            ));
        }

        let params = PrintToPdfParams::builder()
            // paged.js has already computed the page boxes; honour ITS size rather than
            // re-imposing one, or every page comes out cropped or letterboxed.
            .prefer_css_page_size(true)
            // paged.js draws running heads and folios INTO the content, so Chrome's own
            // header/footer would double them up.
            .display_header_footer(false)
            .print_background(true)
            .build();
        tokio::time::timeout(phase, page.pdf(params))
            .await
            .map_err(|_| "printToPDF timed out".to_string())?
            .map_err(|e| format!("printToPDF: {e}"))
    }
}

/// The in-page wait: resolve `true` once `<html>` carries the completion stamp, `false` on
/// the deadline. Returning a bool rather than hanging means a paged.js that dies mid-chunk
/// is reported as a failure instead of timing the whole command out.
///
/// Split out and pure so it is unit-testable without a browser.
#[cfg(feature = "headless-js")]
fn wait_script(budget_ms: u64) -> String {
    format!(
        "function () {{ return new Promise(function (resolve) {{ \
           var deadline = Date.now() + {budget_ms}; \
           (function poll() {{ \
             if (document.documentElement.getAttribute('{attr}') === 'done') {{ return resolve(true); }} \
             if (Date.now() > deadline) {{ return resolve(false); }} \
             setTimeout(poll, 50); \
           }})(); \
         }}); }}",
        attr = taliesin_core::render::print::PAGED_DONE_ATTR,
    )
}

#[cfg(all(test, feature = "headless-js"))]
mod tests {
    use super::*;

    /// The wait must key on the SAME attribute the assembler stamps. Hard-coding the string
    /// in either place would let them drift into a driver that waits forever for a stamp
    /// that never matches — a hang, not a failure.
    #[test]
    fn the_wait_script_polls_the_attribute_the_assembler_stamps() {
        let script = wait_script(1000);
        assert!(
            script.contains(taliesin_core::render::print::PAGED_DONE_ATTR),
            "the wait script must poll the assembler's completion attribute"
        );
        assert!(
            script.contains("resolve(false)"),
            "the script must resolve false on deadline, not hang"
        );
        assert!(
            script.contains("1000"),
            "the budget must reach the in-page deadline"
        );
    }
}
