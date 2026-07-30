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
        InsertKind::Bibtex => bibtex_paste(doc, text, payload),
        InsertKind::Dataset => dataset_drop(doc, text, Path::new(payload)),
    }
}

/// A path expressed relative to `dir`, or `None` when that would need to climb out of it.
///
/// A `../` dataset or asset path is one the build cannot ship (`copy_local_assets` warns and
/// skips it), so the editor must not emit one.
fn relative_to(dir: &Path, target: &Path) -> Option<String> {
    let dir = dir.canonicalize().ok()?;
    let target = target.canonicalize().ok()?;
    let rel = target.strip_prefix(&dir).ok()?;
    // Always forward slashes: this string goes into a document, not into a syscall.
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// The language of the document's FIRST kernel cell.
///
/// First rather than nearest, so the same document always yields the same answer no matter where
/// the drop lands. Only `python` and `r` count: they are the two kernel languages, and a client
/// language like `{js}` has no CSV loader to write (see [`dataset_drop`]).
fn first_kernel_language(text: &str) -> Option<&'static str> {
    crate::lsp_cells::cell_regions(text)
        .iter()
        .find_map(|r| match r.language.as_str() {
            "python" => Some("python"),
            "r" => Some("r"),
            _ => None,
        })
}

/// A dropped `.csv`: the provenance card, plus a loader cell when the document has a kernel.
///
/// There is deliberately no `datasets:` front-matter scaffold. `corpus/datasets.tmd` documents
/// that block as optional, because the size and checksum are read off the file itself, so the
/// only keys it would add (licence, source, description) are facts the author has and the editor
/// does not. Emitting them empty is noise a lint may flag.
fn dataset_drop(doc: &Path, text: &str, csv: &Path) -> Result<InsertEditResult, String> {
    let dir = doc.parent().ok_or("the document has no directory")?;
    let rel = relative_to(dir, csv).ok_or_else(|| {
        format!(
            "{} is outside the document's folder, so the build could not ship it",
            csv.display()
        )
    })?;

    let card = format!("{{{{< dataset {rel} >}}}}");
    // A client-language document ({js}, {glsl}) gets the card and no loader: there is no CSV
    // reader in `tali-js.js` to call, and inventing one would emit a call to a function that
    // does not exist. `python` is the default only for a document with no kernel cell yet.
    let language = match first_kernel_language(text) {
        Some(lang) => Some(lang),
        // No kernel cell. A document with no cells at all is starting fresh, so `python` is the
        // default; a document whose only cells are a client language has deliberately chosen the
        // browser, and a Python cell would silently require a kernel it does not use.
        None if crate::lsp_cells::cell_regions(text).is_empty() => Some("python"),
        None => None,
    };
    let cell = language.map(|lang| match lang {
        "r" => {
            let import = if text.contains("library(readr)") {
                ""
            } else {
                "library(readr)\n"
            };
            format!("```{{r}}\n{import}df <- read_csv(\"{rel}\")\n```")
        }
        _ => {
            let import = if text.contains("import pandas") {
                ""
            } else {
                "import pandas as pd\n"
            };
            format!("```{{python}}\n{import}df = pd.read_csv(\"{rel}\")\n```")
        }
    });

    Ok(InsertEditResult {
        text: match cell {
            Some(cell) => format!("{card}\n\n{cell}"),
            None => card,
        },
        is_snippet: false,
        write_file: None,
        append: None,
    })
}

/// The citation key of a pasted BibTeX entry: the identifier between `@type{` and the first
/// comma or newline.
///
/// Hand-scanned rather than parsed with [`taliesin_core::cite::parse_bib`], because that returns
/// an empty bibliography for malformed input and this needs to tell "not a BibTeX entry" apart
/// from "an entry with no fields".
fn bibtex_key(entry: &str) -> Option<String> {
    let at = entry.find('@')?;
    let rest = &entry[at + 1..];
    let brace = rest.find('{')?;
    // The `@type` must be a bare word: `@ article{` and `@{` are not entries.
    let kind = rest[..brace].trim();
    if kind.is_empty() || !kind.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let key: String = rest[brace + 1..]
        .chars()
        .take_while(|c| *c != ',' && *c != '\n' && *c != '}')
        .collect();
    let key = key.trim();
    if key.is_empty() || key.chars().any(char::is_whitespace) {
        return None;
    }
    Some(key.to_owned())
}

/// A pasted BibTeX entry: cite it, and append it to the document's `.bib` when it is new.
fn bibtex_paste(doc: &Path, text: &str, entry: &str) -> Result<InsertEditResult, String> {
    let key = bibtex_key(entry).ok_or("that does not look like a BibTeX entry")?;
    let cite = InsertEditResult {
        text: format!("[@{key}]"),
        is_snippet: false,
        write_file: None,
        append: None,
    };

    // The document's own `bibliography:`, read by the same scanner go-to-definition uses on a
    // citation, so the paste lands in the file the author would be taken to.
    let dir = doc.parent().ok_or("the document has no directory")?;
    let Some(rel) = crate::lsp_nav::frontmatter_bib_paths(text)
        .into_iter()
        .next()
    else {
        // No bibliography at all. Cite it and stop: creating the `.bib`, editing the front
        // matter and pasting is three coupled writes for the least common case, and
        // `citations_without_bibliography` already reports the gap.
        return Ok(cite);
    };
    let bib = dir.join(&rel);
    let existing = std::fs::read_to_string(&bib).unwrap_or_default();
    if taliesin_core::cite::parse_bib(&existing).contains(&key) {
        // Appending a key the file already has would make this gesture trip the author's own
        // duplicate-key lint (`parse_bib_warned`), so cite the entry that is already there.
        return Ok(cite);
    }
    // A file not ending in a newline would otherwise glue two entries onto one line.
    let lead = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let trimmed = entry.trim_end();
    Ok(InsertEditResult {
        append: Some(AppendEdit {
            path: bib.to_string_lossy().into_owned(),
            text: format!("{lead}{trimmed}\n"),
        }),
        ..cite
    })
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
    fn a_pasted_bibtex_entry_appends_to_the_bib_and_inserts_the_key() {
        let tmp = TmpDir::new("bib");
        let doc = tmp.path().join("bayes.tmd");
        let text = "---\nbibliography: refs.bib\n---\n\nBody.\n";
        std::fs::write(&doc, text).unwrap();
        std::fs::write(
            tmp.path().join("refs.bib"),
            "@book{knuth1984,\n  title = {TeX},\n}\n",
        )
        .unwrap();

        let entry = "@article{bishop2006,\n  title = {Pattern Recognition},\n  year = {2006},\n}";
        let r = insert_edit(&doc, text, InsertKind::Bibtex, entry).unwrap();

        assert_eq!(r.text, "[@bishop2006]");
        assert!(!r.is_snippet);
        let append = r.append.expect("a new entry is appended to the .bib");
        assert!(append.path.ends_with("refs.bib"), "{}", append.path);
        assert!(append.text.contains("bishop2006"), "{}", append.text);
        assert!(append.text.ends_with('\n'), "the append ends in a newline");
    }

    #[test]
    fn an_append_to_a_bib_with_no_trailing_newline_does_not_glue_two_entries() {
        let tmp = TmpDir::new("bibglue");
        let doc = tmp.path().join("bayes.tmd");
        let text = "---\nbibliography: refs.bib\n---\n\nBody.\n";
        std::fs::write(&doc, text).unwrap();
        // No trailing newline, which is what an editor that trims final newlines leaves.
        std::fs::write(
            tmp.path().join("refs.bib"),
            "@book{knuth1984, title = {TeX}}",
        )
        .unwrap();

        let r = insert_edit(
            &doc,
            text,
            InsertKind::Bibtex,
            "@article{ab2020, title={X}}",
        )
        .unwrap();

        let append = r.append.expect("appended");
        assert!(
            append.text.starts_with('\n'),
            "a leading newline separates the entries: {:?}",
            append.text
        );
    }

    #[test]
    fn an_entry_already_in_the_bib_is_cited_but_not_appended_twice() {
        let tmp = TmpDir::new("bibdup");
        let doc = tmp.path().join("bayes.tmd");
        let text = "---\nbibliography: refs.bib\n---\n\nBody.\n";
        std::fs::write(&doc, text).unwrap();
        std::fs::write(
            tmp.path().join("refs.bib"),
            "@book{bishop2006,\n  title = {PR},\n}\n",
        )
        .unwrap();

        let entry = "@article{bishop2006,\n  title = {Pattern Recognition},\n}";
        let r = insert_edit(&doc, text, InsertKind::Bibtex, entry).unwrap();

        // parse_bib_warned lints duplicate keys, so appending one would make this gesture trip
        // the author's own diagnostic.
        assert_eq!(r.text, "[@bishop2006]");
        assert_eq!(r.append, None, "no second copy of the key");
    }

    #[test]
    fn a_document_with_no_bibliography_still_gets_the_citation() {
        let tmp = TmpDir::new("nobib");
        let doc = tmp.path().join("bayes.tmd");
        let text = "# Bayes\n\nBody.\n";
        std::fs::write(&doc, text).unwrap();

        let r = insert_edit(&doc, text, InsertKind::Bibtex, "@book{ab2020, title={X}}").unwrap();

        // Creating the .bib, editing front matter and pasting is three coupled writes for the
        // least common case. `citations_without_bibliography` already reports the gap.
        assert_eq!(r.text, "[@ab2020]");
        assert_eq!(r.append, None);
    }

    #[test]
    fn a_dropped_csv_gets_a_dataset_card_and_a_loader_cell() {
        let tmp = TmpDir::new("csv");
        let doc = tmp.path().join("bayes.tmd");
        let text = "# Bayes\n";
        std::fs::write(&doc, text).unwrap();
        std::fs::create_dir_all(tmp.path().join("data")).unwrap();
        let csv = tmp.path().join("data/measurements.csv");
        std::fs::write(&csv, "a,b\n1,2\n").unwrap();

        let r = insert_edit(&doc, text, InsertKind::Dataset, csv.to_str().unwrap()).unwrap();

        assert!(
            r.text.contains("{{< dataset data/measurements.csv >}}"),
            "the card, with a doc-relative path: {}",
            r.text
        );
        // No cells at all, so the loader defaults to python.
        assert!(r.text.contains("```{python}"), "{}", r.text);
        assert!(r.text.contains("import pandas as pd"), "{}", r.text);
        assert!(
            r.text.contains("pd.read_csv(\"data/measurements.csv\")"),
            "{}",
            r.text
        );
    }

    #[test]
    fn the_loader_follows_the_documents_first_kernel_cell_language() {
        let tmp = TmpDir::new("csvr");
        let doc = tmp.path().join("bayes.tmd");
        // Two kernel languages, r before python, so "first" and "last" give DIFFERENT answers.
        // A fixture with one kernel cell cannot tell the two apart, and the mutation that
        // swapped `find_map` for `last()` survived against exactly that weaker fixture.
        // The leading bash fence is not a kernel language and must not decide the loader.
        let text = "```bash\nls\n```\n\n```{r}\nsummary(x)\n```\n\n```{python}\nx = 1\n```\n";
        std::fs::write(&doc, text).unwrap();
        let csv = tmp.path().join("m.csv");
        std::fs::write(&csv, "a\n1\n").unwrap();

        let r = insert_edit(&doc, text, InsertKind::Dataset, csv.to_str().unwrap()).unwrap();

        assert!(r.text.contains("```{r}"), "{}", r.text);
        assert!(r.text.contains("library(readr)"), "{}", r.text);
        assert!(r.text.contains("read_csv(\"m.csv\")"), "{}", r.text);
        assert!(
            !r.text.contains("pandas"),
            "not the python loader: {}",
            r.text
        );
    }

    #[test]
    fn an_import_the_document_already_has_is_not_repeated() {
        let tmp = TmpDir::new("csvimp");
        let doc = tmp.path().join("bayes.tmd");
        let text = "```{python}\nimport pandas as pd\n```\n";
        std::fs::write(&doc, text).unwrap();
        let csv = tmp.path().join("m.csv");
        std::fs::write(&csv, "a\n1\n").unwrap();

        let r = insert_edit(&doc, text, InsertKind::Dataset, csv.to_str().unwrap()).unwrap();

        assert!(
            !r.text.contains("import pandas"),
            "already imported: {}",
            r.text
        );
        assert!(r.text.contains("pd.read_csv(\"m.csv\")"), "{}", r.text);
    }

    #[test]
    fn a_client_language_document_gets_the_card_with_no_loader() {
        let tmp = TmpDir::new("csvjs");
        let doc = tmp.path().join("bayes.tmd");
        // `{js}` runs in the browser and `tali-js.js` has no CSV reader, so a loader cell would
        // be a call to a function that does not exist. A python cell would be worse: it would
        // silently require a kernel this document deliberately does not use.
        let text = "```{js}\nconst x = 1;\n```\n";
        std::fs::write(&doc, text).unwrap();
        let csv = tmp.path().join("m.csv");
        std::fs::write(&csv, "a\n1\n").unwrap();

        let r = insert_edit(&doc, text, InsertKind::Dataset, csv.to_str().unwrap()).unwrap();

        assert_eq!(r.text, "{{< dataset m.csv >}}", "the card alone");
    }

    #[test]
    fn a_csv_outside_the_document_folder_is_refused() {
        let tmp = TmpDir::new("csvout");
        std::fs::create_dir_all(tmp.path().join("proj")).unwrap();
        let doc = tmp.path().join("proj/bayes.tmd");
        std::fs::write(&doc, "").unwrap();
        let csv = tmp.path().join("elsewhere.csv");
        std::fs::write(&csv, "a\n1\n").unwrap();

        // A `../` dataset path is one the build cannot ship, so the editor must not emit it.
        let err = insert_edit(&doc, "", InsertKind::Dataset, csv.to_str().unwrap()).unwrap_err();
        assert!(err.contains("outside"), "{err}");
    }

    #[test]
    fn text_that_is_not_a_bibtex_entry_is_refused() {
        let tmp = TmpDir::new("notbib");
        let doc = tmp.path().join("bayes.tmd");
        std::fs::write(&doc, "").unwrap();

        for junk in [
            "@ not an entry",     // a space where the type belongs
            "just prose",         // no @ at all
            "@article{}",         // no key
            "@article{a b, x=1}", // a key with a space in it
        ] {
            let err = insert_edit(&doc, "", InsertKind::Bibtex, junk).unwrap_err();
            assert!(err.contains("BibTeX"), "for {junk:?}: {err}");
        }
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
