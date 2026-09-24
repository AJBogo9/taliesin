//! The time from saving a page to its open tab receiving the change, end to end: the real
//! `taliesin preview` binary, its file watcher, the re-render and the diff, and a websocket
//! client reading the ops the way the browser's does. The instrument behind the guide's
//! "save to the open tab" row (`docs/guide/using/choosing.tmd`), measured on the same post
//! `tools/live-edit-bench` times the render and diff of. Run it in release to reproduce that
//! figure:
//!
//! ```sh
//! cargo test --release -p taliesin-server --test save_to_open_tab -- --nocapture
//! ```
//!
//! The times are printed, not asserted: a wall clock measures the machine. What is asserted
//! is that every save reaches the tab, whether the editor writes the file in place or
//! renames a temp file over it.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The post `tools/live-edit-bench` measures, inside its project.
const PAGE: &str = "posts/em-algorithm/index.tmd";

/// Saves per style.
const SAVES: usize = 9;

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if matches!(name.as_ref(), "_site" | "_freeze") || name.starts_with('.') {
            continue;
        }
        let (src, dst) = (entry.path(), to.join(&*name));
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&src, &dst);
        } else {
            fs::copy(&src, &dst).unwrap();
        }
    }
}

/// A spawned preview, always reaped.
struct Preview(Child);

impl Drop for Preview {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The port the preview's banner names, once it has printed it.
fn port_from(log: &Path) -> Option<u16> {
    let text = fs::read_to_string(log).ok()?;
    let at = text.find("http://127.0.0.1:")? + "http://127.0.0.1:".len();
    let digits: String = text[at..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// A websocket to the page's live channel, handshake done, as the browser opens it.
fn connect(port: u16, rel: &str) -> TcpStream {
    let mut sock = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    write!(
        sock,
        "GET /ws?page={rel} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\n\
         Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n"
    )
    .unwrap();
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        sock.read_exact(&mut byte).expect("handshake");
        head.push(byte[0]);
    }
    assert!(
        head.starts_with(b"HTTP/1.1 101"),
        "{}",
        String::from_utf8_lossy(&head)
    );
    sock
}

/// The next text message on `sock`, or `None` when none arrives within `wait`. Server
/// frames are unmasked; a message may span continuation frames.
fn next_text(sock: &mut TcpStream, wait: Duration) -> Option<String> {
    sock.set_read_timeout(Some(wait)).unwrap();
    let mut message = Vec::new();
    loop {
        let mut head = [0u8; 2];
        sock.read_exact(&mut head).ok()?;
        let len = match head[1] & 0x7f {
            126 => {
                let mut b = [0u8; 2];
                sock.read_exact(&mut b).ok()?;
                u64::from(u16::from_be_bytes(b))
            }
            127 => {
                let mut b = [0u8; 8];
                sock.read_exact(&mut b).ok()?;
                u64::from_be_bytes(b)
            }
            n => u64::from(n),
        };
        let mut payload = vec![0u8; usize::try_from(len).unwrap()];
        sock.read_exact(&mut payload).ok()?;
        match head[0] & 0x0f {
            0 | 1 => message.extend(payload),
            8 => return None,
            _ => continue, // ping, pong
        }
        if head[0] & 0x80 != 0 {
            return Some(String::from_utf8_lossy(&message).into_owned());
        }
    }
}

/// Whether a message changes the page body: what the client repaints on.
fn is_op(message: &str) -> bool {
    ["update", "insert", "remove", "set_meta", "full_render"]
        .iter()
        .any(|t| message.contains(&format!("\"type\":\"{t}\"")))
}

/// Read until `quiet` passes with nothing arriving.
fn settle(sock: &mut TcpStream, quiet: Duration) {
    while next_text(sock, quiet).is_some() {}
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

#[test]
fn a_save_reaches_the_open_tab() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/tech-blog");
    let dir: PathBuf =
        std::env::temp_dir().join(format!("tali-save-to-tab-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    copy_tree(&corpus, &dir);
    let log = dir.with_extension("log");
    let child = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("preview")
        .arg(&dir)
        .arg("0")
        .arg("--no-exec")
        .env("TALIESIN_NO_CLEAR", "1")
        .stdout(Stdio::null())
        .stderr(fs::File::create(&log).unwrap())
        .spawn()
        .expect("spawn preview");
    let _preview = Preview(child);
    let deadline = Instant::now() + Duration::from_secs(60);
    let port = loop {
        if let Some(port) = port_from(&log) {
            break port;
        }
        assert!(
            Instant::now() < deadline,
            "the preview never printed its port"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut tab = connect(port, PAGE);
    // The opening snapshot, then the page's first build.
    settle(&mut tab, Duration::from_secs(2));

    let path = dir.join(PAGE);
    let base = fs::read_to_string(&path).unwrap();
    let (mut in_place, mut renamed) = (Vec::new(), Vec::new());
    for i in 0..SAVES * 2 {
        let edited = base.replacen("\n\n", &format!("\n\nSaved {i}.\n\n"), 1);
        let started = Instant::now();
        if i % 2 == 0 {
            fs::write(&path, &edited).unwrap();
        } else {
            let tmp = path.with_file_name(".index.tmd.save");
            fs::write(&tmp, &edited).unwrap();
            fs::rename(&tmp, &path).unwrap();
        }
        loop {
            let message = next_text(&mut tab, Duration::from_secs(10))
                .unwrap_or_else(|| panic!("save {i} never reached the open tab"));
            if is_op(&message) {
                break;
            }
        }
        let took = started.elapsed();
        if i % 2 == 0 {
            in_place.push(took);
        } else {
            renamed.push(took);
        }
        settle(&mut tab, Duration::from_millis(300));
    }
    eprintln!(
        "save to the open tab, {PAGE} of corpus/tech-blog: in place {:.1} ms, renamed over \
         {:.1} ms (medians of {SAVES} each)",
        median(in_place).as_secs_f64() * 1e3,
        median(renamed).as_secs_f64() * 1e3,
    );
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_file(&log);
}
