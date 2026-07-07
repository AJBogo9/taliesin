//! Cell-option parsing: the `#|`/`//|`/`%%|` directive primitive and the pure leaf
//! parsers that key off it — language detection, boolean flags, document execute
//! defaults, code-fold, option stripping, source slicing, and `{js}` option parsing.
//! All take a code literal/lines + key and return derived strings/bools; none touches
//! the orchestrator's shared state.

use super::JsOpts;

/// If `line` is a leading cell-option directive, return the content after the pipe.
/// Recognizes `#|` (most langs), `//|` (JS), `%%|` (mermaid), each tolerating optional
/// whitespace between the comment marker and the pipe (`# |`, `// |`, `%% |`); the
/// spaced form is accepted, so the corpus may use it (e.g. `posts/pca-geometry`).
/// Returns `None` for a plain comment or code line. This is the single primitive every
/// option parser keys off (`cell_option`, `strip_cell_options`, `validate`).
pub(crate) fn option_directive(line: &str) -> Option<&str> {
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
            return Some(v.trim().trim_matches(['"', '\'']));
        }
    }
    None
}

/// A boolean cell option (`#| echo: false`) that falls back to a document default
/// (from `execute:`) when the cell doesn't set it. Only an explicit `false` turns
/// it off, so `echo: fenced` etc. still count as "shown".
pub(super) fn cell_flag_or(literal: &str, key: &str, default: bool) -> bool {
    match cell_option(literal, key) {
        Some("false") => false,
        Some(_) => true,
        None => default,
    }
}

/// Document-level cell defaults from a front-matter `execute:` block:
///
/// ```yaml
/// execute:
///   echo: false
///   include: false
///   cache: false
/// ```
///
/// Returns `(echo, include, cache)`, each defaulting to `true`. Per-cell `#|`
/// options override these. (`eval`/`output`/`warning` are not yet honoured.)
pub(super) fn detect_execute_defaults(front_matter: &str) -> (bool, bool, bool) {
    // Apply one `key: value` pair from an `execute:` mapping (shared by the block
    // and the inline flow form).
    fn apply_kv(k: &str, v: &str, echo: &mut bool, include: &mut bool, cache: &mut bool) {
        let v = v.trim().trim_matches(['"', '\'']);
        match k.trim() {
            "echo" => *echo = v != "false",
            "include" => *include = v != "false",
            "cache" => *cache = v != "false",
            _ => {}
        }
    }

    let (mut echo, mut include, mut cache) = (true, true, true);
    let mut in_block = false;
    for line in front_matter.lines() {
        let indent = line.len() - line.trim_start().len();
        let t = line.trim();
        if !in_block {
            if indent == 0
                && let Some(rest) = t.strip_prefix("execute:")
            {
                let rest = rest.trim();
                if let Some(inner) = rest.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    // Flow form on one line: `execute: {echo: false, cache: false}`.
                    for pair in inner.split(',') {
                        if let Some((k, v)) = pair.split_once(':') {
                            apply_kv(k, v, &mut echo, &mut include, &mut cache);
                        }
                    }
                } else if rest.is_empty() {
                    in_block = true; // block form: indented lines follow
                }
            }
            continue;
        }
        if t.is_empty() {
            continue;
        }
        if indent == 0 {
            break; // dedent ends the block
        }
        if let Some((k, v)) = t.split_once(':') {
            apply_kv(k, v, &mut echo, &mut include, &mut cache);
        }
    }
    (echo, include, cache)
}

/// A code cell whose source is suppressed (`#| echo: false` / `#| include: false`)
/// still needs a block in the list so the executor runs it and the output can be
/// placed after it; render it as an empty hidden marker carrying the data attrs.
pub(super) fn hidden_cell(attrs: &str) -> String {
    format!("<div{attrs} class=\"tali-cell-hidden\" hidden></div>")
}

/// If a cell sets `code-fold`, return `(start_open, summary)`. `true` folds
/// (starts closed), `show` folds but starts open; `code-summary` overrides the
/// "Code" label.
pub(super) fn code_fold(literal: &str) -> Option<(bool, String)> {
    let v = cell_option(literal, "code-fold")?;
    if v != "true" && v != "show" {
        return None;
    }
    let summary = cell_option(literal, "code-summary")
        .unwrap_or("Code")
        .to_string();
    Some((v == "show", summary))
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

pub(super) fn slice_lines(lines: &[&str], start: usize, end: usize) -> String {
    let s = start.saturating_sub(1);
    let e = end.min(lines.len());
    if s >= e {
        return String::new();
    }
    lines[s..e].join("\n")
}

/// Language for a fenced block: `{python}`/`{.python}`/`{js}` -> "python"/"js",
/// plain ` ```rust ` -> "rust". Pandoc raw-output attributes (`{=html}`,
/// `{=latex}`, ...) are not languages and return `None`.
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
    let lang = token.split_whitespace().next().unwrap_or("");
    if lang.is_empty() || lang.starts_with('=') {
        return None;
    }
    Some(lang.to_string())
}

/// Parse the native `{js}` cell options (`//| name:`/`//| viewof:`/`//| input:`)
/// from the raw fence body. Empty for every other language.
pub(super) fn parse_js_opts(literal: &str, lang: &str) -> JsOpts {
    if lang != "js" {
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
