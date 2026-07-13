//! `taliesin mcp` is a stdio JSON-RPC MCP server over Taliesin's read/validate/build
//! surfaces. This pins the load-bearing contract:
//!  (a) `tools/list` is exactly the read/validate/build set — NO write/edit/preview tool
//!      (the single-editing-surface guardrail: the .tmd stays the agent's edit surface);
//!  (b) the `check` tool returns the same `{diagnostics, environment}` as
//!      `check --format json` (so the two channels can't drift);
//!  (c) every line on stdout is valid JSON-RPC (log noise goes to stderr only).

use std::collections::BTreeSet;
use std::io::Write;
use std::process::{Command, Stdio};

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

/// Drive an `taliesin mcp` session: write each request line to stdin, close it, and return
/// the parsed stdout response lines (in order).
fn mcp_session(requests: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn taliesin mcp");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        for r in requests {
            writeln!(stdin, "{r}").expect("write request");
        }
        // Dropping stdin closes it, so the server's read loop hits EOF and exits.
    }
    let out = child.wait_with_output().expect("wait mcp");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str(l)
                .unwrap_or_else(|e| panic!("stdout line is not JSON-RPC ({e}): {l}"))
        })
        .collect()
}

fn req(id: i64, method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

#[test]
fn tools_list_exposes_read_validate_build_only() {
    let responses = mcp_session(&[
        req(1, "initialize", serde_json::json!({})),
        req(2, "tools/list", serde_json::json!({})),
    ]);
    let list = responses
        .iter()
        .find(|r| r["id"] == 2)
        .expect("tools/list response");
    let names: BTreeSet<String> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    let expected: BTreeSet<String> = ["check", "read", "symbols", "map", "vocab", "build"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(names, expected, "exactly the read/validate/build tool set");
    // The single-editing-surface guardrail: no tool that writes/edits the source or opens a
    // preview may exist.
    for forbidden in ["write", "edit", "preview", "serve", "new", "init"] {
        assert!(
            !names.contains(forbidden),
            "the MCP server must not expose a `{forbidden}` tool"
        );
    }
}

#[test]
fn check_tool_matches_check_format_json() {
    let path = corpus("diagnostics/typos.tmd");
    let responses = mcp_session(&[
        req(1, "initialize", serde_json::json!({})),
        req(
            2,
            "tools/call",
            serde_json::json!({ "name": "check", "arguments": { "path": path } }),
        ),
    ]);
    let call = responses
        .iter()
        .find(|r| r["id"] == 2)
        .expect("call response");
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let via_mcp: serde_json::Value = serde_json::from_str(text).expect("tool output is json");

    // The same payload as the CLI's check --format json.
    let cli = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["check", &path, "--format", "json"])
        .output()
        .expect("run check");
    let via_cli: serde_json::Value = serde_json::from_slice(&cli.stdout).expect("cli json");

    assert_eq!(
        via_mcp["diagnostics"], via_cli["diagnostics"],
        "the check tool's diagnostics must equal check --format json"
    );
    assert!(
        via_mcp.get("environment").is_some(),
        "the check tool carries the environment probe too"
    );
}

#[test]
fn unknown_method_is_a_jsonrpc_error_not_a_crash() {
    let responses = mcp_session(&[req(1, "no/such/method", serde_json::json!({}))]);
    let r = responses.iter().find(|r| r["id"] == 1).expect("response");
    assert_eq!(r["error"]["code"], -32601, "method-not-found: {r}");
}
