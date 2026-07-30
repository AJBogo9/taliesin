//! Block-level transclusion (backlog item 160): `{{< include file.tmd#sec-id >}}` pulls
//! **one anchored section** out of a shared file instead of the whole file.
//!
//! `corpus/transclude.tmd` is the pin, and `corpus/_includes/shared-derivation.tmd` is the
//! shared file it pulls from. The unit tests in `includes.rs` cover the slicing rules on
//! synthetic input; what this file adds is the claim those cannot make — that the
//! **product** does it, through `render_single_doc`, which is the entry point `build` and
//! `preview` use. (`render_document_with_includes` would assert something true of the
//! library and possibly false of the tool: that gap is what let PP-3 ship.)

use std::fs;
use std::path::PathBuf;

mod common;
use common::corpus_dir;

fn shared_path() -> PathBuf {
    corpus_dir().join("_includes/shared-derivation.tmd")
}

fn rendered() -> taliesin_core::RenderedDoc {
    let src = fs::read_to_string(corpus_dir().join("transclude.tmd")).unwrap();
    taliesin_core::render_single_doc(&src, &corpus_dir())
}

/// The whole point: a fragment brings its section and **leaves the rest of the file
/// behind**. Without the negative half this passes on an implementation that ignores the
/// fragment entirely and splices the whole file, which is the most likely way to get this
/// wrong.
#[test]
fn a_fragment_include_brings_one_section_and_leaves_the_rest() {
    let body = rendered().body_html();

    // The pin document writes this directive TWICE: once inside a code fence, where it is
    // documentation and must stay literal, and once on its own line, where it must expand.
    // So the escaped form appears exactly once. A plain "does not contain" would fail on
    // the fenced copy, and dropping the check would miss the directive failing to expand.
    let literal = body
        .matches("{{&lt; include _includes/shared-derivation.tmd#sec-bias-variance")
        .count();
    assert_eq!(
        literal, 1,
        "expected exactly the fenced example to stay literal: 2 means the real directive \
         never expanded, 0 means the fenced one was wrongly treated as a directive"
    );
    // Present: both named sections, and the subsection nested inside one of them.
    for present in [
        "The bias-variance decomposition",
        "The last term is irreducible",
        "The normal equations",
        "Why the inverse can be avoided", // a `###` INSIDE `sec-normal-equations`
    ] {
        assert!(body.contains(present), "expected {present:?} in the output");
    }
    // Absent: the section nobody names, and the shared file's own preamble. Each is a
    // different way of pulling too much — a whole-file splice would bring both.
    for absent in [
        "A section nobody transcludes",
        "it is the control",
        "nothing about a file makes it",
    ] {
        assert!(
            !body.contains(absent),
            "{absent:?} is outside every named section but was transcluded anyway"
        );
    }
}

/// **The merge gate item 160 was filed under: the source map must not perturb.**
///
/// A transcluded section is a slice taken from the middle of its file, so the obvious
/// implementation reports its first line as line 1 and every block below it is off by
/// however far down the file the section starts. Click-to-source would then land near the
/// top of the shared file — and, per `Block::sourcepos`, a wrong-but-parseable line does
/// not fail visibly, it just navigates somewhere plausible and wrong.
///
/// So this does not hardcode line numbers (they would rot the first time the fixture is
/// edited). It reads the shared file and asserts each block's reported line **actually
/// holds that block's text**, which is the property click-to-source needs and which no
/// off-by-N can satisfy.
#[test]
fn every_transcluded_block_reports_the_line_it_really_occupies_in_the_shared_file() {
    let doc = rendered();
    let shared = fs::read_to_string(shared_path()).unwrap();
    let shared_lines: Vec<&str> = shared.lines().collect();

    let from_shared: Vec<_> = doc
        .blocks
        .iter()
        .filter(|b| b.source_file.as_deref() == Some("_includes/shared-derivation.tmd"))
        .collect();
    assert!(
        from_shared.len() >= 6,
        "expected several blocks from the shared file, got {} — a probe whose every \
         cell is negative is a broken probe",
        from_shared.len()
    );

    let mut checked = 0;
    for b in &from_shared {
        let Some((start, _)) = b.sourcepos.split_once('-') else {
            continue; // a gathered block legitimately has no single honest range
        };
        let line: usize = start.split(':').next().unwrap().parse().unwrap();
        assert!(
            line >= 1 && line <= shared_lines.len(),
            "block {} reports line {line}, outside the shared file's {} lines",
            b.id,
            shared_lines.len()
        );
        // The claim under test: the reported line in the SHARED file is the line whose
        // text this block was built from. A heading is the cleanest witness — its text
        // survives rendering verbatim and appears nowhere else in the file.
        let src_line = shared_lines[line - 1];
        if let Some(title) = src_line
            .strip_prefix("## ")
            .or(src_line.strip_prefix("### "))
        {
            let title = title.split(" {#").next().unwrap();
            assert!(
                b.html.contains(title),
                "block at shared line {line} should be the heading {title:?}, but its \
                 html is {:?} — the fragment's line offset is not being applied",
                b.html.chars().take(120).collect::<String>()
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 3,
        "only {checked} heading blocks were cross-checked against their source line; \
         the witness stopped matching, so this test was passing vacuously"
    );

    // The control, in the other direction: the first block that is NOT from the shared
    // file must be one of the pin document's own, still numbered against the pin.
    let own = doc
        .blocks
        .iter()
        .find(|b| b.source_file.is_none() && !b.sourcepos.is_empty())
        .expect("the pin document contributes blocks of its own");
    assert!(
        own.source_file.is_none(),
        "a primary-document block must carry no source file"
    );
}

/// Two fragments of one file are not a cycle. `corpus/transclude.tmd` pulls two sections
/// from the same shared file, which the cycle guard must allow — it keys on the file, so
/// the naive "already expanding this path" check would refuse the second one.
#[test]
fn two_fragments_of_one_file_both_expand_in_a_real_document() {
    let doc = rendered();
    let body = doc.body_html();
    assert!(
        body.contains("The bias-variance decomposition") && body.contains("The normal equations"),
        "both transcluded sections must survive; a file-keyed cycle guard that refused \
         the second would drop one silently"
    );
    assert!(
        doc.warnings
            .iter()
            .all(|w| !w.message.contains("include cycle")),
        "no cycle warning is legitimate here: {:?}",
        doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}

/// The shared file is a document in its own right and the corpus walker renders it as one.
/// This pins the property the feature leans on: nothing marks a file as "an include", so
/// the same file must stay valid standalone.
#[test]
fn the_shared_file_is_still_a_valid_document_on_its_own() {
    let src = fs::read_to_string(shared_path()).unwrap();
    let doc = taliesin_core::render_single_doc(&src, shared_path().parent().unwrap());
    let body = doc.body_html();
    assert!(
        body.contains("A section nobody transcludes"),
        "rendered standalone, the shared file keeps every section"
    );
    assert!(
        doc.warnings.is_empty(),
        "the shared file warns when rendered on its own: {:?}",
        doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}
