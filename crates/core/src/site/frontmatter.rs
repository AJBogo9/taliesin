//! Per-page `---` front-matter parsing (the fields discovery needs: title,
//! date, listings, about/hero blocks, page-layout). Split out of mod.rs.

use super::*;
use std::path::Path;

#[derive(Default)]
pub(crate) struct FrontInfo {
    pub(crate) title: Option<String>,
    pub(crate) date: Option<String>,
    pub(crate) description: Option<String>,
    /// Front-matter `author` (a scalar or a list), for scholarly `citation_author` meta.
    pub(crate) authors: Vec<String>,
    pub(crate) image: Option<String>,
    /// Front-matter `image-alt`: alt text for the listing card image.
    pub(crate) image_alt: Option<String>,
    pub(crate) categories: Vec<String>,
    pub(crate) listings: Vec<ListingSpec>,
    pub(crate) about: Option<AboutSpec>,
    pub(crate) hero: Option<HeroSpec>,
    pub(crate) page_layout: Option<String>,
    /// `draft: true` — excluded from a website build (output, nav, listings).
    pub(crate) draft: bool,
}

/// Parse a page's `---` front-matter block (YAML) into the fields discovery
/// needs. Tolerant: a missing or malformed block just yields defaults. `label` (the
/// page rel) tags any warning (`warnings`) raised while parsing, e.g. a `listing:`
/// with no `contents:`.
pub(crate) fn parse_front_matter(
    path: &Path,
    label: &str,
    warnings: &mut Vec<String>,
) -> FrontInfo {
    let Ok(src) = std::fs::read_to_string(path) else {
        return FrontInfo::default();
    };
    let Some(block) = crate::frontmatter::front_matter_block(&src) else {
        return FrontInfo::default();
    };
    let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(block) else {
        return FrontInfo::default();
    };
    FrontInfo {
        title: scalar(val.get("title")),
        date: scalar(val.get("date")),
        description: scalar(val.get("description")),
        authors: string_list(val.get("author")),
        image: scalar(val.get("image")),
        image_alt: scalar(val.get("image-alt")),
        categories: string_list(val.get("categories")),
        listings: parse_listings(val.get("listing"), label, warnings),
        about: parse_about(val.get("about")),
        hero: parse_hero(val.get("hero")),
        page_layout: scalar(val.get("page-layout")),
        draft: val
            .get("draft")
            .and_then(serde_yaml::Value::as_bool)
            .unwrap_or(false),
    }
}

/// Parse an `about:` mapping into a profile spec (template + image + links).
pub(crate) fn parse_about(v: Option<&serde_yaml::Value>) -> Option<AboutSpec> {
    let map = match v? {
        serde_yaml::Value::Mapping(_) => v?,
        _ => return None,
    };
    let links = match map.get("links") {
        Some(serde_yaml::Value::Sequence(seq)) => seq
            .iter()
            .map(|it| NavItem {
                text: scalar(it.get("text")),
                href: scalar(it.get("href")),
                icon: scalar(it.get("icon")),
            })
            .filter(|n| n.href.is_some())
            .collect(),
        _ => Vec::new(),
    };
    Some(AboutSpec {
        template: scalar(map.get("template")).unwrap_or_else(|| "jolla".to_string()),
        image: scalar(map.get("image")),
        image_alt: scalar(map.get("image-alt")),
        links,
    })
}

pub(crate) fn parse_hero(v: Option<&serde_yaml::Value>) -> Option<HeroSpec> {
    let map = match v? {
        serde_yaml::Value::Mapping(_) => v?,
        _ => return None,
    };
    let actions = match map.get("actions") {
        Some(serde_yaml::Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|it| {
                Some(HeroAction {
                    text: scalar(it.get("text"))?,
                    href: scalar(it.get("href"))?,
                    primary: it.get("primary").and_then(serde_yaml::Value::as_bool) == Some(true)
                        || scalar(it.get("class")).as_deref() == Some("primary"),
                })
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(HeroSpec {
        eyebrow: scalar(map.get("eyebrow")),
        headline: scalar(map.get("headline")),
        lead: scalar(map.get("lead")),
        actions,
    })
}

/// A YAML scalar (string/number/bool) as a display string.
pub(crate) fn scalar(v: Option<&serde_yaml::Value>) -> Option<String> {
    match v? {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// A YAML value that is either a single scalar or a sequence of scalars → a list
/// of strings (used for `categories`).
pub(crate) fn string_list(v: Option<&serde_yaml::Value>) -> Vec<String> {
    match v {
        Some(serde_yaml::Value::Sequence(seq)) => {
            seq.iter().filter_map(|x| scalar(Some(x))).collect()
        }
        Some(other) => scalar(Some(other)).into_iter().collect(),
        None => Vec::new(),
    }
}

/// Parse a `listing:` value: a single map, or a sequence of maps (cv.qmd). A map with
/// no `contents:` (nothing to list) is warned about via `warnings`, keyed by `label`
/// (the page rel), instead of being silently dropped.
pub(crate) fn parse_listings(
    v: Option<&serde_yaml::Value>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Vec<ListingSpec> {
    let maps: Vec<&serde_yaml::Value> = match v {
        Some(serde_yaml::Value::Sequence(seq)) => seq.iter().collect(),
        Some(map @ serde_yaml::Value::Mapping(_)) => vec![map],
        _ => return Vec::new(),
    };
    let mut specs = Vec::new();
    for m in maps {
        match parse_listing_spec(m) {
            Some(spec) => specs.push(spec),
            // A `listing:` mapping that parsed to nothing lacks `contents:`, so it
            // renders no cards — warn rather than drop it silently.
            None if m.is_mapping() => warnings.push(format!(
                "`{label}`: a `listing:` block has no `contents:` and was skipped (nothing to list)"
            )),
            None => {}
        }
    }
    specs
}

pub(crate) fn parse_listing_spec(v: &serde_yaml::Value) -> Option<ListingSpec> {
    // `contents` is what makes a listing renderable; without it there's nothing
    // to list (and we only support a single directory string for now).
    let contents = scalar(v.get("contents"))?;
    let sort_desc = scalar(v.get("sort"))
        .map(|s| !s.contains("asc"))
        .unwrap_or(true);
    let max_items = v
        .get("max-items")
        .and_then(serde_yaml::Value::as_u64)
        .map(|n| n as usize);
    Some(ListingSpec {
        id: scalar(v.get("id")),
        contents,
        grid: scalar(v.get("type")).as_deref() == Some("grid"),
        sort_desc,
        max_items,
        categories: v
            .get("categories")
            .and_then(|c| c.as_bool())
            .unwrap_or(false),
    })
}
