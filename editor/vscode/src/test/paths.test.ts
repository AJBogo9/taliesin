import { test } from "node:test";
import assert from "node:assert";
import { parseSourcepos, resolveSourceFile, relativeKey, isSourceFile } from "../paths";

test("isSourceFile: accepts .tmd (native) and .qmd (deprecated), rejects others", () => {
  assert.equal(isSourceFile("/p/index.tmd"), true);
  assert.equal(isSourceFile("/p/post/old.qmd"), true);
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
  assert.equal(resolveSourceFile("/p/post/index.qmd", null), "/p/post/index.qmd");
});

test("resolveSourceFile: relative is joined to the doc's dir", () => {
  assert.equal(
    resolveSourceFile("/p/post/index.qmd", "../_includes/x.qmd"),
    "/p/_includes/x.qmd"
  );
});

test("relativeKey: the main doc maps to null", () => {
  assert.equal(relativeKey("/p/post/index.qmd", "/p/post/index.qmd"), null);
});

test("relativeKey: an included file maps to its relative path", () => {
  assert.equal(relativeKey("/p/post/index.qmd", "/p/_includes/x.qmd"), "../_includes/x.qmd");
});
