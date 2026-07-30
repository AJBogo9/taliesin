//! AVIF derivatives for a built page's local raster images (backlog item 169).
//!
//! # Why AVIF and nothing else
//!
//! The item was filed as "WebP/AVIF". Measured, that is not a choice of two:
//!
//! - `image-webp` (pure Rust) **encodes lossless only** — its own README says so — and
//!   lossless WebP on a photograph is larger than the JPEG it would replace.
//! - Lossy WebP means `libwebp`, which is C, vendored, and needs a toolchain on every build
//!   machine including the macOS cross-builds in `release.yml`.
//! - AVIF encodes in pure Rust through `image`'s own `avif` feature, and beat WebP q80 on
//!   three of the four real corpus images it was measured against.
//!
//! **Never reach for `ravif` directly.** It turns on rav1e's `asm` feature, whose build
//! script fails with "NASM build failed. Make sure you have nasm installed" — measured on
//! this machine, which has no nasm. `image`'s `avif` feature configures rav1e without asm,
//! which is why a cold release build of it is 21.8 s here rather than a hard error.
//!
//! # Why this is build-only
//!
//! One 1200x630 encode at the quality/speed below is **~0.9 s**. A preview that transcoded on
//! demand would cost a six-image page ~6 s on first load, so the preview deliberately serves
//! the author's original bytes and only the *build* produces derivatives. The preview is not
//! lying about the page: core annotates the `<img>` identically on both paths
//! (`taliesin_core::render::image_meta`), and `<picture>` is a non-rendering wrapper, so the
//! two lay out the same and differ only in which bytes a browser chooses to fetch.

use std::path::{Path, PathBuf};

use image::ImageEncoder;
use rayon::prelude::*;

/// Encoder identity baked into every cache key. **Bump on any change to the parameters
/// below**, or a rebuild would serve bytes the current settings would not produce.
const ENCODER_TAG: &str = "avif-q72-s4-v1";

/// Measured on the real corpus (`og-card.png` 1200x630): quality 72 at speed 4 is 21,651 B
/// against 27,468 B at speed 8 and 42,003 B at speed 10 — speed 10 is nearly double the bytes
/// for the same pixels. Speed 4's ~0.9 s cost is paid once, behind the cache below.
const QUALITY: u8 = 72;
const SPEED: u8 = 4;

/// Candidate widths, before the never-upscale filter in [`rungs`].
const RUNG_WIDTHS: [u32; 2] = [480, 960];

/// Matches `--tali-maxw: 46rem` (736 px) in `base.css`.
const SIZES: &str = "(max-width: 46rem) 100vw, 736px";

/// Extensions worth transcoding. `.webp` and `.avif` are absent on purpose: an author who
/// already shipped one has made the decision this module exists to make.
const SOURCE_EXT: [&str; 4] = ["png", "jpg", "jpeg", "gif"];

/// What one build produced, for the build's own reporting line.
#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) struct Stats {
    /// `<img>` tags wrapped in a `<picture>`.
    pub(crate) images: usize,
    /// Bytes the AVIF rungs saved against serving the original at every rung.
    pub(crate) saved: u64,
    /// Encodes served from the persistent cache instead of being recomputed.
    pub(crate) cached: usize,
    /// Every derivative written, **relative to the output root and normalized**.
    ///
    /// This is not bookkeeping: `build::sweep_stale` deletes any file under the output tree
    /// that is not in its `keep` set, so a derivative the caller cannot name is written and
    /// then removed by the same build — with every unit test in this file still green,
    /// because they never run the sweep. The caller MUST feed these into `keep`.
    pub(crate) written: Vec<PathBuf>,
}

impl Stats {
    pub(crate) fn merge(&mut self, other: Stats) {
        self.images += other.images;
        self.saved += other.saved;
        self.cached += other.cached;
        self.written.extend(other.written);
    }

    /// Derivative files written, for the build's reporting line.
    pub(crate) fn files(&self) -> usize {
        self.written.len()
    }
}

/// Join `rel_dir` and a page-relative `rel`, resolving `.`/`..` lexically, so the result can
/// be compared against `sweep_stale`'s `keep` set (whose entries come from `strip_prefix` and
/// are always normalized). Returns `None` if the reference climbs above the output root.
///
/// A listing card's `src` really does carry `../`: `site/mod.rs` emits `src="{up}{path}"`.
fn normalize_under(rel_dir: &Path, rel: &str) -> Option<PathBuf> {
    let mut parts: Vec<std::ffi::OsString> = rel_dir
        .components()
        .map(|c| c.as_os_str().to_os_string())
        .collect();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s.into()),
        }
    }
    let mut out = PathBuf::new();
    for p in parts {
        out.push(p);
    }
    Some(out)
}

/// The width rungs to emit for a source `native` pixels wide.
///
/// **Never above `native`.** Measured: re-encoding `astar.png` (native 637 px) at an 800 px
/// rung produced 12,274 B against 4,798 B at native — 44% *larger than the original file it
/// was meant to shrink*, because upscaling invents pixels the codec then has to store.
fn rungs(native: u32) -> Vec<u32> {
    let mut out: Vec<u32> = RUNG_WIDTHS
        .iter()
        .copied()
        .filter(|w| *w < native)
        .collect();
    out.push(native);
    out
}

/// Cache key for one derivative: source **bytes**, not mtime, so a `git checkout` or a
/// touched file does not invalidate and a changed pixel always does. Same no-stale-hits
/// property the freeze cache gives cell outputs.
fn key(src_bytes: &[u8], width: u32) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(src_bytes);
    h.update(width.to_le_bytes());
    h.update(ENCODER_TAG.as_bytes());
    format!("{:x}", h.finalize())[..16].to_string()
}

/// Encode one rung, consulting (and filling) the persistent cache.
fn encode_rung(
    img: &image::DynamicImage,
    src_bytes: &[u8],
    width: u32,
    cache_dir: Option<&Path>,
) -> Option<(String, Vec<u8>, bool)> {
    let k = key(src_bytes, width);
    let cached = cache_dir.map(|d| d.join(format!("{k}.avif")));
    if let Some(p) = &cached
        && let Ok(bytes) = std::fs::read(p)
    {
        return Some((k, bytes, true));
    }
    // `resize` only ever shrinks here (see `rungs`), so Lanczos3's ringing on upscale is
    // not reachable.
    let scaled = if width == img.width() {
        img.clone()
    } else {
        let h = (img.height() as u64 * width as u64 / img.width().max(1) as u64).max(1) as u32;
        img.resize_exact(width, h, image::imageops::FilterType::Lanczos3)
    };
    let rgba = scaled.to_rgba8();
    let mut buf: Vec<u8> = Vec::new();
    image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut buf, SPEED, QUALITY)
        .write_image(
            rgba.as_raw(),
            scaled.width(),
            scaled.height(),
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    if let Some(p) = &cached
        && let Some(parent) = p.parent()
    {
        let _ = std::fs::create_dir_all(parent);
        let _ = std::fs::write(p, &buf);
    }
    Some((k, buf, false))
}

/// One derivative, ready to write.
struct Rung {
    width: u32,
    /// Path relative to the page, i.e. what goes in the `srcset`.
    rel: String,
    bytes: Vec<u8>,
    from_cache: bool,
}

/// Build every worthwhile rung for one image reference.
///
/// Returns `None` when the reference is not ours to touch or when no rung is worth serving.
fn derivatives(src_ref: &str, base: &Path, cache_dir: Option<&Path>) -> Option<Vec<Rung>> {
    let path = &src_ref[..src_ref.find(['?', '#']).unwrap_or(src_ref.len())];
    if path.is_empty() || path.starts_with('/') || path.contains("://") {
        return None;
    }
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())?
        .to_ascii_lowercase();
    if !SOURCE_EXT.contains(&ext.as_str()) {
        return None;
    }
    let abs = base.join(path);
    let src_bytes = std::fs::read(&abs).ok()?;
    let img = image::load_from_memory(&src_bytes).ok()?;
    let stem = path.strip_suffix(&format!(".{ext}")).unwrap_or(path);

    let out: Vec<Rung> = rungs(img.width())
        .into_par_iter()
        .filter_map(|w| {
            let (k, bytes, from_cache) = encode_rung(&img, &src_bytes, w, cache_dir)?;
            // A rung that is not smaller than the file it would replace is not an
            // optimization. Small flat PNGs really do encode larger as AVIF.
            (bytes.len() < src_bytes.len()).then(|| Rung {
                width: w,
                rel: format!("{stem}.{k}-{w}w.avif"),
                bytes,
                from_cache,
            })
        })
        .collect();
    (!out.is_empty()).then_some(out)
}

/// Rewrite every local raster `<img>` in `html` into a `<picture>` with an AVIF `srcset`,
/// writing the derivative files under `out_root`.
///
/// The `<img>` itself is copied through **byte for byte**: it stays the fallback for a
/// browser without AVIF, and keeping it identical is what makes the built page and the
/// preview lay out the same.
///
/// `base` is the directory a page's `src` values resolve against on disk; `rel_dir` is that
/// page's directory *relative to `out_root`*, so a derivative can be named the two ways it
/// has to be named at once — page-relative for the `srcset`, output-relative for `keep`.
pub(crate) fn optimize(
    html: &str,
    base: &Path,
    out_root: &Path,
    rel_dir: &Path,
    cache_dir: Option<&Path>,
) -> (String, Stats) {
    let mut stats = Stats::default();
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = find_img_tag(rest) {
        let (before, from_tag) = rest.split_at(pos);
        out.push_str(before);
        let Some(end) = from_tag.find('>') else {
            out.push_str(from_tag);
            return (out, stats);
        };
        let tag = &from_tag[..=end];
        rest = &from_tag[end + 1..];

        let Some(src) = attr_value(tag, "src") else {
            out.push_str(tag);
            continue;
        };
        // A themed pair (`dark=`) is two <img>s whose CSS picks one; wrapping each in its own
        // <picture> keeps that selector working, since `picture` is not a layout box.
        let Some(rung_set) = derivatives(&src, base, cache_dir) else {
            out.push_str(tag);
            continue;
        };
        let mut one = Stats::default();
        let mut biggest = 0usize;
        for r in &rung_set {
            // Named twice on purpose: page-relative for the `srcset` the browser reads, and
            // normalized output-relative for `sweep_stale`'s keep set.
            let Some(rel_out) = normalize_under(rel_dir, &r.rel) else {
                continue;
            };
            let to = out_root.join(&rel_out);
            if let Some(parent) = to.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::write(&to, &r.bytes).is_ok() {
                one.written.push(rel_out);
                biggest = biggest.max(r.bytes.len());
                if r.from_cache {
                    one.cached += 1;
                }
            }
        }
        if one.written.is_empty() {
            out.push_str(tag);
            continue;
        }
        let srcset = rung_set
            .iter()
            .map(|r| format!("{} {}w", r.rel, r.width))
            .collect::<Vec<_>>()
            .join(", ");
        let orig_len = std::fs::metadata(base.join(&src))
            .map(|m| m.len())
            .unwrap_or(0);
        one.saved = orig_len.saturating_sub(biggest as u64);
        one.images = 1;
        stats.merge(one);
        out.push_str(&format!(
            "<picture><source type=\"image/avif\" srcset=\"{srcset}\" sizes=\"{SIZES}\">{tag}</picture>"
        ));
    }
    out.push_str(rest);
    (out, stats)
}

/// Byte offset of the next `<img` opener, requiring a delimiter so SVG's `<image>` is not one.
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

/// The double-quoted value of a whole attribute named `name`.
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let needle = format!("{name}=\"");
    let mut i = 0;
    while let Some(pos) = tag[i..].find(&needle) {
        let at = i + pos;
        let lead_ok = at > 0 && bytes[at - 1].is_ascii_whitespace();
        let start = at + needle.len();
        let len = tag[start..].find('"')?;
        if lead_ok {
            return Some(tag[start..start + len].to_string());
        }
        i = start + len;
    }
    None
}

/// Subdirectory of a project's `_freeze/` holding encoded derivatives, alongside the cell
/// outputs it borrows its no-stale-hits property from.
pub(crate) const CACHE_SUBDIR: &str = "img";

/// A byte count for the build's report line. Decimal units, because that is what a reader
/// comparing against a browser's network panel will see.
pub(crate) fn human_bytes(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1} MB", n as f64 / 1_000_000.0),
        n if n >= 1_000 => format!("{:.0} kB", n as f64 / 1_000.0),
        n => format!("{n} B"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "tali-imgopt-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A noisy image, so AVIF cannot win by encoding a flat field into nothing — a flat
    /// fixture would make every size assertion below pass for the wrong reason.
    fn noisy_png(dir: &Path, name: &str, w: u32, h: u32) {
        let mut buf = image::RgbaImage::new(w, h);
        let mut seed = 12345u32;
        for p in buf.pixels_mut() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let b = seed.to_le_bytes();
            *p = image::Rgba([b[0], b[1], b[2], 255]);
        }
        buf.save(dir.join(name)).unwrap();
    }

    #[test]
    fn a_rung_is_never_wider_than_the_source() {
        // astar.png is 637 px native; an 800 px rung measured 44% LARGER than the original.
        assert_eq!(rungs(637), vec![480, 637]);
        assert_eq!(rungs(300), vec![300]);
        assert_eq!(rungs(1200), vec![480, 960, 1200]);
        for native in [1, 120, 479, 480, 481, 960, 4000] {
            assert!(
                rungs(native).iter().all(|w| *w <= native),
                "rung above native for {native}: {:?}",
                rungs(native)
            );
        }
    }

    #[test]
    fn a_derivative_is_named_output_relative_so_the_stale_sweep_keeps_it() {
        // `sweep_stale` deletes every file under the output not in `keep`, and its entries
        // come from `strip_prefix(out)`, so they are normalized. A listing card's src really
        // does climb: `site/mod.rs` emits `src="{up}{path}"`. An unnormalized `posts/a/../..`
        // would never match, and the derivative would be written and then deleted by the same
        // build with every other test in this file still green.
        assert_eq!(
            normalize_under(Path::new("posts/a"), "thumb.k-480w.avif"),
            Some(PathBuf::from("posts/a/thumb.k-480w.avif"))
        );
        assert_eq!(
            normalize_under(Path::new("posts/a"), "../../img/t.k-480w.avif"),
            Some(PathBuf::from("img/t.k-480w.avif"))
        );
        assert_eq!(
            normalize_under(Path::new(""), "t.avif"),
            Some(PathBuf::from("t.avif"))
        );
        // Climbing above the output root is not ours to write.
        assert_eq!(
            normalize_under(Path::new("posts"), "../../escape.avif"),
            None
        );
    }

    #[test]
    fn derivatives_land_beside_the_page_that_references_them() {
        let d = tmp("reldir");
        std::fs::create_dir_all(d.join("posts/a")).unwrap();
        noisy_png(&d.join("posts/a"), "t.png", 700, 400);
        let out = d.join("out");
        let (html, stats) = optimize(
            r#"<img src="t.png" />"#,
            &d.join("posts/a"),
            &out,
            Path::new("posts/a"),
            None,
        );
        assert_eq!(stats.images, 1, "{html}");
        for rel in &stats.written {
            assert!(
                rel.starts_with("posts/a"),
                "derivative must be named output-relative for the sweep: {rel:?}"
            );
            assert!(
                out.join(rel).is_file(),
                "advertised but not written: {rel:?}"
            );
        }
        // The srcset stays PAGE-relative, which is the other half of the same fact.
        assert!(
            html.contains(r#"srcset="t."#) && !html.contains("srcset=\"posts/"),
            "srcset must be page-relative: {html}"
        );
    }

    #[test]
    fn the_cache_key_changes_with_bytes_width_and_encoder_but_not_otherwise() {
        assert_eq!(key(b"abc", 480), key(b"abc", 480));
        assert_ne!(key(b"abc", 480), key(b"abd", 480));
        assert_ne!(key(b"abc", 480), key(b"abc", 960));
        // The encoder tag is in the key; if this ever stops being true, a parameter change
        // would silently serve bytes the current settings would not produce.
        assert!(
            ENCODER_TAG.contains(&QUALITY.to_string()) && ENCODER_TAG.contains(&SPEED.to_string()),
            "ENCODER_TAG {ENCODER_TAG} must name the parameters it guards"
        );
    }

    #[test]
    fn an_image_is_wrapped_in_a_picture_whose_img_is_byte_identical() {
        let d = tmp("wrap");
        noisy_png(&d, "a.png", 700, 400);
        let out_dir = d.join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let img = r#"<img src="a.png" alt="x" width="700" height="400" fetchpriority="high" />"#;
        let (html, stats) = optimize(&format!("<p>{img}</p>"), &d, &out_dir, Path::new(""), None);
        assert_eq!(stats.images, 1, "expected one wrap: {html}");
        assert!(
            html.contains(&format!(r#" sizes="{SIZES}">{img}</picture>"#)),
            "the <img> must survive byte for byte inside the <picture>: {html}"
        );
        assert!(html.starts_with("<p><picture>") && html.ends_with("</picture></p>"));
        assert!(
            html.contains(r#"type="image/avif""#) && html.contains(" 480w, "),
            "expected an avif srcset with both rungs: {html}"
        );
        // Every advertised rung was actually written.
        for w in [480, 700] {
            assert!(
                std::fs::read_dir(&out_dir)
                    .unwrap()
                    .filter_map(|e| e.ok())
                    .any(|e| e
                        .file_name()
                        .to_string_lossy()
                        .ends_with(&format!("-{w}w.avif"))),
                "no file written for the {w}w rung"
            );
        }
    }

    #[test]
    fn the_second_build_reads_the_cache_instead_of_re_encoding() {
        let d = tmp("cache");
        noisy_png(&d, "a.png", 700, 400);
        let out_dir = d.join("out");
        let cache = d.join("_freeze/img");
        let html = r#"<img src="a.png" alt="x" />"#;
        let (_, first) = optimize(html, &d, &out_dir, Path::new(""), Some(&cache));
        assert_eq!(first.cached, 0, "a cold build must encode: {first:?}");
        let (_, second) = optimize(html, &d, &out_dir, Path::new(""), Some(&cache));
        assert_eq!(
            second.cached,
            first.files(),
            "every rung should have come from the cache the second time: {second:?}"
        );
    }

    #[test]
    fn already_optimized_and_remote_sources_are_left_alone() {
        let d = tmp("skip");
        noisy_png(&d, "a.png", 700, 400);
        let out = d.join("out");
        for src in [
            "https://example.com/a.png",
            "/a.png",
            "a.webp",
            "a.avif",
            "a.svg",
            "missing.png",
            "data:image/png;base64,iVBORw0KG",
        ] {
            let html = format!("<img src=\"{src}\" />");
            let (got, stats) = optimize(&html, &d, &out, Path::new(""), None);
            assert_eq!(got, html, "must not rewrite {src}");
            assert_eq!(stats.images, 0);
        }
    }

    #[test]
    fn a_rung_that_would_be_larger_than_the_original_is_dropped() {
        // A tiny already-compact source: AVIF's container overhead alone can exceed it, and
        // shipping a "derivative" bigger than the file it replaces is not an optimization.
        let d = tmp("bigger");
        let out = d.join("out");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))
            .save(d.join("tiny.png"))
            .unwrap();
        let html = r#"<img src="tiny.png" />"#;
        let (got, stats) = optimize(html, &d, &out, Path::new(""), None);
        assert_eq!(got, html, "a losing rung must leave the tag alone");
        assert_eq!(stats.images, 0);
    }
}
