# OpenGraph Card Generation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** At build time, auto-generate a branded 1200×630 OpenGraph card per content page (url-gated) and point `og:image` / `twitter:image` / JSON-LD `image` at it.

**Architecture:** A pure generator in `taliesin-core` (`site/card.rs`) hand-composites the card into an RGBA8 canvas (`ab_glyph` for text, procedural drawing for the bell-curve mark) and PNG-encodes it. `card_url` names a deterministic `/og/<fnv1a-hash>.png` from the page's `CardSpec`. The build writes one file per page in its url-gated aux zone; the preview serves them lazily. `meta.rs` swaps its image source to `card_url`.

**Tech Stack:** Rust (edition 2024), `ab_glyph` (glyph rasterization, pure-Rust), `png` (encode, pure-Rust), the bundled Newsreader variable font (OFL).

## Global Constraints

- Rust edition 2024, workspace resolver 3. New deps go in root `[workspace.dependencies]` and are referenced with `{ workspace = true }`.
- **Only two new deps: `ab_glyph` and `png`.** Both pure-Rust, zero C. Do NOT add `image`, `resvg`, `usvg`, `tiny-skia`, `fontdb`, or any libwebp binding.
- **Offline + deterministic.** No network at render/build time (the font is `include_bytes!`-bundled). The same `CardSpec` MUST produce byte-identical output (reproducible build + stable stale-sweep).
- **Encoder is PNG**, RGBA8. `CARD_EXT = "png"`; `og:image` resolves to `/og/<hash>.png`. (WebP is a deferred optional encoder swap, out of scope here.)
- **Font: bundle the Newsreader variable font** (`Newsreader[opsz,wght].ttf`, OFL) and render its **default (Regular) instance** for all text; build hierarchy via size + color only (ab_glyph cannot set the weight axis of a variable font). `THIRD_PARTY.md` must list Newsreader (OFL), `ab_glyph`, and `png` — the `crates/core/tests/third_party.rs` pin enforces presence.
- **Palette — dark `--tali-*` tokens only** (exact RGB): bg `#16181d`=(22,24,29); fg `#e6e6e6`=(230,230,230); muted `#9aa0aa`=(154,160,170); accent `#9aa8dc`=(154,168,220); border `#363a44`=(54,58,68).
- **Card is 1200×630, always dark** (a single static asset; no light variant).
- **Url-gated:** cards only when `_site.yml` sets `url:` (same gate as the SEO sidecars + JSON-LD).
- **Do-NOT-touch:** the exec/kernel zone; the single-editing-surface invariant (cards are a build/serve artifact, never a source write-back).
- The generated card drives the **social image only**. `image:` front-matter stays the in-page/listing thumbnail and MUST NOT be repurposed.

---

### Task 1: Deps, bundled font, module skeleton, blank card + PNG encode

**Files:**
- Modify: `Cargo.toml` (root `[workspace.dependencies]`)
- Modify: `crates/core/Cargo.toml` (`[dependencies]`)
- Create: `crates/core/assets/fonts/Newsreader[opsz,wght].ttf`, `crates/core/assets/fonts/OFL.txt`
- Create: `crates/core/src/site/card.rs`
- Modify: `crates/core/src/site/mod.rs` (register `mod card;` + re-exports)

**Interfaces:**
- Produces: `taliesin_core::site::{CardSpec, render_card, CARD_W, CARD_H, CARD_EXT, CARD_DESIGN_VERSION}`; a private `Canvas` with `new`/`blend`/`fill_rect`/`into_png`.

- [ ] **Step 1: Add the dependencies**

Root `Cargo.toml`, under `[workspace.dependencies]` (after `libc = "0.2"`):

```toml
# OG social-card rendering (crates/core/src/site/card.rs). Both pure-Rust, no C:
# ab_glyph rasterizes glyphs from the bundled Newsreader TTF; png encodes the RGBA card.
ab_glyph = "0.2"
png = "0.17"
```

`crates/core/Cargo.toml`, under `[dependencies]` (after `two-face = { workspace = true }`):

```toml
ab_glyph = { workspace = true }
png = { workspace = true }
```

- [ ] **Step 2: Fetch the bundled Newsreader font (OFL)**

Run (github is the source; the static TTFs are not published, so we bundle the variable font):

```bash
mkdir -p crates/core/assets/fonts
curl -sL --max-time 30 -o "crates/core/assets/fonts/Newsreader[opsz,wght].ttf" \
  "https://github.com/google/fonts/raw/main/ofl/newsreader/Newsreader%5Bopsz%2Cwght%5D.ttf"
curl -sL --max-time 30 -o "crates/core/assets/fonts/OFL.txt" \
  "https://raw.githubusercontent.com/google/fonts/main/ofl/newsreader/OFL.txt"
# Verify it is a real TrueType file (magic 0x00010000) and a sane size (~450KB):
python3 - <<'PY'
p="crates/core/assets/fonts/Newsreader[opsz,wght].ttf"
b=open(p,"rb").read()
assert b[:4]==b"\x00\x01\x00\x00", f"not a TTF: {b[:4]!r}"
assert len(b)>200000, f"too small: {len(b)}"
print("OK", len(b), "bytes")
PY
```

Expected: `OK 451664 bytes` (or similar). If the download fails or the assertion trips, STOP and report — the font is a hard prerequisite.

- [ ] **Step 3: Write the failing test**

Create `crates/core/src/site/card.rs` with only this test module for now (the rest is added in Step 4):

```rust
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
}
```

- [ ] **Step 4: Write the minimal implementation**

Prepend to `crates/core/src/site/card.rs` (above the test module):

```rust
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
        for c in 0..3 {
            let bg = self.px[i + c] as f32;
            let fg = color[c] as f32;
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
```

- [ ] **Step 5: Register the module**

In `crates/core/src/site/mod.rs`, add alongside the other `mod` lines (e.g. after `mod book;` / its `pub use`, near line 188):

```rust
mod card;
pub use card::{CARD_DESIGN_VERSION, CARD_EXT, CARD_H, CARD_W, CardSpec, render_card};
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p taliesin-core --lib site::card`
Expected: `render_card_emits_a_1200x630_png` and `render_card_is_deterministic` PASS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/core/Cargo.toml "crates/core/assets/fonts" crates/core/src/site/card.rs crates/core/src/site/mod.rs Cargo.lock
git commit -m "feat(card): OG-card module skeleton — blank 1200x630 PNG, bundled Newsreader"
```

---

### Task 2: Anti-aliased polyline stroke (for the bell-curve mark)

**Files:**
- Modify: `crates/core/src/site/card.rs`

**Interfaces:**
- Consumes: `Canvas::blend` (Task 1).
- Produces: `Canvas::stroke_polyline(&mut self, pts: &[(f32,f32)], width: f32, color: [u8;3])`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `card.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-core --lib site::card::tests::stroke_marks`
Expected: FAIL — `no method named stroke_polyline`.

- [ ] **Step 3: Write the implementation**

Add these methods/functions to `card.rs` (the method inside `impl Canvas`, the helper free-standing):

```rust
impl Canvas {
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p taliesin-core --lib site::card`
Expected: PASS (all three card tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/site/card.rs
git commit -m "feat(card): anti-aliased polyline stroke for the bell-curve mark"
```

---

### Task 3: Text — font load, measure, wrap, truncate, draw

**Files:**
- Modify: `crates/core/src/site/card.rs`

**Interfaces:**
- Consumes: `Canvas::blend` (Task 1), `FONT_BYTES` (Task 1).
- Produces: `font() -> FontRef<'static>`; `text_width(&FontRef, &str, f32, f32) -> f32`; `wrap(&FontRef, &str, f32, f32) -> Vec<String>`; `wrap_clamp(&FontRef, &str, f32, f32, usize) -> Vec<String>`; `truncate_line(&FontRef, &str, f32, f32) -> String`; `Canvas::draw_text(&mut self, &FontRef, &str, f32, f32, f32, [u8;3], f32)`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
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
fn draw_text_marks_at_least_one_pixel() {
    let mut c = Canvas::new(200, 80, BG);
    c.draw_text(&font(), "Ag", 10.0, 55.0, 48.0, FG, 0.0);
    assert!(c.px.chunks(4).any(|p| p[0] != BG[0]), "some glyph pixels drawn");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-core --lib site::card::tests::text_width`
Expected: FAIL — `cannot find function font`.

- [ ] **Step 3: Write the implementation**

Add to `card.rs`. Add the import at the top of the file (below the doc comment):

```rust
use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
```

Then the functions and the `Canvas::draw_text` method:

```rust
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
    let mut lines = wrap(f, text, px, max_w);
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = lines.pop().unwrap();
        lines.push(truncate_line(f, &format!("{last} \u{2026}"), px, max_w));
    }
    lines
}

impl Canvas {
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
                    self.blend(bb.min.x as i32 + gx as i32, bb.min.y as i32 + gy as i32, color, cov);
                });
            }
            pen += scaled.h_advance(gid) + tracking;
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p taliesin-core --lib site::card`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/site/card.rs
git commit -m "feat(card): text layout — measure, wrap, truncate, draw glyphs"
```

---

### Task 4: Compose the full card + spec/url helpers

**Files:**
- Modify: `crates/core/src/site/card.rs`
- Modify: `crates/core/src/site/mod.rs` (extend the re-exports)

**Interfaces:**
- Consumes: `Canvas` + primitives (Tasks 1-3); `crate::hash::fnv1a`; `super::{Site, Page}`; `page.hero` (`HeroSpec { eyebrow, headline, lead }`), `page.title`, `page.description`, `page.date`, `page.categories`, `page.url`, `site.config.{title, description, url}`.
- Produces: `card_spec(&Site, &Page) -> CardSpec` (pub); `card_rel_path(&CardSpec) -> String` (pub, e.g. `"og/1a2b….png"`); `card_url(&Site, &Page) -> Option<String>` (`pub(crate)`, e.g. `"/og/1a2b….png"`).

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module. These need `Site`/`Page`; use the existing `write_site` test helper (as `jsonld_tests` in `meta.rs` does):

```rust
use crate::site::{Site, tests::write_site};

#[test]
fn card_spec_home_uses_the_hero() {
    let root = write_site(
        "cardhome",
        &[
            ("_site.yml", "title: Andreas Bogossian\nurl: https://ex.com\n"),
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
    let post = site.pages.iter().find(|p| p.url.contains("posts/a")).unwrap();
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
    assert!(url.starts_with("/og/") && url.ends_with(".png"), "got {url}");
    assert_eq!(format!("/{}", card_rel_path(&card_spec(&site, home))), url);
    let _ = std::fs::remove_dir_all(&with);

    let without = write_site(
        "cardnourl",
        &[("_site.yml", "title: B\n"), ("index.tmd", "---\ntitle: H\n---\n\nx\n")],
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
        CardSpec { eyebrow: None, headline: String::new(), lead: None, footer_wordmark: String::new(), domain: None },
        CardSpec { eyebrow: Some(long.clone()), headline: long.clone(), lead: Some(long), footer_wordmark: "W".into(), domain: Some("x.com".into()) },
    ] {
        let png = render_card(&spec);
        assert_eq!(png_dims(&png), (CARD_W, CARD_H));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-core --lib site::card::tests::card_spec_home`
Expected: FAIL — `cannot find function card_spec`.

- [ ] **Step 3: Write the implementation**

At the top of `card.rs`, extend the imports:

```rust
use super::{Page, Site};
use crate::hash::fnv1a;
```

Add the spec/url helpers:

```rust
/// Host portion of `url:` (`https://ex.com/blog/` -> `ex.com`).
fn host_of(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    after_scheme.split('/').next().unwrap_or(after_scheme).trim_end_matches('/').to_string()
}

/// Derive a page's card text: home (has a `hero:`) uses the hero; a post (`date:`)
/// uses its title + first category (else date); any page falls back to the site title.
pub fn card_spec(site: &Site, page: &Page) -> CardSpec {
    let site_title = site.config.title.clone().unwrap_or_default();
    let domain = site.config.url.as_deref().map(host_of);
    if let Some(hero) = page.hero.as_ref() {
        return CardSpec {
            eyebrow: hero.eyebrow.clone(),
            headline: hero.headline.clone().or_else(|| page.title.clone()).unwrap_or_else(|| site_title.clone()),
            lead: hero.lead.clone().or_else(|| site.config.description.clone()),
            footer_wordmark: site_title,
            domain,
        };
    }
    let eyebrow = if page.date.is_some() {
        page.categories.first().cloned().or_else(|| page.date.clone())
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
    site.config.url.as_ref()?;
    Some(format!("/{}", card_rel_path(&card_spec(site, page))))
}
```

Replace the Task-1 stub `render_card` body with the full composition:

```rust
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
    let mark_top = foot - 34.0;
    let mark_h = 26.0;
    let n = 48;
    let curve: Vec<(f32, f32)> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = (t - 0.5) * 6.0;
            (mark_x + t * mark_w, (foot - 6.0) - (-(u * u) / 2.0).exp() * mark_h)
        })
        .collect();
    c.stroke_polyline(&curve, 4.0, ACCENT);
    c.stroke_polyline(&[(mark_x, foot - 6.0), (mark_x + mark_w, foot - 6.0)], 3.0, ACCENT);
    // Wordmark after the mark.
    if !spec.footer_wordmark.is_empty() {
        c.draw_text(&f, &spec.footer_wordmark, mark_x + mark_w + 22.0, foot, 30.0, FG, 0.0);
    }
    // Domain, right-aligned.
    if let Some(dom) = spec.domain.as_deref().filter(|s| !s.is_empty()) {
        let w = text_width(&f, dom, 28.0, 0.0);
        c.draw_text(&f, dom, CARD_W as f32 - pad - w, foot, 28.0, MUTED, 0.0);
    }

    c.into_png()
}
```

- [ ] **Step 4: Extend the re-exports**

In `crates/core/src/site/mod.rs`, update the card re-export line from Task 1 to:

```rust
mod card;
pub use card::{CARD_DESIGN_VERSION, CARD_EXT, CARD_H, CARD_W, CardSpec, card_rel_path, card_spec, render_card};
pub(crate) use card::card_url;
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p taliesin-core --lib site::card`
Expected: PASS (all card tests, including the new spec/url/overflow ones).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/site/card.rs crates/core/src/site/mod.rs
git commit -m "feat(card): compose the full card + deterministic card_spec/card_url"
```

---

### Task 5: Wire `og:image` / `twitter:image` / JSON-LD image to the card

**Files:**
- Modify: `crates/core/src/site/meta.rs`

**Interfaces:**
- Consumes: `card::card_url(&Site, &Page) -> Option<String>` (Task 4).

- [ ] **Step 1: Write the failing test**

Add to the `jsonld_tests` module in `meta.rs` (it already has `write_site` in scope):

```rust
#[test]
fn social_image_is_the_generated_card_not_the_page_image() {
    let root = write_site(
        "cardsocial",
        &[
            ("_site.yml", "title: Blog\nurl: https://ex.com\n"),
            (
                "posts/a/index.tmd",
                "---\ntitle: My Post\ndate: 2026-05-15\ndescription: About.\nimage: fig.webp\n---\n\nx\n",
            ),
        ],
    );
    let site = crate::site::Site::discover(&root);
    let post = site.pages.iter().find(|p| p.url.contains("posts/a")).unwrap();
    let rel = crate::site::card_rel_path(&crate::site::card_spec(&site, post));
    let html = site.render_page("posts/a/index.tmd").unwrap();
    assert!(html.contains(&format!(r#"property="og:image" content="https://ex.com/{rel}""#)), "og:image = card");
    assert!(html.contains(&format!(r#"name="twitter:image" content="https://ex.com/{rel}""#)), "twitter:image = card");
    assert!(!html.contains("fig.webp"), "the page image: is not the social card");
    let _ = std::fs::remove_dir_all(&root);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-core --lib site::meta::jsonld_tests::social_image`
Expected: FAIL — asserts the old `image:`-derived URL, `fig.webp` still present.

- [ ] **Step 3: Rewrite the image derivation in `social_head`**

In `meta.rs`, add `card` to the imports (top of file): change `use super::{Page, Site, is_external_or_special};` to also bring in `card` — the simplest is a second line `use super::card;`.

Replace the `let image = page.card_image ...;` block (roughly lines 32-42) with:

```rust
    // Card image: the build-generated, branded OG card (absolute). Url-gated exactly
    // like the sidecars — `card_url` is Some only when `url:` is set, and `base` is Some
    // in the same case. The page's own `image:` stays the in-page/listing thumbnail.
    let image = card::card_url(site, page).and_then(|rel| base.map(|b| format!("{b}{rel}")));
```

- [ ] **Step 4: Rewrite the image block in `jsonld_head`**

In `jsonld_head`, replace the `if let Some(img) = page.card_image ... { bp["image"] = ...; }` block (roughly lines 148-159) with:

```rust
        if let Some(rel) = card::card_url(site, page) {
            bp["image"] = json!(format!("{base}{rel}"));
        }
```

- [ ] **Step 5: Drop the now-unused import if the compiler flags it**

`is_external_or_special` may now be unused in `meta.rs`. Run the build; if it warns/errors on the unused import, remove `is_external_or_special` from the `use super::{...};` line.

Run: `cargo build -p taliesin-core 2>&1 | grep -i "unused\|warning: unused" || echo "no unused-import warning"`

- [ ] **Step 6: Run the tests**

Run: `cargo test -p taliesin-core --lib site::meta`
Expected: PASS (existing jsonld tests + the new `social_image` test).

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/site/meta.rs
git commit -m "feat(card): point og:image/twitter:image/JSON-LD image at the generated card"
```

---

### Task 6: Emit cards at build + keep them through the stale-sweep + corpus pin

**Files:**
- Modify: `crates/server/src/build.rs` (aux-file zone ~L1141-1168 + keep-set ~L1186)
- Modify: `crates/core/tests/tech_blog.rs` (corpus pin)

**Interfaces:**
- Consumes: `taliesin_core::site::{card_spec, card_rel_path, render_card}` (Task 4).

- [ ] **Step 1: Declare the card-paths accumulator**

In `build.rs`, next to `let mut seo_written: Vec<PathBuf> = Vec::new();` (~L1141), add:

```rust
    let mut card_paths: Vec<PathBuf> = Vec::new();
```

- [ ] **Step 2: Emit the cards inside the url-gated aux block**

Inside the existing `if site.config.url.is_some() { ... }` block, after the `llms-full.txt` emit (~L1167, still inside the `if`), add:

```rust
        // OG social cards: one branded 1200x630 PNG per content page (og:image points
        // at /og/<hash>.png). Identical specs dedupe by hash. A failed encode/write is a
        // warning, never a build abort — the page still ships, its og:image just 404s.
        let mut seen_cards: std::collections::HashSet<String> = std::collections::HashSet::new();
        for page in &site.pages {
            if page.url == "404.html" {
                continue;
            }
            let spec = taliesin_core::site::card_spec(&site, page);
            let rel = taliesin_core::site::card_rel_path(&spec);
            if !seen_cards.insert(rel.clone()) {
                continue;
            }
            let dest = out.join(&rel);
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&dest, taliesin_core::site::render_card(&spec)) {
                Ok(()) => card_paths.push(PathBuf::from(&rel)),
                Err(e) => log::warn(&format!("cannot write {rel}: {e}")),
            }
        }
```

- [ ] **Step 3: Keep the cards through the stale-sweep**

At the keep-set assembly (~L1186, after `keep.extend(asset_paths.iter().cloned());`), add:

```rust
    keep.extend(card_paths.iter().cloned());
```

- [ ] **Step 4: Write the corpus pin (failing)**

Add to `crates/core/tests/tech_blog.rs`:

```rust
use taliesin_core::site::{card_rel_path, card_spec};

/// The blog home ships a generated OG card (never the removed static `og-image.webp`),
/// and its og:image URL is exactly the card path the build writes.
#[test]
fn home_og_image_is_the_generated_card() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let home = site.pages.iter().find(|p| p.url == "index.html").expect("home page");
    let rel = card_rel_path(&card_spec(&site, home)); // "og/<hex>.png"
    let html = site.render_page("index.tmd").unwrap();
    assert!(
        html.contains(&format!(r#"property="og:image" content="https://andreasbogossian.com/{rel}""#)),
        "home og:image points at the generated card ({rel})"
    );
    assert!(!html.contains("og-image.webp"), "stale static card is not referenced");
}
```

- [ ] **Step 5: Run test to verify it fails, then passes**

Run: `cargo test -p taliesin-core --test tech_blog home_og_image_is_the_generated_card`
Expected: FAIL first if run before Task 5 landed; with Tasks 4-5 in, PASS. (The `og-image.webp` reference assertion passes now because `card_url` ignores `image:`; Task 8 deletes the file + config line.)

- [ ] **Step 6: Verify the build actually writes the files**

Run:

```bash
cargo run -p taliesin-server -- build corpus/tech-blog --out /tmp/ogcard-check >/dev/null 2>&1
ls /tmp/ogcard-check/og/*.png | head && echo "cards written: $(ls /tmp/ogcard-check/og/*.png | wc -l)"
grep -ho 'og:image" content="[^"]*"' /tmp/ogcard-check/index.html
```

Expected: several `og/*.png` files; the home `index.html` og:image is `https://andreasbogossian.com/og/<hex>.png`. (Ignore any kernel-unavailable cell diagnostics — cards are prose-derived.)

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/build.rs crates/core/tests/tech_blog.rs
git commit -m "feat(card): emit per-page OG cards at build, keep them, pin the home card"
```

---

### Task 7: Serve cards in the live preview (phase 2)

**Files:**
- Modify: `crates/server/src/serve_site/mod.rs`

**Interfaces:**
- Consumes: `SiteApp.site: Mutex<Site>`; `taliesin_core::site::{card_spec, card_rel_path, render_card}`.

- [ ] **Step 1: Add the extractor import**

In `serve_site/mod.rs`, extend `use axum::extract::{Query, State};` to `use axum::extract::{Path as AxumPath, Query, State};`.

- [ ] **Step 2: Register the route**

In the `Router::new()` chain (~L180), add before `.fallback(page_or_asset)`:

```rust
        .route("/og/{name}", get(og_card))
```

- [ ] **Step 3: Add the handler**

Add near the other small handlers (e.g. after `favicon`):

```rust
/// Serve a preview OG card: find the page whose card hash matches `name` and render it
/// on demand (so the shared og:image tag is never a dead link during preview).
async fn og_card(State(app): State<Arc<SiteApp>>, AxumPath(name): AxumPath<String>) -> impl IntoResponse {
    let want = format!("og/{name}");
    let bytes = {
        let site = app.site.lock();
        site.pages.iter().find_map(|page| {
            let spec = taliesin_core::site::card_spec(&site, page);
            (taliesin_core::site::card_rel_path(&spec) == want)
                .then(|| taliesin_core::site::render_card(&spec))
        })
    };
    match bytes {
        Some(b) => ([(axum::http::header::CONTENT_TYPE, "image/png")], b).into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}
```

- [ ] **Step 4: Verify it compiles and serves**

Run: `cargo build -p taliesin-server`
Expected: builds clean.

Then manually (optional, needs a terminal): `cargo run -p taliesin-server -- preview corpus/tech-blog 4388`, open the home page, read its `og:image` path, and `curl -sI http://localhost:4388/og/<hex>.png` → `200` + `content-type: image/png`.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/serve_site/mod.rs
git commit -m "feat(card): serve OG cards lazily in the live preview"
```

---

### Task 8: Cleanup — delete the stale card, update THIRD_PARTY, final verify

**Files:**
- Delete: `corpus/tech-blog/og-image.webp`
- Modify: `corpus/tech-blog/_site.yml` (remove `image: og-image.webp`)
- Modify: `THIRD_PARTY.md`

- [ ] **Step 1: Confirm nothing else references the stale card**

Run: `grep -rn "og-image.webp" --include='*.tmd' --include='*.yml' --include='*.md' --include='*.rs' . | grep -v docs/superpowers`
Expected: only `corpus/tech-blog/_site.yml` (and possibly this plan). If a `.tmd`/template references it directly, STOP and reconcile.

- [ ] **Step 2: Remove the stale card + its config line**

```bash
git rm corpus/tech-blog/og-image.webp
```

Edit `corpus/tech-blog/_site.yml`: delete the `image: og-image.webp` line.

- [ ] **Step 3: Update THIRD_PARTY.md**

Add entries (match the file's existing format) for: **Newsreader** (SIL Open Font License 1.1; bundled at `crates/core/assets/fonts/`, license text in `OFL.txt`), **ab_glyph** (Apache-2.0/MIT), **png** (MIT/Apache-2.0). Read `THIRD_PARTY.md` first to match its exact section style.

- [ ] **Step 4: Run the THIRD_PARTY pin + the card/meta/corpus suites**

Run:

```bash
cargo test -p taliesin-core --test third_party
cargo test -p taliesin-core --lib site::card site::meta
cargo test -p taliesin-core --test tech_blog home_og_image_is_the_generated_card
```

Expected: all PASS. If `third_party` fails, it names the missing crate/font — add it.

- [ ] **Step 5: Full regression + rebuild**

Run:

```bash
cargo test -p taliesin-core
cargo test -p taliesin-server 2>/dev/null || true   # timing-flaky exec/kernel tests are unrelated (see backlog)
cargo run -p taliesin-server -- build corpus/tech-blog --out /tmp/ogcard-final >/dev/null 2>&1
echo "cards: $(ls /tmp/ogcard-final/og/*.png | wc -l)"; ls /tmp/ogcard-final/og-image.webp 2>&1 | head -1
```

Expected: core suite PASS; several `og/*.png`; `og-image.webp` is absent from the build output (`No such file`).

- [ ] **Step 6: Browser-verify a card (chrome-devtools MCP)**

Preview the built site (or `taliesin preview corpus/tech-blog 4388`), then load the home card image URL and a post card URL directly in the browser (`localhost:4388/og/<hex>.png`). Screenshot each. Confirm: 1200×630, dark, the **new** tagline on the home card (from the hero lead, never "Notes on challenging technical things"), the post title on a post card, the bell-curve mark + wordmark + domain in the footer, text not clipped. Iterate on the Task-4 layout constants if anything overflows.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "chore(card): remove stale og-image.webp, record Newsreader/ab_glyph/png in THIRD_PARTY"
```

---

## Self-Review

**1. Spec coverage:**
- Card generator (raster, ab_glyph, no C) → Tasks 1-4. ✓
- Bundled OFL serif → Task 1 (variable font, Regular instance; deviation noted below). ✓
- `og:image`/`twitter:image`/JSON-LD image = card, `image:` stays thumbnail → Task 5. ✓
- Deterministic hashed `/og/<hash>` path, url-gated → Task 4 (`card_rel_path`/`card_url`). ✓
- Build emit in the aux zone + stale-sweep keep → Task 6. ✓
- Preview lazy-serve → Task 7. ✓
- Delete `og-image.webp` + config line; THIRD_PARTY → Task 8. ✓
- Corpus pin (home card generated, not stale; URL == written path) → Task 6, plus card_spec/card_url/determinism/overflow unit pins → Tasks 1,4. Honest caveat (inputs/existence/dims/determinism pinned, not pixel text) preserved. ✓
- Error handling (encode/write warn-not-abort; empty/overlong wrap+truncate) → Task 6 Step 2, Task 4 overflow test. ✓

**2. Placeholder scan:** No TBD/TODO; every code step carries complete code; commands have expected output. ✓

**3. Type consistency:** `CardSpec` fields, `card_spec`/`card_rel_path`/`card_url`/`render_card` signatures, and `Canvas` methods are used identically across Tasks 1-7. `CARD_EXT`/`.png` and the `/og/<hash>` shape agree between `card_rel_path` (Task 4), `meta.rs` (Task 5), build (Task 6), and preview (Task 7). ✓

**Deviations from the spec (flag to the author):**
1. **Font: one variable font, Regular-only** (spec said Regular + Bold static TTFs). Forced by availability — the static TTFs 404; only `Newsreader[opsz,wght].ttf` is fetchable, and ab_glyph can't set a variable font's weight axis. Hierarchy via size + color instead of a bold face. A legitimate editorial-card look; revisit if a true bold is wanted (would need offline VF instancing).
2. **Encoder: PNG, not WebP** (spec authorized PNG as the fallback). Keeps deps to `ab_glyph` + `png` (both zero-C) and every code block concrete. WebP is a later optional encoder swap (bump `CARD_DESIGN_VERSION` + `CARD_EXT`).
