// Shared, Rust-authoritative data fetched from the `taliesin` binary: the static editor
// vocabulary (`taliesin vocab`) and a document's cross-reference registry (`taliesin
// symbols`). Both the completion and hover providers read from here, so there is one
// definition of how the editor shells out to the engine.
import { spawn } from "node:child_process";
import { parseSymbolsJson, XrefSymbol } from "./complete";

export interface Named {
  name: string;
  description: string;
}

export interface Vocab {
  frontmatter: { keys: Named[]; nested: Record<string, Named[]> };
  cellOptions: Named[];
  calloutKinds: Named[];
  theoremKinds: Named[];
  divClasses: Named[];
  inputTypes: string[];
  xrefPrefixes: { prefix: string; label: string }[];
  // Suggested values for the front-matter keys with a closed set (`format`, `theme`).
  frontmatterValues: Record<string, Named[]>;
}

// Spawn `taliesin vocab` and parse its JSON. Rejects on spawn failure or bad JSON.
export function fetchVocab(binary: string): Promise<Vocab> {
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
// command, a render panic), so callers degrade gracefully instead of vanishing. `symbols`
// is parse-only and never starts a kernel, which is what makes it safe to run live.
export function fetchSymbols(binary: string, file: string): Promise<XrefSymbol[]> {
  return new Promise((resolve) => {
    let stdout = "";
    const child = spawn(binary, ["symbols", file, "--format", "json"]);
    child.on("error", () => resolve([]));
    child.stdout?.on("data", (b) => (stdout += b.toString()));
    child.on("close", (code) => resolve(code === 0 ? parseSymbolsJson(stdout) : []));
  });
}
