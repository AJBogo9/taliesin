//! Front-matter FIELD extraction: lightweight key scans over the raw front-matter
//! string (toc / title-block detection + a generic top-level field reader).
//!
//! Distinct from crate-level `frontmatter.rs` (a full YAML parse + lint): these are
//! read-only string classifiers the render orchestrator runs before any heavy parse.
//! None touches the orchestrator's shared state.

/// The **top-level** front-matter `toc:` setting as a tri-state: `Some(true)`/
/// `Some(false)` when the page sets it, `None` when absent. Returning `Option` lets a
/// site distinguish an explicit `toc: false` (which overrides the site default) from an
/// unset toc (which inherits it).
///
/// Indented lines are skipped, like [`extract_field`] and [`detect_format`]: an indented
/// `toc:` belongs to whatever block encloses it, and no block owns a `toc`. This scan
/// used to trim every line first — a Quarto-era accommodation for `format: html: toc:` —
/// so a `toc:` under ANY block (`hero:`, `listing:`, `execute:`) silently set the
/// document's TOC. Top-level is also the only form the guide teaches and the vocab
/// documents.
pub(super) fn detect_toc(front_matter: &str) -> Option<bool> {
    top_level_lines(front_matter).find_map(|l| {
        // Coerce the YAML-1.1 boolean words too (`toc: yes`/`no`/`on`/`off`), which
        // serde reads as strings — otherwise an explicit `toc: yes` silently no-ops
        // and inherits the site default. See `crate::frontmatter::yaml_bool_word`.
        l.strip_prefix("toc:")
            .and_then(crate::frontmatter::yaml_bool_word)
    })
}

/// `title-block-style: none` suppresses the visible title-block header while
/// keeping the `title` metadata. Used by nav landing pages
/// (Blog/Projects/Publications) where a big `<h1>` repeats the navbar.
/// Top-level only, for the same reason as [`detect_toc`].
pub(super) fn detect_title_block_hidden(front_matter: &str) -> bool {
    top_level_lines(front_matter)
        .any(|l| l.strip_prefix("title-block-style:").map(str::trim) == Some("none"))
}

/// Whether a render of a document with this front matter emits a visible title block —
/// and therefore demotes every body heading one level so the page keeps a single `<h1>`.
///
/// `pub(crate)` because the site's *source-side* section numbering (`site/xref.rs`) has
/// to answer the same question without rendering: a demoted chapter numbers its sections
/// from one level deeper, so a scan that guessed differently would resolve `@sec-x` to a
/// number the heading does not show.
pub(crate) fn emits_title_block(front_matter: &str) -> bool {
    !detect_title_block_hidden(front_matter) && extract_field(front_matter, "title").is_some()
}

/// The un-indented (top-level) front-matter lines, trimmed. The shared primitive behind
/// every top-level key scan, so they cannot drift on what "top-level" means.
fn top_level_lines(front_matter: &str) -> impl Iterator<Item = &str> {
    front_matter
        .lines()
        .filter(|l| !l.starts_with(char::is_whitespace))
        .map(str::trim)
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

/// Parsed `theorems:` front-matter config: which kinds share one counter, and nothing else.
///
/// Numbering itself is not configurable, and neither is its scope. A theorem is numbered; in
/// a numbered book chapter it scopes to that chapter ("Theorem 2.3") and is flat everywhere
/// else, the same rule every float follows. The `numbered:` key that could switch that off
/// was retired on 2026-08-02 along with the book-wide `_site.yml theorems:` policy that
/// carried it, so this is a per-document statement with no inheritance chain behind it.
///
/// Public as an opaque handle so the site search API can carry one through; its fields and
/// accessors stay crate-internal.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct TheoremConfig {
    /// Kinds that share a single counter, in declaration order. Empty = the default
    /// (each kind counts independently).
    shared: Vec<String>,
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
}

/// Parse a `theorems:` block out of an already-parsed YAML value into a `TheoremConfig`.
/// A missing block or unexpected shape yields the default (per-kind counters). `shared:`
/// is a YAML list of kind names.
pub(crate) fn parse_theorem_config_value(value: &serde_yaml::Value) -> TheoremConfig {
    let mut config = TheoremConfig::default();
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
    config
}

/// The theorem config for a page, from its own `theorems:` block. A YAML parse failure
/// yields the default; there is no project-level fallback to preserve.
pub(crate) fn parse_theorem_config(front_matter: &str) -> TheoremConfig {
    // comrak's FrontMatter node includes the `---` fences; serde_yaml treats a leading or
    // trailing `---` as a document marker, so strip the fences to one YAML document.
    let body = front_matter.trim();
    let body = body.strip_prefix("---").unwrap_or(body);
    let body = body.strip_suffix("---").unwrap_or(body);
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(body) else {
        return TheoremConfig::default();
    };
    parse_theorem_config_value(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_group_collapses_to_one_counter_key() {
        let cfg = parse_theorem_config("---\ntheorems:\n  shared: [theorem, lemma]\n---");
        // Every member of the group keys by the group's first member, so they draw one
        // sequence; the visible label stays per-kind.
        assert_eq!(cfg.counter_key("theorem"), "theorem");
        assert_eq!(cfg.counter_key("lemma"), "theorem");
        // An unlisted kind keys by itself and counts separately.
        assert_eq!(cfg.counter_key("definition"), "definition");
    }

    #[test]
    fn no_theorems_block_counts_every_kind_separately() {
        let cfg = parse_theorem_config("---\ntitle: X\n---");
        assert_eq!(cfg.counter_key("theorem"), "theorem");
        assert_eq!(cfg.counter_key("lemma"), "lemma");
    }

    /// The retired `numbered:` key must not survive as a silent parse. It is an unknown
    /// sub-key now, diagnosed by `frontmatter::validate_front_matter`, and it must not
    /// reach back into the counter behaviour by any route.
    #[test]
    fn a_leftover_numbered_key_does_not_change_counting() {
        let cfg = parse_theorem_config("---\ntheorems:\n  numbered: false\n---");
        assert_eq!(cfg, TheoremConfig::default());
    }

    #[test]
    fn malformed_front_matter_yields_the_default() {
        assert_eq!(
            parse_theorem_config("---\ntheorems: [oops\n---"),
            TheoremConfig::default()
        );
    }
}
