//! End to end: what an author waits for after a save, measured against the real binary.
//!
//! The in-process rows time the passes a save runs; these time the save itself. A
//! `taliesin preview` is started on a copy of the project and watched over its websocket
//! exactly as `web-client/client.js` connects (`/ws?page=…`, same-origin `Origin`), and each
//! row is the time from the file write to the first message the server sends back. That
//! includes everything the in-process rows leave out: the watcher's debounce, rediscovery,
//! the edited page's own build and the diff. The language server row is the same for
//! `taliesin lsp --stdio`: a save as VS Code reports one (the file written, then
//! `workspace/didChangeWatchedFiles`), to its `publishDiagnostics`.
//!
//! Never the author's files: every run works on a copy in a temporary directory.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// Medians over the rounds, in nanoseconds, of save to first message. `None` when the kind
/// could not be measured on this page (no `title:` line, no `## ` heading).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SaveToOp {
    pub project: String,
    pub pages: usize,
    pub rounds: usize,
    /// A prose line edited in place: no anchor moves.
    pub body_ns: Option<u128>,
    /// A `## ` heading with an anchor inserted and removed above the page's sections, so its
    /// section numbers and anchors move.
    pub heading_ns: Option<u128>,
    /// The front matter's `title:` edited: discovery input, so the project is rediscovered.
    pub title_ns: Option<u128>,
    /// A prose edit saved the way vim, Helix and `sed -i` save: a new file renamed over the
    /// old one.
    pub atomic_ns: Option<u128>,
    /// The preview's resident memory after the last round, in KiB (Linux only).
    pub preview_rss_kib: Option<u64>,
    /// The language server's save to `publishDiagnostics`.
    pub lsp_save_ns: Option<u128>,
}

/// Copy `from` to a fresh `to`, leaving out build outputs and caches.
pub fn copy_project(from: &Path, to: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(name.to_str(), Some("_site" | "_book" | "_freeze" | ".git")) {
            continue;
        }
        let (src, dst) = (entry.path(), to.join(&name));
        if entry.file_type()?.is_dir() {
            copy_project(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn median(mut xs: Vec<u128>) -> Option<u128> {
    xs.sort_unstable();
    xs.get(xs.len() / 2).copied()
}

/// One edit of the page: a transform of its current text, or `None` when the page has no
/// place for it.
struct Editor {
    k: usize,
    heading_in: bool,
}

const INSERTED: &str = "## Inserted bench heading {#sec-bench-ins}\n\nInserted paragraph.\n\n";

impl Editor {
    /// Append a counter to the first long prose line outside front matter and fences.
    fn body(&mut self, text: &str) -> Option<String> {
        self.k += 1;
        let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        let (mut in_fm, mut fence) = (lines.first().is_some_and(|l| l.trim() == "---"), false);
        let at = lines.iter().enumerate().position(|(i, l)| {
            if in_fm {
                if i > 0 && l.trim() == "---" {
                    in_fm = false;
                }
                return false;
            }
            if l.starts_with("```") || l.starts_with("~~~") {
                fence = !fence;
                return false;
            }
            !fence && l.len() > 40 && l.starts_with(|c: char| c.is_ascii_alphabetic())
        })?;
        let line = &mut lines[at];
        if let Some(cut) = line.find(" Bench edit ") {
            line.truncate(cut);
        }
        line.push_str(&format!(" Bench edit {}.", self.k));
        Some(lines.join("\n"))
    }

    /// Insert a labelled heading above the first `## `, or take it out again.
    fn heading(&mut self, text: &str) -> Option<String> {
        let out = if self.heading_in {
            text.replacen(INSERTED, "", 1)
        } else {
            let at = text.find("\n## ")? + 1;
            format!("{}{INSERTED}{}", &text[..at], &text[at..])
        };
        self.heading_in = !self.heading_in;
        Some(out)
    }

    /// Append a counter to the front matter's `title:`.
    fn title(&mut self, text: &str) -> Option<String> {
        self.k += 1;
        let start = text.find("\ntitle:")? + 1;
        let end = start + text[start..].find('\n')?;
        let line = &text[start..end];
        let base = line
            .split(" v-")
            .next()
            .unwrap_or(line)
            .trim_end_matches('"');
        let quoted = line.trim_end().ends_with('"');
        let q = if quoted { "\"" } else { "" };
        Some(format!(
            "{}{base} v-{}{q}{}",
            &text[..start],
            self.k,
            &text[end..]
        ))
    }
}

/// A websocket client to the preview, reading only text frames.
struct Ws(TcpStream);

impl Ws {
    fn connect(port: u16, page: &str) -> std::io::Result<Ws> {
        // Percent-encoded as the preview writes it into the page (`encode_query`).
        let page: String = page
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect();
        let mut s = TcpStream::connect(("127.0.0.1", port))?;
        write!(
            s,
            "GET /ws?page={page} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
             Origin: http://127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        )?;
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            s.read_exact(&mut byte)?;
            head.push(byte[0]);
        }
        if !head.starts_with(b"HTTP/1.1 101") {
            return Err(std::io::Error::other(
                String::from_utf8_lossy(&head).into_owned(),
            ));
        }
        Ok(Ws(s))
    }

    /// The next text message, or `None` when none arrives within `wait`. Answers a ping.
    fn next(&mut self, wait: Duration) -> Option<String> {
        self.0.set_read_timeout(Some(wait)).ok()?;
        loop {
            let mut h = [0u8; 2];
            self.0.read_exact(&mut h).ok()?;
            self.0
                .set_read_timeout(Some(Duration::from_secs(30)))
                .ok()?;
            let mut len = u64::from(h[1] & 0x7f);
            if len == 126 {
                let mut b = [0u8; 2];
                self.0.read_exact(&mut b).ok()?;
                len = u64::from(u16::from_be_bytes(b));
            } else if len == 127 {
                let mut b = [0u8; 8];
                self.0.read_exact(&mut b).ok()?;
                len = u64::from_be_bytes(b);
            }
            let mut payload = vec![0u8; len as usize];
            self.0.read_exact(&mut payload).ok()?;
            match h[0] & 0x0f {
                1 => return String::from_utf8(payload).ok(),
                // A client frame is masked; a zero mask leaves the payload as it is.
                9 => {
                    let mut pong = vec![0x8a, 0x80 | payload.len() as u8, 0, 0, 0, 0];
                    pong.extend_from_slice(&payload);
                    self.0.write_all(&pong).ok()?;
                }
                8 => return None,
                _ => {}
            }
        }
    }

    /// Read until the server has been quiet for `quiet`.
    fn drain(&mut self, quiet: Duration) {
        while self.next(quiet).is_some() {}
    }
}

/// Whether a preview message tells the tab to reload.
fn is_reload(message: &str) -> bool {
    serde_json::from_str::<Value>(message)
        .is_ok_and(|v| v.get("type").and_then(Value::as_str) == Some("reload"))
}

fn rss_kib(pid: u32) -> Option<u64> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// Kill the child on drop, so a failed measurement leaves no server behind.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Start `taliesin preview` on `root` and return it with the port it bound.
fn start_preview(bin: &Path, root: &Path, port: u16) -> std::io::Result<(Reaped, u16)> {
    let log = root.with_extension("preview.log");
    let child = Command::new(bin)
        .args([
            "preview".as_ref(),
            root.as_os_str(),
            port.to_string().as_ref(),
        ])
        .arg("--no-exec")
        .env("TALIESIN_NO_EXEC", "1")
        .env("NO_COLOR", "1")
        // The banner that names the address goes to stderr.
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&log)?)
        .spawn()?;
    let child = Reaped(child);
    let deadline = Instant::now() + Duration::from_secs(120);
    while Instant::now() < deadline {
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        if let Some(at) = text.find("http://127.0.0.1:") {
            let digits: String = text[at + 17..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if let Ok(bound) = digits.parse() {
                return Ok((child, bound));
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(std::io::Error::other(
        "the preview never printed its address",
    ))
}

/// Time `rounds` rounds of the four save kinds, interleaved so machine noise lands on each
/// alike, on `page` of a copy of `project`.
pub fn measure_preview(
    bin: &Path,
    label: &str,
    project: &Path,
    page: &str,
    pages: usize,
    rounds: usize,
    port: u16,
) -> std::io::Result<SaveToOp> {
    let root = std::env::temp_dir().join(format!("live-edit-bench-{}-{port}", std::process::id()));
    copy_project(project, &root)?;
    let (child, bound) = start_preview(bin, &root, port)?;
    let mut ws = Ws::connect(bound, page)?;
    ws.drain(Duration::from_millis(1500));
    let path = root.join(page);
    let mut ed = Editor {
        k: 0,
        heading_in: false,
    };
    let mut times: [Vec<u128>; 4] = Default::default();
    for _ in 0..rounds {
        for (kind, slot) in times.iter_mut().enumerate() {
            let text = std::fs::read_to_string(&path)?;
            let edited = match kind {
                0 | 3 => ed.body(&text),
                1 => ed.heading(&text),
                _ => ed.title(&text),
            };
            let Some(edited) = edited else { continue };
            ws.drain(Duration::from_millis(50));
            let t0 = Instant::now();
            if kind == 3 {
                let tmp = path.with_extension("tmd.bench-swap");
                std::fs::write(&tmp, &edited)?;
                std::fs::rename(&tmp, &path)?;
            } else {
                std::fs::write(&path, &edited)?;
            }
            let first = ws.next(Duration::from_secs(60));
            if first.is_some() {
                slot.push(t0.elapsed().as_nanos());
            }
            // A save that moves the page's chrome (a title the navbar or a book's drawer
            // shows) is answered with `reload`, and the preview drops the tab's state: the
            // browser loads the page again and reconnects, so the bench reconnects too.
            // Kept on the old socket, every later save waited out the 60 s above unanswered.
            if first.as_deref().is_none_or(is_reload) {
                ws = Ws::connect(bound, page)?;
                ws.drain(Duration::from_millis(1500));
            } else {
                ws.drain(Duration::from_millis(350));
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    }
    let preview_rss_kib = rss_kib(child.0.id());
    drop(child);
    let lsp_save_ns = measure_lsp(bin, &root, page, rounds).ok().flatten();
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(root.with_extension("preview.log"));
    let [body, heading, title, atomic] = times;
    Ok(SaveToOp {
        project: label.to_string(),
        pages,
        rounds,
        body_ns: median(body),
        heading_ns: median(heading),
        title_ns: median(title),
        atomic_ns: median(atomic),
        preview_rss_kib,
        lsp_save_ns,
    })
}

/// The language server over stdio, answering its requests with `null`.
struct Lsp {
    child: Reaped,
    stdin: ChildStdin,
    rx: Receiver<Value>,
    id: u64,
}

impl Lsp {
    fn start(bin: &Path, cwd: &Path) -> std::io::Result<Lsp> {
        let mut child = Command::new(bin)
            .args(["lsp", "--stdio"])
            .current_dir(cwd)
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            loop {
                let mut len = 0usize;
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let line = line.trim();
                    if line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.strip_prefix("Content-Length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; len];
                if r.read_exact(&mut body).is_err() {
                    return;
                }
                let Ok(msg) = serde_json::from_slice::<Value>(&body) else {
                    continue;
                };
                if tx.send(msg).is_err() {
                    return;
                }
            }
        });
        Ok(Lsp {
            child: Reaped(child),
            stdin,
            rx,
            id: 0,
        })
    }

    fn send(&mut self, msg: Value) -> std::io::Result<()> {
        let body = msg.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{body}", body.len())?;
        self.stdin.flush()
    }

    fn notify(&mut self, method: &str, params: Value) -> std::io::Result<()> {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    fn request(&mut self, method: &str, params: Value) -> std::io::Result<u64> {
        self.id += 1;
        let id = self.id;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        Ok(id)
    }

    /// Wait for a message `pred` accepts, answering server requests on the way.
    fn wait(&mut self, wait: Duration, pred: impl Fn(&Value) -> bool) -> Option<Value> {
        let deadline = Instant::now() + wait;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let msg = self.rx.recv_timeout(left).ok()?;
            if msg.get("method").is_some() && msg.get("id").is_some() {
                let reply = json!({"jsonrpc": "2.0", "id": msg["id"], "result": null});
                self.send(reply).ok()?;
                continue;
            }
            if pred(&msg) {
                return Some(msg);
            }
        }
        None
    }
}

/// Median save to `publishDiagnostics` over `rounds` saves of `page`: each save first types
/// into the buffer (so the buffer is ahead of the file, as it is before a save), then writes
/// the buffer to disk and sends the watcher event VS Code sends.
fn measure_lsp(
    bin: &Path,
    root: &Path,
    page: &str,
    rounds: usize,
) -> std::io::Result<Option<u128>> {
    let mut lsp = Lsp::start(bin, root)?;
    let root_uri = format!("file://{}", root.display());
    let uri = format!("file://{}", root.join(page).display());
    let init = lsp.request(
        "initialize",
        json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}},
                "textDocument": {"publishDiagnostics": {}, "synchronization": {"didSave": true}}
            }
        }),
    )?;
    lsp.wait(Duration::from_secs(30), |m| m["id"] == init);
    lsp.notify("initialized", json!({}))?;
    let path = root.join(page);
    let mut text = std::fs::read_to_string(&path)?;
    let published = |m: &Value| {
        m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == uri.as_str()
    };
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "taliesin", "version": 1, "text": text}}),
    )?;
    lsp.wait(Duration::from_secs(120), published);
    let mut ed = Editor {
        k: 0,
        heading_in: false,
    };
    let mut saves = Vec::new();
    for version in 2..2 + rounds {
        let Some(edited) = ed.body(&text) else {
            break;
        };
        text = edited;
        lsp.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": uri, "version": version}, "contentChanges": [{"text": text}]}),
        )?;
        lsp.wait(Duration::from_secs(60), published);
        std::thread::sleep(Duration::from_millis(300));
        std::fs::write(&path, &text)?;
        let t0 = Instant::now();
        lsp.notify(
            "workspace/didChangeWatchedFiles",
            json!({"changes": [{"uri": uri, "type": 2}]}),
        )?;
        if lsp.wait(Duration::from_secs(60), published).is_some() {
            saves.push(t0.elapsed().as_nanos());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    let shutdown = lsp.request("shutdown", Value::Null)?;
    lsp.wait(Duration::from_secs(10), |m| m["id"] == shutdown);
    let _ = lsp.notify("exit", Value::Null);
    let _ = lsp.child.0.wait();
    Ok(median(saves))
}

/// Where the release binary this crate's workspace builds lives, or `TALIESIN_BIN`.
pub fn release_binary() -> Option<PathBuf> {
    let bin = std::env::var_os("TALIESIN_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/release/taliesin")
        });
    bin.is_file().then_some(bin)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each save kind changes the page every time and undoes nothing it did not do: the body
    /// edit replaces its own counter, the heading comes out as it went in, and the title
    /// keeps its quoting.
    #[test]
    fn every_edit_changes_the_page_and_only_what_it_owns() {
        let page = "---\ntitle: \"Chapter 1\"\n---\n\n# One\n\n\
                    A prose line that is comfortably longer than forty characters.\n\n\
                    ## Section {#sec-a}\n\nMore.\n";
        let mut ed = Editor {
            k: 0,
            heading_in: false,
        };
        let once = ed.body(page).unwrap();
        let twice = ed.body(&once).unwrap();
        assert!(
            once.contains("characters. Bench edit 1.")
                && twice.contains("characters. Bench edit 2.")
        );
        assert!(!twice.contains("Bench edit 1."));
        let with = ed.heading(page).unwrap();
        assert!(with.contains(INSERTED) && with.find(INSERTED) < with.find("## Section"));
        assert_eq!(ed.heading(&with).unwrap(), page);
        let titled = ed.title(page).unwrap();
        assert!(titled.contains("title: \"Chapter 1 v-3\"\n"), "{titled}");
        assert!(
            ed.title(&titled)
                .unwrap()
                .contains("title: \"Chapter 1 v-4\"\n")
        );
    }
}
