//! `:::` fenced-div preprocessing and container building: scan div spans,
//! strip the fence markers (line-preserving) so inner content parses as normal
//! blocks, then regroup blocks into callouts/columns/etc. Also the Pandoc
//! attribute parsers. Split out of the render module; `use super::*` reaches
//! the block model + helpers (Block, FlatBlock, DivAttrs, make_id, escaping).

use super::*;

/// Parse a `.class #id` attribute block. Returns `None` unless every token is a
/// `.class` or `#id` (so non-attribute braces are left untouched).
pub(crate) fn parse_pandoc_attrs(s: &str) -> Option<(Vec<String>, Option<String>)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut classes = Vec::new();
    let mut id = None;
    for tok in s.split_whitespace() {
        if let Some(c) = tok.strip_prefix('.').filter(|c| !c.is_empty()) {
            classes.push(c.to_string());
        } else if let Some(i) = tok.strip_prefix('#').filter(|i| !i.is_empty()) {
            id = Some(i.to_string());
        } else {
            return None;
        }
    }
    Some((classes, id))
}

/// Blank out Quarto fenced-div markers (`::: {...}` / `:::`) without changing
/// the line count, so the inner content parses as ordinary blocks and every
/// other block's sourcepos line numbers stay valid against the original source.
pub(crate) fn preprocess(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for (i, line) in src.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if parse_fence(line.trim_start()).is_none() {
            out.push_str(line);
        }
    }
    if src.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// A pandoc/Quarto fenced-div marker: 3+ colons, then nothing (close) or an
/// attribute block / bare class name (open).
enum Fence {
    /// Opening fence; carries the raw attribute string (without the braces).
    Open(String),
    /// Closing fence (bare colons).
    Close,
}

fn parse_fence(s: &str) -> Option<Fence> {
    let colons = s.chars().take_while(|&c| c == ':').count();
    if colons < 3 {
        return None;
    }
    let rest = s[colons..].trim();
    if rest.is_empty() {
        Some(Fence::Close)
    } else if let Some(inner) = rest.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        Some(Fence::Open(inner.trim().to_string()))
    } else if rest.chars().next().is_some_and(char::is_alphabetic) {
        // bare `::: classname` -> treat the first word as a class
        Some(Fence::Open(format!(
            ".{}",
            rest.split_whitespace().next().unwrap_or("")
        )))
    } else {
        None
    }
}

/// A fenced-div span in buffer-line space (1-based, inclusive of the markers).
pub(crate) struct DivSpan {
    open: usize,
    close: usize,
    /// Raw attribute string from the opening fence (e.g. `.callout-note title="X"`).
    attrs: String,
}

/// Find all fenced-div spans (stack-based, so nesting is handled). Sorted so
/// that for a shared opening line the outermost (latest close) comes first.
pub(crate) fn scan_div_spans(src: &str) -> Vec<DivSpan> {
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut spans: Vec<DivSpan> = Vec::new();
    for (i, line) in src.lines().enumerate() {
        match parse_fence(line.trim_start()) {
            Some(Fence::Open(attrs)) => stack.push((i + 1, attrs)),
            Some(Fence::Close) => {
                if let Some((open, attrs)) = stack.pop() {
                    spans.push(DivSpan {
                        open,
                        close: i + 1,
                        attrs,
                    });
                }
            }
            None => {}
        }
    }
    spans.sort_by_key(|s| (s.open, std::cmp::Reverse(s.close)));
    spans
}

/// Parse a fenced-div attribute string: `.class`, `#id`, and `key=val`
/// (value optionally quoted), whitespace-separated.
pub(crate) fn parse_attrs(s: &str) -> DivAttrs {
    let mut attrs = DivAttrs::default();
    for tok in tokenize_attrs(s) {
        if let Some(c) = tok.strip_prefix('.') {
            attrs.classes.push(c.to_string());
        } else if let Some(i) = tok.strip_prefix('#') {
            attrs.id = Some(i.to_string());
        } else if let Some((k, v)) = tok.split_once('=') {
            attrs
                .kv
                .push((k.to_string(), v.trim_matches(['"', '\'']).to_string()));
        } else if !tok.is_empty() {
            attrs.classes.push(tok.to_string());
        }
    }
    attrs
}

/// Split on whitespace, but keep quoted values (e.g. `title="a b"`) together.
fn tokenize_attrs(s: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in s.chars() {
        match quote {
            Some(q) => {
                cur.push(ch);
                if ch == q {
                    quote = None;
                }
            }
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                cur.push(ch);
            }
            None if ch.is_whitespace() => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}

/// Group flat top-level blocks back into fenced-div container blocks (callouts,
/// layout grids, generic divs), honoring nesting. Blocks inside a div become a
/// single container block whose HTML embeds them (they keep their own ids and
/// sourcepos, so click-to-source still works inside).
pub(crate) fn group_divs(
    flat: Vec<FlatBlock>,
    spans: &[DivSpan],
    origins: Option<&[LineOrigin]>,
    counts: &mut HashMap<String, u32>,
) -> Vec<Block> {
    struct Open<'a> {
        span: &'a DivSpan,
        inner: Vec<Block>,
    }
    let mut result: Vec<Block> = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut span_idx = 0;

    let push_block =
        |stack: &mut Vec<Open>, result: &mut Vec<Block>, b: Block| match stack.last_mut() {
            Some(top) => top.inner.push(b),
            None => result.push(b),
        };

    for (i, fb) in flat.iter().enumerate() {
        // Open every span that starts before this block and contains it.
        while span_idx < spans.len()
            && spans[span_idx].open < fb.buf_start
            && spans[span_idx].close > fb.buf_start
        {
            stack.push(Open {
                span: &spans[span_idx],
                inner: Vec::new(),
            });
            span_idx += 1;
        }
        // Skip any spans that contain no blocks (degenerate/empty divs).
        while span_idx < spans.len() && spans[span_idx].close < fb.buf_start {
            span_idx += 1;
        }

        push_block(&mut stack, &mut result, fb.block.clone());

        // Close spans that end before the next block begins (innermost first).
        let next_start = flat.get(i + 1).map(|n| n.buf_start).unwrap_or(usize::MAX);
        while let Some(top) = stack.last() {
            if top.span.close < next_start {
                let done = stack.pop().unwrap();
                let container = build_container(done.span, done.inner, origins, counts);
                push_block(&mut stack, &mut result, container);
            } else {
                break;
            }
        }
    }
    // Close anything still open (e.g. unterminated div at EOF).
    while let Some(done) = stack.pop() {
        let container = build_container(done.span, done.inner, origins, counts);
        push_block(&mut stack, &mut result, container);
    }
    result
}

/// Render one fenced div as a container block: callouts, layout grids, or a
/// generic class div.
fn build_container(
    span: &DivSpan,
    mut inner: Vec<Block>,
    origins: Option<&[LineOrigin]>,
    counts: &mut HashMap<String, u32>,
) -> Block {
    let attrs = parse_attrs(&span.attrs);
    let id = make_id(&format!("div:{}", span.attrs), counts);
    let (file, open_line) = map_origin(origins, span.open);
    let (_, close_line) = map_origin(origins, span.close);
    let sourcepos = format!("{open_line}:1-{close_line}:3");
    let file_attr = match &file {
        Some(f) => format!(" data-source-file=\"{}\"", escape_attr(f)),
        None => String::new(),
    };
    let data = format!(" data-block-id=\"{id}\" data-sourcepos=\"{sourcepos}\"{file_attr}");

    let html = if let Some(kind) = attrs.callout_kind() {
        // Callout: use a `title="..."` attr, else a leading heading, else the kind.
        let title = match attrs.get("title") {
            Some(t) => html_escape(t),
            None if inner.first().is_some_and(|b| is_heading(&b.html)) => {
                strip_tags(&inner.remove(0).html)
            }
            None => capitalize(kind),
        };
        let body: String = inner.iter().map(|b| b.html.as_str()).collect();
        // `collapse="true"` makes the callout a native <details> (starts closed);
        // `collapse="false"` is collapsible but starts open.
        match attrs.get("collapse") {
            Some(v) => {
                let open = if v == "false" { " open" } else { "" };
                format!(
                    "<div class=\"callout callout-{kind} callout-collapse\"{data}><details{open}><summary class=\"callout-title\">{title}</summary><div class=\"callout-body\">{body}</div></details></div>"
                )
            }
            None => format!(
                "<div class=\"callout callout-{kind}\"{data}><div class=\"callout-title\">{title}</div><div class=\"callout-body\">{body}</div></div>"
            ),
        }
    } else if let Some(ncol) = attrs.get("layout-ncol").and_then(|n| n.parse::<u32>().ok()) {
        let body: String = inner.iter().map(|b| b.html.as_str()).collect();
        format!(
            "<div class=\"qmd-layout\" style=\"display:grid;grid-template-columns:repeat({ncol},minmax(0,1fr));gap:1rem\"{data}>{body}</div>"
        )
    } else if attrs.classes.iter().any(|c| c == "magic-move") {
        // Magic-move: the contained code blocks are animation steps. Line-wrap each so
        // the deck engine can match + glide lines between consecutive blocks.
        let body: String = inner
            .iter()
            .map(|b| super::emit::wrap_pre_lines(&b.html))
            .collect();
        format!("<div class=\"magic-move\"{data}>{body}</div>")
    } else {
        let mut class = attrs.classes.join(" ");
        if class.is_empty() {
            class.push_str("qmd-div");
        }
        let id_attr = match &attrs.id {
            Some(i) => format!(" id=\"{}\"", escape_attr(i)),
            None => String::new(),
        };
        let body: String = inner.iter().map(|b| b.html.as_str()).collect();
        format!("<div class=\"{class}\"{id_attr}{data}>{body}</div>")
    };

    Block {
        id,
        sourcepos,
        source_file: file,
        html,
        cell: None,
    }
}
