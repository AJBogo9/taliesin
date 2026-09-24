use super::clean::clean;
use super::*;
use crate::render::{Block, Warning};
use std::collections::HashMap;

fn bib() -> Bibliography {
    parse_bib(
        "@book{bishop2006pattern,\n  title = {Pattern Recognition and Machine Learning},\n  author = {Bishop, Christopher M},\n  year = {2006},\n  publisher = {Springer}\n}\n",
    )
}

/// The `broken citation` warnings out of `process`'s output. Kept as a filter rather than
/// asserting on the whole list: a document citing a typo'd key drew TWO families at once
/// until the "declared but never cited" lint was cut on 2026-08-20, and selecting the
/// family under test is still the honest shape for a test about one of them.
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
    process(&mut blocks, &b, &HashMap::new(), None);
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
    let w = process(&mut blocks, &bib(), &HashMap::new(), None);
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

/// What resolves and what is offered are ONE list, by construction. [`XREF_LABELS`] used to
/// carry seven extra theorem prefixes that `vocab` subtracted back out again, so the two
/// answers were kept equal by a filter; the tuples went on 2026-08-18 and the filter with
/// them, and this pins the equality that replaced it. A prefix added to the table without a
/// construct that can define its target now shows up here as an offer nothing can satisfy.
#[test]
fn every_prefix_that_resolves_is_also_offered_and_the_reverse() {
    let offered: Vec<String> = crate::vocab::vocab()["xrefPrefixes"]
        .as_array()
        .expect("the vocabulary offers cross-reference prefixes")
        .iter()
        .map(|p| p["prefix"].as_str().unwrap_or_default().to_owned())
        .collect();
    let resolving: Vec<String> = XREF_LABELS.iter().map(|(k, _)| (*k).to_owned()).collect();
    assert_eq!(offered, resolving, "the two lists are the same list");
    // Positive control, so this cannot pass by both lists being empty.
    assert!(offered.iter().any(|o| o == "fig"), "offered: {offered:?}");
}

/// The seven theorem prefixes are gone from the READ, not merely from the vocabulary.
/// Dropping a name from a table only makes it undiagnosed; the parser going on honouring it
/// is what leaves a withdrawn construct quietly working, so the pin has to be behavioural.
///
/// A `@thm-a` is now literal text that reports nothing — the same treatment `@figg-scree`
/// (a typo) and `@Fig-scree` (wrong case) have always had. That silence is the deliberate
/// consequence recorded on [`XREF_LABELS`]: an unknown prefix cannot be diagnosed without
/// false-firing on ordinary prose like `@rust-lang`.
#[test]
fn a_withdrawn_theorem_prefix_is_no_longer_read_at_all() {
    for prefix in ["thm", "lem", "cor", "def", "prp", "exm", "rem"] {
        assert!(
            !crate::cite::is_xref_anchor(&format!("{prefix}-a")),
            "`{prefix}-a` must no longer be a cross-reference anchor shape"
        );
        let doc = crate::render_document_with_includes(
            &format!("---\ntitle: T\n---\n\nSee @{prefix}-a.\n"),
            std::path::Path::new("."),
        );
        let html: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
        assert!(
            html.contains(&format!("@{prefix}-a")) && !html.contains("tali-xref"),
            "`@{prefix}-a` must stay literal text, not a cross-reference link: {html}"
        );
        assert!(
            !doc.warnings
                .iter()
                .any(|w| w.message.contains("cross-reference")),
            "an unknown prefix reports nothing, like any other unknown word: {:?}",
            doc.warnings
        );
    }
    // Positive control: a LIVE prefix with no target still errors, so the silence above is
    // about the prefix being unknown and not about the check having stopped running.
    let live = crate::render_document_with_includes(
        "---\ntitle: T\n---\n\nSee @fig-nope.\n",
        std::path::Path::new("."),
    );
    let w = validate_xrefs(&live.blocks, None);
    assert!(
        w.iter().any(|x| x.message.contains("@fig-nope")),
        "a dangling LIVE reference is still reported: {w:?}"
    );
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
    let w = process(&mut blocks, &bib(), &HashMap::new(), None);
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
    let w = process(&mut blocks, &bib(), &HashMap::new(), None);
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
    process(&mut blocks, &b, &HashMap::new(), None);
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
    process(&mut blocks, &Bibliography::default(), &xrefs, None);
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
    process(&mut blocks, &b, &HashMap::new(), None);
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

/// A corporate author whose own name contains " and " was split by the author-list
/// separator and each half then formatted as a person: `{{Food and Drug Administration}}`
/// published as "Food and D. Administration". Silent — the key resolves, no validator reads
/// a formatted name — and it hits any agency spelled this way (FDA, NIST's parent, "Centers
/// for Disease Control and Prevention"). The separator only separates at brace depth 0.
#[test]
fn a_corporate_author_containing_and_is_one_author() {
    for (key, name) in [
        ("fda", "Food and Drug Administration"),
        ("cdc", "Centers for Disease Control and Prevention"),
    ] {
        let b = parse_bib(&format!(
            "@misc{{{key},\n author = {{{{{name}}}}},\n title = {{T}},\n year = {{2020}}\n}}\n"
        ));
        let f = b.format(key).unwrap();
        assert!(
            f.starts_with(&format!("{name}, ")),
            "corporate author was split on its own conjunction: {f}"
        );
        assert!(!f.contains("D. Administration"), "got: {f}");
        assert!(!f.contains("P. Prevention"), "got: {f}");
    }
    // The separator still separates real co-authors, including beside a corporate one.
    let b = parse_bib(
        "@misc{mix,\n author = {{{Food and Drug Administration}} and Doe, Jane},\n title = {T},\n year = {2020}\n}\n",
    );
    let f = b.format("mix").unwrap();
    assert!(
        f.starts_with("Food and Drug Administration and J. Doe, "),
        "got: {f}"
    );
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
    process(&mut blocks, &b, &HashMap::new(), None);
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
    process(&mut blocks, &b, &HashMap::new(), None);
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
    process(&mut blocks, &b, &HashMap::new(), None);

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
    process(&mut blocks, &b, &HashMap::new(), None);
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

/// An entry whose closing `}` was lost ends where the next entry starts, and says so.
///
/// It used to swallow that next entry whole: the field loop read `@article{smith2020` as a
/// field name, found no `=`, broke, and the outer scan resumed past the `@` it had already
/// consumed. `[@smith2020]` then became a broken citation whose did-you-mean offered
/// `@smith2019` as a one-click fix, which cites a different paper (audit 2026-09-24 G3).
#[test]
fn an_unclosed_entry_ends_where_the_next_one_starts_and_is_reported() {
    let (b, w) = parse_bib_warned(
        "@article{smith2019,\n  author = {Smith, John},\n  title = {First},\n  year = {2019}\n\n\
         @article{smith2020,\n  author = {Smith, John},\n  title = {Second},\n  year = {2020}\n}\n",
    );
    let second = b.format("smith2020").expect("the next entry survives");
    assert!(
        second.contains("Second") && second.contains("2020"),
        "{second}"
    );
    let first = b
        .format("smith2019")
        .expect("the unclosed entry keeps what it read");
    assert!(first.contains("First") && first.contains("2019"), "{first}");
    assert!(
        w.iter()
            .any(|m| m.contains("smith2019") && m.contains("not closed") && m.contains("line 1")),
        "the lost brace is reported, naming the entry and its line: {w:?}"
    );

    // An unbalanced `{` inside a value is the same failure one level down: the value runs
    // on until the next entry starts, and that entry must still be read.
    let (b, w) = parse_bib_warned(
        "@article{good1, title={Before}, year={2000}}\n\n\
         @article{broken, title={Missing close {brace}, year={2001}}\n\n\
         @article{good2, title={After}, year={2002}}\n",
    );
    assert!(
        b.format("good2").is_some_and(|f| f.contains("After")),
        "good2 lost"
    );
    assert!(
        w.iter()
            .any(|m| m.contains("broken") && m.contains("not closed")),
        "{w:?}"
    );

    // Reaching the end of the file inside an entry is reported too.
    let (_, w) = parse_bib_warned("@article{last, title={T}, year={2002}\n");
    assert!(
        w.iter()
            .any(|m| m.contains("last") && m.contains("not closed")),
        "{w:?}"
    );

    // A well-formed file draws nothing.
    let (_, w) = parse_bib_warned(
        "@article{a, title={A}, year={1}}\n@misc(b, title={B})\n@string{j = {J}}\n",
    );
    assert!(w.is_empty(), "{w:?}");
}

/// A key with a character `[@…]` cannot name is reported and not stored, instead of being
/// stored truncated with no fields (audit 2026-09-24, bibtex #14).
///
/// The key read used to stop at the first character `is_cite_key_char` rejects, so
/// `smith&jones2020` became the key `smith` with an empty entry: citing it printed an empty
/// reference row, and the phantom replaced a real `smith` entry defined earlier.
#[test]
fn a_key_the_citation_syntax_cannot_name_is_reported_not_stored_truncated() {
    let (b, w) = parse_bib_warned(
        "@misc{smith, title={Real smith}, year={2019}}\n\
         @misc{smith&jones2020, title={Ampersand}, year={2020}}\n\
         @misc{o'brien2020, title={Apostrophe}, year={2020}}\n",
    );
    let smith = b.format("smith").expect("the real entry");
    assert!(
        smith.contains("Real smith"),
        "no phantom replaces it: {smith}"
    );
    assert!(b.format("o").is_none(), "no truncated key is stored");
    for (key, line) in [("smith&jones2020", "line 2"), ("o'brien2020", "line 3")] {
        assert!(
            w.iter()
                .any(|m| m.contains(key) && m.contains("cannot be cited") && m.contains(line)),
            "{key}: {w:?}"
        );
    }
    assert!(!w.iter().any(|m| m.contains("duplicate")), "{w:?}");
}

/// A name is a literal (corporate) name only when the WHOLE name is one brace group.
///
/// The test used to be "the name starts with `{`", and that is exactly how every exporter
/// writes an accent on a name's first letter: Google Scholar `{\"O}zt{\"u}rk`, DBLP
/// `{\"{O}}zt{\"{u}}rk`, Better BibTeX `{\"O}`. Those names were published unformatted,
/// "Öztürk, Ayşe", in the middle of an IEEE list (audit 2026-09-24 G1).
#[test]
fn a_name_starting_with_a_braced_accent_is_still_a_person() {
    let cases = [
        // Google Scholar and Better BibTeX, comma form.
        (r#"{\"O}zt{\"u}rk, Ay{\c{s}}e"#, "A. Öztürk"),
        (r#"{\O}rsted, Hans"#, "H. Ørsted"),
        (r#"{\AA}ngstr{\"o}m, Anders"#, "A. Ångström"),
        (r#"{\v{S}}koda, Emil"#, "E. Škoda"),
        (r#"{\"O}zt{\"u}rk, {\c{S}}ule"#, "Ş. Öztürk"),
        // DBLP's doubly braced accents.
        (r#"{\"{O}}zt{\"{u}}rk, Ay{\c{s}}e"#, "A. Öztürk"),
        // Better BibTeX writes the cedilla with a space inside the group, which is one
        // word, not two initials.
        (r#"{\"O}zt{\"u}rk, Ay{\c s}e"#, "A. Öztürk"),
        // First-Last form, the accent on the given name.
        (r#"{\'E}mile Durkheim"#, "É. Durkheim"),
        (r#"{\'{A}}lvaro Garc{\'\i}a"#, "Á. García"),
    ];
    for (raw, want) in cases {
        assert_eq!(super::author::format_authors(raw), want, "{raw}");
    }
    // The corporate marker still holds: one group around the whole name.
    assert_eq!(
        super::author::format_authors("{World Health Organization}"),
        "World Health Organization"
    );
}

/// BibTeX's von rule for the "First von Last" order that DBLP and arXiv use for every
/// name: the surname starts at the first lowercase word (not the last word), so the
/// particle is printed as written instead of being turned into initials. `Laurens van der
/// Maaten` published as "L. V. D. Maaten" (audit 2026-09-24 G2).
#[test]
fn a_lowercase_particle_in_first_last_order_is_part_of_the_surname() {
    let cases = [
        // DBLP.
        (
            "Laurens van der Maaten and Geoffrey E. Hinton",
            "L. van der Maaten and G. E. Hinton",
        ),
        (r#"A{\"{a}}ron van den Oord"#, "A. van den Oord"),
        ("Hado van Hasselt", "H. van Hasselt"),
        ("Ulrike von Luxburg", "U. von Luxburg"),
        ("Nando de Freitas", "N. de Freitas"),
        ("Jean de la Fontaine", "J. de la Fontaine"),
        ("Ludwig van Beethoven", "L. van Beethoven"),
        // No particle: the last word is the surname, as before.
        ("Geoffrey E. Hinton", "G. E. Hinton"),
        // The comma forms already kept the particle, and still do.
        ("van Beethoven, Ludwig", "L. van Beethoven"),
        ("Van der Maaten, Laurens", "L. Van der Maaten"),
    ];
    for (raw, want) in cases {
        assert_eq!(super::author::format_authors(raw), want, "{raw}");
    }
}

/// The rest of BibTeX's name grammar that exporters rely on (audit 2026-09-24, bibtex #3):
/// the three-part "von Last, Jr, First" form (Better BibTeX), hyphenated given names
/// (`Klaus-Robert`, DBLP's `Ming{-}Wei`), a tie between initials, and an ` AND ` in
/// capitals, which BibTeX reads case-insensitively.
#[test]
fn bibtex_name_forms_the_exporters_use_are_split_like_bibtex() {
    let cases = [
        ("King, Jr., Martin Luther", "M. L. King, Jr."),
        (r#"Klaus-Robert M{\"u}ller"#, "K.-R. Müller"),
        (
            r#"M{\"u}ller, Klaus-Robert and Serre, Jean-Pierre"#,
            "K.-R. Müller and J.-P. Serre",
        ),
        ("Ming{-}Wei Chang", "M.-W. Chang"),
        ("D.~E. Knuth", "D. E. Knuth"),
        ("Smith, John AND Doe, Jane", "J. Smith and J. Doe"),
        ("SMITH AND JONES", "SMITH and JONES"),
        // Unchanged: a Jr written inside the surname stays there.
        ("Steele Jr, Guy L", "G. L. Steele Jr"),
    ];
    for (raw, want) in cases {
        assert_eq!(super::author::format_authors(raw), want, "{raw}");
    }
}

/// A LaTeX control word the cleaner does not know is kept, not deleted, and a math span
/// is left as the TeX the author wrote (audit 2026-09-24 G5).
///
/// Unknown macros used to be dropped with their name, which is harmless for a formatting
/// command whose argument follows (`\emph{x}`) and destroys text for everything else:
/// arXiv titles keep their math verbatim, so `{$\alpha$}-Synuclein` published as
/// "$$-Synuclein", `$O(n \log n)$` lost its `\log`, and `The {\TeX}book` became "The book".
#[test]
fn unknown_control_words_and_math_are_kept_not_deleted() {
    let cases = [
        // Math spans are verbatim, braces and all.
        (r"{$\alpha$}-Synuclein", r"$\alpha$-Synuclein"),
        (r"{\(\ell_1\)}-Regularized", r"\(\ell_1\)-Regularized"),
        (
            r"An {$O(n \log n)$} Algorithm",
            r"An $O(n \log n)$ Algorithm",
        ),
        (r"$\frac{a}{b}$-norm", r"$\frac{a}{b}$-norm"),
        (r"{\ensuremath{\beta}}-VAE", r"$\beta$-VAE"),
        // An escaped dollar is a dollar, not math.
        (r"\$5 and \$6", "$5 and $6"),
        // The symbols and logos exporters write.
        (r"The {\TeX}book", "The TeXbook"),
        (r"\LaTeX{} and \BibTeX", "LaTeX and BibTeX"),
        (
            r"Deep Learning \textendash{} A Survey",
            "Deep Learning \u{2013} A Survey",
        ),
        (r"1990\textemdash{}2000", "1990\u{2014}2000"),
        (r"Alzheimer\textquoteright{}s", "Alzheimer\u{2019}s"),
        (
            r"37{\textdegree}C and 5{\texttimes}",
            "37\u{b0}C and 5\u{d7}",
        ),
        (r"Wait\ldots", "Wait\u{2026}"),
        (
            r"\S 3, \copyright{} 2020, \textregistered",
            "\u{a7}3, \u{a9} 2020, \u{ae}",
        ),
        (r"a \textless{} b \textgreater{} c", "a < b > c"),
        // Declarations that print nothing still print nothing.
        (r"{\em Emphasised} and {\sc Caps}", "Emphasised and Caps"),
        // A command with an argument keeps its argument, as before.
        (r"\emph{Deep} \textit{learning}", "Deep learning"),
        // Anything else stays visible, so the author sees what was not understood.
        (r"a \foo b", r"a \foo b"),
        (r"a \foo{} b", r"a \foo b"),
    ];
    for (raw, want) in cases {
        assert_eq!(clean(raw), want, "{raw}");
    }
}

/// DBLP writes an accented i as `{\'{\i}}`: the accent's argument is a BRACED dotless i.
/// Only the bare `\'\i` form was mapped to the dotted letter, so this one published a
/// dotless ı plus a combining acute, which looks close but is not NFC: Ctrl-F and the
/// search index miss "Martínez" (audit 2026-09-24, bibtex #17).
#[test]
fn an_accent_on_a_braced_dotless_i_is_the_precomposed_letter() {
    for (raw, want) in [
        (r"Mart{\'{\i}}nez", "Mart\u{ed}nez"),
        (r"Rodr\'{\i}guez", "Rodr\u{ed}guez"),
        (r"Garc{\'\i}a", "Garc\u{ed}a"),
        (r#"Na{\"{\i}}ve"#, "Na\u{ef}ve"),
    ] {
        assert_eq!(clean(raw), want, "{raw}");
    }
}

/// TeX's input ligatures print as TeX typesets them, in text fields and never in a URL:
/// ``` ``quoted'' ``` as curly quotes (the renderer's smart typography gives prose the
/// same), `--` and `---` as en and em dashes, `~` as a no-break space. They printed
/// literally: "``double''", "1990--2000", "Proc.~of", "E.~coli" (audit 2026-09-24,
/// bibtex #16 and the escaping lens's adjacent note).
#[test]
fn tex_ligatures_in_text_fields_print_as_typeset() {
    for (raw, want) in [
        ("``Quoted'' title", "\u{201c}Quoted\u{201d} title"),
        ("1990--2000", "1990\u{2013}2000"),
        ("a---b", "a\u{2014}b"),
        ("Proc.~of the ACM", "Proc.\u{a0}of the ACM"),
        // Not a ligature: an accent, a braced break, a symbol macro, and math.
        (r"Espa\~na", "Espa\u{f1}a"),
        ("-{}-", "--"),
        (r"\textasciitilde", "~"),
        ("$a--b$", "$a--b$"),
    ] {
        assert_eq!(clean(raw), want, "{raw}");
    }
    let b = parse_bib("@misc{u, title={A--B}, url={http://example.org/~user/a--b}}\n");
    let f = b.format("u").unwrap();
    assert!(
        f.contains("href=\"http://example.org/~user/a--b\""),
        "a URL is not text: {f}"
    );
}

/// IEEE punctuation on common shapes (audit 2026-09-24, bibtex #16). A title that ends in
/// `?`, `!` or `.` keeps its own mark instead of gaining a comma or period inside the
/// quote ("…Networks?,”", "…End?.”", Google Scholar's "t-SNE.,”"); a note does not double
/// a period; a single page is "p." and a range is dashed even with one hyphen, as BibTeX's
/// `n.dashify` does.
#[test]
fn ieee_punctuation_follows_the_title_and_the_page_count() {
    let b = parse_bib(concat!(
        "@article{q1, author={Xu, Keyulu}, title={How Powerful are Graph Neural Networks?}, journal={ICLR}, year={2019}}\n",
        "@misc{q2, title={Is This the End?}}\n",
        "@article{gs, title={Visualizing data using t-SNE.}, journal={JMLR}, year={2008}}\n",
        "@misc{t6, title={Note only}, note={Accessed: 2023-01-01}}\n",
        "@misc{t8, title={Title ending period.}, note={A note.}}\n",
        "@article{p1, title={P}, journal={J}, pages={42}, year={2020}}\n",
        "@article{p2, title={P}, journal={J}, pages={123-145}, year={2020}}\n",
        "@article{p3, title={P}, journal={J}, pages={123 -- 145}, year={2020}}\n",
        "@article{p7, title={P}, journal={J}, pages={1--5, 7--9}, year={2020}}\n",
    ));
    let f = |k: &str| b.format(k).unwrap();
    assert_eq!(
        f("q1"),
        "K. Xu, \u{201c}How Powerful are Graph Neural Networks?\u{201d} <em>ICLR</em>, 2019."
    );
    assert_eq!(f("q2"), "\u{201c}Is This the End?\u{201d}");
    assert!(
        f("gs").starts_with("\u{201c}Visualizing data using t-SNE.\u{201d} <em>JMLR</em>"),
        "{}",
        f("gs")
    );
    assert_eq!(f("t6"), "\u{201c}Note only.\u{201d} Accessed: 2023-01-01.");
    assert_eq!(f("t8"), "\u{201c}Title ending period.\u{201d} A note.");
    assert!(f("p1").contains("<em>J</em>, p. 42, 2020"), "{}", f("p1"));
    assert!(f("p2").contains("pp. 123\u{2013}145"), "{}", f("p2"));
    assert!(f("p3").contains("pp. 123\u{2013}145"), "{}", f("p3"));
    assert!(
        f("p7").contains("pp. 1\u{2013}5, 7\u{2013}9"),
        "{}",
        f("p7")
    );
}

/// Zotero's "Better BibLaTeX" and built-in BibLaTeX exports name three fields differently:
/// `journaltitle`, `date` and `location`. Only the BibTeX names were read, so every article
/// lost its journal and year and every `@online` its year, silently (audit 2026-09-24 G6,
/// bibtex #7).
#[test]
fn a_biblatex_export_keeps_its_journal_year_and_place() {
    let b = parse_bib(concat!(
        "@article{bl, title = {Deep Learning for {{DNA}} Sequencing: {{A}} Review},\n",
        "  author = {Smith, John and Müller, Hans}, date = {2020-01},\n",
        "  journaltitle = {Nature Methods}, volume = {17}, number = {3}, pages = {123--145}}\n",
        "@online{web, title = {Some {{Web Page}}}, author = {{Mozilla Contributors}},\n",
        "  date = {2021-03-04}, url = {https://developer.mozilla.org/en-US/docs/Web},\n",
        "  organization = {{MDN Web Docs}}}\n",
        "@book{bk, title = {A Book}, date = {1999}, publisher = {OUP}, location = {Oxford}}\n",
        // The BibTeX name wins when both are present.
        "@article{both, title = {T}, journal = {BibTeX J}, journaltitle = {BibLaTeX J}, year = {2001}, date = {2002}}\n",
    ));
    let bl = b.format("bl").unwrap();
    assert!(
        bl.ends_with("<em>Nature Methods</em>, vol. 17, no. 3, pp. 123\u{2013}145, 2020."),
        "{bl}"
    );
    assert!(b.format("web").unwrap().contains("MDN Web Docs, 2021."));
    assert!(b.format("bk").unwrap().contains("Oxford: OUP, 1999."));
    let both = b.format("both").unwrap();
    assert!(both.contains("BibTeX J") && both.contains("2001"), "{both}");
}

/// Four fields real exports rely on were never read (audit 2026-09-24, bibtex #15):
/// `doi` (a DOI-only entry, as Mendeley and Better BibTeX write one, had no link),
/// `editor` (an edited volume rendered with no names at all), `crossref` (DBLP's standard
/// export puts a conference paper's venue, year and publisher on the parent entry, so the
/// child lost them all, and its pages too) and `school` (a thesis lost its university).
#[test]
fn doi_editor_crossref_and_school_are_read() {
    let b = parse_bib(concat!(
        "@article{doi, author={Smith, John}, title={T}, journal={J}, year={2019}, doi={10.1000/xyz123}}\n",
        "@misc{doiurl, title={T}, doi={https://doi.org/10.1000/xyz}}\n",
        "@misc{both, title={T}, url={https://example.org/p}, doi={10.1000/xyz}}\n",
        "@book{ed, title={Edited Volume}, editor={Keeper, Kay}, year={2018}, publisher={OUP}, address={Oxford}}\n",
        "@book{eds, title={Two Editors}, editor={Keeper, Kay and Other, Olga}, year={2018}, publisher={OUP}}\n",
        "@inproceedings{child, author={Poe, P.}, title={Crossref child}, crossref={conf20}, pages={1--10}}\n",
        "@proceedings{conf20, title={Proceedings of Conf 2020}, booktitle={Proceedings of Conf 2020}, year={2020}, publisher={ACM}}\n",
        "@phdthesis{thesis, author={Graves, Alex}, title={Supervised sequence labelling}, year={2008}, school={Technische Universit{\\\"a}t M{\\\"u}nchen}}\n",
    ));
    let f = |k: &str| b.format(k).unwrap();
    assert!(
        f("doi").ends_with(
            "[Online]. Available: <a href=\"https://doi.org/10.1000/xyz123\">https://doi.org/10.1000/xyz123</a>"
        ),
        "{}",
        f("doi")
    );
    assert!(
        f("doiurl").contains("href=\"https://doi.org/10.1000/xyz\""),
        "a DOI written as a URL is not doubled: {}",
        f("doiurl")
    );
    assert!(
        f("both").contains("href=\"https://example.org/p\"") && !f("both").contains("doi.org"),
        "a url wins over the doi: {}",
        f("both")
    );
    assert_eq!(
        f("ed"),
        "K. Keeper, Ed., <em>Edited Volume</em>. Oxford: OUP, 2018."
    );
    assert!(
        f("eds").starts_with("K. Keeper and O. Other, Eds., "),
        "{}",
        f("eds")
    );
    assert_eq!(
        f("child"),
        "P. Poe, \u{201c}Crossref child,\u{201d} in <em>Proceedings of Conf 2020</em>, ACM, 2020, pp. 1\u{2013}10."
    );
    assert!(
        f("thesis").contains("Technische Universität München, 2008."),
        "{}",
        f("thesis")
    );
}

/// Every field reaches the page escaped; `edition` was interpolated raw, in both the book
/// and the chapter format, so markup in it became real elements (and a `<!--` in it hid
/// the rest of the page) (audit 2026-09-24, bibtex #18 / escaping #8e).
#[test]
fn the_edition_field_is_escaped() {
    let b = parse_bib(concat!(
        "@book{bk, title={B}, edition={<b>3</b> & more}, publisher={P}, year={2000}}\n",
        "@incollection{ch, title={C}, booktitle={B}, edition={<i>2</i>}, publisher={P}, year={2000}}\n",
    ));
    let bk = b.format("bk").unwrap();
    assert!(
        bk.contains("&lt;b&gt;3&lt;/b&gt; &amp; more ed.") && !bk.contains("<b>"),
        "{bk}"
    );
    let ch = b.format("ch").unwrap();
    assert!(
        ch.contains("&lt;i&gt;2&lt;/i&gt; ed.") && !ch.contains("<i>"),
        "{ch}"
    );
}

/// An entry missing its title (or everything but its author) leaves no dangling
/// punctuation: `@misc{k, author=…}` rendered "J. Smith, " and `@book{k, author=…}`
/// "J. Smith, ." (audit 2026-09-24, bibtex #19). The gap stays visible in the page, which
/// is where the author sees it; only the stray marks go.
#[test]
fn an_entry_missing_fields_leaves_no_dangling_punctuation() {
    let b = parse_bib(concat!(
        "@misc{m, author={Smith, John}}\n",
        "@book{b, author={Smith, John}}\n",
        "@book{bp, author={Smith, John}, publisher={P}, year={2000}}\n",
        "@misc{u, author={Smith, John}, url={https://example.org/x}}\n",
    ));
    assert_eq!(b.format("m").unwrap(), "J. Smith.");
    assert_eq!(b.format("b").unwrap(), "J. Smith.");
    assert_eq!(b.format("bp").unwrap(), "J. Smith, P, 2000.");
    assert_eq!(
        b.format("u").unwrap(),
        "J. Smith, [Online]. Available: <a href=\"https://example.org/x\">https://example.org/x</a>"
    );
}

/// A citation group is parsed on its TEXT, not on the escaped HTML around it, and escaped
/// once on the way out (audit 2026-09-24, bibtex #13 and escaping #4).
///
/// It was split on `;` as comrak had escaped it, so the `;` of `&amp;` split the group:
/// `[@k1, pp. 3 & 7]` published "[1, pp. 3 &amp]", " 7" lost. Then the text was escaped
/// a second time, so a group that was not a citation read "[Q&amp;A @ noon]".
#[test]
fn a_citation_group_is_parsed_on_text_not_on_escaped_html() {
    let b = parse_bib("@misc{k1, title={T}, year={2020}}\n");
    let mut blocks = vec![block(
        "<p>A [@k1, pp. 3 &amp; 7]. B [@k1, ch. &lt;2&gt;]. C [Q&amp;A @ noon].</p>",
    )];
    process(&mut blocks, &b, &HashMap::new(), None);
    let html = &blocks[0].html;
    assert!(
        html.contains("[<a href=\"#ref-k1\">1</a>, pp. 3 &amp; 7]"),
        "{html}"
    );
    assert!(
        html.contains("[<a href=\"#ref-k1\">1</a>, ch. &lt;2&gt;]"),
        "{html}"
    );
    assert!(html.contains("[Q&amp;A @ noon]"), "{html}");
    assert!(!html.contains("&amp;amp;"), "escaped twice: {html}");
}

/// A bracket is a citation group only when every item in it starts with `@` (after an
/// optional `-`); anything else is left as the text it is (audit 2026-09-24, bibtex #12).
///
/// The item's `@` used to be found anywhere in it and the text before it thrown away, so
/// `[see @a, p. 3; also @b]` published "[1, p. 3, 2]" with "see" and "also" silently gone,
/// `[x < y @a]` published "[1]", `[by mail at bob@smith.2020]` cited `smith.2020`, and
/// `[bob@example.com]` cited a key `example.com`. Left literal, a bare `@key` in the text is
/// then reported by the bare-citation check rather than lost.
#[test]
fn a_bracket_is_a_citation_only_when_every_item_starts_with_at() {
    let b =
        parse_bib("@misc{smith.2020, title={S}}\n@misc{doe+roe, title={D}}\n@misc{a, title={A}}\n");
    let mut xrefs = HashMap::new();
    xrefs.insert("fig-x".to_string(), "3".to_string());
    let mut blocks = vec![
        block("<p>A [see @smith.2020, pp. 33\u{2013}35; also @doe+roe, sec. 2].</p>"),
        block("<p>B Contact us [by mail at bob@smith.2020].</p>"),
        block("<p>C Email [bob@example.com] for details.</p>"),
        block("<p>D [x &lt; y @a].</p>"),
        block("<p>E [see @fig-x].</p>"),
        block("<p>F [@smith.2020; -@doe+roe, p. 2].</p>"),
    ];
    let w = process(&mut blocks, &b, &xrefs, None);
    let html = |i: usize| blocks[i].html.clone();
    assert!(
        html(0).contains("[see @smith.2020, pp. 33\u{2013}35; also @doe+roe, sec. 2]"),
        "{}",
        html(0)
    );
    assert!(
        html(1).contains("[by mail at bob@smith.2020]"),
        "{}",
        html(1)
    );
    assert!(html(2).contains("[bob@example.com]"), "{}", html(2));
    assert!(html(3).contains("[x &lt; y @a]"), "{}", html(3));
    // A bare cross-reference inside a literal bracket still links.
    assert!(
        html(4).contains("[see <a href=\"#fig-x\" class=\"tali-xref\">Figure&nbsp;3</a>]"),
        "{}",
        html(4)
    );
    // A real group still renders, and only its keys are numbered.
    assert!(
        html(5)
            .contains("[<a href=\"#ref-smith.2020\">1</a>, <a href=\"#ref-doe+roe\">2</a>, p. 2]"),
        "{}",
        html(5)
    );
    let refs = &blocks.last().unwrap().html;
    assert_eq!(refs.matches("class=\"csl-entry\"").count(), 2, "{refs}");
    assert!(!refs.contains("example.com"), "{refs}");
    assert!(broken(&w).is_empty(), "{w:?}");
}

/// The bare-citation errors a page draws: a `@key` that names a bibliography entry but
/// shipped as literal text. Rendered with a one-entry `.bib` beside the page.
fn bare_key_errors(tag: &str, body: &str) -> Vec<Warning> {
    let dir = std::env::temp_dir().join(format!("tali-bare-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("refs.bib"),
        "@article{knuth84, title={Literate Programming}, author={Knuth, D.}, year={1984}}\n",
    )
    .unwrap();
    let src = format!("---\ntitle: T\nbibliography: refs.bib\n---\n\n{body}");
    let doc = crate::render_document_with_includes(&src, &dir);
    let mut w = doc.warnings.clone();
    let _ = std::fs::remove_dir_all(&dir);
    w.retain(|w| w.message.contains("is not a citation"));
    w
}

/// A correct citation in a figure caption renders correctly, and must not fail the gate:
/// the old check substring-scanned the finished HTML, so the caption's raw text in the
/// image's `alt` read as a bare `@key` and `build --check-only` exited 1 on a valid page
/// (audit 2026-09-24 G4).
#[test]
fn a_citation_in_a_figure_caption_is_not_a_bare_key() {
    let w = bare_key_errors("fig", "![Adapted from [@knuth84]](img.png){#fig-a}\n");
    assert!(w.is_empty(), "{w:?}");
}

/// Code and comments are not prose, wherever they sit (audit 2026-09-24 G4, and the
/// scanners lens's #7): inline code, a code block nested in a list item or a callout,
/// and an HTML comment. Only a top-level `<pre>` block used to be skipped.
#[test]
fn a_key_in_code_or_a_comment_is_not_a_bare_key() {
    for (tag, body) in [
        ("block", "```\n@knuth84\n```\n"),
        ("inline", "Write `[@knuth84]` to cite.\n"),
        ("list", "- An item:\n\n  ```\n  see @knuth84\n  ```\n"),
        (
            "callout",
            "::: {.callout-note}\n```\nsee @knuth84\n```\n:::\n",
        ),
        ("comment", "Prose.\n\n<!-- cite [@knuth84] here -->\n"),
    ] {
        let w = bare_key_errors(tag, body);
        assert!(w.is_empty(), "{tag}: {w:?}");
    }
}

/// The real defect is still caught, once, located, with the bracketed form offered; and
/// a key left in a bracket that is not a citation group (text before its `@`) is caught
/// too, instead of being dropped in silence.
#[test]
fn a_bare_key_in_prose_is_still_an_error() {
    let w = bare_key_errors("prose", "As shown by @knuth84.\n");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].message.contains("[@knuth84]"), "{}", w[0].message);
    assert_eq!(w[0].severity, crate::render::Severity::Error);
    assert_eq!(w[0].line, Some(6));
    let w = bare_key_errors("bracket", "Read it [see @knuth84, p. 3].\n");
    assert_eq!(w.len(), 1, "{w:?}");
    // A real citation is clean.
    assert!(bare_key_errors("cited", "As shown [@knuth84].\n").is_empty());
    // Membership gating is what makes the rule safe: `is_cite_key_char` admits `/ . : +`,
    // so without it `@media`, `@types/node` and addresses would all fire.
    let w = bare_key_errors(
        "noise",
        "Use @media queries, install @types/node, mail bob@knuth84.com or ping \
         @knuth84XYZ today.\n",
    );
    assert!(w.is_empty(), "{w:?}");
}

/// No bibliography, nothing to match: every `@word` is prose.
#[test]
fn a_page_without_a_bibliography_draws_no_bare_key_error() {
    let src = "---\ntitle: T\n---\n\nPlease refer to @knuth84.\n";
    let doc = crate::render_document(src);
    assert!(
        !doc.warnings
            .iter()
            .any(|w| w.message.contains("is not a citation")),
        "{:?}",
        doc.warnings
    );
}

/// A key ends at its last letter, digit or `_`: punctuation belongs to a key only inside it,
/// Pandoc's rule (audit 2026-09-24, bibtex #11). `[@knuth:1984: a note]` read the colon
/// after the year as part of the key and published a broken citation; the sentence period
/// of `[@smith.2020.]` did the same. A bib key that ENDS in punctuation can therefore never
/// be cited, so it is reported and skipped like any other key the syntax cannot name.
#[test]
fn a_key_ends_at_its_last_letter_digit_or_underscore() {
    let b = parse_bib("@misc{knuth:1984, title={K}}\n@misc{smith.2020, title={S}}\n");
    let mut blocks = vec![block(
        "<p>A [@knuth:1984: a note]. B [@smith.2020.]. C [@knuth:1984, ch. 2].</p>",
    )];
    let w = process(&mut blocks, &b, &HashMap::new(), None);
    let html = &blocks[0].html;
    assert!(
        html.contains("[<a href=\"#ref-knuth:1984\">1</a>"),
        "{html}"
    );
    assert!(
        html.contains("[<a href=\"#ref-smith.2020\">2</a>"),
        "{html}"
    );
    assert!(broken(&w).is_empty(), "{w:?}");

    let (b, w) = parse_bib_warned("@misc{end.dot., title={T}}\n@misc{snake_, title={U}}\n");
    assert!(b.format("end.dot.").is_none() && b.format("end.dot").is_none());
    assert!(
        w.iter()
            .any(|m| m.contains("end.dot.") && m.contains("cannot be cited")),
        "{w:?}"
    );
    assert!(
        b.format("snake_").is_some(),
        "a trailing `_` is part of a key"
    );
}

/// The punctuation a key's run ends with is the separator before its locator, so it is
/// part of neither: once the key stopped before the colon, `[@knuth:1984: a note]` printed
/// "[1, : a note]" (audit 2026-09-24, WP10 leftover). It reads like the comma form.
#[test]
fn a_locator_drops_the_separator_its_key_ended_with() {
    let b = parse_bib("@misc{knuth:1984, title={K}}\n@misc{smith.2020, title={S}}\n");
    let mut blocks = vec![block(
        "<p>A [@knuth:1984: a note]. B [@smith.2020. p. 3]. C [@knuth:1984, ch. 2].</p>",
    )];
    process(&mut blocks, &b, &HashMap::new(), None);
    let html = &blocks[0].html;
    for want in [
        "[<a href=\"#ref-knuth:1984\">1</a>, a note]",
        "[<a href=\"#ref-smith.2020\">2</a>, p. 3]",
        "[<a href=\"#ref-knuth:1984\">1</a>, ch. 2]",
    ] {
        assert!(html.contains(want), "{want} in {html}");
    }
}

/// The key under a cursor, read by the render's own group grammar (audit 2026-09-24,
/// bibtex #11): every key of a group, a locator after the key, `-@` and every character a
/// key may hold. The editor's hover and go-to-definition used a scanner of their own that
/// resolved 2 of 11 real citation shapes.
#[test]
fn the_key_at_a_column_is_read_by_the_group_grammar() {
    let line = "A [@k1, p. 3] B [@a; @b] C [-@k2] D [@DBLP:j/abs-17] E [@müller2020] \
                F [see @x] G [@knuth:1984: a note] H [@k3";
    let at = |needle: &str, off: usize| {
        let byte = line.find(needle).unwrap_or_else(|| panic!("{needle}"));
        let col = line[..byte].chars().count() + off;
        citation_key_at(line, col).map(|(key, span)| {
            let shown: String = line.chars().skip(span.start).take(span.len()).collect();
            (key, shown)
        })
    };
    let hit = |k: &str| Some((k.to_string(), format!("@{k}")));
    assert_eq!(at("@k1", 0), hit("k1"), "the `@` itself");
    assert_eq!(at("@k1", 3), hit("k1"), "just past the key");
    assert_eq!(at("@k1", 5), None, "on the locator");
    assert_eq!(at("@a;", 1), hit("a"));
    assert_eq!(at("@b]", 1), hit("b"), "a later key of the group");
    assert_eq!(at("@k2", 2), hit("k2"), "an author-suppressed key");
    assert_eq!(at("@DBLP", 9), hit("DBLP:j/abs-17"));
    assert_eq!(at("@müller", 3), hit("müller2020"), "a non-ASCII key");
    assert_eq!(at("@x]", 1), None, "a bracket that is not a group");
    assert_eq!(at("@knuth", 2), hit("knuth:1984"));
    assert_eq!(at("@k3", 1), None, "no closing bracket");
}

/// Where each entry a bibliography stores sits in its file, for the editor's hover and
/// go-to-definition (audit 2026-09-24, bibtex #11): paren-delimited entries too, and only
/// keys the bibliography keeps.
#[test]
fn entry_spans_are_the_entries_the_bibliography_stores() {
    let text = "% c\n@misc{a, title={T {nested}}}\n@book(p1, title = \"P\")\n\
                @misc{smith&jones2020, title={X}}\n@misc{müller2020, title={M}}\n";
    let spans = entry_spans(text);
    let got: Vec<(&str, &str)> = spans
        .iter()
        .map(|(k, r)| (k.as_str(), &text[r.clone()]))
        .collect();
    assert_eq!(
        got,
        vec![
            ("a", "@misc{a, title={T {nested}}}"),
            ("p1", "@book(p1, title = \"P\")"),
            ("müller2020", "@misc{müller2020, title={M}}"),
        ]
    );
}
