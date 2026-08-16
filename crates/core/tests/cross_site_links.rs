//! The four published sites link to each other by ABSOLUTE URL, and nothing else checks them.
//!
//! Until 2026-08-16 this repo's sites were composed into one tree by a script, and that
//! script's whole point was that it *resolved* every cross-project link against the composed
//! output rather than trusting it. The `mounts:` key before it existed for the same reason:
//! a deploy whose primary call-to-action 404'd, from a script nobody ran (item 149).
//!
//! Separate deploys removed the composition, and with it that guarantee. A link like
//! `https://guide.taliesin.sh/using/getting-started.html` is now outbound as far as every
//! validator in the tool is concerned: `site/`'s own link checker cannot see the Guide, and
//! `publish.sh` only resolves the gallery's own exhibits. Rename that page, or change a
//! project's `url:`, and the marketing site's "Get started" button 404s with every gate green
//! — the exact failure, in the exact place, that this project has already shipped once.
//!
//! So derive BOTH sides from the tree and compare them:
//!
//! 1. Every `taliesin.sh` origin a shipped document links to must be some project's declared
//!    `url:`. This catches a typo'd or renamed subdomain.
//! 2. Every path under such a link must exist as a page in the project that declares that
//!    origin. This is the item-149 guard, restored at full strength.
//!
//! Deliberately NOT a network check: it resolves against the source tree, so it is offline,
//! deterministic and true before the first deploy exists.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every shipped document in the tree: `.tmd` pages and the `_site.yml` files whose chrome
/// ships on every page of a project. `notes/` is working material, `target/` and `_freeze/`
/// are generated, and every dot-directory is either not ours (`.git/`) or a tool's scratch
/// space — `editor/vscode/.vscode-test/user-data/User/History/` holds VS Code's own
/// timestamped copies of `.tmd` buffers, six of which carry a link this gate would otherwise
/// judge as a shipped one.
fn shipped_documents() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if p.is_dir() {
                if name.starts_with('.')
                    || matches!(name, "target" | "_freeze" | "notes" | "node_modules")
                {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "tmd") || name == "_site.yml" {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo_root(), &mut out);
    assert!(!out.is_empty(), "found no shipped documents");
    out
}

/// `origin -> project directory`, read from each `_site.yml`'s own `url:`. Derived, so a new
/// site joins this gate by declaring its `url:` and nothing else.
fn declared_origins() -> BTreeMap<String, PathBuf> {
    let mut out = BTreeMap::new();
    for doc in shipped_documents() {
        if doc.file_name().is_some_and(|n| n == "_site.yml") {
            let Ok(text) = std::fs::read_to_string(&doc) else {
                continue;
            };
            for line in text.lines() {
                let line = line.trim();
                let Some(rest) = line.strip_prefix("url:") else {
                    continue;
                };
                let url = rest.trim().trim_matches(['"', '\'']).trim_end_matches('/');
                if url.contains("taliesin.sh") {
                    let dir = doc.parent().expect("_site.yml has a parent").to_path_buf();
                    out.insert(url.to_string(), dir);
                }
                break;
            }
        }
    }
    out
}

/// Every `https://…taliesin.sh…` occurrence in `text`, as `(origin, path)`. The path keeps
/// its leading slash and drops any fragment.
fn taliesin_links(text: &str) -> Vec<(String, String)> {
    const MARK: &str = "https://";
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(MARK) {
        rest = &rest[i..];
        let end = rest
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '"' | '\'' | ')' | '<' | '>' | '`' | ',' | '}')
            })
            .unwrap_or(rest.len());
        let (url, tail) = rest.split_at(end);
        rest = tail;
        // `taliesin.sh` as a HOST, not as a substring of some other domain.
        let after_scheme = &url[MARK.len()..];
        let host_end = after_scheme.find('/').unwrap_or(after_scheme.len());
        let (host, path) = after_scheme.split_at(host_end);
        if host != "taliesin.sh" && !host.ends_with(".taliesin.sh") {
            continue;
        }
        let path = path.split('#').next().unwrap_or("");
        out.push((format!("{MARK}{host}"), path.to_string()));
    }
    out
}

/// The source file a published path came from: `/using/x.html` -> `using/x.tmd`, a directory
/// or empty path -> that directory's `index.tmd`. Returns the raw relative path for anything
/// that is not a page (an asset), which is checked as a file on disk instead.
fn source_for(path: &str) -> String {
    let rel = path.trim_start_matches('/');
    if rel.is_empty() || rel.ends_with('/') {
        return format!("{rel}index.tmd");
    }
    match rel.strip_suffix(".html") {
        Some(stem) => format!("{stem}.tmd"),
        None => rel.to_string(),
    }
}

#[test]
fn every_cross_site_link_resolves_to_a_page_of_the_project_that_declares_its_origin() {
    let origins = declared_origins();
    // Anti-vacuity, measured at 4 on 2026-08-16 (site, gallery, docs/guide, docs/internals).
    // A refactor that stops finding `_site.yml` files must fail here, not pass silently.
    assert!(
        origins.len() >= 4,
        "expected at least the four published sites to declare a taliesin.sh `url:`, \
         found {origins:?}"
    );

    let mut checked = 0usize;
    let mut problems: Vec<String> = Vec::new();

    for doc in shipped_documents() {
        let Ok(text) = std::fs::read_to_string(&doc) else {
            continue;
        };
        let rel = doc
            .strip_prefix(repo_root())
            .unwrap_or(&doc)
            .display()
            .to_string();
        for (origin, path) in taliesin_links(&text) {
            checked += 1;
            let Some(project) = origins.get(&origin) else {
                problems.push(format!(
                    "{rel} links to `{origin}`, which no `_site.yml` in this repo declares \
                     as its `url:` (declared: {:?})",
                    origins.keys().collect::<Vec<_>>()
                ));
                continue;
            };
            // A project's own `url:` line is the declaration, not a link into itself.
            if path.is_empty() && doc.file_name().is_some_and(|n| n == "_site.yml") {
                continue;
            }
            let target = project.join(source_for(&path));
            if !target.is_file() {
                problems.push(format!(
                    "{rel} links to `{origin}{path}`, and `{}` is not a page of the project \
                     that publishes that origin — the link 404s in the deploy",
                    target
                        .strip_prefix(repo_root())
                        .unwrap_or(&target)
                        .display()
                ));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "{} cross-site link problem(s):\n  {}",
        problems.len(),
        problems.join("\n  ")
    );
    // Anti-vacuity: measured at 19 on 2026-08-16. A scan that finds nothing would satisfy
    // every assertion above it.
    assert!(
        checked >= 15,
        "the scan collected {checked} cross-site links, expected >= 15 — it is not looking \
         where the links are"
    );
}
