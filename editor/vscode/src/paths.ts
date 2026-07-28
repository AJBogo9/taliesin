import * as path from "node:path";

// The accepted source extensions, mirroring crates/core/src/ext.rs ACCEPTED_SOURCE_EXTS.
// NOT importable from Rust, so `src/test/paths.test.ts` reads ext.rs and asserts the two agree:
// this list once carried a `.qmd` the renderer had already stopped accepting.
export const ACCEPTED_SOURCE_EXTS = [".tmd"];

export function isSourceFile(fileName: string): boolean {
  return ACCEPTED_SOURCE_EXTS.some((ext) => fileName.endsWith(ext));
}

/**
 * Which document a preview request is for, given the resource the invocation named and the
 * path of the focused text editor. `null` when neither is previewable.
 *
 * The two callers do not agree on what "current" means, which is the whole reason this is a
 * function. VS Code invokes an `editor/title` command with the resource of the editor whose
 * button was clicked, and clicking that button need not make the editor active — so
 * `activeTextEditor` is stale, or `undefined` outright once a webview holds focus, which is
 * the state opening a preview leaves you in. The keybinding is gated on a focused `.tmd`
 * editor and therefore always agreed with `activeTextEditor`. Trusting only the latter is
 * what made the button fail intermittently while the shortcut never did.
 *
 * The named resource wins; the focused editor is the fallback for the keybinding and the
 * command palette, which pass no argument.
 */
export function previewTarget(
  resourcePath: string | null,
  activePath: string | null
): string | null {
  return [resourcePath, activePath].find((p) => p !== null && isSourceFile(p)) ?? null;
}

export function parseSourcepos(sp: string): { line: number; col: number } | null {
  const m = /^(\d+):(\d+)/.exec(sp || "");
  return m ? { line: +m[1], col: +m[2] } : null;
}

export function resolveSourceFile(docPath: string, sourceFile: string | null): string {
  if (!sourceFile) return docPath;
  return path.resolve(path.dirname(docPath), sourceFile);
}

export function relativeKey(docPath: string, editorPath: string): string | null {
  if (path.resolve(editorPath) === path.resolve(docPath)) return null;
  const rel = path.relative(path.dirname(docPath), editorPath);
  return rel.split(path.sep).join("/"); // POSIX separators for the protocol
}
