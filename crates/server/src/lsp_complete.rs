//! Pure completion-context detection + live-candidate harvest for `.tmd`: decide WHICH
//! vocabulary applies at the cursor, and gather the document-defined ids / `.bib` keys /
//! sibling files that the static vocabulary can't know. A Rust port of the companion's
//! `complete.ts` (`detectContext` + the harvest helpers + `shortcodePathCandidates`),
//! hand-rolled (no `regex` dependency; each pattern is anchored at the cursor or the line
//! start, so a short scan replaces it), so the `lsp` server can answer completion for any
//! editor. The static vocabulary stays Rust-authoritative (`taliesin_core::vocab`); this
//! module only routes to it and harvests suggestion-only ids (`check` remains the arbiter).

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
    FrontmatterKey { parent: Option<String> },
    FrontmatterValue { key: String, typed: String },
    CellOption,
    DivClass,
    Xref { typed: String },
    Cite,
    ShortcodePath { shortcode: Shortcode, typed: String },
}

/// Classify the completion context at the cursor. `line_prefix` is the current line up to the
/// cursor; `doc_prefix` is the whole document up to the cursor. The order mirrors
/// `complete.ts`: a shortcode path wins over `@`/`:::` (a path can contain `@`); a citation
/// `[@` wins over an xref `@`.
pub(crate) fn detect_context(line_prefix: &str, doc_prefix: &str) -> CompletionContext {
    if let Some(c) = detect_shortcode_path(line_prefix) {
        return c;
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
    if is_cell_option_line(line_prefix) && in_code_cell(doc_prefix) {
        return CompletionContext::CellOption;
    }
    if in_frontmatter(doc_prefix) {
        if let Some((key, typed)) = frontmatter_value(line_prefix) {
            return CompletionContext::FrontmatterValue { key, typed };
        }
        if is_frontmatter_key_line(line_prefix) {
            return CompletionContext::FrontmatterKey {
                parent: nested_parent(doc_prefix),
            };
        }
    }
    CompletionContext::None
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
    let shortcode = match chars[kw_start..j].iter().collect::<String>().as_str() {
        "embed" => Shortcode::Embed,
        "include" => Shortcode::Include,
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
    Some(CompletionContext::ShortcodePath { shortcode, typed })
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

/// A fenced-div class position: `:::{.` / `::: {.` then a partial class name.
/// `/:::\s*\{\.[\w-]*$/`.
fn is_div_class_context(line_prefix: &str) -> bool {
    let chars: Vec<char> = line_prefix.chars().collect();
    let mut j = chars.len();
    while j > 0 && is_id_char(chars[j - 1]) {
        j -= 1;
    }
    if j < 2 || chars[j - 1] != '.' || chars[j - 2] != '{' {
        return false;
    }
    let mut k = j - 2; // the `{`
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
fn is_frontmatter_key_line(line_prefix: &str) -> bool {
    line_prefix.trim_start().chars().all(is_id_char)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(line: &str, doc: &str) -> CompletionContext {
        detect_context(line, doc)
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

    #[test]
    fn detects_a_div_class() {
        assert_eq!(
            ctx("::: {.callout-", "::: {.callout-"),
            CompletionContext::DivClass
        );
        assert_eq!(ctx(":::{.col", ":::{.col"), CompletionContext::DivClass);
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
            // --- shortcode path: the keyword needs a separating space, and the path ends at ws/`>`
            ("", "{{<", CompletionContext::None),
            ("", "{{< include", CompletionContext::None),
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
