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
exec/kernel zone + the single-editing-surface invariant. Review subagents use read-only git.
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
binaries at the first failure**, so a flake (see Tier 2's two load-sensitive timing tests, which
fire when something else is using the CPU) hides every later binary's result: re-run before reading
a total.

**What is left is a flat list; none of it is a grind chunk.** All **three owner rulings are
CLOSED**, both **ruling rounds are spent**, and the **whole M2-M6 exec/kernel audit is finished
except M6a, whose sign-off was refused** (2026-07-17). What remains is §2's ten small live defects,
D70 (unruled), D72 (declined), and one deliberate deferral. Everything else is Tier 2/3
(demand-driven). **There is no gated work waiting on the owner: the next session can just build.**

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

**Pick up here (2026-07-17, latest session — the search-index fix):**
1. **Start with §2 #7 (stale `client.js` under a green pill)** — now the only item left that is
   genuinely **one small change**: wire the boot-mismatch branch (`client.js:~950`, which today only
   re-mounts) to the `reload()` lever that already has three live senders. Purely client-side; no
   JS test harness exists, so it is browser-verified only. #8/#9/#10 are each 2-3 independent fixes
   — fine to take, but price them first and do not promise them as one. *(#6 landed, `2bd8385`.)*
2. **§2 is now EIGHT items** (#4's entity half landed `9e52b71`, #6 landed `2bd8385`). Do not trust
   that count either — recount.
3. **When an entry says "path X already fixes exactly this", that is evidence, not a template.**
   #6's entry pointed at `deck_meta_changed` and the fix it implied (force a re-mount) was wrong:
   the deck re-mounts because its title slide is *structural*, a reason that does not transfer to an
   HTML page, where a re-mount discards every live `{js}` cell. Check what *forced* the neighbouring
   shape before copying it.
3. **The lesson that keeps paying: read the helper the entry points AWAY from.** #4 named one defect
   and the function held four; the other three were invisible to the entry because it blamed the
   symptom's location instead of the code next door that already did the job right. **`grep` for the
   correct helper before writing one** — it has now existed already in 5 of 5 cases (`search.rs` vs
   `render/text.rs`, `llms.rs`, `meta.rs`, `minify.rs`'s own JS branch, and `seo.rs` vs `feed.rs`'s
   `rfc3339`, still uncalled today).

**Pick up here (2026-07-17, after the M-audit was finished):**
1. **Sections 1 and 4's exec/kernel half are now EMPTY.** M1, M2, M3+M4+M5 and M6b have all landed;
   **M6a (`MAX_WARM_PAGES`) is the only exec/kernel item left and its sign-off was REFUSED**, so it
   is not available to pick up. D69 landed; D72 is declined. **Section 2 (live defects, now 9) is
   what is actually left**, plus D70, which is unruled.
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
per-item cost notes before promising anything: **#8 is three independent fixes**, **#9 is two**
(a mechanical clamp ×3, plus a different fix to the shared `wrap()` shrink trigger), and **#10 is
three** (only the `=>` case is cheap; `)`/`]` need real context tracking). #6 and #7 are genuinely
one small change each and are the two cleanest builds on this list. **The recurring finding is not
drift, it is under-pricing:** an entry's summary counts symptoms, and symptoms do not map 1:1 to
changes.*

1. **Duplicate-label warnings are unlocated** (`render/mod.rs:1568-1571`, `site/xref.rs:50-56` emit
   no file/line), half-reproducing the exact Quarto flaw D53 critiques. *(The harvest's own duplicate
   warning, added 2026-07-16, is unlocated for the same reason: `site/mod.rs:953`.)* **Price it
   before promising it (2026-07-17): this reads as one small fix and is TWO, with very different
   costs.** `render/mod.rs:1568` is on the `Vec<Warning>` channel, which already has `.at(file,
   line)` (`render/model.rs:166`) — cheap. But `site/xref.rs:23` and `site/mod.rs:172` are
   `Vec<String>`: **no location field exists in that channel at all**, so half of this item is a
   channel type change. (Line numbers here had drifted by 30 and 6; the symptom held.)
2. **The xref registry goes stale on a warm content edit** (`serve_site/mod.rs:1186-1204` refreshes
   only the Cmd-K search fragment). Confirmed: `harvest_xref_numbers` has exactly one caller,
   `site/mod.rs:392`, inside `discover` — and `xref` appears nowhere in `crates/server/src/serve_site/`.
3. **Cross-reference labels are English-only** (re-filed 2026-07-17; **was "`lang: fr` promises
   French, delivers English (`render/page.rs:239`)", and both halves were wrong**). `page.rs:239` is
   *correct code*: `<html lang="{lang}">` fed by `doc.lang`, doing exactly its job. The true site is
   `cite/render.rs:15-21`, a hardcoded English const table — and `lang` appears **zero** times in
   that whole file, so there is no localization seam to fix. Nor is the "promise" real: the docs and
   `vocab.rs` only ever promise `lang:` sets `<html lang>`, which it does. **So this is an absent
   i18n FEATURE with a scope question, not a small live defect** (no corpus doc demands it). A
   textbook wrong-*layer* pointer: the entry named a real symptom and a file that is not the cause.
4. **A cross-page `@fig-`/`@sec-` is indexed WITHOUT its number.** *(This is the remaining half of
   the old "Cmd-K index stores raw `&nbsp;`" item. The entity half LANDED 2026-07-17, `9e52b71` —
   and its stated cause, "the text extraction never decodes entities", was wrong: the extraction
   decoded five entities and simply lacked `&nbsp;`. Reading the helper it pointed away from found
   **three more** defects in the same function — KaTeX math indexed three times with its raw TeX,
   a `>` inside a quoted attribute ending the tag early, and `&amp;lt;` double-decoding to `<`.
   `section_text` now delegates to the shared `render::indexable_text`.)*
   **Measured on `corpus/demo-book`, so the symptom is real:** the page reads "refines the chapter
   overview from Figure&nbsp;1.1 into the steps", the index reads "…from Figure into the steps" —
   the snippet contradicts the page it points at, and the number is unsearchable.
   **The old entry's cause was wrong and would send you to the wrong file:** this does NOT live in
   `search.rs`'s text extraction. It is an **ordering** fact — `search::build_sections` runs at
   `site/mod.rs:368`, but `harvest_xref_numbers()`, the only thing that fills `xref_targets`, runs
   at `:392`; and `rewrite_cross_refs` has exactly one caller (`:880`, the page-render path). The
   targets **do not exist yet** when the index is built, so the fix is a reorder or a second pass
   over the fragments, **not** a call you can drop in. Checked and leave alone: `build_hover_index`
   (`:395`) already sits on the right side of that line, and its `&nbsp;` are *correct* (it stores
   HTML for a card, not text to search).
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
7. **A restarted server leaves tabs on stale `client.js` under a green "live" pill** (same audit).
   No protocol version; `boot_id` detects the restart (`client.js:943-955`) and only forces a
   re-mount; `CLIENT_JS` is `include_str!`-compiled. Same trap CLAUDE.md warns about for assets,
   except the server *knows* it restarted. **Correction (2026-07-17): this entry said the `reload()`
   lever "is unused" — false.** It has three live senders (`serve_site/mod.rs:835`, `:1216`,
   `serve/mod.rs:1086`) plus a protocol test. It is unwired *to the boot-mismatch path* only, so the
   fix is narrower than the entry implies: wire that one path to the existing lever. Someone
   trusting "unused" would go hunting a dead function that is not dead.
8. **`seo.rs` emits machine-invalid output with no diagnostic** (executed, same audit).
   `<lastmod>` is verbatim (`date: "May 15, 2026"` ships as-is; W3C Datetime needs zero-padded
   `YYYY-MM-DD`, and `feed.rs` *does* enforce RFC-3339); `<loc>` is entity-escaped but never
   URL-escaped (`posts/two words/` -> a raw space, and the same URL goes into `llms.txt` where it
   isn't a CommonMark link); a scheme-less `url: ex.com` builds clean and emits `<loc>ex.com/</loc>`
   + `Sitemap: ex.com/sitemap.xml`. `check` reports "no problems found", exit 0, in all three cases.
   11/11 lastmods valid today: latent traps, not live breaks. Wants a **diagnostic, not a knob**
   (the `69c228b` value-lint + D37 precedent).
   **Priced 2026-07-17: this is THREE independent fixes, not one** (different functions, no overlap):
   the `<lastmod>` normalize, the `<loc>` percent-encode, and the scheme check. **The `<lastmod>` half
   is nearly free and nobody noticed:** `feed.rs:185-204` already exports `pub(crate) fn rfc3339`,
   already used by the Atom feed at `:130`/`:158`, and `seo.rs` is a sibling that **can already reach
   it** (`super::feed::rfc3339`, no visibility change). `seo.rs:24-26` just doesn't call it — `esc` is
   `escape_attr`, an XML escaper, not a date normalizer. **That is the same "correct helper next door,
   not called" shape as the search-index fix** (`9e52b71`) and the three the machine-facing audit
   found: when an entry says output is malformed, grep for the validator before writing one.
9. **`card.rs` overflow, the fields nobody clamped** (rendered, same audit; the headline ellipsis +
   the glyph-coverage diagnostic LANDED 2026-07-16, this is the remainder). Eyebrow, wordmark and
   domain get no wrap/truncate/width check at all (`card.rs:358-361`, `:399-409`, `:411-414`): a long
   site title overlaps the wordmark into the right-aligned domain ("Learnindgsbogossian.com"), and a
   long domain drives x negative and clips left. Separately, an over-long single *word* clips
   mid-glyph past the pad edge (`:134-154`: `wrap`'s `cur.is_empty()` guard accepts any first word,
   and the shrink only fires above 3 lines, which one long word never reaches) —
   `NullPointerExceptionHandlerFactory` clips at x=1199 against a 1128 pad edge, and the test
   `wrap_keeps_an_overlong_word_on_its_own_line` **asserts** this without checking fit. 0/153 live
   fields today.
10. **Two minifier latents remain** (proven, same audit; the acorn token-equivalence guard LANDED
   2026-07-16, so they can no longer ship *silently* — but the minifier is still wrong for these
   inputs). A regex literal after `=>`/`)`/`]` is read as division (`minify.rs:102-110`); if its body
   holds a quote it flips quote parity for the rest of the file and can truncate a later string
   literal, **killing all of `app.js`** (trigger `s => /['"]/.test(x)` is an ordinary escaping
   idiom). Nested template literals are rewritten (`:252-283`, no `${}` depth) — proven on real
   mermaid, **token count identical**. The guard covers `core_enhance_js()` only, so the
   **vendored-lib bypass is still unenforced** (`build.rs:1102-1104` is a *comment* with no test):
   routing `mermaid_bundle_js()` through `minify_js` would corrupt it outside the guard's reach.

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
    - **A THIRD one, observed 2026-07-17: `exec::tests::a_successful_probe_pins_the_freeze_key_format`.**
      Failed once in a full gate-3 run while a Chrome instance + a local HTTP server were loading the
      machine; **passed in isolation and in a full re-run on a quiet machine** (205 passed, 0 failed).
      It awaits `probe_interp_id(..., Duration::from_secs(10))` against a stand-in `#!/bin/sh` script,
      so a load-starved probe tripping its own 10s timeout is the plausible mechanism — **but the panic
      message was not captured, so this is an OBSERVATION, not a diagnosis. Do not "fix" it from this
      note; reproduce it under load and read the assertion first.** Filed because the set was believed
      to be two, and because `cargo test` aborts later binaries at the first failure — this one hid a
      whole gate's result and looked at first like the search change had broken the exec zone.
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
