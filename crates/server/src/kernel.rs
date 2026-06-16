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
use std::time::Duration;

use jupyter_protocol::{
    ConnectionInfo, ExecuteRequest, JupyterMessage, JupyterMessageContent, Media, MediaType,
    Stdio, Transport,
};
use jupyter_zmq_client::{
    ClientIoPubConnection, ClientShellConnection, create_client_iopub_connection,
    create_client_shell_connection_with_identity, peek_ports, peer_identity_for_session,
    wait_for_iopub_welcome,
};
use tokio::process::{Child, Command};
use tokio::time::timeout;

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

/// One execution output, already rendered to a self-contained HTML fragment.
#[derive(Debug, Clone)]
pub enum Output {
    Stream { stderr: bool, text: String },
    /// Rich output (execute_result / display_data) rendered to HTML.
    Rich(String),
    Error { ename: String, evalue: String, traceback: Vec<String> },
}

/// A live kernel process plus its shell/iopub client connections.
pub struct Kernel {
    child: Child,
    shell: ClientShellConnection,
    iopub: ClientIoPubConnection,
    conn_dir: PathBuf,
}

impl Kernel {
    /// Spawn `python -m ipykernel_launcher` and connect to it. The kernel stays
    /// warm for the lifetime of this value.
    pub async fn start(python: &Path) -> io::Result<Kernel> {
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
            kernel_name: Some("python3".to_string()),
        };

        let conn_dir = std::env::temp_dir().join(format!("qmd-kernel-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&conn_dir)?;
        let conn_file = conn_dir.join("connection.json");
        std::fs::write(&conn_file, serde_json::to_vec(&info)?)?;

        let child = Command::new(python)
            .args(["-m", "ipykernel_launcher", "-f"])
            .arg(&conn_file)
            .arg("--quiet")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| io::Error::other(format!("failed to launch kernel ({python:?}): {e}")))?;

        let session = uuid::Uuid::new_v4().to_string();
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

        let mut kernel = Kernel { child, shell, iopub, conn_dir };
        // Define `ojs_define(**kwargs)` so docs can bridge Python values to OJS
        // cells: it emits `<script type="ojs-define">{json}</script>`, which the
        // Observable runtime reads. (Mirrors Quarto's Jupyter setup.)
        let _ = kernel.execute(OJS_DEFINE_PREAMBLE).await;
        Ok(kernel)
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
        loop {
            let msg = match timeout(Duration::from_secs(60), self.iopub.read()).await {
                Ok(Ok(msg)) => msg,
                Ok(Err(e)) => return Err(io::Error::other(e)),
                Err(_) => {
                    outputs.push(Output::Error {
                        ename: "Timeout".into(),
                        evalue: "cell execution exceeded 60s".into(),
                        traceback: vec![],
                    });
                    break;
                }
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
                let class = if *stderr { "qmd-stream qmd-stderr" } else { "qmd-stream" };
                s.push_str(&format!("<pre class=\"{class}\">{}</pre>", esc(text)));
            }
            Output::Rich(html) => s.push_str(html),
            Output::Error { ename, evalue, traceback } => {
                let tb: String = traceback.iter().map(|l| strip_ansi(l)).collect::<Vec<_>>().join("\n");
                let body = if tb.trim().is_empty() { format!("{ename}: {evalue}") } else { tb };
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
        return format!("<img alt=\"output\" src=\"data:image/png;base64,{}\" />", b.trim());
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
        return format!("<img alt=\"output\" src=\"data:image/jpeg;base64,{}\" />", b.trim());
    }
    if let Some(t) = pick(&|t| match t {
        MediaType::Plain(t) => Some(t.clone()),
        _ => None,
    }) {
        return format!("<pre>{}</pre>", esc(&t));
    }
    String::new()
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Runs only when QMD_FAST_PYTHON points at a python with ipykernel.
    #[test]
    fn kernel_executes_streams_results_state_and_errors() {
        let Some(py) = std::env::var_os("QMD_FAST_PYTHON") else {
            eprintln!("skipping kernel test: set QMD_FAST_PYTHON to a python with ipykernel");
            return;
        };
        let py = PathBuf::from(py);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut k = Kernel::start(&py).await.expect("kernel should start");

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
        });
    }
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
