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
//! renames a temp file over it, and that a save of a script a page imports reaches it as a
//! reload, since a browser keeps a module for the life of the page that loaded it.

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

/// A fresh temporary directory named after `tag`.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A project of `{js}` pages and the modules they import, in a fresh directory: post `a`
/// imports `a-mod.js`, which imports `a-dep.js`; post `b` imports `b-mod.js`; the listing
/// imports nothing. Beside post `a` sits a Node test of its module, which no page imports.
/// No `{python}` cell, so a preview of it needs no kernel.
fn js_project(tag: &str) -> PathBuf {
    let dir = scratch(tag);
    for (path, text) in [
        ("_site.yml", "title: T\n"),
        (
            "index.tmd",
            "---\ntitle: Home\nlisting:\n  contents: posts\n---\n\nPosts.\n",
        ),
        (
            "posts/a/index.tmd",
            "---\ntitle: A\n---\n\nProse of A.\n\n```{js}\nconst m = await import(\"./a-mod.js\");\n```\n",
        ),
        (
            "posts/a/a-mod.js",
            "import { dep } from \"./a-dep.js\";\nexport const word = dep;\n",
        ),
        ("posts/a/a-dep.js", "export const dep = \"a\";\n"),
        (
            "posts/a/_a.test.mjs",
            "import { word } from \"./a-mod.js\";\n",
        ),
        (
            "posts/b/index.tmd",
            "---\ntitle: B\n---\n\n```{js}\nconst m = await import(\"./b-mod.js\");\n```\n",
        ),
        ("posts/b/b-mod.js", "export const word = \"b\";\n"),
    ] {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
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

/// `taliesin preview <target>` on any free port, logging to `log` (outside the project, so
/// the log's own writes are no save), and the port it bound. With `no_exec` its code cells
/// render as source, `{js}` cells included, so its pages load no module at all.
fn start(target: &Path, log: &Path, no_exec: bool) -> (Preview, u16) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_taliesin"));
    command.arg("preview").arg(target).arg("0");
    if no_exec {
        command.arg("--no-exec");
    } else {
        command.env_remove("TALIESIN_NO_EXEC");
    }
    let child = command
        .env("TALIESIN_NO_CLEAR", "1")
        .stdout(Stdio::null())
        .stderr(fs::File::create(log).unwrap())
        .spawn()
        .expect("spawn preview");
    let preview = Preview(child);
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(port) = port_from(log) {
            return (preview, port);
        }
        assert!(
            Instant::now() < deadline,
            "the preview never printed its port"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A websocket to `path` (a page's live channel), handshake done, as the browser opens it.
fn connect(port: u16, path: &str) -> TcpStream {
    let mut sock = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    write!(
        sock,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\n\
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

/// Whether a message tells the tab to reload.
fn is_reload(message: &str) -> bool {
    message == r#"{"type":"reload"}"#
}

/// Read until `quiet` passes with nothing arriving.
fn settle(sock: &mut TcpStream, quiet: Duration) {
    while next_text(sock, quiet).is_some() {}
}

/// Whether a `reload` arrives on `sock` within `wait`, passing over any other message.
fn reload_within(sock: &mut TcpStream, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    while let Some(left) = deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
    {
        match next_text(sock, left) {
            Some(message) if is_reload(&message) => return true,
            Some(_) => {}
            None => return false,
        }
    }
    false
}

/// How many `reload`s arrive on `sock` before `quiet` passes with nothing arriving.
fn reloads_until_quiet(sock: &mut TcpStream, quiet: Duration) -> usize {
    let mut reloads = 0;
    while let Some(message) = next_text(sock, quiet) {
        reloads += usize::from(is_reload(&message));
    }
    reloads
}

/// The body of a plain `GET` of `url`, as the browser loads a page.
fn get(port: u16, url: &str) -> String {
    let mut sock = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    write!(
        sock,
        "GET /{url} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    sock.read_to_string(&mut response).unwrap();
    response
}

/// The websocket path a served page connects to: its `window.TALIESIN_WS_PATH`.
fn ws_path_of(page: &str) -> String {
    let key = "window.TALIESIN_WS_PATH = \"";
    let at = page.find(key).expect("a live page names its websocket") + key.len();
    page[at..at + page[at..].find('"').unwrap()].to_string()
}

/// A tab on `url` as the browser opens one: the page loaded, its websocket connected to the
/// path the page names, and its opening messages read. The first is the page's render: a
/// tab told to reload the moment it connects would reload forever.
fn open_tab(port: u16, url: &str) -> TcpStream {
    let mut tab = connect(port, &ws_path_of(&get(port, url)));
    let first = next_text(&mut tab, Duration::from_secs(10)).expect("an opening message");
    assert!(
        first.contains(r#""type":"full_render""#),
        "a freshly served tab of `{url}` opened with {first}"
    );
    settle(&mut tab, Duration::from_millis(800));
    tab
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

#[test]
fn a_save_reaches_the_open_tab() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/tech-blog");
    let dir = scratch("save-to-tab");
    copy_tree(&corpus, &dir);
    let log = dir.with_extension("log");
    let (_preview, port) = start(&dir, &log, true);
    let mut tab = connect(port, &format!("/ws?page={PAGE}"));
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

/// Assert that the tab at `want`, if any, is told to reload, at least once and at most
/// `most` times, and that no other tab is.
fn assert_reloads(
    tabs: &mut [TcpStream],
    urls: &[&str],
    want: Option<usize>,
    most: usize,
    what: &str,
) {
    if let Some(i) = want {
        assert!(
            reload_within(&mut tabs[i], Duration::from_secs(10)),
            "{what}: the tab on `{}` was not reloaded",
            urls[i]
        );
        let more = reloads_until_quiet(&mut tabs[i], Duration::from_millis(400));
        assert!(
            more < most,
            "{what}: the tab on `{}` was reloaded {} times",
            urls[i],
            more + 1
        );
    }
    for (i, tab) in tabs.iter_mut().enumerate() {
        if Some(i) != want {
            assert_eq!(
                reloads_until_quiet(tab, Duration::from_millis(600)),
                0,
                "{what}: the tab on `{}` was reloaded",
                urls[i]
            );
        }
    }
}

/// Save `text` to `path` as vim and `sed -i` do, a temp file renamed over it. The watcher
/// sees one event for `path` (the `.`-prefixed temp file is no project file), so the save is
/// one batch however the test thread is scheduled; a write in place is a truncate and a
/// write, which a thread descheduled between them splits into two.
fn save(path: &Path, text: &str) {
    let name = path.file_name().unwrap().to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.save"));
    fs::write(&tmp, text).unwrap();
    fs::rename(&tmp, path).unwrap();
}

/// A browser keeps an ES module for the life of the page that imported it, so a `{js}`
/// cell's `import("./a-mod.js")` goes on returning the module the tab loaded first,
/// whatever the file holds now. A save of the module reached no tab at all, and the tab
/// threw calling an export the file had gained (tech-blog `posts/cusum`, 2026-11-06).
///
/// The tab whose page imports the saved module, directly or through another module,
/// reloads once per save however the save is made, and no other tab does; a script no page
/// imports reloads nothing. A tab that connects after missing a reload reloads as it
/// connects. A page save still reaches its tab as block ops, and nothing else a save can
/// touch reloads a tab.
#[test]
fn a_module_save_reloads_the_tabs_whose_page_imports_it() {
    let dir = js_project("module-save");
    let log = dir.with_extension("log");
    let (_preview, port) = start(&dir, &log, false);
    let urls = ["posts/a/index.html", "posts/b/index.html", "index.html"];
    let mut tabs: Vec<TcpStream> = urls.iter().map(|url| open_tab(port, url)).collect();
    let post = dir.join("posts/a");
    let module = post.join("a-mod.js");
    let original = fs::read_to_string(&module).unwrap();

    // In place, so possibly two batches (see `save`), each of which reloads.
    fs::write(&module, format!("{original}export const probe = 1;\n")).unwrap();
    assert_reloads(&mut tabs, &urls, Some(0), 2, "an edit in place");

    save(&module, &original);
    assert_reloads(&mut tabs, &urls, Some(0), 1, "a temp file renamed over it");

    save(&post.join("a-dep.js"), "export const dep = \"a2\";\n");
    assert_reloads(&mut tabs, &urls, Some(0), 1, "a module that module imports");

    // Ten saves back to back are one batch, or two or three if the thread is descheduled
    // between them: never a reload per save.
    for i in 0..10 {
        save(&module, &format!("{original}// save {i}\n"));
    }
    assert_reloads(&mut tabs, &urls, Some(0), 3, "ten saves back to back");

    let other = dir.join("posts/b/b-mod.js");
    save(&other, "export const word = \"b2\";\n");
    assert_reloads(&mut tabs, &urls, Some(1), 1, "the other post's module");

    // Scripts no page imports: the Node test beside the post, and a new module.
    let test = post.join("_a.test.mjs");
    save(&test, "import { word } from \"./a-mod.js\";\n// edited\n");
    assert_reloads(&mut tabs, &urls, None, 1, "the Node test saved");
    save(&post.join("unused.js"), "export {};\n");
    assert_reloads(&mut tabs, &urls, None, 1, "a module nothing imports");

    // The tabs reload as the browser reloads them: the old socket closes, the page loads
    // again, a new socket opens.
    drop(tabs);
    let mut tabs: Vec<TcpStream> = urls.iter().map(|url| open_tab(port, url)).collect();

    // A page save reaches its own tab as block ops, which keep its `{js}` cells running.
    let page = post.join("index.tmd");
    let text = fs::read_to_string(&page).unwrap();
    save(&page, &text.replace("Prose of A.", "Prose of A, edited."));
    loop {
        let message = next_text(&mut tabs[0], Duration::from_secs(10))
            .expect("the page save never reached its tab");
        assert!(!is_reload(&message), "a page save reloaded its tab");
        if is_op(&message) {
            break;
        }
    }
    assert_reloads(&mut tabs, &urls, None, 1, "a page save");

    // No reload from a stylesheet, an image, or a script inside a tree the preview leaves
    // alone (build output, the freeze cache, packages, a `.`-prefixed directory).
    fs::write(post.join("style.css"), "body {}\n").unwrap();
    fs::write(post.join("pic.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    for tree in ["_site", "_freeze", "node_modules", ".cache"] {
        fs::create_dir_all(dir.join(tree)).unwrap();
        fs::write(dir.join(tree).join("a-mod.js"), "export {};\n").unwrap();
    }
    assert_reloads(&mut tabs, &urls, None, 1, "a save of no page's script");

    // A tab served before a script save, its socket opened after the save landed: it was
    // reloading, or asleep, when the reload went out.
    let stale = ws_path_of(&get(port, urls[0]));
    save(&other, "export const word = \"b3\";\n");
    assert!(reload_within(&mut tabs[1], Duration::from_secs(10)));
    let mut late = connect(port, &stale);
    let first = next_text(&mut late, Duration::from_secs(10)).expect("an opening message");
    assert!(
        is_reload(&first),
        "a tab served before a script save must reload as it connects, not open with {first}"
    );
    drop(late);

    // A tab whose page lost its state while the tab was between sockets: a save of another
    // page drops the state of every page no socket is on. Served from the state that is
    // gone, the tab reloads instead of opening on an empty page.
    let mut tabs = tabs.into_iter();
    let (old_a, mut tab_b) = (tabs.next().unwrap(), tabs.next().unwrap());
    let served = ws_path_of(&get(port, urls[0]));
    drop(old_a);
    std::thread::sleep(Duration::from_millis(300));
    let page_b = dir.join("posts/b/index.tmd");
    let text_b = fs::read_to_string(&page_b).unwrap();
    save(&page_b, &format!("{text_b}\nMore of B.\n"));
    loop {
        let message = next_text(&mut tab_b, Duration::from_secs(10))
            .expect("the save of B never reached its tab");
        if is_op(&message) {
            break;
        }
    }
    let mut late = connect(port, &served);
    let first = next_text(&mut late, Duration::from_secs(10)).expect("an opening message");
    assert!(
        is_reload(&first),
        "a tab whose page lost its state must reload as it connects, not open with {first}"
    );
    // Its reload's visit renders the state again, and it opens normally.
    open_tab(port, urls[0]);

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_file(&log);
}

/// The same for a document previewed on its own (`preview note.tmd`, no `_site.yml`), a
/// project of one page served at `/`.
#[test]
fn a_module_save_reloads_a_single_document_preview() {
    let dir = scratch("module-single");
    let note = dir.join("note.tmd");
    let cell = "```{js}\nconst m = await import(\"./m.js\");\n```\n";
    fs::write(&note, format!("---\ntitle: Note\n---\n\nProse.\n\n{cell}")).unwrap();
    fs::write(dir.join("m.js"), "export const a = 1;\n").unwrap();
    let log = dir.with_extension("log");
    let (_preview, port) = start(&note, &log, false);
    let mut tab = open_tab(port, "");

    save(&dir.join("m.js"), "export const b = 2;\n");
    assert!(
        reload_within(&mut tab, Duration::from_secs(10)),
        "a module save must reload the document's tab"
    );
    drop(tab);
    let mut tab = open_tab(port, "");
    save(
        &note,
        &format!("---\ntitle: Note\n---\n\nProse, edited.\n\n{cell}"),
    );
    loop {
        let message = next_text(&mut tab, Duration::from_secs(10))
            .expect("the page save never reached its tab");
        assert!(!is_reload(&message), "a page save reloaded its tab");
        if is_op(&message) {
            break;
        }
    }
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_file(&log);
}
