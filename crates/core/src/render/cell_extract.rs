//! Cell-option parsing: the `#|`/`//|`/`%%|` directive primitive and the pure leaf
//! parsers that key off it: language detection, boolean flags, code-fold, option
//! stripping, source slicing, and `{js}` option parsing.
//! All take a code literal/lines + key and return derived strings/bools; none touches
//! the orchestrator's shared state.

use super::{BufLine, CodeFold, JsOpts};

/// If `line` is a leading cell-option directive, return the content after the pipe.
/// Recognizes `#|` (most langs), `//|` (JS), `%%|` (mermaid), each tolerating optional
/// whitespace between the comment marker and the pipe (`# |`, `// |`, `%% |`). No corpus
/// document uses the spaced form any more; `spaced_option_directives_are_recognized` pins it.
/// Returns `None` for a plain comment or code line. This is the single primitive every
/// option parser keys off (`cell_option`, `strip_cell_options`, `validate`).
pub fn option_directive(line: &str) -> Option<&str> {
    let t = line.trim_start();
    for marker in ["#", "//", "%%"] {
        if let Some(rest) = t.strip_prefix(marker) {
            return rest.trim_start_matches([' ', '\t']).strip_prefix('|');
        }
    }
    None
}

/// Read a leading `#| key: value` cell option (returns the unquoted value).
/// Only scans the contiguous leading option block, stopping at the first code
/// line. See [`option_directive`] for the recognized prefixes.
pub(super) fn cell_option<'a>(literal: &'a str, key: &str) -> Option<&'a str> {
    for line in literal.lines() {
        let Some(opt) = option_directive(line) else {
            break;
        };
        if let Some((k, v)) = opt.split_once(':')
            && k.trim() == key
        {
            return Some(option_value(v));
        }
    }
    None
}

/// A raw `#|` option value as YAML reads it: a trailing comment dropped, then the
/// surrounding quotes. YAML's rule is that a `#` after whitespace starts a comment; in a
/// quoted value only the `#` after the closing quote does. Without this,
/// `#| echo: false  # hide setup` was not the word `false`, and the hidden cell's code
/// was published.
fn option_value(raw: &str) -> &str {
    let v = raw.trim();
    let end = match v.chars().next() {
        // Quoted: the closing quote is the first one after which only a comment follows.
        Some(q @ ('"' | '\'')) => v
            .char_indices()
            .skip(1)
            .find(|&(i, c)| {
                c == q && {
                    let rest = v[i + 1..].trim_start();
                    rest.is_empty() || rest.starts_with('#')
                }
            })
            .map_or(v.len(), |(i, _)| i + 1),
        // Plain: the first `#` at the start or after whitespace.
        _ => v
            .char_indices()
            .find(|&(i, c)| c == '#' && (i == 0 || v[..i].ends_with([' ', '\t'])))
            .map_or(v.len(), |(i, _)| i),
    };
    v[..end].trim_end().trim_matches(['"', '\''])
}

/// A boolean cell option (`#| echo: false`) that falls back to a document default
/// (from `execute:`) when the cell doesn't set it. Only an explicit `false` turns
/// it off, so `echo: fenced` etc. still count as "shown".
pub(super) fn cell_flag_or(literal: &str, key: &str, default: bool) -> bool {
    match cell_option(literal, key) {
        // A recognized false word (`false`/`no`/`off`) turns it off; any other value
        // (`fenced`, `true`, …) counts as "on". Catches the YAML-1.1 words serde reads
        // as strings, so `#| echo: no` suppresses instead of silently echoing.
        Some(v) => crate::frontmatter::yaml_bool_word(v) != Some(false),
        None => default,
    }
}

/// A code cell whose source is suppressed (`#| echo: false` / `#| include: false`)
/// still needs a block in the list so the executor runs it and the output can be
/// placed after it; render it as an empty hidden marker carrying the data attrs.
pub(super) fn hidden_cell(attrs: &str) -> String {
    format!("<div{attrs} class=\"tali-cell-hidden\" hidden></div>")
}

/// If a cell sets `code-fold`, return its disclosure state. `true` folds (starts closed),
/// `show` folds but starts open; `code-summary` overrides the "Code" label — and whether it
/// did is carried through, because an authored label is the author's voice and the fallback
/// is the tool's.
pub(super) fn code_fold(literal: &str) -> Option<CodeFold> {
    let v = cell_option(literal, "code-fold")?;
    if v != "true" && v != "show" {
        return None;
    }
    Some(CodeFold {
        open: v == "show",
        summary: cell_option(literal, "code-summary")
            .unwrap_or("Code")
            .to_string(),
    })
}

/// Drop leading cell-option lines (`#|` for most languages, `//|` for JS,
/// `%%|` for mermaid; see [`option_directive`] for the spaced forms too).
pub(super) fn strip_cell_options(literal: &str) -> String {
    let mut body = String::new();
    let mut skipping = true;
    for line in literal.lines() {
        if skipping && option_directive(line).is_some() {
            continue;
        }
        skipping = false;
        body.push_str(line);
        body.push('\n');
    }
    if !literal.ends_with('\n') {
        body.pop();
    }
    body
}

pub(super) fn slice_lines(lines: &[&str], start: BufLine, end: BufLine) -> String {
    let s = start.get().saturating_sub(1);
    let e = end.get().min(lines.len());
    if s >= e {
        return String::new();
    }
    lines[s..e].join("\n")
}

/// Language for a fenced block: `{python}`/`{.python}`/`{js}` -> "python"/"js",
/// plain ` ```rust ` -> "rust". Pandoc raw-output attributes (`{=html}`,
/// `{=latex}`, ...) are not languages and return `None`.
/// Whether a fence info string marks an EXECUTABLE cell: ```` ```{python} ````, but not
/// ```` ```{.python code-line-numbers="1|2-3"} ````, whose leading dot is the documented
/// display-only form for a non-executing block (`docs/guide/reference/cell-options.tmd`),
/// nor a plain ```` ```python ```` fence.
///
/// Kept beside [`code_lang`] because the two must agree on fence syntax while answering
/// different questions: `code_lang` deliberately strips the dot (the display path still
/// highlights `{.python}` AS python, and the text projection still needs its language),
/// so it cannot itself be the executable/display gate. Testing only `starts_with('{')`
/// and leaning on `code_lang` is what let a display-only snippet warm a kernel and take
/// an output block.
pub(super) fn is_executable_fence(info: &str) -> bool {
    info.trim_start()
        .strip_prefix('{')
        .is_some_and(|inner| !inner.trim_start().starts_with('.'))
}

pub(super) fn code_lang(info: &str) -> Option<String> {
    let info = info.trim();
    if info.is_empty() {
        return None;
    }
    let token = if let Some(inner) = info.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        inner.trim().trim_start_matches('.')
    } else {
        info
    };
    // Split on a comma as well as whitespace. ` ```rust,ignore ` is the shared convention of
    // mdBook, rustdoc, Pandoc, Docusaurus and GitHub for "this is Rust, and here is an
    // attribute for it", and Taliesin used to take the whole string as the language: measured
    // on `rust-lang/book`, **329 of 457** reported problems were this one shape (734 fence
    // occurrences), and on one real chapter 11 of 18 code blocks rendered unstyled with
    // `class="language-rust,ignore"` — a comma is not a valid class token, so no highlighter
    // and no CSS rule could ever match it. The diagnostic then called the ecosystem's standard
    // form a spelling error (item 127). Everything after the first token is an attribute for a
    // tool Taliesin is not; the language is the first token either way.
    let lang = token.split([',', ' ', '\t']).next().unwrap_or("").trim();
    if lang.is_empty() || lang.starts_with('=') {
        return None;
    }
    Some(lang.to_string())
}

/// Parse the reactive cell options (`//| name:`/`//| viewof:`/`//| input:`) from the raw
/// fence body. Read for every **client-side** language (`{js}`, `{glsl}`; see
/// [`super::client_lang`]) and empty for every other, so a `{glsl}` shader can take a
/// `//| input: k` uniform through the same graph a `{js}` cell uses. `//` is the comment
/// marker in both languages, so [`option_directive`] already recognizes the directive.
pub(super) fn parse_js_opts(literal: &str, lang: &str) -> JsOpts {
    if super::client_lang(lang).is_none() {
        return JsOpts::default();
    }
    JsOpts {
        name: cell_option(literal, "name").map(str::to_string),
        viewof: cell_option(literal, "viewof").map(str::to_string),
        inputs: cell_option(literal, "input")
            .map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
    }
}
