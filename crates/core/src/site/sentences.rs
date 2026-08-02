//! Reading-form text and sentence splitting.
//!
//! Two jobs, both about turning rendered HTML into the words a person actually reads:
//! [`plain`] flattens an HTML fragment to prose, and [`sentence_at`] finds the sentence
//! containing an offset in it. `backlinks.rs` is the consumer — it pulls the sentence a
//! citing page wrote around its reference.
//!
//! Carved out of `skim.rs` on 2026-08-03 when the layer-cake projection was retired. The
//! projection is what the sentence splitter was BUILT for, and it is gone; these two
//! survive because a backlink quote has the same problem and had already been pointed at
//! them, deliberately, rather than growing a second "strip the tags" pass.

use crate::render;

/// Plain text for the projection: [`render::indexable_text`], then the spaces that pass
/// leaves against punctuation removed.
///
/// `strip_tags_separated` inserts a space at *every* tag boundary, which is right for the
/// search index (two adjacent blocks must not weld into one word) but shows up here as
/// "equal-length columns , each" wherever a sentence ends on inline code or emphasis. The
/// fix belongs on this side: the index is keyed on that exact text and matched by `indexOf`,
/// so normalizing it upstream would change what a search finds.
///
/// **Both sides**, symmetrically. The closing half shipped first and the opening half was
/// missing, so a parenthesised inline element came out as "a Rust toolchain ( cargo
/// build)" — 7 occurrences in `docs/guide`'s projection, and the shape is common because
/// `(@sec-x)` and `(`code`)` are both ordinary. Found by reading real output, not source.
///
/// `pub(super)` because `backlinks.rs` reads the same reading-form text to pull a citing
/// sentence out of a referring block. Two extractors that "both strip tags" is exactly the
/// R1 divergence this codebase already carries once; one is enough.
pub(super) fn plain(html: &str) -> String {
    let text = render::indexable_text(html);
    let mut out = String::with_capacity(text.len());
    let mut prev: Option<char> = None;
    for (i, c) in text.char_indices() {
        let next_is_closing = text[i + c.len_utf8()..].chars().next().is_some_and(|n| {
            matches!(n, ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']' | '”' | '’')
        });
        let after_opening = prev.is_some_and(|p| matches!(p, '(' | '[' | '“' | '‘'));
        if c == ' ' && (next_is_closing || after_opening) {
            continue;
        }
        out.push(c);
        prev = Some(c);
    }
    out
}

/// Abbreviations that end in `.` without ending a sentence. Deliberately short: every entry
/// is a word that genuinely appears mid-sentence in technical prose. A longer list buys
/// little and costs a wrong split in the other direction.
const ABBREVIATIONS: &[&str] = &[
    "e.g", "i.e", "cf", "vs", "etc", "al", "Fig", "fig", "Eq", "eq", "Dr", "Mr", "Mrs", "Ms",
    "Prof", "St", "approx", "ca", "no", "No", "vol", "Vol", "ch", "Ch", "pp", "Sec", "sec",
];

/// The first sentence of `text` (already plain, whitespace-collapsed), or `None` when there
/// is no prose.
///
/// A sentence ends at `.`, `?` or `!` followed by whitespace and a capital/quote/digit —
/// but **not** when the period is a decimal point (`3.14`), an ellipsis, an initial (`A.
/// Turing`), or one of [`ABBREVIATIONS`]. Getting this wrong is not cosmetic: the projection
/// is read as evidence about how a section opens, so a split at `e.g.` would invent a
/// two-word opening sentence that the author never wrote and the reader never sees.
///
/// When no terminator is found the whole text is the sentence (a heading-only line, a list
/// item, a section that opens on a fragment).
pub fn first_sentence(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let bytes = text.as_bytes();
    let mut idx = 0;
    while let Some(rel) = text[idx..].find(['.', '?', '!']) {
        let at = idx + rel;
        let term = bytes[at];
        // Consume a run of terminators, so `?!` and `...` are one boundary.
        let mut end = at + 1;
        while end < bytes.len() && matches!(bytes[end], b'.' | b'?' | b'!') {
            end += 1;
        }
        let run = end - at;
        let after = text[end..].trim_start();
        let ate_space = text[end..].len() != after.len();
        // A terminator that ends the string always ends the sentence.
        if after.is_empty() {
            return Some(text.to_string());
        }
        // No decimal-point guard: a decimal has no space after the dot, and `ate_space`
        // below already requires one, so the two can never both hold. A separate
        // `1.17`-shaped test passed with the guard deleted, which is how it was caught.
        let ellipsis = term == b'.' && run >= 3;
        let word = text[..at]
            .rsplit(|c: char| c.is_whitespace())
            .next()
            .unwrap_or("");
        // An initial is a single capital letter before the dot (`A. Turing`).
        let initial = term == b'.'
            && run == 1
            && word.chars().count() == 1
            && word.chars().next().is_some_and(char::is_uppercase);
        let abbrev = term == b'.' && run == 1 && ABBREVIATIONS.contains(&word);
        let starts_new = after.chars().next().is_some_and(|c| {
            c.is_uppercase() || c.is_ascii_digit() || matches!(c, '"' | '\'' | '“' | '‘' | '(')
        });
        if ate_space && starts_new && !ellipsis && !initial && !abbrev {
            return Some(text[..end].trim_end().to_string());
        }
        idx = end;
    }
    Some(text.to_string())
}

/// The sentence of `text` containing byte offset `at`, walking [`first_sentence`] forward
/// so the two can never disagree on where a sentence ends. Used by the backlink line to
/// quote the sentence a cross-reference is made in.
///
/// An `at` past the end returns the last sentence rather than `None`: the caller's offset
/// comes from a marker in the same string, so it is always in range, and clamping is the
/// harmless reading of a would-be-impossible input.
pub(super) fn sentence_at(text: &str, at: usize) -> Option<String> {
    let mut start = 0usize;
    loop {
        let rest = text[start..].trim_start();
        let off = text.len() - rest.len(); // `rest` is a suffix, so this is its offset
        let sentence = first_sentence(rest)?;
        let end = off + sentence.len();
        // `end >= text.len()` also covers the trailing-whitespace case, where the last
        // sentence ends before the string does and no further sentence exists.
        if at < end || end >= text.len() {
            return Some(sentence);
        }
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_closes_the_gap_on_both_sides_of_a_bracketed_inline_element() {
        // `indexable_text` puts a space at every tag boundary, so a parenthesised inline
        // element gets one on EACH side. The closing half was already handled; the
        // opening half was not, and it showed up 7 times in `docs/guide`'s own
        // projection ("a Rust toolchain ( cargo build)").
        assert_eq!(
            plain("<p>Builds with a toolchain (<code>cargo build</code>), then runs.</p>"),
            "Builds with a toolchain (cargo build), then runs."
        );
        assert_eq!(
            plain("<p>A quote \u{201c}<em>so</em>\u{201d} and a list [<code>a</code>].</p>"),
            "A quote \u{201c}so\u{201d} and a list [a]."
        );
        // An ordinary space between words is untouched, in both neighbourhoods.
        assert_eq!(
            plain("<p>one <em>two</em> three (four five)</p>"),
            "one two three (four five)"
        );
    }

    #[test]
    fn a_plain_sentence_stops_at_its_period() {
        assert_eq!(
            first_sentence("One thing. Then another."),
            Some("One thing.".to_string())
        );
    }

    /// `sentence_at` walks `first_sentence` forward to find the sentence containing an
    /// offset. Nothing exercised it directly — it is reached through the backlink line, which
    /// quotes the sentence a cross-reference was made in, so a wrong answer there is a
    /// plausible-looking quotation of the wrong sentence.
    #[test]
    fn sentence_at_returns_the_sentence_containing_the_offset() {
        //             0.........10........20........30
        let text = "First one. Second two. Third three.";
        assert_eq!(sentence_at(text, 0).as_deref(), Some("First one."));
        assert_eq!(sentence_at(text, 5).as_deref(), Some("First one."));
        assert_eq!(sentence_at(text, 12).as_deref(), Some("Second two."));
        assert_eq!(sentence_at(text, 30).as_deref(), Some("Third three."));
        // The boundary itself: offset 10 is the space AFTER the first sentence's stop, so it
        // belongs to what follows, not to what it terminates.
        assert_eq!(sentence_at(text, 10).as_deref(), Some("Second two."));
        // Past the end clamps to the last sentence rather than answering nothing — the
        // caller's offset comes from a marker in this same string, so `None` would be a
        // silently dropped backlink quote.
        assert_eq!(sentence_at(text, 999).as_deref(), Some("Third three."));
        assert_eq!(sentence_at("", 0), None);
    }

    #[test]
    fn an_abbreviation_does_not_end_a_sentence() {
        // The whole point: splitting here would invent a two-word opening the author
        // never wrote.
        assert_eq!(
            first_sentence("Some formats, e.g. HTML, are live. Others are not."),
            Some("Some formats, e.g. HTML, are live.".to_string())
        );
        assert_eq!(
            first_sentence("The cap is gone, i.e. Nothing truncates. Good."),
            Some("The cap is gone, i.e. Nothing truncates.".to_string())
        );
    }

    #[test]
    fn a_decimal_point_does_not_end_a_sentence() {
        // Pinned by the no-space-after rule, not by a decimal-specific guard (there is
        // none: see the comment at the `ellipsis` binding). The `2.` case is the one that
        // actually bites — a digit both precedes and follows, and `2` would satisfy
        // `starts_new` on its own.
        assert_eq!(
            first_sentence("It grew by 1.17x overall. Then it stopped."),
            Some("It grew by 1.17x overall.".to_string())
        );
        assert_eq!(
            first_sentence("Version 2.1 shipped. Then 3.0 did."),
            Some("Version 2.1 shipped.".to_string())
        );
    }

    #[test]
    fn an_initial_does_not_end_a_sentence() {
        assert_eq!(
            first_sentence("Named for A. Turing, who proved it. Later."),
            Some("Named for A. Turing, who proved it.".to_string())
        );
    }

    /// A sentence that ends on a proper noun still ends. The initial rule (`A. Turing`) is a
    /// conjunction of four conditions, and dropping the "the word is a single character" half
    /// leaves "the word before the dot is capitalised" — which is every sentence ending in a
    /// name, a place or a product, so the projection would silently weld two sentences
    /// together for a whole class of ordinary prose.
    #[test]
    fn a_capitalised_last_word_is_not_an_initial() {
        assert_eq!(
            first_sentence("He went to Rome. Then home."),
            Some("He went to Rome.".to_string())
        );
    }

    #[test]
    fn an_ellipsis_does_not_end_a_sentence() {
        assert_eq!(
            first_sentence("It trails off... Then resumes here. End."),
            Some("It trails off... Then resumes here.".to_string())
        );
    }
}
