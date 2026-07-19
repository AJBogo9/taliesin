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

/// Greedy word-wrap so each line's width is <= `max_w`. A single over-long word keeps its
/// own line rather than being dropped, but is ellipsized to fit: the `cur.is_empty()` guard
/// below admits ANY first word regardless of width, which used to be the whole story, so
/// `NullPointerExceptionHandlerFactory` sailed out past the card's pad edge and clipped
/// mid-glyph. The line-count clamp above this can't catch that — one long word is one line,
/// and never trips a limit expressed in lines.
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
    // Only a lone over-long word can still overflow (every other line was admitted by the
    // width test above), and only ever by ellipsizing it — never by dropping the line.
    for line in &mut lines {
        if text_width(f, line, px, 0.0) > max_w {
            *line = truncate_line(f, line, px, max_w, 0.0);
        }
    }
    lines
}

/// Truncate a single line to `max_w` with a trailing ellipsis when it overflows.
/// `tracking` must match what the caller will DRAW with, or the fit is measured against a
/// different string than the one that lands on the canvas (the eyebrow draws at 3.0).
fn truncate_line(f: &FontRef, text: &str, px: f32, max_w: f32, tracking: f32) -> String {
    if text_width(f, text, px, tracking) <= max_w {
        return text.to_string();
    }
    let mut s = String::new();
    for ch in text.chars() {
        if text_width(f, &format!("{s}{ch}\u{2026}"), px, tracking) > max_w {
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
        lines.push(truncate_line(
            f,
            &format!("{last} \u{2026}"),
            px,
            max_w,
            0.0,
        ));
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
pub(crate) fn card_url(site: &Site, page: &Page) -> Option<String> {
    // The 404 page is not scraped content and the build's card-emit loop skips it
    // (`build.rs`, `page.url == "404.html"`), so its og:image/twitter:image tags must not
    // reference a card that is never written. Keep this exclusion in sync with that skip.
    if page.url == "404.html" {
        return None;
    }
    site.config.url.as_ref()?;
    Some(format!("/{}", card_rel_path(&card_spec(site, page))))
}

/// Characters in `spec`'s text that the bundled font has no glyph for, first-seen order,
/// deduped. Each renders as a `.notdef` tofu box with a non-zero advance, so the card
/// lays out and encodes "successfully" while reading as garbage to every crawler that
/// sees it. Nobody looks at a social card, so this cannot be caught by eye.
///
/// The font is Latin/Latin-ext/Vietnamese (658 glyphs), which makes the realistic
/// casualties Greek/Cyrillic/CJK/emoji and, for a maths-heavy author, `∑ ∫ ∞`.
pub fn uncovered_glyphs(spec: &CardSpec) -> Vec<char> {
    let f = font();
    let mut out: Vec<char> = Vec::new();
    let fields = [
        Some(spec.headline.clone()),
        // Checked as DRAWN: `render_card` uppercases the eyebrow, and only the eyebrow.
        // Uppercasing the rest would cry wolf over a char that is covered in the form it
        // actually renders in.
        spec.eyebrow.as_deref().map(str::to_uppercase),
        spec.lead.clone(),
        Some(spec.footer_wordmark.clone()),
        spec.domain.clone(),
    ];
    for text in fields.iter().flatten() {
        for ch in text.chars() {
            // Whitespace has no outline by design and is never .notdef-substituted.
            if ch.is_whitespace() || out.contains(&ch) {
                continue;
            }
            if f.glyph_id(ch).0 == 0 {
                out.push(ch);
            }
        }
    }
    out
}

/// Lay the headline out: up to 3 lines at 76px, with one shrink step to 60px if it
/// overflows. Returns the lines and the chosen size.
///
/// Split out of `render_card` so the overflow behaviour is assertable at all: the render
/// path's only output is a PNG, and nothing decodes one, so `render_card`'s own
/// overlong-text test can (and did) pass while the headline was silently cut.
fn headline_layout(f: &FontRef, headline: &str, max_w: f32) -> (Vec<String>, f32) {
    let mut hsize = 76.0_f32;
    if wrap(f, headline, hsize, max_w).len() > 3 {
        hsize = 60.0;
    }
    // `wrap_clamp`, not `truncate`: a bare truncate leaves a complete-looking but wrong
    // headline with no signal, and the lead beside it already ellipsizes this way.
    (wrap_clamp(f, headline, hsize, max_w, 3), hsize)
}

/// The card's margin: no ink may fall outside `[PAD, CARD_W - PAD]`.
const PAD: f32 = 72.0;
const MARK_W: f32 = 52.0;
/// Where the footer wordmark starts (after the bell-curve mark).
const WORD_X: f32 = PAD + MARK_W + 22.0;
const WORD_SIZE: f32 = 30.0;
const DOM_SIZE: f32 = 28.0;
/// The least breathing room allowed between the wordmark and the domain.
const FOOT_GAP: f32 = 24.0;

/// The footer domain, fitted. Right-aligned as `CARD_W - PAD - width`, so an unbounded
/// width drives its own x NEGATIVE and clips off the left edge — the cap is what stops
/// that. It keeps its natural width otherwise: a domain is short by nature, and the cap is
/// a backstop, not a layout rule.
fn footer_domain_fitted(f: &FontRef, spec: &CardSpec) -> Option<String> {
    spec.domain
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|d| truncate_line(f, d, DOM_SIZE, (CARD_W as f32 - PAD * 2.0) * 0.45, 0.0))
}

/// The footer wordmark, fitted to whatever the (right-aligned) domain leaves of the row.
/// Clamping each field to the pad box independently is NOT enough here: these two share one
/// line, so a long site title slid under the domain and rendered "Learnindgsbogossian.com"
/// while both still sat inside the box. The domain wins the tie — it is the shorter, more
/// load-bearing identity, and it is what a reader uses to place the card.
fn footer_wordmark_fitted(f: &FontRef, spec: &CardSpec) -> String {
    let dom_w = footer_domain_fitted(f, spec).map_or(0.0, |d| text_width(f, &d, DOM_SIZE, 0.0));
    let avail = (CARD_W as f32 - PAD - dom_w - FOOT_GAP) - WORD_X;
    truncate_line(f, &spec.footer_wordmark, WORD_SIZE, avail.max(0.0), 0.0)
}

/// Render `spec` onto a 1200x630 dark card and return the encoded PNG bytes.
pub fn render_card(spec: &CardSpec) -> Vec<u8> {
    let f = font();
    let mut c = Canvas::new(CARD_W, CARD_H, BG);
    let pad = 72.0_f32;
    let max_w = CARD_W as f32 - pad * 2.0;

    // Vertical layout: measure the eyebrow + headline + lead block and center it in the
    // region between the top margin and the footer rule (y=540), so a tall 2-line
    // headline + 2-line lead never collides with the rule and a sparse card stays
    // balanced. Baselines are approximated as 0.76*size below each line's top; the
    // clearance to the rule stays >40px in the worst case, so the approximation is safe.
    const REGION_TOP: f32 = 96.0;
    const RULE_Y: f32 = 540.0;
    const EYE_SIZE: f32 = 28.0;
    const LEAD_SIZE: f32 = 34.0;
    const EYE_GAP: f32 = 26.0; // eyebrow box -> headline
    const HEAD_GAP: f32 = 30.0; // headline -> lead

    // One line, so it truncates rather than wraps — and at the tracking it is DRAWN with
    // (3.0), or the fit is measured against a narrower string than the one that lands.
    // It had no width check at all and simply ran off the right edge.
    let eyebrow = spec
        .eyebrow
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| truncate_line(&f, &s.to_uppercase(), EYE_SIZE, max_w, 3.0));

    // Headline: large fg serif, up to 3 lines, one shrink step if it overflows.
    let (hlines, hsize) = headline_layout(&f, &spec.headline, max_w);
    let head_lh = hsize * 1.16;

    // Lead: muted, up to 2 lines, ellipsized.
    let lead_lines: Vec<String> = spec
        .lead
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|lead| wrap_clamp(&f, lead, LEAD_SIZE, max_w, 2))
        .unwrap_or_default();
    let lead_lh = LEAD_SIZE * 1.32;

    let eye_h = if eyebrow.is_some() {
        EYE_SIZE + EYE_GAP
    } else {
        0.0
    };
    let lead_h = if lead_lines.is_empty() {
        0.0
    } else {
        HEAD_GAP + lead_lines.len() as f32 * lead_lh
    };
    let block_h = eye_h + hlines.len() as f32 * head_lh + lead_h;

    // `y` tracks the TOP of the current line box; baseline = y + 0.76*size.
    let mut y = (REGION_TOP + ((RULE_Y - REGION_TOP) - block_h) / 2.0).max(REGION_TOP);

    if let Some(eb) = &eyebrow {
        c.draw_text(&f, eb, pad, y + EYE_SIZE * 0.76, EYE_SIZE, MUTED, 3.0);
        y += EYE_SIZE + EYE_GAP;
    }
    for line in &hlines {
        c.draw_text(&f, line, pad, y + hsize * 0.76, hsize, FG, 0.0);
        y += head_lh;
    }
    if !lead_lines.is_empty() {
        y += HEAD_GAP;
        for line in &lead_lines {
            c.draw_text(&f, line, pad, y + LEAD_SIZE * 0.76, LEAD_SIZE, MUTED, 0.0);
            y += lead_lh;
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
    // Wordmark after the mark, fitted to what the domain leaves of the row.
    let wordmark = footer_wordmark_fitted(&f, spec);
    if !wordmark.is_empty() {
        c.draw_text(&f, &wordmark, WORD_X, foot, WORD_SIZE, FG, 0.0);
    }
    // Domain, right-aligned.
    if let Some(dom) = footer_domain_fitted(&f, spec) {
        let w = text_width(&f, &dom, DOM_SIZE, 0.0);
        c.draw_text(
            &f,
            &dom,
            CARD_W as f32 - pad - w,
            foot,
            DOM_SIZE,
            MUTED,
            0.0,
        );
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

    /// The OG card is composited at build time (Rust -> PNG), so it can't read the CSS
    /// tokens at runtime: its colours are the consts above. The card renders on the dark
    /// brand surface, so drift-lock BG/FG/ACCENT/MUTED/BORDER to the dark palette in
    /// tokens-dark.css. If a dark token moves, this fails loudly instead of the card
    /// silently drifting off-brand (the same anti-drift idiom as schema.rs/third_party.rs).
    #[test]
    fn card_palette_tracks_the_dark_tokens() {
        fn token_rgb(name: &str) -> [u8; 3] {
            let css = crate::render::TOKENS_DARK_CSS;
            let i = css
                .find(&format!("{name}:"))
                .unwrap_or_else(|| panic!("no `{name}:` in tokens-dark.css"));
            let rest = &css[i + name.len() + 1..];
            let h = rest.find('#').expect("a hex colour after the token");
            let hex = &rest[h + 1..h + 7];
            let ch = |a: usize| u8::from_str_radix(&hex[a..a + 2], 16).unwrap();
            [ch(0), ch(2), ch(4)]
        }
        for (name, konst) in [
            ("--tali-bg", BG),
            ("--tali-fg", FG),
            ("--tali-muted", MUTED),
            ("--tali-accent", ACCENT),
            ("--tali-border", BORDER),
        ] {
            assert_eq!(
                token_rgb(name),
                konst,
                "card.rs {name} const drifted from tokens-dark.css"
            );
        }
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
        assert_eq!(truncate_line(&f, "hi", 40.0, 1000.0, 0.0), "hi");
        let t = truncate_line(&f, "a very long line of words here", 40.0, 120.0, 0.0);
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

    /// An over-long single word keeps its own line and is never dropped — **and it fits**.
    /// This used to assert only the first half (`contains("supercalifragilistic")` against a
    /// 50px budget), which does not merely miss the bug, it PINS it: `wrap`'s `cur.is_empty()`
    /// guard admits any first word at any width, and the clamp above it is expressed in LINES,
    /// which one long word never trips. So the word ran past the card's pad edge and clipped
    /// mid-glyph, with a green test asserting the overflowing text was present.
    #[test]
    fn wrap_keeps_an_overlong_word_on_its_own_line_and_fits_it() {
        let f = font();
        let max = 200.0;
        let lines = wrap(&f, "supercalifragilisticexpialidocious", 40.0, max);
        assert_eq!(
            lines.len(),
            1,
            "an over-long single word is one line, not dropped"
        );
        assert!(
            text_width(&f, &lines[0], 40.0, 0.0) <= max,
            "and it fits inside max_w: {:?}",
            lines[0]
        );
        assert!(
            lines[0].starts_with("sup"),
            "kept from the start: {:?}",
            lines[0]
        );
        assert!(
            lines[0].ends_with('\u{2026}'),
            "ellipsized, so the cut is visible rather than a silent clip: {:?}",
            lines[0]
        );
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
    fn uncovered_glyphs_reports_what_the_font_cannot_draw() {
        // The bundled Newsreader is Latin/Latin-ext/Vietnamese only. A char it lacks is
        // drawn as .notdef — a tofu box with a NON-ZERO advance, so layout "succeeds" and
        // nothing errors. Verified by rendering: a title of "Deriving ∑ log p(x) and the
        // ∫ bound" produces a card reading "Deriving ▯ log p(x) and the ▯ bound", with the
        // build reporting success. Math symbols are the realistic trigger for this author.
        let spec = |headline: &str| CardSpec {
            eyebrow: None,
            headline: headline.into(),
            lead: None,
            footer_wordmark: "W".into(),
            domain: None,
        };
        assert_eq!(
            uncovered_glyphs(&spec("Deriving ∑ log p(x) and the ∫ bound")),
            vec!['∑', '∫']
        );
        // Latin, punctuation and the ellipsis we now emit ourselves must all be covered,
        // or the diagnostic cries wolf on every card.
        assert!(uncovered_glyphs(&spec("The EM-algorithm: a “deep” dive… (2026)")).is_empty());
        // Every text field is checked, not just the headline.
        let mut s = spec("Fine");
        s.lead = Some("Приветствие".into());
        assert!(!uncovered_glyphs(&s).is_empty(), "the lead is checked too");
        // Each missing char is reported ONCE, in first-seen order.
        assert_eq!(uncovered_glyphs(&spec("∑ ∑ ∫ ∑")), vec!['∑', '∫']);
    }

    #[test]
    fn an_overlong_headline_is_ellipsized_not_silently_cut() {
        let f = font();
        let max_w = CARD_W as f32 - 72.0 * 2.0;
        // A headline too long even at the 60px shrink step. Cutting it without an
        // ellipsis yields a COMPLETE-LOOKING but wrong headline on the social card, with
        // no signal to anyone: the card is only ever read by crawlers. The `lead` already
        // ellipsizes correctly via wrap_clamp in this same file.
        let long = "Deriving the Kruskal-Wallis test carefully from first principles \
                    with many additional qualifying words that simply keep going and going";
        let (lines, size) = headline_layout(&f, long, max_w);
        assert!(lines.len() <= 3, "clamped to 3 lines: {lines:?}");
        assert_eq!(size, 60.0, "and took the shrink step first");
        assert!(
            lines.last().expect("a line").ends_with('\u{2026}'),
            "the cut must be visible: {lines:?}"
        );
        // A headline that FITS must not gain a spurious ellipsis.
        let (short, _) = headline_layout(&f, "A Short Title", max_w);
        assert_eq!(short, vec!["A Short Title".to_string()]);
    }

    /// Decode a rendered card and return the x of the leftmost and rightmost inked
    /// (non-background) pixel — "did anything escape the pad box", asked of the actual
    /// image rather than of the layout maths that is supposed to prevent it.
    fn ink_x_range(png: &[u8]) -> Option<(u32, u32)> {
        let dec = png::Decoder::new(png);
        let mut reader = dec.read_info().expect("decodable png");
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("frame");
        let (mut lo, mut hi) = (u32::MAX, 0u32);
        for y in 0..info.height {
            for x in 0..info.width {
                let i = ((y * info.width + x) * 4) as usize;
                // Tolerate anti-aliasing: only count a pixel that is meaningfully inked.
                let d = (buf[i] as i32 - BG[0] as i32).abs()
                    + (buf[i + 1] as i32 - BG[1] as i32).abs()
                    + (buf[i + 2] as i32 - BG[2] as i32).abs();
                if d > 24 {
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
            }
        }
        (lo != u32::MAX).then_some((lo, hi))
    }

    /// Every field must stay inside the 72px pad box. The eyebrow, wordmark and domain got
    /// no width check at all, so a long site title ran the wordmark under the right-aligned
    /// domain, and a long domain (right-aligned as `CARD_W - pad - width`) drove x NEGATIVE
    /// and clipped off the left edge. `render_card_survives_empty_and_overlong_text` covers
    /// these same fields and passes, because it only asserts the PNG's dimensions — a card
    /// whose text runs off the edge is still 1200x630.
    #[test]
    fn no_field_draws_outside_the_pad_box() {
        let pad = 72u32;
        // A sane card is the control: if this fails the assertion itself is wrong.
        let (lo, hi) = ink_x_range(&render_card(&sample())).expect("sample card has ink");
        assert!(
            lo + 2 >= pad && hi <= CARD_W - pad + 2,
            "the sample card already escapes the pad box: ink x {lo}..{hi}, box {pad}..{}",
            CARD_W - pad
        );

        for (what, spec) in [
            (
                "eyebrow",
                CardSpec {
                    eyebrow: Some("a very long kicker that keeps going and going".repeat(2)),
                    ..sample()
                },
            ),
            (
                "wordmark",
                CardSpec {
                    footer_wordmark: "Learnings from a Very Long Site Title Indeed".into(),
                    ..sample()
                },
            ),
            (
                "domain",
                CardSpec {
                    domain: Some(
                        "an-extremely-long-domain-name-that-cannot-fit.example.com".into(),
                    ),
                    ..sample()
                },
            ),
            (
                "headline single word",
                CardSpec {
                    headline: "NullPointerExceptionHandlerFactoryProviderStrategyDelegate".into(),
                    ..sample()
                },
            ),
            (
                // The lead line was the one text field this case list omitted, so a lead
                // wrapped at the wrong (full-canvas instead of pad-box) width ran off the
                // right edge unnoticed — the only test rendering a long lead asserts PNG
                // dimensions only.
                "lead",
                CardSpec {
                    lead: Some(
                        "a lead paragraph deliberately far too long to sit on one line, which \
                         must wrap and clamp inside the pad box instead of running past the \
                         right edge of the card"
                            .repeat(2),
                    ),
                    ..sample()
                },
            ),
        ] {
            let (lo, hi) = ink_x_range(&render_card(&spec)).expect("card has ink");
            assert!(
                lo + 2 >= pad,
                "{what}: ink starts at x={lo}, left of the {pad}px pad edge"
            );
            assert!(
                hi <= CARD_W - pad + 2,
                "{what}: ink reaches x={hi}, past the {} pad edge",
                CARD_W - pad
            );
        }
    }

    /// The wordmark and the domain share the footer row, so clamping each to the pad box
    /// is not enough: they must not collide with each other either.
    #[test]
    fn a_long_wordmark_does_not_run_into_the_domain() {
        let f = font();
        // Long enough to genuinely reach the domain: a merely longish wordmark stays inside
        // the pad box AND clear of the domain, so it proves nothing.
        let spec = CardSpec {
            footer_wordmark: "Learnings from a Very Long Site Title That Simply Keeps Going".into(),
            domain: Some("andreasbogossian.com".into()),
            ..sample()
        };
        // The unfitted string must actually collide, or this test would pass on a bug.
        let dom = footer_domain_fitted(&f, &spec).unwrap();
        let dom_x = CARD_W as f32 - PAD - text_width(&f, &dom, DOM_SIZE, 0.0);
        let raw_end = WORD_X + text_width(&f, &spec.footer_wordmark, WORD_SIZE, 0.0);
        assert!(
            raw_end > dom_x,
            "fixture is too short to collide (raw ends {raw_end}, domain starts {dom_x})"
        );
        let word_end = WORD_X + text_width(&f, &footer_wordmark_fitted(&f, &spec), WORD_SIZE, 0.0);
        assert!(
            word_end <= dom_x - FOOT_GAP + 1.0,
            "wordmark ends at {word_end}, domain starts at {dom_x}: they collide"
        );
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
