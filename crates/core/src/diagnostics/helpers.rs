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

/// Every value of attribute `attr` (e.g. `"id=\""`) found in `html`, appended to `out`.
pub(crate) fn collect_attr_values<'a>(
    html: &'a str,
    attr: &str,
    out: &mut std::collections::HashSet<&'a str>,
) {
    let mut i = 0;
    while let Some(pos) = html[i..].find(attr) {
        let start = i + pos + attr.len();
        let Some(len) = html[start..].find('"') else {
            break;
        };
        out.insert(&html[start..start + len]);
        i = start + len;
    }
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

/// The heading level (1..=6) of a block whose HTML opens with `<h1>`..`<h6>`, else
/// `None`. Reads only the second byte of the tag (`<hN`), the same shape the heading-id
/// check keys off.
pub(crate) fn heading_level(html: &str) -> Option<u8> {
    if !html.starts_with("<h") {
        return None;
    }
    let d = html.as_bytes().get(2)?;
    if d.is_ascii_digit() && (b'1'..=b'6').contains(d) {
        Some(d - b'0')
    } else {
        None
    }
}

/// The value of attribute `attr` (e.g. `"src=\""`) on the tag opened at the start of
/// `tag` (everything before the first `>`), if present. Used to read `src`/`poster` off
/// a `<video>`/`<source>` tag.
pub(crate) fn tag_attr<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let pos = tag.find(attr)? + attr.len();
    let rest = &tag[pos..];
    let len = rest.find('"')?;
    Some(&rest[..len])
}

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
