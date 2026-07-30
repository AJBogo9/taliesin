//! Pure completion-context detection + live-candidate harvest for `.tmd`: decide WHICH
//! vocabulary applies at the cursor, and gather the document-defined ids / `.bib` keys /
//! sibling files that the static vocabulary can't know. Hand-rolled (no `regex` dependency;
//! each pattern is anchored at the cursor or the line start, so a short scan replaces it),
//! so the `lsp` server can answer completion for any editor. The static vocabulary stays
//! Rust-authoritative (`taliesin_core::vocab`); this module only routes to it and harvests
//! suggestion-only ids (`check` remains the arbiter).
//!
//! **This is the only implementation.** It began as a port of the VS Code companion's
//! `complete.ts`, which is gone as of 2026-07-28: the companion is now a thin client over
//! `taliesin lsp`. Adding a completion here gives it to every editor at once. See
//! `notes/2026-07-28-vscode-companion-audit.md`.

/// Front-matter parents whose immediate children have their own vocabulary.
const NESTED_PARENTS: &[&str] = &[
    "execute",
    "listing",
    "about",
    "hero",
    "prose-lint",
    "theorems",
];

/// Build/vcs dirs never worth offering as a `{{< embed/include >}}` target.
const IGNORE_DIRS: &[&str] = &[".git", "target", "node_modules", "_site", "_freeze"];

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
fn is_id_char(c: char) -> bool {
    is_word(c) || c == '-'
}
fn is_hspace(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// The two shortcodes that take a file-path first argument.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Shortcode {
    Embed,
    Include,
}

/// What completion applies at the cursor, decided from the line + document prefix.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CompletionContext {
    None,
    FrontmatterKey {
        parent: Option<String>,
    },
    FrontmatterValue {
        key: String,
        typed: String,
    },
    CellOption,
    DivClass,
    Xref {
        typed: String,
    },
    Cite,
    ShortcodePath {
        shortcode: Shortcode,
        typed: String,
    },
    /// A `\command` being typed inside `$…$` / `$$…$$`. `typed` includes the backslash, so
    /// the caller can replace the whole control sequence rather than append to it.
    MathCommand {
        typed: String,
    },
    /// A filesystem path being typed somewhere a path is legal: a path-valued front-matter
    /// key, a markdown link or image target, or a `{{< video >}}` source. `kind` narrows
    /// which files are worth offering.
    Path {
        typed: String,
        kind: PathKind,
    },
    /// `{{< ` then a partial shortcode name.
    ShortcodeName {
        typed: String,
    },
    /// A `#| key:` cell option whose value has a closed set, then the partial value.
    CellOptionValue {
        key: String,
        typed: String,
    },
    /// A ` ```{lang} ` cell's language, being typed.
    CellLanguage {
        typed: String,
    },
    /// A `{#id}` anchor being DEFINED (not referenced), then the partial id. Offers the
    /// cross-reference prefixes, which is the only part of an id that has a vocabulary.
    AnchorId {
        typed: String,
    },
    /// A `{{< input type=` value.
    InputType {
        typed: String,
    },
    /// A `key=` attribute slot in a fenced div's `::: {…}` list, with the classes already
    /// typed on that fence. The classes decide which attributes are offered: `render/divs.rs`
    /// dispatches on class, so `state=` on a callout is a no-op, not an option.
    DivAttrKey {
        classes: Vec<String>,
        typed: String,
    },
}

/// What a path position is for, which decides the extensions worth offering. A path
/// completion that lists every file in the directory is barely better than none: the point
/// is that `bibliography:` shows you the `.bib` files.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum PathKind {
    Bibliography,
    Style,
    Image,
    Html,
    Media,
    /// A markdown link target: another document, or anything else on disk.
    Link,
}

impl PathKind {
    /// The extensions offered, or `&[]` for "any file".
    pub(crate) fn extensions(self) -> &'static [&'static str] {
        match self {
            PathKind::Bibliography => &["bib"],
            PathKind::Style => &["css"],
            PathKind::Image => &["png", "jpg", "jpeg", "gif", "svg", "webp", "avif"],
            PathKind::Html => &["html", "htm"],
            PathKind::Media => &["mp4", "webm", "ogv", "mov", "m4v"],
            PathKind::Link => &[],
        }
    }

    /// The one-word label shown beside a file candidate.
    pub(crate) fn detail(self) -> &'static str {
        match self {
            PathKind::Bibliography => "bibliography",
            PathKind::Style => "stylesheet",
            PathKind::Image => "image",
            PathKind::Html => "HTML partial",
            PathKind::Media => "video",
            PathKind::Link => "file",
        }
    }
}

/// The front-matter keys whose value is a path, and what kind. Sourced from what the
/// renderer actually resolves relative to the page, so a key that stops taking a path stops
/// offering files.
const PATH_KEYS: &[(&str, PathKind)] = &[
    ("bibliography", PathKind::Bibliography),
    ("css", PathKind::Style),
    ("image", PathKind::Image),
    ("logo", PathKind::Image),
    ("image-alt", PathKind::Image), // alt text, but authors reach for the file name first
    ("include-in-header", PathKind::Html),
    ("include-before-body", PathKind::Html),
    ("include-after-body", PathKind::Html),
];

/// The shortcodes offered by name, as `(name, description)`. Mirrors the built-ins
/// `render::extension::render_shortcode` dispatches on plus `include`, which
/// `includes.rs` resolves before expansion.
const SHORTCODE_NAMES: &[(&str, &str)] = &[
    ("include", "Splice another .tmd file in at this point."),
    ("embed", "Embed a deck or page in an iframe."),
    ("video", "Embed a local or remote video."),
    ("input", "A reader-facing control that {js} cells can read."),
    (
        "dataset",
        "A provenance card for a data file: size, checksum, licence, where it came from.",
    ),
];

/// Cell options whose value has a closed set, as `(key, [(value, description)])`.
const CELL_OPTION_VALUES: &[(&str, &[(&str, &str)])] = &[
    (
        "echo",
        &[
            ("true", "Show the cell's source."),
            ("false", "Hide the cell's source."),
        ],
    ),
    (
        "include",
        &[
            ("true", "Include the cell's output."),
            ("false", "Run the cell but show nothing."),
        ],
    ),
    (
        "cache",
        &[
            ("true", "Persist the output in `_freeze/`."),
            ("false", "Never persist this cell's output."),
        ],
    ),
    (
        "code-fold",
        &[
            ("true", "Start collapsed."),
            ("false", "Never collapse."),
            ("show", "Collapsible, but start expanded."),
        ],
    ),
];

/// Classify the completion context at the cursor. `line_prefix` is the current line up to the
/// cursor; `doc_prefix` is the whole document up to the cursor.
///
/// **Order is load-bearing**, because these patterns overlap: a shortcode path wins over
/// `@`/`:::` (a path can contain either), a link target `](` wins over a citation `[@`, and a
/// citation `[@` wins over a cross-reference `@`.
pub(crate) fn detect_context(line_prefix: &str, doc_prefix: &str) -> CompletionContext {
    // Math first: `\` opens no other context, and the guard is `in_math`, not the backslash,
    // so prose is unaffected. Taliesin renders math with KaTeX in-process, which is what
    // makes this list authoritative rather than a guess (see `math_vocab.rs`).
    if let Some(typed) = detect_math_command(line_prefix)
        && in_math(doc_prefix)
    {
        return CompletionContext::MathCommand { typed };
    }
    // A cell's language: ` ```{py `. Before everything else because a fence line cannot be
    // any other context, and `{` would otherwise fall through to the div-class scan.
    if let Some(typed) = detect_cell_language(line_prefix) {
        return CompletionContext::CellLanguage { typed };
    }
    if let Some(typed) = detect_input_type(line_prefix) {
        return CompletionContext::InputType { typed };
    }
    if let Some(c) = detect_shortcode_path(line_prefix) {
        return c;
    }
    // `{{< ` with the name not yet finished. After `detect_shortcode_path`, so a completed
    // `{{< include ` is a path position rather than a name still being typed.
    if let Some(typed) = detect_shortcode_name(line_prefix) {
        return CompletionContext::ShortcodeName { typed };
    }
    // A markdown link/image target, before the `[@cite]` rule: `](` is unambiguous, and a
    // path may legitimately contain an `@`.
    if let Some((typed, kind)) = detect_link_target(line_prefix) {
        return CompletionContext::Path { typed, kind };
    }
    if is_cite_context(line_prefix) {
        return CompletionContext::Cite;
    }
    if let Some(typed) = detect_xref(line_prefix) {
        return CompletionContext::Xref { typed };
    }
    if is_div_class_context(line_prefix) {
        return CompletionContext::DivClass;
    }
    // The other two things a fenced div's attribute slot can hold. `is_div_class_context`
    // above already reaches any slot for a `.class`; an `#id` and a `key=` in a *later* slot
    // had nothing, so `::: {.theorem #thm-` and `::: {.theorem ti` both answered silence.
    if let Some(region) = div_attr_region(line_prefix) {
        let (before, token) = trailing_slot(region);
        if let Some(id) = token.strip_prefix('#') {
            return CompletionContext::AnchorId {
                typed: id.to_string(),
            };
        }
        // A key only once a slot has been opened by whitespace: at `::: {ti` the author is
        // typing a BARE CLASS (`parse_attrs` reads an undotted token as a class), and
        // offering attribute keys there would answer the wrong question.
        if !before.is_empty() && !token.contains('=') && !token.starts_with('.') {
            return CompletionContext::DivAttrKey {
                classes: div_classes_typed(before),
                typed: token.to_string(),
            };
        }
    }
    // `{#` DEFINES an anchor. The `@` completion offers the prefixes for a reference; the
    // definition side had nothing, so the one place an id is invented was the one place the
    // prefix vocabulary was withheld.
    if let Some(typed) = detect_anchor_id(line_prefix) {
        return CompletionContext::AnchorId { typed };
    }
    if in_code_cell(doc_prefix) {
        if is_cell_option_line(line_prefix) {
            return CompletionContext::CellOption;
        }
        if let Some((key, typed)) = cell_option_value(line_prefix) {
            // `label:` is where a cell's cross-reference id is INVENTED, and getting its
            // prefix right is what decides whether the cell becomes a numbered figure at
            // all — so it gets the same prefix vocabulary as `{#`, not a value list.
            if key == "label" {
                return CompletionContext::AnchorId { typed };
            }
            if CELL_OPTION_VALUES.iter().any(|(k, _)| *k == key) {
                return CompletionContext::CellOptionValue { key, typed };
            }
        }
    }
    if in_frontmatter(doc_prefix) {
        if let Some((key, typed)) = frontmatter_value(line_prefix) {
            // A path-valued key offers files, not a word list. `frontmatterValues` only
            // ever had `format` and `theme`, so every other key — including the six that
            // name a file — was detected as a value position and then answered nothing.
            if let Some((_, kind)) = PATH_KEYS.iter().find(|(k, _)| *k == key) {
                return CompletionContext::Path { typed, kind: *kind };
            }
            return CompletionContext::FrontmatterValue { key, typed };
        }
        // A YAML list item under a path-valued key (`bibliography:` then `  - refs.bib`).
        if let Some(typed) = yaml_list_item(line_prefix)
            && let Some(kind) = enclosing_path_key(doc_prefix)
        {
            return CompletionContext::Path { typed, kind };
        }
        if is_frontmatter_key_line(line_prefix) {
            return CompletionContext::FrontmatterKey {
                parent: nested_parent(doc_prefix),
            };
        }
    }
    CompletionContext::None
}

/// A partially-typed math command at the cursor, returned exactly as typed so the caller
/// replaces the whole thing rather than appending to it.
///
/// Two spellings reach the same vocabulary:
///
/// - A **control sequence**: `\` followed by letters. `\` alone counts (typing the backslash
///   should open the list, which is what makes it a useful trigger character); `\\` does not,
///   since that is a row break rather than a command being typed.
/// - A **bare token**, with no backslash at all — the stepless path. Knowing a symbol's name
///   should be enough; remembering that it needs a leading backslash is a step, and the
///   editor already knows the cursor is in math.
///
/// The bare path is the one that can misfire, because single letters are what math is *made*
/// of, so it is withheld in the three places a short name is a name rather than a command:
/// a token of one ASCII character (`$x$`), and a token introduced by `{`, `_` or `^`
/// (`x_{max}`, `a_ij`, `\begin{ali`). A glyph escapes the length rule — `α` is not a
/// variable, and turning it into `\alpha` is exactly what the vocabulary's glyphs are for.
///
/// Everything here is still gated on [`in_math`] by the caller, which is what keeps the bare
/// path from firing on every word in the prose.
fn detect_math_command(line_prefix: &str) -> Option<String> {
    // A glyph belongs to the token for the same reason a letter does: `\α` and a bare `α`
    // are both a way of asking for `\alpha`.
    let is_command_char = |c: char| char::is_ascii_alphabetic(&c) || !c.is_ascii();
    let chars: Vec<char> = line_prefix.chars().collect();
    let mut j = chars.len();
    while j > 0 && is_command_char(chars[j - 1]) {
        j -= 1;
    }
    if j > 0 && chars[j - 1] == '\\' {
        // An even run of backslashes before this one means it is itself escaped, so this is
        // a row break. What follows it is still a bare token in math, so fall through rather
        // than refusing: `\\ al` and `x al` are the same position.
        let mut backslashes = 0;
        let mut k = j - 1;
        while chars[k] == '\\' {
            backslashes += 1;
            if k == 0 {
                break;
            }
            k -= 1;
        }
        if backslashes % 2 == 1 {
            return Some(chars[j - 1..].iter().collect());
        }
    }
    let token: String = chars[j..].iter().collect();
    if token.is_empty() {
        return None;
    }
    // A brace, subscript or superscript introduces a NAME, not a command.
    if j > 0 && matches!(chars[j - 1], '{' | '_' | '^') {
        return None;
    }
    // One ASCII letter is a variable. One glyph is not.
    if token.chars().count() < 2 && token.is_ascii() {
        return None;
    }
    Some(token)
}

/// Is the cursor (the end of `doc_prefix`) inside a math span?
///
/// Scans for `$` delimiters, honoring `\` escapes, skipping fenced code blocks, and resetting
/// inline state at every newline (an inline `$…$` cannot span lines — `render::math_close`
/// gives up at `\n`). `$$` toggles display state, a single `$` toggles inline.
///
/// Unlike the renderer's scanner this must answer for an UNCLOSED span: the author is typing
/// inside math whose closing `$` does not exist yet, so `math_close` (which needs the close)
/// cannot be reused. It carries the renderer's "an opening `$` is not followed by whitespace"
/// guard, which is what keeps `$ 5 ` from opening math; a bare `$5` price still does, and
/// costs at most an offered `\alpha` after a backslash later on that same line.
fn in_math(doc_prefix: &str) -> bool {
    // The cursor sits at the end of `doc_prefix`, so "inside math" is exactly "a span is
    // still open at end-of-input". `scan_math` drops a span abandoned at a line break, so an
    // unclosed span here always means the author is typing inside one.
    crate::lsp_nav::scan_math(doc_prefix)
        .iter()
        .any(|s| !s.closed)
}

/// A ` ```{lang} ` cell language being typed: a fence line whose brace is open.
fn detect_cell_language(line_prefix: &str) -> Option<String> {
    let t = line_prefix.trim_start();
    let rest = t.strip_prefix("```").or_else(|| t.strip_prefix("~~~"))?;
    let inner = rest.strip_prefix('{')?;
    // Still inside the brace, and still on the bare language name.
    if inner.contains('}') || inner.contains(char::is_whitespace) {
        return None;
    }
    Some(inner.to_string())
}

/// A `{{< input type=` value being typed.
fn detect_input_type(line_prefix: &str) -> Option<String> {
    let open = line_prefix.rfind("{{<")?;
    let inner = &line_prefix[open + 3..];
    if inner.contains(">}}") || inner.split_whitespace().next()? != "input" {
        return None;
    }
    let typed = inner.rsplit_once("type=")?.1;
    (!typed.contains(char::is_whitespace)).then(|| typed.to_string())
}

/// A `{#id}` anchor being defined: `{#` then the partial id, before any `}` or space.
///
/// Only where an anchor can actually be attached — a `{#` that follows a `.class` in a
/// fenced-div attribute block is an id there too, so both are accepted; what is excluded is
/// `@#`-style noise and an already-closed brace.
fn detect_anchor_id(line_prefix: &str) -> Option<String> {
    let open = line_prefix.rfind("{#")?;
    let typed = &line_prefix[open + 2..];
    if typed.contains('}') || typed.contains(char::is_whitespace) {
        return None;
    }
    Some(typed.to_string())
}

/// The text between a fenced div's opening `{` and the cursor, or `None` when the cursor is
/// not in an attribute list at all. `::: {.callout-note ti` yields `.callout-note ti`.
///
/// Scanning FORWARD from the fence is what keeps this honest: the class detector walks
/// backwards and has to recognize every token shape it steps over, whereas the region is
/// simply "after the `{`, before the cursor". Two conditions close it:
///
/// - a `}` in the region means the brace closed before the cursor, so the cursor is in
///   prose after the fence, not in the list;
/// - an odd number of unescaped `"` means the cursor is INSIDE a value (`title="a b`),
///   where a list of attribute keys is noise.
fn div_attr_region(line_prefix: &str) -> Option<&str> {
    let trimmed = line_prefix.trim_start();
    // `:` is one byte, so the leading run's char count is also its byte offset.
    let colons = trimmed.chars().take_while(|c| *c == ':').count();
    if colons < 3 {
        return None;
    }
    let region = trimmed[colons..]
        .trim_start_matches([' ', '\t'])
        .strip_prefix('{')?;
    if region.contains('}') {
        return None;
    }
    let mut quotes = 0usize;
    let mut escaped = false;
    for c in region.chars() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => quotes += 1,
            _ => {}
        }
    }
    quotes.is_multiple_of(2).then_some(region)
}

/// Split an attribute region into (everything before the slot being typed, that slot).
/// The slot is the trailing run after the last whitespace, so it is `""` when the cursor
/// sits just after a space — which is a legitimate "offer me everything" position.
fn trailing_slot(region: &str) -> (&str, &str) {
    let idx = region.rfind([' ', '\t']).map(|i| i + 1).unwrap_or(0);
    (&region[..idx], &region[idx..])
}

/// The classes already named on this fence, read with the RENDERER's own tokenizer so a
/// quoted value cannot be mistaken for one: splitting `title="a b"` on whitespace leaves a
/// stray `b"` that looks exactly like a bare class name.
///
/// Both spellings count, because `parse_attrs` accepts both: a dotted `.callout-note` and a
/// bare `columns`.
fn div_classes_typed(before: &str) -> Vec<String> {
    taliesin_core::render::tokenize_attrs(before)
        .into_iter()
        .filter_map(|tok| match tok.strip_prefix('.') {
            Some(c) => (!c.is_empty()).then(|| c.to_string()),
            None => (!tok.starts_with('#') && !tok.contains('=')).then_some(tok),
        })
        .collect()
}

/// The shortcodes offered by name, with their one-line descriptions.
pub(crate) fn shortcode_names() -> &'static [(&'static str, &'static str)] {
    SHORTCODE_NAMES
}

/// The closed value set for a cell option, or `&[]` when it has none.
pub(crate) fn cell_option_values(key: &str) -> &'static [(&'static str, &'static str)] {
    CELL_OPTION_VALUES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .unwrap_or(&[])
}

/// A shortcode name still being typed: the last `{{<` with only word characters after it.
///
/// Returns `None` once a space follows the name, which is what makes this safe to run after
/// `detect_shortcode_path`: by then the name is settled and the cursor is on an argument.
fn detect_shortcode_name(line_prefix: &str) -> Option<String> {
    let chars: Vec<char> = line_prefix.chars().collect();
    let n = chars.len();
    let start = (0..n.saturating_sub(2))
        .rev()
        .find(|&i| chars[i] == '{' && chars[i + 1] == '{' && chars[i + 2] == '<')?;
    let mut j = start + 3;
    while j < n && is_hspace(chars[j]) {
        j += 1;
    }
    let typed: String = chars[j..].iter().collect();
    // A closed `>}}` earlier on the line means this `{{<` is finished, not being typed.
    if typed.chars().any(|c| !c.is_ascii_alphabetic()) {
        return None;
    }
    Some(typed)
}

/// A markdown link or image target being typed: `](` then the path, before the closing `)`.
/// An image (`![alt](`) offers images; a plain link offers anything.
fn detect_link_target(line_prefix: &str) -> Option<(String, PathKind)> {
    let open = line_prefix.rfind("](")?;
    let typed = &line_prefix[open + 2..];
    if typed.contains(')') || typed.contains(' ') {
        return None; // past the target (closed, or into a title)
    }
    // `![` before the label makes it an image. Walk back to the label's opening bracket.
    let label_open = line_prefix[..open].rfind('[')?;
    let is_image = line_prefix[..label_open].ends_with('!');
    Some((
        typed.to_string(),
        if is_image {
            PathKind::Image
        } else {
            PathKind::Link
        },
    ))
}

/// A YAML sequence item being typed (`  - refs.bib`), returning the value after the dash.
fn yaml_list_item(line_prefix: &str) -> Option<String> {
    let t = line_prefix.trim_start();
    let rest = t.strip_prefix('-')?;
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None; // `--` or `-x`: not a sequence item
    }
    let value = rest.trim_start();
    (!value.contains(char::is_whitespace)).then(|| value.to_string())
}

/// The [`PathKind`] of the path-valued front-matter key a list item sits under, scanning
/// back for the nearest less-indented `key:` line.
fn enclosing_path_key(doc_prefix: &str) -> Option<PathKind> {
    let lines: Vec<&str> = doc_prefix.split('\n').collect();
    let current = lines.last()?;
    let indent = current.len() - current.trim_start().len();
    for line in lines.iter().rev().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let line_indent = line.len() - line.trim_start().len();
        if line_indent < indent {
            let key = line.trim().split_once(':')?.0;
            return PATH_KEYS.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
        }
    }
    None
}

/// A `#| key: value` directive with the cursor in the VALUE, as `(key, typed)`. The sibling
/// of [`is_cell_option_line`], which covers the key position.
fn cell_option_value(line_prefix: &str) -> Option<(String, String)> {
    let t = line_prefix.trim_start();
    let rest = ["#|", "//|", "%%|"]
        .iter()
        .find_map(|p| t.strip_prefix(p))?
        .trim_start();
    let (key, value) = rest.split_once(':')?;
    if !key.chars().all(is_id_char) || key.is_empty() {
        return None;
    }
    let typed = value.trim_start();
    (!typed.contains(char::is_whitespace)).then(|| (key.to_string(), typed.to_string()))
}

/// `{{< embed `/`{{< include ` then the first (path) token, while still typing that token.
/// A space or `>` after it means we've moved on to named args, not a path. `/\{\{<\s*(embed|
/// include)\s+([^\s>]*)$/`.
fn detect_shortcode_path(line_prefix: &str) -> Option<CompletionContext> {
    let chars: Vec<char> = line_prefix.chars().collect();
    let n = chars.len();
    // The last `{{<` (only the last one can reach the cursor as an open path token).
    let start = (0..n.saturating_sub(2))
        .rev()
        .find(|&i| chars[i] == '{' && chars[i + 1] == '{' && chars[i + 2] == '<')?;
    let mut j = start + 3;
    while j < n && is_hspace(chars[j]) {
        j += 1;
    }
    let kw_start = j;
    while j < n && chars[j].is_ascii_alphabetic() {
        j += 1;
    }
    let keyword: String = chars[kw_start..j].iter().collect();
    // `video`'s positional source is a path too, but it is a media file, not a `.tmd`, and
    // it has no directory-descent behaviour to preserve — so it routes to the general
    // `Path` context with the media extensions rather than to `ShortcodePath`.
    let shortcode = match keyword.as_str() {
        "embed" => Some(Shortcode::Embed),
        "include" => Some(Shortcode::Include),
        "video" => None,
        _ => return None,
    };
    let ws_start = j;
    while j < n && is_hspace(chars[j]) {
        j += 1;
    }
    if j == ws_start {
        return None; // need whitespace between the keyword and the path
    }
    let typed: String = chars[j..].iter().collect();
    if typed.chars().any(|c| is_hspace(c) || c == '>') {
        return None; // past the path token (into named args / the closer)
    }
    // A `key=value` named argument is not the positional source, so `{{< video poster=x`
    // must not be completed as if `poster=x` were the clip.
    if typed.contains('=') {
        return None;
    }
    Some(match shortcode {
        Some(shortcode) => CompletionContext::ShortcodePath { shortcode, typed },
        None => CompletionContext::Path {
            typed,
            kind: PathKind::Media,
        },
    })
}

/// The cursor is inside an open `[@…` citation: the last `[@` has no `]` before the cursor.
/// `/\[@[^\]]*$/`.
fn is_cite_context(line_prefix: &str) -> bool {
    match line_prefix.rfind("[@") {
        Some(i) => !line_prefix[i + 2..].contains(']'),
        None => false,
    }
}

/// A cross-reference `@id` at the cursor, `@` preceded by start or a non-word/non-`@` char
/// (so an email local-part is skipped). Returns the typed id. `/(^|[^\w@])@([\w-]*)$/`.
fn detect_xref(line_prefix: &str) -> Option<String> {
    let chars: Vec<char> = line_prefix.chars().collect();
    let mut j = chars.len();
    while j > 0 && is_id_char(chars[j - 1]) {
        j -= 1;
    }
    if j == 0 || chars[j - 1] != '@' {
        return None;
    }
    let at = j - 1;
    let prev_ok = at == 0 || {
        let p = chars[at - 1];
        !is_word(p) && p != '@'
    };
    if !prev_ok {
        return None;
    }
    Some(chars[j..].iter().collect())
}

/// A fenced-div class position: `:::{.` / `::: {.` then a partial class name — at **any**
/// attribute slot, not just the first.
///
/// The port's original `/:::\s*\{\.[\w-]*$/` required the `{` immediately before the `.`, so
/// only the opening class completed: in `::: {.theorem .proof` the second `.` saw a space and
/// gave up. `divs.rs` collects classes into a `Vec` and joins them, so a multi-class div is
/// rendered syntax, not a typo — the completion was the only half that stopped at one. So walk
/// back over any already-typed attributes (`.class`, `#id`, and the spaces between) to reach
/// the `{`, then require the `:::`.
fn is_div_class_context(line_prefix: &str) -> bool {
    let chars: Vec<char> = line_prefix.chars().collect();
    let mut j = chars.len();
    while j > 0 && is_id_char(chars[j - 1]) {
        j -= 1;
    }
    if j == 0 || chars[j - 1] != '.' {
        return false;
    }
    let mut k = j - 1; // the `.` opening the class being typed
    loop {
        while k > 0 && is_hspace(chars[k - 1]) {
            k -= 1;
        }
        if k > 0 && chars[k - 1] == '{' {
            k -= 1;
            break;
        }
        // Another attribute token (`.class` / `#id`); no progress means this is not an
        // attribute list at all (e.g. a bare `word .foo` in prose), so give up.
        let end = k;
        while k > 0 && is_id_char(chars[k - 1]) {
            k -= 1;
        }
        if k > 0 && (chars[k - 1] == '.' || chars[k - 1] == '#') {
            k -= 1;
        }
        if k == end {
            return false;
        }
    }
    while k > 0 && is_hspace(chars[k - 1]) {
        k -= 1;
    }
    k >= 3 && chars[k - 1] == ':' && chars[k - 2] == ':' && chars[k - 3] == ':'
}

/// A cell-option directive line in key position: `#|` / `//|` / `%%|` then optional ws then a
/// partial key. `/^\s*(#\||\/\/\||%%\|)\s*[\w-]*$/` (the code-cell guard is applied by the caller).
fn is_cell_option_line(line_prefix: &str) -> bool {
    let t = line_prefix.trim_start();
    let rest = t
        .strip_prefix("#|")
        .or_else(|| t.strip_prefix("//|"))
        .or_else(|| t.strip_prefix("%%|"));
    match rest {
        Some(rest) => rest.trim_start().chars().all(is_id_char),
        None => false,
    }
}

/// An odd number of ``` fences before the current line ⇒ the cursor is inside a code cell.
fn in_code_cell(doc_prefix: &str) -> bool {
    let lines: Vec<&str> = doc_prefix.split('\n').collect();
    let mut fences = 0;
    // Exclude the current (last) line: a `#|` opener is inside the cell it began.
    for line in lines.iter().take(lines.len().saturating_sub(1)) {
        if line.trim_start().starts_with("```") {
            fences += 1;
        }
    }
    fences % 2 == 1
}

/// The cursor is inside the leading `---` front-matter block (opener present, not yet closed
/// before the current line).
fn in_frontmatter(doc_prefix: &str) -> bool {
    let lines: Vec<&str> = doc_prefix.split('\n').collect();
    if lines.first().map(|l| l.trim()) != Some("---") {
        return false;
    }
    for line in lines.iter().take(lines.len().saturating_sub(1)).skip(1) {
        let t = line.trim();
        if t == "---" || t == "..." {
            return false;
        }
    }
    true
}

/// The nearest less-indented ancestor key (a recognized nested parent) above the current line.
fn nested_parent(doc_prefix: &str) -> Option<String> {
    let lines: Vec<&str> = doc_prefix.split('\n').collect();
    let current = lines.last().copied().unwrap_or("");
    let indent = current.len() - current.trim_start().len();
    if indent == 0 {
        return None;
    }
    for i in (0..lines.len().saturating_sub(1)).rev() {
        let line = lines[i];
        if line.trim().is_empty() {
            continue;
        }
        let line_indent = line.len() - line.trim_start().len();
        if line_indent < indent {
            let trimmed = line.trim();
            let key: String = trimmed.chars().take_while(|c| is_id_char(*c)).collect();
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

/// `key:` then the value token being typed. `/^\s*([\w-]+):\s*(\S*)$/`.
fn frontmatter_value(line_prefix: &str) -> Option<(String, String)> {
    let chars: Vec<char> = line_prefix.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n && is_hspace(chars[i]) {
        i += 1;
    }
    let key_start = i;
    while i < n && is_id_char(chars[i]) {
        i += 1;
    }
    if i == key_start || i >= n || chars[i] != ':' {
        return None;
    }
    let key: String = chars[key_start..i].iter().collect();
    i += 1; // past `:`
    while i < n && is_hspace(chars[i]) {
        i += 1;
    }
    let typed: String = chars[i..].iter().collect();
    if typed.chars().any(char::is_whitespace) {
        return None; // `\S*$`: a value token is a single non-whitespace run
    }
    Some((key, typed))
}

/// A front-matter key position: a partial word so far, no colon yet. `/^\s*[\w-]*$/`.
///
/// A key never opens with `-` or `.`, and excluding those is what keeps the **closing**
/// `---` out: it is all `-`, which `is_id_char` admits, so typing the delimiter that ends
/// the front matter used to pop the entire 27-key list. [`in_frontmatter`] cannot catch it,
/// because it deliberately ignores the current line so a cursor *on* a key line still counts
/// as inside. The same exclusion covers the rarer `...` terminator and a YAML `- ` list dash.
fn is_frontmatter_key_line(line_prefix: &str) -> bool {
    let t = line_prefix.trim_start();
    !t.starts_with('-') && !t.starts_with('.') && t.chars().all(is_id_char)
}

/// Harvest `{#id}` anchors (heading ids + figure/table labels) from the buffer, deduplicated
/// and sorted. Suggestion-only; the provider filters by the typed prefix. `/\{#([\w-]+)\}/g`.
pub(crate) fn harvest_anchor_ids(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut seen = std::collections::BTreeSet::new();
    let mut i = 0;
    while i + 1 < n {
        if chars[i] == '{' && chars[i + 1] == '#' {
            let mut j = i + 2;
            while j < n && is_id_char(chars[j]) {
                j += 1;
            }
            if j > i + 2 && j < n && chars[j] == '}' {
                seen.insert(chars[i + 2..j].iter().collect::<String>());
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    seen.into_iter().collect()
}

/// Harvest BibTeX citation keys (`@type{key,`) from a `.bib` file's text, deduplicated and
/// sorted. `/@\w+\s*\{\s*([^,\s}]+)\s*,/g`.
pub(crate) fn harvest_bib_keys(bib: &str) -> Vec<String> {
    let chars: Vec<char> = bib.chars().collect();
    let n = chars.len();
    let mut seen = std::collections::BTreeSet::new();
    let mut i = 0;
    while i < n {
        if chars[i] == '@' {
            let mut j = i + 1;
            let type_start = j;
            while j < n && is_word(chars[j]) {
                j += 1;
            }
            if j > type_start {
                while j < n && chars[j].is_whitespace() {
                    j += 1;
                }
                if j < n && chars[j] == '{' {
                    j += 1;
                    while j < n && chars[j].is_whitespace() {
                        j += 1;
                    }
                    let key_start = j;
                    while j < n && !matches!(chars[j], ',' | '}') && !chars[j].is_whitespace() {
                        j += 1;
                    }
                    if j > key_start {
                        let mut k = j;
                        while k < n && chars[k].is_whitespace() {
                            k += 1;
                        }
                        if k < n && chars[k] == ',' {
                            seen.insert(chars[key_start..j].iter().collect::<String>());
                        }
                    }
                }
            }
        }
        i += 1;
    }
    seen.into_iter().collect()
}

/// One directory entry the caller read from disk (name + whether it is a directory).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// A path completion: the insert value (dirs suffixed `/`) + a short detail label.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PathCandidate {
    pub value: String,
    pub detail: String,
}

/// Candidates for a `{{< embed/include <path> >}}` file argument: the `.tmd` files and
/// descendable subdirs in the directory of `typed`, filtered by its leaf and returned as
/// insert-values relative to the document (dirs suffixed `/` so you can keep descending).
/// `entries` is that directory's listing; `file_detail` labels the `.tmd` hits.
pub(crate) fn shortcode_path_candidates(
    entries: &[DirEntry],
    typed: &str,
    file_detail: &str,
) -> Vec<PathCandidate> {
    let (dir_part, leaf) = match typed.rfind('/') {
        Some(slash) => (&typed[..slash + 1], &typed[slash + 1..]),
        None => ("", typed),
    };
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for e in entries {
        if !e.name.starts_with(leaf) {
            continue;
        }
        // Hide dotfiles unless the user is explicitly typing a dot prefix.
        if e.name.starts_with('.') && !leaf.starts_with('.') {
            continue;
        }
        if e.is_dir {
            if IGNORE_DIRS.contains(&e.name.as_str()) {
                continue;
            }
            dirs.push(PathCandidate {
                value: format!("{dir_part}{}/", e.name),
                detail: "directory".to_string(),
            });
        } else if e.name.ends_with(".tmd") {
            files.push(PathCandidate {
                value: format!("{dir_part}{}", e.name),
                detail: file_detail.to_string(),
            });
        }
    }
    dirs.sort_by(|a, b| a.value.cmp(&b.value));
    files.sort_by(|a, b| a.value.cmp(&b.value));
    dirs.into_iter().chain(files).collect()
}

/// Like [`shortcode_path_candidates`], but for any path position: `exts` are the extensions
/// worth offering (empty = every file). Descendable subdirectories always come first, so a
/// path can be walked down without leaving the menu.
pub(crate) fn path_candidates(
    entries: &[DirEntry],
    typed: &str,
    exts: &[&str],
    file_detail: &str,
) -> Vec<PathCandidate> {
    let (dir_part, leaf) = match typed.rfind('/') {
        Some(slash) => (&typed[..slash + 1], &typed[slash + 1..]),
        None => ("", typed),
    };
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for e in entries {
        if !e.name.starts_with(leaf) {
            continue;
        }
        if e.name.starts_with('.') && !leaf.starts_with('.') {
            continue;
        }
        if e.is_dir {
            if IGNORE_DIRS.contains(&e.name.as_str()) {
                continue;
            }
            dirs.push(PathCandidate {
                value: format!("{dir_part}{}/", e.name),
                detail: "directory".to_string(),
            });
            continue;
        }
        let matches = exts.is_empty()
            || e.name
                .rsplit_once('.')
                .is_some_and(|(_, ext)| exts.iter().any(|w| w.eq_ignore_ascii_case(ext)));
        if matches {
            files.push(PathCandidate {
                value: format!("{dir_part}{}", e.name),
                detail: file_detail.to_string(),
            });
        }
    }
    dirs.sort_by(|a, b| a.value.cmp(&b.value));
    files.sort_by(|a, b| a.value.cmp(&b.value));
    dirs.into_iter().chain(files).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(line: &str, doc: &str) -> CompletionContext {
        detect_context(line, doc)
    }

    fn math(typed: &str) -> CompletionContext {
        CompletionContext::MathCommand {
            typed: typed.to_string(),
        }
    }

    #[test]
    fn detects_a_math_command_inside_inline_math() {
        assert_eq!(
            ctx(r"The $\al", "---\nt: x\n---\n\nThe $\\al"),
            math(r"\al")
        );
    }

    #[test]
    fn a_bare_backslash_inside_math_opens_the_list() {
        // Typing the trigger character must show something; requiring a letter first would
        // make `\` a trigger that opens an empty menu.
        assert_eq!(ctx(r"$\", "---\nt: x\n---\n\n$\\"), math(r"\"));
    }

    #[test]
    fn detects_a_math_command_inside_display_math() {
        let doc = "---\nt: x\n---\n\n$$\n\\begin{ali";
        assert_eq!(ctx(r"\begin{ali", doc), CompletionContext::None);
        // `{` ends the control sequence, so the command being typed is `\begin` already
        // accepted; the partial after it is not a control sequence.
        let doc2 = "---\nt: x\n---\n\n$$\n\\alp";
        assert_eq!(ctx(r"\alp", doc2), math(r"\alp"));
    }

    #[test]
    fn display_math_survives_a_line_break_but_inline_math_does_not() {
        // `$$` opened two lines up: still math.
        assert_eq!(ctx(r"\su", "$$\nx = 1\n\\su"), math(r"\su"));
        // An unclosed inline `$` does not reach the next line.
        assert_eq!(ctx(r"\su", "$x = 1\n\\su"), CompletionContext::None);
    }

    #[test]
    fn a_backslash_in_prose_offers_nothing() {
        assert_eq!(
            ctx(r"a \emph", "---\nt: x\n---\n\na \\emph"),
            CompletionContext::None
        );
    }

    #[test]
    fn a_backslash_in_a_code_cell_offers_nothing() {
        // The `$x$` inside the cell must not open math for the line after it: a fenced cell
        // is code, and a shell/regex backslash there is not a control sequence.
        let line = r"pattern = \d";
        let doc = format!("---\nt: x\n---\n\n```{{python}}\ns = $x$\n{line}");
        assert_eq!(ctx(line, &doc), CompletionContext::None);
    }

    #[test]
    fn closed_math_does_not_leak_into_the_prose_after_it() {
        assert_eq!(
            ctx(r"$x$ and \al", "---\nt: x\n---\n\n$x$ and \\al"),
            CompletionContext::None
        );
    }

    #[test]
    fn an_escaped_dollar_does_not_open_math() {
        assert_eq!(
            ctx(r"\$5 \al", "---\nt: x\n---\n\n\\$5 \\al"),
            CompletionContext::None
        );
    }

    #[test]
    fn an_escaped_backslash_is_a_line_break_not_a_command() {
        // `\\` ends a row in `aligned`; completing after it would offer commands for a
        // backslash the author already finished.
        assert_eq!(ctx(r"$$ x \\", "$$ x \\\\"), CompletionContext::None);
    }

    #[test]
    fn a_bare_word_inside_math_is_a_command_being_typed() {
        // The stepless path: `\` is a step, and the author who knows the symbol's name
        // should not have to know that it needs a backslash first.
        assert_eq!(ctx("$alp", "---\nt: x\n---\n\n$alp"), math("alp"));
    }

    #[test]
    fn a_single_letter_inside_math_is_a_variable_not_a_command() {
        // `$a$`, `$x + y$`: single letters are what math is MADE of. Opening a menu on
        // every one of them is the failure mode that makes stepless completion unusable.
        assert_eq!(ctx("$a", "---\nt: x\n---\n\n$a"), CompletionContext::None);
        assert_eq!(
            ctx("$x + y", "---\nt: x\n---\n\n$x + y"),
            CompletionContext::None
        );
    }

    #[test]
    fn a_bare_word_in_a_brace_is_a_subscript_not_a_command() {
        // `x_{max}`, `a_{ij}`, and `\begin{ali` all put a short NAME after a brace. The
        // backslash path still works there; only the stepless shortcut is withheld.
        assert_eq!(
            ctx("$x_{max", "---\nt: x\n---\n\n$x_{max"),
            CompletionContext::None
        );
        assert_eq!(
            ctx("$x_ij", "---\nt: x\n---\n\n$x_ij"),
            CompletionContext::None
        );
    }

    #[test]
    fn a_bare_word_in_prose_is_just_a_word() {
        // The whole guard is `in_math`. Without it, stepless completion would fire on
        // every word in the document.
        assert_eq!(
            ctx("the alpha channel", "---\nt: x\n---\n\nthe alpha channel"),
            CompletionContext::None
        );
    }

    #[test]
    fn a_glyph_is_a_way_to_ask_for_its_command() {
        // The vocabulary carries the glyph, so the glyph is a legitimate query: an author
        // who can produce `α` should be able to turn it into `\alpha`. One char is enough
        // here (unlike a bare word) because a glyph is not a variable name.
        assert_eq!(ctx("$α", "---\nt: x\n---\n\n$α"), math("α"));
        assert_eq!(ctx(r"$\α", "---\nt: x\n---\n\n$\\α"), math(r"\α"));
    }

    #[test]
    fn a_dollar_followed_by_a_space_is_not_an_opening_delimiter() {
        // Mirrors `render::strip_math_for_slug`'s open guard, which is what keeps a lone
        // currency `$` from swallowing the rest of the line.
        assert_eq!(
            ctx(r"costs $ 5 and \al", "costs $ 5 and \\al"),
            CompletionContext::None
        );
    }

    #[test]
    fn a_path_valued_frontmatter_key_offers_files_not_a_word_list() {
        assert_eq!(
            ctx("bibliography: ref", "---\nbibliography: ref"),
            CompletionContext::Path {
                typed: "ref".to_string(),
                kind: PathKind::Bibliography
            }
        );
        assert_eq!(
            ctx("css: ", "---\ncss: "),
            CompletionContext::Path {
                typed: String::new(),
                kind: PathKind::Style
            }
        );
        // A key with a word list still gets the word list.
        assert_eq!(
            ctx("format: de", "---\nformat: de"),
            CompletionContext::FrontmatterValue {
                key: "format".to_string(),
                typed: "de".to_string()
            }
        );
    }

    #[test]
    fn a_yaml_list_item_inherits_its_parent_keys_path_kind() {
        assert_eq!(
            ctx("  - re", "---\nbibliography:\n  - re"),
            CompletionContext::Path {
                typed: "re".to_string(),
                kind: PathKind::Bibliography
            }
        );
        // Under a key that takes no path, a list item is just a value.
        assert_eq!(
            ctx("  - wr", "---\ncategories:\n  - wr"),
            CompletionContext::None
        );
    }

    #[test]
    fn a_markdown_link_target_completes_and_an_image_narrows_to_images() {
        assert_eq!(
            ctx("See [the notes](no", "---\nt: x\n---\n\nSee [the notes](no"),
            CompletionContext::Path {
                typed: "no".to_string(),
                kind: PathKind::Link
            }
        );
        assert_eq!(
            ctx("![A plot](fig", "---\nt: x\n---\n\n![A plot](fig"),
            CompletionContext::Path {
                typed: "fig".to_string(),
                kind: PathKind::Image
            }
        );
        // Past the closing paren there is no target being typed.
        assert_eq!(
            ctx("[a](b.tmd) and ", "---\nt: x\n---\n\n[a](b.tmd) and "),
            CompletionContext::None
        );
    }

    #[test]
    fn a_video_source_offers_media_files_and_a_named_arg_offers_nothing() {
        assert_eq!(
            ctx("{{< video cl", "{{< video cl"),
            CompletionContext::Path {
                typed: "cl".to_string(),
                kind: PathKind::Media
            }
        );
        assert_eq!(
            ctx("{{< video poster=x", "{{< video poster=x"),
            CompletionContext::None
        );
    }

    #[test]
    fn a_cell_option_value_completes_only_for_options_with_a_closed_set() {
        let doc = |line: &str| format!("---\nt: x\n---\n\n```{{python}}\n{line}");
        assert_eq!(
            ctx("#| echo: tr", &doc("#| echo: tr")),
            CompletionContext::CellOptionValue {
                key: "echo".to_string(),
                typed: "tr".to_string()
            }
        );
        // `label:` invents a cross-reference id, so it gets the prefix vocabulary.
        assert_eq!(
            ctx("#| label: fig-", &doc("#| label: fig-")),
            CompletionContext::AnchorId {
                typed: "fig-".to_string()
            }
        );
        // Outside a cell, a `#|` line is prose.
        assert_eq!(
            ctx("#| echo: tr", "---\nt: x\n---\n\n#| echo: tr"),
            CompletionContext::None
        );
    }

    #[test]
    fn path_candidates_filter_by_extension_and_list_directories_first() {
        let entries = vec![
            DirEntry {
                name: "refs.bib".to_string(),
                is_dir: false,
            },
            DirEntry {
                name: "notes.tmd".to_string(),
                is_dir: false,
            },
            DirEntry {
                name: "bib".to_string(),
                is_dir: true,
            },
            DirEntry {
                name: ".hidden".to_string(),
                is_dir: false,
            },
            DirEntry {
                name: "_freeze".to_string(),
                is_dir: true,
            },
        ];
        let got: Vec<String> = path_candidates(&entries, "", &["bib"], "bibliography")
            .into_iter()
            .map(|c| c.value)
            .collect();
        assert_eq!(got, vec!["bib/", "refs.bib"]);
        // An empty extension list means every file.
        let any: Vec<String> = path_candidates(&entries, "", &[], "file")
            .into_iter()
            .map(|c| c.value)
            .collect();
        assert_eq!(any, vec!["bib/", "notes.tmd", "refs.bib"]);
    }

    #[test]
    fn shortcode_names_and_cell_option_values_are_non_empty_closed_sets() {
        assert!(shortcode_names().iter().any(|(n, _)| *n == "include"));
        assert!(shortcode_names().iter().any(|(n, _)| *n == "video"));
        assert!(cell_option_values("echo").iter().any(|(v, _)| *v == "true"));
        assert!(cell_option_values("label").is_empty());
    }

    #[test]
    fn a_cell_language_completes_only_while_the_brace_is_open() {
        assert_eq!(
            ctx("```{py", "---\nt: x\n---\n\n```{py"),
            CompletionContext::CellLanguage {
                typed: "py".to_string()
            }
        );
        // Closed brace: the language is settled.
        assert_eq!(
            ctx("```{python}", "---\nt: x\n---\n\n```{python}"),
            CompletionContext::None
        );
        // A plain (unbraced) fence is a highlighting hint, not a cell.
        assert_eq!(
            ctx("```py", "---\nt: x\n---\n\n```py"),
            CompletionContext::None
        );
    }

    #[test]
    fn an_anchor_definition_offers_the_prefix_vocabulary() {
        assert_eq!(
            ctx(
                "# Scree plot {#fig-",
                "---\nt: x\n---\n\n# Scree plot {#fig-"
            ),
            CompletionContext::AnchorId {
                typed: "fig-".to_string()
            }
        );
        // A closed attribute block is not an id being typed.
        assert_eq!(
            ctx("# H {#sec-intro} ", "---\nt: x\n---\n\n# H {#sec-intro} "),
            CompletionContext::None
        );
    }

    #[test]
    fn an_input_control_offers_its_types() {
        assert_eq!(
            ctx("{{< input type=sl", "{{< input type=sl"),
            CompletionContext::InputType {
                typed: "sl".to_string()
            }
        );
        // A different shortcode's `type=` is not an input control.
        assert_eq!(
            ctx("{{< video type=x", "{{< video type=x"),
            CompletionContext::None
        );
    }

    #[test]
    fn detects_a_shortcode_path_and_not_a_stray_xref_in_it() {
        assert_eq!(
            ctx("{{< include intro", "{{< include intro"),
            CompletionContext::ShortcodePath {
                shortcode: Shortcode::Include,
                typed: "intro".to_string()
            }
        );
        // A `@` inside a path must not be read as an xref (shortcode is checked first).
        assert_eq!(
            ctx("{{< embed a@b", "{{< embed a@b"),
            CompletionContext::ShortcodePath {
                shortcode: Shortcode::Embed,
                typed: "a@b".to_string()
            }
        );
        // Past the path token (a space after it) → no longer a path context.
        assert_eq!(
            ctx("{{< embed deck.tmd ", "{{< embed deck.tmd "),
            CompletionContext::None
        );
    }

    #[test]
    fn cite_wins_over_xref() {
        assert_eq!(ctx("see [@smi", "see [@smi"), CompletionContext::Cite);
        // A closed bracket is no longer a cite context.
        assert_eq!(
            ctx("see [@smith2020] and ", "see [@smith2020] and "),
            CompletionContext::None
        );
    }

    #[test]
    fn detects_an_xref_but_not_an_email() {
        assert_eq!(
            ctx("as in @fig-", "as in @fig-"),
            CompletionContext::Xref {
                typed: "fig-".to_string()
            }
        );
        assert_eq!(ctx("mail me a@b", "mail me a@b"), CompletionContext::None);
    }

    fn div_attr(classes: &[&str], typed: &str) -> CompletionContext {
        CompletionContext::DivAttrKey {
            classes: classes.iter().map(|c| c.to_string()).collect(),
            typed: typed.to_string(),
        }
    }

    /// An attribute slot in a `::: {…}` list, with the classes carried along — they are what
    /// narrows the offer, since `divs.rs` dispatches on class.
    #[test]
    fn detects_a_div_attribute_key() {
        assert_eq!(
            ctx("::: {.theorem ", "::: {.theorem "),
            div_attr(&["theorem"], "")
        );
        assert_eq!(
            ctx("::: {.callout-note ti", "::: {.callout-note ti"),
            div_attr(&["callout-note"], "ti")
        );
        // A bare (undotted) class is a class to `parse_attrs`, so it must be one here too.
        assert_eq!(
            ctx("::: {columns nc", "::: {columns nc"),
            div_attr(&["columns"], "nc")
        );
        // Indented, and a longer fence: both are the same fence to the renderer.
        assert_eq!(
            ctx("  :::: {.step li", "  :::: {.step li"),
            div_attr(&["step"], "li")
        );
    }

    /// The quoted-value cases, which are where a hand-rolled whitespace split goes wrong.
    #[test]
    fn a_quoted_div_value_neither_ends_the_list_nor_becomes_a_class() {
        // Inside the value: an attribute-key list is noise there.
        assert_eq!(
            ctx(
                "::: {.callout-note title=\"a b",
                "::: {.callout-note title=\"a b"
            ),
            CompletionContext::None
        );
        // Closed again: back to a key slot, and `b"` must NOT have become a class.
        assert_eq!(
            ctx(
                "::: {.callout-note title=\"a b\" ic",
                "::: {.callout-note title=\"a b\" ic"
            ),
            div_attr(&["callout-note"], "ic")
        );
    }

    /// The three positions that must NOT become an attribute key.
    #[test]
    fn a_div_attribute_key_does_not_swallow_its_neighbours() {
        // Still typing the value, not a new key.
        assert_eq!(
            ctx("::: {.step lines=", "::: {.step lines="),
            CompletionContext::None
        );
        // The brace closed: the cursor is in prose on the fence line.
        assert_eq!(
            ctx("::: {.callout-note} and ti", "::: {.callout-note} and ti"),
            CompletionContext::None
        );
        // A bare first token is a CLASS being typed, not a key.
        assert_eq!(ctx("::: {colu", "::: {colu"), CompletionContext::None);
        // Prose that merely contains a brace is not a fence.
        assert_eq!(ctx("see {a ti", "see {a ti"), CompletionContext::None);
    }

    /// An `#id` in a LATER attribute slot. `detect_anchor_id` matches a literal `{#`, so
    /// `::: {.theorem #thm-` used to answer nothing — the same one-slot limit the class
    /// detector was fixed for.
    #[test]
    fn detects_an_anchor_id_in_a_later_div_slot() {
        assert_eq!(
            ctx("::: {.theorem #thm-", "::: {.theorem #thm-"),
            CompletionContext::AnchorId {
                typed: "thm-".to_string()
            }
        );
        // The first slot still works (it always did, via the `{#` path).
        assert_eq!(
            ctx("::: {#thm-", "::: {#thm-"),
            CompletionContext::AnchorId {
                typed: "thm-".to_string()
            }
        );
    }

    #[test]
    fn detects_a_div_class() {
        assert_eq!(
            ctx("::: {.callout-", "::: {.callout-"),
            CompletionContext::DivClass
        );
        assert_eq!(ctx(":::{.col", ":::{.col"), CompletionContext::DivClass);
    }

    // A class is completable at every attribute slot, not just the first: `divs.rs` joins a
    // `Vec` of classes, so `{.a .b}` is rendered syntax. Previously only the opening class
    // completed, because the check demanded `{` immediately before the `.`.
    #[test]
    fn detects_a_div_class_after_earlier_attributes() {
        for line in [
            "::: {.theorem .",
            "::: {.theorem .pro",
            "::: {.theorem #thm-a .",
            ":::{.a .b .c",
        ] {
            assert_eq!(ctx(line, line), CompletionContext::DivClass, "line: {line}");
        }
        // Still not a class list: a bare dotted word in prose, and an attribute brace with
        // no `:::` fence in front of it.
        for line in ["see the .foo", "{.foo", "text {.a .b"] {
            assert_eq!(ctx(line, line), CompletionContext::None, "line: {line}");
        }
    }

    // The CLOSING `---` is the front-matter boundary, not a key position. It is all `-`,
    // which `is_id_char` admits, so typing it used to pop the whole front-matter key list.
    #[test]
    fn the_closing_frontmatter_delimiter_is_not_a_key_position() {
        for closer in ["-", "--", "---", "..."] {
            let doc = format!("---\ntitle: Hi\n{closer}");
            assert_eq!(
                ctx(closer, &doc),
                CompletionContext::None,
                "closer: {closer}"
            );
        }
        // A real key position in the same buffer still completes.
        assert_eq!(
            ctx("dat", "---\ntitle: Hi\ndat"),
            CompletionContext::FrontmatterKey { parent: None }
        );
        // And a blank line inside the block still offers keys.
        assert_eq!(
            ctx("", "---\ntitle: Hi\n"),
            CompletionContext::FrontmatterKey { parent: None }
        );
    }

    #[test]
    fn cell_option_only_inside_a_code_cell() {
        let doc_in = "```{python}\n#| ec";
        assert_eq!(ctx("#| ec", doc_in), CompletionContext::CellOption);
        // Same line, but no open fence above → not a cell option.
        assert_eq!(ctx("#| ec", "#| ec"), CompletionContext::None);
    }

    #[test]
    fn frontmatter_key_value_and_nested_parent() {
        // Key position (bare word, no colon).
        assert_eq!(
            ctx("titl", "---\ntitl"),
            CompletionContext::FrontmatterKey { parent: None }
        );
        // Value position for a closed-set key.
        assert_eq!(
            ctx("format: de", "---\nformat: de"),
            CompletionContext::FrontmatterValue {
                key: "format".to_string(),
                typed: "de".to_string()
            }
        );
        // A key nested under `execute:`.
        assert_eq!(
            ctx("  ec", "---\nexecute:\n  ec"),
            CompletionContext::FrontmatterKey {
                parent: Some("execute".to_string())
            }
        );
        // Outside the front-matter block, a bare word is nothing.
        assert_eq!(ctx("titl", "# Heading\n\ntitl"), CompletionContext::None);
    }

    #[test]
    fn harvest_anchor_ids_finds_brace_anchors_only() {
        assert_eq!(
            harvest_anchor_ids("# A {#sec-a}\n\n![x](i.png){#fig-1}\n\nsee @fig-1"),
            vec!["fig-1".to_string(), "sec-a".to_string()]
        );
        // A `{.theorem #x}` is not a `{#id}` anchor form.
        assert!(harvest_anchor_ids("::: {.theorem #pyth}\n:::").is_empty());
    }

    #[test]
    fn harvest_bib_keys_reads_entry_headers() {
        assert_eq!(
            harvest_bib_keys("@article{smith2020,\n  title={x}\n}\n@book{jones19 ,\n}"),
            vec!["jones19".to_string(), "smith2020".to_string()]
        );
        assert!(harvest_bib_keys("% just a comment\nno entries").is_empty());
    }

    /// Every trigger in `detect_context` is decided from the *end* of the line prefix, so its
    /// edges are "one character before the trigger completes" and "one character past the token".
    /// The 2026-07-27 mutation round found 24 survivors across these deciders because the tests
    /// above only ever pass a prefix sitting comfortably in the middle of a context. This table is
    /// written the other way round: every row is an edge, and the rows that expect `None` are the
    /// ones that matter most, since a mutated boundary shows up as a context appearing too early
    /// or lasting too long.
    ///
    /// Written as an explicit table rather than a mechanical sweep on purpose — an expectation
    /// computed from the prefix would just re-derive the implementation and could not fail.
    #[test]
    fn every_completion_trigger_is_pinned_one_character_either_side_of_its_edge() {
        const FM: &str = "---\n"; // an open front-matter block, for the key/value rows
        const CELL: &str = "```{python}\n"; // an open code cell, for the cell-option rows
        // (document text *before* the current line, the line prefix at the cursor, expected)
        let cases: Vec<(&str, &str, CompletionContext)> = vec![
            // --- shortcode NAME: an unfinished `{{<` is a name being typed, not nothing.
            // Both of these used to be `None`, which is what made `{{<` a dead keystroke:
            // an author had to already know the four shortcode names to type one.
            (
                "",
                "{{<",
                CompletionContext::ShortcodeName {
                    typed: String::new(),
                },
            ),
            (
                "",
                "{{< inc",
                CompletionContext::ShortcodeName {
                    typed: "inc".to_string(),
                },
            ),
            // A finished name (space after it) is a path position, not a name position.
            (
                "",
                "{{< include",
                CompletionContext::ShortcodeName {
                    typed: "include".to_string(),
                },
            ),
            (
                "",
                "{{< include ",
                CompletionContext::ShortcodePath {
                    shortcode: Shortcode::Include,
                    typed: String::new(),
                },
            ),
            (
                "",
                "{{< embed a",
                CompletionContext::ShortcodePath {
                    shortcode: Shortcode::Embed,
                    typed: "a".to_string(),
                },
            ),
            // One space past the path token, and the closer, are both outside it.
            ("", "{{< embed a ", CompletionContext::None),
            ("", "{{< embed a>", CompletionContext::None),
            ("", "{{< inclu x", CompletionContext::None),
            ("", "{{ include x", CompletionContext::None),
            // All three characters of the `{{<` opener are load-bearing and must be checked at
            // their own offsets: a lone `{`, or `{{` with the wrong third character, is not one.
            ("", "{x< include a", CompletionContext::None),
            ("", "{{x include a", CompletionContext::None),
            // A `{` *inside* the path must not be mistaken for the start of a new opener, which
            // is what makes "the last `{{<`" a scan for the whole three-character sequence.
            (
                "",
                "{{< include a{b",
                CompletionContext::ShortcodePath {
                    shortcode: Shortcode::Include,
                    typed: "a{b".to_string(),
                },
            ),
            // Only the *last* `{{<` can still be an open path token.
            (
                "",
                "{{< include a >}} {{< embed b",
                CompletionContext::ShortcodePath {
                    shortcode: Shortcode::Embed,
                    typed: "b".to_string(),
                },
            ),
            // --- citation: open on `[@`, closed by `]`, and the last `[@` wins
            ("", "see [@", CompletionContext::Cite),
            ("", "see [@a]", CompletionContext::None),
            ("", "see [@a] [@b", CompletionContext::Cite),
            // --- xref: a bare `@` is already a context; an email local part never is
            (
                "",
                "@",
                CompletionContext::Xref {
                    typed: String::new(),
                },
            ),
            ("", "a@b", CompletionContext::None),
            // --- div class: needs exactly three colons, then `{.`
            ("", ":::{.", CompletionContext::DivClass),
            ("", "::: {.", CompletionContext::DivClass),
            ("", ":::  {.", CompletionContext::DivClass),
            ("", "::{.", CompletionContext::None),
            ("", ":::{", CompletionContext::None),
            // `{` followed by something that is neither `.` nor an id char: the `.` test and the
            // `{` test are separate rejections, not one.
            ("", ":::{:", CompletionContext::None),
            // Only whitespace before the `{.`: the look-back for the colons must stop at the
            // start of the line rather than walking off it.
            ("", " {.", CompletionContext::None),
            // An indented div opener (a div inside a list item). The three colons are located
            // relative to the `{`, so each of their offsets has to be counted, not divided.
            ("", "     :::{.", CompletionContext::DivClass),
            // --- cell option: only inside an open fence, and only in key position
            (CELL, "#|", CompletionContext::CellOption),
            (CELL, "//| ec", CompletionContext::CellOption),
            (CELL, "%%| ec", CompletionContext::CellOption),
            // Past the key, into the value: no longer a cell-option key position.
            (CELL, "#| ec: 1", CompletionContext::None),
            // The same line with no open fence above it is not a cell option at all.
            ("", "#| ec", CompletionContext::None),
            // --- front matter: the block must be open, and `key:` splits key from value
            (FM, "", CompletionContext::FrontmatterKey { parent: None }),
            (
                FM,
                "format:",
                CompletionContext::FrontmatterValue {
                    key: "format".to_string(),
                    typed: String::new(),
                },
            ),
            // A value is a single token: a space inside it means we are past it.
            (FM, "format: a b", CompletionContext::None),
            (FM, ":", CompletionContext::None),
            (
                "---\nexecute:\n",
                "  echo",
                CompletionContext::FrontmatterKey {
                    parent: Some("execute".to_string()),
                },
            ),
            // A *sibling* key at the same indent sits between the cursor and the parent. The
            // look-back has to step over it and keep going: stopping at an equal indent, or
            // mis-measuring either indent, would report `echo` as the parent instead of `execute`.
            (
                "---\nexecute:\n  echo: true\n",
                "  ca",
                CompletionContext::FrontmatterKey {
                    parent: Some("execute".to_string()),
                },
            ),
            // An indented key in *value* position: the leading whitespace must be skipped before
            // the key is read, or an indented `key:` looks like no key at all.
            (
                "---\nexecute:\n",
                "  echo: tru",
                CompletionContext::FrontmatterValue {
                    key: "echo".to_string(),
                    typed: "tru".to_string(),
                },
            ),
            // A blank line between the parent and the child must not break the look-back.
            (
                "---\nexecute:\n\n",
                "  echo",
                CompletionContext::FrontmatterKey {
                    parent: Some("execute".to_string()),
                },
            ),
            // Indented under a key that is not a recognized nested parent.
            (
                "---\ntitle: x\n",
                "  echo",
                CompletionContext::FrontmatterKey { parent: None },
            ),
            // Below the closing fence the block is shut, so a bare word is prose.
            ("---\ntitle: x\n---\n", "titl", CompletionContext::None),
        ];

        for (before, line_prefix, want) in &cases {
            let doc_prefix = format!("{before}{line_prefix}");
            assert_eq!(
                &detect_context(line_prefix, &doc_prefix),
                want,
                "line prefix {line_prefix:?} (doc {doc_prefix:?})"
            );
        }
    }

    /// `harvest_bib_keys` scans a whole `.bib` with a hand-rolled cursor, and 20 of its boundary
    /// mutants survived: nothing fed it an entry that ends at EOF, an empty key, or whitespace in
    /// the places BibTeX allows it. Each row below is one of those shapes.
    #[test]
    fn harvest_bib_keys_is_pinned_at_the_shapes_a_real_bib_contains() {
        // Whitespace is legal between the type, the brace, the key and the comma.
        assert_eq!(
            harvest_bib_keys("@article {k1 ,\n}"),
            vec!["k1".to_string()]
        );
        assert_eq!(
            harvest_bib_keys("@article{\n  k1,\n}"),
            vec!["k1".to_string()]
        );
        // A key must be followed by a comma to be an entry header.
        assert!(harvest_bib_keys("@article{k1}").is_empty());
        // Truncated at EOF, mid-key: the scan must stop at the end, not read past it.
        assert!(harvest_bib_keys("@article{k1").is_empty());
        assert!(harvest_bib_keys("@article{").is_empty());
        // Truncated with no brace at all: the type scan and the whitespace skip after it must
        // both stop at the end of the buffer.
        assert!(harvest_bib_keys("@article").is_empty());
        assert!(harvest_bib_keys("@article ").is_empty());
        // A `@type` *not* followed by `{` is not an entry header, however entry-shaped the rest
        // of the line looks.
        assert!(harvest_bib_keys("@article xyz,").is_empty());
        // `@` with no entry type, and an entry with no key.
        assert!(harvest_bib_keys("@{k1,}").is_empty());
        assert!(harvest_bib_keys("@article{,x}").is_empty());
        // A bare `@` in prose is not an entry.
        assert!(harvest_bib_keys("mail a@b.com").is_empty());
        // Deduplicated and sorted.
        assert_eq!(
            harvest_bib_keys("@a{dup,}\n@b{dup,}\n@c{alpha,}"),
            vec!["alpha".to_string(), "dup".to_string()]
        );
    }

    /// Same story for `harvest_anchor_ids` (10 survivors): the fixtures above never put an anchor
    /// at offset 0, never put two of them back to back, and never truncated one at EOF.
    #[test]
    fn harvest_anchor_ids_is_pinned_at_the_text_edges() {
        // At the very start, and as the entire text.
        assert_eq!(harvest_anchor_ids("{#a}"), vec!["a".to_string()]);
        // Back to back: the scan must resume after the `}`, not inside it.
        assert_eq!(
            harvest_anchor_ids("{#a}{#b}"),
            vec!["a".to_string(), "b".to_string()]
        );
        // Empty id, truncated at EOF, and a space where an id char must be.
        assert!(harvest_anchor_ids("{#}").is_empty());
        assert!(harvest_anchor_ids("x{#a").is_empty());
        assert!(harvest_anchor_ids("{# a}").is_empty());
        assert!(harvest_anchor_ids("{#a b}").is_empty());
        // A trailing `{` as the final character: the scan must not look at the character after it.
        assert_eq!(harvest_anchor_ids("{#a}{"), vec!["a".to_string()]);
        // Deduplicated.
        assert_eq!(harvest_anchor_ids("{#a}\n{#a}"), vec!["a".to_string()]);
    }

    #[test]
    fn shortcode_path_candidates_sorts_dirs_then_tmd_files() {
        let entries = vec![
            DirEntry {
                name: "intro.tmd".to_string(),
                is_dir: false,
            },
            DirEntry {
                name: "chapters".to_string(),
                is_dir: true,
            },
            DirEntry {
                name: "target".to_string(),
                is_dir: true,
            }, // ignored
            DirEntry {
                name: ".hidden".to_string(),
                is_dir: false,
            },
            DirEntry {
                name: "notes.txt".to_string(),
                is_dir: false,
            }, // non-.tmd
        ];
        let got = shortcode_path_candidates(&entries, "", "partial");
        assert_eq!(
            got,
            vec![
                PathCandidate {
                    value: "chapters/".to_string(),
                    detail: "directory".to_string()
                },
                PathCandidate {
                    value: "intro.tmd".to_string(),
                    detail: "partial".to_string()
                },
            ]
        );
        // A dir prefix in `typed` is preserved on the insert value.
        let got2 = shortcode_path_candidates(
            &[DirEntry {
                name: "a.tmd".to_string(),
                is_dir: false,
            }],
            "sub/a",
            "partial",
        );
        assert_eq!(got2[0].value, "sub/a.tmd");

        // The dotfile policy, in both directions. The `.hidden` entry above cannot show it: a
        // non-`.tmd` file never reaches the output whether the dot filter ran or not, so deleting
        // the filter's negation changed nothing observable and the mutant survived. A dotfile that
        // *is* a `.tmd` is what makes the rule testable.
        let dotted = vec![
            DirEntry {
                name: ".draft.tmd".to_string(),
                is_dir: false,
            },
            DirEntry {
                name: "intro.tmd".to_string(),
                is_dir: false,
            },
        ];
        let values = |typed: &str| -> Vec<String> {
            shortcode_path_candidates(&dotted, typed, "partial")
                .into_iter()
                .map(|c| c.value)
                .collect()
        };
        // Not typing a dot: the dotfile is hidden.
        assert_eq!(values(""), vec!["intro.tmd".to_string()]);
        // Explicitly typing a dot: the dotfile is offered.
        assert_eq!(values(".d"), vec![".draft.tmd".to_string()]);
    }
}
