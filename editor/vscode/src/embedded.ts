import { createHash } from "crypto";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/node";
import { remapDefinitions } from "./shadowlinks";

// Completion, hover, signature help and go-to-definition inside a code cell, answered by
// whoever owns that language.
//
// These are the language features that CANNOT live in `taliesin lsp`, and the reason is the
// protocol: LSP has no way for a server to say "this range is Python, go ask Pylance". The
// routing has to happen in the editor, against the editor's own provider registry.
//
// So the split is deliberate: the server still owns the KNOWLEDGE (where the cells are, and
// what language each one is — `taliesin/cellRegions`), and this file owns only the
// mechanical forwarding VS Code requires. No fence scanning happens here; that would be the
// TypeScript re-implementation this branch exists to have deleted.
//
// The mechanism is a "shadow" document: a real FILE, outside the workspace, that is only ever
// written on disk and never edited as a buffer. Measured in a real Extension Host on 1.126.0
// (2026-09-24) against the alternatives:
//
//   - a custom URI scheme + TextDocumentContentProvider: the built-in TS server does NOT
//     analyze it (word-based fallback only). Dead end. `context.globalStorageUri` is a trap of
//     the same kind: it is a `vscode-userdata:` URI, and a Python shadow opened through it got
//     no hover and no completion from Pylance.
//   - an UNTITLED document: real IntelliSense, but an untitled buffer with content is dirty,
//     and VS Code opens every dirty text model (untitled or file) as a background tab 800 ms
//     after it turns dirty. That put an `Untitled-N` tab in the strip, which hot exit then
//     restores after a restart with nothing left to own it.
//   - a file updated by WorkspaceEdit + save: no tab when the save beats the 800 ms, but save
//     participants (format on save, trim whitespace) rewrite it, which breaks the 1:1 map.
//
// A file written with `workspace.fs.writeFile` is never dirty, so it never becomes a tab, and
// nothing participates in the write. Both servers analyze it (hover, completion and a module
// at the workspace root all resolve as they did for untitled), and a hot exit has nothing to
// restore. The price is latency: a write reaches the open document through a file watcher,
// about 85 ms later, where an edit to an untitled buffer landed in a few.
//
// Go-to-definition is the forward that navigates without a click, so its targets are moved
// onto the `.tmd` (`shadowlinks.ts`) and F12 never lands in the shadow. A JSDoc `{@link}` in a
// forwarded TypeScript hover can still open the shadow if clicked. Auto-import edits are
// dropped (see `embeddedCompletions`).
//
// KNOWN LIMITATIONS: every `{js}` cell runs as its own AsyncFunction (tali-js.js), but the
// shadow joins them into one script. A name declared in ANOTHER `{js}` cell therefore
// resolves here although it is out of scope at run time, and a name declared in several
// resolves to all of them. The shadow also keeps plain display fences (```python, ```js),
// which never run, so a definition can land on a display sample.

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

/**
 * Shadow documents, keyed by `<parent uri>::<language id>`. A promise, and each refresh
 * chains onto the last: completion and signature help fire together on `(`, a hover or F12
 * can land during either, and two concurrent first requests would otherwise each open a
 * shadow of their own.
 */
const shadows = new Map<string, Promise<vscode.TextDocument>>();

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
 * runs `{python}`: one warm kernel, shared state. `{js}` does not (the KNOWN LIMITATION above).
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
  // Line 0 is never inside a cell (a cell's region starts after its opening fence), so it is
  // free for a comment that silences the shadow's own diagnostics without moving anything.
  if (lines[0] === "") lines[0] = QUIET_HEADER[languageId] ?? "";
  // Every JS shadow sits in one directory as a loose script, and TypeScript puts loose scripts
  // in one global scope, so a name declared in one `.tmd`'s `{js}` cell resolved in another's
  // (measured: hover in b.tmd typed a const declared only in a.tmd). A line appended AFTER the
  // last one moves nothing, and `export {}` makes the file a module, whose names stay its own.
  if (languageId === "javascript" || languageId === "typescript") lines.push("export {};");
  return lines.join("\n");
}

/**
 * A shadow's diagnostics are about text the author never wrote (the blanked lines around the
 * cells), so none may reach the Problems panel. By default neither server reports on a document
 * that is not in a tab, but Pylance's `diagnosticMode: "workspace"` and TypeScript's project
 * diagnostics with `checkJs` do (measured: 2 Warnings, and 2 `2451` Errors). The first line of
 * the file silences them. A syntax error in a `{js}` cell still reaches TypeScript's list.
 */
const QUIET_HEADER: Record<string, string> = {
  python: "# type: ignore",
  javascript: "// @ts-nocheck",
  typescript: "// @ts-nocheck",
};

/**
 * The shadow directory: a fresh private one per extension host, made by `mkdtemp` under the OS
 * temp dir (mode 0700, a name nobody can guess), outside every workspace, so a shadow is never
 * in the explorer, search or git, and no other local user can read a cell's code or put a file
 * in its place. `file:` on purpose: Pylance and the TS server serve `file:` and `untitled:`
 * only. `deactivate` removes it; one left behind by a crash goes with the temp dir.
 */
let shadowDir: vscode.Uri | undefined;

/** The extension a shadow needs for VS Code to give it `languageId`. */
const SHADOW_EXTENSIONS: Record<string, string> = {
  python: "py",
  javascript: "js",
  typescript: "ts",
  julia: "jl",
  sql: "sql",
  shellscript: "sh",
};

/**
 * The directory, made and watched on first use. The workbench watches a file outside the
 * workspace only while an editor SHOWS it, and a shadow is never shown: without this watcher a
 * write reaches the disk and never the open document (measured: 0 of 5 writes arrived; with it,
 * each one arrived in about 85 ms). The watcher lives as long as the extension host.
 */
function ensureShadowDir(): vscode.Uri {
  if (!shadowDir) {
    shadowDir = vscode.Uri.file(fs.mkdtempSync(path.join(os.tmpdir(), "taliesin-shadows-")));
    vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(shadowDir, "*"));
  }
  return shadowDir;
}

/** Delete the shadow directory and everything in it; the next forward makes a new one. */
export function removeShadowDir(): void {
  if (shadowDir) fs.rmSync(shadowDir.fsPath, { recursive: true, force: true });
  shadowDir = undefined;
}

/**
 * One file per `.tmd` and language. The `-<hash>` keeps it unimportable: Pylance puts a file's
 * own directory on the import path, and a hyphen is not legal in a module name.
 */
function shadowUri(dir: vscode.Uri, parent: vscode.Uri, languageId: string): vscode.Uri {
  const stem = path.basename(parent.path, path.extname(parent.path));
  const hash = createHash("sha1").update(parent.toString()).digest("hex").slice(0, 12);
  const extension = SHADOW_EXTENSIONS[languageId] ?? "txt";
  return vscode.Uri.joinPath(dir, `${stem}-${hash}.${extension}`);
}

/**
 * Resolves once `doc` holds `text`, or after `timeoutMs`: a disk write reaches the document
 * through the watcher, not synchronously. A reload that never lands costs one stale answer.
 */
function untilText(doc: vscode.TextDocument, text: string, timeoutMs = 1000): Promise<void> {
  if (doc.getText() === text) return Promise.resolve();
  return new Promise((resolve) => {
    const done = () => {
      sub.dispose();
      clearTimeout(timer);
      resolve();
    };
    const sub = vscode.workspace.onDidChangeTextDocument((e) => {
      if (e.document === doc && doc.getText() === text) done();
    });
    const timer = setTimeout(done, timeoutMs);
  });
}

function shadowFor(
  parent: vscode.TextDocument,
  languageId: string,
  content: string
): Promise<vscode.TextDocument> {
  const key = shadowKey(parent.uri, languageId);
  const previous = shadows.get(key);
  const next = (async () => {
    const existing = await previous?.catch(() => undefined);
    if (existing && !existing.isClosed && existing.getText() === content) return existing;
    const uri = shadowUri(ensureShadowDir(), parent.uri, languageId);
    await vscode.workspace.fs.writeFile(uri, Buffer.from(content, "utf8"));
    // A shadow VS Code has let go of (it releases an unshown document after a while) is read
    // fresh from disk, so only a live one has to wait for the reload.
    const doc =
      existing && !existing.isClosed ? existing : await vscode.workspace.openTextDocument(uri);
    await untilText(doc, content);
    return doc;
  })();
  shadows.set(key, next);
  return next;
}

/**
 * The shadow for the cell `position` sits in, refreshed to the parent's current text, or
 * `undefined` when it is not in a cell (or the cell's language has no provider we can name).
 */
async function inShadow(
  client: LanguageClient | undefined,
  document: vscode.TextDocument,
  position: vscode.Position,
  token: vscode.CancellationToken
): Promise<vscode.TextDocument | undefined> {
  if (!client) return undefined;

  let regions: CellRegion[];
  try {
    regions = await client.sendRequest<CellRegion[]>(CELL_REGIONS, {
      textDocument: { uri: document.uri.toString() },
    });
  } catch {
    return undefined; // an old server that does not know the method: no embedded support
  }
  if (token.isCancellationRequested) return undefined;

  const region = (regions ?? []).find(
    (r) => position.line >= r.startLine && position.line <= r.endLine
  );
  if (!region) return undefined;
  const languageId = LANGUAGE_IDS[region.language.toLowerCase()];
  if (!languageId) return undefined;
  return shadowFor(document, languageId, project(document, regions, languageId));
}

/**
 * Ask the shadow for `position` through one of VS Code's `vscode.execute*Provider` commands.
 * `undefined` outside a cell, once `token` is cancelled, and for any failure: the completion
 * middleware merges this with Taliesin's own answer, which a failed forward must never take
 * down with it.
 */
async function forward<T>(
  client: LanguageClient | undefined,
  document: vscode.TextDocument,
  position: vscode.Position,
  token: vscode.CancellationToken,
  command: string,
  ...rest: unknown[]
): Promise<{ shadow: vscode.TextDocument; answer: T | undefined } | undefined> {
  try {
    const shadow = await inShadow(client, document, position, token);
    if (!shadow || token.isCancellationRequested) return undefined;
    const answer = await vscode.commands.executeCommand<T>(command, shadow.uri, position, ...rest);
    if (token.isCancellationRequested) return undefined;
    return { shadow, answer };
  } catch {
    return undefined;
  }
}

/**
 * Completions for `position` from the language of the cell it sits in, or `undefined` when
 * it is not in a cell (or the cell's language has no provider we can name).
 */
export async function embeddedCompletions(
  client: LanguageClient | undefined,
  document: vscode.TextDocument,
  position: vscode.Position,
  context: vscode.CompletionContext,
  token: vscode.CancellationToken
): Promise<vscode.CompletionItem[] | undefined> {
  const got = await forward<vscode.CompletionList>(
    client,
    document,
    position,
    token,
    "vscode.executeCompletionItemProvider",
    context.triggerCharacter
  );
  if (!got) return undefined;
  return (got.answer?.items ?? []).map((item) => {
    // Auto-import edits are computed against the SHADOW, where every non-cell line is blank.
    // Applying them to the real document would write an import into the middle of prose.
    // The completion itself is still correct; only the extra edit is unsafe.
    const { additionalTextEdits: _dropped, ...rest } = item;
    return rest as vscode.CompletionItem;
  });
}

/**
 * The hover for `position` from the language of its cell, or `undefined`. That language may
 * have several hover providers, and a provider returns one hover, so their contents are joined.
 */
export async function embeddedHover(
  client: LanguageClient | undefined,
  document: vscode.TextDocument,
  position: vscode.Position,
  token: vscode.CancellationToken
): Promise<vscode.Hover | undefined> {
  const got = await forward<vscode.Hover[]>(
    client,
    document,
    position,
    token,
    "vscode.executeHoverProvider"
  );
  const hovers = got?.answer ?? [];
  if (hovers.length === 0) return undefined;
  return new vscode.Hover(
    hovers.flatMap((h) => h.contents),
    hovers.find((h) => h.range)?.range
  );
}

/**
 * Definitions for `position` from the language of its cell, moved onto the `.tmd` wherever
 * they landed in the shadow (see `remapDefinitions`); empty outside a cell.
 */
export async function embeddedDefinitions(
  client: LanguageClient | undefined,
  document: vscode.TextDocument,
  position: vscode.Position,
  token: vscode.CancellationToken
): Promise<vscode.LocationLink[]> {
  const got = await forward<(vscode.Location | vscode.LocationLink)[]>(
    client,
    document,
    position,
    token,
    "vscode.executeDefinitionProvider"
  );
  if (!got) return [];
  return remapDefinitions(got.answer, got.shadow.uri, document.uri, ensureShadowDir().toString());
}

/**
 * Signature help for `position` from the language of its cell, or `undefined`. The command
 * always asks as a fresh Invoke, so an overload picked with the arrow keys resets on the next
 * keystroke.
 */
export async function embeddedSignatureHelp(
  client: LanguageClient | undefined,
  document: vscode.TextDocument,
  position: vscode.Position,
  token: vscode.CancellationToken,
  context: vscode.SignatureHelpContext
): Promise<vscode.SignatureHelp | undefined> {
  const got = await forward<vscode.SignatureHelp>(
    client,
    document,
    position,
    token,
    "vscode.executeSignatureHelpProvider",
    context.triggerCharacter
  );
  return got?.answer;
}

/**
 * Forget a document's shadows and delete their files. Called for EVERY closed document, so it
 * only deletes files it created. A shadow still open in VS Code survives the delete as a
 * document (no tab, not dirty) until VS Code lets it go.
 */
export function disposeShadowsFor(uri: vscode.Uri): void {
  const prefix = `${uri.toString()}::`;
  for (const key of [...shadows.keys()]) {
    if (!key.startsWith(prefix)) continue;
    shadows.delete(key);
    if (!shadowDir) continue;
    const file = shadowUri(shadowDir, uri, key.slice(prefix.length));
    vscode.workspace.fs.delete(file).then(undefined, () => undefined);
  }
}
