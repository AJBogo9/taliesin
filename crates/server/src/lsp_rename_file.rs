//! `taliesin/renameFileEdits`: keep a renamed or moved file's references correct.
//!
//! Two halves, and both are needed for a MOVE to be honest:
//!
//! - **inbound**, every `{{< include >}}`, `{{< embed >}}`, relative link and `_site.yml` entry
//!   pointing AT the file;
//! - **outbound**, the file's own relative references, which break the moment it changes
//!   directory (see [`outbound_edits`]).
//!
//! A rename is a one-shot event, so this WALKS the project rather than maintaining an index. That
//! is the whole reason this is cheap: the parked "project index" idea exists for features that
//! answer on every keystroke, and a rename is not one of them.
//!
//! Editing `_site.yml` as text rather than re-serializing it is deliberate: a YAML round-trip
//! reformats the author's file and drops its comments, and `corpus/tarn/_site.yml` carries a
//! comment that explains a bug pinned by a test.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenameFileEditsParams {
    /// The Explorer can rename a multi-selection, so this is a list.
    pub(crate) files: Vec<RenamedFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenamedFile {
    pub(crate) old_uri: lsp_types::Url,
    pub(crate) new_uri: lsp_types::Url,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileEdits {
    pub(crate) uri: lsp_types::Url,
    pub(crate) edits: Vec<lsp_types::TextEdit>,
}

/// Extensions whose references this repairs. A `.tmd` is referenced two ways (as itself and as the
/// `.html` it becomes); anything else is referenced only as itself.
const SOURCE_EXT: &str = "tmd";

/// Every edit needed to keep `params.files`' references correct.
///
/// An empty list is the correct answer for "nothing to repair", so there is no error path: a
/// rename must never fail because the repair found nothing to do.
pub(crate) fn rename_file_edits(params: &RenameFileEditsParams) -> Vec<FileEdits> {
    let mut out: Vec<FileEdits> = Vec::new();
    for file in &params.files {
        let (Ok(old), Ok(new)) = (file.old_uri.to_file_path(), file.new_uri.to_file_path()) else {
            continue;
        };
        for edits in inbound_edits(&old, &new) {
            merge(&mut out, edits);
        }
        if let Some(edits) = outbound_edits(&old, &new) {
            merge(&mut out, edits);
        }
    }
    out
}

/// Fold `incoming` into `out`, joining the edit lists when the same file is touched twice.
///
/// Two renames in one gesture can both edit the same referring page, and a client applying two
/// `FileEdits` for one URI would either drop one or apply overlapping ranges.
fn merge(out: &mut Vec<FileEdits>, incoming: FileEdits) {
    if let Some(existing) = out.iter_mut().find(|f| f.uri == incoming.uri) {
        existing.edits.extend(incoming.edits);
        // Later lines first, so a client applying them in order cannot shift a range it has not
        // reached yet.
        existing
            .edits
            .sort_by_key(|e| (e.range.start.line, e.range.start.character));
    } else {
        out.push(incoming);
    }
}

/// Every page and `_site.yml` in the project that mentions `old`, rewritten to `new`.
///
/// Returns nothing when the document is under no `_site.yml`: item 70 rules that such a tree
/// declares no boundary, and that inferring one is the wrong move. There is nothing to walk.
fn inbound_edits(old: &Path, new: &Path) -> Vec<FileEdits> {
    let Some(root) = taliesin_core::site::enclosing_site_root(old) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for page in project_files(&root) {
        // The renamed file's own references are the outbound half, not this one.
        if same_path(&page, old) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&page) else {
            continue;
        };
        let Some(dir) = page.parent() else { continue };
        let mut edits = Vec::new();
        for (from, to) in spellings(dir, old, new) {
            edits.extend(occurrences(&text, &from, &to));
        }
        if edits.is_empty() {
            continue;
        }
        edits.sort_by_key(|e: &lsp_types::TextEdit| (e.range.start.line, e.range.start.character));
        if let Ok(uri) = lsp_types::Url::from_file_path(&page) {
            out.push(FileEdits { uri, edits });
        }
    }
    out
}

/// The `(old, new)` reference spellings to look for, both relative to `dir`.
///
/// A `.tmd` is referenced TWO ways and both must be handled: as `intro.tmd` (an include, or a
/// link an author wrote against the source) and as `intro.html` (what a cross-page link is
/// authored as, per the books' own convention). Handling one spelling leaves a dead link while
/// reporting success, which is worse than not running at all.
fn spellings(dir: &Path, old: &Path, new: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let (Some(o), Some(n)) = (rel(dir, old), rel(dir, new)) {
        out.push((o.clone(), n.clone()));
        if old.extension().is_some_and(|e| e == SOURCE_EXT)
            && new.extension().is_some_and(|e| e == SOURCE_EXT)
            && let (Some(oh), Some(nh)) = (swap_ext(&o, "html"), swap_ext(&n, "html"))
        {
            out.push((oh, nh));
        }
    }
    out
}

/// `a/b/c.tmd` with its extension replaced, or `None` when it has none.
fn swap_ext(rel: &str, ext: &str) -> Option<String> {
    let dot = rel.rfind('.')?;
    // A dot in a directory name is not an extension.
    if rel[dot..].contains('/') {
        return None;
    }
    Some(format!("{}.{ext}", &rel[..dot]))
}

/// `target` relative to `dir`, forward-slashed, using `..` when it has to climb.
///
/// Unlike the asset path in `lsp_insert`, climbing is legitimate here: a page in `parts/one/` may
/// perfectly well include `../../_includes/x.tmd`.
fn rel(dir: &Path, target: &Path) -> Option<String> {
    let dir = normalize(dir);
    let target = normalize(target);
    let shared = dir
        .components()
        .zip(target.components())
        .take_while(|(a, b)| a == b)
        .count();
    let up = dir.components().count() - shared;
    let down: Vec<String> = target
        .components()
        .skip(shared)
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if up == 0 && down.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_owned(), up).collect();
    parts.extend(down);
    Some(parts.join("/"))
}

/// Lexically resolve `.` and `..` without touching the filesystem, so a comparison is stable and
/// a symlink is not followed.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn same_path(a: &Path, b: &Path) -> bool {
    normalize(a) == normalize(b)
}

/// Every occurrence of `needle` in `text`, as an edit replacing it with `replacement`.
///
/// The boundary check is the whole correctness story: a plain substring search for `intro.tmd`
/// also matches `my-intro.tmd` and `intro.tmda`, so renaming one file would silently corrupt a
/// reference to another.
fn occurrences(text: &str, needle: &str, replacement: &str) -> Vec<lsp_types::TextEdit> {
    let mut out = Vec::new();
    for (line_no, line) in text.split('\n').enumerate() {
        let mut from = 0usize;
        while let Some(hit) = line[from..].find(needle).map(|i| from + i) {
            from = hit + needle.len();
            let before = line[..hit].chars().next_back();
            let rest = &line[hit + needle.len()..];
            // A reference must START at a boundary: the start of the line, or a character that
            // cannot be part of a path. `/` counts, so `sub/intro.tmd` does not match on its
            // `intro.tmd` tail, and `-` counts, so `my-intro.tmd` is left alone.
            let starts_clean = before.is_none_or(|c| !is_path_char(c));
            if starts_clean && ends_reference(rest) {
                out.push(lsp_types::TextEdit {
                    range: lsp_types::Range {
                        start: utf16_position(line, line_no, hit),
                        end: utf16_position(line, line_no, hit + needle.len()),
                    },
                    new_text: replacement.to_owned(),
                });
            }
        }
    }
    out
}

/// Whether `c` can appear inside a path reference, for the START boundary.
fn is_path_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | '/')
}

/// Whether a match really ends here, given the text that follows it.
///
/// The end boundary cannot reuse [`is_path_char`], and this cost a debugging round. Treating `.`
/// as a path character rejects `intro.tmd` at the end of a sentence ("the source intro.tmd."),
/// which is far more common than the case it was protecting against. Accepting every `.` instead
/// would rewrite the stem of `intro.tmd.bak`, a genuinely different file.
///
/// So: a following alphanumeric, `-` or `_` means the match was a prefix of a longer name
/// (`intro.tmda`), and a following `.` means it too only when what comes after THAT continues the
/// name (`intro.tmd.bak`). A bare `.`, a `)`, a comma or end of line all end the reference.
fn ends_reference(rest: &str) -> bool {
    let mut chars = rest.chars();
    match chars.next() {
        None => true,
        Some(c) if c.is_alphanumeric() || c == '-' || c == '_' => false,
        Some('.') => !chars.next().is_some_and(|n| n.is_alphanumeric()),
        Some(_) => true,
    }
}

/// An LSP position, whose `character` is a UTF-16 offset rather than a byte offset.
///
/// This is not pedantry: a heading or caption above the reference is irrelevant, but a non-ASCII
/// character EARLIER ON THE SAME LINE shifts every byte offset after it, and the editor would
/// apply the edit at the wrong column.
fn utf16_position(line: &str, line_no: usize, byte: usize) -> lsp_types::Position {
    let character = line[..byte].encode_utf16().count() as u32;
    lsp_types::Position {
        line: line_no as u32,
        character,
    }
}

/// The renamed file's OWN relative references, rebased from its old directory to its new one.
///
/// `None` when the directory is unchanged: an in-place rename breaks nothing inside the file, and
/// rewriting it anyway would churn the diff and risk breaking a path that was already correct.
fn outbound_edits(old: &Path, new: &Path) -> Option<FileEdits> {
    let old_dir = old.parent()?;
    let new_dir = new.parent()?;
    if normalize(old_dir) == normalize(new_dir) {
        return None;
    }
    let text = std::fs::read_to_string(old).ok()?;
    let mut edits = Vec::new();
    for (line_no, line) in text.split('\n').enumerate() {
        for (start, reference) in outgoing_references(line) {
            // Only a relative path can break, and only one that resolves to something real: a
            // rebase computed from a target that does not exist would replace a path the author is
            // still writing with a confidently wrong one.
            if !is_relative_reference(reference) {
                continue;
            }
            let target = normalize(&old_dir.join(reference));
            if !target.exists() {
                continue;
            }
            let Some(rebased) = rel(new_dir, &target) else {
                continue;
            };
            if rebased == reference {
                continue;
            }
            edits.push(lsp_types::TextEdit {
                range: lsp_types::Range {
                    start: utf16_position(line, line_no, start),
                    end: utf16_position(line, line_no, start + reference.len()),
                },
                new_text: rebased,
            });
        }
    }
    if edits.is_empty() {
        return None;
    }
    // The edits are emitted in document order already, but a client is entitled to assume it.
    edits.sort_by_key(|e: &lsp_types::TextEdit| (e.range.start.line, e.range.start.character));
    Some(FileEdits {
        uri: lsp_types::Url::from_file_path(old).ok()?,
        edits,
    })
}

/// Whether a reference is a relative path this can rebase.
///
/// Everything excluded here would be actively damaged by a rebase: a URL is not ours, a
/// root-absolute path is deliberately anchored to the site root, and a bare `#sec-x` is an anchor
/// in the current page.
fn is_relative_reference(reference: &str) -> bool {
    !reference.is_empty()
        && !reference.starts_with('/')
        && !reference.starts_with('#')
        // A scheme (`https:`, `mailto:`, `data:`). Checked before the first `/` so that a path
        // containing a colon later on is not mistaken for one.
        && !reference
            .split_once(':')
            .is_some_and(|(head, _)| !head.contains('/') && !head.is_empty())
}

/// The outgoing path references on one line, each with its byte offset.
///
/// Two shapes carry a path: a Markdown link or image target, `](…)`, and a shortcode argument,
/// `{{< include … >}}`. A link's title (`](a.png "Caption")`) stops at the first space, which is
/// also what keeps a `](…)` containing prose from being treated as a path.
fn outgoing_references(line: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();

    let mut at = 0usize;
    while let Some(open) = line[at..].find("](").map(|i| at + i + 2) {
        let Some(close) = line[open..].find(')').map(|i| open + i) else {
            break;
        };
        let inner = &line[open..close];
        // A title after the target, and a trailing `{#fig-…}` attribute block, are not the path.
        let target = inner.split_whitespace().next().unwrap_or("");
        if !target.is_empty() {
            out.push((open, target));
        }
        at = close;
    }

    // `{{< include path >}}`, `{{< embed path >}}`, `{{< dataset path >}}`: the first argument.
    let mut at = 0usize;
    while let Some(open) = line[at..].find("{{<").map(|i| at + i + 3) {
        let Some(close) = line[open..].find(">}}").map(|i| open + i) else {
            break;
        };
        let inner = &line[open..close];
        let mut words = inner.split_whitespace();
        let name = words.next().unwrap_or("");
        if matches!(name, "include" | "embed" | "dataset")
            && let Some(arg) = words.next()
        {
            // The offset of the argument inside the line, found by searching from the name so an
            // identical string earlier on the line cannot be picked instead.
            let name_end = open + inner.find(name).unwrap_or(0) + name.len();
            if let Some(rel_at) = line[name_end..close].find(arg) {
                out.push((name_end + rel_at, arg));
            }
        }
        at = close;
    }
    out.sort_by_key(|(at, _)| *at);
    out
}

/// Every `.tmd` in the project, plus its `_site.yml`.
///
/// Skips build outputs and the freeze cache: a rename must not rewrite `_site/` or `_book/`, which
/// are generated and would be overwritten anyway, and `_freeze` holds cell output keyed by hash.
fn project_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let site_yml = root.join("_site.yml");
    if site_yml.is_file() {
        out.push(site_yml);
    }
    walk(root, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            // `_site`/`_book` are build outputs, `_freeze` is the execution cache, and a dotted
            // directory is `.git` and friends.
            if name.starts_with('.') || matches!(name.as_ref(), "_site" | "_book" | "_freeze") {
                continue;
            }
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == SOURCE_EXT) {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A scratch directory that removes itself, following the `freeze.rs` idiom (`tempfile` is not
    /// a dependency of this crate).
    struct TmpDir(PathBuf);

    impl TmpDir {
        fn new(tag: &str) -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "tali-rename-{tag}-{}-{}",
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

    fn url(p: &Path) -> lsp_types::Url {
        lsp_types::Url::from_file_path(p).unwrap()
    }

    fn rename(old: &Path, new: &Path) -> Vec<FileEdits> {
        rename_file_edits(&RenameFileEditsParams {
            files: vec![RenamedFile {
                old_uri: url(old),
                new_uri: url(new),
            }],
        })
    }

    /// Apply the returned edits and read every touched file back, so an assertion is about the
    /// resulting TEXT rather than about ranges. A range assertion passes while pointing at the
    /// wrong column, which is the failure mode `utf16_position` exists for.
    fn applied(edits: &[FileEdits]) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        for f in edits {
            let path = f.uri.to_file_path().unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
            // Later edits first, so an earlier one cannot shift the range a later one names.
            let mut sorted = f.edits.clone();
            sorted
                .sort_by_key(|e| std::cmp::Reverse((e.range.start.line, e.range.start.character)));
            for e in sorted {
                let i = e.range.start.line as usize;
                let line = &lines[i];
                // The ranges are UTF-16, so convert back the same way an editor would.
                let units: Vec<u16> = line.encode_utf16().collect();
                let head = String::from_utf16(&units[..e.range.start.character as usize]).unwrap();
                let tail = String::from_utf16(&units[e.range.end.character as usize..]).unwrap();
                lines[i] = format!("{head}{}{tail}", e.new_text);
            }
            out.push((path, lines.join("\n")));
        }
        out
    }

    /// A copy of `corpus/tarn`: 12 chapters, three parts including a nested one, and real
    /// cross-page links. Copied because the corpus walker renders every corpus document on every
    /// `cargo test`, so an in-place edit would poison every later assertion.
    fn tarn_copy(tag: &str) -> (TmpDir, PathBuf) {
        let tmp = TmpDir::new(tag);
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/tarn");
        let dst = tmp.path().join("tarn");
        copy_tree(&src, &dst);
        (tmp, dst)
    }

    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (f, t) = (e.path(), to.join(e.file_name()));
            // `_freeze` is a build artifact and can be large.
            if e.file_name() == "_freeze" {
                continue;
            }
            if f.is_dir() {
                copy_tree(&f, &t);
            } else {
                std::fs::copy(&f, &t).unwrap();
            }
        }
    }

    #[test]
    fn moving_a_file_rebases_its_own_relative_references() {
        let tmp = TmpDir::new("outbound");
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
        std::fs::create_dir_all(root.join("chapters")).unwrap();
        std::fs::create_dir_all(root.join("parts/one")).unwrap();
        std::fs::create_dir_all(root.join("_includes")).unwrap();
        std::fs::write(root.join("chapters/scree.png"), "").unwrap();
        std::fs::write(root.join("_includes/x.tmd"), "X\n").unwrap();
        let old = root.join("chapters/intro.tmd");
        std::fs::write(
            &old,
            "![A scree plot](scree.png){#fig-scree}\n\n{{< include ../_includes/x.tmd >}}\n",
        )
        .unwrap();

        let edits = rename(&old, &root.join("parts/one/intro.tmd"));
        let (_, text) = applied(&edits)
            .into_iter()
            .find(|(p, _)| p.ends_with("intro.tmd"))
            .expect("the moved file is edited");

        // chapters/ -> parts/one/ is one level up then two down.
        assert!(text.contains("](../../chapters/scree.png)"), "{text}");
        assert!(text.contains("{{< include ../../_includes/x.tmd >}}"), "{text}");
        // The caption and the label are prose and an anchor, not paths.
        assert!(text.contains("[A scree plot]"), "{text}");
        assert!(text.contains("{#fig-scree}"), "{text}");
    }

    #[test]
    fn an_in_place_rename_leaves_the_files_own_references_alone() {
        let tmp = TmpDir::new("inplace");
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
        std::fs::write(root.join("scree.png"), "").unwrap();
        let old = root.join("intro.tmd");
        // `./scree.png` is ALREADY correct, but `rel` would spell it `scree.png`, so without the
        // same-directory guard an in-place rename emits a gratuitous normalizing edit to a path
        // that was never broken. That is the difference between the guard and the
        // already-equal check, and a plainly-spelled path cannot tell them apart.
        std::fs::write(&old, "![S](./scree.png){#fig-s}\n![T](scree.png)\n").unwrap();

        let edits = rename(&old, &root.join("overview.tmd"));

        // The directory did not change, so nothing inside the file broke. Rewriting it anyway
        // would churn the diff and risk breaking a path that was already correct.
        assert!(
            !edits
                .iter()
                .any(|f| f.uri.to_file_path().unwrap().ends_with("intro.tmd")),
            "no outbound edits for a same-directory rename: {edits:?}"
        );
    }

    #[test]
    fn an_absolute_or_external_reference_is_never_rebased() {
        let tmp = TmpDir::new("external");
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("b")).unwrap();
        let old = root.join("a/p.tmd");
        // The last one is the load-bearing case: a root-absolute path that really EXISTS. The
        // others are excluded by the relative-reference filter, but they would also be skipped
        // for merely failing to resolve, so on their own they cannot prove the filter runs.
        std::fs::write(root.join("real.png"), "").unwrap();
        let abs = root.join("real.png").display().to_string();
        std::fs::write(
            &old,
            format!(
                "[x](https://example.org/x.png) [y](mailto:a@b.c) [z](#sec-here) [w]({abs})\n"
            ),
        )
        .unwrap();

        let edits = rename(&old, &root.join("b/p.tmd"));

        let moved = edits
            .iter()
            .find(|f| f.uri.to_file_path().unwrap().ends_with("p.tmd"));
        assert!(
            moved.is_none(),
            "a URL, a root-absolute path, a mailto: and a bare anchor are all left alone: {moved:?}"
        );
    }

    #[test]
    fn a_reference_that_does_not_resolve_is_left_for_the_author() {
        let tmp = TmpDir::new("dangling");
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("b")).unwrap();
        let old = root.join("a/p.tmd");
        // A path to a file that does not exist: the author is mid-sentence, or it is a typo the
        // diagnostics already report. Rebasing it would replace one wrong path with another,
        // confidently, and destroy the evidence of what they meant.
        std::fs::write(&old, "![](not-yet-drawn.png)\n").unwrap();

        let edits = rename(&old, &root.join("b/p.tmd"));
        assert!(
            !edits
                .iter()
                .any(|f| f.uri.to_file_path().unwrap().ends_with("p.tmd")),
            "an unresolvable reference is not rebased: {edits:?}"
        );
    }

    #[test]
    fn a_rename_rewrites_every_inbound_reference_in_a_real_book() {
        let (_tmp, root) = tarn_copy("book");
        // Not a hard-coded chapter name: find one the book actually links to, and assert that
        // precondition. A test that renames an UNREFERENCED file passes while proving nothing,
        // and `corpus/tarn`'s contents are free to change.
        let old = root.join("api-frame.tmd");
        let referrers: Vec<PathBuf> = project_files(&root)
            .into_iter()
            .filter(|p| {
                *p != old && std::fs::read_to_string(p).is_ok_and(|t| t.contains("api-frame.tmd"))
            })
            .collect();
        assert!(
            referrers.len() >= 2,
            "precondition: several files reference api-frame.tmd, found {referrers:?}"
        );

        let new = root.join("api-dataframe.tmd");
        let edits = rename(&old, &new);
        assert!(!edits.is_empty(), "the project walk found the referrers");

        for (path, text) in applied(&edits) {
            assert!(
                !text.contains("api-frame.tmd"),
                "{} still references the old name",
                path.display()
            );
            assert!(
                text.contains("api-dataframe.tmd"),
                "{} was rewritten",
                path.display()
            );
        }
        // The `_site.yml` chapter entry is one of them: a book whose spine still names the old
        // file drops the chapter entirely.
        assert!(
            edits
                .iter()
                .any(|f| f.uri.to_file_path().unwrap().ends_with("_site.yml")),
            "the book spine was updated"
        );
    }

    #[test]
    fn an_html_spelled_cross_page_link_is_rewritten_too() {
        let tmp = TmpDir::new("html");
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
        std::fs::write(root.join("intro.tmd"), "# Intro\n").unwrap();
        // Cross-page links are authored as `.html` (the books' own convention), so a rename that
        // only handles the `.tmd` spelling leaves a dead link and reports success.
        std::fs::write(
            root.join("two.tmd"),
            "See [intro](intro.html), or the source intro.tmd.\n",
        )
        .unwrap();

        let edits = rename(&root.join("intro.tmd"), &root.join("overview.tmd"));

        let (_, text) = applied(&edits)
            .into_iter()
            .next()
            .expect("two.tmd was edited");
        assert!(
            text.contains("(overview.html)"),
            "the .html spelling: {text}"
        );
        assert!(text.contains("overview.tmd"), "the .tmd spelling: {text}");
        assert!(
            !text.contains("intro.html"),
            "no old reference left: {text}"
        );
        assert!(!text.contains("intro.tmd"), "no old reference left: {text}");
        // The link LABEL is prose, not a path, and must survive untouched. Rewriting it would be
        // the tool editing the author's words.
        assert!(
            text.contains("[intro]"),
            "the human-readable label is left alone: {text}"
        );
    }

    #[test]
    fn a_site_yml_entry_is_edited_as_text_and_keeps_its_comments() {
        let tmp = TmpDir::new("yml");
        let root = tmp.path();
        let yml = "title: P\n# keep this comment\nchapters:\n  - intro.tmd   # and this one\n";
        std::fs::write(root.join("_site.yml"), yml).unwrap();
        std::fs::write(root.join("intro.tmd"), "# Intro\n").unwrap();

        let edits = rename(&root.join("intro.tmd"), &root.join("overview.tmd"));

        let (_, text) = applied(&edits)
            .into_iter()
            .find(|(p, _)| p.ends_with("_site.yml"))
            .expect("_site.yml was edited");
        assert!(text.contains("overview.tmd"), "{text}");
        // A YAML round-trip would reformat the file and drop both comments.
        assert!(text.contains("# keep this comment"), "{text}");
        assert!(text.contains("# and this one"), "{text}");
    }

    #[test]
    fn a_similarly_named_file_is_not_touched() {
        let tmp = TmpDir::new("prefix");
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
        std::fs::write(root.join("intro.tmd"), "# Intro\n").unwrap();
        std::fs::write(root.join("my-intro.tmd"), "# Mine\n").unwrap();
        // A plain substring search would corrupt all three of these while renaming `intro.tmd`.
        std::fs::write(
            root.join("two.tmd"),
            "[a](intro.tmd) [b](my-intro.tmd) [c](intro.tmda) [d](sub/intro.tmd)\n",
        )
        .unwrap();

        let edits = rename(&root.join("intro.tmd"), &root.join("overview.tmd"));
        let (_, text) = applied(&edits)
            .into_iter()
            .next()
            .expect("two.tmd was edited");

        assert!(
            text.contains("[a](overview.tmd)"),
            "the real reference: {text}"
        );
        assert!(text.contains("[b](my-intro.tmd)"), "a longer name: {text}");
        assert!(
            text.contains("[c](intro.tmda)"),
            "a longer extension: {text}"
        );
        assert!(
            text.contains("[d](sub/intro.tmd)"),
            "a different directory: {text}"
        );
    }

    #[test]
    fn the_end_boundary_separates_a_sentence_period_from_a_second_extension() {
        let tmp = TmpDir::new("boundary");
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
        std::fs::write(root.join("intro.tmd"), "# Intro\n").unwrap();
        // These two pull in opposite directions and a single rule cannot have both:
        // `intro.tmd.` ends a SENTENCE and must be rewritten, `intro.tmd.bak` names a DIFFERENT
        // file and must not be. Treating `.` as part of the path fails the first; ignoring it
        // fails the second.
        std::fs::write(
            root.join("two.tmd"),
            "Read the source intro.tmd.\nBut not intro.tmd.bak or intro.tmda.\n",
        )
        .unwrap();

        let edits = rename(&root.join("intro.tmd"), &root.join("overview.tmd"));
        let (_, text) = applied(&edits)
            .into_iter()
            .next()
            .expect("two.tmd was edited");

        assert!(
            text.contains("source overview.tmd."),
            "a trailing sentence period ends the reference: {text}"
        );
        assert!(
            text.contains("intro.tmd.bak"),
            "a second extension is another file: {text}"
        );
        assert!(
            text.contains("intro.tmda"),
            "a longer extension is another file: {text}"
        );
    }

    #[test]
    fn a_reference_after_a_non_ascii_character_is_ranged_in_utf16() {
        let tmp = TmpDir::new("utf16");
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
        std::fs::write(root.join("intro.tmd"), "# Intro\n").unwrap();
        // The emoji is 2 UTF-16 units and 4 bytes, and the naive maths is off by two columns. The
        // `applied` helper converts back through UTF-16, so a byte-offset range lands wrong here.
        std::fs::write(root.join("two.tmd"), "See 🎯 the [intro](intro.tmd).\n").unwrap();

        let edits = rename(&root.join("intro.tmd"), &root.join("overview.tmd"));
        let (_, text) = applied(&edits)
            .into_iter()
            .next()
            .expect("two.tmd was edited");

        assert_eq!(text.trim_end(), "See 🎯 the [intro](overview.tmd).");
    }

    #[test]
    fn a_document_under_no_site_yml_gets_no_inbound_walk() {
        let tmp = TmpDir::new("noboundary");
        let root = tmp.path();
        // No `_site.yml`: item 70 rules that such a tree declares no boundary and that inferring
        // one is the wrong move, so there is nothing to walk.
        std::fs::write(root.join("a.tmd"), "See [b](b.html).\n").unwrap();
        std::fs::write(root.join("b.tmd"), "# B\n").unwrap();

        let edits = rename(&root.join("b.tmd"), &root.join("c.tmd"));
        assert!(
            edits.is_empty(),
            "no boundary means no inbound repair: {edits:?}"
        );
    }

    #[test]
    fn an_include_and_an_embed_are_both_rewritten() {
        let tmp = TmpDir::new("shortcode");
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
        std::fs::create_dir_all(root.join("_includes")).unwrap();
        std::fs::write(root.join("_includes/bio.tmd"), "Bio.\n").unwrap();
        std::fs::write(
            root.join("page.tmd"),
            "{{< include _includes/bio.tmd >}}\n\n{{< embed _includes/bio.tmd >}}\n",
        )
        .unwrap();

        let edits = rename(
            &root.join("_includes/bio.tmd"),
            &root.join("_includes/author.tmd"),
        );
        let (_, text) = applied(&edits)
            .into_iter()
            .find(|(p, _)| p.ends_with("page.tmd"))
            .expect("page.tmd was edited");

        assert!(
            text.contains("{{< include _includes/author.tmd >}}"),
            "{text}"
        );
        assert!(
            text.contains("{{< embed _includes/author.tmd >}}"),
            "{text}"
        );
    }

    #[test]
    fn renaming_an_asset_repairs_the_pages_that_show_it() {
        let tmp = TmpDir::new("asset");
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
        std::fs::write(root.join("scree.png"), "").unwrap();
        std::fs::write(
            root.join("page.tmd"),
            "![A scree plot](scree.png){#fig-s}\n",
        )
        .unwrap();

        // An asset has no outbound references of its own, so this is the inbound half only. The
        // scan is the identical code path, which is why the widening was close to free.
        let edits = rename(&root.join("scree.png"), &root.join("eigenvalues.png"));
        let (_, text) = applied(&edits)
            .into_iter()
            .next()
            .expect("page.tmd was edited");

        assert!(text.contains("](eigenvalues.png)"), "{text}");
        // And no `.html` spelling was invented for a non-source file.
        assert!(!text.contains("eigenvalues.html"), "{text}");
    }
}
