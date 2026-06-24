# qmd-fast backlog

**Scope: corpus-plus-roadmap.** "Done" still means the docs under `corpus/` render
correctly (the corpus is the regression net), but each new capability now ships pinned
by a target corpus doc. Output stays **HTML-only**. The active roadmap is
`BEYOND-QUARTO.md`.

> Kept deliberately small (read often). **Only open tasks live here.** Completed work is
> in git + the history docs: `BEYOND-QUARTO.md` (Beyond-Quarto waves), `DROP-QUARTO.md`
> (the native-rewrite), `AUDITS.md` (the three audit passes). Don't re-add `[x]` items.

## State (2026-06-24, `main` @ `1c9905a`, version 0.1.0)

All four formats render + deploy; the dev loop is strong (block-level incremental updates
with DOM-state preservation, warm server + Jupyter kernel, `_freeze` cache, Alt-click
click-to-source + reverse cursor sync, located/framed diagnostics, CSS hot-swap, Cmd-K
search). Nothing is pushed to any remote.

**Shipped initiatives** (history in the docs above): DROP-QUARTO (fully native, no shims/
reveal.js/OJS). Beyond-Quarto **Waves 0-3 complete** + **Wave 4 built**: the schema
validator + JSON schemas, the live-edit benchmark, all six Wave-3 craft/breadth features
(walkthrough, tabset+margin, callout contract, typography, lightbox gallery, js-reactive
graph), the reverse-sync audit, and the VS Code editor companion (Phase 1, headlessly
verified). Recent backlog fixes: build-residue skip, `{js}`-import bundling, `mounts:`
build warning, `draft:` wire-up.

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

### Highest-value
- [ ] **Security #1d: per-session token in the `--host` URL/QR (pre-OSS gate).** The last
  piece of the security pass: a token gating LAN access (LAN-snooping defense). Touches
  `client.js` + both servers' routing; the VS Code companion is localhost-only so it's
  unaffected, but the token must thread into the `--host` URL/QR. Do before any OSS release.
- [ ] **Book chapter label ignores front-matter `title:` for dual-use docs.** A chapter's
  sidebar label comes from its first `# H1` (`site/book.rs` `push_chapter`, ~`book.rs:70-73`),
  falling back to `title:` only when there's no H1 — so a stand-alone-capable doc (`title:` +
  flat `#` sections) is labelled by its first *section*, not its title. Fix: prefer
  front-matter `title:`, or allow a `_site.yml` per-chapter label override (`- file:, text:`).
  **Decide bug-vs-Quarto-parity first** (Quarto treats the first H1 as the chapter title).
  Trivial fix; affects any book built from standalone docs.
- [ ] **Output fidelity vs Quarto, systematized (#4).** Turn the `qmd-fast-testbed` sibling
  repo into a corpus-wide sweep: render each doc in both, structural-diff, catalog each
  divergence as bug-or-deliberate. The only thing that de-risks "replaces Quarto" past
  self-judgment.
- [ ] **a11y audit of the *output*.** ~4-5 high-confidence client-side DOM checks (missing
  alt text, heading-level skips, low-contrast `--qmd-*`, missing `lang`) surfaced as
  click-to-source diagnostics in the existing panel. No server work; recurring + invisible
  issues, so worth it.

### Polish / docs
- [ ] **Docs: "Project structure & reserved names" reference (medium).** Annotated-tree
  section in `configuration.qmd`: the `_`/`.`-skip rule, `_freeze/`, `_includes/`, and a
  "how a deck gets built" note (chaptered vs embedded vs standalone — the omission that
  orphaned `docs/guide/tour.qmd`).
- [ ] **Corpus hygiene (low).** Rename `corpus/bayesian-book` (a single-page *website*, not
  a book; dir + the `book_*` test fn). Delete `tech-blog/**/_metadata.yml` (ignored; teaches
  a non-existent cascade). Fix the `corpus/README.md` demo-book row (`book: chapters:` →
  native flat `chapters:`). Vendor the liquid-glass deck's remote Unsplash + Google Fonts.
- [ ] **`docs/guide/tour.qmd` is orphaned (low).** A deck in the book dir, neither chaptered
  nor embedded, so the build never produces it. Embed / chapter / or move it out.
- [ ] **CI: wire `cargo-deny`.** `deny.toml` exists (Wave 0); the CI step was deferred
  (cargo-deny not installable/verifiable locally). Add it when CI is set up.

### Deck
- [ ] **Mobile / touch (deeper).** Pinch/pan + touch gestures on the deck, and `{js}` widgets
  tuned for touch. (Hard to verify without a real device.)
- [ ] **Footer / logo (deferred).** No corpus deck needs one yet; thread `footer:`/`logo:`
  through both deck-page builders + the asset-copy set when one does.
- Decided against: inline `{.r-stretch}` image (use the `:::{.r-stretch}` div), `#`-section
  quick-jump anchors (redundant with the minimap + `/` filter).

### Execution cache
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

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now
(***REMOVED***; see `STARTUP-PLAN.md`). Open-source
the repo + publish the site when ready; the GitHub/install CTAs become real then. The
security #1d token is the gate.
