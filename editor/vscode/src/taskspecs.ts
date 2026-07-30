// Which Taliesin tasks exist for a project, as a pure function.
//
// Split from `tasks.ts` for the same reason `pastekind.ts` is split from `insert.ts`: no
// `vscode` import here, so `node --test` can check it against the manifest without an
// Extension Host.

/** One offered task: the name VS Code shows, and the argv it runs. */
export interface TaskSpec {
  name: string;
  args: string[];
}

/**
 * The three tasks offered for a project root.
 *
 * Every one targets the **root**, never a single file. `check <file.tmd>` is a narrower thing
 * that cannot see cross-page anchors, so it reports every legitimate cross-chapter reference
 * as broken; offering it as a task would put those false positives in the Problems panel.
 */
export function taskSpecs(root: string): TaskSpec[] {
  return [
    { name: "check", args: ["check", root] },
    { name: "build", args: ["build", root] },
    // Relative to the project, which is where `build --out` already writes when run by hand.
    { name: "build --out", args: ["build", root, "--out", "_site"] },
  ];
}
