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

/// The two liveness caps on a single cell execution, and their defaults.
///
/// **A cell is capped on SILENCE, not on wall-clock time** (item 175a). A cell
/// printing an epoch line every 30 s is alive no matter how long it runs; a cell
/// that has said nothing for ten minutes is the real runaway. The previous default
/// (120 s of wall-clock) killed a 40-minute training cell at two minutes, which is
/// a wall on first contact for anyone doing computationally heavy work.
///
/// The wall-clock cap is kept, but **off by default**: setting
/// `TALIESIN_CELL_TIMEOUT=120` reproduces the pre-175a behavior exactly.
struct Caps {
    wall: Option<Duration>,
    silence: Option<Duration>,
}

const DEFAULT_CAPS: Caps = Caps {
    wall: None,
    silence: Some(Duration::from_secs(600)),
};

/// Which cap owns the current budget. The payload is that cap's own length in
/// seconds, so an expiry message names the budget the author would raise rather
/// than reporting a silence kill as a wall-clock one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapKind {
    Wall(u64),
    Silence(u64),
    None,
}

/// Interpret a raw cap value. `0` disables the cap; an unset or unparseable value
/// keeps `default`. One helper so the "`0` disables" rule cannot drift apart between
/// the two variables.
///
/// It takes the *already-read* value rather than the variable's name on purpose:
/// `env_help_lists_every_runtime_env_var` finds runtime knobs by scanning for a
/// `var("TALIESIN_…")` literal, so passing the name into a helper would hide **both**
/// caps from that gate and let the CLI's ENV block drift silently.
fn env_cap(raw: Option<String>, default: Option<Duration>) -> Option<Duration> {
    match raw.and_then(|s| s.parse::<u64>().ok()) {
        Some(0) => None,
        Some(secs) => Some(Duration::from_secs(secs)),
        None => default,
    }
}

/// Wall-clock cap (`TALIESIN_CELL_TIMEOUT` seconds, default **off**). Read once.
fn cell_timeout() -> Option<Duration> {
    static T: OnceLock<Option<Duration>> = OnceLock::new();
    *T.get_or_init(|| {
        env_cap(
            std::env::var("TALIESIN_CELL_TIMEOUT").ok(),
            DEFAULT_CAPS.wall,
        )
    })
}

/// Silence cap (`TALIESIN_CELL_SILENCE` seconds, default 600; `0` disables).
/// Resets on every iopub message, so it measures how long the cell has been
/// *quiet*, not how long it has been running. Read once.
fn silence_timeout() -> Option<Duration> {
    static T: OnceLock<Option<Duration>> = OnceLock::new();
    *T.get_or_init(|| {
        env_cap(
            std::env::var("TALIESIN_CELL_SILENCE").ok(),
            DEFAULT_CAPS.silence,
        )
    })
}

/// How long a cell gets to answer an interrupt before the loop stops waiting on it: long
/// enough for the `KeyboardInterrupt` + `Idle` that resync the channels, short enough that
/// a cell which will never answer does not hold the page.
///
/// One constant for both interrupt sites, but they read its expiry differently. A cell the
/// **flood cap** interrupted is still talking, so a window that runs dry is the wanted
/// outcome. A cell a **liveness cap** interrupted has already gone quiet, so a window that
/// runs out means SIGINT was not honoured — see [`Output::interrupt_ignored`].
const INTERRUPT_GRACE: Duration = Duration::from_secs(5);

/// How long to wait before the next liveness check, and which cap owns that
/// budget. A zero budget means that cap has expired. `Duration::MAX` with
/// [`CapKind::None`] means neither cap is armed, so the cell runs until it
/// finishes or the kernel dies.
///
/// Pure on purpose: this is the whole cap decision, so it can be tested with
/// synthetic durations instead of by waiting. The loop around it only polls.
fn cell_budget(
    elapsed: Duration,
    since_last_msg: Duration,
    wall: Option<Duration>,
    silence: Option<Duration>,
) -> (Duration, CapKind) {
    let w = wall.map(|d| (d.saturating_sub(elapsed), CapKind::Wall(d.as_secs())));
    let s = silence.map(|d| {
        (
            d.saturating_sub(since_last_msg),
            CapKind::Silence(d.as_secs()),
        )
    });
    match (w, s) {
        // Whichever expires first owns both the budget and the message.
        (Some(a), Some(b)) => {
            if a.0 <= b.0 {
                a
            } else {
                b
            }
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => (Duration::MAX, CapKind::None),
    }
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
///   - a legend frame is a `Patch`, not a `Text`, so it needs its own pass: it takes
///     the theme *background* (keeping the author's `framealpha`) rather than going
///     transparent, because the box is what makes a legend readable over the data.
///
/// Data colours are never touched. The wrap installs lazily (on the first cell that
/// mentions matplotlib) so non-plotting documents pay nothing.
const MPL_THEME_PREAMBLE: &str = r#"
try:
    _ip = get_ipython()
    if _ip is not None:
        # Transparency for the inline image only (not global rcParams).
        _ip.run_line_magic('config', "InlineBackend.print_figure_kwargs = {'facecolor': 'none', 'edgecolor': 'none', 'bbox_inches': 'tight'}")

        # (foreground, grid, background) per theme — kept in sync with --tali-fg /
        # --tali-border / --tali-bg in assets/css/tokens{,-dark}.css.
        #
        # The background is only used for artists that paint their OWN backing and so
        # cannot just be made transparent: a legend frame. Everything else is set to
        # 'none' and lets the page show through.
        _TALI_LIGHT = ('#22201a', '#d9d7d2', '#fbf9f5')
        _TALI_DARK = ('#eae7e0', '#33312b', '#14130f')
        _tali_orig_png = [None]  # the real Figure->png formatter, captured once

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
            # color='white'; the light render turned that white into #22201a.
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

        def _tali_legends(fig):
            # Every Legend in the figure. They hang off three different places and a
            # figure can hold all three at once: `fig.legends` (figure-level), the
            # `ax.legends` list, and `ax.get_legend()` (the usual `ax.legend()` case,
            # which on older matplotlib is NOT mirrored into `ax.legends`).
            _out, _seen = [], set()
            for _lg in list(getattr(fig, 'legends', ())):
                if id(_lg) not in _seen:
                    _seen.add(id(_lg)); _out.append(_lg)
            for _ax in fig.axes:
                for _lg in (*getattr(_ax, 'legends', ()), _ax.get_legend()):
                    if _lg is not None and id(_lg) not in _seen:
                        _seen.add(id(_lg)); _out.append(_lg)
            return _out

        def _tali_recolour(fig, fg, grid, bg):
            # Recolour foreground (text/spines/ticks) to `fg` and grid lines to
            # `grid`, and make axes backgrounds transparent. Returns the originals
            # so the figure can be restored exactly. Data colours are untouched, and
            # so is any text sitting ON a data colour (see _tali_texts_on_fill).
            import matplotlib.colors as _mc
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
            # A legend frame is a Patch, so the Text walk above recoloured the LABELS
            # but left the box behind them alone. Its facecolor was resolved once in
            # Legend.__init__ from rcParams['axes.facecolor'] (`legend.facecolor:
            # inherit`) — white — so the dark render put near-white labels on a white
            # box: measured 1.23:1, against WCAG AA's 4.5:1. Unlike `ax.patch` this
            # cannot simply go transparent: the box is what makes a legend readable
            # where it overlaps the data, so it takes the page background instead.
            #
            # The alpha is carried over rather than replaced. `framealpha` is the
            # author's call about seeing data through the legend, and it is a
            # transparency, not a colour, so theming the hue leaves it untouched.
            for _lg in _tali_legends(fig):
                _fr = _lg.get_frame()
                _fc, _ec = _fr.get_facecolor(), _fr.get_edgecolor()
                saved.append((_fr.set_facecolor, _fc)); _fr.set_facecolor(_mc.to_rgba(bg, _mc.to_rgba(_fc)[3]))
                saved.append((_fr.set_edgecolor, _ec)); _fr.set_edgecolor(_mc.to_rgba(grid, _mc.to_rgba(_ec)[3]))
            return saved

        def _tali_render(fig, fg, grid, bg):
            # Produce a base64 PNG (transparent bg) with `fg`/`grid`/`bg` recolouring,
            # restoring the live figure afterwards.
            _orig = _tali_orig_png[0]
            _saved = _tali_recolour(fig, fg, grid, bg)
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
except Exception as _e:
    # Caught, because a kernel that cannot theme a figure must still run cells — but
    # SAID, on stderr, because the alternative is a `pass` that turns "this Python
    # broke our hook" into "your figures stopped matching the page" months later
    # (FA27). The Rust side reads this line back and names it on the console.
    import sys as _sys
    print('theme hook not installed:', _e, file=_sys.stderr)
"#;

/// One block of startup code, plus **what the author loses if it does not run**.
///
/// The second field is the whole reason this is a struct and not a bare `&str`. A
/// preamble that fails leaves the kernel perfectly usable and one feature silently
/// absent, so the symptom arrives cells later and wearing a disguise: a `NameError` on
/// `define`, or figures that quietly stop matching the page. Naming the casualty at the
/// moment of failure is what turns that into a pointer (FA27).
struct Preamble {
    /// What stops working, as a sentence that completes "…, so …".
    provides: &'static str,
    code: &'static str,
}

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
    /// Code run once at startup (the Python->JS `define` bridge + matplotlib theme
    /// for Python; nothing for R yet).
    preambles: &'static [Preamble],
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
                    // kernel's ppid-to-be.
                    // NOT `PR_SET_PDEATHSIG`: that fires on a parent *thread* exit,
                    // which a tokio worker can do mid-session, killing a live kernel.
                    format!("--IPKernelApp.parent_handle={}", std::process::id()),
                ]
            },
            preambles: &[
                Preamble {
                    provides: "`define(...)` will not exist, so every `{js}` cell reading a \
                               Python value fails",
                    code: OJS_DEFINE_PREAMBLE,
                },
                Preamble {
                    provides: "inline matplotlib figures will not follow the page's light/dark \
                               palette",
                    code: MPL_THEME_PREAMBLE,
                },
            ],
        }
    }
}

/// One execution output, already rendered to a self-contained HTML fragment.
///
/// `PartialEq` exists for the streaming tests: the live-vs-final invariant compares
/// output lists directly, and comparing rendered HTML instead would let an escaping
/// change mask a divergence.
#[derive(Debug, Clone, PartialEq)]
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

    /// A liveness cap interrupted the cell and the cell **did not stop**: it outlived
    /// [`INTERRUPT_GRACE`] without reaching Idle, so it is still running inside the warm
    /// kernel and every later cell queues behind it. SIGINT is a request (a cell may
    /// install its own handler, or sit in a C extension that never checks signals), and
    /// there is no second signal that stops the cell without killing the kernel — so this
    /// says what happened instead of letting it read as a plain cap expiry.
    ///
    /// **Carries no pid**, though the pid is the one thing an operator wants: this string
    /// is rendered into the built page, and a pid there makes two builds of the same
    /// document differ. The pid goes to the console beside this, where no reader sees it.
    pub(crate) fn interrupt_ignored() -> Self {
        Output::Error {
            ename: "InterruptIgnored".into(),
            evalue: "cell ignored the interrupt and is still running in the kernel; restart \
                     the kernel to reclaim it"
                .into(),
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
    /// The kernel process. Directly spawned (`python -m ipykernel_launcher`) and
    /// `kill_on_drop`, so liveness is a non-blocking `try_wait` and teardown is a
    /// `start_kill`.
    proc: Child,
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
    /// Silence cap: how long a cell may produce *no output at all* before it is
    /// interrupted. See [`silence_timeout`]. Held per kernel for the same reason as
    /// [`Kernel::cell_cap`] above, so a test can set it without fighting a `OnceLock`.
    silence_cap: Option<Duration>,
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

/// Run a spec's startup preambles against a live kernel and return one console line per
/// preamble that did not run clean (empty when they all did).
///
/// Separate from [`Kernel::finish`], which only logs what comes back, so the detection is
/// reachable from a test with a deliberately poisoned preamble: a rule about a failure
/// nobody has ever seen fail is worth exactly what it is exercised by.
async fn run_preambles(kernel: &mut Kernel, spec: &KernelSpec) -> Vec<String> {
    let mut problems = Vec::new();
    for preamble in spec.preambles {
        let outcome = kernel.execute(preamble.code).await;
        let detail = match &outcome {
            Err(e) => Some(e.to_string()),
            Ok(outputs) => outputs.iter().find_map(preamble_failure),
        };
        if let Some(detail) = detail {
            problems.push(format!(
                "kernel preamble failed on `{}` ({detail}), so {}",
                spec.program.display(),
                preamble.provides
            ));
        }
    }
    problems
}

/// What a preamble output says went wrong, or `None` for an output that is fine.
///
/// Two shapes count, because the ~270 lines of version-sensitive Python have two ways to
/// break: an exception the interpreter raises out (the `define` bridge is unguarded, so a
/// future syntax or API change lands here), and a line on **stderr**, which is how the
/// matplotlib preamble reports a failure it has to catch itself — it must not abort the
/// hook registration around it. Ordinary output is not a failure: a preamble that printed
/// something would otherwise cry wolf at every kernel start.
fn preamble_failure(out: &Output) -> Option<String> {
    match out {
        Output::Error { ename, evalue, .. } => Some(format!("{ename}: {evalue}")),
        Output::Stream { stderr: true, text } => {
            let line = text.lines().find(|l| !l.trim().is_empty())?;
            Some(line.trim().to_string())
        }
        _ => None,
    }
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
/// Factored out of [`Kernel::start`] so the connection file's modes are stated once.
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
    /// ports rather than in the callers, which each grew a private copy; the one caller
    /// without one, the child half of
    /// `cold_kernel_self_reaps_on_ungraceful_parent_death` (which cold-starts a kernel
    /// in a re-spawned test binary), inherited the race and flaked. Captured
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
    /// (`scipy.io.wavfile.write`, matplotlib's `savefig`, R's `ggsave`)
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
        Kernel::finish(child, conn_dir, iopub, shell, spec).await
    }

    /// Shared tail of every spawn path: build the `Kernel` and run each startup
    /// preamble once against the now-live kernel.
    async fn finish(
        proc: Child,
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
            silence_cap: silence_timeout(),
        };
        // Language-specific startup (e.g. Python's `define` bridge + matplotlib theme);
        // each preamble runs once against the warm kernel. A failure here is NOT fatal —
        // the kernel runs cells perfectly well without the `define` bridge — but it must
        // not be silent either, which it was until FA27.
        for problem in run_preambles(&mut kernel, spec).await {
            crate::log::warn(&problem);
        }
        Ok(kernel)
    }

    /// Whether the kernel process is still alive — a cheap, non-blocking check
    /// (`try_wait`) used to detect a kernel that died mid-session so it can be
    /// respawned instead of hanging on the next execute's timeout.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.proc.try_wait(), Ok(None))
    }

    /// Run `code` and collect its outputs (waits until the kernel is idle).
    pub async fn execute(&mut self, code: &str) -> io::Result<Vec<Output>> {
        self.execute_streaming(code, |_| {}).await
    }

    /// [`Kernel::execute`], but `on_output` is called with each output **as it
    /// arrives** rather than only with the finished vector (item 175b). The returned
    /// vector is unchanged, so a caller that wants no streaming passes a no-op and
    /// sees exactly the previous behavior.
    ///
    /// The callback fires from one watermark flush rather than from each of the
    /// seven `outputs.push` sites, so a push added later cannot silently stop being
    /// streamed.
    pub async fn execute_streaming(
        &mut self,
        code: &str,
        mut on_output: impl FnMut(&Output),
    ) -> io::Result<Vec<Output>> {
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
        // The two liveness caps (item 175a). Silence is the primary one and is on by
        // default; wall-clock is off unless `TALIESIN_CELL_TIMEOUT` is set. On hitting
        // either we SIGINT the kernel, then drain a short grace window so the resulting
        // KeyboardInterrupt + Idle resync the channels and the *next* cell still works.
        //
        // A streaming runaway (`while True: print(x)`) never goes silent, so it is NOT
        // caught here: it is caught by the output caps below, which interrupt as soon as
        // `capped` trips. That is why dropping the wall-clock default loses no protection.
        let wall = self.cell_cap;
        let silence = self.silence_cap;
        let started = Instant::now();
        // Set when we have interrupted and are draining the resulting KeyboardInterrupt +
        // Idle. `grace_after_cap` records which of the two interrupt sites put us here,
        // because they read the window's expiry differently (see [`INTERRUPT_GRACE`]).
        let mut grace_until: Option<Instant> = None;
        let mut grace_after_cap = false;
        // Last time THIS cell produced output: the silence cap measures from here, so it
        // resets on every output and a chatty long cell is never capped.
        let mut last_msg = Instant::now();
        // How many outputs have been handed to `on_output`. Flushed at the top of
        // every iteration and once after the loop, so every path that pushes and then
        // either loops or breaks is covered without touching the push sites.
        let mut streamed = 0usize;
        loop {
            while streamed < outputs.len() {
                on_output(&outputs[streamed]);
                streamed += 1;
            }
            let now = Instant::now();
            // Time left before this cell's REAL deadline: the post-interrupt grace window,
            // or whichever liveness cap expires first.
            let (budget, expired) = match grace_until {
                Some(g) => (g.saturating_duration_since(now), CapKind::None),
                None => cell_budget(
                    now.duration_since(started),
                    now.duration_since(last_msg),
                    wall,
                    silence,
                ),
            };
            // A cap's grace window ran out with this cell still not Idle: the interrupt was
            // not honoured. The cell is STILL RUNNING in the warm kernel — nothing else this
            // process can send stops it without killing the kernel — so report that and stop
            // waiting, rather than dropping out silently as if the cap had done its job. The
            // pid goes to the console (an operator can act on it) and not into the page (two
            // builds of one document must not differ by a pid).
            if grace_after_cap && budget.is_zero() {
                if let Some(pid) = self.running_pid() {
                    crate::log::warn(&format!(
                        "kernel (pid {pid}) ignored the interrupt: a cell is still running \
                         there. Restart the kernel, or kill that process."
                    ));
                }
                outputs.push(Output::interrupt_ignored());
                break;
            }
            // Poll on a short interval (capped at the budget) so a kernel that EXITS
            // mid-cell is noticed within ~1s and reported as a distinct `KernelDied`,
            // instead of blocking the full budget and then mislabeling the crash as
            // "Timeout" (and interrupting a corpse). A healthy long cell just re-polls.
            let poll = budget.min(Duration::from_secs(1));
            let msg = match timeout(poll, self.iopub.read()).await {
                Ok(Ok(msg)) => msg,
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
                        // The flood cap's grace window ran dry: the kernel has stopped
                        // flooding us, which is what the interrupt was for. (A *liveness*
                        // cap's window expiring is handled at the top of the loop, where a
                        // quiet window means the interrupt was ignored.)
                        break;
                    }
                    // A cap expired. Both paths interrupt: a wedged cell must actually be
                    // stopped, not just abandoned, or it keeps running in the warm kernel
                    // and every later cell queues behind it. The message names the cap
                    // that fired and its budget, so the author knows which to raise.
                    let note = match expired {
                        CapKind::Wall(secs) => {
                            format!("cell exceeded {secs}s of wall-clock time; sent interrupt")
                        }
                        CapKind::Silence(secs) => {
                            format!("cell produced no output for {secs}s; sent interrupt")
                        }
                        // Unreachable: an unarmed cap yields `Duration::MAX`, which is
                        // never zero, so this arm cannot be entered via an expired budget.
                        CapKind::None => break,
                    };
                    self.interrupt();
                    outputs.push(Output::timeout(note));
                    grace_until = Some(Instant::now() + INTERRUPT_GRACE);
                    grace_after_cap = true;
                    continue;
                }
            };
            // Only messages parented by our request belong to this execution.
            let ours = msg.parent_header.as_ref().map(|h| h.msg_id.as_str()) == Some(&msg_id);
            if !ours {
                continue;
            }
            // Re-arm the silence window HERE, past that filter, not on every read. iopub is
            // a BROADCAST channel: a background thread an earlier cell left running, or a
            // runaway this loop already gave up on, keeps publishing under its own parent
            // header. Counting that traffic as this cell's output disarms the cap for the
            // one cell it exists to govern, and the silent runaway then runs forever —
            // nothing else stops it, since the wall-clock cap is off by default (FA8).
            last_msg = Instant::now();
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
                grace_until = Some(Instant::now() + INTERRUPT_GRACE);
            }
        }
        // Anything pushed on the way out (a cap's notice, `kernel_died`) still reaches
        // the client, so a cell that dies mid-run says so in the live view too.
        while streamed < outputs.len() {
            on_output(&outputs[streamed]);
            streamed += 1;
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

    /// Send SIGINT to the kernel process, stopping a runaway cell while the warm kernel
    /// and prior cell state survive. See [`interrupt_pid`] for what an interrupt is.
    fn interrupt(&self) {
        if let Some(pid) = self.proc.id() {
            interrupt_pid(pid);
        }
    }

    /// This kernel process's OS pid, while it is alive.
    ///
    /// Exists so an interrupt can be raised from OUTSIDE the task that owns this kernel.
    /// [`Kernel::interrupt`] is reachable only from the polling loop, which is precisely
    /// the code that is blocked for as long as a runaway cell runs — so the one caller
    /// that most needs to interrupt is the one caller that cannot. The executor publishes
    /// this pid on an `AtomicU32` around each cell (`Executor::set_interrupt_handle`) and
    /// the dev server's websocket task reads it there.
    ///
    /// An accessor of this shape existed as `Kernel::pid()` until wave 13 deleted it, on
    /// the correct observation that it then had no caller. This is that caller.
    pub(crate) fn running_pid(&self) -> Option<u32> {
        self.proc.id()
    }
}

/// Send `SIGINT` to a kernel process by PID: the `interrupt_mode: signal` path that raises
/// `KeyboardInterrupt` in the running cell (ipykernel and IRkernel both honour it).
///
/// Free-standing, and the single implementation of "interrupt a kernel". It stays a named
/// function because the non-Unix no-op is the part that must not be duplicated, and because
/// the whole value of the feature is the answer it encodes: an interrupt kills the cell,
/// not the kernel.
///
/// **Two callers, distinguished by where they are standing.** The silence/wall-clock cap
/// calls it from INSIDE the polling loop, having watched the cell go quiet, and reaches the
/// pid through `&self`. The dev server's websocket task calls it from OUTSIDE that loop,
/// on the reader's "Restart kernel", and cannot reach `&self` at all — the builder task
/// owns the executor and is blocked in it. That caller reads the pid off the `AtomicU32`
/// the executor publishes ([`Kernel::running_pid`], `Executor::set_interrupt_handle`).
/// Both raise the same signal on the same process; only the vantage point differs.
///
/// Unix-only; a no-op elsewhere (the cap still ends its own wait).
pub(crate) fn interrupt_pid(pid: u32) {
    #[cfg(unix)]
    // Safety: `kill` with a valid pid + signal is sound; a stale pid just returns ESRCH,
    // which we ignore.
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGINT);
    }
    #[cfg(not(unix))]
    let _ = pid;
}

/// Cleanup guard for [`Kernel::start`]: the 0700 `/tmp/tali-kernel-<uuid>` connection
/// dir created by [`prepare_connection`] has no owner until the ZMQ handshake succeeds
/// and the live [`Kernel`] (whose `Drop` removes it) takes over. An early return on any
/// startup failure would otherwise drop the `PathBuf` and leak the dir. This guard
/// removes it on drop and is `disarm`ed on the success path. The kernel *process* is
/// handled separately (`kill_on_drop` on the spawn command).
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

impl Drop for Kernel {
    fn drop(&mut self) {
        let _ = self.proc.start_kill();
        let _ = std::fs::remove_dir_all(&self.conn_dir);
    }
}

/// Render outputs into an HTML fragment (the inner content of an output block),
/// or empty if there are none. The caller wraps this in the block element.
/// Apply terminal carriage-return semantics to one text run: `\r` returns the cursor
/// to column 0, so what follows replaces the current line. A line already committed
/// by `\n` is never touched.
///
/// A real terminal overwrites character by character, leaving a tail behind when the
/// new line is shorter. We clear instead, because the writers that use `\r` (tqdm and
/// friends) redraw a full padded line each frame, and a stale tail would be a visual
/// artefact of emulating the terminal too faithfully.
fn apply_carriage_returns(text: &str) -> String {
    let mut committed = String::new();
    let mut line = String::new();
    for ch in text.chars() {
        match ch {
            '\r' => line.clear(),
            '\n' => {
                committed.push_str(&line);
                committed.push('\n');
                line.clear();
            }
            c => line.push(c),
        }
    }
    committed.push_str(&line);
    committed
}

/// What the client should do with an arriving output.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LiveOp {
    Append(Output),
    ReplaceLast(Output),
}

/// Accumulates outputs the way the browser does, one at a time, deciding for each
/// whether it extends the list or redraws its last element.
///
/// **Consecutive chunks of the same stream become one output.** A cell's stdout is
/// one stream, and where the kernel chose to cut it into messages is an artefact of
/// the kernel, not of the document: `print` in a loop may arrive as one message or as
/// twenty depending on buffering and timing. Rendering each as its own `<pre>` turned
/// a log into a stack of boxes and made the emitted HTML depend on that chunking.
///
/// This is the single definition of the rule. [`collapse_carriage_returns`] is a fold
/// over it, so the streamed view and the authoritative block **cannot** drift apart:
/// a divergence would have to be a divergence from itself.
#[derive(Default)]
pub(crate) struct LiveOutputs {
    last: Option<Output>,
}

impl LiveOutputs {
    pub(crate) fn push(&mut self, next: Output) -> LiveOp {
        // Same stream (stdout with stdout, stderr with stderr) merges; anything else
        // starts a new output. stdout and stderr stay apart because they are styled
        // differently and interleaving them would attribute one to the other.
        let merge = matches!(
            (&self.last, &next),
            (
                Some(Output::Stream { stderr: prev, .. }),
                Output::Stream { stderr: now, .. },
            ) if prev == now
        );
        if merge {
            let (stderr, prev) = match self.last.take() {
                Some(Output::Stream { stderr, text }) => (stderr, text),
                _ => unreachable!("merge is only set when the last output is a stream"),
            };
            let Output::Stream { text, .. } = &next else {
                unreachable!("merge is only set when the next output is a stream")
            };
            let merged = Output::Stream {
                stderr,
                text: apply_carriage_returns(&(prev + text)),
            };
            self.last = Some(merged.clone());
            return LiveOp::ReplaceLast(merged);
        }
        let fresh = match &next {
            Output::Stream { stderr, text } => Output::Stream {
                stderr: *stderr,
                text: apply_carriage_returns(text),
            },
            other => other.clone(),
        };
        self.last = Some(fresh.clone());
        LiveOp::Append(fresh)
    }
}

/// Batch form of [`LiveOutputs`]: what the whole output list looks like once
/// carriage returns have been applied. Identity for any run containing no `\r`, so
/// documents that do not draw progress bars render exactly as they did before.
pub(crate) fn collapse_carriage_returns(outputs: &[Output]) -> Vec<Output> {
    let mut acc: Vec<Output> = Vec::with_capacity(outputs.len());
    let mut live = LiveOutputs::default();
    for o in outputs {
        match live.push(o.clone()) {
            LiveOp::Append(o) => acc.push(o),
            LiveOp::ReplaceLast(o) => {
                acc.pop();
                acc.push(o);
            }
        }
    }
    acc
}

pub fn render_outputs(outputs: &[Output]) -> String {
    let mut s = String::new();
    // Carriage returns are resolved here rather than at capture time, so the cached
    // and replayed paths get the same treatment as a fresh run and a progress bar
    // never renders as a stack of frames. Identity when no `\r` is present.
    let collapsed = collapse_carriage_returns(outputs);
    for o in &collapsed {
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
        // `alt=""`, not `alt="output"` (item 41). An executed cell's image is spliced into
        // a captioned `<figure>`, so the caption is already the accessible description;
        // a second one reading "output" is noise a screen reader says out loud before it
        // gets to the sentence that means something. Empty alt marks it presentational,
        // which is the correct role for an image whose description sits beside it. The
        // matplotlib twin-render path has always emitted `alt=""`; this is the same
        // treatment for every other inline image (R figures, PIL, anything else).
        return format!(
            "<img alt=\"\" src=\"data:image/png;base64,{}\" />",
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
            "<img alt=\"\" src=\"data:image/jpeg;base64,{}\" />",
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

    // --- cell liveness caps (item 175a) ---------------------------------------
    // `cell_budget` is the whole cap decision, extracted so it can be tested with
    // synthetic durations instead of by waiting. The loop around it only polls.

    const S: fn(u64) -> Duration = Duration::from_secs;

    #[test]
    fn the_default_cap_is_silence_not_wall_clock() {
        // The item's premise: a 40-minute training cell that prints an epoch line
        // every 30s must NOT be killed. Under the old wall-clock default it died at
        // 120s no matter how much output it was producing.
        let (budget, kind) =
            cell_budget(S(40 * 60), S(30), DEFAULT_CAPS.wall, DEFAULT_CAPS.silence);
        assert_eq!(
            kind,
            CapKind::Silence(600),
            "a live, chatty cell must be governed by silence, not wall-clock"
        );
        assert_eq!(budget, S(570), "silence budget resets on every output");
        assert!(
            DEFAULT_CAPS.wall.is_none(),
            "the wall-clock cap must be off by default; it is the wall the item is about"
        );
    }

    #[test]
    fn a_silent_cell_is_capped_and_names_the_silence_budget() {
        // The real runaway: no output at all for the whole budget.
        let (budget, kind) = cell_budget(S(10_000), S(600), None, Some(S(600)));
        assert!(budget.is_zero(), "an expired cap must yield a zero budget");
        assert_eq!(kind, CapKind::Silence(600));
    }

    #[test]
    fn the_nearer_of_the_two_caps_owns_the_budget() {
        // Both armed: whichever expires first must own the budget AND the message,
        // or a wall-clock kill would be reported as a silence timeout.
        let (budget, kind) = cell_budget(S(100), S(1), Some(S(120)), Some(S(600)));
        assert_eq!(
            (budget, kind),
            (S(20), CapKind::Wall(120)),
            "wall is nearer"
        );

        let (budget, kind) = cell_budget(S(100), S(599), Some(S(9_999)), Some(S(600)));
        assert_eq!(
            (budget, kind),
            (S(1), CapKind::Silence(600)),
            "silence is nearer"
        );
    }

    #[test]
    fn setting_the_wall_clock_cap_reproduces_the_old_default_exactly() {
        // `TALIESIN_CELL_TIMEOUT=120` must still mean what it meant before this
        // change, so the escape hatch has a home and the change stays bisectable.
        let (budget, kind) = cell_budget(S(119), S(0), Some(S(120)), None);
        assert_eq!((budget, kind), (S(1), CapKind::Wall(120)));
    }

    #[test]
    fn both_caps_disabled_never_expires() {
        let (budget, kind) = cell_budget(S(10_000), S(10_000), None, None);
        assert_eq!(kind, CapKind::None);
        assert!(
            !budget.is_zero(),
            "an unarmed cap must never expire the cell"
        );
    }

    // --- carriage returns / streaming (item 175b) ------------------------------

    fn out(text: &str) -> Output {
        Output::Stream {
            stderr: false,
            text: text.into(),
        }
    }

    #[test]
    fn a_carriage_return_overwrites_the_current_line() {
        // Terminal semantics: `\r` returns the cursor to column 0, so what follows
        // replaces the line. This is how a progress bar redraws itself in place.
        assert_eq!(
            collapse_carriage_returns(&[out("10%\r20%\r30%\n")]),
            vec![out("30%\n")]
        );
        // A committed line (one ended by `\n`) is never touched by a later `\r`.
        assert_eq!(
            collapse_carriage_returns(&[out("done\nbar 1\rbar 2")]),
            vec![out("done\nbar 2")]
        );
    }

    #[test]
    fn consecutive_chunks_of_one_stream_become_one_output() {
        // A cell's stdout is ONE stream. Where the kernel cut it into messages is an
        // artefact of buffering and timing, so rendering a `<pre>` per message made
        // the emitted HTML depend on that chunking (and a printing loop render as a
        // stack of boxes rather than a log). Verified against the whole corpus when
        // this landed: merging changed no existing document's output.
        assert_eq!(
            collapse_carriage_returns(&[out("first\n"), out("second\n"), out("third\n")]),
            vec![out("first\nsecond\nthird\n")]
        );

        // stdout and stderr do NOT merge: they are styled differently, and joining
        // them would attribute a warning to stdout or vice versa.
        let err = |t: &str| Output::Stream {
            stderr: true,
            text: t.into(),
        };
        assert_eq!(
            collapse_carriage_returns(&[out("out 1\n"), err("warn\n"), out("out 2\n")]),
            vec![out("out 1\n"), err("warn\n"), out("out 2\n")],
            "stdout and stderr must stay separate outputs"
        );

        // A rich output (a figure) also breaks a run, so text keeps its position
        // relative to the image it was printed around.
        assert_eq!(
            collapse_carriage_returns(&[
                out("before\n"),
                Output::Rich("<img>".into()),
                out("after\n"),
            ]),
            vec![
                out("before\n"),
                Output::Rich("<img>".into()),
                out("after\n"),
            ]
        );
    }

    #[test]
    fn a_progress_bar_arriving_in_chunks_collapses_across_outputs() {
        // The real tqdm shape: each redraw is its own iopub message, so the `\r`
        // handling has to work ACROSS outputs, not just inside one.
        let chunks: Vec<Output> = ["\r 0%|    |", "\r 50%|##  |", "\r100%|####|\n"]
            .iter()
            .map(|c| out(c))
            .collect();
        assert_eq!(
            collapse_carriage_returns(&chunks),
            vec![out("100%|####|\n")],
            "a 3-frame bar must render as one line, not three stacked ones"
        );

        // A chunk carrying no `\r` of its own still joins the run, so tqdm's final
        // newline and the line printed after it land in the same block as the bar.
        //
        // Note the live-vs-final invariant test CANNOT pin this: both sides of that
        // comparison run the same code, so a rule change moves them together. Only a
        // written-out expectation catches it, which is how the first version of these
        // tests let a mutant live through exactly this case.
        assert_eq!(
            collapse_carriage_returns(&[out("\rbar 1"), out(" done\n"), out("next\n")]),
            vec![out("bar 1 done\nnext\n")],
            "a redrawing run must keep absorbing plain chunks until something breaks it"
        );
        let mixed = vec![out("\rbar"), Output::Rich("<img>".into()), out("\rbar2")];
        assert_eq!(
            collapse_carriage_returns(&mixed),
            vec![out("bar"), Output::Rich("<img>".into()), out("bar2")]
        );
    }

    #[test]
    fn the_live_stream_and_the_final_render_agree() {
        // THE invariant for 175b. The client builds its live view by applying the
        // ops the server emits as each output arrives; the block that replaces it is
        // `render_outputs` over the whole list. If those two disagree, a finished
        // cell visibly rewrites itself, so they are pinned equal here rather than
        // checked by eye in a browser.
        let raw = vec![
            out("epoch 1\n"),
            out("\r 10%"),
            out("\r 90%"),
            out("\r100%\n"),
            Output::Rich("<img src=x>".into()),
            out("done\n"),
            Output::Stream {
                stderr: true,
                text: "warning\n".into(),
            },
        ];

        // Replay the wire ops into the list a client would hold.
        let mut client: Vec<Output> = Vec::new();
        let mut state = LiveOutputs::default();
        for o in &raw {
            match state.push(o.clone()) {
                LiveOp::Append(o) => client.push(o),
                LiveOp::ReplaceLast(o) => {
                    client.pop();
                    client.push(o);
                }
            }
        }

        assert_eq!(
            client,
            collapse_carriage_returns(&raw),
            "the replayed live view diverged from the batch collapse"
        );
        assert_eq!(
            render_outputs(&client),
            render_outputs(&raw),
            "the live HTML diverged from the authoritative block HTML"
        );
    }

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

    // Item 175a, against a REAL kernel: the whole point of the change is that a cell
    // which keeps talking survives past a budget that would have killed it, while a
    // cell that goes quiet does not. Both halves run against tiny caps (2s) so the
    // test costs seconds, not the 120s the shipped default would need. The pure
    // `cell_budget` tests above prove the arithmetic; this proves the arithmetic is
    // actually the thing governing a live execution.
    #[test]
    fn a_chatty_cell_outlives_the_silence_budget_and_a_quiet_one_does_not() {
        let Some(py) = std::env::var_os("TALIESIN_PYTHON") else {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL is set but TALIESIN_PYTHON is unset: the live-kernel \
                 tests would silently skip. Point TALIESIN_PYTHON at a python with ipykernel."
            );
            eprintln!("SKIPPED (no live kernel): set TALIESIN_PYTHON to exercise the cell caps.");
            return;
        };
        let py = PathBuf::from(py);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut k = Kernel::start_with_retry(&KernelSpec::python(&py), None)
                .await
                .expect("kernel should start");
            // Silence-only, as shipped: no wall-clock cap at all.
            k.cell_cap = None;
            k.silence_cap = Some(Duration::from_secs(2));

            // Chatty: 4s of runtime, a line every 0.2s. Under the OLD wall-clock rule a
            // 2s cap kills this at 2s; under the silence rule it must run to completion,
            // because the budget resets on every print.
            let t = std::time::Instant::now();
            let chatty = render_outputs(
                &k.execute("import time\nfor i in range(20):\n    print(i, flush=True); time.sleep(0.2)\nprint('finished')")
                    .await
                    .unwrap(),
            );
            assert!(
                chatty.contains("finished"),
                "a cell printing every 0.2s was killed by a 2s SILENCE cap, so the budget is \
                 not resetting on output (this is the 175a regression): {chatty}"
            );
            assert!(
                !chatty.contains("no output for"),
                "chatty cell reported a silence timeout: {chatty}"
            );
            assert!(
                t.elapsed() >= Duration::from_secs(3),
                "the cell returned too fast to have actually run its 4s loop"
            );

            // Quiet: one long silent sleep. This is the real runaway, and it must be
            // interrupted at the silence budget and NAME that budget.
            let t = std::time::Instant::now();
            let quiet = render_outputs(&k.execute("import time\ntime.sleep(60)").await.unwrap());
            assert!(
                quiet.contains("no output for 2s"),
                "a silent cell was not capped, or the message did not name the silence \
                 budget: {quiet}"
            );
            assert!(
                t.elapsed() < Duration::from_secs(30),
                "silent cell ran {:?}, far past its 2s budget",
                t.elapsed()
            );

            // The warm kernel survives the interrupt: state from before is still there.
            let after = render_outputs(&k.execute("print('alive', 6 * 7)").await.unwrap());
            assert!(
                after.contains("alive 42"),
                "kernel did not recover after a silence interrupt: {after}"
            );
        });
    }

    // FA8, first half: iopub is a BROADCAST channel, so output parented to an EARLIER
    // cell keeps arriving while this one runs — from a background thread it left behind,
    // or from a runaway the loop gave up on. Re-arming the silence window on that traffic
    // disarms the cap for the cell it is supposed to govern, and the silent runaway then
    // runs forever (the wall-clock cap is off by default, so nothing else stops it).
    //
    // The chatty thread runs under a `contextvars.copy_context()` captured DURING cell 1,
    // which is what pins its output to cell 1's parent header: ipykernel's `OutStream`
    // keeps the parent header in a `ContextVar` and reads it in `write()`, on the writing
    // thread. Without the copied context a plain thread falls back to the global header —
    // i.e. whichever cell is current — and there would be nothing to test.
    #[test]
    fn output_parented_to_an_earlier_cell_does_not_re_arm_the_silence_cap() {
        let Some(py) = std::env::var_os("TALIESIN_PYTHON") else {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL is set but TALIESIN_PYTHON is unset: the live-kernel \
                 tests would silently skip. Point TALIESIN_PYTHON at a python with ipykernel."
            );
            eprintln!("SKIPPED (no live kernel): set TALIESIN_PYTHON to exercise the cell caps.");
            return;
        };
        let py = PathBuf::from(py);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut k = Kernel::start_with_retry(&KernelSpec::python(&py), None)
                .await
                .expect("kernel should start");
            k.cell_cap = None;
            k.silence_cap = Some(Duration::from_secs(1));

            // Cell 1 finishes immediately, leaving a thread that prints every 100 ms under
            // cell 1's parent header for the next minute.
            let first = render_outputs(
                &k.execute(
                    "import threading, time, contextvars\n\
                     ctx = contextvars.copy_context()\n\
                     def chat():\n    \
                         for _ in range(600):\n        \
                             print('tick', flush=True); time.sleep(0.1)\n\
                     threading.Thread(target=lambda: ctx.run(chat), daemon=True).start()\n\
                     print('started')",
                )
                .await
                .unwrap(),
            );
            assert!(first.contains("started"), "cell 1 did not run: {first}");

            // Cell 2 says nothing at all. Its 1s silence cap must fire on schedule even
            // though the kernel is emitting a line every 100 ms the whole time.
            let t = std::time::Instant::now();
            let quiet = render_outputs(&k.execute("import time\ntime.sleep(60)").await.unwrap());
            assert!(
                quiet.contains("no output for 1s"),
                "a silent cell outlived its silence cap while ANOTHER cell's output kept \
                 arriving: the window is being re-armed by traffic that is not this cell's \
                 (FA8): {quiet}"
            );
            assert!(
                t.elapsed() < Duration::from_secs(20),
                "silent cell ran {:?}, far past its 1s budget",
                t.elapsed()
            );
        });
    }

    // FA8, second half: SIGINT is a request, not a guarantee. A cell that installs its own
    // handler (or sits in a C extension that never checks signals) outlives the interrupt,
    // and the loop then drops out of the grace window — which used to be SILENT, leaving a
    // page that says "timeout" while the runaway still owns the warm kernel and every later
    // cell queues behind it. Say so instead, and put the pid on the console.
    #[test]
    fn an_ignored_interrupt_is_reported_rather_than_read_as_a_plain_timeout() {
        let Some(py) = std::env::var_os("TALIESIN_PYTHON") else {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL is set but TALIESIN_PYTHON is unset: the live-kernel \
                 tests would silently skip. Point TALIESIN_PYTHON at a python with ipykernel."
            );
            eprintln!("SKIPPED (no live kernel): set TALIESIN_PYTHON to exercise the cell caps.");
            return;
        };
        let py = PathBuf::from(py);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut k = Kernel::start_with_retry(&KernelSpec::python(&py), None)
                .await
                .expect("kernel should start");
            k.cell_cap = None;
            k.silence_cap = Some(Duration::from_secs(1));

            let t = std::time::Instant::now();
            let out = render_outputs(
                &k.execute(
                    "import signal, time\n\
                     signal.signal(signal.SIGINT, lambda *a: None)\n\
                     time.sleep(120)",
                )
                .await
                .unwrap(),
            );
            assert!(
                out.contains("no output for 1s"),
                "the silence cap did not fire: {out}"
            );
            assert!(
                out.contains("still running"),
                "the interrupt was swallowed and the cell is still running in the kernel, \
                 but the page says only that a cap fired (FA8): {out}"
            );
            // The cap (1s) plus the grace window plus the shell drain, and no longer: the
            // point of the escalation is that it does not wait on a cell that will not stop.
            assert!(
                t.elapsed() < INTERRUPT_GRACE + Duration::from_secs(15),
                "the loop waited {:?} on a cell that ignored its interrupt",
                t.elapsed()
            );
        });
    }

    // FA27: the startup preambles are ~270 lines of version-sensitive Python run against
    // whatever interpreter the author points us at, and their failure used to be dropped
    // on the floor (`let _ = kernel.execute(...)`). The author then met the symptom cells
    // later — a `NameError` on `define`, or figures that stopped matching the page — with
    // nothing linking it to startup. A poisoned preamble stands in for the future Python
    // that breaks a real one.
    #[test]
    fn a_failing_startup_preamble_names_the_interpreter_and_what_broke() {
        let Some(py) = std::env::var_os("TALIESIN_PYTHON") else {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL is set but TALIESIN_PYTHON is unset: the live-kernel \
                 tests would silently skip. Point TALIESIN_PYTHON at a python with ipykernel."
            );
            eprintln!("SKIPPED (no live kernel): set TALIESIN_PYTHON to exercise the preambles.");
            return;
        };
        let py = PathBuf::from(py);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut k = Kernel::start_with_retry(&KernelSpec::python(&py), None)
                .await
                .expect("kernel should start");

            // The shipped preambles run clean on this interpreter: the check must not be
            // one that fires on a healthy start.
            let healthy = run_preambles(&mut k, &KernelSpec::python(&py)).await;
            assert!(
                healthy.is_empty(),
                "the shipped preambles reported a problem on a working interpreter: {healthy:?}"
            );

            // Both ways a preamble breaks: raising out (the `define` bridge is unguarded)
            // and reporting on stderr (the matplotlib hook catches its own failure so the
            // rest of the kernel survives).
            let mut poisoned = KernelSpec::python(&py);
            poisoned.preambles = &[
                Preamble {
                    provides: "the bridge is gone",
                    code: "raise RuntimeError('poisoned preamble')",
                },
                Preamble {
                    provides: "the hook is gone",
                    code: "import sys; print('theme hook not installed: nope', file=sys.stderr)",
                },
            ];
            let problems = run_preambles(&mut k, &poisoned).await;
            assert_eq!(
                problems.len(),
                2,
                "both failure shapes report: {problems:?}"
            );
            assert!(
                problems[0].contains("poisoned preamble")
                    && problems[0].contains(&py.display().to_string())
                    && problems[0].contains("the bridge is gone"),
                "an exception must name the interpreter, the error and the casualty: {}",
                problems[0]
            );
            assert!(
                problems[1].contains("theme hook not installed")
                    && problems[1].contains("the hook is gone"),
                "a stderr report must be read back the same way: {}",
                problems[1]
            );

            // A preamble that merely prints is not a failure.
            let mut chatty = KernelSpec::python(&py);
            chatty.preambles = &[Preamble {
                provides: "nothing",
                code: "print('hello from a preamble')",
            }];
            assert!(
                run_preambles(&mut k, &chatty).await.is_empty(),
                "ordinary preamble output must not be reported as a failure"
            );
        });
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
                let pid = k.proc.id().expect("an owned kernel has a pid");
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
