import * as vscode from "vscode";
import * as fs from "node:fs";
import * as path from "node:path";
import { classifyHover, bibEntryFor } from "./hover";
import { frontmatterBibPaths, harvestAnchorIds, mergeXrefTargets, XrefSymbol } from "./complete";
import { fetchVocab, fetchSymbols, Vocab } from "./backend";
import { isSourceFile } from "./paths";

// Hover intelligence for `.tmd`: resolve a cross-reference to its rendered label
// (`@fig-2` -> "Figure 2"), a front-matter key to its documentation, and a `[@key]`
// citation to its raw BibTeX entry. Read-only, reusing the same Rust-authoritative data
// the completion path uses (`taliesin vocab`/`symbols`, the front-matter `.bib`).
export function registerHover(context: vscode.ExtensionContext): void {
  const binaryPath = () =>
    vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");

  let cachedVocab: Promise<Vocab> | undefined;
  const vocab = () => (cachedVocab ??= fetchVocab(binaryPath()));

  // `symbols` reads the file on disk, so its answer only changes on save; cache per document.
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
        cachedVocab = undefined;
        symbolCache.clear();
      }
    }),
    vscode.workspace.onDidSaveTextDocument((doc) => symbolCache.delete(doc.uri.fsPath))
  );

  const provider: vscode.HoverProvider = {
    async provideHover(document, position) {
      if (!isSourceFile(document.fileName)) return undefined;
      const target = classifyHover(document.getText(), position.line, position.character);
      if (target.kind === "none") return undefined;
      const range = new vscode.Range(position.line, target.start, position.line, target.end);

      switch (target.kind) {
        case "xref": {
          let v: Vocab;
          try {
            v = await vocab();
          } catch {
            return undefined;
          }
          const labels = Object.fromEntries(v.xrefPrefixes.map((p) => [p.prefix, p.label]));
          const merged = mergeXrefTargets(
            harvestAnchorIds(document.getText()),
            await symbols(document),
            labels
          );
          const hit = merged.find((t) => t.id === target.id);
          if (!hit) return undefined; // unknown target: say nothing rather than guess
          return new vscode.Hover(
            new vscode.MarkdownString(`**${hit.detail}** — \`@${target.id}\``),
            range
          );
        }
        case "frontmatter-key": {
          let v: Vocab;
          try {
            v = await vocab();
          } catch {
            return undefined;
          }
          const list = target.parent
            ? v.frontmatter.nested[target.parent] ?? []
            : v.frontmatter.keys;
          const found = list.find((n) => n.name === target.key);
          if (!found) return undefined;
          const scope = target.parent ? ` (under \`${target.parent}:\`)` : "";
          return new vscode.Hover(
            new vscode.MarkdownString(`\`${target.key}:\`${scope}\n\n${found.description}`),
            range
          );
        }
        case "cite": {
          const dir = path.dirname(document.fileName);
          for (const rel of frontmatterBibPaths(document.getText())) {
            try {
              const text = fs.readFileSync(path.resolve(dir, rel), "utf8");
              const entry = bibEntryFor(text, target.key);
              if (entry) {
                const md = new vscode.MarkdownString();
                md.appendCodeblock(entry, "bibtex");
                return new vscode.Hover(md, range);
              }
            } catch {
              /* missing/unreadable .bib -> try the next one */
            }
          }
          return undefined;
        }
      }
      return undefined;
    },
  };

  context.subscriptions.push(
    vscode.languages.registerHoverProvider({ language: "taliesin" }, provider)
  );
}
