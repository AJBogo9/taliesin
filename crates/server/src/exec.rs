//! Execution engine: runs a document's Python code cells against a warm kernel
//! and splices the outputs back into the block list as their own blocks.
//!
//! Granularity is simple notebook semantics: on each rebuild, find the earliest
//! cell whose source changed and re-run it plus everything after it (downstream
//! cells may depend on the changed kernel state); cells before it reuse their
//! cached output. Each output block's id is derived from its cell's id, so it
//! swaps in place when the cell (or an upstream cell) re-runs.

use std::path::PathBuf;

use qmd_fast_core::{Block, render::Cell};

use crate::kernel::{Kernel, render_outputs};

/// A code cell pulled out of the block list, with what the engine needs to run
/// it and to build its output block.
struct CellRef {
    block_index: usize,
    id: String,
    code: String,
    sourcepos: String,
    source_file: Option<String>,
}

struct Cached {
    id: String,
    output: String, // inner output HTML (may be empty)
}

pub struct Executor {
    python: PathBuf,
    kernel: Option<Kernel>,
    failed: bool,
    cached: Vec<Cached>,
}

impl Executor {
    pub fn new() -> Self {
        let python = std::env::var_os("QMD_FAST_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("python3"));
        Self { python, kernel: None, failed: false, cached: Vec::new() }
    }

    /// Execute the document's Python cells (changed cells + downstream) and
    /// return the block list with output blocks spliced in after each cell.
    pub async fn run(&mut self, blocks: Vec<Block>) -> Vec<Block> {
        let cells: Vec<CellRef> = blocks
            .iter()
            .enumerate()
            .filter_map(|(i, b)| match &b.cell {
                Some(Cell { lang, code }) if lang == "python" => Some(CellRef {
                    block_index: i,
                    id: b.id.clone(),
                    code: code.clone(),
                    sourcepos: b.sourcepos.clone(),
                    source_file: b.source_file.clone(),
                }),
                _ => None,
            })
            .collect();

        if cells.is_empty() {
            self.cached.clear();
            return blocks;
        }

        let outputs = self.compute_outputs(&cells).await;

        // Map cell block index -> its output block (when non-empty).
        let mut output_blocks: std::collections::HashMap<usize, Block> = std::collections::HashMap::new();
        for (cell, inner) in cells.iter().zip(&outputs) {
            if inner.trim().is_empty() {
                continue;
            }
            output_blocks.insert(cell.block_index, output_block(cell, inner));
        }

        let mut result = Vec::with_capacity(blocks.len() + output_blocks.len());
        for (i, b) in blocks.into_iter().enumerate() {
            result.push(b);
            if let Some(ob) = output_blocks.remove(&i) {
                result.push(ob);
            }
        }
        result
    }

    /// Outputs (inner HTML) for each cell, reusing cache before the earliest
    /// changed cell and executing from there to the end.
    async fn compute_outputs(&mut self, cells: &[CellRef]) -> Vec<String> {
        let first_changed = (0..cells.len())
            .find(|&i| self.cached.get(i).map(|c| c.id.as_str()) != Some(cells[i].id.as_str()))
            .unwrap_or(cells.len());

        let mut outputs = Vec::with_capacity(cells.len());
        for (i, cell) in cells.iter().enumerate() {
            if i < first_changed {
                outputs.push(self.cached[i].output.clone());
            } else {
                outputs.push(self.exec_cell(&cell.code).await);
            }
        }

        self.cached = cells
            .iter()
            .zip(&outputs)
            .map(|(c, o)| Cached { id: c.id.clone(), output: o.clone() })
            .collect();
        outputs
    }

    async fn exec_cell(&mut self, code: &str) -> String {
        if self.failed {
            return String::new();
        }
        if self.kernel.is_none() {
            match Kernel::start(&self.python).await {
                Ok(k) => self.kernel = Some(k),
                Err(e) => {
                    eprintln!(
                        "qmd-fast: kernel unavailable ({e}); code cells will render as source only \
                         (set QMD_FAST_PYTHON to a python with ipykernel)"
                    );
                    self.failed = true;
                    return String::new();
                }
            }
        }
        match self.kernel.as_mut().unwrap().execute(code).await {
            Ok(outs) => render_outputs(&outs),
            Err(e) => {
                eprintln!("qmd-fast: execution error: {e}");
                format!("<pre class=\"qmd-error\">execution error: {}</pre>", esc(&e.to_string()))
            }
        }
    }
}

/// Build the output block for a cell. Its id is the cell id + `-out`, and it
/// points click-to-source at the cell's own source position.
fn output_block(cell: &CellRef, inner: &str) -> Block {
    let id = format!("{}-out", cell.id);
    let source_file_attr = match &cell.source_file {
        Some(f) => format!(" data-source-file=\"{}\"", esc(f)),
        None => String::new(),
    };
    let html = format!(
        "<div class=\"qmd-output\" data-block-id=\"{id}\" data-sourcepos=\"{}\"{source_file_attr}>{inner}</div>",
        cell.sourcepos
    );
    Block {
        id,
        sourcepos: cell.sourcepos.clone(),
        source_file: cell.source_file.clone(),
        html,
        cell: None,
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}
