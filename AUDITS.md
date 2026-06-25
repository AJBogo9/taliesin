# qmd-fast audit records

Consolidated historical audit reports, newest first. The active backlog
(open items only) lives in [backlog.md](backlog.md); these are the detailed
findings behind it, kept for reference.

-----------------------------------------------------------------------------

# Corpus fidelity sweep vs Quarto (2026-06-25)

Systematized output-fidelity check (backlog "highest-value #4"). The sibling
`qmd-fast-testbed` gained `sweep_corpus.py`: render every real corpus single-doc
(the spec) in **both** qmd-fast and Quarto with execution disabled, reduce to the
block skeleton, structural-diff, and catalog. The full classification lives in
`qmd-fast-testbed/CORPUS-FINDINGS.md`; the generated report is `corpus_sweep.md`.

**Result: 3/8 exact match** (callouts/kinds, narrate/walkthrough, posts/born-machines);
the other 5 differ only in items below, all classified. A `run.py` normalization fix
(code-token whitespace was inventing `np.polyfit` → `np . polyfit` noise) also lifted
the existing conformance suite 16/27 → 18/27.

**Deliberate (qmd-fast intentionally different):** native `{js}` cells render as live
widgets not dumped OJS source; flat `div.qmd-layout` vs Quarto's
`quarto-layout-panel/row/cell`; references in a semantic `section.qmd-references` vs
`div.references`; display-math in `div.qmd-math` (KaTeX server-side, nothing dropped)
vs Quarto's `<p>\[…\]</p>`; tabset's no-JS form (`h2` + stacked panels, upgraded to
ARIA tabs by client JS).

**qmd-fast does better:** resolves `@fig-`/`@sec-` cross-refs even without executing
the target cell (Quarto leaves `?@fig-…` under `--no-execute`).

**Real bug-candidates (REPORTED, follow-ups — not yet fixed):**
- **`#|` option lines leak into displayed code.** A non-executed `{python}` cell renders
  its `#| label:` / `#| fig-cap:` directive lines as visible highlighted code; Quarto
  strips `#|` from echoed source. Affects the no-kernel render/preview path. Fix in the
  cell source-emit path (`crates/core/src/render/emit.rs`): strip leading `#|`/`//|`
  option lines before highlighting source-rendered cells. *Confirmed in the HTML.*
- **Captioned code listing is not a `<figure>`.** qmd-fast emits `div.qmd-listing` with a
  `<figcaption>` (a `<figcaption>` outside `<figure>` is out of place); Quarto uses
  `<figure class="quarto-float-lst">`. Minor/semantic (`render/figure.rs`/`emit.rs`).

-----------------------------------------------------------------------------

# qmd-fast — round-2 adversarial audit (2026-06-21)

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
| Slide deck (`corpus/liquid-glass-slides`) | Top-tier wow | Frosted-glass title slide + per-bullet glass panels; a third-party reveal theme renders on qmd-fast's own engine. Best single demo asset. |
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
