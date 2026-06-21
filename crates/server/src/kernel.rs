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
use qmd_fast_core::html_escape as esc;
use tokio::process::{Child, Command};
use tokio::time::{Instant, timeout};

/// Wall-clock cap on a single cell execution, after which the kernel is sent
/// SIGINT (`QMD_FAST_CELL_TIMEOUT` seconds, default 120; `0` disables the cap and
/// falls back to a per-output silent-hang backstop). Read once.
fn cell_timeout() -> Option<Duration> {
    static T: OnceLock<Option<Duration>> = OnceLock::new();
    *T.get_or_init(|| {
        let secs = std::env::var("QMD_FAST_CELL_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(120);
        (secs > 0).then(|| Duration::from_secs(secs))
    })
}

/// Python `ojs_define(**kwargs)`, run once at kernel start. Serializes each
/// keyword (with a pandas convenience for DataFrame/Series) and emits a
/// `<script type="ojs-define">` HTML output the OJS runtime consumes. Mirrors
/// Quarto's Jupyter setup so existing `.qmd` docs work unchanged.
const OJS_DEFINE_PREAMBLE: &str = r#"
def ojs_define(**kwargs):
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
    display(HTML('<script type="ojs-define">' + json.dumps(v) + '</script>'), metadata=dict(ojs_define=True))
globals()["ojs_define"] = ojs_define
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
///     emitted together as a `text/html` fragment whose `.qmd-fig-light` /
///     `.qmd-fig-dark` images the page swaps on a `data-theme` change, so the axes
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

        # (foreground, grid) per theme — kept in sync with --qmd-fg / --qmd-border
        # in assets/css/{base,dark}.css.
        _QMD_LIGHT = ('#1a1a1a', '#d0d0d0')
        _QMD_DARK = ('#e6e6e6', '#363a44')
        _qmd_pending_export = []
        _qmd_orig_png = [None]  # the real Figure->png formatter, captured once

        def _qmd_do_export(fig):
            # Write the (still-pristine) figure to the files a `#| fig-export:` cell
            # requested, with print-clean styling for LaTeX/print. PNG gets a print
            # DPI; vector formats (.pdf/.svg) are resolution-independent.
            if not _qmd_pending_export:
                return
            import os as _os, sys as _sys
            for _p in list(_qmd_pending_export):
                _d = _os.path.dirname(_p)
                if _d:
                    _os.makedirs(_d, exist_ok=True)
                _kw = {'bbox_inches': 'tight', 'facecolor': 'white', 'edgecolor': 'white'}
                if _p.lower().endswith('.png'):
                    _kw['dpi'] = 200
                try:
                    fig.savefig(_p, **_kw)
                except Exception as _e:
                    print('qmd-fast: fig-export failed for %r: %s' % (_p, _e), file=_sys.stderr)
            _qmd_pending_export.clear()

        def _qmd_recolour(fig, fg, grid):
            # Recolour foreground (text/spines/ticks) to `fg` and grid lines to
            # `grid`, and make axes backgrounds transparent. Returns the originals
            # so the figure can be restored exactly. Data colours are untouched.
            import matplotlib.text as _t
            saved = []
            for _o in fig.findobj(_t.Text):
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

        def _qmd_render(fig, fg, grid):
            # Produce a base64 PNG (transparent bg) with `fg`/`grid` recolouring,
            # restoring the live figure afterwards.
            _orig = _qmd_orig_png[0]
            _saved = _qmd_recolour(fig, fg, grid)
            try:
                return _orig(fig)
            finally:
                for _set, _val in reversed(_saved):
                    _set(_val)

        def _qmd_ensure_inline():
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

        def _qmd_install():
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
            if getattr(_cur_html, '_qmd_themed', False):
                return
            # Capture the real png formatter (the one matplotlib_inline registered),
            # before we replace it with the suppressor.
            try:
                _real = _png.lookup_by_type(Figure)
            except Exception:
                _real = None
            if _real is not None and not getattr(_real, '_qmd_suppress', False):
                _qmd_orig_png[0] = _real
            if _qmd_orig_png[0] is None:
                return
            def _themed_html(fig):
                if not fig.axes and not fig.lines:
                    return None  # empty figure: emit nothing (matches print_figure)
                _qmd_do_export(fig)
                _l = _qmd_render(fig, *_QMD_LIGHT)
                _d = _qmd_render(fig, *_QMD_DARK)
                if _l is None or _d is None:
                    return None
                return ('<img class="qmd-fig qmd-fig-light" alt="" src="data:image/png;base64,' + _l + '">'
                        '<img class="qmd-fig qmd-fig-dark" alt="" src="data:image/png;base64,' + _d + '">')
            _themed_html._qmd_themed = True
            _html.for_type(Figure, _themed_html)
            def _suppress(fig):
                return None
            _suppress._qmd_suppress = True
            _png.for_type(Figure, _suppress)

        def _qmd_export(paths, install=False):
            # Called via a line the executor prepends to a `#| fig-export:` cell.
            _qmd_pending_export[:] = [p for p in paths if p]
            if install:
                try:
                    import matplotlib.pyplot  # noqa: F401
                    _qmd_ensure_inline()
                    _qmd_install()
                except Exception:
                    pass

        def _qmd_pre(*_a, **_k):
            _info = _a[0] if _a else None
            _src = getattr(_info, 'raw_cell', '') or ''
            if ('matplotlib' in _src) or ('pyplot' in _src) or ('plt' in _src) or ('seaborn' in _src):
                try:
                    import matplotlib.pyplot  # noqa: F401
                    _qmd_ensure_inline()
                    _qmd_install()
                except Exception:
                    pass

        _ip.events.register('pre_run_cell', _qmd_pre)
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
                ]
            },
            preambles: &[OJS_DEFINE_PREAMBLE, MPL_THEME_PREAMBLE],
        }
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
            preambles: &[],
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
    },
}

/// A live kernel process plus its shell/iopub client connections.
pub struct Kernel {
    child: Child,
    shell: ClientShellConnection,
    iopub: ClientIoPubConnection,
    conn_dir: PathBuf,
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

impl Kernel {
    /// Spawn the kernel described by `spec` (Python ipykernel or R IRkernel) and
    /// connect to it. The kernel stays warm for the lifetime of this value.
    pub async fn start(spec: &KernelSpec) -> io::Result<Kernel> {
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
            kernel_name: Some(spec.kernel_name.to_string()),
        };

        // The connection file holds the HMAC key + ZMQ ports — anyone who can read
        // it can drive the kernel. It lives in the shared temp dir, so lock it down:
        // a 0700 dir and a 0600 file, created with those modes from the start (no
        // world-readable window) on Unix.
        let conn_dir = std::env::temp_dir().join(format!("qmd-kernel-{}", uuid::Uuid::new_v4()));
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

        // Capture stderr so a startup failure (e.g. the interpreter lacks the
        // ipykernel/IRkernel module) can be reported instead of swallowed.
        let mut child = Command::new(&spec.program)
            .args((spec.argv)(&conn_file))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                io::Error::other(format!(
                    "cannot launch `{}`: {e} (is it installed / on PATH?)",
                    spec.program.display()
                ))
            })?;
        let child_stderr = child.stderr.take();

        let session = uuid::Uuid::new_v4().to_string();
        // Connect over ZMQ. The client's connect has a long (30s) timeout, so if
        // the interpreter dies at startup (missing ipykernel/IRkernel) we'd hang
        // the full timeout and then report an opaque "connect timed out". Instead,
        // race the connect against the process exiting: a bad interpreter then
        // fails in ~1s with its actual stderr (e.g. "No module named ipykernel").
        let connect = async {
            let mut iopub = create_client_iopub_connection(&info, "", &session)
                .await
                .map_err(io::Error::other)?;
            let identity = peer_identity_for_session(&session).map_err(io::Error::other)?;
            let shell = create_client_shell_connection_with_identity(&info, &session, identity)
                .await
                .map_err(io::Error::other)?;
            // Reading one iopub message confirms our SUB subscription is live, which
            // sidesteps the ZMQ slow-joiner problem before the first execution.
            let _ = wait_for_iopub_welcome(&mut iopub, Duration::from_secs(5)).await;
            Ok::<(ClientIoPubConnection, ClientShellConnection), io::Error>((iopub, shell))
        };

        let (iopub, shell) = tokio::select! {
            r = connect => r?,
            _ = child.wait() => {
                return Err(startup_failure(spec, child_stderr).await);
            }
        };

        let mut kernel = Kernel {
            child,
            shell,
            iopub,
            conn_dir,
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
        matches!(self.child.try_wait(), Ok(None))
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
        let mut capped = false;
        // Total wall-clock cap (not per-message, so a *streaming* runaway cell is
        // still caught). On hitting it we SIGINT the kernel, then drain a short
        // grace window so the resulting KeyboardInterrupt + Idle resync the
        // channels and the *next* cell still works.
        let cap = cell_timeout();
        let deadline = cap.map(|d| Instant::now() + d);
        let mut grace_until: Option<Instant> = None;
        loop {
            let budget = match grace_until {
                Some(g) => g.saturating_duration_since(Instant::now()),
                None => deadline
                    .map(|dl| dl.saturating_duration_since(Instant::now()))
                    .unwrap_or(Duration::from_secs(60)),
            };
            let msg = match timeout(budget, self.iopub.read()).await {
                Ok(Ok(msg)) => msg,
                Ok(Err(e)) => return Err(io::Error::other(e)),
                Err(_) if grace_until.is_some() => {
                    // The kernel ignored SIGINT within the grace window; give up on
                    // this cell. The channels may be desynced — the dev-menu
                    // "Restart kernel" is the escape hatch.
                    break;
                }
                Err(_) => match cap {
                    // Hit the hard cap: interrupt and switch to the grace window.
                    Some(d) => {
                        self.interrupt();
                        outputs.push(Output::Error {
                            ename: "Timeout".into(),
                            evalue: format!("cell exceeded {}s; sent interrupt", d.as_secs()),
                            traceback: vec![],
                        });
                        grace_until = Some(Instant::now() + Duration::from_secs(5));
                        continue;
                    }
                    // No cap (opt-out): a silent hang still times out per-output.
                    None => {
                        outputs.push(Output::Error {
                            ename: "Timeout".into(),
                            evalue: "cell produced no output for 60s".into(),
                            traceback: vec![],
                        });
                        break;
                    }
                },
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
                    text: format!("\n[qmd-fast: output truncated at {MAX_OUTPUTS} items]\n"),
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
                            text: format!(
                                "\n[qmd-fast: output truncated at {} KB]\n",
                                MAX_STREAM_BYTES / 1024
                            ),
                        });
                        capped = true;
                    }
                }
                JupyterMessageContent::ExecuteResult(r) if !capped => {
                    outputs.push(Output::Rich(render_media(&r.data)))
                }
                JupyterMessageContent::DisplayData(d) if !capped => {
                    outputs.push(Output::Rich(render_media(&d.data)))
                }
                JupyterMessageContent::ErrorOutput(e) => outputs.push(Output::Error {
                    ename: e.ename,
                    evalue: e.evalue,
                    traceback: e.traceback,
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
        if let Some(pid) = self.child.id() {
            // Safety: `kill` with a valid pid + signal is sound; a stale pid just
            // returns ESRCH, which we ignore.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGINT);
            }
        }
    }
}

impl Drop for Kernel {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
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
                    "qmd-stream qmd-stderr"
                } else {
                    "qmd-stream"
                };
                s.push_str(&format!("<pre class=\"{class}\">{}</pre>", esc(text)));
            }
            Output::Rich(html) => s.push_str(html),
            Output::Error {
                ename,
                evalue,
                traceback,
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
                s.push_str(&format!("<pre class=\"qmd-error\">{}</pre>", esc(&body)));
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
        assert_eq!(out, "<pre class=\"qmd-stream\">a &lt; b\n</pre>");
        let err = render_outputs(&[Output::Stream {
            stderr: true,
            text: "oops".into(),
        }]);
        assert!(
            err.contains("class=\"qmd-stream qmd-stderr\""),
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
        }]);
        assert_eq!(bare, "<pre class=\"qmd-error\">ValueError: bad</pre>");

        // A traceback is ANSI-stripped, joined, and escaped.
        let tb = render_outputs(&[Output::Error {
            ename: "E".into(),
            evalue: "v".into(),
            traceback: vec!["\u{1b}[31mline 1\u{1b}[0m".into(), "a < b".into()],
        }]);
        assert!(tb.contains("class=\"qmd-error\""), "got: {tb}");
        assert!(tb.contains("line 1"), "ansi not stripped: {tb}");
        assert!(!tb.contains("\u{1b}["), "raw ANSI leaked: {tb}");
        assert!(tb.contains("a &lt; b"), "traceback not escaped: {tb}");
    }

    // Runs only when QMD_FAST_PYTHON points at a python with ipykernel; without
    // one it reports ok WITHOUT exercising a real kernel (the pure-logic tests
    // above carry the unconditional coverage).
    #[test]
    fn kernel_executes_state_errors_and_interrupts_runaway_cell() {
        let Some(py) = std::env::var_os("QMD_FAST_PYTHON") else {
            eprintln!(
                "SKIPPED (no live kernel): set QMD_FAST_PYTHON to a python with ipykernel to \
                 actually exercise kernel.rs; this run did not."
            );
            return;
        };
        // Short per-cell cap so the runaway case below trips fast. Set before the
        // first execute(), since `cell_timeout()` reads the env once.
        // Safety: single-threaded test, before any threads observe the env.
        unsafe { std::env::set_var("QMD_FAST_CELL_TIMEOUT", "3") };
        let py = PathBuf::from(py);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut k = Kernel::start(&KernelSpec::python(&py))
                .await
                .expect("kernel should start");

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
        });
    }
}
