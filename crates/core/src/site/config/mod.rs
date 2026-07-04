//! Project config (`_site.yml`). The flat native schema is the only model.
//!
//! Native schema (everything top-level, HTML-only so no `format: html:` nesting):
//!
//! ```yaml
//! title: "My Site"
//! description: "…"
//! url: "https://…"            # site URL
//! favicon: favicon.svg
//! output: _site              # build output dir
//! toc: true
//! css: custom.css
//! head:  head.html           # include-in-header
//! body-end: body.html        # include-after-body  (also: body-start)
//! nav:                       # a list ⇒ left side; or { left: […], right: […] }
//!   - { text: Blog, href: blog.tmd }
//! footer:                    # a string ⇒ left text; or { left/center/right }
//!   right: [{ icon: github, href: "…" }]
//! chapters: [index.tmd, …]   # presence ⇒ a book (no `type:` needed)
//! ```

use super::*;
use serde::Deserialize;

/// The resolved project config — the single internal model every downstream
/// consumer reads.
#[derive(Debug, Clone, Default)]
pub struct SiteConfig {
    /// `chapters:` present ⇒ a book (a centred reading column + chapter drawer, no navbar).
    pub is_book: bool,
    /// `build` output dir (default `_site`, or `_book` for a book).
    pub output_dir: Option<String>,
    pub title: Option<String>,
    pub author: Option<serde_yaml::Value>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub favicon: Option<String>,
    /// Default social-card image (native `image:`, Quarto `open-graph: image:`),
    /// used for `og:image`/`twitter:image` when a page sets no `image:` of its own.
    pub card_image: Option<String>,
    pub toc: Option<bool>,
    pub css: Option<serde_yaml::Value>,
    /// `head` → include-in-header; `body-start`/`body-end` → before/after body.
    pub head: Option<serde_yaml::Value>,
    pub body_start: Option<serde_yaml::Value>,
    pub body_end: Option<serde_yaml::Value>,
    pub nav: Navbar,
    pub footer: Option<Footer>,
    /// Ordered chapter list (book only): a file name or `{ part, chapters }`.
    pub chapters: Vec<serde_yaml::Value>,
    /// `mounts:` — other taliesin projects to mount under a URL prefix, so a site
    /// can link to e.g. a separate docs `book` at `/docs`. In `preview` they're
    /// served live; the static `build` recipe wires them with a second `build`.
    pub mounts: Vec<Mount>,
}

/// One `mounts:` entry: serve the project at `path` (relative to the site root)
/// under the `/at/` URL prefix.
#[derive(Debug, Clone)]
pub struct Mount {
    pub at: String,
    pub path: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Navbar {
    #[serde(default)]
    pub left: Vec<NavItem>,
    #[serde(default)]
    pub right: Vec<NavItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Footer {
    #[serde(default)]
    pub left: Vec<NavItem>,
    #[serde(default)]
    pub center: Vec<NavItem>,
    #[serde(default)]
    pub right: Vec<NavItem>,
}

/// A navbar/footer entry. `text` is the label; `href` the link; `icon` a bundled
/// social glyph name (github / linkedin / rss / …) rendered as an inline SVG.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NavItem {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub href: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

/// Every recognized top-level native key (drives typo validation).
pub(crate) const NATIVE_KEYS: &[&str] = &[
    "title",
    "author",
    "description",
    "url",
    "favicon",
    "image",
    "output",
    "toc",
    "css",
    "head",
    "body-start",
    "body-end",
    "nav",
    "footer",
    "chapters",
    "mounts",
];

/// Stable prefix on the warning a malformed `_site.yml` pushes. A malformed config is a
/// *real* error (the site silently degrades to defaults), distinct from a legitimately
/// absent `_site.yml`. The site build matches this prefix to count a malformed config as a
/// `--strict` problem, and the live preview watcher matches it to keep the last-good site
/// instead of replacing it with the degraded default. Keep it stable: those consumers key
/// off it (see `crates/server/src/build.rs` + `serve_site/mod.rs`).
pub const MALFORMED_CONFIG_PREFIX: &str = "_site.yml is not valid YAML";

/// Load + parse `_site.yml` at `root` into the native flat schema.
pub(in crate::site) fn load_config(root: &Path, warnings: &mut Vec<String>) -> SiteConfig {
    let path = root.join("_site.yml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        // A missing `_site.yml` is legitimate (a bare directory of `.tmd` pages), not an
        // error — distinct from the malformed case below, which downstream counts.
        warnings.push(format!("no _site.yml at {}", root.display()));
        return SiteConfig::default();
    };
    let value: serde_yaml::Value = match serde_yaml::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            // Malformed YAML: degrade to defaults but tag the warning so the build can
            // fail `--strict` on it and the preview can keep its last-good config.
            warnings.push(format!("{MALFORMED_CONFIG_PREFIX}: {e}"));
            return SiteConfig::default();
        }
    };
    parse_native(&value, warnings)
}

/// Whether a discovery warning is the malformed-`_site.yml` marker (a real error, not the
/// benign "no _site.yml" case). Shared by the server's build + watcher.
pub fn is_malformed_config_warning(warning: &str) -> bool {
    warning.starts_with(MALFORMED_CONFIG_PREFIX)
}

fn parse_native(value: &serde_yaml::Value, warnings: &mut Vec<String>) -> SiteConfig {
    validate_keys(value, warnings);
    let str_of = |k: &str| value.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let chapters = value
        .get("chapters")
        .and_then(|v| v.as_sequence())
        .cloned()
        .unwrap_or_default();
    SiteConfig {
        is_book: !chapters.is_empty(),
        output_dir: str_of("output"),
        title: str_of("title"),
        author: value.get("author").cloned(),
        description: str_of("description"),
        url: str_of("url"),
        favicon: str_of("favicon"),
        card_image: str_of("image"),
        toc: value.get("toc").and_then(|v| v.as_bool()),
        css: value.get("css").cloned(),
        head: value.get("head").cloned(),
        body_start: value.get("body-start").cloned(),
        body_end: value.get("body-end").cloned(),
        nav: nav_from(value.get("nav")),
        footer: footer_from(value.get("footer")),
        chapters,
        mounts: mounts_from(value.get("mounts")),
    }
}

/// Parse `mounts:` — a map `{ docs: ../docs }` or a sequence of `{ at, path }`.
fn mounts_from(v: Option<&serde_yaml::Value>) -> Vec<Mount> {
    match v {
        Some(serde_yaml::Value::Mapping(m)) => m
            .iter()
            .filter_map(|(k, val)| {
                Some(Mount {
                    at: k.as_str()?.trim_matches('/').to_string(),
                    path: val.as_str()?.to_string(),
                })
            })
            .collect(),
        Some(serde_yaml::Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|it| {
                Some(Mount {
                    at: it.get("at")?.as_str()?.trim_matches('/').to_string(),
                    path: it.get("path")?.as_str()?.to_string(),
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Warn on unrecognized top-level keys (the native schema is a closed set), with a
/// "did you mean" for near-misses.
fn validate_keys(value: &serde_yaml::Value, warnings: &mut Vec<String>) {
    let Some(map) = value.as_mapping() else {
        return;
    };
    for k in map.keys() {
        let Some(key) = k.as_str() else { continue };
        if !NATIVE_KEYS.contains(&key) {
            let hint = crate::frontmatter::closest(key, NATIVE_KEYS)
                .map(|s| format!(" (did you mean `{s}`?)"))
                .unwrap_or_default();
            warnings.push(format!("unknown config key `{key}`{hint}"));
        }
    }
}

/// `nav:` is a list of items (the left side) or `{ left: […], right: […] }`.
fn nav_from(v: Option<&serde_yaml::Value>) -> Navbar {
    match v {
        Some(v) if v.is_mapping() => Navbar {
            left: items(v.get("left")),
            right: items(v.get("right")),
        },
        Some(v) => Navbar {
            left: items(Some(v)),
            right: Vec::new(),
        },
        None => Navbar::default(),
    }
}

/// `footer:` is a string (a single left label) or `{ left/center/right }`, each of
/// which is a string, a single item, or a list.
fn footer_from(v: Option<&serde_yaml::Value>) -> Option<Footer> {
    match v {
        Some(v) if v.is_mapping() => Some(Footer {
            left: items(v.get("left")),
            center: items(v.get("center")),
            right: items(v.get("right")),
        }),
        Some(v) => Some(Footer {
            left: items(Some(v)),
            ..Footer::default()
        }),
        None => None,
    }
}

/// Coerce a value into a list of [`NavItem`]: a string → one text item, a single
/// `{…}` → one item, a list → many. Bare strings *inside* a list are handled too
/// (they would otherwise fail to deserialize into a struct and be silently dropped).
fn items(v: Option<&serde_yaml::Value>) -> Vec<NavItem> {
    match v {
        None => Vec::new(),
        Some(serde_yaml::Value::Sequence(seq)) => seq.iter().filter_map(nav_item).collect(),
        Some(v) => nav_item(v).into_iter().collect(),
    }
}

/// One nav/footer entry from a YAML value: a bare string becomes a text label; a
/// `{…}` mapping deserializes into a [`NavItem`].
fn nav_item(v: &serde_yaml::Value) -> Option<NavItem> {
    match v {
        serde_yaml::Value::String(s) => Some(NavItem {
            text: Some(s.clone()),
            ..NavItem::default()
        }),
        other => serde_yaml::from_value(other.clone()).ok(),
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("tali-cfg-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn malformed_site_yml_pushes_tagged_warning_distinct_from_missing() {
        // A malformed `_site.yml` is a real error: it degrades to defaults AND tags its
        // warning so the build/preview can treat it differently from a missing file.
        let dir = tmp("malformed");
        // Unterminated double-quoted scalar -> serde_yaml parse error.
        std::fs::write(dir.join("_site.yml"), "title: \"unterminated\nfoo: bar\n").unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        assert!(cfg.title.is_none(), "malformed config degrades to default");
        assert!(
            warnings.iter().any(|w| is_malformed_config_warning(w)),
            "malformed YAML must be tagged: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_site_yml_is_not_a_malformed_config() {
        // A bare directory with no `_site.yml` is legitimate; its warning must NOT match
        // the malformed marker (so the build doesn't fail `--strict` on a missing file).
        let dir = tmp("missing");
        let mut warnings = Vec::new();
        let _ = load_config(&dir, &mut warnings);
        assert!(
            warnings.iter().any(|w| w.starts_with("no _site.yml")),
            "missing config warns: {warnings:?}"
        );
        assert!(
            !warnings.iter().any(|w| is_malformed_config_warning(w)),
            "a missing file must not be reported as malformed: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn valid_site_yml_has_no_config_warnings() {
        let dir = tmp("valid");
        std::fs::write(dir.join("_site.yml"), "title: My Site\n").unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        assert_eq!(cfg.title.as_deref(), Some("My Site"));
        assert!(
            !warnings.iter().any(|w| is_malformed_config_warning(w)),
            "a valid config is not malformed: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
