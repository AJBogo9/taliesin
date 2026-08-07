//! `textDocument/references`: every use of the cross-reference anchor under the cursor,
//! across the whole project.
//!
//! **Why this is its own module rather than another arm in `lsp.rs`.** The answer already
//! existed — `taliesin/projectRefs` computes it — but on a *proprietary* method only the VS
//! Code companion knows to call. `documentHighlight` is the standard method that comes
//! closest and it stops at the file boundary, which in a 25-chapter book is the boundary the
//! question is asked across. This publishes the same answer on the method every LSP client
//! already has a keybinding for.
//!
//! **The open buffer wins for its own file.** [`crate::lsp_project::ProjectCache`] reads pages
//! from disk, so a reference the author has typed and not saved is absent from the walk and a
//! reference they just deleted is still in it. Both are wrong in the direction that matters
//! (the file on screen), so this file's rows come from the live buffer and the walk supplies
//! only the *other* pages. Same rule as `resolve_definition`, for the same reason.

use std::collections::HashMap;
use std::path::PathBuf;

/// Every location that mentions the anchor under the cursor, in project reading order
/// (path, then line). `None` when the cursor is not on a cross-reference anchor at all —
/// which the editor renders as "no references found" rather than as an error.
///
/// `include_declaration` is the client's, not ours: VS Code's "Go to References" sets it
/// false and its "Find All References" view sets it true, and answering the same list for
/// both would put the definition in a list the author asked to exclude.
pub(crate) fn references(
    docs: &HashMap<lsp_types::Url, String>,
    project: &mut crate::lsp_project::ProjectCache,
    params: &lsp_types::ReferenceParams,
) -> Option<Vec<lsp_types::Location>> {
    let uri = &params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let text = docs.get(uri)?;
    let cursor_char = crate::lsp_pos::utf16_to_char(
        crate::lsp_pos::nth_line(text, pos.line as usize),
        pos.character as usize,
    );
    let (id, _, _) = crate::lsp_nav::anchor_at(text, pos.line as usize, cursor_char)?;
    let include_declaration = params.context.include_declaration;

    // This file, from the buffer. `anchor_occurrences` returns definitions *and* references,
    // so the definition site is what the `include_declaration` filter drops.
    let here = uri.to_file_path().ok();
    let definition = crate::lsp_nav::definition_site(text, &id);
    let mut out: Vec<lsp_types::Location> = crate::lsp_nav::anchor_occurrences(text, &id)
        .into_iter()
        .filter(|(l, s, _)| include_declaration || definition != Some((*l, *s)))
        .map(|(line, start, end)| {
            lsp_types::Location::new(uri.clone(), span(text, line, start, end))
        })
        .collect();

    // Every other page of the project. Read once per file: the columns arrive as scalar
    // offsets and converting them needs that file's own line, so a per-row read would open
    // the same chapter once per reference in it.
    if let Some(here) = here.as_deref()
        && let Some(scan) = project.get(here)
    {
        let mut rows: Vec<(PathBuf, u32, u32, u32)> = Vec::new();
        let width = id.chars().count() as u32;
        for u in scan.uses.iter().filter(|u| u.id == id && u.path != here) {
            rows.push((u.path.clone(), u.line, u.col, u.col + width));
        }
        if include_declaration {
            for a in scan.anchors.iter().filter(|a| a.id == id && a.path != here) {
                // The walk records an anchor's line but not its column, and re-deriving one
                // from the file would be a second scanner free to disagree with
                // `anchor_occurrences`. Ask that scanner instead, on the file it is about.
                rows.push((a.path.clone(), a.line, 0, 0));
            }
        }
        out.extend(locations_in_other_files(rows, &id));
    }

    out.sort_by(|a, b| {
        (a.uri.as_str(), a.range.start.line, a.range.start.character).cmp(&(
            b.uri.as_str(),
            b.range.start.line,
            b.range.start.character,
        ))
    });
    out.dedup_by(|a, b| a.uri == b.uri && a.range == b.range);
    Some(out)
}

/// Turn `(path, line, start_col, end_col)` scalar rows into LSP locations, reading each file
/// once. A zero-width row (`start == end`) is an *anchor* row, whose real span is asked of
/// `anchor_occurrences` on that file's own text rather than re-derived here.
fn locations_in_other_files(
    rows: Vec<(PathBuf, u32, u32, u32)>,
    id: &str,
) -> Vec<lsp_types::Location> {
    let mut by_file: HashMap<PathBuf, Vec<(u32, u32, u32)>> = HashMap::new();
    for (path, line, start, end) in rows {
        by_file.entry(path).or_default().push((line, start, end));
    }
    let mut out = Vec::new();
    for (path, rows) in by_file {
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(uri) = lsp_types::Url::from_file_path(&path) else {
            continue;
        };
        // The definition sites in this file, for the zero-width anchor rows above.
        let defs = crate::lsp_nav::definition_site(&body, id);
        for (line, start, end) in rows {
            let (start, end) = match start == end {
                false => (start, end),
                true => match defs {
                    Some((l, c)) if l == line => (c, c + id.chars().count() as u32),
                    // The file changed under the walk, or the anchor sits somewhere this
                    // scanner does not call a definition. Point at the line, not at a
                    // column we would be inventing.
                    _ => (0, 0),
                },
            };
            out.push(lsp_types::Location::new(
                uri.clone(),
                span(&body, line, start, end),
            ));
        }
    }
    out
}

/// A single-line range built from scalar columns, converted to the UTF-16 units LSP counts.
fn span(text: &str, line: u32, start: u32, end: u32) -> lsp_types::Range {
    let l = crate::lsp_pos::nth_line(text, line as usize);
    lsp_types::Range::new(
        lsp_types::Position::new(
            line,
            crate::lsp_pos::char_to_utf16(l, start as usize) as u32,
        ),
        lsp_types::Position::new(line, crate::lsp_pos::char_to_utf16(l, end as usize) as u32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A project root with the given pages. Named per test so two of them never share a
    /// directory under a parallel run.
    fn fixture(name: &str, pages: &[(&str, &str)]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("tali-lsp-refs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        for (rel, src) in pages {
            std::fs::write(root.join(rel), src).unwrap();
        }
        root
    }

    fn params(
        uri: &lsp_types::Url,
        line: u32,
        character: u32,
        decl: bool,
    ) -> lsp_types::ReferenceParams {
        lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                position: lsp_types::Position::new(line, character),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: decl,
            },
        }
    }

    /// `(file name, start line, start col)` for each answer, so an assertion reads as a place.
    fn sites(found: &[lsp_types::Location]) -> Vec<(String, u32, u32)> {
        found
            .iter()
            .map(|l| {
                let name = l
                    .uri
                    .path_segments()
                    .and_then(|mut s| s.next_back())
                    .unwrap_or_default()
                    .to_string();
                (name, l.range.start.line, l.range.start.character)
            })
            .collect()
    }

    /// The whole point of the item: `documentHighlight` answers within one file, and the
    /// question "where is this label used" is asked across a book.
    #[test]
    fn a_reference_on_another_page_is_found() {
        let root = fixture(
            "crossfile",
            &[
                ("a.tmd", "# A\n\n![p](i.png){#fig-scree}\n"),
                ("b.tmd", "# B\n\nSee @fig-scree.\n"),
            ],
        );
        let a = root.join("a.tmd");
        let uri = lsp_types::Url::from_file_path(&a).unwrap();
        let text = std::fs::read_to_string(&a).unwrap();
        let mut docs = HashMap::new();
        docs.insert(uri.clone(), text);
        let mut project = crate::lsp_project::ProjectCache::new();

        // Cursor on the `{#fig-scree}` definition in a.tmd.
        let found = references(&docs, &mut project, &params(&uri, 2, 17, false)).unwrap();
        assert_eq!(
            sites(&found),
            vec![("b.tmd".to_string(), 2, 5)],
            "the use on the other page, and not the definition the request excluded"
        );

        let with_def = references(&docs, &mut project, &params(&uri, 2, 17, true)).unwrap();
        assert_eq!(
            sites(&with_def),
            vec![("a.tmd".to_string(), 2, 13), ("b.tmd".to_string(), 2, 5)],
            "include_declaration adds the definition site, and nothing else"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The buffer is ahead of the file on disk, and the file on screen is the one the author
    /// is judging the answer by. A walk-only implementation would list a reference they had
    /// just deleted and miss the one they had just typed.
    #[test]
    fn the_open_buffer_wins_for_its_own_file() {
        let root = fixture(
            "buffer",
            &[
                (
                    "a.tmd",
                    "# A\n\n![p](i.png){#fig-scree}\n\nSee @fig-scree.\n",
                ),
                ("b.tmd", "# B\n"),
            ],
        );
        let a = root.join("a.tmd");
        let uri = lsp_types::Url::from_file_path(&a).unwrap();
        // Unsaved: the on-disk reference on line 4 is gone, a new one on line 2 is not.
        let buffer = "# A\n\n![p](i.png){#fig-scree} and @fig-scree\n\nnothing here\n";
        let mut docs = HashMap::new();
        docs.insert(uri.clone(), buffer.to_string());
        let mut project = crate::lsp_project::ProjectCache::new();

        let found = references(&docs, &mut project, &params(&uri, 2, 17, false)).unwrap();
        assert_eq!(
            sites(&found),
            vec![("a.tmd".to_string(), 2, 29)],
            "the unsaved reference, not the saved one the walk would have found"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A cursor on prose is not an empty reference list — those are different answers, and
    /// `[]` would claim this anchor has no uses.
    #[test]
    fn a_cursor_on_prose_answers_none_rather_than_an_empty_list() {
        let root = fixture("prose", &[("a.tmd", "# A\n\nordinary words here\n")]);
        let a = root.join("a.tmd");
        let uri = lsp_types::Url::from_file_path(&a).unwrap();
        let mut docs = HashMap::new();
        docs.insert(uri.clone(), std::fs::read_to_string(&a).unwrap());
        let mut project = crate::lsp_project::ProjectCache::new();
        assert!(references(&docs, &mut project, &params(&uri, 2, 3, true)).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A standalone document has no project to walk, and must still answer about itself
    /// rather than fall through to `None` — which would read as "not an anchor".
    #[test]
    fn a_document_outside_any_project_still_answers_about_itself() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-refs-solo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("solo.tmd");
        let src = "# S\n\n![p](i.png){#fig-a}\n\nSee @fig-a twice: @fig-a.\n";
        std::fs::write(&file, src).unwrap();
        let uri = lsp_types::Url::from_file_path(&file).unwrap();
        let mut docs = HashMap::new();
        docs.insert(uri.clone(), src.to_string());
        let mut project = crate::lsp_project::ProjectCache::new();

        let found = references(&docs, &mut project, &params(&uri, 4, 6, false)).unwrap();
        assert_eq!(found.len(), 2, "both uses on the page: {:?}", sites(&found));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// LSP columns are UTF-16 code units. A reference after an astral character on the same
    /// line is the case where a scalar column silently lands in the wrong place.
    #[test]
    fn columns_on_another_page_are_utf16_units() {
        let root = fixture(
            "utf16",
            &[
                ("a.tmd", "# A\n\n![p](i.png){#fig-x}\n"),
                ("b.tmd", "# B\n\n🌄🌄 see @fig-x.\n"),
            ],
        );
        let a = root.join("a.tmd");
        let uri = lsp_types::Url::from_file_path(&a).unwrap();
        let mut docs = HashMap::new();
        docs.insert(uri.clone(), std::fs::read_to_string(&a).unwrap());
        let mut project = crate::lsp_project::ProjectCache::new();

        let found = references(&docs, &mut project, &params(&uri, 2, 16, false)).unwrap();
        assert_eq!(
            sites(&found),
            vec![("b.tmd".to_string(), 2, 10)],
            "scalar col 8 + two astral chars = UTF-16 col 10"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
