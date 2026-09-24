mod common;
use common::TempProj;

/// A typo'd cell option produces a located, click-to-source warning; the cell still
/// renders. (No kernel needed: the cell renders as source, and validation runs in the
/// render pass regardless of execution.)
#[test]
fn typo_cell_option_warns_with_location() {
    let proj = TempProj::new();
    let src = "# Title\n\nIntro.\n\n```{python}\n#| eccho: false\nprint(1)\n```\n";
    let doc = taliesin_core::render_document_scoped_with_site(src, &proj.0, None, None);
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
    let doc = taliesin_core::render_document_scoped_with_site(src, &proj.0, None, None);
    assert!(
        !doc.warnings
            .iter()
            .any(|w| w.message.contains("cell option")),
        "no cell-option warnings expected, got: {:?}",
        doc.warnings
    );
}

/// An option the cell's language never reads draws a located warning. The language comes
/// from the fence, so the same `echo: false` that hides a `{python}` cell's source is
/// reported on a `{js}` cell, whose source the page never shows while it runs.
#[test]
fn an_option_the_cell_language_never_reads_warns_with_location() {
    let proj = TempProj::new();
    let src =
        "# T\n\n```{python}\n#| echo: false\nx = 1\n```\n\n```{js}\n//| echo: false\nx\n```\n";
    let doc = taliesin_core::render_document_scoped_with_site(src, &proj.0, None, None);
    let inert: Vec<_> = doc
        .warnings
        .iter()
        .filter(|w| w.message.contains("has no effect"))
        .map(|w| (w.message.as_str(), w.line))
        .collect();
    assert_eq!(
        inert,
        vec![(
            "`echo` has no effect on a `{js}` cell: a running `{js}` cell never shows its source",
            Some(9)
        )],
        "only the {{js}} cell's echo is inert, got: {:?}",
        doc.warnings
    );
}

/// Why `include` is not reported on a `{js}` cell: a `lst-` listing of any language is
/// hidden by `include: false`, so the option is not inert there.
#[test]
fn include_false_hides_a_js_listing_and_is_not_reported() {
    let proj = TempProj::new();
    let src = "# T\n\n```{js}\n//| label: lst-a\n//| include: false\nconst secret = 1;\n```\n";
    let doc = taliesin_core::render_document_scoped_with_site(src, &proj.0, None, None);
    let html: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
    assert!(
        !html.contains("secret"),
        "include: false must hide the listing: {html}"
    );
    assert!(
        !doc.warnings
            .iter()
            .any(|w| w.message.contains("has no effect")),
        "got: {:?}",
        doc.warnings
    );
}

/// A cell whose role is a listing (`label: lst-` or an `lst-cap`, where a `fig-` label or a
/// `fig-cap` wins) shows its source whatever `echo` says, in every language, so `echo` is
/// reported there with the listing as the reason. A figure or plain `{python}` cell reads
/// `echo`, so the same line is silent on each.
#[test]
fn echo_on_a_listing_is_reported_in_any_language_and_nowhere_else() {
    let proj = TempProj::new();
    let src = "# T\n\n```{js}\n//| label: lst-a\n//| echo: false\nconst JSSECRET = 1;\n```\n\n\
               ```{python}\n#| lst-cap: A listing\n#| echo: false\nPYSECRET = 2\n```\n\n\
               ```{python}\n#| label: fig-a\n#| lst-cap: Not a listing\n#| echo: false\nx = 1\n```\n\n\
               ```{python}\n#| echo: false\ny = 1\n```\n";
    let doc = taliesin_core::render_document_scoped_with_site(src, &proj.0, None, None);
    let html: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
    assert!(
        html.contains("JSSECRET") && html.contains("PYSECRET"),
        "a listing shows its source despite `echo: false`: {html}"
    );
    let inert: Vec<_> = doc
        .warnings
        .iter()
        .filter(|w| w.message.contains("has no effect"))
        .map(|w| (w.message.as_str(), w.line))
        .collect();
    let why = "`echo` has no effect on a listing: a listing always shows its source \
               (`include: false` hides it)";
    assert_eq!(
        inert,
        [(why, Some(5)), (why, Some(11))],
        "got: {:?}",
        doc.warnings
    );
}

/// The Table arm executes only a kernel language. A `{js}` table cell never runs, so it
/// shows its source whatever `echo` says, and `echo` is reported with that reason. A
/// `{python}` table cell runs and `echo: false` hides its source, so the same line is
/// silent there.
#[test]
fn echo_on_a_table_cell_is_reported_only_where_the_table_arm_ignores_it() {
    let proj = TempProj::new();
    let src = "# T\n\n```{js}\n//| label: tbl-a\n//| echo: false\nconst JSSECRET = 1;\n```\n\n\
               ```{python}\n#| tbl-cap: A table\n#| echo: false\nPYSECRET = 2\n```\n";
    let doc = taliesin_core::render_document_scoped_with_site(src, &proj.0, None, None);
    let html: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
    assert!(
        html.contains("JSSECRET") && !html.contains("PYSECRET"),
        "the {{js}} table keeps its source and the {{python}} one hides it: {html}"
    );
    let inert: Vec<_> = doc
        .warnings
        .iter()
        .filter(|w| w.message.contains("has no effect"))
        .map(|w| (w.message.as_str(), w.line))
        .collect();
    assert_eq!(
        inert,
        [(
            "`echo` has no effect on a `{js}` table cell: it never runs, so it always shows its \
             source (`include: false` hides it)",
            Some(5)
        )],
        "got: {:?}",
        doc.warnings
    );
}

/// Render reads the first line that sets a key, so a later one is reported on its own line,
/// naming the first by the author's own line number in the author's own file. Through an
/// include, a buffer line would name the wrong line in a real, openable file.
#[test]
fn a_repeated_key_names_the_first_line_in_the_authors_own_file() {
    let proj = TempProj::new();
    proj.file("_site.yml", "title: T\n");
    proj.file(
        "_part.tmd",
        "Para.\n\n```{python}\n#| label: fig-z\n#| label: setup\nx = 1\n```\n",
    );
    let src = "# T\n\nIntro.\n\nMore.\n\n{{< include _part.tmd >}}\n";
    let doc = taliesin_core::render_document_scoped_with_site(src, &proj.0, None, None);
    let got: Vec<_> = doc
        .warnings
        .iter()
        .filter(|w| w.message.contains("repeated") || w.message.contains("setup"))
        .map(|w| {
            (
                w.message.as_str(),
                w.file.as_deref().is_some_and(|f| f.ends_with("_part.tmd")),
                w.line,
            )
        })
        .collect();
    assert_eq!(
        got,
        [(
            "repeated `label:`: only the first, on line 4, is read",
            true,
            Some(5)
        )],
        "got: {:?}",
        doc.warnings
    );
}
