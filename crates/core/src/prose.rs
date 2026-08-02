//! The prose selection: what counts as prose in a `.tmd`, and how many words of it there
//! are. [`word_count`] is the reading-time measure, and via [`for_each_prose_line`] it is
//! the single definition of "prose" — front matter, fenced code, `:::` fences, and inline
//! code/math/links/HTML all excluded — shared by the reading-time estimate, the book
//! chapter-cost signal, the LSP outline, and `map`.
//!
//! This module was also an opt-in prose LINTER (doubled words, weasel words, a
//! `prose-lint: { banned: [...] }` list). It was retired on 2026-08-02: it was opt-in and
//! never opted into, by the person who writes daily. The selection walk survives because it
//! always had the other, load-bearing consumer.

/// Count prose words in markdown `src` — the reading-time measure. Uses the prose
/// selection of [`for_each_prose_line`] (front matter, fenced code, `:::` fences, and
/// inline code/math/links/HTML all excluded), matching the client's live count that drops
/// `<pre>`/`.katex` from the DOM. `src` is expected include-expanded (so an included
/// file's prose counts). Rounding to whole minutes lives at the call site.
pub fn word_count(src: &str) -> usize {
    let mut n = 0;
    for_each_prose_line(src, |_, text| n += words(text).len());
    n
}

/// Walk `src`'s prose lines — skipping front matter, fenced code blocks, and `:::` div
/// fences — invoking `f(1-based line, stripped)` with the [`strip_inline`]'d prose text of
/// each remaining line. The single source of "what counts as prose"; [`word_count`] is its
/// only caller today, and it stays a separate walk so the next prose measure cannot
/// disagree with the reading time.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_prose_words_only() {
        assert_eq!(word_count("Two words."), 2);
    }

    #[test]
    fn skips_front_matter_code_math_and_fences() {
        // Everything but "Prose here." is excluded from the selection: front matter, a
        // fenced block, inline code, display math, and a `:::` fence.
        let src = "---\ntitle: A Long Title\n---\n\nProse here.\n\n\
`ignored_code` $x + y$\n\n```\nfn ignored() {}\n```\n\n\
::: {.callout-note}\n:::\n";
        assert_eq!(word_count(src), 2, "only `Prose here.` counts");
    }

    #[test]
    fn link_text_counts_but_the_url_does_not() {
        // "See the docs" — the URL's own path segments are not prose.
        assert_eq!(
            word_count("See [the docs](http://example.com/deep/path)."),
            3
        );
    }
}
