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
//! output: _site              # build output dir
//! toc: true                 # right-rail "on this page" TOC (website only; inert in a book)
//! css: custom.css
//! head:  head.html           # include-in-header
//! body-end: body.html        # include-after-body  (also: body-start)
//! nav:                       # a list ⇒ left side; or { left: […], right: […] }
//!   - { text: Blog, href: blog.tmd }
//! footer:                    # a string ⇒ left text; or { left/center/right }
//!   right: [{ icon: github, href: "…" }]
//! chapters: [index.tmd, …]   # presence ⇒ a book (no `type:` needed)
//! ```

use super::*;
use serde::Deserialize;

/// The resolved project config — the single internal model every downstream
/// consumer reads.
#[derive(Debug, Clone, Default)]
pub struct SiteConfig {
    /// `chapters:` present ⇒ a book (a centred reading column + chapter drawer, no navbar).
    pub is_book: bool,
    /// `build` output dir (default `_site`, or `_book` for a book).
    pub output_dir: Option<String>,
    pub title: Option<String>,
    /// `author:` as a scalar (`author: Ada`) or a sequence (`author: [Ada, Alan]`),
    /// normalized the same way a page's `author:` is (`frontmatter::string_list`). Held
    /// as a list, not a raw scalar, because reading a sequence as a scalar silently
    /// yielded nothing and published the site *title* as the author instead.
    pub authors: Vec<String>,
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
    /// `toc:` — the right-rail "on this page" table of contents. **Website only:** a
    /// book's in-chapter outline is the chapter drawer, so `Site::page_toc` ignores this
    /// for a book and `validate_toc_scope` tells the author the key is inert (item 76).
    pub toc: Option<bool>,
    pub css: Option<serde_yaml::Value>,
    /// `head` → include-in-header; `body-start`/`body-end` → before/after body.
    pub head: Option<serde_yaml::Value>,
    pub body_start: Option<serde_yaml::Value>,
    pub body_end: Option<serde_yaml::Value>,
    pub nav: Navbar,
    pub footer: Option<Footer>,
    /// Ordered chapter list (book only): a file name or `{ part, chapters }`.
    pub chapters: Vec<serde_yaml::Value>,
    /// `mounts:` — other taliesin projects to mount under a URL prefix, so a site
    /// can link to e.g. a separate docs `book` at `/docs`. In `preview` they're
    /// served live; the static `build` recipe wires them with a second `build`.
    pub mounts: Vec<Mount>,
    /// `publish:` deploy target for `taliesin publish` (absent unless configured).
    pub publish: Option<PublishConfig>,
    /// Project-pinned Python interpreter (`python:` in `_site.yml`), highest
    /// precedence in interpreter resolution. `None` falls back to `.venv`/env/default.
    pub python: Option<String>,
    /// Project-pinned R interpreter (`r:` in `_site.yml`). `None` falls back to env/`R`.
    pub r: Option<String>,
    /// Book-wide theorem-numbering policy (`theorems:` in `_site.yml`). `Some` only when the
    /// config declares it; a chapter with no `theorems:` block of its own inherits it, a
    /// chapter that declares one overrides it wholesale. `None` = no book-level policy.
    pub theorems: Option<crate::render::TheoremConfig>,
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

/// `publish:` says where `taliesin publish` deploys this project. Optional; when
/// absent, publish falls back to a slug of the project directory name. The passcode is
/// never stored here (it lives only as a Cloudflare Pages secret).
#[derive(Debug, Clone, Default)]
pub struct PublishConfig {
    /// Deploy provider. Only `cloudflare` is recognized today.
    pub provider: Option<String>,
    /// Cloudflare Pages project name (overrides the dir-name slug default).
    pub project: Option<String>,
    /// Passcode gate. Absent or `true` = gated (the safe default); `false` = public.
    pub gate: Option<bool>,
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
    "output",
    "toc",
    "css",
    "head",
    "body-start",
    "body-end",
    "nav",
    "footer",
    "chapters",
    "mounts",
    "publish",
    "python",
    "r",
    "theorems",
];

/// `nav:` section keys (the `{ left, right }` mapping form). A typo here silently drops
/// the whole side, so it warns.
const NAV_SECTION_KEYS: &[&str] = &["left", "right"];
/// `footer:` section keys (the `{ left, center, right }` mapping form).
const FOOTER_SECTION_KEYS: &[&str] = &["left", "center", "right"];
/// The keys of a single nav/footer item (`{ text, href, icon }`).
const NAV_ITEM_KEYS: &[&str] = &["text", "href", "icon"];
/// The keys of a `mounts:` sequence entry (`{ at, path }`).
const MOUNT_ITEM_KEYS: &[&str] = &["at", "path"];
/// The keys of the `publish:` block (`{ provider, project, gate }`).
pub(crate) const PUBLISH_KEYS: &[&str] = &["provider", "project", "gate"];

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

/// `toc:` configures the right-rail "on this page" table of contents, and a book no longer
/// has one (item 76, owner ruling 2026-07-27): its in-chapter outline is the chapter
/// drawer, which lists the current chapter to h3 where the rail listed h2 only. So the key
/// is inert in a book — and every book in this repo shipped `toc: true`, which is exactly
/// the "a shipped string says we do something we don't" class item 75 was about. Warn
/// rather than fail: it is a stale line in a config that is otherwise correct, and deleting
/// it is the whole fix. Conditioned on `chapters:` because that is what makes a book.
fn validate_toc_scope(
    value: &serde_yaml::Value,
    warnings: &mut Vec<String>,
    src: ConfigSource<'_>,
) {
    let is_book = value
        .get("chapters")
        .and_then(|v| v.as_sequence())
        .is_some_and(|s| !s.is_empty());
    if is_book && value.get("toc").is_some() {
        warnings.push(format!(
            "{} `toc:` has no effect in a book — a book's in-chapter outline is the \
             chapter drawer, not a right-hand rail: delete the key",
            src.at("toc")
        ));
    }
}

fn parse_native(
    value: &serde_yaml::Value,
    warnings: &mut Vec<String>,
    src: ConfigSource<'_>,
) -> SiteConfig {
    validate_keys(value, warnings, src);
    validate_url(value, warnings);
    validate_toc_scope(value, warnings, src);
    let str_of = |k: &str| value.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let chapters = value
        .get("chapters")
        .and_then(|v| v.as_sequence())
        .cloned()
        .unwrap_or_default();
    // Book-wide `theorems:` policy, inherited by any chapter without its own block. Validate
    // the `numbered:` value like a per-document block (unlocated here: `parse_native` holds
    // only the parsed value, not the raw text), then parse. Absent -> `None`, so the render
    // fallback can tell "book set a policy" from "book said nothing".
    let theorems = if value.get("theorems").is_some() {
        if let Some(map) = value.as_mapping() {
            let mut tw: Vec<crate::render::Warning> = Vec::new();
            crate::frontmatter::validate_theorem_values(map, "", &mut tw);
            warnings.extend(tw.into_iter().map(|w| w.message));
        }
        Some(crate::render::parse_theorem_config_value(value))
    } else {
        None
    };
    SiteConfig {
        is_book: !chapters.is_empty(),
        output_dir: str_of("output"),
        title: str_of("title"),
        authors: crate::site::frontmatter::string_list(value.get("author")),
        description: str_of("description"),
        url: str_of("url"),
        favicon: str_of("favicon"),
        logo: str_of("logo"),
        toc: value.get("toc").and_then(|v| v.as_bool()),
        css: value.get("css").cloned(),
        head: value.get("head").cloned(),
        body_start: value.get("body-start").cloned(),
        body_end: value.get("body-end").cloned(),
        nav: nav_from(value.get("nav")),
        footer: footer_from(value.get("footer")),
        chapters,
        mounts: mounts_from(value.get("mounts")),
        publish: publish_from(value.get("publish")),
        python: str_of("python"),
        r: str_of("r"),
        theorems,
    }
}

/// Parse `mounts:` — a map `{ docs: ../docs }` or a sequence of `{ at, path }`.
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
        Some(serde_yaml::Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|it| {
                Some(Mount {
                    at: it.get("at")?.as_str()?.trim_matches('/').to_string(),
                    path: it.get("path")?.as_str()?.to_string(),
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
            // Match the key token exactly, not a prefix: `nav:` must not match `navigation:`.
            t.strip_prefix(key)
                .is_some_and(|rest| rest.starts_with(':'))
        })
        .map(|i| i + 1)
}

/// Warn on unrecognized keys against the closed native schema: top-level, and the
/// nested `nav:`/`footer:`/`mounts:`/`publish:` structures (a typo in one of those
/// silently drops the whole section/item, so it warns with a "did you mean"). Every
/// warning is prefixed `_site.yml` so it is file-located rather than an anonymous string.
fn validate_keys(value: &serde_yaml::Value, warnings: &mut Vec<String>, src: ConfigSource<'_>) {
    let Some(map) = value.as_mapping() else {
        return;
    };
    let warn = |warnings: &mut Vec<String>, what: &str, key: &str, allowed: &[&'static str]| {
        warnings.push(format!(
            "{} unknown {what} `{key}`{}",
            src.at(key),
            did_you_mean(key, allowed)
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
            "publish" => validate_publish(v, warnings, src),
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
fn validate_mounts(v: &serde_yaml::Value, warnings: &mut Vec<String>, src: ConfigSource<'_>) {
    let serde_yaml::Value::Sequence(seq) = v else {
        return;
    };
    for item in seq {
        if let serde_yaml::Value::Mapping(m) = item {
            for k in m.keys().filter_map(|k| k.as_str()) {
                if !MOUNT_ITEM_KEYS.contains(&k) {
                    warnings.push(format!(
                        "{} unknown mount key `{k}`{}",
                        src.at(k),
                        did_you_mean(k, MOUNT_ITEM_KEYS)
                    ));
                }
            }
        }
    }
}

/// Validate the `publish:` mapping's keys against [`PUBLISH_KEYS`]. A typo silently
/// drops a setting (publish would fall back to a default), so it warns.
fn validate_publish(v: &serde_yaml::Value, warnings: &mut Vec<String>, src: ConfigSource<'_>) {
    let serde_yaml::Value::Mapping(m) = v else {
        return;
    };
    for k in m.keys().filter_map(|k| k.as_str()) {
        if !PUBLISH_KEYS.contains(&k) {
            warnings.push(format!(
                "{} unknown publish key `{k}`{}",
                src.at(k),
                did_you_mean(k, PUBLISH_KEYS)
            ));
        }
    }
}

/// Parse the `publish:` mapping into [`PublishConfig`] (a non-mapping value yields None).
fn publish_from(v: Option<&serde_yaml::Value>) -> Option<PublishConfig> {
    let pv = v?;
    if !pv.is_mapping() {
        return None;
    }
    let s = |k: &str| pv.get(k).and_then(|x| x.as_str()).map(str::to_string);
    Some(PublishConfig {
        provider: s("provider"),
        project: s("project"),
        gate: pv.get("gate").and_then(|x| x.as_bool()),
    })
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
    fn parses_python_and_r_interpreter_pins() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("title: X\npython: .venv/bin/python\nr: /usr/bin/R\n").unwrap();
        let cfg = parse_native(&v, &mut w, ConfigSource(None));
        assert_eq!(cfg.python.as_deref(), Some(".venv/bin/python"));
        assert_eq!(cfg.r.as_deref(), Some("/usr/bin/R"));
        assert!(w.is_empty(), "valid keys warn about nothing: {w:?}");
    }

    #[test]
    fn parses_book_level_theorems_and_its_absence() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("title: X\ntheorems:\n  numbered: false\n").unwrap();
        let cfg = parse_native(&v, &mut w, ConfigSource(None));
        assert!(
            cfg.theorems.is_some(),
            "a declared theorems: parses to Some"
        );
        assert!(
            w.iter().all(|m| !m.contains("config key")),
            "theorems is a recognized _site.yml key: {w:?}"
        );

        let mut w2 = Vec::new();
        let v2: serde_yaml::Value = serde_yaml::from_str("title: X\n").unwrap();
        let cfg2 = parse_native(&v2, &mut w2, ConfigSource(None));
        assert!(
            cfg2.theorems.is_none(),
            "an absent theorems: parses to None"
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

    #[test]
    fn toc_in_a_book_is_diagnosed_as_inert_and_left_alone_in_a_website() {
        // Item 76 (2026-07-27) removed a book's right-rail TOC, which leaves `toc:` doing
        // nothing in a book config. Every book in this repo shipped `toc: true`, so silence
        // would leave a key that reads as configuring a surface that no longer exists —
        // exactly the stale-string class item 75 was about. `chapters:` is what makes it a
        // book, so that is the condition; a website is untouched.
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("toc: true\nchapters:\n  - a.tmd\n").unwrap();
        let _ = parse_native(&v, &mut w, ConfigSource(None));
        assert!(
            w.iter().any(|m| m.contains("toc:") && m.contains("book")),
            "an inert `toc:` in a book must be diagnosed: {w:?}"
        );
        // `toc: false` is equally inert and equally worth deleting, so it warns too.
        let mut w_false = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("toc: false\nchapters:\n  - a.tmd\n").unwrap();
        let _ = parse_native(&v, &mut w_false, ConfigSource(None));
        assert!(
            w_false.iter().any(|m| m.contains("toc:")),
            "`toc: false` in a book is inert too: {w_false:?}"
        );
        // A website (no `chapters:`) still honours it: no warning.
        let mut w_site = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str("title: X\ntoc: true\n").unwrap();
        let _ = parse_native(&v, &mut w_site, ConfigSource(None));
        assert!(
            !w_site.iter().any(|m| m.contains("toc:")),
            "a website's `toc:` is live and must not warn: {w_site:?}"
        );
        // …and a book that says nothing about `toc:` gets no advice it did not earn.
        let mut w_quiet = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str("chapters:\n  - a.tmd\n").unwrap();
        let _ = parse_native(&v, &mut w_quiet, ConfigSource(None));
        assert!(
            !w_quiet.iter().any(|m| m.contains("toc:")),
            "a book with no `toc:` must stay silent: {w_quiet:?}"
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

        // A `mounts:` sequence entry key typo drops the mount silently.
        let w = cfg_warnings("mounts:\n  - att: /docs\n    path: ../docs\n");
        assert!(
            w.iter()
                .any(|w| w.contains("mount key `att`") && w.contains("`at`")),
            "mount key typo: {w:?}"
        );
    }

    #[test]
    fn valid_nested_nav_footer_mounts_have_no_warnings() {
        // The real corpus shape: `{ left: [...], right: [...] }` with text/href items,
        // a footer with left/center/right, and a mounts sequence — none may warn.
        let w = cfg_warnings(concat!(
            "title: Site\n",
            "nav:\n  left:\n    - text: Blog\n      href: blog.tmd\n  right:\n    - icon: github\n      href: 'https://x'\n",
            "footer:\n  left:\n    - text: © 2026\n  center:\n    - text: mid\n  right:\n    - text: end\n",
            "mounts:\n  - at: /docs\n    path: ../docs\n",
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

    #[test]
    fn publish_block_parses_provider_and_project() {
        let dir = tmp("publish-ok");
        std::fs::write(
            dir.join("_site.yml"),
            "title: Book\npublish:\n  provider: cloudflare\n  project: my-book\n",
        )
        .unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        let publish = cfg.publish.expect("publish block parsed");
        assert_eq!(publish.provider.as_deref(), Some("cloudflare"));
        assert_eq!(publish.project.as_deref(), Some("my-book"));
        assert!(
            !warnings.iter().any(|w| w.contains("unknown")),
            "a valid publish block must not warn: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_publish_key_warns_with_did_you_mean() {
        // A typo in a publish key silently drops the setting, so it must warn.
        let w = cfg_warnings("publish:\n  provder: cloudflare\n");
        assert!(
            w.iter()
                .any(|w| w.contains("publish key `provder`") && w.contains("`provider`")),
            "publish key typo: {w:?}"
        );
    }

    #[test]
    fn publish_gate_false_parses() {
        let dir = tmp("publish-gate");
        std::fs::write(
            dir.join("_site.yml"),
            "title: Book\npublish:\n  provider: cloudflare\n  gate: false\n",
        )
        .unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        let publish = cfg.publish.expect("publish block parsed");
        assert_eq!(publish.gate, Some(false));
        assert!(
            !warnings.iter().any(|w| w.contains("unknown")),
            "a valid gate must not warn: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_gate_typo_warns() {
        let w = cfg_warnings("publish:\n  gat: false\n");
        assert!(
            w.iter()
                .any(|w| w.contains("publish key `gat`") && w.contains("`gate`")),
            "gate typo did-you-mean: {w:?}"
        );
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

    #[test]
    fn absent_publish_block_is_none() {
        let dir = tmp("publish-absent");
        std::fs::write(dir.join("_site.yml"), "title: Book\n").unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        assert!(cfg.publish.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
