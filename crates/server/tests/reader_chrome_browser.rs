//! A browser test of the reader-facing page chrome: the mobile contents handle (item 198).
//!
//! **Why a browser test.** The handle is built entirely in JS off the live DOM, so
//! *nothing* about it reaches the served HTML — the Rust suite can only prove the
//! enhancer script is bundled, which it would keep proving with the handle's press
//! behaviour broken.
//!
//! Gated exactly like `deck_browser.rs` / `reactive_browser.rs`: no system Chrome → skip,
//! unless `TALIESIN_REQUIRE_CHROME=1` turns the skip into a hard failure. One browser run
//! serves every test here (a `OnceLock`).
//!
//! The figure lightbox's dismissal contract (item 195) that used to live here was deleted
//! 2026-08-03 (visual minimalism pass) along with the whole viewer. Reading-position
//! resume/progress (item 199) that used to live here was deleted the same day along with
//! the enhancer that produced it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
};
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;

// ---------------------------------------------------------------------------
// Chrome gate (mirrors reactive_browser.rs, which mirrors deck_browser.rs)
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
        "TALIESIN_REQUIRE_CHROME=1 but no system Chrome found: the resume position and \
         the mobile contents handle would both go untested"
    );
    eprintln!("skipping: no system Chrome (set CHROME_PATH or install google-chrome/chromium)");
    false
}

// ---------------------------------------------------------------------------
// what one run observed
// ---------------------------------------------------------------------------

/// One reading of the mobile contents handle.
#[derive(Debug, Clone, serde::Deserialize)]
struct Handle {
    /// Whether the handle has a box at all. `display: none` reports false.
    shown: bool,
    /// Its visible text with the (absolutely-positioned, hidden-at-rest) current-section
    /// chip removed — i.e. what a reader can actually read on the button.
    label: String,
    /// `getComputedStyle(...).cursor`. `grab` is what made it read as a drag grip.
    cursor: String,
    /// The `aria-expanded` it publishes.
    expanded: String,
    /// Whether the sheet is open (`body.tali-toc-open`).
    #[serde(rename = "sheetOpen")]
    sheet_open: bool,
    /// Whether anything is painted at the handle's centre AND that thing is the handle (or
    /// inside it). The control on `shown`: an element with a box can still be buried under
    /// the sheet's backdrop, where a press reaches the backdrop instead.
    #[serde(rename = "hitIsHandle")]
    hit_is_handle: bool,
}

struct Run {
    /// The mobile contents handle: at rest, after a press, and after a second press.
    handle: Vec<Handle>,
}

static RUN: OnceLock<Result<Run, String>> = OnceLock::new();

fn run() -> &'static Result<Run, String> {
    RUN.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("tali-reader-chrome-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;
        let out = tokio::runtime::Runtime::new()
            .map_err(|e| format!("tokio runtime: {e}"))?
            .block_on(drive(&dir));
        let _ = std::fs::remove_dir_all(&dir);
        out
    })
}

/// Build one corpus document into a standalone page. `reader/long-read.tmd` is the
/// deliberately-long scrolling fixture the mobile-handle probe runs against.
fn build(dir: &Path, rel: &str, name: &str) -> Result<PathBuf, String> {
    let src = format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"));
    let out = dir.join(name);
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", &src])
        .arg(&out)
        .output()
        .map_err(|e| format!("run build: {e}"))?;
    if !res.status.success() {
        return Err(format!(
            "build corpus/{rel} failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }
    out.exists()
        .then_some(out)
        .ok_or_else(|| format!("build of {rel} reported success but wrote no page"))
}

async fn drive(dir: &Path) -> Result<Run, String> {
    let exe = which_chrome().ok_or_else(|| "chrome unavailable".to_string())?;
    let profile =
        std::env::temp_dir().join(format!("tali-reader-chrome-profile-{}", std::process::id()));
    let config = BrowserConfig::builder()
        .chrome_executable(&exe)
        .new_headless_mode()
        // Same reasoning as the sibling browser tests: Chrome's own sandbox needs
        // unprivileged user namespaces and is unavailable in containers/CI. The page is a
        // `file://` document this repo just rendered from its own corpus.
        .no_sandbox()
        .user_data_dir(&profile)
        .window_size(1280, 900)
        .launch_timeout(Duration::from_secs(20))
        .request_timeout(Duration::from_secs(20))
        .args(vec![
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
    let handle = read_toc_handle(browser, dir).await?;

    Ok(Run { handle })
}

/// The mobile contents handle at a phone viewport: at rest, after one press, after a second.
///
/// Emulated metrics rather than a resized window, because a headless Chrome window will not
/// go below roughly 500 px wide and the sheet breakpoint is 60rem — a plain resize would test
/// the desktop rail while reporting phone numbers.
async fn read_toc_handle(browser: &Browser, dir: &Path) -> Result<Vec<Handle>, String> {
    // `reader/long-read.tmd` sets `toc: true`, which is what puts the sheet chrome on the page.
    let path = build(dir, "reader/long-read.tmd", "long-read-toc.html")?;
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| format!("new page: {e}"))?;
    let metrics = SetDeviceMetricsOverrideParams::builder()
        .width(390)
        .height(844)
        .device_scale_factor(1.0)
        .mobile(true)
        .build()
        .map_err(|e| format!("metrics params: {e}"))?;
    page.execute(metrics)
        .await
        .map_err(|e| format!("emulate phone: {e}"))?;
    page.goto(format!("file://{}", path.display()))
        .await
        .map_err(|e| format!("navigate: {e}"))?;
    for _ in 0..200 {
        let ready: bool = read(
            &page,
            "function () { return document.body.classList.contains('tali-toc-sheet'); }",
        )
        .await?;
        if ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let mut shots = vec![handle_shot(&page).await?];
    for _ in 0..2 {
        press_handle(&page).await?;
        // The sheet slides on a .3s transition; the class flips at once but the hit-test
        // below needs the paint to have settled where it ends up.
        tokio::time::sleep(Duration::from_millis(500)).await;
        shots.push(handle_shot(&page).await?);
    }
    Ok(shots)
}

/// A real press on the handle: `pointerdown`/`pointerup` at its centre, because the handle
/// is driven by pointer events (a synthetic `.click()` would bypass the tap/drag decision
/// that is the thing being asserted).
async fn press_handle(page: &Page) -> Result<(), String> {
    let pt: Vec<f64> = read(
        page,
        "function () {
           var r = document.getElementById('tali-toc-handle').getBoundingClientRect();
           return [r.left + r.width / 2, r.top + r.height / 2];
         }",
    )
    .await?;
    match pt.as_slice() {
        [x, y] => mouse_click(page, *x, *y).await,
        _ => Err("could not measure the contents handle".to_string()),
    }
}

async fn handle_shot(page: &Page) -> Result<Handle, String> {
    read(
        page,
        "function () {
           var h = document.getElementById('tali-toc-handle');
           if (!h) return { shown: false, label: '', cursor: 'none', expanded: '',
                            sheetOpen: false, hitIsHandle: false };
           var r = h.getBoundingClientRect();
           var cur = document.getElementById('tali-toc-cur');
           var text = (h.textContent || '').replace(cur ? cur.textContent || '' : '', '').trim();
           var hit = r.width > 0 && r.height > 0
             ? document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2)
             : null;
           return {
             shown: r.width > 0 && r.height > 0,
             label: text,
             cursor: getComputedStyle(h).cursor,
             expanded: h.getAttribute('aria-expanded') || '',
             sheetOpen: document.body.classList.contains('tali-toc-open'),
             hitIsHandle: !!(hit && (hit === h || h.contains(hit))),
           };
         }",
    )
    .await
}

async fn read<T: serde::de::DeserializeOwned>(page: &Page, script: &str) -> Result<T, String> {
    let res = tokio::time::timeout(Duration::from_secs(15), page.evaluate_function(script))
        .await
        .map_err(|_| "reading page state timed out".to_string())?
        .map_err(|e| format!("evaluate: {e}"))?;
    res.into_value().map_err(|e| format!("decode state: {e}"))
}

/// A trusted press/release pair at a viewport coordinate, the way a mouse sends one.
async fn mouse_click(page: &Page, x: f64, y: f64) -> Result<(), String> {
    for kind in [
        DispatchMouseEventType::MousePressed,
        DispatchMouseEventType::MouseReleased,
    ] {
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
    }
    Ok(())
}

fn observed() -> &'static Run {
    match run() {
        Ok(r) => r,
        Err(e) => panic!("reader-chrome browser run failed: {e}"),
    }
}

// ---------------------------------------------------------------------------
// the assertions
// ---------------------------------------------------------------------------

/// Item 198: the bottom-centre contents handle on a phone must read as a button and behave
/// like one. It was a 42x5 px grip with `cursor: grab`, no chevron, no visible label, and
/// `display: none` while the sheet was open — so it announced "drag me from the bottom edge"
/// and offered no press-again-to-close.
#[test]
fn the_mobile_contents_handle_reads_and_behaves_as_a_toggle() {
    if !have_chrome() {
        return;
    }
    let shots = &observed().handle;
    let (rest, opened, closed) = (&shots[0], &shots[1], &shots[2]);

    assert!(
        rest.shown && rest.hit_is_handle,
        "control: the handle must be on screen and pressable at rest: {rest:?}"
    );
    assert!(
        rest.label.contains("Contents"),
        "the handle must say what it opens: {rest:?}"
    );
    assert_ne!(
        rest.cursor, "grab",
        "`grab` is the cursor that made it read as a drag grip: {rest:?}"
    );
    assert_eq!(
        rest.expanded, "false",
        "at rest the sheet is shut: {rest:?}"
    );

    assert!(
        opened.sheet_open,
        "a press must open the contents sheet: {opened:?}"
    );
    assert!(
        opened.shown && opened.hit_is_handle,
        "and the handle must stay mounted AND pressable over the open sheet, or there is \
         no press-again-to-close: {opened:?}"
    );
    assert_eq!(opened.expanded, "true");

    assert!(
        !closed.sheet_open,
        "a second press must close it again: {closed:?}"
    );
    assert_eq!(closed.expanded, "false");
}
