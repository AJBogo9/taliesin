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
        } else {
            let i = tok.strip_prefix('#').filter(|i| !i.is_empty())?;
            id = Some(i.to_string());
        }
    }
    Some((classes, id))
}

/// Blank out fenced-div markers (`::: {...}` / `:::`) without changing
/// the line count, so the inner content parses as ordinary blocks and every
/// other block's sourcepos line numbers stay valid against the original source.
///
/// Also indents display-math continuation lines that would otherwise start a new
/// block (see [`interrupts_paragraph`]). Both passes are line-preserving, which is
/// what keeps every sourcepos honest.
pub(crate) fn preprocess(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_code: Option<(char, usize)> = None;
    // Indentation of the line that opened the display-math block we are inside.
    let mut math_open: Option<usize> = None;
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
        // Display math is only display math outside a code fence, for the same reason.
        let outside_code = !was_in_code && in_code.is_none();
        let mut masked = None;
        if outside_code && !blank {
            match math_open {
                None => math_open = display_math_open_indent(line),
                // A blank line ends the paragraph, so the block never closes and there
                // is nothing left to protect; closing delimiters end it normally.
                Some(_) if line.trim().is_empty() || closes_display_math(line) => math_open = None,
                Some(open_indent) => {
                    let indent = line.len() - line.trim_start_matches(' ').len();
                    let target = open_indent + 4;
                    if indent < target && interrupts_paragraph(line) {
                        masked = Some(format!("{}{line}", " ".repeat(target - indent)));
                    }
                }
            }
        }
        if !blank {
            out.push_str(masked.as_deref().unwrap_or(line));
        }
    }
    if src.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Indentation of `line` if it opens a multi-line display-math block: `$$` or a bare
/// `\begin{env}` that does not also close on the same line. A one-line `$$a+b$$` needs
/// no protection (nothing can interrupt a single line), so it does not open a region.
fn display_math_open_indent(line: &str) -> Option<usize> {
    let trimmed = line.trim_start_matches(' ');
    let indent = line.len() - trimmed.len();
    if let Some(rest) = trimmed.strip_prefix("$$") {
        return (!rest.contains("$$")).then_some(indent);
    }
    // Pandoc treats a bare `\begin{env}…\end{env}` as display math; `bare_math_env`
    // renders it, and it is split by a list marker exactly the same way.
    if trimmed.starts_with("\\begin{") && !trimmed.contains("\\end{") {
        return Some(indent);
    }
    None
}

fn closes_display_math(line: &str) -> bool {
    line.contains("$$") || line.contains("\\end{")
}

/// Would this line start a new block, interrupting the paragraph that a multi-line
/// display-math block lives inside? `math_dollars` is an inline extension, so the
/// whole `$$…$$` run is one paragraph and CommonMark lets these markers cut it in
/// two. Only these lines are re-indented: leaving every other line untouched keeps
/// block-id churn (ids hash the source) to the documents that were actually broken.
fn interrupts_paragraph(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() >= 4 {
        return false; // already indented enough to be a lazy continuation
    }
    let Some(first) = trimmed.chars().next() else {
        return false;
    };
    let rest = &trimmed[first.len_utf8()..];
    // A run of 3+ `-`/`*`/`_` (spaces allowed) is a thematic break.
    let thematic = matches!(first, '-' | '*' | '_')
        && trimmed.chars().all(|c| c == first || c == ' ')
        && trimmed.chars().filter(|&c| c == first).count() >= 3;
    match first {
        // A bullet marker interrupts only with non-empty content after it.
        '-' | '+' | '*' => thematic || (rest.starts_with(' ') && !rest.trim().is_empty()),
        '_' => thematic,
        '>' => true,
        '#' => {
            let hashes = trimmed.chars().take_while(|&c| c == '#').count();
            (1..=6).contains(&hashes)
                && (trimmed.len() == hashes || trimmed[hashes..].starts_with(' '))
        }
        // A fenced code block interrupts a paragraph.
        '`' | '~' => trimmed.chars().take_while(|&c| c == first).count() >= 3,
        // Only `1.`/`1)` may interrupt a paragraph (CommonMark restricts the start number).
        '1' => rest.starts_with(". ") || rest.starts_with(") "),
        _ => false,
    }
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
///
/// `pub` so the LSP's attribute-slot completion reads the div's own tokenizer instead of
/// re-deriving it: splitting on whitespace naively makes `title="a b"` two tokens and the
/// stray `b"` look like a class name.
pub fn tokenize_attrs(s: &str) -> Vec<String> {
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
    debug_names: &mut HashSet<String>,
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

    // An empty div that names a real feature (a `.input` control, a `.callout-*`, a
    // `.panel-tabset`, a theorem, …) is silently dropped below (the "skip degenerate spans"
    // step) and renders nothing — the exact `::: {.input name="k"}` trap. Warn (located) before
    // dropping it. Position-independent (a span is empty when no flat block falls between its
    // fences), so a trailing or standalone empty feature div is caught too; a plain/custom
    // empty div stays silent (`validate_empty_feature_div` returns `None`).
    for span in spans {
        let has_content = flat
            .iter()
            .any(|fb| fb.buf_start > span.open && fb.buf_start < span.close);
        if has_content {
            continue;
        }
        let (file, line) = map_origin(origins, span.open);
        let attrs = parse_attrs(&span.attrs);
        if let Some(w) = super::validate::validate_empty_feature_div(&attrs.classes, line, file) {
            warnings.push(w);
        }
    }

    for (i, fb) in flat.iter().enumerate() {
        // Skip any spans that already closed before this block (degenerate/empty divs, spans
        // whose blocks are all consumed) BEFORE opening containing spans. Skipping first is
        // load-bearing: an empty div at `span_idx` has `close < buf_start` but is not a
        // containing span, so the open loop would stop on it and never reach the block's own
        // container — which silently drops the block out of its div (a `.column` after an empty
        // `.input`, say). Spans are open-sorted, so a still-open ancestor already sits below
        // `span_idx` on the stack and is never skipped here.
        while span_idx < spans.len() && spans[span_idx].close < fb.buf_start {
            span_idx += 1;
        }
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

        push_block(&mut stack, &mut result, fb.block.clone());

        // Close spans that end before the next block begins (innermost first).
        let next_start = flat.get(i + 1).map(|n| n.buf_start).unwrap_or(usize::MAX);
        while let Some(top) = stack.last() {
            if top.span.close < next_start {
                let done = stack.pop().unwrap();
                let container = build_container(
                    done.span,
                    done.inner,
                    origins,
                    counts,
                    debug_names,
                    warnings,
                );
                push_block(&mut stack, &mut result, container);
            } else {
                break;
            }
        }
    }
    // Close anything still open (e.g. unterminated div at EOF).
    while let Some(done) = stack.pop() {
        let container = build_container(
            done.span,
            done.inner,
            origins,
            counts,
            debug_names,
            warnings,
        );
        push_block(&mut stack, &mut result, container);
    }
    // A traced cell that never made it into a `.debug` div: every div type folds its
    // inner blocks into ONE composite top-level `Block` (this function's own loop above,
    // via `build_container`), so a stray `#| trace: true` (bare, or nested inside some
    // OTHER div) is still findable as a top-level `result` entry whose html carries the
    // trace marker without the `.debug` container's own class alongside it.
    for b in &result {
        if let Some(w) = super::validate::validate_stray_trace(
            &b.html,
            sourcepos_start_line(&b.sourcepos),
            b.source_file.clone(),
        ) {
            warnings.push(w);
        }
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

/// Give every `.step` in `steps` the semantics its container implies, and point it at the
/// sticky stage it drives (AP7-3).
///
/// A `.scrolly` and a `.code-walkthrough` were measured carrying **no** accessibility
/// semantics at all: 0 steps with `aria`/`role`, `null` root role, nothing associating a
/// step with the thing it advances. The step prose reads fine linearly, so a screen-reader
/// user gets the words; what they never got is the **stage** — the sticky visual, or the
/// highlighted code lines — whose state moves only as a consequence of *visual scrolling*.
///
/// Two facts the renderer already has and threw away: the step's ordinal, and (in a
/// walkthrough) the `lines=` range it focuses. A labelled `group` carries both without
/// injecting text into the document: an `aria-label` on a bare `<div>` is ignored by AT,
/// but on a `role="group"` it is announced, and unlike a visually-hidden span it stays out
/// of `indexable_text`, so the Cmd-K index and `llms.txt` are unchanged.
///
/// Steps are deliberately NOT made focusable. They are prose, a keyboard user reads them by
/// scrolling like anyone else, and putting `tabindex` on paragraphs would add tab stops
/// without adding any capability.
fn label_steps(steps: &str, stage_id: &str, lines_in_label: bool) -> String {
    const OPEN: &str = "<div class=\"step\"";
    let total = steps.matches(OPEN).count();
    if total == 0 {
        return steps.to_string();
    }
    let mut out = String::with_capacity(steps.len() + total * 80);
    let mut rest = steps;
    let mut n = 0usize;
    while let Some(i) = rest.find(OPEN) {
        n += 1;
        out.push_str(&rest[..i + OPEN.len()]);
        rest = &rest[i + OPEN.len()..];
        // The step's own attributes run to the end of its opening tag; read `data-cw-lines`
        // out of them so the label can name the range this step highlights.
        let tag_end = rest.find('>').unwrap_or(0);
        let label = match lines_in_label
            .then(|| attr_value(&rest[..tag_end], "data-cw-lines"))
            .flatten()
        {
            Some(spec) => format!("Step {n} of {total}, highlighting {}", spoken_lines(&spec)),
            None => format!("Step {n} of {total}"),
        };
        out.push_str(&format!(
            " role=\"group\" aria-label=\"{label}\" aria-controls=\"{stage_id}\""
        ));
    }
    out.push_str(rest);
    out
}

/// A `.step lines=` spec written for speech: `"3-4"` -> `"lines 3 to 4"`, `"1"` ->
/// `"line 1"`, `"3-5,8"` -> `"lines 3 to 5, 8"`. The raw spec is a machine value that a
/// screen reader reads as "three dash four" or "three minus four"; this is the one place it
/// becomes something a person hears, so it is spelled out.
fn spoken_lines(spec: &str) -> String {
    let parts: Vec<String> = spec
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('-') {
            Some((a, b)) if !a.is_empty() && !b.is_empty() => format!("{a} to {b}"),
            _ => p.to_string(),
        })
        .collect();
    // Plural unless it is exactly one bare line number ("lines 3 to 4" is still plural).
    let one = parts.len() == 1 && !parts[0].contains(" to ");
    format!(
        "{} {}",
        if one { "line" } else { "lines" },
        parts.join(", ")
    )
}

/// The container's own `data-block-id`, read back out of the `data` attribute string the
/// caller is about to interpolate. Used to mint a stage id that is unique per container and
/// stable across re-renders of unchanged source (the block id is a content hash), so the
/// `aria-controls` wiring cannot collide between two walkthroughs on one page.
fn block_id_of(data: &str) -> Option<String> {
    attr_value(data, "data-block-id")
}

/// The value of `name="…"` inside an already-emitted opening tag, or `None`. The value is
/// left exactly as emitted (already attribute-escaped), like [`first_step_state`].
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let i = tag.find(&key)? + key.len();
    let rest = &tag[i..];
    let end = rest.find('"')?;
    (!rest[..end].is_empty()).then(|| rest[..end].to_string())
}

/// Whether an already-emitted block is a cell marked `trace: true`. The emitter has
/// stripped the `#|`/`//|` directive lines out of the displayed source by this point, so
/// the marker is read off the attribute `emit.rs` leaves behind, never by re-scanning the
/// source text.
fn is_traced_cell(b: &Block) -> bool {
    b.html.contains("data-tali-trace=\"1\"")
}

/// Whether a folded child block is a rendered code block (a cell or a plain fence).
fn is_code_block(b: &Block) -> bool {
    b.html.contains("<pre") && b.html.contains("<code")
}

/// The child index a `::: {.debug}` turns into its stepped code panel: the traced cell,
/// else the first code block.
///
/// The panel follows the trace, not document order. A `.debug` may hold more than one code
/// block (`validate_debug` warns on it, but the div still renders), and if the traced cell
/// isn't the first code block, showing the first one would silently step through code the
/// reader never sees highlighted. Falling back to the first code block only when nothing is
/// traced is covered by `validate_debug`'s "no traced cell" warning.
///
/// ONE definition because two callers need the same answer: the `.debug` branch builds the
/// panel from it, and the nested-cell collection above skips exactly that child (a `.debug`
/// hoists its traced cell onto the container's own `cell`, so collecting it as a nested cell
/// too would run it twice per rebuild).
fn debug_code_idx(inner: &[Block]) -> Option<usize> {
    inner
        .iter()
        .position(|b| is_code_block(b) && is_traced_cell(b))
        .or_else(|| inner.iter().position(is_code_block))
}

/// The attribute marking a container's empty output slot, keyed by the folded cell's own
/// block id.
///
/// ONE definition, shared with `taliesin-server`'s `exec::fill_output_slot`, which finds
/// the slot by this exact name when the cell's output comes back. Two spellings of a
/// string one side writes and the other searches for would fail the way this project likes
/// least: silently, as an output that simply never appears.
pub const CELL_OUT_SLOT_ATTR: &str = "data-tali-out-for";

/// The empty output slot a container leaves in its HTML after a code cell it folds away,
/// so the executor can splice that cell's output back INSIDE the container.
///
/// Byte-compatible with the top-level output block `exec.rs` builds for an unfolded cell —
/// same `tali-output` class, same `{id}-out` block id, same click-to-source position — so
/// the browser code that finds a running cell's output by `{id}-out` (the streaming host in
/// `client.js`, the per-cell state ring) works on a nested cell with no second lookup path,
/// and `.tali-output:empty` in base.css collapses one that never filled.
///
/// [`CELL_OUT_SLOT_ATTR`] comes LAST on purpose: the executor fills the slot by splicing
/// at the exact literal `<attr>="<id>"></div>`, which needs no HTML parsing, cannot match
/// a filled slot, and cannot collide with anything else on the page (block ids are unique).
fn output_slot(b: &Block) -> String {
    format!(
        "<div class=\"tali-output\" data-block-id=\"{id}-out\" data-sourcepos=\"{pos}\"{file} {attr}=\"{id}\"></div>",
        id = b.id,
        pos = b.sourcepos,
        file = source_file_attr(b.source_file.as_deref()),
        attr = CELL_OUT_SLOT_ATTR,
    )
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
        _ => "",
    }
}

/// (display name, amsthm style suffix) for a NUMBERED theorem kind. `proof` is handled
/// separately (unnumbered) and never reaches here; an unknown kind never enters the arm.
pub(crate) fn theorem_meta(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "theorem" => ("Theorem", "plain"),
        "lemma" => ("Lemma", "plain"),
        "corollary" => ("Corollary", "plain"),
        "definition" => ("Definition", "definition"),
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
    debug_names: &mut HashSet<String>,
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

    // A fenced-div's own composite block never carries a `Cell`: its children are
    // folded into one `html` string below, and by construction that folding is the
    // only place their per-block identity survives. For `.debug` that is fatal to
    // the whole feature, because `Executor::run_through` (crates/server/src/exec.rs)
    // only scans TOP-LEVEL blocks for a `Cell` to run: once this container replaces
    // its children in the flat list, a traced cell folded away with `cell: None`
    // would never execute at all. Set below, inside the `.debug` branch, to the same
    // cell `code_idx` there resolves to (the traced cell, or the first code block).
    let mut debug_cell: Option<Cell> = None;

    // Every OTHER folded cell has the same problem and, until item 210, the same fate: a
    // `{python}` cell in a `.callout-note`, a `.panel-tabset` or a `layout-ncol` grid
    // rendered and never ran. They are collected onto `Block::nested` here, each with an
    // empty output slot left after it in the folded HTML, so the executor can run them in
    // document order and put each output back where its cell sits. `.debug` is the one
    // exception: it hoists its traced cell onto `debug_cell` instead, because `debug.js`
    // reads that trace off the SIBLING block the executor splices in after the container
    // (and collecting it here as well would run the cell twice per rebuild).
    //
    // `group_divs` closes containers innermost-first, so a nested container has already
    // done this to its own children: taking its `nested` list wholesale flattens every
    // cell in the document to one level while each slot stays where it belongs inside the
    // folded HTML. That is also what makes a `.debug` inside another div work, which
    // `validate_nested_debug` used to have to warn about.
    let debug_hoist = attrs
        .classes
        .iter()
        .any(|c| c == "debug")
        .then(|| debug_code_idx(&inner))
        .flatten();
    //
    // Only a cell in a language the *kernel* runs earns a slot. A `{js}` cell mounts its
    // own live target client-side and never produces a server-side output block, so a slot
    // after one would be an element that can never fill — which is exactly what the
    // `explorable/scrolly.tmd` snapshot caught. `executes_to_kernel` is the canonical set
    // (drift-locked to `exec::kernel_lang` by a test), so this asks it rather than
    // re-listing the languages.
    let mut nested: Vec<Block> = Vec::new();
    for (i, b) in inner.iter_mut().enumerate() {
        nested.append(&mut b.nested);
        let runs = b.cell.as_ref().is_some_and(|c| executes_to_kernel(&c.lang));
        if runs && Some(i) != debug_hoist {
            b.html.push_str(&output_slot(b));
            nested.push(b.clone());
        }
    }

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
    } else if attrs.classes.iter().any(|c| c == "debug") {
        // Algorithm debug mode: the first code block becomes the stepped panel, the rest
        // (a `{js}` view cell, prose) ride alongside. `debug.js` builds the transport bar,
        // the variables panel and the data views from the trace at runtime, so the server
        // emits only structure. Line-wrapped with the SAME helper magic-move and the
        // walkthrough use, so the cursor reuses the `.tali-hl-ln` contract already styled
        // in base.css instead of inventing a second one.
        //
        // `.column-page` is applied here rather than left to the author: the reading
        // measure (~70ch) cannot hold a code panel beside a data view, and requiring
        // `::: {.debug .column-page}` would make every author repeat the same escape.
        let traced = inner.iter().filter(|b| is_traced_cell(b)).count();
        // Resolved once, above, because the nested-cell collection has to skip exactly the
        // child this hoists (see `debug_code_idx`).
        let code_idx = debug_hoist;
        let name = attrs.get("name").filter(|n| !n.is_empty());
        for w in super::validate::validate_debug(
            traced,
            code_idx.is_some(),
            name.is_some(),
            open_line,
            file.clone(),
        ) {
            warnings.push(w);
        }
        // Two `.debug` blocks sharing one `name=` overwrite each other's
        // `tali.frame(name)` registry entry AND fight over one `[data-tali-input]`
        // bridge (`debug.js`'s `registry`/`mount`): a silent collision this project
        // treats as an authoring mistake, not a supported pattern. First definition
        // wins (kept in `debug_names`), the SECOND is the one located, matching
        // `register_xref`'s "keep first, warn on the duplicate" rule.
        if let Some(n) = name
            && !debug_names.insert(n.to_string())
        {
            warnings.push(super::validate::validate_duplicate_debug_name(
                n,
                open_line,
                file.clone(),
            ));
        }
        let hidden = match name {
            Some(n) => format!(
                "<input type=\"hidden\" class=\"tali-debug-input\" data-tali-input=\"{}\" value=\"0\">",
                escape_attr(n)
            ),
            None => String::new(),
        };
        let name_attr = match name {
            Some(n) => format!(" data-debug-name=\"{}\"", escape_attr(n)),
            None => String::new(),
        };
        let code_id = format!("{}-code", block_id_of(&data).unwrap_or_default());
        match code_idx {
            Some(i) => {
                let panel = super::emit::wrap_pre_lines(&inner[i].html);
                // Carry the panel cell's own `Cell` onto the container (see the
                // `debug_cell` declaration above): this is what makes the traced
                // cell reachable by the executor at all once its block is folded
                // away below.
                debug_cell = inner[i].cell.clone();
                // A traced `{js}` cell's own `//| input:` names, so `debug.js` knows
                // which reactive inputs should re-capture and re-render this block
                // when they change (the JS adapter's whole reason to exist over the
                // Python one: the reader can change the input and re-run). The server
                // strips `//|` option lines from the displayed source, so this is the
                // only place those names survive into the DOM. `debug_cell.js.inputs`
                // is empty for a Python cell (options are only parsed for a
                // client-side language), so the attribute is simply absent there.
                let inputs_attr = debug_cell
                    .as_ref()
                    .filter(|c| !c.js.inputs.is_empty())
                    .map(|c| {
                        format!(
                            " data-debug-inputs=\"{}\"",
                            escape_attr(&c.js.inputs.join(","))
                        )
                    })
                    .unwrap_or_default();
                let rest: String = inner
                    .iter()
                    .enumerate()
                    .filter_map(|(j, b)| (j != i).then_some(b.html.as_str()))
                    .collect();
                format!(
                    "<div class=\"tali-debug column-page\" role=\"group\" \
                     aria-label=\"Algorithm debugger\"{data}{name_attr}{inputs_attr}>\
                     {hidden}<div class=\"dbg-code\" id=\"{code_id}\">{panel}</div>\
                     <div class=\"dbg-views\">{rest}</div></div>"
                )
            }
            None => {
                let body = concat(&inner);
                format!(
                    "<div class=\"tali-debug column-page\"{data}{name_attr}>\
                     <div class=\"dbg-views\">{body}</div></div>"
                )
            }
        }
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
                // AP7-3: name the region and tie each step to the code panel it drives. The
                // stage id is derived from the container's own block id, so it is unique per
                // walkthrough and stable across re-renders of unchanged source.
                let stage_id = format!("{}-stage", block_id_of(&data).unwrap_or_default());
                let steps = label_steps(&steps, &stage_id, true);
                format!(
                    "<div class=\"code-walkthrough\" role=\"group\" aria-label=\"Code walkthrough\"{data}>\
                     <div class=\"cw-steps\">{steps}</div>\
                     <div class=\"cw-stage\" id=\"{stage_id}\" role=\"group\" \
                     aria-label=\"The code these steps walk through\">\
                     <div class=\"cw-code\">{panel}</div></div></div>"
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
                // A `|` here is a deck `code-line-numbers=` habit that focuses zero lines in a
                // step's comma-only parser — warn (located) instead of degrading silently.
                if let Some(w) = super::validate::validate_step_lines(spec, open_line, file.clone())
                {
                    warnings.push(w);
                }
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
                        // `hidden="until-found"`, not a bare `hidden`: an inactive panel's
                        // text is in the built HTML and IS in the Cmd-K index, but a bare
                        // `hidden` makes it invisible to the browser's own find-in-page, so
                        // the tool advertised a searchability Ctrl-F did not honour. With
                        // `until-found` the browser reveals the panel and scrolls to the hit.
                        // Engines without it fall back to plain `hidden` (today's behaviour),
                        // so nothing regresses.
                        "<div class=\"tabset-panel\" role=\"tabpanel\" id=\"{panel_id}\" aria-labelledby=\"{tab_id}\"{}>{body}</div>",
                        if sel { "" } else { " hidden=\"until-found\"" },
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
        // hidden `data-tali-input` so a sticky `{js}` cell reacts via `//| input:` through the
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
                        "<input type=\"hidden\" class=\"tali-scrolly-input\" data-tali-input=\"{}\" value=\"{first_state}\">",
                        escape_attr(n)
                    ),
                )
            }
            _ => (String::new(), String::new()),
        };
        // AP7-3, the same treatment as `.code-walkthrough`: name the region, and tie each
        // step to the sticky stage whose state it advances. A scrolly step has no `lines=`
        // to name (its `state=` is an author token for `scrolly.js`, not reader prose), so
        // its label is the ordinal alone.
        let stage_id = format!("{}-stage", block_id_of(&data).unwrap_or_default());
        let steps = label_steps(&steps, &stage_id, false);
        format!(
            "<div class=\"tali-scrolly\" role=\"group\" aria-label=\"Scroll-driven walkthrough\"{data}{name_attr}>\
             {hidden}<div class=\"scrolly-steps\">{steps}</div>\
             <div class=\"scrolly-stage\" id=\"{stage_id}\" role=\"group\" \
             aria-label=\"The graphic these steps drive\">{stage}</div></div>"
        )
    } else if let Some(kind) = attrs.theorem_kind() {
        if kind == "proof" {
            let body = concat(&inner);
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
            // Title: an explicit `title="..."`, else a leading heading (the same gesture that
            // names a callout, so a theorem led by a heading names the box instead of rendering
            // the heading as body), else nothing. A hoisted heading that carried an xref anchor
            // keeps its id on the title span so `@thm-x`/`#thm-x` still resolves + scrolls here.
            let title = match attrs.get("title") {
                Some(t) => format!(
                    " <span class=\"tali-theorem-title\">({})</span>",
                    html_escape(t)
                ),
                None if inner.first().is_some_and(|b| is_heading(&b.html)) => {
                    let heading = inner.remove(0).html;
                    let id = extract_attr(&heading, "id")
                        .filter(|id| crate::cite::is_xref_anchor(id))
                        .map(|hid| format!(" id=\"{}\"", escape_attr(&hid)))
                        .unwrap_or_default();
                    format!(
                        " <span class=\"tali-theorem-title\"{id}>({})</span>",
                        strip_tags(&heading)
                    )
                }
                None => String::new(),
            };
            let body = concat(&inner);
            format!(
                "<div class=\"tali-theorem tali-theorem-{kind} tali-thm-style-{style}\"{id_attr} data-tali-theorem-kind=\"{kind}\"{data}><p class=\"tali-theorem-head\"><span class=\"tali-theorem-label\">{name}<span class=\"tali-theorem-number\"></span></span>{title}</p><div class=\"tali-theorem-body\">{body}</div></div>"
            )
        }
    } else {
        // Generic div: any class the author wants (styled by their own CSS). A class that is a
        // near-miss of a known feature/theorem class is almost certainly a typo that silently
        // degraded (a `.columns` that stacked, a `.fragmnet` that never revealed) — warn with a
        // did-you-mean (located, click-to-source). Genuine custom classes stay silent.
        if let Some(w) =
            super::validate::validate_div_class(&attrs.classes, open_line, file.clone())
        {
            warnings.push(w);
        }
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
        cell: debug_cell,
        nested,
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
