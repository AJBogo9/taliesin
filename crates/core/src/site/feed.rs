//! RSS 2.0 feed generation for a website. The feed lists the site's posts (pages
//! under `posts/`), newest first. Books and sites without a configured `url:`
//! produce no feed, since RSS items need absolute links.

use super::{Page, Site};

/// Build an RSS 2.0 document for the site's posts, or `None` when there's nothing
/// to syndicate: a book, no `url:` configured, or no posts.
pub(super) fn rss(site: &Site) -> Option<String> {
    if site.is_book() {
        return None;
    }
    let base = site.config.url.as_deref()?.trim_end_matches('/');
    if base.is_empty() {
        return None;
    }

    // Posts, newest first. ISO dates sort lexically; undated posts sort last.
    let mut posts: Vec<&Page> = site.pages.iter().filter(|p| p.is_post).collect();
    if posts.is_empty() {
        return None;
    }
    posts.sort_by(|a, b| {
        b.date
            .as_deref()
            .unwrap_or("")
            .cmp(a.date.as_deref().unwrap_or(""))
    });

    let title = site.config.title.as_deref().unwrap_or("");
    let description = site.config.description.as_deref().unwrap_or(title);

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n");
    out.push_str("<channel>\n");
    out.push_str(&format!("<title>{}</title>\n", xml(title)));
    out.push_str(&format!("<link>{}/</link>\n", xml(base)));
    out.push_str(&format!(
        "<description>{}</description>\n",
        xml(description)
    ));
    out.push_str("<generator>qmd-fast</generator>\n");
    out.push_str(&format!(
        "<atom:link href=\"{}/feed.xml\" rel=\"self\" type=\"application/rss+xml\"/>\n",
        xml(base)
    ));

    for p in posts.iter().take(50) {
        let link = format!("{base}/{}", p.url);
        out.push_str("<item>\n");
        if let Some(t) = &p.title {
            out.push_str(&format!("<title>{}</title>\n", xml(t)));
        }
        out.push_str(&format!("<link>{}</link>\n", xml(&link)));
        out.push_str(&format!(
            "<guid isPermaLink=\"true\">{}</guid>\n",
            xml(&link)
        ));
        if let Some(d) = p.date.as_deref().and_then(rfc822) {
            out.push_str(&format!("<pubDate>{d}</pubDate>\n"));
        }
        if let Some(desc) = &p.description {
            out.push_str(&format!("<description>{}</description>\n", xml(desc)));
        }
        for c in &p.categories {
            out.push_str(&format!("<category>{}</category>\n", xml(c)));
        }
        out.push_str("</item>\n");
    }

    out.push_str("</channel>\n</rss>\n");
    Some(out)
}

/// Minimal XML text escaping for feed content.
fn xml(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            _ => o.push(c),
        }
    }
    o
}

/// Convert an ISO `YYYY-MM-DD` (leading 10 chars) to the RFC-822 date RSS
/// `pubDate` wants, at midnight GMT. `None` if it doesn't parse — the weekday is
/// computed with Zeller's congruence so no date dependency is needed.
fn rfc822(date: &str) -> Option<String> {
    let d = date.get(..10)?;
    let mut it = d.split('-');
    let y: i32 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let day: u32 = it.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    // Zeller: treat Jan/Feb as months 13/14 of the prior year.
    let (mz, yz) = if m < 3 { (m + 12, y - 1) } else { (m, y) };
    let (k, j) = (yz % 100, yz / 100);
    let h = (day as i32 + 13 * (mz as i32 + 1) / 5 + k + k / 4 + j / 4 + 5 * j).rem_euclid(7);
    // h: 0 = Saturday … 6 = Friday.
    let wd = ["Sat", "Sun", "Mon", "Tue", "Wed", "Thu", "Fri"][h as usize];
    let mon = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(m - 1) as usize];
    Some(format!("{wd}, {day:02} {mon} {y:04} 00:00:00 +0000"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc822_known_weekdays() {
        // 2024-01-15 was a Monday; 2020-02-29 a Saturday; 1999-12-31 a Friday.
        assert_eq!(
            rfc822("2024-01-15"),
            Some("Mon, 15 Jan 2024 00:00:00 +0000".into())
        );
        assert_eq!(
            rfc822("2020-02-29"),
            Some("Sat, 29 Feb 2020 00:00:00 +0000".into())
        );
        assert_eq!(
            rfc822("1999-12-31"),
            Some("Fri, 31 Dec 1999 00:00:00 +0000".into())
        );
        // Accepts a full ISO timestamp (uses the date part) and rejects junk.
        assert_eq!(
            rfc822("2024-01-15T09:30:00Z"),
            Some("Mon, 15 Jan 2024 00:00:00 +0000".into())
        );
        assert_eq!(rfc822("not-a-date"), None);
    }

    #[test]
    fn xml_escapes_specials() {
        assert_eq!(
            xml("a & b < c > \"d\" 'e'"),
            "a &amp; b &lt; c &gt; &quot;d&quot; &apos;e&apos;"
        );
    }
}
