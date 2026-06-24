// Relay-bridge verification harness (browser-driven, not part of `npm test`).
//
// The webview relay (src/webview.ts `relayHtml`) bridges two contexts that don't exist
// in a Node test: the preview iframe and the VS Code extension host. This harness serves
// the REAL `relayHtml` output with a same-origin stub iframe so a browser (e.g. the
// chrome-devtools MCP) can drive both message directions against the actual code.
//
// Usage:
//   1. npm run compile-tests        # produce out/webview.js
//   2. node scripts/relay-harness.cjs   # prints RELAY_PORT=<n>, serves on it
//   3. In a browser, inject a stub before load:
//        window.acquireVsCodeApi = () => ({ postMessage: m => (window.qmdToHost ??= []).push(m) });
//      navigate to http://127.0.0.1:<n>/ , then:
//        - FORWARD: the stub iframe auto-posts a `qmd-goto`; assert window.qmdToHost has it.
//        - REVERSE: window.postMessage({type:'qmd-cursor',file:null,line:9},'*');
//          assert document.getElementById('qmd').contentWindow.qmdGot === that message.
//
// Verified 2026-06-24 via the chrome-devtools MCP: both directions route correctly.
const http = require("node:http");
const fs = require("node:fs");
const path = require("node:path");
const { relayHtml } = require("../out/webview.js");

const dir = fs.mkdtempSync(path.join(require("node:os").tmpdir(), "qmd-relay-"));
fs.writeFileSync(
  path.join(dir, "inner.html"),
  `<!doctype html><meta charset=utf-8><body><script>
  window.qmdGot = null;
  window.addEventListener("message", function (e) {
    if (e.data && e.data.type === "qmd-cursor") window.qmdGot = e.data;
  });
  parent.postMessage({ type: "qmd-goto", source_file: null, sourcepos: "7:1" }, "*");
</script></body>`
);

const srv = http.createServer((req, res) => {
  const name = req.url === "/" ? "/relay.html" : req.url;
  fs.readFile(path.join(dir, name), (err, buf) => {
    if (err) { res.statusCode = 404; res.end("not found"); return; }
    res.setHeader("content-type", "text/html");
    res.end(buf);
  });
});
srv.listen(0, "127.0.0.1", () => {
  const port = srv.address().port;
  fs.writeFileSync(
    path.join(dir, "relay.html"),
    relayHtml(`http://127.0.0.1:${port}/inner.html`, "vscode-webview://unit-test")
  );
  console.log("RELAY_PORT=" + port);
});
