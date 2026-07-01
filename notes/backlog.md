# qmd-fast backlog

**Scope: corpus-plus-roadmap.** "Done" still means the docs under `corpus/` render
correctly (the corpus is the regression net), but each new capability now ships pinned
by a target corpus doc. Output stays **HTML-only**. The active roadmap is
`BEYOND-QUARTO.md`.

> Kept deliberately small (read often). **Only open tasks live here.** Completed work is
> in git + the history docs: `BEYOND-QUARTO.md` (Beyond-Quarto waves), `DROP-QUARTO.md`
> (the native-rewrite), `AUDITS.md` (the three audit passes). Don't re-add `[x]` items.

## State (2026-06-30, `main` @ `6cdbc21` on origin, version 0.2.0; I commit+merge+push to main on request)

**Continue from here (two big merges landed on `main` 2026-06-30):**
1. **Reading-first redesign** (squash `280f451`): book is now ONE centred ~70ch column + a
   chapter drawer (no left rail); theme **follows the OS** (falls back light); accent is
   WCAG-AA (`--qmd-link`); body 18px; website TOC **auto-gated by heading count**; a deck
   opened as a link defaults to **scroll/reader** (`?qmd=present` to present); `--qmd-chrome-maxw`
   widens the chrome bars past the reading column.
2. **Deep-audit P1** (squash `6cdbc21`): all 11 correctness/robustness + all 7 a11y + the CI
   P1 items (kernel **KernelDied** fast-fail, shared `catch_unwind` guard, `--strict` honesty
   on malformed `_site.yml`/panics/`--out`/unknown-flags, bare-@-xref boundary, `{#id}` dedup,
   SetMeta multi-sourcepos→Update, lightbox keyboard, deck `inert`, reader-menu disclosure,
   24x24, `[role]` a11y gate, kernel+tsc CI jobs). Detail in "### Deep audit findings" below.

**New (2026-07-01): author book-testing triage.** The author read the built book and filed
21 bugs/ideas; all were validated against the code and deduped (an 8-lane audit workflow,
each finding adversarially re-checked) into the new "### Author book-testing findings
(2026-07-01)" section below. It adds two subsections the author explicitly asked for
(**Portability audit** and **Cross-page references**). Biggest confirmed bugs (all P2): the
static book right-TOC drops to the page bottom on narrow screens, cross-page Cmd-K search
silently dies on `file://`, and `$...$` math is not rendered in `fig-cap:`/`lst-cap:` captions.
Also surfaced and now tracked: the settled-but-unshipped **Taliesin rename** (its own `###`
section near the end of Open/next).

**Highest-value next (all detailed in their sections below):** the deep-audit **P2** clusters
(visual craft / sepia first-classing, deck engine, site/books silent-omissions, citations/math/bib,
performance) and the reading-first **P3 identity polish** (judgment-based; overlaps the deferred
marketing site, so confirm direction first). Lower: deep-audit **P3** (CLI/docs polish, security
hardening), the **extension-ecosystem audit** (incl. the dead liquid-glass `Reveal` extension), and
the Testing/CI residuals (headless `scanA11y` gate; tsc + `@ts-check` over all of `assets/js/*`).
Process unchanged: branch per feature, TDD, browser-verify via chrome-devtools, audit-qmd review,
Do-NOT-touch the exec/kernel zone + the single-editing-surface invariant.

All four formats render + deploy; the dev loop is strong (block-level incremental updates
with DOM-state preservation, warm server + Jupyter kernel, `_freeze` cache, Alt-click
click-to-source + reverse cursor sync, located/framed diagnostics, CSS hot-swap, Cmd-K
search). The author syncs to `origin` between sessions (agents do NOT push); the public
open-source release + site publish is still gated on readiness (the security token, now shipped).

**Shipped initiatives** (history in the docs above): DROP-QUARTO (fully native, no shims/
reveal.js/OJS). Beyond-Quarto **Waves 0-3 complete** + **Wave 4 built**: the schema
validator + JSON schemas, the live-edit benchmark, all six Wave-3 craft/breadth features
(walkthrough, tabset+margin, callout contract, typography, lightbox gallery, js-reactive
graph), the reverse-sync audit, and the VS Code editor companion (Phase 1, headlessly
verified). Recent backlog fixes: build-residue skip, `{js}`-import bundling, `mounts:`
build warning, `draft:` wire-up.

**2026-06-25 session** (all merged to `main`): the **`--host` session token** (LAN-snooping
defense, the OSS gate), the **per-chapter `{ file:, text: }` book-label override**, the
**client-side a11y audit** (diagnostics panel), the **corpus fidelity sweep vs Quarto**
(`qmd-fast-testbed/sweep_corpus.py` + `CORPUS-FINDINGS.md`; surfaced 2 bug-candidates,
see below), the **project-structure & reserved-names** docs reference, **corpus hygiene**
(bayesian-book→bayesian-website rename, README native-schema fixes, liquid-glass deck
vendored offline), and the **tour.qmd embed** (was an orphaned deck).

**2026-06-25 session (later): the reader-experience cluster** (the active thrust; all merged;
built ultracode-style with design + adversarial-review workflows). A set of **reader-side,
read-only** enhancers (state in the reader's own `localStorage` keyed by `location.pathname`;
never writes the author `.qmd`; in-scope per the single-editing-surface invariant), all
`qmdEnhancers`-registered in `crates/core/assets/js/code-enhance.js`, deck-skipped,
corpus-pinned under `corpus/reader/`, spec'd in `docs/superpowers/specs/2026-06-25-*`,
chrome-devtools-verified: reader **display prefs** (theme incl. sepia / text size / width /
**line spacing** / **letter + word spacing** — the last two completing **WCAG 1.4.12 Text
Spacing**, all applied pre-paint via `render/theme.rs`), **reading progress + resume**,
**highlights + index + Markdown export**, **section bookmarks**, a **selection toolbar** (copy /
quote / native W3C text-fragment share link), and a **modal focus-trap** (lightbox + Cmd-K; the
reader menu stays an untrapped popover). All controls consolidated into one **Reader menu**
(`qmdInitReaderMenu` + `window.qmdReaderMenu.addSection`). Also **Cmd-K search relevance**
rewritten hand-rolled (multi-term / prefix / fuzzy, replacing the single `indexOf`; MiniSearch
rejected — it inlines on every TOC page). Idea pool = repo-root `FEATURE-IDEAS.md`.

**2026-06-26 session: reader cluster continued** (all in `code-enhance.js`, deck-skipped,
chrome-devtools-verified, specs under `docs/superpowers/specs/2026-06-26-*`): **read-state TOC**,
**copy-as-citation** (Cite → BibTeX), **anchor copy-link** (`#` on hover → deep link),
**hover cross-reference cards** (`link-preview`, `#`-stripped + pinnable), **focus / reading
mode**, and **read-aloud study mode** — a "Listen" control (reader menu + floating mini-player:
play/pause, prev/next block, speed, voice) that speaks the page from the block in view, prose
sentence-by-sentence with the sentence highlighted (CSS Custom Highlight API, distinct
`--qmd-ra-highlight` token) + auto-scrolled, code announced then **line-stepped** (line Ranges,
no `.qhl-ln` dependency), figures/equations/tables announced; offline (Web Speech), ephemeral
position, rate+voice persisted; injectable `window.__qmdSpeakImpl` speak seam for headless test.
Pinned `corpus/reader/read-aloud.qmd`. Math a11y was found **already handled** (KaTeX
`htmlAndMathml` emits MathML + `aria-hidden` visual layer), so not rebuilt. **Docs gap noted:**
the reader-experience cluster has no User Guide page yet (only `corpus/reader/` + specs) — a
worthwhile follow-up docs task covering the whole cluster.

**2026-06-26: Pillar III `{input}` reactive controls shipped.** A built-in shortcode
`{{< input name="k" type="slider" … >}}` (5 types: slider/number/checkbox/text/select) emits a
static, keyboard-accessible labeled control feeding the shipped `{js}` reactive graph — "drag
the slider, the chart updates" with no `//| viewof` boilerplate. **Authored as a shortcode, NOT
a `:::` div** (a bodyless div is dropped by `group_divs`; emitting empty containers would touch
the Do-NOT-touch `:::` machine). Reuses `registerInput`/`scheduleFrom` (additive ~15-line
`qmd-js.js` scan) + `validate_input` (located diagnostics) + the `.qmd-input` CSS. Pinned
`corpus/reactive/inputs.qmd`; browser-verified incl. transitive propagation. (Same surface-fork
lesson as elsewhere: bodyless controls belong in the shortcode seam, not the div machine.)

**2026-06-26: Pillar III scrollytelling `:::{.scrolly}` shipped.** A sticky visual stage (the
non-`.step` inner blocks) beside a scrolling `.step` column; the active step (a new
`scrolly.js` enhancer reusing the walkthrough IntersectionObserver band) sets
`data-scrolly-state` on the root (pure-CSS effects) and, with `name=`, drives a hidden
`data-qmd-input` so a sticky `{js}` cell re-runs via `//| input:` — **reusing the shipped
`{input}` registration with NO `qmd-js.js`/reactive-runtime change** (scrollytelling = a
reactive input driven by scroll). Generalizes the `.code-walkthrough` machine; `.step` gained
`state=`→`data-state`; `validate_scrolly` for located diagnostics. Pinned
`corpus/explorable/scrolly.qmd`; browser-verified (scrollIntoView flips the active step +
state; the `{js}` chart re-runs linear↔quadratic per scene; 0 console errors).

**2026-06-26: Pillar I prose-lint shipped.** An opt-in (`prose-lint: true | { banned: [...] }`),
markdown-aware prose linter (new `crate::prose`): doubled words + weasel words + custom banned
terms, emitting located click-to-source warnings through the existing diagnostics channel
(skips code/math/links/HTML/fences). `prose-lint` added to `KNOWN_KEYS` + `PROSE_LINT_KEYS`
(nested-validated; the generated JSON Schema regenerated/blessed). Fully Rust-tested (unit +
`corpus/diagnostics/prose.qmd` exact-warning test); no browser needed. **Passive voice deferred**
(too noisy). Documented in `docs/guide/reference/configuration.qmd`.

**2026-06-26: Pillar I `qmd-fast check` CLI shipped.** A static, kernel-free subcommand that
renders a file or site in memory and lists every located diagnostic from the warning channel
(schema/front-matter/`_site.yml`/cell-option/container validation + did-you-mean, broken `@xref`,
unknown shortcodes, missing bibliography, opt-in prose-lint), `--format human|json`, exit 1 on
any finding (a CI gate). Pure `crates/server` addition (`collect_diagnostics`/`cmd_check`) reusing
`render_document_with_includes` + `cite::validate_xrefs` + `Site::render_page_doc_warned`; no core
change; serde_json (already a server dep) for JSON. Tested in `main.rs` (file/clean/empty-site +
both formatters). Documented in `docs/guide/reference/cli.qmd`. **Deferred:** `--format sarif`
(needs a rule/ruleId field on `Warning`), a11y/dead-link checks, an `--exec` runtime-error mode.
Also: the reader + interactive User Guide pages shipped (`docs/guide/using/{reading,interactive}.qmd`),
closing the reader/explorable docs gap.

**2026-06-26: reader polish bundle shipped** (FEATURE-IDEAS #16/#19/#23/#55; all client-side +
CSS, no core change). (1) **Typography polish** (base.css, global): `text-wrap: pretty` on prose,
`balance` on headings/captions, `orphans/widows: 2`, figure `break-inside: avoid` + caption
`break-before: avoid`. (2) **Skip-to-content link** (`qmdInitSkipLink`): visually-hidden-until-focus,
resolves the content container (`main`/`#qmd-root`/first block) at runtime, focuses it on activate.
(3) **Keyboard reader** (`qmdInitKeyboard` + a `window.qmdOpenSearch` export in search.js): `?`
cheatsheet (focus-trapped via `qmdFocusTrap`), `/` search, `←`/`→` prev/next chapter (the book
anchors), all guarded against typing/modals/focused controls. Browser-verified on the guide book
(arrow nav navigated reading→interactive; cheatsheet; `/`; skip link; guard; 0 console errors).
Documented in `docs/guide/using/reading.qmd`. Deferred: hyphenation (#17), forced-colors/contrast
(#20), hanging punctuation (#22).

**2026-06-27: `--bare` build output + content-gated enhancer JS shipped** (spec
`docs/superpowers/specs/2026-06-27-bare-build-and-enhancer-gating.md`; audit-qmd reviewed, 5
low/med findings — all test/doc-coverage gaps, pinned). A new `enum OutputMode {Preview, Build,
Bare}` threaded onto `PageParts` (live preview stays `Preview`, byte-identical). **Phase 1** (every
build, no flag): `code_scripts_for(body, mode)` content-gates the *separate* enhancers
(mermaid/`{js}`/walkthrough/tabset/scrolly) to the DOM a page actually contains; `code-enhance.js`
(reader menu + skip-link/keyboard a11y) still ships on every page (the coarse-gate-it-off-prose idea
was **rejected** — that file carries the whole reader+a11y layer, not 4 small features, so dropping it
from prose was an a11y regression). **Phase 2** `qmd-fast build <file> --bare` (single-doc only):
guaranteed **zero `<script>`/zero CDN**, CSS-only theming (`bare_theme_css` rewrites
`html[data-theme="dark"]`→`:root`; forced theme hard-coded, else OS-following via
`prefers-color-scheme`), math kept (server-rendered), `{js}` script blocks stripped from the body,
decks/sites refused, `{js}`/Mermaid drops warned (never silent). Pinned `corpus/bare-draft.qmd` +
unit/corpus tests (gating, all 3 theme branches, site-path gating, click-to-source survives the strip);
browser-verified light+dark, 0 console msgs; ~58% smaller than a normal build. Documented in
`docs/guide/reference/cli.qmd` (also documented the previously-undocumented `--strict`).

**2026-06-27 (later): release-hardening batch shipped** (branch `release-hardening`, 5 parallel
worktree lanes + coordinator integration; all merged on the branch, full `cargo test` green +
chrome-devtools-verified). **(1) `check` superset extended** (`crate::diagnostics`): broken
internal/relative links + cross-page link/anchor existence (site page registry), local **video**
paths, and a static mirror of the `{js}` reactive graph flagging dangling `//| input` names +
dependency cycles (Kahn's). External `http(s)` links are deliberately never fetched (offline +
deterministic). Pinned `corpus/diagnostics/links.qmd`. **(2) a11y/touch chrome parity:** every nav
landmark aria-labelled (Primary / Chapters / Pagination / Table of contents — incl. `site/chrome.rs`
+ the preview TOC placeholders), one shared `--qmd-focus` `:focus-visible` ring, lightbox dialog
name, deck **slide roles + "Slide N of M"** + a polite live region, `forced-colors`/`prefers-contrast`
blocks, and **server-side** skip-link + focusable `<main>` + real image `alt` (no longer JS-only).
**(3) `.bib` fixes** (cite.rs, author-greenlit, byte-stable IEEE guard): LaTeX accents→Unicode,
brace-protected corporate authors render whole, `@string`, `@inbook`/`@incollection` booktitle+pages,
auto-References dedup vs a manual `# References`. **(4) render/asset:** Mermaid offline now shows a
visible `[data-mermaid-error]` banner (+ `QMD_FAST_MERMAID_URL` to self-host) instead of failing
silently; `figure height=` honored; `{{< video …?query >}}` ships intact; `THIRD_PARTY.md` CDN
inventory corrected. **(5) CLI onboarding:** `qmd-fast init` scaffold, README install/prereqs,
`usage()` advertises the `<dir>` site mode, unknown-command did-you-mean; **site `build` honors the
author's `404.qmd`** (no overwrite, excluded from search). Cross-lane snag caught at integration:
lane-D's corpus video tripped lane-A's new local-video rule → switched it to a (realistic) remote
token URL.

## To resume

**Working method:** branch per feature; brainstorm if there's a fork; write a spec under
`docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or
the `@vscode/test-electron`/relay harnesses for the extension); fast-forward merge locally;
mark the item here. Caveat: any review subagents must use read-only git (`git diff a..b`,
never `git checkout`) — they share the working tree.

**Author policy (feature-first):** finish framework features before marketing-site work;
the `live-edit-hero-demo` clip + the "Marketing site" section stay deferred until then.

**Pending author action:** F5-accept the VS Code companion — `cd editor/vscode && npm
install && npm run build`, then F5 and run the `editor/vscode/README.md` checklist (cursor
→ block highlight; Alt-click → source). Report anything off and I'll fix it.

## Open / next

### Author book-testing findings (2026-07-01)
Source: the author read through the built book and filed 21 bugs/ideas. Each was validated
against the code by an 8-lane audit workflow (one lane per cluster), then a fresh skeptic
re-checked every "already fixed / already tracked / not reproducible" call so nothing real got
dropped. Tally: **6 confirmed bugs, 3 design-decisions/explanations, 10 feature-requests or
open questions, 2 not-reproducible-in-code (need a repro).** The recurring theme matches the
deep audit: **file:// / no-server "portability" is under-built, and silent failure is the
default.** Verdicts + file:line evidence below.

**Priority order / work queue (2026-07-01, being ticked top-down).** Confirmed bugs first,
then polish, then design-forks; items needing the author's input are parked at the bottom.
Tick items in the detailed subsections as they land.
1. **C2** math in `fig-cap:`/`lst-cap:` captions (P2, Rust) [DONE]
2. **E3** `file://` cross-page search: script-loadable index (P2, Rust) [DONE]
3. **E2a** gate dead click-to-source out of static builds (P2/P3, Rust) [DONE]
4. **A3** narrow-screen static book right-TOC: ship the pull-up sheet (P2) [DONE]
5. **B1** cross-block selection toolbar decouple (P3, JS) [DONE]
6. **B3** themed chevron on folded `<details>` (P3, CSS) [DONE]
7. **F2b** cross-page number for non-heading anchors (P3, Rust) [DEFERRED: needs a design call, see below]
8. **B2** hover card: add pin + fix scroll-inside dismiss (P3, JS) [DONE]
9. **F2a** cross-page hover preview (P3) [DEFERRED: needs the cross-page index, groups with F2b]
10. **F2c** book-wide bookmarks (P3, JS) [DONE]
11. **B4a** focus-mode native fullscreen (P3, JS) [DONE] · prev-arrow half still open (needs your input)
12. **A1** number the chapter title for continuity (P3, Rust) [DONE]
13. **G1** Google-Scholar `citation_*` meta (P3, Rust) [DONE]
14. **A2** document the right-TOC `>=3` gate (P3, docs) [DONE]
15. **D1** `.code-walkthrough` wide-desktop layout: code pinned top (P3, CSS) [DONE] · `.scrolly` left as-is
16. **E2b** self-contained portable site/book build (P2) [DONE via E2a+E3 + docs; mermaid-vendor deferred]
17. **E1** `file://` theme-isolation [DONE: accepted (browser limit); OS-follow default mitigates]

*Parked, needs the author's input (not tickable solo):* **C1** (need a repro), **B4-arrow**
(which viewport?), **F3** cross-ref graph (backlinks-first vs graph-canvas decision), **G3**
publishing workflow (product design), **H1** `.qmd` format-on-save (brainstorm the
click-to-source line-stability tradeoff), **H2** Taliesin rename (owner-gated identity call),
**H3** FL-weather migrate (needs the external project).

#### Portability audit (file:// / no-server output) [NEW, the author's explicit "portability audit" ask]
The self-contained-zip / email-a-book story. `--bare` (shipped above) solved this for SINGLE
docs only; sites and books have no equivalent, and several things silently break when the built
output is opened from disk with no dev server.
- [x] **Cross-page Cmd-K search silently dies on `file://`** (P2, confirmed).
  DONE 2026-07-01 (branch `author-testing-fixes`): the index is now written/served as a
  `search-index.js` script that assigns `window.QMD_SEARCH_INDEX`, and `search.js` loads it via a
  dynamically-injected `<script>` (works under `file://`, still lazy: first palette open only)
  instead of `fetch` (CORS-blocked on `file://`). On a genuine load error it now shows "Search index
  failed to load" instead of a silent empty palette. Touched: `search.js` (loadIndexThen +
  error row), `site/mod.rs` (wire `search-index.js`), `build.rs` (write the `.js`), `serve_site`
  (route `/search-index.js` + mounted guard, `text/javascript`). TDD test
  `cross_page_search_wires_a_script_loadable_index_not_a_raw_fetch` (core/tests/search.rs); core +
  server suites green; tsc clean; browser-verified from `file://` (169-entry index loads,
  cross-page results resolve, 0 relevant console errors). *Noted in passing (out of scope):* the
  search palette + wiring only ride on pages that have a TOC, so short chapters (`<3` headings)
  have no Cmd-K at all — ties into A2/A3.
- [x] **Self-contained site/book build** (P2) — mostly delivered incidentally by E2a + E3.
  DONE 2026-07-01 (branch `author-testing-fixes`): `build <dir> --out <folder>` is now an offline,
  self-contained folder for the common case, WITHOUT a new flag: E2a stripped the dead click-to-source
  from static output, E3 made cross-page search load via a `<script>` (works on `file://`), and math
  (KaTeX + inlined fonts) / `{js}` runtimes (vendored d3+Plot) / nav / theme / bookmarks / the mobile
  TOC sheet are all already local. **Verified**: a built guide-book page opened from `file://` makes
  exactly ONE external request (see below); everything else is `file://`. Documented in
  `docs/guide/reference/cli.qmd` (new "Portable, offline builds" section). `check docs/guide` clean.
  - [ ] **Remaining gap (deferred, distinct decision): vendor Mermaid offline.** The one external
    request is `cdn.jsdelivr.net/.../mermaid.min.js` — a book WITH Mermaid diagrams isn't fully offline
    unless `QMD_FAST_MERMAID_URL` points at a local copy (documented). Closing it means vendoring
    `mermaid.min.js` (~2–3 MB) into the repo + copying it to the build (content-gated to mermaid pages)
    — a repo-bloat call worth making deliberately, not in a sweep. Until then, the escape hatch +
    error banner (from the release-hardening batch) cover it.
- [x] **Dead click-to-source in static output** (P3, confirmed).
  DONE 2026-07-01 (branch `author-testing-fixes`): removed `STATIC_CLICK_TO_SOURCE` from the static
  page assembler (`page.rs`; `scripts_pre: ""` + deleted the const). Built/rendered pages (single-doc
  `render`/`build` + sites/books) no longer draw a `.qmd-hl` outline or `console.log` on every click.
  Click-to-source is now live-preview-only (client.js, which is unchanged). TDD test
  `static_page_has_no_dead_click_to_source_outline` (render/tests.rs); core suite + clippy green.
- [x] **Book theme not book-wide from disk** (P3) — ACCEPTED (browser limitation, not a bug).
  Resolved 2026-07-01 as a known caveat: NOT a keying bug (the theme key is a single global
  `qmd-theme`, `theme.rs:86,140`); the symptom is browser `file://` localStorage origin isolation
  (Firefox isolates per file path; **Chromium shares one bucket, so it already works there**).
  **Mitigated by default:** the theme follows the OS (`prefers-color-scheme`) unless the reader
  explicitly toggles, so a fresh `file://` page is themed correctly regardless; only a manual toggle
  fails to propagate across chapters on Firefox-from-disk. Accepted as "hosted / dev-server (or
  Chromium) for a book-wide manual toggle"; documented alongside F2c's book-wide-bookmarks caveat. A
  non-localStorage carrier would be hacky for a narrow case; revisit only on demand.

#### Cross-page references [NEW, the author raised this twice]
`site/xref.rs` is a deliberately lightweight source-scan (page URLs + section numbers only), so it
lacks parity with the richer in-page ref path. These generalize the existing theorem-only bullet
under "Release regression-hunt deferrals" (cross-page theorem refs drop the number); widen that
item rather than duplicating it.
- [ ] **Cross-page number lost for ALL non-heading anchors** (P3, DEFERRED — needs a design call).
  `xref.rs:108-109` pushes an empty number for every `fig-`/`eq-`/`tbl-`/`lst-`/`thm-` anchor
  (headings resolve book-wide via `section_number`; the rest render bare cross-page). *Investigated
  2026-07-01:* not a quick fix. The xref registry (`scan_xref_targets`) is a deliberate **source-scan
  with NO render** (avoids a second execution pass); but fig/eq/tbl/lst/thm numbers only exist AFTER
  render (`render/mod.rs` `xref_registry`, built by `apply_table_captions`/`number_theorems`/figure
  counters). So a correct fix is either **(a)** a render-harvest: render all pages once, collect each
  page's anchor→number map (expose it on `RenderedDoc`), enrich `Site::xref_targets`, THEN
  `resolve_cross_refs` — reversing the no-double-render design (acceptable for `build`, slows preview
  startup); or **(b)** replicate the full figure/theorem counting (incl. number-within + shared
  counters) in the source scan — high drift risk vs the render. Section numbers work only because
  heading counting is trivial. Recommend (a), build-gated, or accept the bare label. Touches
  load-bearing numbering — do it deliberately, not in an autonomous sweep.
- [ ] **No hover preview for cross-page refs** (P3, DEFERRED — needs the cross-page index, groups with
  F2b): `12-link-preview.js` only fires on same-page `#` links (`getElementById`); a cross-page xref is
  `{url}#{anchor}` (`xref.rs:229`), whose target block lives on another page. *Investigated 2026-07-01:*
  a real preview needs cross-page TARGET CONTENT, i.e. a site-wide anchor→preview index (same
  render-harvest gap as F2b). Lighter path worth a focused effort: for `@sec-` refs, reuse the already-
  shipped `search-index.js` (it has heading text + a section-body snippet per `sec-` anchor) to render a
  text card — lazy-load the index on first cross-page-ref hover, build the card from the entry; `fig-`/
  `eq-`/`thm-` cross-page previews still need rendered HTML (the F2b index). Not a quick sweep item.
- [x] **Reader bookmarks are per-page, not book-wide** (P3).
  DONE 2026-07-01 (branch `author-testing-fixes`): a book now shares ONE bookmark store across chapters,
  keyed by the resolved book-root URL (the topbar `.qmd-book-brand` href, stable + present on every
  chapter); single docs / websites stay per-page. Entry schema is now `{page, anchor, block, label}`;
  the reader-menu list shows bookmarks from ALL chapters (a cross-chapter one carries a ` · <page>`
  suffix and jumps to `{page}#{anchor}`; a current-page one scrolls + flashes in place), and the margin
  star marks only current-page entries. Browser-verified from `file://` (Chromium shares the `file://`
  localStorage bucket): capture stores the right schema; the menu lists a current + a cross-chapter
  bookmark. **Caveat (ties to E1):** some browsers (Firefox) isolate `file://` localStorage per path, so
  "book-wide" only spans chapters when served over http(s); logged in the portability audit.
- [ ] **Cross-reference graph / backlinks (Obsidian-style)** (strategic, open question): "referenced
  by" backlinks already fit (FEATURE-IDEAS #27: read-only, build-time, reuses `xref.rs`'s forward-ref
  scan) and are the cheap high-value core. A full interactive force-directed graph canvas is a
  distinct, scope-risky second nav surface. Recommendation: ship backlinks first, defer or decide on
  the graph. Read-only + HTML-only if pursued (no new write path, single-editing-surface holds).

#### Reader interactions
- [x] **Cross-block text selection shows NO toolbar** (P3, confirmed).
  DONE 2026-07-01 (branch `author-testing-fixes`): `onSelect` (`16-highlights.js`) now computes a
  `single` flag; a single-block prose selection gets the full toolbar incl. Highlight, while ANY other
  selection (cross-block, or code/math where the offset walk skips) still gets Copy/Quote/Share/Cite
  (they use `pending.text = sel.toString()` only) with just the Highlight button hidden. Remove-highlight
  path resets `btn.hidden`. Browser-verified from `file://`: cross-block selection shows
  Copy/Quote/Share/Cite (Highlight hidden), single-block shows all five, 0 relevant console errors.
  Cross-block *highlighting* stays scoped out (per the reader-experience note below).
- [x] **Hover cross-ref card is not pinnable** (P3, confirmed + doc drift).
  DONE 2026-07-01 (branch `author-testing-fixes`): the real bug was the `scroll` listener
  (`12-link-preview.js`) firing on ANY scroll incl. scrolling INSIDE the overflowing card, so it
  dismissed the moment you tried to read past the fold. Now: (1) a scroll whose target is inside the
  card no longer dismisses it (you can scroll the `max-height:50vh` overflow); (2) click-to-pin
  (`pinned` state) keeps it open through mouse-leave + page scroll, with Esc / outside-click to
  release. Resolves the "pinnable" doc-drift (backlog:78 / FEATURE-IDEAS #52 now true). Browser-verified
  from `file://`: card-internal scroll keeps it open, page scroll hides it, click pins (survives page
  scroll), Esc closes + unpins. Core suite + tsc green.
- [x] **Focus mode: native fullscreen** (P3, feature).
  DONE 2026-07-01 (branch `author-testing-fixes`): focus mode now enters native fullscreen too (the
  author's "hide everything but the text" ask). `03-focus-mode.js` `setFocus` calls a best-effort
  `goFullscreen` (requestFullscreen on enter / exitFullscreen on leave; both `f` and the menu button
  are user gestures; degrades silently where blocked), and a `fullscreenchange` sync drops focus mode
  if the reader leaves fullscreen via F11/Esc, so the two stay coupled. Browser-verified: `f` toggles
  focus on/off, Esc exits, no JS errors (the requestFullscreen rejection under *synthetic* test events
  is caught; a real keypress engages fullscreen with no warning). Coupled by design per the request;
  trivially decouple-able to opt-in if it reads as jarring.
  - [ ] **STILL OPEN — "prev arrow too far left" (needs your input):** does NOT reproduce in code — the
    book pager (`.qmd-book-postnav`/`.qmd-book-prev`, `site.css:200-215`) already sits inside the centred
    reading column, and there is no viewport-edge prev arrow. Which view/viewport showed it?
- [x] (Polish) **Themed chevron on folded `<details>`** (P3).
  DONE 2026-07-01 (branch `author-testing-fixes`): folded code (`qmd-code-fold`) + collapsible proofs
  now hide the browser-default triangle and draw a `currentColor` caret that rotates from right (▶) to
  down (▼) on `[open]` (base.css). Callouts left alone (their icon header is already an affordance).
  Browser-verified from `file://` (chevron rotates 45°→135° on open, native marker hidden, source
  reveals). Directly addresses the author's "collapsed cells need an arrow indicator."

#### Book TOC / layout (ties into the reading-first redesign)
- [x] **Static book right-TOC useless on narrow screens** (P2, confirmed).
  DONE 2026-07-01 (branch `author-testing-fixes`, author chose "ship the pull-up sheet"): the mobile
  pull-up TOC sheet is now shipped into static builds (was preview-only). New self-contained
  `web-client/toc-sheet.js` enhancer (self-inits; drag/tap/keyboard/backdrop/a11y-inert, ported from
  client.js but standalone), bundled build-only via `render::toc_scripts()` (`TOC_SHEET_JS`); page.rs
  emits the `#qmd-toc-handle`/`#qmd-toc-backdrop` chrome on any TOC page. **Progressive enhancement:**
  the server ships the body WITHOUT `qmd-toc-sheet` (so no-JS keeps the in-flow TOC, never off-screen)
  and `toc-sheet.js` adds the class at runtime, then wires the sheet. client.js (preview) untouched (no
  double-wiring: `toc_scripts` is build-only). TDD smoke test `static_toc_page_ships_the_mobile_pull_up_sheet`
  (render/tests.rs); core+clippy+tsc green; browser-verified at 390px (handle at bottom, tap opens the
  sheet, backdrop closes, `aria-expanded` correct) and 1440px (handle hidden, TOC = sticky sidebar).
  *Real-device follow-up:* the drag-gesture physics (drag up to open / drag down to dismiss) were ported
  but only tap/keyboard/backdrop were exercised headlessly; confirm drag on a touch device.
- [x] **Book section-number UX confusion** (P3, explains "strange numbers in the left TOC").
  DONE 2026-07-01 (branch `author-testing-fixes`, author chose "number the chapter title"):
  `number_chapter_headings` (`site/chapter.rs`) now numbers the chapter's title block too — a new
  `prefix_title_number` inserts the bare chapter number just inside `<h1 class="title">` (the title is
  a `<header class="qmd-title-block">`, so `heading_level` never saw it), without advancing the h2+
  counters. So a numbered chapter reads "4 Executable content" and its "4.1 Code cells" flows naturally
  instead of a number appearing from nowhere; unnumbered prefaces (index / `{.unnumbered}`) stay
  unnumbered. TDD unit tests (`numbers_the_chapter_title_block`, `detects_the_title_block_but_not_a_heading`);
  core suite green; browser-verified on the guide book. (The left Chapters drawer was already correct.)
- [x] **Right-TOC auto-gate documented** (P3, was "reads as arbitrary").
  DONE 2026-07-01 (branch `author-testing-fixes`): added a **Table of contents** note to
  `docs/guide/reference/configuration.qmd` explaining the `>=3` heading auto-gate, the per-page `toc:`
  override (explicit always wins), and the new narrow-screen pull-up sheet. `check docs/guide` clean.
  (Kept as a docs clarification rather than changing the gate; the correlation the author spotted is
  real and explained below.)
- [ ] ~~Right-TOC auto-gate reads as arbitrary~~ (superseded by the documented note above): the
  "double numbers ↔ has a right TOC" correlation the author spotted is REAL, and both are downstream of
  one fact, how many
  subsections a chapter has. The `>=3` heading gate (`mod.rs:565`, `toc_entry_count`) hides the right
  TOC on short chapters while `chapter.rs` still numbers their subsections, so which chapters get a
  right rail looks random. Consider documenting the per-chapter `toc:` override, or surfacing the gate
  decision. (The numbers do not cause the TOC; the section numbers just make the shared cause visible.)
- [x] **`.code-walkthrough` wide-desktop layout** (P3, design/feature, author's explicit request).
  DONE 2026-07-01 (branch `author-testing-fixes`): promoted the narrow "code pinned full-width on top,
  prose steps scroll beneath" layout to the DEFAULT at all widths for `.code-walkthrough` (base.css:
  flex-column + `.cw-stage{order:-1;position:sticky;top:0}` full-width, was a 1fr/.85fr 2-col grid that
  made the code column too narrow). Directly delivers the author's ask ("code pinned at top, steps go
  over it, code block wider"). Line-focus (`.qhl-ln-hl`) unchanged; the `max-width:100%` overflow guard
  kept. Browser-verified at 1440px (code stage 100% width, sticky, above the steps; line-focus intact).
  **`.scrolly` left as the classic side-by-side** (sticky viz + scrolling text is the standard
  scrollytelling pattern; the author only flagged the CODE walkthrough — change it too if wanted).

#### Render
- [x] **Math (`$...$`) in `fig-cap:`/`lst-cap:` captions renders as literal text** (P2, confirmed).
  DONE 2026-07-01 (branch `author-testing-fixes`): `numbered_caption` now renders the caption string
  as inline markdown via a new `caption_inline_html` helper (`cell_numbered.rs`), reusing the same
  `parse_options()` (math_dollars on) + `emit_children` path the image-alt caption uses, instead of
  html-escaping. So `$...$`, `*emphasis*`, and `` `code` `` in `fig-cap:`/`lst-cap:` (mermaid /
  code-listing / {js}-figure / code-cell) render like an image-alt caption. TDD unit test
  `math_in_option_string_caption_renders_katex` (render/tests.rs); full core suite green; browser-verified
  (listing caption shows KaTeX, no literal `$`, 0 relevant console errors).
- [ ] **"Code blocks need a refresh to appear": NOT reproducible in code, need a repro** (author to
  confirm): highlighting is server-side and present in the first paint (`highlight.rs:49`→`emit.rs:65,
  82`); the only client code enhancer is the copy button; there is no hydration step to miss (even
  un-run `{python}` cells render as highlighted source on first paint). If it recurs, capture exact
  steps (viewport / kernel state / after a WS reconnect?) and reopen as an open question.

#### Discoverability & distribution
- [ ] **Author publishing/share workflow** (P2, product): the distribution primitives already exist,
  `qmd-fast build <dir> --out <folder>` is a portable static site you can host or zip. The immediate
  answer for sharing the book with supervisors: build `--out` a folder, then push it to a
  `gh-pages`/rendered-HTML branch or any static host (the author's own instinct is right). The gap is
  a *designed, documented* publishing UX: (a) document the recipe in the guide; (b) consider a thin
  `qmd-fast publish` that pushes `_site/` to a rendered-HTML branch. Distinct from the DEFERRED
  marketing-site deploy (that publishes the project's OWN site). Tool stays closed-source (the stance
  under Product / distribution holds; publish is a read-only export, no write-back to source).
- [x] **Google-Scholar `citation_*` (Highwire) meta** (P3, feature).
  DONE 2026-07-01 (branch `author-testing-fixes`): plumbed a per-page `authors: Vec<String>` (front
  matter `author`, scalar or list, via `string_list`) through `FrontInfo` -> `Page` (both the website +
  book constructors) and `meta.rs:social_head` now emits `citation_title` / one `citation_author` per
  author / `citation_publication_date` / `citation_journal_title` (site title) / `citation_public_url`,
  gated to an ARTICLE (has a `date`) that names an author. `citation_pdf_url` intentionally absent
  (no PDF; print track deferred). TDD test `scholarly_citation_meta_for_authored_dated_posts_only`
  (emits per-author for a dated+authored post; absent on a plain page); core+clippy+fmt green. NOTE:
  "farming references" by self-citing is a non-goal/misconception (Scholar wants a real article shape,
  not HTML meta alone). Complements (does not duplicate) `build-seo-completeness`'s JSON-LD.
- Already tracked (no new entry): **generic SEO** (sitemap.xml / robots.txt / JSON-LD) is fully
  specced in `build-seo-completeness` (Wave 5) + the Long-tail SEO line + `BEYOND-QUARTO.md:308-312`.
  Existing SEO today is og/twitter/canonical, gated on a configured `url:`.

#### Tooling / format future
- [ ] **Companion: surface `check`/prose-lint as editor diagnostics** (P3): VS Code squiggles from
  `qmd-fast check --format json` / `crate::prose` located warnings (read-only, no buffer writes). New
  vs the Phase-2 text-transform commands. The onboarding sub-ask ("how do I start using it daily") is
  already answered by `editor/vscode/README.md` + the pending F5-accept, so it needs no new code.
- [ ] **`.qmd` format-on-save** (open question, NOT Phase 2): a source pretty-printer would write the
  editor BUFFER (an allowed surface, that IS the single editing surface) but must preserve
  `data-sourcepos` line stability for click-to-source. Brainstorm whether the reflow is worth the
  click-to-source risk before any work.
- [ ] **Dogfood: migrate the external FL-weather book to qmd-fast** (P3): a real-world Quarto→qmd-fast
  migration and portability stress test (exercises `book.rs`, includes, the freeze cache, the
  `_quarto.yml` breadcrumb, and file-mode portability). If it renders clean, consider pinning a
  reduced version under `corpus/` as a regression doc.
- **Drop `.qmd` / own the syntax highlighting**: this ask IS the already-settled **Taliesin rename**,
  which was spec'd but never tracked. Now logged under its own `### Taliesin rename` section near the
  end of Open/next.

### Reading-first default layout & style re-evaluation (2026-06-30)
Method: a design re-audit of the three formats' DEFAULTS against a reading-first / iA-Writer-Bear-Tufte
brief (author's chosen direction, willing to rethink structural paradigms, not just polish). Inputs: a
3-agent read-only inventory of the design tokens + site/book chrome + deck engine; live headless
screenshots of the built site/book/deck at desktop+mobile, light+dark; and a `deep-research` web pass
(23 sources, 25 claims adversarially verified, 16 confirmed). Verified evidence cited per item.
**Surprise finding: the typographic foundation already matches the brief** (ui-serif 17px/1.7, ~70ch
measure, system-fonts-only, right-rail scrollspy TOC, sidenotes degrade-to-inline, scroll book, deck
scroll mode). The deltas below are targeted, not a rewrite.

**Keep / do NOT "fix" (research-validated, logged so they aren't re-litigated):**
- Serif body is fine for long-form screen reading ("serif hurts legibility" refuted 0-3; Section508 +
  Tufte CSS). Do not switch to sans.
- ~70ch measure (`--qmd-maxw: 46rem`) is correct: 45-75 CPL band, target the upper end for screens
  (Bringhurst/Rutter; Baymard). Do not narrow it.
- Right-rail scrollspy "on this page" TOC is the NN/g-correct placement; sidenotes as a width-gated
  progressive enhancement matches Tufte/Gwern. Keep both.
- Scroll (not pagination) book reading: no comprehension difference (Joshi et al., CHI EA '25). Keep scroll.
- System-font-only (no webfont) is right for offline + no reflow. If a serif webfont is ever bundled,
  ship REAL bold/italic faces, never browser-synthesized (Tufte rule).

**Shipped (branch `reading-first-redesign`, 2026-06-30; all 7 P1+P2 items, corpus-pinned +
browser-verified light/dark, cargo+clippy+fmt green):**
- **Book: one centred ~70ch reading column.** The three-pane rail|content|rail is gone — a slim
  sticky `.qmd-book-topbar` (Chapters launcher · brand · search · theme toggle) over a centred
  column, with the chapter list in a focus-managed off-canvas drawer (`#qmd-book-drawer`,
  `BOOK_DRAWER_SCRIPT`). Build (`page.rs`) + preview (`serve_site/mod.rs`) kept byte-aligned.
- **Theme default -> follow the OS** `prefers-color-scheme`, fall back light (`theme.rs`: `var MODE`
  + `DEFAULT()`; OS-change re-applies unless a reader explicitly toggled). Pinned + browser-verified.
- **Accent WCAG-AA:** foreground `color: var(--qmd-accent)` -> `var(--qmd-link)` across base.css +
  site.css (decorative border/outline/accent-color kept `--qmd-accent`).
- **Body 17 -> 18px** (`--qmd-font-body`).
- **Website TOC auto-gate by heading count:** `Site::page_toc` gates a site-wide `toc:` on
  `render::toc_entry_count(blocks) >= MIN_TOC_HEADINGS` (3); explicit per-page `toc:` still forces.
  Pinned by `site_auto_gates_on_this_page_toc_by_heading_count`.
- **Deck scroll-view default when opened as a link** (standalone, non-embedded); `?qmd=present` /
  `?qmd=slides` opts into the stepped deck; embedded decks unchanged; scroll-mode reading text in serif.
- **Chrome wider than the reading column:** new `--qmd-chrome-maxw` (62rem) on navbar + footer + book topbar.

#### Identity polish (P3, design judgment; the "templated" diagnosis itself is UNVERIFIED, see caveat) — STILL OPEN, deferred
- [ ] **Hero as typeset reading, not a marketing slab.** The eyebrow + big headline + lead + two-button
  hero is the generic SaaS shape (site/mod.rs hero block); for a typography tool the most honest hero is
  beautifully-set prose that shows the real type system.
- [ ] **Drop bordered feature-card grids** for a typeset list with strong hierarchy (site.css `.qmd-card`).
- [ ] **Reconsider the tech-blue accent** for a quieter near-monochrome plus one restrained accent (Bear/iA
  register).
- [ ] **Introduce a spacing scale** (`--space-1..6`). Spacing is ad-hoc rem literals throughout (base.css),
  which makes the calm, consistent rhythm reading-first design needs hard to enforce.

**Research caveats (do not over-claim):** the deep-research pass could NOT verify (a) what
Stripe/Linear/Mintlify/Docusaurus/GitBook/Vercel docs concretely do in 2025-26, (b) the "Bootstrap/Quarto
looks dated/templated" homogenization thesis, or (c) command-palette-as-nav-replacement and Gamma/Pitch
deck specifics. Treat the P3 items and any competitor framing as judgment, not evidence; re-check
competitor layouts live before banking a default on "X does Y." Only the P3 identity-polish items remain
(the P1+P2 set shipped, see above); treat them as judgment, and they overlap the deferred marketing-site work.

### Deep audit findings (2026-06-30; 16-dimension, 33-agent sweep, adversarially verified)
Method: 16 harsh-critic dimension agents (render / diff / deck / exec / dev-server / site / cite-math /
client-js / enhancer-js / css / a11y / security / robustness / testing / perf / dx) in 4 waves -> a fresh
skeptic re-read the code to refute/dedupe each finding. **131 survived (128 confirmed): 11 high, 28 med,
92 low.** Full per-dimension write-ups + the executive verdict are in `notes/AUDITS.md` (2026-06-30 section).
Three recurring themes: (1) **silent failure is still the default** (the dominant cross-cutting weakness);
(2) **a11y is advertised but shallow** (a real focus-trap + ARIA tabs alongside a mouse-only lightbox,
AT-visible off-camera deck slides, and a 3-rule static gate that can't see any of it, so a green `check`
over-vouches); (3) **the JS layer + executed-code stack have no CI** (kernel tests no-op when
`QMD_FAST_PYTHON` is unset; client.js + `assets/js/` untyped + untested). Highest-leverage systemic move:
a discovery/parse warnings channel that reaches `check`/`--strict` + the preview diagnostics overlay, then
route the long tail of silently-dropped cases through it.

**Done this session (2026-06-30):**
- [x] **README/site "double-click" -> "Alt-click (Option-click on Mac)"** (top finding: the marquee
  feature's own instruction was wrong in the most-read files; code binds `e.altKey`, client.js:1044).
  Fixed README.md:8/22/86, web-client/README.md:6, site/demo.qmd:13, site/features.qmd:36.
- [x] **`.code-walkthrough` horizontal overflow on narrow viewports (<~800px)** (found in the parallel
  responsive-viewport sweep, not the agent audit). `.cw-code`'s unwrapped `<pre>` forced `.cw-stage` to
  ~752px, overflowing the page on tablet/phone (showcase + corpus/narrate). Fixed with `max-width:100%`
  on `.cw-stage`/`.cw-steps` (base.css:492); verified at 390/500/1440 (page overflow 0, desktop 2-col grid
  intact), `cargo test -p qmd-fast-core` green.

**Deep-audit P1 SHIPPED (branch `deep-audit-p1`, 2026-06-30; 4 parallel tracks — core / a11y-frontend /
server-robustness + a foreground kernel/client/CI lane — corpus-pinned, browser-verified, audit-qmd reviewed;
cargo+clippy+fmt green incl. a live-kernel CI job):**
- **Correctness / robustness (all 11):** unknown `--flag` -> hard error + did-you-mean (build/check/serve);
  a shared `catch_unwind` guard for build/check/render/blocks (`serve::guarded`); **kernel-died-mid-cell**
  now fast-fails as a distinct `KernelDied` (poll the iopub read + `is_alive()` probe — no full-cap hang or
  "Timeout" mislabel; live-tested); bare `@`-xref word boundary (`bob@host.com` no longer xref'd); SetMeta
  with >1 `data-sourcepos` -> full Update (fixes Alt-click inside fenced divs); explicit `{#id}` dedup
  (+ located warning; `check` coverage preserved); malformed `_site.yml` -> a `--strict` problem + last-good
  kept in the watcher; page-task panic -> `problems`; `--out` no-value -> hard error; `ws.onmessage`
  try/catch -> overlay; theme hot-swap moved out of the `else` (fires on re-mounts); out-of-tree includes
  -> recursive base-dir watch + a warn.
- **Accessibility (all 7):** lightbox keyboard-open (Enter/Space, WCAG 2.1.1); deck `inert` on non-current
  leaf slides (cleared in overview/scroll/print, re-applied on exit); `aria-hidden` on
  `.qmd-lod`/minimap/threads; single-key shortcut opt-out (localStorage, honored by both keyboard.js +
  focus-mode.js) + `SELECT` guard; reader-menu `role=dialog` -> clean disclosure (aria-expanded +
  aria-controls); 24x24 targets (`.qmd-ra-btn`/`.qmd-anchor`); `a11y.rs` gate now matches `[role=button|link|tab]`.
- **Testing / CI:** a CI **kernel job** (ipykernel + a `QMD_FAST_REQUIRE_KERNEL=1` canary so the exec stack
  can't silently re-skip) + a **tsc job** (client.js).

**Residual (deferred):** the *headless `scanA11y` gate* (run the runtime contrast/lang checks in a headless
browser during `check`/`build` — its own browser-in-CI project), and **extending tsc + `@ts-check` to
search.js/toc-spy.js/assets/js/*** (surfaces a large pre-existing error backlog; client.js is gated now).

#### Visual craft / theming (P2)
- [ ] Sepia overrides: `.qhl-*` syntax palette + output/stderr/error boxes + copy button; darken `--qmd-muted` to AA (base.css:33,385,599) — sepia is first-class but only redefines 6 tokens
- [ ] Tokenize copy button + overlay shadows (`var(--qmd-*)` / `--qmd-edge-shadow`) (base.css:255,293,398,696)
- [ ] Add prose rhythm: tokenized p/list margins + a flat `hr` (base.css; currently UA defaults)
- [ ] Dark-mode `--qmd-thm-*` theorem border variants (dark.css)
- [ ] Drop dead `.hero h1` border/padding reset (base.css:321)

#### Deck engine (P2)
- [ ] Debounce/rAF resize; drop `fitSlide` from the resize path (deck.js:1536,242)
- [ ] Speaker view: snapshot clones or embed-mode skips `{js}` execution (currently 2 live iframes) (deck.js:970)
- [ ] Encode + restore fragment index in the URL hash (deck.js:514,1142)
- [ ] Blackout: any nav key resumes; unhide cursor on pointermove (deck.js:1255)
- [ ] fragsOf: skip `PRE` inside `.magic-move` (double-counted as steps) (deck.js:406)
- [ ] Speaker window: `pagehide` clears spClock + nulls speakerWin (deck.js:976)

#### Site / books — surface silent omissions (P2)
- [ ] `contents: .` / own-dir listing: match siblings or reject (currently lists nothing) (mod.rs:627, links.rs:112)
- [ ] `listing:` without `contents:`: warn instead of silently drop (frontmatter.rs:137)
- [ ] Warn when `image:` set but `url:` missing (og/canonical/twitter silently suppressed) (meta.rs:20)
- [ ] Don't drop titleless posts from listings (or warn) (mod.rs:633)
- [ ] Warn on mount/page collision (config/mod.rs:161); warn on missing chapter file (book.rs:98)
- [ ] Per-page `image-alt:` for listing cards (mod.rs:694)

#### Citations / math / bib (P2)
- [ ] Math render failure: harvest the KaTeX error, thread a located Warning (only render path with no diagnostic) (math.rs:31)
- [ ] Quoted single-brace author `"{First Last}"`: strip one brace level like the brace arm + test (cite/parse.rs:165)
- [ ] `strip_tags`: make quote-aware (alt-text truncation + math-heading TOC/slug garble) (mod.rs:1537,1408)
- [ ] `\url`: require `\url{...}`, strip to arg (currently naive global replace) (cite/clean.rs:11)
- [ ] Bibliography path: parse YAML value as string/seq (spaces split it); `.at()` the dup-key warning (mod.rs:754, parse.rs:91)
- [ ] Reconcile cite-key vs bib-key char sets (render.rs:240 / parse.rs:58)

#### Performance (P2-P3)
- [ ] Batch WS ops into one message; run `afterChange()`/scrollspy once per batch, rAF-coalesced — kills the O(ops x doc) cliff on the save hot path (client.js:845, serve/mod.rs:1065, toc-spy.js:86)
- [ ] Merge the two `validate_cross_page_links` renders; make the discover-time search index lazy (mod.rs:382, search.rs:30)
- [ ] math/KaTeX cache: evict a fraction / bounded LRU instead of full clear on overflow (math.rs:40)
- [ ] emit.rs: `write!`/`push_str` instead of `format!`+push_str per tag (emit.rs:16,274,330)

#### Testing / CI (P1-P2)
- [x] **CI kernel job SHIPPED** (`ci.yml`: ipykernel + `QMD_FAST_PYTHON=python` + `QMD_FAST_REQUIRE_KERNEL=1`
  canary that hard-fails the live test if the interpreter goes missing) — the exec stack is now CI-verified.
- [x] **CI tsc job SHIPPED** for `client.js` (`ci.yml` typecheck job). *Still open:* extending tsc + `@ts-check`
  to `search.js`/`toc-spy.js`/`assets/js/*` (surfaces a large pre-existing error backlog — its own pass).
- [ ] insta snapshots on `body_html()` for reactive/explorable/bayesian docs through the exec path (corpus.rs is structural-only) (corpus.rs:99)
- [ ] CI job for editor/vscode tests (gated to editor/vscode/**)
- [ ] deny.toml: `multiple-versions = deny` + skip-tree allowlist (or document allowed dups)
- [ ] `#[serial]` the kernel-load determinism tests; assert a dropped output is a hard named error (the known silent-drop flake)

#### CLI / docs polish (P3)
- [ ] `build --out` with no value: hard error instead of silent default target (build.rs:73)
- [ ] render/blocks: `is_dir()` branch with a clear message (raw OS error today) (query.rs:21,66)
- [ ] usage() build line: add `[--jobs <N>]` + extend the microcopy test (main.rs:104)
- [ ] Reconcile scaffold/usage/README/getting-started repo-URL placeholders (cli.rs:24, main.rs:87, README.md:38)
- [ ] Drop the `{mermaid}` cell from the first getting-started example, or add an offline note (getting-started.qmd:100)
- [ ] README Usage: add `qmd-fast check .`; tie the diagnostics bullet to `check` (README.md:90,136)

#### Security hardening (P3, single-author trust model)
- [ ] `history.replaceState` to scrub `?t=` after mount (security.rs:150, client.js)
- [ ] `qmd_token` cookie: add `; HttpOnly` (security.rs:124)
- [ ] Injected Mermaid `<script>`: `integrity` + `crossorigin`; emit `Referrer-Policy: no-referrer` (mod.rs:858, page.rs:150)
- [ ] `origin_allowed`: only blanket-allow loopback when loopback-bound (security.rs:13)
- [ ] Deck postMessage: gate null/'' origin on `file://` only (deck.js:893)
- [ ] Extension-resource fallback: re-check containment after the symlink walk (serve/mod.rs:387)

### Polish audit findings (2026-06-26; 32-agent UX/polish sweep, adversarially verified)
Method: 20 personas + 12 polish dimensions exercised the real binary + read render/client-JS code;
every bug claim was adversarially refuted against the code; 95 confirmed (1 critical, 10 high, 58 med,
26 low). Full machine-readable list: the workflow result (`scratchpad/.../tasks/wtghv94nr.output`,
digest + 95 bugs). The unifying theme: **silent failure is the default; `check` is the weakest validator
in the toolchain.** Doctrine to adopt: *a green build/check must mean publishable.*

**Wave 1 shipped** (branch `polish-audit-wave1`, 6 parallel lanes + audit-qmd-reviewed; detail in git):
the CRITICAL relative-path include (canonicalize `base_dir` + located warning on any unresolved include);
crashing-cell located warning + `--strict` non-zero exit; `render` warns when given executable cells;
keyboard/SR-operable mobile nav `<button>` + 390px horizontal-overflow fix; `{js}`/Three.js teardown on
edit + `full_render` reset (WebGL/RAF leak); reader touch (selection toolbar + bookmark tap) + lightbox
`#` strip; dark `.qmd-js-error`/callout-border parity + AA-contrast accent tokens; reader Width on TOC
pages; VS Code F5 `launch.json`. Pinned by `corpus/reactive/js-error.qmd`, a nav-a11y corpus assertion,
and `include_relative_base.rs`. Browser-verified across 390/900/1440; **touch gestures + WebGL-cap +
the F5 round-trip still want real-device confirmation.**

**Tier 1 release-hardening shipped (2026-06-27, branch `tier1-hardening`; tagged `v0.2.0`).**
Made "a green `check`/build = publishable" stronger + clippy green: (1) **a11y gate in `check`**
(`diagnostics::validate_a11y`) — heading-level skips (≥2, mid-doc, decks exempt), `<img>` missing
`alt`, and `<a>`/`<button>` with no accessible name; ported from the live `scanA11y`, conservative,
**0 false positives across the corpus**, pinned `corpus/diagnostics/a11y.qmd`. (2) **per-subcommand
`--help`/`-h`** (focused synopsis + flags + example for preview/build/check/render/schema/blocks/init).
(3) **`check --format json` honesty** — an unreadable/missing path now emits `{"error":…}` on stdout
(exit 1) so `| jq` never chokes. (4) **`_quarto.yml` migration breadcrumb** (replaces the confusing
`no _site.yml`; `build` logs it too). (5) **R-aware build kernel hint** (uses `exec::diagnostic()`
instead of hardcoding `QMD_FAST_PYTHON`). (6) **clippy green** (`collapsible_if`→let-chain).

**Still open — high value**
- [ ] **`check` online-link mode (opt-in).** Broken plain/external `http(s)` links are intentionally
  NOT fetched (offline + deterministic by design). If ever wanted, gate a real fetch behind an explicit
  opt-in flag (e.g. `--online`) so the default `check` stays kernel-free and network-free.

**Still open — medium**
- [ ] **CLI microcopy (residual):** raw ANSI leaks into HTML for R stream/stderr (`kernel.rs:672` —
  **DEFERRED, exec/kernel Do-NOT-touch**). The `check --format json`-on-unreadable-path, `_quarto.yml`
  breadcrumb, and the language-aware build kernel hint all shipped in the 2026-06-27 Tier 1 batch.
- [ ] Long tail (perf: shared/minified/compressed assets, O(change) per-edit; SEO: sitemap/robots/JSON-LD;
  doc/code drift) — see the audit digest `polishThemes` + `whatsMissing`.

### Reader experience (the active thrust; idea pool in `FEATURE-IDEAS.md`)
Pattern for any new reader control: `window.qmdReaderMenu.addSection(title, node, onOpen)`; state
in the reader's own `localStorage` keyed by `location.pathname`; deck-skip; pre-paint via
`render/theme.rs` for anything that must not flash. **GOTCHA (line-spacing review):** prose CSS
like `body p, body li { … }` leaks into chrome that wraps prose (TOC, sidebars, navbar — all
`<nav><ul><li>`, search `role=listbox`, margin notes); re-pin with `nav li, [role="listbox"] li,
.sidenote p, .column-margin p, … { line-height: inherit }`. **For letter/word spacing the leak is
worse** (tracking *inherits into inline descendants*), so `5bca91a` also resets monospace + math
directly: `code, pre, kbd, samp, .katex { letter-spacing: normal; word-spacing: normal }`.
- [ ] **Search-hit visual cue (design settled 2026-06-30, spec pending).** On a Cmd-K result click,
  land on the heading as today, then **flash the matched term** via the CSS Custom Highlight API (zero
  DOM mutation — honours read-only-preview; theme-token styled like read-aloud; fades out), and auto-scroll
  to the first occurrence *only if it is off-screen* (option B). Cross-page handoff via **sessionStorage**
  (option A): write the query terms before `location.href`, read + clear on load, then run the same
  locate-and-flash so in-page and cross-page share one code path. Fuzzy/title-only matches just land on the
  heading (no cue). Deck-skip. Next: write the spec under `docs/superpowers/specs/`, then TDD. Reuses the
  `termRanges` logic in search.js + the read-aloud highlight precedent; native `#:~:text=` rejected as
  primary (highlight not theme-able, no fade control, patchy in Firefox).
- Decided/known: the reader menu is intentionally an untrapped popover (not a modal); highlights
  are single-block prose only (margin notes / cross-block / colours were scoped out — see specs).

### Library-outsourcing audit follow-ups (2026-06-25; method: multi-agent sweep of every from-scratch subsystem vs mature OSS, each candidate adversarially verified against the invariants)
- [ ] **Correct the `serde_yaml` fallback target (watch-item).** The `Cargo.toml` workspace
  comment names `serde_yml` as the fallback, but it carries **RUSTSEC-2025-0068 (unsound +
  unmaintained)**; `serde_norway` is 1+ yr stale. The maintained continuation is
  **`serde_yaml_ng`** (v0.10). No urgency (input is trusted local config; 0.9 still builds). If
  0.9 ever breaks against a future serde/edition, swap to `serde_yaml_ng`, gated on a test that
  `Error::location().line()` still works (the only click-to-source-relevant API). Fix the stale
  comment when touched.
- Decided against (so they aren't re-litigated): **hayagriva**/**biblatex** (citations — mature
  but large integration, heavy deps incl. the very serde_yaml 0.9 we're leaving, and zero corpus
  demand: only IEEE is used, which the hand-roll already produces; revisit only for live
  multi-CSL switching); **schemars** (reopens the schema↔validator drift already closed);
  **jsonschema** (loses source-line diagnostics); **morphdom**/**idiomorph** (reverse the 83x
  live-edit payload win + risk live-state loss; the diff is server-authoritative); **similar**/
  **dissimilar** (give up the unique-block-id→LIS reduction); **clap**; **owo-colors**; **slug**
  (transliterates non-ASCII → breaks anchors/`@sec-`); **html-escape** (breaks the
  anti-double-escape contract); **lightningcss**/**palette** (no Rust color math — CSS uses native
  `color-mix`); IntersectionObserver/scrollspy libs (can't do the dynamic activation line +
  bottom-pinning); deck micro-helpers d3-interpolate/screenfull/hotkeys/hammer (each force an
  offline bundle onto every deck for a few lines).

### Extension ecosystem audit (deferred — its own separate pass)
*Author decision (2026-06-27): the `_extensions/` story (themes + functionality extensions) gets
its own dedicated audit pass, not piecemeal fixes. Goal: survey the whole extension ecosystem —
what an extension can hook (theme `--qmd-*` tokens, bundled CSS/JS, shortcodes, `{{< embed >}}`),
where the seams are sharp vs. sharp-edged, and which capabilities are missing or under-documented —
and produce a prioritized list of improvements. Treat the native deck contract (`window.QmdDeck`,
`.qmd-deck`/`.qmd-slide`) as the stable target any extension must build against.*
- [ ] **Run the extension-ecosystem audit** (themes + functionality): inventory the worked
  examples under `_extensions/`, exercise each against the real binary, and find rough edges +
  gaps + missing docs. Output a prioritized improvement list.
- [ ] **Known finding to fold in — liquid-glass corpus extension is dead.** Live `Uncaught
  ReferenceError: Reveal is not defined`; its CSS targets `.reveal` DOM the native engine never
  emits, so the headline glass effect of THE worked extension example is non-functional. Fix:
  port to `window.QmdDeck` + `.qmd-slide`/`.qmd-deck`; add a corpus test asserting the theme
  actually applies. (Deferred out of the polish-audit high-value list into this pass.)

### Deck
- [ ] **Mobile / touch (deeper).** Pinch/pan + touch gestures on the deck, and `{js}` widgets
  tuned for touch. (Hard to verify without a real device.)
- [ ] **Footer / logo (deferred).** No corpus deck needs one yet; thread `footer:`/`logo:`
  through both deck-page builders + the asset-copy set when one does.
- Decided against: inline `{.r-stretch}` image (use the `:::{.r-stretch}` div), `#`-section
  quick-jump anchors (redundant with the minimap + `/` filter).

### Release regression-hunt deferrals (2026-06-30; adversarially-verified, LOW / pre-existing / by-design)
From the pre-release 8-dimension regression hunt over the `stable-2026-06-25..HEAD` delta. Its 7
in-window regressions were FIXED (brace_id quote/multi-block; `check` mount-link false positives +
mounted-site test; theorem `number-within`/`numbered` value validation + schema enums; site
referenced-asset deploy; client.js tsc-clean; TOC chip `#` strip). These remaining findings are real
but LOW, pre-existing, or by-design:
- [ ] **Boot-failure diagnostic overwrites a cache-hit cell's output** (`exec.rs:491-505`). Only fires
  when a cached cell sits upstream of an uncached one AND the kernel fails to boot through all retries —
  the build is already flagged `error` (never green) and the cell still shows its source, so it is
  strictly more honest than the old silent empty. Optional: restore from freeze for `known(i)` before
  the diagnostic. (exec/kernel Do-NOT-touch.)
- [ ] **Warm-pool `in_flight` counter can leak (inert the pool) if a refill task panics**
  (`warm_pool.rs:456-490`). No reachable panic site today (the `warm_one` chain is all `Result`/`?`);
  worst case is graceful cold-start fallback. An RAII drop-guard on `in_flight` would harden it — fold
  into the forkserver Do-NOT-touch pass below.
- [ ] **Cross-page theorem refs drop the number** ("Theorem 2.1" renders bare "Theorem" across pages;
  `site/xref.rs` pushes an empty number for non-heading anchors). Pre-existing (`@fig-`/`@eq-` always
  had it); HEAD is a net gain (the link resolves at all). Harvest numbers from the per-page rendered
  registry if parity is wanted. Ties into the theorem ref-name polish in `BEYOND-QUARTO.md`.
- [ ] **A theorem nested inside another fenced div** (`.column-margin`/`.callout`) loses its number +
  xref registration (`number_theorems` walks only top-level blocks). The xref half IS surfaced by
  `check`/`build` (not silent); residual: an *unreferenced* nested theorem renders unnumbered on a green
  check. Optional: warn when a `data-qmd-theorem-kind` div is found nested.
- [ ] **Backslash-escaped quotes in a `title=`/`fig-cap=`/`lst-cap=` value truncate it + leak `\`**
  (`render/divs.rs` `tokenize_attrs`). Pre-existing, narrow (escaped-quote-in-quoted-title; the
  single/double swap is not a reliable workaround). Teach `tokenize_attrs` to honor `\` escapes, or lint
  a backslash-before-quote.
- [ ] **Doc drift: the no-kernel build now embeds a per-cell "kernel unavailable" diagnostic** (intended;
  fixes the silent-output-drop bug, test-covered), not just the old preview-only banner. Reconcile the
  wording in `CLAUDE.md:122-123` + `docs/guide/using/getting-started.qmd:44`, and the misleading
  `build.rs:232` stderr "uncaught exception" for the kernel-never-launched case.

### Execution cache
- [ ] **Kernel/forkserver resource leaks on build exit (observed during the 2026-06-29 port-race
  fix).** Two cleanups, both in the exec/kernel Do-NOT-touch zone (careful): (a) warm-pool
  **forkserver daemons survive a completed `build`** — ~30 orphaned `multiprocessing.forkserver`
  procs (each ~100 MB preloaded numpy/matplotlib) were left after a batch of normal-exit builds;
  likely the daemon child isn't reaped on CLI exit (check whether `build` exits via `process::exit`,
  which skips `Drop`, or whether the daemon `Arc` outlives the runtime). (b) a **failed
  `Kernel::start` leaks its `/tmp/qmd-kernel-<uuid>` connection dir** — only a *successful* `Kernel`
  owns the dir for Drop-cleanup; the early-return error paths drop the `PathBuf` without removing the
  dir. The 2026-06-29 kernel-start retry slightly amplifies (b) (each failed attempt leaks one). Fix:
  kill the forkserver daemon on build teardown; remove the conn dir on a failed start (or have the
  retry loop clean up its abandoned attempts). Low-priority (temp dirs + procs, reclaimed on reboot)
  but unbounded under repeated failures.
- [ ] **Cold-start kernel warming (follow-up, deferred).** After a cold full-replay, the
  first edit re-runs the whole doc to rebuild kernel state. Could speculatively warm the
  kernel in the background. Inherent to a plain Jupyter kernel; not worth it until it bites.

### Deferred / demand-driven
- [ ] **Image optimization (large).** WebP/AVIF transcode + responsive `srcset` +
  lazy-load, behind a content-hashed asset cache. Deferred until posts get image-heavy.
- [ ] **Wave 5 / later** (`BEYOND-QUARTO.md`): `print-pdf-track` (paged render *of* the
  built HTML), `docs-as-spec` (RFC-2119 dialect + protocol reference), `{glsl}` cell-language
  registry, `build-seo-completeness` (sitemap/robots/JSON-LD at publish with `url:`).
- [ ] **VS Code companion Phase 2 (deferred, capped).** Editor commands (insert block /
  reorder slide) — strictly `.qmd`-buffer text transforms in the editor, never preview
  gestures.

### Marketing site (DEFERRED — feature-first; rolls into a demo-machine rebuild)
- [ ] `live-edit-hero-demo`: the recorded split-screen-vs-Quarto clip (the bench numbers +
  `tools/record-demo` recorder already exist).
- [ ] Swap placeholders in `site/_site.yml` (`url:` + GitHub links); rebuild the hero pages
  demo-led (motion, one value line, the vs-Quarto table, install on-ramp). Folds in the open
  visual bugs: 390px prose overflow (`page-layout: full` + `hero:`), theme/video desync (drive
  the `{{< video >}}` variant off the site toggle), leftover em dashes in copy.
- [ ] Refine the mobile embed (narrow iframe → reader). Deploy (Cloudflare / GitHub-Pages).

### Audit residuals (deferred, low-risk; detail in `AUDITS.md`)
- [ ] **Robustness.** Combined content+theme edit drops the hot-swap until reload
  (`serve.rs`); initial synchronous render isn't panic-guarded; `front_matter_block`
  terminates early on `---`/`...` inside a block scalar; mounted sub-sites don't route
  embedded decks (a mount miss serves a bare 404).
- [ ] **Perf.** `updateWordCount` deep-clones all of `#qmd-root` per op (`client.js`);
  visited pages are never evicted from `app.pages` (`serve_site.rs`, unbounded growth); a
  tens-of-MB cell output blocks the ZMQ receive before the cap fires (`kernel.rs`).
- [ ] **Bib / build edge cases.** `@inbook`/`@incollection` drop `booktitle`/pages;
  query-string asset refs aren't bundled (`main.rs`). The remaining LOW findings live in
  `AUDITS.md`; pull up only when relevant.

### Taliesin rename (settled design, UNSHIPPED, was untracked until 2026-07-01)
Spec: `docs/superpowers/specs/2026-06-27-taliesin-rename-design.md` (decisions settled). Surfaced by
the 2026-07-01 book-testing triage as a large, settled-but-untracked task (the author's "drop `.qmd`
and own the highlighting" ask). Owner-gated identity call; ties into public-OSS-release timing.
- [ ] **Execute the Taliesin rename**: `.qmd`→`.tmd` routed through a central `crates/core/src/ext.rs`
  constant module (`.qmd` kept as deprecated-accepted input with a warn-nudge, i.e. a clean break WITH
  a migration path, not a hard drop, per the spec's Markdown-familiarity north star); package names
  →`taliesin-*`; binary `taliesin` + `tali`; the `qmd-*` contract prefix →`tali-*` with back-compat
  aliases. Large multi-surface change (corpus + docs churn).
- [ ] **Own the syntax-highlighting grammar** via the `.tmd` language association (the VS Code
  companion already sets its own regardless; spec §3). This is the concrete answer to "rename so I
  fully control the highlighting."

### Interactive/explorable numerics for scientific docs (2026-07-01; idea pool in `FEATURE-IDEAS.md` #62–66)
Surfaced by dogfooding: building an interactive PML/Bayesian-ML study site on the shipped `{input}`
(#47) + `{js}` reactive graph. The substrate is there; what math/ML explorables lack is a numerics
story and two controls. All stay HTML-only/offline and must **not** reintroduce a reactive VM (the
stated top design risk). None spec'd or corpus-pinned yet, so the detail lives upstream in
`FEATURE-IDEAS.md`; promote an item here with a corpus pin when it graduates. Highest-leverage first:
**#62 + #63**.
- [ ] **#62 Bundled numerics/stats global for `{js}`** (P2): a small curated global beside `Plot`/`d3`
  — distribution pdf/cdf (gaussian/gamma/beta/poisson/exp), mean/var, a **seeded** PRNG, small dense
  linalg (matmul, Cholesky, 2×2 eig/inv). Kills the #1 friction (hand-rolling pdfs); a bundled global,
  not new machinery. Pin `corpus/reactive/numerics.qmd`.
- [ ] **#63 Two ML `{{< input >}}` types — `animate`/play tick + draggable `point`** (P2–P3): a
  play/step/reset tick for iterative demos (EM/CAVI/gradient descent) and a drag/click 2-D point for
  "place a data point" (mixtures/FA). Reuse `registerInput`/`scheduleFrom` (scrolly #46 already proves
  non-slider inputs); the tick schedules **one** downstream pass per frame via the scheduler +
  `invalidation` — not a dataflow loop.
- [ ] **#64 `qmd.state` cross-re-run store** (P3, needs-care): keyed state that survives scheduled
  re-runs so iterative demos accumulate (EM params across ticks); cleared on cell edit; deck-skip; no
  write-back. Pairs with #63; scope tightly (not general mutable dataflow).
- [ ] **#65 Richer `{js}` output helpers** (P3): KaTeX-typeset a returned number/array/matrix + a minimal
  table renderer, over the existing DOM-return contract. Closes the rich-display gap vs Jupyter.
- [ ] **#66 Opt-in Pyodide `{python}` cell** (L, needs-care): client-side numpy/scipy/sklearn, no kernel
  (this is JupyterLite) — the general "match Jupyter's Python" answer. **Bundle guard**: ~10 MB+, opt-in
  per page, vendored offline; sibling to DuckDB-WASM `{sql}` (#50). Caveat: **no torch in Pyodide**
  (Bayes-by-Backprop won't run). Cell-language-registry graduate; cut until a corpus doc needs it.

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now
(***REMOVED***; see `STARTUP-PLAN.md`). Open-source
the repo + publish the site when ready; the GitHub/install CTAs become real then. The
security #1d token is the gate.
