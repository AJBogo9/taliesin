# Native Math Highlighting in the `.tmd` Editor Grammar — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make LaTeX written in `$…$` / `$$…$$` inside a `.tmd` file highlight natively in the editor, with no external LaTeX extension.

**Architecture:** Add a small, self-contained `#math_body` rule to the Taliesin injection grammar and reference it from the two math rules, exactly as executable code cells embed a sub-grammar (`contentName` + `include`), but pointing at a local rule instead of the nonexistent `source.latex`. Editor-only: no renderer, KaTeX, or Rust change.

**Tech Stack:** TextMate grammar (`.tmLanguage.json`, Oniguruma regex) for the VS Code companion at `editor/vscode`; the offline `vscode-textmate` + `vscode-oniguruma` tokenization harness (`node --test`) as the regression gate.

## Global Constraints

Copied verbatim from the spec (`docs/superpowers/specs/2026-07-10-tmd-math-highlighting-design.md`). Every task implicitly includes these.

- Keep `$…$` / `$$…$$` as the math delimiters. No delimiter change, no `{latex}` cell, no ` ```latex ` code-sample support.
- No external dependency: ship the highlighting inside the extension (no third-party LaTeX grammar, no vendored `LaTeX.tmLanguage` blob).
- No renderer / KaTeX / Rust change. Rendered HTML must be byte-for-byte identical; this is editor-only.
- The only file that defines the grammar is `editor/vscode/syntaxes/tmd.injection.tmLanguage.json`. The regression gate is `editor/vscode/src/test/grammar.test.ts`.
- Preserve the existing region scopes (`meta.embedded.math.tmd`, `markup.math.inline.tmd`, `markup.math.display.tmd`) and the `{ #eq-… }` label scope.
- JSON backslash rule (load-bearing): a regex matching one literal backslash is `\\`, written `\\\\` in the JSON string.
- `%` starts a comment that runs to end-of-line (LaTeX-faithful). A `%` must never share a line with a closing `$`/`$$` or a `\end{…}` in a test fixture, or it swallows them.

---

## File Structure

- **`editor/vscode/syntaxes/tmd.injection.tmLanguage.json`** (modify): add the `math_body` repository rule; add `"patterns": [{ "include": "#math_body" }]` to `math_display` and `math_inline`. This is the entire feature.
- **`editor/vscode/src/test/grammar.test.ts`** (modify): update one existing assertion whose needle the change splits, and add one new test asserting the inner math scopes. This is the regression gate.
- **`editor/vscode/taliesin-companion.vsix`** (rebuild, not committed): repackaged so the change reaches the author's installed editor. The `.vsix` is gitignored (a local build artifact).

No new files. No renderer/Rust files.

---

## Task 1: Add native math-body highlighting to the injection grammar

**Files:**
- Modify: `editor/vscode/syntaxes/tmd.injection.tmLanguage.json`
- Test: `editor/vscode/src/test/grammar.test.ts`

**Interfaces:**
- Consumes: the existing `math_inline` / `math_display` begin/end rules (unchanged) and the harness helpers already in `grammar.test.ts`: `tokenizeTmd(src): Promise<Tok[]>` (multi-line, threads the rule stack), `hasScope(toks, needle, scopePrefix, line?): boolean` (true if any token whose text includes `needle` has a scope starting with `scopePrefix`).
- Produces: the `#math_body` repository rule and these token scopes inside math regions — `keyword.control.tmd.math` (commands), `keyword.control.tmd.math.environment` (`\begin`/`\end`), `support.class.tmd.math.environment` (env name), `keyword.operator.tmd.math` (`_`/`^`), `keyword.operator.tmd.math.align` (`&`), `constant.character.escape.tmd.math` (`\,`, `\%`, `\\`), `comment.line.percentage.tmd.math` (`%…`), `punctuation.definition.tmd.math.group` (braces), `constant.numeric.tmd.math` (numbers). The existing region scopes coexist (they layer above these).

- [ ] **Step 1: Write the tests (one new test; one needle fix on an existing test)**

In `editor/vscode/src/test/grammar.test.ts`, FIRST fix the existing inline assertion. The new `[_^]` rule (added in Step 3) will split `e^` into two tokens, so no token's text contains `e^` and the current needle would go red. Change only line 219.

Replace this exact line (currently line 219):

```ts
  assert.ok(hasScope(inl, "e^", "meta.embedded.math.tmd") || hasScope(inl, "e^", "markup.math.inline.tmd"), "inline $…$ math scoped");
```

with (needle `e^` → `e`; a single-token needle that carries the region scope both before and after the change):

```ts
  assert.ok(hasScope(inl, "e", "markup.math.inline.tmd"), "inline $…$ math region scoped");
```

THEN add this new test immediately after the closing `});` of the `"Phase 2 (injection): inline $…$ and display $$…$$ math get a math scope"` test (after the current line 223):

```ts
test("Phase 2 (injection): math-body inner tokens are highlighted natively (no external LaTeX grammar)", async () => {
  // Multi-line so the `% note` comment stays on an interior line (a `%` runs to EOL, so it must
  // not share a line with the closing `$$` or `\end{…}`). Empirically the form that tokenizes
  // cleanly: `$$\begin{aligned}` opens, comment on the middle line, `\end{aligned}$$` closes.
  const src =
    "$$\\begin{aligned}\n" +
    "\\int_0^1 x^2 \\,dx &= \\frac{1}{3} % note\n" +
    "\\end{aligned}$$\n";
  const toks = await tokenizeTmd(src);

  // Commands.
  assert.ok(hasScope(toks, "\\int", "keyword.control.tmd.math"), "\\int is a math command");
  assert.ok(hasScope(toks, "\\frac", "keyword.control.tmd.math"), "\\frac is a math command");
  // Environments.
  assert.ok(hasScope(toks, "\\begin", "keyword.control.tmd.math.environment"), "\\begin is an environment keyword");
  assert.ok(hasScope(toks, "\\end", "keyword.control.tmd.math.environment"), "\\end is an environment keyword");
  assert.ok(hasScope(toks, "aligned", "support.class.tmd.math.environment"), "the environment name is scoped");
  // Operators.
  assert.ok(hasScope(toks, "^", "keyword.operator.tmd.math"), "^ is a math operator");
  assert.ok(hasScope(toks, "_", "keyword.operator.tmd.math"), "_ is a math operator");
  assert.ok(hasScope(toks, "&", "keyword.operator.tmd.math.align"), "& is an alignment operator");
  // Escape, comment, number.
  assert.ok(hasScope(toks, "\\,", "constant.character.escape.tmd.math"), "\\, is an escaped symbol");
  assert.ok(hasScope(toks, "% note", "comment.line.percentage.tmd.math"), "% note is a comment");
  assert.ok(hasScope(toks, "3", "constant.numeric.tmd.math"), "a digit is numeric");
  // The region scope still coexists with the new inner scopes.
  assert.ok(hasScope(toks, "\\int", "meta.embedded.math.tmd"), "region contentName preserved under inner scopes");
});
```

- [ ] **Step 2: Run the tests and verify the NEW test FAILS (grammar not yet changed)**

```bash
cd editor/vscode && node scripts/ensure-vscode.cjs && npm test
```

Expected: compiles, then `node --test` runs. The new test **fails** — e.g. `\int is a math command` throws, because without `#math_body` the math body is one flat token scoped only `markup.math.display.tmd` / `meta.embedded.math.tmd`, so no token starts with `keyword.control.tmd.math`. All other tests (including the edited line-219 assertion, which still passes) are green. (`ensure-vscode.cjs` is a no-op if `.vscode-test` is already present; it only downloads base grammars.)

- [ ] **Step 3: Implement the grammar change**

Edit `editor/vscode/syntaxes/tmd.injection.tmLanguage.json`.

3a. In the `math_display` rule, add a `patterns` key after `contentName`. Replace:

```json
      "name": "markup.math.display.tmd",
      "contentName": "meta.embedded.math.tmd"
    },
    "math_inline": {
```

with:

```json
      "name": "markup.math.display.tmd",
      "contentName": "meta.embedded.math.tmd",
      "patterns": [{ "include": "#math_body" }]
    },
    "math_inline": {
```

3b. In the `math_inline` rule, add the same `patterns` key after its `contentName`, AND insert the new `math_body` rule immediately after `math_inline` (before `shortcode`). Replace:

```json
      "name": "markup.math.inline.tmd",
      "contentName": "meta.embedded.math.tmd"
    },
    "shortcode": {
```

with:

```json
      "name": "markup.math.inline.tmd",
      "contentName": "meta.embedded.math.tmd",
      "patterns": [{ "include": "#math_body" }]
    },
    "math_body": {
      "comment": "Self-contained math-mode LaTeX highlighting for $…$ / $$…$$ bodies. No external grammar: VS Code bundles no TeX grammar, so we own a thin set tuned to math mode. Order matters (vscode-textmate takes the leftmost match, ties broken by list order): comment first; \\begin{}/\\end{} before the generic command rule so the environment name is captured; the escaped-symbol rule (matching at the backslash) protects \\% from being read as a comment.",
      "patterns": [
        { "match": "(?<!\\\\)%.*$", "name": "comment.line.percentage.tmd.math" },
        {
          "match": "(\\\\(?:begin|end))(\\{)([^}]*)(\\})",
          "captures": {
            "1": { "name": "keyword.control.tmd.math.environment" },
            "2": { "name": "punctuation.definition.tmd.math" },
            "3": { "name": "support.class.tmd.math.environment" },
            "4": { "name": "punctuation.definition.tmd.math" }
          }
        },
        { "match": "\\\\[A-Za-z]+", "name": "keyword.control.tmd.math" },
        { "match": "\\\\[^A-Za-z]", "name": "constant.character.escape.tmd.math" },
        { "match": "[_^]", "name": "keyword.operator.tmd.math" },
        { "match": "&", "name": "keyword.operator.tmd.math.align" },
        { "match": "[{}]", "name": "punctuation.definition.tmd.math.group" },
        { "match": "[0-9]+(?:\\.[0-9]+)?", "name": "constant.numeric.tmd.math" }
      ]
    },
    "shortcode": {
```

- [ ] **Step 4: Run the tests and verify ALL pass**

```bash
cd editor/vscode && npm test
```

Expected: every test passes, including the new `math-body inner tokens` test and the edited inline assertion. If `npm test` errors before running (e.g. a JSON parse error from `parseRawGrammar`), the grammar JSON is malformed — re-check the backslash counts against the Global Constraints JSON rule and that both `patterns` keys have a preceding comma after `contentName`.

- [ ] **Step 5: Commit**

```bash
git add editor/vscode/syntaxes/tmd.injection.tmLanguage.json editor/vscode/src/test/grammar.test.ts
git commit -m "feat(editor): native math highlighting in the .tmd grammar

Add a self-contained #math_body rule to the injection grammar so \$…\$ /
\$\$…\$\$ bodies highlight (commands, environments, operators, escapes, comments,
numbers) with no external LaTeX extension. Mirrors how code cells embed a
sub-grammar (contentName + include). Extend grammar.test.ts with inner-scope
assertions; update the inline region-scope needle the new [_^] rule now splits."
```

---

## Task 2: Repackage and reinstall the companion extension

The grammar is an `include_str`-free JSON asset loaded at runtime and bundled into the `.vsix`, but the committed `taliesin-companion.vsix` is stale (built before this change). Repackage so the change reaches the author's installed editor. The `.vsix` is gitignored (a local build artifact), so this task produces no commit.

**Files:**
- Rebuild: `editor/vscode/taliesin-companion.vsix` (gitignored)

- [ ] **Step 1: Package the extension**

```bash
cd editor/vscode && npx --yes @vscode/vsce package --out taliesin-companion.vsix
```

Expected: `@vscode/vsce` is fetched on demand and writes `taliesin-companion.vsix`. It runs `vscode:prepublish` (`npm run build`, the esbuild bundle) first. If vsce aborts on a missing `repository` field, re-run with `--allow-missing-repository`; if it warns about activation or license, those are warnings, not failures.

- [ ] **Step 2: Verify the new grammar is actually bundled in the `.vsix`**

```bash
cd editor/vscode && unzip -p taliesin-companion.vsix extension/syntaxes/tmd.injection.tmLanguage.json | grep -c "math_body"
```

Expected: prints a number `>= 1` (the `math_body` rule is present in the shipped grammar). A `0` means a stale package was produced — re-run Step 1.

- [ ] **Step 3: Author's manual gate — reinstall and F5 visual check**

This is the author's action (installing into their real VS Code), not automatable here:

```bash
code --install-extension editor/vscode/taliesin-companion.vsix --force
```

Then reload VS Code and open a `.tmd` file containing `$e^{i\pi}$` and a `$$\begin{aligned}…\end{aligned}$$` block; confirm commands, the environment name, `^`/`_`, and a `% comment` are colored, with no other LaTeX extension installed. Alternatively, during development, press **F5** in the `editor/vscode` folder to launch the Extension Development Host and check there (per the README's F5 checklist).

---

## Self-Review

**1. Spec coverage.**
- Mechanism (`contentName` + local `#math_body` include) → Task 1 Step 3. ✓
- The full `#math_body` rule + both `patterns` includes → Task 1 Step 3 (verbatim from the spec). ✓
- Scope-names table → asserted token-by-token in Task 1 Step 1's new test. ✓
- "What does NOT change" (region scopes, `injectionSelector`, `package.json`, renderer) → nothing in the plan touches them; coexistence asserted (`region contentName preserved`). ✓
- Testing (multi-line fixture; the required `grammar.test.ts:219` needle edit; run with `npm test`) → Task 1 Steps 1–4. ✓
- Rollout (repackage + reinstall + F5) → Task 2. ✓
- Non-goals (no delimiter change, no `{latex}` cell, no vendored grammar) → nothing in the plan adds them; captured in Global Constraints. ✓

**2. Placeholder scan.** No TBD/TODO; every code and command step shows exact content. ✓

**3. Type/name consistency.** Scope names in the new test match the `#math_body` JSON exactly (`keyword.control.tmd.math`, `…​.environment`, `support.class.tmd.math.environment`, `keyword.operator.tmd.math[.align]`, `constant.character.escape.tmd.math`, `comment.line.percentage.tmd.math`, `constant.numeric.tmd.math`). The env-brace scope `punctuation.definition.tmd.math` and the group-brace scope `punctuation.definition.tmd.math.group` are deliberately distinct (matches the spec's Scope-names table). Helper signatures (`tokenizeTmd`, `hasScope`) match `grammar.test.ts`. ✓

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-10-tmd-math-highlighting.md`.
