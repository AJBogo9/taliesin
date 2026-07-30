import * as assert from "node:assert";
import { test } from "node:test";
import * as vm from "node:vm";
import { relayHtml } from "../webview";

/**
 * Run the relay's own script with stubbed hosts and hand back what each side received.
 *
 * The relay is executed rather than grepped because it is the one hop in click-to-source with
 * no end-to-end coverage at all: the e2e harness stops at the panel, so a message type added
 * to the client and the host but forgotten here fails silently, in the browser, at runtime.
 */
function runRelay(messages: unknown[]): { toHost: unknown[]; toIframe: unknown[] } {
  const html = relayHtml("http://127.0.0.1:4388/", "vscode-webview://x");
  const script = /<script nonce="[^"]*">([\s\S]*?)<\/script>/.exec(html);
  assert.ok(script, "the relay carries an inline script");

  const toHost: unknown[] = [];
  const toIframe: unknown[] = [];
  const handlers: ((e: { data: unknown }) => void)[] = [];
  const context = {
    acquireVsCodeApi: () => ({ postMessage: (m: unknown) => toHost.push(m) }),
    document: {
      getElementById: () => ({
        contentWindow: { postMessage: (m: unknown) => toIframe.push(m) },
      }),
    },
    window: {
      addEventListener: (type: string, fn: (e: { data: unknown }) => void) => {
        if (type === "message") handlers.push(fn);
      },
    },
  };
  vm.runInNewContext(script![1], context);
  assert.strictEqual(handlers.length, 1, "the relay listens for messages");
  for (const m of messages) handlers[0]({ data: m });
  return { toHost, toIframe };
}

test("the relay carries click-to-source and page reports up to the host", () => {
  const goto = { type: "tali-goto", source_file: "a.tmd", sourcepos: "3:1" };
  // `tali-page` is how the host learns which page the webview is showing after it follows a
  // cross-page link, which is what anchors the cursor key (item 150 §4).
  const page = { type: "tali-page", doc_path: "/g/using/preview.tmd", base_dir: "/g/using" };
  const { toHost, toIframe } = runRelay([goto, page]);
  assert.deepEqual(toHost, [goto, page]);
  assert.deepEqual(toIframe, []);
});

test("the relay carries cursor and navigation down to the page", () => {
  const cursor = { type: "tali-cursor", file: null, line: 12, reveal: false };
  // The host cannot set `iframe.contentWindow.location` across origins, so selecting a page
  // has to be a message the client acts on.
  const navigate = { type: "tali-navigate", url: "using/preview.html" };
  const { toHost, toIframe } = runRelay([cursor, navigate]);
  assert.deepEqual(toIframe, [cursor, navigate]);
  assert.deepEqual(toHost, []);
});

test("the relay drops anything it does not recognise", () => {
  // The iframe is a page the author's own document can put scripts on, so the relay is a
  // whitelist: an unknown message must not be handed to the extension host.
  const { toHost, toIframe } = runRelay([
    { type: "tali-unknown" },
    "a string",
    null,
    { noType: true },
  ]);
  assert.deepEqual(toHost, []);
  assert.deepEqual(toIframe, []);
});
