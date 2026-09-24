//! What each source line of a `.tmd` is, as comrak parses it: markdown, code, raw HTML or
//! front matter, how deep in block quotes and list items it sits, and where headings start.
//!
//! The ONE answer for every pass that reads source line by line instead of reading the
//! render: the include pass, the shortcode pass, the `:::` div scan, the prose count and the
//! site's anchor scan. Each of them used to carry its own ``` / `~~~` tracker, and every
//! tracker disagreed with comrak in its own way (audit 2026-09-24, B2): a line starting with
//! ```` ```pip install x``` ```` (inline code, a paragraph to comrak) opened a "fence" that
//! swallowed every later callout, an HTML comment was invisible (a commented-out
//! `{{< include >}}` was expanded and published its draft), and a code sample indented four
//! spaces in a list item was rewritten. Parsing with the renderer's own options is what makes
//! a disagreement impossible rather than unlikely.
//!
//! Parse-only and cheap (0.2 ms for a 40 KB post, release, 2026-09-24), and it takes any
//! text: a half-typed buffer parses like any other, an unclosed fence simply runs on. The
//! walk is iterative, so a deeply nested document cannot overflow the caller's stack here.

use comrak::arena_tree::NodeEdge;
use comrak::nodes::{NodeValue, Sourcepos};
use comrak::{Arena, parse_document};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex};

/// What one source line is to comrak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Read as markdown: prose, headings, list and quote markers, table rows, blank lines,
    /// and HTML blocks of types 6 and 7 (a `<div>` line), which end at the first blank line.
    Markdown,
    /// The leading front-matter block, both delimiters included, exactly as
    /// [`crate::frontmatter::front_matter_block`] delimits it.
    FrontMatter,
    /// A fenced code block's opening fence.
    FenceOpen,
    /// A line inside a fenced code block.
    FenceBody,
    /// A fenced code block's closing fence.
    FenceClose,
    /// A line of an indented code block.
    IndentedCode,
    /// Raw HTML whose content CommonMark never reads as markdown: an HTML block of types 1
    /// to 5 (`<pre>`, `<script>`, `<style>`, `<textarea>`, a comment, a processing
    /// instruction, a declaration, CDATA), and the inner lines of an inline comment or tag
    /// that spans lines.
    RawHtml,
}

impl Kind {
    /// Whether markdown is read on this line (it is not code, raw HTML or front matter).
    pub fn is_markdown(self) -> bool {
        self == Kind::Markdown
    }
}

/// One source line, classified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub kind: Kind,
    /// How many block quotes, list items and footnote definitions contain the line
    /// (0 = top level).
    pub depth: u32,
    /// The level of the heading that starts on this line, ATX or setext.
    pub heading: Option<u8>,
}

/// A line past the end of the text: top-level markdown, like a blank line.
const PAST_END: Line = Line {
    kind: Kind::Markdown,
    depth: 0,
    heading: None,
};

/// One fenced code block. Lines are 0-based.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fence {
    /// The opening fence.
    pub open: usize,
    /// The last line the block covers: its closing fence when `closed`.
    pub end: usize,
    /// Whether a closing fence ended it. `false` when its container or the end of the text
    /// did, which in an included file means it runs on into whatever follows the include.
    pub closed: bool,
    /// The info string (`python`, `{python}`, …), as comrak read it.
    pub info: String,
}

/// A position in the text: 0-based line, byte column within that line.
pub type Pos = (usize, usize);

/// A text, classified line by line. See [`classify`].
#[derive(Clone, Debug, Default)]
pub struct Lines {
    lines: Vec<Line>,
    /// Every fenced code block, in document order.
    pub fences: Vec<Fence>,
    /// Every inline code span, `[start, end)`, in document order. A span may cross lines.
    pub code_spans: Vec<(Pos, Pos)>,
}

impl Lines {
    /// The class of 0-based line `i`. A line past the end of the text is top-level
    /// markdown, so a caller iterating a slightly different line split never panics.
    pub fn line(&self, i: usize) -> Line {
        self.lines.get(i).copied().unwrap_or(PAST_END)
    }

    /// Whether byte column `col` of 0-based line `line` is inside an inline code span.
    pub fn in_code_span(&self, line: usize, col: usize) -> bool {
        self.code_spans
            .iter()
            .any(|&(start, end)| start <= (line, col) && (line, col) < end)
    }
}

/// Classify every line of `src` by parsing it with the renderer's own comrak options.
///
/// Line numbering is comrak's (a line ends at `\n`, `\r\n` or a lone `\r`), which is also
/// `str::lines` on the render path, where a lone `\r` was normalized away at ingest. The front
/// matter is found by [`crate::frontmatter::front_matter_block`], the one splitter, and
/// blanked before the parse, so its YAML is never read as markdown.
///
/// Memoized on the text (see `Memo`).
pub fn classify(src: &str) -> Lines {
    let hit = MEMO
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .map
        .get(src)
        .cloned();
    if let Some(lines) = hit {
        return lines;
    }
    let lines = classify_uncached(src);
    MEMO.lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert_bounded(src, lines.clone(), MEMO_BUDGET_BYTES);
    lines
}

/// A `text -> classification` memo, bounded by the bytes of text it holds, evicted
/// oldest-first: the shape of `highlight`'s memo, for the same reason.
///
/// The same text is classified many times over. A site build renders every page in two
/// whole-project passes before the page's own build, the anchor scan reads the same
/// include-resolved text the render classifies, and on a save every page but the edited
/// one is unchanged. Measured on `docs/guide` (2026-09-24, release): 224 parses for one
/// `--check-only` of 16 pages, and without this memo `refresh_xrefs`, which runs on every
/// save, took 12.7 ms where it took 3.1 ms before this module existed (3.7 ms with it).
#[derive(Default)]
struct Memo {
    map: HashMap<Arc<str>, Lines>,
    order: VecDeque<Arc<str>>,
    bytes: usize,
}

impl Memo {
    /// Insert `text -> lines`, evicting oldest-first until `budget` bytes of text fit. A
    /// no-op for a text already held and for one larger than the whole budget.
    fn insert_bounded(&mut self, text: &str, lines: Lines, budget: usize) {
        if self.map.contains_key(text) || text.len() > budget {
            return;
        }
        while self.bytes + text.len() > budget {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            self.map.remove(&old);
            self.bytes -= old.len();
        }
        let text: Arc<str> = text.into();
        self.bytes += text.len();
        self.order.push_back(Arc::clone(&text));
        self.map.insert(text, lines);
    }
}

static MEMO: LazyLock<Mutex<Memo>> = LazyLock::new(|| Mutex::new(Memo::default()));

/// 16 MiB of source text: every page of the largest project here, several times over.
const MEMO_BUDGET_BYTES: usize = 16 * 1024 * 1024;

fn classify_uncached(src: &str) -> Lines {
    let starts = line_starts(src);
    let n = starts.len();
    let fm_lines = crate::frontmatter::front_matter_block(src)
        .map_or(0, |yaml| yaml.matches('\n').count() + 2);
    let parsed = blank_front_matter(src, &starts, fm_lines);
    let arena = Arena::new();
    let root = parse_document(&arena, &parsed[..], &crate::render::parse_options());

    let mut kinds = vec![Kind::Markdown; n];
    for kind in kinds.iter_mut().take(fm_lines) {
        *kind = Kind::FrontMatter;
    }
    // Container depth as a difference array: +1 where a container starts, -1 past its end.
    let mut depth_delta = vec![0i64; n + 1];
    let mut headings = vec![None; n];
    let mut out = Lines::default();
    // A node's last line, as the walk corrects it (see `leaf_end`), for each open ancestor:
    // a container ends no earlier than its last child.
    let mut ends: Vec<usize> = Vec::new();
    // `traverse` is an iterative walk: no recursion, whatever the nesting.
    for edge in root.traverse() {
        let node = match edge {
            NodeEdge::Start(node) => {
                ends.push(line_span(node.data.borrow().sourcepos).1);
                continue;
            }
            NodeEdge::End(node) => node,
        };
        let data = node.data.borrow();
        let (s, e) = line_span(data.sourcepos);
        let e = leaf_end(&data.value, s)
            .unwrap_or(e)
            .max(ends.pop().unwrap_or(e));
        if let Some(parent) = ends.last_mut() {
            *parent = (*parent).max(e);
        }
        match &data.value {
            NodeValue::BlockQuote | NodeValue::Item(_) | NodeValue::FootnoteDefinition(_) => {
                depth_delta[s.min(n)] += 1;
                depth_delta[(e + 1).min(n)] -= 1;
            }
            NodeValue::CodeBlock(cb) if cb.fenced => {
                mark(&mut kinds, s, e, Kind::FenceBody);
                mark(&mut kinds, s, s, Kind::FenceOpen);
                if cb.closed && e > s {
                    mark(&mut kinds, e, e, Kind::FenceClose);
                }
                out.fences.push(Fence {
                    open: s,
                    end: e,
                    closed: cb.closed,
                    info: cb.info.clone(),
                });
            }
            NodeValue::CodeBlock(_) => mark(&mut kinds, s, e, Kind::IndentedCode),
            NodeValue::HtmlBlock(h) if (1..=5).contains(&h.block_type) => {
                mark(&mut kinds, s, e, Kind::RawHtml);
            }
            // Only the lines strictly inside: the first and last also hold markdown.
            NodeValue::HtmlInline(_) if e > s + 1 => mark(&mut kinds, s + 1, e - 1, Kind::RawHtml),
            NodeValue::Heading(h) if s < n => headings[s] = Some(h.level),
            NodeValue::Code(_) => {
                let sp = data.sourcepos;
                out.code_spans.push((
                    (
                        sp.start.line.saturating_sub(1),
                        sp.start.column.saturating_sub(1),
                    ),
                    (sp.end.line.saturating_sub(1), sp.end.column),
                ));
            }
            _ => {}
        }
    }
    // The walk met each node at its END, children first, so restore document order.
    out.fences.sort_by_key(|f| f.open);
    out.code_spans.sort();
    let mut depth = 0i64;
    out.lines = kinds
        .into_iter()
        .zip(headings)
        .enumerate()
        .map(|(i, (kind, heading))| {
            depth += depth_delta[i];
            Line {
                kind,
                depth: depth.max(0) as u32,
                heading,
            }
        })
        .collect();
    out
}

/// The lines of `src` as comrak counts them (a line ends at `\n`, `\r\n` or a lone `\r`),
/// without their terminators: the numbering [`Lines::line`] is indexed by. On text whose
/// lone `\r`s were normalized away (the render path) this is `str::lines`.
pub fn split(src: &str) -> impl Iterator<Item = &str> {
    let starts = line_starts(src);
    (0..starts.len()).map(move |i| {
        let end = starts.get(i + 1).copied().unwrap_or(src.len());
        src[starts[i]..end].trim_end_matches(['\n', '\r'])
    })
}

/// `src` with its first `fm_lines` lines (the front matter, both fences) blanked line for
/// line, so comrak parses the body alone while every line keeps its number.
///
/// MERGE NOTE (audit 2026-09-24, WP7): `frontmatter::blank_front_matter` is the one
/// blanker the render uses once the front-matter work lands; this local copy goes then, and
/// the call above becomes `crate::frontmatter::blank_front_matter(src)`.
fn blank_front_matter<'a>(
    src: &'a str,
    starts: &[usize],
    fm_lines: usize,
) -> std::borrow::Cow<'a, str> {
    if fm_lines == 0 {
        return std::borrow::Cow::Borrowed(src);
    }
    let body = starts.get(fm_lines).map_or("", |&s| &src[s..]);
    std::borrow::Cow::Owned("\n".repeat(fm_lines) + body)
}

/// A node's first and last line, 0-based. comrak ends a block that a later line closed at
/// column 0 of the line after its content; that line is not part of it.
fn line_span(sp: Sourcepos) -> (usize, usize) {
    let s = sp.start.line.saturating_sub(1);
    let mut e = sp.end.line.saturating_sub(1);
    if sp.end.column == 0 && e > s {
        e -= 1;
    }
    (s, e.max(s))
}

/// The last line of a code or HTML block, counted from its content rather than read from
/// its sourcepos: comrak 0.52 ends a block that its CONTAINER closed (an unclosed fence, or
/// indented code, in a list item followed by a blank line and an unindented line) at its
/// first line, while the block really runs over every line of its literal. `None` for any
/// other node, whose sourcepos is right.
fn leaf_end(value: &NodeValue, s: usize) -> Option<usize> {
    match value {
        NodeValue::CodeBlock(cb) if cb.fenced => {
            Some(s + cb.literal.lines().count() + usize::from(cb.closed))
        }
        NodeValue::CodeBlock(cb) => Some(s + cb.literal.lines().count().saturating_sub(1)),
        NodeValue::HtmlBlock(h) => Some(s + h.literal.lines().count().saturating_sub(1)),
        _ => None,
    }
}

/// Set lines `from..=to` (0-based, clamped to the text) to `kind`.
fn mark(kinds: &mut [Kind], from: usize, to: usize, kind: Kind) {
    let to = to.min(kinds.len().saturating_sub(1));
    for k in kinds.iter_mut().take(to + 1).skip(from) {
        *k = kind;
    }
}

/// The byte offset where each line starts, splitting as comrak does (`\n`, `\r\n`, lone
/// `\r`). A final line with no terminator counts; the empty "line" after a trailing
/// terminator does not, matching `str::lines`.
fn line_starts(src: &str) -> Vec<usize> {
    let b = src.as_bytes();
    let mut starts = Vec::new();
    let mut i = 0;
    while i < b.len() {
        starts.push(i);
        while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
            i += 1;
        }
        if i < b.len() {
            i += if b[i] == b'\r' && b.get(i + 1) == Some(&b'\n') {
                2
            } else {
                1
            };
        }
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kinds of every line, as one compact string per line for readable assertions.
    fn kinds(src: &str) -> Vec<Kind> {
        let c = classify(src);
        (0..src.lines().count()).map(|i| c.line(i).kind).collect()
    }

    use Kind::*;

    #[test]
    fn fences_are_what_comrak_calls_fences() {
        assert_eq!(
            kinds("a\n```py\nx\n```\nb\n"),
            [Markdown, FenceOpen, FenceBody, FenceClose, Markdown]
        );
        // A backtick fence's info string may not contain a backtick, so this is a
        // paragraph with inline code, and the lines after it are not code.
        assert_eq!(
            kinds("```pip install x``` installs it.\n\n::: {.note}\n"),
            [Markdown, Markdown, Markdown]
        );
        // A longer outer fence is not closed by a shorter inner one.
        assert_eq!(
            kinds("````\n```\nx\n```\n````\n"),
            [FenceOpen, FenceBody, FenceBody, FenceBody, FenceClose]
        );
        // `~~~` is not closed by backticks, and a `` ``` info `` line is not a close.
        assert_eq!(
            kinds("~~~\n```\n~~~\n```\n``` python\n```\n"),
            [
                FenceOpen, FenceBody, FenceClose, FenceOpen, FenceBody, FenceClose
            ]
        );
    }

    #[test]
    fn a_fence_ended_by_its_container_ends_there() {
        // The unclosed fence belongs to the list item; the paragraph after the list is prose.
        let c = classify("- a\n\n  ```\n  x\n\ny\n");
        assert_eq!(c.line(3).kind, FenceBody);
        assert_eq!(c.line(5).kind, Markdown);
        assert!(!c.fences[0].closed);
        // comrak's own sourcepos ends this fence (and its item) on the opening line; the
        // block really runs over every line up to the one that ended the item.
        let c = classify("- a\n\n  ```\n  x\n\n  y\n\nz\n");
        assert_eq!(
            (2..8)
                .map(|i| (c.line(i).kind, c.line(i).depth))
                .collect::<Vec<_>>(),
            [
                (FenceOpen, 1),
                (FenceBody, 1),
                (FenceBody, 1),
                (FenceBody, 1),
                (FenceBody, 1), // the blank line: comrak keeps it in the literal
                (Markdown, 0)
            ]
        );
        // The same for indented code in a list item.
        let c = classify("- a\n\n      code\n\n      more\n\nz\n");
        assert_eq!(c.line(4).kind, IndentedCode);
        assert_eq!(c.line(4).depth, 1);
    }

    #[test]
    fn indented_code_raw_html_and_front_matter_are_not_markdown() {
        assert_eq!(
            kinds("    x\n\ty\n\nz\n")[..2],
            [IndentedCode, IndentedCode]
        );
        assert_eq!(
            kinds("<!--\n{{< include x >}}\n-->\n\nprose\n"),
            [RawHtml, RawHtml, RawHtml, Markdown, Markdown]
        );
        assert_eq!(
            kinds("<pre>\n```\n</pre>\n\n```\ncode\n```\n"),
            [
                RawHtml, RawHtml, RawHtml, Markdown, FenceOpen, FenceBody, FenceClose
            ]
        );
        // A `<div>` is an HTML block of type 6: markdown resumes after the blank line.
        assert_eq!(
            kinds("<div>\n\n```\nx\n```\n")[2..],
            [FenceOpen, FenceBody, FenceClose]
        );
        // An inline comment that spans lines: its inner lines are raw.
        assert_eq!(
            kinds("text <!--\n{{< include x >}}\n--> end\n"),
            [Markdown, RawHtml, Markdown]
        );
        // The front matter, `...` closer included, and a fence inside its YAML is inert.
        assert_eq!(
            kinds("---\ndescription: |\n  ```\n...\n\n::: {.note}\n"),
            [
                FrontMatter,
                FrontMatter,
                FrontMatter,
                FrontMatter,
                Markdown,
                Markdown
            ]
        );
    }

    #[test]
    fn depth_counts_quotes_items_and_footnotes() {
        let c = classify("- a\n\n  b\n\n> q\n> > r\n\n[^1]: n\n\n    m\n\nz[^1]\n");
        let depths: Vec<u32> = (0..12).map(|i| c.line(i).depth).collect();
        assert_eq!(depths, [1, 1, 1, 0, 1, 2, 0, 1, 1, 1, 0, 0]);
    }

    #[test]
    fn headings_include_setext_and_indented_atx() {
        let c = classify("Title\n=====\n\n  ## Two {#sec-x}\n\nSub\n---\n\n    # not\n");
        let h: Vec<Option<u8>> = (0..9).map(|i| c.line(i).heading).collect();
        assert_eq!(
            h,
            [
                Some(1),
                None,
                None,
                Some(2),
                None,
                Some(2),
                None,
                None,
                None
            ]
        );
    }

    #[test]
    fn code_spans_carry_byte_positions_across_lines() {
        let c = classify("éé `a` b\n\nUse `x\n{{< input >}}` here.\n");
        // `éé ` is five bytes, so the span starts at byte 5.
        assert!(c.in_code_span(0, 5) && c.in_code_span(0, 7) && !c.in_code_span(0, 8));
        // The second line of a two-line span is inside it up to its closing backtick.
        assert!(c.in_code_span(3, 0) && c.in_code_span(3, 13) && !c.in_code_span(3, 14));
    }

    #[test]
    fn line_numbering_matches_comrak_for_every_line_ending() {
        for src in ["a\n```\nb\n", "a\r\n```\r\nb\r\n", "a\r```\rb\r"] {
            let c = classify(src);
            assert_eq!(
                (0..3).map(|i| c.line(i).kind).collect::<Vec<_>>(),
                [Markdown, FenceOpen, FenceBody],
                "{src:?}"
            );
        }
        // Past the end is top-level markdown, never a panic.
        assert_eq!(classify("").line(7), PAST_END);
    }

    /// The memo is transparent and bounded: a repeat is served from it, the oldest text is
    /// evicted first, and nothing is counted twice.
    #[test]
    fn classify_is_memoized_within_a_byte_budget() {
        let src = "memo test, unique to this test\n\n```\nx\n```\n";
        let first = classify(src);
        let held = MEMO.lock().unwrap().map.get(src).cloned();
        assert_eq!(held.map(|l| l.lines), Some(first.lines.clone()));
        assert_eq!(classify(src).lines, first.lines);

        let mut m = Memo::default();
        for t in ["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc"] {
            m.insert_bounded(t, Lines::default(), 30);
        }
        m.insert_bounded("cccccccccc", Lines::default(), 30);
        assert_eq!(
            (m.map.len(), m.bytes),
            (3, 30),
            "a repeat is not counted twice"
        );
        m.insert_bounded("dddddddddd", Lines::default(), 30);
        assert!(!m.map.contains_key("aaaaaaaaaa") && m.map.contains_key("dddddddddd"));
        m.insert_bounded(&"x".repeat(31), Lines::default(), 30);
        assert_eq!(m.map.len(), 3, "a text over the budget is not held");
    }

    #[test]
    fn deep_nesting_does_not_overflow_a_small_stack() {
        // The render guards its own recursion (`MAX_NESTING_DEPTH`); this walk must need no
        // guard at all, since the include pass and the site scan call it on their own
        // threads. 200k levels on a 2 MB thread.
        let src = format!("{}x\n", "> ".repeat(200_000));
        let depth = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || classify(&src).line(0).depth)
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(depth, 200_000);
    }
}
