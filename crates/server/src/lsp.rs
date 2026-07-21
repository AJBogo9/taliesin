//! The `taliesin lsp` subcommand: a synchronous, offline LSP server over stdio.
//!
//! **What:** holds open `.tmd` buffers warm and publishes live diagnostics (the
//! `check` validators) on every edit, to any LSP editor. Parse-only, kernel-free.
//!
//! **stdout is the JSON-RPC wire** — this module must never write to stdout; all
//! human output goes to `crate::log` (stderr). See the plan's Global Constraints.

use lsp_server::{Connection, Message};
use lsp_types::{
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
};
use std::process::ExitCode;

/// The one advertised capability: full-text document sync (whole buffer on every
/// change), which maps 1:1 onto `check`'s whole-buffer linting. No provider
/// capabilities yet (hover/completion/definition land as follow-ups).
pub(crate) fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..Default::default()
            },
        )),
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

/// Read messages until `shutdown`/`exit`. Requests other than shutdown are ignored
/// (no request capabilities advertised); text-document notifications drive diagnostics.
fn main_loop(connection: &Connection) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // URIs we have accepted as taliesin documents. `didChange` carries no languageId, so
    // we only lint what `didOpen` admitted.
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

/// Dispatch a text-document notification: keep the tracked set current and (re)publish
/// diagnostics for the affected buffer.
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
}
