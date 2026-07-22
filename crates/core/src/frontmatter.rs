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
/// children of `execute:` / `listing:` / `hero:` are linted; `format:`
/// sub-keys are not (an extension owns them).
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
    "css",
    "page-layout",
    // Drafts: `draft: true` holds a page (or book chapter) out of the published build
    // (output, nav, listings); the live preview still shows it, badged.
    "draft",
    // Title block: `title-block-style: none` is honored (suppresses the visible
    // header); see `render::detect_title_block_hidden`.
    "title-block-style",
    // Per-document head/body injection, honored by `render::resolve_doc_includes`.
    "include-in-header",
    "include-before-body",
    "include-after-body",
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
    // Prose lint (opt-in): `prose-lint: true | { banned: [...] }`; see `crate::prose`.
    "prose-lint",
    // Theorem environments (per-document numbering config; see render::TheoremConfig).
    "theorems",
];

/// Keys taliesin RECOGNIZES but does not honor: it reads them, then ignores them.
///
/// They stay in [`KNOWN_KEYS`] on purpose, and the reason is not politeness. `csl` is edit
/// distance 1 from `css`, so dropping it would make the did-you-mean machinery answer a
/// `csl:` key with "did you mean `css`?" — confidently telling the author to rename their
/// citation-style key to a stylesheet key. That is worse than the silence it replaces, so
/// the key is recognized and a dedicated diagnostic says the honest thing instead
/// (`diagnostics::csl_recognized_but_unsupported`).
///
/// Also the exclusion list for the editor vocabulary (`vocab::vocab`): an unsupported key
/// must never be OFFERED as a completion. Recognizing what an author already wrote and
/// suggesting they write it are different acts.
pub(crate) const UNSUPPORTED_KEYS: &[&str] = &["csl"];

/// `execute:` sub-keys taliesin honors (document-level cell defaults; see
/// `render::detect_execute_defaults`).
pub(crate) const EXECUTE_KEYS: &[&str] = &["echo", "include", "cache"];

/// `listing:` sub-keys taliesin honors (see `site::frontmatter::parse_listing_spec`).
pub(crate) const LISTING_KEYS: &[&str] =
    &["contents", "id", "sort", "type", "max-items", "categories"];

/// `hero:` sub-keys taliesin honors (see `site::frontmatter::parse_hero`).
pub(crate) const HERO_KEYS: &[&str] = &[
    "eyebrow",
    "headline",
    "lead",
    "actions",
    "image",
    "image-alt",
];

/// `prose-lint:` sub-keys taliesin honors (the mapping form; see `crate::prose::config`).
pub(crate) const PROSE_LINT_KEYS: &[&str] = &["banned"];

/// `theorems:` sub-keys taliesin honors: `shared` (shared counters), `numbered`
/// (whether/when to number). The VALUES of `numbered` are checked by
/// [`validate_theorem_values`] so an unrecognized value warns rather than being silently
/// ignored. Numbering *scope* is deliberately not a key: a theorem scopes to its numbered
/// book chapter automatically, as every float does.
pub(crate) const THEOREM_KEYS: &[&str] = &["shared", "numbered"];

/// String values `theorems.numbered` honors besides a YAML bool; also the did-you-mean
/// suggestion candidates.
const THEOREM_NUMBERED: &[&str] = &["false", "unless-unique"];

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
    validate_nested(map, "execute", "execute key", EXECUTE_KEYS, block, &mut out);
    validate_nested(map, "hero", "hero key", HERO_KEYS, block, &mut out);
    validate_nested(
        map,
        "prose-lint",
        "prose-lint key",
        PROSE_LINT_KEYS,
        block,
        &mut out,
    );
    validate_nested(
        map,
        "theorems",
        "theorems key",
        THEOREM_KEYS,
        block,
        &mut out,
    );
    validate_theorem_values(map, block, &mut out);
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

/// Value-level checks for `theorems:`. The parser silently ignores an unrecognized
/// `numbered` value (rendering the OPPOSITE of intent — e.g. `numbered: never` stays
/// numbered), so flag it with a did-you-mean rather than certifying it on a green check.
/// Mirrors the accepted set in `render::fm_extract::parse_theorem_config`.
fn validate_theorem_values(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    let Some(serde_yaml::Value::Mapping(thm)) = map.get("theorems") else {
        return;
    };
    // `numbered` honors a YAML bool (true/false) or the string `unless-unique`.
    if let Some(v) = thm.get("numbered")
        && !(matches!(v, serde_yaml::Value::Bool(_)) || v.as_str() == Some("unless-unique"))
    {
        let line = nested_key_line(block, "theorems", "numbered");
        out.push(located(
            unknown_value_message("theorems numbered value", &value_label(v), THEOREM_NUMBERED),
            line,
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

/// A YAML scalar rendered for a diagnostic message (best-effort).
fn value_label(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Null => "null".to_string(),
        _ => "<value>".to_string(),
    }
}

/// Like [`unknown_key_message`] but for an unrecognized VALUE.
fn unknown_value_message(what: &str, value: &str, candidates: &[&'static str]) -> String {
    match closest(value, candidates) {
        Some(s) => format!("unknown {what} `{value}` (did you mean `{s}`?)"),
        None => format!("unknown {what} `{value}`"),
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
/// line rule). Top-level keys are unindented, so `col` is 1 and `end_col` is `1 + key.len()`.
/// Columns are Unicode-scalar counts; a front-matter key is ASCII, so scalar == byte == UTF-16.
pub(crate) fn block_key_span(block: &str, key: &str) -> Option<(u32, u32, u32)> {
    block.lines().enumerate().find_map(|(i, line)| {
        let t = line.trim_start();
        (line.len() == t.len() && key_matches(t, key))
            .then(|| (i as u32 + 2, 1, 1 + key.chars().count() as u32))
    })
}

/// The 1-based SOURCE-FILE line of an immediate child `key` under top-level
/// `parent:` (best-effort). Scans from `parent:` to the next indent-0 key, matching
/// `key:` at any indent (including a leading `- ` sequence item).
fn nested_key_line(block: &str, parent: &str, key: &str) -> Option<u32> {
    nested_key_span(block, parent, key).map(|(l, _, _)| l)
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
    candidates
        .iter()
        .copied()
        .map(|k| (levenshtein(key, k), k))
        .filter(|&(d, _)| d > 0 && d <= 2)
        .min_by_key(|&(d, _)| d)
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
    fn flags_top_level_typo_with_suggestion_and_location() {
        let w = validate_front_matter("---\ntreme: darkly\ntitle: X\n---\n\nbody\n");
        assert_eq!(w.len(), 1, "got: {w:?}");
        assert_eq!(
            w[0].message,
            "unknown front-matter key `treme` (did you mean `theme`?)"
        );
        assert_eq!(w[0].line, Some(2), "`treme` is on file line 2");
    }

    #[test]
    fn theorems_numbered_is_recognized() {
        assert!(msgs("---\ntheorems:\n  numbered: false\n---\n").is_empty());
        assert!(msgs("---\ntheorems:\n  numbered: unless-unique\n---\n").is_empty());
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
        assert_eq!(w[0].message, "unknown theorems key `number-within`");
        assert_eq!(w[0].line, Some(3), "`number-within` is on file line 3");
    }

    #[test]
    fn theorems_flags_an_unrecognized_numbered_value() {
        // A value the parser silently ignores (renders the OPPOSITE of intent) must warn,
        // not pass a green check — `numbered` honors only a bool or `unless-unique`.
        let m = msgs("---\ntheorems:\n  numbered: never\n---\n");
        assert!(
            m.iter()
                .any(|w| w.contains("numbered") && w.contains("never")),
            "bad numbered value warns: {m:?}"
        );
    }

    #[test]
    fn theorems_accepts_every_valid_numbered_value() {
        assert!(
            msgs("---\ntheorems:\n  shared: [theorem, lemma]\n  numbered: unless-unique\n---\n")
                .is_empty(),
            "shared + unless-unique are valid"
        );
        assert!(
            msgs("---\ntheorems:\n  numbered: false\n---\n").is_empty(),
            "numbered: false (bool) is valid"
        );
        assert!(
            msgs("---\ntheorems:\n  numbered: true\n---\n").is_empty(),
            "numbered: true (bool) is valid"
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
        let m = msgs("---\ntitle: X\nexecute:\n  eccho: false\n  cache: true\n---\n");
        assert_eq!(
            m,
            vec!["unknown execute key `eccho` (did you mean `echo`?)"]
        );
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
            "---\ntitle: X\ntoc: true\nexecute:\n  echo: false\n  cache: true\nlisting:\n  contents: posts\n  type: grid\n---\n\nx\n",
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
        // let the unknown-key lint speak. That is a TRAP, and this pins why: `css` is edit
        // distance 1 from `csl`, so the did-you-mean would confidently tell the author to
        // rename their citation-style key to a STYLESHEET key. Wrong advice is worse than
        // the silence it replaces. `csl` therefore stays recognized, and
        // `diagnostics::csl_recognized_but_unsupported` is what speaks.
        assert_eq!(
            levenshtein("csl", "css"),
            1,
            "the hazard is edit distance 1"
        );
        let without_csl: Vec<&'static str> =
            KNOWN_KEYS.iter().copied().filter(|k| *k != "csl").collect();
        assert_eq!(
            closest("csl", &without_csl),
            Some("css"),
            "dropping `csl` from KNOWN_KEYS makes the did-you-mean suggest `css`"
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
            "---\ntitle: X\ntitle-block-style: none\ninclude-in-header:\n  text: \"<meta>\"\n---\n",
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
    fn prose_lint_key_is_recognized_and_nested_validated() {
        assert!(
            validate_front_matter("---\ntitle: T\nprose-lint: true\n---\n").is_empty(),
            "prose-lint should be a known top-level key"
        );
        let w = validate_front_matter("---\ntitle: T\nprose-lint:\n  bnned: [x]\n---\n");
        assert!(
            w.iter()
                .any(|x| x.message.contains("bnned") && x.message.contains("banned")),
            "nested prose-lint typo should be flagged, got: {w:?}"
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
}
