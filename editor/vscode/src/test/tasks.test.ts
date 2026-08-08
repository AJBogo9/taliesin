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
import { taskSpecs, taskLocation, runSpec, runOutcome } from "../taskspecs";

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

test("the check task targets the project root, not a single file", () => {
  // `check <file.tmd>` is a different, narrower thing: it cannot see cross-page anchors, so
  // it reports every legitimate cross-chapter reference as broken.
  const check = taskSpecs("/r").find((t) => t.name === "check")!;
  assert.deepStrictEqual(check.args, ["check", "/r"]);
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
    "check",
    "docs/guide",
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
    "_site.yml: warning[TAL-CONFIG-KEY]: unknown config key `naav` (did you mean `nav`?)";
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
  // Every spec the extension can BUILD, not only the ones `provideTasks` lists: a run task
  // is executed straight from the Run Cell lens and never offered in the task picker, but
  // its definition is checked against this same enum.
  const built = [...taskSpecs("/r").map((s) => s.name), runSpec("/r/a.tmd", 1).name];
  for (const name of built) {
    assert.ok(allowed.includes(name), `${name} is built but the task definition does not allow it`);
  }
  assert.deepStrictEqual(
    allowed.slice().sort(),
    built.slice().sort(),
    "the manifest enum and the provider must not drift"
  );
});

test("the run task points the CLI at one cell, by source line", () => {
  // The same `--line` the Run Cell lens has always sent: the editor knows the cursor, and an
  // ordinal computed here would be a second copy of "which fences count".
  assert.deepStrictEqual(runSpec("/w/post.tmd", 12).args, [
    "run",
    "/w/post.tmd",
    "--line",
    "12",
  ]);
});

test("the run task can ask for the whole document", () => {
  assert.deepStrictEqual(runSpec("/w/post.tmd", "all").args, ["run", "/w/post.tmd", "--all"]);
});

test("`run` is not one of the tasks offered for a project", () => {
  // The task picker offers project-wide work. A run needs a file and a cursor line, so an
  // entry there would either run the wrong thing or nothing at all.
  assert.ok(
    !taskSpecs("/r").some((s) => s.name === "run"),
    "run must not appear in the project task list"
  );
});

test("a quick, successful run says nothing at all", () => {
  // The terminal already showed it. A toast per keystroke-to-run cycle is the opposite of
  // the fast iteration the run loop exists for.
  assert.strictEqual(runOutcome(0, 500).kind, "silent");
  assert.strictEqual(runOutcome(0, 9_000).kind, "silent");
});

test("a long run that succeeded announces itself, which is the whole point", () => {
  // CHI 2020's verbatim want for long-running cells: "when the process is done, it
  // automatically creates a notification".
  const out = runOutcome(0, 30_000);
  assert.strictEqual(out.kind, "info");
  assert.match(out.message, /30\.0 s/);
});

test("a long run reports minutes rather than a hundred seconds", () => {
  assert.match(runOutcome(0, 65_000).message, /1 min 5 s/);
});

test("a failed run always reports, however fast it failed", () => {
  const out = runOutcome(1, 200);
  assert.strictEqual(out.kind, "error");
  assert.match(out.message, /exit 1/);
});

test("a run the author stopped is not reported as a failure", () => {
  // MEASURED: `onDidEndTaskProcess` carries `exitCode: undefined` when the process was
  // terminated rather than exited. Hitting the terminal's stop button is not an error, and
  // an error toast for it would train the author to ignore the real ones.
  assert.strictEqual(runOutcome(undefined, 4_000).kind, "silent");
});
