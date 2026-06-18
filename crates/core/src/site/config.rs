//! Project config: the typed `_quarto.yml` model (`website:`/`book:`/`format:`)
//! and its tolerant parser. Split out of the site module; `use super::*` reaches
//! the shared imports.

use super::*;
use serde::Deserialize;

/// The root project config, parsed from `_quarto.yml`. Only the subset qmd-fast
/// understands is modelled; unknown keys are ignored (Quarto compatibility —
/// a real config carries far more than this foundation consumes).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SiteConfig {
    #[serde(default)]
    pub project: ProjectSection,
    #[serde(default)]
    pub website: WebsiteSection,
    #[serde(default)]
    pub book: BookSection,
    #[serde(default)]
    pub format: FormatSection,
}
/// The `format:` block. Only `html:` is read (qmd-fast is HTML-only); within it,
/// the `include-*` / `css` keys are honoured site-wide (other keys are ignored).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FormatSection {
    #[serde(default)]
    pub html: FormatHtml,
}
/// Site-wide `format: html:` asset injection (applied to every page). Each value
/// is left as raw YAML and resolved by `render::includes_from_parts` (a path
/// string, a `{text:}`/`{file:}` map, or a list of those; `css` files inlined).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FormatHtml {
    /// Site-wide `toc:` default (applied to article pages — not listing/about ones).
    #[serde(default)]
    pub toc: Option<bool>,
    #[serde(default, rename = "include-in-header")]
    pub include_in_header: Option<serde_yaml::Value>,
    #[serde(default, rename = "include-before-body")]
    pub include_before_body: Option<serde_yaml::Value>,
    #[serde(default, rename = "include-after-body")]
    pub include_after_body: Option<serde_yaml::Value>,
    #[serde(default)]
    pub css: Option<serde_yaml::Value>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectSection {
    /// `website` (default) or `book`.
    #[serde(default, rename = "type")]
    pub project_type: Option<String>,
    /// Where `build` writes the site (default `_site`, or `_book` for a book).
    #[serde(default, rename = "output-dir")]
    pub output_dir: Option<String>,
}
/// The `book:` block (only present for `project: type: book`). `chapters` is an
/// ordered list whose entries are either a chapter file name or a
/// `{ part: <name>, chapters: [...] }` group.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BookSection {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub author: Option<serde_yaml::Value>,
    #[serde(default)]
    pub chapters: Vec<serde_yaml::Value>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WebsiteSection {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "site-url")]
    pub site_url: Option<String>,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub navbar: Navbar,
    #[serde(default, rename = "page-footer")]
    pub page_footer: Option<Footer>,
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
/// A navbar/footer entry. `text` is the label (plain text in the navbar, allowed
/// raw HTML in the footer for icon SVGs); `href` is its link.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NavItem {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub href: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}
/// Load + parse `_quarto.yml` at `root`, tolerating malformed sections (warn,
/// don't reject — Quarto configs carry keys/shapes we don't model).
pub(super) fn load_config(root: &Path, warnings: &mut Vec<String>) -> SiteConfig {
    let path = root.join("_quarto.yml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        warnings.push(format!("no _quarto.yml at {}", root.display()));
        return SiteConfig::default();
    };
    // Parse to a generic value first, then deserialize each known section on its
    // own, so one unfamiliar section can't sink the whole config.
    let root_val: serde_yaml::Value = match serde_yaml::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!("_quarto.yml is not valid YAML: {e}"));
            return SiteConfig::default();
        }
    };
    let mut cfg = SiteConfig::default();
    if let Some(v) = root_val.get("project").cloned() {
        match serde_yaml::from_value(v) {
            Ok(p) => cfg.project = p,
            Err(e) => warnings.push(format!("ignoring malformed `project` config: {e}")),
        }
    }
    if let Some(v) = root_val.get("website").cloned() {
        match serde_yaml::from_value(v) {
            Ok(w) => cfg.website = w,
            Err(e) => warnings.push(format!("ignoring malformed `website` config: {e}")),
        }
    }
    if let Some(v) = root_val.get("book").cloned() {
        match serde_yaml::from_value(v) {
            Ok(b) => cfg.book = b,
            Err(e) => warnings.push(format!("ignoring malformed `book` config: {e}")),
        }
    }
    if let Some(v) = root_val.get("format").cloned() {
        match serde_yaml::from_value(v) {
            Ok(f) => cfg.format = f,
            Err(e) => warnings.push(format!("ignoring malformed `format` config: {e}")),
        }
    }
    cfg
}
