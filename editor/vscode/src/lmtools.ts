// `taliesin mcp` advertised to VS Code, so the MCP server this project already ships is
// discovered instead of hand-registered in user configuration.
//
// This is the companion's ONLY AI surface. Five always-on language-model tools used to sit
// above it, wrapping `check`/`read`/`symbols`/`map`/`vocab` — a hand-maintained subset of the
// table the server below already exposes, kept honest against `mcp.rs` by drift gates. The
// capability was never the problem; the second copy of it was. What survives is the surface a
// user opts into, which is also the wider one.

import * as vscode from "vscode";

/**
 * Advertise `taliesin mcp` to VS Code, so the MCP server this project already ships is
 * discovered instead of hand-registered in user configuration.
 *
 * The whole server exists already; this is the difference between "there is an MCP server, go
 * read the docs and edit a JSON file" and "it is there". It offers `build`, which writes and
 * executes — correct here, because pointing an agent at an MCP server is an explicit act,
 * where an always-on editor tool is not.
 *
 * Needs VS Code 1.101: `registerMcpServerDefinitionProvider` is absent from the 1.100 API
 * surface. That measurement is what set the engine floor, and `manifest.test.ts` pins it.
 */
export function registerMcpProvider(context: vscode.ExtensionContext): void {
  const changed = new vscode.EventEmitter<void>();
  context.subscriptions.push(
    changed,
    vscode.lm.registerMcpServerDefinitionProvider("taliesin", {
      onDidChangeMcpServerDefinitions: changed.event,
      provideMcpServerDefinitions: async () => {
        // Read at call time, not at activation: the author may point `taliesin.path` at a
        // different build after the extension has started.
        const binary = vscode.workspace
          .getConfiguration("taliesin")
          .get<string>("path", "taliesin");
        return [new vscode.McpStdioServerDefinition("Taliesin", binary, ["mcp"])];
      },
    }),
    // A changed binary path means a different server; tell VS Code to ask again.
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("taliesin.path")) changed.fire();
    })
  );
}
