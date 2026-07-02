use super::clean::clean;
use super::*;
use crate::render::Block;
use std::collections::HashMap;

fn bib() -> Bibliography {
    parse_bib(
        "@book{bishop2006pattern,\n  title = {Pattern Recognition and Machine Learning},\n  author = {Bishop, Christopher M},\n  year = {2006},\n  publisher = {Springer}\n}\n",
    )
}

#[test]
fn parses_and_formats_entry() {
    let b = bib();
    let f = b.format("bishop2006pattern").unwrap();
    assert!(f.contains("C. M. Bishop"), "got: {f}");
    assert!(f.contains("<em>Pattern Recognition and Machine Learning</em>"));
    assert!(f.contains("Springer") && f.contains("2006"));
}

#[test]
fn article_is_ieee_quoted_title_italic_journal_and_et_al() {
    let b = parse_bib(
        "@article{k,\n author = {Ziegler, Daniel M. and Stiennon, Nisan and Wu, Jeffrey and Brown, Tom B. and Radford, Alec and Amodei, Dario and Christiano, Paul and Irving, Geoffrey},\n title = {Fine-Tuning Language Models},\n journal = {arXiv preprint arXiv:1909.08593},\n year = {2019},\n url = {https://arxiv.org/abs/1909.08593}\n}\n",
    );
    let f = b.format("k").unwrap();
    // 8 authors -> first + italic et al.; article title quoted; journal italic.
    assert!(f.starts_with("D. M. Ziegler <em>et al.</em>, "), "got: {f}");
    assert!(
        f.contains("\u{201c}Fine-Tuning Language Models,\u{201d}"),
        "got: {f}"
    );
    assert!(
        f.contains("<em>arXiv preprint arXiv:1909.08593</em>, 2019."),
        "got: {f}"
    );
    assert!(
        f.contains("[Online]. Available: <a href=\"https://arxiv.org/abs/1909.08593\">"),
        "got: {f}"
    );
}

#[test]
fn book_with_edition_is_ieee_ordinal() {
    let b = parse_bib(
        "@book{r,\n author = {Russell, Stuart and Norvig, Peter},\n title = {Artificial Intelligence: A Modern Approach},\n edition = {4},\n publisher = {Pearson},\n year = {2022}\n}\n",
    );
    let f = b.format("r").unwrap();
    assert_eq!(
        f,
        "S. Russell and P. Norvig, <em>Artificial Intelligence: A Modern Approach</em>, 4th ed. Pearson, 2022."
    );
}

#[test]
fn misc_online_uses_howpublished_url_and_corporate_author() {
    let b = parse_bib(
        "@misc{w,\n author = {{Wikipedia contributors}},\n title = {Analysis of variance},\n howpublished = {\\url{https://en.wikipedia.org/wiki/Analysis_of_variance}},\n year = {2025},\n note = {Accessed: 2026-04-25}\n}\n",
    );
    let f = b.format("w").unwrap();
    // Braced corporate author stays literal (no initials); \url{} unwrapped.
    assert!(f.starts_with("Wikipedia contributors, "), "got: {f}");
    assert!(
        f.contains("\u{201c}Analysis of variance,\u{201d} 2025."),
        "got: {f}"
    );
    assert!(
        f.contains(
            "[Online]. Available: <a href=\"https://en.wikipedia.org/wiki/Analysis_of_variance\">"
        ),
        "got: {f}"
    );
    assert!(f.trim_end().ends_with("Accessed: 2026-04-25."), "got: {f}");
}

#[test]
fn and_others_collapses_to_et_al() {
    let b = parse_bib(
        "@article{o,\n author = {Ouyang, Long and Wu, Jeffrey and others},\n title = {T},\n journal = {J},\n year = {2022}\n}\n",
    );
    let f = b.format("o").unwrap();
    assert!(f.starts_with("L. Ouyang <em>et al.</em>, "), "got: {f}");
    assert!(!f.contains("others"), "literal 'others' leaked: {f}");
}

#[test]
fn citation_becomes_numbered_link_with_locator() {
    let b = bib();
    let mut blocks = vec![Block {
        id: "x".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<p>fails [@bishop2006pattern, chap. 9].</p>".into(),
        cell: None,
    }];
    process(&mut blocks, &b, &HashMap::new());
    assert!(
        blocks[0]
            .html
            .contains("[<a href=\"#ref-bishop2006pattern\">1</a>, chap. 9]")
    );
    // a References section was appended
    let refs = blocks.last().unwrap();
    assert!(refs.html.contains("id=\"ref-bishop2006pattern\""));
    assert!(refs.html.contains("[1] C. M. Bishop"));
}

#[test]
fn broken_citation_warns_only_when_a_bib_exists() {
    let mk = || {
        vec![Block {
            id: "x".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<p>see [@nosuchkey].</p>".into(),
            cell: None,
        }]
    };
    // A non-empty bib + an unknown key -> one "broken citation" warning.
    let mut blocks = mk();
    let w = process(&mut blocks, &bib(), &HashMap::new());
    assert_eq!(w.len(), 1, "got: {w:?}");
    assert!(w[0].message.contains("@nosuchkey") && w[0].message.contains("broken citation"));
    // No bibliography at all -> not flagged (the missing-file case is separate).
    let mut blocks2 = mk();
    assert!(process(&mut blocks2, &Bibliography::default(), &HashMap::new()).is_empty());
}

#[test]
fn validate_xrefs_flags_only_unresolved_markers() {
    let broken = vec![Block {
        id: "x".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<a href=\"#fig-gone\" class=\"tali-xref\" data-qmd-xref=\"fig-gone\">Figure</a>"
            .into(),
        cell: None,
    }];
    let w = validate_xrefs(&broken);
    assert_eq!(w.len(), 1, "got: {w:?}");
    assert!(w[0].message.contains("@fig-gone") && w[0].message.contains("broken cross-reference"));
    // A resolved xref (marker already rewritten away) is not flagged.
    let ok = vec![Block {
        id: "y".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<a href=\"#fig-x\" class=\"tali-xref\">Figure&nbsp;1</a>".into(),
        cell: None,
    }];
    assert!(validate_xrefs(&ok).is_empty());
}

#[test]
fn crossref_becomes_labelled_link() {
    let b = Bibliography::default();
    let mut blocks = vec![Block {
        id: "x".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<p>see @fig-scree for details</p>".into(),
        cell: None,
    }];
    process(&mut blocks, &b, &HashMap::new());
    // Unresolved here: linked label, marked for cross-page resolution by a site.
    assert!(
        blocks[0].html.contains(
            "<a href=\"#fig-scree\" class=\"tali-xref\" data-qmd-xref=\"fig-scree\">Figure</a>"
        ),
        "got: {}",
        blocks[0].html
    );
    // no citations -> no References section
    assert_eq!(blocks.len(), 1);
}

#[test]
fn crossref_resolves_number_from_registry() {
    let mut xrefs = HashMap::new();
    xrefs.insert("fig-scree".to_string(), "3".to_string());
    let mut blocks = vec![Block {
        id: "x".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<p>see @fig-scree for the elbow</p>".into(),
        cell: None,
    }];
    process(&mut blocks, &Bibliography::default(), &xrefs);
    assert!(
        blocks[0]
            .html
            .contains("<a href=\"#fig-scree\" class=\"tali-xref\">Figure&nbsp;3</a>"),
        "got: {}",
        blocks[0].html
    );
}

#[test]
fn citations_inside_code_are_left_alone() {
    let b = bib();
    let mut blocks = vec![Block {
        id: "x".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<pre><code>x = [@bishop2006pattern]</code></pre>".into(),
        cell: None,
    }];
    process(&mut blocks, &b, &HashMap::new());
    assert!(
        blocks[0].html.contains("[@bishop2006pattern]"),
        "code was rewritten"
    );
    assert_eq!(blocks.len(), 1, "no citation should have been counted");
}

// --- Lane C: `.bib` rendering fixes ---------------------------------------

#[test]
fn latex_accents_render_as_unicode() {
    // Brace-grouped umlaut, double-acute (Erdős), and standalone forms.
    assert_eq!(clean(r#"M{\"u}ller"#), "Müller");
    assert_eq!(clean(r#"Erd{\H{o}}s"#), "Erdős");
    assert_eq!(clean(r#"\'Emile"#), "Émile");
    assert_eq!(clean(r#"Caf\'e"#), "Café");
    assert_eq!(clean(r#"\`a"#), "à");
    assert_eq!(clean(r#"\^o"#), "ô");
    assert_eq!(clean(r#"\~n"#), "ñ");
    assert_eq!(clean(r#"\c{c}"#), "ç");
    assert_eq!(clean(r#"\v{s}"#), "š");
    assert_eq!(clean(r#"Stra\ss{}e"#), "Straße");
    // A control WORD (`\AA`) must be terminated by a brace or space, not run into
    // the following letters — `{\AA}rhus` / `\AA{}rhus` / `\AA rhus` are the valid
    // forms (`\AArhus` is one undefined macro in real TeX).
    assert_eq!(clean(r#"{\AA}rhus"#), "Århus");
    assert_eq!(clean(r#"\AA{}rhus"#), "Århus");
    assert_eq!(clean(r#"\o{}re"#), "øre");
    assert_eq!(clean(r#"\j"#), "ȷ");
    // Accent on a nested dotless-i/j macro (the standard `\"\i` idiom): the
    // precomposed form uses the dotted letter (`\"\i` -> ï, not ı + diaeresis).
    assert_eq!(clean(r#"Na\"\i ve"#), "Naïve");
    assert_eq!(clean(r#"\'\j"#), "j\u{301}"); // no precomposed j-acute: decomposed
    // Literal-escape macros are UNescaped, not dropped (regression: AT&T / 50% / C#).
    assert_eq!(clean(r#"AT\&T"#), "AT&T");
    assert_eq!(clean(r#"50\% off"#), "50% off");
    assert_eq!(clean(r#"C\#"#), "C#");
    assert_eq!(clean(r#"foo\_bar"#), "foo_bar");
    assert_eq!(clean(r#"\$5"#), "$5");
    // Author formatting routes through clean(): accents survive initialization.
    let b = parse_bib(
        "@article{m,\n author = {M{\\\"u}ller, Hans and Erd{\\H{o}}s, P{\\'a}l},\n title = {T},\n journal = {J},\n year = {2020}\n}\n",
    );
    let f = b.format("m").unwrap();
    assert!(f.starts_with("H. Müller and P. Erdős, "), "got: {f}");
}

#[test]
fn corporate_brace_author_stays_whole() {
    // The DOUBLE brace `{{...}}` is the BibTeX corporate marker: rendered whole.
    let b = parse_bib(
        "@misc{who,\n author = {{World Health Organization}},\n title = {Guidelines},\n year = {2021}\n}\n",
    );
    let f = b.format("who").unwrap();
    assert!(
        f.starts_with("World Health Organization, "),
        "corporate author was split/initialized: {f}"
    );
    assert!(!f.contains("W. H. Organization"), "got: {f}");
}

#[test]
fn single_brace_first_last_author_is_still_initialized() {
    // Regression guard: a single-brace `{First Last}` is an ordinary author and
    // MUST initialize (it is NOT corporate — only `{{...}}` is). Without this,
    // existing corpus entries like `{Umar Jamil}` regressed to "Umar Jamil".
    let b = parse_bib("@misc{j,\n author = {Umar Jamil},\n title = {T},\n year = {2023}\n}\n");
    let f = b.format("j").unwrap();
    assert!(f.starts_with("U. Jamil, "), "got: {f}");
}

#[test]
fn string_macros_are_resolved_and_substituted() {
    let b = parse_bib(
        "@string{springer = \"Springer-Verlag\"}\n@string{jmlr = \"Journal of Machine Learning Research\"}\n@book{x,\n author = {Doe, Jane},\n title = {A Book},\n publisher = springer,\n year = {2020}\n}\n@article{y,\n author = {Roe, Rich},\n title = {A Paper},\n journal = jmlr,\n year = {2021}\n}\n",
    );
    let fb = b.format("x").unwrap();
    assert!(fb.contains("Springer-Verlag"), "got: {fb}");
    let fa = b.format("y").unwrap();
    assert!(
        fa.contains("<em>Journal of Machine Learning Research</em>"),
        "got: {fa}"
    );
}

#[test]
fn inbook_and_incollection_render_booktitle_and_pages() {
    let b = parse_bib(
        "@incollection{c,\n author = {Bengio, Yoshua},\n title = {Practical Recommendations},\n booktitle = {Neural Networks: Tricks of the Trade},\n pages = {437--478},\n publisher = {Springer},\n year = {2012}\n}\n",
    );
    let f = b.format("c").unwrap();
    assert!(
        f.contains("\u{201c}Practical Recommendations,\u{201d}"),
        "chapter title not quoted: {f}"
    );
    assert!(
        f.contains("in <em>Neural Networks: Tricks of the Trade</em>"),
        "booktitle missing/not italic: {f}"
    );
    assert!(f.contains("pp. 437\u{2013}478"), "pages dropped: {f}");
    assert!(f.contains("Springer") && f.contains("2012"), "got: {f}");
}

#[test]
fn manual_references_heading_suppresses_auto_heading() {
    let b = bib();
    let mut blocks = vec![
        Block {
            id: "p".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<p>see [@bishop2006pattern].</p>".into(),
            cell: None,
        },
        Block {
            id: "h".into(),
            sourcepos: "3:1-3:12".into(),
            source_file: None,
            html: "<h1 id=\"references\" data-block-id=\"h\" data-sourcepos=\"3:1-3:12\">References</h1>".into(),
            cell: None,
        },
    ];
    process(&mut blocks, &b, &HashMap::new());
    let refs = blocks.last().unwrap();
    // The list + anchors are still emitted...
    assert!(
        refs.html.contains("id=\"ref-bishop2006pattern\""),
        "got: {}",
        refs.html
    );
    assert!(
        refs.html.contains("class=\"tali-references\""),
        "got: {}",
        refs.html
    );
    // ...but the auto <h2>References</h2> is suppressed (the manual one stands).
    assert!(
        !refs.html.contains("<h2>References</h2>"),
        "auto References heading should be suppressed when a manual one exists: {}",
        refs.html
    );
    // Exactly one "References" heading remains across all blocks.
    let count: usize = blocks
        .iter()
        .map(|b| b.html.matches("References</h").count())
        .sum();
    assert_eq!(
        count, 1,
        "expected one References heading, blocks: {blocks:?}"
    );
}

#[test]
fn no_manual_heading_keeps_auto_references_heading() {
    // Regression guard: without a manual heading, the auto <h2> stays.
    let b = bib();
    let mut blocks = vec![Block {
        id: "p".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<p>see [@bishop2006pattern].</p>".into(),
        cell: None,
    }];
    process(&mut blocks, &b, &HashMap::new());
    assert!(blocks.last().unwrap().html.contains("<h2>References</h2>"));
}

#[test]
fn url_macro_unwraps_and_keeps_underscores_without_mangling_words() {
    // \url{...} resolves to its argument with underscores intact (not read as \_),
    // via the generic unknown-macro path (the old naive `replace("\\url","")` both
    // deleted a bare \url and corrupted any word merely CONTAINING the substring).
    assert_eq!(clean(r"\url{http://a.com/x_y}"), "http://a.com/x_y");
    assert_eq!(
        clean(r"See \url{http://a.com/p_q} now"),
        "See http://a.com/p_q now"
    );
    assert_eq!(clean(r"\urlstyle{same}"), "same"); // not "stylesame"
}

#[test]
fn quoted_single_brace_author_is_initialized_like_the_brace_form() {
    // author = "{First Last}" is an ordinary (case-protected) person, NOT corporate:
    // the `"..."` arm now strips one outer brace level like the `{..}` arm, so it
    // initializes rather than rendering whole.
    let b =
        parse_bib("@misc{q,\n author = \"{Ada Lovelace}\",\n title = {T},\n year = {2020}\n}\n");
    let f = b.format("q").unwrap();
    assert!(
        f.starts_with("A. Lovelace, "),
        "quoted single-brace author not initialized: {f}"
    );
}

#[test]
fn quoted_double_brace_author_stays_corporate() {
    // Consistency: `"{{Corp}}"` keeps one brace pair after the single strip, so it is
    // still literal, exactly like the `{{Corp}}` (brace-delimited) form.
    let b = parse_bib(
        "@misc{q2,\n author = \"{{Open Data Institute}}\",\n title = {T},\n year = {2020}\n}\n",
    );
    let f = b.format("q2").unwrap();
    assert!(f.starts_with("Open Data Institute, "), "got: {f}");
    assert!(!f.contains("O. D. Institute"), "got: {f}");
}

#[test]
fn cite_key_and_bib_key_charsets_agree() {
    // A key using every allowed special char must (a) parse into the bib WHOLE and
    // (b) be read WHOLE from prose — both sides share one `is_cite_key_char`.
    let key = "smith.2020:v2/rev+1_a";
    let src = "@article{".to_string()
        + key
        + ",\n author = {Smith, Jo},\n title = {T},\n journal = {J},\n year = {2020}\n}\n";
    let b = parse_bib(&src);
    assert!(
        b.format(key).is_some(),
        "bib parser truncated the special-char key"
    );
    let mut blocks = vec![Block {
        id: "p".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: format!("<p>see [@{key}].</p>"),
        cell: None,
    }];
    process(&mut blocks, &b, &HashMap::new());
    assert!(
        blocks[0]
            .html
            .contains("href=\"#ref-smith.2020:v2/rev+1_a\""),
        "reference didn't read the whole key: {}",
        blocks[0].html
    );
}
