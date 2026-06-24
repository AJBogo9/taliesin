import { spawn, ChildProcess } from "node:child_process";
import * as path from "node:path";
import { freePort, waitForHttp } from "./ports";

export class PreviewServer {
  private constructor(
    readonly port: number,
    private readonly child: ChildProcess
  ) {}

  static async start(binary: string, file: string): Promise<PreviewServer> {
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
    const ready = waitForHttp(port, 8000).then((ok) => {
      if (!ok) throw new Error(`qmd-fast preview did not answer on ${port} within 8s`);
      return new PreviewServer(port, child);
    });
    return Promise.race([ready, spawnError]);
  }

  dispose(): void {
    try {
      this.child.kill("SIGTERM");
    } catch {
      /* already gone */
    }
  }
}
