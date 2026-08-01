//! Page + deck discovery: walk `.tmd` files under the project root (path-ordered) into
//! [`Page`]s, and find loose reveal decks. The filesystem-walking front end of
//! `Site::discover`.

use std::collections::HashSet;

use super::*;

/// A website's pages: every `.tmd` under `root` (path-ordered), each mapped to a
/// [`Page`] from its front matter.
pub(super) fn website_pages(
    root: &Path,
    mode: DraftMode,
    warnings: &mut Vec<String>,
    excluded: &mut Vec<String>,
) -> Vec<Page> {
    let mut inputs = Vec::new();
    collect_pages(root, &mut inputs);
    inputs.sort();
    let mut pages: Vec<Page> = inputs
        .into_iter()
        .filter_map(|input| {
            let rel = rel_str(root, &input);
            let url = tmd_to_html(&rel);
            let fm = parse_front_matter(&input, &rel, warnings);
            // `draft: true`: dropped from the published set (Exclude) — recorded so the
            // build can report it — or kept and tagged for the preview view (Include).
            // (Listings + prev/next nav derive from `self.pages`, so an Include draft
            // naturally appears in them, badged.)
            //
            // **This also decides that `check` and `build --strict` do not lint drafts,
            // and that is deliberate** (ruled 2026-07-28, backlog item 110). Both walk the
            // published set, so a dropped page reaches no validator: measured, `check .`
            // was clean on a project whose `wip.tmd` `check wip.tmd` reported 3 problems
            // in. Keeping it that way is the right call twice over — linting a page that
            // does not ship reports defects the author has not finished creating, and the
            // live preview uses `DraftMode::Include`, so the diagnostics still appear in
            // the one place the author is actually writing. What was wrong was that the
            // omission was *silent*; `check` now names the held-back drafts (`scope_note`
            // in `check.rs`), the way `build` always has. Do not "fix" this by linting
            // drafts here — that reverses the ruling and re-opens the noise it avoids.
            if fm.draft && mode == DraftMode::Exclude {
                excluded.push(rel);
                return None;
            }
            // `image` is relative to the page's own directory; store it
            // site-root-relative so a listing card on another page can link it.
            // An absolute/external URL (og:image social card, CDN-hosted thumb) is
            // left untouched — `join_rel` would otherwise fold its scheme into a
            // broken relative path (`posts/https:/cdn.example.com/card.png`).
            let card_image = fm.image.map(|img| {
                if is_external_or_special(&img) {
                    img
                } else {
                    join_rel(&rel, &img)
                }
            });
            // A page with no front-matter `title:` takes its leading `# H1` (as a book
            // chapter does), so <title>, og:title, listing cards, nav, and search — all of
            // which read `Page.title` — agree instead of falling back to the site name /
            // rel-path. Front matter still wins when present.
            let title = fm.title.or_else(|| chapter_heading(&input).0);
            Some(Page {
                input,
                rel,
                url,
                title,
                date: fm.date,
                description: fm.description,
                authors: fm.authors,
                card_image,
                card_image_alt: fm.image_alt,
                categories: fm.categories,
                listings: fm.listings,
                hero: fm.hero,
                page_layout: fm.page_layout,
                has_bibliography: fm.has_bibliography,
                doi: fm.doi,
                venue: fm.venue,
                links: fm.links,
                draft: fm.draft,
            })
        })
        .collect();
    pages.sort_by(|a, b| a.rel.cmp(&b.rel));
    pages
}

/// Resolve every `{{< embed PATH >}}` across the pages to a deduped [`DeckRef`].
/// The path is written relative to the embedding page, so it's mapped to a
/// site-root-relative path via [`join_rel`]; a target that isn't a file is warned
/// about and skipped (the embed iframe would otherwise 404).
pub(super) fn discover_decks(
    root: &Path,
    pages: &[Page],
    warnings: &mut Vec<String>,
) -> Vec<DeckRef> {
    let mut decks: Vec<DeckRef> = Vec::new();
    for page in pages {
        let Ok(src) = std::fs::read_to_string(&page.input) else {
            continue;
        };
        // Expand `{{< include >}}` first: an `{{< embed >}}` living inside an included
        // partial must be discovered too (else the deck flattens to an article + leaks
        // into search). The embed path stays relative to the embedding page.
        let base = page.input.parent().unwrap_or(root);
        let (src, _origins) = crate::includes::resolve(&src, base);
        for target in crate::render::embed_targets(&src) {
            let rel = join_rel(&page.rel, &target);
            let url = tmd_to_html(&rel);
            if decks.iter().any(|d| d.url == url) {
                continue;
            }
            let input = root.join(&rel);
            if input.is_file() {
                decks.push(DeckRef { input, url });
            } else {
                warnings.push(format!("{}: embedded deck not found: {target}", page.rel));
            }
        }
    }
    decks
}

/// Recursively collect input `.tmd` pages under `dir`, skipping `_`-prefixed
/// directories (`_includes`, `_freeze`, `_site`, …) and dotfiles.
///
/// The walk reads directories directly rather than resolving paths through
/// [`crate::includes::safe_join`], so it applies that function's symlink boundary by
/// hand: a link is followed only while it stays inside the repository.
/// Public so the editor's project walk enumerates pages exactly the way discovery does,
/// symlink boundary included. A second walk would let the sidebar list a page the build does
/// not publish, or miss one it does.
pub fn collect_pages(dir: &Path, out: &mut Vec<PathBuf>) {
    let boundary = crate::includes::repo_boundary(dir);
    let mut walked = HashSet::new();
    // Seed with the root itself, so a link pointing back at it is a repeat, not a
    // second copy of every page beneath it.
    if let Ok(c) = dir.canonicalize() {
        walked.insert(c);
    }
    collect_pages_in(dir, &boundary, &mut walked, out);
}

/// `boundary` is the repository the walk may not leave; `walked` holds the canonical
/// directories already visited.
fn collect_pages_in(
    dir: &Path,
    boundary: &Path,
    walked: &mut HashSet<PathBuf>,
    out: &mut Vec<PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        // Checking the link itself is enough: anything deeper can only leave the
        // repository through a link that this same test already refused.
        if entry.file_type().is_ok_and(|t| t.is_symlink())
            && !p.canonicalize().is_ok_and(|c| c.starts_with(boundary))
        {
            continue;
        }
        if p.is_dir() {
            // A link back up the tree stays inside the repository, so the boundary above
            // permits it and only this cycle guard ends the walk. Without it the recursion
            // ran until the path outgrew `PATH_MAX`, emitting one output page per level.
            if p.canonicalize().is_ok_and(|c| !walked.insert(c)) {
                continue;
            }
            collect_pages_in(&p, boundary, walked, out);
        } else if crate::ext::is_source_path(&p) {
            out.push(p);
        }
    }
}

/// Path of `p` relative to `root`, using `/` separators.
fn rel_str(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}
