export function relayHtml(iframeSrc: string, cspSource: string): string {
  const nonce = Math.random().toString(36).slice(2); // host-side only; not a security boundary
  const origin = new URL(iframeSrc).origin;
  return `<!DOCTYPE html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none';
  frame-src ${origin} ${cspSource}; script-src 'nonce-${nonce}'; style-src 'unsafe-inline';">
<style>html,body,iframe{margin:0;padding:0;border:0;width:100%;height:100vh;display:block}</style>
</head><body>
<iframe id="tali-preview" src="${iframeSrc}" allow="clipboard-read; clipboard-write"></iframe>
<script nonce="${nonce}">
  const vscode = acquireVsCodeApi();
  const iframe = document.getElementById("tali-preview");
  // iframe (preview) -> host, and host -> iframe (the extension posts to THIS window).
  // A whitelist in both directions: the iframe renders the author's own document, so an
  // unrecognised message must never reach the extension host.
  const UP = ["tali-goto", "tali-page"];      // preview reports: a click, and which page it is
  const DOWN = ["tali-cursor", "tali-navigate"]; // host asks: mark a block, or select a page
  window.addEventListener("message", (e) => {
    const m = e.data;
    if (!m || typeof m !== "object") return;
    if (UP.includes(m.type)) { vscode.postMessage(m); return; }
    if (DOWN.includes(m.type) && iframe.contentWindow) {
      iframe.contentWindow.postMessage(m, "*");
    }
  });
</script>
</body></html>`;
}
