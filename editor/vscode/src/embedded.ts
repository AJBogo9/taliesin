import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/node";

// Completion inside a code cell, answered by whoever owns that language.
//
// This is the one language feature that CANNOT live in `taliesin lsp`, and the reason is the
// protocol: LSP has no way for a server to say "this range is Python, go ask Pylance". The
// routing has to happen in the editor, against the editor's own provider registry.
//
// So the split is deliberate: the server still owns the KNOWLEDGE (where the cells are, and
// what language each one is — `taliesin/cellRegions`), and this file owns only the
// mechanical forwarding VS Code requires. No fence scanning happens here; that would be the
// TypeScript re-implementation this branch exists to have deleted.
//
// The mechanism is a "shadow" document. Two candidates were measured in a real Extension
// Host before this was written:
//
//   - a custom URI scheme + TextDocumentContentProvider: the built-in TS server does NOT
//     analyze it. `greeting.` offered `const, greeting, hi` — word-based fallback, not
//     IntelliSense. Dead end.
//   - an UNTITLED document of the right language: 52 items including `charAt`. Real.
//
// Hence untitled. The shadow is never shown in an editor, so it never becomes a tab the
// author has to close or a save prompt.

interface CellRegion {
  language: string;
  startLine: number;
  endLine: number;
}

/** The custom request the server answers. Must match `lsp::CELL_REGIONS_METHOD`. */
const CELL_REGIONS = "taliesin/cellRegions";

// A cell language as the DOCUMENT spells it -> the language id VS Code registers providers
// under. The server deliberately does not know these: `javascript` is VS Code's name for
// what a `.tmd` calls `js`, and the same server also answers Neovim and Helix.
const LANGUAGE_IDS: Record<string, string> = {
  python: "python",
  py: "python",
  js: "javascript",
  javascript: "javascript",
  ts: "typescript",
  typescript: "typescript",
  julia: "julia",
  sql: "sql",
  bash: "shellscript",
  sh: "shellscript",
  shell: "shellscript",
};

/** Shadow documents, keyed by `<parent uri>::<language id>`. */
const shadows = new Map<string, vscode.TextDocument>();

function shadowKey(parent: vscode.Uri, languageId: string): string {
  return `${parent.toString()}::${languageId}`;
}

/**
 * The parent document reprojected so that only the lines belonging to `languageId` cells
 * survive; every other line becomes empty.
 *
 * Blanking rather than slicing is what makes positions map 1:1 — a completion at line 6 of
 * the `.tmd` is a completion at line 6 of the shadow, with no offset arithmetic to get wrong.
 * Keeping EVERY cell of that language (not just the one under the cursor) is what makes
 * `import os` in the first cell visible to `os.` in the third, which matches how Taliesin
 * actually runs them: one warm kernel, shared state.
 */
function project(
  parent: vscode.TextDocument,
  regions: CellRegion[],
  languageId: string
): string {
  const keep = new Set<number>();
  for (const r of regions) {
    if (LANGUAGE_IDS[r.language.toLowerCase()] !== languageId) continue;
    for (let l = r.startLine; l <= r.endLine && l < parent.lineCount; l++) keep.add(l);
  }
  const lines: string[] = [];
  for (let l = 0; l < parent.lineCount; l++) {
    lines.push(keep.has(l) ? parent.lineAt(l).text : "");
  }
  return lines.join("\n");
}

async function shadowFor(
  parent: vscode.TextDocument,
  languageId: string,
  content: string
): Promise<vscode.TextDocument> {
  const key = shadowKey(parent.uri, languageId);
  const existing = shadows.get(key);
  if (existing && !existing.isClosed) {
    if (existing.getText() !== content) {
      const edit = new vscode.WorkspaceEdit();
      const end = existing.lineAt(existing.lineCount - 1).range.end;
      edit.replace(existing.uri, new vscode.Range(new vscode.Position(0, 0), end), content);
      await vscode.workspace.applyEdit(edit);
    }
    return existing;
  }
  const created = await vscode.workspace.openTextDocument({ language: languageId, content });
  shadows.set(key, created);
  return created;
}

/**
 * Completions for `position` from the language of the cell it sits in, or `undefined` when
 * it is not in a cell (or the cell's language has no provider we can name).
 */
export async function embeddedCompletions(
  client: LanguageClient | undefined,
  document: vscode.TextDocument,
  position: vscode.Position,
  context: vscode.CompletionContext
): Promise<vscode.CompletionItem[] | undefined> {
  if (!client) return undefined;

  let regions: CellRegion[];
  try {
    regions = await client.sendRequest<CellRegion[]>(CELL_REGIONS, {
      textDocument: { uri: document.uri.toString() },
    });
  } catch {
    return undefined; // an old server that does not know the method: no embedded support
  }

  const region = (regions ?? []).find(
    (r) => position.line >= r.startLine && position.line <= r.endLine
  );
  if (!region) return undefined;
  const languageId = LANGUAGE_IDS[region.language.toLowerCase()];
  if (!languageId) return undefined;

  const shadow = await shadowFor(document, languageId, project(document, regions, languageId));

  const list = (await vscode.commands.executeCommand(
    "vscode.executeCompletionItemProvider",
    shadow.uri,
    position,
    context.triggerCharacter
  )) as vscode.CompletionList | undefined;

  return (list?.items ?? []).map((item) => {
    // Auto-import edits are computed against the SHADOW, where every non-cell line is blank.
    // Applying them to the real document would write an import into the middle of prose.
    // The completion itself is still correct; only the extra edit is unsafe.
    const { additionalTextEdits: _dropped, ...rest } = item;
    return rest as vscode.CompletionItem;
  });
}

/** Forget a document's shadows. The untitled buffer itself is left to VS Code to reclaim. */
export function disposeShadowsFor(uri: vscode.Uri): void {
  for (const key of [...shadows.keys()]) {
    if (key.startsWith(`${uri.toString()}::`)) shadows.delete(key);
  }
}
