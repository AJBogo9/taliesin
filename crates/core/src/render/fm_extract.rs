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
        // Coerce the YAML-1.1 boolean words too (`toc: yes`/`no`/`on`/`off`), which
        // serde reads as strings — otherwise an explicit `toc: yes` silently no-ops
        // and inherits the site default. See `crate::frontmatter::yaml_bool_word`.
        l.trim()
            .strip_prefix("toc:")
            .and_then(crate::frontmatter::yaml_bool_word)
    })
}

/// `title-block-style: none` suppresses the visible title-block header while
/// keeping the `title` metadata. Used by nav landing pages
/// (Blog/Projects/Publications) where a big `<h1>` repeats the navbar.
pub(super) fn detect_title_block_hidden(front_matter: &str) -> bool {
    front_matter.lines().any(|l| {
        let t = l.trim();
        t.strip_prefix("title-block-style:").map(str::trim) == Some("none")
    })
}

/// Whether a document's front matter selects a slide deck. Reads only the
/// front matter (no full parse), so site discovery can cheaply flag a loose deck
/// dropped into a website — which would otherwise be flattened into an article.
pub fn is_reveal_doc(src: &str) -> bool {
    crate::frontmatter::front_matter_block(src)
        .is_some_and(|fm| detect_format(fm) == DocFormat::Reveal)
}

/// Detect the output format from raw front matter. A deck declares a `format:`
/// whose inline value or indented sub-keys name a deck variant (`deck` or a
/// `<name>-deck` form). Everything else is a standard page.
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
            // `format: deck` (or a list `[html, deck]`): match a format *name*,
            // not any substring, so a theme/filename that merely contains "deck"
            // (e.g. `theme: my-deck.css`) can't flip an HTML doc to a deck.
            return if inline.split(['[', ']', ',', ' ']).any(is_reveal_format) {
                DocFormat::Reveal
            } else {
                DocFormat::Html
            };
        }
        // Block form: the sub-keys are format *names* (`html:`, `deck:`,
        // `<name>-deck:`). Match the key, never a value substring.
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

/// Whether a `format:` name selects a slide deck. The only spelling is `deck`
/// (or an extension variant `<ext>-deck`); the engine is taliesin's own.
fn is_reveal_format(name: &str) -> bool {
    let n = name.trim().trim_matches(['"', '\'']);
    n == "deck" || n.ends_with("-deck")
}

/// The `bibliography:` front-matter value as a list of paths. Accepts a scalar
/// (`bibliography: refs.bib`, INCLUDING a quoted path with spaces), an inline seq
/// (`[a.bib, b.bib]`), or a block seq (`- a.bib` / `- b.bib`). Strips the `---` fences
/// (which the real caller's comrak node carries) and parses as YAML for a faithful read;
/// when the YAML won't parse it falls back to the lenient scanner — `,[]`-split for the
/// scalar/inline form, plus a block-sequence read — so a malformed-but-linted doc still
/// resolves what it can.
pub(crate) fn bibliography_paths(front_matter: &str) -> Vec<String> {
    // comrak's FrontMatter node includes the `---` fences, which serde_yaml reads as
    // document markers and rejects — so the faithful parse below would ALWAYS fail on the
    // real caller's input and silently fall through to the lenient scanner (which can't
    // read a block sequence). Strip the fences first (a no-op on a fence-free string),
    // matching `parse_theorem_config`.
    let body = front_matter.trim();
    let body = body.strip_prefix("---").unwrap_or(body);
    let body = body.strip_suffix("---").unwrap_or(body);
    if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(body) {
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
    // Lenient fallback for a front matter that won't parse as YAML at all: the scalar /
    // inline-seq form via `extract_field`, plus a block sequence (`- a.bib` lines) it
    // can't reach — so a malformed-but-linted doc still resolves what it can.
    match extract_field(front_matter, "bibliography") {
        Some(raw) => raw
            .split([',', '[', ']'])
            .map(|t| t.trim().trim_matches(['"', '\'']).to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        None => block_seq_items(front_matter, "bibliography"),
    }
}

/// Read a top-level block-sequence value (`key:` on its own line, then indented `- item`
/// lines) from raw front matter — the one shape [`extract_field`] can't reach. Used as
/// the last-resort fallback when the YAML won't parse. Stops at the first dedent or
/// non-sequence line.
fn block_seq_items(front_matter: &str, key: &str) -> Vec<String> {
    let prefix = format!("{key}:");
    let mut lines = front_matter.lines();
    // The block opens at a top-level `key:` with an empty inline value.
    let opened = lines.by_ref().any(|l| {
        !l.starts_with(char::is_whitespace)
            && l.trim().strip_prefix(&prefix).map(str::trim) == Some("")
    });
    if !opened {
        return Vec::new();
    }
    let mut out = Vec::new();
    for l in lines {
        let t = l.trim();
        if t.is_empty() {
            continue;
        }
        if !l.starts_with(char::is_whitespace) {
            break; // dedent to a new top-level key ends the block
        }
        let Some(item) = t.strip_prefix('-') else {
            break; // an indented non-item line is not part of the sequence
        };
        let v = item.trim().trim_matches(['"', '\'']).trim();
        if !v.is_empty() {
            out.push(v.to_string());
        }
    }
    out
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

/// `theorems: numbered:` mode. `UnlessUnique` numbers a kind only when it appears more
/// than once (a lone Theorem shows just "Theorem").
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Numbered {
    #[default]
    Yes,
    No,
    UnlessUnique,
}

/// Parsed `theorems:` front-matter config (`shared` counters + `numbered` mode).
/// Numbering *scope* is not configurable: a theorem in a numbered book chapter scopes to
/// it ("Theorem 2.3") and is flat everywhere else, the same rule every float follows.
#[derive(Default)]
pub(crate) struct TheoremConfig {
    /// Kinds that share a single counter, in declaration order. Empty = the default
    /// (each kind counts independently).
    shared: Vec<String>,
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
    match value.get("theorems").and_then(|t| t.get("numbered")) {
        Some(serde_yaml::Value::Bool(false)) => config.numbered = Numbered::No,
        Some(v) if v.as_str() == Some("unless-unique") => config.numbered = Numbered::UnlessUnique,
        _ => {} // true / absent / unrecognized -> Yes (default)
    }
    config
}
