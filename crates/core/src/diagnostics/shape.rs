//! Document-**shape** lints: structural defects a reader feels but no other family catches.
//!
//! Every rule here is threshold-free and binary — it asks "is this heading empty / repeated
//! / hollow?", never "is this section too long?". The threshold-bearing candidates (run
//! length, prose density, emphasis rate, fan-out) were measured against the corpus and cut:
//! none had a defensible cut-off, and the flagship one fired on a single page that turned
//! out to be a false positive.
//!
//! These are **advice**, so every code is `SUGGESTION`: they surface in `check`, never fail
//! `build --strict` or `publish`. The author edits the `.tmd`; nothing here rewrites source.

use super::helpers::{heading_level, start_line, strip_tags};
use crate::render::{Block, Warning};

/// The inner HTML between the first `open` tag and the next `close`, if both are present.
fn inner_between<'a>(html: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = html.find(open)? + open.len();
    let end = html[start..].find(close)? + start;
    Some(&html[start..end])
}

/// Visible text with `&nbsp;` folded to a real space first, so the emitted
/// `Figure&nbsp;2: ` reads as the two words a human sees rather than one token.
fn caption_text(html: &str) -> String {
    strip_tags(&html.replace("&nbsp;", " "))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A heading / title compared for sameness: case- and whitespace-insensitive, with the
/// hover permalink (`#`) and trailing punctuation dropped.
fn norm(text: &str) -> String {
    text.trim_end_matches('#')
        .trim()
        .trim_end_matches([':', '.', ',', '!', '?'])
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The page's front-matter title, read off the emitted title block
/// (`<header class="tali-title-block">` wrapping an `<h1 class="title">`). `None` when the
/// document has no title block, in which case its leading heading *is* the title and
/// "repeats the title" is not a meaningful question.
fn page_title(blocks: &[Block]) -> Option<String> {
    let b = blocks
        .iter()
        .find(|b| b.html.contains("class=\"tali-title-block\""))?;
    let inner = inner_between(&b.html, "<h1 class=\"title\">", "</h1>")?;
    let t = strip_tags(inner);
    (!t.is_empty()).then_some(t)
}

/// Whether a caption carries no descriptive text: either empty outright, or only the
/// auto-generated `Figure 2:` label with nothing after the colon. Both have the same fix
/// (write a caption), so they are one code rather than two.
fn caption_is_bare(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return true;
    }
    // `Figure 2:` / `Table 10.` / `Listing 3` and nothing else.
    let rest = ["Figure", "Table", "Listing"]
        .iter()
        .find_map(|p| t.strip_prefix(p))
        .map(str::trim_start);
    let Some(rest) = rest else { return false };
    let rest = rest.trim_end_matches([':', '.']).trim();
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Whether a block contributes something a reader can actually read under a heading.
/// Every non-heading block counts, which is the whole point: a section whose body is a
/// list, a code cell, a figure or a table has content, even though it has no prose
/// paragraph. (Deriving this from the `skim` projection instead is what made an earlier
/// draft of this rule fire on 11.8% of the corpus, essentially all false positives —
/// `skim` reads the first `<p>` and a `<ul>` is not a `<p>`.)
fn is_content(b: &Block) -> bool {
    heading_level(&b.html).is_none() && !b.html.trim().is_empty()
}

/// Structural lints over one rendered document: empty / duplicated / hollow / title-echoing
/// headings, and numbered floats whose caption is only the label.
///
pub fn validate_document_shape(blocks: &[Block]) -> Vec<Warning> {
    use std::collections::HashSet;
    let mut out = Vec::new();
    let title = page_title(blocks).map(|t| norm(&t));

    let at = |b: &Block, w: Warning| match start_line(&b.sourcepos) {
        Some(l) => w.at(b.source_file.clone(), l),
        None => w,
    };

    let headings: Vec<usize> = (0..blocks.len())
        .filter(|&i| heading_level(&blocks[i].html).is_some())
        .collect();

    let mut seen: HashSet<String> = HashSet::new();
    for (nth, &i) in headings.iter().enumerate() {
        let b = &blocks[i];
        let text = strip_tags(&b.html);
        let key = norm(&text);

        if key.is_empty() {
            out.push(at(
                b,
                Warning::new(
                    "empty heading: it opens a section with no name, so the TOC, the outline \
                     and every cross-reference to it read as a blank row"
                        .to_string(),
                ),
            ));
        } else {
            if !seen.insert(key.clone()) {
                out.push(at(b, Warning::new(format!(
                    "duplicate heading text `{text}`: an earlier heading on this page reads the same, \
                     so the TOC shows two rows a reader cannot tell apart"
                ))));
            }
            // Only a *body* heading counts as an echo. A leading heading that restates the
            // front-matter title is the ordinary landing-page idiom (it is how both dogfood
            // books open), and flagging it would make the rule fire on house style alone.
            if nth > 0 && title.as_deref() == Some(key.as_str()) {
                out.push(at(b, Warning::new(format!(
                    "heading `{text}` repeats the page title: it adds a TOC row that tells a reader \
                     nothing the title did not already say"
                ))));
            }
        }

        // Hollow: nothing but headings between this one and the next, AND no subsection
        // beneath it. A heading followed by *deeper* headings is an ordinary grouping
        // parent — it does have content in the document tree, and demanding an intro
        // paragraph there is a style opinion, not a defect. A heading followed by a
        // same-or-shallower heading (or by nothing) has neither prose nor subsections, so
        // the section is empty on any reading. Measured: the broad form fired 13 times
        // across the 14 corpus projects, most of them ordinary grouping parents.
        let end = headings.get(nth + 1).copied().unwrap_or(blocks.len());
        let next_is_subsection = headings
            .get(nth + 1)
            .and_then(|&j| heading_level(&blocks[j].html))
            .is_some_and(|next| heading_level(&b.html).is_some_and(|cur| next > cur));
        if !next_is_subsection && !blocks[i + 1..end].iter().any(is_content) {
            out.push(at(b, Warning::new(format!(
                "heading `{text}` has no content under it: it has neither text nor subsections, \
                 so it is a TOC row that leads nowhere"
            ))));
        }
    }

    for b in blocks {
        let mut rest = b.html.as_str();
        while let Some(inner) = inner_between(rest, "<figcaption>", "</figcaption>") {
            let text = caption_text(inner);
            if caption_is_bare(&text) {
                out.push(at(
                    b,
                    Warning::new(
                        "figure caption is only its label: the figure is numbered and can be \
                     cross-referenced, but the caption says nothing about what it shows"
                            .to_string(),
                    ),
                ));
            }
            let Some(next) = rest.find("</figcaption>") else {
                break;
            };
            rest = &rest[next + "</figcaption>".len()..];
        }
    }

    out
}
