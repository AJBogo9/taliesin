//! Bibliography-vs-citation mismatches: citations with no `bibliography:` declared,
//! the inverse blind spot (a bare `@key` that never became a citation), and the
//! recognized-but-inert `csl:` key.

use super::helpers::start_line;
use crate::render::{Block, Warning};
use std::path::Path;

/// Citations are present (`cite::process` appended the `qmd-references` section) but the
/// front matter declares no `bibliography:`, so every reference renders as a raw key with
/// no diagnostic today. (A declared-but-missing bibliography file is a separate warning.)
pub fn citations_without_bibliography(src: &str, blocks: &[Block]) -> Vec<Warning> {
    let has_citations = blocks.iter().any(|b| b.id == "qmd-references");
    if !has_citations {
        return Vec::new();
    }
    let declares_bib = crate::frontmatter::front_matter_block(src)
        .and_then(|fm| serde_yaml::from_str::<serde_yaml::Value>(fm).ok())
        .and_then(|v| v.as_mapping().map(|m| m.get("bibliography").is_some()))
        .unwrap_or(false);
    if declares_bib {
        return Vec::new();
    }
    vec![Warning::new(
        "citations are present but no `bibliography:` is declared, so every reference renders as a raw key",
    )]
}

/// `csl:` is recognized but not honored, so setting it is a no-op the author cannot see.
///
/// Nothing reads the value. Reference formatting is hardcoded IEEE-numeric (`cite::render`
/// emits `[n]`; `cite::author` implements the shipped `ieee.csl`'s own `et-al-min=7`), and
/// the `.csl` file's content is never parsed. The key was nonetheless advertised on four
/// surfaces: the front-matter allowlist, the editor completion, the JSON schema, and the
/// include resolver (which dutifully RESOLVES and watches the `.csl` file it will never
/// read). So the tool did not merely ignore `csl:`, it recommended it, and an author who
/// wrote `csl: apa.csl` got a clean `check` and IEEE output with no signal at all.
///
/// **Why a diagnostic rather than dropping the key:** `css` is edit distance 1 from `csl`,
/// so removing it from `KNOWN_KEYS` would make the unknown-key lint answer with "did you
/// mean `css`?", i.e. advise renaming a citation-style key to a stylesheet key. Recognized
/// + warned is the honest shape; see `frontmatter::UNSUPPORTED_KEYS`.
///
/// Located at the `csl:` line, since that is where the fix (deleting it) belongs. Carries
/// no "did you mean" hint on purpose: there is no replacement, and
/// `codes::extract_suggestion` would lift one into a structured fix an agent would apply.
pub fn csl_recognized_but_unsupported(src: &str) -> Vec<Warning> {
    let Some(block) = crate::frontmatter::front_matter_block(src) else {
        return Vec::new();
    };
    // A real YAML parse, so a `csl` mentioned in prose or nested under another key can't
    // trip it (the allowlist lint decides membership the same way).
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(block) else {
        return Vec::new(); // the parse error is reported by `frontmatter::yaml_error`
    };
    let Some(map) = value.as_mapping() else {
        return Vec::new();
    };
    if map.get("csl").is_none() {
        return Vec::new();
    }
    let w = Warning::new(
        "`csl:` is recognized but not supported, so it has no effect: references always \
         render in the built-in IEEE style (remove the key, or the citations will not \
         match the style you asked for)",
    );
    vec![match crate::frontmatter::block_key_line(block, "csl") {
        Some(line) => w.at(None, line),
        None => w,
    }]
}

/// A bare `@key` that names a real bibliography entry but shipped as literal prose.
///
/// Pandoc's bare `@key` citation form is not supported: only the bracketed `[@key]` is.
/// A bare one is not a citation, is not a cross-reference (`parse_xref` finds no known
/// `fig-`/`sec-` prefix), and so falls through to text. Nothing else catches this:
/// [`citations_without_bibliography`] fires only on the INVERSE case (a rendered
/// references section with no `bibliography:`), so a document whose every citation is
/// bare resolves zero of them, renders an empty References heading, and passes `check`.
/// That is exactly how `corpus/tech-blog/posts/a-star` shipped broken.
///
/// **Gated on bibliography membership**, which is what makes the rule safe. An unguarded
/// scan would eat `@media`, `@types/node` and e-mail addresses, because
/// `cite::is_cite_key_char` admits `/ . : +`. Only an `@word` whose key is actually in the
/// author's `.bib` can fire, so the warning is always actionable and never a guess.
///
/// Located to the prose block that carries the key, not to the front-matter
/// `bibliography:` line, since the fix belongs at the `@key`.
pub fn bare_citation_key_not_rendered(src: &str, blocks: &[Block], base: &Path) -> Vec<Warning> {
    let Some(fm) = crate::frontmatter::front_matter_block(src) else {
        return Vec::new();
    };
    let paths = crate::render::bibliography_paths(fm);
    if paths.is_empty() {
        return Vec::new();
    }
    // Reuse render's own resolution so a key set that renders is the key set we scan for.
    let mut text = String::new();
    for path in &paths {
        let path = path.trim();
        if !path.ends_with(".bib") {
            continue; // an unsupported suffix is its own warning at render time
        }
        if let Some(content) =
            crate::includes::safe_join(base, path).and_then(|p| std::fs::read_to_string(&p).ok())
        {
            text.push_str(&content);
            text.push('\n');
        }
    }
    let bib = crate::cite::parse_bib(&text);
    let keys: Vec<&str> = bib.keys().collect();
    if keys.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for b in blocks {
        // A resolved citation is `<a href="#ref-key">n</a>`, so a surviving literal
        // `@key` in the emitted HTML is precisely the failure. Skip code: a `@key` in a
        // sample is prose about code, not a citation.
        if b.html.starts_with("<pre") || b.id == "qmd-references" {
            continue;
        }
        for key in &keys {
            if !mentions_bare_key(&b.html, key) {
                continue;
            }
            let w = Warning::new(format!(
                "`@{key}` is not a citation, so it renders as literal text \
                 (did you mean `[@{key}]`?)"
            ));
            out.push(match start_line(&b.sourcepos) {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}

/// Whether `html` contains a bare `@key` at both boundaries.
///
/// The trailing test is deliberately NOT `cite::is_cite_key_char`: that predicate admits
/// `.`, so it would reject the sentence-final `@key.` that is the single most common way
/// to write this mistake (and the exact form the a-star post used).
fn mentions_bare_key(html: &str, key: &str) -> bool {
    let needle = format!("@{key}");
    let mut from = 0;
    while let Some(i) = html[from..].find(&needle) {
        let at = from + i;
        let before_ok = html[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric() && c != '@');
        let after_ok = html[at + needle.len()..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric() && !matches!(c, '_' | '-'));
        if before_ok && after_ok {
            return true;
        }
        from = at + needle.len();
    }
    false
}
