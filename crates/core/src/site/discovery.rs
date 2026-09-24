//! Page discovery: walk `.tmd` files under the project root (path-ordered) into
//! [`Page`]s. The filesystem-walking front end of
//! `Site::discover`.

use std::collections::HashSet;

use super::*;

/// A website's pages: every `.tmd` under `root` (path-ordered), each mapped to a
/// [`Page`] from its front matter.
pub(super) fn website_pages(
    root: &Path,
    mode: DraftMode,
    warnings: &mut Vec<Warning>,
    excluded: &mut Vec<String>,
) -> Vec<Page> {
    let mut inputs = Vec::new();
    collect_pages(root, &mut inputs);
    inputs.sort();
    nested_projects(root, &inputs, warnings);
    let mut pages: Vec<Page> = inputs
        .into_iter()
        .filter_map(|input| {
            let page = website_page(root, input, warnings);
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
            if page.draft && mode == DraftMode::Exclude {
                excluded.push(page.rel);
                return None;
            }
            Some(page)
        })
        .collect();
    pages.sort_by(|a, b| a.rel.cmp(&b.rel));
    pages
}

/// Report every folder under `root` holding its own `_site.yml` that `inputs` (the pages the
/// walk found) sit in. A project never contains another (nested projects were cut): the walk
/// publishes such a folder's pages as this project's own, under its chrome, and nothing
/// reads the inner `_site.yml`. That was silent, so an author who dropped a project inside
/// another got its pages under the wrong navbar and its config ignored (audit 2026-09-24,
/// config-seam #18). Each is reported once, located at the ignored file.
fn nested_projects(root: &Path, inputs: &[PathBuf], warnings: &mut Vec<Warning>) {
    let mut seen = HashSet::new();
    for input in inputs {
        let mut dir = input.parent();
        while let Some(d) = dir.filter(|d| *d != root && d.starts_with(root)) {
            if seen.insert(d.to_path_buf()) && d.join("_site.yml").is_file() {
                let rel = rel_str(root, &d.join("_site.yml"));
                let folder = rel_str(root, d);
                let mut w = Warning::new(format!(
                    "`{rel}` is ignored: a project cannot contain another, so the pages under \
                     `{folder}/` are built as this project's own pages, with its navigation \
                     (build `{folder}` on its own to publish it as a project)"
                ));
                w.file = Some(rel);
                warnings.push(w);
            }
            dir = d.parent();
        }
    }
}

/// The website [`Page`] for one `input` under `root`, from its front matter: what
/// [`website_pages`] makes of each file it walks, and what a single-document discovery
/// makes of the one file it was handed, without walking anything else.
pub(super) fn website_page(root: &Path, input: PathBuf, warnings: &mut Vec<Warning>) -> Page {
    let rel = rel_str(root, &input);
    let url = tmd_to_html(&rel);
    let fm = parse_front_matter(&input, &rel, warnings);
    let card_image = card_image(&rel, fm.image);
    // A page with no front-matter `title:` takes its leading `# H1` (as a book
    // chapter does), so <title>, og:title, listing cards, nav, and search — all of
    // which read `Page.title` — agree instead of falling back to the site name /
    // rel-path. Front matter still wins when present.
    let title = fm.title.or_else(|| chapter_heading(&input));
    Page {
        input,
        rel,
        url,
        title,
        date: fm.date,
        description: fm.description,
        card_image,
        card_image_alt: fm.image_alt,
        categories: fm.categories,
        listings: fm.listings,
        hero: fm.hero,
        draft: fm.draft,
    }
}

/// A digest of everything discovery reads of one page's source: its front-matter block and
/// its leading `# H1`, the text that heading shows and whether it is `.unnumbered`. The H1
/// names a book chapter before its `title:` does (`book::push_chapter`), numbers or skips
/// it, and titles a website page that has no `title:` ([`website_page`]).
///
/// Two sources with one digest discover to the same page, which is what lets the preview
/// re-discover a project only when a save moves this. A digest of the front-matter block
/// alone missed an edit of the heading: a chapter retitled or made `.unnumbered` in place
/// left the drawer, the pager and every later chapter's numbers stale (audit 2026-09-24 C2).
pub fn discovery_digest(src: &str) -> u64 {
    let src = crate::includes::normalize_line_endings(src);
    let mut read = crate::frontmatter::front_matter_block(&src)
        .unwrap_or("")
        .to_string();
    // A separator no front matter can end with, so no block and heading pair collides
    // with another split of the same text.
    read.push('\0');
    if let Some((text, unnumbered)) = crate::render::leading_h1(&src) {
        read.push_str(&text);
        read.push(if unnumbered { '\u{1}' } else { '\u{2}' });
    }
    crate::hash::fnv1a(&read)
}

/// The [`discovery_digest`] of the source at each of `paths`, read as discovery reads it (an
/// unreadable file digests as an empty one), across cores and in order. The preview records
/// every page's before it answers its first request, and each digest parses its page to
/// find the H1: one page after another, that was 4 ms of `docs/guide`'s time to ready
/// (audit 2026-09-24, WP20).
pub fn discovery_digests(paths: &[PathBuf]) -> Vec<u64> {
    super::fanout::map_ordered(paths, |path| {
        discovery_digest(&crate::includes::read_source(path).unwrap_or_default())
    })
}

/// A page's front-matter `image:`, stored site-root-relative so a listing card on another
/// page and the `og:image` can link it (it is written relative to the page's own
/// directory). An absolute/external URL (og:image social card, CDN-hosted thumb) is left
/// untouched: `join_rel` would otherwise fold its scheme into a broken relative path
/// (`posts/https:/cdn.example.com/card.png`). Shared by website pages and book chapters,
/// which read the same front matter.
pub(super) fn card_image(rel: &str, image: Option<String>) -> Option<String> {
    image.map(|img| {
        if is_external_or_special(&img) {
            img
        } else {
            join_rel(rel, &img)
        }
    })
}

/// Recursively collect input `.tmd` pages under `dir`, skipping `_`-prefixed
/// directories (`_includes`, `_freeze`, `_site`, …) and dotfiles.
///
/// The walk reads directories directly rather than resolving paths through
/// `includes::safe_join_in`, so a symlink is held to the one publication rule,
/// [`crate::includes::publishable`] (as the build's asset mirror holds every entry): it is
/// followed only while its real path stays inside the repository and adds no `.`/`_`
/// component to the path it shares with the project. Testing the link's own NAME let an
/// ordinary `vendor -> ../.private` publish every page under it (audit 2026-09-24, WP1
/// residual).
/// Public so the editor's project walk enumerates pages exactly the way discovery does,
/// symlink rule included. A second walk would let the sidebar list a page the build does
/// not publish, or miss one it does.
pub fn collect_pages(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut walked = HashSet::new();
    // Seed with the root itself, so a link pointing back at it is a repeat, not a
    // second copy of every page beneath it.
    if let Ok(c) = dir.canonicalize() {
        walked.insert(c);
    }
    collect_pages_in(dir, dir, &mut walked, out);
}

/// `root` is the project the walk publishes from; `walked` holds the canonical directories
/// already visited.
fn collect_pages_in(
    root: &Path,
    dir: &Path,
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
        // repository, or reach a private path, through a link this same test refused.
        if entry.file_type().is_ok_and(|t| t.is_symlink()) {
            let rel = p.strip_prefix(root).unwrap_or(&p);
            let reach = crate::includes::Reach::Wholesale;
            if crate::includes::publishable(root, root, rel, reach).is_err() {
                continue;
            }
        }
        if p.is_dir() {
            // A link back up the tree stays inside the repository, so the rule above
            // permits it and only this cycle guard ends the walk. Without it the recursion
            // ran until the path outgrew `PATH_MAX`, emitting one output page per level.
            if p.canonicalize().is_ok_and(|c| !walked.insert(c)) {
                continue;
            }
            collect_pages_in(root, &p, walked, out);
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

#[cfg(test)]
mod tests {
    use super::discovery_digest;

    /// The digest moves with everything discovery reads of a page and with nothing else, so
    /// the preview neither misses a retitled chapter nor re-discovers on a body edit.
    #[test]
    fn the_discovery_digest_moves_with_what_discovery_reads_and_nothing_else() {
        let base = discovery_digest("---\ntitle: A\n---\n\n# Intro\n\nBody.\n");
        let same = |src: &str| discovery_digest(src) == base;
        assert!(
            same("---\ntitle: A\n---\n\n# Intro\n\nAnother body.\n"),
            "a body edit"
        );
        assert!(
            !same("---\ntitle: B\n---\n\n# Intro\n\nBody.\n"),
            "the front matter"
        );
        assert!(
            !same("---\ntitle: A\n---\n\n# Introduction\n\nBody.\n"),
            "the heading's text"
        );
        assert!(
            !same("---\ntitle: A\n---\n\n# Intro {.unnumbered}\n\nBody.\n"),
            "the heading's `.unnumbered`"
        );
        assert!(
            !same("---\ntitle: A\n---\n\nBody.\n"),
            "the heading removed"
        );
        // What the heading SHOWS is what names the chapter, so markup around the same
        // text changes nothing discovery keeps.
        assert!(same("---\ntitle: A\n---\n\n# *Intro*\n\nBody.\n"));
    }

    /// Each source is read as discovery reads it, and the digests come back in the order
    /// asked, which is what the preview pairs them with its pages by: an unreadable file
    /// digests as an empty one, which is what discovery makes of it.
    #[test]
    fn the_digests_read_each_source_as_discovery_does_in_order() {
        let dir = std::env::temp_dir().join(format!("tali-digests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (one, two) = ("---\ntitle: A\n---\n\n# One\n", "# Two\n");
        std::fs::write(dir.join("one.tmd"), one).unwrap();
        std::fs::write(dir.join("two.tmd"), two).unwrap();
        let paths = ["two.tmd", "missing.tmd", "one.tmd"].map(|f| dir.join(f));
        assert_eq!(
            super::discovery_digests(&paths),
            [two, "", one].map(discovery_digest)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
