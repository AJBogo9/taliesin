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

/// Front-matter parents whose immediate children have their own vocabulary (mirrors
/// `lsp_complete`).
const NESTED_PARENTS: &[&str] = &[
    "execute",
    "listing",
    "about",
    "hero",
    "prose-lint",
    "theorems",
];

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
    /// An enclosing `$…$` / `$$…$$` span. Unlike every other target this one can cross
    /// lines (display math routinely does), so it carries absolute positions rather than
    /// the line-relative `start`/`end` the single-line targets use.
    Math {
        latex: String,
        display: bool,
        start_line: usize,
        start_char: usize,
        end_line: usize,
        end_char: usize,
    },
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
fn is_xref_id_char(c: char) -> bool {
    is_word(c) || c == '-'
}
fn is_cite_key_char(c: char) -> bool {
    is_word(c) || c == ':' || c == '.' || c == '-'
}
fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}
/// Inclusive of both ends, so a cursor just past the last char still hovers the token
/// (matches the editor's word-range behaviour).
fn covers(s: usize, e: usize, ch: usize) -> bool {
    ch >= s && ch <= e
}

/// One `$…$` / `$$…$$` span. `start`/`end` are char offsets into the document and cover the
/// delimiters; `latex` is what sits between them.
pub(crate) struct MathSpan {
    pub(crate) latex: String,
    pub(crate) display: bool,
    pub(crate) start: usize,
    pub(crate) end: usize,
    /// `false` for a span whose closing delimiter has not been typed yet — which is the
    /// case completion cares about and hover deliberately ignores.
    pub(crate) closed: bool,
}

/// Every math span in `text`, in document order.
///
/// This is the single owner of Taliesin's `$` delimiter rules, so completion ("am I inside
/// math?") and hover ("which expression am I inside?") cannot drift apart: a `\` escapes the
/// next character, a fenced block is code and not math, an inline `$…$` is abandoned at a
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
    let close = |spans: &mut Vec<MathSpan>, open: usize, delim: usize, at: usize| {
        spans.push(MathSpan {
            latex: chars[open + delim..at].iter().collect(),
            display: delim == 2,
            start: open,
            end: at + delim,
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
                            Some(open) => close(&mut spans, open, 2, j),
                            None => display = Some(j),
                        }
                        j += 2;
                    }
                    '$' => {
                        match inline.take() {
                            // Closing never checks the guard; only an OPEN needs a non-space.
                            Some(open) => close(&mut spans, open, 1, j),
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
    for (open, delim) in [(display, 2usize), (inline, 1usize)].into_iter() {
        if let Some(open) = open {
            spans.push(MathSpan {
                latex: chars[(open + delim).min(n)..].iter().collect(),
                display: delim == 2,
                start: open,
                end: n,
                closed: false,
            });
        }
    }
    spans.sort_by_key(|s| s.start);
    spans
}

/// The innermost CLOSED math span covering `offset`, delimiters included.
///
/// Unclosed spans are skipped on purpose: half-typed math has no complete expression to
/// preview, and offering one would render whatever fragment exists so far.
fn enclosing_math(text: &str, offset: usize) -> Option<MathSpan> {
    scan_math(text)
        .into_iter()
        .filter(|s| s.closed && offset >= s.start && offset <= s.end)
        .min_by_key(|s| s.end - s.start)
}

/// Char offset of 0-based (`line`, `character`), clamped to the document.
fn line_char_to_offset(text: &str, line: usize, character: usize) -> usize {
    let mut offset = 0;
    for (idx, l) in text.split('\n').enumerate() {
        let len = l.chars().count();
        if idx == line {
            return offset + character.min(len);
        }
        offset += len + 1; // + the newline
    }
    text.chars().count()
}

/// The inverse of [`line_char_to_offset`].
fn offset_to_line_char(text: &str, offset: usize) -> (usize, usize) {
    let mut seen = 0;
    for (idx, l) in text.split('\n').enumerate() {
        let len = l.chars().count();
        if offset <= seen + len {
            return (idx, offset - seen);
        }
        seen += len + 1;
    }
    (0, 0)
}

/// Classify the token at 0-based (`line`, `character`). Citation `[@k]` wins over xref
/// `@k`; a front-matter key is recognized only inside the `---` body, on the key token.
pub(crate) fn classify_target(text: &str, line: usize, character: usize) -> Target {
    let lines: Vec<&str> = text.split('\n').collect();
    let lt: Vec<char> = lines.get(line).copied().unwrap_or("").chars().collect();
    let n = lt.len();

    // Citation `[@key]` first (its `@` must not be read as an xref).
    let mut i = 0;
    while i + 1 < n {
        if lt[i] == '[' && lt[i + 1] == '@' {
            let key_start = i + 2;
            let mut j = key_start;
            while j < n && is_cite_key_char(lt[j]) {
                j += 1;
            }
            if j > key_start && j < n && lt[j] == ']' {
                let (start, end) = (i + 1, j); // `@` .. `]`
                if covers(start, end, character) {
                    return Target::Cite {
                        key: lt[key_start..j].iter().collect(),
                        start,
                        end,
                    };
                }
            }
        }
        i += 1;
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

    // Include / embed shortcode path.
    if let Some(t) = classify_include(&lt, character) {
        return t;
    }

    // Front-matter key.
    if let Some(t) = classify_frontmatter_key(&lines, line, character) {
        return t;
    }

    // Math last: every target above is a token that can legitimately appear inside `$…$`
    // (an `@` or a `[@key]` in a label, say), and the specific answer beats the general one.
    if let Some(span) = enclosing_math(text, line_char_to_offset(text, line, character)) {
        let (start_line, start_char) = offset_to_line_char(text, span.start);
        let (end_line, end_char) = offset_to_line_char(text, span.end);
        return Target::Math {
            latex: span.latex,
            display: span.display,
            start_line,
            start_char,
            end_line,
            end_char,
        };
    }

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
            if kw == "include" || kw == "embed" {
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
            return if has_colon && NESTED_PARENTS.contains(&key.as_str()) {
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

/// Whether the id token starting at char offset `i` in `chars` sits in a rename SITE — a
/// cross-reference *reference* (`@id`, the `@` a real xref sigil: not preceded by a word char,
/// `@`, or `[`, so a `[@key]` citation is excluded) or a *definition* (`#id` attribute, or
/// `label: id` cell label). This is `definition_site`'s `prefix_ok` test plus the reference
/// form, factored out so the rename set can't disagree with go-to-definition on what an anchor
/// is. `i` is assumed to start a bounded xref-id token (the caller checks the trailing boundary).
fn is_anchor_site(chars: &[char], i: usize) -> bool {
    if i == 0 {
        return false; // an anchor always carries a `@`/`#`/`label:` sigil before it
    }
    match chars[i - 1] {
        '@' => {
            i == 1 || {
                let p = chars[i - 2];
                !is_word(p) && p != '@' && p != '['
            }
        }
        // A `#` is our sigil in exactly two places: a `{#id}` attribute (the definition), and
        // a link destination that is a BARE fragment, `](#id)`, which points into THIS
        // document and so must follow a rename. Accepting every `#` meant renaming a section
        // also rewrote `[x](https://example.com/p.html#id)` — a fragment on someone else's
        // page, silently retargeted to a heading that only exists here.
        '#' => {
            i >= 2
                && (chars[i - 2] == '{' || (chars[i - 2] == '(' && i >= 3 && chars[i - 3] == ']'))
        }
        c if is_ws(c) => {
            let mut j = i;
            while j > 0 && is_ws(chars[j - 1]) {
                j -= 1;
            }
            j >= 6 && chars[j - 6..j].iter().collect::<String>() == "label:"
        }
        _ => false,
    }
}

/// Every site in `text` where cross-reference anchor `id` appears as a whole xref-id token in a
/// rename site — its definition (`{#id}` / `#| label: id`) and all `@id` references — each a
/// 0-based `(line, start_col, end_col)` covering **exactly the id** (never the `@`/`#` sigil).
/// Why `name` cannot replace cross-reference anchor `old`, or `None` when it can. Called
/// before a rename builds any edit, because `rename` is the ONE sanctioned write path back
/// into source (the preview never writes) — so an unusable name does not fail loudly, it
/// corrupts every site at once. Two conditions, and neither was checked:
///
/// * **Every character must be an xref-id char.** The scanners above stop at anything else,
///   so `F2` -> `my section` wrote `{#my section}` — an anchor of `my` with a stray word
///   after it — and rewrote every reference to match. A newline split the heading in two.
/// * **The kind prefix must survive.** [`anchor_at`] only recognises an id whose prefix is a
///   known xref kind, so renaming `sec-a` to `intro` strands every `@intro` as prose.
///
/// The prefix in the message is read off `old` rather than from a copy of the kind table,
/// which cannot go stale as kinds are added.
pub(crate) fn anchor_name_error(old: &str, name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("a cross-reference anchor cannot be empty".to_string());
    }
    if let Some(bad) = name.chars().find(|c| !is_xref_id_char(*c)) {
        return Some(format!(
            "{name:?} is not a cross-reference anchor: {bad:?} is not allowed \
             — use letters, digits, `-` and `_`"
        ));
    }
    if !taliesin_core::cite::is_xref_anchor(name) {
        let kind = old.split_once('-').map(|(p, _)| p).unwrap_or("sec");
        return Some(format!(
            "{name:?} has no cross-reference kind prefix, so `@{name}` would stop \
             resolving — keep the `{kind}-` prefix"
        ));
    }
    None
}

/// The set a rename rewrites; includes the definition so renaming keeps references resolving.
pub(crate) fn anchor_occurrences(text: &str, id: &str) -> Vec<(u32, u32, u32)> {
    let chars: Vec<char> = text.chars().collect();
    let idc: Vec<char> = id.chars().collect();
    let (n, m) = (chars.len(), idc.len());
    let mut out = Vec::new();
    if m == 0 {
        return out;
    }
    let mut i = 0;
    while i + m <= n {
        let bounded = i + m >= n || !is_xref_id_char(chars[i + m]);
        if bounded && chars[i..i + m] == idc[..] && is_anchor_site(&chars, i) {
            let (line, col) = offset_to_line_col(&chars, i);
            out.push((line, col, col + m as u32));
            i += m;
        } else {
            i += 1;
        }
    }
    out
}

/// The nesting chain at `(line, col)`, innermost first, as
/// `(start_line, start_char, end_line, end_char)` in scalar offsets.
///
/// Levels are built inside-out — word, enclosing inline construct, paragraph, then every
/// enclosing fold (div, then section) — and each is kept only if it strictly *grows* the one
/// below it, so the containment invariant LSP requires holds by construction rather than by
/// hope. A level that cannot be determined is skipped, never emitted as a zero-width guess.
pub(crate) fn selection_chain(text: &str, line: usize, col: usize) -> Vec<(u32, u32, u32, u32)> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(src) = lines.get(line).copied() else {
        return Vec::new();
    };
    let l = line as u32;
    let mut chain: Vec<(u32, u32, u32, u32)> = Vec::new();

    // 1. The word under the cursor.
    if let Some((s, e)) = word_span(src, col) {
        chain.push((l, s as u32, l, e as u32));
    }
    // 2. The inline construct enclosing it, when there is one. Math is classified against the
    //    whole document because a `$$…$$` span crosses lines; the rest are line-local.
    match classify_target(text, line, col) {
        Target::Xref { start, end, .. }
        | Target::Cite { start, end, .. }
        | Target::Include { start, end, .. } => chain.push((l, start as u32, l, end as u32)),
        Target::Math {
            start_line,
            start_char,
            end_line,
            end_char,
            ..
        } => chain.push((
            start_line as u32,
            start_char as u32,
            end_line as u32,
            end_char as u32,
        )),
        _ => {}
    }
    // 3. The paragraph, then 4-5. every enclosing fold, innermost first.
    let full = |s: u32, e: u32| (s, 0, e, lines[e as usize].chars().count() as u32);
    if let Some((s, e)) = paragraph_span(&lines, line) {
        chain.push(full(s, e));
    }
    let mut folds: Vec<(u32, u32)> = crate::lsp_fold::folding_ranges(text)
        .into_iter()
        .filter(|r| r.start_line <= l && r.end_line >= l && (r.end_line as usize) < lines.len())
        .map(|r| (r.start_line, r.end_line))
        .collect();
    // Innermost fold first: the one that starts latest and ends earliest encloses least.
    folds.sort_by_key(|&(s, e)| (std::cmp::Reverse(s), e));
    for (s, e) in folds {
        chain.push(full(s, e));
    }

    // Keep only levels that strictly grow.
    let mut kept: Vec<(u32, u32, u32, u32)> = Vec::new();
    for cand in chain {
        match kept.last() {
            None => kept.push(cand),
            Some(&prev) => {
                let starts_before_or_at = (cand.0, cand.1) <= (prev.0, prev.1);
                let ends_after_or_at = (cand.2, cand.3) >= (prev.2, prev.3);
                let grew =
                    (cand.0, cand.1) < (prev.0, prev.1) || (cand.2, cand.3) > (prev.2, prev.3);
                if starts_before_or_at && ends_after_or_at && grew {
                    kept.push(cand);
                }
            }
        }
    }
    kept
}

/// `[start, end)` of the word at `col`, in scalar offsets, or `None` in whitespace.
fn word_span(line: &str, col: usize) -> Option<(usize, usize)> {
    let cs: Vec<char> = line.chars().collect();
    if col >= cs.len() || !is_word(cs[col]) {
        return None;
    }
    let mut s = col;
    while s > 0 && is_word(cs[s - 1]) {
        s -= 1;
    }
    let mut e = col;
    while e < cs.len() && is_word(cs[e]) {
        e += 1;
    }
    Some((s, e))
}

/// The blank-line-delimited paragraph containing `line`, as inclusive line numbers.
fn paragraph_span(lines: &[&str], line: usize) -> Option<(u32, u32)> {
    if lines.get(line)?.trim().is_empty() {
        return None;
    }
    let mut s = line;
    while s > 0 && !lines[s - 1].trim().is_empty() {
        s -= 1;
    }
    let mut e = line;
    while e + 1 < lines.len() && !lines[e + 1].trim().is_empty() {
        e += 1;
    }
    Some((s as u32, e as u32))
}

/// Every site of the cross-reference anchor under the cursor, as `(line, start_col, end_col,
/// is_definition)` in scalar offsets, in reading order. Empty when the cursor is not on an
/// anchor — highlighting every word would be worse than highlighting none.
///
/// This is [`anchor_occurrences`] plus the one thing document highlight adds: which site is
/// the *definition*, so an editor can paint it as a write and the rest as reads. It
/// deliberately does not rescan; a second scanner could disagree with rename about what an
/// anchor is, and rename is the one sanctioned write path back into source.
pub(crate) fn anchor_highlights(text: &str, line: usize, col: usize) -> Vec<(u32, u32, u32, bool)> {
    let Some((id, _, _)) = anchor_at(text, line, col) else {
        return Vec::new();
    };
    let def = definition_site(text, &id);
    anchor_occurrences(text, &id)
        .into_iter()
        .map(|(l, s, e)| (l, s, e, def == Some((l, s))))
        .collect()
}

/// The cross-reference anchor id under the cursor and its 0-based `[start, end)` char span on
/// `line`, whether the cursor is on an `@id` *reference* or a `{#id}` / `label: id`
/// *definition*. The span covers the id only (never the `@`/`#`), so a rename's placeholder and
/// its edits agree. `None` when the token is not a known xref anchor (a plain heading id, a
/// citation key, prose). Underlies `prepareRename` and `rename`.
pub(crate) fn anchor_at(
    text: &str,
    line: usize,
    character: usize,
) -> Option<(String, usize, usize)> {
    // Reference form: reuse the shared classifier (it also covers a cursor sitting on the `@`).
    if let Target::Xref { id, start, end } = classify_target(text, line, character)
        && taliesin_core::cite::is_xref_anchor(&id)
    {
        return Some((id, start + 1, end)); // drop the leading `@`
    }
    // Definition form: the maximal xref-id run under the cursor whose sigil marks a `#`/`label:`.
    let lines: Vec<&str> = text.split('\n').collect();
    let lt: Vec<char> = lines.get(line).copied().unwrap_or("").chars().collect();
    let n = lt.len();
    let mut s = character.min(n);
    while s > 0 && is_xref_id_char(lt[s - 1]) {
        s -= 1;
    }
    let mut e = s;
    while e < n && is_xref_id_char(lt[e]) {
        e += 1;
    }
    if e == s || !covers(s, e, character) || !is_anchor_site(&lt, s) {
        return None;
    }
    let id: String = lt[s..e].iter().collect();
    taliesin_core::cite::is_xref_anchor(&id).then_some((id, s, e))
}

/// The char offset of the BibTeX entry header `@type{key,` for `key` in `chars`, or None
/// when absent. Shared by `bib_entry_site` (offset → line/col) and `bib_entry_text`
/// (offset → brace-balanced entry text) so the two can't drift.
fn bib_entry_offset(chars: &[char], keyc: &[char]) -> Option<usize> {
    let (n, m) = (chars.len(), keyc.len());
    if m == 0 {
        return None;
    }
    let mut i = 0;
    while i < n {
        if chars[i] == '@' {
            let mut j = i + 1;
            let type_start = j;
            while j < n && is_word(chars[j]) {
                j += 1;
            }
            if j > type_start {
                while j < n && is_ws(chars[j]) {
                    j += 1;
                }
                if j < n && chars[j] == '{' {
                    j += 1;
                    while j < n && is_ws(chars[j]) {
                        j += 1;
                    }
                    if j + m <= n && chars[j..j + m] == *keyc {
                        let mut k = j + m;
                        while k < n && is_ws(chars[k]) {
                            k += 1;
                        }
                        if k < n && chars[k] == ',' {
                            return Some(i);
                        }
                    }
                }
            }
        }
        i += 1;
    }
    None
}

/// The 0-based (line, col) of the BibTeX entry header `@type{key,` for `key` in `bib`,
/// or None when absent.
pub(crate) fn bib_entry_site(bib: &str, key: &str) -> Option<(u32, u32)> {
    let chars: Vec<char> = bib.chars().collect();
    let keyc: Vec<char> = key.chars().collect();
    let i = bib_entry_offset(&chars, &keyc)?;
    Some(offset_to_line_col(&chars, i))
}

/// The raw BibTeX entry (`@type{key, … }`) for `key`, brace-balanced so a `{…}` inside a
/// field value doesn't cut it short; None when the key is absent. A Rust port of the
/// companion's `bibEntryFor`, used by the LSP hover to show the citation source.
pub(crate) fn bib_entry_text(bib: &str, key: &str) -> Option<String> {
    let chars: Vec<char> = bib.chars().collect();
    let keyc: Vec<char> = key.chars().collect();
    let start = bib_entry_offset(&chars, &keyc)?;
    let brace_open = (start..chars.len()).find(|&i| chars[i] == '{')?;
    let mut depth = 0usize;
    for i in brace_open..chars.len() {
        match chars[i] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(
                        chars[start..=i]
                            .iter()
                            .collect::<String>()
                            .trim()
                            .to_string(),
                    );
                }
            }
            _ => {}
        }
    }
    // Unbalanced .bib: give back what we have (mirrors `bibEntryFor`).
    Some(chars[start..].iter().collect::<String>().trim().to_string())
}

fn strip_quotes(s: &str) -> String {
    let s = s.strip_prefix(['"', '\'']).unwrap_or(s);
    let s = s.strip_suffix(['"', '\'']).unwrap_or(s);
    s.to_string()
}

/// The front-matter `bibliography:` paths (scalar or YAML list), raw as written.
pub(crate) fn frontmatter_bib_paths(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.first().map(|l| l.trim()) != Some("---") {
        return vec![];
    }
    let mut out = vec![];
    let mut i = 1;
    while i < lines.len() {
        let t = lines[i].trim();
        if t == "---" || t == "..." {
            break;
        }
        if let Some(rest) = lines[i].strip_prefix("bibliography:") {
            let val = rest.trim();
            if !val.is_empty() {
                out.push(strip_quotes(val));
            } else {
                for l in &lines[i + 1..] {
                    let t2 = l.trim();
                    if t2 == "---" || t2 == "..." {
                        break;
                    }
                    match l.trim_start().strip_prefix('-') {
                        Some(item) if !item.trim().is_empty() => {
                            out.push(strip_quotes(item.trim()))
                        }
                        _ => break,
                    }
                }
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_inline_math_around_the_cursor() {
        match classify_target("see $\\alpha + \\beta$ here", 0, 8) {
            Target::Math {
                latex,
                display,
                start_line,
                start_char,
                end_line,
                end_char,
            } => {
                assert_eq!(latex, "\\alpha + \\beta");
                assert!(!display, "single `$` is inline");
                // The range covers the delimiters, so the editor highlights the whole span.
                assert_eq!((start_line, start_char), (0, 4));
                assert_eq!((end_line, end_char), (0, 20));
            }
            other => panic!("expected math, got {other:?}"),
        }
    }

    #[test]
    fn display_math_spans_lines_and_reports_a_multi_line_range() {
        let text = "before\n\n$$\n\\frac{a}{b}\n$$\n\nafter\n";
        match classify_target(text, 3, 2) {
            Target::Math {
                latex,
                display,
                start_line,
                end_line,
                ..
            } => {
                assert_eq!(latex, "\n\\frac{a}{b}\n");
                assert!(display, "`$$` is display math");
                assert_eq!(
                    (start_line, end_line),
                    (2, 4),
                    "range covers both delimiters"
                );
            }
            other => panic!("expected display math, got {other:?}"),
        }
    }

    // A `$` in a code cell is a shell variable or a regex anchor, not math. Hovering it with
    // a rendered preview would assert the renderer does something to it that it never does.
    #[test]
    fn math_inside_a_code_fence_is_code_not_math() {
        let text = "```python\ns = \"$x$\"\n```\n";
        assert_eq!(classify_target(text, 1, 6), Target::None);
    }

    // Half-typed math has no complete expression to preview.
    #[test]
    fn unclosed_math_is_not_hoverable() {
        assert_eq!(classify_target("half $\\alpha", 0, 8), Target::None);
    }

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

    #[test]
    fn anchor_at_finds_a_reference_and_a_definition() {
        // `![p](i.png){#fig-scree}` — `#` at 12, id `fig-scree` at [13, 22); line 2 has the ref.
        let text = "![p](i.png){#fig-scree}\n\nSee @fig-scree.\n";
        // Cursor inside the `@fig-scree` reference (line 2, char 6): id-only span [5, 14).
        assert_eq!(
            anchor_at(text, 2, 6),
            Some(("fig-scree".to_string(), 5, 14))
        );
        // Cursor inside the `{#fig-scree}` definition (line 0, char 14): id-only span [13, 22).
        assert_eq!(
            anchor_at(text, 0, 14),
            Some(("fig-scree".to_string(), 13, 22))
        );
        // A `#| label:` cell-label definition is an anchor site too.
        assert_eq!(
            anchor_at("#| label: fig-1\n", 0, 12),
            Some(("fig-1".to_string(), 10, 15))
        );
    }

    #[test]
    fn anchor_at_rejects_a_cite_key_and_a_plain_heading_id() {
        // A citation key is not an xref anchor (and its `@` is not a real xref sigil).
        assert_eq!(anchor_at("[@smith2020] and more", 0, 4), None);
        // A plain custom heading id has no xref prefix, so it is not renameable here.
        assert_eq!(anchor_at("## Intro {#intro}\n", 0, 13), None);
        // Prose is nothing.
        assert_eq!(anchor_at("just some words", 0, 5), None);
    }

    #[test]
    fn anchor_occurrences_covers_definition_and_references_only() {
        // Definition + two references + one citation (which must be excluded).
        let text =
            "![p](i.png){#fig-scree}\n\nSee @fig-scree, again @fig-scree, cite [@fig-scree].\n";
        let sites = anchor_occurrences(text, "fig-scree");
        assert_eq!(
            sites,
            vec![(0, 13, 22), (2, 5, 14), (2, 23, 32)],
            "expected the definition and both `@` references, never the `[@…]` citation"
        );
    }

    #[test]
    fn anchor_highlights_mark_the_definition_apart_from_its_references() {
        let text = "# R {#sec-r}\n\nSee @sec-r and again @sec-r.\n";
        // Cursor inside the SECOND reference: the answer must not depend on which site the
        // cursor is on.
        let hits = anchor_highlights(text, 2, 23);
        assert_eq!(
            hits,
            vec![(0, 6, 11, true), (2, 5, 10, false), (2, 22, 27, false)],
            "one definition (write) and two references (read)"
        );
        // From the definition itself, the same set.
        assert_eq!(anchor_highlights(text, 0, 7), hits);
    }

    #[test]
    fn anchor_highlights_do_not_match_a_longer_id_that_starts_the_same() {
        let text = "See @sec-r and @sec-results.\n";
        let hits = anchor_highlights(text, 0, 6);
        assert_eq!(
            hits.len(),
            1,
            "`sec-r` must not match inside `sec-results`: {hits:?}"
        );
    }

    // Guards are a separate axis from span arithmetic: a cursor walk over well-formed text
    // never reaches them. A cursor in prose, past the end of a line, on a citation key, and
    // in three half-typed buffers.
    #[test]
    fn anchor_highlights_are_empty_when_the_cursor_is_not_on_an_anchor() {
        for (what, text, line, col) in [
            ("prose", "just prose here\n", 0, 4),
            ("past the end of the line", "See @sec-r.\n", 0, 99),
            ("past the last line", "See @sec-r.\n", 9, 0),
            ("a citation key", "cite [@sec-r] here\n", 0, 8),
            ("a heading id with no xref kind", "# T {#plain}\n", 0, 7),
        ] {
            let hits = anchor_highlights(text, line, col);
            assert!(
                hits.is_empty(),
                "{what}: expected no highlights, got {hits:?}"
            );
        }
    }

    // Guards are a separate axis from span arithmetic: a cursor walk over well-formed text
    // never reaches them. A half-typed buffer is the normal case for a provider that fires as
    // the cursor moves, so each of these must return rather than panic.
    #[test]
    fn anchor_highlights_on_a_malformed_buffer_do_not_panic() {
        for (text, line, col) in [
            ("::: {.callout}\n@sec-r", 1, 3),
            ("$$\n\\frac{1}{\n@sec-r", 2, 3),
            ("---\ntitle: x\n\n@sec-r", 3, 3),
            ("@ @- @sec-\n", 0, 6),
            ("", 0, 0),
        ] {
            let _ = anchor_highlights(text, line, col);
        }
    }

    #[test]
    fn each_selection_level_strictly_contains_the_previous() {
        let text = "# S {#sec-s}\n\n::: {.callout}\nSee @fig-x here.\n:::\n";
        let chain = selection_chain(text, 3, 6); // inside `@fig-x`
        assert!(
            chain.len() >= 3,
            "expected word, construct, paragraph, div, section: {chain:?}"
        );
        for w in chain.windows(2) {
            let (inner, outer) = (w[0], w[1]);
            let starts_at_or_before = (outer.0, outer.1) <= (inner.0, inner.1);
            let ends_at_or_after = (outer.2, outer.3) >= (inner.2, inner.3);
            assert!(
                starts_at_or_before && ends_at_or_after,
                "level {outer:?} must contain {inner:?}"
            );
        }
    }

    // The chain starts at the WORD, not at the line: an editor's expand-selection is useless
    // if the first press already swallows the sentence.
    #[test]
    fn the_innermost_level_is_the_word_under_the_cursor() {
        let text = "# S {#sec-s}\n\nSee @fig-x here.\n";
        let chain = selection_chain(text, 2, 12); // inside `here`
        assert_eq!(
            chain.first().copied(),
            Some((2, 11, 2, 15)),
            "expected the word `here` first: {chain:?}"
        );
    }

    // The case the containment filter exists for, which a tidy document never reaches: with no
    // blank line around the div, the blank-line-delimited PARAGRAPH runs past the div's
    // closing fence, so the fold that comes after it in the chain is SMALLER. Emitting both
    // would hand the client word ⊂ paragraph ⊃ div, which is not a nesting chain at all.
    #[test]
    fn a_level_that_would_shrink_the_selection_is_dropped_rather_than_emitted() {
        let text = "::: {.callout}\ninside\n:::\nmore text\n";
        let chain = selection_chain(text, 1, 2); // inside the word `inside`
        assert!(!chain.is_empty(), "expected a chain: {chain:?}");
        for w in chain.windows(2) {
            let (inner, outer) = (w[0], w[1]);
            assert!(
                (outer.0, outer.1) <= (inner.0, inner.1)
                    && (outer.2, outer.3) >= (inner.2, inner.3),
                "level {outer:?} must contain {inner:?}, chain was {chain:?}"
            );
        }
    }

    #[test]
    fn a_position_in_empty_space_yields_a_chain_without_panicking() {
        let text = "\n\n\n";
        let _ = selection_chain(text, 1, 0);
    }

    #[test]
    fn a_position_past_the_end_of_the_document_is_an_empty_chain() {
        let text = "# S\n\ntext\n";
        assert!(selection_chain(text, 99, 0).is_empty());
    }

    #[test]
    fn bib_entry_site_finds_the_entry_header() {
        assert_eq!(
            bib_entry_site("@article{smith2020,\n  title={x}\n}", "smith2020"),
            Some((0, 0))
        );
        assert_eq!(
            bib_entry_site("% comment\n@book{key1 ,\n}", "key1"),
            Some((1, 0))
        );
        assert_eq!(bib_entry_site("@article{other,}", "smith2020"), None);
    }

    #[test]
    fn bib_entry_text_is_brace_balanced() {
        // A `{…}` inside a field value must not cut the entry short.
        assert_eq!(
            bib_entry_text(
                "@article{smith2020,\n  title = {A {Deep} Study}\n}\ntrailing",
                "smith2020"
            )
            .as_deref(),
            Some("@article{smith2020,\n  title = {A {Deep} Study}\n}")
        );
        assert_eq!(bib_entry_text("@book{other,\n}", "smith2020"), None);
        // Unbalanced .bib: return what we have rather than nothing.
        assert_eq!(
            bib_entry_text("@misc{k1,\n  note = {open", "k1").as_deref(),
            Some("@misc{k1,\n  note = {open")
        );
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

    /// The anchor scanners walk backwards from a match, so their edge is the *start of the text*
    /// rather than a span boundary. Every fixture here is one the mutation round showed nothing
    /// reached: an id at offset 0, an id preceded only by whitespace, `label:` with no space, and
    /// a text that is nothing but the id.
    #[test]
    fn anchor_scanners_are_pinned_at_the_start_of_the_text() {
        // At offset 0 there is no sigil to inspect, and looking for one must not read backwards.
        assert_eq!(definition_site("fig-1 is here", "fig-1"), None);
        // Preceded only by whitespace: the `label:` look-back walks to offset 0 and stops.
        assert_eq!(definition_site("  fig-1", "fig-1"), None);
        assert_eq!(definition_site("{#fig-1}", "fig-1"), Some((0, 2)));
        assert_eq!(definition_site("#| label: fig-1", "fig-1"), Some((0, 10)));

        // `label:` must be followed by whitespace to introduce a label.
        assert!(anchor_occurrences("label:fig-1", "fig-1").is_empty());
        assert!(anchor_occurrences("  fig-1", "fig-1").is_empty());
        // The whole text is the id: there is no trailing character to test for a boundary.
        assert!(anchor_occurrences("fig-1", "fig-1").is_empty());
        assert_eq!(
            anchor_occurrences("{#fig-1} @fig-1 @fig-1", "fig-1"),
            vec![(0, 2, 7), (0, 10, 15), (0, 17, 22)]
        );
    }

    /// `{{< embed >}}` is navigable and every *other* shortcode is not.
    ///
    /// The include scan accepts exactly two keywords, and only `include` was ever typed here, so
    /// the `embed` arm was free to invert: dropping `embed` costs go-to-definition on an embedded
    /// deck, and accepting everything else makes the first argument of `{{< video … >}}` (or any
    /// future shortcode) look like a document to open.
    #[test]
    fn only_include_and_embed_shortcodes_are_navigable() {
        match classify_target("{{< embed deck.tmd >}}", 0, 12) {
            Target::Include { path, .. } => assert_eq!(path, "deck.tmd"),
            other => panic!("expected `embed` to be a navigable include, got {other:?}"),
        }
        assert_eq!(
            classify_target("{{< video clip.mp4 >}}", 0, 12),
            Target::None
        );
    }

    /// `anchor_at`'s definition form needs *both* the cursor on the token and a sigil before it.
    ///
    /// Its guard is three `||`ed rejections, and every fixture above puts the cursor squarely on a
    /// real anchor, so two of the three could be `&&`ed away unseen: one lets a cursor that is
    /// nowhere near the token rename it anyway, the other makes any bare id in prose a rename site.
    #[test]
    fn anchor_at_needs_the_cursor_on_the_token_and_a_sigil_before_it() {
        // One past the last char still hovers the token (`covers` is inclusive of both ends)…
        assert_eq!(anchor_at("@fig-1", 0, 6), Some(("fig-1".to_string(), 1, 6)));
        // …two past does not, even though the backwards walk still finds the id.
        assert_eq!(anchor_at("@fig-1", 0, 7), None);
        // A bare id with no `@` / `#` / `label:` sigil is prose, not a rename site.
        assert_eq!(anchor_at("fig-1 is here", 0, 2), None);
    }

    fn bib_offset(bib: &str, key: &str) -> Option<usize> {
        let chars: Vec<char> = bib.chars().collect();
        let keyc: Vec<char> = key.chars().collect();
        bib_entry_offset(&chars, &keyc)
    }

    /// The `@type{key,` scan: whitespace tolerance, and stopping at the end of a truncated `.bib`.
    ///
    /// The two tests above reach this scanner only through canonical, complete entries, which
    /// leaves 17 mutants alive: every one of its four bounds checks can be widened past the end of
    /// the buffer, and both of its whitespace-skipping loops can be made no-ops, without a fixture
    /// noticing. Both are reachable in practice — `.bib` files are written by hand and by export
    /// tools, and this scans one straight off disk on every hover and every go-to-definition of a
    /// `[@key]`, including while the author has that file open and half-written.
    #[test]
    fn bib_entry_offset_skips_whitespace_and_stops_at_the_end_of_a_truncated_bib() {
        // Canonical, and the offset is the `@`, not the key.
        assert_eq!(
            bib_offset("x\n@article{smith2020,\n}", "smith2020"),
            Some(2)
        );
        // BibTeX allows whitespace before the brace and after it, so both must be skipped.
        assert_eq!(bib_offset("@article {key,\n}", "key"), Some(0));
        assert_eq!(bib_offset("@article{ key,\n}", "key"), Some(0));
        assert_eq!(bib_offset("@article { key ,\n}", "key"), Some(0));
        // …but the key itself must match whole: a longer key is not a hit on its prefix.
        assert_eq!(bib_offset("@article{keyword,\n}", "key"), None);
        // An entry needs a type; `@{…}` is not a header.
        assert_eq!(bib_offset("@{key,\n}", "key"), None);
        // An empty key matches nothing rather than every entry.
        assert_eq!(bib_offset("@article{key,\n}", ""), None);

        // Truncated after each part of the header in turn: None, never a read past the end.
        for truncated in [
            "@article",
            "@article ",
            "@article{",
            "@article{ ",
            "@article{key",
            "@article{key ",
        ] {
            assert_eq!(
                bib_offset(truncated, "key"),
                None,
                "a `.bib` truncated at {truncated:?} must not resolve a key"
            );
        }
    }

    #[test]
    fn frontmatter_bib_paths_reads_scalar_and_list() {
        assert_eq!(
            frontmatter_bib_paths("---\nbibliography: refs.bib\n---\n"),
            vec!["refs.bib".to_string()]
        );
        assert_eq!(
            frontmatter_bib_paths("---\nbibliography: \"a.bib\"\n---"),
            vec!["a.bib".to_string()]
        );
        assert_eq!(
            frontmatter_bib_paths("---\nbibliography:\n  - a.bib\n  - b.bib\n---"),
            vec!["a.bib".to_string(), "b.bib".to_string()]
        );
        assert_eq!(
            frontmatter_bib_paths("---\ntitle: x\n---"),
            Vec::<String>::new()
        );
        assert_eq!(
            frontmatter_bib_paths("bibliography: x.bib"),
            Vec::<String>::new()
        );
    }

    /// Where the front-matter scan starts, where it stops, and that it walks forwards.
    ///
    /// Every fixture above puts `bibliography:` on the *first* line of a *terminated* front
    /// matter, which is the one shape that hides all three of this loop's defects: a cursor that
    /// walks backwards still reads line 1, a scan that never terminates still finds the key, and a
    /// bound one line too wide is only reached when the document has no closing `---`.
    #[test]
    fn frontmatter_bib_paths_scans_forwards_and_only_inside_the_front_matter() {
        // Not the first key: the scan has to walk forwards to reach it.
        assert_eq!(
            frontmatter_bib_paths("---\ntitle: x\nbibliography: refs.bib\n---\n"),
            vec!["refs.bib".to_string()]
        );
        // A `bibliography:` line in the body is not front matter.
        assert_eq!(
            frontmatter_bib_paths("---\nbibliography: a.bib\n---\n\nbibliography: body.bib\n"),
            vec!["a.bib".to_string()]
        );
        // `...` closes front matter as well as `---`.
        assert_eq!(
            frontmatter_bib_paths("---\nbibliography: a.bib\n...\nbibliography: body.bib\n"),
            vec!["a.bib".to_string()]
        );
        // Unterminated front matter (an author mid-edit): stop at the last line, not past it.
        assert_eq!(
            frontmatter_bib_paths("---\nbibliography: a.bib\n"),
            vec!["a.bib".to_string()]
        );
    }

    /// A `bibliography:` list ends at its first non-item, and an empty `-` is a non-item.
    ///
    /// The guard is the whole stopping rule: without it an empty `-` yields an empty path (which
    /// `dir.join` resolves to the document's own directory) and the scan carries on past the end
    /// of the list, so a half-typed entry silently changes which files are read.
    #[test]
    fn a_bibliography_list_stops_at_the_first_non_item() {
        assert_eq!(
            frontmatter_bib_paths("---\nbibliography:\n  - a.bib\n  -\n  - b.bib\n---\n"),
            vec!["a.bib".to_string()]
        );
    }
}
