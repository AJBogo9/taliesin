# qmd-fast backlog

**Scope: corpus-plus-roadmap.** "Done" means the docs under `corpus/` render correctly
(the corpus is the regression net), but each new capability now ships pinned by a target
corpus doc. Output stays **HTML-only**. The active roadmap is `BEYOND-QUARTO.md`.

> Kept deliberately small (read often). **Only open tasks live here.** Completed work is in
> git + the history docs: `BEYOND-QUARTO.md` (Beyond-Quarto waves), `DROP-QUARTO.md` (the
> native rewrite), `AUDITS.md` (the audit passes). Don't re-add `[x]` items — delete them
> once landed.

## State (2026-07-01)

`main` @ `8c45fbc` == `origin/main`, version 0.2.0 (in sync; I commit+merge+push to main on
request, the author syncs between sessions, agents do NOT push). All four formats render +
deploy. The dev loop is strong: block-level incremental updates with DOM-state preservation,
warm server + Jupyter kernel, `_freeze` cache, Alt-click click-to-source + reverse cursor
sync, located/framed diagnostics, CSS hot-swap, Cmd-K search. The public OSS release + site
publish is gated on readiness (the `--host` security token, now shipped).

**Shipped initiatives** (history in the docs above + git): DROP-QUARTO (fully native, no
shims/reveal.js/OJS); Beyond-Quarto **Waves 0-4** (schema validator + JSON schemas, live-edit
benchmark, the six Wave-3 craft/breadth features, reverse-sync audit, VS Code companion Phase 1);
the reader-experience cluster; Pillars I + III (`check`/prose-lint, `{input}` + scrolly); the
`--bare` build; the release-hardening + Tier-1 hardening batches; the **reading-first redesign**
(one centred column, OS-follow theme, WCAG-AA accent) and **deep-audit P1** (both @ 2026-06-30);
and the **author book-testing fixes** (2026-07-01, 15 of 21: caption math, `file://` cross-page
search, mobile TOC sheet, book-wide bookmarks, numbered chapter titles, Scholar meta, walkthrough
code-on-top, portable offline build, themed chevron, focus-mode fullscreen, cross-block toolbar,
pinnable hover cards …).

**Working method:** branch per feature; brainstorm if there's a fork; write a spec under
`docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
`@vscode/test-electron`/relay harnesses for the extension); fast-forward merge locally; delete
the item here. Do-NOT-touch: the exec/kernel zone + the single-editing-surface invariant. Review
subagents use read-only git (`git diff a..b`, never `git checkout`; they share the working tree).

**Author policy (feature-first):** finish framework features before marketing-site work; the
`live-edit-hero-demo` clip + the "Marketing site" section stay deferred until then.

**Pending author action:** F5-accept the VS Code companion: `cd editor/vscode && npm install
&& npm run build`, then F5 and run the `editor/vscode/README.md` checklist (cursor → block
highlight; Alt-click → source). Report anything off.

## Priority queue (what's next)

The single ordered view; each line points into a detail section below. Tiers, not a strict rank.

**Tier 1: COMPLETE (2026-07-02, all 7 clusters shipped to `main`, each browser-/test-verified;
lower-value residuals within each cluster logged in its section):**
1. ✅ Citations / math / bib (`ba6de8d`) — math-render diagnostic, `\url`, quoted-brace authors,
   string/seq bibliography, shared key charset, quote-aware `strip_tags`.
2. ✅ Site / books silent omissions (`b3beabc`) — 6 warn/reject cases + `contents:.` + card `image-alt`.
3. ✅ Visual craft / theming (`0a99528`) — sepia first-classing, tokenized copy/shadows, prose rhythm,
   dark theorem borders.
4. ✅ Deck engine (`2be9428`) — fragment-in-hash, blackout resume, coalesced resize, fragsOf/pagehide.
5. ✅ Performance (`3612fa5`) — rAF-coalesced save `afterChange`, single-render cross-page check, KaTeX LRU.
6. ✅ Search-hit visual cue (`ab6437c`) — CSS Custom Highlight flash + cross-page `sessionStorage` handoff.
7. ✅ Author publishing/share recipe (`e48e869`) — documented in the guide (publish CLI deferred).

**Tier 2: needs a design or owner decision before building (NEXT — the author decides):**
- Cross-page refs (F2b number for non-heading anchors, F2a hover preview): both blocked on the
  render-harvest vs source-scan design call. → *Cross-page references*
- Cross-reference graph / backlinks: ship backlinks first vs graph-canvas decision. → *Cross-page references*
- Reading-first identity polish (hero / cards / accent / spacing scale): overlaps deferred
  marketing; confirm direction. → *Reading-first identity polish*
- Vendor Mermaid offline: a repo-bloat call (~2-3 MB). → *Cross-page references / portability*
- Taliesin rename: owner-gated identity call, ties to OSS-release timing. → *Taliesin rename*
- Parked on the author (need a repro / a viewport / a fork): B4-arrow, C1, "code blocks need a
  refresh", `.qmd` format-on-save, FL-weather migrate. → their sections.

**Tier 3: hardening / lower (P3):**
- Testing / CI residuals; CLI / docs polish; Security hardening; Extension-ecosystem audit (own
  pass, incl. the dead liquid-glass extension); Companion `check`/prose-lint diagnostics;
  Execution-cache leaks (do-NOT-touch, careful); Release regression-hunt deferrals (LOW/by-design);
  Audit residuals.

**Tier 4: deferred / demand-driven:**
- Image optimization; Wave 5 (print-pdf track, docs-as-spec, `{glsl}`, SEO completeness);
  Interactive/explorable numerics (#62-66); VS Code companion Phase 2; Marketing site;
  `serde_yaml` fallback watch-item.

## Open tasks (by area)

### Cross-page references
`site/xref.rs` is a deliberately lightweight source-scan (page URLs + section numbers only).
*F2b (cross-page NUMBERS) SHIPPED 2026-07-02 (`main` @ 68f7740, author-approved render-harvest):
`RenderedDoc.xref_numbers` + build-only `Site::harvest_xref_numbers()` fill the fig/eq/tbl/lst/thm
numbers the scan couldn't, so a cross-page `@fig-x` renders "Figure&nbsp;1"; preview stays bare (the
accepted no-double-render tradeoff). Pinned by `harvest_numbers_cross_page_figure_refs`.*
- [ ] **F2a: hover preview for cross-page refs** (P3). `12-link-preview.js` only fires on same-page
  `#` links; a cross-page xref target lives on another page. The render-harvest infra is now in place
  (extend it to collect an anchor→preview snippet, serve it like `search-index.js`, wire the hover
  card for `.qmd-xref` cross-page links). Grouped with the graph below (shared cross-page content
  index). Deferred behind the graph canvas.
- [ ] **Cross-reference graph canvas (Obsidian-style)** (author chose the FULL graph, not
  backlinks-first). An interactive force-directed graph of the project's pages + their cross-page
  references, click-to-navigate. Read-only + HTML-only (single-editing-surface holds). Reuses the
  vendored d3 (has `d3-force`) + a build-time reference-edge scan. NEXT Tier-2 build.
*Vendor Mermaid offline: SHIPPED 2026-07-02 (`main` @ 30bdb17, author-approved). `mermaid@11.4.1`
vendored + inlined into static Build pages that have a diagram (content-gated); a `--out` doc/book
renders diagrams with ZERO network (browser-verified from `file://`, 1 request). Preview keeps the
lean lazy loader. THIRD_PARTY updated; pinned by `mermaid_library_inlined_into_build_pages_only`.*

### Discoverability & distribution
*Publishing recipe DOCUMENTED 2026-07-02 (`main`): a "Publishing & sharing" section in
`docs/guide/reference/cli.qmd` (send-a-zip, GitHub Pages 3-line push, drag-drop hosts, rsync/S3, +
the `url:`-before-publish + `check`-first notes). Remaining, deferred:*
- [ ] **Thin `qmd-fast publish` command** (P3, optional): push `_site/` to a rendered-HTML branch
  (e.g. `gh-pages`) in one step. Its own design (git integration, force-push safety, auth) — deferred;
  the documented manual recipe fully covers the need today. ***REMOVED***
  read-only export, no write-back. Distinct from the deferred marketing-site deploy.
- Already tracked (no new entry): **generic SEO** (sitemap.xml / robots.txt / JSON-LD) is fully
  specced in `build-seo-completeness` (Wave 5) + `BEYOND-QUARTO.md:308-312`. Existing SEO today is
  og/twitter/canonical, gated on a configured `url:`.

### Reader experience
Pattern for any new reader control: `window.qmdReaderMenu.addSection(title, node, onOpen)`; state in
the reader's own `localStorage` keyed by `location.pathname`; deck-skip; pre-paint via
`render/theme.rs` for anything that must not flash. **GOTCHA:** prose CSS (`body p, body li { … }`)
leaks into chrome that wraps prose (TOC, sidebars, navbar `<nav><ul><li>`, search `role=listbox`,
margin notes); re-pin with `nav li, [role="listbox"] li, .sidenote p, … { line-height: inherit }`.
Letter/word spacing leaks worse (inherits into inline descendants), so also reset monospace + math
directly (`code, pre, kbd, samp, .katex { letter-spacing: normal; word-spacing: normal }`).
*Search-hit visual cue SHIPPED 2026-07-02 (`main` @ ab6437c): Cmd-K result → flash the matched term
via the CSS Custom Highlight API, theme-aware fade, cross-page `sessionStorage` handoff. Spec
`docs/superpowers/specs/2026-07-02-search-hit-visual-cue.md`.*
- [ ] **Focus mode "prev arrow too far left"** (needs the author's input). Does NOT reproduce in code —
  the book pager (`.qmd-book-postnav`/`.qmd-book-prev`, `site.css:200-215`) already sits inside the
  centred reading column and there's no viewport-edge prev arrow. Which view/viewport showed it?
- [ ] **"Code blocks need a refresh to appear"** (need a repro). Not reproducible in code:
  highlighting is server-side and present on first paint (`highlight.rs:49`→`emit.rs:65,82`); the only
  client enhancer is the copy button; no hydration step to miss. If it recurs, capture exact steps
  (viewport / kernel state / after a WS reconnect?) and reopen.
- Decided/known: the reader menu is intentionally an untrapped popover (not a modal); highlights are
  single-block prose only (margin notes / cross-block / colours scoped out — see specs).

### Visual craft / theming (deep-audit P2)
*Cluster shipped 2026-07-01 (`main` @ 0a99528), browser-verified light/dark/sepia: sepia
first-classed (warm `.qhl-*` + warm output/stderr/error/js-error boxes + AA `--qmd-muted`),
copy button + four box-shadows tokenized, prose rhythm (paragraph margin + flat `hr`), dark
theorem-border variants, dead `.hero h1` reset dropped. Residual (deferred, low):*
- [ ] **List-margin rhythm** left to UA defaults: a global `li { margin }` leaks into chrome
  nav/TOC `<li>`s. A scoped content-only selector (`#qmd-main`/`#qmd-root`) would let lists get
  the same tokenized rhythm as paragraphs; skipped for now (paragraphs + `hr` cover most of it).
- [ ] **Sepia callout/theorem HEADER tints** still color-mix from fixed cool colors, so a Note
  header reads slightly cool on the warm page (borders are fine). Low; revisit if sepia gets a
  polish pass.

### Deck engine (deep-audit P2)
*Cluster shipped 2026-07-01 (`main` @ 2be9428), browser-verified in present mode: fragment-in-hash
encode+restore, blackout resume-on-any-nav-key + idle-gated cursor, rAF-coalesced resize, fragsOf
skips `<pre>` in `.magic-move`, speaker/presentation `pagehide` cleanups. Two items deferred:*
- [ ] **Speaker-preview iframes still run `{js}` live** (2 live embed iframes). Skipping `{js}` in
  `deck.mode==='embed'` is a one-line guard in qmd-js.js, BUT it loses the presenter's live preview
  visual — an author call (skip for perf vs snapshot-clone the current state). Deferred pending that
  decision (deck.js `initSpeaker`).
- [ ] **Drop `fitSlide` from the resize path** (the coalesce shipped; the full re-fit of every slide
  per frame remains). `apply()` doesn't fit the current slide today, so a lazy fit-on-show refactor
  (fit current on resize/nav, mark others dirty) is needed to safely drop the all-slides fit. Medium.

### Site / books: silent omissions (deep-audit P2)
*Cluster shipped 2026-07-01 (`main` @ b3beabc): all six items — `contents: .` root listing fix,
`listing:`-without-`contents:` warning, site-`image:`-without-`url:` warning, titleless-post
warning, mount/page-collision + missing-chapter-file warnings, per-page `image-alt:` on cards.
6 new tests; verified no spurious warnings across all 5 real corpus/docs sites. Image-alt now also
corpus-pinned (tech-blog listing test @ 52ae47d).*
- [ ] **`contents: .` lacks a persistent corpus PAGE** (LOW, audit-qmd residual). The root-listing fix
  is solidly unit-tested (`contents_dot_at_root_lists_siblings_and_warns_titleless`) but no `corpus/`
  doc uses `contents: .`, so the corpus arbiter doesn't see it. Deferred rather than distort the real
  tech-blog nav with a synthetic "list everything" page; add a small dedicated fixture if pinning is wanted.

### Citations / math / bib (deep-audit P2)
*Cluster shipped 2026-07-01 (`main` @ ba6de8d): math-render diagnostic (`diagnostics::validate_math`),
`\url` naive-replace fix, quoted single/double-brace authors, string/seq `bibliography:`, shared
`is_cite_key_char`, quote-aware `strip_tags`. Remaining low residuals:*
- [ ] **Dup-key bib warning stays unlocated** (parse.rs): a `.bib` duplicate-key warning can't point at
  a meaningful `.qmd` line (it's about an external file); left unlocated deliberately. If wanted, locate
  it at the front-matter `bibliography:` line (needs that line threaded through `load_bibliography`).
- [ ] **Math-in-heading TOC/slug garble** (mod.rs `strip_tags` on KaTeX): quote-awareness shipped, but
  KaTeX's `<annotation>` source text still concatenates into slug/TOC text for a heading containing
  `$…$`. Cheap follow-up: skip `<annotation …>…</annotation>` contents in `strip_tags` (or prefer the
  MathML aria text).

### Performance (deep-audit P2-P3)
*Shipped 2026-07-01 (`main` @ 3612fa5): rAF-coalesced `afterChange` on the save hot path (the
dominant O(ops × doc) win, browser-verified live-edit), single-render `validate_cross_page_links`,
FIFO-bounded KaTeX cache (+ test). Remaining, lower-value:*
- [ ] **Protocol-level op-message batching**: send a save's block ops in ONE websocket message
  instead of one-per-op (client.js + serve/mod.rs + a `protocol_contract` update). Smaller win than
  the coalesced `afterChange` already shipped (message overhead ≪ the O(doc) recompute), so deferred.
- [ ] **Lazy discover-time search index** (search.rs:30): build `search_index_json` on first use
  rather than eagerly in `Site::discover`. Lower value (it's a text scan, not a render); needs an
  `Option<String>` + interior mutability on `Site`.
- [ ] emit.rs: `write!` instead of `format!`+`push_str` per tag (emit.rs; 19 sites). Low-value P3
  micro-opt (saves a temp alloc per tag); deferred to avoid churn risk in the hot emit path.

### Testing / CI
- [ ] insta snapshots on `body_html()` for reactive/explorable/bayesian docs through the exec path
  (corpus.rs is structural-only) (corpus.rs:99).
- [ ] CI job for editor/vscode tests (gated to editor/vscode/**).
- [ ] deny.toml: `multiple-versions = deny` + skip-tree allowlist (or document allowed dups).
- [ ] `#[serial]` the kernel-load determinism tests; assert a dropped output is a hard named error
  (the known silent-drop flake).
- [ ] Extend tsc + `@ts-check` to `search.js`/`toc-spy.js`/`assets/js/*` (surfaces a large pre-existing
  error backlog — its own pass; client.js is already gated in CI). *Also gated now: the CI kernel job
  + client.js tsc job.*

### CLI / docs polish (P3)
- [ ] `build --out` with no value: hard error instead of silent default target (build.rs:73).
- [ ] render/blocks: `is_dir()` branch with a clear message (raw OS error today) (query.rs:21,66).
- [ ] usage() build line: add `[--jobs <N>]` + extend the microcopy test (main.rs:104).
- [ ] Reconcile scaffold/usage/README/getting-started repo-URL placeholders (cli.rs:24, main.rs:87,
  README.md:38).
- [ ] Drop the `{mermaid}` cell from the first getting-started example, or add an offline note
  (getting-started.qmd:100).
- [ ] README Usage: add `qmd-fast check .`; tie the diagnostics bullet to `check` (README.md:90,136).
- [ ] Reconcile the no-kernel-build wording (now embeds a per-cell "kernel unavailable" diagnostic,
  not just the preview banner): `CLAUDE.md:122-123`, `docs/guide/using/getting-started.qmd:44`, and the
  misleading `build.rs:232` stderr "uncaught exception".

### Security hardening (P3, single-author trust model)
- [ ] `history.replaceState` to scrub `?t=` after mount (security.rs:150, client.js).
- [ ] `qmd_token` cookie: add `; HttpOnly` (security.rs:124).
- [ ] Injected Mermaid `<script>`: `integrity` + `crossorigin`; emit `Referrer-Policy: no-referrer`
  (mod.rs:858, page.rs:150).
- [ ] `origin_allowed`: only blanket-allow loopback when loopback-bound (security.rs:13).
- [ ] Deck postMessage: gate null/'' origin on `file://` only (deck.js:893).
- [ ] Extension-resource fallback: re-check containment after the symlink walk (serve/mod.rs:387).

### Reading-first identity polish (P3, design judgment; deferred, overlaps marketing)
The "templated" diagnosis is itself UNVERIFIED (see caveat); treat these as judgment, not evidence,
and re-check competitor layouts live before banking a default on "X does Y."
- [ ] **Hero as typeset reading, not a marketing slab.** The eyebrow + big headline + lead + two-button
  hero is the generic SaaS shape (site/mod.rs hero block); for a typography tool the most honest hero
  is beautifully-set prose that shows the real type system.
- [ ] **Drop bordered feature-card grids** for a typeset list with strong hierarchy (site.css `.qmd-card`).
- [ ] **Reconsider the tech-blue accent** for a quieter near-monochrome plus one restrained accent.
- [ ] **Introduce a spacing scale** (`--space-1..6`). Spacing is ad-hoc rem literals throughout
  (base.css), making the calm, consistent rhythm reading-first design needs hard to enforce.

### Extension ecosystem audit (deferred, its own pass)
*Author decision (2026-06-27): the `_extensions/` story (themes + functionality) gets a dedicated
audit pass, not piecemeal fixes. Survey what an extension can hook (theme `--qmd-*` tokens, bundled
CSS/JS, shortcodes, `{{< embed >}}`), where the seams are sharp vs sharp-edged, and what's missing /
under-documented; produce a prioritized improvement list. Treat the native deck contract
(`window.QmdDeck`, `.qmd-deck`/`.qmd-slide`) as the stable target.*
- [ ] **Run the audit** (themes + functionality): inventory the worked examples under `_extensions/`,
  exercise each against the real binary, find rough edges + gaps + missing docs.
- [ ] **Known finding to fold in — liquid-glass corpus extension is dead.** Live `Uncaught
  ReferenceError: Reveal is not defined`; its CSS targets `.reveal` DOM the native engine never emits,
  so the headline glass effect of THE worked example is non-functional. Fix: port to `window.QmdDeck`
  + `.qmd-slide`/`.qmd-deck`; add a corpus test asserting the theme applies.

### Tooling / format future
- [ ] **Companion: surface `check`/prose-lint as editor diagnostics** (P3). VS Code squiggles from
  `qmd-fast check --format json` / `crate::prose` located warnings (read-only, no buffer writes). New
  vs the Phase-2 text-transform commands.
- [ ] **`.qmd` format-on-save** (open question, NOT Phase 2). A source pretty-printer would write the
  editor BUFFER (the allowed surface) but must preserve `data-sourcepos` line stability for
  click-to-source. Brainstorm whether the reflow is worth the click-to-source risk before any work.
- [ ] **Dogfood: migrate the external FL-weather book to qmd-fast** (P3). A real-world Quarto→qmd-fast
  migration + portability stress test (exercises `book.rs`, includes, the freeze cache, the
  `_quarto.yml` breadcrumb, file-mode portability). If it renders clean, consider pinning a reduced
  version under `corpus/`.
- [ ] **`check` online-link mode (opt-in).** Broken plain/external `http(s)` links are intentionally
  NOT fetched (offline + deterministic by design). If ever wanted, gate a real fetch behind an explicit
  `--online` flag so the default `check` stays kernel-free and network-free.

### Execution cache (exec/kernel Do-NOT-touch, careful)
- [ ] **Kernel/forkserver resource leaks on build exit** (observed 2026-06-29). (a) warm-pool
  forkserver daemons survive a completed `build` — ~30 orphaned `multiprocessing.forkserver` procs
  (~100 MB each) left after normal-exit builds; likely the daemon child isn't reaped on CLI exit
  (check `process::exit` skipping `Drop`, or the daemon `Arc` outliving the runtime). (b) a failed
  `Kernel::start` leaks its `/tmp/qmd-kernel-<uuid>` connection dir (only a *successful* `Kernel` owns
  it for Drop-cleanup; error paths drop the `PathBuf` without removing the dir); the 2026-06-29
  start-retry amplifies (b). Fix: kill the forkserver daemon on teardown; remove the conn dir on a
  failed start. Low-priority (reclaimed on reboot) but unbounded under repeated failures.
- [ ] **Boot-failure diagnostic overwrites a cache-hit cell's output** (`exec.rs:491-505`). Only when a
  cached cell sits upstream of an uncached one AND the kernel fails to boot through all retries — the
  build is already flagged `error` (never green) and the cell shows its source, so strictly more honest
  than the old silent empty. Optional: restore from freeze for `known(i)` before the diagnostic.
- [ ] **Warm-pool `in_flight` counter can leak** (inert the pool) if a refill task panics
  (`warm_pool.rs:456-490`). No reachable panic site today (the `warm_one` chain is all `Result`/`?`);
  worst case is graceful cold-start fallback. An RAII drop-guard on `in_flight` would harden it.
- [ ] **Cold-start kernel warming** (deferred). After a cold full-replay, the first edit re-runs the
  whole doc to rebuild kernel state. Could speculatively warm the kernel in the background. Inherent to
  a plain Jupyter kernel; not worth it until it bites.

### Release regression-hunt deferrals (LOW / pre-existing / by-design)
- [ ] **Cross-page theorem refs drop the number** ("Theorem 2.1" → bare "Theorem" across pages;
  `site/xref.rs`). Widened by the *Cross-page references* item above (same root gap); harvest numbers
  from the per-page rendered registry if parity is wanted. Ties into theorem ref-name polish in
  `BEYOND-QUARTO.md`.
- [ ] **A theorem nested inside another fenced div** (`.column-margin`/`.callout`) loses its number +
  xref registration (`number_theorems` walks only top-level blocks). The xref half IS surfaced by
  `check`/`build`; residual: an *unreferenced* nested theorem renders unnumbered on a green check.
  Optional: warn when a `data-qmd-theorem-kind` div is found nested.
- [ ] **Backslash-escaped quotes in a `title=`/`fig-cap=`/`lst-cap=` value truncate it + leak `\`**
  (`render/divs.rs` `tokenize_attrs`). Pre-existing, narrow. Teach `tokenize_attrs` to honor `\`
  escapes, or lint a backslash-before-quote.
- [ ] **CLI microcopy (residual):** raw ANSI leaks into HTML for R stream/stderr (`kernel.rs:672` —
  DEFERRED, exec/kernel Do-NOT-touch).

### Audit residuals (deferred, low-risk; detail in `AUDITS.md`)
- [ ] **Robustness.** Combined content+theme edit drops the hot-swap until reload (`serve.rs`); initial
  synchronous render isn't panic-guarded; `front_matter_block` terminates early on `---`/`...` inside a
  block scalar; mounted sub-sites don't route embedded decks (a mount miss serves a bare 404).
- [ ] **Perf.** `updateWordCount` deep-clones all of `#qmd-root` per op (`client.js`); visited pages
  are never evicted from `app.pages` (`serve_site.rs`, unbounded growth); a tens-of-MB cell output
  blocks the ZMQ receive before the cap fires (`kernel.rs`).
- [ ] **Bib / build edge cases.** `@inbook`/`@incollection` drop `booktitle`/pages; query-string asset
  refs aren't bundled (`main.rs`). Remaining LOW findings live in `AUDITS.md`; pull up only when relevant.
- [ ] **Long tail** (from the polish audit): perf (shared/minified/compressed assets, O(change)
  per-edit); doc/code drift — see the audit digest `polishThemes` + `whatsMissing`.

### Deck (mobile / footer)
- [ ] **Mobile / touch (deeper).** Pinch/pan + touch gestures on the deck, and `{js}` widgets tuned
  for touch. (Hard to verify without a real device.)
- [ ] **Footer / logo (deferred).** No corpus deck needs one yet; thread `footer:`/`logo:` through
  both deck-page builders + the asset-copy set when one does.
- Decided against: inline `{.r-stretch}` image (use the `:::{.r-stretch}` div); `#`-section quick-jump
  anchors (redundant with the minimap + `/` filter).

### Taliesin rename (settled design, UNSHIPPED)
Spec: `docs/superpowers/specs/2026-06-27-taliesin-rename-design.md` (decisions settled). Owner-gated
identity call; ties into public-OSS-release timing. The author's "drop `.qmd` and own the highlighting"
ask.
- [ ] **Execute the rename**: `.qmd`→`.tmd` routed through a central `crates/core/src/ext.rs` constant
  module (`.qmd` kept as deprecated-accepted input with a warn-nudge — a clean break WITH a migration
  path, per the spec's Markdown-familiarity north star); package names →`taliesin-*`; binary `taliesin`
  + `tali`; the `qmd-*` contract prefix →`tali-*` with back-compat aliases. Large multi-surface change
  (corpus + docs churn).
- [ ] **Own the syntax-highlighting grammar** via the `.tmd` language association (the VS Code companion
  already sets its own regardless; spec §3). The concrete answer to "rename so I fully control the
  highlighting."

### Interactive/explorable numerics (idea pool in `FEATURE-IDEAS.md` #62-66)
Surfaced by dogfooding a Bayesian-ML study site on the shipped `{input}` + `{js}` reactive graph. The
substrate is there; what math/ML explorables lack is a numerics story + two controls. All stay
HTML-only/offline and must **not** reintroduce a reactive VM (the stated top design risk). None spec'd
/ corpus-pinned yet — promote here with a pin when one graduates. Highest-leverage: **#62 + #63**.
- [ ] **#62 Bundled numerics/stats global for `{js}`** (P2): a small curated global beside `Plot`/`d3`
  — distribution pdf/cdf (gaussian/gamma/beta/poisson/exp), mean/var, a **seeded** PRNG, small dense
  linalg (matmul, Cholesky, 2×2 eig/inv). Kills the #1 friction (hand-rolling pdfs). Pin
  `corpus/reactive/numerics.qmd`.
- [ ] **#63 Two ML `{{< input >}}` types — `animate`/play tick + draggable `point`** (P2-P3): a
  play/step/reset tick for iterative demos (EM/CAVI/gradient descent) and a drag/click 2-D point for
  "place a data point". Reuse `registerInput`/`scheduleFrom`; the tick schedules **one** downstream pass
  per frame via the scheduler + `invalidation` — not a dataflow loop.
- [ ] **#64 `qmd.state` cross-re-run store** (P3, needs-care): keyed state that survives scheduled
  re-runs so iterative demos accumulate (EM params across ticks); cleared on cell edit; deck-skip; no
  write-back. Pairs with #63; scope tightly.
- [ ] **#65 Richer `{js}` output helpers** (P3): KaTeX-typeset a returned number/array/matrix + a
  minimal table renderer, over the existing DOM-return contract. Closes the rich-display gap vs Jupyter.
- [ ] **#66 Opt-in Pyodide `{python}` cell** (L, needs-care): client-side numpy/scipy/sklearn, no kernel
  (JupyterLite). **Bundle guard**: ~10 MB+, opt-in per page, vendored offline; sibling to DuckDB-WASM
  `{sql}` (#50). Caveat: **no torch in Pyodide**. Cell-language-registry graduate; cut until a corpus
  doc needs it.

### Deferred / demand-driven
- [ ] **Image optimization (large).** WebP/AVIF transcode + responsive `srcset` + lazy-load, behind a
  content-hashed asset cache. Deferred until posts get image-heavy.
- [ ] **Wave 5 / later** (`BEYOND-QUARTO.md`): `print-pdf-track` (paged render *of* the built HTML),
  `docs-as-spec` (RFC-2119 dialect + protocol reference), `{glsl}` cell-language registry,
  `build-seo-completeness` (sitemap/robots/JSON-LD at publish with `url:`).
- [ ] **VS Code companion Phase 2 (capped).** Editor commands (insert block / reorder slide) — strictly
  `.qmd`-buffer text transforms in the editor, never preview gestures.
- [ ] **`serde_yaml` fallback watch-item.** The `Cargo.toml` workspace comment names `serde_yml` as the
  fallback, but it carries RUSTSEC-2025-0068 (unsound + unmaintained); `serde_norway` is 1+ yr stale.
  The maintained continuation is **`serde_yaml_ng`** (v0.10). No urgency (trusted local config; 0.9 still
  builds). If 0.9 ever breaks against a future serde/edition, swap to `serde_yaml_ng`, gated on a test
  that `Error::location().line()` still works. Fix the stale comment when touched.

### Marketing site (DEFERRED, feature-first; rolls into a demo-machine rebuild)
- [ ] `live-edit-hero-demo`: the recorded split-screen-vs-Quarto clip (the bench numbers +
  `tools/record-demo` recorder already exist).
- [ ] Swap placeholders in `site/_site.yml` (`url:` + GitHub links); rebuild the hero pages demo-led
  (motion, one value line, the vs-Quarto table, install on-ramp). Folds in the open visual bugs: 390px
  prose overflow (`page-layout: full` + `hero:`), theme/video desync (drive the `{{< video >}}` variant
  off the site toggle), leftover em dashes in copy.
- [ ] Refine the mobile embed (narrow iframe → reader). Deploy (Cloudflare / GitHub-Pages).

## Decided against / do-not-re-litigate

**Reading-first defaults — research-validated keeps** (do NOT "fix"): serif body is fine for long-form
screen reading ("serif hurts legibility" refuted; Section508 + Tufte CSS) — don't switch to sans; ~70ch
measure (`--qmd-maxw: 46rem`) is correct (45-75 CPL, target the upper end for screens) — don't narrow;
right-rail scrollspy "on this page" TOC is NN/g-correct, sidenotes as a width-gated progressive
enhancement matches Tufte/Gwern — keep both; scroll (not pagination) book reading has no comprehension
difference (Joshi et al., CHI EA '25) — keep scroll; system-font-only is right for offline + no reflow
(if a serif webfont is ever bundled, ship REAL bold/italic faces, never browser-synthesized).
*Research caveat:* the deep-research pass could NOT verify what Stripe/Linear/Mintlify/Docusaurus/GitBook
concretely do in 2025-26, the "Bootstrap/Quarto looks dated" thesis, or command-palette-as-nav specifics
— treat the identity-polish items and competitor framing as judgment, not evidence.

**Library outsourcing — decided against** (multi-agent sweep of every from-scratch subsystem vs mature
OSS, each adversarially verified against the invariants): **hayagriva**/**biblatex** (citations — large
integration, heavy deps incl. serde_yaml 0.9, zero corpus demand: only IEEE is used, already hand-rolled;
revisit only for live multi-CSL switching); **schemars** (reopens the schema↔validator drift already
closed); **jsonschema** (loses source-line diagnostics); **morphdom**/**idiomorph** (reverse the 83x
live-edit payload win + risk live-state loss; the diff is server-authoritative); **similar**/**dissimilar**
(give up the unique-block-id→LIS reduction); **clap**; **owo-colors**; **slug** (transliterates non-ASCII →
breaks anchors/`@sec-`); **html-escape** (breaks the anti-double-escape contract); **lightningcss**/**palette**
(no Rust color math — CSS uses native `color-mix`); IntersectionObserver/scrollspy libs (can't do the
dynamic activation line + bottom-pinning); deck micro-helpers d3-interpolate/screenfull/hotkeys/hammer
(each force an offline bundle onto every deck for a few lines).

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; see `STARTUP-PLAN.md`). Open-source the repo + publish the site when
ready; the GitHub/install CTAs become real then. The security token (the gate) is shipped.
