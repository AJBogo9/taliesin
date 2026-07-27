//! Every SVG the corpus and the site ship must actually render in an `img` element.
//!
//! Item 74's `logo:` shipped two brand SVGs that returned HTTP 200 and were copied into the
//! build correctly, and still painted a broken-image glyph on the forward-facing blog: each
//! carried an explanatory CSS comment that mentioned a tag name in angle brackets. An SVG
//! `style` element is **XML, not HTML** — it is not an implicit CDATA section — so that bare
//! `<` opened a tag, the closing `</style>` mismatched, and the browser refused the whole
//! document. `naturalWidth` was 0 while `fetch` reported `200 image/svg+xml`, which is why
//! every existing check passed: "the file exists and is served" is not "the file renders".
//!
//! These are dependency-free structural checks, not a full XML parse (the workspace has no
//! direct XML parser and one is not worth adding for this). They cover the two properties an
//! `<img src="…svg">` actually needs, and the second is the exact defect above.

use std::path::{Path, PathBuf};

/// Every tracked `.svg` under the document roots, excluding generated output.
fn shipped_svgs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = Vec::new();
    for top in ["corpus", "site", "docs", "crates/core/assets"] {
        walk(&root.join(top), &mut found);
    }
    found.sort();
    assert!(
        found.len() >= 10,
        "the SVG walk found only {} files — it is looking in the wrong place, and a pin that \
         scans nothing passes vacuously",
        found.len()
    );
    found
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name();
        let name = name.to_string_lossy();
        // `_site`/`_book`/`_freeze` are build output: they are copies, so a failure there is
        // a duplicate of the source failure and fixing it means fixing the source.
        if p.is_dir() {
            if !matches!(
                name.as_ref(),
                "_site" | "_book" | "_freeze" | "node_modules"
            ) {
                walk(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "svg") {
            out.push(p);
        }
    }
}

/// An SVG in an `img` element gets no size from CSS alone; without a `viewBox` the browser
/// has nothing to scale, and `.tali-brand-logo`'s `width: auto` collapses it.
#[test]
fn every_shipped_svg_declares_a_viewbox() {
    for path in shipped_svgs() {
        let text = std::fs::read_to_string(&path).expect("read svg");
        assert!(
            text.contains("viewBox"),
            "{}: an SVG loaded through an img element needs a viewBox to have an intrinsic \
             size; without one it renders 0-wide",
            path.display()
        );
    }
}

/// The defect item 74 shipped: markup-significant characters inside a `style` element.
///
/// In HTML a `<style>` body is raw text; in XML — which is what an SVG document is — it is
/// ordinary parsed content, so `<` starts a tag and `&` starts an entity. A CSS comment that
/// mentions a tag name therefore destroys the file. Nothing else in the tree catches this:
/// the renderer passes SVGs through untouched, the asset copier only checks existence, and
/// `check` never opens them.
#[test]
fn no_shipped_svg_puts_markup_characters_in_a_style_element() {
    for path in shipped_svgs() {
        let text = std::fs::read_to_string(&path).expect("read svg");
        let mut rest = text.as_str();
        while let Some(open) = rest.find("<style") {
            let after_open = &rest[open..];
            let body_start = after_open.find('>').expect("unclosed <style tag") + 1;
            let body_len = after_open[body_start..]
                .find("</style>")
                .unwrap_or_else(|| panic!("{}: <style> is never closed", path.display()));
            let body = &after_open[body_start..body_start + body_len];

            for (ch, what) in [('<', "angle bracket"), ('&', "ampersand")] {
                assert!(
                    !body.contains(ch),
                    "{}: a bare {what} inside a <style> element makes the SVG un-parseable as \
                     XML, so the browser renders a broken image even though the file exists \
                     and is served as image/svg+xml. Spell tag names out in words.\nstyle \
                     body:\n{body}",
                    path.display()
                );
            }
            rest = &after_open[body_start + body_len..];
        }
    }
}
