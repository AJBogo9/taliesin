# Do not rebuild / do not re-scope

Split out of [backlog.md](backlog.md) on 2026-08-07, when that file was pruned to the release
critical path only. **Nothing here is a task.** This is the anti-rot register: what shipped, what
was declined, what a measurement refuted, and the known non-defects — kept so a later round does
not rebuild deleted work, re-file a closed ruling, or re-derive an answer that already cost a
session.

**One line per entry.** Detail lives in git, in [AUDITS.md](AUDITS.md), in the dated findings docs
and in [LESSONS.md](LESSONS.md). A batch's date and branch are enough to find its commits. If you
are about to add a paragraph here, it belongs in one of those files instead.

> The pruning that created this file deleted ~38 open items outright, on the owner's instruction:
> everything that was not on the path to publishing. They are recoverable from git
> (`git show 99d781a2:notes/backlog.md`) but are **deliberately not indexed here** — the standing
> rule is that a dropped item comes back only when it actually bites, and it is a higher priority
> then because it bit.

## Known non-defects and operational notes

Each of these was filed as a backlog item and is not a task. Kept because deleting them costs a
session to rediscover.

- **The cold-build cliff (3,981 ms vs 789 ms warm) is correct as-is.** Kernel *variable* state is
  never cached — the property that makes the freeze cache trustworthy — so a cold start genuinely
  cannot skip work unless the whole document is unchanged. **The waste is inherent to a correctness
  guarantee worth keeping.** Do not "optimise" it.
- **Do NOT grow `corpus/` toward real-world document shapes.** Two external documents
  (`rust-lang/book` + a real Quarto book) contain what `corpus/` has nowhere: `lang,attr` fences
  (734 occurrences), ` ```console ` (209), links with a non-`.tmd` extension (128), a `SUMMARY.md`
  chapter spine, **112 pages in one flat directory** (the largest corpus project is 14), and chapter
  files with **no front matter at all**. Only the two that earned a pin got one (127, 128, shipped
  2026-07-28); the rest are recorded so a later round does not re-derive them.
- **A companion e2e `EMFILE` failure is an inotify limit, not the code.**
  `fs.inotify.max_user_instances` was the kernel default of 128 while the desktop session held ~154;
  raised to 512 via `/etc/sysctl.d/99-inotify.conf`. The same limit throttles `taliesin preview`'s
  watchers: if previews stop hot-reloading, or VS Code refuses to start, check
  `find /proc/*/fd -lname 'anon_inode:inotify' | wc -l` against
  `/proc/sys/fs/inotify/max_user_instances` before suspecting the code.
- **Two companion e2e list-continuation tests are pre-existing failures** (27 pass / 2 fail at
  `origin/main`), and they fail at load ~2-3.4, not the "load ~6-7" once recorded. Treat both
  `pressEnterAfter` tests as unreliable at any load and **always compare against an alternating
  baseline run**, never a recorded number. The e2e runs `target/debug/taliesin`, which `cargo test`
  does **not** rebuild — `cargo build --bin taliesin` before believing any e2e result.
- **Moving the cursor into another chapter moves the preview to that chapter** (item 150,
  2026-07-30), on the passive path, not only on the explicit reveal. Deliberate: a preview showing a
  chapter you are not editing is stale, and the yank the reveal/mark split guards against is a
  cursor in the page *already* on screen, which still never navigates. If it turns out wrong, the
  answer is a better default, not a knob.
- **`crates/server/Cargo.toml` does not list tokio's `process` feature** though `kernel.rs` /
  `warm_pool.rs` / `exec.rs` all use it; it compiles only via feature unification. Pre-existing and
  load-bearing — a dependency trim can break the build in a way that reads as unrelated.
- **A *hanging* (not missing) interpreter costs ~161 s recovery** and is unhandled, downstream of
  the bounded `interp_id` probe in the warm-pool forkserver READY wait plus kernel-start retries.
  `kernel::tests::transient_start_errors_retry_but_missing_interpreter_does_not` shows the *missing*
  case is handled and the hanging one is not. Needs its own exec/kernel ruling if revived.
- **The M4 test's `sleep 300` stand-in kernel survives ~2 of 8 full-suite runs**, only when the
  build is cold. Measured, unexplained, argued test-only. Worth an hour only if a real kernel is
  ever seen outliving its pool.
- **Bare `@key` (D72) is declined**, not deferred: the diagnostic already ships, so nothing renders
  wrong silently, which makes it a feature question and not a defect. Edits
  `crates/core/src/cite/`; needs sign-off if revived.

## Tier 3 — demand-driven, and what was deliberately not filed

- **Dogfood: migrate the FL-weather book to Taliesin.** **Explicitly declined 2026-07-29** as
  unnecessary, kept only because the *class* of defect it would surface is real (the same class the
  external-document audit found). Revive on a concrete portability doubt, not on capacity.
- **Deliberately NOT filed** (2026-07-31 survey §5): the al-folio / academicpages **publications
  list** (a `.bib` rendered as a page with per-entry PDF/code/bibtex badges). It is the
  personal-homepage job, distinct from `cite_this.rs`'s outbound single-page citation, and filing it
  would widen scope on speculation. Revisit only if the author wants Taliesin to host their own
  academic homepage.

## Audit lenses — closed, do not open a new round

[AUDITS.md](AUDITS.md) is the round index and a *record*, not a menu. The 14-round slate
([spec](../docs/superpowers/specs/2026-07-27-audit-slate-design.md)) is **complete except R12**,
real-device mobile on Android, which needs the author's phone. Its priority order is in the spec: the
book drawer scroll lock first, then the `--host` QR flow, momentum scrolling and the dynamic viewport
toolbar, tablet widths, TalkBack. **Record explicitly that an Android round does not cover
WebKit/iOS**, or it will later read as full mobile coverage. **An audit's value decays to zero if its
findings never ship** — three waves have shipped, and the P1 queue is now the work.

**Two lenses remain un-run and both are blocked, not declined.** L3: `lsp.rs`, `complete.rs`,
`skim.rs` and `manifest.rs` post-date every lens that would have owned them, though the mutation
campaign has since pinned much of what one would look at. L6: a real external document, blocked on a
repository that is not on this machine.

Durable artefacts, so a later round does not rebuild them: the deck exemption register (R14), the
sensitivity/tradeoff register (R6), the D≥8 detection cluster (R7, now
[DETECTION-DEBT.md](DETECTION-DEBT.md)), the draft ACR (R9, now published in the guide) and the
external-document shape inventory (R11, item 129).

**One NEW family is open and proposed, not closed: feature-importance (FV).** The 2026-08-01
[feature-value audit](2026-08-01-feature-value-audit.md) opened it — the first round to ask *what
earns its keep* rather than *does it work* or *is it wanted*. It measured **adoption**, which is the
cheapest axis and not the strongest, and its own "Successor rounds" section carries six lenses with
a method and a **kill condition** each (a round that would only rebuild existing rows is not worth
running). Ranked as that round left them:

- **FV-2, ablation — run this next.** Delete a feature, run the corpus, count what breaks. Turns
  every cut verdict from a judgement into a measurement. **Run it across all of T2–T4 or not at
  all**: run against only the three already-named cuts it just rebuilds 203/204/209. **Commit
  first** — the mutation-testing footgun eats uncommitted work.
- **FV-3, cost-to-carry.** Churn + gates + defects per feature, i.e. the cost to *keep* rather than
  to *build*, which is the shape the first round is blind to. Ten-minute pre-check: if churn just
  concentrates in `render/mod.rs` and `build.rs` regardless of feature, the signal is noise — stop.
- **FV-4, cognitive surface.** Closest lens to the author's stated "fits the hand like a glove"
  goal: bloat is felt as *recall load*, not binary size. Count the vocabulary a user must hold vs
  how much the tool teaches at point of need.
- **FV-5, the LSP/editor value round.** ~11,300 LOC, the largest single investment in the tool, and
  **structurally invisible** to the first round (shell history cannot see a companion-spawned
  process). Blocked on method, not will; any instrumentation stays off by default.
- **FV-6, reader-side value.** Every number so far is *author* adoption. **Needs an outside human —
  do not run it as a desk exercise.**
- **FV-7, inherited vocabulary.** How much of the surface exists because Quarto had it (`columns`
  was one) rather than because it was chosen. Mind the triage doc's own degenerate-heading caveat.

FV-8 is not a lens: it is item **208**, the dated 2026-09-15 re-measure.

**Correction, noticed 2026-08-01 while adding the above:** the paragraph two above says L6 (a real
external document) is "blocked on a repository that is not on this machine". [AUDITS.md](AUDITS.md)
records that **R11 ran 2026-07-28** against `rust-lang/book` (112 files, 25,962 lines) and a real
Quarto book, producing items 127-130. L6 is **not** blocked and should not be re-scoped as such.

## Quarto catalog (policy, not a task)

**Owner ruling 2026-07-16: no sweep. Triage an area on demand, when you next work that area.** Before
consulting it read the triage doc's "three layers" section
([2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md)): the entries are the
asset and were well-grounded on 2026-07-03, but the heading status is degenerate and the executive
summary is misleading. A skeptic verdict is evidence, never a ruling (its "drop Atom feeds" verdict
was overruled; Atom shipped with autodiscovery).

## Do not re-add / re-scope

**One line per entry.** Detail lives in git, in [AUDITS.md](AUDITS.md), in the dated findings docs and
in [LESSONS.md](LESSONS.md) — look there rather than re-expanding this list. A batch's date and
branch are enough to find its commits.

### Shipped

- **2026-08-07 corpus completeness: `taliesin features corpus` reports 0 unused, and a test now
  enforces it** (closes 207 in full; the owner call was *pin*, not drop). The 15 it reported were
  four different things: **3 vacuous zeroes** (`hero.actions.text/href/primary` — the scanner
  descended one level and `actions:` is a sequence of maps at depth 2), **3 phantom catalogue
  entries** (`prp`/`exm`/`rem` survived in `XREF_LABELS` after the 2026-08-03 theorem retirement,
  reachable by nothing — not a theorem env, not a manual `{#prp-a}`), **2 by-design** (`csl`
  inert, `range` a `slider` alias) and **7 real gaps**. `features_cli.rs`'s
  `every_catalogued_feature_is_pinned_by_a_corpus_document` replaced the test that asserted the
  gap, so a capability landing without its pin doc now FAILS. **Closing the gaps found two
  shipping bugs neither the suite nor any audit had caught, because nothing exercised them:**
  `build --out` never copied `poster=`/`data-src=` assets (a built folder's poster and
  off-theme clip both 404'd, and the missing poster collapsed the element to the UA-default
  150px), and the book brand linked to a hardcoded `index.html` that a book whose `chapters:`
  does not start with `index.tmd` never emits (`corpus/theorem-book/`: dead title link on every
  page, both slots). Also new: `TAL-THM-KIND` — `theorems: shared:` validated its KEY but never
  its VALUES, so `shared: [theorem, lemna]` drew separate counters and `proposition` stayed
  accepted four days after retirement, both with a clean `check`.

- **2026-08-04 `::: {.debug}`, algorithm debug mode** (not a backlog item; requested directly).
  A `#| trace: true` `{python}` cell is recorded with `sys.settrace` in the warm kernel at build
  time; a `//| trace: true` `{js}` cell returns a generator drained client-side and **re-captured**
  when a `//| input:` control changes. One frame contract, two capture adapters, shared chrome in
  `assets/js/debug.js`: transport, a line cursor reusing `.tali-hl-ln-hl`, variables, call stack,
  stdout, four **closed-set** auto data views (numeric array → bars, other array/string → boxes,
  2-D → grid, in-range integer → labelled pointer caret), fullscreen, `.column-page` by default.
  Pinned by `corpus/debug/{sorting,leetcode,dp,custom-view}.tmd`; `site/showcase.tmd` shows binary
  search. **The step index publishes into a hidden `[data-tali-input]`, the same bridge `.scrolly`
  uses**. Stepping is a reactive input driven by a counter instead of scroll position, so no new
  reactive machinery. Load-bearing facts, all learned the hard way: the trace blob lands as a
  **SIBLING** of `.tali-debug` (the executor splices output as the next top-level block, never back
  inside the serialized composite div); `reads` **cannot** come from `settrace` and are derived by a
  per-line `Subscript` scan (an `AugAssign` target counts as a read, a plain `Store` target does
  not); a `line` event fires **before** that line runs, so `frame.line` is the line about to run and
  `frame.changed` describes what the previous one did; the trace rides as an `Output::Rich` blob so
  `_freeze` needed **no** change, but the traced flag had to be folded into the cache key or
  toggling `trace:` replays the old untraced output. `yield_scan.rs` refuses rather than guesses:
  a missed stamp costs a cursor position, an invented one would corrupt the cell. **`yield*` was
  the case the five adversarial tests missed** and it shipped broken through a whole-branch review;
  it now refuses that site. Non-finite floats must not reach `json.dumps` (bare `Infinity` is not
  JSON, and the widget vanished silently on any Dijkstra or DP cell using `float('inf')`).

- **2026-08-02 the adoption report and the two cuts it measures** (202, 203, 204, plus 201 and
  207's policy half, item now closed): `taliesin features <file|dir>` reports what a document uses and what nothing
  uses; `::: {.columns}`/`.column`/`ncol=` are **removed** for `{layout-ncol=N}`; `{{< dataset >}}`
  carries its own annotations and the `datasets:` key is retired. **Do not rebuild `features` on
  `vocab.rs`** (the offered-completions subset: 11 div classes where the validator has 16, and a
  shortcode list missing `input`/`dataset`, so it reports live features as unused) and **do not
  instrument the render to collect it** (the validator walk only sees divs that missed every
  feature class, and the warm incremental render is not a thing to tax for a report). **Do not
  re-file 204 as "derive it instead"** — its filed cause was false; the derivable half was already
  derived and the move was about shape, not redundancy. A withdrawn div class needs
  `RETIRED_DIV_CLASSES`: div classes are an OPEN vocabulary, so without it a leftover `.columns`
  gets **silence**, not a wrong hint (verified by mutation). A retirement trips **eight** gates,
  not six.
- **2026-08-02 the author-reported band + the scholar-block gate** (194-200): `citation_*` follows
  the page→site `author:` chain the Cite-this box and the JSON-LD already did (and stops at the site
  author, never the site title); the enlarged lightbox image closes the viewer; the top
  reading-progress bar, the mobile handle's bare grip and **sepia** are all deleted; the companion
  sidebar collapses; the project outline is in `chapters:` order. **Do not re-add the reading bar**
  (it duplicates the native scrollbar — ruling 2026-08-01, reversing 2026-07-06; `frac()` and
  `taliInitReadingProgress` stay, the resume pill and the book "Continue reading" slot live in the
  same function), **the TOC grip** (a bare 42x5 px bar is what read as "drag me"), or **sepia**
  (the a11y argument was made, considered and overruled). **Do not extend lightbox click-to-close
  to the video or the mermaid box** — a `<video>` click belongs to the native control bar and
  `.tali-lb-svg` is a pannable scroller. Withdrawing a theme needs **no migration code**:
  `theme.rs` validates the stored `tali-theme` against the allowed list and anything else reads as
  `auto`. 197's open design question was ruled in the build: a page `chapters:` never names is
  KEPT, flagged `listed: false` and grouped under `Unlisted`, while a website reports `book: false`
  and keeps path order. **`showCollapseAll` is unobservable from any extension API** (VS Code
  registers the per-view `collapseAll` command for every tree pane regardless) — that gap is in
  [DETECTION-DEBT.md](DETECTION-DEBT.md), the probe traps are in [LESSONS.md](LESSONS.md), and
  `crates/server/tests/reader_chrome_browser.rs` is where a reader-chrome browser pin goes.
- **2026-08-01 margin sidenotes + structured authors** (183, 184): a `[^note]` renders beside the
  line that cites it and there is **no gathered endnote section** (one copy, or all four text
  projections report every note twice). **Do not re-file the print fragmentation** — an in-flow
  printed note splits its paragraph at the marker; the fix is `float: footnote` in the print track
  (159) and it is recorded in `base.css`. **Do not "restore" the indexed affiliation form**: 184
  writes affiliations as NAMES and derives the superscripts, on purpose, so there is no
  `affiliations:` key and no index to desync. **Do not "simplify" the affiliation numbers back to a
  list marker** (an inline `<li>` has none). A pre-existing rail collision was fixed with it: on the
  two TOC grid modes any right-margin float, `.column-margin` included, drew on the rail's rows.
  Method lessons in [LESSONS.md](LESSONS.md); five of the two items' filed claims were false.
- **2026-08-04 `{pyodide}` WITHDRAWN** (MVP scope pass): client-side Python and its vendored
  15.7 MiB Pyodide+NumPy payload are **deleted**: runtime, enhancer, both cargo features, the
  corpus pin, the guide section, three `gates.sh` canaries and ~1,570 LOC of tests. **Do not
  re-add it.** The ruling was not size (item 205's cargo feature had already made a default build
  pay nothing): it was that the payload can only ever ship the stdlib + NumPy, since the tool does
  no network fetch, so the one workload that justifies a WASM CPython (`scipy`/`sklearn`) was
  designed out, leaving work `{js}` already does at zero marginal bytes. Adoption at withdrawal:
  author 0 / manual 1 / pin 1, and the marketing site never mentioned it. A withdrawn **cell
  language** needs a `RETIRED_CELL_LANGS` entry (`diagnostics/code_lang.rs`) for the same reason a
  div class needs `RETIRED_DIV_CLASSES`: fence languages are an OPEN vocabulary, so without one an
  author gets the generic "check the spelling", which is *wrong*: the spelling was right and the
  capability is gone.
- **2026-08-01 layout escapes** (181): there are **FIVE container modes, not three** (single-doc
  `body`, `body.has-toc`, `.tali-site-main`, `.tali-site-main.has-toc`, `.tali-book-main`). **Do
  not "simplify" the two TOC grids back to page-centred** — the rail is text with no background, so
  an escape grows LEFT there, right edge flush to the prose. `overflow-x: clip`, never `hidden`
  (which makes `<html>` a scroll container and kills every sticky element).
- **2026-07-31 print/PDF track** (159): `taliesin pdf` renders a typeset PDF *from the built HTML*
  via paged.js + CDP. **paged.js is load-bearing, not a fallback** (Chrome 150 implements `@page`
  margin boxes and `counter(page)` but NOT `string-set` or `target-counter()`, measured), it
  **cannot be driven from the Chrome CLI** (`--print-to-pdf` truncates at 2 pages at every
  `--virtual-time-budget`), and `auto: false` fires `config.after` **before** any pagination — stamp
  completion from the `preview()` promise. `eager_media()` exists because a paginated render never
  scrolls, so `loading="lazy"` images never load.
- **2026-07-31 reader affordances** (171, 172, 173, 56's backlinks half): live-HTTP mounts coverage,
  `publish --init`, the three C-READ/C-NAV affordances, and the first cross-chapter references either
  dogfooded book has carried. **Do not re-file C-READ-2's data half** (it is `{{< dataset >}}`, item
  176) and **do not re-file 173**. `@view-transition` moved into `base.css` and
  `corpus/tech-blog/custom.css` was deleted (finishing the 2026-07-11 audit's `#custom-css-mostly-dead`
  prescription); **do not re-add the two dropped prefetch mechanisms.**
- **2026-07-30 long-running cells** (175a + 175b): a cell is capped on **silence**
  (`TALIESIN_CELL_SILENCE`, default 600 s) instead of wall-clock, and a running cell **streams its
  output**. **Do not re-add a wall-clock default** on the theory that runaways are unguarded — a
  streaming runaway never goes silent and is caught by the output caps. Consecutive chunks of one
  stream now merge into a single output (measured: zero drift across the whole corpus).
- **2026-07-30 image optimization** (169): the build derives AVIF rungs behind a `_freeze/img/` cache
  and wraps the byte-identical `<img>` in a `<picture>`. **Do not re-file the WebP half** —
  `image-webp` encodes lossless only, so AVIF is the only pure-Rust lossy encoder. **Never depend on
  `ravif` directly** (it enables rav1e's `asm` feature, which hard-fails the release runners); use
  `image`'s own `avif` feature. `sweep_stale` deletes every output not in `keep`, and keep entries
  must be normalized — `image_derivatives_survive_the_sweep.rs` is the only test that catches it.
- **2026-07-30 editor scope completion** (`FEATURE-IDEAS.md` Session 3 ideas 75-80, 85): the editor
  scope is **CLOSED**. Ideas **67, 72, 74 and 83 were CUT on measured evidence** and **81 by owner
  ruling**; the only editor-surface idea still open anywhere is **86**, filed as item 175(d). The
  engine floor is `^1.101.0` with **`@types/vscode` pinned EXACTLY** (a caret resolves to latest and
  reopens the gap a test now closes).
- **2026-07-30 editor authoring gestures** (ideas 73, 84, 82): six paste/drop gestures,
  rename-repairs-references in both directions, clickable `file:line:` in the terminal. Two custom
  requests (`taliesin/insertEdit`, `taliesin/renameFileEdits`) hold every piece of `.tmd` knowledge;
  the TypeScript owns only the clipboard, the file write and undo grouping. **Do not re-file the
  ideas** or re-derive the four corrections written back into the ideas file. A pasted image lands
  *beside* the document (measured: 24 image refs beside vs 7 in a subdirectory).
- **2026-07-30 editor commands + dev panel** (165, 166, 162, 161): section move / heading promote,
  Format Document, the dev menu's edit annotations and the draft's per-section pass. **166's recorded
  blocker was rot** (`BlockOp::SetMeta` had always handled line shifts); the prose *reflow* is
  declined on measured grounds instead — 86 of 174 corpus documents are hand-wrapped, so there is no
  house style to enforce. `ctrl+shift+k` deliberately shadows `editor.action.deleteLines`.
- **2026-07-30 LSP editor ergonomics** (178, 177): inlay hints, folding, document highlight, selection
  ranges, plus visible math delimiters, with **zero new TypeScript** — the capabilities reach
  Zed/Neovim/Helix too. `didChange` is coalesced in a 120 ms window (measured: one publish on the
  largest guide page is 33 ms in a debug build, so debouncing alone sufficed and the anchor scan got
  no memo); `pending` is a **list**, or an edit to B discards the diagnostics owed to A.
- **2026-07-30 transclusion, datasets, site preview** (179, 180, 176, 160, 150): a chapter under a
  `_site.yml` previews as its *project*. **180 is closed by ruling** — all three inlay-hint kinds
  stay on by default.
- **2026-07-29/30 site bibliography + SEO board correction** (163, 168): `bibliography:` in
  `_site.yml`, merged UNDER each page's own, plus the unused-entry lint (site-wide by necessity).
  **168 was already shipped and the entry was rot** — `sitemap.xml`, `robots.txt` and the JSON-LD all
  emit; **grep `site/seo.rs` first.** `render_single_doc` now reads the nearest `_site.yml`'s key, so
  `preview post.tmd` and `preview <dir>` render one document.
- **2026-07-29 explorable cluster** (153-157): the cell-language registry with `{glsl}` as its proof,
  the `num` global (**keep it curated** — what a *drawing* cell needs, not a numeric library),
  `animate` + `point`, `tali.state`, and `tali.tex` / `tali.table` (**not** KaTeX-the-parser; only
  its CSS + fonts are bundled). The `animate` tick is a `type="number"` field, not `type="hidden"` —
  the latter hands every downstream cell the *string*. Two coverage illusions were deleted rather
  than shipped and live in [DETECTION-DEBT.md](DETECTION-DEBT.md).
- **2026-07-29 ruled-and-built batch** (101, 122, 71, 78, 149's buildable half, 18's doc halves, 41's
  `alt` half, 150's risk half): `LICENSE-OUTPUT-EXCEPTION.md` is an **additional permission under
  AGPL §7** covering what Taliesin *emits* (deliberately **no** per-asset licence headers); `check`
  names the interpreter it would use and still spawns nothing; both deck-on-touch behaviours; figure
  text on a data fill keeps its own colour.
- **2026-07-29 the demand tail's "LSP for language intelligence" line was DELETED as stale, not
  promoted** — `taliesin lsp` exists, the companion was rewritten as a thin client over it on
  2026-07-28, and the `#| label:` completion drift it cited is closed and pinned. **Do not re-file
  it.**
- **2026-07-29 first-hour + positioning** (144, 151, 87, 88, 94, 95, 96, 135, 136): eight CLI /
  diagnostic / LSP residuals, the two lying ui-audit probes, the first-run execution notice, and
  `docs/guide/using/choosing.tmd`. **Three filed causes were false.**
- **2026-07-29 `taliesin build site` no longer 404s its own CTA:** `site/build.sh` builds all 8
  projects into one tree, and **the ordering is load-bearing** — the parent build's `sweep_stale`
  deletes anything under the output dir it did not write, so a mount built first is silently swept
  away, and re-running `taliesin build site` alone afterwards puts you back to the broken tree.
  Pinned by `site_build_script.rs`.
- **2026-07-28 block model + docs gate** (138, 146, 143's path half): every block has exactly one root
  element; prose is gated against the tree rather than a needle list. **`notes/` and
  `docs/superpowers/` are excluded from that gate and must stay excluded** — they are dated records.
- **2026-07-28 deck harness** (112, 125, 113, 111): `deck.js` has a browser test; deck content is
  auditable at 0 violations across 100% of slides. It found **two shipped layout defects on its first
  run**, neither visible to any emission test. The eleven deck shapes 113 listed stay deliberately
  unbuilt.
- **2026-07-28 honesty + build cost** (91, 110, 115, 119, 126, 134, 143): `chromiumoxide` is an opt-in
  `headless-js` feature, off by default; not linting `draft:` pages is **ruled correct** and the
  defect was the silence; the ACR is published; DETECTION-DEBT.md is the live register.
- **2026-07-28 verified sweep** (85, 86, 97, 98, 99, 114, 123, 130): a `theme:` extension bundle is
  contained; no built page fetches off-origin; a shortcode source is a path, not a URL; both
  `jsconfig.json` include lists are globbed.
- **2026-07-28 critique-round client/LSP/manifest** (139, 140, 141, 142): rename validates the new
  name; `toc_html` stopped double-escaping an explicit heading id; the Cmd-K palette locks the
  background scroller; the web manifest stops shipping Taliesin's brand. **The splash colour is
  deliberately one light value** — a manifest cannot express an OS-conditional colour.
- **2026-07-28 reader cost** (150's Phase B half, 137, 124): the body typeface ships as
  content-hashed files, not base64 in the render-blocking sheet (**125 KB gzipped off every page's
  critical path**); the three conditional blobs are written only when something links them.
- **2026-07-28 publication readiness** (84, 89, 90, 92, 93): `tools/gates.sh`, `CONTRIBUTING.md` with
  the inbound relicensing grant, `ci.yml` + `release.yml` **guarded inert until the repo is public**,
  the measured install expectation and platform matrix, and "Coming from Quarto".
- **2026-07-28 launch blockers** (79-82, 109, 117, 118, 120, 121, 127, 128): `mounts:` is contained;
  `check` does not spawn a project-supplied interpreter; `--no-exec` covers `{js}`; a deck in a site
  is validated; comma fences highlight; a migrated link gets a did-you-mean. **`--no-exec` is
  deliberately NOT a sanitizer** (2026-07-03 CSP ruling).
- **2026-07-28 item 83 — the five pre-relicence MIT tags are deleted** (owner-approved; none had ever
  been pushed). All five commits stay reachable from `main`. **Never tag before the licence is
  settled**, and cut a release tag only from a tree whose `LICENSE` matches `Cargo.toml`.
- **2026-07-27 item 76 — a book has no right-rail TOC** (owner ruling, reversing 2026-07-06). The gate
  is `Site::page_toc`, ahead of the page's own `toc:`. **Do not re-scope as "give books their TOC
  back" or as "delete the rail everywhere"** — websites and single documents keep it.
- **2026-07-27 item 77** (the 72-75 residuals): shortcode arguments linted against a closed
  vocabulary; `TAL-SHORTCODE` is its own WARNING family; `favicon:` resolves like `logo:`. **A book
  with neither title nor logo still emits no brand link, deliberately.**
- **2026-07-27 mutation campaign** (58-69): every measured survivor in `crates/core`'s five
  post-07-18 files, the ten `crates/server` files and `lsp_nav.rs` is triaged and pinned. **Do not
  re-run it against the same scope.** Method in [LESSONS.md](LESSONS.md).
- **2026-07-27 item 66** (`404.html` links the shared `_assets/` bundle; its hrefs are root-absolute
  on purpose) and **item 67** (the `~/.local/bin/taliesin` launcher exits early for `__complete` only,
  24.3 s → 0.024 s per tab press; **`completions` is deliberately NOT exempt**).
- **2026-07-26 deck weight + headless bounding** (52, 55): a site deck went 4.6 MB → 7 KB via a
  separate `deck.<hash>.{css,js}` pair. **A deck cannot link the page's `app.js`** — `search.js` would
  steal Cmd-K. The standalone artifact stays 4.4 MB and self-contained on purpose.
- **2026-07-26 path parity** (50, 51, 57): `render_single_doc` decides the single-document containment
  root once (nearest `_site.yml`, **never `.git`**); `TOC_SHEET_MARKUP` is the one copy all four
  assemblers emit. **Do not re-scope as "give the single-file build the inferred root"** — that is a
  revert of `9359a2c`.
- **2026-07-26, earlier**: migration UX (53, 54); the mobile batch (42-49); owner rulings 24, 17 (book
  breadcrumb ruled **no**) and 2 (deck presenter tools declined); reporting surfaces 39, 40; demand
  probe #4 (`corpus/analyst`); AP1-R1 and DOCS-2/3/4/5.
- **2026-07-25 and earlier, closed:** AP7-1..5 (a11y), AP3-1, AP11-1 (`TAL-KERNEL`), DIAG-1, DOCS-1,
  AP3-3, PA-M3, PA-M13, PA-H1's residuals, the backlink-context + resume batch, book wayfinding, the
  hardening batch, book-level `theorems:`, live-executor mounts (F-04), book-aware `read`, AP8-1's
  output scrub, DET-1, the DX audit batch, `taliesin lsp`, DX17(a)+(b), the deck audit, the polish
  audit batch, the PMF builds, corpus coverage, the machine-facing audit, AI-native packaging, the
  R/Python ANSI leak, ungraceful-death reaping, and the `assets/js` `tsc` gate.

### Numbers retained, never reused

Each closed by a ruling or folded into another item, kept so a later round does not re-derive them.
**189 was issued twice** and the collision was fixed 2026-08-01: `{{< pdf >}}` + `license:` (Tier 3,
filed 2026-07-31) **keeps** 189, because
[2026-07-31-research-publishing-survey.md](2026-07-31-research-publishing-survey.md) names it as
"189 in Tier 3" and that file is a dated record that must not be rewritten. The scholar-block item
(filed 2026-08-01, P1) was the later claimant and **became 194**. This is the one sanctioned
exception to "never renumbered": two live items sharing a number is worse than one moving.
**25** — the pre-public flip procedure, folded into **100** (its options (a)/(b)/(c) were settled by
that ruling). **116** — the positional cascade vs a Python DAG, CLOSED, do not build; reactivity is
marimo's well-made claim while reproducibility is unclaimed by anyone, so tell the cascade story
instead. **132**/**133** — R8's value-stream pricing; a deck's defects are found by an *audience*,
the latest and most expensive point in the stream. **145** — retired into 137. **147** — retired
into 101. **151** — closed 2026-07-29. **182** — hover previews for citations and cross-refs; **filed
2026-07-31 against a false measurement and deleted 2026-08-01 unbuilt**, because `site/hover.rs` +
`code-enhance/12-link-preview.js` shipped it on 2026-07-06 (server-rendered, cross-page, with section
headings deliberately excluded).

### Decided against

- **"Adjacent slides bleed into the deck's letterbox" (DT-5, filed and RETRACTED 2026-07-27):**
  **false — the letterbox is empty.** The probe intersected each neighbour with the **viewport**
  instead of with its **clipping ancestor**. **Do not re-file it from a rect measurement**; the only
  valid evidence is a rendered pixel.
- **Deck presenter tools** (one-command publish, laser/spotlight, auto-advance): declined 2026-07-22
  and **re-declined 2026-07-26** on the same grounds — no real speaker ask. Revive only when the
  author actually presents from Taliesin. (`footer:`/`logo:` from that item did ship.)
- **A prose reflow / hard-wrap formatter** (2026-07-30, item 166): 86 of 174 corpus documents are
  hand-wrapped and 379 prose lines pass 100 columns, so there is no house style to enforce. The
  render-identical subset shipped instead, gated by `formatting_the_whole_corpus_renders_identical_html`.
- **WS op-message batching** (declined 2026-07-25 **on measurement, premise confirmed**): the worst
  case is 55 ops in one frame, but a warm edit is 32.2 ms of which the diff is 0.94 ms, so batching
  saves ~220 bytes on a 32,303-byte payload. Reopen only if render cost drops far enough that framing
  is measurable.
- **Item 29's reduction residuals R1 + T2** (closed 2026-07-25 without code): R1's `text_content` /
  `indexable_text` fork is deliberate and equalizing them would leak raw entities into `llms.txt`;
  T2's "three modules pre-scan" is partly rotted.
- **Deck-motion, whole item** (detail: [2026-07-24-deck-motion-audit.md](2026-07-24-deck-motion-audit.md)):
  Option A + residuals shipped; **(3) no-change** ruled; **(4) Option C (shared-element FLIP)
  declined — do not re-cost it a third time**. A coverage-weighted refinement of (5) measured *worse*.
- **A separate per-page outline artifact for the book drawer** (declined 2026-07-25 while building
  it): the index it would duplicate is already lazy-loaded on every page.
- **`drawer-typeahead`** (declined 2026-07-25): Cmd-K plus the drawer's collapsible outline covers it,
  and a second search-like box beside a Search button is a discoverability smell.
- **A "~N min read" label on a book chapter** (2026-07-25): `prose::word_count` excludes fenced code
  and math, so a code-heavy chapter is understated — and reading code is *slower* than prose, so the
  error goes into a promise about the reader's time in the wrong direction. (`is_article` is
  test-pinned; do not touch it.)
- **Flipping a book chapter's label to prefer `title:` over its `# H1`** (resolved 2026-07-25):
  measured across every book in the repo, only 3 of 48 chapters differ and in 2 the `# H1` is the
  *better* nav label. Resolved as documentation, not code.
- **CAD-as-code** (`{openscad}` / CadQuery cell → live 3-D preview; researched 2026-07-23, NOT built):
  technically feasible and legally green, killed on **demand**. **Do not bundle openscad-wasm (GPL).**
  Five named revisit triggers in [2026-07-23-cad-as-code-research.md](2026-07-23-cad-as-code-research.md).
- **2026-07-22 rulings:** DX16 update-nudge = **skip** (a version check is network egress that
  undercuts the offline-first identity); cross-ref label i18n = **defer**; item 9's design questions
  documented as intentional.
- **2026-07-12 wishlist cut to `FEATURE-IDEAS.md`** (revive only when a corpus doc needs one):
  cross-revision diff, repro manifest, List-of-Figures/Tables/Theorems, interactive tables,
  line-level code xrefs, image `dark=`. Reader text-size/line-spacing controls declined (a11y-exempt
  substrate in `14-reader-prefs.js`).
- **TODO / FIXME surfacing** (owner ruled 2026-07-10): no `level` concept exists, so a TODO warning
  would fail `check` on every draft. If revived, a preview-only `Diagnostic::info` beats re-plumbing
  a real `level`, and the scan must NOT reuse `prose::strip_inline`.
- **AI-native leftovers declined 2026-07-16:** `check --online` citation resolution, the
  numeric-claim-without-citation hint, and a per-page text/JSON sidecar.
- **Refuted by measurement (do NOT re-scope):** heading demotion **already ships**; `build` does not
  leak forkserver subtrees; the warm pool booting Python on prose-only builds is hygiene, not
  latency; dev attributes are 0.29% of page bytes; a `--version -dirty` marker is
  stale-by-construction; the `assets/css` stale-embed claim did not reproduce (re-verify for
  `assets/js` before any touch-render workaround); include symlink-loop SIGABRT does not exist;
  **decks pass path parity outright**, and `mounts:` differs from direct serving by 4 bytes.
- **`_redirects`/`_headers` preserved, never generated** (`build.rs` treats them as author-placed
  deploy metadata; `stale_sweep.rs` pins it).
- **Gate the gate:** a drift test that cannot fail is worse than none. Any new drift gate must be
  mutation-checked against exactly the shape it guards.
- **Library outsourcing decided against** (each verified vs the invariants): hayagriva/biblatex,
  schemars, jsonschema, morphdom/idiomorph, similar/dissimilar, clap, owo-colors, slug, html-escape,
  lightningcss/palette, IntersectionObserver/scrollspy libs, deck micro-helpers. Keep `two_face`
  extras filling gaps only (the bundled syntect set is consulted first and must win).
- **Reading-first defaults, research-validated keeps** (do NOT "fix"): serif body for long-form screen
  reading; ~70ch measure `--tali-maxw: 46rem`; right-rail scrollspy + width-gated sidenotes; scroll
  (not pagination) book reading; ship REAL bold/italic faces, never synthesized.
- **2026-07-06 decisions:** book pager stays bottom-only; book page-TOC fix-in-place, keep both nav
  surfaces; xref graph tool removed; focus mode stays ephemeral; deck overview keeps per-slide
  backgrounds; dev-menu + `#tali-progress` + reading-progress bar stay three separate signals
  (**the reading-bar half was REVERSED 2026-08-01 and the bar was DELETED 2026-08-02.** The other
  two signals are unaffected and stay separate.)
- **2026-07-18 PMF re-derivations:** the reader "Cite this" box (D70) was REVIVED and shipped as B1;
  the deck desktop "async handout" reading view stays CUT (do not re-open without a fresh ruling).
