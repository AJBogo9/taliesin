mod common;
use common::TempProj;

/// A typo'd cell option produces a located, click-to-source warning; the cell still
/// renders. (No kernel needed: the cell renders as source, and validation runs in the
/// render pass regardless of execution.)
#[test]
fn typo_cell_option_warns_with_location() {
    let proj = TempProj::new();
    let src = "# Title\n\nIntro.\n\n```{python}\n#| eccho: false\nprint(1)\n```\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &proj.0);
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("`eccho`"))
        .expect("a warning for the misspelled cell option");
    assert_eq!(
        w.message,
        "unknown cell option `eccho` (did you mean `echo`?)"
    );
    // The fence ```{python} is on line 5, so the option (next line) is line 6.
    assert_eq!(w.line, Some(6), "got: {w:?}");
}

/// A cell using only recognized options is silent.
#[test]
fn recognized_cell_options_do_not_warn() {
    let proj = TempProj::new();
    let src =
        "# T\n\n```{python}\n#| echo: false\n#| label: fig-x\n#| fig-cap: Cap\nprint(1)\n```\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &proj.0);
    assert!(
        !doc.warnings
            .iter()
            .any(|w| w.message.contains("cell option")),
        "no cell-option warnings expected, got: {:?}",
        doc.warnings
    );
}
