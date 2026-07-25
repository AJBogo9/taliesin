//! The **layer-cake projection**: a book reduced to the layers a reader actually skims
//! (numbered headings, each section's opening sentence, and the captions / callout titles /
//! theorem statements that carry meaning on their own), as one linear stream.
//!
//! This is the *measuring instrument* for structural work, which is a stronger reason for it
//! to exist than its standalone use: a structural lint cannot be calibrated against a corpus
//! nobody can see the shape of. That role sets its one hard rule — **the raw first sentence
//! is always printed**, and any judgement about it is a visible annotation beside it, never a
//! suppression. The moment a weak section and a heuristic misfire render identically, the
//! instrument stops measuring and starts asserting.
//!
//! Numbers exist only *after* the render finishes its post-passes, so the projection reads
//! rendered blocks (via [`search::render_finished`], the same recipe the search index uses)
//! rather than markdown. Word counts are the exception: they come from [`crate::prose`], so
//! this agrees with the reading-time figure the page itself shows.

use super::*;

/// One page of the projection, in reading order.
#[derive(Debug, Clone)]
pub struct PageSkim {
    /// Output URL relative to the site root.
    pub url: String,
    /// The page title as the reader sees it (front matter, else the document's own).
    pub title: String,
    /// Book chapter number, when the page is a numbered chapter.
    pub chapter: Option<u32>,
    /// Prose words, counted by [`crate::prose::word_count`] — the same prose-selection
    /// `lint` and the live reading-time count use, so the three can never disagree.
    pub words: usize,
    /// Text before the first section heading (the chapter's opening), already projected
    /// to its first sentence.
    pub intro: Option<String>,
    /// Every anchored section, in document order.
    pub sections: Vec<SectionSkim>,
}

/// One anchored section: its heading, its opening sentence, and the standalone layers
/// inside it.
#[derive(Debug, Clone)]
pub struct SectionSkim {
    /// The heading's anchor id.
    pub id: String,
    /// The heading's absolute HTML level (1-6).
    pub level: u8,
    /// Indent depth measured against *this page's own shallowest* heading, not the
    /// absolute level. A `###`-rooted chapter and a `##`-rooted one both start at 0.
    /// (Ship A shipped the same rule client-side after indenting by level rendered a
    /// `###`-rooted chapter three steps deeper than its neighbour.)
    pub depth: usize,
    /// Heading text as rendered, carrying the section number the page shows.
    pub title: String,
    /// The section's opening sentence, or `None` when the section has no prose at all.
    /// Never suppressed for being short, weak, or odd: see the module note.
    pub first_sentence: Option<String>,
    /// Captions, callout titles and theorem statements found in this section.
    ///
    /// There is deliberately **no per-section word count**: `words` counts prose the way
    /// `lint` selects it (fenced code and `:::` fences excluded), which is a *markdown*
    /// notion, and a section's markdown extent is not available here — this walks rendered
    /// HTML, whose plain text would count code as prose. Per-chapter prose length is its
    /// own backlog item, and it needs the markdown extents to be honest.
    pub layers: Vec<Layer>,
}

/// A standalone meaning-carrying element inside a section.
#[derive(Debug, Clone)]
pub struct Layer {
    pub kind: LayerKind,
    pub text: String,
}

/// Which layer of the cake an entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    /// A `<figcaption>` or table `<caption>`.
    Caption,
    /// A callout's title bar.
    Callout,
    /// A theorem/definition/lemma statement head.
    Theorem,
    /// A proof head.
    Proof,
}

impl LayerKind {
    /// The short tag the human projection prints in the left gutter.
    pub fn tag(self) -> &'static str {
        match self {
            LayerKind::Caption => "caption",
            LayerKind::Callout => "callout",
            LayerKind::Theorem => "theorem",
            LayerKind::Proof => "proof",
        }
    }
}

/// Project one page, or `None` when the page is excluded (the author's 404 chrome) or its
/// source can't be read — the same exclusions the search index makes, for the same reason.
pub(super) fn page_skim(
    page: &Page,
    chapter: Option<u32>,
    targets: &HashMap<String, XrefTarget>,
    book_theorems: Option<&render::TheoremConfig>,
) -> Option<PageSkim> {
    if page.url == "404.html" {
        return None;
    }
    let (src, doc) = super::search::render_finished(page, chapter, targets, book_theorems)?;
    let title = page
        .title
        .clone()
        .or(doc.title.clone())
        .unwrap_or_else(|| page.url.clone());
    let body: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
    let hs = super::search::headings_with_pos(&body);
    // Same rule as the search index: an untitled page opening at `# H1` has that heading as
    // its own title, not a section, so it must not appear as both.
    let title_heading_is_first =
        !render::emits_title_block(crate::frontmatter::front_matter_block(&src).unwrap_or(""))
            && hs.first().is_some_and(|h| h.0 == 1);
    let skip = usize::from(title_heading_is_first);

    // Depth is measured against this page's OWN shallowest heading, never the absolute
    // level: whether a chapter emits a title block and where it roots both shift the
    // absolute level, so a `###`-rooted chapter would otherwise indent three steps beside a
    // `##`-rooted one in the same book. (Ship A shipped this rule client-side after seeing
    // exactly that.)
    let shallowest = hs.iter().skip(skip).map(|h| h.0).min().unwrap_or(1);

    // The intro runs from the top of the page to its first real section. It deliberately
    // does NOT skip past the page's own title heading: [`first_prose_sentence`] reads the
    // first `<p>`, and a heading is an `<hN>`, so the heading is already excluded. A branch
    // that skipped it was written first and then deleted — with it removed the projection of
    // both `corpus/tarn` and `docs/guide` is byte-identical, so it was pure dead weight
    // pretending to be the fix. (What actually welded "1 Installation" onto the chapter's
    // opening sentence was reading the whole slice as flattened text, not the slice bounds.)
    let intro_end = hs.get(skip).map(|h| h.3).unwrap_or(body.len());
    let intro = body.get(..intro_end).and_then(first_prose_sentence);

    let mut sections = Vec::new();
    for (idx, (level, id, heading, _open, close_end)) in hs.iter().enumerate().skip(skip) {
        if heading.is_empty() {
            continue;
        }
        let sec_end = hs.get(idx + 1).map(|n| n.3).unwrap_or(body.len());
        let sec_html = body.get(*close_end..sec_end).unwrap_or("");
        sections.push(SectionSkim {
            id: id.clone(),
            level: *level,
            depth: usize::from(level.saturating_sub(shallowest)),
            title: heading.clone(),
            first_sentence: first_prose_sentence(sec_html),
            layers: layers_in(sec_html),
        });
    }
    Some(PageSkim {
        url: page.url.clone(),
        title,
        chapter,
        // Include-expanded, which is `word_count`'s documented contract and what the book
        // drawer's cost signal counts: `render_finished` hands back the RAW source (it
        // expands internally, for the render), so counting that directly reported an
        // include-assembled chapter at the length of its own directive lines. `skim`, `map`
        // and the drawer must agree, or a reader and an agent read different books.
        words: crate::prose::word_count(
            &crate::includes::resolve(&src, page.input.parent().unwrap_or_else(|| Path::new(".")))
                .0,
        ),
        intro,
        sections,
    })
}

/// Every standalone layer inside one section's HTML, in document order.
///
/// Each layer is a element whose text carries meaning *without* its surrounding prose,
/// which is precisely what makes it worth projecting: a reader skimming a page reads these
/// even when they read no paragraph.
fn layers_in(html: &str) -> Vec<Layer> {
    let mut out: Vec<(usize, Layer)> = Vec::new();
    let mut push = |kind: LayerKind, at: usize, text: String| {
        let text = text.trim().to_string();
        if !text.is_empty() {
            out.push((at, Layer { kind, text }));
        }
    };
    // `<figcaption>` and a table's `<caption>` are plain tags; the rest are class-tagged
    // divs/spans, so each is found by its opening tag and read to its matching close.
    for (tag, kind) in [
        ("figcaption", LayerKind::Caption),
        ("caption", LayerKind::Caption),
    ] {
        for (at, _, inner) in tag_spans(html, tag) {
            push(kind, at, plain(inner));
        }
    }
    for (class, kind) in [
        ("callout-title", LayerKind::Callout),
        ("tali-theorem-head", LayerKind::Theorem),
        ("tali-proof-head", LayerKind::Proof),
    ] {
        for (at, _, inner) in class_spans(html, class) {
            push(kind, at, plain(inner));
        }
    }
    out.sort_by_key(|(at, _)| *at);
    out.into_iter().map(|(_, l)| l).collect()
}

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

/// Containers whose text is **not** the section's prose. `<pre>` is code and cell output;
/// `<figure>`, `<table>` and the callout/theorem/proof boxes are projected as their own
/// layers, so reading them here would both duplicate a layer and pass code off as a
/// sentence.
const NON_PROSE_TAGS: &[&str] = &["pre", "figure", "table", "aside", "blockquote"];
/// Class-tagged containers excluded for the same reason as [`NON_PROSE_TAGS`].
const NON_PROSE_CLASSES: &[&str] = &["callout", "tali-theorem", "tali-proof", "panel-tabset"];

/// The first sentence of the first **prose paragraph** in `html`, or `None` when the section
/// has no prose at all.
///
/// Not `indexable_text` over the whole section: that pass deliberately flattens *everything*
/// (which is right for search, where a reader may look for a word inside a code block), and
/// using it here produced openings like "The tarn binary is self-contained: … macOS Linux
/// Windows brew install tarn curl -LsSf …" — a tabset's labels and shell commands read as
/// one sentence, because the flattened stream has no terminator between them. A reader's eye
/// lands on the first paragraph, so that is what the projection reports.
fn first_prose_sentence(html: &str) -> Option<String> {
    let mut excluded: Vec<(usize, usize)> = Vec::new();
    for tag in NON_PROSE_TAGS {
        excluded.extend(tag_spans(html, tag).into_iter().map(|(s, e, _)| (s, e)));
    }
    for class in NON_PROSE_CLASSES {
        excluded.extend(class_spans(html, class).into_iter().map(|(s, e, _)| (s, e)));
    }
    tag_spans(html, "p")
        .into_iter()
        .find(|(at, _, inner)| {
            !inner.trim().is_empty() && !excluded.iter().any(|(s, e)| at > s && at < e)
        })
        .and_then(|(_, _, inner)| first_sentence(&plain(inner)))
}

/// Every `<tag …>inner</tag>` in `html`, as `(element_start, element_end, inner)`. Skips a
/// longer tag that merely ends with `tag`, so `caption` never matches a `figcaption`.
fn tag_spans<'a>(html: &'a str, tag: &str) -> Vec<(usize, usize, &'a str)> {
    let mut out = Vec::new();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut pos = 0;
    while let Some(rel) = html[pos..].find(&open) {
        let at = pos + rel;
        // The next char must end the tag name (`<figcaption` must not match `<caption`).
        let after_name = html[at + open.len()..].chars().next();
        if !matches!(after_name, Some(c) if c == '>' || c == '/' || c.is_whitespace()) {
            pos = at + open.len();
            continue;
        }
        let Some(gt) = html[at..].find('>') else {
            break;
        };
        let start = at + gt + 1;
        let Some(end_rel) = html[start..].find(&close) else {
            break;
        };
        out.push((
            at,
            start + end_rel + close.len(),
            &html[start..start + end_rel],
        ));
        pos = start + end_rel + close.len();
    }
    out
}

/// Every element whose `class` contains the token `class`, as
/// `(element_start, element_end, inner)`. The close is found by matching nesting depth on
/// the element's own tag name, so a `callout` holding an inner `<div>` reads to its true end.
///
/// Matched as a whitespace-delimited token inside the attribute, not as a substring: the
/// emitted class lists are compound (`callout callout-note callout-collapse`), so a
/// substring test for `callout` would also be satisfied by `callout-title` and a test for
/// `tali-theorem` by `tali-theorem-body`.
fn class_spans<'a>(html: &'a str, class: &str) -> Vec<(usize, usize, &'a str)> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(rel) = html[pos..].find(class) {
        let at = pos + rel;
        let after = html[at + class.len()..].chars().next();
        let before = html[..at].chars().next_back();
        // A token boundary on both sides, and inside a `class="…"` attribute.
        let token =
            matches!(after, Some('"') | Some(' ')) && matches!(before, Some('"') | Some(' '));
        if !token || !in_class_attr(html, at) {
            pos = at + class.len();
            continue;
        }
        let Some(lt) = html[..at].rfind('<') else {
            pos = at + class.len();
            continue;
        };
        let name: String = html[lt + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        let Some(gt) = html[lt..].find('>') else {
            break;
        };
        let start = lt + gt + 1;
        let (open, close) = (format!("<{name}"), format!("</{name}>"));
        let mut depth = 1usize;
        let mut scan = start;
        let end = loop {
            let next_open = html[scan..].find(&open).map(|i| scan + i);
            let next_close = html[scan..].find(&close).map(|i| scan + i);
            match (next_open, next_close) {
                (Some(o), Some(c)) if o < c => {
                    depth += 1;
                    scan = o + open.len();
                }
                (_, Some(c)) => {
                    depth -= 1;
                    if depth == 0 {
                        break Some(c);
                    }
                    scan = c + close.len();
                }
                _ => break None,
            }
        };
        let Some(end) = end else { break };
        out.push((lt, end + close.len(), &html[start..end]));
        pos = end + close.len();
    }
    out
}

/// Whether byte `at` sits inside a `class="…"` attribute value — i.e. the nearest preceding
/// `class="` is closer than the nearest preceding `"` that would have closed it. Keeps a
/// class token from matching the same word in body text or in another attribute.
fn in_class_attr(html: &str, at: usize) -> bool {
    let before = &html[..at];
    match (before.rfind("class=\""), before.rfind('"')) {
        (Some(c), Some(q)) => c + 6 >= q,
        _ => false,
    }
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

    /// The nav's cost signal and `skim`/`map`'s `words` must be one number, and the only
    /// shape that can tell them apart is a chapter assembled from `{{< include >}}`.
    /// `corpus/tarn` has no such chapter — nor does any book in the repo — so the
    /// cross-surface pin over tarn passes with this fix deleted (checked: it does).
    /// Mint the missing shape here rather than leave the guard vacuous.
    #[test]
    fn an_include_built_chapter_skims_at_the_length_a_reader_will_read() {
        let dir = std::env::temp_dir().join(format!(
            "tali-skim-include-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("_site.yml"),
            "title: \"Inc\"\nchapters:\n  - a.tmd\n",
        )
        .unwrap();
        std::fs::write(dir.join("a.tmd"), "# Assembled\n\n{{< include _p.tmd >}}\n").unwrap();
        std::fs::write(
            dir.join("_p.tmd"),
            "one two three four five six seven eight\n",
        )
        .unwrap();
        let site = crate::site::Site::discover(&dir);
        let skimmed = site.skim();
        let entry = site
            .book
            .as_ref()
            .and_then(|b| b.chapters().first().copied().cloned())
            .expect("the book has one chapter");
        std::fs::remove_dir_all(&dir).ok();
        let page = skimmed
            .iter()
            .find(|p| p.url == "a.html")
            .expect("the chapter skims");
        assert_eq!(
            page.words, 9,
            "`Assembled` (1) + the 8 words the include pulls in"
        );
        assert_eq!(
            page.words, entry.words,
            "the skim projection and the nav's cost signal must be one number"
        );
    }

    #[test]
    fn a_plain_sentence_stops_at_its_period() {
        assert_eq!(
            first_sentence("One thing. Then another."),
            Some("One thing.".to_string())
        );
    }

    #[test]
    fn empty_prose_has_no_sentence() {
        assert_eq!(first_sentence("   "), None);
        assert_eq!(first_sentence(""), None);
    }

    #[test]
    fn a_text_with_no_terminator_is_all_one_sentence() {
        assert_eq!(
            first_sentence("A fragment with no stop"),
            Some("A fragment with no stop".to_string())
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
    fn an_initial_does_not_end_a_sentence() {
        assert_eq!(
            first_sentence("Named for A. Turing, who proved it. Later."),
            Some("Named for A. Turing, who proved it.".to_string())
        );
    }

    #[test]
    fn an_ellipsis_does_not_end_a_sentence() {
        assert_eq!(
            first_sentence("It trails off... Then resumes here. End."),
            Some("It trails off... Then resumes here.".to_string())
        );
    }

    #[test]
    fn a_question_or_bang_ends_a_sentence_and_a_run_counts_once() {
        assert_eq!(
            first_sentence("Does it work? Yes."),
            Some("Does it work?".to_string())
        );
        assert_eq!(
            first_sentence("It works?! Really."),
            Some("It works?!".to_string())
        );
    }

    #[test]
    fn a_terminator_at_the_very_end_keeps_the_whole_text() {
        assert_eq!(first_sentence("Only one."), Some("Only one.".to_string()));
    }

    #[test]
    fn a_lowercase_continuation_is_not_a_new_sentence() {
        // `.` then lowercase is a file name or version, not a boundary.
        assert_eq!(
            first_sentence("Edit the file client.js then rebuild. Done."),
            Some("Edit the file client.js then rebuild.".to_string())
        );
    }

    #[test]
    fn a_period_with_no_following_space_is_not_a_boundary() {
        assert_eq!(
            first_sentence("See render/mod.Rs for the pipeline. Next."),
            Some("See render/mod.Rs for the pipeline.".to_string())
        );
    }
}
