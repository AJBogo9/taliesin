//! Item 171: `mounts:` served over real HTTP by a real `taliesin preview`.
//!
//! The routing seam (`match_mount` / `resolve_project` / `classify_change`) is unit-pinned
//! in `serve_site/mod.rs` and the live wiring on top of it was browser-verified and
//! nothing else. That leaves the part those unit tests structurally cannot see: whether a
//! request that arrives at the *server* is answered by the project the prefix names. Four
//! things only a live request can decide, each of which the pure helpers would keep
//! passing through:
//!
//! 1. A mounted project's pages answer under the prefix at all, and the root still owns
//!    everything else.
//! 2. `search-index.js` is served **per project** off the route table, never written to
//!    disk in preview. `page_or_asset` resolves it *after* the mount split, so a wrong
//!    split silently hands a mounted page the root's index and Cmd-K goes quietly wrong
//!    rather than 404ing — the failure mode a status-code test cannot see.
//! 3. Longest-prefix wins between two mounts where one prefix prefixes the other.
//! 4. A miss inside a mount answers with **that project's** 404 page, not the root's.
//!
//! Bin-crate, so it drives `CARGO_BIN_EXE_taliesin` and hand-rolls its HTTP the way
//! `preview_single_instance.rs` does — the crate has no HTTP client dependency and this
//! needs a handful of GETs.
//!
//! **Rebuild before believing a failure**: this runs the built binary, and `cargo test`
//! does not rebuild it for you.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Ports
// ---------------------------------------------------------------------------

/// Floor of this binary's port bands. Deliberately **below** the 21,000 floor
/// `preview_single_instance.rs` uses: the two test binaries run concurrently under one
/// `cargo test`, and that file's range reaches ~31,000 once its per-process stride is
/// applied. Everything here stays under 21,000 and therefore also under the ephemeral
/// range (32768+ on Linux), so the kernel never hands one of these out as an outbound
/// source port either.
const PORT_FLOOR: u16 = 15_000;

/// How far past its requested port a preview walks when that port is taken
/// (`serve/mod.rs`: `for p in port+1..=port+9`).
const FALLBACK_SPAN: u16 = 9;

/// Ports per test, wider than the fallback walk so one test's preview cannot land in
/// another's slot.
const SLOT: u16 = 16;

/// Ports per band (one band per [`free_port`] caller).
const BAND: u16 = 64;

/// Per-process shift, so a re-run does not ask for the ports the previous run just left
/// covered in TIME-WAIT sockets. Room for every band this binary allocates.
const STRIDE: u16 = 384;

static NEXT_BAND: AtomicU16 = AtomicU16::new(0);

fn port_is_free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// A port no other test in this binary and no recent run of it will ask for.
///
/// **A returned port is only *peeked* at**, never acquired — `port_is_free` binds and
/// releases. That is why every caller re-checks identity against `/__taliesin` below
/// rather than assuming the server it talks to is its own.
fn free_port() -> u16 {
    let band = NEXT_BAND.fetch_add(1, Ordering::SeqCst);
    let start = PORT_FLOOR + (std::process::id() as u16 % 16) * STRIDE + band * BAND;
    for base in (start..start + BAND).step_by(SLOT as usize) {
        if (0..=FALLBACK_SPAN).all(|i| port_is_free(base + i)) {
            return base;
        }
    }
    panic!("no free port in band {band}");
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// A spawned preview that is always reaped, even when an assertion panics: a leaked dev
/// server holds a port and a kernel subtree.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Server {
    fn spawn(root: &Path, port: u16) -> Server {
        let child = Command::new(env!("CARGO_BIN_EXE_taliesin"))
            .arg("preview")
            .arg(root)
            .arg(port.to_string())
            // The preview clears the screen and streams a banner; keep the test output
            // clean and make sure a full pipe can never block the child.
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn preview");
        Server(child)
    }
}

/// One loopback HTTP/1.1 GET: `(status, body)`.
fn http_get(port: u16, path: &str) -> Option<(u16, String)> {
    let mut sock = TcpStream::connect(("127.0.0.1", port)).ok()?;
    sock.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
    sock.write_all(
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").as_bytes(),
    )
    .ok()?;
    // The body is HTML/JS, but a byte-lossy read is fine: every needle below is ASCII.
    let mut raw = Vec::new();
    sock.read_to_end(&mut raw).ok()?;
    let raw = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = raw.split_once("\r\n\r\n")?;
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())?;
    Some((status, body.to_string()))
}

fn get(port: u16, path: &str) -> (u16, String) {
    http_get(port, path).unwrap_or_else(|| panic!("GET {path} on {port} failed"))
}

/// Bring a preview up and prove the server answering on `port` is **ours**.
///
/// The identity check is not ceremony: [`free_port`] only peeks, so on a lost race the
/// preview walks to `port+1` and something else answers here. Without this a routing
/// regression and a port collision look identical.
fn start(root: &Path) -> (Server, u16) {
    let port = free_port();
    let server = Server::spawn(root, port);
    let canonical = fs::canonicalize(root).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if let Some((200, body)) = http_get(port, "/__taliesin")
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&body)
            && v["root"].as_str().map(Path::new) == Some(canonical.as_path())
        {
            return (server, port);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("preview of {} never came up on {port}", root.display());
}

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

/// A root site plus two sibling projects mounted under `/manual` and `/manual/deep`.
///
/// **Siblings, not children.** `collect_pages` does not stop at a nested `_site.yml`, so a
/// project inside the root's tree is walked as pages of the root as well — which is why
/// this repo's own books sit beside `site/` rather than under it. A nested fixture would
/// pass for the wrong reason.
///
/// The nesting of the two prefixes is the point of the second one: `/manual/deep/` matches
/// both `manual` and `manual/deep`, so only longest-prefix routing serves the right project.
fn fixture(tag: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("tali-mount-live-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let root = base.join("site");
    let manual = base.join("manual");
    let deep = base.join("deep");
    for d in [&root, &manual, &deep] {
        fs::create_dir_all(d).unwrap();
    }

    fs::write(
        root.join("_site.yml"),
        "title: RootSite\nmounts:\n  - at: manual\n    path: ../manual\n  \
         - at: manual/deep\n    path: ../deep\n",
    )
    .unwrap();
    // Every needle below is a coined word. A bare word like \"Home\" would be found in the
    // CSS/JS payload every page inlines, so a whole-page `contains` for it passes on a page
    // that rendered nothing.
    fs::write(
        root.join("index.tmd"),
        "---\ntitle: RootHomePage\n---\n\nRootbodyprose.\n",
    )
    .unwrap();

    fs::write(manual.join("_site.yml"), "title: ManualSite\n").unwrap();
    fs::write(
        manual.join("index.tmd"),
        "---\ntitle: ManualHomePage\n---\n\nManualbodyprose.\n",
    )
    .unwrap();
    fs::write(
        manual.join("chapter.tmd"),
        "---\ntitle: ManualChapterPage\n---\n\nManualchapterprose.\n",
    )
    .unwrap();
    fs::write(manual.join("notes.txt"), "manual-static-asset-body\n").unwrap();

    fs::write(deep.join("_site.yml"), "title: DeepSite\n").unwrap();
    fs::write(
        deep.join("index.tmd"),
        "---\ntitle: DeepHomePage\n---\n\nDeepbodyprose.\n",
    )
    .unwrap();

    root
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The base case: each project answers under its own prefix, and the root keeps
/// everything the mounts do not claim. Both directions are asserted on every response —
/// serving the root's page for a mounted URL is the regression, and it has a 200 status.
#[test]
fn a_mounted_project_serves_its_pages_under_the_prefix_and_the_root_keeps_the_rest() {
    let root = fixture("serve");
    let (_server, port) = start(&root);

    let (status, body) = get(port, "/");
    assert_eq!(status, 200, "root index");
    assert!(
        body.contains("Rootbodyprose"),
        "root index renders the root page"
    );

    for url in ["/manual/", "/manual/index.html"] {
        let (status, body) = get(port, url);
        assert_eq!(status, 200, "{url}");
        assert!(
            body.contains("Manualbodyprose"),
            "{url} is answered by the mounted project"
        );
        assert!(
            !body.contains("Rootbodyprose"),
            "{url} is NOT answered by the root project"
        );
    }

    let (status, body) = get(port, "/manual/chapter.html");
    assert_eq!(status, 200, "mounted non-index page");
    assert!(
        body.contains("Manualchapterprose"),
        "a mounted project's non-index page renders"
    );

    // A static file under the mounted project's root, reached through the prefix.
    let (status, body) = get(port, "/manual/notes.txt");
    assert_eq!(status, 200, "mounted static asset");
    assert!(
        body.contains("manual-static-asset-body"),
        "a mounted project's static asset is served from ITS directory"
    );
}

/// `search-index.js` is route-served per project and never written to disk in preview, so
/// it resolves *after* the mount split. A wrong split hands a mounted page the root's
/// index: Cmd-K on a mounted page then finds the wrong project's pages while every status
/// code stays 200. This is the assertion that discriminates, and it needs both halves —
/// the mounted title present AND the root's absent.
#[test]
fn each_project_serves_its_own_search_index_under_its_own_prefix() {
    let root = fixture("search");
    let (_server, port) = start(&root);

    let (status, mounted) = get(port, "/manual/search-index.js");
    assert_eq!(status, 200, "mounted search index");
    assert!(
        mounted.contains("ManualChapterPage"),
        "the mounted prefix serves the MOUNTED project's search index: {mounted}"
    );
    assert!(
        !mounted.contains("RootHomePage"),
        "the mounted prefix does not serve the ROOT's search index: {mounted}"
    );

    let (status, rootidx) = get(port, "/search-index.js");
    assert_eq!(status, 200, "root search index");
    assert!(
        rootidx.contains("RootHomePage"),
        "the root still serves its own search index: {rootidx}"
    );
    assert!(
        !rootidx.contains("ManualChapterPage"),
        "the root's index does not absorb a mounted project's pages: {rootidx}"
    );
}

/// `/manual/deep/` matches both mount prefixes. Longest-prefix must win over the *live*
/// request, not merely over a `&[String]` in a unit test.
#[test]
fn the_longest_matching_mount_prefix_wins_over_a_live_request() {
    let root = fixture("longest");
    let (_server, port) = start(&root);

    let (status, body) = get(port, "/manual/deep/");
    assert_eq!(status, 200, "deeper mount index");
    assert!(
        body.contains("Deepbodyprose"),
        "the deeper prefix wins: {}",
        &body[..body.len().min(400)]
    );
    assert!(
        !body.contains("Manualbodyprose"),
        "the shorter prefix did not swallow the deeper mount's request"
    );
}

/// A miss inside a mount is that project's miss. Answering it from the root's 404 shows
/// the wrong chrome and the wrong nav for the section the reader is in — and, like the
/// search-index case, does it while looking healthy.
#[test]
fn a_miss_inside_a_mount_answers_with_that_projects_404_not_the_roots() {
    let root = fixture("notfound");
    let (_server, port) = start(&root);

    let (status, body) = get(port, "/manual/no-such-page.html");
    assert_eq!(
        status, 404,
        "a miss inside a mount is a 404, so preview mirrors the deployed 404.html"
    );
    assert!(
        body.contains("ManualSite"),
        "the mounted project's own 404 page answers: {}",
        &body[..body.len().min(400)]
    );
    assert!(
        !body.contains("RootSite"),
        "the ROOT's 404 page did not answer for a path inside a mount"
    );
}
