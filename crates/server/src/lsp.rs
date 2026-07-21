//! The `taliesin lsp` subcommand: a synchronous, offline LSP server over stdio.
//!
//! **What:** holds open `.tmd` buffers warm and publishes live diagnostics (the
//! `check` validators) on every edit, to any LSP editor. Parse-only, kernel-free.
//!
//! **stdout is the JSON-RPC wire** — this module must never write to stdout; all
//! human output goes to `crate::log` (stderr). See the plan's Global Constraints.

use lsp_server::{Connection, Message};
use lsp_types::{
    OneOf, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions,
};
use std::process::ExitCode;

/// Advertised capabilities: full-text document sync (whole buffer on every change, which
/// maps 1:1 onto `check`'s whole-buffer linting), go-to-definition, and document symbols
/// (the heading outline). Hover / completion land as later follow-ups on this same server.
pub(crate) fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..Default::default()
            },
        )),
        definition_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
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
pub(crate) fn run(connection: Connection) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let caps = serde_json::to_value(server_capabilities())?;
    let _initialize_params = connection.initialize(caps)?;
    main_loop(&connection)?;
    Ok(())
}

/// Read messages until `shutdown`/`exit`. Text-document notifications keep the open-buffer
/// store current and drive diagnostics; requests (other than shutdown) are answered from
/// that store.
fn main_loop(connection: &Connection) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Open taliesin documents, by URI → current buffer text. `didChange` carries no
    // languageId, so we only track what `didOpen` admitted; a request between edits reads
    // the buffer text from here.
    let mut docs: std::collections::HashMap<lsp_types::Url, String> =
        std::collections::HashMap::new();
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                handle_request(connection, &docs, req)?;
            }
            Message::Notification(notif) => handle_notification(connection, &mut docs, notif)?,
            Message::Response(_) => {}
        }
    }
    Ok(())
}

/// Dispatch a text-document notification: keep the open-buffer store current and
/// (re)publish diagnostics for the affected buffer.
fn handle_notification(
    connection: &Connection,
    docs: &mut std::collections::HashMap<lsp_types::Url, String>,
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
            let uri = p.text_document.uri;
            docs.insert(uri.clone(), p.text_document.text);
            publish(connection, &uri, &docs[&uri])?;
        }
    } else if method == DidChangeTextDocument::METHOD {
        let mut p: DidChangeTextDocumentParams = serde_json::from_value(notif.params)?;
        let uri = p.text_document.uri;
        // FULL sync: the last content change holds the entire new buffer. Only act on a
        // document `didOpen` admitted.
        if docs.contains_key(&uri)
            && let Some(change) = p.content_changes.pop()
        {
            docs.insert(uri.clone(), change.text);
            publish(connection, &uri, &docs[&uri])?;
        }
    } else if method == DidCloseTextDocument::METHOD {
        let p: DidCloseTextDocumentParams = serde_json::from_value(notif.params)?;
        docs.remove(&p.text_document.uri);
        publish_diagnostics(connection, &p.text_document.uri, Vec::new())?;
    }
    Ok(())
}

/// Answer a request from the open-buffer store. Only `textDocument/definition` is handled;
/// any other request gets a `MethodNotFound` reply so the client never hangs waiting.
fn handle_request(
    connection: &Connection,
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    req: lsp_server::Request,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use lsp_types::request::{DocumentSymbolRequest, GotoDefinition, Request as _};
    let response = if req.method == GotoDefinition::METHOD {
        let params: lsp_types::GotoDefinitionParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(resolve_definition(docs, &params))?),
            error: None,
        }
    } else if req.method == DocumentSymbolRequest::METHOD {
        let params: lsp_types::DocumentSymbolParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(document_symbols(
                docs,
                &params.text_document.uri,
            ))?),
            error: None,
        }
    } else {
        lsp_server::Response {
            id: req.id,
            result: None,
            error: Some(lsp_server::ResponseError {
                // JSON-RPC MethodNotFound.
                code: -32601,
                message: format!("unhandled request: {}", req.method),
                data: None,
            }),
        }
    };
    connection.sender.send(Message::Response(response))?;
    Ok(())
}

/// Resolve go-to-definition for the token under the cursor, or `None` when it points
/// nowhere resolvable (an undefined xref, a cross-file ref, a missing include/bib).
fn resolve_definition(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    params: &lsp_types::GotoDefinitionParams,
) -> Option<lsp_types::GotoDefinitionResponse> {
    use crate::lsp_nav::Target;
    use lsp_types::{Location, Position, Range, Url};

    let uri = &params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    let text = docs.get(uri)?;
    let point = |line: u32, col: u32, end: u32| {
        Range::new(Position::new(line, col), Position::new(line, end))
    };

    let location =
        match crate::lsp_nav::classify_target(text, pos.line as usize, pos.character as usize) {
            // `{{< include x.tmd >}}` → the file (position 0:0), when it exists on disk.
            Target::Include { path, .. } => {
                let dir = uri.to_file_path().ok()?;
                let abs = dir.parent()?.join(&path);
                if !abs.exists() {
                    return None;
                }
                Location::new(Url::from_file_path(&abs).ok()?, point(0, 0, 0))
            }
            // `@fig-x` → its definition in this document (cross-file refs get nothing).
            Target::Xref { id, .. } => {
                let (line, col) = crate::lsp_nav::definition_site(text, &id)?;
                Location::new(
                    uri.clone(),
                    point(line, col, col + id.chars().count() as u32),
                )
            }
            // `[@key]` → the BibTeX entry in the first front-matter `.bib` that defines it.
            Target::Cite { key, .. } => {
                let dir = uri.to_file_path().ok()?;
                let dir = dir.parent()?;
                let mut hit = None;
                for rel in crate::lsp_nav::frontmatter_bib_paths(text) {
                    let abs = dir.join(&rel);
                    if let Ok(bib) = std::fs::read_to_string(&abs)
                        && let Some((line, col)) = crate::lsp_nav::bib_entry_site(&bib, &key)
                    {
                        hit = Some(Location::new(
                            Url::from_file_path(&abs).ok()?,
                            point(line, col, col),
                        ));
                        break;
                    }
                }
                hit?
            }
            Target::FrontmatterKey { .. } | Target::None => return None,
        };
    Some(lsp_types::GotoDefinitionResponse::Scalar(location))
}

/// The heading outline for `uri` as nested LSP document symbols, or `None` when the buffer
/// is unknown.
fn document_symbols(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    uri: &lsp_types::Url,
) -> Option<lsp_types::DocumentSymbolResponse> {
    let text = docs.get(uri)?;
    let lines: Vec<&str> = text.split('\n').collect();
    let symbols = crate::lsp_outline::outline(text)
        .iter()
        .map(|n| to_document_symbol(n, &lines))
        .collect();
    Some(lsp_types::DocumentSymbolResponse::Nested(symbols))
}

/// Convert one outline node (and its children) to an LSP `DocumentSymbol`. `range` spans the
/// whole section; `selection_range` is the heading line (contained in `range`). Mirrors the
/// companion's `outline-provider.ts` mapping.
fn to_document_symbol(
    node: &crate::lsp_outline::OutlineNode,
    lines: &[&str],
) -> lsp_types::DocumentSymbol {
    use lsp_types::{Position, Range, SymbolKind};
    let last = lines.len().saturating_sub(1);
    let start = node.start_line.min(last);
    let end = node.end_line.max(node.start_line).min(last);
    let line_len = |i: usize| lines.get(i).map_or(0, |l| l.chars().count()) as u32;
    let name = if node.title.is_empty() {
        "(untitled)".to_string()
    } else {
        node.title.clone()
    };
    #[allow(deprecated)] // `deprecated` is a required (deprecated) field of DocumentSymbol.
    lsp_types::DocumentSymbol {
        name,
        detail: None,
        kind: SymbolKind::STRING,
        tags: None,
        deprecated: None,
        range: Range::new(
            Position::new(start as u32, 0),
            Position::new(end as u32, line_len(end)),
        ),
        selection_range: Range::new(
            Position::new(start as u32, 0),
            Position::new(start as u32, line_len(start)),
        ),
        children: Some(
            node.children
                .iter()
                .map(|c| to_document_symbol(c, lines))
                .collect(),
        ),
    }
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
    let diagnostics = crate::check::buffer_diagnostics(&path, text)
        .iter()
        .map(|d| d.to_lsp(&lines))
        .collect();
    publish_diagnostics(connection, uri, diagnostics)
}

/// Send a `textDocument/publishDiagnostics` notification (an empty vec clears squiggles).
fn publish_diagnostics(
    connection: &Connection,
    uri: &lsp_types::Url,
    diagnostics: Vec<lsp_types::Diagnostic>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use lsp_types::notification::{Notification as _, PublishDiagnostics};
    let params = lsp_types::PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics,
        version: None,
    };
    connection
        .sender
        .send(Message::Notification(lsp_server::Notification {
            method: PublishDiagnostics::METHOD.to_owned(),
            params: serde_json::to_value(params)?,
        }))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
    use lsp_types::notification::Notification as _;
    use lsp_types::notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, PublishDiagnostics,
    };
    use lsp_types::request::Request as _;
    use lsp_types::{
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        PublishDiagnosticsParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
        TextDocumentItem, Url, VersionedTextDocumentIdentifier,
    };
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    // Canonicalized so the path carries no `..` segments: a URI built from it round-trips
    // through the server's JSON parse (which does RFC 3986 dot-segment removal) unchanged,
    // matching what a real editor sends. Without this the echoed URI would differ only by
    // normalization.
    fn corpus(name: &str) -> PathBuf {
        std::fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus/diagnostics")
                .join(name),
        )
        .unwrap()
    }

    // Send initialize + initialized so the server enters its main loop.
    fn handshake(client: &Connection) {
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(1),
                method: "initialize".to_owned(),
                params: serde_json::json!({ "capabilities": {} }),
            }))
            .unwrap();
        let _ = client.receiver.recv().unwrap(); // InitializeResult
        client
            .sender
            .send(Message::Notification(Notification {
                method: "initialized".to_owned(),
                params: serde_json::json!({}),
            }))
            .unwrap();
    }

    // Block (bounded) until the next publishDiagnostics notification; panics on any other
    // message or on timeout, so a server that never publishes fails fast instead of hanging.
    fn recv_publish(client: &Connection) -> PublishDiagnosticsParams {
        match client.receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(Message::Notification(n)) if n.method == PublishDiagnostics::METHOD => {
                serde_json::from_value(n.params).unwrap()
            }
            other => panic!("expected publishDiagnostics, got {other:?}"),
        }
    }

    fn shutdown(client: &Connection) {
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(99),
                method: "shutdown".to_owned(),
                params: serde_json::Value::Null,
            }))
            .unwrap();
        let _ = client.receiver.recv().unwrap();
        client
            .sender
            .send(Message::Notification(Notification {
                method: "exit".to_owned(),
                params: serde_json::Value::Null,
            }))
            .unwrap();
    }

    fn did_open(client: &Connection, uri: &Url, text: String) {
        client
            .sender
            .send(Message::Notification(Notification {
                method: DidOpenTextDocument::METHOD.to_owned(),
                params: serde_json::to_value(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri.clone(),
                        language_id: "taliesin".to_owned(),
                        version: 1,
                        text,
                    },
                })
                .unwrap(),
            }))
            .unwrap();
    }

    fn goto_params(uri: &Url, line: u32, character: u32) -> lsp_types::GotoDefinitionParams {
        lsp_types::GotoDefinitionParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: lsp_types::Position::new(line, character),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }
    }

    // Block (bounded) for the response with `id`, skipping any stray publishDiagnostics
    // notifications that arrive first.
    fn recv_response(client: &Connection, id: RequestId) -> Response {
        loop {
            match client.receiver.recv_timeout(Duration::from_secs(10)) {
                Ok(Message::Response(r)) if r.id == id => return r,
                Ok(Message::Notification(_)) => continue,
                other => panic!("expected response {id:?}, got {other:?}"),
            }
        }
    }

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

        server_thread
            .join()
            .unwrap()
            .expect("server loop should exit Ok");
    }

    #[test]
    fn didopen_then_didchange_publishes_and_clears_diagnostics() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let path = corpus("typos.tmd");
        let uri = Url::from_file_path(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        did_open(&client, &uri, text);

        let opened = recv_publish(&client);
        assert_eq!(opened.uri, uri);
        assert!(
            !opened.diagnostics.is_empty(),
            "typos.tmd should produce diagnostics"
        );
        assert!(
            opened
                .diagnostics
                .iter()
                .all(|d| d.source.as_deref() == Some("taliesin"))
        );

        // Replace with a clean buffer (FULL sync): diagnostics should clear.
        client
            .sender
            .send(Message::Notification(Notification {
                method: DidChangeTextDocument::METHOD.to_owned(),
                params: serde_json::to_value(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: uri.clone(),
                        version: 2,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: "# Clean\n\nBody.\n".to_owned(),
                    }],
                })
                .unwrap(),
            }))
            .unwrap();

        let changed = recv_publish(&client);
        assert_eq!(changed.uri, uri);
        assert!(
            changed.diagnostics.is_empty(),
            "a clean buffer should clear diagnostics, got {:?}",
            changed.diagnostics
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    #[test]
    fn didclose_clears_and_a_fixture_stays_in_bounds() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let path = corpus("refs.tmd");
        let uri = Url::from_file_path(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let line_count = text.split('\n').count() as u32;
        did_open(&client, &uri, text);

        let opened = recv_publish(&client);
        // Every range must fall on a real line of the buffer (no out-of-bounds positions).
        assert!(
            opened
                .diagnostics
                .iter()
                .all(|d| d.range.start.line < line_count && d.range.end.line < line_count),
            "a diagnostic range escaped the buffer"
        );

        client
            .sender
            .send(Message::Notification(Notification {
                method: DidCloseTextDocument::METHOD.to_owned(),
                params: serde_json::to_value(DidCloseTextDocumentParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                })
                .unwrap(),
            }))
            .unwrap();
        let closed = recv_publish(&client);
        assert_eq!(closed.uri, uri);
        assert!(
            closed.diagnostics.is_empty(),
            "didClose should clear diagnostics"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    #[test]
    fn goto_definition_resolves_a_same_doc_xref() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-def.tmd").unwrap();
        let text = "# Title {#fig-1}\n\nSee @fig-1 for details.\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client); // didOpen diagnostics

        // Definition at the `@fig-1` reference (line 2, char 5) → the `{#fig-1}` on line 0.
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(5),
                method: lsp_types::request::GotoDefinition::METHOD.to_owned(),
                params: serde_json::to_value(goto_params(&uri, 2, 5)).unwrap(),
            }))
            .unwrap();
        let resp = recv_response(&client, RequestId::from(5));
        let result: lsp_types::GotoDefinitionResponse =
            serde_json::from_value(resp.result.expect("a definition result")).unwrap();
        match result {
            lsp_types::GotoDefinitionResponse::Scalar(loc) => {
                assert_eq!(loc.uri, uri);
                assert_eq!(loc.range.start, lsp_types::Position::new(0, 10));
            }
            other => panic!("expected a scalar location, got {other:?}"),
        }

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    #[test]
    fn goto_definition_resolves_a_citation_into_the_bib() {
        // Write a doc + its .bib to a temp dir so the server can resolve across files.
        let dir = std::env::temp_dir().join(format!("tali-lsp-cite-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("refs.bib"),
            "@article{smith2020,\n  title = {A Study}\n}\n",
        )
        .unwrap();
        let doc = dir.join("paper.tmd");
        let text =
            "---\nbibliography: refs.bib\n---\n\nAs shown in [@smith2020], the result holds.\n"
                .to_string();
        std::fs::write(&doc, &text).unwrap();

        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);
        let uri = Url::from_file_path(&doc).unwrap();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // `[@smith2020]` sits on line 4; the key starts at char 14, so char 15 is inside it.
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(7),
                method: lsp_types::request::GotoDefinition::METHOD.to_owned(),
                params: serde_json::to_value(goto_params(&uri, 4, 15)).unwrap(),
            }))
            .unwrap();
        let resp = recv_response(&client, RequestId::from(7));
        let result: lsp_types::GotoDefinitionResponse =
            serde_json::from_value(resp.result.expect("a definition result")).unwrap();
        match result {
            lsp_types::GotoDefinitionResponse::Scalar(loc) => {
                assert_eq!(loc.uri, Url::from_file_path(dir.join("refs.bib")).unwrap());
                assert_eq!(loc.range.start, lsp_types::Position::new(0, 0));
            }
            other => panic!("expected a scalar location into the .bib, got {other:?}"),
        }

        shutdown(&client);
        thread.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn document_symbol_returns_the_nested_heading_tree() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-outline.tmd").unwrap();
        let text = "# Top\n\ntext\n\n## Sub A\n\n## Sub B\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(9),
                method: lsp_types::request::DocumentSymbolRequest::METHOD.to_owned(),
                params: serde_json::to_value(lsp_types::DocumentSymbolParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                })
                .unwrap(),
            }))
            .unwrap();
        let resp = recv_response(&client, RequestId::from(9));
        // The server emits Nested symbols (a plain array of DocumentSymbol).
        let syms: Vec<lsp_types::DocumentSymbol> =
            serde_json::from_value(resp.result.expect("a documentSymbol result")).unwrap();
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Top");
        assert_eq!(
            syms[0].selection_range.start,
            lsp_types::Position::new(0, 0)
        );
        let kids = syms[0].children.as_ref().expect("Top has children");
        assert_eq!(
            kids.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            vec!["Sub A", "Sub B"]
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }
}
