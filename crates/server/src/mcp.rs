//! `taliesin mcp`: a local, offline, stdio JSON-RPC MCP server exposing Taliesin's
//! read/validate/build surfaces as tools, so an MCP host drives the loop without shelling
//! out per call.
//!
//! **No tool writes `.tmd` source.** There is deliberately no write/edit/preview tool: the
//! `.tmd` stays the agent's direct edit surface (the single-editing-surface guardrail,
//! pinned by the `tools/list` assertion in `mcp_stdio.rs`). Each tool WRAPS an existing
//! collection fn (`check::check_json`, `query::symbols_json`/`map_json`/`read_text`,
//! `build::build_json`, `vocab::VOCAB_JSON`) — no re-implementation, no shell-out to itself.
//!
//! **This is not a sandbox, and a host must not allowlist it as one.** The guardrail above
//! is about the *editing surface*; it is not containment. This module used to say
//! "read/validate/build ONLY", which reads as a stronger promise than the source makes, so
//! two things are stated plainly here instead:
//!
//!  - **There is no project root and no path containment.** A `path` argument reaches the
//!    wrapped fn exactly as given: no canonicalization, no confinement. Any tool therefore
//!    reads any file this process can read (verified: `read {"path": "/etc/passwd"}`
//!    returns it, as does a `../`-climbing relative path). [`cmd_mcp`] discards its args,
//!    so no root exists even in principle.
//!  - **`build` is not side-effect-free.** It writes HTML beside whatever path it is handed
//!    and executes the document's code cells, which launches an interpreter.
//!
//! Documented rather than implemented (owner ruling 2026-07-17): a root here would withhold
//! nothing real, since a host that can run this binary already has the filesystem access it
//! would pretend to withhold. The boundary that counts is the host's process sandbox and
//! working directory. The point of writing it down is that a host operator reads the
//! guarantee we actually make instead of inferring a bigger one from the tool names.
//!
//! Transport is hand-rolled newline-delimited JSON-RPC 2.0 over stdin/stdout (zero new
//! deps, offline-guaranteed). All logging goes to stderr, so stdout is a clean JSON-RPC
//! stream.

use std::io::{BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

use serde_json::{Value, json};

/// The MCP protocol revision this server speaks (echoed back on `initialize`).
const PROTOCOL_VERSION: &str = "2024-11-05";

/// One exposed tool: its name, one-line description, and whether it takes a `path` argument.
struct Tool {
    name: &'static str,
    description: &'static str,
    /// `true` for the tools that operate on a file/dir (`path` required); `false` for
    /// `vocab` (no arguments).
    takes_path: bool,
}

/// The read/validate/build tool set. NO write/edit/preview tool: the `.tmd` is the agent's
/// edit surface, not this server's.
const TOOLS: &[Tool] = &[
    Tool {
        name: "check",
        description: "Validate a .tmd file or project directory. Runs no code and touches no network. Returns {diagnostics, environment}: each diagnostic carries a stable code, severity, file/line, message, and (for a typo) a suggested replacement. An environment entry names the interpreter a language would use; when the project itself supplied that interpreter (a _site.yml python:/r: field, or its .venv) it is NOT spawned, so `runs` is null and `not_probed` says why.",
        takes_path: true,
    },
    Tool {
        name: "read",
        description: "Project a rendered .tmd document to plain text (headings, resolved figure/cross-reference numbers, callouts, fenced code, math as TeX) — the agent's browser-free view of what it made.",
        takes_path: true,
    },
    Tool {
        name: "symbols",
        description: "List a .tmd document's cross-reference targets (every anchor you can name after @, with its resolved number).",
        takes_path: true,
    },
    Tool {
        name: "map",
        description: "Outline a whole project directory: pages in nav/chapter order, nav, mounts, and the cross-reference graph.",
        takes_path: true,
    },
    Tool {
        name: "vocab",
        description: "The closed-set vocabulary Taliesin accepts (front-matter keys, cell options, callout/theorem kinds, div classes, cross-reference prefixes) as JSON. Takes no arguments.",
        takes_path: false,
    },
    Tool {
        name: "build",
        description: "Build a .tmd file or project directory to self-contained HTML (executes code cells). Returns the structured {diagnostics:[…]}.",
        takes_path: true,
    },
];

/// `taliesin mcp`: run the stdio JSON-RPC loop until stdin closes.
pub(crate) fn cmd_mcp(_args: &[String]) -> ExitCode {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else {
            // A malformed line: JSON-RPC parse error, id null (we can't know the real id).
            let _ = writeln!(stdout, "{}", rpc_error(Value::Null, -32700, "parse error"));
            let _ = stdout.flush();
            continue;
        };
        // A request has an `id`; a notification does not (and gets no response).
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let outcome = dispatch(method, &req);
        if let Some(id) = id {
            let msg = match outcome {
                Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string(),
                Err((code, message)) => rpc_error(id, code, &message),
            };
            let _ = writeln!(stdout, "{msg}");
            let _ = stdout.flush();
        }
    }
    ExitCode::SUCCESS
}

fn rpc_error(id: Value, code: i64, message: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}).to_string()
}

/// [`handle`] under a panic boundary. An MCP session is persistent: an unguarded panic
/// unwinds out of the read loop and kills the server, so every *subsequent* tool call from
/// the agent fails, not just the offending one. Turn it into a JSON-RPC InternalError and
/// keep serving instead. Mirrors what `serve`/`build` already do around rendering.
fn dispatch(method: &str, req: &Value) -> Result<Value, (i64, String)> {
    match crate::serve::guarded(|| handle(method, req)) {
        Ok(outcome) => outcome,
        Err(panic) => {
            crate::log::error(&format!("mcp: panic handling {method}: {panic}"));
            Err((-32603, format!("internal error handling {method}")))
        }
    }
}

/// Dispatch a JSON-RPC method to its result (or a `(code, message)` error).
fn handle(method: &str, req: &Value) -> Result<Value, (i64, String)> {
    #[cfg(test)]
    assert!(method != PANIC_PROBE_METHOD, "injected mcp panic");
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "taliesin", "version": taliesin_core::VERSION },
        })),
        // A notification (no id) — nothing to return; the loop won't send a response anyway.
        "notifications/initialized" | "notifications/cancelled" => Ok(Value::Null),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => tools_call(req.get("params").unwrap_or(&Value::Null)),
        other => Err((-32601, format!("method not found: {other}"))),
    }
}

/// The `tools/list` payload: each tool's name, description, and JSON-Schema input.
fn tool_definitions() -> Vec<Value> {
    TOOLS
        .iter()
        .map(|t| {
            let input_schema = if t.takes_path {
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path to a .tmd file or project directory." }
                    },
                    "required": ["path"],
                })
            } else {
                json!({ "type": "object", "properties": {} })
            };
            json!({ "name": t.name, "description": t.description, "inputSchema": input_schema })
        })
        .collect()
}

/// Handle `tools/call`: look up the tool, run it, wrap the output as MCP text content.
fn tools_call(params: &Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "tools/call requires a tool name".to_string()))?;
    let args = params.get("arguments").unwrap_or(&Value::Null);
    let path = args.get("path").and_then(Value::as_str);

    let result = match name {
        "vocab" => Ok(taliesin_core::vocab::VOCAB_JSON.to_string()),
        "check" => run_path_tool(path, name, |p| Ok(crate::check::check_json(Path::new(p)))),
        "build" => run_path_tool(path, name, |p| Ok(crate::build::build_json(Path::new(p)))),
        "read" => run_path_tool(path, name, crate::query::read_text),
        "symbols" => run_path_tool(path, name, crate::query::symbols_json),
        "map" => run_path_tool(path, name, crate::query::map_json),
        other => return Err((-32602, format!("unknown tool: {other}"))),
    };

    // A tool-level failure is reported as an MCP `isError` result (not a JSON-RPC error),
    // per the tools/call contract.
    Ok(match result {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
        Err(msg) => json!({ "content": [{ "type": "text", "text": msg }], "isError": true }),
    })
}

/// Run a tool that needs a `path` argument, erroring cleanly if it's missing.
fn run_path_tool(
    path: Option<&str>,
    tool: &str,
    f: impl FnOnce(&str) -> Result<String, String>,
) -> Result<String, String> {
    match path {
        Some(p) => f(p),
        None => Err(format!("the `{tool}` tool requires a `path` argument")),
    }
}

/// Test-only method name that panics inside [`handle`]. Real input does not panic the
/// dispatch, so injecting one here is the only way to exercise the panic boundary.
/// `#[cfg(test)]`, so it is absent from the shipped binary.
#[cfg(test)]
const PANIC_PROBE_METHOD: &str = "taliesin/testPanic";

#[cfg(test)]
mod tests {
    use super::*;

    // The resilience property the unguarded loop could not have: a panicking method yields a
    // JSON-RPC InternalError, and the *next* call still answers. Mutation check: drop the
    // `guarded` in `dispatch` and this test aborts the harness thread instead of failing.
    #[test]
    fn a_panicking_method_becomes_an_error_and_the_next_call_still_answers() {
        let prior = std::panic::take_hook();
        // The injected panic is expected; don't spray a backtrace over the test output.
        std::panic::set_hook(Box::new(|_| {}));
        let panicked = dispatch(PANIC_PROBE_METHOD, &json!({}));
        std::panic::set_hook(prior);

        let (code, message) = panicked.expect_err("a panicking method must report an error");
        assert_eq!(code, -32603, "JSON-RPC InternalError");
        assert!(
            message.contains(PANIC_PROBE_METHOD),
            "names the method: {message}"
        );

        // The boundary's whole point: the dispatcher is still usable afterwards.
        let after = dispatch("ping", &json!({})).expect("dispatch survives a prior panic");
        assert_eq!(after, json!({}));
    }
}
