//! Front-matter schema validation.
//!
//! taliesin reads a leading YAML `---` block and validates it against its OWN
//! recognized key set. A key taliesin does not
//! implement, whether a typo like `treme:` or a legacy term it does not honor, is
//! flagged by [`validate_front_matter`]: every unknown top-level key, plus every
//! unknown immediate child of the nested `execute:` / `listing:` / `about:` /
//! `hero:` blocks, each suggesting the closest known key. It only warns (located
//! for click-to-source); rendering is unaffected, an unknown key still renders.

use crate::render::Warning;

/// Top-level front-matter keys taliesin recognizes: the closed set of keys it
/// actually implements, plus every key the corpus/docs use. Intentionally tight
/// (the native flip), so a key taliesin doesn't implement, or a typo,
/// now warns instead of being silently ignored. Top-level keys plus the immediate
/// children of `execute:` / `listing:` / `about:` / `hero:` are linted; `format:`
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
    "about",
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

/// `about:` sub-keys taliesin honors (see `site::frontmatter::parse_about`).
pub(crate) const ABOUT_KEYS: &[&str] = &["template", "image", "image-alt", "links"];

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
/// `listing:`, `about:`, and `hero:` blocks. Membership is decided by a real YAML
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
            let line = block_key_line(block, key);
            out.push(located(
                unknown_key_message("front-matter key", key, KNOWN_KEYS),
                line,
            ));
        }
    }
    validate_format_value(map, block, &mut out);
    validate_unsupported_keys(map, block, &mut out);
    validate_page_layout_value(map, block, &mut out);
    validate_nested(map, "execute", "execute key", EXECUTE_KEYS, block, &mut out);
    validate_nested(map, "about", "about key", ABOUT_KEYS, block, &mut out);
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

/// A `Warning` for `message`, located when `line` is `Some` (file `None` = the
/// previewed doc, the client falls back to its path).
fn located(message: String, line: Option<u32>) -> Warning {
    match line {
        Some(l) => Warning::new(message).at(None, l),
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
            let line = nested_key_line(block, parent, key);
            out.push(located(unknown_key_message(what, key, allowed), line));
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
        out.push(located(
            format!(
                "`{key}:` is recognized but not supported, so it has no effect: references \
                 always render in the built-in IEEE style (remove the key, or the citations \
                 will not match the style you asked for)"
            ),
            block_key_line(block, key),
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
    block.lines().enumerate().find_map(|(i, line)| {
        let t = line.trim_start();
        (line.len() == t.len() && key_matches(t, key)).then_some(i as u32 + 2)
    })
}

/// The 1-based SOURCE-FILE line of an immediate child `key` under top-level
/// `parent:` (best-effort). Scans from `parent:` to the next indent-0 key, matching
/// `key:` at any indent (including a leading `- ` sequence item).
fn nested_key_line(block: &str, parent: &str, key: &str) -> Option<u32> {
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
        let body = t.strip_prefix("- ").map(str::trim_start).unwrap_or(t);
        if key_matches(body, key) {
            return Some(i as u32 + 2);
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
    fn flags_unknown_about_and_hero_children() {
        let a = msgs("---\ntitle: X\nabout:\n  template: jolla\n  imagee: me.png\n---\n");
        assert_eq!(
            a,
            vec!["unknown about key `imagee` (did you mean `image`?)"]
        );
        let h = msgs("---\ntitle: X\nhero:\n  headlin: Hi\n---\n");
        assert_eq!(
            h,
            vec!["unknown hero key `headlin` (did you mean `headline`?)"]
        );
    }

    #[test]
    fn clean_doc_with_nested_blocks_has_no_warnings() {
        let w = validate_front_matter(
            "---\ntitle: X\ntoc: true\nexecute:\n  echo: false\n  cache: true\nlisting:\n  contents: posts\n  type: grid\nabout:\n  template: jolla\n  links:\n    - text: GH\n      href: https://x\n---\n\nx\n",
        );
        assert!(w.is_empty(), "got: {w:?}");
    }

    #[test]
    fn format_subkeys_are_not_linted() {
        // `format:` sub-keys (a deck's `revealjs:`/`deck:` options) are format config,
        // not top-level keys, so they must not warn.
        let w = validate_front_matter(
            "---\ntitle: X\nformat:\n  html:\n    toc: true\n    anything: 1\n---\n",
        );
        assert!(w.is_empty(), "got: {w:?}");
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
}
