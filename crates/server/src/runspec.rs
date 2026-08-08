//! What "Run" was asked to run: resolving a user's request to a **block index cap**.
//!
//! The editor says "run the cell my cursor is in" (a line), the CLI says `--cell 3` (an
//! ordinal), and both mean the same thing to [`crate::exec::Executor::run_through`]: run
//! no further than this block. Doing the resolution here, against the rendered blocks the
//! rebuild already produced, means no second parse of the document and no second opinion
//! about which fences are executable cells.
//!
//! The cap is deliberately *inclusive of the named cell*: "Run cell 3" means "make the
//! document true THROUGH cell 3", which is the doc-semantics contract. Everything about
//! how far back the run starts is [`crate::exec::plan`]'s business, not this module's.

use taliesin_core::Block;

/// A run request as the caller expressed it, before it is resolved against a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunScope {
    /// The whole document (`taliesin run <file>`, and every ordinary rebuild).
    All,
    /// Through the executable cell containing (or most recently preceding) this **1-based**
    /// source line. This is what an editor sends: it knows the cursor, not the ordinal.
    ThroughLine(u32),
    /// Through the **1-based** n-th executable cell in document order (`--cell N`).
    ThroughCell(usize),
}

/// The block index this run may not execute past, or `None` for "no cap" ([`RunScope::All`]).
///
/// `Some(i)` caps at block `i`; a request that resolves to no cell at all yields
/// [`Unresolvable`], which the caller reports rather than silently widening to a full run.
/// Silently running the whole document because the cursor was in prose would be the worst
/// possible answer for a cell that takes twenty minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Resolved {
    Uncapped,
    Cap(usize),
    Unresolvable,
}

/// Block indices of the document's executable cells, in document order.
///
/// "Executable" is [`crate::exec::kernel_lang`]'s answer, the same predicate the executor
/// itself uses, so a `{bash}` or `{mermaid}` fence is never counted as a runnable cell and
/// the CLI's `--cell N` cannot mean a different N than the engine's.
fn executable_cells(blocks: &[Block]) -> Vec<usize> {
    blocks
        .iter()
        .enumerate()
        .flat_map(|(i, b)| {
            b.cells()
                .filter(|c| crate::exec::kernel_lang(&c.lang).is_some())
                .map(move |_| i)
        })
        .collect()
}

/// Resolve `scope` against `blocks`.
pub(crate) fn resolve(scope: RunScope, blocks: &[Block]) -> Resolved {
    let cells = executable_cells(blocks);
    match scope {
        RunScope::All => Resolved::Uncapped,
        RunScope::ThroughCell(n) => match n.checked_sub(1).and_then(|i| cells.get(i)) {
            Some(&idx) => Resolved::Cap(idx),
            None => Resolved::Unresolvable,
        },
        RunScope::ThroughLine(line) => {
            // The last cell that STARTS at or before the line. A cursor inside a cell body
            // lands on that cell; a cursor in the prose after it lands on the cell above,
            // which is what "run everything up to where I am" should mean. A cursor above
            // the first cell resolves to nothing rather than to cell 1: there is no cell at
            // or before it, and inventing one would run code the author did not point at.
            match cells
                .iter()
                .rev()
                .find(|&&i| start_line(&blocks[i]) <= line && start_line(&blocks[i]) != 0)
            {
                Some(&idx) => Resolved::Cap(idx),
                None => Resolved::Unresolvable,
            }
        }
    }
}

/// A block's 1-based start line, or 0 when it carries no position (a generated block).
/// Delegates to core so this file holds no second reading of the `L:C-L:C` format.
fn start_line(b: &Block) -> u32 {
    taliesin_core::render::sourcepos_start_line(&b.sourcepos)
}

/// A run request as `taliesin run` sends it over `POST /__taliesin/run`.
///
/// Shared by both servers rather than declared twice, so the single-doc and site paths
/// cannot drift on what the wire means.
#[derive(serde::Deserialize)]
pub(crate) struct RunReq {
    /// Absolute path to the `.tmd` to run. A path, not a page key, because the client is
    /// an editor or a shell: it knows the file it is looking at, not the site's rel.
    pub(crate) file: String,
    /// 1-based source line to run **through**. What an editor sends (it knows the cursor).
    pub(crate) line: Option<u32>,
    /// 1-based ordinal among the document's executable cells, to run **through**.
    pub(crate) cell: Option<usize>,
}

impl RunReq {
    /// The scope this request names. `cell` wins over `line` when both are present: it is
    /// the more explicit of the two, and a client sending both has already decided.
    pub(crate) fn scope(&self) -> RunScope {
        match (self.cell, self.line) {
            (Some(n), _) => RunScope::ThroughCell(n),
            (None, Some(l)) => RunScope::ThroughLine(l),
            (None, None) => RunScope::All,
        }
    }
}

/// The page's live broadcast, forwarded as NDJSON until this run's terminal message.
///
/// Everything the page emits during the run goes out verbatim — the same `cell-state` /
/// `cell-output-append` / `build-state` stream a browser sees — so the terminal and the
/// browser cannot drift on what a cell produced. Only the tail marker is filtered on
/// `runId`, so two concurrent runs cannot end each other's stream.
pub(crate) fn event_stream(
    mut rx: tokio::sync::broadcast::Receiver<String>,
    page: Option<String>,
    run_id: String,
) -> impl futures_util::Stream<Item = Result<String, std::io::Error>> {
    use tokio::sync::broadcast::error::RecvError;
    let (tx, out) = tokio::sync::mpsc::unbounded_channel::<Result<String, std::io::Error>>();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    let terminal = is_run_done_for(&msg, &run_id);
                    if tx.send(Ok(format!("{msg}\n"))).is_err() {
                        break; // the client hung up
                    }
                    if terminal {
                        break;
                    }
                }
                // Lagged: this client fell behind the broadcast ring. Say so rather than
                // drop output silently — a run whose log has holes must not look complete.
                // The run itself is unaffected; only this observer missed messages.
                Err(RecvError::Lagged(n)) => {
                    let note = serde_json::json!({
                        "type": "run-lagged", "page": page, "runId": run_id, "dropped": n
                    })
                    .to_string();
                    if tx.send(Ok(format!("{note}\n"))).is_err() {
                        break;
                    }
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
    futures_util::stream::unfold(out, |mut out| async move {
        out.recv().await.map(|item| (item, out))
    })
}

/// Is `msg` the `run-done` for `run_id`?
///
/// Parsed rather than substring-matched: cell output is arbitrary text, and a cell that
/// merely *prints* the words `run-done` next to a plausible id would otherwise end the
/// stream early and report a run as finished while it was still executing.
fn is_run_done_for(msg: &str, run_id: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(msg)
        .ok()
        .is_some_and(|v| {
            v.get("type").and_then(|t| t.as_str()) == Some("run-done")
                && v.get("runId").and_then(|r| r.as_str()) == Some(run_id)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use taliesin_core::render::Cell;

    /// A block at `line` that is either an executable cell of `lang`, or prose.
    fn blk(line: u32, lang: Option<&str>) -> Block {
        Block {
            id: format!("b-{line}"),
            sourcepos: format!("{line}:1-{line}:9"),
            source_file: None,
            html: String::new(),
            cell: lang.map(|lang| Cell {
                lang: lang.to_string(),
                code: "print(1)".into(),
                figure: None,
                table: None,
                echo: true,
                include: true,
                cache: true,
                js: Default::default(),
            }),
            nested: Vec::new(),
        }
    }

    /// prose(1), python(3), prose(7), r(9), bash(13), python(17)
    fn doc() -> Vec<Block> {
        vec![
            blk(1, None),
            blk(3, Some("python")),
            blk(7, None),
            blk(9, Some("python")),
            blk(13, Some("bash")),
            blk(17, Some("python")),
        ]
    }

    #[test]
    fn run_done_detection_survives_a_cell_that_prints_the_marker() {
        // The reason this parses instead of matching a substring: cell output is arbitrary
        // text and reaches this stream verbatim.
        let real = crate::protocol::run_done(Some("p.tmd"), "abc-123", "ok", None);
        assert!(is_run_done_for(&real, "abc-123"));
        assert!(!is_run_done_for(&real, "other-id"), "must filter on runId");

        let hostile = crate::protocol::cell_output_append(
            Some("p.tmd"),
            "cell-1",
            "append",
            r#"<pre>{"type":"run-done","runId":"abc-123"}</pre>"#,
        );
        assert!(
            !is_run_done_for(&hostile, "abc-123"),
            "a cell PRINTING the terminal message must not end the stream"
        );
        assert!(!is_run_done_for("not json at all", "abc-123"));
    }

    #[test]
    fn cell_wins_over_line_when_a_client_sends_both() {
        let req = RunReq {
            file: "/tmp/x.tmd".into(),
            line: Some(99),
            cell: Some(2),
        };
        assert_eq!(req.scope(), RunScope::ThroughCell(2));
        let req = RunReq {
            file: "/tmp/x.tmd".into(),
            line: Some(99),
            cell: None,
        };
        assert_eq!(req.scope(), RunScope::ThroughLine(99));
        let req = RunReq {
            file: "/tmp/x.tmd".into(),
            line: None,
            cell: None,
        };
        assert_eq!(req.scope(), RunScope::All);
    }

    #[test]
    fn all_never_caps() {
        assert_eq!(resolve(RunScope::All, &doc()), Resolved::Uncapped);
    }

    #[test]
    fn cell_ordinals_skip_non_executable_fences() {
        // `{bash}` is a cell block but no kernel runs it, so it must not consume an
        // ordinal — otherwise `--cell 3` means a different cell to the user than to the
        // engine. Cells are the python@1, python@3, python@5 blocks.
        let d = doc();
        assert_eq!(resolve(RunScope::ThroughCell(1), &d), Resolved::Cap(1));
        assert_eq!(resolve(RunScope::ThroughCell(2), &d), Resolved::Cap(3));
        assert_eq!(resolve(RunScope::ThroughCell(3), &d), Resolved::Cap(5));
    }

    #[test]
    fn an_out_of_range_or_zero_ordinal_is_unresolvable_not_a_full_run() {
        let d = doc();
        assert_eq!(
            resolve(RunScope::ThroughCell(4), &d),
            Resolved::Unresolvable
        );
        // 0 is not a 1-based ordinal. `checked_sub` must not wrap it into a huge index.
        assert_eq!(
            resolve(RunScope::ThroughCell(0), &d),
            Resolved::Unresolvable
        );
    }

    #[test]
    fn a_line_inside_a_cell_resolves_to_that_cell() {
        let d = doc();
        assert_eq!(resolve(RunScope::ThroughLine(3), &d), Resolved::Cap(1));
        // Inside the second cell's body: the last cell starting at or before line 10.
        assert_eq!(resolve(RunScope::ThroughLine(10), &d), Resolved::Cap(3));
        assert_eq!(resolve(RunScope::ThroughLine(17), &d), Resolved::Cap(5));
    }

    #[test]
    fn a_line_in_prose_resolves_to_the_cell_above_it() {
        // "Run everything up to where I am" — the cursor sits after the python cell.
        assert_eq!(resolve(RunScope::ThroughLine(8), &doc()), Resolved::Cap(1));
    }

    #[test]
    fn a_line_above_every_cell_is_unresolvable_rather_than_running_cell_one() {
        // The dangerous default: widening "there is no cell here" into "run the first
        // cell" would execute code the author never pointed at.
        assert_eq!(
            resolve(RunScope::ThroughLine(1), &doc()),
            Resolved::Unresolvable
        );
    }

    #[test]
    fn a_positionless_generated_block_is_never_the_answer() {
        // Output blocks the executor splices in carry an empty sourcepos (line 0). A cap
        // resolving onto one would be meaningless, and `<= line` would otherwise always
        // match it.
        let mut d = doc();
        let mut generated = blk(0, Some("python"));
        generated.sourcepos = String::new();
        d.insert(0, generated);
        assert_eq!(
            resolve(RunScope::ThroughLine(2), &d),
            Resolved::Unresolvable,
            "a block with no source position must not satisfy a line cap"
        );
    }
}
