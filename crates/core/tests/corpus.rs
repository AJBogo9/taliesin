//! Corpus-wide invariants: every real document must render and satisfy the
//! load-bearing guarantees (a block id + valid sourcepos on every block, ids
//! unique, blocks in document order). The corpus is the spec, so this runs the
//! whole pipeline over each real `.tmd` rather than synthetic snippets.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

mod common;
use common::corpus_dir;

fn collect_tmd(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "_extensions" || name == "expected" {
                continue; // not source documents
            }
            collect_tmd(&p, out);
        } else if taliesin_core::ext::is_source_path(&p) {
            out.push(p);
        }
    }
}

/// Parse "L:C-L:C" into (start_line, end_line).
fn line_range(sourcepos: &str) -> (usize, usize) {
    let (start, end) = sourcepos.split_once('-').expect("sourcepos has a dash");
    let sl = start
        .split(':')
        .next()
        .unwrap()
        .parse()
        .expect("start line");
    let el = end.split(':').next().unwrap().parse().expect("end line");
    (sl, el)
}

#[test]
fn every_corpus_doc_has_clean_front_matter() {
    // taliesin's front-matter validator must not warn on any real document: a warning
    // here means the allowlist is missing a key the corpus legitimately uses.
    // corpus/diagnostics/ is exempt (it deliberately holds typo'd keys).
    let mut files = Vec::new();
    collect_tmd(&corpus_dir(), &mut files);
    let mut offenders = Vec::new();
    for f in &files {
        if f.components().any(|c| c.as_os_str() == "diagnostics") {
            continue;
        }
        let src = fs::read_to_string(f).unwrap();
        for w in taliesin_core::frontmatter::validate_front_matter(&src) {
            let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
            offenders.push(format!("{label}: {}", w.message));
        }
    }
    assert!(
        offenders.is_empty(),
        "front-matter validator warned on corpus docs:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn every_corpus_doc_emits_no_unknown_key_warnings() {
    // taliesin has its own closed vocabulary: every real corpus doc must use only
    // recognized cell options, callout kinds, and config keys, so the validators stay
    // silent. corpus/diagnostics/ is exempt (its exact warnings are pinned in
    // crates/core/tests/nested_validation.rs).
    let mut files = Vec::new();
    collect_tmd(&corpus_dir(), &mut files);
    let mut offenders = Vec::new();
    for f in &files {
        if f.components().any(|c| c.as_os_str() == "diagnostics") {
            continue;
        }
        let src = fs::read_to_string(f).unwrap();
        let base = f.parent().unwrap();
        let doc = taliesin_core::render_document_with_includes(&src, base);
        for w in doc
            .warnings
            .iter()
            .filter(|w| w.message.starts_with("unknown "))
        {
            let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
            offenders.push(format!("{label}: {}", w.message));
        }
    }
    assert!(
        offenders.is_empty(),
        "validator warned on corpus docs (clean the doc or extend the vocabulary):\n{}",
        offenders.join("\n")
    );
}

/// Attributes that make the browser FETCH something, per element. Everything absent from
/// this table may legitimately carry an absolute URL and must not be scanned:
/// `<a href>` is an author's outbound link, `<link rel=canonical href>` is metadata rather
/// than a request, `<meta content>` holds `og:url`, JSON-LD's *body* is full of URLs, and
/// an inline `<svg xmlns="http://www.w3.org/2000/svg">` is an XML namespace that is not a
/// network address at all. Scanning "every http in the page" flags all five and is why
/// this has to be per-element rather than a substring sweep.
const FETCHING: &[(&str, &[&str])] = &[
    ("script", &["src"]),
    ("img", &["src", "srcset"]),
    ("image", &["href", "xlink:href"]), // SVG <image>
    ("iframe", &["src"]),
    ("frame", &["src"]),
    ("embed", &["src"]),
    ("object", &["data"]),
    ("video", &["src", "poster"]),
    ("audio", &["src"]),
    ("source", &["src", "srcset"]),
    ("track", &["src"]),
    ("input", &["src"]),
];

/// The `rel` values on a `<link>` that cause a fetch. `canonical`, `alternate` and
/// `author` deliberately do not: they name a resource without requesting it.
const FETCHING_LINK_RELS: &[&str] = &[
    "stylesheet",
    "preload",
    "modulepreload",
    "prefetch",
    "preconnect",
    "dns-prefetch",
    "icon",
    "shortcut icon",
    "apple-touch-icon",
    "mask-icon",
    "manifest",
];

/// Is `url` a reference the browser would resolve OFF this origin?
fn is_offsite(url: &str) -> bool {
    let u = url.trim();
    // Protocol-relative is the sneaky one: it looks like a path and is a network fetch.
    u.starts_with("//")
        || u.starts_with("http://")
        || u.starts_with("https://")
        // A scheme-ful reference to anything that is not the page itself. `data:` and
        // `blob:` are inline payloads (offline by construction) and are allowed.
        || matches!(u.split_once(':'), Some((s, _))
            if s.len() > 1
                && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
                && !matches!(s, "data" | "blob"))
}

/// Read the value of `attr` out of an already-isolated tag's text.
fn tag_attr(tag: &str, attr: &str) -> Option<String> {
    let mut from = 0;
    while let Some(rel) = tag[from..].find(attr) {
        let at = from + rel;
        // A real attribute boundary: preceded by whitespace, followed by `=`.
        let before_ok = at > 0 && tag.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = tag[at + attr.len()..].trim_start();
        if before_ok && let Some(v) = rest.strip_prefix('=') {
            let v = v.trim_start();
            let quote = v.chars().next()?;
            if quote == '"' || quote == '\'' {
                let body = &v[1..];
                return body.find(quote).map(|e| body[..e].to_string());
            }
        }
        from = at + attr.len();
    }
    None
}

/// Every off-origin subresource reference in a built page, as `(element, url)`.
fn offsite_refs(page: &str) -> Vec<(String, String)> {
    let mut hits = Vec::new();
    let mut i = 0;
    while let Some(rel) = page[i..].find('<') {
        let open = i + rel;
        let Some(close_rel) = page[open..].find('>') else {
            break;
        };
        let tag = &page[open..open + close_rel + 1];
        i = open + close_rel + 1;
        let name: String = tag[1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        let attrs: Vec<&str> = if name == "link" {
            let rel_val = tag_attr(tag, "rel")
                .unwrap_or_default()
                .to_ascii_lowercase();
            if FETCHING_LINK_RELS.contains(&rel_val.trim()) {
                vec!["href", "imagesrcset"]
            } else {
                vec![]
            }
        } else {
            match FETCHING.iter().find(|(n, _)| *n == name) {
                Some((_, a)) => a.to_vec(),
                None => vec![],
            }
        };
        for a in attrs {
            if let Some(v) = tag_attr(tag, a) {
                // `srcset` is a comma-separated candidate list, each `url [descriptor]`.
                for cand in v.split(',') {
                    let url = cand.split_whitespace().next().unwrap_or("");
                    if !url.is_empty() && is_offsite(url) {
                        hits.push((name.clone(), url.to_string()));
                    }
                }
            }
        }
    }
    // CSS fetches too, and an inlined stylesheet is where a font or a background would
    // hide. `@import` and `url(…)` are the two shapes that reach the network.
    for (marker, skip) in [("url(", 4), ("@import", 7)] {
        let mut from = 0;
        while let Some(rel) = page[from..].find(marker) {
            let at = from + rel + skip;
            from = at;
            let rest = page[at..].trim_start();
            // `@import url("x")` and `@import "x"` are both legal. The first is already
            // counted by the `url(` pass above, so skip it here rather than double-count
            // it; only the bare-string form is this pass's to find.
            if marker == "@import" && rest.starts_with("url(") {
                continue;
            }
            let url: String = rest
                .trim_start_matches(['"', '\''])
                .chars()
                .take_while(|c| !matches!(c, '"' | '\'' | ')' | ';' | ' '))
                .collect();
            if is_offsite(&url) {
                hits.push(("css".to_string(), url));
            }
        }
    }
    hits
}

#[test]
fn no_built_page_fetches_anything_off_origin() {
    // Item 86. The offline guarantee — "the binary is self-contained; a built page needs
    // no network" — was asserted on exactly two surfaces: `--bare` (which ships zero
    // <script> at all, so it cannot witness the claim for a normal page) and the reveal.js
    // case in `render/tests.rs`. Nothing pinned it on the page shape a reader actually
    // gets, which is every page this tool builds.
    //
    // This is NOT about a live CDN fetch: `render/mod.rs`'s jsdelivr string is a
    // never-reached fallback (OFF-2, fixed 2026-07-22). It is the coverage residual — the
    // property was true and untested, which is how it would have regressed silently.
    //
    // WHAT THIS DOES NOT COVER, stated because an overclaimed gate is worse than none.
    // It reads STATIC subresource references only: markup attributes and CSS `url(` /
    // `@import`. A URL that inlined JavaScript fetches at RUNTIME is invisible to it —
    // and that is exactly where `MERMAID_DEFAULT` lives, substituted into `MERMAID_JS`
    // and reached by a dynamic import. Scanning script bodies would therefore fail on a
    // string the project deliberately keeps as an unreachable fallback, so the boundary
    // is drawn here on purpose rather than by oversight. Verified by mutation in both
    // directions: reinstating the OFF-2 fallback does NOT fail this test (the blind spot
    // is real), while pointing one emitted `<link rel=icon>` at a CDN does.
    let mut files = Vec::new();
    collect_tmd(&corpus_dir(), &mut files);
    files.sort();
    assert!(files.len() >= 5, "expected the corpus docs");

    let mut checked = 0usize;
    let mut with_subresources = 0usize;
    for f in &files {
        let label = f
            .strip_prefix(corpus_dir())
            .unwrap_or(f)
            .display()
            .to_string();
        let src = fs::read_to_string(f).unwrap();
        let doc = taliesin_core::render_document_with_includes(&src, f.parent().unwrap());
        let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("page");
        // BOTH shipping modes. `Build` is what gets published; `Preview` ships every
        // enhancer unconditionally (it cannot content-gate against a live edit), so it is
        // the larger surface and the one where a new bundled asset would first appear.
        for mode in [
            taliesin_core::OutputMode::Build,
            taliesin_core::OutputMode::Preview,
        ] {
            let page = taliesin_core::render_doc_to_page(&doc, stem, mode);
            let hits = offsite_refs(&page);
            assert!(
                hits.is_empty(),
                "{label} ({mode:?}): a built page must fetch nothing off-origin, found {hits:?}"
            );
            // The scanner has to be looking at something. A page with zero fetching
            // elements would pass with the assertion disabled, so count the pages that do
            // carry subresources and require below that the corpus produced some.
            if page.contains("<script") || page.contains("<img") {
                with_subresources += 1;
            }
        }
        checked += 1;
    }
    assert!(checked >= 5, "walked {checked} docs");
    assert!(
        with_subresources >= 5,
        "only {with_subresources} of {checked} built pages carried any subresource at all — \
         the scanner would be passing vacuously"
    );

    // The positive control, and the reason this test can be trusted: the scanner must
    // actually FIRE on the shapes it claims to catch, and must stay silent on the four
    // absolute-URL shapes a correct page legitimately contains.
    let caught = offsite_refs(
        "<script src=\"https://cdn.jsdelivr.net/x.js\"></script>\
         <link rel=\"stylesheet\" href=\"//fonts.example/f.css\">\
         <img srcset=\"local.png 1x, https://cdn.example/2x.png 2x\">\
         <style>@import url(\"https://evil.example/a.css\");</style>\
         <style>@import \"https://evil.example/b.css\";</style>\
         <style>body{background:url('https://cdn.example/bg.png')}</style>",
    );
    assert_eq!(
        caught.len(),
        6,
        "the scanner must catch every off-origin shape, and each exactly once: {caught:?}"
    );
    let allowed = offsite_refs(
        "<a href=\"https://example.com/post\">an outbound link</a>\
         <link rel=\"canonical\" href=\"https://taliesin.sh/p.html\">\
         <meta property=\"og:url\" content=\"https://taliesin.sh/p.html\">\
         <script type=\"application/ld+json\">{\"url\":\"https://taliesin.sh\"}</script>\
         <svg xmlns=\"http://www.w3.org/2000/svg\"><image href=\"inline.png\"/></svg>\
         <img src=\"data:image/png;base64,iVBOR\">",
    );
    assert!(
        allowed.is_empty(),
        "an outbound link, a canonical URL, og:url, JSON-LD and an SVG namespace are not \
         fetches: {allowed:?}"
    );
}

#[test]
fn every_corpus_doc_renders_with_invariants() {
    let mut files = Vec::new();
    collect_tmd(&corpus_dir(), &mut files);
    files.sort();
    assert!(
        files.len() >= 5,
        "expected the corpus docs, found {}",
        files.len()
    );

    for f in &files {
        let label = f
            .strip_prefix(corpus_dir())
            .unwrap_or(f)
            .display()
            .to_string();
        let src = fs::read_to_string(f).unwrap();
        let base = f.parent().unwrap();
        let doc = taliesin_core::render_document_with_includes(&src, base);

        assert!(!doc.blocks.is_empty(), "{label}: produced no blocks");

        let mut ids = HashSet::new();
        // Document order holds within one contiguous RUN of blocks from a single source
        // file. Included files reset to their own line numbering, so the run is the unit.
        //
        // It used to be tracked per file for the whole document, which is the same thing
        // as long as every include is a whole file: those splice in one unbroken run, so
        // a file's blocks can only ever be seen in ascending order. **Block-level
        // transclusion (item 160) breaks that**, legitimately — a document may pull
        // `#sec-b` before `#sec-a`, and `corpus/transclude.tmd` does exactly that, so the
        // second run starts at an earlier line than the first ended.
        //
        // Nothing downstream needs the stronger version: `highlightAtLine` in
        // `web-client/client.js` scans every `[data-sourcepos]` and takes the smallest
        // covering range (falling back to the latest-starting preceding one), which is a
        // min/max over the whole set and assumes no ordering at all. What the check is
        // still worth keeping for is what `render/tests.rs` names: a gathered block
        // claiming a span it did not come from would show up as a line going backwards
        // *inside* a run.
        //
        // Residual, stated rather than hidden: two transclusions of the SAME file with no
        // block between them, later section first, would read as one run and trip this.
        // No corpus document does that today. If one ever does, the fix is here — the
        // renderer would need to mark run boundaries — not in the document.
        let mut prev_start: std::collections::HashMap<Option<String>, usize> = HashMap::new();
        let mut prev_file: Option<Option<String>> = None;
        for b in &doc.blocks {
            assert!(!b.html.is_empty(), "{label}: empty html for block {}", b.id);
            assert!(ids.insert(&b.id), "{label}: duplicate block id {}", b.id);

            // `data-source-file` is relative to the primary document's directory, on
            // every machine. An absolute label ships the author's home directory into
            // published HTML and makes the build machine-dependent.
            if let Some(sf) = b.source_file.as_deref() {
                assert!(
                    !Path::new(sf).is_absolute(),
                    "{label}: absolute source_file {sf:?}"
                );
            }

            // Generated blocks (e.g. the References section) carry no sourcepos.
            if b.sourcepos.is_empty() {
                continue;
            }
            let (sl, el) = line_range(&b.sourcepos);
            assert!(
                sl >= 1,
                "{label}: zero/invalid start line in {}",
                b.sourcepos
            );
            assert!(sl <= el, "{label}: start line after end in {}", b.sourcepos);
            // A change of source file ends the run, so the next one starts fresh.
            if prev_file.as_ref() != Some(&b.source_file) {
                prev_start.insert(b.source_file.clone(), 0);
                prev_file = Some(b.source_file.clone());
            }
            let prev = prev_start.entry(b.source_file.clone()).or_insert(0);
            assert!(
                sl >= *prev,
                "{label}: blocks out of order within one run of {:?} ({sl} after {prev})",
                b.source_file
            );
            *prev = sl;
        }
    }
}

#[test]
fn includes_are_resolved_with_origin_files() {
    // pca-geometry pulls in _includes/three-scene.tmd via {{< include >}}.
    //
    // The tech-blog copy, not the loose `corpus/posts/` one, and deliberately: the loose
    // copy's nearest project marker is the repository's own `.git`, so this assertion used
    // to pass only inside a checkout and fail in any export, vendored copy or `docker COPY`
    // without VCS metadata. That is not a hypothetical — it is what kept the unmutated
    // cargo-mutants baseline red (its scratch copy carries no `.git`), which blocked the
    // mutation re-run outright. The tech-blog page sits under a real `_site.yml`, so what
    // bounds it is the project the author declared rather than a fact about the checkout.
    let dir = corpus_dir().join("tech-blog/posts/pca-geometry");
    let src = fs::read_to_string(dir.join("index.tmd")).unwrap();
    // The entry point the commands use, not the library-only one. Rendering via
    // `render_document_with_includes` here asserted something true of the library and
    // false of the product: `build <this page>` dropped the include and warned while this
    // test stayed green (PP-3).
    let doc = taliesin_core::render_single_doc(&src, &dir);

    let body = doc.body_html();
    assert!(
        !body.contains("{{< include"),
        "include shortcode leaked into output"
    );

    // some blocks must now originate from the included file, with their own lines
    let from_include: Vec<_> = doc
        .blocks
        .iter()
        .filter(|b| {
            b.source_file
                .as_deref()
                .is_some_and(|f| f.contains("three-scene"))
        })
        .collect();
    assert!(
        !from_include.is_empty(),
        "expected blocks sourced from the included three-scene.tmd"
    );

    // Every include label is relative to the primary document's directory. An absolute
    // label would ship the author's home directory into published HTML, make two machines
    // produce different bytes, and break the click-to-source round trip (the companion
    // resolves the label against the doc's dir, and generates the reverse-sync key the
    // same way). `three-scene.tmd` is reached through `../../`, the case that regressed.
    let labels: Vec<&str> = from_include
        .iter()
        .filter_map(|b| b.source_file.as_deref())
        .collect();
    assert!(
        labels
            .iter()
            .all(|f| *f == "../../_includes/three-scene.tmd"),
        "include label must be primary-doc-relative, got {labels:?}"
    );

    // the single-page report pulls in subsections; every subsection contributes blocks
    let book = corpus_dir().join("single-page-report");
    let bsrc = fs::read_to_string(book.join("index.tmd")).unwrap();
    let bdoc = taliesin_core::render_document_with_includes(&bsrc, &book);
    assert!(!bdoc.body_html().contains("{{< include"));
    let included_files: HashSet<_> = bdoc
        .blocks
        .iter()
        .filter_map(|b| b.source_file.clone())
        .collect();
    assert!(
        included_files.len() >= 5,
        "expected blocks from several subsection files, got {included_files:?}"
    );
}

/// EVERY corpus document must resolve its includes when built **on its own**, which is
/// what `taliesin build <file.tmd>` does and what a reader copying one example does.
///
/// The test above deliberately renders the tech-blog copy, because that one sits under a
/// `_site.yml`. That left the loose `corpus/posts/pca-geometry/` copy — same bytes, no
/// project marker above it — with no coverage at all, and it rotted: a standalone build
/// shipped the literal `{{< include ../../_includes/three-scene.tmd >}}` as text plus
/// three "couldn't load" boxes where the 3D figures belong, because a single invoked
/// document is confined to its own directory (PT-2, see `include_root_parity.rs`) so
/// `../../` escapes. Sweeping every doc through the single-doc entry point is what makes
/// that unmissable for the next one.
#[test]
fn every_corpus_doc_resolves_its_includes_when_built_alone() {
    /// Drop `<code>`/`<pre>` subtrees, so a document that *shows* the include syntax
    /// as an example is not mistaken for one that failed to expand it.
    fn strip_code(html: &str) -> String {
        let mut out = String::with_capacity(html.len());
        let mut rest = html;
        while let Some(open) = rest.find("<code").or_else(|| rest.find("<pre")) {
            out.push_str(&rest[..open]);
            let close_tag = if rest[open..].starts_with("<code") {
                "</code>"
            } else {
                "</pre>"
            };
            match rest[open..].find(close_tag) {
                Some(rel) => rest = &rest[open + rel + close_tag.len()..],
                None => return out,
            }
        }
        out.push_str(rest);
        out
    }

    let mut files = Vec::new();
    collect_tmd(&corpus_dir(), &mut files);
    let mut leaked = Vec::new();
    let mut checked = 0;
    for f in &files {
        let Ok(src) = fs::read_to_string(f) else {
            continue;
        };
        if !src.contains("{{< include") {
            continue;
        }
        // Partials (`_includes/…`) are pulled INTO a page, never built on their own —
        // the site walker skips any `_`-prefixed segment and so does this.
        if f.components().any(|c| {
            c.as_os_str()
                .to_str()
                .is_some_and(|s| s.starts_with('_') && s != "_")
        }) {
            continue;
        }
        let dir = f.parent().unwrap();
        // The product entry point for one invoked file, not the library-only one: an
        // include assertion true of the library and false of the command is exactly the
        // vacuous shape this file has been bitten by before.
        let doc = taliesin_core::render_single_doc(&src, dir);
        checked += 1;
        // The leaked directive is ESCAPED in the body (`{{&lt; include …`), so a needle
        // for the raw `{{< include` matches nothing and passes on a broken page — this
        // assertion was written that way first and stayed green with the bug in place.
        // Code has to come out first: `transclude.tmd` *documents* the syntax in code
        // spans, and those are literal content, not an unresolved directive.
        let body = strip_code(&doc.body_html());
        let leaked_text = body.contains("{{&lt; include") || body.contains("{{< include");
        let warned = doc
            .warnings
            .iter()
            .any(|w| w.message.contains("include not resolved"));
        if leaked_text || warned {
            leaked.push(
                f.strip_prefix(corpus_dir())
                    .unwrap_or(f)
                    .display()
                    .to_string(),
            );
        }
    }
    // Guard against the sweep silently matching nothing (a renamed shortcode, a moved
    // corpus): this assertion is only worth anything if it actually rendered documents.
    assert!(
        checked >= 3,
        "expected several corpus docs to use `{{{{< include >}}}}`, walked {checked}"
    );
    assert!(
        leaked.is_empty(),
        "these corpus docs ship an unresolved `{{{{< include >}}}}` when built on their \
         own (a reader copying one gets literal shortcode text): {leaked:?}"
    );
}

#[test]
fn a11y_chrome_emits_landmarks_and_a_skip_link() {
    // A page with a TOC emits the skip-link + focusable <main> SERVER-SIDE (works
    // with JS off) and a distinguishable TOC landmark. ---
    let page = taliesin_core::render_doc_to_page(
        &taliesin_core::render_document(
            "---\ntitle: \"T\"\ntoc: true\n---\n\n# One\n\nbody\n\n## Two\n\nmore\n",
        ),
        "fallback",
        taliesin_core::OutputMode::Build,
    );
    // Skip-to-content link is the first thing in the body, before JS runs.
    assert!(
        page.contains("class=\"tali-skip\"") && page.contains("href=\"#tali-main\""),
        "server-side skip-to-content link missing"
    );
    // The content container is a focusable <main id="tali-main">.
    assert!(
        page.contains("<main id=\"tali-main\" tabindex=\"-1\">"),
        "server-side focusable <main id=tali-main> missing"
    );
    // The TOC is a distinguishable landmark (named + role) for screen-reader landmark nav.
    assert!(
        page.contains("role=\"doc-toc\"") && page.contains("aria-label=\"Table of contents\""),
        "TOC landmark must carry role + an aria-label"
    );
    // AP7-5: and it is REACHABLE from the keyboard without walking the whole chapter. The
    // TOC is a sticky sidebar visible the whole time, but it is emitted after the reading
    // column, so on a real chapter it was tab stop 58 of 72 — behind every heading anchor
    // and every code copy button. The landmark rotor already covered screen-reader users;
    // this is for keyboard-only users not running AT, for whom the skip link is the only
    // mechanism. `tabindex="-1"` so the link lands focus IN the nav (as `<main>` does)
    // rather than merely near it, without adding a tab stop.
    assert!(
        page.contains("href=\"#TOC\"") && page.contains("Skip to table of contents"),
        "a page with a TOC must offer a skip link to it"
    );
    assert!(
        page.contains("<nav id=\"TOC\" tabindex=\"-1\""),
        "the TOC landmark must be programmatically focusable for that link to land in it"
    );

    // ...and a page WITHOUT a TOC offers only the one link: a skip link to a target that
    // does not exist is worse than no skip link, since it spends a tab stop going nowhere.
    let no_toc = taliesin_core::render_doc_to_page(
        &taliesin_core::render_document("---\ntitle: \"T\"\n---\n\nbody only\n"),
        "fallback",
        taliesin_core::OutputMode::Build,
    );
    assert!(
        no_toc.contains("href=\"#tali-main\""),
        "the skip-to-content link is unconditional"
    );
    assert!(
        !no_toc.contains("href=\"#TOC\""),
        "no TOC on the page means no skip link to one"
    );
}

#[test]
fn website_renders_with_toc_anchored_headings_and_numbered_figures() {
    // single-page-report is a single-page website (no `chapters:`), assembled from
    // `subsections/` includes — not a book; the assertions below exercise TOC,
    // heading anchors, and document-order figure numbering on that one page.
    let dir = corpus_dir().join("single-page-report");
    let src = fs::read_to_string(dir.join("index.tmd")).unwrap();
    let page = taliesin_core::render_html_page_with_includes(&src, &dir, "report");

    // toc: true -> a TOC nav + the sidebar layout, with anchor-linked entries.
    assert!(
        page.contains("id=\"TOC\""),
        "book should render a table of contents"
    );
    assert!(page.contains("class=\"has-toc\""), "missing toc layout");
    assert!(
        page.contains("<a href=\"#introduction\">Introduction</a>"),
        "TOC entry missing"
    );
    // Headings carry matching anchor ids. This titled page has a `<h1 class="title">`
    // title block, so heading demotion (#11) renders its body `# Introduction` as <h2>
    // (one <h1> per page); the anchor id is text-derived and unchanged.
    assert!(
        page.contains("<h2 id=\"introduction\""),
        "heading anchor missing"
    );

    // The three labelled image figures render as numbered <figure>s, attrs not
    // leaked. They are Figures 4–6: three earlier `#| fig-cap:` code cells take
    // numbers 1-3 (counted in document order, even though those
    // R cells aren't executed here, so their output isn't shown).
    assert!(
        !page.contains("{#fig-"),
        "figure attribute block leaked into output"
    );
    assert!(
        page.contains("id=\"fig-model-hierarchical\""),
        "figure id missing"
    );
    for n in 4..=6 {
        assert!(
            page.contains(&format!("Figure&nbsp;{n}:")),
            "missing 'Figure {n}:' caption"
        );
    }
    // The labelled image figure resolves to its number via the registry.
    assert!(page.contains("id=\"fig-model-hierarchical\""));
}

#[test]
fn reverse_sync_sourcepos_is_total() {
    // Reverse cursor-sync (`highlightAtLine` in web-client/client.js) scans every
    // `[data-sourcepos]` element and matches the strict regex `^(\d+):\d+-(\d+):\d+$`;
    // a non-matching sourcepos is silently skipped (the block becomes cursor-invisible).
    // So EVERY non-empty `data-sourcepos` in the emitted HTML must match that exact
    // format. A GATHERED block (References, the footnotes section) is exempt at block
    // level: it collects content from many scattered lines, so it has no one honest
    // range and carries an empty sourcepos. Its locatable units are nested instead —
    // a footnote `<li>` carries its own definition's `data-sourcepos`, and is scanned
    // by the loop below like any other element.
    let re = |s: &str| -> bool {
        // crude ^(\d+):\d+-(\d+):\d+$ check without the regex crate
        let (a, b) = match s.split_once('-') {
            Some(x) => x,
            None => return false,
        };
        let ok = |p: &str| {
            let mut it = p.split(':');
            let (l, c) = (it.next(), it.next());
            it.next().is_none()
                && l.is_some_and(|x| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit()))
                && c.is_some_and(|x| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit()))
        };
        ok(a) && ok(b)
    };
    let mut files = Vec::new();
    collect_tmd(&corpus_dir(), &mut files);
    let mut offenders = Vec::new();
    for f in &files {
        let src = fs::read_to_string(f).unwrap();
        let base = f.parent().unwrap();
        let doc = taliesin_core::render_document_with_includes(&src, base);
        // Scan EVERY data-sourcepos="..." in the emitted HTML (what highlightAtLine sees),
        // not just top-level blocks — nested elements inside containers carry their own.
        let html = doc.body_html();
        let mut rest = html.as_str();
        while let Some(i) = rest.find("data-sourcepos=\"") {
            rest = &rest[i + "data-sourcepos=\"".len()..];
            let end = rest.find('"').unwrap_or(rest.len());
            let sp = &rest[..end];
            rest = &rest[end..];
            if !sp.is_empty() && !re(sp) {
                let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
                offenders.push(format!("{label}: sourcepos={sp:?}"));
            }
        }
    }
    offenders.sort();
    offenders.dedup();
    assert!(
        offenders.is_empty(),
        "{} block(s) have a sourcepos that reverse cursor-sync cannot match \
         (must be `L:C-L:C`); fix at the attr-injection seam:\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

#[test]
fn footnote_sidenotes_are_locatable() {
    // A note renders beside its own reference, INSIDE the referencing block (owner
    // ruling 2026-08-01: margin placement is the default). So the note's own attributes
    // are the only thing making it click-to-source-able, and the block-level checks
    // above cannot see it — they inspect the referencing block's leading tag, which
    // carries the *paragraph's* sourcepos. Pin the nested unit directly.
    //
    // `data-block-id` is load-bearing and not decorative here: client.js `locatable()`
    // matches `closest("[data-tali-src], [data-block-id]")`, so without it a Ctrl-click
    // on a note walks up to the enclosing paragraph and lands on the paragraph's first
    // line instead of the line the note was written on — silently the wrong line, which
    // is worse than a no-op because it reads as if it worked.
    let mut files = Vec::new();
    collect_tmd(&corpus_dir(), &mut files);
    let mut seen = 0;
    let mut offenders = Vec::new();
    for f in &files {
        let src = fs::read_to_string(f).unwrap();
        let doc = taliesin_core::render_document_with_includes(&src, f.parent().unwrap());
        let html = doc.body_html();
        for (i, _) in html.match_indices("<span class=\"tali-sidenote\"") {
            let tag = &html[i..i + html[i..].find('>').unwrap_or(0)];
            seen += 1;
            if !tag.contains("data-block-id=\"") || !tag.contains("data-sourcepos=\"") {
                let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
                offenders.push(format!("{label}: {tag}>"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "footnote sidenote missing data-block-id/data-sourcepos (Ctrl-click would land \
         on the enclosing paragraph's line):\n{}",
        offenders.join("\n")
    );
    // Guard against the assert above passing vacuously if footnotes leave the corpus.
    assert!(seen > 0, "no footnote sidenote in the corpus to check");
}

#[test]
fn gathered_sections_stay_unlocatable() {
    // The other half of the contract `footnote_lis_are_locatable` pins, and the reason
    // that test can trust its `<li>`s. client.js `locatable()` resolves a Ctrl-click to
    // `closest("[data-tali-src], [data-block-id]")` but SKIPS a block whose sourcepos is
    // not usable (`^[1-9]\d*:\d+`), walking on to a usable ancestor or resolving to
    // nothing at all. That guard is the only thing standing between a gathered section
    // and `openSource()`'s line-1 default, which is silently the wrong line rather than
    // a no-op.
    //
    // So these sections must keep claiming NOTHING. A well-meaning "every block should
    // have a sourcepos" change that stamped `1:1-1:1` on them would satisfy the guard,
    // sail past the block-level checks above, and quietly restore the line-1 landing on
    // every reference and on the footnote section's own chrome. Pin the emptiness.
    //
    // They must also carry no `data-tali-src`: that attribute is the OTHER way to be
    // locatable (an explicit file, for site chrome), and neither section has one.
    let mut files = Vec::new();
    collect_tmd(&corpus_dir(), &mut files);
    let mut seen = 0;
    let mut offenders = Vec::new();
    for f in &files {
        let src = fs::read_to_string(f).unwrap();
        let doc = taliesin_core::render_document_with_includes(&src, f.parent().unwrap());
        let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
        // The block model's own view: the generated sections carry an empty sourcepos.
        for b in &doc.blocks {
            if b.id != "tali-references" && b.id != "tali-footnotes" {
                continue;
            }
            seen += 1;
            if !b.sourcepos.is_empty() {
                offenders.push(format!(
                    "{label}: block {} claims sourcepos {:?}; a gathered section has no \
                     honest range, and a non-empty one makes client.js jump to it",
                    b.id, b.sourcepos
                ));
            }
        }
        // The emitted HTML, which is what the client actually reads.
        let html = doc.body_html();
        for id in ["tali-references", "tali-footnotes"] {
            let needle = format!("data-block-id=\"{id}\"");
            let Some(i) = html.find(&needle) else {
                continue;
            };
            let start = html[..i].rfind('<').unwrap_or(0);
            let tag = &html[start..start + html[start..].find('>').unwrap_or(0)];
            if tag.contains("data-sourcepos=\"") || tag.contains("data-tali-src=") {
                offenders.push(format!("{label}: {tag}>"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a gathered section became locatable again; Ctrl-clicking a reference or the \
         footnote section's <hr> would land on line 1:\n{}",
        offenders.join("\n")
    );
    // Guard against a vacuous pass if the corpus loses its citations + footnotes.
    assert!(seen > 0, "no gathered section in the corpus to check");
}

#[test]
fn ids_and_sourcepos_present_on_visible_blocks() {
    // Every visible block element should carry both data attributes. (Raw HTML
    // comment blocks legitimately carry neither — they are emitted verbatim.)
    let src = fs::read_to_string(corpus_dir().join("posts/em-algorithm/index.tmd")).unwrap();
    let doc = taliesin_core::render_document(&src);
    for b in &doc.blocks {
        // Raw HTML comments are emitted verbatim; generated blocks (References)
        // have no sourcepos. Both legitimately lack the data attributes.
        if b.html.starts_with("<!--") || b.sourcepos.is_empty() {
            continue;
        }
        assert!(
            b.html.contains("data-block-id=") && b.html.contains("data-sourcepos="),
            "block missing data attributes: {}",
            &b.html[..b.html.len().min(80)]
        );
    }
}

#[test]
fn tech_blog_site_discovers_renders_chrome_and_rewrites_links() {
    use taliesin_core::Site;
    let root = corpus_dir().join("tech-blog");
    let site = Site::discover(&root);

    // The project config parses (navbar items) and every `.tmd` page is found,
    // each mapped to a `.html` output url.
    assert!(
        site.pages.len() >= 10,
        "expected the tech-blog pages, found {}",
        site.pages.len()
    );
    assert!(
        !site.config.nav.left.is_empty(),
        "navbar items should parse from _site.yml"
    );
    for p in &site.pages {
        assert!(p.url.ends_with(".html"), "page url not .html: {}", p.url);
    }

    // A top-level page renders with the site chrome and rewrites its nav links.
    let blog = site.render_page("blog.tmd").expect("blog renders");
    assert!(blog.contains("tali-site-nav"), "navbar missing");
    assert!(
        blog.contains("<nav class=\"tali-nav-inner\" aria-label=\"Primary\">"),
        "the website primary nav must be aria-labelled"
    );
    assert!(blog.contains("tali-site-footer"), "footer missing");
    // Mobile nav toggle must stay a real, keyboard/SR-operable <button> (WCAG 2.1.1):
    // never regress to the old display:none `<input type=checkbox>` + role-less label hack.
    assert!(
        blog.contains("<button")
            && blog.contains("class=\"tali-nav-burger\"")
            && blog.contains("aria-expanded")
            && blog.contains("aria-controls=\"tali-nav-links\""),
        "mobile nav toggle must be an aria <button.tali-nav-burger> controlling #tali-nav-links"
    );
    assert!(
        blog.contains("id=\"tali-nav-links\""),
        "nav menu must carry the controlled id"
    );
    // The exact old-hack signature (a checkbox input as the toggle) must be gone. (Bare
    // `type="checkbox"` would false-match the unrelated `input[type="checkbox"]` CSS selector.)
    assert!(
        !blog.contains("<input type=\"checkbox\" id=\"tali-nav-toggle\""),
        "mobile nav must not use the inaccessible checkbox-hack toggle"
    );
    assert!(
        blog.contains("href=\"blog.html\""),
        "nav link not rewritten"
    );
    assert!(
        !blog.contains("href=\"blog.tmd\""),
        "raw .tmd nav link leaked"
    );
    // The blog sets `url:`, so the build now generates an Atom feed (`blog.xml`) and the
    // footer's local `.xml` item is honored (feed.rs + chrome.rs). There is still no
    // legacy `feed.xml` path and no RSS-specific discovery <link> (Taliesin emits Atom).
    assert!(!blog.contains("feed.xml"), "no legacy feed.xml path");
    assert!(
        blog.contains("href=\"blog.xml\""),
        "footer feed link honored now that a feed is generated"
    );
    assert!(
        !blog.contains("application/rss+xml"),
        "no RSS discovery <link> (Taliesin emits Atom)"
    );

    // A post now carries a single "back to listing" link to the listing that owns it
    // (the Blog page; the home page's recent-posts preview is `max-items`-capped, so it
    // does not count as an owner), and cross-page `.tmd` links are still rewritten to
    // `.html`. See `tech_blog::post_pages_link_back_to_their_listing` for the full rule.
    let post = site
        .render_page("posts/evidence-lower-bound/index.tmd")
        .expect("post renders");
    assert!(
        post.contains("<nav class=\"tali-postnav tali-listing-backnav\"")
            && post.contains("href=\"../../blog.html\"")
            && post.contains("</span> Blog</a>"),
        "post should link back to the Blog listing"
    );
    assert!(
        post.contains("../KL-divergence/index.html"),
        "cross-page .tmd link not rewritten to .html"
    );
    assert!(
        !post.contains("../KL-divergence/index.tmd"),
        "raw cross-page .tmd link leaked"
    );

    // OpenGraph meta for sharing, reduced on 2026-08-08 to the five tags an unfurl needs:
    // og:title, og:description, an absolute og:url (the site has a `url:`), og:image from
    // the page's own `image:`, and the twitter card kind. Plus `meta name="description"`,
    // which is what a search result reads. `og:type`, `og:site_name`, `twitter:title`,
    // `twitter:description`, `twitter:image` and `rel="canonical"` went with the rest.
    assert!(post.contains("property=\"og:title\""), "og:title missing");
    assert!(
        post.contains("property=\"og:description\""),
        "og:description missing"
    );
    assert!(
        post.contains("property=\"og:url\" content=\"https://"),
        "absolute og:url missing"
    );
    assert!(
        post.contains("<meta name=\"description\""),
        "meta description missing"
    );
    assert!(
        post.contains("name=\"twitter:card\""),
        "twitter card missing"
    );

    // Reading-time estimate: a post's title block carries a subtle "N min read", rendered
    // server-side so it lives in the static HTML (no JS, SEO-visible).
    assert!(
        post.contains("class=\"tali-read-time\"") && post.contains(" min read"),
        "post should show a reading-time estimate"
    );

    // No per-tag archive pages, and a post no longer carries a category strip
    // linking to them.
    let fourier = site
        .render_page("posts/fourier-transform/index.tmd")
        .expect("fourier post renders");
    assert!(
        !fourier.contains("tali-post-cats") && !fourier.contains("categories/mathematics/"),
        "post should not carry a category archive strip"
    );
}

#[test]
fn standalone_doc_carries_opengraph_seo_meta() {
    // A single .tmd (no site) gets text OpenGraph/SEO meta from its own front matter.
    let doc = taliesin_core::render_document(
        "---\ntitle: \"T\"\ndescription: \"D\"\n---\n\n# Hi\n\nbody\n",
    );
    let page =
        taliesin_core::render_doc_to_page(&doc, "fallback", taliesin_core::OutputMode::Build);
    assert!(
        page.contains("property=\"og:title\" content=\"T\""),
        "og:title"
    );
    assert!(
        page.contains("property=\"og:description\" content=\"D\""),
        "og:description"
    );
    assert!(
        page.contains("name=\"description\" content=\"D\""),
        "meta description"
    );
    // PL20: an undated standalone page is a generic `website`, not an `article` (the article
    // gate is a front-matter `date:`, the same gate the reading-time estimate uses).
    assert!(
        page.contains("property=\"og:type\" content=\"website\""),
        "an undated standalone doc is og:type=website, not article"
    );
    assert!(page.contains("name=\"twitter:card\""), "twitter card");
    // A dated doc (a post) IS an article.
    let dated =
        taliesin_core::render_document("---\ntitle: \"T\"\ndate: \"2026-01-01\"\n---\n\nbody\n");
    let dated_page =
        taliesin_core::render_doc_to_page(&dated, "fallback", taliesin_core::OutputMode::Build);
    assert!(
        dated_page.contains("property=\"og:type\" content=\"article\""),
        "a dated standalone doc is og:type=article"
    );

    // A doc with no description omits the description tags but still has og:title.
    let bare = taliesin_core::render_doc_to_page(
        &taliesin_core::render_document("---\ntitle: \"Only\"\n---\n\n# x\n"),
        "fb",
        taliesin_core::OutputMode::Build,
    );
    assert!(bare.contains("property=\"og:title\" content=\"Only\""));
    assert!(
        !bare.contains("name=\"description\""),
        "no description tag when absent"
    );
}

#[test]
fn bare_build_is_script_free_css_themed_and_drops_js() {
    // The `--bare` build target: zero <script>, zero CDN, CSS-only theming — yet
    // server-rendered math still works and a {js} cell is dropped (not shipped dead).
    let src = fs::read_to_string(corpus_dir().join("bare-draft.tmd")).unwrap();
    let doc = taliesin_core::render_document_with_includes(&src, &corpus_dir());
    let bare = taliesin_core::render_doc_to_page(&doc, "bare", taliesin_core::OutputMode::Bare);

    // The contract: not one <script> tag (no theme bootstrap, no enhancers, no {js}
    // runtime, no TOC/search) and no CDN host.
    assert!(
        !bare.contains("<script"),
        "bare output ships zero <script> tags"
    );
    assert!(!bare.contains("cdn.jsdelivr"), "no jsDelivr CDN reference");
    assert!(!bare.contains("unpkg.com"), "no unpkg CDN reference");

    // Server-rendered math survives a script-free page.
    assert!(
        bare.contains("class=\"katex"),
        "KaTeX math renders without JS"
    );

    // The {js} cell's runtime `<script type="application/tali-js">` is stripped.
    assert!(
        !bare.contains("application/tali-js"),
        "bare drops the {{js}} cell script block"
    );

    // Click-to-source must survive the {js}-script strip: the cell wrapper keeps its
    // block id + sourcepos (the strip removes only the inner <script>, and
    // `emit_js_cell` puts the block attrs on the outer <div>). This pins the
    // load-bearing block-model invariants on the bare-assembled page specifically.
    let cell_at = bare
        .find("class=\"cell tali-js-cell\"")
        .expect("the {js} cell wrapper survives the strip");
    let tag_open = bare[..cell_at].rfind('<').expect("wrapper open tag");
    let wrapper_tag = &bare[tag_open..cell_at];
    assert!(
        wrapper_tag.contains("data-block-id=\"b-"),
        "bare {{js}} cell wrapper keeps its data-block-id: {wrapper_tag}"
    );
    assert!(
        wrapper_tag.contains("data-sourcepos=\""),
        "bare {{js}} cell wrapper keeps its data-sourcepos: {wrapper_tag}"
    );

    // Theming is CSS-only: an unforced (auto) theme follows the OS via a media query
    // that carries the dark layer rewritten from `[data-theme="dark"]` onto `:root`.
    assert!(
        bare.contains("@media (prefers-color-scheme: dark)"),
        "bare auto-theme uses a prefers-color-scheme media query"
    );
    assert!(
        bare.contains(":root .tali-hl-"),
        "the dark layer is rewritten from [data-theme] onto :root"
    );

    // Contrast: a normal (non-bare) build of the same doc DOES ship the enhancer
    // bundle and the {js} cell, proving `--bare` is what strips them.
    let build = taliesin_core::render_doc_to_page(&doc, "build", taliesin_core::OutputMode::Build);
    assert!(
        build.contains("<script"),
        "a normal build still ships scripts"
    );
    assert!(
        build.contains("application/tali-js"),
        "a normal build keeps the {{js}} cell"
    );
}

#[test]
fn site_auto_gates_on_this_page_toc_by_heading_count() {
    use taliesin_core::Site;
    // tech-blog sets a site-wide `toc: true`. The "on this page" TOC is auto-gated by
    // heading count (NN/g: only long, chunkable pages earn it), so a substantial post
    // keeps the sidebar TOC while a short article reads as one column — with no per-page
    // `toc:` toggling.
    let site = Site::discover(&corpus_dir().join("tech-blog"));

    // A post with 4 section headings (Theory / Key properties / Code demo / Summary; the
    // `#`-prefixed lines inside the {python} cell are code comments, not headings) -> the
    // TOC nav + the has-toc two-column layout (`.tali-site-main has-toc` on a site page).
    let post = site
        .render_page("posts/KL-divergence/index.tmd")
        .expect("KL-divergence post renders");
    // `id="TOC"` is the unambiguous signal: the rendered TOC <nav>. (`has-toc` is unusable
    // here — the bundled CSS ships `.has-toc` selectors, so it is always present.)
    assert!(
        post.contains("id=\"TOC\""),
        "a long, chunkable post should keep the auto-gated sidebar TOC"
    );

    // A 2-heading project article (below MIN_TOC_HEADINGS, no hero/listing, no explicit
    // `toc:`) -> a single reading column, no near-empty TOC, despite the site enabling TOCs.
    let short = site
        .render_page("projects/iphone-premium-analysis/index.tmd")
        .expect("project article renders");
    assert!(
        !short.contains("id=\"TOC\""),
        "a short article must not get a near-empty auto-gated TOC"
    );
}

#[test]
fn book_discovers_chapters_with_parts_numbering_and_chrome() {
    use taliesin_core::Site;
    let root = corpus_dir().join("demo-book");
    let site = Site::discover(&root);

    // Detected as a book; the chapter pages come from `book: chapters:` in order.
    assert!(site.is_book(), "demo-book should be a book project");
    assert_eq!(site.output_dir(), "_book", "book builds to _book");
    let book = site.book.as_ref().expect("book nav resolved");
    assert_eq!(book.title.as_deref(), Some("A Short Demo Book"));

    // The sidebar order: Preface (unnumbered), Introduction (1), the "Core" part
    // header, Methodology (2), Results (3), Wrap-up (4). "Methodology" and "Wrap-up"
    // come from per-chapter `{ file:, text: }` label overrides in `_site.yml`
    // (Methods/Summary are the chapters' own H1s, which the override replaces).
    let chapters: Vec<(&str, Option<u32>)> = book
        .entries
        .iter()
        .filter(|e| e.part.is_none())
        .map(|e| (e.title.as_str(), e.number))
        .collect();
    assert_eq!(
        chapters,
        vec![
            ("Preface", None),
            ("Introduction", Some(1)),
            ("Methodology", Some(2)),
            ("Results", Some(3)),
            ("Wrap-up", Some(4)),
        ],
        "chapter order + numbering with per-chapter label overrides (preface unnumbered)"
    );
    assert!(
        book.entries
            .iter()
            .any(|e| e.part.as_deref() == Some("Core")),
        "the `Core` part header should be in the sidebar"
    );

    // A chapter renders with the book chrome: the chapter-list nav (active chapter),
    // section numbers on its headings, and prev/next-chapter navigation.
    let methods = site.render_page("methods.tmd").expect("methods renders");
    assert!(
        methods.contains("<nav class=\"tali-book-sidebar\""),
        "book chapter-list nav missing"
    );
    // The book is the single-column relayout: a sticky topbar with a "Chapters" drawer
    // launcher + an off-canvas drawer holding the list — NOT the old three-pane `.tali-book`
    // flex wrapper (rail | content | rail). A regression to that wrapper must fail here.
    assert!(
        methods.contains("class=\"tali-book-topbar\"")
            && methods.contains("id=\"tali-book-drawer-btn\"")
            && methods.contains("id=\"tali-book-drawer\""),
        "book topbar + chapter drawer chrome missing"
    );
    // The drawer launcher promises `aria-haspopup="dialog"`, so the drawer panel must
    // actually BE a dialog (role + accessible name); the focus trap / aria-modal are
    // wired at runtime (BOOK_DRAWER_SCRIPT + taliFocusTrap). Batch 3g.
    //
    // `aria-modal` is deliberately NOT static here, re-confirmed by browser measurement in
    // the 2026-07-26 mobile batch: `taliFocusTrap` SETS it on open and REMOVES it on
    // release, which is the correct lifecycle (a hidden dialog is not modal). A static copy
    // was tried and reverted — the trap's release stripped it, so the attribute was present
    // on load and gone after the first close, which is worse than either end state.
    assert!(
        methods.contains("aria-haspopup=\"dialog\"")
            && methods.contains(
                "class=\"tali-book-drawer-panel\" role=\"dialog\" aria-label=\"Chapters\""
            ),
        "the chapter drawer must be a real role=dialog to honour aria-haspopup=dialog"
    );
    assert!(
        !methods.contains("class=\"tali-book\""),
        "the removed three-pane `.tali-book` flex wrapper must not return"
    );
    // Every structural nav landmark carries a distinguishing accessible name
    // (a screen reader can tell the chapter list from the pager).
    assert!(
        methods.contains(
            "class=\"tali-book-sidebar\" data-tali-src=\"_site.yml\" aria-label=\"Chapters\""
        ) && methods.contains("class=\"tali-postnav tali-book-postnav\" aria-label=\"Pagination\""),
        "book nav landmarks must be aria-labelled"
    );
    assert!(
        methods.contains("tali-book-chapter tali-book-active"),
        "active chapter not marked"
    );
    assert!(
        methods.contains("tali-section-number\">2</span>")
            && methods.contains("tali-section-number\">2.1</span>"),
        "chapter/section numbering missing"
    );
    assert!(
        methods.contains("tali-book-postnav")
            && methods.contains("3  Results")
            && methods.contains("1  Introduction"),
        "prev/next-chapter navigation missing"
    );
    // A book uses the sidebar, not the website navbar element.
    assert!(
        !methods.contains("<header class=\"tali-site-nav\""),
        "a book should not emit the website navbar"
    );

    // Cross-chapter `@ref`s resolve to the other page with the right number: the
    // Results chapter references `@sec-methods` (a chapter -> "Chapter 2") and
    // `@sec-setup` (a subsection -> "Section 2.1"), both on methods.html.
    let results = site.render_page("results.tmd").expect("results renders");
    assert!(
        results.contains(
            "<a href=\"methods.html#sec-methods\" class=\"tali-xref\">Chapter&nbsp;2</a>"
        ),
        "cross-chapter ref to a chapter not resolved: {}",
        results
            .match_indices("tali-xref")
            .next()
            .map(|_| "(see tali-xref links)")
            .unwrap_or("(no tali-xref at all)")
    );
    assert!(
        results.contains(
            "<a href=\"methods.html#sec-setup\" class=\"tali-xref\">Section&nbsp;2.1</a>"
        ),
        "cross-chapter ref to a subsection not resolved"
    );
    // No unresolved marker should leak into the output once a target is known.
    assert!(
        !results.contains("data-tali-xref=\"sec-methods\""),
        "resolved cross-ref still carries its marker"
    );
    // A cross-PAGE theorem ref resolves to the defining chapter WITH its number: a
    // theorem is a source-literal `:::` div, so `discover`'s render-harvest knows its
    // number ("Theorem 2.1" — methods is chapter 2, which scopes it, no config) in the
    // live preview as well as the static build.
    assert!(
        results
            .contains("<a href=\"methods.html#thm-kl\" class=\"tali-xref\">Theorem&nbsp;2.1</a>"),
        "cross-chapter theorem ref not numbered: {results}"
    );
    assert!(
        !results.contains("data-tali-xref=\"thm-kl\""),
        "resolved theorem cross-ref still carries its broken marker"
    );
}

/// A book's `logo:` reaches BOTH `.tali-book-brand` slots — the sticky topbar and the
/// chapter drawer's head — with the book title as the image's alt, so each link keeps an
/// accessible name. The two slots are separate emissions in `site/chrome.rs`, so a fix
/// applied to one of them leaves the other a bare wordmark; counting the construct is
/// what catches that. Needles the whole `<a …><img …></a>`, never a bare `logo`
/// substring: every page inlines the full CSS + JS payload.
#[test]
fn demo_book_logo_brands_both_the_topbar_and_the_chapter_drawer() {
    use taliesin_core::Site;
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let intro = site.render_page("intro.tmd").expect("intro renders");
    let brand = "<a class=\"tali-book-brand\" href=\"index.html\">\
                 <img class=\"tali-brand-logo\" src=\"logo.svg\" alt=\"A Short Demo Book\" /></a>";
    assert_eq!(
        intro.matches(brand).count(),
        2,
        "both book brand slots must carry the logo: {intro}"
    );
}

#[test]
fn book_chapter_scopes_theorem_numbers() {
    use taliesin_core::Site;
    let site = Site::discover(&corpus_dir().join("demo-book"));
    // methods.tmd is chapter 2, so its theorems scope to it with no config at all.
    let methods = site.render_page("methods.tmd").expect("methods renders");
    assert!(
        methods.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;2.1</span></span>"
        ),
        "the chapter-2 theorem numbers as 2.1: {methods}"
    );
    assert!(
        methods.contains("<a href=\"#thm-kl\" class=\"tali-xref\">Theorem&nbsp;2.1</a>"),
        "its in-page cross-ref agrees: {methods}"
    );
}

/// A numbered book scopes float numbers to the chapter, so two chapters no longer both
/// open with a "Figure 1" and a cross-chapter `@fig-` ref is unambiguous. intro.tmd is
/// chapter 1 and methods.tmd is chapter 2; each carries one labelled figure, and methods
/// references BOTH — its own (2.1) and the intro's (1.1) — so one page pins the
/// disambiguation this exists for. Unlike theorems, floats scope with no front matter
/// asking for it.
#[test]
fn book_chapter_scopes_float_numbers_across_chapters() {
    use taliesin_core::Site;
    let site = Site::discover(&corpus_dir().join("demo-book"));

    let intro = site.render_page("intro.tmd").expect("intro renders");
    assert!(
        intro.contains("<figcaption>Figure&nbsp;1.1: How this book"),
        "chapter 1's first figure numbers as 1.1: {intro}"
    );
    assert!(
        intro.contains("<a href=\"#fig-structure\" class=\"tali-xref\">Figure&nbsp;1.1</a>"),
        "the intro's own ref to it agrees: {intro}"
    );

    let methods = site.render_page("methods.tmd").expect("methods renders");
    // Chapter 2's first figure is 2.1, NOT the flat "Figure 2" a shared per-page counter
    // would have produced.
    assert!(
        methods.contains("<figcaption>Figure&nbsp;2.1: The three estimation stages.</figcaption>"),
        "chapter 2's first figure numbers as 2.1: {methods}"
    );
    assert!(
        methods.contains("<a href=\"#fig-pipeline\" class=\"tali-xref\">Figure&nbsp;2.1</a>"),
        "its same-chapter ref agrees: {methods}"
    );
    // The cross-chapter ref keeps the DEFINING chapter's number and links to its page.
    assert!(
        methods.contains(
            "<a href=\"intro.html#fig-structure\" class=\"tali-xref\">Figure&nbsp;1.1</a>"
        ),
        "the cross-chapter ref resolves to the intro's chapter-scoped number: {methods}"
    );
    assert!(
        !methods.contains("data-tali-xref=\"fig-structure\""),
        "resolved cross-chapter figure ref still carries its broken marker: {methods}"
    );
}

/// Authored source extensions that must stay in lockstep between twinned corpus
/// documents. Generated media is excluded on purpose: `fourier-transform`'s own
/// `{python}` cell writes `chord.wav`/`tone_*.wav` at render time, so those bytes
/// are an output, not an authored invariant. The gitignored `_freeze/` cache is
/// likewise skipped.
const TWINNED_SOURCE_EXTS: [&str; 4] = ["tmd", "bib", "js", "css"];

fn is_twinned_source(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| TWINNED_SOURCE_EXTS.contains(&e))
}

/// Every authored file that exists under both `a_root` and `b_root` at the same
/// relative path, discovered rather than hardcoded so a renamed or newly-shared
/// document is picked up automatically.
fn shared_sources(a_root: &Path, b_root: &Path, rel: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(a_root.join(rel)) else {
        return;
    };
    for entry in entries {
        let p = entry.unwrap().path();
        let name = p.file_name().unwrap().to_owned();
        let child = rel.join(&name);
        if p.is_dir() {
            if name == "_freeze" {
                continue;
            }
            shared_sources(a_root, b_root, &child, out);
        } else if is_twinned_source(&p) && b_root.join(&child).is_file() {
            out.push(child);
        }
    }
}

/// `corpus/posts/<slug>/` and `corpus/tech-blog/posts/<slug>/` hold identical copies of
/// three posts (plus a shared `_includes/three-scene.tmd`), and both copies are live
/// documents in the regression net. Nothing stopped a content fix from landing in one
/// copy and rotting the other; `fa200e5`'s own message notes that "every fix lands
/// twice". This pins that.
///
/// The one licensed difference is an `{{< include >}}`'s **path prefix**. The two copies
/// sit under different project boundaries, and the boundary decides what a relative
/// include may reach: the tech-blog copy resolves `../../_includes/three-scene.tmd`
/// against `corpus/tech-blog/_site.yml` (pinned as that exact literal by
/// `includes_are_resolved_with_origin_files` and `include_relative_base.rs`), while the
/// loose copy has no `_site.yml` above it and so is confined to its own directory
/// (PT-2). Byte-identity and both-copies-resolve cannot both hold. Normalising the
/// include target to its basename keeps the pin on *which file* is pulled in and on all
/// the prose/code around it, and gives up only the prefix the boundary dictates.
#[test]
fn twinned_corpus_sources_stay_byte_identical() {
    /// Reduce `{{< include <path>/<name>.tmd … >}}` to `{{< include <name>.tmd … >}}`,
    /// so the two copies' differing project-relative prefixes compare equal.
    fn normalize_include_paths(bytes: &[u8]) -> String {
        let text = String::from_utf8_lossy(bytes);
        text.lines()
            .map(|line| match line.find("{{< include ") {
                None => line.to_string(),
                Some(at) => {
                    let head = &line[..at + "{{< include ".len()];
                    let tail = &line[at + "{{< include ".len()..];
                    let (path, rest) = tail.split_once(' ').unwrap_or((tail, ""));
                    let base = path.rsplit('/').next().unwrap_or(path);
                    format!("{head}{base} {rest}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    let corpus = corpus_dir();
    let roots = [
        (corpus.join("posts"), corpus.join("tech-blog/posts")),
        (corpus.join("_includes"), corpus.join("tech-blog/_includes")),
    ];

    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (a_root, b_root) in &roots {
        let mut rels = Vec::new();
        shared_sources(a_root, b_root, Path::new(""), &mut rels);
        pairs.extend(rels.into_iter().map(|r| (a_root.join(&r), b_root.join(&r))));
    }
    pairs.sort();

    // A rename must not silently make this test vacuous.
    assert!(
        pairs.len() >= 8,
        "expected at least the 3 twinned posts' sources + the shared include, found {}: {pairs:#?}",
        pairs.len()
    );

    let drifted: Vec<String> = pairs
        .iter()
        .filter(|(a, b)| {
            normalize_include_paths(&fs::read(a).unwrap())
                != normalize_include_paths(&fs::read(b).unwrap())
        })
        .map(|(a, b)| {
            format!(
                "  {} != {}",
                a.strip_prefix(&corpus).unwrap().display(),
                b.strip_prefix(&corpus).unwrap().display()
            )
        })
        .collect();

    assert!(
        drifted.is_empty(),
        "twinned corpus sources have drifted; a fix landed in one copy only:\n{}",
        drifted.join("\n")
    );
}

/// `<title>` precedence, pinned on the real book. `_site.yml` gives `methods.tmd` the
/// chapter label "Methodology" while the file's own heading is `# Methods`; the authored
/// override must win. A chapter with no override and no front-matter title falls back to
/// its leading H1, which beats the empty string the site path used to emit (and which let
/// `og:title` quietly borrow the site's own name).
#[test]
fn a_site_page_prefers_its_authored_title_then_its_leading_h1() {
    use taliesin_core::Site;
    let root = corpus_dir().join("demo-book");
    let site = Site::discover(&root);

    let title_of = |rel: &str| -> String {
        let page = site
            .pages
            .iter()
            .find(|p| p.rel == rel)
            .unwrap_or_else(|| panic!("no page {rel}"));
        let src = fs::read_to_string(&page.input).unwrap();
        let base = page.input.parent().unwrap();
        let doc =
            taliesin_core::render_document_with_includes_scoped(&src, base, site.chapter_for(page));
        let (html, _) = site.render_page_doc_warned(page, doc);
        html.split("<title>")
            .nth(1)
            .and_then(|s| s.split("</title>").next())
            .unwrap_or("")
            .to_string()
    };

    // An `_site.yml` `text:` override beats the file's own `# Methods` heading; every
    // chapter tab also carries the " · <site>" site-name suffix (the book title here).
    let book = " · A Short Demo Book";
    assert_eq!(title_of("methods.tmd"), format!("Methodology{book}"));
    assert_eq!(title_of("summary.tmd"), format!("Wrap-up{book}"));
    // No override, no front matter: the leading H1, never the empty string.
    assert_eq!(title_of("results.tmd"), format!("Results{book}"));
    assert!(!title_of("results.tmd").is_empty());
}

#[test]
fn every_titled_post_emits_exactly_one_h1() {
    // Heading demotion (#11): a post renders its title as the sole <h1>; its body `#`
    // sections demote to <h2>+ so the page keeps a single-<h1> document outline (a11y/SEO).
    let posts_dir = corpus_dir().join("tech-blog/posts");
    let mut posts = Vec::new();
    collect_tmd(&posts_dir, &mut posts);
    assert!(
        !posts.is_empty(),
        "expected posts under {}",
        posts_dir.display()
    );
    for f in &posts {
        let src = fs::read_to_string(f).unwrap();
        let doc = taliesin_core::render_document_with_includes(&src, f.parent().unwrap());
        let n = doc.body_html().matches("<h1").count();
        let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
        assert_eq!(n, 1, "{label} should emit exactly one <h1>, found {n}");
    }
}
