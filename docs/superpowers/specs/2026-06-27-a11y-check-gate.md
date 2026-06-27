# a11y check gate — port the static a11y audit into `qmd-fast check`

Date: 2026-06-27
Lane: release-hardening / tier1-a11y

## Goal

Today the accessibility audit only runs in the LIVE PREVIEW: a client-side
`scanA11y` in `web-client/client.js` checks alt text, heading-level skips,
link/button accessible names, document language, and body-text contrast. None
of that reaches `qmd-fast check`, the kernel-free static diagnostics channel.
The project doctrine is "a green check/build must mean publishable", so the
statically-knowable subset of that audit belongs in `check` too.

This lane adds a new `pub fn validate_a11y(...)` to
`crates/core/src/diagnostics.rs` (the check-superset module) and wires it into
`collect_diagnostics` (both the single-file and site paths) in
`crates/server/src/main.rs`, exactly as the existing validators
(`validate_local_links`, `validate_local_media`, `validate_js_reactive_graph`,
…) are wired.

## Which `scanA11y` rules port, and which do not

| `scanA11y` rule | static? | in `validate_a11y`? |
| --- | --- | --- |
| 1. `<img>` missing `alt` | yes | YES (raw/passthrough `<img>` only) |
| 2. heading level skips `>1` deeper | yes | YES (conservative; decks skipped) |
| 3. link/button with no accessible name | yes | YES (content only) |
| 4. document missing `<html lang>` | partly | NO — `lang:` defaults to `en` in the page builders, so a built page is never lang-less; this is a live-DOM concern, not a source defect |
| 5. body-text contrast < 4.5:1 | NO | NO — contrast needs *computed* CSS (resolved colors, cascade, theme), which a static block-model pass cannot evaluate. Left to the live audit. |

So three rules ship; `lang` and `contrast` are judged not-statically-knowable
(or already guaranteed by the builder) and are intentionally skipped.

## The three shipped rules

`validate_a11y(blocks: &[Block], format: DocFormat) -> Vec<Warning>`. It takes
`format` (unlike the other validators) only to skip decks for the heading-skip
rule — slides are intentionally slide-structured, so a per-slide `## … ####`
sequence is not a document outline. The image + accessible-name rules still run
on decks.

1. **Heading-level skip.** Walk the heading blocks (`<h1>`..`<h6>`) in document
   order; flag a heading whose level is `>= prev + 2` (e.g. an `<h2>` followed
   by an `<h4>`). Mirror `scanA11y` exactly: only a *mid-document* skip
   (`prev > 0`) counts, so we never flag "doesn't start at h1". SKIP entirely
   when `format == DocFormat::Reveal`.

   Message: `heading level skips from h{prev} to h{lvl} (add an intervening heading, or demote this one)`

2. **Interactive element with no accessible name.** An `<a href>` or
   `<button>` in content whose text content (tags stripped) is empty AND which
   carries no `aria-label`/`title` AND no `alt`-bearing `<img>` / labelled
   `<svg>` descendant. This catches an icon-only link with nothing for a screen
   reader to announce. Chrome controls already carry `aria-label`s and never
   pass through the block model, so this only sees content.

   Message: `link has no accessible name (icon-only? add aria-label or visible text)` / `button has no accessible name …`

3. **`<img>` without `alt`.** A raw/passthrough `<img>` tag with no `alt`
   attribute at all. Markdown `![]()` always emits an `alt` (even `alt=""`), so
   this catches hand-written `<img>` only.

   Message: `image is missing alt text (add alt text, or alt="" if decorative)`

Each warning is located to the block's start line via `start_line(&b.sourcepos)`
+ `b.source_file`, so it is click-to-source like every other diagnostic.

## Zero false positives across the corpus

`check_superset_has_no_false_positives_across_corpus` renders every corpus doc
(projects as dirs, standalone docs as files, `corpus/diagnostics/` exempt) and
asserts no diagnostic matches any check-superset substring. After wiring
`validate_a11y` in, its message substrings are added to that guard list and the
test must stay green. The rules are deliberately conservative (only raw `<img>`
without `alt`; only a `>=2`-level mid-document skip; decks skipped; only a
truly empty interactive element) so they do not fire on existing corpus docs.
If a corpus doc has a genuine a11y defect, fix the doc minimally; bias toward a
conservative check.

## Files

- `crates/core/src/diagnostics.rs` — `validate_a11y` + unit tests.
- `crates/server/src/main.rs` — wire into `collect_file_diagnostics` +
  `collect_site_diagnostics`; extend the corpus guard list. (No other regions
  touched — another lane owns `usage()`, `main()`, `cmd_check` formatting,
  `cmd_build`, kernel hints.)
- `corpus/diagnostics/a11y.qmd` — the existing a11y demo doc (exempt from the
  guard) trips each rule; asserted by a `collect_diagnostics`-style integration
  test.
- `docs/guide/reference/cli.qmd` — add the a11y rules to the "check rules" table.

## Invariants

Read-only static analysis. Only reads `Block.html` / `.sourcepos` /
`.source_file`. Does not touch the live audit in `web-client/client.js`
(porting a subset, not refactoring). Keeps `cargo fmt` clean.
