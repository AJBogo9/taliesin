# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git, [AUDITS.md](AUDITS.md) and
> [ROADMAP.md](ROADMAP.md); **delete an item when it lands**, never leave a `[x]`. Method lessons
> that outlive their item go to [LESSONS.md](LESSONS.md). "Do not re-add / re-scope" is a compact
> anti-rot guard — **one line per entry**, not a changelog. This file was 1,767 lines on
> 2026-07-29 because that rule was not enforced; if it is growing again, the fix is to move detail
> out, not to add a summary at the top.

## Now

**Fresh session with no context: read this section, then "Standing constraints", then P1. That is
enough to start.**

- **Ask git, never this file, for git state.** No SHA, branch name or commit count is recorded
  here on purpose: the author and parallel sessions both push, and a recorded SHA is the line that
  rots first.

  ```sh
  git log --oneline origin/main..HEAD   # what is unpushed
  git branch -vv                        # what branches still exist
  ```

- **The board was refilled on 2026-07-29 by an owner ruling.** Every feature parked in the old
  "Tier 3, demand-driven" tail was reviewed with the author and **promoted**. That includes the
  **print/PDF track**, which the author had been cool on and is now warm to, so its Wave 5
  deferral no longer holds. P1 is therefore a **ranked build queue of 24**, not a drained board.
  Take from the top.
- **Exactly one thing was declined:** the FL-weather Quarto migration, which is now the sole
  line in the demand-driven tail.
- **Everything below P2 is still blocked** on an owner ruling, a device, or a real user. The
  audit slate is complete except **R12** (real-device mobile, Android, needs the author's
  phone), and **no new round should be opened**: an audit's value decays to zero if its findings
  never ship, three waves of them have now shipped, and the P1 queue is now the work.
- **Nothing is owed by the author** except R12 and the rulings in P3.
- **Two measurement hazards, both of which have cost time.** (1) `target/release/taliesin` is
  shared across sessions and may be built from another branch — check `taliesin --version` against
  your own HEAD before trusting any CLI number. (2) A table-shaped probe whose every cell is
  negative is a **broken probe** until proven otherwise; carry a known-positive row. The full trap
  catalogue is [LESSONS.md](LESSONS.md), and it is worth reading before writing any probe or pin.
- **Entries rot; the rule for reading one is in "Standing constraints" below.** It has now been
  vindicated on three consecutive batches — most recently item 151, whose filed cause was flatly
  false while its symptom was real.

## Standing constraints (read before working)

- **Do-NOT-touch (one freeze):** `MAX_WARM_PAGES` + the deterministic LRU eviction in
  `serve_site/exec_pool.rs` (M6a, sign-off refused 2026-07-17) and the **single-editing-surface**
  invariant (the preview is read-only; it must never write back to source). The rest of the
  exec/kernel zone is not frozen.
- **Website / brand** (2026-07-11 audit, detail:
  [2026-07-11-website-design-audit.md](2026-07-11-website-design-audit.md)): the personal blog
  (`corpus/tech-blog/`) is the forward-facing brand, direction **"Marginalia"**; its 14 explicit KEEPs
  live in that file. Every change stays invariant-safe: no CDN, no preview write-back, no new output
  format, offline bundling, `--tali-*` tokens only.
- **Author policy:** feature-first (finish framework features before marketing-site work).
- **Working method:** branch per feature; brainstorm if there's a fork; spec under
  `docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
  extension harnesses); fast-forward merge locally; **delete the item here when it lands.** Push to
  `origin/main` only when the author asks. **Review subagents get a git worktree or you commit
  first** (a "read-only" reviewer with `Bash` still writes scratch files to your CWD; one ran
  `cat > Cargo.toml` in the repo root and destroyed the workspace manifest).
- **Tests: three gates, or the suite silently under-tests itself:** `TALIESIN_REQUIRE_NODE=1`,
  `TALIESIN_R=R TALIESIN_REQUIRE_R=1`, `TALIESIN_PYTHON=… TALIESIN_REQUIRE_KERNEL=1` (a missing
  interpreter must be a hard fail, not a skip). `cargo test` aborts the remaining binaries at the
  first failure, so re-run before trusting a total. A **fourth** gate nothing else runs:
  `TALIESIN_REQUIRE_CHROME=1 --test read_run_js`.
- **A red `exec`/`kernel` probe is now real signal, not a coin flip.** The flake was fixed 2026-07-25
  (a port race in `prepare_connection`; the re-roll now lives on `Kernel::start_with_retry`, and
  `crates/server/tests/kernel_start_is_retried.rs` fails if any caller reaches the un-retried
  primitive). Verified 0 failures in 45 post-fix runs under the same load.
- **`corpus/tarn` is the fixture for scale-sensitive work** (12 numbered chapters, 3 parts + a nested
  part) and deliberately carries the shapes the rest of the corpus lacks. **Use it instead of minting
  a fixture.** It is a *documentation* book, not a scale fixture: do NOT grow it toward 200 pages and
  do NOT mint `corpus/longbook` (the walker renders every corpus doc on every `cargo test`).
- **Git:** `git reflog show origin/main` before believing any "not pushed" claim in any notes file.
- **How this file lies to you:** entries rot. Before picking an item, **grep its named symbol/flag in
  source** and prefer measuring the running product over reading this file. Trust an item's
  *symptom*, never its cause, line number or stated cost. Verify a fix by **mutation** (restore the
  bug, watch the named test fail), not by a green suite. **What would ship silently is tracked
  per class in [DETECTION-DEBT.md](DETECTION-DEBT.md)** — a live register, updated in the same
  change as the fix, not a dated findings doc. **The full trap catalogue — probes,
  instruments, cargo-mutants scoping, the coverage illusions — is in [LESSONS.md](LESSONS.md); read
  it before writing a probe or a pin.**

## Open items

**Ranked by what a session should pick up, not by theme.** P1 is buildable today; P2 is filed so it
is not rediscovered as a defect; P3, P4 and P5 are blocked and are listed so they are not
re-scoped. **Item numbers are stable** and are referenced from the findings docs and
[AUDITS.md](AUDITS.md): they are never renumbered and a closed item's number is never reused.

**Standing rule for a batch:** branch per batch, verify each fix by *mutation* (restore the bug,
watch the named test fail), browser-verify anything client-side, and **delete the item from this
file when it lands**.

### P1 — build now

**A ranked build queue, not a menu: the order below IS the priority order.** Take from the top.
The ranking encodes three things: **dependencies** (153 before its graduate 158), **size** (cheap
substrate and small wins first, the two large swings at positions 14 and 15), and the author's
standing **feature-first policy** (170, the marketing site, is deliberately last).

**Items 153-174 were promoted on 2026-07-29 by owner ruling** from the demand-driven tail, where
several had sat since 2026-06-24. Each keeps a **pointer** to its design detail in
[ROADMAP.md](ROADMAP.md) or [FEATURE-IDEAS.md](FEATURE-IDEAS.md) instead of re-expanding it here;
that is the anti-bloat rule this file exists under. Two standing conditions apply to all of them:

- **Each still owes a corpus pin doc** (corpus-plus-roadmap: a capability ships pinned by a target
  corpus document added in the same change). Where the pin is already named upstream it is
  repeated below. **Do not grow `corpus/` past the pin a feature needs**, the walker renders every
  corpus doc on every `cargo test`.
- **Promotion is not a design.** Several of these were parked with an open design question, not
  just for lack of demand (166's line-shift problem, 160's source-map gate, 155/156's reactive-VM
  trap). Those say so; brainstorm before coding.

150. **Phase A2: site-aware in-editor preview.** (MEDIUM, own spec.) Opening a book chapter in the
     companion previews the single file, so the author gets an orphan page: no nav, dead cross-page
     links. Resolution rule is the nearest `_site.yml` walking up (the include-root rule, **never**
     `.git`); the file-to-URL map already exists in Rust as `taliesin map <dir> --format json`
     (`{rel, url}` per page), so TS reads JSON and reimplements nothing. **The risk is not the
     wiring:** once the webview navigates between pages, `docPath` goes stale and
     `resolveSourceFile` (`paths.ts:39`) resolves a `tali-goto` from page B against page A's
     directory, opening the wrong file. `relativeKey` has the mirror problem. Resolution must key
     off the project root; `serve_site` already emits `root` in `TALIESIN_DOC`
     (`serve_site/mod.rs:788`), so the data is there. Write the spec before the code.

56. **L5-1 residual: the manual's cross-page references.** (The `description:` half shipped
    2026-07-26: 0 of 36 tracked pages → 36 of 36.) What is left is not the authoring pass the item
    assumed, and splits two ways:
    - **Glossary, term index and float digest have no surface to feed.** `glossary`, `term-index` and
      `float-digest` grep to **zero** across `crates/core/src` + `crates/server/src`, so "they render
      empty until an authoring pass happens" describes a *feature proposal*, not authoring work.
      Writing `{.definition}` blocks today feeds only `skim.rs`, which reads them as statement heads.
    - **Backlinks ship and render nothing, and authoring genuinely could fix that.**
      `site/backlinks.rs` builds its reverse index from **cross-page** xref markers; the books' 33
      xrefs (17 guide + 16 internals) are all intra-page, so **0** "Referenced by" lines are emitted
      in either book. Real cross-chapter references would light it up, but they have to be references
      someone means — a writing judgment, not a sweep.

### P2 — filed so it is not rediscovered as a defect

Not worth a session on its own. Each is a record or a known cost, not a task.

131. **The cold-build cliff: 3,981 ms vs 789 ms warm.** (LOW, and probably correct as-is.) Filed so
     it is not rediscovered as a defect. Kernel *variable* state is never cached — the property that
     makes the cache trustworthy — so a cold start genuinely cannot skip work unless the whole
     document is unchanged. **The waste is inherent to a correctness guarantee worth keeping.**

129. **Shape inventory from two real external documents — the durable half of R11.** (MEDIUM, mostly
     a record.) What real documents contain that `corpus/` has nowhere: `lang,attr` fences (734
     occurrences → item 127), ` ```console ` (209), links with a non-`.tmd` extension (128 → item
     128), a `SUMMARY.md`-driven chapter spine, **112 pages in one flat directory** (the largest
     corpus project is 14), and chapter files with **no front matter at all**. **Do NOT grow
     `corpus/` toward these** — the walker renders every corpus doc on every `cargo test`. **Pin
     only the two that earned it** (127 and 128) — **both pinned and shipped 2026-07-28**: the
     `lang,attr` fence is now a fixture in `corpus/highlight.tmd` with its own test, and the
     link-extension shape has `crates/core/tests/migrated_link_extensions.rs`. The rest are
     recorded so a later round does not re-derive them.

152. **RESOLVED 2026-07-28: the companion e2e suite runs again.** It had been failing with
     `EMFILE: too many open files` inside VS Code startup because `fs.inotify.max_user_instances`
     was still the kernel default of 128 while the desktop session already held ~154 (dconf ~40,
     code ~32, plus Electron apps). Raised to 512 via `/etc/sysctl.d/99-inotify.conf`. Kept here
     as the diagnosis, because the same limit throttles `taliesin preview`'s own file watchers:
     if previews ever stop hot-reloading, or VS Code refuses to start, check
     `find /proc/*/fd -lname 'anon_inode:inotify' | wc -l` against
     `/proc/sys/fs/inotify/max_user_instances` before suspecting the code.

### P3 — blocked on an owner ruling (not a task until then)

101. **State the licence position on what Taliesin *emits*.** (HIGH, ruling.) Measured: **zero**
     statements across README, LICENSE, THIRD_PARTY, SECURITY and both books' source about what a
     user's output is licensed as. It is a genuinely non-obvious question here because every built
     page inlines AGPL-licensed CSS/JS (`base.css`, `deck.js`, `tali-js.js` spot-checked, **none
     carries a licence header**), so a user's blog contains AGPL material. **No licence change is
     proposed** — the finding is that the position is unstated, and stating it discounts the
     licence, bus-factor and portability anxieties at once.
     **Narrowed 2026-07-28:** the other branch added `crates/core/assets/js/LICENSES.md`, which
     carries the full permission notices for the **vendored third-party** bundles. That is adjacent
     and good, but its own text says "This covers the redistributed third-party bundles only", so
     **it does not answer this item**: what a *user's built page* is licensed as, given it inlines
     Taliesin's *own* AGPL scripts, is still unstated.
     **Merged in from the critique round's item 88 (2026-07-28), which found the same question
     independently with sharper evidence and a named remedy** — that item is retired into this one:
     - **Mechanism, measured.** Taliesin's own runtime JS is `include_str!`'d into every page a user
       builds (`render/mod.rs:1658-1660` plus the `code-enhance/` fragments): a probe page measured
       **1.2 MB with 13 `taliEnhancers` hits and zero licence statements**. If that runtime is AGPL,
       arguably **every page a user publishes is an AGPL work** — an adoption tax larger than §13's,
       landing on *document authors* rather than on a hosted competitor.
     - **The standard remedy is known:** an explicit output exception (the GCC runtime-library /
       Bison-output pattern), or an MIT carve-out for the emitted runtime only.
     - **Decide before publishing anything.** The first published page fixes the answer in the wild.
     - **A separable second question the same finding raised: AGPL vs MPL-2.0.** §13 is *not*
       inapplicable here — a `--host` LAN preview is network interaction (`LICENSE:542-548` +
       `SECURITY.md:44-47`) — so the "nobody stands in that hole" framing is wrong. But the tax
       lands on exactly the adopters the project needs. MPL keeps file-level copyleft, passes most
       corporate bans, and ***REMOVED*** `deny.toml` protects.
     - ~~**Either way, the reservation at `README.md:156-158` is fiction the moment one outside PR
       merges without a CLA or DCO.**~~ **DISCHARGED 2026-07-28 by item 89:** `CONTRIBUTING.md`
       clause 3 is the inbound grant (perpetual, worldwide, irrevocable, sublicensable, explicitly
       including **relicensing**), and `gate_script.rs` fails the suite if that grant disappears.
       **This does not touch the rest of item 101**, which is about what a *user's built page* is
       licensed as, not what a contribution is.

100. **RULED 2026-07-28 — the answer is "archive plus fresh public", and it is specced.** See
     [2026-07-28-public-flip-audit-design.md](../docs/superpowers/specs/2026-07-28-public-flip-audit-design.md).
     The ruling threads the needle both routes below missed: **the history IS published** (1,608
     single-author commits are the evidence a grant applicant wants), and the private planning docs
     leave *every* commit that ever held them. Mechanism: relocate the purged docs to
     `~/Documents/personal/taliesin-private/`, rewrite history, rename this remote to
     `taliesin-private-archive` (stays private, complete backup), create a **new public**
     `AJBogo9/taliesin` and push the rewritten history there. No force-push, no destructive remote
     op, and the private blobs never reach the public repo at all. Zero forks and never having been
     public is what makes it cheap. **Kept, not purged:** security audits, `.claude/`,
     `docs/superpowers/`, `AGENTS.md`, `LESSONS.md` — for the stated goal those are the exhibit.
     **Purged:** money and strategy documents only, plus ~11 commit subjects that name them.
     **Execution status: NOT STARTED and not to be started without a separate instruction.**
     Phase 1 is a read-only audit and is safe whenever wanted; **Phase 2 is irreversible** and is
     additionally gated on Phase 1's findings being signed off.
     **What still lands on this item:** the spec's own D-checks (incl. the provenance check on
     corpus documents), and its rule that any still-open finding reading as an **exploit recipe** is
     reported for individual judgement, default keep — which is exactly items **79, 80, 81**, so
     **fix those before Phase 2** rather than deciding whether to redact them. **All three are
     FIXED (2026-07-28, `launch-blockers-2026-07-28`, unpushed)**, so this clause is discharged once
     that branch is merged — verify, do not trust this line. **New input for Phase 2 from item
     83:** five local tags ship an MIT `LICENSE` and none has been pushed, so whether tags travel
     to the new public repo belongs to this spec.
     *Original framing, kept because it records why the ruling was hard:*
     `notes/STARTUP-PLAN.md:126` records a plan to publish as a **fresh repo with no history**
     ("Keep this repo private forever; the public one is a separate repo"), *not* to flip this
     repo's visibility. Those two routes resolve different findings, so the prune work cannot be
     scoped until this is ruled. Two hard facts either way:
     - **`notes/FUNDING-RESEARCH.md` and `notes/STARTUP-PLAN.md` are git-TRACKED while their own
       text says they must not be** (`FUNDING-RESEARCH.md:4` "keep this file out of";
       `STARTUP-PLAN.md:119` "remove anything private: `STARTUP-PLAN.md`"). They carry the ***REMOVED***
       analysis, a table of named funders being skipped and why, a funder's contact address, and
       "***REMOVED***". The 2026-07-17 round already filed this and
       recorded the prune as **not done**.
     - **A fresh `git init` fixes none of the tree-level findings** and discards the 1,573-commit
       process record, which is the strongest evidence an individual grant applicant has. The
       due-diligence doc's §6 proposes a third route (targeted `filter-repo`) with its honest cost.
     Supersedes the "flip-day artefact checklist" framing; **extends item 25, does not replace it.**

122. **`check` says "no problems found" on a document whose code cell cannot run — BUT ITS
     PROPOSED FIX REVERSES A DATED RULING, so read this before building it.** (MEDIUM.)
     Measured cold: plain `check` prints exactly that and exits 0, while `build` on the same file
     warns twice and `doctor` names the missing package. The Environment section is shown **only**
     to a user who already passed `--require-kernel`. **Do not** make that flag the default (it
     would break the kernel-free property). Filed fix: print the Environment line unconditionally
     when the document contains a code cell, exit code unchanged.
     **The conflict, found 2026-07-28 while shipping item 81** (which touches this exact code):
     that is **PL14**, a deliberate decision with a spec
     ([2026-07-19-pl14-check-env-footer.md](../docs/superpowers/specs/2026-07-19-pl14-check-env-footer.md))
     and a test that pins it by name — `default_human_check_omits_the_environment_block`
     (`check_cli.rs`), whose comment states the reason: the footer "duplicated `doctor` on every
     keystroke/CI run". Implementing 122 as filed deletes that test. **So this is an owner ruling,
     not a task.** A shape that satisfies both is available and is the recommendation: print the
     Environment *line* unconditionally but keep **not probing** by default, so the line names the
     interpreter that would be used and says it was not spawned. Item 81 already built exactly that
     reporting shape (`runs: null` + `not_probed`), so the remaining work is only where the line is
     printed. **Cost to check first:** `collect_environment` re-renders every page of a site to find
     used languages, so putting it on the default path doubles a site `check`'s render work
     (`check <site>` was measured at 538 ms). Measure that before wiring it in.
     **MEASURED 2026-07-28 (release `b8c93bb`), and the cost objection is weaker than filed.**
     Default vs `--require-kernel` (which is the only surface that collects the environment today),
     best of three each: `docs/guide` (20 pages) **0.36 s → 0.54 s**; `corpus/tech-blog` (17)
     **0.54 s → 0.79 s**; `docs/internals` (15) **0.21 s → 0.31 s**; `site` (5) **0.11 s → 0.15 s**.
     So it is **about +50%, not a doubling**, and **+100-250 ms absolute** on the largest projects
     in the tree — and the "538 ms" in the line above did not reproduce as a *baseline*: 540 ms is
     the whole `--require-kernel` run on the slowest project. **That delta is an upper bound for the
     recommended shape**, because it includes actually spawning the interpreters, which the
     "name it, do not probe it" line would not do. The remaining open question is not cost but
     whether used-language detection can be had without the render walk; if not, the walk is what
     the +50% buys.

102. **Decide what to do about constructs that render elsewhere and silently do not here.**
     (Ruling.) Detail in [adoption friction](2026-07-27-adoption-friction-audit.md).

103. **Clear the name in software classes before the flip.** (Ruling, legal not code.) Trademark
     search in the relevant classes; the name is the retained optionality per the product stance.

71. **Two deck-on-touch behaviours that are working-as-written, and may be working-as-wrong**
    (DT-3 + DT-4, detail: [2026-07-27-deck-touch-audit.md](2026-07-27-deck-touch-audit.md)).
    Neither is a bug; both are a choice someone made that the touch crossing put a number on.
    - **A slow swipe does nothing.** Measured: 200 px in ~30 ms navigates, the same 200 px over
      **750 ms** does not (`deck.js:1859`, `dt > 600`). A swipe's time bound normally separates a
      swipe from a pan/scroll — but in stepped mode there is no competing one-finger gesture to
      separate from (`deck.feed` returns at `:1798`, `deck.overview` at `:1799`, both above it, and
      the stepped stage does not scroll), and the 50 px distance floor already rejects a tap. So in
      the only mode where the bound is live it can *only* reject input the reader meant, and what it
      rejects is the slow deliberate swipe a motor-impaired reader makes. Proposed: drop `dt` in
      stepped mode, keep the distance floor. **No real user has been observed failing on it.**
    - **The share panel says "Point a phone here" — to a phone.** The QR takes most of the card and
      is the one useless half on the device reading it; Copy is the action that works and is
      secondary. Panel geometry is otherwise correct at 390 px (nothing clipped, QR legible).
      `navigator.share` was absent under emulation, **so the Web Share option was not measured and
      is not claimed**.

148. **Distribution: the binary channel now has a MECHANISM but still has no artifact; the package
    managers are untouched.** **Amended 2026-07-28 by item 92** — read this before re-filing any of
    it. What shipped: `.github/workflows/release.yml` builds Linux x86-64, macOS arm64 and macOS
    x86-64 on a `v*` tag, attaches a tarball + `.sha256` with `LICENSE` + `THIRD_PARTY.md` inside,
    and the README states the matrix (Windows explicitly unsupported). **What is still true:** no
    tag has been cut and the workflow is guarded inert until the repo is public, so `gh release
    list` is still empty and there is still nothing to download **today**; crates.io `taliesin` /
    `taliesin-core` / `taliesin-server` are all still 404 (all three names free); no Homebrew, Nix,
    or install script. The remaining work is therefore **cut a tag after the flip**, then decide
    about crates.io / brew / nix separately.
    - **Cold-build cost re-measured 2026-07-28: 2m11s, 268 crates, 2.6 GB peak RSS at `-j4`**, for
      one ~38 MB binary; the README now states this. The filed **2m59s** was a different machine or
      job count, not a regression. Either way the argument stands: the audience for a documentation
      tool is not the population that will install a Rust toolchain and wait it out, which is
      exactly why the release workflow exists.
    - **Prerequisite the critic missed and the defender found:** `cargo publish` will *reject*
      this workspace as-is. `Cargo.toml:14` declares `taliesin-core = { path = "crates/core" }`
      with **no `version`**; add `version = "0.2.0"` first.
    - Also blank on crates.io without it: no `keywords`, `categories`, `readme`, `homepage` or
      `documentation` in any manifest, so the crate pages would carry one description line and
      nothing else. Watch `crates/core` = 7.3 MiB tracked against the 10 MiB `.crate` cap.

149. **Launch presentation, all gated on the flip.** Grouped because none is actionable until the
    repo is public, and each is small once it is.
    - **The README does not lead with the speed moat**, contradicting this file's own ruling at
      `:577-579`. "Quarto" appears **zero** times in the README and in `site/*.tmd`, and
      `tools/live-edit-bench/RESULTS.md` (cold 123,994.9 µs vs warm 28,425.1 µs, diff 685.6 µs,
      83× smaller payload) is cited from nowhere. Note the ruling says *lead with the moat*; it
      does not say *name Quarto* — that inference is the critic's.
    - **The GitHub repo is a dead first impression**: description defines Taliesin in terms of
      Taliesin, `homepageUrl` empty, one topic ("rust"), zero releases, and the README's only
      image is the licence badge — while four screencasts demonstrating the moat sit committed
      in `site/assets/` and appear on no page a visitor sees. (They are MP4; a GIF conversion or
      an uploaded asset URL is needed, not a one-line embed.)
    - ~~**No platform statement anywhere**~~ **DONE 2026-07-28 (item 92).** The README carries a
      platform matrix naming the three built targets and stating Windows unsupported, and
      `release_targets.rs` pins it against the release workflow in both directions. The underlying
      fact is unchanged and still worth knowing: `/proc` is read directly in five places with
      `#[cfg(not(unix))]` fallbacks that `LESSONS.md:88` records as never executed by any test.
    - **CoC and issue templates only** — ~~CONTRIBUTING / CLA or DCO~~ **DONE 2026-07-28 (item
      89).** `CONTRIBUTING.md` exists and its clause 3 is the inbound grant, explicitly including
      relicensing, so `README.md:156-158` is no longer ended by the first merged outside PR;
      `gate_script.rs` fails the suite if that grant disappears. Still absent: a code of conduct and
      GitHub issue templates, both of which are only worth doing once the repo is public.
    - **`taliesin.dev` resolves to nothing** (registered, NS + SPF + a google-site-verification
      TXT, zero web records) and is baked into every canonical URL, `og:url`, sitemap and feed.
      `site/README.md:11-12` already flags it as a placeholder.
    - **`taliesin build site` 404s its own primary CTA** — `docs/guide/` and five `gallery/*`
      mounts are preview-only, and `site/README.md` documents an 8-command build that nothing
      runs. Worse than filed: `--strict` exits **0** on it and `check` says "no problems found",
      so both automated gates bless a deploy the tool has already warned about. A `site/build.sh`
      is the cheap fix; counting mount warnings as `--strict` problems is the durable one.
    - **The name** (surfaced, not a task): TALIESIN is a live registered mark of the Frank Lloyd
      Wright Foundation (Reg. 4150375). Software is outside the recited goods so legal risk is
      low; the cost is permanent SEO invisibility, and `github.com/taliesin` + `/taliesins` are
      both taken. Renaming twice is worse than a bad search name — if keeping it, always publish
      as "Taliesin — the `.tmd` dev server" so the disambiguator travels.

25. **Pre-public release: the flip procedure, and a contradiction to resolve first** (detail:
    [2026-07-17-security-release-audit.md](2026-07-17-security-release-audit.md) and
    [2026-07-28-launch-critique.md](2026-07-28-launch-critique.md)). All five code items shipped
    2026-07-25. **oss-4 was ruled 2026-07-25: deferred** ("I'll do it at the end of summer").

    **Author leaning, 2026-07-28 (a leaning, NOT a ruling — re-confirm before acting):** do a
    **visibility flip** with the sensitive documents removed, deliberately keeping the commit
    history public so readers can see how the work was done.

    **The one fact that decides whether that plan works.** A visibility flip exposes **every past
    commit**, and `git rm` in a new commit does not remove a file from history. Two documents are
    tracked and both instruct otherwise in their own headers —
    `notes/STARTUP-PLAN.md:3-5` ("keeping it out of any public release") and
    `notes/FUNDING-RESEARCH.md:4` ("keep this file out of git") — and they contain the ***REMOVED***
    ***REMOVED***, and "MIT
    would let a competitor or a cloud provider close it against you. ***REMOVED***."
    So **"flip + delete the files" leaves them fully readable in history**, which is the opposite
    of the intent. Only three options actually work:
    - **(a) Flip, and rewrite history first** (`git filter-repo` over those paths). Keeps the
      visible history the author wants, at the cost of rewriting every SHA — and any SHA recorded
      in `notes/` or in a findings doc stops resolving.
    - **(b) Fresh public repo** per `notes/STARTUP-PLAN.md:111-127`, which is a *dated ruling*
      ("decided 2026-06-18") prescribing exactly this: `rsync -a --exclude='.git'`, remove the
      private docs, "Keep this repo private forever; the public one is a separate repo." Clean,
      but **discards the commit history**, which is the thing the author said they wanted to keep.
    - **(c) Flip as-is and accept the exposure.** Cheapest, and the least consistent with having
      written "keep this out of git" twice.

    **Note the procedure collision, because two committed documents currently disagree:**
    `***REMOVED*** (fresh repo), while this file and
    `2026-07-17-security-release-audit.md:217-218` both sequence the `oss-*` items to "whenever
    the repo actually flips public". Whichever option is chosen, **fix the losing document in the
    same change** or the next session will follow the wrong one.

    Still open under whichever route: whether to prune `notes/` + `docs/superpowers/`. The
    deferral's stated reason — "no secret is exposed … but it is a curated bug roadmap" —
    describes the audit notes and **does not describe the two files above**, which is why they
    were never named in it. Scale, measured: `git ls-files notes/` = 63, `docs/superpowers` = 69,
    and the largest is `2026-07-03-quarto-design-decisions-catalog.md` at **1,129,387 bytes** of
    adversarial self-critique sitting under `docs/`, which a visitor reads as "the manual".

    **Correction to this item's own former text.** It claimed "**Verified NOT open, do not
    re-scope:** … the tracked `/home/bogo` paths are scrubbed." Measured 2026-07-28:
    `git grep -Il "/home/bogo"` → **11 files**. The 2026-07-17 scrub was scoped to the four paths
    under `docs/superpowers/*` and did do that; the summary generalised it to "the tracked paths",
    and one new occurrence has since accreted (`2026-07-18-shell-completion-dynamic-design.md:189`,
    dated the day after). Eight of the remaining ten are `notes/*` prose covered by the prune
    above, and two are self-references *documenting the scrub*. Low impact — the username is
    already public via git author metadata — but **a "verified NOT open" line in this file was
    measurably false**, which is the failure mode `LESSONS.md` warns about. Still correctly
    closed: `SECURITY.md` exists, PT-1 / PT-2 / NET-1 / OUT-1 / DEP-01 / DEP-02 all shipped
    2026-07-17, and `dos-yaml` + NET-3 were refuted.

### P4 — blocked on a device, a real user, or working-as-intended

Kept visible so they are not re-scoped. Revive on a real signal, not on capacity.

4. **Deck engine mobile polish** (P2): mobile pinch/pan + touch gestures (they matter for the
   phone-feed deck mode); drop `fitSlide` from the resize path (needs a lazy fit-on-show refactor
   first). *(The desktop trackpad half shipped 2026-07-24 — pinch / ctrl+wheel-down opens the overview
   map, with a 250 ms hysteresis.)* **The device blocker is gone.** **Partly measured 2026-07-27**
   (deck × touch round): with synthetic touch events, swipe navigation works (h 0→1→0), a two-finger
   pinch-in opens the overview, and an overview one-finger pan neither navigates nor exits (B6-31
   holds). **What is still unmeasured is the part emulation cannot reach**: a real finger, and
   overview pan while zoomed *past* fit — at fit scale `clampOv` has nothing to pan, so the probe
   proved only that pan does not misfire, not that panning works. Chromium touch emulation is still
   not evidence for a pinch on glass.

78. **The figure recolour has no notion of "text sitting on a data fill", so it can *cause* the
    contrast failure it exists to prevent** (P3, filed 2026-07-27 while fixing item 77's fourth
    residual; item 41's family). `MPL_THEME_PREAMBLE`'s `_tali_recolour` sets **every** `Text` in a
    figure to the reader's foreground. That is right for titles, axis labels and ticks, which sit on
    the transparent page background — and wrong for an annotation drawn *inside* a data-coloured
    mark, whose background does not change with the theme. **Measured** on
    `corpus/tech-blog/posts/pca-geometry/`'s covariance heatmap: the `1.00` cells are near-black
    `#67000d`, so in the **light** render the annotation is recoloured to near-black `#1a1a1a` on
    near-black and is effectively illegible; the dark render is fine. The author cannot fix it in the
    document — an explicit `color=` on the annotation is exactly what the recolour overrides, which
    is what makes this a tool item and not a corpus one.
    **Not obvious how to fix, which is why it is filed rather than done.** Matplotlib does not mark
    which `Text` is "on" a mark, so candidates are all heuristics: skip a `Text` whose axes-fraction
    position lands inside a filled artist; skip `Text` parented to a `QuadMesh`/`AxesImage`; or pick
    per-annotation black/white from the *underlying* fill's luminance instead of the page
    foreground (what matplotlib's own `annotate` helpers do). **Do NOT "fix" it by dropping the
    recolour** — that reinstates the baked-foreground bug the preamble exists for.

41. **R graphics cannot follow the page theme; matplotlib figures can** (P3, M; detail:
    [2026-07-26-corpus-demand-probe-analyst.md](2026-07-26-corpus-demand-probe-analyst.md), AN-2b).
    Taliesin renders every inline matplotlib figure **twice** (light + dark foreground) and swaps them
    on the theme toggle (`kernel.rs`'s `MPL_THEME_PREAMBLE`); measured on `corpus/analyst/` the Python
    figure emits two genuinely different PNGs and the ggplot figure emits one, so a mixed-language
    report has half its figures track the reader's theme and half baked. **Blocked on being a feature,
    not a fix:** a real version re-renders the figure twice against two foregrounds. **Do NOT confuse
    this with AN-2a, which is fixed** — the R device no longer paints opaque white under a transparent
    figure; the *ink* is still baked at one colour, and that is what is left. The documented workaround
    (a neutral mid-grey palette) is the second instance of the convention named in item 18's F-02.
    Minor and separable: an R figure is emitted `<img alt="output">` where the Python pair is `alt=""`;
    both sit inside a captioned `<figure>`, so `alt=""` is right and `"output"` is noise read aloud.

18. **Demand-probe (interactive-explainer) residuals** (P3; detail:
    [2026-07-22-corpus-demand-probe-interactive-explainer.md](2026-07-22-corpus-demand-probe-interactive-explainer.md)):
    - **F-02 (gap, P3):** an authored numbered figure is emitted as `<img src="fig.svg">`, and an
      `<img>`-embedded SVG is style-isolated: it can't see `--tali-*` or the theme toggle, only the
      **OS** `prefers-color-scheme`. So a reader who forces the page theme opposite their OS gets the
      figure in the wrong palette. Inline `{js}`/SVG graphics on the same page track the toggle fine.
      Candidates: an inline-SVG figure path so `![](x.svg)` inherits page vars, or a documented
      neutral-palette convention. Edits `crates/core/src/render/figure.rs`.
    - **F-03 (WAI, authoring nuance):** a `{js}` "once" cell's returned node is mounted *after* the
      cell body runs, so an attachment-gated init (`if (!node.isConnected) return`) silently no-ops the
      first paint. Gate teardown on `invalidation`, not DOM attachment. Candidate: a doc line in the
      `{js}`-cell reference, or an optional post-mount hook.

70. **A project with no `_site.yml` declares no boundary** (P3, filed 2026-07-27 from the path-parity
    batch's "surfaced, not fixed"). `build <dir>` accepts a bare directory, so a single-document render
    of one of its pages roots at that page, and the site path's own inference can still widen to
    `.git`. Nothing can infer an undeclared boundary; the fix is for the author to declare one. Live
    instance: `corpus/posts/pca-geometry/` (the loose twin of the tech-blog page, byte-identical to it
    and pinned so by `twinned_corpus_sources_stay_byte_identical`) sits under no project marker, so
    `build` of it warns `include not resolved` — true since PT-2 shipped and **now uncovered by any
    test**, since the corpus pin moved to the tech-blog copy. Decide whether that warning is correct
    behaviour or wants a better message before writing code.

104. **Three Wave 1 items whose own round could not verify them, filed with the measurement each
     needs.** (Do not build until measured — each says so in its findings doc.)
     - **The `.gitattributes` line that makes `.tmd` behave like `.md`** on GitHub. Needs GitHub
       linguist-override behaviour confirmed; the round could not.
     - **The Jupyter on-ramp that already exists outside the project.** Needs `nbconvert` output
       confirmed to survive the rename.
     - **The scale ceiling**, measured with a **runtime-generated fixture that never enters the
       corpus walker** — deliberately shaped to respect the standing ban on growing `corpus/tarn`
       and on minting `corpus/longbook`, whose stated reason is that the walker renders every
       corpus doc on every `cargo test`.

105. **The headless `--no-sandbox` rationale rests on an assumption this round retired.** (LOW.)
     The justification assumed only author-written documents reach the headless path; item 79's
     family says otherwise. Re-derive the rationale before changing the flag.

10. **Two kernel limitations with no clean fix** (P3, dev-facing):
    - **R cold kernels still orphan on ungraceful parent death.** IRkernel has no `ParentPollerUnix`
      equivalent, so there is nothing to arm; PDEATHSIG is the only other lever and is hazardous. R is
      rarely the cold single-doc path, and the warm-pool, cold-Python and `/tmp`-sweep halves all
      landed. `kernel.rs`.
    - **A tens-of-MB cell output blocks ZMQ receive before the cap fires.** `kernel.rs`. (Not
      forbidden — the old "do-not-touch" note was the completed rewrite-scoping list, not a freeze.)

12. **i18n / Unicode: done bar a demand-driven residual.** The LSP UTF-16 fix shipped 2026-07-22
    (detail: [2026-07-22-i18n-unicode-sourcepos-audit.md](2026-07-22-i18n-unicode-sourcepos-audit.md)).
    *Residual, do not spin up without a real ask: RTL layout, CJK line-breaking, non-ASCII heading-slug
    collisions.*

### P5 — frozen, do not spin up

- **M6a `MAX_WARM_PAGES` / `exec_pool.rs` eviction:** the standing freeze; sign-off refused
  2026-07-17. Eviction drops the executor and kills its kernel child processes, so this is kernel
  lifecycle, not a constant. Do not tune without a new ruling.
- **M2's hanging-interpreter sibling** *(needs its own exec/kernel ruling)*: a *hanging* (not missing)
  interpreter costs ~161s recovery, downstream of the (bounded) `interp_id` probe in the warm-pool
  forkserver READY wait + kernel-start retries.
  `kernel::tests::transient_start_errors_retry_but_missing_interpreter_does_not` shows the *missing*
  case is handled and the *hanging* one is not. `kernel.rs`/`warm_pool.rs`. *(Aside, pre-existing +
  load-bearing: `crates/server/Cargo.toml` doesn't list tokio's `process` feature though
  `kernel.rs`/`warm_pool.rs`/`exec.rs` use it; it compiles only via feature unification.)*
- **M4 test stand-in flake:** the M4 test's `sleep 300` stand-in kernel survives ~2 of 8 full-suite
  runs, only when the build is cold. Measured, unexplained, argued test-only (a real kernel has three
  reclaim nets where the stand-in has one). Worth an hour only if a real kernel is ever seen outliving
  its pool.
- **D72 bare `@key`:** declined for now (the diagnostic already ships, so nothing renders wrong
  silently, which makes it a feature question not a defect). Edits `crates/core/src/cite/`, needs
  sign-off if revived.

## Tier 3 — demand-driven (below every band above; build only when a real user asks)

**Waits on demand, not on capacity.** The PMF audit's verdict is that what is missing is **real users,
not more features**, so nothing here is scheduled. One line each; the reasoning lives in the linked
audits.

- **An end-to-end live-HTTP test for `mounts:` serving.** The F-04 work unit-pins the pure
  `match_mount`/`resolve_project`/`classify_change` helpers and live mount serving is browser-verified;
  what is missing is only the bin-crate gap of a real `reqwest`/`TcpListener` harness. Mounts are
  preview-only, so this waits for a reason to exist.
- **Companion (Phase 2):** editor commands (insert block / reorder slide) — strictly `.tmd`-buffer text
  transforms in the editor, never preview gestures.
- **`.tmd` format-on-save for PROSE** (open question, and narrower than it was). The
  table-only formatter shipped 2026-07-28 (`crates/server/src/lsp_format.rs`,
  `textDocument/formatting`), and it sidesteps the recorded objection rather than answering
  it: a table's rows map one-to-one onto its lines, so the replacement has exactly the line
  count of the range it replaces and no `data-sourcepos` below it moves
  (`formatting_never_changes_the_line_count` pins that). **A prose pretty-printer still has
  the original problem** — reflowing paragraphs moves every line after them — so the
  brainstorm is still owed before any reflow work.
- **Dogfood: migrate the FL-weather book to Taliesin** — a real Quarto to Taliesin migration +
  portability stress test (exercises `book.rs`, includes, the freeze cache, file-mode portability). If
  it renders clean, consider pinning a reduced version under `corpus/`.
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic, kernel-free and
  network-free).
- **`taliesin publish` follow-ups:** an optional `--init` wrapper for the one-time `wrangler` setup.
- **Interactive/explorable numerics** (`FEATURE-IDEAS.md` #62-66; none pinned; promote one only with a
  corpus pin).
- **Wave 5** (`ROADMAP.md`): print-pdf track (paged render *of* the built HTML), docs-as-spec (RFC-2119
  dialect + protocol reference), `{glsl}` cell-language registry, SEO completeness (sitemap/robots/JSON-LD
  at publish with `url:`).
- **Site-level shared bibliography + hygiene** (M). `bibliography:` is per-document only, so a growing
  blog retypes keys per post and nothing reports an unused or duplicate entry. Allow `bibliography:` in
  `_site.yml` merged under each page's own, plus two **read-only** diagnostics ("entry never cited",
  "duplicate key"). Explicitly does not touch the BibTeX parser / CSL formatter.
- **Author structure panel** (M/L). A read-only preview sidebar: the heading tree with per-section word
  count and a badge per node for unresolved xref / TODO / over-goal length; click to scroll. This is the
  *revision* view, not the reader TOC. Scope it as an annotation layer on the dev panel, or it grows to L.
- **Session revision digest** (M). Surface the `BlockOp` stream the client already receives: a session
  word delta (`+340 / -180`) plus a feed of the last N ops, each click-to-source. Honest caveat: the pin
  is behavioural (a `tools/live-edit-bench` assertion), not a corpus doc.
- **Block-level transclusion** `{{< include file.tmd#sec-id >}}` (M). Reuse a section across a series
  without copy-paste drift. Must ride **on top of** the `includes.rs` source-map pass (resolve the
  fragment to a block range, hand the existing machinery a sub-slice), never rewrite it. Hard merge
  gate: the source map must not perturb.
- **LSP for the language intelligence, browser stays the view** (L). Everything an LSP needs is already
  in Rust (`check`, `vocab`, `register_xref`, the bib parser, `closest()`); it is write-once for
  Neovim/Helix/Zed/VS Code and removes the drift that causes the `#| label:` completion gap (JS regexes
  reimplementing Rust knowledge). An LSP cannot render the preview and does not need to.
- **Image optimization** (large): WebP/AVIF transcode + responsive `srcset` + lazy-load behind a
  content-hashed asset cache. Deferred until posts get image-heavy.
- **Marketing site** (deferred, feature-first; rolls into a demo-machine rebuild): `live-edit-hero-demo`
  clip; swap `site/_site.yml` placeholders; demo-led hero rebuild; mobile embed refine; deploy.
- **`serde_yaml` fallback watch-item:** the `Cargo.toml` workspace comment names `serde_yml`, which
  carries RUSTSEC-2025-0068 (unsound + unmaintained); `serde_norway` is 1+ yr stale. The maintained
  continuation is **`serde_yaml_ng`** (v0.10). No urgency (trusted local config; 0.9 still builds). If
  0.9 ever breaks against a future serde/edition, swap, gated on a test that `Error::location().line()`
  still works. Fix the stale comment when touched.
- **PMF demand-driven tail** ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md), Tier C): a
  document-level reader show/hide-code toggle, a reader code+data download affordance, instant
  client-side navigation polish. Each waits on a real ask.

## Audit lenses — closed, do not open a new round

[AUDITS.md](AUDITS.md) is the round index and a *record*, not a menu. The 14-round slate
([spec](../docs/superpowers/specs/2026-07-27-audit-slate-design.md)) is **complete except R12**,
real-device mobile on Android, which needs the author's phone. Its priority order is in the spec:
the book drawer scroll lock first (item 76 made the drawer a book's only nav surface), then the
`--host` QR flow, momentum scrolling and the dynamic viewport toolbar, tablet widths, TalkBack.
**Record explicitly that an Android round does not cover WebKit/iOS**, or it will later read as
full mobile coverage.

The slate's own thesis is the part worth carrying: every earlier lens asked *is this correct?*,
and asking instead whether the tool is **detectable**, **holds under stress**, would be **adopted**
and can be **handed over** produced three HIGH security findings in one pass, none of them a
correctness bug. Wave findings docs are linked from AUDITS.md. Durable artefacts, so a later round
does not rebuild them: the deck exemption register (R14), the sensitivity/tradeoff register (R6),
the D>=8 detection cluster (R7, now living in [DETECTION-DEBT.md](DETECTION-DEBT.md)), the draft
ACR (R9, now published in the guide) and the external-document shape inventory (R11, item 129).

**Two lenses remain un-run and both are blocked, not declined.** L3: `lsp.rs`, `complete.rs`,
`skim.rs` and `manifest.rs` post-date every lens that would have owned them, though the mutation
campaign has since pinned much of what one would look at. L6: a real external document, blocked on
a repository that is not on this machine.

**Never scope a round from the exemptions that are written down.** R14's premise was too generous
by an order of magnitude: the two documented `DocFormat::Reveal` exemptions turned out to be
*correct*, while the real hole was that a deck in a site never reaches the code those exemptions
live in. A dense do-not-touch cluster is not evidence of coverage; it is a reason to measure.

## Quarto catalog (policy, not a task)

**Owner ruling 2026-07-16: no sweep. Triage an area on demand, when you next work that area.** Before
consulting it read the triage doc's "three layers" section
([2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md)): the entries are the asset
and were well-grounded on 2026-07-03, but the heading status is degenerate and the executive summary is
misleading. A skeptic verdict is evidence, never a ruling (its "drop Atom feeds" verdict was overruled;
Atom shipped with autodiscovery).

## Do not re-add / re-scope

**One line per entry.** Detail lives in git, in [AUDITS.md](AUDITS.md), in the dated findings docs
and in [LESSONS.md](LESSONS.md) — look there rather than re-expanding this list. A batch's date and
branch are enough to find its commits.

### Shipped

- **2026-07-29 first-hour + positioning** (144, 151, 87, 88, 94, 95, 96, 135, 136): eight CLI /
  diagnostic / LSP residuals, the two lying ui-audit probes (suite now 7/7), the first-run
  execution notice, and `docs/guide/using/choosing.tmd`. **Three filed causes were false** — 151's
  "`id="TOC"` is in no emitter" (it is; the probe targeted a *book*, which by ruling has no rail),
  94's stale 8.59% (7.3% today), and 144c's scope (also an unfiled per-page repeat).
- **2026-07-28 block model + docs gate** (138, 146, 143's path half): every block has exactly one
  root element; prose is gated against the tree (dead source paths, retired front-matter keys,
  undocumented CLI flags) rather than against a needle list. **`notes/` and `docs/superpowers/` are
  excluded from that gate and must stay excluded** — they are dated records.
- **2026-07-28 deck harness** (112, 125, 113, 111): `deck.js` has a browser test; deck content is
  auditable at 0 violations across 100% of slides. It found **two shipped layout defects on its
  first run** (code blocks clipped on 5 of 21 slides; a focus ring around every vertical-stack
  slide), neither filed and neither visible to any emission test. The eleven deck shapes 113 listed
  stay deliberately unbuilt — the walker renders every corpus doc on every `cargo test`.
- **2026-07-28 honesty + build cost** (91, 110, 115, 119, 126, 134, 143): `chromiumoxide` is an
  opt-in `headless-js` feature, off by default; not linting `draft:` pages is **ruled correct** and
  the defect was the silence; `Block::sourcepos`'s empty-string contract is documented; the ACR is
  published; [DETECTION-DEBT.md](DETECTION-DEBT.md) is the live register.
- **2026-07-28 verified sweep** (85, 86, 97, 98, 99, 114, 123, 130): a `theme:` extension bundle is
  contained (**item 80's absolute-`Path::join` footgun in a second place**); no built page fetches
  off-origin; a shortcode source is a path, not a URL; both `jsconfig.json` include lists are
  globbed. 130 was already fixed and 99 was a clean measurement — both closed with no code.
- **2026-07-28 critique-round client/LSP/manifest** (139, 140, 141, 142): rename validates the new
  name and leaves an external URL's fragment alone; `toc_html` stopped double-escaping an explicit
  heading id (a dead link in the published build); the Cmd-K palette locks the background scroller;
  the web manifest stops shipping Taliesin's brand and stops pointing at a 404. **The splash colour
  is deliberately still one light value** — a manifest cannot express an OS-conditional colour.
- **2026-07-28 reader cost** (150's Phase B half, 137, 124): the body typeface ships as
  content-hashed files, not base64 in the render-blocking sheet (**125 KB gzipped off the critical
  path of every page**); the three conditional blobs are written only when something links them
  (94% cut on prose-only `corpus/tarn`); the Label-in-Name static rule.
- **2026-07-28 publication readiness** (84, 89, 90, 92, 93): `tools/gates.sh` (the one script that
  runs every gate and **refuses to be green when one skipped**), `CONTRIBUTING.md` with the inbound
  relicensing grant, `ci.yml` + `release.yml` **guarded inert until the repo is public**, the
  measured install expectation and platform matrix, and "Coming from Quarto".
- **2026-07-28 launch blockers** (79-82, 109, 117, 118, 120, 121, 127, 128): `mounts:` is contained;
  `check` does not spawn a project-supplied interpreter; `--no-exec` covers `{js}`; a deck in a site
  is validated; comma fences highlight; a migrated link gets a did-you-mean. **`--no-exec` is
  deliberately NOT a sanitizer** (2026-07-03 CSP ruling) — do not re-scope as "strip the HTML too".
- **2026-07-28 item 83 — the five pre-relicence MIT tags are deleted** (owner-approved; none had
  ever been pushed). All five commits stay reachable from `main`, so only the labels went. **The
  durable rule: never tag before the licence is settled**, and cut a release tag only from a tree
  whose `LICENSE` matches `Cargo.toml`.
- **2026-07-27 item 76 — a book has no right-rail TOC** (owner ruling, reversing 2026-07-06). The
  gate is `Site::page_toc`, ahead of the page's own `toc:`. **Do not re-scope as "give books their
  TOC back" or as "delete the rail everywhere"** — websites and single documents keep it. The
  drawer marks which section of the open chapter you are in, computed on each open (the drawer
  locks the root scroller, so a scrollspy would watch a dead event).
- **2026-07-27 item 77** (the 72-75 residuals): shortcode arguments linted against a closed
  vocabulary; `TAL-SHORTCODE` is its own WARNING family; `favicon:` resolves like `logo:`. **A book
  with neither title nor logo still emits no brand link, deliberately.** The fourth residual was
  refuted by measurement.
- **2026-07-27 mutation campaign** (58-69): every measured survivor in `crates/core`'s five
  post-07-18 files, the ten `crates/server` files and `lsp_nav.rs` is triaged and pinned. **Do not
  re-run it against the same scope.** Method in [LESSONS.md](LESSONS.md).
- **2026-07-27 item 66** (`404.html` links the shared `_assets/` bundle; its hrefs are root-absolute
  on purpose) and **item 67** (the `~/.local/bin/taliesin` launcher exits early for `__complete`
  only, 24.3 s -> 0.024 s per tab press; **`completions` is deliberately NOT exempt**).
- **2026-07-26 deck weight + headless bounding** (52, 55): a site deck went 4.6 MB -> 7 KB via a
  separate `deck.<hash>.{css,js}` pair. **A deck cannot link the page's `app.js`** — `search.js`
  would steal Cmd-K. The standalone artifact stays 4.4 MB and self-contained on purpose.
- **2026-07-26 path parity** (50, 51, 57): `render_single_doc` decides the single-document
  containment root once (nearest `_site.yml`, **never `.git`**); `TOC_SHEET_MARKUP` is the one copy
  all four assemblers emit. **Do not re-scope as "give the single-file build the inferred root"** —
  that is a revert of `9359a2c`.
- **2026-07-26, earlier**: migration UX (53, 54); the mobile batch (42-49 — the tree asks what
  device it is on via `hover`/`pointer`, it had none); owner rulings 24 (`data-section-end` shipped
  as option (b)), 17 (book breadcrumb ruled **no**) and 2 (deck presenter tools declined); reporting
  surfaces 39, 40; demand probe #4 (`corpus/analyst`); AP1-R1 (the freeze cache is byte-bounded) and
  DOCS-2/3/4/5.
- **2026-07-25 and earlier, closed:** AP7-1..5 (a11y), AP3-1, AP11-1 (`TAL-KERNEL`), DIAG-1, DOCS-1,
  AP3-3, PA-M3, PA-M13, PA-H1's residuals, the backlink-context + resume batch, book wayfinding, the
  hardening batch, book-level `theorems:`, live-executor mounts (F-04), book-aware `read`, AP8-1's
  output scrub, DET-1, the DX audit batch, `taliesin lsp`, DX17(a)+(b), the deck audit, the polish
  audit batch, the PMF builds, corpus coverage, the machine-facing audit, AI-native packaging, the
  R/Python ANSI leak, ungraceful-death reaping, and the `assets/js` `tsc` gate.

**Numbers retained, never reused** (each closed by a ruling or folded into another item, and kept
here so a later round does not re-derive them): **116** — the positional cascade vs a Python DAG,
CLOSED, do not build; reactivity is marimo's well-made claim while reproducibility is unclaimed by
anyone, so tell the cascade story instead. **132** and **133** — R8's value-stream pricing of 109
and of 127/128; a deck's defects are found by an *audience*, the latest and most expensive point in
the stream, and 447 of the 457 diagnostics a real external book produces are the tool's vocabulary
gap rather than the author's mistakes. **145** — retired into 137. **147** — retired into 101.
**151** and **152** — see P2 and the 2026-07-29 batch.

### Decided against

- **"Adjacent slides bleed into the deck's letterbox" (DT-5, filed and RETRACTED 2026-07-27, same
  day):** **false — the letterbox is empty.** `.tali-deck` is sized to the 16:9 stage
  (`min(100%, 100vh*16/9)`) with `overflow: hidden`, and its comment already says "adjacent cells
  fall outside and are clipped (no peek)". The probe intersected each neighbour with the
  **viewport** instead of with its **clipping ancestor**, and `getBoundingClientRect` knows nothing
  about `overflow: hidden` — re-measured, the neighbour contributes **0 px** inside the clip box and
  `elementFromPoint` returns `BODY` there. **Do not re-file it from a rect measurement**; if it ever
  looks true again, the only valid evidence is a rendered pixel, not a rectangle.
- **Deck presenter tools** (one-command publish, laser/spotlight, auto-advance): declined 2026-07-22 and
  **re-declined 2026-07-26** on the same grounds — no real speaker ask has appeared. Revive only when the
  author actually presents from Taliesin. (`footer:`/`logo:` from that item did ship.)
- **WS op-message batching** (declined 2026-07-25 **on measurement, premise confirmed**): the worst case
  is 55 ops in one frame, but a warm edit is 32.2 ms of which the diff is 0.94 ms, so batching saves
  ~220 bytes on a 32,303-byte payload (0.7%), none on the critical path. Reopen only if render cost drops
  far enough that framing is measurable.
- **Item 29's reduction residuals R1 + T2** (closed 2026-07-25 without code): R1's `text_content` /
  `indexable_text` fork is deliberate and equalizing them would leak raw entities into `llms.txt`; T2's
  "three modules pre-scan" is partly rotted — the real duplication is a six-line idiom in two places, and
  the divergence that looked like a latent bug is unreachable.
- **Deck-motion, whole item** (detail: [2026-07-24-deck-motion-audit.md](2026-07-24-deck-motion-audit.md)):
  Option A + residuals shipped; **(3) no-change** ruled; **(4) Option C (shared-element FLIP) declined —
  do not re-cost it a third time**. A coverage-weighted refinement of (5) measured *worse* (15 of 25
  slides vs 23 of 25); do not re-refine without measuring.
- **A separate per-page outline artifact for the book drawer** (declined 2026-07-25 while building it):
  the index it would duplicate is already lazy-loaded on every page, so a sidecar buys ~55 KB gzipped on
  one cached subresource in exchange for a second copy of the render recipe, assembly, invalidation,
  route and build write.
- **`drawer-typeahead`** (declined 2026-07-25): Cmd-K plus the drawer's collapsible outline covers it, and
  a second search-like box beside a Search button is a discoverability smell.
- **A "~N min read" label on a book chapter** (2026-07-25): `prose::word_count` excludes fenced code and
  math, so a code-heavy chapter is understated — and reading code is *slower* than prose, so the error
  goes into a promise about the reader's time in the wrong direction, on exactly the chapters this tool
  exists for. (The dated-post estimate in `render/mod.rs` is a different surface; `is_article` is
  test-pinned, do not touch it.)
- **Flipping a book chapter's label to prefer `title:` over its `# H1`** (resolved 2026-07-25): measured
  across every book in the repo, only 3 of 48 chapters differ and in 2 the `# H1` is the *better* nav
  label. Resolved as documentation, not code.
- **CAD-as-code** (`{openscad}` / CadQuery cell → live 3-D preview; researched 2026-07-23, NOT built):
  technically feasible and legally green, killed on **demand**. **Do not bundle openscad-wasm (GPL).**
  Five named revisit triggers in [2026-07-23-cad-as-code-research.md](2026-07-23-cad-as-code-research.md).
- **2026-07-22 rulings:** DX16 update-nudge = **skip** (a version check is network egress that undercuts
  the offline-first identity); cross-ref label i18n = **defer** (no corpus doc demands it); item 9's
  design questions documented as intentional (the deck serif/sans inversion, no `//| uses:` alias, the
  callout-namespaced / theorem-bare asymmetry).
- **2026-07-12 wishlist cut to `FEATURE-IDEAS.md`** (revive only when a corpus doc needs one):
  cross-revision diff, repro manifest, List-of-Figures/Tables/Theorems, interactive tables, line-level
  code xrefs, image `dark=`. Reader text-size/line-spacing controls declined (a11y-exempt substrate in
  `14-reader-prefs.js`).
- **TODO / FIXME surfacing** (owner ruled 2026-07-10): no `level` concept exists, so a TODO warning would
  fail `check` on every draft. If revived, a preview-only `Diagnostic::info` beats re-plumbing a real
  `level`, and the scan must NOT reuse `prose::strip_inline` (it blanks code, where TODOs live).
- **AI-native leftovers declined 2026-07-16:** `check --online` citation resolution (the only proposed
  network egress; check-only and off by default if ever revived), the numeric-claim-without-citation hint
  (its own spec rates it FP-prone), and a per-page text/JSON sidecar (redundant).
- **Refuted by measurement (do NOT re-scope):** heading demotion **already ships** (AP9's "12 `<h1>`"
  measured a stale gitignored build artifact; the only multi-`<h1>` corpus docs are decks, exempt by
  design); `build` does not leak forkserver subtrees; the warm pool booting Python on prose-only builds
  is hygiene, not latency; dev attributes are 0.29% of page bytes (don't strip); a `--version -dirty`
  marker is stale-by-construction; the `assets/css` stale-embed claim did not reproduce (re-verify for
  `assets/js` before any touch-render workaround); the 390px `hero:` overflow + theme/video desync are
  fixed; include symlink-loop SIGABRT does not exist (Linux caps at `MAXSYMLINKS=40`); **decks pass path
  parity outright** and `mounts:` differs from direct serving by 4 bytes (boot nonce + ws path).
- **`_redirects`/`_headers` preserved, never generated** (`build.rs` treats them as author-placed deploy
  metadata; `stale_sweep.rs` pins it).
- **Gate the gate:** a drift test that cannot fail is worse than none. Any new drift gate must be
  mutation-checked against exactly the shape it guards.
- **Library outsourcing decided against** (each verified vs the invariants): hayagriva/biblatex, schemars,
  jsonschema, morphdom/idiomorph, similar/dissimilar, clap, owo-colors, slug, html-escape,
  lightningcss/palette, IntersectionObserver/scrollspy libs, deck micro-helpers. Keep `two_face` extras
  filling gaps only (the bundled syntect set is consulted first and must win).
- **Reading-first defaults, research-validated keeps** (do NOT "fix"): serif body for long-form screen
  reading; ~70ch measure `--tali-maxw: 46rem`; right-rail scrollspy + width-gated sidenotes; scroll (not
  pagination) book reading; ship REAL bold/italic faces, never synthesized.
- **2026-07-06 decisions:** book pager stays bottom-only; book page-TOC fix-in-place, keep both nav
  surfaces; xref graph tool removed; focus mode stays ephemeral; deck overview keeps per-slide
  backgrounds; dev-menu + `#tali-progress` + reading-progress bar stay three separate signals.
- **2026-07-18 PMF re-derivations:** the reader "Cite this" box (D70) was REVIVED and shipped as B1; the
  deck desktop "async handout" reading view stays CUT (do not re-open without a fresh ruling).

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Per the PMF audit (2026-07-18) the tool is
feature-complete for ~one real user, so the highest-leverage next move is **real users**, not more
features. When publishing, lead the copy with the **speed moat** (warm server, block-level incremental,
no per-edit rebuild), the single most-repeated Quarto grievance and the most under-marketed asset.
