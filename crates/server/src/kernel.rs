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

/// Make inline matplotlib figures follow the page theme. Set once at kernel start
/// via the IPython InlineBackend config (so it's lazy: nothing is imported until a
/// cell actually uses matplotlib). A transparent figure/axes background lets the
/// page colour show through and track light/dark instantly, and a neutral grey for
/// axes, ticks, labels and text reads on both themes; data colours are untouched.
/// An author's explicit `style`/`facecolor` still overrides these defaults.
const MPL_THEME_PREAMBLE: &str = r#"
try:
    _ip = get_ipython()
    if _ip is not None:
        _ip.run_line_magic('config', "InlineBackend.rc = {'figure.facecolor': 'none', 'axes.facecolor': 'none', 'savefig.facecolor': 'none', 'savefig.edgecolor': 'none', 'text.color': '#888888', 'axes.edgecolor': '#888888', 'axes.labelcolor': '#888888', 'xtick.color': '#888888', 'ytick.color': '#888888', 'grid.color': '#888888', 'legend.framealpha': 0.0}")
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

        let conn_dir = std::env::temp_dir().join(format!("qmd-kernel-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&conn_dir)?;
        let conn_file = conn_dir.join("connection.json");
        std::fs::write(&conn_file, serde_json::to_vec(&info)?)?;

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
            match msg.content {
                JupyterMessageContent::StreamContent(s) => outputs.push(Output::Stream {
                    stderr: matches!(s.name, Stdio::Stderr),
                    text: s.text,
                }),
                JupyterMessageContent::ExecuteResult(r) => {
                    outputs.push(Output::Rich(render_media(&r.data)))
                }
                JupyterMessageContent::DisplayData(d) => {
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
        }
        // Drain the matching shell execute_reply so the channel stays in sync.
        let _ = timeout(Duration::from_secs(5), self.shell.read()).await;
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

    // Runs only when QMD_FAST_PYTHON points at a python with ipykernel.
    #[test]
    fn kernel_executes_state_errors_and_interrupts_runaway_cell() {
        let Some(py) = std::env::var_os("QMD_FAST_PYTHON") else {
            eprintln!("skipping kernel test: set QMD_FAST_PYTHON to a python with ipykernel");
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
