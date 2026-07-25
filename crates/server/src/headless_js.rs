//! Headless-Chrome observation of `{js}` cells for `read --run` (DX17b).
//!
//! **Why:** a `{python}`/`{r}` cell runs on a kernel, so `build`/`read --run` already sees
//! its output and DX17(a) can report `[figure: produced]`. A `{js}` cell (Observable Plot,
//! the corpus's own idiom) runs *in the browser*, so nothing server-side ever sees whether
//! its chart painted — a headless agent driving `read --run` is blind to it. This module
//! drives a **local, system-provided** headless Chrome over the built page the way the cell
//! actually runs, and reports per cell whether an `<svg>`/`<canvas>` appeared.
//!
//! **Invariants held:** observation-only (no input, no re-run, no `_freeze` write — the CUT
//! reactive-VM `js-kernel-rerun` trap stays out); offline (a local `file://` page whose
//! assets are already inlined + a local browser, no network — the browser-download `fetcher`
//! feature is off); gated + optional (no Chrome, or a launch/timeout failure, degrades every
//! cell to a `Skipped` outcome, never a hard error). The classifier ([`classify_js_node`]) is
//! pure over the DOM facts the in-page snippet returns, so the mapping is unit-tested with no
//! Chrome; a `TALIESIN_REQUIRE_CHROME`-gated integration test proves the live loop.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How a `{js}` cell's browser output classified. Server-computed: a `{js}` cell runs in the
/// browser, so unlike python/r there is no server-side output block for
/// [`taliesin_core::classify_exec_output`] to read — the facts come from the live DOM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JsOutcome {
    /// The cell painted output. `w`/`h` are the rounded CSS-pixel dimensions of the
    /// `<svg>`/`<canvas>` (both `0` for a generic HTML widget with no single visual node).
    Produced { kind: JsKind, w: i64, h: i64 },
    /// The cell threw; the string is the first line of the browser error (the message).
    Error(String),
    /// The cell ran but painted no DOM — a `//| name:` value publisher or `//| input:`
    /// effect legitimately produces nothing, so this is not a failure.
    Empty,
    /// Not observed: no system Chrome, a launch/navigation/eval failure, or this cell never
    /// finished within the settle budget. The reason is agent-readable.
    Skipped(String),
}

/// The kind of visual node a produced `{js}` cell painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsKind {
    Svg,
    Canvas,
    /// Some other element output (an `Inputs.table`, a bare `<div>` widget, …).
    Html,
}

impl JsKind {
    fn label(self) -> &'static str {
        match self {
            JsKind::Svg => "svg",
            JsKind::Canvas => "canvas",
            JsKind::Html => "html",
        }
    }
}

/// The raw DOM facts the in-page snippet returns for one `{js}` cell. Only meaningful when
/// `done` (a cell whose script never stamped `data-tali-done` is reported timed-out, not
/// classified from a half-run DOM).
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct JsNode {
    #[serde(rename = "blockId")]
    pub block_id: String,
    pub done: bool,
    /// The `.tali-js-error` text, when the cell threw (exclusive of the produced facts — the
    /// enhancer replaces the output with the error `<pre>`).
    pub error: Option<String>,
    #[serde(rename = "hasSvg")]
    pub has_svg: bool,
    #[serde(rename = "hasCanvas")]
    pub has_canvas: bool,
    /// Any element child of the output container (the generic-HTML fallback).
    #[serde(rename = "hasOther")]
    pub has_other: bool,
    pub w: i64,
    pub h: i64,
}

/// Map a cell's observed DOM facts to an outcome. **Pure** — the real, un-rottable coverage
/// (no Chrome needed to test the mapping). Priority: a cell that never finished is
/// timed-out; an error is exclusive; then `<svg>` beats `<canvas>` beats generic HTML;
/// nothing painted is `Empty`.
pub(crate) fn classify_js_node(node: &JsNode) -> JsOutcome {
    if !node.done {
        return JsOutcome::Skipped("timed out".to_string());
    }
    if let Some(err) = &node.error {
        let summary = err
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("");
        return JsOutcome::Error(if summary.is_empty() {
            "unknown error".to_string()
        } else {
            summary.to_string()
        });
    }
    if node.has_svg {
        return JsOutcome::Produced {
            kind: JsKind::Svg,
            w: node.w,
            h: node.h,
        };
    }
    if node.has_canvas {
        return JsOutcome::Produced {
            kind: JsKind::Canvas,
            w: node.w,
            h: node.h,
        };
    }
    if node.has_other {
        return JsOutcome::Produced {
            kind: JsKind::Html,
            w: 0,
            h: 0,
        };
    }
    JsOutcome::Empty
}

impl JsOutcome {
    /// The `[js: …]` line appended after a `{js}` cell's source in the `read` text
    /// projection, mirroring python/r's `[figure: produced]` / `[cell error: …]` lines.
    pub(crate) fn text_line(&self) -> String {
        match self {
            JsOutcome::Produced {
                kind: JsKind::Html, ..
            } => "[js: produced]".to_string(),
            JsOutcome::Produced { kind, w, h } => {
                format!("[js: produced, <{} {w}×{h}>]", kind.label())
            }
            JsOutcome::Error(msg) => format!("[js error: {msg}]"),
            JsOutcome::Empty => "[js: no visible output]".to_string(),
            JsOutcome::Skipped(why) => format!("[js: skipped ({why})]"),
        }
    }

    /// The JSON `kind` for `read --format json`. Distinct from python/r's kinds so an agent
    /// can tell a browser-observed cell apart.
    pub(crate) fn json_kind(&self) -> &'static str {
        match self {
            JsOutcome::Produced { .. } => "js",
            JsOutcome::Error(_) => "js-error",
            JsOutcome::Empty => "js-empty",
            JsOutcome::Skipped(_) => "skipped",
        }
    }

    /// Whether the cell painted visible output (the JSON `produced` flag).
    pub(crate) fn produced(&self) -> bool {
        matches!(self, JsOutcome::Produced { .. })
    }

    /// The JSON `detail`: the node kind + dims for a produced cell, the skip reason for a
    /// skip, `None` otherwise (an error's message rides the `error` field instead).
    pub(crate) fn detail(&self) -> Option<String> {
        match self {
            JsOutcome::Produced {
                kind: JsKind::Html, ..
            } => Some("html".to_string()),
            JsOutcome::Produced { kind, w, h } => Some(format!("{} {w}×{h}", kind.label())),
            JsOutcome::Skipped(why) => Some(why.clone()),
            JsOutcome::Empty | JsOutcome::Error(_) => None,
        }
    }

    /// The JSON `error` message for an errored cell.
    pub(crate) fn error(&self) -> Option<String> {
        match self {
            JsOutcome::Error(m) => Some(m.clone()),
            _ => None,
        }
    }
}

/// The system Chrome to drive: `$CHROME_PATH` (an explicit but non-existent value means
/// "skip", so the negative test can force a Chrome-less run), else the first of these on
/// `PATH`. Mirrors `tools/ui-audit/lib/browser.mjs`'s candidate list.
pub(crate) fn chrome_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CHROME_PATH") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    const CANDIDATES: &[&str] = &[
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
    ];
    CANDIDATES.iter().find_map(|name| which_on_path(name))
}

/// Whether a local headless Chrome is available (drives the gate: no Chrome → every `{js}`
/// cell reports `skipped (chrome unavailable)`, python/r-only users never touch this path).
pub(crate) fn chrome_available() -> bool {
    chrome_path().is_some()
}

/// First executable named `name` on `$PATH` (a tiny `which`, so no `which` crate is pulled).
fn which_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

/// Per-page settle budget for `{js}` observation. `TALIESIN_JS_TIMEOUT` (seconds; `0`/unset
/// → default 10) is its own knob so a long `TALIESIN_CELL_TIMEOUT` python budget doesn't
/// bloat the browser wait.
fn settle_timeout() -> Duration {
    let secs = std::env::var("TALIESIN_JS_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&s| s > 0)
        .unwrap_or(10);
    Duration::from_secs(secs)
}

/// Observe every `{js}` cell in a built page, returning `block_id → outcome` for exactly the
/// requested `cell_ids`. Never errors to the caller: any launch/navigation/eval failure
/// degrades the whole set to `Skipped(reason)`, and a cell the browser didn't surface (e.g.
/// `include: false`) reports `Skipped("not observed")`. Observation-only.
pub(crate) async fn observe_js_cells(
    page_path: &Path,
    cell_ids: &[String],
) -> HashMap<String, JsOutcome> {
    match observe_inner(page_path).await {
        Ok(observed) => cell_ids
            .iter()
            .map(|id| {
                let outcome = observed
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| JsOutcome::Skipped("not observed".to_string()));
                (id.clone(), outcome)
            })
            .collect(),
        Err(reason) => cell_ids
            .iter()
            .map(|id| (id.clone(), JsOutcome::Skipped(reason.clone())))
            .collect(),
    }
}

/// Launch a throwaway headless Chrome (its own temp profile, so it never collides with a
/// dev/MCP Chrome), observe, then always tear the browser + profile down.
async fn observe_inner(page_path: &Path) -> Result<HashMap<String, JsOutcome>, String> {
    use chromiumoxide::{Browser, BrowserConfig};
    use futures::StreamExt;

    let exe = chrome_path().ok_or_else(|| "chrome unavailable".to_string())?;
    let profile = unique_profile_dir();
    let config = BrowserConfig::builder()
        .chrome_executable(&exe)
        .new_headless_mode()
        .no_sandbox()
        .user_data_dir(&profile)
        .window_size(1280, 900)
        .args(vec![
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--hide-scrollbars",
            "--disable-extensions",
        ])
        .build()
        .map_err(|e| format!("chrome config: {e}"))?;

    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| format!("chrome launch failed: {e}"))?;
    let handler_task = tokio::spawn(async move { while handler.next().await.is_some() {} });

    let result = observe_page(&browser, page_path).await;

    // Tear down regardless of the observation result.
    let _ = browser.close().await;
    let _ = browser.wait().await;
    handler_task.abort();
    let _ = std::fs::remove_dir_all(&profile);
    result
}

/// Open the built page over `file://`, wait for every `{js}` cell to settle, and classify.
async fn observe_page(
    browser: &chromiumoxide::Browser,
    page_path: &Path,
) -> Result<HashMap<String, JsOutcome>, String> {
    let url = format!("file://{}", page_path.display());
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| format!("new page: {e}"))?;
    // Flip qmd-js.js into full-error mode for THIS observation only: a built page hides the
    // real error behind a terse reader message unless `window.taliOpenPageSource` is defined
    // (the live preview defines it for real). A no-op has no other effect in a built page,
    // and gives the agent the actual error instead of "couldn't load".
    let _ = page
        .evaluate_on_new_document("window.taliOpenPageSource = function () {};")
        .await;
    page.goto(&url)
        .await
        .map_err(|e| format!("navigate: {e}"))?;

    let budget = settle_timeout();
    let script = build_observe_script(budget.as_millis() as u64);
    // An outer wall-clock timeout in case the in-page wait/eval never returns (a crashed
    // page): the settle budget plus slack, so `read --run` can never hang.
    let eval = tokio::time::timeout(
        budget + Duration::from_secs(5),
        page.evaluate_function(script),
    )
    .await;
    let nodes: Vec<JsNode> = match eval {
        Ok(Ok(res)) => res.into_value().map_err(|e| format!("decode: {e}"))?,
        Ok(Err(e)) => return Err(format!("evaluate: {e}")),
        Err(_) => return Err("timed out".to_string()),
    };
    Ok(nodes
        .into_iter()
        .map(|n| (n.block_id.clone(), classify_js_node(&n)))
        .collect())
}

/// A unique temp user-data dir, `tali-headless-<pid>_<uuid>`. Distinct prefix from the
/// kernel/warm-pool dirs so the startup sweep leaves it alone; `observe_inner` removes it.
fn unique_profile_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "tali-headless-{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

/// The in-page async snippet: wait until every `application/qmd-js` script is stamped
/// `data-tali-done` (or the deadline), then return one facts object per cell. Keyed off the
/// script's `data-target` (`qmd-js-<block_id>`), so it joins to the block model for both a
/// plain `{js}` cell and a numbered `{js}` figure (both emit the same script + target div).
fn build_observe_script(deadline_ms: u64) -> String {
    format!(
        r#"async function () {{
  const deadline = {deadline_ms};
  const clock = () => ((self.performance && performance.now) ? performance.now() : Date.now());
  const start = clock();
  const scripts = () => Array.prototype.slice.call(
    document.querySelectorAll('script[type="application/qmd-js"]'));
  while (clock() - start < deadline) {{
    const all = scripts();
    if (all.length === 0) break;
    if (all.every(s => s.hasAttribute('data-tali-done'))) break;
    await new Promise(r => setTimeout(r, 50));
  }}
  return scripts().map(function (s) {{
    const target = s.getAttribute('data-target') || '';
    const blockId = target.replace(/^qmd-js-/, '');
    const out = target ? document.getElementById(target) : null;
    const errEl = out ? out.querySelector('.tali-js-error') : null;
    const svg = out ? out.querySelector('svg') : null;
    const canvas = out ? out.querySelector('canvas') : null;
    let w = 0, h = 0;
    const dim = svg || canvas;
    if (dim && dim.getBoundingClientRect) {{
      const r = dim.getBoundingClientRect();
      w = Math.round(r.width); h = Math.round(r.height);
    }}
    return {{
      blockId: blockId,
      done: s.hasAttribute('data-tali-done'),
      error: errEl ? (errEl.textContent || '').trim() : null,
      hasSvg: !!svg, hasCanvas: !!canvas,
      hasOther: out ? out.childElementCount > 0 : false,
      w: w, h: h
    }};
  }});
}}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(done: bool) -> JsNode {
        JsNode {
            block_id: "b-1".to_string(),
            done,
            error: None,
            has_svg: false,
            has_canvas: false,
            has_other: false,
            w: 0,
            h: 0,
        }
    }

    #[test]
    fn svg_classifies_as_produced_with_dims() {
        let n = JsNode {
            has_svg: true,
            w: 640,
            h: 400,
            ..node(true)
        };
        assert_eq!(
            classify_js_node(&n),
            JsOutcome::Produced {
                kind: JsKind::Svg,
                w: 640,
                h: 400
            }
        );
    }

    #[test]
    fn canvas_classifies_as_produced_canvas() {
        let n = JsNode {
            has_canvas: true,
            w: 800,
            h: 600,
            ..node(true)
        };
        assert_eq!(
            classify_js_node(&n),
            JsOutcome::Produced {
                kind: JsKind::Canvas,
                w: 800,
                h: 600
            }
        );
    }

    #[test]
    fn svg_wins_over_a_coexisting_other_child() {
        // The output div always has element children when it has an svg; svg must win.
        let n = JsNode {
            has_svg: true,
            has_other: true,
            w: 100,
            h: 50,
            ..node(true)
        };
        assert!(matches!(
            classify_js_node(&n),
            JsOutcome::Produced {
                kind: JsKind::Svg,
                ..
            }
        ));
    }

    #[test]
    fn error_is_exclusive_and_summarizes_first_line() {
        let n = JsNode {
            error: Some("ReferenceError: Plot is not defined\n    at <anonymous>:1:1".to_string()),
            // Even if the DOM somehow reports children, an error wins.
            has_other: true,
            ..node(true)
        };
        assert_eq!(
            classify_js_node(&n),
            JsOutcome::Error("ReferenceError: Plot is not defined".to_string())
        );
    }

    #[test]
    fn blank_error_text_falls_back_to_unknown() {
        let n = JsNode {
            error: Some("   \n  ".to_string()),
            ..node(true)
        };
        assert_eq!(
            classify_js_node(&n),
            JsOutcome::Error("unknown error".to_string())
        );
    }

    #[test]
    fn nothing_painted_is_empty_not_produced() {
        assert_eq!(classify_js_node(&node(true)), JsOutcome::Empty);
    }

    #[test]
    fn other_child_only_is_generic_html() {
        let n = JsNode {
            has_other: true,
            ..node(true)
        };
        assert_eq!(
            classify_js_node(&n),
            JsOutcome::Produced {
                kind: JsKind::Html,
                w: 0,
                h: 0
            }
        );
    }

    #[test]
    fn a_cell_that_never_finished_is_timed_out() {
        // `!done` must not fall through to `Empty` (that would hide a hung cell as "ran,
        // painted nothing").
        let n = JsNode {
            has_svg: true, // even with facts present, an unfinished cell is not trusted
            ..node(false)
        };
        assert_eq!(
            classify_js_node(&n),
            JsOutcome::Skipped("timed out".to_string())
        );
    }

    #[test]
    fn text_lines_read_naturally_per_variant() {
        assert_eq!(
            JsOutcome::Produced {
                kind: JsKind::Svg,
                w: 640,
                h: 400
            }
            .text_line(),
            "[js: produced, <svg 640×400>]"
        );
        assert_eq!(
            JsOutcome::Produced {
                kind: JsKind::Canvas,
                w: 800,
                h: 600
            }
            .text_line(),
            "[js: produced, <canvas 800×600>]"
        );
        assert_eq!(
            JsOutcome::Produced {
                kind: JsKind::Html,
                w: 0,
                h: 0
            }
            .text_line(),
            "[js: produced]"
        );
        assert_eq!(
            JsOutcome::Error("boom".to_string()).text_line(),
            "[js error: boom]"
        );
        assert_eq!(JsOutcome::Empty.text_line(), "[js: no visible output]");
        assert_eq!(
            JsOutcome::Skipped("chrome unavailable".to_string()).text_line(),
            "[js: skipped (chrome unavailable)]"
        );
    }

    #[test]
    fn json_projection_fields_match_the_outcome() {
        let produced = JsOutcome::Produced {
            kind: JsKind::Svg,
            w: 640,
            h: 400,
        };
        assert_eq!(produced.json_kind(), "js");
        assert!(produced.produced());
        assert_eq!(produced.detail().as_deref(), Some("svg 640×400"));
        assert!(produced.error().is_none());

        let err = JsOutcome::Error("TypeError: x".to_string());
        assert_eq!(err.json_kind(), "js-error");
        assert!(!err.produced());
        assert!(err.detail().is_none());
        assert_eq!(err.error().as_deref(), Some("TypeError: x"));

        let skipped = JsOutcome::Skipped("chrome unavailable".to_string());
        assert_eq!(skipped.json_kind(), "skipped");
        assert!(!skipped.produced());
        assert_eq!(skipped.detail().as_deref(), Some("chrome unavailable"));

        let empty = JsOutcome::Empty;
        assert_eq!(empty.json_kind(), "js-empty");
        assert!(!empty.produced());
        assert!(empty.detail().is_none());
    }

    #[test]
    fn chrome_path_skips_an_explicit_nonexistent_binary() {
        // The negative integration case relies on this: CHROME_PATH set but missing → skip,
        // NOT a fall back to a real PATH Chrome. Uses a path that cannot exist.
        // SAFETY: single-threaded test; restore the prior value after.
        let prev = std::env::var_os("CHROME_PATH");
        unsafe { std::env::set_var("CHROME_PATH", "/nonexistent/definitely-not-chrome") };
        assert!(chrome_path().is_none());
        match prev {
            Some(v) => unsafe { std::env::set_var("CHROME_PATH", v) },
            None => unsafe { std::env::remove_var("CHROME_PATH") },
        }
    }

    #[test]
    fn observe_script_joins_on_data_target_and_waits_on_done() {
        let s = build_observe_script(10_000);
        assert!(s.contains("application/qmd-js"), "selects js cell scripts");
        assert!(s.contains("data-tali-done"), "waits on the settle signal");
        assert!(
            s.contains("qmd-js-") && s.contains("data-target"),
            "derives block id from the script's data-target"
        );
        assert!(
            !s.contains("data-tali-ran"),
            "must NOT gate on data-tali-ran (the ui-audit false-settle lesson)"
        );
    }
}
