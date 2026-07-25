# Taliesin Companion (Phase 1)

Hosts the Taliesin live preview in a VS Code webview with **bidirectional source sync**.
It is the missing *producer* for the source-sync protocol the preview client
(`web-client/client.js`) already consumes:

- **Forward (preview → editor):** Alt-click a block in the preview → the editor jumps to
  that block's source line (`tali-goto`).
- **Reverse (editor → preview):** move the cursor in the `.tmd` → the matching block
  highlights and scrolls into view in the preview, and the deck jumps to the right slide
  (`tali-cursor` → `highlightAtLine`).

The preview stays **read-only**: the extension only navigates and highlights; it never
writes back to the source.

## Syntax highlighting (the `.tmd` grammar)

The extension also **owns editor syntax highlighting for `.tmd`** via a `taliesin` language
(`contributes.languages` / `contributes.grammars`). The grammar is a thin superset: it
`include`s VS Code's built-in **MIT** `text.html.markdown` for all CommonMark, then adds only
the Taliesin deltas — braced exec cells (` ```{python} `/`{r}`/`{js}`/…, with real embedded
inner-language color and `#|`/`//|`/`%%|` cell options scoped as directives), `:::` fenced divs
+ `{.class #id key=val}` attrs, `$…$`/`$$…$$` math (with `{#eq-…}` labels), `{{< shortcodes >}}`,
`@fig-`/`@sec-`/… cross-refs, `[@cite]` citations, and the deck `. . .` pause. Leading `---` YAML
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

## Completions and snippets

Everything the editor offers is **Rust-authoritative**, so it cannot drift from what `check`
enforces. Front-matter keys, cell options, callout kinds, div classes and theorem kinds come
from `taliesin vocab`. Cross-reference targets come from `taliesin symbols`, which reads the
resolved xref registry: that is how `@`-completion finds a figure labelled from a cell
(`#| label: fig-scree`), which no regex over the source can see. Since `symbols` reads the file
on disk, the results are merged with a live scan of the buffer, so an anchor you typed a moment
ago is completable before you save. Citation keys are read from the front matter's
`bibliography:`.

`contributes.snippets` ships a small set of `.tmd` snippets (`fm`, `cell`, `figcell`, `jscell`,
`foldcell`, `callout`, `fig`, `tabset`, `thm`, `include`, `video`, `input`). Their bodies are
gated against the same vocabulary by `src/test/manifest.test.ts`: a snippet that offers a callout
kind or cell option Taliesin no longer knows fails the build, and the callout snippet's choice
list must equal `vocab.calloutKinds` exactly, in order.

## Develop / run

1. `cd editor/vscode && npm install && npm run build`
2. Open the **`editor/vscode`** folder in VS Code and press **F5** ("Run Taliesin
   Companion"). A second *Extension Development Host* window opens with the extension
   loaded. (Run `npm run build` again after any source change, then reload the host.)
3. Ensure the `taliesin` binary is on `PATH` (the launcher), or set the `taliesin.path` setting
   to the binary (e.g. `target/release/taliesin`). To exercise the grammar, open any `.tmd`
   file (e.g. `corpus/native-tmd.tmd`) — the status bar should read **Taliesin**.

## Manual verification (the loop the headless tests can't close)

A VS Code extension runs in the Extension Development Host, which can't be driven
headlessly — so this checklist is run by hand. In the Extension Development Host window,
opened on the Taliesin repo:

1. Open `corpus/posts/em-algorithm/index.tmd`. Run **Taliesin: Open Preview** (command
   palette, or the editor-title button). The preview opens beside the editor and renders.
2. **Reverse sync:** move the cursor onto a heading / paragraph — the matching block in the
   preview gains the `.tali-hl` outline and scrolls into view.
3. **Forward sync:** Alt-click a block in the preview — the editor cursor jumps to that
   block's source line.
4. **Deck:** open `corpus/deck.tmd`, Open Preview, move the cursor
   into a later slide's content — the deck jumps to that slide.
5. Close the preview panel — the spawned `taliesin preview` process exits (no orphan).
6. Open a `.tmd` with a front-matter typo (or any `taliesin check` finding): a yellow squiggle appears on the offending line, refreshing on save.
7. Autocomplete fires inside front matter, after `#|` in a code cell, after `:::{.`, after `@`, and inside `[@ ]`, offering keys, cell options, callout/theorem/div classes, cross-reference prefixes, and citation keys from `taliesin vocab`.

## Known risk

`vscode.env.asExternalUri` localhost-iframing inside a webview CSP can behave differently
across VS Code versions / remote setups. If the iframe is blocked, fall back to a webview
`portMapping`, or open the preview in an external browser tab (`vscode.env.openExternal`) —
forward sync still works there via the `vscode://file…` deep links the client already emits.

## Scope

Phase 1 = host + cursor loop, localhost only (no `--host`, so backlog #1d's LAN token isn't
needed yet). Phase 2 (editor commands like insert-block / reorder-slide, strictly as
`.tmd`-buffer text transforms) is deferred. See
`docs/superpowers/specs/2026-06-24-vscode-editor-companion-design.md`.

## Automated verification (three layers)

1. **Unit + grammar (`npm test`)** — `node:test` for `ports.ts` (free-port pick, HTTP wait),
   `paths.ts` (sourcepos parse, source-file mapping, `isSourceFile`), **`manifest.test.ts`** (the
   no-drift gate: the default binary path must be the name cargo builds, and every config key,
   command id and menu `when` clause the source uses must be one the manifest declares), and
   **`grammar.test.ts`**:
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
   VS Code and runs `src/e2e/` inside the real Extension Host: asserts the command is
   registered, that the **`taliesin` language is contributed and a `.tmd` file resolves to it**,
   and that *Open Preview* actually opens a webview panel (which spawns the server). The preview
   test needs the locally-built `target/debug/taliesin`. The runner clears `ELECTRON_RUN_AS_NODE`
   (set in some sandboxes) and passes `--no-sandbox` so VS Code launches headless.

What those three **don't** cover — and what the F5 checklist above is for — is the final
visual round-trip *through the live preview iframe*: cursor-move → the block actually
highlights/scrolls, and Alt-click → the editor cursor lands. Layers 2+3 verify each side of
that bridge independently; F5 confirms them end-to-end in a real editor.
