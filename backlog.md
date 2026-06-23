# qmd-fast backlog

**Scope rule: the corpus is the spec.** "Done" means the docs under `corpus/`
render correctly, not that a Quarto feature checklist is complete.

> Kept deliberately small (read often). Only open tasks live here; the history of
> completed work is in git. Detailed audit findings live in `AUDIT-BACKLOG.md`
> (round 1, P0–P3) and `AUDIT-2.md` (round 2, empirical).

## State (2026-06-21, on `main`)

All four formats render and deploy. The dev loop is strong: block-level incremental
updates (DOM state preserved), warm server + Jupyter kernel, a `_freeze` execution
cache, Alt-click click-to-source (+ reverse cursor sync), located code-framed
front-matter errors, CSS/theme hot-swap (no reload), broken-citation /
cross-reference warnings, an error overlay, LAN + QR (`--host`), `--open`, port
auto-fallback, and Cmd-K search. The native slide engine has the overview map +
minimap, speaker view, fragments, `. . .` pause, black-screen (B/`.`), magic-move,
auto-animate, `:::{.r-stretch}`, per-slide backgrounds, PDF export, and drawing.

**Two adversarial audit passes are done (2026-06-21).** Round 1 (every source file
deep-read + verified, every feature exercised live) closed all P0/P1 and most P2/P3:
security containment (path traversal, kernel-file perms, attribute/postMessage
escaping), the `:::`-in-code-fence + cell-`tbl-` corpus correctness bugs, site
preview/build parity, and theme-matched matplotlib. Round 2 (a battery of ~80 hostile
docs actually run) closed the real failure modes a power user would hit: per-cell
output caps (a 5 MB print went 70 s → 1.7 s), an in-place-build data-loss guard,
lenient Quarto `website:` parsing, mount search/feed routing, a 256 MB render stack
for deep nesting, and duplicate-label warnings.

**Session 2026-06-21 (HEAD `84fee87`):** code-simplification pass (net −50 lines);
priorities #1 (security, minus the LAN token), #2 (editing-transience), and the
headline of #3 (error DX) landed. 211 workspace tests pass, clippy + fmt clean.

**Session 2026-06-23 (branch `refactor/depecialize-posts-dir`):** de-specialized the
`posts/` directory: removed the RSS feed, category/tag archive pages, post prev/next,
and per-post decoration, so every page now renders uniformly (`og:type` keys on
`date:`). Added two silent-failure warnings (a loose `format: revealjs` deck in a site;
a `{{< embed >}}` in a single-doc build). Synced the docs. Ran a third audit
(format/structure + shareability); the open items live under "Open / next". The
Alt-click click-to-source visual affordance shipped to `main` (commit `d8ac06f`).

## Priorities (next, in order)

New viewpoints the prior audits never took (they judged the tool against itself:
correct/fast/clean/robust/pretty). These look at it from outside: the trust lens, the
tool *in motion* while editing, the newcomer's first five minutes, head-to-head vs
Quarto, and the pre-open-source gate. Curated to what's actually worth the time.

1. **[~] Security: untrusted-document trust boundary.** Shipped (2026-06-21): (a)
   cross-origin websocket upgrades are rejected (blocks a malicious page in your
   browser driving your dev server; tested + live-verified 403/101), (b) `--no-exec`
   / `QMD_FAST_NO_EXEC` previews untrusted docs as source without starting a kernel,
   (c) a LAN-exec warning on `--host`. **Remaining: (d)** a per-session token in the
   `--host` URL/QR (the LAN-snooping defense; touches `client.js` + both servers'
   routing). Do (d) before any open-source release.
2. **[x] Editing-transience resilience pass.** DONE (2026-06-21). (i) **DOM state
   now survives structural edits** via the `BlockOp::SetMeta` op (live-verified: an
   open `<details>` survives an edit above it, same element, sourcepos patched). (ii)
   **Transient render/diff robustness audited + regression-tested** (`tests/
   transient.rs`): the renderer never panics on any line-prefix of the 59 corpus +
   docs files (sampled) nor on 23 half-typed constructs (unclosed `:::`/fence/`$$`/
   YAML frame/shortcode/link/...), and the full render→diff seam never panics across
   a keystroke sequence. No bugs found: the pipeline handles mid-typing gracefully.
   (iii) Kernel-wedge-on-transient-bad-cell is covered by round-2's verified kernel
   resilience (exception/segfault/timeout recovery) + the save debounce. The only
   "blank" transient is an unterminated YAML frame (semantically correct: no body
   yet), left as-is.
3. **[~] Error-message & first-run DX pass.** Audited every failure mode and graded
   each. Already strong (A): no-kernel (names the bad path + the fix env var), broken
   extension (names it + where it looked), port-in-use (auto-fallback), front-matter
   (located + framed in preview). **Fixed the two silent-failure traps (2026-06-21):**
   an unknown shortcode (typo'd `{{< name >}}`) and a missing include in `build` both
   now warn, located + named, in the build log AND the preview panel. Remaining
   (lower value): a cell that raises during `build` bakes the traceback into the page
   with no stderr signal; `[@key]` with no bibliography is silent; "dir passed as a
   file" is a raw OS error (`Is a directory`); `render` (one-shot) prints no warnings
   (only `build` does).
4. **[ ] Output fidelity vs Quarto, systematized.** Turn the testbed sibling repo
   into a corpus-wide sweep: render each doc in both, structural-diff, catalog every
   divergence as bug-or-deliberate. The only thing that de-risks "drop Quarto
   completely" past self-judgment ("the corpus is the spec" is necessary, not
   sufficient, for a *replacement* claim).
5. **[ ] Supply-chain / licensing audit (pre-OSS gate).** Verify every vendored asset
   (KaTeX, syntect grammars, the reveal *theme*, fonts, OJS runtime) permits
   redistribution and is attributed in `THIRD_PARTY.md`. Cheap now, expensive to
   discover after the repo is public.
6. **[ ] Typography pass on the themes.** Measure (line length), vertical rhythm,
   optical heading sizes, code-block font pairing. The single biggest "feels premium
   vs feels templated" lever and pure craft (use the `frontend-design` skill).
7. **[ ] Quantify + demo the differentiators.** A tiny benchmark page (Quarto cold
   render vs qmd-fast warm edit) and make state-preserving live-edit the hero demo.
   You bet the architecture on sub-second warm edits and DOM-state preservation but
   never measured or marketed either; both are things Quarto structurally can't match.

Lower, fold-in: a11y of the *output* (semantic landmarks, deck focus/keyboard,
theme contrast, reduced-motion) folds into the existing a11y item below; built-site
production quality (Lighthouse/CWV, no-JS fallback, hashed assets, robots/sitemap);
cross-browser check of the deck (Safari/Firefox/iOS, since the visual audit was
Chromium-only). Not worth now: deeper i18n/RTL (English solo author).

## Open / next

### Format & structure audit (round 3, 2026-06-23): open items

A multi-agent audit of the single→site/book/deck journey plus project
layout/shareability (49 confirmed findings). The shipped fixes (posts/
de-specialization, the loose-deck and dead-embed warnings, the docs sync) are in git;
the open items follow.

- [ ] **P2: the Quarto-compat config path has no validation (high).** The native
  config path warns on unknown keys (`config/mod.rs:204`), but the Quarto-shaped path
  (`config/quarto.rs::from_value`) gives zero feedback, so typos vanish and load-bearing
  keys drop silently. Quarto's keyspace is *open*, so do NOT warn on every unmodeled key.
  (a) Typo-detect top-level keys (near-miss of `{project, website, book, format, execute,
  jupyter}` via `frontmatter::closest`). (b) Add one targeted "recognized but ignored"
  warning for `format: html: theme:` (the whole visual theme drops silently; qmd-fast
  ships its own, and custom styling goes through `css:` or a theme extension). Skip a
  `resources:` warning, since those files *are* copied by `mirror_assets`.
- [ ] **P3a: `build` leaks residue into the deployed output (medium).** `mirror_assets`
  (`main.rs:500`) copies every non-`_`/`.` file ignoring `.gitignore`, dragging R/Quarto
  caches (`*_cache/`, `index_cache/`, `.RData`), private notes, and `.bib`/`.Rproj` into
  `_site/`. Min fix: also skip `*_cache/`/`*_files/` dirs at `main.rs:521`. Cleaner: honor
  the project `.gitignore` (the `ignore` crate).
- [ ] **P3b: `mounts:` works in `preview` but `build <site>` ignores it (medium).**
  `build_site` (`main.rs`) never reads `site.config.mounts`, so a previewed site with
  working `/docs/*` nav links deploys with 404'ing links. Warn first (name each unwired
  mount, print the `build <path> --out <out>/<at>` command); auto-build into `<out>/<at>/`
  later. Also fix `docs/internals/sites.qmd` "## Mounts", which still claims the build
  mirrors mounts.
- [ ] **P3c: single-doc `build --out` drops OJS local imports (medium).**
  `copy_local_assets` (`main.rs:264`) only scans `src=`/`href=` HTML attributes, so an OJS
  `import {...} from "./helper.js"` is invisible and the standalone interactive post 404s.
  Warn, then scan OJS cell source for relative imports (recursively).
- [ ] **Docs: no "Project structure & reserved names" reference (medium).** Add an
  annotated-tree section (`configuration.qmd`) covering the `_`/`.`-skip rule, `_freeze/`,
  and `_includes/`, plus a "how a deck gets built" note (chaptered vs embedded vs
  standalone, the omission that orphaned `docs/guide/tour.qmd`).
- [ ] **Wire up `draft:` (low).** Already in the front-matter lint allowlist
  (`frontmatter.rs:85`) but connected to nothing, so `draft: true` is a silent no-op. Add
  a `draft: bool` to `FrontInfo` and filter it out of `website_pages` (which also drops it
  from nav + listings).
- [ ] **Corpus hygiene (low).** `corpus/bayesian-book` is a single-page *website*, not a
  book; rename it (dir + the `book_*` test fn) to stop blurring the website/book line.
  Delete `tech-blog/**/_metadata.yml` (ignored; teaches a cascade that does not exist).
  Fix the `corpus/README.md` demo-book row (it says `book: chapters:`; the file uses native
  flat `chapters:`). The liquid-glass deck depends on remote Unsplash + Google Fonts and a
  `quarto add` line; vendor those instead.
- [ ] **`docs/guide/tour.qmd` is orphaned (low).** A deck in the book dir, neither
  chaptered nor embedded, so the build never produces it. Embed it, chapter it, or move it
  out.

(The full 49-finding report came from a one-shot workflow run and was not persisted; the
items above are the confirmed, actionable ones.)

### Slide deck
- [ ] **Mobile / touch (deeper).** Reader flatten + light-deck dark-bg done; still:
  pinch/pan + touch gestures on the deck itself, and interactive widgets (OJS slider)
  tuned for touch. (Hard to verify without a real device.)
- [ ] **Footer / logo (deferred).** No corpus deck needs one yet. When one does:
  thread `footer:`/`logo:` front-matter through `RenderedDoc` → both deck-page
  builders (`reveal_page_from_doc` + `serve.rs` live `PageCtx`), render fixed chrome
  inside `.reveal` (hidden in overview/print), and add the logo to the build's
  asset-copy set.
- Decided against: inline-image r-stretch (`![](x){.r-stretch}`), images become
  numbered figures, so use the `:::{.r-stretch}` div form. `#`-section quick-jump
  anchors, redundant with labeled topic rows + the minimap + `/` filter.

### Execution cache
- [ ] **Cold-start kernel warming (follow-up).** After a cold full-replay (a preview
  restart on an unchanged doc), the kernel isn't booted, so the *first* edit re-runs
  the whole document to rebuild kernel state. Could speculatively warm the kernel in
  the background (run the cached prefix) so that first edit stays incremental.
  Inherent to a plain Jupyter kernel; not worth it until it bites.

### Book / site
- [ ] **Book chapter label ignores front-matter `title:` for dual-use docs.** A book
  chapter's sidebar label comes from its first `# H1` (`site/book.rs` `chapter_heading`
  → `push_chapter`), falling back to the front-matter `title:` only when there is *no*
  H1. So a doc written to also stand alone — a front-matter `title:` plus flat
  `#`-level sections — gets labelled by its first *section* name, not its title.
  *Repro:* a chapter with `title: "The Lossless Engine"` whose first heading is
  `# Why this exists` shows "Why this exists" in the sidebar; an `index.qmd` preface
  (no H1) correctly uses its front-matter title. *Candidate fix:* prefer front-matter
  `title:` over the first H1 in `push_chapter` (`book.rs:70-73`), or allow a per-chapter
  label override in `_quarto.yml` (`- file: x.qmd, text: "..."`). **May be deliberate
  Quarto parity** (Quarto treats the first H1 as the chapter title), so decide whether
  this is a bug or intended before changing it. Low value, trivial fix; affects any book
  built from standalone docs. (Surfaced 2026-06-23 building a 2-chapter book in the
  invertible-speech-disentanglement repo.)

### Dev-loop DX (researched 2026-06-20 vs Vite / Astro / Hugo / VitePress)
Three Tier-1 items shipped (located errors, CSS hot-swap, broken-ref warnings).
Remaining candidates, with an honest value read for a solo-author personal tool
(most are low-value here, kept so they aren't re-derived):
- [ ] **a11y audit, the standout if quality matters.** Client-side DOM checks
  (missing alt text, heading-level skips, low-contrast `--qmd-*`, missing `lang`)
  surfaced as click-to-source diagnostics in the existing panel. ~4-5 high-confidence
  rules, no server work. Worth it because the issues are recurring + invisible.
- [ ] **Image optimization, broad value if posts get image-heavy; large effort.**
  Transcode local images to WebP/AVIF + responsive `srcset` + lazy-loading, behind a
  content-hashed asset cache. The one expensive-but-genuinely-useful item; deferred
  until images actually bite.
- Evaluated and skipped (revisit only on a concrete need): `--share` tunnel (just run
  `cloudflared` by hand), deck follower/multiplex (projector / screen-share already
  covers it), front-matter schema validation (existing key-lint + the YAML frame
  cover most), console→terminal forwarding (mainly an AI-agent debugging aid; cheap
  if wanted), copy-permalink (headings already have anchors), edit-this-page /
  last-updated (solo author, no external contributors).

### Marketing site (visual-ux-audit 2026-06-19: the hero pages roll into a demo-machine rebuild)
- [ ] Swap placeholders in `site/_quarto.yml`: `url:` + the GitHub links.
- [ ] Rebuild the hero pages demo-led (lead with motion, one value line, the
  vs-Quarto table, an install/quickstart on-ramp). Folds in the open visual bugs:
  mobile prose overflow at 390px (`page-layout: full` + `hero:`), theme/video desync
  (drive the `{{< video >}}` variant off the site toggle, not the OS media query),
  and the leftover em dashes in the copy.
- [ ] Refine the mobile embed (narrow iframe → reader / nested-scroll).
- [ ] Deploy: Cloudflare or a GitHub-Pages single-tree pipeline (when publishing).

### Audit residuals (deferred, low-risk for this tool's scope; detail in the two audit files)
- [ ] **Robustness / correctness.** Combined content+theme edit drops the hot-swap
  until reload (`serve.rs`); initial synchronous render isn't panic-guarded
  (`serve.rs`); `front_matter_block` terminates early on a `---`/`...` inside a block
  scalar (`frontmatter.rs`); duplicate cross-ref labels still emit two identical
  `id=` attributes (number/anchor now agree + warn); mounted sub-sites don't route
  embedded decks, and a mount miss serves a bare asset 404.
- [ ] **Perf.** `updateWordCount` deep-clones all of `#qmd-root` on every op
  (`client.js`); visited pages are never evicted from `app.pages` (unbounded
  block-state growth, `serve_site.rs`); a single tens-of-MB cell output still blocks
  the ZMQ receive before the cap can fire (`kernel.rs`).
- [ ] **Bib / build edge cases (no corpus entry yet).** `@inbook`/`@incollection`
  drop `booktitle`/pages; query-string asset refs aren't bundled (`main.rs`).
- The remaining LOW findings, nits, and the P3 test-coverage gaps live in
  `AUDIT-BACKLOG.md` + `AUDIT-2.md`; pull them up here only when one becomes relevant.

## Product / distribution
Direction resolved (2026-06-20): ship as **open source + personal tool**, no company
for now (***REMOVED***; see
`STARTUP-PLAN.md`). Open-source the repo + publish the site when ready; the
GitHub/install CTAs become real then.
