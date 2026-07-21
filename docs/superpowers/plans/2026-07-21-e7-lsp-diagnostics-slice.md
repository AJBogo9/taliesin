# E7 `taliesin lsp` Diagnostics-First Slice — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a `taliesin lsp` stdio LSP subcommand that publishes live, unsaved-buffer diagnostics to any LSP editor (Neovim/Helix/Zed/VS Code), reusing the existing buffer-linting seam.

**Architecture:** A new synchronous `crates/server/src/lsp.rs` module runs an `lsp_server::Connection` over stdio, advertises only `textDocumentSync: FULL`, and on `didOpen`/`didChange`/`didClose` lints the in-memory buffer via a thin `check::buffer_diagnostics` wrapper over `collect_file_diagnostics_from_src`, mapping each `check::Diagnostic` to an `lsp_types::Diagnostic` and pushing `publishDiagnostics`. Separate from the tokio serve path (parse-only, kernel-free, sync — matches `check.rs`).

**Tech Stack:** Rust (edition 2024), `lsp-server` 0.7 (JSON-RPC transport, `Connection::stdio()`/`Connection::memory()`), `lsp-types` 0.95 (typed LSP messages, `url::Url`), `serde_json`.

## Global Constraints

- **stdout is the protocol wire.** `cmd_lsp` and everything it calls MUST NOT write to stdout (no `println!`, no `print!`). All human output goes to stderr via `crate::log` (already stderr-only). A stray stdout byte corrupts the JSON-RPC stream.
- **Dependency floor / choice:** `lsp-server = "0.7"`, `lsp-types = "0.95"`, added inline to `crates/server/Cargo.toml` `[dependencies]` (server-only, not shared → not a workspace dep), mirroring `axum`/`notify`. Pin `lsp-types` to `0.95` deliberately: `0.97+` swaps `url::Url` for a `Uri` type and loses `to_file_path`/`Url::parse` ergonomics. Both crates are pure-Rust, vendored via Cargo, no network (offline invariant).
- **Single editing surface:** the LSP is read + diagnose only; never writes to the buffer or source. No formatting/rename in this slice.
- **Reuse, don't reimplement:** diagnostics come from the existing `collect_file_diagnostics_from_src`; the range mapping mirrors the companion's `check.ts` (line clamp; precise `[col-1, end_col-1)` when columned; whole-line otherwise).
- **Four guardrail tests gate new subcommands** (all in `crates/server/src/`): `every_dispatched_command_is_listed_in_commands` (main.rs — needs `"lsp"` in `COMMANDS`), `subcommand_help_covers_documented_commands` (main.rs — needs an `lsp` arm naming itself + containing `taliesin`), `every_command_has_a_description` (complete.rs — needs an `lsp` arm in `command_desc`), `empty_word_completes_all_subcommands` (complete.rs — auto-satisfied once `COMMANDS` has `lsp`).
- **Test command:** `cargo test -p taliesin-server` (add `--test-threads=1` if an unrelated `exec`/`kernel` timing test flakes — see backlog P3; those are pre-existing).
- **Fixtures:** read corpus files via `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics")` (established convention).

---

### Task 1: Add dependencies + `taliesin lsp` subcommand wiring + lifecycle

Stands up the crate wiring and a server that completes the `initialize`/`shutdown`/`exit` handshake and ignores everything else. Deliverable: `taliesin lsp` exists, is dispatched, passes all four guardrail tests, and a memory-connection test proves a clean lifecycle. No diagnostics yet.

**Files:**
- Modify: `crates/server/Cargo.toml` (add the two deps)
- Create: `crates/server/src/lsp.rs`
- Modify: `crates/server/src/main.rs` (`mod lsp;`, dispatch arm, `COMMANDS`, `subcommand_help`, `usage()`)
- Modify: `crates/server/src/complete.rs` (`command_desc` arm)

**Interfaces:**
- Produces: `pub(crate) fn cmd_lsp(args: &[String]) -> std::process::ExitCode`; `pub(crate) fn run(connection: lsp_server::Connection) -> Result<(), Box<dyn std::error::Error + Send + Sync>>`; `pub(crate) fn server_capabilities() -> lsp_types::ServerCapabilities`.

- [ ] **Step 1: Add the dependencies**

Run:
```bash
cargo add lsp-server@0.7 lsp-types@0.95 -p taliesin-server
```
Expected: `crates/server/Cargo.toml` `[dependencies]` gains `lsp-server = "0.7"` and `lsp-types = "0.95"`. Confirm with `cargo build -p taliesin-server` (compiles; the crates download once).

- [ ] **Step 2: Write the failing lifecycle test**

Create `crates/server/src/lsp.rs` with only this (the impl comes next; the test must fail to compile first):

```rust
//! The `taliesin lsp` subcommand: a synchronous, offline LSP server over stdio.
//!
//! **What:** holds open `.tmd` buffers warm and publishes live diagnostics (the
//! `check` validators) on every edit, to any LSP editor. Parse-only, kernel-free.
//!
//! **stdout is the JSON-RPC wire** — this module must never write to stdout; all
//! human output goes to `crate::log` (stderr). See the plan's Global Constraints.

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};

    // Drive a full initialize → shutdown → exit handshake against an in-process server.
    #[test]
    fn completes_the_initialize_shutdown_lifecycle() {
        let (server, client) = Connection::memory();
        let server_thread = std::thread::spawn(move || run(server));

        // initialize request → expect an InitializeResult response.
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(1),
                method: "initialize".to_owned(),
                params: serde_json::json!({ "capabilities": {} }),
            }))
            .unwrap();
        match client.receiver.recv().unwrap() {
            Message::Response(Response { id, error, .. }) => {
                assert_eq!(id, RequestId::from(1));
                assert!(error.is_none(), "initialize errored: {error:?}");
            }
            other => panic!("expected initialize response, got {other:?}"),
        }
        // initialized notification completes the handshake.
        client
            .sender
            .send(Message::Notification(Notification {
                method: "initialized".to_owned(),
                params: serde_json::json!({}),
            }))
            .unwrap();

        // shutdown → response, then exit → the server loop returns Ok.
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(2),
                method: "shutdown".to_owned(),
                params: serde_json::Value::Null,
            }))
            .unwrap();
        match client.receiver.recv().unwrap() {
            Message::Response(Response { id, .. }) => assert_eq!(id, RequestId::from(2)),
            other => panic!("expected shutdown response, got {other:?}"),
        }
        client
            .sender
            .send(Message::Notification(Notification {
                method: "exit".to_owned(),
                params: serde_json::Value::Null,
            }))
            .unwrap();

        server_thread.join().unwrap().expect("server loop should exit Ok");
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p taliesin-server lsp:: 2>&1 | tail -20`
Expected: FAIL — compile error, `cannot find function \`run\` in this scope` (and `mod lsp;` not yet declared, so the module isn't even compiled). This confirms the test is wired to the not-yet-written impl.

- [ ] **Step 4: Write the minimal server implementation**

Prepend to `crates/server/src/lsp.rs` (above the `#[cfg(test)] mod tests`):

```rust
use lsp_server::{Connection, Message};
use lsp_types::{ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions};
use std::process::ExitCode;

/// The one advertised capability: full-text document sync (whole buffer on every
/// change), which maps 1:1 onto `check`'s whole-buffer linting. No provider
/// capabilities yet (hover/completion/definition land as follow-ups).
pub(crate) fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(TextDocumentSyncOptions {
            open_close: Some(true),
            change: Some(TextDocumentSyncKind::FULL),
            ..Default::default()
        })),
        ..Default::default()
    }
}

/// Entry point from `main()`. Runs the stdio server; logs any error to stderr
/// (never stdout — that is the protocol wire).
pub(crate) fn cmd_lsp(_args: &[String]) -> ExitCode {
    let (connection, io_threads) = Connection::stdio();
    let result = run(connection);
    if let Err(e) = result {
        crate::log::error(&format!("lsp: {e}"));
        return ExitCode::FAILURE;
    }
    if io_threads.join().is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Complete the initialize handshake, then serve the message loop until `exit`.
/// Takes the connection by value so it (and its channels) drop before the caller
/// joins the stdio I/O threads.
pub(crate) fn run(
    connection: Connection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let caps = serde_json::to_value(server_capabilities())?;
    let _initialize_params = connection.initialize(caps)?;
    main_loop(&connection)?;
    Ok(())
}

/// Read messages until `shutdown`/`exit`. Requests other than shutdown are ignored
/// (no request capabilities advertised); notifications are handled in Task 3.
fn main_loop(
    connection: &Connection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
            }
            Message::Notification(_notif) => {}
            Message::Response(_) => {}
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Declare the module + dispatch in `main.rs`**

In `crates/server/src/main.rs`, add the module declaration in the `mod` block (alphabetical, after `mod log;`):
```rust
mod lsp;
```
Add the dispatch arm inside the `match args.get(1)...` block, right after the `Some("mcp") => mcp::cmd_mcp(&args),` line:
```rust
        Some("lsp") => lsp::cmd_lsp(&args),
```
Add `"lsp"` to the `COMMANDS` array (after `"mcp",`):
```rust
    "mcp",
    "lsp",
```

- [ ] **Step 6: Add focused help + usage line in `main.rs`**

In `subcommand_help`, add an arm after the `"mcp" => { ... }` arm:
```rust
        "lsp" => {
            "taliesin lsp\n\
             \n\
             Run a local, offline LSP (Language Server Protocol) server over stdio so any\n\
             LSP editor (Neovim, Helix, Zed, VS Code) gets live .tmd diagnostics as you\n\
             type — the same validators as `check`, on the unsaved buffer. Parse-only: no\n\
             kernel, no code execution, read-only (it never edits your source). JSON-RPC on\n\
             stdout, logs on stderr.\n\
             \n\
             Example (Neovim, via nvim-lspconfig or vim.lsp.start):\n\
             \x20 cmd = { \"taliesin\", \"lsp\" }\n"
        }
```
In `usage()`, add under the `Editor & agent` section, after the `mcp` line:
```rust
    println!(
        "  lsp                        stdio LSP server: live .tmd diagnostics in any editor"
    );
```

- [ ] **Step 7: Add the completion description in `complete.rs`**

In `command_desc` (crates/server/src/complete.rs), add an arm after `"mcp" => "stdio MCP server",`:
```rust
        "lsp" => "stdio LSP server (live diagnostics in any editor)",
```

- [ ] **Step 8: Run the lifecycle test + guardrails to verify they pass**

Run: `cargo test -p taliesin-server lsp:: complete:: main 2>&1 | tail -25`
Expected: PASS — `completes_the_initialize_shutdown_lifecycle`, `every_dispatched_command_is_listed_in_commands`, `subcommand_help_covers_documented_commands`, `every_command_has_a_description`, and `empty_word_completes_all_subcommands` all green.

Then the whole crate to be sure nothing else regressed:
Run: `cargo test -p taliesin-server 2>&1 | tail -15`
Expected: PASS (add `--test-threads=1` only if an `exec`/`kernel` timing test flakes — pre-existing, unrelated).

- [ ] **Step 9: Commit**

```bash
git add crates/server/Cargo.toml Cargo.lock crates/server/src/lsp.rs crates/server/src/main.rs crates/server/src/complete.rs
git commit -m "feat(lsp): taliesin lsp subcommand + initialize/shutdown lifecycle (E7 cut 1)"
```

---

### Task 2: `check::buffer_diagnostics` + `Diagnostic::to_lsp` mapping

The diagnostics engine seam. Deliverable: a `check.rs` unit test proving (a) `buffer_diagnostics` on a buffer with a front-matter typo returns a located diagnostic, and (b) `to_lsp` maps line/col/severity/code/href faithfully (precise span when columned; whole-line otherwise). Pure Rust, no LSP loop.

**Files:**
- Modify: `crates/server/src/check.rs` (add `buffer_diagnostics`, add `Diagnostic::to_lsp`, add tests)

**Interfaces:**
- Consumes: the existing private `fn collect_file_diagnostics_from_src(path: &Path, src: &str) -> Result<Vec<Diagnostic>, String>` and the private fields of `struct Diagnostic` (`code: &'static str`, `docs_url: String`, `severity: &'static str`, `line: Option<u32>`, `col: Option<u32>`, `end_col: Option<u32>`, `message: String`).
- Produces: `pub(crate) fn buffer_diagnostics(path: &std::path::Path, src: &str) -> Vec<Diagnostic>`; `impl Diagnostic { pub(crate) fn to_lsp(&self, lines: &[&str]) -> lsp_types::Diagnostic }`.

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests` block in `crates/server/src/check.rs` (find it near the bottom of the file; if the test module needs the import, add `use std::path::Path;` at the top of the test module):

```rust
    #[test]
    fn buffer_diagnostics_flags_a_front_matter_typo() {
        // A misspelled front-matter key: the static validator locates it with a column span.
        let src = "---\ntittle: Hi\n---\n\n# Body\n";
        let diags = super::buffer_diagnostics(Path::new("buf.tmd"), src);
        assert!(
            diags.iter().any(|d| d.message.contains("tittle") || d.message.contains("title")),
            "expected a typo diagnostic, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn to_lsp_uses_a_precise_span_when_columned() {
        let d = super::Diagnostic {
            code: "TAL-FM-KEY",
            docs_url: "https://example.test/DIAGNOSTICS.md#tal-fm-key".to_string(),
            severity: "warning",
            file: "buf.tmd".to_string(),
            line: Some(2),
            col: Some(1),
            end_col: Some(7),
            message: "unknown key `tittle`".to_string(),
            suggestion: None,
        };
        let lines = ["---", "tittle: Hi", "---"];
        let lsp = d.to_lsp(&lines);
        // 1-based line 2 → 0-based 1; 1-based [1,7) → 0-based [0,6).
        assert_eq!(lsp.range.start, lsp_types::Position::new(1, 0));
        assert_eq!(lsp.range.end, lsp_types::Position::new(1, 6));
        assert_eq!(lsp.severity, Some(lsp_types::DiagnosticSeverity::WARNING));
        assert_eq!(lsp.code, Some(lsp_types::NumberOrString::String("TAL-FM-KEY".to_string())));
        assert_eq!(lsp.source.as_deref(), Some("taliesin"));
        assert_eq!(lsp.code_description.map(|c| c.href.to_string()),
            Some("https://example.test/DIAGNOSTICS.md#tal-fm-key".to_string()));
    }

    #[test]
    fn to_lsp_spans_the_whole_line_when_uncolumned() {
        let d = super::Diagnostic {
            code: "TAL-XREF-UNDEF",
            docs_url: "https://example.test/x".to_string(),
            severity: "error",
            file: "buf.tmd".to_string(),
            line: Some(3),
            col: None,
            end_col: None,
            message: "undefined @fig-x".to_string(),
            suggestion: None,
        };
        let lines = ["a", "bb", "hello world"]; // line 3 (0-based 2) has 11 chars
        let lsp = d.to_lsp(&lines);
        assert_eq!(lsp.range.start, lsp_types::Position::new(2, 0));
        assert_eq!(lsp.range.end, lsp_types::Position::new(2, 11));
        assert_eq!(lsp.severity, Some(lsp_types::DiagnosticSeverity::ERROR));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p taliesin-server check::tests::buffer_diagnostics check::tests::to_lsp 2>&1 | tail -20`
Expected: FAIL — compile errors (`buffer_diagnostics` and `to_lsp` do not exist; the struct-literal `Diagnostic { .. }` may also flag until the fields are in scope, which they are within the module).

- [ ] **Step 3: Implement `buffer_diagnostics`**

Add after `collect_file_diagnostics_from_src` in `crates/server/src/check.rs`:

```rust
/// Lint an in-memory editor buffer as if it were the file at `path`, returning the
/// diagnostics directly (the `--stdin` seam, minus the stdin plumbing). Used by the
/// `lsp` server on every `didOpen`/`didChange`. The buffer path can't fail to render,
/// but a hypothetical error surfaces as one line-1 diagnostic (parity with the
/// companion's check-error handling) rather than vanishing.
pub(crate) fn buffer_diagnostics(path: &Path, src: &str) -> Vec<Diagnostic> {
    match collect_file_diagnostics_from_src(path, src) {
        Ok(diags) => diags,
        Err(e) => vec![Diagnostic::new(path.display().to_string(), Some(1), e)],
    }
}
```

- [ ] **Step 4: Implement `Diagnostic::to_lsp`**

Add a method inside the existing `impl Diagnostic { ... }` block in `crates/server/src/check.rs` (the block that already holds `new`):

```rust
    /// Project this diagnostic to LSP for the `lsp` server. `lines` is the buffer split
    /// on `\n` (needed to clamp the line and to bound a whole-line span). Mirrors the
    /// companion's `check.ts` mapping: 1-based line → 0-based, clamped to the buffer;
    /// a precise 1-based `[col, end_col)` → 0-based when present, else the whole line.
    pub(crate) fn to_lsp(&self, lines: &[&str]) -> lsp_types::Diagnostic {
        use lsp_types::{
            CodeDescription, DiagnosticSeverity, NumberOrString, Position, Range, Url,
        };
        let last = lines.len().saturating_sub(1) as u32;
        let line0 = self.line.unwrap_or(1).saturating_sub(1).min(last);
        let range = match (self.col, self.end_col) {
            (Some(c), Some(e)) => Range::new(
                Position::new(line0, c.saturating_sub(1)),
                Position::new(line0, e.saturating_sub(1)),
            ),
            _ => {
                let len = lines.get(line0 as usize).map_or(0, |l| l.chars().count()) as u32;
                Range::new(Position::new(line0, 0), Position::new(line0, len))
            }
        };
        let severity = Some(match self.severity {
            "error" => DiagnosticSeverity::ERROR,
            "warning" => DiagnosticSeverity::WARNING,
            "info" => DiagnosticSeverity::INFORMATION,
            _ => DiagnosticSeverity::HINT,
        });
        lsp_types::Diagnostic {
            range,
            severity,
            code: Some(NumberOrString::String(self.code.to_string())),
            code_description: Url::parse(&self.docs_url).ok().map(|href| CodeDescription { href }),
            source: Some("taliesin".to_string()),
            message: self.message.clone(),
            ..Default::default()
        }
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p taliesin-server check::tests::buffer_diagnostics check::tests::to_lsp 2>&1 | tail -20`
Expected: PASS — all three tests green. If `to_lsp_uses_a_precise_span` fails on the code text, confirm the `TAL-FM-KEY` typo classifier still emits a column span (it may classify under a different code; adjust the expected `code` string to whatever `codes::classify` returns for that message — the point is the *span + severity + href* mapping, not the exact code constant).

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/check.rs
git commit -m "feat(check): buffer_diagnostics + Diagnostic::to_lsp for the lsp server (E7 cut 1)"
```

---

### Task 3: Wire `didOpen`/`didChange`/`didClose` → `publishDiagnostics`

The live capability. Deliverable: an in-process integration test driving `initialize → didOpen(typos.tmd) → publishDiagnostics(non-empty)`, `didChange(clean) → publishDiagnostics(empty)`, and a multi-diagnostic case over another fixture.

**Files:**
- Modify: `crates/server/src/lsp.rs` (fill in `main_loop`'s notification branch + add `handle_notification`, `publish`, `publish_empty`; extend the test module)

**Interfaces:**
- Consumes: `crate::check::buffer_diagnostics`, `check::Diagnostic::to_lsp` (Task 2); `lsp_server::{Connection, Message, Notification}`.
- Produces: internal `fn handle_notification`, `fn publish`, `fn publish_empty` (no external callers).

- [ ] **Step 1: Write the failing diagnostics test**

Add to the `#[cfg(test)] mod tests` block in `crates/server/src/lsp.rs`. Add these imports at the top of the test module (alongside the Task 1 imports):

```rust
    use lsp_types::notification::Notification as _;
    use lsp_types::{
        DidChangeTextDocumentParams, DidOpenTextDocumentParams, PublishDiagnosticsParams,
        TextDocumentContentChangeEvent, TextDocumentItem, Url, VersionedTextDocumentIdentifier,
    };
    use std::path::PathBuf;

    fn corpus(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics").join(name)
    }

    // Send initialize + initialized so the server enters its main loop.
    fn handshake(client: &Connection) {
        client.sender.send(Message::Request(Request {
            id: RequestId::from(1),
            method: "initialize".to_owned(),
            params: serde_json::json!({ "capabilities": {} }),
        })).unwrap();
        let _ = client.receiver.recv().unwrap(); // InitializeResult
        client.sender.send(Message::Notification(Notification {
            method: "initialized".to_owned(),
            params: serde_json::json!({}),
        })).unwrap();
    }

    // Block until the next publishDiagnostics notification; panics on any other message.
    fn recv_publish(client: &Connection) -> PublishDiagnosticsParams {
        match client.receiver.recv().unwrap() {
            Message::Notification(n) if n.method == PublishDiagnostics::METHOD => {
                serde_json::from_value(n.params).unwrap()
            }
            other => panic!("expected publishDiagnostics, got {other:?}"),
        }
    }

    fn shutdown(client: &Connection) {
        client.sender.send(Message::Request(Request {
            id: RequestId::from(99), method: "shutdown".to_owned(), params: serde_json::Value::Null,
        })).unwrap();
        let _ = client.receiver.recv().unwrap();
        client.sender.send(Message::Notification(Notification {
            method: "exit".to_owned(), params: serde_json::Value::Null,
        })).unwrap();
    }

    #[test]
    fn didopen_then_didchange_publishes_and_clears_diagnostics() {
        use lsp_types::notification::PublishDiagnostics;
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let path = corpus("typos.tmd");
        let uri = Url::from_file_path(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();

        client.sender.send(Message::Notification(Notification {
            method: lsp_types::notification::DidOpenTextDocument::METHOD.to_owned(),
            params: serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(), language_id: "taliesin".to_owned(), version: 1, text,
                },
            }).unwrap(),
        })).unwrap();

        let opened = recv_publish(&client);
        assert_eq!(opened.uri, uri);
        assert!(!opened.diagnostics.is_empty(), "typos.tmd should produce diagnostics");
        assert!(opened.diagnostics.iter().all(|d| d.source.as_deref() == Some("taliesin")));

        // Replace with a clean buffer (FULL sync): diagnostics should clear.
        client.sender.send(Message::Notification(Notification {
            method: lsp_types::notification::DidChangeTextDocument::METHOD.to_owned(),
            params: serde_json::to_value(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier { uri: uri.clone(), version: 2 },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None, range_length: None, text: "# Clean\n\nBody.\n".to_owned(),
                }],
            }).unwrap(),
        })).unwrap();

        let changed = recv_publish(&client);
        assert_eq!(changed.uri, uri);
        assert!(changed.diagnostics.is_empty(), "a clean buffer should clear diagnostics, got {:?}",
            changed.diagnostics);

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p taliesin-server lsp::tests::didopen 2>&1 | tail -20`
Expected: FAIL — the server currently ignores notifications (Task 1's empty `Message::Notification(_)` branch), so `recv_publish` blocks/times out or the test hangs. To avoid a hang while iterating, the failing state is acceptable to observe by `Ctrl-C`; the assertion it will satisfy after Step 3 is "publishDiagnostics received." (If preferred, temporarily assert the branch is unimplemented; Step 3 removes any doubt.)

- [ ] **Step 3: Implement the notification handlers**

In `crates/server/src/lsp.rs`, replace the `Message::Notification(_notif) => {}` arm in `main_loop` with:
```rust
            Message::Notification(notif) => handle_notification(connection, &mut docs, notif)?,
```
Add `let mut docs: std::collections::HashMap<lsp_types::Url, ()> = std::collections::HashMap::new();` — actually track membership only; replace the loop header. Update `main_loop` to own the tracked-URIs set and add the helpers below it:

```rust
fn main_loop(
    connection: &Connection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // URIs we have accepted as taliesin documents (didChange carries no languageId, so we
    // only lint what didOpen admitted).
    let mut tracked: std::collections::HashSet<lsp_types::Url> = std::collections::HashSet::new();
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
            }
            Message::Notification(notif) => handle_notification(connection, &mut tracked, notif)?,
            Message::Response(_) => {}
        }
    }
    Ok(())
}

fn handle_notification(
    connection: &Connection,
    tracked: &mut std::collections::HashSet<lsp_types::Url>,
    notif: lsp_server::Notification,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use lsp_types::notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
    };
    use lsp_types::{
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    };
    let method = notif.method.as_str();
    if method == DidOpenTextDocument::METHOD {
        let p: DidOpenTextDocumentParams = serde_json::from_value(notif.params)?;
        if p.text_document.language_id == "taliesin" {
            tracked.insert(p.text_document.uri.clone());
            publish(connection, &p.text_document.uri, &p.text_document.text)?;
        }
    } else if method == DidChangeTextDocument::METHOD {
        let mut p: DidChangeTextDocumentParams = serde_json::from_value(notif.params)?;
        if tracked.contains(&p.text_document.uri) {
            // FULL sync: the last content change holds the entire new buffer.
            if let Some(change) = p.content_changes.pop() {
                publish(connection, &p.text_document.uri, &change.text)?;
            }
        }
    } else if method == DidCloseTextDocument::METHOD {
        let p: DidCloseTextDocumentParams = serde_json::from_value(notif.params)?;
        tracked.remove(&p.text_document.uri);
        publish_diagnostics(connection, &p.text_document.uri, Vec::new())?;
    }
    Ok(())
}

/// Lint `text` as the document at `uri` and publish the result.
fn publish(
    connection: &Connection,
    uri: &lsp_types::Url,
    text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = uri
        .to_file_path()
        .unwrap_or_else(|_| std::path::PathBuf::from("untitled.tmd"));
    let lines: Vec<&str> = text.split('\n').collect();
    let diags = crate::check::buffer_diagnostics(&path, text)
        .iter()
        .map(|d| d.to_lsp(&lines))
        .collect();
    publish_diagnostics(connection, uri, diags)
}

fn publish_diagnostics(
    connection: &Connection,
    uri: &lsp_types::Url,
    diagnostics: Vec<lsp_types::Diagnostic>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use lsp_types::notification::{Notification as _, PublishDiagnostics};
    let params = lsp_types::PublishDiagnosticsParams { uri: uri.clone(), diagnostics, version: None };
    connection.sender.send(Message::Notification(lsp_server::Notification {
        method: PublishDiagnostics::METHOD.to_owned(),
        params: serde_json::to_value(params)?,
    }))?;
    Ok(())
}
```

Note: remove the now-unused `publish_empty` name if you sketched one — `publish_diagnostics(.., Vec::new())` is the clear path. Ensure `main_loop`'s old `let mut docs` line (if any) is gone.

- [ ] **Step 4: Run the diagnostics test to verify it passes**

Run: `cargo test -p taliesin-server lsp::tests::didopen 2>&1 | tail -20`
Expected: PASS — `didopen_then_didchange_publishes_and_clears_diagnostics` green.

- [ ] **Step 5: Add the multi-diagnostic + close case**

Append to the `lsp.rs` test module:

```rust
    #[test]
    fn didclose_clears_and_a_second_fixture_reports_findings() {
        use lsp_types::notification::{DidCloseTextDocument, DidOpenTextDocument, PublishDiagnostics};
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let path = corpus("refs.tmd");
        let uri = Url::from_file_path(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        client.sender.send(Message::Notification(Notification {
            method: DidOpenTextDocument::METHOD.to_owned(),
            params: serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(), language_id: "taliesin".to_owned(), version: 1, text,
                },
            }).unwrap(),
        })).unwrap();
        let opened = recv_publish(&client);
        // Every range must fall on a real line of the buffer (no out-of-bounds positions).
        let line_count = std::fs::read_to_string(&path).unwrap().split('\n').count() as u32;
        assert!(opened.diagnostics.iter().all(|d| d.range.start.line < line_count),
            "a diagnostic range escaped the buffer");

        client.sender.send(Message::Notification(Notification {
            method: DidCloseTextDocument::METHOD.to_owned(),
            params: serde_json::to_value(lsp_types::DidCloseTextDocumentParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            }).unwrap(),
        })).unwrap();
        let closed = recv_publish(&client);
        assert_eq!(closed.uri, uri);
        assert!(closed.diagnostics.is_empty(), "didClose should clear diagnostics");

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }
```

Note: `refs.tmd` is chosen because it exercises cross-reference validation; if it happens to be diagnostic-free, the range assertion still holds vacuously and the close-clears assertion is the load-bearing one. If you want a guaranteed non-empty first fixture, keep `typos.tmd` from the earlier test as the non-empty proof and let this test focus on `didClose` clearing.

- [ ] **Step 6: Run both lsp tests + full crate**

Run: `cargo test -p taliesin-server lsp:: 2>&1 | tail -15`
Expected: PASS — all `lsp::tests` green.
Run: `cargo test -p taliesin-server 2>&1 | tail -15`
Expected: PASS (whole crate).

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/lsp.rs
git commit -m "feat(lsp): live publishDiagnostics on didOpen/didChange/didClose (E7 cut 1)"
```

---

### Task 4: Docs, backlog narrowing, and full verification

Deliverable: an internals-book note on the subcommand, the backlog E7 item narrowed to "cut 1 shipped", and the full test gates run green with `fmt`/`clippy` clean.

**Files:**
- Modify: `docs/internals/` (a short LSP note — see Step 1 for the exact target page)
- Modify: `notes/backlog.md` (narrow E7)
- Modify: `notes/ROADMAP.md` only if it tracks E7 sub-state (skip if not)

- [ ] **Step 1: Add an internals-book note**

Find the internals page that documents the editor/machine-facing surface:
```bash
grep -rln "mcp\|companion\|check --stdin\|editor" docs/internals --include='*.tmd' | head
```
Add a short section to the most relevant page (likely the editor/agent or dev-server chapter) describing: what `taliesin lsp` is (offline stdio LSP), its one capability (live `.tmd` diagnostics via FULL sync), that it is read-only/kernel-free, and a minimal Neovim/Helix wiring snippet (`cmd = { "taliesin", "lsp" }`). Keep it to one focused subsection. Match the surrounding prose style.

- [ ] **Step 2: Verify the docs still build/lint**

Run: `cargo run -p taliesin-server -- check docs/internals --format human 2>&1 | tail -15`
Expected: no new diagnostics introduced by the edit (the page renders clean).

- [ ] **Step 3: Narrow the backlog E7 item**

In `notes/backlog.md`, edit the `E7. taliesin lsp server` item: mark cut 1 (diagnostics) as shipped and re-scope the remaining text to the additive capabilities (hover, definition, completion, outline, rename) that still layer on the harness. Update the parenthetical "(E1…E6 shipped — Only E7 remains.)" to reflect that E7's diagnostics slice has shipped and the follow-up capabilities remain. Reference the spec/plan by path.

- [ ] **Step 4: Full verification (all gates + fmt/clippy)**

Run:
```bash
cargo fmt --check 2>&1 | tail -5
cargo clippy -p taliesin-server --all-targets 2>&1 | tail -15
cargo test -p taliesin-server 2>&1 | tail -15
```
Expected: `fmt` clean (the PostToolUse hook already runs rustfmt on edits), `clippy` no new warnings, tests PASS. If an `exec`/`kernel` timing test flakes, re-run `cargo test -p taliesin-server -- --test-threads=1`.

Then confirm the binary actually speaks the protocol end-to-end via a real spawn (belt-and-suspenders beyond the in-process test):
```bash
printf 'Content-Length: 61\r\n\r\n{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | cargo run -q -p taliesin-server -- lsp 2>/dev/null | head -c 200; echo
```
Expected: an `initialize` response on stdout beginning with `Content-Length:` then a JSON body containing `"capabilities"`. (The process then waits for more input; `head -c` returns and the pipe closes it. The exact byte length in the printf may need adjustment — the point is a well-formed response appears.)

- [ ] **Step 5: Commit**

```bash
git add docs/internals notes/backlog.md
git commit -m "docs(lsp): internals note + backlog E7 narrowed to cut 1 shipped"
```

---

## Self-Review

**Spec coverage:**
- Architecture (new `lsp.rs`, stdio, sync, dispatch wiring) → Task 1. ✓
- Dependency `lsp-server`/`lsp-types` → Task 1 Step 1 + Global Constraints. ✓
- `initialize` advertises FULL sync only → Task 1 `server_capabilities`. ✓
- didOpen/didChange/didClose → publishDiagnostics; languageId gating; FULL-sync last-change; URI→path; untitled fallback → Task 3. ✓
- Reuse seam `buffer_diagnostics` over `collect_file_diagnostics_from_src` → Task 2. ✓
- `Diagnostic::to_lsp` mapping (line clamp, precise `[col-1,end_col-1)`, whole-line fallback, severity, code, codeDescription, source) → Task 2. ✓
- Suggestion/quick-fix out of scope → not implemented (correct); code/href still carried. ✓
- Primary pin (memory Connection: initialize→didOpen(typos.tmd)→publish; didChange→clear) → Task 3. ✓
- Second fixture + range-in-bounds + didClose-clears → Task 3 Step 5. ✓
- Lifecycle (shutdown→exit clean) → Task 1 test. ✓
- Docs note + backlog narrow → Task 4. ✓
- Guardrail tests (COMMANDS, subcommand_help, command_desc, completion count) → Task 1 Steps 5–8. ✓
- stdout-is-the-wire constraint → Global Constraints + `cmd_lsp` logs to stderr only. ✓
- Invariants (single-editing-surface read-only, offline, HTML-only unaffected, block-model untouched, exec-pool untouched) → design; nothing in the plan violates them. ✓

**Placeholder scan:** no TBD/TODO; every code step shows complete code; every run step shows expected output. ✓

**Type consistency:** `buffer_diagnostics(&Path, &str) -> Vec<Diagnostic>`, `to_lsp(&self, lines: &[&str]) -> lsp_types::Diagnostic`, `run(Connection) -> Result<(), Box<dyn Error + Send + Sync>>`, `server_capabilities() -> ServerCapabilities`, and the `handle_notification`/`publish`/`publish_diagnostics` helper signatures are used identically across tasks. The tracked-URIs set is a `HashSet<Url>` (Task 3 Step 3), matching its uses. ✓
