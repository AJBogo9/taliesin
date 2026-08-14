# Docs cut: design, 2026-08-14

Spec for reducing `docs/guide` and `docs/internals` to content that clears an explicit
importance bar, and for gating them so the result cannot rot silently.

**Location note.** The brainstorming skill's default is
`docs/superpowers/specs/`. That is wrong here: `docs/` is the published manual,
`tools/build-site.sh` composes it, and two gates lint it. Specs live in `notes/`, which is
this project's convention and which nothing publishes.

## Why now

The publish path has two blocking author decisions left (S16 hosting, S17 repo history,
both in `notes/2026-08-13-mvp-audit-backlog.md`). S16 is what makes these two books public
artifacts. They are not currently shaped for that.

## The problem, measured

| | Value |
|---|---|
| Total | 5,952 lines, 26 chapters |
| Guide | 4,249 lines, 19 pages |
| Internals | 1,703 lines, 7 pages |
| Passages of "X was removed / used to / no longer / is deliberate" | 59, across 20 of 26 chapters |
| Dated statements (`2026-0N-NN`) in prose | 28 |
| Verified stale claims | 7 (below) |

Two distinct defects, and they need different fixes.

**Defect 1: the books are partly a decision record.** They document what was built and
then cut, at length. `docs/guide/using/reading.tmd:55-105` is a catalogue of eleven
removed reader features: 36% of that chapter, 0% instruction. The prose is good, but it
answers a skeptic's question inside chapters a reader opened to learn a task.

**Defect 2: nothing gates the books for truth.** `build --check-only` validates links,
cross-references, anchors and assets. It cannot tell that a sentence describes a deleted
subsystem. Seven claims have rotted with every gate green.

### The seven stale claims

Each verified against the source on 2026-08-14, not inherited from a note.

| # | Location | Claim | Reality |
|---|---|---|---|
| 1 | `internals/architecture.tmd:273` | the `preview` / `build` / **`run`** / `lsp` / `init` subcommands | `main.rs:101` `COMMANDS` is `["build", "doctor", "lsp", "init", "new", "preview", "help"]`. `run` cut in Wave 13; `doctor` and `new` missing |
| 2 | `internals/architecture.tmd:112-135` | "One door, two server paths", a table contrasting `serve/mod.rs` (single doc, own `DocState`, own `Executor`) against `serve_site/mod.rs` | `grep -rn 'struct DocState' crates/` returns nothing. `serve/` holds `mod.rs` + `security.rs` only. The prose at `:127-135` already contradicts the table |
| 3 | `internals/extending.tmd:31` | pre-push runs `build docs/guide --check-only` | `.githooks/pre-push:94-96` runs **both** books |
| 4 | `guide/using/formats.tmd:141` | "`theme:` selects the built-in `light` (default) or `dark`" | `render/theme.rs:17` matches `"light" \| "default" \| "dark"` and pushes a retired-value warning (retired 2026-08-13). `theming.tmd:23` says so correctly, so the book contradicts itself |
| 5 | `guide/using/preview.tmd:157` | the dev menu adds a theme toggle only when the page lacks one, because "A site puts one in its navbar" | `data-tali-theme-toggle` appears only in `client.js` and `theme.rs`'s wiring script. No navbar emits one |
| 6 | `internals/execution.tmd:25` | `langs` annotated `// "python" / "r"` | `{r}` cut in Wave 6 |
| 7 | `internals/execution.tmd:249` | "R has none" (startup preambles) | same |

Claims 1, 2, 4 and 6 are enumerations a `const` already holds. That is the pattern.

## The cutoff criterion

An item stays only if it passes all three:

1. **Consequence.** Without it, does the reader fail, get it wrong, or decline to adopt?
   Not: are they less informed?
2. **Irreplaceability.** Is it faster to learn here than from `taliesin help`, an error
   message, or the source file?
3. **Decay.** Will it be false in six months with no gate to catch it?

> **Keep it if the reader makes a wrong decision without it and there is nowhere faster to
> learn it. Turn it into a pointer if the source states it better. Cut it if it mainly
> decays.**

Question 3 is what taste alone does not give you, and it is what all seven stale claims
fail. The Internals index already states the principle at line 108 ("one description that
cannot go stale, rather than two that can disagree"); its own chapters break it.

## Decisions taken

Three, by the author, on 2026-08-14:

1. **The rationale layer is cut from the books entirely.** Not concentrated into a
   "what we chose not to build" chapter. The reasoning stays in `notes/`,
   `DO-NOT-REBUILD.md` and commit messages, where it already is.
2. **Internals is written for the evaluator**, meaning a technical reader deciding whether
   the tool is serious, plus the author's future self. Not for a contributor who does not
   exist yet.
3. **The truth gate ships in this work**, not as a follow-up.

## Revised target, and an honest correction

The brainstorm estimated Internals at ~750 lines on the strength of chapter headings.
Having since read `rendering.tmd`, `block-model.tmd`, `server.tmd` and `execution.tmd` in
full, **that estimate was too aggressive and is revised up to ~1,100.** Those four
chapters are much denser than their headings suggested, and most of their content passes
the criterion: the LIS/patience-sorting reason the diff is `O(n log n)`, the
removes-before-inserts ordering rule, the 256 MB render stack and why a stack overflow is
uncatchable, the footnote definitions folded into a block's hash input, the cumulative-hash
chain seeded by interpreter identity, the `packages:` digest as the one axis the key cannot
see. None of that is reconstructible from source in less time than reading it here.

The criterion working against my own first estimate is the criterion working.

**The Guide estimate moved for the same reason.** Summing the per-chapter dispositions
below gives ~2,750, not the ~2,300 the brainstorm carried. The difference is the reference
section: `frontmatter`, `configuration`, `cell-options`, `cli` and `troubleshooting` are
lookup-shaped, and lookup-shaped content is exactly what passes question 2. It is dense per
line and read a few lines at a time, so cutting it optimizes a read-time nobody spends.

| | Now | Target | Change |
|---|---|---|---|
| Guide | 4,249 | ~2,750 | -35% |
| Internals | 1,703 | ~1,100 | -35% |
| **Total** | **5,952** | **~3,850** | **-35%** |

**So the headline is -35%, not the -49% the brainstorm projected**, and the correction is
in one direction: two thirds of what I expected to cut turned out to pass the bar on
reading it. The pages that go are still the ones that should: an entire chapter of
archaeology, two chapters that were never chapters, four stale tables, and 59 asides.

The Guide figure remains an estimate: 7 of its 18 chapters have been read in full. Firm it
per chapter during implementation and update this file if the total moves by more than 10%.

## Target shape: the Guide

19 pages to 13.

| Chapter | Now | Disposition |
|---|---|---|
| `index.tmd` | 101 | **Keep, trim.** Fold "Three things it gets right" and "The 60-second version" together (they say the same thing twice). Keep the scope callout |
| `using/getting-started.tmd` | 232 | **Keep.** Path is frozen (see hazards). Trim the env-var table's prose tail |
| `using/preview.tmd` | 230 | **Keep, trim to ~170.** Fix claim 5. Compress the "Sections" revision view (27 lines to ~6) |
| `using/writing.tmd` | 295 | **Keep, trim to ~250.** Absorb `{{< include >}}` from `shortcodes.tmd`. Cut the `.sidenote`/`.marginnote` retirement aside and the cut-lint aside |
| `using/code.tmd` | 396 | **Keep, trim to ~260.** Absorb the `echo`/`include`/`cache` explanation from `cell-options.tmd`. Its "3D graphics (Three.js/WebGL)" section duplicates `interactive.tmd`'s "Interactive 3-D with a `{js}` cell"; keep one, in `interactive.tmd` |
| `using/interactive.tmd` | 167 | **Keep, ~180.** Absorb `{{< input >}}` from `shortcodes.tmd` and the 3-D section from `code.tmd` |
| `using/formats.tmd` | 149 | **Merge** into Books & sites content within `recipes.tmd` and `configuration`. Fix claim 4 wherever the text survives |
| `using/theming.tmd` | 297 | **Keep, trim to ~180.** Keep the `--tali-*` model and the three customization routes. Cut "Which palette a reader sees is not yours to set" (rationale) |
| `using/recipes.tmd` | 333 | **Keep, trim to ~200.** Highest value per line in the book. Reduce four recipes to the two that differ most (blog, docs book) and absorb what `formats.tmd` uniquely had |
| `using/reading.tmd` | 141 | **Delete.** Keyboard + a11y (~35 lines) moves to `reference/accessibility.tmd`. The eleven-item cut catalogue goes |
| `using/choosing.tmd` | 156 | **Keep, trim to ~120.** The evaluator page. Move the 29 KB payload-shape analysis (`:84-94`) to Internals, where it belongs |
| `reference/frontmatter.tmd` | 307 | **Merge** with `configuration.tmd` into one "Every key" reference (~330 combined) |
| `reference/configuration.tmd` | 275 | merged above |
| `reference/cell-options.tmd` | 239 | **Keep as a table, ~120.** Explanation moves to `code.tmd` |
| `reference/cli.tmd` | 496 | **Keep, trim to ~260.** Cut the diagnostics-code archaeology at `:130` and `:158`, and the "Publishing & sharing" section (`:291-358`), which restates `getting-started.tmd`'s Publish section |
| `reference/troubleshooting.tmd` | 171 | **Keep.** Symptom-organized already. Cut the `TAL-*` history at `:32` |
| `reference/shortcodes.tmd` | 110 | **Delete.** Two shortcodes is not a chapter; both move to their feature chapters |
| `reference/accessibility.tmd` | 88 | **Keep** as an appendix, plus the 35 lines from `reading.tmd` |
| `reference/licensing.tmd` | 66 | **Keep, trim to ~30.** High consequence, low word count |
| **NEW: `reference/cheatsheet.tmd`** | 0 | **Add, ~90.** Every construct in one scannable table. The highest value-per-line page for a reader who will not spend time, and it does not exist today |

## Target shape: the Internals

7 pages to 5.

| Chapter | Now | Target | Disposition |
|---|---|---|---|
| `index.tmd` | 111 | ~95 | Keep the one idea + the running example. Cut "Six chapters, and that is deliberate" (`:104-111`) |
| `architecture.tmd` | 290 | ~150 | **Keep the diagrams and the re-render/execute/diff ordering prose**, which is the chapter's whole value. **Cut the three module/file-map tables (`:246-286`) and the stale two-server table (`:112-135`)**, replacing both with a pointer. Fixes claims 1 and 2. Absorb `server.tmd`'s watcher, panic isolation and binding/guards sections |
| `rendering.tmd` | 211 | ~195 | Keep nearly whole. It passes the criterion densely |
| `block-model.tmd` | 261 | ~240 | Keep nearly whole. The protocol table stays: it is an at-a-glance contract and it is stable |
| `execution.tmd` | 368 | ~330 | Keep nearly whole. Fix claims 6 and 7. Receives the payload-shape analysis from `choosing.tmd` |
| `server.tmd` | 147 | 0 | **Merge** into `architecture.tmd`. Its save-loop diagram duplicates `fig-save-flow`; "Why one server, not two" (`:108-126`) is archaeology and goes |
| `extending.tmd` | 315 | ~90 | **Cut to an appendix**: the conventions, the two seams, the enhancer contract with its example. Cut the LSP capability table, both capability-removal paragraphs (`:141-159`), and the "corpus records" rationale (`:290-302`). Fixes claim 3 |

## The truth gate

**Two tests appended to the existing `crates/core/tests/stale_docs.rs`**, whose stated
purpose is already "gates that compare shipped prose against shipped behaviour" and which
already walks both books, `site/` and `README.md`. Run by `cargo test --workspace` and
therefore by `gates.sh` and `.githooks/pre-push` with no new wiring.

The design below replaced a first draft during pre-flight, after each of that draft's three
tests was measured against the tree and found not to catch its own claim. Both survivors
were verified before execution: each fails today, on exactly the intended lines, with zero
false positives.

1. **No reader-facing doc names a `RETIRED_COMMANDS` verb as a bare backticked token.**
   Catches claim 1. Measured: 17 retired verbs, exactly 1 hit across `docs/guide`,
   `docs/internals`, `site/` and `README.md`, and it is the defect.
   *Why this shape:* the defect writes the verbs as `` `preview` / `build` / `run` ``, a
   backticked list with no `taliesin` prefix on the line, so the obvious
   `taliesin <verb>` scan misses it entirely (and matches 12 false positives on prose like
   "taliesin renders").
2. **No reader-facing doc presents a retired built-in mode as a `theme:` value.** Catches
   claim 4. Measured: 2 hits, both on the one stale line.
   *Why this cannot be a register lookup:* `theme:` is a **live** key whose three built-in
   **values** were retired, and all three retirement registers key on the key. `RETIRED_KEYS`
   structurally cannot hold this, which is precisely how the existing
   `shipped_docs_do_not_use_a_retired_front_matter_key` stayed green over it.

**Scope.** Reader-facing docs only. `CLAUDE.md` and `LICENSE-OUTPUT-EXCEPTION.md` are
excluded because both legitimately name cut verbs to explain their removal (measured: 6
mentions, all correct, one of them an editorial correction whose point is that those words
are *not* commands).

**Claims 2, 3, 5, 6 and 7 are not mechanically catchable** and are fixed by hand. Claim 6
joined that list during pre-flight: it is a Rust string literal inside a ```` ```rust ````
fence, and claim 7 is the bare word "R" in prose. A cell-language test was drafted, refined
until it had no false positives, and then **dropped because it matched nothing at all** and
would have shipped green. A gate that is green before the fix asserts nothing, which is the
vacuity `stale_docs.rs` already guards against elsewhere.

**This is a drift gate in the project's existing idiom** (`gate_script.rs`,
`retired_names.rs`, `stale_docs.rs`), not a new mechanism, and it needs no production
change: `taliesin-server` is binary-only, so both registers are read as text exactly as
`gate_script.rs` reads its own.

## Hazards

1. **Gate 5 pins one docs URL.** `tools/build-site.sh` verifies every `site/` cross-project
   link resolves against the composed output. Those links are `docs/guide/`,
   `docs/internals/` and **`docs/guide/using/getting-started.html`**. That path must
   survive, or `site/` is updated in the same commit. A rename with a green
   `cargo test` still turns gate 5 red.
2. **The census gate does not fire.** Gate 11 asserts figures in `README.md` and
   `docs/guide/using/choosing.tmd`, and it keys on `corpus/**/*.tmd`, not on `docs/`.
   This change adds and removes no corpus document, so the census is untouched. `choosing.tmd`
   is edited, so **the six per-family `| n | share |` rows and the four totals in it must
   survive the trim byte-identical**, or gate 11 goes red for a different reason.
3. **A deleted chapter takes its cross-references with it.** `docs/` chapters link to each
   other by relative `.tmd` path. Deleting `reading.tmd`, `shortcodes.tmd`, `formats.tmd`
   and `server.tmd` breaks every inbound link. `build --check-only` catches these, which is
   what it is for, so run it per chapter rather than at the end.
4. **`_site.yml` chapter lists are the source of order.** Both books' `_site.yml` enumerate
   chapters. A deletion that misses the config leaves a dangling entry.
5. **`docs/internals` was linted by nothing until 2026-08-13.** Both books are now in the
   hook and in `gates.sh`. Do not assume the internals book is as exercised as the guide.

## Verification

Per chapter, not at the end:

```sh
cargo run -q -p taliesin-server -- build docs/guide     --check-only --no-exec
cargo run -q -p taliesin-server -- build docs/internals --check-only --no-exec
```

Before the commit, all of it:

```sh
TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh
```

Green means the script's own verdict line reports every gate ran. Take the count from that
line, never from prose.

A read-through of both books end to end is also owed, because no gate measures whether the
result reads well, which is the actual goal.

## Out of scope

- S16 and S17. Author decisions, unaffected by this.
- The marketing site (`site/`), except a link update if hazard 1 forces one.
- `corpus/`, `README.md`, `CLAUDE.md`.
- Adding any documentation for a feature that does not exist. The cut removes; the only
  addition is the cheat sheet, which documents constructs that already ship.
