# Shed Quarto — design

**Date:** 2026-07-03
**Status:** approved (design), pending implementation plan
**Branch:** `shed-quarto`
**Relates to:** `2026-06-27-taliesin-rename-design.md` (the executed rename, whose §7
"deliberate keeps" this spec deliberately *reopens*), memory `quarto-separation-direction`.

## 1. Goal

Make Taliesin a fully separate tool from Quarto by closing the compatibility windows the
rename left open. This is the confirmed **"separate tool, keep Markdown"** direction: shed
Quarto's *identity and compat coupling*, while keeping all Markdown/Pandoc syntax
(`:::` divs, `#|`/`//|`/`%%|` cell options, `@fig-`/`@sec-` xrefs, `[@cite]` citations,
`{{< shortcode >}}`, YAML front matter). That syntax is **Pandoc/Markdown lineage, not Quarto
branding** — it stays, and is the near-zero-learning-cost north star.

### Owner decisions (2026-07-03)

1. **`.qmd` input → full drop, `.tmd`-only.** Migrate the whole corpus + docs to `.tmd`,
   remove `.qmd` acceptance entirely, and drop the `format: revealjs` + `ojs_define()`
   aliases. One clean breaking release. Any `.qmd` file (including external books) must be
   renamed to build.
2. **Migration on-ramps → remove all (purist).** Drop the `_quarto.yml` migration breadcrumb,
   the `.quarto` watcher-skip, and the "Coming from Quarto" doc framing. Taliesin makes no
   mention of Quarto in anything a user runs or reads.
3. **Removal boundary → user-facing surfaces only.** Purge Quarto from CLI output/help,
   warning messages, README, the docs *guide* prose, and the `.tmd` corpus. **Keep** it where
   it is factually accurate and non-identity: `THIRD_PARTY.md`/licensing, the `tech_blog` test
   asserting "no quarto-ojs-runtime", internal code comments, test function names, and dated
   historical records (`notes/DROP-QUARTO.md`, `notes/BEYOND-QUARTO.md`, the planning specs,
   `CLAUDE.md`).

## 2. Target end-state

- Taliesin accepts **`.tmd` only**. `.qmd` is not a source extension.
- `format: deck` (and `<name>-deck`) is the only deck spelling; `revealjs`/`*-revealjs` is not.
- `define()` is the only reactive-publish global; `ojs_define()` is gone.
- A `_quarto.yml`-only directory gets Taliesin's generic onboarding, with no Quarto breadcrumb.
- `.quarto` is not in the file-watcher ignore list.
- No CLI output, warning, README line, or docs-*guide* page names Quarto.
- The corpus + docs are entirely `.tmd`; every intra-site link is `.tmd`.
- Markdown/Pandoc syntax is untouched.

## 3. Change areas (precise locations)

Verified against current `main` (post `f5239cb`, which already removed the third-party
`_extensions` loading system — so `extension_ref` and the `-revealjs` *extension-suffix*
acceptance are already gone; only the *format-name* `-revealjs` in §3.B survives).

### A. `.qmd` input

- `crates/core/src/ext.rs:13` — `ACCEPTED_SOURCE_EXTS: &[&str] = &["tmd", "qmd"]` → `&["tmd"]`.
- Reword the module doc comment (drops the "`.qmd` (the Quarto spelling) stays accepted"
  paragraph) to describe `.tmd` as the sole source extension.
- Update the `accepts_both_spellings_rejects_others` unit test → `.tmd`-only (rename it).
- There is **no** `warn_if_deprecated_source_ext` in the code today (`.qmd` is silently
  accepted), so there is nothing to delete on the nudge path — dropping the const entry is the
  whole behavioral flip. Every downstream consumer (page walker, `check` walker, link
  rewriting, book chapter naming, deck/embed href mapping) routes through `ext.rs`, so the flip
  propagates from this one spot.

### B. `format: revealjs` / `*-revealjs` alias

- `crates/core/src/render/fm_extract.rs:94` — `is_deck_format_name`: drop `n == "revealjs"`
  and `n.ends_with("-revealjs")`; keep `n == "deck"` / `n.ends_with("-deck")`.
- The inline `format: revealjs` scan (`fm_extract.rs` ~L60-71) that flips an HTML doc to a deck
  — drop the `revealjs` spellings there too.
- Update the internal doc comments at `fm_extract.rs:47,62,71,90` and `model.rs:118` that
  describe the `*-revealjs` form (accuracy, not purge — after the change the spelling is gone).
- Update the ~25 `crates/core/src/render/tests.rs` unit tests that build decks with
  `format: revealjs` → `format: deck`. Add/keep one test pinning that `format: revealjs` is
  **no longer** recognized as a deck (renders as a normal page + validator warns, see §4).

### C. `ojs_define()` alias

- `crates/server/src/kernel.rs:68` — delete `globals()["ojs_define"] = define`. Line 67
  (`globals()["define"] = define`) stays: `define` is the native author API.
- The `qmd-define` **script-type** wire (`kernel.rs:66`, `freeze.rs:36`, `qmd-js.js:60`) is a
  frozen internal contract (cache-bound, invisible) — **not** touched (§5).
- Internal comments that describe the bridge as "the `ojs_define` bridge" (model.rs:44,
  build.rs:260, reactive.rs, serve/serve_site) may be lightly updated to say `define` for
  accuracy; not required by the purge (they are internal).

### D. Migration on-ramps

- `crates/server/src/check.rs:178-247` — remove `quarto_migration_hint`, its call site in the
  check flow (~L241-247), the `_quarto.yml` diagnostic string, and its test. A `_quarto.yml`
  directory then falls through to the normal "no `_site.yml`" onboarding.
- `crates/server/src/serve/mod.rs:924` — remove `".quarto"` from the watcher ignore list (keep
  `.git`, `_site/`, `_book/`, editor swap files). Update the L905 doc comment.

### E. User-facing prose

- `README.md:11-12` — reframe off "focused replacement for Quarto / goals Quarto's
  architecture can't deliver" to a stand-alone identity ("a performance-oriented tool for
  authoring HTML from `.tmd` …"). Keep it honest, Quarto-free.
- `docs/guide/using/migrating-from-quarto.qmd` (18 mentions) — **delete** the page and its nav
  entry (it is the "Coming from Quarto" on-ramp; purist removal deletes it).
- Reframe the remaining **guide** prose that names Quarto: `docs/guide/index.qmd` (5),
  `docs/guide/using/formats.qmd` (2), `docs/guide/reference/cli.qmd` (2),
  `docs/guide/reference/frontmatter.qmd` (4), `docs/guide/reference/configuration.qmd` (4),
  `docs/guide/reference/cell-options.qmd` (1), `docs/guide/tour.qmd` (1).
- The 9 docs teaching `ojs_define` (guide: cli, cell-options, tour, code; internals: protocol,
  data-types, rendering, execution, validation) → teach `define()`.
- **Internals** book prose (`sites.qmd` 5, `validation.qmd` 6, `repository.qmd` 2, plus
  rendering/server/execution 1 each): case-by-case per §4 — keep accurate architectural-contrast
  mentions, drop pure identity/marketing ones.

## 4. Behavior after the drop (approved defaults)

- **`format: revealjs`** → no longer a deck. The front-matter validator emits
  *"unknown format `revealjs` — did you mean `deck`?"* using the existing did-you-mean infra
  (consistent with the clean-break stance), rather than silently rendering a plain page.
- **`ojs_define()`** → a `NameError` at runtime (the Python global is gone); docs teach
  `define()`. (No shim, no warning — clean break.)
- **`_quarto.yml`-only dir** → the generic "no `_site.yml` — run `taliesin init`" onboarding,
  no Quarto mention.
- **Unknown Quarto-only front-matter keys** → keep rejecting them (the clean-break stance +
  tests stay), but reword any Quarto-specific *message* to a generic unknown-key one; the
  validator's did-you-mean already supplies guidance.
- **Internals-book prose split** → guide/ prose fully reframed; internals/ prose keeps accurate
  architectural-contrast mentions where naming the contrast is genuinely explanatory, drops pure
  identity/marketing mentions.

## 5. Explicit keeps (the chosen boundary)

Not touched by this change:

- Internal code comments and test function names (e.g. `dropped_quarto_keys_now_warn`,
  `quarto_shaped_config_is_no_longer_parsed_and_warns`, `docs_do_not_claim_quarto_config_still_works`)
  — these *enforce* the separation; keep the tests and the stance, keep their names. This change
  does not *purge* Quarto from internal comments; incidental accuracy edits (e.g. §3.C, where an
  alias a comment describes is being removed) are fine.
- `THIRD_PARTY.md` / licensing notes and the `tech_blog` test asserting no
  `quarto-ojs-runtime` / no `window._ojs` — factually accurate provenance.
- Dated historical records: `notes/DROP-QUARTO.md`, `notes/BEYOND-QUARTO.md`, the planning
  specs under `docs/superpowers/`, and `CLAUDE.md` (developer-facing, not end-user).
- The frozen `qmd-define` script-type wire, the `qmd-js-<id>` cell target id, `data-qmd-*`
  attrs, private Python `_QMD_*` — internal, cache-bound, invisible (per the rename memory).
- Structural dir names `_site.yml` / `_freeze/` / `_extensions/` (theme dir) / `_book/`.

## 6. Migration mechanics (the riskiest area)

The bulk of the diff and where breakage hides:

- `git mv` all **60 corpus + 34 docs `.qmd` → `.tmd`** (currently 1 `.tmd` exists), plus any
  `.qmd` test fixtures under `crates/core/tests/` and the `common/mod.rs` fixture helper that
  writes `.qmd` paths. Path-based tests (corpus walker, site, `check`, includes) will otherwise
  fail once `.qmd` is unaccepted.
- **Rewrite every intra-site `.qmd` link in source to `.tmd`.** Critical: the site rewrites
  `*.qmd` → `*.html` via `strip_source_ext` (`site/mod.rs:11,421`). Once `.qmd` is no longer an
  accepted ext, a link still written as `other.qmd` stops being recognized and is left unrewritten
  → a broken link in the built site. `taliesin check` catches any miss (this is the safety net).
- Update `_site.yml` chapter lists, `{{< include x.qmd >}}` / `{{< embed deck.qmd >}}`
  shortcode targets, and README corpus paths.
- Cross-*book* links are already written as relative `.html` (per CLAUDE.md), so they need no
  rewrite — only intra-site source `.qmd` links do.
- `corpus/deck.qmd` is already `format: deck`; the `corpus.rs` walker's `_extensions`/`expected`
  skip needs no logic change, just the rename.

## 7. Phasing (recommended: one branch, corpus-first, green after each commit)

1. **Mechanical `.tmd` migration.** `git mv` corpus + docs + test fixtures; rewrite internal
   `.qmd`→`.tmd` links; update `_site.yml`/includes/embeds/README paths — *while `ext.rs` still
   accepts both*. Zero behavior change; `cargo test -p taliesin-core` + `taliesin check` green.
2. **Drop `.qmd` acceptance + the `revealjs`/`ojs_define` aliases.** §3.A, §3.B, §3.C, §4
   (validator did-you-mean). Nothing references `.qmd` anymore, so this is a clean flip. Green.
3. **Remove the migration on-ramps + reword user-facing prose.** §3.D, §3.E, delete
   `migrating-from-quarto.qmd`. Green + `check` clean.

Each phase is an independently reviewable, bisectable commit.

## 8. Verification (corpus-plus-roadmap)

- `cargo test -p taliesin-core` green after **each** phase.
- `taliesin check` clean on the migrated corpus + docs books — proves there are no broken
  `.qmd`-links post-rename (the primary risk in §6).
- Browser deck smoke (chrome-devtools MCP): a `format: deck` doc still mounts the native engine,
  0 console errors.
- Proposed durable **grep-gate** (a test or CI check): assert no user-facing "Quarto" survives —
  grep CLI `--help` output + built docs-*guide* HTML + `README.md`, allowlisting the §5 keeps —
  so the purge can't silently regress. (If cheap; otherwise a one-off grep at the end.)
- `cargo fmt` clean (enforced by the PostToolUse hook + CI); clippy clean; `tsc` clean for
  `qmd-js.js`/client.

## 9. Non-goals

- **No syntax divergence.** Markdown/Pandoc syntax stays exactly as-is; "beyond Markdown" is a
  separate, bigger, not-now decision.
- **No `taliesin migrate` importer.** The owner chose purist removal, not the "reframe as a
  one-way importer" option; there is no convert command in scope.
- **No touching the internal wire / freeze format.** The `qmd-define` script type and
  `freeze.rs` `FORMAT_VERSION` are untouched (renaming them would cold-replay every cache for
  zero author benefit).

## 10. Accepted tradeoffs

- Removing `.quarto` from the watcher ignore-list means a stray `.quarto/` cache dir (left over
  from a user's prior Quarto project) would trigger preview reloads. Acceptable: a Taliesin
  project should not carry one, and the owner chose full purist removal over keeping the
  functional skip.
- This is a **breaking release**: existing `.qmd` files (including the owner's external books,
  e.g. FL-weather) must be renamed to `.tmd` and have `format: revealjs`/`ojs_define` updated.
  That is the intended cost of a clean separation.
