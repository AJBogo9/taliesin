//! Pure document-outline extraction for `.tmd`: the ATX-heading tree that powers the LSP
//! `textDocument/documentSymbol` response (outline view, breadcrumbs, sticky scroll). A Rust
//! port of the companion's `outline.ts`, so it is editor-agnostic. Skips headings inside
//! fenced code blocks and the leading `---` front-matter block, and strips a trailing
//! `{#id}`/`{.class}` attribute block + inline emphasis markers from the title.

/// One heading in the nested outline.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OutlineNode {
    pub title: String,
    pub level: u8,
    /// 0-based line of the heading.
    pub start_line: usize,
    /// 0-based last line of the section body (inclusive): the line before the next heading
    /// at the same or a higher level; the last heading runs to EOF.
    pub end_line: usize,
    pub children: Vec<OutlineNode>,
}

struct Flat {
    title: String,
    level: u8,
    line: usize,
}

/// Heading text minus a trailing `{…}` attribute block and inline emphasis markers.
fn clean_title(raw: &str) -> String {
    let s = raw.trim();
    // Strip a trailing `{…}` (no nested `}`): mirrors `\s*\{[^}]*\}\s*$`.
    let no_attr = match s.rfind('{') {
        Some(open) if s.ends_with('}') && !s[open + 1..s.len() - 1].contains('}') => {
            s[..open].trim()
        }
        _ => s,
    };
    let no_emph: String = no_attr
        .chars()
        .filter(|c| !matches!(c, '*' | '_' | '`'))
        .collect();
    let no_emph = no_emph.trim();
    if no_emph.is_empty() {
        raw.trim().to_string()
    } else {
        no_emph.to_string()
    }
}

/// A leading `\s*(```+|~~~+)` fence marker char, or None.
fn fence_marker(line: &str) -> Option<char> {
    let t = line.trim_start();
    if t.starts_with("```") {
        Some('`')
    } else if t.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

/// `^(#{1,6})\s+(.*)$` → (level, title-slice).
fn atx_heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &line[hashes..];
    let title = rest.trim_start_matches([' ', '\t']);
    if title.len() == rest.len() {
        return None; // no whitespace after the `#`s
    }
    Some((hashes as u8, title))
}

/// The ATX headings in reading order, skipping fenced code and a leading `---` block.
fn headings(text: &str) -> Vec<Flat> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let mut in_fence = false;
    let mut fence = ' ';
    let mut start = 0;
    if lines.first().map(|l| l.trim()) == Some("---") {
        for (i, l) in lines.iter().enumerate().skip(1) {
            let t = l.trim();
            if t == "---" || t == "..." {
                start = i + 1;
                break;
            }
        }
    }
    for (i, line) in lines.iter().enumerate().skip(start) {
        if let Some(marker) = fence_marker(line) {
            if !in_fence {
                in_fence = true;
                fence = marker;
            } else if fence == marker {
                in_fence = false;
            }
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some((level, title)) = atx_heading(line) {
            out.push(Flat {
                title: clean_title(title),
                level,
                line: i,
            });
        }
    }
    out
}

/// Build the nested outline tree from the flat heading list.
pub(crate) fn outline(text: &str) -> Vec<OutlineNode> {
    let flat = headings(text);
    let line_count = text.split('\n').count().max(1);
    let n = flat.len();

    // Each heading's section end: the line before the next same-or-higher-level heading.
    let mut ends = vec![line_count - 1; n];
    for i in 0..n {
        for j in (i + 1)..n {
            if flat[j].level <= flat[i].level {
                ends[i] = flat[j].line.saturating_sub(1);
                break;
            }
        }
    }

    // Parent = nearest preceding heading of strictly smaller level (stack, mirrors outline.ts).
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut roots: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for i in 0..n {
        while stack
            .last()
            .is_some_and(|&top| flat[top].level >= flat[i].level)
        {
            stack.pop();
        }
        match stack.last() {
            Some(&parent) => children[parent].push(i),
            None => roots.push(i),
        }
        stack.push(i);
    }

    fn build(i: usize, flat: &[Flat], ends: &[usize], children: &[Vec<usize>]) -> OutlineNode {
        OutlineNode {
            title: flat[i].title.clone(),
            level: flat[i].level,
            start_line: flat[i].line,
            end_line: ends[i],
            children: children[i]
                .iter()
                .map(|&c| build(c, flat, ends, children))
                .collect(),
        }
    }
    roots
        .iter()
        .map(|&r| build(r, &flat, &ends, &children))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(
        title: &str,
        level: u8,
        start: usize,
        end: usize,
        children: Vec<OutlineNode>,
    ) -> OutlineNode {
        OutlineNode {
            title: title.to_string(),
            level,
            start_line: start,
            end_line: end,
            children,
        }
    }

    #[test]
    fn nests_headings_and_bounds_sections() {
        let text = "# A\n\ntext\n\n## B\n\nmore\n\n## C\n\n# D\n";
        // lines: 0 "# A", 4 "## B", 8 "## C", 10 "# D"; line_count = 12 (trailing \n).
        assert_eq!(
            outline(text),
            vec![
                node(
                    "A",
                    1,
                    0,
                    9,
                    vec![node("B", 2, 4, 7, vec![]), node("C", 2, 8, 9, vec![])]
                ),
                node("D", 1, 10, 11, vec![]),
            ]
        );
    }

    #[test]
    fn skips_frontmatter_and_fenced_code() {
        let text =
            "---\ntitle: x\n# not a heading\n---\n\n# Real\n\n```\n# fenced\n```\n\n## Sub\n";
        let out = outline(text);
        let titles: Vec<&str> = flatten(&out);
        assert_eq!(titles, vec!["Real", "Sub"]);
    }

    #[test]
    fn strips_attribute_block_and_emphasis_from_titles() {
        assert_eq!(clean_title("Intro {#sec-x}"), "Intro");
        assert_eq!(clean_title("*Bold* `code`"), "Bold code");
        assert_eq!(clean_title("Plain"), "Plain");
        // A `{...}` not at the end is kept.
        assert_eq!(clean_title("a {b} c"), "a {b} c");
    }

    #[test]
    fn a_seven_hash_line_is_not_a_heading() {
        assert!(outline("####### too deep\n").is_empty());
        assert!(outline("#no-space\n").is_empty());
    }

    fn flatten(nodes: &[OutlineNode]) -> Vec<&str> {
        let mut out = Vec::new();
        for n in nodes {
            out.push(n.title.as_str());
            out.extend(flatten(&n.children));
        }
        out
    }
}
