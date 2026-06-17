//! Execution engine: runs a document's Python code cells against a warm kernel
//! and splices the outputs back into the block list as their own blocks.
//!
//! Granularity is simple notebook semantics: on each rebuild, find the earliest
//! cell whose source changed and re-run it plus everything after it (downstream
//! cells may depend on the changed kernel state); cells before it reuse their
//! cached output. Each output block's id is derived from its cell's id, so it
//! swaps in place when the cell (or an upstream cell) re-runs.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use qmd_fast_core::{Block, render::CellFigure};

use crate::kernel::{Kernel, render_outputs};

/// After a failed kernel start, wait at least this long before retrying — long
/// enough that a genuinely missing/bad Python doesn't re-hang every save, short
/// enough that fixing `QMD_FAST_PYTHON`/the venv self-heals within a few saves.
const KERNEL_RETRY_AFTER: Duration = Duration::from_secs(20);

/// A code cell pulled out of the block list, with what the engine needs to run
/// it and to build its output block.
struct CellRef {
    block_index: usize,
    id: String,
    code: String,
    sourcepos: String,
    source_file: Option<String>,
    /// When set, the cell's output is wrapped in a numbered `<figure>`.
    figure: Option<CellFigure>,
    /// `#| include: false`: run the cell (for downstream state) but emit no output.
    include: bool,
}

struct Cached {
    id: String,
    output: String, // inner output HTML (may be empty)
}

pub struct Executor {
    python: PathBuf,
    kernel: Option<Kernel>,
    /// When the last kernel *start* failed (None = never failed / recovered).
    /// Drives a retry backoff instead of a permanent "failed" latch, so a fixed
    /// config self-heals without restarting the server.
    failed_at: Option<Instant>,
    cached: Vec<Cached>,
}

impl Executor {
    pub fn new() -> Self {
        let python = std::env::var_os("QMD_FAST_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("python3"));
        Self {
            python,
            kernel: None,
            failed_at: None,
            cached: Vec::new(),
        }
    }

    /// A user-facing warning about the executor's state, if any: the kernel start
    /// failed (recently), so code cells are rendering as source.
    pub fn diagnostic(&self) -> Option<String> {
        (self.kernel.is_none() && self.failed_at.is_some()).then(|| {
            "kernel unavailable; code cells render as source \
             (set QMD_FAST_PYTHON to a python with ipykernel, then Restart kernel)"
                .to_string()
        })
    }

    /// Drop the current kernel and clear the failure backoff, so the next run
    /// starts a fresh kernel immediately. Backs the dev-menu "Restart kernel"
    /// action and recovery after fixing `QMD_FAST_PYTHON`. (Dropping the kernel
    /// kills its child process.)
    pub fn restart_kernel(&mut self) {
        self.kernel = None;
        self.failed_at = None;
        self.cached.clear(); // force a full re-run against the fresh kernel
    }

    /// Execute the document's Python cells (changed cells + downstream) and
    /// return the block list with output blocks spliced in after each cell.
    pub async fn run(&mut self, blocks: Vec<Block>) -> Vec<Block> {
        let cells: Vec<CellRef> = blocks
            .iter()
            .enumerate()
            .filter_map(|(i, b)| match &b.cell {
                Some(c) if c.lang == "python" => Some(CellRef {
                    block_index: i,
                    id: b.id.clone(),
                    code: c.code.clone(),
                    sourcepos: b.sourcepos.clone(),
                    source_file: b.source_file.clone(),
                    figure: c.figure.clone(),
                    include: c.include,
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
        let mut output_blocks: std::collections::HashMap<usize, Block> =
            std::collections::HashMap::new();
        for (cell, inner) in cells.iter().zip(&outputs) {
            // `include: false` cells run (above) for their kernel-state side effects
            // but contribute no visible output block.
            if inner.trim().is_empty() || !cell.include {
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

        let to_run = cells.len().saturating_sub(first_changed);
        // Boot the kernel up-front (the real wait), so the per-cell progress below
        // reflects actual execution rather than the startup it used to hide.
        if to_run > 0 {
            self.ensure_kernel().await;
        }
        let mut outputs = Vec::with_capacity(cells.len());
        for (i, cell) in cells.iter().enumerate() {
            if i < first_changed {
                outputs.push(self.cached[i].output.clone());
            } else {
                // Progress only when the kernel is up; otherwise cells are instant
                // no-ops and a "cell k/n" line would be misleading.
                if self.kernel.is_some() {
                    crate::log::exec(i - first_changed + 1, to_run);
                }
                outputs.push(self.exec_cell(&cell.code).await);
            }
        }

        self.cached = cells
            .iter()
            .zip(&outputs)
            .map(|(c, o)| Cached {
                id: c.id.clone(),
                output: o.clone(),
            })
            .collect();
        outputs
    }

    /// Ensure a live kernel before executing. Three cases:
    ///   - a kernel that died mid-session is dropped and respawned (self-healing,
    ///     so a crash doesn't make every later cell hang on the execute timeout);
    ///   - after a failed *start* we back off for `KERNEL_RETRY_AFTER` before
    ///     retrying, so a missing/bad Python doesn't re-hang every save, but a
    ///     fixed config recovers on its own within a few saves;
    ///   - otherwise (no kernel, not in backoff) we start one.
    /// Logs the (often multi-second) boot so the wait is visible.
    async fn ensure_kernel(&mut self) {
        if let Some(k) = self.kernel.as_mut() {
            if k.is_alive() {
                return;
            }
            crate::log::warn("kernel exited; restarting");
            self.kernel = None;
            self.cached.clear(); // kernel state is gone; re-run everything
        }
        if let Some(at) = self.failed_at {
            if at.elapsed() < KERNEL_RETRY_AFTER {
                return; // still backing off; cells render as source
            }
        }
        crate::log::kernel(&format!("starting ({})", self.python.display()));
        match Kernel::start(&self.python).await {
            Ok(k) => {
                crate::log::kernel(&format!("ready ({})", self.python.display()));
                self.kernel = Some(k);
                self.failed_at = None;
            }
            Err(e) => {
                crate::log::warn(&format!(
                    "kernel unavailable ({e}); cells render as source only \
                     (set QMD_FAST_PYTHON to a python with ipykernel)"
                ));
                self.failed_at = Some(Instant::now());
            }
        }
    }

    async fn exec_cell(&mut self, code: &str) -> String {
        let Some(kernel) = self.kernel.as_mut() else {
            return String::new(); // kernel unavailable: cell renders as source
        };
        match kernel.execute(code).await {
            Ok(outs) => render_outputs(&outs),
            Err(e) => {
                crate::log::error(&format!("execution error: {e}"));
                format!(
                    "<pre class=\"qmd-error\">execution error: {}</pre>",
                    esc(&e.to_string())
                )
            }
        }
    }
}

/// Build the output block for a cell. Its id is the cell id + `-out`, and it
/// points click-to-source at the cell's own source position. A `#| label: fig-x`
/// cell wraps its output in a numbered `<figure>` so `@fig-x` resolves.
fn output_block(cell: &CellRef, inner: &str) -> Block {
    let id = format!("{}-out", cell.id);
    let source_file_attr = match &cell.source_file {
        Some(f) => format!(" data-source-file=\"{}\"", esc(f)),
        None => String::new(),
    };
    let inner = match &cell.figure {
        Some(fig) => figure_wrap(fig, inner),
        None => inner.to_string(),
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

/// Wrap a cell's rendered output in a numbered `<figure>` (caption below),
/// carrying the `#fig-` anchor so `@fig-x` cross-references resolve to it.
fn figure_wrap(fig: &CellFigure, inner: &str) -> String {
    let id_attr = match &fig.anchor {
        Some(a) => format!(" id=\"{}\"", esc(a)),
        None => String::new(),
    };
    let caption = fig.caption.as_deref().unwrap_or("").trim();
    let figcap = if caption.is_empty() {
        format!("Figure&nbsp;{}", fig.number)
    } else {
        format!("Figure&nbsp;{}: {}", fig.number, esc(caption))
    };
    format!(
        "<figure{id_attr} class=\"qmd-figure qmd-figure-center\">{inner}\
         <figcaption>{figcap}</figcaption></figure>"
    )
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
