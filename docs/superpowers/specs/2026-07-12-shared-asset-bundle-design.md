# Shared asset bundle for `build <dir>` — design (#17)

Date: 2026-07-12
Backlog: Section B item #17 ([notes/backlog.md](../../../notes/backlog.md)).
Branch: `shared-asset-bundle` (built after `publish-hardening` merges).

## Problem

Every page a site build emits inlines the **full framework CSS + KaTeX fonts + JS**. The
CSS (`fonts + base + dark + site`, ~95 KB) and the base64 KaTeX fonts (~347 KB) are
**byte-identical on every page** — on a 712 KB post they are ~64% of the bytes, re-sent in
full for every page a reader opens. This is the biggest repeat-visit perf loss in the built
output.

The fix: emit each shared blob **once** as a content-hashed file under the output tree, and
link it from every page instead of inlining. One download, cached across the whole site.

`build file.tmd` (single self-contained file), `--bare`, and live `preview` must stay
**fully inlined** — a single file has to work over `file://`, and preview relies on inline
CSS hot-swap. Only the multi-page `build <dir>` path changes.

## Decisions already ruled (this brainstorm)

- **JS: fully split, all shared.** Separate content-hashed shared files, each linked only on
  pages that use it (not one superset on every page).
- **Minify: conservative hand-rolled.** No `lightningcss` (decided-against). A small
  string/comment-aware minifier we own and test.
- **KaTeX fonts stay base64-inlined** inside the external `katex.<hash>.css` (still one
  cached file; avoids separate font files + CORS/relative-path issues).

## File layout

Written once to `<out>/_assets/` — **underscore-prefixed** so it (a) cannot clobber a user's
own `assets/` content dir and (b) is framework-owned + swept correctly.

| File | Contents | Minified? | Linked on |
|---|---|---|---|
| `_assets/app.<hash>.css` | `FONTS_CSS + BASE_CSS + DARK_CSS + SITE_CSS` (site-constant) | yes (CSS) | every page |
| `_assets/katex.<hash>.css` | `KATEX_CSS` (base64 fonts kept inside) | yes (CSS) | pages with math |
| `_assets/app.<hash>.js` | all *our own* JS: code-enhance stack + walkthrough + tabset + scrolly + toc-spy + toc-sheet + search + static-enhance | yes (JS, conservative) | every page |
| `_assets/mermaid.<hash>.js` | vendored `mermaid.min.js` | no (already minified) | pages with mermaid |
| `_assets/jslibs.<hash>.js` | vendored `d3.min.js` + `plot.umd.min.js` | no (already minified) | pages with `{js}` cells |

Small conditional scripts of *our own* (walkthrough/tabset/scrolly, **and the `qmd-js.js`
`{js}`-cell enhancer** — distinct from the vendored d3/plot libs it drives) fold into the
always-on `app.js` — they are tiny and already no-op without their target DOM, so
proliferating one hashed file per feature buys nothing. Only the **big vendored libs**
(mermaid, d3, plot) get their own conditional files.

**Load order.** The vendored libs currently sit in `<head>` (d3/plot via `js_cell_head`);
the enhancers run at end-of-body. Externalized, `mermaid.<hash>.js` / `jslibs.<hash>.js`
keep their `<head>` position and `app.<hash>.js` stays at end-of-body, all with `defer` so
they execute in document order — the vendored globals are guaranteed present before
`app.js`'s enhancer looks for `{js}`/mermaid cells. (Today's inline scripts run in this same
order; `defer` preserves it.)

`<hash>` = short hex (first 12 chars of a SHA-256 over the file's final bytes). Content hash
⇒ any byte change ⇒ new filename ⇒ automatic cache-bust; no manual versioning.

## Architecture — thread hrefs through core, don't regex the HTML

The single page-assembly choke point is `assemble_html_page` in
[render/page.rs:125](../../../crates/core/src/render/page.rs#L125). It already decides every
conditional asset: `ship_katex` (a `PageParts` flag), `has_js_cells(body)` (the d3+Plot
head), and `code_scripts_for(body, mode)` (mermaid / qmd-js / walkthrough / tabset / scrolly
from body content). **Keep those conditions as the single source of truth** — the build
supplies href strings, core keeps deciding *which* to emit.

### Core changes (`crates/core`)

1. **Expose the shared blobs** (accessors in `render/mod.rs`, next to the existing
   `include_str!` consts) so the build can compute their bytes + hashes without duplicating
   the concatenation:
   - `pub fn shared_site_css() -> String` → `FONTS_CSS + BASE_CSS + DARK_CSS + SITE_CSS`
     (exactly what a site page inlines: not bare, site chrome on).
   - `pub fn katex_css() -> &'static str` → `KATEX_CSS`.
   - `pub fn core_enhance_js() -> String` → all of our own JS: the code-enhance stack +
     walkthrough + tabset + scrolly + the `qmd-js.js` `{js}` enhancer + toc-spy + toc-sheet +
     search + static-enhance. That is, everything `code_scripts_for` (+ toc/search) emits for
     a site build **minus** the two big vendored libs (mermaid, d3/plot). (Refactor
     `code_scripts_for` so this core slice and the two lib slices are nameable — see below.)
   - `pub fn mermaid_lib_js() -> &'static str`, `pub fn js_cell_libs_js() -> String`
     (d3 + plot, the current `js_cell_head` payload minus the `<script>` wrapper).

2. **Refactor `code_scripts_for`** into composable pieces so both the inline path and the
   external path share the exact same bytes + conditions:
   - `core_scripts()` — always-on, our own code.
   - a `mermaid` piece, gated on `has_mermaid(body)`.
   - a `jslibs` piece, gated on `has_js_cells(body)`.
   The inline path concatenates the ones a page uses (unchanged output); the external path
   emits `<script src>` for the same ones.

3. **`PageParts` gains an asset mode:**
   ```rust
   pub enum AssetMode<'a> {
       Inline,                     // Preview, single-file Build, Bare — today's behavior
       External(ExternalAssets<'a>),
   }
   pub struct ExternalAssets<'a> {
       pub app_css: &'a str,       // depth-adjusted href, e.g. "../_assets/app.abcd…​.css"
       pub katex_css: &'a str,     // href; emitted only when ship_katex
       pub app_js: &'a str,        // href; always
       pub mermaid_js: &'a str,    // href; emitted only when body has mermaid
       pub jslibs_js: &'a str,     // href; emitted only when body has {js} cells
   }
   ```
   In `assemble_html_page`, `External` replaces the inlined `<style>{fonts}{base}{dark}{site}</style>`
   with `<link rel="stylesheet" href="{app_css}">`, the `{katex}` inline block with a gated
   `<link>`, and the inline `<script>`s with gated `<script src="…" defer>` — using the same
   `ship_katex` / `has_mermaid` / `has_js_cells` conditions the inline path uses.
   `theme_css` (extension theme, per-call) stays inline for v1 (small, site-level; a possible
   later extension).

Core's inline path stays raw + unminified (portability + preview hot-swap unchanged). Core
does **not** depend on the minifier — minification is a build-time concern (below).

### Build changes (`crates/server/src/build.rs`)

In the site build (`build_site_async` / around `build_one_page` + the writer choke point at
[build.rs:839-850](../../../crates/server/src/build.rs#L839)):

1. **Before rendering pages**, compute the bundle once from the core accessors:
   - `app_css = minify_css(shared_site_css())`
   - `katex_css = minify_css(katex_css())`
   - `app_js = minify_js(core_enhance_js())`
   - `mermaid_js = mermaid_lib_js()` (pass-through, already min)
   - `jslibs_js = js_cell_libs_js()` (pass-through, already min)
   Hash each → `_assets/<name>.<hash>.<ext>`; write all five once under `<out>/_assets/`.
2. **Per page**, compute `asset_prefix` from `page.url` depth (count `/` in the rel url;
   `index.html` → `_assets/…`, `using/x.html` → `../_assets/…` — the same depth-rebasing the
   link rewriter already does), build `ExternalAssets` with the prefixed hrefs, and render in
   `External` mode.
3. **Stale sweep:** clear `<out>/_assets/` at the start of a site build (or reconcile it) so
   a prior build's hashes don't accumulate. `_assets/` is underscore-prefixed so it isn't
   caught by the general content sweep; own its lifecycle explicitly.

The single-file `build file.tmd` path and `--bare` keep calling core in `Inline` mode — no
`_assets/`, fully self-contained. Live preview is untouched.

### Minifier — new `crates/server/src/minify.rs` (build-time only)

- `minify_css(&str) -> String`: a state machine tracking `"`/`'` strings and `/* */`
  comments. Strip comments; collapse runs of whitespace to one space; drop whitespace
  around structural punctuation (`{ } : ; , >`) only outside strings. Must be safe on
  `url(data:image/…)`, `content:"  "`, and the base64 KaTeX blob.
- `minify_js(&str) -> String`: **ultra-conservative, only ever runs on our own hand-written
  JS.** Track strings / template literals / regex / comments; strip `/* */` + `//` comments
  and blank lines + leading indentation; **preserve every remaining newline** (so automatic
  semicolon insertion is untouched); no token/identifier mangling. Vendored `*.min.js`
  bypass it entirely.
- Tests: known-input assertions; guards that string/url/regex content survives; a guard that
  a deliberately ASI-sensitive snippet is byte-preserved across line breaks.

## Invariants preserved

- **Body HTML unchanged** — only the `<head>`/script wrapper differs, so `data-block-id`,
  `data-sourcepos`, `data-source-file`, incremental block-swap, and the corpus invariants are
  untouched.
- No CDN — every file is local under `_assets/`, content-hashed.
- No preview write-back; no new output format; `--tali-*` tokens only.
- Single-file build + `--bare` + preview: byte-for-byte the same as today (Inline mode).

**Correctness test:** render one representative doc in `Inline` and `External` mode; assert
the `External` head references files whose (minified) bytes equal the (minified) inlined
blobs, and that both bodies are identical.

## Pin (scope policy — a corpus-anchored assertion)

Extend the site-build tests (`tests/tech_blog.rs` or a new `build`-level test) building
`corpus/tech-blog`:

- `_assets/app.<hash>.css` and `_assets/app.<hash>.js` exist, non-empty.
- No emitted page contains an inlined framework `<style>` block (assert the base-CSS
  sentinel is absent from page HTML, present in the shared file).
- Two different pages reference the **same** `app.<hash>.css` filename (dedup proven).
- A math page links `katex.<hash>.css`; a prose-only page does not.
- A diagram page links `mermaid.<hash>.js`; a prose-only page does not.
- Nested page hrefs carry the correct `../` depth prefix.

## Edge cases / out of scope

- **Mounts:** each built project writes its own `<out>/<mount>/_assets/`; cross-mount dedup
  (docs books sharing the marketing site's bundle) is out of scope for v1 — correct, with
  bounded duplication across mount roots.
- **`theme_css`** (extension theme) stays inline in v1.
- Minification of vendored libs is intentionally skipped (already minified).

## Non-goals

Not touched: the exec/kernel zone, the deck engine's own `deck.css`/`deck.js` bundling
(decks are single-doc/self-contained), the single-file build's self-containment.
