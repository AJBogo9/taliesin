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
    PreviewServer.start(script, path.join(os.tmpdir(), "doc.tmd"), os.tmpdir(), 400),
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

// Item 150 §1. A site preview serves a PROJECT while the author is editing a file inside it,
// so the thing to serve and the directory to serve it from are no longer the same path with
// `dirname` applied. `start` used to derive the cwd from the target, which is only right for
// a single file; a book chapter must spawn `preview <root>` while the cwd stays the root too,
// and a caller that gets those two confused would serve the wrong tree.
test("start serves the target it is handed, from the cwd it is handed", async (t) => {
  if (process.platform === "win32") {
    t.skip("POSIX shebang script");
    return;
  }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tali-start-"));
  const record = path.join(dir, "spawn.json");
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), "tali-cwd-"));
  const script = path.join(dir, "fake-taliesin.js");
  // Records how it was invoked, then answers on the port it was given — a preview server
  // reduced to exactly the two facts this test is about.
  fs.writeFileSync(
    script,
    `#!/usr/bin/env node
const fs = require("node:fs"), http = require("node:http");
fs.writeFileSync(${JSON.stringify(record)},
  JSON.stringify({ argv: process.argv.slice(2), cwd: process.cwd() }));
http.createServer((_, res) => res.end("ok")).listen(Number(process.argv[4]), "127.0.0.1");
`,
    { mode: 0o755 }
  );

  const server = await PreviewServer.start(script, dir, cwd, 5000);
  t.after(() => {
    server.dispose();
    fs.rmSync(dir, { recursive: true, force: true });
    fs.rmSync(cwd, { recursive: true, force: true });
  });

  const spawned = JSON.parse(fs.readFileSync(record, "utf8"));
  assert.deepEqual(spawned.argv, ["preview", dir, String(server.port)]);
  assert.equal(fs.realpathSync(spawned.cwd), fs.realpathSync(cwd), "cwd is the caller's, not dirname(target)");
});
