//! Front-matter schema + linting.
//!
//! Quarto (and qmd-fast) read a leading YAML `---` block. Quarto silently
//! ignores keys it doesn't recognize, so a typo like `treme:` just does nothing
//! and the author never finds out. [`lint`] parses the block and flags every
//! unknown top-level key, suggesting the closest known one. It only warns;
//! rendering is unaffected (an unknown key still renders), so a Quarto document
//! using keys qmd-fast doesn't implement still works.

/// Top-level front-matter keys qmd-fast or Quarto recognize. Deliberately broad
/// (it includes keys qmd-fast doesn't implement yet) so a valid Quarto document
/// never warns; only genuinely unknown keys (typically typos) do. Nested keys
/// (e.g. under `format:` or `execute:`) are not linted.
const KNOWN_KEYS: &[&str] = &[
    // Identity / metadata
    "title",
    "subtitle",
    "author",
    "date",
    "date-modified",
    "description",
    "abstract",
    "keywords",
    "categories",
    "lang",
    "license",
    "copyright",
    "doi",
    "funding",
    // Images / social
    "image",
    "image-alt",
    "image-height",
    "image-width",
    // Output / format / theme
    "format",
    "theme",
    "css",
    "html-math-method",
    "page-layout",
    "title-block-banner",
    "title-block-style",
    "include-in-header",
    "include-before-body",
    "include-after-body",
    // Table of contents / numbering
    "toc",
    "toc-depth",
    "toc-title",
    "toc-location",
    "toc-expand",
    "number-sections",
    "number-depth",
    // Code
    "code-fold",
    "code-tools",
    "code-line-numbers",
    "code-overflow",
    "code-copy",
    "highlight-style",
    // Figures / cross-refs / citations
    "fig-cap",
    "fig-align",
    "fig-width",
    "fig-height",
    "fig-format",
    "fig-dpi",
    "crossref",
    "reference-location",
    "tbl-cap-location",
    "bibliography",
    "csl",
    "citation",
    "link-citations",
    // Execution
    "execute",
    "jupyter",
    "engine",
    "kernel",
    // Listings / project pages
    "listing",
    "about",
    "draft",
    "order",
    "aliases",
    "resources",
    "site-url",
    "search",
    "comments",
    // Misc presentation
    "smooth-scroll",
    "link-external-icon",
    "link-external-newwindow",
    "filters",
    "format-links",
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

/// The leading `---` ... `---`/`...` block of a document, without the fences.
/// `None` if the source doesn't open with a front-matter fence.
fn front_matter_block(src: &str) -> Option<&str> {
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
    KNOWN_KEYS
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
    fn clean_front_matter_has_no_warnings() {
        // top-level known keys + a nested `format:` block (nested keys not linted)
        let w = lint(
            "---\ntitle: X\nsubtitle: Y\ntoc: true\ncategories: [a, b]\nformat:\n  html:\n    toc: true\n    theme: darkly\n---\n\nx\n",
        );
        assert!(w.is_empty(), "got: {w:?}");
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
