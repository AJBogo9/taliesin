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

**EVERY section (A through G) is now closed.** A (blog identity) finished with #7 draft-aware preview
(2026-07-16). B (publish hardening) was **backlog rot** — all three items were already shipped by
the author; entries deleted with evidence (see the note in section A). C (theme/a11y follow-ups)
finished 2026-07-16: six items built, two more were rot (see the note in section C). **D (reading-first
identity) closed 2026-07-16 and was also rot**: two of its three "re-verified" layout targets
dissolved on inspection, and the direction ruling it was blocked on turned out to be a question about
a fork that does not exist (see §D). F (the deck audit) is fully landed except the deliberately-deferred
B3-18. G (AI-native authoring) was **backlog rot**: 8 of its 10 items plus two slices of a third
shipped 2026-07-13, and the owner declined the three ruling-gated leftovers on 2026-07-16. E (catalog
triage) closed 2026-07-16 after wave 1 measured the catalog at 35% stale; the owner ruled **triage on
demand** instead of sweeping the remaining 131. **What remains is a flat list of items, not sections:
§E's ruling-ready leftovers + 5 live defects.** → See "Next session: start here" below.

**Before picking any item: grep its named symbol/flag in source first, and prefer measuring the
running product over reading this file.** The author pushes work mid-session, so an entry can go
stale with no signal here (that is how B, D and G all rotted). Trust an item's described *symptom*,
never its cause or line number. **An entry marked "verified against source" is not enough**: §D's
layout targets carried exactly that label and were quoting a real CSS line that a rule ten lines
below already overrode. A browser measurement dissolved two of them in minutes.

**Working method:** branch per feature; brainstorm if there's a fork; spec under
`docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
extension harnesses); fast-forward merge locally; delete the item here. Agents commit + ff-merge to
local `main` on request; push to `origin/main` only when the author asks. **Do-NOT-touch:** the
exec/kernel zone + the single-editing-surface invariant. Review subagents use read-only git.
**Author policy (feature-first):** finish framework features before marketing-site work.

## Next session: start here

**Git.** Do not trust a SHA written here; any commit that records one falsifies it. **Check, do not
read:** `git log --oneline origin/main..main` for what is unpushed (the author pushes, not the
agent), and `git reflog show origin/main` before believing ANY "not pushed" claim, including one in
a session handoff. The author pushes mid-session with no signal in this file, and a handoff has now
been wrong about this **three** times: on 2026-07-16 a handoff said "+6 unpushed" and the author had
already pushed all six, then pushed **twice more mid-session** while work was in flight. Re-run the
checks too, do not assume: `cargo test -p taliesin-core` + `-p taliesin-server`, `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`.

**There are no open sections left.** A through G are all closed. What remains is a flat list, and
none of it is a grind chunk. In recommended order:

1. **The §E leftovers.** Each is verified and ruling-ready. **D37** (lint `format:` sub-keys) is the
   cleanest build: a diagnostic, not a knob, following the `69c228b` precedent. **D34** and **D70**
   are OWNER-RULING, not builds. **D72/D69** need Do-NOT-touch citation-zone sign-off.
2. **The 5 live defects** (§E, "Live defects"). Small and independent. #1 (References click-to-source
   lands on line 1) is logged-not-fixed by owner ruling and needs a *design* answer first, not code.
3. **B3-18** (§F), the last deck-audit item, deliberately deferred: a structural deck edit re-mounts
   the whole deck and nukes `{js}`/WebGL widget state.

**Method note that paid off twice on 2026-07-16, use it:** when an entry names a file:line, open the
*running product* before the file. §D's layout targets were labelled "re-verified against source" and
still dissolved under a browser measurement, because the quoted CSS line was real but overridden ten
lines below. Grep traps are live here too: a bare word matches prose (`feature` matched the English
word), `grep | head` reports **head's** exit code (so `|| echo "absent"` never fires and `$?` lies),
and zsh needs `--include='*.tmd'` quoted. See [[grep-verification-traps]].

**Decided this session (2026-07-16), do not re-litigate:** §G's three leftovers (`check --online`,
the numeric-claim hint, the per-page text sidecar) are **declined** (see "Decided against"). §E is
**triage on demand**, not a sweep. **§D is closed as rot** and its "direction ruling" is void: the
fork was false (see §D). The **References** click-to-source bug is **logged, not fixed**, pending a
design answer. The **`--space-1..6` scale is dropped** (pure refactor, no payoff). The catalog's
summary and status field are **not trustworthy**; read the triage doc's "three layers" section first.

## Now — the grind queue (priority order)

The 2026-07-11 website audit (99 findings; detail:
[2026-07-11-website-design-audit.md](2026-07-11-website-design-audit.md)) makes the **personal blog**
(`corpus/tech-blog/`) the priority — it's the forward-facing brand. Direction **"Marginalia"**
(iron-gall manuscript ink). 14 explicit **KEEPs** (serif/sans pairing, offline bundling, `meta.rs` OG
head, live-figure thumbnails) live in the detail file — protect them. Every fix stays invariant-safe
(no CDN, no preview write-back, no new output format, `--tali-*` tokens only).

### A. Blog identity + de-Quarto (build-ready; quick wins first)

*(Section A is empty: #7 draft-aware preview LANDED 2026-07-16 — preview shows drafts inline
(listing badge + page banner + dev-menu count/list), build/publish exclude them and report
"N drafts not published: …", book chapters are draftable. Spec:
[2026-07-16-draft-aware-preview-design.md](../docs/superpowers/specs/2026-07-16-draft-aware-preview-design.md).
Dropped 2026-07-12: #12 chronological post prev/next — for a 7-post topic-diverse blog the
ordering is meaningless and over-promises; the reading-first listing is the right hub, and
sequential nav already exists via books. A category-driven "related posts" strip could revisit
this, but only after a richer corpus makes "related" meaningful.)*

*(Section B, publish/build hardening — `publish --public`, strict-by-default + `--no-strict`,
built-site shared asset bundle — was already SHIPPED by the author; the entries were backlog
rot, verified against source + removed 2026-07-16. See [[backlog-entries-rot]].)*

### C. Theme colour-system a11y follow-ups (2026-07-09 audit; CLOSED 2026-07-16)

*(Section C is closed, 2026-07-16. Six items built: the single-key-shortcut reader toggle
(WCAG 2.1.4, gating `f`/`?`/`/`, not just `f`, which the audit under-scoped), settings-popover
focus-on-open, category chips' `aria-pressed` + live count, keyboard-reachable link previews,
the forced-colors nav marker, and settings-panel reflow at 200%. Spec:
[2026-07-16-section-c-a11y-batch-design.md](../docs/superpowers/specs/2026-07-16-section-c-a11y-batch-design.md);
plan: [2026-07-16-section-c-a11y-batch.md](../docs/superpowers/plans/2026-07-16-section-c-a11y-batch.md).
Two items were NOT built: both had rotted, closed by §F's deck theming/a11y step, verified
against source before deletion. "Embedded deck ignores a sepia host" was already fixed at its
own named anchor (`render/deck.rs:164` reads `(t==='sepia' ? 'light' : null)`, the recommended
fix verbatim). "Deck slide-number chip not restyled per-slide" was fixed by removing the
premise: the chip is now one dark-glass surface in both themes (`deck.css:352-361`), so the
`html.tali-deck-dark`-scoped restyle the bug described no longer exists. See
[[backlog-entries-rot]].*

*Known + accepted (not a bug to re-file): at 200% the 4-button Theme seg wraps, and the first
button of the wrapped line keeps its 1px `border-left`, doubling against the container's own
border. Measured, judged cosmetic (same colour, contiguous, invisible unmeasured). The fix
(gap-dividers + `flex:1 0 auto`) was prototyped in-browser and REJECTED: it stretches the
wrapped button to full width, a bigger visual change than the hairline it removes.)*

Owner-calls kept as-is (one-line changes if ever wanted): table cells use the 1.28:1 hairline
(`base.css:436` — border-strong on every cell heavies every table); callout `tip`/`important`
collapse under protanopia (icon + title already carry meaning, hue never the sole cue); deck has no
sepia palette (document decks as light/dark-only, or add + teach the reader/scroll path).

### D. Reading-first identity polish (CLOSED 2026-07-16)

*(Section D is closed. It was **backlog rot**, and the "direction ruling" it had been blocked on
turned out to be a question about a fork that does not exist. Direction **"Marginalia"** (iron-gall
manuscript ink) is fully landed: theme/colour 2026-07-09, type ~2026-07-12, layout already shipped.)*

*The **type** pointer was rot first (the old "type → item 13" named a `#13` that exists nowhere;
§A's numbering died when §A closed). The owned Newsreader body face is wired at `base.css:35`,
applied at `:216`, both variable faces bundled at `font-weight: 200 800` and inlined as `data:` URIs.*

*Then the three "re-verified, NOT rot" layout targets were re-checked on 2026-07-16, **in a browser
rather than by reading the file**, and two of the three dissolved:*

1. ***"Hero as typeset, not a marketing slab" was ALREADY SHIPPED.** The entry quoted `base.css`'s
   `.hero { text-align: center }` as proof, but that is the **full-width landing** branch; the
   override ~10 lines below (`.tali-site-main:not(.tali-wide) .hero`) is what a reading-measure page
   gets, and its own comment calls it "an editorial masthead ... e.g. a personal homepage". Measured
   live on the blog index: `text-align: left`, lead `max-width: none`, and the iron-gall eyebrow
   hairline rendering at 40x2px in `rgb(56,65,101)`. This is the "trust the symptom, never the line
   number" trap in its purest form: the quoted line was real, and irrelevant.*
2. ***"Drop bordered feature-card grids" is MARKETING-ONLY.** The blog never authors them. Exact-match
   grep: `.feature-grid`/`.feature` appear in `site/features.tmd` + `site/index.tmd` only, **zero**
   hits in `corpus/tech-blog/`. So reshaping them is deferred marketing work, not blog work.*
3. ***The `--space-1..6` scale is genuinely absent** (verified, grep exit 1), but with 1 and 2 gone it
   is a pure refactor: no visible change, regression risk across `base.css`. Owner ruled **drop it**;
   if spacing ever actually hurts, it returns as a real item.*

***The fork was false twice over*** *(and this is the reusable lesson): (a) `page-layout: full` →
`.tali-wide` **already partitions** the deferred marketing site from the blog, and `base.css` already
exploits that partition for `.hero`, so "cannot be scoped to the blog by CSS alone" was simply wrong;
(b) the blog does not author the contested component at all. **Do not re-open D**: the identity work
is done, and the only thing left in those primitives belongs to the marketing rebuild.*

### E. Catalog-derived work (the SWEEP is closed; the items below are OPEN)

**Owner ruling 2026-07-16: stop the sweep, triage an area on demand.** Wave 1 triaged the 4
highest-leverage areas (34/165: crossref, citations, slides, config) and measured the base:
**12 of 34 (35%) outright stale or superseded, 20 of 34 (59%) contain at least one false statement
about today's source.** Triaging the remaining 131 against that base is not worth a session, and the
staleness only grows as more ships. **Consult an area's entries when you next work that area**, using
the trust caveats in the triage doc. Full results, per-entry verdicts, and the caveats:
[2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md).

**Before consulting the catalog, read the triage doc's "three layers" section.** In short: the
entries are the asset and were well-grounded on 2026-07-03; the **heading status is degenerate**
(162/165 skeptic verdicts are `revise`, so 94 read "Proposed (revised)" regardless of conclusion);
and the **executive summary is misleading** (it describes a per-entry tag scheme that does not exist,
miscounts, and its "rule on these first" list mixes open questions with already-shipped work).
A skeptic verdict is evidence, never a ruling: D135's skeptic insisted on dropping Atom feeds as "a
documented non-goal" and Atom shipped anyway, with autodiscovery.

**Wave 1's live output, still open** (each verified against source; the catalog D-number is the
detail pointer):
- **D49 chapter-scoped float numbering** (ADOPT, high). Figures/tables/equations are flat per-page
  counters, so two chapters each get a "Figure 1" and a cross-chapter `@fig-x` cannot disambiguate.
  Only theorems are chapter-scoped. Auto-scoping in a numbered book is a better default, not a knob.
  *Sub-fork:* it would put an auto-scoped "Figure 2.3" beside an opt-in "Theorem 5" in one book.
- **D72/D69 citations** (ADOPT, but **both edit `crates/core/src/cite/`, a Do-NOT-touch zone, and
  need explicit sign-off**). D72: support bare `@key` at all? (The *diagnostic* shipped 2026-07-16,
  `8a45d59`, so the failure is now caught; the engine question is separate.) D69: the reference list
  is `push`ed at the end, so an appendix after `# References` orphans the heading.
  *(D67 LANDED 2026-07-16, never needed the zone. See the note below.)*
- **D37 lint `format:` sub-keys** (ADOPT). The honored `format: deck:` key set is empty, so
  whitelisting `transition` would validate no-ops as supported. This adds a **diagnostic, not a knob**,
  following the from-quarto value-lint precedent (`69c228b`).
- **D34 project defaults** (OWNER-RULING). `bibliography`/`csl`/`execute`/`theme` are absent from the
  19-key `NATIVE_KEYS`, but no corpus doc repeats them across pages, so it fails minimal-config today.
  Recommendation: **subtract before adding**, delete the dead `image:`/`SiteConfig.card_image` field
  (zero readers; its own doc comment concedes it) and defer the defaults until a corpus doc hurts.
- **D70 "Cite this" card** (OWNER-RULING). Its machine-readable half already shipped
  (`.citations.json` + ScholarlyArticle JSON-LD). A card would render **author-free for every current
  post** (0 of 8 tech-blog posts set `author:`).

***Landed 2026-07-16 (deleted from the list above, recorded here so they are not re-scoped):***
- ***D67 `csl:` recognized-but-unsupported.** Shipped, and it **never needed the citation zone**. It
  was **five** surfaces, not four: `AGENTS.md` also taught the key (both it and the vocab JSON are
  *derived* from `vocab::vocab()`, so one filter fixed both). Proved inert by rendering
  `corpus/bayesian-website` with and without the key: byte-identical (980300 bytes). The `css`
  did-you-mean hazard is now **mechanically pinned** (`csl_stays_recognized_because_dropping_it_would_mis_suggest_css`
  builds `KNOWN_KEYS` without `csl` and asserts the suggestion becomes `css`), so a future cleanup
  cannot re-introduce it. The rule finally lives in `frontmatter::validate_unsupported_keys`, on the
  **render path**, not in `diagnostics/` as this entry originally instructed: `diagnostics/` is
  check-only (it appears once in the whole server crate), so the first cut left the **preview**
  silent, which is the surface the author actually reads. Orphaned `ieee.csl` (17KB) deleted with it.*
- ***D74 footnote reverse-sync.** Shipped, and the symptom was **worse** than this entry said: the
  section hardcodes `data-block-id="qmd-footnotes"`, so `closest()` DID resolve, to a block with no
  sourcepos, leaving `openSource()` on its `line = "1"` default. Every footnote silently jumped to
  **line 1**; it was never a no-op. Fixed per-`<li>` (nested positions, the pattern `:::` divs already
  use); the block-level empty sourcepos is **deliberately kept** (a block-level range would break
  `corpus.rs:151`'s monotonic-source-order assert and make reverse-sync swallow the document). **No
  exemption existed to remove:** the checks skip on `sourcepos.is_empty()` *generically*, which is
  exactly how the hole hid.*
- ***D107 deck fragment effects.** Shipped as `::: {.fragment .fade-out}` / `{.fragment .highlight}`
  (a second class on the existing fenced div, so no new authoring form). **CSS-only** (`deck.css`), no
  Rust/JS: the effects reuse the `.tali-frag-visible` marker deck.js already toggles. Declines held
  (no `incremental:` knob, no `data-fragment-index`).*

**Live defects wave 1 found that the catalog never knew about** (small, independent of §E):
1. **The References section repeats the footnote bug** (found while fixing D74, 2026-07-16).
   `cite/render.rs:102` hardcodes `data-block-id="qmd-references"` with an empty sourcepos, so
   Alt-clicking any reference silently lands on **line 1** (same mechanism as D74). **In the
   Do-NOT-touch citation zone**, and unlike footnotes there is **no clean per-`<li>` fix**: a CSL
   entry's real position is in the `.bib` file, not the `.tmd`. *Owner ruled 2026-07-16: log it, do
   not fix it yet.* **It needs a design answer before code:** where should a reference's
   click-to-source land (the `.bib` entry in another file? the `[@key]` citation site? nowhere)?
   Related, deliberately left: clicking the footnote section's own chrome (the `<hr>`/padding) still
   resolves to line 1; closing that needs `locatable()` to require a *usable* sourcepos, a client change.
2. **Duplicate-label warnings are unlocated** (`render/mod.rs:1538`, `site/xref.rs:56` emit no
   file/line), half-reproducing the exact Quarto flaw D53 critiques.
3. **`{.python code-line-numbers=...}` is routed to the executable path** though it is authored as
   display-only in `corpus/deck.tmd:46` and two docs pages; `code_lang` splits naively. Invisible to
   the kernel-free corpus. *Unverified against a live kernel.*
4. **The xref registry goes stale on a warm content edit** (`serve_site/mod.rs:1148-1199` refreshes
   only the Cmd-K search fragment).
5. **`lang: fr` promises French, delivers English** cross-ref labels (`render/page.rs:239`).

*Landed 2026-07-16 and deleted from this list: the **deck key sheet** (it advertised "↑ ↓ Vertical
slides" while `up()`/`down()` call `moveTopic`; the pin now reads the binding and the sheet together
so they cannot drift apart again); **`author: [A, B]`** (a YAML sequence read via `.as_str()` gave
`None`, so both consumers fell through `.or(config.title)` and a multi-author site published its own
**title** as the author in the Atom feed and JSON-LD; `SiteConfig.authors` now reuses the same
`frontmatter::string_list` a page's `author:` always used, and the deliberate RFC-4287 title fallback
is pinned to fire only when there is genuinely no author); and the phantom **`number-sections`** doc
comment (the key existed nowhere in the source but the comment claiming it; numbering is really
decided by `chapter_for`). Note the 2026-06-29 theorem spec still reasons about "the `number-sections`
feature" as though it shipped: it is a dated record, left as written.*

### F. Deck rework (2026-07-12 slides audit → [2026-07-12-deck-audit.md](2026-07-12-deck-audit.md))

**Start in [2026-07-12-deck-audit.md](2026-07-12-deck-audit.md)** — the wide slide-deck audit: 43
confirmed bugs + a keep/cut/fix/add feature verdict + a mobile-feed spec + a grind order. Owner-decided
shape change this session (REMOVE, don't fix the old behavior): a deck opens **as a deck** (desktop =
stepped slides; phone/portrait = a new TikTok-style scroll-snap **slide feed**, keyed on aspect not
width); **delete reader/scroll mode**; **delete print/PDF** (the critical dark-deck-blank-PDF bug is
resolved by removal); trim the overview flourishes (minimap/LOD/threads/filter/pen/van-Wijk zoom). The
file's grind order: (1) pin kept features in `corpus/deck.tmd` first (net); (2) flip the front door +
delete reader/PDF (kills whole bug families); (3) crashes/correctness (`. . .`-before-plain-code wedges
nav; readHash anchor/digit misroute → slide 0; live `---`/`. . .` not structural; "Title Slide" id
collision); (4) build the mobile feed; (5) trim flourishes; (6) theming/a11y/perf; (7) share-link +
live-input deep-link + wake-lock adds.

**Progress (2026-07-16): the ENTIRE audit is landed except one deliberately-deferred item.**
Steps 1-7 all done (front door + feed + correctness + flourish trim + theming/a11y/perf + docs
+ the C-ADD share-link/QR, live-input deep-link, feed notes-narration, wake-lock adds). See the
audit file's top-of-doc **Status** block for the per-item tracker. **Only remaining: B3-18** — a
structural deck edit re-mounts the *whole* deck, nuking every `{js}`/WebGL widget's state;
re-mount only the edited `<section>` subtree. Deferred on purpose (touches the client's re-mount
path; bigger blast radius). Nothing else in section F is open.

### G. AI-native authoring (2026-07-12 audit → [2026-07-12-ai-native-backlog.md](2026-07-12-ai-native-backlog.md))

*(Section G is closed as a grind chunk, 2026-07-16. It was **backlog rot**: the whole browser-free
loop (the three items this entry called "the recommended first bets") shipped 2026-07-13, along
with 5 more. Verified against source + all 30 named pins run green before deleting the entries. See
[[backlog-entries-rot]].*

*Shipped (item → anchor → pin): #1 AGENTS.md onramp → `core/agents.rs:42` → `agents_md_cli.rs`;
#2 `taliesin read` → `render/text.rs` + `model.rs:283 body_text()` → `text_projection.rs` +
`read_cli.rs`; #3 agent-grade diagnostics → `server/check.rs:23` (`{code, severity, file, line,
message, suggestion?}`) + `core/diagnostics/codes.rs` → `check_cli.rs`; #4 Claude Code skill →
`editor/claude-code/skills/taliesin/SKILL.md` → `skill_freshness.rs`; #5 `map` → `map_cli.rs`;
#6 `taliesin-mcp` → `server/mcp.rs` → `mcp_stdio.rs`; #7 scaffolds + `paper` kind → `new_cli.rs`;
#10 structured build/publish errors → `structured_build_errors.rs`. Plus **#8(b)** placeholder-alt
(`a11y.rs:337`) and **#9(B)** ScholarlyArticle (`meta.rs:150`, author-free trigger) + **#9(C)**
per-page cited-refs sidecar (`build.rs:355,1560`) → `citations_sidecar.rs`.)*

*The three ruling-gated leftovers (#8a `check --online`, #8c numeric-claim hint, #9A per-page text
sidecar) were ruled **decline** by the owner on 2026-07-16; reasoning is recorded under "Decided
against". **Nothing in section G is open.***

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
  - `build.rs:926` warms the pool before knowing any page needs a kernel, even under
    `TALIESIN_NO_EXEC=1`. Hygiene, not perf (0.25 s vs 0.27 s on a prose-only site).
  - `fork_kernel` cross-call edge (low): a timed-out-but-queued fork mis-pairs the next `SPAWNED
    <pid>`; poison the daemon on any fork timeout so later `take`s cold-start.
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
