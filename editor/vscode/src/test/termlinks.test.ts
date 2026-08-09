// A DRIFT GATE, not just a unit test.
//
// `DIAGNOSTIC_LINE` is the one piece of Taliesin knowledge that lives in TypeScript instead of
// being asked of the server, because a terminal line is not a document and no LSP request can
// describe one. So it can drift: Rust changes how `check` prints a diagnostic, the pattern goes on
// matching nothing, and the only symptom is that links quietly stop appearing.
//
// The gate has two halves and needs both. The first tests the pattern against the shapes the tools
// print. The second pins the Rust FORMAT STRINGS those shapes come from, so changing either side
// without the other fails here. Testing only the pattern would leave it green while the format
// moved underneath it, which is exactly the failure this file exists to prevent.
import { test } from "node:test";
import assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { DIAGNOSTIC_LINE, resolveUnique } from "../diaglink";

const REPO_ROOT = path.join(__dirname, "..", "..", "..", "..");

test("check's located form matches, and yields the file and the line", () => {
  const m = DIAGNOSTIC_LINE.exec("posts/intro.tmd:12: warning: unresolved @fig-a");
  assert.ok(m, "the located form must match");
  assert.strictEqual(m[1], "posts/intro.tmd");
  assert.strictEqual(m[2], "12");
});

test("check's unlocated form matches, with no line", () => {
  const m = DIAGNOSTIC_LINE.exec("posts/intro.tmd: error: bad front matter");
  assert.ok(m, "the unlocated form must match");
  assert.strictEqual(m[1], "posts/intro.tmd");
  assert.strictEqual(m[2], undefined);
});

test("build's bare form matches", () => {
  const m = DIAGNOSTIC_LINE.exec("chapters/two.tmd:7: include not resolved");
  assert.ok(m);
  assert.strictEqual(m[1], "chapters/two.tmd");
  assert.strictEqual(m[2], "7");
});

test("prose that merely mentions a file is not a link", () => {
  // The `:` right after the extension is what makes a location; without it this is a sentence.
  assert.strictEqual(DIAGNOSTIC_LINE.exec("rendered posts/intro.tmd in 12ms"), null);
  assert.strictEqual(DIAGNOSTIC_LINE.exec("see posts/intro.tmd for the shape"), null);
});

test("a location must start the line, so a path inside a message is not matched", () => {
  // Anchored, so the severity colour (which sits after the path) cannot interfere, and a filename
  // quoted mid-message does not produce a second, wrong link.
  assert.strictEqual(DIAGNOSTIC_LINE.exec("  posts/intro.tmd:12: indented"), null);
  assert.strictEqual(DIAGNOSTIC_LINE.exec("warning: posts/intro.tmd:12: bad"), null);
});

test("the Rust format strings this pattern was written against have not moved", () => {
  // The other half of the gate. If a format changes, the samples above become fiction and the
  // pattern may match nothing while every test here still passes, so pin the literals.
  const lint = fs.readFileSync(path.join(REPO_ROOT, "crates/server/src/lint.rs"), "utf8");
  const build = fs.readFileSync(path.join(REPO_ROOT, "crates/server/src/build.rs"), "utf8");

  assert.ok(
    lint.includes('"{}:{}: {}: {}\\n"'),
    "lint.rs no longer prints `file:line: severity: message`; revisit DIAGNOSTIC_LINE"
  );
  assert.ok(
    lint.includes('"{}: {}: {}\\n"'),
    "lint.rs no longer prints the unlocated `file: severity: message`; revisit DIAGNOSTIC_LINE"
  );
  assert.ok(
    build.includes('"{}:{line}: {message}"'),
    "build.rs no longer prints `file:line: message`; revisit DIAGNOSTIC_LINE"
  );
  // `run_print.rs` was the third producer of this shape, and the newest, until Wave 13 cut
  // `taliesin run`. Two remain, and both are checked above.
  //
  // And the property the pattern depends on most: no column is printed anywhere. A `:col` group
  // would match none of the three forms.
  assert.ok(
    !lint.includes('"{}:{}:{}: '),
    "lint.rs appears to print a column now; DIAGNOSTIC_LINE has no column group"
  );
});

test("an ambiguous path produces no link rather than the wrong file", () => {
  const base = fs.mkdtempSync(path.join(os.tmpdir(), "tali-tl-"));
  const a = path.join(base, "a");
  const b = path.join(base, "b");
  fs.mkdirSync(a);
  fs.mkdirSync(b);
  fs.writeFileSync(path.join(a, "intro.tmd"), "");
  fs.writeFileSync(path.join(b, "intro.tmd"), "");

  // Two roots both hold `intro.tmd`. Opening the wrong chapter is worse than plain text: the
  // author edits a file that has no problem and the diagnostic never goes away.
  assert.strictEqual(resolveUnique("intro.tmd", [a, b]), null);
  // One root: unambiguous.
  assert.strictEqual(resolveUnique("intro.tmd", [a]), path.join(a, "intro.tmd"));
  // Named but absent.
  assert.strictEqual(resolveUnique("missing.tmd", [a, b]), null);
});
