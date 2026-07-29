//! BibTeX parsing (`@string` macro expansion, brace/quote values) into a [`Bibliography`].

use super::{Bibliography, Entry};
use std::collections::HashMap;

/// Parse a BibTeX string into a [`Bibliography`]. Tolerant of `{...}`/`"..."`
/// values and brace nesting; ignores comments and `@string`/`@comment`.
pub fn parse_bib(text: &str) -> Bibliography {
    parse_bib_warned(text).0
}

/// Like [`parse_bib`] but also returns warnings — currently for duplicate citation
/// keys, which would otherwise silently last-write-win and render the wrong reference
/// (the same overwrite-vs-warn class as duplicate cross-reference labels).
pub fn parse_bib_warned(text: &str) -> (Bibliography, Vec<String>) {
    let mut warnings = Vec::new();
    let mut entries = HashMap::new();
    // `@string{ key = "value" }` macro table (keys are case-insensitive in BibTeX),
    // resolved as we parse so later entries can reference earlier definitions.
    let mut strings: HashMap<String, String> = HashMap::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
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
        if kind == "comment" || kind == "preamble" {
            skip_entry(&chars, &mut i, open, close);
            continue;
        }
        if kind == "string" {
            // One `name = value` pair; `value` may itself reference earlier strings.
            skip_ws(&chars, &mut i);
            let name = take_while(&chars, &mut i, |c| c != '=' && c != '}' && c != ')')
                .trim()
                .to_ascii_lowercase();
            skip_ws(&chars, &mut i);
            if i < chars.len() && chars[i] == '=' {
                i += 1;
                skip_ws(&chars, &mut i);
                let value = read_value(&chars, &mut i, &strings);
                if !name.is_empty() {
                    strings.insert(name, value);
                }
            }
            skip_entry(&chars, &mut i, open, close);
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
        loop {
            skip_ws(&chars, &mut i);
            if i >= chars.len() || chars[i] == close {
                break;
            }
            let name = take_while(&chars, &mut i, |c| c != '=' && c != '}' && c != ',')
                .trim()
                .to_ascii_lowercase();
            skip_ws(&chars, &mut i);
            if i >= chars.len() || chars[i] != '=' {
                break;
            }
            i += 1; // past '='
            skip_ws(&chars, &mut i);
            let value = read_value(&chars, &mut i, &strings);
            if !name.is_empty() {
                fields.insert(name, value);
            }
            skip_ws(&chars, &mut i);
            if i < chars.len() && chars[i] == ',' {
                i += 1;
            }
        }
        if i < chars.len() && chars[i] == close {
            i += 1;
        }
        if !key.is_empty() {
            if entries.contains_key(&key) {
                warnings.push(format!(
                    "duplicate bibliography key \u{201c}{key}\u{201d} (using the last definition)"
                ));
            }
            entries.insert(key, Entry { kind, fields });
        }
    }
    // A freshly parsed database is a single layer, so every key is page-local. A
    // project-wide layer loses that status when the page's own is laid over it
    // (`Bibliography::overlay`).
    let local = entries.keys().cloned().collect();
    (Bibliography { entries, local }, warnings)
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
/// not close the entry early.
fn skip_entry(chars: &[char], i: &mut usize, open: char, close: char) {
    let mut depth = 1;
    while *i < chars.len() && depth > 0 {
        let c = chars[*i];
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
        }
        *i += 1;
    }
}

/// Read a (possibly `#`-concatenated) field value: a sequence of `{...}`
/// (brace-nested), `"..."`, or bare-token parts joined by `#`. A bare token is
/// resolved against the `@string` macro table (`strings`); an unknown bare token is
/// kept verbatim (BibTeX would error, but tolerance beats dropping content). One
/// level of braces is stripped, so a double-brace value (`{{Corporate Name}}`)
/// retains its inner braces for the author formatter to treat as a literal name.
fn read_value(chars: &[char], i: &mut usize, strings: &HashMap<String, String>) -> String {
    let mut parts: Vec<String> = Vec::new();
    loop {
        skip_ws(chars, i);
        match chars.get(*i) {
            Some('{') => {
                let mut inner = String::new();
                let mut depth = 0;
                while *i < chars.len() {
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
                // A `"..."` value may contain brace groups; honor their nesting so an
                // embedded `"` inside braces doesn't end the value prematurely.
                let mut depth: usize = 0;
                while *i < chars.len() {
                    match chars[*i] {
                        '{' => depth += 1,
                        '}' => depth = depth.saturating_sub(1),
                        '"' if depth == 0 => {
                            *i += 1;
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
    normalize_ws(&parts.join(""))
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
