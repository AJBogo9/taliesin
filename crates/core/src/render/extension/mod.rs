//! Format extensions: `format: <ext>-revealjs|-html` loads
//! `_extensions/<ext>/_extension.yml` and injects the includes/theme/resources its
//! flat native manifest declares. Kept in its own module so the core stays a thin
//! injector; `use super::*` reaches PageIncludes + the shared include/theme helpers.

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
/// extension name (the native manifest is format-agnostic, so the base itself is
/// only used to *recognize* the reference, not retained).
struct ExtensionRef {
    name: String,
    /// `<base_dir>/_extensions/<name>`.
    dir: PathBuf,
}

/// Parse `format:` into an [`ExtensionRef`], or `None` when it is absent, names a
/// bare base format (`revealjs`/`html`), or there is no project dir — none of
/// which is an error (so no warning). A `None` here means "not an extension
/// request"; a request that *is* made but then fails to load *does* warn.
fn extension_ref(front_matter: &str, base_dir: Option<&Path>) -> Option<ExtensionRef> {
    let fmt = detect_format_name(front_matter)?;
    let ext = ["revealjs", "html"]
        .iter()
        .find_map(|b| fmt.strip_suffix(&format!("-{b}")))?;
    let dir = base_dir?;
    if ext.is_empty() {
        return None;
    }
    Some(ExtensionRef {
        name: ext.to_string(),
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
fn load_manifest(r: &ExtensionRef, warnings: &mut Vec<Warning>) -> Option<serde_yaml::Value> {
    let manifest = r.dir.join("_extension.yml");
    let text = match std::fs::read_to_string(&manifest) {
        Ok(t) => t,
        Err(_) => {
            warnings.push(Warning::new(format!(
                "format extension '{}' not found (looked for {})",
                r.name,
                manifest.display()
            )));
            return None;
        }
    };
    match serde_yaml::from_str::<serde_yaml::Value>(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            warnings.push(Warning::new(format!(
                "could not parse {}: {e}",
                manifest.display()
            )));
            None
        }
    }
}

/// The normalized set of things an extension contributes, built from the flat
/// native manifest. Values are the raw YAML (resolved into includes/resources by
/// the caller).
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
fn validate_manifest(m: &serde_yaml::Value, ext: &str, warnings: &mut Vec<Warning>) {
    let Some(map) = m.as_mapping() else { return };
    for k in map.keys() {
        let Some(key) = k.as_str() else { continue };
        if !NATIVE_MANIFEST_KEYS.contains(&key) {
            let hint = crate::frontmatter::closest(key, NATIVE_MANIFEST_KEYS)
                .map(|s| format!(" (did you mean `{s}`?)"))
                .unwrap_or_default();
            warnings.push(Warning::new(format!(
                "extension '{ext}': unknown manifest key `{key}`{hint}"
            )));
        }
    }
}

/// Build a [`Contribution`] from the flat native manifest, warning on any
/// unrecognized top-level key.
fn load_contribution(
    r: &ExtensionRef,
    m: &serde_yaml::Value,
    warnings: &mut Vec<Warning>,
) -> Contribution {
    validate_manifest(m, &r.name, warnings);
    Contribution::from_native(m)
}

impl ExtensionRef {
    /// A reference to an explicitly-named extension (the `extensions: [..]` key).
    fn named(name: &str, base_dir: &Path) -> ExtensionRef {
        ExtensionRef {
            name: name.to_string(),
            dir: find_extension_dir(base_dir, name),
        }
    }
}

/// Load an extension's manifest and build its [`Contribution`] (+ its directory),
/// or `None` if the manifest can't be read/parsed.
fn contribution_for(
    r: &ExtensionRef,
    warnings: &mut Vec<Warning>,
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

/// The built-in base mode (`dark`/`light`) a contributed `theme:` selects, if any:
/// the first entry of a `[dark, x.css]` list or a bare `theme: dark` scalar.
/// `.css`/`.scss` layer names are not base modes (they're inlined separately).
fn contributed_theme_base(theme: Option<&serde_yaml::Value>) -> Option<&'static str> {
    let first = match theme? {
        serde_yaml::Value::String(s) => s.as_str(),
        serde_yaml::Value::Sequence(seq) => seq.first()?.as_str()?,
        _ => return None,
    };
    match first {
        "dark" => Some("dark"),
        "light" | "default" => Some("light"),
        _ => None,
    }
}

/// If the doc's `format:` names a format extension (`<ext>-revealjs`/`<ext>-html`),
/// load `_extensions/<ext>/_extension.yml` and resolve what it contributes: the
/// injected `PageIncludes` plus the built-in theme base it selects (so the deck
/// defaults to the extension's light/dark when the doc names no `theme:`). Empty
/// when there's no such extension; a *failed* load is reported via `warnings`.
pub(super) fn resolve_format_extension(
    front_matter: &str,
    base_dir: Option<&Path>,
    warnings: &mut Vec<Warning>,
) -> (PageIncludes, Option<&'static str>) {
    let Some(r) = extension_ref(front_matter, base_dir) else {
        return (PageIncludes::default(), None);
    };
    match contribution_for(&r, warnings) {
        Some((dir, c)) => (
            apply_contribution(&c, &dir),
            contributed_theme_base(c.theme.as_ref()),
        ),
        None => (PageIncludes::default(), None),
    }
}

/// Apply the `extensions: [a, b]` list — the general (format-agnostic) activation.
/// Each named extension is merged in order (so a later one wins). This is how a
/// shortcode/enhancer extension is switched on without hijacking `format:`.
pub(super) fn resolve_named_extensions(
    front_matter: &str,
    base_dir: Option<&Path>,
    warnings: &mut Vec<Warning>,
) -> PageIncludes {
    let Some(dir) = base_dir else {
        return PageIncludes::default();
    };
    let mut inc = PageIncludes::default();
    for name in parse_extensions(front_matter) {
        let r = ExtensionRef::named(&name, dir);
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

/// Shortcode templates (name → HTML template) from every active extension: the
/// `format:` one plus each `extensions:` entry. Loads silently — failures are
/// reported once by the include resolvers on the same render (this runs in the
/// earlier shortcode-expansion pass).
fn shortcode_templates(front_matter: &str, base_dir: Option<&Path>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut ignore: Vec<Warning> = Vec::new();
    // the format extension (`format: <ext>-base`)
    if let Some(r) = extension_ref(front_matter, base_dir) {
        gather_shortcodes(&r, &mut out, &mut ignore);
    }
    // each `extensions: [..]` entry
    if let Some(dir) = base_dir {
        for name in parse_extensions(front_matter) {
            gather_shortcodes(&ExtensionRef::named(&name, dir), &mut out, &mut ignore);
        }
    }
    out
}

/// Merge one extension's declared shortcodes into `out`.
fn gather_shortcodes(
    r: &ExtensionRef,
    out: &mut HashMap<String, String>,
    warnings: &mut Vec<Warning>,
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
pub(super) fn expand_shortcodes(src: &str, base_dir: Option<&Path>) -> (String, Vec<Warning>) {
    let templates = shortcode_templates(
        crate::frontmatter::front_matter_block(src).unwrap_or(""),
        base_dir,
    );
    let mut warnings: Vec<Warning> = Vec::new();
    // Process whenever a `{{<` is present: besides extension-declared templates,
    // `render_shortcode` also handles the built-in `{{< embed >}}`, which must work
    // with no extensions loaded.
    if !src.contains("{{<") {
        return (src.to_string(), warnings);
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
            out.push_str(&expand_in_line(line, &templates, i + 1, &mut warnings));
        }
    }
    if src.ends_with('\n') {
        out.push('\n');
    }
    (out, warnings)
}

/// Replace every `{{< name args >}}` that opens and closes on this line with its
/// declared template; leave unrecognized ones (and unterminated spans) verbatim.
/// Inline code spans (`` `…` ``, ``` ``…`` ```) are copied through untouched, so a
/// shortcode shown as an *example* in backticks (e.g. `` `{{< embed x.qmd >}}` ``)
/// stays literal — mirroring how fenced blocks are skipped in `expand_shortcodes`.
fn expand_in_line(
    line: &str,
    templates: &HashMap<String, String>,
    line_no: usize,
    warnings: &mut Vec<Warning>,
) -> String {
    if !line.contains("{{<") {
        return line.to_string();
    }
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < line.len() {
        if bytes[i] == b'`' {
            // An inline code span: copy through the matching backtick run verbatim
            // so a `{{< … >}}` inside it is not expanded.
            let run = line[i..].bytes().take_while(|&c| c == b'`').count();
            let ticks = &line[i..i + run];
            if let Some(rel) = line[i + run..].find(ticks) {
                let close = i + run + rel + run;
                out.push_str(&line[i..close]);
                i = close;
            } else {
                out.push_str(ticks); // unterminated run: copy the backticks, keep scanning
                i += run;
            }
        } else if line[i..].starts_with("{{<") {
            let Some(rel_end) = line[i + 3..].find(">}}") else {
                out.push_str(&line[i..]); // no close on this line: leave as written
                break;
            };
            let end = i + 3 + rel_end;
            let inner = line[i + 3..end].trim();
            // The built-in `{{< input >}}` reactive control needs the line number + the
            // warning sink (for located diagnostics), which render_shortcode doesn't carry,
            // so it is expanded here. An extension may still override `input` with its own
            // declared template (checked first).
            if inner.split_whitespace().next() == Some("input") && !templates.contains_key("input")
            {
                let toks = tokenize_args(inner);
                out.push_str(&input_shortcode(&toks[1..], line_no, warnings));
                i = end + 3;
                continue;
            }
            match render_shortcode(inner, templates) {
                Some(html) => out.push_str(&html),
                None => {
                    // No extension or built-in declares this name. Keep it verbatim
                    // (nothing is lost), but warn: a typo'd shortcode name should be
                    // visible in the build log / preview diagnostics, not shipped as
                    // literal text into the page. `include` is handled in an earlier
                    // pass (`includes::resolve`); a leftover one means that pass already
                    // reported it (file missing/unsafe/cyclic), so don't double-warn.
                    let name = inner.split_whitespace().next().unwrap_or(inner);
                    if name != "include" {
                        warnings.push(
                            Warning::new(format!(
                                "unknown shortcode `{{{{< {name} >}}}}` at line {line_no} \
                                 (no extension declares it; left as literal text)"
                            ))
                            .at(None, line_no as u32),
                        );
                    }
                    out.push_str(&line[i..end + 3]); // unknown: keep verbatim
                }
            }
            i = end + 3;
        } else {
            let ch = line[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Render one `name args` shortcode body against the templates, or `None` when
/// the name isn't declared. Args are `key=value` (named → `{{key}}`) or bare
/// (positional → `{{1}}`, `{{2}}`, …); quotes group values with spaces.
fn render_shortcode(inner: &str, templates: &HashMap<String, String>) -> Option<String> {
    let toks = tokenize_args(inner);
    let (name, args) = toks.split_first()?;
    let Some(template) = templates.get(name) else {
        // No extension declares this name. `{{< embed deck.qmd [title="…"] >}}` is a
        // built-in fallback: it embeds another document's deck view in an isolating
        // iframe with a fullscreen affordance (the deck is built/served as a
        // dependency, see `embed_targets`). An extension's own `embed` template, if
        // declared, takes precedence above.
        if name == "embed" {
            return embed_path(args).map(|p| embed_html(&p, embed_title(args).as_deref()));
        }
        // `{{< video clip.mp4 [dark=clip-dark.mp4] [poster=…] [caption="…"] >}}` — a
        // framed, autoplaying, muted, looping screencast (the marketing pattern),
        // authored in Markdown so a page needs no raw `<video>` HTML. With `dark=`, the
        // light clip plays on a light page and the dark clip on a dark page (toggled by
        // `html[data-theme]`), so the screencast matches the surrounding theme.
        if name == "video" {
            return embed_path(args).map(|src| {
                video_html(
                    &src,
                    shortcode_named(args, "dark").as_deref(),
                    shortcode_named(args, "poster").as_deref(),
                    shortcode_named(args, "caption").as_deref(),
                )
            });
        }
        return None;
    };
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

/// The first bare (non `key=value`) argument of an `embed`/`video` shortcode: the path
/// to the deck document or media file, relative to the embedding page.
///
/// A token is a *named* argument only when it looks like `key=value` with a plain
/// identifier key (`[A-Za-z][A-Za-z0-9_-]*` before the first `=`). Anything else is the
/// positional path, so a path carrying a query string (`clip.mp4?token=abc`) is **not**
/// mistaken for a named arg just because it contains an `=` after the `?`.
fn embed_path(args: &[String]) -> Option<String> {
    args.iter().find(|a| !is_named_arg(a)).cloned()
}

/// Whether `tok` is a `key=value` named shortcode argument: an identifier key
/// (`[A-Za-z][A-Za-z0-9_-]*`) immediately followed by `=`. A `?` (or any other
/// non-identifier character) before the first `=` means the `=` belongs to a query
/// string / value, not a key, so the token is positional (a path) instead.
fn is_named_arg(tok: &str) -> bool {
    let Some(key) = tok.split('=').next().filter(|_| tok.contains('=')) else {
        return false;
    };
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The optional `title="…"` argument (used as the iframe's accessible name).
fn embed_title(args: &[String]) -> Option<String> {
    shortcode_named(args, "title")
}

/// A shortcode's `key=value` argument by name (quotes already stripped by the tokenizer).
fn shortcode_named(args: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    args.iter()
        .find_map(|a| a.strip_prefix(&prefix).map(str::to_string))
}

/// The built-in `{{< input name="k" type="slider" … >}}` reactive control: a static,
/// keyboard-accessible labeled control whose value feeds the `{js}` reactive graph
/// (qmd-js.js registers `[data-qmd-input]` and reuses the same `registerInput`/`scheduleFrom`
/// path as `//| viewof` cells). Five types (slider/range, number, checkbox, text, select);
/// the slider gets a live `<output>` readout. Emits located diagnostics (missing name,
/// unknown type with a did-you-mean, select without options) via `validate_input`. Raw-HTML,
/// passed through — the block model assigns it an id/sourcepos like any HTML block. Read-only:
/// reader interaction with the rendered view, never a source write.
fn input_shortcode(args: &[String], line_no: usize, warnings: &mut Vec<Warning>) -> String {
    let name = shortcode_named(args, "name").unwrap_or_default();
    let kind = shortcode_named(args, "type").unwrap_or_else(|| "slider".to_string());
    let label = shortcode_named(args, "label").unwrap_or_else(|| name.clone());
    let options = shortcode_named(args, "options");
    let value = shortcode_named(args, "value");
    for w in super::validate::validate_input(
        (!name.is_empty()).then_some(name.as_str()),
        Some(kind.as_str()),
        options.as_deref(),
        line_no,
        None,
    ) {
        warnings.push(w);
    }
    let ctrl_id = format!("qin-{line_no}");
    let name_a = escape_attr(&name);
    let num_attr = |k: &str| {
        shortcode_named(args, k)
            .map(|v| format!(" {k}=\"{}\"", escape_attr(&v)))
            .unwrap_or_default()
    };
    let control = match kind.as_str() {
        "select" => {
            let opts: String = options
                .as_deref()
                .unwrap_or("")
                .split(',')
                .map(str::trim)
                .filter(|o| !o.is_empty())
                .map(|o| {
                    let sel = if value.as_deref() == Some(o) {
                        " selected"
                    } else {
                        ""
                    };
                    format!("<option{sel}>{}</option>", html_escape(o))
                })
                .collect();
            format!(
                "<select id=\"{ctrl_id}\" class=\"qmd-input-control\" data-qmd-input=\"{name_a}\">{opts}</select>"
            )
        }
        "checkbox" => {
            let checked = if value.as_deref() == Some("true") {
                " checked"
            } else {
                ""
            };
            format!(
                "<input id=\"{ctrl_id}\" class=\"qmd-input-control\" data-qmd-input=\"{name_a}\" type=\"checkbox\"{checked}>"
            )
        }
        "text" => format!(
            "<input id=\"{ctrl_id}\" class=\"qmd-input-control\" data-qmd-input=\"{name_a}\" type=\"text\"{}>",
            num_attr("value")
        ),
        other => {
            // slider/range/number: numeric, sharing min/max/step/value
            let html_type = if other == "number" { "number" } else { "range" };
            format!(
                "<input id=\"{ctrl_id}\" class=\"qmd-input-control\" data-qmd-input=\"{name_a}\" type=\"{html_type}\"{}{}{}{}>",
                num_attr("min"),
                num_attr("max"),
                num_attr("step"),
                num_attr("value")
            )
        }
    };
    let readout = if kind == "slider" || kind == "range" {
        format!(
            "<output class=\"qmd-input-out\" data-qmd-out>{}</output>",
            html_escape(value.as_deref().unwrap_or(""))
        )
    } else {
        String::new()
    };
    format!(
        "<div class=\"qmd-input\"><label class=\"qmd-input-label\" for=\"{ctrl_id}\">{}</label>{control}{readout}</div>",
        html_escape(&label)
    )
}

/// The HTML for a `{{< video >}}`: a framed autoplaying/muted/looping `<video>` (a
/// silent screencast) with an optional caption. With a `dark` source, both clips are
/// emitted and CSS shows the one matching `html[data-theme]`. Raw-HTML, passed through.
fn video_html(
    src: &str,
    dark: Option<&str>,
    poster: Option<&str>,
    caption: Option<&str>,
) -> String {
    let poster_attr = poster
        .map(|p| format!(" poster=\"{}\"", escape_attr(p)))
        .unwrap_or_default();
    let video = |s: &str, class: &str| {
        format!(
            "<video{cls} src=\"{}\"{poster_attr} autoplay muted loop playsinline></video>",
            escape_attr(s),
            cls = if class.is_empty() {
                String::new()
            } else {
                format!(" class=\"{class}\"")
            },
        )
    };
    let videos = match dark {
        Some(d) => format!(
            "{}{}",
            video(src, "qmd-video-light"),
            video(d, "qmd-video-dark")
        ),
        None => video(src, ""),
    };
    let cap = caption
        .map(|c| format!("<figcaption>{}</figcaption>", html_escape(c)))
        .unwrap_or_default();
    format!("<figure class=\"qmd-video\">{videos}{cap}</figure>")
}

/// Map a deck source path to its built output URL (`x.qmd` → `x.html`), leaving a
/// path that is already `.html` (or anything else) untouched.
fn deck_href(path: &str) -> String {
    match path.strip_suffix(".qmd") {
        Some(stem) => format!("{stem}.html"),
        None => path.to_string(),
    }
}

/// The HTML for an embedded deck: a responsive 16:9 iframe (isolating the deck's
/// full-viewport CSS/JS/keyboard from the host page) plus a fullscreen button and an
/// "open in a new tab" link. Emitted as a raw-HTML block, which the renderer passes
/// through.
fn embed_html(path: &str, title: Option<&str>) -> String {
    let href = escape_attr(&deck_href(path));
    // `title` lands in a double-quoted attribute, so escape `"` too (escape_attr,
    // not html_escape) — otherwise a `"` in the title breaks out of the attribute.
    let title = escape_attr(title.unwrap_or("Embedded slide deck"));
    format!(
        "<div class=\"qmd-embed\">\
         <div class=\"qmd-embed-stage\">\
         <iframe class=\"qmd-embed-frame\" src=\"{href}\" title=\"{title}\" loading=\"lazy\" allowfullscreen></iframe>\
         <button type=\"button\" class=\"qmd-embed-expand\" aria-label=\"Fullscreen\" onclick=\"this.closest('.qmd-embed').querySelector('iframe').requestFullscreen()\">\u{2922}</button>\
         </div>\
         <div class=\"qmd-embed-bar\">\
         <button type=\"button\" class=\"qmd-embed-btn\" onclick=\"this.closest('.qmd-embed').querySelector('iframe').requestFullscreen()\">\u{2922} Fullscreen</button>\
         <a class=\"qmd-embed-btn\" href=\"{href}\" target=\"_blank\" rel=\"noopener\">Open \u{2197}</a>\
         </div></div>"
    )
}

/// Invoke `f` with the inner body of each `{{< … >}}` on `line` that is *not* inside
/// an inline code span, so a shortcode shown as an example in backticks is ignored
/// (the same discipline `expand_in_line` uses when expanding).
fn each_shortcode(line: &str, mut f: impl FnMut(&str)) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < line.len() {
        if bytes[i] == b'`' {
            let run = line[i..].bytes().take_while(|&c| c == b'`').count();
            let ticks = &line[i..i + run];
            match line[i + run..].find(ticks) {
                Some(rel) => i = i + run + rel + run,
                None => i += run,
            }
        } else if line[i..].starts_with("{{<") {
            let Some(rel_end) = line[i + 3..].find(">}}") else {
                break;
            };
            let end = i + 3 + rel_end;
            f(line[i + 3..end].trim());
            i = end + 3;
        } else {
            i += line[i..].chars().next().unwrap().len_utf8();
        }
    }
}

/// Every deck referenced by a `{{< embed PATH >}}` in `src` (paths as written,
/// relative to the page), deduped and in document order. Fenced and inline code are
/// skipped so an `embed` shown as an example stays inert. The site build/preview uses
/// this to also build/serve each referenced deck.
pub fn embed_targets(src: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut in_code = false;
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        each_shortcode(line, |inner| {
            let toks = tokenize_args(inner);
            if let Some((name, args)) = toks.split_first()
                && name == "embed"
                && let Some(p) = embed_path(args)
                && !out.contains(&p)
            {
                out.push(p);
            }
        });
    }
    out
}
