//! Resolution for `_site.yml`'s `head:` — the one raw-injection hatch.
//!
//! Turns the configured value (a path, `{text:}`/`{file:}`, or a list of those) into
//! ready-to-inject `<head>` markup, reading referenced files relative to `base_dir` behind
//! the path-traversal guard. A missing file degrades to an HTML comment (warn, don't
//! reject); nothing here touches the orchestrator's shared state.
//!
//! This module used to resolve a family of seven: the per-document `include-in-header` /
//! `include-before-body` / `include-after-body` / `css` front-matter keys and `_site.yml`'s
//! `css:` / `body-start:` / `body-end:`. All seven were retired on 2026-08-02 at measured
//! zero adoption across 218 documents and 17 configs — one escape hatch is what a published
//! tool needs, and seven ways to reach it is surface area, not capability.
//!
//! Distinct from crate-level `includes.rs` (the `{{< include >}}` resolver) and
//! `frontmatter.rs` (YAML parse + lint).

use super::PageIncludes;
use std::path::{Path, PathBuf};

/// Build [`PageIncludes`] from `_site.yml`'s already-parsed `head:` value.
///
/// Only `in_header` is populated. `before_body`/`after_body` remain live slots on
/// [`PageIncludes`] — the site chrome writes the draft banner into `before_body` — they
/// simply have no *configured* source any more.
pub fn includes_from_parts(
    in_header: Option<&serde_yaml::Value>,
    base_dir: Option<&Path>,
    root: Option<&Path>,
) -> PageIncludes {
    let mut out = String::new();
    if let Some(v) = in_header {
        resolve_include_into(v, base_dir, root, &mut out);
    }
    PageIncludes {
        in_header: out,
        ..PageIncludes::default()
    }
}

/// Resolve one `head:` value: a path string (file contents), a `{text: …}` or `{file: …}`
/// map, or a list of those. The markup is injected verbatim — this is the escape hatch, so
/// it does not second-guess what the author wrote.
fn resolve_include_into(
    v: &serde_yaml::Value,
    base_dir: Option<&Path>,
    root: Option<&Path>,
    out: &mut String,
) {
    use serde_yaml::Value;
    match v {
        Value::String(s) => append_include(&read_include_file(base_dir, s, root), out),
        Value::Mapping(_) => {
            if let Some(Value::String(t)) = v.get("text") {
                append_include(t, out);
            } else if let Some(Value::String(f)) = v.get("file") {
                append_include(&read_include_file(base_dir, f, root), out);
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                resolve_include_into(item, base_dir, root, out);
            }
        }
        _ => {}
    }
}

fn append_include(content: &str, out: &mut String) {
    out.push_str(content);
    out.push('\n');
}

/// Read an include/css file relative to the doc (or site root). A missing file is
/// reported as an HTML comment rather than aborting the render (warn, don't reject).
fn read_include_file(base_dir: Option<&Path>, rel: &str, root: Option<&Path>) -> String {
    // Containment: an absolute path or one escaping the project root is refused
    // (path-traversal guard), reported the same as a missing file.
    let path = match base_dir {
        Some(dir) => crate::includes::safe_join_in(dir, rel, root),
        None => Path::new(rel).is_relative().then(|| PathBuf::from(rel)),
    };
    match path.and_then(|p| std::fs::read_to_string(&p).ok()) {
        Some(s) => s,
        None => format!(
            "<!-- taliesin: include file not found: {} -->",
            esc_comment(rel)
        ),
    }
}

/// Sanitize a path for an HTML comment (no `--`, which would close the comment).
fn esc_comment(s: &str) -> String {
    s.replace("--", "__")
}
