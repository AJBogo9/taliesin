# Taliesin backlog

**Scope: corpus-plus-roadmap.** "Done" = the docs under `corpus/` render correctly (the
corpus is the regression net); each new capability ships pinned by a target corpus doc.
Output stays **HTML-only**. Roadmap: `ROADMAP.md`.

> Kept small (read often). **Only open tasks live here** — delete items once landed; don't
> leave `[x]`. Completed work is in git + `ROADMAP.md` / `native-rewrite.md` / `AUDITS.md`.

## State (2026-07-11)

v0.2.0. All four formats render + deploy; the dev loop is strong (block-level incremental
updates with DOM-state preservation, warm server + Jupyter kernel, `_freeze` cache, Alt-click +
reverse cursor sync, located diagnostics, CSS hot-swap, Cmd-K search). **Tier 1 is empty** — the
2026-07-09 polish audit's Batches A-F all landed 2026-07-10 (detail in git + `ROADMAP.md` /
`AUDITS.md`). What remains open is below: a handful of small carry-overs, the theme/a11y
follow-ups, Tier 2 hardening, the Tier 3 demand-driven queue, and the owner-gated rulings.

Agents commit + fast-forward-merge to local main on request, and push to `origin/main` only when
the author explicitly asks.

**Working method:** branch per feature; brainstorm if there's a fork; spec under
`docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
extension harnesses); fast-forward merge locally; delete the item here. **Do-NOT-touch:** the
exec/kernel zone + the single-editing-surface invariant. Review subagents use read-only git.
**Author policy (feature-first):** finish framework features before marketing-site work.

## Needs your ruling (the blockers)

None block Tier 1 (which is empty). Six rulings owed, each filed in full under "Owner-gated"
below:
- **Draft-aware preview** (flips a default: `draft: true` currently also hides from *preview*).
- **Reading time in the built page** (reverses a decision a corpus test pins).
- **`taliesin publish --public`** (relaxes the fail-closed passcode gate).
- **Built-site shared asset bundle** (changes the shape of the build output).
- **Plain `publish` strict by default** (`publish --strict` already inherits the full check
  superset; making it the default is a fail-closed change, so it was not assumed).
- **TODO / FIXME surfacing** — design A (preview-only info) vs design B (a real severity level
  through the shared check/build/publish gate). *Owner ruled 2026-07-10: skip for now; analysis kept below.*

## Priority queue

### Tier 1 — small, build-ready (no blocker)

Small carry-overs and one-liners surfaced while landing the 2026-07-09/07-10 batches. Full
evidence in [2026-07-09-polish-audit-findings.md](2026-07-09-polish-audit-findings.md).

- **A `.theorem`/`.lemma`/... div whose id lacks the kind prefix is silently unreferenceable, and
  nothing says so** (S, med; a new `check` rule). `::: {.theorem #pythagoras}` is numbered and
  displayed as "Theorem 1", but `cite::is_xref_anchor("pythagoras")` is false (no `-`), so
  `@pythagoras` renders as **literal text** and `check` reports "no problems found". The div's id
  is registered into `xref_numbers` unfiltered, unlike the `sec-`/`fig-`/`tbl-` paths which gate
  on the prefix. Prefer fix (a): warn at registration ("theorem id `pythagoras` cannot be
  referenced; use `thm-pythagoras`"). *Pin: `corpus/diagnostics/`.*
- **`og:title` and the listing card still read `Page::title`, so they disagree with `<title>`.**
  A *website* page with no front-matter title and a leading `# H1` now renders a correct `<title>`
  but an `og:title` borrowed from the site name, and a listing card labelled by its rel-path.
  Book chapters are unaffected (`book.rs` already falls back to the H1). Coherent fix: give
  `site/discovery.rs`'s `website_pages` the same H1 fallback `book.rs::push_chapter` has, so
  `<title>`, `og:title`, cards, nav and search all agree. Widens blast radius to four consumers;
  no corpus page exercises it (add a fixture first).
- **Shell completions** (M, med; audit §7). No completions and no seam: the CLI is hand-rolled, no
  clap. 12 stable command names; a static bash/zsh/fish script is ~120 lines. If built, gate the
  command list against `main.rs::COMMANDS` so it cannot drift, the way
  `env_help_lists_every_runtime_env_var` gates the ENV block.
- **The date renders verbatim** (S, low, taste; audit §8). `render/mod.rs` emits `2026-04-14`, not
  "14 April 2026". Pure taste, no defect. (Alt-less `![](x.png)` deliberately not filed: alt-less
  is the a11y convention for decorative.)

Stale-doc one-liners (each reproduced; harmless but true):
- `corpus/tech-blog/.claude/skills/new-post/SKILL.md` + `.../new-project/SKILL.md` are still stale
  (emit `.qmd`, say `quarto preview`). The `new-post` one is the workaround `taliesin new` retires;
  left alone deliberately so as not to remove the feature's evidence. `taliesin new` has no
  `project` kind, so `new-project` was left rather than half-migrated. Decide whether to retire both.
- The twinned post dirs disagree on what is **git-tracked**: `tech-blog/posts/fourier-transform/`
  tracks `thumbnail.png` (unreferenced; `image:` names the `.webp`) and four generated `.wav`
  files, while `posts/fourier-transform/` tracks neither. The `.wav`s are written at render time by
  the post's own `{python}` cell, so tracking them is the anomaly. Harmless, but decide one way.
- The companion's `symbolCache` only invalidates on save (`completions.ts`, low). An out-of-band
  change (`git checkout`, an external formatter) fires `onDidChangeTextDocument`, not
  `onDidSaveTextDocument`, so cell-labelled targets lag until the next save. Bounded and graceful
  (the `xref` case unions with the live buffer scan). Left as-is deliberately; noted so it isn't
  re-discovered.

**One carry-over, not an open task:** the `showcase` 3D canvas is absent from a no-scroll
full-page capture at 390px, because its `IntersectionObserver` never fires while the host is below
the fold. A reader who scrolls gets it. To make the harness capture it, emulate
`prefers-reduced-motion: reduce` in `browser.mjs` `forceTheme`; `build()` then runs synchronously
— but it would then never see the animated one. Not decided.

### Theme colour-system follow-ups (2026-07-09 colour audit)

The colour audit itself landed (one owned iron-gall accent at OKLCH H271; nine vendor hexes banned
by a test; `--tali-border-strong`; opacity-dimmed text → `--tali-muted`; xref underlines;
print/prefers-contrast specificity; Auto theme). These findings **survived adversarial
verification but were NOT built** (WCAG + APCA + OKLCH + Vienot-CVD harness evidence):

- **Bare `f` forces native fullscreen with no opt-out** (`03-focus-mode.js:80`, medium). An
  unmodified single key both toggles focus mode and calls `requestFullscreen()`. WCAG 2.1.4 wants
  a way to turn single-key shortcuts off. Fix: keep `requestFullscreen` on an explicit menu action,
  add a reader toggle to disable single-key shortcuts.
- **Settings popover never takes focus when opened** (`13-reader-menu.js:60`, medium). `openMenu()`
  unhides a body-end panel and does not focus it; Esc already restores focus to the launcher
  (`:79`), so the asymmetry is the bug. Fix: focus the panel's first control on open.
- **Category-filter chips expose state only visually** (`10-category-filter.js:27`, medium). A bare
  `classList.toggle('tali-cat-active')`, no `aria-pressed`, and the filtered result is never
  announced. Fix: mirror the class with `aria-pressed`, render it on the server's initial "All"
  chip, and write "Showing 4 of 12 posts" into a visually-hidden `aria-live="polite"` node.
- **Embedded deck ignores a sepia host** (`render/deck.rs:164`, medium). `hostTheme()` accepts only
  light/dark, so an `{{< embed deck.tmd >}}` in a sepia page can drop a dark panel into cream
  paper. Minimal fix: map `sepia -> light` so the deck matches the host's lightness.
- **Link preview is hover-only** (`12-link-preview.js:174`, low). `mouseover`/`mouseout` are the
  only triggers; grep finds zero `focusin` in `assets/js/`. Keyboard readers never get the
  citation/xref preview. Fix: bind `focusin`/`focusout` too, set `aria-describedby` while open.
- **`forced-color-adjust: none` hides the current nav item** (`site.css:293` + `base.css:780`,
  low). Pins `.tali-nav-active` / `a[aria-current="page"]` to an author foreground with no author
  background, so under a High-Contrast OS theme of the opposite polarity the "you are here" marker
  vanishes. Only the reader-seg pressed button (which pins a matching bg+fg pair) needs the opt-out.
- **Deck slide-number chip is not restyled per-slide** (`deck.css:455`, low). The dark restyle is
  scoped to whole-deck `html.tali-deck-dark`, so on a `.tali-dark-bg` slide the chip reads ~2.8-3.0:1.
- **Settings panel does not reflow at 200% text.** The content-loss half is fixed (`box-sizing:
  border-box` + `calc(100vw - 2rem)` cap), but at 200% text the seg buttons and shortcut list still
  overflow into a horizontal scroll. Needs a real reflow (stack the rows), not a token change.

Owner calls (kept as-is deliberately, one-line changes if ever wanted):
- **Table cells still use the 1.28:1 hairline** (`base.css:436`). `--tali-border-strong` applied to
  controls only; whether a data table's grid is "required to understand the content" (WCAG 1.4.11)
  is a judgment call, and border-strong on every cell visibly heavies every table.
- **Callout `tip` vs `important` collapse under protanopia** (dE 9.1; deutan worst pair 17.7).
  Darkening `tip` lifts every dichromat pair ≥ 11 at the cost of the family's uniform weight. Owner
  kept the uniform family: the icon shape + text title already carry the meaning, hue is never the
  sole cue.
- **Deck has no sepia palette** (`deck.css`). A sepia reader gets a stark white/black deck. Either
  document decks as deliberately light/dark-only, or add the palette and teach the deck reader/
  scroll path to adopt it. (The reader menu already skips decks, so nothing is broken today.)
- **No owned typeface** (`base.css:18`, brand). Typography is 100% system stack, named as the
  biggest non-colour "assembled from defaults" tell. Bundling ONE distinctive-but-readable face
  offline (as the KaTeX fonts already ship) is a better default, not a knob. Avoid the
  display-serif cliche. (Overlaps the reading-first layout+type session below.)

### Decided 2026-07-07 — each needs its own dedicated session
- **Quarto design-decisions catalog triage, reframed.** Branch `quarto-decisions-catalog`, commit
  `535b4e1`: 165 decisions, adversarially verified. Rule on each by "is this the right design for
  Taliesin", not "does it beat Quarto" — the same-day repositioning commit (`de3de37`) retired
  Quarto as the defining reference. Fan the 165 into batches, each with a recommended verdict +
  evidence, so you rule, not derive.
- **Reading-first identity polish** (design judgment; overlaps deferred marketing: confirm
  direction before building). The theme/colour half landed 2026-07-09. What remains is layout +
  type: hero-as-typeset not a marketing slab; drop bordered feature-card grids; a `--space-1..6`
  scale; and the owned typeface (see the colour-system follow-ups above).

### Tier 2 — hardening (P3)
- **Execution-cache leaks — remainder open** (exec/kernel Do-NOT-touch, careful). The
  forkserver/dir/slot trio + the graceful-shutdown + fork-protocol follow-ups all landed 2026-07-08.
  **Still open:**
  - **Ungraceful-death path (S/M):** no defense against SIGKILL / a closed terminal / a crash, which
    no `Drop` can catch. Confirmed absent: `PR_SET_PDEATHSIG` on the warm-pool helper (grep for
    `PDEATHSIG|prctl` in `crates/server/src/` is empty; the helper already has its own process
    group, so the signal is cheap to add), and any startup sweep of stale `/tmp/tali-warmpool-*` /
    `/tmp/tali-kernel-*` dirs whose owner pid is dead. Measured live 2026-07-10: a `kill -9` on a
    `preview` orphaned 8 processes (451 MB — the pool preloads `numpy, matplotlib, torch`) + 123
    `/tmp/tali-*` dirs.
  - **Flaky timing tests (LOAD-sensitive, not orphan-sensitive).**
    `exec::tests::pooled_kernel_serves_cells_without_a_long_warming_state` **and**
    `kernel::tests::kernel_executes_state_errors_and_interrupts_runaway_cell` both fail under CPU
    load (measured: 2/3 full-suite runs with a concurrent `cargo test`; 1/4 with nothing else;
    0/6 running only the bin unittests; 0/4 on the untouched parent commit). Both spawn a real kernel
    and assert on **timing**. The fix likely means making the two assertions wait on a **state
    signal** rather than a duration, not just reaping orphans.
  - `build.rs:926` warms the pool before knowing whether any page needs a kernel, and does so **even
    under `TALIESIN_NO_EXEC=1`** (neither `build.rs` nor `warm_pool.rs` consults it). Hygiene item,
    **not** perf: measured 0.25 s vs 0.27 s on a prose-only site, so the boot is off the critical path.
  - Pre-existing `fork_kernel` cross-call edge (low): if a fork times out but its request was queued,
    the daemon's later `SPAWNED <pid>` is read by the *next* `fork_kernel`, mis-pairing pids. Rare now
    that the fork-protocol fix removed the main timeout trigger; proper fix is to poison the daemon on
    any fork timeout so later `take`s cold-start.
  - R stream/stderr still leaks raw ANSI into HTML (`kernel.rs` `Output::Stream` emits `esc(text)`
    with no `strip_ansi`, do-not-touch).
- **Testing / CI — `assets/js/*` `tsc`/`@ts-check` pass** (its own large session). The web-client
  tier is done (`client.js` + `search.js` + `toc-spy.js` + `toc-sheet.js`, `@ts-check`'d and in the
  CI `typecheck` job). Remaining is `crates/core/assets/js`: measured **812 errors** on a throwaway
  strict jsconfig (`deck.js` alone 402), plus `qmd-js.js`/`scrolly.js`/`tabset.js`/`walkthrough.js`/
  `mermaid.js` + the 16 `code-enhance/` fragments (exclude the vendored `*.min.js`). Needs its own
  ambient globals + a config that compiles the concatenated `code-enhance/` fragments as one shared
  script scope (confirmed: compiling fragments in isolation adds 12 `TS2304`s that vanish when
  concatenated).
- **Security:** injected Mermaid `<script>` SRI + `crossorigin` — deferred (only the live Preview
  lazy-loads mermaid from the CDN; a static build inlines the vendored copy). Needs a hash pinned to
  the CDN build; both `integrity` + `crossorigin` would break a non-CORS `TALIESIN_MERMAID_URL`
  override.
- **Deck engine (P2, deferred):** drop `fitSlide` from the resize path (needs a lazy fit-on-show
  refactor first); mobile pinch/pan + touch gestures (hard to verify without a device); thread
  `footer:`/`logo:` through both deck-page builders (no corpus deck needs one yet).
- **Perf (low):** protocol-level op-message batching (one WS message per save, not one-per-op). The
  realistic worst case is an edit near the top of a long doc, where every downstream block emits a
  `SetMeta` for its shifted sourcepos (`diff.rs` `anchor_op`). Client and server ship together, so
  no wire-compat constraint.
- **Audit long-tail** (`AUDITS.md`): a tens-of-MB cell output blocks ZMQ receive before the cap
  fires (`kernel.rs`, exec/kernel Do-NOT-touch).

### Tier 3 — deferred / demand-driven
- **Companion (Phase 2):** editor commands (`.tmd`-buffer text transforms only, never preview
  gestures); `editor.wordWrap` default for `[taliesin]` (respect the global setting until prose
  overflow is a real complaint, then ship `"on"`); grammar polish (YAML-type the `#|`/`//|`/`%%|`
  option value; recommend the cell-language extensions via `.vscode/extensions.json`); **marketplace
  packaging hygiene** (`.vscodeignore` does not exclude `.vscode-test/` (**1.8 GB**),
  `test-fixtures/`, `scripts/`, `out/test/`, `out/e2e/`; no top-level
  `icon`/`repository`/`license`/`keywords`; `"private": true` blocks publish). Diagnostics are
  save-triggered and whole-line (`diagnostics.ts:66-68`), fine for this workflow and subsumed by the
  LSP direction below.
- **`.tmd` format-on-save** (open question): a source pretty-printer writing the editor buffer must
  preserve `data-sourcepos` line stability for click-to-source — brainstorm reflow-vs-risk first.
- **Dogfood: migrate the FL-weather book to Taliesin** — a real-world Quarto→Taliesin migration +
  portability stress test; pin a reduced version under `corpus/` if it renders clean.
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic).
- **`taliesin publish` follow-ups** (command shipped 2026-07-08: build + shared-passcode gate +
  `wrangler pages deploy`): optional `--init` wrapper for the one-time `wrangler` setup;
  email-allowlist (Cloudflare Access) mode; **`--public` / `publish.gate: false`** (owner-gated
  below — relaxes a fail-closed default).
- **Interactive/explorable numerics** (`FEATURE-IDEAS.md` #62-66; none spec'd/pinned — promote with
  a corpus pin when one graduates; must NOT reintroduce a reactive VM). Highest-leverage: **#62** a
  bundled numerics/stats global for `{js}` (distributions, seeded PRNG, small dense linalg) + **#63**
  `animate`/play-tick + draggable-`point` `{{< input >}}` types. Then #64 `qmd.state` cross-re-run
  store, #65 richer `{js}` output helpers, #66 opt-in Pyodide `{python}` (~10 MB, no torch).
- **Wave 5** (`ROADMAP.md`): print-pdf track (paged render *of* the built HTML), docs-as-spec,
  `{glsl}` cell language, SEO completeness (sitemap/robots/JSON-LD at publish with `url:`). **Fold
  `llms.txt` + `llms-full.txt` into the SEO item:** the old deploy ritual generated it and the
  migration silently dropped the capability. The block model already separates clean prose from code
  and math (`client.js:50`), so it would be more accurate than the Python scraper it replaced. A
  plain-text sidecar is the same category as `sitemap.xml`, not a new output format. *Pin: a
  `tech_blog.rs` assertion that `llms.txt` lists the discovered pages and `llms-full.txt` excludes
  drafts.* Verified absent: no `llms` hit anywhere in `crates/`.
- **Site-level shared bibliography + bib hygiene** (M, med-high). `bibliography:` is per-document
  only (`cite/mod.rs:42`), so a growing blog retypes keys per post and nothing reports an unused or
  duplicate entry. Allow `bibliography:` in `_site.yml`, merged under each page's own; add two
  **read-only** diagnostics over the parsed registry ("entry never cited", "duplicate key"). Does
  **not** touch the BibTeX parser/CSL formatter (Do-NOT-touch): only reads parsed entries and counts
  citations. Keep "unused entry" info-level or `check`-only. *Pin: a small site with a site-level
  bib, one entry cited from two pages, one uncited.*
- **Author structure panel** (M/L, high). A read-only preview sidebar: the heading tree with
  per-section word count (the dev panel already counts, `client.js:50-58`) and a badge per node for
  unresolved xref / TODO / over-goal length. Click to scroll; under the companion, move the editor
  cursor via the existing cursor sync. This is the *revision* view, not the reader TOC. Scope it as
  an annotation layer on the dev panel, not a new component. *Pin: `corpus/layout/structure.tmd`.*
- **Session revision digest** (M, med). Surface the `BlockOp` stream the client already receives: a
  session word delta (`+340 / -180`) plus a feed of the last N ops, each click-to-source. Cashes the
  diff moat; no batch compiler has a diff to show. Honest caveat: the pin is behavioral (a
  `tools/live-edit-bench` assertion), not a corpus doc.
- **Block-level transclusion** `{{< include file.tmd#sec-id >}}` (M, med). Reuse a section across a
  series without copy-paste drift. Must ride **on top of** the `includes.rs` source-map pass
  (resolve the fragment to a block range, hand the existing machinery a sub-slice), never rewrite
  it. Hard merge gate: the source map must not perturb. Defer until a real series needs it.
- **LSP for the language intelligence, browser stays the view** (L). Everything an LSP needs is
  already in Rust (`check`, `vocab`, `register_xref`, the bib parser, `closest()`), it is write-once
  for Neovim/Helix/Zed/VS Code, and it removes the drift that causes the `#| label:` completion gap.
  An LSP cannot render the preview and does not need to: the preview is already editor-agnostic
  (the sync surface is two `postMessage` shapes in `docs/internals/protocol.tmd:325-350`). The only
  thing binding it to VS Code is the hardcoded `vscode://` open scheme. Do **not** rebuild the
  preview as an LSP; do not invest further in the webview beyond what shipped.
- **Built-site shared asset bundle** (L, high, reader-facing; owner-gated below). Measured on the
  built `corpus/tech-blog`: largest post 1.72 MB; on `KL-divergence` (712 KB) the inlined `<style>`
  is 64% of the page, of which **339 KB is base64 KaTeX woff2**, and **seven pages carry that
  identical font block.** Inlining is correct for `build file.tmd` (portable, `file://`); for `build
  <dir>` a returning reader re-downloads ~97% of every page. Extract to content-hashed
  `app.<hash>.css` / `app.<hash>.js` / `katex.<hash>.css`, linked once, minify while there.
- **Image optimization** (WebP/AVIF + `srcset` + lazy-load behind a content-hashed cache) — until
  posts get image-heavy.
- **Marketing site** (deferred, feature-first; rolls into a demo-machine rebuild):
  `live-edit-hero-demo` clip; swap `site/_site.yml` placeholders; demo-led hero rebuild (during
  which: a 3-viewport spot-check of the already-code-fixed 390px hero overflow + theme/video desync,
  plus any leftover em dashes); mobile embed refine; deploy (Cloudflare / GitHub Pages).
- **`serde_yaml` fallback watch-item:** if 0.9 ever breaks against a future serde/edition, swap to
  `serde_yaml_ng` (v0.10), gated on a test that `Error::location().line()` still works. Fix the
  stale `Cargo.toml` comment (it names the unsound `serde_yml`) when touched.

## Owner-gated: do NOT build without your ruling

- **Draft-aware preview (flips an established default).** `draft: true` currently hides a page from
  the site *preview* as well as the build (`site/discovery.rs:19-23`), so a half-written post cannot
  be seen among its own listings, nav and cross-refs until it is un-drafted — exactly when the author
  wants to see it. Proposed better default: **preview includes drafts** (quiet DRAFT badge, count in
  the dev menu), **build/publish exclude them** and print `2 drafts not published: …`. The gate: it
  flips a default and widens a discovery code path. Related, cheap either way: `book_pages`
  (`book.rs:172`) never reads `fm.draft`, so a book chapter cannot be a draft at all.
- **Reading time in the built page (reverses a deliberate decision).** Word count + reading time are
  computed but trapped in the author-facing dev panel (`client.js:50-58`), and `corpus.rs:530-533`
  **pins their absence** from the built page on purpose. Promoting them is a reader-facing flip of
  that ruling, not a bug fix.
- **`taliesin publish --public`** (relaxes the fail-closed passcode gate). `cmd_publish`
  unconditionally calls `inject_gate` (`publish.rs:194`) and `_middleware.js:9` fails closed (503
  when `PASSWORD` is unset), so a **public** blog cannot use `publish` at all — which is why the real
  blog still deploys via a side-channel `deploy` skill instead of the command built for it.
- **Plain `publish` strict by default.** `publish --strict` now inherits the full check superset
  (verified: it refuses a site with a missing image). Whether plain `publish` should be strict is a
  fail-closed default change, filed rather than assumed. Same shape: `publish` without `--strict`
  still deploys a site `check` would reject.
- **Built-site shared asset bundle** (changes the shape of the build output). See Tier 3.
- **Scroll-vs-shrink for embedded media (UI-audit #7 + #12), CONFIRMED but a design choice, not a
  defect.** `base.css:365-367` states the intent: embedded media (`canvas, svg, video, iframe`) is
  clamped to the page width so a fixed-size canvas can't force a horizontal scroll on mobile. The
  scroll-not-shrink treatment (`overflow-x: auto` + scroll shadow) is deliberately reserved for
  *text* (`pre`, `table`, `.katex-display`). Consequence: a wide **mermaid diagram** (#7) shrinks its
  `foreignObject` labels to ~5.8px at 390px; the **features.html demo video** (#12) downscales
  baked-in desktop text ~3x. The ruling: *is a mermaid diagram text-you-must-read (→ table
  treatment) or embedded media (→ keep the clamp)?* Flipping #7 costs `pre.mermaid { overflow-x:
  auto }` + `pre.mermaid svg { max-width: none }` and trades illegible-shrink for the mobile h-scroll
  the rule exists to prevent. #12 has no engine fix (re-record the clip, or ship a mobile source);
  also already deferred under the marketing item.
- **Add (gate: adopt, but confirm):** shareable/deep-linkable `{{< input >}}` state via the URL
  fragment (reader-local, hydrate from `data-qmd-input`, no Rust/model change) (`qmd-js.js`); reader
  text-size + line-spacing controls (a11y-exempt per CLAUDE.md; substrate exists)
  (`14-reader-prefs.js`).
- **Add (deferred, need a scope/default ruling):** cross-revision block-diff "what changed" view;
  reader-facing reproducibility manifest; web-native List of Figures/Tables/Theorems; interactive
  data tables; "Cite this" export; code-line xrefs (`@lst-3:line`); theme-aware `dark=` figures.
- **TODO / FIXME surfacing needs a severity concept that does not exist.** `prose.rs::lint` returns
  markdown-aware, code/math-skipping located `(line, message)` pairs, so the *scan* is small. The
  trap: **there is no severity anywhere.** `render::Warning`, `check::Diagnostic` and
  `protocol::Diagnostic` know only `warning|error`, and the warning channel is a **hard gate**
  (`cmd_check` exits non-zero on any diagnostic; `build --strict` and `publish --strict` inherit it).
  So a TODO warning that reaches the shared gate fails `check` on every draft. Two designs:
  - **A (S, safe):** preview-only. A `todo_scan` injected at `serve/mod.rs::compute_diagnostics`
    through a new `protocol::Diagnostic::info` + a `.tali-diag-info` rule; cannot reach the shared
    gate by construction. TODOs appear in the browser preview, not as VS Code squiggles.
  - **B (L):** a real `level` threaded through `render::Warning` + `check::Diagnostic` +
    `format_json` + the `cmd_check` exit gate + **both** `build --strict` tallies + the hardcoded
    `DiagnosticSeverity.Warning` in `diagnostics.ts:59`. Re-plumbs the very gate Batch B just
    unified; its own session.
  The scan must **not** reuse `prose::strip_inline` (it blanks code, and a TODO usually lives in a
  code comment). Pin any fixture inside `corpus/diagnostics/`. *Owner ruled 2026-07-10: skip for now;
  analysis kept.*

## Decided against / do-not-re-litigate

**Refuted by measurement — do NOT re-scope:**
- **"`build` leaks forkserver subtrees."** FALSE. Snapshot→build→wait→snapshot, run twice, once with
  `TALIESIN_NO_CACHE=1` forcing real execution: zero new survivors. The 2026-07-08 process-group
  reaping fix holds on the graceful path. The real gap is the *ungraceful* path (Tier 2).
- **"The warm pool boots Python on prose-only builds, costing latency."** The boot is real (even
  under `TALIESIN_NO_EXEC=1`) but the latency claim is false: 0.25 s vs 0.27 s. Resource-hygiene, not
  perf.
- **"Dev attributes bloat published pages."** FALSE: `data-block-id` + `data-sourcepos` +
  `data-source-file` + `data-qmd-src` total 2104 bytes on a 712 KB page = 0.29%. Do not propose
  stripping them for size.
- **`--version -dirty` marker: NOT worth building.** `build.rs` declares `cargo:rerun-if-changed`, so
  cargo runs it once and never again when the tree becomes dirty (proved with a side-effect log). A
  marker computed there is stale, i.e. worse than absent. The `rerun-if-changed=<nonexistent>` escape
  forces a rerun every build but costs 0.85 s per warm build, and the launcher rebuilds every
  invocation. Refused.
- **`CLAUDE.md`'s stale-asset warning is imprecise for CSS.** Cargo tracks `assets/css/base.css` in
  dep-info; a marker appended to it did appear in the freshly built binary. Any claim that `cargo
  build` silently embeds stale `assets/css/` was not reproducible. Re-verify for `assets/js/` before
  repeating the touch-render workaround.
- **Already fixed in code — do NOT re-open as bugs:** the 390px `page-layout: full` + `hero:` prose
  overflow (`site.css` `.tali-site-main { box-sizing: border-box }`) and the theme/video desync
  (`theme.rs` `syncThemeVideos()` on `qmd:themechange`). Only residue is a 3-viewport spot-check,
  folded into the deferred Marketing rebuild.
- **Include symlink-loop SIGABRT does not exist.** Linux caps symlink traversal at `MAXSYMLINKS = 40`
  (`ELOOP` at depth 41), and `includes.rs:148` already `drop_with_warning`s on `Err(_)`. Its
  co-listed "lexical-only `safe_join`" is likewise a non-issue: includes are author-local, never
  attacker-controlled. A `MAX_INCLUDE_DEPTH` cap would be harmless defense-in-depth, not a defect.
- **Perf refutations:** `app.pages` is bounded by the site's finite page count (holds only
  lightweight `PageState`); the heavy resource (warm kernels) is already an LRU capped at
  `MAX_WARM_PAGES = 6`. A "lazy discover-time search index" buys nothing (15.6-27.1 ms one-time vs a
  0.38-0.64 s warm build) and would regress the Batch-8 live-search refresh.

**Gate the gate:** a drift test that cannot fail is worse than none. Two of the three Batch-F drift
gates could not fail on their first draft (a line-based Rust gate missed a rustfmt-wrapped match arm;
a TS gate's `spawn(\s*\w+\s*,` regex skipped `spawn(binaryPath(), …)`). Any new drift gate must be
mutation-checked against exactly the shape it guards.

**Library outsourcing — decided against** (each adversarially verified vs the invariants):
hayagriva/biblatex (heavy deps, only IEEE used); schemars (reopens schema↔validator drift);
jsonschema (loses source-line diagnostics); morphdom/idiomorph (reverse the 83x live-edit payload
win); similar/dissimilar (give up the block-id→LIS reduction); clap; owo-colors; slug (transliterates
non-ASCII → breaks anchors); html-escape (breaks the anti-double-escape contract); lightningcss/
palette (CSS uses native `color-mix`); IntersectionObserver/scrollspy libs; deck micro-helpers (force
an offline bundle onto every deck). The reader menu is intentionally an untrapped popover. Also: keep
`two_face` extras filling gaps only — the bundled syntect set is consulted first and must win, because
`two_face::syntax::extra_newlines()` is bat's own curated set (different scope spans), NOT a superset.

**Reading-first defaults — research-validated keeps** (do NOT "fix"): serif body for long-form screen
reading (don't switch to sans); ~70ch measure `--tali-maxw: 46rem` (don't narrow); right-rail
scrollspy + width-gated sidenotes (keep both); scroll (not pagination) book reading; system-font-only
(if a serif webfont is ever bundled, ship REAL bold/italic faces, never synthesized). *Caveat:* the
competitor framing (Stripe/Linear/Mintlify/Docusaurus/GitBook) is unverified judgment, not evidence.

**2026-07-06 session decisions:** book pager stays **bottom-only** (a top pager fights the calm
column; the Chapters drawer gives random access). Book page-TOC: **fix in place, keep both nav
surfaces** — do NOT fold the rail into the chapter drawer (loses the always-visible scrollspy). Xref
graph tool: **removed** (interaction not good enough). Focus mode stays **ephemeral** (no persistence
across chapters): `requestFullscreen()` needs a user gesture, so persistence could only restore CSS
chrome-hiding and would silently drop fullscreen on nav. Deck overview **keeps per-slide backgrounds**
(documented recognizability "fingerprint", no contrast bug today). Dev-menu + `#tali-progress` +
reading-progress bar stay **three separate signals** (author diagnostics / build-exec status / reader
scroll-position); `#tali-progress` is the exec chip, NOT a reading-progress chip.

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Open-source the repo + publish the site when
ready; the GitHub/install CTAs become real then. The security token gate is shipped.
