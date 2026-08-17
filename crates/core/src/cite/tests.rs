use super::clean::clean;
use super::*;
use crate::render::{Block, Warning};
use std::collections::HashMap;

fn bib() -> Bibliography {
    parse_bib(
        "@book{bishop2006pattern,\n  title = {Pattern Recognition and Machine Learning},\n  author = {Bishop, Christopher M},\n  year = {2006},\n  publisher = {Springer}\n}\n",
    )
}

/// The `broken citation` warnings out of `process`'s output. A document citing only a
/// typo'd key legitimately draws TWO families at once — the broken reference AND the real
/// entry left uncited — so a test about one family selects it rather than asserting it is
/// the only warning.
fn broken(w: &[Warning]) -> Vec<&Warning> {
    w.iter()
        .filter(|x| x.message.contains("broken citation"))
        .collect()
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
fn a_duplicate_bib_key_keeps_the_last_definition() {
    // The duplicate-key warning promises "using the last definition" (bib_warning_located.rs
    // pins the warning and its location), but nothing rendered a duplicate-keyed entry to
    // confirm which one actually WINS. Two `@book{dup}` differ by title + year; the SECOND
    // must format. A silent flip to first-wins would keep the warning honest-looking while
    // publishing the wrong reference.
    let b = parse_bib(
        "@book{dup, title={First}, year={2001}}\n@book{dup, title={Second}, year={2002}}\n",
    );
    let f = b.format("dup").expect("the duplicate key formats");
    assert!(
        f.contains("Second") && f.contains("2002"),
        "the last definition must win: {f}"
    );
    assert!(
        !f.contains("First") && !f.contains("2001"),
        "the first definition must be superseded: {f}"
    );
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
        nested: Vec::new(),
    }];
    process(&mut blocks, &b, &HashMap::new(), None, None);
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
            nested: Vec::new(),
        }]
    };
    // A non-empty bib + an unknown key -> one "broken citation" warning.
    let mut blocks = mk();
    let w = process(&mut blocks, &bib(), &HashMap::new(), None, None);
    let w = broken(&w);
    assert_eq!(w.len(), 1, "got: {w:?}");
    assert!(w[0].message.contains("@nosuchkey"));
    // No bibliography at all -> not flagged (the missing-file case is separate).
    let mut blocks2 = mk();
    assert!(
        process(
            &mut blocks2,
            &Bibliography::default(),
            &HashMap::new(),
            None,
            None
        )
        .is_empty()
    );
}

#[test]
fn validate_xrefs_flags_only_unresolved_markers() {
    let broken = vec![Block {
        id: "x".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<a href=\"#fig-gone\" class=\"tali-xref\" data-tali-xref=\"fig-gone\">Figure</a>"
            .into(),
        cell: None,
        nested: Vec::new(),
    }];
    let w = validate_xrefs(&broken, None);
    assert_eq!(w.len(), 1, "got: {w:?}");
    assert!(w[0].message.contains("@fig-gone") && w[0].message.contains("broken cross-reference"));
    // A resolved xref (marker already rewritten away) is not flagged.
    let ok = vec![Block {
        id: "y".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<a href=\"#fig-x\" class=\"tali-xref\">Figure&nbsp;1</a>".into(),
        cell: None,
        nested: Vec::new(),
    }];
    assert!(validate_xrefs(&ok, None).is_empty());
}

/// The two halves of a cross-reference retirement, which must not drift apart. Seven of the
/// twelve prefixes name a construct nothing can define any more: `prp`/`exm`/`rem` lost
/// their theorem kinds on 2026-08-03, and `thm`/`lem`/`cor`/`def` lost theirs on 2026-08-08
/// when the theorem environments went entirely. The prefixes stay in [`XREF_LABELS`] on
/// purpose, so a leftover `@thm-a` still resolves far enough to be reported broken rather
/// than passing through as literal text (the silent fallthrough `RETIRED_DIV_CLASSES` exists
/// to prevent). But they must not be *offered*: completing `@thm-` invites the author to
/// write a reference guaranteed to break.
///
/// "Offered" reads the editor vocabulary, which is where the offer is actually made —
/// `vocab::vocab()["xrefPrefixes"]`, served to the LSP's completion. It used to read
/// `cite::xref_prefixes`, a second filtered copy that existed for the feature catalogue and
/// went with it in Wave 2.
#[test]
fn a_retired_xref_prefix_is_diagnosable_but_not_offered() {
    let offered: Vec<String> = crate::vocab::vocab()["xrefPrefixes"]
        .as_array()
        .expect("the vocabulary offers cross-reference prefixes")
        .iter()
        .map(|p| p["prefix"].as_str().unwrap_or_default().to_owned())
        .collect();
    for p in RETIRED_XREF_PREFIXES {
        assert!(
            XREF_LABELS.iter().any(|(k, _)| k == p),
            "`{p}` must stay in the label table, or a leftover `@{p}-x` goes silent"
        );
        assert!(
            !offered.iter().any(|o| o == p),
            "`{p}` must not be offered: no construct can define its target"
        );
    }
    // Positive control, so this cannot pass by both lists being empty.
    assert!(offered.iter().any(|o| o == "fig"), "offered: {offered:?}");
    assert!(XREF_LABELS.iter().any(|(k, _)| *k == "fig"));

    // The anti-silence half is behaviour, not just table membership.
    let leftover = vec![Block {
        id: "x".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<a href=\"#prp-a\" class=\"tali-xref\" data-tali-xref=\"prp-a\">Proposition</a>"
            .into(),
        cell: None,
        nested: Vec::new(),
    }];
    let w = validate_xrefs(&leftover, None);
    assert_eq!(
        w.len(),
        1,
        "a leftover retired reference is still reported: {w:?}"
    );
    assert!(w[0].message.contains("@prp-a"));
}

/// One block carrying `html`, at line 1. The did-you-mean tests only care about HTML.
fn block(html: &str) -> Block {
    Block {
        id: "x".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: html.into(),
        cell: None,
        nested: Vec::new(),
    }
}

#[test]
fn broken_citation_suggests_the_nearest_bib_key() {
    // `bishop2006patern` is one deletion away from the bib's `bishop2006pattern`.
    let mut blocks = vec![block("<p>see [@bishop2006patern].</p>")];
    let w = process(&mut blocks, &bib(), &HashMap::new(), None, None);
    let w = broken(&w);
    assert_eq!(w.len(), 1, "got: {w:?}");
    assert!(
        w[0].message
            .contains("(did you mean `@bishop2006pattern`?)"),
        "got: {}",
        w[0].message
    );
}

#[test]
fn a_citation_with_no_near_key_keeps_the_plain_message() {
    let mut blocks = vec![block("<p>see [@nosuchkey].</p>")];
    let w = process(&mut blocks, &bib(), &HashMap::new(), None, None);
    let w = broken(&w);
    assert_eq!(w.len(), 1, "got: {w:?}");
    assert!(
        w[0].message.contains("(not in the bibliography)")
            && !w[0].message.contains("did you mean"),
        "got: {}",
        w[0].message
    );
}

#[test]
fn broken_xref_suggests_the_nearest_anchor_of_the_same_kind() {
    let blocks = vec![
        block("<figure id=\"fig-results\"><img src=\"x.png\"></figure>"),
        block("<h2 id=\"sec-summary\">Summary</h2>"),
        block("<p>see <a href=\"#fig-reslts\" data-tali-xref=\"fig-reslts\">Figure</a></p>"),
    ];
    let w = validate_xrefs(&blocks, None);
    assert_eq!(w.len(), 1, "got: {w:?}");
    assert!(
        w[0].message.contains("(did you mean `@fig-results`?)"),
        "got: {}",
        w[0].message
    );
}

#[test]
fn a_broken_xref_never_suggests_an_anchor_of_a_different_kind() {
    // `sec-results` is one edit from `fig-reslts`'s stem, but a Figure is not a Section.
    let blocks = vec![
        block("<h2 id=\"sec-results\">Results</h2>"),
        block("<p>see <a href=\"#fig-reslts\" data-tali-xref=\"fig-reslts\">Figure</a></p>"),
    ];
    let w = validate_xrefs(&blocks, None);
    assert_eq!(w.len(), 1, "got: {w:?}");
    assert!(
        !w[0].message.contains("did you mean"),
        "got: {}",
        w[0].message
    );
}

#[test]
fn short_or_distant_anchor_names_get_no_suggestion() {
    // Short stems: a distance-2 edit rewrites most of the name, so `fig-c` must not
    // "suggest" `fig-a`. Distant stems: `zzzzzzz` is nobody's typo of `appendix`.
    let blocks = vec![
        block("<figure id=\"fig-a\"></figure>"),
        block("<figure id=\"fig-appendix\"></figure>"),
        block("<p><a data-tali-xref=\"fig-c\">F</a><a data-tali-xref=\"fig-zzzzzzz\">F</a></p>"),
    ];
    let w = validate_xrefs(&blocks, None);
    assert_eq!(w.len(), 2, "got: {w:?}");
    for warning in &w {
        assert!(
            !warning.message.contains("did you mean"),
            "got: {}",
            warning.message
        );
    }
}

#[test]
fn the_anchor_scan_never_harvests_a_data_block_id() {
    // `data-block-id="…"` ends in `id="`, so an unanchored substring scan would treat a
    // block's content hash as a cross-reference anchor. The values here are synthetic:
    // the trap is spelled to WIN the tie against the real anchor (`reslts2` sorts before
    // `results` at equal edit distance), so a regression cannot pass this by accident.
    let blocks = vec![
        block("<figure data-block-id=\"fig-reslts2\" id=\"fig-results\"></figure>"),
        block("<p><a data-tali-xref=\"fig-reslts\">Figure</a></p>"),
    ];
    let w = validate_xrefs(&blocks, None);
    assert_eq!(w.len(), 1, "got: {w:?}");
    assert!(
        w[0].message.contains("(did you mean `@fig-results`?)"),
        "got: {}",
        w[0].message
    );
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
        nested: Vec::new(),
    }];
    process(&mut blocks, &b, &HashMap::new(), None, None);
    // Unresolved here: linked label, marked for cross-page resolution by a site.
    assert!(
        blocks[0].html.contains(
            "<a href=\"#fig-scree\" class=\"tali-xref\" data-tali-xref=\"fig-scree\">Figure</a>"
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
        nested: Vec::new(),
    }];
    process(&mut blocks, &Bibliography::default(), &xrefs, None, None);
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
        nested: Vec::new(),
    }];
    process(&mut blocks, &b, &HashMap::new(), None, None);
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
            nested: Vec::new(),
        },
        Block {
            id: "h".into(),
            sourcepos: "3:1-3:12".into(),
            source_file: None,
            html: "<h1 id=\"references\" data-block-id=\"h\" data-sourcepos=\"3:1-3:12\">References</h1>".into(),
            cell: None,
            nested: Vec::new(),
        },
    ];
    process(&mut blocks, &b, &HashMap::new(), None, None);
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
        nested: Vec::new(),
    }];
    process(&mut blocks, &b, &HashMap::new(), None, None);
    assert!(blocks.last().unwrap().html.contains("<h2>References</h2>"));
}

#[test]
fn the_reference_list_lands_under_its_manual_heading_not_after_a_later_appendix() {
    // D69: the list used to be `push`ed unconditionally at the very END of the block
    // list. That was right by luck for the common shape (`# References` is the last
    // heading, as all three corpus documents have it), and wrong for a document that
    // keeps writing afterwards: the refs sailed past the appendix and landed under the
    // WRONG heading, orphaning the `# References` the author wrote. The heading is the
    // author's placement instruction, so honor it: insert directly after that block.
    let b = bib();
    let mut blocks = vec![
        Block {
            id: "p".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<p>see [@bishop2006pattern].</p>".into(),
            cell: None,
            nested: Vec::new(),
        },
        Block {
            id: "refs-h".into(),
            sourcepos: "3:1-3:12".into(),
            source_file: None,
            html: "<h1 id=\"references\">References</h1>".into(),
            cell: None,
            nested: Vec::new(),
        },
        Block {
            id: "appx-h".into(),
            sourcepos: "5:1-5:10".into(),
            source_file: None,
            html: "<h1 id=\"appendix\">Appendix</h1>".into(),
            cell: None,
            nested: Vec::new(),
        },
        Block {
            id: "appx-p".into(),
            sourcepos: "7:1-7:20".into(),
            source_file: None,
            html: "<p>Derivation details.</p>".into(),
            cell: None,
            nested: Vec::new(),
        },
    ];
    process(&mut blocks, &b, &HashMap::new(), None, None);

    let idx = |id: &str| {
        blocks
            .iter()
            .position(|b| b.id == id)
            .unwrap_or_else(|| panic!("block {id} vanished, blocks: {blocks:?}"))
    };
    // Directly after its heading, and strictly before the appendix that follows.
    assert_eq!(
        idx("tali-references"),
        idx("refs-h") + 1,
        "the reference list must sit directly under `# References`, blocks: {blocks:?}"
    );
    assert!(
        idx("tali-references") < idx("appx-h"),
        "the reference list must not be orphaned past a later appendix, blocks: {blocks:?}"
    );
    // The appendix keeps its own order, and nothing else moved.
    assert!(idx("appx-h") < idx("appx-p"));
    assert_eq!(idx("p"), 0);
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
        nested: Vec::new(),
    }];
    process(&mut blocks, &b, &HashMap::new(), None, None);
    assert!(
        blocks[0]
            .html
            .contains("href=\"#ref-smith.2020:v2/rev+1_a\""),
        "reference didn't read the whole key: {}",
        blocks[0].html
    );
}

#[test]
fn inproceedings_and_conference_render_booktitle_and_pages() {
    // The commonest CS/ML citation type: a paper in conference proceedings. Its
    // `booktitle` (the proceedings) + `pages` must render like a chapter, not be
    // silently dropped by the misc/online fallback.
    for kind in ["inproceedings", "conference"] {
        let src = format!(
            "@{kind}{{p,\n author = {{Vaswani, Ashish}},\n title = {{Attention Is All You Need}},\n booktitle = {{Advances in Neural Information Processing Systems}},\n pages = {{5998--6008}},\n year = {{2017}}\n}}\n"
        );
        let b = parse_bib(&src);
        let f = b.format("p").unwrap();
        assert!(
            f.contains("\u{201c}Attention Is All You Need,\u{201d}"),
            "{kind}: paper title not quoted: {f}"
        );
        assert!(
            f.contains("in <em>Advances in Neural Information Processing Systems</em>"),
            "{kind}: booktitle missing/not italic: {f}"
        );
        assert!(
            f.contains("pp. 5998\u{2013}6008"),
            "{kind}: pages dropped: {f}"
        );
        assert!(f.contains("2017"), "{kind}: year dropped: {f}");
    }
}

#[test]
fn parenthesis_delimited_entries_do_not_cascade_drop() {
    // JabRef (and older BibTeX) also emit `@type(...)` with PAREN delimiters. The
    // parser must close each entry at its matching `)`, or the field loop runs past
    // it and swallows every following `@entry` — dropping the whole rest of the file.
    let b = parse_bib(
        "@article(first,\n author = {Ada Lovelace},\n title = {First},\n journal = {J},\n year = {2020}\n)\n\n@book(second,\n author = {Alan Turing},\n title = {Second},\n publisher = {Springer},\n year = {2021}\n)\n",
    );
    let first = b.format("first").expect("paren entry #1 dropped");
    assert!(first.contains("A. Lovelace"), "got: {first}");
    let second = b
        .format("second")
        .expect("paren entry #2 cascade-dropped after entry #1");
    assert!(
        second.contains("A. Turing") && second.contains("<em>Second</em>"),
        "got: {second}"
    );
    // A paren-delimited entry followed by a brace-delimited one also stays intact.
    let mixed = parse_bib(
        "@misc(one, author = {A. One}, title = {One}, year = {2019})\n@misc{two, author = {B. Two}, title = {Two}, year = {2019}}\n",
    );
    assert!(
        mixed.format("one").is_some(),
        "paren-then-brace: #1 dropped"
    );
    assert!(
        mixed.format("two").is_some(),
        "paren-then-brace: #2 cascade-dropped"
    );
}

/// A `Block` carrying `html`, for the lint tests below (which never inspect position).
fn para(html: &str) -> Block {
    Block {
        id: "b".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: html.to_string(),
        cell: None,
        nested: Vec::new(),
    }
}

#[test]
fn a_declared_entry_that_is_never_cited_is_reported() {
    let mut blocks = vec![para("<p>Prose citing nothing.</p>")];
    let w = process(&mut blocks, &bib(), &HashMap::new(), Some(7), None);
    assert_eq!(w.len(), 1, "one warning for the set: {w:?}");
    assert!(
        w[0].message.contains("`@bishop2006pattern`") && w[0].message.contains("never cited"),
        "names the dead entry: {}",
        w[0].message
    );
    // Located on the front-matter `bibliography:` line, like every other `.bib` diagnostic
    // (a `.bib` entry has no position in the `.tmd`).
    assert_eq!(w[0].line, Some(7), "click-to-source at bibliography:");
}

#[test]
fn a_partly_cited_bibliography_reports_only_the_dead_entries() {
    // The case every real document is in: some entries cited, some not. Kept distinct from
    // the cites-nothing case on purpose — a shadowed `warnings` binding once discarded this
    // lint for exactly the pages that cite something, and every test whose page cited
    // nothing passed straight through it.
    let b = parse_bib(
        "@book{cited,\n  title = {Cited},\n  author = {A. One},\n  year = {2001}\n}\n\
         @book{dead,\n  title = {Dead},\n  author = {B. Two},\n  year = {2002}\n}\n",
    );
    let mut blocks = vec![para("<p>See [@cited].</p>")];
    let w = process(&mut blocks, &b, &HashMap::new(), Some(3), None);
    let uncited: Vec<&String> = w
        .iter()
        .map(|x| &x.message)
        .filter(|m| m.contains("never cited"))
        .collect();
    assert_eq!(uncited.len(), 1, "the dead entry is still reported: {w:?}");
    assert!(
        uncited[0].contains("`@dead`") && !uncited[0].contains("`@cited`"),
        "only the uncited one: {}",
        uncited[0]
    );
    // And the reference list still renders — the lint must not disturb the output.
    assert!(
        blocks.iter().any(|x| x.id == "tali-references"),
        "the references section is still appended"
    );
}

#[test]
fn a_cited_entry_is_not_reported_as_uncited() {
    let mut blocks = vec![para("<p>See [@bishop2006pattern].</p>")];
    let w = process(&mut blocks, &bib(), &HashMap::new(), None, None);
    assert!(
        !w.iter().any(|x| x.message.contains("never cited")),
        "a cited entry is in use: {w:?}"
    );
}

#[test]
fn the_uncited_lint_is_scoped_to_the_pages_own_layer() {
    // The whole point of the two-layer model: a project-wide entry this page ignores is
    // NOT this page's problem (some other page may cite it), so only the page's own
    // declared keys can be reported here.
    let mut shared = parse_bib(
        "@book{shared_only,\n  title = {Shared},\n  author = {A. One},\n  year = {2001}\n}\n",
    );
    shared.overlay(parse_bib(
        "@book{page_only,\n  title = {Local},\n  author = {B. Two},\n  year = {2002}\n}\n",
    ));
    let mut blocks = vec![para("<p>Prose citing nothing.</p>")];
    let w = process(&mut blocks, &shared, &HashMap::new(), None, None);
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].message.contains("`@page_only`") && !w[0].message.contains("`@shared_only`"),
        "only the page's own layer is judged here: {}",
        w[0].message
    );
}

#[test]
fn a_page_entry_overrides_a_shared_entry_with_the_same_key() {
    let mut b = parse_bib(
        "@book{k,\n  title = {From the project},\n  author = {A. One},\n  year = {2001}\n}\n",
    );
    b.overlay(parse_bib(
        "@book{k,\n  title = {From the page},\n  author = {B. Two},\n  year = {2002}\n}\n",
    ));
    let f = b.format("k").expect("the merged key formats");
    assert!(
        f.contains("From the page") && !f.contains("From the project"),
        "the page's layer wins: {f}"
    );
}

#[test]
fn many_uncited_entries_collapse_into_one_capped_message() {
    // Seven dead entries must not become seven diagnostics on one line, nor one
    // unreadable line naming all seven.
    let src: String = (0..7)
        .map(|i| format!("@book{{k{i},\n  title = {{T{i}}},\n  year = {{2000}}\n}}\n"))
        .collect();
    let mut blocks = vec![para("<p>Nothing cited.</p>")];
    let w = process(&mut blocks, &parse_bib(&src), &HashMap::new(), None, None);
    assert_eq!(w.len(), 1, "{w:?}");
    let m = &w[0].message;
    assert!(
        m.starts_with("7 bibliography entries"),
        "counts them all: {m}"
    );
    assert!(
        m.contains("`@k0`") && m.contains("`@k4`"),
        "names the first five: {m}"
    );
    assert!(
        !m.contains("`@k5`") && m.contains("and 2 more"),
        "summarizes the rest instead of listing them: {m}"
    );
}

#[test]
fn cited_keys_in_source_reads_bracketed_citations_only() {
    let keys = cited_keys_in_source(
        "Prose @notcited and [@one; @two] and [@three, p. 4] plus a ref to [@fig-x] and @four.\n",
    );
    assert_eq!(
        keys,
        vec!["one", "two", "three"],
        "bracketed citations only, cross-reference anchors excluded"
    );
    // The key-character set is shared with the BibTeX parser, so a key BibTeX accepts is a
    // key this scanner reads whole rather than truncating.
    assert_eq!(
        cited_keys_in_source("[@doe+roe:2020a]"),
        vec!["doe+roe:2020a"]
    );
}

/// A broken `@ref` is squiggled under the token, not across the line, and the
/// whole-line fallback survives for a token the scan cannot find.
///
/// **The defect (Fable audit FA30, author-observed on `corpus/diagnostics/refs.tmd:18`).**
/// The xref validator recovers its anchors from the RENDERED HTML, after the source is
/// gone, so all it could say was which block the reference was in and it filed a
/// whole-line warning. `Warning` has carried `col`/`end_col` all along and `lint.rs`'s
/// `to_lsp` maps a columned diagnostic to an exact range; only the front-matter linter used
/// it. The compounding cost was the quick fix: `to_lsp` attaches the one-click-fix payload
/// ONLY for a precisely-columned diagnostic, so the did-you-mean this message already
/// computes could never become a "Change to `@fig-results`" code action.
#[test]
fn a_broken_cross_reference_is_columned_to_its_own_token() {
    let src = "---\ntitle: T\n---\n\n# H {#sec-summary}\n\n\
               A paragraph that runs on\nand mentions @fig-reslts here.\n\n\
               ![cap](a.png){#fig-results}\n";
    let doc = crate::render_document(src);
    let warnings = validate_xrefs(&doc.blocks, Some(src));
    let w = warnings
        .iter()
        .find(|w| w.message.contains("@fig-reslts"))
        .unwrap_or_else(|| panic!("no broken-xref warning: {warnings:?}"));
    assert!(
        w.message.contains("did you mean `@fig-results`"),
        "the did-you-mean is what the fix payload is built from: {}",
        w.message
    );
    // Line 8 of the source, not line 7 where the paragraph block starts: the scan covers
    // the block's whole sourcepos span, because a reference is rarely on its first line.
    assert_eq!(w.line, Some(8), "located to the line holding the token");
    let line = src.lines().nth(7).expect("line 8");
    let (col, end_col) = (w.col.expect("a column"), w.end_col.expect("an end column"));
    assert_eq!(
        &line[col as usize - 1..end_col as usize - 1],
        "@fig-reslts",
        "the span must cover exactly the token, in line {line:?}"
    );

    // The fallback is not a formality: with no source to scan, the warning must still be
    // filed, whole-line, rather than dropped or given a guessed span.
    let blind = validate_xrefs(&doc.blocks, None);
    let w = blind
        .iter()
        .find(|w| w.message.contains("@fig-reslts"))
        .expect("still reported");
    assert_eq!((w.col, w.end_col), (None, None), "whole line, as before");
}

/// The same for a broken citation, which has the same structure and the same fix.
#[test]
fn a_broken_citation_is_columned_to_its_own_token() {
    let dir = std::env::temp_dir().join(format!("tali-cite-col-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("refs.bib"),
        "@article{knuth1984,\n title={Literate Programming},\n author={Knuth},\n year={1984}\n}\n",
    )
    .unwrap();
    let src = "---\ntitle: T\nbibliography: refs.bib\n---\n\n\
               A paragraph that runs on\nand cites [@knuth1985] here.\n";
    let doc = crate::render_document_with_includes(src, &dir);
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("broken citation"))
        .unwrap_or_else(|| panic!("no broken-citation warning: {:?}", doc.warnings));
    assert_eq!(w.line, Some(7), "located to the line holding the key");
    let line = src.lines().nth(6).expect("line 7");
    let (col, end_col) = (w.col.expect("a column"), w.end_col.expect("an end column"));
    assert_eq!(
        &line[col as usize - 1..end_col as usize - 1],
        "@knuth1985",
        "the span must cover exactly the key, in line {line:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
