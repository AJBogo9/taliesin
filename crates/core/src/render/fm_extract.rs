//! Front-matter FIELD extraction: lightweight key scans over the raw front-matter
//! string (format / toc / title-block detection + a generic top-level field reader).
//!
//! Distinct from crate-level `frontmatter.rs` (a full YAML parse + lint): these are
//! read-only string classifiers the render orchestrator runs before any heavy parse,
//! and `is_reveal_doc` is a public fast-path for site discovery. None touches the
//! orchestrator's shared state.

use super::DocFormat;

/// The front-matter `toc:` setting (typically under `format: html:`) as a
/// tri-state: `Some(true)`/`Some(false)` when the page sets it, `None` when
/// absent. A lightweight scan, matching the corpus book's usage. Returning
/// `Option` lets a site distinguish an explicit `toc: false` (which overrides
/// the site default) from an unset toc (which inherits it).
pub(super) fn detect_toc(front_matter: &str) -> Option<bool> {
    front_matter.lines().find_map(|l| {
        let t = l.trim();
        match t.strip_prefix("toc:").map(str::trim) {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        }
    })
}

/// `title-block-style: none` suppresses the visible title-block header while
/// keeping the `title` metadata (Quarto-compatible). Used by nav landing pages
/// (Blog/Projects/Publications) where a big `<h1>` repeats the navbar.
pub(super) fn detect_title_block_hidden(front_matter: &str) -> bool {
    front_matter.lines().any(|l| {
        let t = l.trim();
        t.strip_prefix("title-block-style:").map(str::trim) == Some("none")
    })
}

/// Whether a document's front matter selects a revealjs deck. Reads only the
/// front matter (no full parse), so site discovery can cheaply flag a loose deck
/// dropped into a website — which would otherwise be flattened into an article.
pub fn is_reveal_doc(src: &str) -> bool {
    crate::frontmatter::front_matter_block(src)
        .is_some_and(|fm| detect_format(fm) == DocFormat::Reveal)
}

/// Detect the output format from raw front matter. A reveal.js deck declares a
/// `format:` whose inline value or indented sub-keys name a revealjs variant
/// (`revealjs`, `liquid-glass-revealjs`, …). Everything else is a standard page.
pub(super) fn detect_format(front_matter: &str) -> DocFormat {
    let lines: Vec<&str> = front_matter.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        // Only consider the top-level `format:` key, not nested ones.
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some(rest) = line.trim_end().strip_prefix("format:") else {
            continue;
        };
        let inline = rest.trim();
        if !inline.is_empty() {
            // `format: revealjs` (or a list `[html, revealjs]`): match a format
            // *name*, not any substring, so a theme/filename that merely contains
            // "revealjs" (e.g. `theme: my-revealjs.css`) can't flip an HTML doc to
            // a deck.
            return if inline.split(['[', ']', ',', ' ']).any(is_reveal_format) {
                DocFormat::Reveal
            } else {
                DocFormat::Html
            };
        }
        // Block form: the sub-keys are format *names* (`html:`, `revealjs:`,
        // `liquid-glass-revealjs:`). Match the key, never a value substring.
        for sub in &lines[i + 1..] {
            if sub.trim().is_empty() {
                continue;
            }
            if !sub.starts_with(char::is_whitespace) {
                break;
            }
            let key = sub.trim().split(':').next().unwrap_or("");
            if is_reveal_format(key) {
                return DocFormat::Reveal;
            }
        }
        return DocFormat::Html;
    }
    DocFormat::Html
}

/// Whether a `format:` name selects a revealjs deck: `revealjs` itself or an
/// extension variant `<ext>-revealjs` (e.g. `liquid-glass-revealjs`).
fn is_reveal_format(name: &str) -> bool {
    let n = name.trim().trim_matches(['"', '\'']);
    n == "revealjs" || n.ends_with("-revealjs")
}

/// The `bibliography:` front-matter value as a list of paths. Accepts a scalar
/// (`bibliography: refs.bib`, INCLUDING a quoted path with spaces), an inline seq
/// (`[a.bib, b.bib]`), or a block seq (`- a.bib` / `- b.bib`). Parses the front matter
/// as YAML for a faithful read; falls back to the lenient line-scanner + `,[]`-split
/// (the prior behaviour, MINUS the space split that broke spaced paths) when the YAML
/// won't parse, so a malformed-but-linted doc still resolves what it can.
pub(super) fn bibliography_paths(front_matter: &str) -> Vec<String> {
    if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(front_matter) {
        match val.get("bibliography") {
            Some(serde_yaml::Value::Sequence(seq)) => {
                return seq
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
            }
            Some(serde_yaml::Value::String(s)) => return vec![s.clone()],
            _ => {}
        }
    }
    match extract_field(front_matter, "bibliography") {
        Some(raw) => raw
            .split([',', '[', ']'])
            .map(|t| t.trim().trim_matches(['"', '\'']).to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        None => Vec::new(),
    }
}

/// Extract a top-level `key:` value from raw front matter. Lightweight scan,
/// not a YAML parse; returns the inline value (empty for block/list values).
pub(super) fn extract_field(front_matter: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    for line in front_matter.lines() {
        // top-level keys only (not indented sub-keys)
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some(rest) = line.trim().strip_prefix(&prefix) {
            let v = rest.trim().trim_matches(['"', '\'']).trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// `theorems: number-within:` scope. Only `Chapter` is honored this increment (book
/// chapter pages render "Theorem 2.3"); other values degrade to `None`.
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub(crate) enum NumberWithin {
    #[default]
    None,
    Chapter,
}

/// `theorems: numbered:` mode. `UnlessUnique` numbers a kind only when it appears more
/// than once (a lone Theorem shows just "Theorem").
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Numbered {
    #[default]
    Yes,
    No,
    UnlessUnique,
}

/// Parsed `theorems:` front-matter config (`shared` counters + `number-within` scope +
/// `numbered` mode).
#[derive(Default)]
pub(crate) struct TheoremConfig {
    /// Kinds that share a single counter, in declaration order. Empty = the default
    /// (each kind counts independently).
    shared: Vec<String>,
    /// Numbering scope. `Chapter` prepends the book chapter number ("Theorem 2.3").
    number_within: NumberWithin,
    /// Whether/when a number is shown.
    numbered: Numbered,
}

impl TheoremConfig {
    /// The counter key for `kind`: every kind in the shared group collapses to one key
    /// (the group's first member) so they draw a single sequence; an unlisted kind keys
    /// by itself. This governs only the NUMBER; the visible label stays per-kind.
    pub(crate) fn counter_key<'a>(&'a self, kind: &'a str) -> &'a str {
        if self.shared.iter().any(|k| k == kind) {
            self.shared.first().map(String::as_str).unwrap_or(kind)
        } else {
            kind
        }
    }

    /// Whether theorem numbers are chapter-scoped (`number-within: chapter`).
    pub(crate) fn chapter_scoped(&self) -> bool {
        self.number_within == NumberWithin::Chapter
    }

    /// The `numbered:` mode (whether/when to show a number).
    pub(crate) fn numbered(&self) -> Numbered {
        self.numbered
    }
}

/// Parse the `theorems:` block out of a front-matter string into a `TheoremConfig`.
/// An absent block, a parse failure, or an unexpected shape yields the default
/// (per-kind numbering). `shared:` is a YAML list of kind names. Uses serde_yaml,
/// already a dependency (see `frontmatter::validate_front_matter`).
pub(crate) fn parse_theorem_config(front_matter: &str) -> TheoremConfig {
    let mut config = TheoremConfig::default();
    // comrak's FrontMatter node includes the `---` fences; serde_yaml treats a leading or
    // trailing `---` as a document marker, so strip the fences to one YAML document.
    let body = front_matter.trim();
    let body = body.strip_prefix("---").unwrap_or(body);
    let body = body.strip_suffix("---").unwrap_or(body);
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(body) else {
        return config;
    };
    if let Some(shared) = value
        .get("theorems")
        .and_then(|t| t.get("shared"))
        .and_then(|s| s.as_sequence())
    {
        config.shared = shared
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
    }
    if value
        .get("theorems")
        .and_then(|t| t.get("number-within"))
        .and_then(|v| v.as_str())
        == Some("chapter")
    {
        config.number_within = NumberWithin::Chapter;
    }
    match value.get("theorems").and_then(|t| t.get("numbered")) {
        Some(serde_yaml::Value::Bool(false)) => config.numbered = Numbered::No,
        Some(v) if v.as_str() == Some("unless-unique") => config.numbered = Numbered::UnlessUnique,
        _ => {} // true / absent / unrecognized -> Yes (default)
    }
    config
}
