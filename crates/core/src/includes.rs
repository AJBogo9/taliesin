//! Resolve `{{< include path >}}` shortcodes into a single expanded
//! buffer, while keeping a line-level **source map** so every line of the
//! result can be traced back to the file and line it came from. This is what
//! lets click-to-source jump into the *included* file rather than the parent.

use std::path::{Component, Path, PathBuf};

use crate::render::parse_heading_attr;

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
    resolve_warned_in(src, base_dir, None)
}

/// Like [`resolve_warned`], but with an explicit containment `root` (see [`safe_join_in`]).
/// First-party single-document invocations pass the invoked doc's own directory so an
/// untrusted document cannot `{{< include ../../.. >}}` out of it into a parent checkout.
/// `None` keeps the inferred-marker walk (the site and corpus loose-doc behavior).
pub fn resolve_warned_in(
    src: &str,
    base_dir: &Path,
    root: Option<&Path>,
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
        0,
        root,
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
    // How many lines of the current file sit ABOVE `src`. Non-zero only for a
    // `#fragment` include, where `src` is a slice of the file rather than all of it.
    // Every `LineOrigin` adds it, so a transcluded section maps to the line it really
    // occupies in its own file and click-to-source lands on the source heading rather
    // than N lines above it. This is the source-map gate item 160 was filed under.
    line_offset: usize,
    root: Option<&Path>, // explicit containment root (constant across recursion)
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
                line: line_offset + idx + 1,
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
                line: line_offset + idx + 1,
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
        let Some(raw) = directive else {
            keep_line();
            continue;
        };
        // `part.tmd#sec-proof` names a section of a file, not a file (item 160).
        let (rel, fragment) = split_target(raw);
        // Unsafe path (absolute or escaping the project root), or an include cycle:
        // leave the directive visible rather than reading outside the project / looping.
        let Some(target) = safe_join_in(base_dir, rel, root) else {
            drop_with_warning(raw, "path escapes the project root (or is absolute)");
            continue;
        };
        // The cycle guard keys on the FILE, not the file-plus-fragment: two different
        // sections of one file are not a cycle, but a section that transitively pulls its
        // own file back in is, and keying on the pair would walk straight into it.
        if stack.contains(&target) {
            drop_with_warning(raw, "include cycle");
            continue;
        }
        match std::fs::read_to_string(&target) {
            Ok(content) => {
                // A whole file starts at its first line; a fragment starts wherever its
                // anchored heading sits, and carries that offset into the source map.
                let (body, offset) = match fragment {
                    None => (content.as_str(), 0),
                    Some(id) => match section_lines(&content, id) {
                        Some((start, end)) => (slice_lines(&content, start, end), start),
                        None => {
                            drop_with_warning(
                                raw,
                                &format!("no section anchored {{#{id}}} in {rel}"),
                            );
                            continue;
                        }
                    },
                };
                let label = label_for(&target, primary_base);
                let child_base = target.parent().unwrap_or(base_dir).to_path_buf();
                stack.push(target.clone());
                expand(
                    body,
                    &child_base,
                    primary_base,
                    Some(label),
                    offset,
                    root,
                    stack,
                    out_lines,
                    out_origins,
                    out_warnings,
                );
                stack.pop();
            }
            // unreadable include: leave the directive visible
            Err(_) => drop_with_warning(raw, "file not found or unreadable"),
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

/// Every local file a document's **front matter** points at as a resource:
/// `bibliography:`, `csl:`, `css:`, and the three `include-*-body`/`-in-header` keys.
/// Absolute + normalized, resolved with the same containment rule as `{{< include >}}`.
///
/// Read-only, and deliberately separate from [`dependencies`], which tracks only
/// `{{< include >}}`. The site dev server watches both: it filtered its rebuild set by
/// `{{< include >}}` alone, so a `.bib` edit matched no page and the preview kept showing
/// the stale citation (the single-doc server rebuilds on any relevant event, so it was
/// never affected). Nothing here reads or parses the referenced files.
pub fn resource_dependencies(src: &str, base_dir: &Path) -> Vec<PathBuf> {
    let Some(fm) = crate::frontmatter::front_matter_block(src) else {
        return Vec::new();
    };
    let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(fm) else {
        return Vec::new(); // malformed front matter is reported elsewhere
    };
    let mut out = Vec::new();
    for key in [
        "bibliography",
        "csl",
        "css",
        "include-in-header",
        "include-before-body",
        "include-after-body",
    ] {
        collect_resource_paths(v.get(key), base_dir, &mut out);
    }
    out
}

/// Walk a front-matter value that may be a path, a `{ file: … }` map, or a sequence of
/// either, pushing each safely-resolvable path. Mirrors the shapes `doc_includes` and
/// `bibliography_paths` accept; a `{ text: … }` inline block names no file.
fn collect_resource_paths(v: Option<&serde_yaml::Value>, base_dir: &Path, out: &mut Vec<PathBuf>) {
    use serde_yaml::Value;
    let mut push = |s: &str| {
        if let Some(p) = safe_join(base_dir, s.trim())
            && !out.contains(&p)
        {
            out.push(p);
        }
    };
    match v {
        Some(Value::String(s)) => push(s),
        Some(Value::Mapping(_)) => {
            if let Some(Value::String(f)) = v.and_then(|v| v.get("file")) {
                push(f);
            }
        }
        Some(Value::Sequence(seq)) => {
            for item in seq {
                collect_resource_paths(Some(item), base_dir, out);
            }
        }
        _ => {}
    }
}

fn collect_deps(src: &str, base_dir: &Path, stack: &mut Vec<PathBuf>, out: &mut Vec<PathBuf>) {
    let mut in_code: Option<(char, usize)> = None;
    for line in src.lines() {
        let was_in_code = in_code.is_some();
        in_code = next_code_state(in_code, line);
        if was_in_code || in_code.is_some() {
            continue; // a `{{< include >}}` inside a code fence isn't a dependency
        }
        let Some(raw) = parse_include(line) else {
            continue;
        };
        // The watch list is a list of FILES. A `#fragment` include still depends on the
        // whole file (editing any part of it can move the anchored section), and leaving
        // the fragment on would make the path unresolvable — so the dev server would
        // watch nothing and a fragment include would never hot-reload.
        let (rel, _fragment) = split_target(raw);
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

/// Split an include target into its path and its optional `#fragment`
/// (`part.tmd#sec-proof` → `("part.tmd", Some("sec-proof"))`).
///
/// The **first** `#` separates, as it does in a URL. A path is free to contain a later
/// one; a path that begins with `#`, or a target with an empty fragment, is not a
/// fragment reference at all and is returned whole so the usual "file not found"
/// diagnostic reports what the author actually wrote.
///
/// Every consumer of an include path has to route through this or it resolves
/// `part.tmd#sec-proof` as a filename: the expander, the dev server's watch list, and on
/// the editor side the document link, go-to-definition and the line-count inlay hint.
pub fn split_target(target: &str) -> (&str, Option<&str>) {
    match target.split_once('#') {
        Some((path, frag)) if !path.is_empty() && !frag.is_empty() => (path, Some(frag)),
        _ => (target, None),
    }
}

/// The half-open line range (0-based) of the section anchored `{#id}` in `text`: the
/// heading line that carries the id, through to the next heading of **equal or shallower**
/// level. `None` when no heading carries it.
///
/// The heading line is part of the range. A transcluded section keeps its own heading, so
/// what lands in the parent is the section as written rather than a decapitated body.
///
/// Three things it deliberately shares with the rest of the tree rather than re-deriving:
/// the anchor is whatever [`crate::render::parse_heading_attr`] calls an explicit `{#id}`
/// (a second parser could disagree with cross-references about what an anchor *is*), the
/// "equal or shallower closes it" rule is the one `lsp_fold` folds by, and fenced code is
/// skipped with this module's own [`next_code_state`] — a `# comment` on a `{python}`
/// cell's first line is not an h1.
pub fn section_lines(text: &str, id: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut in_code: Option<(char, usize)> = None;
    let mut found: Option<(usize, u8)> = None;

    for (i, line) in lines.iter().enumerate().skip(front_matter_end(&lines)) {
        let was_in_code = in_code.is_some();
        in_code = next_code_state(in_code, line);
        if was_in_code || in_code.is_some() {
            continue;
        }
        let Some((level, _)) = atx_heading(line) else {
            continue;
        };
        match found {
            // Still looking: does this heading carry the id we were asked for?
            None => {
                if parse_heading_attr(line).and_then(|(_, id)| id).as_deref() == Some(id) {
                    found = Some((i, level));
                }
            }
            // Inside the section: the first heading at or above its level ends it.
            Some((start, start_level)) if level <= start_level => return Some((start, i)),
            _ => {}
        }
    }
    found.map(|(start, _)| (start, lines.len()))
}

/// The line index just past a leading `---` front-matter block, or 0 when there is none.
/// YAML comments (`# note:`) inside front matter would otherwise scan as h1 headings.
fn front_matter_end(lines: &[&str]) -> usize {
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return 0;
    }
    lines
        .iter()
        .skip(1)
        .position(|l| l.trim_end() == "---")
        // +1 for the skipped opening fence, +1 to land past the closing one.
        .map(|i| i + 2)
        .unwrap_or(0)
}

/// The text of lines `[start, end)` of `text`, borrowed rather than copied. `end` past the
/// last line means "to the end", which is what a section running to EOF asks for.
fn slice_lines(text: &str, start: usize, end: usize) -> &str {
    let (mut line, mut pos) = (0usize, 0usize);
    let (mut begin, mut finish) = (None, text.len());
    for l in text.split_inclusive('\n') {
        if line == start {
            begin = Some(pos);
        }
        if line == end {
            finish = pos;
            break;
        }
        pos += l.len();
        line += 1;
    }
    let b = begin.unwrap_or(text.len());
    &text[b..finish.max(b)]
}

/// `^(#{1,6})\s+(.*)$` → `(level, title)`. The same shape `lsp_outline::atx_heading` has;
/// duplicated across the crate boundary rather than made public, because core cannot
/// depend on the server crate.
fn atx_heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &line[hashes..];
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some((hashes as u8, rest.trim()))
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

/// A label for an included file, **always relative to the primary document's
/// directory**, climbing with `..` when the include lives outside it. `target` is
/// absolute (it comes from [`safe_join`]), so `primary_base` is absolutized to the
/// same coordinate system first.
///
/// The relative form is not cosmetic: it is the contract `data-source-file` carries.
/// The editor companion resolves a label with `path.resolve(dirname(doc), label)` and
/// generates the reverse-sync key with `path.relative(dirname(doc), file)`, so a label
/// that is not primary-doc-relative breaks click-to-source both ways. Emitting the
/// absolute path also leaked the author's home directory into published HTML and made
/// builds differ between machines.
fn label_for(target: &Path, primary_base: &Path) -> String {
    let primary = absolutize(primary_base);
    relative_from(&primary, target).unwrap_or_else(|| target.to_string_lossy().into_owned())
}

/// The lexical path from directory `base` to `target`, climbing with `..` as needed and
/// joined with `/` (the separator the source-map protocol uses). Both must be absolute
/// and normalized. `None` when they sit on different filesystem roots (distinct Windows
/// drive/UNC prefixes), where no relative path exists.
fn relative_from(base: &Path, target: &Path) -> Option<String> {
    let b: Vec<Component> = base.components().collect();
    let t: Vec<Component> = target.components().collect();
    if b.first() != t.first() {
        return None;
    }
    let shared = b.iter().zip(&t).take_while(|(x, y)| x == y).count();
    let mut parts = vec![".."; b.len() - shared]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    parts.extend(
        t[shared..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Resolve `rel` against `base_dir`, refusing path-traversal escapes. An absolute
/// `rel`, or a result that climbs above the *project root* (the nearest ancestor of
/// `base_dir` holding a `.git` or `_site.yml`, else `base_dir` itself), returns
/// `None` so the caller can refuse it. This blocks `{{< include /etc/passwd >}}`
/// and `../../../../etc/...` while still allowing the corpus's `../../_includes/...`
/// (the repo root contains both the doc and `_includes/`). Shared by include
/// resolution, theme/CSS includes, and format-resource reads.
pub(crate) fn safe_join(base_dir: &Path, rel: &str) -> Option<PathBuf> {
    try_join_in(base_dir, rel, None).ok()
}

/// Why [`try_join_in`] refused a path. Callers that report to the author use this to
/// separate "the file is not there" (their own read fails) from "the file is there and
/// was deliberately not read" — different problems with different fixes, and reporting
/// the second as the first is what let a refused-but-present `.bib` go unnoticed while
/// every reference on the page silently degraded to a bare citation key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refused {
    /// Absolute, or a `../` climb above the containment root.
    OutsideRoot,
    /// In-root lexically, but the path resolves through a symlink to a target outside
    /// the enclosing repository.
    SymlinkOutsideRepo,
}

/// Like [`safe_join`], but the containment boundary can be given explicitly as `root`
/// instead of being inferred by walking to the nearest ancestor `.git`/`_site.yml`.
/// First-party single-document invocations (preview/build of one `.tmd`) pass the
/// invoked doc's own directory here so an untrusted document dropped inside a larger
/// checkout cannot `../`-climb to a sibling repo-local file (the walk would otherwise
/// widen the boundary to that ancestor's marker). `None` keeps the walk, which the
/// site path relies on (its `_site.yml` marker bounds the walk to the project) and the
/// corpus's loose `../../_includes/` fixture depends on.
pub(crate) fn safe_join_in(
    base_dir: &Path,
    rel: &str,
    explicit_root: Option<&Path>,
) -> Option<PathBuf> {
    try_join_in(base_dir, rel, explicit_root).ok()
}

/// [`safe_join_in`] with the refusal reason kept, for callers that report it.
pub(crate) fn try_join_in(
    base_dir: &Path,
    rel: &str,
    explicit_root: Option<&Path>,
) -> Result<PathBuf, Refused> {
    let relp = Path::new(rel);
    // An absolute path (incl. a Windows drive/UNC root) escapes immediately.
    if relp.has_root() || relp.is_absolute() {
        return Err(Refused::OutsideRoot);
    }
    // Resolve against an *absolute* base so the containment check and the returned
    // target share one coordinate system: a relative CLI path (e.g. the doc's
    // `corpus/posts/x` parent) would otherwise make `containment_root`'s absolute
    // boundary and a relative `target` incomparable, silently rejecting legitimate
    // `../../_includes/…` includes. `std::path::absolute` only prepends the cwd +
    // normalizes lexically (no filesystem touch, no symlink resolution).
    let abs_base = absolutize(base_dir);
    let target = normalize(&abs_base.join(relp));
    // An explicit root (a first-party single-doc invocation) bounds the boundary to
    // exactly that directory; otherwise infer it by walking to an ancestor marker.
    let root = match explicit_root {
        Some(r) => absolutize(r),
        None => containment_root(&abs_base),
    };
    // Lexical containment first. This also lets a not-yet-existing in-root target
    // through, so the caller's read fails with a "not found" diagnostic rather than a
    // traversal one.
    if !target.starts_with(&root) {
        return Err(Refused::OutsideRoot);
    }
    // Symlink defense: a lexical check alone is fooled by an in-tree symlink whose target
    // escapes the project (its bytes would be read + inlined verbatim). When the target
    // exists, its *canonical* path must stay within the canonical `symlink_root`,
    // mirroring `serve_asset_from`. The lexical `target` is still what we return, so
    // labels / `data-source-file` are unchanged.
    match target.canonicalize() {
        // A non-existent target cannot be a symlink escape; the caller's read reports it.
        Err(_) => Ok(target),
        Ok(ctarget) => {
            let boundary = symlink_root(&abs_base, &root);
            match boundary.canonicalize() {
                Ok(cboundary) if ctarget.starts_with(&cboundary) => Ok(target),
                // Either the target escaped, or no boundary could be canonicalized to
                // clear it against. Both refuse: an unresolvable boundary used to skip
                // the check entirely, so a bare-filename invocation (empty base dir,
                // hence empty root) disabled it and inlined the escaping target.
                _ => Err(Refused::SymlinkOutsideRepo),
            }
        }
    }
}

/// Make `p` absolute by prepending the current working directory if needed, then
/// normalizing `.`/`..` lexically. No symlink resolution.
///
/// `std::path::absolute` errors on the **empty** path, which is exactly what
/// `Path::new("index.tmd").parent()` yields when the CLI is handed a bare filename.
/// Returning `p` unchanged there left the base relative and the containment root empty,
/// which no longer names a directory that can be canonicalized. Resolve against the cwd
/// instead, so every caller gets a real absolute boundary.
pub(crate) fn absolutize(p: &Path) -> PathBuf {
    let abs = std::path::absolute(p)
        .or_else(|_| std::path::absolute(Path::new(".")).map(|cwd| cwd.join(p)))
        .unwrap_or_else(|_| p.to_path_buf());
    normalize(&abs)
}

/// The boundary the *symlink* check uses: the enclosing repository (nearest ancestor
/// holding `.git`), falling back to the lexical `root` when the project is not a
/// checkout.
///
/// It is deliberately wider than the lexical root. The lexical check governs what the
/// *document text* may ask for, where `../../etc/passwd` is plainly an escape attempt. A
/// symlink is a different thing: a filesystem fact placed by whoever owns the checkout,
/// which the document text cannot conjure. The repository is therefore the honest unit of
/// first-party trust, and confining symlinks to a narrower `_site.yml` root only forced
/// authors to duplicate files that are already theirs (a book sharing one
/// `references.bib` with the `paper/` beside it was refused). Escapes that actually leave
/// the checkout, `/etc/passwd` or `~/.ssh/id_rsa`, are still refused.
///
/// The walk only ever *widens*: a `.git` found below `root` (a nested checkout) is
/// skipped, so an explicit root can never be narrowed by a marker inside it.
fn symlink_root(base_dir: &Path, root: &Path) -> PathBuf {
    let base = base_dir.to_path_buf();
    let mut cur: &Path = &base;
    loop {
        if cur.join(".git").exists() && root.starts_with(cur) {
            return cur.to_path_buf();
        }
        match cur.parent() {
            Some(p) if !p.as_os_str().is_empty() => cur = p,
            _ => return root.to_path_buf(),
        }
    }
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

/// The containment root for a **single invoked document** — `build`, `preview`, `check`,
/// `read` or the LSP handed one `.tmd` rather than a project directory: the nearest
/// ancestor of `doc_dir` holding `_site.yml`, else `doc_dir` itself.
///
/// This is deliberately *not* [`containment_root`], which also stops at `.git`. The two
/// markers mean different things to a document that was named on a command line:
///
/// - `_site.yml` is an author declaring a project boundary, and it is the same root the
///   site build passes. Honouring it is what makes `build <page>` and `build <site>` emit
///   the same document (PP-3, 2026-07-26): before this, a page pulling
///   `../../_includes/…` built one way with its include and the other way without it.
/// - `.git` is a checkout, not a project the author pointed this tool at. Widening to it
///   is exactly the escape PT-2 closed (`9359a2c`): an untrusted `.tmd` dropped anywhere
///   inside a checkout could `../`-climb to a sibling repo-local file. It never widens a
///   single invoked document again.
///
/// So the boundary a document gets is the project it belongs to, and a document with no
/// declared project is its own project. Pinned by
/// `crates/core/tests/include_root_parity.rs`.
///
/// **Known gap, deliberate:** a site with no `_site.yml` at all (`build <dir>` accepts a
/// bare directory) declares no boundary, so a single-document render of one of its pages
/// still roots at that page. Nothing in the tree can infer an undeclared boundary; the fix
/// is to declare one.
pub fn single_doc_root(doc_dir: &Path) -> PathBuf {
    let base = absolutize(doc_dir);
    let mut cur: &Path = &base;
    loop {
        if cur.join("_site.yml").exists() {
            return cur.to_path_buf();
        }
        match cur.parent() {
            Some(p) if !p.as_os_str().is_empty() => cur = p,
            _ => return base.clone(),
        }
    }
}

/// The canonical repository boundary for `dir` (see [`symlink_root`]), for callers that
/// walk the filesystem themselves instead of resolving a path through [`try_join_in`].
/// Page discovery and the build's asset mirror are those callers: they read directories
/// directly, so each has to apply this boundary by hand or it applies none at all.
pub fn repo_boundary(dir: &Path) -> PathBuf {
    let abs = absolutize(dir);
    let root = symlink_root(&abs, &abs);
    root.canonicalize().unwrap_or(root)
}

/// Lexically normalize a path (resolve `.` and `..`) without touching the
/// filesystem, so labels and cycle checks are stable.
pub(crate) fn normalize(p: &Path) -> PathBuf {
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

    /// Item 160. A scratch project holding one shared fragment file with three anchored
    /// sections, so a test can assert both what is pulled and what is left behind.
    fn fragment_project(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tali-frag-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("_site.yml"), b"title: t\n").unwrap();
        // Line numbers matter to every assertion below, so they are written out here:
        //  1 `# Shared parts {#sec-all}`      5 `## Setup {#sec-setup}`
        //  3 (blank) 4 `Intro prose.`         7 `Setup body.`
        //  9 `## Derivation {#sec-deriv}`    11 `Derivation body.`
        // 13 `### A detail {#sec-detail}`    15 `Detail body.`
        // 17 `## After {#sec-after}`         19 `After body.`
        std::fs::write(
            root.join("parts.tmd"),
            "# Shared parts {#sec-all}\n\nIntro prose.\n\n## Setup {#sec-setup}\n\n\
             Setup body.\n\n## Derivation {#sec-deriv}\n\nDerivation body.\n\n\
             ### A detail {#sec-detail}\n\nDetail body.\n\n## After {#sec-after}\n\n\
             After body.\n",
        )
        .unwrap();
        root
    }

    #[test]
    fn a_fragment_include_pulls_only_the_anchored_section() {
        let root = fragment_project("only");
        let (text, _origins, warnings) = resolve_warned(
            "before\n{{< include parts.tmd#sec-deriv >}}\nafter\n",
            &root,
        );

        assert!(
            warnings.is_empty(),
            "expected a clean expansion: {warnings:?}"
        );
        assert!(text.contains("## Derivation {#sec-deriv}"), "got:\n{text}");
        assert!(text.contains("Derivation body."), "got:\n{text}");
        // The section runs to the next heading of EQUAL OR SHALLOWER level, so its own
        // deeper subsection comes with it...
        assert!(
            text.contains("### A detail {#sec-detail}") && text.contains("Detail body."),
            "a subsection of the named section belongs to it:\n{text}"
        );
        // ...and everything outside it stays behind. Each of these is a separate way the
        // slice could be wrong: too early a start, too late an end, or no slicing at all.
        for absent in ["Intro prose.", "Setup body.", "After body."] {
            assert!(
                !text.contains(absent),
                "{absent:?} is outside the named section but was pulled in:\n{text}"
            );
        }
        // The parent's own lines survive on both sides of the transclusion.
        assert!(text.starts_with("before\n") && text.trim_end().ends_with("after"));

        let _ = std::fs::remove_dir_all(&root);
    }

    /// **The merge gate item 160 was filed under: the source map must not perturb.**
    ///
    /// A transcluded section is a *slice* of its file, so the naive implementation maps
    /// its first line to line 1 and every line after it is off by however far down the
    /// file the section sits. Click-to-source would then land near the top of the shared
    /// file instead of on the heading, which is the one thing this feature must not cost.
    #[test]
    fn a_fragment_maps_back_to_its_real_lines_in_the_source_file() {
        let root = fragment_project("map");
        let (text, origins, _w) =
            resolve_warned("intro\n{{< include parts.tmd#sec-deriv >}}\n", &root);

        assert_eq!(origins.len(), text.lines().count(), "one origin per line");
        // The parent's own first line is the parent's line 1, with no file label.
        assert_eq!(origins[0].file, None);
        assert_eq!(origins[0].line, 1);

        // Every transcluded line must name the shared file AND its true line there.
        let heading = text
            .lines()
            .position(|l| l.contains("{#sec-deriv}"))
            .expect("the section heading was transcluded");
        assert_eq!(origins[heading].file.as_deref(), Some("parts.tmd"));
        assert_eq!(
            origins[heading].line, 9,
            "`## Derivation` is line 9 of parts.tmd; mapping it to 1 is the off-by-offset \
             bug this test exists for"
        );
        let body = text
            .lines()
            .position(|l| l == "Derivation body.")
            .expect("the section body was transcluded");
        assert_eq!(origins[body].line, 11, "the body is line 11 of parts.tmd");
        let detail = text
            .lines()
            .position(|l| l == "Detail body.")
            .expect("the nested subsection was transcluded");
        assert_eq!(origins[detail].line, 15, "the detail body is line 15");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_whole_file_include_is_untouched_by_the_fragment_work() {
        // The regression guard on the `line_offset` parameter: with no fragment the
        // offset is 0 and every origin is exactly what it was before item 160.
        let root = fragment_project("whole");
        let (text, origins, warnings) = resolve_warned("{{< include parts.tmd >}}\n", &root);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(text.contains("Intro prose.") && text.contains("After body."));
        assert_eq!(origins[0].file.as_deref(), Some("parts.tmd"));
        assert_eq!(
            origins[0].line, 1,
            "a whole-file include still starts at line 1"
        );
        let last = text.lines().position(|l| l == "After body.").unwrap();
        assert_eq!(origins[last].line, 19);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_fragment_that_names_no_section_warns_and_stays_literal() {
        let root = fragment_project("missing");
        let (text, _origins, warnings) =
            resolve_warned("{{< include parts.tmd#sec-nope >}}\n", &root);
        assert!(
            text.contains("{{< include parts.tmd#sec-nope >}}"),
            "an unresolvable fragment stays literal, like every other bad include:\n{text}"
        );
        let w = warnings.first().expect("a located warning");
        assert_eq!(w.line, 1);
        assert_eq!(
            w.target, "parts.tmd#sec-nope",
            "the warning quotes what was written"
        );
        assert!(
            w.reason.contains("sec-nope") && w.reason.contains("parts.tmd"),
            "the reason must name both halves so the author can see which is wrong: {}",
            w.reason
        );
        // Nothing from the file leaked in on the way to failing.
        assert!(!text.contains("Derivation body."));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_fragment_include_is_a_dependency_of_the_whole_file() {
        // If `dependencies` kept the `#fragment` on the path, `safe_join` would resolve
        // nothing and the dev server would watch NO file — a fragment include would
        // silently stop hot-reloading.
        let root = fragment_project("deps");
        let deps = dependencies("{{< include parts.tmd#sec-deriv >}}\n", &root);
        assert_eq!(deps.len(), 1, "the fragment's file is watched: {deps:?}");
        assert!(deps[0].ends_with("parts.tmd"), "{:?}", deps[0]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn two_fragments_of_one_file_are_not_a_cycle() {
        // The cycle guard keys on the file. Keying on file+fragment would let a real
        // cycle through; keying too eagerly would refuse this, which is the normal case
        // the feature exists for.
        let root = fragment_project("twice");
        let (text, _o, warnings) = resolve_warned(
            "{{< include parts.tmd#sec-setup >}}\n\n{{< include parts.tmd#sec-after >}}\n",
            &root,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(
            text.contains("Setup body.") && text.contains("After body."),
            "both sections expand:\n{text}"
        );
        assert!(
            !text.contains("Derivation body."),
            "and nothing between them:\n{text}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn split_target_separates_a_path_from_its_fragment() {
        assert_eq!(split_target("a.tmd#sec-x"), ("a.tmd", Some("sec-x")));
        assert_eq!(split_target("d/a.tmd"), ("d/a.tmd", None));
        // Degenerate forms are NOT fragment references: returning them whole makes the
        // ordinary "file not found" diagnostic quote what the author actually typed.
        assert_eq!(split_target("a.tmd#"), ("a.tmd#", None));
        assert_eq!(split_target("#sec-x"), ("#sec-x", None));
    }

    #[test]
    fn section_lines_bounds_a_section_by_heading_level() {
        let text = "# A {#sec-a}\n\nx\n\n## B {#sec-b}\n\ny\n\n### C {#sec-c}\n\nz\n\n## D {#sec-d}\n\nw\n";
        // `## B` runs through its own `### C` and stops at the next `##`.
        assert_eq!(section_lines(text, "sec-b"), Some((4, 12)));
        // A deeper section stops at the next heading of any shallower level.
        assert_eq!(section_lines(text, "sec-c"), Some((8, 12)));
        // The last section runs to the end of the file.
        assert_eq!(section_lines(text, "sec-d"), Some((12, 15)));
        // The top-level heading owns everything under it.
        assert_eq!(section_lines(text, "sec-a"), Some((0, 15)));
        assert_eq!(section_lines(text, "sec-missing"), None);
    }

    #[test]
    fn section_lines_ignores_headings_that_are_not_headings() {
        // A `# comment` on a code cell's first line is the exact shape that broke
        // folding in the LSP batch, and it would end a section early here too.
        let fenced =
            "## S {#sec-s}\n\n```{python}\n# not a heading {#sec-fake}\nx = 1\n```\n\nafter\n";
        assert_eq!(
            section_lines(fenced, "sec-s"),
            Some((0, 8)),
            "a fenced `#` line must not close the section"
        );
        assert_eq!(
            section_lines(fenced, "sec-fake"),
            None,
            "and it defines no anchor either"
        );
        // A YAML comment in front matter is the same trap one layer up.
        let fm = "---\ntitle: t\n# note {#sec-yaml}\n---\n\n## S {#sec-s}\n\nbody\n";
        assert_eq!(section_lines(fm, "sec-yaml"), None);
        assert_eq!(section_lines(fm, "sec-s"), Some((5, 8)));
        // `#nospace` is not an ATX heading.
        assert_eq!(section_lines("#x {#sec-x}\n", "sec-x"), None);
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
    fn resource_dependencies_finds_bibliography_csl_and_css() {
        let root = std::env::temp_dir().join(format!("tali-resdeps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(".git"), b"").unwrap(); // project-root marker for safe_join

        let src = "---\ntitle: T\nbibliography:\n  - refs.bib\n  - more.bib\ncsl: ieee.csl\n\
                   css: custom.css\ninclude-in-header: head.html\n---\n\nBody.\n";
        let deps = resource_dependencies(src, &root);
        let names: Vec<String> = deps
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [
                "refs.bib",
                "more.bib",
                "ieee.csl",
                "custom.css",
                "head.html"
            ],
            "every front-matter resource, in key order"
        );
        assert!(deps.iter().all(|p| p.is_absolute()), "absolute: {deps:?}");

        // A scalar `bibliography:` and a `{ file: … }` map are the other accepted shapes.
        let one = resource_dependencies("---\nbibliography: refs.bib\n---\n", &root);
        assert_eq!(one.len(), 1);
        let mapped = resource_dependencies("---\ncss:\n  file: a.css\n---\n", &root);
        assert_eq!(mapped.len(), 1);
        // An inline `{ text: … }` block names no file.
        assert!(resource_dependencies("---\ncss:\n  text: 'p{}'\n---\n", &root).is_empty());

        // No front matter, malformed front matter, and an escaping path yield nothing.
        assert!(resource_dependencies("# Just prose\n", &root).is_empty());
        assert!(resource_dependencies("---\nbib: \"unterminated\n---\n", &root).is_empty());
        assert!(
            resource_dependencies("---\nbibliography: /etc/passwd\n---\n", &root).is_empty(),
            "an absolute path escapes the project root"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn label_for_a_sibling_include_is_relative_to_the_primary_doc() {
        // `data-source-file` is *defined* as "relative to the primary document's
        // directory": the companion resolves it with `path.resolve(dirname(doc), label)`
        // and produces the reverse-sync key with `path.relative(dirname(doc), file)`.
        // An include reached via `../` used to fall through to the absolute path, which
        // leaked the author's home directory into published HTML, made builds
        // machine-dependent, and broke reverse sync (no `..` form to match).
        let primary = Path::new("/proj/posts/pca");
        let target = Path::new("/proj/_includes/three-scene.tmd");
        assert_eq!(
            label_for(target, primary),
            "../../_includes/three-scene.tmd"
        );

        // Underneath the primary dir: unchanged, no `./` prefix.
        assert_eq!(
            label_for(Path::new("/proj/posts/pca/_bits/x.tmd"), primary),
            "_bits/x.tmd"
        );

        // The label must round-trip: joining it to the primary dir returns the target.
        for t in [
            "/proj/_includes/three-scene.tmd",
            "/proj/posts/pca/_bits/x.tmd",
            "/other/tree/y.tmd",
        ] {
            let label = label_for(Path::new(t), primary);
            assert!(
                !label.starts_with('/'),
                "label must not be absolute: {label}"
            );
            assert_eq!(normalize(&primary.join(&label)), Path::new(t));
        }
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

    #[test]
    #[cfg(unix)]
    fn safe_join_refuses_an_in_tree_symlink_that_escapes_the_root() {
        // PT-1: `safe_join` confined only *lexically* (no symlink resolution), so an
        // in-tree symlink whose target is OUTSIDE the project root passed the
        // `starts_with(root)` check and the bytes were read + inlined verbatim into the
        // rendered page (arbitrary-file disclosure, surviving `--no-exec`). The canonical
        // path of the resolved target must stay within the canonical root.
        use std::os::unix::fs::symlink;
        let uniq = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(format!("tali-symlink-root-{uniq}"));
        let secret = std::env::temp_dir().join(format!("tali-symlink-secret-{uniq}.txt"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("post")).unwrap();
        std::fs::write(root.join(".git"), b"").unwrap();
        std::fs::write(&secret, b"TOP SECRET").unwrap();
        // An in-tree symlink pointing at the external secret.
        symlink(&secret, root.join("post/theme.css")).unwrap();
        // A real in-root file, to prove the fix does not reject legitimate resources.
        std::fs::write(root.join("post/real.css"), b"body{}").unwrap();

        let base = root.join("post");
        assert!(
            safe_join(&base, "real.css").is_some(),
            "a real in-root file must still resolve"
        );
        assert!(
            safe_join(&base, "theme.css").is_none(),
            "an in-tree symlink whose target escapes the root must be refused"
        );

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_file(&secret);
    }

    #[test]
    fn safe_join_in_confines_to_an_explicit_root_despite_an_ancestor_marker() {
        // PT-2: `containment_root`'s walk widens the boundary to the nearest ancestor
        // holding `.git`/`_site.yml`. An untrusted doc dropped inside an existing checkout
        // could therefore `../`-climb to a sibling repo-local file. With an EXPLICIT root
        // (the CLI-invoked doc dir), `safe_join_in` must confine to that root and refuse
        // any climb above it, even when a `.git` sits higher up.
        //   <tmp>/.git                 (ancestor checkout marker)
        //   <tmp>/proj/doc/            (the invoked doc's dir = the explicit root)
        //   <tmp>/proj/sibling.txt     (a repo-local file ABOVE the explicit root)
        let uniq = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let tmp = std::env::temp_dir().join(format!("tali-pt2-{uniq}"));
        let _ = std::fs::remove_dir_all(&tmp);
        let doc = tmp.join("proj/doc");
        std::fs::create_dir_all(&doc).unwrap();
        std::fs::write(tmp.join(".git"), b"").unwrap();
        std::fs::write(tmp.join("proj/sibling.txt"), b"secret").unwrap();
        std::fs::write(doc.join("local.txt"), b"ok").unwrap();

        // With the doc dir as the explicit root: an in-root file resolves...
        assert!(
            safe_join_in(&doc, "local.txt", Some(&doc)).is_some(),
            "an in-root file must resolve under an explicit root"
        );
        // ...but climbing above the explicit root is refused, despite the ancestor `.git`.
        assert!(
            safe_join_in(&doc, "../sibling.txt", Some(&doc)).is_none(),
            "a climb above the explicit root must be refused even with an ancestor .git"
        );
        // Contrast: the walk (None) climbs to `<tmp>/.git`, so the SAME escape is allowed.
        // That widening is exactly what the explicit root closes.
        assert!(
            safe_join(&doc, "../sibling.txt").is_some(),
            "sanity: the inferred-marker walk still permits the climb (the behavior PT-2 bounds)"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
