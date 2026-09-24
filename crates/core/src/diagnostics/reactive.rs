//! Static validation of the `{js}` reactive graph (dangling inputs + dependency cycles).

use super::helpers::collect_attr_values;
use crate::render::sourcepos_start_line as start_line;
use crate::render::{Block, Severity, Warning};

/// One `{js}` cell's reactive wiring, distilled from the block model for the static graph
/// check: the names it `defines` (its `//| name` and/or `//| viewof`), the names it
/// `inputs` (its `//| input:` list), and where it lives (for the located warning).
struct JsNode {
    defines: Vec<String>,
    inputs: Vec<String>,
    file: Option<String>,
    line: Option<u32>,
    /// A human label for cycle diagnostics: the first define name, else "(unnamed cell)".
    label: String,
}

/// The reactive wiring of the client cells a `:::` container folded away, read back off the
/// container's emitted HTML.
///
/// **The block model cannot answer this.** `Block::nested` records only the cells the KERNEL
/// runs, because those are the ones that need an output slot spliced back inside the
/// container (`divs.rs`); a `{js}` cell mounts its own target client-side, so it earns no
/// slot and is not recorded. Its `Block` — and with it its `Cell` — stops existing when the
/// container folds it, so `b.cell` and `Block::cells()` alike are blind to it. What survives
/// is the `<script data-name=… data-viewof=… data-inputs=…>` the cell emitted, which is the
/// very wiring the browser's own `buildGraph` reads, so reading it here is what makes the
/// static check agree with the runtime instead of contradicting it.
///
/// One node per SCRIPT tag, not one per container: the cycle half of this check is about
/// per-cell edges, and pooling two folded cells' names into one node would invent cycles
/// that are not there.
///
/// Attribute values arrive still entity-escaped, and are compared against raw `//| name:`
/// values without decoding. That is sound rather than lucky: `&`, `<`, `>` and `"` are the
/// only characters `escape_attr` touches and none of them can appear in a name
/// `tali-js.js` is able to bind, so the escape is a provable no-op for every name that
/// could work at runtime.
fn folded_client_nodes(container: &Block) -> Vec<JsNode> {
    let mut out = Vec::new();
    // The cell's own block attrs ride on the wrapper element the script sits inside, so the
    // most recent tag carrying a `data-sourcepos` is that cell's. Both halves are read off
    // ONE tag, which is what keeps the file and the line a matched pair (`render/CLAUDE.md`:
    // a `source_file` may only ever be paired with a mapped line).
    let mut here: (Option<String>, Option<u32>) = (None, None);
    for tag in crate::render::tags(&container.html) {
        if let Some(pos) = crate::render::attr_value(&tag, "data-sourcepos") {
            here = (
                crate::render::attr_value(&tag, "data-source-file")
                    .map(std::borrow::Cow::into_owned),
                start_line(&pos),
            );
        }
        if !tag.name.eq_ignore_ascii_case("script") {
            continue;
        }
        let (mut defines, mut inputs) = (Vec::new(), Vec::new());
        for a in crate::render::attrs(&tag) {
            match a.name {
                "data-name" | "data-viewof" => defines.push(a.value.to_string()),
                "data-inputs" => inputs.extend(
                    a.value
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                ),
                _ => {}
            }
        }
        if defines.is_empty() && inputs.is_empty() {
            continue; // not a client cell: a bundled library, the search index, …
        }
        let label = defines
            .first()
            .cloned()
            .unwrap_or_else(|| "(unnamed cell)".to_string());
        out.push(JsNode {
            defines,
            inputs,
            file: here.0.clone().or_else(|| container.source_file.clone()),
            line: here.1.or_else(|| start_line(&container.sourcepos)),
            label,
        });
    }
    out
}

/// Static mirror of `tali-js.js`'s `buildGraph`: flag (a) a `//| input: x` referencing a
/// name that no cell/`{{< input >}}`/Python `define(...)` *defines*, and (b) a dependency
/// cycle among `{js}` cells (Kahn's topo-sort over `define -> consumer` edges; any cell
/// left undrained is in a cycle). Read-only: it never touches the reactive runtime.
///
/// A Python `define(...)` publishes its names at *runtime*, but the kernel preamble
/// declares it `def define(**kwargs)`, so every name it can publish is a keyword spelled in
/// the call, and [`define_keywords`] reads them there. Only a call whose names a static
/// read cannot know suppresses the *dangling-input* half, page-wide, conservative like
/// `validate_internal_anchors`. The *cycle* half is a structural fact among `{js}` cells,
/// so it always runs.
pub fn validate_js_reactive_graph(blocks: &[Block]) -> Vec<Warning> {
    // A block that IS a cell contributes itself, with its own file and line. A block that is
    // not may be a container that folded client cells away, and those are recoverable only
    // from its HTML — so the two sources are disjoint and cannot double-count a cell.
    let folded: Vec<JsNode> = blocks
        .iter()
        .filter(|b| b.cell.is_none())
        .flat_map(folded_client_nodes)
        .collect();
    let nodes: Vec<JsNode> = blocks
        .iter()
        .filter_map(|b| {
            let cell = b.cell.as_ref()?;
            // A `{js}` cell's `//| name`/`viewof`/`input` wiring makes it a node of the graph.
            if !crate::render::is_client_lang(&cell.lang) {
                return None;
            }
            let mut defines = Vec::new();
            if let Some(n) = cell.js.name.as_deref() {
                defines.push(n.to_string());
            }
            if let Some(v) = cell.js.viewof.as_deref() {
                defines.push(v.to_string());
            }
            let label = defines
                .first()
                .cloned()
                .unwrap_or_else(|| "(unnamed cell)".to_string());
            Some(JsNode {
                defines,
                inputs: cell.js.inputs.clone(),
                file: b.source_file.clone(),
                line: start_line(&b.sourcepos),
                label,
            })
        })
        .chain(folded)
        .collect();
    if nodes.is_empty() {
        return Vec::new();
    }

    // Every statically-known define name: js-cell names/viewofs plus declarative
    // `{{< input name="k" >}}` controls (which emit `data-tali-input="k"`).
    let mut defined: std::collections::HashSet<String> = std::collections::HashSet::new();
    for n in &nodes {
        for d in &n.defines {
            defined.insert(d.clone());
        }
    }
    for b in blocks {
        let mut vals = std::collections::HashSet::new();
        collect_attr_values(&b.html, "data-tali-input", &mut vals);
        for v in vals {
            defined.insert(v.to_string());
        }
    }

    // (a) Dangling inputs, suppressed only where a name really could appear unseen.
    //
    // A KERNEL cell's `define(name=…)` keywords are defined names like any other. Three
    // earlier spellings each suppressed the whole check instead. `lang != "js"` did it on
    // any page carrying a second CLIENT language, which publishes nothing at runtime. "Any
    // kernel cell" did it on every page with a `{python}` cell at all. "A kernel cell whose
    // code contains `define(`" still did it on every real blog post that uses the bridge,
    // although every call there is keyword form and names exactly what it publishes.
    //
    // Only a cell that runs on a kernel publishes anything. A display fence (`{bash}`,
    // `{scheme}`, `{c}`) never runs, so its `define` neither defines a name nor switches the
    // check off. A define in an `include: false` cell still publishes its names: the
    // `<script type="tali-define">` blob is a side channel, not the cell's visible output,
    // so hiding the output does not drop it.
    //
    // Asked through `Block::cells()`, not `b.cell`: a bridge cell inside a `.callout-note`
    // or a `layout-ncol` grid executes and publishes its name at runtime exactly like a
    // top-level one, but reading `cell` alone forgets every cell a container folded away
    // (`Block::cells`: "reading `self.cell` directly instead is the bug"), leaving the
    // dangling-input check armed and the page drawing a false error. A KERNEL cell is
    // always recorded in `nested`, since it is the class of cell that earns an output slot.
    let mut runtime_defines = false;
    for c in blocks.iter().flat_map(Block::cells) {
        if !crate::render::executes_to_kernel(&c.lang) {
            continue;
        }
        match define_keywords(&c.code) {
            Some(names) => defined.extend(names),
            None => runtime_defines = true,
        }
    }

    let mut out = Vec::new();
    if !runtime_defines {
        let candidates: Vec<String> = defined.iter().cloned().collect();
        for n in &nodes {
            for inp in &n.inputs {
                if defined.contains(inp) {
                    continue;
                }
                let suggestion = closest_owned(inp, &candidates);
                let msg = match suggestion {
                    Some(s) => format!(
                        "unknown reactive input `{inp}`: no `{{js}}` cell, `{{{{< input >}}}}` or Python `define(...)` defines it (did you mean `{s}`?)"
                    ),
                    None => format!(
                        "unknown reactive input `{inp}`: no `{{js}}` cell, `{{{{< input >}}}}` or Python `define(...)` defines it"
                    ),
                };
                let w = Warning::new(msg).severity(Severity::Error);
                out.push(match n.line {
                    Some(l) => w.at(n.file.clone(), l),
                    None => w,
                });
            }
        }
    }

    // (b) Cycle detection — Kahn's topological sort over `define -> consumer` edges, the
    // same model `buildGraph` uses. Any node never drained is part of a cycle.
    // consumers[name] = indices of nodes listing `name` in their inputs.
    let mut consumers: std::collections::HashMap<&str, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        for inp in &n.inputs {
            consumers.entry(inp.as_str()).or_default().push(i);
        }
    }
    let mut indeg = vec![0usize; nodes.len()];
    for n in &nodes {
        for d in &n.defines {
            if let Some(cs) = consumers.get(d.as_str()) {
                for &c in cs {
                    indeg[c] += 1;
                }
            }
        }
    }
    let mut queue: std::collections::VecDeque<usize> =
        (0..nodes.len()).filter(|&i| indeg[i] == 0).collect();
    let mut drained = vec![false; nodes.len()];
    while let Some(i) = queue.pop_front() {
        drained[i] = true;
        for d in &nodes[i].defines {
            if let Some(cs) = consumers.get(d.as_str()) {
                for &c in cs {
                    indeg[c] -= 1;
                    if indeg[c] == 0 {
                        queue.push_back(c);
                    }
                }
            }
        }
    }
    for (i, n) in nodes.iter().enumerate() {
        if drained[i] {
            continue;
        }
        let w = Warning::new(format!(
            "reactive dependency cycle involving `{}`: `{{js}}` cells form a loop, so none can run",
            n.label
        ))
        .severity(Severity::Error);
        out.push(match n.line {
            Some(l) => w.at(n.file.clone(), l),
            None => w,
        });
    }

    out
}

/// `frontmatter::closest_of` over an owned candidate list (the reactive-graph define names
/// are dynamic, so they can't be the `&'static` slice `closest` wants). Delegated rather
/// than re-derived: this copy had drifted to a distance-only tie-break, and its candidates
/// come out of a `HashSet` — randomly seeded per process — so two equally close names made
/// the suggestion differ between runs of the same unchanged document.
fn closest_owned(key: &str, candidates: &[String]) -> Option<String> {
    crate::closest_of(key, candidates.iter().map(String::as_str)).map(str::to_string)
}

/// The names a kernel cell's `define(...)` calls publish, read off the calls themselves:
/// each `name=` keyword argument. `None` when the cell may publish a name this read cannot
/// see, which is the one case the caller still suppresses the dangling-input check for: a
/// `*`/`**` splat or a positional argument, a call with no balancing `)`, a `define` nested
/// in another call's arguments, a `define` not immediately called (an alias `d = define`, a
/// `\` continuation, a `(` on the next line), or the word `define` inside a string literal,
/// which an f-string field or an `exec` could still run.
///
/// A lexical read of Python, not a parse: strings and `#` comments are skipped, so a comma,
/// bracket or `define(` inside either is never taken for code, and brackets nest. `define`
/// must stand alone as an identifier, so `redefine(` and `defined(` are other functions, and
/// `obj.define(...)` is some object's method, not the bridge.
fn define_keywords(code: &str) -> Option<Vec<String>> {
    let b = code.as_bytes();
    let mut names = Vec::new();
    let mut i = 0;
    while let Some(at) = next_define(b, i, b.len())? {
        let mut open = at + "define".len();
        while b.get(open).is_some_and(|&c| c == b' ' || c == b'\t') {
            open += 1;
        }
        if b.get(open) != Some(&b'(') {
            return None; // not immediately a call
        }
        let (args, end) = call_args(code, open + 1)?;
        if next_define(b, open + 1, end)?.is_some() {
            return None; // a call nested in this one's arguments
        }
        for arg in args {
            let arg = skip_blank_and_comments(arg);
            if arg.is_empty() {
                continue; // `define()`, or a trailing comma
            }
            let (name, rest) =
                arg.split_at(arg.find(|c: char| !(c.is_alphanumeric() || c == '_'))?);
            let rest = rest.trim_start();
            if name.is_empty()
                || name.starts_with(|c: char| c.is_ascii_digit())
                || !rest.starts_with('=')
                || rest.starts_with("==")
            {
                return None; // a splat or a positional argument
            }
            names.push(name.to_string());
        }
        i = end;
    }
    Some(names)
}

/// Where the next bare `define` identifier in `b[i..end]` starts, skipping strings, `#`
/// comments and an attribute `obj.define`: `Some(None)` when there is none, `None` when the
/// word sits inside a string literal.
fn next_define(b: &[u8], mut i: usize, end: usize) -> Option<Option<usize>> {
    while i < end {
        if let Some(stop) = skip_string_or_comment(b, i) {
            if b[i] != b'#' && (i..stop).any(|j| is_define_at(b, j)) {
                return None;
            }
            i = stop;
            continue;
        }
        if is_define_at(b, i)
            && b[..i].iter().rev().find(|&&c| c != b' ' && c != b'\t') != Some(&b'.')
        {
            return Some(Some(i));
        }
        i += 1;
    }
    Some(None)
}

/// Whether `define` starts at `b[i]` as a whole identifier.
fn is_define_at(b: &[u8], i: usize) -> bool {
    let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80;
    b[i..].starts_with(b"define")
        && (i == 0 || !is_ident(b[i - 1]))
        && !b.get(i + "define".len()).is_some_and(|&c| is_ident(c))
}

/// The arguments of a call whose `(` ends just before `start`, split at top-level commas,
/// and the index just past its balancing `)`. `None` when nothing balances it.
fn call_args(code: &str, start: usize) -> Option<(Vec<&str>, usize)> {
    let b = code.as_bytes();
    let (mut depth, mut arg_start, mut i) = (0usize, start, start);
    let mut args = Vec::new();
    while i < b.len() {
        if let Some(end) = skip_string_or_comment(b, i) {
            i = end;
            continue;
        }
        match b[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth > 0 => depth -= 1,
            b')' => {
                args.push(&code[arg_start..i]);
                return Some((args, i + 1));
            }
            b']' | b'}' => return None,
            b',' if depth == 0 => {
                args.push(&code[arg_start..i]);
                arg_start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// If a Python string literal or `#` comment starts at `b[i]`, the index just past it
/// (a line comment stops before its newline; an unterminated literal runs to the end of
/// its line, or of the code for a triple-quoted one). A prefix such as `f`/`r`/`b` is an
/// ordinary identifier byte to the caller and needs no handling here.
fn skip_string_or_comment(b: &[u8], i: usize) -> Option<usize> {
    match b[i] {
        b'#' => Some(
            b[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |n| i + n),
        ),
        q @ (b'"' | b'\'') => {
            let triple = b[i..].starts_with(&[q, q, q]);
            let mut j = i + if triple { 3 } else { 1 };
            while j < b.len() {
                match b[j] {
                    b'\\' => j += 2,
                    c if c == q && (!triple || b[j..].starts_with(&[q, q, q])) => {
                        return Some(j + if triple { 3 } else { 1 });
                    }
                    b'\n' if !triple => return Some(j),
                    _ => j += 1,
                }
            }
            Some(b.len())
        }
        _ => None,
    }
}

/// `arg` with its leading whitespace and whole-line `#` comments removed.
fn skip_blank_and_comments(mut arg: &str) -> &str {
    arg = arg.trim_start();
    while let Some(rest) = arg.strip_prefix('#') {
        arg = rest.split_once('\n').map_or("", |(_, r)| r).trim_start();
    }
    arg
}
