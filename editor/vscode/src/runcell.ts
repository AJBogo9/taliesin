import * as path from "node:path";
import * as vscode from "vscode";
import { isSourceFile } from "./paths";
import { TASK_TYPE } from "./tasks";
import { runSpec, runOutcome } from "./taskspecs";

// Running a code cell: the task, the progress indicator, and the completion notification.
//
// **The buttons are not here any more.** This file used to hold a `CodeLensProvider` that
// asked the server for `taliesin/cellRegions` and drew Run / Run Above over each executable
// fence — which meant the execution loop reached VS Code and no other editor. The server
// answers `textDocument/codeLens` now (`crates/server/src/lsp_lens.rs`), so the buttons come
// over the protocol, in every LSP client, and they carry a `⚡ cached` label this side could
// never have computed. What is left is the half a code lens cannot express: a lens names a
// command, and *running* one is the client's job.
//
// Execution itself is `taliesin run` as a **task**. Nothing about the kernel, the cache, or
// the session lives on this side: the CLI attaches to the project's warm session, and this
// file only decides which line to point it at, and watches for the end of the run.

function binaryPath(): string {
  return vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");
}

/**
 * `taliesin run <file> --line <L>` (or `--all`) as a task.
 *
 * **Still a real terminal, and that part is not negotiable**: the run's own formatting —
 * colours, the in-place `\r` progress redraw, the ctrl-clickable figure paths — is written
 * for a terminal, and piping it through an output channel would strip exactly the parts that
 * make it readable. A task terminal is a pty (measured: a task process sees `isatty()` true),
 * so all three survive. What a task adds over `terminal.sendText` is the thing the extension
 * could not learn any other way: **that the run ended, and with what exit code**.
 *
 * `ProcessExecution`, not `ShellExecution`, and that is measured rather than preference: the
 * same argv given to a shell task came back exit 1 with the work never done, because the
 * shell re-split arguments the old code had to hand-quote POSIX-style — and a document path
 * is arbitrary text. Run directly, each argument reaches the binary exactly as spelled.
 *
 * **The problem matchers, and the one condition on them.** `run_print.rs` now prints a failed
 * cell twice: `✗ cell 3/12` for the human reading the terminal, and
 * `posts/a.tmd:12: error: …` for the editor reading over their shoulder — which is exactly the
 * shape `$taliesin` already matches. That was the change this file used to say was owed ("a
 * change to what `run` PRINTS, in Rust, not a matcher declared here"), so the matchers can
 * finally be attached.
 *
 * They are still **not free to attach on spec**: measured, a task carrying them in a window with
 * no workspace folder (where `${workspaceFolder}` cannot resolve) reported no process at all,
 * while the same task without them ran and reported its exit code. So they go on only when a
 * folder exists — losing the Problems entry in a folderless window, never the run.
 */
export function runTask(
  file: string,
  target: number | "all",
  cwd: string,
  binary: string
): vscode.Task {
  const spec = runSpec(file, target);
  // Scope to the folder the run actually happens in when there is one, as `tasks.ts` does.
  const folder = vscode.workspace.workspaceFolders?.find((f) => f.uri.fsPath === cwd);
  const task = new vscode.Task(
    // The file and the target ride in the definition because VS Code keys a task's IDENTITY
    // off it. Measured: executing a task whose identity is already running registers a
    // second execution that never starts — so a definition of `{type, command}` alone would
    // mean that asking for cell 40 while cell 3 is still going, or running a cell in a
    // second chapter, silently did nothing at all. It is also what `endOf` recognises the
    // run by, without depending on object identity surviving a trip through the platform.
    { type: TASK_TYPE, command: spec.name, file, target: String(target) },
    folder ?? vscode.TaskScope.Workspace,
    spec.name,
    TASK_TYPE,
    new vscode.ProcessExecution(binary, spec.args, { cwd }),
    // Both matchers, and only with a folder — see the note above. `run` emits located cell
    // failures; the unlocated one is there for the same reason `tasks.ts` carries it, so a
    // finding with no line is reported rather than silently dropped.
    folder ? ["$taliesin", "$taliesin-unlocated"] : undefined
  );
  task.presentationOptions = {
    // Task terminals do not reuse by name the way `createTerminal({ name })` did, so this is
    // the nearest equivalent: one shared panel, wiped at the start of each run.
    reveal: vscode.TaskRevealKind.Always,
    panel: vscode.TaskPanelKind.Shared,
    clear: true,
    // The author asked to run a cell, not to leave the editor: `terminal.show(true)` used to
    // preserve focus and this keeps that.
    focus: false,
    showReuseMessage: false,
  };
  return task;
}

/** The document's own directory, which is where the run should be rooted from. */
function cwdFor(doc: vscode.TextDocument): string {
  const folder = vscode.workspace.getWorkspaceFolder(doc.uri);
  return folder ? folder.uri.fsPath : vscode.Uri.joinPath(doc.uri, "..").fsPath;
}

/** Runs this window has started and not yet seen the end of, keyed by document and target. */
const inFlight = new Set<string>();

/**
 * Start a run, hold a progress indicator for as long as it lasts, and report how it ended.
 *
 * The progress indicator is half the point. CHI 2020's long-running-task complaint is that a
 * running computation "provides no feedback on progress", and until now an editor-side run
 * was indistinguishable from a run that never started. This is the client's half — a
 * spinner for the duration and a notification at the end. Per-cell progress needs the
 * server's `ProgressSink`, which today reaches the browser only.
 */
async function startRun(doc: vscode.TextDocument, target: number | "all"): Promise<void> {
  const key = `${doc.uri.fsPath} ${target}`;
  if (inFlight.has(key)) {
    // VS Code would drop this one on the floor (same identity, still running). Say so
    // rather than let the button look broken.
    vscode.window.showInformationMessage("Taliesin: that run is already going.");
    return;
  }
  const task = runTask(doc.uri.fsPath, target, cwdFor(doc), binaryPath());
  const started = Date.now();
  // Watch BEFORE executing. A cell that finishes in milliseconds can end before an await on
  // `executeTask` has come back, and a listener attached afterwards would miss the only event
  // it exists for: the spinner would then run forever and `inFlight` would keep that cell
  // from ever being run again this session.
  const watch = endOf(task);
  try {
    await vscode.tasks.executeTask(task);
  } catch (e) {
    watch.cancel();
    vscode.window.showErrorMessage(
      `Taliesin: could not start the run (${(e as Error).message}). ` +
        `Check that "taliesin.path" points at the binary.`
    );
    return;
  }
  inFlight.add(key);
  const where = path.basename(doc.uri.fsPath);
  const title =
    target === "all" ? `Taliesin: running ${where}` : `Taliesin: running ${where} (line ${target})`;
  const exitCode = await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Window, title },
    () => watch.ended
  );
  inFlight.delete(key);

  const outcome = runOutcome(exitCode, Date.now() - started);
  if (outcome.kind === "error") vscode.window.showErrorMessage(outcome.message);
  else if (outcome.kind === "info") vscode.window.showInformationMessage(outcome.message);
}

/**
 * Watch for the end of `task`: its exit code, or `undefined` if it ended without one.
 *
 * Recognised by its DEFINITION rather than by the `TaskExecution` object, so that the watch
 * can be armed before `executeTask` is even called — see the race at the call site. The
 * definition names the file and the target, so it identifies this run and no other.
 *
 * Listening for the TASK end as well as the process end is what keeps the progress indicator
 * honest: a task the system declines to start (measured in a folderless window, where the
 * task system is inert by VS Code's own rule) ends without ever reporting a process, and a
 * promise waiting only on `onDidEndTaskProcess` would leave a spinner running for the rest
 * of the session. The process event fires first when there is one, so it wins.
 */
function endOf(task: vscode.Task): { ended: Promise<number | undefined>; cancel: () => void } {
  const subs: vscode.Disposable[] = [];
  const cancel = () => subs.splice(0).forEach((s) => s.dispose());
  const isThisRun = (other: vscode.Task) =>
    other.definition.type === task.definition.type &&
    other.definition.command === task.definition.command &&
    other.definition.file === task.definition.file &&
    other.definition.target === task.definition.target;
  const ended = new Promise<number | undefined>((resolve) => {
    const settle = (code: number | undefined) => {
      cancel();
      resolve(code);
    };
    subs.push(
      vscode.tasks.onDidEndTaskProcess((e) => {
        if (isThisRun(e.execution.task)) settle(e.exitCode);
      }),
      vscode.tasks.onDidEndTask((e) => {
        if (isThisRun(e.execution.task)) settle(undefined);
      })
    );
  });
  return { ended, cancel };
}

export function registerRunCell(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    // Invoked by the SERVER'S code lens (with args) and from the command palette (without),
    // where the cursor names the cell instead. The lens used to be built here; it is
    // `crates/server/src/lsp_lens.rs` now, so every LSP editor gets the buttons and this
    // file keeps only the plumbing a lens cannot express — the task, the progress indicator
    // and the completion notification.
    vscode.commands.registerCommand(
      "taliesin.runCell",
      async (uri?: vscode.Uri | string, line?: number) => {
        const editor = vscode.window.activeTextEditor;
        // A code lens's arguments are plain JSON, so the server sends the URI as a string.
        // `openTextDocument(string)` would read it as a *file path* and open a file called
        // `file:///…`, so parse it back first.
        const target = typeof uri === "string" ? vscode.Uri.parse(uri) : uri;
        const doc = target
          ? await vscode.workspace.openTextDocument(target)
          : editor?.document;
        if (!doc || !isSourceFile(doc.fileName)) {
          vscode.window.showWarningMessage("Open a .tmd file to run a cell.");
          return;
        }
        const through = line ?? (editor ? editor.selection.active.line + 1 : 1);
        await doc.save();
        await startRun(doc, through);
      }
    ),
    vscode.commands.registerCommand("taliesin.runAll", async () => {
      const doc = vscode.window.activeTextEditor?.document;
      if (!doc || !isSourceFile(doc.fileName)) {
        vscode.window.showWarningMessage("Open a .tmd file to run it.");
        return;
      }
      await doc.save();
      await startRun(doc, "all");
    })
  );
}
