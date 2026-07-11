//! OpenGraph social-card generation: a branded 1200x630 PNG per content page,
//! hand-composited (no headless browser, no CDN). Deterministic: the same `CardSpec`
//! renders byte-identical output. Text uses the bundled Newsreader variable font's
//! default (Regular) instance; hierarchy is size + color.

pub const CARD_W: u32 = 1200;
pub const CARD_H: u32 = 630;
/// Bumped when the template changes, to cache-bust every card URL.
pub const CARD_DESIGN_VERSION: u32 = 1;
/// Encoded card format extension (see the plan's Global Constraints).
pub const CARD_EXT: &str = "png";

// Task 1 bundles the font and palette but only Task 4 draws text/borders with them;
// silence dead_code until then rather than trimming what later tasks need verbatim.
#[allow(dead_code)]
const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/Newsreader[opsz,wght].ttf");

const BG: [u8; 3] = [22, 24, 29];
#[allow(dead_code)]
const FG: [u8; 3] = [230, 230, 230];
#[allow(dead_code)]
const MUTED: [u8; 3] = [154, 160, 170];
#[allow(dead_code)]
const ACCENT: [u8; 3] = [154, 168, 220];
#[allow(dead_code)]
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

    #[allow(dead_code)] // wired up from Task 4 onward (fill_rect + glyph drawing)
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

    #[allow(dead_code)] // wired up from Task 4 onward
    fn fill_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 3]) {
        for y in y0.max(0)..y1.min(self.h as i32) {
            for x in x0.max(0)..x1.min(self.w as i32) {
                self.blend(x, y, color, 1.0);
            }
        }
    }

    #[allow(dead_code)] // wired up from Task 4 onward (bell-curve mark)
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

/// Render `spec` onto a 1200x630 dark card and return the encoded PNG bytes.
/// Task 1 fills only the background; Task 4 composes the full card.
pub fn render_card(spec: &CardSpec) -> Vec<u8> {
    let _ = spec; // used from Task 4 onward
    let canvas = Canvas::new(CARD_W, CARD_H, BG);
    canvas.into_png()
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
}
