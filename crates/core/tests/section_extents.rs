//! Pin for `section-extents` option (b), ruled 2026-07-26: every heading block records
//! where its section ends, as `data-section-end="<block-id>"`.
//!
//! Option (a) — a real `<section>` wrapper — was explicitly deferred, because it changes
//! the parent/child shape the incremental diff mounts. The marker is purely additive, so
//! what has to be pinned is that it is *complete* (no heading without one), *sound* (it
//! names a block that exists in the same document) and *nesting* (an `##` section
//! contains its `###` subsections rather than stopping at the first one).
//!
//! `corpus/layout/structure.tmd` is the pin document; it carries each shape the rule has
//! to survive, including the two that have no natural home in an ordinary page: an empty
//! section, and a final section followed by generated furniture.

mod common;
use common::corpus_dir;
use taliesin_core::render::Block;

fn structure_blocks() -> Vec<Block> {
    let path = corpus_dir().join("layout/structure.tmd");
    let src = std::fs::read_to_string(&path).expect("structure.tmd is readable");
    let base = path.parent().unwrap().to_path_buf();
    taliesin_core::render_document_with_includes(&src, &base).blocks
}

/// The attribute's value on a block, if present.
fn section_end(html: &str) -> Option<&str> {
    let marker = "data-section-end=\"";
    let start = html.find(marker)? + marker.len();
    let len = html[start..].find('"')?;
    Some(&html[start..start + len])
}

fn heading_level(html: &str) -> Option<u8> {
    let b = html.as_bytes();
    (b.len() >= 4 && b[0] == b'<' && b[1] == b'h' && b[2].is_ascii_digit())
        .then(|| b[2] - b'0')
        .filter(|l| (1..=6).contains(l))
        .filter(|_| matches!(b[3], b' ' | b'>'))
}

/// Completeness + soundness, the two properties a consumer reads the attribute under.
/// A heading without one would make every consumer carry a fallback path; one naming a
/// block that is not in the document would make the fallback silent.
#[test]
fn every_heading_carries_an_extent_that_names_a_real_block() {
    let blocks = structure_blocks();
    let ids: Vec<&str> = blocks.iter().map(|b| b.id.as_str()).collect();
    let mut headings = 0;
    for b in &blocks {
        if heading_level(&b.html).is_none() {
            continue;
        }
        headings += 1;
        let end = section_end(&b.html).unwrap_or_else(|| {
            panic!("heading block has no data-section-end: {}", b.html);
        });
        assert!(
            ids.contains(&end),
            "data-section-end=\"{end}\" names no block in this document (heading: {})",
            b.html
        );
    }
    assert!(
        headings >= 7,
        "fixture precondition: structure.tmd should carry the whole heading shape \
         (nested, empty, final); found {headings} headings"
    );
}

/// An extent never runs backwards, and never past the end of the block list. Stated as
/// its own test because both failures are silently plausible: a heading immediately
/// followed by a sibling heading is the natural source of an off-by-one that points at
/// the block *before* the heading.
#[test]
fn an_extent_never_precedes_its_own_heading() {
    let blocks = structure_blocks();
    let pos = |id: &str| blocks.iter().position(|b| b.id == id);
    for (i, b) in blocks.iter().enumerate() {
        if heading_level(&b.html).is_none() {
            continue;
        }
        let end = section_end(&b.html).expect("heading carries an extent");
        let j = pos(end).expect("extent names a real block");
        assert!(
            j >= i,
            "section extent runs backwards: heading at {i} ends at {j} ({})",
            b.html
        );
    }
}

/// The empty-section case: `## A heading with nothing beneath it` is followed
/// immediately by a sibling heading, so its extent is its own id. This is what lets a
/// consumer treat every extent as a non-empty inclusive range.
#[test]
fn a_heading_with_no_body_ends_on_itself() {
    let blocks = structure_blocks();
    let empty = blocks
        .iter()
        .find(|b| b.html.contains("id=\"sec-empty\""))
        .expect("structure.tmd defines #sec-empty");
    assert_eq!(
        section_end(&empty.html),
        Some(empty.id.as_str()),
        "an empty section must end on its own block: {}",
        empty.html
    );
}

/// Extents nest: the `##` that opens `#sec-nested` contains both `###` subsections, so
/// its extent must reach past them. A flat "stop at the next heading of any level" rule
/// would end it at the first `###`, which is the whole difference the ruling turns on.
#[test]
fn a_section_extent_covers_its_subsections() {
    let blocks = structure_blocks();
    let pos_of = |needle: &str| {
        blocks
            .iter()
            .position(|b| b.html.contains(needle))
            .unwrap_or_else(|| panic!("structure.tmd defines {needle}"))
    };
    let parent = pos_of("id=\"sec-nested\"");
    let second_sub = pos_of("id=\"sec-nested-second\"");
    let end_id = section_end(&blocks[parent].html).expect("parent carries an extent");
    let end = blocks
        .iter()
        .position(|b| b.id == end_id)
        .expect("extent names a real block");
    assert!(
        end > second_sub,
        "the ## section must cover its ### subsections: it ends at block {end}, but its \
         second subsection opens at {second_sub}"
    );
}

/// A section ending at end-of-document does not swallow the generated furniture that is
/// appended after the body. `structure.tmd` cites a source, so its block list ends with
/// the References section; the last authored section must stop before it.
///
/// It used to be a *footnote* that supplied the trailing block. That stopped being true
/// on 2026-08-01, when a note moved to the margin beside its own reference and so became
/// part of the referencing block rather than a gathered section appended after the body
/// (item 183). References is the furniture the same page's prose already named.
#[test]
fn the_last_section_stops_before_the_generated_references_block() {
    let blocks = structure_blocks();
    let refs = blocks
        .iter()
        .position(|b| b.id == "tali-references")
        .expect("fixture precondition: structure.tmd's citation should emit a References block");
    assert_eq!(
        refs,
        blocks.len() - 1,
        "fixture precondition: the References block should be last"
    );
    let last = blocks
        .iter()
        .find(|b| b.html.contains("id=\"sec-last\""))
        .expect("structure.tmd defines #sec-last");
    let end_id = section_end(&last.html).expect("the final heading carries an extent");
    assert_ne!(
        end_id, "tali-references",
        "the last section must not claim the References block as its content"
    );
    let end = blocks
        .iter()
        .position(|b| b.id == end_id)
        .expect("extent names a real block");
    assert!(
        end < refs,
        "the last section ends at block {end}, at or past the References block {refs}"
    );
}
