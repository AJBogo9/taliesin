//! A warm Jupyter kernel: spawned once, kept alive, and reused for every cell
//! execution so there is no per-edit startup cost (Problem 3).
//!
//! We talk the Jupyter ZMQ protocol via `jupyter-zmq-client`: send an
//! `execute_request` on the shell channel, then collect iopub outputs
//! (stream / execute_result / display_data / error) until the kernel returns
//! to `idle` for our message.

use std::io;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use jupyter_protocol::{
    ConnectionInfo, ExecuteRequest, JupyterMessage, JupyterMessageContent, Media, MediaType, Stdio,
    Transport,
};
use jupyter_zmq_client::{
    ClientIoPubConnection, ClientShellConnection, create_client_iopub_connection,
    create_client_shell_connection_with_identity, peek_ports, peer_identity_for_session,
    wait_for_iopub_welcome,
};
use taliesin_core::html_escape as esc;
use tokio::process::{Child, Command};
use tokio::time::{Instant, timeout};

/// Wall-clock cap on a single cell execution, after which the kernel is sent
/// SIGINT (`TALIESIN_CELL_TIMEOUT` seconds, default 120; `0` disables the cap and
/// falls back to a per-output silent-hang backstop). Read once.
fn cell_timeout() -> Option<Duration> {
    static T: OnceLock<Option<Duration>> = OnceLock::new();
    *T.get_or_init(|| {
        let secs = std::env::var("TALIESIN_CELL_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(120);
        (secs > 0).then(|| Duration::from_secs(secs))
    })
}

/// The prefix of every notice this module appends when a cell's output hits a cap
/// (`… at 4096 items]`, `… at 512 KB]`, `… at 8 MB of rich output]`). Defined once here,
/// beside the emitters, because `exec::is_uncacheable` matches on it to keep a truncated
/// output out of the freeze cache: two copies of the literal could drift apart silently and
/// the only symptom would be a silently-cached truncated result.
///
/// Matched in this **bracketed** form on purpose. A bare `taliesin: output truncated` also
/// matches a cell that merely *prints* the phrase, which refuses that cell the cache
/// forever, exactly the false-positive the `tali-error` check was hardened against.
pub(crate) const TRUNCATION_MARKER: &str = "[taliesin: output truncated at ";

/// Total bytes of *rich* output (rendered `ExecuteResult`/`DisplayData`) one cell may
/// accumulate.
///
/// The stream cap counts text bytes and the output cap counts item *count*, so a handful of
/// very large rich outputs sailed under both: a few base64-encoded images are only a few
/// items and no stream bytes at all, yet each is megabytes that then get cloned into the
/// block model, the freeze cache, and the warm-state record, and pushed down every open
/// websocket. 8 MB is far above a legitimate figure (a detailed matplotlib PNG is a few
/// hundred KB base64) while still bounding the blast radius.
const MAX_RICH_BYTES: usize = 8 * 1024 * 1024;

/// Append one rich output, or the truncation notice if it would cross [`MAX_RICH_BYTES`].
///
/// Unlike a stream, a rich output cannot be cut to a prefix: half a data URI or half a
/// `<table>` is broken markup. So an output that crosses the cap is dropped whole and the
/// notice takes its place, which also keeps `is_uncacheable` honest (the truncated result is
/// never frozen).
fn push_rich(outputs: &mut Vec<Output>, rich_bytes: &mut usize, capped: &mut bool, html: String) {
    if *rich_bytes + html.len() > MAX_RICH_BYTES {
        outputs.push(Output::Stream {
            stderr: true,
            text: format!(
                "\n{TRUNCATION_MARKER}{} MB of rich output]\n",
                MAX_RICH_BYTES / (1024 * 1024)
            ),
        });
        *capped = true;
    } else {
        *rich_bytes += html.len();
        outputs.push(Output::Rich(html));
    }
}

/// R's inline graphics device is opened with an **opaque white** background
/// (`repr.plot.bg` defaults to `"white"`), so a figure whose own backgrounds the author
/// made transparent still rasterises onto a white slab — on a page whose default theme
/// is dark. Ask the device for transparency instead: the R counterpart of the
/// `InlineBackend.print_figure_kwargs` facecolor that [`MPL_THEME_PREAMBLE`] already
/// gives Python.
///
/// **This is additive, not a restyle.** A default `ggplot` (whose `theme_grey` paints
/// its own white `plot.background`) and base-R graphics still rasterise opaque —
/// measured through a real kernel, both stay 8-bit RGB with no alpha channel at all.
/// All this removes is the white the *device* painted underneath a figure that had
/// already asked to be transparent. Pinned by `tests/r_kernel.rs`.
const R_TRANSPARENT_DEVICE_PREAMBLE: &str = r#"
options(repr.plot.bg = "transparent")
"#;

/// Python `define(**kwargs)`, run once at kernel start. Serializes each
/// keyword (with a pandas convenience for DataFrame/Series) and emits a
/// `<script type="tali-define">` HTML output the native `{js}` runtime consumes
/// (the Python -> JS bridge). `define` is the native and only author API.
const OJS_DEFINE_PREAMBLE: &str = r#"
def define(**kwargs):
    import json
    try:
        from IPython.display import display, HTML
    except Exception:
        from IPython.core.display import display, HTML
    def convert(v):
        try:
            import pandas as pd
        except ModuleNotFoundError:
            return v
        if type(v) == pd.Series:
            v = pd.DataFrame(v)
        if type(v) == pd.DataFrame:
            j = json.loads(v.T.to_json(orient='split'))
            return dict((k, v) for (k, v) in zip(j["index"], j["data"]))
        return v
    v = dict(contents=list(dict(name=key, value=convert(value)) for (key, value) in kwargs.items()))
    display(HTML('<script type="tali-define">' + json.dumps(v) + '</script>'), metadata=dict(tali_define=True))
globals()["define"] = define
"#;

/// Make inline matplotlib figures follow the page theme **without tainting the
/// author's saved figures**. The previous approach set `InlineBackend.rc` globally,
/// which leaks into `matplotlib.rcParams` and so into any `savefig` the author runs
/// (e.g. a vector figure exported for a LaTeX paper would come out grey-on-
/// transparent instead of black-on-white). Instead we keep global rcParams pristine
/// and theme only the *inline render*:
///
///   - transparency comes from `InlineBackend.print_figure_kwargs` (applied solely
///     when the inline backend rasterises the figure — never global rcParams);
///   - instead of one washed-out neutral grey, we render the figure **twice** —
///     once with the light theme's near-black foreground, once with the dark
///     theme's near-white foreground — by recolouring the figure's artists right
///     before each inline image is produced, then restoring them. The two PNGs are
///     emitted together as a `text/html` fragment whose `.tali-fig-light` /
///     `.tali-fig-dark` images the page swaps on a `data-theme` change, so the axes
///     and text always match the surrounding page exactly (same mechanism as the
///     theme-matched `{{< video >}}`). The standalone `image/png` is suppressed so
///     only the dual-theme HTML is emitted.
///
/// This also powers `#| fig-export:`: the export writes the *pristine* figure to
/// disk (print-clean) at display time, before any inline recolour. Data colours are
/// never touched. The wrap installs lazily (on the first cell that mentions
/// matplotlib) so non-plotting documents pay nothing.
const MPL_THEME_PREAMBLE: &str = r#"
try:
    _ip = get_ipython()
    if _ip is not None:
        # Transparency for the inline image only (not global rcParams).
        _ip.run_line_magic('config', "InlineBackend.print_figure_kwargs = {'facecolor': 'none', 'edgecolor': 'none', 'bbox_inches': 'tight'}")

        # (foreground, grid) per theme — kept in sync with --tali-fg / --tali-border
        # in assets/css/{base,dark}.css.
        _TALI_LIGHT = ('#1a1a1a', '#d0d0d0')
        _TALI_DARK = ('#e6e6e6', '#363a44')
        _tali_pending_export = []
        _tali_orig_png = [None]  # the real Figure->png formatter, captured once

        def _tali_do_export(fig):
            # Write the (still-pristine) figure to the files a `#| fig-export:` cell
            # requested, with print-clean styling for LaTeX/print. PNG gets a print
            # DPI; vector formats (.pdf/.svg) are resolution-independent.
            if not _tali_pending_export:
                return
            import os as _os, sys as _sys
            for _p in list(_tali_pending_export):
                _d = _os.path.dirname(_p)
                if _d:
                    _os.makedirs(_d, exist_ok=True)
                _kw = {'bbox_inches': 'tight', 'facecolor': 'white', 'edgecolor': 'white'}
                if _p.lower().endswith('.png'):
                    _kw['dpi'] = 200
                try:
                    fig.savefig(_p, **_kw)
                except Exception as _e:
                    print('taliesin: fig-export failed for %r: %s' % (_p, _e), file=_sys.stderr)
            _tali_pending_export.clear()

        def _tali_fill_boxes(ax):
            # Data-space rectangles painted by a colour-mapped artist: an image
            # (imshow) or a quad mesh (pcolormesh). These are the regions whose
            # colour comes from the DATA and therefore does not change with the
            # page theme. Deliberately not PolyCollection in general: `fill_between`
            # produces one spanning most of the axes, and over-skipping would leave
            # real chrome baked at one colour.
            boxes = []
            for _im in getattr(ax, 'images', ()):
                try:
                    _x0, _x1, _y0, _y1 = _im.get_extent()
                    boxes.append((min(_x0, _x1), max(_x0, _x1), min(_y0, _y1), max(_y0, _y1)))
                except Exception:
                    pass
            try:
                import matplotlib.collections as _mc
            except Exception:
                return boxes
            for _c in getattr(ax, 'collections', ()):
                if not isinstance(_c, _mc.QuadMesh):
                    continue
                try:
                    if _c.get_array() is None:
                        continue
                    _b = _c.get_datalim(ax.transData)
                    boxes.append((min(_b.x0, _b.x1), max(_b.x0, _b.x1),
                                  min(_b.y0, _b.y1), max(_b.y0, _b.y1)))
                except Exception:
                    pass
            return boxes

        def _tali_texts_on_fill(fig):
            # ids of Text artists sitting INSIDE a colour-mapped fill (item 78).
            #
            # `_tali_recolour` exists because a figure's chrome sits on the
            # transparent page background, so it has to follow the reader's theme.
            # An annotation drawn on top of a heatmap cell is the opposite case: its
            # background is a data colour that is identical in both themes, so
            # forcing it to the page foreground is what MAKES it illegible. Measured:
            # a `1.00` cell is near-black #67000d and the author had written
            # color='white'; the light render turned that white into #1a1a1a.
            #
            # Only `ax.texts` is considered, which is exactly the artists the author
            # added with text()/annotate(). The title, axis labels and tick labels are
            # NOT in that list (they hang off `ax.title` / `ax.xaxis`), so chrome can
            # never be skipped by accident however the axes are laid out.
            _on = set()
            for _ax in fig.axes:
                _boxes = _tali_fill_boxes(_ax)
                if not _boxes:
                    continue
                for _o in list(getattr(_ax, 'texts', ())):
                    try:
                        _x, _y = _o.get_position()
                    except Exception:
                        continue
                    for _x0, _x1, _y0, _y1 in _boxes:
                        if _x0 <= _x <= _x1 and _y0 <= _y <= _y1:
                            _on.add(id(_o))
                            break
            return _on

        def _tali_recolour(fig, fg, grid):
            # Recolour foreground (text/spines/ticks) to `fg` and grid lines to
            # `grid`, and make axes backgrounds transparent. Returns the originals
            # so the figure can be restored exactly. Data colours are untouched, and
            # so is any text sitting ON a data colour (see _tali_texts_on_fill).
            import matplotlib.text as _t
            saved = []
            _on_fill = _tali_texts_on_fill(fig)
            for _o in fig.findobj(_t.Text):
                if id(_o) in _on_fill:
                    continue
                saved.append((_o.set_color, _o.get_color())); _o.set_color(fg)
            for _ax in fig.axes:
                saved.append((_ax.patch.set_facecolor, _ax.patch.get_facecolor())); _ax.patch.set_facecolor('none')
                for _sp in _ax.spines.values():
                    saved.append((_sp.set_edgecolor, _sp.get_edgecolor())); _sp.set_edgecolor(fg)
                for _ln in (*_ax.xaxis.get_ticklines(), *_ax.yaxis.get_ticklines()):
                    saved.append((_ln.set_color, _ln.get_color())); _ln.set_color(fg)
                for _ln in (*_ax.get_xgridlines(), *_ax.get_ygridlines()):
                    saved.append((_ln.set_color, _ln.get_color())); _ln.set_color(grid)
            return saved

        def _tali_render(fig, fg, grid):
            # Produce a base64 PNG (transparent bg) with `fg`/`grid` recolouring,
            # restoring the live figure afterwards.
            _orig = _tali_orig_png[0]
            _saved = _tali_recolour(fig, fg, grid)
            try:
                return _orig(fig)
            finally:
                for _set, _val in reversed(_saved):
                    _set(_val)

        def _tali_ensure_inline():
            # Make sure the inline backend's Figure->png formatter exists, activating
            # it (once) only when it doesn't, so we never reset an existing formatter.
            from matplotlib.figure import Figure
            _png = _ip.display_formatter.formatters.get('image/png')
            if _png is None:
                return
            try:
                _cur = _png.lookup_by_type(Figure)
            except Exception:
                _cur = None
            if _cur is None:
                try:
                    _ip.run_line_magic('matplotlib', 'inline')
                except Exception:
                    pass

        def _tali_install():
            # Register a text/html Figure formatter emitting both theme variants, and
            # suppress the standalone image/png. Idempotent: skips if the html
            # formatter is already ours; re-wraps if something replaced it.
            try:
                from matplotlib.figure import Figure
            except Exception:
                return
            _fmts = getattr(_ip, 'display_formatter', None)
            if _fmts is None:
                return
            _png = _fmts.formatters.get('image/png')
            _html = _fmts.formatters.get('text/html')
            if _png is None or _html is None:
                return
            try:
                _cur_html = _html.lookup_by_type(Figure)
            except Exception:
                _cur_html = None
            if getattr(_cur_html, '_tali_themed', False):
                return
            # Capture the real png formatter (the one matplotlib_inline registered),
            # before we replace it with the suppressor.
            try:
                _real = _png.lookup_by_type(Figure)
            except Exception:
                _real = None
            if _real is not None and not getattr(_real, '_tali_suppress', False):
                _tali_orig_png[0] = _real
            if _tali_orig_png[0] is None:
                return
            def _themed_html(fig):
                if not fig.axes and not fig.lines:
                    return None  # empty figure: emit nothing (matches print_figure)
                _tali_do_export(fig)
                _l = _tali_render(fig, *_TALI_LIGHT)
                _d = _tali_render(fig, *_TALI_DARK)
                if _l is None or _d is None:
                    return None
                return ('<img class="tali-fig tali-fig-light" alt="" src="data:image/png;base64,' + _l + '">'
                        '<img class="tali-fig tali-fig-dark" alt="" src="data:image/png;base64,' + _d + '">')
            _themed_html._tali_themed = True
            _html.for_type(Figure, _themed_html)
            def _suppress(fig):
                return None
            _suppress._tali_suppress = True
            _png.for_type(Figure, _suppress)

        def _tali_export(paths, install=False):
            # Called via a line the executor prepends to a `#| fig-export:` cell.
            _tali_pending_export[:] = [p for p in paths if p]
            if install:
                try:
                    import matplotlib.pyplot  # noqa: F401
                    _tali_ensure_inline()
                    _tali_install()
                except Exception:
                    pass

        def _tali_pre(*_a, **_k):
            _info = _a[0] if _a else None
            _src = getattr(_info, 'raw_cell', '') or ''
            if ('matplotlib' in _src) or ('pyplot' in _src) or ('plt' in _src) or ('seaborn' in _src):
                try:
                    import matplotlib.pyplot  # noqa: F401
                    _tali_ensure_inline()
                    _tali_install()
                except Exception:
                    pass

        _ip.events.register('pre_run_cell', _tali_pre)
except Exception:
    pass
"#;

/// How to launch a Jupyter kernel for one language. The ZMQ protocol is
/// language-agnostic, so only the spawn command, the kernel-spec name, and any
/// startup preambles differ between Python (ipykernel) and R (IRkernel).
pub struct KernelSpec {
    /// The interpreter binary (`python3`, `R`, …).
    program: PathBuf,
    /// `kernel_name` reported in the connection info.
    kernel_name: &'static str,
    /// Builds the process argv given the path to the written connection file
    /// (ipykernel takes `-f <conn>`, IRkernel takes `--args <conn>`).
    argv: fn(&Path) -> Vec<String>,
    /// Code run once at startup (the `ojs_define` bridge + matplotlib theme for
    /// Python; nothing for R yet).
    preambles: &'static [&'static str],
}

impl KernelSpec {
    /// Python via `python -m ipykernel_launcher`.
    pub fn python(program: &Path) -> KernelSpec {
        KernelSpec {
            program: program.to_path_buf(),
            kernel_name: "python3",
            argv: |conn| {
                vec![
                    "-m".into(),
                    "ipykernel_launcher".into(),
                    "-f".into(),
                    conn.display().to_string(),
                    "--quiet".into(),
                    // Reap this kernel if taliesin dies ungracefully (SIGKILL / crash
                    // / closed terminal), where our `Drop` never runs and the kernel
                    // would otherwise orphan — measured: it reparents to a subreaper
                    // (not init) and leaks its /tmp connection dir. ipykernel's
                    // `ParentPollerUnix` polls its parent pid and self-exits once it
                    // changes. It needs our REAL pid, not `1`: ipykernel 7 disables the
                    // poller for `parent_handle == 1` (the old `ppid == init` check is
                    // subreaper-fragile), and our pid arms the robust "ppid changed"
                    // path. Built in the parent before spawn, so `process::id()` is the
                    // kernel's ppid-to-be. Warm-pool kernels reap by a different route
                    // (the forkserver helper, `warm_pool.rs`) and never see this argv.
                    // NOT `PR_SET_PDEATHSIG`: that fires on a parent *thread* exit,
                    // which a tokio worker can do mid-session, killing a live kernel.
                    format!("--IPKernelApp.parent_handle={}", std::process::id()),
                ]
            },
            preambles: &[OJS_DEFINE_PREAMBLE, MPL_THEME_PREAMBLE],
        }
    }

    /// The kernel-spec name (`python3` / `ir`), used to stamp connection files for
    /// the warm-pool's forkserver-spawned kernels just as `start` stamps them
    /// (called from `warm_pool::PoolInner::warm_one`).
    pub(crate) fn kernel_name(&self) -> &'static str {
        self.kernel_name
    }

    /// R via `R --slave -e 'IRkernel::main()'` (the IRkernel kernelspec invocation).
    pub fn r(program: &Path) -> KernelSpec {
        KernelSpec {
            program: program.to_path_buf(),
            kernel_name: "ir",
            argv: |conn| {
                vec![
                    "--slave".into(),
                    "-e".into(),
                    "IRkernel::main()".into(),
                    "--args".into(),
                    conn.display().to_string(),
                ]
            },
            preambles: &[R_TRANSPARENT_DEVICE_PREAMBLE],
        }
    }
}

/// The OS process backing a [`Kernel`].
///
/// A directly-spawned kernel (`python -m ipykernel_launcher`, today's cold path and
/// the warm-pool's eager-preboot fallback) is an [`Owned`](KernelProc::Owned) tokio
/// `Child` we wait/kill directly. A kernel forked from the forkserver warm-pool
/// daemon is a [`Forked`](KernelProc::Forked) bare PID: it is *not* our direct
/// child (the forkserver server reaps it), so liveness, SIGINT, and teardown go
/// through the PID with plain signals — exactly the same primitives `interrupt()`
/// already uses. Either way the ZMQ handshake, preambles, and `execute()` loop are
/// identical, so the rest of `Kernel` never has to care which spawn path produced it.
enum KernelProc {
    Owned(Child),
    /// A forkserver-spawned kernel, addressed by PID. Holding the daemon handle
    /// keeps the forkserver alive for the kernel's lifetime (and lets later
    /// children reuse its warm preloaded image). Constructed by the warm pool
    /// (`Kernel::adopt_forked`), which the preview server and parallel build now
    /// draw their kernels from.
    Forked {
        pid: u32,
        _daemon: std::sync::Arc<crate::warm_pool::ForkserverDaemon>,
    },
}

impl KernelProc {
    /// Liveness without blocking: `try_wait` for an owned child, `kill(pid, 0)` for
    /// a forked PID (ESRCH => gone). Mirrors the old direct-child `is_alive`.
    fn is_alive(&mut self) -> bool {
        match self {
            KernelProc::Owned(child) => matches!(child.try_wait(), Ok(None)),
            KernelProc::Forked { pid, .. } => {
                #[cfg(unix)]
                {
                    // Safety: signal 0 only probes for the process; a stale pid
                    // returns ESRCH (-> false), never touching another process
                    // within the brief window before our Drop kills it.
                    unsafe { libc::kill(*pid as libc::pid_t, 0) == 0 }
                }
                #[cfg(not(unix))]
                {
                    true
                }
            }
        }
    }

    /// The PID for signalling (SIGINT/SIGKILL), if known.
    fn pid(&self) -> Option<u32> {
        match self {
            KernelProc::Owned(child) => child.id(),
            KernelProc::Forked { pid, .. } => Some(*pid),
        }
    }
}

/// One execution output, already rendered to a self-contained HTML fragment.
#[derive(Debug, Clone)]
pub enum Output {
    Stream {
        stderr: bool,
        text: String,
    },
    /// Rich output (execute_result / display_data) rendered to HTML.
    Rich(String),
    Error {
        ename: String,
        evalue: String,
        traceback: Vec<String>,
        /// Set when the **executor** wrote this error about a cell that did not complete,
        /// rather than the interpreter raising about code that ran: one of the
        /// [`crate::exec`] `NOT_RUN_*` kinds. `None` is a genuine traceback, the only case
        /// the console may call an exception.
        ///
        /// A required field rather than an inferred one on purpose. Keying on `ename`
        /// would misread a Python `raise Timeout()` as executor-authored, and inferring
        /// from an empty `traceback` is the same guess with extra steps; making it explicit
        /// means a fourth executor-authored site has to state `None` to lie, instead of
        /// omitting a marker and quietly becoming "raised an uncaught exception".
        not_run: Option<&'static str>,
    },
}

impl Output {
    /// The cell hit its wall-clock cap and was interrupted: it did not finish, and the fix
    /// is `TALIESIN_CELL_TIMEOUT` or the cell, never a traceback the author can read.
    pub(crate) fn timeout(evalue: String) -> Self {
        Output::Error {
            ename: "Timeout".into(),
            evalue,
            traceback: vec![],
            not_run: Some(crate::exec::NOT_RUN_TIMEOUT),
        }
    }

    /// The kernel process exited while this cell was in flight.
    pub(crate) fn kernel_died() -> Self {
        Output::Error {
            ename: "KernelDied".into(),
            evalue: "kernel process exited mid-cell".into(),
            traceback: vec![],
            not_run: Some(crate::exec::NOT_RUN_DIED),
        }
    }
}

/// A live kernel process plus its shell/iopub client connections.
pub struct Kernel {
    proc: KernelProc,
    shell: ClientShellConnection,
    iopub: ClientIoPubConnection,
    conn_dir: PathBuf,
    /// This kernel's wall-clock cap on one cell, resolved once at start from
    /// [`cell_timeout`]. Held per kernel rather than re-read per execution so a test can
    /// set it directly: `cell_timeout` memoizes in a `OnceLock`, so a test that sets
    /// `TALIESIN_CELL_TIMEOUT` only had any effect when it happened to be the first test in
    /// the binary to reach that lock. That ordering dependence is what made
    /// `kernel_executes_state_errors_and_interrupts_runaway_cell` "flaky under load": it
    /// was never about load, it was about who initialised the lock first.
    cell_cap: Option<Duration>,
}

/// Whether a kernel-start failure is worth retrying with a fresh port allocation.
/// Under concurrent starts, `peek_ports` can hand two kernels the same loopback port
/// (it tests-then-releases each), so the loser exits with "address already in use" or
/// an incompatible-sockets error — transient, since a retry re-rolls the ports. A
/// missing interpreter / kernel module is permanent: retrying only delays the honest
/// error. Default to transient (retry) for unrecognized failures, since the harm being
/// fixed is a *silent* drop from a recoverable race, and the permanent cases are named.
pub(crate) fn start_error_is_transient(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    !(m.contains("cannot launch") // spawn failed: the interpreter binary is missing
        || m.contains("no such file") // ditto (the OS error for a missing program)
        || m.contains("no module named")) // the kernel module (ipykernel/IRkernel) is absent
}

/// The error for a kernel process that exited during startup: read its stderr
/// tail (the interpreter's own message, e.g. "No module named ipykernel") so the
/// failure is actionable rather than an opaque connect timeout.
async fn startup_failure(
    spec: &KernelSpec,
    stderr: Option<tokio::process::ChildStderr>,
) -> io::Error {
    let mut buf = Vec::new();
    if let Some(mut e) = stderr {
        use tokio::io::AsyncReadExt;
        let _ = e.read_to_end(&mut buf).await;
    }
    let err = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = err.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = lines[lines.len().saturating_sub(3)..].join("; ");
    io::Error::other(format!(
        "`{}` exited at startup: {}",
        spec.program.display(),
        if tail.is_empty() {
            format!(
                "no output (is the {} kernel module installed?)",
                spec.kernel_name
            )
        } else {
            tail
        }
    ))
}

/// Peek 5 free loopback ports and write a locked-down `connection.json` for a
/// kernel of `kernel_name`. The connection file holds the HMAC key + ZMQ ports —
/// anyone who can read it can drive the kernel — so it lives in a 0700 temp dir as
/// a 0600 file, created with those modes from the start (no world-readable window)
/// on Unix. Returns the connection info, the temp dir (owned by the `Kernel` so it
/// is removed on drop), and the connection-file path.
///
/// Shared verbatim by the direct-spawn (`Kernel::start`) and forkserver warm-pool
/// paths so both produce identical, equally-secured connection files.
pub(crate) async fn prepare_connection(
    kernel_name: &str,
) -> io::Result<(ConnectionInfo, PathBuf, PathBuf)> {
    let ip: IpAddr = "127.0.0.1".parse().unwrap();
    let ports = peek_ports(ip, 5).await.map_err(io::Error::other)?;
    let info = ConnectionInfo {
        ip: ip.to_string(),
        transport: Transport::TCP,
        shell_port: ports[0],
        iopub_port: ports[1],
        stdin_port: ports[2],
        control_port: ports[3],
        hb_port: ports[4],
        key: uuid::Uuid::new_v4().to_string(),
        signature_scheme: "hmac-sha256".to_string(),
        kernel_name: Some(kernel_name.to_string()),
    };

    // Pid-tagged so a later run can reclaim it if we die ungracefully (`Drop` won't
    // run to remove it). See `runtime_dirs`.
    let conn_dir = crate::runtime_dirs::kernel_conn_dir();
    {
        let mut b = std::fs::DirBuilder::new();
        b.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        b.create(&conn_dir)?;
    }
    let conn_file = conn_dir.join("connection.json");
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        opts.open(&conn_file)?
            .write_all(&serde_json::to_vec(&info)?)?;
    }
    Ok((info, conn_dir, conn_file))
}

/// Wait until the kernel's shell + iopub ports accept a TCP connection, i.e. the
/// kernel has bound its ZMQ sockets. The pure-Rust `zeromq` client's `connect`
/// eagerly establishes the TCP link and, if the endpoint isn't listening yet, can
/// burn its full 30s connect timeout before erroring — so for a freshly *forked*
/// kernel (which binds a beat after the daemon reports its PID) we must let it
/// finish binding first. A directly-spawned kernel races connect against the child
/// exiting, so it doesn't need this; the fork path has no owned child to race.
///
/// Cheap fast-failing `TcpStream::connect` probes (not ZMQ) poll until both ports
/// listen or `deadline` passes; the subsequent ZMQ handshake then completes
/// immediately instead of fighting the connect-retry backoff.
async fn wait_until_reachable(info: &ConnectionInfo, deadline: Instant) -> io::Result<()> {
    use tokio::net::TcpStream;
    let ip: IpAddr = info.ip.parse().map_err(io::Error::other)?;
    let ports = [info.shell_port, info.iopub_port];
    loop {
        let mut all = true;
        for p in ports {
            // A short per-probe timeout so a refused/hung port retries quickly.
            match timeout(Duration::from_millis(200), TcpStream::connect((ip, p))).await {
                Ok(Ok(_stream)) => {} // listening; the probe socket drops immediately
                _ => {
                    all = false;
                    break;
                }
            }
        }
        if all {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other(
                "forked kernel did not bind its ZMQ ports in time",
            ));
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// Connect both ZMQ channels (iopub + shell) to a kernel described by `info` and
/// wait for the iopub welcome. Reading one iopub message confirms the SUB
/// subscription is live, sidestepping the ZMQ slow-joiner problem before the first
/// execution. Shared by every spawn path.
async fn connect_handshake(
    info: &ConnectionInfo,
) -> io::Result<(ClientIoPubConnection, ClientShellConnection)> {
    let session = uuid::Uuid::new_v4().to_string();
    let mut iopub = create_client_iopub_connection(info, "", &session)
        .await
        .map_err(io::Error::other)?;
    let identity = peer_identity_for_session(&session).map_err(io::Error::other)?;
    let shell = create_client_shell_connection_with_identity(info, &session, identity)
        .await
        .map_err(io::Error::other)?;
    let _ = wait_for_iopub_welcome(&mut iopub, Duration::from_secs(5)).await;
    Ok((iopub, shell))
}

impl Kernel {
    /// [`Kernel::start`], re-rolling the port allocation on a *transient* failure.
    ///
    /// [`prepare_connection`] peeks free ports by binding then immediately releasing
    /// them, so two kernels starting concurrently can be handed the same loopback port
    /// and the loser exits with `zmq.error.ZMQError: Address already in use`. Every
    /// caller needs that re-roll, so it lives with the primitive that allocates the
    /// ports rather than in the callers: `exec.rs` and `warm_pool.rs` each grew a
    /// private copy, and the one caller without one — the child half of
    /// `cold_kernel_self_reaps_on_ungraceful_parent_death`, which cold-starts a kernel
    /// in a re-spawned test binary — inherited the race and flaked. Captured
    /// 2026-07-25 by looping the `--bin` suite; before that the flake was recorded
    /// against an unrelated interrupt test and theorized as a timing edge, which is
    /// why it survived a "fix".
    ///
    /// A permanent failure (missing interpreter or kernel module) returns at once:
    /// retrying cannot help and would only delay the honest error.
    pub async fn start_with_retry(spec: &KernelSpec, cwd: Option<&Path>) -> io::Result<Kernel> {
        const ATTEMPTS: usize = 4;
        let mut attempt = 1;
        loop {
            match Kernel::start(spec, cwd).await {
                Ok(k) => return Ok(k),
                Err(e) if attempt < ATTEMPTS && start_error_is_transient(&e.to_string()) => {
                    crate::log::warn(&format!(
                        "{} kernel start hit a transient failure ({e}); retrying ({attempt}/{ATTEMPTS})",
                        spec.kernel_name
                    ));
                    // Attempt-scaled backoff: let the peer that won the port bind first.
                    tokio::time::sleep(Duration::from_millis(40 * attempt as u64)).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Spawn the kernel described by `spec` (Python ipykernel or R IRkernel) and
    /// connect to it. The kernel stays warm for the lifetime of this value.
    ///
    /// `cwd` is the kernel process's working directory: a cell's relative file I/O
    /// (`scipy.io.wavfile.write`, a `#| fig-export:` `savefig`, R's `ggsave`)
    /// resolves against it, so generated media lands beside the document rather
    /// than wherever the server was launched. `None` inherits the server's cwd.
    pub async fn start(spec: &KernelSpec, cwd: Option<&Path>) -> io::Result<Kernel> {
        let (info, conn_dir, conn_file) = prepare_connection(spec.kernel_name).await?;
        // The 0700 `/tmp/tali-kernel-<uuid>` dir has no owner until the handshake
        // succeeds and the live `Kernel` (whose `Drop` removes it) takes over: any
        // early `?`/`return` below would drop the `PathBuf` and leak the dir. Guard
        // it, then disarm once the kernel is built. (`kill_on_drop` below is the
        // process half: a `child` dropped on the connect-fail path is SIGKILL'd
        // rather than left running against its now-orphaned ports.)
        let dir_guard = ConnDirGuard::arm(conn_dir);

        // Capture stderr so a startup failure (e.g. the interpreter lacks the
        // ipykernel/IRkernel module) can be reported instead of swallowed.
        let mut cmd = Command::new(&spec.program);
        cmd.args((spec.argv)(&conn_file))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        // Run cells in the document's directory so their relative file writes land
        // beside the source (the connection file is an absolute path, so this
        // doesn't disturb the ZMQ handshake).
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let mut child = cmd.spawn().map_err(|e| {
            io::Error::other(format!(
                "cannot launch `{}`: {e} (is it installed / on PATH?)",
                spec.program.display()
            ))
        })?;
        let child_stderr = child.stderr.take();

        // Connect over ZMQ, racing the handshake against the process exiting so a
        // bad interpreter fails fast with its actual stderr instead of the full
        // connect timeout.
        let connect = connect_handshake(&info);
        let (iopub, shell) = tokio::select! {
            r = connect => r?,
            _ = child.wait() => {
                return Err(startup_failure(spec, child_stderr).await);
            }
        };

        // Handshake succeeded: hand the dir to the `Kernel`, which now owns teardown.
        let conn_dir = dir_guard.disarm();
        Kernel::finish(KernelProc::Owned(child), conn_dir, iopub, shell, spec).await
    }

    /// Adopt an already-spawned kernel **PID** (forked from the warm-pool
    /// forkserver daemon) by completing the same ZMQ handshake + preambles
    /// `start` runs for a directly-spawned child. `info`/`conn_dir` are the
    /// connection this PID was launched against; `daemon` is kept alive for the
    /// kernel's lifetime so the forkserver stays warm.
    ///
    /// Unlike `start`, there is no owned `Child` to race the connect against, so a
    /// dead fork surfaces as a connect timeout. The warm-pool spawns these eagerly
    /// off the hot path, so that latency is hidden; callers still treat a `None`
    /// pool result as "fall back to `start`".
    pub(crate) async fn adopt_forked(
        pid: u32,
        info: ConnectionInfo,
        conn_dir: PathBuf,
        daemon: std::sync::Arc<crate::warm_pool::ForkserverDaemon>,
        spec: &KernelSpec,
    ) -> io::Result<Kernel> {
        // Until ownership passes to a live `Kernel`, the forked PID + its /tmp
        // connection dir have no owner: if the handshake below fails (a dead fork, or
        // ports that never bind), an early `?` return would leak both. Guard them,
        // then disarm once the connection succeeds and the `Kernel` takes over.
        let guard = ForkedCleanup {
            pid,
            conn_dir: Some(conn_dir),
        };
        // The forked kernel binds its ZMQ ports a beat after the daemon reports its
        // PID; wait for them to listen so the ZMQ connect succeeds immediately
        // rather than burning the client's 30s connect timeout.
        wait_until_reachable(&info, Instant::now() + Duration::from_secs(15)).await?;
        let (iopub, shell) = connect_handshake(&info).await?;
        // From here to the `Kernel` construction inside `finish` must stay infallible:
        // once disarmed, the pid + dir are only re-homed when the `Kernel` struct owns
        // them, so a `?` inserted in this window would leak both. Keep it fallible-free.
        let conn_dir = guard.disarm();
        let proc = KernelProc::Forked {
            pid,
            _daemon: daemon,
        };
        Kernel::finish(proc, conn_dir, iopub, shell, spec).await
    }

    /// Shared tail of every spawn path: build the `Kernel` and run each startup
    /// preamble once against the now-live kernel.
    async fn finish(
        proc: KernelProc,
        conn_dir: PathBuf,
        iopub: ClientIoPubConnection,
        shell: ClientShellConnection,
        spec: &KernelSpec,
    ) -> io::Result<Kernel> {
        let mut kernel = Kernel {
            proc,
            shell,
            iopub,
            conn_dir,
            cell_cap: cell_timeout(),
        };
        // Language-specific startup (e.g. Python's `ojs_define` bridge + matplotlib
        // theme); each preamble runs once against the warm kernel.
        for preamble in spec.preambles {
            let _ = kernel.execute(preamble).await;
        }
        Ok(kernel)
    }

    /// Whether the kernel process is still alive — a cheap, non-blocking check
    /// (`try_wait`) used to detect a kernel that died mid-session so it can be
    /// respawned instead of hanging on the next execute's timeout.
    pub fn is_alive(&mut self) -> bool {
        self.proc.is_alive()
    }

    /// Run `code` and collect its outputs (waits until the kernel is idle).
    pub async fn execute(&mut self, code: &str) -> io::Result<Vec<Output>> {
        let request = JupyterMessage::new(
            JupyterMessageContent::ExecuteRequest(ExecuteRequest::new(code.to_string())),
            None,
        );
        let msg_id = request.header.msg_id.clone();
        self.shell.send(request).await.map_err(io::Error::other)?;

        let mut outputs: Vec<Output> = Vec::new();
        // Caps so a cell that emits a huge amount of output can't hang the renderer
        // or blow memory (the output is later cloned into the block, the freeze
        // cache, and the warm-state record, and HTML-escaped). We keep *draining* to
        // Idle to stay in channel sync, but stop accumulating past the caps.
        const MAX_STREAM_BYTES: usize = 512 * 1024;
        const MAX_OUTPUTS: usize = 4096;
        let mut stream_bytes = 0usize;
        let mut rich_bytes = 0usize;
        let mut capped = false;
        // Total wall-clock cap (not per-message, so a *streaming* runaway cell is
        // still caught). On hitting it we SIGINT the kernel, then drain a short
        // grace window so the resulting KeyboardInterrupt + Idle resync the
        // channels and the *next* cell still works.
        let cap = self.cell_cap;
        let deadline = cap.map(|d| Instant::now() + d);
        let mut grace_until: Option<Instant> = None;
        // Last time any iopub message arrived; drives the uncapped "no output for 60s"
        // budget so it resets on every output (matching the old per-`read` timeout).
        let mut last_msg = Instant::now();
        loop {
            let now = Instant::now();
            // Time left before this cell's REAL deadline: the post-interrupt grace window,
            // the hard cap, or — when uncapped — 60s of silence since the last output.
            let budget = match grace_until {
                Some(g) => g.saturating_duration_since(now),
                None => match deadline {
                    Some(dl) => dl.saturating_duration_since(now),
                    None => Duration::from_secs(60).saturating_sub(now.duration_since(last_msg)),
                },
            };
            // Poll on a short interval (capped at the budget) so a kernel that EXITS
            // mid-cell is noticed within ~1s and reported as a distinct `KernelDied`,
            // instead of blocking the full budget and then mislabeling the crash as
            // "Timeout" (and interrupting a corpse). A healthy long cell just re-polls.
            let poll = budget.min(Duration::from_secs(1));
            let msg = match timeout(poll, self.iopub.read()).await {
                Ok(Ok(msg)) => {
                    last_msg = Instant::now();
                    msg
                }
                Ok(Err(e)) => return Err(io::Error::other(e)),
                Err(_) => {
                    // No output this interval. Did the kernel process die?
                    if !self.is_alive() {
                        outputs.push(Output::kernel_died());
                        break;
                    }
                    // Still alive: only act once the REAL budget (not just a poll) is spent.
                    if !budget.is_zero() {
                        continue;
                    }
                    if grace_until.is_some() {
                        // Ignored SIGINT within the grace window; give up on this cell. The
                        // channels may be desynced — the dev-menu "Restart kernel" is the
                        // escape hatch.
                        break;
                    }
                    match cap {
                        // Hit the hard cap: interrupt and switch to the grace window.
                        Some(d) => {
                            self.interrupt();
                            outputs.push(Output::timeout(format!(
                                "cell exceeded {}s; sent interrupt",
                                d.as_secs()
                            )));
                            grace_until = Some(Instant::now() + Duration::from_secs(5));
                            continue;
                        }
                        // No cap (opt-out): a silent hang still times out per-output.
                        None => {
                            outputs.push(Output::timeout(
                                "cell produced no output for 60s".to_string(),
                            ));
                            break;
                        }
                    }
                }
            };
            // Only messages parented by our request belong to this execution.
            let ours = msg.parent_header.as_ref().map(|h| h.msg_id.as_str()) == Some(&msg_id);
            if !ours {
                continue;
            }
            // Past the item cap, stop accumulating (but keep draining): emit one
            // marker. Only an *output-producing* message trips this — not an Error or
            // the terminal Idle Status — so a cell that emits exactly MAX_OUTPUTS items
            // and then finishes cleanly is not falsely marked as truncated.
            let accumulating = matches!(
                &msg.content,
                JupyterMessageContent::StreamContent(_)
                    | JupyterMessageContent::ExecuteResult(_)
                    | JupyterMessageContent::DisplayData(_)
            );
            if !capped && accumulating && outputs.len() >= MAX_OUTPUTS {
                outputs.push(Output::Stream {
                    stderr: true,
                    text: format!("\n{TRUNCATION_MARKER}{MAX_OUTPUTS} items]\n"),
                });
                capped = true;
            }
            match msg.content {
                JupyterMessageContent::StreamContent(s) if !capped => {
                    let stderr = matches!(s.name, Stdio::Stderr);
                    let remaining = MAX_STREAM_BYTES.saturating_sub(stream_bytes);
                    if s.text.len() <= remaining {
                        stream_bytes += s.text.len();
                        outputs.push(Output::Stream {
                            stderr,
                            text: s.text,
                        });
                    } else {
                        // Keep a char-boundary-safe prefix, then mark + stop.
                        let mut cut = remaining;
                        while cut > 0 && !s.text.is_char_boundary(cut) {
                            cut -= 1;
                        }
                        if cut > 0 {
                            outputs.push(Output::Stream {
                                stderr,
                                text: s.text[..cut].to_string(),
                            });
                        }
                        outputs.push(Output::Stream {
                            stderr: true,
                            text: format!("\n{TRUNCATION_MARKER}{} KB]\n", MAX_STREAM_BYTES / 1024),
                        });
                        capped = true;
                    }
                }
                JupyterMessageContent::ExecuteResult(r) if !capped => push_rich(
                    &mut outputs,
                    &mut rich_bytes,
                    &mut capped,
                    render_media(&r.data),
                ),
                JupyterMessageContent::DisplayData(d) if !capped => push_rich(
                    &mut outputs,
                    &mut rich_bytes,
                    &mut capped,
                    render_media(&d.data),
                ),
                // The interpreter raising about code that ran: a real traceback, so no
                // not-run marker. This is the ONE site that may leave it `None`.
                JupyterMessageContent::ErrorOutput(e) => outputs.push(Output::Error {
                    ename: e.ename,
                    evalue: e.evalue,
                    traceback: e.traceback,
                    not_run: None,
                }),
                JupyterMessageContent::Status(st) => {
                    if matches!(st.execution_state, jupyter_protocol::ExecutionState::Idle) {
                        break;
                    }
                }
                _ => {}
            }
            // Once capped, interrupt the kernel so it stops flooding us (a huge-output
            // cell otherwise keeps streaming megabytes we'd have to read + discard,
            // and the per-message receive is super-linear). Then drain a short grace
            // window for the resulting KeyboardInterrupt + Idle and stop.
            if capped && grace_until.is_none() {
                self.interrupt();
                grace_until = Some(Instant::now() + Duration::from_secs(5));
            }
        }
        // Drain *our* shell execute_reply so the channel stays in sync. Match on
        // msg_id: after an interrupt a previous cell's late reply can still be in the
        // queue, and consuming it here would leave every later cell one reply behind.
        let drain_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let budget = drain_deadline.saturating_duration_since(Instant::now());
            if budget.is_zero() {
                break;
            }
            match timeout(budget, self.shell.read()).await {
                Ok(Ok(reply)) => {
                    if reply.parent_header.as_ref().map(|h| h.msg_id.as_str()) == Some(&msg_id) {
                        break;
                    }
                    // A stale (non-matching) reply: discard and keep draining.
                }
                _ => break, // timeout or read error: give up draining
            }
        }
        Ok(outputs)
    }

    /// Send SIGINT to the kernel process: the `interrupt_mode: signal` path that
    /// raises `KeyboardInterrupt` in the running cell (ipykernel and IRkernel both
    /// honour it), stopping a runaway cell while the warm kernel and prior cell
    /// state survive. Unix-only; a no-op (the cap still ends the wait) elsewhere.
    fn interrupt(&self) {
        #[cfg(unix)]
        if let Some(pid) = self.proc.pid() {
            // Safety: `kill` with a valid pid + signal is sound; a stale pid just
            // returns ESRCH, which we ignore. For a forkserver child this is the
            // exact same `kill(pid, SIGINT)` an owned child gets — the fork-spawn
            // path keeps runaway-cell interruption working identically.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGINT);
            }
        }
    }
}

/// Cleanup guard for [`Kernel::start`]: the 0700 `/tmp/tali-kernel-<uuid>` connection
/// dir created by [`prepare_connection`] has no owner until the ZMQ handshake succeeds
/// and the live [`Kernel`] (whose `Drop` removes it) takes over. An early return on any
/// startup failure would otherwise drop the `PathBuf` and leak the dir. This guard
/// removes it on drop and is `disarm`ed on the success path. The kernel *process* is
/// handled separately (`kill_on_drop` on the spawn command); the sibling
/// [`ForkedCleanup`] guards the fork path, which additionally SIGKILLs a non-child pid.
///
/// Also used by the warm pool's `warm_one`, the step *between* these two guards: it
/// creates the connection dir and then forks, so it owns the dir for exactly the window
/// where no kernel does. It is the same "dir with no owner yet" hazard, so it reuses
/// this guard rather than growing a third one.
pub(crate) struct ConnDirGuard {
    conn_dir: Option<PathBuf>,
}

impl ConnDirGuard {
    /// Arm the guard over a freshly-[`prepare_connection`]ed dir: from here until
    /// `disarm`, any early return removes it.
    pub(crate) fn arm(conn_dir: PathBuf) -> ConnDirGuard {
        ConnDirGuard {
            conn_dir: Some(conn_dir),
        }
    }

    /// Hand the connection dir to the now-live kernel and defuse the guard.
    pub(crate) fn disarm(mut self) -> PathBuf {
        self.conn_dir.take().expect("conn_dir present until disarm")
    }
}

impl Drop for ConnDirGuard {
    fn drop(&mut self) {
        if let Some(dir) = self.conn_dir.take() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// Cleanup guard for [`Kernel::adopt_forked`]: until the forked kernel's ownership
/// passes to a live [`Kernel`] (whose own `Drop` then owns teardown), the PID + its
/// `/tmp` connection dir have no owner. If the ZMQ handshake fails (a dead fork, or
/// ports that never bind), an early return would leak both. This guard mirrors the
/// `Kernel` `Drop` below (SIGKILL the PID + remove the dir) and is `disarm`ed on the
/// success path so the live kernel keeps its process and dir.
struct ForkedCleanup {
    pid: u32,
    conn_dir: Option<PathBuf>,
}

impl ForkedCleanup {
    /// Hand the connection dir back to the caller (to pass to `Kernel::finish`) and
    /// defuse the guard, so the now-live kernel keeps its process + dir.
    fn disarm(mut self) -> PathBuf {
        self.conn_dir.take().expect("conn_dir present until disarm")
    }
}

impl Drop for ForkedCleanup {
    fn drop(&mut self) {
        let Some(dir) = self.conn_dir.take() else {
            return; // disarmed: the live Kernel owns teardown now
        };
        // Not our direct child (the forkserver reaps it), so signal by PID —
        // identical to the `Kernel` Drop's SIGKILL teardown of a forked kernel.
        #[cfg(unix)]
        unsafe {
            libc::kill(self.pid as libc::pid_t, libc::SIGKILL);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

impl Drop for Kernel {
    fn drop(&mut self) {
        match &mut self.proc {
            KernelProc::Owned(child) => {
                let _ = child.start_kill();
            }
            KernelProc::Forked { pid, .. } => {
                // Not our direct child (the forkserver server reaps it), so we
                // can't `start_kill`; signal it to exit by PID. SIGKILL is the
                // teardown analogue of the owned child's `start_kill`.
                #[cfg(unix)]
                unsafe {
                    libc::kill(*pid as libc::pid_t, libc::SIGKILL);
                }
            }
        }
        let _ = std::fs::remove_dir_all(&self.conn_dir);
    }
}

/// Render outputs into an HTML fragment (the inner content of an output block),
/// or empty if there are none. The caller wraps this in the block element.
pub fn render_outputs(outputs: &[Output]) -> String {
    let mut s = String::new();
    for o in outputs {
        match o {
            Output::Stream { stderr, text } => {
                let class = if *stderr {
                    "tali-stream tali-stderr"
                } else {
                    "tali-stream"
                };
                s.push_str(&format!(
                    "<pre class=\"{class}\">{}</pre>",
                    esc(&scrub_kernel_paths(&strip_ansi(text)))
                ));
            }
            Output::Rich(html) => s.push_str(html),
            Output::Error {
                ename,
                evalue,
                traceback,
                not_run,
            } => {
                let tb: String = traceback
                    .iter()
                    .map(|l| strip_ansi(l))
                    .collect::<Vec<_>>()
                    .join("\n");
                let body = if tb.trim().is_empty() {
                    format!("{ename}: {evalue}")
                } else {
                    tb
                };
                // An executor-authored error is the same HTML shape as a traceback on
                // purpose (styled as an error, never cached), so the marker is what tells
                // the console apart — without it a timeout-killed cell was reported as
                // "raised an uncaught exception", which is false twice over.
                let mark = not_run.map(crate::exec::not_run_mark).unwrap_or_default();
                s.push_str(&format!(
                    "<pre class=\"tali-error\"{mark}>{}</pre>",
                    esc(&body)
                ));
            }
        }
    }
    s
}

/// Pick the richest available representation of a rich output and render it.
fn render_media(media: &Media) -> String {
    let c = &media.content;
    let pick = |f: &dyn Fn(&MediaType) -> Option<String>| c.iter().find_map(f);

    if let Some(h) = pick(&|t| match t {
        MediaType::Html(h) => Some(h.clone()),
        _ => None,
    }) {
        return h;
    }
    if let Some(b) = pick(&|t| match t {
        MediaType::Png(b) => Some(b.clone()),
        _ => None,
    }) {
        return format!(
            "<img alt=\"output\" src=\"data:image/png;base64,{}\" />",
            b.trim()
        );
    }
    if let Some(s) = pick(&|t| match t {
        MediaType::Svg(s) => Some(s.clone()),
        _ => None,
    }) {
        return s;
    }
    if let Some(b) = pick(&|t| match t {
        MediaType::Jpeg(b) => Some(b.clone()),
        _ => None,
    }) {
        return format!(
            "<img alt=\"output\" src=\"data:image/jpeg;base64,{}\" />",
            b.trim()
        );
    }
    if let Some(t) = pick(&|t| match t {
        MediaType::Plain(t) => Some(t.clone()),
        _ => None,
    }) {
        return format!("<pre>{}</pre>", esc(&t));
    }
    String::new()
}

/// Replace a Jupyter cell's non-deterministic source path with a stable `<cell>` marker.
/// An executed cell's stream — matplotlib's Agg `UserWarning`, any `warnings.warn`, or a
/// `print(__file__)` — cites the kernel's per-process temp file
/// `<tmpdir>/ipykernel_<PID>/<HASH>.py`. The PID (and hash across machines) change every
/// run, so leaving it in leaks a local absolute path into the published HTML AND makes
/// cold/CI/cross-machine builds non-reproducible (AP8-1). The IPython *traceback* arm
/// already reads `Cell In[N]` (IPython rewrites the frame filename), so only these stream
/// paths and the legacy `<ipython-input-…>` form remain. Mirrors nbconvert/Quarto; the
/// trailing `:<line>:` is deterministic (the cell's own line) and is kept. Applies to every
/// stream regardless of language — R streams simply carry no such path to match.
fn scrub_kernel_paths(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        // Legacy `<ipython-input-…>` (older IPython): replace the whole `<…>` token.
        if let Some(after) = rest.strip_prefix("<ipython-input-")
            && let Some(gt) = after.find('>')
        {
            out.push_str("<cell>");
            rest = &after[gt + 1..];
            continue;
        }
        // `<tmpdir>/ipykernel_<digits>/<digits>.py`: anchor on `ipykernel_`, match the tail
        // forward, then drop the already-emitted tmpdir prefix back to a whitespace boundary
        // so the whole absolute path token (not just the filename) becomes `<cell>`.
        if rest.starts_with("ipykernel_")
            && let Some(tail) = ipykernel_tail_len(rest)
        {
            let keep = out.trim_end_matches(|c: char| !c.is_whitespace()).len();
            out.truncate(keep);
            out.push_str("<cell>");
            rest = &rest[tail..];
            continue;
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}

/// If `s` begins with `ipykernel_<digits>/<digits>.py`, the byte length of that match, else
/// `None` (so `ipykernel_launcher` and other non-cell tokens are left untouched).
fn ipykernel_tail_len(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let after_pid = take_ascii_digits(b, "ipykernel_".len())?;
    if b.get(after_pid) != Some(&b'/') {
        return None;
    }
    let after_hash = take_ascii_digits(b, after_pid + 1)?;
    s[after_hash..].starts_with(".py").then_some(after_hash + 3)
}

/// Advance over one-or-more ASCII digits from `i`; `None` if there is not at least one.
fn take_ascii_digits(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    while b.get(j).is_some_and(u8::is_ascii_digit) {
        j += 1;
    }
    (j > i).then_some(j)
}

/// Strip ANSI SGR escape sequences (IPython colourises tracebacks).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            // Skip until the terminating letter of the escape sequence.
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_start_errors_retry_but_missing_interpreter_does_not() {
        // A port-allocation race / socket reuse under concurrent kernel starts is
        // transient: a retry with a fresh port allocation typically succeeds, so the
        // page never silently loses its cell output. A missing interpreter or kernel
        // module is permanent: retrying only wastes time, so it must fail fast.
        assert!(start_error_is_transient(
            "`python` exited at startup: zmq.error.ZMQError: Address already in use (addr='tcp://127.0.0.1:43049')"
        ));
        assert!(start_error_is_transient(
            "python kernel unavailable (Provided sockets combination is not compatible)"
        ));
        assert!(start_error_is_transient(
            "forked kernel did not bind its ZMQ ports in time"
        ));
        assert!(
            !start_error_is_transient(
                "cannot launch `/nonexistent/python`: No such file or directory (is it installed / on PATH?)"
            ),
            "a missing interpreter is permanent"
        );
        assert!(
            !start_error_is_transient(
                "`python` exited at startup: ModuleNotFoundError: No module named 'ipykernel'"
            ),
            "a missing kernel module is permanent"
        );
    }

    // `render_outputs` turns kernel messages into the output block's inner HTML.
    // It's pure, so it's covered here unconditionally — the live-kernel test below
    // needs an interpreter and is skipped without one, so this is what guarantees
    // the output-formatting path stays green in CI.
    #[test]
    fn render_outputs_formats_streams_rich_and_errors() {
        assert_eq!(render_outputs(&[]), "", "no outputs -> empty fragment");

        // stdout vs stderr get distinct classes; text is HTML-escaped.
        let out = render_outputs(&[Output::Stream {
            stderr: false,
            text: "a < b\n".into(),
        }]);
        assert_eq!(out, "<pre class=\"tali-stream\">a &lt; b\n</pre>");
        let err = render_outputs(&[Output::Stream {
            stderr: true,
            text: "oops".into(),
        }]);
        assert!(
            err.contains("class=\"tali-stream tali-stderr\""),
            "got: {err}"
        );

        // Rich output is already HTML and passes through verbatim (not escaped).
        assert_eq!(
            render_outputs(&[Output::Rich("<table><tr><td>1</td></tr></table>".into())]),
            "<table><tr><td>1</td></tr></table>"
        );

        // An error with no traceback falls back to "ename: evalue".
        let bare = render_outputs(&[Output::Error {
            ename: "ValueError".into(),
            evalue: "bad".into(),
            traceback: vec![],
            not_run: None,
        }]);
        assert_eq!(bare, "<pre class=\"tali-error\">ValueError: bad</pre>");

        // A traceback is ANSI-stripped, joined, and escaped.
        let tb = render_outputs(&[Output::Error {
            ename: "E".into(),
            evalue: "v".into(),
            traceback: vec!["\u{1b}[31mline 1\u{1b}[0m".into(), "a < b".into()],
            not_run: None,
        }]);
        assert!(tb.contains("class=\"tali-error\""), "got: {tb}");
        assert!(tb.contains("line 1"), "ansi not stripped: {tb}");
        assert!(!tb.contains("\u{1b}["), "raw ANSI leaked: {tb}");
        assert!(tb.contains("a &lt; b"), "traceback not escaped: {tb}");
    }

    // Regression: R's `message()`/`warning()` (and Python `rich`/coloured output)
    // write ANSI SGR codes to a *stream*, not just to a traceback. The error path
    // already strips them; the stream path must match, or the codes leak into the
    // page as visible `[31m…[0m` garbage (the ESC char is invisible, its argument
    // bytes are not).
    #[test]
    fn render_outputs_strips_ansi_from_streams() {
        let out = render_outputs(&[Output::Stream {
            stderr: true,
            text: "\u{1b}[31mWarning:\u{1b}[0m in f(): a < b\n".into(),
        }]);
        assert!(
            out.contains("Warning: in f(): a &lt; b"),
            "text preserved: {out}"
        );
        assert!(!out.contains('\u{1b}'), "raw ESC leaked: {out}");
        assert!(
            !out.contains("[31m") && !out.contains("[0m"),
            "ANSI SGR code leaked as visible text: {out}"
        );
    }

    // Regression (AP8-1): an executed cell's stderr warning (matplotlib's Agg
    // `UserWarning`, any `warnings.warn`, or a stdout `print(__file__)`) cites the kernel's
    // per-process cell file `<tmpdir>/ipykernel_<PID>/<HASH>.py`. Left in, the PID/hash make
    // cold/CI/cross-machine builds non-reproducible AND leak a local absolute path into the
    // published HTML. Scrub it to a stable `<cell>` marker (the IPython traceback arm already
    // reads `Cell In[N]`). Captured verbatim from a real ipykernel-7 build.
    #[test]
    fn render_outputs_scrubs_nondeterministic_kernel_paths() {
        let out = render_outputs(&[Output::Stream {
            stderr: true,
            text: "/tmp/ipykernel_3761593/2688443964.py:2: UserWarning: reproducible?\n".into(),
        }]);
        assert!(
            out.contains(":2: UserWarning: reproducible?"),
            "the deterministic line + message must be kept: {out}"
        );
        assert!(
            !out.contains("ipykernel_") && !out.contains("3761593") && !out.contains("/tmp/"),
            "the non-deterministic PID / local temp path leaked into the page: {out}"
        );
        // The stable marker survives HTML-escaping (renders as `<cell>` for the reader).
        assert!(
            out.contains("&lt;cell&gt;:2:"),
            "stable marker missing: {out}"
        );
    }

    #[test]
    fn scrub_kernel_paths_normalizes_cell_source_paths() {
        // The exact string a Python warning emits (captured from a real ipykernel-7 build).
        assert_eq!(
            scrub_kernel_paths("/tmp/ipykernel_3761593/2688443964.py:2: UserWarning: hi"),
            "<cell>:2: UserWarning: hi",
        );
        // TMPDIR need not be /tmp (macOS uses /var/folders/…); the whole path token goes.
        assert_eq!(
            scrub_kernel_paths("/var/folders/ab/T/ipykernel_9/12.py:6: UserWarning: agg"),
            "<cell>:6: UserWarning: agg",
        );
        // Legacy `<ipython-input-N-hash>` form (older IPython).
        assert_eq!(
            scrub_kernel_paths("<ipython-input-12-3a9f2b1c>:1: DeprecationWarning: old"),
            "<cell>:1: DeprecationWarning: old",
        );
        // Two occurrences in one stream are both scrubbed; surrounding text is preserved.
        assert_eq!(
            scrub_kernel_paths("a /tmp/ipykernel_1/2.py:3 b /tmp/ipykernel_1/4.py:5 c"),
            "a <cell>:3 b <cell>:5 c",
        );
        // No false positive: the launcher module name has no `<digits>/<digits>.py` tail.
        assert_eq!(
            scrub_kernel_paths("using ipykernel_launcher here"),
            "using ipykernel_launcher here",
        );
        // Ordinary warning text is untouched.
        assert_eq!(
            scrub_kernel_paths("just a warning, be careful"),
            "just a warning, be careful",
        );
    }

    // Runs only when TALIESIN_PYTHON points at a python with ipykernel; without
    // one it reports ok WITHOUT exercising a real kernel (the pure-logic tests
    // above carry the unconditional coverage).
    #[test]
    fn kernel_executes_state_errors_and_interrupts_runaway_cell() {
        let Some(py) = std::env::var_os("TALIESIN_PYTHON") else {
            // TALIESIN_REQUIRE_KERNEL=1 (set by the CI kernel job) turns the usual skip into
            // a HARD FAIL, so an env regression that unsets TALIESIN_PYTHON can't silently
            // re-green the whole exec stack — this test is the canary.
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL is set but TALIESIN_PYTHON is unset: the live-kernel \
                 tests would silently skip. Point TALIESIN_PYTHON at a python with ipykernel."
            );
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with ipykernel to \
                 actually exercise kernel.rs; this run did not."
            );
            return;
        };
        let py = PathBuf::from(py);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut k = Kernel::start_with_retry(&KernelSpec::python(&py), None)
                .await
                .expect("kernel should start");
            // A short per-cell cap so the runaway case below trips fast. Set on the kernel,
            // NOT via `TALIESIN_CELL_TIMEOUT`: `cell_timeout()` memoizes in a `OnceLock`, so
            // the env var only ever took effect when this test happened to be the first in
            // the binary to touch that lock. When it was not, the cap stayed at the 120 s
            // default, the runaway ran for two minutes, and the 20 s assertion below failed —
            // which read as "flaky under load" but was purely test-ordering. Nothing about
            // the assertion is timing-sensitive once the cap is deterministic.
            k.cell_cap = Some(Duration::from_secs(3));

            // stdout stream + a bare expression result
            let html = render_outputs(&k.execute("print('hello'); 6 * 7").await.unwrap());
            assert!(html.contains("hello"), "missing stdout: {html}");
            assert!(html.contains("42"), "missing result: {html}");

            // warmth: kernel state persists across executions
            k.execute("x = 21").await.unwrap();
            let warm = render_outputs(&k.execute("print(x * 2)").await.unwrap());
            assert!(warm.contains("42"), "kernel state not retained: {warm}");

            // errors are captured
            let err = render_outputs(&k.execute("1 / 0").await.unwrap());
            assert!(err.contains("ZeroDivisionError"), "missing error: {err}");

            // A *streaming* runaway cell (the case a per-message timeout never
            // catches) is interrupted at the cap, then the warm kernel recovers.
            let t = std::time::Instant::now();
            let runaway = render_outputs(
                &k.execute("import time\nwhile True:\n    print('x'); time.sleep(0.05)")
                    .await
                    .unwrap(),
            );
            assert!(
                t.elapsed() < Duration::from_secs(20),
                "runaway cell should be interrupted well before 20s (took {:?})",
                t.elapsed()
            );
            assert!(
                runaway.contains("interrupt") || runaway.contains("KeyboardInterrupt"),
                "runaway cell should report an interrupt: {runaway}"
            );
            // The kernel survived the interrupt and still holds warm state (x == 21).
            let recovered = render_outputs(&k.execute("print(x * 2)").await.unwrap());
            assert!(
                recovered.contains("42"),
                "kernel did not recover after interrupt: {recovered}"
            );

            // A kernel that DIES mid-cell is caught by the is_alive() probe and reported
            // as a distinct `KernelDied` — not mislabeled "Timeout" after blocking the full
            // cap + grace. Fails fast (the probe fires within a poll interval; only the
            // shell-drain post-amble remains). Runs LAST: it SIGKILLs the kernel.
            let t = std::time::Instant::now();
            // The path we hardened: the read timed out and is_alive() saw the corpse, so
            // execute returns Ok with a KernelDied output. (If instead the iopub socket
            // errored on the dead peer, execute returns Err — also acceptable; either way
            // it must not hang, which the elapsed assertion below enforces.)
            if let Ok(outputs) = k
                .execute("import os, signal\nos.kill(os.getpid(), signal.SIGKILL)")
                .await
            {
                let html = render_outputs(&outputs);
                assert!(
                    html.contains("KernelDied"),
                    "a kernel that exits mid-cell must report KernelDied, got: {html}"
                );
                assert!(
                    !html.contains("Timeout"),
                    "a dead kernel must not be mislabeled a timeout: {html}"
                );
            }
            assert!(
                t.elapsed() < Duration::from_secs(10),
                "a dead kernel must fail fast, not hang the full cap+grace (took {:?})",
                t.elapsed()
            );
        });
    }

    #[test]
    #[cfg(unix)]
    fn forked_cleanup_armed_kills_pid_and_removes_conn_dir() {
        // The leak `adopt_forked` closes: when the handshake fails, the forked PID +
        // its /tmp connection dir have no owner. The armed guard's Drop must SIGKILL
        // the PID and remove the dir. A real child process stands in for the fork.
        let dir = std::env::temp_dir().join(format!("tali-forkclean-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn a stand-in child process");
        let pid = child.id();
        {
            let _guard = ForkedCleanup {
                pid,
                conn_dir: Some(dir.clone()),
            };
        } // armed drop: must kill + remove

        assert!(
            !dir.exists(),
            "armed guard must remove the /tmp connection dir"
        );
        // The child must have exited (SIGKILL). `try_wait` also reaps the zombie so
        // `kill(pid, 0)` isn't fooled by a still-in-table corpse.
        let mut exited = false;
        for _ in 0..100 {
            match child.try_wait() {
                Ok(Some(_)) => {
                    exited = true;
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => break,
            }
        }
        assert!(exited, "armed guard must SIGKILL the forked PID");
    }

    #[test]
    #[cfg(unix)]
    fn forked_cleanup_disarm_preserves_pid_and_conn_dir() {
        // On the success path the guard is disarmed: it returns the dir for the live
        // Kernel to own, and must NOT kill the process or delete the dir.
        let dir = std::env::temp_dir().join(format!("tali-forkclean-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn a stand-in child process");
        let pid = child.id();

        let guard = ForkedCleanup {
            pid,
            conn_dir: Some(dir.clone()),
        };
        let returned = guard.disarm();

        assert_eq!(
            returned, dir,
            "disarm returns the dir for the Kernel to own"
        );
        assert!(dir.exists(), "disarm must NOT remove the connection dir");
        assert!(
            matches!(child.try_wait(), Ok(None)),
            "disarm must NOT kill the forked PID"
        );

        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn conn_dir_guard_armed_removes_dir_and_disarm_keeps_it() {
        // The leak `Kernel::start` closes: a startup failure before the live Kernel
        // takes ownership must remove the 0700 /tmp connection dir on the guard's
        // drop. Pure filesystem logic, so this runs in CI without a kernel.
        let armed = std::env::temp_dir().join(format!("tali-conndir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&armed).unwrap();
        {
            let _guard = ConnDirGuard {
                conn_dir: Some(armed.clone()),
            };
        } // armed drop: must remove the dir
        assert!(
            !armed.exists(),
            "an armed ConnDirGuard must remove the connection dir on drop"
        );

        // Success path: `disarm` hands the dir to the live Kernel and must NOT remove
        // it, so the kernel keeps ownership (its own Drop cleans up later).
        let kept = std::env::temp_dir().join(format!("tali-conndir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&kept).unwrap();
        let guard = ConnDirGuard {
            conn_dir: Some(kept.clone()),
        };
        let returned = guard.disarm();
        assert_eq!(
            returned, kept,
            "disarm returns the dir for the Kernel to own"
        );
        assert!(kept.exists(), "disarm must NOT remove the connection dir");
        let _ = std::fs::remove_dir_all(&kept);
    }

    #[test]
    fn cold_python_kernel_argv_arms_parent_death_reaping() {
        // A cold-started kernel is a direct child of taliesin, so on an *ungraceful*
        // death (SIGKILL / crash / closed terminal) our `Drop` never runs and the
        // kernel orphans. We arm ipykernel's `ParentPollerUnix` by passing our REAL
        // pid as `--IPKernelApp.parent_handle` (the argv is built in-parent, before
        // spawn, so `process::id()` is the kernel's ppid-to-be).
        let py = KernelSpec::python(Path::new("python3"));
        let args = (py.argv)(Path::new("/tmp/tali-kernel-x/connection.json"));
        let want = format!("--IPKernelApp.parent_handle={}", std::process::id());
        assert!(
            args.iter().any(|a| a == &want),
            "python cold-kernel argv must arm parent-death reaping ({want}); got {args:?}"
        );
        // `parent_handle=1` is a NO-OP in ipykernel 7 (the old `ppid==init` mode is
        // subreaper-fragile and explicitly disabled); guard against it creeping back.
        assert!(
            !args.iter().any(|a| a == "--IPKernelApp.parent_handle=1"),
            "parent_handle=1 is disabled in ipykernel 7; pass the real pid"
        );

        // R/IRkernel has no `ParentPollerUnix` equivalent, so its argv must NOT carry
        // the ipykernel-only flag (it would be an unknown option to R).
        let r = KernelSpec::r(Path::new("R"));
        let rargs = (r.argv)(Path::new("/tmp/tali-kernel-y/connection.json"));
        assert!(
            !rargs.iter().any(|a| a.contains("parent_handle")),
            "R kernel argv must not carry the ipykernel-only parent_handle flag; got {rargs:?}"
        );
    }

    /// End-to-end proof that a cold kernel self-reaps when taliesin dies ungracefully.
    ///
    /// Runs in two modes. CHILD mode (env set) plays "taliesin": it cold-starts a real
    /// kernel, records its pid + conn dir, then blocks forever so ONLY an outside
    /// SIGKILL can end it (never `Drop`). PARENT mode re-spawns this binary in child
    /// mode, SIGKILLs it, and asserts the orphaned kernel self-terminated — which only
    /// happens because the argv armed `ParentPollerUnix` (mutation: drop the flag or
    /// set it to `1` and the kernel survives, failing this test). Reproduced without
    /// the fix: the orphan reparents to a *subreaper* (not init), so the poller must
    /// key off "ppid changed", which the real-pid `parent_handle` arms.
    #[test]
    #[cfg(target_os = "linux")]
    fn cold_kernel_self_reaps_on_ungraceful_parent_death() {
        const CHILD_ENV: &str = "TALIESIN_COLD_REAP_CHILD";

        // ---- CHILD ("fake taliesin") mode ----
        if let Ok(pidfile) = std::env::var(CHILD_ENV) {
            let py = PathBuf::from(
                std::env::var_os("TALIESIN_PYTHON").expect("child mode requires TALIESIN_PYTHON"),
            );
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                // `start_with_retry`, not `start`: the suite runs many kernel starts at
                // once, so this child can lose the `peek_ports` race and exit with
                // "Address already in use". That is what made this test flake ~1 run in
                // 13 — the parent then timed out and blamed its own 30 s wait.
                let k = Kernel::start_with_retry(&KernelSpec::python(&py), None)
                    .await
                    .expect("cold kernel should start in child mode");
                let pid = k.proc.pid().expect("an owned kernel has a pid");
                let dir = k.conn_dir.clone();
                std::fs::write(&pidfile, format!("{pid}\n{}", dir.display()))
                    .expect("write pidfile");
                // Hold the kernel alive but NEVER drop it: block until the parent
                // SIGKILLs us. `Drop` would reap gracefully and mask the poller.
                std::mem::forget(k);
                loop {
                    std::thread::sleep(Duration::from_secs(3600));
                }
            });
            unreachable!("child mode blocks until SIGKILLed");
        }

        // ---- PARENT mode ----
        if std::env::var_os("TALIESIN_PYTHON").is_none() {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL is set but TALIESIN_PYTHON is unset: the cold-kernel \
                 reaping test would silently skip."
            );
            eprintln!("SKIPPED (no live kernel): set TALIESIN_PYTHON to test cold-kernel reaping.");
            return;
        }

        let pidfile =
            std::env::temp_dir().join(format!("tali-coldreap-{}.pid", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_file(&pidfile);

        let mut child =
            std::process::Command::new(std::env::current_exe().expect("current test binary path"))
                // A plain (non-`--exact`) filter matching this test's unique name, so the
                // re-spawned harness runs exactly this one test in child mode.
                .arg("cold_kernel_self_reaps_on_ungraceful_parent_death")
                .env(CHILD_ENV, &pidfile)
                .spawn()
                .expect("re-spawn the test binary in child mode");
        let child_pid = child.id();

        // Wait (up to ~30s: cold start + ZMQ handshake, generous under load) for the
        // child to report the live kernel's pid + conn dir.
        let mut reported = None;
        for _ in 0..300 {
            if let Ok(s) = std::fs::read_to_string(&pidfile) {
                let mut lines = s.lines();
                if let (Some(p), Some(d)) = (lines.next(), lines.next())
                    && let Ok(pid) = p.trim().parse::<u32>()
                {
                    reported = Some((pid, PathBuf::from(d.trim())));
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        let Some((kernel_pid, conn_dir)) = reported else {
            unsafe { libc::kill(child_pid as libc::pid_t, libc::SIGKILL) };
            let _ = child.wait();
            let _ = std::fs::remove_file(&pidfile);
            panic!("child never reported a live kernel pid (Kernel::start failed?)");
        };

        assert!(
            pid_is_live(kernel_pid),
            "kernel {kernel_pid} should be alive before we kill its parent"
        );

        // Ungraceful death of the parent ("taliesin"): SIGKILL, so no `Drop` runs and
        // the kernel is orphaned to a subreaper.
        unsafe { libc::kill(child_pid as libc::pid_t, libc::SIGKILL) };
        let _ = child.wait();

        // The orphan must self-terminate via `ParentPollerUnix` (its ppid changed).
        // Poll interval is 1s; allow generous slack for load.
        let mut reaped = false;
        for _ in 0..120 {
            if !pid_is_live(kernel_pid) {
                reaped = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        // Tidy up regardless (the forgotten Kernel's dir leaked with the child's death).
        let _ = std::fs::remove_dir_all(&conn_dir);
        let _ = std::fs::remove_file(&pidfile);
        if !reaped {
            unsafe { libc::kill(kernel_pid as libc::pid_t, libc::SIGKILL) };
        }
        assert!(
            reaped,
            "an orphaned cold kernel must self-reap on ungraceful taliesin death; \
             pid {kernel_pid} survived"
        );
    }

    /// A pid is "live" iff /proc/<pid>/stat exists and its state is not Zombie — a
    /// self-exited orphan lingers as a `Z` entry until its subreaper reaps it, and that
    /// must read as dead (a bare `kill(pid, 0)` is fooled by the corpse in the table).
    #[cfg(target_os = "linux")]
    fn pid_is_live(pid: u32) -> bool {
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return false;
        };
        let Some(rest) = stat.rsplit_once(')').map(|(_, r)| r.trim_start()) else {
            return false;
        };
        rest.split_whitespace().next().unwrap_or("") != "Z"
    }
}
