// The text of a SHADOW document (embedded.ts): the `.tmd` as one language's server should see
// it. Kept free of `vscode` so the unit suite can cover it: `npm test` runs in plain node,
// where importing `vscode` throws.

/** One code cell's body, as `taliesin/cellRegions` reports it (`lsp_cells::CellRegion`). */
export interface CellRegion {
  language: string;
  startLine: number;
  endLine: number;
  /** The function the cell runs in, for a `{js}` cell; absent when its scope is the document's. */
  wrap?: { open: string; close: string };
}

// A cell language as the DOCUMENT spells it -> the language id VS Code registers providers
// under. The server deliberately does not know these: `javascript` is VS Code's name for
// what a `.tmd` calls `js`, and the same server also answers Neovim and Helix.
export const LANGUAGE_IDS: Record<string, string> = {
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
 * `lines`, the parent document's, reprojected so that only the lines belonging to `languageId`
 * cells survive; every other line becomes empty.
 *
 * Blanking rather than slicing is what makes positions map 1:1 (a completion at line 6 of
 * the `.tmd` is a completion at line 6 of the shadow, with no offset arithmetic to get wrong).
 * Keeping EVERY cell of that language (not just the one under the cursor) is what makes
 * `import os` in the first cell visible to `os.` in the third, which matches how Taliesin
 * runs `{python}`: one warm kernel, shared state. A `{js}` cell runs as its own function
 * instead, and the server says so (`wrap`): its open goes on the line above the body and its
 * close on the line below, both a fence or an option line and so blank here, which gives each
 * cell its own scope and parameters without moving a line.
 */
export function project(
  lines: readonly string[],
  regions: readonly CellRegion[],
  languageId: string
): string {
  const out = lines.map(() => "");
  for (const r of regions) {
    if (LANGUAGE_IDS[r.language.toLowerCase()] !== languageId) continue;
    for (let l = r.startLine; l <= r.endLine && l < lines.length; l++) out[l] = lines[l];
    // Line 0 is the quiet header's (below), so a cell whose open would land there (its fence
    // is the first line and no option line follows it) stays unwrapped: TypeScript reads
    // `@ts-nocheck` only from a line comment before the first token, so the two cannot share
    // the line.
    if (!r.wrap || r.startLine === 1) continue;
    out[r.startLine - 1] = r.wrap.open;
    // An unterminated fence runs to the end of the document, so its close is a line of its own.
    if (r.endLine + 1 < out.length) out[r.endLine + 1] = r.wrap.close;
    else out.push(r.wrap.close);
  }
  // Line 0 is never inside a cell (a cell's region starts after its opening fence), so it is
  // free for a comment that silences the shadow's own diagnostics without moving anything.
  if (out[0] === "") out[0] = QUIET_HEADER[languageId] ?? "";
  // Every JS shadow sits in one directory as a loose script, and TypeScript puts loose scripts
  // in one global scope, so a name declared in one `.tmd`'s `{js}` cell resolved in another's
  // (measured: hover in b.tmd typed a const declared only in a.tmd). A line appended AFTER the
  // last one moves nothing, and `export {}` makes the file a module, whose names stay its own.
  if (languageId === "javascript" || languageId === "typescript") out.push("export {};");
  return out.join("\n");
}
