// The task provider's pure half, and the problem matcher it ships with.
//
// **Every diagnostic string below is REAL `taliesin build … --check-only` output**, copied from
// a run against `corpus/` and `docs/guide`, not invented. The plan for this feature guessed the
// format as `WARNING[TAL0042]` and was wrong three times over: the severity word is lowercase,
// there is no code bracket at all (the `TAL-*` catalogue went on 2026-08-08), and a message is
// full of colons. A matcher tested only against a hand-written fixture is a matcher tested
// against your assumption of the format.
import { test } from "node:test";
import assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import { taskSpecs, taskLocation } from "../taskspecs";

const EXT_ROOT = path.join(__dirname, "..", "..");
const manifest = JSON.parse(fs.readFileSync(path.join(EXT_ROOT, "package.json"), "utf8"));

interface Pattern {
  regexp: string;
  file: number;
  line?: number;
  severity?: number;
  message: number;
}
interface Matcher {
  name: string;
  severity?: string;
  fileLocation?: string | string[];
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

test("the check task lints without writing, and targets the project root", () => {
  // Two properties, both load-bearing. `--check-only` means the task cannot leave a `_site/`
  // behind (it was the `check` verb until 2026-08-08, which wrote nothing by construction).
  // And the target is the ROOT, never a single file: a single-file lint cannot see cross-page
  // anchors, so it reports every legitimate cross-chapter reference as broken.
  const check = taskSpecs("/r").find((t) => t.name === "check")!;
  assert.deepStrictEqual(check.args, ["build", "/r", "--check-only"]);
});

test("the build --out task writes to _site under the project, not the cwd", () => {
  // MEASURED: `--out` resolves against the process cwd, not the project. The task now runs
  // from the workspace folder (so the problem matcher can resolve paths), so a bare `_site`
  // would follow the cwd and write the built site to the repository root.
  assert.deepStrictEqual(taskSpecs("docs/guide").find((t) => t.name === "build --out")!.args, [
    "build",
    "docs/guide",
    "--out",
    "docs/guide/_site",
  ]);
  // When the folder IS the project, `.` must not leak into the output path as `./_site`.
  assert.deepStrictEqual(taskSpecs(".").find((t) => t.name === "build --out")!.args, [
    "build",
    ".",
    "--out",
    "_site",
  ]);
});

test("every matcher declares a fileLocation VS Code actually accepts", () => {
  // This is the assertion whose absence shipped both matchers dead. `["autoDetected", …]` is
  // not in VS Code's enum, its parser lowercases and compares against `autodetect`, so
  // `FileLocationKind.fromString` returned undefined, `checkProblemMatcherValid` logged
  // "the description doesn't define a file location" and the matcher was never registered.
  // Every task then printed "Problem matcher $taliesin can't be resolved" and nothing at all
  // reached the Problems panel. The regex tests below all passed throughout.
  const allowed = ["absolute", "relative", "autoDetect", "search"];
  for (const name of ["taliesin", "taliesin-unlocated"]) {
    const loc = matcher(name).fileLocation;
    const kind = Array.isArray(loc) ? loc[0] : loc;
    assert.ok(
      kind && allowed.includes(kind),
      `${name}: "${kind}" is not one of VS Code's ${allowed.join(" | ")}; ` +
        "an unrecognised kind makes the whole matcher unregistrable"
    );
    // The base must be a directory the task actually runs in; see `taskLocation`.
    assert.deepStrictEqual(loc, ["relative", "${workspaceFolder}"], `${name} base`);
  }
});

test("a nested project is run from the workspace folder, named relative to it", () => {
  // The whole reason the matcher can work: `${workspaceFolder}` is the only base VS Code can
  // resolve, so the task has to run there and name the project from there. Every project in
  // this repository is nested (`docs/guide`, `corpus/*`, `site`), so this is the normal case,
  // not an edge one.
  const { cwd, target } = taskLocation("/w/docs/guide", ["/w"]);
  assert.strictEqual(cwd, "/w");
  assert.strictEqual(target, "docs/guide");
  assert.deepStrictEqual(taskSpecs(target).find((t) => t.name === "check")!.args, [
    "build",
    "docs/guide",
    "--check-only",
  ]);
});

test("a project that is itself the workspace folder is named `.`, never an empty argument", () => {
  const { cwd, target } = taskLocation("/w", ["/w"]);
  assert.strictEqual(cwd, "/w");
  assert.strictEqual(target, ".", "an empty string would be passed to argv as an empty arg");
});

test("a project outside every workspace folder falls back to running in its own root", () => {
  // Nothing to resolve against, so the honest answer is the root itself. Picking an unrelated
  // folder's path would produce a target that names a different project entirely.
  const { cwd, target } = taskLocation("/elsewhere/book", ["/w", "/other"]);
  assert.strictEqual(cwd, "/elsewhere/book");
  assert.strictEqual(target, "/elsewhere/book");
});

test("the containing folder is chosen by directory boundary, not by string prefix", () => {
  // `/w-old` is a string prefix of neither, but `/w` IS a string prefix of `/w-old/book`.
  // Choosing by prefix would run the task in `/w` and name the project `../w-old/book`.
  const { cwd, target } = taskLocation("/w-old/book", ["/w", "/w-old"]);
  assert.strictEqual(cwd, "/w-old");
  assert.strictEqual(target, "book");
});

test("the problem matcher matches real located lint output for every severity", () => {
  const re = new RegExp(locatedPattern().regexp);
  const p = locatedPattern();
  const cases: [string, string, string, string][] = [
    [
      "index.tmd:103: error: broken cross-reference: @fig-nope (no such figure/section/…)",
      "index.tmd",
      "103",
      "error",
    ],
    [
      "diagnostics/a11y.tmd:13: warning: image is missing alt text (add alt text, or alt=\"\" if decorative)",
      "diagnostics/a11y.tmd",
      "13",
      "warning",
    ],
    [
      "posts/fourier-transform/index.tmd:9: suggestion: 3 bibliography entries are declared but never cited: `@brigham1988fast`",
      "posts/fourier-transform/index.tmd",
      "9",
      "suggestion",
    ],
  ];
  for (const [line, file, lineNo, severity] of cases) {
    const m = re.exec(line);
    assert.ok(m, `the pattern does not match real output: ${line}`);
    assert.strictEqual(m[p.file], file);
    assert.strictEqual(m[p.line!], lineNo);
    assert.strictEqual(m[p.severity!], severity);
  }
});

test("the matcher declares no code group, because the output carries no code", () => {
  // The `TAL-*` catalogue went on 2026-08-08. A pattern still requiring `severity[CODE]:`
  // matches nothing at all, which is silent: the task runs, exits non-zero, and the Problems
  // panel stays empty.
  for (const p of [locatedPattern(), unlocatedPattern()]) {
    assert.strictEqual(
      (p as Pattern & { code?: number }).code,
      undefined,
      `the pattern must not ask for a code group: ${p.regexp}`
    );
    assert.ok(!p.regexp.includes("\\["), `no code bracket in the pattern: ${p.regexp}`);
  }
});

test("the message group keeps a message that itself contains a colon", () => {
  // Real messages are full of colons ("broken cross-reference: @fig-nope"). A pattern that
  // stopped at the first one would truncate almost every diagnostic in the Problems panel.
  const p = locatedPattern();
  const m = new RegExp(p.regexp).exec(
    "index.tmd:103: error: broken cross-reference: @fig-nope (x)"
  );
  assert.strictEqual(m![p.message], "broken cross-reference: @fig-nope (x)");
});

test("the located pattern declares no column, because the lint output has none", () => {
  // `format_human` emits `file:line: severity: message`. A pattern requiring `:col` matches
  // nothing at all, which is the failure this assertion exists to prevent.
  assert.strictEqual(
    (locatedPattern() as Pattern & { column?: number }).column,
    undefined,
    "the output carries no column; a pattern that requires one matches nothing"
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
  assert.strictEqual(re.exec("no problems found"), null);
  assert.strictEqual(re.exec("  built   _site  ·  19 pages  ·  215ms"), null);
});

test("an unlocated diagnostic is matched by its own pattern", () => {
  // The lint also emits `file: severity: message` with no line, for a finding about a file as
  // a whole (a `_site.yml` problem). The located pattern must NOT match it, and a second
  // pattern must, or those findings never reach the Problems panel.
  const real = "_site.yml: error: unknown config key `naav` (did you mean `nav`?)";
  assert.strictEqual(
    new RegExp(locatedPattern().regexp).exec(real),
    null,
    "the located pattern must not swallow an unlocated line"
  );
  const p = unlocatedPattern();
  const m = new RegExp(p.regexp).exec(real);
  assert.ok(m, "no matcher handles an unlocated diagnostic");
  assert.strictEqual(m[p.file], "_site.yml");
  assert.strictEqual(m[p.severity!], "error");
});

test("every task the provider offers is declared by the manifest's task type", () => {
  const defs = manifest.contributes.taskDefinitions ?? [];
  const taliesin = defs.find((d: { type: string }) => d.type === "taliesin");
  assert.ok(taliesin, "package.json must declare a `taliesin` task type");
  const allowed: string[] = taliesin.properties.command.enum;
  // Every spec the extension can build. Until Wave 13 this also carried the `run` task,
  // which was executed straight from the Run Cell lens and never offered in the picker; with
  // `taliesin run` gone, what the provider offers and what the manifest allows are one set.
  const built = taskSpecs("/r").map((s) => s.name);
  for (const name of built) {
    assert.ok(allowed.includes(name), `${name} is built but the task definition does not allow it`);
  }
  assert.deepStrictEqual(
    allowed.slice().sort(),
    built.slice().sort(),
    "the manifest enum and the provider must not drift"
  );
});

