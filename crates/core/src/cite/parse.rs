//! BibTeX parsing (`@string` macro expansion, brace/quote values) into a [`Bibliography`].

use super::{Bibliography, Entry};
use std::collections::HashMap;

/// Parse a BibTeX string into a [`Bibliography`]. Tolerant of `{...}`/`"..."`
/// values and brace nesting; ignores comments and `@string`/`@comment`.
pub fn parse_bib(text: &str) -> Bibliography {
    parse_bib_warned(text).0
}

/// Like [`parse_bib`] but also returns warnings: a duplicate citation key, which would
/// otherwise silently last-write-win and render the wrong reference (the same
/// overwrite-vs-warn class as duplicate cross-reference labels), and an entry that is
/// never closed.
pub fn parse_bib_warned(text: &str) -> (Bibliography, Vec<String>) {
    let mut bib = Bibliography::default();
    let warnings = read_into(&mut bib, "", text, &mut HashMap::new());
    (bib, warnings)
}

/// Parse the text of ONE `.bib` file into `bib`, returning its diagnostics. `file` names
/// the file in those messages (empty for a bare text).
///
/// One file at a time, because a file is where BibTeX's own syntax ends: an entry left
/// unclosed at the end of `a.bib` is an error at the end of `a.bib`, not the start of an
/// entry that swallows the first one in `b.bib`. The files of one bibliography used to be
/// concatenated and parsed as one text, which is exactly what let that happen.
///
/// `strings` is the `@string` macro table (keys are case-insensitive in BibTeX). It is the
/// caller's so it can flow from one file into the next, as `\bibliography{a,b}` shares
/// macros across its files.
pub(crate) fn read_into(
    bib: &mut Bibliography,
    file: &str,
    text: &str,
    strings: &mut HashMap<String, String>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let entries = &mut bib.entries;
    let chars: Vec<char> = text.chars().collect();
    // Where a diagnostic is, for a message: "`refs.bib` line 12".
    let place = |at: usize| {
        let line = chars[..at].iter().filter(|&&c| c == '\n').count() + 1;
        if file.is_empty() {
            format!("line {line}")
        } else {
            format!("`{file}` line {line}")
        }
    };
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        let at = i;
        i += 1;
        let kind = take_while(&chars, &mut i, |c| c.is_alphanumeric()).to_ascii_lowercase();
        skip_ws(&chars, &mut i);
        // Any entry (`@string`/`@preamble`/regular) may use either `{...}` or `(...)`
        // delimiters; it closes at the MATCHING delimiter. A paren entry does NOT end
        // at a `}`, so the close char must be tracked — otherwise the field loop runs
        // past the `)` and swallows every following `@entry` (JabRef and older BibTeX
        // both emit the paren form).
        if i >= chars.len() || !matches!(chars[i], '{' | '(') {
            continue;
        }
        let open = chars[i];
        let close = if open == '(' { ')' } else { '}' };
        i += 1; // past the opening delimiter
        let unclosed = |name: &str| {
            format!(
                "{}: entry `{name}` is not closed; add its closing `{close}`",
                place(at)
            )
        };
        if kind == "comment" || kind == "preamble" {
            if !skip_entry(&chars, &mut i, open, close) {
                warnings.push(unclosed(&format!("@{kind}")));
            }
            continue;
        }
        if kind == "string" {
            // One `name = value` pair; `value` may itself reference earlier strings.
            skip_ws(&chars, &mut i);
            let name = take_while(&chars, &mut i, |c| c != '=' && c != '}' && c != ')')
                .trim()
                .to_ascii_lowercase();
            skip_ws(&chars, &mut i);
            let mut closed = true;
            if i < chars.len() && chars[i] == '=' {
                i += 1;
                skip_ws(&chars, &mut i);
                let (value, ok) = read_value(&chars, &mut i, strings);
                closed = ok;
                if !name.is_empty() {
                    strings.insert(name, value);
                }
            }
            if !(closed && skip_entry(&chars, &mut i, open, close)) {
                warnings.push(unclosed("@string"));
            }
            continue;
        }
        // Read the entry key with the SAME predicate the in-prose reference scanner
        // uses (`is_cite_key_char`), so any key the bib stores can also be `[@cited]`.
        // Skip whitespace on both sides (`@article{ key ,` stays tolerant) since the
        // predicate — unlike the old `!= ',' && != '}'` catch-all — stops at spaces.
        skip_ws(&chars, &mut i);
        let key = take_while(&chars, &mut i, super::is_cite_key_char);
        let mut fields = HashMap::new();
        skip_ws(&chars, &mut i);
        if i < chars.len() && chars[i] == ',' {
            i += 1;
        }
        // How the entry ended. Not at its own closing delimiter when the file ends first,
        // or when a new entry starts at the beginning of a line while this one is still
        // open: the one place a `}` goes missing is the end of an entry, so the next
        // `@type{` is where the author meant this one to stop. The outer scan resumes
        // there, so that entry is read rather than swallowed.
        let mut end = End::Unclosed;
        loop {
            skip_ws(&chars, &mut i);
            if i >= chars.len() || entry_starts_at(&chars, i) {
                break;
            }
            if chars[i] == close {
                i += 1;
                end = End::Closed;
                break;
            }
            let name_at = i;
            let name = take_while(&chars, &mut i, |c| {
                c != '=' && c != '}' && c != ',' && c != '@'
            })
            .trim()
            .to_ascii_lowercase();
            skip_ws(&chars, &mut i);
            if i >= chars.len() || entry_starts_at(&chars, i) {
                break;
            }
            if chars[i] != '=' {
                end = End::Malformed(name_at);
                break;
            }
            i += 1; // past '='
            skip_ws(&chars, &mut i);
            let (value, value_closed) = read_value(&chars, &mut i, strings);
            if !name.is_empty() {
                fields.insert(name, value);
            }
            if !value_closed {
                break;
            }
            skip_ws(&chars, &mut i);
            if i < chars.len() && chars[i] == ',' {
                i += 1;
            }
        }
        if key.is_empty() {
            continue;
        }
        match end {
            End::Closed => {}
            End::Unclosed => warnings.push(unclosed(&key)),
            End::Malformed(pos) => warnings.push(format!(
                "{}: entry `{key}` has a field with no `=`, so the rest of the entry was not read",
                place(pos)
            )),
        }
        if entries.contains_key(&key) {
            warnings.push(format!(
                "duplicate bibliography key \u{201c}{key}\u{201d} (using the last definition)"
            ));
        }
        entries.insert(key, Entry { kind, fields });
    }
    warnings
}

/// Whether an entry (`@type{` or `@type(`) starts at `chars[i]`, which must be the first
/// non-blank character of its line. That position is what makes it a recovery point and
/// not text: an e-mail address or an `@` inside a value never sits there followed by a
/// type name and an opening delimiter.
fn entry_starts_at(chars: &[char], i: usize) -> bool {
    if chars.get(i) != Some(&'@') {
        return false;
    }
    let line_start = chars[..i]
        .iter()
        .rev()
        .take_while(|&&c| c != '\n')
        .all(|c| *c == ' ' || *c == '\t');
    if !line_start {
        return false;
    }
    let mut j = i + 1;
    let name_start = j;
    while j < chars.len() && chars[j].is_ascii_alphabetic() {
        j += 1;
    }
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    j > name_start && matches!(chars.get(j), Some('{' | '('))
}

/// How an entry's field loop ended.
enum End {
    /// At the entry's own closing delimiter.
    Closed,
    /// At the end of the file, or where the next entry starts.
    Unclosed,
    /// At a field with no `=` (its start offset), after which nothing more is read.
    Malformed(usize),
}

fn take_while(chars: &[char], i: &mut usize, pred: impl Fn(char) -> bool) -> String {
    let start = *i;
    while *i < chars.len() && pred(chars[*i]) {
        *i += 1;
    }
    chars[start..*i].iter().collect()
}

fn skip_ws(chars: &[char], i: &mut usize) {
    while *i < chars.len() && chars[*i].is_whitespace() {
        *i += 1;
    }
}

/// Skip to just past the matching close delimiter of an entry opened with `open`
/// (`{` or `(`), counting nested pairs of the SAME delimiter so an inner group does
/// not close the entry early. `false` when the file ends, or the next entry starts
/// ([`entry_starts_at`]), before it closes.
fn skip_entry(chars: &[char], i: &mut usize, open: char, close: char) -> bool {
    let mut depth = 1;
    while *i < chars.len() && depth > 0 {
        let c = chars[*i];
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
        } else if entry_starts_at(chars, *i) {
            return false;
        }
        *i += 1;
    }
    depth == 0
}

/// Read a (possibly `#`-concatenated) field value: a sequence of `{...}`
/// (brace-nested), `"..."`, or bare-token parts joined by `#`. A bare token is
/// resolved against the `@string` macro table (`strings`); an unknown bare token is
/// kept verbatim (BibTeX would error, but tolerance beats dropping content). One
/// level of braces is stripped, so a double-brace value (`{{Corporate Name}}`)
/// retains its inner braces for the author formatter to treat as a literal name.
///
/// The `bool` is `false` when a `{...}` or `"..."` part never closed: it stops where the
/// next entry starts ([`entry_starts_at`]) instead of running on through the rest of the
/// file, and the caller reports the entry as unclosed.
fn read_value(chars: &[char], i: &mut usize, strings: &HashMap<String, String>) -> (String, bool) {
    let mut parts: Vec<String> = Vec::new();
    let mut closed = true;
    loop {
        skip_ws(chars, i);
        match chars.get(*i) {
            Some('{') => {
                let mut inner = String::new();
                let mut depth = 0;
                closed = false;
                while *i < chars.len() {
                    if entry_starts_at(chars, *i) {
                        break;
                    }
                    match chars[*i] {
                        '{' => {
                            depth += 1;
                            if depth > 1 {
                                inner.push('{');
                            }
                        }
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                *i += 1;
                                closed = true;
                                break;
                            }
                            inner.push('}');
                        }
                        c => inner.push(c),
                    }
                    *i += 1;
                }
                parts.push(inner);
            }
            Some('"') => {
                let mut inner = String::new();
                *i += 1;
                closed = false;
                // A `"..."` value may contain brace groups; honor their nesting so an
                // embedded `"` inside braces doesn't end the value prematurely.
                let mut depth: usize = 0;
                while *i < chars.len() {
                    if entry_starts_at(chars, *i) {
                        break;
                    }
                    match chars[*i] {
                        '{' => depth += 1,
                        '}' => depth = depth.saturating_sub(1),
                        '"' if depth == 0 => {
                            *i += 1;
                            closed = true;
                            break;
                        }
                        _ => {}
                    }
                    inner.push(chars[*i]);
                    *i += 1;
                }
                // Strip one outer brace level, matching the `{...}` arm: a
                // `"{First Last}"` is an ordinary (case-protected) person name, so it
                // must reach the author formatter WITHOUT the leading `{` that would
                // otherwise mark it a literal corporate name; `"{{Corp}}"` keeps one
                // brace pair and stays literal, exactly like the `{{Corp}}` form.
                parts.push(strip_one_outer_brace_group(&inner));
            }
            _ => {
                let token = take_while(chars, i, |c| {
                    c != ',' && c != '}' && c != ')' && c != '#' && !c.is_whitespace()
                });
                if token.is_empty() {
                    break;
                }
                // Bare token: a number stays literal, otherwise resolve as a @string ref.
                let resolved = strings
                    .get(&token.to_ascii_lowercase())
                    .cloned()
                    .unwrap_or(token);
                parts.push(resolved);
            }
        }
        if !closed {
            break;
        }
        skip_ws(chars, i);
        if *i < chars.len() && chars[*i] == '#' {
            *i += 1; // concatenation: keep reading parts
            continue;
        }
        break;
    }
    // A double-brace value (`{{World Health Organization}}`) keeps its INNER braces
    // here (the brace arm strips only one level), so the author formatter sees a
    // leading `{` and renders it as a literal corporate name. A single-brace
    // `{First Last}` keeps no braces and initials normally — the standard convention.
    (normalize_ws(&parts.join("")), closed)
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// If `s` is entirely one brace group (`{...}` whose opening brace matches the final
/// char), return the inside; otherwise return `s` unchanged. Used to give the `"..."`
/// value arm the same single-level strip the `{...}` arm performs inline, so a
/// whole-value brace group is peeled once (and no more) regardless of the delimiter.
fn strip_one_outer_brace_group(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.first() != Some(&'{') {
        return s.to_string();
    }
    let mut depth = 0usize;
    for (idx, &c) in chars.iter().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    // The opening brace closes here; only peel it if it wraps the WHOLE
                    // value (`{First Last}`), not a leading group (`{A} and {B}`).
                    return if idx == chars.len() - 1 {
                        chars[1..idx].iter().collect()
                    } else {
                        s.to_string()
                    };
                }
            }
            _ => {}
        }
    }
    s.to_string() // unbalanced: leave as-is
}
