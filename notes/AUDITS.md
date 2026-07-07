# Taliesin audit records

Consolidated historical audit reports, newest first. The active backlog
(open items only) lives in [backlog.md](backlog.md); these are the detailed
findings behind it, kept for reference.

-----------------------------------------------------------------------------

# Corpus fidelity sweep vs Quarto (2026-06-25)

Systematized output-fidelity check (backlog "highest-value #4"). The sibling
`qmd-fast-testbed` gained `sweep_corpus.py`: render every real corpus single-doc
(the spec) in **both** Taliesin and Quarto with execution disabled, reduce to the
block skeleton, structural-diff, and catalog. The full classification lives in
`qmd-fast-testbed/CORPUS-FINDINGS.md`; the generated report is `corpus_sweep.md`.

**Result: 3/8 exact match** (callouts/kinds, narrate/walkthrough, posts/born-machines);
the other 5 differ only in items below, all classified. A `run.py` normalization fix
(code-token whitespace was inventing `np.polyfit` → `np . polyfit` noise) also lifted
the existing conformance suite 16/27 → 18/27.

**Deliberate (Taliesin intentionally different):** native `{js}` cells render as live
widgets not dumped OJS source; flat `div.qmd-layout` vs Quarto's
`quarto-layout-panel/row/cell`; references in a semantic `section.qmd-references` vs
`div.references`; display-math in `div.qmd-math` (KaTeX server-side, nothing dropped)
vs Quarto's `<p>\[…\]</p>`; tabset's no-JS form (`h2` + stacked panels, upgraded to
ARIA tabs by client JS).

**Taliesin does better:** resolves `@fig-`/`@sec-` cross-refs even without executing
the target cell (Quarto leaves `?@fig-…` under `--no-execute`).

**Real bug-candidates (REPORTED, follow-ups — not yet fixed):**
- **`#|` option lines leak into displayed code. FIXED (2026-06-25).** Root cause was *not*
  the originally-prescribed emit-path strip (`#|` already strips fine, 123 corpus uses + a
  test): `posts/pca-geometry` writes the options with a space (`# | label:`, `# | echo:
  false`), and every option parser only matched `#|` (no space). So the spaced lines were
  neither stripped (→ leaked into source) nor parsed (→ `echo: false` ignored, source shown;
  `label: fig-data-3d` unregistered, throwing figure numbers off by one vs Quarto). Quarto
  accepts the spaced form, so Taliesin now does too: a single `option_directive()` primitive
  (`render/mod.rs`) tolerates optional whitespace between the comment marker and `|`, and
  `cell_option` / `strip_cell_options` / `validate::cell_option_keys` all key off it.
  Pinned by `render::tests::spaced_option_directives_are_recognized`.
- **Captioned code listing is not a `<figure>`.** Taliesin emits `div.qmd-listing` with a
  `<figcaption>` (a `<figcaption>` outside `<figure>` is out of place); Quarto uses
  `<figure class="quarto-float-lst">`. Minor/semantic (`render/figure.rs`/`emit.rs`).

-----------------------------------------------------------------------------

# Taliesin — round-2 adversarial audit (2026-06-21)

A second pass after the round-1 backlog (P0–P3) was fixed. Method: **empirical** (a
battery of ~80 hostile/extreme documents actually run through `build`/`preview`, with
panic/segfault/hang/timing capture and output inspection) plus three parallel
adversarial code-reads of the render, execution/server, and site/client layers. None
of these overlap the round-1 `AUDIT-BACKLOG.md` items.

**Status legend:** `CONFIRMED` = reproduced live in this session; `REPORTED` =
high-confidence static finding (tied to specific code), not yet run.

---

## RESOLVED (2026-06-21)

All findings below were fixed and verified, then a 6-agent verification+completeness
workflow re-attacked the patched code and surfaced 16 follow-ups (0 high) — those were
triaged and the real ones fixed in the same batch. 202 tests pass, clippy + fmt clean.

- **#1 output hang:** per-cell output caps (512 KB stream / 4096 items) + interrupt the
  kernel on cap. A 5 MB `print` went **70 s → 1.7 s**. Truncated output is marked and
  kept out of the `_freeze` cache (no stale-truncated replay).
- **#2 in-place build data loss:** `build` refuses when the output dir == source dir;
  `copy_resources` also skips self-copies. Verified: asset preserved.
- **#3 Quarto config wipe:** the whole `website:` block is now read field-by-field
  (lenient), so a bare-string nav item — or any one wrong-typed field — no longer drops
  title/url/favicon/footer. Verified.
- **#4 mount search 404:** mounts now route `search.json` + `feed.xml`. Verified 200.
- **#5 deep-nesting abort:** render runs on a 256 MB-stack worker (falls back to inline
  if the spawn fails under `ulimit -v`). 3000/10000 blockquotes + 5000-deep lists build.
- **#6 duplicate labels:** warn + first-wins (`@x` now agrees with the anchor), in-doc
  and cross-page (deduped). The analogous **duplicate `.bib` key** now warns too.
- **#7 front-matter terminator:** FALSE POSITIVE (code uses `trim_end`, i.e. column-0;
  verified an indented `...` does not truncate). Not changed.
- **#8 category slug collision / #9 listing id-target / #10 comma in category:** fixed +
  verified (merged archives; `tag_end`-based leading-tag match; filter reads badges).
- **#11 shell-reply desync:** the execute_reply drain is now parent-matched.

### Known residuals (deliberately not fully closed; low risk for this tool's scope)
- **Single >~tens-of-MB cell output** still blocks during the ZMQ receive (one giant
  message blocks an uninterruptible read before the cap can fire). Realistic streamed
  output is fixed; a single 50 MB `print` is bounded only by completion / the cell
  timeout. Fully fixing needs frame-level capping in the jupyter/zmq layer.
- **Stack overflow** at absurd nesting (hundreds of thousands deep) still aborts: the
  big stack raises the ceiling ~32×, but a hard depth cap (or iterative emit) was not
  added — comrak overflows during parse, before a post-parse cap could run, and a
  source-level depth heuristic risks false-positives on legitimate docs.
- **Duplicate cross-ref labels** still emit two identical HTML `id=` attributes (the
  number/anchor *disagreement* and the missing warning are fixed; the invalid-but-now-
  consistent duplicate id is not deduped).
- **Mounted sub-sites** don't route category-archive pages or embedded decks, and a
  mount miss serves a bare asset 404 rather than the styled 404 (the shipping mounts are
  books, which have neither).
- **Conflicting shortcode names** across two active extensions silently last-wins
  (author-controlled; rare).

---

## HIGH

### 1. A large cell output hangs the build/preview (super-linear in output size) — CONFIRMED
A `{python}` cell that prints a moderately large string wedges the renderer:
`print('x'*1MB)` → 1.6 s, `print('x'*5MB)` → **>60 s (timed out)**, 20 MB → timed out.
That is catastrophically super-linear (likely O(n²) copying: the output string is
cloned into the output block, the `_freeze` cache, and `state.ran`, plus HTML-escaped
and diffed). There is no cap on accumulated cell output (`kernel.rs` `execute` →
`render_outputs` → `exec.rs`). This is the **most likely real-world failure for a power
user**: `print(big_df)`, a verbose training log, or a large array silently hangs the
preview. → cap per-cell output bytes (truncate with a marker) and avoid the repeated
full-size clones. (`crates/server/src/kernel.rs` execute/render_outputs, `exec.rs`.)

### 2. Building with the output dir == source dir destroys source assets (data loss) — CONFIRMED
`output: "."` in `_quarto.yml` (or `build <dir> --out <same dir>`) makes `mirror_assets`
`fs::copy(p, dest)` every asset onto itself; `fs::copy` opens the dest `O_TRUNC` first,
so each file is truncated to 0 bytes. Reproduced: `logo.png` 34 → **0 bytes**. The
single-doc path has a `same_file` guard; the site path does not. → error out when
`out.canonicalize() == root.canonicalize()`, and skip self-copies in `mirror_assets`.
(`crates/server/src/main.rs` `build_site`/`mirror_assets`.)

### 3. One bare-string nav item silently wipes the entire Quarto `website:` config — CONFIRMED
A Quarto-shaped `_quarto.yml` with `navbar: left: [Home, …]` (bare strings — extremely
common in real Quarto configs) fails to deserialize `NavItem` (it wants a map), and
because the shim deserializes the whole `website:` block atomically and falls back to
`Website::default()` on any error, the site loses its **title, url, description,
favicon, footer, and navbar**. Reproduced: title absent from output. The native config
path handles bare strings; only the Quarto-compat path is fragile. → deserialize
nav/footer leniently (per-item, like the native `nav_item`). (`crates/core/src/site/config/quarto.rs` `section`/`Website`.)

### 4. Cmd-K search is dead inside any mounted sub-site (`mounts:`) — CONFIRMED
A mounted book page emits `QMD_SEARCH_URL="search.json"`, which resolves to
`/<mount>/search.json` → **404** (only the parent's `/search.json` route exists).
Reproduced on the shipping `site/` (mounts `/docs/guide` + `/docs/internals`): the
mounted page returns 200 but `GET /docs/guide/search.json` is 404. Search silently
returns nothing on every mounted-book page. → add a per-mount search route (and feed),
or point `QMD_SEARCH_URL` at the parent. (`crates/server/src/serve_site.rs` mount
handler + `site/mod.rs` chrome.)

---

## HIGH-ish (crash)

### 5. Deeply nested blockquotes/lists abort the process via stack overflow — CONFIRMED
A document with ~3000 nested `>` blockquotes (or ~3000-deep nested list) overflows the
stack and the process aborts (`fatal runtime error: stack overflow`, exit 134 / core
dump). Depth 1500 is fine, 3000 is not; nested `:::` divs do **not** overflow (they're
preprocessed iteratively). The recursion is in the parse/emit of the deep AST. A single
pathological (or pasted) document crashes `build` outright, or crashes the **preview
server** mid-session. → cap nesting depth (reject/flatten beyond N) or render
iteratively; at minimum catch and report instead of aborting. (`crates/core/src/render/emit.rs` recursive emit; comrak parse.)

---

## MEDIUM

### 6. Duplicate cross-reference labels resolve silently to the wrong target — CONFIRMED (single-doc) / REPORTED (cross-page)
Two figures both labelled `{#fig-dup}` produce **two elements with `id="fig-dup"`**
(invalid HTML) and `@fig-dup` renders "Figure 2" while the `#fig-dup` anchor points at
Figure 1 — the number and the link destination disagree, with no warning. Cross-page,
`scan_xref_targets` uses `entry().or_insert()`, so the first page defining an anchor
wins project-wide and later redefinitions are silently ignored. → warn on duplicate
labels; dedupe ids. (`crates/core/src/render/mod.rs` xref registry, `site/xref.rs:43`.)

### 7. `front_matter_block` ends early on a `---`/`...` line inside a multi-line value — REPORTED
The terminator scan stops at the first line trimming to `---`/`...`, so a YAML block
scalar (e.g. a `description: |` whose body contains a `...` line) truncates the front
matter for the linter, the site parser, and `parse_extensions` (which disagree with
comrak's own parse) → later keys (`theme:`, `categories:`, …) are silently lost.
(`crates/core/src/frontmatter.rs:154`.)

### 8. Colliding category slugs overwrite each other's archive pages — REPORTED
Archive URLs key on `slugify(name)` but the index keys on the raw name, so two names
that slugify identically (`"Machine Learning"` vs `"machine-learning"`) write the same
`categories/<slug>/index.html` (second overwrites first) and posts under the losing
name link to the wrong archive. No warning. (`crates/core/src/site/mod.rs`
category_pages/render_category_page.)

### 9. Listing `id:` target matches the first substring occurrence of `id="x"` — REPORTED
`listing: { id: x }` finds its placeholder with `b.html.contains("id=\"x\"")`, so a
code sample or any block that merely *contains that text* (the docs books show HTML)
captures the cards instead of the intended `::: {#x}`. (`crates/core/src/site/mod.rs` `expand_page`.)

### 10. A category name containing a comma breaks the client-side filter — REPORTED
Card categories are serialized comma-joined into `data-categories` and the filter splits
on `,`, so a category literally named `"A, B"` becomes two tokens and its own chip never
matches → the card is hidden whenever a filter is active. (`site/mod.rs` +
`assets/js/code-enhance.js`.)

### 11. Cell output / stale-reply robustness — REPORTED
A cell that ignores SIGINT past the timeout can leave the shell channel one
`execute_reply` behind (the shell drain isn't parent-matched), desyncing later cells
until a manual Restart. (`crates/server/src/kernel.rs:493/552`.)

---

## LOW (selected; full list in the agent transcripts)

- **Citations rewritten inside rendered math:** the cite/xref rewriter's skip-list
  omits KaTeX spans, so `$\text{@fig-x}$` becomes a link inside the equation. (`cite.rs` SKIP.)
- **Quadratic citation scan** on a run of unmatched `[` (e.g. ASCII art): O(n²) per
  render, i.e. per keystroke in preview. (`cite.rs` rewrite_text.)
- **`execute: {echo: false}` (YAML flow form) ignored** — only the block form is parsed,
  so cells echo despite the front matter. (`render/mod.rs` detect_execute_defaults.)
- **First-visit TOCTOU**: the HTTP GET and the websocket open both queue a build for a
  fresh page → duplicate first-paint builds. (`serve_site.rs` ensure_and_render_page.)
- **RSS `<link>`/`<guid>` use `…/index.html`** while canonical/`og:url` use the clean
  directory URL → reader/analytics mismatch. (`feed.rs` vs `meta.rs`.)
- **XML escaping omits C0 control chars** → a title with `\x0c` yields an invalid feed. (`feed.rs` xml.)
- **`window.QMD_PAGE_URL` interpolates `page.url` unescaped** into a JS string (other
  script values go through `json_str`). (`site/mod.rs`.)
- **No mount validation:** an empty `at:` shadows the parent; `path:` can point outside
  the root (only `is_dir` checked). (`config/mod.rs` mounts_from.)
- **Single-doc watcher** misses an out-of-tree `{{< include >}}` added after startup.
  (`serve.rs` watch_dirs.)
- **`force_next` leaks** when a Restart happens on a doc with no code cells. (`exec.rs`.)
- **Blocking `<prog> --version`** runs on a tokio worker (no timeout). (`exec.rs` interp_id.)
- **`code-line-numbers=` substring-matched** anywhere in a fence info string. (`emit.rs`.)
- **Shortcode template `{{n}}` substitution** is order-dependent / can double-expand. (`extension/mod.rs`.)
- **`insert` op with an unresolved `after_id`** prepends the block to the top. (`web-client/client.js`.)

---

## What held up under attack (the negative space)
Attribute/`<html lang>`/title/`js_str` escaping all held against injection payloads
(`"`, `>`, `</script>`, `</title>` were correctly escaped — the round-1 fixes work).
Kernel **crash** handling is solid: `os._exit`, a segfault (`ctypes.string_at(0)`), and a
raised exception all recover gracefully (the kernel respawns, downstream cells re-run);
an infinite loop is SIGINT'd at the timeout and the doc continues. Unicode headings
(emoji / RTL / combining / zero-width / CJK) slug and dedupe correctly with no
non-char-boundary panic. Malformed math degrades to error blocks. Huge flat documents
(8000 paragraphs, 3000-row table) render in <0.2 s — no quadratic there. The diff and
the cumulative-hash execution plan are sound under reorder/insert/delete/`cache:false`.


-----------------------------------------------------------------------------

# Taliesin — full audit backlog (2026-06-20)

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
titles (the corpus `post-nav.js` was updated to read Taliesin's compact `search.json`).
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
`TALIESIN_OPEN=0` still enables it (`main.rs:51`). `qmd` skip is case-sensitive
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


-----------------------------------------------------------------------------

# Visual & UX audit (2026-06-19)

In-browser audit (chrome-devtools) of all six public-facing surfaces, captured at
desktop + mobile, light + dark, with console + theme checks. Lens: this is about
*showing the project* and winning over **Quarto/Jupyter switchers**, not selling.

## Summary

The four format demos and the docs are in genuinely strong shape; the wow is real
and unfaked. Issues cluster on the **marketing site's hero pages**, which the
planned demo-machine rebuild replaces anyway. **Every console was clean** across all
six surfaces.

| Surface | Verdict | Notes |
|---|---|---|
| Slide deck (`corpus/liquid-glass-slides`) | Top-tier wow | Frosted-glass title slide + per-bullet glass panels; a third-party reveal theme renders on Taliesin's own engine. Best single demo asset. |
| Blog post (`corpus/posts/em-algorithm`) | Strong | 90 KaTeX spans, executed Python with dark-mode-aware matplotlib, collapsible code folds, callouts. |
| Multi-page website (`corpus/tech-blog`) | Strong | Navbar, `about:` header, listing cards w/ thumbnails + tags, RSS, footer. Mobile wraps correctly. |
| Docs book (`docs/`) | Strong | Numbered Mermaid figures, component tables, section numbering, sub-TOC. Internals reads as a credibility asset. |
| Multi-chapter book (`corpus/demo-book`) | Solid | Parts, numbering, prev/next, sidebar. Same shell as docs (consistent engine). |
| Marketing site (`site/`) | Needs the planned rebuild | Text-led not demo-led; mobile overflow; theme/video desync; passive top CTA. |

## Bugs (fix regardless of redesign)

1. **[High] Mobile prose overflow** on the marketing hero pages (`page-layout: full`
   + `hero:`). The intro paragraph clips off the right edge at 390px. Isolated to
   those pages: the shared site chrome wraps fine (verified on tech-blog). Contained
   CSS fix in the full-layout body container.
2. **[Med] Theme / video desync.** Site chrome uses a manual toggle (defaults dark,
   ignores OS `prefers-color-scheme`); the `{{< video >}}` light/dark swap follows
   the OS media query. OS-light => light video inside a dark page. Drive the video
   variant off the site toggle, not the media query.
3. **[Low] Mermaid diagrams** run a little low-contrast (grey-on-dark) in the
   internals book.
4. **[Low] Em dashes** throughout the marketing copy (against the author's writing
   rule; also worth tightening for the leaner demo voice).

## Strategic direction (decided this session)

**Two separate docs books + a demo-machine website.**

- **User Guide** = `docs/using/` + `docs/reference/` (how to *use* the tool; the
  adoption funnel for switchers).
- **Internals** = `docs/internals/` (how it's *built*; a public credibility piece,
  written as explanation).
- **Website** = a demo machine: lead with motion above the fold, one crisp value
  line on top, a vs-Quarto table (reuse the one already in the docs index), a real
  install/quickstart on-ramp into the User Guide. Cap embedded slides at one hero
  deck per page.

**The two-book split is cheap:** only 2 cross-links to fix (both
`internals/ -> using/`, zero the other way), no shared `@sec-` cross-refs, numbering
just restarts per book.


---

# Deep audit — 2026-06-30 (16-dimension, 33-agent, adversarially verified)

_131 findings survived verification (128 confirmed, 3 uncertain): 11 high, 28 medium, 92 low. Method: 16 harsh-critic dimension agents in 4 waves -> a fresh skeptic re-read the code to refute/dedupe each finding -> synthesis. Full machine-readable findings + per-dimension detail captured in the workflow result this session._

Both confirmed: README.md:8 says "Double-click" while build.rs:86 has the silent `s if s.starts_with("--") => {}` catch-all. The audit data is accurate. Here is the report.

---

# Taliesin Deep Audit — Synthesis Report

## 1. Executive verdict

Taliesin is a genuinely well-engineered personal tool: the execution/freeze/kernel zone, the block-diff core (LIS reduction, Remove-before-Insert sort), the security path-resolution, and the CSS craft in dark mode are all above the bar for a solo project, and the load-bearing invariants survive most hostile reads. But it is not yet *polished*, and three themes recur across nearly every dimension. **First, "silent failure is the default" is a doctrine, not an accident** — listings, social cards, nav items, theme hot-swaps, misspelled CLI flags, broken `_site.yml`, and math render errors all degrade with no diagnostic, which is precisely the trust-eroding behavior a live-preview tool can least afford. **Second, accessibility is advertised but shallow** — a real focus trap and ARIA tabs coexist with a flagship lightbox that is mouse-only (WCAG 2.1.1 fail), an entire deck whose off-camera slides stay in the AT tree and tab order, and a static a11y gate (3 rules) that cannot see any of it, so a green check over-vouches. **Third, the JS behavior layer and the executed-code stack are untested in CI** — every kernel test no-ops when `TALIESIN_PYTHON` is unset (which it is on CI), and client.js plus all of `assets/js/` has no standing type-check or browser test, so the two most behavior-rich subsystems can regress fully green. What separates this from genuinely polished is unglamorous follow-through: surface the silent failures, finish the a11y/sepia work that was started-but-abandoned, close the panic-guard asymmetry between `preview` and `build`/`check`, and fix the README's headline gesture (it says "double-click"; the code is Alt-click — the #1 feature fails on a new user's first attempt).

## 2. Top 10 highest-leverage fixes

| # | Title | Sev | Dimension | Why it matters |
|---|-------|-----|-----------|----------------|
| 1 | README says "double-click", code is Alt-click | high | Docs | One-line fix; the marquee feature silently fails on the first thing a new user reads |
| 2 | `build`/`check` misspelled flags silently dropped (`--stict`) | high | DX | A typo silently disables the `--strict` CI gate → broken pages ship green; cheap did-you-mean fix |
| 3 | `build`/`check`/`render` lack preview's panic guard | high | Robustness | A doc that previews safely hard-crashes on `build`/`check`; factor the existing guard into a shared helper |
| 4 | Kernel that dies mid-cell hangs for the full cell-timeout | high | Execution | One unused `is_alive()` probe turns a 120s freeze + mislabel into a ms-fast clear error |
| 5 | Lightbox is mouse-only (WCAG 2.1.1 A fail) | high | A11y | Flagship reader feature unreachable by keyboard/SR; add tabindex+role+keydown on decorated media |
| 6 | Off-camera deck slides not hidden from AT / tab order | high | Deck/A11y | One `inert` sweep per commit fixes both the AT-tree and tab-order traps reveal.js scopes correctly |
| 7 | SetMeta leaves nested div sourcepos stale | high | Diff | Silently breaks click-to-source + reverse-sync inside every fenced div; fall through to full Update on multi-sourcepos blocks |
| 8 | Bare `@`-xref fires mid-word (`bob@rem-server.com`) | high | Citations | Corrupts real prose + emits unsuppressable phantom diagnostics; gate on a word boundary |
| 9 | Kernel/exec tests silently no-op in CI | high | Test gap | The entire execution stack has zero CI verification; add one `pip install ipykernel` job |
| 10 | Malformed `_site.yml` → silent default config, exit 0 under `--strict` | medium | Robustness | A YAML typo ships a config-less site green; count it as a `--strict` problem + keep last-good in preview |

## 3. Findings by theme

### Robustness / panics / silent failure

- **`build`/`check`/`render` lack the panic guard `preview` has** — high — `build.rs:294`, `check.rs:48`, `query.rs:26,70`. A doc that previews safely (guarded by `catch_unwind` at `serve/mod.rs:117`) hard-crashes with a raw backtrace + non-zero exit on the batch commands. *Fix:* factor the preview guard into a shared render-guarded helper and route all four through it.
- **Kernel that dies mid-cell hangs the full cell-timeout** — high — `kernel.rs:671-709`, `exec.rs:565`. The crashing cell's `iopub.read()` blocks to the cap deadline, then mislabels a crash as "Timeout". `kernel_alive()` (`exec.rs:506`) only guards *subsequent* cells. *Fix:* probe `self.proc.is_alive()` on read timeout inside the execute loop; break with `Error{ename:"KernelDied"}`.
- **Malformed `_site.yml` silently falls back to default config, not a `--strict` problem** — medium — `config/mod.rs:122-128`, `build.rs:727-734`, `serve_site/mod.rs:937-941`. A YAML typo drops *all* config; the warning is only `log::warn`'d, so `build --strict` exits 0 and ships a config-less site. Preview replaces site state with defaults on every save of a temporarily-broken config. *Fix:* feed the parse error into the `problems` count; keep last-good config in the watcher.
- **Site-build page-task panic logged but exits 0 even under `--strict`** — medium — `build.rs:852-873`. The `JoinError` arm only `log::error`s; `problems` never increments, so a panicked (missing) page ships with broken nav and green CI. *Fix:* increment `problems` on `JoinError`.
- **preview `render_doc` returns `None` on read/decode failure, indistinguishable from "no change"** — low — `serve/mod.rs:276-282`. A non-UTF8/IO read leaves a stale/blank view with no diagnostic. *Fix:* surface a "could not read/decode" diagnostic instead of `.ok()?`.
- **math.rs cache full-clears on overflow** — low — `math.rs:23-44`. Crossing 8192 distinct expressions wipes the whole process-global cache, re-rendering every expression on the next save. *(Also surfaced under Performance — same root cause as the KaTeX-cache finding; merge.)* *Fix:* evict a fraction / use a bounded LRU.
- **Warm-pool chdir error silently runs kernel in the wrong directory** — low — `exec.rs:792-807`. Only the transport-`Err` arm warns; a Python exception inside `os.chdir` is discarded, so figures/`ggsave`/audio land in the wrong place. *Fix:* inspect the chdir outputs for an error, or chdir at fork time.
- **Forkserver footguns** *(cluster, all low)* — `warm_pool.rs`: a post-`SPAWNED` init failure burns the 15s adopt timeout *and stops the refill loop* (`93-104,478-487`); a `FORK_TIMEOUT` leaves an unread reply that cross-wires the next fork (`253-271`); the daemon's child-failure message goes to `Stdio::null()` (`193`). *Fix:* tag fork requests with a monotonic id; pipe+drain daemon stderr.
- **Freeze temp file uses a fixed `.json.tmp` suffix** — low — `freeze.rs:192-201`. A preview server + a CLI build on the same page race the shared temp path. *Fix:* unique per-write suffix (pid+counter).
- **execute() shell-reply drain caps at 5s after a hard interrupt** — *uncertain*, low — `kernel.rs:791-809`. May leave the shell channel one reply behind for a pathologically slow interrupted cell (bounded by the "Restart kernel" escape hatch).

### Block diff / incremental updates (live-state invariant)

- **SetMeta patches only the outer element's sourcepos, leaving nested div sourcepos stale** — high — `diff.rs:83-119`, `client.js:934-947`. After a line-shifting edit above a fenced div, Alt-click on inner content and reverse cursor-sync both go to the wrong line — directly breaking the load-bearing click-to-source invariant, silently. *Fix:* when a block's html has >1 `data-sourcepos`, fall through to a full Update; add a corpus test.
- **Deleting the first of two identical blocks destroys the SECOND block's live state** — medium — `mod.rs:1474-1478`, `diff.rs:165-205`. Positional `-N` tiebreak means the survivor re-hashes to the base id; LCS removes the second's DOM node (tearing down `{js}`/canvas/`<details>`/video state). *Fix:* add a stable structural discriminator to the duplicate tiebreak, or at minimum pin the behavior in a test.
- **Every block op re-runs the full `afterChange()` suite over the whole document — O(ops × doc)** — medium — `client.js:845-853` (per-op at 898/921/931), `serve/mod.rs:1065` (one ws message per op). An N-op structural edit near the top of a large doc runs `buildToc`/`scanA11y`/`updateWordCount`/etc N times. *(Broader than the backlog's `updateWordCount`-clone note; see Performance.)* *Fix:* batch ops into one message and run `afterChange()` once, or rAF-coalesce.
- **Gaps between anchors paired positionally with no inner LCS** — low — `diff.rs:123-153`. An edit+insert in one save can reassign a derived/keyed block's live DOM to the wrong block. *Fix:* document the tradeoff, or skip positional pairing when an old id recurs unchanged in the gap.
- **Combined deck-structural + theme save drops the theme hot-swap** — low — `[dup]` — `serve/mod.rs:1059-1074`, `serve_site/mod.rs:799-810`. Same root cause as the backlog's "combined content+theme edit drops hot-swap"; the one-line fix (move `if theme_changed { send style }` outside the else) resolves all variants. *(See Dev-server theme finding below — merge.)*
- **`insert` after_id resolving to null silently prepends to doc top** — *uncertain*, low — `diff.rs:147-152`, `client.js:914-918`. Real fragile path, unproven trigger. *Fix:* fall back to last-inserted position + `console.warn` instead of `root.prepend`.
- **Inconsistent programmatic scroll feel (instant vs smooth)** — low — `client.js:823` vs `1141/525/140`. Defensible-but-uncentralized; one helper with an explicit policy.

### Accessibility (the recurring "advertised but shallow" theme)

- **Lightbox is mouse-only (WCAG 2.1.1 A fail)** — high — `11-lightbox.js:124-137`. Open is bound only to a delegated mouse click; the keydown handler early-returns unless already open. *Fix:* `tabindex=0`+`role=button`+`aria-label`+Enter/Space keydown on decorated media.
- **Deck off-camera slides never hidden from AT** — high — `deck.js` (~252-261, 1204-1224), `deck.rs:343`. A screen reader reads the whole deck as one document. *Fix:* set `aria-hidden`/`inert` on every non-current leaf section per commit.
- **Deck keyboard focus walks into invisible off-screen slides** — high — `deck.js:570-576`. No `inert`/tabindex sweep of non-current slides. *Fix:* same `inert` pass as above (covers both AT and tab order).
- **Global single-key shortcuts (f, /, ?, arrows) have no off/remap (WCAG 2.1.4)** — medium — `07-keyboard.js:55-80`, `03-focus-mode.js:52-62`. Speech/motor users can't disable them; focus-mode also omits `SELECT` from its typing guard (inconsistent with keyboard.js). *Fix:* a Reader-menu opt-out flag honored by both; add `SELECT` to the focus-mode guard.
- **Reader-menu `role="dialog"` never moves focus in** — medium — `13-reader-menu.js:31-40`. Dialog announced while focus stays on the launcher. *Fix:* move focus to the panel on open, or drop `role=dialog` for a disclosure shape.
- **Primary reader controls below 24×24 (WCAG 2.5.8)** — medium — `base.css:130-131`, `342-348`. Read-aloud transport buttons + per-heading anchor lack an enforced minimum target. *Fix:* `min-width/height:24px` + expanded hit padding.
- **Static a11y gate ships 3 rules, blind to `role=button` and all JS behavior** — medium — `a11y.rs:106-152`. `interactives()` matches only literal `<a `/`<button`. *Fix:* match `[role=button|link|tab]`; longer-term wire `scanA11y` into a headless gate.
- **Deck LOD title card duplicates each heading into the AT tree** — low — `deck.js:303-317`. `opacity:0`, no `aria-hidden`. *Fix:* `aria-hidden="true"` on `.qmd-lod` + decorative minimap/threads.
- **Read-aloud code-stepping has no aria-live announcement** — low — `05-read-aloud.js:274-285`. *Fix:* announce active line / "line N of M" through the polite region.
- **Listing card image always `alt=""` with no author knob** — low — `mod.rs:694-700`. Asymmetric with `about:`'s `image-alt`. *Fix:* support per-page `image-alt:`.
- **Slider `<output>` not associated (`for=`); controls fall back to raw name as label** — low — `extension/mod.rs:613,679-690`.
- **Scrolly narrative conveys active step visually only** — low — `scrolly.js:19-25`. *Fix:* `aria-current` on the active step + optional live region.
- **Link-preview hover card has no keyboard/focus trigger; missing `aria-describedby`** — low — `12-link-preview.js:24-27,89-104`.
- **Selection toolbar keyboard-inaccessible despite `role=toolbar`** — low — `16-highlights.js:88-89,179,194-225`. *Fix:* roving-tabindex + arrow nav + an `h` shortcut.

### Visual craft / theming (the "started-but-abandoned sepia" theme)

- **Sepia inherits the entire light syntax palette, copy button, and output/error boxes** — high — `base.css:32-38,385-404,599-610`. Sepia is a first-class reader theme but only redefines 6 tokens; GitHub-light code colors + a white copy button sit on warm paper. *Fix:* add `html[data-theme="sepia"]` overrides mirroring `dark.css`, or re-point chrome at `var(--qmd-*)` so it follows any theme.
- **Sepia `--qmd-muted` (#7a6a55) fails AA (4.44:1 / 4.05:1)** — medium — `base.css:33`. Drives captions, footnotes, TOC, sidenotes, chips. *Fix:* darken to ~#6b5a44 and verify against bg + code-bg.
- **Body prose has no paragraph/list/hr spacing rules** — low — `base.css` (none; only scoped `.footnotes hr`). Falls back to UA defaults in a stylesheet advertising "intentional vertical rhythm"; bare `<hr>` shows the beveled UA line. *Fix:* tokenized `p`/list margins + a flat `hr`.
- **Copy button uses hardcoded light hex instead of tokens** — low — `base.css:398-404`. Forces a per-theme override sepia never gets. *(Same root cause as the sepia copy-button symptom above.)*
- **Sepia syntax comments lose contrast (#6e7781 → 3.52:1); highlight tints added but not the syntax palette** — low — `base.css:111,120` vs `385-390`. *Fix:* an `html[data-theme="sepia"] .qhl-*` block with warm AA-passing hues.
- **Theorem `--qmd-thm-*` tokens have no dark override** — low — `base.css:16,469-474`. Tints adapt (color-mix) but the raw-token left borders don't. *Fix:* lightened dark variants in `dark.css`.
- **Overlay surfaces/shadows use hardcoded `rgba()` instead of `--qmd-edge-shadow`** — low — `base.css:255/259/293/696/707/714`. Black drop-shadows near-invisible on dark. *Fix:* route the *shadows* (not the scrim backdrops, which are correct-by-design) through the token.
- **Dead `.hero h1 { border:0; padding:0 }` reset** — low — `base.css:321-322`. Cancels nothing in any bundled stylesheet. *Fix:* drop it or comment why it stays.

### Deck engine (beyond a11y)

- **Resize re-fits every slide synchronously, no debounce** — medium — `deck.js:1536,242,534-549`. `fitSlide` does 4 reflow/write cycles × N slides per resize tick, though its own comment says it's viewport-independent. *Fix:* rAF/debounce; drop `fitSlide` from the resize path.
- **Speaker view loads two full deck iframes that re-execute all `{js}` cells** — medium — `deck.js:970-971`. Doubles live-widget CPU/memory on battery. *Fix:* render cur/next as innerHTML-snapshot clones, or make `?qmd=embed` skip `{js}` execution.
- **Fragment steps never update the URL hash** — low — `deck.js:514-528,1142-1165,1183`. Reload/share mid-build loses the step. *Fix:* encode `#/<h>/<v>/<frag>`.
- **Blackout overlay can trap an arrow-only clicker** — low — `deck.css:608`, `deck.js:1255-1258,1287-1304`. Keys swallowed + cursor hidden (mouse pointerdown does dismiss). *Fix:* treat any nav key as resume.
- **magic-move + code-line-numbers double-counted as fragment steps** — low — `deck.js:406-425`. *Fix:* skip `PRE` inside `.magic-move` in `fragsOf`.
- **Speaker window leaks a `setInterval` + stale `speakerWin` on close** — low — `deck.js:976-977,907-912`. *Fix:* `pagehide` handler.
- **Overview wheel heuristic misclassifies high-res mouse wheels as trackpad pans** — low — `deck.js:660`. *Fix:* prefer `ctrlKey`/`deltaMode` signals over a 100px gate.
- **Print/PDF drops per-slide backgrounds + clips overflow** — low (documented v1 limit) — `deck.css:372,517`.

### Multi-page site / books (the "correctness-of-omission" theme)

- **Root-level `contents: .` listing silently lists nothing** — medium — `mod.rs:627-636`, `links.rs:112-135`. `join_rel` yields `""` → prefix `"/"` → no page matches. *Fix:* special-case empty prefix to match siblings, or reject `contents: .`.
- **`listing:` without `contents:` silently dropped** — medium — `frontmatter.rs:137-140`. No diagnostic, inconsistent with closed-set `_site.yml` validation. *Fix:* thread a warnings channel; warn on a present-but-spec-less listing.
- **og:image/og:url/twitter:image/canonical silently suppressed unless `url:` set** — medium — `meta.rs:20,27-79`. Social cards vanish with no warning. *Fix:* warn when `image:` is set but `url:` is not; document the coupling.
- **Listing drops any post lacking `title:` with no warning** — low — `mod.rs:633-636,706`. The renderer already has a `rel` fallback the filter ignores. *Fix:* drop the `title.is_some()` filter or warn.
- **`mounts:` prefix colliding with a page path shadows the page, no warning** — low (preview-only) — `config/mod.rs:161-184`, `serve_site/mod.rs:267-293`.
- **Book `chapters:` entry pointing at a missing file added to sidebar, shifts numbering** — low — `book.rs:98-133`. *Fix:* warn when `!input.is_file()`.
- **Duplicate cross-page anchors resolve to the alphabetically-first page** — low — `xref.rs:45-69`, `discovery.rs:43`. Warning fires but the resolved link is arbitrary. *Fix:* prefer same-chapter; name both pages in the warning.
- **Hero/about headline+lead fully HTML-escaped (no inline markdown)** — low (design) — `mod.rs:746-835`. Inconsistent trust model vs raw-HTML footer items.
- **`check` cross-page link validation renders each page twice** — low — `mod.rs:382,400`. *(See Performance.)*

### Citations / math / bib (parser correctness)

- **Bare `@`-xref fires mid-word** — high — `cite/render.rs:219-229,277-299`. `bob@rem-server.com`, `@def-list` become xref links + phantom unsuppressable diagnostics. *Fix:* require a word boundary before the bare-`@` branch.
- **Quoted single-brace author misclassified as corporate** — medium — `cite/parse.rs:165-186` vs `140-164`. `author="{First Last}"` renders verbatim while `{First Last}` initializes. *Fix:* strip one brace level in the quote arm like the brace arm; add a regression test.
- **Math render failures produce no diagnostic** — medium — `math.rs:31-69`, callers `emit.rs:92`/`mod.rs:430,1600`. The only render path with no located warning or click-to-source. *Fix:* harvest the KaTeX error and thread a located Warning like the citation channel.
- **TOC/deck-slug from a math heading garbled by `strip_tags` over KaTeX** — low — `mod.rs:1408`, `deck.rs:291`. *Fix:* drop the katex-mathml/`<annotation>` subtree before collecting heading text.
- **`\url` removal is a naive global string replace** — low — `cite/clean.rs:11`. `\urlstyle{tt}`→`style{tt}`. *Fix:* require `\url{...}` and strip to its argument.
- **Bibliography path splits on spaces; duplicate-key warning un-located** — low — `mod.rs:754,770-771`, `parse.rs:91-96`. *Fix:* parse the YAML value as string/sequence; add `.at()` to the dup-key warning.
- **Citation-key char set narrower than the bib-key char set the parser accepts** — low — `render.rs:240-242` vs `parse.rs:58`. `[@smith&jones2020]` truncates + false-warns. *Fix:* pick one source of truth (the code's own comment demands they agree).
- **prose-lint: common weasel words + line-local doubled-word detection** — low — `prose.rs:15-29,182-198`. Flags `just`/`simply`; misses `the\nthe`. *Fix:* trim the list; carry `prev` across lines.
- **prose-lint `$`-math stripping swallows currency runs** — low — `prose.rs:123-137`. *Fix:* treat `$` as math only when not followed by a digit/space.
- **`nested_key_line` mislocates when a grandchild shares an unknown child's name** — low — `frontmatter.rs:203-223`. *Fix:* match only at the immediate-child indent.

### Render pipeline core

- **Explicit heading `{#id}` anchors never deduplicated → duplicate DOM ids** — medium — `render/mod.rs:407-417`. The explicit-id arm clones `id` verbatim and never consults `heading_slugs`; `#foo`/TOC/`@sec-` all resolve to the first match. *Fix:* route explicit ids through `dedup_with_suffix`, or warn-and-suffix on any duplicate.
- **Code-block language class interpolated unescaped** — low (security) — `emit.rs:60`, `cell_numbered.rs:89`. `{.a"x=y}` breaks out of the quoted attribute. *Fix:* `escape_attr` both sites.
- **`strip_tags` ignores quoted attribute values, truncating alt text** — low — `mod.rs:1537-1549` (used by `figure.rs:96`). A `>` inside a caption's attribute value leaks markup into alt text. *Fix:* make `strip_tags` quote-aware like the sibling `tag_end`.
- **`bare_math_env` accepts mismatched `\begin{a}...\end{b}`** — low — `mod.rs:1570-1573`. Hijacks prose into broken display math, no diagnostic. *Fix:* require matching env names + nothing trailing.
- **Figure/Table/Listing labels hardcoded English, ignore `lang:`** — low (design/i18n) — `figure.rs:100`, `cell_numbered.rs:17-21`, `mod.rs:1372`. *Fix:* a lang-keyed label table threaded into the emitters.
- **Per-block emission allocates a throwaway String per tag** — low (perf) — `emit.rs:16-18,21-23,274,278,330,332`. *Fix:* `write!`/direct `push_str`.

### Web client / dev server (robustness + the silent-stale-preview theme)

- **`ws.onmessage` parses JSON with no try/catch** — medium — `client.js:977`. A malformed frame or `handle()` throw aborts that message with no reconnect, silently halting updates. *Fix:* wrap `JSON.parse`+`handle()` in try/catch; surface `handle()` errors via the existing overlay.
- **Out-of-tree includes added after startup never watched** — medium — `serve/mod.rs:863-895`. The single-doc watch set is computed once and frozen; a new sibling-dir include silently stops hot-reload. *Fix:* recompute/diff `watch_dirs` after each rebuild, or warn on an out-of-tree include.
- **Theme hot-swap dropped on error-recovery + deck-structural re-mounts** — medium — `serve/mod.rs:1059-1074`, `serve_site/mod.rs:799-810`. `full_render` carries no CSS; theme silently stays stale. *(Merges the diff-dimension "combined deck-structural + theme" finding — one fix.)* `[partial dup]` of the backlog's combined-edit item. *Fix:* move `if theme_changed { send style }` out of the else branch in both files.
- **WebSocket reconnect: fixed 1s forever, no backoff/cap/parse-guard** — low — `client.js:971-980`. *Fix:* capped exponential backoff; try/catch around `connect()`.
- **WS URL hardcodes `ws://`** — low — `client.js:975`. Breaks under any TLS-fronted/`asExternalUri` origin. *Fix:* derive from `location.protocol`.
- **Two always-on timers** — low — `client.js:470-478` (cell-elapsed 200ms, never cleared), `search.js` build-twice. *Fix:* lazy start/stop mirroring `warmTimer`.
- **Cmd/Ctrl-K capture is unconditionally global** — low — `search.js:442-451`. Steals the shortcut from page inputs. *Fix:* skip when target is editable and palette closed.
- **`buildToc` emits invalid nesting on a heading-level skip (h1→h3)** — low — `client.js:661-684`. `<ul><ul>` with no `<li>`. *Fix:* wrap intermediate `<ul>` in `<li>`, or clamp descent.
- **`buildIndex()` called twice per search open** — low — `search.js:160,173-175`. *Fix:* build once, reuse.
- **idle `updateProgress` clobbers a title set by a later `full_render`** — low — `client.js:486,556,865`. Stale tab title after each build. *Fix:* track current doc title, restore to that.
- **`highlightAtLine` early-returns before clearing stale `.qmd-hl`** — low — `client.js:1129-1131`. *Fix:* clear before the early return.
- **Renamed/deleted open page leaks its warm kernel (≤6, LRU-bounded) + reloads to a bare 404** — low — `serve_site/mod.rs:985-992,231-323`. *Fix:* drop removed rels' executors; show a "removed/renamed" notice.
- **First page visit can build twice (concurrent-GET race)** — low — `serve_site/mod.rs:334-351`. *Fix:* gate `build_tx.send` on `Entry::Vacant` inside the lock.
- **Watcher debounce coalesces same-burst events into an extra rebuild** — low — `serve/mod.rs:922-924`. *Fix:* trailing-edge debounce.
- **Broadcast buffer overflow forces a full re-render mid-execution** — *uncertain*, low — `serve/mod.rs:101,745-749`.

### Bundled enhancer JS

- **`{js}` reactive runtime leaks `onInput` callbacks across re-mounts** — medium — `qmd-js.js:90-94,187-206`. `teardownIn()` never touches `r.listeners`, so stale closures fire forever (latent: public API only, no corpus cell exercises it). *Fix:* track `(name, cb)` pairs and delete them in `teardownIn`.
- **Escape/arrow keys fan out to 4-5 uncoordinated global keydown listeners** — low — `12-link-preview.js:104` (the only unguarded one) + lightbox/reader-menu/keyboard/focus-mode. No current visible bug; maintainability fragility. *Fix:* a shared Esc/overlay stack.
- **Focus-mode `f` typing-guard omits `SELECT`** — low — `03-focus-mode.js:54`. *(Same as the WCAG-2.1.4 finding's SELECT sub-claim — one fix.)*
- **Reader-local localStorage keys grow per-path, no eviction/quota recovery** — low — `16-highlights.js:13`, `18-bookmarks.js:12`, `15-reading-progress.js:80-85`. *Fix:* evict oldest `qmd-pos:` on QuotaExceeded.
- **Bookmark/highlight save flashes success even when persistence failed** — low — `18-bookmarks.js:80-85`, `16-highlights.js:180-193`. *Fix:* return the `setItem` success bool; revert + announce on false.
- **Read-aloud `<mark>` fallback mutates content DOM, can orphan marks** — low — `05-read-aloud.js:240-290` (fallback path only).
- **scrolly/walkthrough self-clean only on next scroll** — low — `scrolly.js:51-66`, `walkthrough.js:76-90`. Transient duplicate listeners. *Fix:* eager block-teardown hook.
- **Late-registered enhancers get no phase signal** — low (DX) — `01-registry.js:16-31`.

### Security (single-author trust model — all hardening, all low)

- **`--host` token persists in URL bar + history** — low — `security.rs:150-155`. Cookie already authenticates subsequent requests. *Fix:* `history.replaceState` to scrub `?t=` after mount.
- **`qmd_token` cookie not `HttpOnly`** — low — `security.rs:124`. Zero-downside (no first-party JS reads it). *Fix:* append `; HttpOnly`.
- **`--host` Mermaid loads from jsDelivr with no SRI / Referrer-Policy / CSP** — low — `mod.rs:858`, `mermaid.js:71-72`, `page.rs:150-151`. CDN tradeoff is documented; the missing SRI is net-new. *Fix:* `integrity`+`crossorigin`; emit `Referrer-Policy: no-referrer`.
- **`origin_allowed` trusts any loopback origin regardless of server Host** — low — `security.rs:13-24`. A loopback peer can drive the control ws cross-origin (worst case: kernel restart). *Fix:* only blanket-allow loopback when loopback-bound.
- **Deck postMessage treats `origin === 'null'/''` as same-origin on http(s)** — low — `deck.js:893,913-919`. *Fix:* gate the null allowance on `file://` only.
- **Asset serving has no content allowlist** — low — `serve/mod.rs:363-380`. Traversal guard is sound; `.qmd`/`_freeze`/`_site.yml` are web-served over `--host`. *Fix:* document the decision or add an allowlist.
- **Extension-resource fallback serves by bare name without re-checking containment after a symlink-following walk** — low — `serve/mod.rs:387-419`. *Fix:* re-apply `canonicalize`+`starts_with` on the candidate.
- **Figure/link URLs attribute-escaped but not scheme-validated** — low (informational) — `figure.rs:101`.
- **LAN token comparison is non-constant-time** — low (effectively unexploitable, 122-bit) — `security.rs:83,86`.

### Performance (none break invariants)

- **~163 KiB CSS+JS inlined uncompressed into every page, duplicated site-wide** — medium — `[dup]` (backlog "long tail") — `page.rs:155`, `mod.rs:902`. gzip alone is a 3.6× win; a 50-page site ships ~8 MB of byte-identical boilerplate. *Fix:* shared external `_site/assets/*` for multi-page builds; minify; precompress.
- **Full-document re-scan on every block op** — medium — `[dup]` — `client.js:845-853`, `serve/mod.rs:1065`. *(Same root cause as the diff-dimension `afterChange` finding — one fix: batch + coalesce.)*
- **Scrollspy forces synchronous layout per-heading on every op** — medium — `toc-spy.js:86`. *Fix:* coalesce in rAF; cache `offsetTop`; skip when no heading changed.
- **discover/check render each page redundantly** — medium — `search.rs:30` (1× on every preview/build start), `mod.rs:382,400` (2× check-only). *Fix:* merge the two `validate_cross_page_links` loops; make the discover-time search index lazy.
- **code-enhance.js (91 KiB) ships in full on every non-bare page** — low — `[dup]` — `mod.rs:887-908`. Remedy already evaluated + rejected upstream (a11y regression); only optional UI fragments could be gated.
- **KaTeX cache full-clears on overflow** — low — `math.rs:40-42`. *(Same as the Robustness math-cache finding — merge.)*

### Test gaps / CI

- **All kernel/exec tests silently no-op in CI** — high — `exec.rs` (5 sites), `kernel.rs:1038`, `parallel_build_determinism.rs:279,314`, `ci.yml`. `TALIESIN_PYTHON` unset → early-return = pass; CI never installs ipykernel. *Fix:* a separate `pip install ipykernel` + `TALIESIN_PYTHON=… cargo test -p taliesin-server` job; optionally a `TALIESIN_REQUIRE_KERNEL=1` that fails loudly.
- **client.js has no automated tests and isn't type-checked in CI** — medium — `web-client/client.js`, `ci.yml`. *Fix:* wire `tsc -p jsconfig.json` into CI; optionally a jsdom/Playwright diff-op harness (seed from live-edit-bench).
- **jsconfig type-checks only client.js; all `assets/js/` untyped/untested** — medium — `jsconfig.json`, `crates/core/assets/js/`. *Fix:* `@ts-check` + jsconfig covering search.js/toc-spy.js/`assets/js/*`; extract pure functions for `node:test` units.
- **Corpus invariant test renders but never executes cells or asserts feature output** — medium — `corpus.rs:99`. Structural-only over 65 docs; reactive/theorem/explorable regressions render clean and pass. *Fix:* `insta` snapshots on `body_html()` for a few high-value docs run through the exec path under the new kernel job.
- **cargo-deny `multiple-versions = warn`, no graph gate** — low — `deny.toml`. *Fix:* flip to `deny` with a skip-tree allowlist, or document allowed duplicates.
- **editor/vscode tests never run in CI** — low — `editor/vscode/src/test/*`, `ci.yml`. *Fix:* a Node CI job gated to `editor/vscode/**`.
- **Known-flaky `parallel_build_determinism` tests unserialized, no quarantine** — low — `[dup]` (backlog silent-drop flake) — latent until the kernel CI job lands. *Fix:* `#[serial]` + assert a dropped output is a hard named error.

### CLI / docs / consistency

- **README says "double-click", code is Alt-click** — high — `README.md:8,23,86`, `web-client/README.md:6` (verified against `client.js:1044`). *Fix:* replace with "Alt-click (Option-click on Mac)"; add a grep doc-lint.
- **`build`/`serve` silently ignore misspelled flags** — high — `build.rs:86`, `cli.rs:99-108` (verified: `s if s.starts_with("--") => {}`). Defeats the `--strict` CI gate. *Fix:* collect unknown `--` tokens, return Err with `closest()` did-you-mean; mirror in `cmd_serve`.
- **`build --out` with no value silently builds to the default target** — low — `build.rs:73-78`. Asymmetric with `--jobs`'s clean error. *Fix:* Err when the value is absent/another flag.
- **init scaffold injects a dead `github.com/anthropics/Taliesin` URL** — low — `cli.rs:24`, `main.rs:87`, `README.md:38`; `getting-started.qmd:202` uses a *different* placeholder. *Fix:* one canonical URL (reconcile before any public release).
- **`render`/`blocks` give a raw OS error on a directory while preview/build accept one** — low — `query.rs:21,66`. *Fix:* an `is_dir()` branch with a clear message.
- **Top-level `usage()` omits `--jobs` that focused `build --help` documents** — low — `main.rs:104` vs `153`. *Fix:* add `[--jobs <N>]`; extend the microcopy test.
- **Getting-started's first example leads with a `{mermaid}` cell that fails offline** — low — `getting-started.qmd:100-103`. *Fix:* drop the mermaid cell from the first example, or add an inline offline note.
- **README Usage omits `check`/`schema`/`init`** — low — `README.md:78-97,136`. The CI publish-gate story isn't discoverable. *Fix:* add `taliesin check .` + tie the diagnostics bullet to `check`.

## 4. Cross-cutting recommendations

1. **Make silent failure loud by default.** The single highest-leverage systemic move: introduce a discovery/parse warnings channel that reaches `build`/`check` `problems` (and the `--strict` exit) and the preview diagnostics overlay, then route the long tail of "silently dropped" cases through it — empty listings, `listing:` without `contents:`, suppressed social cards, titleless posts, missing chapter files, mount/page collisions, broken `_site.yml`, math render errors, and unknown CLI flags. This is the theme that recurs in *every* dimension and is what most separates the tool from "trustworthy live preview".
2. **Finish the a11y layer you advertised, then gate it.** Add `inert` to non-current deck slides, a keyboard path to the lightbox, a single-key-shortcut opt-out, and target-size minimums — then extend `a11y.rs` to `role=button` and wire the live `scanA11y` JS checks into a headless CI gate so the green check stops over-vouching.
3. **Stand up CI for the two untested behavior layers.** One kernel job (`pip install ipykernel` + `TALIESIN_PYTHON`) and one Node/`tsc` job (client.js + search.js + toc-spy.js + `assets/js/*`, plus the existing vscode tests) would convert the execution stack and the entire JS preview/enhancer layer from "fully-green-on-regression" to actually gated. Add a `TALIESIN_REQUIRE_KERNEL=1` so an env regression can't silently re-skip.
4. **Unify the renderer-hardening + theme-hot-swap + scroll seams behind shared helpers.** A shared render-guard helper closes the `preview` vs `build`/`check` panic asymmetry; moving `if theme_changed { send style }` outside the else fixes every hot-swap-drop variant; and one programmatic-scroll helper with an explicit instant-vs-smooth policy ends that drift. Each is a single seam fixing several findings.
5. **Tokenize the sepia/chrome CSS so new themes inherit correctly.** Re-point the copy button, output/error boxes, and overlay shadows at `var(--qmd-*)`/`--qmd-edge-shadow`, add the sepia `.qhl-*` palette + AA-corrected `--qmd-muted`, and add the missing prose/`hr` rhythm rules. The pattern "hardcoded light hex → per-theme override that sepia never gets" recurs; tokens kill the whole class.
6. **Coalesce per-op client work and the per-op WS fan-out.** Batch ops into one message and run `afterChange()`/scrollspy once per batch (rAF-coalesced). This single change removes the O(ops × doc) cliff, the forced-reflow scrollspy cost, and the title-clobber race in one move, on the keystroke-save hot path the project exists to optimize.

## 5. Ready-to-paste backlog block

```markdown
### Correctness / robustness (P1)
- [ ] README: replace "double-click" with "Alt-click (Option-click on Mac)" (README.md:8,23,86; web-client/README.md:6); add grep doc-lint
- [ ] build/serve: unknown --flag is a hard error with did-you-mean (build.rs:86, cli.rs:99-108) — restores the --strict gate
- [ ] Wrap build/check/render in the preview catch_unwind guard (build.rs:294, check.rs:48, query.rs:26,70) via a shared helper
- [ ] Kernel-died-mid-cell: probe is_alive() on iopub read timeout, fail fast (kernel.rs:671-709)
- [ ] Bare @-xref: require a word boundary so emails/@-mentions aren't xref'd (cite/render.rs:219-229)
- [ ] SetMeta with >1 inner data-sourcepos: fall through to full Update; add corpus test (diff.rs:83-119, client.js:934-947)
- [ ] Explicit heading {#id}: route through dedup_with_suffix or warn-and-suffix duplicates (render/mod.rs:407-417)
- [ ] Malformed _site.yml: count as a --strict problem; keep last-good in the watcher (config/mod.rs:122, build.rs:727, serve_site/mod.rs:937)
- [ ] Site-build page-task panic: increment problems so --strict fails (build.rs:852-873)
- [ ] ws.onmessage: try/catch JSON.parse + handle(); surface errors via overlay (client.js:977)
- [ ] Out-of-tree includes added after startup: recompute/diff watch set or warn (serve/mod.rs:863-895)
- [ ] Theme hot-swap: move `if theme_changed { send style }` out of the else (serve/mod.rs:1059, serve_site/mod.rs:799) — covers deck-structural + recovered

### Accessibility (P1)
- [ ] Lightbox: tabindex+role=button+aria-label+Enter/Space keydown on decorated media (11-lightbox.js:124)
- [ ] Deck: inert non-current leaf slides per commit (AT tree + tab order) (deck.js ~252, 570)
- [ ] Single-key shortcuts (f / ? arrows): Reader-menu opt-out flag; add SELECT to focus-mode guard (07-keyboard.js:55, 03-focus-mode.js:54)
- [ ] Reader-menu role=dialog: move focus into the panel on open (or drop role) (13-reader-menu.js:31)
- [ ] Min 24x24 target on .qmd-ra-btn + .qmd-anchor (base.css:130,342)
- [ ] a11y.rs: match [role=button|link|tab]; plan a headless scanA11y gate (a11y.rs:106)
- [ ] aria-hidden on .qmd-lod / minimap / threads (deck.js:303)

### Visual craft / theming (P2)
- [ ] Sepia overrides: .qhl-* palette + output/stderr/error boxes; darken --qmd-muted to AA (base.css:33,385,599)
- [ ] Tokenize copy button + overlay shadows (var(--qmd-*) / --qmd-edge-shadow) (base.css:255,293,398,696)
- [ ] Add prose rhythm: p/list margins + flat tokenized hr (base.css)
- [ ] Dark-mode --qmd-thm-* border variants (dark.css)
- [ ] Drop dead .hero h1 border/padding reset (base.css:321)

### Deck engine (P2)
- [ ] Debounce/rAF resize; drop fitSlide from resize path (deck.js:1536,242)
- [ ] Speaker view: snapshot clones or embed-mode skips {js} execution (deck.js:970)
- [ ] Encode + restore fragment index in the URL hash (deck.js:514,1142)
- [ ] Blackout: any nav key resumes; unhide cursor on pointermove (deck.js:1255)
- [ ] fragsOf: skip PRE inside .magic-move (deck.js:406)
- [ ] Speaker window: pagehide clears spClock + nulls speakerWin (deck.js:976)

### Site / books — surface silent omissions (P2)
- [ ] contents: . / own-dir listing: match siblings or reject (mod.rs:627, links.rs:112)
- [ ] listing: without contents: warn instead of drop (frontmatter.rs:137)
- [ ] Warn when image: set but url: missing (og/canonical suppressed) (meta.rs:20)
- [ ] Don't drop titleless posts from listings (mod.rs:633)
- [ ] Warn on mount/page collision (config/mod.rs:161); warn on missing chapter file (book.rs:98)
- [ ] Per-page image-alt: for listing cards (mod.rs:694)

### Citations / math / bib (P2)
- [ ] Math render failure: harvest KaTeX error, thread a located Warning (math.rs:31)
- [ ] Quoted single-brace author: strip one brace level like the brace arm + test (cite/parse.rs:165)
- [ ] strip_tags: make quote-aware (alt-text + math-heading TOC/slug) (mod.rs:1537,1408)
- [ ] \url: require \url{...}, strip to arg (cite/clean.rs:11)
- [ ] Bibliography: parse YAML value as string/seq (spaces); .at() the dup-key warning (mod.rs:754, parse.rs:91)
- [ ] Reconcile cite-key vs bib-key char sets (render.rs:240 / parse.rs:58)

### Performance (P2-P3)
- [ ] Batch WS ops into one message; run afterChange()/scrollspy once per batch, rAF-coalesced (client.js:845, serve/mod.rs:1065, toc-spy.js:86)
- [ ] Merge the two validate_cross_page_links renders; make discover-time search index lazy (mod.rs:382, search.rs:30)
- [ ] math/KaTeX cache: evict a fraction / bounded LRU instead of full clear (math.rs:40)
- [ ] emit.rs: write!/push_str instead of format!+push_str (emit.rs:16,274,330)

### Testing / CI (P1-P2)
- [ ] CI job: pip install ipykernel + TALIESIN_PYTHON cargo test -p taliesin-server; add TALIESIN_REQUIRE_KERNEL=1
- [ ] CI: tsc -p jsconfig.json (extend include to search.js/toc-spy.js/assets/js/*); add @ts-check headers
- [ ] insta snapshots on body_html() for reactive/explorable/bayesian docs through the exec path (corpus.rs:99)
- [ ] CI job for editor/vscode tests (gated to editor/vscode/**)
- [ ] deny.toml: multiple-versions = deny + skip-tree allowlist (or document allowed dups)
- [ ] #[serial] the kernel-load determinism tests; assert dropped output is a hard error

### CLI / docs polish (P3)
- [ ] build --out with no value: hard error (build.rs:73)
- [ ] render/blocks: is_dir() branch with a clear message (query.rs:21,66)
- [ ] usage() build line: add [--jobs <N>] + test (main.rs:104)
- [ ] Reconcile scaffold/usage/README/getting-started repo URL placeholders (cli.rs:24, main.rs:87, README.md:38)
- [ ] Drop {mermaid} from the first getting-started example, or add an offline note (getting-started.qmd:100)
- [ ] README Usage: add `taliesin check .`; tie the diagnostics bullet to check (README.md:90,136)

### Security hardening (P3, single-author model)
- [ ] history.replaceState to scrub ?t= after mount (security.rs:150, client.js)
- [ ] qmd_token cookie: add ; HttpOnly (security.rs:124)
- [ ] Injected Mermaid <script>: integrity + crossorigin; emit Referrer-Policy: no-referrer (mod.rs:858, page.rs:150)
- [ ] origin_allowed: only blanket-allow loopback when loopback-bound (security.rs:13)
- [ ] Deck postMessage: gate null/'' origin on file:// only (deck.js:893)
- [ ] Extension-resource fallback: re-check containment after the symlink walk (serve/mod.rs:387)
```

Key file references for the lead items, all verified against source this session: `README.md:8` ("Double-click a rendered element") vs `crates/server/src/build.rs:86` (`s if s.starts_with("--") => {}` silent catch-all). Merges applied: the diff-dimension theme-hot-swap finding folds into the dev-server one (single fix); the `afterChange` per-op finding and the Performance full-rescan finding are one root cause; the Robustness math-cache and Performance KaTeX-cache findings are one. `[dup]` marks the four backlog-overlapping items (asset shipping, per-op rescan, code-enhance always-ships, and the flaky-determinism/silent-drop test).