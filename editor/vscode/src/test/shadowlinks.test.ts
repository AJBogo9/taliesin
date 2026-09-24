// Go-to-definition inside a code cell is answered against the SHADOW document (embedded.ts),
// a file the author never asked for. Every location that comes back has to be moved onto the
// `.tmd` before VS Code acts on it: a definition left pointing at the shadow would take the
// author into that file, a projection of their document with every prose line blanked,
// instead of the document itself.
import { test } from "node:test";
import assert from "node:assert";
import { remapDefinitions } from "../shadowlinks";

/** Enough of `vscode.Uri` for the remap: a scheme, and a string form to compare by. */
class FakeUri {
  constructor(
    readonly scheme: string,
    private readonly rest: string
  ) {}
  toString(): string {
    return `${this.scheme}:${this.rest}`;
  }
}

function range(startLine: number, endLine = startLine) {
  return { start: { line: startLine, character: 0 }, end: { line: endLine, character: 4 } };
}

const parent = new FakeUri("file", "///p/posts/demo/index.tmd");
const shadowDir = "file:///tmp/taliesin-shadows-Ab12Cd";
const shadow = new FakeUri("file", "///tmp/taliesin-shadows-Ab12Cd/index-0123456789ab.py");

test("a definition in the shadow lands in the .tmd at the same line", () => {
  const got = remapDefinitions([{ uri: shadow, range: range(4) }], shadow, parent, shadowDir);
  assert.deepStrictEqual(got, [{ targetUri: parent, targetRange: range(4) }]);
  assert.strictEqual(got[0].targetUri, parent, "the parent's own Uri object, not a copy");
});

test("a LocationLink keeps its origin and selection ranges; only the target moves", () => {
  // A different object that names the same shadow: the comparison is by URI, not identity.
  const sameShadow = new FakeUri("file", "///tmp/taliesin-shadows-Ab12Cd/index-0123456789ab.py");
  const got = remapDefinitions(
    [
      {
        originSelectionRange: range(9),
        targetUri: sameShadow,
        targetRange: range(2, 4),
        targetSelectionRange: range(2),
      },
    ],
    shadow,
    parent,
    shadowDir
  );
  assert.deepStrictEqual(got, [
    {
      originSelectionRange: range(9),
      targetUri: parent,
      targetRange: range(2, 4),
      targetSelectionRange: range(2),
    },
  ]);
});

test("a real file passes through untouched", () => {
  const numpy = new FakeUri("file", "///venv/site-packages/numpy/_core/function_base.py");
  const dom = new FakeUri("file", "///code/extensions/node_modules/typescript/lib/lib.dom.d.ts");
  const got = remapDefinitions(
    [
      { uri: numpy, range: range(120) },
      { originSelectionRange: range(3), targetUri: dom, targetRange: range(1200, 1210) },
    ],
    shadow,
    parent,
    shadowDir
  );
  assert.deepStrictEqual(got, [
    { targetUri: numpy, targetRange: range(120) },
    { originSelectionRange: range(3), targetUri: dom, targetRange: range(1200, 1210) },
  ]);
  assert.strictEqual(got[0].targetUri, numpy);
  assert.strictEqual(got[1].targetUri, dom);
});

test("no definition can ever point into the shadow directory", () => {
  // The shadow itself, and another document's shadow beside it: TypeScript can put two loose
  // files of one directory into one inferred project. Neither may survive as a target.
  const otherShadow = new FakeUri("file", "///tmp/taliesin-shadows-Ab12Cd/other-ba9876543210.py");
  const found = [
    { uri: shadow, range: range(4) },
    { uri: otherShadow, range: range(4) },
    { targetUri: otherShadow, targetRange: range(1) },
    { uri: new FakeUri("file", "///lib/real.py"), range: range(1) },
  ];
  const got = remapDefinitions(found, shadow, parent, shadowDir);
  for (const link of got) {
    assert.ok(!link.targetUri.toString().startsWith(`${shadowDir}/`), `${link.targetUri} is a shadow`);
  }
  assert.deepStrictEqual(
    got.map((l) => l.targetUri.toString()),
    ["file:///p/posts/demo/index.tmd", "file:///lib/real.py"]
  );
});

test("an untitled buffer is the author's own and passes through", () => {
  // Shadows are files now, so an untitled target can only be a buffer the author opened: an
  // unsaved scratch file, or an unsaved .tmd, which is also the parent the shadow maps onto.
  const scratch = new FakeUri("untitled", "Untitled-7");
  const unsaved = new FakeUri("untitled", "Untitled-1");
  const got = remapDefinitions(
    [
      { uri: shadow, range: range(3) },
      { uri: scratch, range: range(3) },
    ],
    shadow,
    unsaved,
    shadowDir
  );
  assert.deepStrictEqual(
    got.map((l) => l.targetUri.toString()),
    ["untitled:Untitled-1", "untitled:Untitled-7"]
  );
});

test("a sibling of the shadow directory is not inside it", () => {
  // The prefix test must stop at a path boundary: `...-Ab12Cd2/x.py` is not in `...-Ab12Cd`.
  const sibling = new FakeUri("file", "///tmp/taliesin-shadows-Ab12Cd2/x.py");
  const got = remapDefinitions([{ uri: sibling, range: range(1) }], shadow, parent, shadowDir);
  assert.deepStrictEqual(got, [{ targetUri: sibling, targetRange: range(1) }]);
});

test("no answer is an empty list", () => {
  assert.deepStrictEqual(remapDefinitions(undefined, shadow, parent, shadowDir), []);
  assert.deepStrictEqual(remapDefinitions(null, shadow, parent, shadowDir), []);
  assert.deepStrictEqual(remapDefinitions([], shadow, parent, shadowDir), []);
});
