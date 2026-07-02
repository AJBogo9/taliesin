//! Document-level front-matter include + CSS resolution.
//!
//! Resolves the `include-in-header` / `include-before-body` / `include-after-body` /
//! `css` front-matter keys into ready-to-inject markup, reading referenced files
//! relative to `base_dir` behind the path-traversal guard. A missing file degrades to
//! an HTML comment (warn, don't reject); nothing here touches the orchestrator's shared
//! state. Distinct from crate-level `includes.rs` (the `{{< include >}}` resolver) and
//! `frontmatter.rs` (YAML parse + lint).

use super::PageIncludes;
use std::path::{Path, PathBuf};

/// Resolve the `include-in-header`/`include-before-body`/`include-after-body` +
/// `css` keys from a doc's front-matter YAML into ready-to-inject markup, reading
/// referenced files relative to `base_dir`.
pub(super) fn resolve_doc_includes(front_matter: &str, base_dir: Option<&Path>) -> PageIncludes {
    // comrak hands us the block *with* its `---` fences; strip them so serde_yaml
    // sees a single document (the bare `---` would otherwise read as a separator).
    let body = {
        let mut lines: Vec<&str> = front_matter.lines().collect();
        while lines.first().is_some_and(|l| l.trim().is_empty()) {
            lines.remove(0);
        }
        if lines.first().is_some_and(|l| l.trim() == "---") {
            lines.remove(0);
        }
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        if lines.last().is_some_and(|l| l.trim() == "---") {
            lines.pop();
        }
        lines.join("\n")
    };
    let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&body) else {
        return PageIncludes::default();
    };
    includes_from_parts(
        v.get("include-in-header"),
        v.get("include-before-body"),
        v.get("include-after-body"),
        v.get("css"),
        base_dir,
    )
}

/// Build [`PageIncludes`] from already-located YAML values for each key. Shared by
/// the single-doc front-matter path and the site `format: html:` path (which keep
/// these as typed `serde_yaml::Value` fields). `css` files are wrapped in `<style>`
/// and placed ahead of the header text so an author stylesheet can override ours.
pub fn includes_from_parts(
    in_header: Option<&serde_yaml::Value>,
    before_body: Option<&serde_yaml::Value>,
    after_body: Option<&serde_yaml::Value>,
    css: Option<&serde_yaml::Value>,
    base_dir: Option<&Path>,
) -> PageIncludes {
    let mut head = resolve_include_value(css, base_dir, true);
    head.push_str(&resolve_include_value(in_header, base_dir, false));
    PageIncludes {
        in_header: head,
        before_body: resolve_include_value(before_body, base_dir, false),
        after_body: resolve_include_value(after_body, base_dir, false),
        resources: Vec::new(),
    }
}

/// Resolve one include value: a path string (file contents), a `{text: …}` or
/// `{file: …}` map, or a list of those. `css == true` wraps each resolved chunk
/// in a `<style>` block; otherwise the markup is injected verbatim.
fn resolve_include_value(
    v: Option<&serde_yaml::Value>,
    base_dir: Option<&Path>,
    css: bool,
) -> String {
    let mut out = String::new();
    if let Some(v) = v {
        resolve_include_into(v, base_dir, css, &mut out);
    }
    out
}

fn resolve_include_into(
    v: &serde_yaml::Value,
    base_dir: Option<&Path>,
    css: bool,
    out: &mut String,
) {
    use serde_yaml::Value;
    match v {
        Value::String(s) => append_include(&read_include_file(base_dir, s), css, out),
        Value::Mapping(_) => {
            if let Some(Value::String(t)) = v.get("text") {
                append_include(t, css, out);
            } else if let Some(Value::String(f)) = v.get("file") {
                append_include(&read_include_file(base_dir, f), css, out);
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                resolve_include_into(item, base_dir, css, out);
            }
        }
        _ => {}
    }
}

fn append_include(content: &str, css: bool, out: &mut String) {
    if css {
        out.push_str("<style>\n");
        out.push_str(content);
        out.push_str("\n</style>\n");
    } else {
        out.push_str(content);
        out.push('\n');
    }
}

/// Read an include/css file relative to the doc (or site root). A missing file is
/// reported as an HTML comment rather than aborting the render (warn, don't reject).
fn read_include_file(base_dir: Option<&Path>, rel: &str) -> String {
    // Containment: an absolute path or one escaping the project root is refused
    // (path-traversal guard), reported the same as a missing file.
    let path = match base_dir {
        Some(dir) => crate::includes::safe_join(dir, rel),
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
