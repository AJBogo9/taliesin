import * as vscode from "vscode";
import { languageClient } from "./client";
import { SitePage, sitePages } from "./paths";

/** `taliesin/siteMap`: where each publishable page of a project is served. */
const SITE_MAP = "taliesin/siteMap";

/**
 * The publishable pages of a project, from the language server.
 *
 * `null` for every failure, and deliberately so: the site-aware preview is an *upgrade* on
 * the single-file one (item 150 §2), so a missing or unreadable map costs the author nav and
 * cross-page links, never the preview itself. Failures seen in practice are a directory that
 * is not a project and a server that is not running (a wrong `taliesin.path`).
 *
 * This spawned `taliesin map <root> --format json` until Wave 2 cut the verb. It asks the
 * running language client instead — one in-process request over a connection the companion
 * already holds, rather than a process launch and a JSON parse per preview.
 */
export async function readSiteMap(root: string): Promise<SitePage[] | null> {
  const client = languageClient();
  if (!client) return null;
  try {
    return sitePages(
      await client.sendRequest<unknown>(SITE_MAP, {
        uri: vscode.Uri.file(root).toString(),
      })
    );
  } catch {
    // A server that answers with an error must not be worse than one that is absent.
    return null;
  }
}
