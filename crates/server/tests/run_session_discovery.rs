//! `taliesin run` has to *find* the session that serves its document.
//!
//! Discovery is two agreeing derivations of one key: the server writes a hint file named
//! after the root it serves and answers `/__taliesin` with that same root, and the client
//! looks the hint up under the key it derives for the same document. Nothing type-checks
//! that agreement, and when it broke there was no test to notice: `run` waited out its
//! full 45-second start timeout and reported that a session it had itself started had
//! never come up.
//!
//! That is what happened. Folding the single-document server into `serve_site` moved an
//! out-of-project document's published root from the *document* to the *directory it sits
//! in*, while `run_cmd` went on asking for the document. The catalogue's "run: pin 0
//! integration tests" is what this file buys, and it goes through the real binary because
//! the failure lives between two processes.
//!
//! `XDG_RUNTIME_DIR` is redirected into the test's own tree so the hint files here are
//! this test's and no session of the developer's can answer for one of them.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Mirrors `run_cmd::SESSION_READY_TIMEOUT`. A run that attaches must be nowhere near
/// this; a run that has to wait it out is the regression.
const SESSION_READY_TIMEOUT: Duration = Duration::from_secs(45);

fn taliesin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_taliesin"))
}

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-runsess-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A port nothing else in this binary is using. Same reasoning as
/// `preview_single_instance.rs`: above the default preview port so a developer's own
/// server cannot collide, below the ephemeral range so the kernel never hands it out.
fn free_port() -> u16 {
    for p in 26_000..26_400 {
        if TcpListener::bind(("127.0.0.1", p)).is_ok() {
            return p;
        }
    }
    panic!("no free port");
}

/// Minimal loopback GET. The crate has no HTTP client dependency and this needs one
/// request, so hand-roll it (as `preview_single_instance.rs` does).
fn http_get(port: u16, path: &str) -> Option<String> {
    let mut sock = TcpStream::connect(("127.0.0.1", port)).ok()?;
    sock.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    sock.write_all(
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").as_bytes(),
    )
    .ok()?;
    let mut raw = String::new();
    sock.read_to_string(&mut raw).ok()?;
    let (_head, body) = raw.split_once("\r\n\r\n")?;
    Some(body.to_string())
}

fn identity(port: u16) -> Option<serde_json::Value> {
    let body = http_get(port, "/__taliesin")?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    // A bound socket answers from the listen backlog before the handlers are installed,
    // so "it connected" is not "it is up"; a `pid` in the body is.
    (!v["pid"].is_null()).then_some(v)
}

fn wait_until_ready(port: u16, dur: Duration) -> bool {
    let deadline = Instant::now() + dur;
    while Instant::now() < deadline {
        if identity(port).is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// A spawned session, always reaped: a leaked dev server holds a port and a kernel
/// subtree, and these tests exist because sessions are hard to find again.
struct Session(Child);

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The test's private hint directory, and every session that has registered in it.
///
/// `taliesin run` spawns its session *detached* — nothing reaps it — so when discovery
/// fails the run leaves a live server behind. Redirecting `XDG_RUNTIME_DIR` makes those
/// strays enumerable: every session in this tree registered here, so the ports in this
/// directory are exactly the ones the test is responsible for.
struct HintDir(PathBuf);

impl HintDir {
    fn new(base: &Path) -> HintDir {
        let d = base.join("xdg");
        fs::create_dir_all(&d).unwrap();
        HintDir(d)
    }

    fn ports(&self) -> Vec<u16> {
        let Ok(entries) = fs::read_dir(self.0.join("taliesin")) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|e| fs::read_to_string(e.path()).ok())
            .filter_map(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .filter_map(|v| v["port"].as_u64())
            .filter_map(|p| u16::try_from(p).ok())
            .collect()
    }

    /// SIGKILL any session registered here other than `keep` (the child the test owns and
    /// reaps itself).
    fn reap_strays(&self, keep: u32) {
        for port in self.ports() {
            let Some(pid) = identity(port).and_then(|v| v["pid"].as_u64()) else {
                continue;
            };
            if pid != u64::from(keep) {
                // SAFETY: a pid that just identified itself over loopback as a taliesin
                // preview registered in this test's own private hint directory.
                unsafe { libc::kill(pid as i32, libc::SIGKILL) };
            }
        }
    }
}

/// The regression, end to end: a run must attach to the session already serving its
/// document rather than starting a second one and then failing to find that.
///
/// Two independent assertions, because either alone can pass for the wrong reason. A run
/// that *starts* a session also eventually succeeds, so only the absence of the start
/// notice proves it attached; and a run that attaches is instant, so the elapsed bound is
/// what catches a discovery that succeeds only after waiting something out.
#[test]
fn a_run_attaches_to_the_session_already_serving_an_out_of_project_document() {
    let dir = tmp_dir("attach");
    let hints = HintDir::new(&dir);
    // No `_site.yml`: this is the "project of just that document" path, the one that
    // broke. No code cells either, so the claim under test is discovery and no kernel is
    // needed to reach it.
    let doc = dir.join("scratch.tmd");
    fs::write(&doc, "---\ntitle: Scratch\n---\n\nProse.\n").unwrap();
    let port = free_port();

    let session = Session(
        taliesin()
            .arg("preview")
            .arg(&doc)
            .arg(port.to_string())
            .arg("--__session")
            .env("XDG_RUNTIME_DIR", &hints.0)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn session"),
    );
    assert!(
        wait_until_ready(port, Duration::from_secs(30)),
        "the session never came up on {port}"
    );

    let t0 = Instant::now();
    let out = taliesin()
        .arg("run")
        .arg(&doc)
        .env("XDG_RUNTIME_DIR", &hints.0)
        .output()
        .expect("run");
    let elapsed = t0.elapsed();
    hints.reap_strays(session.0.id());

    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        !stderr.contains("starting a session"),
        "the run started a SECOND session instead of attaching to the one on {port}; \
         its stderr was:\n{stderr}"
    );
    assert!(
        elapsed < SESSION_READY_TIMEOUT / 3,
        "the run took {elapsed:?}; attaching to a live session is immediate, so this is \
         waiting a start timeout out"
    );
    assert!(
        out.status.success(),
        "run exited {:?}; stderr:\n{stderr}",
        out.status.code()
    );
}

/// The server-side half of the same contract, asserted directly so a regression names its
/// cause instead of only its symptom.
///
/// A server serving one out-of-project document identifies as **that document**. It is a
/// "project of just that document" (`Site::discover_single`), so the document is what it
/// serves, and the directory it happens to sit in may hold unrelated `.tmd` files this
/// server knows nothing about — answering with the directory claims pages it would 404.
#[test]
fn a_session_for_an_out_of_project_document_identifies_as_that_document() {
    let dir = tmp_dir("identity");
    let hints = HintDir::new(&dir);
    let doc = dir.join("scratch.tmd");
    fs::write(&doc, "---\ntitle: Scratch\n---\n\nProse.\n").unwrap();
    // A sibling this session does NOT serve: it is what makes the directory the wrong
    // answer rather than merely a broader one.
    fs::write(dir.join("other.tmd"), "---\ntitle: Other\n---\n\nProse.\n").unwrap();
    let port = free_port();

    let _session = Session(
        taliesin()
            .arg("preview")
            .arg(&doc)
            .arg(port.to_string())
            .arg("--__session")
            .env("XDG_RUNTIME_DIR", &hints.0)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn session"),
    );
    assert!(
        wait_until_ready(port, Duration::from_secs(30)),
        "the session never came up on {port}"
    );

    let json = identity(port).expect("identity");
    assert_eq!(
        json["root"].as_str().map(Path::new),
        Some(fs::canonicalize(&doc).unwrap().as_path()),
        "a single-document session must publish the document it serves: {json}"
    );
    assert_eq!(
        hints.ports(),
        vec![port],
        "the hint must be registered under that same key, and be the only one"
    );
}
