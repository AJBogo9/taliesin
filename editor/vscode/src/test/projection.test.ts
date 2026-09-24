// The shadow a code cell's language server reads (embedded.ts): the `.tmd` with every line
// outside that language's cells blanked, so a position in the one is the same position in the
// other. A `{js}` cell also gets the function `tali-js.js` runs it in, written on the fence
// lines around its body, which are blank in the shadow anyway.
import { test } from "node:test";
import assert from "node:assert";
import { CellRegion, project } from "../projection";

const WRAP = { open: "(async function (tali) {", close: "});" };

const doc = [
  "---", // 0
  "title: T", // 1
  "---", // 2
  "```{python}", // 3
  "import os", // 4
  "```", // 5
  "Prose.", // 6
  "```{js}", // 7
  "//| echo: false", // 8
  "const a = 1;", // 9
  "return a;", // 10
  "```", // 11
];
const regions: CellRegion[] = [
  { language: "python", startLine: 4, endLine: 4 },
  { language: "js", startLine: 9, endLine: 10, wrap: WRAP },
];

test("a shadow keeps its language's cells, blanks every other line and keeps the count", () => {
  const shadow = project(doc, regions, "python").split("\n");
  assert.strictEqual(shadow.length, doc.length);
  assert.deepStrictEqual(shadow, [
    "# type: ignore",
    "",
    "",
    "",
    "import os",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
  ]);
});

test("a wrapped cell's function opens above its body and closes below it", () => {
  const shadow = project(doc, regions, "javascript").split("\n");
  assert.deepStrictEqual(shadow, [
    "// @ts-nocheck",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    WRAP.open, // the option line, blank in any shadow
    "const a = 1;",
    "return a;",
    WRAP.close, // the closing fence
    "export {};",
  ]);
});

test("an unterminated cell closes on a line appended after the document, then the export", () => {
  const open = ["Prose.", "```{js}", "const a = 1;", "a;"];
  const cell: CellRegion = { language: "js", startLine: 2, endLine: 3, wrap: WRAP };
  const shadow = project(open, [cell], "javascript").split("\n");
  assert.deepStrictEqual(shadow, [
    "// @ts-nocheck",
    WRAP.open,
    "const a = 1;",
    "a;",
    WRAP.close,
    "export {};",
  ]);
});

test("line 0 keeps the quiet header, so a cell whose fence is line 0 goes unwrapped", () => {
  // TypeScript reads `@ts-nocheck` only from a line comment before the first token, so the
  // header and a wrapper cannot share the line (a `/* @ts-nocheck */` is ignored).
  const first = ["```{js}", "x;", "```", "```{js}", "y;", "```"];
  const cells: CellRegion[] = [
    { language: "js", startLine: 1, endLine: 1, wrap: WRAP },
    { language: "js", startLine: 4, endLine: 4, wrap: WRAP },
  ];
  const shadow = project(first, cells, "javascript").split("\n");
  assert.deepStrictEqual(shadow, [
    "// @ts-nocheck",
    "x;",
    "",
    WRAP.open,
    "y;",
    WRAP.close,
    "export {};",
  ]);
  // An option line under the fence frees line 0: the wrapper opens on the option line.
  const opts = ["```{js}", "//| echo: false", "x;", "```"];
  const cell: CellRegion = { language: "js", startLine: 2, endLine: 2, wrap: WRAP };
  const wrapped = project(opts, [cell], "javascript").split("\n");
  assert.deepStrictEqual(wrapped, ["// @ts-nocheck", WRAP.open, "x;", WRAP.close, "export {};"]);
});

test("a wrapper belongs to its own language's shadow only", () => {
  const shadow = project(doc, regions, "python").split("\n");
  assert.ok(!shadow.includes(WRAP.open) && !shadow.includes(WRAP.close), shadow.join("\n"));
});
