//! Page + deck discovery: walk `.tmd` files under the project root (path-ordered) into
//! [`Page`]s, and find loose reveal decks. The filesystem-walking front end of
//! `Site::discover`.

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
            let url = qmd_to_html(&rel);
            let fm = parse_front_matter(&input, &rel, warnings);
            // `draft: true`: dropped from the published set (Exclude) — recorded so the
            // build can report it — or kept and tagged for the preview view (Include).
            // (Listings + prev/next nav derive from `self.pages`, so an Include draft
            // naturally appears in them, badged.)
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
            let url = qmd_to_html(&rel);
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
fn collect_pages(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            collect_pages(&p, out);
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
