// Populate ./.vscode-test with a VS Code build so the OFFLINE grammar tokenization test
// (src/test/grammar.test.ts) can load the bundled MIT markdown/python/yaml base grammars it
// `include`s. Locally this is already present from a prior e2e run; in CI it downloads once
// (the actual Extension Host is NOT launched here — this only unzips the grammars). Pinned to a
// known build for determinism; the grammar test locates grammars by glob, so the exact version
// does not matter.
const { downloadAndUnzipVSCode } = require("@vscode/test-electron");

downloadAndUnzipVSCode("1.126.0")
  .then((p) => console.log("VS Code base grammars ready at:", p))
  .catch((e) => {
    console.error("failed to fetch VS Code for the grammar test:", e);
    process.exit(1);
  });
