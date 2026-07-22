//! Pure, LSP-free navigation helpers for `.tmd`: classify the token under the cursor and
//! resolve xref/cite/include definitions. Hand-rolled scanning (no `regex` dependency; the
//! tokens are simple), a Rust port of the companion's `hover.ts` (`classifyHover`,
//! `definitionSite`, `bibEntryOffset`) + `complete.ts` (`frontmatterBibPaths`), so the
//! `lsp` server can answer go-to-definition for any editor.
//!
//! Offsets are scalar (`char`) based, matching the diagnostics slice's `to_lsp`; the `lsp`
//! server converts them to/from the wire's UTF-16 columns at its boundary (`lsp_pos`), so
//! parity with the UTF-16 companion holds for all text, astral characters included.

/// Front-matter parents whose immediate children have their own vocabulary (mirrors
/// `hover.ts`/`complete.ts`).
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
        '#' => true,
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
}
