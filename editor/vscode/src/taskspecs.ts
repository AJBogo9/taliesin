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
 * *names* only. `check` prints every path re-rooted on the target as typed, so running
 * `taliesin check docs/guide` from the folder prints `docs/guide/sub/page.tmd:5:`, which
 * resolves against `${workspaceFolder}`.
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
 * Every one targets the **root**, never a single file. `check <file.tmd>` is a narrower thing
 * that cannot see cross-page anchors, so it reports every legitimate cross-chapter reference
 * as broken; offering it as a task would put those false positives in the Problems panel.
 *
 * `target` is the project **as reachable from the directory the task runs in**, which is the
 * workspace folder — `docs/guide`, or `.` when the folder is itself the project. That is what
 * makes the problem matcher work: `check` prints each path re-rooted on the target as typed,
 * so a matcher based at `${workspaceFolder}` resolves it. Handing it an absolute root instead
 * would print absolute paths, and handing it a project-relative one would print paths relative
 * to a directory VS Code cannot name.
 */
export function taskSpecs(target: string): TaskSpec[] {
  return [
    { name: "check", args: ["check", target] },
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

/**
 * `taliesin run` for one cell, or for the whole document.
 *
 * Not part of `taskSpecs`, and deliberately: those are project-wide and are what the task
 * picker offers, while a run needs a file and a cursor line. It is a `TaskSpec` all the same
 * because it is executed as a **task**, so its `command` is checked against the manifest's
 * `taskDefinitions` enum by the same drift gate — the mismatch class that kept the companion
 * silently inert for months.
 *
 * `--line` rather than `--cell N` because the editor knows the cursor, and an ordinal
 * computed here would be a second copy of "which fences count" (`taliesin/cellRegions` owns
 * that). The path is passed as one argv element and never quoted: the task runs the binary
 * directly, with no shell to re-split it.
 */
export function runSpec(file: string, target: number | "all"): TaskSpec {
  return {
    name: "run",
    args: target === "all" ? ["run", file, "--all"] : ["run", file, "--line", String(target)],
  };
}

/** What to tell the author when a run ends, if anything. */
export interface RunOutcome {
  kind: "error" | "info" | "silent";
  message: string;
}

/**
 * A run finished after `elapsedMs` with `exitCode`. Say what?
 *
 * Three answers, and "nothing" is the common one. The notification exists for CHI 2020's
 * long-running-task complaint ("when the process is done, it automatically creates a
 * notification"), which is about the run you walked away from — a toast after every
 * 300 ms cell would be noise that trains the author to dismiss without reading.
 *
 * `exitCode` is `undefined` in two cases, and silence is right for both. VS Code reports it
 * for a process that was **terminated** rather than exited — the author stopped the run, which
 * is not a failure to announce. And measured: the very first task executed in a window reports
 * no exit code whatever it is, so the first run of a session ends quietly.
 */
export function runOutcome(exitCode: number | undefined, elapsedMs: number): RunOutcome {
  if (exitCode === undefined) return { kind: "silent", message: "" };
  if (exitCode !== 0) {
    return {
      kind: "error",
      message: `Taliesin: the run failed (exit ${exitCode}). The run terminal has the detail.`,
    };
  }
  if (elapsedMs < NOTIFY_AFTER_MS) return { kind: "silent", message: "" };
  return { kind: "info", message: `Taliesin: run finished in ${formatElapsed(elapsedMs)}.` };
}

/** Long enough that the author has plausibly looked away, short enough to still be the run. */
const NOTIFY_AFTER_MS = 10_000;

/** `12.3 s`, or `2 min 5 s` once seconds stop being a number anyone reads. */
function formatElapsed(ms: number): string {
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)} s`;
  const whole = Math.round(seconds);
  return `${Math.floor(whole / 60)} min ${whole % 60} s`;
}
