//! Turning a session's event stream into something worth reading in a terminal.
//!
//! The session speaks the preview client's protocol (`cell-state`,
//! `cell-output-append`, `build-state`, `run-done`) because that is the stream a browser
//! already consumes, and reusing it verbatim is what stops the terminal and the browser
//! from ever disagreeing about what a cell produced. This module is the *only* place that
//! stream becomes prose.
//!
//! # Figures
//!
//! A cell's figure arrives as a base64 `data:` image, because that is what the HTML page
//! embeds. A terminal cannot show it, and the ways to make it try (Sixel, iTerm inline
//! images) need an opt-in setting, GPU acceleration, a non-Windows host, and a special
//! matplotlib backend. So the image is decoded to a file and its path printed: it works
//! over SSH, in any terminal, and the file is the same bytes the page will embed. An
//! editor makes the path clickable, which is the whole affordance needed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Renders one run's event stream to stdout and tracks whether it succeeded.
pub(crate) struct Printer {
    quiet: bool,
    /// Where decoded figures go: `<root>/_freeze/figs`. Beside the cache they belong to,
    /// so they are gitignored and disposable exactly like it.
    fig_dir: PathBuf,
    /// `cell_id` -> the 1-based ordinal shown to the author. Assigned in the order the
    /// session announces cells, which is document order.
    ordinals: HashMap<String, usize>,
    /// The cell currently printing output, so a run of appends does not repeat its header.
    open_cell: Option<String>,
    ran: usize,
    cached: usize,
    /// Cells the cap stopped short of, which have no cached output either. Reported
    /// separately from `cached` because they produced nothing at all, and rolling them
    /// into "cached" is how a capped run came to claim outputs that did not exist.
    skipped: usize,
    figs: usize,
    /// Set by a `run-done` with a non-ok status, or by any cell reporting `error`.
    failure: Option<String>,
    /// A `run-lagged` means this observer missed messages. The run is unaffected, but the
    /// transcript has holes and must not be presented as complete.
    lagged: bool,
    /// Error diagnostics already printed. The session re-broadcasts its full list every
    /// pass, so this is what stops one "kernel unavailable" becoming twenty.
    seen_diags: std::collections::HashSet<String>,
    /// Diagnostics in arrival order, printed only on failure (see [`Self::diagnostics`]).
    diags: Vec<String>,
}

impl Printer {
    pub(crate) fn new(quiet: bool, root: &Path) -> Self {
        Self {
            quiet,
            fig_dir: root.join("_freeze").join("figs"),
            ordinals: HashMap::new(),
            open_cell: None,
            ran: 0,
            cached: 0,
            skipped: 0,
            figs: 0,
            failure: None,
            lagged: false,
            seen_diags: std::collections::HashSet::new(),
            diags: Vec::new(),
        }
    }

    /// Consume one NDJSON line. Returns `true` when the run is over.
    pub(crate) fn consume(&mut self, line: &str) -> bool {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            return false; // not ours; the stream carries the page's whole protocol
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("cell-state") => self.cell_state(&v),
            Some("cell-output-append") => self.cell_output(&v),
            // The session's diagnostics carry the one explanation a failing cell often
            // cannot: "no kernel". Without this, a run with no interpreter printed a bare
            // `✗ cell 1` and left the author to guess — the error text never reaches
            // `cell-output-append` because the cell never ran to produce any.
            Some("diagnostics") => self.diagnostics(&v),
            Some("run-lagged") => {
                self.lagged = true;
                crate::log::warn(
                    "output was dropped: this run produced more than the stream could buffer",
                );
            }
            Some("run-done") => {
                if v.get("status").and_then(|s| s.as_str()) != Some("ok") {
                    self.failure = Some(
                        v.get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("the run failed")
                            .to_string(),
                    );
                }
                return true;
            }
            _ => {}
        }
        false
    }

    /// Collect the session's diagnostics, to be printed only if the run fails.
    ///
    /// Held rather than printed, because the two audiences differ. A successful run wants
    /// silence: a document with five lint warnings would otherwise reprint all five on
    /// every keystroke-to-run cycle, which is the opposite of fast iteration. A FAILED run
    /// wants every clue available, and the most important one — "kernel unavailable" — is
    /// `warn`-level (right for a preview, which still renders) and carries the only
    /// explanation a cell that never ran can offer. Deduped because the session
    /// re-broadcasts its whole list every pass.
    fn diagnostics(&mut self, v: &serde_json::Value) {
        let Some(list) = v.get("messages").and_then(|m| m.as_array()) else {
            return;
        };
        for d in list {
            let Some(msg) = d.get("message").and_then(|m| m.as_str()) else {
                continue;
            };
            if self.seen_diags.insert(msg.to_string()) {
                self.diags.push(msg.to_string());
            }
        }
    }

    fn cell_state(&mut self, v: &serde_json::Value) {
        let Some(id) = v.get("cell_id").and_then(|c| c.as_str()) else {
            return;
        };
        let next = self.ordinals.len() + 1;
        let n = *self.ordinals.entry(id.to_string()).or_insert(next);
        let state = v.get("state").and_then(|s| s.as_str()).unwrap_or("");
        let cached = v.get("source").and_then(|s| s.as_str()) == Some("cache");
        let ms = v.get("duration_ms").and_then(|d| d.as_u64());

        match state {
            "running" => {
                if !self.quiet {
                    println!("\x1b[1m▸ cell {n}\x1b[0m");
                }
                self.open_cell = Some(id.to_string());
            }
            "done" if cached => {
                // Counted but not narrated: a document with forty cached cells should not
                // bury the two that ran under thirty-eight lines saying nothing happened.
                self.cached += 1;
            }
            "done" => {
                self.ran += 1;
                if !self.quiet {
                    println!("\x1b[32m✓ cell {n}\x1b[0m{}", took(ms));
                }
                self.open_cell = None;
            }
            "skipped" => self.skipped += 1,
            "error" => {
                self.ran += 1;
                self.failure
                    .get_or_insert_with(|| format!("cell {n} failed"));
                println!("\x1b[31m✗ cell {n}\x1b[0m{}", took(ms));
                self.open_cell = None;
            }
            _ => {}
        }
    }

    fn cell_output(&mut self, v: &serde_json::Value) {
        if self.quiet {
            return;
        }
        let Some(html) = v.get("html").and_then(|h| h.as_str()) else {
            return;
        };
        let id = v.get("cell_id").and_then(|c| c.as_str()).unwrap_or("");
        // A `replace_last` is a `\r` progress bar redrawing itself. A terminal can do that
        // natively, so re-print in place rather than stacking a line per frame.
        let replace = v.get("op").and_then(|o| o.as_str()) == Some("replace_last");
        let n = self.ordinals.get(id).copied().unwrap_or(0);

        let (text, images) = split_output(html);
        if !text.trim().is_empty() {
            for l in text.lines() {
                if replace {
                    print!("\r\x1b[2K  {l}");
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                } else {
                    println!("  {l}");
                }
            }
        }
        for img in images {
            match self.write_figure(n, &img) {
                Ok(path) => {
                    self.figs += 1;
                    println!("  \x1b[36m→ {}\x1b[0m", path.display());
                }
                Err(e) => crate::log::warn(&format!("could not write the figure: {e}")),
            }
        }
    }

    /// Decode one base64 figure to `_freeze/figs/` and return its path.
    fn write_figure(&self, cell: usize, img: &Figure) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.fig_dir)?;
        let name = format!("cell{cell}-{}.{}", self.figs + 1, img.ext);
        let path = self.fig_dir.join(name);
        let bytes = base64_decode(&img.b64).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "malformed base64 image")
        })?;
        std::fs::write(&path, bytes)?;
        Ok(path)
    }

    /// The stream ended without a terminal message.
    pub(crate) fn truncated(&mut self) {
        crate::log::error("the session stopped before the run finished (kernel or server died)");
    }

    /// Print the summary and produce the process exit code.
    pub(crate) fn finish(&mut self) -> ExitCode {
        if let Some(why) = &self.failure {
            crate::log::error(why);
            // The session's own explanation, which for the commonest failure ("no kernel")
            // is the ONLY one: the cell never ran, so it produced no traceback to print.
            for d in &self.diags {
                crate::log::warn(d);
            }
            return ExitCode::FAILURE;
        }
        if !self.quiet {
            let mut parts = vec![format!("{} ran", self.ran)];
            if self.cached > 0 {
                parts.push(format!("{} cached", self.cached));
            }
            if self.skipped > 0 {
                parts.push(format!("{} below the cut, not run", self.skipped));
            }
            if self.figs > 0 {
                parts.push(format!("{} figure(s)", self.figs));
            }
            println!("\x1b[2m{}\x1b[0m", parts.join(", "));
        }
        // A run whose transcript has holes exits non-zero even though the cells
        // themselves succeeded: a script must not read a truncated log as a clean pass.
        if self.lagged {
            return ExitCode::FAILURE;
        }
        ExitCode::SUCCESS
    }
}

/// `  2.4s` / `  840ms`, or empty when the session reported no duration.
fn took(ms: Option<u64>) -> String {
    match ms {
        None => String::new(),
        Some(ms) if ms < 1000 => format!("  \x1b[2m{ms}ms\x1b[0m"),
        Some(ms) => format!("  \x1b[2m{:.1}s\x1b[0m", ms as f64 / 1000.0),
    }
}

/// One image pulled out of a cell's output HTML.
pub(crate) struct Figure {
    pub(crate) b64: String,
    pub(crate) ext: &'static str,
}

/// Split a cell's output HTML into terminal text and the figures to write out.
///
/// A theme-paired figure emits a light AND a dark `<img>` of the same plot; only the
/// light one is kept, because printing two paths for one figure is noise, not information.
pub(crate) fn split_output(html: &str) -> (String, Vec<Figure>) {
    let mut images = Vec::new();
    let mut text = String::new();
    let mut rest = html;
    while let Some(start) = rest.find("<img") {
        text.push_str(&rest[..start]);
        let tag_end = match rest[start..].find('>') {
            Some(i) => start + i + 1,
            None => rest.len(),
        };
        let tag = &rest[start..tag_end];
        if !tag.contains("tali-fig-dark")
            && let Some(fig) = data_uri_image(tag)
        {
            images.push(fig);
        }
        rest = &rest[tag_end..];
    }
    text.push_str(rest);
    (tags_to_text(&text), images)
}

/// The base64 payload of a `src="data:image/<ext>;base64,..."` attribute in one tag.
fn data_uri_image(tag: &str) -> Option<Figure> {
    let at = tag.find("data:image/")?;
    let after = &tag[at + "data:image/".len()..];
    let (kind, after) = after.split_once(';')?;
    let b64 = after.strip_prefix("base64,")?;
    let end = b64.find(['"', '\''])?;
    let ext = match kind {
        "jpeg" | "jpg" => "jpg",
        "svg+xml" => "svg",
        "webp" => "webp",
        _ => "png",
    };
    Some(Figure {
        b64: b64[..end].to_string(),
        ext,
    })
}

/// Strip tags and unescape entities, keeping the line structure a `<pre>` carries.
///
/// Not a general HTML renderer and not trying to be: a cell's output is overwhelmingly a
/// `<pre>` of text, and the alternative for the rest (a table) reads better as its rows
/// than as an attempt at ASCII art.
fn tags_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => {
                // A block-level close is a line break, so table rows and paragraphs do not
                // run together into one unreadable line.
                let rest: String = chars.clone().take(4).collect();
                if rest.starts_with("/tr") || rest.starts_with("/p") || rest.starts_with("/div") {
                    out.push('\n');
                }
                in_tag = true;
            }
            '>' if in_tag => in_tag = false,
            '&' if !in_tag => {
                let entity: String = chars.clone().take(6).collect();
                if let Some((decoded, len)) = decode_entity(&entity) {
                    out.push(decoded);
                    for _ in 0..len {
                        chars.next();
                    }
                } else {
                    out.push('&');
                }
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    // Collapse the blank runs the tag stripping leaves behind.
    let mut lines: Vec<&str> = Vec::new();
    for l in out.lines() {
        let l = l.trim_end();
        if l.trim().is_empty() && lines.last().map(|p: &&str| p.trim().is_empty()) == Some(true) {
            continue;
        }
        lines.push(l);
    }
    lines.join("\n").trim_matches('\n').to_string()
}

/// Decode one HTML entity at the start of `s`, returning it and how many chars it spans
/// after the `&`.
fn decode_entity(s: &str) -> Option<(char, usize)> {
    for (name, ch) in [
        ("amp;", '&'),
        ("lt;", '<'),
        ("gt;", '>'),
        ("quot;", '"'),
        ("#39;", '\''),
        ("#x27;", '\''),
        ("nbsp;", ' '),
    ] {
        if s.starts_with(name) {
            return Some((ch, name.len()));
        }
    }
    None
}

/// Standard base64 decode. Hand-rolled to keep the dependency graph where it is; the
/// input is a data URI this same binary produced.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (i, b) in TABLE.iter().enumerate() {
        lookup[*b as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for b in s.bytes() {
        // Whitespace and `=` padding carry no data; everything else must be alphabet.
        if b == b'=' || b.is_ascii_whitespace() {
            continue;
        }
        let v = lookup[b as usize];
        if v == 255 {
            return None;
        }
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_the_encoder_the_kernel_uses() {
        // Cases chosen for the padding boundaries: 3-byte groups, and the 1- and 2-byte
        // remainders that produce `==` and `=`.
        for (b64, want) in [
            ("TWFu", "Man"),
            ("TWE=", "Ma"),
            ("TQ==", "M"),
            ("aGVsbG8gd29ybGQ=", "hello world"),
        ] {
            assert_eq!(
                base64_decode(b64).unwrap(),
                want.as_bytes(),
                "decoding {b64}"
            );
        }
        // PNG magic, the actual thing being decoded here.
        assert_eq!(
            base64_decode("iVBORw0KGgo=").unwrap(),
            [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        assert!(base64_decode("not base64!").is_none());
    }

    #[test]
    fn base64_ignores_whitespace_and_padding() {
        assert_eq!(base64_decode("TWFu\n").unwrap(), b"Man");
        assert_eq!(base64_decode("TWE = ").unwrap(), b"Ma");
    }

    #[test]
    fn a_pre_block_keeps_its_lines_and_unescapes_entities() {
        let (text, figs) =
            split_output("<pre class=\"tali-out\">a &lt; b &amp;&amp; c\nline2</pre>");
        assert_eq!(text, "a < b && c\nline2");
        assert!(figs.is_empty());
    }

    #[test]
    fn a_theme_paired_figure_yields_exactly_one_file() {
        // The kernel emits BOTH variants of one plot. Two printed paths for one figure
        // would read as two figures.
        let html = concat!(
            "<img class=\"tali-fig tali-fig-light\" alt=\"\" src=\"data:image/png;base64,TWFu\">",
            "<img class=\"tali-fig tali-fig-dark\" alt=\"\" src=\"data:image/png;base64,TWE=\">"
        );
        let (_, figs) = split_output(html);
        assert_eq!(figs.len(), 1, "expected one figure, got {}", figs.len());
        assert_eq!(figs[0].b64, "TWFu", "must keep the LIGHT variant");
        assert_eq!(figs[0].ext, "png");
    }

    #[test]
    fn figure_extension_follows_the_data_uri_type() {
        for (mime, want) in [
            ("png", "png"),
            ("jpeg", "jpg"),
            ("svg+xml", "svg"),
            ("webp", "webp"),
        ] {
            let html = format!("<img src=\"data:image/{mime};base64,TWFu\">");
            let (_, figs) = split_output(&html);
            assert_eq!(figs[0].ext, want, "for image/{mime}");
        }
    }

    #[test]
    fn text_around_a_figure_survives_the_split() {
        let (text, figs) = split_output(
            "<pre>before</pre><img src=\"data:image/png;base64,TWFu\"><pre>after</pre>",
        );
        assert_eq!(figs.len(), 1);
        assert!(text.contains("before"), "got {text:?}");
        assert!(text.contains("after"), "got {text:?}");
    }

    #[test]
    fn a_table_becomes_one_line_per_row() {
        let (text, _) = split_output(
            "<table><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>",
        );
        assert_eq!(text.lines().count(), 2, "expected 2 rows, got {text:?}");
    }

    #[test]
    fn an_img_without_a_data_uri_is_not_a_figure() {
        // A cell that emits `<img src="plot.png">` (a file it wrote itself) has nothing to
        // decode; inventing an empty file for it would be worse than ignoring it.
        let (_, figs) = split_output("<img src=\"plot.png\">");
        assert!(figs.is_empty());
    }

    #[test]
    fn an_unterminated_img_tag_does_not_hang_or_panic() {
        // Truncated output is reachable: the stream is delivered mid-cell.
        let (_, figs) = split_output("<pre>x</pre><img src=\"data:image/png;base64,TWFu");
        assert!(figs.is_empty(), "a tag with no `>` yields no figure");
    }

    #[test]
    fn durations_render_at_both_scales() {
        assert!(took(Some(840)).contains("840ms"));
        assert!(took(Some(2400)).contains("2.4s"));
        assert_eq!(took(None), "");
    }

    #[test]
    fn ordinals_follow_announcement_order_and_are_stable() {
        let dir = std::env::temp_dir();
        let mut p = Printer::new(true, &dir);
        for id in ["c-aaa", "c-bbb", "c-aaa"] {
            p.consume(&crate::protocol::cell_state(
                None, id, "queued", None, None, None,
            ));
        }
        assert_eq!(p.ordinals.get("c-aaa"), Some(&1));
        assert_eq!(p.ordinals.get("c-bbb"), Some(&2), "second cell seen is 2");
    }

    #[test]
    fn a_cached_cell_is_counted_but_a_failure_sets_the_exit_code() {
        let dir = std::env::temp_dir();
        let mut p = Printer::new(true, &dir);
        p.consume(&crate::protocol::cell_state(
            None,
            "c1",
            "done",
            None,
            None,
            Some("cache"),
        ));
        assert_eq!(p.cached, 1);
        assert!(p.failure.is_none());

        p.consume(&crate::protocol::cell_state(
            None, "c2", "error", None, None, None,
        ));
        assert!(p.failure.is_some(), "an errored cell must fail the run");
    }

    #[test]
    fn run_done_ends_the_stream_and_a_bad_status_fails() {
        let dir = std::env::temp_dir();
        let mut p = Printer::new(true, &dir);
        assert!(
            !p.consume(&crate::protocol::cell_state(
                None, "c1", "queued", None, None, None
            )),
            "a cell-state must not end the stream"
        );
        assert!(
            p.consume(&crate::protocol::run_done(
                None,
                "rid",
                "error",
                Some("boom")
            )),
            "run-done must end the stream"
        );
        assert_eq!(p.failure.as_deref(), Some("boom"));
    }

    #[test]
    fn a_lagged_stream_fails_even_when_every_cell_succeeded() {
        // A transcript with holes must not be reported as a clean pass to a script.
        let dir = std::env::temp_dir();
        let mut p = Printer::new(true, &dir);
        p.consume(r#"{"type":"run-lagged","dropped":7}"#);
        assert!(p.lagged);
        // `ExitCode` has no comparison; assert through the flag the code is derived from.
        assert!(p.failure.is_none(), "no cell failed");
        assert!(p.lagged, "yet the run must not be reported clean");
    }

    #[test]
    fn a_non_json_line_is_ignored_rather_than_fatal() {
        let dir = std::env::temp_dir();
        let mut p = Printer::new(true, &dir);
        assert!(!p.consume("this is not json"));
        assert!(!p.consume(""));
    }
}
