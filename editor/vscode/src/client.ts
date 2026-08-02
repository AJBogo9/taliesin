import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
  State,
} from "vscode-languageclient/node";
import { disposeShadowsFor, embeddedCompletions } from "./embedded";

// The language-intelligence half of the companion: a thin client over `taliesin lsp`.
//
// Everything here used to be re-implemented in TypeScript (completion, hover,
// go-to-definition, document symbols, diagnostics, quick fixes) against the same binary,
// via `taliesin vocab|symbols|check` subprocesses. The engine already owns all of it in
// `crates/server/src/lsp*.rs`, so the second copy could only drift — and had:
// `lsp_complete.rs` describes itself as "a Rust port of the companion's complete.ts", the
// server had a `:` completion trigger and rename that the companion never gained, and no
// gate compared them. One implementation, in the language that owns the vocabulary, is the
// only version of this that stays true.
//
// The preview webview (server.ts / webview.ts) is deliberately NOT here: bidirectional
// source sync is not an LSP concept, and the preview stays a read-only view either way.

let client: LanguageClient | undefined;

/**
 * The running language server, or `undefined` before it starts (and after a failed start).
 *
 * Exported for the editor commands that are not language *intelligence* but still need the
 * server's answer: the structural transforms ask `taliesin/sectionEdit` for the edits rather
 * than deriving them from a heading scan in TypeScript.
 */
export function languageClient(): LanguageClient | undefined {
  return client;
}

function binaryPath(): string {
  return vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");
}

/** Start the language server, replacing any running one. Resolves once it is ready. */
async function start(output: vscode.LogOutputChannel): Promise<void> {
  await stop();
  const command = binaryPath();
  // One definition for both profiles: there is no separate debug build of the server, and
  // duplicating the command is how a debug profile silently keeps launching a stale path.
  const run = { command, args: ["lsp"], transport: TransportKind.stdio };
  const serverOptions: ServerOptions = { run, debug: run };

  const clientOptions: LanguageClientOptions = {
    // `untitled` as well as `file`. A scratch buffer has no directory, so the server
    // answers nothing for the features that resolve a path against one (citations, include
    // paths, document links) — but front matter, cell options, div classes, math commands,
    // the outline, rename and every diagnostic need no path at all, and a `file`-only
    // selector silently withheld all of them from a buffer you had not saved yet.
    documentSelector: [
      { scheme: "file", language: "taliesin" },
      { scheme: "untitled", language: "taliesin" },
    ],
    outputChannel: output,
    // A `.tmd` edit is the only thing that changes an answer, but `_site.yml` and the
    // bibliography feed diagnostics too, so the server hears about those as well.
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/{*.tmd,_site.yml,*.bib}"),
    },
    // Completion inside a `{python}` / `{r}` / `{js}` cell is forwarded to whoever owns that
    // language and merged with ours. Ours still answers in a cell (that is where `#|` cell
    // options live), so this adds rather than replaces. See embedded.ts for why this one
    // feature cannot live in the server.
    middleware: {
      provideCompletionItem: async (document, position, context, token, next) => {
        const [ours, theirs] = await Promise.all([
          next(document, position, context, token),
          embeddedCompletions(client, document, position, context),
        ]);
        if (!theirs || theirs.length === 0) return ours;
        const mine = Array.isArray(ours) ? ours : (ours?.items ?? []);
        const incomplete = !Array.isArray(ours) && (ours?.isIncomplete ?? false);
        return new vscode.CompletionList([...mine, ...theirs], incomplete);
      },
    },
    // Surface a start failure where the author is, rather than only in a channel nobody
    // opened. `taliesin.path` is the fix for essentially all of them.
    initializationFailedHandler: (error: Error) => {
      vscode.window.showErrorMessage(
        `Taliesin: language server failed to start (${error}). ` +
          `Set "taliesin.path" to the taliesin binary.`
      );
      return false; // do not retry: a wrong path will not become right by trying again
    },
  };

  client = new LanguageClient(
    "taliesin",
    "Taliesin Language Server",
    serverOptions,
    clientOptions
  );

  try {
    await client.start();
  } catch (e) {
    // A spawn failure (ENOENT for a missing binary) never reaches
    // `initializationFailedHandler` — it throws out of `start()` instead — so the one
    // error an author actually hits needs its own message.
    client = undefined;
    vscode.window.showErrorMessage(
      `Taliesin: could not run \`${command} lsp\` (${(e as Error).message}). ` +
        `Set "taliesin.path" to the taliesin binary.`
    );
  }
}

/** Stop the running server, if any. Safe to call when nothing is running. */
async function stop(): Promise<void> {
  const running = client;
  client = undefined;
  if (!running) return;
  try {
    await running.stop();
  } catch {
    /* already dead: nothing to stop */
  }
}

export function registerLanguageClient(context: vscode.ExtensionContext): void {
  // A LOG channel, not a plain one: the client writes levelled records, and this is what
  // gives the author a "Taliesin Language Server" entry in the Output pane's log-level
  // picker rather than an undifferentiated wall of text.
  const output = vscode.window.createOutputChannel("Taliesin Language Server", { log: true });
  context.subscriptions.push(output);

  context.subscriptions.push(
    vscode.commands.registerCommand("taliesin.restartServer", async () => {
      await start(output);
      if (client?.state === State.Running) {
        vscode.window.setStatusBarMessage("Taliesin: language server restarted", 3000);
      }
    }),
    vscode.commands.registerCommand("taliesin.showServerLog", () => output.show()),
    // A closed document's shadow can never be asked for again; keeping it would pin the
    // projection of a buffer that no longer exists.
    vscode.workspace.onDidCloseTextDocument((doc) => disposeShadowsFor(doc.uri)),
    // Pointing at a different binary means a different server: restart rather than keep
    // answering from the old one, which would silently serve a stale vocabulary.
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("taliesin.path")) void start(output);
    }),
    { dispose: () => void stop() }
  );

  void start(output);
}
