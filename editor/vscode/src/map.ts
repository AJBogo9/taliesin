import { execFile } from "node:child_process";
import { SitePage, sitePages } from "./paths";

/**
 * The publishable pages of a project, from `taliesin map <root> --format json`.
 *
 * `null` for every failure, and deliberately so: the site-aware preview is an *upgrade* on
 * the single-file one (item 150 §2), so a missing or unreadable map costs the author nav and
 * cross-page links, never the preview itself. Failures seen in practice are a wrong
 * `taliesin.path`, a directory that is not a project, and a tool that answers with a
 * diagnostic rather than JSON.
 */
export function readSiteMap(binary: string, root: string): Promise<SitePage[] | null> {
  return new Promise((resolve) => {
    execFile(
      binary,
      ["map", root, "--format", "json"],
      // No `cwd`: `root` is absolute and `map` reads nothing relative to the process's
      // directory, so inheriting one is a failure mode (a cwd that has been deleted) bought
      // for nothing.
      { maxBuffer: 32 * 1024 * 1024, timeout: 15000 },
      (err, stdout) => resolve(err ? null : sitePages(stdout))
    );
  });
}
