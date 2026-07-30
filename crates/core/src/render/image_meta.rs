//! Intrinsic dimensions and loading hints for local raster images.
//!
//! **Why this is a post-emission pass and not part of the emitters.** Two places emit an
//! `<img>`: [`super::emit`]'s inline-image arm and [`super::figure`]'s `<figure>`. Neither is
//! reachable from a `base_dir` — they are free functions inside comrak's recursive AST walk,
//! and threading a path through `emit_node`/`emit_children` would touch every call site to
//! serve two of them. The orchestrator already establishes the alternative one line away
//! (`shift_heading_html`): transform a block's finished HTML before it becomes a [`Block`],
//! where `base_dir` is in scope. One pass then covers both emitters with the same shape the
//! build's own `local_refs` scanner uses.
//!
//! `data-block-id` is a content hash of the **source**, so rewriting emitted HTML here leaves
//! it stable; the block model, the diff and click-to-source are untouched.
//!
//! **Why `width`/`height` at all.** They reserve the image's box before its bytes arrive, so
//! the text below it does not jump when it loads. That is worth having in the *preview* too,
//! which is why it lives in core rather than in the build beside the AVIF transcode: the
//! preview must keep predicting the built page.

use std::path::Path;

/// Extensions whose intrinsic size we can read. Exactly the decoders enabled on the `image`
/// dependency, and no more: `.svg` has no reliable intrinsic pixel size, and `.avif` decoding
/// is deliberately absent because it would mean dav1d (C).
const RASTER_EXT: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

/// Rewrites `<img>` tags across a document's blocks, in document order.
///
/// Stateful because of the LCP rule below: "the first image" is a property of the document,
/// not of the block, so one annotator is threaded across the whole block walk.
pub(super) struct ImageAnnotator {
    seen_first: bool,
}

impl ImageAnnotator {
    pub(super) fn new() -> Self {
        Self { seen_first: false }
    }

    /// Annotate every qualifying local raster `<img>` in one block's `html`.
    ///
    /// Adds intrinsic `width`/`height` always. The **first** image in the document is left
    /// eager and marked `fetchpriority="high"`; every later one gets `loading="lazy"` and
    /// `decoding="async"`. Lazy-loading an above-the-fold image *delays* LCP rather than
    /// improving it, so a blanket `loading="lazy"` would be a regression dressed as an
    /// optimization.
    pub(super) fn annotate(&mut self, html: &str, base: &Path) -> String {
        let mut out = String::with_capacity(html.len());
        let mut rest = html;
        while let Some(pos) = find_img_tag(rest) {
            let (before, from_tag) = rest.split_at(pos);
            out.push_str(before);
            // The tag runs to the first `>`; an unterminated tag is not ours to repair.
            let Some(end) = from_tag.find('>') else {
                out.push_str(from_tag);
                return out;
            };
            let tag = &from_tag[..end];
            match self.attrs_for(tag, base) {
                Some(extra) => {
                    // Split off any self-closing slash so `<img … />` stays `… />`. Both the
                    // head and the slash are re-spaced from scratch rather than preserved:
                    // the two emitters differ in their trailing whitespace (figure.rs leaves
                    // one before `/>`, and an empty `{style}` leaves two), so splicing at the
                    // raw offset produced `alt="x"  width="1"` and `"high"/>`.
                    let (head, tail) = match tag.trim_end().strip_suffix('/') {
                        Some(h) => (h.trim_end(), " /"),
                        None => (tag.trim_end(), ""),
                    };
                    out.push_str(head);
                    out.push_str(&extra);
                    out.push_str(tail);
                }
                None => out.push_str(tag),
            }
            out.push('>');
            rest = &from_tag[end + 1..];
        }
        out.push_str(rest);
        out
    }

    /// The attribute string to splice into `tag`, or `None` to leave it alone.
    fn attrs_for(&mut self, tag: &str, base: &Path) -> Option<String> {
        // An author-set `width=` owns the box; a second one would be ambiguous.
        if attr_value(tag, "width").is_some() {
            return None;
        }
        let src = attr_value(tag, "src")?;
        let (w, h) = intrinsic_size(&src, base)?;
        let hints = if self.seen_first {
            " loading=\"lazy\" decoding=\"async\""
        } else {
            // Eager, and hinted as the likely LCP element.
            " fetchpriority=\"high\""
        };
        self.seen_first = true;
        Some(format!(" width=\"{w}\" height=\"{h}\"{hints}"))
    }
}

/// Byte offset of the next `<img` tag opener, requiring a delimiter after the name so the
/// SVG element `<image>` is not mistaken for one.
fn find_img_tag(hay: &str) -> Option<usize> {
    let bytes = hay.as_bytes();
    let mut i = 0;
    while let Some(pos) = hay[i..].find("<img") {
        let at = i + pos;
        match bytes.get(at + 4) {
            Some(c) if c.is_ascii_whitespace() || *c == b'>' || *c == b'/' => return Some(at),
            _ => i = at + 4,
        }
    }
    None
}

/// The double-quoted value of `name` in `tag`, matching whole attribute names only.
///
/// The whole-name test is the same one `build::local_refs` needs and for the same reason:
/// `fetchpriority="high"` ends in `priority=`, and a naive `find("width=")` inside a tag
/// carrying `data-width="…"` reads the wrong attribute.
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let needle = format!("{name}=\"");
    let mut i = 0;
    while let Some(pos) = tag[i..].find(&needle) {
        let at = i + pos;
        let lead_ok = at > 0 && bytes[at - 1].is_ascii_whitespace();
        let start = at + needle.len();
        let Some(len) = tag[start..].find('"') else {
            return None;
        };
        if lead_ok {
            return Some(tag[start..start + len].to_string());
        }
        i = start + len;
    }
    None
}

/// Intrinsic pixel size of a local relative raster `src`, or `None` if it is not one we
/// handle or does not resolve to a readable image under `base`.
///
/// Skipped, each for its own reason: an absolute URL and a protocol-relative `//host/x` are
/// not ours; a `data:` URI is what an executed `{python}`/`{r}` figure is, and it already
/// carries its pixels; a root-absolute `/x.png` has no meaning relative to `base` (the
/// build's asset scanner skips it for the same reason); a non-raster extension has no
/// intrinsic pixel size to state.
fn intrinsic_size(src: &str, base: &Path) -> Option<(u32, u32)> {
    if src.is_empty() || src.starts_with('/') || src.contains("://") || src.starts_with("data:") {
        return None;
    }
    // A query or fragment is addressing, not path.
    let path = &src[..src.find(['?', '#']).unwrap_or(src.len())];
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())?
        .to_ascii_lowercase();
    if !RASTER_EXT.contains(&ext.as_str()) {
        return None;
    }
    // Header read only: no decode, measured in microseconds.
    image::image_dimensions(base.join(path)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real PNG of the given size, written under a per-test temp dir.
    fn fixture(dir: &Path, name: &str, w: u32, h: u32) {
        std::fs::create_dir_all(dir).unwrap();
        let buf = image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255]));
        buf.save(dir.join(name)).unwrap();
    }

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "tali-imgmeta-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn intrinsic_dimensions_are_emitted_for_a_local_raster_image() {
        let d = tmp("dims");
        fixture(&d, "a.png", 640, 480);
        let got = ImageAnnotator::new().annotate(r#"<img src="a.png" alt="x" />"#, &d);
        assert!(
            got.contains(r#"width="640""#) && got.contains(r#"height="480""#),
            "expected intrinsic size in: {got}"
        );
        assert!(got.ends_with("/>"), "self-closing form must survive: {got}");
    }

    #[test]
    fn the_first_image_is_eager_and_later_ones_are_lazy() {
        // Lazy-loading the LCP image delays it. The first image in the DOCUMENT is the one
        // that must stay eager, which is why the annotator is threaded across blocks rather
        // than constructed per block.
        let d = tmp("lcp");
        fixture(&d, "a.png", 64, 64);
        fixture(&d, "b.png", 64, 64);
        let mut ann = ImageAnnotator::new();
        let first = ann.annotate(r#"<img src="a.png" />"#, &d);
        let second = ann.annotate(r#"<img src="b.png" />"#, &d);
        assert!(
            first.contains(r#"fetchpriority="high""#) && !first.contains("loading="),
            "first image must stay eager: {first}"
        );
        assert!(
            second.contains(r#"loading="lazy""#) && second.contains(r#"decoding="async""#),
            "later images must be lazy: {second}"
        );
    }

    #[test]
    fn remote_data_and_root_absolute_sources_are_left_alone() {
        let d = tmp("skip");
        for src in [
            "https://example.com/a.png",
            "//cdn.example.com/a.png",
            "data:image/png;base64,iVBORw0KG",
            "/a.png",
            "diagram.svg",
            "missing.png",
        ] {
            let html = format!("<img src=\"{src}\" />");
            assert_eq!(
                ImageAnnotator::new().annotate(&html, &d),
                html,
                "must not annotate {src}"
            );
        }
    }

    #[test]
    fn an_author_set_width_is_never_overridden() {
        let d = tmp("author");
        fixture(&d, "a.png", 640, 480);
        let html = r#"<img src="a.png" width="50%" />"#;
        assert_eq!(ImageAnnotator::new().annotate(html, &d), html);
    }

    #[test]
    fn the_svg_image_element_is_not_mistaken_for_an_img_tag() {
        let d = tmp("svgimage");
        fixture(&d, "a.png", 64, 64);
        let html = r#"<svg><image href="a.png" /></svg>"#;
        assert_eq!(ImageAnnotator::new().annotate(html, &d), html);
    }

    #[test]
    fn surrounding_markup_and_multiple_images_survive_the_rewrite() {
        let d = tmp("multi");
        fixture(&d, "a.png", 10, 20);
        fixture(&d, "b.png", 30, 40);
        let got = ImageAnnotator::new().annotate(
            r#"<p>lead <img src="a.png" alt="one"> mid <img src="b.png" alt="two"> tail</p>"#,
            &d,
        );
        assert!(
            got.starts_with("<p>lead ") && got.ends_with(" tail</p>"),
            "{got}"
        );
        assert!(got.contains(r#"alt="one" width="10" height="20""#), "{got}");
        assert!(got.contains(r#"alt="two" width="30" height="40""#), "{got}");
    }

    #[test]
    fn the_rewritten_tag_is_exactly_shaped_not_merely_containing_the_attributes() {
        // Pinned as a whole tag, not by `contains`. The first version spliced at the raw
        // offset and emitted `alt="one"  width="10"` (two spaces, from an empty `{style}` in
        // figure.rs) and `fetchpriority="high"/>` (no space before the slash). Every
        // `contains`-style assertion in this file passed on that output.
        let d = tmp("shape");
        fixture(&d, "a.png", 10, 20);
        fixture(&d, "b.png", 30, 40);
        let mut ann = ImageAnnotator::new();
        assert_eq!(
            ann.annotate(r#"<img src="a.png" alt="one" />"#, &d),
            r#"<img src="a.png" alt="one" width="10" height="20" fetchpriority="high" />"#
        );
        // The non-self-closing spelling keeps its own shape.
        assert_eq!(
            ann.annotate(r#"<img src="b.png" alt="two">"#, &d),
            r#"<img src="b.png" alt="two" width="30" height="40" loading="lazy" decoding="async">"#
        );
    }

    #[test]
    fn a_query_string_does_not_hide_the_file() {
        let d = tmp("query");
        fixture(&d, "a.png", 12, 34);
        let got = ImageAnnotator::new().annotate(r#"<img src="a.png?v=2" />"#, &d);
        assert!(got.contains(r#"width="12""#), "{got}");
    }
}
