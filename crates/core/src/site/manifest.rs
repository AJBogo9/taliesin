//! `manifest.webmanifest` for a built site: the packaging that makes a site or book
//! installable ("Install app" in Chromium, "Add to Home Screen" on iOS, "Add to Dock" in
//! Safari). A sidecar generator like `feed.rs` / `llms.rs` / `seo.rs`.
//!
//! Deliberately NOT a service worker: installing changes how a reader RETURNS to a site,
//! not whether it works offline. Offline is the book `<book>.zip`
//! (`crates/server/src/zip.rs`), which the reader owns outright, with no cache living in
//! their browser to go stale.

use super::*;
use std::path::Path;

/// The light-mode page background, mirroring `--tali-bg` in
/// `crates/core/assets/css/tokens.css`. A manifest is JSON, so this cannot be
/// `var(--tali-bg)` and has to be duplicated here; `manifest_colors_match_the_tali_bg_tokens`
/// pins the duplicate so it cannot drift.
pub const MANIFEST_LIGHT_BG: &str = "#ffffff";
/// The token file the constant above mirrors. Read only by
/// `manifest_color_matches_the_tali_bg_token`, which is the whole point of keeping it: it
/// exists so the duplication is pinned, not so anything reads the CSS at runtime.
#[cfg(test)]
const TOKENS_LIGHT_CSS: &str = include_str!("../../assets/css/tokens.css");

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
/// "Taliesin"). A head that is empty or *longer* than 30 characters buys nothing, so the
/// full name is kept and the OS ellipsizes. (The prose used to read "no shorter than 30",
/// i.e. `>= 30`, which is not what the code does and nothing pinned either way.)
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
/// scales it) rather than shipping a fourth size.
///
/// Emits NO `<meta name="theme-color">`. The pre-paint theme bootstrap
/// (`render::theme::theme_head`) already creates one and keeps it in lockstep with the
/// canvas, following the reader's in-page toggle rather than only the OS preference, and
/// sourcing the colour from its own `BG` map. A static `prefers-color-scheme` pair here
/// would be strictly worse (OS-only) AND inert: the bootstrap runs earlier in the head, so
/// its media-less meta comes first in tree order and wins.
fn manifest_head_at(depth: usize, name: &str) -> String {
    let up = "../".repeat(depth);
    format!(
        "<link rel=\"manifest\" href=\"{up}manifest.webmanifest\" />\
         <link rel=\"apple-touch-icon\" href=\"{up}{ICON_192}\" />\
         <meta name=\"apple-mobile-web-app-title\" content=\"{label}\" />",
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
        // The boundary itself, which nothing pinned: 30 characters still counts as short
        // enough, 31 does not. Without these, widening `> 30` to `>= 30` survives.
        let head30 = "x".repeat(30);
        assert_eq!(short_name(&format!("{head30}: tail")), head30);
        let head31 = "x".repeat(31);
        let full31 = format!("{head31}: tail");
        assert_eq!(short_name(&full31), full31);
    }

    #[test]
    fn manifest_json_carries_the_installability_members() {
        let cfg = SiteConfig {
            title: Some("My Guide".into()),
            description: Some("A \"quoted\" guide".into()),
            ..SiteConfig::default()
        };
        let j = manifest_json_for(
            &cfg,
            "fallback-dir",
            Icons {
                author_supplied: false,
                maskable: true,
            },
        );
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
            Icons {
                author_supplied: true,
                maskable: false,
            },
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

    /// The manifest is JSON, so its colour cannot be `var(--tali-bg)` and must be
    /// duplicated as a Rust constant. Pin the duplicate against the CSS it mirrors.
    #[test]
    fn manifest_color_matches_the_tali_bg_token() {
        fn tali_bg(css: &str) -> String {
            let at = css.find("--tali-bg:").expect("tokens define --tali-bg");
            let rest = &css[at + "--tali-bg:".len()..];
            let end = rest.find(';').expect("declaration ends with ;");
            rest[..end].trim().to_string()
        }
        assert_eq!(tali_bg(TOKENS_LIGHT_CSS), MANIFEST_LIGHT_BG);
    }

    /// The pre-paint theme bootstrap owns `<meta name="theme-color">` and keeps it in
    /// lockstep with the reader's chosen theme. A static `prefers-color-scheme` pair here
    /// would be inert (it parses after the bootstrap's media-less meta, which wins on tree
    /// order) and would duplicate a hex the bootstrap's `BG` map single-sources.
    #[test]
    fn the_install_head_leaves_theme_color_to_the_theme_bootstrap() {
        assert!(!manifest_head_at(0, "X").contains("theme-color"));
    }

    #[test]
    fn manifest_head_is_depth_relative_and_escapes_the_label() {
        let head = manifest_head_at(2, "Ada & Co: Notes");
        assert!(
            head.contains("href=\"../../manifest.webmanifest\""),
            "{head}"
        );
        assert!(head.contains("href=\"../../icon-192.png\""), "{head}");
        assert!(head.contains("content=\"Ada &amp; Co\""), "{head}");

        let root = manifest_head_at(0, "X");
        assert!(root.contains("href=\"manifest.webmanifest\""), "{root}");
    }
}
