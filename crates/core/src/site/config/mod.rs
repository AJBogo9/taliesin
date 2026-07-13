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
    /// Site-level `image:` (also accepted as `open-graph: image:`). This used to be
    /// the default `og:image`/`twitter:image` when a page set no `image:` of its
    /// own; that role is now filled entirely by the auto-generated per-page social
    /// card (`card::card_url`), so this field has no `og:image`/`twitter:image`
    /// consequence anymore — it is still parsed from `_site.yml` but not read for
    /// meta tags. Removing the field outright is a separate change.
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
    /// `publish:` deploy target for `taliesin publish` (absent unless configured).
    pub publish: Option<PublishConfig>,
    /// Project-pinned Python interpreter (`python:` in `_site.yml`), highest
    /// precedence in interpreter resolution. `None` falls back to `.venv`/env/default.
    pub python: Option<String>,
    /// Project-pinned R interpreter (`r:` in `_site.yml`). `None` falls back to env/`R`.
    pub r: Option<String>,
}

/// One `mounts:` entry: serve the project at `path` (relative to the site root)
/// under the `/at/` URL prefix.
#[derive(Debug, Clone)]
pub struct Mount {
    pub at: String,
    pub path: String,
}

/// `publish:` says where `taliesin publish` deploys this project. Optional; when
/// absent, publish falls back to a slug of the project directory name. The passcode is
/// never stored here (it lives only as a Cloudflare Pages secret).
#[derive(Debug, Clone, Default)]
pub struct PublishConfig {
    /// Deploy provider. Only `cloudflare` is recognized today.
    pub provider: Option<String>,
    /// Cloudflare Pages project name (overrides the dir-name slug default).
    pub project: Option<String>,
    /// Passcode gate. Absent or `true` = gated (the safe default); `false` = public.
    pub gate: Option<bool>,
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
    "publish",
    "python",
    "r",
];

/// `nav:` section keys (the `{ left, right }` mapping form). A typo here silently drops
/// the whole side, so it warns.
const NAV_SECTION_KEYS: &[&str] = &["left", "right"];
/// `footer:` section keys (the `{ left, center, right }` mapping form).
const FOOTER_SECTION_KEYS: &[&str] = &["left", "center", "right"];
/// The keys of a single nav/footer item (`{ text, href, icon }`).
const NAV_ITEM_KEYS: &[&str] = &["text", "href", "icon"];
/// The keys of a `mounts:` sequence entry (`{ at, path }`).
const MOUNT_ITEM_KEYS: &[&str] = &["at", "path"];
/// The keys of the `publish:` block (`{ provider, project, gate }`).
pub(crate) const PUBLISH_KEYS: &[&str] = &["provider", "project", "gate"];

/// Stable prefix on the warning a malformed `_site.yml` pushes. A malformed config is a
/// *real* error (the site silently degrades to defaults), distinct from a legitimately
/// absent `_site.yml`. The site build matches this prefix to count a malformed config as a
/// `--strict` problem, and the live preview watcher matches it to keep the last-good site
/// instead of replacing it with the degraded default. Keep it stable: those consumers key
/// off it (see `crates/server/src/build.rs` + `serve_site/mod.rs`).
pub const MALFORMED_CONFIG_PREFIX: &str = "_site.yml is not valid YAML";

/// Stable prefix on the advisory a *missing* `_site.yml` pushes. A bare directory of `.tmd`
/// pages is a legitimate project, so this is a note rather than a defect: `build` already
/// declines to count it toward `--strict`, and `check` must not fail on it either. Keep it
/// stable (see `crates/server/src/check.rs`).
pub const MISSING_CONFIG_PREFIX: &str = "no _site.yml at";

/// Load + parse `_site.yml` at `root` into the native flat schema.
pub(in crate::site) fn load_config(root: &Path, warnings: &mut Vec<String>) -> SiteConfig {
    let path = root.join("_site.yml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        // A missing `_site.yml` is legitimate (a bare directory of `.tmd` pages), not an
        // error — distinct from the malformed case below, which downstream counts.
        warnings.push(format!("{MISSING_CONFIG_PREFIX} {}", root.display()));
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

/// Whether a discovery warning is the benign "this directory has no `_site.yml`" advisory,
/// as opposed to a real defect. `check` uses it to keep an advisory out of its problem tally.
pub fn is_missing_config_warning(warning: &str) -> bool {
    warning.starts_with(MISSING_CONFIG_PREFIX)
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
        publish: publish_from(value.get("publish")),
        python: str_of("python"),
        r: str_of("r"),
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

/// A ` (did you mean `x`?)` suffix for a near-miss key, else empty.
fn did_you_mean(key: &str, candidates: &[&'static str]) -> String {
    crate::frontmatter::closest(key, candidates)
        .map(|s| format!(" (did you mean `{s}`?)"))
        .unwrap_or_default()
}

/// Warn on unrecognized keys against the closed native schema: top-level, and the
/// nested `nav:`/`footer:`/`mounts:`/`publish:` structures (a typo in one of those
/// silently drops the whole section/item, so it warns with a "did you mean"). Every
/// warning is prefixed `_site.yml` so it is file-located rather than an anonymous string.
fn validate_keys(value: &serde_yaml::Value, warnings: &mut Vec<String>) {
    let Some(map) = value.as_mapping() else {
        return;
    };
    let warn = |warnings: &mut Vec<String>, what: &str, key: &str, allowed: &[&'static str]| {
        warnings.push(format!(
            "_site.yml: unknown {what} `{key}`{}",
            did_you_mean(key, allowed)
        ));
    };
    for (k, v) in map {
        let Some(key) = k.as_str() else { continue };
        if !NATIVE_KEYS.contains(&key) {
            warn(warnings, "config key", key, NATIVE_KEYS);
            continue;
        }
        match key {
            "nav" => validate_nav_like(v, NAV_SECTION_KEYS, "nav", warnings),
            "footer" => validate_nav_like(v, FOOTER_SECTION_KEYS, "footer", warnings),
            "mounts" => validate_mounts(v, warnings),
            "publish" => validate_publish(v, warnings),
            _ => {}
        }
    }
}

/// Validate a `nav:`/`footer:` value: a `{ left/right/center }` mapping (section keys
/// checked, then each section's items), a bare list of items, or a string label
/// (nothing to check).
fn validate_nav_like(
    v: &serde_yaml::Value,
    section_keys: &[&'static str],
    ctx: &str,
    warnings: &mut Vec<String>,
) {
    match v {
        serde_yaml::Value::Mapping(m) => {
            for (k, section) in m {
                let Some(key) = k.as_str() else { continue };
                if section_keys.contains(&key) {
                    validate_items(section, ctx, warnings);
                } else {
                    warnings.push(format!(
                        "_site.yml: unknown {ctx} section `{key}`{}",
                        did_you_mean(key, section_keys)
                    ));
                }
            }
        }
        serde_yaml::Value::Sequence(_) => validate_items(v, ctx, warnings),
        _ => {}
    }
}

/// Validate one or a list of nav/footer items: each mapping's keys against
/// [`NAV_ITEM_KEYS`] (a bare string item is a plain label, nothing to check).
fn validate_items(v: &serde_yaml::Value, ctx: &str, warnings: &mut Vec<String>) {
    let items: Vec<&serde_yaml::Value> = match v {
        serde_yaml::Value::Sequence(seq) => seq.iter().collect(),
        other => vec![other],
    };
    for item in items {
        if let serde_yaml::Value::Mapping(m) = item {
            for k in m.keys().filter_map(|k| k.as_str()) {
                if !NAV_ITEM_KEYS.contains(&k) {
                    warnings.push(format!(
                        "_site.yml: unknown {ctx} item key `{k}`{}",
                        did_you_mean(k, NAV_ITEM_KEYS)
                    ));
                }
            }
        }
    }
}

/// Validate `mounts:` in its sequence form (`- { at, path }`); the mapping form
/// (`{ prefix: path }`) has author-chosen keys, so it can't be checked.
fn validate_mounts(v: &serde_yaml::Value, warnings: &mut Vec<String>) {
    let serde_yaml::Value::Sequence(seq) = v else {
        return;
    };
    for item in seq {
        if let serde_yaml::Value::Mapping(m) = item {
            for k in m.keys().filter_map(|k| k.as_str()) {
                if !MOUNT_ITEM_KEYS.contains(&k) {
                    warnings.push(format!(
                        "_site.yml: unknown mount key `{k}`{}",
                        did_you_mean(k, MOUNT_ITEM_KEYS)
                    ));
                }
            }
        }
    }
}

/// Validate the `publish:` mapping's keys against [`PUBLISH_KEYS`]. A typo silently
/// drops a setting (publish would fall back to a default), so it warns.
fn validate_publish(v: &serde_yaml::Value, warnings: &mut Vec<String>) {
    let serde_yaml::Value::Mapping(m) = v else {
        return;
    };
    for k in m.keys().filter_map(|k| k.as_str()) {
        if !PUBLISH_KEYS.contains(&k) {
            warnings.push(format!(
                "_site.yml: unknown publish key `{k}`{}",
                did_you_mean(k, PUBLISH_KEYS)
            ));
        }
    }
}

/// Parse the `publish:` mapping into [`PublishConfig`] (a non-mapping value yields None).
fn publish_from(v: Option<&serde_yaml::Value>) -> Option<PublishConfig> {
    let pv = v?;
    if !pv.is_mapping() {
        return None;
    }
    let s = |k: &str| pv.get(k).and_then(|x| x.as_str()).map(str::to_string);
    Some(PublishConfig {
        provider: s("provider"),
        project: s("project"),
        gate: pv.get("gate").and_then(|x| x.as_bool()),
    })
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
    fn parses_python_and_r_interpreter_pins() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("title: X\npython: .venv/bin/python\nr: /usr/bin/R\n").unwrap();
        let cfg = parse_native(&v, &mut w);
        assert_eq!(cfg.python.as_deref(), Some(".venv/bin/python"));
        assert_eq!(cfg.r.as_deref(), Some("/usr/bin/R"));
        assert!(w.is_empty(), "valid keys warn about nothing: {w:?}");
    }

    #[test]
    fn a_typod_interpreter_key_warns_via_native_keys() {
        let mut w = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str("pyton: .venv/bin/python\n").unwrap();
        let _ = parse_native(&v, &mut w);
        assert!(
            w.iter().any(|m| m.contains("pyton")),
            "an unknown config key must warn (did-you-mean python): {w:?}"
        );
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

    fn cfg_warnings(yml: &str) -> Vec<String> {
        // A unique dir per call: several tests hit `cfg_warnings` and run in parallel,
        // so a shared dir name would let one test's cleanup nuke another's `_site.yml`.
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = tmp(&format!("warn-{}", N.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(dir.join("_site.yml"), yml).unwrap();
        let mut warnings = Vec::new();
        let _ = load_config(&dir, &mut warnings);
        let _ = std::fs::remove_dir_all(&dir);
        warnings
    }

    #[test]
    fn unknown_top_level_key_is_located_at_site_yml() {
        // The warning must name `_site.yml` (it was previously anonymous) and suggest
        // the near-miss.
        let w = cfg_warnings("titel: My Site\n");
        assert!(
            w.iter().any(|w| w.starts_with("_site.yml:")
                && w.contains("`titel`")
                && w.contains("`title`")),
            "{w:?}"
        );
    }

    #[test]
    fn nested_nav_footer_mount_typos_warn_instead_of_silently_dropping() {
        // A `nav:` section typo drops the whole side silently — must warn.
        let w = cfg_warnings("nav:\n  lefft:\n    - text: Blog\n      href: blog.tmd\n");
        assert!(
            w.iter()
                .any(|w| w.contains("nav section `lefft`") && w.contains("`left`")),
            "nav section typo: {w:?}"
        );

        // A nav ITEM key typo drops the label/link silently.
        let w = cfg_warnings("nav:\n  left:\n    - txt: Blog\n      href: blog.tmd\n");
        assert!(
            w.iter()
                .any(|w| w.contains("nav item key `txt`") && w.contains("`text`")),
            "nav item typo: {w:?}"
        );

        // A `footer:` center is valid; a bogus footer section warns.
        let w = cfg_warnings("footer:\n  centre:\n    - text: hi\n");
        assert!(
            w.iter()
                .any(|w| w.contains("footer section `centre`") && w.contains("`center`")),
            "footer section typo: {w:?}"
        );

        // A `mounts:` sequence entry key typo drops the mount silently.
        let w = cfg_warnings("mounts:\n  - att: /docs\n    path: ../docs\n");
        assert!(
            w.iter()
                .any(|w| w.contains("mount key `att`") && w.contains("`at`")),
            "mount key typo: {w:?}"
        );
    }

    #[test]
    fn valid_nested_nav_footer_mounts_have_no_warnings() {
        // The real corpus shape: `{ left: [...], right: [...] }` with text/href items,
        // a footer with left/center/right, and a mounts sequence — none may warn.
        let w = cfg_warnings(concat!(
            "title: Site\n",
            "nav:\n  left:\n    - text: Blog\n      href: blog.tmd\n  right:\n    - icon: github\n      href: 'https://x'\n",
            "footer:\n  left:\n    - text: © 2026\n  center:\n    - text: mid\n  right:\n    - text: end\n",
            "mounts:\n  - at: /docs\n    path: ../docs\n",
        ));
        assert!(w.iter().all(|w| !w.contains("unknown")), "{w:?}");
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

    #[test]
    fn publish_block_parses_provider_and_project() {
        let dir = tmp("publish-ok");
        std::fs::write(
            dir.join("_site.yml"),
            "title: Book\npublish:\n  provider: cloudflare\n  project: my-book\n",
        )
        .unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        let publish = cfg.publish.expect("publish block parsed");
        assert_eq!(publish.provider.as_deref(), Some("cloudflare"));
        assert_eq!(publish.project.as_deref(), Some("my-book"));
        assert!(
            !warnings.iter().any(|w| w.contains("unknown")),
            "a valid publish block must not warn: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_publish_key_warns_with_did_you_mean() {
        // A typo in a publish key silently drops the setting, so it must warn.
        let w = cfg_warnings("publish:\n  provder: cloudflare\n");
        assert!(
            w.iter()
                .any(|w| w.contains("publish key `provder`") && w.contains("`provider`")),
            "publish key typo: {w:?}"
        );
    }

    #[test]
    fn publish_gate_false_parses() {
        let dir = tmp("publish-gate");
        std::fs::write(
            dir.join("_site.yml"),
            "title: Book\npublish:\n  provider: cloudflare\n  gate: false\n",
        )
        .unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        let publish = cfg.publish.expect("publish block parsed");
        assert_eq!(publish.gate, Some(false));
        assert!(
            !warnings.iter().any(|w| w.contains("unknown")),
            "a valid gate must not warn: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_gate_typo_warns() {
        let w = cfg_warnings("publish:\n  gat: false\n");
        assert!(
            w.iter()
                .any(|w| w.contains("publish key `gat`") && w.contains("`gate`")),
            "gate typo did-you-mean: {w:?}"
        );
    }

    #[test]
    fn absent_publish_block_is_none() {
        let dir = tmp("publish-absent");
        std::fs::write(dir.join("_site.yml"), "title: Book\n").unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        assert!(cfg.publish.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
