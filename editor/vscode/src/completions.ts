import * as vscode from "vscode";
import { spawn } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import {
  detectContext,
  harvestAnchorIds,
  harvestBibKeys,
  frontmatterBibPaths,
  parseSymbolsJson,
  mergeXrefTargets,
  XrefSymbol,
} from "./complete";

interface Named {
  name: string;
  description: string;
}
interface Vocab {
  frontmatter: { keys: Named[]; nested: Record<string, Named[]> };
  cellOptions: Named[];
  calloutKinds: Named[];
  theoremKinds: Named[];
  divClasses: Named[];
  inputTypes: string[];
  xrefPrefixes: { prefix: string; label: string }[];
}

// Spawn `taliesin vocab` and parse its JSON. Rejects on spawn failure or bad JSON.
function fetchVocab(binary: string): Promise<Vocab> {
  return new Promise((resolve, reject) => {
    let stdout = "";
    const child = spawn(binary, ["vocab"]);
    child.on("error", (e) => reject(e));
    child.stdout?.on("data", (b) => (stdout += b.toString()));
    child.on("close", () => {
      try {
        resolve(JSON.parse(stdout) as Vocab);
      } catch (e) {
        reject(e);
      }
    });
  });
}

// Spawn `taliesin symbols <file> --format json` for the document's cross-reference
// targets. Resolves to `[]` on any failure (no binary, an older binary without the
// command, a render panic), so @-completion degrades to the buffer scan instead of
// vanishing. `symbols` is parse-only and never starts a kernel, which is what makes it
// safe to run from a completion request.
function fetchSymbols(binary: string, file: string): Promise<XrefSymbol[]> {
  return new Promise((resolve) => {
    let stdout = "";
    const child = spawn(binary, ["symbols", file, "--format", "json"]);
    child.on("error", () => resolve([]));
    child.stdout?.on("data", (b) => (stdout += b.toString()));
    child.on("close", (code) => resolve(code === 0 ? parseSymbolsJson(stdout) : []));
  });
}

function item(label: string, detail: string, kind: vscode.CompletionItemKind): vscode.CompletionItem {
  const ci = new vscode.CompletionItem(label, kind);
  if (detail) ci.detail = detail;
  return ci;
}

export function registerCompletions(context: vscode.ExtensionContext): void {
  let cached: Promise<Vocab> | undefined;
  const binaryPath = () =>
    vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");
  const vocab = () => (cached ??= fetchVocab(binaryPath()));

  // `symbols` reads the file on disk, so its answer can only change when the file is
  // written. Cache per document and drop the entry on save (and on a binary change).
  const symbolCache = new Map<string, Promise<XrefSymbol[]>>();
  const symbols = (doc: vscode.TextDocument): Promise<XrefSymbol[]> => {
    if (doc.isUntitled) return Promise.resolve([]); // never saved: nothing on disk to read
    const key = doc.uri.fsPath;
    let hit = symbolCache.get(key);
    if (!hit) {
      hit = fetchSymbols(binaryPath(), key);
      symbolCache.set(key, hit);
    }
    return hit;
  };

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("taliesin.path")) {
        cached = undefined; // re-fetch next request
        symbolCache.clear();
      }
    }),
    vscode.workspace.onDidSaveTextDocument((doc) => symbolCache.delete(doc.uri.fsPath))
  );

  const provider: vscode.CompletionItemProvider = {
    async provideCompletionItems(document, position) {
      const linePrefix = document.getText(
        new vscode.Range(position.line, 0, position.line, position.character)
      );
      const docPrefix = document.getText(new vscode.Range(0, 0, position.line, position.character));
      const ctx = detectContext(linePrefix, docPrefix);
      if (ctx.kind === "none") return undefined;

      let v: Vocab;
      try {
        v = await vocab();
      } catch {
        return undefined; // no binary / bad vocab -> stay quiet, no completions
      }
      const K = vscode.CompletionItemKind;

      switch (ctx.kind) {
        case "frontmatter-key": {
          const list = ctx.parent ? v.frontmatter.nested[ctx.parent] ?? [] : v.frontmatter.keys;
          return list.map((n) => item(n.name, n.description, K.Property));
        }
        case "cell-option":
          return v.cellOptions.map((n) => item(n.name, n.description, K.Property));
        case "div-class": {
          const callouts = v.calloutKinds.map((n) =>
            item(`callout-${n.name}`, n.description, K.Class)
          );
          const theorems = v.theoremKinds.map((n) => item(n.name, n.description, K.Class));
          const divs = v.divClasses.map((n) => item(n.name, n.description, K.Class));
          return [...callouts, ...theorems, ...divs];
        }
        case "xref": {
          const prefixes = v.xrefPrefixes.map((p) =>
            item(`${p.prefix}-`, p.label, K.Reference)
          );
          // The buffer scan sees `{#id}` anchors, including ones typed but not yet saved.
          // `symbols` sees the resolved registry, including the `#| label:` cell figures
          // and tables a regex cannot find. An author needs both.
          const labels = Object.fromEntries(v.xrefPrefixes.map((p) => [p.prefix, p.label]));
          const ids = mergeXrefTargets(harvestAnchorIds(document.getText()), await symbols(document), labels)
            .filter((t) => ctx.typed === "" || t.id.startsWith(ctx.typed))
            .map((t) => item(t.id, t.detail, K.Reference));
          return [...prefixes, ...ids];
        }
        case "cite": {
          const dir = path.dirname(document.fileName);
          const keys = new Set<string>();
          for (const rel of frontmatterBibPaths(document.getText())) {
            try {
              const text = fs.readFileSync(path.resolve(dir, rel), "utf8");
              for (const k of harvestBibKeys(text)) keys.add(k);
            } catch {
              /* missing/unreadable .bib -> skip */
            }
          }
          return [...keys].map((k) => item(k, "citation key", K.Reference));
        }
      }
      return undefined;
    },
  };

  context.subscriptions.push(
    vscode.languages.registerCompletionItemProvider(
      { language: "taliesin" },
      provider,
      "@",
      ".",
      "|",
      "-"
    )
  );
}
