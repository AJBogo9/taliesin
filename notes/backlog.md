# Taliesin backlog

**Scope: corpus-plus-roadmap.** "Done" = the docs under `corpus/` render correctly (the corpus is
the regression net); each new capability ships pinned by a target corpus doc. Output stays
**HTML-only**. Roadmap: `ROADMAP.md`.

> Kept small (read often). **Only open tasks live here** — delete items once landed; don't leave
> `[x]`. Completed work is in git + `ROADMAP.md` / `native-rewrite.md` / `AUDITS.md`.

## State (2026-07-16)

v0.2.0. All four formats render + deploy; the dev loop is strong (block-level incremental updates
with DOM-state preservation, warm server + Jupyter kernel, `_freeze` cache, Alt-click + reverse
cursor sync, located diagnostics, CSS hot-swap, Cmd-K search). **Tier 1 is empty.**

**The old A-G sections are gone.** Every one closed, so on 2026-07-16 the lettering was dissolved
and the survivors flattened into the priority list below — the letters had stopped carrying
meaning, and four of the seven sections existed only to say "closed". The closure records (incl.
the rot evidence for B, D and G) moved **verbatim** to [AUDITS.md](AUDITS.md); they were kept
rather than deleted because those three were re-scoped by later sessions precisely when the
reasoning lived only in git. **Do not re-open them; do not re-scope from them.**

**Before picking any item: grep its named symbol/flag in source first, and prefer measuring the
running product over reading this file.** The author pushes work mid-session, so an entry can go
stale with no signal here (that is how B, D and G all rotted). Trust an item's described *symptom*,
never its cause or line number. **An entry marked "verified against source" is not enough**: §D's
layout targets carried exactly that label and were quoting a real CSS line that a rule ten lines
below already overrode. A browser measurement dissolved two of them in minutes. **An entry's stated
*cost* rots too**: the theorem ruling's "breaks 2 pins" named two unrelated tests, and the pins it
feared passed untouched. Price a change by making it and reading the failures.

**Working method:** branch per feature; brainstorm if there's a fork; spec under
`docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
extension harnesses); fast-forward merge locally; delete the item here. Agents commit + ff-merge to
local `main` on request; push to `origin/main` only when the author asks. **Do-NOT-touch:** the
exec/kernel zone + the single-editing-surface invariant. Review subagents use read-only git —
**and that instruction is not enough.** On 2026-07-17 a `rust-reviewer` told "read-only, report
findings, do not modify anything" decided to build a differential harness and ran `cat > Cargo.toml`
in the **repo root**, destroying the workspace manifest (and `Cargo.lock`, 2711 lines) mid-session.
It touched no git command at all. A review agent holding `Bash` will write scratch files, and its
CWD is your tree. **Give it a worktree, or commit before dispatching it** — "read-only" describes
an intent, not a permission.
**Author policy (feature-first):** finish framework features before marketing-site work.

**Standing constraints on any change** (from the 2026-07-11 website audit, 99 findings; detail:
[2026-07-11-website-design-audit.md](2026-07-11-website-design-audit.md)): the **personal blog**
(`corpus/tech-blog/`) is the forward-facing brand, direction **"Marginalia"** (iron-gall manuscript
ink), and its 14 explicit **KEEPs** (serif/sans pairing, offline bundling, `meta.rs` OG head,
live-figure thumbnails) live in that detail file — protect them. Every fix stays invariant-safe: no
CDN, no preview write-back, no new output format, `--tali-*` tokens only.

## Next session: start here

**Git.** Do not trust a SHA written here; any commit that records one falsifies it — and on
2026-07-16 an agent wrote a SHA into this file that **did not exist at all**. **Check, do not
read:** `git log --oneline origin/main..main` for what is unpushed (the author pushes, not the
agent), and `git reflog show origin/main` before believing ANY "not pushed" claim, including one in
a session handoff. The author pushes mid-session with no signal in this file, and a handoff has now
been wrong about this **five** times: on 2026-07-16 one said "+6 unpushed" (all six were already
pushed), he then pushed twice more mid-session, and later that day a handoff said "the last backlog
commit is local" while it was pushed, before he pushed four more in-flight commits. Re-run the
checks too, do not assume: `cargo test -p taliesin-core` + `-p taliesin-server`, `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`. *(It happened a sixth time on 2026-07-16: the author
pushed `2368e4a` mid-session while an agent was mid-task. The count was checked, not reported.)*
*(A **seventh** time on 2026-07-17: the author pushed `50e0e71` mid-session while an agent was
mid-task, so three of that session's own commits were already on `origin/main` before it finished.
Checked, not assumed — which is the only reason the handoff below is right.)* *(An **eighth** time,
same day: `origin/main` reached `044bdc7` overnight, so the ten commits the previous handoff called
"unpushed" were **all** already pushed before anyone read it. That handoff was written accurately
and was stale within hours. **This is now the single most reliable way this file lies to you.**)*

**THREE gates now, or the suite quietly under-tests itself:** `TALIESIN_REQUIRE_NODE=1 cargo test
--workspace` (the JS-equivalence guard skips without Node), `TALIESIN_R=R TALIESIN_REQUIRE_R=1
cargo test -p taliesin-server --test r_kernel` (R needs an interpreter + IRkernel), and **new
2026-07-17** `TALIESIN_PYTHON=~/.local/share/qmd-venv/bin/python TALIESIN_REQUIRE_KERNEL=1` (the
pool-booted `--jobs` path; a missing interpreter is a HARD FAIL, not a skip). CI sets all three.
**A plain `cargo test` silently skips all of them.** Note `cargo test` **aborts the remaining test
binaries at the first failure**, so a flake hides every later binary's result: re-run before reading
a total. **Two distinct flake families now, and conflating them cost a day** (both in Tier 2): two
genuinely load-sensitive *timing* tests, and **two `exec::tests` probe tests that are a concurrency
race, not timing** — they fail ~2 runs in 3 on an *idle* machine and pass 3/3 under
`--test-threads=1`, which is 4x slower. If an `exec` probe test fails, **`--test-threads=1` before
you blame your change or the CPU.**

**What is left is a flat list; none of it is a grind chunk.** All **three owner rulings are
CLOSED**, both **ruling rounds are spent**, and the **whole M2-M6 exec/kernel audit is finished
except M6a, whose sign-off was refused** (2026-07-17). What remains is §2's live defects (**go count
the un-struck ones**; a number written here has now been wrong twice, once by the very session that
wrote the header telling it not to, so this sentence no longer carries one), D70 (unruled), D72
(declined), and one deliberate deferral. Everything else is Tier 2/3 (demand-driven). **There is no
gated work waiting on the owner: the next session can just build.**

**This file was measured against source on 2026-07-17 and was wrong in six places.** A sweep
re-derived every open item from today's code. **The rot was not the author pushing mid-session:
`2368e4a` ("prune the machine-facing items that landed") pruned only the *machine-facing* items, so
entries from other sources were never re-checked** — and the start-here block was then refreshed
90 minutes *after* two of them had landed, still advertising one as the next thing to build. **A
scoped prune leaves the unscoped half looking freshly reviewed.** When you prune, either prune the
whole list or say which slice you pruned. Corrections applied below; the sweep's verdicts are in
[2026-07-17-backlog-truth-sweep.md](2026-07-17-backlog-truth-sweep.md).

**It then happened a seventh time, to the session that wrote the paragraph above.** The theorem-docs
item sat in §2 for hours *after that same session built the page it asked for*, and was caught only
because a later pass checked the file existed instead of reading the entry. So the lesson is not
"other people's entries rot": **you will land something and leave its entry standing on the same
day you write a warning about exactly that.** The habit that catches it is cheap: before you commit
a fix, grep this file for the thing you just built.

**Second ruling round, 2026-07-17 (do not re-litigate):**
- **Exec/kernel sign-off GRANTED for M2 and the M3+M4+M5 bundle.** **M6a (`MAX_WARM_PAGES`) was
  NOT granted: the zone stays frozen there.** Do not tune that constant or touch `exec_pool.rs`
  eviction without a new ruling. M6b (the `/proc` probe) never needed sign-off and is free.
- **Citation sign-off GRANTED for D69 only** (the appendix-after-`# References` orphan). **D72
  (bare `@key`) DECLINED for now**: the diagnostic already ships, so nothing renders wrong silently,
  which makes it a feature question, not a defect.
- **A reference's click-to-source lands NOWHERE.** ~~Live defect #1~~ **LANDED 2026-07-17**
  (`5bfd703`): `locatable()` now requires a *usable* sourcepos (`^[1-9]\d*:\d+`), so the References
  section is simply not click-to-source instead of silently landing every click on line 1. A CSL
  entry's real position is in the `.bib`, not the `.tmd`, so there is nothing truthful to point at,
  and pointing at the `[@key]` site would dress a guess up as navigation. As predicted, the one
  change also closed the footnote-chrome bug, **verified in a real browser**: the section and its
  `<hr>` resolve to NOWHERE while a footnote `<li>` still resolves to `fn-aside@29:1-29:89`, because
  the walk continues *past* an unusable block to a usable ancestor. It added no new bridge back to
  source; it removed two false ones. **Consequence the owner should know:** the generated title
  block lost its Alt-click too, where line 1 was accidentally truthful (front matter starts there).
- **D34: SUBTRACT.** Delete the dead `SiteConfig.card_image` (zero readers; its own doc comment
  concedes it); **defer** the `bibliography`/`csl`/`execute`/`theme` project defaults until a corpus
  doc actually hurts. "Perfect the default before adding a knob."
- **D70 "Cite this" card: still unruled**, and low priority — it would render author-free for all 8
  tech-blog posts (0 set `author:`), and its machine-readable half already ships.

**The three rulings landed 2026-07-17 (do not re-litigate):**
1. **`--jobs N` means N parallel PAGES.** The CLI was right; the code was the bug. Warm pool is now
   ADDITIVE under an explicit `--jobs`; auto mode is untouched (`f141cac`). Measured on the running
   product: `--jobs 3` + pool → 3 pages (was 1); auto + pool → 14 + 2 warm (unchanged).
2. **The `<title>` clobber is fixed at the PRODUCER** (`full_render` carries the display-ready
   title). Consumer/`PageDoc.title` alternatives were rejected: each closes only one of the two
   symptoms.
3. **MCP gets no root; the false claim goes instead** (`d0819fc`). A root would withhold nothing
   real (a host that can run the binary already has the filesystem). The module + `taliesin help
   mcp` now say plainly: not a sandbox, no containment, and `build` writes HTML and runs cells.

**Pick up here (2026-07-17, latest session — the §2 sitting: #8's `<lastmod>`, #2+#4, #9, #10):**
1. **§2 is down to FOUR un-struck items: #1, #3, #5, and #8's other two thirds.** *Recount, do not
   read this line* — `awk '/^### 2\./,/^### 3\./' notes/backlog.md | grep -cE '^[0-9]+\. '` still
   says ten; six are struck. (An agent wrote "SEVEN" here on 2026-07-17, under a header telling it
   not to, by forgetting #2.) **None of the four is mechanical**, so the "one small change" seam is
   genuinely mined out: #1 is two channels; #3 is a design question in a defect's clothes; #5 wants
   an anchor-registration *rule*; #8's `<loc>` halves are two fixes plus a diagnostic.
2. **Four entries were priced this session; three were wrong, and #9 was the only one that held.**
   #8's "close to free" third cost the most on the list (the helper it told me to call was itself
   broken). #2's implied incremental-invalidation design collapsed into "re-run the two passes"
   the moment a full re-harvest was *measured* at 27ms. #10 named three regex contexts, two were
   padding, and it missed three. **Price by building and measure before designing** — the estimate
   in an entry is the one part of it nobody re-derived.
   **And an entry is not the only thing that rots: so does your own reasoning, within the hour.**
   #10's first fix shipped a paragraph arguing a one-char change was unsafe, with three examples
   that were not counter-examples, plus a test that could not fail asserting the same wrong rule.
   Review caught it; the suite could not. **Two of the three vacuous tests found today were mine,
   written the same hour I wrote the warning about vacuous tests.** The habit that works is
   mechanical, not attentional: **mutate the fix, watch the named test fail.** Nothing else caught
   these.
3. **A workaround sitting in the tree is not a constraint.** #10's entry treated the vendored-lib
   bypass as a standing fact to design around; the bypass existed only because the minifier was
   broken, and fixing the minifier dissolved it. When an entry says "X is unsafe, so we avoid X",
   check whether X is still unsafe before you build around it.
4. **When an entry says "path X already fixes exactly this", that is evidence, not a template.**
   #6's entry pointed at `deck_meta_changed` and the fix it implied (force a re-mount) was wrong:
   the deck re-mounts because its title slide is *structural*, a reason that does not transfer to an
   HTML page, where a re-mount discards every live `{js}` cell. Check what *forced* the neighbouring
   shape before copying it.
5. **The lesson that keeps paying: read the helper the entry points AWAY from.** #4 named one defect
   and the function held four; the other three were invisible to the entry because it blamed the
   symptom's location instead of the code next door that already did the job right. **`grep` for the
   correct helper before writing one** — it has now existed already in 5 of 5 cases (`search.rs` vs
   `render/text.rs`, `llms.rs`, `meta.rs`, `minify.rs`'s own JS branch, and `seo.rs` vs `feed.rs`'s
   `rfc3339`). **But 5-of-5 is only half the rule:** `rfc3339` turned out to be *wrong* when finally
   called (`2026-99-99` passed it), so the sixth case cost more than writing one would have. Grep
   for the helper, then **test the helper** — see #8.

**Pick up here (2026-07-17, after the M-audit was finished):**
1. **Sections 1 and 4's exec/kernel half are now EMPTY.** M1, M2, M3+M4+M5 and M6b have all landed;
   **M6a (`MAX_WARM_PAGES`) is the only exec/kernel item left and its sign-off was REFUSED**, so it
   is not available to pick up. D69 landed; D72 is declined. **Section 2 (live defects) is
   what is actually left**, plus D70, which is unruled. *(A count stood here and rotted; the
   newer block above owns it. Go count.)*
2. **Blast-radius labels were wrong 4 of 5 times** (D49, D67, M1, M1's other half; only M2-M5 truly
   needed the zone). **The label travels with the SUMMARY, not the code**: M6 was filed as one item
   ("a constant and a `/proc` probe") and split into a free-standing file read and a
   kernel-lifecycle policy sitting on opposite sides of the line.
3. **The exec/kernel zone is now well-tested, which changes the cost of future work here.** A
   stand-in daemon that speaks the fork protocol makes the ERROR and outran-the-deadline paths
   reachable without waiting on rare load, and `TALIESIN_REQUIRE_KERNEL=1` hard-fails instead of
   skipping. Reuse that pattern rather than inventing another.
4. **Verify by mutation, not by green.** Every fix this session was confirmed by *restoring its bug*
   and watching the new test fail (5 for 5). This caught nothing wrong, which is the point: it is
   what makes "the tests pass" mean something. Two of those tests would otherwise have been
   plausible and vacuous.
5. **One open question, filed in §4:** the M4 test's `sleep 300` stand-in survives ~2 of 8
   full-suite runs and only when the build is cold. Measured, unexplained, argued test-only. Do not
   re-run the ruled-out checks listed there.

**Method note that paid off three times on 2026-07-16, use it:** when an entry names a file:line,
open the *running product* before the file. §D's layout targets were labelled "re-verified against
source" and still dissolved under a browser measurement, because the quoted CSS line was real but
overridden ten lines below. A true cause can also name the wrong *layer*: the cell-label xref entry
correctly blamed `scan_page_anchors`, but the fix belonged in `harvest_xref_numbers`, which already
had the data — **ask "who already knows this?" before "where does the entry point?"**. Grep traps
are live here too: a bare word matches prose (`feature` matched the English word), `grep | head`
reports **head's** exit code (so `|| echo "absent"` never fires and `$?` lies), and zsh needs
`--include='*.tmd'` quoted. See [[grep-verification-traps]].

**Machine-facing audit (2026-07-16), the lens worth reusing:** every prior round was **eye-driven**
(browser screenshots, "does it render right"), so it covered what an eye lands on and missed what
none does. Auditing the surfaces with **no viewer** (output read by browsers, crawlers, agents and
the preview client) found ~35 findings in a codebase that had already survived 134-finding scrutiny:
detail + evidence in [2026-07-16-machine-facing-audit.md](2026-07-16-machine-facing-audit.md).
Landed the same day: the `llms.rs` separator (production was garbled), the `meta.rs` `<` escape (a
`<!--<script>` description blanked the whole page), the `minify.rs` CSS token fusion, the `set_meta`
wire test, an acorn token-equivalence guard, live R CI, the card headline ellipsis, and the card
glyph diagnostic — plus **M1**, the `--jobs` collapse (`--jobs N` now builds exactly N pages;
measured 2/3 -> 1 before, 2/3 -> 2/3 after). **Two lessons that generalize:** the correct helper already existed next door and
was reimplemented weaker where nothing looks (3x — `search.rs` vs `llms.rs`/`meta.rs`, and
`minify.rs`'s own JS branch vs its CSS branch), and **the tests certify the defects** (the `--jobs`
unit test blesses the collapse; the refill test re-implements the loop with no error arm; the card
overflow test asserts dimensions only and passes *while* truncating). When a test can only exercise
the case that works, that is the finding.

**Decided 2026-07-16, do not re-litigate:** §G's three leftovers (`check --online`, the
numeric-claim hint, the per-page text sidecar) are **declined** (see "Decided against"). Catalog
work is **triage on demand**, not a sweep. The **References** click-to-source bug is **logged, not
fixed**, pending a design answer. The **`--space-1..6` scale is dropped** (pure refactor, no
payoff). Theorem numbering was ruled **auto-scope + delete `number-within`** and has landed.

## Open work (flat, priority order)

### 1. Build-ready now (no ruling needed)

**Empty.** D37 (lint `format:` sub-keys) was the only entry and had already landed (`515fbd7`) when
this section still called it "the cleanest build on the list". Do not re-add it.

### 2. Live defects (small, independent; count them, don't trust a number written here)

*Re-derived from source 2026-07-17. Line numbers below are corrected; several had drifted, and one
entry (`lang: fr`) was pointing at correct code.*

*Re-verified against source again 2026-07-17 (later session), items 6-10: **all five are LIVE** and
their described symptoms hold. But **three of five were mispriced as one change**, so read the
per-item cost notes before promising anything: **#8 is three independent fixes** (the first landed),
**#9 is two** (a mechanical clamp ×3, plus a different fix to the shared `wrap()` shrink trigger),
and **#10 is three** (only the `=>` case is cheap; `)`/`]` need real context tracking). **The
recurring finding is not drift, it is under-pricing:** an entry's summary counts symptoms, and
symptoms do not map 1:1 to changes.*

*Priced again by BUILDING them, 2026-07-17 (#8a, #2, #4). **Under-pricing is not the only direction,
and the summary is not the only thing that lies — the stated CAUSE is too.** #2 came in cheaper than
its fix implied (a whole-site re-derive measured 27ms, so the incremental invalidation everyone would
have designed was never needed) and dearer in the part nobody wrote down (fixing the registry fixed
nothing a reader could see; the dependent page never rebuilt). #8a's entry was true in every clause
and wrong in its conclusion, because "the correct helper is next door, just call it" has a
precondition nobody checks: **that the helper is correct.** The one habit that caught all of it:
**measure the running product before and after, not the unit test.** Every one of these had a green
unit test while the live server still served the bug.*

1. **Duplicate-label warnings are unlocated** (`render/mod.rs:1568-1571`, `site/xref.rs:50-56` emit
   no file/line), half-reproducing the exact Quarto flaw D53 critiques. *(The harvest's own duplicate
   warning, added 2026-07-16, is unlocated for the same reason: `site/mod.rs:953`.)* **Price it
   before promising it (2026-07-17): this reads as one small fix and is TWO, with very different
   costs.** `render/mod.rs:1568` is on the `Vec<Warning>` channel, which already has `.at(file,
   line)` (`render/model.rs:166`) — cheap. But `site/xref.rs:23` and `site/mod.rs:172` are
   `Vec<String>`: **no location field exists in that channel at all**, so half of this item is a
   channel type change. (Line numbers here had drifted by 30 and 6; the symptom held.)
2. ~~**The xref registry goes stale on a warm content edit**~~ **LANDED 2026-07-17** (`4b35bb5`),
   together with #4 — they were the two ends of one seam and shipped as one change. `refresh_xrefs()`
   re-runs both producers on a `.tmd` change. **Three things the entry did not know, all found by
   measuring the running server, and the lesson is the same each time: the filed symptom was the
   visible tip.** (1) The obvious gate is WRONG: refreshing when `to_rebuild` is non-empty reads
   right ("refresh when we rebuild") and does nothing — that list is the *open tabs* whose own
   sources moved, and registry staleness is a function of the CHANGED FILES. Editing `intro.tmd`
   while only `methods.html` is open leaves it empty, which is exactly the cross-page case.
   (2) **Fixing the registry fixes nothing a reader can read.** With the registry provably holding
   `number: "1.2"`, the page still served "Figure 1.1": a served page comes from its cached render,
   and a page that only *cross-references* an edited page names no changed file, so it never
   rebuilds. `backlinks` was that missing edge, already reversed. (3) The registry wasn't merely
   stale, it was **dead for anything new**: an anchor created while warm rendered as
   `<a href="#fig-new">Figure</a>`, a same-page link to an anchor not on that page, landing nowhere
   with no warning. **Cheaper than feared where it counted:** a whole-site re-derive measured 27ms
   on the largest real book, so no incremental invalidation was needed at all.
   **A review then found the reader-visible half STILL didn't land** (fixed, `c3314ba`), and how it
   survived a live browser check is the durable part: the refresh was gated `!structural`, which also
   skipped the rebuild — so it worked for an in-place write and failed for delete+recreate
   (`git checkout`, editors that unlink first). **The reviewer predicted it for save-via-rename and
   was wrong about that trigger** (Linux `mv` emits `Modify(Name(To))`, not `Create`), **and right
   about the mechanism.** Take a review's *mechanism* seriously even when its example doesn't
   reproduce: go find the trigger yourself. One browser measurement is one code path.
3. **Cross-reference labels are English-only** (re-filed 2026-07-17; **was "`lang: fr` promises
   French, delivers English (`render/page.rs:239`)", and both halves were wrong**). `page.rs:239` is
   *correct code*: `<html lang="{lang}">` fed by `doc.lang`, doing exactly its job. The true site is
   `cite/render.rs:15-21`, a hardcoded English const table — and `lang` appears **zero** times in
   that whole file, so there is no localization seam to fix. Nor is the "promise" real: the docs and
   `vocab.rs` only ever promise `lang:` sets `<html lang>`, which it does. **So this is an absent
   i18n FEATURE with a scope question, not a small live defect** (no corpus doc demands it). A
   textbook wrong-*layer* pointer: the entry named a real symptom and a file that is not the cause.
4. ~~**A cross-page `@fig-`/`@sec-` is indexed WITHOUT its number.**~~ **LANDED 2026-07-17**
   (`4b35bb5`, with #2 — one seam, one change). The entry's "reorder or a second pass" was right
   about the ordering and **incomplete about the cause: it is TWO causes, not one.** The reorder
   alone is not enough — `page_fragment` renders each page ALONE, and a single-doc render cannot
   know a cross-page number, so the marker survives unresolved no matter when you build the index.
   It now resolves through the same `xref::resolve_blocks` the served page uses, so the two cannot
   disagree. Verified live: `/search-index.js` carries the post-edit "Figure 1.2".
   *The entry's own grep trap is worth keeping: its quoted phrase spans a line break in both source
   and HTML, so a single-line grep returns nothing and it reads like rot. Flatten newlines first.*
   *(Historical: the entity half of this item landed separately, `9e52b71`.)*
   *Grep trap that hid this twice: the entry's own quoted example spans a line break in both
   `methods.tmd:15-16` and the emitted HTML, so a single-line `grep` for the phrase returns
   nothing and the entry reads like rot. Flatten newlines before believing it.*
5. **A labelled `include: false` python/R cell registers an anchor that never exists** (found by the
   adversarial review of the cell-label fix, 2026-07-16). `register_xref` runs *before* the lang
   match (`render/mod.rs:~523`), so `#| label: fig-x` + `#| include: false` registers `fig-x` with a
   number, while `exec.rs:379` (`!cell.include → continue`) drops the output block, so no `id="fig-x"`
   is ever emitted. `@fig-x` then renders a confident numbered link to a fragment that exists nowhere.
   **Pre-existing on the same page** (main has the identical dead link for a same-page `@fig-x`); the
   cell-label fix **widened it to cross-page** and, in doing so, silenced the "broken cross-reference"
   warning that used to fire there — the one diagnostic that flagged it. Only affects python/R:
   mermaid/`{js}` emit their figure at render time, so their anchor is real regardless of `include`.
   The fix is lang-dependent (do not register when the figure is known to never materialize, or warn
   that a labelled `include: false` cell is unreferenceable, mirroring the theorem-prefix warning at
   `render/mod.rs:1699`) and belongs in the render/exec seam, so it wants its own change.
   Re-confirmed 2026-07-17 at `render/mod.rs:525` (`register_xref` precedes the `match lang` at
   `:527`); note the `CellRole::Listing` arm next door **gets this right** (gate `:572` precedes
   register `:578`), so the correct shape already exists in the same function.
   ***"This sandbox has no `ipykernel`" is FALSE*** (2026-07-17): `~/.local/share/qmd-venv/bin/python`
   has it, and the warm forkserver boots (`preloaded: numpy, matplotlib`). Set `TALIESIN_PYTHON` to
   it. This item is verifiable NOW, and so was the false premise blocking it.
6. ~~**A front-matter `title:`-only edit broadcasts nothing**~~ **LANDED 2026-07-17** (`2bd8385`).
   A `title` message now rides after the body, mirroring `theme_changed → style()`; both servers
   send it (the single-doc path had the same hole for any non-deck doc). **Two things the entry got
   wrong, worth keeping because they generalize.** (1) Its mechanism was a special case: measured on
   a live preview, the diff was **not** empty — the body updated (`<h1>` = "CHANGED Title") while
   the tab stayed "Original Title". It broadcast one op and still lost the title, because *no
   non-remount path carries it*; an empty diff is merely the case where nothing is heard at all.
   (2) Its recommended fix (fold into `remount`, as `deck_meta_changed` does) was **rejected**: a
   deck must re-mount because its title slide is structural, but re-mounting an HTML page replaces
   the body and discards every `{js}`/WebGL cell's live state — B3-18's exact defect, paid for a tab
   label. **A neighbouring fix's shape is evidence, not a template: copy it only after checking that
   what forced it there applies here.** Also fixed a second, pre-existing bug it would have hidden:
   `baseTitle` was captured once at load and restored after every build, so assigning
   `document.title` alone reverts on the next save.
7. ~~**A restarted server leaves tabs on stale `client.js` under a green "live" pill**~~ **LANDED
   2026-07-17** (`ca5c3c2`). The boot-mismatch branch now calls `location.reload()` instead of only
   re-mounting (a re-mount replaces the body, never the running `<script>`). **Reproduced with a
   build marker before fixing**: server served v2 while the tab still executed v1 under a pill
   reading "live". After: server v4, tab reloaded onto v4, page intact. **No server change was
   needed** — `boot` already rides on every `full_render`, so the client decides alone; the
   `reload()` *message* stays a server-initiated lever with its own three senders. **Known and
   deliberate**: the tab must already run a client carrying the check, so the *first* restart after
   this shipped still lands stale (old code cannot be taught to reload); it self-corrects after.
   *The check that mattered as much as the fix: a sentinel proving three consecutive edits stay
   incremental and do NOT reload — the failure mode here was an edit-triggered reload loop.*
8. **`seo.rs` emits machine-invalid output with no diagnostic** (executed, same audit). **The
   `<lastmod>` third LANDED 2026-07-17 (`3041f87`); the other two are still open and still
   independent:** `<loc>` is entity-escaped but never URL-escaped (`posts/two words/` -> a raw space,
   and the same URL goes into `llms.txt` where it isn't a CommonMark link); a scheme-less
   `url: ex.com` builds clean and emits `<loc>ex.com/</loc>` + `Sitemap: ex.com/sitemap.xml`. `check`
   reports "no problems found", exit 0, for both. Wants a **diagnostic, not a knob** (the `69c228b`
   value-lint + D37 precedent — and `<lastmod>` now *is* that precedent too).
   **What the landed third proves, because it cost more than its price tag said.** The entry read
   "`rfc3339` exists, `seo.rs` can reach it, it just doesn't call it — close to free". Every clause
   was true and the conclusion was still wrong twice. (1) **Calling it breaks a pin one file over:**
   `sitemap_lists_pages_...` asserts a date-only `<lastmod>`, and is *right* to — the feed's
   `T00:00:00Z` exists only because Atom requires a timestamp, a force that does not travel to the
   sitemap. Share the **validator**, not the format. (2) **`rfc3339` did not enforce what the entry
   credited it with** (`2026-99-99` passed; its `T` fast-path returned *before any check*), so
   "just call it" would have spread a bug while closing the symptom. **The real find:** the entry
   named the sitemap as the victim and the feed as the enforcer. Backwards — 11/11 sitemap dates were
   valid, while `date: Thursday` was *live-publishing* `<updated>Thursday</updated>` into the Atom
   feed under a green `check`. **So "the correct helper is next door, just call it" has a
   precondition nobody checks: that the helper is correct.** Grep for the validator, then *test the
   validator*. Root cause was three readers of `date:` each answering "is this a date" at a different
   strictness; `frontmatter::calendar_date` now owns it (the `yaml_bool_word` shape).
9. ~~**`card.rs` overflow, the fields nobody clamped**~~ **LANDED 2026-07-17** (`935536a`). The
   entry's pricing ("a mechanical clamp ×3, plus a different fix to the shared `wrap()` shrink
   trigger") **held exactly** — the first entry all day whose cost was right. Two things worth
   keeping. (1) **"Clamp ×3" hides a fourth fix**: the wordmark and domain share one footer row, so
   clamping each to the pad box *independently* still lets a long title slide under the domain — the
   entry's own "Learnindgsbogossian.com" symptom is a COLLISION, not an overflow, and no per-field
   clamp fixes it. (2) **The tests are what let all four survive, and one PINNED the bug:**
   `render_card_survives_empty_and_overlong_text` feeds an 80-word eyebrow and asserts only
   `png_dims == (1200,630)` (a card whose text runs off the edge is still 1200×630);
   `wrap_keeps_every_line_within_max_width` checks fit but only for text that *wraps*; and
   `wrap_keeps_an_overlong_word_...` asserted the overflowing word was PRESENT. Between them sits a
   hole shaped exactly like the defect. **A green suite is not coverage; read what the assertions
   actually say.** Tests now decode the PNG and assert no inked pixel escapes `[72, 1128]` — the
   layout maths was the thing that was wrong, so asserting on the layout maths would have been
   marking its own homework. All 16 real OG cards byte-identical (the entry's "0/153" held).
10. ~~**Two minifier latents remain**~~ **LANDED 2026-07-17** (`8d73657`). Both fixed, bypass now
   tested. The entry's *framing* held perfectly — the **whole built blog is byte-identical**
   before/after, every file, so nothing that ships today moved. That is what "latent" earns the
   name for, and it is why this was safe to land. Its *contents* were another matter.
   **It named three regex contexts. Two were padding, and it MISSED three.** `)` and `]` are
   **correct as division** (`(a + b) / 2`, `xs[i] / 2`); a regex can only follow either as an
   expression STATEMENT (`if (x) /re/.test(y)`) whose value is discarded, so "fixing" them would
   misread live division to serve dead code. (Adversarial review tried to refute this and could
   not.) Meanwhile `>`, `>>` and `>>>` are regex context too, and the entry named none of them:
   **`prev == '>'` means the previous token ENDED in `>`, which in plain JS is exactly `>`, `>>`,
   `>>>`, `=>` — all operators, none able to end an expression.** So the arrow was never a special
   case, and the one-line fix is `>` in the punctuator set.
   **My first fix was worse than that, and the comment defending it had the reasoning backwards.**
   I wrote "a bare `>` is ambiguous — `a > b`, `a >= b`, `a >> b`" and built a check on `out`'s
   tail to spot the two-char arrow. None of those examples has a `/` after the `>` at all: at the
   `/` in `a > b / c` the previous significant char is `b`, so the `>` branch never runs. **That
   conflated "is the TOKEN `>` ambiguous" (yes) with "is a `/` FOLLOWING one ambiguous" (no) —
   and the test I wrote to guard it asserted `a > b / c` stays division, a property no
   implementation can break.** It guarded a path it could not reach, and cemented the error.
   Caught by review, not by me; `a > /['"]/.test(b)` still corrupted after my "fix". **When a
   one-char change feels unsafe, write the counter-example down before designing around it — if
   it will not come, the fear is the bug.**
   **The bypass inverted, and that is the find.** The entry treated *"routing `mermaid_bundle_js()`
   through `minify_js` would corrupt it"* as a standing fact that justified the bypass. It was
   true: neutering the fix reproduces it exactly, mermaid failing on **token 476206, count
   identical**. Fixing `${}` depth made it **false** — both vendored bundles now pass the token
   guard through the minifier. **So the bypass was never an invariant; it was a bug report with a
   workaround attached, and the entry had promoted the workaround to a constraint.** Keep the
   bypass (re-minifying a megabyte of already-minified vendor code is build cost for ~nothing) but
   it is now a *choice*, pinned by `vendored_libs_are_written_verbatim_not_reminified`, with
   `minify_js` checked against both bundles anyway — half a million tokens of dense real JS, the
   most adversarial corpus in the repo, and worth more than any fixture I could write.
   **The nested-template bug had a quiet twin the entry missed:** `${...}` bodies were never
   scanned as code at *all*, so every interpolation silently skipped minification (a comment
   inside one shipped). Both halves are pinned, because a "fix" that just made templates opaque
   passes every nested-template test while leaving that half broken. Craft note: `assert_eq!` on
   two megabyte bundles prints **both** on failure (3.5MB), burying the one line that says what
   broke — compare, then report short.

### 3. Needs an owner ruling (not builds)

*(The `--jobs` semantics ruling is CLOSED — N = pages, landed `f141cac`. See the start-here block.)*

- **D34 project defaults** (OWNER-RULING). `bibliography`/`csl`/`execute`/`theme` are absent from the
  19-key `NATIVE_KEYS`, but no corpus doc repeats them across pages, so it fails minimal-config today.
  Recommendation: **subtract before adding**, delete the dead `image:`/`SiteConfig.card_image` field
  (zero readers; its own doc comment concedes it) and defer the defaults until a corpus doc hurts.
- **D70 "Cite this" card** (OWNER-RULING). Its machine-readable half already shipped
  (`.citations.json` + ScholarlyArticle JSON-LD). A card would render **author-free for every current
  post** (0 of 8 tech-blog posts set `author:`).

### 4. Needs Do-NOT-touch sign-off (citation zone)

- **M2-M6 machine-facing audit, exec/kernel half** (**needs sign-off**; detail + evidence:
  [2026-07-16-machine-facing-audit.md](2026-07-16-machine-facing-audit.md)). Audited read-only and
  left unfixed. **M1 LANDED 2026-07-16** (`--jobs` no longer docks for a pool that never boots):
  on inspection it touched only concurrency arithmetic in `build.rs`/`build_budget.rs` — no
  execution semantics, no kernel lifecycle, no freeze keying — so it was **not** in the zone after
  all. That is the third time an entry named this zone and did not need it (after D49 and D67):
  **check the actual blast radius before assuming M2-M6 need sign-off too.** *(Settled 2026-07-17:
  M2 and M3-M5 did need it and were signed off and fixed; M6 was two items, one free-standing and
  one refused. **The whole audit is closed except M6a.**)* Ranked:
  - ~~**M2 `interp_id` wedges the rebuild pipeline forever**~~ — **FIXED 2026-07-17** (`f9eea8d`,
    signed off). Probe is now async + `tokio::time::timeout` (10s, `kill_on_drop`), and only an
    *answer* is memoized (a spawn error or timeout is retried; "ran and printed nothing" still
    caches, so the healthy path stays one fork per process). **Measured end to end against an
    interpreter that hangs forever** (`sleep infinity`): `taliesin-stable` (old) **exit 124, still
    wedged at 200s**; new **exit 0 at 161s**, degrading to "kernel unavailable". **Freeze key proven
    byte-identical**, independently of the agent that wrote it: a `_freeze/` entry written by
    `taliesin-stable` (`df394c6`, the old blocking probe) **replays under the new binary with zero
    kernel boots**, key `58a59a3611fc6ba7` untouched.
    - **NEW, found by that measurement: M2 has a sibling, and the sibling now owns the delay.**
      161s is a long recovery, and **none of it is `interp_id`** (bounded at 10s). The time is
      downstream: the warm-pool forkserver READY wait, then kernel-start retries. A hanging (not
      *missing*) interpreter is the trigger; the test
      `kernel::tests::transient_start_errors_retry_but_missing_interpreter_does_not` shows the
      *missing* case is handled and the *hanging* one is not. **Not
      covered by M2's sign-off** (it is `kernel.rs`/`warm_pool.rs`, not the probe). Needs its own
      ruling. *Only visible end-to-end: a unit test on the probe cannot see it, and it is exactly
      why the first end-to-end attempt looked like M2 had failed when it had not.*
    - **Also found, not fixed:** `crates/server/Cargo.toml` does not list tokio's `process` feature,
      though `kernel.rs`, `warm_pool.rs` and now `exec.rs` all use `tokio::process`. It compiles only
      via feature unification from elsewhere in the graph. Pre-existing and already load-bearing.
  - ~~**M3 refill goes dark** + **M4 `fork_kernel` PID desync** + **M5 `warm_one` `/tmp` leak**~~:
    **FIXED 2026-07-17 as ONE change** (`4520996`, signed off). Correlation ids on the fork protocol
    (`SPAWNED <id> <pid>`) make a late reply self-identifying, so it is skipped and its orphaned
    kernel reclaimed; refill now triggers on a **miss** too (the state that needs re-warming, and
    the gate that made "empty" unescapable); `ConnDirGuard` is armed over the fork window
    (`kernel.rs` exports `arm()`); `FORK_ATTEMPTS` mirrors the cold path's `START_ATTEMPTS` and
    defers to its `start_error_is_transient`. Each of the three new tests was **mutation-checked**
    by restoring its bug. The Tier-2 duplicate of M4 ("cross-call edge") was deleted with it.
    - **NEW, open, and filed rather than buried in that commit: the M4 test's stand-in kernel
      (`sleep 300`) survives 2 of 8 full-suite runs.** Both leaks were on runs that were also
      *compiling*; six runs against a warm build leaked none, and instrumenting the drop hid it
      entirely (a Heisenbug). Ruled out by measurement, so do not re-check these: the drop fires for
      all six daemons every run, always with a helper pid, always with the group alive at kill time
      (`kill(-pgid, 0)` -> rc=0); the survivor is genuinely alive (`/proc` `State: S`), **not** the
      zombie the interrupted session guessed. Unexplained: it sits in its dead leader's group, and a
      child inherits its pgid at fork, so the group SIGKILL should have reached it. **Test-only on
      the evidence** (`sleep 300` exists solely as that stand-in, deliberately unowned to prove the
      reclaim is surgical, and it self-exits in 5 min); a real kernel has three nets where it has
      one (`Kernel`'s own SIGKILL-on-drop, `ForkedCleanup`, then the group kill). Worth an hour only
      if a real kernel is ever seen outliving its pool.
  - **M6 is TWO items, and they land on opposite sides of the line** (split 2026-07-17; it was
    filed as one because the summary said "a constant and a `/proc` probe"):
    - **M6a `MAX_WARM_PAGES = 6` sits outside the budget built to bound it** (`serve_site/exec_pool.rs:14`
      — a file the entry never named). **NEEDS SIGN-OFF, despite looking like a constant.** Eviction
      at `:87` drops the executor, which *kills its kernel child processes* (`:17`), destroying that
      page's kernel variable state and forcing a cold replay; `:3` states the eviction order must
      stay deterministic because the build relies on it. That is kernel lifecycle.
    - **M6b the RAM probe fails OPEN in a container** (`build_budget.rs:36-46`: host-wide
      `/proc/meminfo` while `available_parallelism` honours cgroup CPU quota). **FREE-STANDING, no
      sign-off**: a file read returning `Option<u64>`, exactly M1's shape. Only affects auto mode
      (an explicit `--jobs` never consults memory). `probe_free_mb` has zero tests.
  *(Two entries that named the citation zone — D49, D67 — turned out not to need it. Check before
  assuming these do; but M1-M6 were all read-only-audited precisely because they do.)*

- **D72 bare `@key`** (ADOPT in principle; edits `crates/core/src/cite/`, needs sign-off). Support
  bare `@key` at all? **DECLINED for now** in the 2026-07-17 ruling round: the *diagnostic* shipped
  2026-07-16 (`8a45d59`), so nothing renders wrong silently, which makes this a feature question
  rather than a defect. *(D69 landed 2026-07-17, `acfbe8f`: `any` became `position`, so one
  `Option<usize>` now drives both decisions it always implied, the suppressed `<h2>` and an
  `insert(i + 1)`. It had been right by luck in all three corpus docs that carry the heading,
  because each ends with it.)*

### 5. Deliberately deferred

- **B3-18** (the last deck-audit item; detail:
  [2026-07-12-deck-audit.md](2026-07-12-deck-audit.md)): a structural deck edit re-mounts the *whole*
  deck, nuking every `{js}`/WebGL widget's state; re-mount only the edited `<section>` subtree.
  Deferred on purpose (touches the client's re-mount path; bigger blast radius). Everything else in
  that audit has landed — see [AUDITS.md](AUDITS.md).

### Consulting the Quarto catalog (policy, not a task)

**Owner ruling 2026-07-16: no sweep. Triage an area on demand, when you next work that area.** Wave
1 measured the base: **12 of 34 (35%) outright stale or superseded, 20 of 34 (59%) carry at least
one false statement about today's source**.
**Before consulting it, read the triage doc's "three layers" section** —
[2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md).
In short: the entries are the asset and were well-grounded on 2026-07-03; the **heading status is
degenerate** (162/165 skeptic verdicts are `revise`, so 94 read "Proposed (revised)" regardless of
conclusion); and the **executive summary is misleading** (it describes a per-entry tag scheme that
does not exist, miscounts, and its "rule on these first" list mixes open questions with
already-shipped work). A skeptic verdict is evidence, never a ruling: D135's skeptic insisted on
dropping Atom feeds as "a documented non-goal" and Atom shipped anyway, with autodiscovery.

## Tier 2 — hardening (P3)

- **Execution-cache leaks — remainder** (exec/kernel Do-NOT-touch, careful):
  - **Ungraceful-death path (S/M):** no defense vs SIGKILL / closed terminal / crash. Absent:
    `PR_SET_PDEATHSIG` on the warm-pool helper (it has its own process group, so cheap), and a
    startup sweep of stale `/tmp/tali-warmpool-*` / `/tmp/tali-kernel-*` dirs whose owner pid is dead.
    (Measured: `kill -9` on a preview orphaned 8 procs / 451 MB + 123 `/tmp/tali-*` dirs.)
  - **Flaky timing tests** (LOAD-sensitive):
    `exec::tests::pooled_kernel_serves_cells_without_a_long_warming_state` +
    `kernel::tests::kernel_executes_state_errors_and_interrupts_runaway_cell` fail under CPU load;
    both assert on **timing**. Fix: wait on a **state signal**, not a duration.
    - **A THIRD + FOURTH one, and they are NOT timing at all.**
      `exec::tests::a_successful_probe_pins_the_freeze_key_format` +
      `exec::tests::a_failed_interp_probe_is_not_memoized_for_the_process_lifetime`. Filed
      2026-07-17 as a load-sensitive observation, and **that entry did the right thing: it refused
      to diagnose without the panic message and said "reproduce it and read the assertion first".
      Doing exactly that inverted it.** *(Left in this LOAD-sensitive bullet on purpose — the two
      above are still timing; these two are not, and the grouping is what misled.)*
      **Measured 2026-07-17, on pristine `main` (so it is nobody's change):** fails ~2 runs in 3 on
      an **idle** machine, in a full `--bins` run; **never** fails filtered to `exec::tests`, and
      **passes 3/3 under `--test-threads=1`** — which takes **6.4s vs 1.6s parallel**. *More
      wall-clock per test and no failure: that refutes load/timing outright.* It is a **concurrency
      race**, and it needs the rest of the suite present, so the two tests are victims, not causes.
      **The assertion, captured at last:** the freeze key's interpreter-id segment comes back
      **empty** — `left: "python::/tmp/tali-interp-fmt-<uuid>/nonzero-exit::"` vs
      `right: "...::Python 3.1.2"`. In `probe_interp_id` (`exec.rs:1013`) the version is
      `answer.unwrap_or_default()`, and `probe_version` returns `None` for exactly two reasons:
      **spawn failed, or it hung past `bound`.** `bound` is 10s and the whole parallel suite
      finishes in 1.6s, so **the timeout cannot have fired — the spawn failed.** The memo is not
      the culprit either: each test writes its stubs under a fresh uuid dir, so no key collides.
      **Leading hypothesis, NOT verified: `ETXTBSY`.** `write_exe` (`:1228`) does `fs::write` then
      `set_permissions`, and the suite forks constantly across tokio threads; a child forked while
      another thread holds a write-fd to a stub inherits that fd, and any exec of that stub before
      the child execs fails with "Text file busy" — the classic write-then-exec race in a
      multithreaded process. `probe_version` swallows the spawn error into `None`, which is why
      this was invisible for a day. **Do not fix from this note either** (exec/kernel zone, and the
      hypothesis is unproven): the cheap first move is to make `probe_version` log *why* it
      returned `None`, and re-run the full suite until it trips.
  - `build.rs:926` warms the pool before knowing any page needs a kernel, even under
    `TALIESIN_NO_EXEC=1`. Hygiene, not perf (0.25 s vs 0.27 s on a prose-only site).
  - R stream/stderr leaks raw ANSI into HTML (`kernel.rs` `Output::Stream` emits `esc(text)` with no
    `strip_ansi`, do-not-touch).
- **Interpreter selection is silent + has no project-local override (DX; S+M).** Resolved once at
  `exec.rs:217` (`TALIESIN_PYTHON` else `python3`; `TALIESIN_R` else `R`). Two gaps bit a real user
  (2026-07-11: a global `TALIESIN_PYTHON` in `~/.zshrc` errored a whole book's ~35 cells):
  - **No "which python?" signal (S, highest-leverage).** A dep-less interpreter is indistinguishable
    from a code error. Log `executing cells with <abs path>` at build start, and/or a `taliesin
    check` reporting interpreter + `ipykernel` presence (like `quarto check`). Lives in the
    build/serve entry, not the Do-NOT-touch core.
  - **No project-local declaration (M).** Add a `python:` / `r:` field in `_site.yml` (parsed in
    `schema.rs`/`frontmatter.rs`, threaded into `Executor::build`), and/or auto-detect a sibling
    `.venv/bin/python` when the env var is unset. Env var stays the override; the field wins for
    reproducibility. (Downstream `invertible-speech-disentanglement` BUG-002.)
- **`assets/js/*` `tsc`/`@ts-check` pass** (own large session). The web-client tier is done + in CI;
  remaining is `crates/core/assets/js` (measured 812 errors on a throwaway strict jsconfig; `deck.js`
  402). Needs ambient globals + a config compiling the concatenated `code-enhance/` fragments as one
  shared scope (isolated compile adds 12 spurious `TS2304`s).
- **Mermaid `<script>` SRI + `crossorigin`** — deferred (only live Preview lazy-loads from the CDN; a
  build inlines the vendored copy). Needs a hash pinned to the CDN build; `integrity`+`crossorigin`
  would break a non-CORS `TALIESIN_MERMAID_URL` override.
- **Deck engine (P2):** drop `fitSlide` from the resize path (needs a lazy fit-on-show refactor
  first); mobile pinch/pan + touch gestures (hard to verify without a device); thread
  `footer:`/`logo:` through both deck-page builders (no corpus deck needs one yet).
- **Perf (low):** protocol-level op-message batching (one WS message per save, not per-op). Worst
  case: an edit near the top of a long doc where every downstream block emits a `SetMeta` (`diff.rs`
  `anchor_op`). Client + server ship together, no wire-compat constraint.
- **Audit long-tail** (`AUDITS.md`): a tens-of-MB cell output blocks ZMQ receive before the cap fires
  (`kernel.rs`, Do-NOT-touch).

- **AI-native authoring — packaging + guardrails** (detail: [2026-07-12-ai-native-backlog.md](2026-07-12-ai-native-backlog.md); anchors verified). Tier-2 slice of the §G initiative:
  - **`taliesin map --format json`** (M) — one-call project outline (pages/nav/drafts/xref-graph/mounts) for agent planning; mirror `cmd_symbols` (`query.rs:232`), reuse `Site::discover`. Pin: `tests/map_cli.rs` over `corpus/demo-book`.
  - **Correct-by-construction scaffolds + `--json` on `new`/`init`** (S–M) — a citation-wired `paper` kind (`bibliography:` + `[@key]` + shipped `references.bib`) and machine-readable create output; seam `cli.rs:178` `new_files()`. Pin: byte-pin `corpus/scaffold/posts/my-paper/` via `cli.rs:658`.
  - **Sharpen `check` as the LLM-mistake catcher** (L, sliced) — default-on placeholder-alt nudge (do first; `a11y.rs:284` + `helpers::tag_attr`), opt-in numeric-claim-without-citation hint (`prose.rs:55`), opt-in `check --online` DOI check (the sole sanctioned egress; needs a small additive read-only accessor on `cite::Bibliography`). Pin: `corpus/diagnostics/llm-mistakes.tmd`.
  - **`build`/`publish` structured errors (`--format json`)** (M) — retain the already-computed `page_static_diagnostics` as structured `Diagnostic`s (reuse `check.rs` shape) instead of logging+dropping; coupled edit across `build.rs` + `publish.rs` (`run_site_build:868`). Pin: `tests/structured_build_errors.rs`.
  - **Taliesin Claude Code skill/plugin** (S–M, soft dep §G#1) — a distributable `taliesin` skill (loop + dialect crib + source-not-preview rule) driving the CLI, pinned against the live binary (`tests/skill_freshness.rs`) so it can't rot like the retired external scaffolder.

## Tier 3 — deferred / demand-driven

- **Companion (Phase 2):** editor commands (`.tmd`-buffer text transforms only, never preview
  gestures); `editor.wordWrap` default for `[taliesin]`; grammar polish (YAML-type `#|`/`//|`/`%%|`
  values; recommend cell-language extensions via `.vscode/extensions.json`); **marketplace packaging
  hygiene** (`.vscodeignore` misses `.vscode-test/` (1.8 GB), `test-fixtures/`, `scripts/`,
  `out/test/`, `out/e2e/`; no top-level `icon`/`repository`/`license`/`keywords`; `"private": true`
  blocks publish). `symbolCache` only invalidates on save (`completions.ts`, low) — an out-of-band
  change lags until the next save; bounded + graceful, noted so it isn't re-discovered.
- **`.tmd` format-on-save** (open question): a source pretty-printer must preserve `data-sourcepos`
  line stability for click-to-source — brainstorm reflow-vs-risk first.
- **Dogfood: migrate the FL-weather book to Taliesin** — a real Quarto→Taliesin migration +
  portability stress test; pin a reduced version under `corpus/` if it renders clean.
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic).
- **`taliesin publish` follow-ups:** optional `--init` wrapper for the one-time `wrangler` setup;
  email-allowlist (Cloudflare Access) mode.
- **Interactive/explorable numerics** (`FEATURE-IDEAS.md` #62-66; none pinned — promote with a corpus
  pin when one graduates; must NOT reintroduce a reactive VM). Highest-leverage: **#62** a bundled
  numerics/stats global for `{js}` + **#63** `animate`/play-tick + draggable-`point` `{{< input >}}`.
- **Wave 5** (`ROADMAP.md`): print-pdf track (paged render *of* the built HTML), docs-as-spec,
  `{glsl}` cell language, SEO completeness. **Fold `llms.txt`/`llms-full.txt`** in (the block model
  separates clean prose from code/math at `client.js:50`, so it'd be more accurate than the old
  scraper). *Pin: a `tech_blog.rs` assertion that `llms.txt` lists discovered pages + `llms-full.txt`
  excludes drafts.*
- **Site-level shared bibliography + hygiene** (M). `bibliography:` is per-document only
  (`cite/mod.rs:42`). Allow it in `_site.yml`, merged under each page's; add two read-only diagnostics
  ("entry never cited", "duplicate key") over the parsed registry (does NOT touch the BibTeX/CSL
  Do-NOT-touch core). *Pin: a small site, one entry cited from two pages, one uncited.*
- **Author structure panel** (M/L). A read-only preview sidebar: the heading tree with per-section
  word count (`client.js:50-58` already counts) + a badge per node for unresolved xref / TODO /
  over-goal length. Click to scroll; move the editor cursor via cursor sync under the companion. An
  annotation layer on the dev panel, not a new component. *Pin: `corpus/layout/structure.tmd`.*
- **Session revision digest** (M). Surface the `BlockOp` stream the client already receives: a
  session word delta + a feed of the last N ops, each click-to-source. (Also the home for the cut
  "cross-revision what-changed" idea if it's ever revived.) Behavioral pin (a `tools/live-edit-bench`
  assertion), not a corpus doc.
- **Block-level transclusion** `{{< include file.tmd#sec-id >}}` (M). Reuse a section across a series.
  Must ride **on top of** the `includes.rs` source-map pass (resolve fragment → block range, hand a
  sub-slice), never rewrite it. Hard gate: the source map must not perturb. Defer until a series needs
  it.
- **LSP for the language intelligence** (L). Everything an LSP needs is already in Rust (`check`,
  `vocab`, `register_xref`, bib parser, `closest()`); write-once for Neovim/Helix/Zed/VS Code, removes
  the `#| label:` completion drift. The preview stays the view (editor-agnostic; two `postMessage`
  shapes in `docs/internals/protocol.tmd:325-350`). Do NOT rebuild the preview as an LSP.
- **Image optimization** (WebP/AVIF + `srcset` + lazy-load behind a content-hashed cache) — until
  posts get image-heavy.
- **Marketing site** (deferred, feature-first; rolls into a demo-machine rebuild):
  `live-edit-hero-demo` clip; swap `site/_site.yml` placeholders; demo-led hero rebuild (with a
  3-viewport spot-check of the already-fixed 390px hero overflow + theme/video desync, plus any
  leftover em dashes); **#12 demo video needs a pause affordance (WCAG 2.2.2) + reduced-motion
  respect** and its baked-in desktop text downscales ~3x on mobile (re-record or ship a mobile
  source); mobile embed refine; deploy.
- **`serde_yaml` fallback watch-item:** if 0.9 breaks against a future serde/edition, swap to
  `serde_yaml_ng` (v0.10), gated on a test that `Error::location().line()` still works. Fix the stale
  `Cargo.toml` comment (names the unsound `serde_yml`) when touched.


## Decided against / do-not-re-litigate

**2026-07-12 rulings (don't re-open):** the feature-idea wishlist (cross-revision diff, repro
manifest, List-of-Figures/Tables/Theorems, interactive tables, "Cite this", line-level code xrefs,
image `dark=`) → **cut to `FEATURE-IDEAS.md`** (revive only when a corpus doc needs one). Reader
text-size/line-spacing controls → **declined for now** (a11y-exempt substrate exists in
`14-reader-prefs.js`; revisit if requested). Twinned `fourier-transform` post dirs git-tracking
anomaly → **left as-is**. Stale `new-post`/`new-project` scaffolder skills → **retired** (done this
session; the `deploy` skill stays).

**TODO / FIXME surfacing — owner ruled skip (2026-07-10).** No `level` concept exists
(`render::Warning` / `check::Diagnostic` / `protocol::Diagnostic` know only warning|error, and the
warning channel is a hard gate), so a TODO warning would fail `check` on every draft. If ever
revived: design A (preview-only `Diagnostic::info` at `serve/mod.rs::compute_diagnostics`, cannot
reach the gate) beats design B (re-plumb a real `level` through the whole gate). The scan must NOT
reuse `prose::strip_inline` (blanks code, where TODOs live); pin any fixture in `corpus/diagnostics/`.

**Refuted by measurement — do NOT re-scope:** `build` does not leak forkserver subtrees (graceful
path reaped 2026-07-08; the gap is the *ungraceful* path, Tier 2); the warm pool booting Python on
prose-only builds is hygiene, not latency (0.25 vs 0.27 s); dev attributes are 0.29% of page bytes
(don't strip); a `--version -dirty` marker computed in `build.rs` is stale-by-construction (refused);
the `assets/css` stale-embed claim did not reproduce (re-verify for `assets/js` before the
touch-render workaround); the 390px `hero:` overflow + theme/video desync are already fixed in code;
include symlink-loop SIGABRT does not exist (Linux caps at `MAXSYMLINKS=40`; includes are
author-local).

**Gate the gate:** a drift test that cannot fail is worse than none. Two of three Batch-F drift gates
couldn't fail on first draft. Any new drift gate must be mutation-checked against exactly the shape it
guards.

**AI-native leftovers, owner ruled decline 2026-07-16 (don't re-open).** The other 8.5 of §G's 10
items shipped 2026-07-13; these three were the ruling-gated remainder, declined on the evidence:
- **`check --online` citation resolution** (§G#8a) → **declined.** It is the only proposed network
  egress in the tool: the workspace carries no HTTP client dep and `CHECK_FLAGS` is `["--format"]`
  only, so this buys a link-rot check at the cost of the offline invariant. Revive only if a real
  workflow demands it, and then check-only, off by default, never reachable from `build`/`publish`
  (`build.rs` shares `page_static_diagnostics`, so a network call there would make builds phone home).
- **Numeric/quoted-claim-without-citation hint** (§G#8c) → **declined.** Its own spec rates it
  FP-prone and recommends default-off. A linter that cries wolf gets switched off, taking the good
  rules with it.
- **Per-page text/JSON sidecar** (§G#9A) → **declined as redundant.** It was specified to reuse §G#2's
  projection; #2 shipped as `taliesin read`, and site-level `llms.txt`/`llms-full.txt` already ship.
  Revive only when a named consumer asks for a per-page file.

**Library outsourcing — decided against** (each verified vs the invariants): hayagriva/biblatex,
schemars, jsonschema, morphdom/idiomorph, similar/dissimilar, clap, owo-colors, slug, html-escape,
lightningcss/palette, IntersectionObserver/scrollspy libs, deck micro-helpers. Keep `two_face` extras
filling gaps only (the bundled syntect set is consulted first and must win — `extra_newlines()` is
bat's own curated set, different scope spans, NOT a superset).

**Reading-first defaults — research-validated keeps** (do NOT "fix"): serif body for long-form screen
reading; ~70ch measure `--tali-maxw: 46rem`; right-rail scrollspy + width-gated sidenotes; scroll
(not pagination) book reading; if a serif webfont is bundled, ship REAL bold/italic faces (see item
13), never synthesized. *Caveat:* the competitor framing (Stripe/Linear/Mintlify/…) is unverified
judgment.

**2026-07-06 decisions:** book pager stays bottom-only; book page-TOC fix-in-place, keep both nav
surfaces; xref graph tool removed; focus mode stays ephemeral; deck overview keeps per-slide
backgrounds; dev-menu + `#tali-progress` + reading-progress bar stay three separate signals
(`#tali-progress` is the exec chip, not a reading chip).

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Open-source the repo + publish the site when
ready; the security token gate is shipped.
