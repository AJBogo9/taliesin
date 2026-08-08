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
//! head: head.html            # raw markup for every page's <head> — the ONE escape hatch
//! nav:                       # a list ⇒ left side; or { left: […], right: […] }
//!   - { text: Blog, href: blog.tmd }
//! footer:                    # a string ⇒ left text; or { left/center/right }
//!   right: [{ icon: github, href: "…" }]
//! chapters: [index.tmd, …]   # presence ⇒ a book (no `type:` needed)
//! mounts:                    # URL prefix ⇒ another project's directory
//!   docs: ../docs
//! ```
//!
//! Six keys were retired on 2026-08-02: `output:` and `toc:` (both wrote what the tool
//! already does — the build dir is `_site`/`_book`, and the sidebar TOC is decided per page
//! by heading count), and `css:`/`body-start:`/`body-end:` (raw injection at zero adoption,
//! folded into the surviving `head:`). `theorems:` went with the book-wide numbering policy.
//! Each is in `frontmatter::RETIRED_KEYS` under the `config key` scope, so a stale one is
//! answered with what to do instead rather than a did-you-mean.

use super::*;
use serde::Deserialize;

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
    /// `head:` — raw markup injected into every page's `<head>`. The **one** raw-injection
    /// hatch the tool keeps (analytics, a search-console `<meta>`, a custom stylesheet);
    /// the per-document `css:`/`include-*` family and `body-start:`/`body-end:` were retired
    /// on 2026-08-02 at zero adoption. Deliberately not a knob with a default to perfect: it
    /// exists precisely for what the tool cannot anticipate.
    pub head: Option<serde_yaml::Value>,
    pub nav: Navbar,
    pub footer: Option<Footer>,
    /// Ordered chapter list (book only): a file name or `{ part, chapters }`.
    pub chapters: Vec<serde_yaml::Value>,
    /// `mounts:` — other taliesin projects to mount under a URL prefix, so a site
    /// can link to e.g. a separate docs `book` at `/docs`. In `preview` they're
    /// served live; the static `build` recipe wires them with a second `build`.
    pub mounts: Vec<Mount>,
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

/// One `mounts:` entry: serve the project at `path` (relative to the site root)
/// under the `/at/` URL prefix.
#[derive(Debug, Clone)]
pub struct Mount {
    pub at: String,
    pub path: String,
}

/// Why a `mounts:` entry was refused by [`Mount::resolve`].
///
/// "Relative to the site root" was this key's documented contract and nothing enforced it,
/// which made one config line the widest hole in the tool: `Path::join` **replaces** the
/// base when its argument is absolute, and `..` climbed without limit, so
/// `mounts: { x: /etc }` served `/etc` over HTTP (measured: `GET /x/hostname` → 200) and
/// executed any `.tmd` found under a mounted tree. This is not a restriction on what a
/// document may *compute* — it is a restriction on where a *config key* may point the
/// server, which is the one class of untrusted-document defect this project does enforce
/// rather than document (see `SECURITY.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountRefusal {
    /// An absolute `path:`. Never legitimate: a project config that names an absolute
    /// directory is not portable to the machine that clones it.
    Absolute,
    /// The resolved directory left [`Mount::boundary`] — a `..` climb past the sibling
    /// level, or a symlink whose target does.
    OutsideBoundary,
}

impl Mount {
    /// The directory a `path:` may resolve inside: the **parent** of the site root.
    ///
    /// `mounts:` exists to serve another project *beside* this one — every real mount in
    /// this repo is one (`../docs/guide`, `../docs/internals`, `../corpus/course`,
    /// `../corpus/tarn`) — so one level of `..` is the whole documented use, and anything
    /// deeper has no caller. A root with no parent (the filesystem root) bounds to itself.
    pub fn boundary(site_root: &Path) -> PathBuf {
        let root = crate::includes::absolutize(site_root);
        root.parent().map(Path::to_path_buf).unwrap_or(root)
    }

    /// Resolve `path:` against `site_root`, refusing anything outside [`Mount::boundary`].
    ///
    /// Returns the *lexical* target (callers canonicalize for display/serving, as before),
    /// so a mount naming a directory that does not exist still reaches the caller's own
    /// "no directory at …" warning instead of being reported as a traversal.
    pub fn resolve(&self, site_root: &Path) -> Result<PathBuf, MountRefusal> {
        let rel = Path::new(&self.path);
        // `has_root` as well as `is_absolute` so a Windows drive/UNC root cannot slip past
        // a Unix-only check, matching `includes::try_join_in`.
        if rel.is_absolute() || rel.has_root() {
            return Err(MountRefusal::Absolute);
        }
        let root = crate::includes::absolutize(site_root);
        let target = crate::includes::normalize(&root.join(rel));
        let boundary = Self::boundary(site_root);
        if !target.starts_with(&boundary) {
            return Err(MountRefusal::OutsideBoundary);
        }
        // Symlink defense. The lexical check governs the config *text*; a symlink is a
        // filesystem fact the text cannot conjure, so `SECURITY.md`'s symlink allowance
        // ("you placed it") holds for your own checkout and fails for an archive someone
        // sent you — which is exactly the case `mounts:` is reachable in. Only checked when
        // the target exists; a missing one cannot be an escape.
        if let Ok(canon) = target.canonicalize() {
            let cboundary = boundary.canonicalize().unwrap_or(boundary);
            if !canon.starts_with(&cboundary) {
                return Err(MountRefusal::OutsideBoundary);
            }
        }
        Ok(target)
    }

    /// The warning shown for a refused mount: what was refused, why, and the shape that
    /// works. Phrased so the fix is obvious to the author of a legitimate config, since a
    /// misplaced project is a likelier cause than an attack.
    pub fn refusal_warning(&self, site_root: &Path, why: MountRefusal) -> String {
        let tail = match why {
            MountRefusal::Absolute => "an absolute path is not allowed".to_string(),
            MountRefusal::OutsideBoundary => {
                format!(
                    "it resolves outside {}",
                    Self::boundary(site_root).display()
                )
            }
        };
        format!(
            "mount '{}': ignoring `path: {}` — {tail}. A mount serves another project \
             beside this one, so `path:` must be relative to the site root and may climb \
             at most one level (e.g. `../docs`)",
            self.at, self.path
        )
    }
}

/// Drop every `mounts:` entry that fails containment, warning once per entry.
///
/// Enforced here, at the single parse boundary, rather than at each consumer: `preview`
/// serves mounts live, `build` prints a per-mount recipe, `map` reports them and
/// `link_targets_enclosing_mount` validates links into them, and a check placed in one of
/// those would leave the others reading the raw string. A refused mount is *absent*, so
/// every one of them degrades the same way.
fn retain_contained_mounts(root: &Path, mounts: &mut Vec<Mount>, warnings: &mut Vec<String>) {
    mounts.retain(|m| match m.resolve(root) {
        Ok(_) => true,
        Err(why) => {
            warnings.push(m.refusal_warning(root, why));
            false
        }
    });
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Navbar {
    #[serde(default)]
    pub left: Vec<NavItem>,
    #[serde(default)]
    pub right: Vec<NavItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Footer {
    #[serde(default)]
    pub left: Vec<NavItem>,
    #[serde(default)]
    pub center: Vec<NavItem>,
    #[serde(default)]
    pub right: Vec<NavItem>,
}

/// A navbar/footer entry. `text` is the label; `href` the link; `icon` a bundled
/// social glyph name (github / linkedin / rss / …) rendered as an inline SVG.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NavItem {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub href: Option<String>,
    #[serde(default)]
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
    //
    // `head:` is the ONE raw-injection hatch that survives (analytics, search-console
    // verification, a custom stylesheet) — a published tool needs exactly one, and the
    // per-document `css:`/`include-*` family plus `body-start:`/`body-end:` went with the
    // rest on 2026-08-02 at measured zero adoption.
    "head",
    "nav",
    "footer",
    "chapters",
    "mounts",
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
/// *real* error (the site silently degrades to defaults), distinct from a legitimately
/// absent `_site.yml`. The site build matches this prefix to count a malformed config as a
/// `--strict` problem, and the live preview watcher matches it to keep the last-good site
/// instead of replacing it with the degraded default. Keep it stable: those consumers key
/// off it (see `crates/server/src/build.rs` + `serve_site/mod.rs`).
pub const MALFORMED_CONFIG_PREFIX: &str = "_site.yml is not valid YAML";

/// Stable prefix on the advisory a *missing* `_site.yml` pushes. A bare directory of `.tmd`
/// pages is a legitimate project, so this is a note rather than a defect: `build` already
/// declines to count it toward `--strict`, and `check` must not fail on it either. Keep it
/// stable (see `crates/server/src/check.rs`).
pub const MISSING_CONFIG_PREFIX: &str = "no _site.yml at";

/// Load + parse `_site.yml` at `root` into the native flat schema.
pub(in crate::site) fn load_config(root: &Path, warnings: &mut Vec<String>) -> SiteConfig {
    let path = root.join("_site.yml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        // A directory still holding the pre-rename `_quarto.yml` is NOT the bare-directory
        // case: it has a config and every setting in it is being ignored, so the project
        // builds with its `title:` and everything else silently defaulted. Reporting it as
        // "no config here" is what hid it, because that advisory is the one `check` drops
        // from its tally on purpose. Name the file that is actually on disk instead.
        if root.join("_quarto.yml").is_file() {
            warnings.push(format!(
                "found `_quarto.yml` at {}, but the project config is now `_site.yml`: \
                 rename it, or its settings go on being ignored",
                root.display()
            ));
            return SiteConfig::default();
        }
        // A missing `_site.yml` is legitimate (a bare directory of `.tmd` pages), not an
        // error — distinct from the malformed case below, which downstream counts.
        warnings.push(format!("{MISSING_CONFIG_PREFIX} {}", root.display()));
        return SiteConfig::default();
    };
    let value: serde_yaml::Value = match serde_yaml::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            // Malformed YAML: degrade to defaults but tag the warning so the build can
            // fail `--strict` on it and the preview can keep its last-good config.
            warnings.push(format!("{MALFORMED_CONFIG_PREFIX}: {e}"));
            return SiteConfig::default();
        }
    };
    let mut cfg = parse_native(&value, warnings, ConfigSource(Some(&text)));
    retain_contained_mounts(root, &mut cfg.mounts, warnings);
    cfg
}

/// Whether a discovery warning is the benign "this directory has no `_site.yml`" advisory,
/// as opposed to a real defect. `check` uses it to keep an advisory out of its problem tally.
pub fn is_missing_config_warning(warning: &str) -> bool {
    warning.starts_with(MISSING_CONFIG_PREFIX)
}

/// Whether a discovery warning is the malformed-`_site.yml` marker (a real error, not the
/// benign "no _site.yml" case). Shared by the server's build + watcher.
pub fn is_malformed_config_warning(warning: &str) -> bool {
    warning.starts_with(MALFORMED_CONFIG_PREFIX)
}

/// `url:`, when set, must be an absolute origin with a scheme: it seeds every machine-read
/// absolute URL (sitemap `<loc>`, `robots.txt`/feed `Sitemap:`, `og:url`, llms.txt links).
/// A scheme-less `url: ex.com` builds clean and emits `<loc>ex.com/</loc>` +
/// `Sitemap: ex.com/sitemap.xml` — machine-invalid, under a green `check`. Warn (a
/// diagnostic, not a knob — the `page-layout` / site-`image:` precedent). A blank `url:` is
/// treated as unset by [`Site::canonical_base`], so it is left alone.
fn validate_url(value: &serde_yaml::Value, warnings: &mut Vec<String>) {
    let Some(url) = value.get("url").and_then(|v| v.as_str()).map(str::trim) else {
        return;
    };
    if !url.is_empty() && !(url.starts_with("http://") || url.starts_with("https://")) {
        warnings.push(format!(
            "url: `{url}` has no scheme — sitemap, robots.txt, feed and og:url need an \
             absolute URL (write `https://{url}`)"
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
fn validate_chapters(value: &serde_yaml::Value, warnings: &mut Vec<String>, src: ConfigSource<'_>) {
    fn walk(list: &[serde_yaml::Value], warnings: &mut Vec<String>, src: ConfigSource<'_>) {
        for item in list {
            // A bare path string is the common form and always well-formed.
            let Some(map) = item.as_mapping() else {
                continue;
            };
            for k in map.keys().filter_map(|k| k.as_str()) {
                if !CHAPTER_ITEM_KEYS.contains(&k) {
                    warnings.push(format!(
                        "{} unknown chapter key `{k}`{} — an entry taliesin cannot read as \
                         a chapter (`file:`) or a part (`part:`) is DROPPED from the book \
                         silently, so this is a missing chapter, not a cosmetic warning",
                        src.at(k),
                        did_you_mean(k, CHAPTER_ITEM_KEYS)
                    ));
                }
            }
            // A mapping that names neither a file nor a part is the silent-drop case even
            // when every key it does carry is spelled correctly (e.g. a lone `text:`).
            if !map.contains_key("file") && !map.contains_key("part") {
                let first = map.keys().filter_map(|k| k.as_str()).next().unwrap_or("");
                warnings.push(format!(
                    "{} a `chapters:` entry names no `file:` and no `part:`, so it is \
                     dropped from the book: give it a `file:`",
                    src.at(first)
                ));
            }
            if let Some(inner) = map.get("chapters").and_then(|v| v.as_sequence()) {
                walk(inner, warnings, src);
            }
        }
    }
    if let Some(list) = value.get("chapters").and_then(|v| v.as_sequence()) {
        walk(list, warnings, src);
    }
}

fn parse_native(
    value: &serde_yaml::Value,
    warnings: &mut Vec<String>,
    src: ConfigSource<'_>,
) -> SiteConfig {
    validate_keys(value, warnings, src);
    validate_url(value, warnings);
    validate_chapters(value, warnings, src);
    let str_of = |k: &str| value.get(k).and_then(|v| v.as_str()).map(str::to_string);
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
        head: value.get("head").cloned(),
        nav: nav_from(value.get("nav")),
        footer: footer_from(value.get("footer")),
        chapters,
        mounts: mounts_from(value.get("mounts")),
        python: str_of("python"),
        bibliography: crate::site::frontmatter::string_list(value.get("bibliography")),
    }
}

/// Parse `mounts:` — a map of URL prefix to project directory, `{ docs: ../docs }`.
///
/// One spelling. The `- { at:, path: }` sequence form said the same thing with two extra
/// key names and was retired on 2026-08-02 unused; a leftover one now draws the
/// `mounts entry key` diagnostic from [`validate_mounts`] rather than mounting nothing.
fn mounts_from(v: Option<&serde_yaml::Value>) -> Vec<Mount> {
    match v {
        Some(serde_yaml::Value::Mapping(m)) => m
            .iter()
            .filter_map(|(k, val)| {
                Some(Mount {
                    at: k.as_str()?.trim_matches('/').to_string(),
                    path: val.as_str()?.to_string(),
                })
            })
            .collect(),
        _ => Vec::new(),
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
    /// The `file:line:` prefix for a diagnostic about `key`, in the same linter form as
    /// the page-level warnings (so the editor can jump to it). Falls back to the bare
    /// filename when the key cannot be located — a warning without a line still beats a
    /// wrong one.
    fn at(&self, key: &str) -> String {
        match self.0.and_then(|t| key_line(t, key)) {
            Some(line) => format!("_site.yml:{line}:"),
            None => "_site.yml:".to_string(),
        }
    }
}

/// The 1-based line where `key` is written in `_site.yml`, at any nesting depth (a list
/// item's `- key:` counts). First match wins: a duplicate key is a YAML error the parse
/// step already reports.
fn key_line(text: &str, key: &str) -> Option<usize> {
    text.lines()
        .position(|l| {
            let t = l.trim_start().trim_start_matches("- ").trim_start();
            // Also look inside a flow mapping: `- { file: a.tmd, text: A }` is how chapter
            // and nav entries are usually written, and a diagnostic about one of those keys
            // is worth a line number.
            let t = t.strip_prefix('{').map_or(t, str::trim_start);
            // Match the key token exactly, not a prefix: `nav:` must not match `navigation:`.
            t.strip_prefix(key)
                .is_some_and(|rest| rest.starts_with(':'))
        })
        .map(|i| i + 1)
}

/// Warn on unrecognized keys against the closed native schema: top-level, and the
/// nested `nav:`/`footer:`/`mounts:` structures (a typo in one of those silently drops
/// the whole section/item, so it warns with a "did you mean"). Every
/// warning is prefixed `_site.yml` so it is file-located rather than an anonymous string.
fn validate_keys(value: &serde_yaml::Value, warnings: &mut Vec<String>, src: ConfigSource<'_>) {
    let Some(map) = value.as_mapping() else {
        return;
    };
    let warn = |warnings: &mut Vec<String>, what: &str, key: &str, allowed: &[&'static str]| {
        // Through `unknown_key_message`, not a bare did-you-mean: it consults
        // `RETIRED_KEYS` first, so a key this config USED to honor is answered with what
        // to do instead. Without it a retired `toc:` draws "did you mean `logo`?" — a
        // confident instruction to write something unrelated. The `what` label is the
        // register's scope column, so `config key` / `mounts entry key` entries are only
        // consulted where they actually lived.
        warnings.push(format!(
            "{} {}",
            src.at(key),
            crate::frontmatter::unknown_key_message(what, key, allowed)
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
            "mounts" => validate_mounts(v, warnings, src),
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
    warnings: &mut Vec<String>,
    src: ConfigSource<'_>,
) {
    match v {
        serde_yaml::Value::Mapping(m) => {
            for (k, section) in m {
                let Some(key) = k.as_str() else { continue };
                if section_keys.contains(&key) {
                    validate_items(section, ctx, warnings, src);
                } else {
                    warnings.push(format!(
                        "{} unknown {ctx} section `{key}`{}",
                        src.at(key),
                        did_you_mean(key, section_keys)
                    ));
                }
            }
        }
        serde_yaml::Value::Sequence(_) => validate_items(v, ctx, warnings, src),
        _ => {}
    }
}

/// Validate one or a list of nav/footer items: each mapping's keys against
/// [`NAV_ITEM_KEYS`] (a bare string item is a plain label, nothing to check).
fn validate_items(
    v: &serde_yaml::Value,
    ctx: &str,
    warnings: &mut Vec<String>,
    src: ConfigSource<'_>,
) {
    let items: Vec<&serde_yaml::Value> = match v {
        serde_yaml::Value::Sequence(seq) => seq.iter().collect(),
        other => vec![other],
    };
    for item in items {
        if let serde_yaml::Value::Mapping(m) = item {
            for k in m.keys().filter_map(|k| k.as_str()) {
                if !NAV_ITEM_KEYS.contains(&k) {
                    warnings.push(format!(
                        "{} unknown {ctx} item key `{k}`{}",
                        src.at(k),
                        did_you_mean(k, NAV_ITEM_KEYS)
                    ));
                }
            }
        }
    }
}

/// Validate `mounts:` in its sequence form (`- { at, path }`); the mapping form
/// (`{ prefix: path }`) has author-chosen keys, so it can't be checked.
/// Flag a `mounts:` written in the retired `- { at:, path: }` sequence form.
///
/// `mounts_from` reads the mapping form only, so a leftover sequence mounts *nothing* and
/// the pages it should have served 404 — a silent failure worth a diagnostic. Every key in
/// such an entry is reported through the shared `unknown_key_message`, which finds `at` and
/// `path` in `RETIRED_KEYS` under the `mounts entry key` scope and answers with the mapping
/// form to write instead.
fn validate_mounts(v: &serde_yaml::Value, warnings: &mut Vec<String>, src: ConfigSource<'_>) {
    let serde_yaml::Value::Sequence(seq) = v else {
        return;
    };
    for item in seq {
        if let serde_yaml::Value::Mapping(m) = item {
            for k in m.keys().filter_map(|k| k.as_str()) {
                warnings.push(format!(
                    "{} {}",
                    src.at(k),
                    crate::frontmatter::unknown_key_message("mounts entry key", k, &[])
                ));
            }
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

/// Coerce a value into a list of [`NavItem`]: a string → one text item, a single
/// `{…}` → one item, a list → many. Bare strings *inside* a list are handled too
/// (they would otherwise fail to deserialize into a struct and be silently dropped).
fn items(v: Option<&serde_yaml::Value>) -> Vec<NavItem> {
    match v {
        None => Vec::new(),
        Some(serde_yaml::Value::Sequence(seq)) => seq.iter().filter_map(nav_item).collect(),
        Some(v) => nav_item(v).into_iter().collect(),
    }
}

/// One nav/footer entry from a YAML value: a bare string becomes a text label; a
/// `{…}` mapping deserializes into a [`NavItem`].
fn nav_item(v: &serde_yaml::Value) -> Option<NavItem> {
    match v {
        serde_yaml::Value::String(s) => Some(NavItem {
            text: Some(s.clone()),
            ..NavItem::default()
        }),
        other => serde_yaml::from_value(other.clone()).ok(),
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

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
        assert_eq!(cfg.python.as_deref(), Some(".venv/bin/python"));
        assert!(w.is_empty(), "valid keys warn about nothing: {w:?}");
    }

    /// Every key this config retired on 2026-08-02 must answer with its REASON, not with a
    /// did-you-mean. The register is scoped, so these only resolve as `config key`; the
    /// wiring that makes that happen is `validate_keys`' use of `unknown_key_message`, and
    /// without it `toc:` drew "did you mean `logo`?" — an instruction to write something
    /// unrelated. One assertion per retired key, because a missing register entry is
    /// silent by construction.
    #[test]
    fn every_retired_config_key_explains_itself_instead_of_guessing() {
        for (key, yaml, needle) in [
            ("toc", "title: X\ntoc: true\n", "now automatic"),
            // The needle is the INSTRUCTION, not the justification. It used to be "wrote
            // the default" — a sentence about why the key went, which a note collapsed to
            // one sentence correctly drops. What an author needs is where the build writes.
            ("output", "title: X\noutput: _site\n", "`--out`"),
            ("css", "title: X\ncss: extra.css\n", "use `head:`"),
            (
                "body-start",
                "title: X\nbody-start: a.html\n",
                "no successor",
            ),
            ("body-end", "title: X\nbody-end: a.html\n", "no successor"),
            (
                "theorems",
                "title: X\ntheorems:\n  numbered: false\n",
                "its own front matter",
            ),
        ] {
            let mut w = Vec::new();
            let v: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
            parse_native(&v, &mut w, ConfigSource(None));
            let msg = w
                .iter()
                .find(|m| m.contains(&format!("`{key}`")))
                .unwrap_or_else(|| panic!("retired `{key}:` drew no diagnostic at all: {w:?}"));
            assert!(
                msg.contains("removed on 2026-08-02"),
                "`{key}:` must say it was removed, got: {msg}"
            );
            assert!(
                msg.contains(needle),
                "`{key}:` must say what to do instead ({needle:?}), got: {msg}"
            );
            assert!(
                !msg.contains("did you mean"),
                "a retired key must never be answered with a rename hint: {msg}"
            );
        }
    }

    /// `head:` is the survivor of the raw-injection family and must keep working.
    #[test]
    fn head_is_the_one_raw_injection_hatch_that_stays() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("title: X\nhead: |\n  <meta name=\"x\" content=\"y\">\n").unwrap();
        let cfg = parse_native(&v, &mut w, ConfigSource(None));
        assert!(cfg.head.is_some(), "head: parses");
        assert!(w.is_empty(), "head: is a recognized key: {w:?}");
    }

    /// A `mounts:` left in the retired `- { at:, path: }` form mounts nothing, so it must
    /// not be silent: `mounts_from` reads the mapping form only.
    #[test]
    fn a_retired_mounts_sequence_is_diagnosed_not_silently_ignored() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("title: X\nmounts:\n  - { at: docs, path: ../docs }\n").unwrap();
        let cfg = parse_native(&v, &mut w, ConfigSource(None));
        assert!(cfg.mounts.is_empty(), "the sequence form mounts nothing");
        for key in ["at", "path"] {
            let msg = w
                .iter()
                .find(|m| m.contains(&format!("`{key}`")))
                .unwrap_or_else(|| panic!("no diagnostic for `{key}`: {w:?}"));
            assert!(
                msg.contains("mapping of URL prefix"),
                "`{key}` must name the form to write instead, got: {msg}"
            );
        }
    }

    /// The mapping form is the one spelling and stays clean.
    #[test]
    fn the_mounts_mapping_form_parses_without_warning() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("title: X\nmounts:\n  docs/guide: ../docs/guide\n").unwrap();
        let cfg = parse_native(&v, &mut w, ConfigSource(None));
        assert_eq!(cfg.mounts.len(), 1);
        assert_eq!(cfg.mounts[0].at, "docs/guide");
        assert_eq!(cfg.mounts[0].path, "../docs/guide");
        assert!(w.is_empty(), "the mapping form warns about nothing: {w:?}");
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
    fn a_bad_book_level_theorems_numbered_value_warns() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("title: X\ntheorems:\n  numbered: banana\n").unwrap();
        let _ = parse_native(&v, &mut w, ConfigSource(None));
        assert!(
            w.iter().any(|m| m.contains("numbered")),
            "a bad book-level numbered value is diagnosed: {w:?}"
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
        // A list-item key is found too (`- at:` inside `mounts:`).
        assert_eq!(key_line("mounts:\n  - at: /docs\n", "at"), Some(2));
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
            assert!(
                !w.iter().any(|m| m.contains("scheme")),
                "a scheme'd or blank url must not warn ({url:?}): {w:?}"
            );
        }
    }

    /// The book-vs-website `toc:` diagnostic is gone with the key itself: `_site.yml toc:` was
    /// retired on 2026-08-02 and the sidebar TOC is decided per page by `Site::page_toc`, which
    /// already returns `false` for a book unconditionally. A book that still carries the key is
    /// answered by the retired register (asserted above), not by a special book-scope rule.
    #[test]
    fn a_book_carrying_the_retired_toc_key_gets_the_retirement_message() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("toc: true\nchapters:\n  - a.tmd\n").unwrap();
        parse_native(&v, &mut w, ConfigSource(None));
        let msg = w
            .iter()
            .find(|m| m.contains("`toc`"))
            .unwrap_or_else(|| panic!("no diagnostic: {w:?}"));
        assert!(msg.contains("now automatic"), "got: {msg}");
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
            warnings.iter().any(|w| is_malformed_config_warning(w)),
            "malformed YAML must be tagged: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_site_yml_is_not_a_malformed_config() {
        // A bare directory with no `_site.yml` is legitimate; its warning must NOT match
        // the malformed marker (so the build doesn't fail `--strict` on a missing file).
        let dir = tmp("missing");
        let mut warnings = Vec::new();
        let _ = load_config(&dir, &mut warnings);
        assert!(
            warnings.iter().any(|w| w.starts_with("no _site.yml")),
            "missing config warns: {warnings:?}"
        );
        assert!(
            !warnings.iter().any(|w| is_malformed_config_warning(w)),
            "a missing file must not be reported as malformed: {warnings:?}"
        );
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
                .any(|w| w.contains("_quarto.yml") && w.contains("_site.yml")),
            "name the file that is there AND the name it needs: {warnings:?}"
        );
        assert!(
            !warnings.iter().any(|w| is_missing_config_warning(w)),
            "must not fall back to the advisory `check` filters out: {warnings:?}"
        );
        assert!(
            !warnings.iter().any(|w| is_malformed_config_warning(w)),
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
        warnings
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
    fn nested_nav_footer_mount_typos_warn_instead_of_silently_dropping() {
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

        // A `mounts:` written in the retired sequence form mounts NOTHING, so every key in
        // it is reported (see `a_retired_mounts_sequence_is_diagnosed_not_silently_ignored`
        // for the message); here the point is that the shape is not accepted in silence.
        let w = cfg_warnings("mounts:\n  - att: /docs\n    path: ../docs\n");
        assert!(
            w.iter().any(|w| w.contains("mounts entry key `att`")),
            "mount key typo: {w:?}"
        );
    }

    #[test]
    fn valid_nested_nav_footer_mounts_have_no_warnings() {
        // The real corpus shape: `{ left: [...], right: [...] }` with text/href items,
        // a footer with left/center/right, and a `mounts:` mapping — none may warn.
        let w = cfg_warnings(concat!(
            "title: Site\n",
            "nav:\n  left:\n    - text: Blog\n      href: blog.tmd\n  right:\n    - icon: github\n      href: 'https://x'\n",
            "footer:\n  left:\n    - text: © 2026\n  center:\n    - text: mid\n  right:\n    - text: end\n",
            "mounts:\n  docs: ../docs\n",
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
            !warnings.iter().any(|w| is_malformed_config_warning(w)),
            "a valid config is not malformed: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Items 80 + 117 (2026-07-28). `mounts:` had *no* containment: measured before the fix,
    // `mounts: { escaped: /etc }` under `preview` answered `GET /escaped/hostname` with 200
    // and the contents of `/etc/hostname`, because `Path::join` replaces the base on an
    // absolute argument and `..` climbed without limit.
    //
    // The positive row is load-bearing, not decoration: an all-negative table is a broken
    // probe until proven otherwise (LESSONS.md), and it is also what stops a future "just
    // reject `..`" simplification, since every real mount in this repo climbs one level.
    #[test]
    fn a_mount_may_not_escape_the_directory_beside_the_project() {
        let dir = tmp("mount-escape");
        // `ghost:` climbs to a directory that does not exist, which is the only row the
        // *lexical* check owns: for a target that exists, the canonical symlink check below
        // it refuses the same path, so without this row disabling the lexical check leaves
        // every assertion green (measured — the mutant survived).
        std::fs::write(
            dir.join("_site.yml"),
            "title: X\nmounts:\n  etc: /etc\n  up: ../../..\n  ghost: ../../nowhere-unlikely\n  \
             sibling: ../beside\n",
        )
        .unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);

        let kept: Vec<&str> = cfg.mounts.iter().map(|m| m.at.as_str()).collect();
        assert_eq!(
            kept,
            ["sibling"],
            "an absolute and a climbing mount must both be dropped, the sibling kept: {kept:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("mount 'etc'") && w.contains("absolute path is not allowed")),
            "the absolute mount must say why: {warnings:?}"
        );
        for at in ["up", "ghost"] {
            assert!(
                warnings
                    .iter()
                    .any(|w| w.contains(&format!("mount '{at}'")) && w.contains("resolves outside")),
                "the climbing mount '{at}' must name the boundary: {warnings:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // The lexical check alone is fooled by a symlink, which is the shape an untrusted
    // *archive* carries (`SECURITY.md`'s symlink allowance assumes you placed it, which is
    // false for a project someone sent you — item 88's family).
    #[cfg(unix)]
    #[test]
    fn a_mount_may_not_reach_outside_through_a_symlink() {
        let dir = tmp("mount-symlink");
        // `/etc` is outside the boundary (the temp dir), exists, and is read-only.
        std::os::unix::fs::symlink("/etc", dir.join("link")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: X\nmounts:\n  x: link\n").unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        assert!(
            cfg.mounts.is_empty(),
            "a mount whose lexical path is in-bounds but whose symlink target is not \
             must be refused: {:?}",
            cfg.mounts
        );
        assert!(
            warnings.iter().any(|w| w.contains("resolves outside")),
            "the symlink escape must be reported: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // The repo's own four mounts are the real-world positive row: they are the only mounts
    // that exist anywhere, and all four climb one level (`../docs/guide`, `../corpus/tarn`).
    // If containment ever narrows to "no `..` at all", this fails instead of the docs site
    // silently losing its `/docs/` tree in preview.
    #[test]
    fn the_repos_own_site_keeps_every_mount() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site");
        let text = std::fs::read_to_string(root.join("_site.yml")).unwrap();
        // Declared-vs-kept as an identity rather than a literal count, so adding a mount to
        // the marketing site does not fail this test — only *refusing* one does.
        let declared = serde_yaml::from_str::<serde_yaml::Value>(&text)
            .unwrap()
            .get("mounts")
            .and_then(|m| m.as_mapping().map(serde_yaml::Mapping::len))
            .unwrap();
        assert!(declared >= 4, "the fixture must be real: {declared} mounts");

        let mut warnings = Vec::new();
        let cfg = load_config(&root, &mut warnings);
        assert_eq!(
            cfg.mounts.len(),
            declared,
            "every declared mount must survive containment; kept {:?} (warnings {:?})",
            cfg.mounts.iter().map(|m| &m.at).collect::<Vec<_>>(),
            warnings
        );
        assert!(
            !warnings.iter().any(|w| w.contains("ignoring `path:")),
            "no real mount may be refused: {warnings:?}"
        );
    }
}
