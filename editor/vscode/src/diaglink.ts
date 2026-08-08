// The terminal-link pattern and its path resolution, kept free of `vscode` so the unit suite can
// cover them: `npm test` runs in plain node, where importing `vscode` throws.
//
// This is the one piece of Taliesin knowledge that lives in TypeScript rather than being asked of
// the server, because a terminal line is not a document. `src/test/termlinks.test.ts` is therefore
// a DRIFT GATE, not just a unit test: it pins the Rust format strings this pattern was written
// against.
import * as path from "node:path";
import * as fs from "node:fs";

/**
 * A diagnostic location at the start of a terminal line.
 *
 * Matches the three shapes the tools actually print, and **no column group**, because none of
 * them emits one:
 *
 * ```text
 * posts/intro.tmd:12: warning: unresolved @fig-a   lint.rs, located
 * posts/intro.tmd: error: bad front matter        lint.rs, unlocated
 * chapters/two.tmd:7: include not resolved        build.rs
 * ```
 *
 * Anchored at the start of the line, which makes it correct whether or not
 * `TerminalLinkContext.line` arrives with the ANSI severity colour stripped: the colour sits
 * after the path, never before it. (Measured: it arrives stripped, because the terminal buffer
 * holds rendered cells rather than escape sequences.)
 *
 * The `\S+?` is lazy and the `\.tmd` is required, so a line merely mentioning a file in prose
 * ("rendered posts/intro.tmd in 12ms") does not match: the `:` immediately after the extension is
 * what makes it a location rather than a mention.
 */
export const DIAGNOSTIC_LINE = /^(\S+?\.tmd)(?::(\d+))?:\s/;

/**
 * The one existing file `rel` names, or `null`.
 *
 * Returns `null` when SEVERAL roots hold a matching path: a link that opens the wrong file is
 * worse than plain text, because the author edits the wrong chapter and the diagnostic stays.
 */
export function resolveUnique(rel: string, roots: readonly string[]): string | null {
  const hits = new Set<string>();
  for (const root of roots) {
    const candidate = path.resolve(root, rel);
    try {
      if (fs.statSync(candidate).isFile()) hits.add(candidate);
    } catch {
      // Not there. Try the next root.
    }
  }
  return hits.size === 1 ? [...hits][0] : null;
}
