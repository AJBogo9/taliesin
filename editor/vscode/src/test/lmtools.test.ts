// The language-model tool surface: what is offered, and what is deliberately withheld.
//
// Three drift gates matter here, and they point in different directions: manifest vs code,
// code vs manifest, and both vs `mcp.rs`, which is the real owner of what Taliesin exposes to
// a model at all.
import { test } from "node:test";
import assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import { LM_TOOLS, WITHHELD_SUBCOMMANDS } from "../lmspecs";

const EXT_ROOT = path.join(__dirname, "..", "..");
const REPO_ROOT = path.join(EXT_ROOT, "..", "..");
const manifest = JSON.parse(fs.readFileSync(path.join(EXT_ROOT, "package.json"), "utf8"));
const declared: { name: string; toolReferenceName?: string; modelDescription?: string }[] =
  manifest.contributes.languageModelTools ?? [];

/** Every `name: "…"` in the MCP server's `TOOLS` table. */
function mcpToolNames(): string[] {
  const rust = fs.readFileSync(path.join(REPO_ROOT, "crates/server/src/mcp.rs"), "utf8");
  const table = rust.slice(rust.indexOf("const TOOLS"), rust.indexOf("/// `taliesin mcp`"));
  return [...table.matchAll(/name:\s*"([a-z_]+)"/g)].map((m) => m[1]);
}

test("every registered tool is declared in the manifest", () => {
  const names = new Set(declared.map((t) => t.name));
  for (const tool of LM_TOOLS) {
    assert.ok(names.has(tool.name), `${tool.name} is registered in code but not declared`);
  }
});

test("every declared tool is registered in code", () => {
  // The other direction. A manifest entry with no implementation is a tool the model can call
  // and get nothing from, which is worse than not offering it.
  const registered = new Set(LM_TOOLS.map((t) => t.name));
  for (const t of declared) {
    assert.ok(registered.has(t.name), `${t.name} is declared but never registered`);
  }
});

test("the sanity check on reading mcp.rs actually found the tool table", () => {
  // Guards this file's own probe. If the slice or the regex stops matching, the two drift
  // gates below would pass vacuously against an empty list and certify nothing.
  const names = mcpToolNames();
  assert.ok(names.length >= 5, `only found ${names.length} MCP tools: the probe is broken`);
  assert.ok(names.includes("check") && names.includes("vocab"), `got ${names}`);
});

test("every tool offered here is one the MCP server actually exposes", () => {
  // `mcp.rs` owns what Taliesin offers a model. Offering something here that it does not
  // expose means the two AI surfaces disagree about what this tool can do.
  const exposed = mcpToolNames();
  for (const tool of LM_TOOLS) {
    assert.ok(
      exposed.includes(tool.cli[0]),
      `${tool.name} runs \`${tool.cli[0]}\` but mcp.rs does not expose it`
    );
  }
});

test("no tool shells out to a subcommand that writes, executes or publishes", () => {
  // `build` writes `_site/` AND runs code cells. A model invoking it unprompted is a surprise
  // write and a surprise kernel run. The MCP server may offer it, because using that server is
  // an explicit opt-in; an always-on editor tool is not.
  for (const tool of LM_TOOLS) {
    for (const banned of WITHHELD_SUBCOMMANDS) {
      assert.ok(
        !tool.cli.includes(banned),
        `${tool.name} would run \`${banned}\`, which writes or executes`
      );
    }
  }
});

test("build is exposed by the MCP server and withheld here, on purpose", () => {
  // Pins the asymmetry itself, so a later change that quietly adds `build` to the editor
  // surface has to argue with a test rather than slip through.
  assert.ok(mcpToolNames().includes("build"), "precondition: mcp.rs exposes build");
  assert.ok(
    !LM_TOOLS.some((t) => t.cli[0] === "build"),
    "build must stay out of the always-on editor tool surface"
  );
});

test("every declared tool can be referenced in a prompt and describes itself", () => {
  for (const t of declared) {
    assert.ok(t.toolReferenceName, `${t.name} has no toolReferenceName, so #-referencing fails`);
    assert.ok(
      (t.modelDescription ?? "").length > 30,
      `${t.name} needs a modelDescription the model can act on`
    );
  }
});

test("the MCP provider id the source registers is the one the manifest declares", () => {
  // VS Code matches these by string. A mismatch means the provider is registered and never
  // consulted, which looks exactly like the server not existing.
  const declaredIds: string[] = (manifest.contributes.mcpServerDefinitionProviders ?? []).map(
    (p: { id: string }) => p.id
  );
  const source = fs.readFileSync(path.join(EXT_ROOT, "src", "lmtools.ts"), "utf8");
  const used = [...source.matchAll(/registerMcpServerDefinitionProvider\(\s*"([^"]+)"/g)].map(
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

test("only the path-taking tools declare a path input", () => {
  for (const spec of LM_TOOLS) {
    const d = declared.find((x) => x.name === spec.name) as
      | { inputSchema?: { properties?: Record<string, unknown>; required?: string[] } }
      | undefined;
    const props = Object.keys(d?.inputSchema?.properties ?? {});
    if (spec.takesPath) {
      assert.deepStrictEqual(props, ["path"], `${spec.name} should take a path`);
      assert.deepStrictEqual(d?.inputSchema?.required, ["path"]);
    } else {
      // `vocab` takes no arguments. Declaring a required path it ignores would make every
      // invocation need a value that changes nothing.
      assert.deepStrictEqual(props, [], `${spec.name} takes no arguments`);
    }
  }
});
