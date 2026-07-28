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
