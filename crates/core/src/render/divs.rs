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

/// Blank out fenced-div markers (`::: {...}` / `:::`) without changing
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

/// A Pandoc fenced-div marker: 3+ colons, then nothing (close) or an
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
/// Also returns the 1-based line of any `:::` open that was never closed — the
/// orchestrator warns on those (an unterminated fence otherwise drops its wrapper
/// silently and the content renders unfenced).
pub(crate) fn scan_div_spans(src: &str) -> (Vec<DivSpan>, Vec<usize>) {
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
    let mut unclosed: Vec<usize> = stack.into_iter().map(|(open, _)| open).collect();
    unclosed.sort_unstable();
    (spans, unclosed)
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
            attrs.kv.push((k.to_string(), unquote_value(v)));
        } else if !tok.is_empty() {
            attrs.classes.push(tok.to_string());
        }
    }
    attrs
}

/// Split on whitespace, but keep quoted values (e.g. `title="a b"`) together.
/// Inside a quote, a backslash escapes the next character, so `title="a \"b\""`
/// stays one token instead of ending at the first inner quote.
fn tokenize_attrs(s: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for ch in s.chars() {
        if escaped {
            // Already inside a quote (escape state is only set there); keep the
            // escaped char verbatim (unescaping happens in `unquote_value`).
            cur.push(ch);
            escaped = false;
            continue;
        }
        match quote {
            Some(q) => {
                cur.push(ch);
                if ch == '\\' {
                    escaped = true;
                } else if ch == q {
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

/// Strip one matching outer quote pair from an attribute value and unescape the
/// quote escapes `\"`/`\'` that the tokenizer preserved. Only quote escapes are
/// consumed: any other backslash (a LaTeX macro in a `fig-cap`/`title`, e.g.
/// `$\alpha$`) passes through untouched so math still renders.
fn unquote_value(v: &str) -> String {
    let inner = {
        let mut ch = v.chars();
        match (ch.next(), ch.next_back()) {
            (Some(a @ ('"' | '\'')), Some(b)) if a == b && v.len() >= 2 => ch.as_str(),
            // Smart-punctuation curly quotes: comrak rewrites straight quotes in the
            // rendered text, so a quoted figure `width="60%"` reaches the parser as
            // `“60%”`. Strip the matching curly pair too, else the curly quotes leak
            // into the CSS (`style="width:“60%”"`) and the value silently no-ops.
            (Some('\u{201c}'), Some('\u{201d}')) | (Some('\u{2018}'), Some('\u{2019}'))
                if v.chars().count() >= 2 =>
            {
                ch.as_str()
            }
            _ => v,
        }
    };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && matches!(chars.peek(), Some('"' | '\'')) {
            out.push(chars.next().unwrap());
        } else {
            out.push(c);
        }
    }
    out
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

/// The `data-state="…"` of the first `.step` block in `inner` (already attribute-escaped,
/// since it is read back out of the step's emitted html). Used to seed the scrolly hidden
/// input's initial value so consumer cells read a sane value before any scroll.
fn first_step_state(inner: &[Block]) -> Option<String> {
    let step = inner
        .iter()
        .find(|b| b.html.trim_start().starts_with("<div class=\"step\""))?;
    let i = step.html.find("data-state=\"")?;
    let rest = &step.html[i + "data-state=\"".len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Bundled inline icon for a callout `kind` (GitHub Octicons, MIT — see THIRD_PARTY.md;
/// `fill="currentColor"` so it takes the kind's accent). Empty for an unknown kind, which
/// is already flagged by `validate_callout_kind`. Keyed by the same vocabulary as
/// `validate::CALLOUT_KINDS`.
fn callout_icon(kind: &str) -> &'static str {
    match kind {
        // info
        "note" => {
            "<svg class=\"callout-icon\" viewBox=\"0 0 16 16\" width=\"16\" height=\"16\" fill=\"currentColor\" aria-hidden=\"true\"><path d=\"M0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm8-6.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM6.5 7.75A.75.75 0 0 1 7.25 7h1a.75.75 0 0 1 .75.75v2.75h.25a.75.75 0 0 1 0 1.5h-2a.75.75 0 0 1 0-1.5h.25v-2h-.25a.75.75 0 0 1-.75-.75ZM8 6a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z\"/></svg>"
        }
        // light-bulb
        "tip" => {
            "<svg class=\"callout-icon\" viewBox=\"0 0 16 16\" width=\"16\" height=\"16\" fill=\"currentColor\" aria-hidden=\"true\"><path d=\"M8 1.5c-2.363 0-4 1.69-4 3.75 0 .984.424 1.625.984 2.304l.214.253c.223.264.47.556.673.848.284.411.537.896.621 1.49a.75.75 0 0 1-1.484.211c-.04-.282-.163-.547-.37-.847a8.456 8.456 0 0 0-.542-.68c-.084-.1-.173-.205-.268-.32C3.201 7.75 2.5 6.766 2.5 5.25 2.5 2.31 4.863 0 8 0s5.5 2.31 5.5 5.25c0 1.516-.701 2.5-1.328 3.259-.095.115-.184.22-.268.319-.207.245-.383.453-.541.681-.208.3-.33.565-.37.847a.751.751 0 0 1-1.485-.212c.084-.593.337-1.078.621-1.489.203-.292.45-.584.673-.848.075-.088.147-.173.213-.253.561-.679.985-1.32.985-2.304 0-2.06-1.637-3.75-4-3.75ZM5.75 12h4.5a.75.75 0 0 1 0 1.5h-4.5a.75.75 0 0 1 0-1.5ZM6 15.25a.75.75 0 0 1 .75-.75h2.5a.75.75 0 0 1 0 1.5h-2.5a.75.75 0 0 1-.75-.75Z\"/></svg>"
        }
        // alert triangle
        "warning" => {
            "<svg class=\"callout-icon\" viewBox=\"0 0 16 16\" width=\"16\" height=\"16\" fill=\"currentColor\" aria-hidden=\"true\"><path d=\"M6.457 1.047c.659-1.234 2.427-1.234 3.086 0l6.082 11.378A1.75 1.75 0 0 1 14.082 15H1.918a1.75 1.75 0 0 1-1.543-2.575Zm1.763.707a.25.25 0 0 0-.44 0L1.698 13.132a.25.25 0 0 0 .22.368h12.164a.25.25 0 0 0 .22-.368Zm.53 3.996v2.5a.75.75 0 0 1-1.5 0v-2.5a.75.75 0 0 1 1.5 0ZM9 11a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z\"/></svg>"
        }
        // report
        "important" => {
            "<svg class=\"callout-icon\" viewBox=\"0 0 16 16\" width=\"16\" height=\"16\" fill=\"currentColor\" aria-hidden=\"true\"><path d=\"M0 1.75C0 .784.784 0 1.75 0h12.5C15.216 0 16 .784 16 1.75v9.5A1.75 1.75 0 0 1 14.25 13H8.06l-2.573 2.573A1.458 1.458 0 0 1 3 14.543V13H1.75A1.75 1.75 0 0 1 0 11.25Zm1.75-.25a.25.25 0 0 0-.25.25v9.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h6.5a.25.25 0 0 0 .25-.25v-9.5a.25.25 0 0 0-.25-.25Zm7 2.25v2.5a.75.75 0 0 1-1.5 0v-2.5a.75.75 0 0 1 1.5 0ZM9 9a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z\"/></svg>"
        }
        // stop
        "caution" => {
            "<svg class=\"callout-icon\" viewBox=\"0 0 16 16\" width=\"16\" height=\"16\" fill=\"currentColor\" aria-hidden=\"true\"><path d=\"M4.47.22A.749.749 0 0 1 5 0h6c.199 0 .39.079.53.22l4.25 4.25c.141.14.22.331.22.53v6a.749.749 0 0 1-.22.53l-4.25 4.25A.749.749 0 0 1 11 16H5a.749.749 0 0 1-.53-.22L.22 11.53A.749.749 0 0 1 0 11V5c0-.199.079-.39.22-.53Zm.84 1.28L1.5 5.31v5.38l3.81 3.81h5.38l3.81-3.81V5.31L10.69 1.5ZM8 4a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0v-3.5A.75.75 0 0 1 8 4Zm0 8a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z\"/></svg>"
        }
        _ => "",
    }
}

/// (display name, amsthm style suffix) for a NUMBERED theorem kind. `proof` is handled
/// separately (unnumbered) and never reaches here; an unknown kind never enters the arm.
fn theorem_meta(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "theorem" => ("Theorem", "plain"),
        "lemma" => ("Lemma", "plain"),
        "corollary" => ("Corollary", "plain"),
        "proposition" => ("Proposition", "plain"),
        "definition" => ("Definition", "definition"),
        "example" => ("Example", "definition"),
        "remark" => ("Remark", "remark"),
        _ => ("", "plain"),
    }
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
        // Validate the kind against taliesin's callout vocabulary (an unknown kind
        // warns, click-to-source, and still renders with its given class).
        if let Some(w) = super::validate::validate_callout_kind(kind, open_line, file.clone()) {
            warnings.push(w);
        }
        // Callout: use a `title="..."` attr, else a leading heading, else the kind.
        // When the title comes from a heading that carried a cross-reference anchor
        // (`{#sec-x}`), preserve that id on the title element — else the anchor is
        // stripped with the tags while `@sec-x` still resolves to a number, leaving a
        // dead link. `id` on the title makes `#sec-x` scroll to the callout. Only
        // xref-prefixed ids are hoisted (a plain autoslug title stays id-less, as before).
        let mut title_id_attr = String::new();
        let title = match attrs.get("title") {
            Some(t) => html_escape(t),
            None if inner.first().is_some_and(|b| is_heading(&b.html)) => {
                let heading = inner.remove(0).html;
                if let Some(hid) =
                    extract_attr(&heading, "id").filter(|id| crate::cite::is_xref_anchor(id))
                {
                    title_id_attr = format!(" id=\"{}\"", escape_attr(&hid));
                }
                strip_tags(&heading)
            }
            None => capitalize(kind),
        };
        // A bundled kind icon precedes the title text unless `icon="false"`.
        let icon = if attrs.get("icon") == Some("false") {
            ""
        } else {
            callout_icon(kind)
        };
        // `appearance=` selects a presentation variant (default boxed / simple / minimal).
        let appearance = match attrs.get("appearance") {
            Some("simple") => " callout-simple",
            Some("minimal") => " callout-minimal",
            _ => "",
        };
        let body = concat(&inner);
        // `collapse="true"` makes the callout a native <details> (starts closed);
        // `collapse="false"` is collapsible but starts open.
        match attrs.get("collapse") {
            Some(v) => {
                let open = if v == "false" { " open" } else { "" };
                format!(
                    "<div class=\"callout callout-{kind} callout-collapse{appearance}\"{data}><details{open}><summary class=\"callout-title\"{title_id_attr}>{icon}{title}</summary><div class=\"callout-body\">{body}</div></details></div>"
                )
            }
            None => format!(
                "<div class=\"callout callout-{kind}{appearance}\"{data}><div class=\"callout-title\"{title_id_attr}>{icon}{title}</div><div class=\"callout-body\">{body}</div></div>"
            ),
        }
    } else if let Some(ncol) = attrs.get("layout-ncol").and_then(|n| n.parse::<u32>().ok()) {
        let body = concat(&inner);
        format!(
            "<div class=\"tali-layout\" style=\"display:grid;grid-template-columns:repeat({ncol},minmax(0,1fr));gap:1rem\"{data}>{body}</div>"
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
        // highlighting (walkthrough.js, reusing the `.tali-hl-ln` contract). Read-only:
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
        // A scroll step: carry its line-focus spec as `data-cw-lines` (walkthrough.js) and/or
        // its scrolly state as `data-state` (scrolly.js); keep the div's own id/sourcepos so
        // its prose stays locatable. Meaningful inside `.code-walkthrough`/`.scrolly`.
        let id_attr = id_attr(attrs.id.as_deref());
        let cw_lines = match attrs.get("lines") {
            Some(spec) if !spec.is_empty() => {
                format!(" data-cw-lines=\"{}\"", escape_attr(spec))
            }
            _ => String::new(),
        };
        let state = match attrs.get("state") {
            Some(s) if !s.is_empty() => format!(" data-state=\"{}\"", escape_attr(s)),
            _ => String::new(),
        };
        let body = concat(&inner);
        format!("<div class=\"step\"{id_attr}{data}{cw_lines}{state}>{body}</div>")
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
                        // `label` is `strip_tags` output — already HTML-safe (the heading's
                        // entities are intact), so it must NOT be `html_escape`'d again
                        // (that turned `&amp;` into `&amp;amp;`).
                        "<button class=\"tabset-tab\" role=\"tab\" id=\"{tab_id}\" aria-controls=\"{panel_id}\" aria-selected=\"{sel}\" tabindex=\"{}\">{label}</button>",
                        if sel { "0" } else { "-1" },
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
    } else if attrs.classes.iter().any(|c| c == "scrolly") {
        // Scrollytelling: a sticky visual stage (the non-.step inner blocks) beside a
        // scrolling column of `.step` divs. The active step (scrolly.js, IntersectionObserver)
        // sets `data-scrolly-state` on the root for CSS, and — when `name=` is set — drives a
        // hidden `data-qmd-input` so a sticky `{js}` cell reacts via `//| input:` through the
        // shipped reactive graph. Read-only: scroll is reader interaction, never a source write.
        let is_step = |b: &Block| b.html.trim_start().starts_with("<div class=\"step\"");
        let steps: String = inner
            .iter()
            .filter(|b| is_step(b))
            .map(|b| b.html.as_str())
            .collect();
        let stage: String = inner
            .iter()
            .filter(|b| !is_step(b))
            .map(|b| b.html.as_str())
            .collect();
        let has_steps = inner.iter().any(is_step);
        let has_stage = inner.iter().any(|b| !is_step(b));
        for w in super::validate::validate_scrolly(has_stage, has_steps, open_line, file.clone()) {
            warnings.push(w);
        }
        // The reactive bridge: a hidden input named `name` whose value is the active step's
        // state (initial = the first .step's state, so consumer cells read a sane value).
        let (name_attr, hidden) = match attrs.get("name") {
            Some(n) if !n.is_empty() => {
                // `first_step_state` is read back out of the already-emitted step html, so it
                // is already attribute-escaped — do NOT re-escape it.
                let first_state = first_step_state(&inner).unwrap_or_default();
                (
                    format!(" data-scrolly-name=\"{}\"", escape_attr(n)),
                    format!(
                        "<input type=\"hidden\" class=\"tali-scrolly-input\" data-qmd-input=\"{}\" value=\"{first_state}\">",
                        escape_attr(n)
                    ),
                )
            }
            _ => (String::new(), String::new()),
        };
        format!(
            "<div class=\"tali-scrolly\"{data}{name_attr}>{hidden}<div class=\"scrolly-steps\">{steps}</div><div class=\"scrolly-stage\">{stage}</div></div>"
        )
    } else if let Some(kind) = attrs.theorem_kind() {
        let body = concat(&inner);
        if kind == "proof" {
            // Unnumbered, not cross-referenceable (matches common convention). Auto-QED.
            let head = attrs
                .get("title")
                .map(html_escape)
                .unwrap_or_else(|| "Proof".to_string());
            let qed = "<span class=\"tali-qed\" aria-hidden=\"true\">\u{220e}</span>";
            // `collapse="true"` folds the proof behind a native <details> (starts closed);
            // `collapse="false"` is collapsible but starts open. QED rides inside <details>
            // so a collapsed proof shows only its "Proof." summary.
            match attrs.get("collapse") {
                Some(v) => {
                    let open = if v == "false" { " open" } else { "" };
                    format!(
                        "<div class=\"tali-proof tali-proof-collapse\"{data}><details{open}><summary class=\"tali-proof-head\">{head}.</summary><div class=\"tali-theorem-body\">{body}</div>{qed}</details></div>"
                    )
                }
                None => format!(
                    "<div class=\"tali-proof\"{data}><p class=\"tali-proof-head\">{head}.</p><div class=\"tali-theorem-body\">{body}</div>{qed}</div>"
                ),
            }
        } else {
            // The number slot is filled by the `number_theorems` post-pass (after
            // group_divs, before cite::process), so numbering stays document-ordered.
            let (name, style) = theorem_meta(kind);
            let id_attr = id_attr(attrs.id.as_deref());
            let title = match attrs.get("title") {
                Some(t) => format!(
                    " <span class=\"tali-theorem-title\">({})</span>",
                    html_escape(t)
                ),
                None => String::new(),
            };
            format!(
                "<div class=\"tali-theorem tali-theorem-{kind} tali-thm-style-{style}\"{id_attr} data-qmd-theorem-kind=\"{kind}\"{data}><p class=\"tali-theorem-head\"><span class=\"tali-theorem-label\">{name}<span class=\"tali-theorem-number\"></span></span>{title}</p><div class=\"tali-theorem-body\">{body}</div></div>"
            )
        }
    } else {
        let mut class = attrs.classes.join(" ");
        if class.is_empty() {
            class.push_str("tali-div");
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

#[cfg(test)]
mod tests {
    use super::parse_attrs;

    #[test]
    fn escaped_quotes_inside_a_value_do_not_truncate_or_leak_backslash() {
        // A `\"` inside a quoted value must NOT end the value early, and the
        // backslash must not survive into the parsed text.
        let a = parse_attrs(r#".callout-note title="She said \"hi\"""#);
        assert_eq!(a.get("title"), Some("She said \"hi\""));
        assert_eq!(a.classes, vec!["callout-note".to_string()]);
    }

    #[test]
    fn escaped_single_quotes_unescape_inside_single_quoted_value() {
        let a = parse_attrs(r#"title='it\'s here'"#);
        assert_eq!(a.get("title"), Some("it's here"));
    }

    #[test]
    fn latex_backslashes_in_a_caption_survive_unchanged() {
        // Only `\"`/`\'` are escapes; a LaTeX macro's backslash passes through so
        // math in a `fig-cap`/`title` still renders (e.g. `$\alpha$`).
        let a = parse_attrs(r#"fig-cap="$\alpha$ and \beta""#);
        assert_eq!(a.get("fig-cap"), Some(r"$\alpha$ and \beta"));
    }

    #[test]
    fn plain_quoted_and_unquoted_values_are_unchanged() {
        let a = parse_attrs(r#".x #anchor key="a b" bare=v"#);
        assert_eq!(a.classes, vec!["x".to_string()]);
        assert_eq!(a.id.as_deref(), Some("anchor"));
        assert_eq!(a.get("key"), Some("a b"));
        assert_eq!(a.get("bare"), Some("v"));
    }
}
