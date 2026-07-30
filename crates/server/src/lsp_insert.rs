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
        _ => todo!("later tasks"),
    }
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
