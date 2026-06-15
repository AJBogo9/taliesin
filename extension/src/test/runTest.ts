import * as path from "path";
import { runTests } from "@vscode/test-electron";

async function main() {
  // The folder containing the extension manifest (package.json).
  const extensionDevelopmentPath = path.resolve(__dirname, "../../");
  // The compiled Mocha entry point.
  const extensionTestsPath = path.resolve(__dirname, "./suite/index");

  // Some sandboxes set ELECTRON_RUN_AS_NODE=1, which makes the downloaded VS
  // Code binary run as plain Node and reject all GUI flags. Clear it so the
  // launched instance is a real (headless) editor.
  delete process.env.ELECTRON_RUN_AS_NODE;

  await runTests({
    extensionDevelopmentPath,
    extensionTestsPath,
    // Sandbox often can't initialize under restricted user namespaces; the
    // dev extension is still loaded despite --disable-extensions.
    launchArgs: ["--no-sandbox", "--disable-extensions"],
  });
}

main().catch((err) => {
  console.error("Failed to run extension tests:", err);
  process.exit(1);
});
