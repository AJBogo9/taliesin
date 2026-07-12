import { test } from "node:test";
import assert from "node:assert";
import { parseCheckJson, toDiagnostics } from "../check";

test("parseCheckJson reads the diagnostics array (legacy bare-array shape)", () => {
  const out = parseCheckJson('[{"file":"a.tmd","line":3,"message":"unknown key `titel`"}]');
  assert.deepEqual(out, {
    kind: "diags",
    diags: [{ file: "a.tmd", line: 3, message: "unknown key `titel`" }],
  });
});

test("parseCheckJson reads .diagnostics from the { diagnostics, environment } object", () => {
  const out = parseCheckJson(
    JSON.stringify({
      diagnostics: [{ file: "a.tmd", line: 3, message: "unknown key `titel`" }],
      environment: [
        {
          lang: "python",
          path: "/x/.venv/bin/python",
          provenance: ".venv",
          runs: true,
          kernel_pkg: "ipykernel",
          kernel_pkg_ok: true,
          version: "Python 3.12.1",
        },
      ],
    })
  );
  // The informational `environment` block is ignored: only diagnostics drive squiggles.
  assert.deepEqual(out, {
    kind: "diags",
    diags: [{ file: "a.tmd", line: 3, message: "unknown key `titel`" }],
  });
});

test("parseCheckJson reads an empty .diagnostics array in the object shape", () => {
  const out = parseCheckJson(JSON.stringify({ diagnostics: [], environment: [] }));
  assert.deepEqual(out, { kind: "diags", diags: [] });
});

test("parseCheckJson tolerates a null line", () => {
  const out = parseCheckJson('[{"file":"_site.yml","line":null,"message":"needs a name"}]');
  assert.equal(out.kind, "diags");
  assert.equal((out as any).diags[0].line, null);
});

test("parseCheckJson surfaces the {error} envelope", () => {
  const out = parseCheckJson('{"error":"cannot read missing.tmd"}');
  assert.deepEqual(out, { kind: "error", error: "cannot read missing.tmd" });
});

test("parseCheckJson treats malformed output as an error, not a throw", () => {
  const out = parseCheckJson("not json at all");
  assert.equal(out.kind, "error");
});

test("parseCheckJson treats an empty string as no diagnostics", () => {
  assert.deepEqual(parseCheckJson(""), { kind: "diags", diags: [] });
});

test("toDiagnostics maps a 1-based line to a 0-based line", () => {
  const shapes = toDiagnostics(
    { kind: "diags", diags: [{ file: "a.tmd", line: 3, message: "m" }] },
    10
  );
  assert.deepEqual(shapes, [{ line0: 2, message: "m" }]);
});

test("toDiagnostics clamps a null line and an over-long line to the document", () => {
  const shapes = toDiagnostics(
    {
      kind: "diags",
      diags: [
        { file: "a.tmd", line: null, message: "doc-level" },
        { file: "a.tmd", line: 999, message: "past EOF" },
      ],
    },
    5
  );
  assert.deepEqual(shapes, [
    { line0: 0, message: "doc-level" },
    { line0: 4, message: "past EOF" }, // clamped to last line (lineCount - 1)
  ]);
});

test("toDiagnostics renders the {error} envelope as one document-level diagnostic", () => {
  const shapes = toDiagnostics({ kind: "error", error: "cannot read x" }, 3);
  assert.deepEqual(shapes, [{ line0: 0, message: "cannot read x" }]);
});

test("toDiagnostics on empty diags is empty", () => {
  assert.deepEqual(toDiagnostics({ kind: "diags", diags: [] }, 3), []);
});
