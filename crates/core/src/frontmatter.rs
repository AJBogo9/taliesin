//! Front-matter schema + linting.
//!
//! Quarto (and qmd-fast) read a leading YAML `---` block. Quarto silently
//! ignores keys it doesn't recognize, so a typo like `treme:` just does nothing
//! and the author never finds out. [`lint`] parses the block and flags every
//! unknown top-level key, suggesting the closest known one. It only warns;
//! rendering is unaffected (an unknown key still renders), so a Quarto document
//! using keys qmd-fast doesn't implement still works.

/// Top-level front-matter keys qmd-fast recognizes: the closed set of keys it
/// actually implements, plus every key the corpus/docs use. Intentionally tight
/// (Phase 3 of the Quarto drop), so a key qmd-fast doesn't implement, or a typo,
/// now warns instead of being silently ignored. Only top-level keys are linted;
/// nested keys (under `format:`, `execute:`, `about:`, `listing:`, `hero:`) are not.
const KNOWN_KEYS: &[&str] = &[
    // Identity / metadata
    "title",
    "subtitle",
    "author",
    "date",
    "description",
    "lang",
    "categories",
    // Images / social
    "image",
    "image-alt",
    // Output / format / theme
    "format",
    "theme",
    "css",
    "extensions",
    "page-layout",
    "title-block-banner",
    "title-block-style",
    "include-in-header",
    "include-before-body",
    "include-after-body",
    // Table of contents
    "toc",
    // Citations
    "bibliography",
    "csl",
    // Execution
    "execute",
    // Listings / project pages
    "listing",
    "about",
    "hero",
    "site-url",
];

/// Lint a document's front matter, returning one warning per unknown top-level
/// key (with a "did you mean" suggestion when a known key is close). Empty when
/// there is no front matter, it is valid, or it isn't a key/value mapping.
pub fn lint(src: &str) -> Vec<String> {
    let Some(block) = front_matter_block(src) else {
        return Vec::new();
    };
    if block.trim().is_empty() {
        return Vec::new();
    }
    let value: serde_yaml::Value = match serde_yaml::from_str(block) {
        Ok(v) => v,
        Err(e) => return vec![format!("front matter is not valid YAML: {e}")],
    };
    let Some(map) = value.as_mapping() else {
        return Vec::new();
    };
    let mut warnings = Vec::new();
    for key in map.keys().filter_map(|k| k.as_str()) {
        if KNOWN_KEYS.contains(&key) {
            continue;
        }
        warnings.push(match closest_known(key) {
            Some(s) => format!("unknown front-matter key `{key}` (did you mean `{s}`?)"),
            None => format!("unknown front-matter key `{key}`"),
        });
    }
    warnings
}

/// If the document has front matter that is present but not valid YAML, return the
/// parse-error message and its 1-based line in the SOURCE FILE. The front-matter
/// block starts on the line after the opening `---`, so a YAML line `L` is file
/// line `L + 1`; `serde_yaml` locations are 1-based, and we fall back to the fence
/// line when the error carries none. `None` when there is no front matter or it
/// parses cleanly. Powers a located, click-to-source diagnostic in the dev server.
pub fn yaml_error(src: &str) -> Option<(String, u32)> {
    let block = front_matter_block(src)?;
    if block.trim().is_empty() {
        return None;
    }
    match serde_yaml::from_str::<serde_yaml::Value>(block) {
        Ok(_) => None,
        Err(e) => {
            let line = e.location().map(|l| l.line() as u32 + 1).unwrap_or(1);
            Some((format!("front matter is not valid YAML: {e}"), line))
        }
    }
}

/// The leading `---` ... `---`/`...` block of a document, without the fences.
/// `None` if the source doesn't open with a front-matter fence. The one canonical
/// front-matter splitter (BOM- and `...`-terminator-aware); the site parser and the
/// shortcode/extension scanner reuse it so every path agrees on edge cases.
pub(crate) fn front_matter_block(src: &str) -> Option<&str> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let first = src.split_inclusive('\n').next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let after = first.len();
    let mut pos = after;
    for line in src[after..].split_inclusive('\n') {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            return Some(&src[after..pos]);
        }
        pos += line.len();
    }
    None
}

/// The known key closest to `key` within an edit distance of 2 (likely a typo).
fn closest_known(key: &str) -> Option<&'static str> {
    closest(key, KNOWN_KEYS)
}

/// The candidate within edit distance 2 of `key` (a "did you mean"), or `None`.
/// Shared by the front-matter linter and the project-config validator.
pub(crate) fn closest(key: &str, candidates: &[&'static str]) -> Option<&'static str> {
    candidates
        .iter()
        .copied()
        .map(|k| (levenshtein(key, k), k))
        .filter(|&(d, _)| d > 0 && d <= 2)
        .min_by_key(|&(d, _)| d)
        .map(|(_, k)| k)
}

/// Plain Levenshtein edit distance (two-row DP).
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_typo_with_suggestion() {
        let w = lint("---\ntreme: darkly\ntitle: X\n---\n\nbody\n");
        assert_eq!(w.len(), 1, "got: {w:?}");
        assert!(
            w[0].contains("`treme`") && w[0].contains("did you mean `theme`"),
            "got: {w:?}"
        );
    }

    #[test]
    fn yaml_error_reports_the_file_line() {
        // `: x` after a value on line 3 of the FILE (line 2 of the YAML block) is a
        // mapping error; the reported line is offset past the opening `---`.
        let (msg, line) = yaml_error("---\ntitle: ok\nbad: : x\n---\n\nbody\n").expect("an error");
        assert!(msg.contains("not valid YAML"), "got: {msg}");
        assert_eq!(line, 3, "should point at the file line, past the fence");
    }

    #[test]
    fn yaml_error_none_when_valid_or_absent() {
        assert!(yaml_error("---\ntitle: X\n---\n\nbody\n").is_none());
        assert!(yaml_error("no front matter\n").is_none());
    }

    #[test]
    fn clean_front_matter_has_no_warnings() {
        // top-level known keys + a nested `format:` block (nested keys not linted)
        let w = lint(
            "---\ntitle: X\nsubtitle: Y\ntoc: true\ncategories: [a, b]\nformat:\n  html:\n    toc: true\n    theme: darkly\n---\n\nx\n",
        );
        assert!(w.is_empty(), "got: {w:?}");
    }

    #[test]
    fn unimplemented_quarto_key_now_warns() {
        // A valid Quarto key qmd-fast doesn't implement (dropped from KNOWN_KEYS in
        // the Phase-3 schema close) now warns rather than being silently ignored.
        let w = lint("---\ntitle: X\nnumber-sections: true\n---\n");
        assert_eq!(w.len(), 1, "got: {w:?}");
        assert!(w[0].contains("`number-sections`"), "got: {w:?}");
    }

    #[test]
    fn unknown_key_without_close_match_has_no_suggestion() {
        let w = lint("---\ntitle: X\nzzzcustomplugin: 1\n---\n");
        assert_eq!(w.len(), 1, "got: {w:?}");
        assert!(
            w[0].contains("`zzzcustomplugin`") && !w[0].contains("did you mean"),
            "got: {w:?}"
        );
    }

    #[test]
    fn no_front_matter_yields_no_warnings() {
        assert!(lint("# Just a heading\n\ntext\n").is_empty());
        assert!(lint("").is_empty());
    }

    #[test]
    fn malformed_yaml_is_reported_not_panicked() {
        let w = lint("---\ntitle: X\n: : :\n---\n");
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("not valid YAML"), "got: {w:?}");
    }
}
