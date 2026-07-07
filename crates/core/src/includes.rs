//! Resolve `{{< include path >}}` shortcodes into a single expanded
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

/// An include directive that could not be expanded (unsafe path, cycle, or
/// unreadable file), located back to the file + line that holds the directive so
/// the caller can surface a click-to-source diagnostic instead of silently
/// shipping the literal `{{< include … >}}`.
#[derive(Debug, Clone)]
pub struct IncludeWarning {
    /// The raw include target as written in the directive.
    pub target: String,
    /// Why it couldn't be resolved (a short human phrase).
    pub reason: String,
    /// The file holding the directive (`None` = the primary document), matching
    /// [`LineOrigin::file`].
    pub file: Option<String>,
    /// 1-based line of the directive within `file`.
    pub line: usize,
}

/// Expand includes in `src`. `base_dir` is the directory of the primary
/// document; include paths are resolved relative to the file that contains
/// them. Returns the expanded text plus one [`LineOrigin`] per line.
pub fn resolve(src: &str, base_dir: &Path) -> (String, Vec<LineOrigin>) {
    let (text, origins, _warnings) = resolve_warned(src, base_dir);
    (text, origins)
}

/// Like [`resolve`], but also returns one [`IncludeWarning`] per include that
/// could not be expanded (so build/preview/`check` can report it located rather
/// than leaking the directive silently).
pub fn resolve_warned(
    src: &str,
    base_dir: &Path,
) -> (String, Vec<LineOrigin>, Vec<IncludeWarning>) {
    let mut lines = Vec::new();
    let mut origins = Vec::new();
    let mut warnings = Vec::new();
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
        &mut warnings,
    );

    let mut text = lines.join("\n");
    if had_trailing_newline {
        text.push('\n');
    }
    (text, origins, warnings)
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
    out_warnings: &mut Vec<IncludeWarning>,
) {
    let mut in_code: Option<(char, usize)> = None;
    for (idx, line) in src.lines().enumerate() {
        // Emit `line` verbatim, mapped back to the current file (used whenever a
        // directive isn't expanded: ordinary text, or an unsafe/cyclic/unreadable include).
        let mut keep_line = || {
            out_lines.push(line.to_string());
            out_origins.push(LineOrigin {
                file: file_label.clone(),
                line: idx + 1,
            });
        };
        // Record a located warning for an include that couldn't be expanded, so the
        // dropped directive surfaces as a click-to-source diagnostic in
        // build/preview/`check` instead of leaking silently.
        let mut drop_with_warning = |target: &str, reason: &str| {
            keep_line();
            out_warnings.push(IncludeWarning {
                target: target.to_string(),
                reason: reason.to_string(),
                file: file_label.clone(),
                line: idx + 1,
            });
        };
        let was_in_code = in_code.is_some();
        in_code = next_code_state(in_code, line);
        // A `{{< include >}}` inside a code fence is documentation, not a directive —
        // leave it literal (matches the fenced-div handling).
        let directive = if was_in_code || in_code.is_some() {
            None
        } else {
            parse_include(line)
        };
        let Some(rel) = directive else {
            keep_line();
            continue;
        };
        // Unsafe path (absolute or escaping the project root), or an include cycle:
        // leave the directive visible rather than reading outside the project / looping.
        let Some(target) = safe_join(base_dir, rel) else {
            drop_with_warning(rel, "path escapes the project root (or is absolute)");
            continue;
        };
        if stack.contains(&target) {
            drop_with_warning(rel, "include cycle");
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
                    out_warnings,
                );
                stack.pop();
            }
            // unreadable include: leave the directive visible
            Err(_) => drop_with_warning(rel, "file not found or unreadable"),
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
    let mut in_code: Option<(char, usize)> = None;
    for line in src.lines() {
        let was_in_code = in_code.is_some();
        in_code = next_code_state(in_code, line);
        if was_in_code || in_code.is_some() {
            continue; // a `{{< include >}}` inside a code fence isn't a dependency
        }
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

/// A Markdown code-fence marker line (3+ backticks/tildes after at most 3 spaces),
/// as `(fence_char, run_len)` — so a `{{< include >}}` *inside* a code block is left
/// literal rather than resolved.
fn code_fence(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let ch = trimmed.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let run = trimmed.chars().take_while(|&c| c == ch).count();
    (run >= 3).then_some((ch, run))
}

/// Advance the fenced-code state by one line (open on a fence, close on a bare
/// same-char fence of at least the opening length).
fn next_code_state(state: Option<(char, usize)>, line: &str) -> Option<(char, usize)> {
    match state {
        Some((ch, run)) => match code_fence(line) {
            Some((c2, r2))
                if c2 == ch
                    && r2 >= run
                    && line.trim_start().trim_start_matches(ch).trim().is_empty() =>
            {
                None
            }
            _ => Some((ch, run)),
        },
        None => code_fence(line),
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
/// `target` is absolute (it comes from [`safe_join`]), so `primary_base` is
/// absolutized to the same coordinate system before stripping.
fn label_for(target: &Path, primary_base: &Path) -> String {
    let primary = absolutize(primary_base);
    match target.strip_prefix(&primary) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => target.to_string_lossy().into_owned(),
    }
}

/// Resolve `rel` against `base_dir`, refusing path-traversal escapes. An absolute
/// `rel`, or a result that climbs above the *project root* (the nearest ancestor of
/// `base_dir` holding a `.git` or `_site.yml`, else `base_dir` itself), returns
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
    // Resolve against an *absolute* base so the containment check and the returned
    // target share one coordinate system: a relative CLI path (e.g. the doc's
    // `corpus/posts/x` parent) would otherwise make `containment_root`'s absolute
    // boundary and a relative `target` incomparable, silently rejecting legitimate
    // `../../_includes/…` includes. `std::path::absolute` only prepends the cwd +
    // normalizes lexically (no filesystem touch, no symlink resolution).
    let abs_base = absolutize(base_dir);
    let target = normalize(&abs_base.join(relp));
    let root = containment_root(&abs_base);
    target.starts_with(&root).then_some(target)
}

/// Make `p` absolute by prepending the current working directory if needed, then
/// normalizing `.`/`..` lexically. No filesystem access, no symlink resolution.
fn absolutize(p: &Path) -> PathBuf {
    normalize(&std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()))
}

/// The containment boundary for [`safe_join`]: the nearest ancestor of `base_dir`
/// that looks like a project root (`.git` or `_site.yml`), falling back to
/// `base_dir` itself when none is found.
///
/// Expects an **absolute, normalized** `base_dir` (see [`absolutize`] in
/// [`safe_join`]). The parent-walk must start absolute: when the CLI is given a
/// relative path (e.g. `corpus/posts/x/index.tmd`), a relative parent-walk hits an
/// empty path before ever seeing the absolute ancestor that actually holds
/// `.git`/`_site.yml`, so it would fall back to `base_dir` itself and then reject a
/// legitimate `../../_includes/…` include as "escaping" that fake root.
fn containment_root(base_dir: &Path) -> PathBuf {
    let base = base_dir.to_path_buf();
    let mut cur: &Path = &base;
    loop {
        if cur.join(".git").exists() || cur.join("_site.yml").exists() {
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
        assert_eq!(parse_include("{{< include foo.tmd >}}"), Some("foo.tmd"));
        assert_eq!(
            parse_include("  {{< include \"a/b.tmd\" >}}  "),
            Some("a/b.tmd")
        );
        assert_eq!(parse_include("text {{< include x >}}"), None); // not alone on the line
        assert_eq!(parse_include("{{< video x >}}"), None); // different shortcode
    }

    #[test]
    fn normalize_resolves_dotdot() {
        assert_eq!(normalize(Path::new("a/b/../c")), PathBuf::from("a/c"));
        assert_eq!(normalize(Path::new("./a/./b")), PathBuf::from("a/b"));
    }

    #[test]
    fn unresolvable_include_is_reported_not_silently_dropped() {
        // An escaping include leaves the directive literal *and* emits a located
        // warning (the silent-drop fix), rather than vanishing without a trace.
        let (text, _origins, warnings) = resolve_warned(
            "a\n{{< include ../../../etc/passwd >}}\nb\n",
            Path::new("."),
        );
        assert!(
            text.contains("{{< include ../../../etc/passwd >}}"),
            "the directive stays literal when it can't be expanded"
        );
        let w = warnings
            .first()
            .expect("an unresolvable include produces a warning");
        assert_eq!(w.line, 2, "warning is located on the directive line");
        assert_eq!(w.file, None, "directive lives in the primary document");
        assert!(w.target.contains("etc/passwd"));
    }

    #[test]
    fn safe_join_allows_sibling_include_under_project_root() {
        // The regression in miniature: a project root marked by `.git`, a doc in a
        // nested subdir, and a `../`-reaching include into a sibling `_includes/`.
        // `containment_root` must find the marked root (not fall back to the doc
        // dir), so the sibling include is allowed.
        let root = std::env::temp_dir().join(format!(
            "tali-safejoin-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("post")).unwrap();
        std::fs::create_dir_all(root.join("_includes")).unwrap();
        std::fs::write(root.join(".git"), b"").unwrap();

        let base = root.join("post");
        // A sibling include under the marked root resolves.
        assert!(
            safe_join(&base, "../_includes/x.tmd").is_some(),
            "a sibling include under the project root must resolve"
        );
        // Climbing above the root is refused.
        assert!(safe_join(&base, "../../escape.tmd").is_none());
        // An absolute target is always refused.
        assert!(safe_join(&base, "/etc/passwd").is_none());

        let _ = std::fs::remove_dir_all(&root);
    }
}
