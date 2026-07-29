import { test } from "node:test";
import assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import {
  parseSourcepos,
  resolveSourceFile,
  relativeKey,
  isSourceFile,
  previewTarget,
  ACCEPTED_SOURCE_EXTS,
  projectRootFor,
} from "../paths";

const REPO_ROOT = path.join(__dirname, "..", "..", "..", "..");

test("the accepted source extensions match crates/core/src/ext.rs", () => {
  // paths.ts cannot import from Rust, so this is the gate that keeps the copy honest.
  // It went stale once: the list still carried `.qmd` (with a comment claiming "the
  // renderer still accepts it") long after the legacy-format clean break made `.tmd`
  // the only input.
  const rs = fs.readFileSync(path.join(REPO_ROOT, "crates/core/src/ext.rs"), "utf8");
  const m = /ACCEPTED_SOURCE_EXTS:\s*&\[&str\]\s*=\s*&\[([^\]]*)\]/.exec(rs);
  assert.ok(m, "ext.rs declares ACCEPTED_SOURCE_EXTS");
  const rust = [...m![1].matchAll(/"([^"]+)"/g)].map((x) => `.${x[1]}`);
  assert.deepEqual(ACCEPTED_SOURCE_EXTS, rust);
});

test("isSourceFile: accepts .tmd, rejects everything else", () => {
  assert.equal(isSourceFile("/p/index.tmd"), true);
  assert.equal(isSourceFile("/p/post/old.qmd"), false);
  assert.equal(isSourceFile("/p/notes.md"), false);
  assert.equal(isSourceFile("/p/data.tmdx"), false);
  assert.equal(isSourceFile("README"), false);
});

test("parseSourcepos reads leading L:C", () => {
  assert.deepEqual(parseSourcepos("12:3-14:7"), { line: 12, col: 3 });
  assert.deepEqual(parseSourcepos("5:1"), { line: 5, col: 1 });
  assert.equal(parseSourcepos("garbage"), null);
});

test("resolveSourceFile: null = the doc itself", () => {
  assert.equal(resolveSourceFile("/p/post/index.tmd", null), "/p/post/index.tmd");
});

test("resolveSourceFile: relative is joined to the doc's dir", () => {
  assert.equal(
    resolveSourceFile("/p/post/index.tmd", "../_includes/x.tmd"),
    "/p/_includes/x.tmd"
  );
});

test("relativeKey: the main doc maps to null", () => {
  assert.equal(relativeKey("/p/post/index.tmd", "/p/post/index.tmd"), null);
});

test("relativeKey: an included file maps to its relative path", () => {
  assert.equal(relativeKey("/p/post/index.tmd", "/p/_includes/x.tmd"), "../_includes/x.tmd");
});

// The reported bug: the title-bar button "sometimes gives errors", the keybinding never
// does. They disagree about what is active — the button names its resource and does not
// have to focus it, so `activeTextEditor` may be a webview (a preview already open, hence
// "sometimes") or another file entirely. The keybinding is gated on a focused .tmd editor,
// which is exactly why it always worked.
test("previewTarget: the clicked resource wins over whatever is focused", () => {
  assert.equal(previewTarget("/p/post/index.tmd", "/p/other.tmd"), "/p/post/index.tmd");
});

test("previewTarget: no resource falls back to the active editor", () => {
  // The keybinding and the command palette pass no argument at all.
  assert.equal(previewTarget(null, "/p/post/index.tmd"), "/p/post/index.tmd");
});

test("previewTarget: a focused webview leaves only the clicked resource", () => {
  // `activeTextEditor` is undefined whenever a webview holds focus, which is the state the
  // button put the user in after the first preview was opened.
  assert.equal(previewTarget("/p/post/index.tmd", null), "/p/post/index.tmd");
});

test("previewTarget: nothing previewable is null, not a guess", () => {
  assert.equal(previewTarget(null, null), null);
  assert.equal(previewTarget("/p/notes.md", null), null);
  // A non-.tmd resource must not shadow a perfectly good active .tmd.
  assert.equal(previewTarget("/p/notes.md", "/p/post/index.tmd"), "/p/post/index.tmd");
});

test("resolveSourceFile prefers the anchor the page sent over the opened document", () => {
  // Item 150. `source_file` is relative to the CURRENTLY-LOADED page's directory. In a site
  // preview the webview navigates between pages, so anchoring on the document the preview
  // was opened for resolves a click on chapter B against chapter A's directory — and with
  // the book convention of one `index.tmd` per chapter directory, that silently opens a real
  // file that is the wrong one. The page now sends its own anchor and it wins.
  const opened = "/proj/chapters/intro/index.tmd";
  const onPageB = { baseDir: "/proj/chapters/methods", docPath: "/proj/chapters/methods/index.tmd" };
  assert.strictEqual(
    resolveSourceFile(opened, "shared.tmd", onPageB),
    "/proj/chapters/methods/shared.tmd"
  );
  // …and with no anchor (an older preview client), the previous behaviour is kept exactly.
  assert.strictEqual(
    resolveSourceFile(opened, "shared.tmd"),
    "/proj/chapters/intro/shared.tmd"
  );
  // A null `source_file` means "the previewed document itself" — which, on a navigated
  // preview, is the page now showing, not the one opened.
  assert.strictEqual(resolveSourceFile(opened, null, onPageB), onPageB.docPath);
  assert.strictEqual(resolveSourceFile(opened, null), opened);
});

test("projectRootFor walks up to the nearest _site.yml and never to .git", () => {
  // The rule is the include-root rule (backlog items 50/51/57 and 70): a project starts at
  // a `_site.yml`. A `.git` is a REPOSITORY boundary, and rooting there would swallow every
  // unrelated directory in the repo into one project.
  const present = new Set([
    "/repo/book/_site.yml",
    "/repo/.git", // deliberately NOT a project marker
  ]);
  const exists = (p: string) => present.has(p);

  assert.strictEqual(projectRootFor("/repo/book/chapters/one.tmd", exists), "/repo/book");
  assert.strictEqual(projectRootFor("/repo/book/index.tmd", exists), "/repo/book");
  // Outside any project: no boundary to infer, and `.git` must not become one.
  assert.strictEqual(projectRootFor("/repo/loose/post.tmd", exists), null);
  // The walk terminates at the filesystem root rather than looping.
  assert.strictEqual(projectRootFor("/post.tmd", exists), null);
});

test("projectRootFor picks the NEAREST _site.yml, not the outermost", () => {
  // The docs books are siblings under a container precisely because a nested project must
  // win: `docs/guide` is its own project even if an ancestor ever gained a `_site.yml`.
  const present = new Set(["/repo/_site.yml", "/repo/docs/guide/_site.yml"]);
  const exists = (p: string) => present.has(p);
  assert.strictEqual(projectRootFor("/repo/docs/guide/using/x.tmd", exists), "/repo/docs/guide");
  assert.strictEqual(projectRootFor("/repo/other/y.tmd", exists), "/repo");
});
