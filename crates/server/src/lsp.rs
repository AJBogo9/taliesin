//! The `taliesin lsp` subcommand: a synchronous, offline LSP server over stdio.
//!
//! **What:** holds open `.tmd` buffers warm and publishes live diagnostics (the
//! `check` validators) on every edit, to any LSP editor. Parse-only, kernel-free.
//!
//! **stdout is the JSON-RPC wire** — this module must never write to stdout; all
//! human output goes to `crate::log` (stderr). See the plan's Global Constraints.

use lsp_server::{Connection, Message};
use lsp_types::{
    CodeActionProviderCapability, CompletionOptions, HoverProviderCapability, OneOf, RenameOptions,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
};
use std::process::ExitCode;

/// Advertised capabilities: full-text document sync (whole buffer on every change, which maps
/// 1:1 onto `check`'s whole-buffer linting), go-to-definition, document symbols (the heading
/// outline), hover, completion, quick-fix code actions, and rename (with prepare) of a
/// cross-reference anchor + its references. This is the full E7 intelligence surface.
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
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        completion_provider: Some(CompletionOptions {
            // The chars that open a completable context: `@` (xref/cite), `.` (div class),
            // `|` (cell option), `-` (xref prefix), `/` (path), `:` (front-matter value).
            trigger_characters: Some(
                ["@", ".", "|", "-", "/", ":"]
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
    use lsp_types::request::{
        CodeActionRequest, Completion, DocumentSymbolRequest, GotoDefinition, HoverRequest,
        PrepareRenameRequest, Rename, Request as _,
    };
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
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(resolve_rename(docs, &params))?),
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
    let markup = |value: String, start: usize, end: usize| {
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(Range::new(
                Position::new(pos.line, start as u32),
                Position::new(pos.line, end as u32),
            )),
        })
    };

    match crate::lsp_nav::classify_target(text, pos.line as usize, pos.character as usize) {
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
        Target::Include { .. } | Target::None => None,
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
    let (_, start, end) =
        crate::lsp_nav::anchor_at(text, pos.line as usize, pos.character as usize)?;
    Some(PrepareRenameResponse::Range(Range::new(
        Position::new(pos.line, start as u32),
        Position::new(pos.line, end as u32),
    )))
}

/// `textDocument/rename`: rename the cross-reference anchor under the cursor — its definition
/// (`{#id}` / `#| label: id`) and every `@id` reference in this document — to `new_name`, as one
/// `WorkspaceEdit`. `None` when the cursor is on no anchor or `new_name` is empty. The edit flows
/// through the editor (the legitimate editing surface), never the preview.
fn resolve_rename(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    params: &lsp_types::RenameParams,
) -> Option<lsp_types::WorkspaceEdit> {
    use lsp_types::{Position, Range, TextEdit, WorkspaceEdit};
    let uri = &params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let new_name = params.new_name.trim();
    if new_name.is_empty() {
        return None;
    }
    let text = docs.get(uri)?;
    let (id, _, _) = crate::lsp_nav::anchor_at(text, pos.line as usize, pos.character as usize)?;
    let edits: Vec<TextEdit> = crate::lsp_nav::anchor_occurrences(text, &id)
        .into_iter()
        .map(|(line, start, end)| TextEdit {
            range: Range::new(Position::new(line, start), Position::new(line, end)),
            new_text: new_name.to_string(),
        })
        .collect();
    if edits.is_empty() {
        return None;
    }
    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), edits);
    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}

/// Resolve completion at the cursor: route to the vocabulary that applies (front-matter key /
/// value, cell option, div class, xref, cite, shortcode path) and emit its items. `None` when
/// the cursor is in no completable context. A port of the companion's `completions.ts`,
/// drawing on the same Rust-authoritative `vocab` + live document scans.
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
    let lines: Vec<&str> = text.split('\n').collect();
    let line = lines.get(pos.line as usize).copied().unwrap_or("");
    let line_prefix: String = line.chars().take(pos.character as usize).collect();
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
            // cleanly rather than appending to a half-typed segment.
            let typed_len = typed.chars().count() as u32;
            let replace = Range::new(
                Position::new(pos.line, pos.character.saturating_sub(typed_len)),
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
    crate::serve::guarded(|| {
        taliesin_core::render_document_with_includes_rooted(text, &base, Some(&base))
    })
    .ok()
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

    fn hover_params(uri: &Url, line: u32, character: u32) -> lsp_types::HoverParams {
        lsp_types::HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: lsp_types::Position::new(line, character),
            },
            work_done_progress_params: Default::default(),
        }
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
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(id),
                method: lsp_types::request::HoverRequest::METHOD.to_owned(),
                params: serde_json::to_value(hover_params(uri, line, character)).unwrap(),
            }))
            .unwrap();
        let resp = recv_response(client, RequestId::from(id));
        serde_json::from_value(resp.result.expect("a hover result (got null)")).unwrap()
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

    // Send a completion request and return the item list.
    fn complete_at(
        client: &Connection,
        uri: &Url,
        id: i32,
        line: u32,
        character: u32,
    ) -> Vec<lsp_types::CompletionItem> {
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(id),
                method: lsp_types::request::Completion::METHOD.to_owned(),
                params: serde_json::to_value(complete_params(uri, line, character)).unwrap(),
            }))
            .unwrap();
        let resp = recv_response(client, RequestId::from(id));
        match serde_json::from_value(resp.result.expect("a completion result")).unwrap() {
            lsp_types::CompletionResponse::Array(items) => items,
            lsp_types::CompletionResponse::List(l) => l.items,
        }
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
        assert!(
            md.contains("title"),
            "expected the key's documentation, got {md:?}"
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
        assert!(
            labels.contains(&"echo"),
            "expected an `execute:` child key, got {labels:?}"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
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
}
