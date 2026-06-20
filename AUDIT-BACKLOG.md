# qmd-fast — full audit backlog (2026-06-20)

A complete audit of the project: every Rust + JS/CSS source file deep-read and
**adversarially verified** by a second reviewer, every feature exercised live in
the browser, every corpus project built and its output validated.

## Health summary

The tool is **fundamentally sound**: 201 tests pass, `clippy` + `cargo fmt` clean,
**0 critical** issues, nothing broken at the foundation. Verified working end to end:
single-doc preview (incremental block swap, Alt-click click-to-source, located
errors, CSS hot-swap, diagnostics, math/highlight/citations), the whole deck engine
(fragments, code-stepping, magic-move, auto-animate, overview/minimap/`/`-filter,
blackout, drawing, print, speaker), and the multi-page site (nav, Cmd-K search,
listings, cross-page hot reload). Output is self-contained and offline (decks ship
**0 CDN refs**, KaTeX/highlight inlined), feeds/sitemaps/OG-meta valid.

**Findings: 106 confirmed** (0 critical · 3 high · 28 medium · 57 low · 18 nit),
plus build/live findings below. Priorities P0–P3 group them by what to fix first.

---

## P0 — fix first (correctness that bites real input, + security)

### Render / authoring correctness
- **`:::` inside a code fence is silently deleted.** The fenced-div preprocessor
  isn't code-fence-aware, so `::: {...}` lines inside a ```` ``` ```` block are
  blanked. Reproduced: a code block showing `::: {.callout-note}` renders with those
  lines *gone*. **Hits your own docs**, which document `:::` syntax in code blocks.
  → `crates/core/src/render/divs.rs:33` (preprocess) + `:89` (scan_div_spans): track
  fenced-code state in both so `:::` isn't treated as a fence inside code.
- **Cell-produced tables aren't captioned/numbered/anchored.** `#| label: tbl-x` +
  `#| tbl-cap:` is handled for figures (`fig-`) and listings (`lst-`) but **not
  tables** — so the caption is dropped, no "Table N", no `#tbl-x` id, and `@tbl-x`
  cross-refs break. Corpus hit: `corpus/tech-blog/posts/Kruskal-Wallis-test`.
  → `crates/core/src/render/mod.rs:236-242`: add a `tbl-` branch mirroring figures.
- **A `code-line-numbers` block right after a `. . .` pause becomes permanently
  hidden.** `add_fragment_class` adds `.fragment` to the `<pre>`, but `fragsOf`
  treats it as code-only and never adds `qmd-frag-visible`, so it stays
  `visibility:hidden` for the whole talk. (Interaction between the pause + code-step
  features.) → `reveal.rs:383` skip `.fragment` on `pre[data-code-lines]`, **or**
  `deck.js` fragsOf reveal it.
- **`format:` mis-detection.** Any value containing `revealjs` (a theme name, a CSS
  filename) makes an HTML doc render as a deck — it's a substring scan over nested
  values. → `mod.rs:751,761`: match the format *key*, not any substring (or parse the
  `format` block with serde_yaml, already done at `:598`).
- **Reorder-toward-front diff emits Insert-before-Remove of the same id**, so the
  client's first-match `querySelector` deletes the wrong (newly inserted) element.
  → `crates/core/src/diff.rs:64-94`: emit all Removes before Inserts (or guarantee a
  Remove of id X precedes any Insert of id X); + client dup-id defense
  (`web-client/client.js:486`).

### Execution
- **Kernel recovery skips re-execution.** A cell run while the kernel was down still
  records `ran`, so when the kernel self-heals the cell is *not* re-run → stale/missing
  output. → `crates/server/src/exec.rs:347-354`: only record `ran` `if has_kernel`.
- **`#| cache: false` cells are reused** from the warm in-memory prefix, contradicting
  the documented "always re-executes". → `exec.rs:449-460`: cap the warm `shared`
  prefix at the first non-cacheable cell.

### Security
- **Path traversal (arbitrary file read).** Include / theme / format-resource paths
  have no containment — absolute paths and `../` escape the base dir. → `includes.rs:55`,
  `render/mod.rs:684` (read_include_file), `theme.rs:146`: reject absolute / leading-`..`
  after lexical normalize; require the result stays under the doc/extension root.
- **Kernel connection file world-readable** (HMAC key + ZMQ ports) in the shared temp
  dir. → `kernel.rs:210`: 0700 dir / 0600 file, or `$XDG_RUNTIME_DIR`.
- **Attribute injection** — values text-escaped but injected into double-quoted attrs:
  `{{< embed >}}` title (`extension/mod.rs:618`) and figure `alt` (double-escaped,
  `figure.rs:84`). Also `add_fragment_class` / `emit_html_block` split the opening tag
  at the first `>` even inside a quoted attribute (`reveal.rs:383`, `emit.rs:339`).
  → use `escape_attr` and a quote-aware tag-end scan.
- **Deck `postMessage` accepts any origin and posts to `'*'`** — a third-party page
  embedding the deck can drive it (and read its slide position). → `deck.js:926`: gate
  on `e.origin === location.origin` (allow `file:`); target `location.origin`.

---

## P1 — correctness & robustness (medium)

### Site dev server (preview ≠ build, staleness)
- **Live preview skips `validate_xrefs` + `decorate_post`** that build runs, so broken
  cross-refs and reading-time/category badges never appear in preview. → `site/mod.rs:333`
  vs `serve_site.rs:328`: share one `finish_blocks(page, blocks, &mut warnings)`.
- **Site re-discovered only on `_quarto.yml` change** — new/renamed pages, edited
  titles/dates/listings never refresh live. → `serve_site.rs:948`: re-run `Site::discover`
  on any `.qmd` add/remove/rename (debounced).
- **`_quarto.yml` change reloads tabs but serves stale cached block state** →
  `serve_site.rs:957`: clear `app.pages` (or re-queue builds) before broadcasting reload.
- **Site watcher has no relevance filter** — every fs event (incl. `_freeze/` writes,
  `.git`) triggers a full per-page dependency rescan. → add the relevance filter +
  `_freeze` to SKIP_DIRS (also `serve.rs:828` for single-doc: a freeze write triggers a
  redundant rebuild). → `serve_site.rs:904`.
- **Synthesized listing blocks can collide on `data-block-id`**, breaking the diff.
  → `site/mod.rs:1114`: thread the listing index into the id.

### Citations / cross-refs
- **`[@fig-x]` (bracketed cross-ref) mis-parsed as a citation** → wrong link + spurious
  broken-citation warning. → `cite.rs:575`: detect xref-prefixed keys before treating as
  a citation.
- **Citation-key grammar omits `.`/`+`** that BibTeX keys allow → truncated keys + false
  warnings. → `cite.rs:642` vs `:236`: share one `is_cite_key_char`.
- **Cross-ref section numbers diverge from rendered numbers when a chapter uses
  `{{< include >}}`** → `site/xref.rs:25`: build the registry from include-resolved source.

### Reveal / slides
- **Per-slide `background`/`auto-animate` on an h3+ heading is silently dropped**
  (attrs only hoist from slide-level headings). → `reveal.rs:332`: strip them from
  non-lead headings or warn.

### Misc
- **TOC emits invalid `<ul>`-in-`<ul>`** when heading levels are skipped → `mod.rs:1195`.
- **Bare-string nav/footer items silently dropped** from config sequences →
  `site/config/mod.rs:255`.
- **RSS `pubDate` dropped for any non-zero-padded ISO date** → `feed.rs:96`.
- **Quarto `open-graph: image:` ignored** → site loses its default social card →
  `config/quarto.rs:20` + `meta.rs`.
- **Port 0 surfaces a broken URL** (returns requested addr, not the bound `local_addr`)
  → `serve.rs:182`.

---

## P2 — polish / minor robustness (notable low)

- **Build emits no warnings to stderr** — broken refs/citations/front-matter only
  surface in preview's dev menu, so a broken site deploys silently. Add a build-time
  warning summary. (Also: single-doc `build` doesn't run `validate_xrefs` — `cite.rs:504`.)
- **Prev/next post nav broken on the tech-blog** — your `post-nav.js` fetches Quarto's
  `/listings.json`, which qmd-fast doesn't emit (404). Emit a compatible `listings.json`,
  or ship native blog prev/next.
- **Canonical/`og:url` for index pages use `.../index.html`** instead of the clean
  directory URL → `site/meta.rs:21`.
- **Mermaid loads from a CDN** (violates the offline goal) and a load failure silently
  wedges rendering → `assets/js/mermaid.js:38`. Bundle it.
- **Captioned `.r-stretch` image overflows the slide** on decks → `deck.css:105`.
- **`<html lang>` interpolated unescaped** in both page shells → `reveal.rs:74`, `page.rs:110`.
- **`js_str` doesn't escape `</script>`/newlines** in embedded paths (both servers) →
  `serve.rs:480`, `serve_site.rs:421`.
- **Visited pages never evicted from `app.pages`** — unbounded block-state growth →
  `serve_site.rs:38`.
- **setext-heading `{#id}` not stripped/applied** (leaks literal text, breaks the anchor)
  → `mod.rs:974`.
- **`strip_trailing_hardbreak` is an unanchored global replace** that can edit non-trailing
  content → `mod.rs:1080`.
- **Include resolver isn't fence-aware** (a `{{< include >}}` inside a code block is
  resolved) → `includes.rs:53`.
- **`is_uncacheable` substring-matches `qmd-error`** → a successful cell whose output
  mentions that string is never cached → `exec.rs:465`.
- **Non-ASCII-only category names slugify to empty** → colliding `/categories//` pages →
  `site/mod.rs:817`.
- **Speaker clock `setInterval` never cleared**; **End key lands on v=0 of the last stack**;
  **`onHashChange` doesn't `broadcastState`** (desyncs speaker) → `deck.js:988/1249/1188`.
- **`mirror_assets` / `find_file_named` can infinite-recurse on a symlink cycle** →
  `main.rs:443`, `serve.rs:361`.
- **`updateWordCount` deep-clones all of `#qmd-root` on every op** (perf) →
  `client.js:49`.
- `@dataset`/`@online`/`inbook` drop carried fields; malformed author values leak;
  empty venue/year doubles punctuation (`cite.rs:45/355/86`). `decorate_post` injects
  meta into a `hero:`/`about:` header (`site/mod.rs:340`). Footer treats any `.xml` as
  the feed (`chrome.rs:140`). `--out` greedily consumes a following flag (`main.rs:49`).
  Combined content+theme edit drops the hot-swap until reload (`serve.rs:918`). Initial
  synchronous render isn't panic-guarded (`serve.rs:108`). Query-string asset refs aren't
  bundled (`main.rs:481`). `yaml_error` off-by-one past EOF (`frontmatter.rs:144`).

---

## P3 — test gaps & docs

### Missing test coverage (add regression tests)
- HTML string-surgery helpers: attr injection, hardbreak strip, link/heading attrs,
  `add_fragment_class` (`render/mod.rs:1024`, `reveal.rs`). `:::`-in-code-fence,
  figure-alt escaping, raw-HTML attr inject (`render/tests.rs`).
- Kernel-died-mid-run / no-kernel `ran` path, `cache:false` reuse (`exec.rs`).
- Asset path-traversal guard, `percent_decode`, `code_frame` bounds (`serve.rs:330`).
- `dispatch_changes` dep tracking, config re-discovery, mounts (`serve_site.rs`).
- CLI helpers `local_refs`/`is_local_ref`/arg parsing (`main.rs` — no tests yet).
- pause/fragment + background hoisting beyond the happy path (`reveal.rs`).

### Docs staleness (the dogfooded books)
- **`render/reveal.rs` still called "reveal.js slide assembly"** (reveal.js was removed)
  → `docs/internals/architecture.qmd:193`; stale "reveal.js" wording in the guide lead +
  two mermaid diagrams (`docs/guide/index.qmd:8`).
- Docs reference **`render/extension.rs`** but it's now a directory module →
  `docs/internals/rendering.qmd:30`.
- Block-model protocol table **omits the `style` (theme hot-swap) message** →
  `docs/internals/block-model.qmd:99`.
- `extending.qmd` describes declarative shortcodes only via the Quarto-shaped key;
  `theme.rs:46` comments claim an OS-following "auto" theme the impl removed.

### Nits (trivial)
Diagnostic with missing message renders `[object Object]` (`client.js:98`). Search
palette can flash stale results on rapid re-open (`search.js:104`). `rfc822` accepts
Feb 30 (`feed.rs:101`). reduced-motion forces `iteration-count:1` globally; no `@page`
print margins; `color-scheme:dark` not reset for print (`base.css`,`dark.css`).
`QMD_FAST_OPEN=0` still enables it (`main.rs:51`). `qmd` skip is case-sensitive
(`main.rs:460`). Dead `qmd-dark-bg` removal in deck.js; menu arrows bypass scroll guard.
Swapped `search_json`/`feed_xml` doc comments (`serve_site.rs:194`). `is_pause`
over-matches an emphasized `. . .` (`reveal.rs:377`). Figure `width=` injects raw CSS
(`figure.rs:80`).

---

## Suggested order of attack
1. **P0 security** (path containment, kernel-file perms, attribute escaping, postMessage
   origin) — small, bounded, and the only class with an external-trust dimension.
2. **P0 correctness** — `:::`-in-code-fence and cell-`tbl-` (both corpus-is-spec hits),
   then the pause+code-step hidden block, format mis-detection, the reorder diff + kernel
   `ran` recovery.
3. **P1 site preview/build parity** (`finish_blocks`, re-discovery, watcher filter) — these
   make the dev loop trustworthy for sites/books.
4. **P2/P3** opportunistically; the test-gap list is the cheapest insurance against
   regressions while you fix the above.
