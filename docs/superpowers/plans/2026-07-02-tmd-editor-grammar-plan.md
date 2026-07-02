# Own the `.tmd` editor syntax-highlighting grammar (VS Code companion)

Status: plan / ready to execute (research + judged design done; no code written yet)
Date: 2026-07-02
Origin: the single open item under the "Taliesin rename" backlog section
(`notes/backlog.md`) — "the concrete answer to *rename so I fully control the highlighting*".
Grounded by a 4-reader / 3-approach-judged scoping workflow; raw output at
`tasks/wtnlhoj39.output`.

---

## 0. TL;DR for the executing session

Ship a **thin owned `taliesin` language** in `editor/vscode/`: a real `contributes.languages`
+ `contributes.grammars` pair bound to **`.tmd` only** (see §6 decision 1 — `.qmd` is deliberately
NOT claimed), whose TextMate grammar `include`s VS Code's built-in **MIT** `text.html.markdown`
grammar for all CommonMark and adds **only the ~8 Taliesin deltas** (braced `{python}`/`{r}`/`{js}`
cells, `#|`/`//|`/`%%|` cell options, `---` YAML front matter, `:::` divs + `{.class #id key=val}`
attrs, `$…$`/`$$…$$` math, `{{< shortcodes >}}`, `@fig-`/`[@cite]` refs, deck `. . .` pause), plus an
`embeddedLanguages` map so cells/front-matter/math delegate to the real sub-grammars.

**Quick start (do this first, prove registration before authoring any rule):**
In `editor/vscode/`, add `contributes.languages` (id `taliesin`, extensions `[".tmd"]`) +
`contributes.grammars` (scopeName `text.tmd.markdown`) to `package.json`, and create
`syntaxes/tmd.tmLanguage.json` containing only:
```json
{ "scopeName": "text.tmd.markdown", "name": "Taliesin", "patterns": [ { "include": "text.html.markdown" } ] }
```
Then F5 (Extension Development Host) → open `corpus/native-tmd.tmd` → confirm the status bar reads
**"Taliesin"** and inherited Markdown coloring works. Everything else builds on that.

This work is **editor-only**: a grammar only colorizes the buffer. It never touches the render
pipeline, never writes to source, so the read-only-preview + `data-block-id`/`data-sourcepos` +
`qmd-goto`/`qmd-cursor` invariants are untouched. Low-risk, additive `contributes` surface.

---

## 1. What "own the highlighting" means (disambiguation)

There are **two** highlighting systems; only one is the goal.

- **SERVER-side rendered code highlighting — ALREADY OWNED, out of scope.** `syntect` emits scope
  classes prefixed `tali-hl-` (`crates/core/src/highlight.rs:23`, renamed from `qhl-` in Phase 3d).
  This colors code *in the rendered HTML*. Do **not** touch it.
- **EDITOR-side highlighting — THE OPEN GOAL.** A TextMate grammar (`.tmLanguage.json`) + a
  `contributes.languages`/`contributes.grammars` association so the `.tmd` **editing surface in VS
  Code** is colorized by a Taliesin-owned grammar, instead of `.tmd` falling back to plaintext and
  `.qmd` being grabbed by Quarto's extension.

## 2. Current state (grounded facts to trust)

- **The companion contributes NO language/grammar today.** `editor/vscode/package.json:12` `contributes`
  has only `commands`, `menus`, `configuration`. No `languages`, no `grammars`, no `syntaxes/`, no
  `language-configuration.json`. Greenfield.
- **The companion is entirely pre-rename and `.qmd`-only.** Name `qmd-fast-companion`, publisher
  `qmd-fast`, command `qmdFast.openPreview`, config `qmdFast.path`, and three hardcoded `.qmd` gates:
  `src/extension.ts:14` (open guard), `src/extension.ts:63` (reverse-sync filter), `package.json:18`
  (menu `when: resourceExtname == .qmd`). Phases 2/3 of the rename never reached `editor/vscode/`
  (its last commit `2836454` predates them). **`.tmd` cannot even open the preview today.**
- **Two different extension scopes — keep them distinct:** the *renderer* still accepts both
  (`crates/core/src/ext.rs:16` `ACCEPTED_SOURCE_EXTS = ["tmd", "qmd"]`, `.qmd` deprecated-but-accepted),
  but the *editor grammar/language* claims **`.tmd` only** (decision 1). So: `contributes.languages`
  `extensions` = `[".tmd"]`; the preview **gates** (Phase 3) still accept `.qmd` so existing `.qmd`
  files can be previewed — they just aren't given the `taliesin` language/highlighting. A `.qmd` file
  keeps whatever language another extension (Quarto) or the plaintext fallback assigns it.
- **Build/test:** esbuild bundles `src/` → `out/extension.js`; `tsc -p . --noEmit` is clean today.
  Two tiers: `npm test` (node:test unit for ports/paths) and `npm run test:e2e`
  (`@vscode/test-electron`, downloads a throwaway VS Code into gitignored `.vscode-test/`). The
  `ELECTRON_RUN_AS_NODE` global-export gotcha is already handled in `src/e2e/runTest.ts:11,18`
  (`delete` it + `--no-sandbox`).
- **Packaging gotcha:** `.vscodeignore:1` excludes `src/` from the VSIX, so the grammar JSON **must
  live under a non-`src/` dir** (`editor/vscode/syntaxes/`). Test *fixtures* under `src/` are fine
  (they're inputs, not shipped).
- **No CI for the extension.** `.github/workflows/ci.yml` has zero `editor/vscode` references;
  `notes/backlog.md:202` tracks the missing `editor/vscode/**` job.
- **Reference grammars are inspectable in-repo** (in the downloaded `.vscode-test/…/extensions/`):
  MIT `markdown-basics` (full-grammar + ~55 `embeddedLanguages` template) and MIT `markdown-math`
  (the injection template). Quarto's own extension (`~/.vscode/extensions/quarto.quarto-1.134.0/`)
  is a **full standalone `quarto` language** (scopeName `text.html.quarto`) — and it is **AGPL-3.0**.

## 3. Decision: thin owned `taliesin` language

Three approaches were judged; the recommendation grafts the ownership of a full grammar onto the
minimal authoring of an injection:

| Approach | Verdict |
|---|---|
| **A — inject onto built-in `markdown`** | Cheapest, but a pure injection binds to the `markdown` language id: it **leaks our tokens into every `.md` file** (TextMate has no filename scoping) and the status bar still says "Markdown". Fails the "own it" intent. |
| **B — full standalone grammar** | Total control, but you must re-own ~150 base-markdown + ~55 fenced rules (that's the 165KB Quarto file) for **zero coverage gain** over `include`. Maintenance/regression surface. |
| **C — fork Quarto's grammar** | **Disqualified: AGPL-3.0** contaminates a ***REMOVED***, and it re-imports the `text.html.quarto` vocabulary the rename is shedding. Salvage the *technique*, never the file. |
| **✅ Thin owned language (A⊕B)** | Define a real `taliesin` language owning **`.tmd` only**; its small grammar `include`s `text.html.markdown` and adds only the deltas + its own `embeddedLanguages`. Owns the brand, stops the `.md` leak, **stays entirely clear of the Quarto `.qmd` association** (no precedence fight), license-clean (MIT base only), tiny reviewable data-only diff. |

**Why:** it is the only option that (1) owns a distinct `taliesin` language/brand, (2) doesn't leak into
`.md`, (3) never collides with Quarto (it doesn't touch `.qmd` — a clean separation, per the owner's
"leave Quarto behind" direction), (4) has zero AGPL exposure, (5) inherits all
CommonMark + embedding for free via `include`, and (6) delivers the two deltas the owner most wants and
even Quarto leaves as plain comments: **`#|` cell options** and **`@xref`/`[@cite]`**. The one unavoidable
piece in *every* approach — highlighting the **braced** `{python}` form (base markdown only matches bare
`python` / dotted `{.python}`) — is authored once here.

Scope-name convention: `text.tmd.markdown` (grammar scopeName), sub-token scopes suffixed `.tmd`.

## 4. Constraints / invariants to respect

- **Offline-first:** the grammar is static bundled JSON (no network). Sub-grammars degrade *gracefully*
  to no inner color if a language extension is absent — that's acceptable, not an error.
- **Single editing surface / read-only preview:** a grammar only colorizes; it never writes back.
  Do not touch the render pipeline or the `qmd-goto`/`qmd-cursor` IPC.
- **Drift-lock discipline:** color the **generic** `:::`+`{.class}` construct — do NOT enumerate the
  callout/theorem/layout vocab from `validate.rs`, or it drifts from the Rust lists.
- **Stay in scope:** the manifest **rebrand** (`qmd-fast`→Taliesin identity: name, publisher, command
  id, config key, panel id) is **explicitly deferred** to a separate follow-up. This item adds the
  language/grammar and teaches the `.tmd` gates only.

## 5. Phased plan

### Phase 0 — Scaffold the language + grammar (registration only)
**Goal:** a real `taliesin` language resolves for `.tmd` with an empty grammar that `include`s
markdown, so CommonMark + inherited YAML/math already color. Prove registration headlessly first.
- `package.json` → `contributes.languages`: `{ "id": "taliesin", "aliases": ["Taliesin","tmd"],
  "extensions": [".tmd"], "configuration": "./language-configuration.json" }`. **`.tmd` only** —
  do NOT list `.qmd` (decision 1: no Quarto `.qmd` association). Leave a comment noting `.tmd` is the
  native extension and `.qmd` is intentionally left to Quarto/plaintext.
- `package.json` → `contributes.grammars`: `{ "language": "taliesin", "scopeName": "text.tmd.markdown",
  "path": "./syntaxes/tmd.tmLanguage.json", "embeddedLanguages": { "meta.embedded.block.frontmatter":
  "yaml", "meta.embedded.block.python": "python", "meta.embedded.block.r": "r",
  "meta.embedded.block.js": "javascript", "meta.embedded.block.julia": "julia",
  "meta.embedded.block.mermaid": "mermaid", "meta.embedded.block.sql": "sql",
  "meta.embedded.math.tmd": "latex" } }`.
- `syntaxes/tmd.tmLanguage.json`: `$schema`, `name`, `scopeName: text.tmd.markdown`, `patterns:
  [{ "include": "text.html.markdown" }]` only (no deltas yet). Under `syntaxes/`, NOT `src/`.
- `language-configuration.json`: HTML comments, brackets, autoClosingPairs incl. `{{< ` / ` >}}`
  and `$`/`$$`, wordPattern. Author fresh (do NOT copy Quarto's — AGPL).
- **Files:** `package.json`, `syntaxes/tmd.tmLanguage.json`, `language-configuration.json`.
- **Verify:** e2e (`npm run test:e2e`) — open a `.tmd`, assert `doc.languageId === 'taliesin'` and
  `getLanguages()` includes `taliesin`. Manual F5 on `corpus/native-tmd.tmd` → status bar "Taliesin",
  markdown + fenced-python body colored via the inherited grammar.

### Phase 1 — Braced exec cells + cell-option directives (the load-bearing delta)
**Goal:** `{python}`/`{r}`/`{js}`/`{mermaid}` cells get real inner-language color; `#|`/`//|`/`%%|`
option lines + keys get distinct scopes.
- `repository.taliesin_cell` — a begin/end fenced rule matching an info string like
  ``` ```{python} ```; begin ≈ `^(\s*)(` + "```" + `{3,})\s*\{\.?(?<lang>[a-zA-Z][\w-]*)[^}]*\}\s*$`,
  end matches the same-length close. TextMate can't set `contentName` dynamically → **one rule per
  supported lang** (python, r, js, julia, mermaid, sql, ojs), each hardcoding `contentName:
  meta.embedded.block.<lang>` + `patterns: [ <cell-option rule>, { "include": "source.<lang>" } ]`.
  Scope the `{lang}` header as `keyword.other.taliesin` / `fenced_code.block.language.tmd`.
- `repository.taliesin_cell_option` — match `^\s*(#\||//\||%%\||#\s\||//\s\||%%\s\|)\s*([\w-]+)(:)`
  (mirror the tolerated-space forms from `cell_extract.rs:15-23`). Scope marker as
  `punctuation.definition.directive.taliesin`, key as `keyword.other.tmd.cell-option`. **Reference it
  from inside each cell body** so it wins before `source.<lang>`'s own comment rule.
- Exclude raw-output attrs: `{=html}`/`{=latex}` are NOT languages (`cell_extract.rs` returns None) —
  make the `lang` capture reject a leading `=` so they fall through to markdown's generic fence.
- **Ordering:** register `taliesin_cell` in the top-level `patterns` **before** `include:
  text.html.markdown`, or markdown's generic fence swallows it and loses the embedding.
- **Files:** `syntaxes/tmd.tmLanguage.json`.
- **Verify:** tokenization test (prefer the offline `vscode-textmate` + `vscode-oniguruma` harness) on a
  fixture with `{python}` (assert `meta.embedded.block.python` on the body + `keyword.other.tmd.cell-option`
  on the `#| echo` key), `{r}`, and `{=html}` (assert NO embedded-python). Manual F5.

### Phase 2 — Front matter, math, and remaining Pandoc deltas
**Goal:** cover `---`-anchored YAML front matter, `$…$`/`$$…$$` math (incl. `$$ {#eq-x}` + bare
`\begin{env}`), `:::` divs + attrs, `{{< shortcodes >}}`, `@xref`, `[@cite]`, deck `. . .`.
- **Front matter:** `taliesin_frontmatter` anchored to `\A` **only** — begin `\A-{3}\s*$`, end
  `^(-{3}|\.{3})\s*$`, `contentName: meta.embedded.block.frontmatter`, `include: source.yaml`. `\A`
  anchoring keeps a mid-doc `---` a thematic break (deck slide separator). Decide inherit-vs-override by
  Phase-0 F5 observation of whether inherited YAML color already fires.
- **Math:** inline `$…$` (`meta.embedded.math.tmd` + `include: text.tex`), display `$$…$$`, a
  `$$…$$ {#eq-…}` label tail, and bare `\begin{env}…\end{env}` (`mod.rs:443,:1662`). Guard inline `$`
  against currency/word-adjacency false positives (Oniguruma lookarounds).
- **Divs:** `taliesin_div` — `^:{3,}` open with optional `{ .class #id key="val" }` (scope `.class`
  entity.name.tag, `#id` entity.other.attribute, `key=` attribute), bare `::: classname`, bare `:::`
  close. **Generic — do NOT enumerate vocab.**
- **Shortcodes:** `taliesin_shortcode` — `{{<\s*(\w[\w-]*)\s*.*?>}}`, name `keyword.control.taliesin.shortcode`,
  args `key=value`. Single-line; must not fire inside code/backticks.
- **Xrefs + cites:** `taliesin_xref` — bare `@(fig|sec|tbl|eq|lst|thm|lem|cor|prp|def|exm|rem)-[\w-]+`
  at a word boundary (`cite/render.rs:11,285`; the `bob@rem-server.com` guard). `taliesin_cite` —
  `\[-?@[\w:-]+ … \]` groups (`render.rs:312`).
- **Deck pause:** a line whose only content is `. . .` (`deck.rs:378`).
- **Files:** `syntaxes/tmd.tmLanguage.json`.
- **Verify:** tokenization fixture covering all deltas; assert intended scopes, that mid-doc `---`
  is a thematic break (not front matter), and `bob@rem-server.com` gets NO xref scope. Manual F5 on
  `corpus/callouts/*` + `corpus/native-tmd.tmd`; **open a plain `.md` and confirm it's UNAFFECTED**
  (proves no leak).

### Phase 3 — Extension coherence (`.tmd` gates) + CI + docs
**Goal:** the companion recognizes `.tmd` everywhere it hardcodes `.qmd`; land the grammar under an
automated CI gate. **Manifest rebrand is OUT of scope.**
- Teach the three gates `.tmd`: `extension.ts:14` + `:63` → factor an `isSourceFile(name)` helper
  accepting **both** `.tmd` and `.qmd` (mirror `ACCEPTED_SOURCE_EXTS` — the *renderer* handles both, so
  an existing `.qmd` file must still be previewable even though its editor language isn't `taliesin`).
  `package.json:18` menu `when` → `resourceExtname == .tmd || resourceExtname == .qmd` (extname, NOT
  `resourceLangId == taliesin` — that would strand `.qmd` files from the preview button since the
  grammar is `.tmd`-only). **Leave `qmd-goto`/`qmd-cursor` alone.**
- Add the `editor/vscode/**` CI job (`notes/backlog.md:202`): path-gated `npm ci` + `npm run build` +
  `npm test` + the grammar tokenization test. Prefer the **offline `vscode-textmate` harness** for the
  grammar gate so CI needs no VS Code download; keep `test:e2e` as an optional/local gate.
- Update `editor/vscode/README.md`: document the `.tmd` language association (`.tmd`-only — `.qmd` is
  intentionally left to Quarto/plaintext), the optional `"files.associations": {"*.qmd": "taliesin"}`
  opt-in for anyone who wants their `.qmd` files highlighted by Taliesin, and how to run the grammar test.
- **Files:** `src/extension.ts`, `package.json`, `README.md`, `.github/workflows/ci.yml`.
- **Verify:** `npm run build` + `tsc -p . --noEmit` exit 0; grammar test + CI job pass; Open Preview
  now works from a `.tmd`; F5 regression check that cursor sync still works after the gate change.

## 6. Decisions

1. **Claim `.qmd` too, or `.tmd`-only?** → **DECIDED 2026-07-02: `.tmd` only.** The grammar/language
   binds `.tmd` and never touches `.qmd`, so there is no Quarto precedence conflict — Taliesin owns its
   own extension cleanly. This is the deliberate first step of the owner's "completely leave Quarto
   behind / a separate tool" direction (see `[[quarto-separation-direction]]` in memory). The *renderer*
   still accepts `.qmd` as deprecated input, and the preview command/gates still accept `.qmd` files, so
   nothing existing breaks — those `.qmd` files simply keep whatever editor language another extension
   (Quarto) or the plaintext fallback gives them. No `files.associations` escape hatch is shipped (a user
   who wants `.qmd` to use the Taliesin grammar can add `"files.associations": {"*.qmd": "taliesin"}`
   themselves).

Still to decide at execution time:

2. **Bespoke themable scopes for `#|`/`@xref`/`[@cite]`, or reuse `comment.*`/`markup.*`?** → *Recommend
   bespoke* (the owner wants full control) — pick scope names most themes color reasonably.
3. **Grammar test mechanism:** `vscode-textmate` offline unit harness (2 new devDeps, no VS Code
   download) vs scope-inspection in the existing `@vscode/test-electron`. → *Recommend `vscode-textmate`*
   for the CI grammar gate; keep `test:e2e` for language-registration + local.
4. **Rebrand the extension identity now or separately?** → *Recommend separate* — the grammar is
   additive and shippable on the still-`qmd-fast`-named extension; the rebrand is a broader change.

## 7. Risks (from the adversarial pass)

- **Oniguruma ≠ JS regex.** Named groups, `\A`, lookbehind/possessive quantifiers differ. Author/test
  the braced-fence pairing + `$`-math guards against the **real** tokenizer (`vscode-textmate` uses the
  Oniguruma WASM), never Node `RegExp`, or they pass a naive test and fail in the editor.
- **Embedded scope names must be exact** (`source.python`, `source.r`, `source.js`, `source.yaml`,
  `text.tex.latex`, `source.sql`). A typo → silent no-inner-color. `source.mermaid`/`source.julia`
  need the user's language extension; absent → plaintext body (document it).
- **~~`.qmd` vs Quarto precedence~~ — RESOLVED by decision 1** (`.tmd`-only): the grammar never claims
  `.qmd`, so there is no association conflict with Quarto. (If a user *wants* `.qmd` to use the Taliesin
  grammar, that's their opt-in `files.associations`, not our default.)
- **Rule greediness / ordering:** deltas MUST precede `include: text.html.markdown`; verify by
  tokenization test, not by eye.
- **Front-matter double-handling:** the inherited grammar may already tokenize `---` front matter;
  resolve inherit-vs-override empirically in Phase 0/2 F5.
- **Manifest still `qmd-fast`:** a `taliesin` language on a `qmd-fast`-named extension is coherent but
  slightly odd — flag the rebrand as a tracked follow-up, not silently skipped.

## 8. Acceptance criteria

- F5 on a `.tmd` shows **"Taliesin"** in the status bar; `getLanguages()` includes `taliesin` (asserted).
- The `contributes.languages` `extensions` array is **`[".tmd"]`** — opening a `.qmd` file does NOT
  resolve to the `taliesin` language (it stays Quarto/markdown/plaintext); no `.qmd` association is
  contributed. The preview **command** still works on a `.qmd` file (renderer back-compat).
- A `{python}` cell body tokenizes as `meta.embedded.block.python`; `{=html}` does **not** (real
  Oniguruma tokenization test).
- `#|`/`//|`/`%%|` options + keys, `@fig-`/`@sec-`, `[@cite]`, `::: {.class #id}`, `{{< shortcode >}}`,
  `$…$`/`$$…$$`, deck `. . .` each get their intended scope; `bob@rem-server.com` does not.
- CommonMark colors via the inherited grammar with **no vendored base rules**; a mid-doc `---` stays a
  thematic break.
- A plain `.md` shows **none** of the Taliesin scopes (no leak).
- Grammar is a data-only asset under `syntaxes/`; `npm run build` + `tsc --noEmit` exit 0.
- All three `.qmd`-only gates recognize `.tmd`; the frozen IPC strings + preview/cursor sync still work.
- An `editor/vscode/**` CI job runs the grammar test **offline** (no VS Code download); nothing needs
  the network.
- **No AGPL Quarto grammar copied/derived** — only the MIT markdown grammar is `include`d.

## 9. Appendix — the Taliesin syntax deltas the grammar must add (grounded in the parser)

Everything else (headings, emphasis, inline/fenced code with *bare* info strings + embedding, links,
images, blockquotes, lists, tables, strikethrough, autolinks, footnotes, thematic break) is **already
covered** by `text.html.markdown` — the grammar adds ONLY these:

| Construct | Example | Parser reference | Sub-lang? |
|---|---|---|---|
| Braced exec cell | ` ```{python} ` | `cell_extract.rs:165` (`code_lang`) | yes (python/r/js/…) |
| Cell options `#\|`/`//\|`/`%%\|` | `#\| echo: false` | `cell_extract.rs:15`; keys `validate.rs:18` | no |
| YAML front matter | `---\ntitle: X\n---` | `frontmatter.rs:318`; keys `:19` | yes (yaml) |
| Fenced div `:::` + attrs | `::: {.callout-note title="x"}` | `divs.rs:101` (fence), `:162` (attrs) | no |
| Pandoc attrs on headings/spans/imgs | `## Title {#sec-x}` | `divs.rs:11` (`parse_pandoc_attrs`) | no |
| Math `$…$` / `$$…$$` / `$$ {#eq-x}` / `\begin{}` | `$e^{i\pi}$` | `mod.rs:28,443,1662`; `math.rs` | yes (latex) |
| Cross-refs `@fig-`/`@sec-`/… | `see @fig-scree` | `cite/render.rs:11,285` (+ word-boundary guard) | no |
| Citations `[@key]` | `[@bishop2006, p.12]` | `cite/render.rs:312` | no |
| Shortcodes `{{< … >}}` | `{{< embed slides.tmd >}}` | `extension/mod.rs:364,440,491,499`; `includes.rs:224` | no |
| Deck pause `. . .` | `. . .` | `deck.rs:378` (`is_pause`) | no |

Fixture seed: `corpus/native-tmd.tmd` already exercises front matter + a bare python block +
a `.callout-note` div. `corpus/explorable/scrolly.qmd` and `corpus/callouts/` have richer examples.

## 10. Reference material (all inspectable, all MIT unless noted)

- MIT full-grammar + `embeddedLanguages` template: `.vscode-test/…/extensions/markdown-basics/`
  (`package.json`, `syntaxes/markdown.tmLanguage.json` — see `fenced_code_block_python` + `frontMatter`
  rules for the `contentName: meta.embedded.block.*` + `include: source.*` technique).
- MIT injection template: `.vscode-test/…/extensions/markdown-math/` (`injectTo` + `injectionSelector`
  + `embeddedLanguages → latex`).
- **AGPL, reference-only, do NOT copy:** `~/.vscode/extensions/quarto.quarto-1.134.0/` (full `quarto`
  language, scopeName `text.html.quarto`, 165KB plist grammar; note its `#|` reads as a plain comment —
  we beat that).
