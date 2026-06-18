//! Format extensions: `format: <ext>-revealjs|-html` loads
//! `_extensions/<ext>/_extension.yml` and injects the includes/theme/resources
//! its `contributes:` block declares. Kept in its own module so the core stays a
//! thin injector; `use super::*` reaches PageIncludes + the shared include/theme
//! helpers.

use super::*;

/// The raw `format:` key (`glass-revealjs`, `revealjs`, `html`, …) — inline value
/// or the first block sub-key. Used to spot a format-extension reference.
fn detect_format_name(front_matter: &str) -> Option<String> {
    let lines: Vec<&str> = front_matter.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some(rest) = line.trim_end().strip_prefix("format:") else {
            continue;
        };
        let inline = rest.trim();
        if !inline.is_empty() {
            return Some(inline.trim_matches(['"', '\'']).to_string());
        }
        // Block form: the first indented sub-key is the format name.
        for sub in &lines[i + 1..] {
            if sub.trim().is_empty() {
                continue;
            }
            if !sub.starts_with(char::is_whitespace) {
                break; // dedented out of the block without a sub-key
            }
            let key = sub.trim().trim_end_matches(':').trim_matches(['"', '\'']);
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }
        return None;
    }
    None
}
/// If the doc's `format:` names a format extension (`<ext>-revealjs`/`<ext>-html`),
/// load `_extensions/<ext>/_extension.yml` and resolve the includes + theme its
/// `contributes: formats: <base>:` block injects, with files resolved relative to
/// the extension's own directory. Empty when there's no such extension.
pub(super) fn resolve_format_extension(
    front_matter: &str,
    base_dir: Option<&Path>,
) -> PageIncludes {
    let Some(fmt) = detect_format_name(front_matter) else {
        return PageIncludes::default();
    };
    // Recognized base formats; the part before `-<base>` is the extension name.
    let Some((ext, base)) = ["revealjs", "html"]
        .iter()
        .find_map(|b| fmt.strip_suffix(&format!("-{b}")).map(|e| (e, *b)))
    else {
        return PageIncludes::default();
    };
    let (Some(dir), false) = (base_dir, ext.is_empty()) else {
        return PageIncludes::default();
    };
    let ext_dir = dir.join("_extensions").join(ext);
    let Ok(text) = std::fs::read_to_string(ext_dir.join("_extension.yml")) else {
        return PageIncludes::default();
    };
    let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
        return PageIncludes::default();
    };
    let Some(cfg) = v
        .get("contributes")
        .and_then(|c| c.get("formats"))
        .and_then(|f| f.get(base))
    else {
        return PageIncludes::default();
    };
    let mut inc = includes_from_parts(
        cfg.get("include-in-header"),
        cfg.get("include-before-body"),
        cfg.get("include-after-body"),
        cfg.get("css"),
        Some(&ext_dir),
    );
    // The contributed `theme:` CSS layers, inlined ahead of the header so the deck's
    // own front matter can still override. (`.scss` layers need a compiler we don't
    // ship yet, so only `.css` is inlined; named base themes are handled elsewhere.)
    let theme = resolve_theme_layers(cfg.get("theme"), &ext_dir);
    if !theme.is_empty() {
        inc.in_header = format!("{theme}{}", inc.in_header);
    }
    // `format-resources` (a scalar or list of file names relative to the extension)
    // are copied verbatim next to the output so an injected `<script src="x.js">`
    // resolves at runtime, rather than inlined.
    if let Some(res) = cfg.get("format-resources") {
        for name in res
            .as_sequence()
            .map(|s| s.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_else(|| res.as_str().into_iter().collect())
        {
            inc.resources.push(ext_dir.join(name));
        }
    }
    inc
}
