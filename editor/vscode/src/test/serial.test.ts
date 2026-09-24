import * as assert from "node:assert";
import { test } from "node:test";
import { serialize } from "../serial";

// Lead client.ts:52 (audit 2026-09-24, leads cluster 9). `start` stops the running server,
// awaits the new one and, on failure, clears the module's client. Two calls in flight at
// once (a restart command during the activation start, or a `taliesin.path` edit) let the
// older one's failure clear the client the newer one had just set, and let both spawn a
// server. Chained, a call starts only once every earlier one has settled.
test("a serialized call starts only after the one before it settles", async () => {
  const events: string[] = [];
  let release!: () => void;
  const gate = new Promise<void>((r) => (release = r));
  const start = serialize(async (name: string) => {
    events.push(`start ${name}`);
    if (name === "first") await gate;
    events.push(`end ${name}`);
  });
  const first = start("first");
  const second = start("second");
  await new Promise((r) => setTimeout(r, 20));
  assert.deepEqual(events, ["start first"], "the second waits for the first");
  release();
  await Promise.all([first, second]);
  assert.deepEqual(events, ["start first", "end first", "start second", "end second"]);
});

test("a failed call does not stop the next one", async () => {
  const start = serialize(async (fail: boolean) => {
    if (fail) throw new Error("spawn failed");
  });
  await assert.rejects(start(true), /spawn failed/);
  await start(false);
});
