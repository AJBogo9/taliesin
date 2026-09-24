//! IEEE per-type reference formatting (`Bibliography::format` + the `fmt_*` helpers).

use super::author::format_authors;
use super::clean::{clean, clean_url};
use super::{Bibliography, Fields};
use crate::render::escape_attr as esc;

impl Bibliography {
    /// Format one entry as an IEEE reference string (HTML). `None` if unknown.
    /// IEEE varies by entry type: article = quoted title + italic journal +
    /// vol/no/pp; book = italic title + edition + publisher; everything else
    /// (misc/online) = quoted title + `[Online]. Available:` link.
    pub(crate) fn format(&self, key: &str) -> Option<String> {
        let e = self.entries.get(key)?;
        let f = &e.fields;
        let body = match e.kind.as_str() {
            "article" => fmt_article(f),
            // A chapter in a book/collection, or a paper in conference proceedings:
            // quoted title + "in <booktitle>" + pages. `@inproceedings`/`@conference`
            // are the commonest CS/ML type; without this they fell to `fmt_misc` and
            // silently dropped `booktitle` + `pages`. Falls back to plain-book (chapter
            // types) or misc (proceedings) formatting when no `booktitle` is set.
            "inbook" | "incollection" | "inproceedings" | "conference"
                if f.contains_key("booktitle") =>
            {
                fmt_inbook(f)
            }
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
        segs.push(pages(p));
    }
    if let Some(y) = f.get("year").filter(|s| !s.is_empty()) {
        segs.push(esc(&clean(y)));
    }
    let mut out = title_with_segs(quoted_title(f), &segs);
    append_url(&mut out, f);
    out
}

/// Join a quoted title (`"Title,"`) with trailing IEEE segments (venue/year/…),
/// adding the final period. When nothing follows, the dangling comma inside the
/// closing quote becomes a period (`"Title."`) instead of `"Title,".`, and a title that
/// already ends a sentence (`"Is This the End?"`) takes nothing.
fn title_with_segs(mut out: String, segs: &[String]) -> String {
    if segs.is_empty() {
        if let Some(stripped) = out.strip_suffix(",\u{201d}") {
            out = format!("{stripped}.\u{201d}");
        } else if !out.is_empty() && !ends_sentence(&out) {
            out.push('.');
        }
    } else {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&segs.join(", "));
        out.push('.');
    }
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

/// IEEE chapter (`@inbook`/`@incollection` WITH a `booktitle`):
/// `"Chapter," in <Booktitle>, Nth ed. City: Publisher, Year, pp. X–Y.`
/// The chapter title is quoted (like an article); the containing work is italic.
fn fmt_inbook(f: &Fields) -> String {
    // `"Chapter," in <Booktitle>`
    let mut out = quoted_title(f);
    if let Some(bt) = f.get("booktitle").filter(|s| !s.is_empty()) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&format!("in <em>{}</em>", esc(&clean(bt))));
    }
    if let Some(ed) = f.get("edition").filter(|s| !s.is_empty()) {
        out.push_str(&format!(", {} ed.", ordinal(&clean(ed))));
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
    if let Some(p) = f.get("pages").filter(|s| !s.is_empty()) {
        segs.push(pages(p));
    }
    // After the italic booktitle (which ends in `</em>`), a comma separates the
    // publisher/year/pages list; the whole entry ends with a period.
    if !segs.is_empty() {
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(&segs.join(", "));
    }
    if !out.is_empty() && !out.ends_with('.') {
        out.push('.');
    }
    append_url(&mut out, f);
    out
}

/// IEEE misc / online (the fallback): `"Title," Year. [Online]. Available: URL.`
fn fmt_misc(f: &Fields) -> String {
    let mut segs: Vec<String> = Vec::new();
    // A `@dataset`/`@online` often carries the issuing body (Kaggle, a standards org)
    // as publisher/organization/institution — keep it rather than drop it.
    if let Some(p) = f
        .get("publisher")
        .or_else(|| f.get("organization"))
        .or_else(|| f.get("institution"))
        .filter(|s| !s.is_empty())
    {
        segs.push(esc(&clean(p)));
    }
    if let Some(y) = f.get("year").filter(|s| !s.is_empty()) {
        segs.push(esc(&clean(y)));
    }
    let mut out = title_with_segs(quoted_title(f), &segs);
    append_url(&mut out, f);
    if let Some(note) = f.get("note").filter(|s| !s.is_empty()) {
        // Start a new sentence after a URL (which ends in `</a>`, not punctuation).
        if !out.is_empty() && !ends_sentence(&out) {
            out.push('.');
        }
        let note = esc(&clean(note));
        let stop = if ends_sentence(&note) { "" } else { "." };
        out.push_str(&format!(" {note}{stop}"));
    }
    out
}

/// Whether `s` already ends a sentence: a `.`, `?` or `!`, possibly inside a closing
/// quote, so no period is added after it.
fn ends_sentence(s: &str) -> bool {
    s.trim_end_matches('\u{201d}').ends_with(['.', '?', '!'])
}

/// A title in IEEE quotes with the trailing comma inside the closing quote
/// (`"Title,"`), ready for the venue/year to follow. Empty if no title. A title that
/// ends in `.`, `?` or `!` keeps that mark instead ("How Powerful are GNNs?"), which is
/// how IEEE prints it.
fn quoted_title(f: &Fields) -> String {
    match f
        .get("title")
        .map(|t| esc(&clean(t)))
        .filter(|t| !t.is_empty())
    {
        Some(t) if ends_sentence(&t) => format!("\u{201c}{t}\u{201d}"),
        Some(t) => format!("\u{201c}{t},\u{201d}"),
        None => String::new(),
    }
}

/// Append `[Online]. Available: <link>` from `url` (or a `\url{}` in
/// `howpublished`) when present.
fn append_url(out: &mut String, f: &Fields) {
    let url = f
        .get("url")
        .or_else(|| f.get("howpublished"))
        .map(|u| clean_url(u))
        .filter(|u| u.starts_with("http"));
    if let Some(u) = url {
        let u = esc(&u);
        out.push_str(&format!(" [Online]. Available: <a href=\"{u}\">{u}</a>"));
    }
}

/// The IEEE page segment: "p. 42" for one page, "pp. 123–145" for a range or a list.
fn pages(p: &str) -> String {
    let p = clean_pages(p);
    let label = if p.contains(['\u{2013}', ',', '+']) {
        "pp."
    } else {
        "p."
    };
    format!("{label} {}", esc(&p))
}

/// Page ranges use one en dash, however the range was written (`12-34`, `12--34`,
/// `12 -- 34`), as BibTeX's `n.dashify` does. Done before [`clean`], which would read
/// `---` as an em dash.
fn clean_pages(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if c == '-' {
            let run = rest.len() - rest.trim_start_matches('-').len();
            out.truncate(out.trim_end().len());
            out.push('\u{2013}');
            rest = rest[run..].trim_start();
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    clean(&out)
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
