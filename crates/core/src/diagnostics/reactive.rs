//! Static validation of the `{js}` reactive graph (dangling inputs + dependency cycles).

use super::helpers::{collect_attr_values, start_line};
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

/// Static mirror of `tali-js.js`'s `buildGraph`: flag (a) a `//| input: x` referencing a
/// name that no cell/`{{< input >}}` *defines*, and (b) a dependency cycle among `{js}`
/// cells (Kahn's topo-sort over `define -> consumer` edges; any cell left undrained is in
/// a cycle). Read-only — never touches the reactive runtime.
///
/// Conservative, matching `validate_internal_anchors`: a Python `define(...)` publishes
/// names at *runtime* via a blob a static pass can't enumerate, so a cell that calls it
/// suppresses the *dangling-input* half. The *cycle* half is a structural fact among
/// `{js}` cells, so it always runs.
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
                crate::render::attr_value(&tag, "data-source-file").map(str::to_string),
                start_line(pos),
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
            // Every client-side language shares the `//| name`/`viewof`/`input` wiring, so
            // a second registered language taking a `//| input:` is a node in the same graph
            // and gets the same dangling-input / cycle diagnostics.
            crate::render::client_lang(&cell.lang)?;
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

    let mut out = Vec::new();

    // (a) Dangling inputs — suppressed only where a name really could appear at runtime.
    //
    // The predicate is "a KERNEL cell that CALLS `define(`", narrowed from "any kernel
    // cell" on 2026-08-03. Two earlier spellings were each wrong in their own direction.
    // `lang != "js"` suppressed the check on any page carrying a second CLIENT language,
    // which publishes nothing at runtime. Then "any kernel cell" suppressed it on every page
    // with a `{python}` cell at all — which is every real blog post in the corpus, so the
    // check was off exactly where documents are longest and a typo'd input most likely.
    // Reading the cell's own literal is what distinguishes "this document uses the bridge"
    // from "this document runs Python", and only the first can define a name invisibly.
    //
    // Asked through `Block::cells()`, not `b.cell`: a bridge cell inside a `.callout-note`
    // or a `layout-ncol` grid executes and publishes its name at runtime exactly like a
    // top-level one, but reading `cell` alone forgets every cell a container folded away
    // (`Block::cells`: "reading `self.cell` directly instead is the bug"), leaving the
    // dangling-input check armed and the page drawing a false error. A KERNEL cell is
    // always recorded in `nested`, since it is the class of cell that earns an output slot.
    let runtime_defines = blocks.iter().any(|b| {
        b.cells()
            .any(|c| crate::render::client_lang(&c.lang).is_none() && c.code.contains("define("))
    });
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
                        "unknown reactive input `{inp}`: no `{{js}}` cell or `{{{{< input >}}}}` defines it (did you mean `{s}`?)"
                    ),
                    None => format!(
                        "unknown reactive input `{inp}`: no `{{js}}` cell or `{{{{< input >}}}}` defines it"
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
