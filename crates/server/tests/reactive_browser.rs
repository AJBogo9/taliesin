//! A browser test of the explorable cluster: the cell-language registry and `{glsl}` (153),
//! the `num` global (154), `tali.state` (156), and `tali.tex`/`tali.table` (157). (Item
//! 155's `animate` tick and draggable `point` were retired on 2026-08-03; `tali.state`,
//! which shared their fixture, is driven from a slider here instead.)
//!
//! **Why a browser test and not more emission tests.** Every one of those five items is
//! mostly JavaScript. The Rust suite can prove a `<script type="application/tali-glsl">`
//! reaches the page; it cannot see whether the shader compiled, whether `tali.state`
//! survives a re-run, or whether `num.gaussian.pdf` returns the right number. Those are the claims
//! the features actually make, and without this file all of them would be untested — the
//! exact hole `deck_browser.rs` was written to close for `deck.js`.
//!
//! Everything below is read from the **live** DOM after the page settles. Nothing
//! re-derives what the runtime should have done, and every negative assertion is paired
//! with a known-positive reading, because a probe whose every cell is negative is a broken
//! probe until proven otherwise.
//!
//! Gated exactly like `deck_browser.rs` and `read_run_js.rs`: no system Chrome → skip,
//! unless `TALIESIN_REQUIRE_CHROME=1` turns the skip into a hard failure. One browser run
//! serves every test here (a `OnceLock`).
//!
//! `api.publish` (the asynchronous-value hook) is NOT exercised here, and deliberately so:
//! no language in this file produces an asynchronous value. Its only driver was `{pyodide}`,
//! withdrawn along with its runtime, so the hook is currently unexercised. A test asserting
//! it exists without a language driving it would pass with `scheduleFrom` deleted.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use chromiumoxide::cdp::browser_protocol::input::{
    DispatchKeyEventParams, DispatchKeyEventType, DispatchMouseEventParams, DispatchMouseEventType,
    MouseButton,
};
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;

// ---------------------------------------------------------------------------
// Chrome gate (mirrors deck_browser.rs, which mirrors headless_js::chrome_path)
// ---------------------------------------------------------------------------

fn which_chrome() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CHROME_PATH") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    for name in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
    ] {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

fn have_chrome() -> bool {
    if which_chrome().is_some() {
        return true;
    }
    assert!(
        std::env::var_os("TALIESIN_REQUIRE_CHROME").is_none(),
        "TALIESIN_REQUIRE_CHROME=1 but no system Chrome found: the whole reactive client \
         would go untested"
    );
    eprintln!("skipping: no system Chrome (set CHROME_PATH or install google-chrome/chromium)");
    false
}

// ---------------------------------------------------------------------------
// what one run observed
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize)]
struct Numerics {
    /// `num.gaussian.pdf(0, 0, 1)`, whose closed form is 1/sqrt(2π).
    #[serde(rename = "gaussPdf0")]
    gauss_pdf0: f64,
    /// `num.gaussian.cdf(1.96, 0, 1)`, the textbook 0.975.
    #[serde(rename = "gaussCdf196")]
    gauss_cdf196: f64,
    /// `num.beta.cdf(0.5, 2, 2)`, exactly 0.5 by symmetry.
    #[serde(rename = "betaCdfHalf")]
    beta_cdf_half: f64,
    /// `num.gamma.cdf(1, 1, 1)` = 1 − e⁻¹.
    #[serde(rename = "gammaCdf1")]
    gamma_cdf1: f64,
    /// `num.poisson.cdf(2, 2)`, summed by hand as e⁻²(1 + 2 + 2).
    #[serde(rename = "poissonCdf2")]
    poisson_cdf2: f64,
    /// The sample mean of 4,000 seeded normals: near 0, and it must be the SAME value on
    /// a second generator built from the same seed.
    #[serde(rename = "seededMean")]
    seeded_mean: f64,
    /// Whether two generators with the same seed produced identical streams. The whole
    /// reason the PRNG is seeded rather than `Math.random`.
    #[serde(rename = "seedIsReproducible")]
    seed_is_reproducible: bool,
    /// Whether two generators with DIFFERENT seeds diverged. The control on the row above:
    /// a generator that ignored its seed entirely would satisfy reproducibility too.
    #[serde(rename = "seedsDiffer")]
    seeds_differ: bool,
    /// max |L Lᵀ − A| for a Cholesky of a 2×2 covariance: the factor must reconstruct it.
    #[serde(rename = "cholErr")]
    chol_err: f64,
    /// max |A A⁻¹ − I| for `inv2`.
    #[serde(rename = "invErr")]
    inv_err: f64,
    /// The larger eigenvalue of `[[2, .8], [.8, 1]]`, whose closed form is
    /// (3 + sqrt(1 + 4·0.64))/2.
    #[serde(rename = "eigTop")]
    eig_top: f64,
    /// Whether `num.cholesky` rejects a non-positive-definite matrix instead of returning
    /// NaNs that would silently poison a plot.
    #[serde(rename = "cholRejectsIndefinite")]
    chol_rejects_indefinite: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PageFacts {
    /// Every client-side cell on the page reported `data-tali-done`.
    #[serde(rename = "allCellsDone")]
    all_cells_done: bool,
    /// How many cells that was. The control on `all_cells_done`: zero cells are all done.
    cells: usize,
    /// `.tali-js-error` boxes currently on the page. Must be 0 on every corpus page here.
    errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GlslFacts {
    #[serde(flatten)]
    page: PageFacts,
    /// Canvases the `{glsl}` language mounted.
    canvases: usize,
    /// Whether the first canvas has a live WebGL context.
    #[serde(rename = "hasContext")]
    has_context: bool,
    /// Distinct pixel colours on the LEAST-varied canvas. A shader that failed to compile,
    /// or one whose uniforms never arrived, paints a single flat colour (or nothing); both
    /// corpus shaders paint structure, so this must be well above 1. Reporting the worst
    /// canvas rather than the first means one working shader cannot hide a broken sibling.
    #[serde(rename = "distinctColours")]
    distinct_colours: usize,
    /// Whether any pixel is non-transparent. Separates "drew one flat colour" from
    /// "never drew at all", which `distinct_colours == 1` alone cannot.
    painted: bool,
    /// Whether ANY canvas reported `isContextLost()`. A lost context cannot paint by
    /// definition, so a reading taken through one is **void, not negative** — see
    /// `read_glsl`, which retakes it rather than believing it.
    #[serde(rename = "contextLost")]
    context_lost: bool,
    /// Distinct colours on each canvas in document order, so a failure says *which*
    /// shader was blank. The animated one and the static one fail for different reasons
    /// and `[394, 1]` is a very different bug report from `[1, 1]`.
    #[serde(rename = "perCanvas")]
    per_canvas: Vec<usize>,
    /// How many sample rounds the probe needed, and how long that took. Not asserted on —
    /// it is the diagnostic that separates "ran out of time" from "read a genuinely blank
    /// canvas over and over", which is the distinction the 2026-07-30 flake turned on.
    rounds: usize,
    #[serde(rename = "waitedMs")]
    waited_ms: f64,
    /// How many page loads it took to get a reading through a live context. `> 1` means
    /// the environment lost a context and the probe retook the reading.
    #[serde(default)]
    attempts: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StateFacts {
    #[serde(flatten)]
    page: PageFacts,
    /// The accumulating cell's readout, e.g. "last frames: 0, 1, 2".
    #[serde(rename = "seenText")]
    seen_text: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TexFacts {
    #[serde(flatten)]
    page: PageFacts,
    /// `.tali-math` roots on the page.
    roots: usize,
    /// Cells in the first matrix's grid, and its column count from the inline grid style.
    #[serde(rename = "matrixCells")]
    matrix_cells: usize,
    #[serde(rename = "matrixCols")]
    matrix_cols: String,
    /// The rendered text of the first matrix, so the numbers can be checked and the
    /// U+2212 minus confirmed (a hyphen here is a typographic bug that reads as a dash).
    #[serde(rename = "matrixText")]
    matrix_text: String,
    /// `tali.table` output: header cells and body rows.
    #[serde(rename = "tableCols")]
    table_cols: Vec<String>,
    #[serde(rename = "tableRows")]
    table_rows: usize,
}

struct Run {
    glsl: GlslFacts,
    /// Animate: (at rest, after one Step, after ~a second of Play, after Reset).
    state: Vec<StateFacts>,
    /// Point: (initial, after ArrowRight×2 + ArrowUp, after a real mouse click at 25%/25%).
    tex: TexFacts,
    numerics: Numerics,
}

static RUN: OnceLock<Result<Run, String>> = OnceLock::new();

fn run() -> &'static Result<Run, String> {
    RUN.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("tali-reactive-browser-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;
        let out = tokio::runtime::Runtime::new()
            .map_err(|e| format!("tokio runtime: {e}"))?
            .block_on(drive(&dir));
        let _ = std::fs::remove_dir_all(&dir);
        out
    })
}

/// Build one corpus document into a standalone page.
///
/// **No `TALIESIN_NO_EXEC` here, unlike `deck_browser.rs`**: the subject IS the browser
/// execution. None of these four documents has a kernel cell, so the page is identical on
/// a laptop and in CI regardless.
fn build(dir: &Path, doc: &str) -> Result<PathBuf, String> {
    let src = format!(
        "{}/../../corpus/reactive/{doc}.tmd",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = dir.join(format!("{doc}.html"));
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", &src])
        .arg(&out)
        .output()
        .map_err(|e| format!("run build: {e}"))?;
    if !res.status.success() {
        return Err(format!(
            "build corpus/reactive/{doc}.tmd failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }
    out.exists()
        .then_some(out)
        .ok_or_else(|| format!("build of {doc} reported success but wrote no page"))
}

async fn drive(dir: &Path) -> Result<Run, String> {
    let exe = which_chrome().ok_or_else(|| "chrome unavailable".to_string())?;
    let profile =
        std::env::temp_dir().join(format!("tali-reactive-profile-{}", std::process::id()));
    let config = BrowserConfig::builder()
        .chrome_executable(&exe)
        .new_headless_mode()
        // Same reasoning as headless_js.rs: Chrome's own sandbox needs unprivileged user
        // namespaces and is unavailable in containers/CI. The page is a `file://` document
        // this repo just rendered from its own corpus.
        .no_sandbox()
        .user_data_dir(&profile)
        .window_size(1280, 900)
        .launch_timeout(Duration::from_secs(20))
        .request_timeout(Duration::from_secs(20))
        .args(vec![
            "--disable-dev-shm-usage",
            "--hide-scrollbars",
            "--disable-extensions",
            // WebGL, in software. Headless Chrome has no GPU, so `{glsl}` would report
            // "WebGL is unavailable" and this file's whole shader half would pass
            // vacuously on an error box. SwiftShader is the supported way to get a real
            // conformant GL context without one.
            "--use-gl=angle",
            "--use-angle=swiftshader",
            "--enable-unsafe-swiftshader",
        ])
        .build()
        .map_err(|e| format!("chrome config: {e}"))?;

    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| format!("chrome launch failed: {e}"))?;
    let handler_task = tokio::spawn(async move { while handler.next().await.is_some() {} });

    let result = observe(&browser, dir).await;

    let closed = tokio::time::timeout(Duration::from_secs(20), browser.close())
        .await
        .is_ok();
    let exited = closed
        && tokio::time::timeout(Duration::from_secs(20), browser.wait())
            .await
            .is_ok();
    if !exited {
        let _ = tokio::time::timeout(Duration::from_secs(20), browser.kill()).await;
    }
    handler_task.abort();
    let _ = std::fs::remove_dir_all(&profile);
    result
}

/// How many page loads the shader reading gets before the environment is blamed out loud.
/// At the measured ~30% context-loss rate under load ≈ 60 this leaves a ~0.8% chance of a
/// spurious failure, against ~30% for a single load; on a quiet machine it never retries.
const GLSL_ATTEMPTS: usize = 4;

/// The shader reading, retaken when the environment throws it away (item 179).
///
/// **This is not a loosened assertion, and the distinction is the whole design.** A lost
/// WebGL context cannot paint, so pixels read through one are not evidence *against* the
/// shader — they are no evidence at all, the same way a kernel that never started says
/// nothing about a cell. `Kernel::start_with_retry` is the existing precedent in this
/// repo: re-roll a genuinely transient infrastructure failure, and fail hard when it does
/// not clear. So a void reading is retaken on a fresh page, a live one is returned
/// immediately and asserted on in full, and if every attempt is void the last one is
/// handed back *with `context_lost` set* so the test fails naming the real reason.
///
/// The alternative the backlog explicitly forbids — relaxing the assertion to "a context
/// exists" — is what this avoids: the pixel checks below are byte for byte the ones that
/// ran before, and they run on every non-void reading.
async fn read_glsl(browser: &Browser, dir: &Path) -> Result<GlslFacts, String> {
    let built = build(dir, "glsl")?;
    let mut last: Option<GlslFacts> = None;
    for attempt in 1..=GLSL_ATTEMPTS {
        let page = open(browser, &built).await?;
        // One extra frame beyond "all cells done": an unanimated shader draws in a
        // `requestAnimationFrame` after mount, so the pixels exist a tick later than the
        // cell. A head start only — `GLSL_BODY` polls for the draw itself.
        tokio::time::sleep(Duration::from_millis(400)).await;
        let mut facts: GlslFacts = read(&page, &probe(GLSL_BODY)).await?;
        let _ = page.close().await;
        facts.attempts = attempt;
        let lost = facts.context_lost;
        last = Some(facts);
        if !lost {
            break;
        }
        // Let the compositor settle before asking it for another context; a fresh page
        // immediately after a loss tends to land in the same bad moment.
        tokio::time::sleep(Duration::from_millis(750)).await;
    }
    last.ok_or_else(|| "glsl probe produced no reading at all".to_string())
}

async fn observe(browser: &Browser, dir: &Path) -> Result<Run, String> {
    // --- {glsl} -----------------------------------------------------------
    let glsl = read_glsl(browser, dir).await?;

    // --- tali.state -------------------------------------------------------
    // Driven by moving the slider, which is what a scheduled re-run is: each move is one
    // downstream pass, and the accumulating cell must carry its store across them.
    let state_page = open(browser, &build(dir, "state")?).await?;
    let mut state = vec![read::<StateFacts>(&state_page, &probe(STATE_BODY)).await?];
    for v in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
        set_slider(&state_page, v).await?;
        tokio::time::sleep(Duration::from_millis(80)).await;
        if v == 1 {
            state.push(read(&state_page, &probe(STATE_BODY)).await?);
        }
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    state.push(read(&state_page, &probe(STATE_BODY)).await?);
    let _ = state_page.close().await;

    // --- num + tali.tex + tali.table --------------------------------------
    let num_page = open(browser, &build(dir, "numerics")?).await?;
    let tex: TexFacts = read(&num_page, &probe(TEX_BODY)).await?;
    let numerics: Numerics = read(&num_page, NUMERICS_SCRIPT).await?;
    let _ = num_page.close().await;

    Ok(Run {
        glsl,
        state,
        tex,
        numerics,
    })
}

// ---------------------------------------------------------------------------
// page driving
// ---------------------------------------------------------------------------

/// Open a built page and wait until every client-side cell has reported `data-tali-done`.
async fn open(browser: &Browser, path: &Path) -> Result<Page, String> {
    let url = format!("file://{}", path.display());
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| format!("new page: {e}"))?;
    page.goto(&url)
        .await
        .map_err(|e| format!("navigate {url}: {e}"))?;
    for _ in 0..200 {
        let facts: PageFacts = read(&page, &probe(READY_BODY)).await?;
        if facts.cells > 0 && facts.all_cells_done {
            return Ok(page);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(format!("cells never finished mounting at {url}"))
}

async fn read<T: serde::de::DeserializeOwned>(page: &Page, script: &str) -> Result<T, String> {
    let res = tokio::time::timeout(Duration::from_secs(15), page.evaluate_function(script))
        .await
        .map_err(|_| "reading page state timed out".to_string())?
        .map_err(|e| format!("evaluate: {e}"))?;
    res.into_value().map_err(|e| format!("decode state: {e}"))
}

async fn click(page: &Page, selector: &str) -> Result<(), String> {
    let script = format!(
        "function () {{ var e = document.querySelector({selector:?}); if (!e) return false; e.click(); return true; }}"
    );
    let hit: bool = read(page, &script).await?;
    hit.then_some(())
        .ok_or_else(|| format!("no element matched {selector}"))
}

/// Set the page's one `{{< input >}}` slider and fire the `input` event a reader's drag
/// fires, so the runtime schedules exactly the downstream pass it would for a real move.
async fn set_slider(page: &Page, value: i32) -> Result<(), String> {
    page.evaluate(format!(
        "(() => {{ const el = document.querySelector('[data-tali-input]'); \
         if (!el) return false; el.value = String({value}); \
         el.dispatchEvent(new Event('input', {{ bubbles: true }})); return true; }})()"
    ))
    .await
    .map_err(|e| format!("set_slider: {e}"))?;
    Ok(())
}

async fn focus(page: &Page, selector: &str) -> Result<(), String> {
    let script = format!(
        "function () {{ var e = document.querySelector({selector:?}); if (!e) return false; e.focus(); return document.activeElement === e; }}"
    );
    let ok: bool = read(page, &script).await?;
    ok.then_some(())
        .ok_or_else(|| format!("{selector} did not take focus"))
}

/// A trusted key event pair, the way Chrome itself sends a non-text key.
async fn press(page: &Page, key: &str, code: &str, vk: i64) -> Result<(), String> {
    for kind in [
        DispatchKeyEventType::RawKeyDown,
        DispatchKeyEventType::KeyUp,
    ] {
        let params = DispatchKeyEventParams::builder()
            .r#type(kind)
            .key(key)
            .code(code)
            .windows_virtual_key_code(vk)
            .native_virtual_key_code(vk)
            .build()
            .map_err(|e| format!("key params: {e}"))?;
        page.execute(params)
            .await
            .map_err(|e| format!("dispatch {key}: {e}"))?;
    }
    Ok(())
}

async fn mouse(page: &Page, kind: DispatchMouseEventType, x: f64, y: f64) -> Result<(), String> {
    let params = DispatchMouseEventParams::builder()
        .r#type(kind)
        .x(x)
        .y(y)
        .button(MouseButton::Left)
        .click_count(1)
        .build()
        .map_err(|e| format!("mouse params: {e}"))?;
    page.execute(params)
        .await
        .map_err(|e| format!("dispatch mouse: {e}"))?;
    Ok(())
}

/// Viewport coordinates of a fractional position inside the pad. Scrolls it into view
/// first: a click dispatched at a coordinate the pad does not occupy lands on the page.
async fn pad_point(page: &Page, fx: f64, fy: f64) -> Result<(f64, f64), String> {
    let script = format!(
        "function () {{
           var p = document.querySelector('.tali-point-pad');
           p.scrollIntoView({{ block: 'center' }});
           var b = p.getBoundingClientRect();
           return [b.left + b.width * {fx}, b.top + b.height * {fy}];
         }}"
    );
    let xy: Vec<f64> = read(page, &script).await?;
    Ok((xy[0], xy[1]))
}

// ---------------------------------------------------------------------------
// in-page probes — facts only
// ---------------------------------------------------------------------------

/// Shared preamble: how many client-side cells there are, whether all finished, and any
/// error boxes. Spliced into each page probe so every reading carries its own control.
const PAGE_FACTS: &str = r#"
  var scripts = document.querySelectorAll('script[type^="application/tali-"]');
  var done = 0;
  scripts.forEach(function (s) { if (s.hasAttribute('data-tali-done')) done++; });
  var errors = [];
  document.querySelectorAll('.tali-js-error').forEach(function (e) {
    errors.push((e.textContent || '').slice(0, 300));
  });
  var pageFacts = {
    cells: scripts.length,
    allCellsDone: scripts.length > 0 && done === scripts.length,
    errors: errors,
  };
"#;

/// Wrap a probe body in the shared page-facts preamble. Done here rather than with a
/// concat macro so the preamble exists exactly once and every probe carries its own
/// control (cell count, completion, error boxes) without a dependency.
fn probe(body: &str) -> String {
    format!("function () {{{PAGE_FACTS}{body}}}")
}

const READY_BODY: &str = "return pageFacts;";

/// The shader probe is **asynchronous**, and the ordering below is the whole trick.
///
/// A WebGL drawing buffer is discarded the moment the frame composites
/// (`preserveDrawingBuffer` is off, and turning it on would tax every reader to make a
/// test easier), so `readPixels` from an ordinary evaluate returns transparency no matter
/// how well the shader works — the first version of this test read exactly that and
/// reported "never drew" for a shader that was painting correctly. The read has to happen
/// inside the frame that drew, *after* the draw.
///
/// So: nudge the slider (both corpus shaders list `freq`, so both redraw — the animated one
/// would repaint anyway, the static one draws only on an input change). The cells' own
/// `requestAnimationFrame(draw)` calls are queued during that dispatch, the first
/// synchronously and the rest in microtasks. One macrotask turn (`setTimeout`) drains all
/// of them, so the read registered after it runs last in the next frame.
///
/// **Item 179, and the filed cause was wrong.** This test was flaky under load, and the
/// backlog recorded the reason as "the probe samples the canvas before the draw lands,
/// so poll for the paint with a generous timeout". Measured 2026-07-30 at load ≈ 30-60,
/// that fix does **nothing**: polling for a full 10 s returned `[1, 1]` colours after 43
/// rounds. Instrumenting the context instead named the real cause in one run —
/// `isContextLost() === true` and `getError()` 37442 (`CONTEXT_LOST_WEBGL`) on **both**
/// canvases, ~20 ms after the page settled, on 3 runs in 10. SwiftShader's context simply
/// dies under CPU starvation, and no amount of waiting revives it. Two things were
/// refuted on the way, so neither needs retrying: `--disable-gpu-watchdog` does not
/// prevent it (still 3 in 10), and `glsl.js`'s own `dispose()` — which really does call
/// `WEBGL_lose_context` — is not reached, because nothing calls `taliJs.reset`/`teardown`
/// on a static `file://` page. The retry lives in `read_glsl`.
///
/// What the loop below is for is the **second, rarer** mode the same instrumentation
/// found: `[394, 1]`, the animated shader painted and the static one blank. That one *is*
/// an ordering miss, and it is why the loop keeps its `setTimeout(0)` cadence rather than
/// backing off. A delay is actively wrong here: the static cell draws once per input
/// change, so a read registered 250 ms later lands in a frame that composited long ago
/// and is blank *by construction* (measured — a 25→250 ms backoff turned a 23% failure
/// rate into 33%). Tight turns are the whole trick; what the deadline adds is **more of
/// them than a hardcoded 10** when the machine is slow, and an early exit that keeps a
/// quiet machine at one or two. `read`'s own `evaluate` timeout is 15 s, so the budget
/// stays under it, and the round cap bounds the `readPixels` traffic (~1.2 MB a canvas a
/// round) so the probe cannot starve the draw it is waiting for.
const GLSL_BODY: &str = r#"
  var DEADLINE_MS = 8000;
  var MAX_ROUNDS = 120;
  var cs = document.querySelectorAll('canvas.tali-glsl-canvas');
  var out = Object.assign({}, pageFacts, {
    canvases: cs.length, hasContext: false, distinctColours: 0, painted: false,
    contextLost: false, perCanvas: [], rounds: 0, waitedMs: 0,
  });
  if (!cs.length) return Promise.resolve(out);
  out.hasContext = !!cs[0].getContext('webgl');
  var slider = document.querySelector('[data-tali-input]');
  // The slider's own value, bounced between two in-range settings rather than climbing.
  // A deadline loop can run for hundreds of rounds, and `+1` per round would pin the
  // input at its `max` — where the value stops changing and the static shader, which
  // redraws only on a *change*, would stop redrawing exactly when we need it to.
  var base = slider ? Number(slider.value) : 0;
  var min = slider && slider.min !== '' ? Number(slider.min) : base - 1;
  var max = slider && slider.max !== '' ? Number(slider.max) : base + 1;
  var lo = base - 1 >= min ? base - 1 : (base + 1 <= max ? base + 1 : base);
  function nudge(round) {
    if (!slider) return;
    slider.value = String(round % 2 === 0 ? base : lo);
    slider.dispatchEvent(new Event('input', { bubbles: true }));
  }
  function anyContextLost() {
    for (var i = 0; i < cs.length; i++) {
      var g = cs[i].getContext('webgl');
      if (g && g.isContextLost()) return true;
    }
    return false;
  }
  function sample(c) {
    var gl = c.getContext('webgl');
    if (!gl) return { colours: 0, painted: false };
    var w = c.width, h = c.height;
    var px = new Uint8Array(w * h * 4);
    gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, px);
    var seen = {};
    var painted = false;
    for (var i = 0; i < px.length; i += 4 * 97) {  // stride: a sparse but even sample
      if (px[i + 3] !== 0) painted = true;
      seen[px[i] + ',' + px[i + 1] + ',' + px[i + 2] + ',' + px[i + 3]] = 1;
    }
    return { colours: Object.keys(seen).length, painted: painted };
  }
  // The WORST canvas of the best readings, so one working shader cannot hide a broken
  // sibling. This is what the test asserts on, and therefore also what decides whether
  // the loop below has waited long enough.
  var best = [];
  function worstSoFar() {
    var worst = null;
    for (var j = 0; j < cs.length; j++) {
      var b = best[j] || { colours: 0, painted: false };
      if (!worst || b.colours < worst.colours) worst = b;
    }
    return worst || { colours: 0, painted: false };
  }
  return new Promise(function (resolve) {
    var round = 0;
    var started = Date.now();
    function finish() {
      var worst = worstSoFar();
      out.distinctColours = worst.colours;
      out.painted = worst.painted;
      out.rounds = round;
      out.waitedMs = Date.now() - started;
      out.contextLost = anyContextLost();
      for (var k = 0; k < cs.length; k++) out.perCanvas.push((best[k] || {}).colours || 0);
      resolve(out);
    }
    function go() {
      nudge(round);
      // A macrotask turn drains the microtasks in which the cells queue their own
      // `requestAnimationFrame(draw)`, so the read below is usually registered after them.
      // "Usually" is why this repeats: the static cell's `run()` resumes after an `await`,
      // so on some rounds its draw lands in a later turn than this one and reads blank.
      // Keeping the best reading per canvas across rounds makes that ordering irrelevant
      // instead of hoping for it (a single round was measurably flaky).
      setTimeout(function () {
        requestAnimationFrame(function () {
          for (var i = 0; i < cs.length; i++) {
            var r = sample(cs[i]);
            if (!best[i] || r.colours > best[i].colours) best[i] = r;
          }
          round++;
          // Stop as soon as every canvas satisfies what the test asserts; otherwise keep
          // waiting for the draw until the budget is spent. Reporting a partial reading
          // at the deadline (rather than throwing) keeps the failure message honest: the
          // test still says which assertion failed, and `rounds`/`waitedMs` say whether
          // it was starved or genuinely blank.
          // Stop as soon as every canvas clears what the test asserts. A lost context
          // ends it too: it cannot come back on this page, so further rounds would only
          // burn the deadline before reporting the same thing.
          var w = worstSoFar();
          var satisfied = w.painted && w.colours > 8;
          var out_of_budget = round >= MAX_ROUNDS || Date.now() - started >= DEADLINE_MS;
          if (satisfied || anyContextLost() || out_of_budget) {
            finish();
            return;
          }
          go();
        });
      }, 0);
    }
    go();
  });
"#;

const STATE_BODY: &str = r#"
  // The accumulating cell's own paragraph, found by its text rather than by a class the
  // corpus document does not carry.
  var seen = '';
  document.querySelectorAll('.tali-js-out p').forEach(function (p) {
    if ((p.textContent || '').indexOf('last frames') === 0) seen = p.textContent;
  });
  return Object.assign({}, pageFacts, { seenText: seen });
"#;

const TEX_BODY: &str = r#"
  var roots = document.querySelectorAll('.tali-math');
  var grids = document.querySelectorAll('.tali-math-grid');
  var grid = grids.length ? grids[grids.length - 1] : null;  // the 2x2 Cholesky factor
  var table = document.querySelector('.tali-mini-table table');
  var cols = [];
  if (table) {
    table.querySelectorAll('thead th').forEach(function (th) { cols.push(th.textContent || ''); });
  }
  return Object.assign({}, pageFacts, {
    roots: roots.length,
    matrixCells: grid ? grid.children.length : 0,
    matrixCols: grid ? String(grid.style.gridTemplateColumns) : '',
    matrixText: grid ? (grid.textContent || '') : '',
    tableCols: cols,
    tableRows: table ? table.querySelectorAll('tbody tr').length : 0,
  });
"#;

/// Pure-function checks against `num`, evaluated in the page so it is the SHIPPED bundle
/// under test rather than a copy.
const NUMERICS_SCRIPT: &str = r#"function () {
  var n = window.taliNum;
  var a = n.random(42), b = n.random(42), c = n.random(43);
  var sa = a.sample(4000, function () { return a.normal(0, 1); });
  var sb = b.sample(4000, function () { return b.normal(0, 1); });
  var sc = c.sample(8, function () { return c.uniform(); });
  var sa8 = sa.slice(0, 8);
  var same = JSON.stringify(sa.slice(0, 32)) === JSON.stringify(sb.slice(0, 32));
  var differ = JSON.stringify(sa8) !== JSON.stringify(sc);

  var A = [[2, 0.8], [0.8, 1]];
  var L = n.cholesky(A);
  var R = n.matmul(L, n.transpose(L));
  var cholErr = 0;
  for (var i = 0; i < 2; i++) for (var j = 0; j < 2; j++) {
    cholErr = Math.max(cholErr, Math.abs(R[i][j] - A[i][j]));
  }
  var I = n.matmul(A, n.inv2(A));
  var invErr = 0;
  for (var p = 0; p < 2; p++) for (var q = 0; q < 2; q++) {
    invErr = Math.max(invErr, Math.abs(I[p][q] - (p === q ? 1 : 0)));
  }
  var rejects = false;
  try { n.cholesky([[1, 2], [2, 1]]); } catch (e) { rejects = true; }

  return {
    gaussPdf0: n.gaussian.pdf(0, 0, 1),
    gaussCdf196: n.gaussian.cdf(1.96, 0, 1),
    betaCdfHalf: n.beta.cdf(0.5, 2, 2),
    gammaCdf1: n.gamma.cdf(1, 1, 1),
    poissonCdf2: n.poisson.cdf(2, 2),
    seededMean: n.mean(sa),
    seedIsReproducible: same,
    seedsDiffer: differ,
    cholErr: cholErr,
    invErr: invErr,
    eigTop: n.eig2(A).values[0],
    cholRejectsIndefinite: rejects,
  };
}"#;

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

fn observed() -> Option<&'static Run> {
    if !have_chrome() {
        return None;
    }
    match run() {
        Ok(r) => Some(r),
        Err(e) => panic!("browser run failed: {e}"),
    }
}

fn assert_clean(page: &PageFacts, what: &str) {
    assert!(page.cells > 0, "{what}: the page mounted no cells at all");
    assert!(page.all_cells_done, "{what}: not every cell finished");
    assert!(
        page.errors.is_empty(),
        "{what}: cells reported errors: {:?}",
        page.errors
    );
}

/// Item 153, end to end: the registry seam really does run a second language. If `glsl.js`
/// never registered, or the mime disagreed, or the shader failed to compile, the page would
/// carry an error box (or an unmounted canvas) instead of pixels.
#[test]
fn a_glsl_cell_compiles_and_paints() {
    let Some(r) = observed() else { return };
    assert_clean(&r.glsl.page, "glsl");
    assert_eq!(r.glsl.canvases, 2, "both shader cells mounted a canvas");
    assert!(r.glsl.has_context, "the canvas has a live WebGL context");
    // Checked BEFORE the pixels, because it changes what a blank canvas means. Every
    // pixel assertion below is conditional on a live context, and this is the line that
    // establishes it — so a machine that cannot hold a software GL context fails here,
    // naming that, instead of failing below as "the shader never drew".
    assert!(
        !r.glsl.context_lost,
        "the WebGL context was lost on all {} attempts, so no pixel reading was possible: \
         this machine could not hold a SwiftShader context, it is not a shader defect",
        r.glsl.attempts
    );
    // `rounds`/`waitedMs`/`perCanvas` are in both messages on purpose. They separate the
    // failure modes that look identical from the assertion alone: a full budget spent
    // means starved, a couple of fast rounds means genuinely blank, and `[394, 1]` means
    // the *static* shader missed its frame while the animated one drew fine. Without
    // them, one afternoon went into reading an environment failure as a code bug.
    assert!(
        r.glsl.painted,
        "no pixel was painted — the shader mounted but never drew (polled {} rounds over \
         {:.0} ms across {} page load(s), colours per canvas {:?})",
        r.glsl.rounds, r.glsl.waited_ms, r.glsl.attempts, r.glsl.per_canvas
    );
    // Kept in step with the probe's own early-exit threshold: it stops waiting once every
    // canvas clears this bar, so raising it here without raising it there would let the
    // loop stop short of what is now being asserted.
    assert!(
        r.glsl.distinct_colours > 8,
        "the corpus shader paints a ring, so a flat canvas ({} distinct colours) means the \
         uniforms never arrived or the draw is wrong (polled {} rounds over {:.0} ms across \
         {} page load(s), colours per canvas {:?})",
        r.glsl.distinct_colours,
        r.glsl.rounds,
        r.glsl.waited_ms,
        r.glsl.attempts,
        r.glsl.per_canvas
    );
}

/// Item 156: state survives a scheduled re-run (so the list grows) and is bounded by the
/// cell's own logic rather than by anything the runtime does.
#[test]
fn tali_state_accumulates_across_re_runs() {
    let Some(r) = observed() else { return };
    let (rest, stepped, played) = (&r.state[0], &r.state[1], &r.state[2]);
    let count = |s: &str| s.split(',').count();

    assert!(
        rest.seen_text.starts_with("last frames:"),
        "the accumulating cell did not render: {:?}",
        rest.seen_text
    );
    assert!(
        count(&stepped.seen_text) > count(&rest.seen_text),
        "state did not survive the re-run: {:?} -> {:?}",
        rest.seen_text,
        stepped.seen_text
    );
    // The cell keeps the last 8, so a store that were cleared every run would show one
    // entry forever and a store that never dropped anything would show dozens.
    assert!(
        count(&played.seen_text) > 2 && count(&played.seen_text) <= 8,
        "after ~10 passes the cell's own `slice(-8)` should bound the list, got {:?}",
        played.seen_text
    );
}

/// Item 157: `tali.tex` typesets a matrix as a real grid, and `tali.table` builds a table
/// with inferred columns.
#[test]
fn tex_and_table_render_structured_output() {
    let Some(r) = observed() else { return };
    assert_clean(&r.tex.page, "numerics page");
    assert_eq!(
        r.tex.roots, 2,
        "the corpus page makes exactly two `tali.tex` calls (the third mention is prose)"
    );
    assert_eq!(
        r.tex.matrix_cells, 4,
        "a 2x2 Cholesky factor is four grid cells"
    );
    assert!(
        r.tex.matrix_cols.contains("repeat(2") || r.tex.matrix_cols.split(' ').count() == 2,
        "the grid must be two columns wide, got {:?}",
        r.tex.matrix_cols
    );
    assert!(
        !r.tex.matrix_text.contains('-'),
        "a negative must be typeset with U+2212 MINUS, never a hyphen: {:?}",
        r.tex.matrix_text
    );
    assert_eq!(
        r.tex.table_cols,
        vec!["component", "eigenvalue", "x", "y"],
        "columns are inferred from the records in first-seen order"
    );
    assert_eq!(r.tex.table_rows, 2, "two rows in, two rows out");
}

/// Item 154: the shipped bundle's numbers, checked against closed forms rather than
/// against itself.
#[test]
fn the_numerics_global_is_numerically_right() {
    let Some(r) = observed() else { return };
    let n = &r.numerics;
    let close = |got: f64, want: f64, tol: f64, what: &str| {
        assert!(
            (got - want).abs() < tol,
            "{what}: got {got}, expected {want} (tol {tol})"
        );
    };
    close(
        n.gauss_pdf0,
        1.0 / (2.0 * std::f64::consts::PI).sqrt(),
        1e-12,
        "gaussian.pdf(0)",
    );
    close(n.gauss_cdf196, 0.975, 1e-4, "gaussian.cdf(1.96)");
    close(n.beta_cdf_half, 0.5, 1e-9, "beta.cdf(0.5, 2, 2)");
    close(
        n.gamma_cdf1,
        1.0 - std::f64::consts::E.recip(),
        1e-9,
        "gamma.cdf(1, 1, 1)",
    );
    close(
        n.poisson_cdf2,
        5.0 * (-2.0f64).exp(),
        1e-9,
        "poisson.cdf(2, 2)",
    );
    close(
        n.eig_top,
        (3.0 + (1.0 + 4.0 * 0.64f64).sqrt()) / 2.0,
        1e-9,
        "eig2 top",
    );
    assert!(
        n.chol_err < 1e-12,
        "L Lᵀ must reconstruct A: {}",
        n.chol_err
    );
    assert!(n.inv_err < 1e-12, "A A⁻¹ must be I: {}", n.inv_err);
    assert!(
        n.chol_rejects_indefinite,
        "cholesky must reject a non-positive-definite matrix rather than return NaNs"
    );
}

/// The seeded generator is the reason `num.random` exists at all: a published explorable
/// that resamples on every render is not reproducible. Both directions are asserted —
/// a generator that ignored its seed would satisfy the first alone.
#[test]
fn the_prng_is_seeded_and_seeds_are_independent() {
    let Some(r) = observed() else { return };
    assert!(
        r.numerics.seed_is_reproducible,
        "two generators with the same seed must produce the same stream"
    );
    assert!(
        r.numerics.seeds_differ,
        "two generators with different seeds must diverge (the control on the row above)"
    );
    // 4,000 standard normals: |mean| < 4/sqrt(4000) ≈ 0.063 unless the generator is
    // biased. Loose on purpose — this is a smoke test for a broken distribution, not a
    // statistical test.
    assert!(
        r.numerics.seeded_mean.abs() < 0.08,
        "the seeded normal stream looks biased: mean {}",
        r.numerics.seeded_mean
    );
}
