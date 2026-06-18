//! Quarto-`_quarto.yml`-shape compatibility shim. Translates the nested
//! `project:` / `website:` / `book:` / `format:` layout into the native
//! [`SiteConfig`].
//!
//! ISOLATED ON PURPOSE. To drop Quarto support entirely: delete this file and the
//! `quarto::from_value` dispatch branch in `mod.rs`. Nothing else references these
//! types, so the native path and every downstream consumer keep working.

use super::{Footer, Navbar, SiteConfig};
use serde::Deserialize;

#[derive(Deserialize, Default)]
struct Project {
    #[serde(rename = "type")]
    project_type: Option<String>,
    #[serde(rename = "output-dir")]
    output_dir: Option<String>,
}

#[derive(Deserialize, Default)]
struct Website {
    title: Option<String>,
    description: Option<String>,
    #[serde(rename = "site-url")]
    site_url: Option<String>,
    favicon: Option<String>,
    #[serde(default)]
    navbar: Navbar,
    #[serde(rename = "page-footer")]
    page_footer: Option<Footer>,
}

#[derive(Deserialize, Default)]
struct Book {
    title: Option<String>,
    author: Option<serde_yaml::Value>,
    #[serde(default)]
    chapters: Vec<serde_yaml::Value>,
}

#[derive(Deserialize, Default)]
struct Format {
    #[serde(default)]
    html: FormatHtml,
}

#[derive(Deserialize, Default)]
struct FormatHtml {
    toc: Option<bool>,
    #[serde(rename = "include-in-header")]
    include_in_header: Option<serde_yaml::Value>,
    #[serde(rename = "include-before-body")]
    include_before_body: Option<serde_yaml::Value>,
    #[serde(rename = "include-after-body")]
    include_after_body: Option<serde_yaml::Value>,
    css: Option<serde_yaml::Value>,
}

/// Translate a Quarto-shaped config into the native [`SiteConfig`]. Each section
/// deserializes on its own so one unfamiliar section can't sink the whole config.
pub(super) fn from_value(value: &serde_yaml::Value, warnings: &mut Vec<String>) -> SiteConfig {
    let project: Project = section(value, "project", warnings);
    let website: Website = section(value, "website", warnings);
    let book: Book = section(value, "book", warnings);
    let format: Format = section(value, "format", warnings);

    let is_book = project.project_type.as_deref() == Some("book") || !book.chapters.is_empty();
    SiteConfig {
        is_book,
        output_dir: project.output_dir,
        // book metadata lives under `book:`, site metadata under `website:`
        title: book.title.or(website.title),
        author: book.author,
        description: website.description,
        url: website.site_url,
        favicon: website.favicon,
        toc: format.html.toc,
        css: format.html.css,
        head: format.html.include_in_header,
        body_start: format.html.include_before_body,
        body_end: format.html.include_after_body,
        nav: website.navbar,
        footer: website.page_footer,
        chapters: book.chapters,
    }
}

fn section<T: Default + serde::de::DeserializeOwned>(
    value: &serde_yaml::Value,
    name: &str,
    warnings: &mut Vec<String>,
) -> T {
    match value.get(name) {
        None => T::default(),
        Some(v) => serde_yaml::from_value(v.clone()).unwrap_or_else(|e| {
            warnings.push(format!("ignoring malformed `{name}` config: {e}"));
            T::default()
        }),
    }
}
