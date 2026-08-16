//! `build` and `preview` render a *project*, and a project is what `_site.yml` declares.
//! A bare directory is refused with guidance, the same stance `read` already takes
//! (`read_of_a_non_site_directory_is_rejected_with_guidance` in `read_book.rs`).
//!
//! `a_standalone_document_builds_and_previews_without_site_chrome` additionally drives a
//! real `preview` over HTTP (the only way to reach `page_chrome` for a document with no
//! ancestor `_site.yml`; `build` never constructs a `SiteCtx` at all). It duplicates the
//! free-port/`Server`/`http_get` shape of a live-preview test, a separate test
//! binary, so nothing there can be imported — and picks its own port band, disjoint from
//! that file's and from `preview_single_instance.rs`'s (see [`PORT_FLOOR`]).

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

/// Like [`run`], but with the child's own working directory set to `dir` — needed to hand
/// the binary a path that is genuinely relative (no baked-in `CARGO_MANIFEST_DIR`
/// prefix), which is the only way to observe whether it echoes a path back in the
/// caller's own spelling or silently resolves it to an absolute one.
fn run_in(dir: &str, args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run taliesin");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// This crate's repo root (`crates/server/../..`), as a genuinely relative-friendly base
/// to run a child process from.
fn repo_root() -> String {
    format!("{}/../..", env!("CARGO_MANIFEST_DIR"))
}

/// Every continuation line of a `log::error` message hangs under its 10-column tag
/// gutter (`"  " + a 7-wide tag + " "`), so a multi-line error reads as one block
/// instead of half a message sitting flush against the left margin. `first` is the
/// index of the message's own first line within `stderr` (the caller may have other
/// log lines, or a `serve: ` prefix, ahead of it).
fn assert_continuations_hang_under_the_gutter(stderr: &str, first_line_needle: &str) {
    let lines: Vec<&str> = stderr.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.contains(first_line_needle))
        .unwrap_or_else(|| panic!("stderr must contain {first_line_needle:?}: {stderr}"));
    for cont in &lines[start + 1..] {
        assert!(
            cont.starts_with("          "),
            "continuation line must hang under the 10-column gutter, not sit flush left: \
             {cont:?} in {stderr}"
        );
    }
}

#[test]
fn build_of_a_non_project_directory_is_rejected_with_guidance() {
    let (ok, _out, stderr) = run(&["build", &corpus("agent")]);
    assert!(!ok, "a bare directory (no _site.yml) must fail");
    assert!(stderr.contains("no _site.yml"), "says why: {stderr}");
    assert!(
        stderr.contains("<page>.tmd"),
        "offers the name-one-document fix: {stderr}"
    );
    assert!(
        stderr.contains("add a _site.yml"),
        "offers the make-it-a-project fix: {stderr}"
    );
    assert_continuations_hang_under_the_gutter(&stderr, "has no _site.yml");
}

#[test]
fn build_of_a_subdirectory_of_a_project_names_the_project() {
    let (ok, _out, stderr) = run(&["build", &corpus("tech-blog/posts")]);
    assert!(!ok, "a project subdirectory is not itself a project");
    assert!(
        stderr.contains("tech-blog") && stderr.contains("did you mean"),
        "leads with the enclosing project: {stderr}"
    );
}

#[test]
fn build_of_a_real_project_still_works() {
    let (ok, _out, stderr) = run(&["build", &corpus("shared-bib"), "--no-exec"]);
    assert!(
        ok,
        "a directory WITH _site.yml still builds; stderr: {stderr}"
    );
}

#[test]
fn preview_of_a_non_project_directory_is_rejected_with_guidance() {
    // Must fail before binding a port, so this returns rather than serving forever.
    let (ok, _out, stderr) = run(&["preview", &corpus("agent"), "4399"]);
    assert!(!ok, "a bare directory (no _site.yml) must fail");
    assert!(stderr.contains("no _site.yml"), "says why: {stderr}");
    assert!(
        stderr.contains("<page>.tmd"),
        "offers the name-one-document fix: {stderr}"
    );
    assert_continuations_hang_under_the_gutter(&stderr, "has no _site.yml");
}

/// Defect regression: `preview`'s `resolve_target` used to canonicalize its root
/// BEFORE building this message, so the same non-project directory read back as the
/// as-typed path from `build` and a fully resolved absolute path from `preview` — the
/// two verbs disagreeing about the very same input, which is exactly what this branch
/// (`build`/`preview` refuse a non-project directory identically) must not do. Both
/// must now echo the directory exactly as the author typed it.
#[test]
fn build_and_preview_report_the_same_path_for_the_same_directory() {
    // Deliberately NOT canonical (carries a `../../`), so a regression that
    // resolves the path before rendering the message would visibly disagree.
    let dir = corpus("agent");
    let expected_line = format!("{dir} has no _site.yml, so it is not a project.");

    let (build_ok, _o, build_err) = run(&["build", &dir]);
    assert!(!build_ok, "a bare directory (no _site.yml) must fail");
    assert!(
        build_err.contains(&expected_line),
        "build must echo the path exactly as typed: {build_err}"
    );

    let (preview_ok, _o, preview_err) = run(&["preview", &dir, "4399"]);
    assert!(!preview_ok, "a bare directory (no _site.yml) must fail");
    assert!(
        preview_err.contains(&expected_line),
        "preview must echo the SAME path as build, not a canonicalized one: {preview_err}"
    );
}

/// The other half of Defect B: the subject path was already fixed to echo as typed, but
/// the ANCESTOR named in the "did you mean" suggestion (`taliesin_core::site::
/// enclosing_site_root`) canonicalizes internally, so it used to stay absolute even when
/// the subject was relative — mixing a relative subject with an absolute suggested fix
/// (a 50-character absolute command where `taliesin build corpus/tech-blog` would do).
/// Run with the child's cwd set to the repo root so `corpus/tech-blog/posts` is a
/// genuinely relative argument, not one built from an absolute `CARGO_MANIFEST_DIR`.
#[test]
fn build_and_preview_report_the_ancestor_in_the_same_spelling_as_the_subject() {
    let root = repo_root();
    let dir = "corpus/tech-blog/posts";

    let (build_ok, _o, build_err) = run_in(&root, &["build", dir]);
    assert!(!build_ok, "a project subdirectory is not itself a project");
    assert!(
        build_err.contains("its ancestor corpus/tech-blog is a project"),
        "the ancestor must be spelled the same way the (relative) subject was: {build_err}"
    );
    assert!(
        build_err.contains("taliesin build corpus/tech-blog"),
        "the suggested command must use the same relative spelling: {build_err}"
    );
    assert!(
        !build_err.contains(env!("CARGO_MANIFEST_DIR")),
        "must not fall back to an absolute path when the subject was relative: {build_err}"
    );

    let (preview_ok, _o, preview_err) = run_in(&root, &["preview", dir, "4399"]);
    assert!(
        !preview_ok,
        "a project subdirectory is not itself a project"
    );
    assert!(
        preview_err.contains("its ancestor corpus/tech-blog is a project"),
        "preview must render the ancestor the same way build does: {preview_err}"
    );
    assert!(
        preview_err.contains("taliesin preview corpus/tech-blog"),
        "the suggested command must use the same relative spelling: {preview_err}"
    );
    assert!(
        !preview_err.contains(env!("CARGO_MANIFEST_DIR")),
        "must not fall back to an absolute path when the subject was relative: {preview_err}"
    );
}

// ---------------------------------------------------------------------------
// A real `preview`, over real HTTP
// ---------------------------------------------------------------------------
//
// `build <file.tmd>` (`build.rs` -> `render_doc_to_page` -> `page.rs` with `site: None`)
// never constructs a `SiteCtx` and never reaches `page_chrome`'s `navbar_html: if book ||
// self.standalone` gate. The bug this whole plan exists to catch lived in `preview
// <file.tmd>`'s `Site::discover_single` -> `page_chrome` path
// (`serve_site/mod.rs::site_page_html`'s `let chrome = { project.site.lock().page_chrome
// (page) };`), so only a test that drives `preview` over HTTP can regress-guard it. The
// `build` half below stays as the control: it pins the half of the contract that was
// never broken, so the test as a whole says "preview matches build", not merely "preview
// has no chrome".

/// A port band clear of this crate's other live-server test binaries —
/// this file (15,000..~21,000) and `preview_single_instance.rs` /
/// `run_session_discovery.rs` (21,000..~31,500) — and clear of the 4321 default a real
/// preview a developer is running might be on.
const PORT_FLOOR: u16 = 10_000;

/// How far past its requested port a preview walks when that port is taken
/// (`serve/mod.rs`: `for p in port+1..=port+9`).
const FALLBACK_SPAN: u16 = 9;

const SLOT: u16 = 16;
const BAND: u16 = 64;
const STRIDE: u16 = 256;

static NEXT_BAND: AtomicU16 = AtomicU16::new(0);

fn port_is_free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// A port no other test in this binary and no recent run of it will ask for. Only
/// *peeked* at, never acquired — [`start`]'s identity check is what actually proves the
/// server answering here is ours.
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
    fn spawn(doc: &Path, port: u16) -> Server {
        let child = Command::new(env!("CARGO_BIN_EXE_taliesin"))
            .arg("preview")
            .arg(doc)
            .arg(port.to_string())
            // Mirrors the build half's `--no-exec`: this test pins chrome, not code-cell
            // execution, and must not depend on a Jupyter kernel being installed.
            .arg("--no-exec")
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

/// Bring a preview of a single out-of-project document up and prove the server answering
/// on `port` is ours.
///
/// `doc` is a *document*, not a directory. Per `Resolved::session_key`'s doc comment
/// (`serve_site/mod.rs`), an out-of-project document publishes ITS OWN canonical path as
/// identity, not its parent directory — a project of just that document, not the
/// directory it happens to sit in — so the identity check below compares against the
/// document itself, the same way the live-preview probes compare against a
/// project root.
fn start(doc: &Path) -> (Server, u16) {
    let port = free_port();
    let server = Server::spawn(doc, port);
    let canonical = fs::canonicalize(doc).unwrap();
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
    panic!("preview of {} never came up on {port}", doc.display());
}

/// The contract: for a document with no ancestor `_site.yml`, what `preview` serves and
/// what `build` writes carry the same chrome. `build` never had the bug (it never reaches
/// `page_chrome` at all), so its half below is the control; the `preview` half is what
/// actually pins the "Home" button regression, which lived in `preview`'s `page_chrome`
/// gate, not `build`'s render path.
#[test]
fn a_standalone_document_builds_and_previews_without_site_chrome() {
    let doc = corpus("agent/executed-read.tmd");

    // Control: `build <file.tmd>` constructs no `SiteCtx`, so this half was never able to
    // regress the way `preview` did; it stays green throughout as the reference point.
    let out = std::env::temp_dir().join(format!(
        "tali-standalone-chrome-{}.html",
        std::process::id()
    ));
    let out_s = out.to_string_lossy().into_owned();
    let (ok, _o, stderr) = run(&["build", &doc, &out_s, "--no-exec"]);
    assert!(ok, "single-document build; stderr: {stderr}");
    let built = std::fs::read_to_string(&out).expect("built page");
    let _ = std::fs::remove_file(&out);
    for marker in [
        "tali-site-nav",
        "tali-nav-brand",
        "tali-nav-burger",
        "tali-site-footer",
    ] {
        assert!(
            !built.contains(marker),
            "a standalone BUILD must carry no `{marker}`"
        );
    }
    // And no theme control either. It never survived the build as a CONTROL: only the
    // preview dev menu creates the button, and a build ships no client. This asserted the
    // OPPOSITE until 2026-08-16 and passed, because `theme_head` inlined the button-wiring
    // helper into every page and its `querySelectorAll("[data-tali-theme-toggle]")` held
    // this needle — so the assertion was reading 1.7 kB of JS that could never fire as a
    // shipped feature. `docs/guide/reference/cli.tmd` made the same mistake from the same
    // source comment and promised readers a build-time toggle. A built page follows the
    // reader's device and offers nothing to click (docs/guide/reference/accessibility.tmd).
    assert!(
        !built.contains("tali-theme-toggle"),
        "a standalone BUILD must carry no theme control: the page follows the reader's device"
    );

    // The assertion that actually discriminates: `preview` of the same document, over
    // real HTTP, through the `page_chrome` gate `preview` runs and `build` does not.
    //
    // Checked as the literal `<header>`/`<footer>` opening tags and the brand/burger
    // ids, not the bare class names used above: `preview` unconditionally inlines the
    // full site CSS on every page ("Live preview always ships everything",
    // `serve_site/mod.rs::site_page_html`'s `with_site_css: true`), so `.tali-site-nav {
    // … }` and friends are always present as CSS *rules*, chrome-free page or not — a
    // bare `contains("tali-site-nav")` is true either way and cannot discriminate here
    // (confirmed empirically: it matches this exact page's CSS bundle even on the
    // correctly-fixed build). Only the markup-level open tags/ids appear exclusively
    // when the elements are actually emitted.
    let (_server, port) = start(Path::new(&doc));
    let (status, served) = get(port, "/");
    assert_eq!(status, 200, "the standalone document's own page");
    for marker in [
        "<header class=\"tali-site-nav\"",
        "class=\"tali-nav-brand\"",
        "id=\"tali-nav-toggle\"",
        "<footer class=\"tali-site-footer\"",
    ] {
        assert!(
            !served.contains(marker),
            "a standalone PREVIEW must carry no `{marker}`"
        );
    }
    // The dev menu's quick toggle stays, and this is the half where it is real. It is not a
    // reader affordance (there are none); it is an authoring control, and the preview is the
    // only place it exists. The preview inlines `web-client/client.js`, so this needle
    // matches the code that CREATES the button — the button is made at runtime and cannot
    // appear in served HTML at all. Browser-verified 2026-08-16: it renders in the dev menu,
    // flips the mode, and syncs its icon and `aria-label`.
    assert!(
        served.contains("tali-theme-toggle"),
        "the dev menu's theme toggle must still be wired in preview"
    );
}
