# Web App Manifest (installable sites and books) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A static site build emits `manifest.webmanifest` plus app icons, and every built page links them, so readers can install a Taliesin site or book as an app from Chrome, Edge, Safari and Android browsers.

**Architecture:** A new `crates/core/src/site/manifest.rs` generates the manifest JSON and the `<head>` block from the already-parsed `SiteConfig`, in the same shape as the existing `feed.rs` / `llms.rs` / `seo.rs` sidecar generators. `crates/server/src/build.rs` writes the file and the icons next to the other sidecars. The head tags are injected in `Site::render_page_doc_external`, which is the one static-build render path, so a live preview can never emit them.

**Tech Stack:** Rust (edition 2024), no new dependencies. Icons are PNGs generated once by hand with `inkscape` + ImageMagick `convert` and committed.

**Spec:** `docs/superpowers/specs/2026-07-24-webmanifest-install-design.md`

## Global Constraints

- **No service worker.** Nothing in this plan may register one, and no UI or docs copy may claim that installing makes a site work offline. The book `.zip` owns the offline claim.
- **No new `_site.yml` keys.** `NATIVE_KEYS` in `crates/core/src/site/config/mod.rs` must not change.
- **Build output only.** No manifest link, `apple-touch-icon`, or `theme-color` pair may appear on any preview render path (`Site::render_page_doc_warned`, `serve_site`).
- **No new colour literals.** The only hex values introduced are the existing `--tali-bg` tokens, duplicated into two constants that a test pins against the CSS.
- **No new dependency.** Icons are pre-rasterized and committed; no `resvg` or image crate enters `Cargo.toml`.
- **Icon resolution is all-or-nothing.** Either both author PNGs or the full bundled set. Never a mixture.
- Rust edition 2024. A `PostToolUse` hook runs `rustfmt` on every edited `.rs` file; CI enforces `cargo fmt`.

## Two deviations from the spec

**Head-tag location.** The spec places the `<head>` tags in `crates/core/src/render/page.rs` beside `favicon_link`. Reading the code showed a better fit: `Site::page_chrome` already pushes `meta::social_head`, `jsonld_head` and `feed_head` into `SiteCtx.includes.in_header` (`crates/core/src/site/mod.rs:596-598`), and that string reaches the same `<head>`. Task 3 uses that existing mechanism, so `page.rs` and `PageParts` are untouched.

## Scope change from the spec

The spec's `taliesin check` note (a hint when a project sets `favicon:` but ships no icons) is **dropped**. `crates/core/src/diagnostics/codes.rs:12-15` documents that `check` has only `error` and `warning` severities and "exits non-zero on ANY diagnostic regardless of severity". A cosmetic note would therefore turn `check` red for every project that has not shipped custom icons, including every project that exists today. Discoverability moves to the docs section in Task 4. Task 4 also amends the spec to record this.

---

### Task 1: Core manifest module

**Files:**
- Create: `crates/core/src/site/manifest.rs`
- Modify: `crates/core/src/site/mod.rs` (add the module declaration and re-export)
- Test: inline `#[cfg(test)] mod tests` at the bottom of `crates/core/src/site/manifest.rs`

**Interfaces:**
- Consumes: `SiteConfig` (`crates/core/src/site/config/mod.rs:28`), `Site` (`crates/core/src/site/mod.rs:139`, which has `pub root: PathBuf` and `pub config: SiteConfig`), `super::search::json_str`, and `esc` (which is `crate::render::escape_attr`, imported in `mod.rs:21` and reachable from a child module via `use super::*`).
- Produces, all used by Tasks 2 and 3:
  - `pub const MANIFEST_LIGHT_BG: &str`, `pub const MANIFEST_DARK_BG: &str`
  - `pub const ICON_192: &str`, `ICON_512: &str`, `ICON_MASKABLE_512: &str`
  - `pub const BUNDLED_ICONS: [(&str, &[u8]); 3]`
  - `pub struct Icons { pub author_supplied: bool, pub maskable: bool }`
  - `impl Site { pub fn manifest_icons(&self) -> Icons; pub fn manifest_json(&self) -> String; pub fn manifest_head(&self, page: &Page) -> String }`

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/site/manifest.rs` containing ONLY this test module for now (the code above it comes in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_name_truncates_at_the_first_colon() {
        assert_eq!(short_name("Taliesin: The User Guide"), "Taliesin");
        assert_eq!(short_name("My Site"), "My Site");
        // A leading colon leaves nothing usable, so the full name is kept.
        assert_eq!(short_name(": Odd"), ": Odd");
        // A head longer than 30 chars is no shorter in practice, so keep the full name
        // and let the OS ellipsize.
        let long = "A Very Long Book Title That Runs On: part two";
        assert_eq!(short_name(long), long);
    }

    #[test]
    fn manifest_json_carries_the_installability_members() {
        let cfg = SiteConfig {
            title: Some("My Guide".into()),
            description: Some("A \"quoted\" guide".into()),
            ..SiteConfig::default()
        };
        let j = manifest_json_for(&cfg, "fallback-dir", Icons { author_supplied: false, maskable: true });
        assert!(j.contains("\"name\":\"My Guide\""), "{j}");
        assert!(j.contains("\"short_name\":\"My Guide\""), "{j}");
        // Description is JSON-escaped, not raw.
        assert!(j.contains("\\\"quoted\\\""), "{j}");
        assert!(j.contains("\"start_url\":\"./\""), "{j}");
        assert!(j.contains("\"scope\":\"./\""), "{j}");
        assert!(j.contains("\"display\":\"standalone\""), "{j}");
        assert!(j.contains("\"sizes\":\"192x192\""), "{j}");
        assert!(j.contains("\"sizes\":\"512x512\""), "{j}");
        assert!(j.contains("\"purpose\":\"maskable\""), "{j}");
    }

    #[test]
    fn manifest_json_falls_back_to_the_directory_name_and_omits_an_unset_description() {
        let j = manifest_json_for(
            &SiteConfig::default(),
            "my-project",
            Icons { author_supplied: true, maskable: false },
        );
        assert!(j.contains("\"name\":\"my-project\""), "{j}");
        assert!(!j.contains("description"), "{j}");
        // No maskable file means no maskable entry, never a bundled substitute.
        assert!(!j.contains("maskable"), "{j}");
    }

    #[test]
    fn resolve_icons_is_all_or_nothing() {
        let dir = std::env::temp_dir().join(format!("tali-icons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Nothing supplied: the bundled set, which always has a maskable variant.
        let none = resolve_icons(&dir);
        assert!(!none.author_supplied);
        assert!(none.maskable);

        // Only the 512: still the bundled set, because a lone 512 would emit an icon
        // list missing the 192 Chrome requires for installability.
        std::fs::write(dir.join(ICON_512), b"x").unwrap();
        assert!(!resolve_icons(&dir).author_supplied);

        // Both: the author's set, with no maskable declared.
        std::fs::write(dir.join(ICON_192), b"x").unwrap();
        let both = resolve_icons(&dir);
        assert!(both.author_supplied);
        assert!(!both.maskable);

        // Both plus a maskable: the author's set, maskable declared.
        std::fs::write(dir.join(ICON_MASKABLE_512), b"x").unwrap();
        assert!(resolve_icons(&dir).maskable);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The manifest is JSON, so its colours cannot be `var(--tali-bg)` and must be
    /// duplicated as Rust constants. Pin the duplicates against the CSS they mirror.
    #[test]
    fn manifest_colors_match_the_tali_bg_tokens() {
        fn tali_bg(css: &str) -> String {
            let at = css.find("--tali-bg:").expect("tokens define --tali-bg");
            let rest = &css[at + "--tali-bg:".len()..];
            let end = rest.find(';').expect("declaration ends with ;");
            rest[..end].trim().to_string()
        }
        assert_eq!(tali_bg(TOKENS_LIGHT_CSS), MANIFEST_LIGHT_BG);
        assert_eq!(tali_bg(TOKENS_DARK_CSS), MANIFEST_DARK_BG);
    }

    #[test]
    fn manifest_head_is_depth_relative_and_escapes_the_label() {
        let head = manifest_head_at(2, "Ada & Co: Notes");
        assert!(head.contains("href=\"../../manifest.webmanifest\""), "{head}");
        assert!(head.contains("href=\"../../icon-192.png\""), "{head}");
        assert!(head.contains("content=\"Ada &amp; Co\""), "{head}");
        assert!(head.contains("(prefers-color-scheme: dark)"), "{head}");

        let root = manifest_head_at(0, "X");
        assert!(root.contains("href=\"manifest.webmanifest\""), "{root}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

First add the module declaration so the file compiles into the crate. In `crates/core/src/site/mod.rs`, next to the other module declarations (search for `mod search;`), add:

```rust
mod manifest;
```

and next to the other `pub use` re-exports in the same file, add:

```rust
pub use manifest::{BUNDLED_ICONS, ICON_192, ICON_512, ICON_MASKABLE_512, Icons};
```

Run: `cargo test -p taliesin-core --lib manifest`
Expected: FAIL to compile, with errors like `cannot find function 'short_name' in this scope`.

- [ ] **Step 3: Write the implementation**

Insert this ABOVE the test module in `crates/core/src/site/manifest.rs`:

```rust
//! `manifest.webmanifest` for a built site: the packaging that makes a site or book
//! installable ("Install app" in Chromium, "Add to Home Screen" on iOS, "Add to Dock" in
//! Safari). A sidecar generator like `feed.rs` / `llms.rs` / `seo.rs`.
//!
//! Deliberately NOT a service worker: installing changes how a reader RETURNS to a site,
//! not whether it works offline. Offline is the book `<book>.zip` (`crates/server/src/zip.rs`),
//! which the reader owns outright, with no cache living in their browser to go stale.

use super::*;
use std::path::Path;

/// The light-mode page background, mirroring `--tali-bg` in
/// `crates/core/assets/css/tokens.css`. A manifest is JSON, so this cannot be
/// `var(--tali-bg)` and has to be duplicated here; `manifest_colors_match_the_tali_bg_tokens`
/// pins the duplicate so it cannot drift.
pub const MANIFEST_LIGHT_BG: &str = "#ffffff";
/// The dark-mode page background, mirroring `--tali-bg` in `tokens-dark.css`. Used only by
/// the `prefers-color-scheme: dark` meta tag; a manifest holds exactly one colour.
pub const MANIFEST_DARK_BG: &str = "#16181d";

const TOKENS_LIGHT_CSS: &str = include_str!("../../assets/css/tokens.css");
const TOKENS_DARK_CSS: &str = include_str!("../../assets/css/tokens-dark.css");

/// Icon file names, at the output root. The same names serve as the author-override
/// convention (drop them next to `_site.yml`) and as the bundled fallback's output names,
/// so the manifest references one set of paths either way.
pub const ICON_192: &str = "icon-192.png";
pub const ICON_512: &str = "icon-512.png";
pub const ICON_MASKABLE_512: &str = "icon-maskable-512.png";

/// The bundled Taliesin mark, rasterized once from `web-client/favicon.svg` and committed
/// (see `crates/core/assets/icons/README.md` for the exact command). Committing PNGs keeps
/// a rasterizer dependency out of the build.
pub const BUNDLED_ICONS: [(&str, &[u8]); 3] = [
    (ICON_192, include_bytes!("../../assets/icons/icon-192.png")),
    (ICON_512, include_bytes!("../../assets/icons/icon-512.png")),
    (
        ICON_MASKABLE_512,
        include_bytes!("../../assets/icons/icon-maskable-512.png"),
    ),
];

/// Which icon set a project's manifest describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icons {
    /// The project supplies BOTH `icon-192.png` and `icon-512.png` at its root, so the
    /// build must not write the bundled set over them.
    pub author_supplied: bool,
    /// A maskable icon is available, so the manifest declares the `purpose: maskable`
    /// entry Android uses for adaptive icons.
    pub maskable: bool,
}

/// Resolve the icon set, all-or-nothing per source. Per-file fallback would produce a
/// mixed-brand result (the author's mark in the launcher, the Taliesin mark in Android's
/// adaptive-icon slot), and a lone `icon-512.png` would emit a list missing the 192 that
/// Chrome requires for installability, silently costing the install prompt.
fn resolve_icons(root: &Path) -> Icons {
    let has = |n: &str| root.join(n).is_file();
    if has(ICON_192) && has(ICON_512) {
        Icons {
            author_supplied: true,
            maskable: has(ICON_MASKABLE_512),
        }
    } else {
        // The bundled set always ships a maskable variant.
        Icons {
            author_supplied: false,
            maskable: true,
        }
    }
}

/// The launcher label: the name up to its first colon ("Taliesin: The User Guide" ->
/// "Taliesin"). A head that is empty or no shorter than 30 characters buys nothing, so the
/// full name is kept and the OS ellipsizes.
fn short_name(name: &str) -> String {
    let head = name.split(':').next().unwrap_or(name).trim();
    if head.is_empty() || head.chars().count() > 30 {
        name.trim().to_string()
    } else {
        head.to_string()
    }
}

/// The app name: `title:`, else the project directory name.
fn app_name<'a>(cfg: &'a SiteConfig, dir_name: &'a str) -> &'a str {
    cfg.title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(dir_name)
}

/// The manifest body. Every URL inside is relative to the manifest's own location, which
/// is why this needs no `url:` (unlike the feeds and sitemap) and why a book deployed under
/// `/docs/guide/` scopes to itself with no configuration.
fn manifest_json_for(cfg: &SiteConfig, dir_name: &str, icons: Icons) -> String {
    let name = app_name(cfg, dir_name);
    let short = short_name(name);
    let mut entries = format!(
        "{{\"src\":\"{ICON_192}\",\"sizes\":\"192x192\",\"type\":\"image/png\"}},\
         {{\"src\":\"{ICON_512}\",\"sizes\":\"512x512\",\"type\":\"image/png\"}}"
    );
    if icons.maskable {
        entries.push_str(&format!(
            ",{{\"src\":\"{ICON_MASKABLE_512}\",\"sizes\":\"512x512\",\
             \"type\":\"image/png\",\"purpose\":\"maskable\"}}"
        ));
    }
    let description = match cfg
        .description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(d) => format!("\"description\":\"{}\",", search::json_str(d)),
        None => String::new(),
    };
    format!(
        "{{\"name\":\"{n}\",\"short_name\":\"{s}\",{description}\
         \"start_url\":\"./\",\"scope\":\"./\",\"display\":\"standalone\",\
         \"theme_color\":\"{MANIFEST_LIGHT_BG}\",\"background_color\":\"{MANIFEST_LIGHT_BG}\",\
         \"icons\":[{entries}]}}\n",
        n = search::json_str(name),
        s = search::json_str(&short),
    )
}

/// The `<head>` block that makes a page installable, relative to a page at `depth`
/// directories below the output root. `apple-touch-icon` reuses the 192px asset (iOS
/// scales it) rather than shipping a fourth size. The theme-colour pair is what follows the
/// reader's light/dark toggle: a manifest holds only one static colour.
fn manifest_head_at(depth: usize, name: &str) -> String {
    let up = "../".repeat(depth);
    format!(
        "<link rel=\"manifest\" href=\"{up}manifest.webmanifest\" />\
         <link rel=\"apple-touch-icon\" href=\"{up}{ICON_192}\" />\
         <meta name=\"apple-mobile-web-app-title\" content=\"{label}\" />\
         <meta name=\"theme-color\" media=\"(prefers-color-scheme: light)\" \
         content=\"{MANIFEST_LIGHT_BG}\" />\
         <meta name=\"theme-color\" media=\"(prefers-color-scheme: dark)\" \
         content=\"{MANIFEST_DARK_BG}\" />",
        label = esc(&short_name(name)),
    )
}

impl Site {
    /// The icon set this project's manifest describes.
    pub fn manifest_icons(&self) -> Icons {
        resolve_icons(&self.root)
    }

    /// This project's `manifest.webmanifest` body.
    pub fn manifest_json(&self) -> String {
        let dir = self
            .root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        manifest_json_for(&self.config, &dir, self.manifest_icons())
    }

    /// The install `<head>` block for one page. Call only from the static build path: a
    /// live preview that emitted this would let Chrome install `localhost`, and that
    /// installed app breaks the moment the dev server stops.
    pub fn manifest_head(&self, page: &Page) -> String {
        let dir = self
            .root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let depth = page.url.matches('/').count();
        manifest_head_at(depth, app_name(&self.config, &dir))
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

The `BUNDLED_ICONS` `include_bytes!` will fail until Task 2 creates the PNGs. Create three empty placeholders so this task compiles, and Task 2 overwrites them with real images:

```bash
mkdir -p crates/core/assets/icons
touch crates/core/assets/icons/icon-192.png \
      crates/core/assets/icons/icon-512.png \
      crates/core/assets/icons/icon-maskable-512.png
```

Run: `cargo test -p taliesin-core --lib manifest`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/site/manifest.rs crates/core/src/site/mod.rs crates/core/assets/icons
git commit -m "feat(site): manifest.webmanifest generation (no service worker)"
```

---

### Task 2: Bundled icons and build emission

**Files:**
- Replace: `crates/core/assets/icons/icon-192.png`, `icon-512.png`, `icon-maskable-512.png` (real images over Task 1's placeholders)
- Create: `crates/core/assets/icons/README.md`
- Modify: `crates/server/src/build.rs` (emission next to the other sidecars, and the stale-sweep keep set)
- Test: `crates/server/tests/webmanifest.rs`

**Interfaces:**
- Consumes: `Site::manifest_json()`, `Site::manifest_icons()`, `taliesin_core::site::BUNDLED_ICONS`, `Icons::author_supplied` (Task 1).
- Produces: `manifest.webmanifest` plus, when the author supplied none, `icon-192.png` / `icon-512.png` / `icon-maskable-512.png` at the build output root. Task 3's head tags reference these paths.

- [ ] **Step 1: Generate and commit the real icons**

The mark in `web-client/favicon.svg` is a full-bleed rounded rect, so every export is flattened onto that same rect's fill: transparent corners render black on an iOS home screen. The colour is read out of the SVG rather than retyped, so it cannot drift from the favicon.

```bash
BG=$(grep -o 'rx="14" fill="#[0-9a-fA-F]*"' web-client/favicon.svg | grep -o '#[0-9a-fA-F]*')
echo "background: $BG"   # expect the favicon's rounded-rect fill

inkscape web-client/favicon.svg --export-type=png --export-width=192 --export-height=192 \
  --export-filename=/tmp/tali-192.png
inkscape web-client/favicon.svg --export-type=png --export-width=512 --export-height=512 \
  --export-filename=/tmp/tali-512.png
# Android masks a maskable icon to a circle 80% of the icon's width, so the mark has to sit
# inside a 410px safe zone centred on an opaque 512px canvas.
inkscape web-client/favicon.svg --export-type=png --export-width=410 --export-height=410 \
  --export-filename=/tmp/tali-410.png

convert /tmp/tali-192.png -background "$BG" -flatten \
  crates/core/assets/icons/icon-192.png
convert /tmp/tali-512.png -background "$BG" -flatten \
  crates/core/assets/icons/icon-512.png
convert /tmp/tali-410.png -background "$BG" -gravity center -extent 512x512 \
  crates/core/assets/icons/icon-maskable-512.png
```

Verify the dimensions:

Run: `file crates/core/assets/icons/*.png`
Expected: `192 x 192`, `512 x 512`, `512 x 512`, all PNG.

Open the three PNGs and confirm the mark is centred and legible, and that the maskable one has visible padding on all sides.

Then record how they were made, in `crates/core/assets/icons/README.md`:

```markdown
# App icons

The bundled Taliesin mark, used for `manifest.webmanifest` when a project supplies no
`icon-192.png` + `icon-512.png` of its own. Rasterized once and committed so that no
rasterizer dependency enters the build.

Regenerate after changing `web-client/favicon.svg` (requires `inkscape` and ImageMagick):

    BG=$(grep -o 'rx="14" fill="#[0-9a-fA-F]*"' web-client/favicon.svg | grep -o '#[0-9a-fA-F]*')
    inkscape web-client/favicon.svg --export-type=png --export-width=192 --export-height=192 \
      --export-filename=/tmp/tali-192.png
    inkscape web-client/favicon.svg --export-type=png --export-width=512 --export-height=512 \
      --export-filename=/tmp/tali-512.png
    inkscape web-client/favicon.svg --export-type=png --export-width=410 --export-height=410 \
      --export-filename=/tmp/tali-410.png
    convert /tmp/tali-192.png -background "$BG" -flatten crates/core/assets/icons/icon-192.png
    convert /tmp/tali-512.png -background "$BG" -flatten crates/core/assets/icons/icon-512.png
    convert /tmp/tali-410.png -background "$BG" -gravity center -extent 512x512 \
      crates/core/assets/icons/icon-maskable-512.png

The maskable variant pads the mark into the 80%-of-width safe zone Android crops to.
Every export is flattened onto the favicon's own background: transparent corners render
black on an iOS home screen.
```

- [ ] **Step 2: Write the failing test**

Create `crates/server/tests/webmanifest.rs`:

```rust
//! A site build emits `manifest.webmanifest` plus app icons at its output root, so a
//! reader can install the site or book as an app. Packaging only: there is no service
//! worker and no offline claim (the book `.zip` owns offline).

use std::fs;
use std::process::Command;

/// Build a throwaway project and return `(output dir, stderr)`. The caller deletes the
/// source tree; the output lives inside it, so read what you need before dropping it.
fn build(name: &str, files: &[(&str, &str)]) -> (std::path::PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("tali-manifest-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for (rel, body) in files {
        let dest = dir.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(dest, body).unwrap();
    }
    let out = dir.join("_out");
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run build");
    let stderr = String::from_utf8_lossy(&res.stderr).to_string();
    assert!(res.status.success(), "build failed: {stderr}");
    (out, stderr)
}

#[test]
fn a_website_build_emits_a_manifest_and_the_bundled_icons() {
    // No `url:` — unlike sitemap.xml and the feeds, the manifest must not be gated on it,
    // because every URL inside a manifest is relative to the manifest itself.
    let (out, _) = build(
        "site",
        &[
            ("_site.yml", "title: My Site\ndescription: Hello there\n"),
            ("index.tmd", "---\ntitle: Home\n---\n\n# Home\n\nHi.\n"),
        ],
    );
    let manifest = fs::read_to_string(out.join("manifest.webmanifest")).unwrap_or_default();
    let icon_192 = fs::read(out.join("icon-192.png")).unwrap_or_default();
    let icon_512 = fs::read(out.join("icon-512.png")).unwrap_or_default();
    let maskable = fs::read(out.join("icon-maskable-512.png")).unwrap_or_default();
    let sitemap = out.join("sitemap.xml").exists();
    let _ = fs::remove_dir_all(out.parent().unwrap());

    assert!(
        manifest.contains("\"name\":\"My Site\""),
        "manifest missing or unnamed: {manifest}"
    );
    assert!(manifest.contains("\"display\":\"standalone\""), "{manifest}");
    assert!(manifest.contains("\"description\":\"Hello there\""), "{manifest}");
    assert!(
        !sitemap,
        "sitemap.xml needs `url:`; its absence is what proves the manifest is not gated on it"
    );
    // Real PNGs, not the empty placeholders.
    assert_eq!(&icon_192[..4], b"\x89PNG", "icon-192.png is not a PNG");
    assert_eq!(&icon_512[..4], b"\x89PNG", "icon-512.png is not a PNG");
    assert_eq!(&maskable[..4], b"\x89PNG", "icon-maskable-512.png is not a PNG");
}

#[test]
fn a_book_build_emits_one_too_and_names_it_from_the_title() {
    let (out, _) = build(
        "book",
        &[
            ("_site.yml", "title: My Guide\nchapters:\n  - index.tmd\n"),
            ("index.tmd", "---\ntitle: Intro\n---\n\n# Intro\n\nHello.\n"),
        ],
    );
    let manifest = fs::read_to_string(out.join("manifest.webmanifest")).unwrap_or_default();
    let _ = fs::remove_dir_all(out.parent().unwrap());
    assert!(manifest.contains("\"name\":\"My Guide\""), "{manifest}");
}

#[test]
fn author_icons_win_and_suppress_the_bundled_set() {
    // A 1x1 PNG is enough: the build copies bytes, it never decodes them.
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let dir = std::env::temp_dir().join(format!("tali-manifest-own-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("_site.yml"), "title: Mine\n").unwrap();
    fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\n# Home\n\nHi.\n").unwrap();
    fs::write(dir.join("icon-192.png"), PNG_1X1).unwrap();
    fs::write(dir.join("icon-512.png"), PNG_1X1).unwrap();
    let out = dir.join("_out");
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run build");
    let stderr = String::from_utf8_lossy(&res.stderr).to_string();
    let manifest = fs::read_to_string(out.join("manifest.webmanifest")).unwrap_or_default();
    let shipped_192 = fs::read(out.join("icon-192.png")).unwrap_or_default();
    let maskable_exists = out.join("icon-maskable-512.png").exists();
    let _ = fs::remove_dir_all(&dir);

    assert!(res.status.success(), "build failed: {stderr}");
    assert_eq!(
        shipped_192, PNG_1X1,
        "the author's icon must not be overwritten by the bundled mark"
    );
    assert!(
        !maskable_exists,
        "an author set without a maskable file must not gain a bundled one (no mixed brands)"
    );
    assert!(!manifest.contains("maskable"), "{manifest}");
}

#[test]
fn an_incomplete_author_set_falls_back_to_the_bundled_icons() {
    // Only a 512: Chrome needs a 192 too, so a partial set must not be used at all.
    let (out, _) = build(
        "partial",
        &[
            ("_site.yml", "title: Partial\n"),
            ("index.tmd", "---\ntitle: Home\n---\n\n# Home\n\nHi.\n"),
            ("icon-512.png", "not-a-real-png-but-mirrored"),
        ],
    );
    let icon_192 = fs::read(out.join("icon-192.png")).unwrap_or_default();
    let manifest = fs::read_to_string(out.join("manifest.webmanifest")).unwrap_or_default();
    let _ = fs::remove_dir_all(out.parent().unwrap());
    assert_eq!(
        &icon_192[..4],
        b"\x89PNG",
        "an incomplete author set must fall back to the bundled icons"
    );
    assert!(manifest.contains("maskable"), "{manifest}");
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p taliesin-server --test webmanifest`
Expected: FAIL, all four tests, with "manifest missing or unnamed" and empty icon reads (the build writes nothing yet).

- [ ] **Step 4: Write the implementation**

In `crates/server/src/build.rs`, immediately BEFORE the `// SEO + discoverability sidecars` comment at line 1717, insert:

```rust
    // Installable-app packaging: `manifest.webmanifest` + the app icons at the output root,
    // so a reader can install this site/book from Chrome's omnibox, iOS "Add to Home Screen"
    // or Safari's "Add to Dock". Deliberately NOT gated on `url:` like the SEO sidecars
    // below: every URL in a manifest resolves against the manifest itself, so a project with
    // no configured site URL installs correctly anyway. No service worker ships with it —
    // installing changes how a reader returns, not whether the site works offline (that is
    // the book `<book>.zip`).
    let mut manifest_written: Vec<PathBuf> = Vec::new();
    match std::fs::write(out.join("manifest.webmanifest"), site.manifest_json()) {
        Ok(()) => manifest_written.push(PathBuf::from("manifest.webmanifest")),
        Err(e) => log::warn(&format!("cannot write manifest.webmanifest: {e}")),
    }
    // The author's own `icon-192.png` + `icon-512.png` are already mirrored into the output
    // by `mirror_assets`, so the bundled mark ships only when they supplied no usable set.
    if !site.manifest_icons().author_supplied {
        for (name, bytes) in taliesin_core::site::BUNDLED_ICONS {
            match std::fs::write(out.join(name), bytes) {
                Ok(()) => manifest_written.push(PathBuf::from(name)),
                Err(e) => log::warn(&format!("cannot write {name}: {e}")),
            }
        }
    }
```

Then, in the stale-sweep keep set, immediately after `keep.extend(seo_written.iter().cloned());` (line 1837), add:

```rust
    keep.extend(manifest_written.iter().cloned());
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p taliesin-server --test webmanifest`
Expected: PASS, 4 tests.

- [ ] **Step 6: Run the full suite to check nothing regressed**

Run: `cargo test -p taliesin-core && cargo test -p taliesin-server --test book_offline_archive`
Expected: PASS. The stale-sweep change is the risk here: if `manifest.webmanifest` were missing from `keep`, the sweep would delete it and the tests above would fail.

- [ ] **Step 7: Commit**

```bash
git add crates/core/assets/icons crates/server/src/build.rs crates/server/tests/webmanifest.rs
git commit -m "feat(build): emit manifest.webmanifest + app icons for site builds"
```

---

### Task 3: Build-only head tags

**Files:**
- Modify: `crates/core/src/site/mod.rs:718-733` (`render_page_doc_external`)
- Test: append to `crates/server/tests/webmanifest.rs`, plus one unit test in `crates/core/src/site/mod.rs`'s existing test module

**Interfaces:**
- Consumes: `Site::manifest_head(&self, page: &Page) -> String` (Task 1), `SiteCtx.includes.in_header` (`crates/core/src/render/page.rs:43`).
- Produces: the manifest `<link>`, `apple-touch-icon`, `apple-mobile-web-app-title` and theme-colour pair in every statically built page's `<head>`, and in no preview page's.

- [ ] **Step 1: Write the failing tests**

Append to `crates/server/tests/webmanifest.rs`:

```rust
#[test]
fn every_built_page_links_the_manifest_at_its_own_depth() {
    let (out, _) = build(
        "depth",
        &[
            (
                "_site.yml",
                "title: Deep Book: Volume One\nchapters:\n  - index.tmd\n  - sub/two.tmd\n",
            ),
            ("index.tmd", "---\ntitle: Intro\n---\n\n# Intro\n\nHello.\n"),
            ("sub/two.tmd", "---\ntitle: Two\n---\n\n# Two\n\nWorld.\n"),
        ],
    );
    let index = fs::read_to_string(out.join("index.html")).unwrap_or_default();
    let nested = fs::read_to_string(out.join("sub/two.html")).unwrap_or_default();
    let _ = fs::remove_dir_all(out.parent().unwrap());

    assert!(
        index.contains("rel=\"manifest\" href=\"manifest.webmanifest\""),
        "root page does not link the manifest"
    );
    assert!(
        nested.contains("rel=\"manifest\" href=\"../manifest.webmanifest\""),
        "a nested page's manifest link must be depth-relative"
    );
    assert!(
        nested.contains("rel=\"apple-touch-icon\" href=\"../icon-192.png\""),
        "a nested page's apple-touch-icon must be depth-relative"
    );
    // The launcher label is the title up to its first colon.
    assert!(
        index.contains("name=\"apple-mobile-web-app-title\" content=\"Deep Book\""),
        "{index}"
    );
    assert!(
        index.contains("(prefers-color-scheme: dark)"),
        "the theme-colour pair is what follows the reader's toggle"
    );
}
```

And in the `#[cfg(test)]` module of `crates/core/src/site/mod.rs`, next to `external_site_render_keeps_search_index_inline_drops_shared_toc_js` (line 2302), add, reusing that module's `write_site` and `render_page` helpers:

```rust
    /// The preview render path must never emit the install head. A manifest served from
    /// `localhost` lets Chrome install the dev server, and that installed app breaks
    /// permanently the moment the server stops.
    #[test]
    fn only_the_static_build_path_emits_the_install_head() {
        let root = write_site(
            "install-head",
            &[
                ("_site.yml", "title: Demo\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\n# Home\n\nHi.\n"),
            ],
        );
        let site = Site::discover(&root);
        let (preview_html, _) = render_page(&site, "index.tmd");

        let page = site.pages.iter().find(|p| p.rel == "index.tmd").unwrap();
        let src = std::fs::read_to_string(&page.input).unwrap();
        let doc = crate::render::render_document_with_includes(&src, &site.root);
        let ext = render::ExternalAssets {
            app_css: "_assets/app.a.css",
            katex_css: "_assets/katex.b.css",
            app_js: "_assets/app.c.js",
            mermaid_js: "_assets/mermaid.d.js",
            jslibs_js: "_assets/jslibs.e.js",
        };
        let (build_html, _) = site.render_page_doc_external(page, doc, ext);
        let _ = std::fs::remove_dir_all(&root);

        assert!(
            build_html.contains("rel=\"manifest\""),
            "the static build path must emit the install head: {build_html}"
        );
        assert!(
            !preview_html.contains("rel=\"manifest\""),
            "preview must not offer an installable manifest: {preview_html}"
        );
        assert!(!preview_html.contains("apple-touch-icon"), "{preview_html}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p taliesin-server --test webmanifest && cargo test -p taliesin-core --lib only_the_static_build_path`
Expected: the depth test FAILS with "root page does not link the manifest". The core test PASSES already (nothing emits the head yet), which is correct: it is a regression guard, not a red-then-green test.

- [ ] **Step 3: Write the implementation**

In `crates/core/src/site/mod.rs`, in `render_page_doc_external`, change:

```rust
        // The multi-page build path: a book emits `<book>.zip` at its output root, so this
        // is the one place the offline-download link is wired.
        let ctx = self.page_chrome(page, self.is_book());
```

to:

```rust
        // The multi-page build path: a book emits `<book>.zip` at its output root, so this
        // is the one place the offline-download link is wired.
        let mut ctx = self.page_chrome(page, self.is_book());
        // Same reasoning for the install head (`manifest.webmanifest` + the iOS icon/label +
        // the theme-colour pair): the manifest is a build artifact, and a live preview that
        // emitted it would let Chrome install `localhost`, leaving the reader an app that
        // breaks the moment the dev server stops.
        ctx.includes
            .in_header
            .push_str(&self.manifest_head(page));
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p taliesin-server --test webmanifest && cargo test -p taliesin-core`
Expected: PASS, 5 tests in `webmanifest`, and the whole core suite green (including the corpus invariant tests, which must be unaffected: nothing here touches a block, an id, or a sourcepos).

- [ ] **Step 5: Verify in a real browser**

```bash
cargo build -p taliesin-server
cargo run -p taliesin-server -- build docs/guide --out /tmp/guide-manifest
```

Serve it and open it with the chrome-devtools MCP:

```bash
python3 -m http.server 4388 --directory /tmp/guide-manifest
```

In the browser at `http://localhost:4388/`: confirm no console errors, and that DevTools > Application > Manifest shows the name, the icons, and no installability errors other than the HTTPS one that plain-HTTP localhost is exempt from. Take a screenshot of the Manifest pane as evidence.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/site/mod.rs crates/server/tests/webmanifest.rs
git commit -m "feat(site): link the manifest + app icons from statically built pages"
```

---

### Task 4: Documentation and spec amendment

**Files:**
- Modify: `docs/guide/reference/` (the publishing or deployment page; find it with the command in Step 1)
- Modify: `docs/superpowers/specs/2026-07-24-webmanifest-install-design.md` (record the dropped `check` note)
- Modify: `notes/FEATURE-IDEAS.md:174` (mark the install half of item 14 shipped, leaving the service-worker half parked)

**Interfaces:**
- Consumes: the behaviour built in Tasks 1 to 3.
- Produces: no code. This is the discoverability path for the icon convention, replacing the `check` note the spec originally called for.

- [ ] **Step 1: Find the right guide page**

Run: `ls docs/guide/reference/ docs/guide/using/ && grep -rln "publish\|deploy" docs/guide/`
Expected: a list of `.tmd` pages. Add to the one covering publishing or deployment; if there is none, create `docs/guide/reference/installing.tmd` and add it to `docs/guide/_site.yml`'s `chapters:`.

- [ ] **Step 2: Write the section**

```markdown
## Installing a site as an app

Every built site and book ships a `manifest.webmanifest`, so a reader can install it:

- **Chrome and Edge** (desktop and Android) offer an "Install" button in the address bar.
- **iOS and iPadOS**: Share, then "Add to Home Screen". It opens without browser chrome.
- **macOS Safari**: "Add to Dock".
- **Firefox on desktop** ignores manifests. Nothing breaks; there is simply no install button.

Installing changes how a reader *returns* to your site. It does **not** make the site work
offline. For offline reading, a book build also emits a `<book>.zip` that readers download
from the topbar and keep.

### Using your own icon

By default an installed site wears the Taliesin mark. To use your own, drop two PNGs next
to `_site.yml`:

    icon-192.png          192 x 192
    icon-512.png          512 x 512
    icon-maskable-512.png 512 x 512, optional

Both of the first two are required: with only one, the built-in mark is used instead,
because Chrome needs both sizes before it will offer to install at all. The optional
maskable icon is what Android crops to a circle, so keep its artwork inside the middle 80%.
Make the images square and fully opaque; transparent corners render black on an iOS home
screen.
```

- [ ] **Step 3: Verify the docs build**

Run: `cargo run -p taliesin-server -- check docs/guide`
Expected: exit 0, no new diagnostics.

- [ ] **Step 4: Amend the spec and the idea list**

In `docs/superpowers/specs/2026-07-24-webmanifest-install-design.md`, replace the paragraph beginning "`taliesin check` emits a note in two cases" with:

```markdown
**Dropped during implementation.** The spec originally called for a `taliesin check` note
when a project ships no icons or an incomplete set. `crates/core/src/diagnostics/codes.rs:12-15`
has only `error` and `warning` severities and states that `check` "exits non-zero on ANY
diagnostic regardless of severity", so a cosmetic note would turn `check` red for every
project that has not shipped custom icons. Discoverability lives in the user guide instead.
```

Delete the corresponding test item from the spec's testing list. In `notes/FEATURE-IDEAS.md`, edit item 14 so the installable half is marked shipped with today's date and the service-worker half stays parked, matching how other shipped items in that file are annotated.

- [ ] **Step 5: Commit**

```bash
git add docs/guide docs/superpowers/specs/2026-07-24-webmanifest-install-design.md notes/FEATURE-IDEAS.md
git commit -m "docs(guide): how to install a site as an app + bring your own icon"
```

---

## Final verification

- [ ] Run: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings`
  Expected: clean.
- [ ] Run: `cargo test -p taliesin-core && cargo test -p taliesin-server`
  Expected: green. If the full-parallelism run flakes, re-run the failing binary with `--test-threads=1` before concluding anything is broken.
- [ ] Confirm the preview loop is untouched: `cargo run -p taliesin-server -- preview docs/guide`, open it, and check with DevTools that the page `<head>` contains no `rel="manifest"` and no `apple-touch-icon`.
