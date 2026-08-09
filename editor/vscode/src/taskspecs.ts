// Which Taliesin tasks exist, where they run, and what to say when one ends, as pure
// functions.
//
// Split from `tasks.ts` for the same reason `pastekind.ts` is split from `insert.ts`: no
// `vscode` import here, so `node --test` can check it against the manifest without an
// Extension Host.

import * as path from "node:path";

/** One offered task: the name VS Code shows, and the argv it runs. */
export interface TaskSpec {
  name: string;
  args: string[];
}

/** True when `target` is `dir` or sits under it, by directory boundary rather than by prefix. */
export function isInside(dir: string, target: string): boolean {
  const rel = path.relative(path.resolve(dir), path.resolve(target));
  return rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel));
}

/**
 * Where the task runs, and what it calls the project from there.
 *
 * The task runs in the **workspace folder**, not the project root, because that is the only
 * directory a problem matcher can name: its `fileLocation` base is `${workspaceFolder}`, VS
 * Code has no variable meaning "the Taliesin project" (`${cwd}` resolves to the workspace
 * folder too, not to the task's cwd — read off `AbstractVariableResolverService`), and a
 * provider cannot supply a base in code either, since `Task.problemMatchers` takes matcher
 * *names* only. The lint prints every path re-rooted on the target as typed, so running
 * `taliesin build docs/guide --check-only` from the folder prints `docs/guide/sub/page.tmd:5:`,
 * which resolves against `${workspaceFolder}`.
 *
 * With no folder containing the project, the root is both cwd and target. That case has no
 * `${workspaceFolder}` to resolve against either, so the matcher can only ever be right for a
 * project inside the workspace.
 */
export function taskLocation(
  root: string,
  folders: readonly string[]
): { cwd: string; target: string } {
  const folder = folders.find((f) => isInside(f, root));
  if (!folder) return { cwd: root, target: root };
  // `.` rather than "", which argv would pass through as an empty argument.
  return { cwd: folder, target: path.relative(folder, root) || "." };
}

/**
 * The three tasks offered for a project root.
 *
 * Every one targets the **root**, never a single file: a single-file lint cannot see
 * cross-page anchors the way a project lint can, so offering one as a task would put
 * legitimate cross-chapter references in the Problems panel as false positives.
 *
 * `target` is the project **as reachable from the directory the task runs in**, which is the
 * workspace folder — `docs/guide`, or `.` when the folder is itself the project. That is what
 * makes the problem matcher work: the lint prints each path re-rooted on the target as typed,
 * so a matcher based at `${workspaceFolder}` resolves it. Handing it an absolute root instead
 * would print absolute paths, and handing it a project-relative one would print paths relative
 * to a directory VS Code cannot name.
 *
 * `--check-only` is the front door the retired `check` verb was: it renders in memory, reports
 * every located diagnostic and writes nothing, so the lint task cannot leave a `_site/` behind.
 */
export function taskSpecs(target: string): TaskSpec[] {
  return [
    { name: "check", args: ["build", target, "--check-only"] },
    { name: "build", args: ["build", target] },
    // `--out` resolves against the PROCESS CWD, not the project (measured), so the target has
    // to be spelled into it. Left bare it followed the cwd to the workspace folder and wrote
    // the built site to the repository root.
    { name: "build --out", args: ["build", target, "--out", outDir(target)] },
  ];
}

/** `_site` under the target, without a `./` when the target is the working directory itself. */
function outDir(target: string): string {
  return target === "." ? "_site" : `${target.replace(/\/+$/, "")}/_site`;
}
