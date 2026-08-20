//! Resolve `{{< include path >}}` shortcodes into a single expanded
//! buffer, while keeping a line-level **source map** so every line of the
//! result can be traced back to the file and line it came from. This is what
//! lets click-to-source jump into the *included* file rather than the parent.

use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};

/// `src` with every **lone `\r`** rewritten to `\n`, which is what makes the rest of the
/// crate's `str::lines()` agree with comrak.
///
/// CommonMark ends a line at `\r\n`, at `\n`, **or at a lone `\r`**, and comrak implements
/// that; `str::lines()` implements the first two. A single stray CR (pasted terminal
/// output, a file that crossed a classic-Mac tool) therefore split the document into two
/// different line models, and every line index after it was off by a growing amount. It was
/// not a cosmetic drift: `slice_lines` handed the block walk the WRONG line, so from the CR
/// onwards every heading id became the empty-slug fallback `section` and every block id
/// became `fnv1a("")`, ids collided and deduped to `-1` suffixes, and no diagnostic
/// anywhere fired. Reproduced 2026-08-17 with
/// `printf 'line one\rline two\n\n## A heading\n\npara.\n'`.
///
/// **Normalizing rather than teaching ten call sites a new splitter** is the choice here.
/// The substitution is one character for one character, so every line number and every
/// column is preserved exactly, and click-to-source still lands where the author's cursor
/// is. The LSP is not affected and keeps `lsp_pos::lines`: it works against the client's
/// raw buffer, whose positions must stay in the client's own coordinates.
///
/// CRLF is deliberately left alone: `str::lines()` already strips it and already agrees
/// with comrak, so rewriting it would be churn.
///
/// Borrows when there is nothing to do, which is every real document.
pub fn normalize_line_endings(src: &str) -> Cow<'_, str> {
    let mut bytes = src.bytes().enumerate().filter(|(_, b)| *b == b'\r');
    if !bytes.any(|(i, _)| src.as_bytes().get(i + 1) != Some(&b'\n')) {
        return Cow::Borrowed(src);
    }
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(i) = rest.find('\r') {
        out.push_str(&rest[..i]);
        // CRLF stays whole; a lone CR becomes the terminator every other reader sees.
        if rest[i + 1..].starts_with('\n') {
            out.push_str("\r\n");
            rest = &rest[i + 2..];
        } else {
            out.push('\n');
            rest = &rest[i + 1..];
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

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
    root: Option<&Path>,        // explicit containment root (constant across recursion)
    stack: &mut Vec<PathBuf>,
    out_lines: &mut Vec<String>,
    out_origins: &mut Vec<LineOrigin>,
    out_warnings: &mut Vec<IncludeWarning>,
) {
    // The one ingest point for every source text this crate renders: the primary document
    // arrives here, and so does each included file (this function recurses with its
    // contents). See [`normalize_line_endings`] for what a lone `\r` did before this line.
    let normalized = normalize_line_endings(src);
    let src = normalized.as_ref();
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
        let Some(raw) = directive else {
            keep_line();
            continue;
        };
        let rel = raw;
        // Unsafe path (absolute or escaping the project root), or an include cycle:
        // leave the directive visible rather than reading outside the project / looping.
        let Some(target) = safe_join_in(base_dir, rel, root) else {
            drop_with_warning(raw, "path escapes the project root (or is absolute)");
            continue;
        };
        if stack.contains(&target) {
            drop_with_warning(raw, "include cycle");
            continue;
        }
        match std::fs::read_to_string(&target) {
            Ok(content) => {
                let label = label_for(&target, primary_base);
                let child_base = target.parent().unwrap_or(base_dir).to_path_buf();
                stack.push(target.clone());
                expand(
                    content.as_str(),
                    &child_base,
                    primary_base,
                    Some(label),
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

/// Every local file a document's **front matter** points at as a resource. `bibliography:`
/// is the whole list. Absolute + normalized, resolved with the same containment rule as
/// `{{< include >}}`.
///
/// `css:`, the three `include-*-body`/`-in-header` keys and `csl:` were listed here until
/// 2026-08-20, after the last of their reads was retired — so the dev server was watching
/// files that nothing parses, and a save on one rebuilt a page that could not have changed.
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
    collect_resource_paths(v.get("bibliography"), base_dir, &mut out);
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
        let Some(target) = safe_join(base_dir, raw) else {
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
/// `pub` because interpreter resolution (`taliesin-server`) needs the identical
/// treatment for the same reason: its upward `.venv` walk must start from an absolute
/// path, and a relative `python:` field has to be normalized against the project dir.
/// A second copy of this in the server crate is exactly the kind of near-duplicate that
/// drifts.
pub fn absolutize(p: &Path) -> PathBuf {
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
        assert_eq!(parse_include("{{< input x >}}"), None); // different shortcode
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
    fn resource_dependencies_finds_the_bibliography() {
        let root = std::env::temp_dir().join(format!("tali-resdeps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(".git"), b"").unwrap(); // project-root marker for safe_join

        let src = "---\ntitle: T\nbibliography:\n  - refs.bib\n  - more.bib\n---\n\nBody.\n";
        let deps = resource_dependencies(src, &root);
        let names: Vec<String> = deps
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            ["refs.bib", "more.bib"],
            "every front-matter resource, in declaration order"
        );
        assert!(deps.iter().all(|p| p.is_absolute()), "absolute: {deps:?}");

        // `css:` and `csl:` were watched here until 2026-08-20; neither names a read now,
        // so a watcher entry would rebuild pages that cannot have changed.
        for retired in [
            "---\ncss:\n  file: a.css\n---\n",
            "---\ncsl: ieee.csl\n---\n",
        ] {
            assert!(
                resource_dependencies(retired, &root).is_empty(),
                "a withdrawn key names no resource: {retired:?}"
            );
        }

        // A scalar `bibliography:` and a `{ file: … }` map are the other accepted shapes.
        // The collector is deliberately more permissive about shape than the reader: a
        // watcher that missed a file because it could not parse the spelling would show a
        // stale page, which is worse than watching one file too many.
        let one = resource_dependencies("---\nbibliography: refs.bib\n---\n", &root);
        assert_eq!(one.len(), 1);
        let mapped = resource_dependencies("---\nbibliography:\n  file: a.bib\n---\n", &root);
        assert_eq!(mapped.len(), 1);
        // An inline `{ text: … }` block names no file.
        assert!(
            resource_dependencies("---\nbibliography:\n  text: 'p{}'\n---\n", &root).is_empty()
        );

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
