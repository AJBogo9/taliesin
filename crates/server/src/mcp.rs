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

/// One exposed resource: a document an agent may read, addressed by URI.
///
/// **These are not new capabilities; they are the same answers off the tool path.** A tool
/// call is a request to *do* something, and the host pays for it with a round trip (and, for
/// a host that shells out, a process). "Give me the `_site.yml` schema" and "what does
/// `TAL-FM-KEY` mean" are not actions — they are documents, which is exactly what MCP
/// resources are for. Everything below already existed as a `taliesin` subcommand.
struct Resource {
    uri: &'static str,
    name: &'static str,
    description: &'static str,
    mime: &'static str,
}

const RESOURCES: &[Resource] = &[
    Resource {
        uri: "taliesin://schema/site",
        name: "_site.yml JSON Schema",
        description: "The JSON Schema for a project's `_site.yml`. The same file `taliesin schema` writes, so an agent validates the config against the schema the tool itself enforces.",
        mime: "application/schema+json",
    },
    Resource {
        uri: "taliesin://schema/frontmatter",
        name: "front-matter JSON Schema",
        description: "The JSON Schema for a `.tmd` document's YAML front matter: every key, its type, and which are nested under `execute:`/`hero:`/`listing:`.",
        mime: "application/schema+json",
    },
    Resource {
        uri: "taliesin://vocab",
        name: "the .tmd vocabulary",
        description: "The closed-set vocabulary Taliesin accepts (front-matter keys, cell options, callout and theorem kinds, div classes, cross-reference prefixes) as JSON. Read this before inventing a key.",
        mime: "application/json",
    },
    Resource {
        uri: "taliesin://agents",
        name: "AGENTS.md — the authoring loop",
        description: "The agent onramp: the whole loop (edit the .tmd, gate on `check`, the dialect) in one document. The same text `taliesin init` scaffolds into a new project.",
        mime: "text/markdown",
    },
    Resource {
        uri: "taliesin://diagnostics",
        name: "the diagnostic catalogue",
        description: "Every `TAL-*` code with its cause and canonical fix. Read `taliesin://diagnostic/{code}` for one of them.",
        mime: "text/markdown",
    },
];

/// The one URI *template*: a diagnostic code resolves to its own explanation.
///
/// A template rather than 47 listed resources, and rather than a tool call: an agent that has
/// just read `error[TAL-FM-KEY]` out of a `check` result needs that one code, and spawning a
/// `check --explain` process per lookup is the cost this removes.
const DIAGNOSTIC_TEMPLATE: &str = "taliesin://diagnostic/{code}";

/// One prompt: a scaffold, offered as something a host can put in front of a user.
///
/// **Why prompts and not another tool.** `taliesin new post|page|deck|paper` already encodes
/// what this project thinks a post, a page, a deck and a paper *are* — which front matter each
/// carries, what a first section looks like. Without prompts an agent reverse-engineers that
/// from the docs and gets a plausible-looking document that is not this project's idiom. A
/// prompt is how a host offers the answer instead.
struct Prompt {
    /// Matches [`crate::cli::NEW_KINDS`], asserted by `every_scaffold_kind_has_a_prompt`.
    kind: &'static str,
    description: &'static str,
    /// What the agent should do, with `{slug}` substituted from the argument.
    body: &'static str,
}

const PROMPTS: &[Prompt] = &[
    Prompt {
        kind: "post",
        description: "Start a dated blog post in this project's idiom.",
        body: "Run `taliesin new post {slug}` in the project directory. It writes a `.tmd` with the front matter a post needs (`title:`, `date:`, `description:`) and a first section.\n\nThen edit that `.tmd` — it is the only editing surface; never write HTML, and never edit anything under `_site/` or `_freeze/`. When you are done, call the `check` tool on the project directory and fix every diagnostic it reports; read `taliesin://diagnostic/{code}` for any code you do not recognise.",
    },
    Prompt {
        kind: "page",
        description: "Start a standalone page in this project's idiom.",
        body: "Run `taliesin new page {slug}` in the project directory. A page is undated and is reached from the nav rather than from a listing, so it carries `title:` and `description:` and no `date:`.\n\nThen edit that `.tmd` — it is the only editing surface. When you are done, call the `check` tool on the project directory and fix every diagnostic it reports.",
    },
    Prompt {
        kind: "deck",
        description: "Start a slide deck in this project's idiom.",
        body: "Run `taliesin new deck {slug}` in the project directory. A deck is `format: deck` in front matter and splits on `##` headings, one slide per heading; there is no separate slide syntax to learn.\n\nThen edit that `.tmd` — it is the only editing surface. Preview it with `taliesin preview <file.tmd>`, and gate it with the `check` tool like any other page.",
    },
    Prompt {
        kind: "paper",
        description: "Start an academic paper in this project's idiom.",
        body: "Run `taliesin new paper {slug}` in the project directory. A paper carries the scholarly front matter (`authors:`, `abstract:`, `bibliography:`) and expects `[@key]` citations against a `.bib`.\n\nThen edit that `.tmd` — it is the only editing surface. Cross-reference figures and sections with `@fig-`/`@sec-` anchors rather than writing \"see above\"; call the `check` tool to catch a citation or anchor that resolves to nothing.",
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
            // Three capabilities, not one. `resources` and `prompts` carry no `listChanged`:
            // both lists are compiled into the binary, so there is nothing to notify about
            // and claiming otherwise would ask a host to subscribe to silence.
            "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
            "serverInfo": { "name": "taliesin", "version": taliesin_core::VERSION },
        })),
        // A notification (no id) — nothing to return; the loop won't send a response anyway.
        "notifications/initialized" | "notifications/cancelled" => Ok(Value::Null),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => tools_call(req.get("params").unwrap_or(&Value::Null)),
        "resources/list" => Ok(json!({ "resources": resource_definitions() })),
        "resources/templates/list" => Ok(json!({ "resourceTemplates": template_definitions() })),
        "resources/read" => resources_read(req.get("params").unwrap_or(&Value::Null)),
        "prompts/list" => Ok(json!({ "prompts": prompt_definitions() })),
        "prompts/get" => prompts_get(req.get("params").unwrap_or(&Value::Null)),
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

/// The `resources/list` payload.
fn resource_definitions() -> Vec<Value> {
    RESOURCES
        .iter()
        .map(|r| {
            json!({
                "uri": r.uri, "name": r.name,
                "description": r.description, "mimeType": r.mime,
            })
        })
        .collect()
}

/// The `resources/templates/list` payload: the one parameterised URI.
fn template_definitions() -> Vec<Value> {
    vec![json!({
        "uriTemplate": DIAGNOSTIC_TEMPLATE,
        "name": "one diagnostic code, explained",
        "description": "The cause and the canonical fix for a single `TAL-*` code — the same text `check --explain <CODE>` prints. Substitute the code you read out of a `check` result (case-insensitive), e.g. `taliesin://diagnostic/TAL-FM-KEY`.",
        "mimeType": "text/markdown",
    })]
}

/// Handle `resources/read`: resolve a URI to its text.
///
/// Everything here is compiled into the binary or generated from it, so a read touches no
/// filesystem and no network — which is the difference between this and the tools, and worth
/// keeping true: a host may reasonably treat a resource read as cheap and safe.
fn resources_read(params: &Value) -> Result<Value, (i64, String)> {
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .ok_or((-32602, "resources/read requires a `uri`".to_string()))?;
    let (mime, text) =
        resource_body(uri).ok_or_else(|| (-32602, format!("no such resource: {uri}")))?;
    Ok(json!({
        "contents": [{ "uri": uri, "mimeType": mime, "text": text }],
    }))
}

/// `(mime, text)` for a resource URI, listed or templated. `None` for anything else.
fn resource_body(uri: &str) -> Option<(&'static str, String)> {
    use taliesin_core::diagnostics::codes;
    // The template first: it is the open half of the space, and none of the fixed URIs
    // below can start with this prefix.
    if let Some(code) = uri.strip_prefix("taliesin://diagnostic/") {
        let e = codes::explain(code)?;
        return Some((
            "text/markdown",
            format!(
                "# {}\n\n_{}_\n\n## Cause\n\n{}\n\n## Fix\n\n{}\n",
                e.code, e.title, e.cause, e.fix
            ),
        ));
    }
    let r = RESOURCES.iter().find(|r| r.uri == uri)?;
    let text = match uri {
        "taliesin://schema/site" => taliesin_core::schema::SITE_SCHEMA.to_string(),
        "taliesin://schema/frontmatter" => taliesin_core::schema::FRONTMATTER_SCHEMA.to_string(),
        "taliesin://vocab" => taliesin_core::vocab::VOCAB_JSON.to_string(),
        "taliesin://agents" => taliesin_core::agents::AGENTS_MD.to_string(),
        "taliesin://diagnostics" => codes::diagnostics_markdown(),
        // Unreachable: `r` was found in RESOURCES, and every row is handled above. A row
        // added without a body arrives here rather than serving an empty document.
        _ => return None,
    };
    Some((r.mime, text))
}

/// The `prompts/list` payload. Each takes the slug the scaffold will name the file after.
fn prompt_definitions() -> Vec<Value> {
    PROMPTS
        .iter()
        .map(|p| {
            json!({
                "name": format!("new-{}", p.kind),
                "description": p.description,
                "arguments": [{
                    "name": "slug",
                    "description": "The file name to create, lowercase with hyphens (e.g. `scree-slopes`).",
                    "required": true,
                }],
            })
        })
        .collect()
}

/// Handle `prompts/get`: the instruction text, with the caller's slug in it.
fn prompts_get(params: &Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "prompts/get requires a `name`".to_string()))?;
    let prompt = PROMPTS
        .iter()
        .find(|p| format!("new-{}", p.kind) == name)
        .ok_or_else(|| (-32602, format!("no such prompt: {name}")))?;
    // A missing slug is not an error: the host may be showing the prompt before the user has
    // typed one, and a placeholder the agent can see is more useful than a refusal.
    let slug = params
        .get("arguments")
        .and_then(|a| a.get("slug"))
        .and_then(Value::as_str)
        .unwrap_or("<slug>");
    Ok(json!({
        "description": prompt.description,
        "messages": [{
            "role": "user",
            "content": { "type": "text", "text": prompt.body.replace("{slug}", slug) },
        }],
    }))
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

    /// A scaffold kind with no prompt is a kind an agent has to reverse-engineer from the
    /// docs, which is the exact gap `prompts` closes. This is the gate that makes adding a
    /// fifth `taliesin new` kind fail here rather than quietly ship a half-offered surface.
    #[test]
    fn every_scaffold_kind_has_a_prompt() {
        let offered: Vec<&str> = PROMPTS.iter().map(|p| p.kind).collect();
        assert_eq!(
            offered,
            crate::cli::NEW_KINDS.to_vec(),
            "`taliesin new` kinds and MCP prompts must be the same list, in the same order"
        );
    }

    /// Every listed resource must actually read. A row with no body would list a document
    /// and then refuse it, which is worse than not listing it.
    #[test]
    fn every_listed_resource_resolves_to_a_body() {
        for r in RESOURCES {
            let (mime, text) = resource_body(r.uri)
                .unwrap_or_else(|| panic!("{} is listed but does not resolve", r.uri));
            assert_eq!(mime, r.mime, "{} advertises a different mime type", r.uri);
            assert!(!text.trim().is_empty(), "{} resolved to nothing", r.uri);
        }
    }

    /// The template's whole point: a code read out of a `check` result resolves without a
    /// process per lookup. Case-insensitively, because a code arrives however it was printed.
    #[test]
    fn the_diagnostic_template_resolves_a_real_code() {
        let (_, text) = resource_body("taliesin://diagnostic/TAL-FM-KEY").expect("a real code");
        let e = taliesin_core::diagnostics::codes::explain("TAL-FM-KEY").unwrap();
        assert!(text.contains(e.cause), "the cause: {text}");
        assert!(text.contains(e.fix), "and the fix: {text}");
        assert!(
            resource_body("taliesin://diagnostic/tal-fm-key").is_some(),
            "a code arrives however it was printed"
        );
    }

    /// And the reject axis: an unknown URI is an error, not an empty document. A resource
    /// that silently answered "" would look to an agent like a code with no explanation.
    #[test]
    fn an_unknown_resource_is_refused_rather_than_answered_empty() {
        assert!(resource_body("taliesin://diagnostic/TAL-NOPE").is_none());
        assert!(resource_body("taliesin://nothing").is_none());
        let err = resources_read(&json!({ "uri": "taliesin://nothing" }))
            .expect_err("an unknown uri must be an error");
        assert_eq!(err.0, -32602, "InvalidParams");
    }

    /// The slug is what turns a prompt into an instruction the agent can follow verbatim.
    #[test]
    fn a_prompt_carries_the_slug_the_host_supplied() {
        let got = prompts_get(&json!({
            "name": "new-post",
            "arguments": { "slug": "scree-slopes" },
        }))
        .expect("a known prompt");
        let text = got["messages"][0]["content"]["text"].as_str().unwrap_or("");
        assert!(
            text.contains("taliesin new post scree-slopes"),
            "the command, ready to run: {text}"
        );
        assert!(
            !text.contains("{slug}"),
            "no placeholder may survive substitution: {text}"
        );
    }

    /// A host may show a prompt before the user has typed a slug. Refusing there would make
    /// the prompt unlistable in half the UIs that would offer it.
    #[test]
    fn a_prompt_without_a_slug_is_still_answered() {
        let got = prompts_get(&json!({ "name": "new-deck" })).expect("no slug is not an error");
        let text = got["messages"][0]["content"]["text"].as_str().unwrap_or("");
        assert!(text.contains("<slug>"), "a visible placeholder: {text}");
    }

    #[test]
    fn an_unknown_prompt_is_refused() {
        assert_eq!(
            prompts_get(&json!({ "name": "new-novel" }))
                .expect_err("unknown prompt")
                .0,
            -32602
        );
    }

    /// The capability block is the only thing that tells a host these exist. An unadvertised
    /// resource is one no host ever asks for, exactly as an unadvertised LSP provider is.
    #[test]
    fn initialize_advertises_all_three_capabilities() {
        let caps = handle("initialize", &json!({})).expect("initialize")["capabilities"].clone();
        for name in ["tools", "resources", "prompts"] {
            assert!(
                caps.get(name).is_some(),
                "`{name}` is served but not advertised: {caps}"
            );
        }
    }
}
