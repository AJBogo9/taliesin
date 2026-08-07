// The Explorer badge's severity roll-up.
//
// The rows below are what `decorations.ts` flattens `vscode.languages.getDiagnostics()` into:
// one `(absolute path, severity name)` per Taliesin diagnostic. The severity *names* are the
// three `crates/core/src/diagnostics/codes.rs` defines, because `decorations.ts` converts VS
// Code's enum back to them — the badge and the gate must rank the same words.
import { test } from "node:test";
import assert from "node:assert";
import { worstByFile, badgeFor, type FileSeverity } from "../checkstatus";

const rows: FileSeverity[] = [
  { file: "/r/a.tmd", severity: "warning" },
  { file: "/r/a.tmd", severity: "error" },
  { file: "/r/b.tmd", severity: "suggestion" },
];

test("a file's worst severity wins over its others", () => {
  const worst = worstByFile(rows);
  assert.strictEqual(
    worst.get("/r/a.tmd"),
    "error",
    "a.tmd has both a warning and an error; the badge must show the worse one"
  );
  assert.strictEqual(worst.get("/r/b.tmd"), "suggestion");
});

test("severity order does not depend on the order diagnostics arrive in", () => {
  // The same two diagnostics, reversed. A naive last-write-wins map gets this wrong, and
  // neither the server nor VS Code promises an order within a file.
  assert.strictEqual(worstByFile([...rows].reverse()).get("/r/a.tmd"), "error");
});

test("a clean project decorates nothing", () => {
  assert.strictEqual(worstByFile([]).size, 0);
});

test("an unlocated diagnostic still badges its file", () => {
  // `_site.yml: warning[TAL-SHORTCODE]: …` has no line. It is still a real finding about a
  // real file, and dropping it would make a project look cleaner than it is.
  const unlocated: FileSeverity[] = [{ file: "/r/_site.yml", severity: "warning" }];
  assert.strictEqual(worstByFile(unlocated).get("/r/_site.yml"), "warning");
});

test("a row with no file is skipped rather than badging an empty key", () => {
  assert.strictEqual(worstByFile([{ file: "", severity: "error" }]).size, 0);
});

test("an unrecognised severity is treated as an error, not ignored", () => {
  // Matches `codes::severity_rank`, which ranks an unknown severity with error on purpose: a
  // diagnostic nobody classified is not something to silently stop caring about.
  const odd: FileSeverity[] = [
    { file: "/r/a.tmd", severity: "warning" },
    { file: "/r/a.tmd", severity: "catastrophe" },
  ];
  assert.strictEqual(worstByFile(odd).get("/r/a.tmd"), "error");
});

test("each severity has a distinct badge glyph", () => {
  const glyphs = (["error", "warning", "suggestion"] as const).map((s) => badgeFor(s).badge);
  assert.strictEqual(new Set(glyphs).size, 3, `badges must be distinguishable: ${glyphs}`);
  // VS Code truncates a file decoration badge to two characters; anything longer is silently
  // cut, which turns a considered glyph into a mystery.
  for (const g of glyphs) assert.ok(g.length <= 2, `badge ${g} is longer than 2 characters`);
});
