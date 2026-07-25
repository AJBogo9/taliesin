//! Opt-in, markdown-aware prose linter. Diagnostic-only: [`lint`] returns `(line, message)`
//! pairs that `render` maps into located, click-to-source warnings (the same channel as
//! broken xrefs / unknown shortcodes). Three high-precision rules — doubled words, weasel
//! words, a custom banned-terms list — skipping code, math, links, and HTML so only prose is
//! checked. Off unless the doc opts in via the `prose-lint` front-matter key ([`config`]).

use serde_yaml::Value;

/// Resolved `prose-lint` configuration (the linter is off when [`config`] returns `None`).
pub(crate) struct ProseLint {
    pub banned: Vec<String>,
}

/// Conservative, well-known hedges. Whole-word, case-insensitive.
const WEASEL_WORDS: &[&str] = &[
    "very",
    "really",
    "quite",
    "just",
    "actually",
    "basically",
    "simply",
    "clearly",
    "obviously",
    "essentially",
    "fairly",
    "somewhat",
    "rather",
];

/// Parse the `prose-lint` front-matter key. `None` = linter off. `true` enables the built-in
/// rules; a mapping additionally reads a `banned` string list.
pub(crate) fn config(front_matter: &str) -> Option<ProseLint> {
    let value: Value = serde_yaml::from_str(front_matter).ok()?;
    let pl = value.get("prose-lint")?;
    match pl {
        Value::Bool(true) => Some(ProseLint { banned: Vec::new() }),
        Value::Mapping(_) => {
            let banned = pl
                .get("banned")
                .and_then(Value::as_sequence)
                .map(|seq| {
                    seq.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            Some(ProseLint { banned })
        }
        _ => None, // false / null / scalar -> off
    }
}

/// Scan markdown `src` for prose-rule violations. Returns `(1-based line, message)`.
pub(crate) fn lint(src: &str, cfg: &ProseLint) -> Vec<(usize, String)> {
    let banned: Vec<String> = cfg.banned.iter().map(|b| b.to_lowercase()).collect();
    let mut out = Vec::new();
    for_each_prose_line(src, |line_no, text| {
        scan_line(text, line_no, &banned, &mut out)
    });
    out
}

/// Count prose words in markdown `src` — the reading-time measure. Reuses the exact
/// prose-selection of [`lint`] (front matter, fenced code, `:::` fences, and inline
/// code/math/links/HTML all excluded), matching the client's live count that drops
/// `<pre>`/`.katex` from the DOM. `src` is expected include-expanded (so an included
/// file's prose counts). Rounding to whole minutes lives at the call site.
pub fn word_count(src: &str) -> usize {
    let mut n = 0;
    for_each_prose_line(src, |_, text| n += words(text).len());
    n
}

/// Walk `src`'s prose lines — skipping front matter, fenced code blocks, and `:::` div
/// fences — invoking `f(1-based line, stripped)` with the [`strip_inline`]'d prose text of
/// each remaining line. The single source of "what counts as prose", shared by [`lint`]
/// and [`word_count`] so they can never disagree.
fn for_each_prose_line(src: &str, mut f: impl FnMut(usize, &str)) {
    let mut in_front = false;
    let mut fence: Option<char> = None; // inside a ``` or ~~~ code block
    for (i, raw) in src.lines().enumerate() {
        let t = raw.trim_start();
        // Front matter: a leading `---` (line 1 only) opens; the next `---`/`...` closes.
        if i == 0 && t == "---" {
            in_front = true;
            continue;
        }
        if in_front {
            if t == "---" || t == "..." {
                in_front = false;
            }
            continue;
        }
        // Fenced code blocks: skip the fence lines and everything between.
        if let Some(f) = fence {
            if (f == '`' && t.starts_with("```")) || (f == '~' && t.starts_with("~~~")) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") {
            fence = Some('`');
            continue;
        }
        if t.starts_with("~~~") {
            fence = Some('~');
            continue;
        }
        // `:::` div fence lines carry attributes, not prose.
        if t.starts_with(":::") {
            continue;
        }
        let text = strip_inline(raw);
        f(i + 1, &text);
    }
}

/// Blank out inline code, math, link/image targets, autolinks, and HTML tags (replaced with
/// spaces, so word boundaries survive) leaving only prose text. Line numbers are all we need,
/// so per-byte space padding is fine.
fn strip_inline(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let blank = |out: &mut String, n: usize| {
        for _ in 0..n {
            out.push(' ');
        }
    };
    while i < line.len() {
        if bytes[i] == b'`' {
            let run = line[i..].bytes().take_while(|&b| b == b'`').count();
            let ticks = &line[i..i + run];
            if let Some(rel) = line[i + run..].find(ticks) {
                let close = i + run + rel + run;
                blank(&mut out, close - i);
                i = close;
            } else {
                blank(&mut out, run);
                i += run;
            }
        } else if bytes[i] == b'$' {
            let marker = if line[i..].starts_with("$$") {
                "$$"
            } else {
                "$"
            };
            let start = i + marker.len();
            if let Some(rel) = line[start..].find(marker) {
                let close = start + rel + marker.len();
                blank(&mut out, close - i);
                i = close;
            } else {
                out.push('$');
                i += 1;
            }
        } else if line[i..].starts_with("](") {
            if let Some(rel) = line[i + 2..].find(')') {
                let close = i + 2 + rel + 1;
                blank(&mut out, close - i);
                i = close;
            } else {
                out.push_str("](");
                i += 2;
            }
        } else if bytes[i] == b'<' {
            if let Some(rel) = line[i..].find('>') {
                let close = i + rel + 1;
                blank(&mut out, close - i);
                i = close;
            } else {
                out.push('<');
                i += 1;
            }
        } else {
            let ch = line[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Maximal runs of alphanumeric + apostrophe, as the prose "words".
fn words(text: &str) -> Vec<String> {
    let mut ws = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '\'' {
            cur.push(ch);
        } else if !cur.is_empty() {
            ws.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        ws.push(cur);
    }
    ws
}

fn scan_line(text: &str, line_no: usize, banned: &[String], out: &mut Vec<(usize, String)>) {
    let ws = words(text);
    let mut prev: Option<String> = None;
    for w in &ws {
        let lw = w.to_lowercase();
        let is_alpha = lw.chars().next().is_some_and(|c| c.is_alphabetic());
        if is_alpha && prev.as_deref() == Some(lw.as_str()) {
            out.push((line_no, format!("repeated word `{lw}`")));
        }
        if WEASEL_WORDS.contains(&lw.as_str()) {
            out.push((line_no, format!("weasel word `{lw}` (consider cutting)")));
        }
        if banned.contains(&lw) {
            out.push((line_no, format!("banned term `{lw}`")));
        }
        prev = Some(lw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(banned: &[&str]) -> ProseLint {
        ProseLint {
            banned: banned.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn config_off_unless_opted_in() {
        assert!(config("title: T").is_none());
        assert!(config("prose-lint: false").is_none());
        assert!(config("prose-lint: true").is_some());
    }

    #[test]
    fn config_reads_banned_list() {
        let c = config("prose-lint:\n  banned: [utilize, leverage]").expect("on");
        assert_eq!(c.banned, vec!["utilize", "leverage"]);
    }

    #[test]
    fn flags_doubled_words() {
        let w = lint("We we should fix it.", &cfg(&[]));
        assert_eq!(w, vec![(1, "repeated word `we`".to_string())]);
    }

    #[test]
    fn flags_weasel_words() {
        let w = lint("This is very fast and really clever.", &cfg(&[]));
        assert!(w.contains(&(1, "weasel word `very` (consider cutting)".to_string())));
        assert!(w.contains(&(1, "weasel word `really` (consider cutting)".to_string())));
    }

    #[test]
    fn flags_banned_terms_case_insensitively() {
        let w = lint("Please Utilize the API.", &cfg(&["utilize"]));
        assert_eq!(w, vec![(1, "banned term `utilize`".to_string())]);
    }

    #[test]
    fn skips_code_math_link_urls_and_fences() {
        // `utilize` in code, `very` in math, and `utilize` inside a link URL must all be
        // skipped (link *text* is prose and would be linted, so keep it clean here); the
        // fenced block is skipped too.
        let src = "`utilize` code, $very$ math, and a [plain link](http://utilize.example) stay fine.\n\n```\nutilize very very\n```\n";
        let w = lint(src, &cfg(&["utilize"]));
        assert!(
            w.is_empty(),
            "code, math, link URLs, and fences must be skipped, got: {w:?}"
        );
    }

    #[test]
    fn reports_correct_line_numbers() {
        let src = "Clean line.\nAnother clean one.\nPlease utilize this.";
        let w = lint(src, &cfg(&["utilize"]));
        assert_eq!(w, vec![(3, "banned term `utilize`".to_string())]);
    }
}
