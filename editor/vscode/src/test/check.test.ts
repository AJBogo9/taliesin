import { test } from "node:test";
import assert from "node:assert";
import { parseCheckJson, toDiagnostics, suggestionSpan, fixSpan } from "../check";

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

test("toDiagnostics renders the {error} envelope as one document-level error", () => {
  // A check failure (unreadable file, render panic) is a real error, not a yellow warning.
  const shapes = toDiagnostics({ kind: "error", error: "cannot read x" }, 3);
  assert.deepEqual(shapes, [{ line0: 0, message: "cannot read x", severity: "error" }]);
});

test("toDiagnostics on empty diags is empty", () => {
  assert.deepEqual(toDiagnostics({ kind: "diags", diags: [] }, 3), []);
});

// --- rich diagnostic fields (severity / code / docs_url / suggestion) ---

test("parseCheckJson reads severity, code, docs_url and suggestion from the object shape", () => {
  const out = parseCheckJson(
    JSON.stringify({
      diagnostics: [
        {
          file: "a.tmd",
          line: 3,
          message: "unknown front-matter key `treme` (did you mean `theme`?)",
          severity: "warning",
          code: "TAL-FM-KEY",
          docs_url: "https://example/DIAGNOSTICS.md#tal-fm-key",
          suggestion: { replacement: "theme" },
        },
      ],
    })
  );
  assert.equal(out.kind, "diags");
  const d = (out as any).diags[0];
  assert.equal(d.severity, "warning");
  assert.equal(d.code, "TAL-FM-KEY");
  // The snake_case `docs_url` wire field is exposed as camelCase `docsUrl`.
  assert.equal(d.docsUrl, "https://example/DIAGNOSTICS.md#tal-fm-key");
  assert.deepEqual(d.suggestion, { replacement: "theme" });
});

test("parseCheckJson leaves the rich fields undefined for a legacy diagnostic", () => {
  // An older `taliesin` emits only {file,line,message}; the extra fields must stay absent
  // (not be invented), so the whole-warning fallback still applies.
  const out = parseCheckJson('[{"file":"a.tmd","line":1,"message":"m"}]');
  const d = (out as any).diags[0];
  assert.equal(d.severity, undefined);
  assert.equal(d.code, undefined);
  assert.equal(d.docsUrl, undefined);
  assert.equal(d.suggestion, undefined);
});

test("parseCheckJson ignores a malformed suggestion (no string replacement)", () => {
  const out = parseCheckJson(
    JSON.stringify({ diagnostics: [{ file: "a.tmd", line: 1, message: "m", suggestion: { replacement: 7 } }] })
  );
  assert.equal((out as any).diags[0].suggestion, undefined);
});

test("toDiagnostics carries severity, code, docsUrl and suggestion through", () => {
  const shapes = toDiagnostics(
    {
      kind: "diags",
      diags: [
        {
          file: "a.tmd",
          line: 2,
          message: "m",
          severity: "error",
          code: "TAL-XREF-UNDEF",
          docsUrl: "https://x#tal-xref-undef",
          suggestion: { replacement: "@fig-y" },
        },
      ],
    },
    10
  );
  assert.deepEqual(shapes, [
    {
      line0: 1,
      message: "m",
      severity: "error",
      code: "TAL-XREF-UNDEF",
      docsUrl: "https://x#tal-xref-undef",
      suggestion: { replacement: "@fig-y" },
    },
  ]);
});

test("toDiagnostics omits absent rich fields (no undefined-valued keys)", () => {
  // A legacy diagnostic must round-trip to the bare {line0,message} shape so nothing
  // downstream sees a spurious `severity: undefined`.
  const shapes = toDiagnostics(
    { kind: "diags", diags: [{ file: "a.tmd", line: 1, message: "m" }] },
    3
  );
  assert.deepEqual(shapes, [{ line0: 0, message: "m" }]);
});

// --- suggestionSpan: the token a "did you mean `X`" quick-fix should replace ---

test("suggestionSpan finds a one-edit typo token and returns its span", () => {
  assert.deepEqual(suggestionSpan("treme: dark", "theme"), { start: 0, end: 5 });
});

test("suggestionSpan finds an @-prefixed xref typo", () => {
  const line = "See @fig-reslts for details.";
  const span = suggestionSpan(line, "@fig-results");
  assert.ok(span);
  assert.equal(line.slice(span!.start, span!.end), "@fig-reslts");
});

test("suggestionSpan returns null when no token is close to the replacement", () => {
  assert.equal(suggestionSpan("wholly unrelated prose", "theme"), null);
});

test("suggestionSpan returns null when the closest token already equals the replacement", () => {
  assert.equal(suggestionSpan("theme: dark", "theme"), null);
});

test("suggestionSpan returns null when two tokens tie as the closest match", () => {
  // Both `treme` and `thyme` are one edit from `theme`: ambiguous, so offer no fix.
  assert.equal(suggestionSpan("treme thyme", "theme"), null);
});

test("parseCheckJson reads col/end_col when present", () => {
  const out = parseCheckJson(
    JSON.stringify({
      diagnostics: [
        { file: "a.tmd", line: 3, message: "unknown key `treme`", col: 1, end_col: 6 },
      ],
    })
  );
  assert.equal(out.kind, "diags");
  assert.equal((out as any).diags[0].col, 1);
  assert.equal((out as any).diags[0].endCol, 6);
});

test("parseCheckJson ignores a non-numeric col", () => {
  const out = parseCheckJson(
    JSON.stringify({ diagnostics: [{ file: "a.tmd", line: 3, message: "x", col: "1" }] })
  );
  assert.equal((out as any).diags[0].col, undefined);
});

test("toDiagnostics carries the column span through", () => {
  const out = parseCheckJson(
    JSON.stringify({ diagnostics: [{ file: "a.tmd", line: 3, message: "x", col: 1, end_col: 6 }] })
  );
  const shapes = toDiagnostics(out, 10);
  assert.equal(shapes[0].col, 1);
  assert.equal(shapes[0].endCol, 6);
});

test("fixSpan prefers an exact span over the edit-distance guess", () => {
  assert.deepEqual(fixSpan({ replacement: "theme", span: { start: 0, end: 5 } }, "treme: dark"), {
    start: 0,
    end: 5,
  });
});

test("fixSpan falls back to suggestionSpan when no span is present", () => {
  assert.deepEqual(
    fixSpan({ replacement: "theme" }, "treme: dark"),
    suggestionSpan("treme: dark", "theme")
  );
});
