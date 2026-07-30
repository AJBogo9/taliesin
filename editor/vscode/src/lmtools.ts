// Taliesin's read-only commands as VS Code language-model tools, so a model working in this
// editor can check a document, read what it renders to, and look up the vocabulary instead of
// guessing.
//
// This does not contradict the reverted Ask-AI hand-off. That was rejected because AI belongs
// in the reader's browser extension rather than baked into a published document. This is AI in
// the **editor**, through the platform's own surface, which is the same principle applied
// consistently.
//
// Which tools are offered, and which are deliberately withheld, lives in `lmspecs.ts` with the
// drift gates that keep it honest against `mcp.rs`.

import { execFile } from "node:child_process";
import * as vscode from "vscode";
import { LM_TOOLS, type LmToolSpec } from "./lmspecs";

/** Run one read-only Taliesin subcommand and hand back its stdout. */
function run(binary: string, args: string[], cwd: string | undefined): Promise<string> {
  return new Promise((resolve) => {
    execFile(binary, args, { cwd, maxBuffer: 16 * 1024 * 1024 }, (err, stdout, stderr) => {
      // A non-zero exit is normal for `check`: it exits non-zero whenever it finds anything,
      // which is exactly when the model most wants the output. Only surface an error when
      // there is no output at all to hand back.
      if (stdout.trim()) return resolve(stdout);
      resolve(stderr.trim() || String(err ?? "no output"));
    });
  });
}

function toolFor(spec: LmToolSpec): vscode.LanguageModelTool<{ path?: string }> {
  return {
    invoke: async (options, _token) => {
      const binary = vscode.workspace
        .getConfiguration("taliesin")
        .get<string>("path", "taliesin");
      const target = options.input?.path;
      if (spec.takesPath && !target) {
        return new vscode.LanguageModelToolResult([
          new vscode.LanguageModelTextPart(`${spec.name} needs a path to a .tmd file or project.`),
        ]);
      }
      const args = [...spec.cli];
      if (spec.takesPath && target) args.push(target);
      // Structured output wherever the command offers it: a model parses JSON far more
      // reliably than it re-reads a human summary line.
      if (spec.cli[0] === "check") args.push("--format", "json");
      const out = await run(binary, args, vscode.workspace.workspaceFolders?.[0]?.uri.fsPath);
      return new vscode.LanguageModelToolResult([new vscode.LanguageModelTextPart(out)]);
    },
  };
}

/** Register every offered tool. */
export function registerLmTools(context: vscode.ExtensionContext): void {
  for (const spec of LM_TOOLS) {
    context.subscriptions.push(vscode.lm.registerTool(spec.name, toolFor(spec)));
  }
}
