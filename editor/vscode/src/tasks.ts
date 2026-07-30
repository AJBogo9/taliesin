// Taliesin's build and check commands as VS Code tasks, so project-wide health does not need
// a trip to the terminal.
//
// The point is the problem matcher more than the tasks: `check` finds problems on pages that
// are **not open**, and the language server only ever diagnoses buffers it has been sent. Run
// the check task and those findings land in the Problems panel with real file locations.
//
// The task shapes themselves are a pure function in `taskspecs.ts`, unit-tested against the
// manifest so the offered set and the declared `enum` cannot drift apart.

import * as path from "node:path";
import * as vscode from "vscode";
import { projectRootFor, isSourceFile } from "./paths";
import { taskSpecs, type TaskSpec } from "./taskspecs";

const TASK_TYPE = "taliesin";

/** True when `target` is `dir` or sits under it, by directory boundary rather than by prefix. */
function isInside(dir: string, target: string): boolean {
  const rel = path.relative(path.resolve(dir), path.resolve(target));
  return rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel));
}

/** The project root the tasks should target, or `null` when nothing tells us one. */
function activeRoot(): string | null {
  const doc = vscode.window.activeTextEditor?.document;
  if (doc?.uri.scheme === "file" && isSourceFile(doc.fileName)) {
    const root = projectRootFor(doc.fileName);
    if (root) return root;
  }
  // Fall back to the workspace folder: the author may be looking at a terminal or a preview
  // rather than at a `.tmd`, and a task list that empties itself when focus moves is useless.
  const folder = vscode.workspace.workspaceFolders?.[0];
  return folder ? folder.uri.fsPath : null;
}

function buildTask(spec: TaskSpec, root: string, binary: string): vscode.Task {
  // Scope to the workspace folder the project actually lives in when there is one, so a
  // multi-root workspace runs the task against the right folder rather than the first.
  //
  // No `Global` fallback: measured in a real Extension Host with no folder open,
  // `vscode.tasks.fetchTasks()` returns zero tasks of ANY type, so VS Code's task system is
  // inert without a workspace regardless of the scope a provider asks for. A `Global` scope
  // there would be dead code dressed as a fix.
  const folder = vscode.workspace.workspaceFolders?.find((f) => isInside(f.uri.fsPath, root));
  const task = new vscode.Task(
    { type: TASK_TYPE, command: spec.name },
    folder ?? vscode.TaskScope.Workspace,
    spec.name,
    TASK_TYPE,
    new vscode.ShellExecution(binary, spec.args, { cwd: root }),
    // Both matchers: `check` emits located and unlocated diagnostics, and a run that reported
    // only the located half would quietly drop every `_site.yml` finding.
    ["$taliesin", "$taliesin-unlocated"]
  );
  task.group = spec.name === "check" ? vscode.TaskGroup.Test : vscode.TaskGroup.Build;
  task.presentationOptions = {
    reveal: vscode.TaskRevealKind.Silent,
    panel: vscode.TaskPanelKind.Shared,
    clear: true,
  };
  return task;
}

/** Offer `check`, `build` and `build --out` for the active project. */
export function registerTasks(context: vscode.ExtensionContext): void {
  const binary = () =>
    vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");

  context.subscriptions.push(
    vscode.tasks.registerTaskProvider(TASK_TYPE, {
      provideTasks: () => {
        const root = activeRoot();
        if (!root) return [];
        return taskSpecs(root).map((s) => buildTask(s, root, binary()));
      },
      // Called for a task the user wrote into `tasks.json` by hand, where VS Code hands back
      // the definition and expects the execution filled in. Returning `undefined` for one we
      // do not recognise leaves it to whoever does.
      resolveTask: (task: vscode.Task) => {
        const command = (task.definition as { command?: string }).command;
        const root = activeRoot();
        if (!command || !root) return undefined;
        const spec = taskSpecs(root).find((s) => s.name === command);
        return spec ? buildTask(spec, root, binary()) : undefined;
      },
    })
  );
}
