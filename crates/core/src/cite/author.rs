//! IEEE author-list formatting (initials, et al. collapsing, corporate names).

use super::clean::clean;
use crate::render::escape_attr as esc;

/// IEEE author list (HTML). Initials precede the surname ("C. M. Bishop"); a
/// `\u{201c}{Corporate Name}\u{201d}`-style braced author stays literal. Per the
/// shipped ieee.csl (`et-al-min=7`, `et-al-use-first=1`), seven or more authors
/// (or a trailing BibTeX `and others`) collapse to the first author + italic
/// "et al.". Otherwise: "A and B" for two, "A, B, and C" (Oxford comma) for more.
pub(crate) fn format_authors(raw: &str) -> String {
    let mut names: Vec<&str> = split_on_and(raw)
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut et_al = false;
    if names
        .last()
        .is_some_and(|n| n.eq_ignore_ascii_case("others"))
    {
        names.pop();
        et_al = true;
    }
    if et_al || names.len() >= 7 {
        et_al = true;
        names.truncate(1);
    }
    // Drop any name that formats to nothing (a stray `,`/brace), so a malformed
    // entry can't leak an empty slot like "A, , and B".
    let people: Vec<String> = names
        .iter()
        .map(|n| esc(&format_one_author(n)))
        .filter(|s| !s.trim().is_empty())
        .collect();
    let mut out = join_authors(&people);
    if et_al {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str("<em>et al.</em>");
    }
    out
}

/// Split a BibTeX author list on its ` and ` separator at **brace depth 0**, the same
/// depth-counting idiom `parse.rs` reads field values with.
///
/// A brace-protected corporate name may contain the conjunction itself, and a plain
/// `split(" and ")` tore it in half and then formatted each half as a person:
/// `{{Food and Drug Administration}}` published as "Food and D. Administration". Silent —
/// the key resolves and nothing validates a formatted name — and it hits every agency
/// spelled this way (the FDA, "Centers for Disease Control and Prevention", …).
fn split_on_and(raw: &str) -> Vec<&str> {
    const SEP: &str = " and ";
    let mut out = Vec::new();
    let (mut depth, mut start, mut skip_to) = (0usize, 0usize, 0usize);
    for (i, c) in raw.char_indices() {
        if i < skip_to {
            continue;
        }
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 && raw[i..].starts_with(SEP) => {
                out.push(&raw[start..i]);
                skip_to = i + SEP.len();
                start = skip_to;
            }
            _ => {}
        }
    }
    out.push(&raw[start..]);
    out
}

/// One author -> "F. M. Surname". Handles "Surname, First Mid", "First Mid
/// Surname", and a brace-wrapped corporate name (kept verbatim).
///
/// Corporate means the WHOLE name is one brace group. Starting with `{` is not enough:
/// that is how every exporter writes an accent on the first letter (`{\"O}zt{\"u}rk`).
fn format_one_author(name: &str) -> String {
    let name = name.trim();
    if super::parse::one_brace_group(name).is_some() {
        return clean(name);
    }
    if let Some((last, first)) = name.split_once(',') {
        format!("{}{}", initials(first), clean(last.trim()))
    } else {
        let words = words(name);
        match words.split_last() {
            Some((last, firsts)) if !firsts.is_empty() => {
                format!("{}{}", initials(&firsts.join(" ")), clean(last))
            }
            _ => clean(name),
        }
    }
}

/// First/middle names -> space-terminated initials: "Daniel M." -> "D. M. ".
/// Each word is `clean`ed first so an accented initial (`{\'E}mile` -> `Émile`)
/// initials as its Unicode letter (`É.`), not a stray brace/backslash.
fn initials(first: &str) -> String {
    words(first)
        .into_iter()
        .filter_map(|w| clean(w).chars().find(|c| c.is_alphabetic()))
        .map(|c| format!("{}. ", c.to_uppercase()))
        .collect()
}

/// The words of a name part, split at whitespace outside braces: a brace group is part
/// of its word even when it holds a space, as Better BibTeX's `Ay{\c s}e` does.
fn words(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0usize, None);
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
                if let Some(st) = start.take() {
                    out.push(&s[st..i]);
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(i);
    }
    if let Some(st) = start {
        out.push(&s[st..]);
    }
    out
}

/// Join names IEEE-style: "" / "A" / "A and B" / "A, B, and C".
fn join_authors(people: &[String]) -> String {
    match people {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} and {b}"),
        [head @ .., last] => format!("{}, and {last}", head.join(", ")),
    }
}
