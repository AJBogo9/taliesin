import * as path from "node:path";

// The accepted source extensions, mirroring crates/core/src/ext.rs ACCEPTED_SOURCE_EXTS
// (native `.tmd` first, `.qmd` deprecated-but-accepted). NOT importable from Rust — keep in sync.
// Note: the editor LANGUAGE/grammar owns `.tmd` only (package.json contributes.languages); the
// PREVIEW accepts `.qmd` too because the renderer still does, so existing `.qmd` files aren't stranded.
export const ACCEPTED_SOURCE_EXTS = [".tmd", ".qmd"];

export function isSourceFile(fileName: string): boolean {
  return ACCEPTED_SOURCE_EXTS.some((ext) => fileName.endsWith(ext));
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
