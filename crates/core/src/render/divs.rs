//! `:::` fenced-div preprocessing and container building: scan div spans,
//! strip the fence markers (line-preserving) so inner content parses as normal
//! blocks, then regroup blocks into callouts/columns/etc. Also `parse_attrs`, the
//! fenced-div attribute parser. Split out of the render module; `use super::*` reaches
//! the block model + helpers (Block, FlatBlock, DivAttrs, make_id, escaping).

use super::*;

/// Blank out the fenced-div markers `divs` found (line-preserving), so the inner content
/// parses as ordinary blocks and every other block's sourcepos line numbers stay valid
/// against the original source.
///
/// Also indents display-math continuation lines that would otherwise start a new
/// block (see [`interrupts_paragraph`]). Both passes are line-preserving, which is
/// what keeps every sourcepos honest.
pub(crate) fn preprocess(src: &str, divs: &DivFences) -> String {
    let blanked = divs.blank(src);
    if !blanked
        .lines()
        .any(|line| display_math_open_indent(line).is_some())
    {
        return blanked;
    }
    // Display math is only display math where markdown is read: a `$$` shown in a code
    // sample, a comment or the front matter is text. Asked of the buffer comrak is about to
    // parse, with its div markers already blank.
    let lines = crate::lines::classify(&blanked);
    let mut out = String::with_capacity(blanked.len());
    // Indentation of the line that opened the display-math block we are inside.
    let mut math_open: Option<usize> = None;
    for (i, line) in blanked.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let mut masked = None;
        if lines.line(i).kind.is_markdown() && !divs.is_marker(i) {
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
        out.push_str(masked.as_deref().unwrap_or(line));
    }
    if blanked.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// The structure the render will parse from `src`: [`crate::lines::classify`] of the buffer
/// with its div markers blank, which is exactly what comrak is handed. Blanking matters: a
/// `:::` line read as text is a paragraph line, and the four-space-indented line after it a
/// lazy continuation of it rather than the indented code the page renders. A raw source is
/// taken too: its lone `\r`s are normalized as the render's ingest does, so the numbering
/// is comrak's either way.
pub(crate) fn rendered_lines(src: &str) -> crate::lines::Lines {
    let src = crate::includes::normalize_line_endings(src);
    crate::lines::classify(&DivFences::find(&src).blank(&src))
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

/// The `:::` lines of a buffer that are fenced-div markers, found once and shared by
/// [`scan_div_spans`] (which pairs them) and [`preprocess`] (which blanks them).
///
/// A marker is a `:::` line indented at most three spaces (like every block start) that
/// comrak reads as top-level markdown: not code, not raw HTML, not front matter, and not
/// inside a list item or block quote. That last rule is not new policy but what
/// [`group_divs`] could always do: it wraps top-level blocks, so a div opened inside a list
/// item never had a block to wrap and was reported as an empty div.
#[derive(Default)]
pub(crate) struct DivFences {
    /// `(0-based line, marker)`, in line order.
    markers: Vec<(usize, Fence)>,
    /// 0-based lines opening a div inside a list item or block quote: text there, and warned.
    in_container: Vec<usize>,
}

impl DivFences {
    /// Find the markers of `src` (a post-include buffer).
    ///
    /// Where a marker sits is asked of comrak with the marker replaced by `***`, a thematic
    /// break at the same indentation. Not the raw line: a `:::` line read as text is a lazy
    /// continuation of a list item or paragraph above it, so the `:::` closing a callout
    /// that ends in a list would sit inside the list. Not a blank line either, though a blank
    /// is what the render will see: a blank after a list item's last line is outside the
    /// item even when the marker was indented into it, so the closing marker of a div
    /// written inside an item would close the div the whole list sits in. A thematic break
    /// starts a block wherever a marker does and ends the list, quote or paragraph above it
    /// unless indented into it, which is exactly the question.
    pub(crate) fn find(src: &str) -> DivFences {
        // `(line, length of what precedes the colons, quoted, marker)`.
        let candidates: Vec<(usize, usize, bool, Fence)> = src
            .lines()
            .enumerate()
            .filter_map(|(i, line)| {
                div_marker(line).map(|(prefix, f)| (i, prefix, line[..prefix].contains('>'), f))
            })
            .collect();
        if candidates.is_empty() {
            return DivFences::default();
        }
        let mut marked = String::with_capacity(src.len());
        let mut next = candidates.iter().peekable();
        for (i, line) in src.lines().enumerate() {
            match next.peek() {
                Some((at, prefix, _, _)) if *at == i => {
                    marked.push_str(&line[..*prefix]);
                    marked.push_str("***");
                    next.next();
                }
                _ => marked.push_str(line),
            }
            marked.push('\n');
        }
        let lines = crate::lines::classify(&marked);
        let mut divs = DivFences::default();
        for (i, _, quoted, fence) in candidates {
            let line = lines.line(i);
            if !line.kind.is_markdown() {
                continue; // code, raw HTML or front matter: literal content, silently
            }
            if line.depth == 0 && !quoted {
                divs.markers.push((i, fence));
            } else if line.depth > 0 && matches!(fence, Fence::Open(_)) {
                divs.in_container.push(i);
            }
        }
        divs
    }

    /// Whether 0-based line `i` is a marker.
    fn is_marker(&self, i: usize) -> bool {
        self.markers.binary_search_by_key(&i, |(at, _)| *at).is_ok()
    }

    /// The 1-based buffer lines of a `:::` div opened inside a list item or block quote,
    /// which renders as text there.
    pub(crate) fn in_container(&self) -> impl Iterator<Item = BufLine> + '_ {
        self.in_container.iter().map(|&i| BufLine::new(i + 1))
    }

    /// `src` with every marker line emptied, line-preserving.
    fn blank(&self, src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        for (i, line) in src.lines().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            if !self.is_marker(i) {
                out.push_str(line);
            }
        }
        if src.ends_with('\n') {
            out.push('\n');
        }
        out
    }
}

/// A `:::` marker line, indented at most three spaces like every other block start, as
/// (the length of what precedes the colons, the marker). Block-quote `>` markers may precede
/// it: such a marker is never a div, but it is worth a warning.
fn div_marker(line: &str) -> Option<(usize, Fence)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let body = trimmed.trim_start_matches(['>', ' ', '\t']);
    let body = if body.len() < trimmed.len() && trimmed.starts_with('>') {
        body
    } else {
        trimmed
    };
    parse_fence(body).map(|f| (line.len() - body.len(), f))
}

/// A fenced-div span in buffer-line space (1-based, inclusive of the markers).
pub(crate) struct DivSpan {
    open: BufLine,
    close: BufLine,
    /// Raw attribute string from the opening fence (e.g. `.callout-note title="X"`).
    attrs: String,
}

/// Pair the markers into fenced-div spans (stack-based, so nesting is handled). Sorted so
/// that for a shared opening line the outermost (latest close) comes first.
/// Also returns the 1-based lines of every marker that pairs with nothing: a `:::` open
/// that was never closed, and a close with no div open. The orchestrator warns on both
/// (an unterminated fence otherwise drops its wrapper silently and the content renders
/// unfenced; a stray close usually means one above it closed the wrong div).
pub(crate) fn scan_div_spans(divs: &DivFences) -> (Vec<DivSpan>, Vec<BufLine>, Vec<BufLine>) {
    let mut stack: Vec<(BufLine, String)> = Vec::new();
    let mut spans: Vec<DivSpan> = Vec::new();
    let mut stray: Vec<BufLine> = Vec::new();
    for (i, fence) in &divs.markers {
        match fence {
            Fence::Open(attrs) => stack.push((BufLine::new(i + 1), attrs.clone())),
            Fence::Close => match stack.pop() {
                Some((open, attrs)) => spans.push(DivSpan {
                    open,
                    close: BufLine::new(i + 1),
                    attrs,
                }),
                None => stray.push(BufLine::new(i + 1)),
            },
        }
    }
    spans.sort_by_key(|s| (s.open, std::cmp::Reverse(s.close)));
    let mut unclosed: Vec<BufLine> = stack.into_iter().map(|(open, _)| open).collect();
    unclosed.sort_unstable();
    (spans, unclosed, stray)
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
    // `.column-page` escape, …) is silently dropped below (the "skip degenerate spans"
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
        // Walk every span that starts before this block, opening the ones that contain it and
        // stepping over the ones that already closed (degenerate/empty divs, spans whose
        // blocks are all consumed). Skipping a stale span must not HALT the walk: an empty
        // div is not a containing span, and stopping on it silently drops the block out of
        // its own div (a `.column` after an empty `.input`, say).
        //
        // This is one pass rather than a skip pass followed by an open pass, because a stale
        // span can sit anywhere in the order, not only at the front. Spans are sorted by
        // OPEN, so `::: {.outer}` wrapping an empty `::: {.foo}` and then a `.callout-note`
        // gives [outer, foo, callout]: a leading skip pass stops at `outer` (still open), and
        // the open pass then halts on `foo` and never reaches `callout`. The callout's
        // wrapper, title bar and class vanished from the published page with no diagnostic —
        // it has content, so the empty-div check above skips it, and `.foo` is a custom class,
        // so its own check is silent.
        //
        // A still-open ancestor already sits below `span_idx` on the stack, so it is never
        // revisited here, and open-sorted order means an ancestor is always pushed before its
        // descendant.
        while span_idx < spans.len() && spans[span_idx].open < fb.buf_start {
            let span = &spans[span_idx];
            span_idx += 1;
            if span.close > fb.buf_start {
                stack.push(Open {
                    span,
                    inner: Vec::new(),
                });
            }
        }

        push_block(&mut stack, &mut result, fb.block.clone());

        // Close spans that end before the next block begins (innermost first).
        let next_start = flat
            .get(i + 1)
            .map(|n| n.buf_start)
            .unwrap_or(BufLine::new(usize::MAX));
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
    // One range, one file: a `:::` opened in a partial and closed in the parent would
    // otherwise mix the two files' numbering into one `L:C-L:C` (see `map_span`).
    let (file, open_line, close_line) = map_span(origins, span.open, span.close);
    let sourcepos = format!("{open_line}:1-{close_line}:3");
    let file_attr = source_file_attr(file.as_deref());
    let data = format!(" data-block-id=\"{id}\" data-sourcepos=\"{sourcepos}\"{file_attr}");
    let concat = |inner: &[Block]| -> String { inner.iter().map(|b| b.html.as_str()).collect() };

    // A fenced-div's own composite block never carries a `Cell`: its children are
    // folded into one `html` string below, and by construction that folding is the
    // only place their per-block identity survives. A folded cell would therefore have
    // rendered and never run, because `Executor::run_through` (crates/server/src/exec.rs)
    // only scans TOP-LEVEL blocks for a `Cell`. So every folded cell — a `{python}` cell
    // in a `.callout-note`, a `.column-page` escape or a `layout-ncol` grid — is collected onto
    // `Block::nested` here, each with an empty output slot left after it in the folded
    // HTML, so the executor can run them in document order and put each output back where
    // its cell sits. There is no exception to this rule (`.debug` was the one, and it is
    // gone).
    //
    // `group_divs` closes containers innermost-first, so a nested container has already
    // done this to its own children: taking its `nested` list wholesale flattens every
    // cell in the document to one level while each slot stays where it belongs inside the
    // folded HTML.
    //
    // Only a cell in a language the *kernel* runs earns a slot. A `{js}` cell mounts its
    // own live target client-side and never produces a server-side output block, so a slot
    // after one would be an element that can never fill — which is exactly what the
    // `explorable/` snapshots caught. `executes_to_kernel` is the canonical set
    // (drift-locked to `exec::kernel_lang` by a test), so this asks it rather than
    // re-listing the languages.
    let mut nested: Vec<Block> = Vec::new();
    for b in inner.iter_mut() {
        nested.append(&mut b.nested);
        let runs = b.cell.as_ref().is_some_and(|c| executes_to_kernel(&c.lang));
        if runs {
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
        // Branches (1) and (2) are the AUTHOR's own words; only (3) is the tool speaking.
        // Only (3) is set at the label size; an authored title is the body size, because it
        // stands in for the heading the author often literally wrote. Marked with a class
        // rather than left to a selector, so the distinction is structural and a stylesheet
        // edit cannot lose it.
        let mut generated_kind_label = false;
        let title = match attrs.get("title") {
            Some(t) => html_escape(t),
            None if inner
                .first()
                .is_some_and(|b| block_heading_level(&b.html).is_some()) =>
            {
                let heading = inner.remove(0).html;
                if let Some(hid) =
                    extract_attr(&heading, "id").filter(|id| crate::cite::is_xref_anchor(id))
                {
                    title_id_attr = format!(" id=\"{}\"", escape_attr(&hid));
                }
                strip_tags(&heading)
            }
            None => {
                generated_kind_label = true;
                capitalize(kind)
            }
        };
        let title_class = if generated_kind_label {
            "callout-title callout-kind"
        } else {
            "callout-title"
        };
        let body = concat(&inner);
        // `collapse="true"` makes the callout a native <details> (starts closed);
        // `collapse="false"` is collapsible but starts open.
        match attrs.get("collapse") {
            Some(v) => {
                let open = if v == "false" { " open" } else { "" };
                format!(
                    "<div class=\"callout callout-{kind} callout-collapse\"{data}><details{open}><summary class=\"{title_class}\"{title_id_attr}>{title}</summary><div class=\"callout-body\">{body}</div></details></div>"
                )
            }
            None => format!(
                "<div class=\"callout callout-{kind}\"{data}><div class=\"{title_class}\"{title_id_attr}>{title}</div><div class=\"callout-body\">{body}</div></div>"
            ),
        }
    } else if let Some(ncol) = attrs.get("layout-ncol").and_then(|n| n.parse::<u32>().ok()) {
        let body = concat(&inner);
        format!(
            "<div class=\"tali-layout\" style=\"display:grid;grid-template-columns:repeat({ncol},minmax(0,1fr));gap:1rem\"{data}>{body}</div>"
        )
    } else {
        // Generic div: any class the author wants (styled by their own CSS). A class that is a
        // near-miss of a known feature class is almost certainly a typo that silently
        // degraded (a `.column-margn` that never moved into the margin) — warn with a
        // did-you-mean (located, click-to-source). A RETIRED class gets its removal note
        // instead, which is what every widget and theorem fence hits now. Genuine custom
        // classes stay silent.
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
        // A container never carries a `Cell` of its own: every folded cell that runs is
        // collected onto `nested` above, with an output slot left where it sits.
        cell: None,
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
