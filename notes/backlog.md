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

**Still open — high value**
- [ ] **a11y gate runnable from `check`** (the last slice of the a11y-parity item; the rest shipped
  2026-06-27). The client-side a11y audit still lives only in live preview — port a static subset
  (missing landmarks, `img:not([alt])`, statically-knowable contrast) into the `check` channel so a
  green `check` also vouches for a11y. Pairs with the now-server-side `alt`/landmarks/skip-link.
- [ ] **`check`: broken plain/external links + per-subcommand `--help`.** External `http(s)` links are
  intentionally not fetched (the rest of the link/anchor/video/reactive superset shipped 2026-06-27);
  if ever wanted, gate behind an opt-in online flag. `usage()` now advertises `<dir>` + did-you-mean,
  but a true per-subcommand `--help` is still a one-line-each gap.

**Still open — medium**
- [ ] **[B] Book chapter sidebar dumps stacked on top of content at ≤~900px (laptop-portrait band)** instead
  of collapsing to a drawer. (Deck scales+centers gracefully in portrait — leave it.)
- [ ] **CLI microcopy:** build kernel hint hardcodes `QMD_FAST_PYTHON` even when R failed (discards the
  language-aware `exec.rs` diagnostic); raw ANSI leaks into HTML for R stream/stderr (`kernel.rs:672` —
  **DEFERRED, exec/kernel Do-NOT-touch**); `check --format json` on an unreadable path prints human text to
  stderr (breaks `| jq`); a `_quarto.yml` project yields a `_site.yml: no _site.yml` diagnostic naming a
  nonexistent file (add a migration breadcrumb).
- [ ] Long tail (perf: shared/minified/compressed assets, O(change) per-edit; SEO: sitemap/robots/JSON-LD;
  publish hygiene: stop mirroring `.md`/`.scss`/planning into `_site/`; dead CSS/tokens `--qmd-ink`/`--qmd-scale`;
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
