import test from "node:test";
import assert from "node:assert/strict";
import { classifyHover, bibEntryFor } from "../hover";

test("hover: classifies an @xref token, spanning @ through the id", () => {
  const line = "As shown in @fig-scree, the elbow is clear.";
  const at = line.indexOf("fig-scree") + 2; // somewhere inside the id
  const t = classifyHover(line, 0, at);
  assert.equal(t.kind, "xref");
  if (t.kind !== "xref") return;
  assert.equal(t.id, "fig-scree");
  assert.equal(line.slice(t.start, t.end), "@fig-scree");
});

test("hover: an @xref at the start of the line is recognized", () => {
  const line = "@thm-lagrange gives the result.";
  const t = classifyHover(line, 0, 3);
  assert.equal(t.kind, "xref");
  if (t.kind !== "xref") return;
  assert.equal(t.id, "thm-lagrange");
});

test("hover: a position off any token classifies as none", () => {
  const line = "Plain prose with no targets here.";
  assert.equal(classifyHover(line, 0, 4).kind, "none");
});

test("hover: an email local-part is not an xref", () => {
  const line = "Reach me at sam@fig.example for details.";
  const at = line.indexOf("fig.example");
  assert.equal(classifyHover(line, 0, at).kind, "none");
});

test("hover: classifies a [@cite] key, and cite wins over xref for the inner @", () => {
  const line = "This follows Knuth [@knuth1984tex] closely.";
  const at = line.indexOf("knuth1984tex") + 1;
  const t = classifyHover(line, 0, at);
  assert.equal(t.kind, "cite");
  if (t.kind !== "cite") return;
  assert.equal(t.key, "knuth1984tex");
  assert.equal(line.slice(t.start, t.end), "@knuth1984tex");
});

test("hover: classifies a top-level front-matter key", () => {
  const doc = "---\ntitle: My Post\nauthor: A\n---\n\nBody.\n";
  // line 1 is `title: My Post`; hover on `title`
  const t = classifyHover(doc, 1, 2);
  assert.equal(t.kind, "frontmatter-key");
  if (t.kind !== "frontmatter-key") return;
  assert.equal(t.key, "title");
  assert.equal(t.parent, null);
});

test("hover: a nested front-matter key resolves its parent", () => {
  const doc = "---\nexecute:\n  echo: false\n---\n\nBody.\n";
  // line 2 is `  echo: false`; hover on `echo`
  const t = classifyHover(doc, 2, 3);
  assert.equal(t.kind, "frontmatter-key");
  if (t.kind !== "frontmatter-key") return;
  assert.equal(t.key, "echo");
  assert.equal(t.parent, "execute");
});

test("hover: a front-matter VALUE is not classified as a key", () => {
  const doc = "---\ntitle: My Post\n---\n\nBody.\n";
  const line = "title: My Post";
  const at = line.indexOf("My"); // on the value, not the key
  assert.equal(classifyHover(doc, 1, at).kind, "none");
});

test("hover: a key-looking line OUTSIDE the front matter is not a front-matter key", () => {
  const doc = "---\ntitle: X\n---\n\nnote: this is prose, not YAML\n";
  const t = classifyHover(doc, 4, 1); // on `note`
  assert.equal(t.kind, "none");
});

test("bibEntryFor: extracts the raw entry for a key", () => {
  const bib = [
    "@book{knuth1984tex,",
    "  title = {The TeXbook},",
    "  author = {Knuth, Donald E.},",
    "  year = {1984}",
    "}",
    "",
    "@article{other2020,",
    "  title = {Something Else}",
    "}",
  ].join("\n");
  const e = bibEntryFor(bib, "knuth1984tex");
  assert.ok(e, "found the entry");
  assert.ok(e!.startsWith("@book{knuth1984tex,"));
  assert.ok(e!.includes("The TeXbook"));
  assert.ok(!e!.includes("Something Else"), "stops at the entry's own closing brace");
});

test("bibEntryFor: handles nested braces in a field value", () => {
  const bib = "@article{k, title = {A study of {NP}-hardness}, year = {2001}}";
  const e = bibEntryFor(bib, "k");
  assert.ok(e);
  assert.ok(e!.includes("{NP}-hardness"));
  assert.ok(e!.endsWith("}"));
});

test("bibEntryFor: returns null for an absent key", () => {
  assert.equal(bibEntryFor("@book{a, title={X}}", "missing"), null);
});
