// The paste gestures' pure decisions, unit-tested without a clipboard or an Extension Host.
//
// Everything the SERVER decides (the figure shape, the pipe table, the citation key) is tested in
// `lsp_insert.rs`. What is left here is the routing, which is the client's own judgement and the
// one place a bug is invisible from Rust.
import { test } from "node:test";
import assert from "node:assert";
import { classifyPaste, isUrl, isBibtex } from "../pastekind";

test("an image mime routes to the image gesture", () => {
  assert.strictEqual(classifyPaste(["image/png"]), "image");
  assert.strictEqual(classifyPaste(["image/svg+xml", "text/plain"]), "image");
});

test("HTML wins over the plain-text copy a spreadsheet puts alongside it", () => {
  assert.strictEqual(classifyPaste(["text/html", "text/plain"]), "htmlTable");
});

test("plain text routes to the text inspection whether or not there is a selection", () => {
  // The regression this pins: an earlier draft took `hasSelection` here and returned null
  // without one, which made pasting a BibTeX entry or a TSV grid with nothing selected, the
  // ordinary case, silently do nothing at all.
  assert.strictEqual(classifyPaste(["text/plain"]), "text");
});

test("an unknown mime routes nowhere, so the plain paste wins", () => {
  assert.strictEqual(classifyPaste(["application/octet-stream"]), null);
  assert.strictEqual(classifyPaste([]), null);
});

test("a URL is recognised only when it is a single absolute http(s) URL", () => {
  assert.ok(isUrl("https://taliesin.dev/guide"));
  assert.ok(isUrl("  http://example.org/x?a=1#b  "), "surrounding whitespace is trimmed");
  assert.ok(!isUrl("taliesin.dev/guide"), "no scheme");
  assert.ok(!isUrl("mailto:a@b.c"), "not a web URL");
  assert.ok(!isUrl("see https://x.dev for more"), "prose containing a URL is not a URL");
  assert.ok(!isUrl(""));
});

test("a BibTeX entry is recognised by its @type{ head", () => {
  assert.ok(isBibtex("@article{bishop2006, title = {PR}}"));
  assert.ok(isBibtex("\n  @book{k1984,\n  title = {TeX},\n}"), "leading whitespace is allowed");
  assert.ok(isBibtex("@ARTICLE{upper2020}"), "the type is case-insensitive");
  assert.ok(!isBibtex("an email like a@b.com{x}"), "the @ must start the text");
  assert.ok(!isBibtex("@ {nospace}"), "a bare @ is not a type");
});
