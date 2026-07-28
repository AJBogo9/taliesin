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
  // `tali-goto` / `tali-cursor` are the wire names of the postMessage protocol the preview
  // client speaks; they are deliberately frozen and are not branding.
  const allowed = /tali-goto|tali-cursor|getElementById\("tali-preview"\)|id="tali-preview"/;
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

/** The subcommand names `main()` will accept, from `const COMMANDS` in the server crate. */
function cargoCommands(): string[] {
  const src = fs.readFileSync(path.join(REPO_ROOT, "crates/server/src/main.rs"), "utf8");
  const m = /const COMMANDS: &\[&str\] = &\[([\s\S]*?)\];/.exec(src);
  assert.ok(m, "crates/server/src/main.rs declares a COMMANDS const");
  return [...m![1].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
}

test("every taliesin subcommand the extension spawns is a real command", () => {
  // The companion talks to the CLI by spawning it with a bare subcommand string. Rename or
  // remove a command in Rust and the extension keeps spawning the old name, failing exactly
  // as silently as the `qmd-fast` default did: `fetchSymbols`/`fetchVocab` swallow the error
  // and simply return no completions. Nothing else ties the two sides together.
  const commands = cargoCommands();
  const spawned = new Set<string>();
  let parsed = 0;
  for (const src of sources) {
    for (const m of src.matchAll(/spawn\(\s*[^,]+,\s*\[\s*"([^"]+)"/g)) {
      spawned.add(m[1]);
      parsed++;
    }
  }
  // A `spawn(` this pattern fails to parse would be skipped in silence, and the gate would
  // pass while the extension shelled out to a command that no longer exists. (The first
  // draft matched only a bare identifier, `spawn(\s*\w+\s*,`, so `spawn(binaryPath(), …)`
  // would have slipped through.) Every call site must be accounted for.
  const callSites = [...allSource.matchAll(/spawn\(/g)].length;
  assert.equal(
    parsed,
    callSites,
    `parsed ${parsed} of ${callSites} spawn(…) call sites; the scan missed one, so this gate cannot be trusted`
  );
  for (const cmd of spawned) {
    assert.ok(
      commands.includes(cmd),
      `the extension spawns \`taliesin ${cmd}\`, which is not in main.rs's COMMANDS (${commands.join(", ")})`
    );
  }
});

test("the language server subcommand is a real command", () => {
  // The subcommand the whole companion now rests on does NOT go through `spawn(`: the
  // LanguageClient launches it from `ServerOptions`. So the gate above, which scans
  // `spawn(` call sites, stopped covering the single most important one the moment the
  // providers moved to LSP. Cover it explicitly, from the same COMMANDS list.
  const m = /args:\s*\[\s*"([^"]+)"\s*\]/.exec(allSource);
  assert.ok(m, "client.ts declares the server args as a literal array");
  assert.ok(
    cargoCommands().includes(m![1]),
    `the language client launches \`taliesin ${m![1]}\`, which is not in main.rs's COMMANDS`
  );
});

test("every command the manifest contributes is registered in the source", () => {
  // A contributed command with no `registerCommand` shows up in the palette and then fails
  // with "command not found" — the manifest is a promise the source has to keep.
  const registered = new Set(
    [...allSource.matchAll(/registerCommand\(\s*"([^"]+)"/g)].map((m) => m[1])
  );
  const contributed: string[] = (manifest.contributes?.commands ?? []).map(
    (c: { command: string }) => c.command
  );
  assert.ok(contributed.length > 0, "the manifest contributes at least one command");
  for (const cmd of contributed) {
    assert.ok(
      registered.has(cmd),
      `\`${cmd}\` is contributed by package.json but never registered in src/ ` +
        `(registered: ${[...registered].join(", ")})`
    );
  }
});

test("every keybinding and menu entry points at a contributed command", () => {
  const contributed = new Set(
    ((manifest.contributes?.commands ?? []) as { command: string }[]).map((c) => c.command)
  );
  const referenced: string[] = [
    ...((manifest.contributes?.keybindings ?? []) as { command: string }[]),
    ...Object.values(
      (manifest.contributes?.menus ?? {}) as Record<string, { command: string }[]>
    ).flat(),
  ].map((e) => e.command);
  assert.ok(referenced.length > 0, "the manifest binds at least one command");
  for (const cmd of referenced) {
    assert.ok(contributed.has(cmd), `\`${cmd}\` is bound but not contributed`);
  }
});

// --- .tmd snippets stay in step with the Rust vocabulary ------------------------------
//
// Snippets insert source text the author then edits, so they never touch the preview and
// cannot violate the single-editing-surface rule. The failure mode is quieter: a snippet
// that offers `.callout-hint` or `#| fig-alt:` after that name was removed from Rust keeps
// inserting text `check` will reject. The vocabulary is Rust-authoritative
// (crates/core/assets/vocab/tali-vocab.json, itself golden-locked by vocab.rs), so every
// name a snippet body mentions is checked against it here.

interface VocabNamed { name: string }
const vocab = JSON.parse(
  fs.readFileSync(path.join(REPO_ROOT, "crates/core/assets/vocab/tali-vocab.json"), "utf8")
);
const names = (list: VocabNamed[]) => list.map((n) => n.name);

const snippetContributions: { language: string; path: string }[] = manifest.contributes?.snippets ?? [];

/** Every snippet body in every contributed file, as one string per snippet. */
function snippetBodies(): { name: string; body: string }[] {
  const out: { name: string; body: string }[] = [];
  for (const c of snippetContributions) {
    const file = JSON.parse(fs.readFileSync(path.join(EXT_ROOT, c.path), "utf8"));
    for (const [name, snip] of Object.entries<any>(file)) {
      out.push({ name, body: Array.isArray(snip.body) ? snip.body.join("\n") : String(snip.body) });
    }
  }
  return out;
}

test("contributes.snippets binds a real snippet file to a declared language", () => {
  assert.ok(snippetContributions.length > 0, "the manifest contributes at least one snippet file");
  const languages = new Set((manifest.contributes?.languages ?? []).map((l: any) => l.id));
  for (const c of snippetContributions) {
    assert.ok(languages.has(c.language), `snippets bind to a declared language, got "${c.language}"`);
    const p = path.join(EXT_ROOT, c.path);
    assert.ok(fs.existsSync(p), `snippet file exists: ${c.path}`);
    const file = JSON.parse(fs.readFileSync(p, "utf8")); // throws on malformed JSON
    assert.ok(Object.keys(file).length > 0, `${c.path} defines at least one snippet`);
    for (const [name, snip] of Object.entries<any>(file)) {
      assert.ok(typeof snip.prefix === "string" && snip.prefix, `${name} has a prefix`);
      assert.ok(snip.body, `${name} has a body`);
    }
  }
});

test("the .vscodeignore does not exclude the snippets from the package", () => {
  const ignore = fs.readFileSync(path.join(EXT_ROOT, ".vscodeignore"), "utf8");
  for (const c of snippetContributions) {
    const dir = c.path.replace(/^\.\//, "").split("/")[0];
    assert.ok(
      !ignore.split("\n").some((l) => l.trim() === `${dir}/` || l.trim() === dir),
      `.vscodeignore must not exclude ${dir}/, or the shipped extension has no snippets`
    );
  }
});

test("every callout kind, div class and theorem a snippet inserts is in the vocabulary", () => {
  const callouts = new Set(names(vocab.calloutKinds));
  const divs = new Set([...names(vocab.divClasses), ...names(vocab.theoremKinds)]);
  for (const { name, body } of snippetBodies()) {
    for (const m of body.matchAll(/:::+\s*\{\.([\w-]+)/g)) {
      const cls = m[1];
      // `.callout-${1|note,tip|}` names its kind with a choice placeholder, not a literal;
      // the reverse-parity test below checks those against the vocabulary's exact order.
      if (body.slice(m.index + m[0].length).startsWith("${")) continue;
      if (cls.startsWith("callout-")) {
        assert.ok(callouts.has(cls.slice("callout-".length)), `${name}: unknown callout \`${cls}\``);
      } else {
        assert.ok(divs.has(cls), `${name}: unknown div class \`${cls}\``);
      }
    }
    // A `${1|a,b|}` choice in a callout opener must offer only real kinds.
    for (const m of body.matchAll(/:::+\s*\{\.callout-\$\{\d+\|([^|]+)\|\}/g)) {
      for (const kind of m[1].split(",")) {
        assert.ok(callouts.has(kind), `${name}: choice offers unknown callout \`${kind}\``);
      }
    }
  }
});

test("every cell option a snippet inserts is in the vocabulary", () => {
  const options = new Set(names(vocab.cellOptions));
  for (const { name, body } of snippetBodies()) {
    for (const m of body.matchAll(/^\s*(?:#\||\/\/\||%%\|)\s*([\w-]+):/gm)) {
      assert.ok(options.has(m[1]), `${name}: unknown cell option \`#| ${m[1]}:\``);
    }
  }
});

test("every cross-reference prefix a snippet inserts is in the vocabulary", () => {
  const prefixes = new Set(vocab.xrefPrefixes.map((p: { prefix: string }) => p.prefix));
  for (const { name, body } of snippetBodies()) {
    for (const m of body.matchAll(/[#@](fig|tbl|sec|eq|lst|thm|lem|cor|prp|def|exm|rem|[a-z]{2,4})-/g)) {
      assert.ok(prefixes.has(m[1]), `${name}: unknown xref prefix \`${m[1]}-\``);
    }
  }
});

test("the callout snippet offers exactly the vocabulary's kinds, in order", () => {
  // The forcing function: add or remove a callout kind in Rust and this fails until the
  // snippet is updated. Without it the snippet rots silently.
  const expected = `\${1|${names(vocab.calloutKinds).join(",")}|}`;
  const found = snippetBodies().filter((s) => s.body.includes("callout-"));
  assert.ok(found.length > 0, "a callout snippet exists");
  assert.ok(
    found.some((s) => s.body.includes(expected)),
    `a callout snippet must offer the vocabulary's kinds in order: ${expected}`
  );
});

// The `_site.yml` schema is shipped as a COPY, because `contributes.yamlValidation` needs a
// file inside the extension and the authoritative one lives in the Rust crate
// (`crates/core/assets/schema/`, itself golden-file-locked against `schema.rs`). A copy with
// nothing watching it is how a stale schema starts telling authors that a key the validator
// rejects is fine.
test("the bundled _site.yml schema matches the one the binary emits", () => {
  const entries = manifest.contributes?.yamlValidation ?? [];
  assert.ok(entries.length > 0, "yamlValidation should be contributed");
  for (const entry of entries) {
    const copy = path.join(EXT_ROOT, entry.url);
    assert.ok(
      fs.existsSync(copy),
      `yamlValidation points at ${entry.url}, which does not exist`
    );
    const source = path.join(
      REPO_ROOT,
      "crates/core/assets/schema",
      path.basename(entry.url)
    );
    assert.ok(
      fs.existsSync(source),
      `${entry.url} has no counterpart in crates/core/assets/schema`
    );
    assert.equal(
      fs.readFileSync(copy, "utf8"),
      fs.readFileSync(source, "utf8"),
      `${entry.url} has drifted from the crate's schema; re-copy it`
    );
  }
});

// Only `_site.yml` is read by the site loader (`config::load` does `root.join("_site.yml")`,
// with no `.yaml` fallback), so validating any other name would tell an author their config
// is valid when Taliesin never opens it.
test("yamlValidation matches only the config filename the loader reads", () => {
  const matches = (manifest.contributes?.yamlValidation ?? []).map(
    (e: { fileMatch: string }) => e.fileMatch
  );
  assert.deepEqual(matches, ["_site.yml"]);
  const loader = fs.readFileSync(
    path.join(REPO_ROOT, "crates/core/src/site/config/mod.rs"),
    "utf8"
  );
  assert.ok(
    loader.includes('root.join("_site.yml")'),
    "the site loader no longer joins `_site.yml`; update the yamlValidation fileMatch"
  );
});

// The schema has to actually reach the packaged extension. `.vscodeignore` was rewritten to
// ship 11 files, and a new directory is exactly the kind of thing that rewrite would exclude
// by accident — leaving a `yamlValidation` entry pointing at a file that is not there.
test("the schema directory is not excluded from the .vsix", () => {
  const ignore = fs.readFileSync(path.join(EXT_ROOT, ".vscodeignore"), "utf8");
  const excluded = ignore
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("#"));
  for (const pattern of excluded) {
    assert.ok(
      !"schema/".startsWith(pattern.replace(/\*+$/, "")) || pattern.startsWith("!"),
      `.vscodeignore pattern \`${pattern}\` would drop the schema/ directory`
    );
  }
});

// A walkthrough is the extension's first-run experience, and every part of it is a string
// that resolves at RUNTIME: a media path VS Code loads, a `command:` link it invokes, a
// completion event it waits for. A typo in any of them shows a broken card to the one user
// who has never seen the tool before, and nothing else in the build would notice.
test("every walkthrough step resolves: media file, commands, completion events", () => {
  const declared = new Set<string>(
    (manifest.contributes?.commands ?? []).map((c: { command: string }) => c.command)
  );
  const walkthroughs = manifest.contributes?.walkthroughs ?? [];
  assert.ok(walkthroughs.length > 0, "a walkthrough should be contributed");
  for (const w of walkthroughs) {
    assert.ok(w.steps?.length > 0, `walkthrough ${w.id} has no steps`);
    for (const step of w.steps) {
      const media = step.media?.markdown ?? step.media?.image;
      assert.ok(media, `step ${step.id} has no media`);
      assert.ok(
        fs.existsSync(path.join(EXT_ROOT, media)),
        `step ${step.id} points at ${media}, which does not exist`
      );
      for (const event of step.completionEvents ?? []) {
        const cmd = event.startsWith("onCommand:") ? event.slice("onCommand:".length) : null;
        if (cmd) {
          assert.ok(
            declared.has(cmd),
            `step ${step.id} completes on \`${cmd}\`, which is not a contributed command`
          );
        }
      }
      // `[label](command:x)` links in the description are buttons on the card.
      for (const m of String(step.description ?? "").matchAll(/\(command:([^)\s]+)\)/g)) {
        assert.ok(
          declared.has(m[1]),
          `step ${step.id} links to \`${m[1]}\`, which is not a contributed command`
        );
      }
    }
  }
});

// The manifest names an icon; the .vsix has to contain it. A marketplace listing with a
// missing icon is the kind of thing only noticed after publishing.
test("the declared icon exists and is a PNG", () => {
  const icon = manifest.icon;
  assert.ok(icon, "package.json should declare a marketplace icon");
  const file = path.join(EXT_ROOT, icon);
  assert.ok(fs.existsSync(file), `icon ${icon} does not exist`);
  // PNG magic. An SVG here is the easy mistake: the marketplace rejects it, and the language
  // icon beside it IS an SVG.
  const head = fs.readFileSync(file).subarray(0, 8);
  assert.deepEqual(
    [...head],
    [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
    `${icon} is not a PNG`
  );
});
