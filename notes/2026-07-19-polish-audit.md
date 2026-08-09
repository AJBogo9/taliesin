# Polish audit (2026-07-19)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

A **feature-polish** audit: for every *existing* feature, "could it be simplified with the
same power to the user, or is it implemented in an unintuitive way?" — the lens none of the
prior audits used (DX / PMF / machine-facing / vacuous-test / website-design / deck were
about discoverability, product fit, machine-readable correctness, test rigour, brand, and
the deck respectively). Goal: make the tool feel **extremely mature and well-thought-out**.

## Provenance / method

- **4 read-only auditors**, one per surface (CLI/DX · authoring · live view · theming), each
  with online research on how mature tools (rustc, cargo, gh, deno, vite, hugo, astro, quarto,
  MyST, Typst, Reveal/Slidev, iA/Bear, Tufte-CSS, VitePress) implement the same features.
- **Per-finding source verification** by the orchestrator: every headline item was re-checked
  against current source before filing (the backlog rots; trust code, not claims). Items marked
  **VERIFIED** below were read to the line by the orchestrator; others were auditor-cited with a
  `file:line` and are high-confidence but not independently line-checked — **grep the named
  symbol before promising anything** (this file's standing law).
- **Live browser verification was blocked**: the chrome-devtools MCP profile was held by another
  running instance, so the deck/reader interaction items were confirmed from source only. A
  couple of interaction-timing items (PL13 deck theme, the cold-open hint) are worth a live
  chrome-devtools pass at implementation time.
- No code changed — report-only by request. No SHAs.

## The three patterns (the actual finding)

The tool's own best disciplines each have a small hole:

1. **Silent holes in an otherwise fully-diagnosed surface.** Almost everything is validated +
   located with did-you-mean, yet a few features offer co-equal spellings with no canonical, or
   *silently ignore/drop* input the tool's own vocabulary invites. This is the highest-value
   cluster because it contradicts the tool's strongest quality.
2. **The design system is tokenized for colour but not geometry/motion, and the "one owned
   palette" is physically duplicated** across four files/languages with no drift-lock.
3. **Human-facing surfaces under-sell machinery that already ships** (the `--explain` catalog
   is invisible in human output; the deck's theme model is poorer than the page's).

Everything below stays inside every invariant: no block-model change, no preview write-back, no
new output format, no CDN, frozen `qmd-theme`/`qmd:themechange` names untouched.

---

## Pattern 1 — silent holes / co-equal spellings

### PL1 — Surface the diagnostic code + severity + `--explain` in `check`'s human output. VERIFIED
`check` computes a stable `TAL-*` code and severity per diagnostic and ships a full rustc-grade
`--explain` catalog with drift-locked `docs/DIAGNOSTICS.md` — then the human formatter throws it
all away: `format_human` (`crates/server/src/check.rs:374-383`) prints only `file:line: message`,
and the summary is a bare `N problem(s)` (`:575`). The whole DX6 investment is invisible to the
99% of runs that read human output. **Fix:** prefix each line `severity[CODE]`, split the summary
`(1 error, 2 warnings)`, and print a footer pointing at `taliesin check --explain <CODE>` (rustc's
"For more information, try `--explain`"). Output-only; `--format json` untouched. **S · high · [surface].**

### PL2 — Make the `{{< input >}}` reactive control coherent (found independently by two auditors). VERIFIED
The reactive input control is the one interactive feature that is a `{{< >}}` shortcode instead of a
`:::` div, so an author who reaches for `::: {.input name="k"}` (matching `.scrolly` / `.panel-tabset`
/ `//| viewof`) gets **silently nothing** — an empty div is dropped at `render/divs.rs:295-298` — while
the validator's own error text calls it `.input` (`render/validate.rs:202`: "`.input` needs a `name=`"),
a syntax that does not exist as a div. On the CSS side a leftover legacy `.tali-input` block
(`assets/css/base.css:918-925`, same specificity, later source order) overrides the shortcode's intended
`.tali-input` at `:201` and, because `.tali-input-label` (`:203`) sets only `font-weight`, mutes the
shortcode's label to `--tali-muted`. **Fix:** (a) emit a located warning when an empty div carries a known
feature class (`.input`, `.callout-*`, `.panel-tabset`, …) instead of dropping it silently; (b) delete the
dead/colliding legacy `.tali-input` rules (moving the still-live `.tali-js-error` block out first). **S · high.**

### PL3 — Unify column layout; stop silently discarding `.column width=`. VERIFIED
Three spellings make a column grid — `::: {layout-ncol=3}` (the *only* structural feature dispatched on a
bare `key=value` attribute), `::: {.columns}`, and `.column` children (`render/divs.rs:450-468`) — and the
`.columns` arm's own comment admits "their widths are ignored (equal columns)", so a reveal/Quarto author's
`::: {.column width="70%"}` is silently equalized with no warning. `.columns` is explicitly marketed as
"reveal muscle memory", which is exactly who brings the `width=` habit. **Fix:** bless `.columns` (dot-consistent)
as canonical with an optional `ncol=`, keep `layout-ncol` as a silent alias, and either honour `width=` on
`.column` children or emit a located warning. **S–M · high · [author].**

### PL7 — Line-highlight uses two names + two delimiter grammars; a deck habit silently no-ops. VERIFIED
Highlighting code lines is `code-line-numbers="1|2-3"` (pipe-delimited steps) on decks/listings
(`render/emit.rs:234`) but `::: {.step lines="6-8"}` split on `,` only in the walkthrough enhancer
(`assets/js/walkthrough.js:23`). A deck-trained author who writes `::: {.step lines="1|2-3"}` gets **zero**
highlighted lines (the whole spec matches neither the range regex nor `^\d+$`), silently. **Fix:** at minimum
warn when a `.step lines=` value contains `|`; ideally align the grammars / alias the attribute name. **S · med-high.**

### PL9 — Deck fragment-effect classes escape the validated vocabulary. VERIFIED
`.fade-out` and `.highlight` are real styled fragment effects (`assets/css/deck.css:299-336`) but neither is
in `DIV_FEATURE_CLASSES` (`render/validate.rs:59-75`), which powers the div-class did-you-mean. So a typo in
the *effect* modifier (`::: {.fragment .fade-ot}` / `.hihglight`) renders a plain fragment with no diagnostic —
the one incomplete spot in an otherwise-complete vocabulary, and it's exactly a deck author's fiddly modifier.
**Fix:** add `fade-out`, `highlight` (audit `deck.css` for any others) to `DIV_FEATURE_CLASSES` + update the
`vocab.rs` subset test. **Trivial · med.**

### PL10 — A `{js}` runtime error ships a raw stack trace to readers in *built* output. VERIFIED
On any cell throw, `assets/js/qmd-js.js:212` sets `pre.textContent = String((e && e.stack) || e)` (and already
`console.error`s the same at `:209`). Since `{js}` cells execute in the reader's browser in *built* HTML, a
runtime throw on a published page shows the reader `TypeError… at <anonymous>:3:5`. Right for the authoring
preview, a dev-internals leak in production. **Fix:** in the build (non-preview) path degrade to a terse themed
"this interactive element couldn't load", keep the full stack in `console.error`; preview keeps the full box.
A preview-vs-build client signal already exists (the gate that decides `taliRestartKernel`/`taliOpenPageSource`
presence). **S · med.**

---

## Pattern 2 — asymmetric / duplicated design system

### PL4 — Single-source the owned palette across four files; drift-lock the OG card + deck. VERIFIED
The brand story is "ONE owned hue, exact hexes banned by a test" (`base.css:2-8`), but the palette is re-typed
in **four** places in three languages: light `:root` tokens (`base.css:9-13`), dark tokens (`dark.css:5-11`), a
parallel `--deck-*` namespace re-declaring the same literals (`deck.css:696-704`), and the OG-card generator's
hardcoded Rust consts (`site/card.rs:20-24`, the dark family). The deck bundle is `{FONTS_CSS}{DECK_CSS}` with no
base.css (`render/deck.rs:75`) and card.rs is a build-time PNG, so the copies are structurally forced — but
nothing keeps them in sync, and the "banned hexes" test guards old *vendor* blues, not card/deck drifting from
the tokens. **Fix (the project's own anti-drift idiom):** (a) extract the token `:root`/`[data-theme]` blocks into
a shared `TOKENS_CSS` const `include_str!`'d into both bundles, renaming `--deck-*` → the matching `--tali-*`
(deck keeps `--deck-w/-h`); (b) add a `#[cfg(test)]` drift-lock (à la `schema.rs`/`third_party.rs`) asserting
`card.rs`'s `BG/FG/ACCENT/MUTED/BORDER` consts match the dark tokens. **M · high.**

### PL11 — Colour is exhaustively tokenized; geometry/motion are ad-hoc literals. VERIFIED (grep-derived)
The system tokenizes colour, fonts, `--tali-maxw`, and the focus ring, but has **zero** tokens for roundness,
elevation, or motion: 13 distinct `border-radius` literals (2–16px + 50% + 999px), 25 hand-written `box-shadow`s,
and 6 transition durations (`.12s` alone repeated 23×). For a single-design-language thesis, this is the
asymmetry that reads "unfinished" next to Primer/VitePress/Astro. **Fix:** `--tali-radius-sm/md/lg` (~3 buckets,
keep 999px pills + 50% circles as intentional specials), `--tali-shadow-sm/md/lg` (offset+blur + the existing
`--tali-edge-shadow` colour), `--tali-dur:.12s` / `--tali-dur-slow:.25s`; migrate mechanically. Don't
over-tokenize spacing. **M · med.**

### PL12 — Exec/error boxes opt out of tokens, which forces the whole-doc print theme-swap. VERIFIED (cited)
Callouts/theorems are fully tokenized (light/dark/sepia derive from one `color-mix` definition), but
`.tali-stderr`/`.tali-error`/`.tali-js-error` take the callout token for the border and then **hardcode**
background + text as per-theme literals (`base.css:681-693`, `:931`, `dark.css:32-35`). That untokenized surface
is *why* the pre-paint script has to force the whole document to the light theme when printing
(`render/theme.rs:144-157`) — the token print-reset can't reach literals. **Fix:** derive the box surfaces via
`color-mix(in srgb, var(--tali-callout-warning) 12%, var(--tali-bg))` (stderr) / `--tali-callout-important`
(error/js-error), drop the ~6 per-theme override rules, and shrink the print swap. (Syntax-scope colours still
justify their own per-theme palette.) **S · med.**

### PL13 — Deck theme is a permanent binary toggle with no "Auto / follow system". VERIFIED
A standalone deck's stored toggle wins in `resolve()` **forever** with no clear-path (`render/deck.rs:165-175`;
`taliDeckSetTheme` persists to `localStorage['qmd-deck-theme']`), while a page reader gets Auto/Light/Dark/Sepia
where "Auto" clears the key and resumes OS-follow (`assets/js/code-enhance/14-reader-prefs.js:12`). Tap "Dark mode"
once on a shared deck at night and it's opted out of daylight light mode with no visible undo. (Scope: standalone
decks only — an *embedded* deck correctly follows the host at `deck.rs:167`.) **Fix:** replace the binary item with
a 3-state Auto/Light/Dark segment mirroring the page; "Auto" clears the key. **S · med.** *(worth a live check.)*

---

## Pattern 3 — CLI/config consistency & under-sold machinery

### PL5 — Unify `--json` vs `--format json` across the subcommand family. VERIFIED (flag lists)
`init`/`new` take a boolean `--json`; `build`/`publish`/`check`/`doctor`/`map`/`symbols` take
`--format human|json`; they don't cross-accept, and the did-you-mean doesn't bridge them (`new … --format json`
dead-ends with no suggestion — `--format` is >2 edits from `new`'s flags). **Fix:** accept `--json` as an alias
for `--format json` everywhere (the clig.dev shorthand) and add both to each command's flag-candidate list so the
wrong one suggests the right one. **S · med-high · [surface].**

### PL6 — Route kernel failures to `taliesin doctor`; stop blaming the interpreter path. VERIFIED
The kernel-unavailable diagnostic (`crates/server/src/exec.rs:328-329`) frames every failure as a wrong
interpreter path ("fix the interpreter (TALIESIN_PYTHON or _site.yml python:)…"), but the usual cause is a
missing `ipykernel`/`IRkernel` package on a fine interpreter — exactly what the dedicated `doctor` reports.
Nothing routes there. **Fix:** append "Run `taliesin doctor` to see whether it's the interpreter or a missing
kernel package", and soften "fix the interpreter" to "…or install its Jupyter kernel package." **S · med.**

### PL14 — `check`'s Environment footer spawns interpreters + prints an always-green block. VERIFIED
`collect_environment(target)` runs unconditionally (`check.rs:562`) and prints an Environment footer whenever
non-empty (`:581`) — even when all-green — on a command documented "does NOT execute code cells", spawning
`python3`/`R` on every keystroke/CI run and duplicating `doctor`. **Fix:** in human mode print the block only
when a used language is degraded (`!runs || !kernel_pkg_ok`), tail it with "run `taliesin doctor`", and consider
skipping the probe entirely unless a diagnostic or `--require-kernel` needs it. Keep JSON `environment` always-on
(agents want the full probe). **S–M · med.**

### PL15 — Document `new --draft`/`--tour`; replace the drift-prone `usage:` one-liners. VERIFIED
`new` parses `--draft`/`--tour` (`cli.rs:495-497`, `NEW_FLAGS`), and the `init` scaffold *advertises* `--draft`
(`cli.rs:31`), but both help surfaces list only `[--dir] [--json]` (`main.rs:152`, `:348`). Separately, the
hand-maintained one-line `usage:` strings have already drifted — `build`'s (`build.rs:160`) omits `--format json`
that `subcommand_help("build")` documents. **Fix:** add `[--draft]`/`[--tour]` to `new`'s help; on a missing
positional print `subcommand_help(cmd)` (one source of truth) instead of the parallel one-liner. **Trivial–S · med.**

### PL16 — Group the 16-command `help` by purpose. VERIFIED
`usage()` (`main.rs:149-208`) prints one flat block of 16 commands, mixing the everyday three with ten an
author rarely types. git/cargo/gh group by purpose; clig.dev endorses it. **Fix:** section `COMMANDS:` into
**Author** (init, new) · **Preview & build** (preview, build, publish) · **Inspect** (check, doctor, map, read,
render, blocks, symbols) · **Editor & agent** (schema, vocab, mcp). Pure formatting. **S · med.**

### PL18 — One `--format` error wording/style; resolve the hidden `--out`/`--dir` aliasing. VERIFIED (cited)
The same bad-`--format` error has two wordings and two output paths (`log::error` styled vs bare `eprintln!`) across
`check`/`map`/`symbols` (`check.rs:511`) vs `doctor`/`publish` (`doctor.rs:239`, `publish.rs:68`). And each
dir-taking command secretly accepts the other of `--out`/`--dir` while documenting only one (`new` parses
`--dir|--out`; `schema`/`publish` parse `--out|--dir`). **Fix:** one shared `bad_format_error` helper; pick one
name per semantic (`--dir` = scaffold input root, `--out` = output dir) and document it. **Trivial · low-med.**

---

## Pattern 4 — container-feature coherence (authoring)

### PL17 — Callouts adopt a leading heading as title; theorems ignore it. VERIFIED
A callout's title comes from `title="…"` *else a leading heading* else the kind (`render/divs.rs:411-423`); a
theorem's title comes from `title="…"` **only** (`:650-656`), so a theorem that leads with a heading renders it
as body. Same gesture (lead with a heading to name the box), two outcomes. **Fix:** make theorems adopt a leading
heading as the parenthetical title (reuse the callout hoist), or emit a located "did you mean `title=`?" warning. **S · med.**

### PL19 — Name a canonical margin-note; keep the three aliases labelled. VERIFIED
`.sidenote` / `.marginnote` / `.column-margin` / `.aside` are four co-equal classes for one identical CSS
(`base.css:655`, all four in `DIV_FEATURE_CLASSES`); the corpus only ever uses `.column-margin`. Four names for
one thing is surface to scan past. **Fix (docs-only):** name `.column-margin` canonical in the guide, keep the
other three as explicitly-labelled aliases (Quarto/Tufte compat). **Trivial · low-med.**

---

## Small maturity adds (bundle as one pass)

- **PL8 — Add `<meta name="theme-color">` (dynamic) + `<meta name="generator">`.** The head (`render/page.rs:269-271`)
  has neither; mobile browser chrome stays white against a dark page. The pre-paint script already holds the `BG`
  map (`render/theme.rs:103`) to feed a dynamic `theme-color`; the Atom feed already advertises a generator
  (`site/feed.rs:158`). **S/trivial · med.** VERIFIED.
- **PL20 — Deck/reader micro-polish (each trivial, ship together):** cold-opened stepped deck hides all nav after
  3 s (`deck.js:1932`, `deck.css:603`) with no first-run hint — delay the idle-hide on cold open and/or show a
  one-time hint reusing the overview-hint styling (*worth a live check*); reduced-motion isn't honoured on the
  deck's programmatic slide-jumps (`deck.js:1404`, one-line `!reducedMotion()` thread); the deck key-sheet
  (`deck.js:1679`) omits Home/End + `0`; the "minor-third modular scale" comment (`base.css:351`) doesn't match the
  actual drifting hand-tuned ratios (soften the comment or compute from a real `--tali-scale`); `og:type` is
  hardcoded `"article"` for every standalone-doc build (`render/mod.rs:880`) even a non-article page.

---

## Design questions (owner ruling first — not build-ready)

- **Deck inverts the page's serif/sans logic** (page: serif body / sans heads; deck: serif heads / sans body,
  `deck.css:705-711`). Both individually defensible; crossing an embedded deck ↔ article switches heading voice.
  Accept-and-document, or unify?
- **Focus/reading mode is welded to OS fullscreen** (`03-focus-mode.js:39-45`): pressing `f` both hides chrome and
  requests fullscreen, and leaving fullscreen by any route drops focus mode. The code comments this as "the
  author's ask", so it's a design call: decouple the calm column from commandeering the screen (iA/Bear dim without
  fullscreen), or keep the coupling?
- **`//| input:` (consume) vs `{{< input >}}` (produce)** — one word, opposite data-flow roles. Add `//| uses:`
  as the documented alias for the consumer? (Adds vocabulary; Observable users know `viewof` — weigh before adding.)
- **Callout kinds are namespaced (`.callout-note`) but theorem kinds are bare (`.theorem`)** — the "prefix the kind
  with its family" rule holds for one family only, and the bare theorem kinds are the less-diagnosable (a far miss
  gets no did-you-mean). Document the two conventions, or reconsider?

---

## What's already excellent (credit — this is a maturity audit)

- **Diagnostics infra:** the `--explain` catalog + computed `docs_url` + generated `DIAGNOSTICS.md` is rustc-grade
  (drift-locked, offline, case-insensitive, did-you-mean on bad codes); the only gap is PL1, *surfacing* it. Did-you-mean
  spans commands, flags, `new` kinds, `--port` typos, front-matter keys, and codes. A single `page_static_diagnostics`
  keeps `check`/`build --strict`/`publish` from drifting.
- **Config discipline** already honours "perfect the default before a knob" harder than most SSGs: site `image:`
  removed-and-warns, `csl:` recognized-but-warned, `page-layout` honours only `full`, `toc:` auto-gates above 3
  headings, `python:`/`r:` auto-detect `.venv`. Very few naked knobs survive.
- **The colour system** (one owned OKLCH accent, per-theme documented contrast ratios, `forced-colors`/`prefers-contrast`/
  `prefers-reduced-motion`, `color-mix` tints) and the pre-paint no-flash theme script are well above typical.
- **The deck engine** (single-camera model, FLIP auto-animate with a generation-guarded settle, magic-move line-glide,
  offline QR share of the exact deep-linked view, live-region a11y, `inert` off-camera slides) and the **reader
  enhancers** (link-preview, focus-trapped lightbox, Cmd-K ARIA-1.2 combobox palette, mobile TOC sheet, reactive `{js}`
  teardown discipline) are above Reveal/Slidev/typical-reader baseline.
- **OG/SEO/JSON-LD completeness** (per-page branded card, canonical, Twitter, `BlogPosting`/`ScholarlyArticle`/`WebSite`+`Person`
  JSON-LD, Highwire `citation_*`, normalized `<lastmod>` sitemap, robots.txt, Atom autodiscovery) exceeds most SSGs' defaults.

## Suggested grind order

**Pattern-1 silent holes first** (PL1, PL2, PL3, PL7, PL9 — each small, high-confidence, and each closes a place the
tool's own diagnosis discipline fails silently, the biggest lever on "feels well-thought-out"). Then the **design-system
single-sourcing** as one CSS-token pass (PL4, PL11, PL12, PL8). Then the **CLI consistency** sweep (PL5, PL6, PL14, PL15,
PL16, PL18). PL10/PL13/PL17/PL19/PL20 fold in opportunistically. Each fits branch → spec → corpus-pin → browser-verify.
