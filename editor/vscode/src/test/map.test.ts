import * as assert from "node:assert";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { test } from "node:test";
import { readSiteMap } from "../map";

/** A stand-in `taliesin` that prints `stdout`, then exits with `code`. */
function fakeBinary(t: { after: (fn: () => void) => void }, stdout: string, code = 0): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tali-map-"));
  const script = path.join(dir, "fake-taliesin.js");
  fs.writeFileSync(
    script,
    `#!/usr/bin/env node
process.stdout.write(${JSON.stringify(stdout)});
process.exit(${code});
`,
    { mode: 0o755 }
  );
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  return script;
}

const MAP = JSON.stringify({
  title: "Guide",
  is_book: true,
  pages: [
    { rel: "index.tmd", url: "index.html" },
    { rel: "using/preview.tmd", url: "using/preview.html" },
  ],
});

test("readSiteMap returns the project's pages", async (t) => {
  const pages = await readSiteMap(fakeBinary(t, MAP), "/repo/guide");
  assert.deepEqual(pages, [
    { rel: "index.tmd", url: "index.html" },
    { rel: "using/preview.tmd", url: "using/preview.html" },
  ]);
});

test("readSiteMap is null when the tool fails, never a throw", async (t) => {
  // Losing the map must cost the author the site-aware preview, not the preview. Three ways
  // it can go wrong, one answer: a non-zero exit, output that is not JSON, and a binary that
  // is not there at all (a wrong `taliesin.path` setting).
  assert.strictEqual(await readSiteMap(fakeBinary(t, MAP, 1), "/repo/guide"), null);
  assert.strictEqual(await readSiteMap(fakeBinary(t, "taliesin: no _site.yml"), "/repo/guide"), null);
  assert.strictEqual(
    await readSiteMap(path.join(os.tmpdir(), "definitely-not-a-binary"), "/repo/guide"),
    null
  );
});
