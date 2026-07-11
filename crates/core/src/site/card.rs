//! OpenGraph social-card generation: a branded 1200x630 PNG per content page,
//! hand-composited (no headless browser, no CDN). Deterministic: the same `CardSpec`
//! renders byte-identical output. Text uses the bundled Newsreader variable font's
//! default (Regular) instance; hierarchy is size + color.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};

use super::{Page, Site};
use crate::hash::fnv1a;

pub const CARD_W: u32 = 1200;
pub const CARD_H: u32 = 630;
/// Bumped when the template changes, to cache-bust every card URL.
pub const CARD_DESIGN_VERSION: u32 = 1;
/// Encoded card format extension (see the plan's Global Constraints).
pub const CARD_EXT: &str = "png";

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/Newsreader[opsz,wght].ttf");

const BG: [u8; 3] = [22, 24, 29];
const FG: [u8; 3] = [230, 230, 230];
const MUTED: [u8; 3] = [154, 160, 170];
const ACCENT: [u8; 3] = [154, 168, 220];
const BORDER: [u8; 3] = [54, 58, 68];

/// The text + branding a card renders. Derived per page by `card_spec` (Task 4).
pub struct CardSpec {
    pub eyebrow: Option<String>,
    pub headline: String,
    pub lead: Option<String>,
    pub footer_wordmark: String,
    pub domain: Option<String>,
}

/// A row-major RGBA8 pixel buffer with straight-alpha compositing over an opaque bg.
struct Canvas {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: u32, h: u32, bg: [u8; 3]) -> Self {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..(w * h) {
            px.extend_from_slice(&[bg[0], bg[1], bg[2], 255]);
        }
        Canvas { w, h, px }
    }

    fn blend(&mut self, x: i32, y: i32, color: [u8; 3], cov: f32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let cov = cov.clamp(0.0, 1.0);
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        for (c, &channel) in color.iter().enumerate() {
            let bg = self.px[i + c] as f32;
            let fg = channel as f32;
            self.px[i + c] = (fg * cov + bg * (1.0 - cov)).round() as u8;
        }
    }

    fn fill_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 3]) {
        for y in y0.max(0)..y1.min(self.h as i32) {
            for x in x0.max(0)..x1.min(self.w as i32) {
                self.blend(x, y, color, 1.0);
            }
        }
    }

    /// Stroke a polyline with round-ish AA: coverage falls off within half a pixel
    /// of the `width`-thick centerline. Used for the bell-curve mark.
    fn stroke_polyline(&mut self, pts: &[(f32, f32)], width: f32, color: [u8; 3]) {
        if pts.len() < 2 {
            return;
        }
        let hw = width / 2.0;
        let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in pts {
            minx = minx.min(x);
            miny = miny.min(y);
            maxx = maxx.max(x);
            maxy = maxy.max(y);
        }
        let x0 = (minx - hw - 1.0) as i32;
        let y0 = (miny - hw - 1.0) as i32;
        let x1 = (maxx + hw + 1.0) as i32;
        let y1 = (maxy + hw + 1.0) as i32;
        for py in y0..=y1 {
            for px in x0..=x1 {
                let p = (px as f32 + 0.5, py as f32 + 0.5);
                let mut d = f32::MAX;
                for w in pts.windows(2) {
                    d = d.min(dist_pt_seg(p, w[0], w[1]));
                }
                let cov = (hw + 0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.blend(px, py, color, cov);
                }
            }
        }
    }

    fn into_png(self) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.w, self.h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().expect("png header");
            writer.write_image_data(&self.px).expect("png data");
        }
        out
    }
}

/// The bundled Newsreader font (default = Regular instance). Cheap to construct
/// (borrows the static bytes); constructed per render, which stays deterministic.
fn font() -> FontRef<'static> {
    FontRef::try_from_slice(FONT_BYTES).expect("bundled Newsreader is a valid TTF")
}

/// Advance width of `text` at `px`, adding `tracking` extra px after each glyph.
fn text_width(f: &FontRef, text: &str, px: f32, tracking: f32) -> f32 {
    let scaled = f.as_scaled(PxScale::from(px));
    text.chars()
        .map(|ch| scaled.h_advance(f.glyph_id(ch)) + tracking)
        .sum()
}

/// Greedy word-wrap so each line's width is <= `max_w` (a single over-long word
/// still occupies its own line rather than being dropped).
fn wrap(f: &FontRef, text: &str, px: f32, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let trial = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if cur.is_empty() || text_width(f, &trial, px, 0.0) <= max_w {
            cur = trial;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Truncate a single line to `max_w` with a trailing ellipsis when it overflows.
fn truncate_line(f: &FontRef, text: &str, px: f32, max_w: f32) -> String {
    if text_width(f, text, px, 0.0) <= max_w {
        return text.to_string();
    }
    let mut s = String::new();
    for ch in text.chars() {
        if text_width(f, &format!("{s}{ch}\u{2026}"), px, 0.0) > max_w {
            break;
        }
        s.push(ch);
    }
    format!("{s}\u{2026}")
}

/// Wrap, then clamp to `max_lines`, ellipsizing the last kept line if content was cut.
fn wrap_clamp(f: &FontRef, text: &str, px: f32, max_w: f32, max_lines: usize) -> Vec<String> {
    if max_lines == 0 {
        return Vec::new();
    }
    let mut lines = wrap(f, text, px, max_w);
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = lines.pop().unwrap(); // safe: max_lines >= 1, so >=1 line remains
        lines.push(truncate_line(f, &format!("{last} \u{2026}"), px, max_w));
    }
    lines
}

impl Canvas {
    #[allow(clippy::too_many_arguments)] // matches the brief's spec verbatim
    /// Draw `text` with its baseline at (`x`, `baseline`), `tracking` px between glyphs.
    fn draw_text(
        &mut self,
        f: &FontRef,
        text: &str,
        x: f32,
        baseline: f32,
        px: f32,
        color: [u8; 3],
        tracking: f32,
    ) {
        let scaled = f.as_scaled(PxScale::from(px));
        let mut pen = x;
        for ch in text.chars() {
            let gid = f.glyph_id(ch);
            let glyph = gid.with_scale_and_position(PxScale::from(px), point(pen, baseline));
            if let Some(outline) = f.outline_glyph(glyph) {
                let bb = outline.px_bounds();
                outline.draw(|gx, gy, cov| {
                    self.blend(
                        bb.min.x as i32 + gx as i32,
                        bb.min.y as i32 + gy as i32,
                        color,
                        cov,
                    );
                });
            }
            pen += scaled.h_advance(gid) + tracking;
        }
    }
}

/// Host portion of `url:` (`https://ex.com/blog/` -> `ex.com`).
fn host_of(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    after_scheme
        .split('/')
        .next()
        .unwrap_or(after_scheme)
        .trim_end_matches('/')
        .to_string()
}

/// Derive a page's card text: home (has a `hero:`) uses the hero; a post (`date:`)
/// uses its title + first category (else date); any page falls back to the site title.
pub fn card_spec(site: &Site, page: &Page) -> CardSpec {
    let site_title = site.config.title.clone().unwrap_or_default();
    let domain = site.config.url.as_deref().map(host_of);
    if let Some(hero) = page.hero.as_ref() {
        return CardSpec {
            eyebrow: hero.eyebrow.clone(),
            headline: hero
                .headline
                .clone()
                .or_else(|| page.title.clone())
                .unwrap_or_else(|| site_title.clone()),
            lead: hero
                .lead
                .clone()
                .or_else(|| site.config.description.clone()),
            footer_wordmark: site_title,
            domain,
        };
    }
    let eyebrow = if page.date.is_some() {
        page.categories
            .first()
            .cloned()
            .or_else(|| page.date.clone())
    } else {
        None
    };
    CardSpec {
        eyebrow,
        headline: page.title.clone().unwrap_or_else(|| site_title.clone()),
        lead: page.description.clone(),
        footer_wordmark: site_title,
        domain,
    }
}

/// Deterministic content key: design version + every spec field + a font tag.
fn spec_key(spec: &CardSpec) -> String {
    format!(
        "v{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}newsreader",
        CARD_DESIGN_VERSION,
        spec.eyebrow.as_deref().unwrap_or(""),
        spec.headline,
        spec.lead.as_deref().unwrap_or(""),
        spec.footer_wordmark,
        spec.domain.as_deref().unwrap_or(""),
    )
}

/// Site-root-relative card file path, e.g. `og/1a2b3c4d5e6f7a8b.png`. Same hash the
/// build writes and `card_url` serves, so the URL and the file always agree.
pub fn card_rel_path(spec: &CardSpec) -> String {
    format!("og/{:016x}.{}", fnv1a(&spec_key(spec)), CARD_EXT)
}

/// The page's card URL (`/og/<hash>.png`), or `None` when `_site.yml` has no `url:`.
#[allow(dead_code)] // wired into og:image/twitter:image/JSON-LD by Task 5
pub(crate) fn card_url(site: &Site, page: &Page) -> Option<String> {
    site.config.url.as_ref()?;
    Some(format!("/{}", card_rel_path(&card_spec(site, page))))
}

/// Render `spec` onto a 1200x630 dark card and return the encoded PNG bytes.
pub fn render_card(spec: &CardSpec) -> Vec<u8> {
    let f = font();
    let mut c = Canvas::new(CARD_W, CARD_H, BG);
    let pad = 72.0_f32;
    let max_w = CARD_W as f32 - pad * 2.0;

    // Eyebrow: small caps, letter-spaced, muted.
    if let Some(eb) = spec.eyebrow.as_deref().filter(|s| !s.is_empty()) {
        c.draw_text(&f, &eb.to_uppercase(), pad, 150.0, 28.0, MUTED, 3.0);
    }

    // Headline: large fg serif, up to 3 lines, one shrink step if it overflows.
    let mut size = 76.0_f32;
    let mut lines = wrap(&f, &spec.headline, size, max_w);
    if lines.len() > 3 {
        size = 60.0;
        lines = wrap(&f, &spec.headline, size, max_w);
    }
    lines.truncate(3);
    let line_h = size * 1.18;
    let mut y = 214.0 + size;
    for line in &lines {
        c.draw_text(&f, line, pad, y, size, FG, 0.0);
        y += line_h;
    }

    // Lead: muted, up to 2 lines, ellipsized.
    if let Some(lead) = spec.lead.as_deref().filter(|s| !s.is_empty()) {
        let lp = 34.0_f32;
        let mut ly = y + 24.0;
        for line in wrap_clamp(&f, lead, lp, max_w, 2) {
            c.draw_text(&f, &line, pad, ly, lp, MUTED, 0.0);
            ly += lp * 1.3;
        }
    }

    // Footer: hairline rule, bell-curve mark + wordmark (left), domain (right).
    c.fill_rect(pad as i32, 540, (CARD_W as f32 - pad) as i32, 542, BORDER);
    let foot = 588.0_f32;
    // Bell-curve mark (gaussian), ~52px wide, sitting on a short baseline.
    let mark_x = pad;
    let mark_w = 52.0;
    let mark_h = 26.0;
    let n = 48;
    let curve: Vec<(f32, f32)> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = (t - 0.5) * 6.0;
            (
                mark_x + t * mark_w,
                (foot - 6.0) - (-(u * u) / 2.0).exp() * mark_h,
            )
        })
        .collect();
    c.stroke_polyline(&curve, 4.0, ACCENT);
    c.stroke_polyline(
        &[(mark_x, foot - 6.0), (mark_x + mark_w, foot - 6.0)],
        3.0,
        ACCENT,
    );
    // Wordmark after the mark.
    if !spec.footer_wordmark.is_empty() {
        c.draw_text(
            &f,
            &spec.footer_wordmark,
            mark_x + mark_w + 22.0,
            foot,
            30.0,
            FG,
            0.0,
        );
    }
    // Domain, right-aligned.
    if let Some(dom) = spec.domain.as_deref().filter(|s| !s.is_empty()) {
        let w = text_width(&f, dom, 28.0, 0.0);
        c.draw_text(&f, dom, CARD_W as f32 - pad - w, foot, 28.0, MUTED, 0.0);
    }

    c.into_png()
}

fn dist_pt_seg(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (px, py) = p;
    let (ax, ay) = a;
    let (bx, by) = b;
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= 0.0 {
        0.0
    } else {
        (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
    };
    let (cx, cy) = (ax + t * dx, ay + t * dy);
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::{Site, tests::write_site};

    fn sample() -> CardSpec {
        CardSpec {
            eyebrow: Some("Statistics".into()),
            headline: "The EM algorithm".into(),
            lead: Some("A worked derivation.".into()),
            footer_wordmark: "Andreas Bogossian".into(),
            domain: Some("andreasbogossian.com".into()),
        }
    }

    /// A PNG's IHDR carries width/height as big-endian u32 at byte offsets 16 and 20
    /// (8-byte signature + 4-byte length + "IHDR"). Read them without a decoder dep.
    fn png_dims(bytes: &[u8]) -> (u32, u32) {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        (w, h)
    }

    #[test]
    fn render_card_emits_a_1200x630_png() {
        let png = render_card(&sample());
        assert_eq!(&png[1..4], b"PNG", "PNG signature");
        assert_eq!(png_dims(&png), (CARD_W, CARD_H));
    }

    #[test]
    fn render_card_is_deterministic() {
        assert_eq!(render_card(&sample()), render_card(&sample()));
    }

    #[test]
    fn stroke_marks_the_line_and_spares_far_pixels() {
        let mut c = Canvas::new(40, 40, BG);
        c.stroke_polyline(&[(2.0, 20.0), (38.0, 20.0)], 3.0, FG);
        // A pixel on the line is lightened away from bg; a far corner stays bg.
        let on = ((20u32 * 40 + 20) * 4) as usize;
        let far = ((2u32 * 40 + 2) * 4) as usize;
        assert!(c.px[on] > BG[0], "on-line pixel drawn");
        assert_eq!(c.px[far], BG[0], "far pixel untouched");
    }

    #[test]
    fn text_width_is_zero_empty_and_grows_with_length() {
        let f = font();
        assert_eq!(text_width(&f, "", 40.0, 0.0), 0.0);
        assert!(text_width(&f, "wwww", 40.0, 0.0) > text_width(&f, "w", 40.0, 0.0));
    }

    #[test]
    fn wrap_keeps_every_line_within_max_width() {
        let f = font();
        let text = "the expectation maximization algorithm derived from first principles";
        let max = 300.0;
        let lines = wrap(&f, text, 40.0, max);
        assert!(lines.len() > 1, "long text wraps to multiple lines");
        for line in &lines {
            assert!(text_width(&f, line, 40.0, 0.0) <= max, "line {line:?} fits");
        }
    }

    #[test]
    fn truncate_line_adds_ellipsis_only_when_overflowing() {
        let f = font();
        assert_eq!(truncate_line(&f, "hi", 40.0, 1000.0), "hi");
        let t = truncate_line(&f, "a very long line of words here", 40.0, 120.0);
        assert!(t.ends_with('\u{2026}'), "overflow gets an ellipsis: {t:?}");
        assert!(
            text_width(&f, &t, 40.0, 0.0) <= 120.0,
            "truncated line fits"
        );
    }

    #[test]
    fn wrap_clamp_limits_lines_ellipsizes_on_cut_and_survives_zero() {
        let f = font();
        let text = "the expectation maximization algorithm derived from first principles at length";
        let two = wrap_clamp(&f, text, 40.0, 200.0, 2);
        assert!(two.len() <= 2, "clamped to <=2 lines");
        assert!(
            two.last().unwrap().ends_with('\u{2026}'),
            "last line ellipsized when content cut"
        );
        assert_eq!(
            wrap_clamp(&f, "two words", 40.0, 1000.0, 2),
            vec!["two words".to_string()]
        );
        assert!(
            wrap_clamp(&f, "hello", 40.0, 300.0, 0).is_empty(),
            "max_lines==0 -> empty, no panic"
        );
    }

    #[test]
    fn wrap_keeps_an_overlong_word_on_its_own_line() {
        let f = font();
        let lines = wrap(&f, "supercalifragilisticexpialidocious", 40.0, 50.0);
        assert_eq!(
            lines.len(),
            1,
            "an over-long single word is one line, not dropped"
        );
        assert!(lines[0].contains("supercalifragilistic"));
    }

    #[test]
    fn draw_text_marks_at_least_one_pixel() {
        let mut c = Canvas::new(200, 80, BG);
        c.draw_text(&font(), "Ag", 10.0, 55.0, 48.0, FG, 0.0);
        assert!(
            c.px.chunks(4).any(|p| p[0] != BG[0]),
            "some glyph pixels drawn"
        );
    }

    #[test]
    fn card_spec_home_uses_the_hero() {
        let root = write_site(
            "cardhome",
            &[
                (
                    "_site.yml",
                    "title: Andreas Bogossian\nurl: https://ex.com\n",
                ),
                (
                    "index.tmd",
                    "---\nhero:\n  eyebrow: Writing\n  headline: First principles\n  lead: Machine learning, worked out.\n---\n\nHi.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let home = site.pages.iter().find(|p| p.url == "index.html").unwrap();
        let spec = card_spec(&site, home);
        assert_eq!(spec.eyebrow.as_deref(), Some("Writing"));
        assert_eq!(spec.headline, "First principles");
        assert_eq!(spec.lead.as_deref(), Some("Machine learning, worked out."));
        assert_eq!(spec.domain.as_deref(), Some("ex.com"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn card_spec_post_uses_title_and_first_category() {
        let root = write_site(
            "cardpost",
            &[
                ("_site.yml", "title: Blog\nurl: https://ex.com\n"),
                (
                    "posts/a/index.tmd",
                    "---\ntitle: The EM algorithm\ndate: 2026-05-15\ndescription: A derivation.\ncategories: [Statistics, ML]\n---\n\nx\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let post = site
            .pages
            .iter()
            .find(|p| p.url.contains("posts/a"))
            .unwrap();
        let spec = card_spec(&site, post);
        assert_eq!(spec.headline, "The EM algorithm");
        assert_eq!(spec.eyebrow.as_deref(), Some("Statistics"));
        assert_eq!(spec.lead.as_deref(), Some("A derivation."));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn card_url_gated_on_site_url_and_names_a_png() {
        let with = write_site(
            "cardurly",
            &[
                ("_site.yml", "title: B\nurl: https://ex.com\n"),
                ("index.tmd", "---\ntitle: H\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&with);
        let home = site.pages.iter().find(|p| p.url == "index.html").unwrap();
        let url = card_url(&site, home).expect("url set -> Some");
        assert!(
            url.starts_with("/og/") && url.ends_with(".png"),
            "got {url}"
        );
        assert_eq!(format!("/{}", card_rel_path(&card_spec(&site, home))), url);
        let _ = std::fs::remove_dir_all(&with);

        let without = write_site(
            "cardnourl",
            &[
                ("_site.yml", "title: B\n"),
                ("index.tmd", "---\ntitle: H\n---\n\nx\n"),
            ],
        );
        let site2 = Site::discover(&without);
        let home2 = site2.pages.iter().find(|p| p.url == "index.html").unwrap();
        assert!(card_url(&site2, home2).is_none(), "no url -> None");
        let _ = std::fs::remove_dir_all(&without);
    }

    #[test]
    fn render_card_survives_empty_and_overlong_text() {
        let long = "word ".repeat(80);
        for spec in [
            CardSpec {
                eyebrow: None,
                headline: String::new(),
                lead: None,
                footer_wordmark: String::new(),
                domain: None,
            },
            CardSpec {
                eyebrow: Some(long.clone()),
                headline: long.clone(),
                lead: Some(long),
                footer_wordmark: "W".into(),
                domain: Some("x.com".into()),
            },
        ] {
            let png = render_card(&spec);
            assert_eq!(png_dims(&png), (CARD_W, CARD_H));
        }
    }
}
