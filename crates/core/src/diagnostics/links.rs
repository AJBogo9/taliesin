//! Local relative cross-file link existence validation, plus the link-text collision
//! lint (two links on a page that read the same but go somewhere different).

use super::a11y::interactives;
use super::helpers::{is_local_ref, start_line, strip_tags, tag_attr};
use crate::render::{Block, Warning};
use std::path::Path;

/// Unique local link targets from MANUAL `<a href>` tags only, paired with their tag
/// span so the caller can locate each. Cross-reference links (`tali-xref`) are skipped
/// (validated by `validate_xrefs`); bare in-page `#fragment` links are skipped (validated
/// by [`super::anchors::validate_internal_anchors`]). Returns each `href` value verbatim
/// (path + optional `#frag`), so a caller can split the path from the fragment.
fn local_link_refs(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find("<a ") {
        let tag_start = i + pos;
        let Some(rel_end) = html[tag_start..].find('>') else {
            break;
        };
        let tag = &html[tag_start..tag_start + rel_end];
        i = tag_start + rel_end + 1;
        if tag.contains("tali-xref") {
            continue; // a cross-reference, validated separately
        }
        let Some(val) = tag_attr(tag, "href=\"") else {
            continue;
        };
        // A bare in-page anchor (`#frag`) is `validate_internal_anchors`'s job.
        if val.starts_with('#') {
            continue;
        }
        if is_local_ref(val) && !out.contains(&val) {
            out.push(val);
        }
    }
    out
}

/// Whether `path` (relative to a doc at `base`) is backed by a file on disk, accepting the
/// forms a site build produces: the literal file, a `.html` link whose source (any
/// extension in [`crate::ext::ACCEPTED_SOURCE_EXTS`]) exists, and a directory link (`x/` or
/// `x`) whose `x/index.<ext>`/`.html` exists. So a single-doc check doesn't false-flag an
/// intra-project `.html` link whose source page is present (the site build will emit the
/// `.html`).
fn link_target_exists(base: &Path, path: &str) -> bool {
    let join = |p: &str| base.join(p).is_file();
    if join(path) {
        return true;
    }
    // `x.html` → its source (the built page is produced from it), tried in every
    // accepted spelling (`x.tmd`).
    if let Some(stem) = path.strip_suffix(".html")
        && crate::ext::ACCEPTED_SOURCE_EXTS
            .iter()
            .any(|ext| join(&format!("{stem}.{ext}")))
    {
        return true;
    }
    // A directory link (`dir/` or `dir`) → that dir's index page (source or `.html`).
    let dir = path.trim_end_matches('/');
    crate::ext::ACCEPTED_SOURCE_EXTS
        .iter()
        .any(|ext| base.join(format!("{dir}/index.{ext}")).is_file())
        || base.join(format!("{dir}/index.html")).is_file()
}

/// Manual relative links (`[text](other.tmd)`, `[text](sub/page.html#x)`) whose local
/// **target file** does not exist under the doc base dir — a broken cross-file jump that
/// ships silently. External (`http(s)://`, `mailto:`, …) and absolute (`/…`) links are
/// out of scope (external links are never fetched — `check` stays offline + deterministic);
/// bare `#anchor` links and the in-page fragment are handled by
/// [`super::anchors::validate_internal_anchors`]. Cross-page `#fragment` resolution is a
/// site-registry job (the server's site path resolves anchors). A `.html` link whose source
/// exists on disk (any accepted extension) is accepted (see [`link_target_exists`]) so an
/// intra-project link to a yet-to-be-built page is not false-flagged — only a target with no
/// file *and* no source is broken.
pub fn validate_local_links(blocks: &[Block], base: &Path) -> Vec<Warning> {
    let mut out = Vec::new();
    for b in blocks {
        let line = start_line(&b.sourcepos);
        for val in local_link_refs(&b.html) {
            let path = &val[..val.find(['?', '#']).unwrap_or(val.len())];
            if path.is_empty() || path.starts_with('/') || link_target_exists(base, path) {
                continue;
            }
            // Nothing on disk backs it — but a document inside a site may legitimately link
            // a project that site MOUNTS, which resolves by URL prefix and so has no file
            // under this document's directory. Asked only here, on a link already about to
            // be reported, so the common path still costs nothing.
            if crate::site::link_targets_enclosing_mount(base, path) {
                continue;
            }
            // A migrated document's links keep the extension the old tool used, and the
            // answer is usually sitting right next to the link (item 128). Suggest, never
            // rewrite: a `.md` link may point at a real shipped `.md`.
            let hint = crate::ext::migrated_source_candidates(path)
                .into_iter()
                .find(|c| base.join(c).is_file())
                .map(|c| format!(" (did you mean `{c}`?)"))
                .unwrap_or_default();
            let w = Warning::new(format!(
                "broken link: `{path}` (no such file under the document directory){hint}"
            ));
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}

/// A link's accessible name, normalized for comparison: its visible text (or its
/// `aria-label`, which overrides the text), lowercased and whitespace-collapsed.
///
/// `aria-label` wins because that is what assistive tech announces — and a page whose
/// two "Read more" links carry distinguishing `aria-label`s has already solved this.
fn link_name(open: &str, inner: &str) -> String {
    let raw = tag_attr(open, "aria-label=\"")
        .map(str::to_string)
        .unwrap_or_else(|| strip_tags(inner));
    raw.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Two links in one document that read the same but point somewhere different — the
/// reader (and a screen-reader user pulling up a links list, where the link text is
/// *all* they get) cannot tell which is which.
///
/// Compared **modulo fragment**: two `#`-deep links into the same document are one
/// destination for this purpose. That is the whole difference between this rule and a
/// false-positive factory, and it is the reason the audit gave for trimming it. It is
/// also what silences footnote back-references, measured rather than assumed: every one
/// on a page reads "Back to content" (an `aria-label`) and points at a *bare* fragment,
/// so they all share one destination here. (The audit's proposed `data-footnote-ref` /
/// `data-footnote-backref` exemption would have been dead code — this project emits
/// `role="doc-noteref"` and `class="tali-fn-back"`, not comrak's attributes.)
///
/// **Cross-references are exempt** because the author does not write their text: an
/// *unnumbered* theorem renders every reference to it as a bare "Theorem", so two refs
/// to two different unnumbered theorems collide on text the author cannot reword without
/// abandoning `@`-references.
///
/// **Scope is the document's own blocks**, so site chrome (navbar, footer, TOC, pager)
/// is out of view by construction. That is correct for a lint whose fix is "edit your
/// prose": the author does not write the TOC's link text, and a `#`-deep TOC has one
/// destination under this rule anyway.
///
/// The "here"/"read more" stop-list from the same audit finding is deliberately NOT
/// here: it fires zero times across all corpus + docs files, and `crate::prose` is this
/// project's existing style linter and is opt-in behind `prose-lint:` on purpose.
pub fn validate_link_text_collisions(blocks: &[Block]) -> Vec<Warning> {
    use std::collections::{HashMap, HashSet};
    let mut seen: HashMap<String, String> = HashMap::new(); // name → destination, sans fragment
    let mut reported: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for b in blocks {
        for el in interactives(&b.html) {
            if el.kind != "link" || el.open.contains("tali-xref") {
                continue;
            }
            let Some(href) = tag_attr(el.open, "href=\"") else {
                continue;
            };
            let dest = &href[..href.find('#').unwrap_or(href.len())];
            let name = link_name(el.open, el.inner);
            if name.is_empty() {
                continue; // an unnamed link is `validate_a11y`'s finding, not this one
            }
            match seen.get(&name) {
                // Reported once per phrase, and `seen` keeps the FIRST destination: a
                // third copy of the same text is the same defect, and three findings on
                // one phrase is noise, not three times the signal.
                Some(first) if first != dest && reported.insert(name.clone()) => {
                    let w = Warning::new(format!(
                        "ambiguous link text `{name}`: another link on this page reads the same \
                         but points somewhere else, so neither one says where it goes"
                    ));
                    out.push(match start_line(&b.sourcepos) {
                        Some(l) => w.at(b.source_file.clone(), l),
                        None => w,
                    });
                }
                Some(_) => {}
                None => {
                    seen.insert(name, dest.to_string());
                }
            }
        }
    }
    out
}
