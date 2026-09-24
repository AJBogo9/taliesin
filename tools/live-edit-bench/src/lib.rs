//! Measure taliesin's live-edit moat through the real core seam
//! (`render_document_with_includes` -> `diff_blocks`): cold render, a warm
//! edit-above render+diff, the emitted `BlockOp` payload vs the full page HTML, and
//! DOM preservation at the diff level (a `<details>` / cell block below the edit gets
//! a `SetMeta`, not an `Update`). Pure measurement: it edits an in-memory copy of the
//! source, never the file, and reads only block id / sourcepos / html.

use std::path::Path;
use std::time::Instant;
use taliesin_core::{BlockOp, diff_blocks, render_document_with_includes};

pub mod e2e;

/// One live edit's measurements. Times are nanoseconds and machine-dependent (the
/// regression gate asserts the deterministic structural fields, not the times).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LiveEditMetrics {
    pub doc: String,
    pub cold_render_ns: u128,
    pub warm_edit_ns: u128, // render(edited) + diff
    pub diff_ns: u128,
    pub op_count: usize,
    pub set_meta_count: usize, // DOM-preserving ops
    pub update_count: usize,   // DOM-replacing ops
    pub insert_count: usize,
    pub remove_count: usize,
    pub full_html_bytes: usize, // body_html().len(): what a full reload re-sends
    pub edit_payload_bytes: usize, // the BlockOp wire payload for the edit
    pub dom_preserved: bool,    // a <details> block below the edit kept its node
}

/// The wire-payload size of one op: a faithful proxy of the server's JSON message
/// (the variable-length html / ids / sourcepos plus a small fixed envelope).
fn op_payload_bytes(op: &BlockOp) -> usize {
    const ENVELOPE: usize = 32; // {"type":"...","target_id":"..."} scaffolding
    match op {
        BlockOp::Update { target_id, html } => ENVELOPE + target_id.len() + html.len(),
        BlockOp::Insert { after_id, html } => {
            ENVELOPE + after_id.as_deref().map_or(0, str::len) + html.len()
        }
        BlockOp::Remove { target_id } => ENVELOPE + target_id.len(),
        BlockOp::SetMeta {
            target_id,
            sourcepos,
            source_file,
            inner,
        } => {
            ENVELOPE
                + target_id.len()
                + sourcepos.len()
                + source_file.as_deref().map_or(0, str::len)
                + inner.iter().map(String::len).sum::<usize>()
        }
    }
}

/// Render `src` cold, apply `edit` (a deterministic source transform), render the
/// edited source, and diff. `edit` should change text ABOVE the cells/collapsible so
/// the blocks below shift their line numbers (yielding `SetMeta`s).
pub fn measure_live_edit(
    doc_label: &str,
    src: &str,
    base: &Path,
    edit: impl Fn(&str) -> String,
) -> LiveEditMetrics {
    let t = Instant::now();
    let cold = render_document_with_includes(src, base);
    let cold_render_ns = t.elapsed().as_nanos();
    let full_html_bytes = cold.body_html().len();

    let edited = edit(src);
    let t = Instant::now();
    let new_doc = render_document_with_includes(&edited, base);
    let render_ns = t.elapsed().as_nanos();

    let t = Instant::now();
    let ops = diff_blocks(&cold.blocks, &new_doc.blocks);
    let diff_ns = t.elapsed().as_nanos();

    let (mut set_meta_count, mut update_count, mut insert_count, mut remove_count) = (0, 0, 0, 0);
    let mut edit_payload_bytes = 0;
    let mut set_meta_ids = std::collections::HashSet::new();
    for op in &ops {
        edit_payload_bytes += op_payload_bytes(op);
        match op {
            BlockOp::SetMeta { target_id, .. } => {
                set_meta_count += 1;
                set_meta_ids.insert(target_id.clone());
            }
            BlockOp::Update { .. } => update_count += 1,
            BlockOp::Insert { .. } => insert_count += 1,
            BlockOp::Remove { .. } => remove_count += 1,
        }
    }
    // DOM preserved: a `<details>` block (a collapse callout, the stateful element the
    // moat is about) below the edit kept its identity, so it got a `SetMeta` rather
    // than being re-rendered. False when the doc has no such element.
    let dom_preserved = new_doc
        .blocks
        .iter()
        .any(|b| b.html.contains("<details") && set_meta_ids.contains(&b.id));

    LiveEditMetrics {
        doc: doc_label.to_string(),
        cold_render_ns,
        warm_edit_ns: render_ns + diff_ns,
        diff_ns,
        op_count: ops.len(),
        set_meta_count,
        update_count,
        insert_count,
        remove_count,
        full_html_bytes,
        edit_payload_bytes,
        dom_preserved,
    }
}

/// What one save costs a whole PROJECT, which is a different question from
/// [`LiveEditMetrics`] and the reason both exist.
///
/// `measure_live_edit` measures the seam a single document goes through. Inside a site
/// preview a save also runs whole-project passes, so its cost tracks the size of the
/// *project*, not the size of the edit. Each field is the median of `runs`, in ns, of the
/// pass one kind of save runs before the edited page is rebuilt.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectPassMetrics {
    pub project: String,
    pub pages: usize,
    pub runs: usize,
    /// `Site::refresh_xrefs`: what every save of a page runs.
    pub refresh_xrefs_ns: u128,
    /// `Site::rebuild_search_index`: what a save that moves an anchor runs on top.
    pub search_index_ns: u128,
    /// `Site::discover` (warm): what a save that changes the page set or a page's front
    /// matter runs instead, and what the preview runs at startup.
    pub discover_ns: u128,
    /// `Site::discover_registry`: what the language server runs on every save.
    pub registry_ns: u128,
}

/// Time the passes a save runs on a real project, `runs` times each, and keep the medians.
/// Read-only: it discovers and renders in memory and writes nothing.
pub fn measure_project_passes(label: &str, root: &Path, runs: usize) -> Option<ProjectPassMetrics> {
    if !root.join("_site.yml").is_file() {
        return None;
    }
    let mut site = taliesin_core::Site::discover(root);
    let time = |f: &mut dyn FnMut()| -> u128 {
        let mut ns: Vec<u128> = (0..runs)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_nanos()
            })
            .collect();
        ns.sort_unstable();
        ns[ns.len() / 2]
    };
    Some(ProjectPassMetrics {
        project: label.to_string(),
        pages: site.pages.len(),
        runs,
        refresh_xrefs_ns: time(&mut || site.refresh_xrefs()),
        search_index_ns: time(&mut || site.rebuild_search_index()),
        discover_ns: time(&mut || drop(taliesin_core::Site::discover(root))),
        registry_ns: time(&mut || drop(taliesin_core::Site::discover_registry(root))),
    })
}

/// The project-scale table: one row per measured project.
pub fn project_markdown_report(ms: &[ProjectPassMetrics]) -> String {
    let ms_ = |ns: u128| format!("{:.1} ms", ns as f64 / 1e6);
    let mut s = String::from(
        "## project-scale save: the whole-project passes\n\n\
         Median of the runs, in-process, release build. A save of a page runs\n\
         `refresh_xrefs`; one that moves an anchor also rebuilds the search index; one that\n\
         changes the page set or a page's front matter runs `discover` instead. The language\n\
         server runs `discover_registry` on every save.\n\n\
         | project | pages | save: refresh_xrefs | anchor moved: + search index | front matter: discover | language server: registry |\n\
         |---|---|---|---|---|---|\n",
    );
    for m in ms {
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} |\n",
            m.project,
            m.pages,
            ms_(m.refresh_xrefs_ns),
            ms_(m.search_index_ns),
            ms_(m.discover_ns),
            ms_(m.registry_ns),
        ));
    }
    s
}

/// The end-to-end table: save to the first websocket message, and to `publishDiagnostics`.
pub fn e2e_markdown_report(ms: &[e2e::SaveToOp]) -> String {
    let ms_ =
        |ns: Option<u128>| ns.map_or("n/a".to_string(), |ns| format!("{:.0} ms", ns as f64 / 1e6));
    let mut s = String::from(
        "## end to end: save to the first message\n\n\
         Median of the rounds, against the release binary. From the file write to the first\n\
         websocket message the preview sends, so the watcher's debounce, rediscovery and the\n\
         edited page's own build are all inside; the language server column is the save to\n\
         `publishDiagnostics`, its 120 ms coalescing window inside.\n\n\
         | project | pages | body | heading (moves anchors) | title | atomic save | preview RSS after | language server |\n\
         |---|---|---|---|---|---|---|---|\n",
    );
    for m in ms {
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
            m.project,
            m.pages,
            ms_(m.body_ns),
            ms_(m.heading_ns),
            ms_(m.title_ns),
            ms_(m.atomic_ns),
            m.preview_rss_kib
                .map_or("n/a".to_string(), |k| format!("{} MB", k / 1024)),
            ms_(m.lsp_save_ns),
        ));
    }
    s
}

/// A deterministic word source for [`write_synthetic_book`] (xorshift), so two runs time
/// the same text.
struct Words(u64);

impl Words {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }

    /// A paragraph of `n` words ending in an inline math expression.
    fn para(&mut self, n: usize) -> String {
        const WORDS: &[&str] = &[
            "estimate",
            "model",
            "prior",
            "posterior",
            "sample",
            "variance",
            "kernel",
            "gradient",
            "likelihood",
            "entropy",
            "matrix",
            "vector",
            "projection",
            "signal",
            "filter",
            "sequence",
            "bound",
            "proof",
            "lemma",
            "algorithm",
            "graph",
            "search",
            "optimal",
            "measure",
            "density",
            "expectation",
            "the",
            "a",
            "of",
            "and",
            "to",
            "in",
        ];
        let mut p = String::from("Consider");
        for _ in 0..n {
            p.push(' ');
            p.push_str(WORDS[self.below(WORDS.len())]);
        }
        let (i, j) = (self.below(9) + 1, self.below(4) + 2);
        p + &format!(" with *emphasis* and inline math $x_{{{i}}}^{j} + \\alpha_{{{i}}}$.")
    }
}

/// Write a synthetic book of `pages` pages under `root`: an unnumbered preface and
/// `pages - 1` chapters in parts of ten, each 2 to 10 KB of prose with inline and display
/// math, a python and a rust block, a figure, a table, cross-chapter `@sec-`/`@fig-`/`@eq-`
/// and `@tbl-` references, and in every seventh chapter a shared partial. The shape of the
/// books the 2026-09-24 audit measured at scale.
pub fn write_synthetic_book(root: &Path, pages: usize) -> std::io::Result<()> {
    use std::fmt::Write as _;
    let mut w = Words(0x2545_f491_4f6c_dd1d ^ pages as u64);
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root.join("chapters"))?;
    std::fs::create_dir_all(root.join("_includes"))?;
    std::fs::create_dir_all(root.join("figures"))?;
    std::fs::write(
        root.join("figures/fig.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"50\"></svg>\n",
    )?;
    for k in 0..5 {
        let body = format!(
            "Shared partial {k}. {}\n\n$$ \\int_0^{{{}}} x^{{{}}} \\, dx $$\n",
            w.para(60),
            k + 1,
            k + 2
        );
        std::fs::write(root.join(format!("_includes/part{k}.tmd")), body)?;
    }
    let chapters = pages.saturating_sub(1);
    let mut site = String::from("title: \"Synthetic book\"\nchapters:\n  - index.tmd\n");
    for first in (1..=chapters).step_by(10) {
        let _ = writeln!(site, "  - part: \"Part {}\"\n    chapters:", first / 10 + 1);
        for i in first..(first + 10).min(chapters + 1) {
            let _ = writeln!(site, "      - chapters/ch{i:03}.tmd");
        }
    }
    std::fs::write(root.join("_site.yml"), site)?;
    let preface = format!(
        "# Preface {{.unnumbered}}\n\n{}\n\nSee @sec-ch001.\n",
        w.para(80)
    );
    std::fs::write(root.join("index.tmd"), preface)?;
    for i in 1..=chapters {
        let target = 2000 + w.below(8000);
        let [a, b, c, d] = [0; 4].map(|_| w.below(chapters) + 1);
        let mut t = format!(
            "---\ntitle: \"Chapter {i}\"\ndescription: \"Synthetic chapter {i}.\"\n---\n\n\
             # Topic {i} {{#sec-ch{i:03}}}\n\n{} See @sec-ch{a:03} and @fig-ch{b:03}.\n\n\
             ## Background {{#sec-ch{i:03}-bg}}\n\n{} Recall @eq-ch{c:03} and @tbl-ch{d:03}.\n\n\
             $$ \\sum_{{k=1}}^{{{i}}} a_k x^k = f_{{{i}}}(x) $$ {{#eq-ch{i:03}}}\n\n\
             ```python\ndef step_{i}(x, lr=0.{i}):\n    return x - lr * grad(x)\n```\n\n\
             ![Figure for chapter {i}.](../figures/fig.svg){{#fig-ch{i:03}}}\n\n\
             | k | value |\n|---|---|\n| {i} | {} |\n\n: Table of chapter {i}. {{#tbl-ch{i:03}}}\n\n",
            w.para(70),
            w.para(90),
            i * i
        );
        if i % 7 == 0 {
            let _ = writeln!(t, "{{{{< include ../_includes/part{}.tmd >}}}}\n", i % 5);
        }
        let mut s = 2;
        while t.len() < target {
            let _ = writeln!(t, "## Section {s} {{#sec-ch{i:03}-s{s}}}\n");
            for _ in 0..2 {
                let n = 40 + w.below(80);
                let _ = writeln!(t, "{}\n", w.para(n));
            }
            if s % 3 == 0 {
                let _ = writeln!(
                    t,
                    "$$ \\mathbb{{E}}[X_{{{s}}}] = \\int x\\, p_{{{i}}}(x)\\, dx $$\n"
                );
            }
            if s % 4 == 0 {
                let _ = writeln!(
                    t,
                    "```rust\nfn f_{i}_{s}(v: &[f64]) -> f64 {{ v.iter().sum() }}\n```\n"
                );
            }
            s += 1;
        }
        std::fs::write(root.join(format!("chapters/ch{i:03}.tmd")), t)?;
    }
    Ok(())
}

/// A human-readable markdown table for one measurement (printed by the binary and
/// snapshotted into `RESULTS.md`). Times shown in microseconds for readability.
pub fn markdown_report(m: &LiveEditMetrics) -> String {
    let us = |ns: u128| ns as f64 / 1000.0;
    let ratio = if m.edit_payload_bytes == 0 {
        0.0
    } else {
        m.full_html_bytes as f64 / m.edit_payload_bytes as f64
    };
    format!(
        "## live-edit benchmark: `{doc}`\n\n\
         | metric | value |\n\
         |---|---|\n\
         | cold full render | {cold:.1} us |\n\
         | warm edit (render + diff) | {warm:.1} us |\n\
         | diff only | {diff:.1} us |\n\
         | ops emitted | {ops} (insert {ins}, set_meta {sm}, update {up}, remove {rm}) |\n\
         | full page HTML | {html} bytes |\n\
         | warm-edit payload | {payload} bytes |\n\
         | payload shrink vs full reload | {ratio:.0}x smaller |\n\
         | open `<details>` survives as same DOM node | {dom} |\n",
        doc = m.doc,
        cold = us(m.cold_render_ns),
        warm = us(m.warm_edit_ns),
        diff = us(m.diff_ns),
        ops = m.op_count,
        ins = m.insert_count,
        sm = m.set_meta_count,
        up = m.update_count,
        rm = m.remove_count,
        html = m.full_html_bytes,
        payload = m.edit_payload_bytes,
        ratio = ratio,
        dom = if m.dom_preserved { "yes" } else { "no" },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_payload_bytes_sums_html_plus_envelope() {
        let update = BlockOp::Update {
            target_id: "b-1".into(),
            html: "<p>hi</p>".into(),
        };
        assert_eq!(op_payload_bytes(&update), 32 + 3 + 9);
        let set_meta = BlockOp::SetMeta {
            target_id: "b-2".into(),
            sourcepos: "5:1-7:3".into(),
            source_file: None,
            inner: Vec::new(),
        };
        assert_eq!(op_payload_bytes(&set_meta), 32 + 3 + 7);
    }

    #[test]
    fn markdown_report_renders_the_headline_rows() {
        let m = LiveEditMetrics {
            doc: "x".into(),
            cold_render_ns: 1000,
            warm_edit_ns: 1200,
            diff_ns: 50,
            op_count: 3,
            set_meta_count: 2,
            update_count: 0,
            insert_count: 1,
            remove_count: 0,
            full_html_bytes: 10000,
            edit_payload_bytes: 100,
            dom_preserved: true,
        };
        let md = markdown_report(&m);
        assert!(md.contains("100x smaller"), "ratio row: {md}");
        assert!(
            md.contains("survives as same DOM node | yes"),
            "dom row: {md}"
        );
    }
}
