# Bug: "check produced unexpected output" on line 1 of every `.tmd` file

**Reported:** 2026-07-13 (opening any `.tmd` in VS Code)
**Status:** Root cause confirmed. Source already fixed (`b40ec0e`); the **packaged + installed** companion is stale. Remediation = rebuild the vsix and reinstall.
**Severity:** Cosmetic-but-noisy. Real diagnostics still work; this drowns them under a false line-1 error on every file.

## Symptom

A red squiggle + Problems-panel entry appears on **line 1** (the front-matter YAML) of **every** `.tmd` file you open, reading:

```
check produced unexpected output: {
  "diagnostics": [],
  "environment": [
    {
      "kernel_pkg": "ipykernel",
      "kernel_pkg_ok": true,
      "lang": "python",
      "path": "/home/…/…
```

(The blob is the CLI's JSON, truncated at 200 chars by the parser.)

## Root cause: version skew between the CLI and the installed companion

The message is emitted by the VS Code companion's check-output parser when it cannot
recognize the JSON that `taliesin check --format json` printed. It is **not** the CLI
misbehaving and **not** a problem with your document. The CLI's output format moved
forward; the *installed* extension's parser did not.

**What the CLI emits now** — [`crates/server/src/check.rs:304-309`](crates/server/src/check.rs#L304-L309),
`format_json`, emits a pretty-printed object:

```json
{ "diagnostics": [...], "environment": [...] }
```

The `environment` block (resolved interpreter + kernel-pkg probe) landed in
`1fa3e9e feat(check): informational Environment section` on **2026-07-12 13:57**.
Before that commit the CLI emitted a **bare array** `[{file,line,message}, …]`.

**What the installed extension expects** — the runtime bundle
`~/.vscode/extensions/taliesin.taliesin-companion-0.1.0/out/extension.js`
(built **2026-07-11 08:45**) contains the **old** parser: it only accepts
`Array.isArray(value)` and otherwise falls through to
`check produced unexpected output`. It has **zero** `.diagnostics` object handling.

So: new CLI emits an object → old parser sees a non-array, non-`{error}` value →
returns `{ kind: "error" }` → the wiring maps a parser error to a **whole-line
diagnostic on line 0** (see `toDiagnostics` in `editor/vscode/src/check.ts:64-75`,
`line0: 0`), which renders on line 1 (the front-matter) of whatever file is open.
That is why it shows up "in the beginning configuration YAML in every file."

The `environment` array being populated vs. empty is incidental — a `{python}` cell
makes it non-empty (hence the `ipykernel` probe in the pasted output), but even
`{"diagnostics":[],"environment":[]}` trips the old parser, because it's an object,
not an array.

## Why the source looks fixed but the bug persists

The parser was already fixed in the tree:

- `b40ec0e fix(companion): parse check --format json object shape` (**2026-07-12 14:29**)
  taught `editor/vscode/src/check.ts` to accept all three shapes: the `{diagnostics,environment}`
  object, the legacy bare array, and the `{error}` envelope.
- The working-tree compiled `editor/vscode/out/check.js` (Jul 12 14:28) has the fix.

But the artifacts a user actually runs were **never rebuilt from the fixed source**:

| Artifact | Date | Parser |
|---|---|---|
| `crates/server/src/check.rs` (CLI, on PATH) | current | emits object shape ✅ |
| `editor/vscode/src/check.ts` (source) | Jul 12 14:29 (`b40ec0e`) | fixed ✅ |
| `editor/vscode/out/*.js` (working-tree build) | Jul 12 14:28 | fixed ✅ |
| `editor/vscode/taliesin-companion.vsix` (packaged, untracked) | **Jul 11 08:10** | **old** ❌ |
| `~/.vscode/extensions/taliesin.taliesin-companion-0.1.0/` (installed) | **Jul 11 08:45** | **old** ❌ |

The packaged `.vsix` and the installed copy both predate the fix. This is a
**deployment/packaging gap**, not a code defect. It matches the standing note that
the remaining companion work was "repackage/republish the vsix."

## Reproduction

1. Ensure the `taliesin` on PATH is recent enough to emit the `environment` block
   (`>= 1fa3e9e`). Verify:
   ```sh
   taliesin check --format json corpus/demo-book/index.tmd
   # -> { "diagnostics": [...], "environment": [...] }   (object, not a bare array)
   ```
2. With `taliesin.taliesin-companion-0.1.0` (the Jul 11 build) installed, open any
   `.tmd` file in VS Code.
3. A line-1 diagnostic appears: `check produced unexpected output: {…`.

## Fix / remediation

The source is already correct, so no code change is required — only rebuild + reinstall:

```sh
cd editor/vscode
npm run build                 # recompile extension.js from the fixed check.ts
npx vsce package              # produce a fresh taliesin-companion-0.1.0.vsix
code --install-extension taliesin-companion.vsix --force
# then reload the VS Code window
```

After reload, the object shape parses cleanly and the false line-1 error disappears
(real diagnostics keep working).

### Follow-ups worth considering

- **Bump the extension version** (still `0.1.0`) when repackaging, so a stale install
  is visible at a glance instead of silently shadowing a fixed build.
- **Harden against the *next* format bump.** The parser already tolerates old + new
  shapes, but a future CLI change could re-trip it. Options: have the companion invoke
  a pinned/bundled CLI, or make the CLI carry a small `"schema"`/version field the
  parser can branch on. (Design call — the current "accept both shapes" approach is a
  fine minimal fix.)
- The in-repo `editor/vscode/taliesin-companion.vsix` is an **untracked build artifact**
  (`git ls-files` shows it unversioned). If it's meant to be the distributed build,
  it should be regenerated as part of release; otherwise it's a stale trap sitting in
  the tree.

## Evidence (commands run 2026-07-13)

- Installed runtime bundle is pre-fix:
  `grep -c "\.diagnostics" ~/.vscode/extensions/taliesin.taliesin-companion-0.1.0/out/extension.js` → **0**;
  only `Array.isArray(value)` branches + the `produced unexpected output` string. Dated **2026-07-11 08:45**.
- Packaged vsix is pre-fix: unzipped `out/check.js` → **0** `diagnostics` matches; only the `Array.isArray` branch. Vsix dated **2026-07-11 08:10**.
- CLI emits the object shape today: `taliesin check --format json corpus/demo-book/index.tmd` → `{ "diagnostics": [], "environment": [] }`.
- Timeline: `1fa3e9e` (CLI environment block) 2026-07-12 13:57 → `b40ec0e` (parser fix) 2026-07-12 14:29; both **after** the Jul 11 vsix/install.
