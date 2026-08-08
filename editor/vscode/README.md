# Taliesin Companion

Hosts the Taliesin live preview in a VS Code webview with **bidirectional source sync**.
It is the missing *producer* for the source-sync protocol the preview client
(`web-client/client.js`) already consumes:

- **Inverse search (preview → editor):** Ctrl-click a block in the preview → the editor
  jumps to that block's source line (`tali-goto`).
- **Forward search (editor → preview):** moving the cursor in the `.tmd` *marks* the
  matching block with `.tali-hl` and never moves the page; **`Ctrl+Alt+J`**
  (**Taliesin: Reveal Cursor in Preview**) scrolls it into view
  (`tali-cursor {reveal}` → `highlightAtLine`). Marking is continuous because
  it costs the author nothing; scrolling is on request because it takes their scroll
  position away.

The preview stays **read-only**: the extension only navigates and highlights; it never
writes back to the source.

## Syntax highlighting (the `.tmd` grammar)

The extension also **owns editor syntax highlighting for `.tmd`** via a `taliesin` language
(`contributes.languages` / `contributes.grammars`). The grammar is a thin superset: it
`include`s VS Code's built-in **MIT** `text.html.markdown` for all CommonMark, then adds only
the Taliesin deltas — braced exec cells (` ```{python} `/`{js}`/…, with real embedded
inner-language color and `#|`/`//|`/`%%|` cell options scoped as directives), `:::` fenced divs
+ `{.class #id key=val}` attrs, `$…$`/`$$…$$` math (with `{#eq-…}` labels), `{{< shortcodes >}}`,
`@fig-`/`@sec-`/… cross-refs and `[@cite]` citations. Leading `---` YAML
front matter is handled by the inherited markdown grammar. Inline deltas live in a small
**injection grammar** scoped to `text.tmd.markdown` (so they fire mid-paragraph but never leak
into plain `.md` files).

`.tmd` **only** is claimed, mirroring `crates/core/src/ext.rs` (a `paths.test.ts` gate asserts the
two lists agree). A stray `.qmd` is left to Quarto's extension (or plaintext); there is no
association conflict. The companion offers Open Preview on `.tmd` alone — the legacy-format clean
break made `.tmd` the only input Taliesin discovers, so a `.qmd` is not a source document here even
though `taliesin build` will still render one you hand it by path.

No Quarto grammar is copied: the base is the MIT `markdown-basics` grammar; Quarto's own VS Code
grammar is **AGPL-3.0** and is not used.

## Language intelligence: a client over `taliesin lsp`

**The extension implements no language features of its own.** Completion, hover,
go-to-definition, document links, the outline, diagnostics, quick fixes and rename all come
from `taliesin lsp` — the offline, kernel-free LSP server built into the binary
(`crates/server/src/lsp*.rs`) — over stdio. `src/client.ts` is the whole client.

It used to be otherwise: every one of those was re-implemented here in TypeScript, shelling
out to `taliesin`'s own CLI verbs, while the Rust server that already did all of it
went unused. `lsp_complete.rs` still describes itself as "a Rust port of the companion's
`complete.ts`". Nothing gated the two against each other and they had already drifted — the
server had a `:` completion trigger and rename that the companion never gained. One
implementation, in the language that owns the vocabulary, is the only version of this that
stays true, and every other LSP editor (Neovim, Helix, Zed: `cmd = { "taliesin", "lsp" }`)
gets each feature at the same moment VS Code does.

Everything offered is Rust-authoritative, so it cannot drift from what `check` enforces.
Front-matter keys, cell options, callout kinds, div classes, cell languages
and input types come from `taliesin_core::vocab`. Cross-reference targets come from the
resolved xref registry merged with a live buffer scan, which is how `@`-completion finds a
figure labelled from a cell (`#| label: fig-scree`) *and* an anchor you typed a moment ago.
Citation keys are read from the front matter's `bibliography:`.

**Math** (`$…$` / `$$…$$`) completes on `\`, from a vocabulary that is authoritative for an
unusual reason: KaTeX is *in the binary*, so `math_vocab.rs`'s `every_command_renders` test
renders every offered command through the same code path a document uses. A command KaTeX
cannot parse fails the build instead of shipping a suggestion that renders as a red error
span for the reader. Commands taking arguments insert a snippet (`\frac{$1}{$2}`), and
**Taliesin: Insert Math Symbol** (`Ctrl+Alt+M`) opens a picker searchable by name, glyph or
category — because a symbol you cannot spell is not reachable by completion at all.

`contributes.snippets` ships a small set of `.tmd` snippets (`fm`, `cell`, `figcell`, `jscell`,
`foldcell`, `callout`, `fig`, `include`, `input`). Their bodies are
gated against the same vocabulary by `src/test/manifest.test.ts`: a snippet that offers a callout
kind or cell option Taliesin no longer knows fails the build, and the callout snippet's choice
list must equal `vocab.calloutKinds` exactly, in order.

## What is left in TypeScript

Two things the LSP has no concept of:

- **The live preview + bidirectional source sync** (`server.ts`, `webview.ts`, the
  `openPreview` half of `extension.ts`). It owns a webview, spawns `taliesin preview`, and
  bridges click-to-source.
- **Editor commands** (`commands.ts`): *Check This Document*, *Diagnose Setup (doctor)*,
  *Insert Math Symbol*, *Restart Language Server*, *Show Language Server Log*.

Plus the parts that are pure manifest: the grammar, the snippets, and
`language-configuration.json`.

## Develop / run

1. `cd editor/vscode && npm install && npm run build`
2. Open the **`editor/vscode`** folder in VS Code and press **F5** ("Run Taliesin
   Companion"). A second *Extension Development Host* window opens with the extension
   loaded. (Run `npm run build` again after any source change, then reload the host.)
3. Ensure the `taliesin` binary is on `PATH` (the launcher), or set the `taliesin.path` setting
   to the binary (e.g. `target/release/taliesin`). To exercise the grammar, open any `.tmd`
   file (e.g. `corpus/native-tmd.tmd`) — the status bar should read **Taliesin**.

## Manual verification (the loop the headless tests can't close)

Most of what used to be on this checklist is now covered by `npm run test:e2e`, which drives
a real Extension Host (see below). What remains is the one thing no headless harness can
see: the visual round trip *through the live preview iframe*.

1. Open `corpus/posts/em-algorithm/index.tmd`. Run **Taliesin: Open Preview**
   (`Ctrl+Shift+K`, the command palette, or the editor-title button).
2. **Forward search, passive:** move the cursor onto a heading / paragraph — the matching
   block in the preview gains the `.tali-hl` outline. The preview must **not** scroll.
3. **Forward search, active:** press `Ctrl+Alt+J` — the preview scrolls that block into
   view and the editor keeps focus.
4. **Inverse search:** Ctrl-click a block in the preview — the editor cursor jumps to that
   block's source line.
5. **Reuse:** press `Ctrl+Shift+K` a second time — the existing panel is revealed, and no
   second `taliesin preview` process appears.
6. Close the preview panel — the spawned `taliesin preview` process exits (no orphan).

## Known risk

`vscode.env.asExternalUri` localhost-iframing inside a webview CSP can behave differently
across VS Code versions / remote setups. If the iframe is blocked, fall back to a webview
`portMapping`, or open the preview in an external browser tab (`vscode.env.openExternal`) —
inverse search still works there via the `vscode://file…` deep links the client already emits.

## Scope

The preview stays a **read-only view**: the extension navigates and highlights, and never
writes back to the source. Editor commands that transform the `.tmd` buffer are in scope;
preview gestures that mutate the document are not (a drag-to-reorder feature was removed for
breaking exactly this). See the repo `CLAUDE.md`, "The `.tmd` file is the single editing
surface".

## Automated verification (three layers)

1. **Unit + grammar (`npm test`)** — `node:test` for `ports.ts` (free-port pick, HTTP wait),
   `paths.ts` (sourcepos parse, source-file mapping, `isSourceFile`), **`manifest.test.ts`**
   (the no-drift gate: the default binary path must be the name cargo builds; every config
   key, command id and menu `when` clause the source uses must be one the manifest declares;
   every contributed command must actually be registered; every keybinding and menu entry
   must point at a contributed command; and **the subcommand the language client launches
   must be a real one** — that last rule exists because the client starts `taliesin lsp`
   from `ServerOptions` rather than through `spawn(`, so the older scan stopped covering the
   single command the whole companion now rests on), and **`grammar.test.ts`**:
   an offline `vscode-textmate` + `vscode-oniguruma` tokenization gate that loads the `.tmd`
   grammar + injection and asserts token scopes for every delta (cells embed their language,
   `#|` options are directives, `{=html}` is not a cell, math/div/shortcode/xref/cite scopes,
   `bob@rem-x` is not a ref, `$`/`@` inside a cell stay code). It reads the bundled base
   grammars from a downloaded VS Code build (`node scripts/ensure-vscode.cjs` fetches it in CI;
   locally it's already present). No Extension Host launch. This is the `editor-vscode` CI gate.
2. **Relay bridge (`node scripts/relay-harness.cjs`)** — serves the real `relayHtml` with a
   same-origin stub iframe so a browser can drive both message directions against the actual
   code (see the script header). Verified: `tali-goto` from the iframe reaches the host;
   `tali-cursor` from the host reaches the iframe.
3. **Extension Host (`npm run test:e2e`)** — `@vscode/test-electron` downloads a throwaway
   VS Code and runs `src/e2e/` inside the real Extension Host. This is where the language
   features are actually proved: a unit test can show the server answers, but only a real
   host shows VS Code asked it and rendered the reply. It asserts every contributed command
   registers, that a `.tmd` resolves to the `taliesin` language, that *Open Preview* opens a
   webview panel, and — all via the language server — that diagnostics arrive located and
   named, that cell options / div classes / math commands / paths / shortcode names /
   cell-option values complete (and that math does **not** complete in prose), that an
   `{{< include >}}` path is painted as a document link and named on hover, and that
   renaming a cross-reference anchor rewrites its definition and every reference.
   It needs the locally-built `target/debug/taliesin`. The runner clears
   `ELECTRON_RUN_AS_NODE` (set in some sandboxes) and passes `--no-sandbox` so VS Code
   launches headless.

What those three **don't** cover — and what the F5 checklist above is for — is the final
visual round-trip *through the live preview iframe*: cursor-move → the block actually
highlights, `Ctrl+Alt+J` → the preview scrolls, and Ctrl-click → the editor cursor lands. Layers 2+3 verify each side of
that bridge independently; F5 confirms them end-to-end in a real editor.
