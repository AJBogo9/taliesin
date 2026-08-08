//! Small console output for the dev server: a startup banner and consistent,
//! lightly-coloured event lines. Everything goes to stderr so `render`'s HTML on
//! stdout stays clean. Colour is auto-disabled when stderr is not a TTY or when
//! `NO_COLOR` is set; the startup screen-clear is likewise gated on a TTY and can
//! be turned off with `TALIESIN_NO_CLEAR`.

use std::io::{IsTerminal, Write};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
enum Style {
    Ready,
    Network,
    Built,
    Watch,
    Update,
    Source,
    Kernel,
    Exec,
    Info,
    Warn,
    Error,
}

impl Style {
    /// The fixed-width tag and its ANSI colour code.
    fn parts(self) -> (&'static str, &'static str) {
        match self {
            Style::Ready => ("ready", "\x1b[32m"),     // green
            Style::Network => ("network", "\x1b[36m"), // cyan
            Style::Built => ("built", "\x1b[32m"),     // green
            Style::Watch => ("watch", "\x1b[2m"),      // dim
            Style::Update => ("update", "\x1b[36m"),   // cyan
            Style::Source => ("source", "\x1b[34m"),   // blue
            Style::Kernel => ("kernel", "\x1b[35m"),   // magenta
            Style::Exec => ("exec", "\x1b[35m"),       // magenta (kernel work)
            Style::Info => ("info", "\x1b[90m"),       // grey (a note, not an outcome)
            Style::Warn => ("warn", "\x1b[33m"),       // yellow
            Style::Error => ("error", "\x1b[31m"),     // red
        }
    }
}

fn colored() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal())
}

fn paint(text: &str, code: &str) -> String {
    if colored() {
        format!("{code}{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn line(style: Style, msg: &str) {
    let (tag, code) = style.parts();
    eprintln!("  {} {msg}", paint(&format!("{tag:<7}"), code));
}

/// Clear the screen at startup so the dev-server log starts at the top, free of
/// previous shell output. Soft clear (Vite-style): the current viewport is pushed
/// up into the scrollback buffer, then the cursor homes and clears downward — so
/// earlier output stays recoverable by scrolling up. We never emit `CSI 3J`
/// (which wipes scrollback). No-op when stderr isn't a TTY (so piped/redirected
/// logs stay clean) or when `TALIESIN_NO_CLEAR` is set.
pub fn clear_screen() {
    let mut err = std::io::stderr();
    if !err.is_terminal() || std::env::var_os("TALIESIN_NO_CLEAR").is_some() {
        return;
    }
    // Push the visible lines into scrollback, then home + erase-to-end. Falling
    // back to a plain home+erase when the row count is unknown.
    let rows = terminal_size::terminal_size_of(&err)
        .map(|(_, h)| h.0)
        .unwrap_or(0);
    let blanks = "\n".repeat(rows.saturating_sub(1) as usize);
    let _ = write!(err, "{blanks}\x1b[H\x1b[J");
    let _ = err.flush();
}

/// The opening banner: tool name + version.
pub fn banner(version: &str) {
    eprintln!();
    eprintln!(
        "  {} {}",
        paint("taliesin", "\x1b[1;32m"),
        paint(version, "\x1b[2m")
    );
}

/// The server is up; `url` is highlighted as the thing to open, with how long
/// startup took.
pub fn ready(url: &str, elapsed: std::time::Duration) {
    line(
        Style::Ready,
        &format!(
            "{}  {}  {}ms",
            paint(url, "\x1b[1m"),
            paint("·", "\x1b[2m"),
            elapsed.as_millis()
        ),
    );
}

/// The LAN URL the preview is reachable at (printed only with `--host`).
pub fn network(url: &str) {
    line(Style::Network, &paint(url, "\x1b[1m"));
}

/// The one-line orientation hint for the preview's controls. Taliesin has no interactive
/// stdin key loop — a reader coming from Vite (or a Vite-style dev server) who presses
/// `r`/`o`/`u`/`c`/`q`/`h` at the terminal gets silence, because every control lives in the
/// browser's `◇` dev menu instead. Pure so its wording is unit-testable.
fn keys_hint_body() -> &'static str {
    "controls live in the browser — open the ◇ dev menu (bottom-left)"
}

/// Print the controls hint once at startup. TTY-gated: a human at a terminal is the only one
/// with keystrokes to misdirect, so an agent/CI/piped run keeps its captured log clean.
pub fn keys_hint() {
    if !std::io::stderr().is_terminal() {
        return;
    }
    line(Style::Info, keys_hint_body());
}

/// The first-run notice's wording. Pure so `first_run_notice_states_what_runs` can assert
/// it without touching the filesystem or a terminal.
fn first_run_notice_body() -> &'static str {
    "first run: previewing a .tmd RUNS it (code cells, {js}, raw HTML) — \
     `--no-exec` skips the cells; see SECURITY.md"
}

/// Where the "already told them" marker lives. A user-level path, not a project one: the
/// fact is about the tool, so learning it once is enough no matter which project is open.
/// Same base resolution as the math-hover cache (`XDG_CACHE_HOME`, else `~/.cache`, else
/// the temp dir).
fn first_run_marker() -> std::path::PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
        .join("taliesin")
        .join("first-run-notice")
}

/// Say once, on this machine, that previewing a document executes it.
///
/// The trust model is stated correctly in `SECURITY.md` and in the CLI reference, and a
/// first-time user reads neither before their first `taliesin preview` — which is the one
/// moment the fact is load-bearing. So it is said in the terminal, exactly once ever.
///
/// TTY-gated like [`keys_hint`] (an agent or CI run keeps its captured log clean) and
/// marker-gated so the author who lives in this tool sees it on day one and never again.
/// A failure to write the marker prints the notice again next time rather than crashing:
/// repeating a one-line notice is the cheap failure.
pub fn first_run_notice() {
    if !std::io::stderr().is_terminal() {
        return;
    }
    let marker = first_run_marker();
    if marker.exists() {
        return;
    }
    line(Style::Info, first_run_notice_body());
    if let Some(dir) = marker.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&marker, b"");
}

/// A one-shot `build` wrote a file.
pub fn built(path: &str) {
    line(Style::Built, &paint(path, "\x1b[1m"));
}

/// What is being watched, plus a short format descriptor (e.g. "html, toc").
pub fn watching(path: &str, desc: &str) {
    line(
        Style::Watch,
        &format!("{path}  {}  {desc}", paint("·", "\x1b[2m")),
    );
    eprintln!();
}

/// A save was applied: `n` blocks changed.
pub fn update(n: usize) {
    line(
        Style::Update,
        &format!("{n} block{}", if n == 1 { "" } else { "s" }),
    );
}

/// Escape every C0/C1 control character as a visible `\xNN`.
///
/// Click-to-source locations arrive over the preview websocket, which accepts
/// control messages without auth (see `serve::handle_client_msg`), so they are
/// the one log payload this server does not author itself. Printed raw, an OSC
/// (`\x1b]0;…`) would retitle the author's terminal and a `\r` or `\n` would
/// overwrite the log line or forge a new one. Escaping rather than dropping
/// keeps the tampering visible.
fn escape_control(s: &str) -> String {
    if !s.chars().any(char::is_control) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        // `char::is_control` is the Unicode Cc category: U+0000..=U+001F,
        // U+007F..=U+009F. All fit in two hex digits.
        if c.is_control() {
            out.push_str(&format!("\\x{:02x}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

/// A click-to-source request (a location the preview asked the editor to open).
pub fn source(loc: &str) {
    line(Style::Source, &escape_control(loc));
}

/// Kernel lifecycle / status.
pub fn kernel(msg: &str) {
    line(Style::Kernel, msg);
}

/// Code-cell execution progress, shown while the kernel runs. `page` names the document
/// the cell belongs to: a cold multi-page build runs pages concurrently, so an unlabelled
/// `cell 2/5` cannot be attributed to any of them.
pub fn exec(page: Option<&str>, done: usize, total: usize) {
    match page {
        Some(p) => line(Style::Exec, &format!("{p}  cell {done}/{total}")),
        None => line(Style::Exec, &format!("cell {done}/{total}")),
    }
}

/// The body of the closing cache summary (DX9), pure so its wording + pluralization are
/// unit-testable: `N` cells replayed from cache (the warm in-memory prefix + the disk
/// `_freeze` tail), `M` re-ran fresh. Answers "why didn't my cell re-run?" — the info was
/// always in the freeze plan, just never surfaced.
fn cache_tally_body(cached: usize, ran: usize) -> String {
    format!(
        "restored {cached} cached cell{} · {ran} re-ran",
        if cached == 1 { "" } else { "s" }
    )
}

/// The closing cache summary, printed once per run (only when at least one cell replayed).
/// `page` names the document on a multi-page build, where pages run concurrently.
pub fn cache_tally(page: Option<&str>, cached: usize, ran: usize) {
    let body = cache_tally_body(cached, ran);
    match page {
        Some(p) => line(Style::Exec, &format!("{p}  {body}")),
        None => line(Style::Exec, &body),
    }
}

/// General informational message: a note about what is about to happen or is winding
/// down. It gets its own tag — reusing the green `built` tag made a build *start* print
/// `built building with…` and Ctrl-C print `built shutting down…`, both reading as
/// completed builds.
pub fn info(msg: &str) {
    line(Style::Info, msg);
}

pub fn warn(msg: &str) {
    line(Style::Warn, msg);
}

pub fn error(msg: &str) {
    line(Style::Error, msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_tally_body_reads_naturally_and_pluralizes() {
        // The all-cached replay is exactly the "why didn't my cell re-run?" case: it must
        // still speak (0 re-ran), and the count must be real, not hard-coded.
        assert_eq!(cache_tally_body(5, 0), "restored 5 cached cells · 0 re-ran");
        assert_eq!(cache_tally_body(1, 2), "restored 1 cached cell · 2 re-ran");
        assert_eq!(cache_tally_body(3, 1), "restored 3 cached cells · 1 re-ran");
    }

    /// The first-run notice must say the one thing a first-time user does not know: that
    /// opening a document runs it. It also has to name the escape hatch, or it is a warning
    /// with no action attached.
    #[test]
    fn first_run_notice_states_what_runs_and_the_way_out() {
        let body = first_run_notice_body();
        for needle in ["RUNS", "--no-exec", "SECURITY.md"] {
            assert!(
                body.contains(needle),
                "the first-run notice must carry {needle:?}: {body:?}"
            );
        }
        // One line. A wall of text at startup is scrolled past, which is the same as not
        // saying it — and the long forms already exist in SECURITY.md and the CLI reference.
        assert!(!body.contains('\n'), "one line only: {body:?}");
    }

    /// The marker is a USER-level path, not a project one: the fact is about the tool, so a
    /// second project must not re-announce it. Also pinned: it is under a `taliesin/`
    /// directory, so the notice never drops a bare file into someone's cache root.
    #[test]
    fn the_first_run_marker_is_user_level_and_namespaced() {
        let dir = std::env::temp_dir().join(format!("tali-frn-{}", std::process::id()));
        // SAFETY: single-threaded scope in this test; the value is read immediately below.
        let previous = std::env::var_os("XDG_CACHE_HOME");
        unsafe { std::env::set_var("XDG_CACHE_HOME", &dir) };
        let marker = first_run_marker();
        match previous {
            Some(v) => unsafe { std::env::set_var("XDG_CACHE_HOME", v) },
            None => unsafe { std::env::remove_var("XDG_CACHE_HOME") },
        }
        assert!(
            marker.starts_with(&dir),
            "the marker honours XDG_CACHE_HOME: {marker:?}"
        );
        assert_eq!(
            marker.parent().and_then(|p| p.file_name()),
            Some(std::ffi::OsStr::new("taliesin")),
            "namespaced under taliesin/: {marker:?}"
        );
    }

    #[test]
    fn keys_hint_points_readers_at_the_browser_menu() {
        // The whole point is redirecting terminal muscle-memory to the browser: it must name
        // the browser and the ◇ menu glyph, or the hint fails silently.
        let body = keys_hint_body();
        assert!(
            body.contains("browser"),
            "hint must name the browser: {body:?}"
        );
        assert!(body.contains('◇'), "hint must name the ◇ menu: {body:?}");
    }

    /// The hint named the wrong corner for as long as it shipped: it said `(top-right)`
    /// while `STATUS_CSS` has always pinned the menu `bottom: .6rem; left: .6rem`. It
    /// rotted silently because the assertions above only ask for "browser" and the glyph,
    /// and because the hint is TTY-gated it never appears in a captured log either. So do
    /// not assert a second hard-coded corner — derive it from the stylesheet that actually
    /// places the menu, and moving the menu reddens this instead of rotting the prose.
    #[test]
    fn keys_hint_names_the_corner_the_stylesheet_puts_the_menu_in() {
        let rule = crate::serve::STATUS_CSS
            .split_once("#tali-controls.tali-dev {")
            .expect("STATUS_CSS must still carry the dev-menu rule")
            .1
            .split_once('}')
            .expect("the dev-menu rule must be closed")
            .0;

        let edge = |candidates: [&str; 2]| -> String {
            let mut found = candidates
                .iter()
                .filter(|e| rule.contains(&format!("{e}:")))
                .collect::<Vec<_>>();
            assert_eq!(
                found.len(),
                1,
                "the dev-menu rule must anchor to exactly one of {candidates:?}: {rule:?}"
            );
            found.pop().unwrap().to_string()
        };
        let corner = format!("{}-{}", edge(["top", "bottom"]), edge(["left", "right"]));

        let body = keys_hint_body();
        assert!(
            body.contains(&corner),
            "hint must name the {corner} corner STATUS_CSS places the menu in: {body:?}"
        );
    }

    #[test]
    fn ordinary_locations_are_untouched() {
        assert_eq!(
            escape_control("posts/intro.tmd  12:3"),
            "posts/intro.tmd  12:3"
        );
        assert_eq!(escape_control("(primary)  ?"), "(primary)  ?");
        // Non-ASCII is not a control char and must survive verbatim.
        assert_eq!(
            escape_control("posts/héllo-ü.tmd  1:1"),
            "posts/héllo-ü.tmd  1:1"
        );
    }

    #[test]
    fn control_bytes_from_the_browser_are_escaped() {
        // An OSC title-set, a CSI erase-line, and a bare CR: the three shapes a
        // crafted `source_file` would use to forge or overwrite terminal output.
        let hostile = "\x1b]0;pwned\x07\x1b[2K\rgit push --force\n";
        let safe = escape_control(hostile);
        assert!(
            !safe.chars().any(char::is_control),
            "control chars survived: {safe:?}"
        );
        assert!(safe.contains("\\x1b"), "ESC not escaped visibly: {safe:?}");
        assert!(safe.contains("\\x0d"), "CR not escaped visibly: {safe:?}");
        // The payload text is kept, just defanged, so the author still sees it.
        assert!(safe.contains("git push --force"));
    }

    #[test]
    fn c1_controls_are_escaped_too() {
        // U+009D is a one-byte OSC introducer on terminals that decode C1.
        let safe = escape_control("a\u{9d}0;x\u{9c}b");
        assert!(!safe.chars().any(char::is_control), "{safe:?}");
        assert_eq!(safe, "a\\x9d0;x\\x9cb");
    }
}
