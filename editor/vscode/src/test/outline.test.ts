import { test } from "node:test";
import assert from "node:assert";
import { outline } from "../outline";

test("nests headings by level", () => {
  const t = "# A\n\ntext\n\n## B\n\n## C\n";
  const tree = outline(t);
  assert.equal(tree.length, 1);
  assert.equal(tree[0].title, "A");
  assert.deepEqual(
    tree[0].children.map((c) => c.title),
    ["B", "C"]
  );
});

test("a section runs to just before the next same-or-higher heading", () => {
  const t = "# A\nl1\n## B\nl3\n# C\n"; // lines 0..4
  const tree = outline(t);
  assert.equal(tree[0].endLine, 3); // A's body ends before `# C`
  assert.equal(tree[0].children[0].endLine, 3); // B ends before `# C`
  assert.equal(tree[1].startLine, 4);
});

test("ignores headings inside a fenced code block", () => {
  const t = "# Real\n\n```\n# not a heading\n```\n";
  const tree = outline(t);
  assert.deepEqual(
    tree.map((n) => n.title),
    ["Real"]
  );
});

test("ignores the front-matter block and strips a trailing {#id}", () => {
  const t = "---\ntitle: X\n# fake\n---\n# Intro {#sec-intro}\n";
  const tree = outline(t);
  assert.deepEqual(
    tree.map((n) => n.title),
    ["Intro"]
  );
});

test("a deeper heading nests under a shallower sibling's predecessor", () => {
  const t = "# A\n## B\n### C\n## D\n";
  const tree = outline(t);
  assert.equal(tree[0].children.length, 2); // B and D under A
  assert.equal(tree[0].children[0].children[0].title, "C"); // C under B
});

test("no headings yields an empty outline", () => {
  assert.deepEqual(outline("just prose\n\nmore prose\n"), []);
});
