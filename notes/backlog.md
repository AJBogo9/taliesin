# Taliesin backlog

**Scope: corpus-plus-roadmap.** "Done" = the docs under `corpus/` render correctly (the
corpus is the regression net); each new capability ships pinned by a target corpus doc.
Output stays **HTML-only**. Roadmap: `ROADMAP.md`.

> Kept small (read often). **Only open tasks live here** — delete items once landed; don't
> leave `[x]`. Completed work is in git + `ROADMAP.md` / `native-rewrite.md` / `AUDITS.md`.

## State (2026-07-10)

v0.2.0. All four formats render + deploy;
the dev loop is strong (block-level incremental updates with DOM-state preservation, warm server +
Jupyter kernel, `_freeze` cache, Alt-click + reverse cursor sync, located diagnostics, CSS hot-swap,
Cmd-K search). Agents commit + fast-forward-merge to local main on request, and push to `origin/main`
when the author explicitly asks.

**Recently shipped** (detail in git + `ROADMAP.md` / `native-rewrite.md` / `AUDITS.md`): the native
rewrite + roadmap Waves 0-4, the reader cluster, `check`/prose-lint + `{input}`/scrolly, the `--bare`
build, the reading-first redesign, deep-audit P1/P2, the Taliesin rename, the `.tmd` editor grammar, the
legacy-format clean break (`.tmd`-only input, `deck`/`define()` the only spellings), the security-P3
batch, the VS Code companion language features, F2a cross-page hover-preview, nested-theorem numbering,
the 2026-07-07 audit batches (1-4, Batch 5 in full [high-value half + remainder], 6, 7, the Batch 8
robustness trio: watcher prune + live search index + reconnect state, and Batch 9: the freeze/kernel
honesty + resource-hygiene trio), cross-reference backlinks (a quiet per-target "Referenced by"
line — the reverse of forward xref; `site/backlinks.rs`), and the Batch 8 consolidation (the
duplicated diff-then-broadcast tail hoisted into the shared `protocol::Broadcast` helper, so the
single-doc and site dev servers can't drift on the block-level incremental invariant), and the
2026-07-08 hardening pair (byte-safe `percent_decode` so a crafted `/%<raw-utf8>` request can't panic
the handler; the active-nav highlight surviving a `#fragment`/`?query` nav href), and the audit
top-leverage #7 fix (the same-page hover link-preview card now strips the cloned block's
`data-block-id`/`data-sourcepos`/`data-source-file` via a shared `stripSourceAttrs` that also covers
the cross-page card, so the read-only preview is never a duplicate-block-id or a click-to-source target),
and the **2026-07-09 UI-audit engine batch** (findings #1/#3/#4/#5/#6/#8/#10 from
`2026-07-09-ui-audit-findings.md`, all CSS, each re-measured in-browser at 390/900/1440 with the old rule
re-injected as a counterfactual; plus finding #2, the stale-`_freeze` figure bug, fixed at its real root:
`FORMAT_VERSION` had never been bumped since it was introduced, so the `qmd-fig-*` → `tali-fig-*` rename
[`8bb0a65`] silently orphaned every cached figure. Bumping it to 3 makes the loader discard all pre-v3
entries and self-heal on the next build, so no cache file had to be deleted by hand), and the
**2026-07-09 UI-audit content batch** (findings #9/#11/#13/#14, each re-diagnosed from source rather than
from the audit's screenshots and browser-verified with the pre-fix state re-injected as a counterfactual;
plus `pca-geometry`'s previously-unknown twin of #13, which the stale `_freeze` had been masking: its
scree plot's cumulative-variance line was invisible on light, at contrast 1.00:1, leaving a labelled right
axis measuring a series that was never drawn). The same batch resolved the never-settling-pages question:
not a runaway loop but a `settle()` false negative, since a `//| name:` value cell legitimately paints no
DOM; `qmd-js.js` now stamps `data-qmd-done` when a cell's `run()` resolves and the harness gates on that
(the tempting `data-qmd-ran` is stamped *before* cells run, so it would have caused premature capture).

**2026-07-10: the polish audit's Tier-1 batches A-E all landed.** The VS Code companion runs
for the first time since the rename (its default binary was `qmd-fast`); the published site no
longer carries the author's home directory or twelve `.tmd` sources; nine CLI papercuts;
`<title>` falls back to the leading H1; a typo'd category is caught by `check`; and `check`'s
ten static validators now run in `build --strict` and `publish` (they ran nowhere else, so
`--strict` shipped a broken `<img>` with exit 0). Three drift gates were added where the bug
was *invisible* rather than merely unfixed: `editor/vscode/src/test/manifest.test.ts` (the
companion's default binary must be the `[[bin]]` name cargo builds), `paths.test.ts` (its
source-extension list must equal `ext.rs`'s), and `main.rs::env_help_lists_every_runtime_env_var`
(every `TALIESIN_*` the code reads must be in `usage()`; it found two more on first run).

Finally, the **2026-07-09 Tier-2 grind** (branch `backlog-grind-2026-07-09`): `log::escape_control` so a
crafted `source_file` on the unauthenticated preview websocket cannot write OSC/CSI/CR into the author's
terminal; `twinned_corpus_sources_stay_byte_identical`, which discovers the duplicated post pairs by
walking both roots and fails on injected drift; `debug_assert!`s pinning the block-id uniqueness that
`lcs_pairs`' LCS-to-LIS reduction silently assumes; **TypeScript and TOML now actually highlight**
(syntect's bundled set carries neither, so 30 code blocks in the project's own docs rendered as plain
text; the `two-face` extras supply them, but the **bundled set is consulted first** and the extras only
fill gaps: `two_face::syntax::extra_newlines()` is *not* a superset, it is bat's own curated set whose
Rust/Python/JS/JSON/HTML/YAML definitions emit different scope spans, so preferring it wholesale would
silently re-highlight every code block in every document. Caught in review; the first attempt did
exactly that, guarded by a test that compared `SyntaxReference::name`, which is identical for the
drifting languages. The guard now asserts provenance by pointer plus byte-equality against the bundled
set, and fails if the preference order is flipped); `validate_code_languages`, which warns on a fence whose
language resolves to nothing (the backlog's "needs an invasive warnings channel" excuse was false: the
located-diagnostics channel already existed and `highlight()` is untouched); `corpus/highlight.tmd` +
zero-dep `body_html()` snapshots for the four hermetic `{js}` docs. Six of the ten Tier-2 items scoped
for this batch turned out to be stale or misdiagnosed; the evidence is recorded inline below rather
than re-derivable only by re-reading the code.

**Working method:** branch per feature; brainstorm if there's a fork; spec under
`docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
extension harnesses); fast-forward merge locally; delete the item here. **Do-NOT-touch:** the
exec/kernel zone + the single-editing-surface invariant. Review subagents use read-only git.

**Author policy (feature-first):** finish framework features before marketing-site work.

## Needs your input (the blockers)

Nothing blocks Tier 1. Batches A-E of the 2026-07-09 polish audit **landed 2026-07-10**; the
only Tier-1 work left is **Batch F** (writing-productivity features), all of which is decided
and build-ready.

Five **rulings owed**, none blocking (all filed under "Owner-gated" below): draft-aware preview
(flips a default), reading time in the built page (reverses a decision a corpus test pins),
`publish --public` (relaxes a fail-closed gate), the built-site shared asset bundle (changes the
shape of the build output), and now **whether plain `publish` should be strict by default**
(`publish --strict` already inherits the full check-superset; making it the default is a
fail-closed change, so it was not assumed).

## Priority queue

### Tier 1 — decided, build-ready (no blocker)

**Polish / productivity audit, 2026-07-09.** Six read-only agents (CLI, authoring surface,
live preview, build+publish, editor bridge, ideation); every root cause re-derived from
source by hand, and three agent diagnoses **refuted** on measurement (recorded under
"Decided against"). Full evidence, reproductions and fix sketches:
[2026-07-09-polish-audit-findings.md](2026-07-09-polish-audit-findings.md).

**Batches A-E all LANDED 2026-07-10** (one branch each, ff-merged to local main; commits
`b2c4a5a` companion, `48b5d38` output leaks, `b5001a6` CLI papercuts, `aa7f3c5` authoring
defaults, `2bd8194` + `41c164f` + `7c75322` the confidence gap). Every claim was
re-derived from current source and adversarially refuted before any code was written; the
corrections that changed the work are recorded under "Decided against" below. What the
audit called Batch B ("the change that most makes the tool feel mature") is done in full:
`check`'s superset now has one definition, `check::page_static_diagnostics`, shared by
`check`, `build --strict` and `publish`.

**Batch F is done except TODO surfacing**, which turned out NOT to be build-ready as filed
and is now owner-gated (below). Landed 2026-07-10: did-you-mean on broken refs (`c687fcc`),
`taliesin symbols` + the completion fix (`1213bb2`), `.tmd` snippets (`d7c950c`), and
`taliesin new` (this batch). Corrections that changed the work are under "Decided against".

**Historical (landed, kept only as a pointer):** the UI-audit content batch (#9/#11/#13/#14,
plus the `pca-geometry` twin of #13) and the never-settling-pages investigation both landed
2026-07-09. The three pages were never a runaway loop: it was a false negative in the
harness's `settle()` predicate. Detail + evidence in `2026-07-09-ui-audit-findings.md`
(triage header and §7).

**One carry-over, not an open task:**
- The `showcase` 3D canvas is absent from a no-scroll full-page capture at 390px, because its
  `IntersectionObserver` never fires while the host is below the fold. A reader who scrolls
  gets it. To make the harness capture it, emulate `prefers-reduced-motion: reduce` in
  `browser.mjs` `forceTheme`; `build()` then runs synchronously. Cheap, and a UI audit arguably
  *wants* the reduced-motion rendering. The cost is that it would then never see the animated
  one. Not decided.

*(The twinned-corpus-posts carry-over LANDED 2026-07-09: `twinned_corpus_sources_stay_byte_identical`
in `crates/core/tests/corpus.rs` discovers the pairs by walking both roots and asserts byte-identity
of the authored sources, verified to fail on injected drift.)*

**Never filed from the 2026-07-09 audit (found missing on 2026-07-10 while reconciling):**
- **Shell completions** (M, med; audit §7). No completions, and no seam: the CLI is hand-rolled,
  no clap. 12 stable command names; a static bash/zsh/fish script is ~120 lines. Precedent exists
  (`schema`/`vocab` already feed *editor* autocomplete). If built, gate the command list against
  `main.rs::COMMANDS` so it cannot drift, the same way `env_help_lists_every_runtime_env_var` now
  gates the ENV block.
- **The date renders verbatim** (S, low, taste; audit §8). `render/mod.rs` emits `2026-04-14`, not
  "14 April 2026". Pure taste, no defect. (The audit's third §8 item, a *forgotten* `alt` on
  `![](x.png)` getting no nudge, it deliberately did not file: alt-less is the a11y convention for
  decorative. Leave it.)

**New, small, surfaced 2026-07-10 while building the Tier-1 batches:**
- **`og:title` and the listing card still read `Page::title`, so they disagree with `<title>`.**
  The H1-promotion fix is scoped to `<title>` (per the audit's own caution about silently
  retitling a listing). The consequence: a *website* page with no front-matter title and a
  leading `# H1` now renders a correct `<title>` but an `og:title` borrowed from the site name,
  and a listing card labelled by its rel-path. Book chapters are unaffected (`book.rs` already
  falls back to the H1). The coherent fix is to give `site/discovery.rs`'s `website_pages` the
  same H1 fallback `book.rs::push_chapter` has, so `<title>`, `og:title`, cards, nav and search
  all agree. Small, but it widens blast radius to four consumers and no corpus page exercises it
  (add a fixture first).
- **`publish` without `--strict` still deploys a site `check` would reject.** `publish --strict`
  now inherits the full superset (verified: it refuses a site with a missing image). Whether
  plain `publish` should be strict-by-default is a ruling, not a bug: it would be a fail-closed
  default change. Filed here rather than assumed.
- **`corpus/tech-blog/.claude/skills/new-post/SKILL.md` is still stale** (emits `.qmd`, says
  `quarto preview`). Unchanged by this batch; it is the workaround `taliesin new` (Batch F)
  would retire. Left alone deliberately: fixing the skill would remove the evidence for the
  feature.

**New, small, surfaced 2026-07-09 while grinding Tier 2:**
- The twinned post dirs disagree on what is **git-tracked**: `tech-blog/posts/fourier-transform/`
  tracks `thumbnail.png` (unreferenced; `image:` names the `.webp`) and the four generated `.wav`
  files, while `posts/fourier-transform/` tracks neither. The `.wav`s are written at render time by
  the post's own `{python}` cell, so tracking them is the anomaly. Harmless, but decide one way.
- `Cargo.toml:27-29` claims syntect uses a "pure-Rust regex, no oniguruma C dependency". False:
  `comrak` pulls `syntect` with default features, so `onig` is in the tree (it already was at
  `1b02564`, before `two-face`) and feature unification means the C backend wins at runtime. Either
  fix the comment or make comrak's syntect dep `default-features = false`. Not measured either way.
- `docs/internals/validation.tmd`'s check-superset table omits `validate_math`, which `check.rs`
  has run for a while. One missing row.
- `corpus/README.md:16` still describes the fourier-transform post as using an `ojs_define`
  Python->`{js}` bridge. The authoring spelling has been `define(...)` since the legacy-format
  clean break; `ojs_define` survives only in internal comments (`kernel.rs:258,655`). One line.
  (Found while retiring the stale `new-post` skill, which `taliesin new` now replaces.)
- `corpus/tech-blog/.claude/skills/new-project/SKILL.md` is still stale the same way the
  `new-post` skill was (`.qmd`, `quarto`). `taliesin new` has no `project` kind, so it was left
  alone rather than half-migrated.

**Theme colour-system follow-ups, 2026-07-09.** The colour audit itself LANDED (one owned
iron-gall accent at OKLCH H271; nine vendor hexes now banned by
`no_vendor_default_colours_remain_in_any_bundled_stylesheet`; `--tali-border-strong` for control
boundaries; opacity-dimmed text replaced by `--tali-muted`; xref underlines; print/prefers-contrast
specificity; Auto theme option). Evidence: WCAG + APCA + OKLCH + Vienot-CVD harness, every new test
mutation-checked. These are the findings that survived adversarial verification but were NOT built:

- **Bare `f` forces native fullscreen with no opt-out** (`03-focus-mode.js:80`, medium). An
  unmodified single-key shortcut both toggles focus mode and calls `requestFullscreen()`. WCAG
  2.1.4 wants a way to turn single-key shortcuts off. Fix: keep `requestFullscreen` on an explicit
  menu action, not on the `f` accelerator, and add a reader toggle to disable single-key shortcuts.
- **Settings popover never takes focus when opened** (`13-reader-menu.js:60`, medium). `openMenu()`
  unhides a panel appended at body-end and does not focus it; Esc already restores focus to the
  launcher (`:79`), so the asymmetry is the bug. On sites/books a keyboard reader must Tab the whole
  page to reach the theme controls. Fix: focus the panel's first control on open.
- **Category-filter chips expose state only visually** (`10-category-filter.js:27`, medium). A bare
  `classList.toggle('tali-cat-active')`, no `aria-pressed`, and the filtered result is never
  announced. Fix: mirror the class with `aria-pressed`, render it on the server's initial "All"
  chip, and write "Showing 4 of 12 posts" into a visually-hidden `aria-live="polite"` node.
- **Embedded deck ignores a sepia host** (`render/deck.rs:164`, medium). `hostTheme()` accepts only
  light/dark, so an `{{< embed deck.tmd >}}` in a sepia page falls back to the OS and can drop a
  dark panel into cream paper. Minimal fix: map `sepia -> light` so the deck at least matches the
  host's lightness.
- **Link preview is hover-only** (`12-link-preview.js:174`, low). `mouseover`/`mouseout` are the only
  triggers; grep finds zero `focusin` anywhere in `assets/js/`. Keyboard readers never get the
  citation/xref preview. Fix: bind `focusin`/`focusout` too, and set `aria-describedby` while open.
- **`forced-color-adjust: none` hides the current nav item** (`site.css:293` + `base.css:780`, low).
  It pins `.tali-nav-active` / `a[aria-current="page"]` to an author foreground with no author
  background, so under a High-Contrast OS theme of the opposite polarity the "you are here" marker
  becomes invisible. Only the reader-seg pressed button (which pins a matching bg+fg pair) needs the
  opt-out. Reachable only via a forced or reader-chosen theme opposite the HC polarity, hence low.
- **Deck slide-number chip is not restyled per-slide** (`deck.css:455`, low). The dark restyle is
  scoped to whole-deck `html.tali-deck-dark`, so on a `.tali-dark-bg` slide the chip reads ~2.8-3.0:1.
- **Table cells still use the 1.28:1 hairline** (`base.css:436`). `--tali-border-strong` was applied
  to controls only; whether a data table's grid is "required to understand the content" (WCAG 1.4.11)
  is a judgment call, and border-strong on every cell visibly heavies every table. Owner ruling.
- **Settings panel does not reflow at 200% text.** The content-loss half is fixed (the panel used to
  hang 72px off the left edge; it is now `box-sizing: border-box` with a `calc(100vw - 2rem)` cap),
  but at 200% text the seg buttons and the keyboard-shortcut list still overflow into a horizontal
  scroll. Needs a real reflow (stack the rows), not a token change.
- **Callout `tip` vs `important` collapse under protanopia** (dE 9.1; deutan worst pair is 17.7).
  Darkening `tip` to `#1b603b` (light) / `#124429` (sepia) lifts every dichromat pair >= 11, at the
  cost of the family's uniform weight. Owner kept the uniform family: the distinct icon shape and the
  text title already carry the meaning, so hue is never the sole cue. One-line change if wanted.
- **Deck has no sepia palette** (`deck.css`, owner call). A sepia reader gets a stark white/black
  deck. Either document decks as deliberately light/dark-only, or add the palette and teach the deck
  reader/scroll path to adopt it. (The reader menu already skips decks, so nothing is broken today.)
- **No owned typeface** (`base.css:18`, brand). Typography is 100% system stack, which the research
  named as the biggest non-colour "assembled from defaults" tell. Bundling ONE distinctive-but-
  readable face offline (the way the KaTeX fonts already ship) is a better default, not a knob. Avoid
  the display-serif cliche.

### Decided 2026-07-07 — each needs its own dedicated session
- **Quarto design-decisions catalog triage, reframed.** Branch `quarto-decisions-catalog`, commit
  `535b4e1`: 165 decisions, adversarially verified. Rule on each by "is this the right design for
  Taliesin", not "does it beat Quarto" — the same-day repositioning commit (`de3de37`) retired Quarto
  as the defining reference, so drop that framing even though the fact-checked Quarto evidence is
  still useful input. Fan the 165 into batches, each with a recommended verdict + evidence, so you
  rule, not derive.
- **Reading-first identity polish** (design judgment; overlaps deferred marketing: confirm direction
  before building). The **theme/colour half LANDED 2026-07-09**: the "templated" diagnosis was
  confirmed (four vendor blues, GitHub's syntax palette verbatim, Material's error red) and fixed, 
  one owned near-monochrome accent, light/dark/sepia cohesion, sepia's low-contrast preserved. What
  remains is layout + type: hero-as-typeset not a marketing slab; drop bordered feature-card grids;
  a `--space-1..6` scale; and the owned typeface (see the colour-system follow-ups in Tier 1).

### Tier 2 — hardening (P3)
- **Execution-cache leaks — forkserver/dir/slot trio + both follow-ups LANDED (2026-07-08); small
  remainder open** (exec/kernel Do-NOT-touch, careful). Shipped, corpus/kernel-gated tests + end-to-end
  build + live SIGINT/SIGTERM verified (forkserver subtree reaped in ~1s, no new orphan; regression test
  fails without the fix; rust-reviewer clean): (a) orphaned `multiprocessing.forkserver` subtrees
  surviving a completed `build` — the helper boots a forkserver *server* grandchild (which forks the
  kernels) that `kill_on_drop` never reached and that *ignores SIGINT*; now the helper is spawned into its
  own process group (`process_group(0)`, pgid captured at boot) and `Drop`/boot-error SIGKILLs the whole
  group (`warm_pool.rs`); (b) `Kernel::start` no longer leaks its `/tmp/tali-kernel-<uuid>` dir or the
  spawned child on error paths (`ConnDirGuard` RAII + `kill_on_drop`, `kernel.rs`); (c) warm-pool
  `in_flight` slot hardened with a `SlotReservation` RAII guard (`in_flight` moved to a `std::sync::Mutex`
  so the sync `Drop` releases the slot even on a refill panic). **Follow-ups (both LANDED 2026-07-08):**
  (1) **graceful-shutdown handler** — both dev servers now race `axum::serve` against a SIGINT/SIGTERM
  `shutdown_signal()` (not `with_graceful_shutdown`, which would hang on the persistent preview websocket)
  and `run()` does a bounded `rt.shutdown_timeout(5s)`, so a Ctrl-C'd `preview` drops the watcher/builder
  tasks that own the kernels + warm pool and runs their teardown Drops. Verified: site+SIGINT reaps the
  whole group, single-doc+SIGTERM reaps the kernel, both exit in ~1s (`serve/mod.rs`,
  `serve_site/mod.rs`, tokio `signal` feature). (2) **fork protocol de-fragilized** — the forked kernel
  child now detaches its stdout from the daemon control pipe (`os.dup2(devnull, 1)` in `_child_entry`
  before ipykernel starts), so ipykernel's startup NOTE can't corrupt the `SPAWNED <pid>` handshake;
  `fork_kernel` also skips stray non-protocol lines defensively. Verified: no more "bad fork reply" /
  cold-fallback across repeated runs + a full build. (3) **boot-diagnostic cache-hit clobber FIXED** —
  on a kernel boot failure, a run-range cell that's a valid freeze hit (in the range only because a
  DOWNSTREAM cell must run) now RESTORES its cached output instead of being overwritten by the "kernel
  unavailable" diagnostic (`exec.rs` `!has_kernel` branch; the `error` cell-state stays as an honest
  "didn't run fresh" signal). Deterministic regression test (bogus interpreter forces the boot failure,
  freeze pre-seeded; fails without the fix); rust-reviewer: ship. **STILL OPEN (do-not-touch / pre-existing):**
  R stream/stderr still leaks raw ANSI into HTML (`kernel.rs` `Output::Stream` emits `esc(text)` with no
  `strip_ansi`, do-not-touch); and a pre-existing `fork_kernel` cross-call edge (rust-reviewer, low) — if
  a fork times out but its request was queued, the daemon's later `SPAWNED <pid>` is read by the *next*
  `fork_kernel`, mis-pairing pids (liveness/SIGINT/teardown then target the wrong pid; the ZMQ-connected
  kernel is still correct). Now rare since #2 removed the main timeout trigger; the proper fix is to
  poison the daemon on any fork timeout so later `take`s cold-start.
  **REPRODUCED LIVE 2026-07-10, with a new symptom:** `kill -9` on a `preview` server (used to
  tear down browser-verification fixtures) orphaned its warm-pool forkserver subtree every time:
  8 processes reparented to `systemd --user`, holding **451 MB** (the pool preloads
  `numpy, matplotlib, torch`), plus 123 `/tmp/tali-*` dirs. Reaped by hand. **The new symptom is a
  flaky test:** `exec::tests::pooled_kernel_serves_cells_without_a_long_warming_state` failed twice
  today, both times in a full-suite run started seconds after such a `kill -9`, and never otherwise
  (0/12 in isolation, 0/6 under deliberate CPU load, and it passes on pre-change code). So the
  orphaned pool is not merely a hygiene cost: it perturbs a later test run on the same machine.
  That makes `PR_SET_PDEATHSIG` + a startup sweep of stale `/tmp/tali-*` dirs whose owner pid is
  dead worth more than "S/M, hygiene" suggested.
  **Refined 2026-07-10 (while building Batch F's `symbols`): the flake is LOAD-sensitive, not
  orphan-sensitive, and it is not one test.** Measured with no orphaned pool alive (only 157
  stale `/tmp/tali-*` dirs, no live forkserver): `exec::tests::pooled_kernel_serves_cells_without_a_long_warming_state`
  **and** `kernel::tests::kernel_executes_state_errors_and_interrupts_runaway_cell` both fail.
  Rates: **2/3** full-suite runs failed while a review subagent ran `cargo test` concurrently in
  the same checkout; **1/4** with nothing else running; **0/6** running only the bin unittests
  (`--bin taliesin`, where both tests live); **0/4** on the untouched parent commit. So a
  preceding `kill -9` is *sufficient but not necessary*: concurrent CPU load reproduces it, and
  both tests spawn a real kernel and assert on timing. The Batch-F diff cannot be the cause (it
  touches no `exec.rs`/`kernel.rs`/`warm_pool.rs`, and its new `tests/symbols_cli.rs` binary runs
  *after* the bin unittests). Fixing this likely means making the two assertions wait on a state
  signal rather than a duration, not just reaping orphans.

  **NEW (polish audit 2026-07-09), the ungraceful-death path (S/M, exec/kernel Do-NOT-touch):**
  the 2026-07-08 reaping fix **holds** (a controlled snapshot -> build -> snapshot experiment, run
  twice, once with `TALIESIN_NO_CACHE=1` forcing real cell execution, produced **zero** new
  survivors; see the refutation under "Decided against"). What is missing is any defense against
  SIGKILL / a closed terminal / a crash, which no `Drop` can catch. Measured on this machine right
  now: **2 orphaned forkserver subtrees** (one alive since 08:39, reparented to `systemd --user`,
  so its `taliesin` parent is gone) and **21 leftover `/tmp/tali-*` dirs, 16 with no live process,
  77 MB on disk**. Confirmed absent: `PR_SET_PDEATHSIG` on the warm-pool helper (grep for
  `PDEATHSIG|prctl` in `crates/server/src/` is empty; the helper already gets its own process
  group, so the signal is cheap to add), and any startup sweep of stale `/tmp/tali-warmpool-*` /
  `/tmp/tali-kernel-*` dirs whose owner pid is dead. Independently: `build.rs:926` warms the pool
  before knowing whether any page needs a kernel, and does so **even under `TALIESIN_NO_EXEC=1`**
  (neither `build.rs` nor `warm_pool.rs` consults it). That is a hygiene item, **not** a perf item:
  measured 0.25 s vs 0.27 s on a prose-only site, so the boot is off the critical path.
- **Testing / CI:** the trio here is now **one item, not three.** (a) `deny.toml` multiple-versions:
  **already done** at `1b02564` (`[bans] multiple-versions = "warn"`, lines 42-43) and CI already runs
  `cargo deny check` (`ci.yml:114`); `warn` is the correct terminal state, do NOT escalate to `deny`.
  (b) `#[serial]` + the silent-drop flake: **closed.** The silent drop was fixed at source
  (`kernel.rs:393 start_error_is_transient` + its named-error unit test); only one test spawns a kernel
  and it is the same test that mutates `TALIESIN_CELL_TIMEOUT`, so the race is theoretical and
  `serial_test` would buy nothing. (c) `body_html()` snapshots: **LANDED 2026-07-09** for the four
  hermetic `{js}` docs (`crates/core/tests/body_html_snapshots.rs`, zero-dep, `UPDATE_SNAPSHOTS=1` to
  rewrite). Deliberately not `insta`: it would be the workspace's first dev-dependency and it pulls
  `similar`, already rejected below. The `{r}` bayesian doc stays unsnapshotted, since without an
  IRkernel in CI it would only pin the "kernel unavailable" fallback.
  **`tsc`/`@ts-check` — web-client tier DONE:** `search.js` + `toc-spy.js` + `toc-sheet.js`
  (the last already carried `@ts-check` but was never in the `include`) are now `@ts-check`'d, fixed
  (159 errors → 0), and wired into `web-client/jsconfig.json` + the CI `typecheck` job alongside the
  already-gated `client.js`. **Remaining: `assets/js/*` — its own (large) pass** (~800+ errors:
  `deck.js` alone ~400, plus `qmd-js.js`/`scrolly.js`/`tabset.js`/`walkthrough.js`/`mermaid.js` + the
  16 `code-enhance/` fragments; exclude the vendored `*.min.js`). Needs its own ambient globals + a
  config that compiles the concatenated `code-enhance/` fragments as one shared script scope.
  *(Measured 2026-07-09, not estimated: a throwaway strict jsconfig over `crates/core/assets/js`
  yields **812 errors**, `deck.js` alone 402. The shared-scope claim is confirmed: compiling the
  fragments in isolation adds 12 `TS2304`s that vanish when concatenated. Needs its own session.)*
- **Security:** injected Mermaid `<script>` SRI + `crossorigin` — deferred (only the live Preview
  lazy-loads mermaid from the CDN; a static build inlines the vendored copy). Needs a hash pinned to the
  CDN build, and both `integrity` + `crossorigin` would break a non-CORS `TALIESIN_MERMAID_URL` override.
- **Deck engine (P2, deferred):** drop `fitSlide` from the resize path (needs a lazy fit-on-show
  refactor first); mobile pinch/pan + touch gestures (hard to verify without a device); thread
  `footer:`/`logo:` through both deck-page builders (no corpus deck needs one yet).
- **Perf (low):** protocol-level op-message batching (one WS message per save, not one-per-op). Still
  open, still low: the realistic worst case is an edit near the top of a long doc, where every
  downstream block emits a `SetMeta` for its shifted sourcepos (`diff.rs` `anchor_op`), so one frame
  per block. The client and server ship together, so there is no wire-compat constraint.
  *(The other two "perf" items were **refuted** on 2026-07-09 and are deleted, not deferred:*
  *(1) "visited pages never evicted from `app.pages`, unbounded growth" was a misdiagnosis. `app.pages`
  holds only lightweight `PageState` (rendered block HTML + a broadcast channel), keyed by real page
  rels, so it is bounded by the site's finite page count. The heavy resource, the warm kernels, lives
  in `ExecPool`, which is **already** an LRU capped at `MAX_WARM_PAGES = 6` with eviction tests.*
  *(2) "lazy discover-time search index": measured at 15.6-27.1 ms one-time per site (a micro-bench over
  the three real sites), against a 0.38-0.64 s warm build. It buys nothing on the build path, is
  invisible in preview, and would regress the Batch-8 live-search refresh by turning the first
  post-startup edit into a full rebuild. The backlog's `site/mod.rs:304` was drift; the call is at 311.)*
- **CLI / docs microcopy:** **closed 2026-07-09, no defect.** All seven prose sites plus the two code
  strings were re-read against `exec.rs`'s `!has_kernel` branch, including the probe that mattered
  (does the freeze-hit-restores-cache fix make any doc statement false?). None does: the prose
  describes the first-run case, which stays true, and the code strings' extra actionable detail
  (which env var to set) is worth keeping distinct from the prose. Purely stylistic; not worth a pass.
- **Audit long-tail** (`AUDITS.md`): a tens-of-MB cell output blocks ZMQ receive before the cap fires
  (`kernel.rs`, exec/kernel Do-NOT-touch).
  *(The three server-side clauses that used to sit here were verified **already fixed** on 2026-07-09
  and deleted: the combined content+theme edit does hot-swap (`protocol.rs:255-267` pushes ops and
  style independently, pinned by a test at 307-322); the initial synchronous render **is** panic-guarded
  (`serve/mod.rs:164-188` `catch_unwind`, mechanism test at 1490); and mounted sub-sites **do** route
  embedded decks (`serve_site/mod.rs:353-370` falls back to `Site::deck`). They were verbatim
  pre-2026-07-07 archive entries that had been re-copied forward.)*

### Tier 3 — deferred / demand-driven
- **Companion:** the manifest rebrand **LANDED 2026-07-10** (Tier-1 Batch A): the extension
  defaults to `taliesin`, the `qmdFast.*` namespace is gone, `vscode:prepublish` is wired, and
  `manifest.test.ts` gates the default binary against cargo's `[[bin]]` name. Still Tier 3: Phase 2
  editor commands (`.tmd`-buffer text transforms only, never preview gestures); `editor.wordWrap`
  default for `[taliesin]` (respect the global setting until prose overflow is a real complaint, then
  ship `"on"`); grammar polish (YAML-type the `#|`/`//|`/`%%|` option value; recommend the cell-language
  extensions via `.vscode/extensions.json`); **marketplace packaging hygiene** (`.vscodeignore` does not
  exclude `.vscode-test/` (**1.8 GB**), `test-fixtures/`, `scripts/`, `out/test/`, `out/e2e/`; no
  top-level `icon`/`repository`/`license`/`keywords`; `"private": true` blocks publish). Diagnostics
  are save-triggered and whole-line (`diagnostics.ts:66-68` listens on open+save, not change; `check
  --format json` carries no column), which is fine for this workflow and subsumed by the LSP direction below.
- **`.tmd` format-on-save** (open question): a source pretty-printer writing the editor buffer must
  preserve `data-sourcepos` line stability for click-to-source — brainstorm reflow-vs-risk before work.
- **Dogfood: migrate the FL-weather book to Taliesin** — a real-world Quarto→Taliesin migration +
  portability stress test; pin a reduced version under `corpus/` if it renders clean.
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic).
- **`taliesin publish`: SHIPPED 2026-07-08.** Build + shared-passcode gate
  (`functions/_middleware.js`) + `wrangler pages deploy` to Cloudflare Pages; project name
  defaults to a dir-name slug, override via `publish:` in `_site.yml` or `--project-name`. Passcode is a
  Cloudflare secret (never in git); one-way flow. Spec/plan under `docs/superpowers/`.
  Follow-ups (not built): optional `--init` wrapper for the one-time `wrangler` setup;
  email-allowlist (Cloudflare Access) mode; **`--public` / `publish.gate: false`** (polish audit
  2026-07-09): `cmd_publish` unconditionally calls `inject_gate` (`publish.rs:194`) and
  `_middleware.js:9` fails closed (503 when `PASSWORD` is unset), so a **public** blog cannot use
  `publish` at all. That is why the real blog still deploys via a side-channel `deploy` skill instead
  of the command built for it. Owner-gated below (it relaxes a fail-closed security default).
- **Interactive/explorable numerics** (`FEATURE-IDEAS.md` #62-66; none spec'd/pinned — promote with a
  corpus pin when one graduates; must NOT reintroduce a reactive VM). Highest-leverage: **#62** a
  bundled numerics/stats global for `{js}` (distributions, seeded PRNG, small dense linalg) + **#63**
  `animate`/play-tick + draggable-`point` `{{< input >}}` types. Then #64 `qmd.state` cross-re-run store,
  #65 richer `{js}` output helpers (KaTeX-typeset returns + mini table), #66 opt-in Pyodide `{python}`
  (~10 MB, no torch).
- **Wave 5** (`ROADMAP.md`): print-pdf track (paged render *of* the built HTML), docs-as-spec,
  `{glsl}` cell language, SEO completeness (sitemap/robots/JSON-LD at publish with `url:`).
  **Fold `llms.txt` + `llms-full.txt` into the SEO-completeness item** (polish audit 2026-07-09):
  the old deploy ritual generated it (`corpus/tech-blog/.claude/skills/deploy/SKILL.md:24` runs
  `generate_llms_full.py`) and the migration silently dropped the capability. The block model already
  separates clean prose from code and math (`client.js:50` proves the extraction path), so it would be
  more accurate than the Python scraper it replaces. A plain-text sidecar is the same category as
  `sitemap.xml`, not a new output format. *Pin: a `tech_blog.rs` assertion that `llms.txt` lists the
  discovered pages and `llms-full.txt` excludes drafts.* Verified absent: no `llms` hit anywhere in
  `crates/`.
- **Site-level shared bibliography + bib hygiene** (M, med-high; polish audit 2026-07-09).
  `bibliography:` is per-document only (`cite/mod.rs:42`), so a growing blog retypes keys per post and
  nothing reports an unused or duplicate entry. Allow `bibliography:` in `_site.yml`, merged under each
  page's own; add two **read-only** diagnostics over the parsed registry ("entry never cited",
  "duplicate key"). Explicitly does **not** touch the BibTeX parser/CSL formatter (Do-NOT-touch): it
  only reads parsed entries and counts citations. Keep "unused entry" info-level or `check`-only, since
  a working bib runs ahead of the prose. *Pin: a small site with a site-level bib, one entry cited from
  two pages, one uncited.*
- **Author structure panel** (M/L, high; polish audit 2026-07-09). A read-only preview sidebar: the
  heading tree with per-section word count (the dev panel already counts, `client.js:50-58`) and a badge
  per node for unresolved xref / TODO / over-goal length. Click to scroll; under the companion, move the
  editor cursor via the existing cursor sync. This is the *revision* view, not the reader TOC. Scope it
  as an annotation layer on the dev panel, not a new component, or it grows to L. *Pin:
  `corpus/layout/structure.tmd` (name already reserved by `FEATURE-IDEAS.md` #26).*
- **Session revision digest** (M, med; polish audit 2026-07-09). Surface the `BlockOp` stream the client
  already receives: a session word delta (`+340 / -180`) plus a feed of the last N ops, each
  click-to-source. Cashes the diff moat; no batch compiler has a diff to show. Honest caveat: the pin is
  behavioral (a `tools/live-edit-bench` assertion), not a corpus doc.
- **Block-level transclusion** `{{< include file.tmd#sec-id >}}` (M, med; polish audit 2026-07-09).
  Reuse a section across a series without copy-paste drift. Must ride **on top of** the `includes.rs`
  source-map pass (resolve the fragment to a block range, hand the existing machinery a sub-slice),
  never rewrite it. Hard merge gate: the source map must not perturb. Defer until a real series needs it.
- **LSP for the language intelligence, browser stays the view** (L; the audit's architecture call).
  Everything an LSP needs is already in Rust (`check`, `vocab`, `register_xref`, the bib parser,
  `closest()`), it is write-once for Neovim/Helix/Zed/VS Code, and it removes the drift that causes the
  `#| label:` completion gap (JS regexes reimplementing Rust knowledge). An LSP cannot render the
  preview and does not need to: the preview is already editor-agnostic (any browser; the sync surface is
  two `postMessage` shapes specced in `docs/internals/protocol.tmd:325-350`). The only thing binding it
  to VS Code is the hardcoded `vscode://` open scheme. Do **not** rebuild the preview as an LSP; do not
  invest further in the webview beyond Tier-1 Batch A.
- **Built-site shared asset bundle** (L, high, reader-facing; polish audit 2026-07-09). Measured on the
  built `corpus/tech-blog`: largest post **1.72 MB**; on `KL-divergence` (712 KB) the inlined `<style>`
  is 64% of the page, of which **339 KB is base64 KaTeX woff2 (48% of the page)**, and only 21% is
  content. **Seven pages carry that identical font block.** Inlining is correct for `build file.tmd`
  (portable, `file://`); for `build <dir>` a returning reader re-downloads ~97% of every page. Extract to
  content-hashed `app.<hash>.css` / `app.<hash>.js` / `katex.<hash>.css`, linked once, minify while
  there (this subsumes the "no cache-busting" and "unminified CSS/JS" findings). Distinct from the
  existing "Image optimization" item. Owner-gated below (it changes the shape of the build output).
- **Image optimization** (WebP/AVIF + `srcset` + lazy-load behind a content-hashed cache) — until posts
  get image-heavy.
- **Marketing site** (deferred, feature-first; rolls into a demo-machine rebuild): `live-edit-hero-demo`
  clip; swap `site/_site.yml` placeholders; demo-led hero rebuild (during which: a 3-viewport spot-check
  of the already-code-fixed 390px hero overflow + theme/video desync — see "Decided against" — plus any
  leftover em dashes); mobile embed refine; deploy (Cloudflare / GitHub Pages).
- **`serde_yaml` fallback watch-item:** if 0.9 ever breaks against a future serde/edition, swap to
  `serde_yaml_ng` (v0.10), gated on a test that `Error::location().line()` still works. Fix the stale
  `Cargo.toml` comment (it names the unsound `serde_yml`) when touched.

## Audit 2026-07-07 implementation queue (build-ready)

The 2026-07-07 deep audit's build-ready fixes (decided, no blocker) were worked in
**batches sized as one branch each**. Batches 1-9 all landed, including the Batch 8 consolidation
(the duplicated diff-then-broadcast tail hoisted into the shared `protocol::Broadcast` helper so the
two dev servers can't drift on the incremental invariant). Full per-item detail (repro + fix approach)
and the ~80 low-severity long tail live in [AUDITS.md](AUDITS.md) 2026-07-07. **CONFIRMED unless
marked PLAUSIBLE.** Already-tracked items (op-batching, kernel `/tmp` [`Kernel::start`] + `in_flight`
leaks, boot-diagnostic clobber, `app.pages` LRU, lazy search index, `fitSlide`, R ANSI leak, Mermaid
SRI) stay in **Tier 2** above; the audit only sharpened their exact paths in AUDITS.md.

### Cut (philosophy gate: adopt) — cleared
The `?qmd=embed` dead-mode removal landed (drop the orphaned ternary branch + stale comments;
speaker previews became snapshot clones long ago, so nothing generated the URL). The gate KEPT two
proposed cuts as **do-not-cut:** `data-level` is a live test anchor; the two `.tali-input` CSS blocks
style two different features.

### Low-severity long tail (~80 items) → [AUDITS.md](AUDITS.md) 2026-07-07
Pick up opportunistically alongside whichever batch touches the same file. Includes:
qmd-js initial pass paints in DOM order not topo order; many
citation-render edge cases; and the architecture / waste / stale-but-working-docs tail — including the
**stale-but-working `qmd-*` docs references that still have runtime aliases** (`qmd.*` cell API,
`qmd-input`/`qmd-embed`/`qmd-video`/`qmd-fnref`/`qmd-main` classes, `window.qmdEnhancers`/`QmdDeck`);
renaming those is a separate verify-each-alias pass, not a mechanical sweep.
**Do NOT "rename" these live identifiers — they are correct as-is:** `qmd-goto`/`qmd-cursor` (postMessage),
`qmd_token` (cookie), `qmd-theme` (localStorage key + `<style id>`), `qmd:themechange` (event),
`qmdFast.*` (VS Code config), `qhl-*` (highlight scope).

**Four long-tail entries were closed on 2026-07-09** and removed from the list above. Three shipped
(`diff.rs` LIS uniqueness `debug_assert!`s; the dead `ts`/`typescript`/`toml` aliases, resolved by
actually *adding* the syntaxes rather than deleting the aliases; the silent unresolved-fence
degradation, now `validate_code_languages`; and `click_block` terminal-escape injection, now
`log::escape_control`). One was **refuted**: the "include symlink-loop SIGABRT" does not exist. Linux
caps symlink traversal per path resolution at `MAXSYMLINKS = 40`, so `open()` returns `ELOOP` at depth
41 and the existing `Err(_) => drop_with_warning` at `includes.rs:148` already handles it (reproduced:
deepest successful read at 40 components, failure at 41). Its co-listed "lexical-only `safe_join`" is
likewise a non-issue: includes are author-local and never attacker-controlled. A `MAX_INCLUDE_DEPTH`
cap would be harmless defense-in-depth, not a defect fix. Do not re-scope either.

### Owner-gated: do NOT build without your ruling
- **Scroll-vs-shrink for embedded media (UI-audit #7 + #12), CONFIRMED but a design choice, not a defect.**
  `base.css:365-367` states the intent outright: embedded media (`canvas, svg, video, iframe`) is "clamped
  to the page width so a fixed-size canvas can't force a horizontal scroll on mobile". The scroll-not-shrink
  treatment (`overflow-x: auto` + scroll shadow) is deliberately reserved for *text* content: `pre`, `table`,
  and now `.katex-display`. The consequence is that a wide **mermaid diagram** (#7) shrinks its `foreignObject`
  labels to ~5.8px at 390px, and the **features.html demo video** (#12) downscales baked-in desktop text ~3x.
  The ruling you owe: *is a mermaid diagram text-you-must-read (→ table treatment) or embedded media (→ keep
  the clamp)?* Flipping #7 costs `pre.mermaid { overflow-x: auto }` + `pre.mermaid svg { max-width: none }`
  and trades illegible-shrink for the mobile h-scroll the rule exists to prevent. #12 has no engine fix at
  all (re-record the clip, or ship a mobile source); it is also already deferred under the marketing item.
- **Add (gate: adopt, but confirm):** shareable/deep-linkable `{{< input >}}` state via the URL fragment
  (reader-local, hydrate from `data-qmd-input`, no Rust/model change) (`qmd-js.js`); reader text-size +
  line-spacing controls (a11y-exempt per CLAUDE.md; substrate exists) (`14-reader-prefs.js`).
- **Add (deferred, need a scope/default ruling):** cross-revision block-diff "what changed" view;
  reader-facing reproducibility manifest; web-native List of Figures/Tables/Theorems; interactive data
  tables; "Cite this" export; code-line xrefs (`@lst-3:line`); theme-aware `dark=` figures.

**From Batch F, 2026-07-10** (analysis done; the ruling decides the blast radius):
- **TODO / FIXME surfacing needs a severity concept that does not exist** (was filed as
  "S, med, no ruling needed"; it is neither). `prose.rs::lint` really does return
  markdown-aware, code/math-skipping located `(line, message)` pairs, so the *scan* is small.
  The trap is "info-level": **there is no severity anywhere.** `render::Warning` has only
  `{message, file, line}`; `check::Diagnostic` the same; `protocol::Diagnostic` knows only
  `warning|error`. And the warning channel is a **hard gate**: `cmd_check` exits non-zero if
  *any* diagnostic exists (no filter), `build --strict` does `problems += doc.warnings.len()`,
  and `publish --strict` inherits the same superset. So a TODO warning that reaches
  `doc.warnings` or `page_static_diagnostics` fails `check` on every draft and blocks publish,
  which contradicts the item's own constraint. Two designs, both real:
  - **A (S, safe):** preview-only. A `todo_scan` in `prose.rs` injected at
    `serve/mod.rs::compute_diagnostics` through a new `protocol::Diagnostic::info` + a
    `.tali-diag-info` rule. It cannot reach the shared gate, by construction. TODOs appear in
    the browser preview, not as VS Code squiggles.
  - **B (L):** a real `level` threaded through `render::Warning` (public type) + `check::Diagnostic`
    + `format_json` + the `cmd_check` exit gate + **both** `build --strict` tallies + the
    hardcoded `DiagnosticSeverity.Warning` in `diagnostics.ts:59`. Miss any one of those and
    drafts start failing `--strict`. This re-plumbs the very gate Batch B just unified into one
    definition, so it deserves its own session.
  Note the scan must **not** reuse `prose::strip_inline`: it deliberately blanks code, and a
  TODO usually lives in a code comment. Pin any fixture inside `corpus/diagnostics/` (exempt
  from the clean-render guards). *Owner ruled 2026-07-10: skip for now; keep this analysis.*

**From the polish audit, 2026-07-09** (evidence in `2026-07-09-polish-audit-findings.md`):
- **Draft-aware preview (flips an established default).** `draft: true` currently hides a page from
  the site *preview* as well as the build (`site/discovery.rs:19-23`), so a half-written post cannot be
  seen among its own listings, nav and cross-refs until it is un-drafted, which is exactly when the
  author wants to see it. Proposed better default: **preview includes drafts** (quiet DRAFT badge, count
  in the dev menu), **build/publish exclude them** and print `2 drafts not published: …`. The gate: it
  flips a default and widens a discovery code path (draft-specific bugs would surface in more places).
  Related, and cheap either way: `book_pages` (`book.rs:172`) never reads `fm.draft`, so a book chapter
  cannot be a draft at all.
- **Reading time in the built page (reverses a deliberate decision).** Word count + reading time are
  computed but trapped in the author-facing dev panel (`client.js:50-58`), and `corpus.rs:530-533`
  **pins their absence** from the built page on purpose. Promoting them is a reader-facing flip of that
  ruling, not a bug fix. (An author-facing `goal: 1500w` over/under diagnostic would be a new knob, and
  needs its own justification.)
- **`taliesin publish --public`** (relaxes the fail-closed passcode gate). See the `publish` item above.
- **Built-site shared asset bundle** (changes the shape of the build output). See Tier 3.

## Decided against / do-not-re-litigate

**REFUTED / CORRECTED while building the Tier-1 batches, 2026-07-10.** Each was verified
against source or by measurement before the code was written. Do NOT re-scope them.

- **`--version -dirty` (audit item, Batch D): NOT worth building.** `crates/server/build.rs`
  declares `cargo:rerun-if-changed`, so cargo runs it exactly **once** and never again when
  the working tree becomes dirty. Proved with a side-effect log: 1 run across a dirtied
  `log.rs` and a dirtied `crates/core/src/lib.rs`. A `-dirty` marker computed there is
  therefore stale, i.e. worse than absent. The `rerun-if-changed=<nonexistent path>` escape
  does force a rerun every build and *is* correct, but it costs **0.85 s on every warm
  build** (0.11 s -> 0.96 s, n=3), and the `taliesin` launcher rebuilds on every invocation,
  so every `taliesin preview` would pay it. Refused: a cosmetic marker is not worth the dev
  loop.
- **Batch F's did-you-mean premise was half false** (corrected while building it, LANDED).
  "The candidate set (registered anchors; parsed bib keys) is in hand at warn time" holds
  for citations only. The two warnings are emitted at *different* sites: a broken citation
  warns in `cite/render.rs`, where the `Bibliography` is a parameter; a broken cross-reference
  warns in `cite/validate.rs::validate_xrefs(blocks)`, whose **only** input is the blocks, the
  render-time anchor registry having been dropped by then. The shipped fix reads the anchor
  vocabulary back off the rendered HTML (each target still carries ` id="fig-x"`), so no
  signature changed and the four `validate_xrefs` callers were untouched. Two guards, both
  mutation-checked: a suggestion must share the broken ref's **kind prefix** (a `@fig-` must
  never resolve to a section), and matching runs on the **stem** after that prefix, so the
  shared `fig-` cannot pad a short name past the five-char floor. The ` id="` scan is
  space-anchored because `data-block-id="` also ends in `id="`.
- **Batch F's `symbols` counts were wrong, and its bib half was unnecessary** (LANDED).
  The filed "34 cell-labeled vs 43 brace-anchored, ~44% invisible" understated the gap and
  measured the wrong thing. Real counts over `corpus/` + `docs/`: **44** cell-labeled
  `fig-`/`tbl-`/`lst-` targets, 43 brace anchors, of which only **24** carry an xref kind
  prefix at all. Sharper still: **11 of the 18 docs that use cell labels have no brace xref
  anchor whatsoever**, so `@`-completion offered them *nothing* (`corpus/posts/pca-geometry`
  has 7 targets, all cell-labeled). The **bib-key half was dropped**: `complete.ts`'s
  `harvestBibKeys` + `frontmatterBibPaths` already work, so emitting keys from Rust would
  have added a public `Bibliography::keys()` for zero live bug. Shipped the xref half only.
  Two facts that shaped the design: a **parse-only render already registers cell labels**
  (verified: a `#| label: fig-scree` python cell resolves with `TALIESIN_PYTHON=/nonexistent`
  in 0.19 s), so `symbols` needs no kernel and `RenderedDoc::xref_numbers` alone is the
  complete inventory (no three-source union); and `symbols` reads **disk** while the editor
  buffer may be dirty, so `completions.ts` **merges** the CLI's symbols with the live regex
  scan rather than replacing it. Also landed: three drift gates
  (`every_dispatched_command_is_listed_in_commands`, `subcommand_help_covers_documented_commands`
  now driven by `COMMANDS`, and a cross-language `manifest.test.ts` gate that every subcommand
  the extension spawns is in `main.rs::COMMANDS`). **The first draft of two of those gates could
  not fail** (the Rust one was line-based, so a rustfmt-wrapped `Some(\n "a" | "b",\n) =>` arm
  collected nothing; the TS one matched `spawn(\s*\w+\s*,`, so `spawn(binaryPath(), …)` was
  skipped in silence). Both are now mutation-checked against exactly those shapes. **Gate the
  gate**: a drift test that cannot fail is worse than none.
- **Batch F's snippet volume numbers were inflated ~5x** (LANDED, harmless). Filed as "520
  fenced-div openers"; the real count over `corpus/` + `docs/` is **107 openers** (the 520
  counted every `:::` line, and a div's closer is a bare `:::`). Likewise 176 code cells (not
  184) and 57 callouts (not 64). "108 front-matter blocks" was the `.tmd` file count. The
  feature was still worth it; the justification was just softer than filed. Twelve snippets
  ship (`fm`, `cell`, `figcell`, `jscell`, `foldcell`, `callout`, `fig`, `tabset`, `thm`,
  `include`, `video`, `input`), each verified end to end by expanding its body and running
  `taliesin check` on it: **12/12 clean.** Drift is held by four mutation-checked gates in
  `manifest.test.ts`, the load-bearing one being reverse parity (the callout snippet's choice
  list must equal `vocab.calloutKinds` exactly, in order, so adding a kind in Rust fails the
  build until the snippet follows). A fifth asserts `.vscodeignore` cannot exclude `snippets/`,
  since an excluded dir would ship an extension with no snippets and no test would notice.
- **The audit's C1 fix was wrong.** It proposed labelling an include relative to the *project
  root* (`containment_root`). That would have broken click-to-source for **every** include:
  both consumers resolve `data-source-file` against the *primary document's own directory*
  (`resolveSourceFile` = `path.resolve(dirname(doc), label)`; the reverse-sync key is
  `path.relative(dirname(doc), file)`). The shipped fix computes a true primary-doc-relative
  path, which also silently repaired reverse sync into `../../_includes/three-scene.tmd`.
- **The audit's E2 symptom was fabricated.** `<title>notitle</title>` appears nowhere;
  `grep -rn notitle crates/ web-client/` is empty. The real behaviours were worse and
  *different per path*: standalone emitted the **file stem**, a site page emitted
  **`<title></title>`** and `og:title` then borrowed the site's own name.
- **And the obvious E2 fix regressed the corpus.** Promoting the leading H1 into
  `RenderedDoc::title` retitled `demo-book/methods.html` from "Methodology" (an authored
  `_site.yml` `chapters: text:` override) to "Methods" (the file's H1). Title precedence is
  now: front-matter `title:` > a site's authored page title > the leading H1 > the file stem,
  with the middle two swapping on `in_site` (standalone, the fallback is only a filename).
- **Adversarial verification is not infallible.** A refuter "killed" audit item A3 (stale
  `qmd-fast` branding in the companion README) by grepping the **root** `README.md` instead of
  `editor/vscode/README.md`. The claim was true, and worse than filed: that README also
  asserted the preview "still works on `.qmd` files, the renderer accepts them", which the
  legacy-format clean break had made false. Check *which file* a refutation actually read.
- **A4 was overstated.** The stale `taliesin-companion.vsix` is **not committed**: `.gitignore`
  lists `*.vsix` and `git ls-files` does not track it. It is an untracked local build artifact,
  so "stop committing it" was a no-op. It is still a trap if installed by hand: rebuild or
  delete it. (`vscode:prepublish` now prevents `vsce package` from shipping a stale `out/`.)

**REFUTED by measurement (polish audit, 2026-07-09): do NOT re-scope these.** Three plausible
diagnoses died on measurement; two of them were confidently asserted by audit agents. Recorded because
the symptom-vs-cause trap has bitten this project before.
- **"`build` leaks forkserver subtrees."** FALSE. Controlled experiment (snapshot pids -> build ->
  wait 3 s -> snapshot), run twice, once with `TALIESIN_NO_CACHE=1` forcing real cell execution:
  **zero** new survivors both times. The 2026-07-08 process-group reaping fix holds on the graceful
  path. The real gap is the *ungraceful* path (no `PDEATHSIG`, no stale-dir sweep), filed in Tier 2.
- **"The warm pool boots Python on prose-only builds, costing latency."** The boot is real (and happens
  even under `TALIESIN_NO_EXEC=1`), but the **latency claim is false**: prose-only site, 3 runs each,
  0.25 s default vs 0.27 s with `TALIESIN_NO_EXEC=1`. The boot is off the critical path. It is a
  resource-hygiene item, not a perf item. Do not file it under "Perf".
- **"Dev attributes bloat published pages."** FALSE: `data-block-id` + `data-sourcepos` +
  `data-source-file` + `data-qmd-src` total **2104 bytes on a 712 KB page = 0.29%**. Tier-1 Batch C is a
  correctness problem (an absolute path leaks; `.tmd` sources get published), **not** a weight problem.
  Do not propose stripping them for size.
- **`CLAUDE.md`'s stale-asset warning is at best imprecise.** Cargo tracks `assets/css/base.css` in
  dep-info (`target/debug/taliesin.d`), and a marker appended to it **did** appear in the freshly built
  binary. The documented claim (rebuilding only the *site* re-emits the old bundled CSS) is trivially
  true; any stronger claim that `cargo build` silently embeds stale assets was not reproducible for
  `assets/css/`. Re-verify for `assets/js/` before repeating the workaround.

**Already fixed in code — do NOT re-open as "bugs":** the 390px `page-layout: full` + `hero:` prose
overflow (`site.css` `.tali-site-main { box-sizing: border-box }`) and the theme/video desync
(`theme.rs` `syncThemeVideos()` on `qmd:themechange`) are both guarded in the current tree (the
2026-07-07 audit's "What held up" confirmed the same). The only residue is a 3-viewport spot-check,
folded into the deferred Marketing demo rebuild — not open defects.

**This session (2026-07-06):** book pager stays **bottom-only** (a top pager fights the calm column;
the Chapters drawer already gives random access). Book page-TOC: **fix in place, keep both nav
surfaces** — do NOT fold the rail into the chapter drawer (loses the always-visible scrollspy; the
"rarely used" claim is unverified). Xref graph tool: **removed** (interaction not good enough).
Focus mode stays **ephemeral** (no persistence across chapters): `requestFullscreen()` needs a user
gesture, so persistence could only restore CSS chrome-hiding and would silently drop fullscreen on nav —
a half-broken mode. Deck overview **keeps per-slide backgrounds** (documented recognizability
"fingerprint", no contrast bug today; hiding is a taste-only change — revisit only if a real deck's
overview clashes). Dev-menu + `#tali-progress` + reading-progress bar stay **three separate signals**
(orthogonal: author diagnostics / build-exec status / reader scroll-position; different corners) — and
`#tali-progress` is the exec chip, NOT a reading-progress chip (the ask's label was a misnomer); the
only real issue was the resume-pill/dev-menu overlap, now a Tier-1 fix.

**Reading-first defaults — research-validated keeps** (do NOT "fix"): serif body for long-form screen
reading (don't switch to sans); ~70ch measure `--tali-maxw: 46rem` (don't narrow); right-rail scrollspy
+ width-gated sidenotes (keep both); scroll (not pagination) book reading; system-font-only (if a serif
webfont is ever bundled, ship REAL bold/italic faces, never synthesized). *Caveat:* the competitor
framing (Stripe/Linear/Mintlify/Docusaurus/GitBook, "Bootstrap/Quarto looks dated") is unverified
judgment, not evidence.

**Library outsourcing — decided against** (each adversarially verified vs the invariants):
hayagriva/biblatex (heavy deps, only IEEE used); schemars (reopens schema↔validator drift); jsonschema
(loses source-line diagnostics); morphdom/idiomorph (reverse the 83x live-edit payload win);
similar/dissimilar (give up the block-id→LIS reduction); clap; owo-colors; slug (transliterates
non-ASCII → breaks anchors); html-escape (breaks the anti-double-escape contract); lightningcss/palette
(CSS uses native `color-mix`); IntersectionObserver/scrollspy libs; deck micro-helpers (force an offline
bundle onto every deck). The reader menu is intentionally an untrapped popover. `contents: .` has no
corpus PAGE yet (add a fixture if pinning is wanted).

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Open-source the repo + publish the site when
ready; the GitHub/install CTAs become real then. The security token gate is shipped.
