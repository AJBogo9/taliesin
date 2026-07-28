# VS Code companion + autocomplete audit, 2026-07-28

Scope: `editor/vscode/` (the Taliesin Companion), its completion surface, and what the
state of the art in comparable extensions looks like. Every gap below was **measured**,
not read off the source: the completion-context table comes from driving the compiled
`detectContext` over 21 real cursor positions, the packaging findings from `unzip -l` on
the built `.vsix`, and the LSP findings from the binary's own capability advertisement.

---

## The headline: you already wrote the language server, and the extension does not use it

`crates/server/src/lsp*.rs` is **5,071 lines** of working, tested LSP server
(`taliesin lsp`, stdio, offline, kernel-free). It advertises definition, documentSymbol,
hover, completion, codeAction, **rename with prepareRename**, and live diagnostics.

`editor/vscode/src/` is **~1,900 lines** of TypeScript that re-implements the same
features against the same `taliesin` binary by shelling out to `vocab` / `symbols` /
`check`. It has no `vscode-languageclient` dependency. The two never meet.

`lsp_complete.rs`'s own header says it: *"A Rust port of the companion's `complete.ts`"*.
So the same context-detection logic exists twice, in two languages, and **nothing gates
them against each other**. There is no parity test. They have already drifted:

| | Rust LSP | TS companion |
|---|---|---|
| completion trigger chars | `@ . \| - / :` | `@ . \| - /` (no `:`) |
| rename an xref anchor + its references | yes | **no** |
| hover an `{{< include >}}` path | no | no (fixed today, TS only) |

The `:` divergence is not cosmetic: in the companion, `format:` does not open its value
list until you type another character.

**This is the decision to make first**, because it determines where every recommendation
below gets implemented. Nothing in `notes/backlog.md` or `ROADMAP.md` plans this
migration, so it is genuinely open.

---

## Part 1: the autocomplete audit

### What exists (and it is well built)

Seven completion contexts, all driven by `taliesin vocab` so they cannot drift from what
`check` enforces. That architecture is right and worth keeping:

- front-matter keys (27 top-level + 5 nested parents), front-matter values (`format`, `theme`)
- cell options (14), div classes (9 + 5 callouts + 8 theorem kinds)
- `@` cross-refs, merging a live buffer scan with `taliesin symbols` (which is how a
  `#| label: fig-scree` cell figure completes at all)
- `[@cite]` keys read from the front-matter `.bib`
- `{{< embed/include PATH >}}` with directory descent

### What is missing (measured)

Driving the compiled `detectContext` over 21 realistic cursor positions:

| cursor | result |
|---|---|
| `$\al` (inline math) | **none** |
| `\begin{ali` (display math) | **none** |
| `{{< ` / `{{< vid` (shortcode name) | **none** |
| `{{< video a.mp4 ` (shortcode arg) | **none** |
| `{{< input type=` | **none** |
| ` ```{py ` (cell language) | **none** |
| `#\| echo: ` (cell-option value) | **none** |
| `#\| label: fig-` (label prefix) | **none** |
| `# Heading {#` (anchor id) | **none** |
| `::: {.theorem ` (div attr key) | **none** |
| `[text](` (link target) | **none** |
| `![cap](` (image path) | **none** |
| `bibliography: ` / `css: ` (path-valued front matter) | detected, **yields nothing** |
| front-matter key / value, `@xref`, `[@cite]` | works |

**12 of 21 positions offer nothing.** The vocabulary that *does* exist is exposed well;
the problem is how much of `.tmd`'s surface never reaches it.

### Math specifically

There is no math branch in `detectContext` at all, and `\` is not a trigger character, so
`$…$` and `$$…$$` are dead zones for completion. The grammar *does* colorize math (a
self-contained `math_body` rule set in the injection grammar, plus a
`meta.embedded.math.tmd → latex` mapping), so this reads as an inconsistency: math looks
supported and behaves unsupported.

The fix fits the existing architecture unusually well. Taliesin renders math with
**KaTeX in-process** (`crates/core/src/math.rs`, the `katex` crate), so "what is a valid
command here" is already a closed, known set owned by Rust, exactly like `calloutKinds`
or `cellOptions`. Adding a `mathCommands` array to `crates/core/assets/vocab/tali-vocab.json`
(from KaTeX's supported-functions list) plus a `Math { typed }` context would give
`\` completion that provably matches what renders. There is even an enforcement backstop
already: `check.rs:220` runs `dx::validate_math(blocks)`, and an unrenderable expression
becomes a `tali-math-error` span rather than a silent pass.

Two refinements worth stealing, both from Tinymist (Typst):

- **stepless completion**: `$ar|$` completes to `$arrow.r$` without a separate accept step
- **a symbol picker view**: symbols browsable by category and by keyword, click to insert

And from LaTeX Workshop: **hovering a math environment renders a preview of it**. Taliesin
can do this better than anyone, because KaTeX is already in the binary and already
memoized: a hover could return the rendered expression rather than a description of it.

---

## Part 2: the extension audit

### Security

**`taliesin.path` is workspace-settable, and it names a binary the extension spawns.**
The property declares no `scope`, so it defaults to `window`, which a repository's own
`.vscode/settings.json` can set. Opening an untrusted `.tmd` repo therefore hands it
arbitrary code execution as soon as diagnostics fire (`registerDiagnostics` runs on
open, and `refresh` spawns immediately for every already-open document). The standard
hardening is `"scope": "machine"` (what `python.defaultInterpreterPath` uses), which
makes the setting user/machine-level only. The extension also declares no
`capabilities.untrustedWorkspaces`.

The webview itself is fine: a tight CSP, `default-src 'none'`, a nonce'd relay script,
and the message handler filters on `m.type`.

### Packaging

`unzip -l taliesin-companion-0.1.0.vsix` shows the shipped extension contains:

- `out/test/*.js` and `out/e2e/**` (the whole test suite)
- `test-fixtures/`, `scripts/relay-harness.cjs`, `scripts/ensure-vscode.cjs`
- `.vscode/launch.json`, `.gitignore`
- **both** the 40 KB esbuild bundle *and* every unbundled module it already contains
  (`complete.js`, `completions.js`, `hover.js`, …), so the runtime code ships twice

`.vscodeignore` is four lines and excludes none of it. It should exclude `out/test/`,
`out/e2e/`, `test-fixtures/`, `scripts/`, `.vscode/`, `.gitignore`, `package-lock.json`,
and (given the bundle) every `out/*.js` except `extension.js`.

Related, and already burned once (see `notes/2026-07-13-companion-check-unexpected-output-bug.md`):
nothing rebuilds the `.vsix` as part of any gate, so a source fix can sit in the tree
while the installed companion runs stale code.

### Marketplace / discoverability metadata

`"private": true`, `categories: ["Other"]`, no top-level `icon`, no `keywords`, no
`galleryBanner`, no `badges`, no `CHANGELOG.md`, no `walkthroughs`. If this is ever
published, `categories` should be `["Programming Languages", "Snippets", "Linters",
"Notebooks"]`, and a `contributes.walkthroughs` entry is now the standard first-run
experience (it is how Quarto and Tinymist onboard).

### Feature surface: one command, no keybindings

`contributes.commands` has exactly **one** entry (`taliesin.openPreview`). There are no
keybindings, no editor context-menu entries, and one setting.

Compare what a `.tmd` author does constantly and currently does by hand:

- toggle bold / italic / code (Markdown All in One binds `Ctrl+B` / `Ctrl+I`)
- continue a list or blockquote on Enter (`onEnterRules` handles only `:::` today)
- format a table (`Alt+Shift+F` in Markdown All in One)
- insert a callout / figure / cell (snippets exist, but nothing is bound)
- run the cell under the cursor, run all cells (Quarto binds `Ctrl+Shift+Enter`)
- `taliesin check` / `build` / `skim` as tasks
- restart the kernel

The CLI already exposes `check`, `build`, `skim`, `symbols`, `map`, `read`, `vocab`,
`doctor`, `mcp`, `lsp`. Exactly one of them is reachable from the editor.

### Wiring that exists in the binary but not the extension

- **`taliesin schema`** emits `tali-frontmatter.schema.json` and `tali-site.schema.json`,
  and then *prints instructions telling the author to paste a
  `# yaml-language-server: $schema=…` comment into `_site.yml` by hand*
  (`query.rs:709`). A `contributes.yamlValidation` entry would do this automatically, and
  is exactly the "YAML intelligence for project files" Quarto's extension is praised for.
  As it stands `_site.yml` gets no editor support at all (the extension only activates
  `onLanguage:taliesin`).
- **`taliesin lsp`**, as above.
- **`taliesin doctor`** would make a good "Taliesin: Diagnose setup" command; today a
  missing binary produces one warning toast and silence everywhere else.

### Smaller defects found while reading

1. **`classifyHover` does not skip code fences.** `hover.ts`'s include regex runs over
   every line, so Ctrl-clicking a `{{< include x.tmd >}}` shown as an *example* inside a
   ``` fence offers a go-to-definition the renderer would never follow. (The new
   `links.ts` deliberately does not repeat this.)
2. **`classifyHover`'s include regex does not strip quotes**, so
   `{{< include "part.tmd" >}}` resolves against a path with a literal `"` in it.
3. **The include regex matches mid-line**, but the engine's `parse_include` requires the
   directive to own its whole trimmed line, so a mid-sentence include is offered a
   definition it will never have.
4. **Two independent `vocab` caches and two independent `symbols` caches** exist
   (`completions.ts` and `hover-provider.ts` each build their own), so every document
   spawns `taliesin symbols` twice.
5. **`cite` completion does not filter by the typed prefix** the way `xref` does. Harmless
   today because VS Code filters client-side, but the two paths should agree.

---

## Part 3: what the state of the art looks like

| extension | the thing worth taking |
|---|---|
| **LaTeX Workshop** | completion for `\cite{}`/`\ref{}` from the real bibliography and label set; **math environment hover renders a MathJax preview**; `@`-prefixed Greek letters; `BXY` environment shortcuts; user-configurable package dirs so IntelliSense covers non-standard packages |
| **Tinymist (Typst)** | **symbol picker view** (search by keyword, description, or *handwriting*; grouped by category; click to insert); **stepless symbol completion** (`$ar` → `$arrow.r$`); completion that triggers again on each snippet placeholder; template gallery; document summary / symbols / fonts views; slide thumbnails; workspace label list |
| **Quarto** | LSP-based completion, diagnostics, hover, navigation; completion **inside embedded Python/R/Julia**; YAML intelligence for project files, front matter *and* cell options; **clickable document links for file paths in `_quarto.yml`**; cell-by-cell execution; visual (ProseMirror) editing mode; Zotero-backed citation completion |
| **MyST** | hover + completion for directives and roles; injection grammar over base markdown; enhanced preview |
| **Markdown All in One** | keybindings for bold/italic/heading levels; list and blockquote auto-continuation with CommonMark-correct indentation; list-marker cycling; **table formatter**; auto-updating TOC |

Read together, the bar for a document-format extension in 2026 is: an LSP doing the
intelligence, embedded-language completion inside code cells, path completion everywhere a
path is legal, a symbol/insert palette for the notation-heavy parts, and editing
ergonomics (keybindings, list continuation, table formatting) as table stakes.

Taliesin's companion has the *hardest* piece already (Rust-authoritative vocabulary that
cannot drift from the validator, plus a working LSP). It is missing most of the easy ones.

---

## Part 4: what shipped (same day)

Recommendations A through D below were executed. Every claim here is measured; the
verification block at the end says exactly what ran.

### A. The companion is now a client, not a second implementation

`src/client.ts` is a `vscode-languageclient` (10.1.0) over `taliesin lsp` on stdio. Deleted:
`complete.ts`, `completions.ts`, `hover.ts`, `hover-provider.ts`, `definition-provider.ts`,
`outline.ts`, `outline-provider.ts`, `diagnostics.ts`, `check.ts`, `debounce.ts`,
`backend.ts`, and their seven test files. **~1,900 lines of TypeScript gone**; `src/` is now
`client.ts`, `commands.ts`, `extension.ts`, `paths.ts`, `ports.ts`, `server.ts`,
`webview.ts`.

The drift table in Part 1 no longer has a right-hand column. Rename, which the server had
all along and the companion never exposed, works in VS Code now (asserted end to end). Every
other LSP editor gets each new feature at the same moment.

Two parity gaps had to be closed in Rust **first**, so the migration could not regress
anything:

- **`textDocument/documentLink`** (`crates/server/src/lsp_links.rs`, new): the include/embed
  path scan, mirroring the engine rather than re-deriving it (a shortcode in a code fence or
  backticks is an example and is skipped; an include must own its whole line and may be
  quoted; an embed's path is the first non-`key=value` token, tokenized the way
  `tokenize_args` does). Only targets that exist become links.
- **Include hover**, which both sides had deliberately declined to answer.

Untitled `.tmd` buffers now get language features too. The first `documentSelector` was
`scheme: file` only, and the rename test caught it: a scratch buffer was silently getting
nothing, including the diagnostics and completions that need no path at all.

### B. Hardening

- `taliesin.path` is `"scope": "machine"`, so a repository's `.vscode/settings.json` can no
  longer redirect the binary the extension executes. `capabilities.untrustedWorkspaces` is
  declared `limited` with that setting restricted, and `virtualWorkspaces` false.
- `.vscodeignore` rewritten. The `.vsix` went from **41 files including the whole test
  suite, fixtures, dev scripts and a second unbundled copy of the runtime** to **11 files**.
  `vscode:prepublish` now minifies.
- `engines.vscode` `^1.85` → `^1.91` (what `vscode-languageclient` 10 requires), real
  `categories` and `keywords`, and the redundant `activationEvents` entry dropped.
- Three new manifest gates. The most important: **the subcommand the language client
  launches must be a real one**. The existing gate scanned `spawn(` call sites, and the
  client starts `taliesin lsp` from `ServerOptions` instead — so the moment the providers
  moved to LSP, the single command the whole companion rests on stopped being covered. The
  other two: every contributed command must be registered, and every keybinding and menu
  entry must point at a contributed command.

### C. The completion gaps

Re-measured with the same 21-position probe, driven against the real `taliesin lsp` over
stdio: **9/21 before, 20/21 after.**

| position | before | after |
|---|---|---|
| `$\al`, `$$…\alp` (math) | none | `\alpha`, `\aleph`, … |
| `{{< `, `{{< vid` | none | `include`, `embed`, `video`, `input` |
| `{{< video cl` | none | the media files beside the doc |
| `{{< input type=` | none | `slider`, `range`, `number`, … |
| ` ```{py ` | none | `python` (marked executed), `r`, `js`, `mermaid`, … |
| `#\| echo: ` | none | `true`, `false` |
| `#\| label: fig-` | none | the cross-reference prefixes |
| `# Heading {#` | none | the cross-reference prefixes |
| `[text](`, `![cap](` | none | files; an image narrows to image extensions |
| `bibliography:`, `css:`, `image:`, `logo:`, `include-*:` | detected, empty | the matching files, with directory descent |
| `::: {.theorem ` (div attribute keys) | none | **still none** |

**Math is the one to look at.** `crates/core/src/math_vocab.rs` is 217 commands across nine
categories, and it is authoritative for a reason the other vocabularies get for free:
KaTeX is *in the binary*, so `every_command_renders` renders each entry's probe through
`crate::math` and fails the build if KaTeX cannot parse it. An offered command that would
render as a red `tali-math-error` span for the reader cannot ship. Commands with arguments
insert LSP snippets (`\frac{$1}{$2}`), the edit replaces the typed control sequence rather
than appending to it, and `\` is a completion trigger character — gated on `in_math`, not
on the backslash, so prose is unaffected (pinned by tests for escaped `$`, escaped `\\`,
code cells, currency `$`, and math that closed earlier on the line).

`cellLanguages` is new in the vocabulary and carries an `executes` flag pinned against
`render::executes_to_kernel`, because that function decides whether a labelled cell can
produce a numbered float: a completion that got the split wrong would teach an author to
label a `{bash}` cell `fig-…` and wait for a figure that never comes.

### D. Ergonomics

Commands went from one to six: *Open Preview* (now `Ctrl+Shift+K`), *Check This Document*,
*Diagnose Setup (doctor)*, *Restart Language Server*, *Show Language Server Log*, and
**Insert Math Symbol** (`Ctrl+Alt+M`) — a QuickPick over the same math vocabulary, matching
on name, glyph and category, because a symbol you cannot spell is not reachable by
completion at all. Plus a `taliesin.trace.server` setting and a proper log output channel.

### Verified

- `./tools/gates.sh` (with `TALIESIN_PYTHON` pointed at the ipykernel venv): **PASSED — every
  gate ran and passed.** fmt, clippy `-D warnings`, `cargo test --workspace` with all four
  `TALIESIN_REQUIRE_*` gates armed, both `tsc` checks, the publish-passcode node test, the
  companion suite, `cargo audit`, `cargo deny check`.
- `cargo test --workspace`: 106 test binaries, 0 failures.
- `npm test` (companion): 45 passing, 0 failing.
- `npm run test:e2e`: **10 passing** in a real Extension Host (VS Code 1.130.0) — every
  language assertion now goes through the server.
- `taliesin check docs/internals`: no problems found.

### Not done

- **Div attribute keys** (`::: {.theorem ` → `title=`, `#id`): the one probe position still
  answering nothing.
- **Embedded-language completion inside `{python}` / `{r}` / `{js}` cells** (recommendation
  E). Unchanged, and still the biggest remaining jump.
- **`contributes.yamlValidation` for `_site.yml`.** `taliesin schema` still prints
  instructions telling the author to paste a `# yaml-language-server: $schema=…` comment by
  hand. Wiring it needs the schema files copied into the extension at build time plus a
  drift gate, and it only works when `redhat.vscode-yaml` is installed.
- **A marketplace icon** (a 128×128 PNG; only the language-file SVG exists) and a
  `walkthroughs` first-run experience.
- **Nothing is committed.** The working tree is shared with another session (the branch is
  `deck-harness-2026-07-28` and carries unrelated uncommitted work in `corpus/deck.tmd`,
  `crates/core/tests/`, and a new `crates/server/tests/deck_browser.rs`), so committing here
  would sweep up someone else's changes.

## Part 5: the plan that was executed

**A. Architecture.** Make the companion a thin `vscode-languageclient` over `taliesin lsp`,
delete the duplicated TS providers, and add every new intelligence feature in Rust once.
**Done.**

**B. Hardening.** `"scope": "machine"` on `taliesin.path`; fix `.vscodeignore`; add manifest
gates. **Done.** The four `classifyHover` defects listed in Part 2 went away with the file:
`hover.ts` was deleted, and `lsp_links.rs` was written to mirror the engine's fence,
inline-code, whole-line and quote rules rather than repeat the regex's mistakes.

**C. Completion gaps.** Math, then paths, then shortcode names, then cell values and
languages, then anchor ids. **Done, 20/21.** Div attribute keys remain.

**D. Ergonomics.** Commands and keybindings. **Done.** List/blockquote continuation and a
table formatter are still open — they are pure `language-configuration.json` + command work
and touch nothing else, so they can land whenever.

**E. Embedded-language completion inside `{python}` / `{r}` / `{js}` cells.** Not started,
and still the single biggest "it feels like a real IDE" jump. The grammar already maps the
embedded scopes; making Pylance et al. answer inside a cell is the remaining piece.

## How to verify anything here

The three that matter, in the order that catches the most:

```sh
TALIESIN_PYTHON=~/.local/share/qmd-venv/bin/python ./tools/gates.sh   # every gate, refuses to be green if one skipped
cd editor/vscode && npm test                                          # companion unit + manifest gates
cd editor/vscode && npm run test:e2e                                  # REAL Extension Host, 10 tests
```

**`npm run test:e2e` genuinely works headless** (no `xvfb` needed, and `xvfb-run` is not
installed anyway). `editor/vscode/README.md` says an Extension Host "can't be driven
headlessly"; that is true only of the preview iframe. `@vscode/test-electron` downloads VS
Code once into `.vscode-test/` and runs a real host in about a second. It needs
`target/debug/taliesin` to exist.

**Use it.** For anything the editor surfaces, a unit test proves the server answered; only
the host proves VS Code asked and rendered. The pattern is
`vscode.commands.executeCommand("vscode.execute<X>Provider", ...)`, polled through
`waitForValue` because the server answers asynchronously.

To re-measure completion coverage, drive the built binary over stdio the way
`crates/server/tests/lsp_stdio.rs` does: write fixtures, `didOpen`, then a
`textDocument/completion` per cursor position, and count the non-empty results. Scope every
assertion to the RESPONSE id — the same stream carries `publishDiagnostics` notifications
that quote the document's text, so a `stdout.contains(…)` can pass on a diagnostic while the
response it claims to test is empty.

## What to pick up next

Roughly in value order:

1. ~~**Embedded-language completion in code cells.**~~ **DONE — see Part 7.** The predicted
   shape was right (middleware + virtual document in the client); the virtual-document
   *scheme* was the part that had to be measured. Verified for `{js}` only — read Part 7
   before assuming Python works.
2. ~~**KaTeX hover preview.**~~ **DONE — see Part 6.** The plan above was wrong on its
   central premise (there is no SVG to inline, because KaTeX cannot emit one); what shipped
   is a MathML-derived Unicode preview. Read Part 6 before touching it.
3. **Stepless math completion** (Tinymist's trick: `$ar|$` → `$arrow.r$` with no separate
   accept step), and fuzzy matching on the glyph rather than only the name — the vocabulary
   already carries the glyph in `description`.
4. **`contributes.yamlValidation` for `_site.yml`.** `query.rs:709` currently prints
   instructions telling the author to paste a `# yaml-language-server: $schema=…` comment by
   hand. Needs `crates/core/assets/schema/*.json` copied into the extension at build time
   plus a drift gate asserting the copy matches, and it only takes effect when
   `redhat.vscode-yaml` is installed.
5. **List and blockquote continuation, and a table formatter.** Table stakes since Markdown
   All in One. Continuation is `onEnterRules` in `language-configuration.json` (which handles
   only `:::` today); the formatter is a `DocumentFormattingEditProvider` or a command.
6. **Div attribute keys** (`::: {.theorem ` → `title=`, `#id`), the last of the 21 probe
   positions still answering nothing.
7. **A marketplace icon** (128×128 PNG; only the language-file SVG exists) and a
   `contributes.walkthroughs` first-run experience, if this is ever published.

## Part 6: the math hover (second pass, same day)

### The plan's premise was false, and checking it first saved the work

Item 2 above said to render the expression into the hover, "likely an inline `data:` SVG",
on the reasoning that KaTeX is already in the binary so the render is nearly free. **KaTeX
cannot produce an image at all.** `katex-0.4.6`'s `OutputType` is `Html | Mathml |
HtmlAndMathml` (`opts.rs:235`); the HTML half is `<span>`s positioned by the KaTeX
stylesheet and its web fonts, which a hover cannot load, and there is no SVG mode to reach
for. LaTeX Workshop can do the image version because MathJax has SVG output. Taliesin cannot
follow it without adding a second math engine.

Two further findings from measuring rather than assuming, both worth keeping:

- **VS Code hovers do not render math.** The workbench *does* bundle KaTeX and lazily loads
  `katex.min.js` with a MathML tag allowlist and a CSS-property allowlist shaped exactly to
  KaTeX's inline styles — but it is reached through a `…math.enabled` config in the **chat**
  renderer. The built-in `markdown-math` extension covers the *markdown preview* and
  *notebooks*. Neither path is the hover renderer. `MarkdownString` has no math flag.
- So the only preview a hover can display is **text**.

### What shipped

`crates/core/src/math_preview.rs` (new): `unicode_preview(latex, display) -> Option<String>`,
built from the **MathML half** of the existing `crate::math::render` — so KaTeX does the
parsing and the preview cannot disagree with the reader's page about what the source means.

The walk is structure-aware because a flat text extraction *lies*, which the spike showed
before any code was written:

| source | flat extraction | what ships |
|---|---|---|
| `\frac{a}{b}` | `ab` | `a/b` |
| `\frac{a+1}{b}` | `a+1b` | `(a+1)/b` |
| `x^2` | `x2` (reads as ×) | `x²` |
| `\int_0^1 x\,dx` | `∫01x dx` | `∫₀¹x dx` |
| `\sum_{i=1}^n i` (display) | `∑i=1ni` | `∑ᵢ₌₁ⁿi` |

`munder`/`mover`/`munderover` are handled beside `msub`/`msup`/`msubsup` because display mode
spells the same limits differently — miss them and every `$$…$$` previews worse than every
`$…$`. Scripts fall back to `^`/`_` when Unicode has no script form (Greek exponents).
KaTeX's typographic spacing (`\,` is U+2009) is flattened to U+0020 and zero-width
characters are dropped: a preview must not contain characters you cannot see.

**There is no error branch, deliberately.** The first version had one, and it was dead code
with a vacuous test: removing `if html.contains("katex-error")` entirely still passed all
nine tests, because KaTeX with `throw_on_error = false` replaces the *whole* output with a
bare error span carrying no `<math>` element — so the `?` on the MathML lookup already
returned `None`. The guard is gone and `error_output_carries_no_mathml` is the canary: if
KaTeX ever emits partial MathML beside an error marker, it fails first and loudly, rather
than a reader getting a confident preview of a broken expression.

`lsp_nav.rs` gained `scan_math`, **the single owner of the `$` delimiter rules**, and
`Target::Math`. `lsp_complete::in_math` (35 lines of its own scanner) now delegates to it, so
completion's "am I inside math?" and hover's "which expression am I inside?" cannot drift —
which is the same failure this branch was created to delete. All 34 completion tests,
including every escape/fence/currency edge case, pass unchanged across that refactor.
`Target::Math` carries absolute positions because display math crosses lines and every other
target is line-relative.

### Verified

- `TALIESIN_PYTHON=… ./tools/gates.sh`: **PASSED — every gate ran and passed** (all nine).
- `npm test` (companion): **45 passing**.
- `npm run test:e2e`: **11 passing** (was 10) in a real Extension Host, VS Code 1.130.0 — the
  new one hovers `$\alpha + \beta$` in an untitled buffer and asserts `α+β` came back.
- Two mutation checks, by inverse edit (never `git checkout` on uncommitted work): neutering
  `group()` kills only the compound-fraction test; ignoring the code-fence state kills only
  `math_inside_a_code_fence_is_code_not_math`. Both pins are real, neither is vacuous.

### Left open

- **Hovering a single command** (`\varepsilon` → "ε, Greek") rather than the whole enclosing
  span. The vocabulary already carries the glyph, and this is arguably the higher-value half
  for the long tail (`\preccurlyeq`), since an author usually knows what they just typed.
- **Matrices and cases.** `<mtable>` falls through to plain concatenation, so
  `\begin{matrix}` previews as a run of cells with no row structure. It is not *wrong*, but
  it is the weakest output; a `;`-per-row separator would be the cheap fix.
- Items 1 and 3–7 of the list above are untouched.

## Part 7: embedded-language completion in code cells (third pass, same day)

### The one feature that cannot live in the server

LSP has no way for a server to say "this range is Python, go ask Pylance". The routing must
happen in the editor, against the editor's own provider registry. So this is the legitimate
exception to the rule above — and the split is kept honest: the **server owns the
knowledge**, the client owns only the plumbing.

- **Rust:** `crates/server/src/lsp_cells.rs` + a custom request `taliesin/cellRegions`
  (`lsp::CELL_REGIONS_METHOD`) answering where each cell is and what language it names. 11
  tests. It reuses core's own `render::option_directive` (widened from `pub(crate)`) rather
  than restating the rule, which is what keeps "what is an option line" in one place.
- **TypeScript:** `editor/vscode/src/embedded.ts` + a `provideCompletionItem` middleware.
  **No fence scanning in TypeScript** — that is the thing this branch exists to have deleted.

### Which virtual document, measured not assumed

The plan said "virtual document". There are two kinds and only one works:

| approach | result |
|---|---|
| custom URI scheme + `TextDocumentContentProvider` | `greeting.` → `const, greeting, hi`. **Word-based fallback, not IntelliSense.** The built-in TS server does not analyze a foreign scheme. |
| **untitled document** of the right language | `greeting.` → 52 items incl. `charAt`, `charCodeAt`. **Real.** |

This is worth remembering, because the failure mode is *quiet*: word-based completion looks
like a working feature until you notice every suggestion is a word already on screen. Any
test for this must assert on a member that can only come from a type (`charAt`), never on
"got some items".

### The projection

The shadow is the whole `.tmd` with every non-cell line replaced by an empty line.

- **Blanking, not slicing**, so a completion at line 12 of the `.tmd` is line 12 of the
  shadow. No offset arithmetic to get wrong.
- **Every cell of that language is kept**, not just the one under the cursor, so a later cell
  sees an earlier cell's imports — matching how Taliesin runs them (one warm kernel, shared
  state). Pinned by `a later cell sees an earlier cell's definitions`.
- **Leading `#|` option lines are dropped** by the server. They are directives, not code, and
  `#|` is a *syntax error in JavaScript* — leaving one in poisons the whole shadow buffer.
  The same pin covers this, which is why its fixture puts `#|` in a `{js}` cell.
- **`additionalTextEdits` are stripped** from forwarded items. An auto-import is computed
  against the shadow, where the surrounding lines are blank, so applying one would write an
  import into the middle of the prose.

### Verified, and the honest limit

- `./tools/gates.sh`: **PASSED — all nine.** `npm test`: **45.** `npm run test:e2e`:
  **13 passing** (was 11), in a real Extension Host.
- **Only `{js}` is verified end to end.** That is deliberate and it is the strongest test
  available here: JavaScript is the one cell language whose provider ships *with* VS Code, so
  the assertion runs in a bare host with no extension to install. Python and R go through the
  identical path and the identical projection, but **nobody has watched Pylance answer inside
  a cell** — the test host has no Python extension. Treat Python as expected-to-work, not
  proven. Installing `ms-python.python` into `.vscode-test` would close this.

### Found while verifying: the companion leaked a preview server — now fixed (Part 8)

Not caused by the embedded-completion change; found because it broke the verification of it.
Written up in Part 8.

## Part 8: the preview server outlived VS Code (fourth pass, same day)

### The symptom was that the test suite could no longer run

`npm run test:e2e` started failing at *launch*, before any test, with
`EMFILE: too many open files, watch '/snap/code'`. That is not a test failure and not a
Taliesin error, which is what made it worth chasing rather than retrying.

Cause: **17 orphaned `taliesin preview` processes**, all previewing the same corpus fixture
(`corpus/posts/born-machines.tmd` — the e2e's `SAMPLE_POST`, so unmistakably test-spawned,
not the author's). Each holds a file watcher, and together they had exhausted
`fs.inotify.max_user_instances` (128, and 128 were in use). Past that ceiling **VS Code
itself cannot start**. The e2e suite opens a preview every run, so every run added one.

### Two independent leaks

1. **The shutdown path.** `extension.ts` disposed the server only from
   `panel.onDidDispose`. Closing the *window* tears down the extension host **without**
   disposing panels, so the spawned server simply outlived VS Code. The `PreviewServer` was
   never in `context.subscriptions`, and `deactivate()` was empty. Fix: push it into
   `context.subscriptions`, and make `dispose()` idempotent since both paths can now fire.
2. **The failed-start path**, found while reading for the first. `PreviewServer.start`
   spawns, then rejects if `waitForHttp` times out — and **nothing killed the child it had
   already spawned**. The caller gets an `Error`, not a `PreviewServer`, so no handle to
   dispose exists anywhere. A binary that spawns fine and never serves leaked a process per
   attempt. Fix: kill the child on any failure out of `start`.

### How each is pinned

- **The shutdown leak is checked from the RUNNER** (`e2e/runTest.ts`), not from the Mocha
  suite — the only vantage point that *outlives the Extension Host*, which is exactly what
  the bug is about. It snapshots matching PIDs before and after `runTests`, so a preview the
  author started by hand (or a parallel session's) is never blamed on the run, and it reaps
  what it finds rather than degrading the machine further. It failed before the fix
  (`VS Code exited leaving 1 taliesin preview server(s) running`) and passes after.
- **The failed-start leak** has a unit test (`src/test/server.test.ts`) using a fake
  executable that spawns and never answers. `start` grew a `readyTimeoutMs` parameter
  (default 8000) so the test runs in ~700 ms instead of 8 s.

  One trap worth keeping: with the fix reverted, this test originally **hung** rather than
  failed — a live child handle pins Node's event loop. A hanging test blocks a gate without
  saying why, so its teardown reaps the survivor; the mutation now fails cleanly in 2.7 s
  with `the spawned process must not outlive the failed start`. Verified by inverse edit.

### Still true after the fix

A **hard kill** of the extension host (SIGKILL, a crash) runs no disposal, so the preview
server still survives that. Fixing it properly would need the child to watch its parent, and
the exec-reaping work already ruled out PDEATHSIG for this project. The common paths — close
the panel, close the window, disable/reload the extension, a failed start — are all covered.

## Rules that are now load-bearing

- **Editor features go in Rust, in `crates/server/src/lsp*.rs`.** A second copy in
  TypeScript is exactly what this change deleted. The only things that belong in
  `editor/vscode/src/` are the preview webview, the source-sync bridge, commands, and the
  one feature LSP has no concept of: routing a request into an embedded language
  (`embedded.ts`). Even there the rule holds in substance — the *knowledge* stays in Rust
  behind `taliesin/cellRegions`, and the TypeScript is pure plumbing. If you find yourself
  scanning for ``` in TypeScript, you are re-growing the deleted copy.
- **The math vocabulary must stay KaTeX-gated.** `every_command_renders` is what makes
  `math_vocab.rs` authoritative rather than a guess; adding a command without it reintroduces
  the possibility of completing something that renders as an error span for the reader.
- **stdout is the JSON-RPC wire** in the `lsp` modules. Never print to it; use `crate::log`
  (stderr).
- **`lsp_nav::scan_math` is the only implementation of the `$` delimiter rules.** Completion
  and hover both go through it. A second scanner "just for this one case" is how the
  TypeScript duplicate started.
- **Anything the companion spawns must be registered in `context.subscriptions`.** A
  disposal wired only to a panel, view or editor callback does not run when the window
  closes, and the child outlives VS Code. `e2e/runTest.ts` fails the run if a preview server
  survives it.
- **The math preview must stay structure-aware.** `<mfrac>` and `<msup>` flattened to text
  are not an approximation, they are a different expression (`a+1/b` for `\frac{a+1}{b}`). A
  preview that misinforms is worse than no preview, which is what the hover returns when it
  cannot answer.
