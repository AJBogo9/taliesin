//! DET-1 (backlog item 15, AP8 determinism round): an end-to-end **reproducibility**
//! guard. Building the same multi-page site twice, in two independent processes at two
//! different paths, must produce byte-identical output across *every* emitted file — not
//! just the HTML pages.
//!
//! Why this is wider than `parallel_build_determinism.rs`. That suite proves the
//! `--jobs 1` vs `--jobs N` *scheduling* invariant, but it only compares `.html` files.
//! The outputs most exposed to a future ordering regression are the *aggregate* assets a
//! single page never contains: `search-index.js`, the Atom feed (`index.xml`),
//! `sitemap.xml` and `robots.txt`. Each is assembled by
//! walking a collection (pages, xref targets, categories, discovered files) — so an
//! accidental switch from a sorted structure to an unsorted `HashMap`/`HashSet`, or a
//! reliance on raw `read_dir` order, would silently make the build non-reproducible while
//! every `.html`-only test stayed green. AP8 found the build reproducible *by design*; this
//! locks that in so it cannot regress unobserved.
//!
//! Why two separate processes at two different paths. Rust's default `HashMap` reseeds its
//! hasher per process, so two independent `taliesin build` invocations get *different*
//! iteration orders; if any such order leaked into output, the two builds would diverge.
//! Building the two copies under different absolute paths additionally varies the
//! filesystem `read_dir` order and would surface any absolute-path leak into the output
//! (a would-be offline/repro bug). Byte-identical across both ⇒ neither vector leaks.
//!
//! Kernel-free by construction (no code cells), so it is fast and never depends on
//! interpreter-side reproducibility (that is item 15's separate AP8-1 concern).
//!
//! Mutation-checked (per the backlog's "gate the gate" rule): this shape was verified by
//! deleting the deterministic `entries.sort_by` that once fed the cross-page hover index
//! (`Site::build_hover_index`, deleted 2026-08-03 with the hover-preview feature) and
//! confirming `outputs_are_byte_reproducible_across_separate_processes` failed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A private temp dir for this test process, wiped fresh so a prior run can't bias it.
fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-repro-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write a feature-rich, **kernel-free** multi-page website that populates every
/// aggregate output path at once:
/// - a `listing:` index over `posts/` → listing cards + the Atom feed (`index.xml`);
/// - anchored headings + prose on every page → the full-text search index
///   (`search-index.js`);
/// - two content pages defining eight `.theorem`/`.definition` xref targets, cross-page
///   `@thm-…`/`@def-…` references → the xref registry resolved into each page;
/// - a site `url:` → `sitemap.xml` + `robots.txt`.
fn write_repro_site(root: &Path) {
    fs::create_dir_all(root.join("posts")).unwrap();
    fs::write(
        root.join("_site.yml"),
        "title: DET1 Reproducibility Site\n\
         url: \"https://det1.example.com\"\n\
         nav:\n  left:\n    \
         - text: Home\n      href: index.tmd\n    \
         - text: Concepts\n      href: concepts.tmd\n    \
         - text: Theory\n      href: theory.tmd\n",
    )
    .unwrap();

    // The listing index: cards are assembled from every sibling post's front matter.
    fs::write(
        root.join("index.tmd"),
        "---\ntitle: Blog\nlisting:\n  contents: posts\n  sort: \"date desc\"\n  \
         type: grid\n---\n\nWelcome to the reproducibility blog.\n",
    )
    .unwrap();

    // Two content pages, four xref targets each (theorems + definitions), referenced
    // cross-page below so the xref registry resolves all eight into the linking page.
    fs::write(
        root.join("concepts.tmd"),
        "---\ntitle: Concepts\ndescription: \"Core definitions and theorems for reproducible builds.\"\n---\n\n\
         # Concepts {#sec-concepts}\n\n\
         Prose introducing the concepts so the section body is indexed for full-text search.\n\n\
         ::: {.theorem #thm-alpha}\nThe alpha property holds for all reproducible builds.\n:::\n\n\
         ::: {.definition #def-beta}\nA beta object is one whose bytes are seed-independent.\n:::\n\n\
         ## Details {#sec-details}\n\n\
         More prose about details, enough to populate the search body text for this section.\n\n\
         ::: {.theorem #thm-gamma}\nGamma follows from alpha and beta together.\n:::\n\n\
         ::: {.definition #def-delta}\nDelta is the closure of gamma under composition.\n:::\n",
    )
    .unwrap();
    fs::write(
        root.join("theory.tmd"),
        "---\ntitle: Theory\ndescription: \"Theory page referencing the concepts across pages.\"\n---\n\n\
         # Theory {#sec-theory}\n\n\
         By @thm-alpha and @def-beta the build is reproducible; see also @thm-gamma.\n\n\
         ::: {.theorem #thm-epsilon}\nEpsilon bounds the divergence between two builds at zero.\n:::\n\n\
         ::: {.definition #def-zeta}\nZeta is the set of all byte-identical outputs.\n:::\n\n\
         ## Consequences {#sec-consequences}\n\n\
         Prose about consequences referencing @def-delta and @thm-epsilon for cross links.\n\n\
         ::: {.theorem #thm-eta}\nEta generalizes epsilon to N concurrent builders.\n:::\n\n\
         ::: {.definition #def-theta}\nTheta is the fixed point of the reproducibility operator.\n:::\n",
    )
    .unwrap();

    // Sibling posts: ascending dates → the listing (date desc) shows them newest first.
    // Each carries a category shared with the others and a unique one, plus cross-page
    // references so the xref registry is exercised.
    for i in 0..5 {
        let dir = root.join(format!("posts/post-{i}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("index.tmd"),
            format!(
                "---\ntitle: \"Post Number {i}\"\ndate: \"2026-01-{day:02}\"\n\
                 description: \"Summary of post {i}, which references a theorem.\"\n\
                 categories: [cat{i}, shared]\n---\n\n\
                 # Post Number {i}\n\n\
                 Body of post {i}. It relies on @thm-alpha and mentions @def-zeta for good measure.\n",
                day = i + 1,
            ),
        )
        .unwrap();
    }
}

/// Run `taliesin build <root>` (default jobs) and return **every** file written under
/// `<root>/_site`, keyed by its path relative to that output dir, as raw bytes. Every file
/// is compared — HTML, the index `.js`/`.xml` aggregates, the SEO text files, the `og/*.png`
/// cards, and the content-hashed `_assets/*` — because reproducibility means the whole
/// portable folder, not just its pages.
fn build_and_collect_all(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(root)
        .output()
        .expect("run build");
    assert!(
        out.status.success(),
        "build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let site = root.join("_site");
    let mut map = BTreeMap::new();
    collect_files(&site, &site, &mut map);
    assert!(
        !map.is_empty(),
        "no files produced under {}",
        site.display()
    );
    map
}

fn collect_files(dir: &Path, base: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let p = entry.path();
        if p.is_dir() {
            collect_files(&p, base, map);
        } else {
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().to_string();
            map.insert(rel, fs::read(&p).unwrap());
        }
    }
}

/// Assert two output maps are byte-identical, with a diagnosis that names the first
/// offending file (and, for text, the first differing line) so a regression is actionable.
fn assert_byte_identical(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>) {
    let a_keys: Vec<&String> = a.keys().collect();
    let b_keys: Vec<&String> = b.keys().collect();
    assert_eq!(
        a_keys,
        b_keys,
        "the two builds produced a different SET of output files — a discovery/emit path is \
         not reproducible (only-in-A: {:?}, only-in-B: {:?})",
        a.keys().filter(|k| !b.contains_key(*k)).collect::<Vec<_>>(),
        b.keys().filter(|k| !a.contains_key(*k)).collect::<Vec<_>>(),
    );
    for (rel, a_bytes) in a {
        let b_bytes = &b[rel];
        if a_bytes == b_bytes {
            continue;
        }
        // First differing line, when both sides are UTF-8 text, else a byte offset.
        let detail = match (std::str::from_utf8(a_bytes), std::str::from_utf8(b_bytes)) {
            (Ok(sa), Ok(sb)) => {
                let (na, nb) = sa
                    .lines()
                    .zip(sb.lines())
                    .enumerate()
                    .find(|(_, (la, lb))| la != lb)
                    .map(|(n, (la, lb))| {
                        (format!("A:{}: {la}", n + 1), format!("B:{}: {lb}", n + 1))
                    })
                    .unwrap_or_else(|| ("(differ in length)".into(), String::new()));
                format!("\n  {na}\n  {nb}")
            }
            _ => {
                let off = a_bytes
                    .iter()
                    .zip(b_bytes)
                    .position(|(x, y)| x != y)
                    .unwrap_or_else(|| a_bytes.len().min(b_bytes.len()));
                format!(" (binary; first byte differs at offset {off})")
            }
        };
        panic!(
            "output file `{rel}` differs between two separate-process builds \
             ({} vs {} bytes){detail}\n\
             → the build is not byte-reproducible: an emit path leaked HashMap/HashSet \
             iteration order, raw read_dir order, or an absolute path into this file.",
            a_bytes.len(),
            b_bytes.len(),
        );
    }
}

/// The core DET-1 guard: two independent builds of the same site, in separate processes at
/// separate paths, are byte-identical across every emitted file.
#[test]
fn outputs_are_byte_reproducible_across_separate_processes() {
    let base = tmp_dir("core");
    let a_root = base.join("a");
    let b_root = base.join("b");
    write_repro_site(&a_root);
    write_repro_site(&b_root);

    let a = build_and_collect_all(&a_root);
    let b = build_and_collect_all(&b_root);

    assert_byte_identical(&a, &b);

    let _ = fs::remove_dir_all(&base);
}

/// The guard would pass vacuously if the site never populated the aggregate assets it is
/// meant to protect. Assert that the very files most exposed to an ordering regression are
/// present and non-trivially filled, so a future refactor that stops emitting one of them
/// (or empties it) is caught here rather than silently narrowing the guard above.
#[test]
fn the_repro_site_populates_every_guarded_aggregate() {
    let base = tmp_dir("cover");
    let root = base.join("site");
    write_repro_site(&root);
    let out = build_and_collect_all(&root);

    let text = |name: &str| -> String {
        String::from_utf8(
            out.get(name)
                .unwrap_or_else(|| {
                    panic!(
                        "missing expected output `{name}`; had: {:?}",
                        out.keys().collect::<Vec<_>>()
                    )
                })
                .clone(),
        )
        .unwrap_or_else(|_| panic!("`{name}` is not UTF-8"))
    };

    // Full-text search index: one entry per page + per anchored heading.
    let search = text("search-index.js");
    assert!(
        search.contains("window.TALIESIN_SEARCH_INDEX=[") && search.contains("Concepts"),
        "search-index.js is not a populated index"
    );

    // Listing → Atom feed; site url → sitemap + robots. (`llms.txt` and the `og/*.png`
    // social cards were named here too until wave 4 cut both on 2026-08-08. The feed and
    // the sitemap are what is left of the url:-gated `emit` closure, and both are still
    // built by walking a collection, which is the property this test exists for.)
    assert!(
        out.contains_key("index.xml"),
        "no Atom feed emitted — the listing path is not exercised"
    );
    assert!(
        out.contains_key("sitemap.xml") && out.contains_key("robots.txt"),
        "sitemap.xml / robots.txt missing — the SEO aggregate path is not exercised"
    );

    let _ = fs::remove_dir_all(&base);
}
