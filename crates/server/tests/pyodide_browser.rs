//! The `{pyodide}` browser gate (backlog item 158).
//!
//! **Why this file exists at all.** Every other test of item 158 asserts what *Rust emitted*:
//! that a `<script type="application/tali-pyodide">` reaches the page (`client_lang.rs`), that
//! the `<meta name="tali-pyodide-index">` resolves per mode (`core/tests/pyodide.rs`), that a
//! site build copies the payload and stamps a page-relative index (`asset_bundle.rs`), that the
//! payload is vendored complete (`third_party.rs`). All of them stay green with the runtime
//! to an empty file. Nothing below the emission layer — booting a WASM CPython, capturing its
//! stdout, turning its last expression into a *published* reactive value, and re-running the
//! downstream `{js}` consumer — is observable from Rust. This is the only test that looks.
//!
//! **Why HTTP and not `file://` like `reactive_browser.rs`.** Chrome blocks `fetch()` and ES
//! module imports for `file://` origins, and Pyodide must fetch `pyodide.asm.wasm` and
//! `python_stdlib.zip` to start at all. So the boot test drives a real
//! `taliesin preview corpus/reactive/pyodide.tmd`, which incidentally exercises the served
//! `/_taliesin/pyodide-*/` route as well. The *shield* test (which needs no runtime) keeps
//! the cheaper `file://` build path.
//!
//! **`data-tali-done` is NOT a readiness signal here, and treating it as one is a race.** It
//! is set when the wrapper's `run()` returns, and a `{pyodide}` cell's `run()` returns a
//! PLACEHOLDER immediately by design (note 1 in `crates/core/assets/js/pyodide.js`): blocking
//! it would stall every cell below on the page. So every cell reports done within
//! milliseconds, long before Python exists. The poll below waits for the real evidence —
//! captured stdout on the page and a published array in the reactive scope.
//!
//! Gated exactly like `reactive_browser.rs` and `deck_browser.rs`: no system Chrome → skip,
//! unless `TALIESIN_REQUIRE_CHROME=1` turns the skip into a hard failure. One preview server,
//! one tokio runtime and one Chrome launch serve both browser tests (a `OnceLock`). The third
//! test needs no browser at all.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;

// ---------------------------------------------------------------------------
// Chrome gate (copied from reactive_browser.rs, which mirrors headless_js::chrome_path)
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
        "TALIESIN_REQUIRE_CHROME=1 but no system Chrome found: the whole `{{pyodide}}` runtime \
         would go untested"
    );
    eprintln!("skipping: no system Chrome (set CHROME_PATH or install google-chrome/chromium)");
    false
}

// ---------------------------------------------------------------------------
// a preview server of our own, always reaped
// ---------------------------------------------------------------------------

/// Above Linux's default ephemeral range (`net.ipv4.ip_local_port_range`, 32768-60999), so
/// the kernel never hands one of these out as an outbound source port, and disjoint from
/// `preview_single_instance.rs`'s 21,000-based bands so the two binaries cannot collide even
/// if someone runs them concurrently.
const PORT_FLOOR: u16 = 61_000;

/// How far past its requested port a preview walks when that port is taken
/// (`serve/mod.rs`: `for p in port+1..=port+9`).
const FALLBACK_SPAN: u16 = 9;

/// Ports reserved per caller, wider than the fallback walk so two servers in this binary
/// cannot land on each other's range.
const BAND: u16 = 64;

static NEXT_BAND: AtomicU16 = AtomicU16::new(0);

/// A base port to *ask* for. Deliberately not called "acquire": binding and releasing is a
/// peek, not an acquisition (the lesson recorded in `preview_single_instance.rs::free_run`),
/// and the preview may well end up somewhere else in its fallback walk. Which port it
/// actually took is settled afterwards by [`resolve_port`], by pid, so losing this race
/// costs nothing.
fn base_port() -> u16 {
    let band = NEXT_BAND.fetch_add(1, Ordering::SeqCst);
    let run_offset = (std::process::id() as u16 % 16) * (BAND * 4);
    let start = PORT_FLOOR + run_offset + band * BAND;
    for base in start..start + BAND - FALLBACK_SPAN {
        if TcpListener::bind(("127.0.0.1", base)).is_ok() {
            return base;
        }
    }
    start
}

/// A spawned preview that is always reaped, even when an assertion panics: a leaked dev
/// server keeps a watcher (and, on other documents, a kernel subtree) alive forever, which
/// is precisely what `preview_single_instance.rs` exists to prevent.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Server {
    fn spawn(path: &Path, port: u16) -> Server {
        let child = Command::new(env!("CARGO_BIN_EXE_taliesin"))
            .arg("preview")
            .arg(path)
            .arg(port.to_string())
            // The preview clears the screen and streams a banner; keep the test output clean
            // and make sure a full pipe can never block the child.
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn preview");
        Server(child)
    }

    fn pid(&self) -> u32 {
        self.0.id()
    }
}

/// Which port the preview really took, found by asking every port in its fallback walk who it
/// is and matching the answer's pid against the child we spawned.
///
/// A bare "is the port open" probe would be wrong twice over: another process may hold the
/// base port (in which case the preview walked past it), and a *different* preview may be
/// answering on it. Matching the pid settles both, and does it without hanging.
fn resolve_port(child_pid: u32, base: u16, dur: Duration) -> Option<u16> {
    let deadline = Instant::now() + dur;
    while Instant::now() < deadline {
        for port in base..=base + FALLBACK_SPAN {
            if let Some(body) = http_get(port, "/__taliesin")
                && serde_json::from_str::<serde_json::Value>(&body)
                    .is_ok_and(|v| v["pid"].as_u64() == Some(u64::from(child_pid)))
            {
                return Some(port);
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

/// Minimal loopback HTTP/1.1 GET returning `(response head, body bytes)`. The crate has no
/// HTTP client dependency and this file needs a handful of requests, so hand-roll it the way
/// `preview_single_instance.rs` does — except in BYTES, because one of the bodies below is
/// 9.6 MB of WebAssembly that is not valid UTF-8.
fn http_get_raw(port: u16, path: &str) -> Option<(String, Vec<u8>)> {
    let mut sock = TcpStream::connect(("127.0.0.1", port)).ok()?;
    sock.set_read_timeout(Some(Duration::from_secs(30))).ok()?;
    sock.write_all(
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").as_bytes(),
    )
    .ok()?;
    let mut raw = Vec::new();
    sock.read_to_end(&mut raw).ok()?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    Some((head, raw[split + 4..].to_vec()))
}

fn http_get(port: u16, path: &str) -> Option<String> {
    let (_head, body) = http_get_raw(port, path)?;
    String::from_utf8(body).ok()
}

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-pyodide-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

// ---------------------------------------------------------------------------
// what one run observed
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize)]
struct Reading {
    /// Client-side cells on the page: two `{pyodide}` and one `{js}`.
    cells: usize,
    /// How many reported `data-tali-done`. A structural control only — see the header: a
    /// `{pyodide}` cell reports done before Python exists.
    done: usize,
    /// `.tali-js-error` boxes. The shared wrapper renders one for any cell failure, and
    /// `pyodide.js` renders one for a Python traceback, so this covers both.
    errors: Vec<String>,
    /// The `.tali-pyodide-stdout` blocks' text. Only the second corpus cell prints.
    stdout: Vec<String>,
    /// The `.tali-pyodide-value` blocks' text, i.e. the `repr` display path — a different
    /// code path from stdout, and the one that proves numpy actually computed.
    values: Vec<String>,
    /// `window.__talijs.scope.samples.length`, or `-1` when nothing published an array under
    /// that name.
    #[serde(rename = "publishedLen")]
    published_len: i64,
    /// Every `<rect>`/`<path>` inside an `<svg>` anywhere on the page. Reported for context
    /// only: the page shell's own theme-toggle and copy-button icons contribute 8 of these
    /// before a single cell runs, so this number can never be *asserted* on.
    #[serde(rename = "chartMarks")]
    chart_marks: usize,
    /// The same count scoped to the `{js}` cell's own output container, which is empty until
    /// that cell re-runs with the published data and Observable Plot draws. This is the one
    /// the test asserts on.
    #[serde(rename = "cellChartMarks")]
    cell_chart_marks: usize,
    /// Every distinct error box seen at ANY point during the poll, filled in from Rust rather
    /// than the page. A `#| input:` cell runs once before its input has arrived and again when
    /// it lands, so an error box that later clears is a normal intermediate state and must not
    /// fail the test — but it is worth reporting when something else does fail, because a
    /// transient traceback is usually the first clue to a real ordering bug.
    #[serde(skip)]
    transient_errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Shield {
    /// `typeof tali.publish`.
    direct: String,
    /// `typeof Object.getPrototypeOf(tali).publish` — the axis a masking shield leaked
    /// through during Task 3.
    proto: String,
    /// `Object.getOwnPropertyNames(tali).includes("publish")`, which sees a non-enumerable
    /// own property that `typeof` on an overwritten getter might not.
    own: bool,
    /// `"publish" in tali`, which walks the whole prototype chain.
    #[serde(rename = "inOp")]
    in_op: bool,
    /// `typeof tali.get`. The known-positive: without it, all four negatives above are
    /// satisfied by `tali` being an empty object, or by the cell never running at all.
    sanity: String,
}

struct Run {
    corpus: Reading,
    shield: Shield,
}

static RUN: OnceLock<Result<Run, String>> = OnceLock::new();

fn run() -> &'static Result<Run, String> {
    RUN.get_or_init(|| {
        let dir = tmp_dir("run");
        let doc = format!(
            "{}/../../corpus/reactive/pyodide.tmd",
            env!("CARGO_MANIFEST_DIR")
        );
        // Resolved from CARGO_MANIFEST_DIR, never from the process CWD: `cargo test` runs a
        // test binary from the workspace root today, but that is not a contract.
        if !Path::new(&doc).is_file() {
            return Err(format!("corpus document missing: {doc}"));
        }
        let base = base_port();
        let server = Server::spawn(Path::new(&doc), base);
        let port = match resolve_port(server.pid(), base, Duration::from_secs(60)) {
            Some(p) => p,
            None => {
                return Err(format!(
                    "the preview never answered /__taliesin with pid {} on any of {base}..={}",
                    server.pid(),
                    base + FALLBACK_SPAN
                ));
            }
        };
        let out = tokio::runtime::Runtime::new()
            .map_err(|e| format!("tokio runtime: {e}"))?
            .block_on(drive(&dir, port));
        drop(server);
        let _ = fs::remove_dir_all(&dir);
        out
    })
}

async fn drive(dir: &Path, port: u16) -> Result<Run, String> {
    let exe = which_chrome().ok_or_else(|| "chrome unavailable".to_string())?;
    let profile = std::env::temp_dir().join(format!("tali-pyodide-profile-{}", std::process::id()));
    let config = BrowserConfig::builder()
        .chrome_executable(&exe)
        .new_headless_mode()
        // Same reasoning as headless_js.rs: Chrome's own sandbox needs unprivileged user
        // namespaces and is unavailable in containers/CI. The pages are a loopback preview
        // and a `file://` document this repo just rendered from its own sources.
        .no_sandbox()
        .user_data_dir(&profile)
        // TALLER than reactive_browser.rs's 900, and that is load-bearing rather than
        // cosmetic. `pyodide.js` starts a cell only when it comes within 600 px of the
        // viewport (`whenNear`), so a cell that never enters that reach never runs and the
        // poll below would wait out its whole budget on a page that is working perfectly.
        // 2000 + 600 clears the corpus page's pre-execution height with room to spare, which
        // is what makes the test independent of how tall the prose happens to render.
        .window_size(1280, 2000)
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

    let result = observe(&browser, dir, port).await;

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
    let _ = fs::remove_dir_all(&profile);
    result
}

async fn observe(browser: &Browser, dir: &Path, port: u16) -> Result<Run, String> {
    let corpus = read_corpus(browser, port).await?;
    let shield = read_shield(browser, dir).await?;
    Ok(Run { corpus, shield })
}

/// Rounds of the boot poll, and how long each waits. Two minutes in total.
///
/// Generous on purpose: booting a 15.7 MiB WASM CPython and then loading numpy is seconds even
/// warm, and varies with the disk cache and with how loaded the machine is. This asserts "it
/// eventually works", never "it is fast" — there is no timing claim here to pin.
const BOOT_ROUNDS: usize = 240;
const BOOT_INTERVAL: Duration = Duration::from_millis(500);

/// The corpus page's `#| input:` cell prints this prefix followed by the number of samples it
/// actually received. Its whole job is to make the js-scope -> Python direction observable.
const INPUT_MARKER: &str = "samples from the cell above:";

async fn read_corpus(browser: &Browser, port: u16) -> Result<Reading, String> {
    let url = format!("http://127.0.0.1:{port}/");
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| format!("new page: {e}"))?;
    page.goto(&url)
        .await
        .map_err(|e| format!("navigate {url}: {e}"))?;

    let mut last: Option<Reading> = None;
    let mut transient: Vec<String> = Vec::new();
    for _ in 0..BOOT_ROUNDS {
        let mut r: Reading = read(&page, CORPUS_PROBE).await?;
        for e in &r.errors {
            if !transient.contains(e) {
                transient.push(e.clone());
            }
        }
        r.transient_errors = transient.clone();
        // Settled means "there is something real to assert on, or something went wrong":
        // captured stdout AND a published value AND the consumer's redraw AND the
        // `#| input:` cell having run, or an error box. Returning the LAST reading either
        // way is what keeps a timeout's failure message about the page rather than the clock.
        //
        // The `INPUT_MARKER` clause is load-bearing, not belt-and-braces: that cell re-runs
        // when `samples` publishes, and so does the `{js}` consumer, with no ordering
        // guarantee between them. Settling on the redraw alone could therefore take the
        // reading while the input cell still showed its pre-publish `0`, which is exactly
        // the failure the assertion below exists to catch — a flake that reports a real bug.
        //
        // **An error box does NOT settle this loop, and that is deliberate.** It used to, and
        // that was a race of this test's own making: an `#| input:` cell runs once before its
        // input exists and again when it lands, so a traceback on the way to a correct page is
        // a normal intermediate state. Short-circuiting on it took the reading mid-flight and
        // failed on `published_len: -1` under load — a red gate describing nothing real. A
        // page that is genuinely broken still fails, just via the full budget and the
        // known-positive assertion, with every error it ever showed attached.
        let settled = !r.stdout.is_empty()
            && r.published_len >= 0
            && r.cell_chart_marks > 0
            && r.stdout.join("\n").contains(INPUT_MARKER);
        last = Some(r);
        if settled {
            break;
        }
        tokio::time::sleep(BOOT_INTERVAL).await;
    }
    let _ = page.close().await;
    last.ok_or_else(|| "the pyodide probe produced no reading at all".to_string())
}

async fn read_shield(browser: &Browser, dir: &Path) -> Result<Shield, String> {
    let src = dir.join("shield.tmd");
    fs::write(&src, SHIELD_DOC).map_err(|e| format!("write shield doc: {e}"))?;
    let out = dir.join("shield.html");
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", &src.to_string_lossy()])
        .arg(&out)
        .output()
        .map_err(|e| format!("run build: {e}"))?;
    if !res.status.success() {
        return Err(format!(
            "building the shield probe failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }

    let url = format!("file://{}", out.display());
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| format!("new page: {e}"))?;
    page.goto(&url)
        .await
        .map_err(|e| format!("navigate {url}: {e}"))?;
    for _ in 0..200 {
        if let Some(s) = read::<Option<Shield>>(&page, SHIELD_PROBE).await? {
            let _ = page.close().await;
            return Ok(s);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let _ = page.close().await;
    Err("the shield probe's `{js}` cell never rendered its readout".to_string())
}

async fn read<T: serde::de::DeserializeOwned>(page: &Page, script: &str) -> Result<T, String> {
    let res = tokio::time::timeout(Duration::from_secs(15), page.evaluate_function(script))
        .await
        .map_err(|_| "reading page state timed out".to_string())?
        .map_err(|e| format!("evaluate: {e}"))?;
    res.into_value().map_err(|e| format!("decode state: {e}"))
}

// ---------------------------------------------------------------------------
// in-page probes — facts only
// ---------------------------------------------------------------------------

const CORPUS_PROBE: &str = r#"function () {
  var scripts = document.querySelectorAll('script[type^="application/tali-"]');
  var done = 0;
  scripts.forEach(function (s) { if (s.hasAttribute('data-tali-done')) done++; });
  var errors = [];
  document.querySelectorAll('.tali-js-error').forEach(function (e) {
    errors.push((e.textContent || '').slice(0, 1600));
  });
  var stdout = [];
  document.querySelectorAll('.tali-pyodide-stdout').forEach(function (e) {
    stdout.push(e.textContent || '');
  });
  var values = [];
  document.querySelectorAll('.tali-pyodide-value').forEach(function (e) {
    values.push((e.textContent || '').slice(0, 120));
  });
  var r = window.__talijs;
  var s = r && r.scope ? r.scope.samples : undefined;
  return {
    cells: scripts.length,
    done: done,
    errors: errors,
    stdout: stdout,
    values: values,
    publishedLen: Array.isArray(s) ? s.length : -1,
    chartMarks: document.querySelectorAll('svg rect, svg path').length,
    cellChartMarks: document.querySelectorAll(
      '.tali-js-cell .tali-js-out svg rect, .tali-js-cell .tali-js-out svg path'
    ).length
  };
}"#;

/// A one-cell document whose `{js}` source interrogates its own `tali` object from four
/// directions. Deliberately tiny and self-contained: the claim is about `tali-js.js`, not
/// about any corpus page, and building a page this small keeps the probe fast enough to run
/// on the shared browser without a server.
const SHIELD_DOC: &str = r#"---
title: "publish shield probe"
---

```{js}
const el = document.createElement("pre");
el.id = "tali-publish-probe";
el.textContent = JSON.stringify({
  direct: typeof tali.publish,
  proto: typeof (Object.getPrototypeOf(tali) || {}).publish,
  own: Object.getOwnPropertyNames(tali).includes("publish"),
  inOp: "publish" in tali,
  sanity: typeof tali.get
});
return el;
```
"#;

const SHIELD_PROBE: &str = r#"function () {
  var el = document.getElementById("tali-publish-probe");
  if (!el) return null;
  try { return JSON.parse(el.textContent || "null"); } catch (e) { return null; }
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

/// The canary, pinned by name in `tools/gates.sh` and by `crates/core/tests/gate_script.rs`.
/// Renaming it without updating both silently removes the only proof this feature is
/// exercised at all.
///
/// One test rather than five because the chain is one claim: the runtime boots in the
/// reader's browser, Python runs, its `print` reaches the page, its last expression becomes
/// the PUBLISHED reactive value, and the downstream `{js}` cell re-runs on that publication.
/// Split into five, each piece would need the same two-minute boot.
#[test]
fn a_pyodide_cell_boots_and_publishes_to_a_js_consumer() {
    let Some(r) = observed() else { return };
    let o = &r.corpus;

    // Reported, never asserted on. An `#| input:` cell showing a traceback before its input
    // arrives is legitimate, so this must not fail the run — but it is the first clue to an
    // ordering bug, and it was invisible while the poll treated any error as terminal.
    if !o.transient_errors.is_empty() {
        eprintln!(
            "note: {} error box(es) appeared and cleared while the page settled: {:?}",
            o.transient_errors.len(),
            o.transient_errors
        );
    }

    // KNOWN-POSITIVE FIRST. If the runtime never booted there is no `.tali-pyodide-stdout` on
    // the page at all, and every assertion below is a statement about an empty document
    // rather than about the feature. Nothing else in this test is meaningful without it.
    assert!(
        o.stdout.join("\n").contains("shape: (2, 3)"),
        "the second corpus cell's `print(\"shape:\", a.shape)` never reached the page. If this \
         is the only failure, the Pyodide runtime did not boot (or did not finish within \
         {BOOT_ROUNDS} x {BOOT_INTERVAL:?}) and every other assertion here would be vacuous. \
         Last reading: {o:?}"
    );

    // The `repr` display path, which is a different code path from the stdout capture above:
    // `a.sum()` is the cell's last expression, and 0+1+2+3+4+5 = 15. Also the only assertion
    // that pins numpy having really computed rather than merely imported.
    assert!(
        o.values.iter().any(|v| v.trim() == "15"),
        "the second cell's last expression `a.sum()` must display as 15 via the \
         `.tali-pyodide-value` repr path, got {:?} (full reading: {o:?})",
        o.values
    );

    // The publication itself. `-1` means nothing was published under `samples` — `hooks.publish`
    // never landed, so the reactive scope has no such array. Any other number means the WRONG
    // thing was published: the placeholder, a PyProxy, or a truncated conversion.
    assert_eq!(
        o.published_len, 500,
        "the first cell's `#| name: samples` must publish its last expression — a 500-element \
         list — into the reactive scope. -1 means nothing was published there at all \
         (`hooks.publish` never ran); any other length means something else was. Full \
         reading: {o:?}"
    );

    // The consumer re-ran. Scoped to the `{js}` cell's own output container ON PURPOSE: the
    // page shell's theme-toggle and copy-button icons already put 8 `<path>` nodes in an
    // `<svg>` on this page before any cell runs, so an unscoped `svg path` count is satisfied
    // by a page where Python never produced anything. Until the publish lands, this cell
    // renders the text node "waiting for Python" and this count is 0.
    assert!(
        o.cell_chart_marks > 0,
        "the downstream `{{js}}` cell never re-ran after the publish: its output container \
         holds no plot marks (page-wide, chrome included: {}). Full reading: {o:?}",
        o.chart_marks
    );

    // The OTHER direction across the language boundary, and the one that was silently broken:
    // a `{pyodide}` cell reading, through `#| input:`, a value another cell published into the
    // reactive scope. The enhancer injected `api.value(name)`, which reads `r.inputs`/
    // `r.defines` — controls and kernel defines — and is `undefined` for a `#| name:`
    // publication, which lives in `r.scope` behind `api.get`. Python therefore received `None`
    // while the graph re-ran the cell perfectly, so it looked wired and delivered nothing.
    // `0` here is precisely that regression; the guard in the corpus cell means it reports the
    // count instead of throwing.
    assert!(
        o.stdout.join("\n").contains(&format!("{INPUT_MARKER} 500")),
        "the `#| input:` cell must receive all 500 published samples in its Python namespace. \
         `{INPUT_MARKER} 0` means the injection read the wrong accessor and Python got `None`. \
         Full reading: {o:?}"
    );

    assert!(
        o.errors.is_empty(),
        "cells reported errors: {:?} (full reading: {o:?})",
        o.errors
    );

    // Structural control, not a readiness signal (see the file header): four client-side
    // cells mounted and all four returned from `run()`.
    assert_eq!(
        (o.cells, o.done),
        (4, 4),
        "the corpus page has three `{{pyodide}}` cells and one `{{js}}` consumer, and every \
         wrapper `run()` must have returned. Full reading: {o:?}"
    );
}

/// The publish capability must be unreachable from author cell source, along **every** axis.
///
/// Four axes and not one, because the obvious shield leaks. During Task 3 a masking property
/// on `api` was tried first, and it left the capability reachable through
/// `Object.getPrototypeOf(tali).publish` — a deliberate prototype walk from three characters
/// of author JavaScript. The design that replaced it does not shield anything: `publish` is
/// `setup()`'s FOURTH argument and is never a property of `api` at all
/// (`crates/core/assets/js/tali-js.js:523-541`), so there is no property to walk to. That is a
/// structural claim, and these four readings are what make it falsifiable rather than a
/// comment: `typeof` catches the plain access, the prototype read catches the masking shape
/// that already failed once, `getOwnPropertyNames` catches a non-enumerable own property that
/// a clever getter could hide from `typeof`, and `in` walks the whole chain.
///
/// Why it matters: a cell that could publish to a name it also declares as an `//| input:`
/// would create a feedback edge `buildGraph` never saw and never cycle-checked, recursing
/// without a guard — the reactive VM this project has refused three times.
///
/// Needs no Pyodide and no server: this is a property of `tali-js.js` alone, so it runs on a
/// two-line document built to a `file://` page.
#[test]
fn author_cell_source_cannot_reach_the_publish_hook() {
    let Some(r) = observed() else { return };
    let s = &r.shield;

    // KNOWN-POSITIVE FIRST, and it is the whole reason this test is not vacuous: all four
    // negatives below are equally satisfied by `tali` being an empty object, by the cell
    // never running, or by the probe reading a stale node. `tali.get` being a live function
    // is what establishes that the object under test is the real API surface.
    assert_eq!(
        s.sanity, "function",
        "`tali.get` must be a function — without it this is not the real cell API and the four \
         negatives below prove nothing. Reading: {s:?}"
    );

    assert_eq!(
        s.direct, "undefined",
        "`tali.publish` is reachable from author source. Reading: {s:?}"
    );
    assert_eq!(
        s.proto, "undefined",
        "`Object.getPrototypeOf(tali).publish` is reachable — this is the exact leak the \
         masking shield had. Reading: {s:?}"
    );
    assert!(
        !s.own,
        "`publish` is an own property of `tali`. Reading: {s:?}"
    );
    assert!(
        !s.in_op,
        "`\"publish\" in tali` is true, so it is somewhere on the prototype chain. \
         Reading: {s:?}"
    );
}

/// The site server's `/_taliesin/pyodide-*/` route, over plain loopback HTTP.
///
/// Neither `pyodide_asset` handler had any test: `serve/mod.rs` and `serve_site/mod.rs` carry
/// duplicate copies and both shipped uncovered. The canary above drives the single-doc one
/// (its page could not boot otherwise); this covers the site one, which is registered
/// unconditionally (`serve_site/mod.rs:371`), so a two-file temp site reaches it.
///
/// **`application/wasm` is load-bearing**: `WebAssembly.instantiateStreaming` rejects any
/// other content type outright. The honest limit, and the reason this is asserted here rather
/// than left for the canary to notice, is that Pyodide *falls back* to a buffer instantiate
/// when streaming fails — so serving the wasm as `application/octet-stream` would still
/// produce a working page, just a slower one, and no browser test would go red.
#[test]
fn the_site_preview_serves_the_pyodide_payload_with_its_real_content_types() {
    let dir = tmp_dir("site");
    fs::write(dir.join("_site.yml"), "title: Book\n").unwrap();
    fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nProse.\n").unwrap();

    let base = base_port();
    let server = Server::spawn(&dir, base);
    let port = resolve_port(server.pid(), base, Duration::from_secs(60)).unwrap_or_else(|| {
        panic!(
            "the site preview never answered /__taliesin with pid {} on any of {base}..={}",
            server.pid(),
            base + FALLBACK_SPAN
        )
    });

    let prefix = taliesin_core::PREVIEW_PYODIDE_DIR;

    // The wasm: status, content type, and the bytes themselves.
    let (head, body) = http_get_raw(port, &format!("{prefix}pyodide.asm.wasm"))
        .expect("the site preview should serve pyodide.asm.wasm");
    let lower = head.to_ascii_lowercase();
    assert!(
        lower.starts_with("http/1.1 200"),
        "pyodide.asm.wasm did not come back 200:\n{head}"
    );
    assert!(
        lower.contains("content-type: application/wasm"),
        "the wasm MUST be served as `application/wasm` — `WebAssembly.instantiateStreaming` \
         rejects anything else:\n{head}"
    );
    // Compared against the payload ACCESSOR, never against a hardcoded length: a literal on
    // both sides of a comparison is what made a Task 1 test vacuous.
    let expected = taliesin_core::pyodide_payload()
        .iter()
        .find(|(name, _)| *name == "pyodide.asm.wasm")
        .map(|(_, bytes)| *bytes)
        .expect("pyodide_payload() must carry pyodide.asm.wasm");
    assert_eq!(
        body.len(),
        expected.len(),
        "the served wasm is a different length from the vendored payload"
    );
    assert!(
        body == expected,
        "the served wasm bytes differ from `pyodide_payload()`'s copy at the same length"
    );

    // The loader. `pyodide.mjs` is imported as an ES module, so it must be served as
    // JavaScript; a wrong type here is a hard module-load failure, not a slow path.
    let (head, _) = http_get_raw(port, &format!("{prefix}pyodide.mjs"))
        .expect("the site preview should serve pyodide.mjs");
    let lower = head.to_ascii_lowercase();
    assert!(
        lower.starts_with("http/1.1 200"),
        "pyodide.mjs did not come back 200:\n{head}"
    );
    assert!(
        lower.contains("content-type: text/javascript; charset=utf-8"),
        "pyodide.mjs must be served as JavaScript:\n{head}"
    );

    // Provenance, not payload. `package.json` is vendored so `crates/core/tests/third_party.rs`
    // can read the version and licence out of upstream's own metadata; `pyodide_payload()`
    // deliberately omits it, and the route must not invent a way to reach it.
    let (head, _) = http_get_raw(port, &format!("{prefix}package.json"))
        .expect("the route should answer for package.json, even to refuse it");
    assert!(
        head.to_ascii_lowercase().starts_with("http/1.1 404"),
        "`package.json` is vendored as PROVENANCE only and is not part of the browser \
         payload, so the route must 404 it:\n{head}"
    );
}
