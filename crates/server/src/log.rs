//! Small, dependency-free console output for the dev server: a startup banner
//! and consistent, lightly-coloured event lines. Everything goes to stderr so
//! `render`'s HTML on stdout stays clean. Colour is auto-disabled when stderr
//! is not a TTY or when `NO_COLOR` is set.

use std::io::IsTerminal;
use std::sync::OnceLock;

#[derive(Clone, Copy)]
enum Style {
    Ready,
    Watch,
    Update,
    Source,
    Kernel,
    Warn,
    Error,
}

impl Style {
    /// The fixed-width tag and its ANSI colour code.
    fn parts(self) -> (&'static str, &'static str) {
        match self {
            Style::Ready => ("ready", "\x1b[32m"),   // green
            Style::Watch => ("watch", "\x1b[2m"),    // dim
            Style::Update => ("update", "\x1b[36m"), // cyan
            Style::Source => ("source", "\x1b[34m"), // blue
            Style::Kernel => ("kernel", "\x1b[35m"), // magenta
            Style::Warn => ("warn", "\x1b[33m"),     // yellow
            Style::Error => ("error", "\x1b[31m"),   // red
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
    eprintln!("  {} {msg}", paint(&format!("{tag:<6}"), code));
}

/// The opening banner: tool name + version.
pub fn banner(version: &str) {
    eprintln!();
    eprintln!(
        "  {} {}",
        paint("qmd-fast", "\x1b[1;32m"),
        paint(version, "\x1b[2m")
    );
}

/// The server is up; `url` is highlighted as the thing to open.
pub fn ready(url: &str) {
    line(Style::Ready, &paint(url, "\x1b[1m"));
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

/// A click-to-source request (a location the preview asked the editor to open).
pub fn source(loc: &str) {
    line(Style::Source, loc);
}

/// Kernel lifecycle / status.
pub fn kernel(msg: &str) {
    line(Style::Kernel, msg);
}

pub fn warn(msg: &str) {
    line(Style::Warn, msg);
}

pub fn error(msg: &str) {
    line(Style::Error, msg);
}
