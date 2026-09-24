//! Front-matter schema validation.
//!
//! taliesin reads a leading YAML `---` block and validates it against its OWN
//! recognized key set. A key taliesin does not
//! implement, whether a typo like `treme:` or a legacy term it does not honor, is
//! flagged by [`validate_front_matter`]: every unknown top-level key, plus every
//! unknown immediate child of the nested `execute:` / `listing:` / `hero:`
//! blocks, each suggesting the closest known key. It only warns (located
//! for click-to-source); rendering is unaffected, an unknown key still renders.

use crate::render::{Severity, Warning};

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
    "categories",
    // Images / social
    "image",
    "image-alt",
    // Drafts: `draft: true` holds a page (or book chapter) out of the published build
    // (output, nav, listings); the live preview still shows it, badged.
    "draft",
    // Title block: `title-block-style: none` is honored (suppresses the visible
    // header); see `render::DocFront::title_block_hidden`.
    "title-block-style",
    // Table of contents
    "toc",
    // Citations
    "bibliography",
    // Execution
    "execute",
    // Listings / project pages
    "listing",
    "hero",
];

/// `execute:` sub-keys taliesin honors (document-level cell defaults; see
/// `render::DocFront::exec_cache`).
///
/// One key, and that is the whole set on purpose: `echo`/`include` were document-wide
/// defaults for something every real document says per cell (`#| echo:`), so they were
/// retired on 2026-08-02 rather than kept as a second way to say it.
pub(crate) const EXECUTE_KEYS: &[&str] = &["cache"];

/// `listing:` sub-keys taliesin honors (see `site::frontmatter::parse_listing_spec`).
///
/// No `sort:`. A listing is newest-first, which is what all four real listings wrote out
/// longhand, and `parse_listing_spec` does not read the key
/// (`a_retired_listing_sort_cannot_reverse_the_cards_or_the_feed` pins that).
pub(crate) const LISTING_KEYS: &[&str] = &["contents", "id", "type", "max-items"];

/// The `listing: type:` values `site::frontmatter::parse_listing_spec` reads. Any other
/// value renders the default text list, so it is reported rather than accepted in silence.
pub(crate) const LISTING_TYPES: &[&str] = &["list"];

/// `hero:` sub-keys taliesin honors (see `site::frontmatter::parse_hero`).
///
/// Text and links only: the banner is type, not a figure. `image`/`image-alt` were retired
/// on 2026-08-02 unused.
pub(crate) const HERO_KEYS: &[&str] = &["eyebrow", "headline", "lead", "actions"];

/// The keys of one `hero.actions:` entry (`{ text, href, primary }`). A typo here used to
/// be silent — the item parses, the button renders with no label or no link — which is the
/// same failure shape the `chapters:` validator exists to prevent, so it warns.
pub(crate) const HERO_ACTION_KEYS: &[&str] = &["text", "href", "primary"];

/// Validate a document's front matter against taliesin's vocabulary: every unknown
/// top-level key, plus every unknown immediate child of the nested `execute:`,
/// `listing:`, and `hero:` blocks. Membership is decided by a real YAML
/// parse (so structure, lists, nested maps, never causes a false positive); each
/// warning is best-effort located (click-to-source) at the offending key's source
/// line. Empty when there is no front matter, it is not a mapping, or it fails to
/// parse (the parse error is reported separately by [`yaml_error`]).
pub fn validate_front_matter(src: &str) -> Vec<Warning> {
    let Some(block) = front_matter_block(src) else {
        return misplaced_front_matter(src).into_iter().collect();
    };
    let Some(value) = parse_front_matter_block(block) else {
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
    validate_date_value(map, block, &mut out);
    validate_bool_value(
        map.get("toc"),
        "toc",
        block_key_span(block, "toc"),
        &mut out,
    );
    if let Some(serde_yaml::Value::Mapping(execute)) = map.get("execute") {
        validate_bool_value(
            execute.get("cache"),
            "cache",
            nested_key_span(block, "execute", "cache"),
            &mut out,
        );
    }
    validate_image_alt(map, block, &mut out);
    validate_nested(map, "execute", "execute key", EXECUTE_KEYS, block, &mut out);
    validate_nested(map, "hero", "hero key", HERO_KEYS, block, &mut out);
    validate_hero_actions(map, block, &mut out);
    // `listing:` is one mapping or a sequence of mappings (a page carrying two lists).
    match map.get("listing") {
        Some(serde_yaml::Value::Mapping(m)) => {
            validate_child_keys(m, "listing", "listing key", LISTING_KEYS, block, &mut out);
            validate_listing_values(m, block, &mut out);
        }
        Some(serde_yaml::Value::Sequence(seq)) => {
            for item in seq {
                if let Some(m) = item.as_mapping() {
                    validate_child_keys(m, "listing", "listing key", LISTING_KEYS, block, &mut out);
                    validate_listing_values(m, block, &mut out);
                }
            }
        }
        _ => {}
    }
    out
}

/// A parsed YAML value as a boolean: a real bool, or one of the YAML-1.1 words
/// [`yaml_bool_word`] catches. `None` for anything else. The one boolean read of a parsed
/// value (`toc:`, `execute: cache:`, a hero action's `primary:`).
pub(crate) fn value_bool(v: &serde_yaml::Value) -> Option<bool> {
    match v {
        serde_yaml::Value::Bool(b) => Some(*b),
        serde_yaml::Value::String(s) => yaml_bool_word(s),
        _ => None,
    }
}

/// Interpret a raw YAML scalar as a boolean, catching the YAML-1.1 words serde_yaml
/// (which follows YAML 1.2) reads as plain STRINGS — `yes`/`no`/`on`/`off` — alongside
/// canonical `true`/`false` (case-insensitive, tolerant of surrounding quotes). Returns
/// `None` for any non-boolean value (e.g. `echo: fenced`), so a caller keeps its own
/// meaning for that. The single source of the boolean vocabulary shared by the
/// front-matter (`site::frontmatter::draft_flag`, [`value_bool`]), cell-option
/// (`render::cell_extract`), toc (`render::DocFront::toc`), and `_site.yml`
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

/// `date:` is the one front-matter value read by MACHINES (the sitemap's `<lastmod>`, the
/// Atom feed's `<updated>`), so a value they cannot parse is lost to both while the page
/// still displays it: the sitemap drops the page's `<lastmod>` and the feed drops the
/// entry, and without this a green `check` certified a half-published post. Free text
/// (`date: Spring 2026`) stays legal for display, which is why this reports what is lost
/// rather than calling the value wrong.
fn validate_date_value(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    // Every scalar the site reads as a date (`site::scalar`), a number included: an
    // un-quoted `date: 20260515` is as unreadable to the sitemap and the feed as free text.
    let Some(val) = crate::site::scalar(map.get("date")) else {
        return;
    };
    let val = val.trim().trim_matches(['"', '\'']);
    if val.is_empty() || calendar_date(val).is_some() {
        return;
    }
    out.push(located(
        format!(
            "`date: {val}` isn't a machine-readable date, so the sitemap carries no \
             `<lastmod>` for this page and the Atom feed leaves it out (the page still \
             shows the date); write `YYYY-MM-DD` to publish it"
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

/// A boolean key (`toc:`, `execute: cache:`) whose value [`value_bool`] cannot read. The
/// renderer ignores such a value, so the page quietly got the automatic TOC (or kept the
/// cache on) with nothing reported. A null value is unset, not wrong.
fn validate_bool_value(
    v: Option<&serde_yaml::Value>,
    key: &str,
    span: Option<(u32, u32, u32)>,
    out: &mut Vec<Warning>,
) {
    let Some(v) = v.filter(|v| !v.is_null() && value_bool(v).is_none()) else {
        return;
    };
    let written = match crate::site::scalar(Some(v)) {
        Some(s) => format!("`{key}: {s}`"),
        None => format!("`{key}:` with a list or a mapping"),
    };
    out.push(located_span(
        format!(
            "{written} is not a boolean, so it is ignored: write `{key}: true` or `{key}: false`"
        ),
        span,
    ));
}

/// A `listing:` value its parser cannot use: a `type:` outside [`LISTING_TYPES`], which
/// renders the default text list, and a `max-items:` that is not a whole number (`"2"`,
/// `2.0`, `-1`), which leaves the listing uncapped and so also makes it the posts' owning
/// listing. Both used to be dropped in silence.
fn validate_listing_values(m: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    if let Some(ty) = crate::site::scalar(m.get("type"))
        && !LISTING_TYPES.contains(&ty.as_str())
    {
        out.push(located_span(
            unknown_key_message("listing type", &ty, LISTING_TYPES),
            nested_key_span(block, "listing", "type"),
        ));
    }
    if let Some(v) = m.get("max-items")
        && !v.is_null()
        && v.as_u64().is_none()
    {
        let shown = match v {
            serde_yaml::Value::String(s) => format!("\"{s}\""),
            other => crate::site::scalar(Some(other)).unwrap_or_else(|| "…".to_string()),
        };
        out.push(located_span(
            format!(
                "`max-items: {shown}` is not a whole number, so the listing is not capped \
                 (write a count, like `max-items: 3`)"
            ),
            nested_key_span(block, "listing", "max-items"),
        ));
    }
}

/// Validate each entry of `hero.actions:`, the list of buttons under a landing banner.
///
/// This vocabulary was unvalidated until 2026-08-02, and unlike a top-level key its typo
/// is *silent and visible*: `hef:` renders a button that goes nowhere, `txt:` renders one
/// with no label, and `check` stays green. That is the same failure shape as a typo'd
/// chapter entry (`site::book`), and it earns the same diagnostic.
///
/// Located at the misspelled key, in a flow-style entry (`- { txt: Go }`, the form every
/// real page uses) as in a block-style one; falls back to the `hero:` key when the key
/// cannot be found.
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
                    nested_key_span(block, "hero", key).or_else(|| block_key_span(block, "hero")),
                ));
            }
        }
    }
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
/// line rule). Top-level keys are unindented, so `col` is 1, or 2 for a quoted key, which
/// is the same key to YAML (`"title": x`). Columns are Unicode-scalar counts; a front-matter
/// key is ASCII, so scalar == byte == UTF-16.
pub(crate) fn block_key_span(block: &str, key: &str) -> Option<(u32, u32, u32)> {
    block.lines().enumerate().find_map(|(i, line)| {
        let quote = (!line.starts_with(char::is_whitespace))
            .then(|| key_at(line, key))
            .flatten()?;
        let col = 1 + quote as u32;
        Some((i as u32 + 2, col, col + key.chars().count() as u32))
    })
}

/// `(line, col, end_col)` of a nested child `key` under `parent:`, all 1-based: at the start
/// of a line in the parent's block (after its indentation and any `- ` list marker), or as a
/// key of a flow mapping written on one line (`- { text: Go, href: a.tmd }`, how list
/// entries are usually written). Indentation is ASCII, so the scalar column equals the
/// byte/UTF-16 column.
fn nested_key_span(block: &str, parent: &str, key: &str) -> Option<(u32, u32, u32)> {
    let mut in_block = false;
    for (i, line) in block.lines().enumerate() {
        let t = line.trim_start();
        let at_top = !line.is_empty() && line.len() == t.len();
        if !in_block {
            if at_top && key_at(t, parent).is_some() {
                in_block = true;
            }
            continue;
        }
        if at_top {
            break; // dedent ends the parent block
        }
        // Where a key can start: past the indentation and any `- ` markers, and past
        // each `{` or `,` of a flow mapping.
        let mut starts = Vec::new();
        let mut s = line.len() - t.len();
        while let Some(rest) = line[s..].strip_prefix("- ") {
            s = line.len() - rest.trim_start().len();
        }
        starts.push(s);
        for (b, c) in line.char_indices() {
            if matches!(c, '{' | ',') {
                let after = &line[b + 1..];
                starts.push(line.len() - after.trim_start().len());
            }
        }
        if let Some((start, quote)) = starts
            .into_iter()
            .find_map(|s| key_at(&line[s..], key).map(|q| (s, q)))
        {
            let col = (start + quote) as u32 + 1;
            return Some((i as u32 + 2, col, col + key.chars().count() as u32));
        }
    }
    None
}

/// Whether `text` starts with the mapping key `key` (bare or quoted, then `:`), and if so
/// how many quote characters precede it (0 or 1).
fn key_at(text: &str, key: &str) -> Option<usize> {
    let (quote, body) = match text.chars().next() {
        Some(q @ ('"' | '\'')) => (Some(q), &text[1..]),
        _ => (None, text),
    };
    let rest = body.strip_prefix(key)?;
    let rest = match quote {
        Some(q) => rest.strip_prefix(q)?,
        None => rest,
    };
    rest.trim_start()
        .starts_with(':')
        .then_some(usize::from(quote.is_some()))
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

/// A document's front matter parsed as YAML, the one parse every field reader goes
/// through (the renderer's `render::DocFront` and the site's `parse_front_matter`).
/// `None` when the document has no front matter, when the block is blank, or when it is
/// not valid YAML: [`yaml_error`] reports that last case, located, and the build fails
/// on it, so nothing here recovers a guess from a block YAML cannot read.
pub(crate) fn front_matter_value(src: &str) -> Option<serde_yaml::Value> {
    parse_front_matter_block(front_matter_block(src)?)
}

/// [`front_matter_value`] for a block the caller already split off with
/// [`front_matter_block`].
pub(crate) fn parse_front_matter_block(block: &str) -> Option<serde_yaml::Value> {
    if block.trim().is_empty() {
        return None;
    }
    serde_yaml::from_str(block).ok()
}

/// The leading `---` ... `---`/`...` block of a document, without the fences.
/// `None` if the source doesn't open with a front-matter fence. THE front-matter splitter:
/// the renderer blanks this block before comrak sees the source ([`blank_front_matter`];
/// comrak's own front-matter extension is off), and the lint, site discovery, the listing
/// card, the feed and the dev server's digest all read it, so every path agrees on what the
/// block is. A fence line may carry trailing whitespace, a BOM may precede the opening
/// fence, and the block closes at `---` or `...`, as in Pandoc and Quarto.
///
/// `pub` because the dev server digests this block to tell a body edit from a change to
/// what DISCOVERY reads, and a second hand-rolled `---` splitter is how the two would come
/// to disagree about a BOM or a `...` terminator.
pub fn front_matter_block(src: &str) -> Option<&str> {
    front_matter_span(src).map(|(inner, _)| &src[inner])
}

/// Where [`front_matter_block`]'s block sits in `src`: the byte range between the fences,
/// and the offset just past the closing fence line.
fn front_matter_span(src: &str) -> Option<(std::ops::Range<usize>, usize)> {
    let bom = if src.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let first = src[bom..].split_inclusive('\n').next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let after = bom + first.len();
    let mut pos = after;
    for line in src[after..].split_inclusive('\n') {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            return Some((after..pos, pos + line.len()));
        }
        pos += line.len();
    }
    None
}

/// `src` with its front matter, fences included, blanked line for line: every character
/// but the line breaks goes, so comrak parses the body alone while every later line keeps
/// its number (click-to-source, `data-sourcepos` and every diagnostic stay in the author's
/// own numbering). Borrows when there is no front matter.
pub(crate) fn blank_front_matter(src: &str) -> std::borrow::Cow<'_, str> {
    let Some((_, end)) = front_matter_span(src) else {
        return std::borrow::Cow::Borrowed(src);
    };
    let mut out: String = src[..end]
        .chars()
        .filter(|c| matches!(c, '\n' | '\r'))
        .collect();
    out.push_str(&src[end..]);
    std::borrow::Cow::Owned(out)
}

/// A block that is meant as front matter but that [`front_matter_block`] does not accept,
/// as a located error: a blank line above the opening `---`, or no closing fence. Either
/// way the block is body text, so every key in it is ignored and the YAML is published on
/// the page. Only a block whose keys include a real front-matter key counts as meant, so a
/// document that opens with a thematic break stays silent.
fn misplaced_front_matter(src: &str) -> Option<Warning> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let meant = |yaml: &str| {
        parse_front_matter_block(yaml)
            .as_ref()
            .and_then(serde_yaml::Value::as_mapping)
            .is_some_and(|m| {
                m.keys()
                    .filter_map(|k| k.as_str())
                    .any(|k| KNOWN_KEYS.contains(&k))
            })
    };
    let blank = src.lines().take_while(|l| l.trim().is_empty()).count();
    let (message, line) = if blank > 0 {
        let rest = src.split_inclusive('\n').skip(blank).collect::<String>();
        if !front_matter_block(&rest).is_some_and(meant) {
            return None;
        }
        (
            "front matter must start on the first line: a blank line above this `---` makes \
             the block body text, so every key in it is ignored and the YAML is published \
             on the page (delete the blank line)",
            blank + 1,
        )
    } else {
        let mut lines = src.lines();
        if lines.next()?.trim_end() != "---" {
            return None;
        }
        let head = lines
            .take_while(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if !meant(&head) {
            return None;
        }
        (
            "front matter opened on this line is never closed, so it is body text: every key \
             in it is ignored and the YAML is published on the page (end the block with a \
             `---` line)",
            1,
        )
    };
    Some(
        Warning::new(message)
            .at(None, line as u32)
            .severity(Severity::Error),
    )
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
/// Ties break on the candidate's own spelling, not on iteration order. `min_by_key` keeps
/// the FIRST minimum, so ordering the key by distance alone let whichever candidate the
/// iterator happened to yield first win — and two of the three callers feed it a set: a
/// `HashSet` of reactive define names (randomly seeded per process) and a `read_dir`
/// listing (filesystem order). Measured across 20 identical runs of one document, the same
/// typo suggested `ax` 13 times and `bx` 7, and `--format json`'s machine-readable
/// `suggestion.replacement` flipped with it — so an editor's quick fix rewrote the author's
/// buffer to a different name depending on the run.
pub fn closest_of<'a>(key: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    candidates
        .into_iter()
        .map(|k| (levenshtein(key, k), k))
        .filter(|&(d, _)| d > 0 && d <= 2)
        .min_by_key(|&(d, k)| (d, k))
        .map(|(_, k)| k)
}

/// Build an "unknown <what> `<key>`" message, appending "(did you mean `X`?)" when a
/// known candidate is within edit distance 2. The single message format shared by the
/// front-matter, cell-option, callout, and nested-config validators.
pub(crate) fn unknown_key_message(what: &str, key: &str, candidates: &[&'static str]) -> String {
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
    fn flags_unknown_execute_child() {
        // `cache` is the only `execute:` sub-key left, so a typo of IT is what still draws a
        // did-you-mean.
        let m = msgs("---\ntitle: X\nexecute:\n  cach: true\n---\n");
        assert_eq!(
            m,
            vec!["unknown execute key `cach` (did you mean `cache`?)"]
        );
    }

    /// An unknown `execute:` child is flagged in ITS OWN scope, never against the
    /// top-level key set: `echo:` was an `execute:` sub-key until 2026-08-02 and `#| echo:`
    /// is still a live CELL option, so the two vocabularies must stay separate.
    #[test]
    fn an_unknown_execute_child_is_flagged_in_its_own_scope() {
        for key in ["echo", "include"] {
            let m = msgs(&format!("---\ntitle: X\nexecute:\n  {key}: false\n---\n"));
            let msg = m
                .iter()
                .find(|x| x.contains(&format!("`{key}`")))
                .unwrap_or_else(|| panic!("no diagnostic for unknown `{key}`: {m:?}"));
            assert!(
                msg.starts_with("unknown execute key"),
                "must be scoped to `execute key`: {msg}"
            );
        }
    }

    #[test]
    fn flags_unknown_listing_child_in_a_mapping_and_a_sequence() {
        let m = msgs("---\ntitle: X\nlisting:\n  contents: posts\n  max-itemz: 3\n---\n");
        assert_eq!(
            m,
            vec!["unknown listing key `max-itemz` (did you mean `max-items`?)"]
        );
        // A sequence of listings validates each item. No corpus document carries this
        // shape today (the last one, the tech-blog CV, went 2026-09-11), so this inline
        // case IS the witness — do not delete it as redundant.
        let m2 = msgs("---\ntitle: X\nlisting:\n  - contents: a\n    sort-uii: false\n---\n");
        assert_eq!(m2, vec!["unknown listing key `sort-uii`"]);
    }

    /// A `listing:` value the parser cannot use was dropped in silence: a `type:` outside
    /// the vocabulary rendered the default text list, and a `max-items:` that is not a
    /// whole number (`"2"`, `2.0`, `-1`) left the listing uncapped (which also made it the
    /// posts' owning listing). Both are located now.
    #[test]
    fn listing_type_and_max_items_values_are_validated() {
        let ws = validate_front_matter(
            "---\ntitle: X\nlisting:\n  contents: posts\n  type: lists\n  max-items: 2.0\n---\n",
        );
        let w = ws
            .iter()
            .find(|w| w.message.contains("type"))
            .unwrap_or_else(|| panic!("{ws:?}"));
        assert_eq!(
            w.message,
            "unknown listing type `lists` (did you mean `list`?)"
        );
        assert_eq!(w.line, Some(5), "{w:?}");
        let w = ws
            .iter()
            .find(|w| w.message.contains("max-items"))
            .unwrap_or_else(|| panic!("{ws:?}"));
        assert!(
            w.message.contains("`max-items: 2.0` is not a whole number"),
            "{w:?}"
        );
        assert_eq!(w.line, Some(6), "{w:?}");

        for bad in ["\"2\"", "-1"] {
            let m = msgs(&format!(
                "---\ntitle: X\nlisting:\n  - contents: posts\n    max-items: {bad}\n---\n"
            ));
            assert!(
                m.iter().any(|w| w.contains("not a whole number")),
                "{bad}: {m:?}"
            );
        }
        assert_eq!(
            msgs("---\ntitle: X\nlisting:\n  contents: posts\n  type: table\n---\n"),
            vec!["unknown listing type `table`"]
        );
        assert!(
            msgs("---\ntitle: X\nlisting:\n  contents: posts\n  type: list\n  max-items: 3\n---\n")
                .is_empty()
        );
    }

    /// The listing category-filter chips were deleted 2026-08-03 (visual minimalism
    /// pass): they paid off only on a blog with many posts AND disciplined category
    /// vocabulary — the linter existed precisely because that discipline does not hold
    /// by default. Page-level `categories:` SURVIVES; only `listing.categories` is retired,
    /// and it must be recognized in ITS OWN scope (`listing key`), not answered with the
    /// generic did-you-mean.
    ///
    /// The card badges did NOT survive — they went on 2026-08-15 with spec §9's cut #12, and
    /// this note went with them, because a retirement note that describes a second thing
    /// which has since been deleted sends the author looking for a badge that is not there.
    /// The key itself is not retired and must not be: `feed.rs` still emits one `<category>`
    /// per entry, which is where a tag does real work.
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
            .unwrap_or_else(|| panic!("no diagnostic for `listing.categories`: {m:?}"));
        assert!(
            msg.starts_with("unknown listing key `categories`"),
            "must be scoped to `listing key`, not the generic front-matter scope: {msg}"
        );
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
    fn clean_doc_with_nested_blocks_has_no_warnings() {
        let w = validate_front_matter(
            "---\ntitle: X\ntoc: true\nexecute:\n  cache: true\nlisting:\n  contents: posts\n  type: list\n---\n\nx\n",
        );
        assert!(w.is_empty(), "got: {w:?}");
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

    /// The parser-side pin for the 2026-08-20 `page-layout: full` cut.
    ///
    /// `full` was a rendered no-op long before it was cut: the width override went with the
    /// card grid on 2026-08-15 (commit 6a30b565), after which `page.rs` appended a
    /// `.tali-wide` class that no CSS rule anywhere targeted, while the docs went on
    /// claiming it widened the column. So the value half is gone with the key: BOTH `full`
    /// and a Quarto leftover like `article` are now the same ordinary unknown key, and
    /// neither draws the old "is ignored" value warning.
    #[test]
    fn page_layout_is_an_ordinary_unknown_key_whatever_its_value() {
        for value in ["full", "article"] {
            let m = msgs(&format!("---\ntitle: X\npage-layout: {value}\n---\n"));
            assert!(
                m.iter()
                    .any(|w| w.starts_with("unknown front-matter key `page-layout`")),
                "`page-layout: {value}` draws the generic unknown-key lint: {m:?}"
            );
            assert!(
                !m.iter().any(|w| w.contains("is ignored")),
                "the value validator went with the key: {m:?}"
            );
        }
        assert!(
            !msgs("---\ntitle: X\n---\n")
                .iter()
                .any(|w| w.contains("page-layout")),
            "and a page that does not mention it says nothing"
        );
    }

    /// A `date:` we cannot parse is dropped from `<lastmod>`/`<updated>` rather than
    /// shipped invalid — but dropping it silently is the same "green check, wrong output"
    /// failure one layer down, so the value lints. `date:` is the one front-matter key
    /// whose value is read by *machines* (sitemap, Atom), which is why it earns a value
    /// rule when other free-text keys don't — and since `page-layout:` was cut on
    /// 2026-08-20 it is the only key with one.
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

    /// A `date:` YAML reads as a NUMBER (`date: 20260515`) is as unreadable to the sitemap
    /// and the feed as free text, but the rule only looked at strings, so it said nothing
    /// while the feed stamped the entry with another post's date. And the message says what
    /// actually happens: the page stays in the sitemap without a `<lastmod>`, and the feed
    /// leaves it out.
    #[test]
    fn a_numeric_date_is_linted_and_the_message_says_what_is_lost() {
        let m = msgs("---\ntitle: X\ndate: 20260515\n---\n");
        let w = m
            .iter()
            .find(|w| w.contains("20260515"))
            .unwrap_or_else(|| panic!("a numeric date must warn: {m:?}"));
        assert!(
            w.contains("no `<lastmod>`") && w.contains("Atom feed leaves it out"),
            "{w}"
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

    /// The parser-side pin for the 2026-08-20 `csl:` cut.
    ///
    /// `csl:` was a compatibility note wearing a key's clothes: recognized-but-inert, kept
    /// on the argument that it "names a real thing you brought from another tool". The
    /// 2026-08-17 ruling forecloses that argument -- taliesin answers for its own vocabulary
    /// and nothing else -- and the key's other stated ground was already dead by its own
    /// test's admission (`css` left KNOWN_KEYS on 2026-08-02, so dropping `csl` can no
    /// longer mis-suggest it).
    ///
    /// So this asserts the READ is gone and the generic path took over with no code added:
    /// the key draws the ordinary unknown-key diagnostic, the dedicated
    /// "recognized but not supported" wording exists nowhere, and -- the half that makes
    /// the cut safe -- there is no wrong rename hint, because `css` is not a candidate.
    #[test]
    fn csl_is_an_ordinary_unknown_key_with_no_phantom_suggestion() {
        let ws = validate_front_matter(
            "---\ntitle: T\nbibliography: refs.bib\ncsl: apa.csl\n---\n\nBody.\n",
        );
        let w = ws
            .iter()
            .find(|w| w.message.contains("`csl`"))
            .unwrap_or_else(|| panic!("`csl:` draws the generic unknown-key lint: {ws:?}"));
        assert!(
            w.message.starts_with("unknown front-matter key `csl`"),
            "the generic message, not a dedicated one: {}",
            w.message
        );
        assert!(
            !w.message.contains("did you mean"),
            "no near neighbour, so no guess -- `css` was retired 2026-08-02 and is not a \
             candidate, which is what made this cut safe: {}",
            w.message
        );
        assert_eq!(w.line, Some(4), "located at the `csl:` line: {w:?}");

        // The dedicated wording is gone from the tool entirely, not merely unreached.
        assert!(
            !ws.iter()
                .any(|w| w.message.contains("is recognized but not supported")),
            "the recognized-but-unsupported family has no producer left: {ws:?}"
        );
        assert!(!KNOWN_KEYS.contains(&"csl"), "`csl` left the allowlist");
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
        let w = validate_front_matter("---\ntitle: X\ntitle-block-style: none\ntoc: true\n---\n");
        assert!(w.is_empty(), "honored keys must not warn, got: {w:?}");
    }

    // The YAML-parse-error locator is unchanged.
    #[test]
    fn yaml_error_reports_the_file_line() {
        let (msg, line) = yaml_error("---\ntitle: ok\nbad: : x\n---\n\nbody\n").expect("an error");
        assert!(msg.contains("not valid YAML"), "got: {msg}");
        assert_eq!(line, 3);
    }

    /// `prose-lint:` names nothing this tool does (the linters went on 2026-08-02), so a
    /// leftover one is flagged rather than silently accepted. What it must not do is LINT,
    /// which `render::tests::prose_lint_is_retired_and_lints_nothing` pins.
    #[test]
    fn prose_lint_key_is_not_silently_accepted() {
        let w = validate_front_matter("---\ntitle: T\nprose-lint: true\n---\n");
        assert!(
            w.iter().any(|x| x.message.contains("`prose-lint`")),
            "an unknown key still warns: {w:?}"
        );
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

    /// `toc:` and `execute: cache:` are booleans. Any other value (`toc: 0`, `toc: n`, a
    /// list) was ignored in silence, so the page quietly got the automatic TOC (or kept the
    /// cache on); it is located now. The YAML-1.1 words (`yes`/`off`) are read as booleans
    /// and stay silent, and so does a null.
    #[test]
    fn a_boolean_key_with_a_value_that_is_not_one_is_diagnosed() {
        for (fm, line, needle) in [
            ("toc: 0\n", 3, "`toc: 0`"),
            ("toc: n\n", 3, "`toc: n`"),
            ("toc: [a]\n", 3, "`toc:`"),
            ("execute:\n  cache: 0\n", 4, "`cache: 0`"),
        ] {
            let ws = validate_front_matter(&format!("---\ntitle: X\n{fm}---\n"));
            let w = ws
                .iter()
                .find(|w| w.message.contains("not a boolean"))
                .unwrap_or_else(|| panic!("{fm:?}: {ws:?}"));
            assert!(w.message.contains(needle), "{fm:?}: {}", w.message);
            assert_eq!(w.line, Some(line), "{fm:?}: {w:?}");
        }
        for fm in [
            "toc: yes\n",
            "toc: false\n",
            "toc:\n",
            "execute:\n  cache: off\n",
        ] {
            let ws = validate_front_matter(&format!("---\ntitle: X\n{fm}---\n"));
            assert!(ws.is_empty(), "{fm:?}: {ws:?}");
        }
    }

    /// A hero action is written as a flow mapping on its own line, and a typo in one used
    /// to be located at the `hero:` key, whatever the doc comment promised, because the
    /// locator only matched a key at the start of an unindented line.
    #[test]
    fn a_hero_action_key_typo_is_located_at_the_key() {
        let src = "---\ntitle: T\nhero:\n  headline: H\n  actions:\n    \
                   - { text: Go, href: a.tmd }\n    - { txt: Typo, href: a.tmd }\n---\n";
        let w = validate_front_matter(src);
        let d = w
            .iter()
            .find(|w| w.message.contains("`txt`"))
            .unwrap_or_else(|| panic!("{w:?}"));
        assert_eq!(
            (d.line, d.col, d.end_col),
            (Some(7), Some(9), Some(12)),
            "{d:?}"
        );
    }

    /// A quoted key is the same key to YAML (`"title": x`), so its diagnostic is located the
    /// same way; it used to come out with no line at all.
    #[test]
    fn a_quoted_key_is_located_like_a_plain_one() {
        let w = validate_front_matter(
            "---\n\"titel\": X\n'date': Spring 2026\nexecute:\n  \"cach\": true\n---\n",
        );
        let at = |needle: &str| {
            let d = w
                .iter()
                .find(|w| w.message.contains(needle))
                .unwrap_or_else(|| panic!("{needle}: {w:?}"));
            (d.line, d.col, d.end_col)
        };
        assert_eq!(at("`titel`"), (Some(2), Some(2), Some(7)));
        assert_eq!(at("Spring 2026").0, Some(3));
        assert_eq!(at("`cach`"), (Some(5), Some(4), Some(8)));
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

        // `hero.image` was retired on 2026-08-02 and `parse_hero` stopped reading it, so it
        // must NOT draw an alt-text warning: telling an author to add `image-alt:` to a key
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
                .any(|w| w.message.contains("hero key `image`")),
            "it must still be diagnosed, as an unknown key"
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
