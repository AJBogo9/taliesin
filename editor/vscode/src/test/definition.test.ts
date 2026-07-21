import { test } from "node:test";
import assert from "node:assert";
import { classifyHover, definitionSite, bibEntryOffset } from "../hover";

test("classifyHover classifies an include path token", () => {
  const line = "{{< include chapters/intro.tmd >}}";
  const t = classifyHover(line, 0, line.indexOf("chapters") + 2);
  assert.equal(t.kind, "include");
  assert.equal((t as any).path, "chapters/intro.tmd");
});

test("classifyHover classifies an embed path token too", () => {
  const line = "{{< embed deck.tmd >}}";
  const t = classifyHover(line, 0, line.indexOf("deck") + 1);
  assert.equal(t.kind, "include");
  assert.equal((t as any).path, "deck.tmd");
});

test("definitionSite finds a {#fig-x} definition and ignores the @fig-x reference", () => {
  const text = "See @fig-scree below.\n\n![Scree](s.png){#fig-scree}\n";
  const site = definitionSite(text, "fig-scree");
  assert.deepEqual(site, { line: 2, col: text.split("\n")[2].indexOf("fig-scree") });
});

test("definitionSite finds a `label: fig-x` cell definition", () => {
  const text = "```{python}\n#| label: fig-plot\nplot()\n```\n";
  const site = definitionSite(text, "fig-plot");
  assert.equal(site?.line, 1);
});

test("definitionSite returns null for an undefined id (or a bare reference)", () => {
  assert.equal(definitionSite("only @fig-x here\n", "fig-missing"), null);
  assert.equal(definitionSite("only @fig-x here\n", "fig-x"), null); // a reference is not a def
});

test("bibEntryOffset returns the entry offset and null for a missing key", () => {
  const bib = "@article{smith20,\n  title = {T},\n}\n@book{jones19, title={B}}\n";
  assert.equal(bibEntryOffset(bib, "jones19"), bib.indexOf("@book"));
  assert.equal(bibEntryOffset(bib, "nope"), null);
});
