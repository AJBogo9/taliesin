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

## P0 — RESOLVED (2026-06-21)

All P0 items below were fixed and verified (201+ tests pass, clippy/fmt clean, the
matplotlib + tbl + `:::`-in-fence behaviours browser-verified; the Kruskal-Wallis
corpus post now emits `Table 1`). Details live in git. For the record, the items were:
`:::` inside a code fence silently deleted; cell-produced tables not
captioned/numbered/anchored (`tbl-`); a `code-line-numbers` block after a `. . .`
pause staying hidden; `format:` substring mis-detection; reorder diff emitting
Insert-before-Remove; kernel-recovery recording `ran` with no live kernel; `#| cache:
false` cells reused from the warm prefix; path traversal in include/theme/bib reads;
the world-readable kernel connection file; attribute-injection points; and the deck's
open-origin `postMessage`.

(Also shipped alongside: **theme-matched matplotlib figures** — the kernel now emits a
light + a dark variant of each plot and the page swaps them on a `data-theme` change,
replacing the single washed-out grey render.)

---

## P1 — RESOLVED (2026-06-21)

All P1 items below were fixed and verified (tests pass, clippy/fmt clean; the
cite/xref/TOC fixes checked in rendered output; the site-preview parity checked live —
a post now shows its reading-time + category badges + executed table in preview, and
the watcher no longer rebuilds on `_freeze/` writes). Details in git. The items were:

- **Site dev server:** preview now shares one `finish_blocks` with the build (so
  `validate_xrefs` + `decorate_post` run in preview); the site re-discovers when a
  `.qmd` is added/removed *and the page set actually changes*; a `_quarto.yml` / page-set
  change clears cached block state before reloading tabs; the watcher gained a relevance
  filter (and `_freeze` is in SKIP_DIRS for both servers); synthesized listing blocks
  carry the listing index in their id.
- **Citations / cross-refs:** `[@fig-x]` renders as a cross-ref (not a citation); the
  citation-key grammar accepts `.`/`+`/`/`; the xref registry is built from
  include-resolved source so section numbers match.
- **Reveal:** a `background`/`auto-animate` on a non-lead (h3+) heading is stripped
  rather than left as an inert `data-*` attribute.
- **Misc:** TOC inserts a filler `<li>` so skipped heading levels stay valid; bare-string
  nav/footer items aren't dropped; RSS `pubDate` tolerates non-zero-padded dates; the
  default social card comes from `image:` / Quarto `open-graph: image:`; port 0 reports
  the OS-assigned port.

---

## P2 — polish / minor robustness (notable low)

### Done (2026-06-21)
Build now prints render warnings **and broken cross-refs** to stderr (site + single
doc), so a broken site no longer deploys silently. The site emits a Quarto-compatible
`listings.json` (build + preview route) — tech-blog prev/next works again, with real
titles (the corpus `post-nav.js` was updated to read qmd-fast's compact `search.json`).
Canonical/`og:url` use the clean directory URL; `<html lang>` and `js_str` are escaped
(`</script>`/newlines); a setext-heading `{#id}` is applied + stripped;
`strip_trailing_hardbreak` is end-anchored (no longer corrupts raw-HTML content); the
include resolver skips `{{< include >}}` inside a code fence; `is_uncacheable` matches
the emitted `class="qmd-error"` (not bare text); non-ASCII category names get a real
(or hashed-fallback) slug; a captioned `.r-stretch` figure no longer overflows the
slide; `mirror_assets`/`find_file_named` guard against symlink cycles; deck End key
lands on the last vertical of the last stack, `onHashChange` re-broadcasts to the
speaker, and the speaker clock interval can't double-register.

Mermaid: load-failure no longer wedges (clears the loading flag + leaves source
visible). **Full offline bundling deferred** — it means vendoring ~2.8 MB of
mermaid; wants a decision before growing the repo that much.

### Remaining
Also fixed 2026-06-21: `@dataset`/`@online` now keep their publisher/organization
(corpus: the Kaggle dataset); empty venue/year no longer dangles a comma before the
period; malformed authors that format to nothing are dropped; the footer only maps a
*local* `.xml` to the feed (an external `.xml` URL is left alone); `--out` won't
swallow a following flag as the directory.

Still open:
- **Visited pages never evicted from `app.pages`** — unbounded block-state growth →
  `serve_site.rs`.
- **`updateWordCount` deep-clones all of `#qmd-root` on every op** (perf) →
  `client.js`.
- `@inbook`/`@incollection` drop `booktitle`/pages (no corpus entry yet).
  `decorate_post` injects meta into a `hero:`/`about:` header (`site/mod.rs`).
  Combined content+theme edit drops the hot-swap until reload (`serve.rs`). Initial
  synchronous render isn't panic-guarded (`serve.rs`). Query-string asset refs aren't
  bundled (`main.rs`). `yaml_error` off-by-one past EOF (`frontmatter.rs`).

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

### Docs staleness (the dogfooded books) — DONE 2026-06-21
Fixed: the "reveal.js" wording in the guide lead + the two mermaid diagram nodes +
`architecture.qmd`'s `render/reveal.rs` row; `render/extension.rs` → `render/extension/`
in `rendering.qmd` + `extending.qmd`; the block-model protocol table now lists the
`style` (theme hot-swap) message; `extending.qmd` notes the native `shortcodes:` key;
`theme.rs`'s `theme_default_mode` comment no longer claims an OS-following "auto".

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
