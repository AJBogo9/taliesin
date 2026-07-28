//! The `taliesin lsp` subcommand: a synchronous, offline LSP server over stdio.
//!
//! **What:** holds open `.tmd` buffers warm and publishes live diagnostics (the
//! `check` validators) on every edit, to any LSP editor. Parse-only, kernel-free.
//!
//! **stdout is the JSON-RPC wire** — this module must never write to stdout; all
//! human output goes to `crate::log` (stderr). See the plan's Global Constraints.

use lsp_server::{Connection, Message};
use lsp_types::{
    CodeActionProviderCapability, CompletionOptions, HoverProviderCapability, OneOf,
    PositionEncodingKind, RenameOptions, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions,
};
use std::process::ExitCode;

/// Advertised capabilities: full-text document sync (whole buffer on every change, which maps
/// 1:1 onto `check`'s whole-buffer linting), go-to-definition, document symbols (the heading
/// outline), hover, completion, quick-fix code actions, and rename (with prepare) of a
/// cross-reference anchor + its references. This is the full E7 intelligence surface.
pub(crate) fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        // Columns on the wire are UTF-16 code units (the LSP default, and what the VS Code
        // companion uses). We work in Unicode scalars internally and convert at the boundary
        // (`lsp_pos`); advertise the encoding explicitly so the contract is not implicit.
        position_encoding: Some(PositionEncodingKind::UTF16),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..Default::default()
            },
        )),
        definition_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        // `{{< include >}}` / `{{< embed >}}` paths. Go-to-definition already resolved
        // these, but a definition is invisible: nothing on screen says the path is
        // navigable, so it is only found by an author who already guessed. A document link
        // is the affordance editors paint for exactly this.
        document_link_provider: Some(lsp_types::DocumentLinkOptions {
            // Every link is resolved in one pass (the target is a plain file path), so
            // there is nothing for a second `documentLink/resolve` round trip to add.
            resolve_provider: Some(false),
            work_done_progress_options: Default::default(),
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        completion_provider: Some(CompletionOptions {
            // The chars that open a completable context: `@` (xref/cite), `.` (div class),
            // `|` (cell option), `-` (xref prefix), `/` (path), `:` (front-matter value),
            // `\` (a math command, inside `$…$` only).
            trigger_characters: Some(
                ["@", ".", "|", "-", "/", ":", "\\"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            ..Default::default()
        }),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            // `prepareRename` gates renaming to a cross-reference anchor, so the editor shows
            // "cannot rename here" on anything else.
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
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
                // One bad request must not take the session down with it: unhandled, it leaves
                // this loop and every later request from the editor goes unanswered, silently.
                // A request fails two ways and BOTH must end here, in an error reply:
                //
                //   * it panics — the residual panic surface `guarded` was added for;
                //   * its params do not deserialize — an `Err`, which `guarded` never sees.
                //
                // The second is by far the likelier, and it used to be the more damaging:
                // `?` propagated it out of `main_loop` → `run` → `cmd_lsp`, which logged it
                // and returned `ExitCode::FAILURE`, so the process *exited* rather than
                // merely going quiet. Four of eight plausible client message shapes hit it.
                // `id`/`method` are cloned because dispatch consumes `req`.
                //
                // InvalidParams is accurate for every *reachable* error: `handle_request`'s
                // other two error sites are serializing our own response (lsp_types structs,
                // which cannot fail) and the channel send, which only fails once the client
                // is gone — and then the reply below fails too and `?` ends the session, as
                // it should.
                let (id, method) = (req.id.clone(), req.method.clone());
                let failure = match crate::serve::guarded(|| handle_request(connection, &docs, req))
                {
                    Ok(Ok(())) => None,
                    // JSON-RPC InvalidParams.
                    Ok(Err(e)) => {
                        crate::log::error(&format!("lsp: invalid params for {method}: {e}"));
                        Some((-32602, format!("invalid params for {method}")))
                    }
                    // JSON-RPC InternalError.
                    Err(panic) => {
                        crate::log::error(&format!("lsp: panic handling {method}: {panic}"));
                        Some((-32603, format!("internal error handling {method}")))
                    }
                };
                if let Some((code, message)) = failure {
                    connection
                        .sender
                        .send(Message::Response(lsp_server::Response {
                            id,
                            result: None,
                            error: Some(lsp_server::ResponseError {
                                code,
                                message,
                                data: None,
                            }),
                        }))?;
                }
            }
            // Same two failure modes on the higher-traffic half: `publish` →
            // `buffer_diagnostics` renders the buffer on *every keystroke*. A notification has
            // no reply channel, so both are logged and skipped. Swallowing the `Err` is safe
            // for the one non-deserialization case too (a failed `publish` send): the client
            // is then gone, and `connection.receiver` ends this loop on its own.
            Message::Notification(notif) => {
                let method = notif.method.clone();
                match crate::serve::guarded(|| handle_notification(connection, &mut docs, notif)) {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        crate::log::error(&format!("lsp: invalid params for {method}: {e}"))
                    }
                    Err(panic) => {
                        crate::log::error(&format!("lsp: panic handling {method}: {panic}"))
                    }
                }
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
}

/// Test-only method name that panics inside the dispatch. Real input does not panic the
/// renderer (AP2's fuzz round produced zero unexpected panics), so injecting one here is the
/// only way to exercise the loop's panic boundary — the guard exists for the residual
/// panic surface, not for a known repro. `#[cfg(test)]`, so it is absent from the binary.
#[cfg(test)]
pub(crate) const PANIC_PROBE_METHOD: &str = "taliesin/testPanic";

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
    #[cfg(test)]
    assert!(method != PANIC_PROBE_METHOD, "injected notification panic");
    if method == DidOpenTextDocument::METHOD {
        let p: DidOpenTextDocumentParams = serde_json::from_value(notif.params)?;
        // Accept the buffer on EITHER signal: the `taliesin` language id (what the VS
        // Code companion declares) or a `.tmd` path. Gating on the id alone made the
        // server silently inert in every other editor: the documented Neovim recipe is
        // `cmd = { "taliesin", "lsp" }`, Neovim sends the *filetype* as `languageId`,
        // and nothing in this repo registers a filetype for `.tmd` — so the id arrives
        // as "" or "markdown" and every document was dropped. The server still
        // advertised hover/completion/symbols/rename/diagnostics and then answered
        // null to all of them, with nothing on stderr to say why.
        if p.text_document.language_id == "taliesin" || is_tmd_uri(&p.text_document.uri) {
            let uri = p.text_document.uri;
            docs.insert(uri.clone(), p.text_document.text);
            publish(connection, &uri, &docs[&uri])?;
        } else {
            crate::log::warn(&format!(
                "lsp: ignoring {} (languageId {:?} is not `taliesin` and the path is not .tmd)",
                p.text_document.uri, p.text_document.language_id
            ));
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
    use lsp_types::request::{
        CodeActionRequest, Completion, DocumentLinkRequest, DocumentSymbolRequest, GotoDefinition,
        HoverRequest, PrepareRenameRequest, Rename, Request as _,
    };
    #[cfg(test)]
    assert!(req.method != PANIC_PROBE_METHOD, "injected request panic");
    let response = if req.method == GotoDefinition::METHOD {
        let params: lsp_types::GotoDefinitionParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(resolve_definition(docs, &params))?),
            error: None,
        }
    } else if req.method == Completion::METHOD {
        let params: lsp_types::CompletionParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(resolve_completion(docs, &params))?),
            error: None,
        }
    } else if req.method == CodeActionRequest::METHOD {
        let params: lsp_types::CodeActionParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(resolve_code_actions(&params))?),
            error: None,
        }
    } else if req.method == HoverRequest::METHOD {
        let params: lsp_types::HoverParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(resolve_hover(docs, &params))?),
            error: None,
        }
    } else if req.method == DocumentLinkRequest::METHOD {
        let params: lsp_types::DocumentLinkParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(document_links(
                docs,
                &params.text_document.uri,
            ))?),
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
    } else if req.method == PrepareRenameRequest::METHOD {
        let params: lsp_types::TextDocumentPositionParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(resolve_prepare_rename(docs, &params))?),
            error: None,
        }
    } else if req.method == Rename::METHOD {
        let params: lsp_types::RenameParams = serde_json::from_value(req.params)?;
        match resolve_rename(docs, &params) {
            Ok(edit) => lsp_server::Response {
                id: req.id,
                result: Some(serde_json::to_value(edit)?),
                error: None,
            },
            // JSON-RPC RequestFailed: the request was well-formed, the server refused it.
            // The editor surfaces `message` in the rename box, which is where the author is
            // already looking — a null result would read as "nothing to rename here".
            Err(message) => lsp_server::Response {
                id: req.id,
                result: None,
                error: Some(lsp_server::ResponseError {
                    code: -32803,
                    message,
                    data: None,
                }),
            },
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
    // Incoming column is UTF-16; the nav scanners work in scalar offsets. `point` builds an
    // outgoing range from scalar columns on a given line, converting each back to UTF-16.
    let cursor_char = crate::lsp_pos::utf16_to_char(
        crate::lsp_pos::nth_line(text, pos.line as usize),
        pos.character as usize,
    );
    let point = |src: &str, line: u32, col: u32, end: u32| {
        let l = crate::lsp_pos::nth_line(src, line as usize);
        Range::new(
            Position::new(line, crate::lsp_pos::char_to_utf16(l, col as usize) as u32),
            Position::new(line, crate::lsp_pos::char_to_utf16(l, end as usize) as u32),
        )
    };

    let location = match crate::lsp_nav::classify_target(text, pos.line as usize, cursor_char) {
        // `{{< include x.tmd >}}` → the file (position 0:0), when it exists on disk.
        Target::Include { path, .. } => {
            let dir = uri.to_file_path().ok()?;
            let abs = dir.parent()?.join(&path);
            if !abs.exists() {
                return None;
            }
            Location::new(Url::from_file_path(&abs).ok()?, point("", 0, 0, 0))
        }
        // `@fig-x` → its definition in this document (cross-file refs get nothing).
        Target::Xref { id, .. } => {
            let (line, col) = crate::lsp_nav::definition_site(text, &id)?;
            Location::new(
                uri.clone(),
                point(text, line, col, col + id.chars().count() as u32),
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
                        point(&bib, line, col, col),
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

/// Resolve hover for the token under the cursor: an xref's rendered label + number, a
/// front-matter key's documentation, or a citation's BibTeX entry. `None` when the token
/// resolves to nothing (an unknown xref, an undocumented key, a missing/absent `.bib` entry)
/// or is not a hoverable kind (an include path is go-to-definition only, mirroring the
/// companion). Markdown content, ranged to the token so the editor highlights it.
fn resolve_hover(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    params: &lsp_types::HoverParams,
) -> Option<lsp_types::Hover> {
    use crate::lsp_nav::Target;
    use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Range};

    let uri = &params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    let text = docs.get(uri)?;
    // The hovered token is always on the cursor's line, so scalar↔UTF-16 both convert
    // against that one line: UTF-16 in for the lookup, UTF-16 out for the highlight range.
    let cur_line = crate::lsp_pos::nth_line(text, pos.line as usize);
    let cursor_char = crate::lsp_pos::utf16_to_char(cur_line, pos.character as usize);
    let markup = |value: String, start: usize, end: usize| {
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(Range::new(
                Position::new(
                    pos.line,
                    crate::lsp_pos::char_to_utf16(cur_line, start) as u32,
                ),
                Position::new(
                    pos.line,
                    crate::lsp_pos::char_to_utf16(cur_line, end) as u32,
                ),
            )),
        })
    };

    match crate::lsp_nav::classify_target(text, pos.line as usize, cursor_char) {
        // `@fig-2` → the rendered label + number ("Figure 2"). The label lookup gates it: an
        // anchor whose prefix names no cross-reference kind gets no hover.
        Target::Xref { id, start, end } => {
            let number = xref_number(uri, text, &id)?;
            let label = xref_label(&id)?;
            markup(format!("**{label} {number}** — `@{id}`"), start, end)
        }
        // A front-matter key → its one-line docs, scoped to a nested parent when there is one.
        Target::FrontmatterKey {
            key,
            parent,
            start,
            end,
        } => {
            let description = frontmatter_key_doc(parent.as_deref(), &key)?;
            let scope = match &parent {
                Some(p) => format!(" (under `{p}:`)"),
                None => String::new(),
            };
            markup(format!("`{key}:`{scope}\n\n{description}"), start, end)
        }
        // `[@key]` → the brace-balanced BibTeX entry from the first front-matter `.bib`.
        Target::Cite { key, start, end } => {
            let dir = uri.to_file_path().ok()?;
            let dir = dir.parent()?;
            for rel in crate::lsp_nav::frontmatter_bib_paths(text) {
                if let Ok(bib) = std::fs::read_to_string(dir.join(&rel))
                    && let Some(entry) = crate::lsp_nav::bib_entry_text(&bib, &key)
                {
                    return markup(format!("```bibtex\n{entry}\n```"), start, end);
                }
            }
            None
        }
        // `{{< include x.tmd >}}` → where the path resolves, and whether it is there. This
        // used to answer nothing even though the target was classified and go-to-definition
        // resolved it, so the one cue that a spliced-in file is navigable was missing from
        // the place an author looks first.
        Target::Include { path, start, end } => {
            let dir = uri.to_file_path().ok()?.parent()?.to_path_buf();
            let target = dir.join(&path);
            if target.exists() {
                let href = lsp_types::Url::from_file_path(&target).ok()?;
                markup(
                    format!(
                        "[`{path}`]({href}) — spliced in here.\n\nCtrl-click (Cmd-click on macOS) to open it."
                    ),
                    start,
                    end,
                )
            } else {
                markup(
                    format!("`{path}` — **not found** relative to this document."),
                    start,
                    end,
                )
            }
        }
        Target::None => None,
    }
}

/// Build quick-fix code actions from the diagnostics the client echoed back. For each that
/// carries a `data.replacement` — a precise "did you mean" fix `to_lsp` attaches only when the
/// diagnostic's `range` is exactly the mis-typed token — emit a `QuickFix` that replaces that
/// range with the correction. Read-only w.r.t. the preview; the edit flows through the editor.
fn resolve_code_actions(
    params: &lsp_types::CodeActionParams,
) -> Option<lsp_types::CodeActionResponse> {
    use lsp_types::{CodeAction, CodeActionKind, CodeActionOrCommand, TextEdit, WorkspaceEdit};
    let uri = &params.text_document.uri;
    let mut actions = Vec::new();
    for diag in &params.context.diagnostics {
        let Some(replacement) = diag
            .data
            .as_ref()
            .and_then(|d| d.get("replacement"))
            .and_then(|r| r.as_str())
        else {
            continue;
        };
        let mut changes = std::collections::HashMap::new();
        changes.insert(
            uri.clone(),
            vec![TextEdit {
                range: diag.range,
                new_text: replacement.to_string(),
            }],
        );
        actions.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: format!("Change to `{replacement}`"),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diag.clone()]),
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                ..Default::default()
            }),
            is_preferred: Some(true),
            ..Default::default()
        }));
    }
    Some(actions)
}

/// `textDocument/prepareRename`: if the cursor is on a cross-reference anchor (an `@id`
/// reference, or a `{#id}` / `#| label: id` definition), return the id's range so the editor
/// opens its rename box pre-filled with the id. `None` — which the client surfaces as "cannot
/// rename here" — for anything else. Read-only w.r.t. the preview.
fn resolve_prepare_rename(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    params: &lsp_types::TextDocumentPositionParams,
) -> Option<lsp_types::PrepareRenameResponse> {
    use lsp_types::{Position, PrepareRenameResponse, Range};
    let text = docs.get(&params.text_document.uri)?;
    let pos = params.position;
    let cur_line = crate::lsp_pos::nth_line(text, pos.line as usize);
    let cursor_char = crate::lsp_pos::utf16_to_char(cur_line, pos.character as usize);
    let (_, start, end) = crate::lsp_nav::anchor_at(text, pos.line as usize, cursor_char)?;
    Some(PrepareRenameResponse::Range(Range::new(
        Position::new(
            pos.line,
            crate::lsp_pos::char_to_utf16(cur_line, start) as u32,
        ),
        Position::new(
            pos.line,
            crate::lsp_pos::char_to_utf16(cur_line, end) as u32,
        ),
    )))
}

/// `textDocument/rename`: rename the cross-reference anchor under the cursor — its definition
/// (`{#id}` / `#| label: id`) and every `@id` reference in this document — to `new_name`, as one
/// `WorkspaceEdit`. `Ok(None)` when the cursor is on no anchor. The edit flows through the
/// editor (the legitimate editing surface), never the preview.
///
/// `Err(reason)` when `new_name` is not a usable anchor — see
/// [`crate::lsp_nav::anchor_name_error`] for why an unvalidated name is worse here than
/// almost anywhere else. The caller turns it into a `ResponseError` so the editor shows the
/// reason in its rename box; a silent `None` would read as "nothing to rename".
fn resolve_rename(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    params: &lsp_types::RenameParams,
) -> Result<Option<lsp_types::WorkspaceEdit>, String> {
    use lsp_types::{Position, Range, TextEdit, WorkspaceEdit};
    let uri = &params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let Some(text) = docs.get(uri) else {
        return Ok(None);
    };
    let cursor_char = crate::lsp_pos::utf16_to_char(
        crate::lsp_pos::nth_line(text, pos.line as usize),
        pos.character as usize,
    );
    let Some((id, _, _)) = crate::lsp_nav::anchor_at(text, pos.line as usize, cursor_char) else {
        return Ok(None);
    };
    // Validate BEFORE building any edit, and against the id being replaced: a name that
    // leaves the anchor grammar does not fail loudly, it rewrites every site into something
    // the scanners no longer find.
    let new_name = params.new_name.trim();
    if let Some(why) = crate::lsp_nav::anchor_name_error(&id, new_name) {
        return Err(why);
    }
    // Occurrences span many lines; each edit range converts its own line's scalar columns to
    // UTF-16 so the editor overwrites exactly the id, never a byte off, on any line.
    let edits: Vec<TextEdit> = crate::lsp_nav::anchor_occurrences(text, &id)
        .into_iter()
        .map(|(line, start, end)| {
            let l = crate::lsp_pos::nth_line(text, line as usize);
            TextEdit {
                range: Range::new(
                    Position::new(
                        line,
                        crate::lsp_pos::char_to_utf16(l, start as usize) as u32,
                    ),
                    Position::new(line, crate::lsp_pos::char_to_utf16(l, end as usize) as u32),
                ),
                new_text: new_name.to_string(),
            }
        })
        .collect();
    if edits.is_empty() {
        return Ok(None);
    }
    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), edits);
    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    }))
}

/// Resolve completion at the cursor: route to the vocabulary that applies (front-matter key /
/// value, cell option, div class, xref, cite, shortcode path) and emit its items. `None` when
/// the cursor is in no completable context. Draws on the Rust-authoritative `vocab` plus
/// live document scans; this is the only implementation (the companion is an LSP client).
fn resolve_completion(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    params: &lsp_types::CompletionParams,
) -> Option<lsp_types::CompletionResponse> {
    use crate::lsp_complete::{CompletionContext as Ctx, Shortcode};
    use lsp_types::{
        CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit, Position,
        Range, TextEdit,
    };

    let uri = &params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let text = docs.get(uri)?;

    // Char-based line prefix (line start → cursor) and document prefix (doc start → cursor).
    // The incoming column is UTF-16; convert to a scalar index before slicing by `chars()`.
    let lines: Vec<&str> = text.split('\n').collect();
    let line = lines.get(pos.line as usize).copied().unwrap_or("");
    let cursor_char = crate::lsp_pos::utf16_to_char(line, pos.character as usize);
    let line_prefix: String = line.chars().take(cursor_char).collect();
    let mut doc_prefix = String::new();
    for l in lines.iter().take(pos.line as usize) {
        doc_prefix.push_str(l);
        doc_prefix.push('\n');
    }
    doc_prefix.push_str(&line_prefix);

    let ctx = crate::lsp_complete::detect_context(&line_prefix, &doc_prefix);
    if matches!(ctx, Ctx::None) {
        return None;
    }
    let vocab = taliesin_core::vocab::vocab();

    let item = |label: String, detail: String, kind: CompletionItemKind| CompletionItem {
        label,
        kind: Some(kind),
        detail: (!detail.is_empty()).then_some(detail),
        ..Default::default()
    };
    // A vocab `[{name, description}]` array → items of the given kind.
    let from_named = |v: &serde_json::Value, kind: CompletionItemKind| -> Vec<CompletionItem> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|e| {
                        let name = e["name"].as_str()?;
                        Some(item(
                            name.to_string(),
                            e["description"].as_str().unwrap_or("").to_string(),
                            kind,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let items: Vec<CompletionItem> = match ctx {
        Ctx::None => return None,
        Ctx::FrontmatterKey { parent } => {
            let list = match &parent {
                Some(p) => &vocab["frontmatter"]["nested"][p],
                None => &vocab["frontmatter"]["keys"],
            };
            from_named(list, CompletionItemKind::PROPERTY)
        }
        Ctx::FrontmatterValue { key, typed } => {
            from_named(&vocab["frontmatterValues"][&key], CompletionItemKind::VALUE)
                .into_iter()
                .filter(|it| typed.is_empty() || it.label.starts_with(&typed))
                .collect()
        }
        Ctx::CellOption => from_named(&vocab["cellOptions"], CompletionItemKind::PROPERTY),
        Ctx::DivClass => {
            let mut out = Vec::new();
            if let Some(a) = vocab["calloutKinds"].as_array() {
                for e in a {
                    if let Some(name) = e["name"].as_str() {
                        out.push(item(
                            format!("callout-{name}"),
                            e["description"].as_str().unwrap_or("").to_string(),
                            CompletionItemKind::CLASS,
                        ));
                    }
                }
            }
            out.extend(from_named(
                &vocab["theoremKinds"],
                CompletionItemKind::CLASS,
            ));
            out.extend(from_named(&vocab["divClasses"], CompletionItemKind::CLASS));
            out
        }
        Ctx::Xref { typed } => {
            let mut out = Vec::new();
            if let Some(a) = vocab["xrefPrefixes"].as_array() {
                for e in a {
                    if let (Some(prefix), Some(label)) = (e["prefix"].as_str(), e["label"].as_str())
                    {
                        out.push(item(
                            format!("{prefix}-"),
                            label.to_string(),
                            CompletionItemKind::REFERENCE,
                        ));
                    }
                }
            }
            for (id, detail) in merged_xref_targets(uri, text, &vocab) {
                if typed.is_empty() || id.starts_with(&typed) {
                    out.push(item(id, detail, CompletionItemKind::REFERENCE));
                }
            }
            out
        }
        // `\alpha`, `\frac{}{}`, `\begin{cases}` … inside `$…$`. Each item REPLACES the
        // typed control sequence (rather than appending to it), so accepting `\frac` after
        // typing `\fr` cannot leave `\fr\frac`. Commands that take arguments insert an LSP
        // snippet, so the cursor lands in the first placeholder.
        Ctx::MathCommand { typed } => {
            let start_char = cursor_char.saturating_sub(typed.chars().count());
            let replace = Range::new(
                Position::new(
                    pos.line,
                    crate::lsp_pos::char_to_utf16(line, start_char) as u32,
                ),
                Position::new(pos.line, pos.character),
            );
            vocab["mathCommands"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|e| {
                            let name = e["name"].as_str()?;
                            // The typed text always starts with `\`, so a one-char `\` matches
                            // everything, which is the point of triggering on the backslash.
                            if !name.starts_with(typed.as_str()) {
                                return None;
                            }
                            let snippet = e["snippet"].as_str().unwrap_or("");
                            let insert = if snippet.is_empty() { name } else { snippet };
                            let category = e["category"].as_str().unwrap_or("");
                            Some(CompletionItem {
                                label: name.to_string(),
                                kind: Some(CompletionItemKind::FUNCTION),
                                detail: Some(format!(
                                    "{}  ·  {category}",
                                    e["description"].as_str().unwrap_or("")
                                )),
                                insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                                    range: replace,
                                    new_text: insert.to_string(),
                                })),
                                // Sort by name so the list reads alphabetically rather than
                                // in the vocabulary's category order, which looks arbitrary
                                // once it is filtered down to a few matches.
                                sort_text: Some(name.to_string()),
                                ..Default::default()
                            })
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
        // `{{< ` then a name. Each item inserts the full `name ` so the path/argument
        // completion that follows opens straight away.
        Ctx::ShortcodeName { typed } => crate::lsp_complete::shortcode_names()
            .iter()
            .filter(|(name, _)| name.starts_with(typed.as_str()))
            .map(|(name, description)| CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(description.to_string()),
                insert_text: Some(format!("{name} ")),
                ..Default::default()
            })
            .collect(),
        // ` ```{py ` -> the cell languages, with the executed ones marked (a `{bash}` cell
        // labelled `fig-…` never produces a figure, so the split has to be visible here).
        Ctx::CellLanguage { typed } => vocab["cellLanguages"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|e| {
                        let name = e["name"].as_str()?;
                        if !name.starts_with(typed.as_str()) {
                            return None;
                        }
                        Some(item(
                            name.to_string(),
                            e["description"].as_str().unwrap_or("").to_string(),
                            if e["executes"].as_bool().unwrap_or(false) {
                                CompletionItemKind::EVENT
                            } else {
                                CompletionItemKind::VALUE
                            },
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        // `{#` -> the cross-reference prefixes. Defining an anchor is where the prefix has
        // to be right; `@` already offered them for referencing one.
        Ctx::AnchorId { typed } => vocab["xrefPrefixes"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|e| {
                        let prefix = format!("{}-", e["prefix"].as_str()?);
                        if !prefix.starts_with(typed.as_str()) {
                            return None;
                        }
                        Some(item(
                            prefix,
                            format!("{} anchor", e["label"].as_str().unwrap_or("")),
                            CompletionItemKind::REFERENCE,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        // `{{< input type=` -> the control kinds. `inputTypes` has been in the vocabulary
        // since it was written and nothing ever read it.
        Ctx::InputType { typed } => vocab["inputTypes"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|e| {
                        let name = e.as_str()?;
                        name.starts_with(typed.as_str()).then(|| {
                            item(
                                name.to_string(),
                                "reader-facing control".to_string(),
                                CompletionItemKind::VALUE,
                            )
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        // A `#| echo: ` style option whose value has a closed set.
        Ctx::CellOptionValue { key, typed } => crate::lsp_complete::cell_option_values(&key)
            .iter()
            .filter(|(value, _)| value.starts_with(typed.as_str()))
            .map(|(value, description)| {
                item(
                    value.to_string(),
                    description.to_string(),
                    CompletionItemKind::VALUE,
                )
            })
            .collect(),
        // Any other path position: a path-valued front-matter key, a markdown link or image
        // target. `bibliography:`/`css:`/`image:` were detected as value positions all
        // along and then answered nothing, because the only value vocabulary was the two
        // word lists (`format`, `theme`).
        Ctx::Path { typed, kind } => {
            let doc_dir = uri.to_file_path().ok()?;
            let doc_dir = doc_dir.parent()?.to_path_buf();
            let dir_part = match typed.rfind('/') {
                Some(s) => &typed[..s + 1],
                None => "",
            };
            let entries: Vec<crate::lsp_complete::DirEntry> =
                std::fs::read_dir(doc_dir.join(dir_part))
                    .ok()?
                    .filter_map(|e| e.ok())
                    .map(|e| crate::lsp_complete::DirEntry {
                        name: e.file_name().to_string_lossy().into_owned(),
                        is_dir: e.file_type().map(|t| t.is_dir()).unwrap_or(false),
                    })
                    .collect();
            let start_char = cursor_char.saturating_sub(typed.chars().count());
            let replace = Range::new(
                Position::new(
                    pos.line,
                    crate::lsp_pos::char_to_utf16(line, start_char) as u32,
                ),
                Position::new(pos.line, pos.character),
            );
            crate::lsp_complete::path_candidates(&entries, &typed, kind.extensions(), kind.detail())
                .into_iter()
                .map(|c| {
                    let is_dir = c.value.ends_with('/');
                    CompletionItem {
                        label: c.value.clone(),
                        kind: Some(if is_dir {
                            CompletionItemKind::FOLDER
                        } else {
                            CompletionItemKind::FILE
                        }),
                        detail: Some(c.detail),
                        filter_text: Some(c.value.clone()),
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                            range: replace,
                            new_text: c.value,
                        })),
                        // A directory keeps the menu open so you can descend without re-typing.
                        command: is_dir.then(|| lsp_types::Command {
                            title: String::new(),
                            command: "editor.action.triggerSuggest".to_string(),
                            arguments: None,
                        }),
                        ..Default::default()
                    }
                })
                .collect()
        }
        Ctx::Cite => {
            let dir = uri.to_file_path().ok()?;
            let dir = dir.parent()?.to_path_buf();
            let mut keys = std::collections::BTreeSet::new();
            for rel in crate::lsp_nav::frontmatter_bib_paths(text) {
                if let Ok(bib) = std::fs::read_to_string(dir.join(&rel)) {
                    for k in crate::lsp_complete::harvest_bib_keys(&bib) {
                        keys.insert(k);
                    }
                }
            }
            keys.into_iter()
                .map(|k| item(k, "citation key".to_string(), CompletionItemKind::REFERENCE))
                .collect()
        }
        Ctx::ShortcodePath { shortcode, typed } => {
            let doc_dir = uri.to_file_path().ok()?;
            let doc_dir = doc_dir.parent()?.to_path_buf();
            let dir_part = match typed.rfind('/') {
                Some(s) => &typed[..s + 1],
                None => "",
            };
            let entries: Vec<crate::lsp_complete::DirEntry> =
                std::fs::read_dir(doc_dir.join(dir_part))
                    .ok()?
                    .filter_map(|e| e.ok())
                    .map(|e| crate::lsp_complete::DirEntry {
                        name: e.file_name().to_string_lossy().into_owned(),
                        is_dir: e.file_type().map(|t| t.is_dir()).unwrap_or(false),
                    })
                    .collect();
            let file_detail = match shortcode {
                Shortcode::Embed => "deck / page",
                Shortcode::Include => "partial",
            };
            // Replace the whole typed path (incl. any dir prefix) so descending overwrites
            // cleanly rather than appending to a half-typed segment. The start is the cursor
            // less the typed length in scalars, re-expressed in UTF-16; the end is the cursor.
            let typed_len = typed.chars().count();
            let start_char = cursor_char.saturating_sub(typed_len);
            let replace = Range::new(
                Position::new(
                    pos.line,
                    crate::lsp_pos::char_to_utf16(line, start_char) as u32,
                ),
                pos,
            );
            crate::lsp_complete::shortcode_path_candidates(&entries, &typed, file_detail)
                .into_iter()
                .map(|c| {
                    let is_dir = c.value.ends_with('/');
                    CompletionItem {
                        label: c.value.clone(),
                        kind: Some(if is_dir {
                            CompletionItemKind::FOLDER
                        } else {
                            CompletionItemKind::FILE
                        }),
                        detail: Some(c.detail),
                        filter_text: Some(c.value.clone()),
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                            range: replace,
                            new_text: c.value,
                        })),
                        ..Default::default()
                    }
                })
                .collect()
        }
    };
    Some(CompletionResponse::Array(items))
}

/// Union the two views of a document's cross-reference targets, sorted and deduplicated: the
/// buffer `{#id}` anchors (a just-typed anchor is completable before it numbers) and the live
/// render's numbered registry (the `#| label:` cell figures a scan can't see, with their
/// numbers). `detail` is `"{label} {number}"` when known, else `"cross-reference target"`.
/// The in-process, staleness-free equivalent of the companion's `mergeXrefTargets`.
fn merged_xref_targets(
    uri: &lsp_types::Url,
    text: &str,
    vocab: &serde_json::Value,
) -> Vec<(String, String)> {
    let mut detail: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for id in crate::lsp_complete::harvest_anchor_ids(text) {
        detail.insert(id, "cross-reference target".to_string());
    }
    if let Some(doc) = render_buffer(uri, text) {
        let labels: std::collections::HashMap<&str, &str> = vocab["xrefPrefixes"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|e| Some((e["prefix"].as_str()?, e["label"].as_str()?)))
                    .collect()
            })
            .unwrap_or_default();
        for (id, number) in &doc.xref_numbers {
            if !taliesin_core::cite::is_xref_anchor(id) {
                continue;
            }
            let kind = id.split_once('-').map(|(k, _)| k).unwrap_or("");
            let d = match labels.get(kind) {
                Some(label) if !number.is_empty() => format!("{label} {number}"),
                _ => "cross-reference target".to_string(),
            };
            detail.insert(id.clone(), d);
        }
    }
    detail.into_iter().collect()
}

/// Render the live buffer parse-only (no kernel), rooted at the document's directory, the
/// same render `symbols` uses. Panic-guarded so a malformed buffer yields `None` rather than
/// crashing the request loop. Shared by hover and completion for xref resolution.
fn render_buffer(uri: &lsp_types::Url, text: &str) -> Option<taliesin_core::RenderedDoc> {
    let base = uri
        .to_file_path()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    crate::serve::guarded(|| taliesin_core::render_single_doc(text, &base)).ok()
}

/// The rendered cross-reference number for `id` in the live buffer (e.g. `"2"` / `"2.1"`), or
/// `None` when the buffer defines no such numbered target.
fn xref_number(uri: &lsp_types::Url, text: &str, id: &str) -> Option<String> {
    render_buffer(uri, text)?.xref_numbers.get(id).cloned()
}

/// The label for `id`'s cross-reference kind (`fig` → `Figure`), from the public vocab
/// (built from `cite::XREF_LABELS`), or `None` when the prefix names no cross-reference kind.
fn xref_label(id: &str) -> Option<String> {
    let kind = id.split_once('-').map(|(k, _)| k)?;
    let vocab = taliesin_core::vocab::vocab();
    vocab["xrefPrefixes"]
        .as_array()?
        .iter()
        .find(|e| e["prefix"].as_str() == Some(kind))
        .and_then(|e| e["label"].as_str())
        .map(str::to_string)
}

/// The one-line documentation for front-matter key `key` (optionally under nested `parent`),
/// from the public vocab, or `None` when the key is not documented.
fn frontmatter_key_doc(parent: Option<&str>, key: &str) -> Option<String> {
    let vocab = taliesin_core::vocab::vocab();
    let list = match parent {
        Some(p) => &vocab["frontmatter"]["nested"][p],
        None => &vocab["frontmatter"]["keys"],
    };
    list.as_array()?
        .iter()
        .find(|e| e["name"].as_str() == Some(key))
        .and_then(|e| e["description"].as_str())
        .map(str::to_string)
}

/// `textDocument/documentLink`: the `{{< include >}}` / `{{< embed >}}` paths in `uri`, each
/// pointing at the file it resolves to.
///
/// Only a target that EXISTS on disk becomes a link. A missing path is `check`'s finding to
/// report (`TAL-INCLUDE-*`), and painting it as a link would promise a jump that lands on
/// nothing, which reads as a broken editor rather than a broken path.
fn document_links(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    uri: &lsp_types::Url,
) -> Option<Vec<lsp_types::DocumentLink>> {
    use lsp_types::{DocumentLink, Position, Range};
    let text = docs.get(uri)?;
    // Paths resolve against the including file's own directory, the same base
    // `includes::resolve` recurses with. An untitled buffer has no directory, so nothing
    // can be resolved against it.
    let dir = uri.to_file_path().ok()?.parent()?.to_path_buf();
    let lines: Vec<&str> = text.split('\n').collect();
    let links = crate::lsp_links::path_links(text)
        .into_iter()
        .filter_map(|l| {
            let target = dir.join(&l.path);
            if !target.exists() {
                return None;
            }
            // Scalar columns in, UTF-16 on the wire (`lsp_pos`), converted against the
            // link's own line so an astral char earlier in it cannot shift the span.
            let line_text = lines.get(l.line as usize).copied().unwrap_or("");
            Some(DocumentLink {
                range: Range::new(
                    Position::new(
                        l.line,
                        crate::lsp_pos::char_to_utf16(line_text, l.start) as u32,
                    ),
                    Position::new(
                        l.line,
                        crate::lsp_pos::char_to_utf16(line_text, l.end) as u32,
                    ),
                ),
                target: lsp_types::Url::from_file_path(&target).ok(),
                tooltip: Some(format!("Open {}", l.path)),
                data: None,
            })
        })
        .collect();
    Some(links)
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
/// heading level, so breadcrumbs and sticky scroll read as a document outline.
fn to_document_symbol(
    node: &crate::lsp_outline::OutlineNode,
    lines: &[&str],
) -> lsp_types::DocumentSymbol {
    use lsp_types::{Position, Range, SymbolKind};
    let last = lines.len().saturating_sub(1);
    let start = node.start_line.min(last);
    let end = node.end_line.max(node.start_line).min(last);
    // Column 0 needs no conversion; the end-of-line column is a UTF-16 unit count.
    let line_len = |i: usize| {
        lines
            .get(i)
            .map_or(0, |l| l.chars().map(char::len_utf16).sum::<usize>()) as u32
    };
    let name = if node.title.is_empty() {
        "(untitled)".to_string()
    } else {
        node.title.clone()
    };
    // The section's prose length, shown beside its name in the editor outline — the one
    // structural measure an author can act on while writing, at the moment they can act on
    // it. Counted over the node's own markdown line extent (which `lsp_outline` computed to
    // bound the section) via the shared `prose::word_count`, so the outline, `lint`, `map`
    // and the page's reading-time figure can never report four different lengths. Counting
    // rendered text instead would count fenced code and cell output as prose.
    //
    // A node's extent spans its subsections too, so for a parent this is the whole
    // section's length, not the prose directly under its own heading. That is the number
    // worth showing (it answers "how long is this section" the way a reader means it), but
    // it must not be read as own-prose, so a parent says "total" and a leaf does not.
    let words = taliesin_core::prose::word_count(&lines[start..=end].join("\n"));
    let detail = (words > 0).then(|| {
        if node.children.is_empty() {
            format!("{words} words")
        } else {
            format!("{words} words total")
        }
    });
    #[allow(deprecated)] // `deprecated` is a required (deprecated) field of DocumentSymbol.
    lsp_types::DocumentSymbol {
        name,
        detail,
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

/// Whether a URI names a `.tmd` document, used as the second admission signal beside
/// the `taliesin` language id. Editors other than the VS Code companion have no reason
/// to know that id, so the path extension is what makes `taliesin lsp` work in the
/// Neovim / Helix / Zed setups the CLI reference documents.
fn is_tmd_uri(uri: &lsp_types::Url) -> bool {
    std::path::Path::new(uri.path())
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("tmd"))
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

    // Send a goto-definition request and return what the server answered — `None` when it
    // answered `null` ("this token points nowhere"), which the editor surfaces as a no-op.
    fn definition_at(
        client: &Connection,
        uri: &Url,
        id: i32,
        line: u32,
        character: u32,
    ) -> Option<lsp_types::GotoDefinitionResponse> {
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(id),
                method: lsp_types::request::GotoDefinition::METHOD.to_owned(),
                params: serde_json::to_value(goto_params(uri, line, character)).unwrap(),
            }))
            .unwrap();
        let resp = recv_response(client, RequestId::from(id));
        serde_json::from_value(resp.result.expect("a definition result")).unwrap()
    }

    fn hover_params(uri: &Url, line: u32, character: u32) -> lsp_types::HoverParams {
        lsp_types::HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: lsp_types::Position::new(line, character),
            },
            work_done_progress_params: Default::default(),
        }
    }

    // Send a hover request and return what the server answered — `None` when it answered
    // `null` ("nothing to show here"), which is a real outcome and not a failure: a hover
    // that appears where none should is how a wrong lookup shows up.
    fn hover_raw_at(
        client: &Connection,
        uri: &Url,
        id: i32,
        line: u32,
        character: u32,
    ) -> Option<lsp_types::Hover> {
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(id),
                method: lsp_types::request::HoverRequest::METHOD.to_owned(),
                params: serde_json::to_value(hover_params(uri, line, character)).unwrap(),
            }))
            .unwrap();
        let resp = recv_response(client, RequestId::from(id));
        serde_json::from_value(resp.result.expect("a hover result")).unwrap()
    }

    // Send a hover request and return its resolved `Hover`, failing if the server answered
    // with `null` (no hover).
    fn hover_at(
        client: &Connection,
        uri: &Url,
        id: i32,
        line: u32,
        character: u32,
    ) -> lsp_types::Hover {
        hover_raw_at(client, uri, id, line, character).expect("a hover (got null)")
    }

    fn complete_params(uri: &Url, line: u32, character: u32) -> lsp_types::CompletionParams {
        lsp_types::CompletionParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: lsp_types::Position::new(line, character),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        }
    }

    // Send a completion request and return what the server answered — `None` for `null`
    // ("no completion applies here"), which is distinct from an empty item list.
    fn complete_raw_at(
        client: &Connection,
        uri: &Url,
        id: i32,
        line: u32,
        character: u32,
    ) -> Option<Vec<lsp_types::CompletionItem>> {
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(id),
                method: lsp_types::request::Completion::METHOD.to_owned(),
                params: serde_json::to_value(complete_params(uri, line, character)).unwrap(),
            }))
            .unwrap();
        let resp = recv_response(client, RequestId::from(id));
        let response: Option<lsp_types::CompletionResponse> =
            serde_json::from_value(resp.result.expect("a completion result")).unwrap();
        response.map(|r| match r {
            lsp_types::CompletionResponse::Array(items) => items,
            lsp_types::CompletionResponse::List(l) => l.items,
        })
    }

    // Send a completion request and return the item list.
    fn complete_at(
        client: &Connection,
        uri: &Url,
        id: i32,
        line: u32,
        character: u32,
    ) -> Vec<lsp_types::CompletionItem> {
        complete_raw_at(client, uri, id, line, character).expect("a completion list (got null)")
    }

    // Send a prepareRename request and return its response (None when the server answered null).
    fn prepare_rename_at(
        client: &Connection,
        uri: &Url,
        id: i32,
        line: u32,
        character: u32,
    ) -> Option<lsp_types::PrepareRenameResponse> {
        let params = lsp_types::TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: lsp_types::Position::new(line, character),
        };
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(id),
                method: lsp_types::request::PrepareRenameRequest::METHOD.to_owned(),
                params: serde_json::to_value(params).unwrap(),
            }))
            .unwrap();
        let resp = recv_response(client, RequestId::from(id));
        serde_json::from_value(resp.result.expect("a prepareRename result")).unwrap()
    }

    // Send a rename request and return its WorkspaceEdit (None when the server answered null).
    fn rename_at(
        client: &Connection,
        uri: &Url,
        id: i32,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Option<lsp_types::WorkspaceEdit> {
        let params = lsp_types::RenameParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: lsp_types::Position::new(line, character),
            },
            new_name: new_name.to_owned(),
            work_done_progress_params: Default::default(),
        };
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(id),
                method: lsp_types::request::Rename::METHOD.to_owned(),
                params: serde_json::to_value(params).unwrap(),
            }))
            .unwrap();
        let resp = recv_response(client, RequestId::from(id));
        serde_json::from_value(resp.result.expect("a rename result")).unwrap()
    }

    // Send a rename request and return the RAW response, so a test can assert the server
    // answered with a ResponseError rather than an edit. `rename_at` unwraps `result` and
    // would panic on exactly the case a rejection test exists to check.
    fn rename_raw_at(
        client: &Connection,
        uri: &Url,
        id: i32,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> lsp_server::Response {
        let params = lsp_types::RenameParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: lsp_types::Position::new(line, character),
            },
            new_name: new_name.to_owned(),
            work_done_progress_params: Default::default(),
        };
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(id),
                method: lsp_types::request::Rename::METHOD.to_owned(),
                params: serde_json::to_value(params).unwrap(),
            }))
            .unwrap();
        recv_response(client, RequestId::from(id))
    }

    // Pull the Markdown string out of a hover's contents.
    fn hover_markdown(h: &lsp_types::Hover) -> String {
        match &h.contents {
            lsp_types::HoverContents::Markup(m) => m.value.clone(),
            other => panic!("expected markup hover, got {other:?}"),
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

    // HEALTH-1: the resilience property that could not exist while the dispatch was
    // unguarded. A panic in a request is answered as an InternalError, a panic in a
    // notification is dropped, and in BOTH cases the session keeps serving — previously
    // either one unwound out of `main_loop` and every later message went unanswered while
    // the editor sat there believing it still had language intelligence. Mutation check:
    // remove either `guarded` in `main_loop` and this test hangs on the timeout.
    #[test]
    fn a_panicking_message_does_not_kill_the_session() {
        let (server, client) = Connection::memory();
        let prior = std::panic::take_hook();
        // Both panics below are injected on purpose; keep the backtraces out of the output.
        std::panic::set_hook(Box::new(|_| {}));
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        // (1) A panicking REQUEST is answered, so the client never hangs waiting.
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(10),
                method: PANIC_PROBE_METHOD.to_owned(),
                params: serde_json::Value::Null,
            }))
            .unwrap();
        let err = recv_response(&client, RequestId::from(10))
            .error
            .expect("a panicking request must be answered with an error, not silence");
        assert_eq!(err.code, -32603, "JSON-RPC InternalError");

        // (2) A panicking NOTIFICATION is skipped, and the buffer store still works after —
        // this is the every-keystroke path (`publish` → `buffer_diagnostics`).
        client
            .sender
            .send(Message::Notification(Notification {
                method: PANIC_PROBE_METHOD.to_owned(),
                params: serde_json::Value::Null,
            }))
            .unwrap();

        // (3) The session still answers real work after both panics.
        let path = corpus("typos.tmd");
        let uri = Url::from_file_path(&path).unwrap();
        did_open(&client, &uri, std::fs::read_to_string(&path).unwrap());
        let published = recv_publish(&client);
        assert_eq!(
            published.uri, uri,
            "diagnostics still flow after two panics"
        );

        shutdown(&client);
        std::panic::set_hook(prior);
        thread.join().unwrap().expect("server loop should exit Ok");
    }

    // The sibling half of the guard above, and the likelier one: a message whose params do
    // not deserialize is an `Err`, not a panic, so `guarded` never sees it. Every `let params
    // = serde_json::from_value(…)?` in the dispatch used to propagate straight out of
    // `main_loop` → `run` → `cmd_lsp`, which logged it and returned `ExitCode::FAILURE` — the
    // offending request unanswered and the whole session dead, which is exactly the outcome
    // the panic guard exists to prevent. Measured against the release binary before the fix,
    // four of eight plausible client message shapes killed the process (exit code 1):
    // a `completion` carrying `context: {}` (no `triggerKind`), a float `position`, a
    // negative `position`, and a `rename` with `newName: null`. `taliesin lsp` serves the
    // editors the VS Code companion does not (it has its own TS providers), so these shapes
    // come from clients this repo does not control. Mutation check: restore either `?` in
    // `main_loop`'s dispatch arms and this test fails on the join, not the timeout.
    #[test]
    fn a_malformed_message_does_not_kill_the_session() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        // (1) A REQUEST whose params fail to deserialize (`hover` with no `position`) is
        // answered with InvalidParams rather than killing the process mid-flight.
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(20),
                method: lsp_types::request::HoverRequest::METHOD.to_owned(),
                params: serde_json::json!({ "textDocument": { "uri": "file:///x.tmd" } }),
            }))
            .unwrap();
        let err = recv_response(&client, RequestId::from(20))
            .error
            .expect("a malformed request must be answered with an error, not silence");
        assert_eq!(err.code, -32602, "JSON-RPC InvalidParams");

        // (2) A malformed NOTIFICATION on the every-keystroke channel is logged and skipped.
        client
            .sender
            .send(Message::Notification(Notification {
                method: DidChangeTextDocument::METHOD.to_owned(),
                params: serde_json::json!({ "textDocument": { "uri": "file:///x.tmd" } }),
            }))
            .unwrap();

        // (3) The session still answers real work after both.
        let path = corpus("typos.tmd");
        let uri = Url::from_file_path(&path).unwrap();
        did_open(&client, &uri, std::fs::read_to_string(&path).unwrap());
        let published = recv_publish(&client);
        assert_eq!(
            published.uri, uri,
            "diagnostics still flow after two malformed messages"
        );

        shutdown(&client);
        thread.join().unwrap().expect("server loop should exit Ok");
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
                // The range covers the id itself — `fig-1` is columns 10..15 of
                // `# Title {#fig-1}` — so the editor highlights the anchor it jumped to and
                // nothing else. Only asserting the start left the end arithmetic unpinned.
                assert_eq!(
                    loc.range.end,
                    lsp_types::Position::new(0, 15),
                    "the range should end at the id's last character"
                );
            }
            other => panic!("expected a scalar location, got {other:?}"),
        }

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    // Two axes: an include whose file is on disk jumps to it, and one whose file is missing
    // resolves to nothing. The `!abs.exists()` guard is what separates them, and a test that
    // only ever asks about a file that exists cannot see it inverted.
    #[test]
    fn goto_definition_on_an_include_resolves_only_a_file_that_exists() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-include-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("part.tmd");
        std::fs::write(&part, "Some shared prose.\n").unwrap();
        let doc = dir.join("book.tmd");
        let text = "{{< include part.tmd >}}\n\n{{< include gone.tmd >}}\n".to_string();
        std::fs::write(&doc, &text).unwrap();

        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);
        let uri = Url::from_file_path(&doc).unwrap();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // `{{< include ` is 12 characters, so char 14 is inside the path token on both lines.
        match definition_at(&client, &uri, 41, 0, 14) {
            Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) => {
                assert_eq!(loc.uri, Url::from_file_path(&part).unwrap());
                assert_eq!(loc.range.start, lsp_types::Position::new(0, 0));
            }
            other => panic!("expected a location into part.tmd, got {other:?}"),
        }
        assert_eq!(
            definition_at(&client, &uri, 42, 2, 14),
            None,
            "an include of a file that is not on disk must resolve to nothing, not to a \
             location the editor then fails to open"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A method the server does not implement is answered with JSON-RPC MethodNotFound. The
    // code is the whole contract here: a client reads it to decide between "this server
    // can't do that" and a real failure, and no test looked at it.
    #[test]
    fn an_unhandled_request_is_answered_with_method_not_found() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(43),
                method: lsp_types::request::Formatting::METHOD.to_owned(),
                params: serde_json::Value::Null,
            }))
            .unwrap();
        let resp = recv_response(&client, RequestId::from(43));
        assert!(
            resp.result.is_none(),
            "an unhandled method must not answer with a result"
        );
        let err = resp
            .error
            .expect("an unhandled method must be answered, not met with silence");
        assert_eq!(err.code, -32601, "JSON-RPC MethodNotFound");
        assert!(
            err.message.contains(lsp_types::request::Formatting::METHOD),
            "the message should name the method, got {:?}",
            err.message
        );

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

    #[test]
    fn document_symbol_detail_is_the_sections_prose_length() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-outline-words.tmd").unwrap();
        // Heading text counts as prose, because `prose::word_count` is the same measure
        // behind the page's reading-time figure and a reader reads headings. Fenced code
        // does not. So `Sub` is "Sub" + four body words = 5, and `Top` is "Top" + three
        // body words + all of `Sub` = 9.
        let text = "# Top\n\none two three\n\n```\nnot prose at all here\n```\n\n## Sub\n\nfour \
                    five six seven\n"
            .to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(31),
                method: lsp_types::request::DocumentSymbolRequest::METHOD.to_owned(),
                params: serde_json::to_value(lsp_types::DocumentSymbolParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                })
                .unwrap(),
            }))
            .unwrap();
        let resp = recv_response(&client, RequestId::from(31));
        let syms: Vec<lsp_types::DocumentSymbol> =
            serde_json::from_value(resp.result.expect("a documentSymbol result")).unwrap();

        // A node's extent spans its subsections, so the parent reports the whole section and
        // says "total"; a leaf reports only its own prose. Naming the difference is the
        // point — the same number under both labels would quietly read as own-prose.
        assert_eq!(
            syms[0].detail.as_deref(),
            Some("9 words total"),
            "the parent should report its subtree, labelled as such"
        );
        let kids = syms[0].children.as_ref().expect("Top has children");
        assert_eq!(
            kids[0].detail.as_deref(),
            Some("5 words"),
            "a leaf reports its own prose, unlabelled"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    // A section with no prose at all carries NO detail — not "0 words". The zero is the
    // boundary the `words > 0` gate exists for, and every fixture above is well past it: a
    // heading's own text counts as prose, so reaching zero needs an untitled heading over a
    // body that is entirely fenced code.
    #[test]
    fn document_symbol_detail_is_omitted_for_a_wordless_section() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-outline-empty.tmd").unwrap();
        let text = "# \n\n```\nlet x = 1;\n```\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(55),
                method: lsp_types::request::DocumentSymbolRequest::METHOD.to_owned(),
                params: serde_json::to_value(lsp_types::DocumentSymbolParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                })
                .unwrap(),
            }))
            .unwrap();
        let resp = recv_response(&client, RequestId::from(55));
        let syms: Vec<lsp_types::DocumentSymbol> =
            serde_json::from_value(resp.result.expect("a documentSymbol result")).unwrap();

        assert_eq!(syms.len(), 1, "one heading, got {syms:?}");
        assert_eq!(syms[0].name, "(untitled)");
        assert_eq!(
            syms[0].detail, None,
            "a wordless section states nothing rather than promising `0 words`"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    #[test]
    fn hover_on_an_xref_shows_the_rendered_label_and_number() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-hover-xref.tmd").unwrap();
        // A markdown image is a numbered figure (`fig 1`); a heading `{#fig-1}` would not be.
        let text = "![A scree plot](img.png){#fig-scree}\n\nSee @fig-scree here.\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // `@fig-scree` sits on line 2 starting at char 4; char 6 is inside it.
        let h = hover_at(&client, &uri, 11, 2, 6);
        let md = hover_markdown(&h);
        assert!(
            md.contains("Figure 1"),
            "expected the rendered label, got {md:?}"
        );
        assert!(
            md.contains("@fig-scree"),
            "expected the id echoed, got {md:?}"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    #[test]
    fn hover_on_a_frontmatter_key_shows_its_docs() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-hover-fm.tmd").unwrap();
        let text = "---\ntitle: Hello\n---\n\nBody.\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // The `title` key is on line 1; char 2 is inside the key token.
        let h = hover_at(&client, &uri, 13, 1, 2);
        let md = hover_markdown(&h);
        assert!(
            md.starts_with("`title:`"),
            "expected the key header, got {md:?}"
        );
        // The documentation must be THIS key's, quoted from the vocab. A `contains("title")`
        // here was satisfied by the header alone, so a lookup returning any other key's
        // description — or an empty one, or a constant — passed unnoticed.
        let expected = taliesin_core::vocab::vocab()["frontmatter"]["keys"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == "title")
            .and_then(|e| e["description"].as_str())
            .expect("the vocab documents `title:`")
            .to_string();
        assert!(!expected.is_empty(), "the vocab entry must carry prose");
        assert!(
            md.contains(&expected),
            "expected `title:`'s own documentation ({expected:?}), got {md:?}"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    // The reject axis for the same lookup: a key the vocab does not document gets NO hover.
    // Without this, a lookup that always answers something (a constant, or "the first entry
    // that isn't this one") looks correct from the positive case alone — and would invent
    // documentation for a key that has none, which is worse than saying nothing.
    #[test]
    fn hover_on_an_undocumented_frontmatter_key_shows_nothing() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-hover-fm-unknown.tmd").unwrap();
        let text = "---\nfrobnicate: Hello\n---\n\nBody.\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        assert!(
            hover_raw_at(&client, &uri, 53, 1, 2).is_none(),
            "an undocumented key must not be handed another key's documentation"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    #[test]
    fn hover_on_a_citation_shows_the_bib_entry() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-hovcite-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("refs.bib"),
            "@article{smith2020,\n  title = {A {Deep} Study}\n}\n",
        )
        .unwrap();
        let doc = dir.join("paper.tmd");
        let text =
            "---\nbibliography: refs.bib\n---\n\nAs shown in [@smith2020], it holds.\n".to_string();
        std::fs::write(&doc, &text).unwrap();

        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);
        let uri = Url::from_file_path(&doc).unwrap();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // `[@smith2020]` on line 4; the key starts at char 14, so char 15 is inside it.
        let h = hover_at(&client, &uri, 15, 4, 15);
        let md = hover_markdown(&h);
        assert!(
            md.contains("```bibtex"),
            "expected a bibtex code block, got {md:?}"
        );
        assert!(
            md.contains("@article{smith2020,") && md.contains("A {Deep} Study"),
            "expected the brace-balanced entry, got {md:?}"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn completion_offers_nested_frontmatter_keys() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-comp-fm.tmd").unwrap();
        // Line 2 is an indented key position under `execute:`.
        let text = "---\nexecute:\n  \n---\n\nBody.\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        let items = complete_at(&client, &uri, 21, 2, 2);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        let echo = items
            .iter()
            .find(|i| i.label == "echo")
            .unwrap_or_else(|| panic!("expected an `execute:` child key, got {labels:?}"));
        // Every item carries its kind, which is what the editor draws an icon from and
        // sorts by; without one the list degrades to undifferentiated text.
        assert_eq!(
            echo.kind,
            Some(lsp_types::CompletionItemKind::PROPERTY),
            "a front-matter key completes as a property"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    // The typed-prefix filter, on the axis the `||` short-circuit hides: with nothing typed
    // every value is offered (covered by the tests above, which complete from an empty
    // token), so only a NON-empty prefix shows whether the filter runs at all — and shows
    // it in both directions, since the match must survive and the non-match must not.
    #[test]
    fn completion_filters_frontmatter_values_by_the_typed_prefix() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-comp-fmvalue.tmd").unwrap();
        let text = "---\nformat: d\n---\n\nBody.\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // Line 1 is `format: d`; the cursor sits after the `d`, so `html` cannot match.
        let items = complete_at(&client, &uri, 45, 1, 9);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["deck"],
            "only the values matching what is typed should be offered"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    // Same axis for cross-reference ids, which are the higher-traffic half: an author types
    // `@fig` to narrow a long list, and a filter that drops everything (or filters nothing)
    // is equally useless.
    #[test]
    fn completion_filters_xref_targets_by_the_typed_prefix() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-comp-xref-typed.tmd").unwrap();
        let text = "![A scree plot](img.png){#fig-scree}\n\n## Intro {#sec-intro}\n\nSee @fig\n"
            .to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // Cursor at the end of `See @fig` on line 4.
        let items = complete_at(&client, &uri, 47, 4, 8);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.contains(&"fig-scree"),
            "a target matching the typed prefix should survive, got {labels:?}"
        );
        assert!(
            !labels.contains(&"sec-intro"),
            "a target that does not match the typed prefix should be filtered out, got \
             {labels:?}"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    // An id'd but unnumbered target (`theorems: numbered: false`) registers with an EMPTY
    // number. It must fall back to the generic detail rather than render its label with the
    // number missing — "Theorem " with a trailing space is what the guard exists to prevent.
    #[test]
    fn completion_detail_stays_generic_for_an_unnumbered_target() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-comp-unnumbered.tmd").unwrap();
        let text = "---\ntheorems:\n  numbered: false\n---\n\n::: {.theorem #thm-key}\n\
                    A claim that carries no number.\n:::\n\nSee @\n"
            .to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // Cursor right after the `@` on line 9.
        let items = complete_at(&client, &uri, 49, 9, 5);
        let hit = items
            .iter()
            .find(|i| i.label == "thm-key")
            .expect("the unnumbered theorem's anchor should still be offered");
        assert_eq!(
            hit.detail.as_deref(),
            Some("cross-reference target"),
            "an unnumbered target has no number to show, so it must not claim a label"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    // A shortcode path completion inside a subdirectory: the directory prefix decides which
    // directory is listed, and the item is a full `TextEdit` replacing the whole typed path
    // (label/kind/detail/filterText/textEdit), so descending overwrites rather than appends.
    #[test]
    fn completion_offers_a_shortcode_path_inside_a_subdirectory() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-shortcode-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/part.tmd"), "Shared prose.\n").unwrap();
        let doc = dir.join("book.tmd");
        let text = "{{< include sub/\n".to_string();
        std::fs::write(&doc, &text).unwrap();

        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);
        let uri = Url::from_file_path(&doc).unwrap();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // Cursor at the end of `{{< include sub/` (16 characters) on line 0.
        let items = complete_at(&client, &uri, 51, 0, 16);
        assert_eq!(
            items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(),
            vec!["sub/part.tmd"],
            "the subdirectory's own .tmd file is what should be offered"
        );
        let it = &items[0];
        assert_eq!(it.kind, Some(lsp_types::CompletionItemKind::FILE));
        assert_eq!(it.detail.as_deref(), Some("partial"));
        assert_eq!(
            it.filter_text.as_deref(),
            Some("sub/part.tmd"),
            "the filter text must be the full path, or the editor filters the leaf against \
             the whole typed prefix and hides the item"
        );
        match it.text_edit.as_ref().expect("a text edit") {
            lsp_types::CompletionTextEdit::Edit(e) => {
                assert_eq!(e.new_text, "sub/part.tmd");
                assert_eq!(
                    e.range,
                    lsp_types::Range::new(
                        lsp_types::Position::new(0, 12),
                        lsp_types::Position::new(0, 16)
                    ),
                    "the edit replaces the whole typed path, `sub/` included"
                );
            }
            other => panic!("expected a plain edit, got {other:?}"),
        }

        shutdown(&client);
        thread.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn completion_offers_xref_targets_with_labels() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-comp-xref.tmd").unwrap();
        let text = "![A scree plot](img.png){#fig-scree}\n\nSee @\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // Cursor right after the `@` on line 2.
        let items = complete_at(&client, &uri, 23, 2, 5);
        let hit = items
            .iter()
            .find(|i| i.label == "fig-scree")
            .expect("the buffer's fig anchor should be offered");
        assert_eq!(
            hit.detail.as_deref(),
            Some("Figure 1"),
            "expected the rendered label+number as detail"
        );
        // The prefix stubs are offered too.
        assert!(
            items.iter().any(|i| i.label == "fig-"),
            "expected the `fig-` stub"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    #[test]
    fn completion_offers_citation_keys_from_the_bib() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-compcite-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("refs.bib"),
            "@article{smith2020,\n  title = {A Study}\n}\n@book{jones19,\n}\n",
        )
        .unwrap();
        let doc = dir.join("paper.tmd");
        let text = "---\nbibliography: refs.bib\n---\n\nSee [@\n".to_string();
        std::fs::write(&doc, &text).unwrap();

        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);
        let uri = Url::from_file_path(&doc).unwrap();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // Cursor right after `[@` on line 4.
        let items = complete_at(&client, &uri, 25, 4, 6);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.contains(&"smith2020") && labels.contains(&"jones19"),
            "expected both citation keys, got {labels:?}"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn code_action_offers_a_quick_fix_for_a_frontmatter_typo() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-fix.tmd").unwrap();
        let text = "---\ntittle: Hi\n---\n\nBody.\n".to_string();
        did_open(&client, &uri, text);
        let published = recv_publish(&client);
        // The front-matter typo diagnostic carries its "did you mean `title`" fix on `data`.
        let diag = published
            .diagnostics
            .iter()
            .find(|d| d.data.is_some())
            .expect("a diagnostic carrying a quick-fix")
            .clone();

        // Ask for code actions over that diagnostic's range, echoing it in the context (as a
        // real client does).
        let params = lsp_types::CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: diag.range,
            context: lsp_types::CodeActionContext {
                diagnostics: vec![diag.clone()],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(31),
                method: lsp_types::request::CodeActionRequest::METHOD.to_owned(),
                params: serde_json::to_value(params).unwrap(),
            }))
            .unwrap();
        let resp = recv_response(&client, RequestId::from(31));
        let actions: lsp_types::CodeActionResponse =
            serde_json::from_value(resp.result.expect("a code action result")).unwrap();
        assert_eq!(actions.len(), 1, "one quick-fix expected, got {actions:?}");
        match &actions[0] {
            lsp_types::CodeActionOrCommand::CodeAction(a) => {
                assert_eq!(a.title, "Change to `title`");
                assert_eq!(a.kind, Some(lsp_types::CodeActionKind::QUICKFIX));
                assert_eq!(a.is_preferred, Some(true));
                // The action carries back the diagnostic it fixes: that is what lets the
                // editor clear the squiggle when the fix is applied, and what scopes the
                // lightbulb to the offending token instead of the whole line.
                let carried = a
                    .diagnostics
                    .as_ref()
                    .expect("the quick-fix must name the diagnostic it resolves");
                assert_eq!(carried.len(), 1);
                assert_eq!(carried[0].range, diag.range);
                assert_eq!(carried[0].message, diag.message);
                let edits = a
                    .edit
                    .as_ref()
                    .and_then(|e| e.changes.as_ref())
                    .and_then(|c| c.get(&uri))
                    .expect("an edit for this document");
                assert_eq!(edits.len(), 1);
                assert_eq!(edits[0].new_text, "title");
                assert_eq!(edits[0].range, diag.range);
            }
            other => panic!("expected a CodeAction, got {other:?}"),
        }

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    #[test]
    fn rename_rewrites_an_anchor_definition_and_all_its_references() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-rename.tmd").unwrap();
        // Definition `{#fig-scree}` on line 0; two `@fig-scree` references on line 2.
        let text = "![p](i.png){#fig-scree}\n\nSee @fig-scree and @fig-scree.\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // prepareRename on the first reference (line 2, char 6) → the id-only range [5, 14).
        match prepare_rename_at(&client, &uri, 40, 2, 6).expect("an anchor is renameable here") {
            lsp_types::PrepareRenameResponse::Range(r) => {
                assert_eq!(r.start, lsp_types::Position::new(2, 5));
                assert_eq!(r.end, lsp_types::Position::new(2, 14));
            }
            other => panic!("expected a plain Range, got {other:?}"),
        }
        // A position on prose is not renameable.
        assert!(
            prepare_rename_at(&client, &uri, 41, 2, 0).is_none(),
            "prose should not be renameable"
        );

        // rename to `fig-plot` → the definition + both references, all rewritten.
        let edit = rename_at(&client, &uri, 42, 2, 6, "fig-plot").expect("a rename edit");
        let edits = edit
            .changes
            .as_ref()
            .and_then(|c| c.get(&uri))
            .expect("edits for this document");
        assert_eq!(edits.len(), 3, "definition + two references, got {edits:?}");
        assert!(
            edits.iter().all(|e| e.new_text == "fig-plot"),
            "every edit inserts the new id"
        );
        let ranges: Vec<(u32, u32, u32)> = edits
            .iter()
            .map(|e| {
                (
                    e.range.start.line,
                    e.range.start.character,
                    e.range.end.character,
                )
            })
            .collect();
        assert_eq!(
            ranges,
            vec![(0, 13, 22), (2, 5, 14), (2, 20, 29)],
            "the definition span then both reference spans"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    #[test]
    fn rename_refuses_a_new_name_that_is_not_an_anchor() {
        // `rename` is the ONE sanctioned write path back into source (the preview never
        // writes), so an unvalidated name corrupts the file the author is editing. Accepting
        // any non-blank string meant `F2` -> `my section` emitted `{#my section}` — not an
        // anchor at all, since `is_xref_id_char` stops at the space — and rewrote every
        // reference to match; a newline split the heading line in two. Refuse with a
        // ResponseError so the editor shows the reason in its rename box, and change nothing.
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-rename-invalid.tmd").unwrap();
        did_open(
            &client,
            &uri,
            "## Scree {#sec-scree}\n\nSee @sec-scree.\n".to_string(),
        );
        let _ = recv_publish(&client);

        // Each of these is a distinct way to leave the anchor grammar.
        for (n, bad) in ["my section", "sec scree", "sec\nscree", "sec#scree", "sé"]
            .iter()
            .enumerate()
        {
            let resp = rename_raw_at(&client, &uri, 60 + n as i32, 0, 12, bad);
            assert!(
                resp.result.is_none() || resp.result.as_ref() == Some(&serde_json::Value::Null),
                "rename to {bad:?} must not produce an edit, got {:?}",
                resp.result
            );
            let err = resp
                .error
                .unwrap_or_else(|| panic!("rename to {bad:?} must answer a ResponseError"));
            assert_eq!(err.code, -32803, "expected LSP RequestFailed for {bad:?}");
            assert!(
                err.message.contains("letters, digits"),
                "the message must state the grammar so the editor can show it: {}",
                err.message
            );
        }

        // A name that is grammatically fine but drops the kind prefix is refused too, and for
        // a different reason: `@intro` is not a cross-reference, so every reference would
        // silently degrade to prose rather than break visibly.
        let dropped = rename_raw_at(&client, &uri, 68, 0, 12, "intro");
        let err = dropped
            .error
            .expect("dropping the xref kind prefix must be refused");
        assert_eq!(err.code, -32803);
        assert!(
            err.message.contains("`sec-` prefix"),
            "the message should name the prefix read off the id being renamed: {}",
            err.message
        );

        // The valid neighbours still work, so the guard rejects only what it should.
        let ok = rename_at(&client, &uri, 70, 0, 12, "sec-scree-2").expect("a rename edit");
        assert_eq!(
            ok.changes.as_ref().and_then(|c| c.get(&uri)).unwrap().len(),
            2,
            "definition + reference"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    #[test]
    fn rename_leaves_the_fragment_of_an_external_url_alone() {
        // `is_anchor_site` treated ANY `#` before the id as a definition sigil, so renaming a
        // section silently retargeted outbound links: `[x](https://example.com/p.html#sec-a)`
        // became `…#sec-b`, a fragment on someone else's page. The mutation campaign measured
        // 29 mutants / 0 survivors here, which proved the implemented rule was faithfully
        // pinned — not that the rule was right. This is the fixture it never had.
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-rename-external.tmd").unwrap();
        let text = "## A {#sec-a}\n\
                    \n\
                    See @sec-a and [ours](#sec-a).\n\
                    \n\
                    [theirs](https://example.com/p.html#sec-a)\n"
            .to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        let edit = rename_at(&client, &uri, 80, 0, 9, "sec-b").expect("a rename edit");
        let edits = edit
            .changes
            .as_ref()
            .and_then(|c| c.get(&uri))
            .expect("edits for this document");
        let lines: Vec<u32> = edits.iter().map(|e| e.range.start.line).collect();
        assert!(
            !lines.contains(&4),
            "line 4 is an EXTERNAL url; its fragment is not ours to rewrite: {edits:?}"
        );
        // The definition, the `@` reference and the same-document `](#…)` link all move.
        assert_eq!(
            lines,
            vec![0, 2, 2],
            "definition + @ref + in-document link, and nothing else: {edits:?}"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    #[test]
    fn rename_speaks_utf16_columns_across_an_astral_char() {
        // The write path is the sharpest edge of the encoding boundary: an astral char (😀,
        // two UTF-16 units) before a reference shifts every column. The editor sends UTF-16
        // and expects UTF-16 back; a scalar-vs-UTF-16 mismatch would make the server miss the
        // anchor (wrong incoming column) or overwrite the wrong span (wrong outgoing range).
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-rename-astral.tmd").unwrap();
        // Def `{#fig-1}` on line 0 (ASCII, id chars/UTF-16 [13,18)); ref `@fig-1` on line 2
        // after two emojis, so the id sits at char [3,8) but UTF-16 [5,10).
        let text = "![p](i.png){#fig-1}\n\n😀😀@fig-1\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // Cursor at UTF-16 col 9 (the `1`). Read as a scalar column that is past the 8-scalar
        // line end, so without incoming conversion the server finds no anchor and returns null.
        let edit = rename_at(&client, &uri, 50, 2, 9, "fig-2")
            .expect("the anchor is found when the incoming UTF-16 column is converted");
        let edits = edit
            .changes
            .as_ref()
            .and_then(|c| c.get(&uri))
            .expect("edits for this document");
        let ranges: Vec<(u32, u32, u32)> = edits
            .iter()
            .map(|e| {
                (
                    e.range.start.line,
                    e.range.start.character,
                    e.range.end.character,
                )
            })
            .collect();
        assert_eq!(
            ranges,
            vec![(0, 13, 18), (2, 5, 10)],
            "the ASCII def span, then the ref span shifted by the two emojis' extra UTF-16 units"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    /// The initialize handshake is the *only* thing that tells an editor which features exist:
    /// an unadvertised capability is one the editor never asks for, so it silently does not
    /// exist. Every other test here throws the `InitializeResult` away (`handshake` does
    /// `let _ = recv()`), which is why all twelve mutants in `server_capabilities` survived the
    /// 2026-07-27 mutation run — including replacing its whole body with `Default::default()`.
    /// A server advertising *nothing* passed the entire suite.
    ///
    /// So assert the value that actually goes over the wire, field by field, since each deleted
    /// field is its own silent feature loss. `renameProvider` and `definitionProvider` are the
    /// load-bearing pair: they are click-to-source and its rename counterpart.
    #[test]
    fn the_initialize_handshake_advertises_every_feature_the_editor_needs() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));

        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(1),
                method: "initialize".to_owned(),
                params: serde_json::json!({ "capabilities": {} }),
            }))
            .unwrap();
        let result = match client.receiver.recv().unwrap() {
            Message::Response(Response {
                result: Some(v),
                error: None,
                ..
            }) => v,
            other => panic!("expected an InitializeResult, got {other:?}"),
        };
        let caps = &result["capabilities"];

        // The wire encoding `lsp_pos` converts at; advertised explicitly, not left implicit.
        assert_eq!(caps["positionEncoding"], "utf-16");
        // Whole-buffer sync, plus the open/close notifications the buffer cache is keyed on.
        assert_eq!(caps["textDocumentSync"]["openClose"], true);
        assert_eq!(
            caps["textDocumentSync"]["change"], 1,
            "TextDocumentSyncKind::FULL — incremental sync would desync the cached buffer"
        );
        assert_eq!(caps["definitionProvider"], true);
        assert_eq!(caps["documentSymbolProvider"], true);
        assert_eq!(
            caps["documentLinkProvider"]["resolveProvider"], false,
            "include/embed paths are the only visible cue that they are navigable"
        );
        assert_eq!(caps["hoverProvider"], true);
        assert_eq!(caps["codeActionProvider"], true);
        assert_eq!(
            caps["renameProvider"]["prepareProvider"], true,
            "without prepareRename the editor offers rename on anything, not just an anchor"
        );
        // `@` xref/cite, `.` div class, `|` cell option, `-` xref prefix, `/` path,
        // `:` front-matter value. A dropped trigger character is a completion that never opens.
        assert_eq!(
            caps["completionProvider"]["triggerCharacters"],
            serde_json::json!(["@", ".", "|", "-", "/", ":", "\\"])
        );

        client
            .sender
            .send(Message::Notification(Notification {
                method: "initialized".to_owned(),
                params: serde_json::json!({}),
            }))
            .unwrap();
        shutdown(&client);
        thread.join().unwrap().unwrap();
    }
}
