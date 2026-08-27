//! HTML / sourcepos helpers shared by more than one validator family.

/// 1-based start line from a block's `sourcepos` (`"startLine:col-..."`), if positive.
pub(crate) fn start_line(sourcepos: &str) -> Option<u32> {
    sourcepos
        .split(':')
        .next()?
        .parse::<u32>()
        .ok()
        .filter(|&l| l > 0)
}

/// Every value of the attribute called `name` (e.g. `"id"`) found in `html`, appended to
/// `out`. [`crate::render::attr_values`] is the one reader — see there for why a validator
/// may not spell this as a `name="` needle of its own.
pub(crate) fn collect_attr_values<'a>(
    html: &'a str,
    name: &'a str,
    out: &mut std::collections::HashSet<&'a str>,
) {
    out.extend(crate::render::attr_values(html, name));
}

/// Whether `v` is a local file reference, i.e. not external, an in-page anchor, a data
/// URI, or a non-file scheme. (Mirrors the asset-bundling heuristic in the build path.)
pub(crate) fn is_local_ref(v: &str) -> bool {
    !v.is_empty()
        && !v.starts_with('#')
        && !v.starts_with("//")
        && !v.contains("://")
        && !v.starts_with("data:")
        && !v.starts_with("mailto:")
        && !v.starts_with("tel:")
        && !v.starts_with("vscode:")
        && !v.starts_with("javascript:")
}

/// The heading level (1..=6) of a block whose HTML opens with `<h1>`..`<h6>`, else `None`.
///
/// The renderer's own definition, re-exported rather than re-derived: this module carried a
/// third copy that accepted `<h1abc` and `<h7`, and a heading test that disagrees with the
/// renderer about what a heading is prices every outline diagnostic wrong.
pub(crate) use crate::render::block_heading_level as heading_level;

/// The replacement from an inline "did you mean `X`?" hint, e.g. `treme` -> `theme`,
/// `@fig-reslts` -> `@fig-results`. `None` when the message carries no such hint.
///
/// A plain scan over the message, independent of which validator wrote it: every
/// did-you-mean in this tree is spelled the same way on purpose, and this is what turns one
/// into a structured `suggestion` an editor can apply as a quick fix — so a message that
/// spells the phrase is promising a mechanical rename, and must not use it for anything
/// else.
pub fn extract_suggestion(message: &str) -> Option<String> {
    let key = "did you mean `";
    let at = message.find(key)? + key.len();
    let rest = &message[at..];
    let end = rest.find('`')?;
    Some(rest[..end].to_string())
}
