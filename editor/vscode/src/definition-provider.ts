import * as vscode from "vscode";
import * as fs from "node:fs";
import * as path from "node:path";
import { classifyHover, definitionSite, bibEntryOffset } from "./hover";
import { frontmatterBibPaths } from "./complete";
import { isSourceFile } from "./paths";

// Go-to-definition (F12 / Ctrl-click) for `.tmd`, reusing E4's `classifyHover` to identify the
// token under the cursor, then resolving it from the buffer + filesystem — no `taliesin`
// subprocess, so it is instant and offline. Read-only; navigates, never writes.
//   - `{{< include x.tmd >}}` / `{{< embed >}}` -> the file
//   - `@fig-x` / `@sec-x` / ...              -> its definition in THIS document (cross-file
//                                               refs degrade to "no definition", never a guess)
//   - `[@key]`                                -> the BibTeX entry in the front-matter `.bib`
export function registerDefinitions(context: vscode.ExtensionContext): void {
  const provider: vscode.DefinitionProvider = {
    provideDefinition(document, position) {
      if (!isSourceFile(document.fileName)) return undefined;
      const text = document.getText();
      const target = classifyHover(text, position.line, position.character);
      const dir = path.dirname(document.fileName);

      switch (target.kind) {
        case "include": {
          const abs = path.resolve(dir, target.path);
          if (!fs.existsSync(abs)) return undefined;
          return new vscode.Location(vscode.Uri.file(abs), new vscode.Position(0, 0));
        }
        case "xref": {
          const site = definitionSite(text, target.id);
          if (!site) return undefined;
          return new vscode.Location(document.uri, new vscode.Position(site.line, site.col));
        }
        case "cite": {
          for (const rel of frontmatterBibPaths(text)) {
            try {
              const abs = path.resolve(dir, rel);
              const bib = fs.readFileSync(abs, "utf8");
              const off = bibEntryOffset(bib, target.key);
              if (off !== null) {
                const before = bib.slice(0, off);
                const line = before.split("\n").length - 1;
                const col = off - (before.lastIndexOf("\n") + 1);
                return new vscode.Location(vscode.Uri.file(abs), new vscode.Position(line, col));
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
    vscode.languages.registerDefinitionProvider({ language: "taliesin" }, provider)
  );
}
