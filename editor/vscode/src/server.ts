import { spawn, ChildProcess } from "node:child_process";
import * as path from "node:path";
import { freePort, waitForHttp } from "./ports";

export class PreviewServer {
  private disposed = false;

  private constructor(
    readonly port: number,
    private readonly child: ChildProcess
  ) {}

  static async start(
    binary: string,
    file: string,
    readyTimeoutMs = 8000
  ): Promise<PreviewServer> {
    const port = await freePort();
    const child = spawn(binary, ["preview", file, String(port)], {
      cwd: path.dirname(file),
      stdio: "ignore",
    });
    const spawnError = new Promise<never>((_, reject) =>
      child.on("error", (e) =>
        reject(new Error(`failed to launch \`${binary}\`: ${e.message}`))
      )
    );
    const ready = waitForHttp(port, readyTimeoutMs).then((ok) => {
      if (!ok) {
        throw new Error(
          `taliesin preview did not answer on ${port} within ${readyTimeoutMs}ms`
        );
      }
      return new PreviewServer(port, child);
    });
    try {
      return await Promise.race([ready, spawnError]);
    } catch (e) {
      // A binary can spawn fine and still never serve. The caller gets an Error rather than
      // a PreviewServer, so nothing else holds a reference to dispose — without this, the
      // child runs until the machine reboots, and the live handle also pins the event loop.
      try {
        child.kill("SIGTERM");
      } catch {
        /* never started */
      }
      throw e;
    }
  }

  /** Idempotent: the panel and the extension's own disposal may both reach this. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    try {
      this.child.kill("SIGTERM");
    } catch {
      /* already gone */
    }
  }
}
