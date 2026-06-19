//! Execution engine: runs a document's Python code cells against a warm kernel
//! and splices the outputs back into the block list as their own blocks.
//!
//! Granularity is simple notebook semantics: on each rebuild, find the earliest
//! cell whose source changed and re-run it plus everything after it (downstream
//! cells may depend on the changed kernel state); cells before it reuse their
//! cached output. Each output block's id is derived from its cell's id, so it
//! swaps in place when the cell (or an upstream cell) re-runs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use qmd_fast_core::{Block, escape_attr as esc, render::CellFigure};

use crate::kernel::{Kernel, KernelSpec, render_outputs};

/// After a failed kernel start, wait at least this long before retrying — long
/// enough that a genuinely missing/bad interpreter doesn't re-hang every save,
/// short enough that fixing `QMD_FAST_PYTHON`/`QMD_FAST_R` self-heals within a few
/// saves.
const KERNEL_RETRY_AFTER: Duration = Duration::from_secs(20);

/// Shown for cells skipped after the kernel died mid-run (see `compute_outputs`):
/// they didn't execute, and the next rebuild respawns the kernel and re-runs them.
const KERNEL_DIED_HTML: &str = "<pre class=\"qmd-error\">kernel exited before this cell ran; it will re-run on the next save</pre>";

/// Cell languages qmd-fast can execute, mapped to a stable kernel key. Anything
/// else renders as highlighted source.
fn kernel_lang(lang: &str) -> Option<&'static str> {
    match lang {
        "python" => Some("python"),
        "r" => Some("r"),
        _ => None,
    }
}

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

/// Per-language warm kernel + its output cache. One per executed language so a
/// `{python}` and an `{r}` cell run against independent, isolated kernels.
#[derive(Default)]
struct LangState {
    kernel: Option<Kernel>,
    /// When the last kernel *start* failed (None = never failed / recovered).
    /// Drives a retry backoff instead of a permanent "failed" latch.
    failed_at: Option<Instant>,
    /// The last kernel-start error (the interpreter's own message, e.g. a missing
    /// `ipykernel`), surfaced to the user so a failing kernel isn't opaque.
    last_error: Option<String>,
    cached: Vec<Cached>,
}

pub struct Executor {
    python: PathBuf,
    r: PathBuf,
    /// One warm kernel per executed language ("python", "r"), created lazily.
    langs: HashMap<&'static str, LangState>,
}

impl Executor {
    pub fn new() -> Self {
        let python = std::env::var_os("QMD_FAST_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("python3"));
        let r = std::env::var_os("QMD_FAST_R")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("R"));
        Self {
            python,
            r,
            langs: HashMap::new(),
        }
    }

    /// The launch spec + interpreter path (for logging) for a language.
    fn spec(&self, lang: &str) -> Option<(KernelSpec, PathBuf)> {
        match lang {
            "python" => Some((KernelSpec::python(&self.python), self.python.clone())),
            "r" => Some((KernelSpec::r(&self.r), self.r.clone())),
            _ => None,
        }
    }

    /// A user-facing warning about the executor's state, if any: some language's
    /// kernel start failed (recently), so its code cells are rendering as source.
    /// Includes the interpreter's own error (e.g. a missing `ipykernel`) so the
    /// failure isn't opaque.
    pub fn diagnostic(&self) -> Option<String> {
        self.langs.iter().find_map(|(lang, s)| {
            if s.kernel.is_some() || s.failed_at.is_none() {
                return None;
            }
            let var = if *lang == "r" {
                "QMD_FAST_R"
            } else {
                "QMD_FAST_PYTHON"
            };
            Some(match &s.last_error {
                Some(e) => format!(
                    "{lang} kernel unavailable — {e}. Code cells render as source; \
                     fix the interpreter ({var}) and click Restart kernel."
                ),
                None => format!(
                    "{lang} kernel unavailable; code cells render as source \
                     (set {var} to an interpreter with the Jupyter kernel, then Restart kernel)."
                ),
            })
        })
    }

    /// Drop every language's kernel and clear the failure backoff, so the next run
    /// starts fresh kernels immediately. Backs the dev-menu "Restart kernel" action
    /// and recovery after fixing `QMD_FAST_PYTHON`/`QMD_FAST_R`. (Dropping a kernel
    /// kills its child process.)
    pub fn restart_kernel(&mut self) {
        self.langs.clear();
    }

    /// Execute the document's code cells (changed cells + downstream, per language)
    /// and return the block list with output blocks spliced in after each cell.
    /// Each executable language runs against its own kernel; unknown languages are
    /// left as source.
    pub async fn run(&mut self, blocks: Vec<Block>) -> Vec<Block> {
        // Group executable cells by language, preserving document order.
        let mut by_lang: HashMap<&'static str, Vec<CellRef>> = HashMap::new();
        for (i, b) in blocks.iter().enumerate() {
            if let Some(c) = &b.cell
                && let Some(lang) = kernel_lang(&c.lang)
            {
                by_lang.entry(lang).or_default().push(CellRef {
                    block_index: i,
                    id: b.id.clone(),
                    code: c.code.clone(),
                    sourcepos: b.sourcepos.clone(),
                    source_file: b.source_file.clone(),
                    figure: c.figure.clone(),
                    include: c.include,
                });
            }
        }

        // Drop kernels/caches for languages no longer present in the document.
        self.langs.retain(|lang, _| by_lang.contains_key(lang));

        if by_lang.is_empty() {
            return blocks;
        }

        // Map cell block index -> its output block (when non-empty), across langs.
        let mut output_blocks: HashMap<usize, Block> = HashMap::new();
        for (lang, cells) in &by_lang {
            let outputs = self.compute_outputs(lang, cells).await;
            for (cell, inner) in cells.iter().zip(&outputs) {
                // `include: false` cells run (above) for their kernel-state side
                // effects but contribute no visible output block.
                if inner.trim().is_empty() || !cell.include {
                    continue;
                }
                output_blocks.insert(cell.block_index, output_block(cell, inner));
            }
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

    /// Outputs (inner HTML) for one language's cells, reusing that language's cache
    /// before the earliest changed cell and executing from there to the end.
    async fn compute_outputs(&mut self, lang: &'static str, cells: &[CellRef]) -> Vec<String> {
        let first_changed = self
            .langs
            .get(lang)
            .map(|s| first_changed_index(&s.cached, cells))
            .unwrap_or(0);

        let to_run = cells.len().saturating_sub(first_changed);
        // Boot the kernel up-front (the real wait), so the per-cell progress below
        // reflects actual execution rather than the startup it used to hide.
        if to_run > 0 {
            self.ensure_kernel(lang).await;
        }
        let has_kernel = self
            .langs
            .get(lang)
            .map(|s| s.kernel.is_some())
            .unwrap_or(false);
        let mut outputs = Vec::with_capacity(cells.len());
        let mut ran = 0;
        for (i, cell) in cells.iter().enumerate() {
            if i < first_changed {
                let cached = self
                    .langs
                    .get(lang)
                    .and_then(|s| s.cached.get(i))
                    .map(|c| c.output.clone())
                    .unwrap_or_default();
                outputs.push(cached);
            } else if has_kernel && !self.kernel_alive(lang) {
                // The kernel was up when this run started but has since exited (an
                // earlier cell crashed it). Don't run the rest: each `execute` would
                // just wait out the full cell timeout on a kernel that will never
                // reply. Fail fast; the next rebuild detects the dead kernel,
                // respawns it, and re-runs everything.
                outputs.push(KERNEL_DIED_HTML.to_string());
            } else {
                // Progress only when the kernel is up; otherwise cells are instant
                // no-ops and a "cell k/n" line would be misleading.
                if has_kernel {
                    ran += 1;
                    crate::log::exec(ran, to_run);
                }
                outputs.push(self.exec_cell(lang, &cell.code).await);
            }
        }

        if let Some(state) = self.langs.get_mut(lang) {
            state.cached = cells
                .iter()
                .zip(&outputs)
                .map(|(c, o)| Cached {
                    id: c.id.clone(),
                    output: o.clone(),
                })
                .collect();
        }
        outputs
    }

    /// Ensure a live kernel for `lang` before executing. Three cases:
    ///   - a kernel that died mid-session is dropped and respawned (self-healing,
    ///     so a crash doesn't make every later cell hang on the execute timeout);
    ///   - after a failed *start* we back off for `KERNEL_RETRY_AFTER` before
    ///     retrying, so a missing/bad interpreter doesn't re-hang every save, but a
    ///     fixed config recovers on its own within a few saves;
    ///   - otherwise (no kernel, not in backoff) we start one.
    ///
    /// Logs the (often multi-second) boot so the wait is visible.
    async fn ensure_kernel(&mut self, lang: &'static str) {
        // Build the launch spec before borrowing the per-language state mutably.
        let Some((spec, program)) = self.spec(lang) else {
            return;
        };
        let state = self.langs.entry(lang).or_default();
        if let Some(k) = state.kernel.as_mut() {
            if k.is_alive() {
                return;
            }
            crate::log::warn(&format!("{lang} kernel exited; restarting"));
            state.kernel = None;
            state.cached.clear(); // kernel state is gone; re-run everything
        }
        if let Some(at) = state.failed_at
            && at.elapsed() < KERNEL_RETRY_AFTER
        {
            return; // still backing off; cells render as source
        }
        crate::log::kernel(&format!("starting {lang} ({})", program.display()));
        match Kernel::start(&spec).await {
            Ok(k) => {
                crate::log::kernel(&format!("{lang} ready ({})", program.display()));
                state.kernel = Some(k);
                state.failed_at = None;
                state.last_error = None;
            }
            Err(e) => {
                crate::log::warn(&format!(
                    "{lang} kernel unavailable ({e}); cells render as source only"
                ));
                state.failed_at = Some(Instant::now());
                state.last_error = Some(e.to_string());
            }
        }
    }

    /// Whether `lang` currently has a *live* kernel process. Used mid-run to bail
    /// out instead of waiting out the cell timeout on a kernel that just died.
    fn kernel_alive(&mut self, lang: &'static str) -> bool {
        self.langs
            .get_mut(lang)
            .and_then(|s| s.kernel.as_mut())
            .is_some_and(|k| k.is_alive())
    }

    async fn exec_cell(&mut self, lang: &'static str, code: &str) -> String {
        let Some(kernel) = self.langs.get_mut(lang).and_then(|s| s.kernel.as_mut()) else {
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

/// Index of the earliest cell whose id differs from the previous cached run (or
/// where the cache is shorter): everything from here re-runs, everything before
/// reuses its cached output. Pure over the two id sequences, so the re-run
/// granularity is unit-testable without a kernel.
fn first_changed_index(cached: &[Cached], cells: &[CellRef]) -> usize {
    (0..cells.len())
        .find(|&i| cached.get(i).map(|c| c.id.as_str()) != Some(cells[i].id.as_str()))
        .unwrap_or(cells.len())
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

#[cfg(test)]
mod tests {
    //! The output-splicing helpers are pure (no kernel), so they're tested
    //! directly: they're what carries a cell's executed output back into the
    //! block model, and they must preserve the click-to-source invariants
    //! (output id keyed to the cell, sourcepos/source-file carried through) and
    //! the `#fig-` anchor that lets `@fig-x` resolve to the output.
    use super::*;

    fn cell(id: &str) -> CellRef {
        CellRef {
            block_index: 0,
            id: id.to_string(),
            code: "print(1)".into(),
            sourcepos: "5:1-7:3".into(),
            source_file: None,
            figure: None,
            include: true,
        }
    }

    fn cells(ids: &[&str]) -> Vec<CellRef> {
        ids.iter().map(|id| cell(id)).collect()
    }

    fn cached(ids: &[&str]) -> Vec<Cached> {
        ids.iter()
            .map(|id| Cached {
                id: id.to_string(),
                output: String::new(),
            })
            .collect()
    }

    #[test]
    fn first_changed_index_drives_cache_reuse_granularity() {
        // Unchanged cell ids -> nothing re-runs (index == len, all cached).
        assert_eq!(
            first_changed_index(&cached(&["a", "b", "c"]), &cells(&["a", "b", "c"])),
            3
        );
        // The first differing cell, and everything after it, re-runs.
        assert_eq!(
            first_changed_index(&cached(&["a", "b", "c"]), &cells(&["a", "X", "c"])),
            1
        );
        assert_eq!(
            first_changed_index(&cached(&["a", "b", "c"]), &cells(&["Z", "b", "c"])),
            0
        );
        // A cold cache (or a freshly appended trailing cell) re-runs from the gap.
        assert_eq!(first_changed_index(&[], &cells(&["a", "b"])), 0);
        assert_eq!(
            first_changed_index(&cached(&["a", "b"]), &cells(&["a", "b", "c"])),
            2
        );
        // A removed trailing cell leaves the survivors cached (index past the end).
        assert_eq!(
            first_changed_index(&cached(&["a", "b", "c"]), &cells(&["a", "b"])),
            2
        );
    }

    #[test]
    fn output_block_keys_id_to_cell_and_carries_clickto_source() {
        let b = output_block(&cell("b-abc"), "<pre>1</pre>");
        // id derived from the cell so the output swaps in place when it re-runs.
        assert_eq!(b.id, "b-abc-out");
        // click-to-source points back at the cell's own source position.
        assert_eq!(b.sourcepos, "5:1-7:3");
        assert!(b.cell.is_none(), "an output block is not itself a cell");
        assert!(
            b.html.contains("class=\"qmd-output\"")
                && b.html.contains("data-block-id=\"b-abc-out\"")
                && b.html.contains("data-sourcepos=\"5:1-7:3\""),
            "missing block-model attributes: {}",
            b.html
        );
        assert!(
            b.html.contains("<pre>1</pre>"),
            "inner output dropped: {}",
            b.html
        );
        // no source_file -> no data-source-file attribute.
        assert!(!b.html.contains("data-source-file"), "{}", b.html);
    }

    #[test]
    fn output_block_emits_escaped_data_source_file_for_included_cells() {
        let mut c = cell("b1");
        c.source_file = Some("posts/p&q.qmd".into());
        let b = output_block(&c, "x");
        assert_eq!(b.source_file.as_deref(), Some("posts/p&q.qmd"));
        assert!(
            b.html.contains("data-source-file=\"posts/p&amp;q.qmd\""),
            "source file not emitted/escaped: {}",
            b.html
        );
    }

    #[test]
    fn figure_wrap_numbers_anchors_and_escapes_caption() {
        let fig = CellFigure {
            anchor: Some("fig-cov".into()),
            caption: Some("Cov & vars".into()),
            number: 2,
        };
        let html = figure_wrap(&fig, "<img src=\"c.png\">");
        assert!(
            html.starts_with("<figure id=\"fig-cov\" class=\"qmd-figure qmd-figure-center\">"),
            "anchor/classes wrong: {html}"
        );
        assert!(
            html.contains("<img src=\"c.png\">"),
            "inner dropped: {html}"
        );
        assert!(
            html.contains("<figcaption>Figure&nbsp;2: Cov &amp; vars</figcaption>"),
            "caption not numbered/escaped: {html}"
        );
    }

    #[test]
    fn figure_wrap_without_anchor_or_caption_is_bare_numbered() {
        let fig = CellFigure {
            anchor: None,
            caption: None,
            number: 1,
        };
        let html = figure_wrap(&fig, "out");
        assert!(
            html.starts_with("<figure class=\"qmd-figure"),
            "an unlabelled figure must carry no id: {html}"
        );
        assert!(
            html.contains("<figcaption>Figure&nbsp;1</figcaption>"),
            "bare number missing: {html}"
        );
        assert!(!html.contains(':'), "no caption -> no colon: {html}");
    }

    #[test]
    fn output_block_wraps_a_labelled_cells_output_in_a_figure() {
        let mut c = cell("b2");
        c.figure = Some(CellFigure {
            anchor: Some("fig-x".into()),
            caption: Some("Cap".into()),
            number: 3,
        });
        let b = output_block(&c, "<img>");
        // the figure nests inside the qmd-output wrapper, anchored for @fig-x.
        assert!(b.html.contains("class=\"qmd-output\""), "{}", b.html);
        assert!(
            b.html.contains("id=\"fig-x\""),
            "figure anchor missing: {}",
            b.html
        );
        assert!(
            b.html
                .contains("<figcaption>Figure&nbsp;3: Cap</figcaption>"),
            "{}",
            b.html
        );
    }
}
