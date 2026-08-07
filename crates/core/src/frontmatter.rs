//! Front-matter schema validation.
//!
//! taliesin reads a leading YAML `---` block and validates it against its OWN
//! recognized key set. A key taliesin does not
//! implement, whether a typo like `treme:` or a legacy term it does not honor, is
//! flagged by [`validate_front_matter`]: every unknown top-level key, plus every
//! unknown immediate child of the nested `execute:` / `listing:` / `hero:`
//! blocks, each suggesting the closest known key. It only warns (located
//! for click-to-source); rendering is unaffected, an unknown key still renders.

use crate::render::Warning;

/// Top-level front-matter keys taliesin recognizes: the closed set of keys it
/// actually implements, plus every key the corpus/docs use. Intentionally tight
/// (the native flip), so a key taliesin doesn't implement, or a typo,
/// now warns instead of being silently ignored. Top-level keys plus the immediate
/// children of `execute:` / `listing:` / `hero:` are linted — and so are `format:`
/// sub-keys, by [`validate_format_subkeys`]. They were once deliberately exempt
/// "because an extension owns them", but no extension mechanism exists; see that
/// function's docs for why the exemption was a lie rather than a design.
pub(crate) const KNOWN_KEYS: &[&str] = &[
    // Identity / metadata
    "title",
    "subtitle",
    "author",
    "date",
    "description",
    "lang",
    "categories",
    // Images / social
    "image",
    "image-alt",
    // Deck chrome: a persistent per-slide footer text + corner logo image (deck-only,
    // ignored elsewhere).
    "footer",
    "logo",
    // Output / format / theme
    "format",
    "theme",
    "page-layout",
    // Drafts: `draft: true` holds a page (or book chapter) out of the published build
    // (output, nav, listings); the live preview still shows it, badged.
    "draft",
    // Title block: `title-block-style: none` is honored (suppresses the visible
    // header); see `render::detect_title_block_hidden`.
    "title-block-style",
    // Table of contents
    "toc",
    // Citations. `csl` is recognized but NOT honored; see UNSUPPORTED_KEYS.
    "bibliography",
    "csl",
    // Execution
    "execute",
    // Listings / project pages
    "listing",
    "hero",
    // Theorem environments (per-document shared-counter config; see render::TheoremConfig).
    "theorems",
];

/// Keys taliesin RECOGNIZES but does not honor: it reads them, then ignores them.
///
/// They stay in [`KNOWN_KEYS`] on purpose, and the reason is not politeness. `csl` names a
/// real thing an author brought from another tool, and the honest answer is "recognized,
/// not honored" rather than either silence or a rename hint — so the key is recognized and
/// a dedicated diagnostic says so (`diagnostics::csl_recognized_but_unsupported`).
///
/// This used to be argued from `css` being edit distance 1 away, which would have made a
/// bare did-you-mean tell the author to rename their citation-style key to a stylesheet
/// key. `css` was retired on 2026-08-02, so that specific collision is gone; the reason
/// above is the one that survives it, and it was always the load-bearing half.
///
/// Also the exclusion list for the editor vocabulary (`vocab::vocab`): an unsupported key
/// must never be OFFERED as a completion. Recognizing what an author already wrote and
/// suggesting they write it are different acts.
pub(crate) const UNSUPPORTED_KEYS: &[&str] = &["csl"];

/// Vocabulary taliesin USED to honor. A key here is genuinely unknown now, so it still
/// warns — but a removal and a misspelling are different mistakes and the author needs to
/// be told which one they made. Consulted by [`unknown_key_message`] ahead of the
/// did-you-mean search, which would otherwise either stay silent or offer whichever
/// surviving key happens to be closest, both of which read as "you typed this wrong".
///
/// **No entry is phrased as a did-you-mean, even where a successor exists.**
/// `codes::extract_suggestion` lifts that exact phrase into a structured fix an agent
/// applies mechanically, and none of these are mechanical renames: `hero:` takes a
/// different set of sub-keys than `about:` did, so a blind rename trades a warning for a
/// document that is wrong in a new way. Same reasoning as `validate_unsupported_keys`.
///
/// `(scope, key, what to do instead)`, where `scope` is [`unknown_key_message`]'s `what`
/// label, so a retired *sub*-key is only recognized in the map it actually lived in.
pub(crate) const RETIRED_KEYS: &[(&str, &str, &str)] = &[
    // --- The raw-injection family, retired 2026-08-02. Four front-matter keys and three
    // `_site.yml` ones went; `_site.yml head:` is the survivor, kept deliberately as the
    // one escape hatch a published tool needs (analytics, search-console verification,
    // a custom stylesheet). Every note therefore points at the same successor.
    (
        "front-matter key",
        "css",
        "it was removed on 2026-08-02: per-document stylesheet injection had no users, and \
         one escape hatch is enough. Put the rules in `_site.yml head:` (a `<style>` or a \
         `<link>`), which applies site-wide, or use a `theme:` for colours",
    ),
    (
        "front-matter key",
        "include-in-header",
        "it was removed on 2026-08-02: use `_site.yml head:`, which injects the same markup \
         into `<head>` for the whole project. There is no per-document form any more",
    ),
    (
        "front-matter key",
        "include-before-body",
        "it was removed on 2026-08-02 with no successor: nothing used it, and markup that \
         must precede the content is content — write it in the document. Site-wide markup \
         goes in `_site.yml head:`",
    ),
    (
        "front-matter key",
        "include-after-body",
        "it was removed on 2026-08-02 with no successor: nothing used it, and markup that \
         must follow the content is content — write it in the document. Site-wide markup \
         goes in `_site.yml head:`",
    ),
    (
        "front-matter key",
        "prose-lint",
        "it was removed on 2026-08-02 along with the linter it configured: it was opt-in \
         and never opted into. The doubled-word, weasel-word and banned-phrase checks are \
         gone; nothing replaces them, so delete the key",
    ),
    (
        "execute key",
        "echo",
        "it was removed on 2026-08-02: the per-cell `#| echo: false` is what documents \
         actually use, and a document-wide default that silently changes every cell reads \
         worse than saying it on the cells you mean. Write `#| echo: false` in each cell",
    ),
    (
        "execute key",
        "include",
        "it was removed on 2026-08-02 with no successor: it suppressed a cell's source AND \
         its output, which `#| echo: false` plus simply not writing the cell already cover",
    ),
    (
        "listing key",
        "sort",
        "it was removed on 2026-08-02: every listing that set it wrote `\"date desc\"`, \
         which is what a listing already does. Newest first is the only order now, so \
         delete the key",
    ),
    (
        "hero key",
        "image",
        "it was removed on 2026-08-02: the hero banner is type, not a figure, and nothing \
         set it. Put the image in the page body as a normal figure",
    ),
    (
        "hero key",
        "image-alt",
        "it was removed on 2026-08-02 with `hero.image`, whose alt text it carried",
    ),
    (
        "theorems key",
        "numbered",
        "it was removed on 2026-08-02: theorems are numbered, scoped to their book chapter \
         automatically, and the un-numbering policies had no users. `shared:` (one counter \
         across several kinds) is what survives the block",
    ),
    // --- `_site.yml`. Scope `config key` is what `site::config`'s validator labels a
    // top-level key, so these are only recognized there.
    (
        "config key",
        "css",
        "it was removed on 2026-08-02: use `head:`, which takes the same markup and is the \
         one raw-injection hatch the config keeps",
    ),
    (
        "config key",
        "body-start",
        "it was removed on 2026-08-02 with no successor: nothing used it. Site-wide markup \
         goes in `head:`",
    ),
    (
        "config key",
        "body-end",
        "it was removed on 2026-08-02 with no successor: nothing used it. Site-wide markup \
         goes in `head:`; a deferred script in `<head>` runs at the same point",
    ),
    (
        "config key",
        "output",
        "it was removed on 2026-08-02: both projects that set it wrote the default. `build` \
         writes `_site/` (`_book/` for a book), or wherever `--out` says",
    ),
    (
        "config key",
        "toc",
        "it was removed on 2026-08-02 and the sidebar table of contents is now automatic: \
         an article page with enough headings gets one. To force it on or off for a page, \
         write `toc:` in that page's own front matter",
    ),
    (
        "config key",
        "theorems",
        "it was removed on 2026-08-02 with the `numbered:` policy it carried book-wide. A \
         chapter that shares counters across kinds says so in its own front matter: \
         `theorems: { shared: [...] }`",
    ),
    (
        "mounts entry key",
        "at",
        "the `- { at:, path: }` list form was removed on 2026-08-02. Write `mounts:` as a \
         mapping of URL prefix to project directory instead: `mounts:\\n  docs: ../docs`",
    ),
    (
        "mounts entry key",
        "path",
        "the `- { at:, path: }` list form was removed on 2026-08-02. Write `mounts:` as a \
         mapping of URL prefix to project directory instead: `mounts:\\n  docs: ../docs`",
    ),
    (
        "front-matter key",
        "datasets",
        "it was removed on 2026-08-02 and the annotations now ride the invocation that \
         uses them: write `{{< dataset data/x.csv licence=CC0-1.0 source=... >}}`. There is \
         no `path:`/`url:` any more because the shortcode's own argument already names the \
         file, so this is a move rather than a rename",
    ),
    // --- The academic-publishing cluster, retired 2026-08-03 with the reader-facing
    // cite-this box and the Google Scholar `citation_*` meta it fed. (No quotation marks
    // in this comment on purpose: `quarto_migration_page.rs` scans the block for string
    // literals and chunks them by three, so a quoted phrase here shifts every tuple after
    // it.) Citations themselves are untouched:
    // `bibliography:` and `[@key]` are the load-bearing half and stay. What went is the
    // *outward* record-of-publication layer, which no document outside its own fixture set.
    (
        "front-matter key",
        "doi",
        "it was removed on 2026-08-03 with the appendix and the Google Scholar metadata \
         that were its only readers. A DOI is a link like any other now: write it in the \
         page body, e.g. `[10.5281/zenodo.1825009](https://doi.org/10.5281/zenodo.1825009)`",
    ),
    (
        "front-matter key",
        "links",
        "it was removed on 2026-08-03 with the resource row it rendered under the byline. \
         Write the artefact links in the page body as ordinary Markdown links, which is \
         what a reader clicks either way",
    ),
    (
        "front-matter key",
        "venue",
        "it was removed on 2026-08-03 with the resource row and the Google Scholar \
         `citation_conference_title` it fed. Name the venue in the page body or in \
         `subtitle:`",
    ),
    (
        "front-matter key",
        "award",
        "it was removed on 2026-08-03 with the resource row that rendered its badge. Say it \
         in the page body or in `subtitle:`",
    ),
    (
        "front-matter key",
        "acknowledgments",
        "it was removed on 2026-08-03 with the generated appendix. Acknowledgments are \
         prose: write them as a final section of the document, where you can edit them",
    ),
    (
        "front-matter key",
        "acknowledgements",
        "it was removed on 2026-08-03 with the generated appendix (both spellings were \
         accepted). Acknowledgements are prose: write them as a final section of the \
         document, where you can edit them",
    ),
    (
        "front-matter key",
        "about",
        "it was removed on 2026-07-17 and the landing banner is now `hero:`, which takes \
         a different set of sub-keys (eyebrow / headline / lead / actions), so this is a \
         rewrite rather than a rename",
    ),
    (
        "theorems key",
        "number-within",
        "it was removed when theorem numbers started scoping to a book chapter \
         automatically, so nothing replaces it: delete the key",
    ),
    // --- Visual minimalism pass, 2026-08-03.
    (
        "listing key",
        "categories",
        "it was removed on 2026-08-03: the filter-chip row paid off only on a large \
         archive with a disciplined category vocabulary. Page-level `categories:` still \
         works and still shows as a badge on each card",
    ),
    (
        "callout kind",
        "important",
        "it was removed on 2026-08-03: three kinds cover the distinctions a reader can \
         actually decode. Use `warning` for a consequence, `note` for an aside",
    ),
    (
        "callout kind",
        "caution",
        "it was removed on 2026-08-03: three kinds cover the distinctions a reader can \
         actually decode. Use `warning`",
    ),
];

/// `execute:` sub-keys taliesin honors (document-level cell defaults; see
/// `render::detect_execute_defaults`).
///
/// One key, and that is the whole set on purpose: `echo`/`include` were document-wide
/// defaults for something every real document says per cell (`#| echo:`), so they were
/// retired on 2026-08-02 rather than kept as a second way to say it.
pub(crate) const EXECUTE_KEYS: &[&str] = &["cache"];

/// `listing:` sub-keys taliesin honors (see `site::frontmatter::parse_listing_spec`).
///
/// No `sort:`. A listing is newest-first, which is what all four real listings wrote out
/// longhand; see the `RETIRED_KEYS` entry.
pub(crate) const LISTING_KEYS: &[&str] = &["contents", "id", "type", "max-items"];

/// `hero:` sub-keys taliesin honors (see `site::frontmatter::parse_hero`).
///
/// Text and links only: the banner is type, not a figure. `image`/`image-alt` were retired
/// on 2026-08-02 unused.
pub(crate) const HERO_KEYS: &[&str] = &["eyebrow", "headline", "lead", "actions"];

/// The keys of one `hero.actions:` entry (`{ text, href, primary }`). A typo here used to
/// be silent — the item parses, the button renders with no label or no link — which is the
/// same failure shape the `chapters:` validator exists to prevent, so it warns.
pub(crate) const HERO_ACTION_KEYS: &[&str] = &["text", "href", "primary"];

/// `theorems:` sub-keys taliesin honors: `shared` (draw one counter across several kinds).
///
/// Numbering itself is deliberately not a key: a theorem is numbered, and it scopes to its
/// numbered book chapter automatically, as every float does. `numbered:` carried the
/// policies for opting out of that and was retired on 2026-08-02 with its book-wide
/// `_site.yml` counterpart.
pub(crate) const THEOREM_KEYS: &[&str] = &["shared"];

/// Validate a document's front matter against taliesin's vocabulary: every unknown
/// top-level key, plus every unknown immediate child of the nested `execute:`,
/// `listing:`, and `hero:` blocks. Membership is decided by a real YAML
/// parse (so structure, lists, nested maps, never causes a false positive); each
/// warning is best-effort located (click-to-source) at the offending key's source
/// line. Empty when there is no front matter, it is not a mapping, or it fails to
/// parse (the parse error is reported separately by [`yaml_error`]).
pub fn validate_front_matter(src: &str) -> Vec<Warning> {
    let Some(block) = front_matter_block(src) else {
        return Vec::new();
    };
    if block.trim().is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(block) else {
        return Vec::new();
    };
    let Some(map) = value.as_mapping() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for key in map.keys().filter_map(|k| k.as_str()) {
        if !KNOWN_KEYS.contains(&key) {
            out.push(located_span(
                unknown_key_message("front-matter key", key, KNOWN_KEYS),
                block_key_span(block, key),
            ));
        }
    }
    validate_format_value(map, block, &mut out);
    validate_format_subkeys(map, block, &mut out);
    validate_unsupported_keys(map, block, &mut out);
    validate_page_layout_value(map, block, &mut out);
    validate_date_value(map, block, &mut out);
    validate_image_alt(map, block, &mut out);
    validate_nested(map, "execute", "execute key", EXECUTE_KEYS, block, &mut out);
    validate_nested(map, "hero", "hero key", HERO_KEYS, block, &mut out);
    validate_hero_actions(map, block, &mut out);
    validate_nested(
        map,
        "theorems",
        "theorems key",
        THEOREM_KEYS,
        block,
        &mut out,
    );
    validate_theorem_shared_kinds(map, block, &mut out);
    // `listing:` is one mapping or a sequence of mappings (cv.tmd).
    match map.get("listing") {
        Some(serde_yaml::Value::Mapping(m)) => {
            validate_child_keys(m, "listing", "listing key", LISTING_KEYS, block, &mut out)
        }
        Some(serde_yaml::Value::Sequence(seq)) => {
            for item in seq {
                if let Some(m) = item.as_mapping() {
                    validate_child_keys(m, "listing", "listing key", LISTING_KEYS, block, &mut out);
                }
            }
        }
        _ => {}
    }
    out
}

/// Interpret a raw YAML scalar as a boolean, catching the YAML-1.1 words serde_yaml
/// (which follows YAML 1.2) reads as plain STRINGS — `yes`/`no`/`on`/`off` — alongside
/// canonical `true`/`false` (case-insensitive, tolerant of surrounding quotes). Returns
/// `None` for any non-boolean value (e.g. `echo: fenced`), so a caller keeps its own
/// meaning for that. The single source of the boolean vocabulary shared by the
/// front-matter (`site::frontmatter::bool_field`), cell-option
/// (`render::cell_extract`), toc (`render::fm_extract::detect_toc`), and `_site.yml`
/// readers, so `toc: yes` / `#| echo: no` take effect instead of silently no-oping.
pub(crate) fn yaml_bool_word(s: &str) -> Option<bool> {
    match s
        .trim()
        .trim_matches(['"', '\''])
        .to_ascii_lowercase()
        .as_str()
    {
        "true" | "yes" | "on" => Some(true),
        "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// A `date:` value → the calendar date it names, as `(year, month, day)`. Accepts an
/// un-padded month/day (`2026-5-15`) and ignores any time half (`2026-05-15T09:30:00Z`),
/// since both name one unambiguous day; rejects anything that is not a real date, so
/// `2026-02-30` and `2026-99-99` are `None` rather than plausible-looking output.
///
/// The single source of "is this a date", shared by the three readers of `date:` that
/// each used to answer it themselves at a different strictness: `render::humanize_date`
/// (prints it for people), `site::feed::rfc3339` (stamps the Atom feed) and
/// `site::seo`'s `<lastmod>` (stamps the sitemap). The two machine-facing ones were the
/// loose pair — the sitemap did not check at all, and the feed accepted an un-padded
/// `2026-5-15` and, via a `T` fast-path that returned before any check, the bare word
/// `Thursday`. Callers keep their own FORMATTING (and their own answer for `None`);
/// only the question is shared.
pub(crate) fn calendar_date(date: &str) -> Option<(u32, u32, u32)> {
    // A time half names the same day; `humanize_date` keeps its own `T` rule (it prints
    // a timestamped date verbatim rather than silently dropping the time).
    let d = date.trim();
    let d = d.split_once('T').map_or(d, |(day, _)| day);
    let [y, m, day] = d.split('-').collect::<Vec<_>>()[..] else {
        return None;
    };
    let num = |s: &str, width: usize| -> Option<u32> {
        (!s.is_empty() && s.len() <= width && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    // A 2-digit year is ambiguous, never a date we will stamp into a machine file.
    let year = num(y, 4).filter(|_| y.len() == 4)?;
    let month = num(m, 2).filter(|m| (1..=12).contains(m))?;
    let day = num(day, 2).filter(|d| (1..=days_in_month(year, month)).contains(d))?;
    Some((year, month, day))
}

/// Days in `month` of `year`, Gregorian (so `2026-02-30` is not a date but `2024-02-29`
/// is). `month` is 1-based and already range-checked by [`calendar_date`], its only caller.
fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// `date:` is the one front-matter value read by MACHINES (the sitemap's `<lastmod>`, the
/// Atom feed's `<updated>`), so a value they cannot parse silently vanishes from both while
/// the page still displays it — a green `check` certifying a half-published post. Free text
/// (`date: Spring 2026`) stays legal for display, which is why this reports what is lost
/// rather than calling the value wrong.
/// PA-M13: an `image:` with no `image-alt:` emits `alt=""`, which tells a screen-reader
/// user the image is *decorative* — but a card thumbnail or hero image carries meaning, so
/// the empty alt is an omission rather than a choice. A body `<img>` has been linted for
/// this since the a11y pass (`diagnostics::a11y`); front-matter images were the hole,
/// because by the time they reach the emitted HTML the omission and a deliberate
/// `image-alt: ""` look identical. Here they do not: present-but-empty stays silent, which
/// is the escape hatch for an image that really is decorative.
///
/// Calibrated against the corpus before being written, per the standing constraint: it
/// fires on 4 real pages, all genuine omissions. A grep-based version would also have
/// fired on two `docs/` pages whose `image:` sits inside a YAML *example* in prose — which
/// is exactly why this reads the parsed front matter instead of scanning source lines.
fn validate_image_alt(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    /// A key whose value is a non-empty scalar (so `image:` with nothing after it, which
    /// renders no image at all, is not nagged about its missing alt text).
    fn is_set(v: Option<&serde_yaml::Value>) -> bool {
        v.and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty())
    }
    // "missing alt text" is the needle the diagnostics catalogue already maps to
    // TAL-A11Y-ALT, so this joins the existing family rather than minting a code.
    const HINT: &str = "(set `image-alt:`, or `image-alt: \"\"` if it is purely decorative)";

    if is_set(map.get("image")) && map.get("image-alt").is_none() {
        out.push(located_span(
            format!("social/listing image is missing alt text {HINT}"),
            block_key_span(block, "image"),
        ));
    }
    // No `hero.image` arm: `image`/`image-alt` were retired from `hero:` on 2026-08-02, so a
    // leftover `hero: { image: … }` already draws the retired-key diagnostic. Asking the
    // author to add `image-alt:` to a key that no longer exists would be a second, contrary
    // instruction on the same line.
}

fn validate_date_value(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    let Some(val) = map.get("date").and_then(|v| v.as_str()) else {
        return;
    };
    let val = val.trim().trim_matches(['"', '\'']);
    if val.is_empty() || calendar_date(val).is_some() {
        return;
    }
    out.push(located(
        format!(
            "`date: {val}` isn't a machine-readable date, so it is left out of the sitemap \
             and the Atom feed (the page still shows it) — write `YYYY-MM-DD` to publish it"
        ),
        block_key_line(block, "date"),
    ));
}

/// A `Warning` for `message`, located when `line` is `Some` (file `None` = the
/// previewed doc, the client falls back to its path).
fn located(message: String, line: Option<u32>) -> Warning {
    match line {
        Some(l) => Warning::new(message).at(None, l),
        None => Warning::new(message),
    }
}

/// A `Warning` located at a `[col, end_col)` span, from a `(line, col, end_col)` locator.
/// Used for key-typo diagnostics, whose flagged token IS the front-matter key.
fn located_span(message: String, span: Option<(u32, u32, u32)>) -> Warning {
    match span {
        Some((l, c, e)) => Warning::new(message).at(None, l).span(c, e),
        None => Warning::new(message),
    }
}

/// Validate the immediate children of a single nested mapping block.
fn validate_nested(
    map: &serde_yaml::Mapping,
    parent: &str,
    what: &str,
    allowed: &[&'static str],
    block: &str,
    out: &mut Vec<Warning>,
) {
    if let Some(serde_yaml::Value::Mapping(m)) = map.get(parent) {
        validate_child_keys(m, parent, what, allowed, block, out);
    }
}

fn validate_child_keys(
    m: &serde_yaml::Mapping,
    parent: &str,
    what: &str,
    allowed: &[&'static str],
    block: &str,
    out: &mut Vec<Warning>,
) {
    for key in m.keys().filter_map(|k| k.as_str()) {
        if !allowed.contains(&key) {
            out.push(located_span(
                unknown_key_message(what, key, allowed),
                nested_key_span(block, parent, key),
            ));
        }
    }
}

/// Validate each entry of `hero.actions:`, the list of buttons under a landing banner.
///
/// This vocabulary was unvalidated until 2026-08-02, and unlike a top-level key its typo
/// is *silent and visible*: `hef:` renders a button that goes nowhere, `txt:` renders one
/// with no label, and `check` stays green. That is the same failure shape as a typo'd
/// chapter entry (`site::book`), and it earns the same diagnostic.
///
/// Located at the misspelled key when the `actions:` block is a flow-style list on one
/// line (the form every real page uses, so the span usually lands); falls back to the
/// `hero:` key otherwise.
fn validate_hero_actions(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    let Some(serde_yaml::Value::Mapping(hero)) = map.get("hero") else {
        return;
    };
    let Some(serde_yaml::Value::Sequence(actions)) = hero.get("actions") else {
        return;
    };
    for item in actions {
        let Some(m) = item.as_mapping() else { continue };
        for key in m.keys().filter_map(|k| k.as_str()) {
            if !HERO_ACTION_KEYS.contains(&key) {
                out.push(located_span(
                    unknown_key_message("hero action key", key, HERO_ACTION_KEYS),
                    block_key_span(block, key).or_else(|| block_key_span(block, "hero")),
                ));
            }
        }
    }
}

/// Validate the *values* of `theorems: shared:`, which name theorem kinds rather than keys.
///
/// Only the key was ever checked, so the list itself fell through in silence: `shared:
/// [theorem, lemna]` drew two separate counters with a clean `check`, and
/// `corpus/refs/theorems-shared.tmd` went on declaring `proposition` for four days after
/// that kind was retired. This is the [`crate::render::RETIRED_DIV_CLASSES`]
/// silent-fallthrough hazard one vocabulary over, and it is answered the same way: a
/// retired kind gets its registered note (read through
/// [`crate::render::retired_div_note`], so the two diagnostics cannot drift apart),
/// anything else gets the usual did-you-mean.
fn validate_theorem_shared_kinds(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    let Some(serde_yaml::Value::Mapping(theorems)) = map.get("theorems") else {
        return;
    };
    let Some(serde_yaml::Value::Sequence(shared)) = theorems.get("shared") else {
        return;
    };
    for kind in shared.iter().filter_map(|v| v.as_str()) {
        if crate::render::THEOREM_KINDS.contains(&kind) {
            continue;
        }
        let msg = match crate::render::retired_div_note(kind) {
            Some(note) => format!("unknown theorem kind `{kind}`: {note}"),
            None => match closest(kind, crate::render::THEOREM_KINDS) {
                Some(near) => format!("unknown theorem kind `{kind}` (did you mean `{near}`?)"),
                None => format!("unknown theorem kind `{kind}`"),
            },
        };
        out.push(located_span(
            msg,
            block_key_span(block, "shared").or_else(|| block_key_span(block, "theorems")),
        ));
    }
}

/// Common non-HTML output targets (Pandoc/Quarto) an author might carry over. Taliesin
/// renders HTML only (the deck engine included), so any of these silently produces HTML
/// instead of the requested format — worth a warning rather than a clean `check`.
const NON_HTML_FORMATS: &[&str] = &[
    "pdf", "typst", "docx", "latex", "beamer", "epub", "pptx", "odt", "rtf", "jats", "docbook",
];

/// Validate the top-level `format:` value (string, block-mapping keyed by the spelling,
/// or sequence). Flags the dropped legacy deck spelling (`revealjs` / `*-revealjs`),
/// whose edit distance to `deck` is too large for the generic did-you-mean, and any
/// known non-HTML output target ([`NON_HTML_FORMATS`]) that Taliesin can't produce — both
/// otherwise render a plain HTML page with no signal.
/// `format:` sub-keys (`format:\n  deck:\n    transition: fade`) are read by NOTHING, so
/// they must warn instead of certifying as supported on a green check — the `csl:` rule.
/// `format:` takes a NAME (`html`/`deck`), which is all the guide teaches.
///
/// This used to be deliberately un-linted "because an extension owns them", but no such
/// mechanism exists: `_extensions/` is a theme-CSS lookup (`render/theme.rs`), `DocFormat`
/// has two built-in variants, and `deck.rs` hardcodes its engine init reading no sub-key.
/// The one shape that *appeared* to work — `format: html: toc:` — did so only because
/// `detect_toc` trimmed before matching, which also let a `toc:` under `hero:` set the
/// document's TOC; that scan is now top-level-only, so every sub-key is uniformly inert
/// and this warning is true rather than a lie.
fn validate_format_subkeys(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    // `format: deck` (a bare name) is a String, not a Mapping: nothing to lint.
    let Some(serde_yaml::Value::Mapping(fmt)) = map.get("format") else {
        return;
    };
    for opts in fmt.values() {
        // `format:\n  deck:` with no options parses as Null, not an empty Mapping.
        let serde_yaml::Value::Mapping(opts) = opts else {
            continue;
        };
        for key in opts.keys().filter_map(|k| k.as_str()) {
            // A sub-key that names a real top-level key is the likely mistake (the Quarto
            // shape), so point at where it belongs rather than only rejecting it.
            // Worded to stay distinct from `validate_format_value`'s "unknown format
            // `revealjs`", which is about the format NAME — an "unknown format sub-key"
            // phrasing reads as (and substring-matches) that different diagnostic.
            let msg = if KNOWN_KEYS.contains(&key) {
                format!("`format:` sub-key `{key}` is ignored (did you mean a top-level `{key}:`?)")
            } else {
                format!("`format:` sub-key `{key}` is ignored (nothing reads `format:` sub-keys)")
            };
            out.push(located_span(msg, nested_key_span(block, "format", key)));
        }
    }
}

fn validate_format_value(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    let Some(fmt) = map.get("format") else {
        return;
    };
    // The declared format NAME(s): the scalar, the block-mapping keys (`html:`/`deck:`),
    // or the sequence entries.
    let names: Vec<String> = match fmt {
        serde_yaml::Value::String(s) => vec![s.clone()],
        serde_yaml::Value::Mapping(m) => m
            .keys()
            .filter_map(|k| k.as_str().map(str::to_string))
            .collect(),
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    };
    let line = block_key_line(block, "format");
    for name in &names {
        let n = name.trim().trim_matches(['"', '\'']);
        if n == "revealjs" || n.ends_with("-revealjs") {
            out.push(located(
                format!("unknown format `{n}` (did you mean `deck`?)"),
                line,
            ));
        } else if NON_HTML_FORMATS.contains(&n) {
            out.push(located(
                format!("format `{n}` is not supported (Taliesin renders HTML only)"),
                line,
            ));
        }
    }
}

/// `page-layout:` only honors `full` (widen the reading column); every other value —
/// Quarto's `article` (its default), `custom`, … — is silently ignored. Warn on those so
/// a migration leftover surfaces instead of the author wondering why the layout never
/// changed. A hard gate, so the corpus carries only `full` (or omits the key).
/// Warn on a key taliesin RECOGNIZES but does not honor (see [`UNSUPPORTED_KEYS`]).
///
/// Nothing reads `csl:`. Reference formatting is hardcoded IEEE-numeric and the `.csl`
/// file's content is never parsed, yet the key was advertised on five surfaces, so an
/// author who wrote `csl: apa.csl` got a clean check, IEEE output, and no signal at all.
///
/// This lives here rather than in `diagnostics` on purpose: `diagnostics` is check-only
/// (nothing under `serve/` calls it), and the author reads the preview. Sitting on the
/// render path means preview and `check` say the same thing, like the `page-layout` lint
/// right below. The message wording is load-bearing: `codes::classify` keys the
/// `TAL-FM-UNSUPPORTED` code off "is recognized but not supported".
///
/// Carries no "did you mean" hint on purpose: there is no replacement, and
/// `codes::extract_suggestion` would lift one into a structured fix an agent would apply.
fn validate_unsupported_keys(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    for key in UNSUPPORTED_KEYS {
        if map.get(key).is_none() {
            continue;
        }
        out.push(located_span(
            format!(
                "`{key}:` is recognized but not supported, so it has no effect: references \
                 always render in the built-in IEEE style (remove the key, or the citations \
                 will not match the style you asked for)"
            ),
            block_key_span(block, key),
        ));
    }
}

fn validate_page_layout_value(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    let Some(val) = map.get("page-layout").and_then(|v| v.as_str()) else {
        return;
    };
    let val = val.trim().trim_matches(['"', '\'']);
    if val.is_empty() || val == "full" {
        return;
    }
    out.push(located(
        format!(
            "`page-layout: {val}` is ignored — Taliesin uses the reading-width column by \
             default and only `full` widens it (a Quarto leftover?)"
        ),
        block_key_line(block, "page-layout"),
    ));
}

/// The 1-based source line of the front matter's top-level `bibliography:` key, if
/// present on its own line. Lets `.bib` diagnostics (a missing file, a duplicate
/// key) point at the declaration instead of rendering as an unlocated warning.
pub(crate) fn bibliography_line(src: &str) -> Option<u32> {
    block_key_line(front_matter_block(src)?, "bibliography")
}

/// The 1-based SOURCE-FILE line of a top-level front-matter key (best-effort). The
/// block starts on the file line after the opening `---`, so block line index `i` is
/// file line `i + 2`. `None` if the key is not on its own line (e.g. a flow mapping).
///
/// `pub(crate)` so a `diagnostics` validator can locate a front-matter key on the same
/// click-to-source channel, rather than keeping a second copy of this offset rule.
pub(crate) fn block_key_line(block: &str, key: &str) -> Option<u32> {
    block_key_span(block, key).map(|(l, _, _)| l)
}

/// `(line, col, end_col)` of a top-level `key:`, all 1-based (see [`block_key_line`] for the
/// line rule). Top-level keys are unindented, so `col` is 1 and `end_col` is `1 + key.len()`.
/// Columns are Unicode-scalar counts; a front-matter key is ASCII, so scalar == byte == UTF-16.
pub(crate) fn block_key_span(block: &str, key: &str) -> Option<(u32, u32, u32)> {
    block.lines().enumerate().find_map(|(i, line)| {
        let t = line.trim_start();
        (line.len() == t.len() && key_matches(t, key))
            .then(|| (i as u32 + 2, 1, 1 + key.chars().count() as u32))
    })
}

/// `(line, col, end_col)` of a nested child `key` under `parent:`, all 1-based. `col` follows
/// the line's indentation plus an optional `- ` list prefix. Indentation is ASCII, so the
/// scalar column equals the byte/UTF-16 column.
fn nested_key_span(block: &str, parent: &str, key: &str) -> Option<(u32, u32, u32)> {
    let mut in_block = false;
    for (i, line) in block.lines().enumerate() {
        let t = line.trim_start();
        let at_top = line.len() == t.len();
        if !in_block {
            if at_top && key_matches(t, parent) {
                in_block = true;
            }
            continue;
        }
        if at_top {
            break; // dedent ends the parent block
        }
        let indent = line.len() - t.len();
        let (prefix, body) = match t.strip_prefix("- ") {
            Some(rest) => (
                2 + (rest.len() - rest.trim_start().len()),
                rest.trim_start(),
            ),
            None => (0, t),
        };
        if key_matches(body, key) {
            let col = indent as u32 + prefix as u32 + 1;
            return Some((i as u32 + 2, col, col + key.chars().count() as u32));
        }
    }
    None
}

/// Does `text` start with `key` immediately followed by `:` (a YAML key)?
fn key_matches(text: &str, key: &str) -> bool {
    text.strip_prefix(key)
        .is_some_and(|rest| rest.starts_with(':'))
}

/// If the document has front matter that is present but not valid YAML, return the
/// parse-error message and its 1-based line in the SOURCE FILE. The front-matter
/// block starts on the line after the opening `---`, so a YAML line `L` is file
/// line `L + 1`; `serde_yaml` locations are 1-based, and we fall back to the fence
/// line when the error carries none. `None` when there is no front matter or it
/// parses cleanly. Powers a located, click-to-source diagnostic in the dev server.
pub fn yaml_error(src: &str) -> Option<(String, u32)> {
    let block = front_matter_block(src)?;
    if block.trim().is_empty() {
        return None;
    }
    match serde_yaml::from_str::<serde_yaml::Value>(block) {
        Ok(_) => None,
        Err(e) => {
            let line = e.location().map(|l| l.line() as u32 + 1).unwrap_or(1);
            Some((format!("front matter is not valid YAML: {e}"), line))
        }
    }
}

/// The leading `---` ... `---`/`...` block of a document, without the fences.
/// `None` if the source doesn't open with a front-matter fence. The one canonical
/// front-matter splitter (BOM- and `...`-terminator-aware); the site parser and the
/// shortcode/extension scanner reuse it so every path agrees on edge cases.
pub(crate) fn front_matter_block(src: &str) -> Option<&str> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let first = src.split_inclusive('\n').next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let after = first.len();
    let mut pos = after;
    for line in src[after..].split_inclusive('\n') {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            return Some(&src[after..pos]);
        }
        pos += line.len();
    }
    None
}

/// The candidate within edit distance 2 of `key` (a "did you mean"), or `None`.
/// Shared by the front-matter linter, the project-config validator, and the CLI's
/// unknown-command suggestion (re-exported as `crate::closest`).
pub fn closest(key: &str, candidates: &[&'static str]) -> Option<&'static str> {
    closest_of(key, candidates.iter().copied())
}

/// [`closest`] over borrowed candidates whose lifetime is not `'static` — a directory
/// listing, say, where the names are owned `String`s read at runtime. Same distance rule,
/// deliberately: a "did you mean" that is stricter in one place than another teaches the
/// reader a threshold that is not real. `closest` is the `&'static` convenience over this.
pub fn closest_of<'a>(key: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    candidates
        .into_iter()
        .map(|k| (levenshtein(key, k), k))
        .filter(|&(d, _)| d > 0 && d <= 2)
        .min_by_key(|&(d, _)| d)
        .map(|(_, k)| k)
}

/// The retirement note for `(scope, key)` in [`RETIRED_KEYS`], or `None` if it was never
/// retired (or was retired under a different scope). `pub` (rather than widening
/// `RETIRED_KEYS` itself) so a test outside this crate can assert a retired construct is
/// actually registered — the exact failure mode that produces silence instead of a
/// diagnostic when a scope string typo slips through.
pub fn retired_note(scope: &str, key: &str) -> Option<&'static str> {
    RETIRED_KEYS
        .iter()
        .find(|(s, k, _)| *s == scope && *k == key)
        .map(|(_, _, note)| *note)
}

/// Build an "unknown <what> `<key>`" message, appending "(did you mean `X`?)" when a
/// known candidate is within edit distance 2. The single message format shared by the
/// front-matter, cell-option, callout, and nested-config validators.
pub(crate) fn unknown_key_message(what: &str, key: &str, candidates: &[&'static str]) -> String {
    // A key we removed is still unknown, so the classified prefix stays (`codes::classify`
    // resolves TAL-FM-KEY off it) — but it earns the reason instead of a rename hint.
    if let Some((_, _, note)) = RETIRED_KEYS
        .iter()
        .find(|(scope, retired, _)| *scope == what && *retired == key)
    {
        return format!("unknown {what} `{key}`: {note}");
    }
    match closest(key, candidates) {
        Some(s) => format!("unknown {what} `{key}` (did you mean `{s}`?)"),
        None => format!("unknown {what} `{key}`"),
    }
}

/// Plain Levenshtein edit distance (two-row DP).
pub(crate) fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msgs(src: &str) -> Vec<String> {
        validate_front_matter(src)
            .into_iter()
            .map(|w| w.message)
            .collect()
    }

    #[test]
    fn flags_top_level_typo_with_suggestion_and_location() {
        let w = validate_front_matter("---\ntreme: darkly\ntitle: X\n---\n\nbody\n");
        assert_eq!(w.len(), 1, "got: {w:?}");
        assert_eq!(
            w[0].message,
            "unknown front-matter key `treme` (did you mean `theme`?)"
        );
        assert_eq!(w[0].line, Some(2), "`treme` is on file line 2");
    }

    /// `number-within` was removed when theorem numbers started scoping to a book chapter
    /// automatically. A doc that still carries it must be TOLD, not silently ignored: it
    /// is now an unknown `theorems:` key, which is the whole point of validating sub-keys
    /// (the same hazard `csl:` was fixed for — a key that reads as honored and does
    /// nothing).
    #[test]
    fn theorems_number_within_is_gone_and_says_so() {
        // Asserted against `validate_front_matter` directly, not `msgs`: `msgs` drops
        // `w.line`, and the LINE is the migration story — a located warning is what lets
        // the dev panel jump an author to the dead key. This is also the only pin on
        // `nested_key_line` (the sibling test pins `block_key_line`, the top-level one).
        let w = validate_front_matter("---\ntheorems:\n  number-within: chapter\n---\n\nbody\n");
        assert_eq!(w.len(), 1, "got: {w:?}");
        assert_eq!(
            w[0].message,
            "unknown theorems key `number-within`: it was removed when theorem numbers \
             started scoping to a book chapter automatically, so nothing replaces it: \
             delete the key"
        );
        assert_eq!(w[0].line, Some(3), "`number-within` is on file line 3");
    }

    #[test]
    fn theorems_shared_is_the_whole_block() {
        assert!(
            msgs("---\ntheorems:\n  shared: [theorem, lemma]\n---\n").is_empty(),
            "`shared:` is the one honored sub-key"
        );
    }

    /// `numbered:` was retired on 2026-08-02 with the book-wide `_site.yml theorems:`
    /// policy. A leftover one must explain itself rather than be answered with `shared`.
    #[test]
    fn theorems_numbered_is_retired_and_says_so() {
        for value in ["false", "true", "unless-unique"] {
            let m = msgs(&format!("---\ntheorems:\n  numbered: {value}\n---\n"));
            let msg = m
                .iter()
                .find(|x| x.contains("`numbered`"))
                .unwrap_or_else(|| panic!("`numbered: {value}` drew no diagnostic: {m:?}"));
            assert!(
                msg.contains("removed on 2026-08-02") && msg.contains("`shared:`"),
                "must say it went and what survives: {msg}"
            );
            assert!(!msg.contains("did you mean"), "not a rename hint: {msg}");
        }
    }

    /// `shared:` names theorem KINDS, and only its *key* was ever validated — the values
    /// were not. `shared: [theorem, lemna]` silently drew two separate counters, and
    /// `corpus/refs/theorems-shared.tmd` carried `proposition` for four days after that kind
    /// was retired with `check` still reporting "no problems found". Same silent
    /// fallthrough `RETIRED_DIV_CLASSES` exists to prevent, one vocabulary over.
    #[test]
    fn theorems_shared_validates_its_kind_values() {
        let m = msgs("---\ntheorems:\n  shared: [theorem, lemna]\n---\n");
        assert!(
            m.iter()
                .any(|w| w.contains("unknown theorem kind `lemna`") && w.contains("lemma")),
            "a typo'd kind warns with did-you-mean: {m:?}"
        );

        // A kind retired on 2026-08-03 explains itself instead of being answered with a
        // near-miss rename, exactly as the div-class register does.
        for kind in ["proposition", "example", "remark"] {
            let m = msgs(&format!(
                "---\ntheorems:\n  shared: [theorem, {kind}]\n---\n"
            ));
            let msg = m
                .iter()
                .find(|w| w.contains(&format!("`{kind}`")))
                .unwrap_or_else(|| panic!("`shared: [{kind}]` drew no diagnostic: {m:?}"));
            assert!(
                msg.contains("removed on 2026-08-03"),
                "must say it went and what survives: {msg}"
            );
        }

        // The live set stays clean, so this cannot pass by warning on everything.
        assert!(
            msgs("---\ntheorems:\n  shared: [theorem, lemma, corollary, definition, proof]\n---\n")
                .is_empty(),
            "every live kind must validate silently"
        );
    }

    #[test]
    fn theorems_block_is_validated() {
        assert!(
            msgs("---\ntheorems:\n  shared: [theorem, lemma]\n---\n").is_empty(),
            "a valid theorems block must not warn"
        );
        let m = msgs("---\ntheorems:\n  shard: [theorem]\n---\n");
        assert!(
            m.iter()
                .any(|w| w.contains("unknown theorems key `shard`") && w.contains("shared")),
            "a typo'd child key warns with did-you-mean: {m:?}"
        );
    }

    #[test]
    fn flags_unknown_execute_child() {
        // `cache` is the only `execute:` sub-key left, so a typo of IT is what still draws a
        // did-you-mean.
        let m = msgs("---\ntitle: X\nexecute:\n  cach: true\n---\n");
        assert_eq!(
            m,
            vec!["unknown execute key `cach` (did you mean `cache`?)"]
        );
    }

    /// `echo:`/`include:` were retired from `execute:` on 2026-08-02. A leftover one must
    /// say so and name the per-cell form, not be answered with a rename to `cache`.
    #[test]
    fn a_retired_execute_child_explains_itself() {
        for (key, needle) in [("echo", "`#| echo: false`"), ("include", "no successor")] {
            let m = msgs(&format!("---\ntitle: X\nexecute:\n  {key}: false\n---\n"));
            let msg = m
                .iter()
                .find(|x| x.contains(&format!("`{key}`")))
                .unwrap_or_else(|| panic!("no diagnostic for retired `{key}`: {m:?}"));
            assert!(
                msg.contains("removed on 2026-08-02") && msg.contains(needle),
                "`{key}` must say it was removed and what to write: {msg}"
            );
            assert!(!msg.contains("did you mean"), "not a rename hint: {msg}");
        }
    }

    #[test]
    fn flags_unknown_listing_child_in_a_mapping_and_a_sequence() {
        let m = msgs("---\ntitle: X\nlisting:\n  contents: posts\n  max-itemz: 3\n---\n");
        assert_eq!(
            m,
            vec!["unknown listing key `max-itemz` (did you mean `max-items`?)"]
        );
        // A sequence of listings (cv.tmd shape) validates each item.
        let m2 = msgs("---\ntitle: X\nlisting:\n  - contents: a\n    sort-uii: false\n---\n");
        assert_eq!(m2, vec!["unknown listing key `sort-uii`"]);
    }

    /// The listing category-filter chips were deleted 2026-08-03 (visual minimalism
    /// pass): they paid off only on a blog with many posts AND disciplined category
    /// vocabulary — the linter existed precisely because that discipline does not hold
    /// by default. Page-level `categories:` and the card badges SURVIVE; only
    /// `listing.categories` is retired, and it must be recognized in ITS OWN scope
    /// (`listing key`), not answered with the generic did-you-mean.
    #[test]
    fn the_listing_categories_subkey_is_retired_but_page_categories_live() {
        assert!(
            !LISTING_KEYS.contains(&"categories"),
            "`listing.categories` should be retired"
        );
        assert!(
            KNOWN_KEYS.contains(&"categories"),
            "page-level `categories:` must SURVIVE — only the listing sub-key is retired"
        );
        let m = msgs("---\ntitle: X\nlisting:\n  contents: posts\n  categories: true\n---\n");
        let msg = m
            .iter()
            .find(|x| x.contains("`categories`"))
            .unwrap_or_else(|| panic!("no diagnostic for retired `listing.categories`: {m:?}"));
        assert!(
            msg.starts_with("unknown listing key `categories`"),
            "must be scoped to `listing key`, not the generic front-matter scope: {msg}"
        );
        assert!(
            msg.contains("removed on 2026-08-03") && msg.contains("badge"),
            "must say it went and that the badge survives: {msg}"
        );
        assert!(!msg.contains("did you mean"), "not a rename hint: {msg}");
    }

    #[test]
    fn flags_unknown_hero_children() {
        let h = msgs("---\ntitle: X\nhero:\n  headlin: Hi\n---\n");
        assert_eq!(
            h,
            vec!["unknown hero key `headlin` (did you mean `headline`?)"]
        );
    }

    #[test]
    fn retired_about_key_warns_as_unknown() {
        // `about:` was removed (superseded by `hero:`); a stale author config should now
        // warn that the key is unknown, not be silently accepted.
        let a = msgs("---\ntitle: X\nabout:\n  template: jolla\n---\n");
        assert!(
            a.iter()
                .any(|m| m.contains("unknown front-matter key") && m.contains("about")),
            "a stale `about:` should warn now that the feature is gone, got {a:?}"
        );
    }

    #[test]
    fn a_retired_key_reads_as_a_removal_not_as_a_misspelling() {
        // Removed vocabulary and a typo used to produce the same sentence, so an author
        // carrying last month's front matter was told only that their key was unrecognized.
        let a = msgs("---\ntitle: X\nabout:\n  template: jolla\n---\n");
        assert!(
            a.iter()
                .any(|m| m.contains("`about`") && m.contains("removed") && m.contains("hero")),
            "a removed key must name its successor: {a:?}"
        );
        // Retirement is scoped to where the key lived: `number-within` was a `theorems:`
        // sub-key, never a top-level one.
        let n = msgs("---\ntheorems:\n  number-within: chapter\n---\n");
        assert!(
            n.iter()
                .any(|m| m.contains("`number-within`") && m.contains("removed")),
            "a retired sub-key must be recognized in its own scope: {n:?}"
        );
        // NEVER phrased as a did-you-mean, even though `about:` has a successor:
        // `codes::extract_suggestion` lifts that exact phrase into a structured fix an
        // agent applies mechanically, and `hero:` takes different sub-keys, so a blind
        // rename would produce a document that is wrong in a new way.
        for m in a.iter().chain(n.iter()) {
            assert!(
                !m.contains("did you mean `"),
                "no machine-applied rename: {m}"
            );
        }
        // The classified prefix has to survive, or `codes::classify` stops resolving these
        // to TAL-FM-KEY (it keys off this substring).
        assert!(
            a.iter().any(|m| m.starts_with("unknown front-matter key")),
            "diagnostic-code classification keys off this prefix: {a:?}"
        );
        // And an ordinary typo must still get its suggestion: the registry adds a case, it
        // does not replace the did-you-mean path.
        let t = msgs("---\ntreme: dark\n---\n");
        assert!(
            t.iter().any(|m| m.contains("did you mean `theme`")),
            "a real typo still gets its suggestion: {t:?}"
        );
    }

    #[test]
    fn clean_doc_with_nested_blocks_has_no_warnings() {
        let w = validate_front_matter(
            "---\ntitle: X\ntoc: true\nexecute:\n  cache: true\nlisting:\n  contents: posts\n  type: grid\n---\n\nx\n",
        );
        assert!(w.is_empty(), "got: {w:?}");
    }

    /// `format:` sub-keys are read by NOTHING, so they must warn rather than certify as
    /// supported — the `csl:` rule (a key that reads as honored and does nothing is the
    /// bug). This REPLACES `format_subkeys_are_not_linted`, whose stated rationale ("an
    /// extension owns them") was false: `_extensions/` is only a theme-CSS mechanism
    /// (`render/theme.rs`), `DocFormat` has exactly two built-in variants, and the deck
    /// engine hardcodes its init, reading no sub-key at all.
    #[test]
    fn format_subkeys_warn_because_nothing_reads_them() {
        let w =
            validate_front_matter("---\ntitle: X\nformat:\n  deck:\n    transition: fade\n---\n");
        assert_eq!(w.len(), 1, "got: {w:?}");
        assert_eq!(
            w[0].message,
            "`format:` sub-key `transition` is ignored (nothing reads `format:` sub-keys)"
        );
        assert_eq!(w[0].line, Some(5), "located on the sub-key's own line");
    }

    /// A sub-key that names a real TOP-LEVEL key is the likely mistake (the Quarto shape),
    /// so say where it belongs instead of just rejecting it.
    #[test]
    fn a_format_subkey_that_is_a_top_level_key_says_where_it_belongs() {
        let w = validate_front_matter("---\ntitle: X\nformat:\n  html:\n    toc: true\n---\n");
        assert_eq!(w.len(), 1, "got: {w:?}");
        assert_eq!(
            w[0].message,
            "`format:` sub-key `toc` is ignored (did you mean a top-level `toc:`?)"
        );
    }

    #[test]
    fn a_bare_format_name_never_warns() {
        // `format: deck` / `format: html` (the documented form) has no sub-keys at all.
        assert!(validate_front_matter("---\ntitle: X\nformat: deck\n---\n").is_empty());
        assert!(validate_front_matter("---\ntitle: X\nformat: html\n---\n").is_empty());
        // An empty format block is not a sub-key either.
        assert!(validate_front_matter("---\ntitle: X\nformat:\n  deck:\n---\n").is_empty());
    }

    #[test]
    fn revealjs_format_value_warns_with_did_you_mean() {
        // `format: revealjs` was the dropped legacy deck spelling. Its edit distance to
        // `deck` is too large for the generic did-you-mean, so name the migration
        // explicitly rather than silently rendering a plain HTML page.
        let m = msgs("---\nformat: revealjs\ntitle: T\n---\n");
        assert!(
            m.iter()
                .any(|w| w.contains("unknown format `revealjs`")
                    && w.contains("did you mean `deck`")),
            "expected a revealjs->deck did-you-mean, got {m:?}"
        );
    }

    #[test]
    fn revealjs_format_value_is_located() {
        let w = validate_front_matter("---\ntitle: T\nformat: revealjs\n---\n");
        let hit = w
            .iter()
            .find(|w| w.message.contains("unknown format `revealjs`"))
            .expect("revealjs warning");
        assert_eq!(hit.line, Some(3), "`format:` is on file line 3");
    }

    #[test]
    fn revealjs_format_value_warns_in_block_and_sequence_forms() {
        // Block form: `format:` mapping keyed by the dropped spelling.
        let block = msgs("---\nformat:\n  revealjs:\n    incremental: true\n---\n");
        assert!(
            block
                .iter()
                .any(|w| w.contains("unknown format `revealjs`")),
            "block-form revealjs warns: {block:?}"
        );
        // An extension variant `<name>-revealjs` is dropped too.
        let variant = msgs("---\nformat: acme-revealjs\n---\n");
        assert!(
            variant
                .iter()
                .any(|w| w.contains("unknown format `acme-revealjs`") && w.contains("`deck`")),
            "*-revealjs variant warns: {variant:?}"
        );
    }

    #[test]
    fn non_html_format_values_warn_html_only() {
        // A carried-over non-HTML target (pdf/typst/docx/…) silently renders HTML; flag
        // it, located at the `format:` line, so `check` doesn't certify it green.
        for fmt in ["pdf", "typst", "docx", "latex", "beamer", "epub"] {
            let w = validate_front_matter(&format!("---\ntitle: T\nformat: {fmt}\n---\n"));
            let hit = w
                .iter()
                .find(|w| w.message.contains(&format!("format `{fmt}`")))
                .unwrap_or_else(|| panic!("expected an HTML-only warning for `{fmt}`: {w:?}"));
            assert!(hit.message.contains("HTML only"), "{}", hit.message);
            assert_eq!(hit.line, Some(3), "`format:` is on file line 3");
        }
        // The block-mapping form (`format:\n  pdf:\n    …`) is caught too.
        let block = msgs("---\nformat:\n  pdf:\n    toc: true\n---\n");
        assert!(
            block
                .iter()
                .any(|w| w.contains("format `pdf`") && w.contains("HTML only")),
            "block-form non-HTML format warns: {block:?}"
        );
    }

    #[test]
    fn deck_format_value_is_not_flagged() {
        assert!(
            !msgs("---\nformat: deck\n---\n")
                .iter()
                .any(|w| w.contains("unknown format")),
            "`format: deck` is the accepted spelling"
        );
        // Neither is a normal HTML page or a block-form html deck.
        assert!(
            !msgs("---\nformat: html\n---\n")
                .iter()
                .any(|w| w.contains("unknown format"))
        );
        assert!(
            !msgs("---\nformat:\n  deck:\n    incremental: true\n---\n")
                .iter()
                .any(|w| w.contains("unknown format")),
            "block-form deck is accepted"
        );
    }

    #[test]
    fn quarto_page_layout_value_warns_but_full_does_not() {
        // A recognized key carrying a Quarto-only value Taliesin silently ignores warns, so
        // a migration leftover surfaces instead of mystifying the author when nothing changes.
        let m = msgs("---\ntitle: X\npage-layout: article\n---\n");
        assert!(
            m.iter()
                .any(|w| w.contains("`page-layout: article`") && w.contains("ignored")),
            "page-layout: article must warn: {m:?}"
        );
        // The one honored value (and an absent key) stay silent.
        assert!(
            !msgs("---\ntitle: X\npage-layout: full\n---\n")
                .iter()
                .any(|w| w.contains("page-layout")),
            "page-layout: full is honored"
        );
        assert!(
            !msgs("---\ntitle: X\n---\n")
                .iter()
                .any(|w| w.contains("page-layout"))
        );
    }

    /// A `date:` we cannot parse is dropped from `<lastmod>`/`<updated>` rather than
    /// shipped invalid — but dropping it silently is the same "green check, wrong output"
    /// failure one layer down, so the value lints like `page-layout` does. `date:` is the
    /// one front-matter key whose value is read by *machines* (sitemap, Atom), which is
    /// why it earns a value rule when other free-text keys don't.
    #[test]
    fn an_unparseable_date_warns_but_a_real_one_does_not() {
        for bad in ["May 15, 2026", "Spring 2026", "2026-99-99", "Thursday"] {
            let m = msgs(&format!("---\ntitle: X\ndate: \"{bad}\"\n---\n"));
            assert!(
                m.iter().any(|w| w.contains("date") && w.contains(bad)),
                "date: {bad} must warn: {m:?}"
            );
        }
        // An un-padded date is normalized everywhere it is read, so it publishes fine and
        // warning about it would be noise: the rule reports what is LOST, not what is odd.
        for good in ["2026-05-15", "2026-05-15T09:30:00Z", "2026-5-15"] {
            let m = msgs(&format!("---\ntitle: X\ndate: \"{good}\"\n---\n"));
            assert!(
                !m.iter().any(|w| w.contains("date")),
                "date: {good} is valid: {m:?}"
            );
        }
        // Absent stays silent: `date:` is optional.
        assert!(
            !msgs("---\ntitle: X\n---\n")
                .iter()
                .any(|w| w.contains("date"))
        );
    }

    /// The diagnostic must carry a line, like every other front-matter rule — an unlocated
    /// warning is the exact Quarto flaw D53 critiques.
    #[test]
    fn the_date_warning_is_located() {
        let ws = validate_front_matter("---\ntitle: X\nsubtitle: S\ndate: Thursday\n---\n");
        let w = ws
            .iter()
            .find(|w| w.message.contains("date"))
            .expect("date warning");
        assert_eq!(w.line, Some(4), "date: is on line 4: {w:?}");
    }

    /// The point of warning on `csl:` is that a no-op key should not be silent, and the
    /// author reads the *preview*, not a `check` run. So the rule has to live on the
    /// render path (`validate_front_matter`) like the `page-layout` lint, not only in the
    /// check-only `diagnostics` module, which nothing under `serve/` calls.
    #[test]
    fn csl_warns_on_the_render_path_so_the_preview_is_not_silent() {
        let src = "---\ntitle: T\nbibliography: refs.bib\ncsl: apa.csl\n---\n\nBody.\n";
        let ws = validate_front_matter(src);
        let w = ws
            .iter()
            .find(|w| w.message.contains("is recognized but not supported"))
            .unwrap_or_else(|| panic!("the render path warns on `csl:`: {ws:?}"));
        assert_eq!(
            w.line,
            Some(4),
            "located at the `csl:` line, where the fix (deleting it) belongs: {w:?}"
        );
    }

    /// What the warning must actually say. It names the key, says what happens instead
    /// (IEEE), and offers no "did you mean": there is no replacement, and
    /// `codes::extract_suggestion` would lift one into a structured fix an agent applies.
    #[test]
    fn the_csl_warning_names_the_key_and_offers_no_phantom_fix() {
        let src = "---\ntitle: T\nbibliography: refs.bib\ncsl: apa.csl\n---\n\nBody.\n";
        let ws: Vec<_> = validate_front_matter(src)
            .into_iter()
            .filter(|w| w.message.contains("is recognized but not supported"))
            .collect();
        assert_eq!(ws.len(), 1, "exactly one warning for the inert key: {ws:?}");
        let m = &ws[0].message;
        assert!(m.contains("csl"), "names the key: {m}");
        assert!(m.contains("IEEE"), "says what happens instead: {m}");
        assert!(!m.contains("did you mean"), "not a typo, so no fix: {m}");
    }

    #[test]
    fn a_front_matter_without_csl_stays_clean() {
        let unsupported = |src: &str| {
            validate_front_matter(src)
                .into_iter()
                .any(|w| w.message.contains("is recognized but not supported"))
        };
        assert!(
            !unsupported("---\ntitle: T\nbibliography: refs.bib\n---\n\nBody.\n"),
            "no `csl:`, no warning"
        );
        // `css` is edit distance 1 from `csl` and is a real supported key: the lint keys
        // off the parsed mapping, not a substring, so it must not be mistaken for it.
        assert!(
            !unsupported("---\ntitle: T\ncss: extra.css\n---\n\nBody.\n"),
            "`css` is not `csl`"
        );
        assert!(
            !unsupported("# No front matter\n"),
            "no front matter, nothing to validate"
        );
    }

    #[test]
    fn csl_stays_recognized_because_dropping_it_would_mis_suggest_css() {
        // `csl:` is inert (nothing reads the value; references always render in the
        // built-in IEEE style), so the tempting cleanup is to drop it from KNOWN_KEYS and
        // let the unknown-key lint speak. It stays recognized anyway: `csl` names a real
        // thing an author brought from another tool, and "recognized, not honored" (via
        // `diagnostics::csl_recognized_but_unsupported`) is the honest answer where a bare
        // unknown-key warning is not.
        //
        // This was ALSO argued from `css` being edit distance 1 away, which would have made
        // the did-you-mean tell the author to rename a citation-style key to a stylesheet
        // key. `css` was retired on 2026-08-02, so that collision is gone — and this asserts
        // it is gone, so nobody re-derives the old rationale from a stale comment.
        assert!(
            !KNOWN_KEYS.contains(&"css"),
            "`css` was retired; the edit-distance-1 collision no longer exists"
        );
        let without_csl: Vec<&'static str> =
            KNOWN_KEYS.iter().copied().filter(|k| *k != "csl").collect();
        assert_ne!(
            closest("csl", &without_csl),
            Some("css"),
            "`css` must not be a suggestion candidate any more"
        );
        // As shipped: recognized, so the unknown-key lint stays silent on it.
        assert!(
            KNOWN_KEYS.contains(&"csl"),
            "`csl` must stay in the allowlist"
        );
        assert!(
            !msgs("---\ntitle: X\ncsl: ieee.csl\n---\n")
                .iter()
                .any(|w| w.contains("unknown front-matter key")),
            "`csl` must never be reported as an unknown key"
        );
    }

    #[test]
    fn invalid_yaml_yields_no_lint_warnings() {
        // The YAML parse error is reported separately by `yaml_error`.
        assert!(validate_front_matter("---\ntitle: X\n: : :\n---\n").is_empty());
    }

    #[test]
    fn no_front_matter_yields_no_warnings() {
        assert!(validate_front_matter("# Heading\n\ntext\n").is_empty());
        assert!(validate_front_matter("").is_empty());
    }

    #[test]
    fn dropped_legacy_keys_now_warn() {
        let m = msgs("---\ntitle: X\ntitle-block-banner: false\nsite-url: https://x\n---\n");
        assert!(
            m.iter().any(|w| w.contains("`title-block-banner`")),
            "got: {m:?}"
        );
        assert!(m.iter().any(|w| w.contains("`site-url`")), "got: {m:?}");
    }

    #[test]
    fn honored_keys_do_not_warn() {
        let w = validate_front_matter(
            "---\ntitle: X\ntitle-block-style: none\ntheme: dark\npage-layout: full\n---\n",
        );
        assert!(w.is_empty(), "honored keys must not warn, got: {w:?}");
    }

    // The YAML-parse-error locator is unchanged.
    #[test]
    fn yaml_error_reports_the_file_line() {
        let (msg, line) = yaml_error("---\ntitle: ok\nbad: : x\n---\n\nbody\n").expect("an error");
        assert!(msg.contains("not valid YAML"), "got: {msg}");
        assert_eq!(line, 3);
    }

    #[test]
    fn prose_lint_key_is_retired_and_says_so() {
        let w = validate_front_matter("---\ntitle: T\nprose-lint: true\n---\n");
        let msg = &w
            .iter()
            .find(|x| x.message.contains("`prose-lint`"))
            .expect("a retired key still warns")
            .message;
        assert!(
            msg.contains("removed on 2026-08-02") && msg.contains("nothing replaces them"),
            "must say it went and that there is no successor: {msg}"
        );
        assert!(!msg.contains("did you mean"), "not a rename hint: {msg}");
    }

    #[test]
    fn yaml_error_none_when_valid_or_absent() {
        assert!(yaml_error("---\ntitle: X\n---\n\nbody\n").is_none());
        assert!(yaml_error("no front matter\n").is_none());
    }

    #[test]
    fn unknown_top_level_key_carries_a_column_span() {
        let src = "---\ntitle: X\ntreme: darkly\n---\n";
        let w = validate_front_matter(src);
        let d = w
            .iter()
            .find(|w| w.message.contains("`treme`"))
            .expect("treme flagged");
        assert_eq!(d.line, Some(3));
        assert_eq!(d.col, Some(1)); // `treme` starts at column 1
        assert_eq!(d.end_col, Some(6)); // one past the 5-char key
    }

    #[test]
    fn unknown_nested_key_carries_an_indented_column_span() {
        let src = "---\ntitle: X\nexecute:\n  eccho: false\n---\n";
        let w = validate_front_matter(src);
        let d = w
            .iter()
            .find(|w| w.message.contains("`eccho`"))
            .expect("eccho flagged");
        assert_eq!(d.line, Some(4));
        assert_eq!(d.col, Some(3)); // 2-space indent -> column 3
        assert_eq!(d.end_col, Some(8));
    }

    /// PA-M13. The escape hatch matters as much as the warning: an author who means the
    /// image decoratively writes `image-alt: ""`, and must not be nagged forever.
    #[test]
    fn an_image_without_its_alt_text_warns_but_an_explicitly_empty_alt_does_not() {
        let warn_texts = |src: &str| -> Vec<String> {
            validate_front_matter(src)
                .iter()
                .map(|w| w.message.clone())
                .filter(|m| m.contains("missing alt text"))
                .collect()
        };

        let missing = warn_texts("---\ntitle: A post\nimage: cover.png\n---\n\nBody.\n");
        assert_eq!(
            missing.len(),
            1,
            "an `image:` with no `image-alt:` must warn"
        );
        assert!(
            missing[0].contains("social/listing image"),
            "the warning must say which image it means: {}",
            missing[0]
        );

        assert!(
            warn_texts("---\ntitle: A post\nimage: cover.png\nimage-alt: A cover\n---\n")
                .is_empty(),
            "an `image:` WITH `image-alt:` must stay silent"
        );
        assert!(
            warn_texts("---\ntitle: A post\nimage: cover.png\nimage-alt: \"\"\n---\n").is_empty(),
            "an explicitly empty `image-alt:` is a deliberate decorative image, not an omission"
        );
        assert!(
            warn_texts("---\ntitle: A post\n---\n").is_empty(),
            "a page with no `image:` at all has nothing to warn about"
        );

        // `hero.image` was retired on 2026-08-02, so it draws the retired-key diagnostic and
        // must NOT also draw an alt-text one: telling an author to add `image-alt:` to a key
        // that no longer exists is a second, contrary instruction on the same line.
        let src = "---\ntitle: A page\nhero:\n  headline: Hi\n  image: h.png\n---\n";
        assert!(
            warn_texts(src).is_empty(),
            "a retired `hero.image` must not draw an alt-text warning: {:?}",
            warn_texts(src)
        );
        assert!(
            validate_front_matter(src)
                .iter()
                .any(|w| w.message.contains("hero key `image`")
                    && w.message.contains("removed on 2026-08-02")),
            "it must still be diagnosed, as a retired key"
        );
    }

    /// The calibration guard. `image:` inside a fenced YAML EXAMPLE is prose, not front
    /// matter, and two real `docs/` pages are written exactly this way — a line-scanning
    /// version of this lint fired on both. Reading the parsed front matter is what makes
    /// it correct, so pin that rather than the implementation detail.
    #[test]
    fn an_image_key_inside_a_prose_example_is_not_front_matter() {
        let src = "---\ntitle: Configuration\n---\n\nWrite the front matter like this:\n\n\
                   ```yaml\ntitle: \"A post about rust\"\nimage: cover.png\n```\n";
        let warnings: Vec<_> = validate_front_matter(src)
            .iter()
            .map(|w| w.message.clone())
            .filter(|m| m.contains("missing alt text"))
            .collect();
        assert!(
            warnings.is_empty(),
            "an `image:` in a prose example must not be linted as front matter: {warnings:?}"
        );
    }
}

/// The third link in the front-matter documentation chain.
///
/// `vocab.rs` already ties [`KNOWN_KEYS`] to the editor vocabulary, and `schema.rs`
/// generates the JSON schema from the same consts — but **nothing tied either to the
/// User Guide**, which is the surface a reader actually copies from. The gap is not
/// hypothetical: `about:` was removed at `dcf0588` (2026-07-17), which correctly
/// scrubbed the code, the schema, the vocab, the CSS and `AGENTS.md` and never touched
/// `docs/`. For the nine days after, the guide kept a dedicated `## about:` reference
/// section, a sub-key table, a worked recipe and a `formats.tmd` subsection for a key
/// that had become an `unknown front-matter key` **warning** — so a reader following
/// the guide failed `check`, `build --strict` and `publish`.
///
/// Same drift shape, and the same fix shape, as the CLI-flag and env-var gates in
/// `crates/server/src/main.rs`: make the docs mechanically answerable to the code.
#[cfg(test)]
mod guide_vocabulary_gate {
    use super::KNOWN_KEYS;
    use std::path::{Path, PathBuf};

    fn guide_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/guide")
    }

    /// Every `.tmd` under `docs/guide`, skipping the gitignored build/exec artifacts
    /// (`_book/`, `_freeze/`) — those are stale copies of the very pages being checked,
    /// so including them would report each finding twice and keep reporting it after a fix.
    fn guide_pages() -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for e in entries.flatten() {
                let p = e.path();
                let name = e.file_name();
                let name = name.to_string_lossy();
                if p.is_dir() {
                    if name != "_book" && name != "_freeze" {
                        walk(&p, out);
                    }
                } else if p.extension().and_then(|s| s.to_str()) == Some("tmd") {
                    out.push(p);
                }
            }
        }
        let mut out = Vec::new();
        walk(&guide_root(), &mut out);
        out.sort();
        assert!(
            !out.is_empty(),
            "no guide pages found under {:?}",
            guide_root()
        );
        out
    }

    /// The top-level keys of every fenced ```yaml block in `src` that is a **front-matter**
    /// example (opens with `---`). Yields `(key, line)`.
    ///
    /// Scoped to front matter on purpose: the same pages also show `_site.yml` blocks and
    /// a GitHub Actions workflow, which are different vocabularies and would be pure noise
    /// here. Only column-zero `key:` lines count, so a nested `about:` sub-key like
    /// `template:` is not mistaken for a top-level key.
    fn front_matter_example_keys(src: &str) -> Vec<(String, usize)> {
        let lines: Vec<&str> = src.lines().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            if lines[i].trim_end() != "```yaml" {
                i += 1;
                continue;
            }
            let start = i + 1;
            let mut j = start;
            while j < lines.len() && lines[j].trim_end() != "```" {
                j += 1;
            }
            let body = &lines[start..j.min(lines.len())];
            // A front-matter example opens with the `---` fence.
            if body.first().map(|l| l.trim_end()) == Some("---") {
                for (k, line) in body.iter().enumerate() {
                    if line.starts_with(['-', ' ', '\t', '#']) || line.trim().is_empty() {
                        continue;
                    }
                    if let Some((key, _)) = line.split_once(':')
                        && !key.is_empty()
                        && key
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                    {
                        out.push((key.to_string(), start + k + 1));
                    }
                }
            }
            i = j + 1;
        }
        out
    }

    /// Every top-level key the guide SHOWS in a front-matter example is a real key.
    ///
    /// This is the copy-paste surface: a reader reproduces an example verbatim far more
    /// often than they read a table row, so an example naming a removed key is the most
    /// expensive kind of drift.
    #[test]
    fn every_front_matter_example_in_the_guide_uses_only_real_keys() {
        let mut bad = Vec::new();
        for page in guide_pages() {
            let src = std::fs::read_to_string(&page).unwrap();
            for (key, line) in front_matter_example_keys(&src) {
                if !KNOWN_KEYS.contains(&key.as_str()) {
                    let rel = page
                        .strip_prefix(guide_root().parent().unwrap())
                        .unwrap_or(&page)
                        .display()
                        .to_string();
                    bad.push(format!("{rel}:{line}: `{key}`"));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "the User Guide shows front-matter examples using keys that are NOT in \
             KNOWN_KEYS, so a reader copying them gets an `unknown front-matter key` \
             warning (which fails `check`, `build --strict` and `publish`):\n  {}",
            bad.join("\n  ")
        );
    }

    /// No nested-block reference section documents a key that no longer exists.
    ///
    /// `frontmatter.tmd` gives each nested block (`execute:`, `listing:`, `hero:`,
    /// `prose-lint:`) its own `## \`name:\`` section with a sub-key table. A section for a
    /// removed key is worse than a stale table row: it reads as a whole supported feature.
    #[test]
    fn every_nested_block_section_in_the_reference_names_a_real_key() {
        let path = guide_root().join("reference/frontmatter.tmd");
        let src = std::fs::read_to_string(&path).unwrap();
        let mut bad = Vec::new();
        for (i, line) in src.lines().enumerate() {
            // `## \`about:\` (a profile page) {#…}`
            let Some(rest) = line.strip_prefix("## `") else {
                continue;
            };
            let Some((name, _)) = rest.split_once("`") else {
                continue;
            };
            let key = name.trim_end_matches(':');
            if !KNOWN_KEYS.contains(&key) {
                bad.push(format!("reference/frontmatter.tmd:{}: `{key}`", i + 1));
            }
        }
        assert!(
            bad.is_empty(),
            "the front-matter reference documents a nested block whose key is not in \
             KNOWN_KEYS:\n  {}",
            bad.join("\n  ")
        );
    }

    /// …and the other direction: the reference page claims to be "the full vocabulary,
    /// every top-level key Taliesin reads", so every [`KNOWN_KEYS`] entry must appear on
    /// it. Completeness is the half the existing `--help`/guide gates already enforce for
    /// flags and env vars; without it a key can ship undocumented, which is how
    /// `footer:`/`logo:` (real deck chrome) stayed off the reference page.
    #[test]
    fn the_reference_page_documents_every_known_key() {
        let path = guide_root().join("reference/frontmatter.tmd");
        let src = std::fs::read_to_string(&path).unwrap();
        let missing: Vec<&str> = KNOWN_KEYS
            .iter()
            .copied()
            .filter(|k| !src.contains(&format!("`{k}`")))
            .collect();
        assert!(
            missing.is_empty(),
            "`docs/guide/reference/frontmatter.tmd` calls itself the full front-matter \
             vocabulary but never mentions: {missing:?}"
        );
    }
}
