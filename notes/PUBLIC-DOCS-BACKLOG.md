# Public docs backlog

Durable state for the pre-release audit of the **four public sites**. **This file is the
handoff.** A fresh session needs this file and `notes/PUBLIC-DOCS-WORKLIST.md`, which holds
the remaining per-file items.

> **State as of 2026-08-19. The audit is CLOSED.** Everything landed on branch
> `docs/release-audit-fixes`, uncommitted. Zero open blockers, zero open findings: of the
> 23 accuracy items, 18 were fixed, 4 were already true when re-read, and 1 was skipped
> with a reason; the cut candidate is cut. `notes/PUBLIC-DOCS-WORKLIST.md` records the
> disposition of each and everything that must not be re-proposed. The three items it had
> handed back to the author are also done: the 2.27 MB of unreferenced `site/assets/` is
> deleted, the margin-column selector defect that cost every TOC-rail site page a third of
> its code width is fixed in `base.css`, and showcase's reprinted `{js}` cell has a gate.
> Read it before re-filing anything.

Scope: `site/` (taliesin.sh), `docs/guide/` (guide.taliesin.sh), `docs/internals/`
(internals.taliesin.sh), `gallery/` (gallery.taliesin.sh). 28 `.tmd` files, 5,834 lines,
measured 2026-08-19.

## The standard

> "High average utility for features. I want that every addition serves a purpose. Every
> feature is highly valuable for the user. Use this mentality for the docs and the other
> public facing documents. Make sure that the content of the documents are valuable for the
> end user and that the not so valuable items are cut. I'd rather have something simple with
> high average utility than something complicated that is bloated."

Plus the standing directive: **when a call is close, cut.** The docs are judged on two axes
at once: do they tell the truth about the tool, and are they themselves a convincing
demonstration of it.

## The structural finding

**The reference layer is accurate and current; the marketing and narrative layers have
drifted off it.** In every contradiction found, the reference page is the correct side. That
is where the effort belongs: not in re-verifying the reference pages, but in the pages
wrapped around them.

## Rules

1. **Partition by FILE, never by finding.** Two agents editing one `.tmd` clobber each
   other. Every finding for a file goes to the one agent that owns that file.
2. **Each agent removes the em/en dashes in its own files** as part of its brief. The author
   does not want them. Do not run a separate dash pass; it would conflict with everything.
   Leave dashes inside code fences alone (they are code samples).
3. **Gate-pinned pages are not delegated.** `docs/guide/reference/frontmatter.tmd`,
   `docs/guide/reference/cli.tmd` and `docs/guide/using/choosing.tmd` are read by tests; edit
   them in the main session and run the gate.
4. **Never run `cargo` in a subagent** (build-lock contention). Use the prebuilt
   `target/release/taliesin`. The orchestrator runs the gates.
5. After each wave: `./tools/gates.sh` (needs `TALIESIN_PYTHON="$PWD/.venv/bin/python"`),
   re-read the diff, then commit. One commit per wave.

## The gates that pin doc content

| Gate | Pins |
|---|---|
| `the_reference_page_documents_every_known_key` | `reference/frontmatter.tmd` covers every `KNOWN_KEYS` entry |
| `every_subcommand_has_a_row_in_the_cli_reference` | `reference/cli.tmd` table, both directions |
| `tools/portability-census.py --verify` | `using/choosing.tmd` carries the exact census numbers |
| `crates/core/tests/cross_site_links.rs` | every cross-site absolute URL resolves (all four sites, verified sound) |

**Known hole:** the `_site.yml` key table in `reference/frontmatter.tmd` has **no gate** in
the doc direction, and has drifted (see W1-06). The page-front-matter table is gated; the
project-config table is not.

---

## Wave 1: ships a falsehood to a reader — **LANDED 2026-08-19**

Highest value, smallest diffs, lowest risk. Each is verified against the code.

Verified after landing: all four `build <site> --check-only --strict` clean,
`tools/portability-census.py --verify` exit 0, `cargo test -p taliesin-core` 0 failed
across every suite. Branch `docs/release-audit-fixes`, not yet committed.

W1-09 and W1-10 (`using/theming.tmd`) are **held for Wave 3**: that chapter is a candidate
for a large cut, and rewriting prose that is about to be deleted is wasted work.

| id | file | problem | fix | verified by |
|---|---|---|---|---|
| W1-01 | `site/features.tmd:77` | "`[@key]` citations with **IEEE/CSL** bibliographies" promises a CSL engine | IEEE is the only style; drop "/CSL" | `cite/format.rs` is IEEE-only; `frontmatter.tmd:283` "There is no CSL engine" |
| W1-02 | `site/features.tmd:45-50` | "No runaway cells: a cell that **runs too long** is interrupted" | Cells are capped on **silence**, not runtime; wall-clock cap is off by default | `TALIESIN_CELL_SILENCE` default 600; `TALIESIN_CELL_TIMEOUT` unset |
| W1-03 | `site/formats.tmd:~44,~50` | promises readers a "light/dark **toggle**" twice | No toggle is emitted in a build; the reader's device decides | built HTML has `data-theme` only in the pre-paint script, no control |
| W1-04 | `site/formats.tmd:~50` | docs are "deployed in the same tree under `/docs`" | Four separate Pages projects on four domains | `tools/publish.sh` |
| W1-05 | `site/index.tmd:76-83` | "choose the output in the front matter", "Every **format**" | HTML is the only target; there is no `format:` key | CLAUDE.md; no `format` in `KNOWN_KEYS` |
| W1-06 | `docs/guide/reference/frontmatter.tmd:386,387` | `_site.yml` table still documents `image` and `head`; `head` is called "**the one** raw-injection hatch" | Delete both rows. The same page at line 71 already says `head:` "went on 2026-08-18 ... injects nothing" | neither key is in `editor/vscode/schema/tali-site.schema.json` (12 keys) |
| W1-07 | `docs/guide/using/choosing.tmd:38` | says "6.7%" where the gated census says **6.8%** | one character | `tools/portability-census.py` prints 6.8%; `--verify` only checks 6.8% *appears*, not that 6.7% is absent |
| W1-08 | `docs/guide/using/getting-started.tmd:51-70` | claims "**Every** knob the server reads from the environment, in one place" but omits `TALIESIN_RENDER_TIMEOUT` | add the row (default 30s render watchdog, `0` disables) | `crates/core/src/render/mod.rs:338` |
| W1-09 | `docs/guide/using/theming.tmd:~157-191` | reasons about "a reader who has deliberately set the page to light while their OS is dark" | Impossible: there is no such control | `reference/accessibility.tmd:78` |
| W1-10 | `docs/guide/using/theming.tmd:115-121` | credits `--tali-focus` to "tabs, **the reader menu**, and **the reader controls**" | All three were cut | `13-reader-menu.js` deleted; panel-tabset cut; `accessibility.tmd:89` "deliberately no text-size, line-spacing, or focus-mode control" |
| W1-11 | `docs/internals/extending.tmd:40-53` | "**The two extension points**" that "let you add behaviour without touching [the core]" | Shortcodes are a **closed** set; you cannot add one without editing the core. There is one extension point (client enhancers) | `SHORTCODE_NAMES = ["input","include"]`, commented "the whole CLOSED vocabulary" |

## Wave 2: the page contradicts what the reader sees

| id | file | problem | fix |
|---|---|---|---|
| W2-01 | `site/features.tmd` | `.feature-grid` wraps all 16 feature boxes and **has no CSS anywhere** in `crates/core/assets/css/`. It renders as a bare `<div>`; the grid never happens | Either style it or use `.feature-list`, which *is* styled and is what `index.tmd` uses |
| W2-02 | `site/features.tmd:120-125` | "Zero CDN ... never phone home" while the built marketing site contains `import("https://esm.sh/three@0.163.0")` 4x | The build already warns about this. Either vendor Three.js, scope the claim honestly, or drop the 3D demo |
| W2-03 | `site/showcase.tmd` | 130 of 338 lines are verbatim duplication: every demo hand-copies its cell source into a display fence, unguarded against drift | `{js}` cells cannot echo their own source, so this is a workaround for a real gap. Decide: show once, fold, or add a test that the pairs match |

## Structural decisions (author, 2026-08-19)

Three calls that change what ships, put to the author rather than taken by an agent.

1. **The marketing site collapses from four pages to two.** `features.tmd` and
   `formats.tmd` are **cut**; `index.tmd` and `showcase.tmd` survive. Between them the two
   deleted pages had 11 of 16 feature boxes restating `index.tmd`, and `formats.tmd` was
   `index.tmd`'s "three shapes" section retold at 2.5x length, section for section and
   anchor for anchor. The three facts they uniquely held (click-to-source,
   bundled-not-fetched, host-anywhere output) moved into `index.tmd` as a `.feature-list`,
   and the **"Get started" button that only `formats.tmd` carried is now the hero's primary
   action** — the site's only route to the Guide's first chapter had been below the fold on
   page three. **LANDED.**
2. **The gallery keeps its own domain.** Folding `gallery.taliesin.sh` into
   `taliesin.sh/gallery.html` would touch `tools/publish.sh`, the exhibit prefixes, the
   pre-push hook and every cross-site link, to save 33 lines of prose. Not worth the risk
   before a release. **NOT DOING.**
3. **The "never phone home" claim is scoped honestly rather than made true.** The 3D demos
   stay and keep their `esm.sh` import. The page now says what is actually true: everything
   Taliesin ships is bundled and Taliesin never fetches at runtime, and anything *you*
   `import()` from a `{js}` cell is your own network call. Vendoring Three.js would have
   added ~600 KB to two pages that are already the heaviest on the site. **LANDED.**

## Wave 3: average utility

Driven by `notes/PUBLIC-DOCS-WORKLIST.md`, which carries all 119 accuracy findings and 181
section verdicts per file. ~1,855 lines of proposed reduction across 5,834.

Applied in parallel on 2026-08-19 by six file-partitioned agents, each followed by an
independent reviewer that re-read the diff rather than trusting the editor's report:

| agent | files |
|---|---|
| showcase | `site/showcase.tmd`, `site/_includes/three-scene.tmd` |
| guide-theming-code | `using/theming.tmd`, `using/code.tmd` |
| guide-onboarding | `using/getting-started.tmd`, `guide/index.tmd`, `using/preview.tmd` |
| guide-authoring | `using/writing.tmd`, `using/interactive.tmd`, `using/recipes.tmd` |
| internals | all five `docs/internals/*.tmd` |
| reference | `troubleshooting`, `accessibility`, `cell-options`, `licensing` |

Reserved to the main session (gate-pinned or cross-page): `reference/frontmatter.tmd`,
`reference/cli.tmd`, `using/choosing.tmd`, `reference/cheatsheet.tmd`, and all of `site/`.

## Result (2026-08-19)

**5,834 lines to 4,822: 1,012 lines cut, 17%.** The marketing site is 2 pages, not 4.
(Wave 3 took it to 4,729; Wave 5 added 93 back, almost all of it reference tables and a
`doctor` section that did not exist, against 8 lines of pointer scaffolding cut.)

Verified after the last edit, all in one pass:

| check | result |
|---|---|
| `build <site> --check-only --strict` x4 | all clean |
| `tools/publish.sh --check` | 4 projects clean, 3 gallery links resolve |
| `cargo test --workspace` | 82 suites, 0 failed |
| `cargo clippy --workspace --all-targets` | clean |
| `cargo fmt --all -- --check` | clean |
| `tools/portability-census.py --verify` | pass |
| em/en dashes across all 28 files | **zero** |
| landing page + showcase in a real browser | both render, no console errors |

Two gates were **widened** rather than merely satisfied, and both were proven to fail on
reintroduction rather than passing vacuously:

- `the_landing_page_is_a_masthead_and_prose` checked `.feature-grid` on `index.tmd` alone,
  which is why `features.tmd` sat on four dead wrappers with every gate green. It now walks
  every `site/*.tmd`.
- `same_variant_three_scene_copies_stay_byte_identical` classified the extended helper by
  requiring `loadGLTF`/`frameObject`/`function rebuild(`, which made the classifier the only
  reason 43 lines of unreachable WebGL survived. It now keys on `autoRotate` alone, and the
  43 lines are gone (which also dropped a whole `esm.sh` GLTFLoader dependency from the
  built site).

## Wave 4: still open

- ~~`site/showcase.tmd` page weight.~~ **RESOLVED, measured 2026-08-19 on the built output:**
  `showcase.html` 221,792 to 79,113 bytes (-64%), and the site's `search-index.js` 39,667 to
  20,429 bytes (-48%). Cutting the two duplicated source listings did it.
- ~~**A site page with a TOC rail gave code 749px where a book page gave 960px.**~~
  **FIXED 2026-08-19**, and it was a selector defect rather than a tradeoff: `has-toc` sits
  on `body` only for a single document, so `body:not(.has-toc)` matched every site page and
  inherited the 20rem margin column past the `.tali-site-main:not(.has-toc)` exclusion that
  was meant to stop it. Nine corpus site pages were reserving a note column none of them
  used, and the bleed band paid for it. See the worklist for the measurements.
- **A tool-side smell worth a ruling, not a doc fix.** `page-layout: full` is *honored*
  (no warning) and emits `.tali-wide`, but **nothing styles that class** — the width
  override went with the card grid. So the key is live vocabulary that does nothing
  visible. Same genus as `.feature-grid`. Either restore a rule or retire the key; the doc
  now describes the honest current behaviour either way.
- **The `_site.yml` key table has no gate.** `the_reference_page_documents_every_known_key`
  covers page front matter only, which is why `head:` and `image:` sat there after being
  cut. A gate reading `editor/vscode/schema/tali-site.schema.json` would close it.

## Wave 5: the last 23 accuracy items - **LANDED 2026-08-19**

Line numbers in the worklist were stale by 1,105 lines, so every passage was re-located by
its quoted text and four findings were dropped as already true. What was fixed:

| file | fix |
|---|---|
| `crates/core/tests/cross_site_links.rs` | **The gate now checks the `#fragment` too.** It resolved `…/choosing.html#sec-speed` to a `.tmd` on disk and threw the fragment away, so a renamed heading broke the two deep links between the books with every gate green. Ids are read out of the RENDERED page via `render::tags`/`render::attrs` rather than by copying `slugify`'s rules, so no second copy can drift, and a code sample that merely *prints* `id="x"` is text rather than an anchor. Proven to fail both ways before landing: renaming `{#sec-speed}` fails it, and pointing a link at an id that exists only inside a fence fails it. Anti-vacuity pinned at 2 fragments |
| `docs/guide/_site.yml`, `docs/internals/_site.yml` | Both books were navigational dead ends: no `footer:`, no `nav:`, zero links to any other Taliesin property, and zero mentions of the Gallery anywhere in either book. Both now carry the gallery's own footer shape (Taliesin, the sibling book, Gallery, GitHub), and `guide/index.tmd` names the Gallery beside the Internals link |
| `reference/cheatsheet.tmd` | Math was absent from the whole Reference part while the page offered `@eq-name`: a Math table now carries `$…$`, `$$…$$` and `$$…$$ {#eq-x}`, with one real numbered equation proving the last row. The three live div attributes (`title=`, `collapse=`, `layout-ncol=`, all verified against the binary) were documented nowhere: they are now a note under the Divs table, with the open-vocabulary rule. Added the four highlight-only cell languages, whose real gap is that only the braced form takes `#\|` options. Callouts were described as blue/green/amber; note is `#5F5C54` grey, so all three now describe the job instead of a colour |
| `reference/cli.tmd` | `preview --port <N>` was documented in no published page while `--help` advertised it; the build flag table omitted `--no-exec` and `--format json`. `doctor` had one over-stuffed table row and no section: it now has one, with two real transcripts and the **five-step** interpreter precedence as a list (a project `.venv` outranks `TALIESIN_PYTHON`, an ancestor one does not, which the old parenthetical could not say). `NO_COLOR` joined the Environment table. `troubleshooting.tmd` now opens the kernel section with `doctor` |
| `reference/frontmatter.tmd` | The schema instruction was unfollowable: it said "save the schema somewhere" and named no path. Both paths are now named (deliberately not a raw.githubusercontent URL: the repo is private and the public flip is a fresh repo). "Page blocks (sites)" is cut |
| `using/choosing.tmd` | The page sells `--check-only` as a migration assistant; it now names the one thing it is deliberately silent about, an unknown `@`-prefix, which is exactly what a Quarto migrant carries |
| 11 files | **Code that scrolled off-screen.** The design's own budget is 84 columns (`base.css:709-713`); 12 code boxes across the four sites exceeded it, half of them hiding the trailing `#` annotation that carried the meaning. Measured in Chrome at 1440px before and after: **12 overflowing boxes to 0.** The finding's original census was stale (it claimed 8 of 13 on `cli.html`; the real number after the Wave 3 cut was 1) |

Verified after landing, in one pass, with the real output quoted in the session:

| check | result |
|---|---|
| `build <site> --check-only --strict` x4 | exit 0 on all four |
| `cargo test --workspace` (with `TALIESIN_PYTHON`) | 82 suites, 1,397 passed, 0 failed, 0 ignored |
| `tools/publish.sh --check` | exit 0, 4 projects clean, 3 gallery links resolve |
| `tools/portability-census.py --verify` | exit 0 |
| `./tools/gates.sh` | **PASSED, 12 gates, every one of them ran** |
| em/en dashes in the 26 public `.tmd` | zero, in prose and in fences alike |
| code boxes overflowing, measured in Chrome | 0 of 70 across all four sites |
| landing page + showcase in a real browser | no console errors, no `.tali-js-error`; the new `{python}` figure renders in **both** palettes |

## Deferred / decided against

- **Folding the gallery into the marketing site** (see decision 2). Do not re-propose
  before there is a reason beyond tidiness.
- **Vendoring Three.js** (see decision 3).
- **Deleting `site/_includes/three-scene.tmd` outright.** `crates/core/tests/three_scene_theme.rs`
  pins the `makeScene3D` construct across every copy it finds by walking the tree. Cutting
  the dead helpers inside it is fine; deleting the file is a separate decision.
