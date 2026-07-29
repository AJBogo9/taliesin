//! A browser test of the explorable cluster: the cell-language registry and `{glsl}` (153),
//! the `num` global (154), the `animate` tick and draggable `point` (155), `tali.state`
//! (156), and `tali.tex`/`tali.table` (157).
//!
//! **Why a browser test and not more emission tests.** Every one of those five items is
//! mostly JavaScript. The Rust suite can prove a `<script type="application/tali-glsl">`
//! reaches the page and a `data-tali-tick` field is emitted; it cannot see whether the
//! shader compiled, whether pressing Play advances anything, whether `tali.state` survives
//! a re-run, or whether `num.gaussian.pdf` returns the right number. Those are the claims
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
}

#[derive(Debug, Clone, serde::Deserialize)]
struct AnimateFacts {
    #[serde(flatten)]
    page: PageFacts,
    /// The tick's value.
    tick: f64,
    /// The downstream `//| name: wave` value's first sample, which changes with the tick.
    #[serde(rename = "waveHead")]
    wave_head: f64,
    /// The accumulating cell's readout, e.g. "last frames: 0, 1, 2".
    #[serde(rename = "seenText")]
    seen_text: String,
    /// The Play button's `aria-pressed`.
    #[serde(rename = "playPressed")]
    play_pressed: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PointFacts {
    #[serde(flatten)]
    page: PageFacts,
    /// The published value, parsed — this is the widened `readValue` path.
    x: f64,
    y: f64,
    /// The last point in the consuming cell's published `fitted.pts`, i.e. what a
    /// downstream cell actually RECEIVED. This is the assertion that the widened
    /// `readValue` works: had the cell been handed the raw JSON string, `p.x` would be
    /// `undefined` and these would be null — a silent failure, not an error.
    #[serde(rename = "fittedX")]
    fitted_x: Option<f64>,
    #[serde(rename = "fittedY")]
    fitted_y: Option<f64>,
    /// The dot's `left`/`top` as percentages, so the painted position can be checked
    /// against the published one (and the y-up convention with it).
    #[serde(rename = "dotLeft")]
    dot_left: f64,
    #[serde(rename = "dotTop")]
    dot_top: f64,
    /// The readout's text, which is this control's only visible value.
    #[serde(rename = "outText")]
    out_text: String,
    /// The URL fragment, which must carry the structured value.
    hash: String,
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
    animate: Vec<AnimateFacts>,
    /// Point: (initial, after ArrowRight×2 + ArrowUp, after a real mouse click at 25%/25%).
    point: Vec<PointFacts>,
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

async fn observe(browser: &Browser, dir: &Path) -> Result<Run, String> {
    // --- {glsl} -----------------------------------------------------------
    let glsl_page = open(browser, &build(dir, "glsl")?).await?;
    // One extra frame beyond "all cells done": an unanimated shader draws in a
    // `requestAnimationFrame` after mount, so the pixels exist a tick later than the cell.
    tokio::time::sleep(Duration::from_millis(400)).await;
    let glsl: GlslFacts = read(&glsl_page, &probe(GLSL_BODY)).await?;
    let _ = glsl_page.close().await;

    // --- animate + tali.state ---------------------------------------------
    let anim_page = open(browser, &build(dir, "animate")?).await?;
    let mut animate = vec![read::<AnimateFacts>(&anim_page, &probe(ANIMATE_BODY)).await?];
    click(&anim_page, "[data-tali-animate=\"step\"]").await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    animate.push(read(&anim_page, &probe(ANIMATE_BODY)).await?);
    click(&anim_page, "[data-tali-animate=\"play\"]").await?;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let playing: AnimateFacts = read(&anim_page, &probe(ANIMATE_BODY)).await?;
    click(&anim_page, "[data-tali-animate=\"play\"]").await?; // pause, so the reading is stable
    tokio::time::sleep(Duration::from_millis(200)).await;
    animate.push(playing);
    click(&anim_page, "[data-tali-animate=\"reset\"]").await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    animate.push(read(&anim_page, &probe(ANIMATE_BODY)).await?);
    let _ = anim_page.close().await;

    // --- point ------------------------------------------------------------
    let point_page = open(browser, &build(dir, "point")?).await?;
    let mut point = vec![read::<PointFacts>(&point_page, &probe(POINT_BODY)).await?];
    focus(&point_page, ".tali-point-pad").await?;
    for (key, code, vk) in [
        ("ArrowRight", "ArrowRight", 39),
        ("ArrowRight", "ArrowRight", 39),
        ("ArrowUp", "ArrowUp", 38),
    ] {
        press(&point_page, key, code, vk).await?;
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    tokio::time::sleep(Duration::from_millis(250)).await;
    point.push(read(&point_page, &probe(POINT_BODY)).await?);
    // A REAL mouse press, dispatched through CDP: the pad uses pointer capture, and a
    // synthetic `PointerEvent` carries a pointerId the browser will refuse to capture, so
    // a JS-dispatched event would exercise a different path than a reader's finger does.
    let (px, py) = pad_point(&point_page, 0.25, 0.25).await?;
    mouse(&point_page, DispatchMouseEventType::MousePressed, px, py).await?;
    mouse(&point_page, DispatchMouseEventType::MouseReleased, px, py).await?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    point.push(read(&point_page, &probe(POINT_BODY)).await?);
    let _ = point_page.close().await;

    // --- num + tali.tex + tali.table --------------------------------------
    let num_page = open(browser, &build(dir, "numerics")?).await?;
    let tex: TexFacts = read(&num_page, &probe(TEX_BODY)).await?;
    let numerics: Numerics = read(&num_page, NUMERICS_SCRIPT).await?;
    let _ = num_page.close().await;

    Ok(Run {
        glsl,
        animate,
        point,
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
const GLSL_BODY: &str = r#"
  var cs = document.querySelectorAll('canvas.tali-glsl-canvas');
  var out = Object.assign({}, pageFacts, {
    canvases: cs.length, hasContext: false, distinctColours: 0, painted: false,
  });
  if (!cs.length) return Promise.resolve(out);
  out.hasContext = !!cs[0].getContext('webgl');
  var slider = document.querySelector('[data-tali-input]');
  function nudge() {
    if (!slider) return;
    slider.value = String(Number(slider.value) + 1);
    slider.dispatchEvent(new Event('input', { bubbles: true }));
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
  var best = [];
  return new Promise(function (resolve) {
    var round = 0;
    function go() {
      nudge();
      // A macrotask turn drains the microtasks in which the cells queue their own
      // `requestAnimationFrame(draw)`, so the read below is usually registered after them.
      // "Usually" is why this repeats: the static cell's `run()` resumes after an `await`,
      // so on some rounds its draw lands in a later turn than this one and reads blank.
      // Keeping the best reading per canvas over several rounds makes that ordering
      // irrelevant instead of hoping for it (a single round was measurably flaky).
      setTimeout(function () {
        requestAnimationFrame(function () {
          for (var i = 0; i < cs.length; i++) {
            var r = sample(cs[i]);
            if (!best[i] || r.colours > best[i].colours) best[i] = r;
          }
          if (++round < 10) { go(); return; }
          // Report the WORST canvas, so one working shader cannot hide a broken sibling.
          var worst = null;
          for (var j = 0; j < cs.length; j++) {
            var b = best[j] || { colours: 0, painted: false };
            if (!worst || b.colours < worst.colours) worst = b;
          }
          out.distinctColours = worst ? worst.colours : 0;
          out.painted = !!(worst && worst.painted);
          resolve(out);
        });
      }, 0);
    }
    go();
  });
"#;

const ANIMATE_BODY: &str = r#"
  var rt = window.__talijs || {};
  var tick = document.querySelector('[data-tali-tick]');
  var play = document.querySelector('[data-tali-animate="play"]');
  var wave = (rt.scope || {}).wave;
  // The accumulating cell's own paragraph, found by its text rather than by a class the
  // corpus document does not carry.
  var seen = '';
  document.querySelectorAll('.tali-js-out p').forEach(function (p) {
    if ((p.textContent || '').indexOf('last frames') === 0) seen = p.textContent;
  });
  return Object.assign({}, pageFacts, {
    tick: tick ? Number(tick.value) : NaN,
    waveHead: (wave && wave.length) ? wave[0] : NaN,
    seenText: seen,
    playPressed: play ? String(play.getAttribute('aria-pressed')) : '',
  });
"#;

const POINT_BODY: &str = r#"
  var el = document.querySelector('[data-tali-json]');
  var dot = document.querySelector('.tali-point-dot');
  var out = document.querySelector('[data-tali-out]');
  var v = null;
  try { v = JSON.parse(el.value); } catch (e) {}
  // What the CONSUMING cell received, read off the value it published.
  var fitted = (window.__talijs && window.__talijs.scope) ? window.__talijs.scope.fitted : null;
  var last = (fitted && fitted.pts && fitted.pts.length) ? fitted.pts[fitted.pts.length - 1] : null;
  return Object.assign({}, pageFacts, {
    x: v ? v.x : NaN,
    y: v ? v.y : NaN,
    fittedX: (last && typeof last[0] === 'number' && isFinite(last[0])) ? last[0] : null,
    fittedY: (last && typeof last[1] === 'number' && isFinite(last[1])) ? last[1] : null,
    dotLeft: dot ? parseFloat(dot.style.left) : NaN,
    dotTop: dot ? parseFloat(dot.style.top) : NaN,
    outText: out ? (out.textContent || '') : '',
    hash: location.hash,
  });
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
    assert!(
        r.glsl.painted,
        "no pixel was painted — the shader mounted but never drew"
    );
    assert!(
        r.glsl.distinct_colours > 8,
        "the corpus shader paints a ring, so a flat canvas ({} distinct colours) means the \
         uniforms never arrived or the draw is wrong",
        r.glsl.distinct_colours
    );
}

/// Item 155a: the tick advances, and the downstream cell re-runs with it. Step first
/// (deterministic), then Play (which must keep advancing on its own), then Reset.
#[test]
fn the_animate_control_advances_the_tick_and_its_downstream_cell() {
    let Some(r) = observed() else { return };
    let (rest, stepped, played, reset) =
        (&r.animate[0], &r.animate[1], &r.animate[2], &r.animate[3]);
    for (label, f) in [
        ("at rest", rest),
        ("stepped", stepped),
        ("played", played),
        ("reset", reset),
    ] {
        assert_clean(&f.page, label);
    }

    assert_eq!(rest.tick, 0.0, "the tick starts at its `min`");
    assert_eq!(stepped.tick, 1.0, "Step advances exactly one frame");
    assert!(
        played.tick > stepped.tick + 2.0,
        "~1s of Play at 10 fps should advance several frames, got {} -> {}",
        stepped.tick,
        played.tick
    );
    assert_eq!(reset.tick, 0.0, "Reset returns to `min`");

    // The point of the tick: a downstream cell recomputes. Reading the runtime's published
    // `wave` value, not the tick, is what makes this about the GRAPH rather than the input.
    assert!(
        (stepped.wave_head - rest.wave_head).abs() > 1e-9,
        "the downstream `//| name: wave` cell did not re-run on the tick ({} vs {})",
        rest.wave_head,
        stepped.wave_head
    );

    assert_eq!(
        played.play_pressed, "true",
        "Play must report its pressed state to assistive tech while running"
    );
    assert_eq!(reset.play_pressed, "false", "Reset also stops playback");
}

// ---------------------------------------------------------------------------
// Item 155a's named trap — measured, and NOT automatically covered. Read this before
// adding a test for it.
// ---------------------------------------------------------------------------
//
// The trap is that a tick must schedule ONE downstream pass per frame through the existing
// scheduler, never a continuous dataflow loop. Two mechanisms in `bindAnimate` enforce it:
// frames are paced to `fps`, and the next frame is not requested until the previous pass
// has resolved (`r.pending[name]`).
//
// **Two candidate assertions were written and both were deleted as coverage illusions**,
// each after being mutation-checked with the mechanisms removed:
//
//   1. A frame CEILING ("~1.1 s at fps=10 is under 20 frames"). Green with both mechanisms
//      deleted: headless Chrome's own `requestAnimationFrame` is throttled by the Plot
//      re-render, so a runaway pump advances no faster than a paced one.
//   2. A LAG measure (the drawn wave's tick versus the counter's). Also green with both
//      deleted, because each pass reads `tali.value` when it STARTS, so the last pass to
//      run always reads the newest tick — the figure stays in step even when passes
//      overlap.
//
// So the property has no observable this harness can reach, and a green assertion that
// cannot fail is worse than none. It is enforced by construction in `bindAnimate` and by
// review; recorded in `notes/DETECTION-DEBT.md` as a known gap rather than papered over.

/// Item 156: state survives a scheduled re-run (so the list grows) and is bounded by the
/// cell's own logic rather than by anything the runtime does.
#[test]
fn tali_state_accumulates_across_re_runs() {
    let Some(r) = observed() else { return };
    let (rest, stepped, played) = (&r.animate[0], &r.animate[1], &r.animate[2]);
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
        "after ~10 frames the cell's own `slice(-8)` should bound the list, got {:?}",
        played.seen_text
    );
}

/// Item 155b: the pad publishes a STRUCTURED value, the arrow keys move it, and the
/// published `y` grows upward while the painted dot moves up the screen.
#[test]
fn the_point_pad_publishes_a_structured_value_by_keyboard() {
    let Some(r) = observed() else { return };
    let (start, keyed) = (&r.point[0], &r.point[1]);
    assert_clean(&start.page, "point at rest");
    assert_clean(&keyed.page, "point after keys");

    // The widened `readValue` proved end to end: the consuming cell put the pad's point
    // into its own published `fitted.pts`, which requires `p.x`/`p.y` to have been NUMBERS.
    // Handed the raw JSON string instead, `p.x` is `undefined` — no error, just a plot
    // quietly built on NaN.
    assert_eq!(
        (keyed.fitted_x, keyed.fitted_y),
        (Some(keyed.x), Some(keyed.y)),
        "the downstream cell did not receive the point as an object"
    );
    assert!(
        (keyed.x - (start.x + 0.4)).abs() < 1e-6,
        "two ArrowRights at step=0.2 should move x by 0.4: {} -> {}",
        start.x,
        keyed.x
    );
    assert!(
        (keyed.y - (start.y + 0.2)).abs() < 1e-6,
        "one ArrowUp at step=0.2 should move y by 0.2: {} -> {}",
        start.y,
        keyed.y
    );
    // y up in the value, up on the screen: `top` must DECREASE as `y` increases. This is
    // the assertion that catches a mirrored pad, which no numeric check would.
    assert!(
        keyed.dot_top < start.dot_top && keyed.dot_left > start.dot_left,
        "the painted dot did not follow the value (left {} -> {}, top {} -> {})",
        start.dot_left,
        keyed.dot_left,
        start.dot_top,
        keyed.dot_top
    );
    assert!(
        keyed.out_text.contains(','),
        "the readout is this control's only visible value: {:?}",
        keyed.out_text
    );
    assert!(
        keyed.hash.contains("%22x%22") || keyed.hash.contains("\"x\""),
        "the structured value must round-trip through the URL fragment: {:?}",
        keyed.hash
    );
}

/// The same pad under a REAL pointer press, which is the path `setPointerCapture` is on —
/// a synthetic event carries a pointerId the browser refuses to capture, so this is the
/// only way to know a reader's drag works.
#[test]
fn the_point_pad_follows_a_real_pointer_press() {
    let Some(r) = observed() else { return };
    let (keyed, clicked) = (&r.point[1], &r.point[2]);
    assert_clean(&clicked.page, "point after click");
    // Pressed at 25% across and 25% down, on a -3..3 domain: x ≈ -1.5, and y ≈ +1.5
    // because screen-down is value-up.
    assert!(
        (clicked.x - -1.5).abs() < 0.25,
        "a press at 25% across a -3..3 pad should publish x ≈ -1.5, got {}",
        clicked.x
    );
    assert!(
        (clicked.y - 1.5).abs() < 0.25,
        "a press at 25% DOWN should publish y ≈ +1.5 (y grows upward), got {}",
        clicked.y
    );
    assert!(
        (clicked.x - keyed.x).abs() > 0.1,
        "the press did not move the point at all — it never reached the pad"
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
