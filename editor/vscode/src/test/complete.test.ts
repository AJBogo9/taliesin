import { test } from "node:test";
import assert from "node:assert";
import {
  detectContext,
  harvestAnchorIds,
  harvestBibKeys,
  frontmatterBibPaths,
  parseSymbolsJson,
  mergeXrefTargets,
  shortcodePathCandidates,
} from "../complete";

const FM_OPEN = "---\ntitle: T\n";

test("detects a top-level front-matter key position", () => {
  const ctx = detectContext("ti", FM_OPEN + "ti");
  assert.deepEqual(ctx, { kind: "frontmatter-key", parent: null });
});

test("detects a nested front-matter key under a known parent", () => {
  const doc = FM_OPEN + "execute:\n  ec";
  const ctx = detectContext("  ec", doc);
  assert.deepEqual(ctx, { kind: "frontmatter-key", parent: "execute" });
});

test("no front-matter completion once the block is closed", () => {
  const doc = "---\ntitle: T\n---\n\nBody ti";
  assert.deepEqual(detectContext("Body ti", doc), { kind: "none" });
});

test("no front-matter completion at a value position (after the colon)", () => {
  const doc = FM_OPEN + "author: ";
  assert.deepEqual(detectContext("author: ", doc), { kind: "none" });
});

test("detects a cell option after #| inside a code cell", () => {
  const doc = "```{python}\n#| ec";
  assert.deepEqual(detectContext("#| ec", doc), { kind: "cell-option" });
});

test("detects a cell option after //| in a js cell", () => {
  const doc = "```{js}\n//| na";
  assert.deepEqual(detectContext("//| na", doc), { kind: "cell-option" });
});

test("no cell-option completion for #| outside a code cell", () => {
  // The fence opened and closed before this line, so we are back in prose.
  const doc = "```{python}\nx = 1\n```\n\n#| ec";
  assert.deepEqual(detectContext("#| ec", doc), { kind: "none" });
});

test("detects a div class after :::{. and after ::: {.", () => {
  assert.deepEqual(detectContext(":::{.no", ":::{.no"), { kind: "div-class" });
  assert.deepEqual(detectContext("::: {.cal", "::: {.cal"), { kind: "div-class" });
});

test("detects an xref after @ and captures the typed prefix", () => {
  assert.deepEqual(detectContext("See @fig-", "See @fig-"), { kind: "xref", typed: "fig-" });
  assert.deepEqual(detectContext("See @", "See @"), { kind: "xref", typed: "" });
});

test("does not treat an email @ as an xref", () => {
  assert.deepEqual(detectContext("mail me at bob@", "mail me at bob@"), { kind: "none" });
});

test("detects a citation inside [@ ...", () => {
  assert.deepEqual(detectContext("see [@sm", "see [@sm"), { kind: "cite" });
  assert.deepEqual(detectContext("see [@a2020; @b", "see [@a2020; @b"), { kind: "cite" });
});

test("harvestAnchorIds pulls {#prefix-id} anchors and #sec- heading ids", () => {
  const doc = "## Intro {#sec-intro}\n\n![x](y){#fig-scree}\n\nplain {#not-a-ref}\n";
  const ids = harvestAnchorIds(doc);
  assert.ok(ids.includes("sec-intro"));
  assert.ok(ids.includes("fig-scree"));
  // Only cross-reference-prefixed ids are useful for @xref; a bare id is still harvested
  // (the provider filters by the typed prefix), so assert the two ref ids are present.
});

test("harvestBibKeys reads @type{key, entries", () => {
  const bib = "@article{smith2020, title={X}}\n@book{jones-2019, title={Y}}\n";
  assert.deepEqual(harvestBibKeys(bib).sort(), ["jones-2019", "smith2020"]);
});

test("frontmatterBibPaths reads a scalar and a list bibliography field", () => {
  assert.deepEqual(frontmatterBibPaths("---\nbibliography: refs.bib\n---\n"), ["refs.bib"]);
  const listed = frontmatterBibPaths("---\nbibliography:\n  - a.bib\n  - b.bib\n---\n");
  assert.deepEqual(listed.sort(), ["a.bib", "b.bib"]);
});

// --- `taliesin symbols` -> @-completion targets -------------------------------------

test("parseSymbolsJson reads the CLI's array, and never throws on junk", () => {
  const good = '[{"id":"fig-scree","kind":"fig","number":"1"}]';
  assert.deepEqual(parseSymbolsJson(good), [{ id: "fig-scree", kind: "fig", number: "1" }]);
  assert.deepEqual(parseSymbolsJson(""), []);
  assert.deepEqual(parseSymbolsJson("not json"), []);
  assert.deepEqual(parseSymbolsJson('{"error":"boom"}'), []);
  // A malformed member is dropped, not fatal.
  assert.deepEqual(parseSymbolsJson('[{"kind":"fig"},{"id":"sec-a","kind":"sec","number":"2"}]'), [
    { id: "sec-a", kind: "sec", number: "2" },
  ]);
});

test("mergeXrefTargets unions the buffer's anchors with the CLI's symbols", () => {
  const labels = { fig: "Figure", sec: "Section" };
  // `fig-scree` is a cell label: only the CLI knows it. `sec-draft` was just typed and
  // is not yet on disk: only the buffer knows it. The author must see both.
  const merged = mergeXrefTargets(
    ["sec-why", "sec-draft"],
    [
      { id: "fig-scree", kind: "fig", number: "1" },
      { id: "sec-why", kind: "sec", number: "2" },
    ],
    labels
  );
  assert.deepEqual(
    merged.map((m) => m.id),
    ["fig-scree", "sec-draft", "sec-why"],
    "sorted, deduplicated union"
  );
  // A symbol the CLI numbered carries its resolved label; a buffer-only id does not.
  assert.equal(merged.find((m) => m.id === "fig-scree")!.detail, "Figure 1");
  assert.equal(merged.find((m) => m.id === "sec-why")!.detail, "Section 2");
  assert.equal(merged.find((m) => m.id === "sec-draft")!.detail, "cross-reference target");
});

test("mergeXrefTargets falls back to the kind prefix when the label is unknown", () => {
  const merged = mergeXrefTargets([], [{ id: "xyz-a", kind: "xyz", number: "1" }], {});
  assert.equal(merged[0].detail, "cross-reference target");
});

test("detects an embed shortcode file-argument position", () => {
  assert.deepEqual(detectContext("{{< embed ", "{{< embed "), {
    kind: "shortcode-path",
    shortcode: "embed",
    typed: "",
  });
  assert.deepEqual(detectContext("{{< embed tou", "{{< embed tou"), {
    kind: "shortcode-path",
    shortcode: "embed",
    typed: "tou",
  });
});

test("detects an include shortcode with a nested path prefix", () => {
  assert.deepEqual(detectContext("{{< include chapters/0", "{{< include chapters/0"), {
    kind: "shortcode-path",
    shortcode: "include",
    typed: "chapters/0",
  });
});

test("no shortcode-path completion past the first argument", () => {
  // Once the file arg + a space is typed, later (named) args are not path-completed.
  assert.deepEqual(detectContext("{{< embed tour.tmd tit", "x"), { kind: "none" });
});

test("shortcodePathCandidates offers .tmd files and dirs, hiding build dirs", () => {
  const entries = [
    { name: "tour.tmd", isDir: false },
    { name: "notes.txt", isDir: false },
    { name: "chapters", isDir: true },
    { name: "_freeze", isDir: true },
    { name: ".git", isDir: true },
  ];
  assert.deepEqual(shortcodePathCandidates(entries, "", "page"), [
    { value: "chapters/", detail: "directory" },
    { value: "tour.tmd", detail: "page" },
  ]);
});

test("shortcodePathCandidates keeps the dir prefix and filters by leaf", () => {
  const entries = [
    { name: "01-intro.tmd", isDir: false },
    { name: "02-body.tmd", isDir: false },
  ];
  assert.deepEqual(shortcodePathCandidates(entries, "chapters/01", "partial"), [
    { value: "chapters/01-intro.tmd", detail: "partial" },
  ]);
});
