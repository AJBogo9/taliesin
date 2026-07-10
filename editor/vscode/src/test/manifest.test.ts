// No-drift gate between the extension manifest, the extension source, and the Rust binary.
//
// This exists because the companion was silently inert from the Taliesin rename until
// 2026-07-10: `package.json` kept defaulting the binary path to `qmd-fast`, a binary that
// no longer exists, so completions and diagnostics never once ran. Nothing caught it,
// because the e2e suite overrides the setting to `target/debug/taliesin` before every
// command and therefore never exercises the shipped default.
//
// The rules below are the ones that would have caught it: the default binary name must be
// the name cargo actually builds, and every config/command string the source reads must be
// one the manifest declares.
import { test } from "node:test";
import assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";

const EXT_ROOT = path.join(__dirname, "..", "..");
const REPO_ROOT = path.join(EXT_ROOT, "..", "..");
const SRC = path.join(EXT_ROOT, "src");

const manifest = JSON.parse(fs.readFileSync(path.join(EXT_ROOT, "package.json"), "utf8"));

/** Every `.ts` under src/, read as text (recursively; skips e2e's VS Code-host runner). */
function sourceFiles(dir: string = SRC): string[] {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) return e.name === "e2e" ? [] : sourceFiles(p);
    return e.name.endsWith(".ts") && !e.name.endsWith(".test.ts") ? [p] : [];
  });
}

const sources = sourceFiles().map((p) => fs.readFileSync(p, "utf8"));
const allSource = sources.join("\n");

/** The binary name cargo builds, from `[[bin]] name = "…"` in the server crate. */
function cargoBinName(): string {
  const toml = fs.readFileSync(path.join(REPO_ROOT, "crates/server/Cargo.toml"), "utf8");
  const m = /\[\[bin\]\][\s\S]*?name\s*=\s*"([^"]+)"/.exec(toml);
  assert.ok(m, "crates/server/Cargo.toml declares a [[bin]] name");
  return m![1];
}

const configProps: Record<string, { default?: unknown }> =
  manifest.contributes?.configuration?.properties ?? {};
const configKeys = Object.keys(configProps);

test("the default binary path is the binary cargo actually builds", () => {
  // The bug this file exists for: `"default": "qmd-fast"` after the binary became `taliesin`.
  const pathKey = configKeys.find((k) => k.endsWith(".path"));
  assert.ok(pathKey, `a *.path setting is declared (got ${configKeys.join(", ")})`);
  assert.equal(
    configProps[pathKey!].default,
    cargoBinName(),
    `the default for ${pathKey} must name the binary from crates/server/Cargo.toml`
  );
});

test("every config section the source reads is declared in the manifest", () => {
  const sections = [...allSource.matchAll(/getConfiguration\("([^"]+)"\)/g)].map((m) => m[1]);
  assert.ok(sections.length > 0, "the source reads at least one configuration section");
  const declared = new Set(configKeys.map((k) => k.split(".")[0]));
  for (const s of sections) {
    assert.ok(declared.has(s), `getConfiguration("${s}") has no matching contributes.configuration key`);
  }
});

test("every affectsConfiguration string names a declared setting", () => {
  const watched = [...allSource.matchAll(/affectsConfiguration\("([^"]+)"\)/g)].map((m) => m[1]);
  assert.ok(watched.length > 0, "the source watches at least one setting");
  for (const w of watched) {
    assert.ok(configKeys.includes(w), `affectsConfiguration("${w}") is not a declared setting`);
  }
});

test("every command the source registers is declared in the manifest", () => {
  const registered = [...allSource.matchAll(/registerCommand\("([^"]+)"/g)].map((m) => m[1]);
  assert.ok(registered.length > 0, "the source registers at least one command");
  const declared = new Set<string>((manifest.contributes?.commands ?? []).map((c: { command: string }) => c.command));
  for (const r of registered) {
    assert.ok(declared.has(r), `registerCommand("${r}") is not in contributes.commands`);
  }
});

test("every command in a menu contribution is a declared command", () => {
  const declared = new Set<string>((manifest.contributes?.commands ?? []).map((c: { command: string }) => c.command));
  for (const items of Object.values(manifest.contributes?.menus ?? {}) as { command: string }[][]) {
    for (const item of items) {
      assert.ok(declared.has(item.command), `menu references undeclared command ${item.command}`);
    }
  }
});

test("a menu `when` clause only offers extensions the language claims", () => {
  // `resourceExtname == .qmd` outlived the legacy-format clean break: the renderer no
  // longer accepts `.qmd`, so offering Open Preview on one is a dead button.
  const claimed: string[] = (manifest.contributes?.languages ?? []).flatMap(
    (l: { extensions?: string[] }) => l.extensions ?? []
  );
  for (const items of Object.values(manifest.contributes?.menus ?? {}) as { when?: string }[][]) {
    for (const item of items) {
      for (const m of (item.when ?? "").matchAll(/resourceExtname\s*==\s*(\S+)/g)) {
        assert.ok(claimed.includes(m[1]), `menu offers ${m[1]}, which contributes.languages does not claim`);
      }
    }
  }
});

test("vsce packaging rebuilds the bundle first", () => {
  // Without `vscode:prepublish`, `vsce package` silently ships whatever stale `out/` exists.
  assert.ok(
    manifest.scripts?.["vscode:prepublish"],
    "package.json needs a vscode:prepublish script so packaging cannot ship a stale out/"
  );
});

test("no `qmd-fast` branding survives in the manifest or the extension source", () => {
  // `qmd-goto` / `qmd-cursor` are the wire names of the postMessage protocol the preview
  // client speaks; they are deliberately frozen and are not branding.
  const allowed = /qmd-goto|qmd-cursor|getElementById\("qmd"\)|id="qmd"/;
  const offenders: string[] = [];
  for (const p of sourceFiles()) {
    for (const [i, line] of fs.readFileSync(p, "utf8").split("\n").entries()) {
      if (/qmd-fast|qmdFast|qmdfast/i.test(line) && !allowed.test(line)) {
        offenders.push(`${path.relative(REPO_ROOT, p)}:${i + 1}: ${line.trim()}`);
      }
    }
  }
  assert.deepEqual(offenders, [], "stale qmd-fast branding");
  assert.ok(
    !/qmd-fast|qmdFast/i.test(JSON.stringify(manifest)),
    "stale qmd-fast branding in package.json"
  );
});
