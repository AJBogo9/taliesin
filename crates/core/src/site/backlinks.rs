//! Cross-reference *backlinks*: the reverse of `xref.rs`. `xref` maps a reference
//! (`@fig-x`) forward to its target; this maps a target back to the pages that
//! reference it, and injects a quiet "Referenced by" line into the target's block.
//! `use super::*` reaches `Site`, `Block`, `XrefTarget`, `HashMap`, `esc`.

use super::*;

/// One referring page in the reverse index: where the reference is made, and the
/// sentence it is made in.
///
/// The sentence is the point. A bare page title is a weak proximal cue — the reader
/// cannot tell an aside from the passage that actually builds on this target — while
/// the citing sentence is the strongest cue available and costs the author nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backref {
    /// The referring page's url, relative to the site root.
    pub url: String,
    /// The sentence on that page containing the reference, already plain text and
    /// truncated. `None` when the reference sits somewhere no sentence can be read
    /// out of (an image alt, a stripped subtree).
    pub context: Option<String>,
}

/// Build the reverse index from each page's cross-page reference markers: for every
/// known target `A` a page references, record that page's url + citing sentence under
/// `A`. `per_page` is `(page url, that page's `(anchor, citing sentence)` pairs)` in
/// **site page order**, so each target's referrer list comes out in document order.
/// Referrers are deduped per page (a page that cites `@fig-x` three times is one
/// referrer, keeping the FIRST mention's sentence — the one a reader scanning that page
/// meets first), and a marker whose anchor is not a known target (a dangling reference)
/// contributes nothing.
pub(super) fn build_backlink_index(
    per_page: &[(String, Vec<(String, Option<String>)>)],
    targets: &HashMap<String, XrefTarget>,
) -> HashMap<String, Vec<Backref>> {
    let mut map: HashMap<String, Vec<Backref>> = HashMap::new();
    for (url, refs) in per_page {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (anchor, context) in refs {
            let Some(target) = targets.get(anchor) else {
                continue; // dangling reference — not a known target
            };
            // Never list a page as a referrer of an anchor it defines (belt-and-braces
            // against a same-page marker from a kind/element-mismatched anchor).
            if target.url == *url {
                continue;
            }
            if seen.insert(anchor.as_str()) {
                map.entry(anchor.clone()).or_default().push(Backref {
                    url: url.clone(),
                    context: context.clone(),
                });
            }
        }
    }
    map
}

/// Past this many referrers the line drops the citing sentences and falls back to the
/// bare list of titles. The whole value of the line is that it is a one-line whisper
/// beside the target; three quoted sentences is a paragraph, and a paragraph of
/// apparatus competes with the thing it annotates.
const MAX_CITED_REFERRERS: usize = 2;

/// Longest citing sentence rendered, in characters. Long enough for a real sentence of
/// technical prose (the longest one in the corpus is 148), short enough that two of
/// them still read as one whisper.
const MAX_CONTEXT_CHARS: usize = 160;

/// The sentence containing the cross-reference to `anchor` in one referring block's
/// **resolved** HTML (post-[`xref::rewrite_cross_refs`], so the sentence quotes
/// "Theorem 2.1" exactly as the referring page shows it, not cite's bare "Theorem").
///
/// Located by the `#anchor` fragment in the link's `href`, not by the `data-tali-xref`
/// marker: the resolver consumes that marker, and an *unresolved* link (unknown or
/// same-page target) keeps the same fragment, so one needle covers both shapes.
///
/// The reference's position has to survive tag-stripping to be found in the plain text,
/// so a NUL is planted just inside the link's open tag and read back out of the stripped
/// string. Splitting the HTML and stripping the two halves separately does not work:
/// [`skim::plain`] trims and re-joins on whitespace, so the space separating "in" from
/// the link would be lost on one side and invented on the other.
pub(super) fn citing_sentence(html: &str, anchor: &str) -> Option<String> {
    const MARK: char = '\u{0}';
    let at = html.find(&format!("#{anchor}\""))?;
    let lt = html[..at].rfind('<')?;
    let gt = lt + html[lt..].find('>')?;
    let marked = format!("{}{MARK}{}", &html[..=gt], &html[gt + 1..]);
    let text = skim::plain(&marked);
    let mark_at = text.find(MARK)?;
    let text = text.replace(MARK, "");
    let sentence = skim::sentence_at(&text, mark_at)?;
    Some(truncate_chars(&sentence, MAX_CONTEXT_CHARS))
}

/// Cap `s` at `max` characters, cutting at a word boundary and marking the cut with an
/// ellipsis. Returns `s` unchanged when it already fits.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    let head = &s[..cut];
    // Cut back to the last word boundary; a sentence with no space inside `max` chars is
    // one long token, and cutting it mid-token beats emitting all of it.
    let end = head.rfind(char::is_whitespace).unwrap_or(head.len());
    format!("{}\u{2026}", head[..end].trim_end())
}

/// The muted "Referenced by" line for the target `anchor`, listing each referring
/// page as a quiet link. Emitted as a single-root block carrying its own
/// `data-block-id` (`tali-backref-<anchor>`) — the incremental client mounts only a
/// block's `firstElementChild`, so the line must be one root element, spliced into
/// the block stream as its own block rather than appended to the target's block.
/// `referrers` is `(href, page title, citing sentence)` in document order; the href is
/// already root-adjusted (`../`-prefixed) by the caller. Empty in → empty out (the
/// caller only calls this for a target that has referrers, but never emit a stray
/// `<div>`).
///
/// The citing sentence renders as plain quoted text **beside** the link, never inside
/// it: the sentence carries the reference's own `<a>` on the referring page, and an
/// anchor inside an anchor is invalid HTML. It is dropped past [`MAX_CITED_REFERRERS`].
fn render_backrefs_line(anchor: &str, referrers: &[(String, String, Option<String>)]) -> String {
    if referrers.is_empty() {
        return String::new();
    }
    let cited = referrers.len() <= MAX_CITED_REFERRERS;
    // The id is raw (unescaped): ref anchors are ASCII (`is_ref_anchor` prefixes +
    // the attribute-id charset), so it needs no escaping and must byte-match the
    // `Block.id` the diff keys on (built the same way in `attach_backlinks`). The `↳`
    // is decorative — `aria-hidden` so assistive tech announces only "Referenced by …".
    let mut out = format!(
        "<div class=\"tali-backrefs\" data-block-id=\"tali-backref-{anchor}\">\
         <span aria-hidden=\"true\">\u{21b3}</span> Referenced by "
    );
    for (i, (href, label, context)) in referrers.iter().enumerate() {
        if i > 0 {
            out.push_str(" \u{00b7} "); // middot separator
        }
        out.push_str(&format!(
            "<a href=\"{}\" class=\"tali-backref\">{}</a>",
            esc(href),
            esc(label)
        ));
        if let Some(sentence) = context.as_deref().filter(|_| cited) {
            out.push_str(&format!(
                " <span class=\"tali-backref-cite\">\u{201c}{}\u{201d}</span>",
                esc(sentence)
            ));
        }
    }
    out.push_str("</div>");
    out
}

impl Site {
    /// Splice the quiet "Referenced by" backlink line in after each target defined on
    /// this page that other pages cross-reference. Called right after
    /// [`resolve_cross_refs`](Site::resolve_cross_refs) in `finish_blocks`, so the
    /// static build and the live preview inject identically. Each line is its own
    /// single-root block (`tali-backref-<anchor>`, no sourcepos) so the incremental
    /// client mounts it cleanly. A no-op when nothing cross-references this page's
    /// targets.
    pub(super) fn attach_backlinks(&self, blocks: &mut Vec<Block>, current_url: &str) {
        if self.backlinks.is_empty() {
            return;
        }
        // The referred-to anchors defined ON this page, with their referrer lists.
        let mine: Vec<(&str, &Vec<Backref>)> = self
            .backlinks
            .iter()
            .filter(|(anchor, referrers)| {
                !referrers.is_empty()
                    && self.xref_targets.get(*anchor).map(|t| t.url.as_str()) == Some(current_url)
            })
            .map(|(a, r)| (a.as_str(), r))
            .collect();
        if mine.is_empty() {
            return;
        }
        let up = "../".repeat(current_url.matches('/').count());
        let mut out: Vec<Block> = Vec::with_capacity(blocks.len() + mine.len());
        for b in std::mem::take(blocks) {
            // Which of this page's referred-to anchors does this block define?
            let mut here: Vec<(&str, &Vec<Backref>)> = mine
                .iter()
                .filter(|(anchor, _)| b.html.contains(&format!("id=\"{anchor}\"")))
                .copied()
                .collect();
            here.sort_by(|a, b| a.0.cmp(b.0)); // deterministic if a block defines >1
            out.push(b);
            for (anchor, referrers) in here {
                let links: Vec<(String, String, Option<String>)> = referrers
                    .iter()
                    .map(|r| {
                        (
                            format!("{up}{}", r.url),
                            self.referrer_label(&r.url),
                            r.context.clone(),
                        )
                    })
                    .collect();
                let html = render_backrefs_line(anchor, &links);
                if !html.is_empty() {
                    out.push(Block {
                        id: format!("tali-backref-{anchor}"),
                        sourcepos: String::new(),
                        source_file: None,
                        html,
                        cell: None,
                    });
                }
            }
        }
        *blocks = out;
    }

    /// The display label for a referring page: its front-matter/chapter title, else
    /// its rel path (the existing listing-card fallback), else its url.
    fn referrer_label(&self, url: &str) -> String {
        self.pages
            .iter()
            .find(|p| p.url == url)
            .map(|p| p.title.clone().unwrap_or_else(|| p.rel.clone()))
            .unwrap_or_else(|| url.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_backlink_index_records_referring_pages_dedup_and_ordered() {
        let targets = HashMap::from([
            ("fig-a".to_string(), XrefTarget::default()),
            ("sec-b".to_string(), XrefTarget::default()),
        ]);
        // page order = site order; p1 refers to fig-a twice (dedup) + a dangling
        // anchor (excluded, not a known target); p2 refers to sec-b then fig-a.
        let per_page = vec![
            (
                "p1.html".to_string(),
                vec![
                    ("fig-a".to_string(), Some("First mention.".to_string())),
                    ("fig-a".to_string(), Some("Second mention.".to_string())),
                    ("fig-dangling".to_string(), None),
                ],
            ),
            (
                "p2.html".to_string(),
                vec![
                    ("sec-b".to_string(), None),
                    ("fig-a".to_string(), Some("Elsewhere.".to_string())),
                ],
            ),
        ];
        let idx = build_backlink_index(&per_page, &targets);
        assert_eq!(
            idx.get("fig-a"),
            Some(&vec![
                Backref {
                    url: "p1.html".to_string(),
                    // The FIRST mention's sentence, not the last: a reader scanning the
                    // referring page meets that one first.
                    context: Some("First mention.".to_string()),
                },
                Backref {
                    url: "p2.html".to_string(),
                    context: Some("Elsewhere.".to_string()),
                },
            ])
        );
        assert_eq!(
            idx.get("sec-b"),
            Some(&vec![Backref {
                url: "p2.html".to_string(),
                context: None,
            }])
        );
        assert_eq!(idx.get("fig-dangling"), None);
    }

    #[test]
    fn build_backlink_index_excludes_self_referral() {
        // A page must never list itself as a referrer of an anchor it defines. Cite's
        // marker discipline normally prevents this (no marker for a same-page ref), but
        // a kind/element-mismatched anchor (`## Heading {#thm-x}`) can slip a same-page
        // marker through — the index guards against it structurally.
        let targets = HashMap::from([(
            "thm-x".to_string(),
            XrefTarget {
                url: "p1.html".to_string(),
                number: String::new(),
            },
        )]);
        let per_page = vec![
            ("p1.html".to_string(), vec![("thm-x".to_string(), None)]), // self-reference
            ("p2.html".to_string(), vec![("thm-x".to_string(), None)]), // legitimate cross-page
        ];
        let idx = build_backlink_index(&per_page, &targets);
        assert_eq!(
            idx.get("thm-x"),
            Some(&vec![Backref {
                url: "p2.html".to_string(),
                context: None,
            }]),
            "the defining page (p1) must not list itself as a referrer"
        );
    }

    #[test]
    fn render_backrefs_line_is_a_single_root_block_with_escaped_links() {
        let html = render_backrefs_line(
            "fig-scree",
            &[
                ("../methods.html".to_string(), "Methods".to_string(), None),
                (
                    "../results.html".to_string(),
                    "Results & More".to_string(),
                    Some("Compare with A & B.".to_string()),
                ),
            ],
        );
        // A single-root block carrying its own data-block-id (the incremental client
        // mounts only `firstElementChild`, so the line must be one root element).
        assert!(
            html.starts_with(
                r#"<div class="tali-backrefs" data-block-id="tali-backref-fig-scree">"#
            )
        );
        assert!(html.trim_end().ends_with("</div>"));
        assert_eq!(html.matches("<div").count(), 1, "exactly one root element");
        assert!(html.contains("Referenced by"));
        assert!(html.contains(r#"<a href="../methods.html" class="tali-backref">Methods</a>"#));
        assert!(html.contains("Results &amp; More"), "label must be escaped");
        assert!(html.contains('·'), "referrers joined by a middot separator");
        assert!(
            html.contains(r#"<span class="tali-backref-cite">“Compare with A &amp; B.”</span>"#),
            "the citing sentence renders beside the link, escaped: {html}"
        );
    }

    #[test]
    fn a_citing_sentence_is_never_nested_inside_the_referrer_link() {
        // The sentence carries the reference's own `<a>` on the referring page, so
        // putting it inside the backlink's anchor would nest anchors (invalid HTML) and
        // swallow the sentence into the link's accessible name. It is a sibling span.
        let html = render_backrefs_line(
            "thm-kl",
            &[(
                "results.html".to_string(),
                "Results".to_string(),
                Some("It leans on Theorem 2.1.".to_string()),
            )],
        );
        let anchor = html
            .split(r#"<a href="results.html""#)
            .nth(1)
            .expect("the referrer link is present")
            .split("</a>")
            .next()
            .unwrap();
        assert!(
            !anchor.contains("tali-backref-cite"),
            "the citing sentence must sit outside the anchor, not inside it: {html}"
        );
        assert!(html.contains(r#"</a> <span class="tali-backref-cite">"#));
    }

    #[test]
    fn past_two_referrers_the_line_drops_back_to_bare_titles() {
        // Three quoted sentences is a paragraph of apparatus beside the target; the
        // whisper has to stay a whisper, so the sentences are dropped wholesale rather
        // than truncated harder.
        let refs = |n: usize| -> Vec<(String, String, Option<String>)> {
            (0..n)
                .map(|i| {
                    (
                        format!("p{i}.html"),
                        format!("Page {i}"),
                        Some(format!("Sentence {i}.")),
                    )
                })
                .collect()
        };
        let cited =
            |n: usize| render_backrefs_line("fig-x", &refs(n)).contains("tali-backref-cite");
        assert!(cited(1), "one referrer keeps its sentence");
        assert!(cited(2), "two referrers keep their sentences");
        assert!(!cited(3), "three referrers fall back to bare titles");
        // The titles themselves must survive the fallback.
        let html = render_backrefs_line("fig-x", &refs(3));
        assert_eq!(html.matches(r#"class="tali-backref""#).count(), 3);
        assert!(!html.contains("Sentence 0."));
    }

    #[test]
    fn render_backrefs_line_is_empty_for_no_referrers() {
        assert_eq!(render_backrefs_line("fig-x", &[]), "");
    }

    #[test]
    fn citing_sentence_quotes_the_sentence_the_reference_sits_in() {
        // Two sentences, the reference in the second: the first must not leak in.
        let html = "<p>The setup is described elsewhere. It runs the stages in \
                    <a href=\"methods.html#fig-pipeline\" class=\"tali-xref\">Figure&nbsp;2.1</a>, \
                    which refines the overview.</p>";
        assert_eq!(
            citing_sentence(html, "fig-pipeline").as_deref(),
            Some("It runs the stages in Figure 2.1, which refines the overview.")
        );
    }

    #[test]
    fn citing_sentence_reads_the_resolved_number_not_cites_bare_label() {
        // The whole reason the sentence is harvested AFTER the registry is final: an
        // unresolved marker reads "in Section (in particular…)", which is not a sentence
        // any reader ever sees.
        let resolved = "<p>Building on the methods in \
                        <a href=\"methods.html#sec-methods\" class=\"tali-xref\">Chapter&nbsp;2</a>.</p>";
        assert_eq!(
            citing_sentence(resolved, "sec-methods").as_deref(),
            Some("Building on the methods in Chapter 2.")
        );
    }

    #[test]
    fn citing_sentence_finds_an_unresolved_reference_by_its_fragment() {
        // A marker whose target the registry does not know keeps cite's bare-label link,
        // `href="#anchor"` with the marker still on it. Same needle, both shapes.
        let unresolved = "<p>See <a href=\"#fig-ghost\" class=\"tali-xref\" \
                          data-tali-xref=\"fig-ghost\">Figure</a> here.</p>";
        assert_eq!(
            citing_sentence(unresolved, "fig-ghost").as_deref(),
            Some("See Figure here.")
        );
    }

    #[test]
    fn citing_sentence_keeps_inline_code_flush_against_its_punctuation() {
        // `indexable_text` puts a space at every tag boundary, which reads as "the
        // `plan` ." on a display string. Sharing `skim::plain` is what avoids it — and
        // inline code is in most of the paragraphs this will ever quote.
        let html = "<p>It is keyed by <code>plan</code>, exactly as \
                    <a href=\"exec.html#sec-plan\" class=\"tali-xref\">Chapter&nbsp;4</a>.</p>";
        assert_eq!(
            citing_sentence(html, "sec-plan").as_deref(),
            Some("It is keyed by plan, exactly as Chapter 4.")
        );
    }

    #[test]
    fn citing_sentence_truncates_a_long_sentence_at_a_word_boundary() {
        let tail = "word ".repeat(60);
        let html = format!(
            "<p>See <a href=\"a.html#fig-x\" class=\"tali-xref\">Figure&nbsp;1</a> {tail}end.</p>"
        );
        let got = citing_sentence(&html, "fig-x").expect("a sentence");
        assert!(got.chars().count() <= MAX_CONTEXT_CHARS + 1, "got {got:?}");
        assert!(
            got.ends_with("word\u{2026}"),
            "cut at a word boundary: {got:?}"
        );
        assert!(got.starts_with("See Figure 1 word"));
    }

    #[test]
    fn citing_sentence_is_none_when_the_anchor_is_not_in_the_block() {
        assert_eq!(citing_sentence("<p>No references here.</p>", "fig-x"), None);
    }
}
