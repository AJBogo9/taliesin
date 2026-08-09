// Populate ./.vscode-test with a VS Code build so the OFFLINE grammar tokenization test
// (src/test/grammar.test.ts) can load the bundled MIT markdown/python/yaml base grammars it
// `include`s. In CI it downloads once (the Extension Host is NOT launched here — this only
// unzips the grammars). This is the ONLY thing `@vscode/test-electron` is still here for: the
// Extension Host e2e suite that used to share it went on 2026-08-09, and deleting the dependency
// with it broke the grammar gate. Pinned to a
// known build for determinism; the grammar test locates grammars by glob, so the exact version
// does not matter.
const { downloadAndUnzipVSCode } = require("@vscode/test-electron");

downloadAndUnzipVSCode("1.126.0")
  .then((p) => console.log("VS Code base grammars ready at:", p))
  .catch((e) => {
    console.error("failed to fetch VS Code for the grammar test:", e);
    process.exit(1);
  });
