import * as path from "node:path";
import * as vscode from "vscode";
import { PreviewServer } from "./server";
import { SitePage } from "./paths";

export interface LivePreview {
  panel: vscode.WebviewPanel;
  server: PreviewServer;
  /** The document this preview was opened for. */
  docPath: string;
  /**
   * The project this preview serves, or `null` when it serves `docPath` alone. A project
   * preview serves every page of the book, so `docPath` is only where it *started*.
   */
  root: string | null;
  /** The project's publishable pages, for looking a document's URL up. `null` off-project. */
  pages: SitePage[] | null;
  /**
   * The page the webview is currently showing, as that page reported itself. Mutable, because
   * following a cross-page link inside the preview changes it without the host being asked.
   */
  currentPage?: { docPath: string; baseDir: string };
  /**
   * A cursor waiting for a page to load. Selecting a page is asynchronous — the host asks,
   * the iframe navigates, the new page reports itself — so the block to mark has to survive
   * the round trip.
   */
  pendingCursor?: { editorPath: string; line: number; reveal: boolean } | null;
}

/**
 * What a preview is filed under: its project root when it has one, else the document itself.
 *
 * Keying by document was right while one preview meant one file. A site preview serves the
 * WHOLE book from one server, so opening a second chapter would miss the entry, spawn a
 * second server on a second port, and leave two previews of one book open (item 150 §3).
 */
export function previewKey(docPath: string, root: string | null): string {
  return root ?? docPath;
}

/** True when `file` is inside `dir` — by directory boundary, never by string prefix. */
function contains(dir: string, file: string): boolean {
  const rel = path.relative(path.resolve(dir), path.resolve(file));
  return rel !== "" && !rel.startsWith("..") && !path.isAbsolute(rel);
}

/**
 * The previews currently alive, keyed by {@link previewKey}.
 *
 * Without this, `openPreview` allocated a port, spawned `taliesin preview` and created a
 * webview on every invocation, so pressing the shortcut twice on one file left two servers
 * and two file watchers running against it.
 */
export class PreviewRegistry {
  private readonly live = new Map<string, LivePreview>();
  /**
   * Keys whose server is mid-spawn. A start is `await`ed, so two keypresses inside the
   * startup window would both miss `live` and both spawn — the exact leak the registry exists
   * to prevent, reintroduced through the back door.
   */
  private readonly starting = new Set<string>();

  get size(): number {
    return this.live.size;
  }

  get(key: string): LivePreview | undefined {
    return this.live.get(key);
  }

  set(preview: LivePreview): void {
    this.live.set(previewKey(preview.docPath, preview.root), preview);
  }

  /**
   * Idempotent: closing a panel and disposing the extension may both reach this.
   *
   * It takes the **preview**, not a key, so the key removed cannot disagree with the one
   * `set` filed it under. It could: `openPreview` latches on the project root before it knows
   * whether the project's map is usable, and a document whose `map` fails is then filed under
   * its own path instead. Deleting by the latch key would strand an entry pointing at a
   * disposed panel, and the next Open Preview on that document would reveal it and throw.
   */
  delete(preview: LivePreview): void {
    this.live.delete(previewKey(preview.docPath, preview.root));
  }

  /** True if the caller owns the start; false if one is already in flight. */
  beginStart(key: string): boolean {
    if (this.starting.has(key)) return false;
    this.starting.add(key);
    return true;
  }

  endStart(key: string): void {
    this.starting.delete(key);
  }

  /**
   * The preview a given buffer belongs to: its own if one is open, else the project preview
   * whose root contains it, else the single open preview if there is exactly one.
   *
   * The last fallback is what makes forward search work from an INCLUDED file, whose blocks
   * appear in the parent document's preview but which was never itself previewed. It
   * deliberately declines to guess when several previews are open, because picking the wrong
   * one would scroll a document the author is not looking at. Containment is checked first
   * and is not a guess, so a book preview keeps working with several previews open.
   */
  previewFor(file: string): LivePreview | undefined {
    const own = this.live.get(file);
    if (own) return own;
    for (const p of this.live.values()) {
      if (p.root && contains(p.root, file)) return p;
    }
    if (this.live.size !== 1) return undefined;
    return [...this.live.values()][0];
  }
}
