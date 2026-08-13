//! What a buffer's diagnostics are, and which of them a hover is about.
//!
//! `publishDiagnostics` is the only transport: the server pushes a buffer's findings as the
//! author types, and an editor showing them is showing what the pre-publish lint would say.
//! The 3.17 **pull** model (`textDocument/diagnostic` + `workspace/diagnostic`) was the other
//! half of this file and went on 2026-08-08; a whole-book answer is `build <dir> --check-only`,
//! from a terminal, which is where the author already runs it before publishing.
//!
//! The hover used to carry a second body: a `TAL-*` code's catalogued cause and canonical fix,
//! the same rows `check --explain` printed. Both went on 2026-08-08 with the code catalogue.
//! What replaces it is the diagnostic message itself, which names the fix inline (a
//! did-you-mean, or a retirement note out of the register), so there is one text to keep true
//! instead of two.

use std::path::Path;

/// The narrowest range among our diagnostics covering `pos`, so a hover that carries *only* an
/// explanation still highlights the token it is about rather than nothing.
pub(crate) fn narrowest_range_at(
    diagnostics: &[lsp_types::Diagnostic],
    pos: lsp_types::Position,
) -> Option<lsp_types::Range> {
    diagnostics
        .iter()
        .filter(|d| d.source.as_deref() == Some(crate::lint::LSP_SOURCE))
        .filter(|d| d.range.start <= pos && pos <= d.range.end)
        .min_by_key(|d| {
            (
                d.range.end.line - d.range.start.line,
                d.range
                    .end
                    .character
                    .saturating_sub(d.range.start.character),
            )
        })
        .map(|d| d.range)
}

/// Lint one file — an open buffer if the server holds one, otherwise the file on disk — as the
/// site page it actually is.
///
/// The one place a buffer becomes diagnostics, so what the editor squiggles and what
/// `--check-only` reports cannot drift apart.
pub(crate) fn diagnose_file(
    sites: &mut crate::lsp_project::SiteCache,
    path: &Path,
    text: &str,
) -> Vec<lsp_types::Diagnostic> {
    let lines: Vec<&str> = crate::lsp_pos::lines(text).collect();
    let site = sites.get(path);
    crate::lint::buffer_diagnostics_in_site(path, text, site)
        .iter()
        .map(|d| d.to_lsp(&lines))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Diagnostic, Position, Range};

    /// A diagnostic of ours with a `line`-local `[start, end)` range.
    fn diag(line: u32, start: u32, end: u32) -> Diagnostic {
        Diagnostic {
            range: Range::new(Position::new(line, start), Position::new(line, end)),
            source: Some(crate::lint::LSP_SOURCE.to_string()),
            message: "m".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn the_narrowest_range_is_the_one_a_hover_highlights() {
        let r = narrowest_range_at(&[diag(0, 0, 40), diag(0, 4, 9)], Position::new(0, 5))
            .expect("a covering range");
        assert_eq!((r.start.character, r.end.character), (4, 9));
    }

    /// Another provider's diagnostic is not ours to anchor a hover on. An editor attaches
    /// several to the same buffer, and the `source` guard is the only thing separating them.
    #[test]
    fn only_our_own_diagnostics_anchor_a_hover() {
        let mut theirs = diag(0, 0, 5);
        theirs.source = Some("eslint".to_string());
        assert!(narrowest_range_at(&[theirs], Position::new(0, 2)).is_none());
    }

    #[test]
    fn a_position_outside_every_range_anchors_nothing() {
        assert!(narrowest_range_at(&[diag(3, 0, 5)], Position::new(0, 2)).is_none());
    }

    /// The line a squiggle lands on is comrak's line number read against **our** index of the
    /// buffer, so the two have to count lines the same way. CommonMark ends a line at a lone
    /// `\r` and `text.split('\n')` did not: in the buffer below (three CommonMark lines, not
    /// one `\n` in it) the cross-reference error is on line 3, the split saw a single line,
    /// and `to_lsp` clamped the squiggle onto the empty tail of the buffer at zero width.
    #[test]
    fn a_lone_cr_does_not_move_the_line_a_diagnostic_lands_on() {
        let mut sites = crate::lsp_project::SiteCache::new();
        // No `_site.yml` above it, so the buffer is linted standalone and nothing is read
        // from disk; the path only has to be a `.tmd` somewhere.
        let path = std::env::temp_dir().join(format!("tali-lspcr-{}.tmd", std::process::id()));
        let text = "para one\r\rSee @fig-nope for details.\n";
        let diags = diagnose_file(&mut sites, &path, text);
        let d = diags
            .first()
            .expect("the broken cross-reference is the diagnostic under test");
        assert_eq!(
            (d.range.start.line, d.range.end.line),
            (2, 2),
            "0-based line of `See @fig-nope …`, got {:?} for {:?}",
            d.range,
            d.message
        );
        assert!(
            d.range.end.character > d.range.start.character,
            "a squiggle the author can see, got {:?}",
            d.range
        );
    }
}
