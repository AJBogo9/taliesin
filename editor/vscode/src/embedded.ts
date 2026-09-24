import { createHash } from "crypto";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/node";
import { CellRegion, LANGUAGE_IDS, project } from "./projection";
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
// Each `{js}` cell runs as its own AsyncFunction (tali-js.js), so the shadow wraps each one in
// that function (`projection.ts`, from the server's `wrap`): a name declared in another `{js}`
// cell is out of scope, as it is at run time, and F12 on a name declared in several cells
// lands on the declaration in the cell it was asked from.
//
// KNOWN LIMITATIONS: the shadow also keeps plain display fences (```python, ```js), which never
// run and are projected unwrapped, so a definition can land on a display sample. A `{js}` cell
// whose fence is the document's first line and that has no `//|` option line (its body starts
// on line 1) stays unwrapped too, because line 0 holds the comment that keeps the shadow out of
// the Problems panel, so its names resolve in the cells below it.

/** The custom request the server answers. Must match `lsp::CELL_REGIONS_METHOD`. */
const CELL_REGIONS = "taliesin/cellRegions";

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
  const lines = Array.from({ length: document.lineCount }, (_, l) => document.lineAt(l).text);
  return shadowFor(document, languageId, project(lines, regions, languageId));
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
