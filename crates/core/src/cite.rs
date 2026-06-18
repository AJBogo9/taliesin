//! Pragmatic citations and cross-references.
//!
//! Citations (`[@key]`, `[@key, locator]`, `[@a; @b]`) become numbered links to
//! an auto-generated References section, formatted from a parsed BibTeX file.
//! Cross-references (`@fig-x`, `@sec-x`, ...) become links to their anchor,
//! labelled by kind and, when the anchor's number is known (e.g. a static
//! `#fig-` figure), carrying it ("Figure 3"). This is not a full CSL engine —
//! numbering for *computed* figures would arrive with execution.
//!
//! Processing runs over the already-rendered block HTML, transforming only
//! plain-text runs (never inside tags, code, or math), so block sourcepos is
//! untouched. The only structural change is appending a References block.

use crate::render::Block;
use std::collections::HashMap;

/// A parsed BibTeX database.
#[derive(Default)]
pub struct Bibliography {
    entries: HashMap<String, Entry>,
}

/// One parsed BibTeX entry: its `@type` (lowercased, e.g. `article`/`book`/
/// `misc`) plus field values. The type drives IEEE per-type formatting.
#[derive(Default)]
struct Entry {
    kind: String,
    fields: HashMap<String, String>,
}

type Fields = HashMap<String, String>;

impl Bibliography {
    /// Format one entry as an IEEE reference string (HTML). `None` if unknown.
    /// IEEE varies by entry type: article = quoted title + italic journal +
    /// vol/no/pp; book = italic title + edition + publisher; everything else
    /// (misc/online) = quoted title + `[Online]. Available:` link.
    fn format(&self, key: &str) -> Option<String> {
        let e = self.entries.get(key)?;
        let f = &e.fields;
        let body = match e.kind.as_str() {
            "article" => fmt_article(f),
            "book" | "inbook" | "incollection" => fmt_book(f),
            _ => fmt_misc(f),
        };
        // Authors lead the entry (IEEE: "A. B. Author, <rest>").
        let mut out = String::new();
        if let Some(a) = f
            .get("author")
            .map(|a| format_authors(a))
            .filter(|s| !s.is_empty())
        {
            out.push_str(&a);
            out.push_str(", ");
        }
        out.push_str(&body);
        Some(out)
    }
}

/// IEEE journal article: `"Title," Journal, vol. V, no. N, pp. P, Year.`
fn fmt_article(f: &Fields) -> String {
    let mut segs: Vec<String> = Vec::new();
    if let Some(j) = f.get("journal").filter(|s| !s.is_empty()) {
        segs.push(format!("<em>{}</em>", esc(&clean(j))));
    }
    if let Some(v) = f.get("volume").filter(|s| !s.is_empty()) {
        segs.push(format!("vol. {}", esc(&clean(v))));
    }
    if let Some(n) = f.get("number").filter(|s| !s.is_empty()) {
        segs.push(format!("no. {}", esc(&clean(n))));
    }
    if let Some(p) = f.get("pages").filter(|s| !s.is_empty()) {
        segs.push(format!("pp. {}", esc(&clean_pages(p))));
    }
    if let Some(y) = f.get("year").filter(|s| !s.is_empty()) {
        segs.push(esc(&clean(y)));
    }
    let mut out = quoted_title(f);
    if !segs.is_empty() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&segs.join(", "));
    }
    out.push('.');
    append_url(&mut out, f);
    out
}

/// IEEE book: `Title, Nth ed. City: Publisher, Year.` (title italic).
fn fmt_book(f: &Fields) -> String {
    let mut out = String::new();
    if let Some(t) = f.get("title").filter(|s| !s.is_empty()) {
        out.push_str(&format!("<em>{}</em>", esc(&clean(t))));
    }
    if let Some(ed) = f.get("edition").filter(|s| !s.is_empty()) {
        out.push_str(&format!(", {} ed.", ordinal(&clean(ed))));
    }
    // The edition already ends in a period ("ed."); don't double it.
    if !out.ends_with('.') {
        out.push('.');
    }
    let publisher = match (f.get("address"), f.get("publisher")) {
        (Some(a), Some(p)) if !a.is_empty() => format!("{}: {}", clean(a), clean(p)),
        (_, Some(p)) => clean(p),
        _ => String::new(),
    };
    let mut segs: Vec<String> = Vec::new();
    if !publisher.is_empty() {
        segs.push(esc(&publisher));
    }
    if let Some(y) = f.get("year").filter(|s| !s.is_empty()) {
        segs.push(esc(&clean(y)));
    }
    if !segs.is_empty() {
        out.push(' ');
        out.push_str(&segs.join(", "));
        out.push('.');
    }
    append_url(&mut out, f);
    out
}

/// IEEE misc / online (the fallback): `"Title," Year. [Online]. Available: URL.`
fn fmt_misc(f: &Fields) -> String {
    let mut out = quoted_title(f);
    if let Some(y) = f.get("year").filter(|s| !s.is_empty()) {
        out.push_str(&format!(" {}", esc(&clean(y))));
    }
    out.push('.');
    append_url(&mut out, f);
    if let Some(note) = f.get("note").filter(|s| !s.is_empty()) {
        // Start a new sentence after a URL (which ends in `</a>`, not punctuation).
        if !out.ends_with(['.', ' ']) {
            out.push('.');
        }
        out.push_str(&format!(" {}.", esc(&clean(note))));
    }
    out
}

/// A title in IEEE quotes with the trailing comma inside the closing quote
/// (`"Title,"`), ready for the venue/year to follow. Empty if no title.
fn quoted_title(f: &Fields) -> String {
    match f.get("title").filter(|s| !s.is_empty()) {
        Some(t) => format!("\u{201c}{},\u{201d}", esc(&clean(t))),
        None => String::new(),
    }
}

/// Append `[Online]. Available: <link>` from `url` (or a `\url{}` in
/// `howpublished`) when present.
fn append_url(out: &mut String, f: &Fields) {
    let url = f
        .get("url")
        .or_else(|| f.get("howpublished"))
        .map(|u| clean(u))
        .filter(|u| u.starts_with("http"));
    if let Some(u) = url {
        let u = esc(&u);
        out.push_str(&format!(" [Online]. Available: <a href=\"{u}\">{u}</a>"));
    }
}

/// Strip BibTeX/LaTeX cruft from a field value: `\url{}` wrappers, brace groups
/// (capitalization guards), and the common backslash escapes.
fn clean(s: &str) -> String {
    let s = s
        .replace("\\url", "")
        .replace(['{', '}'], "")
        .replace("\\&", "&")
        .replace("\\%", "%")
        .replace("\\_", "_")
        .replace("\\#", "#")
        .replace("\\$", "$");
    s.trim().to_string()
}

/// Page ranges use an en dash (`12--34` -> `12\u{2013}34`).
fn clean_pages(s: &str) -> String {
    clean(s)
        .replace("---", "\u{2013}")
        .replace("--", "\u{2013}")
}

/// `4` -> `4th`, `21` -> `21st`; passes non-numeric editions through unchanged.
fn ordinal(s: &str) -> String {
    match s.trim().parse::<u32>() {
        Ok(n) => {
            let suffix = if (11..=13).contains(&(n % 100)) {
                "th"
            } else {
                match n % 10 {
                    1 => "st",
                    2 => "nd",
                    3 => "rd",
                    _ => "th",
                }
            };
            format!("{n}{suffix}")
        }
        Err(_) => s.to_string(),
    }
}

/// Parse a BibTeX string into a [`Bibliography`]. Tolerant of `{...}`/`"..."`
/// values and brace nesting; ignores comments and `@string`/`@comment`.
pub fn parse_bib(text: &str) -> Bibliography {
    let mut entries = HashMap::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        i += 1;
        let kind = take_while(&chars, &mut i, |c| c.is_alphanumeric()).to_ascii_lowercase();
        skip_ws(&chars, &mut i);
        if i >= chars.len() || chars[i] != '{' {
            continue;
        }
        i += 1; // past '{'
        if kind == "comment" || kind == "string" || kind == "preamble" {
            skip_entry(&chars, &mut i);
            continue;
        }
        let key = take_while(&chars, &mut i, |c| c != ',' && c != '}')
            .trim()
            .to_string();
        let mut fields = HashMap::new();
        if i < chars.len() && chars[i] == ',' {
            i += 1;
        }
        loop {
            skip_ws(&chars, &mut i);
            if i >= chars.len() || chars[i] == '}' {
                break;
            }
            let name = take_while(&chars, &mut i, |c| c != '=' && c != '}' && c != ',')
                .trim()
                .to_ascii_lowercase();
            skip_ws(&chars, &mut i);
            if i >= chars.len() || chars[i] != '=' {
                break;
            }
            i += 1; // past '='
            skip_ws(&chars, &mut i);
            let value = read_value(&chars, &mut i);
            if !name.is_empty() {
                fields.insert(name, value);
            }
            skip_ws(&chars, &mut i);
            if i < chars.len() && chars[i] == ',' {
                i += 1;
            }
        }
        if i < chars.len() && chars[i] == '}' {
            i += 1;
        }
        if !key.is_empty() {
            entries.insert(key, Entry { kind, fields });
        }
    }
    Bibliography { entries }
}

fn take_while(chars: &[char], i: &mut usize, pred: impl Fn(char) -> bool) -> String {
    let start = *i;
    while *i < chars.len() && pred(chars[*i]) {
        *i += 1;
    }
    chars[start..*i].iter().collect()
}

fn skip_ws(chars: &[char], i: &mut usize) {
    while *i < chars.len() && chars[*i].is_whitespace() {
        *i += 1;
    }
}

fn skip_entry(chars: &[char], i: &mut usize) {
    let mut depth = 1;
    while *i < chars.len() && depth > 0 {
        match chars[*i] {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        *i += 1;
    }
}

/// Read a field value: `{...}` (brace-nested), `"..."`, or a bare token.
fn read_value(chars: &[char], i: &mut usize) -> String {
    let mut out = String::new();
    match chars.get(*i) {
        Some('{') => {
            let mut depth = 0;
            while *i < chars.len() {
                match chars[*i] {
                    '{' => {
                        depth += 1;
                        if depth > 1 {
                            out.push('{');
                        }
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            *i += 1;
                            break;
                        }
                        out.push('}');
                    }
                    c => out.push(c),
                }
                *i += 1;
            }
        }
        Some('"') => {
            *i += 1;
            while *i < chars.len() && chars[*i] != '"' {
                out.push(chars[*i]);
                *i += 1;
            }
            if *i < chars.len() {
                *i += 1;
            }
        }
        _ => {
            out = take_while(chars, i, |c| c != ',' && c != '}');
        }
    }
    normalize_ws(&out)
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// IEEE author list (HTML). Initials precede the surname ("C. M. Bishop"); a
/// `\u{201c}{Corporate Name}\u{201d}`-style braced author stays literal. Per the
/// shipped ieee.csl (`et-al-min=7`, `et-al-use-first=1`), seven or more authors
/// (or a trailing BibTeX `and others`) collapse to the first author + italic
/// "et al.". Otherwise: "A and B" for two, "A, B, and C" (Oxford comma) for more.
fn format_authors(raw: &str) -> String {
    let mut names: Vec<&str> = raw
        .split(" and ")
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
    let people: Vec<String> = names.iter().map(|n| esc(&format_one_author(n))).collect();
    let mut out = join_authors(&people);
    if et_al {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str("<em>et al.</em>");
    }
    out
}

/// One author -> "F. M. Surname". Handles "Surname, First Mid", "First Mid
/// Surname", and a brace-wrapped corporate name (kept verbatim).
fn format_one_author(name: &str) -> String {
    let name = name.trim();
    if name.starts_with('{') {
        return clean(name);
    }
    if let Some((last, first)) = name.split_once(',') {
        format!("{}{}", initials(first), last.trim())
    } else {
        let words: Vec<&str> = name.split_whitespace().collect();
        match words.split_last() {
            Some((last, firsts)) if !firsts.is_empty() => {
                format!("{}{last}", initials(&firsts.join(" ")))
            }
            _ => name.to_string(),
        }
    }
}

/// First/middle names -> space-terminated initials: "Daniel M." -> "D. M. ".
fn initials(first: &str) -> String {
    first
        .split_whitespace()
        .filter_map(|w| w.chars().find(|c| c.is_alphabetic()))
        .map(|c| format!("{}. ", c.to_uppercase()))
        .collect()
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

/// Cross-reference kind prefixes -> display label.
fn xref_label(prefix: &str) -> Option<&'static str> {
    match prefix {
        "fig" => Some("Figure"),
        "tbl" => Some("Table"),
        "sec" => Some("Section"),
        "eq" => Some("Equation"),
        "lst" => Some("Listing"),
        "thm" => Some("Theorem"),
        "def" => Some("Definition"),
        _ => None,
    }
}

/// Resolve citations + cross-references across `blocks`, appending a References
/// block when citations were found and the bibliography could format them.
/// `xrefs` maps a cross-reference anchor (e.g. `fig-scree`) to its resolved
/// number, so `@fig-scree` renders as a linked "Figure 3".
pub fn process(blocks: &mut Vec<Block>, bib: &Bibliography, xrefs: &HashMap<String, String>) {
    let mut order: Vec<String> = Vec::new();
    let mut number: HashMap<String, usize> = HashMap::new();
    let mut cite_key = |key: &str| -> usize {
        *number.entry(key.to_string()).or_insert_with(|| {
            order.push(key.to_string());
            order.len()
        })
    };

    for b in blocks.iter_mut() {
        b.html = transform_html(&b.html, &mut cite_key, xrefs);
    }

    if order.is_empty() {
        return;
    }
    let mut list = String::from(
        "<section class=\"qmd-references\" data-block-id=\"qmd-references\"><h2>References</h2>",
    );
    for (idx, key) in order.iter().enumerate() {
        let formatted = bib
            .format(key)
            .unwrap_or_else(|| format!("<code>{}</code>", esc(key)));
        list.push_str(&format!(
            "<div id=\"ref-{}\" class=\"csl-entry\">[{}] {}</div>",
            esc(key),
            idx + 1,
            formatted
        ));
    }
    list.push_str("</section>");
    blocks.push(Block {
        id: "qmd-references".to_string(),
        sourcepos: String::new(),
        source_file: None,
        html: list,
        cell: None,
    });
}

/// Walk HTML, transforming only plain-text runs (never inside tags or inside
/// `pre`/`code`/`script`/`style`/`annotation` elements).
fn transform_html(
    html: &str,
    cite_key: &mut impl FnMut(&str) -> usize,
    xrefs: &HashMap<String, String>,
) -> String {
    const SKIP: [&str; 5] = ["pre", "code", "script", "style", "annotation"];
    let mut out = String::with_capacity(html.len());
    let mut skip_depth = 0usize;
    let mut rest = html;
    while !rest.is_empty() {
        if rest.starts_with('<') {
            let end = rest.find('>').map(|e| e + 1).unwrap_or(rest.len());
            let tag = &rest[..end];
            let name: String = tag
                .trim_start_matches(['<', '/'])
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            if SKIP.contains(&name.as_str()) {
                if tag.starts_with("</") {
                    skip_depth = skip_depth.saturating_sub(1);
                } else if !tag.ends_with("/>") {
                    skip_depth += 1;
                }
            }
            out.push_str(tag);
            rest = &rest[end..];
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let text = &rest[..end];
            if skip_depth == 0 {
                out.push_str(&rewrite_text(text, cite_key, xrefs));
            } else {
                out.push_str(text);
            }
            rest = &rest[end..];
        }
    }
    out
}

/// Rewrite citations and cross-references within a plain-text run.
fn rewrite_text(
    text: &str,
    cite_key: &mut impl FnMut(&str) -> usize,
    xrefs: &HashMap<String, String>,
) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            if let Some(close) = chars[i + 1..].iter().position(|&c| c == ']') {
                let inner: String = chars[i + 1..i + 1 + close].iter().collect();
                if inner.contains('@') {
                    out.push_str(&render_citation_group(&inner, cite_key));
                    i += close + 2;
                    continue;
                }
            }
        } else if chars[i] == '@'
            && let Some((label, anchor, len)) = parse_xref(&chars[i..])
        {
            // A resolved number renders "Figure&nbsp;3"; otherwise just the label.
            let text = match xrefs.get(&anchor) {
                Some(n) => format!("{label}&nbsp;{n}"),
                None => label.to_string(),
            };
            out.push_str(&format!(
                "<a href=\"#{anchor}\" class=\"qmd-xref\">{text}</a>"
            ));
            i += len;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// `@fig-x` -> ("Figure", "fig-x", consumed_len).
fn parse_xref(chars: &[char]) -> Option<(&'static str, String, usize)> {
    // chars[0] == '@'
    let rest: String = chars[1..].iter().collect();
    let prefix: String = rest
        .chars()
        .take_while(|c| c.is_ascii_lowercase())
        .collect();
    let label = xref_label(&prefix)?;
    let after = &rest[prefix.len()..];
    if !after.starts_with('-') {
        return None;
    }
    let ident: String = after[1..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if ident.is_empty() {
        return None;
    }
    let anchor = format!("{prefix}-{ident}");
    let consumed = 1 + prefix.len() + 1 + ident.len();
    Some((label, anchor, consumed))
}

/// Render `@a; @b, p. 5` style citation group content into `[1, 2, p. 5]`.
fn render_citation_group(inner: &str, cite_key: &mut impl FnMut(&str) -> usize) -> String {
    let mut rendered: Vec<String> = Vec::new();
    for item in inner.split(';') {
        let item = item.trim().trim_start_matches('-'); // `-@key` suppresses author (n/a for numeric)
        let Some(at) = item.find('@') else { continue };
        let after = &item[at + 1..];
        let key: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == ':')
            .collect();
        if key.is_empty() {
            continue;
        }
        let locator = after[key.len()..].trim().trim_start_matches(',').trim();
        let n = cite_key(&key);
        let mut piece = format!("<a href=\"#ref-{}\">{}</a>", esc(&key), n);
        if !locator.is_empty() {
            piece.push_str(&format!(", {}", esc(locator)));
        }
        rendered.push(piece);
    }
    if rendered.is_empty() {
        format!("[{}]", esc(inner))
    } else {
        format!("[{}]", rendered.join(", "))
    }
}

fn esc(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bib() -> Bibliography {
        parse_bib(
            "@book{bishop2006pattern,\n  title = {Pattern Recognition and Machine Learning},\n  author = {Bishop, Christopher M},\n  year = {2006},\n  publisher = {Springer}\n}\n",
        )
    }

    #[test]
    fn parses_and_formats_entry() {
        let b = bib();
        let f = b.format("bishop2006pattern").unwrap();
        assert!(f.contains("C. M. Bishop"), "got: {f}");
        assert!(f.contains("<em>Pattern Recognition and Machine Learning</em>"));
        assert!(f.contains("Springer") && f.contains("2006"));
    }

    #[test]
    fn article_is_ieee_quoted_title_italic_journal_and_et_al() {
        let b = parse_bib(
            "@article{k,\n author = {Ziegler, Daniel M. and Stiennon, Nisan and Wu, Jeffrey and Brown, Tom B. and Radford, Alec and Amodei, Dario and Christiano, Paul and Irving, Geoffrey},\n title = {Fine-Tuning Language Models},\n journal = {arXiv preprint arXiv:1909.08593},\n year = {2019},\n url = {https://arxiv.org/abs/1909.08593}\n}\n",
        );
        let f = b.format("k").unwrap();
        // 8 authors -> first + italic et al.; article title quoted; journal italic.
        assert!(f.starts_with("D. M. Ziegler <em>et al.</em>, "), "got: {f}");
        assert!(
            f.contains("\u{201c}Fine-Tuning Language Models,\u{201d}"),
            "got: {f}"
        );
        assert!(
            f.contains("<em>arXiv preprint arXiv:1909.08593</em>, 2019."),
            "got: {f}"
        );
        assert!(
            f.contains("[Online]. Available: <a href=\"https://arxiv.org/abs/1909.08593\">"),
            "got: {f}"
        );
    }

    #[test]
    fn book_with_edition_is_ieee_ordinal() {
        let b = parse_bib(
            "@book{r,\n author = {Russell, Stuart and Norvig, Peter},\n title = {Artificial Intelligence: A Modern Approach},\n edition = {4},\n publisher = {Pearson},\n year = {2022}\n}\n",
        );
        let f = b.format("r").unwrap();
        assert_eq!(
            f,
            "S. Russell and P. Norvig, <em>Artificial Intelligence: A Modern Approach</em>, 4th ed. Pearson, 2022."
        );
    }

    #[test]
    fn misc_online_uses_howpublished_url_and_corporate_author() {
        let b = parse_bib(
            "@misc{w,\n author = {{Wikipedia contributors}},\n title = {Analysis of variance},\n howpublished = {\\url{https://en.wikipedia.org/wiki/Analysis_of_variance}},\n year = {2025},\n note = {Accessed: 2026-04-25}\n}\n",
        );
        let f = b.format("w").unwrap();
        // Braced corporate author stays literal (no initials); \url{} unwrapped.
        assert!(f.starts_with("Wikipedia contributors, "), "got: {f}");
        assert!(
            f.contains("\u{201c}Analysis of variance,\u{201d} 2025."),
            "got: {f}"
        );
        assert!(
            f.contains(
                "[Online]. Available: <a href=\"https://en.wikipedia.org/wiki/Analysis_of_variance\">"
            ),
            "got: {f}"
        );
        assert!(f.trim_end().ends_with("Accessed: 2026-04-25."), "got: {f}");
    }

    #[test]
    fn and_others_collapses_to_et_al() {
        let b = parse_bib(
            "@article{o,\n author = {Ouyang, Long and Wu, Jeffrey and others},\n title = {T},\n journal = {J},\n year = {2022}\n}\n",
        );
        let f = b.format("o").unwrap();
        assert!(f.starts_with("L. Ouyang <em>et al.</em>, "), "got: {f}");
        assert!(!f.contains("others"), "literal 'others' leaked: {f}");
    }

    #[test]
    fn citation_becomes_numbered_link_with_locator() {
        let b = bib();
        let mut blocks = vec![Block {
            id: "x".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<p>fails [@bishop2006pattern, chap. 9].</p>".into(),
            cell: None,
        }];
        process(&mut blocks, &b, &HashMap::new());
        assert!(
            blocks[0]
                .html
                .contains("[<a href=\"#ref-bishop2006pattern\">1</a>, chap. 9]")
        );
        // a References section was appended
        let refs = blocks.last().unwrap();
        assert!(refs.html.contains("id=\"ref-bishop2006pattern\""));
        assert!(refs.html.contains("[1] C. M. Bishop"));
    }

    #[test]
    fn crossref_becomes_labelled_link() {
        let b = Bibliography::default();
        let mut blocks = vec![Block {
            id: "x".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<p>see @fig-scree for details</p>".into(),
            cell: None,
        }];
        process(&mut blocks, &b, &HashMap::new());
        assert!(
            blocks[0]
                .html
                .contains("<a href=\"#fig-scree\" class=\"qmd-xref\">Figure</a>"),
            "got: {}",
            blocks[0].html
        );
        // no citations -> no References section
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn crossref_resolves_number_from_registry() {
        let mut xrefs = HashMap::new();
        xrefs.insert("fig-scree".to_string(), "3".to_string());
        let mut blocks = vec![Block {
            id: "x".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<p>see @fig-scree for the elbow</p>".into(),
            cell: None,
        }];
        process(&mut blocks, &Bibliography::default(), &xrefs);
        assert!(
            blocks[0]
                .html
                .contains("<a href=\"#fig-scree\" class=\"qmd-xref\">Figure&nbsp;3</a>"),
            "got: {}",
            blocks[0].html
        );
    }

    #[test]
    fn citations_inside_code_are_left_alone() {
        let b = bib();
        let mut blocks = vec![Block {
            id: "x".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<pre><code>x = [@bishop2006pattern]</code></pre>".into(),
            cell: None,
        }];
        process(&mut blocks, &b, &HashMap::new());
        assert!(
            blocks[0].html.contains("[@bishop2006pattern]"),
            "code was rewritten"
        );
        assert_eq!(blocks.len(), 1, "no citation should have been counted");
    }
}
