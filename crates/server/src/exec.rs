//! Execution engine: runs a document's Python/R code cells against a warm kernel
//! and splices the outputs back into the block list as their own blocks.
//!
//! ## Granularity & the persistent cache
//!
//! Cell outputs are keyed by a cumulative content hash (see [`crate::freeze`]):
//! cell `i`'s key folds in every same-language cell's code up to and including it,
//! so editing a cell — or anything upstream — moves its key and every downstream
//! key. Each run plans (via [`plan`]) which contiguous range of cells must actually
//! execute:
//!
//!   - in a **warm session**, the live kernel already holds the unchanged prefix's
//!     state, so only the changed cell + downstream re-run (notebook semantics);
//!   - on a **cold start** (a `build`, or a preview after restart) the kernel holds
//!     nothing, so if every cell hits the disk cache we replay all outputs and never
//!     boot the kernel; if anything changed we re-run from the first cell whose
//!     state the kernel lacks. Kernel *variable* state is never cached (that's what
//!     makes Quarto's per-cell `cache` fragile), so a cold start can only skip work
//!     when the whole document is unchanged.
//!
//! Each output block's id is derived from its cell's id, so it swaps in place when
//! the cell (or an upstream cell) re-runs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use taliesin_core::{
    Block, escape_attr as esc,
    render::{CellFigure, CellTable},
};

use crate::freeze::{self, FreezeCache};
use crate::kernel::{Kernel, KernelSpec, render_outputs};

/// After a failed kernel start, wait at least this long before retrying — long
/// enough that a genuinely missing/bad interpreter doesn't re-hang every save,
/// short enough that fixing `TALIESIN_PYTHON`/`TALIESIN_R` self-heals within a few
/// saves.
const KERNEL_RETRY_AFTER: Duration = Duration::from_secs(20);

/// Shown for cells skipped after the kernel died mid-run (see `compute_outputs`):
/// they didn't execute, and the next rebuild respawns the kernel and re-runs them.
const KERNEL_DIED_HTML: &str = "<pre class=\"tali-error\">kernel exited before this cell ran; it will re-run on the next save</pre>";

/// A callback the server hands the executor to stream build progress
/// (`build-state` messages) to the previewing client: each call receives a
/// ready-to-send JSON string (built by [`crate::protocol::build_state`]). `None`
/// on the headless `build` path, where there's no client to push to. Emission is
/// side-effect-free w.r.t. what executes or caches, so a `None` sink changes
/// nothing else.
pub type ProgressSink = Option<Arc<dyn Fn(String) + Send + Sync>>;

/// Send a progress message if a sink is wired; a no-op when it isn't.
fn emit(sink: &ProgressSink, msg: String) {
    if let Some(s) = sink {
        s(msg);
    }
}

/// Emit a terminal `error` `cell-state` for each cell in the half-open `range`.
/// Used on the two paths where run-range cells *can't* run: the kernel boot failed,
/// or it died mid-run before reaching them. Those cells were already announced
/// `queued`; without this terminal `error` they'd stay `queued` and spin forever in
/// the client. Pure observation — never changes what executes or caches.
fn emit_cell_errors(
    sink: &ProgressSink,
    page: Option<&str>,
    cells: &[CellRef],
    range: std::ops::Range<usize>,
) {
    for cell in &cells[range] {
        emit(
            sink,
            crate::protocol::cell_state(page, &cell.id, "error", None, None),
        );
    }
}

/// Wall-clock epoch millis, for tagging `cell-state` messages with a `started_ms`
/// and computing a cell's `duration_ms`. Saturates to 0 before the epoch (never
/// happens in practice); only ever observed, so it can't change what runs/caches.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Cell languages taliesin can execute, mapped to a stable kernel key. Anything
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
    /// When set, the cell's executed `<table>` gets a numbered caption + `#tbl-` id.
    table: Option<CellTable>,
    /// `#| include: false`: run the cell (for downstream state) but emit no output.
    include: bool,
    /// `#| cache: false`: never restore from / persist to the disk cache; always
    /// re-executes (the escape hatch for non-deterministic cells).
    cache: bool,
    /// `#| fig-export: figures/x.pdf[, …]`: also write the cell's figure to these
    /// files (print-clean) for LaTeX/print. Folded into the cache key so adding or
    /// changing it re-runs the cell (and thus rewrites the file).
    fig_export: Option<String>,
}

/// A cell the *current warm kernel* has executed: its cumulative cache key and the
/// output it produced. Ordered, contiguous from cell 0 — so it doubles as the
/// "what state does the live kernel hold" record the [`plan`]ner diffs against.
struct Ran {
    hash: String,
    output: String, // inner output HTML (may be empty)
}

/// Per-language warm kernel + the cells it has executed. One per executed language
/// so a `{python}` and an `{r}` cell run against independent, isolated kernels.
#[derive(Default)]
struct LangState {
    kernel: Option<Kernel>,
    /// When the last kernel *start* failed (None = never failed / recovered).
    /// Drives a retry backoff instead of a permanent "failed" latch.
    failed_at: Option<Instant>,
    /// The last kernel-start error (the interpreter's own message, e.g. a missing
    /// `ipykernel`), surfaced to the user so a failing kernel isn't opaque.
    last_error: Option<String>,
    /// Cells the live kernel has run, in order from cell 0 (empty when cold). Drives
    /// the warm-prefix reuse: a cell whose key still matches keeps its output and
    /// isn't re-run, because the kernel still holds its state.
    ran: Vec<Ran>,
}

pub struct Executor {
    python: PathBuf,
    r: PathBuf,
    /// One warm kernel per executed language ("python", "r"), created lazily.
    langs: HashMap<&'static str, LangState>,
    /// Disk-backed output cache for this document (the L2 behind the per-language
    /// in-memory `ran`). Disabled by [`Executor::new`]; bound to a `_freeze/` file
    /// by [`Executor::with_freeze`].
    freeze: FreezeCache,
    /// Set by [`Executor::restart_kernel`]: makes the *next* run ignore disk-cache
    /// hits and re-execute every cell against the fresh kernel (then re-persist),
    /// so "Restart kernel" actually re-runs rather than replaying stale outputs.
    force_next: bool,
    /// `--no-exec` / `TALIESIN_NO_EXEC`: never run code cells, render them as source.
    /// The safe way to preview a document you don't trust (executing it would run
    /// its `{python}`/`{r}` cells against a live kernel).
    no_exec: bool,
    /// Working directory for this document's kernels (the document's own dir), so a
    /// cell's relative writes land beside the source instead of in the server's
    /// launch dir. `None` inherits the server's cwd (the default; used by tests).
    work_dir: Option<PathBuf>,
    /// Where to push `build-state` progress (set by a dev server before a build);
    /// `None` on the headless `build` path. Side-effect-free: never changes what
    /// runs or caches.
    sink: ProgressSink,
    /// The source rel-path this executor builds (the site server's page key), tagged
    /// onto each `build-state` so a multi-page client knows which page it's about.
    /// `None` for the single-doc server.
    page: Option<String>,
    /// Optional eager warm pool of pre-booted Python kernels (one per server
    /// process; see [`crate::warm_pool::WarmPool`]). When set, `ensure_kernel` claims
    /// a ready kernel from it for `python` instead of paying a cold `Kernel::start`,
    /// so the first edit is near-instant. `None` (the default, and the `build`/test
    /// path with no pool) cold-starts exactly as before — no behavioral change.
    pool: Option<Arc<crate::warm_pool::WarmPool>>,
}

impl Executor {
    /// An executor with the persistent cache **disabled** (in-memory warm reuse
    /// only). Used where there's no on-disk home for the cache.
    pub fn new() -> Self {
        Self::build(FreezeCache::disabled())
    }

    /// An executor whose outputs are cached in (and restored from) the `_freeze/`
    /// file at `freeze_path`, so unchanged cells survive across `build` runs and
    /// preview restarts.
    pub fn with_freeze(freeze_path: PathBuf) -> Self {
        Self::build(FreezeCache::for_page(freeze_path))
    }

    /// Run this executor's kernels in `dir` (the document's directory), so a cell's
    /// relative file writes (audio, `#| fig-export:` figures, `ggsave`) land beside
    /// the source rather than wherever the server was launched. Canonicalized to an
    /// absolute path (an empty/relative `dir` resolves against the current dir); a
    /// path that can't be canonicalized is used as given.
    pub fn in_dir(mut self, dir: &std::path::Path) -> Self {
        let dir = if dir.as_os_str().is_empty() {
            std::path::Path::new(".")
        } else {
            dir
        };
        self.work_dir = Some(dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf()));
        self
    }

    fn build(freeze: FreezeCache) -> Self {
        let python = std::env::var_os("TALIESIN_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("python3"));
        let r = std::env::var_os("TALIESIN_R")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("R"));
        Self {
            python,
            r,
            langs: HashMap::new(),
            freeze,
            force_next: false,
            no_exec: std::env::var_os("TALIESIN_NO_EXEC").is_some(),
            work_dir: None,
            sink: None,
            page: None,
            pool: None,
        }
    }

    /// Stream this executor's per-build progress (`build-state` messages) through
    /// `sink`, tagged with the page rel-path `page` (the site server's page key;
    /// `None` for the single-doc server). The server sets this once after creating the
    /// executor; the `build` path leaves it unset (no client). Emission never changes
    /// what executes or caches, so freeze determinism is preserved regardless of the
    /// sink. A `&mut self` setter (not a consuming builder) so it can be applied to a
    /// pooled `&mut Executor`.
    pub fn set_progress(&mut self, sink: ProgressSink, page: Option<String>) {
        self.sink = sink;
        self.page = page;
    }

    /// Draw this executor's `python` kernel from the shared warm `pool` when one is
    /// ready, instead of cold-starting it. Mirrors [`Executor::set_progress`]: a
    /// `&mut self` setter so a server can apply one process-wide pool to each pooled
    /// `&mut Executor` it reuses. A pooled kernel runs the **same** ipykernel with the
    /// same startup preambles as a cold one, so it executes cells identically — the
    /// pool changes only *boot latency*, never outputs (determinism preserved).
    /// Unset (the `build`/test default) → every kernel cold-starts as before.
    pub fn set_warm_pool(&mut self, pool: Option<Arc<crate::warm_pool::WarmPool>>) {
        self.pool = pool;
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
                "TALIESIN_R"
            } else {
                "TALIESIN_PYTHON"
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
    /// and recovery after fixing `TALIESIN_PYTHON`/`TALIESIN_R`. (Dropping a kernel
    /// kills its child process.) Also forces the next run to re-execute every cell
    /// (ignoring disk-cache hits), so "Restart kernel" actually re-runs against the
    /// fresh kernel instead of replaying cached outputs.
    pub fn restart_kernel(&mut self) {
        self.langs.clear();
        self.force_next = true;
    }

    /// Execute the document's code cells (changed cells + downstream, per language)
    /// and return the block list with output blocks spliced in after each cell.
    /// Each executable language runs against its own kernel; unknown languages are
    /// left as source.
    pub async fn run(&mut self, blocks: Vec<Block>) -> Vec<Block> {
        // `--no-exec`: never touch a kernel. The cells are already rendered as source
        // in `blocks`; returning them unchanged is exactly "preview as source".
        if self.no_exec {
            return blocks;
        }
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
                    table: c.table.clone(),
                    include: c.include,
                    cache: c.cache,
                    fig_export: c.fig_export.clone(),
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
        // A forced re-run (Restart kernel) applies to every language in this pass,
        // then clears. Flush any newly executed outputs to `_freeze/` once.
        self.force_next = false;
        self.freeze.save();

        let mut result = Vec::with_capacity(blocks.len() + output_blocks.len());
        for (i, b) in blocks.into_iter().enumerate() {
            result.push(b);
            if let Some(ob) = output_blocks.remove(&i) {
                result.push(ob);
            }
        }
        result
    }

    /// Outputs (inner HTML) for one language's cells: restore the unchanged warm
    /// prefix + any disk-cached tail, and execute the contiguous range in between
    /// (see [`plan`]). Freshly executed, cacheable, non-error outputs are persisted.
    async fn compute_outputs(&mut self, lang: &'static str, cells: &[CellRef]) -> Vec<String> {
        // The interpreter identity seeds the cumulative hash chain (a different
        // interpreter/version can't serve another's outputs). Computed up front so
        // even a full cold replay — which never boots the kernel — can key the cache.
        let interp = self
            .spec(lang)
            .map(|(_, program)| interp_id(lang, &program))
            .unwrap_or_else(|| lang.to_string());
        // Fold `#| fig-export:` into the hashed code so adding/changing it moves the
        // cell's key and forces a re-run (which rewrites the exported file); a cell
        // without it hashes exactly as before, so existing caches stay valid.
        let codes: Vec<String> = cells
            .iter()
            .map(|c| match &c.fig_export {
                Some(spec) => format!("{}\n# tali-fig-export: {spec}", c.code),
                None => c.code.clone(),
            })
            .collect();
        let code_refs: Vec<&str> = codes.iter().map(String::as_str).collect();
        let hashes = freeze::cumulative_hashes(&interp, &code_refs);

        // A cell is "known" (restorable without running) when its output is on disk
        // and it isn't opted out (`#| cache: false` always re-executes). A forced
        // re-run (Restart kernel) treats everything as unknown.
        let force = self.force_next;
        let known = |i: usize| !force && cells[i].cache && self.freeze.get(&hashes[i]).is_some();
        let ran: Vec<String> = self
            .langs
            .get(lang)
            .map(|s| s.ran.iter().map(|r| r.hash.clone()).collect())
            .unwrap_or_default();
        let (shared, run_end) = plan(&ran, &hashes, known, |i| cells[i].cache);

        // Per-cell states from the zones `plan()` just computed (pure observation —
        // doesn't change what runs or caches). The warm prefix `[0, shared)` and the
        // cached tail `[run_end, len)` are already available, so they're `done`; the
        // run range `[shared, run_end)` is `queued` and turns `running`/`done`/`error`
        // in the loop below. `cell_id` is each cell's own id (the id the output block
        // is built from as `{id}-out`), so the client can target that block.
        for (i, cell) in cells.iter().enumerate() {
            let state = if i < shared || i >= run_end {
                "done"
            } else {
                "queued"
            };
            emit(
                &self.sink,
                crate::protocol::cell_state(self.page.as_deref(), &cell.id, state, None, None),
            );
        }

        let to_run = run_end.saturating_sub(shared);
        // Boot the kernel up-front (the real wait), so the per-cell progress below
        // reflects actual execution rather than the startup it used to hide. A full
        // replay (to_run == 0) never boots: that's the cold-start fast path — and it
        // must never claim "warming-kernel", so the boot is gated on `to_run > 0`.
        //
        // The `warming-kernel` signal itself is now gated *inside* `ensure_kernel`,
        // which emits it **only** when it actually pays a cold `Kernel::start`. A
        // warm-pool HIT (a ready, pre-booted kernel) is near-instant, so it must not
        // present a long warming state; passing `to_run` lets `ensure_kernel` build
        // the same `build-state` message on the cold path only.
        if to_run > 0 {
            self.ensure_kernel(lang, to_run).await;
        }
        let has_kernel = self
            .langs
            .get(lang)
            .map(|s| s.kernel.is_some())
            .unwrap_or(false);

        // Kernel BOOT failed (we needed to run cells but couldn't start the kernel).
        // Be honest: the build did NOT succeed, so it must not later emit a clean
        // `idle` claiming `ran == total`. Settle on `error` and give every run-range
        // cell a terminal `error` now, so none stays `queued`/spinning (without a
        // kernel the execute loop below treats them as instant no-ops and would never
        // emit a terminal state for them). The cells still render as source.
        let boot_failed = to_run > 0 && !has_kernel;
        if boot_failed {
            emit(
                &self.sink,
                crate::protocol::build_state(self.page.as_deref(), "error", 0, to_run as u32, lang),
            );
            emit_cell_errors(&self.sink, self.page.as_deref(), cells, shared..run_end);
        }

        // Outputs already known without running, pulled out before the execute loop
        // so they don't hold a borrow on `self` across `exec_cell`: the warm prefix
        // from the live kernel's in-memory record, the tail from the disk cache.
        let warm: Vec<String> = self
            .langs
            .get(lang)
            .map(|s| {
                s.ran
                    .iter()
                    .take(shared)
                    .map(|r| r.output.clone())
                    .collect()
            })
            .unwrap_or_default();
        let tail: Vec<String> = (run_end..cells.len())
            .map(|i| self.freeze.get(&hashes[i]).unwrap_or_default().to_string())
            .collect();

        // Cloned out of `self` so the execute loop can still borrow `self` mutably
        // (`exec_cell`/`kernel_alive`) while emitting progress. The sink is an `Arc`
        // (cheap clone), the page a small `Option<String>`.
        let sink = self.sink.clone();
        let page = self.page.clone();

        let mut outputs = Vec::with_capacity(cells.len());
        let mut ran_count = 0;
        for (i, cell) in cells.iter().enumerate() {
            if i < shared {
                outputs.push(warm.get(i).cloned().unwrap_or_default());
            } else if i < run_end {
                if !has_kernel {
                    // The kernel could not boot (a port-allocation race under
                    // concurrent starts, a backoff after a failed start, or no
                    // interpreter): this cell was meant to run but can't. Splice a
                    // VISIBLE diagnostic where its output would go — a build has no
                    // websocket, so the `error` cell_state/build_state emitted above
                    // never reaches the HTML; without this the output div would simply
                    // be absent (the silent drop). The cell still renders as source
                    // above this block. (`tali-error` => styled as an error AND treated
                    // as uncacheable, so it is never persisted to the freeze cache.)
                    outputs.push(kernel_unavailable_html(
                        lang,
                        self.langs.get(lang).and_then(|s| s.last_error.as_deref()),
                    ));
                } else if !self.kernel_alive(lang) {
                    // The kernel was up when this run started but has since exited (an
                    // earlier cell crashed it). Don't run the rest: each `execute`
                    // would just wait out the full cell timeout on a kernel that will
                    // never reply. Fail fast; the next rebuild detects the dead
                    // kernel, respawns it, and re-runs everything.
                    //
                    // This cell was announced `queued`; give it a terminal `error` so it
                    // doesn't stay queued/spinning forever in the client (it didn't run,
                    // and won't this pass). `build-state` stays `executing`/settles on
                    // `idle` for the cells that did run before the crash.
                    // (This mid-run dead-kernel path is production-only; toy runs rarely hit it.)
                    emit(
                        &sink,
                        crate::protocol::cell_state(page.as_deref(), &cell.id, "error", None, None),
                    );
                    outputs.push(KERNEL_DIED_HTML.to_string());
                } else {
                    // Progress only when the kernel is up; otherwise cells are instant
                    // no-ops and a "cell k/n" line would be misleading.
                    if has_kernel {
                        ran_count += 1;
                        crate::log::exec(ran_count, to_run);
                        emit(
                            &sink,
                            crate::protocol::build_state(
                                page.as_deref(),
                                "executing",
                                ran_count as u32,
                                to_run as u32,
                                lang,
                            ),
                        );
                    }
                    // Python `#| fig-export:` cells get a one-line trigger prepended
                    // so the kernel writes the figure to disk when it's displayed.
                    let code = if lang == "python" {
                        export_wrapped(&cell.code, cell.fig_export.as_deref())
                    } else {
                        cell.code.clone()
                    };
                    // queued → running → done|error per cell. Only when a kernel is
                    // actually live: without one the cell is an instant no-op that
                    // stays honestly `queued` (it never ran), and we never emit
                    // `running` without a prior `queued`.
                    let t0 = has_kernel.then(|| {
                        let t0 = now_ms();
                        emit(
                            &sink,
                            crate::protocol::cell_state(
                                page.as_deref(),
                                &cell.id,
                                "running",
                                Some(t0),
                                None,
                            ),
                        );
                        t0
                    });
                    let out = self.exec_cell(lang, &code).await;
                    if let Some(t0) = t0 {
                        let state = if is_uncacheable(&out) {
                            "error"
                        } else {
                            "done"
                        };
                        emit(
                            &sink,
                            crate::protocol::cell_state(
                                page.as_deref(),
                                &cell.id,
                                state,
                                Some(t0),
                                Some(now_ms().saturating_sub(t0)),
                            ),
                        );
                    }
                    outputs.push(out);
                }
            } else {
                outputs.push(tail[i - run_end].clone());
            }
        }

        // Persist freshly executed, cacheable, non-error outputs (only ones a live
        // kernel actually produced). Errors and `cache: false` cells are never
        // stored, so a transient failure or a nondeterministic cell never sticks.
        if has_kernel {
            for i in shared..run_end {
                if cells[i].cache && !is_uncacheable(&outputs[i]) {
                    self.freeze.put(hashes[i].clone(), outputs[i].clone());
                }
            }
        }

        // Record what the kernel now holds: cells [0, run_end) ran contiguously (the
        // warm prefix it already had, plus what we just executed). The disk-restored
        // tail is deliberately NOT recorded — the kernel never ran it, so a later
        // edit there re-runs from here to rebuild state.
        //
        // Only record when a kernel is actually live. Without one, cells [shared,
        // run_end) ran as no-ops (empty output); recording them as `ran` would make
        // them part of the warm prefix, so when the kernel later self-heals they'd
        // be skipped instead of re-run — leaving stale/missing output.
        if has_kernel && let Some(state) = self.langs.get_mut(lang) {
            state.ran = (0..run_end)
                .map(|i| Ran {
                    hash: hashes[i].clone(),
                    output: outputs[i].clone(),
                })
                .collect();
        }

        // The build for this language settled: report `idle` with the full count.
        // An all-cached page (to_run == 0) reaches here without ever emitting
        // `warming-kernel`/`executing`, so its first and only signal is `idle`.
        // Skipped when the kernel boot failed: that build already settled on `error`
        // above, and a trailing `idle` would falsely overwrite it with "success".
        if !boot_failed {
            emit(
                &sink,
                crate::protocol::build_state(
                    page.as_deref(),
                    "idle",
                    to_run as u32,
                    to_run as u32,
                    lang,
                ),
            );
        }
        outputs
    }

    /// Ensure a live kernel for `lang` before executing. Cases, in order:
    ///   - a kernel that died mid-session is dropped and respawned (self-healing,
    ///     so a crash doesn't make every later cell hang on the execute timeout);
    ///   - after a failed *start* we back off for `KERNEL_RETRY_AFTER` before
    ///     retrying, so a missing/bad interpreter doesn't re-hang every save, but a
    ///     fixed config recovers on its own within a few saves;
    ///   - for `python`, a ready kernel from the **warm pool** (if one is wired) is
    ///     claimed instead of cold-starting — near-instant, no `warming-kernel`;
    ///   - otherwise (no kernel, not in backoff, no warm hit) we cold-start one,
    ///     emitting the `warming-kernel` build-state around the real (multi-second)
    ///     wait. `to_run` is the cell count that boot is unblocking, for that message.
    ///
    /// The `warming-kernel` signal is emitted **only** on the genuine cold-start
    /// path, so a warm-pool hit (or a still-live kernel) never shows a long warm-up.
    /// A pooled kernel is the same ipykernel running the same preambles as a cold
    /// one, so it executes cells identically — pooling changes only latency.
    async fn ensure_kernel(&mut self, lang: &'static str, to_run: usize) {
        // Build the launch spec before borrowing the per-language state mutably.
        let Some((spec, program)) = self.spec(lang) else {
            return;
        };
        let work_dir = self.work_dir.clone();
        {
            let state = self.langs.entry(lang).or_default();
            if let Some(k) = state.kernel.as_mut() {
                if k.is_alive() {
                    return; // already warm — no boot, no warming signal
                }
                crate::log::warn(&format!("{lang} kernel exited; restarting"));
                state.kernel = None;
                state.ran.clear(); // kernel state is gone; re-run everything
            }
            if let Some(at) = state.failed_at
                && at.elapsed() < KERNEL_RETRY_AFTER
            {
                return; // still backing off; cells render as source (no signal)
            }
        }

        // Warm-pool fast path (python only): a ready, pre-booted kernel is claimed
        // with no perceptible wait, so we emit **no** `warming-kernel` state. The
        // pool may yield `None` (inert pool, empty queue, or a non-python lang) — in
        // which case we fall through to the unchanged cold start below. A pooled
        // kernel is forked from the daemon's cwd, so we chdir it to this document's
        // `work_dir` to match a cold kernel started with `current_dir(work_dir)`;
        // this keeps relative cell writes (fig-export, audio) landing beside the
        // source, exactly as before (and preserves the per-page file isolation the
        // determinism/clobber tests rely on).
        if lang == "python"
            && let Some(pool) = self.pool.clone()
            && let Some(mut kernel) = pool.take().await
        {
            if let Some(dir) = work_dir.as_deref() {
                set_kernel_cwd(&mut kernel, dir).await;
            }
            crate::log::kernel(&format!("{lang} ready (warm pool)"));
            let state = self.langs.entry(lang).or_default();
            state.kernel = Some(kernel);
            state.failed_at = None;
            state.last_error = None;
            return;
        }

        // Cold start: the real (often multi-second) wait. This is the *only* path
        // that presents `warming-kernel`, so the signal is honest.
        emit(
            &self.sink,
            crate::protocol::build_state(
                self.page.as_deref(),
                "warming-kernel",
                0,
                to_run as u32,
                lang,
            ),
        );
        crate::log::kernel(&format!("starting {lang} ({})", program.display()));
        // Retry a transient start failure with a fresh port allocation. Under
        // concurrent builds `peek_ports` can hand two kernels the same loopback port
        // (it tests-then-releases each), so the loser exits with "address already in
        // use" and — before this — silently rendered its cells as source. A re-roll
        // almost always lands free ports; the short, attempt-scaled backoff lets the
        // colliding peer finish binding first. A permanent failure (missing
        // interpreter/module) breaks out at once so the honest error isn't delayed.
        const START_ATTEMPTS: usize = 4;
        let mut started = Kernel::start(&spec, work_dir.as_deref()).await;
        let mut attempt = 1;
        while let Err(e) = &started {
            if attempt >= START_ATTEMPTS || !crate::kernel::start_error_is_transient(&e.to_string())
            {
                break;
            }
            crate::log::warn(&format!(
                "{lang} kernel start hit a transient failure ({e}); retrying ({attempt}/{START_ATTEMPTS})"
            ));
            tokio::time::sleep(Duration::from_millis(40 * attempt as u64)).await;
            started = Kernel::start(&spec, work_dir.as_deref()).await;
            attempt += 1;
        }
        let state = self.langs.entry(lang).or_default();
        match started {
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
                    "<pre class=\"tali-error\">execution error: {}</pre>",
                    esc(&e.to_string())
                )
            }
        }
    }
}

/// Point a freshly-claimed warm-pool kernel at `dir`, so its relative file writes
/// (fig-export PDFs, `ggsave`, audio) land beside the document's source — matching a
/// cold kernel that `Kernel::start` launches with `current_dir(dir)`. The pool forks
/// kernels from the daemon's cwd, so without this they'd write into the server's
/// launch dir instead; setting it here keeps behavior (and the per-page file
/// isolation the determinism tests assert) identical to the cold path.
///
/// Runs `os.chdir(...)` as a setup statement — it yields no display output, so it
/// never appears in any cell's rendered HTML (output-invisible; can't perturb the
/// freeze cache or the determinism invariant). A failure is logged and ignored: the
/// kernel still works, just with the daemon's cwd, no worse than a fallback.
async fn set_kernel_cwd(kernel: &mut Kernel, dir: &Path) {
    // A normal path string in a Python single-quoted literal; embedded quotes/
    // backslashes are escaped so a path like `O'Brien` or a Windows path can't break
    // out of the literal. (Build dirs are tame, but keep it injection-safe.)
    let escaped = dir
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('\'', "\\'");
    let code = format!("import os as _qmd_os; _qmd_os.chdir('{escaped}'); del _qmd_os");
    if let Err(e) = kernel.execute(&code).await {
        crate::log::warn(&format!(
            "warm-pool: could not set kernel cwd to {} ({e}); using daemon cwd",
            dir.display()
        ));
    }
}

/// Decide the half-open range `[shared, run_end)` of cells that must execute this
/// run. `ran` is the cumulative hashes the warm kernel has already executed
/// (contiguous from cell 0; empty when cold); `hashes` are this run's per-cell
/// keys; `known(i)` reports whether cell `i`'s output is available without running
/// it (a disk-cache hit not opted out).
///
///   - `shared` = longest common prefix of `ran` and `hashes`, capped at the first
///     `#| cache: false` cell: the kernel holds this prefix's state, so those cells
///     restore (never re-run). A non-cacheable cell must always re-execute, so it
///     (and everything after) is kept out of the warm prefix.
///   - cells `[shared, run_end)` execute; `run_end` is one past the last cell whose
///     output we don't already have. A *known* cell inside this range still runs —
///     the kernel needs its state to reach an unknown cell after it.
///   - cells `[run_end, len)` are all known and restore from the disk cache; their
///     kernel state is never needed (nothing after them runs).
///
/// The safety properties fall out of this: a warm session re-runs only the changed
/// cell + downstream; a cold start with everything known runs *nothing* (instant
/// replay, kernel never booted); a cold start with any change runs from the first
/// cell whose state the kernel lacks (kernel variable state is never faked). Pure,
/// so the granularity is unit-testable without a kernel.
fn plan(
    ran: &[String],
    hashes: &[String],
    known: impl Fn(usize) -> bool,
    cacheable: impl Fn(usize) -> bool,
) -> (usize, usize) {
    let lcp = (0..hashes.len())
        .take_while(|&i| ran.get(i) == Some(&hashes[i]))
        .count();
    // A `#| cache: false` cell always re-executes, so the warm prefix can't include
    // it (or anything after it): otherwise an unchanged non-deterministic cell would
    // be replayed from the kernel's prior in-memory output instead of re-run.
    let first_uncacheable = (0..hashes.len())
        .find(|&i| !cacheable(i))
        .unwrap_or(hashes.len());
    let shared = lcp.min(first_uncacheable);
    let mut run_end = shared;
    for i in shared..hashes.len() {
        if !known(i) {
            run_end = i + 1;
        }
    }
    (shared, run_end)
}

/// Prepend a `#| fig-export:` trigger to a Python cell's code so the kernel writes
/// the cell's figure to the requested file(s) (print-clean) the moment it's
/// displayed. A comma-separated list exports to several files at once (e.g. a vector
/// `.pdf` plus a raster `.png`). The `_qmd_export` hook is defined in the Python
/// preamble ([`crate::kernel`]); `install=True` makes it idempotently install the
/// figure wrap even for cells that produce a figure without naming matplotlib.
/// Returns the code unchanged when there's nothing to export.
fn export_wrapped(code: &str, fig_export: Option<&str>) -> String {
    let Some(spec) = fig_export else {
        return code.to_string();
    };
    let paths: Vec<String> = spec
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(py_str_literal)
        .collect();
    if paths.is_empty() {
        return code.to_string();
    }
    format!("_qmd_export([{}], install=True)\n{code}", paths.join(", "))
}

/// Quote a path as a Python single-quoted string literal, escaping backslashes and
/// quotes so a path with spaces or odd characters survives being embedded in the
/// prepended `_qmd_export([...])` call.
fn py_str_literal(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}'")
}

/// Whether an output must not be cached: any execution error (a cell error, a
/// timeout, or the mid-run kernel-died marker — all rendered as a `tali-error` block),
/// so a transient failure is never replayed and the cell re-runs next time. Matches
/// the emitted `class="tali-error"` rather than a bare substring, so a *successful*
/// cell whose output merely prints the text "tali-error" still caches. Also refuses to
/// cache an output the kernel *truncated* at the size cap: if the cell completes
/// cleanly (no KeyboardInterrupt error) the truncated result would otherwise be frozen
/// and replayed silently. The marker text comes from `kernel.rs`'s output caps.
fn is_uncacheable(output: &str) -> bool {
    output.contains("class=\"tali-error\"") || output.contains("taliesin: output truncated")
}

/// A stable identity for a language's interpreter, used to seed the cumulative
/// hash chain so a different interpreter (or an upgraded one) can't serve outputs
/// it didn't compute. Runs `<program> --version` once and memoizes the result per
/// `(lang, program)` for the process; if that fails (e.g. the interpreter isn't
/// installed), falls back to the program path so the id is still stable.
fn interp_id(lang: &str, program: &Path) -> String {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let key = format!("{lang}\u{0}{}", program.display());
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(id) = cache.lock().get(&key) {
        return id.clone();
    }
    let version = std::process::Command::new(program)
        .arg("--version")
        .output()
        .ok()
        .map(|o| {
            // Python prints to stdout, some tools to stderr; take whichever is set.
            let bytes = if o.stdout.is_empty() {
                o.stderr
            } else {
                o.stdout
            };
            String::from_utf8_lossy(&bytes)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .unwrap_or_default();
    let id = format!("{lang}::{}::{version}", program.display());
    cache.lock().insert(key, id.clone());
    id
}

/// A visible "this cell could not run" diagnostic, spliced where a boot-failed (or
/// otherwise kernel-unavailable) cell's output would go. Without it the output `<div>`
/// is simply absent — a silent drop — because a build emits no websocket diagnostic
/// (only the live preview's status banner would show it). Carries `tali-error` so it is
/// styled as an error AND treated as uncacheable (never persisted to the freeze cache).
/// The last kernel error (e.g. a ZMQ "address already in use" from a port-allocation
/// race under concurrent starts) is appended when known, so the page names *why*.
fn kernel_unavailable_html(lang: &str, last_error: Option<&str>) -> String {
    let detail = match last_error {
        Some(e) if !e.is_empty() => format!(" ({})", esc(e)),
        _ => String::new(),
    };
    format!(
        "<pre class=\"tali-error\">{} kernel unavailable; this cell did not execute{detail}</pre>",
        esc(lang)
    )
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
    let inner = if let Some(fig) = &cell.figure {
        figure_wrap(fig, inner)
    } else if let Some(tbl) = &cell.table {
        table_wrap(tbl, inner)
    } else {
        inner.to_string()
    };
    let html = format!(
        "<div class=\"tali-output\" data-block-id=\"{id}\" data-sourcepos=\"{}\"{source_file_attr}>{inner}</div>",
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
        "<figure{id_attr} class=\"tali-figure tali-figure-center\">{inner}\
         <figcaption>{figcap}</figcaption></figure>"
    )
}

/// Inject a numbered `<caption>` (and the `#tbl-` anchor) into a cell's executed
/// table output so `@tbl-x` resolves to "Table N". Finds the first `<table>` in the
/// output; if there is none (the cell produced something that isn't a table), the
/// output is returned unchanged.
fn table_wrap(tbl: &CellTable, inner: &str) -> String {
    let Some(start) = inner.find("<table") else {
        return inner.to_string();
    };
    let Some(rel_gt) = inner[start..].find('>') else {
        return inner.to_string();
    };
    let gt = start + rel_gt + 1;
    let id_attr = tbl
        .anchor
        .as_deref()
        .map(|a| format!(" id=\"{}\"", esc(a)))
        .unwrap_or_default();
    let caption = tbl.caption.as_deref().unwrap_or("").trim();
    let sep = if caption.is_empty() { "" } else { ": " };
    let open = inner[start..gt].replacen("<table", &format!("<table{id_attr}"), 1);
    format!(
        "{}{open}<caption>Table&nbsp;{}{sep}{}</caption>{}",
        &inner[..start],
        tbl.number,
        esc(caption),
        &inner[gt..],
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
    use taliesin_core::render::Cell;

    fn cell(id: &str) -> CellRef {
        CellRef {
            block_index: 0,
            id: id.to_string(),
            code: "print(1)".into(),
            sourcepos: "5:1-7:3".into(),
            source_file: None,
            figure: None,
            table: None,
            include: true,
            cache: true,
            fig_export: None,
        }
    }

    fn python_cell_block(id: &str) -> Block {
        Block {
            id: id.to_string(),
            sourcepos: "1:1-1:9".into(),
            source_file: None,
            html: "<pre data-block-id=\"x\">print(1)</pre>".into(),
            cell: Some(Cell {
                lang: "python".into(),
                code: "print(1)".into(),
                figure: None,
                table: None,
                echo: true,
                include: true,
                cache: true,
                fig_export: None,
                js: Default::default(),
            }),
        }
    }

    #[tokio::test]
    async fn no_exec_renders_cells_as_source_and_never_touches_a_kernel() {
        // With `--no-exec`, `run` must return the blocks exactly as rendered (the
        // cell shows as source, no output block appended) and must not even attempt
        // to start a kernel — so there's no "kernel unavailable" diagnostic either.
        // Both assertions make this fail without the guard regardless of whether a
        // working kernel happens to be installed in the test environment.
        let mut ex = Executor::new();
        ex.no_exec = true;
        let blocks = vec![python_cell_block("b-1")];
        let out = ex.run(blocks.clone()).await;
        assert_eq!(out.len(), 1, "no output block should be appended");
        assert_eq!(out[0].html, blocks[0].html, "the cell stays as source");
        assert!(
            ex.diagnostic().is_none(),
            "no_exec is deliberate, not a kernel failure -> no diagnostic"
        );
    }

    fn python_cell_block_with(id: &str, code: &str) -> Block {
        let mut b = python_cell_block(id);
        if let Some(c) = b.cell.as_mut() {
            c.code = code.to_string();
        }
        b
    }

    #[test]
    fn progress_sink_streams_executing_then_idle_for_a_3_cell_doc() {
        // The exec→client progress seam Task 1 introduces: with a wired `ProgressSink`,
        // running a 3-cell python doc must push `build-state` messages whose `ran`
        // climbs 1→2→3 (== total) under "executing", then a final "idle" with
        // ran == total. Gated on a live kernel (the same env/skip the other
        // kernel-exercising tests use): without `TALIESIN_PYTHON` it reports ok WITHOUT
        // exercising a kernel — the serialization is covered unconditionally in
        // `protocol.rs`.
        if std::env::var_os("TALIESIN_PYTHON").is_none() {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with ipykernel to \
                 exercise build-state progress; this run did not."
            );
            return;
        }

        use std::sync::Mutex as StdMutex;
        let captured: Arc<StdMutex<Vec<serde_json::Value>>> = Arc::new(StdMutex::new(Vec::new()));
        let sink: ProgressSink = {
            let captured = captured.clone();
            Some(Arc::new(move |m: String| {
                captured
                    .lock()
                    .unwrap()
                    .push(serde_json::from_str(&m).unwrap());
            }))
        };

        let mut ex = Executor::new();
        ex.set_progress(sink, Some("ch1.tmd".into()));
        // Distinct code per cell so each is a genuine cell with its own cache key.
        let blocks = vec![
            python_cell_block_with("b-1", "a = 1"),
            python_cell_block_with("b-2", "b = 2"),
            python_cell_block_with("b-3", "print(a + b)"),
        ];

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _ = ex.run(blocks).await;
        });

        if ex.diagnostic().is_some() {
            // No working python kernel here — can't exercise execution progress.
            return;
        }

        let msgs = captured.lock().unwrap();
        // Every `build-state` is well-formed for this page. (The same sink now also
        // carries `cell-state` messages — covered by the cell-state tests below — so
        // this is scoped to build-state rather than asserting over every message.)
        for v in msgs.iter().filter(|v| v["type"] == "build-state") {
            assert_eq!(v["page"], "ch1.tmd");
            assert_eq!(v["lang"], "python");
        }
        // The "executing" `ran` values climb 1, 2, 3 (one per cell), each ≤ total.
        let executing: Vec<u64> = msgs
            .iter()
            .filter(|v| v["phase"] == "executing")
            .map(|v| {
                assert_eq!(v["total"], 3, "total should be the run count: {v}");
                let ran = v["ran"].as_u64().unwrap();
                assert!((1..=3).contains(&ran), "ran out of [1,total]: {v}");
                ran
            })
            .collect();
        assert_eq!(
            executing,
            vec![1, 2, 3],
            "executing `ran` must climb monotonically up to total: {executing:?}"
        );
        // The final message is an `idle` with ran == total.
        let last = msgs.last().expect("at least one build-state was emitted");
        assert_eq!(last["phase"], "idle", "build must settle on idle: {last}");
        assert_eq!(last["ran"], 3, "idle must report ran == total: {last}");
        assert_eq!(last["total"], 3, "idle must report ran == total: {last}");
    }

    /// A `ProgressSink` that records every emitted message as parsed JSON, plus the
    /// captured buffer, for the cell-state tests below.
    fn capturing_sink() -> (ProgressSink, Arc<std::sync::Mutex<Vec<serde_json::Value>>>) {
        let captured: Arc<std::sync::Mutex<Vec<serde_json::Value>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink: ProgressSink = {
            let captured = captured.clone();
            Some(Arc::new(move |m: String| {
                captured
                    .lock()
                    .unwrap()
                    .push(serde_json::from_str(&m).unwrap());
            }))
        };
        (sink, captured)
    }

    /// The ordered `cell-state` states emitted for `cell_id`, in emission order.
    fn cell_states(msgs: &[serde_json::Value], cell_id: &str) -> Vec<String> {
        msgs.iter()
            .filter(|v| v["type"] == "cell-state" && v["cell_id"] == cell_id)
            .map(|v| v["state"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn cell_state_emits_queued_running_done_per_cell_and_error_for_failures() {
        // The per-cell honest-state seam: each executed cell must emit its states in
        // the monotonic order queued → running → done (never `running` without a
        // prior `queued`), and a cell whose execution errors must end on `error`, not
        // `done`. Kernel-gated like the other exec tests: without `TALIESIN_PYTHON`
        // it reports ok without exercising a kernel — `cell_state` serialization is
        // covered unconditionally in `protocol.rs`.
        if std::env::var_os("TALIESIN_PYTHON").is_none() {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with ipykernel to \
                 exercise cell-state progress; this run did not."
            );
            return;
        }

        let (sink, captured) = capturing_sink();
        let mut ex = Executor::new();
        ex.set_progress(sink, Some("ch1.tmd".into()));
        // Cell b-3 raises, so its output is a `tali-error` block (uncacheable) → `error`.
        let blocks = vec![
            python_cell_block_with("b-1", "a = 1"),
            python_cell_block_with("b-2", "b = 2"),
            python_cell_block_with("b-3", "raise ValueError('boom')"),
        ];

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _ = ex.run(blocks).await;
        });

        if ex.diagnostic().is_some() {
            // No working python kernel here — can't exercise execution progress.
            return;
        }

        let msgs = captured.lock().unwrap();
        // Every cell-state is well-formed and tagged with this page.
        for v in msgs.iter().filter(|v| v["type"] == "cell-state") {
            assert_eq!(v["page"], "ch1.tmd", "cell-state page wrong: {v}");
        }
        // The two clean cells go queued → running → done, in that order.
        for id in ["b-1", "b-2"] {
            assert_eq!(
                cell_states(&msgs, id),
                vec!["queued", "running", "done"],
                "cell {id} must be monotonic queued→running→done",
            );
        }
        // The erroring cell ends on `error`, not `done`, after queued → running.
        assert_eq!(
            cell_states(&msgs, "b-3"),
            vec!["queued", "running", "error"],
            "an erroring cell must end on `error`, never `done`",
        );
    }

    #[test]
    fn cell_state_reuses_earlier_cells_and_reruns_only_the_edited_last_cell() {
        // After editing only the last cell, the warm prefix must restore (each earlier
        // cell emits exactly `done`, no running), and only the edited cell re-runs
        // (queued → running → done). This pins that emission tracks the `plan()` zones
        // — observation of what actually ran, not a blanket "everything ran".
        if std::env::var_os("TALIESIN_PYTHON").is_none() {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with ipykernel to \
                 exercise cell-state warm reuse; this run did not."
            );
            return;
        }

        let mut ex = Executor::new();
        ex.set_progress(None, Some("ch1.tmd".into()));
        let base = vec![
            python_cell_block_with("b-1", "a = 1"),
            python_cell_block_with("b-2", "b = 2"),
            python_cell_block_with("b-3", "print(a + b)"),
        ];

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // First run warms the kernel for [b-1, b-2, b-3].
            let _ = ex.run(base.clone()).await;
        });

        if ex.diagnostic().is_some() {
            // No working python kernel here — can't exercise warm reuse.
            return;
        }

        // Second run: edit only the last cell, capturing this pass's cell-states.
        let (sink, captured) = capturing_sink();
        ex.set_progress(sink, Some("ch1.tmd".into()));
        let edited = vec![
            python_cell_block_with("b-1", "a = 1"),
            python_cell_block_with("b-2", "b = 2"),
            python_cell_block_with("b-3", "print(a * b)"),
        ];
        rt.block_on(async {
            let _ = ex.run(edited).await;
        });

        let msgs = captured.lock().unwrap();
        // Earlier cells restore from the warm prefix: exactly one `done`, no `running`.
        for id in ["b-1", "b-2"] {
            assert_eq!(
                cell_states(&msgs, id),
                vec!["done"],
                "warm-prefix cell {id} must restore as `done` without re-running",
            );
        }
        // Only the edited last cell re-runs: queued → running → done.
        assert_eq!(
            cell_states(&msgs, "b-3"),
            vec!["queued", "running", "done"],
            "the edited last cell must re-run queued→running→done",
        );
    }

    /// The ordered `build-state` phases emitted, in emission order.
    fn build_phases(msgs: &[serde_json::Value]) -> Vec<String> {
        msgs.iter()
            .filter(|v| v["type"] == "build-state")
            .map(|v| v["phase"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn emit_cell_errors_marks_only_the_given_range() {
        // The seam both failure paths reuse: cells that could not run (boot failed, or
        // the kernel died before reaching them) must get a terminal `error` so they
        // never stay `queued`/spinning. Pure (no kernel): a fast, always-run guard that
        // the run-range → error mapping is exactly the cells in the half-open range.
        let (sink, captured) = capturing_sink();
        let cells = vec![cell("b-0"), cell("b-1"), cell("b-2"), cell("b-3")];
        emit_cell_errors(&sink, Some("ch1.tmd"), &cells, 1..3);
        let msgs = captured.lock().unwrap();
        let errored: Vec<&str> = msgs
            .iter()
            .filter(|v| v["type"] == "cell-state" && v["state"] == "error")
            .map(|v| v["cell_id"].as_str().unwrap())
            .collect();
        assert_eq!(
            errored,
            vec!["b-1", "b-2"],
            "exactly the half-open run range must be marked `error`",
        );
        // Page is carried; out-of-range cells emit nothing here.
        for v in msgs.iter() {
            assert_eq!(v["page"], "ch1.tmd");
        }
        assert_eq!(msgs.len(), 2, "only the two in-range cells emit: {msgs:?}");
    }

    #[test]
    fn all_cached_rebuild_emits_one_idle_and_no_running_or_error() {
        // Brief Step 2: a fully-cached rebuild (warm executor, nothing changed →
        // to_run == 0) must settle on a single `idle` and never claim a cell ran:
        // zero `running`/`error` cell-states, zero `warming-kernel`/`executing`.
        if std::env::var_os("TALIESIN_PYTHON").is_none() {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with ipykernel to \
                 exercise the all-cached rebuild; this run did not."
            );
            return;
        }

        let mut ex = Executor::new();
        ex.set_progress(None, Some("ch1.tmd".into()));
        let doc = vec![
            python_cell_block_with("b-1", "a = 1"),
            python_cell_block_with("b-2", "b = 2"),
            python_cell_block_with("b-3", "print(a + b)"),
        ];
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _ = ex.run(doc.clone()).await; // warm the kernel
        });
        if ex.diagnostic().is_some() {
            return; // no working python kernel here
        }

        // Re-run the identical doc against the warm executor: to_run == 0.
        let (sink, captured) = capturing_sink();
        ex.set_progress(sink, Some("ch1.tmd".into()));
        rt.block_on(async {
            let _ = ex.run(doc).await;
        });

        let msgs = captured.lock().unwrap();
        let phases = build_phases(&msgs);
        assert_eq!(
            phases,
            vec!["idle"],
            "an all-cached rebuild must emit exactly one `idle`: {phases:?}",
        );
        let cell_msgs: Vec<&str> = msgs
            .iter()
            .filter(|v| v["type"] == "cell-state")
            .map(|v| v["state"].as_str().unwrap())
            .collect();
        assert!(
            cell_msgs.iter().all(|s| *s == "done"),
            "no cell ran, so no `running`/`error`/`queued` — only `done`: {cell_msgs:?}",
        );
    }

    #[test]
    fn boot_failure_emits_error_build_state_and_errors_run_range_cells() {
        // Fix 2: when the kernel BOOT fails (to_run > 0 but no live kernel), the build
        // must be honest — `build-state` `error` (never a trailing clean `idle`
        // claiming success), and every run-range cell ends on `error`, not stuck at
        // `queued`. Forced deterministically (no real kernel needed) by pointing the
        // interpreter at a path that can't start.
        let (sink, captured) = capturing_sink();
        let mut ex = Executor::new();
        ex.python = PathBuf::from("/nonexistent/taliesin-no-such-python");
        ex.set_progress(sink, Some("ch1.tmd".into()));
        let doc = vec![
            python_cell_block_with("b-1", "a = 1"),
            python_cell_block_with("b-2", "b = 2"),
        ];
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _ = ex.run(doc).await;
        });
        assert!(
            ex.diagnostic().is_some(),
            "the bogus interpreter must fail to boot (precondition)",
        );

        let msgs = captured.lock().unwrap();
        let phases = build_phases(&msgs);
        // Warming is announced, then the build settles on `error` — never `idle`.
        assert!(
            phases.last() == Some(&"error".to_string()),
            "a failed boot must settle on `error`, not `idle`: {phases:?}",
        );
        assert!(
            !phases.contains(&"idle".to_string()),
            "a failed boot must not emit a misleading clean `idle`: {phases:?}",
        );
        // No run-range cell is left stuck at `queued`: each ends on `error`.
        for id in ["b-1", "b-2"] {
            let states = cell_states(&msgs, id);
            assert_eq!(
                states.last(),
                Some(&"error".to_string()),
                "run-range cell {id} must end on `error`, not stay queued: {states:?}",
            );
        }
    }

    #[test]
    fn boot_failure_renders_a_visible_diagnostic_not_a_silent_drop() {
        // Silent-output-drop fix: when the kernel can't boot, a cell that was supposed
        // to RUN must emit a VISIBLE diagnostic block in the output — never vanish
        // silently. In a build there is no websocket, so the `cell_state`/`build_state`
        // `error` signals never reach the HTML; without an in-page marker a build ships
        // a page missing computed output with no hint why. (Root cause: a ZMQ
        // port-allocation race made kernels fail to boot under concurrent builds.)
        let mut ex = Executor::new();
        ex.python = PathBuf::from("/nonexistent/taliesin-no-such-python");
        let doc = vec![python_cell_block_with("b-1", "print('hello')\n1 + 1")];
        let rt = tokio::runtime::Runtime::new().unwrap();
        let blocks = rt.block_on(async { ex.run(doc).await });
        let out = blocks
            .iter()
            .find(|b| b.id == "b-1-out")
            .expect("a cell that could not run must still emit an output block (loud, not silent)");
        assert!(
            out.html.contains("class=\"tali-error\""),
            "the output block must be a visible diagnostic, got: {}",
            out.html
        );
        assert!(
            out.html.to_lowercase().contains("kernel"),
            "the diagnostic should name the unavailable kernel, got: {}",
            out.html
        );
    }

    #[test]
    fn pooled_kernel_serves_cells_without_a_long_warming_state() {
        // An Executor wired to a warm pool draws its python kernel
        // from the pool and runs a cell to a correct result, and — because a pooled
        // kernel is near-instant — never presents a `warming-kernel` build-state. The
        // cold path *does* emit `warming-kernel`; here it must be absent. Kernel-gated:
        // the warm pool needs a real `TALIESIN_PYTHON` with ipykernel.
        let Some(py) = std::env::var_os("TALIESIN_PYTHON").map(PathBuf::from) else {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with ipykernel to \
                 exercise the warm-pool exec path; this run did not."
            );
            return;
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let pool = Arc::new(crate::warm_pool::WarmPool::new(&py, 2).await);
            assert!(
                pool.is_warm() && pool.capacity() >= 1,
                "a real python must boot a warm forkserver with capacity >= 1"
            );
            // Let the pool pre-warm at least one kernel so `take` is a hit, not a miss
            // (a miss would legitimately fall back to a cold start + warming signal).
            let mut ready = false;
            for _ in 0..100 {
                if pool.ready_len().await > 0 {
                    ready = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            assert!(ready, "warm pool should pre-warm a kernel within 10s");

            let (sink, captured) = capturing_sink();
            let mut ex = Executor::new();
            ex.set_progress(sink, Some("ch1.tmd".into()));
            ex.set_warm_pool(Some(pool));
            // A cell with a deterministic textual result, so we can prove the pooled
            // kernel actually executed it.
            let blocks = vec![python_cell_block_with("b-1", "print(6 * 7)")];
            let _ = ex.run(blocks).await;

            assert!(
                ex.diagnostic().is_none(),
                "the pooled kernel must be live (no boot diagnostic)"
            );
            // Output proves the pooled kernel ran the cell.
            let msgs = captured.lock().unwrap();
            let phases = build_phases(&msgs);
            assert!(
                !phases.contains(&"warming-kernel".to_string()),
                "a warm-pool hit must NOT present a `warming-kernel` state: {phases:?}"
            );
            // It still reaches `executing` then settles on `idle` with ran == total.
            assert!(
                phases.contains(&"executing".to_string()),
                "the pooled build should still emit `executing`: {phases:?}"
            );
            assert_eq!(
                phases.last(),
                Some(&"idle".to_string()),
                "the pooled build must settle on `idle`: {phases:?}"
            );
        });
    }

    #[tokio::test]
    async fn cells_run_in_the_document_directory() {
        // A cell's relative file write must land in the executor's working dir (the
        // document's own directory), not wherever the server process was launched.
        // This is what keeps generated media (audio, `#| fig-export:` figures) beside
        // the source instead of cluttering the repo root. Skipped (not failed) when no
        // Python kernel is installed, so it stays green in a kernel-less CI.
        let dir = std::env::temp_dir().join(format!("taliesin-cwd-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut blk = python_cell_block("b-cwd");
        if let Some(c) = blk.cell.as_mut() {
            c.code = "open('tali-cwd-marker.txt', 'w').close()".into();
        }
        let mut ex = Executor::new().in_dir(&dir);
        let _ = ex.run(vec![blk]).await;
        if ex.diagnostic().is_some() {
            // No working Python kernel here — can't exercise the behavior.
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let canon = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        let landed = canon.join("tali-cwd-marker.txt").exists();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            landed,
            "the cell's relative write should land in the document dir, not the cwd"
        );
    }

    fn h(ks: &[&str]) -> Vec<String> {
        ks.iter().map(|s| s.to_string()).collect()
    }

    /// `plan` with `ran` = the warm kernel's executed keys and `cur` = this run's
    /// per-cell keys; cells whose key is in `disk` are disk-cache hits. All cells
    /// are cacheable here; `run_plan_nc` covers `#| cache: false`.
    fn run_plan(ran: &[&str], cur: &[&str], disk: &[&str]) -> (usize, usize) {
        let cur = h(cur);
        plan(&h(ran), &cur, |i| disk.contains(&cur[i].as_str()), |_| true)
    }

    /// Like `run_plan`, but `nocache` lists the cell indices marked `#| cache: false`.
    fn run_plan_nc(ran: &[&str], cur: &[&str], disk: &[&str], nocache: &[usize]) -> (usize, usize) {
        let cur = h(cur);
        plan(
            &h(ran),
            &cur,
            |i| disk.contains(&cur[i].as_str()),
            |i| !nocache.contains(&i),
        )
    }

    #[test]
    fn plan_cache_false_cell_always_reruns() {
        // Warm kernel ran [a,b,c], nothing changed, but a `cache: false` cell can't
        // be served from the warm prefix: it (and everything after) re-runs.
        assert_eq!(
            run_plan_nc(&["a", "b", "c"], &["a", "b", "c"], &[], &[1]),
            (1, 3)
        );
        // `cache: false` on the first cell -> re-run everything.
        assert_eq!(
            run_plan_nc(&["a", "b", "c"], &["a", "b", "c"], &[], &[0]),
            (0, 3)
        );
        // `cache: false` on the last cell only -> warm prefix [a,b], re-run just c.
        assert_eq!(
            run_plan_nc(&["a", "b", "c"], &["a", "b", "c"], &[], &[2]),
            (2, 3)
        );
    }

    #[test]
    fn plan_warm_session_reruns_only_changed_cell_and_downstream() {
        // Warm kernel ran [a,b,c]; nothing changed -> run nothing (full reuse).
        assert_eq!(run_plan(&["a", "b", "c"], &["a", "b", "c"], &[]), (3, 3));
        // Edit the middle cell: its key + downstream keys move; re-run from there.
        assert_eq!(run_plan(&["a", "b", "c"], &["a", "X", "Y"], &[]), (1, 3));
        // Edit only the last cell: re-run just it.
        assert_eq!(run_plan(&["a", "b", "c"], &["a", "b", "Z"], &[]), (2, 3));
        // Append a cell: run only the new trailing cell.
        assert_eq!(run_plan(&["a", "b"], &["a", "b", "c"], &[]), (2, 3));
        // Remove the trailing cell: survivors stay warm, run nothing.
        assert_eq!(run_plan(&["a", "b", "c"], &["a", "b"], &[]), (2, 2));
        // Warm prefix [a,b] + a fully disk-cached tail [c,d] -> still run nothing.
        assert_eq!(
            run_plan(&["a", "b"], &["a", "b", "c", "d"], &["c", "d"]),
            (2, 2)
        );
    }

    #[test]
    fn plan_cold_start_replays_only_when_every_cell_is_known() {
        // Cold kernel, every cell on disk -> run NOTHING (instant replay, the
        // kernel never boots). The headline persistent-cache win.
        assert_eq!(run_plan(&[], &["a", "b", "c"], &["a", "b", "c"]), (0, 0));
        // Cold kernel, nothing cached -> run everything from the start.
        assert_eq!(run_plan(&[], &["a", "b", "c"], &[]), (0, 3));
        // Cold start, only the last cell changed (missing from disk): we lack kernel
        // state for the prefix, so we must re-run the whole document.
        assert_eq!(run_plan(&[], &["a", "b", "Z"], &["a", "b"]), (0, 3));
        // Cold start, a middle cell missing: run through it; the cached tail after
        // the last miss restores without running (its state is never needed).
        assert_eq!(run_plan(&[], &["a", "X", "c"], &["a", "c"]), (0, 2));
    }

    #[test]
    fn output_block_keys_id_to_cell_and_carries_clickto_source() {
        let b = output_block(&cell("b-abc"), "<pre>1</pre>");
        // id derived from the cell so the output swaps in place when it re-runs.
        assert_eq!(b.id, "b-abc-out");
        // click-to-source points back at the cell's own source position.
        assert_eq!(b.sourcepos, "5:1-7:3");
        // ...and that position is reverse-sync-valid (`L:C-L:C`), so an executed-cell
        // output is reachable by cursor sync just like a static block. The no-kernel
        // corpus test can't produce executed outputs, so this is where that's pinned.
        let part_ok = |p: &str| {
            let mut it = p.split(':');
            matches!((it.next(), it.next(), it.next()), (Some(l), Some(c), None)
                if !l.is_empty() && l.bytes().all(|x| x.is_ascii_digit())
                    && !c.is_empty() && c.bytes().all(|x| x.is_ascii_digit()))
        };
        assert!(
            b.sourcepos
                .split_once('-')
                .is_some_and(|(a, z)| part_ok(a) && part_ok(z)),
            "output sourcepos must match the reverse-sync format L:C-L:C: {}",
            b.sourcepos
        );
        assert!(b.cell.is_none(), "an output block is not itself a cell");
        assert!(
            b.html.contains("class=\"tali-output\"")
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
        c.source_file = Some("posts/p&q.tmd".into());
        let b = output_block(&c, "x");
        assert_eq!(b.source_file.as_deref(), Some("posts/p&q.tmd"));
        assert!(
            b.html.contains("data-source-file=\"posts/p&amp;q.tmd\""),
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
            html.starts_with("<figure id=\"fig-cov\" class=\"tali-figure tali-figure-center\">"),
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
            html.starts_with("<figure class=\"tali-figure"),
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
        // the figure nests inside the tali-output wrapper, anchored for @fig-x.
        assert!(b.html.contains("class=\"tali-output\""), "{}", b.html);
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
