import * as path from "node:path";

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
