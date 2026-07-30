// The task provider's pure half, and the problem matcher it ships with.
//
// **Every diagnostic string below is REAL `taliesin check` output**, copied from a run against
// `corpus/` and `docs/guide`, not invented. The plan for this feature guessed the format as
// `WARNING[TAL0042]` and was wrong twice over: the severity word is lowercase, and the codes
// are `TAL-XREF-UNDEF`-shaped. A matcher tested only against a hand-written fixture is a
// matcher tested against your assumption of the format.
import { test } from "node:test";
import assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import { taskSpecs } from "../taskspecs";

const EXT_ROOT = path.join(__dirname, "..", "..");
const manifest = JSON.parse(fs.readFileSync(path.join(EXT_ROOT, "package.json"), "utf8"));

interface Pattern {
  regexp: string;
  file: number;
  line?: number;
  severity?: number;
  code?: number;
  message: number;
}
interface Matcher {
  name: string;
  severity?: string;
  pattern: Pattern | Pattern[];
}

function matcher(name: string): Matcher {
  const found = (manifest.contributes.problemMatchers ?? []).find(
    (m: Matcher) => m.name === name
  );
  assert.ok(found, `package.json must contribute a \`${name}\` problem matcher`);
  return found;
}

/** The located pattern, which is the first of the matcher's two. */
function locatedPattern(): Pattern {
  const p = matcher("taliesin").pattern;
  return Array.isArray(p) ? p[0] : p;
}

/** The unlocated pattern (a diagnostic with no line, e.g. one about `_site.yml` itself). */
function unlocatedPattern(): Pattern {
  const p = matcher("taliesin-unlocated").pattern;
  return Array.isArray(p) ? p[0] : p;
}

test("three tasks are offered for a project root", () => {
  const names = taskSpecs("/r").map((t) => t.name);
  assert.deepStrictEqual(names, ["check", "build", "build --out"]);
});

test("the check task targets the project root, not a single file", () => {
  // `check <file.tmd>` is a different, narrower thing: it cannot see cross-page anchors, so
  // it reports every legitimate cross-chapter reference as broken.
  const check = taskSpecs("/r").find((t) => t.name === "check")!;
  assert.deepStrictEqual(check.args, ["check", "/r"]);
});

test("the build --out task writes to _site under the project, not the cwd", () => {
  const out = taskSpecs("/r").find((t) => t.name === "build --out")!;
  assert.deepStrictEqual(out.args, ["build", "/r", "--out", "_site"]);
});

test("the problem matcher matches real located check output for every severity", () => {
  const re = new RegExp(locatedPattern().regexp);
  const p = locatedPattern();
  const cases: [string, string, string, string, string][] = [
    [
      "index.tmd:103: error[TAL-XREF-UNDEF]: broken cross-reference: @fig-nope (no such figure/section/…)",
      "index.tmd",
      "103",
      "error",
      "TAL-XREF-UNDEF",
    ],
    [
      "diagnostics/a11y.tmd:13: warning[TAL-A11Y-ALT]: image is missing alt text (add alt text, or alt=\"\" if decorative)",
      "diagnostics/a11y.tmd",
      "13",
      "warning",
      "TAL-A11Y-ALT",
    ],
    [
      "posts/fourier-transform/index.tmd:9: suggestion[TAL-CITE-UNUSED]: 3 bibliography entries are declared but never cited: `@brigham1988fast`",
      "posts/fourier-transform/index.tmd",
      "9",
      "suggestion",
      "TAL-CITE-UNUSED",
    ],
  ];
  for (const [line, file, lineNo, severity, code] of cases) {
    const m = re.exec(line);
    assert.ok(m, `the pattern does not match real output: ${line}`);
    assert.strictEqual(m[p.file], file);
    assert.strictEqual(m[p.line!], lineNo);
    assert.strictEqual(m[p.severity!], severity);
    assert.strictEqual(m[p.code!], code);
  }
});

test("the message group keeps a message that itself contains a colon", () => {
  // Real messages are full of colons ("broken cross-reference: @fig-nope"). A pattern that
  // stopped at the first one would truncate almost every diagnostic in the Problems panel.
  const p = locatedPattern();
  const m = new RegExp(p.regexp).exec(
    "index.tmd:103: error[TAL-XREF-UNDEF]: broken cross-reference: @fig-nope (x)"
  );
  assert.strictEqual(m![p.message], "broken cross-reference: @fig-nope (x)");
});

test("the located pattern declares no column, because check output has none", () => {
  // `format_human` emits `file:line: severity[CODE]: message`. A pattern requiring `:col`
  // matches nothing at all, which is the failure this assertion exists to prevent.
  assert.strictEqual(
    (locatedPattern() as Pattern & { column?: number }).column,
    undefined,
    "check output carries no column; a pattern that requires one matches nothing"
  );
});

test("the matcher defaults unknown severities to info, so `suggestion` is not an error", () => {
  // VS Code understands error/warning/info. `suggestion` is Taliesin's own third severity and
  // is advice, never a defect, so it must not land in the Problems panel painted as an error.
  assert.strictEqual(matcher("taliesin").severity, "info");
});

test("the problem matcher does not match a line that only looks like a diagnostic", () => {
  const re = new RegExp(locatedPattern().regexp);
  assert.strictEqual(re.exec("2 problems (2 errors)"), null);
  assert.strictEqual(re.exec("  at some/file.rs:12: something"), null);
  assert.strictEqual(
    re.exec("For more information about a diagnostic, try `taliesin check --explain <CODE>`."),
    null
  );
});

test("an unlocated diagnostic is matched by its own pattern", () => {
  // `check` also emits `file: severity[CODE]: message` with no line, for a finding about a
  // file as a whole (a `_site.yml` problem). The located pattern must NOT match it, and a
  // second pattern must, or those findings never reach the Problems panel.
  const real =
    "_site.yml: warning[TAL-SHORTCODE]: deck.tmd: declares `format: deck` but is a loose page in the site";
  assert.strictEqual(
    new RegExp(locatedPattern().regexp).exec(real),
    null,
    "the located pattern must not swallow an unlocated line"
  );
  const p = unlocatedPattern();
  const m = new RegExp(p.regexp).exec(real);
  assert.ok(m, "no matcher handles an unlocated diagnostic");
  assert.strictEqual(m[p.file], "_site.yml");
  assert.strictEqual(m[p.severity!], "warning");
});

test("every task the provider offers is declared by the manifest's task type", () => {
  const defs = manifest.contributes.taskDefinitions ?? [];
  const taliesin = defs.find((d: { type: string }) => d.type === "taliesin");
  assert.ok(taliesin, "package.json must declare a `taliesin` task type");
  const allowed: string[] = taliesin.properties.command.enum;
  for (const spec of taskSpecs("/r")) {
    assert.ok(
      allowed.includes(spec.name),
      `${spec.name} is offered but the task definition does not allow it`
    );
  }
  assert.deepStrictEqual(
    allowed.slice().sort(),
    taskSpecs("/r")
      .map((s) => s.name)
      .sort(),
    "the manifest enum and the provider must not drift"
  );
});
