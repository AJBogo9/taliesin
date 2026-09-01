//! `lsp_pos::lines` is the one allowed way to split a buffer in the LSP: CommonMark
//! ends a line at a lone carriage return too, so a bare newline split desyncs every
//! index after the first stray CR (CLAUDE.md's LSP hazards). Five such sites re-entered
//! production code between 2026-08-13 and 2026-09-01 with every gate green, so the ban
//! gets a committed instrument: scan each lsp_*.rs file's PRODUCTION region (everything
//! before its `mod tests`). `lsp_pos.rs` is exempt — it is the implementation the ban
//! points to.

const FILES: &[(&str, &str)] = &[
    ("lsp.rs", include_str!("../src/lsp.rs")),
    ("lsp_cells.rs", include_str!("../src/lsp_cells.rs")),
    ("lsp_complete.rs", include_str!("../src/lsp_complete.rs")),
    ("lsp_diag.rs", include_str!("../src/lsp_diag.rs")),
    ("lsp_fold.rs", include_str!("../src/lsp_fold.rs")),
    ("lsp_nav.rs", include_str!("../src/lsp_nav.rs")),
    ("lsp_outline.rs", include_str!("../src/lsp_outline.rs")),
    ("lsp_project.rs", include_str!("../src/lsp_project.rs")),
];

#[test]
fn no_lsp_production_code_splits_a_buffer_on_a_bare_newline() {
    // The two banned spellings as they appear in source text.
    let needles = ["split('\\n')", "split(\"\\n\")"];
    // Known-positive control: the scan must be reading real production text, and the
    // mandated helper must actually be in use somewhere it scans.
    assert!(
        FILES.iter().any(|(_, s)| s
            .split("mod tests")
            .next()
            .unwrap()
            .contains("lsp_pos::lines")),
        "control failed: no scanned production region references lsp_pos::lines"
    );
    for (name, src) in FILES {
        let prod = src.split("mod tests").next().unwrap();
        for needle in &needles {
            assert!(
                !prod.contains(needle),
                "{name} production code splits a buffer on a bare newline (`{needle}`): \
                 route it through `lsp_pos::lines` instead"
            );
        }
    }
}
