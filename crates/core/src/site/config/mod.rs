//! Project config (`_site.yml`). The flat native schema is the only model.
//!
//! Native schema (everything top-level, HTML-only so no `format: html:` nesting):
//!
//! ```yaml
//! title: "My Site"
//! description: "…"
//! url: "https://…"            # site URL
//! favicon: favicon.svg
//! logo: logo.svg             # brand image in the navbar / book topbar
//! nav:                       # a list ⇒ left side; or { left: […], right: […] }
//!   - { text: Blog, href: blog.tmd }
//! footer:                    # a string ⇒ left text; or { left/center/right }
//!   right: [{ icon: github, href: "…" }]
//! chapters: [index.tmd, …]   # presence ⇒ a book (no `type:` needed)
//! ```
//!
//! Six keys were retired on 2026-08-02: `output:` and `toc:` (both wrote what the tool
//! already does — the build dir is `_site`/`_book`, and the sidebar TOC is decided per page
//! by heading count), and `css:`/`body-start:`/`body-end:` (raw injection at zero adoption,
//! folded into `head:`). `theorems:` went with the book-wide numbering policy. `head:`
//! itself — the last raw-injection hatch, and still at zero adoption two weeks later —
//! followed on 2026-08-18. None is read any more, and a stale one is reported as an
//! unknown config key.

use super::*;

/// The resolved project config — the single internal model every downstream
/// consumer reads.
#[derive(Debug, Clone, Default)]
pub struct SiteConfig {
    /// `chapters:` present ⇒ a book (a centred reading column + chapter drawer, no navbar).
    pub is_book: bool,
    pub title: Option<String>,
    /// `author:` as a scalar (`author: Ada`) or a sequence (`author: [Ada, Alan]`),
    /// normalized the same way a page's `author:` is (`frontmatter::string_list`). Held
    /// as a list, not a raw scalar, because reading a sequence as a scalar silently
    /// yielded nothing and published the site *title* as the author instead.
    pub(crate) authors: Vec<crate::author::Author>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub favicon: Option<String>,
    /// `logo:` — the brand image for the website navbar and the book topbar/drawer,
    /// resolved like `favicon:` (a project-relative path, depth-prefixed per page).
    /// Deliberately ONE image slot with no size/position sub-keys: the stylesheet caps
    /// it against the bar it sits in, and the `title:` supplies its `alt`, so a branded
    /// project needs exactly this one line. The same key name a deck's front matter
    /// already uses (`render::deck::deck_overlay_html`).
    pub logo: Option<String>,
    pub nav: Navbar,
    pub footer: Option<Footer>,
    /// Ordered chapter list (book only): a file name or `{ part, chapters }`.
    pub chapters: Vec<serde_yaml::Value>,
    /// Project-pinned Python interpreter (`python:` in `_site.yml`), highest
    /// precedence in interpreter resolution. `None` falls back to `.venv`/env/default.
    pub python: Option<String>,
    /// Project-wide `bibliography:` — `.bib` path(s) relative to the site root, shared by
    /// every page. It is a layer *under* each page's own `bibliography:`, so a post can
    /// cite a shared key and still add or override entries locally
    /// (`Site::shared_bibliography`).
    ///
    /// Empty = no shared bibliography, which is the pre-existing per-document-only world.
    pub bibliography: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Navbar {
    pub left: Vec<NavItem>,
    pub right: Vec<NavItem>,
}

#[derive(Debug, Clone, Default)]
pub struct Footer {
    pub left: Vec<NavItem>,
    pub center: Vec<NavItem>,
    pub right: Vec<NavItem>,
}

/// A navbar/footer entry. `text` is the label; `href` the link; `icon` a bundled
/// social glyph name (github / linkedin / rss / …) rendered as an inline SVG.
#[derive(Debug, Clone, Default)]
pub struct NavItem {
    pub text: Option<String>,
    pub href: Option<String>,
    pub icon: Option<String>,
}

/// Every recognized top-level native key (drives typo validation).
pub(crate) const NATIVE_KEYS: &[&str] = &[
    "title",
    "author",
    "description",
    "url",
    "favicon",
    "logo",
    // No site-level `image:`. It used to seed `og:image`/`twitter:image` for pages that
    // set none of their own; the auto-generated per-page social card took that over
    // entirely (`card::card_url`), leaving the key parsed, honored-looking, and inert.
    // Dropping it from the set is what makes it *say* so: a stale `image:` in a
    // `_site.yml` now draws the unknown-key diagnostic instead of silently doing nothing.
    // A page's own front-matter `image:` is unaffected and still live (its listing/in-page
    // thumbnail); this set is `_site.yml` keys only.
    // No `output:`, retired 2026-08-02: both projects that set it wrote the default.
    // No `toc:`, retired 2026-08-02: the sidebar TOC is now automatic on an article page
    // with enough headings (`Site::page_toc`), and a page's own front-matter `toc:` still
    // forces it either way. A site-wide switch in front of an auto-gate was a knob in
    // front of a decision the page already makes.
    // No `head:`, retired 2026-08-18. It was the last raw-injection hatch (the
    // per-document `css:`/`include-*` family and `body-start:`/`body-end:` went on
    // 2026-08-02), kept then on the argument that a published tool needs exactly one. Two
    // weeks later it was still used by zero documents in the tree, so it went the same way
    // as the six it outlived: an escape hatch nobody reaches for is surface, not capability.
    "nav",
    "footer",
    "chapters",
    "python",
    // No `theorems:`. The book-wide numbering policy went with front-matter
    // `theorems.numbered` on 2026-08-02; `shared:` is per-chapter and stays there.
    "bibliography",
];

/// `nav:` section keys (the `{ left, right }` mapping form). A typo here silently drops
/// the whole side, so it warns.
const NAV_SECTION_KEYS: &[&str] = &["left", "right"];
/// `footer:` section keys (the `{ left, center, right }` mapping form).
const FOOTER_SECTION_KEYS: &[&str] = &["left", "center", "right"];
/// The keys of a single nav/footer item (`{ text, href, icon }`).
const NAV_ITEM_KEYS: &[&str] = &["text", "href", "icon"];

/// Stable prefix on the warning a malformed `_site.yml` pushes. A malformed config is a
/// *real* error (the site silently degrades to defaults). The site build matches this
/// prefix to fail on a malformed config with no `--strict`, and the live preview watcher
/// matches it to keep the last-good site instead of replacing it with the degraded
/// default. Keep it stable: those consumers key off it (see `crates/server/src/build.rs`
/// + `serve_site/mod.rs`).
pub const MALFORMED_CONFIG_PREFIX: &str = "_site.yml is not valid YAML";

/// The project config's file name, which every diagnostic about it is located in.
const SITE_YML: &str = "_site.yml";

/// A diagnostic about `_site.yml` itself, at `line` when the key it is about could be
/// found (see [`key_line`]). Located in the file rather than prefixed onto the message:
/// a `_site.yml:7:` baked into the text was a location no reader could click, and every
/// verb then attributed the whole string to `_site.yml` with no line (audit 2026-09-24
/// NEW-A).
pub(super) fn config_warning(line: Option<usize>, severity: Severity, message: String) -> Warning {
    let mut w = Warning::new(message).severity(severity);
    w.file = Some(SITE_YML.to_string());
    w.line = line.map(|l| l as u32);
    w
}

/// The line of `_site.yml` that lists the chapter `rel`, as a `file:` or a bare list item.
pub(super) fn chapter_line(root: &Path, rel: &str) -> Option<usize> {
    let text = read_site_yml(root).ok()?;
    let src = ConfigSource(Some(&text));
    src.at_value(Some("file"), rel)
        .or_else(|| src.at_value(None, rel))
}

/// `_site.yml`'s text, a leading byte-order mark stripped: the one reader of the file
/// (the project load below, and `bibliography::shared_for_single_doc`). YAML takes a BOM
/// as the start of a second document, so the whole config used to be rejected with "more
/// than one document", and every setting silently defaulted.
pub(super) fn read_site_yml(root: &Path) -> std::io::Result<String> {
    let text = std::fs::read_to_string(root.join("_site.yml"))?;
    Ok(match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => text,
    })
}

/// Load + parse `_site.yml` at `root` into the native flat schema.
///
/// A directory with no `_site.yml` says nothing: it is a document's own folder (a lone
/// document is a project of one page), and every verb refuses a DIRECTORY with none before
/// it discovers anything. The advisory this used to push had no reader but four filters
/// that dropped it.
pub(in crate::site) fn load_config(root: &Path, warnings: &mut Vec<Warning>) -> SiteConfig {
    let Ok(text) = read_site_yml(root) else {
        // A directory still holding the pre-rename `_quarto.yml` is not a folder with no
        // config: it has one and every setting in it is being ignored, so the project
        // builds with its `title:` and everything else silently defaulted. Name the file
        // that is actually on disk.
        if root.join("_quarto.yml").is_file() {
            let mut w = Warning::new(format!(
                "found `_quarto.yml` at {}, but the project config is now `_site.yml`: \
                 rename it, or its settings go on being ignored",
                root.display()
            ));
            w.file = Some("_quarto.yml".to_string());
            warnings.push(w);
        }
        return SiteConfig::default();
    };
    let value: serde_yaml::Value = match serde_yaml::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            // Malformed YAML: degrade to defaults but tag the warning so the build can
            // fail on it and the preview can keep its last-good config.
            warnings.push(config_warning(
                e.location().map(|l| l.line()),
                Severity::Error,
                format!("{MALFORMED_CONFIG_PREFIX}: {e}"),
            ));
            return SiteConfig::default();
        }
    };
    parse_native(&value, warnings, ConfigSource(Some(&text)))
}

/// Whether a discovery warning is the malformed-`_site.yml` marker: nothing in the file
/// was read. Shared by the server's build (which fails on it with no `--strict`) and its
/// watcher (which keeps the last-good site).
pub fn is_malformed_config_warning(warning: &Warning) -> bool {
    warning.message.starts_with(MALFORMED_CONFIG_PREFIX)
}

/// `url:`, when set, must be an absolute origin with a scheme: it seeds every machine-read
/// absolute URL (sitemap `<loc>`, `robots.txt`/feed `Sitemap:`, `og:url`, llms.txt links).
/// A scheme-less `url: ex.com` builds clean and emits `<loc>ex.com/</loc>` +
/// `Sitemap: ex.com/sitemap.xml` — machine-invalid, under a green `check`. Warn (a
/// diagnostic, not a knob — the `page-layout` / site-`image:` precedent). A blank `url:` is
/// treated as unset by [`Site::canonical_base`], so it is left alone.
fn validate_url(value: &serde_yaml::Value, warnings: &mut Vec<Warning>, src: ConfigSource<'_>) {
    let Some(url) = value.get("url").and_then(|v| v.as_str()).map(str::trim) else {
        return;
    };
    if !url.is_empty() && !(url.starts_with("http://") || url.starts_with("https://")) {
        warnings.push(config_warning(
            src.at("url"),
            Severity::Warning,
            format!(
                "url: `{url}` has no scheme — sitemap, robots.txt, feed and og:url need an \
                 absolute URL (write `https://{url}`)"
            ),
        ));
    }
}

/// The keys of one `chapters:` entry in its mapping form (`{ file, text }`), or of a part
/// group (`{ part, chapters }`).
const CHAPTER_ITEM_KEYS: &[&str] = &["file", "text", "part", "chapters"];

/// Validate every entry of `chapters:`, at every nesting depth.
///
/// **This is the worst failure shape in the whole config surface, which is why it warns.**
/// `site::book::push_chapter_entry` consumes an entry only when it is a bare path string or
/// a mapping carrying `file:`. Anything else falls through to the part-group branch, which
/// builds a header from a missing `part:` (so: an empty title) and then pops it again
/// because it has no inner `chapters:`. A typo'd `fil: intro.tmd` therefore deletes the
/// chapter — no page built, no nav entry, no diagnostic, `check` exits 0.
///
/// Recurses into `{ part:, chapters: }` groups because `push_group` does, and a typo nested
/// one level down fails exactly the same way.
///
/// Two further shapes lose a chapter with every key spelled correctly, so neither the
/// unknown-key pass nor the no-`file:`-no-`part:` pass sees them:
/// - a `chapters:` value that is not a sequence. Both readers of a chapter list
///   (`parse_native` at the top level, `push_group` for a part's inner list) go through
///   `as_sequence()`, so a scalar there is not a one-item list, it is *no list*.
/// - one mapping carrying `file:` **and** `part:`/`chapters:`. `push_chapter_entry` matches
///   the `file:` first and returns `true`, so `push_group` never reads the rest of that
///   mapping. A forgotten `- ` before `part:` writes exactly this.
///
/// Every one of these is an error: each is a chapter the author wrote that the book will not
/// have.
fn validate_chapters(
    value: &serde_yaml::Value,
    warnings: &mut Vec<Warning>,
    src: ConfigSource<'_>,
) {
    let dropped =
        |line: Option<usize>, message: String| config_warning(line, Severity::Error, message);
    fn walk(
        list: &[serde_yaml::Value],
        warnings: &mut Vec<Warning>,
        src: ConfigSource<'_>,
        dropped: &dyn Fn(Option<usize>, String) -> Warning,
    ) {
        for item in list {
            // A bare path string is the common form and always well-formed.
            let Some(map) = item.as_mapping() else {
                continue;
            };
            for k in map.keys().filter_map(|k| k.as_str()) {
                if !CHAPTER_ITEM_KEYS.contains(&k) {
                    warnings.push(dropped(
                        src.at(k),
                        format!(
                            "unknown chapter key `{k}`{} — an entry taliesin cannot read as \
                             a chapter (`file:`) or a part (`part:`) is DROPPED from the book \
                             silently, so this is a missing chapter, not a cosmetic warning",
                            did_you_mean(k, CHAPTER_ITEM_KEYS)
                        ),
                    ));
                }
            }
            // A mapping that names neither a file nor a part is the silent-drop case even
            // when every key it does carry is spelled correctly (e.g. a lone `text:`).
            if !map.contains_key("file") && !map.contains_key("part") {
                let first = map.keys().filter_map(|k| k.as_str()).next().unwrap_or("");
                warnings.push(dropped(
                    src.at(first),
                    "a `chapters:` entry names no `file:` and no `part:`, so it is \
                     dropped from the book: give it a `file:`"
                        .to_string(),
                ));
            }
            // A chapter and a part in ONE mapping: `push_chapter_entry` consumes it on the
            // `file:` and `push_group` `continue`s, so the part keys beside it are never
            // read. Written by hand it is a missing `- ` before `part:`, which merges two
            // list entries into one mapping that every check above passes: all its keys
            // are legal and it does name a `file:`.
            if map.contains_key("file")
                && (map.contains_key("part") || map.contains_key("chapters"))
            {
                // Anchor on the key that vanished, not the `file:` that survived: that is
                // the line the missing `- ` belongs in front of.
                let lost = if map.contains_key("part") {
                    "part"
                } else {
                    "chapters"
                };
                warnings.push(dropped(
                    src.at(lost),
                    format!(
                        "a `chapters:` entry carries both `file:` and `{lost}:`, and the \
                         `file:` wins: the `{lost}:` and every chapter under it is DROPPED \
                         from the book silently, so split them into two list entries (a \
                         missing `- ` before `part:` merges them into this one)"
                    ),
                ));
                // The whole subtree is already reported gone; walking it would only add
                // diagnostics about chapters that are not built either way.
                continue;
            }
            if let Some(inner) = map.get("chapters") {
                match inner.as_sequence() {
                    Some(seq) => walk(seq, warnings, src, dropped),
                    // A part whose `chapters:` is empty loses nothing (and `push_group`
                    // pops the now-empty header), so only a *value* that is not a list
                    // reports: that one is a chapter the author wrote and will not get.
                    None if inner.is_null() => {}
                    None => warnings.push(dropped(
                        // The entry's own `part:` if it has one: the message is about a
                        // part, so a line showing one reads better than the enclosing
                        // `chapters:` key.
                        src.at(if map.contains_key("part") {
                            "part"
                        } else {
                            "chapters"
                        }),
                        "a part's `chapters:` is not a list, so the part reads as empty: \
                         every chapter under it is DROPPED from the book, and the emptied \
                         part header goes with it (write the entries as a list, \
                         `- intro.tmd`)"
                            .to_string(),
                    )),
                }
            }
        }
    }
    if let Some(chapters) = value.get("chapters") {
        match chapters.as_sequence() {
            Some(list) => walk(list, warnings, src, &dropped),
            // `chapters: []` / a bare `chapters:` is a book with no chapters yet, which is
            // what an author writing one starts from. Nothing is lost, so nothing reports.
            None if chapters.is_null() => {}
            None => warnings.push(dropped(
                src.at("chapters"),
                "`chapters:` is not a list, so it names no chapters at all: every \
                 chapter under it is DROPPED and the project builds as a plain website, \
                 not a book (write the entries as a list, `- intro.tmd`)"
                    .to_string(),
            )),
        }
    }
}

fn parse_native(
    value: &serde_yaml::Value,
    warnings: &mut Vec<Warning>,
    src: ConfigSource<'_>,
) -> SiteConfig {
    validate_keys(value, warnings, src);
    validate_url(value, warnings, src);
    validate_chapters(value, warnings, src);
    // Through `scalar`, like a page's front matter: a number or bool where text is expected
    // (`title: 2026`) is read as its text rather than dropped.
    let str_of = |k: &str| scalar(value.get(k));
    let chapters = value
        .get("chapters")
        .and_then(|v| v.as_sequence())
        .cloned()
        .unwrap_or_default();
    SiteConfig {
        is_book: !chapters.is_empty(),
        title: str_of("title"),
        authors: crate::author::parse(value.get("author")).0,
        description: str_of("description"),
        url: str_of("url"),
        favicon: str_of("favicon"),
        logo: str_of("logo"),
        nav: nav_from(value.get("nav")),
        footer: footer_from(value.get("footer")),
        chapters,
        python: str_of("python"),
        bibliography: crate::site::frontmatter::string_list(value.get("bibliography")),
    }
}

/// A ` (did you mean `x`?)` suffix for a near-miss key, else empty.
fn did_you_mean(key: &str, candidates: &[&'static str]) -> String {
    crate::frontmatter::closest(key, candidates)
        .map(|s| format!(" (did you mean `{s}`?)"))
        .unwrap_or_default()
}

/// Where the config's own diagnostics point. `serde_yaml`'s `Value` has dropped all
/// spans by the time the schema is validated, so the line is recovered from the raw
/// text by finding where the offending key is written.
#[derive(Clone, Copy, Default)]
pub(super) struct ConfigSource<'a>(pub Option<&'a str>);

impl ConfigSource<'_> {
    /// The line of `_site.yml` a diagnostic about `key` points at (so the editor can jump
    /// to it), or `None` when the key cannot be located — a warning without a line still
    /// beats a wrong one.
    fn at(&self, key: &str) -> Option<usize> {
        self.0.and_then(|t| key_line(t, key))
    }

    /// Like [`Self::at`], for the line where `key` holds `value` (`None`: a bare list item
    /// `- value`). Nav and footer items share their keys, so the first `text:` in the file
    /// is usually some other item's: the line that also carries the value is the one meant.
    fn at_value(&self, key: Option<&str>, value: &str) -> Option<usize> {
        self.0
            .and_then(|t| crate::frontmatter::value_line(t, key, value))
    }
}

/// The 1-based line where `key` is written in `_site.yml`, at any nesting depth (a list
/// item's `- key:` counts), or `None` when it is written on more than one line. A key that
/// repeats at one level is a YAML error the parse step already reports, but one that
/// repeats across entries (two `part:`s, a `chapters:` in a part) has no one line to name:
/// the first match is usually another entry's, and pointing there sent the author three
/// lines above the entry that lost its chapters (leads config/mod.rs:329).
pub(crate) fn key_line(text: &str, key: &str) -> Option<usize> {
    let mut lines = text.lines().enumerate().filter(|(_, l)| {
        let t = l.trim_start().trim_start_matches("- ").trim_start();
        // Also look inside a flow mapping: `- { file: a.tmd, text: A }` is how chapter
        // and nav entries are usually written, and a diagnostic about one of those keys
        // is worth a line number.
        let t = t.strip_prefix('{').map_or(t, str::trim_start);
        // Match the key token exactly, not a prefix: `nav:` must not match `navigation:`.
        t.strip_prefix(key)
            .is_some_and(|rest| rest.starts_with(':'))
    });
    match (lines.next(), lines.next()) {
        (Some((i, _)), None) => Some(i + 1),
        _ => None,
    }
}

/// Warn on unrecognized keys against the closed native schema: top-level, and the
/// nested `nav:`/`footer:` structures (a typo in one of those silently drops
/// the whole section/item, so it warns with a "did you mean"). Every
/// warning is located in `_site.yml` rather than anonymous.
fn validate_keys(value: &serde_yaml::Value, warnings: &mut Vec<Warning>, src: ConfigSource<'_>) {
    let Some(map) = value.as_mapping() else {
        // An empty file is an empty config; anything else names no key at all.
        if !value.is_null() {
            warnings.push(config_warning(
                None,
                Severity::Error,
                "its top level is not a mapping of `key: value` settings, so every setting \
                 in it is ignored"
                    .to_string(),
            ));
        }
        return;
    };
    let warn = |warnings: &mut Vec<Warning>, what: &str, key: &str, allowed: &[&'static str]| {
        // Through `unknown_key_message` so the config speaks the same sentence the
        // front-matter validator does, `what` and all: one wording for one kind of
        // mistake, in whichever vocabulary the author was writing.
        warnings.push(config_warning(
            src.at(key),
            Severity::Warning,
            crate::frontmatter::unknown_key_message(what, key, allowed),
        ));
    };
    for (k, v) in map {
        let Some(key) = k.as_str() else { continue };
        if !NATIVE_KEYS.contains(&key) {
            warn(warnings, "config key", key, NATIVE_KEYS);
            continue;
        }
        match key {
            "nav" => validate_nav_like(v, NAV_SECTION_KEYS, "nav", warnings, src),
            "footer" => validate_nav_like(v, FOOTER_SECTION_KEYS, "footer", warnings, src),
            _ => {}
        }
    }
}

/// Validate a `nav:`/`footer:` value: a `{ left/right/center }` mapping (section keys
/// checked, then each section's items), a bare list of items, or a string label
/// (nothing to check).
fn validate_nav_like(
    v: &serde_yaml::Value,
    section_keys: &[&'static str],
    ctx: &str,
    warnings: &mut Vec<Warning>,
    src: ConfigSource<'_>,
) {
    match v {
        serde_yaml::Value::Mapping(m) => {
            for (k, section) in m {
                let Some(key) = k.as_str() else { continue };
                if section_keys.contains(&key) {
                    validate_items(section, ctx, warnings, src);
                } else {
                    warnings.push(config_warning(
                        src.at(key),
                        Severity::Warning,
                        format!(
                            "unknown {ctx} section `{key}`{}",
                            did_you_mean(key, section_keys)
                        ),
                    ));
                }
            }
        }
        _ => validate_items(v, ctx, warnings, src),
    }
}

/// Validate one or a list of nav/footer items: each mapping's keys against
/// [`NAV_ITEM_KEYS`], its `icon:` against the bundled glyphs, and, in `nav:`, that it has an
/// `href:`. The navbar renders only a link, so a nav item with no `href:` (a bare
/// `- a.tmd` entry included) is dropped from it; a footer item with none is a plain text
/// label (a copyright line), which is why that rule is the navbar's alone.
fn validate_items(
    v: &serde_yaml::Value,
    ctx: &str,
    warnings: &mut Vec<Warning>,
    src: ConfigSource<'_>,
) {
    let items: Vec<&serde_yaml::Value> = match v {
        serde_yaml::Value::Sequence(seq) => seq.iter().collect(),
        other => vec![other],
    };
    for item in items {
        match item {
            serde_yaml::Value::Mapping(m) => {
                for k in m.keys().filter_map(|k| k.as_str()) {
                    if !NAV_ITEM_KEYS.contains(&k) {
                        warnings.push(config_warning(
                            src.at(k),
                            Severity::Warning,
                            format!(
                                "unknown {ctx} item key `{k}`{}",
                                did_you_mean(k, NAV_ITEM_KEYS)
                            ),
                        ));
                    }
                }
                let icon = scalar(m.get("icon"));
                if let Some(name) = icon.as_deref()
                    && super::chrome::social_icon(name).is_none()
                {
                    warnings.push(config_warning(
                        src.at_value(Some("icon"), name),
                        Severity::Warning,
                        format!(
                            "unknown {ctx} icon `{name}`: no bundled icon has that name, so \
                             the link shows its text or URL instead"
                        ),
                    ));
                }
                if ctx == "nav" && !m.contains_key("href") {
                    let (key, name) = match (scalar(m.get("text")), icon) {
                        (Some(text), _) => ("text", text),
                        (None, Some(icon)) => ("icon", icon),
                        (None, None) => ("", String::new()),
                    };
                    warnings.push(config_warning(
                        src.at_value(Some(key), &name),
                        Severity::Warning,
                        format!(
                            "a `nav:` item (`{name}`) has no `href:`, so it is dropped from \
                             the navbar"
                        ),
                    ));
                }
            }
            scalar_item if ctx == "nav" => {
                if let Some(name) = scalar(Some(scalar_item)) {
                    warnings.push(config_warning(
                        src.at_value(None, &name),
                        Severity::Warning,
                        format!(
                            "a bare `nav:` entry (`{name}`) has no `href:`, so it is dropped \
                             from the navbar: write it as `{{ text: …, href: {name} }}`"
                        ),
                    ));
                }
            }
            _ => {}
        }
    }
}

/// `nav:` is a list of items (the left side) or `{ left: […], right: […] }`.
fn nav_from(v: Option<&serde_yaml::Value>) -> Navbar {
    match v {
        Some(v) if v.is_mapping() => Navbar {
            left: items(v.get("left")),
            right: items(v.get("right")),
        },
        Some(v) => Navbar {
            left: items(Some(v)),
            right: Vec::new(),
        },
        None => Navbar::default(),
    }
}

/// `footer:` is a string (a single left label) or `{ left/center/right }`, each of
/// which is a string, a single item, or a list.
fn footer_from(v: Option<&serde_yaml::Value>) -> Option<Footer> {
    match v {
        Some(v) if v.is_mapping() => Some(Footer {
            left: items(v.get("left")),
            center: items(v.get("center")),
            right: items(v.get("right")),
        }),
        Some(v) => Some(Footer {
            left: items(Some(v)),
            ..Footer::default()
        }),
        None => None,
    }
}

/// Coerce a value into a list of [`NavItem`]: a scalar → one text item, a single
/// `{…}` → one item, a list → many.
fn items(v: Option<&serde_yaml::Value>) -> Vec<NavItem> {
    match v {
        None => Vec::new(),
        Some(serde_yaml::Value::Sequence(seq)) => seq.iter().filter_map(nav_item).collect(),
        Some(v) => nav_item(v).into_iter().collect(),
    }
}

/// One nav/footer entry from a YAML value: a bare scalar becomes a text label; a `{…}`
/// mapping's fields are read through `scalar`, like every other config value. It used to
/// deserialize into [`NavItem`]'s `Option<String>`s and discard the error, so a number
/// anywhere in the item (`text: 2025`) made the whole item vanish.
fn nav_item(v: &serde_yaml::Value) -> Option<NavItem> {
    match v {
        serde_yaml::Value::Mapping(_) => Some(NavItem {
            text: scalar(v.get("text")),
            href: scalar(v.get("href")),
            icon: scalar(v.get("icon")),
        }),
        other => scalar(Some(other)).map(|text| NavItem {
            text: Some(text),
            ..NavItem::default()
        }),
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    /// A config warning as `file:line: message`, the form a terminal prints it in.
    fn located(w: &Warning) -> String {
        let file = w.file.as_deref().unwrap_or("");
        match w.line {
            Some(l) => format!("{file}:{l}: {}", w.message),
            None => format!("{file}: {}", w.message),
        }
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("tali-cfg-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn parses_the_python_interpreter_pin() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("title: X\npython: .venv/bin/python\n").unwrap();
        let cfg = parse_native(&v, &mut w, ConfigSource(None));
        let w: Vec<String> = w.iter().map(located).collect();
        assert_eq!(cfg.python.as_deref(), Some(".venv/bin/python"));
        assert!(w.is_empty(), "valid keys warn about nothing: {w:?}");
    }

    /// The raw-injection family is gone ENTIRELY, `head:` included (2026-08-18). Dropping
    /// a key from the known set only makes it diagnosed; this pins that the read is gone
    /// too, so a project that still carries the key is told so and gets no injection.
    #[test]
    fn head_is_no_longer_read_and_is_diagnosed_as_unknown() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("title: X\nhead: |\n  <meta name=\"x\" content=\"y\">\n").unwrap();
        let cfg = parse_native(&v, &mut w, ConfigSource(None));
        let w: Vec<String> = w.iter().map(located).collect();
        assert_eq!(
            cfg.title.as_deref(),
            Some("X"),
            "the rest of the config still parses"
        );
        assert!(
            w.iter().any(|d| d.contains("head")),
            "`head:` must draw the unknown-key diagnostic: {w:?}"
        );
    }

    /// `external-prefixes:` was the gallery composition's one config key (cut 2026-08-19
    /// when the gallery became self-contained). The read is gone, not just the docs: a
    /// `_site.yml` still carrying it draws the unknown-key diagnostic, which is all this
    /// test asserts. Links into a formerly external prefix are broken like any other now,
    /// structurally, since the field no longer exists to declare one.
    #[test]
    fn external_prefixes_is_no_longer_read_and_is_diagnosed_as_unknown() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("title: X\nexternal-prefixes:\n  - tarn\n").unwrap();
        let cfg = parse_native(&v, &mut w, ConfigSource(None));
        let w: Vec<String> = w.iter().map(located).collect();
        assert_eq!(
            cfg.title.as_deref(),
            Some("X"),
            "the rest of the config still parses"
        );
        assert!(
            w.iter().any(|d| d.contains("external-prefixes")),
            "`external-prefixes:` must draw the unknown-key diagnostic: {w:?}"
        );
    }

    /// The silent-chapter-drop fix. `site::book::push_chapter_entry` consumes an entry only
    /// when it is a string or carries `file:`; anything else becomes an empty part header
    /// that `push_group` pops again, so the chapter vanishes with `check` exiting 0. Each
    /// case below produced ZERO diagnostics before 2026-08-02.
    #[test]
    fn a_chapter_entry_that_would_be_dropped_is_diagnosed() {
        for (yaml, needle) in [
            // A typo'd `file:`.
            (
                "title: X\nchapters:\n  - { fil: intro.tmd }\n",
                "unknown chapter key `fil`",
            ),
            // Correctly spelled keys that still name no chapter.
            (
                "title: X\nchapters:\n  - { text: Intro }\n",
                "names no `file:` and no `part:`",
            ),
            // Nested one level down, inside a part group — `push_group` recurses, so this
            // fails identically and must be caught identically.
            (
                "title: X\nchapters:\n  - part: One\n    chapters:\n      - { fil: a.tmd }\n",
                "unknown chapter key `fil`",
            ),
        ] {
            let mut w = Vec::new();
            let v: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
            parse_native(&v, &mut w, ConfigSource(None));
            let w: Vec<String> = w.iter().map(located).collect();
            assert!(
                w.iter().any(|m| m.contains(needle)),
                "expected {needle:?} for:\n{yaml}\ngot: {w:?}"
            );
            assert!(
                w.iter()
                    .any(|m| m.contains("DROPPED") || m.contains("dropped")),
                "the diagnostic must say the chapter is lost, not just that a key is odd: {w:?}"
            );
        }
    }

    /// A `chapters:` value that is not a list reads as NO chapters, at either depth, and
    /// both depths reach it through the same `as_sequence()`. Top level: `parse_native`
    /// gets `None`, so `is_book` is false and the project builds as a plain website.
    /// Inside a part: `book::push_group` gets `None`, so it never recurses, and the
    /// empty-part-header pop then deletes the part header too. Both shapes produced ZERO
    /// diagnostics, with `check` exiting 0 over a book missing a chapter.
    #[test]
    fn a_chapters_value_that_is_not_a_list_is_diagnosed() {
        for yaml in [
            // Top level: the project silently stops being a book.
            "title: X\nchapters: deep.tmd\n",
            // Inside a part group: the part and the chapter under it both vanish.
            "title: X\nchapters:\n  - index.tmd\n  - part: Basics\n    chapters: deep.tmd\n",
        ] {
            let mut w = Vec::new();
            let v: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
            parse_native(&v, &mut w, ConfigSource(None));
            let w: Vec<String> = w.iter().map(located).collect();
            assert!(
                w.iter().any(|m| m.contains("`chapters:` is not a list")),
                "expected a not-a-list diagnostic for:\n{yaml}\ngot: {w:?}"
            );
            assert!(
                w.iter().any(|m| m.contains("DROPPED")),
                "the diagnostic must say the chapter is lost, not just that a shape is odd: {w:?}"
            );
        }
        // An empty list and a bare `chapters:` name no chapters at all, so nothing is lost:
        // the diagnostic is for chapters that VANISH, not for a book with none yet.
        for yaml in ["title: X\nchapters: []\n", "title: X\nchapters:\n"] {
            let mut w = Vec::new();
            let v: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
            parse_native(&v, &mut w, ConfigSource(None));
            let w: Vec<String> = w.iter().map(located).collect();
            assert!(
                !w.iter().any(|m| m.contains("not a list")),
                "an empty chapter list loses nothing and must stay silent ({yaml:?}): {w:?}"
            );
        }
    }

    /// One entry carrying `file:` alongside a part's keys. A missing `- ` before `part:`
    /// merges two list entries into a single mapping whose keys are all spelled correctly,
    /// so every other check passes it: `push_chapter_entry` finds the `file:`, consumes the
    /// entry and returns `true`, and `push_group` therefore never looks at the `part:` or
    /// the `chapters:` sitting in the same mapping. The part and every chapter under it are
    /// gone, with `check` exiting 0.
    #[test]
    fn an_entry_mixing_file_with_part_or_chapters_is_diagnosed() {
        for yaml in [
            // The missing `- ` before `part:`.
            "title: X\nchapters:\n  - file: index.tmd\n    part: Basics\n    chapters:\n      - a.tmd\n",
            // The same collision without a `part:`: `file:` still wins and the nested list goes.
            "title: X\nchapters:\n  - file: index.tmd\n    chapters:\n      - a.tmd\n",
        ] {
            let mut w = Vec::new();
            let v: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
            parse_native(&v, &mut w, ConfigSource(None));
            let w: Vec<String> = w.iter().map(located).collect();
            assert!(
                w.iter().any(|m| m.contains("both `file:`")),
                "expected a file/part collision diagnostic for:\n{yaml}\ngot: {w:?}"
            );
            assert!(
                w.iter().any(|m| m.contains("DROPPED")),
                "the diagnostic must say the part is lost, not just that the entry is odd: {w:?}"
            );
        }
    }

    /// Both well-formed chapter shapes, and a part group, stay silent.
    #[test]
    fn well_formed_chapters_warn_about_nothing() {
        let mut w = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str(
            "title: X\nchapters:\n  - intro.tmd\n  - { file: two.tmd, text: Second }\n  \
             - part: Three\n    chapters:\n      - { file: a.tmd }\n",
        )
        .unwrap();
        parse_native(&v, &mut w, ConfigSource(None));
        let w: Vec<String> = w.iter().map(located).collect();
        assert!(w.is_empty(), "valid chapters warn about nothing: {w:?}");
    }

    #[test]
    fn parses_a_site_level_bibliography_in_both_shapes() {
        // A shared `.bib` declared once in `_site.yml` instead of retyped in every post's
        // front matter. Accepts the same two shapes a page's `bibliography:` does.
        for (yaml, want) in [
            ("title: X\nbibliography: refs.bib\n", vec!["refs.bib"]),
            (
                "title: X\nbibliography: [a.bib, b.bib]\n",
                vec!["a.bib", "b.bib"],
            ),
            (
                "title: X\nbibliography:\n  - a.bib\n  - b.bib\n",
                vec!["a.bib", "b.bib"],
            ),
        ] {
            let mut w = Vec::new();
            let v: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
            let cfg = parse_native(&v, &mut w, ConfigSource(None));
            let w: Vec<String> = w.iter().map(located).collect();
            assert_eq!(cfg.bibliography, want, "shape {yaml:?}");
            assert!(
                w.iter().all(|m| !m.contains("config key")),
                "bibliography is a recognized _site.yml key: {w:?}"
            );
        }
        let mut w = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str("title: X\n").unwrap();
        assert!(
            parse_native(&v, &mut w, ConfigSource(None))
                .bibliography
                .is_empty(),
            "an absent bibliography: is an empty list"
        );
    }

    #[test]
    fn a_site_level_image_is_not_a_config_key_and_says_so() {
        // D34's subtraction (owner ruling 2026-07-17). Site-level `image:` used to seed
        // og:image/twitter:image for pages that set none; the auto-generated per-page card
        // took that over entirely, and the key was left parsed into a field with ZERO
        // readers whose own doc comment conceded it did nothing. The marketing site's own
        // `_site.yml` still carried `image: assets/og-card.png` with the trailing comment
        // "default social card (og:image / twitter:image) for every page" -- a line that
        // claimed a job it had already lost.
        //
        // Deleting the field alone would have been the worse half of the fix: the key
        // would stay in NATIVE_KEYS, still parse clean, and still read as honored. That is
        // exactly the shape D37 and the `csl:` precedent call the bug. So the key leaves
        // the set too, which is what makes the silence audible.
        let mut w = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str("image: assets/og-card.png\n").unwrap();
        let _ = parse_native(&v, &mut w, ConfigSource(None));
        let w: Vec<String> = w.iter().map(located).collect();
        assert!(
            w.iter().any(|m| m.contains("image")),
            "a site-level `image:` must be diagnosed, not silently ignored: {w:?}"
        );
    }

    #[test]
    fn an_unknown_key_diagnostic_carries_its_line_number() {
        // A `_site.yml` diagnostic used to be an anonymous string ("_site.yml: unknown
        // config key `pythn`"), so the editor could not jump to it and a long config left
        // the author hunting. Locate it in the same `file:line:` form the page-level
        // warnings use.
        let text = "title: X\ntoc: true\npythn: python3\n";
        let mut w = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str(text).unwrap();
        let _ = parse_native(&v, &mut w, ConfigSource(Some(text)));
        let w: Vec<String> = w.iter().map(located).collect();
        assert!(
            w.iter()
                .any(|m| m.starts_with("_site.yml:3:") && m.contains("pythn")),
            "the unknown-key warning must name line 3: {w:?}"
        );
    }

    #[test]
    fn key_line_matches_the_whole_key_not_a_prefix() {
        // `nav:` must not match `navigation:` — a prefix match would point the author at
        // an unrelated line, which is worse than no line at all.
        let text = "navigation: x\nnav:\n  - a\n";
        assert_eq!(key_line(text, "nav"), Some(2));
        assert_eq!(key_line(text, "navigation"), Some(1));
        assert_eq!(key_line(text, "missing"), None);
        // A list-item key is found too (`- text:` inside `nav:`).
        assert_eq!(key_line("nav:\n  - text: Blog\n", "text"), Some(2));
    }

    /// A key written more than once has no one line to name: the first occurrence is
    /// usually another entry's. Two `part:` entries put a merged `file:`+`part:` warning at
    /// the FIRST part, three lines above the one that lost its chapters (leads
    /// config/mod.rs:329). A warning without a line still beats a wrong one.
    #[test]
    fn a_key_written_twice_has_no_line() {
        let text =
            "chapters:\n  - part: One\n    chapters: [a.tmd]\n  - file: b.tmd\n    part: Two\n";
        assert_eq!(key_line(text, "file"), Some(4), "written once: located");
        assert_eq!(key_line(text, "part"), None, "written twice: no line");
    }

    #[test]
    fn a_scheme_less_url_is_diagnosed_not_silently_shipped() {
        // `url: ex.com` (no scheme) builds clean and emits `<loc>ex.com/</loc>` +
        // `Sitemap: ex.com/sitemap.xml` + `og:url` — machine-invalid absolute URLs, under a
        // green `check`. A scheme is required; warn (a diagnostic, not a knob — the
        // `page-layout`/site-`image:` precedent).
        let mut w = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str("title: X\nurl: ex.com\n").unwrap();
        let _ = parse_native(&v, &mut w, ConfigSource(None));
        let w: Vec<String> = w.iter().map(located).collect();
        assert!(
            w.iter().any(|m| m.contains("url") && m.contains("scheme")),
            "a scheme-less url: must be diagnosed: {w:?}"
        );
    }

    #[test]
    fn a_url_with_a_scheme_or_blank_does_not_warn() {
        // http/https are accepted silently; a blank url: is treated as unset (canonical_base
        // filters it), so it must not warn either.
        for url in ["https://ex.com", "http://localhost:8080", ""] {
            let mut w = Vec::new();
            let v: serde_yaml::Value =
                serde_yaml::from_str(&format!("title: X\nurl: \"{url}\"\n")).unwrap();
            let _ = parse_native(&v, &mut w, ConfigSource(None));
            let w: Vec<String> = w.iter().map(located).collect();
            assert!(
                !w.iter().any(|m| m.contains("scheme")),
                "a scheme'd or blank url must not warn ({url:?}): {w:?}"
            );
        }
    }

    /// The book-vs-website `toc:` diagnostic is gone with the key itself: `_site.yml toc:` was
    /// retired on 2026-08-02 and the sidebar TOC is decided per page by `Site::page_toc`, which
    /// already returns `false` for a book unconditionally. A book that still carries the key is
    /// answered as an unknown config key, not by a special book-scope rule.
    #[test]
    fn a_book_carrying_the_toc_key_gets_no_special_book_scope_rule() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("toc: true\nchapters:\n  - a.tmd\n").unwrap();
        parse_native(&v, &mut w, ConfigSource(None));
        let w: Vec<String> = w.iter().map(located).collect();
        let msg = w
            .iter()
            .find(|m| m.contains("`toc`"))
            .unwrap_or_else(|| panic!("no diagnostic: {w:?}"));
        assert!(
            !msg.contains("has no effect in a book"),
            "the old book-scope wording must not survive the key: {msg}"
        );
    }

    #[test]
    fn a_typod_interpreter_key_warns_via_native_keys() {
        let mut w = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str("pyton: .venv/bin/python\n").unwrap();
        let _ = parse_native(&v, &mut w, ConfigSource(None));
        let w: Vec<String> = w.iter().map(located).collect();
        assert!(
            w.iter().any(|m| m.contains("pyton")),
            "an unknown config key must warn (did-you-mean python): {w:?}"
        );
    }

    #[test]
    fn malformed_site_yml_pushes_tagged_warning_distinct_from_missing() {
        // A malformed `_site.yml` is a real error: it degrades to defaults AND tags its
        // warning so the build/preview can treat it differently from a missing file.
        let dir = tmp("malformed");
        // Unterminated double-quoted scalar -> serde_yaml parse error.
        std::fs::write(dir.join("_site.yml"), "title: \"unterminated\nfoo: bar\n").unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        assert!(cfg.title.is_none(), "malformed config degrades to default");
        assert!(
            warnings.iter().any(is_malformed_config_warning),
            "malformed YAML must be tagged: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A folder with no `_site.yml` is a lone document's own folder, and says nothing about
    /// it: every verb refuses a DIRECTORY with none before it discovers anything, so the
    /// advisory this used to push had no reader, only four filters that dropped it.
    #[test]
    fn a_folder_without_site_yml_reports_nothing() {
        let dir = tmp("missing");
        let mut warnings = Vec::new();
        let _ = load_config(&dir, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pre_rename_quarto_yml_is_named_rather_than_silently_ignored() {
        // The config file was renamed to `_site.yml` on 2026-06-24. A project still
        // carrying the old name is not read at all: it builds with its configuration
        // silently defaulted, dropping its `title:`. It stayed invisible because the only
        // signal was the *missing* advisory, which `check` deliberately discards from its
        // tally (a bare directory of pages is legitimate). Having the old file on disk is
        // a different situation from having no config at all, so it says so.
        let dir = tmp("quarto-legacy");
        std::fs::write(
            dir.join("_quarto.yml"),
            "project:\n  type: book\ntitle: Old title\n",
        )
        .unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        assert!(
            cfg.title.is_none(),
            "the retired file is reported, never read: {cfg:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("_quarto.yml") && w.message.contains("_site.yml")),
            "name the file that is there AND the name it needs: {warnings:?}"
        );
        assert!(
            !warnings.iter().any(is_malformed_config_warning),
            "it is not malformed YAML: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn cfg_warnings(yml: &str) -> Vec<String> {
        // A unique dir per call: several tests hit `cfg_warnings` and run in parallel,
        // so a shared dir name would let one test's cleanup nuke another's `_site.yml`.
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = tmp(&format!("warn-{}", N.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(dir.join("_site.yml"), yml).unwrap();
        let mut warnings = Vec::new();
        let _ = load_config(&dir, &mut warnings);
        let _ = std::fs::remove_dir_all(&dir);
        warnings.iter().map(located).collect()
    }

    #[test]
    fn unknown_top_level_key_is_located_at_site_yml() {
        // The warning must name `_site.yml` (it was previously anonymous) and suggest
        // the near-miss.
        let w = cfg_warnings("titel: My Site\n");
        assert!(
            w.iter().any(|w| w.starts_with("_site.yml:")
                && w.contains("`titel`")
                && w.contains("`title`")),
            "{w:?}"
        );
    }

    #[test]
    fn nested_nav_footer_typos_warn_instead_of_silently_dropping() {
        // A `nav:` section typo drops the whole side silently — must warn.
        let w = cfg_warnings("nav:\n  lefft:\n    - text: Blog\n      href: blog.tmd\n");
        assert!(
            w.iter()
                .any(|w| w.contains("nav section `lefft`") && w.contains("`left`")),
            "nav section typo: {w:?}"
        );

        // A nav ITEM key typo drops the label/link silently.
        let w = cfg_warnings("nav:\n  left:\n    - txt: Blog\n      href: blog.tmd\n");
        assert!(
            w.iter()
                .any(|w| w.contains("nav item key `txt`") && w.contains("`text`")),
            "nav item typo: {w:?}"
        );

        // A `footer:` center is valid; a bogus footer section warns.
        let w = cfg_warnings("footer:\n  centre:\n    - text: hi\n");
        assert!(
            w.iter()
                .any(|w| w.contains("footer section `centre`") && w.contains("`center`")),
            "footer section typo: {w:?}"
        );
    }

    #[test]
    fn valid_nested_nav_footer_have_no_warnings() {
        // The real corpus shape: `{ left: [...], right: [...] }` with text/href items and
        // a footer with left/center/right. Neither may warn.
        let w = cfg_warnings(concat!(
            "title: Site\n",
            "nav:\n  left:\n    - text: Blog\n      href: blog.tmd\n  right:\n    - icon: github\n      href: 'https://x'\n",
            "footer:\n  left:\n    - text: © 2026\n  center:\n    - text: mid\n  right:\n    - text: end\n",
        ));
        assert!(w.iter().all(|w| !w.contains("unknown")), "{w:?}");
    }

    #[test]
    fn valid_site_yml_has_no_config_warnings() {
        let dir = tmp("valid");
        std::fs::write(dir.join("_site.yml"), "title: My Site\n").unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        assert_eq!(cfg.title.as_deref(), Some("My Site"));
        assert!(
            !warnings.iter().any(is_malformed_config_warning),
            "a valid config is not malformed: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A YAML number or bool where the config expects text has an obvious text form, so it
    /// is read as that text. Every scalar went through `as_str()` (or a `Deserialize` into
    /// `Option<String>` whose error `.ok()` discarded), so `title: 2026` built a site with no
    /// title, `python: 3.12` was ignored while `doctor` called the config valid, a nav item
    /// `{ text: 2025, href: … }` vanished whole, and `footer: left: 2026` rendered nothing,
    /// all with zero diagnostics.
    #[test]
    fn a_number_where_text_is_expected_is_read_as_its_text() {
        let mut w = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str(
            "title: 2026\npython: 3.12\nnav:\n  - { text: 2025, href: y2025.tmd }\n  \
             - { text: On, href: on.tmd, icon: true }\nfooter:\n  left: 2026\n",
        )
        .unwrap();
        let cfg = parse_native(&v, &mut w, ConfigSource(None));
        assert_eq!(cfg.title.as_deref(), Some("2026"));
        assert_eq!(cfg.python.as_deref(), Some("3.12"));
        let nav = &cfg.nav.left;
        assert_eq!(nav.len(), 2, "no nav item vanishes: {nav:?}");
        assert_eq!(nav[0].text.as_deref(), Some("2025"));
        assert_eq!(nav[0].href.as_deref(), Some("y2025.tmd"));
        assert_eq!(nav[1].icon.as_deref(), Some("true"));
        let footer = cfg.footer.expect("a footer");
        assert_eq!(footer.left.len(), 1);
        assert_eq!(footer.left[0].text.as_deref(), Some("2026"));
    }

    /// A `nav:` entry with no `href:` links nowhere, so the navbar drops it; that drop was
    /// silent, including for a bare `- a.tmd` written the way `chapters:` entries are. A
    /// footer entry with no `href:` is a plain text item (a copyright line) and stays silent.
    #[test]
    fn a_nav_entry_without_an_href_is_diagnosed() {
        let text = "nav:\n  - { text: Home, href: index.tmd }\n  - a.tmd\n  - { text: Blog }\n\
                    footer:\n  left:\n    - text: (c) 2026\n";
        let mut w = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str(text).unwrap();
        let _ = parse_native(&v, &mut w, ConfigSource(Some(text)));
        let w: Vec<String> = w.iter().map(located).collect();
        let dropped: Vec<&String> = w.iter().filter(|m| m.contains("no `href:`")).collect();
        assert_eq!(dropped.len(), 2, "{w:?}");
        assert!(
            dropped[0].starts_with("_site.yml:3:") && dropped[0].contains("`a.tmd`"),
            "{w:?}"
        );
        assert!(
            dropped[1].starts_with("_site.yml:4:") && dropped[1].contains("`Blog`"),
            "{w:?}"
        );
    }

    /// An `icon:` name with no bundled glyph rendered the link's raw URL as its text, in
    /// silence.
    #[test]
    fn an_unknown_icon_name_is_diagnosed() {
        let text = "nav:\n  - { icon: github, href: \"https://a\" }\n  - { icon: githb, href: \"https://b\" }\n";
        let mut w = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str(text).unwrap();
        let _ = parse_native(&v, &mut w, ConfigSource(Some(text)));
        let w: Vec<String> = w.iter().map(located).collect();
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(
            w[0].starts_with("_site.yml:3:") && w[0].contains("unknown nav icon `githb`"),
            "{w:?}"
        );
    }

    /// A UTF-8 byte-order mark (which some Windows editors write) rejected the whole config
    /// with "more than one document", and the project built with every setting defaulted.
    /// Front matter already stripped one; both `_site.yml` readers now do.
    #[test]
    fn a_byte_order_mark_does_not_reject_the_config() {
        let dir = tmp("bom");
        std::fs::write(
            dir.join("_site.yml"),
            "\u{feff}title: With BOM\nbibliography: refs.bib\n",
        )
        .unwrap();
        std::fs::write(dir.join("refs.bib"), "@misc{k, title={T}}\n").unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        assert_eq!(cfg.title.as_deref(), Some("With BOM"), "{warnings:?}");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            crate::site::shared_for_single_doc(&dir).len(),
            1,
            "the single-document reader of `_site.yml` strips it too"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A `_site.yml` whose top level is a list or a plain value parses, but names no key, so
    /// every setting degraded to its default in silence.
    #[test]
    fn a_config_that_is_not_a_mapping_is_diagnosed() {
        for text in ["- title: X\n- nav: []\n", "just a title\n"] {
            let mut w = Vec::new();
            let v: serde_yaml::Value = serde_yaml::from_str(text).unwrap();
            let _ = parse_native(&v, &mut w, ConfigSource(Some(text)));
            let w: Vec<String> = w.iter().map(located).collect();
            assert!(
                w.iter().any(|m| m.contains("not a mapping")),
                "{text:?}: {w:?}"
            );
        }
        // An empty file is an empty config, not a mistake.
        let mut w = Vec::new();
        let _ = parse_native(&serde_yaml::Value::Null, &mut w, ConfigSource(Some("")));
        let w: Vec<String> = w.iter().map(located).collect();
        assert!(w.is_empty(), "{w:?}");
    }

    /// A book's `part:` label and a chapter's `text:` label are text too.
    #[test]
    fn a_numeric_part_or_chapter_label_is_read_as_its_text() {
        let root = crate::site::tests::write_site(
            "numericpart",
            &[
                (
                    "_site.yml",
                    "title: B\nchapters:\n  - index.tmd\n  - part: 2026\n    chapters:\n      \
                     - { file: a.tmd, text: 1999 }\n",
                ),
                ("index.tmd", "# Home\n"),
                ("a.tmd", "# A\n"),
            ],
        );
        let site = crate::site::Site::discover(&root);
        let entries = &site.book.as_ref().unwrap().entries;
        assert!(
            entries.iter().any(|e| e.part.as_deref() == Some("2026")),
            "{entries:?}"
        );
        assert!(entries.iter().any(|e| e.title == "1999"), "{entries:?}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
