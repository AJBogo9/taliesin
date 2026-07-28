import { execFileSync } from "node:child_process";
import * as assert from "node:assert";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { test } from "node:test";
import { PreviewServer } from "../server";

/** How many live processes have `needle` in their command line. */
function aliveMatching(needle: string): number {
  try {
    const out = execFileSync("ps", ["-eo", "args="], { encoding: "utf8" });
    return out.split("\n").filter((l) => l.includes(needle)).length;
  } catch {
    return 0; // no `ps`
  }
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

// A binary can spawn perfectly well and still never serve HTTP — a wrong path that happens
// to be executable, or a machine slow enough to miss the window. `start` rejects in that
// case, and before this nothing killed the child it had already spawned: the caller only
// gets an Error, so there is no handle left to dispose. That is a second, quieter version of
// the leak that left 17 preview servers running.
test("a preview that spawns but never answers is killed, not abandoned", async (t) => {
  if (process.platform === "win32") {
    t.skip("POSIX `ps` only");
    return;
  }
  const tag = `tali-fake-preview-${process.pid}`;
  const script = path.join(os.tmpdir(), `${tag}.js`);
  // Executable, ignores its arguments, and stays alive: exactly a server that never answers.
  fs.writeFileSync(script, "#!/usr/bin/env node\nsetInterval(() => {}, 1000);\n", {
    mode: 0o755,
  });
  t.after(() => {
    fs.rmSync(script, { force: true });
    // If the assertion below fails, the child is by definition still running — and a live
    // child handle pins Node's event loop, so the suite would HANG instead of reporting the
    // failure. Reap it here so a regression fails loudly and quickly.
    try {
      const out = execFileSync("ps", ["-eo", "pid=,args="], { encoding: "utf8" });
      for (const line of out.split("\n")) {
        const m = /^\s*(\d+)\s+(.*)$/.exec(line);
        if (m && m[2].includes(tag)) process.kill(Number(m[1]), "SIGKILL");
      }
    } catch {
      /* nothing to reap */
    }
  });

  await assert.rejects(
    PreviewServer.start(script, path.join(os.tmpdir(), "doc.tmd"), 400),
    /did not answer/,
    "a server that never answers must reject"
  );

  let alive = 1;
  for (let i = 0; i < 10 && alive > 0; i++) {
    await sleep(200);
    alive = aliveMatching(tag);
  }
  assert.equal(alive, 0, "the spawned process must not outlive the failed start");
});
