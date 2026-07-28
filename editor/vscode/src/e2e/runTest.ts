import { execFileSync } from "node:child_process";
import * as path from "node:path";
import { runTests } from "@vscode/test-electron";

const REPO_ROOT = path.resolve(__dirname, "../../../../");

/**
 * PIDs of `taliesin preview` servers started from THIS repo's build.
 *
 * Snapshotting before and after is what makes the leak check safe to run on a developer's
 * machine: a preview the author started by hand, or one belonging to a parallel session, was
 * already there and is not this run's fault.
 */
function previewPids(): Set<string> {
  if (process.platform === "win32") return new Set();
  const binary = path.join(REPO_ROOT, "target/debug/taliesin");
  const pids = new Set<string>();
  let listing: string;
  try {
    listing = execFileSync("ps", ["-eo", "pid=,args="], { encoding: "utf8" });
  } catch {
    return pids; // no `ps`: skip the check rather than fail the run
  }
  for (const line of listing.split("\n")) {
    const m = /^\s*(\d+)\s+(.*)$/.exec(line);
    if (m && m[2].startsWith(`${binary} preview `)) pids.add(m[1]);
  }
  return pids;
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/**
 * Fail if VS Code exited leaving a preview server behind.
 *
 * This lives in the RUNNER, not the Mocha suite, because it is the only vantage point that
 * outlives the Extension Host — the bug it guards is precisely "the host went away without
 * cleaning up". It is not hypothetical: the suite opens a preview every run, and before the
 * server was registered for disposal these accumulated until they exhausted
 * `fs.inotify.max_user_instances` and VS Code itself could no longer start.
 */
async function assertNoLeakedPreviews(before: Set<string>): Promise<void> {
  let leaked: string[] = [];
  // SIGTERM → exit is not instantaneous; give it a moment before calling it a leak.
  for (let i = 0; i < 10; i++) {
    leaked = [...previewPids()].filter((p) => !before.has(p));
    if (leaked.length === 0) return;
    await sleep(500);
  }
  // Reap them anyway: leaving them would poison the next run too, and a test that degrades
  // the machine it found the bug on is a bad citizen.
  for (const pid of leaked) {
    try {
      process.kill(Number(pid), "SIGTERM");
    } catch {
      /* already gone */
    }
  }
  throw new Error(
    `VS Code exited leaving ${leaked.length} \`taliesin preview\` server(s) running ` +
      `(pids ${leaked.join(", ")}). The companion must dispose the preview server when the ` +
      `extension deactivates, not only when the webview panel is closed.`
  );
}

// Downloads a throwaway VS Code, launches it headless with this extension loaded, and
// runs the Mocha suite (out/e2e/suite) inside the real Extension Host. Verifies the VS
// Code API wiring that the node:test unit suite can't reach.
async function main() {
  // Some sandboxes/agents export ELECTRON_RUN_AS_NODE=1 globally, which makes the
  // downloaded `code` binary run as plain Node and reject every GUI arg. Clear it so the
  // spawned VS Code launches as the real Electron app. (Unset on normal machines → no-op.)
  delete process.env.ELECTRON_RUN_AS_NODE;
  const extensionDevelopmentPath = path.resolve(__dirname, "../../");
  const extensionTestsPath = path.resolve(__dirname, "./suite/index");
  const previewsBefore = previewPids();
  try {
    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath,
      launchArgs: ["--no-sandbox", "--disable-gpu"],
    });
    await assertNoLeakedPreviews(previewsBefore);
  } catch (err) {
    console.error("e2e run failed:", err);
    process.exit(1);
  }
}

main();
