import { test } from "node:test";
import assert from "node:assert/strict";
import { onRequest } from "./_middleware.js";

function ctx({ auth, password } = {}) {
  const headers = new Headers();
  if (auth) headers.set("Authorization", auth);
  return {
    request: new Request("https://example.pages.dev/", { headers }),
    env: password === undefined ? {} : { PASSWORD: password },
    next: async () => new Response("SECRET", { status: 200 }),
  };
}
const basic = (user, pass) =>
  "Basic " + Buffer.from(`${user}:${pass}`).toString("base64");

test("missing PASSWORD secret fails closed (503)", async () => {
  const res = await onRequest(ctx({ auth: basic("x", "hunter2") }));
  assert.equal(res.status, 503);
});

test("no Authorization header returns 401 with WWW-Authenticate", async () => {
  const res = await onRequest(ctx({ password: "hunter2" }));
  assert.equal(res.status, 401);
  assert.match(res.headers.get("WWW-Authenticate"), /Basic realm="draft"/);
});

test("wrong passcode returns 401", async () => {
  const res = await onRequest(ctx({ password: "hunter2", auth: basic("x", "nope") }));
  assert.equal(res.status, 401);
});

test("correct passcode calls next() and serves content", async () => {
  const res = await onRequest(
    ctx({ password: "hunter2", auth: basic("anyuser", "hunter2") }),
  );
  assert.equal(res.status, 200);
  assert.equal(await res.text(), "SECRET");
});
