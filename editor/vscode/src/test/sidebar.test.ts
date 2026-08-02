// The sidebar's two tree builders, tested as pure functions.
//
// They are deliberately separate from the `TreeDataProvider` that wraps them: a provider needs
// a VS Code host, and the shape of the tree is where the bugs are. `node --test` runs these
// with no editor at all, and the e2e suite covers the one thing only a real Extension Host can
// answer, which is whether VS Code accepted the view contributions.
import { test } from "node:test";
import assert from "node:assert";
import { outlineTree, refsTree, floatsTree } from "../sidebartree";

test("the outline tree nests headings under their page and by level", () => {
  const rows = outlineTree({
    root: "/r",
    pages: [
      {
        path: "/r/index.tmd",
        headings: [
          { line: 0, level: 1, text: "One" },
          { line: 4, level: 2, text: "Deeper" },
        ],
      },
    ],
    floats: [],
  });
  assert.strictEqual(rows.length, 1, "one page row");
  assert.strictEqual(rows[0].label, "index.tmd");
  assert.strictEqual(rows[0].children[0].label, "One");
  assert.strictEqual(
    rows[0].children[0].children[0].label,
    "Deeper",
    "a level-2 heading nests under the level-1 above it"
  );
});

test("a heading that skips a level still nests rather than being dropped", () => {
  // `# A` then `### C` is legal Markdown and common in real documents. A tree builder that
  // only accepts level+1 silently loses C, which is worse than showing it one level shallow.
  const rows = outlineTree({
    root: "/r",
    pages: [
      {
        path: "/r/a.tmd",
        headings: [
          { line: 0, level: 1, text: "A" },
          { line: 2, level: 3, text: "C" },
        ],
      },
    ],
    floats: [],
  });
  assert.strictEqual(rows[0].children[0].children[0].label, "C");
});

test("a heading shallower than the one before it closes back out to the right parent", () => {
  // `# A` / `## B` / `# D`: D is a sibling of A, not a child of B. Getting this wrong buries
  // half a book under its first chapter.
  const rows = outlineTree({
    root: "/r",
    pages: [
      {
        path: "/r/a.tmd",
        headings: [
          { line: 0, level: 1, text: "A" },
          { line: 2, level: 2, text: "B" },
          { line: 4, level: 1, text: "D" },
        ],
      },
    ],
    floats: [],
  });
  const top = rows[0].children.map((c) => c.label);
  assert.deepStrictEqual(top, ["A", "D"]);
  assert.deepStrictEqual(
    rows[0].children[0].children.map((c) => c.label),
    ["B"]
  );
});

test("the references tree separates resolved targets from dangling ones", () => {
  const rows = refsTree({
    root: "/r",
    targets: [
      {
        id: "sec-a",
        resolved: true,
        definedIn: "/r/a.tmd",
        definedLine: 0,
        uses: [{ path: "/r/b.tmd", line: 9, col: 4 }],
      },
      {
        id: "sec-gone",
        resolved: false,
        definedIn: null,
        definedLine: null,
        uses: [{ path: "/r/b.tmd", line: 10, col: 4 }],
      },
    ],
  });
  assert.deepStrictEqual(
    rows.map((r) => r.label),
    ["Resolved (1)", "Dangling (1)"]
  );
  assert.strictEqual(rows[1].children[0].label, "sec-gone");
  assert.strictEqual(
    rows[0].children[0].children.length,
    1,
    "a resolved target lists the uses pointing at it"
  );
});

test("a target nobody references is not listed as dangling", () => {
  // An anchor defined and never used is normal, not an error. Only a USE with no definition
  // is dangling, and conflating the two would fill the view with false alarms.
  const rows = refsTree({
    root: "/r",
    targets: [
      { id: "sec-unused", resolved: true, definedIn: "/r/a.tmd", definedLine: 0, uses: [] },
    ],
  });
  assert.deepStrictEqual(
    rows.map((r) => r.label),
    ["Resolved (1)", "Dangling (0)"]
  );
});

test("both group rows are present even when one side is empty", () => {
  // A view that hides its empty half reads as broken rather than as clean.
  const rows = refsTree({ root: "", targets: [] });
  assert.deepStrictEqual(
    rows.map((r) => r.label),
    ["Resolved (0)", "Dangling (0)"]
  );
});

test("an empty reply renders empty views rather than throwing", () => {
  assert.deepStrictEqual(outlineTree({ root: "", pages: [], floats: [] }), []);
  assert.deepStrictEqual(floatsTree({ root: "", pages: [], floats: [] }), []);
});

test("a float row shows its number and points at its page and line", () => {
  const rows = floatsTree({
    root: "/r",
    pages: [],
    floats: [{ id: "fig-a", path: "/r/two.tmd", line: 12, title: "", number: "2.1" }],
  });
  assert.strictEqual(rows[0].label, "fig-a");
  assert.strictEqual(rows[0].description, "2.1");
  assert.strictEqual(rows[0].path, "/r/two.tmd");
  assert.strictEqual(rows[0].line, 12);
});

test("a null reply, which the server sends outside a project, is an empty view", () => {
  // The server answers `null` for a standalone document. Throwing here would surface as a
  // broken view for the entirely normal case of editing a file that is not in a book.
  assert.deepStrictEqual(outlineTree(null), []);
  assert.deepStrictEqual(floatsTree(null), []);
  assert.deepStrictEqual(
    refsTree(null).map((r) => r.label),
    ["Resolved (0)", "Dangling (0)"]
  );
});

test("only the page being edited opens; the rest of the book stays one row each", () => {
  // A whole book used to open fully expanded, which is what made the trees sprawl far
  // enough to want a collapse-all button in the first place (item 196). The reader's
  // question is "where am I in THIS chapter", so that is the one that opens.
  const reply = {
    root: "/r",
    pages: [
      { path: "/r/one.tmd", headings: [{ line: 0, level: 1, text: "One" }] },
      { path: "/r/two.tmd", headings: [{ line: 0, level: 1, text: "Two" }] },
    ],
    floats: [],
  };
  const rows = outlineTree(reply, "/r/two.tmd");
  assert.strictEqual(rows[0].collapsed, true, "the page not being edited starts collapsed");
  assert.strictEqual(rows[1].collapsed, false, "the active page starts open");
  // Headings inside the open page are NOT collapsed: an outline whose sections are all shut
  // shows one line and answers nothing.
  assert.strictEqual(rows[1].children[0].collapsed, undefined);
});

test("with no active page nothing is forced open, so a big book stays readable", () => {
  const reply = {
    root: "/r",
    pages: [{ path: "/r/one.tmd", headings: [{ line: 0, level: 1, text: "One" }] }],
    floats: [],
  };
  assert.strictEqual(outlineTree(reply).length, 1);
  assert.strictEqual(outlineTree(reply)[0].collapsed, true);
});

test("the references view opens the half that needs attention and shuts the other", () => {
  const rows = refsTree({
    root: "/r",
    targets: [
      {
        id: "fig-ok",
        resolved: true,
        definedIn: "/r/a.tmd",
        definedLine: 3,
        uses: [{ path: "/r/a.tmd", line: 9, col: 0 }],
      },
      { id: "fig-missing", resolved: false, definedIn: null, definedLine: null, uses: [{ path: "/r/a.tmd", line: 11, col: 0 }] },
    ],
  });
  const [resolved, dangling] = rows;
  assert.strictEqual(resolved.collapsed, true, "Resolved is the long, uninteresting half");
  assert.strictEqual(dangling.collapsed, false, "Dangling is the actionable half");
  // A target's own uses stay shut either way: the id is the answer, the use list is the
  // follow-up question.
  assert.strictEqual(dangling.children[0].collapsed, true);
});

test("a book's outline follows the server's order and does not re-sort it", () => {
  // The server resolves `chapters:` (the same list the drawer and prev/next use) and sends
  // the pages already ordered. The tree must not have an opinion of its own about order —
  // two owners of "what follows what" is how they drift.
  const rows = outlineTree({
    root: "/r",
    book: true,
    pages: [
      { path: "/r/zeta.tmd", listed: true, headings: [] },
      { path: "/r/alpha.tmd", listed: true, headings: [] },
    ],
    floats: [],
  });
  assert.deepStrictEqual(
    rows.map((r) => r.label),
    ["zeta.tmd", "alpha.tmd"]
  );
});

test("a page missing from the book's chapters gets a named home, not silence", () => {
  const rows = outlineTree({
    root: "/r",
    book: true,
    pages: [
      { path: "/r/one.tmd", listed: true, headings: [] },
      { path: "/r/orphan.tmd", listed: false, headings: [] },
    ],
    floats: [],
  });
  assert.deepStrictEqual(
    rows.map((r) => r.label),
    ["one.tmd", "Unlisted (1)"],
    "an orphan chapter is an authoring mistake worth seeing, not a row to hide"
  );
  assert.strictEqual(rows[1].children[0].label, "orphan.tmd");
  assert.strictEqual(rows[1].collapsed, false, "the group that needs attention opens");
});

test("a book with nothing unlisted grows no group row", () => {
  const rows = outlineTree({
    root: "/r",
    book: true,
    pages: [{ path: "/r/one.tmd", listed: true, headings: [] }],
    floats: [],
  });
  assert.deepStrictEqual(
    rows.map((r) => r.label),
    ["one.tmd"],
    "an empty group would report a problem the project does not have"
  );
});

test("a website has no chapter list, so no page can be missing from it", () => {
  // The defined fallback. `listed` is meaningless without a list, and grouping every page
  // under `Unlisted` would tell a website author their whole site is misfiled.
  const rows = outlineTree({
    root: "/r",
    book: false,
    pages: [
      { path: "/r/a.tmd", listed: true, headings: [] },
      { path: "/r/b.tmd", listed: true, headings: [] },
    ],
    floats: [],
  });
  assert.deepStrictEqual(
    rows.map((r) => r.label),
    ["a.tmd", "b.tmd"]
  );
});
