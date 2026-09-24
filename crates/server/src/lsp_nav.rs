//! Pure, LSP-free navigation helpers for `.tmd`: classify the token under the cursor and
//! resolve xref/cite/include definitions. Hand-rolled scanning (no `regex` dependency; the
//! tokens are simple), so the `lsp` server can answer go-to-definition for any editor.
//!
//! This began as a port of an equivalent TypeScript layer in the VS Code companion. That
//! copy is **gone** (2026-07-28): the companion is now a thin client over `taliesin lsp`,
//! and this is the only implementation. Do not reintroduce a second one — see
//! `notes/2026-07-28-vscode-companion-audit.md` for what two copies of one contract cost.
//!
//! Offsets are scalar (`char`) based, matching the diagnostics slice's `to_lsp`; the `lsp`
//! server converts them to/from the wire's UTF-16 columns at its boundary (`lsp_pos`), so
//! the answer is correct for all text, astral characters included.

/// The token under the cursor, with its 0-based `[start, end)` char span on the line.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Target {
    None,
    Xref {
        id: String,
        start: usize,
        end: usize,
    },
    Cite {
        key: String,
        start: usize,
        end: usize,
    },
    Include {
        path: String,
        start: usize,
        end: usize,
    },
    FrontmatterKey {
        key: String,
        parent: Option<String>,
        start: usize,
        end: usize,
    },
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
fn is_xref_id_char(c: char) -> bool {
    is_word(c) || c == '-'
}
fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}
/// Inclusive of both ends, so a cursor just past the last char still hovers the token
/// (matches the editor's word-range behaviour).
fn covers(s: usize, e: usize, ch: usize) -> bool {
    ch >= s && ch <= e
}

/// One `$…$` / `$$…$$` span, reduced to what its one caller asks: where it opens, and
/// whether it ever closed.
///
/// It carried the expression itself (`latex`), its `display` flag and its `end` offset for
/// the math hover, which went with `math_preview.rs` on 2026-08-09. Completion only needs to
/// know that the cursor is inside an open span.
pub(crate) struct MathSpan {
    pub(crate) start: usize,
    /// `false` for a span whose closing delimiter has not been typed yet — which is the
    /// case completion cares about.
    pub(crate) closed: bool,
}

/// Every math span in `text`, in document order.
///
/// This is the single owner of Taliesin's `$` delimiter rules, which is what keeps
/// completion's "am I inside math?" from becoming a second delimiter scanner: a `\` escapes
/// the next character, a fenced block is code and not math, an inline `$…$` is abandoned at a
/// line break (`render::math_close` gives up at `\n`) while `$$…$$` survives one, and an
/// opening `$` must be followed by a non-space, which is what keeps `$ 5` from opening math.
///
/// A span still open at end-of-input is returned with `closed: false`; one abandoned at a
/// line break is not returned at all, because it was never math.
pub(crate) fn scan_math(text: &str) -> Vec<MathSpan> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut spans: Vec<MathSpan> = Vec::new();
    let mut display: Option<usize> = None;
    let mut inline: Option<usize> = None;
    let mut in_code = false;
    let close = |spans: &mut Vec<MathSpan>, open: usize| {
        spans.push(MathSpan {
            start: open,
            closed: true,
        });
    };

    let mut i = 0;
    while i <= n {
        let line_start = i;
        let mut line_end = line_start;
        while line_end < n && chars[line_end] != '\n' {
            line_end += 1;
        }
        // Read the fence marker off the char slice rather than materializing the line:
        // completion calls this on every keystroke, over the whole buffer prefix.
        let fence = {
            let mut k = line_start;
            while k < line_end && chars[k].is_whitespace() {
                k += 1;
            }
            let run = |c: char| {
                k + 2 < line_end && chars[k] == c && chars[k + 1] == c && chars[k + 2] == c
            };
            run('`') || run('~')
        };
        // An inline span never survives a line break or a fence boundary; drop it unrecorded.
        inline = None;
        if fence {
            in_code = !in_code;
        } else if !in_code {
            let mut j = line_start;
            while j < line_end {
                match chars[j] {
                    '\\' => j += 2, // an escape consumes the next char, so `\$` is literal
                    '$' if j + 1 < line_end && chars[j + 1] == '$' => {
                        match display.take() {
                            Some(open) => close(&mut spans, open),
                            None => display = Some(j),
                        }
                        j += 2;
                    }
                    '$' => {
                        match inline.take() {
                            // Closing never checks the guard; only an OPEN needs a non-space.
                            Some(open) => close(&mut spans, open),
                            None => {
                                if chars.get(j + 1).is_some_and(|c| !c.is_whitespace()) {
                                    inline = Some(j);
                                }
                            }
                        }
                        j += 1;
                    }
                    _ => j += 1,
                }
            }
        }
        if line_end >= n {
            break;
        }
        i = line_end + 1;
    }
    // Whatever is still open at end-of-input is a span the author is mid-way through typing.
    for open in [display, inline].into_iter().flatten() {
        spans.push(MathSpan {
            start: open,
            closed: false,
        });
    }
    spans.sort_by_key(|s| s.start);
    spans
}

/// Classify the token at 0-based (`line`, `character`). A key of a citation group wins over
/// a bare xref `@k`; a front-matter key is recognized only inside the `---` body, on the key
/// token.
pub(crate) fn classify_target(text: &str, line: usize, character: usize) -> Target {
    let lines: Vec<&str> = crate::lsp_pos::lines(text).collect();
    let lt: Vec<char> = lines.get(line).copied().unwrap_or("").chars().collect();
    let n = lt.len();

    // A key of a citation group first, read by the render's own grammar (its `@` must not
    // be read as an xref). A cross-reference key in a group (`[@fig-x]`) renders as the
    // cross-reference it names, so it navigates as one.
    if let Some((key, span)) =
        taliesin_core::cite::citation_key_at(lines.get(line).copied().unwrap_or(""), character)
    {
        return if taliesin_core::cite::is_xref_anchor(&key) {
            Target::Xref {
                id: key,
                start: span.start,
                end: span.end,
            }
        } else {
            Target::Cite {
                key,
                start: span.start,
                end: span.end,
            }
        };
    }

    // Cross-reference `@id`, where `@` is not preceded by a word char, `@`, or `[`.
    let mut i = 0;
    while i < n {
        if lt[i] == '@' {
            let prev_ok = i == 0 || {
                let p = lt[i - 1];
                !is_word(p) && p != '@' && p != '['
            };
            if prev_ok {
                let id_start = i + 1;
                let mut j = id_start;
                while j < n && is_xref_id_char(lt[j]) {
                    j += 1;
                }
                if j > id_start && covers(i, j, character) {
                    return Target::Xref {
                        id: lt[id_start..j].iter().collect(),
                        start: i,
                        end: j,
                    };
                }
            }
        }
        i += 1;
    }

    // Include shortcode path.
    if let Some(t) = classify_include(&lt, character) {
        return t;
    }

    // Front-matter key.
    if let Some(t) = classify_frontmatter_key(&lines, line, character) {
        return t;
    }

    // A `$…$` span was classified last here until 2026-08-09, when the math hover went with
    // `math_preview.rs`. Nothing asks a position what expression encloses it any more;
    // `scan_math` survives because completion still needs to know it is inside math.
    Target::None
}

fn classify_include(lt: &[char], character: usize) -> Option<Target> {
    let n = lt.len();
    let mut i = 0;
    while i + 3 <= n {
        if lt[i] == '{' && lt[i + 1] == '{' && lt[i + 2] == '<' {
            let mut j = i + 3;
            while j < n && (lt[j] == ' ' || lt[j] == '\t') {
                j += 1;
            }
            let mut kw = String::new();
            while j < n && lt[j].is_ascii_alphabetic() {
                kw.push(lt[j]);
                j += 1;
            }
            if kw == "include" {
                let ws_start = j;
                while j < n && (lt[j] == ' ' || lt[j] == '\t') {
                    j += 1;
                }
                if j > ws_start {
                    let path_start = j;
                    while j < n && lt[j] != ' ' && lt[j] != '\t' && lt[j] != '>' {
                        j += 1;
                    }
                    if j > path_start && covers(path_start, j, character) {
                        return Some(Target::Include {
                            path: lt[path_start..j].iter().collect(),
                            start: path_start,
                            end: j,
                        });
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn classify_frontmatter_key(lines: &[&str], line: usize, character: usize) -> Option<Target> {
    let (start_line, end_line) = frontmatter_body(lines)?;
    if line < start_line || line >= end_line {
        return None;
    }
    let chars: Vec<char> = lines.get(line).copied().unwrap_or("").chars().collect();
    let mut k = 0;
    while k < chars.len() && (chars[k] == ' ' || chars[k] == '\t') {
        k += 1;
    }
    let indent = k;
    let key_start = k;
    while k < chars.len() && (is_word(chars[k]) || chars[k] == '-') {
        k += 1;
    }
    let key_end = k;
    if key_end > key_start
        && k < chars.len()
        && chars[k] == ':'
        && covers(indent, key_end, character)
    {
        return Some(Target::FrontmatterKey {
            key: chars[key_start..key_end].iter().collect(),
            parent: nested_parent_of(lines, line, indent),
            start: indent,
            end: key_end,
        });
    }
    None
}

/// The `[start, end)` line range of the front-matter body (key lines between the fences),
/// or None when there is no closed `---` block. 0-based over `lines`.
fn frontmatter_body(lines: &[&str]) -> Option<(usize, usize)> {
    if lines.first().map(|l| l.trim()) != Some("---") {
        return None;
    }
    for (i, l) in lines.iter().enumerate().skip(1) {
        let t = l.trim();
        if t == "---" || t == "..." {
            return Some((1, i));
        }
    }
    None
}

/// The nearest less-indented ancestor key (a recognized nested parent) above `line`.
fn nested_parent_of(lines: &[&str], line: usize, indent: usize) -> Option<String> {
    if indent == 0 {
        return None;
    }
    for i in (0..line).rev() {
        let raw = lines[i];
        if raw.trim().is_empty() {
            continue;
        }
        let line_indent = raw.len() - raw.trim_start().len();
        if line_indent < indent {
            let trimmed = raw.trim();
            let key: String = trimmed
                .chars()
                .take_while(|c| is_word(*c) || *c == '-')
                .collect();
            let has_colon = trimmed[key.len()..].starts_with(':');
            return if has_colon
                && crate::lsp_complete::nested_parents()
                    .iter()
                    .any(|p| p == &key)
            {
                Some(key)
            } else {
                None
            };
        }
    }
    None
}

fn offset_to_line_col(chars: &[char], idx: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut col = 0u32;
    for &c in &chars[..idx] {
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// The 0-based (line, col) where cross-reference `id` is DEFINED in `text`: the first
/// occurrence preceded by `#` (a `{#id}` attribute) or `label:` (a `#| label: id` cell),
/// never `@id` (a reference). None when the id is not defined here.
pub(crate) fn definition_site(text: &str, id: &str) -> Option<(u32, u32)> {
    let chars: Vec<char> = text.chars().collect();
    let idc: Vec<char> = id.chars().collect();
    let (n, m) = (chars.len(), idc.len());
    if m == 0 {
        return None;
    }
    let mut i = 0;
    while i + m <= n {
        if chars[i..i + m] == idc[..] {
            let after_ok = i + m >= n || !is_xref_id_char(chars[i + m]);
            let prefix_ok = (i > 0 && chars[i - 1] == '#') || {
                let mut j = i;
                while j > 0 && is_ws(chars[j - 1]) {
                    j -= 1;
                }
                j >= 6 && chars[j - 6..j].iter().collect::<String>() == "label:"
            };
            if after_ok && prefix_ok {
                return Some(offset_to_line_col(&chars, i));
            }
        }
        i += 1;
    }
    None
}

/// The `.bib` files a citation in the buffer at `uri` resolves against: the page's own and
/// its project's shared `bibliography:`, in the order the render reads them, so a later
/// file's entry wins a key two files define.
pub(crate) fn bib_files(uri: &lsp_types::Url, text: &str) -> Vec<std::path::PathBuf> {
    let Some(dir) = uri
        .to_file_path()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
    else {
        return Vec::new();
    };
    taliesin_core::render::bibliography_files(text, &dir)
}

/// The entry the render cites for `key` among `files` (see [`bib_files`]): the file, its
/// text and the entry's byte range in it, read by core's `.bib` parser. The last definition
/// wins, as it does in the render: a later file over an earlier one, and within one file.
pub(crate) fn bib_entry(
    files: &[std::path::PathBuf],
    key: &str,
) -> Option<(std::path::PathBuf, String, std::ops::Range<usize>)> {
    files.iter().rev().find_map(|path| {
        let text = std::fs::read_to_string(path).ok()?;
        let (_, span) = taliesin_core::cite::entry_spans(&text)
            .into_iter()
            .rfind(|(k, _)| k == key)?;
        Some((path.clone(), text, span))
    })
}

/// Every key the bibliography stores from `files`, sorted: the keys a citation can name.
pub(crate) fn bib_keys(files: &[std::path::PathBuf]) -> std::collections::BTreeSet<String> {
    files
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .flat_map(|text| taliesin_core::cite::entry_spans(&text))
        .map(|(key, _)| key)
        .collect()
}

/// The 0-based (line, scalar column) of byte `at` in `text`, counting lines the way the
/// editor does (`lsp_pos::lines`).
pub(crate) fn line_col(text: &str, at: usize) -> (u32, u32) {
    let before = &text[..at];
    let lines = crate::lsp_pos::lines(before).count().saturating_sub(1);
    let col = crate::lsp_pos::lines(before)
        .last()
        .unwrap_or("")
        .chars()
        .count();
    (lines as u32, col as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_an_xref() {
        assert_eq!(
            classify_target("see @fig-1 here", 0, 6),
            Target::Xref {
                id: "fig-1".to_string(),
                start: 4,
                end: 10
            }
        );
    }

    #[test]
    fn citation_wins_over_xref() {
        assert_eq!(
            classify_target("text [@smith2020] more", 0, 8),
            Target::Cite {
                key: "smith2020".to_string(),
                start: 6,
                end: 16
            }
        );
    }

    #[test]
    fn an_email_local_part_is_not_an_xref() {
        assert_eq!(classify_target("mail a@b.com now", 0, 7), Target::None);
    }

    #[test]
    fn classifies_an_include_path() {
        match classify_target("{{< include intro.tmd >}}", 0, 15) {
            Target::Include { path, .. } => assert_eq!(path, "intro.tmd"),
            other => panic!("expected include, got {other:?}"),
        }
    }

    #[test]
    fn classifies_a_frontmatter_key_and_its_nested_parent() {
        assert_eq!(
            classify_target("---\ntitle: Hi\n---\n", 1, 2),
            Target::FrontmatterKey {
                key: "title".to_string(),
                parent: None,
                start: 0,
                end: 5
            }
        );
        assert_eq!(
            classify_target("---\nexecute:\n  echo: true\n---\n", 2, 3),
            Target::FrontmatterKey {
                key: "echo".to_string(),
                parent: Some("execute".to_string()),
                start: 2,
                end: 6
            }
        );
    }

    #[test]
    fn a_frontmatter_value_is_not_a_key() {
        assert_eq!(classify_target("---\ntitle: Hi\n---\n", 1, 8), Target::None);
    }

    #[test]
    fn definition_site_finds_attribute_and_label_forms_but_not_a_reference() {
        assert_eq!(
            definition_site("# Title {#fig-1}\n\nsee @fig-1", "fig-1"),
            Some((0, 10))
        );
        assert_eq!(
            definition_site("#| label: fig-2\ncode", "fig-2"),
            Some((0, 10))
        );
        // Only a reference present: no definition here.
        assert_eq!(definition_site("see @fig-1 only", "fig-1"), None);
        assert_eq!(definition_site("nothing", "fig-1"), None);
        // A longer id must not match on a prefix.
        assert_eq!(definition_site("{#fig-10}", "fig-1"), None);
    }

    /// One fixture line for the cursor walk. `span` is the **inclusive** `[first, last]` cursor
    /// range over which `classify_target` must report `expect`; `None` means no cursor position
    /// on the line classifies as anything.
    struct Walk {
        what: &'static str,
        text: &'static str,
        line: usize,
        span: Option<(usize, usize)>,
        expect: Target,
    }

    /// Walk the cursor across **every** character of each fixture line, one past its end
    /// included, and assert the classification at each offset.
    ///
    /// The 2026-07-26 mutation round found that every *edge* of every classified span was
    /// unpinned: 31 boundary and cursor-arithmetic mutants survived across these classifiers
    /// because the tests above only ever put the cursor squarely inside a token. A span's edges
    /// are the whole contract here — one character before the `@`, the last character of a key,
    /// the closing `)` of an include — so this asserts them exhaustively rather than at the
    /// handful of offsets someone thought to write down.
    #[test]
    fn a_cursor_walk_pins_every_edge_of_every_classified_span() {
        let walks = vec![
            Walk {
                what: "xref mid-line",
                text: "see @fig-1 here",
                line: 0,
                span: Some((4, 10)),
                expect: Target::Xref {
                    id: "fig-1".to_string(),
                    start: 4,
                    end: 10,
                },
            },
            // A citation key may contain `:` and `.`; nothing above ever typed one, which is why
            // both of `is_cite_key_char`'s `||`s survived.
            Walk {
                what: "cite key containing `:` and `.`",
                text: "[@sec:intro.1] x",
                line: 0,
                span: Some((1, 13)),
                expect: Target::Cite {
                    key: "sec:intro.1".to_string(),
                    start: 1,
                    end: 13,
                },
            },
            // The key runs to the end of the line with no `]`, so the scan must stop at the line
            // end instead of reading past it.
            Walk {
                what: "unterminated cite at end of line",
                text: "see [@smith",
                line: 0,
                span: None,
                expect: Target::None,
            },
            Walk {
                what: "include path",
                text: "{{< include intro.tmd >}}",
                line: 0,
                span: Some((12, 21)),
                expect: Target::Include {
                    path: "intro.tmd".to_string(),
                    start: 12,
                    end: 21,
                },
            },
            // `include` must be followed by whitespace before a path begins.
            Walk {
                what: "include keyword with no separating space",
                text: "{{< include/x.tmd >}}",
                line: 0,
                span: None,
                expect: Target::None,
            },
            Walk {
                what: "top-level front-matter key",
                text: "---\ntitle: Hi\n---\n",
                line: 1,
                span: Some((0, 5)),
                expect: Target::FrontmatterKey {
                    key: "title".to_string(),
                    parent: None,
                    start: 0,
                    end: 5,
                },
            },
            // The span starts at the indent, not at the key, so both indent columns are inside it.
            Walk {
                what: "nested front-matter key under a recognized parent",
                text: "---\nexecute:\n  echo: true\n---\n",
                line: 2,
                span: Some((2, 6)),
                expect: Target::FrontmatterKey {
                    key: "echo".to_string(),
                    parent: Some("execute".to_string()),
                    start: 2,
                    end: 6,
                },
            },
            // Below the closing fence, a `key:` line is prose.
            Walk {
                what: "key-shaped line after the closing fence",
                text: "---\ntitle: Hi\n---\nother: x\n",
                line: 3,
                span: None,
                expect: Target::None,
            },
            Walk {
                what: "key-shaped line with no front matter at all",
                text: "other: x",
                line: 0,
                span: None,
                expect: Target::None,
            },
            // --- a well-formed construct pins a span; these pin the guards that REJECT, which a
            // weakened comparison turns into an acceptance of nonsense.
            // A trailing `[` is the last character, so the `[@` probe must not read past it.
            Walk {
                what: "cite followed by a dangling `[` at the line end",
                text: "see [@k] [",
                line: 0,
                span: Some((5, 7)),
                expect: Target::Cite {
                    key: "k".to_string(),
                    start: 5,
                    end: 7,
                },
            },
            // A stray `]` after an xref does not retroactively make it a citation: the `[` is
            // what distinguishes them, not the `@`.
            Walk {
                what: "xref with a stray closing bracket after it",
                text: "see @smith2020] more",
                line: 0,
                span: Some((4, 14)),
                expect: Target::Xref {
                    id: "smith2020".to_string(),
                    start: 4,
                    end: 14,
                },
            },
            // Empty key and empty id: a token needs at least one character to exist.
            Walk {
                what: "citation brackets with no key",
                text: "[@] x",
                line: 0,
                span: None,
                expect: Target::None,
            },
            Walk {
                what: "a bare `@` with no id after it",
                text: "see @ here",
                line: 0,
                span: None,
                expect: Target::None,
            },
            // --- the include scanner, at each point its cursor can reach the line end
            Walk {
                what: "shortcode opener and nothing else",
                text: "{{<",
                line: 0,
                span: None,
                expect: Target::None,
            },
            Walk {
                what: "include keyword ending the line",
                text: "{{< include",
                line: 0,
                span: None,
                expect: Target::None,
            },
            Walk {
                what: "include with trailing space and no path",
                text: "{{< include ",
                line: 0,
                span: None,
                expect: Target::None,
            },
            Walk {
                what: "include with no path before the closer",
                text: "{{< include >}}",
                line: 0,
                span: None,
                expect: Target::None,
            },
            // An unterminated shortcode still yields its path: the span ends at the line end.
            Walk {
                what: "include path running to the line end, unclosed",
                text: "{{< include intro.tmd",
                line: 0,
                span: Some((12, 21)),
                expect: Target::Include {
                    path: "intro.tmd".to_string(),
                    start: 12,
                    end: 21,
                },
            },
            // The opener is found by scanning the whole line, so it must still be found well
            // past the start.
            Walk {
                what: "include preceded by prose",
                text: "some text here {{< include a.tmd >}}",
                line: 0,
                span: Some((27, 32)),
                expect: Target::Include {
                    path: "a.tmd".to_string(),
                    start: 27,
                    end: 32,
                },
            },
            // All three characters of `{{<` are checked at their own offsets, and a `{` inside
            // the path is not a new opener.
            Walk {
                what: "opener with a wrong second character",
                text: "{x< include a",
                line: 0,
                span: None,
                expect: Target::None,
            },
            Walk {
                what: "opener with a wrong third character",
                text: "{{x include a",
                line: 0,
                span: None,
                expect: Target::None,
            },
            Walk {
                what: "a brace inside the include path",
                text: "{{< include a{b",
                line: 0,
                span: Some((12, 15)),
                expect: Target::Include {
                    path: "a{b".to_string(),
                    start: 12,
                    end: 15,
                },
            },
            // --- front-matter key lines whose scan reaches the end of the line
            Walk {
                what: "whitespace-only front-matter line",
                text: "---\ntitle: x\n   \n---\n",
                line: 2,
                span: None,
                expect: Target::None,
            },
            Walk {
                what: "front-matter word with no colon",
                text: "---\ntitle\n---\n",
                line: 1,
                span: None,
                expect: Target::None,
            },
            Walk {
                what: "front-matter line starting with the colon",
                text: "---\n: x\n---\n",
                line: 1,
                span: None,
                expect: Target::None,
            },
            // A sibling key at the same indent sits between the cursor and its parent: the
            // look-back must step over it, and both indents must be measured, not divided.
            Walk {
                what: "nested key with a sibling above it",
                text: "---\nexecute:\n  echo: true\n  ca: 1\n---\n",
                line: 3,
                span: Some((2, 4)),
                expect: Target::FrontmatterKey {
                    key: "ca".to_string(),
                    parent: Some("execute".to_string()),
                    start: 2,
                    end: 4,
                },
            },
            // Indented under a key that is not a recognized nested parent: having a colon is not
            // enough to be one.
            Walk {
                what: "nested key under an unrecognized parent",
                text: "---\ntitle: x\n  ca: 1\n---\n",
                line: 2,
                span: Some((2, 4)),
                expect: Target::FrontmatterKey {
                    key: "ca".to_string(),
                    parent: None,
                    start: 2,
                    end: 4,
                },
            },
        ];

        for w in &walks {
            let len = w.text.split('\n').nth(w.line).unwrap_or("").chars().count();
            for ch in 0..=len + 1 {
                let got = classify_target(w.text, w.line, ch);
                let inside = w.span.is_some_and(|(a, b)| ch >= a && ch <= b);
                let want = if inside { &w.expect } else { &Target::None };
                assert_eq!(
                    &got, want,
                    "{}: cursor at character {ch} of {:?} (line {})",
                    w.what, w.text, w.line
                );
            }
        }
    }

    /// The anchor scanner walks backwards from a match, so its edge is the *start of the text*
    /// rather than a span boundary. Every fixture here is one the mutation round showed nothing
    /// reached: an id at offset 0, an id preceded only by whitespace, and `label:` with no space.
    #[test]
    fn the_anchor_scanner_is_pinned_at_the_start_of_the_text() {
        // At offset 0 there is no sigil to inspect, and looking for one must not read backwards.
        assert_eq!(definition_site("fig-1 is here", "fig-1"), None);
        // Preceded only by whitespace: the `label:` look-back walks to offset 0 and stops.
        assert_eq!(definition_site("  fig-1", "fig-1"), None);
        assert_eq!(definition_site("{#fig-1}", "fig-1"), Some((0, 2)));
        assert_eq!(definition_site("#| label: fig-1", "fig-1"), Some((0, 10)));
    }

    /// `{{< include >}}` is navigable and every *other* shortcode is not: accepting anything
    /// else makes the first argument of `{{< video … >}}` (or any future shortcode) look like
    /// a document to open.
    #[test]
    fn only_the_include_shortcode_is_navigable() {
        match classify_target("{{< include part.tmd >}}", 0, 14) {
            Target::Include { path, .. } => assert_eq!(path, "part.tmd"),
            other => panic!("expected `include` to be a navigable include, got {other:?}"),
        }
        assert_eq!(
            classify_target("{{< video clip.mp4 >}}", 0, 12),
            Target::None
        );
    }

    /// The client counts lines the way CommonMark does, so a lone `\r` in the buffer starts a
    /// new line for it and used to start nothing for us: the position it sent read past the
    /// end of a `\n`-split shorter by every CR in the file, and go-to-definition and hover
    /// answered `None` on a token plainly under the cursor.
    #[test]
    fn a_lone_cr_does_not_hide_the_token_under_the_cursor() {
        let cr = "para one\rSee [@smith2020] here.";
        assert_eq!(
            classify_target(cr, 1, 6),
            classify_target(&cr.replace('\r', "\n"), 1, 6),
            "the terminator must not change what is under the cursor"
        );
        match classify_target(cr, 1, 6) {
            Target::Cite { key, .. } => assert_eq!(key, "smith2020"),
            other => panic!("expected the citation on the line after the CR, got {other:?}"),
        }
    }
}
