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
//!     makes the naive per-cell `cache` approach fragile), so a cold start can only skip work
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
use crate::interpreter::{Lang, Resolved};
use crate::kernel::{Kernel, KernelSpec, render_outputs};

/// After a failed kernel start, wait at least this long before retrying — long
/// enough that a genuinely missing/bad interpreter doesn't re-hang every save,
/// short enough that fixing `TALIESIN_PYTHON`/`TALIESIN_R` self-heals within a few
/// saves.
const KERNEL_RETRY_AFTER: Duration = Duration::from_secs(20);

/// Marks a `tali-error` block **the executor wrote itself**, about a cell that never ran
/// (or never finished), as opposed to a traceback the interpreter raised about code that
/// did. The two are deliberately the same HTML shape — a `tali-error` pre inside a
/// `tali-output` div, so both are styled as errors and neither is ever cached — which left
/// the only classifier keying on shape and reporting a missing interpreter to the console
/// as "code cell raised an uncaught exception; its traceback is baked into the output"
/// (AP11-1: both claims false). An extra attribute rather than an extra class, because
/// several checks here and in `build.rs` match `class="tali-error"` literally, and
/// uncacheability rides on one of them.
///
/// The value carries WHICH of the three (see the `NOT_RUN_*` consts) so the console can be
/// specific without parsing the diagnostic's prose back out of the HTML — which is not
/// reachable anyway once a `#| label:` cell wraps the block in a `<figure>`.
pub(crate) const NOT_RUN_ATTR: &str = "data-tali-not-run";
/// No kernel could be started for the cell's language (a missing/bad interpreter, a failed
/// boot). The most likely setup failure there is.
pub(crate) const NOT_RUN_UNAVAILABLE: &str = "kernel-unavailable";
/// The kernel died mid-run, so this cell was skipped without being sent.
pub(crate) const NOT_RUN_DIED: &str = "kernel-died";
/// The execute request itself failed (a ZMQ/protocol error, an interrupt), so the
/// interpreter returned no result.
pub(crate) const NOT_RUN_REQUEST: &str = "request-failed";
/// The cell hit a liveness cap (silence, or an opt-in wall-clock one) and was
/// interrupted, so it produced no result.
/// Distinct from [`NOT_RUN_REQUEST`] because the fix is different and knowable: raise the
/// cap or shorten the cell, not repair the transport.
pub(crate) const NOT_RUN_TIMEOUT: &str = "timeout";

/// The `data-tali-not-run="<kind>"` attribute text, leading space included.
pub(crate) fn not_run_mark(kind: &str) -> String {
    format!(" {NOT_RUN_ATTR}=\"{kind}\"")
}

/// Console warnings already emitted this process, so a fact that cannot change between
/// pages is stated once. Keyed on the whole message, which already carries the language,
/// the interpreter path and the interpreter's own error — so a *different* failure is
/// still announced, and only a verbatim repeat is dropped.
static ANNOUNCED: std::sync::Mutex<Option<std::collections::HashSet<String>>> =
    std::sync::Mutex::new(None);

/// `log::warn` the message unless this process already has. See [`ANNOUNCED`].
fn announce_once(message: &str) {
    let mut guard = match ANNOUNCED.lock() {
        Ok(g) => g,
        // A poisoned lock must not silence a warning: say it and move on.
        Err(p) => p.into_inner(),
    };
    if guard
        .get_or_insert_with(Default::default)
        .insert(message.to_string())
    {
        crate::log::warn(message);
    }
}

/// Forget what has been announced, so the same failure is stated again after a deliberate
/// retry. Called by "Restart kernel": an author who fixed `TALIESIN_PYTHON` and asked for a
/// restart is owed the answer, even when it is the same answer as before.
pub(crate) fn reset_announcements() {
    if let Ok(mut g) = ANNOUNCED.lock() {
        *g = None;
    }
}

/// Shown for cells skipped after the kernel died mid-run (see `compute_outputs`):
/// they didn't execute, and the next rebuild respawns the kernel and re-runs them.
pub(crate) const KERNEL_DIED_HTML: &str = "<pre class=\"tali-error\" data-tali-not-run=\"kernel-died\">kernel exited before this cell ran; it will re-run on the next save</pre>";

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
            crate::protocol::cell_state(page, &cell.id, "error", None, None, None),
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
pub(crate) fn kernel_lang(lang: &str) -> Option<&'static str> {
    match lang {
        "python" => Some("python"),
        "r" => Some("r"),
        _ => None,
    }
}

/// Whether a cell block may be run under the `#| trace: true` harness. Two conditions,
/// and the language one is load-bearing: `trace_py::wrap_traced` splices the author's code
/// into a **Python** harness, while everything upstream of here is language-blind
/// (`emit.rs` stamps `data-tali-trace="1"` on any cell carrying the option, and `divs.rs`
/// counts any such cell as the `.debug` div's traced one). Without the `python` check an
/// `{r}` cell was handed `def _tali_debug_run(_src):` to parse and the reader got
/// `Error in parse(text = input): <text>:2:5: unexpected input` where a stepper belonged.
/// `{js}` (the only other language `trace:` supports) is captured in the browser and never
/// reaches this executor, so `{r}` was the sole reachable hole. The author-facing half of
/// this fix is `render::validate::validate_trace_language`, which says so at the source
/// line, so this gate is silent only because the warning is not.
fn is_traced(lang: &str, html: &str) -> bool {
    lang == "python" && html.contains("data-tali-trace=\"1\"")
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
    /// `#| trace: true` inside `::: {.debug}`: run under `trace_py::wrap_traced`
    /// instead of verbatim. Read off the emitted `data-tali-trace="1"` attribute
    /// rather than a `Cell` field: `render::Cell` carries no trace bit of its own
    /// (Task 1 stamped only the `<pre>` attribute), and `divs.rs`'s own
    /// `is_traced_cell` already reads the same attribute for the same reason, so
    /// this mirrors that precedent instead of inventing a second channel for one bit.
    ///
    /// **Python only**, gated by [`is_traced`] above (see its doc comment for why).
    traced: bool,
}

/// A cell the *current warm kernel* has executed: its cumulative cache key and the
/// output it produced. Ordered, contiguous from cell 0 — so it doubles as the
/// "what state does the live kernel hold" record the [`plan`]ner diffs against.
struct Ran {
    hash: String,
    output: String, // inner output HTML (may be empty)
}

/// How one run split between replay and re-execution, summed across languages so the
/// caller can print one legible cache line (DX9): `cached` cells were restored (the warm
/// in-memory prefix + the disk `_freeze` tail), `ran` cells actually executed.
#[derive(Default, Clone, Copy)]
struct CacheTally {
    cached: usize,
    ran: usize,
}

impl std::ops::AddAssign for CacheTally {
    fn add_assign(&mut self, o: Self) {
        self.cached += o.cached;
        self.ran += o.ran;
    }
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
    /// Whether this executor has already logged which interpreter this language runs
    /// (the "which python?" signal). Reset by `restart_kernel` (which clears `langs`),
    /// so a manual restart re-announces.
    announced: bool,
}

pub struct Executor {
    /// The interpreters this executor launches, each as the **whole** resolution record
    /// — path, provenance, and the trail of everything that was considered. Stored as a
    /// `Resolved` rather than a bare path plus a provenance so there is nowhere left in
    /// this file to invent an interpreter: both are produced by
    /// [`crate::interpreter`] and only replaced wholesale by
    /// [`Executor::set_interpreters`]. The trail is what the build's "no kernel
    /// available" failure prints.
    python: Resolved,
    r: Resolved,
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
    ///
    /// **It is not a sanitizer and does not make an untrusted document safe.** It stops
    /// `{python}`/`{r}` cells reaching a live kernel and covers `{js}`, but raw HTML in the
    /// source still passes through verbatim (`emit.rs`), by the documented trust model in
    /// `taliesin_core`'s crate docs: the author owns their own input. There is deliberately
    /// no HTML sanitizer and no CSP (the 2026-07-03 ruling), which the user-facing docs
    /// state outright (`docs/guide/reference/cli.tmd`). This comment used to call the flag
    /// "the safe way to preview a document you don't trust", which contradicted them.
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
    /// The handle an interrupt request uses to stop a run this executor is in the middle of
    /// (see [`crate::run_control`]). Shared with the server's registry; the default is a
    /// private one nobody else holds, which makes every non-server caller (`build`, tests)
    /// behave exactly as before because nothing can ever raise its flag.
    control: Arc<crate::run_control::RunControl>,
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
    /// relative file writes (audio, a `ggsave`/`savefig` figure) land beside
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
        // Delegated, never re-implemented. This used to read `TALIESIN_PYTHON` and fall
        // back to `python3` inline — a second copy of a policy `interpreter.rs` owns,
        // which by construction could not see a `.venv` and so disagreed with every
        // entry point that *did* resolve properly. The cwd is the only project dir a
        // constructor can know; every real entry point still calls `set_interpreters`
        // with the site root immediately after, and now both paths run the same code.
        let cwd = std::path::Path::new(".");
        Self {
            python: crate::interpreter::resolve_python(None, cwd),
            r: crate::interpreter::resolve_r(None, cwd),
            langs: HashMap::new(),
            freeze,
            force_next: false,
            no_exec: exec_disabled(),
            work_dir: None,
            sink: None,
            page: None,
            pool: None,
            control: Arc::default(),
        }
    }

    /// Share this executor's run control with the server's registry, so
    /// `POST /__taliesin/interrupt` can stop a run in flight. Executors that never call
    /// this keep the private default, which nothing else can signal.
    pub(crate) fn set_run_control(&mut self, control: Arc<crate::run_control::RunControl>) {
        self.control = control;
    }

    /// Stream this executor's per-build progress (`build-state` messages) through
    /// `sink`, tagged with the page rel-path `page` (the site server's page key;
    /// `None` for the single-doc server). The server sets this once after creating the
    /// executor. The site `build` path passes a `None` sink (there is no client) but still
    /// sets `page`, so its concurrent per-page `cell k/n` lines can be attributed.
    /// Emission never changes what executes or caches, so freeze determinism is preserved
    /// regardless of the sink. A `&mut self` setter (not a consuming builder) so it can be
    /// applied to a pooled `&mut Executor`.
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

    /// Override the interpreters this executor runs (and the pool warms), with their
    /// provenance for the "which python?" log line. Called once by each build/serve
    /// entry point after resolving `_site.yml`/`.venv`/env. A `&mut self` setter (not a
    /// consuming builder) so a pooled `&mut Executor` can be pointed at the resolved
    /// interpreters. Executors that never call this keep the env/default from `build`.
    pub fn set_interpreters(&mut self, python: Resolved, r: Resolved) {
        self.python = python;
        self.r = r;
    }

    /// The launch spec + interpreter path (for logging) for a language.
    fn spec(&self, lang: &str) -> Option<(KernelSpec, PathBuf)> {
        match lang {
            "python" => Some((
                KernelSpec::python(&self.python.path),
                self.python.path.clone(),
            )),
            "r" => Some((KernelSpec::r(&self.r.path), self.r.path.clone())),
            _ => None,
        }
    }

    /// This executor's resolution record for `lang`, for the surfaces that must explain
    /// a choice rather than just make it (the build's "no kernel available" failure).
    pub fn resolved(&self, lang: Lang) -> &Resolved {
        match lang {
            Lang::Python => &self.python,
            Lang::R => &self.r,
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
            let (var, path) = if *lang == "r" {
                ("TALIESIN_R", self.r.path.display().to_string())
            } else {
                ("TALIESIN_PYTHON", self.python.path.display().to_string())
            };
            Some(Self::kernel_unavailable_message(
                lang,
                &path,
                var,
                s.last_error.as_deref(),
            ))
        })
    }

    /// The build-fatal report: this document had cells to execute, a kernel start was
    /// attempted for their language, and it failed. `None` when nothing needed a kernel
    /// or every kernel started — including the case where every cell replayed from
    /// `_freeze/`, which never attempts a start and so is legitimately not a failure.
    ///
    /// Names what was searched and in what order (the [`Trail`](crate::interpreter::Trail)
    /// recorded by the resolver), because "kernel unavailable" without the search order
    /// leaves the author guessing which of four sources was consulted and which won.
    pub fn kernel_failure_report(&self) -> Option<String> {
        self.langs.iter().find_map(|(lang, s)| {
            if s.kernel.is_some() || s.failed_at.is_none() {
                return None;
            }
            let lang_enum = if *lang == "r" { Lang::R } else { Lang::Python };
            let resolved = self.resolved(lang_enum);
            let var = if *lang == "r" {
                "TALIESIN_R"
            } else {
                "TALIESIN_PYTHON"
            };
            let tried = resolved.path.display();
            let why = s
                .last_error
                .as_deref()
                .map(|e| format!(" ({e})"))
                .unwrap_or_default();
            let order = resolved.trail.report(lang_enum, resolved.provenance);
            let pkg = if *lang == "r" {
                "IRkernel"
            } else {
                "ipykernel"
            };
            let body = format!(
                "no {lang} kernel available, but this document has {lang} cells\n\
                 tried {tried}{why}\n\
                 {lang} interpreter resolution, in order:\n{order}\n\
                 fix: give that interpreter its Jupyter kernel package ({pkg}), or point \
                 {var} / `_site.yml {lang}:` at one that has it. `taliesin doctor` reports \
                 which it is.\n\
                 to render code cells as source on purpose instead, pass --no-exec."
            );
            // Hang every continuation line under `crate::log`'s 10-column tag gutter
            // ("  " + a 7-wide tag + " "), so a multi-line error reads as one block
            // instead of half a message sitting flush against the left margin.
            Some(body.replace('\n', "\n          "))
        })
    }

    /// The shared "kernel unavailable" diagnostic. Pure (so its wording is unit-testable).
    /// The usual cause is a fine interpreter that's just missing the Jupyter kernel package
    /// (`ipykernel`/`IRkernel`), NOT a wrong interpreter path — so name both and route to
    /// `doctor`, which reports exactly which it is (PL6). No "Restart kernel" clause: the
    /// message is shared with headless `build`/`read`/CI, where that dev-menu action doesn't
    /// exist (PA-B1); the live preview still surfaces the Restart button in its dev menu.
    fn kernel_unavailable_message(
        lang: &str,
        path: &str,
        var: &str,
        last_error: Option<&str>,
    ) -> String {
        match last_error {
            Some(e) => format!(
                "{lang} kernel unavailable ({path}): {e}. Code cells render as source; fix \
                 the interpreter ({var} or _site.yml {lang}:) or install its Jupyter kernel \
                 package. Run `taliesin doctor` to see which."
            ),
            None => format!(
                "{lang} kernel unavailable ({path}); code cells render as source (set {var} \
                 or _site.yml {lang}: to an interpreter with the Jupyter kernel). Run \
                 `taliesin doctor` to see whether it's the interpreter or a missing kernel package."
            ),
        }
    }

    /// Drop every language's kernel and clear the failure backoff, so the next run
    /// starts fresh kernels immediately. Backs the dev-menu "Restart kernel" action
    /// and recovery after fixing `TALIESIN_PYTHON`/`TALIESIN_R`. (Dropping a kernel
    /// kills its child process.) Also forces the next run to re-execute every cell
    /// (ignoring disk-cache hits), so "Restart kernel" actually re-runs against the
    /// fresh kernel instead of replaying cached outputs.
    pub fn restart_kernel(&mut self) {
        reset_announcements();
        self.langs.clear();
        self.force_next = true;
    }

    /// Whether this executor currently owns a booted kernel, i.e. whether dropping it
    /// would actually kill a child process.
    ///
    /// A kernel is created lazily on the first executed cell, so a `LangState` can exist
    /// with `kernel: None` (a language whose start *failed*, which is why this asks about
    /// the kernel rather than about `langs` being non-empty). Callers use it to avoid
    /// announcing a kernel death that did not happen — see `serve_site::exec_pool`.
    pub fn has_live_kernel(&self) -> bool {
        self.langs.values().any(|s| s.kernel.is_some())
    }

    /// Execute the document's code cells (changed cells + downstream, per language)
    /// and return the block list with output blocks spliced in after each cell.
    /// Each executable language runs against its own kernel; unknown languages are
    /// left as source.
    pub async fn run(&mut self, blocks: Vec<Block>) -> Vec<Block> {
        self.run_through(blocks, None, None).await
    }

    /// [`Executor::run`], stopping after the cell at block index `until_block`.
    ///
    /// This is the editor's "Run Cell": *make the document true through here*. The
    /// plan's start is unchanged (the first cell whose state the kernel lacks), so a
    /// warm session runs only the edited cell while a cold one runs the prefix it
    /// needs — the cap only says how far this pass may go. Cells past the cap are
    /// left to restore from the disk cache or stay empty; because nothing writes a
    /// freeze entry for a cell that did not run, a capped run can never publish a
    /// stale output. `None` is the uncapped whole-document run.
    ///
    /// The cap is a **block** index rather than a per-language cell index, so it
    /// means the same thing in a document that mixes `{python}` and `{r}`: every
    /// language runs its cells up to that point in the document.
    /// `requested_at` is the [`crate::run_control::RunControl`] epoch this run was **asked
    /// for** at, for a run that arrives through a queue (`POST /__taliesin/run`). The run
    /// stops the moment the live epoch differs, which covers the case a run-start snapshot
    /// cannot: an interrupt that lands while this run is still waiting its turn. `None` means
    /// "not a queued request" (a watcher rebuild, the build path, tests) and snapshots the
    /// epoch here instead, so such a rebuild is never pre-emptively cancelled.
    pub async fn run_through(
        &mut self,
        blocks: Vec<Block>,
        until_block: Option<usize>,
        requested_at: Option<u64>,
    ) -> Vec<Block> {
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
                    traced: is_traced(lang, &b.html),
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
        let mut tally = CacheTally::default();
        let run_epoch = requested_at.unwrap_or_else(|| self.control.epoch());
        for (lang, cells) in &by_lang {
            // A document mixing `{python}` and `{r}` computes one language at a time. Ctrl-C
            // means "stop this run", not "stop the python half and then do all the R", so
            // the cancel has to break out here too.
            if self.control.epoch() != run_epoch {
                break;
            }
            let (outputs, lang_tally) = self
                .compute_outputs(lang, cells, until_block, run_epoch)
                .await;
            tally += lang_tally;
            for (cell, inner) in cells.iter().zip(&outputs) {
                // `include: false` cells run (above) for their kernel-state side
                // effects but contribute no visible output block.
                if inner.trim().is_empty() || !cell.include {
                    // A labelled figure/table cell that ran but emitted nothing left a
                    // dead `@fig-`/`@tbl-` anchor render already committed to — only
                    // knowable now, so warn (it can't be un-burned post-execution).
                    if let Some(w) = empty_labelled_float_warning(cell, inner) {
                        crate::log::warn(&w);
                    }
                    continue;
                }
                output_blocks.insert(cell.block_index, output_block(cell, inner));
            }
        }
        // One legible cache line per run (DX9): only when something replayed, so a cold
        // run (nothing cached) stays quiet and the "why didn't my cell re-run?" case — an
        // all-cached replay reporting `· 0 re-ran` — is the one that speaks up.
        if tally.cached > 0 {
            crate::log::cache_tally(self.page.as_deref(), tally.cached, tally.ran);
        }
        // A forced re-run (Restart kernel) applies to every language in this pass,
        // then clears. Flush any newly executed outputs to `_freeze/` once.
        self.force_next = false;
        self.freeze.save();
        self.control.end_cell();

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
    /// Also returns a [`CacheTally`] (how many cells replayed vs re-ran) so the caller
    /// can print one legible cache summary per run (DX9).
    async fn compute_outputs(
        &mut self,
        lang: &'static str,
        cells: &[CellRef],
        until_block: Option<usize>,
        run_epoch: u64,
    ) -> (Vec<String>, CacheTally) {
        // The interpreter identity seeds the cumulative hash chain (a different
        // interpreter/version can't serve another's outputs). Computed up front so
        // even a full cold replay — which never boots the kernel — can key the cache.
        let interp = match self.spec(lang) {
            Some((_, program)) => interp_id(lang, &program).await,
            None => lang.to_string(),
        };
        // A traced cell's hash key folds in a marker so toggling `#| trace: true` busts
        // the cache. `strip_cell_options` already stripped that directive line out of
        // `c.code` itself (it isn't code the kernel runs), so without this a cell whose
        // UNTRACED output is already cached would silently replay it instead of
        // re-tracing: same `c.code`, same key, `known(i)` reports a hit, and the wrap in
        // the execute loop below never runs because the cell never re-executes at all.
        // Only a traced cell's key changes, so an existing all-untraced cache stays hit.
        let code_refs: Vec<String> = cells
            .iter()
            .map(|c| {
                if c.traced {
                    format!("{}\n#| trace: true", c.code)
                } else {
                    c.code.clone()
                }
            })
            .collect();
        let code_ref_strs: Vec<&str> = code_refs.iter().map(String::as_str).collect();
        let hashes = freeze::cumulative_hashes(&interp, &code_ref_strs);

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
        // The caller's cap is a document **block** index; `plan` counts in this
        // language's own cell indices. `cells` is in document order, so the count of
        // cells at or before the cap is exactly "one past the last cell this pass may
        // execute". A language with no cell at or before the cap gets `Some(0)` and
        // therefore runs nothing, which is right: none of its cells is above the line.
        let limit = until_block.map(|b| cells.iter().take_while(|c| c.block_index <= b).count());
        let (shared, run_end) = plan(&ran, &hashes, known, |i| cells[i].cache, limit);

        // Per-cell states from the zones `plan()` just computed (pure observation —
        // doesn't change what runs or caches). The warm prefix `[0, shared)` and the
        // cached tail `[run_end, len)` are already available, so they're `done`; the
        // run range `[shared, run_end)` is `queued` and turns `running`/`done`/`error`
        // in the loop below. `cell_id` is each cell's own id (the id the output block
        // is built from as `{id}-out`), so the client can target that block.
        for (i, cell) in cells.iter().enumerate() {
            // The warm prefix and the disk-cached tail are both restored without running,
            // so tag them `source: "cache"` (DX9) — that's what turns a client's blank `✓`
            // into `⚡ cached`. The run range is still `queued` (no source yet).
            //
            // A CAPPED run adds a fourth zone that did not exist before: cells past
            // `run_end` that are NOT in the cache. They were not run and have no output,
            // so calling them `done`/`cache` (as the tail rule alone would) is simply
            // false — it reported "1 ran, 2 cached" for a document whose last two cells
            // had never executed at all. They get `skipped`, which is what actually
            // happened, and the next uncapped run or `build` picks them up.
            let (state, source) = if i < shared {
                ("done", Some("cache"))
            } else if i >= run_end {
                if known(i) {
                    ("done", Some("cache"))
                } else {
                    ("skipped", None)
                }
            } else {
                ("queued", None)
            };
            emit(
                &self.sink,
                crate::protocol::cell_state(
                    self.page.as_deref(),
                    &cell.id,
                    state,
                    None,
                    None,
                    source,
                ),
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
        // Where a cancel stopped this language, if it did. Everything from here on was NOT
        // executed, which the freeze + warm-prefix bookkeeping below both have to honour:
        // recording an unexecuted cell as warm would make a later rebuild skip it, and that
        // is precisely how a run that was stopped ends up publishing an output nobody
        // computed.
        let mut stopped_at: Option<usize> = None;
        let control = self.control.clone();
        for (i, cell) in cells.iter().enumerate() {
            if i < shared {
                outputs.push(warm.get(i).cloned().unwrap_or_default());
            } else if i < run_end {
                // Checked BETWEEN cells, which is the only place a run can be stopped
                // cooperatively: the signal ends the cell that is executing, and this ends
                // the ones that have not started. A `stopped_at` already set means an
                // earlier cell in this loop tripped it.
                if stopped_at.is_some() || control.epoch() != run_epoch {
                    let at = *stopped_at.get_or_insert(i);
                    debug_assert!(at <= i);
                    // The cell was announced `queued`. Give it a terminal `skipped` so it
                    // does not spin forever in a client, and so the terminal's summary counts
                    // it honestly. `skipped` already means "did not run, has no output",
                    // which is exactly true here.
                    emit(
                        &sink,
                        crate::protocol::cell_state(
                            page.as_deref(),
                            &cell.id,
                            "skipped",
                            None,
                            None,
                            None,
                        ),
                    );
                    outputs.push(String::new());
                } else if !has_kernel {
                    // The kernel could not boot (a port-allocation race under
                    // concurrent starts, a backoff after a failed start, or no
                    // interpreter): this cell was meant to run but can't. If its output
                    // is still validly cached (it's in the run range only because a
                    // DOWNSTREAM cell must run), restore that cache instead of clobbering
                    // it — a transient boot failure then keeps outputs we already have on
                    // disk. Otherwise splice a VISIBLE diagnostic where its output would
                    // go: a build has no websocket, so the `error` cell_state/build_state
                    // emitted above never reaches the HTML, and without this the output
                    // div would simply be absent (the silent drop). The cell still renders
                    // as source above this block either way, and its `error` cell-state
                    // still honestly signals it did not run fresh. (`tali-error` => styled
                    // as an error AND uncacheable, so the diagnostic is never persisted.)
                    let cached = if !force && cell.cache {
                        self.freeze.get(&hashes[i]).map(str::to_string)
                    } else {
                        None
                    };
                    outputs.push(cached.unwrap_or_else(|| {
                        kernel_unavailable_html(
                            lang,
                            self.langs.get(lang).and_then(|s| s.last_error.as_deref()),
                        )
                    }));
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
                        crate::protocol::cell_state(
                            page.as_deref(),
                            &cell.id,
                            "error",
                            None,
                            None,
                            None,
                        ),
                    );
                    outputs.push(KERNEL_DIED_HTML.to_string());
                } else {
                    // Progress only when the kernel is up; otherwise cells are instant
                    // no-ops and a "cell k/n" line would be misleading.
                    if has_kernel {
                        ran_count += 1;
                        crate::log::exec(page.as_deref(), ran_count, to_run);
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
                    // `#| trace: true` cells run under the settrace harness instead of
                    // verbatim; everything downstream (hashing above, caching, the
                    // output splice below) is unaware and treats the result as
                    // ordinary cell output, which is the whole point (see trace_py.rs).
                    let code = if cell.traced {
                        crate::trace_py::wrap_traced(&cell.code)
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
                                None,
                            ),
                        );
                        t0
                    });
                    // Publish the kernel PID for the duration of this cell, so an interrupt
                    // arriving on another request can signal it. It has to happen here,
                    // before `exec_cell` takes its mutable borrow: once that borrow is
                    // live, nothing else can read the kernel at all.
                    control.begin_cell(
                        lang,
                        self.langs
                            .get(lang)
                            .and_then(|s| s.kernel.as_ref())
                            .and_then(Kernel::pid),
                    );
                    let out = self.exec_cell(lang, &code, &cell.id, page.as_deref()).await;
                    control.end_cell();
                    if let Some(t0) = t0 {
                        let state = if is_uncacheable(&out) {
                            "error"
                        } else {
                            "done"
                        };
                        // A cell that actually executed is `source: "fresh"` (DX9), so a
                        // real "✓ 1.2s" run is never mistaken for a cache restore. Errors
                        // carry no source.
                        let source = (state == "done").then_some("fresh");
                        emit(
                            &sink,
                            crate::protocol::cell_state(
                                page.as_deref(),
                                &cell.id,
                                state,
                                Some(t0),
                                Some(now_ms().saturating_sub(t0)),
                                source,
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
        // A cancel truncates the executed range: cells from `stopped_at` on never ran, and
        // their `outputs` entries are the empty placeholders pushed above. Persisting those
        // would cache emptiness under a hash that means "this cell's real output", which is
        // the worst failure this cache can have — a later build would restore nothing and
        // call it a hit.
        let executed_end = stopped_at.unwrap_or(run_end);
        if has_kernel {
            for i in shared..executed_end {
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
            // A kernel that DIED mid-run (an early cell crashed it) now holds
            // nothing: the executed prefix plus the trailing KERNEL_DIED
            // placeholders must NOT be recorded as warm. Otherwise a later
            // code-unchanged rebuild sees a full hash match (to_run == 0), never
            // calls `ensure_kernel` to reap the corpse + respawn, and serves those
            // KERNEL_DIED placeholders forever. Dropping the prefix makes the next
            // rebuild start cold from cell 0 and self-heal on a fresh kernel.
            let kernel_alive = state.kernel.as_mut().is_some_and(Kernel::is_alive);
            state.ran = if kernel_alive {
                // `executed_end`, not `run_end`: a cancelled run's kernel holds state only
                // up to where it stopped. Recording the untouched tail as warm would make
                // the next rebuild believe those cells' state is already in the kernel and
                // skip them, leaving the document permanently missing their output with
                // nothing to indicate why.
                (0..executed_end)
                    .map(|i| Ran {
                        hash: hashes[i].clone(),
                        output: outputs[i].clone(),
                    })
                    .collect()
            } else {
                Vec::new()
            };
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
        // Cache legibility (DX9): the warm prefix `[0, shared)` and the disk tail
        // `[run_end, len)` were restored without running; `ran_count` is what actually
        // executed. Observational only — computed after all execution, changes nothing.
        let cached = shared + cells.len().saturating_sub(run_end);
        (
            outputs,
            CacheTally {
                cached,
                ran: ran_count,
            },
        )
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
        // Owned before the mutable borrow of `state` below, so the announce can name the
        // resolved interpreter + its provenance without a second borrow of `self`.
        let (prov, lang_enum) = if lang == "r" {
            (self.r.provenance, Lang::R)
        } else {
            (self.python.provenance, Lang::Python)
        };
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
            // Committed to a boot (warm-pool or cold): announce which interpreter runs
            // this language, once per executor. Only languages the document actually
            // runs reach here, so an R-free doc never claims an R interpreter.
            if !state.announced {
                crate::log::kernel(&format!(
                    "{lang} -> {}  (from {})",
                    program.display(),
                    prov.label(lang_enum)
                ));
                state.announced = true;
            }
        }

        // Warm-pool fast path (python only): a ready, pre-booted kernel is claimed
        // with no perceptible wait, so we emit **no** `warming-kernel` state. The
        // pool may yield `None` (inert pool, empty queue, or a non-python lang) — in
        // which case we fall through to the unchanged cold start below. A pooled
        // kernel is forked from the daemon's cwd, so we chdir it to this document's
        // `work_dir` to match a cold kernel started with `current_dir(work_dir)`;
        // this keeps relative cell writes (a saved figure, audio) landing beside the
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
        // Retry a transient start failure with a fresh port allocation. That re-roll now
        // lives on the primitive that allocates the ports (`Kernel::start_with_retry`),
        // because every caller needs it and the one that lacked it flaked; before it
        // existed here, a lost port race silently rendered this doc's cells as source.
        let started = Kernel::start_with_retry(&spec, work_dir.as_deref()).await;
        let state = self.langs.entry(lang).or_default();
        match started {
            Ok(k) => {
                crate::log::kernel(&format!("{lang} ready ({})", program.display()));
                state.kernel = Some(k);
                state.failed_at = None;
                state.last_error = None;
            }
            Err(e) => {
                // The one console emission for this failure, on the one path every
                // command reaches. It used to be a terse line here PLUS the full
                // `diagnostic()` line at the caller, so `build`/`read` printed the same
                // fact twice and the short form said strictly less; a site build printed
                // only the short form, once per page, and never the actionable half.
                // `announce_once` is what makes the per-page repeat one line: the answer
                // to "which interpreter, and why" cannot differ between pages of one run.
                announce_once(&Self::kernel_unavailable_message(
                    lang,
                    &program.display().to_string(),
                    if lang == "r" {
                        "TALIESIN_R"
                    } else {
                        "TALIESIN_PYTHON"
                    },
                    Some(&e.to_string()),
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

    /// Run one cell, streaming its output to the client as it arrives (item 175b).
    ///
    /// `cell_id` and `page` only address the live messages; they do not affect the
    /// returned HTML, which is still the authoritative render of the whole output
    /// vector and is what gets cached and diffed into the block.
    async fn exec_cell(
        &mut self,
        lang: &'static str,
        code: &str,
        cell_id: &str,
        page: Option<&str>,
    ) -> String {
        // Cloned before the kernel borrow so the callback can emit while `self` is
        // mutably borrowed by `execute_streaming`.
        let sink = self.sink.clone();
        let page = page.map(str::to_string);
        let cell_id = cell_id.to_string();
        let Some(kernel) = self.langs.get_mut(lang).and_then(|s| s.kernel.as_mut()) else {
            return String::new(); // kernel unavailable: cell renders as source
        };
        // Mirrors the list the browser is building, so each arriving output becomes
        // either a new element or a redraw of the last one. Same rule the final
        // `render_outputs` applies, by construction: both go through `LiveOutputs`.
        let mut live = crate::kernel::LiveOutputs::default();
        let result = kernel
            .execute_streaming(code, |o| {
                if sink.is_none() {
                    return; // a build has no websocket; skip the render entirely
                }
                let (op, shown) = match live.push(o.clone()) {
                    crate::kernel::LiveOp::Append(o) => ("append", o),
                    crate::kernel::LiveOp::ReplaceLast(o) => ("replace_last", o),
                };
                emit(
                    &sink,
                    crate::protocol::cell_output_append(
                        page.as_deref(),
                        &cell_id,
                        op,
                        &render_outputs(std::slice::from_ref(&shown)),
                    ),
                );
            })
            .await;
        match result {
            Ok(outs) => render_outputs(&outs),
            Err(e) => {
                crate::log::error(&format!("execution error: {e}"));
                execution_error_html(&e.to_string())
            }
        }
    }
}

/// Point a freshly-claimed warm-pool kernel at `dir`, so its relative file writes
/// (a `savefig`/`ggsave` figure, audio) land beside the document's source — matching a
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
    let code = format!("import os as _tali_os; _tali_os.chdir('{escaped}'); del _tali_os");
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
///     the kernel needs its state to reach an unknown cell after it. `run_end` also
///     jumps to `len` when any `#| cache: false` cell exists: that cell re-runs with
///     possibly-different state, so every cell after it must re-run too rather than
///     restore a disk hit the cumulative key can't tell is stale.
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
    limit: Option<usize>,
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
    // A `#| cache: false` cell re-executes every run and may leave *different* kernel
    // state behind, so every cell after it is untrustworthy from disk even when its own
    // key is a cache hit (the cumulative key can't see the upstream cell's
    // non-determinism). Extend the run range to the document end so those downstream
    // cells re-run against the freshly-computed state instead of restoring a stale
    // output. This only bites when a `cache: false` cell exists (`first_uncacheable ==
    // len` otherwise, leaving `run_end` untouched), so the common all-cacheable
    // document is unchanged.
    if first_uncacheable < hashes.len() {
        run_end = hashes.len();
    }
    // A capped run ("make the document true THROUGH cell N" — the editor's Run Cell)
    // may execute no further than `limit`. Applied LAST, after the `cache: false`
    // extension, because the cap answers a different question: not "what is
    // trustworthy" but "how far is this pass allowed to go". Cells past it are simply
    // left stale, and since nothing persists a freeze entry for a cell that did not
    // run, a capped run can never publish a lie — the next uncapped run, preview
    // rebuild, or `build` finishes the job.
    //
    // `.max(shared)` keeps the range well-formed when the warm prefix already reaches
    // past the cap: that is "already up to date through here", i.e. an empty run range.
    if let Some(limit) = limit {
        run_end = run_end.min(limit).max(shared);
    }
    (shared, run_end)
}

/// Whether `--no-exec` / `TALIESIN_NO_EXEC` is in force, i.e. code cells render as source
/// and no interpreter is ever asked to run anything. Read here rather than inline so the
/// *build* can consult the same answer before deciding whether to boot a warm pool.
///
/// Delegates to `taliesin_core::render::no_exec_in_force`, which the render pass consults
/// for `{js}` cells (item 79). Two independent reads of one variable is exactly the shape
/// that lets a flag mean two things in one process, so there is one owner.
pub(crate) fn exec_disabled() -> bool {
    taliesin_core::render::no_exec_in_force()
}

/// Whether an output must not be cached: any execution error (a cell error, a
/// timeout, or the mid-run kernel-died marker — all rendered as a `tali-error` block),
/// so a transient failure is never replayed and the cell re-runs next time. Matches
/// the emitted `class="tali-error"` rather than a bare substring, so a *successful*
/// cell whose output merely prints the text "tali-error" still caches. Also refuses to
/// cache an output the kernel *truncated* at the size cap: if the cell completes
/// cleanly (no KeyboardInterrupt error) the truncated result would otherwise be frozen
/// and replayed silently. The marker text comes from `kernel.rs`'s output caps, and is
/// matched in its **bracketed emitted form** (`[taliesin: output truncated at …`) for the
/// same reason as the `tali-error` half beside it: a cell that merely *prints* the phrase
/// (a doc about this feature, a log line) was otherwise refused the cache forever and
/// re-ran on every single build.
fn is_uncacheable(output: &str) -> bool {
    output.contains("class=\"tali-error\"") || output.contains(crate::kernel::TRUNCATION_MARKER)
}

/// How long `<program> --version` may take before the probe gives up. This sits
/// upstream of `ensure_kernel`, so it is the *first* thing a rebuild waits on and no
/// other timeout can rescue it: an unbounded probe wedges the pipeline forever (a
/// stuck NFS mount, a python blocking on import, a hung conda shim), recoverable only
/// by killing the process. A version probe is a fork+exec+print, not work: it is
/// milliseconds warm and a second or two for a cold conda shim on a slow disk. Ten
/// seconds is far past any healthy interpreter yet short enough that a wedged one
/// degrades to "no version in the id" instead of hanging the build. Deliberately not
/// configurable: a knob here would only ever be turned because something else is
/// broken.
const INTERP_PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// A stable identity for a language's interpreter, used to seed the cumulative
/// hash chain so a different interpreter (or an upgraded one) can't serve outputs
/// it didn't compute. Runs `<program> --version` once and memoizes the *answer* per
/// `(lang, program)` for the process; if the interpreter can't be asked (not
/// installed, or the probe times out), falls back to the program path so the id is
/// still stable.
async fn interp_id(lang: &str, program: &Path) -> String {
    probe_interp_id(lang, program, INTERP_PROBE_TIMEOUT).await
}

/// [`interp_id`] with an injectable bound, so the timeout path is testable in
/// milliseconds instead of [`INTERP_PROBE_TIMEOUT`].
///
/// Two properties are load-bearing here, because this id seeds the freeze cache's
/// cumulative hash chain:
///
///   - **Only an answer is memoized.** A probe that *failed to ask* (spawn error,
///     timeout) is not cached, so a transient failure is retried on the next rebuild
///     rather than poisoning every freeze key for the process lifetime. A program
///     that runs and prints nothing *has* answered ("no version"), and that is cached:
///     it is a stable fact about that program, not a transient failure, and caching it
///     is what keeps the probe to one fork per process.
///   - **A successful probe's string is byte-identical to before.** Anything else
///     silently changes cache identity and invalidates every existing `_freeze/`.
async fn probe_interp_id(lang: &str, program: &Path, bound: Duration) -> String {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let key = format!("{lang}\u{0}{}", program.display());
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(id) = cache.lock().get(&key) {
        return id.clone();
    }
    // The lock is NOT held across the await below: two rebuilds racing the same cold
    // interpreter may both probe, but they compute the same id, so the duplicate fork
    // is the whole cost. Holding it would serialize every language behind one probe.
    let answer = probe_version(program, bound).await;
    let version = answer.clone().unwrap_or_default();
    let id = format!("{lang}::{}::{version}", program.display());
    if answer.is_some() {
        cache.lock().insert(key, id.clone());
    }
    id
}

/// Run `<program> --version` under `bound` and return its reported version line.
/// `None` means *we could not ask* (spawn failed, or it hung past `bound`), as
/// opposed to `Some("")`, which means it ran and reported nothing.
async fn probe_version(program: &Path, bound: Duration) -> Option<String> {
    // Async, not `std::process`: this runs on a tokio worker, so a blocking wait here
    // stalls the runtime as well as the build. `kill_on_drop` means the timeout below
    // actually reaps the hung child instead of leaking it for the process lifetime.
    // stdin is /dev/null to match `Command::output`'s contract (and so an interpreter
    // that drops to a REPL sees EOF rather than waiting on the parent's terminal).
    let child = tokio::process::Command::new(program)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let out = tokio::time::timeout(bound, child.wait_with_output())
        .await
        .ok()?
        .ok()?;
    // Python prints to stdout, some tools to stderr; take whichever is set.
    let bytes = if out.stdout.is_empty() {
        out.stderr
    } else {
        out.stdout
    };
    Some(
        String::from_utf8_lossy(&bytes)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string(),
    )
}

/// A visible "this cell could not run" diagnostic, spliced where a boot-failed (or
/// otherwise kernel-unavailable) cell's output would go. Without it the output `<div>`
/// is simply absent — a silent drop — because a build emits no websocket diagnostic
/// (only the live preview's status banner would show it). Carries `tali-error` so it is
/// styled as an error AND treated as uncacheable (never persisted to the freeze cache).
/// The last kernel error (e.g. a ZMQ "address already in use" from a port-allocation
/// race under concurrent starts) is appended when known, so the page names *why*.
pub(crate) fn kernel_unavailable_html(lang: &str, last_error: Option<&str>) -> String {
    let detail = match last_error {
        Some(e) if !e.is_empty() => format!(" ({})", esc(e)),
        _ => String::new(),
    };
    format!(
        "<pre class=\"tali-error\"{}>{} kernel unavailable; this cell did not execute{detail}</pre>",
        not_run_mark(NOT_RUN_UNAVAILABLE),
        esc(lang)
    )
}

/// A cell whose execution *request* failed (a ZMQ/protocol error, an interrupt): the
/// interpreter never returned a result, so like the two above this is the executor
/// reporting, not the author's code raising.
pub(crate) fn execution_error_html(err: &str) -> String {
    format!(
        "<pre class=\"tali-error\"{}>execution error: {}</pre>",
        not_run_mark(NOT_RUN_REQUEST),
        esc(err)
    )
}

/// A `label: fig-x`/`tbl-x` cell whose executed output turns out **empty** leaves a
/// dead cross-reference: the render pass already reserved it a number and registered
/// `@fig-x`/`@tbl-x` (an optimistic bet, since a figure/table cell normally produces
/// output), but with no output no element carries the anchor — so `@fig-x` resolves
/// to a "Figure N" nothing shows and every later float shifts down by one.
///
/// This is only knowable *here*: the emptiness is a post-execution fact the render
/// pass can't see, so — unlike a non-executable lang (`{bash}`, …), declined up front —
/// it can't be prevented, and the number/ref are already baked. Surface it as a
/// build/serve warning naming the anchor so the author can drop the label or make the
/// cell emit output.
///
/// Deliberately narrow: `include: false` is excluded (that output is *meant* to be
/// dropped, and render already warns the label is unreachable via
/// `unreferenceable_hidden_label`); a kernel-unavailable cell is excluded implicitly,
/// its `inner` being the non-empty "kernel unavailable" diagnostic rather than empty;
/// and an unlabelled empty cell (`x = 1`) is normal, not a phantom.
fn empty_labelled_float_warning(cell: &CellRef, inner: &str) -> Option<String> {
    if !cell.include || !inner.trim().is_empty() {
        return None;
    }
    let (kind, anchor) = cell
        .figure
        .as_ref()
        .and_then(|f| f.anchor.as_deref())
        .map(|a| ("figure", a))
        .or_else(|| {
            cell.table
                .as_ref()
                .and_then(|t| t.anchor.as_deref())
                .map(|a| ("table", a))
        })?;
    Some(format!(
        "the cell labelled `{anchor}` produced no output, so @{anchor} resolves to a \
         {kind} number no element carries — remove the label or make the cell emit output"
    ))
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
/// output; if there is none, the caption and anchor go on a wrapper instead (see
/// [`table_figure_wrap`]) — never nowhere.
fn table_wrap(tbl: &CellTable, inner: &str) -> String {
    let id_attr = tbl
        .anchor
        .as_deref()
        .map(|a| format!(" id=\"{}\"", esc(a)))
        .unwrap_or_default();
    let caption = tbl.caption.as_deref().unwrap_or("").trim();
    let sep = if caption.is_empty() { "" } else { ": " };
    let Some(start) = inner.find("<table") else {
        return table_figure_wrap(tbl, inner, &id_attr, caption, sep);
    };
    let Some(rel_gt) = inner[start..].find('>') else {
        return table_figure_wrap(tbl, inner, &id_attr, caption, sep);
    };
    let gt = start + rel_gt + 1;
    let open = inner[start..gt].replacen("<table", &format!("<table{id_attr}"), 1);
    format!(
        "{}{open}<caption>Table&nbsp;{}{sep}{}</caption>{}",
        &inner[..start],
        tbl.number,
        esc(caption),
        &inner[gt..],
    )
}

/// The fallback for a `#| label: tbl-x` cell whose output holds no `<table>`: an R
/// `kable()` string printed as text, a `data.frame`'s fixed-width repr, a cell that
/// errored, or one that never ran because no kernel was available.
///
/// The number is spent and the cross-reference is already rewritten by the time this
/// runs — `apply_table_captions` numbers and registers `tbl-x` from the *label*, with
/// no knowledge of what the cell will print — so returning the output untouched left
/// `@tbl-x` a live link to an id nothing in the document emits, silently and with a
/// clean `check` (which never executes a cell, so it cannot see this at all). Carry
/// the caption and the anchor on a wrapper instead, exactly as [`figure_wrap`] has
/// always done for a figure cell that produced no image. The caption leads, because a
/// table's caption sits above it.
fn table_figure_wrap(
    tbl: &CellTable,
    inner: &str,
    id_attr: &str,
    caption: &str,
    sep: &str,
) -> String {
    format!(
        "<figure{id_attr} class=\"tali-figure tali-table-figure\">\
         <figcaption>Table&nbsp;{}{sep}{}</figcaption>{inner}</figure>",
        tbl.number,
        esc(caption),
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

    // AP4-3: `is_uncacheable` must match the *emitted* truncation notice, not the bare
    // phrase. A cell that merely prints the phrase (a doc about output caps, a log line
    // quoting one) was refused the cache forever and re-ran on every single build — the
    // same false-positive the `tali-error` half was deliberately hardened against.
    #[test]
    fn only_a_real_truncation_notice_blocks_caching() {
        // What `kernel.rs` actually emits when a cap fires.
        let items = format!(
            "<pre>\n{}4096 items]\n</pre>",
            crate::kernel::TRUNCATION_MARKER
        );
        let bytes = format!("<pre>\n{}512 KB]\n</pre>", crate::kernel::TRUNCATION_MARKER);
        assert!(is_uncacheable(&items), "the item cap must block caching");
        assert!(is_uncacheable(&bytes), "the byte cap must block caching");
        assert!(
            is_uncacheable(r#"<div class="tali-error">boom</div>"#),
            "an execution error must block caching"
        );

        // A successful cell whose output merely *talks about* truncation still caches.
        assert!(
            !is_uncacheable("<pre>taliesin: output truncated is the message it prints</pre>"),
            "printing the phrase is not a truncation"
        );
        assert!(
            !is_uncacheable("<pre>see the tali-error class for details</pre>"),
            "printing the class name is not an error"
        );
        assert!(!is_uncacheable("<pre>42</pre>"), "ordinary output caches");
    }

    #[test]
    fn kernel_unavailable_message_is_headless_safe_and_routes_to_doctor() {
        // Shared with `build`/`read`/CI, where the dev-menu "Restart kernel" action does not
        // exist (PA-B1). It must never tell a headless caller to click it, must route to
        // `taliesin doctor`, and must name the env var to fix.
        for last in [Some("boom"), None] {
            let msg = Executor::kernel_unavailable_message(
                "python",
                "/usr/bin/python3",
                "TALIESIN_PYTHON",
                last,
            );
            assert!(
                !msg.to_lowercase().contains("restart kernel"),
                "must not reference the dev-menu Restart action: {msg}"
            );
            assert!(
                msg.contains("taliesin doctor"),
                "must route to doctor: {msg}"
            );
            assert!(
                msg.contains("TALIESIN_PYTHON"),
                "must name the env var: {msg}"
            );
        }
    }

    /// Only a `{python}` cell is ever handed [`crate::trace_py::wrap_traced`]'s harness.
    ///
    /// `wrap_traced` splices the author's code into a **Python** program, and the marker it
    /// keys off is stamped by `emit.rs` for any language at all. Dropping the `lang` half of
    /// [`is_traced`] therefore does not fail loudly, it feeds `def _tali_debug_run(_src):`
    /// to whatever kernel the cell belongs to: reproduced against a live IRkernel as
    /// `Error in parse(text = input): <text>:2:5: unexpected input` (see
    /// `crates/server/tests/r_kernel.rs`, which pins the same fix end to end).
    #[test]
    fn only_a_python_cell_is_ever_wrapped_in_the_python_trace_harness() {
        let marked = r#"<pre data-tali-cell="r" data-tali-trace="1"><code>x &lt;- 1</code></pre>"#;
        assert!(
            is_traced("python", marked),
            "a marked python cell is the whole point of the feature"
        );
        assert!(
            !is_traced("r", marked),
            "an `{{r}}` cell must never be handed the python harness"
        );
        assert!(
            !is_traced("python", "<pre><code>x = 1</code></pre>"),
            "an unmarked cell is not traced whatever its language"
        );
    }

    /// The render pass reserves a `@fig-`/`@tbl-` number only for a lang core believes
    /// executes (`taliesin_core::render::executes_to_kernel`), while `kernel_lang` is
    /// what actually runs one. If the two sets ever drift, a `label: fig-*` on a lang
    /// core thinks executes but the kernel does not (or the reverse) re-opens the
    /// phantom-anchor bug those two functions exist to prevent. Pin them equal — this
    /// is the "shared executable set" the render-side comment relies on.
    #[test]
    fn kernel_lang_agrees_with_cores_executable_set() {
        for lang in [
            "python", "r", "bash", "sql", "julia", "js", "mermaid", "ruby", "",
        ] {
            assert_eq!(
                kernel_lang(lang).is_some(),
                taliesin_core::render::executes_to_kernel(lang),
                "kernel_lang and executes_to_kernel disagree on {lang:?}"
            );
        }
    }

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
            traced: false,
        }
    }

    /// A `label: fig-x`/`tbl-x` cell that RUNS but prints nothing burns its number +
    /// dead-links `@fig-x`/`@tbl-x` (render bet on output; exec found none). The
    /// warning is what turns that silent post-execution phantom into an author-visible
    /// problem, so it must fire for both a figure and a table label and name the
    /// anchor. (The number/ref are baked at render time and can't be un-burned here.)
    #[test]
    fn empty_output_under_a_figure_or_table_label_warns() {
        let mut fig = cell("fig-silent");
        fig.figure = Some(CellFigure {
            anchor: Some("fig-silent".into()),
            caption: None,
            number: "1".into(),
        });
        let w = empty_labelled_float_warning(&fig, "   \n ")
            .expect("an empty-output figure label must warn");
        assert!(
            w.contains("fig-silent"),
            "warning must name the anchor: {w}"
        );
        assert!(w.contains("figure"), "warning must say it's a figure: {w}");

        let mut tbl = cell("tbl-silent");
        tbl.table = Some(CellTable {
            anchor: Some("tbl-silent".into()),
            caption: None,
            number: String::new(),
        });
        let w =
            empty_labelled_float_warning(&tbl, "").expect("an empty-output table label must warn");
        assert!(
            w.contains("tbl-silent"),
            "warning must name the anchor: {w}"
        );
        assert!(w.contains("table"), "warning must say it's a table: {w}");
    }

    /// The warning is precisely scoped to the phantom, so none of the everyday cases
    /// speak: a figure that DID emit output (the common path), an unlabelled cell that
    /// happens to print nothing (`x = 1`), and an `include: false` cell (its output is
    /// meant to be dropped, and render already warned the label is unreachable).
    #[test]
    fn empty_output_warning_stays_silent_off_the_phantom_path() {
        let mut fig = cell("fig-real");
        fig.figure = Some(CellFigure {
            anchor: Some("fig-real".into()),
            caption: None,
            number: "1".into(),
        });
        assert!(
            empty_labelled_float_warning(&fig, "<b>a real output</b>").is_none(),
            "a figure that produced output is not a phantom"
        );

        assert!(
            empty_labelled_float_warning(&cell("plain"), "").is_none(),
            "an unlabelled empty cell is normal, not a phantom"
        );

        let mut hidden = cell("fig-hidden");
        hidden.include = false;
        hidden.figure = Some(CellFigure {
            anchor: Some("fig-hidden".into()),
            caption: None,
            number: "1".into(),
        });
        assert!(
            empty_labelled_float_warning(&hidden, "").is_none(),
            "include: false drops output by design (render already warned)"
        );
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

    /// Item 175(d). Against a REAL kernel, because every claim here is about live process
    /// state: that the signal lands, that the run stops rather than continuing, and that the
    /// warm variables survive. A mocked kernel could not tell any of those apart.
    #[tokio::test]
    async fn an_interrupt_stops_the_whole_run_and_keeps_the_warm_state() {
        let Some(py) = std::env::var_os("TALIESIN_PYTHON") else {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL is set but TALIESIN_PYTHON is unset: the live-kernel \
                 tests would silently skip. Point TALIESIN_PYTHON at a python with ipykernel."
            );
            eprintln!("SKIPPED (no live kernel): set TALIESIN_PYTHON to exercise the interrupt.");
            return;
        };
        let dir = std::env::temp_dir().join(format!("tali-interrupt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        // Cell 3's side effect is a FILE, deliberately. A variable would be invisible once
        // the run is abandoned, and "cell 3 did not run" is exactly what this must prove.
        let marker = dir.join("cell3-ran");

        let mut ex = Executor::new().in_dir(&dir);
        ex.set_interpreters(
            crate::interpreter::resolve_python(py.to_str(), &dir),
            crate::interpreter::resolve_r(None, &dir),
        );
        let control = std::sync::Arc::new(crate::run_control::RunControl::default());
        ex.set_run_control(control.clone());

        let blocks = vec![
            python_cell_block_with("b-1", "warm = 41"),
            python_cell_block_with("b-2", "import time\ntime.sleep(30)"),
            python_cell_block_with(
                "b-3",
                &format!("open({:?}, 'w').write('ran')", marker.to_string_lossy()),
            ),
        ];

        // Cancel once cell 2 is actually executing. Polling the control rather than sleeping
        // a fixed amount: on a cold kernel boot a fixed sleep either fires before anything
        // runs (testing nothing) or after 30s (testing nothing).
        let waiter = {
            let control = control.clone();
            tokio::spawn(async move {
                let deadline = std::time::Instant::now() + Duration::from_secs(60);
                loop {
                    if control.running_lang().is_some() {
                        // In the middle of a cell. Cell 1 is instant, so this is cell 2.
                        tokio::time::sleep(Duration::from_millis(300)).await;
                        return control.cancel();
                    }
                    if std::time::Instant::now() > deadline {
                        return None;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
        };

        let t0 = std::time::Instant::now();
        let out = ex.run(blocks).await;
        let elapsed = t0.elapsed();
        let cancelled = waiter.await.unwrap();

        assert_eq!(
            cancelled,
            Some("python"),
            "the interrupt never found a running cell, so nothing below is meaningful"
        );
        // The sleep is 30s. Anything near that means the SIGINT did not land and the cell
        // simply finished, which would make the rest of this test pass for the wrong reason.
        assert!(
            elapsed < Duration::from_secs(25),
            "the run took {elapsed:?}; the interrupt did not stop the sleeping cell"
        );
        // The cancellation half: signalling cell 2 ends cell 2. Without the cancel FLAG the
        // run would carry on and execute cell 3, which is the bug this test exists for.
        assert!(
            !marker.exists(),
            "cell 3 ran after the run was interrupted: the signal stopped one cell, not the run"
        );
        // And the whole point of interrupting rather than restarting: the kernel is alive and
        // still holds what cell 1 put there.
        let after = render_outputs(
            &ex.langs
                .get_mut("python")
                .and_then(|s| s.kernel.as_mut())
                .expect("the kernel must survive an interrupt")
                .execute("print(warm + 1)")
                .await
                .unwrap(),
        );
        assert!(
            after.contains("42"),
            "cell 1's variable did not survive the interrupt, so this was a restart: {after}"
        );
        // Nothing was cached for the cell that did not finish, nor for the one that never
        // started. `out` keeps the source blocks either way; the claim is about `_freeze`.
        assert!(
            out.len() >= 3,
            "the block list should still describe the document"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn python_cell_block_with(id: &str, code: &str) -> Block {
        let mut b = python_cell_block(id);
        if let Some(c) = b.cell.as_mut() {
            c.code = code.to_string();
        }
        b
    }

    /// A throwaway executable script at `path`, `chmod +x`. Used to stand in for an
    /// interpreter whose `--version` probe hangs or is briefly unavailable.
    #[cfg(unix)]
    fn write_exe(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// [`probe_interp_id`], retried past a transient failure to *ask*.
    ///
    /// A just-written executable can briefly refuse to exec with `ETXTBSY`: these tests
    /// run in parallel, and another one spawning a process forks a child that momentarily
    /// inherits a write descriptor to this inode, which `execve` refuses to run. The
    /// probe reports that as an empty version, which at this call site is indistinguishable
    /// from a real answer of "no version", so a single unlucky probe reads as a format
    /// regression.
    ///
    /// Retrying is sound rather than a way to make red go green: a failure to *ask* is
    /// deliberately never memoized (that is the contract these tests pin), so a retry
    /// genuinely re-probes, while a wrong id is served identically on every call and the
    /// loop simply expires with the assertion still failing.
    async fn interp_id_settled(lang: &str, program: &Path, want: &str) -> String {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let id = probe_interp_id(lang, program, Duration::from_secs(10)).await;
            if id == want || std::time::Instant::now() >= deadline {
                return id;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_successful_probe_pins_the_freeze_key_format() {
        // The id this returns seeds every freeze key, so its exact bytes are a
        // compatibility surface: change them and every user's `_freeze/` silently
        // misses. Pins the four shapes the extraction has always handled, so a future
        // refactor of the probe cannot quietly move the format.
        let dir = std::env::temp_dir().join(format!("tali-interp-fmt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        // Version on stdout: the ordinary python case.
        let out = dir.join("on-stdout");
        write_exe(&out, "#!/bin/sh\necho 'Python 3.12.3'\n");
        let want = format!("python::{}::Python 3.12.3", out.display());
        assert_eq!(interp_id_settled("python", &out, &want).await, want);

        // Version on stderr only (how `python -V` used to report): stdout is empty, so
        // the probe falls back to stderr rather than recording an empty version.
        let err = dir.join("on-stderr");
        write_exe(&err, "#!/bin/sh\necho 'Python 2.7.18' >&2\n");
        let want = format!("python::{}::Python 2.7.18", err.display());
        assert_eq!(interp_id_settled("python", &err, &want).await, want);

        // Chatty multi-line output with padding: first line only, trimmed (R's banner).
        let multi = dir.join("multi-line");
        write_exe(
            &multi,
            "#!/bin/sh\nprintf '  R version 4.3.1 (2023-06-16) \\nCopyright (C)\\n'\n",
        );
        let want = format!("r::{}::R version 4.3.1 (2023-06-16)", multi.display());
        assert_eq!(interp_id_settled("r", &multi, &want).await, want);

        // Non-zero exit that still prints a version: the interpreter ANSWERED, so its
        // version must reach the id. Gating the probe on exit status instead of on
        // "did it run" would silently rewrite this key.
        let nz = dir.join("nonzero-exit");
        write_exe(&nz, "#!/bin/sh\necho 'Python 3.1.2'\nexit 3\n");
        let want = format!("python::{}::Python 3.1.2", nz.display());
        assert_eq!(interp_id_settled("python", &nz, &want).await, want);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_failed_interp_probe_is_not_memoized_for_the_process_lifetime() {
        // Regression (M2): `interp_id` seeds the freeze cache's cumulative hash chain.
        // It used to memoize `unwrap_or_default()` (an EMPTY version) whenever the
        // probe merely FAILED TO ASK (the binary wasn't there yet), poisoning every
        // freeze key for the rest of the process. A transient failure must be
        // retryable: once the interpreter answers, its real version must reach the id.
        let dir = std::env::temp_dir().join(format!("tali-interp-retry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let prog = dir.join("py-appears-late");

        // Probe 1: the binary does not exist yet -> spawn fails. This is "we failed to
        // ask", not an answer, so it must not be cached.
        let missing = interp_id("python", &prog).await;
        assert_eq!(
            missing,
            format!("python::{}::", prog.display()),
            "a failed probe keeps the established fallback id (program path, empty version)"
        );

        // The interpreter shows up (a slow NFS mount, a shim finishing an install).
        write_exe(&prog, "#!/bin/sh\necho 'Python 9.9.9'\n");

        // Probe 2 goes through `interp_id_settled` because it can itself transiently fail
        // to *ask* (see that helper), which is the same empty version a memoized failure
        // would produce. The regression this test exists for still fails: a memoized
        // failure is served from the cache on every retry, so the version never appears.
        let want = format!("python::{}::Python 9.9.9", prog.display());
        assert_eq!(
            interp_id_settled("python", &prog, &want).await,
            want,
            "the earlier FAILURE must not be memoized: a later successful probe has to \
             report the real version, or the freeze key stays poisoned for the process"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_hanging_interp_probe_gives_up_instead_of_wedging_the_build() {
        // Regression (M2): the probe was a blocking `Command::output()` with NO timeout,
        // called from this async fn BEFORE `ensure_kernel`, so it sat upstream of every
        // timeout in the codebase (`TALIESIN_CELL_TIMEOUT` included). A wedged interpreter
        // (stuck NFS, a python blocking on import, a hung conda shim) wedged the whole
        // rebuild pipeline forever, recoverable only by restarting the process.
        //
        // The bound is injected so the test is fast and deterministic; the production
        // bound is `INTERP_PROBE_TIMEOUT`.
        let dir = std::env::temp_dir().join(format!("tali-interp-hang-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let prog = dir.join("py-that-hangs");
        write_exe(&prog, "#!/bin/sh\nsleep 300\n");

        let started = Instant::now();
        let id = probe_interp_id("python", &prog, Duration::from_millis(200)).await;
        let elapsed = started.elapsed();

        assert!(
            elapsed < Duration::from_secs(10),
            "a hanging interpreter must not wedge the build; probe took {elapsed:?}"
        );
        assert_eq!(
            id,
            format!("python::{}::", prog.display()),
            "giving up falls back to the same id a failed probe has always produced"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_default_executor_resolves_through_the_one_resolver() {
        // `Executor::build` used to read TALIESIN_PYTHON and default to `python3` itself,
        // a second copy of a policy `interpreter.rs` already owned — so an executor that
        // nobody called `set_interpreters` on could not see a `.venv` at all, and the two
        // resolvers disagreed. Structural assertion, not a value one: only a `Resolved`
        // that came out of `resolve_python` carries a recorded `.venv` walk, so a trail
        // here is proof the duplicate policy is gone rather than merely agreeing today
        // in this particular cwd.
        let ex = Executor::new();
        assert!(
            ex.python.trail.ancestor.is_some(),
            "the default python must come from interpreter::resolve_python"
        );
        assert!(
            ex.r.trail.ancestor.is_none(),
            "R still has no venv walk, even by default"
        );
        // And it agrees, exactly, with what the single resolver says for the same dir.
        let want = crate::interpreter::resolve_python(None, std::path::Path::new("."));
        assert_eq!(ex.python.path, want.path);
        assert_eq!(ex.python.provenance, want.provenance);
    }

    #[tokio::test]
    async fn diagnostic_names_the_resolved_interpreter() {
        // `set_interpreters` overrides the env/default python, and a bogus resolved path
        // surfaces in the diagnostic verbatim, so a failure names the exact interpreter
        // (the 2026-07-11 "which python?" gap). A bogus path fails `Kernel::start`
        // deterministically, so this needs no live kernel.
        use crate::interpreter::{Provenance, Resolved};
        let mut ex = Executor::new();
        ex.set_interpreters(
            Resolved::fixed(
                "/nonexistent/tali/py-abc",
                Provenance::Field,
                crate::interpreter::Lang::Python,
            ),
            Resolved::fixed("R", Provenance::Default, crate::interpreter::Lang::R),
        );
        let _ = ex
            .run(vec![python_cell_block_with("py1", "print(1)")])
            .await;
        let diag = ex
            .diagnostic()
            .expect("a bogus interpreter yields a diagnostic");
        assert!(
            diag.contains("/nonexistent/tali/py-abc"),
            "diagnostic must name the resolved interpreter path: {diag}"
        );
    }

    #[tokio::test]
    async fn boot_failure_restores_a_cached_run_range_cell_instead_of_clobbering_it() {
        // Regression: on a kernel BOOT failure, a run-range cell whose output is still
        // validly cached (it's in the range only because a DOWNSTREAM cell must run)
        // must RESTORE that cache, not be overwritten by the "kernel unavailable"
        // diagnostic. Deterministic + no live kernel: a bogus interpreter forces the
        // boot to fail regardless of TALIESIN_PYTHON, and the freeze is pre-seeded with
        // the matching cumulative hash.
        //
        // Precondition: the freeze cache must be live (this test seeds it). Under
        // `TALIESIN_NO_CACHE` the seeded `put` is a no-op so the restore can't happen —
        // skip rather than fail spuriously.
        if std::env::var_os("TALIESIN_NO_CACHE").is_some() {
            eprintln!("SKIPPED: TALIESIN_NO_CACHE disables the freeze cache this test seeds.");
            return;
        }
        let dir = std::env::temp_dir().join(format!("tali-bootclobber-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut ex = Executor::with_freeze(dir.join("page.json"));
        // A bogus interpreter makes `Kernel::start` fail deterministically (a permanent
        // "cannot launch" error, no retry), so `has_kernel` is false -> boot-failed path.
        let bogus = PathBuf::from("/nonexistent/tali-no-python-for-clobber-test");
        ex.python.path = bogus.clone();

        // Seed the freeze so cell A (index 0) is KNOWN but B (index 1) is not, so `plan`
        // puts BOTH in the run range (run_end runs past the last unknown) with A a freeze
        // hit inside it. The interp seed must match what the executor computes for
        // `bogus`: the probe can't run a nonexistent program, so both calls take the
        // fallback and produce the same value (it is NOT memoized -- see
        // `a_failed_interp_probe_is_not_memoized_for_the_process_lifetime`).
        let interp = interp_id("python", &bogus).await;
        let hashes = freeze::cumulative_hashes(&interp, &["a = 1", "b = 2"]);
        ex.freeze
            .put(hashes[0].clone(), "<pre>CACHED_A_OUTPUT</pre>".to_string());

        let blocks = vec![
            python_cell_block_with("a", "a = 1"),
            python_cell_block_with("b", "b = 2"),
        ];
        let out = ex.run(blocks).await;
        let html: String = out.iter().map(|b| b.html.as_str()).collect();

        assert!(
            html.contains("CACHED_A_OUTPUT"),
            "a boot failure must RESTORE cell A's still-valid cached output, not clobber \
             it with the diagnostic: {html}"
        );
        // Cell B has no valid cache -> it still shows the honest kernel-unavailable
        // diagnostic (proving the restore is scoped to genuinely-cached cells).
        assert!(
            html.contains("kernel unavailable"),
            "cell B (no valid cache) should still show the kernel-unavailable diagnostic: {html}"
        );
        // PL6: the executor-state guidance routes to `doctor` and names the Jupyter kernel
        // package, not just the interpreter path (the usual real cause is a missing
        // ipykernel/IRkernel on a perfectly good interpreter).
        let diag = ex.diagnostic().unwrap_or_default();
        assert!(
            diag.contains("taliesin doctor") && diag.contains("Jupyter kernel"),
            "the kernel-unavailable diagnostic must route to doctor + name the kernel package: {diag}"
        );
        let _ = std::fs::remove_dir_all(&dir);
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

        // Item 114's TRUE direction, taken here because this is the one place in the
        // module that has already paid for a live kernel: having run cells, the executor
        // must now report owning one. The false direction is pinned kernel-free in
        // `serve_site::exec_pool`; without this line a `has_live_kernel` stuck at `false`
        // would silence the eviction log entirely and nothing would notice.
        assert!(
            ex.has_live_kernel(),
            "an executor that just ran three python cells owns a kernel"
        );

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
    fn a_running_cell_streams_its_output_before_it_finishes() {
        // 175b's actual claim, and the assertion is on TIMING rather than on order.
        //
        // Order alone cannot prove streaming here: `done` is emitted by the caller
        // after `exec_cell` returns, so even a single flush at the end of `execute`
        // would still land before it and satisfy an index comparison. What only real
        // streaming can produce is appends SPREAD OUT IN TIME, matching the gaps the
        // cell sleeps between prints. A batched flush delivers them microseconds
        // apart. The message shape itself is covered unconditionally in `protocol.rs`.
        if std::env::var_os("TALIESIN_PYTHON").is_none() {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with ipykernel to \
                 exercise output streaming; this run did not."
            );
            return;
        }

        // A sink that stamps each message with its arrival time, so the test can see
        // WHEN an append reached the client, not just that it did.
        let stamped: Arc<Mutex<Vec<(std::time::Instant, serde_json::Value)>>> =
            Arc::new(Mutex::new(Vec::new()));
        let sink: ProgressSink = {
            let stamped = stamped.clone();
            Some(Arc::new(move |m: String| {
                stamped
                    .lock()
                    .push((std::time::Instant::now(), serde_json::from_str(&m).unwrap()));
            }))
        };
        let mut ex = Executor::new();
        ex.set_progress(sink, Some("ch1.tmd".into()));
        // Three prints separated by a real sleep. Under streaming the appends arrive
        // ~150ms apart; under any end-of-cell flush they arrive together.
        let blocks = vec![python_cell_block_with(
            "s-1",
            "import time\nfor i in range(3):\n    print('step', i, flush=True); time.sleep(0.15)",
        )];

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _ = ex.run(blocks).await;
        });
        if ex.diagnostic().is_some() {
            return; // no working kernel here
        }

        let msgs = stamped.lock();
        let appends: Vec<&(std::time::Instant, serde_json::Value)> = msgs
            .iter()
            .filter(|(_, v)| v["type"] == "cell-output-append" && v["cell_id"] == "s-1")
            .collect();

        assert!(
            appends.len() >= 2,
            "expected an append per print, got {}; streaming is not wired up",
            appends.len()
        );
        // The discriminator: real streaming spreads these across the cell's sleeps.
        // 100ms is well above scheduler noise and well below the ~300ms separating
        // the three prints, so this fails loudly if the appends are ever batched.
        let spread = appends[appends.len() - 1].0.duration_since(appends[0].0);
        assert!(
            spread >= std::time::Duration::from_millis(100),
            "all {} appends arrived within {spread:?} of each other, so output is being \
             flushed in one batch rather than streamed as the cell produces it",
            appends.len()
        );
        for (_, v) in &appends {
            assert_eq!(
                v["page"], "ch1.tmd",
                "append is not tagged with its page: {v}"
            );
            assert!(
                v["html"].as_str().unwrap().contains("tali-stream"),
                "append must carry rendered HTML: {v}"
            );
        }
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

    #[test]
    fn mid_run_kernel_death_self_heals_on_the_next_rebuild() {
        // A kernel that dies MID-RUN must not wedge the preview. The cell after the
        // crash gets a KERNEL_DIED placeholder; recording that placeholder into the
        // warm-prefix `ran` would make the next *code-unchanged* rebuild see a full
        // hash match (to_run == 0), so `ensure_kernel` is never called to reap the
        // dead kernel and respawn — serving KERNEL_DIED forever. After the fix the
        // warm prefix is dropped on a mid-run death, so the next rebuild cold-starts
        // a fresh kernel and the post-crash cell heals.
        if std::env::var_os("TALIESIN_PYTHON").is_none() {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with ipykernel to \
                 exercise mid-run kernel-death recovery; this run did not."
            );
            return;
        }

        // A cell that SIGKILLs its kernel the FIRST time it runs, then (once the
        // fresh kernel starts against the now-existing on-disk flag) skips the crash
        // on every later run — a transient mid-run death with byte-identical code
        // across rebuilds. The flag file survives the restart, so the crash fires
        // exactly once. (Linux temp path: no quotes/backslashes to escape.)
        let flag = std::env::temp_dir().join(format!("tali-heal-{}", uuid::Uuid::new_v4()));
        let crash = format!(
            "import os, signal\n\
             if not os.path.exists('{f}'):\n\
             \u{20}   open('{f}', 'w').close()\n\
             \u{20}   os.kill(os.getpid(), signal.SIGKILL)\n",
            f = flag.to_string_lossy(),
        );
        let blocks = vec![
            python_cell_block_with("crash", &crash),
            python_cell_block_with("after", "print('HEALED-MARKER')"),
        ];

        let mut ex = Executor::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Run 1: `crash` kills the kernel; `after` gets a KERNEL_DIED placeholder.
            let _ = ex.run(blocks.clone()).await;
        });
        if ex.diagnostic().is_some() {
            // TALIESIN_PYTHON is set but the kernel never booted — a boot failure
            // leaves `failed_at` set (a mid-run crash does not), so this skips only
            // the genuinely-no-kernel case, never the intentional crash below.
            let _ = std::fs::remove_file(&flag);
            eprintln!("SKIPPED (kernel did not boot): cannot exercise mid-run death.");
            return;
        }

        // Run 2: identical code. The kernel must respawn and `after` must re-run.
        let out = rt.block_on(async { ex.run(blocks.clone()).await });
        let _ = std::fs::remove_file(&flag);

        let after = out
            .iter()
            .find(|b| b.id == "after-out")
            .map(|b| b.html.as_str())
            .unwrap_or("<missing after-out block>");
        assert!(
            !after.contains("kernel exited before this cell ran"),
            "post-crash cell stayed wedged on KERNEL_DIED after a rebuild: {after}"
        );
        assert!(
            after.contains("HEALED-MARKER"),
            "post-crash cell did not re-run on the respawned kernel: {after}"
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
        ex.python.path = PathBuf::from("/nonexistent/taliesin-no-such-python");
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
        ex.python.path = PathBuf::from("/nonexistent/taliesin-no-such-python");
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
            //
            // The bound is deliberately far longer than pre-warming needs (it is well
            // under a second idle). This test was recorded as asserting "on no elapsed
            // time at all", but a bounded poll IS a wall-clock assertion, and at the old
            // 10 s it failed under the full parallel `--bin` suite, where forking a
            // kernel that preloads numpy + matplotlib competes with every other kernel
            // test on the box. What the assertion is actually for is "the pool pre-warms
            // in the background at all", so a generous bound still catches the real
            // regression (it never warms) without failing on a loaded machine.
            let mut ready = false;
            for _ in 0..600 {
                if pool.ready_len().await > 0 {
                    ready = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            assert!(
                ready,
                "warm pool should pre-warm a kernel in the background"
            );

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
        // This is what keeps generated media (audio, a saved figure) beside
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
        plan(
            &h(ran),
            &cur,
            |i| disk.contains(&cur[i].as_str()),
            |_| true,
            None,
        )
    }

    /// Like `run_plan`, but capped: `limit` is one past the last cell this pass may
    /// execute (what `Executor::run_through` derives from a Run-Cell request).
    fn run_plan_capped(ran: &[&str], cur: &[&str], disk: &[&str], limit: usize) -> (usize, usize) {
        let cur = h(cur);
        plan(
            &h(ran),
            &cur,
            |i| disk.contains(&cur[i].as_str()),
            |_| true,
            Some(limit),
        )
    }

    /// Like `run_plan`, but `nocache` lists the cell indices marked `#| cache: false`.
    fn run_plan_nc(ran: &[&str], cur: &[&str], disk: &[&str], nocache: &[usize]) -> (usize, usize) {
        let cur = h(cur);
        plan(
            &h(ran),
            &cur,
            |i| disk.contains(&cur[i].as_str()),
            |i| !nocache.contains(&i),
            None,
        )
    }

    #[test]
    fn capped_plan_runs_the_prefix_the_kernel_lacks_but_stops_at_the_cap() {
        // Cold kernel, nothing cached, "Run cell 2" (limit = 2, i.e. cells 0 and 1).
        // Doc semantics: the cap does NOT skip the prefix — cell 1 needs cell 0's
        // state — it only stops the pass early. Cell 2 stays un-run.
        assert_eq!(run_plan_capped(&[], &["a", "b", "c"], &[], 2), (0, 2));
        // Warm through cell 0; "Run cell 1" then runs exactly one cell.
        assert_eq!(run_plan_capped(&["a"], &["a", "b", "c"], &[], 2), (1, 2));
        // Uncapped, the same state would run everything downstream. This is the
        // whole point of the cap, so pin the contrast rather than trusting it.
        assert_eq!(run_plan(&["a"], &["a", "b", "c"], &[]), (1, 3));
    }

    #[test]
    fn capped_plan_runs_nothing_when_the_warm_prefix_already_passes_the_cap() {
        // Warm kernel already ran [a,b,c]; "Run cell 0" must NOT re-execute cell 0
        // out of order against state built by cells 1-2 (that is exactly Jupyter's
        // hidden-state hazard). An empty range means "already up to date through here".
        let (shared, run_end) = run_plan_capped(&["a", "b", "c"], &["a", "b", "c"], &[], 1);
        assert_eq!(
            shared, run_end,
            "a cap behind the warm prefix must produce an EMPTY run range, got {shared}..{run_end}"
        );
    }

    #[test]
    fn capped_plan_never_extends_past_the_cap_for_a_cache_false_cell() {
        // `cache: false` normally forces `run_end` to the document end so downstream
        // cells re-run against fresh state. Under a cap that extension must still be
        // clipped: the pass may not execute cells the author did not ask for. Cells
        // past the cap keep their invalidated keys and re-run on the next full pass.
        let cur = h(&["a", "b", "c", "d"]);
        let (shared, run_end) = plan(&h(&[]), &cur, |_| true, |i| i != 1, Some(3));
        assert_eq!(
            (shared, run_end),
            (0, 3),
            "the cache:false extension to the document end must be clipped to the cap"
        );
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
    fn plan_cache_false_forces_downstream_disk_hits_to_rerun() {
        // AP4-1 regression: a `#| cache: false` cell re-executes with possibly-different
        // kernel state, so a *cacheable* cell after it must re-run too, even when its own
        // key is a disk-cache hit. The cumulative key can't see the upstream cell's
        // non-determinism, so restoring the downstream cell from disk serves a stale output.

        // Cold build, cell 0 is `cache: false`, cell 1 is cacheable and ON DISK: cell 1
        // must still re-run (run_end reaches the end), not restore stale from disk.
        assert_eq!(run_plan_nc(&[], &["a", "b"], &["b"], &[0]), (0, 2));
        // Warm session, everything unchanged, a `cache: false` middle cell with a
        // disk-cached cell after it: the tail re-runs against fresh state, not from disk.
        assert_eq!(
            run_plan_nc(&["a", "b", "c"], &["a", "b", "c"], &["c"], &[1]),
            (1, 3)
        );
        // A `cache: false` cell with several disk-cached cells after it: everything from
        // the warm-prefix boundary to the end re-runs, no stale tail restore.
        assert_eq!(
            run_plan_nc(
                &["a", "b", "c", "d"],
                &["a", "b", "c", "d"],
                &["c", "d"],
                &[1]
            ),
            (1, 4)
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
            number: "2".into(),
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
            number: "1".into(),
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

    /// A labelled `tbl-` cell whose output is NOT an HTML table must still emit its
    /// anchor, or `@tbl-x` (already rewritten into a link by `apply_table_captions`,
    /// which never sees the output) points at an id nothing emits. Found by the
    /// analyst demand probe: `knitr::kable(format = "html")` prints its markup to
    /// stdout, so the cell's output is a `<pre>` of escaped text and the whole table
    /// carrying the id disappeared — with `check` clean, since `check` never executes.
    #[test]
    fn a_labelled_table_cell_that_prints_text_still_carries_its_anchor() {
        let tbl = CellTable {
            anchor: Some("tbl-coefs".into()),
            caption: Some("Fitted coefficients & SEs".into()),
            number: "3".into(),
        };
        let html = table_wrap(&tbl, "<pre>&lt;table&gt; not really a table</pre>");
        assert!(
            html.contains("id=\"tbl-coefs\""),
            "the anchor must survive a non-table output, else @tbl-coefs dangles: {html}"
        );
        assert!(
            html.contains("<figcaption>Table&nbsp;3: Fitted coefficients &amp; SEs</figcaption>"),
            "the spent number + escaped caption must still be shown: {html}"
        );
        assert!(
            html.contains("<pre>&lt;table&gt; not really a table</pre>"),
            "the cell's own output must be kept verbatim: {html}"
        );
        // The caption leads: a table's caption sits above it, unlike a figure's.
        let cap = html.find("<figcaption>").expect("figcaption present");
        let out = html.find("<pre>").expect("output present");
        assert!(cap < out, "a table caption goes above its content: {html}");
    }

    /// The same fallback covers the no-kernel path, which is how a reader of a built
    /// site most often meets it: the cell never ran, so there is certainly no table,
    /// but the reference in the prose is still a link.
    #[test]
    fn a_table_cell_that_never_ran_still_carries_its_anchor() {
        let tbl = CellTable {
            anchor: Some("tbl-x".into()),
            caption: None,
            number: "1".into(),
        };
        let html = table_wrap(&tbl, "kernel unavailable");
        assert!(html.contains("id=\"tbl-x\""), "{html}");
        assert!(
            html.contains("<figcaption>Table&nbsp;1</figcaption>"),
            "no caption -> bare number, no colon: {html}"
        );
    }

    /// The real-table path is unchanged: the id and caption go *inside* the `<table>`,
    /// not on a wrapper, so existing pages keep their markup.
    #[test]
    fn a_real_table_output_is_still_captioned_in_place() {
        let tbl = CellTable {
            anchor: Some("tbl-cov".into()),
            caption: Some("Coverage".into()),
            number: "2".into(),
        };
        let html = table_wrap(
            &tbl,
            "<div><table border=\"0\"><tr><td>1</td></tr></table></div>",
        );
        assert!(
            html.contains(
                "<table id=\"tbl-cov\" border=\"0\"><caption>Table&nbsp;2: Coverage</caption>"
            ),
            "the in-place caption path regressed: {html}"
        );
        assert!(
            !html.contains("tali-table-figure"),
            "a real table must NOT get the fallback wrapper: {html}"
        );
    }

    #[test]
    fn output_block_wraps_a_labelled_cells_output_in_a_figure() {
        let mut c = cell("b2");
        c.figure = Some(CellFigure {
            anchor: Some("fig-x".into()),
            caption: Some("Cap".into()),
            number: "3".into(),
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
