// The Taliesin status bar item: is a preview running for this project, and how healthy is it.
//
// The wording lives in `statustext.ts` as pure functions; this file is only the VS Code wiring.
// See that file for why live kernel and cache state is deliberately absent.

import * as vscode from "vscode";
import { isSourceFile, projectRootFor } from "./paths";
import type { PreviewRegistry } from "./previews";
import { previewKey } from "./previews";
import { statusText, statusTooltip, type StatusState } from "./statustext";

/** Show the item for `.tmd` work, and keep it current. */
export function registerStatusBar(
  context: vscode.ExtensionContext,
  previews: PreviewRegistry,
  /** Subscribe to the problem count the decoration provider already computes. */
  onProblemCount: (listener: (count: number | null) => void) => void
): void {
  const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  item.command = "taliesin.openPreview";
  context.subscriptions.push(item);

  let problems: number | null = null;

  const update = (): void => {
    const doc = vscode.window.activeTextEditor?.document;
    const file = doc?.uri.scheme === "file" ? doc.fileName : undefined;
    if (!file || !isSourceFile(file)) {
      // Hide rather than go stale: the item is about the document in front of you, and one
      // left showing another project's port is worse than no item at all.
      item.hide();
      return;
    }
    const root = projectRootFor(file);
    const live = previews.get(previewKey(file, root)) ?? previews.get(file);
    const state: StatusState = { previewPort: live?.server.port ?? null, problems };
    item.text = statusText(state);
    item.tooltip = statusTooltip(state);
    item.show();
  };

  // Fed by the decoration provider, which already runs `check` for this project: a second
  // run here would double the cost for the same number. Deliberately a plain callback rather
  // than an internal command, so nothing has to be declared in the manifest that no author
  // should ever see in the palette.
  onProblemCount((count) => {
    problems = count;
    update();
  });

  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor(() => update()),
    vscode.workspace.onDidSaveTextDocument((d) => {
      if (isSourceFile(d.fileName)) update();
    })
  );
  update();
}
