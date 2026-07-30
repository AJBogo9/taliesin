import * as assert from "node:assert";
import { test } from "node:test";
import { PreviewRegistry, LivePreview, previewKey } from "../previews";

/** A LivePreview with only the fields the registry actually reads. */
function fake(docPath: string, root: string | null = null): LivePreview {
  return { docPath, root, panel: {} as never, server: {} as never, pages: null };
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
  const p = fake("/w/a.tmd");
  r.set(p);
  r.delete(p);
  r.delete(p);
  assert.strictEqual(r.size, 0);
});

// `delete` takes the PREVIEW, not a key, so the key it removes cannot disagree with the one
// `set` filed it under. It could: `openPreview` latches on the project root before it knows
// whether the project map is usable, and a document whose `map` fails is then filed under its
// own path instead. Deleting by the latch key would leave a registry entry pointing at a
// disposed panel, and the next Open Preview on that document would reveal it and throw.
test("a preview is removed under the key it was filed under, not the one asked for", () => {
  const r = new PreviewRegistry();
  const degraded = fake("/w/book/one.tmd"); // in a project, but filed as a single file
  r.set(degraded);
  r.delete(degraded);
  assert.strictEqual(r.size, 0, "the entry must be gone whichever key was in play");
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

// Item 150 §3. One `taliesin preview <root>` serves the WHOLE book, so the registry can no
// longer be keyed by document: opening a second chapter would miss the entry, spawn a second
// server on a second port and leave two previews of one book side by side.
test("a second chapter of one book reveals the preview already serving it", () => {
  const r = new PreviewRegistry();
  const book = fake("/w/book/one.tmd", "/w/book");
  r.set(book);
  assert.strictEqual(r.get(previewKey("/w/book/two.tmd", "/w/book")), book);
  assert.strictEqual(r.size, 1);
});

test("a document in no project is still keyed by itself", () => {
  const r = new PreviewRegistry();
  const loose = fake("/w/loose/post.tmd");
  r.set(loose);
  assert.strictEqual(r.get(previewKey("/w/loose/post.tmd", null)), loose);
  // …and does not collide with a different loose document in the same directory.
  assert.strictEqual(r.get(previewKey("/w/loose/other.tmd", null)), undefined);
});

test("beginStart latches per project, so opening two chapters at once spawns one server", () => {
  const r = new PreviewRegistry();
  assert.strictEqual(r.beginStart(previewKey("/w/book/one.tmd", "/w/book")), true);
  assert.strictEqual(r.beginStart(previewKey("/w/book/two.tmd", "/w/book")), false);
});

test("previewFor maps a buffer to the preview whose project contains it", () => {
  // The old rule was "your own preview, else the only one" — which declines to guess as soon
  // as two are open. A project preview does not need to guess: containment is a fact.
  const r = new PreviewRegistry();
  r.set(fake("/w/book/one.tmd", "/w/book"));
  r.set(fake("/w/other.tmd"));
  assert.strictEqual(r.previewFor("/w/book/chapters/two.tmd")?.root, "/w/book");
});

test("containment stops at the directory boundary, not the string prefix", () => {
  // `/w/book` must not claim `/w/bookkeeping/notes.tmd`; two previews are open, so guessing
  // wrong here means marking a block in a document the author is not looking at.
  const r = new PreviewRegistry();
  r.set(fake("/w/book/one.tmd", "/w/book"));
  r.set(fake("/w/other.tmd"));
  assert.strictEqual(r.previewFor("/w/bookkeeping/notes.tmd"), undefined);
});
