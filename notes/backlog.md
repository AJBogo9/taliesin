# qmd-fast backlog

**Scope: corpus-plus-roadmap.** "Done" still means the docs under `corpus/` render
correctly (the corpus is the regression net), but each new capability now ships pinned
by a target corpus doc. Output stays **HTML-only**. The active roadmap is
`BEYOND-QUARTO.md`.

> Kept deliberately small (read often). **Only open tasks live here.** Completed work is
> in git + the history docs: `BEYOND-QUARTO.md` (Beyond-Quarto waves), `DROP-QUARTO.md`
> (the native-rewrite), `AUDITS.md` (the three audit passes). Don't re-add `[x]` items.

## State (2026-06-25, local `main` @ `5bca91a`, version 0.1.0; author pushes between sessions)

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

### Reader experience (the active thrust; idea pool in `FEATURE-IDEAS.md`)
Pattern for any new reader control: `window.qmdReaderMenu.addSection(title, node, onOpen)`; state
in the reader's own `localStorage` keyed by `location.pathname`; deck-skip; pre-paint via
`render/theme.rs` for anything that must not flash. **GOTCHA (line-spacing review):** prose CSS
like `body p, body li { … }` leaks into chrome that wraps prose (TOC, sidebars, navbar — all
`<nav><ul><li>`, search `role=listbox`, margin notes); re-pin with `nav li, [role="listbox"] li,
.sidenote p, .column-margin p, … { line-height: inherit }`. **For letter/word spacing the leak is
worse** (tracking *inherits into inline descendants*), so `5bca91a` also resets monospace + math
directly: `code, pre, kbd, samp, .katex { letter-spacing: normal; word-spacing: normal }`.
- Decided/known: the reader menu is intentionally an untrapped popover (not a modal); highlights
  are single-block prose only (margin notes / cross-block / colours were scoped out — see specs).

### Polish / docs
- [ ] **CI: wire `cargo-deny`.** `deny.toml` exists (Wave 0); the CI step was deferred
  (cargo-deny not installable/verifiable locally). Add it when CI is set up.

### Fidelity follow-ups (from the 2026-06-25 corpus sweep; detail in `AUDITS.md`)
- [ ] **Captioned code listing isn't a `<figure>`.** qmd-fast emits `div.qmd-listing` with a
  bare `<figcaption>` where Quarto uses `<figure class="quarto-float-lst">`. Minor/semantic.

### Library-outsourcing audit follow-ups (2026-06-25; method: multi-agent sweep of every from-scratch subsystem vs mature OSS, each candidate adversarially verified against the invariants)
- [ ] **Deck control-menu focus trap (small, deferred).** The lightbox + Cmd-K palette got the
  shared `qmdFocusTrap` (`7febc97`, hand-rolled, no npm dep); the deck control menus (`deck.js`)
  weren't covered. First check whether they are truly modal (vs. simple toggles) before trapping.
  The reader menu is intentionally a light-dismiss popover (not trapped — `aria-modal` would
  misrepresent it). Reject the `focus-trap` npm dep.
- [ ] **Correct the `serde_yaml` fallback target (watch-item).** The `Cargo.toml` workspace
  comment names `serde_yml` as the fallback, but it carries **RUSTSEC-2025-0068 (unsound +
  unmaintained)**; `serde_norway` is 1+ yr stale. The maintained continuation is
  **`serde_yaml_ng`** (v0.10). No urgency (input is trusted local config; 0.9 still builds). If
  0.9 ever breaks against a future serde/edition, swap to `serde_yaml_ng`, gated on a test that
  `Error::location().line()` still works (the only click-to-source-relevant API). Fix the stale
  comment when touched.
- [ ] **Dedup the FNV-1a hash (small footgun).** Byte-identical copies in `freeze.rs:49` +
  `render/mod.rs:1458` that MUST stay identical (the block-id scheme == the cache-key scheme).
  Pull into one shared helper. Do NOT swap the algorithm: seahash reintroduces cross-version
  instability that would break content-hash block ids, and xxhash/blake3 solve a non-problem.
- [ ] **Arg-parser unit tests (small).** `main.rs` hand-rolled arg dispatch is NOT test-covered
  (existing tests cover asset-mirroring only). Keep the parser (clap rejected: no real burden,
  fiddly to match the permissive flags-anywhere + `QMD_FAST_*` env-var ergonomics) but add tests
  around the `[out.html]` positional vs `--out <dir>` dual meaning + the port parse.
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
