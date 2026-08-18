//! Measure taliesin's live-edit moat through the real core seam
//! (`render_document_with_includes` -> `diff_blocks`): cold render, a warm
//! edit-above render+diff, the emitted `BlockOp` payload vs the full page HTML, and
//! DOM preservation at the diff level (a `<details>` / cell block below the edit gets
//! a `SetMeta`, not an `Update`). Pure measurement: it edits an in-memory copy of the
//! source, never the file, and reads only block id / sourcepos / html.

use std::path::Path;
use std::time::Instant;
use taliesin_core::{BlockOp, diff_blocks, render_document_with_includes};

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
        } => {
            ENVELOPE
                + target_id.len()
                + sourcepos.len()
                + source_file.as_deref().map_or(0, str::len)
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
/// preview a save also pays `Site::refresh_xrefs`, whose harvest renders EVERY page to full
/// HTML and keeps only the cross-page float numbers — so the per-save cost tracks the size
/// of the *project*, not the size of the edit. Without this second measurement the headline
/// warm-edit figure reads as if it described a book-sized save, and it does not.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectSaveMetrics {
    pub project: String,
    pub pages: usize,
    /// Best-of-three `refresh_xrefs`, i.e. the whole-project pass every non-structural
    /// save runs before the edited page is re-rendered.
    pub xref_refresh_ns: u128,
    pub xref_refresh_ns_per_page: u128,
}

/// Time `Site::refresh_xrefs` on a real project — the O(pages) pass a site-preview save
/// runs. Best of three, so first-touch file-cache noise is out. Read-only: it discovers and
/// renders in memory and writes nothing.
pub fn measure_project_save(label: &str, root: &Path) -> Option<ProjectSaveMetrics> {
    if !root.join("_site.yml").is_file() {
        return None;
    }
    let mut site = taliesin_core::Site::discover(root);
    let pages = site.pages.len();
    let mut best = u128::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        site.refresh_xrefs();
        best = best.min(t.elapsed().as_nanos());
    }
    Some(ProjectSaveMetrics {
        project: label.to_string(),
        pages,
        xref_refresh_ns: best,
        xref_refresh_ns_per_page: best / pages.max(1) as u128,
    })
}

/// The project-scale table: one row per measured project, plus the per-page rate that makes
/// the shape (linear in pages) readable rather than implied.
pub fn project_markdown_report(ms: &[ProjectSaveMetrics]) -> String {
    let mut s = String::from(
        "## project-scale save: `Site::refresh_xrefs`\n\n\
         Every non-structural save in a **site** preview runs this before the edited page is\n\
         re-rendered, and its harvest renders every page. So this cost tracks the size of the\n\
         project, not the size of the edit — the warm-edit rows above are one document.\n\n\
         | project | pages | refresh_xrefs | per page |\n|---|---|---|---|\n",
    );
    for m in ms {
        s.push_str(&format!(
            "| `{}` | {} | {:.1} ms | {:.2} ms |\n",
            m.project,
            m.pages,
            m.xref_refresh_ns as f64 / 1e6,
            m.xref_refresh_ns_per_page as f64 / 1e6,
        ));
    }
    s
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
