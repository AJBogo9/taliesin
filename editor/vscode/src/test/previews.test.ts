import * as assert from "node:assert";
import { test } from "node:test";
import { PreviewRegistry, LivePreview } from "../previews";

/** A LivePreview with only the fields the registry actually reads. */
function fake(docPath: string): LivePreview {
  return { docPath, panel: {} as never, server: {} as never };
}

test("a second start for the same document reuses the first", () => {
  const r = new PreviewRegistry();
  const p = fake("/w/a.tmd");
  r.set(p);
  assert.strictEqual(r.get("/w/a.tmd"), p);
  assert.strictEqual(r.size, 1);
});

test("different documents get their own previews", () => {
  const r = new PreviewRegistry();
  r.set(fake("/w/a.tmd"));
  r.set(fake("/w/b.tmd"));
  assert.strictEqual(r.size, 2);
  assert.strictEqual(r.get("/w/a.tmd")?.docPath, "/w/a.tmd");
});

test("beginStart is a one-shot latch, so a double keypress spawns one server", () => {
  const r = new PreviewRegistry();
  assert.strictEqual(r.beginStart("/w/a.tmd"), true, "first caller proceeds");
  assert.strictEqual(r.beginStart("/w/a.tmd"), false, "second caller must bail");
  r.endStart("/w/a.tmd");
  assert.strictEqual(r.beginStart("/w/a.tmd"), true, "released after the start settles");
});

test("delete is idempotent, because both disposal paths may fire", () => {
  const r = new PreviewRegistry();
  r.set(fake("/w/a.tmd"));
  r.delete("/w/a.tmd");
  r.delete("/w/a.tmd");
  assert.strictEqual(r.size, 0);
});

test("previewFor prefers the buffer's own preview", () => {
  const r = new PreviewRegistry();
  r.set(fake("/w/a.tmd"));
  r.set(fake("/w/b.tmd"));
  assert.strictEqual(r.previewFor("/w/b.tmd")?.docPath, "/w/b.tmd");
});

test("previewFor falls back to the only preview, so an included file resolves", () => {
  const r = new PreviewRegistry();
  r.set(fake("/w/book.tmd"));
  assert.strictEqual(r.previewFor("/w/chapter.tmd")?.docPath, "/w/book.tmd");
});

test("previewFor refuses to guess when several previews are open", () => {
  const r = new PreviewRegistry();
  r.set(fake("/w/a.tmd"));
  r.set(fake("/w/b.tmd"));
  assert.strictEqual(r.previewFor("/w/c.tmd"), undefined);
});
