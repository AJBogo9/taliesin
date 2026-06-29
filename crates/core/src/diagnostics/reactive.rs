//! Static validation of the `{js}` reactive graph (dangling inputs + dependency cycles).

use super::helpers::{collect_attr_values, start_line};
use crate::render::{Block, Warning};

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

/// Static mirror of `qmd-js.js`'s `buildGraph`: flag (a) a `//| input: x` referencing a
/// name that no cell/`{{< input >}}` *defines*, and (b) a dependency cycle among `{js}`
/// cells (Kahn's topo-sort over `define -> consumer` edges; any cell left undrained is in
/// a cycle). Read-only — never touches the reactive runtime.
///
/// Conservative, matching `validate_internal_anchors`: a Python `ojs_define` publishes
/// names at *runtime* via a blob a static pass can't enumerate, so when the doc has any
/// non-`{js}` executable cell the *dangling-input* half is suppressed (a name could be
/// defined at runtime). The *cycle* half is a structural fact among `{js}` cells, so it
/// always runs.
pub fn validate_js_reactive_graph(blocks: &[Block]) -> Vec<Warning> {
    let nodes: Vec<JsNode> = blocks
        .iter()
        .filter_map(|b| {
            let cell = b.cell.as_ref()?;
            if cell.lang != "js" {
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
        .collect();
    if nodes.is_empty() {
        return Vec::new();
    }

    // Every statically-known define name: js-cell names/viewofs plus declarative
    // `{{< input name="k" >}}` controls (which emit `data-qmd-input="k"`).
    let mut defined: std::collections::HashSet<String> = std::collections::HashSet::new();
    for n in &nodes {
        for d in &n.defines {
            defined.insert(d.clone());
        }
    }
    for b in blocks {
        let mut vals = std::collections::HashSet::new();
        collect_attr_values(&b.html, "data-qmd-input=\"", &mut vals);
        for v in vals {
            defined.insert(v.to_string());
        }
    }

    let mut out = Vec::new();

    // (a) Dangling inputs — suppressed if a non-js executable cell could define names at
    // runtime (Python/R `ojs_define`).
    let runtime_defines = blocks
        .iter()
        .any(|b| b.cell.as_ref().is_some_and(|c| c.lang != "js"));
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
                let w = Warning::new(msg);
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
        ));
        out.push(match n.line {
            Some(l) => w.at(n.file.clone(), l),
            None => w,
        });
    }

    out
}

/// `frontmatter::closest` over an owned candidate list (the reactive-graph define names
/// are dynamic, so they can't be the `&'static` slice that helper wants). Same edit-
/// distance-≤2 "did you mean" rule.
fn closest_owned(key: &str, candidates: &[String]) -> Option<String> {
    candidates
        .iter()
        .map(|k| (crate::frontmatter::levenshtein(key, k), k))
        .filter(|&(d, _)| d > 0 && d <= 2)
        .min_by_key(|&(d, _)| d)
        .map(|(_, k)| k.clone())
}
