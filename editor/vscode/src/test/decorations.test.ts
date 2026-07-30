// The Explorer badge's severity roll-up.
//
// The diagnostic objects below are the real `check --format json` shape, taken from a run
// (`{code, docs_url, file, line, message, severity}` with lowercase severities), not invented.
import { test } from "node:test";
import assert from "node:assert";
import { worstByFile, badgeFor, type CheckJson } from "../checkstatus";

const json: CheckJson = {
  diagnostics: [
    { severity: "warning", file: "a.tmd", line: 3, code: "TAL-A11Y-ALT", message: "w" },
    { severity: "error", file: "a.tmd", line: 9, code: "TAL-ASSET", message: "e" },
    { severity: "suggestion", file: "b.tmd", line: 1, code: "TAL-CITE-UNUSED", message: "s" },
  ],
  environment: [],
};

test("a file's worst severity wins over its others", () => {
  const worst = worstByFile(json, "/r");
  assert.strictEqual(
    worst.get("/r/a.tmd"),
    "error",
    "a.tmd has both a warning and an error; the badge must show the worse one"
  );
  assert.strictEqual(worst.get("/r/b.tmd"), "suggestion");
});

test("severity order does not depend on the order diagnostics arrive in", () => {
  // The same two diagnostics, reversed. A naive last-write-wins map gets this wrong, and
  // `check` does not promise an order (the real run above emitted line 9 before line 7).
  const reversed: CheckJson = { diagnostics: [...json.diagnostics].reverse(), environment: [] };
  assert.strictEqual(worstByFile(reversed, "/r").get("/r/a.tmd"), "error");
});

test("a clean project decorates nothing", () => {
  assert.strictEqual(worstByFile({ diagnostics: [], environment: [] }, "/r").size, 0);
});

test("an already-absolute file path is not re-rooted", () => {
  const abs: CheckJson = {
    diagnostics: [
      { severity: "error", file: "/elsewhere/c.tmd", line: 1, code: "T", message: "m" },
    ],
    environment: [],
  };
  assert.ok(
    worstByFile(abs, "/r").has("/elsewhere/c.tmd"),
    "joining a root onto an absolute path produces a key nothing in the Explorer matches"
  );
});

test("a nested relative path resolves under the root", () => {
  const nested: CheckJson = {
    diagnostics: [
      { severity: "warning", file: "using/formats.tmd", line: 2, code: "T", message: "m" },
    ],
    environment: [],
  };
  assert.ok(worstByFile(nested, "/r").has("/r/using/formats.tmd"));
});

test("an unlocated diagnostic still badges its file", () => {
  // `_site.yml: warning[TAL-SHORTCODE]: …` has no line. It is still a real finding about a
  // real file, and dropping it would make a project look cleaner than it is.
  const unlocated: CheckJson = {
    diagnostics: [
      { severity: "warning", file: "_site.yml", line: null, code: "TAL-SHORTCODE", message: "m" },
    ],
    environment: [],
  };
  assert.strictEqual(worstByFile(unlocated, "/r").get("/r/_site.yml"), "warning");
});

test("an unrecognised severity is treated as an error, not ignored", () => {
  // Matches `codes::severity_rank`, which ranks an unknown severity with error on purpose: a
  // diagnostic nobody classified is not something to silently stop caring about.
  const odd: CheckJson = {
    diagnostics: [
      { severity: "warning", file: "a.tmd", line: 1, code: "T", message: "m" },
      { severity: "catastrophe", file: "a.tmd", line: 2, code: "T", message: "m" },
    ],
    environment: [],
  };
  assert.strictEqual(worstByFile(odd, "/r").get("/r/a.tmd"), "error");
});

test("each severity has a distinct badge glyph", () => {
  const glyphs = (["error", "warning", "suggestion"] as const).map((s) => badgeFor(s).badge);
  assert.strictEqual(new Set(glyphs).size, 3, `badges must be distinguishable: ${glyphs}`);
  // VS Code truncates a file decoration badge to two characters; anything longer is silently
  // cut, which turns a considered glyph into a mystery.
  for (const g of glyphs) assert.ok(g.length <= 2, `badge ${g} is longer than 2 characters`);
});
