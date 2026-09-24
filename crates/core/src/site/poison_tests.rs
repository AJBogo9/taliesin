//! One fixture that poisons every author scalar a page can carry, to catch the escaping
//! class rather than one site of it.
//!
//! **Why a fixture and not a type.** Hundreds of sites interpolate author data into
//! markup, and each defect in this class was one of them skipping its escape (a nav href,
//! a div class, a file name in a card link). An `Html` newtype would thread through every
//! emitter to catch what one build of a poisoned project catches: the same project is
//! built twice, once with every free-text value and file name plain and once with each
//! carrying `"<b>'&`, and the two must come out as the same MARKUP, tag for tag and
//! attribute name for attribute name. A missing escape shows up as an extra attribute (a
//! `"` closing its value early), an extra element, or a swallowed stretch of the page (a
//! comment or a script left open).
//!
//! **Why it iterates the key lists.** Every front-matter key in [`KNOWN_KEYS`] and every
//! `_site.yml` key in [`NATIVE_KEYS`] is written by iterating the list: a key is poisoned
//! as a free-text string unless [`fm_value`] or [`site_value`] gives it a typed value,
//! with the reason beside it. A new free-text key is therefore poisoned the day it is
//! added. A new typed key is handed a string until it is listed here, which both builds
//! reject alike, so give it its type to have it exercised.

use super::*;
use crate::frontmatter::KNOWN_KEYS;
use config::NATIVE_KEYS;

/// A YAML single-quoted scalar holding `s` verbatim.
fn yq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The front-matter value a post gets for `key`, with `p` in every free-text slot.
fn fm_value(key: &str, p: &str) -> String {
    match key {
        // A list of people, each with the free-text fields `cite/author.rs` reads.
        "author" => format!(
            "\n  - name: {}\n    affiliation: {}\n    url: {}\n    equal: true\n    \
             contribution: {}",
            yq(&format!("Ann {p}")),
            yq(&format!("Inst {p}")),
            yq(&format!("https://ex.com/a{p}")),
            yq(&format!("Wrote {p}"))
        ),
        // A calendar date must stay one, or the page leaves the listing and the feed; its
        // time half is passed through to the feed and the card, so that half is poisoned.
        "date" => yq(&format!("2026-01-02T09:30{p}")),
        "categories" => format!("[{}, {}]", yq(&format!("cat {p}")), yq("plain")),
        // File names: the fixture writes files under these names.
        "image" => yq(&format!("img{p}.png")),
        "bibliography" => yq(&format!("refs{p}.bib")),
        // Typed: a bool, an enum, a map of bools.
        "draft" => "false".to_string(),
        "title-block-style" => "default".to_string(),
        "toc" => "true".to_string(),
        "execute" => "\n  cache: false".to_string(),
        // Only a listing page carries these; see `listing_page`.
        "listing" | "hero" => String::new(),
        // Everything else is free text, poisoned automatically.
        _ => yq(&format!("{key} {p}")),
    }
}

/// The `_site.yml` value for `key`, with `p` in every free-text slot and file name.
fn site_value(key: &str, p: &str) -> Option<String> {
    Some(match key {
        "url" => yq(&format!("https://ex.com/s{p}")),
        "favicon" => yq(&format!("fav{p}.svg")),
        "logo" => yq(&format!("logo{p}.svg")),
        "bibliography" => yq(&format!("refs{p}.bib")),
        "nav" => format!(
            "\n  left:\n    - text: {}\n      href: {}\n  right:\n    - {{ icon: github, href: {} }}",
            yq(&format!("Nav {p}")),
            yq(&format!("posts{p}/p{p}.tmd")),
            yq(&format!("https://ex.com/g{p}"))
        ),
        // Footer item TEXT is raw HTML by design (the trusted-source model: an inline
        // `<svg>` must work), so it stays plain; its href is poisoned.
        "footer" => format!(
            "\n  right:\n    - text: Feed\n      href: {}",
            yq(&format!("https://ex.com/f{p}"))
        ),
        // `chapters:` turns the project into a book; the book chrome's file-name hrefs are
        // pinned by `book_chrome_escapes_chapter_file_names`.
        "chapters" => return None,
        // An interpreter path, never emitted.
        "python" => return None,
        _ => yq(&format!("{key} {p}")),
    })
}

/// The poisons. A scalar or a file name is never markdown, so its poison carries a whole
/// tag (`<b>`), which an unescaped text site turns into an element. Markdown prose passes
/// raw HTML by design (the trust model), so the prose poison carries no tag; and a file
/// name cannot hold the `/` of `</script`, so the `{js}` cell has its own.
struct Poison<'a> {
    scalar: &'a str,
    prose: &'a str,
    js: &'a str,
}

const CLEAN: Poison = Poison {
    scalar: "Zq",
    prose: "Zq",
    js: "",
};

const DIRTY: Poison = Poison {
    scalar: "Zq\"<b>'&",
    prose: "Zq\"<'&",
    js: "</SCRIPT><!--<script>",
};

/// A website whose every author scalar and file name carries the scalar poison.
fn project(tag: &str, poison: &Poison) -> PathBuf {
    let (p, prose) = (poison.scalar, poison.prose);
    let site_yml: String = NATIVE_KEYS
        .iter()
        .filter_map(|k| site_value(k, p).map(|v| format!("{k}: {v}\n")))
        .collect();
    let post_fm: String = KNOWN_KEYS
        .iter()
        .map(|k| (k, fm_value(k, p)))
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| format!("{k}: {v}\n"))
        .collect();
    let title = format!("\"T {}\"", p.replace('"', "\\\""));
    let js = format!("{prose}{}", poison.js).replace('"', "\\\"");
    let post = format!(
        "---\n{post_fm}---\n\n## Heading {prose} {{#sec-one}}\n\nText [@key] and \
         [home](../index.tmd) and [up](#sec-one).\n\n::: {{.callout-note title={title}}}\n\
         Body.\n:::\n\n::: {{.x{p}}}\nDiv.\n:::\n\n![Alt {prose}](img{p}.png)\n\n\
         ```{{js}}\nconst s = \"{js}\";\n```\n\n## Two\n\nx\n\n## Three\n\ny\n",
    );
    let listing = format!(
        "---\ntitle: {}\nlisting:\n  contents: {}\n  id: {}\nhero:\n  eyebrow: {}\n  \
         headline: {}\n  lead: {}\n  actions:\n    - text: {}\n      href: {}\n      \
         primary: true\n---\n\nSee @sec-one.\n",
        yq(&format!("Home {p}")),
        yq(&format!("posts{p}")),
        yq(&format!("list{p}")),
        yq(&format!("Eye {p}")),
        yq(&format!("Head {p}")),
        yq(&format!("Lead {p}")),
        yq(&format!("Go {p}")),
        yq(&format!("posts{p}/p{p}.tmd")),
    );
    let bib =
        "@article{key, author = {A. Person}, title = {A title}, journal = {J}, year = {2020}}\n";
    tests::write_site(
        tag,
        &[
            ("_site.yml", site_yml.as_str()),
            ("index.tmd", "---\ntitle: Home\n---\n\nHome.\n"),
            (&format!("list{p}.tmd"), listing.as_str()),
            (&format!("posts{p}/p{p}.tmd"), post.as_str()),
            (&format!("posts{p}/img{p}.png"), "png"),
            (&format!("posts{p}/refs{p}.bib"), bib),
            (&format!("refs{p}.bib"), bib),
            (&format!("fav{p}.svg"), "<svg/>"),
            (&format!("logo{p}.svg"), "<svg/>"),
        ],
    )
}

/// Every output of a project, each as its markup shape: the tag names in document order,
/// each with its attribute names, read through the one walker.
fn shapes(root: &Path) -> Vec<(String, Vec<String>)> {
    let site = Site::discover(root);
    let mut out: Vec<(String, String)> = site
        .pages
        .iter()
        .map(|p| (p.url.clone(), site.render_page(&p.rel).expect("renders")))
        .collect();
    out.push(("404".into(), site.render_404_page()));
    out.extend(
        site.atom_feeds()
            .into_iter()
            .map(|(_, xml)| ("feed".into(), xml)),
    );
    out.extend(site.sitemap().map(|x| ("sitemap".into(), x)));
    out.into_iter()
        .map(|(name, html)| {
            let shape = crate::render::tags(&html)
                .map(|t| {
                    let names: Vec<&str> = crate::render::attrs(&t).map(|a| a.name).collect();
                    format!("{} {}", t.name, names.join(" "))
                })
                .collect();
            (name, shape)
        })
        .collect()
}

/// The fixture reaches every key: a front-matter key is set on the post (or, for
/// `listing:`/`hero:`, on the listing page), and every `_site.yml` key is either set or
/// excluded above with its reason.
#[test]
fn the_poison_fixture_covers_every_known_key() {
    for k in KNOWN_KEYS {
        assert!(
            !fm_value(k, "p").is_empty() || matches!(*k, "listing" | "hero"),
            "front-matter key `{k}` is not in the poison fixture"
        );
    }
    let covered: Vec<&str> = NATIVE_KEYS
        .iter()
        .copied()
        .filter(|k| site_value(k, "p").is_some())
        .collect();
    assert_eq!(
        NATIVE_KEYS.len() - covered.len(),
        2,
        "only `chapters` and `python` sit out: {covered:?}"
    );
}

/// The project built with every scalar poisoned is the same markup as the clean build.
#[test]
fn a_poisoned_project_renders_the_same_markup_as_a_clean_one() {
    let clean_root = project("poison-clean", &CLEAN);
    let poison_root = project("poison-dirty", &DIRTY);
    let clean = shapes(&clean_root);
    let poison = shapes(&poison_root);
    let names: Vec<&str> = poison.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names.len(),
        6,
        "three pages, the 404, the listing's feed and the sitemap: {names:?}"
    );
    assert_eq!(clean.len(), poison.len(), "the same outputs");
    for ((name, a), (_, b)) in clean.iter().zip(&poison) {
        if a != b {
            let at = a
                .iter()
                .zip(b)
                .position(|(x, y)| x != y)
                .unwrap_or(a.len().min(b.len()));
            panic!(
                "{name}: the poisoned build's markup differs at tag {at}:\n  clean:    {:?}\n  poisoned: {:?}",
                &a[at.saturating_sub(1)..(at + 2).min(a.len())],
                &b[at.saturating_sub(1)..(at + 2).min(b.len())],
            );
        }
    }
    let _ = std::fs::remove_dir_all(&clean_root);
    let _ = std::fs::remove_dir_all(&poison_root);
}
