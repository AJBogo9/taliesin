//! A browser test of the reader-facing page chrome: reading-progress/resume (item 199)
//! and the mobile contents handle (item 198).
//!
//! **Why a browser test.** Both features are built entirely in JS off the live DOM, so
//! *nothing* about either reaches the served HTML — the Rust suite can only prove the
//! enhancer scripts are bundled, which it would keep proving with the resume path
//! throwing on its first scroll.
//!
//! The trap this file is written against is vacuity. "The progress bar is gone" is
//! asserted beside a *positive* reading that the resume position it shared a function
//! with is still being recorded and offered, because deleting the whole enhancer would
//! satisfy the absence on its own.
//!
//! Gated exactly like `deck_browser.rs` / `reactive_browser.rs`: no system Chrome → skip,
//! unless `TALIESIN_REQUIRE_CHROME=1` turns the skip into a hard failure. One browser run
//! serves every test here (a `OnceLock`).
//!
//! The figure lightbox's dismissal contract (item 195) that used to live here was deleted
//! 2026-08-03 (visual minimalism pass) along with the whole viewer.

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

/// What `corpus/reader/long-read.tmd` reports across one scroll-and-revisit cycle.
#[derive(Debug, Clone, serde::Deserialize)]
struct Progress {
    /// Whether a `.tali-readbar` exists anywhere in the document. Item 199 deletes it.
    #[serde(rename = "barPresent")]
    bar_present: bool,
    /// How far down the page the probe actually got. The control on everything below: a
    /// document too short to scroll reports every reading as "nothing happened".
    #[serde(rename = "scrolledFrac")]
    scrolled_frac: f64,
    /// The raw `tali-pos:<path>` record, `"<frac>|<block-id>"`. Written by `saveSoon`,
    /// which calls the same `frac()` the deleted bar used — so a null here is exactly the
    /// failure mode the deletion could cause and a string check could not see.
    stored: Option<String>,
    /// The resume pill's button text on a FRESH visit to the same page.
    #[serde(rename = "resumeText")]
    resume_text: Option<String>,
    /// Uncaught page errors seen across the whole cycle. `onScroll` runs on every scroll
    /// event, so a `ReferenceError` there is silent to every other reading in this file.
    errors: Vec<String>,
}

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
    /// Reading progress + resume, on a long document.
    progress: Progress,
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
/// deliberately-long scrolling fixture used by both the progress/resume and mobile-handle
/// probes.
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
    let progress = read_progress(browser, dir).await?;
    let handle = read_toc_handle(browser, dir).await?;

    Ok(Run { progress, handle })
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

/// Scroll a long page halfway, let the position record settle, then arrive at the same
/// page in a **fresh tab** (which has no scroll-restoration history, so the resume pill's
/// "are we already roughly there" guard cannot suppress it).
async fn read_progress(browser: &Browser, dir: &Path) -> Result<Progress, String> {
    let path = build(dir, "reader/long-read.tmd", "long-read.html")?;
    let url = format!("file://{}", path.display());

    let first = browser
        .new_page("about:blank")
        .await
        .map_err(|e| format!("new page: {e}"))?;
    // Collect uncaught errors from the moment the enhancers run. `onScroll` fires
    // constantly and swallows nothing, so this is where a broken deletion would show.
    first
        .evaluate_on_new_document(
            "window.__taliErrors = []; \
             window.addEventListener('error', function (e) { window.__taliErrors.push(String(e.message)); }); \
             window.addEventListener('unhandledrejection', function (e) { window.__taliErrors.push(String(e.reason)); });",
        )
        .await
        .map_err(|e| format!("install error hook: {e}"))?;
    first
        .goto(&url)
        .await
        .map_err(|e| format!("navigate {url}: {e}"))?;
    for _ in 0..200 {
        let ready: bool = read(
            &first,
            "function () { return !!(window.__taliProgress && document.querySelector('[data-block-id]')); }",
        )
        .await?;
        if ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let bar_present: bool = read(
        &first,
        "function () { return !!document.querySelector('.tali-readbar'); }",
    )
    .await?;

    // Halfway down. `scroll-behavior` is forced to `auto` first: the stylesheet sets
    // `smooth`, under which `scrollTo` only STARTS an animation, so reading the offset back
    // on the next line measures the top of the page and every assertion below reads as a
    // regression. Then a settle, then a wait that outlasts `saveSoon`'s 500 ms debounce.
    read::<bool>(
        &first,
        "function () {
           var h = document.documentElement;
           h.style.scrollBehavior = 'auto';
           var max = (h.scrollHeight || document.body.scrollHeight) - window.innerHeight;
           window.scrollTo(0, Math.round(max * 0.5));
           return max > 0;
         }",
    )
    .await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let scrolled_frac: f64 = read(
        &first,
        "function () {
           var h = document.documentElement;
           var max = (h.scrollHeight || document.body.scrollHeight) - window.innerHeight;
           var y = window.pageYOffset != null ? window.pageYOffset : h.scrollTop;
           return max > 0 ? y / max : 0;
         }",
    )
    .await?;
    tokio::time::sleep(Duration::from_millis(900)).await;

    // Every probe below returns a STRING, never `null`: a CDP result whose `value` is
    // absent decodes as "No value found" rather than as `None`, which reads like a browser
    // failure when it only means "the element is not there yet".
    let stored: String = read(
        &first,
        "function () { try { return localStorage.getItem('tali-pos:' + location.pathname) || ''; } \
         catch (e) { return ''; } }",
    )
    .await?;
    let errors: Vec<String> =
        read(&first, "function () { return window.__taliErrors || []; }").await?;

    // A fresh tab, same URL: this is "close the tab partway through and come back", which
    // is the sentence `corpus/reader/long-read.tmd` makes to the reader.
    let second = browser
        .new_page("about:blank")
        .await
        .map_err(|e| format!("new page: {e}"))?;
    second
        .goto(&url)
        .await
        .map_err(|e| format!("navigate {url}: {e}"))?;
    let mut resume_text = None;
    for _ in 0..80 {
        let text: String = read(
            &second,
            "function () { var b = document.querySelector('.tali-resume-go'); \
             return b ? b.textContent || '' : ''; }",
        )
        .await?;
        if !text.is_empty() {
            resume_text = Some(text);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    Ok(Progress {
        bar_present,
        scrolled_frac,
        stored: (!stored.is_empty()).then_some(stored),
        resume_text,
        errors,
    })
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

/// Item 199: the top reading-progress bar is gone (it duplicates the native scrollbar),
/// **and** the two unrelated features that shared `taliInitReadingProgress` with it — the
/// block-anchored resume position and its pill — still work. Asserting the absence alone
/// would be satisfied by deleting the whole enhancer, which is precisely what must not
/// happen: `frac()` still feeds the position record.
#[test]
fn the_reading_bar_is_gone_but_the_resume_position_is_not() {
    if !have_chrome() {
        return;
    }
    let p = &observed().progress;
    assert!(
        p.scrolled_frac > 0.3,
        "control: corpus/reader/long-read.tmd must be long enough to scroll, got {:?}",
        p
    );
    assert!(
        !p.bar_present,
        "the top reading-progress bar was deleted (item 199): {p:?}"
    );
    let stored = p
        .stored
        .as_deref()
        .unwrap_or_else(|| panic!("scrolling must still record a resume position: {p:?}"));
    let (frac, block) = stored
        .split_once('|')
        .unwrap_or_else(|| panic!("a position record is `<frac>|<block-id>`, got {stored:?}"));
    assert!(
        frac.parse::<f64>().is_ok_and(|f| f > 0.04),
        "the recorded fraction comes from the same frac() the bar used: {stored:?}"
    );
    assert!(
        !block.is_empty(),
        "the record must name the block to return to: {stored:?}"
    );
    assert!(
        p.resume_text
            .as_deref()
            .is_some_and(|t| t.contains("Resume reading")),
        "arriving again must still offer the way back: {p:?}"
    );
    assert!(
        p.errors.is_empty(),
        "the enhancer must not throw on scroll: {:?}",
        p.errors
    );
}
