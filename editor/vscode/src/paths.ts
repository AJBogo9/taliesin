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

/**
 * The absolute source file a `tali-goto` refers to.
 *
 * `sourceFile` is defined relative to the **currently-loaded page's** directory, which is
 * not necessarily the document the preview was opened for: in a site preview the webview
 * navigates between pages, so anchoring on the opened document silently resolves a click on
 * chapter B against chapter A's directory and opens the wrong `index.tmd` (item 150).
 *
 * So the page supplies its own anchor. `anchor` is `{ baseDir, docPath }` as reported by the
 * page that sent the message (`window.TALIESIN_DOC`), and it wins whenever present. It is
 * absent only when an older preview client is talking to this host, in which case the
 * opened document is the best guess available and the previous behaviour is kept exactly.
 */
export function resolveSourceFile(
  docPath: string,
  sourceFile: string | null,
  anchor?: { baseDir?: string | null; docPath?: string | null } | null
): string {
  const baseDir = anchor?.baseDir || null;
  const pageDoc = anchor?.docPath || docPath;
  if (!sourceFile) return pageDoc;
  return path.resolve(baseDir ?? path.dirname(docPath), sourceFile);
}

/**
 * The project a document belongs to: the nearest ancestor directory holding a `_site.yml`,
 * or `null` for a document that is not in one.
 *
 * The rule is **`_site.yml` and nothing else** — deliberately NOT `.git`. A repository
 * boundary is not a document-project boundary: rooting at `.git` swallows every unrelated
 * directory in the repo into one "project", and a document outside any project has no
 * boundary to infer (backlog item 70). This mirrors the include-root rule
 * `render_single_doc` already applies in Rust, so the editor and the renderer agree on
 * where a project starts.
 *
 * `exists` is injected so the walk is unit-testable without a filesystem.
 */
export function projectRootFor(
  docPath: string,
  exists: (p: string) => boolean = (p) => {
    // Local require keeps `node:fs` out of the module's import graph for the pure callers.
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    return require("node:fs").existsSync(p);
  }
): string | null {
  let dir = path.dirname(path.resolve(docPath));
  // Bounded by reaching the filesystem root, where `path.dirname` becomes a fixed point.
  for (;;) {
    if (exists(path.join(dir, "_site.yml"))) return dir;
    const up = path.dirname(dir);
    if (up === dir) return null;
    dir = up;
  }
}

export function relativeKey(docPath: string, editorPath: string): string | null {
  if (path.resolve(editorPath) === path.resolve(docPath)) return null;
  const rel = path.relative(path.dirname(docPath), editorPath);
  return rel.split(path.sep).join("/"); // POSIX separators for the protocol
}
