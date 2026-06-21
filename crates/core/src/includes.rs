//! Resolve Quarto `{{< include path >}}` shortcodes into a single expanded
//! buffer, while keeping a line-level **source map** so every line of the
//! result can be traced back to the file and line it came from. This is what
//! lets click-to-source jump into the *included* file rather than the parent.

use std::path::{Component, Path, PathBuf};

/// Where a line of the expanded buffer originally came from.
#[derive(Debug, Clone)]
pub struct LineOrigin {
    /// `None` for the primary document; `Some(path)` (relative to the primary
    /// document's directory when possible) for an included file.
    pub file: Option<String>,
    /// 1-based line number within `file`.
    pub line: usize,
}

/// Expand includes in `src`. `base_dir` is the directory of the primary
/// document; include paths are resolved relative to the file that contains
/// them. Returns the expanded text plus one [`LineOrigin`] per line.
pub fn resolve(src: &str, base_dir: &Path) -> (String, Vec<LineOrigin>) {
    let mut lines = Vec::new();
    let mut origins = Vec::new();
    let mut stack = Vec::new(); // cycle guard: absolute paths currently expanding
    let had_trailing_newline = src.ends_with('\n');
    expand(
        src,
        base_dir,
        base_dir,
        None,
        &mut stack,
        &mut lines,
        &mut origins,
    );

    let mut text = lines.join("\n");
    if had_trailing_newline {
        text.push('\n');
    }
    (text, origins)
}

#[allow(clippy::too_many_arguments)]
fn expand(
    src: &str,
    base_dir: &Path,            // directory of the file currently being expanded
    primary_base: &Path,        // directory of the primary document (for nice labels)
    file_label: Option<String>, // label of the current file (None = primary)
    stack: &mut Vec<PathBuf>,
    out_lines: &mut Vec<String>,
    out_origins: &mut Vec<LineOrigin>,
) {
    for (idx, line) in src.lines().enumerate() {
        match parse_include(line) {
            Some(rel) => {
                let Some(target) = safe_join(base_dir, rel) else {
                    // Unsafe path (absolute or escaping the project root): leave the
                    // directive visible rather than reading outside the project.
                    out_lines.push(line.to_string());
                    out_origins.push(LineOrigin {
                        file: file_label.clone(),
                        line: idx + 1,
                    });
                    continue;
                };
                if stack.contains(&target) {
                    // include cycle: leave the directive in place rather than loop
                    out_lines.push(line.to_string());
                    out_origins.push(LineOrigin {
                        file: file_label.clone(),
                        line: idx + 1,
                    });
                    continue;
                }
                match std::fs::read_to_string(&target) {
                    Ok(content) => {
                        let label = label_for(&target, primary_base);
                        let child_base = target.parent().unwrap_or(base_dir).to_path_buf();
                        stack.push(target.clone());
                        expand(
                            &content,
                            &child_base,
                            primary_base,
                            Some(label),
                            stack,
                            out_lines,
                            out_origins,
                        );
                        stack.pop();
                    }
                    Err(_) => {
                        // unreadable include: leave the directive visible
                        out_lines.push(line.to_string());
                        out_origins.push(LineOrigin {
                            file: file_label.clone(),
                            line: idx + 1,
                        });
                    }
                }
            }
            None => {
                out_lines.push(line.to_string());
                out_origins.push(LineOrigin {
                    file: file_label.clone(),
                    line: idx + 1,
                });
            }
        }
    }
}

/// All files transitively pulled in by `{{< include >}}` from `src` (absolute,
/// normalized). Used by the dev server to watch the right files.
pub fn dependencies(src: &str, base_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = Vec::new();
    collect_deps(src, base_dir, &mut stack, &mut out);
    out
}

fn collect_deps(src: &str, base_dir: &Path, stack: &mut Vec<PathBuf>, out: &mut Vec<PathBuf>) {
    for line in src.lines() {
        let Some(rel) = parse_include(line) else {
            continue;
        };
        let Some(target) = safe_join(base_dir, rel) else {
            continue;
        };
        if stack.contains(&target) || out.contains(&target) {
            continue;
        }
        out.push(target.clone());
        if let Ok(content) = std::fs::read_to_string(&target) {
            let child_base = target.parent().unwrap_or(base_dir).to_path_buf();
            stack.push(target.clone());
            collect_deps(&content, &child_base, stack, out);
            stack.pop();
        }
    }
}

/// If `line` is solely a `{{< include PATH >}}` shortcode, return PATH.
fn parse_include(line: &str) -> Option<&str> {
    let t = line.trim();
    let inner = t.strip_prefix("{{<")?.strip_suffix(">}}")?.trim();
    let rest = inner.strip_prefix("include")?;
    // require a word boundary after "include"
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let path = rest.trim().trim_matches(['"', '\'']).trim();
    (!path.is_empty()).then_some(path)
}

/// A nice label for an included file: relative to the primary document's
/// directory when it lives underneath it, otherwise the normalized path.
fn label_for(target: &Path, primary_base: &Path) -> String {
    let primary = normalize(primary_base);
    match target.strip_prefix(&primary) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => target.to_string_lossy().into_owned(),
    }
}

/// Resolve `rel` against `base_dir`, refusing path-traversal escapes. An absolute
/// `rel`, or a result that climbs above the *project root* (the nearest ancestor of
/// `base_dir` holding a `.git` or `_quarto.yml`, else `base_dir` itself), returns
/// `None` so the caller can refuse it. This blocks `{{< include /etc/passwd >}}`
/// and `../../../../etc/...` while still allowing the corpus's `../../_includes/...`
/// (the repo root contains both the doc and `_includes/`). Shared by include
/// resolution, theme/CSS includes, and format-resource reads.
pub(crate) fn safe_join(base_dir: &Path, rel: &str) -> Option<PathBuf> {
    let relp = Path::new(rel);
    // An absolute path (incl. a Windows drive/UNC root) escapes immediately.
    if relp.has_root() || relp.is_absolute() {
        return None;
    }
    let target = normalize(&base_dir.join(relp));
    let root = containment_root(base_dir);
    target.starts_with(&root).then_some(target)
}

/// The containment boundary for [`safe_join`]: the nearest ancestor of `base_dir`
/// that looks like a project root (`.git` or `_quarto.yml`), falling back to
/// `base_dir` itself when none is found.
fn containment_root(base_dir: &Path) -> PathBuf {
    let base = normalize(base_dir);
    let mut cur: &Path = &base;
    loop {
        if cur.join(".git").exists() || cur.join("_quarto.yml").exists() {
            return cur.to_path_buf();
        }
        match cur.parent() {
            Some(p) if !p.as_os_str().is_empty() => cur = p,
            _ => return base.clone(),
        }
    }
}

/// Lexically normalize a path (resolve `.` and `..`) without touching the
/// filesystem, so labels and cycle checks are stable.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_include_directive() {
        assert_eq!(parse_include("{{< include foo.qmd >}}"), Some("foo.qmd"));
        assert_eq!(
            parse_include("  {{< include \"a/b.qmd\" >}}  "),
            Some("a/b.qmd")
        );
        assert_eq!(parse_include("text {{< include x >}}"), None); // not alone on the line
        assert_eq!(parse_include("{{< video x >}}"), None); // different shortcode
    }

    #[test]
    fn normalize_resolves_dotdot() {
        assert_eq!(normalize(Path::new("a/b/../c")), PathBuf::from("a/c"));
        assert_eq!(normalize(Path::new("./a/./b")), PathBuf::from("a/b"));
    }
}
