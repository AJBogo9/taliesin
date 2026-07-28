import * as vscode from "vscode";
import { PreviewServer } from "./server";

export interface LivePreview {
  panel: vscode.WebviewPanel;
  server: PreviewServer;
  /** The document this preview was started for. Its directory anchors path resolution. */
  docPath: string;
}

/**
 * The previews currently alive, keyed by document path.
 *
 * Without this, `openPreview` allocated a port, spawned `taliesin preview` and created a
 * webview on every invocation, so pressing the shortcut twice on one file left two servers
 * and two file watchers running against it.
 */
export class PreviewRegistry {
  private readonly live = new Map<string, LivePreview>();
  /**
   * Documents whose server is mid-spawn. A start is `await`ed, so two keypresses inside the
   * startup window would both miss `live` and both spawn — the exact leak the registry exists
   * to prevent, reintroduced through the back door.
   */
  private readonly starting = new Set<string>();

  get size(): number {
    return this.live.size;
  }

  get(docPath: string): LivePreview | undefined {
    return this.live.get(docPath);
  }

  set(preview: LivePreview): void {
    this.live.set(preview.docPath, preview);
  }

  /** Idempotent: closing a panel and disposing the extension may both reach this. */
  delete(docPath: string): void {
    this.live.delete(docPath);
  }

  /** True if the caller owns the start; false if one is already in flight. */
  beginStart(docPath: string): boolean {
    if (this.starting.has(docPath)) return false;
    this.starting.add(docPath);
    return true;
  }

  endStart(docPath: string): void {
    this.starting.delete(docPath);
  }

  /**
   * The preview a given buffer belongs to: its own if one is open, else the single open
   * preview if there is exactly one.
   *
   * The fallback is what makes forward search work from an INCLUDED file, whose blocks appear
   * in the parent document's preview but which was never itself previewed. It deliberately
   * declines to guess when several previews are open, because picking the wrong one would
   * scroll a document the author is not looking at.
   */
  previewFor(file: string): LivePreview | undefined {
    const own = this.live.get(file);
    if (own) return own;
    if (this.live.size !== 1) return undefined;
    return [...this.live.values()][0];
  }
}
