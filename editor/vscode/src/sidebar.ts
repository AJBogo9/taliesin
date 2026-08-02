// The Taliesin sidebar: three read-only views over the project, fed entirely by the server.
//
// A `TreeView` is one of the few things LSP has no concept of, which is why this lives in
// TypeScript at all. The tree *shapes* are pure functions in `sidebartree.ts`; this file is
// only the VS Code wiring.
//
// **Read-only, and not by accident.** Every row navigates and nothing edits. No drag-to-reorder
// of chapters, no rename-in-tree, no delete. A panel that writes back to the source is the
// invariant this project removed a slide-reorder feature to protect, and a tree with drag
// handles is that same mistake wearing a different costume.

import * as vscode from "vscode";
import { languageClient } from "./client";
import { isSourceFile } from "./paths";
import {
  floatsTree,
  outlineTree,
  refsTree,
  type OutlineReply,
  type RefsReply,
  type TreeRow,
} from "./sidebartree";

/** A `TreeDataProvider` over rows produced by one of the builders above. */
class RowProvider implements vscode.TreeDataProvider<TreeRow> {
  private rows: TreeRow[] = [];
  private readonly changed = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.changed.event;

  constructor(private readonly emptyRows: () => TreeRow[]) {
    this.rows = emptyRows();
  }

  replace(rows: TreeRow[]): void {
    this.rows = rows;
    this.changed.fire();
  }

  clear(): void {
    this.replace(this.emptyRows());
  }

  getChildren(row?: TreeRow): TreeRow[] {
    return row ? row.children : this.rows;
  }

  getTreeItem(row: TreeRow): vscode.TreeItem {
    const item = new vscode.TreeItem(
      row.label,
      row.children.length
        ? row.collapsed
          ? vscode.TreeItemCollapsibleState.Collapsed
          : vscode.TreeItemCollapsibleState.Expanded
        : vscode.TreeItemCollapsibleState.None
    );
    item.description = row.description;
    if (row.path !== undefined) {
      const at = new vscode.Position(row.line ?? 0, 0);
      // `vscode.open` with a selection: navigation only. Nothing in this tree may offer an
      // edit, so there is deliberately no other command wired to a row.
      item.command = {
        command: "vscode.open",
        title: "Open",
        arguments: [vscode.Uri.file(row.path), { selection: new vscode.Range(at, at) }],
      };
      item.resourceUri = vscode.Uri.file(row.path);
    }
    return item;
  }

  dispose(): void {
    this.changed.dispose();
  }
}

/** Register the three views and keep them in step with the active document. */
export function registerSidebar(context: vscode.ExtensionContext): void {
  const outline = new RowProvider(() => []);
  const refs = new RowProvider(() => refsTree(null));
  const floats = new RowProvider(() => []);

  context.subscriptions.push(
    outline,
    refs,
    floats,
    // `showCollapseAll` on the two views that nest. The float index is flat by construction
    // (`floatsTree` gives every row zero children), so a button there could never do
    // anything, and one that cannot act is worse than none. VS Code registers a
    // `workbench.actions.treeView.<id>.collapseAll` command per view that asks for it,
    // which is what the e2e asserts — the option object itself is write-only from here.
    vscode.window.createTreeView("taliesin.outline", {
      treeDataProvider: outline,
      showCollapseAll: true,
    }),
    vscode.window.createTreeView("taliesin.references", {
      treeDataProvider: refs,
      showCollapseAll: true,
    }),
    vscode.window.createTreeView("taliesin.floats", { treeDataProvider: floats })
  );

  const refresh = async (): Promise<void> => {
    const doc = vscode.window.activeTextEditor?.document;
    // A webview holding focus leaves `activeTextEditor` undefined, which is the normal state
    // right after opening a preview; keep the last good tree rather than blanking the views.
    if (!doc || doc.uri.scheme !== "file" || !isSourceFile(doc.fileName)) return;
    const client = languageClient();
    if (!client) return; // the server may not have started yet, or failed to
    const uri = doc.uri.toString();
    try {
      const [outlineReply, refsReply] = await Promise.all([
        client.sendRequest<OutlineReply>("taliesin/projectOutline", { uri }),
        client.sendRequest<RefsReply>("taliesin/projectRefs", { uri }),
      ]);
      outline.replace(outlineTree(outlineReply, doc.fileName));
      floats.replace(floatsTree(outlineReply));
      refs.replace(refsTree(refsReply));
    } catch {
      // A request that fails (server restarting, document closed mid-flight) empties the
      // views rather than leaving a tree that no longer matches the project.
      outline.clear();
      floats.clear();
      refs.clear();
    }
  };

  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor(() => void refresh()),
    // On save rather than on change: the server reads pages from DISK, so refreshing on every
    // keystroke would redraw the tree from a file the buffer is already ahead of.
    vscode.workspace.onDidSaveTextDocument((d) => {
      if (isSourceFile(d.fileName)) void refresh();
    })
  );
  void refresh();
}
