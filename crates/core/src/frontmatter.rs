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
    // Drafts: `draft: true` excludes a page from a website build (output, nav, listings).
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
    // Citations
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

/// `execute:` sub-keys taliesin honors (document-level cell defaults; see
/// `render::detect_execute_defaults`).
pub(crate) const EXECUTE_KEYS: &[&str] = &["echo", "include", "cache"];

/// `listing:` sub-keys taliesin honors (see `site::frontmatter::parse_listing_spec`).
pub(crate) const LISTING_KEYS: &[&str] =
    &["contents", "id", "sort", "type", "max-items", "categories"];

/// `about:` sub-keys taliesin honors (see `site::frontmatter::parse_about`).
pub(crate) const ABOUT_KEYS: &[&str] = &["template", "image", "image-alt", "links"];

/// `hero:` sub-keys taliesin honors (see `site::frontmatter::parse_hero`).
pub(crate) const HERO_KEYS: &[&str] = &["eyebrow", "headline", "lead", "actions"];

/// `prose-lint:` sub-keys taliesin honors (the mapping form; see `crate::prose::config`).
pub(crate) const PROSE_LINT_KEYS: &[&str] = &["banned"];

/// `theorems:` sub-keys taliesin honors: `shared` (shared counters), `number-within`
/// (chapter scoping), `numbered` (whether/when to number). The VALUES of the latter two
/// are checked by [`validate_theorem_values`] so an unrecognized value warns rather than
/// being silently ignored.
pub(crate) const THEOREM_KEYS: &[&str] = &["shared", "number-within", "numbered"];

/// Values `theorems.number-within` honors (see `render::fm_extract::parse_theorem_config`);
/// any other value is silently ignored by the parser, so it warns instead.
const THEOREM_NUMBER_WITHIN: &[&str] = &["chapter"];

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
/// `number-within`/`numbered` value (rendering the OPPOSITE of intent — e.g.
/// `numbered: never` stays numbered), so flag it with a did-you-mean rather than
/// certifying it on a green check. Mirrors the accepted set in
/// `render::fm_extract::parse_theorem_config`.
fn validate_theorem_values(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    let Some(serde_yaml::Value::Mapping(thm)) = map.get("theorems") else {
        return;
    };
    if let Some(v) = thm.get("number-within")
        && v.as_str() != Some("chapter")
    {
        let line = nested_key_line(block, "theorems", "number-within");
        out.push(located(
            unknown_value_message(
                "theorems number-within value",
                &value_label(v),
                THEOREM_NUMBER_WITHIN,
            ),
            line,
        ));
    }
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

/// `format: revealjs` / `*-revealjs` was the legacy deck spelling; it is no longer
/// accepted, so a doc naming it renders as a plain HTML page instead of a deck. Its
/// edit distance to `deck` is too large for the generic did-you-mean, so name the
/// migration explicitly. Reads the top-level `format` value in string, block-mapping
/// (keyed by the spelling), and sequence forms.
fn validate_format_value(map: &serde_yaml::Mapping, block: &str, out: &mut Vec<Warning>) {
    let Some(fmt) = map.get("format") else {
        return;
    };
    let named_revealjs = |s: &str| {
        let s = s.trim().trim_matches(['"', '\'']);
        s == "revealjs" || s.ends_with("-revealjs")
    };
    let hit = match fmt {
        serde_yaml::Value::String(s) => named_revealjs(s).then(|| s.clone()),
        serde_yaml::Value::Mapping(m) => m
            .keys()
            .filter_map(|k| k.as_str())
            .find(|k| named_revealjs(k))
            .map(str::to_string),
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str())
            .find(|s| named_revealjs(s))
            .map(str::to_string),
        _ => None,
    };
    if let Some(spelling) = hit {
        let line = block_key_line(block, "format");
        out.push(located(
            format!("unknown format `{spelling}` (did you mean `deck`?)"),
            line,
        ));
    }
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

/// The 1-based SOURCE-FILE line of a top-level front-matter key (best-effort). The
/// block starts on the file line after the opening `---`, so block line index `i` is
/// file line `i + 2`. `None` if the key is not on its own line (e.g. a flow mapping).
fn block_key_line(block: &str, key: &str) -> Option<u32> {
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

    #[test]
    fn theorems_number_within_is_recognized() {
        assert!(
            msgs("---\ntheorems:\n  number-within: chapter\n---\n").is_empty(),
            "number-within is a recognized theorems key"
        );
    }

    #[test]
    fn theorems_flags_unrecognized_number_within_and_numbered_values() {
        // A value the parser silently ignores (renders the OPPOSITE of intent) must warn,
        // not pass a green check — `number-within` honors only `chapter`, `numbered` only
        // a bool or `unless-unique`.
        let m = msgs("---\ntheorems:\n  number-within: section\n  numbered: never\n---\n");
        assert!(
            m.iter()
                .any(|w| w.contains("number-within") && w.contains("section")),
            "bad number-within value warns: {m:?}"
        );
        assert!(
            m.iter()
                .any(|w| w.contains("numbered") && w.contains("never")),
            "bad numbered value warns: {m:?}"
        );
    }

    #[test]
    fn theorems_accepts_every_valid_number_within_and_numbered_value() {
        assert!(
            msgs("---\ntheorems:\n  number-within: chapter\n  numbered: unless-unique\n---\n")
                .is_empty(),
            "chapter + unless-unique are valid"
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
