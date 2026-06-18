//! Format extensions: `format: <ext>-revealjs|-html` loads
//! `_extensions/<ext>/_extension.yml` and injects the includes/theme/resources
//! its `contributes:` block declares. Kept in its own module so the core stays a
//! thin injector; `use super::*` reaches PageIncludes + the shared include/theme
//! helpers.

use super::*;

/// The raw `format:` key (`glass-revealjs`, `revealjs`, `html`, …) — inline value
/// or the first block sub-key. Used to spot a format-extension reference.
fn detect_format_name(front_matter: &str) -> Option<String> {
    let lines: Vec<&str> = front_matter.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some(rest) = line.trim_end().strip_prefix("format:") else {
            continue;
        };
        let inline = rest.trim();
        if !inline.is_empty() {
            return Some(inline.trim_matches(['"', '\'']).to_string());
        }
        // Block form: the first indented sub-key is the format name.
        for sub in &lines[i + 1..] {
            if sub.trim().is_empty() {
                continue;
            }
            if !sub.starts_with(char::is_whitespace) {
                break; // dedented out of the block without a sub-key
            }
            let key = sub.trim().trim_end_matches(':').trim_matches(['"', '\'']);
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }
        return None;
    }
    None
}
/// A `format: <ext>-<base>` reference to an installed format extension, resolved
/// to its directory. Recognized base formats; the part before `-<base>` is the
/// extension name.
struct ExtensionRef {
    name: String,
    base: &'static str,
    /// `<base_dir>/_extensions/<name>`.
    dir: PathBuf,
}

/// Parse `format:` into an [`ExtensionRef`], or `None` when it is absent, names a
/// bare base format (`revealjs`/`html`), or there is no project dir — none of
/// which is an error (so no warning). A `None` here means "not an extension
/// request"; a request that *is* made but then fails to load *does* warn.
fn extension_ref(front_matter: &str, base_dir: Option<&Path>) -> Option<ExtensionRef> {
    let fmt = detect_format_name(front_matter)?;
    let (ext, base) = ["revealjs", "html"]
        .iter()
        .find_map(|b| fmt.strip_suffix(&format!("-{b}")).map(|e| (e, *b)))?;
    let dir = base_dir?;
    if ext.is_empty() {
        return None;
    }
    Some(ExtensionRef {
        name: ext.to_string(),
        base,
        dir: find_extension_dir(dir, ext),
    })
}

/// Locate an installed extension: the nearest `_extensions/<name>/` walking up from
/// `base_dir` to the filesystem root, so a chapter deep in a book finds extensions
/// vendored at the project root (not just beside the page). Falls back to the
/// page-relative path when none is found, so the not-found warning still points
/// somewhere sensible.
fn find_extension_dir(base_dir: &Path, name: &str) -> PathBuf {
    let mut dir = Some(base_dir);
    while let Some(d) = dir {
        let cand = d.join("_extensions").join(name);
        if cand.join("_extension.yml").is_file() {
            return cand;
        }
        dir = d.parent();
    }
    base_dir.join("_extensions").join(name)
}

/// Load + parse an extension's `_extension.yml`. Because the caller asked for this
/// extension explicitly via `format:`, a missing or malformed manifest is an
/// authoring mistake worth surfacing (the dev menu / build log), not a silent
/// no-op — so failures push a `warnings` entry.
fn load_manifest(r: &ExtensionRef, warnings: &mut Vec<String>) -> Option<serde_yaml::Value> {
    let manifest = r.dir.join("_extension.yml");
    let text = match std::fs::read_to_string(&manifest) {
        Ok(t) => t,
        Err(_) => {
            warnings.push(format!(
                "format extension '{}' not found (looked for {})",
                r.name,
                manifest.display()
            ));
            return None;
        }
    };
    match serde_yaml::from_str::<serde_yaml::Value>(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            warnings.push(format!("could not parse {}: {e}", manifest.display()));
            None
        }
    }
}

mod quarto;

/// The normalized set of things an extension contributes, built from either the
/// flat native manifest or the Quarto `contributes.formats.<base>` shape. Values
/// are the raw YAML (resolved into includes/resources by the caller).
#[derive(Default)]
struct Contribution {
    head: Option<serde_yaml::Value>,
    body_start: Option<serde_yaml::Value>,
    body_end: Option<serde_yaml::Value>,
    css: Option<serde_yaml::Value>,
    theme: Option<serde_yaml::Value>,
    resources: Option<serde_yaml::Value>,
    shortcodes: Option<serde_yaml::Value>,
}

impl Contribution {
    /// The flat native manifest: contribution keys live at the top level.
    fn from_native(m: &serde_yaml::Value) -> Contribution {
        Contribution {
            head: m.get("head").cloned(),
            body_start: m.get("body-start").cloned(),
            body_end: m.get("body-end").cloned(),
            css: m.get("css").cloned(),
            theme: m.get("theme").cloned(),
            resources: m.get("resources").cloned(),
            shortcodes: m.get("shortcodes").cloned(),
        }
    }
}

/// Recognized native top-level keys: metadata (informational) + contributions.
const NATIVE_MANIFEST_KEYS: &[&str] = &[
    "name",
    "title",
    "description",
    "author",
    "version",
    "theme",
    "css",
    "head",
    "body-start",
    "body-end",
    "resources",
    "shortcodes",
];

/// Warn on unrecognized top-level keys of a native manifest (a closed set).
fn validate_manifest(m: &serde_yaml::Value, ext: &str, warnings: &mut Vec<String>) {
    let Some(map) = m.as_mapping() else { return };
    for k in map.keys() {
        let Some(key) = k.as_str() else { continue };
        if !NATIVE_MANIFEST_KEYS.contains(&key) {
            let hint = crate::frontmatter::closest(key, NATIVE_MANIFEST_KEYS)
                .map(|s| format!(" (did you mean `{s}`?)"))
                .unwrap_or_default();
            warnings.push(format!(
                "extension '{ext}': unknown manifest key `{key}`{hint}"
            ));
        }
    }
}

/// Build a [`Contribution`], dispatching on manifest shape: a `contributes:` block
/// is the Quarto shape (the isolated compat reader); otherwise the flat native
/// schema. Delete `quarto.rs` + this branch to drop Quarto-extension support.
fn load_contribution(
    r: &ExtensionRef,
    m: &serde_yaml::Value,
    warnings: &mut Vec<String>,
) -> Contribution {
    if m.get("contributes").is_some() {
        quarto::contribution(m, r.base, &r.name, warnings)
    } else {
        validate_manifest(m, &r.name, warnings);
        Contribution::from_native(m)
    }
}

impl ExtensionRef {
    /// A reference to an explicitly-named extension (the `extensions: [..]` key),
    /// resolved for the doc's base format `base`.
    fn named(name: &str, base: &'static str, base_dir: &Path) -> ExtensionRef {
        ExtensionRef {
            name: name.to_string(),
            base,
            dir: find_extension_dir(base_dir, name),
        }
    }
}

/// Load an extension's manifest and build its [`Contribution`] (+ its directory),
/// or `None` if the manifest can't be read/parsed.
fn contribution_for(
    r: &ExtensionRef,
    warnings: &mut Vec<String>,
) -> Option<(PathBuf, Contribution)> {
    let m = load_manifest(r, warnings)?;
    Some((r.dir.clone(), load_contribution(r, &m, warnings)))
}

/// Turn a [`Contribution`] into the `PageIncludes` it injects (head/body/css +
/// theme layers ahead of the header, and `resources` recorded for copying).
fn apply_contribution(c: &Contribution, ext_dir: &Path) -> PageIncludes {
    let mut inc = includes_from_parts(
        c.head.as_ref(),
        c.body_start.as_ref(),
        c.body_end.as_ref(),
        c.css.as_ref(),
        Some(ext_dir),
    );
    // Contributed `theme:` CSS layers, inlined ahead of the header so the doc's own
    // front matter can still override. (`.scss` needs a compiler we don't ship; only
    // `.css` is inlined.)
    let theme = resolve_theme_layers(c.theme.as_ref(), ext_dir);
    if !theme.is_empty() {
        inc.in_header = format!("{theme}{}", inc.in_header);
    }
    // `resources` (file names relative to the extension) are copied next to the
    // output so an injected `<script src="x.js">` resolves at runtime.
    if let Some(res) = &c.resources {
        for name in res
            .as_sequence()
            .map(|s| s.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_else(|| res.as_str().into_iter().collect())
        {
            inc.resources.push(ext_dir.join(name));
        }
    }
    inc
}

/// If the doc's `format:` names a format extension (`<ext>-revealjs`/`<ext>-html`),
/// load `_extensions/<ext>/_extension.yml` and resolve what it contributes. Empty
/// when there's no such extension; a *failed* load is reported via `warnings`.
pub(super) fn resolve_format_extension(
    front_matter: &str,
    base_dir: Option<&Path>,
    warnings: &mut Vec<String>,
) -> PageIncludes {
    let Some(r) = extension_ref(front_matter, base_dir) else {
        return PageIncludes::default();
    };
    match contribution_for(&r, warnings) {
        Some((dir, c)) => apply_contribution(&c, &dir),
        None => PageIncludes::default(),
    }
}

/// Apply the `extensions: [a, b]` list — the general (format-agnostic) activation.
/// Each named extension contributes for the doc's `base` format, merged in order
/// (so a later one wins). This is how a shortcode/enhancer extension is switched on
/// without hijacking `format:`.
pub(super) fn resolve_named_extensions(
    front_matter: &str,
    base_dir: Option<&Path>,
    base: &'static str,
    warnings: &mut Vec<String>,
) -> PageIncludes {
    let Some(dir) = base_dir else {
        return PageIncludes::default();
    };
    let mut inc = PageIncludes::default();
    for name in parse_extensions(front_matter) {
        let r = ExtensionRef::named(&name, base, dir);
        if let Some((d, c)) = contribution_for(&r, warnings) {
            inc.merge(&apply_contribution(&c, &d));
        }
    }
    inc
}

/// The `extensions:` front-matter list (explicitly activated extensions), or empty.
fn parse_extensions(front_matter: &str) -> Vec<String> {
    // Strip the leading `---` and everything from the closing fence, then parse the
    // body as YAML — robust whether or not the fences are present.
    let body = match front_matter.trim_start().strip_prefix("---") {
        Some(rest) => rest.rsplit_once("---").map(|(b, _)| b).unwrap_or(rest),
        None => front_matter,
    };
    serde_yaml::from_str::<serde_yaml::Value>(body)
        .ok()
        .as_ref()
        .and_then(|v| v.get("extensions"))
        .and_then(|v| v.as_sequence())
        .map(|s| {
            s.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// --- Declarative shortcodes --------------------------------------------------

/// The raw front-matter block at the top of `src` (without the `---` fences), or
/// `""` when there isn't one. Used to find the active extension before parsing.
fn front_matter_block(src: &str) -> &str {
    let rest = src
        .strip_prefix("---\n")
        .or_else(|| src.strip_prefix("---\r\n"));
    match rest {
        Some(body) => body.split_once("\n---").map(|(fm, _)| fm).unwrap_or(""),
        None => "",
    }
}

/// Shortcode templates (name → HTML template) from every active extension: the
/// `format:` one plus each `extensions:` entry. Loads silently — failures are
/// reported once by the include resolvers on the same render (this runs in the
/// earlier shortcode-expansion pass).
fn shortcode_templates(front_matter: &str, base_dir: Option<&Path>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut ignore = Vec::new();
    // the format extension (`format: <ext>-base`)
    if let Some(r) = extension_ref(front_matter, base_dir) {
        gather_shortcodes(&r, &mut out, &mut ignore);
    }
    // each `extensions: [..]` entry (shortcodes are format-agnostic, so the base
    // passed here is a don't-care — it only affects the unused includes block)
    if let Some(dir) = base_dir {
        for name in parse_extensions(front_matter) {
            gather_shortcodes(
                &ExtensionRef::named(&name, "html", dir),
                &mut out,
                &mut ignore,
            );
        }
    }
    out
}

/// Merge one extension's declared shortcodes into `out`.
fn gather_shortcodes(
    r: &ExtensionRef,
    out: &mut HashMap<String, String>,
    warnings: &mut Vec<String>,
) {
    let Some(m) = load_manifest(r, warnings) else {
        return;
    };
    let c = load_contribution(r, &m, warnings);
    if let Some(map) = c.shortcodes.as_ref().and_then(|s| s.as_mapping()) {
        for (k, val) in map {
            if let (Some(name), Some(tmpl)) = (k.as_str(), val.as_str()) {
                out.insert(name.to_string(), tmpl.to_string());
            }
        }
    }
}

/// Expand declarative shortcodes (`{{< name args >}}`) using the templates the
/// active extensions contribute. Line-preserving — each invocation opens and
/// closes on one line and expands to inline HTML — so the include source map stays
/// valid. Fenced code blocks are skipped, so a `{{< … >}}` shown as an *example*
/// in ```` ``` ```` stays literal; unknown shortcodes are left untouched.
pub(super) fn expand_shortcodes(src: &str, base_dir: Option<&Path>) -> String {
    let templates = shortcode_templates(front_matter_block(src), base_dir);
    if templates.is_empty() || !src.contains("{{<") {
        return src.to_string();
    }
    let mut out = String::with_capacity(src.len());
    let mut in_code = false;
    for (i, line) in src.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            out.push_str(line);
        } else if in_code {
            out.push_str(line); // literal inside a code block (it's an example)
        } else {
            out.push_str(&expand_in_line(line, &templates));
        }
    }
    if src.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Replace every `{{< name args >}}` that opens and closes on this line with its
/// declared template; leave unrecognized ones (and unterminated spans) verbatim.
fn expand_in_line(line: &str, templates: &HashMap<String, String>) -> String {
    if !line.contains("{{<") {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find("{{<") {
        let Some(rel_end) = rest[start..].find(">}}") else {
            break; // no close on this line: leave the remainder as written
        };
        let end = start + rel_end;
        out.push_str(&rest[..start]);
        let inner = rest[start + 3..end].trim();
        match render_shortcode(inner, templates) {
            Some(html) => out.push_str(&html),
            None => out.push_str(&rest[start..end + 3]), // unknown: keep verbatim
        }
        rest = &rest[end + 3..];
    }
    out.push_str(rest);
    out
}

/// Render one `name args` shortcode body against the templates, or `None` when
/// the name isn't declared. Args are `key=value` (named → `{{key}}`) or bare
/// (positional → `{{1}}`, `{{2}}`, …); quotes group values with spaces.
fn render_shortcode(inner: &str, templates: &HashMap<String, String>) -> Option<String> {
    let toks = tokenize_args(inner);
    let (name, args) = toks.split_first()?;
    let template = templates.get(name)?;
    let mut positional = Vec::new();
    let mut named = Vec::new();
    for a in args {
        match a.split_once('=') {
            Some((k, v))
                if !k.is_empty()
                    && k.chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-') =>
            {
                named.push((k.to_string(), v.to_string()))
            }
            _ => positional.push(a.clone()),
        }
    }
    // Collapse the template to one line (line-preserving), then substitute.
    let mut html = template.replace('\n', " ");
    for (i, v) in positional.iter().enumerate() {
        html = html.replace(&format!("{{{{{}}}}}", i + 1), v);
    }
    for (k, v) in &named {
        html = html.replace(&format!("{{{{{k}}}}}"), v);
    }
    Some(html)
}

/// Whitespace-split `inner`, keeping quoted values (`key="a b"`) as one token and
/// stripping the surrounding quotes.
fn tokenize_args(inner: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in inner.chars() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                } else {
                    cur.push(ch);
                }
            }
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}
