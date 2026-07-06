import * as vscode from "vscode";
import { spawn } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import {
  detectContext,
  harvestAnchorIds,
  harvestBibKeys,
  frontmatterBibPaths,
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

function item(label: string, detail: string, kind: vscode.CompletionItemKind): vscode.CompletionItem {
  const ci = new vscode.CompletionItem(label, kind);
  if (detail) ci.detail = detail;
  return ci;
}

export function registerCompletions(context: vscode.ExtensionContext): void {
  let cached: Promise<Vocab> | undefined;
  const binaryPath = () =>
    vscode.workspace.getConfiguration("qmdFast").get<string>("path", "qmd-fast");
  const vocab = () => (cached ??= fetchVocab(binaryPath()));

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("qmdFast.path")) cached = undefined; // re-fetch next request
    })
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
          const ids = harvestAnchorIds(document.getText())
            .filter((id) => ctx.typed === "" || id.startsWith(ctx.typed))
            .map((id) => item(id, "cross-reference target", K.Reference));
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
