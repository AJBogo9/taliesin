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

/** One publishable page of a project, as `taliesin/siteMap` reports it. */
export interface SitePage {
  /** Source path relative to the project root, POSIX separators. */
  rel: string;
  /** Where the server serves it, relative to the project root. */
  url: string;
}

/**
 * The publishable pages in a `taliesin/siteMap` answer.
 *
 * `null` means "unusable, fall back to a single-file preview": the request failed, or it
 * answered about something that is not a project. Neither may lose the preview, so both are
 * a `null` rather than a throw.
 *
 * The parameter is the parsed answer, not a string: it used to be `taliesin map`'s stdout
 * and had to survive a tool that printed a diagnostic where JSON was expected. Over the wire
 * that failure mode does not exist, but the shape check does — the client must not assume a
 * newer or older server sent what it wants.
 */
export function sitePages(answer: unknown): SitePage[] | null {
  const pages = (answer as { pages?: unknown } | null)?.pages;
  if (!Array.isArray(pages)) return null;
  return pages
    .filter(
      (p): p is SitePage =>
        !!p && typeof p.rel === "string" && typeof p.url === "string"
    )
    .map((p) => ({ rel: p.rel, url: p.url }));
}

/**
 * Where a document is served inside its project, or `null` when the project does not publish
 * it — a draft, an `{{< embed >}}`-referenced deck (deliberately kept out of `site.pages`),
 * or a file outside the root entirely.
 *
 * The lookup is deliberately a *lookup*: `.tmd`→`.html`, book chapter numbering and `index`
 * handling all live in Rust, and deriving the URL here would be the second implementation the
 * LSP rewrite existed to delete.
 */
export function pageUrlFor(pages: SitePage[], root: string, docPath: string): string | null {
  const rel = path
    .relative(path.resolve(root), path.resolve(docPath))
    .split(path.sep)
    .join("/"); // the map speaks POSIX; `path.relative` does not on Windows
  // A file outside the root needs no guard of its own: its `rel` escapes with `../`, and no
  // project-relative `rel` ever does, so the lookup below already answers `null`.
  return pages.find((p) => p.rel === rel)?.url ?? null;
}

export function relativeKey(docPath: string, editorPath: string): string | null {
  if (path.resolve(editorPath) === path.resolve(docPath)) return null;
  const rel = path.relative(path.dirname(docPath), editorPath);
  return rel.split(path.sep).join("/"); // POSIX separators for the protocol
}

/**
 * What the preview should do about the editor cursor: select another page first, mark a block
 * on the page already showing, or both-in-order.
 *
 * `pageDoc` is the page the webview is **currently showing** (as that page reported itself),
 * not the document the preview was opened for. Keying against the opened document is the
 * mirror of the click-to-source staleness bug: once the preview has followed a cross-page
 * link, a key computed against the opened chapter matches nothing on screen and the mark
 * silently lands nowhere (item 150 §4).
 *
 * A cursor in a *different page* of the same project asks for that page — the reverse of the
 * link the reader just followed. This does **not** reintroduce the yank the reveal/mark split
 * exists to prevent: typing in the page already on screen is the first branch and never
 * navigates, so scrolling the preview and then typing one character still leaves it alone.
 */
export function cursorTarget(
  pageDoc: string,
  pages: SitePage[] | null,
  root: string | null,
  editorPath: string
): { navigateTo: string | null; file: string | null } {
  const file = relativeKey(pageDoc, editorPath);
  if (file === null) return { navigateTo: null, file: null };
  const url = root && pages ? pageUrlFor(pages, root, editorPath) : null;
  // No URL means the project does not publish it — a draft, an `{{< embed >}}`ed deck, a
  // file outside the root — so there is no page to select and it is keyed as an include.
  return url ? { navigateTo: url, file: null } : { navigateTo: null, file };
}
