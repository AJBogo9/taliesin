//! `taliesin mcp`: a local, offline, stdio JSON-RPC MCP server exposing Taliesin's
//! read/validate/build surfaces as tools, so an MCP host drives the loop without shelling
//! out per call.
//!
//! **Read/validate/build ONLY.** There is deliberately no write/edit/preview tool: the
//! `.tmd` stays the agent's direct edit surface (the single-editing-surface guardrail,
//! pinned by the `tools/list` assertion in `mcp_stdio.rs`). Each tool WRAPS an existing
//! collection fn (`check::check_json`, `query::symbols_json`/`map_json`/`read_text`,
//! `build::build_json`, `vocab::VOCAB_JSON`) — no re-implementation, no shell-out to itself.
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
        description: "Validate a .tmd file or project directory. Returns {diagnostics, environment}: each diagnostic carries a stable code, severity, file/line, message, and (for a typo) a suggested replacement.",
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
        let outcome = handle(method, &req);
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

/// Dispatch a JSON-RPC method to its result (or a `(code, message)` error).
fn handle(method: &str, req: &Value) -> Result<Value, (i64, String)> {
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
