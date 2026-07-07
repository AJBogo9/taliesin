//! Cross-reference *backlinks*: the reverse of `xref.rs`. `xref` maps a reference
//! (`@fig-x`) forward to its target; this maps a target back to the pages that
//! reference it, and injects a quiet "Referenced by" line into the target's block.
//! `use super::*` reaches `Site`, `Block`, `XrefTarget`, `HashMap`, `esc`.

use super::*;

/// Build the reverse index from each page's cross-page reference markers: for every
/// known target `A` a page references, record that page's url under `A`. `per_page`
/// is `(page url, that page's `data-qmd-xref` anchors)` in **site page order**, so
/// each target's referrer list comes out in document order. Referrers are deduped
/// per page (a page that cites `@fig-x` three times is one referrer), and a marker
/// whose anchor is not a known target (a dangling reference) contributes nothing.
pub(super) fn build_backlink_index(
    per_page: &[(String, Vec<String>)],
    targets: &HashMap<String, XrefTarget>,
) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for (url, anchors) in per_page {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for anchor in anchors {
            let Some(target) = targets.get(anchor) else {
                continue; // dangling reference — not a known target
            };
            // Never list a page as a referrer of an anchor it defines (belt-and-braces
            // against a same-page marker from a kind/element-mismatched anchor).
            if target.url == *url {
                continue;
            }
            if seen.insert(anchor.as_str()) {
                map.entry(anchor.clone()).or_default().push(url.clone());
            }
        }
    }
    map
}

/// The muted "Referenced by" line for the target `anchor`, listing each referring
/// page as a quiet link. Emitted as a single-root block carrying its own
/// `data-block-id` (`qmd-backref-<anchor>`) — the incremental client mounts only a
/// block's `firstElementChild`, so the line must be one root element, spliced into
/// the block stream as its own block rather than appended to the target's block.
/// `referrers` is `(href, page title)` in document order; the href is already
/// root-adjusted (`../`-prefixed) by the caller. Empty in → empty out (the caller
/// only calls this for a target that has referrers, but never emit a stray `<div>`).
fn render_backrefs_line(anchor: &str, referrers: &[(String, String)]) -> String {
    if referrers.is_empty() {
        return String::new();
    }
    // The id is raw (unescaped): ref anchors are ASCII (`is_ref_anchor` prefixes +
    // the attribute-id charset), so it needs no escaping and must byte-match the
    // `Block.id` the diff keys on (built the same way in `attach_backlinks`). The `↳`
    // is decorative — `aria-hidden` so assistive tech announces only "Referenced by …".
    let mut out = format!(
        "<div class=\"tali-backrefs\" data-block-id=\"qmd-backref-{anchor}\">\
         <span aria-hidden=\"true\">\u{21b3}</span> Referenced by "
    );
    for (i, (href, label)) in referrers.iter().enumerate() {
        if i > 0 {
            out.push_str(" \u{00b7} "); // middot separator
        }
        out.push_str(&format!(
            "<a href=\"{}\" class=\"tali-backref\">{}</a>",
            esc(href),
            esc(label)
        ));
    }
    out.push_str("</div>");
    out
}

impl Site {
    /// Splice the quiet "Referenced by" backlink line in after each target defined on
    /// this page that other pages cross-reference. Called right after
    /// [`resolve_cross_refs`](Site::resolve_cross_refs) in `finish_blocks`, so the
    /// static build and the live preview inject identically. Each line is its own
    /// single-root block (`qmd-backref-<anchor>`, no sourcepos) so the incremental
    /// client mounts it cleanly. A no-op when nothing cross-references this page's
    /// targets.
    pub(super) fn attach_backlinks(&self, blocks: &mut Vec<Block>, current_url: &str) {
        if self.backlinks.is_empty() {
            return;
        }
        // The referred-to anchors defined ON this page, with their referrer lists.
        let mine: Vec<(&str, &Vec<String>)> = self
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
            let mut here: Vec<(&str, &Vec<String>)> = mine
                .iter()
                .filter(|(anchor, _)| b.html.contains(&format!("id=\"{anchor}\"")))
                .copied()
                .collect();
            here.sort_by(|a, b| a.0.cmp(b.0)); // deterministic if a block defines >1
            out.push(b);
            for (anchor, referrers) in here {
                let links: Vec<(String, String)> = referrers
                    .iter()
                    .map(|url| (format!("{up}{url}"), self.referrer_label(url)))
                    .collect();
                let html = render_backrefs_line(anchor, &links);
                if !html.is_empty() {
                    out.push(Block {
                        id: format!("qmd-backref-{anchor}"),
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
                    "fig-a".to_string(),
                    "fig-a".to_string(),
                    "fig-dangling".to_string(),
                ],
            ),
            (
                "p2.html".to_string(),
                vec!["sec-b".to_string(), "fig-a".to_string()],
            ),
        ];
        let idx = build_backlink_index(&per_page, &targets);
        assert_eq!(
            idx.get("fig-a"),
            Some(&vec!["p1.html".to_string(), "p2.html".to_string()])
        );
        assert_eq!(idx.get("sec-b"), Some(&vec!["p2.html".to_string()]));
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
            ("p1.html".to_string(), vec!["thm-x".to_string()]), // self-reference
            ("p2.html".to_string(), vec!["thm-x".to_string()]), // legitimate cross-page
        ];
        let idx = build_backlink_index(&per_page, &targets);
        assert_eq!(
            idx.get("thm-x"),
            Some(&vec!["p2.html".to_string()]),
            "the defining page (p1) must not list itself as a referrer"
        );
    }

    #[test]
    fn render_backrefs_line_is_a_single_root_block_with_escaped_links() {
        let html = render_backrefs_line(
            "fig-scree",
            &[
                ("../methods.html".to_string(), "Methods".to_string()),
                ("../results.html".to_string(), "Results & More".to_string()),
            ],
        );
        // A single-root block carrying its own data-block-id (the incremental client
        // mounts only `firstElementChild`, so the line must be one root element).
        assert!(
            html.starts_with(
                r#"<div class="tali-backrefs" data-block-id="qmd-backref-fig-scree">"#
            )
        );
        assert!(html.trim_end().ends_with("</div>"));
        assert_eq!(html.matches("<div").count(), 1, "exactly one root element");
        assert!(html.contains("Referenced by"));
        assert!(html.contains(r#"<a href="../methods.html" class="tali-backref">Methods</a>"#));
        assert!(html.contains("Results &amp; More"), "label must be escaped");
        assert!(html.contains('·'), "referrers joined by a middot separator");
    }

    #[test]
    fn render_backrefs_line_is_empty_for_no_referrers() {
        assert_eq!(render_backrefs_line("fig-x", &[]), "");
    }
}
