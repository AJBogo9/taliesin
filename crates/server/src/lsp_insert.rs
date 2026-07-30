//! `taliesin/insertEdit`: the text a companion paste or drop gesture inserts.
//!
//! The gestures themselves are VS Code APIs and have to live in the client, but every string
//! they insert is `.tmd` vocabulary and is computed here, for the same reason
//! [`crate::lsp_edits::section_edit`] exists: a figure shape, a pipe table or a citation key
//! written in TypeScript is a second copy of knowledge this crate already owns, free to
//! disagree with the renderer about what it means.
//!
//! The client owns exactly two things this module cannot: the clipboard bytes (which never
//! reach a JSON-RPC wire) and the file write. So an image paste returns the *name* to write
//! and the text to insert, and the client puts the bytes there.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InsertEditParams {
    pub(crate) text_document: lsp_types::TextDocumentIdentifier,
    pub(crate) kind: InsertKind,
    /// Per kind: the clipboard mime type for [`InsertKind::Image`], the pasted text for
    /// [`InsertKind::HtmlTable`] / [`InsertKind::TsvTable`] / [`InsertKind::Bibtex`], and the
    /// dropped file's absolute path for [`InsertKind::Dataset`].
    pub(crate) payload: String,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum InsertKind {
    Image,
    HtmlTable,
    TsvTable,
    Bibtex,
    Dataset,
}

#[derive(Debug, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InsertEditResult {
    /// What to insert at the gesture's position.
    pub(crate) text: String,
    /// `true` when [`Self::text`] is a snippet carrying `${n:…}` tab stops rather than literal
    /// text, so the client knows to apply it as one.
    pub(crate) is_snippet: bool,
    /// A file the client must write before applying [`Self::text`], relative to the document's
    /// directory. Only an image paste sets it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) write_file: Option<String>,
    /// An append to a second file (the `.bib`), so the client can carry it in the same undo as
    /// the paste rather than writing behind the editor's back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) append: Option<AppendEdit>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppendEdit {
    pub(crate) path: String,
    pub(crate) text: String,
}

/// The extension a clipboard image is saved under.
///
/// Refused rather than guessed: an unknown mime saved as `.png` produces a file whose bytes
/// contradict its name, which every later tool reads as corruption.
fn image_extension(mime: &str) -> Result<&'static str, String> {
    match mime {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/svg+xml" => Ok("svg"),
        "image/webp" => Ok("webp"),
        "image/gif" => Ok("gif"),
        other => Err(format!("cannot paste {other}: unsupported image type")),
    }
}

/// A document stem reduced to a filename that needs no escaping anywhere it is later written:
/// in a Markdown link, in a shell, and in the snippet text this module returns, where a bare
/// `$` or `}` would be read as snippet syntax.
fn slug(stem: &str) -> String {
    let mut out = String::new();
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_owned();
    if trimmed.is_empty() {
        "figure".to_owned()
    } else {
        trimmed
    }
}

/// The next free `<slug>-NN` in `dir`, ignoring the extension.
///
/// A gap is never reused: two pastes in one session must not collide after the author deletes
/// the first file, and the extension is ignored so a `.png` and a `.jpg` cannot claim the same
/// number and leave two different images sharing one caption.
fn next_index(dir: &Path, slug: &str) -> u32 {
    let prefix = format!("{slug}-");
    let mut max = 0u32;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(rest) = name.strip_prefix(&prefix) else {
                continue;
            };
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(n) = digits.parse::<u32>() {
                max = max.max(n);
            }
        }
    }
    max + 1
}

/// Compute the edit for one gesture.
///
/// `text` is the open buffer, needed by the kinds that must not repeat something the document
/// already has. Taking the kind and payload as arguments rather than the whole
/// [`InsertEditParams`] keeps this callable from a test without minting a URI it never reads.
pub(crate) fn insert_edit(
    doc: &Path,
    text: &str,
    kind: InsertKind,
    payload: &str,
) -> Result<InsertEditResult, String> {
    let _ = text;
    match kind {
        InsertKind::Image => image_paste(doc, payload),
        InsertKind::HtmlTable | InsertKind::TsvTable => table_paste(kind, payload),
        _ => todo!("later tasks"),
    }
}

/// Rows of cells from tab-separated clipboard text.
fn tsv_rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('\t').map(|c| c.trim().to_owned()).collect())
        .collect()
}

/// Rows of cells from clipboard HTML.
///
/// A tolerant scanner rather than a parser, deliberately: the input is one `<table>` put on the
/// clipboard by a spreadsheet or a browser, not an arbitrary document, and this project declines
/// dependencies it can do without. The one shape it gets wrong is a `>` inside an attribute
/// value, which no spreadsheet emits.
fn html_rows(html: &str) -> Vec<Vec<String>> {
    let lower = html.to_ascii_lowercase();
    let mut rows = Vec::new();
    let mut at = 0usize;
    while let Some(tr) = lower[at..].find("<tr").map(|i| at + i) {
        let end = lower[tr..]
            .find("</tr")
            .map(|i| tr + i)
            .unwrap_or(lower.len());
        let mut cells = Vec::new();
        let mut cell_at = tr;
        while let Some(open) = ["<td", "<th"]
            .iter()
            .filter_map(|t| lower[cell_at..end].find(t).map(|i| cell_at + i))
            .min()
        {
            // Step past the tag's own attributes to the content.
            let Some(gt) = lower[open..end].find('>').map(|i| open + i + 1) else {
                break;
            };
            let close = ["</td", "</th"]
                .iter()
                .filter_map(|t| lower[gt..end].find(t).map(|i| gt + i))
                .min()
                .unwrap_or(end);
            cells.push(strip_tags(&html[gt..close]));
            cell_at = close + 1;
        }
        if !cells.is_empty() {
            rows.push(cells);
        }
        at = end + 1;
    }
    rows
}

/// Cell text with inline markup removed and the entities a spreadsheet emits decoded.
///
/// `&amp;` is decoded LAST, or a literal `&amp;nbsp;` in the author's data would turn into a
/// space instead of the text they copied.
fn strip_tags(cell: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for ch in cell.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .trim()
        .to_owned()
}

/// A pasted grid as a pipe table, aligned by the one aligner "Format Document" uses.
///
/// Alignment markers are all default (`---`). Reading `align=` out of clipboard HTML would be
/// speculative work for a shape no corpus document has, and `format_tables` re-derives column
/// widths from the delimiter row either way.
fn table_paste(kind: InsertKind, payload: &str) -> Result<InsertEditResult, String> {
    let rows = match kind {
        InsertKind::HtmlTable => html_rows(payload),
        _ => tsv_rows(payload),
    };
    let width = rows.first().map(Vec::len).unwrap_or(0);
    // Two rows and two columns is the floor, and every row must agree on the column count.
    // Padding a ragged grid to a rectangle would invent cells the author never wrote, and
    // tab-separated prose is not a table at all.
    if rows.len() < 2 || width < 2 || rows.iter().any(|r| r.len() != width) {
        return Err("that does not look like a table (not a rectangular grid)".to_owned());
    }
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        // An unescaped pipe SPLITS the cell, silently turning an n-column row into n+1.
        let cells: Vec<String> = row.iter().map(|c| c.replace('|', r"\|")).collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
        if i == 0 {
            let delims: Vec<&str> = (0..width).map(|_| "---").collect();
            out.push_str(&format!("| {} |\n", delims.join(" | ")));
        }
    }
    let aligned =
        crate::lsp_format::apply_line_edits(&out, &crate::lsp_format::format_tables(&out));
    Ok(InsertEditResult {
        text: aligned.trim_end().to_owned(),
        is_snippet: false,
        write_file: None,
        append: None,
    })
}

fn image_paste(doc: &Path, mime: &str) -> Result<InsertEditResult, String> {
    let ext = image_extension(mime)?;
    let dir = doc.parent().ok_or("the document has no directory")?;
    let stem = doc.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let slug = slug(stem);
    let name = format!("{slug}-{:02}.{ext}", next_index(dir, &slug));
    Ok(InsertEditResult {
        // The caption and the label are the two things only the author can write, so both are
        // tab stops. `name` came out of `slug`, so it cannot contain a `$` or `}` that the
        // client would read as snippet syntax.
        text: format!("![${{1:caption}}]({name}){{#fig-${{2:label}}}}"),
        is_snippet: true,
        write_file: Some(name),
        append: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A scratch directory that removes itself.
    ///
    /// `tempfile` is not a dependency of this crate and the project declines dependencies it
    /// can do without, so this follows the existing `freeze.rs` idiom: the pid plus an atomic
    /// counter, which keeps parallel test binaries and parallel tests apart.
    struct TmpDir(PathBuf);

    impl TmpDir {
        fn new(tag: &str) -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "tali-insert-{tag}-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn an_image_paste_names_the_file_from_the_document_stem() {
        let tmp = TmpDir::new("stem");
        let doc = tmp.path().join("bayes.tmd");
        std::fs::write(&doc, "# Bayes\n").unwrap();

        let r = insert_edit(&doc, "# Bayes\n", InsertKind::Image, "image/png").unwrap();

        assert_eq!(r.write_file.as_deref(), Some("bayes-01.png"));
        assert!(r.is_snippet, "the caption and label are tab stops");
        assert_eq!(
            r.text, "![${1:caption}](bayes-01.png){#fig-${2:label}}",
            "the canonical corpus figure shape, beside the doc, with both tab stops"
        );
    }

    #[test]
    fn the_counter_skips_names_already_on_disk() {
        let tmp = TmpDir::new("counter");
        let doc = tmp.path().join("bayes.tmd");
        std::fs::write(&doc, "").unwrap();
        // Not 02 and not 05: the extension is ignored, so a `.jpg` at 04 still blocks 04, and
        // the gap at 02/03 is not reused.
        std::fs::write(tmp.path().join("bayes-01.png"), "").unwrap();
        std::fs::write(tmp.path().join("bayes-04.jpg"), "").unwrap();

        let r = insert_edit(&doc, "", InsertKind::Image, "image/png").unwrap();
        assert_eq!(r.write_file.as_deref(), Some("bayes-05.png"));
    }

    #[test]
    fn a_document_stem_that_is_not_a_safe_filename_is_slugged() {
        let tmp = TmpDir::new("slug");
        // Spaces and parentheses force every later reference to escape them, and a `$` would
        // be read as a placeholder in the snippet this returns.
        let doc = tmp.path().join("Chapter 1 (draft).tmd");
        std::fs::write(&doc, "").unwrap();

        let r = insert_edit(&doc, "", InsertKind::Image, "image/svg+xml").unwrap();
        assert_eq!(r.write_file.as_deref(), Some("chapter-1-draft-01.svg"));
    }

    #[test]
    fn a_stem_that_slugs_to_nothing_falls_back_to_figure() {
        let tmp = TmpDir::new("empty");
        let doc = tmp.path().join("___.tmd");
        std::fs::write(&doc, "").unwrap();

        let r = insert_edit(&doc, "", InsertKind::Image, "image/png").unwrap();
        assert_eq!(r.write_file.as_deref(), Some("figure-01.png"));
    }

    #[test]
    fn a_pasted_tsv_grid_becomes_an_aligned_pipe_table() {
        let tmp = TmpDir::new("tsv");
        let doc = tmp.path().join("bayes.tmd");
        std::fs::write(&doc, "").unwrap();

        let tsv = "site\tdepth\ttemp\nnorth\t3\t7.1\nsouth\t12\t4.4\n";
        let r = insert_edit(&doc, "", InsertKind::TsvTable, tsv).unwrap();

        assert!(
            !r.is_snippet,
            "a table has nothing for the author to fill in"
        );
        assert_eq!(
            r.text,
            "| site  | depth | temp |\n\
             | ----- | ----- | ---- |\n\
             | north | 3     | 7.1  |\n\
             | south | 12    | 4.4  |",
            "columns padded by the one aligner Format Document uses"
        );
    }

    #[test]
    fn a_cell_containing_a_pipe_is_escaped() {
        let tmp = TmpDir::new("pipe");
        let doc = tmp.path().join("bayes.tmd");
        std::fs::write(&doc, "").unwrap();

        // An unescaped pipe splits the cell, silently turning a 2-column row into 3. This is a
        // trap already recorded in LESSONS.md, which is why it gets its own test.
        let r = insert_edit(
            &doc,
            "",
            InsertKind::TsvTable,
            "expr\tmeaning\na|b\tunion\n",
        )
        .unwrap();

        assert!(r.text.contains(r"a\|b"), "the pipe is escaped: {}", r.text);
        for line in r.text.lines() {
            let bars = line.replace(r"\|", "").matches('|').count();
            assert_eq!(bars, 3, "every row still has 2 columns: {line}");
        }
    }

    #[test]
    fn a_pasted_html_table_reads_th_td_and_decodes_entities() {
        let tmp = TmpDir::new("html");
        let doc = tmp.path().join("bayes.tmd");
        std::fs::write(&doc, "").unwrap();

        // The shape a spreadsheet actually puts on the clipboard: attributes on every tag,
        // inline markup inside cells, and an entity for a blank cell.
        let html = "<table><tr><th align=\"left\">site</th><th>n</th></tr>\
                    <tr><td style=\"x\"><b>north</b></td><td>&nbsp;</td></tr></table>";
        let r = insert_edit(&doc, "", InsertKind::HtmlTable, html).unwrap();

        let lines: Vec<&str> = r.text.lines().collect();
        assert_eq!(lines.len(), 3, "header, delimiter, one body row: {lines:?}");
        assert!(lines[0].contains("site"), "{}", lines[0]);
        assert!(
            lines[2].contains("north"),
            "cell markup stripped: {}",
            lines[2]
        );
        assert!(!r.text.contains("&nbsp;"), "entities decoded: {}", r.text);
        assert!(!r.text.contains("<b>"), "tags stripped: {}", r.text);
    }

    #[test]
    fn a_literal_ampersand_entity_survives_the_decode_order() {
        let tmp = TmpDir::new("amp");
        let doc = tmp.path().join("bayes.tmd");
        std::fs::write(&doc, "").unwrap();

        // `&amp;nbsp;` is the author's literal text "&nbsp;", not a blank. Decoding `&amp;`
        // before `&nbsp;` would turn it into a space and lose what they copied.
        let html =
            "<table><tr><th>a</th><th>b</th></tr><tr><td>&amp;nbsp;</td><td>x</td></tr></table>";
        let r = insert_edit(&doc, "", InsertKind::HtmlTable, html).unwrap();

        assert!(
            r.text.contains("&nbsp;"),
            "the literal entity survives: {}",
            r.text
        );
    }

    #[test]
    fn a_ragged_grid_is_refused_rather_than_silently_squared() {
        let tmp = TmpDir::new("ragged");
        let doc = tmp.path().join("bayes.tmd");
        std::fs::write(&doc, "").unwrap();

        // Tab-separated prose is not a table. Padding it to a rectangle invents cells.
        let err = insert_edit(&doc, "", InsertKind::TsvTable, "a\tb\nc\n").unwrap_err();
        assert!(err.contains("does not look like a table"), "{err}");
    }

    #[test]
    fn a_single_column_is_not_a_table() {
        let tmp = TmpDir::new("onecol");
        let doc = tmp.path().join("bayes.tmd");
        std::fs::write(&doc, "").unwrap();

        let err = insert_edit(&doc, "", InsertKind::TsvTable, "alpha\nbeta\n").unwrap_err();
        assert!(err.contains("does not look like a table"), "{err}");
    }

    #[test]
    fn an_unknown_clipboard_mime_is_refused_rather_than_guessed() {
        let tmp = TmpDir::new("mime");
        let doc = tmp.path().join("bayes.tmd");
        std::fs::write(&doc, "").unwrap();

        let err = insert_edit(&doc, "", InsertKind::Image, "image/tiff").unwrap_err();
        assert!(
            err.contains("image/tiff"),
            "the refusal names the mime: {err}"
        );
    }
}
