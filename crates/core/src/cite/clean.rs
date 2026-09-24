//! LaTeX/BibTeX field cleaning: resolve accent macros + special letters to Unicode,
//! unescape `\&`/`\%`/…, and strip `\url{}`/brace cruft. Used by the formatters
//! (`format`) and the author name builder (`author`).

/// Strip BibTeX/LaTeX cruft from a field value: resolve accent macros to Unicode,
/// unescape the common backslash escapes (`\&`/`\%`/`\_`/`\#`/`\$`), and drop
/// `\url{}` wrappers and brace groups (capitalization guards). Accent + escape
/// resolution (both in `latex_accents`) runs FIRST, while braces still delimit a
/// macro argument (`M{\"u}ller` -> `Müller`); the surviving braces are then stripped.
///
/// `\url{...}` is handled as an unknown macro inside `latex_accents`: the `\url`
/// control word is dropped and its braced argument kept verbatim (then de-braced
/// below), so `\url{http://a.com/x_y}` -> `http://a.com/x_y`. (An earlier naive
/// `replace("\\url", "")` corrupted any word merely CONTAINING the substring, e.g.
/// `\urlstyle`, and deleted a bare `\url` with no argument.)
///
/// A math span (`$…$`, `\(…\)`, `\ensuremath{…}`) is kept as the TeX the author wrote,
/// braces and all: references are not typeset, and TeX text is the honest fallback where
/// resolving its macros one by one would change the formula (`$O(n \log n)$`).
pub(crate) fn clean(s: &str) -> String {
    let mut out = String::new();
    for (part, math) in split_math(s) {
        if math {
            out.push_str(&part);
        } else {
            out.push_str(&latex_accents(&part).replace(['{', '}'], ""));
        }
    }
    out.trim().to_string()
}

/// `s` cut into text and math parts, in order, each flagged `true` for math. The math
/// delimiters are `$…$`, `\(…\)` and `\ensuremath{…}` (written back as `$…$`); an
/// escaped `\$` is text, and a `$` with no partner is text too.
fn split_math(s: &str) -> Vec<(String, bool)> {
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        // `\\` and `\$` are text: the second backslash or the dollar is escaped.
        if rest.starts_with("\\\\") || rest.starts_with("\\$") {
            text.push_str(&rest[..2]);
            i += 2;
            continue;
        }
        let math = if let Some(body) = rest.strip_prefix('$') {
            let mut end = None;
            let mut prev = '$';
            for (k, c) in body.char_indices() {
                if c == '$' && prev != '\\' {
                    end = Some(k);
                    break;
                }
                prev = c;
            }
            end.map(|k| (rest[..k + 2].to_string(), k + 2))
        } else if rest.starts_with("\\(") {
            rest.find("\\)").map(|k| (rest[..k + 2].to_string(), k + 2))
        } else if let Some(body) = rest.strip_prefix("\\ensuremath{") {
            let mut depth = 1usize;
            body.char_indices()
                .find(|&(_, c)| {
                    match c {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    depth == 0
                })
                .map(|(k, _)| {
                    let len = "\\ensuremath{".len() + k + 1;
                    (format!("${}$", &body[..k]), len)
                })
        } else {
            None
        };
        match math {
            Some((span, len)) => {
                if !text.is_empty() {
                    parts.push((std::mem::take(&mut text), false));
                }
                parts.push((span, true));
                i += len;
            }
            None => {
                let c = rest.chars().next().unwrap_or_default();
                text.push(c);
                i += c.len_utf8();
            }
        }
    }
    if !text.is_empty() {
        parts.push((text, false));
    }
    parts
}

/// Resolve the common LaTeX accent / special-letter macros to composed Unicode.
///
/// Two macro shapes are handled:
///
/// * **No-argument letters** (`\ss`, `\AA`, `\o`, `\i`, …): mapped directly, with a
///   trailing `{}` or word-break consumed (`\ss{}` and `\ss ` both -> `ß`).
/// * **Accent + base letter** (`\"o`, `\'e`, `\H{o}`, `\c{c}`, …): the accent macro
///   names a combining diacritic; the next letter (optionally brace-wrapped, and
///   itself possibly a nested macro like `{\H{o}}`) is the base. Resolved to the
///   precomposed character when one exists, else base + combining mark.
///
/// An unknown control word followed by an argument (`\emph{x}`, `\url{…}`) is a
/// formatting command: its name is dropped and the argument kept. Any other unknown
/// control word (`\foo`, `\foo{}`) is kept as written, so the author sees what was not
/// understood instead of losing the text it stood for.
fn latex_accents(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '\\' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // chars[i] == '\\': read the macro name (a run of letters, or a single
        // non-letter accent char like " ' ` ^ ~ = . ).
        let mut j = i + 1;
        let name: String = if j < chars.len() && chars[j].is_ascii_alphabetic() {
            let start = j;
            while j < chars.len() && chars[j].is_ascii_alphabetic() {
                j += 1;
            }
            chars[start..j].iter().collect()
        } else if j < chars.len() {
            let c = chars[j];
            j += 1;
            c.to_string()
        } else {
            out.push('\\');
            break;
        };
        // Literal-escape macros (`\&`, `\%`, `\_`, `\#`, `\$`): the macro IS the
        // character — keep it verbatim. (Escaped braces `\{`/`\}` are intentionally
        // NOT special-cased: like the original `clean`, all braces are stripped.)
        if name.len() == 1 && matches!(name.as_str(), "&" | "%" | "_" | "#" | "$") {
            out.push_str(&name);
            i = j;
            continue;
        }
        // No-argument special letters (ß, Å, ø, ı, …).
        if let Some(rep) = special_letter(&name) {
            out.push_str(rep);
            // Consume an immediately following `{}` (the `\ss{}` idiom) so it doesn't
            // leak as empty braces, and skip the macro-terminating space if any.
            if j + 1 < chars.len() && chars[j] == '{' && chars[j + 1] == '}' {
                j += 2;
            } else if j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            i = j;
            continue;
        }
        // Accent macros take a base letter argument.
        if let Some(diacritic) = accent_diacritic(&name) {
            // A macro-terminating space (`\v s`) is not part of the argument.
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            let (base, next) = read_accent_arg(&chars, j);
            if let Some(base) = base {
                out.push_str(&compose(base, diacritic));
            }
            i = next;
            continue;
        }
        // Unknown control word. With an argument it is a formatting command: drop the
        // name and let later iterations emit the argument. Without one (or with an empty
        // `{}`) keep it as written. A control SYMBOL (`\-`, `\/`) prints nothing.
        if name.starts_with(|c: char| c.is_ascii_alphabetic()) {
            let empty_arg = chars.get(j) == Some(&'{') && chars.get(j + 1) == Some(&'}');
            if empty_arg || chars.get(j) != Some(&'{') {
                out.push('\\');
                out.push_str(&name);
                if empty_arg {
                    j += 2;
                }
            }
        }
        i = j;
    }
    out
}

/// Read the single base "letter" an accent macro applies to, starting at `j`:
/// a brace group `{...}` (recursively de-accented), a nested macro `\i`/`\j`/… (the
/// dotless-i idiom `\"\i`), or one character. Returns the resolved base string
/// (`None` if absent, e.g. `\"{}`) and the index past it.
fn read_accent_arg(chars: &[char], j: usize) -> (Option<String>, usize) {
    match chars.get(j) {
        Some('{') => {
            // Find the matching close brace.
            let mut depth = 0usize;
            let mut k = j;
            while k < chars.len() {
                match chars[k] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                k += 1;
            }
            let inner: String = chars[j + 1..k.min(chars.len())].iter().collect();
            let resolved = latex_accents(&inner);
            let arg = (!resolved.is_empty()).then_some(resolved);
            (arg, (k + 1).min(chars.len()))
        }
        // A nested control sequence as the base (`\"\i` -> ï): consume `\` + the
        // macro name (a letter run, or one symbol) and resolve it.
        Some('\\') => {
            let mut k = j + 1;
            if k < chars.len() && chars[k].is_ascii_alphabetic() {
                while k < chars.len() && chars[k].is_ascii_alphabetic() {
                    k += 1;
                }
            } else if k < chars.len() {
                k += 1;
            }
            let macro_src: String = chars[j..k].iter().collect();
            // `\i`/`\j` are dotless ONLY so an accent can sit on them; when they are
            // the base of an accent, the precomposed letter uses the DOTTED i/j (e.g.
            // `\"\i` -> ï = U+00EF, not the decomposed ı + diaeresis).
            let resolved = match macro_src.as_str() {
                r"\i" => "i".to_string(),
                r"\j" => "j".to_string(),
                _ => latex_accents(&macro_src),
            };
            // A macro-terminating space after a control WORD is swallowed.
            if k < chars.len()
                && chars[k] == ' '
                && macro_src
                    .chars()
                    .nth(1)
                    .is_some_and(|c| c.is_ascii_alphabetic())
            {
                k += 1;
            }
            ((!resolved.is_empty()).then_some(resolved), k)
        }
        Some(&c) => (Some(c.to_string()), j + 1),
        None => (None, j),
    }
}

/// Combine a base string with a combining diacritic, preferring a precomposed
/// character. Only the first scalar of `base` carries the accent (the common case
/// is a single letter; a multi-char base keeps its tail verbatim).
fn compose(base: String, combining: char) -> String {
    let mut it = base.chars();
    let Some(first) = it.next() else {
        return String::new();
    };
    let rest: String = it.collect();
    let combined = match precomposed(first, combining) {
        Some(c) => c.to_string(),
        None => format!("{first}{combining}"),
    };
    format!("{combined}{rest}")
}

/// Map an accent macro name to its Unicode COMBINING diacritic.
fn accent_diacritic(name: &str) -> Option<char> {
    Some(match name {
        "`" => '\u{0300}',  // grave
        "'" => '\u{0301}',  // acute
        "^" => '\u{0302}',  // circumflex
        "~" => '\u{0303}',  // tilde
        "\"" => '\u{0308}', // diaeresis / umlaut
        "=" => '\u{0304}',  // macron
        "." => '\u{0307}',  // dot above
        "u" => '\u{0306}',  // breve
        "v" => '\u{030C}',  // caron / háček
        "H" => '\u{030B}',  // double acute
        "c" => '\u{0327}',  // cedilla
        "k" => '\u{0328}',  // ogonek
        "r" => '\u{030A}',  // ring above
        "d" => '\u{0323}',  // dot below
        "b" => '\u{0331}',  // bar/macron below
        "t" => '\u{0361}',  // tie (double inverted breve)
        _ => return None,
    })
}

/// No-argument special letters, symbols and logos, and the declarations that print
/// nothing.
fn special_letter(name: &str) -> Option<&'static str> {
    Some(match name {
        // Symbols and logos that exporters (Better BibTeX above all) write for Unicode.
        "textendash" => "\u{2013}",
        "textemdash" => "\u{2014}",
        "textquoteleft" => "\u{2018}",
        "textquoteright" => "\u{2019}",
        "textquotedblleft" => "\u{201c}",
        "textquotedblright" => "\u{201d}",
        "ldots" | "dots" | "textellipsis" => "\u{2026}",
        "textdegree" => "\u{b0}",
        "texttimes" => "\u{d7}",
        "S" => "\u{a7}",
        "copyright" | "textcopyright" => "\u{a9}",
        "textregistered" => "\u{ae}",
        "texttrademark" => "\u{2122}",
        "textless" => "<",
        "textgreater" => ">",
        "textasciitilde" => "~",
        "textbackslash" => "\\",
        "TeX" => "TeX",
        "LaTeX" => "LaTeX",
        "BibTeX" => "BibTeX",
        // Font and spacing declarations (`{\em Title}`, `{\sc Fortran}`): no text.
        "em" | "it" | "bf" | "sc" | "rm" | "tt" | "sl" | "sf" | "itshape" | "bfseries"
        | "scshape" | "upshape" | "normalfont" | "relax" => "",
        "AA" => "Å",
        "aa" => "å",
        "AE" => "Æ",
        "ae" => "æ",
        "OE" => "Œ",
        "oe" => "œ",
        "O" => "Ø",
        "o" => "ø",
        "ss" => "ß",
        "L" => "Ł",
        "l" => "ł",
        "DH" => "Ð",
        "dh" => "ð",
        "TH" => "Þ",
        "th" => "þ",
        "i" => "ı",
        "j" => "ȷ",
        _ => return None,
    })
}

/// Precomposed character for a base letter + combining diacritic, when one exists in
/// Unicode. Uses canonical composition (NFC): a base + a single combining mark that
/// has a precomposed form collapses to one scalar (e.g. `o` + `\u{0308}` -> `ö`).
/// Anything without a precomposed form returns `None`, and the caller keeps the
/// base + combining mark (which still renders correctly, just decomposed).
fn precomposed(base: char, combining: char) -> Option<char> {
    use unicode_normalization::UnicodeNormalization;
    let s = format!("{base}{combining}");
    let composed: String = s.nfc().collect();
    let mut it = composed.chars();
    match (it.next(), it.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}
