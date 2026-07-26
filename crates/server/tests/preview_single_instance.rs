//! One preview per root, and a clean death when the terminal goes away.
//!
//! Two process-level behaviors the port-fallback in `bind_with_fallback` used to get
//! wrong. Re-running `taliesin preview <root>` on a root that is *already* being
//! previewed used to bind the next free port instead, so a forgotten preview stayed
//! alive and every stacked instance kept its own watcher + kernel subtree re-executing
//! the same files. Taking the port over instead keeps the invariant "one preview per
//! root" and hands back the canonical URL. A preview of a *different* root is a
//! legitimate second server, so that case must still fall back.
//!
//! Separately, SIGHUP (what a closing terminal tab sends) had no handler, so the
//! default disposition hard-killed the process without running the teardown that reaps
//! the kernel subtree: the exact leak `shutdown_signal` exists to prevent for
//! SIGINT/SIGTERM.
//!
//! These go through the real binary (`CARGO_BIN_EXE_taliesin`): binding, signals and
//! one server terminating another are not observable from a unit test.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-oneinst-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn taliesin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_taliesin"))
}

fn port_is_free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

static NEXT_BAND: AtomicU16 = AtomicU16::new(0);

/// The floor of the port bands: well above the 4321 default, so a test never collides
/// with a real preview the developer has running, and below the ephemeral range the
/// kernel allocates outbound source ports from (`net.ipv4.ip_local_port_range`,
/// 32768-60999 by default on Linux). The bands used to start at 47,000, inside that
/// range, which is a standing hazard for any bind-then-release probe like
/// [`port_is_free`] — though it was *not* the cause of the flake below, which was
/// diagnosed separately and reproduced from 21,000 too.
const PORT_FLOOR: u16 = 21_000;

/// How far past its requested port a preview walks when that port is taken
/// (`serve/mod.rs`: `for p in port+1..=port+9`), so a slot has to be wider than this or
/// one test's preview lands on the ports `free_run` promised another.
const FALLBACK_SPAN: u16 = 9;

/// Ports reserved per test: the longest run any test asks for plus [`FALLBACK_SPAN`].
const SLOT: u16 = 16;

/// Ports reserved per band, i.e. per `free_run` caller.
const BAND: u16 = 128;

/// Bands per process-offset stride. Room for every band this binary allocates, with the
/// whole stride shifted per process — see [`free_run`].
const STRIDE: u16 = 640;

/// Find `count` consecutive free ports in a range no *other test in this binary* and no
/// *recent run of this binary* will touch.
///
/// Two separate isolation problems, both measured 2026-07-26 while chasing a ~1-in-15
/// flake in `a_port_holder_naming_another_process_does_not_get_it_signalled`:
///
/// 1. **Within a run.** These tests run in parallel, so each `free_run` caller gets its
///    own band, and the slot stride clears [`FALLBACK_SPAN`] so a preview walking its
///    fallback range cannot leave its own slot.
/// 2. **Across runs**, which is the pressure behind the flake. The bands were fixed, so
///    every run of the binary asked for the ports the previous run had just finished
///    with, and those are still encumbered: `wait_until_ready` polls with
///    `Connection: close` every 50 ms, so a port that served a preview is left carrying
///    dozens of TIME-WAIT sockets (48 of them on the captured failure). The range is
///    therefore shifted by pid, so consecutive runs land somewhere else.
///
/// The shifted range stays below the ephemeral floor (32768 by default on Linux), so the
/// kernel never hands one of these ports out as an outbound source port either.
///
/// **None of this makes a returned port *acquired*.** [`port_is_free`] binds and releases,
/// so every port here has only been peeked at, and a caller that binds it later can still
/// lose. Shifting the range cut the flake but did not remove it (4 in 60); only
/// [`spawn_liar`] acquiring its own port did (0 in 80). A caller that must *hold* a port
/// should bind it itself.
fn free_run(count: u16) -> u16 {
    assert!(
        SLOT > FALLBACK_SPAN + count,
        "a slot must hold a {count}-port run plus the preview's {FALLBACK_SPAN}-port \
         fallback walk, or bands stop isolating tests"
    );
    let band = NEXT_BAND.fetch_add(1, Ordering::SeqCst);
    let run_offset = (std::process::id() as u16 % 16) * STRIDE;
    let start = PORT_FLOOR + run_offset + band * BAND;
    for base in (start..start + BAND).step_by(SLOT as usize) {
        if (0..count).all(|i| port_is_free(base + i)) {
            return base;
        }
    }
    panic!("no free port run available in band {band}");
}

/// A spawned preview that is always reaped, even when an assertion panics, since a
/// leaked dev server is precisely the failure these tests exist to prevent.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Server {
    fn spawn(path: &Path, port: u16) -> Server {
        let child = taliesin()
            .arg("preview")
            .arg(path)
            .arg(port.to_string())
            // The preview clears the screen and streams a banner; keep the test output
            // clean and make sure a full pipe can never block the child.
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn preview");
        Server(child)
    }

    fn pid(&self) -> u32 {
        self.0.id()
    }

    fn exited_within(&mut self, dur: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + dur;
        while Instant::now() < deadline {
            match self.0.try_wait().expect("try_wait") {
                Some(status) => return Some(status),
                None => std::thread::sleep(Duration::from_millis(50)),
            }
        }
        None
    }
}

/// Wait until the preview *answers*, not merely until the port accepts. A connect
/// succeeds from the listen backlog as soon as the socket is bound, which is before
/// `axum::serve` is racing `shutdown_signal`, so a signal sent on the strength of a
/// bare connect can land while SIGHUP still has its default disposition. Getting a
/// reply proves the select loop is running and the handlers are installed.
fn wait_until_ready(port: u16, dur: Duration) -> bool {
    let deadline = Instant::now() + dur;
    while Instant::now() < deadline {
        if let Some(body) = http_get(port, "/__taliesin")
            && serde_json::from_str::<serde_json::Value>(&body).is_ok_and(|v| !v["pid"].is_null())
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Minimal loopback HTTP/1.1 GET; returns the response body. The crate has no HTTP
/// client dependency and this needs one request, so hand-roll it.
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

fn write_site(dir: &Path) {
    fs::write(dir.join("_site.yml"), "title: Book\n").unwrap();
    fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nProse.\n").unwrap();
}

/// The identity endpoint is what lets a second launch tell "this root is already being
/// previewed" from "some other server owns that port", so it must report the canonical
/// root it is serving and its own pid.
#[test]
fn identity_endpoint_reports_the_served_root_and_pid() {
    let dir = tmp_dir("identity");
    write_site(&dir);
    let port = free_run(1);

    let server = Server::spawn(&dir, port);
    assert!(
        wait_until_ready(port, Duration::from_secs(30)),
        "preview never came up on {port}"
    );

    let body = http_get(port, "/__taliesin").expect("identity request");
    let json: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("identity body was not JSON: {e}\n{body}"));

    let canonical = fs::canonicalize(&dir).unwrap();
    assert_eq!(
        json["root"].as_str().map(Path::new),
        Some(canonical.as_path()),
        "identity reports the canonical root it serves: {json}"
    );
    assert_eq!(
        json["pid"].as_u64(),
        Some(server.pid() as u64),
        "identity reports its own pid: {json}"
    );
}

/// The regression: a second preview of the same root must take the port over, leaving
/// exactly one server, rather than stacking on the next free port.
#[test]
fn second_preview_of_the_same_root_takes_over_the_port() {
    let dir = tmp_dir("takeover");
    write_site(&dir);
    let port = free_run(2);

    let mut first = Server::spawn(&dir, port);
    assert!(
        wait_until_ready(port, Duration::from_secs(30)),
        "first preview never came up on {port}"
    );

    let _second = Server::spawn(&dir, port);

    let status = first
        .exited_within(Duration::from_secs(30))
        .expect("the first preview is terminated by the second");
    assert_eq!(
        status.code(),
        Some(0),
        "the replaced preview shuts down gracefully (so it reaps its kernel), got {status:?}"
    );

    assert!(
        wait_until_ready(port, Duration::from_secs(30)),
        "the second preview holds the canonical port {port}"
    );
    assert!(
        port_is_free(port + 1),
        "the second preview took the port over instead of falling back to {}",
        port + 1
    );
}

/// The guard on the takeover: a *different* root is a legitimate concurrent preview, so
/// the historical next-free-port fallback must survive unchanged.
#[test]
fn preview_of_a_different_root_still_falls_back_to_the_next_port() {
    let one = tmp_dir("fallback-a");
    let two = tmp_dir("fallback-b");
    write_site(&one);
    write_site(&two);
    let port = free_run(2);

    let mut first = Server::spawn(&one, port);
    assert!(
        wait_until_ready(port, Duration::from_secs(30)),
        "first preview never came up on {port}"
    );

    let _second = Server::spawn(&two, port);
    assert!(
        wait_until_ready(port + 1, Duration::from_secs(30)),
        "a preview of another root falls back to {}",
        port + 1
    );
    assert!(
        first.exited_within(Duration::from_secs(2)).is_none(),
        "a preview of another root is left running"
    );
}

/// SIGHUP is what a closing terminal tab delivers. Without a handler the default
/// disposition kills the process outright, skipping the teardown that reaps the kernel
/// subtree; a graceful shutdown exits 0 instead of dying by signal.
#[test]
fn sighup_shuts_down_gracefully_rather_than_killing_the_process() {
    let dir = tmp_dir("sighup");
    write_site(&dir);
    let port = free_run(1);

    let mut server = Server::spawn(&dir, port);
    assert!(
        wait_until_ready(port, Duration::from_secs(30)),
        "preview never came up on {port}"
    );

    // SAFETY: `kill` on a pid this test owns and has not yet reaped.
    unsafe { libc::kill(server.pid() as libc::pid_t, libc::SIGHUP) };

    let status = server
        .exited_within(Duration::from_secs(30))
        .expect("SIGHUP shuts the preview down");
    assert_eq!(
        status.code(),
        Some(0),
        "SIGHUP runs the graceful path (exit 0); dying by signal {:?} leaks the kernel subtree",
        status.signal()
    );
}

/// Hold a port and answer the identity probe with whatever we like — any local user can
/// bind a loopback port, so the probe's answer is untrusted input. Returns the port it
/// actually acquired.
///
/// **It binds first and reports second, rather than being handed a port.** A port from
/// [`free_run`] has only been *peeked* at (bound and released), and this is the caller
/// that binds last, so it was the one that lost the peek-then-bind race: a ~1-in-15 flake
/// whose captured failure showed the port carrying 48 TIME-WAIT sockets from the previous
/// run's polling. Narrowing that window kept the flake (4 in 60 with the range shifted per
/// process); closing it is what removes it. Same lesson as `Kernel::start_with_retry`:
/// a free-port *peek* is not an acquisition, so acquire and then report.
fn spawn_liar(band_base: u16, root: &Path, claimed_pid: i32) -> u16 {
    // The successor has to be free too — that is where the preview must land — but only
    // the liar's own port needs to be *held*, and an unusable successor just moves the
    // search along instead of failing the test.
    let (listener, port) = (band_base..band_base + SLOT)
        .find_map(|p| {
            let l = TcpListener::bind(("127.0.0.1", p)).ok()?;
            port_is_free(p + 1).then_some((l, p))
        })
        .unwrap_or_else(|| {
            panic!("no bindable port with a free successor in slot {band_base}..+{SLOT}")
        });
    let body = format!(
        "{{\"root\":\"{}\",\"pid\":{claimed_pid},\"version\":\"0.2.0\"}}",
        root.display()
    );
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { break };
            let _ = s.read(&mut [0u8; 1024]);
            let _ = s.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
        }
    });
    port
}

/// The takeover signals a pid it learned over a socket, so the pid is untrusted: a port
/// holder that names *someone else's* process must not get that process killed. The
/// answer is checked against the OS before anything is signalled.
#[test]
fn a_port_holder_naming_another_process_does_not_get_it_signalled() {
    let dir = tmp_dir("liar");
    write_site(&dir);
    let band_base = free_run(2);

    // A process of ours that must survive. `sleep` is not a taliesin preview, which is
    // exactly what the out-of-band check catches.
    let mut bystander = Command::new("sleep")
        .arg("300")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn bystander");
    let port = spawn_liar(
        band_base,
        &fs::canonicalize(&dir).unwrap(),
        bystander.id() as i32,
    );

    let _preview = Server::spawn(&dir, port);
    assert!(
        wait_until_ready(port + 1, Duration::from_secs(60)),
        "the preview steps past a port it could not verify, landing on {}",
        port + 1
    );

    let alive = bystander.try_wait().expect("try_wait").is_none();
    let _ = bystander.kill();
    let _ = bystander.wait();
    assert!(
        alive,
        "a process named by an unverified port holder was sent SIGTERM"
    );
}
