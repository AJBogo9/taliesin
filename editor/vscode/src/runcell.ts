import * as path from "node:path";
import * as vscode from "vscode";
import { isSourceFile } from "./paths";
import { languageClient } from "./client";
import { TASK_TYPE } from "./tasks";
import { runSpec, runOutcome } from "./taskspecs";

// The Run / Run Above buttons over every executable code cell.
//
// The buttons are the only new thing here. Both the cell positions and the decision about
// which fences are runnable come from the server (`taliesin/cellRegions`), because a fence
// scan and an executable-language list in TypeScript are exactly the second copies the LSP
// rewrite existed to delete — and a Run button over a `{bash}` fence, or a missing one over
// `{python}`, is what that drift looks like to an author.
//
// Execution itself is `taliesin run` as a **task**. Nothing about the kernel, the cache, or
// the session lives on this side: the CLI attaches to the project's warm session, and this
// file only decides which line to point it at — and, now, watches for the end of the run.

interface CellRegion {
  language: string;
  startLine: number;
  endLine: number;
  executable: boolean;
}

/** The custom request the server answers. Must match `lsp::CELL_REGIONS_METHOD`. */
const CELL_REGIONS = "taliesin/cellRegions";

function binaryPath(): string {
  return vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");
}

async function cellRegions(doc: vscode.TextDocument): Promise<CellRegion[]> {
  const client = languageClient();
  if (!client) return [];
  try {
    const regions = await client.sendRequest<CellRegion[]>(CELL_REGIONS, {
      textDocument: { uri: doc.uri.toString() },
    });
    return regions ?? [];
  } catch {
    // The server is starting, or this buffer is not one it tracks. No lenses is the right
    // answer; a thrown provider would show an error banner on every keystroke.
    return [];
  }
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
 * **No problem matcher.** `tasks.ts` attaches `$taliesin`/`$taliesin-unlocated` because
 * `check` prints `file.tmd:12: error[CODE]: …`, which they match. `taliesin run` does not:
 * `run_print.rs` prints `✗ cell 3` and an indented `error   cell 3 failed`, and both patterns
 * are anchored on a path at the start of the line, so neither can match a word of it. They
 * are not free to attach on spec either — measured, a task carrying them in a window with no
 * workspace folder (where `${workspaceFolder}` cannot resolve) reported no process at all,
 * while the same task without them ran and reported its exit code. Getting a failed cell into
 * the Problems panel is a change to what `run` PRINTS, in Rust, not a matcher declared here.
 */
export function runTask(
  file: string,
  target: number | "all",
  cwd: string,
  binary: string
): vscode.Task {
  const spec = runSpec(file, target);
  const task = new vscode.Task(
    // The file and the target ride in the definition because VS Code keys a task's IDENTITY
    // off it. Measured: executing a task whose identity is already running registers a
    // second execution that never starts — so a definition of `{type, command}` alone would
    // mean that asking for cell 40 while cell 3 is still going, or running a cell in a
    // second chapter, silently did nothing at all. It is also what `endOf` recognises the
    // run by, without depending on object identity surviving a trip through the platform.
    { type: TASK_TYPE, command: spec.name, file, target: String(target) },
    // Scope to the folder the run actually happens in when there is one, as `tasks.ts` does.
    vscode.workspace.workspaceFolders?.find((f) => f.uri.fsPath === cwd) ??
      vscode.TaskScope.Workspace,
    spec.name,
    TASK_TYPE,
    new vscode.ProcessExecution(binary, spec.args, { cwd })
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

const provider: vscode.CodeLensProvider = {
  async provideCodeLenses(doc) {
    if (!isSourceFile(doc.fileName)) return [];
    const regions = await cellRegions(doc);
    const lenses: vscode.CodeLens[] = [];
    const runnable = regions.filter((r) => r.executable);
    runnable.forEach((r, i) => {
      // Anchor on the fence line (the body's first line minus one), so the buttons sit
      // above the cell rather than over its first statement.
      const anchor = new vscode.Range(Math.max(0, r.startLine - 1), 0, Math.max(0, r.startLine - 1), 0);
      // 1-based for the CLI, and the body's first line is inside the cell, which is what
      // `--line` resolves against.
      const line = r.startLine + 1;
      lenses.push(
        new vscode.CodeLens(anchor, {
          title: "▶ Run Cell",
          command: "taliesin.runCell",
          arguments: [doc.uri, line],
          tooltip: `Run this ${r.language} cell (and any earlier cell the kernel is missing)`,
        })
      );
      // "Run Above" is only meaningful when there IS something above: on the first cell it
      // would run nothing and read as a broken button.
      if (i > 0) {
        const prev = runnable[i - 1];
        lenses.push(
          new vscode.CodeLens(anchor, {
            title: "Run Above",
            command: "taliesin.runCell",
            arguments: [doc.uri, prev.startLine + 1],
            tooltip: "Run every cell before this one",
          })
        );
      }
    });
    return lenses;
  },
};

export function registerRunCell(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider({ language: "taliesin" }, provider),
    // Invoked by the lens (with args) and from the command palette (without), where the
    // cursor names the cell instead.
    vscode.commands.registerCommand(
      "taliesin.runCell",
      async (uri?: vscode.Uri, line?: number) => {
        const editor = vscode.window.activeTextEditor;
        const doc = uri
          ? await vscode.workspace.openTextDocument(uri)
          : editor?.document;
        if (!doc || !isSourceFile(doc.fileName)) {
          vscode.window.showWarningMessage("Open a .tmd file to run a cell.");
          return;
        }
        const target = line ?? (editor ? editor.selection.active.line + 1 : 1);
        await doc.save();
        await startRun(doc, target);
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
