# 2026-08-04: visual minimalism pass — Task 17 verification findings

Task 17 makes no code changes. This is the full-gate + three-viewport check that
decides whether the pass (branch `visual-minimalism-pass`, 25 commits on top of
`main` plus 2 unrelated SVG-only commits from a parallel session) is done.

**Verdict: DONE.** Every gate ran and passed, the feature catalogue matches
reality, all four real sites build clean, nothing grew, and the browser check
found the on-page TOC fix (the last commit on the branch) actually works. One
number in the task brief's own baseline turned out to be wrong; see below.

## What was cut

Reader-facing chrome and vocabulary, in commit order:
figure lightbox + its browser canary; reading-position resume + Continue-reading
pill; the mobile floating Contents pill; TOC read checkmarks; hover
cross-reference cards + the hover index; heading/figure anchor copy-links;
video hover-play (native `controls` only); the reader show/hide-code toggle;
Referenced-by backlinks + the sentence splitter; listing category filter chips
+ the linter; the per-chapter outline disclosures; the topbar download button;
callout kinds five→three; four margin-note spellings collapsed to
`column-margin` only; theorem kinds eight→five (kept `exm-`/`prp-`/`rem-` as
xref prefixes only); the `?`/`/` a11y shortcuts and their WCAG off-switch.
Plus a late fix: the on-page TOC on a **site** page (not a book) rendered
below the whole article at narrow widths (`b379e767`) — the single-document
CSS got the narrow-width lift, the site layout's own `has-toc` block didn't.

## Gate run

```
env TALIESIN_PYTHON=/home/bogo/.local/share/qmd-venv/bin/python ./tools/gates.sh
```
(Plain `./tools/gates.sh` refused to run: default `python3` has no `ipykernel`.
`TALIESIN_R` was not needed — system R already has `IRkernel`.)

**Every gate ran and passed.** Final summary:
```
════ gates ════
  pass     cargo fmt --check
  pass     cargo clippy -D warnings
  pass     cargo test --workspace (all four gates)
  pass     tsc: web-client
  pass     tsc: bundled assets JS
  pass     node --test: publish passcode
  pass     VS Code companion
  pass     cargo audit
  pass     cargo deny check

PASSED — every gate ran and passed.
```
2,202 tests passed across 125 test binaries, **zero ignored** (verified by
grepping every `test result:` line in the log — all 125 say `0 ignored`, none
say otherwise). No `FAIL` anywhere in the 4,123-line log.

All 9 pinned canaries printed `... ok` by name (kernel, R, node, and the four
chrome-backed ones: `read_run_js_reports_svg_produced_and_error_kinds`,
`a_glsl_cell_compiles_and_paints`,
`pdf_paginates_a_real_document_into_more_than_one_page`,
`a_pyodide_cell_boots_and_publishes_to_a_js_consumer`), plus the two
pyodide-cargo-feature canaries. **The browser-canary count is confirmed FOUR,
not five**: `crates/core/tests/gate_script.rs`'s own comment says so directly
— "the figure lightbox's was the tenth until the visual minimalism pass
deleted it" — and asserts the script names exactly 9 canaries total (down
from 10), of which exactly 4 carry a `chrome:` tag in gates.sh's per-canary
check. The VS Code companion schema gate (the one `cargo test --workspace`
cannot catch) also passed — no drift found there.

Nothing was skipped. This is a genuinely green `./tools/gates.sh` run, not a
`cargo test --workspace` standing in for it.

## Feature catalogue vs. reality

`./target/release/taliesin features . --format json` (190 documents scanned):

| group | count |
|---|---|
| callout kinds | **3** (note, tip, warning) |
| theorem kinds | **5** (corollary, definition, lemma, proof, theorem) |
| cross-reference kinds | 12 (includes `exm`, `prp`, `rem` — deliberately retained as xref prefixes per the brief, even though their div/theorem kinds are gone) |
| div classes | 13 — only `column-margin` present; no `sidenote`/`marginnote`/`aside` aliases |

Matches every number the brief asked me to cross-check. `RETIRED_DIV_CLASSES`
in `crates/core/src/render/validate.rs` carries the three retired margin
aliases, so a leftover `.sidenote` gets a did-you-mean instead of silent
layout loss.

## Build check — all four real projects

All four built clean (exit 0), env `TALIESIN_PYTHON=` the working venv:

| project | pages | notes |
|---|---|---|
| `docs/guide` | 23 | 2 decks, 2 assets, zip 2166 KB |
| `docs/internals` | 15 | zip 1639 KB |
| `site` | mounts `gallery/graphics3d` (5) + `gallery/analyst` (2) recursively | zip n/a (mounted) |
| `corpus/tech-blog` | 17 | 26 assets, 4 images optimized to 12 AVIF (513 KB saved) |

Only pre-existing warnings (unbundled `esm.sh` three.js CDN refs, external
link warnings) — no errors, no regressions from this pass. **Without**
`TALIESIN_PYTHON` set, `docs/guide` actually **fails to build** (exit 1: "no
python kernel available, but this document has python cells") — that's a
pre-existing environment requirement unrelated to this pass, not a defect in
it; noted so the number below isn't mistaken for a broken build.

### Size deltas — the point is that nothing grew

I rebuilt the merge-base commit (`9a22c78d`) in an isolated `git worktree` with
the *identical* build command and interpreter, so this is an apples-to-apples
before/after rather than trusting a number recorded on a different day in a
possibly-different environment:

| asset | before (merge-base) | after (HEAD) | delta |
|---|---|---|---|
| `app.css` | 70,087 B | 59,222 B | **-10,865 B (-15.5%)** |
| `app.js` | 89,148 B | 46,497 B | **-42,651 B (-47.8%)** |
| `docs/guide/using/writing.html` | 120,215 B | 115,030 B | **-5,185 B (-4.3%)** |

**The task brief's recorded baseline for `writing.html` (69,727 B) is wrong.**
My independently-rebuilt merge-base commit, same build command, same
interpreter, same `--out` mode, produces 120,215 B for that exact file — not
69,727 B. The `app.css`/`app.js` baselines the brief gave (70,087 B /
89,148 B) matched my re-measurement *exactly*, so the build pipeline and
environment are not the problem; only the one recorded page-size number is.
I don't know how 69,727 B was produced (possibly a different flag, a
different file revision, or a transcription slip), and didn't chase it
further since the artifact itself is reproducible and the real comparison —
old commit vs. new commit, identically built — is what matters. Under that
real comparison, `writing.html` **shrank**, consistent with everything else:
nothing grew.

## Line count — measured from git, not the plan's estimate

```
git diff --shortstat $(git merge-base main HEAD) HEAD
# 126 files changed, 3628 insertions(+), 6432 deletions(-)
```

That total includes two commits from a parallel session
(`fbfd565f` "remove baseline from bell curve", `1009403f` "remove bottom
line" — both SVG-only, `corpus/tech-blog/bell-curve.svg` and `logo.svg`, 6
deletions total, no other commit touches either file). Excluding those two:

**124 files changed, 3,628 insertions(+), 6,426 deletions(-), net -2,798
lines.** The plan estimated 2,571 — the real cut is **higher**, continuing
every prior task's pattern of the plan's file-list undercounting.

Breakdown (each row is disjoint by path/extension; the five rows sum exactly
to the total above — 76+22+12+2+0+12 = 124 files, 1673+1852+85+4+0+14 = 3628
insertions, 5865+240+110+4+0+207 = 6426 deletions):

| category | files | + | − | net |
|---|---|---|---|---|
| code (`*.rs` `*.js` `*.css`) | 76 | 1,673 | 5,865 | **-4,192** |
| — of which `*.rs` only | 55 | 1,588 | 3,752 | -2,164 |
| docs (`docs/**/*.tmd`, dogfooded manual) | 22 | 1,852 | 240 | **+1,612** |
| corpus (pass-only, excl. the 2 SVG commits) | 12 | 85 | 110 | -25 |
| editor/vscode | 2 | 4 | 4 | 0 |
| notes/ | 0 | 0 | 0 | 0 |
| misc (schema, vocab, AGENTS.md, gates.sh, tools/, Cargo.toml, samples) | 12 | 14 | 207 | -193 |

Docs grew (+1,612 net) from `b584269f` "reconcile the manual with the visual
minimalism pass" — expected: removing a construct means updating the guide
prose that described it, not just deleting code. Everything code-shaped
shrank hard, especially JS: `web-client/toc-sheet.js` (-184) and
`web-client/toc-spy.js` (-75) were deleted outright, `client.js` lost 154
lines, and the bundled `code-enhance/` fragments (11-lightbox.js,
12-link-preview.js, 13-reader-menu.js, 19-book-outline.js, etc.) account for
most of the rest.

## Browser check — three viewports + a tech-blog post

Chrome MCP profile: found and killed an orphaned Chrome instance holding
`~/.cache/chrome-devtools-mcp/chrome-profile` (PID 2279600, launched by an
earlier session) before starting; `new_page` worked cleanly afterward.

**Caught my own stale-binary trap first**: the release binary on disk was
built at 06:28, nine minutes *before* the branch's last commit (`b379e767`,
06:37 — the on-page-TOC fix). I ran the first check against that stale
binary and it showed the *pre-fix* bug (TOC below the article, `order: 0`,
`position: sticky`). Rebuilt (`cargo build --release -p taliesin-server`),
restarted both preview servers, and re-checked — this matches the CLAUDE.md
warning about `include_str!`-bundled assets needing a rebuild before they
show up, generalized to "the release binary can predate HEAD even when `git
status` is clean."

### `docs/guide` (book layout) — `using/writing.html`

At **390×844** (mobile), **1440×900** (laptop landscape), and **900×1440**
(laptop portrait, the forgotten band — this band sits *under* the 60rem/960px
CSS breakpoint, so it correctly gets the narrow-width single-column
treatment, same as mobile):

- Prose renders correctly at all three; reading column width and margin-note
  collapse behavior look right.
- Topbar: exactly `Chapters` button, brand link, `Search` button, `Settings`
  button — nothing else, confirmed via accessibility snapshot (`banner` node
  has exactly 4 children) at all three widths.
- Settings popover: **Theme only** (Auto/Light/Dark), confirmed via snapshot
  and screenshot.
- Code blocks kept their hover Copy button (clicked one, no error).
- Cmd-K opened and listed the command palette + outline as expected.
- **No hover-card DOM node exists at all** (`document.querySelectorAll` for
  `.tali-hovercard`, `.hover-card`, `.tali-xref-card`, `[role="tooltip"]`
  all return 0) — hovering a citation link produces nothing, at every
  viewport.
- No `#` appears near a heading on hover (screenshot-verified).
- No clickable figure/image found (`figCount: 2, clickableCount: 0` via
  script — neither figure has an onclick or pointer cursor).
- Console: clean at every viewport except one **pre-existing, unrelated**
  DevTools "issue" (not a JS error) — "A form field element should have an
  id or name attribute", pointing at the Cmd-K search `<input>`, which
  predates this pass and isn't part of its scope.

### `corpus/tech-blog` post (`posts/fourier-transform/`, site layout)

Site layout differs from the book layout (own topbar with real nav links,
own `#TOC`/`has-toc` grid) — checked separately per the brief, because the
late TOC fix was specific to this layout.

- **At 390px, the on-page TOC renders ABOVE the article** — confirmed both by
  screenshot (the four heading links appear before the H1) and by computed
  style (`order: -1`, `position: static` on `#TOC`, vs. `order: 0`,
  `position: sticky` on the stale pre-rebuild binary). This was the real
  regression the brief called out, and it is fixed on HEAD.
- At **900×1440** (the forgotten band, still under the 960px breakpoint), the
  post gets the same narrow-width TOC-above-article treatment — correct,
  since the breakpoint is width-based, not a device-class heuristic.
- At **1440×900**, the TOC renders as a normal sticky right-hand rail next to
  the reading column, as expected above the breakpoint.
- Same hover-card/heading-hash/figure-click/copy-button/Cmd-K checks as
  above, all clean, at all three viewports.
- Console: clean except the same pre-existing search-input a11y issue.

## Anything else that surprised me

- The stale-binary trap above cost real time and would have shipped a false
  "regression confirmed" if I'd trusted the first check.
- `docs/guide` genuinely does not build without a working Python kernel
  (exit 1) — a `_freeze/` cache alone isn't enough once *any* upstream
  content changes the cumulative hash. Not a defect in this pass, but worth
  knowing before trusting a "build succeeded" claim on this repo.
- Rebuilding the merge-base commit surfaced one real (pre-existing, unrelated
  to this pass) exception in `using/code.tmd`'s cell at line 166 when forced
  to actually re-execute rather than restore from `_freeze/` — did not chase
  it since it's outside Task 17's scope and did not affect any HEAD build.

## What did NOT run / was skipped

Nothing. `./tools/gates.sh` needed one environment fix (`TALIESIN_PYTHON`
pointed at the venv with `ipykernel`, since the default `python3` lacks it)
but then ran to completion with every `TALIESIN_REQUIRE_*` armed and every
gate passing — no `--allow-missing`, no silent skip.
