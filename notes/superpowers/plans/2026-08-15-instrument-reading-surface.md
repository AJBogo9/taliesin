# Instrument theme, Plan 2 of 4: the reading surface

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the three copies of the width-escape arithmetic with one reading grid, so a
code block leaves the prose measure instead of clipping inside it; make the margin column a
permanently reserved track and give a collapsed sidenote a way back to its reference; and
restyle the components that sit on that surface — code, callouts, figures, captions, tables,
TOC, title block — deleting, in each component's own commit, the knobs that its new anatomy
no longer varies.

**Architecture:** Plan 1 made the *material* right (two owned faces, two scored palettes, one
geometry scale) without changing any anatomy. This plan changes the anatomy of the reading
page and nothing else: chrome (navbar, footer, listing cards, drawer, dev UI) is Plan 3, and
the feature subtractions plus the three additions are Plan 4. Almost all of the work is in
`base.css`; the Rust changes are four small emission edits (a caption label span, a sidenote
back-link, the callout attribute retirements, and two register entries).

**Tech Stack:** Rust (edition 2024), `include_str!`-bundled CSS/JS, CSS Grid with named lines
(no `subgrid`, no container queries — see Task 2), `crates/core/src/render/tests.rs` for the
static gates.

**Spec:** [docs/superpowers/specs/2026-08-14-instrument-theme-design.md](../specs/2026-08-14-instrument-theme-design.md)

**Predecessor:** [docs/superpowers/plans/2026-08-14-instrument-foundation.md](2026-08-14-instrument-foundation.md)
(merged; `main` at `4ce5b58e`, 12/12 gates green)

---

## Global Constraints

- **`TALIESIN_PYTHON` is inherited POISONED** in this environment — it points at another
  project's `.venv`, which does not exist. Prefix **every** `cargo test --workspace`, **every**
  `./tools/gates.sh` and **every** `git push` with `TALIESIN_PYTHON="$PWD/.venv/bin/python"`.
  Without it four kernel tests "FAIL" without ever reaching an assertion, and the failure reads
  exactly like "my change broke the kernel".
- **Never run two `cargo test` invocations against this workspace concurrently** — the second
  hangs the first. If a suite seems slow, check `ps` for a long-`etime`, low-CPU
  `target/debug/deps/taliesin-*` before blaming the code.
- **Every task runs `cargo test --workspace`, not `-p taliesin-core`.** Plan 1 lost a red test
  for three tasks to exactly this, and the crate-only run also cannot see
  `crates/server/tests/asset_bundle.rs`, which this plan's Task 3 touches indirectly.
- **Editing `assets/css/*` or `assets/js/*` needs a `cargo build` before any test observes
  it.** They are `include_str!`-compiled.
- **Run `cargo fmt --all` LAST**, after every `.rs` edit — a `PostToolUse` hook runs `rustfmt`
  per file and fights a mid-stream `cargo fmt`.
- **`./tools/gates.sh` runs once, after the final task**, and its verdict line reports its own
  gate count. Never copy a gate count out of prose.
- **Branch first.** Work on `instrument-theme-reading-surface`; do not commit to `main`.
- **Never publish a number about the tool without a committed instrument.** A number with no
  instrument carries its measured-on date instead.
- **The ordering rule binds.** A corpus document, a docs page and a pin die in the *same*
  commit as the feature they guard, never in an earlier one.
- **A retirement is one register line plus stopping the parser reading the key.** The register
  note is ONE sentence — the date, then the successor or an explicit "nothing" — and may never
  be phrased as a did-you-mean. Do not write a tombstone test; the register derives it.
- **Two line coordinate systems.** If any diagnostic is added, a `source_file` may only be
  paired with a *mapped* line, never a buffer line.
- Values copied verbatim from the spec: measure `32em`, `U = 1.9375rem`, radius `2px` on
  interactive objects and `0` on structure, margin column `16em` with a `3em` gutter engaging
  near 1022 px, mono at `0.92em`, machine voice `0.78rem` uppercase `letter-spacing: .053em`
  weight 400, no shadows, no backdrop blur, hover may not move anything.

---

## Decisions taken before this plan was written

Both were the author's; both were put to them and answered on 2026-08-15.

1. **`thead th` keeps `font-weight: 400`.** It is consistent with the other seven machine-voice
   selectors in the tree (`h4`/`h5`/`h6`, `#TOC`, `.tali-nav-inner`, `.tali-foot-inner`,
   `.tali-title-meta`, `details.tali-code-fold > summary.tali-code-label`,
   `.callout-title.callout-kind`), and `thead th`'s own `border-bottom: 2px` against the body
   rows' `1px` already separates the header without weight. **No task in this plan touches that
   declaration.**
2. **The mobile serif scale is the desktop scale × 0.8.** Under `max-width: 30rem` the body
   already steps 20px → 16px (a factor of 0.8) while `h1` had no override at all, so a document
   title (`1.7rem` = 27.2px) rendered *smaller* than a body `#` (`2.25rem` = 36px). Every serif
   size in that breakpoint takes the same 0.8 the body takes; the mono voice does not move,
   because it is a fixed `0.78rem` on every surface. **Task 7 implements it.**

## Rulings taken while writing this plan

These are decisions the spec did not settle. Each is one or two lines to reverse.

- **R1: spec §3's spacing scale gains `1.5U`, and the scale governs block-level flow spacing
  only.** §3 pins `{0.5U, U, 2U, 3U}` *and* a heading `margin-top : margin-bottom` of 3:1 or
  4:1. Those two rules are jointly unsatisfiable: from a four-member set the only legal pairs
  are `(2U, 0.5U)` = 4:1 and `(3U, U)` = 3:1, and the second is *larger* than the first, so at
  most one heading level can be spaced legally. Adding `1.5U` yields `(1.5U, 0.5U)` = 3:1 and
  the ladder works. Second half of the ruling: `0.5U` is 15.5 px, which as a table-cell padding
  triples a row's height — so the scale governs margins between flow blocks, not the internal
  padding of small objects, which stays on its own quarter-unit sub-multiples. **Task 1 writes
  both halves into the spec; Task 7 implements them.** *Cost if wrong:* one scale member and a
  sentence.
- **R2: `h4`–`h6` take `(U, 0.5U)` = 2:1, a stated deviation from the ratio rule.** With the
  five-member scale the legal 3:1/4:1 pairs are `(1.5U, 0.5U)`, `(2U, 0.5U)` and `(3U, U)`;
  `h2` and `h3` take the first two, and the only one left is bigger than both. Giving `h4` a
  legal ratio needs either a sixth and seventh member (`0.75U`/`0.25U`) or `h4` spaced
  identically to `h3`. The deviation is cheaper than either and it is recorded in the CSS.
  *Cost if wrong:* one `calc()`.
- **R3: a collapsed sidenote keeps the `:target` reveal and gains a back-link**, rather than
  rendering unconditionally in the flow as spec §6's sentence reads literally. The tree carries
  a measured objection to the literal form (`base.css:802-806`: there is one note *per
  reference*, so dropping every note into the flow splits the citing paragraph at the exact
  word the marker sits on). The defect §6 actually names is "with no way back", and the
  back-link is its whole fix. The print path already renders every note in flow, which is where
  "a note must be present" really binds. *Cost if wrong:* one CSS rule.
- **R4: the margin column engages only where nothing else owns the right band** — i.e. not on a
  page with a TOC rail. Engaging it there needs 82.5 rem (1320 px) before the reading grid, the
  gutter, the note column, the rail gap and the rail all fit; below that the text track would
  shrink under the measure to keep the column, and the measure is a spec invariant while the
  margin column is a feature. On a rail page the note keeps exactly today's collapsed form,
  now with the back-link. *Cost if wrong:* one media query, added later, with evidence.
- **R5: callout `appearance=` and `icon=` are retired in this plan, not Plan 4.** Spec §10
  lists them among "knobs a better default kills", which Plan 1 assigned to Plan 4 — but §5's
  restyle *deletes the axes they vary*: `appearance="simple"` removes the tinted title bar and
  `appearance="minimal"` thins the left rule, and after Task 5 there is no tint and the rule is
  2 px for every kind. Keeping them until Plan 4 would leave two named variants of an anatomy
  that no longer exists. Per the ordering rule they die with it. *Cost if wrong:* Plan 4 finds
  two of its rows already done.
- **R6: `column-screen` is cut in Task 2, `column-page` survives.** Spec §9 cut #9 names
  `column-screen` (~70 lines) and it is the only escape the new grid cannot express as a track
  (it needs `100vw`, `overflow-x: clip` on `<html>`, and a scrollbar-width correction, none of
  which a grid track has). `column-page` becomes the `bleed` span. *Cost if wrong:* the cut is
  spec-approved and reversible only by re-authoring the three rules it deletes.

---

## File structure

| File | Responsibility after this plan |
|---|---|
| `docs/superpowers/specs/2026-08-14-instrument-theme-design.md` | The binding authority, amended twice in Task 1 (the machine-voice *rule*; §3's spacing scale). |
| `crates/core/assets/css/tokens.css` | Gains `--tali-note-w`, `--tali-note-gap`, `--tali-bleed`. Still the one place a token is declared. |
| `crates/core/assets/css/base.css` | **The reading grid** (`--tali-prose-cols` + five rules) replacing three escape copies and five overrides; the restyled code block, callout, figure, caption, table, TOC and title block; the spacing scale applied. |
| `crates/core/assets/css/site.css` | Loses its copy of the escape arithmetic and its two "kill the margin note next to a rail" rules; `.tali-site-main` stops owning the measure and the side padding. |
| `crates/core/src/render/emit.rs` | The sidenote gains a back-link to `#fnref-<name>-1`. |
| `crates/core/src/render/figure.rs`, `cell_numbered.rs` | `Figure N` / `Listing N` become `<span class="tali-caption-label">`; the caption text stays serif. |
| `crates/core/src/render/divs.rs` | Loses `appearance=`, `icon=` and the three Octicon blobs. |
| `crates/core/src/render/validate.rs` | `DIV_FEATURE_CLASSES` drops to two; `RETIRED_DIV_CLASSES` gains `column-screen`; callout attributes consult the retirement register. |
| `crates/core/src/vocab.rs` | `DIV_CLASS_NAMES` drops to two (the subset relation is gated). |
| `crates/core/src/frontmatter.rs` | `RETIRED_KEYS` gains two `callout attribute` entries. |
| `crates/server/src/exec.rs` | The two executed-table captions use the shared label helper. |
| `crates/core/assets/js/code-enhance/07-keyboard.js` | **deleted** (spec §9 cut #13), with its `include_str!` and its `register` call. |
| `crates/core/tests/layout_escapes.rs` | Rewritten against the grid: one definition, not three. |
| `corpus/layout/escapes.tmd`, `corpus/callouts/kinds.tmd` | Lose the sections that pin the cut features, in the same commit as the cut. |
| `docs/guide/using/writing.tmd`, `using/preview.tmd`, `reference/cheatsheet.tmd` | Same, for the manual. |
| `crates/core/src/render/tests.rs` | The grid gate, the code-capacity gate + the mono binary pin, the derived-breakpoint gate, the caption-voice gate; the contrast gates stop hardcoding the code ground. |

---

## Task 1: The two carry-overs from Plan 1's final review

**Files:**
- Modify: `docs/superpowers/specs/2026-08-14-instrument-theme-design.md`
- Modify: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing code-facing. It amends the spec every later task argues from, and removes
  a hardcoded ground from the gates that will score Tasks 3-7.

**Why this is first.** Plan 1's final review named two things to do "before or inside Plan 2",
and the first carries the highest expected cost of anything deferred: two contrast gates score
against a *literal* `#f4f1eb`/`#1c1a15` rather than reading `--tali-code-bg`. Any later retune
of the code ground leaves them green while real contrast drops. The second is that the machine
voice has had three exceptions carved by ruling (the wordmark, author names and affiliations,
authored callout titles) and each was found only after it shipped — because the spec lists
*instances* and never states the rule that generates them.

- [ ] **Step 1: Write the machine-voice rule into the spec**

In §4, immediately under the "The machine's voice" heading, before the existing `0.78rem`
sentence, insert:

```markdown
> **The rule, which generates every exception below.** The machine voice attaches to a label
> the TOOL generates. It never attaches to a container that may hold the AUTHOR's text.
> Applying it to a container is how the wordmark (`_site.yml title:`), author names and
> affiliations, and callout titles (34 of 55 in this repo are authored) each shipped uppercased
> and were each fixed one ruling at a time. When a selector *sometimes* holds authored text,
> the distinction is marked structurally at emission — `divs.rs` marks the generated
> kind-word branch, `model.rs` marks the generated code-fold label — never guessed in CSS.
```

- [ ] **Step 2: Reconcile §3's spacing scale with the tree**

In §3's table, replace the `Spacing scale` row with:

```markdown
| Spacing scale | `{0.5U, U, 1.5U, 2U, 3U}` between flow blocks, and nothing else | replaces 39 ad-hoc values. `1.5U` is not decoration: with the four-member set this row first carried, the only pairs satisfying the ratio row below are `(2U, 0.5U)` = 4:1 and `(3U, U)` = 3:1, and the second is larger than the first — so at most ONE heading level could be spaced legally. This row governs margins between blocks; the internal padding of a small object (a table cell, a `kbd`, a copy button) is a quarter-unit sub-multiple, because `0.5U` is 15.5 px and triples a table row |
```

And append to the `Space distribution` row's Why cell:

```markdown
. `h4`-`h6` are a stated exception at 2:1 (`U : 0.5U`): they are the mono label rather than a serif section heading, and no five-member scale can give them a legal ratio that is also smaller than `h3`'s
```

- [ ] **Step 3: Prove the contrast gates are scoring a stale ground**

```bash
grep -n '#f4f1eb\|#1c1a15' crates/core/src/render/tests.rs
```

Expected: six hits, in three tests. Now demonstrate the hole. In `tokens.css`, temporarily
change `--tali-code-bg: #F4F1EB;` to `--tali-code-bg: #6E6A60;` (the comment colour — a ground
that puts inline code at roughly 1:1), then:

```bash
cargo build -p taliesin-core
cargo test -p taliesin-core every_text_colour_is_scored_in_both_palettes \
                            the_syntax_palette_is_owned_and_scored
```

Expected: **both PASS** on a page whose code ground is now unreadable. That is the defect.
Revert `tokens.css` to `#F4F1EB` before continuing (`git diff` must be empty for that file).

- [ ] **Step 4: Make every gate read the ground it actually scores against**

Each of the three tests takes the ground from the token sheet instead of a literal. The
existing helper `color_after(css, "--tali-code-bg:")` already returns the hex.

In `every_text_colour_is_scored_in_both_palettes` and in the callout/inline-code loop that
shares its shape, replace the four-tuple loop header:

```rust
    for (theme, css, bg, code_bg) in [
        ("light", TOKENS_CSS, "#fbf9f5", "#f4f1eb"),
        ("dark", TOKENS_DARK_CSS, "#14130f", "#1c1a15"),
    ] {
```

with:

```rust
    // The grounds are READ, never spelled. A literal here scores against whatever the ground
    // was on the day the test was written: a retune of --tali-bg or --tali-code-bg leaves
    // every assertion green while the page it describes loses real contrast. Demonstrated —
    // with #F4F1EB swapped for #6E6A60 both of these tests passed on an unreadable page.
    for (theme, css) in [("light", TOKENS_CSS), ("dark", TOKENS_DARK_CSS)] {
        let bg = color_after(css, "--tali-bg:");
        let code_bg = color_after(css, "--tali-code-bg:");
```

In `the_syntax_palette_is_owned_and_scored`, the scope colours live in `BASE_CSS`/`DARK_CSS`
while the ground lives in the token sheets, so the loop carries both:

```rust
    for (theme, css, tokens) in [
        ("light", BASE_CSS, TOKENS_CSS),
        ("dark", DARK_CSS, TOKENS_DARK_CSS),
    ] {
        let bg = color_after(tokens, "--tali-code-bg:");
```

and the third test's two `wcag_contrast(light, "#f4f1eb")` / `wcag_contrast(dark, "#1c1a15")`
pairs take the same two `color_after` reads. `color_after` returns an owned `String`; pass it
as `&bg` where the helper wants `&str`.

- [ ] **Step 5: Prove the gates now fail on a bad ground, then restore**

Re-apply the `--tali-code-bg: #6E6A60;` edit, then:

```bash
cargo build -p taliesin-core
cargo test -p taliesin-core every_text_colour_is_scored_in_both_palettes \
                            the_syntax_palette_is_owned_and_scored
```

Expected: **FAIL**, naming the real ratio. Revert `tokens.css` (confirm `git diff
crates/core/assets/css/tokens.css` is empty), rebuild, and re-run:

```bash
cargo build -p taliesin-core
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: green. Both the deliberate failure and the passing run belong in the task report.

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/specs crates/core/src/render/tests.rs
git commit -m "test(theme): score contrast against the token, not a copy of it

Two gates hardcoded #f4f1eb/#1c1a15 as the code ground. Demonstrated: with
--tali-code-bg swapped to #6E6A60 both tests still passed, on a page where inline
code sits at roughly 1:1. They now read --tali-bg and --tali-code-bg from the token
sheet, so a retune of the ground is scored rather than assumed.

Also writes into the spec the RULE that generates its three carved machine-voice
exceptions (the voice attaches to a generated label, never to a container that may
hold author text), and reconciles the spacing scale with the heading ratio rule it
contradicted: a four-member scale admits only one legal heading pair, so it gains
1.5U and a sentence scoping it to flow blocks rather than object padding."
```

---

## Task 2: The reading grid — one definition replacing three

**Files:**
- Modify: `crates/core/assets/css/tokens.css`, `base.css`, `site.css`
- Modify: `crates/core/src/render/validate.rs`, `crates/core/src/vocab.rs`
- Modify: `corpus/layout/escapes.tmd`, `docs/guide/using/writing.tmd`,
  `docs/guide/reference/cheatsheet.tmd`
- Test: `crates/core/tests/layout_escapes.rs`, `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-measure`, `--tali-u` (Plan 1).
- Produces: `--tali-bleed`, `--tali-note-w`, `--tali-note-gap`, `--tali-prose-cols`, and the
  named grid lines **`text`**, **`bleed`**, **`note`**. Tasks 3 and 4 place blocks on those
  lines by name; nothing after this task may re-derive a width with `calc(50% - …)`.

**Why.** Spec §6: the width-escape arithmetic exists in three near-identical copies, and the
replacement is one grid. Two further pairs of rules exist only because those copies could not
see each other — `body.has-toc > main :is(.column-page, .column-screen)` and its `.tali-site-main`
twin, plus the two rules that force a margin note back into the flow beside a TOC rail. The
grid deletes all of it: a note in the `note` track and a rail in a different track of a
different grid cannot overlap by construction.

**Verified facts this task rests on** (measured 2026-08-15 against the tree at `4ce5b58e`):

- `--tali-measure: 32em` of a `1.25rem` body is 640 px = **40rem**; the spec's `16em` margin
  column is **20rem** and its `3em` gutter is **3.75rem**.
- The reading region is `<main id="tali-main">` in a build and site page (`page.rs:621,626`),
  and `<main id="tali-root">` in the live preview (`client.js:962`: "blocks are exactly
  `root`'s element children"). **Both must be grid containers or the redesign is invisible in
  the dev loop.** A dated post additionally wraps its blocks in `<article>` (`page.rs:612`).
- `.tali-site-main` and `.tali-book-main` are `border-box` with `2rem 1rem` padding *inside*
  `max-width: var(--tali-measure)`, so a site page renders **63.7 characters** where a single
  document renders **67.0** — the same token, two measures. Moving the side padding into the
  grid's own gutters fixes that discrepancy as a side effect.
- `column-screen` has one corpus witness (`corpus/layout/escapes.tmd:54-59`), two docs rows
  (`writing.tmd:252,261`, `cheatsheet.tmd:78`) and three register/vocabulary sites.

- [ ] **Step 1: Write the failing test**

In `crates/core/src/render/tests.rs`:

```rust
/// The width-escape arithmetic has ONE definition. It had three near-identical ones
/// (`.column-page/.column-screen`, `body.has-toc > main :is(…)`, and the `.tali-site-main`
/// twin of the second), plus two narrow-screen re-overrides and two rules that forced margin
/// notes back into the flow beside a TOC rail — seven places that had to agree, and the
/// clipping visible in every render of this design happened where they did not.
///
/// The replacement is a grid with named lines, so a block declares WHICH COLUMN it is in
/// instead of recomputing a centring formula. This test is the anti-drift half: it fails if a
/// second definition reappears.
#[test]
fn the_width_escape_has_exactly_one_definition() {
    for (name, css) in [("base.css", BASE_CSS), ("site.css", SITE_CSS)] {
        for (needle, why) in [
            ("--tali-escape-room", "the retired per-container escape budget"),
            ("--tali-escape-w", "the retired computed escape width"),
            ("calc(50% - ", "the retired centring formula"),
            ("column-screen", "cut 2026-08-15; the grid has no full-bleed track"),
        ] {
            assert!(
                !css.contains(needle),
                "{name} still carries `{needle}` ({why}); the reading grid replaced it"
            );
        }
    }
    // One track list, and every consumer names it rather than repeating it.
    assert_eq!(
        BASE_CSS.matches("--tali-prose-cols:").count(),
        1,
        "the track list must be declared exactly once"
    );
    assert_eq!(
        BASE_CSS.matches("grid-template-columns: var(--tali-prose-cols)").count(),
        1,
        "every grid container shares one rule; a second one is a second definition"
    );
    // The preview mounts into `#tali-root` and a build into `#tali-main`; a grid that names
    // only one of them is invisible in exactly the loop the author works in.
    for id in ["#tali-main", "#tali-root"] {
        assert!(
            BASE_CSS.contains(id),
            "the reading grid must cover {id} (build page AND live preview mount)"
        );
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p taliesin-core the_width_escape_has_exactly_one_definition
```

Expected: FAIL on `base.css still carries --tali-escape-room`.

- [ ] **Step 3: Add the three geometry tokens**

In `tokens.css`, after the `--tali-measure` / `--tali-chrome-maxw` block:

```css
    /* The reading grid's geometry. The track LIST itself is `--tali-prose-cols` in base.css,
       where the layout it describes lives; only the numbers are tokens.

       Stated in `rem` and not in the spec's `em`, and that is load-bearing rather than a
       style preference: these are re-declared inside a media query on a container whose own
       `em` would resolve against the ROOT's 16px, silently rendering the margin column 20%
       narrow. The spec's numbers are `em` OF THE BODY FACE, so the conversion is exact and
       fixed — 16em x 1.25rem = 20rem, 3em x 1.25rem = 3.75rem.

       Both note tokens are ZERO here, deliberately: the margin column is a real grid track,
       so collapsing it to nothing is how the layout below its breakpoint is expressed. One
       track list, not two spellings of one. base.css engages them. */
    --tali-note-w: 0;
    --tali-note-gap: 0;
    /* How far a code block or a `::: {.column-page}` may grow, and it grows LEFT: with a
       permanent margin column on the right, left is the only direction that cannot run
       under a note. 20rem + the 40rem measure = 60rem, which is exactly the cap the three
       retired copies of the escape formula each carried as `min(60rem, …)` — the number
       survives the rewrite, only its expression changes. */
    --tali-bleed: 20rem;
```

- [ ] **Step 4: Write the grid, and delete the three copies**

In `base.css`, replace the whole `.column-page, .column-screen` block (the rule, both
`--tali-escape-w` definitions, the `html:has(.column-screen)` clip rule and the `@media print`
escape reset) with:

```css
  /* ===== THE READING GRID ====================================================
     ONE definition of where a block sits horizontally. It replaces three near-identical
     copies of a centring formula (`.column-page/.column-screen`, `body.has-toc > main
     :is(…)`, `.tali-site-main.has-toc > main :is(…)`), their two narrow-screen
     re-overrides, and the two rules that killed margin notes beside a TOC rail.

     Five tracks. Prose sits in `text`. A code block or an author escape spans `bleed` —
     the band to the LEFT plus the text track. The margin column is `note`, permanently
     reserved so a floated note has somewhere to go that nothing else may occupy.

     Why the escape grows LEFT rather than centring: the margin column owns the right band,
     and a box centred on the page runs its text under a note's text. The tree already ruled
     exactly this for the TOC rail on a measurement (a `.column-page` reached x=1331 while
     the rail began at x=1111 at 1702px). With the margin column permanent, the same answer
     holds everywhere — which is precisely what lets three copies become one.

     The side gutters are the grid's own `minmax(1rem, 1fr)` and NOT the containers'
     padding. That is a fix, not tidiness: `.tali-site-main`/`.tali-book-main` are
     `border-box` with `2rem 1rem` INSIDE `max-width: var(--tali-measure)`, so a site page
     rendered 63.7 characters where a single document rendered 67.0 — one token, two
     measures. One gutter, one measure, all three modes. */
  :root {
    --tali-prose-cols:
      [bleed-start] minmax(1rem, 1fr)
      minmax(0, var(--tali-bleed))
      [text-start] min(var(--tali-measure), 100%) [text-end bleed-end]
      var(--tali-note-gap)
      [note-start] var(--tali-note-w) [note-end]
      minmax(1rem, 1fr);
  }
  #tali-main, #tali-root, #tali-main > article {
    display: grid; grid-template-columns: var(--tali-prose-cols); }
  :is(#tali-main, #tali-root, #tali-main > article) > * { grid-column: text; }
  /* A dated post wraps its blocks in an `<article>` landmark, so the grid must reach one
     level deeper. It spans the whole track set and REPEATS the track list rather than using
     `subgrid`: its width equals the parent's content box, so identical tracks compute
     identical sizes, while an unsupported `subgrid` would be dropped as an invalid value and
     silently render every block full-bleed. This rule follows the `> *` rule above at equal
     specificity, so source order is what makes it win. */
  #tali-main > article { grid-column: 1 / -1; }
  /* `::: {.column-page}` is the author's opt-in to the wider band; Task 3 puts `pre` there
     too. There is no full-bleed class any more: `.column-screen` needed `100vw`, a
     scrollbar-width correction and `overflow-x: clip` on `<html>`, none of which is a grid
     track, and it was cut with this rewrite (spec §9). */
  :is(#tali-main, #tali-root, #tali-main > article) > .column-page { grid-column: bleed; }
```

Then, still in `base.css`:

- delete `body.has-toc > main :is(.column-page, .column-screen)` and the `@media (max-width:
  60rem)` copy of it;
- delete `body.has-toc > main .column-margin`, `body.has-toc > main .tali-sidenote` and
  `body.has-toc > main .tali-sidenote:target` (the grid makes the collision they solved
  impossible; Task 4 owns what replaces them);
- retune the two container rules:

```css
  /* The reading grid owns the measure and the side gutters, so `body` is just the page. */
  body { max-width: none; margin: calc(2 * var(--tali-u)) 0; padding: 0;
         font: var(--tali-font-body); color: var(--tali-fg); background: var(--tali-bg);
         overflow-wrap: break-word;
         font-feature-settings: "liga" 1, "calt" 1, "kern" 1;
         -webkit-font-smoothing: antialiased; -moz-osx-font-smoothing: grayscale; }
  /* A page with a TOC keeps its sticky rail as a SIBLING TRACK of the reading grid rather
     than a competitor for the same space: the rail is the second column of this outer grid
     and the reading grid lives entirely inside the first. That is why the four rules that
     used to force notes and escapes back into the flow beside a rail are gone.
     Cap: 85.75rem (the reading grid at full stretch: 1 + 20 + 40 + 3.75 + 20 + 1) + 2.5rem
     gap + 14rem rail = 102.25rem. */
  body.has-toc { display: grid; align-items: start; gap: 2.5rem; justify-content: center;
                 max-width: 102.25rem; grid-template-columns: minmax(0, 1fr) 14rem; }
```

and in the `@media print` block, replace the deleted escape reset with one line:

```css
    /* The reading grid is a screen layout; paper has one column. Collapsing the grid
       collapses every escape and the margin column with it, in one declaration. */
    #tali-main, #tali-root, #tali-main > article { display: block !important; }
```

- [ ] **Step 5: Do the same to `site.css`**

Replace `.tali-site-main`, `.tali-site-main.has-toc` and the four `has-toc > main` override
rules with:

```css
  /* The column no longer owns the measure or the side padding — the reading grid inside
     `<main>` owns both (base.css). Keeping `max-width: var(--tali-measure)` here with
     `border-box` padding is what made a site page's measure 63.7 characters against a
     single document's 67.0. */
  .tali-site-main { box-sizing: border-box; flex: 1 0 auto; width: 100%;
    max-width: none; margin: 0 auto; padding: calc(var(--tali-u)) 0; }
  .tali-site-main > main { min-width: 0; }
  /* `page-layout: full` (the blog/projects card indexes) widens the TEXT track rather than
     the container, because the container no longer has a width to widen. Listing cards are
     Plan 3's subject; this keeps them exactly as wide as they are today. */
  .tali-site-main.tali-wide > main { --tali-measure: 60rem; }
  /* Same outer grid as base.css's single-document `body.has-toc`, and for the same reason:
     the rail is a sibling track, not a competitor for the reading grid's space. */
  .tali-site-main.has-toc { display: grid; align-items: start; gap: 2.5rem;
    max-width: 102.25rem; grid-template-columns: minmax(0, 1fr) 14rem; }
```

and in the `@media (max-width: 60rem)` block, delete the `.tali-site-main.has-toc > main
:is(.column-page, .column-screen)` rule and the `.tali-site-main.has-toc:not(.tali-wide)`
max-width restatement, keeping the `grid-template-columns: minmax(0, 1fr)` collapse and the
`#TOC` re-order.

Finally, the book column, which has no rail at all:

```css
  .tali-book-main { box-sizing: border-box; flex: 1 0 auto; width: 100%;
    max-width: none; margin: 0 auto; padding: var(--tali-u) 0; }
```

- [ ] **Step 6: Retire `column-screen` — register, vocabulary, corpus, docs, in this commit**

In `crates/core/src/render/validate.rs`:

```rust
pub(crate) const DIV_FEATURE_CLASSES: &[&str] = &["column-margin", "column-page"];
```

and add to `RETIRED_DIV_CLASSES` (one sentence, no did-you-mean):

```rust
    (
        "column-screen",
        "it was removed on 2026-08-15 with the reading grid, which has no full-bleed track: \
         use `.column-page`, which reaches 60rem",
    ),
```

In `crates/core/src/vocab.rs`, drop `column-screen` from `DIV_CLASS_NAMES` and from the
descriptions map beside it (`descriptions_present` gates the pair).

In `corpus/layout/escapes.tmd`, delete the `## \`.column-screen\`, edge to edge {#sec-screen}`
section and its fenced div, and any `@sec-screen` reference to it. In
`docs/guide/using/writing.tmd` delete the `.column-screen` clause at :252 and the table row at
:261; in `docs/guide/reference/cheatsheet.tmd` delete the row at :78.

Also update the two `validate.rs` test fixtures that spell the three-class list
(`validate.rs:500` and the neighbouring case at :595) to the surviving two.

- [ ] **Step 7: Rewrite `crates/core/tests/layout_escapes.rs`**

The file exists to pin an arithmetic that no longer exists. It keeps its subject (an escape
leaves the measure; both containers agree) and changes its instrument. Replace its body with:

```rust
//! Layout escapes after the reading grid (2026-08-15): `::: {.column-page}` is a NAMED GRID
//! COLUMN, not a centring formula, so what this file pins is that there is one grid, that the
//! escape spans the wider band, and that the band preserves the cap the formula carried. The
//! three copies this used to compare against each other are gone; comparing them was the whole
//! reason the file was long.
//!
//! Still pinned on the stylesheet SOURCE rather than a rendered page, for the reason the old
//! header gave and this rewrite does not change: the escape has no render path at all — it is
//! a plain class on a plain block — and every Taliesin page inlines the whole stylesheet, so
//! `page.contains(".column-page")` is true on a page that renders no escape.

use std::path::Path;

fn css(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/css")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// The escape is a grid span. If it ever goes back to computing a margin, this fails.
#[test]
fn the_escape_is_a_grid_span_not_an_arithmetic() {
    let base = css("base.css");
    assert!(
        base.contains("> .column-page { grid-column: bleed; }"),
        "`.column-page` must be a named grid span"
    );
    assert!(
        !base.contains("margin-left: calc("),
        "an escape computed from a margin is the formula this grid replaced"
    );
}

/// The band the escape reaches is the cap the retired formula carried: 20rem + the 40rem
/// measure = 60rem. That the number survives is what makes the rewrite lossless.
#[test]
fn the_escape_band_preserves_the_sixty_rem_cap() {
    let bleed = css("tokens.css")
        .split("--tali-bleed:")
        .nth(1)
        .expect("--tali-bleed is defined")
        .split(';')
        .next()
        .unwrap()
        .trim()
        .trim_end_matches("rem")
        .parse::<f64>()
        .expect("--tali-bleed is in rem");
    // 32em of a 1.25rem body = 40rem.
    assert_eq!(bleed + 40.0, 60.0, "the escape cap moved off 60rem");
}
```

Keep the file's own `css()` reader and its `Path` import: this is an integration test binary,
so it reads the sheets from disk rather than through the crate's `include_str!` consts, and the
`rule()` helper it carries today goes with the rules it was written to slice.

- [ ] **Step 8: Run everything**

```bash
cargo build
cargo test -p taliesin-core the_width_escape_has_exactly_one_definition
cargo test -p taliesin-core --test layout_escapes
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: PASS. Two other suites will speak up and both are the gates working:
`every_tali_custom_property_read_is_defined_somewhere` if a deleted token is still read
anywhere, and the `RETIRED_DIV_CLASSES` table-driven tombstone if `column-screen` is still
live in `DIV_FEATURE_CLASSES`, still styled, or still spelled in a corpus document.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(theme): one reading grid replaces three copies of the escape formula

The width-escape arithmetic existed in three near-identical copies plus two
narrow-screen re-overrides plus two rules that forced margin notes back into the flow
beside a TOC rail: seven places that had to agree. A block now declares which grid
column it is in.

Side gutters move from the containers to the grid, which also fixes a real
discrepancy: .tali-site-main and .tali-book-main are border-box with 2rem 1rem inside
max-width: var(--tali-measure), so a site page rendered 63.7 characters where a single
document rendered 67.0 from the same token.

Cuts .column-screen with its register entry, its corpus witness and its two docs rows
in this commit (the ordering rule): it needed 100vw, a scrollbar correction and
overflow-x: clip on <html>, none of which is a grid track."
```

---

## Task 3: Code leaves the measure

**Files:**
- Modify: `crates/core/assets/css/base.css`
- Delete: `crates/core/assets/js/code-enhance/07-keyboard.js`
- Modify: `crates/core/src/render/mod.rs` (the `include_str!` list),
  `crates/core/assets/js/code-enhance/09-register.js`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: the `bleed` grid line from Task 2; `--tali-mono-size`, `--tali-u`,
  `--tali-measure`, `--tali-bleed`.
- Produces: `JETBRAINS_MONO_ADVANCE_EM`, a dated constant, and the mono binary's FNV pin.

**Why.** Spec §6: "**This is what fixes the clipping visible in every render in this design's
history:** a `pre` must leave the prose measure rather than scroll inside it. A 640 px column
at mono 0.92 em fits ~58 columns against PEP 8's 79, and no single width serves both."

**Verified facts** (measured 2026-08-15 with `fontTools` against the vendored binary):
JetBrains Mono has `unitsPerEm = 1000` and **every one of its 229 non-combining glyphs has an
advance of 600** — it is a monospace, so `0.6 em` is exact rather than a sample mean, unlike
Literata's measured 0.4775. At `--tali-mono-size: .92em` of a 20 px body that is 11.04 px per
column. The spec's "~58 columns" is the raw 640 px column; inside `pre`'s padding the real
figure today is **55**. In the `bleed` band it is **84**.

- [ ] **Step 1: Write the failing test**

```rust
/// JetBrains Mono's advance, in `em`. MEASURED 2026-08-15 from the vendored binary with
/// fontTools: `unitsPerEm` 1000, and all 229 non-combining glyphs carry an advance of 600.
/// It is a monospace, so unlike Literata's mean this is exact — but it still describes ONE
/// binary, which is what the hash below is for.
const JETBRAINS_MONO_ADVANCE_EM: f64 = 0.6;

/// A code block must leave the prose measure. At the measure a `pre` fits 55 columns inside
/// its own padding, against PEP 8's 79 and Black's 88 — so the code a reader meets was
/// clipped and scrolled in every render of this design. The grid gives `pre` the `bleed`
/// band; this asserts the capacity that buys, from the tokens rather than from prose.
#[test]
fn a_code_block_escapes_the_measure_and_clears_pep8() {
    let rem = |css: &str, tok: &str, unit: &str| -> f64 {
        css.split(tok)
            .nth(1)
            .unwrap_or_else(|| panic!("{tok} is defined"))
            .split(';')
            .next()
            .unwrap()
            .trim()
            .trim_end_matches(unit)
            .parse()
            .unwrap_or_else(|v| panic!("{tok} must be in `{unit}`: {v}"))
    };
    let measure_em = rem(TOKENS_CSS, "--tali-measure:", "em"); // 32em OF THE BODY
    let bleed_rem = rem(TOKENS_CSS, "--tali-bleed:", "rem");
    let mono_em = rem(TOKENS_CSS, "--tali-mono-size:", "em");
    let u_rem = rem(TOKENS_CSS, "--tali-u:", "rem");

    // The body is 1.25rem, so an `em` of the body face is 20px and a `rem` is 16px.
    let track_px = measure_em * 20.0 + bleed_rem * 16.0;
    // `pre` padding is .5U on each side; asserted literally so this arithmetic cannot drift
    // away from the sheet it describes.
    assert!(
        BASE_CSS.contains("padding: calc(.5 * var(--tali-u))"),
        "pre's padding must be .5U, which this capacity figure subtracts"
    );
    let content_px = track_px - u_rem * 16.0; // 2 x .5U
    let col_px = JETBRAINS_MONO_ADVANCE_EM * mono_em * 20.0;
    let cols = content_px / col_px;
    assert!(
        cols >= 79.0,
        "a code block fits {cols:.0} columns; PEP 8 is 79. Either the bleed band, the \
         measure, the mono size or pre's padding moved."
    );
}

/// The advance above describes ONE binary. Same instrument as the body face's pin: hash the
/// file so a swap cannot be silent.
#[test]
fn the_mono_face_is_the_one_the_columns_were_measured_on() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/fonts/jetbrains-mono-latin-wght-normal.woff2");
    let bytes = std::fs::read(&p).expect("the vendored mono face");
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in &bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    assert_eq!(
        (bytes.len(), h),
        (19_768, 0x0), // <- replace 0x0 with the value the first run prints
        "the mono face changed. Re-measure JETBRAINS_MONO_ADVANCE_EM from the binary \
         (fontTools: hmtx advance / head.unitsPerEm), update it and this hash together, \
         and re-date both."
    );
}
```

- [ ] **Step 2: Run both, read the hash out of the failure**

```bash
cargo test -p taliesin-core a_code_block_escapes_the_measure_and_clears_pep8 \
                            the_mono_face_is_the_one_the_columns_were_measured_on -- --nocapture
```

Expected: the capacity test FAILS (`pre`'s padding is still `1rem`); the pin FAILS printing the
real `(len, hash)` tuple. Paste that tuple in, replacing the `0x0` placeholder — this is the
one kind of placeholder that is correct, because the value is an output of the build rather
than a decision.

- [ ] **Step 3: Put `pre` in the bleed band**

In `base.css`, in the reading-grid block, extend the escape rule and add the `pre` sizing:

```css
  :is(#tali-main, #tali-root, #tali-main > article) > :is(pre, .column-page) {
    grid-column: bleed; }
  /* Code leaves the prose measure, and it grows LEFT from the text column's right edge.
     At the measure a `pre` fits 55 columns inside its padding against PEP 8's 79, so the
     code on the page was clipped and scrolled in every render of this design; in the bleed
     band it fits 84 (measured: JetBrains Mono's advance is exactly .6em, and .92em of a
     20px body is 11.04px per column).

     `width: max-content` with a `min-width` floor of the measure is what keeps a two-line
     snippet from becoming a 960px slab: a short block is exactly as wide as the prose and
     sits in the same place it does today, and only a block that needs the room takes it.
     `justify-self: end` is why it grows left — the right edge stays flush with the text
     column, which is what makes the escape collision-free against the margin column. */
  :is(#tali-main, #tali-root, #tali-main > article) > pre {
    justify-self: end;
    width: max-content;
    min-width: min(var(--tali-measure), 100%);
    max-width: 100%; }
```

and change `pre`'s own padding onto the scale:

```css
  pre { position: relative; padding: calc(.5 * var(--tali-u)); border-radius: var(--tali-radius);
        overflow: auto; line-height: 1.5;
```

(the rest of the `pre` rule — the scroll-shadow background layers — is unchanged.)

- [ ] **Step 4: Delete the arrow-key hijack (spec §9 cut #13)**

`07-keyboard.js` binds left/right to the book pager and skips only a target matching
`a,button,input,select,textarea`. `16-scroll-a11y.js` deliberately gives a scrollable `<pre>`
and `<table>` `tabindex="0"` **so that arrow keys scroll them** — a `<pre>` is not in the skip
list, so in a book the two fragments contradict each other, and this task makes horizontal
scrolling inside a `<pre>` a designed behaviour (84 columns, then scroll) rather than an
accident. The spec's own reason is that the shortcut has been undiscoverable since its
cheatsheet was cut.

```bash
git rm crates/core/assets/js/code-enhance/07-keyboard.js
```

In `crates/core/src/render/mod.rs`, delete the line
`include_str!("../../assets/js/code-enhance/07-keyboard.js"),` from the fragment list. In
`09-register.js`, delete `reg.register(function () { taliInitKeyboard(); });`, leaving:

```js
// Register the built-ins through the public API.
var reg = window.taliEnhancers;
if (reg) {
  reg.register(taliCopyButtons);
  reg.register(function () { taliInitSkipLink(); });
}
```

- [ ] **Step 5: Run everything**

```bash
cargo build
cargo test -p taliesin-core a_code_block_escapes_the_measure_and_clears_pep8 \
                            the_mono_face_is_the_one_the_columns_were_measured_on
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json; cd -
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: PASS, and the `tsc` check clean (a dangling `taliInitKeyboard` reference would show
up there, which is why it runs here and not only in the gate script).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(theme): code leaves the prose measure

At the measure a <pre> fits 55 columns inside its own padding, against PEP 8's 79 —
so the code a reader meets has been clipped and scrolled in every render of this
design. In the grid's bleed band it fits 84, pinned to the vendored binary: JetBrains
Mono's advance is exactly .6em (measured, all 229 non-combining glyphs), and .92em of
a 20px body is 11.04px per column.

A short snippet does not become a slab: width is max-content with the measure as a
floor, so only a block that needs the room takes it, and it grows left so its right
edge stays flush with the prose.

Deletes 07-keyboard.js (spec cut #13): it bound the arrow keys to the book pager while
16-scroll-a11y.js gives a scrollable <pre> tabindex=0 precisely so the arrow keys can
scroll it. This commit makes that scrolling a designed behaviour, so the conflict is
no longer latent."
```

---

## Task 4: The permanent margin column, and the way back

**Files:**
- Modify: `crates/core/assets/css/base.css`
- Modify: `crates/core/src/render/emit.rs`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: the `note` grid track and `--tali-note-w` / `--tali-note-gap` from Task 2.
- Produces: `<a class="tali-sidenote-back">` inside every `.tali-sidenote`, targeting the
  `id="fnref-<name>-1"` that `footnote_ref_markup` already emits.

**Why.** Spec §6: "`16em` beside the `32em` measure with a `3em` gutter, engaging near 1022 px
(today: 1168 px). **Below the breakpoint the note renders inline with a back-link to its
reference.** Today it is `display: none` behind `:target` with no way back — the one component
whose reduced form is a defect rather than a simplification."

Per **R3** the `:target` reveal stays and the back-link is added; per **R4** the column engages
only where nothing else owns the right band. Both are argued above.

**Verified facts:** the reference already carries an id (`emit.rs:283`,
`<sup class="tali-fnref" id="fnref-{name}-{ref_num}">`) and comrak numbers references from 1,
so `#fnref-<name>-1` is the first reference and needs no new markup on that end. The note is
spliced beside its first reference, once (`tests.rs:2425`). **The mono subset has no `↩`
(U+21A9) and no `←` (U+2190)** — the Latin range vendored by `tools/subset-fonts.sh` carries
only `U+2191`/`U+2193` — so a back-link drawn as a glyph would fall back to a system font. It
is a word in the machine voice instead, which is also what the rule written into the spec in
Task 1 prescribes: the tool generated this label.

- [ ] **Step 1: Write the failing test**

```rust
/// A collapsed sidenote can get back to where the reader was. Below the margin breakpoint the
/// note is revealed IN PLACE by its own reference, which leaves the reader wherever the jump
/// landed them: the note was `display: none` behind `:target` with no route back to the
/// sentence they were reading. The reference already carries `id="fnref-<name>-1"`, so this
/// needs no new markup on the other end — only a link.
#[test]
fn a_collapsed_sidenote_links_back_to_its_reference() {
    let page = render_html_page(
        "---\ntitle: T\n---\n\nText with a note.[^a]\n\n[^a]: The note.\n",
        "bl",
    );
    assert!(
        page.contains("id=\"fnref-a-1\""),
        "the reference must carry the id the back-link targets"
    );
    assert!(
        page.contains("<a class=\"tali-sidenote-back\" href=\"#fnref-a-1\""),
        "the note must carry a link back to its first reference"
    );
    // Hidden in the margin form, where the note already sits beside its reference; visible
    // only when the note was revealed by being targeted.
    assert!(
        BASE_CSS.contains(".tali-sidenote-back { display: none; }"),
        "the back-link is chrome in the margin form and must not show there"
    );
    assert!(
        BASE_CSS.contains(".tali-sidenote:target .tali-sidenote-back"),
        "the back-link must be revealed with the note it belongs to"
    );
}

/// The breakpoint that engages the margin column is DERIVED, not chosen: it is the sum of the
/// tracks that have to fit. A hand-picked number drifts away from the geometry the moment
/// either token moves, and nothing would say so.
#[test]
fn the_margin_column_breakpoint_is_the_sum_of_its_tracks() {
    let num = |css: &str, tok: &str, unit: &str| -> f64 {
        css.split(tok).nth(1).unwrap().split(';').next().unwrap().trim()
            .trim_end_matches(unit).parse().unwrap()
    };
    // Engaged values live in the media query in base.css, not in tokens.css (which declares
    // the collapsed 0). Read them from the query itself.
    let engaged = BASE_CSS
        .split("/* ENGAGED */")
        .nth(1)
        .expect("the engagement block is marked `/* ENGAGED */` so this test can find it");
    let note_w = num(engaged, "--tali-note-w:", "rem");
    let note_gap = num(engaged, "--tali-note-gap:", "rem");
    let measure_rem = num(TOKENS_CSS, "--tali-measure:", "em") * 1.25; // em of a 1.25rem body
    let want = 1.0 + measure_rem + note_gap + note_w + 1.0; // two 1rem page gutters
    // The query's value ends at `)`, not at a `;`, so it needs its own read.
    let got: f64 = engaged
        .split("@media (min-width:")
        .nth(1)
        .expect("the engagement block is a min-width query")
        .split(')')
        .next()
        .unwrap()
        .trim()
        .trim_end_matches("rem")
        .parse()
        .expect("the breakpoint is in rem");
    assert!(
        (got - want).abs() < 0.51,
        "the margin column engages at {got}rem but its tracks need {want}rem"
    );
}
```

`render_html_page(markdown, id)` is the helper the neighbouring footnote tests at
`tests.rs:2356-2444` already call; the second argument is a short page id and any unused
two-letter string will do.

- [ ] **Step 2: Run and watch both fail**

```bash
cargo test -p taliesin-core a_collapsed_sidenote_links_back_to_its_reference \
                            the_margin_column_breakpoint_is_the_sum_of_its_tracks
```

Expected: FAIL — there is no back-link and no engagement block.

- [ ] **Step 3: Emit the back-link**

In `crates/core/src/render/emit.rs`, in `footnote_sidenote`, replace the returned format
string with:

```rust
    (
        format!(
            "<span class=\"tali-sidenote\" id=\"fn-{n}\" role=\"doc-footnote\" \
             data-block-id=\"fn-{n}\" data-sourcepos=\"{sourcepos}\"{file_attr}>\
             <span class=\"tali-sidenote-num\">{ix}</span>{inner}\
             <a class=\"tali-sidenote-back\" href=\"#fnref-{n}-1\" \
             aria-label=\"Back to reference {ix}\">Back</a></span>"
        ),
        flattened,
    )
```

and extend the doc comment above it:

```rust
/// The trailing back-link is the collapsed form's whole point of difference. Below the margin
/// breakpoint the note is revealed in place by `:target`, which leaves the reader wherever the
/// jump landed them; `footnote_ref_markup` already emits `id="fnref-<name>-<n>"` and comrak
/// numbers references from 1, so the first reference is a stable target and no markup changes
/// on that end. The label is a WORD and not a `↩`: the vendored mono subset carries neither
/// U+21A9 nor U+2190, so a glyph here would silently fall back to a system font.
```

Update the two tests that pin the exact markup (`tests.rs:2379` and the full string at
`tests.rs:2444`) to include the new suffix.

- [ ] **Step 4: Write the margin column, mobile-first**

In `base.css`, replace the `.tali-sidenote` / `.column-margin` blocks and their two
`@media (max-width: 73rem)` fallbacks with:

```css
  /* THE MARGIN COLUMN — the collapsed form is the default and the column is the addition,
     because the column is what is conditional. `.tali-sidenote` (a footnote) and
     `.column-margin` (an author's own marginal aside) share one geometry deliberately, so
     two notes hanging off the same paragraph occupy ONE column rather than two overlapping
     ones. */
  .tali-sidenote { display: none; }
  /* Revealed in place by its own reference: the `<sup>` is already an `<a href="#fn-x">` and
     the note already carries `id="fn-x"`, so `:target` is the whole mechanism — no checkbox,
     no hidden form control in the prose, no JS. What was missing was the way back. */
  .tali-sidenote:target { display: block; margin: calc(.5 * var(--tali-u)) 0;
    padding-left: .9rem; border-left: 2px solid var(--tali-accent); }
  .column-margin { margin: var(--tali-u) 0; padding-left: .9rem;
    border-left: 2px solid var(--tali-accent); }
  .tali-sidenote, .column-margin {
    font: var(--tali-font-body); font-size: .82rem; font-weight: 400; line-height: 1.5;
    text-transform: none; letter-spacing: normal;
    color: var(--tali-muted); text-align: left; }
  .tali-sidenote-num { font-size: .75em; vertical-align: super; line-height: 0;
    margin-right: .35em; color: var(--tali-accent); }
  .column-margin > :first-child { margin-top: 0; }
  .column-margin > :last-child { margin-bottom: 0; }
  /* The way back. Hidden in the margin form, where the note already sits beside its own
     reference and a back-link would point two centimetres left. */
  .tali-sidenote-back { display: none; }
  .tali-sidenote:target .tali-sidenote-back { display: inline; margin-left: .5em;
    font: 400 .78rem/1.3 var(--tali-font-mono); text-transform: uppercase;
    letter-spacing: .053em; }
  /* ENGAGED */
  /* The margin column engages when the page can afford it, and the number is DERIVED:
     1rem gutter + 32em measure + 3em gutter + 16em note + 1rem gutter = 66rem (1056px). The
     spec's stated ~1022px is the same sum without the two page gutters.
     Not on a page with a TOC rail: there the same sum gains 2.5rem + 14rem = 82.5rem, and
     below that the text track would have to shrink under the measure to keep the column —
     the measure is a spec invariant and the margin column is a feature. A rail page keeps
     the collapsed form it has always had, now with the way back.
     The track and the float engage TOGETHER, on the same selector list: a float sized from
     `--tali-note-w` when the track is 0 lands in the page gutter. */
  @media (min-width: 66rem) {
    body:not(.has-toc), .tali-site-main:not(.has-toc), .tali-book-main {
      --tali-note-w: 20rem; --tali-note-gap: 3.75rem; }
    /* `float` and not grid placement, because a footnote note is a `<span>` inside a
       paragraph and can never be a grid item — but a float with a negative right margin
       lands exactly in the track the grid holds empty for it, and several notes hanging off
       one paragraph push each other DOWN the column in document order, where absolutely
       positioned notes would stack on top of one another. */
    :is(body:not(.has-toc), .tali-site-main:not(.has-toc), .tali-book-main)
      :is(.tali-sidenote, .column-margin) {
      display: block; float: right; clear: right; width: var(--tali-note-w);
      margin: .3rem calc(-1 * (var(--tali-note-w) + var(--tali-note-gap)))
              var(--tali-u) var(--tali-note-gap);
      padding-left: 0; border-left: 0; }
  }
```

In the `@media print` block, keep the existing "every note in the flow" rule and add one line,
so a printed note never carries a dead link:

```css
    .tali-sidenote-back { display: none !important; }
```

- [ ] **Step 5: Run everything**

```bash
cargo build
cargo test -p taliesin-core a_collapsed_sidenote_links_back_to_its_reference \
                            the_margin_column_breakpoint_is_the_sum_of_its_tracks
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: PASS. `crates/core/tests/corpus.rs:781` walks every `<span class="tali-sidenote"` in
the corpus — read its assertion if it fires; the block-id and sourcepos attributes are
unchanged, so it should not.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(theme): a permanent margin column, and a way back out of a note

The margin column is a reserved grid track rather than space borrowed from the page
gutter, and its breakpoint is derived from the tracks that must fit rather than chosen:
1rem + 32em + 3em + 16em + 1rem = 66rem. It does not engage beside a TOC rail, where
the same sum needs 82.5rem and the text track would otherwise shrink under the measure.

The real fix is smaller and older: a collapsed note was display:none behind :target
with no route back to the sentence being read. It now carries a link to
#fnref-<name>-1, which the reference has always emitted. The label is the word BACK
and not a return arrow, because the vendored mono subset carries neither U+21A9 nor
U+2190 and a glyph would silently fall back to a system font.

The note stays :target-revealed rather than rendering unconditionally in the flow, as
spec §6 reads literally: there is one note per REFERENCE, so in-flow notes split the
citing paragraph at the exact word the marker sits on (base.css records the
measurement). Print already renders every note in flow, which is where presence binds."
```

---

## Task 5: Callouts — a rule and a word

**Files:**
- Modify: `crates/core/assets/css/base.css`
- Modify: `crates/core/src/render/divs.rs`, `crates/core/src/frontmatter.rs`
- Modify: `corpus/callouts/kinds.tmd`, `docs/guide/using/preview.tmd`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-callout-note` / `-tip` / `-warning` (Plan 1), the machine-voice
  declaration block.
- Produces: nothing later tasks read.

**Why.** Spec §5: "Three kinds, distinguished by a 2 px left rule and the mono kind-word." Spec
§3 puts radius `0` on callouts and §1 puts no colour in the furniture, so the box, the radius,
the three `color-mix` title tints and the bundled Octicons all go. Per **R5** the two knobs go
with them in this commit, because they vary axes that no longer exist: `appearance="simple"`
removed the tint and `appearance="minimal"` thinned the rule.

- [ ] **Step 1: Write the failing test**

```rust
/// A callout is a 2px left rule and a kind word. The box, the radius, the tinted title bar
/// and the icon are gone (spec §5 + §3's radius row + §1's no-colour-in-chrome rule) — and
/// with the tint and the rule-width gone, `appearance=` and `icon=` vary nothing, so they are
/// retired in the same commit as the anatomy they described.
#[test]
fn a_callout_is_a_left_rule_and_a_kind_word() {
    for (needle, why) in [
        ("color-mix(in srgb, var(--tali-callout-", "the tinted title bar is chrome colour"),
        ("callout-simple", "`appearance=simple` varied only the tint, which is gone"),
        ("callout-minimal", "`appearance=minimal` varied only the rule width, now 2px"),
        ("callout-icon", "the bundled Octicons went with the box"),
    ] {
        assert!(!BASE_CSS.contains(needle), "base.css still has `{needle}`: {why}");
    }
    assert!(
        BASE_CSS.contains("border-left: 2px solid var(--tali-border-strong)"),
        "the callout's own edge is the 2px rule the kind then colours"
    );
    // The knobs stop being READ, not merely undocumented: a register entry alone leaves the
    // key live.
    let html = render_html_page(
        "---\ntitle: T\n---\n\n::: {.callout-tip appearance=\"simple\" icon=\"false\"}\nBody.\n:::\n",
        "ca",
    );
    assert!(!html.contains("callout-simple"), "`appearance=` must no longer be read");
    for key in ["appearance", "icon"] {
        assert!(
            crate::frontmatter::retired_note("callout attribute", key).is_some(),
            "`{key}` needs its one-line register entry under the `callout attribute` scope"
        );
    }
}
```

`retired_note(scope, key) -> Option<&'static str>` is the existing `pub` lookup over
`RETIRED_KEYS` (`frontmatter.rs:819`); `render_html_page(markdown, id)` is the same helper the
footnote tests use.

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p taliesin-core a_callout_is_a_left_rule_and_a_kind_word
```

Expected: FAIL on the `color-mix` tint.

- [ ] **Step 3: Restyle the callout**

In `base.css`, replace the whole callout block (from `.callout {` down to and including
`.callout-minimal .callout-icon`) with:

```css
  /* Three kinds, and the kind is carried by a 2px left rule plus the mono kind-word — not by
     a box, a tint or an icon (spec §5). What goes with that: the 1px box (structure here is
     square and unboxed; the rule and the indent are the whole affordance), the radius (spec
     §3 puts 0 on callouts), the three `color-mix` title tints (chrome carries no colour), and
     the three bundled Octicon blobs. `appearance=` and `icon=` are retired in the same commit
     because they varied exactly those axes: `simple` removed the tint, `minimal` thinned the
     rule, and neither exists to vary now. */
  .callout { border: 0; border-left: 2px solid var(--tali-border-strong); border-radius: 0;
             margin: 0 0 var(--tali-u); padding-left: calc(.5 * var(--tali-u)); }
  .callout-note { border-left-color: var(--tali-callout-note); }
  .callout-tip { border-left-color: var(--tali-callout-tip); }
  .callout-warning { border-left-color: var(--tali-callout-warning); }
```

Then, in the `.callout-title` rules that follow, drop the `padding` and the `gap` (there is no
icon to sit beside any more) and put the title's bottom margin on the scale, leaving the two
voice branches exactly as Plan 1 settled them:

```css
  .callout-title { font: var(--tali-font-body); font-weight: 600; line-height: 1.3;
                   text-transform: none; letter-spacing: normal;
                   margin: 0 0 calc(.5 * var(--tali-u)); display: flex; align-items: center; }
  .callout-title.callout-kind { font: 400 .78rem/1.3 var(--tali-font-mono);
                   text-transform: uppercase; letter-spacing: .053em; }
  .callout-body { padding: 0; }
  .callout-body > :first-child { margin-top: 0; }
```

Keep the `.callout-collapse` caret rules unchanged: `.callout-title` is still `display: flex`,
which is exactly what its `margin-left: auto` needs, and the comment explaining why the caret
is `::after` rather than `::before` still holds.

- [ ] **Step 4: Stop reading the two attributes, and register them**

In `crates/core/src/render/divs.rs`, delete the `icon` and `appearance` blocks and the
`callout_icon` function with its three Octicon `<svg>` blobs, and drop `{icon}` and
`{appearance}` from both `format!` arms:

```rust
                format!(
                    "<div class=\"callout callout-{kind} callout-collapse\"{data}><details{open}><summary class=\"{title_class}\"{title_id_attr}>{title}</summary><div class=\"callout-body\">{body}</div></details></div>"
                )
```

```rust
                "<div class=\"callout callout-{kind}\"{data}><div class=\"{title_class}\"{title_id_attr}>{title}</div><div class=\"callout-body\">{body}</div></div>"
```

In `crates/core/src/frontmatter.rs`, add two `RETIRED_KEYS` entries under a new
`callout attribute` scope (one sentence each, no did-you-mean):

```rust
    (
        "callout attribute",
        "appearance",
        "it was removed on 2026-08-15 with the callout's box and title tint, which are the \
         only things it varied: there is nothing",
    ),
    (
        "callout attribute",
        "icon",
        "it was removed on 2026-08-15 with the bundled callout icons: there is nothing",
    ),
```

Then make the register reachable from a callout's attributes — without inventing an
open-vocabulary lint, which wave 9 cut for exactly this shape of check. In `divs.rs`, where the
callout's attributes are parsed, consult the register and nothing else:

```rust
        // The register is the ENTIRE vocabulary consulted here. Callout attributes are an
        // open set (`title=`, `collapse=`, `id=`, …), so an unknown-attribute lint would be
        // the generic check wave 9 deliberately cut; a retired one, though, is a name the
        // author is still typing and the tool silently ignores.
        for key in attrs.keys() {
            if let Some(note) = crate::frontmatter::retired_note("callout attribute", key) {
                warnings.push(Warning::new(format!("`{key}=` {note}")).at(file, line));
            }
        }
```

`retired_note(scope, key) -> Option<&'static str>` is the existing `pub` lookup at
`frontmatter.rs:819`; call it rather than adding a second one. **The `line` passed here must be
a mapped line, never a buffer line** (see the Global Constraints) — read how the surrounding
callout-kind validator obtains its `file`/`line` and use the same pair; if `divs.rs` has no
warning sink at that point, put the check where `validate_callout_kind` is already called,
which does.

- [ ] **Step 5: Delete the pins in the same commit (the ordering rule)**

In `corpus/callouts/kinds.tmd`, delete the three demonstration callouts at :21-30 (the
`appearance="simple"`, `appearance="minimal"` and `icon="false"` blocks) together with their
prose. In `docs/guide/using/preview.tmd:98`, change "`title=`, `collapse=`, `icon=` and
`appearance=` — the four a callout reads" to name the two that survive.

- [ ] **Step 6: Run everything**

```bash
cargo build
cargo test -p taliesin-core a_callout_is_a_left_rule_and_a_kind_word
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: PASS. Existing callout tests that assert on `callout-icon` or a tinted title will
fail — read each one: if it was pinning the deleted anatomy, delete the assertion; if it was
pinning the authored-vs-generated title split, it must still pass untouched (Plan 1's fix wave
made that split structural in `divs.rs`, and weakening one of those four assertions would undo
it).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(theme): a callout is a left rule and a kind word

Deletes the 1px box, the radius, the three color-mix title tints and the three bundled
Octicon blobs: spec §5 says the kind is carried by a 2px left rule and the mono
kind-word, §3 puts radius 0 on a callout, and §1 puts no colour in the furniture.

appearance= and icon= are retired in the same commit rather than deferred to Plan 4,
because they varied exactly the axes this commit deletes — simple removed the tint,
minimal thinned the rule. Both stop being read, both get their one register line, and
the corpus witnesses and the docs sentence go with them.

The authored-vs-generated title split is untouched: it is marked structurally in
divs.rs and all four of its assertions still pass as written."
```

---

## Task 6: Figures, captions and tables — the number speaks, the caption does not

**Files:**
- Modify: `crates/core/src/render/figure.rs`, `cell_numbered.rs`, `mod.rs` (one re-export)
- Modify: `crates/server/src/exec.rs`
- Modify: `crates/core/assets/css/base.css`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: nothing from Tasks 2-5.
- Produces: `render::caption_label(label, num) -> String`, used by four core call sites and
  two in `crates/server`.

**Why.** Spec §4, and its own correction from a render: "Captions are **prose** and stay in the
serif (italic, 0.92 rem). Only the `Figure 3` *number* takes the machine voice. The first
render set whole captions in mono and they read as terminal output." Today the label and the
caption are one undifferentiated string in six `format!`s, so there is nothing for CSS to
address.

- [ ] **Step 1: Write the failing test**

```rust
/// The generated part of a caption is the tool's label; the rest is the author's sentence.
/// They must be separately addressable, or the choice is between a whole caption in mono
/// (which reads as terminal output — the correction spec §4 records from a render) and a
/// whole caption in serif (which loses the machine voice on the one word that is the tool's).
#[test]
fn a_caption_number_is_the_machine_voice_and_the_caption_is_not() {
    let page = render_html_page(
        "---\ntitle: T\n---\n\n![A river at dusk](img.png){#fig-r}\n",
        "fg",
    );
    assert!(
        page.contains("<span class=\"tali-caption-label\">Figure&nbsp;1</span>: A river at dusk"),
        "the label is a span and the caption is not inside it"
    );
    assert!(
        BASE_CSS.contains(".tali-caption-label { font: 400 .78rem/1.3 var(--tali-font-mono);"),
        "the label takes the machine voice"
    );
    // The caption is prose: serif, italic, and NOT uppercased by the label beside it.
    // `text-transform` and `letter-spacing` are inherited, so the label must reset them
    // rather than rely on the figcaption not setting them.
    assert!(
        BASE_CSS.contains("figcaption { font: var(--tali-font-body); font-size: .92rem;"),
        "a caption is prose in the serif"
    );
    assert!(
        BASE_CSS.contains("font-style: italic"),
        "spec §4: captions are italic at .92rem"
    );
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p taliesin-core a_caption_number_is_the_machine_voice_and_the_caption_is_not
```

Expected: FAIL — the caption is one flat string.

- [ ] **Step 3: Add the shared label helper**

In `crates/core/src/render/cell_numbered.rs`, beside `numbered_caption`:

```rust
/// The generated half of a caption — `Figure 3`, `Table 2`, `Listing 7` — wrapped so CSS can
/// address it. This is the ONE word in a caption the tool wrote; everything after the colon is
/// the author's sentence and stays in the serif (spec §4's correction from a render: whole
/// captions in mono read as terminal output). Shared by the figure, listing, mermaid and
/// `{js}`-figure emitters here and by the executed-table captions in `crates/server`.
pub fn caption_label(label: &str, num: &str) -> String {
    format!("<span class=\"tali-caption-label\">{label}&nbsp;{num}</span>")
}
```

and rewrite `numbered_caption` to use it:

```rust
pub(crate) fn numbered_caption(label: &str, num: &str, caption: Option<&str>) -> String {
    let head = caption_label(label, num);
    match caption.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("{head}: {}", caption_inline_html(c)),
        None => head,
    }
}
```

Re-export it in `render/mod.rs` beside the existing `pub(crate) use cell_numbered::numbered_caption;`:

```rust
pub use cell_numbered::caption_label;
```

- [ ] **Step 4: Route the four remaining call sites through it**

`crates/core/src/render/figure.rs:125-128` builds its caption inline:

```rust
        "<figure{block_attrs}{id_attr} class=\"tali-figure{align_class}\">\
         …\
         <figcaption>{}: {}</figcaption></figure>",
        crate::render::caption_label("Figure", num),
        fig.caption,
```

(`fig.caption` is already rendered HTML, so it does not go through `numbered_caption`, which
parses markdown.) The other three core sites already call `numbered_caption` and need no edit.

In `crates/server/src/exec.rs`, the two `Table&nbsp;` sites at :1591 and :1620 take the same
helper — `render::caption_label` is reachable there (`exec.rs:32` already imports
`render::{self, …}`):

```rust
        "{}{open}<caption>{}{sep}{}</caption>{}",
        …
        render::caption_label("Table", &tbl.num),
```

Read each call's existing argument order before editing; the `{sep}` and caption arguments are
unchanged.

- [ ] **Step 5: Style the caption and the label**

In `base.css`, replace `figure.tali-figure figcaption` and `table caption` with:

```css
  /* A caption is PROSE: the author's sentence about their own figure, so it is the serif,
     italic, one step under the body. Only the generated `Figure 3` inside it is the tool
     speaking. The first render of this theme set whole captions in mono and they read as
     terminal output; that correction is in spec §4. */
  figcaption, table caption { font: var(--tali-font-body); font-size: .92rem;
    font-style: italic; font-weight: 400; color: var(--tali-muted);
    text-transform: none; letter-spacing: normal; }
  figure.tali-figure figcaption { margin-top: calc(.5 * var(--tali-u)); }
  figure.tali-table-figure figcaption { margin-top: 0; margin-bottom: calc(.25 * var(--tali-u)); }
  table caption { caption-side: top; padding-bottom: calc(.25 * var(--tali-u));
    text-align: left; }
  /* The generated half. Upright against the italic caption around it, and it re-declares
     `text-transform`/`letter-spacing` because both are inherited: a label inside a caption
     inside a machine-voice ancestor would otherwise compute the ancestor's tracking. */
  .tali-caption-label { font: 400 .78rem/1.3 var(--tali-font-mono); font-style: normal;
    text-transform: uppercase; letter-spacing: .053em; }
```

Put the figure and table block margins on the scale:

```css
  figure.tali-figure { margin: 0 0 var(--tali-u); }
```

and the table's cell padding on a quarter unit (per **R1**'s second half — `0.5U` is 15.5 px
and triples a row):

```css
  th, td { border: 1px solid var(--tali-border); padding: calc(.25 * var(--tali-u)) .6rem; }
```

**Do not change `thead th`.** Its `font-weight: 400` is an author decision taken 2026-08-15 and
recorded at the top of this plan. The vertical rules between cells are also deliberately left
alone: the spec says nothing about them, and dropping them would be this plan's invention.

- [ ] **Step 6: Run everything**

```bash
cargo build
cargo test -p taliesin-core a_caption_number_is_the_machine_voice_and_the_caption_is_not
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: PASS. Tests that assert on the literal string `Figure&nbsp;1` in emitted HTML will
fail and must gain the span; tests asserting on a cross-reference's link text (`site/xref.rs`
emits `Table&nbsp;1` inside `<a class="tali-xref">`) must **not** change — a cross-reference
sits inside the author's own sentence and is not the machine speaking.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(theme): the caption number speaks, the caption does not

A caption was one flat string in six format!s, so there was nothing for CSS to
address and the choice was between a whole caption in mono — which reads as terminal
output, the correction spec §4 records from a render — and no machine voice at all on
the one word the tool wrote.

caption_label() wraps the generated half; the author's sentence after the colon stays
serif, italic, .92rem. A cross-reference's link text is deliberately untouched: it sits
inside the author's own sentence.

Also puts figure margins and table cell padding on the spacing scale, at a quarter
unit for the cell — 0.5U is 15.5px and triples a table row, which is why the scale
governs flow spacing and not object padding."
```

---

## Task 7: The spacing scale, the TOC and the title block

**Files:**
- Modify: `crates/core/assets/css/base.css`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-u`; the amended spec §3 from Task 1.
- Produces: nothing.

**Why.** `tokens.css:72-79` says honestly that the scale "is not done — this layer only
supplies the unit … The scale lands with the components it measures: Plan 2 for the reading
surface, Plan 3 for the chrome." This is that landing, for the reading surface only, under
**R1** and **R2**. It also carries the author's mobile-scale decision.

- [ ] **Step 1: Write the failing test**

```rust
/// The reading surface's vertical rhythm is the line box. Every margin between flow blocks is
/// a member of `{0.5U, U, 1.5U, 2U, 3U}` — the four-member scale spec §3 first carried could
/// not satisfy its own heading-ratio rule at more than one level, because the only legal pairs
/// from four values are (2U, 0.5U) = 4:1 and (3U, U) = 3:1 and the second is the larger.
///
/// Scoped to the selectors this plan owns: `site.css` is Plan 3's, and object padding is
/// deliberately NOT on this scale (0.5U is 15.5px, which triples a table row).
#[test]
fn the_reading_surface_margins_are_on_the_spacing_scale() {
    // Every `calc(<n> * var(--tali-u))` in base.css must use a scale member.
    const SCALE: &[&str] = &[".25", ".5", "1.5", "2", "3"];
    for seg in BASE_CSS.split("calc(").skip(1) {
        let expr = seg.split(')').next().unwrap_or("");
        if !expr.contains("var(--tali-u)") {
            continue;
        }
        let factor = expr.split('*').next().unwrap_or("").trim();
        assert!(
            SCALE.contains(&factor) || factor.starts_with('-'),
            "`calc({expr})` uses {factor}U, which is not on the scale {SCALE:?}"
        );
    }
    // The heading ladder, with the ratios the spec's own rule asks for.
    assert!(BASE_CSS.contains("h2 { font-size: 1.6rem; line-height: 1.2; \
        margin: calc(2 * var(--tali-u)) 0 calc(.5 * var(--tali-u)); }"));
    assert!(BASE_CSS.contains("h3 { font-size: 1.3rem; line-height: 1.3; \
        margin: calc(1.5 * var(--tali-u)) 0 calc(.5 * var(--tali-u)); }"));
}

/// Under 480px the body steps 20px -> 16px and every serif size steps with it, by the same
/// factor. It did not: `h1` had no override at all, so a document `.title` (1.7rem = 27.2px)
/// rendered SMALLER than a body `#` (2.25rem = 36px). Author decision, 2026-08-15: the mobile
/// scale is the desktop scale x 0.8, which is the factor the body already takes. The mono
/// voice does not move — it is a fixed .78rem on every surface and at every width.
#[test]
fn the_mobile_scale_keeps_the_title_above_the_headings() {
    let mobile = BASE_CSS
        .split("@media (max-width: 30rem) {")
        .nth(1)
        .expect("the mobile breakpoint");
    let size = |sel: &str| -> f64 {
        mobile
            .split(sel)
            .nth(1)
            .unwrap_or_else(|| panic!("{sel} has no mobile size"))
            .split("font-size:")
            .nth(1)
            .unwrap()
            .split(&['r', ';'][..])
            .next()
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    };
    let title = size(".tali-title-block .title {");
    let h1 = size("h1 {");
    let h2 = size("h2 {");
    let h3 = size("h3 {");
    assert!(title > h1 && h1 > h2 && h2 > h3, "the scale must stay ordered: \
        title {title} > h1 {h1} > h2 {h2} > h3 {h3}");
    assert!(h3 * 16.0 >= 16.0, "no serif heading may sit under the 16px mobile body");
}
```

- [ ] **Step 2: Run and watch both fail**

```bash
cargo test -p taliesin-core the_reading_surface_margins_are_on_the_spacing_scale \
                            the_mobile_scale_keeps_the_title_above_the_headings
```

Expected: FAIL — `h3`'s bottom margin is `.4U`, `h4`'s is `.3U`, and there is no mobile `h1`.

- [ ] **Step 3: Snap the heading ladder to the scale**

In `base.css`, extend the heading-scale comment and retune the three margins:

```css
     Vertical rhythm: the ladder is `{0.5U, U, 1.5U, 2U, 3U}`. `1.5U` is not an ad-hoc value —
     from a four-member scale the only pairs satisfying the spec's own 3:1-or-4:1 heading
     ratio are (2U, 0.5U) and (3U, U), and the second is LARGER than the first, so at most one
     heading level could be spaced legally. h2 takes 4:1, h3 takes 3:1, and h4-h6 are a stated
     exception at 2:1: they are the mono label rather than a serif section heading, and no
     five-member scale can give them a legal ratio that is also smaller than h3's. */
  h1 { font-size: 2.25rem; line-height: 1.1; letter-spacing: -.008em; margin: 0 0 calc(.5 * var(--tali-u)); }
  h2 { font-size: 1.6rem; line-height: 1.2; margin: calc(2 * var(--tali-u)) 0 calc(.5 * var(--tali-u)); }
  h3 { font-size: 1.3rem; line-height: 1.3; margin: calc(1.5 * var(--tali-u)) 0 calc(.5 * var(--tali-u)); }
  h4, h5, h6 { font: 400 .78rem/1.3 var(--tali-font-mono); text-transform: uppercase;
       letter-spacing: .053em; margin: var(--tali-u) 0 calc(.5 * var(--tali-u)); }
```

(collapsing the three identical `h4`/`h5`/`h6` rules into one selector list is part of this
step — they were written out separately and have been byte-identical since Plan 1.)

Then sweep the rest of the reading surface onto the scale:

```css
  hr { border: 0; border-top: 1px solid var(--tali-border); margin: calc(2 * var(--tali-u)) 0; }
  .tali-title-block { margin: 0 0 calc(2 * var(--tali-u)); padding-bottom: var(--tali-u);
    border-bottom: 1px solid var(--tali-border); }
  .tali-output { margin: 0 0 var(--tali-u); }
  .tali-references .csl-entry { margin: calc(.5 * var(--tali-u)) 0;
    padding-left: 2.2rem; text-indent: -2.2rem; }
```

`blockquote`, `.tali-appendix` and the listing/card selectors are **not** in this sweep:
`.tali-appendix` dies with structured `author:` in Plan 4 and the cards are Plan 3's.

- [ ] **Step 4: Write the mobile scale**

Replace the `@media (max-width: 30rem)` block's type rules with the full ×0.8 ladder:

```css
  /* Under 480px the body steps 20px -> 16px, a factor of 0.8, and every serif size steps with
     it so the ladder keeps its proportions. It did not before: `h1` had no override at all, so
     a document title (1.7rem = 27.2px) rendered SMALLER than a body `#` (2.25rem = 36px).
     Author decision 2026-08-15. The mono voice does NOT move — .78rem is the machine voice at
     every width and on every surface, and 0.8 of it would be 10px. */
  @media (max-width: 30rem) {
    body { padding-bottom: 2.75rem; font-size: 16px; }
    .tali-title-block .title { font-size: 2rem; }        /* 32px  = 2.5rem  x .8 */
    .tali-title-block .subtitle { font-size: 1.12rem; }  /* 17.9px = 1.4rem x .8 */
    h1 { font-size: 1.8rem; }                            /* 28.8px = 2.25rem x .8 */
    h2 { font-size: 1.28rem; }                           /* 20.5px = 1.6rem  x .8 */
    h3 { font-size: 1.04rem; }                           /* 16.6px = 1.3rem  x .8 */
  }
```

Keep whatever else that block already carries (the side padding, and note that the grid now
supplies the side gutters, so a `padding-left`/`padding-right` here is redundant and should be
deleted — the gutters' `minmax(1rem, 1fr)` floor already reserves the space).

- [ ] **Step 5: The TOC**

`#TOC` already carries the machine voice from Plan 1 and needs only its spacing normalized and
its active marker left alone (`--tali-accent` is the ink, which is the intended emphasis):

```css
  #TOC { position: sticky; top: calc(.5 * var(--tali-u)); max-height: 92vh; overflow: auto;
         font: 400 .78rem/1.3 var(--tali-font-mono); text-transform: uppercase;
         letter-spacing: .053em; }
```

and in the `@media (max-width: 60rem)` block, the stacked form:

```css
    body.has-toc > #TOC { order: -1; position: static; max-height: 45vh; overflow: auto;
      border-bottom: 1px solid var(--tali-border);
      margin-bottom: var(--tali-u); padding-bottom: calc(.5 * var(--tali-u)); }
```

Make the same two-value change to `site.css`'s `.tali-site-main.has-toc > #TOC`, which is a
copy of this rule; that is the only `site.css` edit in this task.

- [ ] **Step 6: Run everything, then every gate**

```bash
cargo build
cargo test -p taliesin-core the_reading_surface_margins_are_on_the_spacing_scale \
                            the_mobile_scale_keeps_the_title_above_the_headings
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
cargo fmt --all
TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh
```

Expected: the workspace suite green, and `gates.sh` reporting `PASSED — every gate ran and
passed (N gates)` with **N taken from the script's own verdict line**. If a document gate
fails it is one of the three `.tmd` files this plan edited; read the message and fix the
document, not the gate.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(theme): the reading surface lands on the spacing scale

The heading ladder takes 4:1 (h2) and 3:1 (h3) against a single 0.5U bottom margin,
which is why the scale gains 1.5U: from the four values spec §3 first carried, the only
pairs satisfying its own ratio rule are (2U, 0.5U) and (3U, U), and the second is the
larger — so at most one heading level could be spaced legally. h4-h6 are a recorded
exception at 2:1; they are the mono label, not a serif section heading.

Under 480px every serif size now takes the 0.8 the body already took. Before this, h1
had no mobile override at all, so a document title rendered 27.2px against a body #
at 36px. Author decision 2026-08-15."
```

---

## Self-review

**Spec coverage.**

| Spec | Where |
|---|---|
| §3 spacing scale, space distribution | Task 1 (amended), Task 7 (implemented) |
| §3 radius `0` on `pre`, `table`, `figure`, callouts | Task 5 (callouts); `pre`/`table`/`figure` already carry it or `var(--tali-radius)` on the copy button, which §3 lists as an interactive object |
| §4 the machine's voice — the rule | Task 1 |
| §4 figure/table *numbers* in the mono, captions in the serif italic | Task 6 |
| §5 callout kinds: three, a 2 px left rule, the mono kind-word | Task 5 |
| §6 the bleed grid, one definition replacing three | Task 2 |
| §6 code leaves the measure | Task 3 |
| §6 the margin column, 16em + 3em gutter, engaging near 1022 px | Task 4 |
| §6 below the breakpoint, a back-link to the reference | Task 4 (as R3) |
| §9 cut #9 `column-screen` | Task 2 |
| §9 cut #11 callout `appearance=`, `icon=`, the Octicons | Task 5 (as R5) |
| §9 cut #13 `07-keyboard.js` | Task 3 |
| §12.3 the measure pinned in characters | Plan 1 Task 3; Task 3 here adds the mono half |

**Explicitly out of scope, and where each goes.** Chrome (navbar, footer, listing cards,
drawer, the dev UI and its status tokens, the CLI banner glyph) is Plan 3. Structured `author:`
and `.tali-appendix`, Cmd-K on standalone builds, the remaining §10 knobs, KaTeX face
subsetting, fonts-as-files, the orphan-page diagnostic and the `theming.tmd` rewrite are Plan
4. Spec §13's verification protocol is a manual browser pass that runs after Plan 3, when there
is a finished surface to verify; every gate in this plan is static analysis, because a browser
smoke test was decided against with evidence on 2026-08-13.

**Carried forward from Plan 1's ledger, not addressed here and deliberately so.**
`.tali-appendix h2` (17.1px) and `.tali-card-title` (17.28px) sit under the 20px prose — the
first dies in Plan 4, the second is a card and belongs to Plan 3. Three hover states in
`site.css` are silent no-ops: Plan 3. The mono subset lacks `U+2318` and `U+2192`, which needs
a re-vendor and invalidates the font hash pin: Plan 4. Task 4 is the one place that would
otherwise have wanted a missing glyph, and it uses a word instead.

**Placeholder scan.** One deliberate placeholder: the FNV tuple in Task 3 Step 1, which is an
output of the build rather than a decision, and Step 2 is the instruction to replace it. Every
helper the plan's test code calls was checked against the tree on 2026-08-15 rather than
assumed: `render_html_page(markdown, id)` (`tests.rs:2356`), `color_after` and `wcag_contrast`
(already used by the gates Task 1 edits), `retired_note(scope, key)` (`frontmatter.rs:819`,
`pub`), and `layout_escapes.rs`'s own `css(name)` disk reader — that file is an integration
binary and cannot see the crate's `include_str!` consts. Two steps still say "read the
surrounding code before editing": where `divs.rs` gets the `file`/`line` pair for a callout
warning (Task 5), and the argument order of `exec.rs`'s two table-caption `format!`s (Task 6).

**Type consistency.** `--tali-note-w`, `--tali-note-gap`, `--tali-bleed` and
`--tali-prose-cols` are introduced in Task 2 and used with those exact names in Tasks 3, 4 and
7. The grid line names `text`, `bleed` and `note` are declared once in `--tali-prose-cols` and
spanned by name in Tasks 2-4. `caption_label` is defined in Task 6 Step 3 and called in Step 4
with the same `(label, num)` signature at all six sites. `JETBRAINS_MONO_ADVANCE_EM` is defined
and read only in Task 3. The `ENGAGED` comment marker in Task 4 Step 4 is what Task 4 Step 1's
breakpoint test splits on — they must stay in sync, which is why the marker is a literal
sentinel rather than prose.

**One risk worth naming.** Task 2's grid selector rests on `#tali-main` and `#tali-root` being
the two reading-region ids. That was verified against `page.rs:621,626` and `client.js:962` on
2026-08-15 and Task 2 Step 1 gates both names — but if a third mount ever appears, the grid
will silently not apply to it and the page will render as one unstyled column. The gate names
the two; it cannot know about a third.
