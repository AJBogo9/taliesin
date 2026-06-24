import { test } from "node:test";
import assert from "node:assert";
import * as http from "node:http";
import { freePort, waitForHttp } from "../ports";

test("freePort returns a usable port", async () => {
  const p = await freePort();
  assert.ok(p > 0 && p < 65536);
});

test("waitForHttp resolves true once a server answers", async () => {
  const p = await freePort();
  const srv = http.createServer((_req, res) => res.end("ok"));
  await new Promise<void>((r) => srv.listen(p, "127.0.0.1", r));
  try {
    assert.equal(await waitForHttp(p, 2000), true);
  } finally {
    srv.close();
  }
});

test("waitForHttp resolves false when nothing answers", async () => {
  const p = await freePort(); // free, nothing listening
  assert.equal(await waitForHttp(p, 600), false);
});
