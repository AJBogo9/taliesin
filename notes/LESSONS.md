# Method lessons and probe traps

Extracted from [backlog.md](backlog.md) on 2026-07-27, when that file was cut back to open work only.
This is the transferable residue of the audit rounds and of the 2026-07-18..27 mutation campaign:
rules that each cost a session to learn and that a green suite will not re-teach. **Findings and
history live in [AUDITS.md](AUDITS.md) and the dated docs; this file is method only.**

Three of these rules corrected a rule the backlog previously *stated*, so read them before trusting
a remembered version of one.

## Writing a test that actually pins

The four rules that governed the whole pin campaign (items 58/59/61 paid for all four):

1. **Verify each pin by *mutation*:** restore the mutant by hand, watch the *named* test fail, then
   restore the fix. A pin never seen to fail is not a pin. **Revert by inverse edit, never
   `git checkout`** — it restores from HEAD and silently deleted the new tests once, making two
   verifications vacuously green. Assert your mutation anchor matches **exactly once**: one that
   matched twice reported "skip" and tested nothing.
2. **Cover two axes, not one.** A sweep over *well-formed* input pins **spans**; most survivors are
   guards that **reject**, and weakening one does not move a boundary, it makes the code *accept
   nonsense* (an empty key, an empty id, a construct read one character past the line end).
   **Malformed input is a separate axis**, and the first pass at item 58 skipped it and killed only
   half. It recurred in every later batch.
3. **Suspect a test that looks like coverage.** Three fully line-covered holes, three shapes:
   a helper that drove the real path and *discarded its result* (`let _ = recv()` on the LSP
   handshake — a server advertising **nothing** passed the entire suite); a fixture that *could not
   reach the assertion* (a `.hidden` dotfile in a filter that only emits `.tmd`); an assertion
   satisfied by furniture (`md.contains("title")` matched the `` `title:` `` header the hover prints
   *above* the description, so the lookup could return any other key's prose and pass).
4. **Never write a test for a timeout**, and expect some survivors to be **unkillable**. 65 of 65
   campaign timeouts are cursor arithmetic in a scan loop (mostly `+=`→`*=`), where a stalled loop
   spins instead of returning a wrong answer, so the hang *is* the detection. **But that presupposes
   a fixture that enters the loop:** a `*=` cursor mutant in the *missed* column means nothing
   executes that loop at all, which is the strictly worse finding. Some mutants are proven equivalent
   or `cfg`-dead; `cargo-mutants` does not evaluate `cfg`, so a `cfg`-dead one **reappears on every
   run**. Prove, record, move on — do not chase a score.

**A media query adds NO specificity, so a capability fix can lose without ever being reordered.**
MOB-4's fix is `@media (hover: none) { .tali-copy { opacity: 1 } }` at (0,1,0) in `base.css`; the
deck declares `.tali-deck .tali-copy { opacity: 0 }` at (0,2,0), so on every deck the button stayed
invisible on touch for a full day after the "fix" shipped. Two consequences worth carrying:
**(a)** an override must *match or beat* the specificity of the declaration it targets, and only if
it merely *matches* does source order then decide — which is why one half of the deck fix had to sit
below `:970` while the `min-width` half could sit anywhere (a different property is not a cascade
contest at all). **(b)** `hover_revealed_copy_controls_stay_reachable_without_a_hover` asserts source
ORDER *within `base.css`* and is structurally blind to a higher-specificity override in another file.
**A file-scoped order assertion cannot express a cross-file specificity fact**, so it passed green
throughout.

**A private helper's name count measures whether it is called directly, never whether it is tested.**
Two items ("this function has NO test at all") were wrong the same way: the function was reached
through its wrappers. What was actually missing was a fixture *shape*.

## Running cargo-mutants

- **Any test-command narrowing fabricates MISSED, and `--lib` is only the loudest case.** Scoping to
  `--lib` measured 102 MISSED / 0 CAUGHT, all artefact. **Scoping to a PACKAGE does the same thing
  and looks far more reasonable:** `-p taliesin-core` is cargo-mutants' *default* for a core mutant
  and cannot run `crates/server/tests/*`, where several core subsystems are actually pinned.
  Re-testing that run's 96 survivors with `--test-workspace=true` flipped **51 of 96 (53%) to
  CAUGHT**. For a `crates/core` file `--test-workspace=true` is **not optional**; for a
  `crates/server` file the package default is sound, because core tests cannot reach server code.
  Cost is real (each core mutant relinks ~50 server test binaries, ~1.7 mutants/min at `-j 4`) —
  budget for it rather than trading it away.
- **Being called is not being tested — the trap that makes a run worth the compute.** An end-to-end
  test *calls* every helper, so replacing a whole function is caught instantly and coverage looks
  fine. What survives is the inside: token boundaries, nesting loops, cursor arithmetic. Measured on
  `skim.rs`: `LayerKind::tag -> ""` and `first_prose_sentence -> None` are both caught, while **35
  finer-grained mutants inside those same functions survived the full workspace suite.** Sampling
  whole-function mutants and generalising from them is how this gets missed.
- **A caught mutant aborts its test run early, so a file's mutants/min tracks its survivor density —
  a *slow* file is a *bad* file.** The 2.3/min figure that argued for a 3.5 h budget was an artefact
  of one such file and cost two sessions of deferral; the real rates were 7.8 and 9.0/min.
- **A partial run's shape-conclusion describes the part it measured, not the file.** "All 36
  survivors are one shape" was true of 338 mutants and wrong about `lsp_nav.rs`, whose remaining 23
  survivors sat in the tail and were a different shape.
- **Housekeeping:** the scratch copy carries no `.git` (that made the baseline red until item 57);
  pass `--output` outside the tree so a run is never mistaken for working state (`mutants.out/` **is**
  in `.gitignore`, line 9 — the earlier claim that it is not was wrong); run from a `git archive`
  snapshot so the working tree stays free during a multi-hour run. **Commit before mutation-testing:**
  `git checkout -- <file>` on an *uncommitted* file restores from HEAD and destroys the working
  implementation (it did, twice).
- **Equivalent mutants already triaged, do not re-triage:** `diagnostics/shape.rs` `is_content`
  (`:81` both conjuncts, `:156` `i + 1` → `i`) — at its only call site the slice runs between two
  consecutive heading indices, and no `Block` anywhere is built with empty `html`, so both conjuncts
  are unreachable-false; writing that test would be the vacuous-test defect the round existed to
  remove. `runtime_dirs.rs` `pid_alive -> false` at `:103` is the `#[cfg(not(unix))]` arm and
  **reappears on every future run.** The rest are in the two 2026-07-27 findings docs' tables.
- **A knowing skip:** `interactive.rs` (5 survivors) is the TTY wizard layer; pinning it needs a PTY
  harness and the *non*-TTY path is already pinned by `crates/server/tests/wizard_gate.rs`.

## Probes and instruments (each one produced a false result first)

- **A table-shaped probe whose every cell is negative is a BROKEN probe until proven otherwise.**
  (2026-07-28, R11/R14.) A 27-row construct inventory across nine decks returned `NONE` for every
  row. The cause was not the corpus: **zsh does not word-split `$VAR` in a `for` loop**, so the
  probe iterated one long string and matched nothing. It was caught only because one row
  (`auto-animate`) had been measured by hand minutes earlier and was *known* to be non-empty.
  **Carry a known-positive row in every table probe and print it first**, so a broken instrument
  announces itself instead of reading as a clean result. Use `${=VAR}` or a real array.
- **`taliesin --version` before any CLI measurement: `target/` is shared with the parallel session.**
  (2026-07-28.) The release binary reported a SHA from the *other* branch, so a round's headline was
  measured against code this branch does not contain. It held on re-measurement after
  `cargo build --release`, but that was luck. **A shared `target/` makes every CLI number suspect;
  check the reported SHA against your own HEAD, and rebuild before trusting one.**
- **Grep the emitted result, not the class name you remember.** (2026-07-28.) A highlight-coverage
  probe grepped `qhl-` (the prefix `CLAUDE.md:67` still names) and returned **0 on a fully
  highlighted page**, reading as "highlighting is broken". The emitter is `tali-hl-`
  (`highlight.rs:23`). Same family as the inlined-asset needle trap: **a needle taken from prose is
  a hypothesis; take it from the emitter.**
- **`cargo test -- <short_name> --exact` matches NOTHING and passes.** `--exact` compares against the
  *full* test path, so a bare `deck_copy_button_is_reachable_on_touch` runs **zero** tests, prints
  `test result: ok. 0 passed`, and a mutation harness looking for "FAILED" scores it SURVIVED. This
  reported **0/5 mutants killed** on tests that in fact kill 5/5 — i.e. it accused the tests of being
  vacuous when the *harness* was. Pass `render::tests::<name>`, and make the harness **assert that
  exactly one test ran** rather than inferring from the absence of a failure.
- **`getBoundingClientRect` reports GEOMETRY, not visibility — a box can span the screen and paint
  nothing.** This produced the only *false finding* in the audit record (DT-5, filed and retracted
  the same day): each of a deck's neighbouring slides was intersected with the **viewport**, giving
  "17.9% of the screen is bleeding through", when the real answer was **0 px** because
  `.tali-deck` clips them with `overflow: hidden`. **To ask whether something is visible, intersect
  it with its CLIPPING ANCESTOR, or ask the renderer** — `elementFromPoint` said `BODY` throughout
  and was explained away. A cheap ancestor walk settles it: climb parents, and if any has
  `overflow: hidden` and the child's rect escapes that parent's rect, the escaping part paints
  nothing.
- **Reproducibility is a property of the instrument, not of the claim.** That measurement was
  consistent, quantified to a decimal, and reproduced on the built artifact as well as in preview —
  every property of a solid finding except being true. **Before writing a finding down, read the
  code that owns the behaviour**; the rule that refuted this one was a CSS comment saying
  "adjacent cells fall outside and are clipped (no peek)". The backlog's standing "trust the
  symptom, never the cause" applies to your OWN findings, not just to inherited ones.
- **Chrome that fades on idle will confound a screenshot comparison.** Two deck screenshots differed
  only because `html.tali-idle` had hidden the controls between them, which briefly looked like
  evidence about the change under test. Force the state you are comparing.
- **The inlined-asset needle trap** (bit three times in one batch): every page inlines the whole
  CSS + enhancer-JS payload into its `<head>`, so **any new class name, `data-` attribute or
  user-facing string is present in the HTML of every page whether or not that page renders the
  feature.** A whole-page `contains("…")` is satisfied by a page rendering none of it. **Needle the
  full emitted tag, or slice the block out first.**
- **A runtime-injected DOM node is invisible to a static grep.** Deck `theme-color` is created by
  `deck.rs:240` (`createElement` + `setAttribute`), so grepping built HTML reports it missing on all
  four deck paths — a false regression of shipped work. When the mechanism is runtime construction,
  the only valid needle is the rendered result in a browser.
- **A uniformly positive parity/coverage row is a broken probe until proven otherwise**, exactly like
  a uniformly negative one. An unset shell variable makes `grep -qF -- "$n"` true on every file.
- **zsh does not word-split an unquoted variable.** `files="a.html b.html"; for f in $files` passes
  all names to `grep` as ONE argument. Write the list literally, or use an array. Other grep traps:
  a bare word matches prose, `grep | head` reports head's exit code, quote `--include='*.tmd'`.
- **`resize_page` floors at ~500px.** It resizes the *window*, and Chrome will not go narrower. Two
  probes reported `innerWidth: 500` while the operator believed they were at 390 — silently across
  the 40rem breakpoint the audit was about. **Use viewport emulation, never window resize, below
  ~500px.**
- **Raw CDP `Network.emulateNetworkConditions` silently no-ops**, with or without `Network.enable`.
  Use puppeteer's `page.emulateNetworkConditions(...)`. A "throttled" number that is not slower than
  the unthrottled one is a broken instrument, not a fast page. (`Emulation.setCPUThrottlingRate`
  does work over raw CDP.)
- **A hidden overlay still renders.** Setting the palette input and reading its list gave a confident
  "No matches" while the overlay was closed the whole time — the list was stale, not empty. **Assert
  the surface is open before believing what it says**, and settle a transition before measuring
  geometry (the mobile sheet reads as off-screen at y=844 synchronously and y=688 once settled).
- **`window.scrollBy` cannot test a scroll lock.** `overflow: hidden` blocks USER scrolling and
  deliberately still permits programmatic scrolling, so the reading is identical with and without the
  fix. Use real key/gesture events (measured with PageDown: drawer closed the page moves 707px,
  drawer open it does not move while the panel scrolls instead).
- **The deck feed flag is on `document.documentElement` (`html.tali-feed`) and its scroller is
  `.tali-slides`, not the document.** Probing `.tali-deck` and `window.scrollY` made a working feed
  look completely dead.
- **`#tali-toc-handle` is an id, not a class.** A `.tali-toc-handle` selector reports the sheet handle
  missing everywhere.
- **To measure anything about cell execution, edit the CELL BODY, not the page.** A cell's freeze key
  is its own code plus all upstream same-language code, so editing a page's *prose* leaves every cell
  hash intact and nothing re-runs. AP3-1's first probe did exactly this and reported 0.09 s with and
  without the fix — a false all-clear on an unfixed build. The same trap makes any "is the kernel
  busy?" setup silently no-op.
- **A message-catalogue sweep must enumerate the EMITTERS, not one command's output.** DIAG-1
  measured `check --format json` over 23 targets and found six uncatalogued diagnostics; there were
  eight. The two it could not see are emitted only by `build`/`publish`, and `check` never executes a
  cell.
- **A CSS rule can be silently discarded by the cascade.** MOB-4's block first landed above
  `.tali-copy`'s own `opacity: 0` at equal specificity; copy stayed invisible while the anchor half
  worked, and a "selector is inside the block" test passed throughout. **Assert source ORDER.**
- **A test whose subject is an env var needs a lock and an assertion on *why* it skipped.** The
  wedged-browser test passed vacuously in 0.02 s whenever it raced the other `CHROME_PATH` test: it
  read that test's `/nonexistent/…`, skipped every cell instantly as "chrome unavailable", and
  satisfied its own elapsed-time assertion by never launching anything.
- **"No automated reproduction" can be false.** A wedged browser is reproducible without a wedged
  browser: point `CHROME_PATH` at a program that launches and then sleeps. 20.00 s before, 7 s after
  — the assertion is on the clock.

- **A gate that skips silently makes someone else's green run meaningless.** Wave 1 measured all
  four hand-run gates PASSING, refuting weeks of "they have probably rotted". The real defect was
  the adjacent one: they skip when an interpreter is absent, so *your* green run proves the gates
  and an *outsider's* proves nothing. Non-vacuity must be asserted, not assumed — confirm a named
  live-kernel test printed `... ok`, and confirm `tsc` with `--listFiles` that files were checked.
- **`cargo test --lib` on `taliesin-server` errors — it is a `bin` crate — and `cmd > log; echo $?`
  reports the exit code of `echo`, not of cargo.** Together those two made a harness report exit 0
  for a gate that never ran. Capture the command's own exit code, and never infer a pass from a log
  file that could be empty for the wrong reason. (Third recorded instance of the bin-crate trap.)
- **A repo-wide grep hits gitignored build output.** `docs/guide/_book/` and `_site/` contain a
  rendered copy of everything, so a `grep -r` for any phrase returns the *artifact* as well as the
  source, and a stale artifact answers a question about the source wrongly. This is AP9's "12
  `<h1>`" trap in a new costume. Exclude `_book/` and `_site/`, or restrict to tracked source.

## What the test net structurally cannot see

The dogfood books (`docs/guide`, `docs/internals`) are **not** in the regression net, so any shape
only they have is invisible to a green suite. Three gaps were measured (enumerated, not grepped) and
each hid a real bug: **(1)** every corpus book chapter opened `# Title` with no front-matter `title:`,
so heading demotion went unexercised while 32 of 32 dogfood chapters use it; **(2)** no book in the
repo has an **include-built chapter**, so any rule reading a chapter's *source* (word counts, `skim`,
prose lints) passes vacuously; **(3)** no corpus book keeps a chapter in a **subdirectory**, so
depth-relative emission (`{up}` hrefs, `../index.html`) is the empty string everywhere the suite can
look. (2) and (3) are now minted in temp dirs by `site/skim.rs` and `tests/book_landing_toc.rs`.
**When a defect is reported on a dogfood page, first ask whether the corpus has that shape at all.**

Grepping the manual for a front-matter key also hits the manual's own *documentation* of that key:
any coverage figure over `docs/` must parse the leading front-matter block, not match a line.

**A set removed for one purpose is removed for all of them.** (2026-07-28, R14.) The single largest
coverage hole found in three audit waves was one line: `site/mod.rs:359` drops `{{< embed >}}`-ed
decks from `site.pages` so they stay out of *nav*, and `check`'s site walk iterates that same set —
so a deck reaches **zero of thirteen** validators while `check` and `--strict` both exit 0. The same
mechanism hides `draft:` pages. **When you filter a collection, ask who else reads it**, and prefer
a derived view over mutating the shared one.

**Scope a coverage round from what the code REACHES, not from the exemptions that are written down.**
(2026-07-28, R14.) That round was scoped on two documented `DocFormat::Reveal` early-returns. Both
turned out to be **correct** (a duplicate-heading rule is 100% false positives on the `auto-animate`
idiom — 3 hits, all deliberate magic-move pairs). The real defect was that a deck never reaches the
function those early-returns live in. **A written-down exemption list is a map of what someone
already thought about; the hole is in what nobody did.**

**An exemption with no replacement check is a hole, and only a register makes holes countable.**
Most exemptions in this tree are individually well-reasoned and should stay. The finding is almost
never "remove the exemption" — it is "the exemption is correct and the deck-appropriate replacement
was never written." Score the *replacement*, not the rationale.

**Score detection, not existence, and score it to the mutation question.** (2026-07-28, R7.) 14 of
17 enumerated failure modes score D ≥ 8 ("would not be caught"), and the suite is *not* weak: 1,658
tests. It is dense on pure functions over the block model and thin-to-absent on anything that only
exists **at runtime, in a browser, in a published artefact, or in prose**. Wave 1's three HIGH
security findings sat in exactly that band, which is why ~30 correctness rounds could not see them:
**correctness rounds read code, and that class only fails when something runs.** When judging
coverage, count test *bodies*, never filenames — `mounts` shows 0 files by name and 3 by body, of
which 2 are false positives.

## Reading an item or a finding before acting on it

- **A dense "do not touch" cluster is not evidence of coverage; it is a reason to measure.**
  The deck accumulated eight declined / retracted / do-not-re-scope entries in `backlog.md`, each
  individually a sound ruling, plus two code-level diagnostic exemptions
  (`diagnostics/shape.rs:97`, `diagnostics/a11y.rs:228`) each with a defensible rationale. The
  aggregate made the **largest hand-written client subsystem in the tree** (`deck.js`, 2,690 lines)
  the one with the fewest automated checks — and made sessions approaching it back off. The first
  draft of the 2026-07-27 audit slate read that thicket and wrote "no deck audit" **without
  measuring anything**, which is the mechanism reproducing itself inside the document meant to fix
  it. **An exemption is not a hole; an exemption whose replacement check was never written is.**
- **Ask what question an audit is asking before adding another one.** By 2026-07-26 four fresh
  lenses in a day produced zero HIGH findings, and the menu read as exhausted. It was not: every
  lens on it asked *is this correct?*. Wave 1 asked four other questions (is it **detectable**, does
  it **hold under scenario stress**, would a stranger **adopt** it, can it be **handed over**) and
  found three HIGH security defects in one pass. **None of them is a correctness bug** — they are
  defects only once a document arrives from someone else.

- **Trust the symptom, never the stated cause, line number or cost** — all three have rotted, and a
  stale *cause* has sat under a real *symptom* (MOB-5: the dialog half was already fixed; the focus
  loss was `19-book-outline.js` re-parenting the focused link during hydration).
- **An audit's stated fix can be a revert.** PP-3's proposed "give the single-file build the same
  inferred root" would have re-opened PT-2 (`9359a2c`). **Read the code the item proposes to change
  before trusting the change.**
- **A documented reason can be true of a sibling path and false of yours.** PP-1 was held back by "the
  client doesn't re-index on a live edit" — true of a site's cross-page index, false of a single doc.
- **A finding that names one instance has not enumerated the shape.** The `.git`-dependence was two
  tests, not the one the audit named; MOB-6 had a third instance nobody listed.
- **An item's proposed data shape can contradict a decision the tree already made.** Item 54 proposed
  a retired-key table carrying replacements, which invites "did you mean `hero`?" — a phrasing
  `frontmatter.rs:487` had already ruled out, because `codes::extract_suggestion` lifts it into a
  structured fix an agent applies mechanically, and `about:` → `hero:` is a rewrite, not a rename.
- **A missing signal and a mis-shaped signal look identical from the outside.** Item 53's warning was
  being pushed the whole time, as a different advisory that `check` discards on purpose. **Before
  adding a diagnostic, check whether one is already being emitted and filtered.**
- **When an item calls a boolean mutant equivalent, check both sides.** Item 65's `&&`→`||` was filed
  as needing a page that emits a title block *and* opens with an `<h1>`; the other side (no title
  block, first heading `##`) was trivially reachable and lost its first section under the mutant.
- **The obvious fix can be the regression.** `min-height: 44px` on a nav link grew the *sticky* bar
  from 52px to 75px at 844x390 — reintroducing MOB-8 while fixing MOB-7. Tap targets on a sticky bar
  must grow by overlay, not by height.
- **Read the dependency's source before believing an "unbounded" claim.** Item 55 listed three
  chromiumoxide calls as unbounded; the library bounds all three (20 s `launch_timeout`, 30 s
  `request_timeout`). The real gaps were the websocket connect and `close()`/`wait()`.
- **A front-matter key can render as visible prose.** `description:` emits a lede under the H1
  (`render/mod.rs:1312`) as well as `<meta>`/og, the book landing annotation and search text — 13
  descriptions drafted *from* each page's opening paragraph printed directly above it. **The browser
  showed it; no grep would have. Check what a metadata key renders before writing 36 of them.**
- **Calibrate a new lint against real output before writing it.** Measuring the proposed `TAL-SHAPE-*`
  rules over all 14 site projects killed four of their own prescriptions, including the most valuable
  one (it fired on 11.8% of the corpus, essentially all false positives) and one whose stated
  justification did not exist in the tree.
