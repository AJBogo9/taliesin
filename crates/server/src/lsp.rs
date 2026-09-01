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
    PositionEncodingKind, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions,
};
use std::process::ExitCode;

/// Advertised capabilities: full-text document sync (whole buffer on every change, which maps
/// 1:1 onto the buffer lint), go-to-definition, document symbols (the heading outline), hover,
/// completion, quick-fix code actions and folding. Six providers, every one of
/// them read-only: the `.tmd` file is the single editing surface and the editor is where it is
/// edited, so nothing here rewrites a buffer on the author's behalf.
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
        // Replaces indentation-based folding, which is what `.tmd` gets without this and is
        // meaningless in a format where nesting is heading level and fences.
        folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
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
    // A transport-level failure: a header with no `\r\n`, a non-UTF-8 or non-JSON body, a body
    // truncated by a miscounted Content-Length. It exits non-zero like the arm above, and a
    // non-zero exit is what VS Code counts toward the "server crashed 5 times" cutoff that
    // stops restarting us (see `main_loop`) — so saying nothing left the author watching the
    // server die repeatedly with no reason anywhere, the one error path in this function that
    // contradicted the doc comment above.
    if let Err(e) = io_threads.join() {
        crate::log::error(&format!("lsp: transport: {e}"));
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Complete the initialize handshake, then serve the message loop until `exit`.
/// Takes the connection by value so it (and its channels) drop before the caller
/// joins the stdio I/O threads.
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
    let init = connection.initialize(caps)?;
    register_file_watchers(&connection, &init)?;
    main_loop(&connection, debounce)?;
    Ok(())
}

/// Ask the client to watch the files that can invalidate an open buffer's diagnostics.
///
/// Without this, `workspace/didChangeWatchedFiles` arrives from **VS Code only**, and only
/// because `editor/vscode/src/client.ts` registers a watcher of its own. Every other editor
/// the docs name would keep showing diagnostics computed against a tree that a `git pull`
/// had already replaced — which would make a freshness fix that lives in Rust behave like
/// one that lives in the companion, against the rule that put it here.
///
/// Guarded on the client having said it supports dynamic registration: sending this to one
/// that did not is a protocol error aimed at a client that was working fine. The reply is
/// ignored on purpose — a client that refuses leaves us exactly where we started, and the
/// main loop already drops responses it did not ask about.
fn register_file_watchers(
    connection: &Connection,
    init: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dynamic = init
        .pointer("/capabilities/workspace/didChangeWatchedFiles/dynamicRegistration")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !dynamic {
        return Ok(());
    }
    // The kinds of file that change what an open page's diagnostics should say without
    // the page itself changing: a sibling page (its anchors and ids), the project config
    // (every rule that reads it), a bibliography (`[@key]` resolution), and a referenced
    // image, because creating the file is the only fix for `local asset not found` and
    // without a watcher the stale squiggle sticks until the next unrelated edit. The
    // image extensions are `lsp_complete::IMAGE_EXTS`, the server's one list of them.
    let image_glob = format!("**/*.{{{}}}", crate::lsp_complete::IMAGE_EXTS.join(","));
    let watchers: Vec<serde_json::Value> = ["**/*.tmd", "**/_site.yml", "**/*.bib"]
        .iter()
        .copied()
        .chain(std::iter::once(image_glob.as_str()))
        .map(|glob| serde_json::json!({ "globPattern": glob }))
        .collect();
    connection
        .sender
        .send(Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from("taliesin-watch-registration".to_owned()),
            method: "client/registerCapability".to_owned(),
            params: serde_json::json!({
                "registrations": [{
                    "id": "taliesin-watched-files",
                    "method": "workspace/didChangeWatchedFiles",
                    "registerOptions": { "watchers": watchers },
                }]
            }),
        }))?;
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
    // Shared by every request that reaches past the open buffer (cross-file definition and
    // hover, workspace symbols, the sidebar's two views). A stat-validated walk, not an index:
    // all of those fire on a user gesture, so re-validating with one `stat` per page is
    // cheaper than the file watching an index would need. See `lsp_project`.
    let mut project = crate::lsp_project::ProjectCache::new();
    // The page registry for the buffer lint, so a document inside a site is linted as one.
    // Separate from `project` above because it is the one memo on the per-keystroke path:
    // see `lsp_project::SiteCache` for the measurement that makes it mandatory.
    let mut sites = crate::lsp_project::SiteCache::new();
    // The last diagnostics published for each open buffer — what the editor is showing right
    // now. Read by hover, so a hover over a squiggle answers with the finding under it.
    let mut published: std::collections::HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>> =
        std::collections::HashMap::new();
    // Which FOREIGN URIs (include partials) each open buffer's last publish reached, so a
    // re-publish can retract the ones that no longer carry diagnostics: nothing else ever
    // publishes for a partial the editor has not opened. See `publish`.
    let mut foreign: std::collections::HashMap<lsp_types::Url, Vec<lsp_types::Url>> =
        std::collections::HashMap::new();
    // Messages the client has sent that we have not dispatched yet, and the ids among them a
    // `$/cancelRequest` in the same batch superseded. See [`read_batch`].
    let mut inbox: std::collections::VecDeque<Message> = std::collections::VecDeque::new();
    let mut cancelled: std::collections::HashSet<lsp_server::RequestId> =
        std::collections::HashSet::new();
    loop {
        if inbox.is_empty() {
            // Block outright when nothing is owed, so an idle server costs nothing; wait only
            // as long as the open window when a publish is pending.
            match read_batch(connection, pending.wait()) {
                Batch::Messages(msgs, dead) => {
                    inbox = msgs;
                    cancelled = dead;
                }
                // The window closed with no further edit: publish the latest text of every
                // buffer that is owed.
                //
                // Guarded and logged, exactly as the `didOpen` publish is on the notification
                // path below, and for the same reason: this is the every-keystroke path, so a
                // panic in a validator reading a half-typed buffer would take the whole
                // session down. Coalescing moved this call out from under the notification
                // guard in `5f2fc9fc` and left the identical call on `didOpen` caught — the
                // busier of the two ended up the unprotected one.
                Batch::Timeout => {
                    for uri in pending.take() {
                        if let Some(text) = docs.get(&uri) {
                            match crate::serve::guarded(|| {
                                publish(
                                    connection,
                                    &docs,
                                    &mut sites,
                                    &mut published,
                                    &mut foreign,
                                    &uri,
                                    text,
                                )
                            }) {
                                Ok(Ok(())) => {}
                                Ok(Err(e)) => crate::log::error(&format!(
                                    "lsp: publishing diagnostics for {uri}: {e}"
                                )),
                                Err(panic) => crate::log::error(&format!(
                                    "lsp: panic publishing diagnostics for {uri}: {panic}"
                                )),
                            }
                        }
                    }
                    continue;
                }
                Batch::Closed => break,
            }
        }
        let Some(msg) = inbox.pop_front() else {
            continue;
        };
        // A request the client withdrew while it was still queued behind other work. Answering
        // `RequestCancelled` is not a courtesy: the protocol owes a response to every id, and a
        // client that never gets one holds the pending entry (and, in VS Code, the progress it
        // was showing) for the rest of the session.
        if let Message::Request(req) = &msg
            && cancelled.remove(&req.id)
        {
            connection
                .sender
                .send(Message::Response(lsp_server::Response {
                    id: req.id.clone(),
                    result: None,
                    error: Some(lsp_server::ResponseError {
                        code: -32800, // JSON-RPC RequestCancelled (LSP)
                        message: format!("{} was cancelled", req.method),
                        data: None,
                    }),
                }))?;
            continue;
        }
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
                    handle_request(
                        connection,
                        &docs,
                        &mut project,
                        &mut sites,
                        &mut published,
                        req,
                    )
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
                    handle_notification(
                        connection,
                        &mut docs,
                        &mut sites,
                        &mut published,
                        &mut foreign,
                        &mut pending,
                        debounce,
                        notif,
                    )
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

/// One turn's worth of client traffic.
enum Batch {
    /// Messages to dispatch, and the ids among them the client withdrew.
    Messages(
        std::collections::VecDeque<Message>,
        std::collections::HashSet<lsp_server::RequestId>,
    ),
    /// The coalescing window closed with nothing new to read.
    Timeout,
    /// The client is gone.
    Closed,
}

/// Read everything the client has already sent, and separate out the requests it has since
/// withdrawn.
///
/// **This is the whole of `$/cancelRequest` in a synchronous server, and it is the right
/// shape for one.** A request already being executed cannot be abandoned here (there is one
/// thread and it is inside the handler), but that was never the case worth catching. The case
/// worth catching is the *queue*: a client that fires a request per keystroke and withdraws
/// each one as the next arrives leaves this loop running work nobody is waiting on. Draining
/// the channel before dispatching lets the cancel that arrived *behind* the superseded request
/// be seen *before* it. (The measured example was `workspace/symbol`, a whole-project walk at
/// **167 ms** on the largest project here; that method went on 2026-08-08, and the loop
/// property it motivated is the client's to rely on regardless of which request is slow.)
///
/// Cancellations are matched only against messages **in this batch**. A cancel for a request
/// already answered is dropped rather than remembered, so a client that reuses request ids
/// cannot have a later request killed by an older cancel.
fn read_batch(connection: &Connection, wait: Option<std::time::Duration>) -> Batch {
    let first = match wait {
        None => match connection.receiver.recv() {
            Ok(m) => m,
            Err(_) => return Batch::Closed,
        },
        Some(remaining) => match connection.receiver.recv_timeout(remaining) {
            Ok(m) => m,
            Err(e) if e.is_timeout() => return Batch::Timeout,
            Err(_) => return Batch::Closed,
        },
    };
    let mut msgs = vec![first];
    // Everything else already buffered. Non-blocking, so an idle client costs one `recv`.
    //
    // **Except past `shutdown`.** `Connection::handle_shutdown` answers it and then reads the
    // channel itself, waiting up to 30 s for the `exit` that follows. Draining `exit` into
    // this batch would take it out of the channel that call is watching, so every clean
    // teardown would spend 30 s timing out and log a protocol complaint about a client that
    // did nothing wrong.
    while !msgs.last().is_some_and(is_shutdown) {
        match connection.receiver.try_recv() {
            Ok(m) => msgs.push(m),
            Err(_) => break,
        }
    }
    let mut cancelled = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::with_capacity(msgs.len());
    for m in msgs {
        match cancel_target(&m) {
            Some(id) => {
                cancelled.insert(id);
            }
            None => queue.push_back(m),
        }
    }
    // Only ids this batch actually carries. A cancel for something already answered is noise.
    let live: std::collections::HashSet<lsp_server::RequestId> = queue
        .iter()
        .filter_map(|m| match m {
            Message::Request(r) => Some(r.id.clone()),
            _ => None,
        })
        .collect();
    cancelled.retain(|id| live.contains(id));
    Batch::Messages(queue, cancelled)
}

/// Is this the `shutdown` request? The one message the batch reader must not read past.
fn is_shutdown(msg: &Message) -> bool {
    use lsp_types::request::Request as _;
    matches!(msg, Message::Request(r)
        if r.method == lsp_types::request::Shutdown::METHOD)
}

/// The request id a `$/cancelRequest` notification names, or `None` for any other message.
fn cancel_target(msg: &Message) -> Option<lsp_server::RequestId> {
    use lsp_types::notification::Notification as _;
    let Message::Notification(n) = msg else {
        return None;
    };
    if n.method != lsp_types::notification::Cancel::METHOD {
        return None;
    }
    match serde_json::from_value::<lsp_types::CancelParams>(n.params.clone())
        .ok()?
        .id
    {
        lsp_types::NumberOrString::Number(n) => Some(lsp_server::RequestId::from(n)),
        lsp_types::NumberOrString::String(s) => Some(lsp_server::RequestId::from(s)),
    }
}

/// Test-only method name that panics inside the dispatch. Real input does not panic the
/// renderer (AP2's fuzz round produced zero unexpected panics), so injecting one here is the
/// only way to exercise the loop's panic boundary — the guard exists for the residual
/// panic surface, not for a known repro. `#[cfg(test)]`, so it is absent from the binary.
#[cfg(test)]
pub(crate) const PANIC_PROBE_METHOD: &str = "taliesin/testPanic";

/// The same probe for the path a method name cannot reach: the COALESCED publish, which no
/// notification dispatches — the main loop runs it once the debounce window closes, on
/// whatever text the buffer holds by then. A buffer containing this line panics inside
/// `publish`. `#[cfg(test)]`, so it is absent from the binary.
#[cfg(test)]
pub(crate) const PANIC_PROBE_TEXT: &str = "taliesin-test-panic-in-publish";

/// Dispatch a text-document notification: keep the open-buffer store current and
/// (re)publish diagnostics for the affected buffer.
#[allow(clippy::too_many_arguments)]
fn handle_notification(
    connection: &Connection,
    docs: &mut std::collections::HashMap<lsp_types::Url, String>,
    sites: &mut crate::lsp_project::SiteCache,
    published: &mut std::collections::HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>>,
    foreign: &mut std::collections::HashMap<lsp_types::Url, Vec<lsp_types::Url>>,
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
        // advertised hover/completion/symbols and diagnostics and then answered
        // null to all of them, with nothing on stderr to say why.
        if p.text_document.language_id == "taliesin" || is_tmd_uri(&p.text_document.uri) {
            let uri = p.text_document.uri;
            docs.insert(uri.clone(), p.text_document.text);
            publish(
                connection,
                docs,
                sites,
                published,
                foreign,
                &uri,
                &docs[&uri],
            )?;
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
    } else if method == lsp_types::notification::DidChangeWatchedFiles::METHOD {
        // A file changed on disk that the editor may never have opened: a `git pull` or
        // `git checkout`, an agent editing a sibling chapter, a `.bib` or `_site.yml` edit.
        //
        // Every open buffer is re-published, because any of them can be wrong now and the
        // notification does not say which: a chapter's cross-page links are judged against
        // the OTHER pages' ids, its citations against the bibliography, its whole lint
        // against `_site.yml`. `publish` is per-URI, so nothing else ever refreshes a
        // document the author is not currently typing in — editing page B in the editor did
        // not refresh page A either. There are only ever a handful of open buffers.
        //
        // Owed rather than published here: a `git pull` touching two hundred files arrives
        // as a burst, and the coalescing window collapses it into one publish per buffer.
        // The params are deliberately not read — no field of them changes the answer.
        for uri in docs.keys().cloned().collect::<Vec<_>>() {
            pending.owe(uri, debounce);
        }
    } else if method == DidCloseTextDocument::METHOD {
        let p: DidCloseTextDocumentParams = serde_json::from_value(notif.params)?;
        docs.remove(&p.text_document.uri);
        published.remove(&p.text_document.uri);
        // Before the clear below, or a window that closes after this would re-publish
        // diagnostics for a buffer that is gone.
        pending.forget(&p.text_document.uri);
        // We own the collection the editor is showing, so closing the buffer has to empty
        // it: nothing else ever will.
        publish_diagnostics(connection, &p.text_document.uri, Vec::new())?;
        // The foreign URIs this buffer's publishes reached are ours to retract too,
        // except one the author has open in its own right: its own publishes govern it.
        if let Some(targets) = foreign.remove(&p.text_document.uri) {
            retract_foreign(connection, docs, published, targets)?;
        }
    }
    Ok(())
}

/// Retract a parent's foreign publishes at `uris`: remove each from `published` AND send
/// the empty publish, always together (splitting the pair desyncs hover from the
/// squiggles), skipping any URI the author has open in its own right (its own publishes
/// govern it, and `did_close` empties it when it goes).
fn retract_foreign(
    connection: &Connection,
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    published: &mut std::collections::HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>>,
    uris: impl IntoIterator<Item = lsp_types::Url>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for t in uris {
        if docs.contains_key(&t) {
            continue;
        }
        published.remove(&t);
        publish_diagnostics(connection, &t, Vec::new())?;
    }
    Ok(())
}

/// Answer a request from the open-buffer store. Only `textDocument/definition` is handled;
/// any other request gets a `MethodNotFound` reply so the client never hangs waiting.
#[allow(clippy::too_many_arguments)]
fn handle_request(
    connection: &Connection,
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    project: &mut crate::lsp_project::ProjectCache,
    sites: &mut crate::lsp_project::SiteCache,
    published: &mut std::collections::HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>>,
    req: lsp_server::Request,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use lsp_types::request::{
        CodeActionRequest, Completion, DocumentSymbolRequest, FoldingRangeRequest, GotoDefinition,
        HoverRequest, Request as _,
    };
    #[cfg(test)]
    assert!(req.method != PANIC_PROBE_METHOD, "injected request panic");
    let response = if req.method == FoldingRangeRequest::METHOD {
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
            result: Some(serde_json::to_value(resolve_hover(
                docs, project, published, &params,
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
    } else if req.method == SITE_MAP_METHOD {
        // The uri is a project ROOT, not a document: the companion has already walked up to
        // the nearest `_site.yml` to decide *whether* to preview the project at all, and
        // re-deriving the root here would be a second answer to a question it has settled.
        #[derive(serde::Deserialize)]
        struct RootParams {
            uri: lsp_types::Url,
        }
        let params: RootParams = serde_json::from_value(req.params)?;
        let answer = params
            .uri
            .to_file_path()
            .ok()
            .and_then(|root| crate::lsp_project::site_map(sites, &root));
        lsp_server::Response {
            id: req.id,
            // `null` for anything that is not a project with pages. It must not be an error:
            // the site-aware preview is an *upgrade* on the single-file one, so an
            // unanswerable map costs the author nav and cross-page links, never the preview.
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
        Target::Include { path, .. } => {
            let dir = uri.to_file_path().ok()?;
            let abs = dir.parent()?.join(&path);
            if !abs.exists() {
                return None;
            }
            Location::new(Url::from_file_path(&abs).ok()?, point("", 0, 0, 0))
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
        // A front-matter key has no definition site to jump to; its answer is the hover.
        Target::FrontmatterKey { .. } | Target::None => return None,
    };
    Some(lsp_types::GotoDefinitionResponse::Scalar(location))
}

/// The custom request a client calls to learn where a document's code cells are, so it can
/// route completion inside one to whoever owns that language. Namespaced, because it is not
/// an LSP method and must never collide with one.
pub(crate) const CELL_REGIONS_METHOD: &str = "taliesin/cellRegions";

/// Where each of a project's pages is served, so the companion can open the preview webview
/// at the document the author is editing. Namespaced for the same reason as the two above.
///
/// This was `taliesin map <root> --format json`, spawned per preview, until Wave 2 cut the
/// machine-facing verbs. The capability was never machine-facing — it is what makes
/// "Preview" open chapter 7 instead of the book's cover — so it moved here rather than
/// going with the verb, which is the doctrine anyway: editor intelligence lives in the LSP.
pub(crate) const SITE_MAP_METHOD: &str = "taliesin/siteMap";

/// Hover: the token under the cursor, plus the message of any diagnostic that covers it.
///
/// The two are merged rather than chosen between, because a hover is the one surface where
/// both questions get asked at the same position — "what is `@fig-2`" and "why is it
/// squiggled". The diagnostic is a *section under* the token's answer, never in place of it.
///
/// The message is the whole explanation now. It used to be joined by a catalogued cause + fix
/// for the diagnostic's `TAL-*` code, and both went with that catalogue on 2026-08-08: every
/// message that has a mechanical fix already names it inline (a did-you-mean), so the second
/// body was mostly a restatement to keep true.
fn resolve_hover(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    project: &mut crate::lsp_project::ProjectCache,
    published: &std::collections::HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>>,
    params: &lsp_types::HoverParams,
) -> Option<lsp_types::Hover> {
    use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

    let uri = &params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    let token = token_hover(docs, project, params);
    // The diagnostics the editor is *currently showing* for this buffer, not a fresh lint: it
    // is the squiggle under the pointer that the author is asking about, and re-linting here
    // would put a whole-project walk on the pointer-move path.
    let empty = Vec::new();
    let diagnostics = published.get(uri).unwrap_or(&empty);
    let range = crate::lsp_diag::narrowest_range_at(diagnostics, pos);
    // Innermost first, deduplicated on the text: a precise token span is a better answer than
    // the whole-line squiggle it sits inside, and two providers can publish the same sentence.
    let mut covering: Vec<&lsp_types::Diagnostic> = diagnostics
        .iter()
        .filter(|d| d.source.as_deref() == Some(crate::lint::LSP_SOURCE))
        .filter(|d| d.range.start <= pos && pos <= d.range.end)
        .collect();
    covering.sort_by_key(|d| {
        (
            d.range.end.line - d.range.start.line,
            d.range
                .end
                .character
                .saturating_sub(d.range.start.character),
        )
    });
    let mut messages: Vec<&str> = Vec::new();
    for d in covering {
        if !messages.contains(&d.message.as_str()) {
            messages.push(&d.message);
        }
    }
    if messages.is_empty() {
        return token;
    }
    let body = messages.join("\n\n---\n\n");
    match token {
        Some(t) => {
            let head = match t.contents {
                HoverContents::Markup(m) => m.value,
                // Neither other variant is ever constructed below; keep the diagnostic
                // rather than dropping it if that ever changes.
                _ => String::new(),
            };
            Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("{head}\n\n---\n\n{body}"),
                }),
                range: t.range,
            })
        }
        // No token here, only a squiggle: highlight the diagnostic's own span so the tooltip
        // is anchored to something rather than floating.
        None => Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: body,
            }),
            range,
        }),
    }
}

/// Resolve hover for the token under the cursor: an xref's rendered label + number, a
/// front-matter key's documentation, or a citation's BibTeX entry. `None` when the token
/// resolves to nothing (an unknown xref, an undocumented key, a missing/absent `.bib` entry)
/// or is not a hoverable kind (an include path is go-to-definition only, mirroring the
/// companion). Markdown content, ranged to the token so the editor highlights it.
fn token_hover(
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
        if diag.source.as_deref() != Some(crate::lint::LSP_SOURCE) {
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
    let lines: Vec<&str> = crate::lsp_pos::lines(text).collect();
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
            // No `theoremKinds` here: theorem environments went in wave 8 and `vocab.rs`
            // has never emitted that key since, so indexing it yielded `Value::Null` and
            // `from_named` returned an empty list. A silent no-op, because serde_json's
            // Index returns Null for a missing key rather than panicking.
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

fn document_symbols(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    uri: &lsp_types::Url,
) -> Option<lsp_types::DocumentSymbolResponse> {
    let text = docs.get(uri)?;
    let lines: Vec<&str> = crate::lsp_pos::lines(text).collect();
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

/// Lint `text` as the document at `uri`, publish the result, and remember it.
///
/// Remembered because two later readers need the diagnostics the editor is *currently
/// showing*, not a fresh lint of their own: hover, to put the `--explain` body under the
/// squiggle the pointer is on, and the pull-diagnostic arms, which must not contradict the
/// squiggles in the window above the Problems panel.
///
/// A diagnostic a validator located into an INCLUDE PARTIAL publishes under the partial's
/// own URI (`publishDiagnostics` may target any URI), never on this buffer at the
/// partial's line: `(source_file, line)` is one coordinate pair, and splitting it
/// squiggled an unrelated (clamped) parent line while `build --check-only` named the
/// partial's location. `foreign` remembers which URIs this buffer's last publish reached,
/// so a re-publish retracts the ones that no longer carry diagnostics; nothing else ever
/// publishes for an unopened partial. A partial that is ALSO open is SKIPPED here, the
/// same guard the retraction below and `did_close` apply: its own publishes govern it,
/// and this side lints the partial's on-DISK text, so publishing would overwrite a dirty
/// buffer's fresher lint with stale-position squiggles. Two open parents including the
/// same partial still race each other's foreign sets (last writer wins, and one parent
/// dropping the include retracts the other's diagnostics until it republishes): accepted
/// as transient, the next edit anywhere heals it.
fn publish(
    connection: &Connection,
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    sites: &mut crate::lsp_project::SiteCache,
    published: &mut std::collections::HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>>,
    foreign: &mut std::collections::HashMap<lsp_types::Url, Vec<lsp_types::Url>>,
    uri: &lsp_types::Url,
    text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(test)]
    assert!(!text.contains(PANIC_PROBE_TEXT), "injected publish panic");
    let path = uri
        .to_file_path()
        .unwrap_or_else(|_| std::path::PathBuf::from("untitled.tmd"));
    // The enclosing project, when this buffer is a page of one. Without it the buffer is
    // linted as a standalone document and the editor gets a strictly weaker answer than the
    // read-only preview: no broken cross-page anchors at all, and broken links described by
    // a rule that does not apply to a site page.
    let routed = crate::lsp_diag::diagnose_file(sites, &path, text);
    published.insert(uri.clone(), routed.own.clone());
    publish_diagnostics(connection, uri, routed.own)?;
    let mut reached: Vec<lsp_types::Url> = Vec::new();
    for (fpath, diags) in routed.foreign {
        let Ok(furi) = lsp_types::Url::from_file_path(&fpath) else {
            continue;
        };
        if docs.contains_key(&furi) {
            continue;
        }
        published.insert(furi.clone(), diags.clone());
        publish_diagnostics(connection, &furi, diags)?;
        reached.push(furi);
    }
    // Retract the foreign URIs the previous publish reached and this one did not, except
    // one the author has open in its own right: its own publishes govern it now.
    if let Some(prev) = foreign.remove(uri) {
        let stale = prev.into_iter().filter(|s| !reached.contains(s));
        retract_foreign(connection, docs, published, stale)?;
    }
    if !reached.is_empty() {
        foreign.insert(uri.clone(), reached);
    }
    Ok(())
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

    // Send initialize + initialized so the server enters its main loop. Declares no client
    // capabilities, which is the PUSH transport — what an editor predating LSP 3.17 has, and
    // what every test written before the pull model exercises.
    fn handshake(client: &Connection) {
        handshake_with(client, serde_json::json!({}));
    }

    /// [`handshake`] with the client capabilities spelled out, for the tests that need the
    /// pull transport (which is chosen from `textDocument.diagnostic`, not by us).
    fn handshake_with(client: &Connection, capabilities: serde_json::Value) {
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(1),
                method: "initialize".to_owned(),
                params: serde_json::json!({ "capabilities": capabilities }),
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

    /// Whether any published diagnostic is a broken cross-page anchor. Matched on the message
    /// opening, because a diagnostic carries no `code` since the `TAL-*` catalogue went on
    /// 2026-08-08; this is the phrase `Site::validate_cross_page_links` writes.
    fn has_broken_anchor(diags: &[lsp_types::Diagnostic]) -> bool {
        diags
            .iter()
            .any(|d| d.message.starts_with("broken link anchor:"))
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

    /// A project root with the given `(relative path, source)` pages. Returns the root.
    fn symbol_fixture(name: &str, pages: &[(&str, &str)]) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("tali-lsp-proj-{}-{name}", std::process::id()));
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

        let hover = resolve_hover(
            &docs,
            &mut project,
            &Default::default(),
            &hover_params(&uri, 0, 6),
        )
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

        // (2) A panicking NOTIFICATION is skipped, and the buffer store still works after.
        // (The every-keystroke path is no longer this one: `didChange` only records what is
        // owed and the main loop publishes once the window closes, so its panic boundary is a
        // different arm and has its own test —
        // `a_panic_in_the_coalesced_publish_does_not_kill_the_session`.)
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

        // The same measurement, on the shared boundary helper the outline above and every
        // whole-line squiggle both go through.
        assert_eq!(
            crate::lsp_pos::line_end_utf16(crate::lsp_pos::nth_line(&crlf, 0)) as u32,
            heading.chars().count() as u32,
            "line_end_utf16 must not count the CR"
        );

        // And a whole-line diagnostic squiggle, which shares the defect through `to_lsp`.
        let lines: Vec<&str> = crlf.split('\n').collect();
        // 1-based line 2 is the blank line between the heading and the prose: with the CR
        // counted it spanned one column of nothing.
        let blank = crate::lint::Diagnostic::new("d.tmd".into(), Some(2), "x".into());
        assert_eq!(
            blank.to_lsp(&lines).range.end.character,
            0,
            "an empty CRLF line spans nothing, not one column"
        );
        // 1-based line 1 is the heading: it spans its visible text, not text + CR.
        let on_heading = crate::lint::Diagnostic::new("d.tmd".into(), Some(1), "x".into());
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
            source: Some(crate::lint::LSP_SOURCE.to_string()),
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

    // A method the server does not implement is answered with JSON-RPC MethodNotFound. The
    // code is the whole contract here: a client reads it to decide between "this server
    // can't do that" and a real failure, and no test looked at it.
    //
    // The probe is RANGE formatting, and it must keep naming a method this server declines:
    // a probe that names a real feature tests the wrong thing. That has already happened once
    // here: it used to probe `textDocument/formatting`, which stopped being unimplemented the
    // day a table formatter landed (and became unimplemented again when that formatter went).
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

    // The reject axis for the same lookup: a key the vocab does not document gets no key
    // documentation. Without this, a lookup that always answers something (a constant, or
    // "the first entry that isn't this one") looks correct from the positive case alone — and
    // would invent documentation for a key that has none, which is worse than saying nothing.
    //
    // What it *does* get is the other half of item 220: an unknown key is a diagnostic, and
    // the diagnostic reaches the hover rather than only the gutter. That is a different answer
    // from a key's docs, and this test is where the two must not be confused for one another.
    #[test]
    fn hover_on_an_undocumented_frontmatter_key_shows_the_diagnostic_instead() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-hover-fm-unknown.tmd").unwrap();
        let text = "---\nfrobnicate: Hello\n---\n\nBody.\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        let h = hover_raw_at(&client, &uri, 53, 1, 2).expect("the diagnostic under the pointer");
        let md = hover_markdown(&h);
        assert!(
            !md.contains("`frobnicate:`"),
            "an undocumented key must not be handed another key's documentation: {md:?}"
        );
        assert!(
            md.contains("unknown front-matter key `frobnicate`"),
            "the squiggle under the pointer is what the author is asking about: {md:?}"
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
        // `cache` is the only `execute:` child since `echo`/`include` were retired
        // (2026-08-02), so it is what an indented completion under `execute:` must offer.
        let cache = items
            .iter()
            .find(|i| i.label == "cache")
            .unwrap_or_else(|| panic!("expected an `execute:` child key, got {labels:?}"));
        assert!(
            !items
                .iter()
                .any(|i| i.label == "echo" || i.label == "include"),
            "a retired sub-key must never be OFFERED: {labels:?}"
        );
        // Every item carries its kind, which is what the editor draws an icon from and
        // sorts by; without one the list degrades to undifferentiated text.
        assert_eq!(
            cache.kind,
            Some(lsp_types::CompletionItemKind::PROPERTY),
            "a front-matter key completes as a property"
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

    // A target the render has no number for falls back to the generic detail rather than
    // rendering its label with the number missing — "Theorem " with a trailing space is what
    // the guard in `merged_xref_targets` exists to prevent.
    //
    // The case this used to drive was `theorems: numbered: false`, retired 2026-08-02. The
    // live route is now the buffer-anchor half of the union: `harvest_anchor_ids` offers an
    // anchor as soon as it is typed, before anything numbers it — which is also what an
    // author sees mid-edit. (The render's own empty-number arm is defensive rather than
    // document-reachable now: something that is not numbered, such as a `.proof`, registers
    // no xref entry at all rather than an empty one. The arm is kept as such.)
    #[test]
    fn completion_detail_stays_generic_for_a_target_with_no_number() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-lsp-comp-unnumbered.tmd").unwrap();
        // `#fig-key` on a span: a real xref prefix, harvested from the buffer, but attached
        // to nothing the render numbers (a span is not a float). It was `#thm-key` until
        // 2026-08-18; once the seven theorem prefixes were cut, `thm` was no longer a
        // prefix at all, so this went on passing while testing the UNKNOWN-prefix path
        // instead of the one it documents.
        let text = "---\ntitle: T\n---\n\n[anchored]{#fig-key}\n\nSee @\n".to_string();
        did_open(&client, &uri, text);
        let _ = recv_publish(&client);

        // Cursor right after the `@` on line 6.
        let items = complete_at(&client, &uri, 49, 6, 5);
        let hit = items
            .iter()
            .find(|i| i.label == "fig-key")
            .expect("the unnumbered target's anchor should still be offered");
        assert_eq!(
            hit.detail.as_deref(),
            Some("cross-reference target"),
            "a target with no number must not claim a label"
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

    /// The initialize handshake is the *only* thing that tells an editor which features exist:
    /// an unadvertised capability is one the editor never asks for, so it silently does not
    /// exist. Every other test here throws the `InitializeResult` away (`handshake` does
    /// `let _ = recv()`), which is why all twelve mutants in `server_capabilities` survived the
    /// 2026-07-27 mutation run, including replacing its whole body with `Default::default()`.
    /// A server advertising *nothing* passed the entire suite.
    ///
    /// So assert the value that actually goes over the wire, field by field, since each deleted
    /// field is its own silent feature loss. `definitionProvider` is the load-bearing one: it
    /// is click-to-source, from the editor's side.
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
        assert_eq!(caps["hoverProvider"], true);
        assert_eq!(caps["codeActionProvider"], true);
        assert_eq!(
            caps["foldingRangeProvider"], true,
            "without this the editor falls back to indentation folding, which is \
             meaningless for a Markdown-derived format"
        );
        // `@` xref/cite, `.` div class, `|` cell option, `-` xref prefix, `/` path,
        // `:` front-matter value. A dropped trigger character is a completion that never opens.
        assert_eq!(
            caps["completionProvider"]["triggerCharacters"],
            serde_json::json!(["@", ".", "|", "-", "/", ":", "\\"])
        );
        // The write paths went on 2026-08-08: the `.tmd` file is the single editing surface
        // and the editor is what edits it. Advertising one again is a decision, not a detail,
        // so it fails here rather than arriving unnoticed. `codeLensProvider` joined them on
        // 2026-08-09 for a different reason: wave 13 cut `taliesin run`, which left the lens
        // a bare `⚡ cached` label shipping an empty command string, and the one ground on
        // record for keeping it — that `runcell.ts` proved a TypeScript lens would regrow in
        // its place — was spent, because that file had already been deleted with the verb.
        for gone in [
            "renameProvider",
            "documentFormattingProvider",
            "documentLinkProvider",
            "inlayHintProvider",
            "documentHighlightProvider",
            "referencesProvider",
            "selectionRangeProvider",
            "workspaceSymbolProvider",
            "diagnosticProvider",
            "codeLensProvider",
        ] {
            assert!(
                caps[gone].is_null(),
                "{gone} is advertised again; it was retired with the LSP long tail"
            );
        }

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
        // An anti-vacuity floor, not a content floor: it exists so a `strip_suffix` that
        // stopped matching fails here instead of passing over an empty list. Measured at
        // exactly 6 on 2026-08-09, when the codeLens provider went; it was 7 before that.
        // A cut that takes another provider lowers this line and records the new count.
        assert!(
            advertised.len() >= 6,
            "only {} providers found — the filter stopped matching, so this test would pass \
             vacuously however stale the table got",
            advertised.len()
        );

        for name in &advertised {
            assert!(
                text.contains(&format!("| `{name}`")),
                "`{name}Provider` is advertised by `server_capabilities()` but \
                 docs/internals/extending.tmd has no `| `{name}`` row for it. An \
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
        // Anti-vacuity, not content: measured at exactly 2 on 2026-08-09, when
        // `taliesin/mathCommands` went with the symbol picker. It was 3 before that.
        assert!(
            methods.len() >= 2,
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

    // HEALTH-1's other half, and the one the coalescing change took away without anyone
    // noticing: the publish `didChange` owes runs in the main loop's timeout arm, not under
    // the notification dispatch's guard, so from `5f2fc9fc` until 2026-08-13 a panic on the
    // BUSIEST path in the server — a validator reading a half-typed buffer — unwound out of
    // `main_loop` and ended the session. Mutation check: drop the `guarded` in the `Timeout`
    // arm and this test fails on the recv below.
    #[test]
    fn a_panic_in_the_coalesced_publish_does_not_kill_the_session() {
        let (server, client) = Connection::memory();
        let prior = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let handle =
            std::thread::spawn(move || run_with_debounce(server, Duration::from_millis(120)));
        handshake(&client);

        let uri = Url::parse("file:///tmp/tali-publish-panic.tmd").unwrap();
        did_open(&client, &uri, typo_doc("tittle"));
        let _ = recv_publish(&client);

        // The edit that panics reaches `publish` only through the coalesced arm.
        did_change(&client, &uri, 2, &format!("{}\n", PANIC_PROBE_TEXT));
        // Nothing is published for it, and nothing may be published for it: the point is only
        // that the session survives to answer the edit after it.
        assert!(
            client
                .receiver
                .recv_timeout(Duration::from_millis(400))
                .is_err(),
            "a panicking publish sends nothing"
        );

        did_change(&client, &uri, 3, &typo_doc("recovered"));
        let after = recv_publish(&client);
        assert!(
            after
                .diagnostics
                .iter()
                .any(|d| d.message.contains("recovered")),
            "the session must still publish after the panic, got: {:?}",
            after
                .diagnostics
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );

        shutdown(&client);
        handle.join().unwrap().unwrap();
        std::panic::set_hook(prior);
    }

    // The window must close on a deadline set by the EDIT, not be reset by every message that
    // arrives. A client that polls (hovers as the pointer moves, completion as it types) sends
    // a steady stream of requests; if each one pushed the deadline out, the pending diagnostics
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

    /// The companion has always paid for this signal and the server has always dropped it:
    /// `client.ts` watches `**/{*.tmd,_site.yml,*.bib}`, so `vscode-languageclient` sends
    /// `workspace/didChangeWatchedFiles` on every external change — a `git pull`, an agent
    /// editing a sibling chapter, a `.bib` edit — and `lsp.rs` had zero occurrences of the
    /// method. Every open buffer kept squiggles computed against a tree that no longer
    /// existed, and `publish` is per-URI so editing page B in the editor never refreshed A.
    ///
    /// Asserted through a REAL change of state, not by counting notifications: `a.tmd`
    /// links to an anchor that `b.tmd` does not define yet, so it opens with a broken-anchor
    /// diagnostic. `b.tmd` then gains the heading on disk — a change to a file the editor
    /// never opened — and the only thing that can clear A's squiggle is the server acting on
    /// the watch notification.
    #[test]
    fn an_external_change_re_publishes_every_open_buffer() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: P\n").unwrap();
        let a = dir.join("a.tmd");
        let a_src = "---\ntitle: A\n---\n\nSee [it](b.html#sec-later).\n";
        std::fs::write(&a, a_src).unwrap();
        std::fs::write(
            dir.join("b.tmd"),
            "---\ntitle: B\n---\n\nNothing here yet.\n",
        )
        .unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        let a_uri = Url::from_file_path(dir.join("a.tmd")).unwrap();

        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);

        did_open(&client, &a_uri, a_src.to_owned());
        let first = recv_publish(&client);
        assert!(
            has_broken_anchor(&first.diagnostics),
            "the buffer should open with a broken cross-page anchor: {:?}",
            first
                .diagnostics
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );

        // The fix happens on DISK, in a file the editor never opened.
        std::fs::write(
            dir.join("b.tmd"),
            "---\ntitle: B\n---\n\n## Later {#sec-later}\n",
        )
        .unwrap();
        client
            .sender
            .send(Message::Notification(Notification {
                method: lsp_types::notification::DidChangeWatchedFiles::METHOD.to_owned(),
                params: serde_json::json!({
                    "changes": [{
                        "uri": Url::from_file_path(dir.join("b.tmd")).unwrap(),
                        "type": 2,
                    }]
                }),
            }))
            .unwrap();

        let after = recv_publish(&client);
        assert_eq!(after.uri, a_uri, "the open buffer is the one re-published");
        assert!(
            !has_broken_anchor(&after.diagnostics),
            "an external fix must clear the squiggle it caused: {:?}",
            after
                .diagnostics
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The parent buffer includes a partial whose broken `@fig-` sits at the PARTIAL's
    /// line 3. `(source_file, line)` is one coordinate pair: the publish used to attach
    /// the diagnostic to the parent's URI at that line, squiggling an unrelated (clamped)
    /// parent line while the partial's own URI got nothing, so the editor disagreed with
    /// `build --check-only`, which names the partial's location. A foreign diagnostic
    /// publishes under the partial's URI: `publishDiagnostics` may target any URI.
    #[test]
    fn an_included_partials_diagnostic_publishes_on_the_partials_own_uri_and_line() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-partial-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("subsections")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: P\n").unwrap();
        let parent_src =
            "---\ntitle: R\n---\n\n# Intro\n\n{{< include subsections/_intro.tmd >}}\n";
        std::fs::write(dir.join("index.tmd"), parent_src).unwrap();
        std::fs::write(
            dir.join("subsections/_intro.tmd"),
            "Intro prose.\n\nSee @fig-nope for details.\n",
        )
        .unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        let parent_uri = Url::from_file_path(dir.join("index.tmd")).unwrap();
        let partial_uri = Url::from_file_path(dir.join("subsections/_intro.tmd")).unwrap();

        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);
        did_open(&client, &parent_uri, parent_src.to_owned());

        // One publish for the buffer itself and one for the partial, in either order.
        let mut by_uri = std::collections::HashMap::new();
        for _ in 0..2 {
            let p = recv_publish(&client);
            by_uri.insert(p.uri.clone(), p.diagnostics);
        }
        let own = by_uri.get(&parent_uri).expect("a publish for the parent");
        assert!(
            own.iter()
                .all(|d| !d.message.starts_with("broken cross-reference:")),
            "the partial's defect must not squiggle the parent: {:?}",
            own.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let foreign = by_uri
            .get(&partial_uri)
            .expect("a publish for the partial's URI");
        let d = foreign
            .iter()
            .find(|d| d.message.starts_with("broken cross-reference:"))
            .unwrap_or_else(|| panic!("no broken xref on the partial: {foreign:?}"));
        assert_eq!(
            d.range.start.line, 2,
            "0-based line of `See @fig-nope` in the PARTIAL's own numbering"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A re-publish must retract a foreign URI that no longer carries diagnostics:
    /// nothing else ever publishes for an unopened partial, so a stale squiggle
    /// would stick there forever.
    #[test]
    fn a_republish_clears_the_partials_stale_foreign_diagnostics() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-partialfix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("subsections")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: P\n").unwrap();
        let parent_src =
            "---\ntitle: R\n---\n\n# Intro\n\n{{< include subsections/_intro.tmd >}}\n";
        std::fs::write(dir.join("index.tmd"), parent_src).unwrap();
        std::fs::write(
            dir.join("subsections/_intro.tmd"),
            "Intro prose.\n\nSee @fig-nope for details.\n",
        )
        .unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        let parent_uri = Url::from_file_path(dir.join("index.tmd")).unwrap();
        let partial_uri = Url::from_file_path(dir.join("subsections/_intro.tmd")).unwrap();

        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);
        did_open(&client, &parent_uri, parent_src.to_owned());
        for _ in 0..2 {
            let _ = recv_publish(&client);
        }

        // The author deletes the include: the partial's diagnostics belong nowhere now.
        client
            .sender
            .send(Message::Notification(Notification {
                method: DidChangeTextDocument::METHOD.to_owned(),
                params: serde_json::to_value(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: parent_uri.clone(),
                        version: 2,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: "---\ntitle: R\n---\n\n# Intro\n".to_owned(),
                    }],
                })
                .unwrap(),
            }))
            .unwrap();

        let mut by_uri = std::collections::HashMap::new();
        for _ in 0..2 {
            let p = recv_publish(&client);
            by_uri.insert(p.uri.clone(), p.diagnostics);
        }
        let cleared = by_uri
            .get(&partial_uri)
            .expect("an empty publish retracting the partial's stale squiggles");
        assert!(
            cleared.is_empty(),
            "stale foreign diagnostics must clear: {cleared:?}"
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The created-asset half of the freshness contract: a missing image draws the
    /// "local asset not found" squiggle, the author creates the FILE (no buffer edit
    /// anywhere), and the watched-files notification must clear it through the same
    /// re-publish path a `.tmd` change takes. The registration test below is what
    /// guarantees editors actually send the notification for image files at all.
    #[test]
    fn creating_a_missing_asset_clears_its_squiggle_via_watched_files() {
        let dir = std::env::temp_dir().join(format!("tali-lsp-asset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: P\n").unwrap();
        let a_src = "---\ntitle: A\n---\n\n![system diagram](logo.png)\n";
        std::fs::write(dir.join("a.tmd"), a_src).unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        let a_uri = Url::from_file_path(dir.join("a.tmd")).unwrap();

        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);
        did_open(&client, &a_uri, a_src.to_owned());
        let missing = |diags: &[lsp_types::Diagnostic]| {
            diags
                .iter()
                .any(|d| d.message.starts_with("local asset not found"))
        };
        let opened = recv_publish(&client);
        assert!(
            missing(&opened.diagnostics),
            "the absent image must squiggle first: {:?}",
            opened.diagnostics
        );

        // The fix is a file APPEARING, not an edit.
        std::fs::write(dir.join("logo.png"), b"\x89PNG\r\n").unwrap();
        client
            .sender
            .send(Message::Notification(Notification {
                method: lsp_types::notification::DidChangeWatchedFiles::METHOD.to_owned(),
                params: serde_json::json!({
                    "changes": [{
                        "uri": Url::from_file_path(dir.join("logo.png")).unwrap(),
                        "type": 1,
                    }]
                }),
            }))
            .unwrap();

        let after = recv_publish(&client);
        assert_eq!(after.uri, a_uri);
        assert!(
            !missing(&after.diagnostics),
            "creating the asset must clear the squiggle: {:?}",
            after.diagnostics
        );

        shutdown(&client);
        thread.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The companion registers a watcher of its own (`client.ts`), which is why VS Code
    /// sends `didChangeWatchedFiles` even without dynamic registration; the two glob
    /// lists must watch the same files or the freshness fix silently diverges per editor.
    #[test]
    fn the_companions_watcher_glob_mirrors_the_servers() {
        let ts = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../editor/vscode/src/client.ts"),
        )
        .expect("client.ts is in the tree");
        for want in ["*.tmd", "_site.yml", "*.bib"] {
            assert!(ts.contains(want), "client.ts watcher lost {want}");
        }
        for ext in crate::lsp_complete::IMAGE_EXTS {
            assert!(
                ts.contains(&format!("*.{ext}")),
                "client.ts watcher misses image extension {ext}"
            );
        }
    }

    /// Handling `didChangeWatchedFiles` only helps an editor that SENDS it, and the only
    /// reason VS Code does is that `client.ts` registers a watcher of its own. Every other
    /// editor the docs name — Neovim, Helix, Zed — sends nothing unless the server asks, so
    /// without this registration the freshness fix would have been VS Code-only, in the one
    /// component whose whole design rule is that editor intelligence lives in Rust so that
    /// every client gets it.
    ///
    /// Guarded on the client saying it supports dynamic registration: sending
    /// `client/registerCapability` to a client that does not is a protocol error against a
    /// client that was working fine.
    #[test]
    fn the_server_registers_its_own_file_watchers_when_the_client_allows_it() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        client
            .sender
            .send(Message::Request(Request {
                id: RequestId::from(1),
                method: "initialize".to_owned(),
                params: serde_json::json!({
                    "capabilities": {
                        "workspace": { "didChangeWatchedFiles": { "dynamicRegistration": true } }
                    }
                }),
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

        let req = match client.receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(Message::Request(r)) => r,
            other => panic!("expected a registration request, got {other:?}"),
        };
        assert_eq!(req.method, "client/registerCapability");
        let globs: Vec<String> = req.params["registrations"][0]["registerOptions"]["watchers"]
            .as_array()
            .expect("watchers")
            .iter()
            .map(|w| w["globPattern"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(
            req.params["registrations"][0]["method"],
            "workspace/didChangeWatchedFiles"
        );
        // The file kinds that can invalidate an open buffer's diagnostics without the
        // buffer itself changing — the same set `client.ts` watches.
        for want in ["tmd", "_site.yml", "bib"] {
            assert!(
                globs.iter().any(|g| g.contains(want)),
                "no watcher covers {want}: {globs:?}"
            );
        }
        // And the referenced assets: creating a missing image is the only fix for a
        // "local asset not found" squiggle, and without a watcher for it the stale
        // squiggle sticks until the author's next unrelated edit. One list defines
        // which extensions count: `lsp_complete::IMAGE_EXTS`.
        for want in crate::lsp_complete::IMAGE_EXTS {
            assert!(
                globs.iter().any(|g| g.contains(want)),
                "no watcher covers image extension {want}: {globs:?}"
            );
        }
        // The client answers a registration request; the server must carry on regardless.
        client
            .sender
            .send(Message::Response(Response {
                id: req.id,
                result: Some(serde_json::Value::Null),
                error: None,
            }))
            .unwrap();
        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    /// And the other half of the guard: a client that never claimed dynamic registration is
    /// sent no request at all. `handshake()` (used by every other test here) sends empty
    /// capabilities, so a server that registered unconditionally would push an unexpected
    /// request into all of them.
    #[test]
    fn a_client_without_dynamic_registration_is_not_sent_one() {
        let (server, client) = Connection::memory();
        let thread = std::thread::spawn(move || run(server));
        handshake(&client);
        let uri = Url::from_file_path(corpus("links.tmd")).unwrap();
        did_open(&client, &uri, "---\ntitle: T\n---\n\nBody.\n".to_owned());
        // The next message must be diagnostics, not a registration request.
        let _ = recv_publish(&client);
        shutdown(&client);
        thread.join().unwrap().unwrap();
    }

    // -----------------------------------------------------------------------------------
    // The 3.17 pull model (backlog item 222). The push half above is unchanged and is what
    // an editor predating 3.17 still gets; these own the half that lets the Problems panel
    // list a book rather than only the chapters that happen to be open.
    // -----------------------------------------------------------------------------------

    // -----------------------------------------------------------------------------------
    // `$/cancelRequest` + `$/progress` (backlog item 223).
    // -----------------------------------------------------------------------------------

    /// A superseded request is answered `RequestCancelled` and never executed.
    ///
    /// This is the Ctrl+T case: `workspace/symbol` is a whole-project walk with a `stat` per
    /// page, and it is the one request a user types into character by character, so a
    /// five-letter query queued five walks and ran all five. The `-32800` reply is not a
    /// courtesy either — the protocol owes a response to every id, and a client that never
    /// gets one keeps the pending entry for the session.
    ///
    /// **Every message is queued before the server starts**, and that is not a shortcut: this
    /// test is about which messages arrive *together*, and racing a running server thread to
    /// arrange that is how it would flake. Queued up front, the batch the server drains is
    /// exactly the one written here.
    #[test]
    fn a_cancelled_request_is_answered_rather_than_run() {
        let root = symbol_fixture("cancel", &[("a.tmd", "# Alpha\n"), ("b.tmd", "# Beta\n")]);
        let (server, client) = Connection::memory();
        let uri = Url::from_file_path(root.join("a.tmd")).unwrap();
        let query = |id: i32, uri: &Url| {
            Message::Request(Request {
                id: RequestId::from(id),
                method: lsp_types::request::DocumentSymbolRequest::METHOD.to_owned(),
                params: serde_json::json!({ "textDocument": { "uri": uri } }),
            })
        };
        for msg in [
            Message::Request(Request {
                id: RequestId::from(1),
                method: "initialize".to_owned(),
                params: serde_json::json!({ "capabilities": {} }),
            }),
            Message::Notification(Notification {
                method: "initialized".to_owned(),
                params: serde_json::json!({}),
            }),
            Message::Notification(Notification {
                method: DidOpenTextDocument::METHOD.to_owned(),
                params: serde_json::to_value(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri.clone(),
                        language_id: "taliesin".to_owned(),
                        version: 1,
                        text: "# Alpha\n".to_owned(),
                    },
                })
                .unwrap(),
            }),
            query(10, &uri),
            query(11, &uri),
            Message::Notification(Notification {
                method: "$/cancelRequest".to_owned(),
                params: serde_json::json!({ "id": 10 }),
            }),
            Message::Request(Request {
                id: RequestId::from(99),
                method: "shutdown".to_owned(),
                params: serde_json::Value::Null,
            }),
            Message::Notification(Notification {
                method: "exit".to_owned(),
                params: serde_json::Value::Null,
            }),
        ] {
            client.sender.send(msg).unwrap();
        }
        let thread = std::thread::spawn(move || run(server));

        let mut answers: Vec<(RequestId, Option<i32>)> = Vec::new();
        while let Ok(msg) = client.receiver.recv_timeout(Duration::from_secs(10)) {
            if let Message::Response(r) = msg {
                answers.push((r.id, r.error.map(|e| e.code)));
            }
        }
        thread.join().unwrap().unwrap();
        // ids 1 (initialize) and 99 (shutdown) bracket the two under test.
        let under_test: Vec<(RequestId, Option<i32>)> = answers
            .into_iter()
            .filter(|(id, _)| *id == RequestId::from(10) || *id == RequestId::from(11))
            .collect();
        assert_eq!(
            under_test,
            vec![
                (RequestId::from(10), Some(-32800)),
                (RequestId::from(11), None),
            ],
            "the superseded query comes back cancelled and the live one is still answered"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Cancellation is scoped to one batch, and this is the rule that keeps it so.
    ///
    /// Driven through [`read_batch`] rather than over a live session, because the property is
    /// *which messages were read together* — the one thing a test racing a server thread
    /// cannot pin. (Asking a running server to answer this is what made the first version of
    /// this test flake under load: the cancel and the request it was not meant to reach
    /// sometimes landed in the same batch after all.)
    #[test]
    fn a_cancel_only_reaches_a_request_in_its_own_batch() {
        let query = |id: i32| {
            Message::Request(Request {
                id: RequestId::from(id),
                method: lsp_types::request::DocumentSymbolRequest::METHOD.to_owned(),
                params: serde_json::json!({ "textDocument": { "uri": "file:///x.tmd" } }),
            })
        };
        let cancel = |id: i32| {
            Message::Notification(Notification {
                method: "$/cancelRequest".to_owned(),
                params: serde_json::json!({ "id": id }),
            })
        };
        let batch = |msgs: Vec<Message>| {
            let (server, client) = Connection::memory();
            for m in msgs {
                client.sender.send(m).unwrap();
            }
            drop(client);
            match read_batch(&server, None) {
                Batch::Messages(queue, cancelled) => (queue.len(), cancelled),
                other => panic!(
                    "expected messages, got a {}",
                    match other {
                        Batch::Timeout => "timeout",
                        _ => "closed channel",
                    }
                ),
            }
        };

        // Together: the cancel reaches the request, and is taken out of the queue.
        let (queued, cancelled) = batch(vec![query(10), query(11), cancel(10)]);
        assert_eq!(queued, 2, "the cancel itself is not dispatched");
        assert!(cancelled.contains(&RequestId::from(10)));
        assert!(!cancelled.contains(&RequestId::from(11)));

        // Alone: a cancel naming nothing in this batch is a cancel for something already
        // answered. Remembering it would let a client that reuses an id have live work
        // dropped at random, which is worse than the wasted walk this feature exists to save.
        let (queued, cancelled) = batch(vec![cancel(7), query(7)]);
        assert_eq!(queued, 1);
        assert!(
            cancelled.contains(&RequestId::from(7)),
            "a cancel BEFORE its request in the same batch still reaches it — order within a \
             batch is arrival order, not causality"
        );
        let (_, cancelled) = batch(vec![cancel(7), query(8)]);
        assert!(
            cancelled.is_empty(),
            "a cancel for an id this batch does not carry must be dropped, not remembered"
        );
    }
}
