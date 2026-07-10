# Native math syntax highlighting in the `.tmd` editor grammar

- **Date:** 2026-07-10
- **Status:** Approved design, pending implementation plan
- **Area:** `editor/vscode` (the Taliesin `.tmd` VS Code companion grammar)
- **Type:** Editor-only enhancement (no renderer / no Rust change)

## Problem

When you edit a `.tmd` file and write LaTeX math inside `$…$` or `$$…$$`, the
math body is **not** syntax-highlighted by default. The `$` delimiters are
scoped, but the commands, operators, and structure inside stay flat.

The reason is deliberate but limiting: the current math rules in
`editor/vscode/syntaxes/tmd.injection.tmLanguage.json` set
`contentName: meta.embedded.math.tmd` and rely on the `embeddedLanguages`
mapping `meta.embedded.math.tmd → latex` in `editor/vscode/package.json`. That
mapping only produces command-level coloring **if the user has a third-party
LaTeX grammar extension installed**. The original grammar plan wanted a hard
`include: text.tex`, but that was walked back because VS Code does not bundle a
TeX grammar and an unresolvable `include` silently disables the whole rule (see
the comment at `tmd.injection.tmLanguage.json` `math_inline`).

The result is a dependency on an external extension for a first-class feature of
the author's own tool. We want the Taliesin companion to be **self-contained**:
math highlights natively, with no other extension installed.

## Goals

- LaTeX written in `$…$` (inline) and `$$…$$` (display) highlights natively in
  the editor: commands, `\begin{}`/`\end{}` environments, sub/superscripts,
  alignment, comments, escapes, and numbers get distinct scopes.
- Zero external dependency. No user needs a separate LaTeX extension.
- No change to the authoring surface: `$…$` / `$$…$$` stay exactly as they are.
- No change to rendered HTML: KaTeX still renders math identically. This is
  purely how the **source** looks while editing.

## Non-goals

- **No delimiter change.** We keep `$…$` / `$$…$$`. The alternative of unifying
  math onto code-fence syntax (` ```latex ` for display, inline backticks for
  inline) was considered and rejected: it kills inline-math-in-prose (a fence is
  block-level and cannot sit inside a sentence; inline backticks collide with
  real inline code and have no language tag), it breaks portability with the
  Pandoc/Markdown syntax the project deliberately keeps, and crucially it does
  **not** grant free highlighting, because VS Code bundles no TeX grammar either
  way, so a LaTeX grammar must be shipped regardless of delimiter choice. The
  highlighting problem and the delimiter choice are independent.
- **No new authoring surface.** No `{latex}` executable cell, no ` ```latex `
  code-sample support in the editor grammar. (Displaying raw LaTeX *source* as a
  highlighted code sample is a separate, out-of-scope feature; the server side
  already highlights ` ```latex ` fences via syntect, and the editor side can be
  added later if desired.)
- **No vendored full-LaTeX grammar.** We only need to highlight math-mode LaTeX
  (what goes between `$…$`), a small bounded vocabulary, so we write a thin
  grammar we own rather than importing a large third-party `LaTeX.tmLanguage`
  blob (which would add a THIRD_PARTY/attribution entry, a drift-tracking
  burden, and highlight far more than math). This matches the project's
  "thin owned grammar" ethos.

## Design

### Mechanism

Mirror exactly how executable code cells embed a sub-grammar. A `{python}` cell
sets `contentName: meta.embedded.block.python` and then embeds the language via
`include: source.python` (`tmd.tmLanguage.json`, `cell_python`). We do the
identical thing for math, except instead of `include: source.latex` (which does
not exist in VS Code and silently dies), we `include: #math_body`, a
self-contained rule defined in the same grammar file. Both `math_inline` and
`math_display` reference it, so inline and display math highlight the same way.

`contentName: meta.embedded.math.tmd` stays on both rules. TextMate applies the
`contentName` scope to the whole region **and** the inner `patterns` on top, so
the existing region-scope assertions keep passing while the new inner scopes are
added. This coexistence is already proven in this grammar by the code cells,
which carry both `contentName` and inner `patterns`.

### The `#math_body` rule

Added to the `repository` of `tmd.injection.tmLanguage.json`:

```jsonc
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
}
```

And the two math rules gain an inner `patterns` array (everything else about
them, `begin`/`end`/`beginCaptures`/`endCaptures`/`name`/`contentName`, is
unchanged):

```jsonc
"math_inline":  { /* …unchanged… */ "patterns": [ { "include": "#math_body" } ] },
"math_display": { /* …unchanged… */ "patterns": [ { "include": "#math_body" } ] }
```

### JSON escaping note (load-bearing)

To match a single literal backslash, the Oniguruma regex is `\\`, which in a
JSON string is written `\\\\`. So `"\\\\[A-Za-z]+"` matches a `\` followed by
letters (a LaTeX command), and `"(?<!\\\\)%.*$"` is a `%` not preceded by a
backslash. Getting the backslash count wrong is the single most likely bug; the
empirical tokenization test (below) is what proves it correct.

### Scope names

| Construct | Example | Scope |
|---|---|---|
| Comment | `% note` | `comment.line.percentage.tmd.math` |
| Environment delimiter | `\begin` / `\end` | `keyword.control.tmd.math.environment` |
| Environment name | `aligned` in `\begin{aligned}` | `support.class.tmd.math.environment` |
| Environment brace | `{` `}` in `\begin{aligned}` | `punctuation.definition.tmd.math` |
| Command | `\frac`, `\alpha`, `\int` | `keyword.control.tmd.math` |
| Escaped symbol / line break | `\,` `\{` `\%` `\\` | `constant.character.escape.tmd.math` |
| Sub/superscript | `_` `^` | `keyword.operator.tmd.math` |
| Alignment | `&` | `keyword.operator.tmd.math.align` |
| Group braces | `{` `}` | `punctuation.definition.tmd.math.group` |
| Number | `2`, `3.14` | `constant.numeric.tmd.math` |

Plain variables/letters (`x`, `y`) are intentionally left unscoped, which is the
normal, uncluttered look for math and matches how LaTeX math grammars behave.
Anything not matched falls through to plain text and degrades gracefully.

### Ordering rationale

vscode-textmate evaluates the `patterns` list by finding the match that begins
at the earliest position in the remaining line, breaking ties by list order.

- **Comment before commands:** a `%` starts a comment; the `(?<!\\)` guard keeps
  `\%` out of it.
- **`\begin{}`/`\end{}` before the generic command rule:** both would match at
  the same backslash, so the environment rule must be listed first to win the
  tie and capture the environment name.
- **Escaped-symbol rule protects `\%`:** at `\%`, the escaped-symbol rule
  matches at the backslash (position *i*) while the comment rule could only
  match at `%` (position *i+1*); the earlier match wins, so `\%` becomes an
  escape, not a comment.
- **Multiline display math is fine:** inside the `$$…$$` begin/end region the
  inner `match` rules are applied line by line, exactly as code cells highlight
  multi-line Python. Commands never span lines, and the comment rule is
  per-line (`.*$`).

## What does NOT change

- **`package.json`.** The `meta.embedded.math.tmd → latex` `embeddedLanguages`
  mapping stays. It does not colorize on its own; with a LaTeX extension
  installed, VS Code routes the embedded region to that grammar for colorization
  and language-service features (bracket matching, comment toggling). Our new
  inner patterns now own the colorization unconditionally, so the mapping is left
  only for those language-service niceties and is harmless.
- **`injectionSelector`.** Unchanged. It already excludes code cells, fenced
  code, and raw blocks, so `$` inside a `{python}` cell is not treated as math,
  and this remains true because we only add inner patterns to a rule that
  already does not fire there.
- **The renderer / KaTeX / any Rust code.** Nothing. Rendered output is byte-for-byte identical.
- **Backward compatibility of scopes.** The region scopes
  (`meta.embedded.math.tmd`, `markup.math.inline.tmd`, `markup.math.display.tmd`)
  and the `{ #eq-… }` label scope are untouched. One existing assertion still
  needs a mechanical edit: `grammar.test.ts:219` matches the compound needle
  `e^`, which the new `[_^]` operator rule now splits into two tokens (`e` and
  `^`), so its needle must change from `e^` to `e` (see Testing). That is an
  assertion-wording change, not a scope regression.

## Testing

This is editor-only, so the regression net is the offline vscode-textmate
tokenization harness (`editor/vscode/src/test/grammar.test.ts`), not the Rust
corpus. That harness loads the real grammars into a vscode-textmate +
vscode-oniguruma registry and asserts token scopes on fixture lines; it is the
CI-safe gate.

Extend the existing math test (`Phase 2 (injection): inline $…$ and display
$$…$$ math …`) with a richer fixture and inner-scope assertions.

**`%` comments to end-of-line (load-bearing for fixtures).** A `%` starts a
comment that runs to the end of its line (LaTeX-faithful). So a `%` comment must
never share a line with a closing `$`/`$$` or a `\end{…}`, or the comment
swallows the delimiter and the math region leaks past its intended close. Display
fixtures must therefore be multi-line, with any `%` on an interior line only.
(This was verified empirically: a single-line fixture with a trailing `% note`
before `\end{aligned}$$` did exactly this.)

Proposed multi-line display fixture (the harness threads the rule stack across
lines, so tokenize it line by line):

```
$$
\begin{aligned}
\int_0^1 x^2 \,dx &= \frac{1}{3} % note
\end{aligned}
$$
```

Assertions on it:

- `\int` and `\frac` get `keyword.control.tmd.math`.
- `\begin` / `\end` get `keyword.control.tmd.math.environment`, and the
  `aligned` environment name (both occurrences) gets
  `support.class.tmd.math.environment` (use the harness's per-line lookup).
- `^` and `_` get `keyword.operator.tmd.math`; `&` gets
  `keyword.operator.tmd.math.align`.
- `% note` (interior line) gets `comment.line.percentage.tmd.math`.
- `\,` gets `constant.character.escape.tmd.math`; a digit gets
  `constant.numeric.tmd.math`.

**Required edit to the existing inline assertion** (not just an addition). The
current assertion at `grammar.test.ts:219` tests the needle `e^` on
`$e^{i\pi}+1=0$`. The new `[_^]` rule splits `e` and `^` into separate tokens, so
no token's text contains `e^` and that assertion would go red. Change it to
assert the single-token needle `e` carries `markup.math.inline.tmd` /
`meta.embedded.math.tmd` (region scope preserved) and that `\pi` carries
`keyword.control.tmd.math` (new inner scope). This edit ships in the same change
as the grammar edit.

Guard assertions (must stay green after the above edits):

- `\%` inside math is `constant.character.escape.tmd.math`, **not** a comment
  (the load-bearing backslash-guard check).
- `$` inside a `{python}` cell is still not math (existing test unchanged).
- Plain `It costs $5 today` does not open inline math (existing behavior; the
  `math_inline` begin requires a later closing `$` on the line).

Run the harness with `cd editor/vscode && npm test` (it compiles the TS tests,
then runs `node --test out/test/*.test.js`); it must stay green. The author's
manual F5 (Extension Development Host) visual check remains the human
confirmation, per the harness's own note.

## Rollout

- During development, the change is visible after an F5 Extension Development
  Host reload.
- To reach the author's installed editor, the extension must be repackaged
  (`taliesin-companion.vsix`) and reinstalled. The implementation plan will call
  out the packaging/reinstall step explicitly; it is not automatic.

## Risks

- **Mis-escaped backslash in a pattern.** Highest-likelihood bug. Mitigated by
  the empirical tokenization test, which asserts the actual scopes the grammar
  produces.
- **A mis-scoped exotic construct.** Some rare math input could be colored
  oddly. Never fatal: unmatched input stays plain, and the tokenization tests
  plus the manual F5 check catch regressions.

## Out of scope / future

- Highlighting ` ```latex ` / ` ```tex ` fenced blocks that **display** LaTeX
  source in the editor (the server already highlights them). Additive, uses the
  same owned grammar, can be pinned later if the author writes LaTeX-about-LaTeX
  content.
- Full (non-math) LaTeX highlighting (preamble, packages, document structure).
  Out of scope because Taliesin renders math via KaTeX and is not a LaTeX
  compiler.
