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
local `main` on request; push to `origin/main` only when the author asks. **Do-NOT-touch (one
freeze, not two):** the standing freeze is `MAX_WARM_PAGES` + `exec_pool.rs` eviction (M6a,
sign-off refused 2026-07-17) and the single-editing-surface invariant. The rest of the exec/kernel
zone is **not** blanket-frozen: its audit finished and the M2-M5 sign-offs were granted + spent
(CLAUDE.md carries the definition). Review subagents use read-only git —
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

**Pick up here (2026-07-19, polish audit — PL1–PL20 queued; NOTHING built yet, report-only by request).** A
feature-*polish* audit ran (4 read-only auditors: CLI/DX · authoring · live view · theming, + online research +
per-finding source verification). Distinct lens from every prior audit (DX/PMF/machine-facing/vacuous-test/website/
deck): "can an existing feature be *simplified with the same power*, or is it *implemented unintuitively*?" Findings
are queued as the **Polish audit batch (§7** of Open work, above Tier 2**)**; full detail + credit +
grind order in [2026-07-19-polish-audit.md](2026-07-19-polish-audit.md). **No code changed; no SHAs.** Headline
pattern (do these first): "silent holes in an otherwise fully-diagnosed surface" — PL1 (`check` hides its own
`--explain`/codes in human output), PL2 (`{{< input >}}` has no `.input` div + colliding CSS that mutes its label),
PL3 (three column spellings, `.column width=` silently dropped), PL7 (deck `|` line-spec silently no-ops in a
walkthrough), PL9 (`.fade-out`/`.highlight` escape the div did-you-mean). **Best single first move: PL1 or PL2**
(both S · high). The top ~15 were read to the line by the orchestrator (marked VERIFIED); still grep the named
symbol before promising, per this file's law.

**Pick up here (2026-07-18, DX audit — DX1–DX5 + DX10[4/4] + DX11 landed; DX6–DX9/DX12–DX19 queued).** A developer-experience audit
landed ([2026-07-18-dx-audit.md](2026-07-18-dx-audit.md); DX/discoverability research + full DX-surface
map + error/feedback-loop audit + 4 persona simulations); 19 findings + 3 design questions queued as the
**DX audit batch (§6** of Open work, above Tier 2**)**. **DX1 (live preview validation) is DONE** (both
serve paths surface the static validators + cross-page + `_site.yml` warnings; browser-verified).
**DX2 (first-run in-preview hint) is DONE** (one-time dismissible callout surfacing Alt-click-to-source +
`?`; browser-verified single-doc/site/deck + matrix). **DX3 (auto-wire config JSON Schema on `init`) is
DONE** (`init` emits `.taliesin/` schemas + the `_site.yml` modeline; integration-verified). **DX10
(teaching scaffolds) is DONE** — all 4 sub-parts (paper worked `{python}` figure + math + xrefs,
`init`→`taliesin new` pointer, `new post --draft`, and `new deck --tour` [the DX10-followup, a guided
browser-verified deck]). **DX5 (silent-degradation "did you mean") is DONE** — `::: {.columns}` aliases to the
`layout-ncol` grid (Lena's on-projector disaster) and a misspelled feature/theorem `:::` class draws a
located did-you-mean (near-miss only; browser-verified). **DX11 (friendly `.pdf`/wrong-target rejection)
is DONE** — `build doc.tmd doc.pdf` (and `.docx`/`.tex`/`.md`/…) is now a hard error naming HTML-only + the
`.html` fix + browser Print + the planned print track, instead of silently writing HTML bytes into the
`.pdf` at exit 0 (🎓's abandonment moment); a pure guard in `parse_build_args` (`NON_HTML_OUTPUT_EXTS`),
unit- + end-to-end-pinned (`strict_robustness.rs`). **DX1–DX3 + DX5 + DX10[3/4] pushed to `origin/main`
2026-07-18** (verify with `git log --oneline origin/main..main`; the author may push again mid-session, so
re-check, never trust a recorded SHA). **DX10-followup (`new deck --tour`) is DONE** — a `--tour` arm on
`new deck` scaffolds a guided, check-clean deck demoing every deck feature (browser-verified: columns
side-by-side, notes hidden); deck-only. **DX4 (`taliesin doctor`) is DONE** — a standalone env self-audit
(both interpreters + kernel packages + active conda/venv + `_site.yml`), ✓/⚠/✗ + fix commands, exits
non-zero only on a configured-but-broken interpreter; `--format json`; all pushed to `origin/main`. **DX6
(`check --explain <CODE>`) is DONE** — a flag on `check` (not a subcommand) that expands a stable `TAL-*`
code into title/cause/canonical-fix (rustc `--explain` style; honours `--format json`; bare `--explain`
lists every code; unknown code → did-you-mean + non-zero), plus a per-diagnostic `docs_url` (computed,
so it can't drift) on every `check --format json` diagnostic. The prose catalog is an `EXPLANATIONS`
table next to the code TABLE in `crates/core/src/diagnostics/codes.rs`, drift-locked by a completeness
test; `docs/DIAGNOSTICS.md` is generated from it (blessed, `TALIESIN_BLESS=1`) so the `#anchor` resolves
on GitHub; `check --explain <TAB>` completes to the code set. **DX8 (Cmd-K command palette) is DONE** —
Cmd-K now runs commands too, not just search: an empty query lists the available actions (a discoverable
menu), a query sorts matching actions above content. Three capability-gated actions (Toggle theme, always;
Restart kernel + Open source in editor, live-preview only via globals that live in `client.js`, so a
static build shows only the theme action). Each reuses the owning module's behavior (`theme.rs`'s
`window.taliToggleTheme`, extracted from + shared with the dev-menu button; the kernel restart; and
`gotoSource(null,1)`); new-post/draft + slide-jump excluded on principle (read-only-preview / deck chrome).
Rust drift pins guard the `include_str!`'d JS; browser-verified via chrome-devtools at 3 viewports + a
static build. **DX7 (dynamic value completion) is DONE** — grepping first showed the flagship (`@`-xref
completion *with descriptions*) was **already shipped** in the companion and shell page/deck/slug names are
already path-completed, so DX7 shipped the two real gaps: `taliesin completions --install` (detect `$SHELL`,
write the script to its XDG-aware dir; pure `install_plan`, unit + e2e verified) and companion completion for
`{{< embed/include >}}` file targets (new `detectContext` context + pure `shortcodePathCandidates`, node:test
+ tsc + bundle verified; in-editor click-through needs a vsix repackage, not done). **DX18 (check exit-
gating) is DONE** — two default-off gate flags: `--errors-only` (drop warnings from output + exit decision)
and `--require-kernel` (fail if a used language's kernel isn't runnable); pure gate helpers + CLI exit-code
integration tests; `--min-severity` folded into `--errors-only` (only two severities today). **DX6 + DX8 +
DX7 + DX18 + DX12 + DX19 landed locally** (push when asked; verify with `git log --oneline origin/main..main`).
**DX9 + DX15 + DX13 landed 2026-07-19** (the three build-ready Tier-2 surfacing items; see §6).
Next up: **DX17** (headless executed-output visibility) is the last DX-batch item, but it is L, net-new, has
a fork (optional headless `{js}` eval) and overlaps `ROADMAP.md` agent work, so **brainstorm before
building** (do NOT scope it straight from the one-liner). The only remaining Tier-2 DX items are DX14
(interactive `new`/`init` wizard, M · net-new) and DX16 (update-available nudge, S · net-new — implies a
network check, so weigh it against the offline invariant first). Most remaining items are *surfacing an
existing capability*.

**Pick up here (2026-07-18, PMF-audit batch — START HERE for feature work).** A product-market-fit
audit landed ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md); 30+7 sourced personas +
author/reader/publish/trust walkthroughs). Headline: the tool is **feature-complete for ~one user**,
so the dedup pass killed most candidates (they already ship: `publish`→Cloudflare, presenter view,
deck a11y, `echo`/`code-fold`, the deck `?` menu). **ALL THREE PMF builds now LANDED 2026-07-18**
(pushed to `origin/main` 2026-07-18): **B1** reader "Cite this" box (`4bb10c7`, §1), **B2** book landing-page
auto-TOC (`b284544`, §1), **B4** deck Marginalia identity (Direction A, serif titles; merge `2cf72f4`,
feat `d04a06c`, §3). **The whole PMF build-batch is done.** What remains is Tier 2/3 (demand-driven):
verify item **C-PUB-1**, the Zenodo DOI on-ramp **B5** (a natural next step now B1 ships the
reader-facing citation), and the rest of the tail. **Check, don't trust the SHAs:** `git log
--oneline origin/main..main`. **Heads-up (not a backlog item):** a parallel **shell-completion**
feature reached local `main` (`d493560`) via `worktree-shell-completion`; as of this batch it has 2
`complete.rs` clippy errors (`-D warnings`) + a failing `skill_freshness` test — its owner's to
resolve before pushing, untouched here (disjoint from B1/B2/B4).

**Pick up here (2026-07-18, newest — C3+C4 coverage gaps landed; the C3–C6 batch was 2/4
already pinned).** Filling the "four uncovered features" set, two were already covered — the
exact backlog rot this file warns about, caught by grepping the named symbol before trusting the
entry. **Do not re-add C3/C4/C6 as coverage gaps.**
- **C6 (Google-Scholar `citation_*`) was ALREADY fully pinned.**
  `config.rs::scholarly_citation_meta_for_authored_dated_posts_only` asserts all five `citation_*`
  names + the non-article negative case. Not a gap; built nothing.
- **C4 (`head:`/`body-*:` injection) was ALREADY 3/4 pinned.**
  `config.rs::site_head_and_body_end_are_injected` covered `head:` + `body-end:`; only
  **`body-start:`** (a real wired slot: `config.body_start` → `before_body` →
  `include_before_body`, ahead of the content) had no test. **LANDED** (`cd14b8a`): extended +
  renamed to `site_head_body_start_and_body_end_are_injected`, injecting all three and asserting
  the before/after ORDERING so a slot-swap can't pass on presence alone.
- **C3 (custom `theme:` `.css`) had a genuine meaty gap.** Only the negative case (`gone.css`
  warns) was pinned — and it passes even with `base_dir = None`, since the `.css` branch
  short-circuits to the not-found warning before any read, so a no-op reader renders clean.
  **LANDED** (`45f9daa`): new `corpus/theme-css/` artifact (a doc + a sibling `brand.css`) +
  `crates/core/tests/theme_css.rs` pinning the file-read path (its CSS reaches both the
  RenderedDoc `theme_css` and the page's `<style id="qmd-theme">`) and the
  `_extensions/<name>/theme.css` bundle branch. Each branch mutation-checked independently.
- **C5 (`mounts:`) is mostly pinned; only the serve path is left, and it is P3.** Config-parse +
  typo-warn (`config/mod.rs`), `map` surfacing (`map_cli.rs`), `check` link-tolerance, and
  build-side `mount_warnings` (`build.rs`) are all covered; mounts are **preview-only** (the
  static build does not wire them, it warns). The one untested surface is the live `serve_site`
  MountedSite discovery/serve — a bin-crate integration gap with no live-HTTP test infra in the
  suite. Low-value, demand-driven; left rather than manufactured.
- **So the C1–C7 coverage batch is effectively CLOSED** (C1/C2/C7 last session; C3/C4 this
  session; C6 was never a gap). Only C5's serve path remains, P3.
Both commits verified: full `taliesin-core` (541 unit + all integration) + `taliesin-server` (218
unit + all integration) suites green, `cargo fmt --check` + `clippy --all-targets -- -D warnings`
clean, every new test mutation-checked (mutate the code → watch the named test fail → revert).
**Check, don't trust the SHAs:** `git log --oneline origin/main..main`.

**Pick up here (2026-07-18, latest — a vacuous-test / mutation audit landed + pushed).** A new
lens: not "find bugs in the source" but "which green tests don't actually constrain the behavior
they name?" — the codebase's own most-repeated finding ("the tests certify the defects") run as a
deliberate sweep for the first time. 4 read-only agents + a `cargo-mutants` backstop, **every
finding verified by real mutation** (14/14, zero misfires). All 14 hardened the same day
(test-only, plus one dead-code removal: the orphaned `vocab` `about` description). Detail +
the full table: [AUDITS.md](AUDITS.md) (top entry). Highlights: a **data:image/svg+xml XSS
exclusion with no test** (C4), the **block-id content-hash** pinned only incidentally by 4
snapshot docs (C1), and machine-facing output (`strip_katex`/`og:type`/OG-card `lead`/reading-time
number) whose tests asserted presence, not correctness. Method note now in AUDITS.md: same-file
mutation reverts need a file backup, not `git checkout` (it eats the new test).

**Pick up here (2026-07-18, later session — a coverage + live-defect batch landed to LOCAL
main; NOT pushed, the author pushes).** Seven commits verified (TDD + mutation-checked where
the code pre-existed; full workspace green, clippy `-D warnings` + fmt clean). **Check, don't
trust:** `git log --oneline origin/main..main`.
- **C1 `{{< embed >}}` pinned** (`9aa76dc`): the top coverage gap closed. New `corpus/embed/`
  mini-site + `crates/server/tests/embed_site_build.rs` (site build builds the deck beside the
  page, iframe resolves, kept out of nav — mutation-checked by disabling the deck loop) + three
  render unit tests (iframe markup, `.tmd`→`.html` href, `embed_targets`).
- **seo #8 CLOSED** (both halves): `47e07af` percent-encodes absolute page URLs at the shared
  `abs_page_url` (a spaced dir shipped a raw space into the sitemap `<loc>` + a broken llms.txt
  link); `124855c` diagnoses a scheme-less `url:` (`url: ex.com` built clean and emitted
  `<loc>ex.com/</loc>` under a green `check`) — verified end-to-end against the real binary.
- **§2 #1 Part A landed** (`108e5bd`): `register_xref`'s duplicate-cross-reference-label warning
  is now located (8 call sites threaded). **Part B (the site cross-page duplicate on the
  `Vec<String>` channel) re-scoped to Tier 2** — it is a channel type change AND a semantic
  question (a cross-page dup has ≥2 locations; which do you point at?). See §2 #1 + Tier 2.
- **C2** (`818663a`) pins `{{< video >}}` `dark=`/`poster=`/`caption=`; **C7** (`4079f9d`) is a
  black-box `render`/`blocks` CLI test; **cli.tmd** (`3216007`) gained the missing `mcp`/`map`/
  `vocab`/`read` rows + the `paper` scaffold kind.
- **M6b was ALREADY LANDED** (`dc5af1e`, "stop the RAM probe failing open in a container") — the
  reduction map + §4 listed it as open with "zero tests"; it has the full cgroup-v2 ancestor-walk
  cap and a dated test module. **Pure backlog rot** (the exact trap this file warns about). §4
  entry struck below.
- **Still open (measured, not rushed):** §2 #1 Part B; and **C5's serve-path gap only** (the live
  `serve_site` mount discovery/serve, P3). *(The rest of the C3–C6 batch landed 2026-07-18 or was
  already pinned — see the newest start-here block; C3/C4 done, C6 never a gap. **R2 also landed**
  2026-07-18, `3dfac8e` — the scanner unification; option A, behavior-preserving.)*

**Pick up here (2026-07-18 — reduction + modularity pass landed & pushed to `origin/main`):**
A staged extension-system exploration ran this session. **Strategic outcome (do not
re-litigate):** grow the extension system in phases — **(i) internal modularity now, (ii)
user opt-in slimness when the tool is in daily use, (iii) third-party ecosystem only if
external users appear.** Present-benefit only: build nothing now that is not earning its keep
today; the tiered extension *design* is captured but deliberately UNBUILT.
- Spec: [2026-07-17-reduction-and-modularity-pass-design.md](../docs/superpowers/specs/2026-07-17-reduction-and-modularity-pass-design.md).
  Verified findings map: [2026-07-17-reduction-audit-map.md](2026-07-17-reduction-audit-map.md).
- **Headline: the codebase is already lean.** A 5-way audit found almost no dead code (tiny
  reduction yield) — which is itself the answer, and why the extension system is deferred:
  nothing in core wants extracting yet.
- **Landed (Phase 2 + T1), committed + pushed:** removed the `about:` block (superseded by
  `hero:`; it cascaded into the schema/vocab/AGENTS.md goldens), the orphaned `TAL-MEDIA` audio
  diagnostic row, and the dead `search_button(full=true)` variant; added `PageParts::defaults()`
  so a new page field is one edit, not four. Net −127 lines; workspace green, fmt + clippy clean,
  tech-blog artifact re-verified (hero intact, `tali-about`/`tali-search-full` gone).
- **R1 was DEFERRED, and that is a finding:** `llms.rs` re-derives text extraction, but it and
  `render::indexable_text` decode DIFFERENT entity sets (`&#8217;`/`&nbsp;`), so reusing it
  would leak raw entities into `llms.txt`. Pinned by a passing test
  (`text_content_decodes_more_entities_than_indexable_text`); aligning them also moves the
  search index, a separate call.
- **New open items** (evidence in the map; the scanner one is filed under Tier 2 below): R2/T2
  unify the three raw-source pre-scans in `site/{xref,book,discovery}.rs`; the 7 corpus-coverage
  gaps C1–C7 (biggest: `{{< embed >}}` is load-bearing with ZERO corpus + unit tests); and
  `docs/guide/reference/cli.tmd`'s command table is stale (missing `mcp`/`map`/`vocab`/`read`,
  and `paper` on the `new` row).
- **Housekeeping (owner's call):** six stray `.claude/worktrees/agent-*` worktrees at the repo
  root — debris or live parallel-session worktrees. `git worktree list` before any
  `git worktree remove`.

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
- **D34: SUBTRACT.** ~~Delete the dead `SiteConfig.card_image`~~ **— the subtract half was already
  done (`084c338`); this line's `SiteConfig.card_image` is a MISNOMER (no such field; the live one
  is `Page.card_image`). See §3.** Defaults half stays **deferred** until a corpus doc actually
  hurts. "Perfect the default before adding a knob."
- **D70 "Cite this" card: DECLINED 2026-07-18** (owner accepted the skip) — it would render
  author-free for all 8 tech-blog posts (0 set `author:`), and its machine-readable half already
  ships. Was "still unruled".

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

**Pick up here (2026-07-17, latest session — landed §2 #5, the hidden-cell phantom anchor):**
1. **§2 is down to THREE un-struck items: #1, #3, and #8's other two thirds.** *Recount, do not read
   this line* — `awk '/^### 2\./,/^### 3\./' notes/backlog.md | grep -cE '^[0-9]+\. '` still says ten;
   SEVEN are now struck. **None of the three is mechanical**: #1 is two channels (one needs a
   `Vec<String>` → located type change); #3 is a design question in a defect's clothes (English-only
   xref labels, an absent i18n feature no corpus doc demands); #8's `<loc>` halves are two fixes plus
   a diagnostic. **#5 held its shape but not its size: it read as "one lang-dependent gate" and was
   the figure arm + the table arm + a burned-number bug + three review findings — 7 real changes.**
   The recurring finding is unchanged: an entry's summary counts *symptoms*, and symptoms do not map
   1:1 to changes. Two NEW Tier-2 items were spun off it (the `{bash}`/`{sql}` lang-axis twin and the
   empty-output trigger) — see the "Execution-cache leaks" bullet's siblings under Tier 2.
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

**This section is now EMPTY — both PMF build-ready items (B1, B2) landed 2026-07-18.** The only
remaining PMF feature is **B4 (deck visual-identity pass)**, which needs an owner *direction* ruling
first (§3). The rest is Tier 2/3 (demand-driven).

**Do not re-add:** **B2 (book landing-page auto-TOC) LANDED 2026-07-18** (local `main` `b284544`,
NOT pushed — the author pushes; spec `6690abb`): `site/book_toc.rs` (`render_book_toc` +
`attach_book_toc`, appended in `finish_blocks`, a no-op unless `is_book()` && the landing
`index.html`; distinct `tali-btoc-*` classes so it never collides with the drawer's drawer-close
markup), `site.css`, `corpus/demo-book/` (`methods.tmd` gained a `description:`) +
`tests/book_landing_toc.rs`. Additive: the scrollspy TOC + chapter drawer are untouched (the
2026-07-06 keep-both-nav-surfaces decision). Browser-verified (light+dark, mobile+desktop, no
console errors).

**Do not re-add:** D37 (lint `format:` sub-keys) already landed (`515fbd7`). **B1 (reader-facing
"Cite this" box) LANDED 2026-07-18** (local `main` `4bb10c7`, NOT pushed — the author pushes;
spec `6f1b5a4`): `site/cite_this.rs` (deterministic BibTeX/CSL-JSON/RIS serializers + resolve/gate +
the `.tali-cite-this` block via `attach_cite_this`, appended last in `finish_blocks`), JS fragment
`17-cite-box.js`, `site.css`, and `corpus/cite-this/` + `tests/cite_this.rs`. Owner ruling
2026-07-18: **site-author fallback** (page `author:` → site `author:`, never the site *title*, so an
authorless post with no site author degrades to nothing); DOI deferred to B5. Browser-verified
(light+dark, mobile+desktop, no console errors). **Do not re-add.**

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

1. **Duplicate-label warnings are unlocated** — half-reproducing the exact Quarto flaw D53
   critiques. **Part A LANDED 2026-07-18** (`108e5bd`): the render-side `register_xref` warning
   (`Vec<Warning>` channel, which already had `.at()`) now carries the duplicate's file/line,
   threaded through its 8 call sites (the five in the main block loop pass `source_file`/
   `buf_start`; the three caption/theorem passes derive it from the block's `sourcepos`). As
   priced, this was NOT one small fix. **Part B still OPEN, re-scoped to Tier 2:** the site
   cross-page duplicate (`site/xref.rs`, `site/mod.rs`, and the harvest's own `site/mod.rs`
   duplicate) is on the `Vec<String>` channel with **no location field at all** — and it is not
   merely a channel type change: a "defined on multiple pages" warning has **≥2 locations**, so
   locating it is a semantic decision (point at the second, or carry both). See Tier 2.
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
5. ~~**A labelled `include: false` python/R cell registers an anchor that never exists**~~ **LANDED
   2026-07-17** (`ce153ff` render fix + `13ff03c` review fixes). The figure/table arms now gate the
   counter AND the registration on "will this materialize?", the rule `CellRole::Listing` already
   used; the cell still runs (the block is emitted hidden, not `continue`d, so downstream cells keep
   their kernel state — the fix's biggest risk, refuted by a real kernel showing `DOWNSTREAM_SEES`
   intact). **The entry filed one symptom; the seam held THREE, and the count is the lesson.**
   (1) `CellRole::Table` had the identical phantom by a *different* route (`apply_table_captions`
   registers off `cell.table`, the intent) — the entry named only Figure. (2) The reader-visible
   half nobody filed: `fig_count += 1` ran before the include check, so a hidden figure **burned a
   number** — the repro's only visible figure came out "Figure 2" with no Figure 1, a defect that
   needs no `@ref` at all to bite. (3) `include: false` is not the only trigger: `exec.rs` also drops
   *empty* output, so a labelled cell that prints nothing phantoms too — NOT knowable at render time,
   filed to Tier 2 rather than widened in. **The adversarial review then found three more, all real,
   all mine:** the new warning let the author's own label pick its diagnostic code
   (`fig-math-model` → TAL-MATH; the pre-existing theorem warning hijacked identically — fixed for
   both, `TAL-XREF-UNREF`); the `js` half of the lang exemption was unpinned (deleting `| "js"`
   passed the ENTIRE core suite — the previous commit's "pinned" claim rested on a mutation that only
   removed both names); and `from_executor` was a lie — the executable set is python|r ONLY
   (`exec::kernel_lang`), so `{bash}`/`{sql}` figures phantom for *any* `include`, which is the same
   bug on the LANG axis and is **still open** (needs `kernel_lang` shared into core; filed Tier 2).
   14 tests, every one mutation-checked; all five real projects build byte-identical to pristine main.
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
8. ~~**`seo.rs` emits machine-invalid output with no diagnostic**~~ **CLOSED 2026-07-18.** All
   three halves landed: `<lastmod>` (`3041f87`), URL-escaping (`47e07af` — percent-encode at the
   shared `abs_page_url`, so sitemap `<loc>` + `llms.txt` + feed + og:url are all fixed at one
   producer; new `percent_encode_path` helper in `feed.rs`), and the scheme-less `url:` diagnostic
   (`124855c` — `validate_url` in `config/parse_native`; `url: ex.com` now fails `check` with a fix
   hint, verified end-to-end). The prediction held: a diagnostic, not a knob (the `page-layout` /
   site-`image:` precedent).
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

- **Deck visual-identity pass (PMF B4) — LANDED 2026-07-18** (local `main`, merge commit `2cf72f4`,
  NOT pushed — the author pushes; feat `d04a06c`, spec `4849a53`). **Owner direction ruling
  2026-07-18: Direction A — Marginalia (serif titles).** A pure `deck.css` pass (no engine/DOM
  change): a `--deck-font-head` **Newsreader** serif token (already inlined via `FONTS_CSS`, was
  unused) applied to all headings + the title; an iron-gall accent rule under the serif title; and a
  designed **section-divider** treatment for the previously-unstyled `section.tali-slide[data-level=
  "1"]` (centered, a large serif numeral from a document-order CSS counter, the serif h1, an accent
  rule). Body/lists/code stay sans. Exemplar `corpus/deck-marginalia.tmd` + `tests/deck_marginalia.rs`
  (pins the identity CSS AND the `data-level="1"` hook). `corpus/deck.tmd` unchanged. Browser-verified
  light+dark. **Deferred (clean follow-up):** the mono eyebrow (needs a small `deck.rs` front-matter
  field). **Do not re-add.**

*Prior entries resolved 2026-07-18 (records kept):*

- **D34 — RESOLVED (its build half was already done; the entry had rotted).** The "subtract" half
  landed at `084c338` ("delete the site-level `image:` that stopped meaning anything (D34)"): the
  dead site-level `image:` field is gone AND the key left `NATIVE_KEYS`, so a stale `image:` now
  warns instead of parsing inert — pinned by `a_site_level_image_is_not_a_config_key_and_says_so`
  (whose comment reads "D34's subtraction (owner ruling 2026-07-17)"). **The ruling block below
  still says "delete the dead `SiteConfig.card_image`" — that field does not exist.** There is no
  `SiteConfig` image field; `Page.card_image` is a LIVE listing-card thumbnail (`card_html`,
  discovery.rs) — do NOT touch it. The "add `bibliography`/`csl`/`execute`/`theme` project
  defaults" half stays **deferred** per the ruling ("perfect the default before adding a knob";
  revive only when a corpus doc repeats a key across pages).
- **D70 "Cite this" card — DECLINED** (owner accepted the skip, 2026-07-18; moved to "Decided
  against"). Its machine-readable half already ships (`.citations.json` + ScholarlyArticle
  JSON-LD); a rendered card would be author-free for all 8 tech-blog posts (none set `author:`).
  **REVIVED 2026-07-18 (same day): the owner reversed this decline and wants it; now in Build-ready
  as PMF B1, gated on sufficient page metadata so the authorless case degrades cleanly.**

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
    - ~~**M6b the RAM probe fails OPEN in a container**~~ **ALREADY LANDED** (`dc5af1e`, "stop the
      RAM probe failing open in a container"), discovered 2026-07-18 to be pure rot: this entry (and
      the reduction map) said "zero tests", but `build_budget.rs` has the full cgroup-v2
      ancestor-walk cap (`cgroup_free_mb_with`, `probe_free_mb` capping host by cgroup) AND a dated
      M6b test module (`the_container_that_failed_open_is_now_capped_by_ram_not_cores`, 25 tests
      green). The classic "grep the symbol before trusting this file" lesson.
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

### 6. DX audit batch (2026-07-18)

**Full rationale + sources + persona friction tables:** [2026-07-18-dx-audit.md](2026-07-18-dx-audit.md)
(2 sourced research passes + 2 codebase audits + 4 persona simulations, all grounded against source).
**These are audit findings, not priced builds — S/M/L are guesses.** Per this file's own law: grep the
named symbol first, trust the *symptom* not the cause/line, and measure the running server before
promising anything. Tags: 🎓 academic · ✍️ blogger · 🎤 speaker · 🤖 agent. Type: [surface] = wire up
an existing capability (most of these), [new] = net-new.

**DX1 — LANDED 2026-07-18** (the dominant finding). Live static validation now runs on **both** serve
paths, feeding the existing dev-menu list + badge; spec/plan under `docs/superpowers/specs|plans/2026-07-18-dx1-live-preview-validation*`,
closure record in [AUDITS.md](AUDITS.md). Scope collapsed hard on grounding (the exact rot this file warns
about): the red-dot badge **already existed**, and single-doc `serve` **already** surfaced xrefs + render
warnings — the real gap was `serve_site` parity (static set + cross-page + `_site.yml` warnings). No
incremental infra needed (~27 ms whole-site re-derive). **Do not re-open.**

**Tier 1 — discoverability family ("in the vein of the shell completion just shipped"), now do-first:**

**DX2 — LANDED 2026-07-18.** First-run in-preview hint: a one-time, dismissible, localStorage-gated
(`tali-hint-seen`, fail-closed) callout tethered above the `◇</>` button surfaces the flagship
Alt-click-to-source gesture + (where live) `?`. All in `web-client/client.js` (built in `buildDevMenu`)
+ a CSS block in the shared `STATUS_CSS` (`serve/mod.rs`); preview-only, four dismissals (Got it /
menu-open / Alt-click / Esc). Per-line liveness omits the `?` line on decks / shortcuts-off. Spec/plan
under `docs/superpowers/specs|plans/2026-07-18-dx2-first-run-preview-hint*`, closure record in
[AUDITS.md](AUDITS.md). Browser-verified single-doc/site/deck + viewport matrix. **Do not re-open.**

**DX3 — LANDED 2026-07-18.** `taliesin init` now emits both bundled schemas into a walker-skipped
`.taliesin/` dot-dir and prepends `# yaml-language-server: $schema=.taliesin/tali-site.schema.json` to
the scaffolded `_site.yml`, so config autocompletes + red-squiggles in-editor with zero manual step.
One-file change (`cli.rs` `INIT_SITE_YML` + `scaffold_init`), reusing the `SITE_SCHEMA`/`FRONTMATTER_SCHEMA`
constants (can't drift from the validator). Spec/plan under
`docs/superpowers/specs|plans/2026-07-18-dx3-init-schema-autowire*`, closure record in [AUDITS.md](AUDITS.md).
Integration-verified (byte-identical to `taliesin schema`; `.taliesin/` never ships into `_site/`).
**Do not re-open.**
**DX4 — LANDED 2026-07-18.** `taliesin doctor [dir]` is a standalone env self-audit: unconditionally
resolves + probes both Python and R (the circular `check`-only probe is now surfaced), reporting a per-item
✓/⚠/✗ line with a fix command, plus **conda/active-env detection** (new: `VIRTUAL_ENV`/`CONDA_PREFIX`) and
`_site.yml` config sanity. Severity: ready = runs + kernel pkg; warn = pkg missing (fix cmd) or an absent
*default* interpreter; error = a *configured* interpreter (`TALIESIN_*`/`_site.yml` field/`.venv`) that
won't run (exits non-zero). `--format json` for agents. Pure testable core reuses `crate::interpreter`
verbatim; never executes the doc. Spec/plan under `docs/superpowers/specs|plans/2026-07-18-dx4-doctor*`,
record in [AUDITS.md](AUDITS.md).
**DX5 — LANDED 2026-07-18.** Both silent-degradation traps closed: (A) `::: {.columns}` with `.column`
children is aliased to the native `layout-ncol` grid (side-by-side, responsive — Lena's on-projector
disaster); (B) a misspelled feature/theorem `:::` class draws a located "did you mean" (`validate_div_class`,
near-miss ≤2 against `DIV_FEATURE_CLASSES ∪ THEOREM_KINDS`) — but only near-misses warn (div classes are an
*open* vocabulary; exact-known + far-custom stay silent). Extends `validate_callout_kind`. Spec/plan under
`docs/superpowers/specs|plans/2026-07-18-dx5-div-class-did-you-mean*`, record in [AUDITS.md](AUDITS.md).
Browser-verified (columns 2×360px grid; `.fragmnet` → "did you mean `fragment`?" in the preview dev menu).
**This unblocked the DX10-followup (`new deck --tour`)** — the columns idiom now works via the alias
(DX10-followup landed 2026-07-18).
**DX7 — Dynamic value completion — LANDED 2026-07-19.** Grepping the named surfaces first (per this file's
anti-rot rule) showed the flagship was **already shipped**: `@`-xref completion *with descriptions* lives in
the companion (`completions.ts` merges buffer `{#id}` anchors + the `symbols` registry as `Figure N`), and
"page/deck names, slugs" in the *shell* are `.tmd` paths `complete.rs` already completes. So DX7 shipped the
two genuine gaps: (1) **`taliesin completions --install`** — detect the shell from `$SHELL` (or a named
shell) and write the script into its conventional, XDG-aware dir (pure `install_plan`, unit-tested; zsh
carries an `fpath` follow-up, powershell is manual-only); (2) **companion completion for the file argument of
`{{< embed … >}}` / `{{< include … >}}`** — the one place page/deck/slug names are *values in a document*
(new `detectContext` context + pure `shortcodePathCandidates`). Deck-vs-page labelling by front-matter peek
and `[](page.tmd)` link completion were left out (noted in the spec). Spec
`docs/superpowers/specs/2026-07-19-dx7-dynamic-completion.md`; record in [AUDITS.md](AUDITS.md).

**Tier 2 — workflow smoothers & delight:**

**DX9 — Make caching legible — LANDED 2026-07-19.** The fresh-run `✓ 1.2s` badge already shipped; the
gap was that a replay carried a null duration, so the client showed a blank `✓` and the console said
nothing. `protocol::cell_state` gained an additive `source` field (`"cache"`|`"fresh"`); `exec.rs` marks
the up-front restored cells + returns a `CacheTally` per run, and `run()` logs one closing `restored N
cached cell(s) · M re-ran` line (only when something replayed, so a cold run stays quiet). `client.js`
renders `⚡ cached` + tags `data-qmd-cell-source` (STATUS_CSS mutes the border to a faded green), and a
dev-menu Cache row ties it to Restart kernel / `TALIESIN_NO_CACHE`. Observational only (nothing about what
runs/caches changed; freeze `FORMAT_VERSION` untouched). Unit-tested (protocol field + tally pluralization)
+ mutation-checked, a Rust drift pin guards the include_str!'d JS/CSS, browser-verified with a real kernel
(cell one `⚡ cached` with muted border; console `restored 1 cached cell · 1 re-ran`; no console errors).
**Do not re-open.** *(Known pre-existing limitation, unrelated: a freshly-edited cell's block id changes,
so its new `-out` block is inserted after the transient cell-state arrives and misses its badge — the
`✓ 1.2s` badge has always had this too.)*
**DX10 — Scaffolds that teach — LANDED 2026-07-18** (all 4 sub-parts). Shipped: `paper` now
scaffolds a worked `{python}` matplotlib figure (`#| label: fig-demo` + `#| fig-cap:`), a `$$` block, and
`@fig-demo`/`@sec-methods` cross-refs (check-clean; corpus mirror `corpus/scaffold/posts/my-paper/`
regenerated); `init`'s `index.tmd` points at `taliesin new`; `new post --draft` (a `NewOpts`-threaded flag
that splices `draft: true`, default-off so existing scaffolds stay byte-identical); and **`new deck --tour`**
(the DX10-followup, unblocked by DX5's `.columns` alias) scaffolds a guided, check-clean deck demoing
fragments/`. . .`/incremental/columns/magic-move/notes + a make-it-yours closer, browser-verified
(columns side-by-side, notes hidden). Spec/plan under
`docs/superpowers/specs|plans/2026-07-18-dx10-teaching-scaffolds*` + `…dx10-followup-deck-tour*`, record in
[AUDITS.md](AUDITS.md).
**DX12 — LANDED 2026-07-19** (the non-strict silent-failure trap). A default `build` (no `--strict`)
still ships when it hits problems (missing image / dead link / broken xref) and exits 0; the per-problem
`warn` lines scroll past above `built`, so the degradation is invisible. Now a shared
`warn_nonstrict_problems` prints one closing line — `built with N problem(s) (run with --strict to fail
the build)` — after `built`, on both build paths, a no-op when clean. The audit's `rebuilding…`
save-start line was dropped (`build` is one-shot; the `· 412ms` suffix already answers "was that slow";
a start line would be noise on every instant build). `build.rs` only (`finalize_build` +
`warn_nonstrict_problems` + `build_dir: ExitCode→bool`). End-to-end-pinned in `strict_robustness.rs`
(single-doc broken-xref + site malformed-config); det-log parity confirmed. Record in
[AUDITS.md](AUDITS.md). **Do not re-open.**
**DX13 — Social-card preview pane — LANDED 2026-07-19.** The entry's premise ("cards only bake at build")
was **stale**: a live on-demand renderer already existed (`/og/{name}` → `og_card`, `serve_site`), but it
was hash-keyed and so unreachable before `_site.yml` sets a `url:` (no hash surfaces without one). So the
gap was a UI affordance + a url-less path. `serve_site` gained `GET /og-preview?page=<rel>` (reuses the
pure/offline `render_card`+`card_spec`, keyed by page identity via `Site::page`, so it works with no
`url:`); `client.js` adds a lazy dev-menu "Show OG card" pane gated on the site-preview page identity
(site-preview only — single-doc `serve` has no `Site`/card concept). Pinned by a `serve_site` test (a
url-less corpus site still renders a real 1200×630 PNG by rel) + a mutation-checked Rust drift pin on the
include_str!'d client route/gate/CSS. Verified end-to-end: `curl /og-preview?page=intro.tmd` → 1200×630
`image/png` on the url-less demo-book, and the pane renders in a real browser (no console errors). **Do not
re-open.**
- **DX14 — Interactive `new`/`init` wizard** (arrow-key kind picker, `-y` to skip) + a `site`/`book` kind.
  M · [new] · 🎓✍️ (flags-only today; no multi-page scaffolder).
**DX15 — Pre-flight publish summary — LANDED 2026-07-19.** Confirmed real: a real (non-dry-run) deploy in
the default *gated* path printed **no gate confirmation at all** (only the ungated path warned) — exactly
the accidental-gating incident. Now a pure `preflight_summary()` prints target + source + access + checks
once the build succeeds and before the (irreversible) upload, on both the dry-run and real paths, with the
access line naming the exact flip both ways (`--public` / `publish.gate: false`, and the reverse). The loud
PUBLIC warn stays as a second unmissable line for the case that leaks. `log::info` → stderr so a
`--format json` stdout stream stays pure. Unit-tested (flip wording, problem count, dry-run verb) +
mutation-checked; verified end-to-end against the real binary (gated + `--public`). **Do not re-open.**
- **DX16 — Update-available nudge** (async, boxed, `NO_UPDATE_NOTIFIER` opt-out). S · [new].

**Tier 3 — agent-DX (the AI-native positioning):**

- **DX17 — Headless executed-output visibility** — the agent's single biggest blind spot: it can't tell
  its chart executed. `read`=source, `check`=static, `build`=python/r only, `{js}` never server-run. (a)
  let `read` project the *built* doc so baked figures surface as `[figure fig-x: produced, alt "…"]`; (b)
  optional headless `{js}` eval. L · [new] · 🤖. Overlaps ROADMAP agent work — check there first.
**DX18 — check exit-gating — LANDED 2026-07-19.** `check` gained two default-off gate flags so an agent/CI
can separate the exit decision from the (still-computed) severity + kernel-readiness. `--errors-only` drops
warnings from BOTH the output and the exit decision (a warning-only doc now passes; an error still fails);
`--require-kernel` promotes a used language whose interpreter is absent/broken or whose Jupyter kernel
package is missing from informational to a failure (a human note names the gate + language). Both are pure,
unit-tested helpers (`at_severity_floor`, `kernel_gate_fails`) + CLI exit-code integration tests
(`check_cli.rs`); wired into `CHECK_FLAGS`, `flags_for("check")` (completion), and the help/usage. The two
severities are just error/warning today, so `--min-severity` folded into `--errors-only` (a better default
than a knob whose only non-default value is "error"); note if a third severity ever appears. Record in
[AUDITS.md](AUDITS.md).
**DX19 — LANDED 2026-07-19** (data-figure recipe). The generated `AGENTS.md` gained a `## Recipes`
section with the one idiom `vocab` can't express as a closed set: the CSV→figure loop (read a data file,
plot it, `#| label: fig-*` so the output is a numbered `@fig-`-referenceable figure). **"Can't drift"** is
enforced two ways: the recipe cell is a Rust const embedded in `agents_md()` (golden-locked like the rest
of the onramp), AND a new `recipe_matches_the_corpus_example` test asserts it is **byte-identical** to a
real, `check`-clean corpus document `corpus/recipes/csv-figure.tmd` (+ `data.csv`) shipped in the same
change (the corpus-leads rule) — change the corpus example and the test fails until the const is updated +
the asset re-blessed. **Scope call:** the recipe lives in `AGENTS.md`, not `vocab` — the audit itself
notes `vocab` is "closed-set structural only", and a worked composition isn't a closed set; polluting the
structural JSON with prose would blur its contract. Verified: `@fig-sales` resolves to "Figure 1" in the
`read` projection (kernel-free), corpus invariants + golden-lock + repo-root sync + drift-lock all green.
Record in [AUDITS.md](AUDITS.md). **Do not re-open.**

**Design questions (owner ruling first — NOT build-ready):**

- Terminal hotkeys (Vite `r/o/u/c/q`) were deliberately dropped when interactivity moved to the browser
  dev menu. Cheap middle path: one banner line pointing at the `◇` menu so a Vite user pressing `h` isn't
  met with silence. (✍️🎤)
- Deck sharing/publish: the Share QR encodes `localhost:PORT`; `build` yields a file the user must self-host.
  One-command deck publish in scope? (🎤)
- Presenter laser/spotlight + auto-advance (reveal.js reflexes). (🎤)

**Suggested order:** ~~DX1~~ ~~DX2~~ ~~DX3~~ ~~DX10~~ ~~DX5~~ ~~DX11~~ ~~DX10-followup~~ ~~DX4~~ ~~DX6~~ ~~DX8~~ ~~DX7~~ ~~DX18~~ ~~DX12~~ ~~DX19~~ ~~DX9~~ ~~DX15~~ ~~DX13~~ (all landed) → DX17 (brainstorm: L, net-new, forked) · DX14/DX16 (Tier 2)
(kill the two silent-failure traps) → DX4/DX6/DX8 → DX17–19 (DX18 is cheap, pull forward). Tier 0–1 and
most of Tier 2 are *surfacing existing capability*, not net-new.

### 7. Polish audit batch (2026-07-19)

**Full findings + evidence + credit:** [2026-07-19-polish-audit.md](2026-07-19-polish-audit.md) (4 read-only
auditors across CLI/DX · authoring · live view · theming, + online research on how mature tools do these, +
per-finding source verification). **These are audit findings, not priced builds — S/M/L and value are guesses.**
Per this file's law: grep the named symbol first, trust the *symptom* not the cause/line, measure the running
product before promising. Type: [surface] expose an existing capability · [craft] CSS/visual · [author] `.tmd`
surface. Items marked **VERIFIED** were read to the line by the orchestrator; the rest are auditor-cited (still
grep first). Live browser verification was blocked (chrome-devtools profile held by another instance), so a few
interaction items (PL13, PL20's deck hint) want a live pass at build time.

**The finding that names the batch:** the tool validates + locates almost everything, yet a handful of features
offer co-equal spellings with no canonical, or *silently ignore/drop* input its own vocabulary invites. Closing
those "silent holes in a fully-diagnosed surface" is the highest lever on "feels well-thought-out". (Two more
patterns: colour is tokenized but geometry/motion aren't and the palette is duplicated 4×; and human output
under-sells machinery that already ships.)

**Tier 1 — silent holes (do-first: cheap, high-confidence, each closes a silent failure):**

- **PL10 — A `{js}` runtime error ships a raw stack trace to readers in *built* output.** `qmd-js.js:212` sets
  `textContent = String(e.stack||e)` (already `console.error`s it at `:209`); a published page shows a reader the
  stack. In the build path degrade to a terse themed notice, keep the console log. S · med. **VERIFIED.**

**Tier 1b — design-system single-sourcing (one CSS-token pass):**

- **PL4 — Single-source the owned palette; drift-lock the OG card + deck.** The palette is re-typed 4× in 3
  languages: `base.css:9-13`, `dark.css:5-11`, the parallel `--deck-*` at `deck.css:696-704`, and Rust consts at
  `card.rs:20-24`. Extract a shared `TOKENS_CSS` const into both bundles (rename `--deck-*` → `--tali-*`), and add
  a `#[cfg(test)]` drift-lock (à la `schema.rs`/`third_party.rs`) tying `card.rs` to the dark tokens. M · high. **VERIFIED.**
- **PL11 — Tokenize geometry/motion.** Colour is fully tokenized; 13 `border-radius` literals, 25 `box-shadow`s, 6
  durations (`.12s` ×23) are not. Add `--tali-radius-sm/md/lg`, `--tali-shadow-sm/md/lg`, `--tali-dur[-slow]`;
  migrate mechanically (keep 999px pills / 50% circles as specials). M · med · [craft]. **VERIFIED (grep-derived).**
- **PL12 — Tokenize the exec/error boxes** (`base.css:681-693`, `:931`, `dark.css:32-35`): hardcoded per-theme
  literals are *why* printing force-swaps the whole doc to light (`theme.rs:144-157`). Derive surfaces via
  `color-mix` from the callout tokens; drop ~6 override rules + shrink the print swap. S · med. **VERIFIED (cited).**

**Tier 2 — CLI/config consistency sweep:**

- **PL5 — Unify `--json` vs `--format json`** across the family (`init`/`new` take the former, six others the
  latter; no cross-accept, and the did-you-mean doesn't bridge them). Alias `--json`→`--format json` everywhere +
  add both to each flag-candidate list. S · med-high · [surface]. **VERIFIED (flag lists).**
- **PL6 — Route kernel failures to `taliesin doctor`.** `exec.rs:328-329` blames the interpreter path; the usual
  cause is a missing `ipykernel`/`IRkernel` package `doctor` was built to find. Append the `doctor` pointer +
  soften "fix the interpreter". S · med. **VERIFIED.**
- **PL18 — One `--format` error helper** (two wordings/styles across `check`/`map`/`symbols` vs `doctor`/`publish`)
  + resolve the hidden per-command `--out`/`--dir` aliasing. Trivial · low-med. **VERIFIED (cited).**

**Tier 2 — authoring/live-view coherence (opportunistic):**

- **PL17 — Theorem title from a leading heading, or warn.** Callouts hoist a leading heading as title
  (`divs.rs:411-423`); theorems take `title=` only (`:650-656`) — same gesture, two outcomes. S · med. **VERIFIED.**
- **PL19 — Name `.column-margin` the canonical margin-note** in docs; keep `.sidenote`/`.marginnote`/`.aside` as
  labelled aliases (four co-equal today, `base.css:655`). Trivial docs · low-med. **VERIFIED.**
- **PL20 — Deck/reader micro-polish (ship together):** cold-open stepped deck hides all nav after 3 s
  (`deck.js:1932`, `deck.css:603`) with no first-run hint (delay it / one-time hint); reduced-motion unhonoured on
  the deck's programmatic slide-jumps (`deck.js:1404`, one-line); key-sheet (`deck.js:1679`) omits Home/End/`0`;
  the "minor-third scale" comment (`base.css:351`) doesn't match the actual ratios; `og:type` hardcoded `"article"`
  for standalone builds (`mod.rs:880`). Trivial each · low.

**Design questions (owner ruling first — NOT build-ready):** deck inverts the page serif/sans logic
(`deck.css:705-711`) — accept+document or unify? · focus mode is welded to OS fullscreen (`03-focus-mode.js:39-45`,
"the author's ask") — decouple the calm column from fullscreen? · add `//| uses:` alias for the consumer
`//| input:` (opposite role to `{{< input >}}`)? — weigh vocab sprawl · callout kinds namespaced but theorem kinds
bare (the family-prefix rule holds for one family) — document or reconsider?

**Suggested order:** PL1 · PL2 · PL3 · PL7 · PL9 (silent-holes, do-first) → PL4 · PL11 · PL12 · PL8 (one
token pass) → PL5 · PL6 · PL14 · PL15 · PL16 · PL18 (CLI sweep) → PL10/PL13/PL17/PL19/PL20 fold in. PL1 or PL2
is the best single first move (both S · high, both close a silent failure).

## Tier 2 — hardening (P3)

- **Verify OG-card coverage on decks + book chapters; emit `_redirects`/`_headers` (PMF C-PUB-1).**
  `card::card_url` gates on `url:` per page (`site/meta.rs`); confirm a standalone deck and a book
  chapter each emit `og:image`/`twitter:card` (the amateur tell is one site-wide card), and that
  `taliesin publish` writes `_redirects`/`_headers` so pretty URLs and caching hold on Cloudflare
  Pages. Verify-first, may already hold. value med / effort S. Detail: `2026-07-18-pmf-audit.md`.
- **`mounts:` live serve/discovery is untested** (C5's only remaining gap, filed 2026-07-18
  after C3/C4 landed). Everything else about `mounts:` is pinned — config-parse + typo-warn
  (`config/mod.rs`), `map` JSON surfacing (`map_cli.rs`), `check` cross-mount link-tolerance
  (`check.rs`), and the build-side preview-only `mount_warnings` (`build.rs`). Untested: the live
  `serve_site` `MountedSite` discovery + serving under the `/at/` prefix (`serve_site/mod.rs`
  ~139-170), incl. the "mount 'x': no directory" warn path. It is a **bin-crate integration**
  gap — the suite has no live-HTTP serve test, so pinning it means new infra (spin the server,
  GET the prefixed URL). Low-value (mounts are preview-only, so nothing ships wrong), demand-driven.
- **Locate the site-side cross-page duplicate-label warning** (§2 #1 Part B, split out
  2026-07-18 after Part A landed). `site/xref.rs` + `site/mod.rs` push
  `"duplicate cross-reference label X defined on multiple pages"` onto the **`Vec<String>`**
  channel, which carries no location. This is **not just a channel type change** (Vec<String> →
  a located type, touching discovery/config/links/xref/book): a cross-page duplicate has **≥2
  real locations** (page A's line and page B's line), so the fix must first decide what to point
  at — the second definition, both, or a per-page list. Design the semantics before the plumbing.
  Corpus-exercised (nothing ships wrong today), so P3.
- **Phantom xref anchors — the two triggers §2 #5 left open — BOTH LANDED (2026-07-19).** (found by the
  adversarial review of the #5 fix, 2026-07-17; both reproduced on the running product, both pre-existing,
  neither shipping in any corpus doc. §2 #5 is now fully closed.)
  - **Non-executed, non-render-emitted langs (`{bash}`, `{sql}`, `{julia}`, …) — LANDED 2026-07-19.**
    #5 gated on `include`, but a lang that is neither executed nor emitted at render time had NO
    figure/table for *any* `include`, yet a `label: fig-x`/`tbl-x` on one still burned a number +
    registered a phantom (verified: `{bash}` + `label: fig-shell` → real figure shifted to "Figure 2").
    Fix: a canonical `taliesin_core::render::executes_to_kernel` (python|r) now shared into core; both
    the figure arm and the table arm gate on `emitted_at_render_time(lang) || (executes_to_kernel(lang)
    && include)`, so a non-materializing lang keeps its **visible source** (not a numbered float), burns
    no number, and draws a located `check` warning naming the lang (`{bash}` is not executed…). Server's
    `exec::kernel_lang` is **drift-locked** to the core set by a test (the "shared, not guessed" the note
    asked for). Pinned: two render unit tests (figure + table axes, mutation-checked) + the drift-lock;
    the sibling `include: false` phantom tests (`hidden_cell_xref_targets.rs`) still green. **Do not
    re-open.**
  - **Empty output also phantoms — LANDED 2026-07-19.** `exec.rs` drops output on
    `inner.trim().is_empty() || !cell.include`, so a `label: fig-x`/`tbl-x` cell that RUNS but prints
    nothing left a dead anchor render had already committed a number to (verified: `label: fig-silent` on
    `x = 1+1` → `@fig-silent` links to a "Figure 1" no element carries, and the sole real figure shifts to
    "Figure 2"). It is a **post-execution** fact — render can't see the output will be empty, and cross-refs
    are resolved at render time (`cite::process`), so unlike #5 it can't be declined or un-burned; the
    number/ref are baked. Fix: a pure `empty_labelled_float_warning` (`exec.rs`) drives a build/serve
    `log::warn` at the output-drop point, naming the anchor + kind (figure/table) and telling the author to
    drop the label or emit output. Deliberately narrow — `include: false` (render already warns via
    `unreferenceable_hidden_label`), kernel-unavailable (its `inner` is the non-empty diagnostic, not
    empty), and unlabelled-empty cells all stay silent; fires on cached replay too (empty output is frozen).
    Pinned: 2 unit tests (figure + table warn / the three silent cases), each mutation-checked;
    end-to-end-verified against the real binary. **Do not re-open.**

- **Execution-cache leaks — remainder** (exec/kernel Do-NOT-touch, careful):
  - **Ungraceful-death path (S/M) — warm-pool half LANDED 2026-07-19; two sub-parts remain.**
    (Measured baseline: `kill -9` on a real preview orphaned a 5-proc / ~243 MB forkserver
    subtree; 22 h+ orphans + 4199 stale `/tmp/tali-*` dirs were found in the wild.)
    - ~~warm-pool forkserver subtree orphans on SIGKILL~~ **DONE:** the helper now self-reaps
      its process group on stdin EOF (parent death) via `os.killpg(getpgrp, SIGKILL)` in
      `warm_pool.rs`'s `FORKSERVER_HELPER` — the same group `kill_process_group` reaps
      gracefully, triggered from inside. **Deliberately NOT `PR_SET_PDEATHSIG`** (the backlog's
      named mechanism): PDEATHSIG signals only the helper (leaving the server + kernels orphaned)
      and can fire on a parent *thread* exit in the tokio runtime; stdin EOF is parent *process*
      death and reaps the whole subtree. Pinned by `ungraceful_parent_death_reaps_the_forkserver_subtree`
      (mutation-checked); verified e2e (5-proc/243 MB → 0 on SIGKILL). **Do not re-open.**
    - **Cold-kernel orphan (NEW, found by the repro; still open).** A single-doc preview cold-starts
      the kernel (no warm pool), a direct child of taliesin with `stdin(null)`; on SIGKILL it survives
      orphaned and leaks its `/tmp/tali-kernel-*` dir (reproduced). No stdin-EOF hook (ipykernel doesn't
      read stdin), so the clean fix is ipykernel's `ParentPollerUnix` (process-death based, safe) — NOT
      a bare PDEATHSIG (thread-exit hazard + silent mid-session kernel-state loss). `kernel.rs:561`.
    - **Stale-`/tmp`-dir sweep (still open; backlog premise was wrong).** The runtime dirs
      (`tali-warmpool-<uuid>`, `tali-kernel-<uuid>`, `tali-interp-<name>`) are **UUID/name-named — no
      owner pid**, so "sweep dirs whose owner pid is dead" cannot work as written. The bulk of the 4199
      is TEST debris (`tali-omit`/`tali-check`/`tali-sbe`/…, which *do* carry a pid). A real sweep needs a
      design call: encode the pid in future runtime dir names + sweep pid-dead ones, or age-based (racy
      vs a long preview). Wiring a startup sweep also touches `main.rs`.
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
- **PMF audit demand-driven tail** ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md), Tier C).
  Filed here, not built, because each waits on a real user asking (the audit's #1 action is *get
  users*, not more features): hover-preview extended to inline `[@key]`/footnotes (reuse
  `site/hover.rs`; note the width-gated sidenotes already cover part of the Distill reader-layer job);
  reader-owned document-level show/hide-code toggle (cell `echo`/`code-fold` already exist; a reader
  switch is a reader-local pref, a11y-exempt from minimal-config); on-page code+data download plus a
  "reproducible" affordance (overlaps the cut repro-manifest, `FEATURE-IDEAS.md`); scroll-synced TOC
  greying passed sections; versioned/permanent-URL scheme for link-rot distrust; deck autoplay/kiosk
  loop; and a docs "deck powers" page (the deck already ships a `?`/`m` shortcut menu with a
  jump-to-slide list, so the gap is that first-timers do not know it exists: docs, not code). A
  Zenodo DOI on-ramp (`CITATION.cff`/`.zenodo.json` to a GitHub-release DOI) belongs with Wave 5's
  repro/print-pdf track; private-share via Cloudflare Access is already filed under the `publish`
  follow-ups above.


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

**2026-07-18 PMF audit re-derivations.** The audit
([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md)) re-proposed two already-ruled items and the
owner ruled on both the same day. (1) **Reader "Cite this" box (audit B1) = D70: REVIVED, not
declined.** The owner reversed the same-day decline and explicitly wants it, so the 2026-07-12 "Cite
this" cut and the D70 decline are SUPERSEDED; it now lives in Build-ready with an author-metadata
gate. (2) **Deck desktop "async handout" reading view stays CUT.** The audit re-derived the exact job
the deck reader/scroll-mode deletion (2026-07-12) addressed; that ruling split it into overview
(browse) + phone-feed (read), leaving only the *desktop send-me-the-slides* slice, which stays cut
unless the job is proven with a real audience. Do not re-open (2) without a fresh ruling.

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Open-source the repo + publish the site when
ready; the security token gate is shipped.

**PMF audit, 2026-07-18 ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md)):** the tool is
feature-complete for ~one real user, so the highest-leverage next move is not more features but
**real users**. The owner is publishing soon to gather feedback (the audit's action A1). When
publishing, lead the copy with the **speed moat** (warm server, block-level incremental, no per-edit
rebuild), the single most-repeated Quarto grievance and the most under-marketed asset (A3). Marketing
build-out stays feature-first (see the deferred marketing-site item in Tier 3).
