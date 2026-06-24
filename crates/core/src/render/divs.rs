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
    let mut in_code: Option<(char, usize)> = None;
    for (i, line) in src.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let was_in_code = in_code.is_some();
        in_code = next_code_state(in_code, line);
        // A `:::` marker is a div fence only outside a code block; inside one it is
        // literal content (e.g. docs that *show* `::: {.callout-note}` in a code
        // block), so leave those lines untouched.
        let blank = !was_in_code && in_code.is_none() && parse_fence(line.trim_start()).is_some();
        if !blank {
            out.push_str(line);
        }
    }
    if src.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// A Markdown code-fence marker line (3+ backticks or tildes after at most 3 spaces
/// of indentation), as `(fence_char, run_len)`. Used to recognise `:::` lines that
/// sit *inside* a code block, which must render literally rather than as div fences.
fn code_fence(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None; // a code fence is indented at most 3 spaces (CommonMark)
    }
    let ch = trimmed.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let run = trimmed.chars().take_while(|&c| c == ch).count();
    (run >= 3).then_some((ch, run))
}

/// Advance the fenced-code state machine by one line: outside a code block a fence
/// opens one; inside, a bare same-char fence of at least the opening length closes
/// it. Keeps `preprocess` and `scan_div_spans` agreeing on what is "inside code".
fn next_code_state(state: Option<(char, usize)>, line: &str) -> Option<(char, usize)> {
    match state {
        Some((ch, run)) => match code_fence(line) {
            // A closing fence carries no info string (only the fence chars + space).
            Some((c2, r2))
                if c2 == ch
                    && r2 >= run
                    && line.trim_start().trim_start_matches(ch).trim().is_empty() =>
            {
                None
            }
            _ => Some((ch, run)),
        },
        None => code_fence(line),
    }
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
    let mut in_code: Option<(char, usize)> = None;
    for (i, line) in src.lines().enumerate() {
        let was_in_code = in_code.is_some();
        in_code = next_code_state(in_code, line);
        if was_in_code || in_code.is_some() {
            continue; // inside (or entering/closing) a code block: not a div fence
        }
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
    warnings: &mut Vec<Warning>,
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
                let container = build_container(done.span, done.inner, origins, counts, warnings);
                push_block(&mut stack, &mut result, container);
            } else {
                break;
            }
        }
    }
    // Close anything still open (e.g. unterminated div at EOF).
    while let Some(done) = stack.pop() {
        let container = build_container(done.span, done.inner, origins, counts, warnings);
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
    warnings: &mut Vec<Warning>,
) -> Block {
    let attrs = parse_attrs(&span.attrs);
    let id = make_id(&format!("div:{}", span.attrs), counts);
    let (file, open_line) = map_origin(origins, span.open);
    let (_, close_line) = map_origin(origins, span.close);
    let sourcepos = format!("{open_line}:1-{close_line}:3");
    let file_attr = source_file_attr(file.as_deref());
    let data = format!(" data-block-id=\"{id}\" data-sourcepos=\"{sourcepos}\"{file_attr}");
    let concat = |inner: &[Block]| -> String { inner.iter().map(|b| b.html.as_str()).collect() };

    let html = if let Some(kind) = attrs.callout_kind() {
        // Validate the kind against qmd-fast's callout vocabulary (an unknown kind
        // warns, click-to-source, and still renders with its given class).
        if let Some(w) = super::validate::validate_callout_kind(kind, open_line, file.clone()) {
            warnings.push(w);
        }
        // Callout: use a `title="..."` attr, else a leading heading, else the kind.
        let title = match attrs.get("title") {
            Some(t) => html_escape(t),
            None if inner.first().is_some_and(|b| is_heading(&b.html)) => {
                strip_tags(&inner.remove(0).html)
            }
            None => capitalize(kind),
        };
        let body = concat(&inner);
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
        let body = concat(&inner);
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
    } else if attrs.classes.iter().any(|c| c == "code-walkthrough") {
        // Narrated code walkthrough: the first code block becomes a sticky panel; the
        // remaining blocks (the `.step` divs) scroll alongside it and drive line-range
        // highlighting (walkthrough.js, reusing the `.qhl-ln` contract). Read-only:
        // inner blocks keep their own ids/sourcepos via the regular grouping.
        let code_idx = inner
            .iter()
            .position(|b| b.html.contains("<pre") && b.html.contains("<code"));
        if let Some(w) =
            super::validate::validate_walkthrough(code_idx.is_some(), open_line, file.clone())
        {
            warnings.push(w);
        }
        match code_idx {
            Some(i) => {
                // Line-wrap the panel so its lines are addressable by ordinal (the same
                // idempotent helper magic-move uses); the rest stay in document order.
                let panel = super::emit::wrap_pre_lines(&inner[i].html);
                let steps: String = inner
                    .iter()
                    .enumerate()
                    .filter_map(|(j, b)| (j != i).then_some(b.html.as_str()))
                    .collect();
                format!(
                    "<div class=\"code-walkthrough\"{data}><div class=\"cw-steps\">{steps}</div><div class=\"cw-stage\"><div class=\"cw-code\">{panel}</div></div></div>"
                )
            }
            None => {
                let body = concat(&inner);
                format!(
                    "<div class=\"code-walkthrough\"{data}><div class=\"cw-steps\">{body}</div></div>"
                )
            }
        }
    } else if attrs.classes.iter().any(|c| c == "step") {
        // A walkthrough step: carry its line-focus spec as `data-cw-lines` (read by
        // walkthrough.js) and keep the div's own id/sourcepos so its prose stays
        // locatable. Meaningful only inside `.code-walkthrough`; harmless elsewhere.
        let id_attr = id_attr(attrs.id.as_deref());
        let cw_lines = match attrs.get("lines") {
            Some(spec) if !spec.is_empty() => {
                format!(" data-cw-lines=\"{}\"", escape_attr(spec))
            }
            _ => String::new(),
        };
        let body = concat(&inner);
        format!("<div class=\"step\"{id_attr}{data}{cw_lines}>{body}</div>")
    } else if attrs.classes.iter().any(|c| c == "panel-tabset") {
        // Tabbed panels: child headings at the shallowest level present become tabs, and
        // the blocks after each become its panel body (deeper headings stay in the body).
        // Labels are emitted as ARIA tab buttons, NOT as <hN>, so they don't pollute the
        // TOC. tab/panel ids derive from the container's block id (unique + stable).
        // Read-only: inner blocks keep their own ids/sourcepos via direct concatenation.
        let min_level = inner
            .iter()
            .filter_map(|b| block_heading_level(&b.html))
            .min();
        if let Some(w) =
            super::validate::validate_tabset(min_level.is_some(), open_line, file.clone())
        {
            warnings.push(w);
        }
        match min_level {
            None => format!("<div class=\"panel-tabset\"{data}>{}</div>", concat(&inner)),
            Some(level) => {
                // Partition: blocks before the first tab heading are an intro; each
                // level-`level` heading opens a new (label, body) tab.
                let mut intro = String::new();
                let mut tabs: Vec<(String, String)> = Vec::new();
                for b in &inner {
                    if block_heading_level(&b.html) == Some(level) {
                        tabs.push((strip_tags(&b.html), String::new()));
                    } else if let Some((_, body)) = tabs.last_mut() {
                        body.push_str(&b.html);
                    } else {
                        intro.push_str(&b.html);
                    }
                }
                let mut tablist = String::from("<div class=\"tabset-tablist\" role=\"tablist\">");
                let mut panels = String::new();
                for (i, (label, body)) in tabs.iter().enumerate() {
                    let sel = i == 0;
                    let (tab_id, panel_id) = (format!("{id}-t{i}"), format!("{id}-p{i}"));
                    tablist.push_str(&format!(
                        "<button class=\"tabset-tab\" role=\"tab\" id=\"{tab_id}\" aria-controls=\"{panel_id}\" aria-selected=\"{sel}\" tabindex=\"{}\">{}</button>",
                        if sel { "0" } else { "-1" },
                        html_escape(label),
                    ));
                    panels.push_str(&format!(
                        "<div class=\"tabset-panel\" role=\"tabpanel\" id=\"{panel_id}\" aria-labelledby=\"{tab_id}\"{}>{body}</div>",
                        if sel { "" } else { " hidden" },
                    ));
                }
                tablist.push_str("</div>");
                format!("<div class=\"panel-tabset\"{data}>{intro}{tablist}{panels}</div>")
            }
        }
    } else {
        let mut class = attrs.classes.join(" ");
        if class.is_empty() {
            class.push_str("qmd-div");
        }
        let id_attr = id_attr(attrs.id.as_deref());
        let body = concat(&inner);
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
