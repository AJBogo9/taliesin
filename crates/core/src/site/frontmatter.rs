//! Per-page `---` front-matter parsing (the fields discovery needs: title,
//! date, listings, about/hero blocks, page-layout). Split out of mod.rs.

use super::*;
use std::path::Path;

#[derive(Default)]
pub(crate) struct FrontInfo {
    pub(crate) title: Option<String>,
    pub(crate) date: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) image: Option<String>,
    /// Front-matter `image-alt`: alt text for the listing card image.
    pub(crate) image_alt: Option<String>,
    pub(crate) categories: Vec<String>,
    pub(crate) listings: Vec<ListingSpec>,
    pub(crate) hero: Option<HeroSpec>,
    /// `draft: true`: held out of the published view (`DraftMode::Exclude` — build,
    /// publish, check, map): no output, nav, listing or book-chapter entry, and the build
    /// reports it as "not published". The live preview (`DraftMode::Include`) keeps it,
    /// badged. Ignored on an `{{< embed >}}` target (it ships with the page embedding it).
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
    // Parsed for its DIAGNOSTICS only. Nothing in the site layer reads a page's authors
    // since JSON-LD was cut on 2026-08-08 — `render/mod.rs` parses `author:` again for the
    // byline — but an author sub-key typo is otherwise invisible (the value is just
    // dropped), and the page rel tags the message the way a `listing:` warning is tagged.
    warnings.extend(
        crate::author::parse(val.get("author"))
            .1
            .into_iter()
            .map(|m| format!("{label}: {m}")),
    );
    FrontInfo {
        title: scalar(val.get("title")),
        date: scalar(val.get("date")),
        description: scalar(val.get("description")),
        image: scalar(val.get("image")),
        image_alt: scalar(val.get("image-alt")),
        categories: string_list(val.get("categories")),
        listings: parse_listings(val.get("listing"), label, warnings),
        hero: parse_hero(val.get("hero")),
        draft: bool_field(&val, "draft", false, label, warnings),
    }
}

/// A boolean front-matter field that also catches the YAML-1.1 words serde_yaml
/// (which follows YAML 1.2) reads as plain STRINGS — `yes`/`no`/`on`/`off`. Without
/// this, `draft: yes` is a string, `as_bool()` is `None`, and the draft silently
/// PUBLISHES. Coerce them fail-safe and warn to use canonical `true`/`false`.
fn bool_field(
    val: &serde_yaml::Value,
    key: &str,
    default: bool,
    label: &str,
    warnings: &mut Vec<String>,
) -> bool {
    let Some(v) = val.get(key) else {
        return default;
    };
    if let Some(b) = v.as_bool() {
        return b;
    }
    if let Some(s) = v.as_str()
        && let Some(b) = crate::frontmatter::yaml_bool_word(s)
    {
        warnings.push(format!(
            "{label}: `{key}: {s}` is a string in YAML 1.2, not a boolean \u{2014} use `{key}: {b}`"
        ));
        return b;
    }
    default
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

/// Parse a `listing:` value: a single map, or a sequence of maps (cv.tmd). A map with
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
    let max_items = v
        .get("max-items")
        .and_then(serde_yaml::Value::as_u64)
        .map(|n| n as usize);
    let ty = scalar(v.get("type"));
    Some(ListingSpec {
        id: scalar(v.get("id")),
        contents,
        grid: ty.as_deref() == Some("grid"),
        with_image: matches!(ty.as_deref(), Some("grid") | Some("list")),
        max_items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `hero.image:`/`image-alt:` were retired on 2026-08-02 and their two-column layout
    /// deleted on 2026-08-08, so `parse_hero` no longer reads either key. Dropping a key
    /// from `HERO_KEYS` only makes it *diagnosed*; this pins that the parser really stopped
    /// consuming it, which is the half no vocabulary list can say.
    #[test]
    fn parse_hero_ignores_the_retired_image_keys() {
        let v: serde_yaml::Value = serde_yaml::from_str(
            "hero:\n  headline: H\n  image: profile.webp\n  image-alt: A face\n",
        )
        .unwrap();
        let h = parse_hero(v.get("hero")).expect("hero parses");
        assert_eq!(
            h.headline.as_deref(),
            Some("H"),
            "the live keys still parse"
        );
        assert!(
            !format!("{h:?}").contains("profile.webp"),
            "a retired `hero.image:` must not reach HeroSpec: {h:?}"
        );
    }
}
