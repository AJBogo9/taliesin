// The first time a cell fails to run for want of a kernel, offer `taliesin doctor`.
//
// Setup is the first pain point a new author hits, and the error-message literature
// (Becker/Denny) is that what makes a message useful is the actionable next step. `doctor`
// answers exactly this diagnostic — it audits interpreters, kernels and what is missing —
// but it has been reachable only from the command palette and the walkthrough, i.e. only by
// someone who already knows it exists.
//
// **A notification, not a code action.** A code action is a language feature, and language
// features live in `taliesin lsp` so that every editor gets them. But a server-side action
// would have to name a VS Code command id in its payload, which couples the engine to one
// client — so this one stays here, where the client-specific knowledge belongs.
//
// The decision itself (what counts as a kernel failure, and the once-per-session latch) is
// in `kernelfail.ts`, which imports no `vscode` and is unit-tested.

import * as vscode from "vscode";
import { KernelPrompt } from "./kernelfail";

/** The label on the button, and the command behind it (contributed in `commands.ts`). */
const RUN_DOCTOR = "Run doctor";
const DOCTOR_COMMAND = "taliesin.doctor";

export function registerDoctorHint(context: vscode.ExtensionContext): void {
  const prompt = new KernelPrompt();
  context.subscriptions.push(
    // Every publisher of diagnostics is watched, not just the language server: a kernel
    // failure reaches the editor from whichever of them saw the cell fail, and which one
    // that was is not something this side can tell (nor should care).
    vscode.languages.onDidChangeDiagnostics((e) => {
      for (const uri of e.uris) {
        const message = prompt.offer(vscode.languages.getDiagnostics(uri));
        if (message === null) continue;
        void offerDoctor(message);
        return;
      }
    })
  );
}

/** The engine's own sentence, plus the one thing the author can do about it. */
async function offerDoctor(message: string): Promise<void> {
  const picked = await vscode.window.showWarningMessage(`Taliesin: ${message}`, RUN_DOCTOR);
  if (picked === RUN_DOCTOR) await vscode.commands.executeCommand(DOCTOR_COMMAND);
}
