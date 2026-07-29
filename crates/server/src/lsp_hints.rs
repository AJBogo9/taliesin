//! Inlay hints: the resolved number beside a cross-reference.

use lsp_types::{InlayHint, InlayHintLabel, Position, Range};

/// One inlay hint per cross-reference in `range` whose anchor this document numbers.
///
/// `xref_numbers` is page-local, so a reference to an anchor defined in another chapter
/// has no entry here. Such a reference is *valid* (the diagnostic path knows the project's
/// anchors) but unnumbered, and we omit the hint rather than render a placeholder: a
/// missing hint reads as "no information", `⟨elsewhere⟩` reads as a claim.
pub(crate) fn inlay_hints(
    text: &str,
    doc: &taliesin_core::RenderedDoc,
    range: Range,
    dir: Option<&std::path::Path>,
) -> Vec<InlayHint> {
    let mut out = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let line_no = line_no as u32;
        if line_no < range.start.line || line_no > range.end.line {
            continue;
        }
        for (id, end_char) in targets_on_line(line, Kind::Xref) {
            let Some(number) = doc.xref_numbers.get(&id) else {
                continue;
            };
            out.push(hint(line, line_no, end_char, number));
        }
        // The rest resolve against files on disk, so they need the document's directory.
        let Some(dir) = dir else { continue };
        for (key, end_char) in targets_on_line(line, Kind::Cite) {
            if let Some(label) = author_year(text, dir, &key) {
                out.push(hint(line, line_no, end_char, &label));
            }
        }
        for (path, end_char) in targets_on_line(line, Kind::Include) {
            if let Some(label) = include_size(dir, &path) {
                out.push(hint(line, line_no, end_char, &label));
            }
        }
    }
    out
}

/// "Bishop 2006" for a key defined in one of the front matter's `.bib` files, or `None`
/// when nothing defines it. An absent entry is not an error here: the diagnostic pass
/// already reports an unresolvable citation, and a second report is a double report.
fn author_year(text: &str, dir: &std::path::Path, key: &str) -> Option<String> {
    for rel in crate::lsp_nav::frontmatter_bib_paths(text) {
        let Ok(bib) = std::fs::read_to_string(dir.join(&rel)) else {
            continue;
        };
        if let Some(entry) = crate::lsp_nav::bib_entry_text(&bib, key) {
            let authors = bib_field(&entry, "author")?;
            let year = bib_field(&entry, "year")?;
            let mut names = authors.split(" and ");
            let surname = surname_of(names.next()?);
            let more = names.next().is_some();
            return Some(match more {
                true => format!("{surname} et al. {year}"),
                false => format!("{surname} {year}"),
            });
        }
    }
    None
}

/// The surname out of one BibTeX name. The format writes either `von Last, First` or
/// `First von Last`, and the surname is on the opposite side in each — taking the first
/// token unconditionally renders "Christopher 2006" for the second form, which is a
/// confidently wrong hint and worse than no hint at all.
fn surname_of(name: &str) -> String {
    match name.split_once(',') {
        Some((last, _)) => last.trim().to_owned(),
        None => name
            .split_whitespace()
            .next_back()
            .unwrap_or(name)
            .to_owned(),
    }
}

/// `"3 lines"` for an include that resolves, `None` for one that does not. A path that
/// resolves to nothing has no size to report, and `check` already reports the broken
/// include.
fn include_size(dir: &std::path::Path, rel: &str) -> Option<String> {
    let body = std::fs::read_to_string(dir.join(rel)).ok()?;
    let n = body.lines().count();
    Some(match n {
        1 => "1 line".to_owned(),
        n => format!("{n} lines"),
    })
}

/// The value of field `name` in one BibTeX entry, brace-balanced.
///
/// The name is matched only where a field name can *start* — after the entry's opening `{`
/// or a `,` separator, at the entry's own brace depth — so a `title = {…the year of magical
/// thinking}` cannot answer a request for `year`. The value is brace-balanced for the same
/// reason [`crate::lsp_nav::bib_entry_text`] is: `author = {Bishop, {Christopher} M}` would
/// otherwise be cut short at the inner brace.
fn bib_field(entry: &str, name: &str) -> Option<String> {
    let cs: Vec<char> = entry.chars().collect();
    let mut depth = 0usize;
    let mut at_field_start = false;
    let mut i = 0usize;
    while i < cs.len() {
        match cs[i] {
            '{' => {
                depth += 1;
                at_field_start = depth == 1;
                i += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                at_field_start = false;
                i += 1;
            }
            ',' if depth == 1 => {
                at_field_start = true;
                i += 1;
            }
            c if c.is_whitespace() => i += 1,
            _ if !at_field_start || depth != 1 => {
                at_field_start = false;
                i += 1;
            }
            _ => {
                at_field_start = false;
                let start = i;
                while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_' || cs[i] == '-') {
                    i += 1;
                }
                let ident: String = cs[start..i].iter().collect();
                while i < cs.len() && cs[i].is_whitespace() {
                    i += 1;
                }
                // BibTeX field names are case-insensitive.
                if i < cs.len() && cs[i] == '=' && ident.eq_ignore_ascii_case(name) {
                    i += 1;
                    while i < cs.len() && cs[i].is_whitespace() {
                        i += 1;
                    }
                    return Some(bib_value(&cs, i));
                }
            }
        }
    }
    None
}

/// One field value at `i`: `{…}` (brace-balanced), `"…"`, or a bare token.
fn bib_value(cs: &[char], mut i: usize) -> String {
    let collect =
        |from: usize, to: usize| cs[from..to].iter().collect::<String>().trim().to_owned();
    match cs.get(i) {
        Some('{') => {
            let start = i + 1;
            let mut depth = 0usize;
            while i < cs.len() {
                match cs[i] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return collect(start, i);
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            // Unbalanced .bib: return what there is rather than nothing.
            collect(start, cs.len())
        }
        Some('"') => {
            let start = i + 1;
            i += 1;
            while i < cs.len() && cs[i] != '"' {
                i += 1;
            }
            collect(start, i)
        }
        _ => {
            let start = i;
            while i < cs.len() && cs[i] != ',' && cs[i] != '}' {
                i += 1;
            }
            collect(start, i)
        }
    }
}

/// An inlay hint reading `⟨label⟩`, positioned just past the construct it annotates.
fn hint(line: &str, line_no: u32, end_char: usize, label: &str) -> InlayHint {
    InlayHint {
        // The wire wants UTF-16; the scan works in scalar offsets.
        position: Position::new(
            line_no,
            crate::lsp_pos::char_to_utf16(line, end_char) as u32,
        ),
        label: InlayHintLabel::String(format!(" ⟨{label}⟩")),
        kind: None,
        text_edits: None,
        tooltip: None,
        padding_left: None,
        padding_right: None,
        data: None,
    }
}

/// Which annotatable construct a line scan is looking for.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Xref,
    Cite,
    Include,
}

/// Every target of `kind` on one line, as (payload, scalar offset just past it). Reuses
/// `lsp_nav`'s cursor classifier rather than adding a second scanner: walking the line and
/// asking "what is here" keeps one definition of what each construct is.
///
/// De-duplicated by END offset, not by payload: the classifier answers the same span at
/// every column the token covers, but the *same id twice on one line* is two references and
/// must get two hints.
fn targets_on_line(line: &str, kind: Kind) -> Vec<(String, usize)> {
    use crate::lsp_nav::Target;
    let mut out: Vec<(String, usize)> = Vec::new();
    for col in 0..line.chars().count() {
        let found = match (kind, crate::lsp_nav::classify_target(line, 0, col)) {
            (Kind::Xref, Target::Xref { id, end, .. }) => Some((id, end)),
            (Kind::Cite, Target::Cite { key, end, .. }) => Some((key, end)),
            (Kind::Include, Target::Include { path, end, .. }) => Some((path, end)),
            _ => None,
        };
        if let Some((payload, end)) = found
            && out.last().map(|&(_, prev)| prev != end).unwrap_or(true)
        {
            out.push((payload, end));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use lsp_types::{Position, Range};

    fn render(text: &str) -> taliesin_core::RenderedDoc {
        taliesin_core::render_single_doc(text, std::path::Path::new("."))
    }

    const DOC: &str = "\
# Results {#sec-results}

![A curve](a.png){#fig-results}

See @fig-results and @sec-results and @fig-nowhere.
";

    fn hints_on_last_line(text: &str) -> Vec<String> {
        let doc = render(text);
        let last = text.lines().count() as u32 - 1;
        super::inlay_hints(
            text,
            &doc,
            Range::new(Position::new(0, 0), Position::new(last, 0)),
            None,
        )
        .into_iter()
        .map(|h| match h.label {
            lsp_types::InlayHintLabel::String(s) => s,
            other => panic!("expected a string label, got {other:?}"),
        })
        .collect()
    }

    #[test]
    fn a_resolving_xref_gets_its_number() {
        let labels = hints_on_last_line(DOC);
        assert!(
            labels.iter().any(|l| l.contains('1')),
            "expected a number hint for @fig-results, got {labels:?}"
        );
    }

    #[test]
    fn an_unresolvable_xref_gets_no_hint() {
        let labels = hints_on_last_line(DOC);
        assert!(
            !labels.iter().any(|l| l.contains("nowhere")),
            "an anchor this document does not define must produce no hint, got {labels:?}"
        );
    }

    // The classifier answers the same span at every column the id covers, so the walk has to
    // de-duplicate — but by SPAN, not by id. The same anchor cited twice on one line is two
    // references and earns two hints; de-duplicating by id would silently drop the second.
    #[test]
    fn the_same_anchor_twice_on_one_line_gets_two_hints() {
        let text = "![A curve](a.png){#fig-x}\n\nSee @fig-x and again @fig-x.\n";
        let doc = render(text);
        let hints = super::inlay_hints(
            text,
            &doc,
            Range::new(Position::new(2, 0), Position::new(2, 0)),
            None,
        );
        assert_eq!(
            hints.len(),
            2,
            "expected one hint per reference, got {hints:?}"
        );
        assert_ne!(
            hints[0].position, hints[1].position,
            "two references must be annotated at their own positions"
        );
    }

    #[test]
    fn hints_outside_the_requested_range_are_not_returned() {
        let doc = render(DOC);
        // Range covering only line 0, which holds no reference.
        let hints = super::inlay_hints(
            DOC,
            &doc,
            Range::new(Position::new(0, 0), Position::new(0, 0)),
            None,
        );
        assert!(
            hints.is_empty(),
            "expected no hints outside the range, got {hints:?}"
        );
    }

    /// A scratch directory holding a `refs.bib`, following the pattern the existing
    /// cross-file LSP tests use: `tempfile` is not a dependency here. Keyed on the test name
    /// as well as the pid so tests that run in parallel cannot share a directory.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tali-hints-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn bib_dir(name: &str, bib: &str) -> std::path::PathBuf {
        let dir = scratch(name);
        std::fs::write(dir.join("refs.bib"), bib).unwrap();
        dir
    }

    fn labels_of(hints: Vec<lsp_types::InlayHint>) -> Vec<String> {
        hints
            .into_iter()
            .map(|h| match h.label {
                lsp_types::InlayHintLabel::String(s) => s,
                other => panic!("expected a string label, got {other:?}"),
            })
            .collect()
    }

    fn hints_in(text: &str, dir: &std::path::Path) -> Vec<lsp_types::InlayHint> {
        let doc = render(text);
        let last = text.lines().count() as u32;
        super::inlay_hints(
            text,
            &doc,
            Range::new(Position::new(0, 0), Position::new(last, 0)),
            Some(dir),
        )
    }

    #[test]
    fn a_citation_shows_author_and_year() {
        let dir = bib_dir(
            "authoryear",
            "@book{bishop2006pattern,\n  author = {Bishop, Christopher M},\n  year = {2006},\n}\n",
        );
        let text = "---\nbibliography: refs.bib\n---\n\nSee [@bishop2006pattern].\n";
        let labels = labels_of(hints_in(text, &dir));
        assert!(
            labels
                .iter()
                .any(|l| l.contains("Bishop") && l.contains("2006")),
            "expected an author-year hint, got {labels:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_citation_with_no_bib_entry_gets_no_hint() {
        let dir = bib_dir("nobibentry", "@book{other,\n}\n");
        let text = "---\nbibliography: refs.bib\n---\n\nSee [@nosuchkey].\n";
        let hints = hints_in(text, &dir);
        assert!(
            hints.is_empty(),
            "an absent entry must produce no hint, got {hints:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // BibTeX writes a name either as "von Last, First" or as "First von Last", and the
    // surname is on the opposite side in each. Taking the first token unconditionally would
    // render "Christopher 2006" for the second form — a confidently wrong hint, which is
    // worse than none.
    #[test]
    fn the_surname_comes_from_the_right_half_of_either_name_form() {
        let dir = bib_dir(
            "nameforms",
            "@book{comma,\n  author = {Bishop, Christopher M},\n  year = {2006},\n}\n\
             @book{nocomma,\n  author = {Christopher M Bishop},\n  year = {2007},\n}\n\
             @book{several,\n  author = {Bishop, C and Murphy, K},\n  year = {2012},\n}\n",
        );
        let text = "---\nbibliography: refs.bib\n---\n\n[@comma] [@nocomma] [@several]\n";
        let labels = labels_of(hints_in(text, &dir));
        assert_eq!(
            labels,
            vec![" ⟨Bishop 2006⟩", " ⟨Bishop 2007⟩", " ⟨Bishop et al. 2012⟩"],
            "surname, then year; a second author becomes `et al.`"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A field name is only a field name where one can start. A `title` containing the words
    // "author" and "year" would otherwise be found first by a substring search, which then
    // reads whatever brace follows — here the NEXT field's value, so the year would render as
    // the author's name. Both words are lowercase on purpose: with `Year` capitalised the
    // naive implementation passes this test while still being wrong.
    #[test]
    fn a_field_name_inside_another_fields_value_is_not_mistaken_for_it() {
        let dir = bib_dir(
            "fieldnames",
            "@book{k,\n  title = {On the author and the year of magical thinking},\n  \
             author = {Didion, Joan},\n  year = {2005},\n}\n",
        );
        let text = "---\nbibliography: refs.bib\n---\n\n[@k]\n";
        assert_eq!(labels_of(hints_in(text, &dir)), vec![" ⟨Didion 2005⟩"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_include_shows_the_line_count_of_what_it_pulls_in() {
        let dir = scratch("include");
        std::fs::write(dir.join("part.tmd"), "# A\n\nb\n").unwrap();
        let text = "{{< include part.tmd >}}\n";
        let labels = labels_of(hints_in(text, &dir));
        assert_eq!(labels, vec![" ⟨3 lines⟩"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_include_of_a_missing_file_gets_no_hint() {
        let dir = scratch("noinclude");
        let text = "{{< include absent.tmd >}}\n";
        let hints = hints_in(text, &dir);
        assert!(
            hints.is_empty(),
            "a path that resolves to nothing has no line count to report; check already \
             reports the broken include: {hints:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_malformed_buffer_yields_no_hints_and_does_not_panic() {
        // Unterminated fenced div and unterminated display math: the normal half-typed case.
        let text = "::: {.callout}\n$$\n\\frac{1}{";
        let doc = render(text);
        let _ = super::inlay_hints(
            text,
            &doc,
            Range::new(Position::new(0, 0), Position::new(2, 0)),
            None,
        );
    }
}
