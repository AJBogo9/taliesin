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
        // Ctrl+T across the whole book. `documentSymbol` is per-file, which for a 25-chapter
        // project means the author has to already know which file holds the heading they
        // want. Answered by a project walk, not an index: the request fires on a gesture, not
        // on every keystroke, so re-validating with one `stat` per page is cheaper than the
        // file watching an index would need.
        workspace_symbol_provider: Some(OneOf::Left(true)),
        // Range-scoped, so only the visible lines are scanned and there is no full-document
        // tokenizer behind this.
        inlay_hint_provider: Some(OneOf::Left(true)),
        // The anchor under the cursor and its other occurrences: a targeted single-id scan,
        // not a full-document tokenizer.
        document_highlight_provider: Some(OneOf::Left(true)),
        // Expand-selection by document structure: word out to the enclosing section.
        selection_range_provider: Some(lsp_types::SelectionRangeProviderCapability::Simple(true)),
        // Replaces indentation-based folding, which is what `.tmd` gets without this and is
        // meaningless in a format where nesting is heading level and fences.
        folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
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
        // Pipe tables only — see `lsp_format`. Advertising this claims "Format Document" for
        // `.tmd`, which is a promise worth being narrow about: a formatter that re-wrapped
        // prose would fight every deliberate line break in the file.
        document_formatting_provider: Some(OneOf::Left(true)),
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
/// Is the editor on a dark colour scheme? Decides the ink of a rasterized math hover, which
/// is the one answer this server gives that cannot adapt to the reader: text in a hover
/// inherits the theme, an image does not.
///
/// Defaults to dark, matching both VS Code's default theme and Taliesin's own. The client
/// sets it at `initialize` and updates it with `taliesin/colorScheme` when the user switches,
/// so a hover never renders black ink onto a dark popup.
static DARK_SCHEME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub(crate) fn dark_scheme() -> bool {
    DARK_SCHEME.load(std::sync::atomic::Ordering::Relaxed)
}

/// Read `{ "colorScheme": "dark" | "light" }` out of a client's payload, leaving the current
/// setting alone when the key is absent or unrecognized — an editor that says nothing must not
/// be treated as having said "light".
fn absorb_color_scheme(payload: Option<&serde_json::Value>) {
    let Some(scheme) = payload
        .and_then(|v| v.get("colorScheme"))
        .and_then(|v| v.as_str())
    else {
        return;
    };
    match scheme {
        "dark" => DARK_SCHEME.store(true, std::sync::atomic::Ordering::Relaxed),
        "light" => DARK_SCHEME.store(false, std::sync::atomic::Ordering::Relaxed),
        other => crate::log::warn(&format!("lsp: ignoring unknown colorScheme {other:?}")),
    }
}

/// How long `didChange` edits are coalesced before diagnostics are published.
///
/// `publish` runs a full render **plus** `site::anchors_defined_elsewhere_in_project`, which
/// walks every page in the project, reads each from disk and resolves its includes — so
/// undebounced, one keystroke cost a whole-book pass. 120 ms is below the threshold at which
/// an author notices a lag in their squiggles and well above a fast typist's inter-key
/// interval, so a burst of typing collapses to one pass.
const DEFAULT_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(120);

pub(crate) fn run(connection: Connection) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_with_debounce(connection, DEFAULT_DEBOUNCE)
}

/// `run` with the coalescing window as a parameter, so a test can pick one it can wait on
/// without either flaking or sleeping for a real editor's interval.
fn run_with_debounce(
    connection: Connection,
    debounce: std::time::Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let caps = serde_json::to_value(server_capabilities())?;
    let initialize_params = connection.initialize(caps)?;
    absorb_color_scheme(initialize_params.get("initializationOptions"));
    main_loop(&connection, debounce)?;
    Ok(())
}

/// Diagnostics that are owed but not yet published, and when their coalescing window closes.
///
/// Two decisions live here. **Repeated edits to one buffer collapse** — that is the whole
/// point — but **an edit to a second buffer does not evict the first**: a single slot would
/// drop A's diagnostics silently the moment B was touched inside the window, and they would
/// not reappear until A was edited again.
///
/// And the deadline is set by the *edit*, not refreshed by every message that arrives. A
/// client that polls (inlay hints on scroll, hovers as the pointer moves) would otherwise
/// push the window out indefinitely and starve the publish it is waiting for.
#[derive(Default)]
struct PendingPublishes {
    uris: Vec<lsp_types::Url>,
    deadline: Option<std::time::Instant>,
}

impl PendingPublishes {
    /// Record that `uri`'s diagnostics are owed, and (re)start the coalescing window.
    fn owe(&mut self, uri: lsp_types::Url, debounce: std::time::Duration) {
        if !self.uris.contains(&uri) {
            self.uris.push(uri);
        }
        self.deadline = Some(std::time::Instant::now() + debounce);
    }

    /// Drop any debt owed to `uri`, so closing a document does not publish diagnostics for a
    /// buffer that is gone.
    fn forget(&mut self, uri: &lsp_types::Url) {
        self.uris.retain(|u| u != uri);
        if self.uris.is_empty() {
            self.deadline = None;
        }
    }

    /// How long to wait for the next message before the window closes, or `None` when nothing
    /// is owed and the loop should simply block.
    fn wait(&self) -> Option<std::time::Duration> {
        self.deadline
            .map(|d| d.saturating_duration_since(std::time::Instant::now()))
    }

    /// Take everything owed, closing the window.
    fn take(&mut self) -> Vec<lsp_types::Url> {
        self.deadline = None;
        std::mem::take(&mut self.uris)
    }
}

/// Read messages until `shutdown`/`exit`. Text-document notifications keep the open-buffer
/// store current and drive diagnostics; requests (other than shutdown) are answered from
/// that store.
fn main_loop(
    connection: &Connection,
    debounce: std::time::Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Open taliesin documents, by URI → current buffer text. `didChange` carries no
    // languageId, so we only track what `didOpen` admitted; a request between edits reads
    // the buffer text from here.
    let mut docs: std::collections::HashMap<lsp_types::Url, String> =
        std::collections::HashMap::new();
    let mut pending = PendingPublishes::default();
    // Shared by every request that needs the rendered document (hover's cross-reference
    // number, inlay hints). Keyed on the buffer text, so it is a hit for as long as the
    // author is reading rather than typing — which is exactly when these fire in bursts.
    let mut memo = crate::lsp_memo::RenderMemo::default();
    // Shared by every request that reaches past the open buffer (cross-file definition and
    // hover, workspace symbols, the sidebar's two views). A stat-validated walk, not an index:
    // all of those fire on a user gesture, so re-validating with one `stat` per page is
    // cheaper than the file watching an index would need. See `lsp_project`.
    let mut project = crate::lsp_project::ProjectCache::new();
    loop {
        // Block outright when nothing is owed, so an idle server costs nothing; wait only as
        // long as the open window when a publish is pending.
        let msg = match pending.wait() {
            None => match connection.receiver.recv() {
                Ok(m) => m,
                Err(_) => break,
            },
            Some(remaining) => match connection.receiver.recv_timeout(remaining) {
                Ok(m) => m,
                // The window closed with no further edit: publish the latest text of every
                // buffer that is owed.
                Err(e) if e.is_timeout() => {
                    for uri in pending.take() {
                        if let Some(text) = docs.get(&uri) {
                            publish(connection, &uri, text)?;
                        }
                    }
                    continue;
                }
                Err(_) => break,
            },
        };
        match msg {
            Message::Request(req) => {
                // Once `shutdown` arrives the session is over, and the exit code is a
                // statement about how it ended. `handle_shutdown` replies, then waits up to
                // 30s for the `exit` notification, and returns `Err` if anything *else*
                // shows up in that window or if the wait times out — a protocol nit from a
                // client that is already tearing us down. Propagating that `Err` reached
                // `cmd_lsp`, which exited **1**, and an editor reads a non-zero exit from
                // its language server as a crash: VS Code counts it toward the "server
                // crashed 5 times" cutoff that stops restarting it. A clean `exit` after
                // `shutdown` exited 0, so whether the author's editor believed Taliesin had
                // crashed came down to a race on the client's side.
                if req.method
                    == <lsp_types::request::Shutdown as lsp_types::request::Request>::METHOD
                {
                    if let Err(e) = connection.handle_shutdown(&req) {
                        crate::log::warn(&format!("lsp: {e}"));
                    }
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
                let failure = match crate::serve::guarded(|| {
                    handle_request(connection, &docs, &mut memo, &mut project, req)
                }) {
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
                match crate::serve::guarded(|| {
                    handle_notification(connection, &mut docs, &mut pending, debounce, notif)
                }) {
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
    pending: &mut PendingPublishes,
    debounce: std::time::Duration,
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
            // Coalesced rather than published here: the main loop publishes once the edits
            // stop. Publishing on every keystroke re-walked the whole project each time
            // (backlog item 178). `didOpen` above still publishes immediately — opening a
            // document is a single event, not a burst, and waiting would only delay the
            // first squiggles an author sees.
            pending.owe(uri, debounce);
        }
    } else if method == DidCloseTextDocument::METHOD {
        let p: DidCloseTextDocumentParams = serde_json::from_value(notif.params)?;
        docs.remove(&p.text_document.uri);
        // Before the clear below, or a window that closes after this would re-publish
        // diagnostics for a buffer that is gone.
        pending.forget(&p.text_document.uri);
        publish_diagnostics(connection, &p.text_document.uri, Vec::new())?;
    } else if method == COLOR_SCHEME_METHOD {
        // The editor switched theme. Only the rasterized math hover cares, and it caches by
        // scheme, so this costs one re-render per expression per scheme and never a stale
        // image. Nothing to publish: hovers are pulled, not pushed.
        absorb_color_scheme(Some(&notif.params));
    }
    Ok(())
}

/// The client tells us the editor's colour scheme here, at `initialize` and whenever the user
/// switches. Custom because LSP has no concept of a theme: it assumes every answer is text
/// the editor will style itself, which stopped being true when the math hover became a
/// picture.
pub(crate) const COLOR_SCHEME_METHOD: &str = "taliesin/colorScheme";

/// Answer a request from the open-buffer store. Only `textDocument/definition` is handled;
/// any other request gets a `MethodNotFound` reply so the client never hangs waiting.
fn handle_request(
    connection: &Connection,
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    memo: &mut crate::lsp_memo::RenderMemo,
    project: &mut crate::lsp_project::ProjectCache,
    req: lsp_server::Request,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use lsp_types::request::{
        CodeActionRequest, Completion, DocumentHighlightRequest, DocumentLinkRequest,
        DocumentSymbolRequest, FoldingRangeRequest, Formatting, GotoDefinition, HoverRequest,
        InlayHintRequest, PrepareRenameRequest, Rename, Request as _, SelectionRangeRequest,
        WorkspaceSymbolRequest,
    };
    #[cfg(test)]
    assert!(req.method != PANIC_PROBE_METHOD, "injected request panic");
    let response = if req.method == InlayHintRequest::METHOD {
        let params: lsp_types::InlayHintParams = serde_json::from_value(req.params)?;
        let uri = &params.text_document.uri;
        // An unopened document, or one that cannot be rendered, is an empty result rather
        // than an error: a half-typed buffer is the normal case for a provider that fires on
        // every scroll.
        // Citations and includes resolve against files beside the document, so the hints
        // need its directory. An unsaved buffer has no path and simply gets fewer hints.
        let file = uri.to_file_path().ok();
        let dir = file.as_deref().and_then(std::path::Path::parent);
        let hints = docs
            .get(uri)
            .and_then(|text| {
                memo.get(uri, text)
                    .map(|doc| crate::lsp_hints::inlay_hints(text, &doc, params.range, dir))
            })
            .unwrap_or_default();
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(hints)?),
            error: None,
        }
    } else if req.method == SelectionRangeRequest::METHOD {
        let params: lsp_types::SelectionRangeParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(resolve_selection_ranges(
                docs, &params,
            ))?),
            error: None,
        }
    } else if req.method == DocumentHighlightRequest::METHOD {
        let params: lsp_types::DocumentHighlightParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(resolve_document_highlight(
                docs, &params,
            ))?),
            error: None,
        }
    } else if req.method == FoldingRangeRequest::METHOD {
        let params: lsp_types::FoldingRangeParams = serde_json::from_value(req.params)?;
        let folds = docs
            .get(&params.text_document.uri)
            .map(|text| crate::lsp_fold::folding_ranges(text))
            .unwrap_or_default();
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(folds)?),
            error: None,
        }
    } else if req.method == GotoDefinition::METHOD {
        let params: lsp_types::GotoDefinitionParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(resolve_definition(
                docs, project, &params,
            ))?),
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
            result: Some(serde_json::to_value(resolve_hover(docs, project, &params))?),
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
    } else if req.method == Formatting::METHOD {
        let params: lsp_types::DocumentFormattingParams = serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(format_document(
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
    } else if req.method == WorkspaceSymbolRequest::METHOD {
        let params: lsp_types::WorkspaceSymbolParams = serde_json::from_value(req.params)?;
        // The request names no file, so the projects are discovered from the open documents.
        // **Every** distinct project, not one of them: an editor routinely holds files from
        // two projects at once, and picking whichever document came first out of a hash map
        // makes Ctrl+T search an arbitrary one of them. An empty editor answers an empty
        // list, which is correct rather than an error.
        let open: Vec<std::path::PathBuf> =
            docs.keys().filter_map(|u| u.to_file_path().ok()).collect();
        let mut found = Vec::new();
        for root in crate::lsp_project::ProjectCache::roots_of(open.iter().map(|p| p.as_path())) {
            // Any page under the root locates it; the scan is keyed on the root itself.
            found.extend(workspace_symbols(
                project,
                &root.join("_site.yml"),
                &params.query,
            ));
        }
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(found)?),
            error: None,
        }
    } else if req.method == PROJECT_OUTLINE_METHOD || req.method == PROJECT_REFS_METHOD {
        // The sidebar's two views. Both take a document uri and answer about its enclosing
        // project, so they share one arm rather than duplicating the parse and the fallback.
        #[derive(serde::Deserialize)]
        struct ProjectParams {
            uri: lsp_types::Url,
        }
        let outline = req.method == PROJECT_OUTLINE_METHOD;
        let params: ProjectParams = serde_json::from_value(req.params)?;
        let answer = params.uri.to_file_path().ok().and_then(|p| {
            if outline {
                project_outline(project, &p)
            } else {
                project_refs(project, &p)
            }
        });
        lsp_server::Response {
            id: req.id,
            // `null` rather than an error for a document outside any project: the sidebar
            // renders an empty view, which is the honest answer for a standalone document.
            result: Some(answer.unwrap_or(serde_json::Value::Null)),
            error: None,
        }
    } else if req.method == CELL_REGIONS_METHOD {
        // A Taliesin extension, not an LSP method: the protocol has no concept of "this
        // range is another language, go ask whoever owns it". A client that wants embedded
        // intelligence needs the regions, and deriving them from a fence scan of its own is
        // how the TypeScript copy this branch deleted got started.
        let params: lsp_types::DocumentSymbolParams = serde_json::from_value(req.params)?;
        let regions = docs
            .get(&params.text_document.uri)
            .map(|text| crate::lsp_cells::cell_regions(text))
            .unwrap_or_default();
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(regions)?),
            error: None,
        }
    } else if req.method == SECTION_EDIT_METHOD {
        // Also a Taliesin extension rather than an LSP method, for the same reason
        // rust-analyzer's "move item up/down" is one: the operation needs the *cursor*, and
        // the two standard homes for an editor-triggered edit cannot carry it. A code action
        // would put a lightbulb on every section (the author asks for this by keystroke, not
        // by browsing a menu), and `workspace/executeCommand`'s client-side forwarder is
        // invoked by a keybinding with no arguments at all.
        let params: crate::lsp_edits::SectionEditParams = serde_json::from_value(req.params)?;
        match docs.get(&params.text_document.uri) {
            Some(text) => match crate::lsp_edits::section_edit(text, params.position, params.op) {
                Ok(edit) => lsp_server::Response {
                    id: req.id,
                    result: Some(serde_json::to_value(edit)?),
                    error: None,
                },
                // A refusal is a first-class answer here — "this is the last section under
                // its parent" is information, not a failure — and the companion shows
                // `message` to the author. `null` would read as "nothing happened".
                Err(message) => lsp_server::Response {
                    id: req.id,
                    result: None,
                    error: Some(lsp_server::ResponseError {
                        code: -32803, // JSON-RPC RequestFailed
                        message,
                        data: None,
                    }),
                },
            },
            None => lsp_server::Response {
                id: req.id,
                result: None,
                error: Some(lsp_server::ResponseError {
                    code: -32803,
                    message: format!("{} is not open on the server", params.text_document.uri),
                    data: None,
                }),
            },
        }
    } else if req.method == INSERT_EDIT_METHOD {
        // Also a Taliesin extension rather than an LSP method. The protocol has no concept of
        // "the author pasted an image, what should the document say", and it could not: the
        // gesture is the client's (only it has the clipboard) while the answer is this crate's
        // vocabulary. Splitting it any other way puts a figure shape, a pipe table or a
        // citation key in TypeScript, free to disagree with the renderer.
        let params: crate::lsp_insert::InsertEditParams = serde_json::from_value(req.params)?;
        let text = docs
            .get(&params.text_document.uri)
            .cloned()
            .unwrap_or_default();
        let result = params
            .text_document
            .uri
            .to_file_path()
            .map_err(|()| format!("{} is not a file", params.text_document.uri))
            .and_then(|path| {
                crate::lsp_insert::insert_edit(&path, &text, params.kind, &params.payload)
            });
        match result {
            Ok(edit) => lsp_server::Response {
                id: req.id,
                result: Some(serde_json::to_value(edit)?),
                error: None,
            },
            // A refusal is a first-class answer here, exactly as for `sectionEdit`: "that is
            // not a table" and "unsupported image type" are information the author should see,
            // and the client shows `message`. A null result would read as "nothing happened".
            Err(message) => lsp_server::Response {
                id: req.id,
                result: None,
                error: Some(lsp_server::ResponseError {
                    code: -32803, // JSON-RPC RequestFailed
                    message,
                    data: None,
                }),
            },
        }
    } else if req.method == RENAME_FILE_EDITS_METHOD {
        // No refusal path: an empty list is the correct answer for "nothing to repair", and a
        // rename must never fail because the repair found nothing to do.
        let params: crate::lsp_rename_file::RenameFileEditsParams =
            serde_json::from_value(req.params)?;
        lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(
                crate::lsp_rename_file::rename_file_edits(&params),
            )?),
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
/// One nested `SelectionRange` per requested position: word → inline construct → paragraph →
/// `:::` div → section, each the `parent` of the one below it.
///
/// A position with no chain still gets an entry, collapsed to the position itself: the
/// response array is positional, so dropping one would silently shift every later cursor's
/// answer onto the wrong cursor.
fn resolve_selection_ranges(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    params: &lsp_types::SelectionRangeParams,
) -> Vec<lsp_types::SelectionRange> {
    use lsp_types::{Position, Range, SelectionRange};

    let text = docs
        .get(&params.text_document.uri)
        .map(String::as_str)
        .unwrap_or("");
    params
        .positions
        .iter()
        .map(|pos| {
            let line = crate::lsp_pos::nth_line(text, pos.line as usize);
            let cursor_char = crate::lsp_pos::utf16_to_char(line, pos.character as usize);
            let chain = crate::lsp_nav::selection_chain(text, pos.line as usize, cursor_char);
            // Built outermost-in, so each level can take the previous as its parent.
            let mut parent: Option<Box<SelectionRange>> = None;
            for &(sl, sc, el, ec) in chain.iter().rev() {
                let (sline, eline) = (
                    crate::lsp_pos::nth_line(text, sl as usize),
                    crate::lsp_pos::nth_line(text, el as usize),
                );
                parent = Some(Box::new(SelectionRange {
                    range: Range::new(
                        Position::new(sl, crate::lsp_pos::char_to_utf16(sline, sc as usize) as u32),
                        Position::new(el, crate::lsp_pos::char_to_utf16(eline, ec as usize) as u32),
                    ),
                    parent,
                }));
            }
            *parent.unwrap_or_else(|| {
                Box::new(SelectionRange {
                    range: Range::new(*pos, *pos),
                    parent: None,
                })
            })
        })
        .collect()
}

/// Every occurrence of the cross-reference anchor under the cursor, the definition marked
/// `WRITE` and the references `READ`. Empty when the cursor is not on an anchor.
///
/// Scalar columns from `lsp_nav` are converted back to UTF-16 here, at the boundary, exactly
/// as `resolve_definition` does.
fn resolve_document_highlight(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    params: &lsp_types::DocumentHighlightParams,
) -> Vec<lsp_types::DocumentHighlight> {
    use lsp_types::{DocumentHighlight, DocumentHighlightKind, Position, Range};

    let uri = &params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    let Some(text) = docs.get(uri) else {
        return Vec::new();
    };
    let cursor_char = crate::lsp_pos::utf16_to_char(
        crate::lsp_pos::nth_line(text, pos.line as usize),
        pos.character as usize,
    );
    crate::lsp_nav::anchor_highlights(text, pos.line as usize, cursor_char)
        .into_iter()
        .map(|(line, start, end, is_def)| {
            let l = crate::lsp_pos::nth_line(text, line as usize);
            DocumentHighlight {
                range: Range::new(
                    Position::new(
                        line,
                        crate::lsp_pos::char_to_utf16(l, start as usize) as u32,
                    ),
                    Position::new(line, crate::lsp_pos::char_to_utf16(l, end as usize) as u32),
                ),
                kind: Some(match is_def {
                    true => DocumentHighlightKind::WRITE,
                    false => DocumentHighlightKind::READ,
                }),
            }
        })
        .collect()
}

fn resolve_definition(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    project: &mut crate::lsp_project::ProjectCache,
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
        // `{{< include x.tmd#sec-y >}}` → the anchored **section's heading line**, which
        // is the whole point of naming one: landing at 0:0 of a shared parts file and
        // hunting for the section is the navigation this feature exists to remove.
        Target::Include { path, .. } => {
            let (rel, fragment) = taliesin_core::includes::split_target(&path);
            let dir = uri.to_file_path().ok()?;
            let abs = dir.parent()?.join(rel);
            if !abs.exists() {
                return None;
            }
            let line = fragment
                .and_then(|id| {
                    let body = std::fs::read_to_string(&abs).ok()?;
                    taliesin_core::includes::section_lines(&body, id).map(|(start, _)| start)
                })
                .unwrap_or(0) as u32;
            Location::new(Url::from_file_path(&abs).ok()?, point("", line, 0, 0))
        }
        // `@fig-x` → its definition. The open buffer wins: it is ahead of the on-disk copy,
        // and an unsaved anchor must not send the author to yesterday's file. Only when the
        // buffer does not define it does the project walk answer, which is what closes the
        // cross-file half of the gap this function's doc comment names.
        Target::Xref { id, .. } => match crate::lsp_nav::definition_site(text, &id) {
            Some((line, col)) => Location::new(
                uri.clone(),
                point(text, line, col, col + id.chars().count() as u32),
            ),
            None => {
                let here = uri.to_file_path().ok()?;
                let anchor = project.get(&here)?.anchors.iter().find(|a| a.id == id)?;
                // The target's own text, so the outgoing range is built against the file the
                // author lands in rather than the one they came from.
                let body = std::fs::read_to_string(&anchor.path).ok()?;
                let target = Url::from_file_path(&anchor.path).ok()?;
                Location::new(target, point(&body, anchor.line, 0, 0))
            }
        },
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
        // Math has no definition site to jump to; it is a hover-only target.
        Target::Math { .. } | Target::FrontmatterKey { .. } | Target::None => return None,
    };
    Some(lsp_types::GotoDefinitionResponse::Scalar(location))
}

/// Every heading and cross-reference anchor in `page`'s project whose name contains `query`,
/// case-insensitively. An empty query returns everything, because that is the state Ctrl+T
/// opens in and an empty list there reads as "this project has no symbols".
///
/// Ranking is deliberately absent: VS Code applies its own fuzzy sort to whatever comes back,
/// and a second ranking here would fight it. Outside a project the answer is an empty list,
/// not an error, which is the honest answer for a standalone document.
fn workspace_symbols(
    project: &mut crate::lsp_project::ProjectCache,
    page: &std::path::Path,
    query: &str,
) -> Vec<lsp_types::SymbolInformation> {
    use lsp_types::{Location, Position, Range, SymbolKind, Url};
    let needle = query.to_lowercase();
    let Some(scan) = project.get(page) else {
        return Vec::new();
    };
    // No empty-query special case is needed: `contains("")` is true for every string, so an
    // empty Ctrl+T query already returns everything. An explicit `needle.is_empty() ||` here
    // was redundant, and a mutation run proved it by surviving its own deletion.
    let matches = |name: &str| name.to_lowercase().contains(&needle);
    let at = |path: &std::path::Path, line: u32| {
        Url::from_file_path(path).ok().map(|uri| {
            Location::new(
                uri,
                Range::new(Position::new(line, 0), Position::new(line, 0)),
            )
        })
    };

    let mut out = Vec::new();
    for h in &scan.headings {
        if matches(&h.text)
            && let Some(location) = at(&h.path, h.line)
        {
            out.push(symbol(h.text.clone(), SymbolKind::MODULE, location, None));
        }
    }
    for a in &scan.anchors {
        if matches(&a.id)
            && let Some(location) = at(&a.path, a.line)
        {
            // The heading an anchor sits on, when it has one: `fig-scree` alone says nothing
            // about which section it belongs to.
            let container = (!a.title.is_empty()).then(|| a.title.clone());
            out.push(symbol(a.id.clone(), SymbolKind::KEY, location, container));
        }
    }
    out
}

/// `SymbolInformation` is deprecated in `lsp-types` in favour of `WorkspaceSymbol`, but the
/// struct literal still has to be spelled out and its deprecated field set. Kept in one helper
/// so the `#[allow]` sits in exactly one place instead of at every construction site.
#[allow(deprecated)]
fn symbol(
    name: String,
    kind: lsp_types::SymbolKind,
    location: lsp_types::Location,
    container_name: Option<String>,
) -> lsp_types::SymbolInformation {
    lsp_types::SymbolInformation {
        name,
        kind,
        tags: None,
        deprecated: None,
        location,
        container_name,
    }
}

/// The custom request a client calls to learn where a document's code cells are, so it can
/// route completion inside one to whoever owns that language. Namespaced, because it is not
/// an LSP method and must never collide with one.
pub(crate) const CELL_REGIONS_METHOD: &str = "taliesin/cellRegions";

/// The custom request behind the companion's four structural commands (move a section up or
/// down, promote or demote a heading). Namespaced for the same reason as
/// [`CELL_REGIONS_METHOD`]: it is not an LSP method.
pub(crate) const SECTION_EDIT_METHOD: &str = "taliesin/sectionEdit";

/// The custom request behind the companion's paste and drop gestures. Namespaced for the same
/// reason as [`SECTION_EDIT_METHOD`]: it is not an LSP method, and it cannot be one. A paste is
/// a client event (only the client has the clipboard) whose *answer* is this crate's vocabulary,
/// so the request carries the gesture in and the text out.
pub(crate) const INSERT_EDIT_METHOD: &str = "taliesin/insertEdit";

/// The custom request behind the companion's rename repair. Namespaced for the same reason as
/// [`SECTION_EDIT_METHOD`]. LSP's `workspace/willRenameFiles` is close, but the companion needs
/// this on its own `onWillRenameFiles` hook so the edits land inside VS Code's rename
/// transaction, and the *knowledge* (which reference spellings exist, where a `_site.yml` scalar
/// sits) is this crate's either way.
pub(crate) const RENAME_FILE_EDITS_METHOD: &str = "taliesin/renameFileEdits";

/// The custom request behind the sidebar's whole-book Outline and Figures views. Namespaced
/// for the same reason as [`CELL_REGIONS_METHOD`]: it is not an LSP method. `workspace/symbol`
/// is the closest standard method and deliberately answers a *flat, queried* list, which is
/// the wrong shape for a tree the author browses.
pub(crate) const PROJECT_OUTLINE_METHOD: &str = "taliesin/projectOutline";

/// The custom request behind the sidebar's References view: every cross-reference target with
/// the uses pointing at it, dangling ones included. Namespaced for the same reason.
pub(crate) const PROJECT_REFS_METHOD: &str = "taliesin/projectRefs";

/// The whole-book outline plus the numbered-float index, as the sidebar's TreeViews want it:
/// grouped by page and in reading order, not flattened. `None` outside a project, which the
/// client renders as an empty view.
fn project_outline(
    project: &mut crate::lsp_project::ProjectCache,
    page: &std::path::Path,
) -> Option<serde_json::Value> {
    let scan = project.get(page)?;
    let mut pages: Vec<serde_json::Value> = Vec::new();
    for h in &scan.headings {
        let path = h.path.to_string_lossy().into_owned();
        let row = serde_json::json!({ "line": h.line, "level": h.level, "text": h.text });
        match pages
            .iter_mut()
            .find(|p| p["path"].as_str() == Some(path.as_str()))
        {
            Some(p) => p["headings"].as_array_mut()?.push(row),
            None => pages.push(serde_json::json!({ "path": path, "headings": [row] })),
        }
    }
    let floats: Vec<serde_json::Value> = scan
        .anchors
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "path": a.path.to_string_lossy(),
                "line": a.line,
                "title": a.title,
                "number": a.number,
            })
        })
        .collect();
    Some(serde_json::json!({
        "root": scan.root.to_string_lossy(),
        "pages": pages,
        "floats": floats,
    }))
}

/// Every cross-reference target with the uses pointing at it. A target with no definition is
/// reported with `resolved: false` rather than omitted: grouping dangling references is the
/// reason [`crate::lsp_nav::xref_occurrences`] exists at all. A target that is defined and
/// never referenced is `resolved: true` with an empty `uses`, which is normal rather than a
/// problem and must not be filed as dangling.
fn project_refs(
    project: &mut crate::lsp_project::ProjectCache,
    page: &std::path::Path,
) -> Option<serde_json::Value> {
    let scan = project.get(page)?;
    let mut ids: Vec<&str> = scan.anchors.iter().map(|a| a.id.as_str()).collect();
    for u in &scan.uses {
        ids.push(&u.id);
    }
    ids.sort_unstable();
    ids.dedup();

    let targets: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| {
            let defined = scan.anchors.iter().find(|a| a.id == *id);
            let uses: Vec<serde_json::Value> = scan
                .uses
                .iter()
                .filter(|u| u.id == *id)
                .map(|u| {
                    serde_json::json!({
                        "path": u.path.to_string_lossy(),
                        "line": u.line,
                        "col": u.col,
                    })
                })
                .collect();
            serde_json::json!({
                "id": id,
                "resolved": defined.is_some(),
                "definedIn": defined.map(|d| d.path.to_string_lossy().into_owned()),
                "definedLine": defined.map(|d| d.line),
                "uses": uses,
            })
        })
        .collect();
    Some(serde_json::json!({ "root": scan.root.to_string_lossy(), "targets": targets }))
}

/// Resolve hover for the token under the cursor: an xref's rendered label + number, a
/// front-matter key's documentation, or a citation's BibTeX entry. `None` when the token
/// resolves to nothing (an unknown xref, an undocumented key, a missing/absent `.bib` entry)
/// or is not a hoverable kind (an include path is go-to-definition only, mirroring the
/// companion). Markdown content, ranged to the token so the editor highlights it.
fn resolve_hover(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    project: &mut crate::lsp_project::ProjectCache,
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
            let label = xref_label(&id)?;
            match xref_number(uri, text, &id) {
                Some(number) => markup(format!("**{label} {number}** — `@{id}`"), start, end),
                // Defined on another page. The *number* belongs to that page's render, which
                // this kernel-free path does not have, so name the page instead of answering
                // nothing: "which chapter is this in" is the question a cross-page hover is
                // actually asked.
                None => {
                    let here = uri.to_file_path().ok()?;
                    let anchor = project.get(&here)?.anchors.iter().find(|a| a.id == id)?;
                    let page = anchor
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let head = if anchor.number.is_empty() {
                        format!("**{label}** `@{id}`")
                    } else {
                        format!("**{label} {}** `@{id}`", anchor.number)
                    };
                    markup(format!("{head}\n\nDefined in `{page}`"), start, end)
                }
            }
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
            let (rel, _fragment) = taliesin_core::includes::split_target(&path);
            let dir = uri.to_file_path().ok()?.parent()?.to_path_buf();
            let target = dir.join(rel);
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
        // `$…$` → what it renders as. KaTeX is in this binary and memoized, so the preview
        // is the SAME engine the reader's page goes through, not a second interpretation of
        // the source. Math KaTeX cannot parse gets no hover: the expression is already
        // squiggled as a diagnostic, and a preview of a broken parse would contradict it.
        Target::Math {
            latex,
            display,
            start_line,
            start_char,
            end_line,
            end_char,
        } => {
            // Prefer the real thing: a rasterized render of the SAME KaTeX output the
            // document gets (`math_image`). It is unavailable in a build without the browser
            // driver, on a host without Chrome, or on a timeout — and in every one of those
            // cases the Unicode approximation still answers, so the hover never goes blank.
            let body = match crate::math_image::data_uri(&latex, display, dark_scheme()) {
                Some(uri) => format!("![{}]({uri})", alt_text(&latex)),
                None => {
                    let preview = taliesin_core::math_preview::unicode_preview(&latex, display)?;
                    if preview.trim().is_empty() {
                        return None;
                    }
                    format!("### {preview}")
                }
            };
            let kind = if display { "Display" } else { "Inline" };
            let to_pos = |l: usize, c: usize| {
                let lt = crate::lsp_pos::nth_line(text, l);
                Position::new(l as u32, crate::lsp_pos::char_to_utf16(lt, c) as u32)
            };
            Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("{body}\n\n{kind} math"),
                }),
                range: Some(Range::new(
                    to_pos(start_line, start_char),
                    to_pos(end_line, end_char),
                )),
            })
        }
        Target::None => None,
    }
}

/// Alt text for a rasterized math hover: the expression's own source, minus the two
/// characters that would close the markdown image early and leave a data URI spilled into the
/// popup as text. It is what a screen reader announces and what shows if the image is ever
/// dropped, so the source is the most useful thing it can carry.
fn alt_text(latex: &str) -> String {
    latex.replace(['[', ']'], "").replace(['\n', '\r'], " ")
}

/// Do two ranges touch? Used to scope code actions to the range the editor asked about.
/// Touching at a boundary counts: a cursor sitting immediately after a mis-typed token is
/// a zero-width range whose start equals the token's end, and refusing it there would make
/// the lightbulb flicker off at the one position an author is most likely to be in.
fn ranges_intersect(a: &lsp_types::Range, b: &lsp_types::Range) -> bool {
    a.start <= b.end && b.start <= a.end
}

/// Build quick-fix code actions from the diagnostics the client echoed back. For each that
/// carries a `data.replacement` — a precise "did you mean" fix `to_lsp` attaches only when the
/// diagnostic's `range` is exactly the mis-typed token — emit a `QuickFix` that replaces that
/// range with the correction. Read-only w.r.t. the preview; the edit flows through the editor.
///
/// Two filters, and both were missing. **Ours only:** `context.diagnostics` is whatever the
/// editor decided to echo, from every provider attached to the buffer — so a diagnostic from
/// a spell checker or an embedded-language server that happened to carry a `data.replacement`
/// key would have been turned into a Taliesin quick fix that rewrote the buffer using
/// somebody else's range. **In range:** the request carries the range the editor is asking
/// about, and ignoring it offered every fix in the file at every cursor position.
fn resolve_code_actions(
    params: &lsp_types::CodeActionParams,
) -> Option<lsp_types::CodeActionResponse> {
    use lsp_types::{CodeAction, CodeActionKind, CodeActionOrCommand, TextEdit, WorkspaceEdit};
    let uri = &params.text_document.uri;
    let mut actions = Vec::new();
    for diag in &params.context.diagnostics {
        // The same `source` `Diagnostic::to_lsp` stamps. Anything else is not ours to fix.
        if diag.source.as_deref() != Some(crate::check::LSP_SOURCE) {
            continue;
        }
        if !ranges_intersect(&diag.range, &params.range) {
            continue;
        }
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
        Ctx::DivAttrKey { classes, typed } => {
            // A class the renderer dispatches on. `layout-ncol` (the one attribute with an
            // empty class list) is offered ONLY where none is present: the dispatch chain
            // tests it second, so on a `.step` or `.panel-tabset` it does not decorate the
            // feature, it silently REPLACES it with a grid. That is a footgun, not a
            // completion.
            let names_in = |list: &str, c: &str| {
                vocab[list]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|e| e["name"].as_str() == Some(c)))
            };
            let is_feature_class = |c: &str| {
                names_in("divClasses", c)
                    || names_in("theoremKinds", c)
                    || c == "columns"
                    || c == "column"
                    || vocab["calloutKinds"].as_array().is_some_and(|a| {
                        a.iter()
                            .filter_map(|e| e["name"].as_str())
                            .any(|k| format!("callout-{k}") == c)
                    })
            };
            let generic = !classes.iter().any(|c| is_feature_class(c));
            vocab["divAttributes"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|e| {
                            let name = e["name"].as_str()?;
                            if !typed.is_empty() && !name.starts_with(&typed) {
                                return None;
                            }
                            let allowed = e["classes"].as_array()?;
                            let offered = if allowed.is_empty() {
                                generic
                            } else {
                                allowed
                                    .iter()
                                    .filter_map(|v| v.as_str())
                                    .any(|c| classes.iter().any(|t| t == c))
                            };
                            if !offered {
                                return None;
                            }
                            Some(CompletionItem {
                                label: name.to_string(),
                                kind: Some(CompletionItemKind::PROPERTY),
                                detail: e["description"].as_str().map(str::to_string),
                                // The snippet carries the `="…"` and, where the value set is
                                // closed, a choice — so `appearance` completes to a value the
                                // renderer recognizes rather than to an empty pair of quotes.
                                insert_text: e["snippet"].as_str().map(str::to_string),
                                insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                                ..Default::default()
                            })
                        })
                        .collect()
                })
                .unwrap_or_default()
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
                            let description = e["description"].as_str().unwrap_or("");
                            // Strip the backslash from BOTH sides, so one rule serves the
                            // control sequence and the bare token: `\alp` and `alp` are the
                            // same query. An empty core (a lone `\`) matches everything,
                            // which is the point of triggering on the backslash.
                            let core = typed.strip_prefix('\\').unwrap_or(typed.as_str());
                            let by_name = name.strip_prefix('\\').unwrap_or(name).starts_with(core);
                            // The vocabulary carries each symbol's glyph as its description,
                            // so the glyph is a query too. Withheld for a single ASCII
                            // character, which would match half the list on a substring.
                            let selective = core.chars().count() >= 2 || !core.is_ascii();
                            let by_glyph = selective
                                && description.to_lowercase().contains(&core.to_lowercase());
                            if !by_name && !by_glyph {
                                return None;
                            }
                            let snippet = e["snippet"].as_str().unwrap_or("");
                            let insert = if snippet.is_empty() { name } else { snippet };
                            let category = e["category"].as_str().unwrap_or("");
                            Some(CompletionItem {
                                label: name.to_string(),
                                kind: Some(CompletionItemKind::FUNCTION),
                                detail: Some(format!("{description}  ·  {category}")),
                                // The client re-filters this list against the text in the
                                // edit range, so an item the SERVER matched can still be
                                // dropped by the EDITOR — which looks exactly like the server
                                // never answering. Leading with what was actually typed makes
                                // every returned item a prefix match, and the name and glyph
                                // that follow keep it matching as more characters arrive.
                                filter_text: Some(format!("{typed} {name} {description}")),
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
pub(crate) fn render_buffer(
    uri: &lsp_types::Url,
    text: &str,
) -> Option<taliesin_core::RenderedDoc> {
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
            // A `#fragment` names a section inside the file, so the FILE is what the link
            // opens; leaving it on the path resolves nothing and the link disappears,
            // which is the affordance `documentLink` exists to provide.
            let (rel, _fragment) = taliesin_core::includes::split_target(&l.path);
            let target = dir.join(rel);
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
/// `textDocument/formatting`: one edit per table whose formatting changes, and nothing else.
///
/// Each edit spans whole lines and ends at the start of the line AFTER the table, so the
/// trailing newline is never part of the replacement — an off-by-one there would eat the
/// blank line under every table it touched.
fn format_document(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    uri: &lsp_types::Url,
) -> Option<Vec<lsp_types::TextEdit>> {
    use lsp_types::{Position, Range, TextEdit};
    let text = docs.get(uri)?;
    Some(
        crate::lsp_format::format_edits(text)
            .into_iter()
            .map(|e| TextEdit {
                range: Range {
                    start: Position {
                        line: e.start_line as u32,
                        character: 0,
                    },
                    end: Position {
                        line: e.end_line as u32,
                        character: line_len_utf16(text, e.end_line),
                    },
                },
                new_text: e.new_text,
            })
            .collect(),
    )
}

/// The length of a line in UTF-16 code units, which is what LSP positions count.
fn line_len_utf16(text: &str, line: usize) -> u32 {
    crate::lsp_pos::line_end_utf16(crate::lsp_pos::nth_line(text, line)) as u32
}

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
    // Column 0 needs no conversion; the end-of-line column is a UTF-16 unit count, and a
    // CRLF buffer's `\r` is not part of the line (`line_end_utf16`).
    let line_len = |i: usize| {
        lines
            .get(i)
            .map_or(0, |l| crate::lsp_pos::line_end_utf16(l)) as u32
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

    /// A two-page project on disk plus the open-buffer map the resolvers read, for the
    /// cross-file tests below. `here_text` is what the editor holds for `one.tmd`, which is
    /// deliberately allowed to differ from what is written to disk: an unsaved buffer being
    /// ahead of its file is the case `a_local_definition_still_wins_over_the_project_walk`
    /// exists for.
    fn cross_page_fixture(
        name: &str,
        other: &str,
        here_text: &str,
    ) -> (
        std::path::PathBuf,
        Url,
        std::collections::HashMap<Url, String>,
    ) {
        let root =
            std::env::temp_dir().join(format!("tali-lsp-xpage-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::write(root.join("two.tmd"), other).unwrap();
        let here = root.join("one.tmd");
        std::fs::write(&here, here_text).unwrap();
        let uri = Url::from_file_path(&here).unwrap();
        let mut docs = std::collections::HashMap::new();
        docs.insert(uri.clone(), here_text.to_string());
        (root, uri, docs)
    }

    /// A project root with the given `(relative path, source)` pages, for the workspace-symbol
    /// tests. Returns the root.
    fn symbol_fixture(name: &str, pages: &[(&str, &str)]) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("tali-lsp-wsym-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        for (rel, src) in pages {
            let p = root.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(p, src).unwrap();
        }
        root
    }

    #[test]
    fn project_outline_lists_every_page_with_its_headings_and_floats() {
        let root = symbol_fixture(
            "outline",
            &[("index.tmd", "# One\n\n## Deeper\n\n![p](i.png){#fig-a}\n")],
        );
        let mut project = crate::lsp_project::ProjectCache::new();
        let out = project_outline(&mut project, &root.join("index.tmd")).unwrap();

        assert_eq!(out["pages"].as_array().unwrap().len(), 1);
        let headings = out["pages"][0]["headings"].as_array().unwrap();
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[1]["text"], "Deeper");
        assert_eq!(headings[1]["level"], 2);
        assert_eq!(out["floats"][0]["id"], "fig-a");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_outline_keeps_each_pages_headings_under_that_page() {
        // Two pages, so a flattening bug (every heading landing under the first page) cannot
        // pass. A single-page fixture would not notice.
        let root = symbol_fixture(
            "outline2",
            &[("a.tmd", "# A\n"), ("b.tmd", "# B\n\n## B2\n")],
        );
        let mut project = crate::lsp_project::ProjectCache::new();
        let out = project_outline(&mut project, &root.join("a.tmd")).unwrap();
        let pages = out["pages"].as_array().unwrap();
        assert_eq!(pages.len(), 2, "one row per page: {pages:?}");
        for p in pages {
            let n = p["headings"].as_array().unwrap().len();
            let expected = if p["path"].as_str().unwrap().ends_with("a.tmd") {
                1
            } else {
                2
            };
            assert_eq!(n, expected, "wrong heading count for {}", p["path"]);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_refs_groups_uses_by_target_and_flags_the_dangling_one() {
        let root = symbol_fixture(
            "refs",
            &[
                ("a.tmd", "# A {#sec-a}\n"),
                ("b.tmd", "See @sec-a and @sec-gone.\n"),
            ],
        );
        let mut project = crate::lsp_project::ProjectCache::new();
        let refs = project_refs(&mut project, &root.join("b.tmd")).unwrap();

        let targets = refs["targets"].as_array().unwrap();
        let resolved = targets.iter().find(|t| t["id"] == "sec-a").unwrap();
        assert_eq!(resolved["resolved"], true);
        assert_eq!(resolved["uses"].as_array().unwrap().len(), 1);

        let dangling = targets.iter().find(|t| t["id"] == "sec-gone").unwrap();
        assert_eq!(dangling["resolved"], false);
        assert!(dangling["definedIn"].is_null());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_refs_lists_a_defined_but_unreferenced_anchor_as_resolved_with_no_uses() {
        // Defined and never used is normal, not an error, and must not be filed as dangling.
        let root = symbol_fixture("refs2", &[("a.tmd", "# A {#sec-unused}\n")]);
        let mut project = crate::lsp_project::ProjectCache::new();
        let refs = project_refs(&mut project, &root.join("a.tmd")).unwrap();
        let t = &refs["targets"][0];
        assert_eq!(t["id"], "sec-unused");
        assert_eq!(t["resolved"], true);
        assert_eq!(t["uses"].as_array().unwrap().len(), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn both_project_requests_answer_none_outside_a_project() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-proj-solo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let solo = dir.join("solo.tmd");
        std::fs::write(&solo, "# Solo\n\nSee @sec-x.\n").unwrap();
        let mut project = crate::lsp_project::ProjectCache::new();
        assert!(project_outline(&mut project, &solo).is_none());
        assert!(project_refs(&mut project, &solo).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_two_new_project_methods_are_namespaced() {
        // The `"taliesin/…"` census below enforces the shape across the whole file; these two
        // are pinned by name so a rename cannot quietly drop the prefix.
        assert!(PROJECT_OUTLINE_METHOD.starts_with("taliesin/"));
        assert!(PROJECT_REFS_METHOD.starts_with("taliesin/"));
    }

    #[test]
    fn workspace_symbols_reach_headings_and_anchors_on_every_page() {
        let root = symbol_fixture(
            "reach",
            &[
                ("index.tmd", "# Introduction\n"),
                ("two.tmd", "# Scree Plots\n\n![p](i.png){#fig-scree}\n"),
            ],
        );
        let mut project = crate::lsp_project::ProjectCache::new();

        let hits = workspace_symbols(&mut project, &root.join("index.tmd"), "scree");
        let names: Vec<&str> = hits.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"Scree Plots"),
            "heading on another page: {names:?}"
        );
        assert!(
            names.contains(&"fig-scree"),
            "anchor on another page: {names:?}"
        );
        assert!(
            !names.contains(&"Introduction"),
            "non-matching symbol leaked in: {names:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_symbols_search_every_open_project_not_an_arbitrary_one() {
        // Found by the e2e suite, which runs with documents from earlier tests still open: the
        // handler picked `docs.keys().find_map(...)`, the FIRST key of a hash map, to stand for
        // "the project". With two projects open that is a coin flip, and Ctrl+T searched
        // whichever one the hasher happened to yield. An author cannot predict that.
        let a = symbol_fixture("multi-a", &[("index.tmd", "# Alpha Heading\n")]);
        let b = symbol_fixture("multi-b", &[("index.tmd", "# Alpha Sibling\n")]);
        let mut project = crate::lsp_project::ProjectCache::new();

        let roots = crate::lsp_project::ProjectCache::roots_of(
            [a.join("index.tmd"), b.join("index.tmd")]
                .iter()
                .map(|p| p.as_path()),
        );
        assert_eq!(roots.len(), 2, "two distinct projects: {roots:?}");

        let mut names: Vec<String> = Vec::new();
        for root in roots {
            names.extend(
                workspace_symbols(&mut project, &root.join("_site.yml"), "Alpha")
                    .into_iter()
                    .map(|s| s.name),
            );
        }
        names.sort();
        assert_eq!(
            names,
            vec!["Alpha Heading".to_string(), "Alpha Sibling".to_string()],
            "both projects must be searched"
        );
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn a_root_locates_its_own_project_so_the_handler_need_not_pick_a_page() {
        // The handler passes `<root>/_site.yml` as the probe path. That file need not be a
        // page, but it must resolve to the project, or workspace symbols answers nothing.
        let root = symbol_fixture("byroot", &[("index.tmd", "# Findable\n")]);
        let mut project = crate::lsp_project::ProjectCache::new();
        assert_eq!(
            workspace_symbols(&mut project, &root.join("_site.yml"), "Findable").len(),
            1
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_symbol_matching_ignores_case() {
        let root = symbol_fixture("case", &[("index.tmd", "# Introduction\n")]);
        let mut project = crate::lsp_project::ProjectCache::new();
        assert_eq!(
            workspace_symbols(&mut project, &root.join("index.tmd"), "INTRO").len(),
            1
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_empty_query_returns_every_symbol_rather_than_none() {
        // VS Code opens Ctrl+T with an empty query and expects a browsable list, not silence.
        let root = symbol_fixture("empty", &[("index.tmd", "# A\n\n## B\n")]);
        let mut project = crate::lsp_project::ProjectCache::new();
        assert_eq!(
            workspace_symbols(&mut project, &root.join("index.tmd"), "").len(),
            2
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_workspace_symbol_points_at_the_line_that_defines_it() {
        // A location every row shares would make the list navigable-looking and useless.
        let root = symbol_fixture("locate", &[("index.tmd", "# A\n\n## Target\n")]);
        let mut project = crate::lsp_project::ProjectCache::new();
        let hits = workspace_symbols(&mut project, &root.join("index.tmd"), "Target");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].location.range.start.line, 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_symbols_outside_a_project_are_empty_rather_than_an_error() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-wsym-solo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let solo = dir.join("solo.tmd");
        std::fs::write(&solo, "# Solo\n").unwrap();
        let mut project = crate::lsp_project::ProjectCache::new();
        assert!(workspace_symbols(&mut project, &solo, "solo").is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_capabilities_advertise_workspace_symbols() {
        let caps = serde_json::to_value(server_capabilities()).unwrap();
        assert_eq!(caps["workspaceSymbolProvider"], true);
    }

    #[test]
    fn go_to_definition_resolves_an_xref_defined_on_another_page() {
        // The gap `resolve_definition` documented for a year: "cross-file refs get nothing".
        let (root, uri, docs) =
            cross_page_fixture("goto", "# Two {#sec-two}\n", "# One\n\nSee @sec-two.\n");
        let mut project = crate::lsp_project::ProjectCache::new();

        let found = resolve_definition(&docs, &mut project, &goto_params(&uri, 2, 6))
            .expect("a cross-page xref must resolve");
        let lsp_types::GotoDefinitionResponse::Scalar(loc) = found else {
            panic!("expected a single location");
        };
        assert!(loc.uri.to_file_path().unwrap().ends_with("two.tmd"));
        assert_eq!(loc.range.start.line, 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_local_definition_still_wins_over_the_project_walk() {
        // The project scan reads DISK; the buffer is ahead of it. Jumping to the on-disk copy
        // of the page you are typing in is worse than useless, so the buffer must win. The
        // fixture makes the two disagree on purpose: `one.tmd` on disk has no anchor at all,
        // while the open buffer defines `sec-x`, and `two.tmd` also defines it.
        let (root, uri, docs) = cross_page_fixture(
            "local",
            "# Other {#sec-x}\n",
            "# Local {#sec-x}\n\nSee @sec-x.\n",
        );
        std::fs::write(root.join("one.tmd"), "# Local\n\nSee @sec-x.\n").unwrap();
        let mut project = crate::lsp_project::ProjectCache::new();

        let lsp_types::GotoDefinitionResponse::Scalar(loc) =
            resolve_definition(&docs, &mut project, &goto_params(&uri, 2, 6)).unwrap()
        else {
            panic!("expected a single location");
        };
        assert_eq!(loc.uri, uri, "the buffer's own definition must win");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_xref_defined_nowhere_still_resolves_to_nothing() {
        let (root, uri, docs) = cross_page_fixture("nowhere", "# Two\n", "See @sec-nowhere.\n");
        let mut project = crate::lsp_project::ProjectCache::new();
        assert!(resolve_definition(&docs, &mut project, &goto_params(&uri, 0, 6)).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn hover_on_a_cross_page_xref_names_the_page_that_defines_it() {
        let (root, uri, docs) =
            cross_page_fixture("hover", "# Two {#sec-two}\n", "See @sec-two.\n");
        let mut project = crate::lsp_project::ProjectCache::new();

        let hover = resolve_hover(&docs, &mut project, &hover_params(&uri, 0, 6))
            .expect("a cross-page xref must hover");
        let lsp_types::HoverContents::Markup(m) = hover.contents else {
            panic!("expected markup");
        };
        assert!(
            m.value.contains("two.tmd"),
            "the hover must name the page, since the number lives in that page's render: {:?}",
            m.value
        );
        let _ = std::fs::remove_dir_all(&root);
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

    /// Positions in a CRLF buffer stop at the last visible character, not one past it.
    ///
    /// Every position the server computes comes from a `\n` split, which on a CRLF buffer
    /// leaves a `\r` on the end of each line. An editor treats CRLF as one terminator, so
    /// that `\r` is not a column a cursor can occupy — but it was counted, and every
    /// end-of-line column the server emitted ran one long. Windows-authored `.tmd` files
    /// and anything through a CRLF-normalizing tool hit this on every symbol, every folding
    /// range, and every whole-line diagnostic squiggle.
    #[test]
    fn crlf_buffers_do_not_run_one_column_long() {
        let uri = Url::parse("file:///tmp/tali-lsp-crlf.tmd").unwrap();
        let heading = "# Intro"; // 7 visible characters
        let body = "Prose.";
        // The SAME document, once with each terminator. Their symbol ranges must agree:
        // the line terminator is not content.
        let lf = format!("{heading}\n\n{body}\n");
        let crlf = lf.replace('\n', "\r\n");

        let symbols = |text: &str| {
            let mut docs = std::collections::HashMap::new();
            docs.insert(uri.clone(), text.to_string());
            match document_symbols(&docs, &uri).expect("an outline") {
                lsp_types::DocumentSymbolResponse::Nested(v) => v,
                other => panic!("expected nested symbols, got {other:?}"),
            }
        };
        let lf_syms = symbols(&lf);
        let crlf_syms = symbols(&crlf);
        assert_eq!(lf_syms.len(), 1, "one heading");
        assert_eq!(
            crlf_syms[0].selection_range, lf_syms[0].selection_range,
            "the heading's own range must not depend on the line terminator"
        );
        assert_eq!(
            crlf_syms[0].selection_range.end.character,
            heading.chars().count() as u32,
            "the heading range ends at the last visible character"
        );
        assert_eq!(
            crlf_syms[0].range, lf_syms[0].range,
            "the section range must not depend on the line terminator either"
        );

        // The same measurement, on the path folding ranges and formatting edits use.
        assert_eq!(
            line_len_utf16(&crlf, 0),
            heading.chars().count() as u32,
            "line_len_utf16 must not count the CR"
        );

        // And a whole-line diagnostic squiggle, which shares the defect through `to_lsp`.
        let lines: Vec<&str> = crlf.split('\n').collect();
        // 1-based line 2 is the blank line between the heading and the prose: with the CR
        // counted it spanned one column of nothing.
        let blank = crate::check::Diagnostic::new("d.tmd".into(), Some(2), "x".into());
        assert_eq!(
            blank.to_lsp(&lines).range.end.character,
            0,
            "an empty CRLF line spans nothing, not one column"
        );
        // 1-based line 1 is the heading: it spans its visible text, not text + CR.
        let on_heading = crate::check::Diagnostic::new("d.tmd".into(), Some(1), "x".into());
        assert_eq!(
            on_heading.to_lsp(&lines).range.end.character,
            heading.chars().count() as u32,
            "a whole-line squiggle stops at the last visible character"
        );
    }

    /// A quick fix is built only from OUR diagnostics, and only for the range the editor
    /// asked about.
    ///
    /// `context.diagnostics` is whatever the client chose to echo, across every provider
    /// attached to the buffer. Building an edit from a foreign diagnostic means applying
    /// *our* replacement text at *their* range — in a `.tmd` that is a Pylance diagnostic
    /// inside a `{python}` cell, whose range is real and whose `data` is not ours to read.
    /// And `params.range` was ignored outright, so every fix in the file was offered at
    /// every cursor position.
    #[test]
    fn code_actions_are_scoped_to_our_diagnostics_and_the_requested_range() {
        use lsp_types::{Position, Range};
        let uri = Url::parse("file:///tmp/tali-lsp-ca-scope.tmd").unwrap();
        let ours = |line: u32| lsp_types::Diagnostic {
            range: Range::new(Position::new(line, 0), Position::new(line, 5)),
            source: Some(crate::check::LSP_SOURCE.to_string()),
            message: "unknown front-matter key `titel`".to_string(),
            data: Some(serde_json::json!({ "replacement": "title" })),
            ..Default::default()
        };
        let ask = |range: Range, diags: Vec<lsp_types::Diagnostic>| {
            resolve_code_actions(&lsp_types::CodeActionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range,
                context: lsp_types::CodeActionContext {
                    diagnostics: diags,
                    only: None,
                    trigger_kind: None,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap_or_default()
        };
        let cursor =
            |line: u32, ch: u32| Range::new(Position::new(line, ch), Position::new(line, ch));

        // The control: our diagnostic, cursor inside it — still one fix, as before.
        assert_eq!(
            ask(cursor(0, 2), vec![ours(0)]).len(),
            1,
            "the fix still works"
        );
        // Touching the token's far edge counts: that is where a cursor lands after typing it.
        assert_eq!(
            ask(cursor(0, 5), vec![ours(0)]).len(),
            1,
            "end boundary is inclusive"
        );

        // Cursor on a different line: nothing on offer.
        assert!(
            ask(cursor(9, 0), vec![ours(0)]).is_empty(),
            "a fix for line 0 must not be offered on line 9"
        );

        // Another provider's diagnostic, at the cursor, carrying a `replacement` key of its
        // own. Not ours, so not ours to fix.
        let theirs = lsp_types::Diagnostic {
            source: Some("Pylance".to_string()),
            ..ours(0)
        };
        assert!(
            ask(cursor(0, 2), vec![theirs.clone()]).is_empty(),
            "another provider's diagnostic must not become a Taliesin quick fix"
        );
        // A diagnostic with no source at all is equally not ours.
        let anonymous = lsp_types::Diagnostic {
            source: None,
            ..ours(0)
        };
        assert!(
            ask(cursor(0, 2), vec![anonymous]).is_empty(),
            "an unattributed diagnostic must not become a Taliesin quick fix"
        );
        // Mixed: only ours survives.
        assert_eq!(ask(cursor(0, 2), vec![theirs, ours(0)]).len(), 1);
    }

    /// A message arriving between `shutdown` and `exit` must not look like a crash.
    ///
    /// `handle_shutdown` errors on anything that is not `exit` in that window, and that
    /// error used to propagate out to `cmd_lsp`, which exited **1**. An editor reads a
    /// non-zero exit from its language server as a crash and counts it toward the restart
    /// cutoff — so a client that sends one last `didChange` while tearing down could get
    /// Taliesin's LSP marked as crashing, from a completely clean session.
    #[test]
    fn a_message_after_shutdown_still_ends_the_session_cleanly() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(77),
                method: "shutdown".to_owned(),
                params: serde_json::Value::Null,
            }))
            .unwrap();
        match client.receiver.recv().unwrap() {
            Message::Response(Response { id, .. }) => assert_eq!(id, RequestId::from(77)),
            other => panic!("expected the shutdown response, got {other:?}"),
        }
        // Not `exit`: a straggler notification, which is exactly what a racing client sends.
        client
            .sender
            .send(Message::Notification(Notification {
                method: "textDocument/didChange".to_owned(),
                params: serde_json::json!({}),
            }))
            .unwrap();
        drop(client);

        thread
            .join()
            .unwrap()
            .expect("a straggler after shutdown is a protocol nit, not a failed session");
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

    /// Item 160. A fragment include names a section, so go-to-definition lands on that
    /// section's heading line. Landing at 0:0 of a shared parts file and making the author
    /// hunt for the section is exactly the navigation the feature exists to remove — and
    /// it is the *silent* failure here, because 0:0 is a real location the editor opens
    /// happily. The `documentLink` half is checked with it: leaving the `#sec-…` on the
    /// path makes the file unresolvable and the link simply vanishes.
    #[test]
    fn goto_definition_on_a_fragment_include_lands_on_the_section() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-frag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("part.tmd");
        // `## Two` is line index 6 (0-based), which is the number under test: a naive
        // implementation reports 0 and an off-by-one reports 5 or 7.
        std::fs::write(
            &part,
            "# All {#sec-all}\n\n## One {#sec-one}\n\nbody\n\n## Two {#sec-two}\n\nmore\n",
        )
        .unwrap();
        let doc = dir.join("book.tmd");
        let text = "{{< include part.tmd#sec-two >}}\n\n{{< include part.tmd#sec-gone >}}\n\
                    \n{{< include part.tmd >}}\n"
            .to_string();
        std::fs::write(&doc, &text).unwrap();

        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);
        let uri = Url::from_file_path(&doc).unwrap();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        match definition_at(&client, &uri, 61, 0, 14) {
            Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) => {
                assert_eq!(loc.uri, Url::from_file_path(&part).unwrap());
                assert_eq!(
                    loc.range.start,
                    lsp_types::Position::new(6, 0),
                    "`## Two {{#sec-two}}` is line 6 of part.tmd; line 0 means the fragment \
                     was ignored, which opens the right file at the wrong place"
                );
            }
            other => panic!("expected a location into part.tmd, got {other:?}"),
        }
        // A fragment that names no section still opens the file — the file is real, only
        // the section is not, and `check` reports the bad anchor separately.
        match definition_at(&client, &uri, 62, 2, 14) {
            Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) => {
                assert_eq!(loc.range.start, lsp_types::Position::new(0, 0));
            }
            other => panic!("expected the file itself, got {other:?}"),
        }
        // The control: no fragment at all is unchanged by any of this.
        match definition_at(&client, &uri, 63, 4, 14) {
            Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) => {
                assert_eq!(loc.range.start, lsp_types::Position::new(0, 0));
            }
            other => panic!("expected the file itself, got {other:?}"),
        }

        // `documentLink` must still offer the file, with the fragment stripped only for
        // resolution. Three includes are written, and only the `#sec-gone` one is still a
        // real file, so all three must produce a link: a filter that kept the fragment on
        // the path would resolve none of them and the affordance would silently vanish.
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(64),
                method: lsp_types::request::DocumentLinkRequest::METHOD.to_owned(),
                params: serde_json::json!({ "textDocument": { "uri": uri } }),
            }))
            .unwrap();
        let resp = recv_response(&client, RequestId::from(64));
        let links: Vec<lsp_types::DocumentLink> =
            serde_json::from_value(resp.result.expect("a documentLink result")).unwrap();
        assert_eq!(
            links.len(),
            3,
            "every include here points at a file that exists: {links:?}"
        );
        assert!(
            links.iter().all(|l| l
                .target
                .as_ref()
                .is_some_and(|t| t.as_str().ends_with("part.tmd"))),
            "each link opens the FILE, fragment stripped: {links:?}"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A method the server does not implement is answered with JSON-RPC MethodNotFound. The
    // code is the whole contract here: a client reads it to decide between "this server
    // can't do that" and a real failure, and no test looked at it.
    //
    // The probe is RANGE formatting, which is deliberately not implemented: the formatter
    // rewrites whole tables and nothing else, so a request scoped to an arbitrary range has
    // no honest answer. (It used to probe `textDocument/formatting`, which stopped being an
    // unimplemented method the day the table formatter landed — a probe naming a real feature
    // tests the wrong thing, so this one has to keep naming a method the server declines.)
    #[test]
    fn an_unhandled_request_is_answered_with_method_not_found() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(43),
                method: lsp_types::request::RangeFormatting::METHOD.to_owned(),
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
            err.message
                .contains(lsp_types::request::RangeFormatting::METHOD),
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

    // The client needs to know where cells are to forward completion into them, and that
    // knowledge stays here rather than becoming a fence scanner in TypeScript.
    #[test]
    fn cell_regions_request_reports_each_cell_body() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-cells.tmd").unwrap();
        let text = "intro\n\n```{python}\nimport os\nos.getcwd()\n```\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(31),
                method: "taliesin/cellRegions".to_owned(),
                params: serde_json::json!({ "textDocument": { "uri": uri } }),
            }))
            .unwrap();
        let resp = recv_response(&client, RequestId::from(31));
        let regions = resp.result.expect("a cellRegions result");

        assert_eq!(
            regions,
            serde_json::json!([{
                "language": "python",
                "startLine": 3,
                "endLine": 4,
                // The editor hangs its Run buttons off this, so it is part of the wire
                // contract, not an internal detail.
                "executable": true,
            }]),
            "got {regions}"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    // A rasterized hover cannot inherit the popup's colour, so getting this wrong is not a
    // cosmetic slip: it is light ink on a light theme, i.e. an empty-looking hover. Silence
    // must therefore mean "keep what we have", never "assume light".
    #[test]
    fn the_colour_scheme_is_absorbed_only_when_the_client_actually_states_it() {
        let restore = dark_scheme();

        absorb_color_scheme(Some(&serde_json::json!({ "colorScheme": "light" })));
        assert!(!dark_scheme(), "an explicit light scheme is taken");
        absorb_color_scheme(Some(&serde_json::json!({ "colorScheme": "dark" })));
        assert!(dark_scheme(), "and so is an explicit dark one");

        // Every shape of "said nothing" leaves the setting alone.
        for quiet in [
            serde_json::json!({}),
            serde_json::json!({ "colorScheme": "chartreuse" }),
            serde_json::json!({ "colorScheme": 3 }),
        ] {
            absorb_color_scheme(Some(&quiet));
            assert!(
                dark_scheme(),
                "unrecognized payload must not flip it: {quiet}"
            );
        }
        absorb_color_scheme(None);
        assert!(
            dark_scheme(),
            "a client that sends no options must not flip it"
        );

        DARK_SCHEME.store(restore, std::sync::atomic::Ordering::Relaxed);
    }

    #[test]
    fn hover_on_math_previews_what_it_renders_as() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-hover-math.tmd").unwrap();
        let text = "Let $\\alpha + \\beta$ stand.\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // The span opens at char 4; char 8 is inside `\alpha`.
        let h = hover_at(&client, &uri, 11, 0, 8);
        let md = hover_markdown(&h);
        // The hover shows the math, never a description of it — but "the math" has two
        // legitimate spellings, and which one arrives depends on the build and the host:
        // a real rasterized render where a browser is available, the Unicode approximation
        // everywhere else. Asserting only the second would have gone red the moment the
        // image path started working, which is the wrong direction for a regression to fire.
        if md.starts_with("![") {
            assert!(
                md.contains("](data:image/png;base64,iVBORw0KGgo"),
                "an image hover must carry a real PNG data URI: {md:.120?}"
            );
            assert!(
                md.contains("![\\alpha + \\beta]"),
                "the alt text must be the source, so it survives a dropped image: {md:.120?}"
            );
        } else {
            assert!(
                md.contains("α+β"),
                "expected the rendered glyphs, got {md:?}"
            );
        }
        assert!(md.ends_with("Inline math"), "got {md:.120?}");

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

    // Stepless math completion. The author who knows the symbol is called "alpha" should not
    // also have to know it needs a leading backslash. The edit must REPLACE the bare token:
    // appending would leave `alp\alpha`.
    #[test]
    fn completion_offers_a_math_command_for_a_bare_token() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-comp-stepless.tmd").unwrap();
        let text = "Let $alp$ stand.\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // `$` opens at char 4, so char 8 sits just past the `p` of `alp`.
        let items = complete_at(&client, &uri, 60, 0, 8);
        let alpha = items
            .iter()
            .find(|i| i.label == "\\alpha")
            .unwrap_or_else(|| panic!("a bare `alp` in math should offer `\\alpha`: {items:?}"));
        let Some(lsp_types::CompletionTextEdit::Edit(edit)) = &alpha.text_edit else {
            panic!("the item must carry a text edit, so the typed token is replaced");
        };
        assert_eq!(edit.new_text, "\\alpha");
        assert_eq!(
            (edit.range.start.character, edit.range.end.character),
            (5, 8),
            "the edit must replace the typed `alp` rather than append after it"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    // The vocabulary carries every symbol's glyph, which makes the glyph a legitimate query:
    // an author who can produce `α` should be able to turn it into `\alpha`.
    //
    // `filterText` is load-bearing here rather than decoration. The client re-filters the
    // server's list against the text in the edit range, so an item whose only match is its
    // glyph is dropped by VS CODE even though the server returned it — the failure would look
    // like the server never answered.
    #[test]
    fn completion_finds_a_math_command_by_its_glyph() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-comp-glyph.tmd").unwrap();
        let text = "Let $α$ stand.\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // `α` is one UTF-16 unit, so the cursor just past it is char 6.
        let items = complete_at(&client, &uri, 62, 0, 6);
        let alpha = items
            .iter()
            .find(|i| i.label == "\\alpha")
            .unwrap_or_else(|| panic!("the glyph `α` should find `\\alpha`: {items:?}"));
        assert!(
            alpha
                .filter_text
                .as_deref()
                .is_some_and(|f| f.contains('α')),
            "the glyph must be in filterText or the client drops the item: {:?}",
            alpha.filter_text
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
            caps["inlayHintProvider"], true,
            "the resolved number beside a cross-reference"
        );
        assert_eq!(
            caps["selectionRangeProvider"], true,
            "expand-selection by structure rather than by the editor's word heuristics"
        );
        assert_eq!(
            caps["documentHighlightProvider"], true,
            "the anchor under the cursor and its other occurrences"
        );
        assert_eq!(
            caps["foldingRangeProvider"], true,
            "without this the editor falls back to indentation folding, which is \
             meaningless for a Markdown-derived format"
        );
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

    /// The Internals book documents this surface as a table, and that table is the only
    /// place an author or a maintainer can read what the server answers. It went stale the
    /// moment four capabilities were added in one batch and **nothing noticed** — the same
    /// failure mode `manifest.test.ts` exists to catch on the companion side, which is why
    /// this gate is worth its lines.
    ///
    /// The direction matters: this asserts every advertised `*Provider` has a row, not that
    /// every row has a provider. A row with no capability behind it is a *documentation*
    /// error the reader can see and report; a capability with no row is invisible, which is
    /// exactly the state item 180 was filed to fix.
    ///
    /// Only `*Provider` keys are checked. `positionEncoding` and `textDocumentSync` are wire
    /// settings rather than things an author gets, and they are deliberately not table rows.
    #[test]
    fn the_internals_capability_table_names_every_capability_the_server_advertises() {
        let doc = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/internals/extending.tmd");
        let text =
            std::fs::read_to_string(&doc).unwrap_or_else(|e| panic!("{}: {e}", doc.display()));

        let caps = serde_json::to_value(server_capabilities()).unwrap();
        let advertised: Vec<String> = caps
            .as_object()
            .expect("ServerCapabilities serializes to an object")
            .keys()
            .filter_map(|k| k.strip_suffix("Provider").map(str::to_owned))
            .collect();
        assert!(
            advertised.len() >= 12,
            "only {} providers found — the filter stopped matching, so this test would pass \
             vacuously however stale the table got",
            advertised.len()
        );

        for name in &advertised {
            // The one place the wire name and the prose name differ. The table says
            // `formatting` because that is what the editor command is called, and calling
            // it `documentFormatting` in the book to satisfy a test would be the test
            // writing the documentation.
            let row = match name.as_str() {
                "documentFormatting" => "formatting",
                other => other,
            };
            assert!(
                text.contains(&format!("| `{row}`")),
                "`{name}Provider` is advertised by `server_capabilities()` but \
                 docs/internals/extending.tmd has no `| `{row}`` row for it. An \
                 undocumented capability is one no author knows to use — add the row in \
                 the same change that adds the capability."
            );
        }
    }

    /// The same gate for the requests that are *not* LSP capabilities. A `taliesin/…` method
    /// advertises nothing — there is no `*Provider` key to notice it is missing — so it is
    /// the one part of this surface that can grow with no documentation at all. Reads the
    /// method names out of this module's own source, so adding a fourth one and forgetting
    /// the book is a failing test rather than a silent gap.
    #[test]
    fn the_internals_book_documents_every_taliesin_namespaced_method() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(root.join("src/lsp.rs")).unwrap();
        let text =
            std::fs::read_to_string(root.join("../../docs/internals/extending.tmd")).unwrap();

        // Every `"taliesin/…"` string literal in this file: the method constants, and only
        // those (nothing else in the module spells one). The trailing-name check is not
        // decoration — the needle above appears in this test's own source, so without it the
        // scan "finds" a method called `taliesin/` and the gate fails on itself.
        let methods: std::collections::BTreeSet<&str> = source
            .match_indices("\"taliesin/")
            .filter_map(|(i, _)| {
                let rest = &source[i + 1..];
                rest.find('"').map(|end| &rest[..end])
            })
            .filter(|m| {
                m.strip_prefix("taliesin/")
                    .is_some_and(|name| !name.is_empty() && name.chars().all(char::is_alphanumeric))
            })
            // `PANIC_PROBE_METHOD` is `#[cfg(test)]` and is not in the shipped binary at all,
            // so documenting it would describe a method no editor can call.
            .filter(|m| *m != PANIC_PROBE_METHOD)
            .collect();
        assert!(
            methods.len() >= 3,
            "found only {methods:?}; the scan stopped matching, so this test proves nothing"
        );
        for method in methods {
            assert!(
                text.contains(&format!("`{method}`")),
                "`{method}` is served but docs/internals/extending.tmd never mentions it"
            );
        }
    }

    // ---------------------------------------------------------------------------
    // `taliesin/sectionEdit` (backlog item 165): the wire half of the companion's four
    // structural commands. `lsp_edits`'s own tests own the transforms; these own the three
    // things only the dispatch can get wrong — that the method is answered at all, that a
    // refusal arrives as an error the companion can show rather than as a null, and that an
    // unopened buffer is not silently treated as an empty one.
    // ---------------------------------------------------------------------------

    fn section_edit_request(
        client: &Connection,
        uri: &Url,
        id: i32,
        line: u32,
        op: &str,
    ) -> Response {
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(id),
                method: SECTION_EDIT_METHOD.to_owned(),
                params: serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": 0 },
                    "op": op,
                }),
            }))
            .unwrap();
        recv_response(client, RequestId::from(id))
    }

    #[test]
    fn section_edit_answers_with_an_edit_and_the_cursor_to_follow_it() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);
        let uri = Url::parse("file:///tmp/sections.tmd").unwrap();
        did_open(&client, &uri, "## Alpha\n\na\n\n## Beta\n\nb\n".to_owned());

        let resp = section_edit_request(&client, &uri, 5, 0, "moveDown");
        assert!(resp.error.is_none(), "{:?}", resp.error);
        let result = resp.result.expect("an edit");
        assert_eq!(result["edits"].as_array().map(Vec::len), Some(1));
        assert!(
            result["edits"][0]["newText"]
                .as_str()
                .unwrap()
                .starts_with("## Beta"),
            "{result}"
        );
        // The camelCase spelling is the wire contract the companion reads; a Rust-side
        // rename to `snake_case` would leave it reading `undefined` and silently skip the
        // cursor fix-up.
        assert_eq!(result["cursor"]["line"], 4, "{result}");

        // A refusal is an error with a message, not a null result: the companion shows it.
        let refused = section_edit_request(&client, &uri, 6, 4, "moveDown");
        assert!(refused.result.is_none());
        let error = refused.error.expect("a refusal");
        assert_eq!(error.code, -32803);
        assert!(error.message.contains("Beta"), "{}", error.message);

        // An unopened buffer: an error too. Answering "no edits" would look like a document
        // the transform declined to change.
        let unopened = Url::parse("file:///tmp/never-opened.tmd").unwrap();
        let missing = section_edit_request(&client, &unopened, 7, 0, "promote");
        assert!(missing.result.is_none());
        assert!(
            missing
                .error
                .expect("an error")
                .message
                .contains("not open"),
            "an unopened document should say so"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    // ---------------------------------------------------------------------------
    // Debounced diagnostics (backlog item 178).
    //
    // `publish` runs a full render PLUS `site::anchors_defined_elsewhere_in_project`, which
    // walks and reads every page in the project. Undebounced, that ran once per keystroke.
    // The debounce is deliberately long here (250 ms) so the five sends below cannot straddle
    // the window on a loaded machine and turn a real regression into a flake.
    // ---------------------------------------------------------------------------

    // A full-buffer `didChange`, which is what FULL sync sends.
    fn did_change(client: &Connection, uri: &Url, version: i32, text: &str) {
        client
            .sender
            .send(Message::Notification(Notification {
                method: DidChangeTextDocument::METHOD.to_owned(),
                params: serde_json::to_value(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: uri.clone(),
                        version,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: text.to_owned(),
                    }],
                })
                .unwrap(),
            }))
            .unwrap();
    }

    // An unknown front-matter key is the cheapest diagnostic that quotes text we choose, so
    // the published message names WHICH edit it describes.
    fn typo_doc(key: &str) -> String {
        format!("---\n{key}: a\n---\n")
    }

    #[test]
    fn rapid_edits_coalesce_into_one_publish_of_the_final_text() {
        let (server, client) = Connection::memory();
        let handle =
            std::thread::spawn(move || run_with_debounce(server, Duration::from_millis(250)));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-debounce.tmd").unwrap();
        did_open(&client, &uri, typo_doc("tittle"));
        let _ = recv_publish(&client); // the didOpen publish is not debounced

        // Five edits inside one window. Only the last text may be reported on.
        for n in 0..5 {
            did_change(&client, &uri, n + 2, &typo_doc(&format!("tittle{n}")));
        }

        let published = recv_publish(&client);
        assert_eq!(published.uri, uri);
        assert!(
            published
                .diagnostics
                .iter()
                .any(|d| d.message.contains("tittle4")),
            "the coalesced publish must describe the LAST edit, got: {:?}",
            published
                .diagnostics
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );

        // Nothing further arrives: the other four edits were dropped, not queued.
        assert!(
            client
                .receiver
                .recv_timeout(Duration::from_millis(600))
                .is_err(),
            "five edits in one window must produce exactly one publish"
        );

        shutdown(&client);
        handle.join().unwrap().unwrap();
    }

    // Coalescing collapses repeated edits to ONE buffer. It must not collapse across buffers:
    // a single-slot `pending` would let an edit to B evict the diagnostics owed to A, and A's
    // squiggles would then never arrive at all — silently, and only for whoever edits two
    // files inside one window.
    #[test]
    fn an_edit_to_a_second_document_does_not_evict_the_first() {
        let (server, client) = Connection::memory();
        let handle =
            std::thread::spawn(move || run_with_debounce(server, Duration::from_millis(250)));
        handshake(&client);

        let a = Url::parse("file:///tmp/tali-debounce-a.tmd").unwrap();
        let b = Url::parse("file:///tmp/tali-debounce-b.tmd").unwrap();
        did_open(&client, &a, typo_doc("aaa"));
        let _ = recv_publish(&client);
        did_open(&client, &b, typo_doc("bbb"));
        let _ = recv_publish(&client);

        did_change(&client, &a, 2, &typo_doc("aaaa"));
        did_change(&client, &b, 2, &typo_doc("bbbb"));

        let mut seen: Vec<Url> = vec![recv_publish(&client).uri, recv_publish(&client).uri];
        seen.sort();
        assert_eq!(seen, vec![a, b], "both documents must be published for");

        shutdown(&client);
        handle.join().unwrap().unwrap();
    }

    // The window must close on a deadline set by the EDIT, not be reset by every message that
    // arrives. A client that polls (inlay hints on scroll, hovers as the pointer moves) sends a
    // steady stream of requests; if each one pushed the deadline out, the pending diagnostics
    // would be starved for as long as the pointer kept moving.
    #[test]
    fn a_stream_of_requests_does_not_starve_a_pending_publish() {
        let (server, client) = Connection::memory();
        let handle =
            std::thread::spawn(move || run_with_debounce(server, Duration::from_millis(120)));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-debounce-starve.tmd").unwrap();
        did_open(&client, &uri, typo_doc("tittle"));
        let _ = recv_publish(&client);

        did_change(&client, &uri, 2, &typo_doc("tittlex"));
        // Hover requests spanning well past the window, each one an opportunity to reset it.
        for n in 0..12 {
            client
                .sender
                .send(Message::Request(Request {
                    id: RequestId::from(200 + n),
                    method: lsp_types::request::HoverRequest::METHOD.to_owned(),
                    params: serde_json::json!({
                        "textDocument": { "uri": uri },
                        "position": { "line": 1, "character": 0 },
                    }),
                }))
                .unwrap();
            std::thread::sleep(Duration::from_millis(30));
        }

        // The publish must be in there somewhere, not still waiting behind the hovers.
        let mut published = None;
        for _ in 0..40 {
            match client.receiver.recv_timeout(Duration::from_secs(5)) {
                Ok(Message::Notification(n)) if n.method == PublishDiagnostics::METHOD => {
                    published =
                        Some(serde_json::from_value::<PublishDiagnosticsParams>(n.params).unwrap());
                    break;
                }
                Ok(_) => continue, // a hover response
                Err(e) => panic!("nothing more arrived: {e}"),
            }
        }
        let published = published.expect("the pending publish was starved by the request stream");
        assert!(
            published
                .diagnostics
                .iter()
                .any(|d| d.message.contains("tittlex")),
            "expected the edited text's diagnostic, got {:?}",
            published
                .diagnostics
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );

        shutdown(&client);
        handle.join().unwrap().unwrap();
    }
}
