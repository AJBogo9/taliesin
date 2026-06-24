import * as path from "node:path";
import { runTests } from "@vscode/test-electron";

// Downloads a throwaway VS Code, launches it headless with this extension loaded, and
// runs the Mocha suite (out/e2e/suite) inside the real Extension Host. Verifies the VS
// Code API wiring that the node:test unit suite can't reach.
async function main() {
  // Some sandboxes/agents export ELECTRON_RUN_AS_NODE=1 globally, which makes the
  // downloaded `code` binary run as plain Node and reject every GUI arg. Clear it so the
  // spawned VS Code launches as the real Electron app. (Unset on normal machines → no-op.)
  delete process.env.ELECTRON_RUN_AS_NODE;
  const extensionDevelopmentPath = path.resolve(__dirname, "../../");
  const extensionTestsPath = path.resolve(__dirname, "./suite/index");
  try {
    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath,
      launchArgs: ["--no-sandbox", "--disable-gpu"],
    });
  } catch (err) {
    console.error("e2e run failed:", err);
    process.exit(1);
  }
}

main();
