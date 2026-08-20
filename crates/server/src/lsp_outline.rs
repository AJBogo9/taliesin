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

/// A leading `\s*(```+|~~~+)` fence marker char, or None. Shared with `lsp_fold`, which
/// must skip fenced code for the same reason this does: a `# comment` inside a `{python}`
/// cell is not a heading.
pub(crate) fn fence_marker(line: &str) -> Option<char> {
    let t = line.trim_start();
    if t.starts_with("```") {
        Some('`')
    } else if t.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

/// `^(#{1,6})\s+(.*)$` → (level, title-slice). Shared with `lsp_fold`.
pub(crate) fn atx_heading(line: &str) -> Option<(u8, &str)> {
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
    let lines: Vec<&str> = crate::lsp_pos::lines(text).collect();
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

/// One heading with the inclusive line extent of its section, in reading order.
///
/// The flat view of the same segmentation [`outline`] nests, and the input [`outline`]
/// builds from. `lsp_edits` re-levelled sections against it too, so that a "move section
/// down" could not segment the document differently from the outline the author was looking
/// at; that provider was cut on 2026-08-08 and [`outline`] is the only reader left.
pub(crate) struct Section {
    pub title: String,
    pub level: u8,
    /// 0-based line of the heading.
    pub start_line: usize,
    /// 0-based last line of the section body (inclusive). Sections **tile**: the next
    /// section starts on the line after this one, so a run of them is contiguous text.
    pub end_line: usize,
}

/// Every heading with its section's extent, in reading order.
pub(crate) fn sections(text: &str) -> Vec<Section> {
    let flat = headings(text);
    let line_count = crate::lsp_pos::lines(text).count().max(1);
    let mut out: Vec<Section> = flat
        .into_iter()
        .map(|f| Section {
            title: f.title,
            level: f.level,
            start_line: f.line,
            // The last heading of its branch runs to EOF until proven otherwise below.
            end_line: line_count - 1,
        })
        .collect();
    // Each heading's section end: the line before the next same-or-higher-level heading.
    for i in 0..out.len() {
        for j in (i + 1)..out.len() {
            if out[j].level <= out[i].level {
                out[i].end_line = out[j].start_line.saturating_sub(1);
                break;
            }
        }
    }
    out
}

/// Build the nested outline tree from the flat heading list.
pub(crate) fn outline(text: &str) -> Vec<OutlineNode> {
    let flat = sections(text);
    let n = flat.len();

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

    fn build(i: usize, flat: &[Section], children: &[Vec<usize>]) -> OutlineNode {
        OutlineNode {
            title: flat[i].title.clone(),
            level: flat[i].level,
            start_line: flat[i].start_line,
            end_line: flat[i].end_line,
            children: children[i]
                .iter()
                .map(|&c| build(c, flat, children))
                .collect(),
        }
    }
    roots.iter().map(|&r| build(r, &flat, &children)).collect()
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
        // An UNCLOSED brace is not an attribute block. The outline runs on the buffer while
        // it is being typed, so `# Intro {#sec-` is a heading mid-keystroke, and stripping
        // there makes the label jump about as the author types the id.
        assert_eq!(clean_title("Intro {#sec-x"), "Intro {#sec-x");
        assert_eq!(clean_title("Intro {"), "Intro {");
        // A `}` before the block belongs to the title, and the block after it still goes.
        // Written with no space between them on purpose: the candidate span starts at the
        // character AFTER the `{`, and a span that starts one character earlier swallows
        // this `}` and concludes the block is malformed.
        assert_eq!(clean_title("Intro }{#sec-x}"), "Intro }");
        // Two blocks are not one block: the candidate span contains a `}`, so nothing is
        // stripped rather than the wrong amount.
        assert_eq!(clean_title("Intro {.a} {#b} c}"), "Intro {.a} {#b} c}");
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
