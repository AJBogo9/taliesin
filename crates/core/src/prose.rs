//! The prose selection: what counts as prose in a `.tmd`, and how many words of it there
//! are. [`word_count`] is the LSP outline's section length, the one consumer left of the
//! reading-time estimate, the book chapter-cost signal and `map` it once also served.
//!
//! This module was also an opt-in prose LINTER (doubled words, weasel words, a
//! `prose-lint: { banned: [...] }` list). It was retired on 2026-08-02: it was opt-in and
//! never opted into, by the person who writes daily.

/// Count prose words in markdown `src`: its lines as the render reads them, skipping front
/// matter, code (fenced or indented), raw HTML (a comment, `<pre>`) and `:::` div fences,
/// with inline code/math/links/HTML blanked by [`strip_inline`]. What is code is the
/// render's own answer ([`crate::render::rendered_lines`]). `src` is expected
/// include-expanded (so an included file's prose counts).
pub fn word_count(src: &str) -> usize {
    let lines = crate::render::rendered_lines(src);
    src.lines()
        .enumerate()
        // `:::` div fence lines carry attributes, not prose.
        .filter(|(i, raw)| {
            lines.line(*i).kind.is_markdown() && !raw.trim_start().starts_with(":::")
        })
        .map(|(_, raw)| words(&strip_inline(raw)).len())
        .sum()
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

    /// Audit 2026-09-24, B2: prose is what the render reads as markdown. Indented code and a
    /// commented-out draft are not prose, and a paragraph that starts with inline code in
    /// triple backticks does not open a "fence" that swallows the words after it.
    #[test]
    fn prose_is_what_the_render_reads_as_markdown() {
        assert_eq!(
            word_count("One two.\n\n    not counted\n\n<!--\nnot counted\n-->\n"),
            2
        );
        assert_eq!(word_count("```pip``` then\n\nthree more words\n"), 4);
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
