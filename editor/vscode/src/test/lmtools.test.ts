// The AI surface this companion offers, which after Wave 4.3 is exactly one: `taliesin mcp`,
// advertised to VS Code rather than hand-registered in user configuration.
//
// The five always-on language-model tools that used to sit beside it are gone. They duplicated
// a subset of the MCP server's own table twenty lines below them, and the duplication is what
// made them worth cutting rather than the capability: an agent that wants `check` or `read`
// still gets them, through the server the user opted into.
import { test } from "node:test";
import assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";

const EXT_ROOT = path.join(__dirname, "..", "..");
const REPO_ROOT = path.join(EXT_ROOT, "..", "..");
const manifest = JSON.parse(fs.readFileSync(path.join(EXT_ROOT, "package.json"), "utf8"));
const SOURCE = fs.readFileSync(path.join(EXT_ROOT, "src", "lmtools.ts"), "utf8");

/** Every `name: "…"` in the MCP server's `TOOLS` table. */
function mcpToolNames(): string[] {
  const rust = fs.readFileSync(path.join(REPO_ROOT, "crates/server/src/mcp.rs"), "utf8");
  const table = rust.slice(rust.indexOf("const TOOLS"), rust.indexOf("/// `taliesin mcp`"));
  return [...table.matchAll(/name:\s*"([a-z_]+)"/g)].map((m) => m[1]);
}

test("the sanity check on reading mcp.rs actually found the tool table", () => {
  // Guards this file's own probe. If the slice or the regex stops matching, the gate below
  // would pass vacuously against an empty list and certify nothing.
  const names = mcpToolNames();
  assert.ok(names.length >= 5, `only found ${names.length} MCP tools: the probe is broken`);
  assert.ok(names.includes("check") && names.includes("vocab"), `got ${names}`);
});

test("the MCP server is the whole AI surface: no always-on editor tools", () => {
  // The retired register for Wave 4.3. `languageModelTools` are offered to every model in the
  // editor with no opt-in, which is why the five that existed were a duplicate surface rather
  // than a second opinion. Pointing an agent at the MCP server is an explicit act; a tool
  // VS Code hands it unprompted is not. Re-adding one has to argue with this test.
  assert.deepStrictEqual(
    manifest.contributes.languageModelTools ?? [],
    [],
    "the manifest must declare no always-on language-model tools"
  );
  assert.ok(
    !/vscode\.lm\.registerTool\b/.test(SOURCE),
    "no always-on language-model tool may be registered in code"
  );
  // And the capability itself did not go with them: the server still exposes the full table.
  assert.ok(mcpToolNames().includes("check"), "the MCP server still offers `check`");
});

test("the MCP provider id the source registers is the one the manifest declares", () => {
  // VS Code matches these by string. A mismatch means the provider is registered and never
  // consulted, which looks exactly like the server not existing.
  const declaredIds: string[] = (manifest.contributes.mcpServerDefinitionProviders ?? []).map(
    (p: { id: string }) => p.id
  );
  const used = [...SOURCE.matchAll(/registerMcpServerDefinitionProvider\(\s*"([^"]+)"/g)].map(
    (m) => m[1]
  );
  assert.ok(used.length > 0, "no registerMcpServerDefinitionProvider call found in lmtools.ts");
  for (const id of used) {
    assert.ok(declaredIds.includes(id), `provider id "${id}" is registered but not declared`);
  }
  for (const id of declaredIds) {
    assert.ok(used.includes(id), `provider id "${id}" is declared but never registered`);
  }
});
