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

/// A problem the include pass found, located back to the file + line that holds it so the
/// caller can surface a click-to-source diagnostic: an include directive that could not be
/// expanded (unsafe path, cycle, unreadable file), which is left literal rather than shipped
/// silently.
#[derive(Debug, Clone)]
pub struct IncludeWarning {
    /// The diagnostic, as the author reads it.
    pub message: String,
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
    resolve_within(src, base_dir, root, Budget::DEFAULT)
}

/// How far one document may expand: includes performed, and bytes of expanded source.
///
/// Both, because each bounds what the other cannot. A diamond (every file including the
/// next one twice) doubles per level, so a handful of small files expand past any memory;
/// the byte cap stops that. With an EMPTY leaf nothing is ever emitted, so no byte count
/// grows while the expansions still double: measured 2026-09-24 (release), 16 levels ran
/// 65,536 includes in 2.5 s with "no problems found", and each level doubles that. The
/// include cap stops that.
///
/// Generous against real pages: on 2026-09-24 the most includes any page in this repo made
/// was 7 and the largest page was 40 KB, so 1,000 includes and 4 MiB are over 100 times
/// either. Tight against the worst shape: one-line paragraphs cost the most per byte, and
/// 0.59 MB of them took 201 MB and 0.8 s to render (release, 2026-09-24), so 4 MiB is about
/// 1.4 GB where 16 MiB would be about 5.6 GB.
#[derive(Clone, Copy)]
struct Budget {
    includes: usize,
    bytes: usize,
}

impl Budget {
    const DEFAULT: Budget = Budget {
        includes: 1_000,
        bytes: 4 * 1024 * 1024,
    };
}

fn resolve_within(
    src: &str,
    base_dir: &Path,
    root: Option<&Path>,
    budget: Budget,
) -> (String, Vec<LineOrigin>, Vec<IncludeWarning>) {
    let primary = normalize_line_endings(src);
    let mut x = Expansion {
        primary_base: base_dir,
        primary: (&primary, absolutize(base_dir)),
        root,
        budget,
        used: Budget {
            includes: 0,
            bytes: 0,
        },
        spent: false,
        stack: Vec::new(),
        lines: Vec::new(),
        origins: Vec::new(),
        warnings: Vec::new(),
    };
    x.expand(src, base_dir, None);
    let mut text = x.lines.join("\n");
    if src.ends_with('\n') {
        text.push('\n');
    }
    (text, x.origins, x.warnings)
}

/// One include expansion's state, threaded through its recursion.
struct Expansion<'a> {
    /// Directory of the primary document (for nice labels).
    primary_base: &'a Path,
    /// The primary document's text and absolute directory. Nothing hands this pass the
    /// primary's PATH, so the cycle guard cannot hold it; a file in that directory with that
    /// text is the primary all the same (see [`Expansion::is_primary`]).
    primary: (&'a str, PathBuf),
    /// Explicit containment root, constant across the recursion.
    root: Option<&'a Path>,
    budget: Budget,
    /// What the expansion has used of `budget`.
    used: Budget,
    /// Set when a directive would pass `budget`: it and every later one are left as written,
    /// and only the first says so.
    spent: bool,
    /// Cycle guard: absolute paths currently expanding.
    stack: Vec<PathBuf>,
    lines: Vec<String>,
    origins: Vec<LineOrigin>,
    warnings: Vec<IncludeWarning>,
}

impl Expansion<'_> {
    /// Whether the file at `target`, holding `content`, is the primary document. Including
    /// it can only repeat the page from the top: its includes resolve exactly as the
    /// primary's did, back to this same file.
    fn is_primary(&self, target: &Path, content: &str) -> bool {
        let (text, dir) = &self.primary;
        target.parent() == Some(dir.as_path()) && normalize_line_endings(content) == *text
    }

    /// Append `src` (the file labelled `file_label`, `None` for the primary document, whose
    /// includes resolve against `base_dir`) with its includes expanded.
    fn expand(&mut self, src: &str, base_dir: &Path, file_label: Option<String>) {
        // The one ingest point for every source text this crate renders: the primary
        // document arrives here, and so does each included file (this function recurses
        // with its contents). See [`normalize_line_endings`] for what a lone `\r` did
        // before this line.
        let normalized = normalize_line_endings(src);
        let src = normalized.as_ref();
        let lines = FileLines::of(src, file_label.is_some());
        for (idx, line) in src.lines().enumerate() {
            // Emit `line` verbatim, mapped back to the current file (used whenever a
            // directive isn't expanded: ordinary text, or an unsafe/cyclic/unreadable
            // include).
            self.lines.push(line.to_string());
            self.origins.push(LineOrigin {
                file: file_label.clone(),
                line: idx + 1,
            });
            self.used.bytes += line.len() + 1;
            let Some(raw) = lines.directive(idx, line) else {
                continue;
            };
            if self.spent {
                continue; // the one budget warning has been given
            }
            // Unsafe path (absolute or escaping the project root), or an include cycle:
            // leave the directive visible rather than reading outside the project / looping.
            let refused: Option<String> = match safe_join_in(base_dir, raw, self.root) {
                None => Some("path escapes the project root (or is absolute)".into()),
                Some(target) if self.stack.contains(&target) => Some("include cycle".into()),
                Some(target) => match std::fs::read_to_string(&target) {
                    Ok(content) if self.is_primary(&target, &content) => {
                        Some("include cycle".into())
                    }
                    Ok(content)
                        if self.used.includes + 1 > self.budget.includes
                            || self.used.bytes + content.len() > self.budget.bytes =>
                    {
                        self.spent = true;
                        Some(format!(
                            "the document passes the include budget: at most {} includes and {} MiB",
                            self.budget.includes,
                            self.budget.bytes / (1024 * 1024)
                        ))
                    }
                    Ok(content) => {
                        // The directive line is replaced by the file it names.
                        self.lines.pop();
                        self.origins.pop();
                        self.used.bytes -= line.len() + 1;
                        self.used.includes += 1;
                        let label = label_for(&target, self.primary_base);
                        let child_base = target.parent().unwrap_or(base_dir).to_path_buf();
                        self.stack.push(target);
                        self.expand(&content, &child_base, Some(label));
                        self.stack.pop();
                        None
                    }
                    Err(_) => Some("file not found or unreadable".into()),
                },
            };
            // Record a located warning for an include that couldn't be expanded, so the
            // directive left in place surfaces as a click-to-source diagnostic in
            // build/preview/`check` instead of leaking silently.
            if let Some(reason) = refused {
                self.warnings.push(IncludeWarning {
                    message: format!("include not resolved ({reason}): {{{{< include {raw} >}}}}"),
                    file: file_label.clone(),
                    line: idx + 1,
                });
            }
        }
        if file_label.is_some()
            && let Some(open) = lines.fence_open_at_end(src.lines().count())
        {
            self.warnings.push(IncludeWarning {
                message: "code fence never closed: this included file ends inside it, so what \
                          follows the include renders as code"
                    .to_string(),
                file: file_label,
                line: open + 1,
            });
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
    let lines = FileLines::of(src, false);
    for (idx, line) in src.lines().enumerate() {
        let Some(raw) = lines.directive(idx, line) else {
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

/// ONE file's lines, as the include pass reads them. Each file is classified on its own,
/// since this pass is what builds the buffer the rest of the render sees.
///
/// Skips the parse when there is nothing to ask: a file that names no shortcode and, if it
/// is an included one, opens no code fence, which is nearly every file.
struct FileLines(Option<crate::lines::Lines>);

impl FileLines {
    fn of(src: &str, included: bool) -> FileLines {
        let needed =
            src.contains("{{<") || (included && (src.contains("```") || src.contains("~~~")));
        FileLines(needed.then(|| crate::lines::classify(src)))
    }

    /// The include target on 0-based line `idx` (whose text is `line`), if it is a
    /// directive: a line holding only `{{< include PATH >}}`, where markdown is read. One
    /// inside code (fenced or indented), raw HTML (a commented-out include stays commented
    /// out) or the front matter is text.
    fn directive<'a>(&self, idx: usize, line: &'a str) -> Option<&'a str> {
        let lines = self.0.as_ref()?;
        lines
            .line(idx)
            .kind
            .is_markdown()
            .then(|| parse_include(line))?
    }

    /// The 0-based line of a code fence the end of a `line_count`-line file leaves open.
    /// In an included file that fence runs on into whatever follows the include.
    fn fence_open_at_end(&self, line_count: usize) -> Option<usize> {
        let lines = self.0.as_ref()?;
        lines
            .fences
            .iter()
            .find(|f| !f.closed && f.end + 1 >= line_count)
            .map(|f| f.open)
    }
}

/// If `line` is solely a `{{< include PATH >}}` shortcode, return PATH.
pub(crate) fn parse_include(line: &str) -> Option<&str> {
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

    /// A scratch directory for a test that needs partials on disk.
    fn partials(tag: &str, files: &[(&str, &str)]) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tali-inc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for (name, text) in files {
            std::fs::write(d.join(name), text).unwrap();
        }
        d
    }

    /// Audit 2026-09-24, scanners #11: a partial that ends inside an open code fence runs
    /// on into the including file, so everything after the include (a second partial, a
    /// callout) was published inside its code block with no diagnostic. Said at the
    /// partial's own opening fence. A closed fence, and the primary document's own
    /// unclosed fence (which runs to its end and swallows nothing), are not reported.
    #[test]
    fn a_partial_that_ends_inside_a_code_fence_is_reported_at_its_fence() {
        let d = partials(
            "unclosed",
            &[
                ("_a.md", "Snippet:\n\n```python\nprint(1)\n"),
                ("_b.md", "SECOND\n\n```\nclosed\n```\n"),
            ],
        );
        let (_, _, warnings) = resolve_warned(
            "Intro.\n\n{{< include _a.md >}}\n\n{{< include _b.md >}}\n\n```\nopen\n",
            &d,
        );
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(warnings[0].file.as_deref(), Some("_a.md"));
        assert_eq!(warnings[0].line, 3);
        assert!(
            warnings[0].message.contains("never closed"),
            "{}",
            warnings[0].message
        );
    }

    /// Audit 2026-09-24, Part H: `main.tmd` including `_a.md`, which includes `main.tmd`,
    /// rendered the page twice (`# main`, then `# main` again as `main-1`) before the stack
    /// caught the cycle one level deeper, because the primary document is not on the stack:
    /// nothing hands this pass its path. A file in the primary's own directory whose text IS
    /// the primary's text is the primary, and expanding it can only repeat the page.
    #[test]
    fn including_the_primary_document_is_a_cycle_at_once() {
        let main = "# main\n\n{{< include _a.md >}}\n";
        let d = partials(
            "selfcycle",
            &[
                ("main.tmd", main),
                ("_a.md", "In a.\n\n{{< include main.tmd >}}\n"),
            ],
        );
        let (text, _, warnings) = resolve_warned(main, &d);
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(text.matches("# main").count(), 1, "{text}");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(
            (warnings[0].file.as_deref(), warnings[0].line),
            (Some("_a.md"), 3)
        );
        assert!(
            warnings[0].message.contains("include cycle"),
            "{}",
            warnings[0].message
        );
    }

    /// Audit 2026-09-24, Part H and leads `includes.rs:111/201`: a diamond (each file
    /// including the next one twice) doubles per level, so 16 tiny files expanded 65k times
    /// and a few more levels exhaust memory, with "no problems found". An empty leaf makes
    /// it worse: nothing is ever emitted, so no byte count grows while the expansions run
    /// for minutes. The expansion stops at the budget, once, with a located warning, and
    /// leaves the remaining directives literal.
    #[test]
    fn an_include_diamond_stops_at_the_budget_with_one_warning() {
        let mut files: Vec<(String, String)> = (0..12)
            .map(|i| {
                let next = format!("{{{{< include _d{}.md >}}}}", i + 1);
                (format!("_d{i}.md"), format!("L{i}\n\n{next}\n\n{next}\n"))
            })
            .collect();
        files.push(("_d12.md".to_string(), String::new()));
        let files: Vec<(&str, &str)> = files
            .iter()
            .map(|(n, t)| (n.as_str(), t.as_str()))
            .collect();
        let d = partials("diamond", &files);
        let src = "Top.\n\n{{< include _d0.md >}}\n";
        let unbounded = Budget {
            includes: usize::MAX,
            bytes: usize::MAX,
        };
        let (_, _, full) = resolve_within(src, &d, None, unbounded);
        assert!(
            full.is_empty(),
            "the unbounded expansion is clean: {full:?}"
        );
        for budget in [
            Budget {
                includes: 100,
                bytes: usize::MAX,
            },
            Budget {
                includes: usize::MAX,
                bytes: 1000,
            },
        ] {
            let (text, _, warnings) = resolve_within(src, &d, None, budget);
            assert!(text.len() < 2000, "{}", text.len());
            assert!(
                text.matches("L11").count() < 100,
                "{}",
                text.matches("L11").count()
            );
            assert_eq!(warnings.len(), 1, "{warnings:?}");
            assert!(
                warnings[0].message.contains("budget"),
                "{}",
                warnings[0].message
            );
            assert!(
                warnings[0].file.is_some(),
                "located in the partial that hit it"
            );
        }
        let _ = std::fs::remove_dir_all(&d);
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
        assert!(w.message.contains("etc/passwd"), "{}", w.message);
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
