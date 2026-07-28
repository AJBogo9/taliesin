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

/// The splash colour a fresh install paints before the page does.
///
/// **This is the theme bootstrap's FALLBACK mode, not "the light theme".** The distinction
/// is the whole reason the pin below was rewritten: a manifest is static JSON with no media
/// query, while the page's theme is `auto` — it follows `prefers-color-scheme` and only
/// falls back to light when the OS expresses no preference (`render::theme::theme_head`'s
/// `DEFAULT()`). So the honest statement of what this value is is "whatever the bootstrap
/// paints when it has nothing to follow", and `manifest_bg_tracks_the_theme_bootstrap_fallback`
/// pins exactly that, sourced from the bootstrap's own `BG` map.
///
/// **Known limit, deliberately not papered over:** installing from a phone in dark mode
/// still shows one white splash frame before the page resolves to `#16181d`. The manifest
/// format cannot express an OS-conditional colour, and there is no site-level theme key to
/// read instead (`SiteConfig` has none — theme is per-document front matter). The *address
/// bar* is unaffected: the bootstrap owns `<meta name="theme-color">` and keeps it in
/// lockstep with the reader's actual theme, which is why `manifest_head_at` emits none.
pub const MANIFEST_LIGHT_BG: &str = "#ffffff";
/// The theme bootstrap, read only by `manifest_bg_tracks_the_theme_bootstrap_fallback` so
/// the splash colour is pinned against the code that actually paints the page rather than
/// against a CSS token that merely happens to agree with it today.
#[cfg(test)]
const THEME_BOOTSTRAP: &str = include_str!("../render/theme.rs");

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

/// The project's `favicon:` promoted to an app icon, for a project that declared a mark but
/// no `icon-*.png` pair. `sizes` is `None` when the file's real dimensions could not be read
/// — the member is optional in the manifest spec, and omitting it is the only honest option:
/// claiming `192x192` for a 32px mark is the kind of false statement this tool exists to
/// avoid, and it would cost the install prompt anyway once the browser checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaviconIcon {
    pub src: String,
    pub mime: &'static str,
    pub sizes: Option<String>,
}

/// Which icon set a project's manifest describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icons {
    /// The project supplies BOTH `icon-192.png` and `icon-512.png` at its root, so the
    /// build must not write the bundled set over them.
    pub author_supplied: bool,
    /// A maskable icon is available, so the manifest declares the `purpose: maskable`
    /// entry Android uses for adaptive icons.
    pub maskable: bool,
    /// The `favicon:` mark, used when the project declared one but supplied no icon pair.
    pub favicon: Option<FaviconIcon>,
}

impl Icons {
    /// Whether the build must write the bundled Taliesin PNGs. False as soon as the project
    /// expressed a brand of its own by either route — otherwise the bundled mark is written
    /// over nothing, is referenced by nothing, and still lands in the deploy.
    pub fn ships_bundled(&self) -> bool {
        !self.author_supplied && self.favicon.is_none()
    }
}

/// The PNG dimensions in `path`, read straight out of the IHDR chunk (the first chunk a PNG
/// is required to carry). 24 bytes and no image dependency; `None` for anything that is not
/// a PNG, which is the caller's cue to omit `sizes` rather than guess one.
fn png_size(path: &Path) -> Option<(u32, u32)> {
    let head = std::fs::read(path).ok()?;
    let head = head.get(..24)?;
    if &head[..8] != b"\x89PNG\r\n\x1a\n" || &head[12..16] != b"IHDR" {
        return None;
    }
    let n = |at: usize| u32::from_be_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
    Some((n(16), n(20)))
}

/// Resolve the icon set, all-or-nothing per source. Per-file fallback would produce a
/// mixed-brand result (the author's mark in the launcher, the Taliesin mark in Android's
/// adaptive-icon slot), and a lone `icon-512.png` would emit a list missing the 192 that
/// Chrome requires for installability, silently costing the install prompt.
///
/// **`favicon:` is consulted before the bundled fallback**, because ignoring it put
/// *Taliesin's* mark on a stranger's home screen for every author who had already said what
/// their mark was. The icon pair still wins over it: those names are the documented way to
/// supply the two raster sizes a launcher actually wants, and a project that ships both has
/// said something more specific than a favicon does.
fn resolve_icons(root: &Path, favicon: Option<&str>) -> Icons {
    let has = |n: &str| root.join(n).is_file();
    if has(ICON_192) && has(ICON_512) {
        return Icons {
            author_supplied: true,
            maskable: has(ICON_MASKABLE_512),
            favicon: None,
        };
    }
    let declared = favicon
        .map(str::trim)
        .filter(|s| !s.is_empty())
        // A remote or absolute mark is not ours to describe: the manifest's URLs resolve
        // against the manifest, and `mirror_assets` never copied it into the output.
        .filter(|s| !s.contains("://") && !s.starts_with('/'))
        .filter(|s| root.join(s).is_file())
        .and_then(|src| {
            let ext = src.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
            let mime = match ext.as_str() {
                "svg" => "image/svg+xml",
                "png" => "image/png",
                // An `.ico` is a multi-size container with no single honest `sizes`, and
                // no launcher wants one. Fall through to the bundled set rather than
                // describe it wrongly.
                _ => return None,
            };
            let sizes = if ext == "svg" {
                // A vector mark genuinely serves every size, which is the one case where
                // `any` is a true statement rather than a convenient one.
                Some("any".to_string())
            } else {
                png_size(&root.join(src)).map(|(w, h)| format!("{w}x{h}"))
            };
            Some(FaviconIcon {
                src: src.to_string(),
                mime,
                sizes,
            })
        });
    Icons {
        author_supplied: false,
        // The bundled set always ships a maskable variant; a promoted favicon does not,
        // and claiming `purpose: maskable` for a mark never drawn for Android's safe zone
        // gets it cropped into its own padding.
        maskable: declared.is_none(),
        favicon: declared,
    }
}

/// Where an installed app opens. `./` resolves to the output root's directory index, which
/// is correct only if something answers there — and a project whose pages are all named
/// (no `index.tmd`) has no `index.html`, so the installed app cold-launched into a 404 with
/// `display: standalone` having removed the address bar to escape it. Falls back to the
/// first page in project order, which is the same page the nav treats as the way in.
fn start_url_for<'a>(urls: impl IntoIterator<Item = &'a str>) -> String {
    let mut first: Option<&str> = None;
    for url in urls {
        if url == "index.html" {
            return "./".to_string();
        }
        first.get_or_insert(url);
    }
    first.unwrap_or("./").to_string()
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
fn manifest_json_for(cfg: &SiteConfig, dir_name: &str, icons: &Icons, start_url: &str) -> String {
    let name = app_name(cfg, dir_name);
    let short = short_name(name);
    let mut entries = match &icons.favicon {
        // The project's own mark, describing only what is known about it.
        Some(f) => {
            let sizes = match &f.sizes {
                Some(s) => format!("\"sizes\":\"{}\",", search::json_str(s)),
                None => String::new(),
            };
            format!(
                "{{\"src\":\"{}\",{sizes}\"type\":\"{}\"}}",
                search::json_str(&f.src),
                f.mime
            )
        }
        None => format!(
            "{{\"src\":\"{ICON_192}\",\"sizes\":\"192x192\",\"type\":\"image/png\"}},\
             {{\"src\":\"{ICON_512}\",\"sizes\":\"512x512\",\"type\":\"image/png\"}}"
        ),
    };
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
         \"start_url\":\"{u}\",\"scope\":\"./\",\"display\":\"standalone\",\
         \"theme_color\":\"{MANIFEST_LIGHT_BG}\",\"background_color\":\"{MANIFEST_LIGHT_BG}\",\
         \"icons\":[{entries}]}}\n",
        n = search::json_str(name),
        s = search::json_str(&short),
        u = search::json_str(start_url),
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
        resolve_icons(&self.root, self.config.favicon.as_deref())
    }

    /// This project's `manifest.webmanifest` body.
    pub fn manifest_json(&self) -> String {
        let dir = self
            .root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        manifest_json_for(
            &self.config,
            &dir,
            &self.manifest_icons(),
            &start_url_for(self.pages.iter().map(|p| p.url.as_str())),
        )
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
            &Icons {
                author_supplied: false,
                maskable: true,
                favicon: None,
            },
            "./",
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
            &Icons {
                author_supplied: true,
                maskable: false,
                favicon: None,
            },
            "./",
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

        // Nothing supplied (and no `favicon:` either): the bundled set, which always has a
        // maskable variant.
        let none = resolve_icons(&dir, None);
        assert!(!none.author_supplied);
        assert!(none.maskable);
        assert!(none.ships_bundled());

        // Only the 512: still the bundled set, because a lone 512 would emit an icon
        // list missing the 192 Chrome requires for installability.
        std::fs::write(dir.join(ICON_512), b"x").unwrap();
        assert!(!resolve_icons(&dir, None).author_supplied);

        // Both: the author's set, with no maskable declared.
        std::fs::write(dir.join(ICON_192), b"x").unwrap();
        let both = resolve_icons(&dir, None);
        assert!(both.author_supplied);
        assert!(!both.maskable);

        // Both plus a maskable: the author's set, maskable declared.
        std::fs::write(dir.join(ICON_MASKABLE_512), b"x").unwrap();
        assert!(resolve_icons(&dir, None).maskable);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The manifest is JSON, so its splash colour cannot be `var(--tali-bg)` and must be
    /// duplicated as a Rust constant. **What the duplicate must track is the theme
    /// bootstrap's fallback**, not "the light token" — those agree today, which is exactly
    /// why the old pin looked right while asserting the wrong thing: it would have kept
    /// passing if the bootstrap's fallback mode changed, and gone red for a pure CSS token
    /// rename that changes nothing a reader sees.
    #[test]
    fn manifest_bg_tracks_the_theme_bootstrap_fallback() {
        // `DEFAULT()` returns "light" whenever the OS expresses no dark preference, so the
        // splash must be the bootstrap's own `BG.light`, read out of the same map it paints
        // from rather than copied.
        let map = THEME_BOOTSTRAP
            .split("var BG = {")
            .nth(1)
            .expect("the bootstrap defines a BG map");
        let map = &map[..map.find('}').expect("the BG map closes")];
        let light = map
            .split("light:")
            .nth(1)
            .expect("the BG map has a light entry")
            .trim()
            .trim_matches(|c: char| c == '\'' || c == ',' || c.is_whitespace());
        assert_eq!(
            light, MANIFEST_LIGHT_BG,
            "splash colour drifted from the theme bootstrap's fallback: {map}"
        );
        assert!(
            THEME_BOOTSTRAP.contains("return \"light\";"),
            "the bootstrap no longer falls back to light — the splash colour must follow it"
        );
    }

    #[test]
    fn a_configured_favicon_becomes_the_app_icon_rather_than_the_taliesin_mark() {
        // An author who has already said what their mark is got Taliesin's mark installed on
        // their readers' home screens: `resolve_icons` looked only for the literal PNG pair,
        // so `favicon: acme.svg` produced a manifest pointing at a byte-identical copy of the
        // bundled brand. A trademark artifact on a stranger's phone.
        let dir = std::env::temp_dir().join(format!("tali-favicon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("acme.svg"), b"<svg/>").unwrap();

        let cfg = SiteConfig {
            favicon: Some("acme.svg".into()),
            ..SiteConfig::default()
        };
        let icons = resolve_icons(&dir, cfg.favicon.as_deref());
        assert_eq!(
            icons.favicon.as_ref().map(|f| f.src.as_str()),
            Some("acme.svg")
        );
        assert!(
            !icons.ships_bundled(),
            "the build must not write the bundled mark next to a manifest that never cites it"
        );
        let j = manifest_json_for(&cfg, "acme", &icons, "./");
        assert!(j.contains("\"src\":\"acme.svg\""), "{j}");
        assert!(j.contains("\"type\":\"image/svg+xml\""), "{j}");
        // A vector mark serves every size, so `any` is a true claim here.
        assert!(j.contains("\"sizes\":\"any\""), "{j}");
        assert!(
            !j.contains("icon-192.png"),
            "Taliesin's mark still shipped: {j}"
        );
        // …and no `purpose: maskable`, which a mark not drawn for Android's safe zone
        // would only get cropped into its own padding.
        assert!(!j.contains("maskable"), "{j}");

        // A raster mark is described with its REAL size, read from the PNG header — never a
        // convenient `192x192` the file does not actually have.
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&512u32.to_be_bytes());
        png.extend_from_slice(&256u32.to_be_bytes());
        std::fs::write(dir.join("brand.png"), &png).unwrap();
        let raster = SiteConfig {
            favicon: Some("brand.png".into()),
            ..SiteConfig::default()
        };
        let icons = resolve_icons(&dir, raster.favicon.as_deref());
        let j = manifest_json_for(&raster, "acme", &icons, "./");
        assert!(j.contains("\"sizes\":\"512x256\""), "{j}");
        assert!(j.contains("\"type\":\"image/png\""), "{j}");

        // An unreadable size is omitted, not invented.
        std::fs::write(dir.join("truncated.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        let trunc = SiteConfig {
            favicon: Some("truncated.png".into()),
            ..SiteConfig::default()
        };
        let icons = resolve_icons(&dir, trunc.favicon.as_deref());
        assert_eq!(icons.favicon.as_ref().unwrap().sizes, None);
        assert!(!manifest_json_for(&trunc, "acme", &icons, "./").contains("\"sizes\""));

        // The documented icon PAIR still wins: it says something more specific than a
        // favicon does, and it is the route that carries a maskable variant.
        std::fs::write(dir.join(ICON_192), b"x").unwrap();
        std::fs::write(dir.join(ICON_512), b"x").unwrap();
        let both = resolve_icons(&dir, cfg.favicon.as_deref());
        assert!(both.author_supplied);
        assert_eq!(both.favicon, None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_favicon_that_is_not_ours_to_describe_falls_back_to_the_bundled_set() {
        let dir = std::env::temp_dir().join(format!("tali-favicon-ext-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mark.ico"), b"x").unwrap();

        for src in [
            "https://cdn.example.com/m.svg", // not copied into the output at all
            "/absolute/m.svg",               // resolves outside the manifest's scope
            "missing.svg",                   // declared but not on disk
            "mark.ico",                      // a multi-size container with no honest `sizes`
        ] {
            let icons = resolve_icons(&dir, Some(src));
            assert_eq!(icons.favicon, None, "{src} should not become an app icon");
            assert!(
                icons.ships_bundled(),
                "{src} must fall back to the bundled set"
            );
            assert!(
                icons.maskable,
                "the bundled set always has a maskable variant"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn start_url_never_points_at_a_page_that_does_not_exist() {
        // `display: standalone` removes the address bar, so an installed app that cold-
        // launches into a 404 has no way out. `./` is only correct when something answers
        // at the directory index.
        assert_eq!(start_url_for([]), "./");
        assert_eq!(
            start_url_for(["about.html", "index.html"]),
            "./",
            "an index exists, so the directory index is the right entry"
        );
        assert_eq!(
            start_url_for(["intro.html", "api.html"]),
            "intro.html",
            "no index.html: open the first page rather than a 404"
        );
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
