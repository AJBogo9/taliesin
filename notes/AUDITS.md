# Taliesin audit records

The current deep audit + its active detail. The build-ready queue lives in
[backlog.md](backlog.md); older audit rounds (pre-2026-07-07) are archived in
[AUDITS-archive.md](AUDITS-archive.md).

-----------------------------------------------------------------------------

# Taliesin: full multi-surface deep audit (2026-07-07)

**Method.** One multi-agent workflow (87 agents, ~6.9M tokens): 24 surface×lens finder
cells (parser/block-model, render, decks, exec+kernel, freeze/warm-pool, site/books,
web-client, diagnostics/check, CLI/build, docs, deps/licensing, accessibility, reader
craft, architecture/waste, feature-scouting), each finding adversarially verified
(refute-by-default), then per-surface dedup + novelty-tagging against the existing
backlog / AUDITS / FEATURE-IDEAS, a philosophy gate on every cut/feature candidate, a
deep-dive pass on the five hottest surfaces, and a final synthesis. 134 findings
survived verification: 0 critical, 1 high code bug, 7 high total. One cluster synthesis
(CLI/build/first-run) hit the structured-output retry cap; its verified findings are
recovered in the appendix below. Read-only audit, no code changed.

**The build-ready, batched implementation queue derived from this report lives in
[backlog.md](backlog.md) ("Audit 2026-07-07 implementation queue").** This section is the
detail reference behind it: verdict, top-leverage fixes, findings by theme, cut/keep/add,
and the low-severity long tail.

## Status — mostly landed (updated 2026-07-08)

**This is the original 2026-07-07 snapshot, not a live checklist.** The batched queue and
the top-leverage fixes have since shipped, so most findings below are already fixed —
**verify against current code before acting on any of them** (this bit an earlier grind:
the `image:`-URL bug, the theming/`#qmd-root`/schema doc-drift, and the deck `{#sec-x}`
anchor all read as open here but are fixed). Remaining open work is tracked in
[backlog.md](backlog.md) (Tier-2/Tier-3), not here.

Landed, by theme (batch → the "Findings by theme" section it clears):

- Batch 2 (`0b466c4`) → **Documentation drift (rename)**, the functionally-broken cluster.
- Batch 3 (`2369d80`) → **Accessibility** (Cmd-K contrast/ARIA, reduced-motion, slide bg,
  keyboard scroll, dialog).
- Batch 4 (`1132df3`) → **Cross-reference / section numbering** + consumed-anchor.
- Batch 5 (`561ff24` + `a6cf810`) → **Silent failure** (unclosed fence, figure `width=`,
  `draft: yes`, single-doc-build YAML, `_site.yml` typos).
- Batch 6 (`92dc677`) → **BibTeX layer** + the TOC/tabset double-escape.
- Batch 7 (`19022b7`) → `image:` URL, multi-page stale-file sweep, embed `--strict`,
  **deck title-slide hot-update**.
- Batch 8 (`41313f9`) → watcher prune, live site search index, reconnect state-preserve.
- Batch 9 (`1eb3238`) → **freeze cache + kernel** honesty + resource hygiene (partial;
  the remaining exec leaks stay in backlog Tier-2).
- Also: top-leverage #1 offline deck build (`478cdc1`), the `?qmd=embed` CUT (`679b76b`),
  the diff-then-broadcast consolidation (`e09744a`), the 2026-07-08 hardening fixes
  (byte-safe `percent_decode`; active-nav highlight on `#fragment`/`?query`), and
  top-leverage #7 (`e488abb`, the same-page link-preview source-attr strip — a shared
  `stripSourceAttrs` now neutralizes both the same-page and cross-page cards).
- Confirmed already-fixed by the audit's own "What held up" and now closed: the 390px hero
  overflow, the theme/video desync, and the heading `{#id}` dedup gap.

Notable **still-open** low-tail items the sweep did *not* touch (so they don't read as
done): `block_tag_has_id` substring match, `app.pages` unbounded ws-key growth, `json_str`
U+2028/2029, the deck `. . .`/`"Title Slide"` collisions, several CLI/build appendix items,
and the stale-but-working `qmd-*` alias docs. See backlog Tier-2/Tier-3 for the tracked set.

## Executive verdict

Taliesin came through a 134-finding, 24-surface deep audit with no critical defects and a single high-severity code bug. The load-bearing invariants the whole design rests on, unique block ids, total sourcepos, block-level incremental diffing, the freeze cache's no-stale-hit promise, and the read-only single-editing-surface, all held up under direct adversarial attack (see "What held up" below). The findings are not decay; they are under-enforcement and over-claiming.

Three themes dominate:

1. **Silent failure is the default.** The largest single cluster (roughly two dozen findings) is features that no-op or render wrong with no diagnostic, in direct tension with the project's own "surface bad news early" directive. Examples span every layer: an unterminated `:::` fence is silently dropped, quoted figure `width=` values are corrupted by smart-punctuation, `draft: yes` silently publishes a draft, the `ts`/`typescript` highlight alias is dead, and the single-doc `build` (the artifact-producing path) never surfaces malformed front-matter YAML that `check` and both preview servers catch.

2. **Accessibility advertised but shallowly delivered.** The a11y foundation is real (per-theme accessible color tokens, an SR-only convention, a `prefers-reduced-motion` CSS gate, emitted ARIA), but several flagship surfaces bypass it: the Cmd-K palette fails WCAG AA in every theme, deck auto-animate/magic-move route around the reduced-motion gate, and three surfaces emit ARIA (`aria-haspopup="dialog"`, `aria-haspopup="menu"`, a named combobox) whose promised behavior they never implement.

3. **Documentation drift from the rename.** The qmd->tali / qmd-fast->Taliesin rename never fully reached the dogfooded books; three onboarding recipes are functionally broken (custom-theme CSS vars, LSP schema filenames, the protocol element id have no back-compat alias) and the User Guide teaches the theme default as the exact opposite of the runtime.

Nearly every fix is a mechanical reconnection of an existing mechanism, not new infrastructure. Confirmation quality was high: of the findings reported here, all but a couple are marked CONFIRMED against source; the few PLAUSIBLE ones are flagged inline.

## Top highest-leverage fixes

1. **Offline-build breach for decks (HIGH, CONFIRMED).** `deck_page_from_doc` takes no `OutputMode` and hardcodes `code_scripts()` = Preview (`crates/core/src/render/deck.rs:94-108`), so a `build`-ed deck containing a Mermaid diagram ships a `cdn.jsdelivr.net` dependency instead of the inlined offline library the HTML page path correctly emits, and also ships every Preview-only enhancer as dead bloat. Thread `OutputMode` through and pin with a corpus test asserting no `cdn.jsdelivr.net` in a built deck.

2. **One located-diagnostic channel for the silent no-ops.** The math and front-matter paths already turn failures into click-to-source `Warning`s; extending that channel to unclosed fences (`divs.rs:143`), unresolved fence languages (`highlight.rs:51`), non-boolean boolean keys (`site/frontmatter.rs:56`), non-`.bib` bibliographies (`render/mod.rs:773`), and unknown `_site.yml` keys (`config/mod.rs:206`) retires most of the largest cluster in one coherent piece of work.

3. **AA token sweep.** Swap raw `var(--tali-accent)` for the existing `var(--tali-accent-fill)`/`--tali-on-accent` in the Cmd-K palette (`web-client/search.js`, the one HIGH a11y offender) and darken the sub-AA syntax comment token per theme (`base.css:338`, sepia `:583`, `deck.css:760`). The accessible tokens already exist; add a contrast assertion to stop regressions.

4. **Unify section numbering.** The flat `sec_count` (`render/mod.rs:390`) is the shared root cause of three-to-four `@sec-` cross-reference bugs. Registering the hierarchical `section_number` when a chapter is present collapses all of them.

5. **Freeze / warm-kernel honesty pass.** Scope the "stale hit impossible by construction / nothing to clear by hand" wording (`freeze.rs:11`) to code + interpreter version (a library upgrade is a real stale-hit path), and fix the mid-run kernel death that poisons the warm-prefix `ran` and wedges the preview (`exec.rs:610`).

6. **Docs rename drift.** Sweep `--qmd-*` -> `--tali-*` and `qmd-*.schema.json` -> `tali-*.schema.json` in the guide, fix the `#qmd-root` -> `#tali-root` protocol id, and correct the inverted theme-default statement (`theming.tmd`).

7. **Same-page link-preview card leaks the source-map surface.** Apply the cross-page attribute-strip to the same-page path too (`12-link-preview.js`), so a read-only preview card stops being a live Alt-click click-to-source target and stops seeding duplicate block ids.

## Findings by theme

### Silent failure (surface-a-diagnostic instead)

- **Authoring surface.** Unterminated `:::` fence silently dropped, content unwrapped, no warning (`render/divs.rs:143`, medium). `::: .callout-note` (leading dot, braces forgotten) renders as literal text; bare `::: classname .extra #id` silently drops all but the first token (`divs.rs:111`, low). Quoted figure `width=`/`height=`/`fig-align=` corrupted by smart-punctuation so the feature silently no-ops, already live and non-functional at `bayesian-website/subsections/_data-modeling.tmd:4` (`figure.rs:55`, medium).
- **Config/validation.** `draft: yes`/`on` silently publishes the draft (YAML-1.2 coerces to the string, then to false); same class mis-reads `toc: yes` and `execute: {echo: no}` (`site/frontmatter.rs:56`, medium). Single-doc `build` never runs `yaml_error()`, so malformed front-matter YAML builds clean and passes `--strict` (`frontmatter.rs:107`, medium). `_site.yml` nested nav/footer/mount typos degrade silently and even top-level site warnings ship unlocated (`config/mod.rs:206`, medium). Non-`.bib` bibliography path ignored with no diagnostic (`render/mod.rs:773`, low). Non-HTML `format:` values (pdf/typst/docx...) pass `check` clean and are silently rendered as HTML (`frontmatter.rs`, low).
- **Highlighting.** `ts`/`typescript` alias is dead (syntect ships no TypeScript), so TS blocks render unhighlighted with no signal; `toml` likewise absent (`highlight.rs:36`, low). Any unresolved fence language degrades to plain text silently (`highlight.rs:51`, low).
- **Runtime.** The file watcher registers a recursive inotify watch over the whole tree including `node_modules`/`.git`, so a large project exhausts `max_user_watches` and silently kills hot reload (`serve/mod.rs:877`, medium). The site Cmd-K index freezes after a content edit in preview while single-doc search stays live (`serve_site/mod.rs:1035`, medium). Mermaid render errors are swallowed by an empty `catch`, and a successful re-render after an offline failure leaves the stale banner (`mermaid.js`, low).

### Cross-reference / citation correctness

- **Section numbering (shared root cause).** Same-page `@sec-x` in a book shows a flat number contradicting the heading's hierarchical number (`render/mod.rs:390`, medium). Cross-page `@sec-x` on a non-book website is mislabeled "Chapter N" (`site/xref.rs:215`, medium). The hover-preview card for a book section heading drops its number (`site/mod.rs:759`, low). All three collapse behind the `section_number` helper.
- **BibTeX layer.** `@inproceedings`/`@conference` silently drop `booktitle` and `pages`, the single most common citation type in the CS/ML audience (`cite/format.rs:22`, medium). Parenthesis-delimited entries cascade-drop every following reference (`cite/parse.rs:32`, medium). Manager exports (JabRef) commonly emit both forms.
- **Citation rendering (deep dive).** Pandoc-style prefix text is silently deleted from a bracket group (`[see @doe2020]` -> `[1]`) (`cite/render.rs`, low). A bib key beginning with an xref prefix (`rem-`, `fig-`) is uncitable and emits a spurious broken-xref warning (low). A locator on a bracketed cross-reference is dropped (low). `transform_html`'s tag scanner treats the first `>` as the tag end, so a `>` inside an HTML comment leaks citation processing into non-text context (low).
- **DOM ids.** Duplicate `fig-`/`eq-`/`tbl-`/`lst-` labels emit invalid duplicate DOM ids (only warned, not deduped); the heading half of this was already fixed, this is the non-heading remainder (`render/mod.rs:455`, low). A heading consumed as a callout title discards its `#id` while its `@sec-` number was already registered, leaving a resolving ref pointing at a missing anchor (`divs.rs:395`, medium). A manual mid-document References heading is left detached from its list (`cite/render.rs:81`, low).

### Load-bearing invariants under-enforced

- **Block model.** The diff's LIS silently assumes globally-unique block ids with no assertion in the post-exec hot path (`diff.rs:185`, low, trivial `debug_assert!`). A block straddling an include boundary emits a mixed-file sourcepos (start file + end line from a different file), a hole totality-only checks miss (`render/mod.rs:286`, low).
- **Incremental payload.** Ops are broadcast one-message-per-op onto a 256-slot ring; a large structural edit self-overflows it and forces a full re-render, discarding the ~83x incremental win (`serve/mod.rs:1069`, medium, already-tracked as op-batching but with new overflow detail). The diff-then-broadcast core is copy-pasted across the two dev servers, giving the payload-shape contract two owners (`serve/mod.rs:992`, medium). A websocket reconnect wholesale-remounts and destroys all live block state (WebGL/{js}/video/open-details) even when the document is byte-identical, on any sleep or wifi blip (`web-client/client.js`, medium).
- **Read-only preview leaks.** Same-page link-preview card keeps cloned `data-block-id`/`data-sourcepos` (`12-link-preview.js`, low). `qmd-cursor` reverse-sync accepts postMessage from any origin/frame (verified read-only, so no write-back breach) (`client.js`, low). `click_block` prints client-controlled strings to the author's terminal unsanitized, a terminal-escape injection beyond the documented worst case (`serve/mod.rs:756`, low).

### Freeze cache + kernel zone (Do-NOT-touch, read-only audit)

- **Over-claims.** Freeze key captures interpreter version but no package fingerprint, so a same-interpreter library upgrade is a realistic stale-hit path the docs say is "impossible by construction" (`freeze.rs:11`, medium; doc-scope fix, no knob). Pooled forkserver kernels pre-populate `sys.modules`, so warm vs cold renders can differ despite the "identical" claim (`exec.rs:253`, low, doc fix).
- **Wedge / correctness.** Mid-run kernel death poisons the in-memory warm-prefix `ran`, so a later code-unchanged rebuild never respawns and serves KERNEL_DIED placeholders indefinitely (`exec.rs:610`, medium). `is_uncacheable` false-positives on legitimate outputs containing the sentinel strings, defeating caching for the self-referential docs (`exec.rs:894`, low, PLAUSIBLE). `interp_id` memoizes an empty version string on a transient `--version` failure (`exec.rs:903`, low).
- **Resource hygiene.** `adopt_forked` leaks both the `/tmp` dir and the forked kernel process on a handshake/bind timeout (`kernel.rs`, medium). Forked-kernel liveness/SIGINT/SIGKILL keys on a bare recyclable PID, defeating KernelDied fast-fail under PID reuse (`kernel.rs`, low). (The failed-`Kernel::start` `/tmp` leak, the boot-diagnostic clobber, the stream-ANSI leak, and the `in_flight` slot leak sharpen existing Tier-2 items with exact paths rather than adding new ones.)

### Accessibility (advertised but shallow)

- **Contrast.** Cmd-K palette selected row + match highlights use raw `--tali-accent`, failing AA in every theme (`search.js`, high). Syntax comment token below 4.5:1 in light/sepia/light-deck (`base.css:338`/`:583`, `deck.css:760`, medium/low). Deck `.tali-menu-slide-n` numerals sub-AA (low).
- **ARIA overpromise.** Book chapter drawer advertises `aria-haspopup="dialog"` and dims the page but has no `role=dialog`/`aria-modal`/focus-trap; same for the mobile TOC sheet (`site/chrome.rs`, medium). Deck control menu declares `aria-haspopup="menu"` over a plain button group with no roving focus (`deck.js:1466`, low). Cmd-K combobox splits `role`/`aria-expanded`/`aria-activedescendant` across wrapper and input, leaves the listbox unnamed, and never clears a stale activedescendant on empty results (`search.js:142`, medium).
- **Motion.** Deck auto-animate FLIP and magic-move morph set inline transitions that bypass the `prefers-reduced-motion` CSS gate (`deck.js:389`, medium). JS-initiated smooth scrolls hardcode `behavior:'smooth'` with no reduced-motion guard across search, reading-progress, and client navigations, and these ship in the static build (`search.js:553` and others, medium).
- **Keyboard / AT.** Overflowing `<pre>` and wide tables are horizontal scroll containers but not keyboard-scrollable, a hard WCAG 2.1.1 failure on the most common content type (`base.css`, medium). Lightbox decoration turns `pre.mermaid` into a `role=button` leaf, hiding the diagram's SVG content from AT, and forces decorative `alt=""` images into focusable tab stops (`11-lightbox.js:178`, medium). Non-hex per-slide background colors are assumed dark, flipping heading/body text to invisible white on light named backgrounds (`deck.js:337`, medium). Deck fragment reveals are not announced to screen readers (`deck.js:449`, low). Lightbox + link-preview lack the `.tali-deck` guard, so deck nav double-handles Arrow/Esc over an open zoomed figure (`11-lightbox.js`, low). Theme/Focus segmented controls use `aria-pressed` toggles for a single-select choice with no arrow-key nav (`14-reader-prefs.js:24`, low). The Resume-reading pill auto-dismisses after 8s with no announcement (WCAG 2.2.1) (`15-reading-progress.js:88`, low). Mobile TOC handle chip leaks the SR-only "(read)" suffix (`toc-sheet.js`, low).

### Reader experience, theming, visual craft

- Floated sidenote/margin-note has no `has-toc` guard, so above 73rem it collides with the sticky TOC (`base.css:554`, medium). `--tali-flash` unthemed for sepia, so live-edit pulses render blue on the warm page (`base.css:35`, low). Sepia comment token 3.47:1 (`base.css:583`, low). Two conflicting `.tali-input` rule blocks (`base.css:779`, low, see CUT). TOC entries and `.panel-tabset` labels double-escape `&`/`<`/`>` because `html_escape` is layered on already-safe `strip_tags` output (`render/mod.rs:1608`, `divs.rs:528`, medium).

### Deck engine (Rust + client, beyond the above)

- Deck title slide is injected as a raw string outside `doc.blocks`, so front-matter title/subtitle edits produce an empty diff and never hot-update the preview (`deck.rs:206-225`, medium). Explicit `{#sec-x}` on a slide heading is dropped, so `@sec-` renders a dead link (`deck.rs`, medium). A code block whose only content is `. . .` is swallowed as a pause marker (`deck.rs`, low). A slide titled "Title Slide" collides with the hardcoded `id="title-slide"` (`deck.rs`, low). Two `{{< input >}}` on one line collide on `qin-{line_no}` (`extension/mod.rs:231`, low). Theme-switched `{{< video dark= >}}` downloads both clips (`extension/mod.rs:311`, low, PLAUSIBLE). Overview touch swipe double-fires (pans + advances index, tile highlight desyncs) (`deck.js:1398`, low). Speaker Current/Next preview is blank for `<canvas>`-rendered cells because `cloneNode` skips the bitmap (`deck.js:983`, low). Internal `DocFormat::Reveal`/`is_reveal_doc` naming survives across ~73 sites after the engine was removed (`model.rs:119`, low).

### Site, protocol, watcher, security

- Absolute `image:` URL is mangled into a broken relative path, breaking og:image + listing social cards (`site/discovery.rs:26`, medium). Single-mapping nav/footer items and href-less bare strings are silently dropped (`config/mod.rs:222`, low). `block_tag_has_id` matches id as a substring, so a listing id can bind to `data-block-id` (`links.rs:119`, low). Active-nav highlight lost on a `#fragment`/`?query` href (`links.rs:51`, low). `percent_decode` slice-panics on a raw non-ASCII request path (`serve/mod.rs:420`, low). `app.pages` grows unbounded from bogus ws `?page=` keys, each preallocating a broadcast ring (`serve_site/mod.rs:630`, low). Front-matter theme flip between built-in light and dark is not hot-swapped (`serve/mod.rs:1043`, low).

### Cmd-K search (beyond staleness + combobox)

- `json_str` leaves U+2028/U+2029 unescaped; a paragraph/line separator in prose can break the whole inlined-JS index on pre-ES2019 engines (`search.rs:159`, low). Single-doc DOM index omits h5/h6 that the server index includes (`search.js:80`, low). The site index is re-mapped and re-lowercased on every open despite a "memoized once" comment (`search.js:70`, low). Fuzzy matcher scans every word of every section per keystroke for a 4+ char non-substring term (`search.js:232`, low, situational).

### Architecture / waste

- The two dev servers duplicate the load-bearing diff-then-broadcast contract, a real drift risk (`serve/mod.rs:992`, medium). Site discover renders every page 2-4x with no shared `RenderedDoc`, and `harvest_xref_numbers` lacks the empty-guard its sibling `build_hover_index` has, so a plain blog pays a full discarded render per page (`site/mod.rs:703`, low; trivial quick-win + medium consolidation). `render_internal_impl` (~500 lines) and `compute_outputs` (~260 lines, in the protected zone) are single dense functions producing the load-bearing block model / freeze reuse (`render/mod.rs:177`, `exec.rs:376`, low). Warning->Diagnostic conversion is copy-pasted four times (`serve/mod.rs:1006`, trivial). Cell-error scanning duplicated across build paths (`build.rs:239`, low). ~90 lines of dev-menu CSS embedded as a Rust string literal (`serve/mod.rs:450`, low). `render/mod.rs` also mixes ~180 lines of asset/script plumbing and attribute post-processing with the orchestrator (low).

### Documentation drift (rename)

- **Functionally broken (no runtime alias).** Theming chapter documents `--qmd-*` CSS vars the runtime never reads (`theming.tmd:153`, high). Schema on-ramp references `qmd-*.schema.json` but the tool emits `tali-*.schema.json` (`frontmatter.tmd:261`, high). Protocol book documents `#qmd-root`; the element id is `#tali-root` (`protocol.tmd:59`, medium).
- **Inverted / wrong guidance.** The guide teaches the theme default as "settles to dark, never follows the OS", the exact opposite of the resolver (auto = follow OS, light fallback), and contradicts the Internals book (`theming.tmd:27-35`, high). Front-matter refs say page-level `image-alt` is ignored and cards emit empty alt; code emits it (test-pinned), so the a11y guidance is inverted (`frontmatter.tmd:55`, medium). getting-started/CLI claim a shipped `~/.local/bin/Taliesin` launcher that does not exist, with wrong casing (`getting-started.tmd:16`, medium). Troubleshooting says the companion defaults to `taliesin`; the extension defaults to the dead `qmd-fast` binary (`troubleshooting.tmd:118`, medium). Internals execution chapter documents `qmd-*` output classes and a persistence guard on `class="qmd-error"` the runtime never emits (`execution.tmd`, medium).
- **Stale-but-working (aliases exist).** Guide's `viewof` example and ~10 corpus posts apply a dead `qmd-input` class (`code.tmd:169`, low). Guide teaches `qmd.*` cell API and `qmd-embed`/`qmd-video`/`qmd-fnref`/`qmd-main` classes as canonical (`code.tmd`, low). Internals teach `window.qmdEnhancers`/`QmdDeck` inconsistently (low). `IRkernel::installspec()` is a no-op Taliesin never uses (`getting-started.tmd:35`, low). README has no License section despite an MIT LICENSE (`README.md`, low). THIRD_PARTY.md claims cargo-deny CI wiring is deferred when it is done, references the deleted `code-enhance.js`, and omits `scrolly.js` (`THIRD_PARTY.md:56`, low). Preview Mermaid CDN pin and the vendored build copy can silently diverge with no provenance guard (`render/mod.rs:877`, low).

## CUT / KEEP / ADD

Driven by the philosophy-gate verdicts. Taliesin's north star (HTML-only, minimal-config, single-editing-surface, no per-edit startup cost) is the arbiter.

### CUT

- **`?qmd=embed` deck mode (adopt).** Verified unreachable dead code: speaker previews now use snapshot clones, and `{{< embed >}}` drives embedding through `window.taliDeckEmbedded`, not a URL mode. Drop the ternary branch and refresh the stale comments (`deck.js:1607`). Shrinks the deck's public state surface, no behavior change.

### KEEP (proposed cuts that should not be cut)

- **`data-level` deck attribute (defer, lean keep).** Not dead: it is a semantic heading-level marker and the count anchor for `corpus.rs:214`. Costs nothing at runtime. Keep and document it as a deliberate test/anchor hook rather than remove it and make the corpus count more fragile.
- **Duplicate `.tali-input` CSS blocks (defer, not a clean cut).** The two blocks style two different features sharing one wrapper class (`{{< input >}}` controls vs `//| viewof:` js-cell inputs). A naive merge would change the `{{< input >}}` layout. Decide the fix (unify deliberately, or split the wrapper classes) first; "cleanup with no user-facing change" is inaccurate.

### ADD (philosophy-gated new capability)

Only two clear adopts; the rest are aligned-but-deferred pending a design call.

- **Shareable/deep-linkable `{{< input >}}` state via the URL fragment (ADOPT).** The textbook realization of "wider = richer browser behavior in a live HTML view": reader-local URL/fragment state hydrated from the existing `data-qmd-input` registry, ~50-80 JS lines, no Rust/model change, no config knob, no write-back. Guardrail: must stay pure reader-local URL state (never a persisted server session) and must coexist with the existing deck (`#/h/v`) and block-anchor fragment routing.
- **Reader text-size and line-spacing controls (ADOPT, explicitly sanctioned).** CLAUDE.md names theme, text size, and spacing as first-class, a11y-exempt reader rights, yet only Theme ships. The whole substrate exists (`window.taliReaderMenu.addSection`, the pre-paint pref script, the segmented widget, `--tali-*` type vars). Add `--tali-reader-scale` and a line-spacing segment, persisted reader-local like theme (`14-reader-prefs.js`). This is the sanctioned exception to "better default over a knob".
- **Deferred but aligned (need a scoping/default ruling before building):** cross-revision block diff ("what changed in this document", the single most on-brand capability durable block identity enables); a reader-facing reproducibility manifest surfacing the freeze/warm-kernel provenance (the strategic wedge vs Jupyter/Quarto); a web-native List of Figures/Tables/Theorems for books; opt-in interactive data tables (sort/filter); a document-level "Cite this" export; code-line cross-references (`@lst-3:line`); theme-aware figures (a `dark=` image variant mirroring the shipped video `dark=`); copy-as-TeX on equations; cell-output export (save PNG / download CSV); estimated reading time; reader-local text highlighting. Each reuses shipped substrate, stays HTML-only and read-only, but collides with either minimal-config (opt-in vs better-default) or an open addressing/scope decision, so none is a clean drop-in.

## What held up under attack (negative space)

The audit is most reassuring in what it did **not** find:

- **The single-editing-surface invariant holds.** The full inbound message surface was traced: `click_block` logging, `restart_kernel`, `qmd-goto` navigation, and reverse cursor-sync are the only paths, and every one is read-only to source. No preview gesture writes back to the `.tmd`. The two "leaks" found (the same-page preview card's cloned ids, the un-origin-checked `qmd-cursor` listener) are source-map-hygiene and defense-in-depth, not write-back breaches.
- **Sourcepos emission is total.** The only hole is the rare include-boundary straddle; the corpus totality test's premise otherwise holds across every block type.
- **The heading `{#id}` dedup gap is already fixed** (`render/mod.rs:402-413` routes explicit ids through `dedup_with_suffix` + warns); only the non-heading anchors remain.
- **The offline invariant holds for HTML pages.** `build`/`render` inline every vendored asset and never touch the network; the deck path is the lone regression, and it is mechanical.
- **The exec/kernel Do-NOT-touch zone was respected and is fundamentally correct.** Every kernel-zone finding is resource hygiene or a doc over-claim; execution semantics and the freeze keying are sound. No stale-hit-by-construction bug was found except the honestly-scopeable dependency-upgrade axis.
- **The deck output contract is clean.** No reveal.js vocabulary leaks into the DOM (`.tali-deck`/`.tali-slide`/`window.TaliesinDeck`); the residue is purely internal type naming.
- **Two "open" visual bugs are already fixed** (390px hero overflow via `box-sizing:border-box`; theme/video desync via `data-theme` driving, zero `prefers-color-scheme` left in bundled assets), so the backlog/AUDITS entries for them can be closed.
- **No critical or high-severity security finding.** Every security item is LOW and consistent with the single-trusted-author, loopback threat model.

### Appendix, CLI / build / first-run (cluster synthesis errored; recovered from the verify journal)

These seven were confirmed by the adversarial verifier but missed the final synthesis
(their cluster agent hit the retry cap):

- **Multi-page `_site` build never sweeps stale files** (net-new, medium): `build_site_async`
  only writes/adds, so renamed or deleted pages persist in `_site/` across rebuilds
  (`crates/server/src/build.rs`). A stale-file sweep (or clean-then-write) would fix it.
- **Embed warnings don't count toward `problems`** (medium): the embed warning loop
  (`build.rs:330-336`) calls `log::warn` but never increments `problems`, unlike
  `doc.warnings`/xrefs, so `--strict` and the exit code under-count them.
- **Positional-target build has no parent-dir creation** (low): `build.rs:205` is a bare
  `fs::write` while the `--out` path calls `create_dir_all` (`build.rs:372`), so
  `build doc.tmd out/sub/x.html` fails when `out/sub` is absent.
- **`--jobs` ignored for single-file builds** (low): parsed into `BuildArgs.jobs` but only
  passed to `build_site` (`build.rs:154`); the single-file branch (156+) drops it.
- **No-kernel build warning doesn't name the language** (low): `build_one_page`
  (`build.rs:714`) reduces the diagnostic to a bool and `PageOutcome` carries no language.
- **Single-doc `serve` swallows a file-read error** (low): `render_doc` (`serve/mod.rs:287`)
  drops the read error via `.ok()?`; `serve()` handles `Ok(None)` silently, so an
  unreadable file serves nothing with no console message.
- **`log::info` reuses the green `built` tag** (low): `log.rs:148-150` routes through
  `Style::Built`, the same tag a real file write uses (`log.rs:30`), making console output
  ambiguous.

-----------------------------------------------------------------------------

**Older audit rounds (2026-06-19 through 2026-06-30) are archived in [AUDITS-archive.md](AUDITS-archive.md).**
