# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git, [AUDITS.md](AUDITS.md) and
> [ROADMAP.md](ROADMAP.md); **delete an item when it lands** — never a `[x]`, never a strikethrough.
> Method lessons go to [LESSONS.md](LESSONS.md), detection gaps to
> [DETECTION-DEBT.md](DETECTION-DEBT.md), and everything that must not be rebuilt, re-filed or
> re-scoped goes to [DO-NOT-REBUILD.md](DO-NOT-REBUILD.md).
>
> **Pruned to the release critical path on 2026-08-07** (owner instruction), the fourth cut-back and
> the first for a reason other than rot: the previous three (1,767 lines on 2026-07-29; 1,298 on
> 2026-08-01; 966 on 2026-08-02) each trimmed a file that had grown a changelog inside itself. This
> one **deleted ~38 open items outright** — every defect, feature, ruling and audit lens that does
> not block publishing. They are in git (`git show 99d781a2:notes/backlog.md`) and are deliberately
> not indexed anywhere: **a dropped item comes back only when it actually bites, and it is a higher
> priority then because it bit.** Do not re-file one from a grep or an audit; wait for the bite.
>
> The corollary is a hard rule for this file's next few months: **nothing new gets filed here unless
> it blocks the release.** File it nowhere, or fix it in the branch you found it in.

## Start here

> **⚠ THE DEFECT QUEUE IS NOT IN THIS FILE. It is
> [2026-08-13-mvp-audit-backlog.md](2026-08-13-mvp-audit-backlog.md)**, the post-cut audit's 29
> confirmed defects plus 18 shipping-surface items, in 12 branchable batches, each carrying its own
> reproduction command and done-condition. **Batch 1 is data loss** (`build --out <dir>` deletes
> files in the output directory and exits 0) and should be taken before anything else in either file.
> That file also records what NOT to do: the audit found the scope itself sound and the document
> vocabulary fully witnessed, so **do not cut another feature**.
>
> **This file stays the release critical path.** The two sequences are independent; the audit queue
> is about correctness, this one is about shipping.

**Everything below this line predates the 2026-08-08 scope reduction and is stale in places** — its
"Standing constraints" section names `taliesin features` (cut in wave 2), "four gates" (there are
eleven), "FIVE drift gates; a RETIRED one trips EIGHT" (`CLAUDE.md` now says four and one), and owes a
four-projection sweep to `taliesin read`, `skim.rs` and `llms-full.txt`, all three cut. Filed as S18
in the audit queue. Trust `CLAUDE.md` over this file on any of those.

**The whole file is now one sequence: ship the thing.** The five items below are ordered, and the
order is the plan, not a ranking. 103 → 100 → 148 → 149 → 170.

**Pre-flight is DISCHARGED for the current tree, and it re-arms on every merge.**
`./tools/gates.sh` ran green on 2026-08-07 (all 9 gates, twice: once at `87af6aa6` as a baseline and
once on the finished `item-210-nested-cell-execution` branch). All 8 CI jobs still arm on the first
push after the flip, against whatever tree exists then, so a red gate discovered *after* the repo is
public is discovered by an audience: **re-run it immediately before Phase 2 of item 100**, not once
and for all.

**Release readiness, re-measured 2026-08-05.** Green: `git-filter-repo` is installed, the history
rewrite is rehearsed end to end, the tree is clean and equal to `origin/main`, the repo has zero
forks, and README's D9-3 claim is already softened to "no prebuilt binaries yet". Not green:
everything in the sequence above.

**The author's feature-first policy is discharged, and 170 is no longer last.** It said "finish
framework features before marketing-site work"; the framework features it deferred to were all
dropped in this prune, so the marketing site is now simply the last step of the release rather than
a thing waiting its turn.

- **Ask git, never this file, for git state.** No SHA, branch name or commit count is recorded here
  on purpose: the author and parallel sessions both push, and a recorded SHA is the line that rots
  first.

  ```sh
  git log --oneline origin/main..HEAD   # what is unpushed
  git branch -vv                        # what branches still exist
  ```

- **Entries rot: trust an item's *symptom*, never its cause, line number or cost.** Grep the named
  symbol in source before pricing the work. This has cost real time repeatedly: item 182 was filed
  as "Taliesin has both link shapes and zero hover machinery (grepped)" while `site/hover.rs` plus
  `code-enhance/12-link-preview.js` had shipped exactly that feature three weeks earlier — it was
  deleted, not built. Three filed causes were false in the three batches before it.
- **Two measurement hazards, both of which have cost time.** (1) `target/release/taliesin` is shared
  across sessions and may be built from another branch — check `taliesin --version` against your own
  HEAD before trusting any CLI number. (2) A table-shaped probe whose every cell is negative is a
  **broken probe** until proven otherwise; carry a known-positive row.
- **`taliesin features <dir>` exists, so do not re-derive an adoption table by grep.** It reads the
  validator consts, not `vocab.rs` (which is the *offered-completions* subset and would report a
  live feature as unused), and it prints zero rows on purpose.
- **Nothing is owed by the author except item 103.**

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
- **Working method:** branch per item; brainstorm if there's a fork; a design note under
  `notes/` if one is worth keeping; implement TDD; verify (cargo + browser via chrome-devtools, or the
  extension harnesses); fast-forward merge locally; **delete the item here when it lands.** Push to
  `origin/main` only when the author asks. **Review subagents get a git worktree or you commit
  first** (a "read-only" reviewer with `Bash` still writes scratch files to your CWD; one ran
  `cat > Cargo.toml` in the repo root and destroyed the workspace manifest).
- **Tests: four gates, or the suite silently under-tests itself.** Run `./tools/gates.sh`, which arms
  `TALIESIN_REQUIRE_KERNEL` / `_R` / `_NODE` / `_CHROME`, asserts each canary printed `... ok` and
  refuses to be green when one skipped. It needs `TALIESIN_PYTHON=$HOME/.local/share/qmd-venv/bin/python`
  or it declines to start (exit **2**; a failed gate is exit **1**). Run the workspace suite
  `-- --test-threads=1` as it does: several tests own process-global state (`CHROME_PATH`), so at
  full parallelism a browser test fails in a way that reads exactly like a regression. `cargo test`
  aborts the remaining binaries at the first failure, so re-run before trusting a total.
- **Derive, don't declare.** Every proposed front-matter key must first answer *what on the page
  already implies this?* A key is the highest fixed cost per feature anywhere in the tool, so the bar
  is that the value is genuinely underivable, not merely convenient to state. Proven precedents:
  `citation_arxiv_id` from the `links:` host, affiliation numbers from first appearance, a dataset's
  size and digest from the file itself, and `doi:` as the counter-example that earns a key.
  **Underivable is not the same as belonging in front matter**: `datasets:` passed the derive test
  and was still retired, because an annotation that describes one invocation belongs *on* that
  invocation.
- **A new front-matter key trips FIVE drift gates; a RETIRED one trips EIGHT.** `CLAUDE.md` names all
  eight and is the current count (this file said SEVEN until 2026-08-07, having missed
  `editor/vscode/schema/tali-site.schema.json`, a bundled copy gated only by the companion's own
  `node --test`). **Two of the eight live outside `taliesin-core`**, so `cargo test --workspace` can
  be green while both are stale; only `./tools/gates.sh` catches them. Four of the five bless with
  `TALIESIN_BLESS=1`; the guide one wants prose. Five gates *come back* when a key is removed. That
  cost is the standing argument for "derive, don't declare" above.
- **Any new generated block owes the four-projection sweep** — `taliesin read`, `skim.rs`, the search
  index and `llms-full.txt` — or its text leaks into the search index. Four projections in three
  modules; two known leaks were found only by building a real site and grepping the artefacts.
- **A new `data-*` attribute or `--tali-*` token in browser code trips a census test**
  (`token_contract.rs`): expected, one sorted line to fix, and it is also the prompt to namespace the
  attribute. An invented `--tali-*` name renders **nothing** (the browser drops the whole
  declaration), which is why the census exists.
- **LSP/editor ranges are UTF-16.** A non-ASCII character earlier on the same line shifts every byte
  offset after it and the edit lands in the wrong column.
- **A red `exec`/`kernel` probe is real signal, not a coin flip.** The flake was fixed 2026-07-25 (a
  port race in `prepare_connection`; the re-roll lives on `Kernel::start_with_retry`, and
  `crates/server/tests/kernel_start_is_retried.rs` fails if any caller reaches the un-retried
  primitive). Verified 0 failures in 45 post-fix runs under the same load.
- **`corpus/tarn` is the fixture for scale-sensitive work** (12 numbered chapters, 3 parts + a nested
  part) and deliberately carries the shapes the rest of the corpus lacks. **Use it instead of minting
  a fixture.** It is a *documentation* book, not a scale fixture: do NOT grow it toward 200 pages and
  do NOT mint `corpus/longbook` (the walker renders every corpus doc on every `cargo test`).
- **Execution pins do not belong in `corpus/`.** The walker renders every corpus doc on every
  `cargo test` but does **not** execute cells, so a corpus pin for execution behavior pays the render
  cost and exercises nothing. Put them in `crates/server/tests/` against a temp-dir fixture, as
  `executed_output_reproducible.rs` and `progress_bar_collapses.rs` do.
- **Verify a fix by mutation** (restore the bug, watch the *named* test fail), not by a green suite.
  **The full trap catalogue is [LESSONS.md](LESSONS.md); read it before writing a probe or a pin.**

## Open items

**Item numbers are stable**: never renumbered, and a closed item's number is never reused. Numbers
absent from this file are closed, dropped or retired — [DO-NOT-REBUILD.md](DO-NOT-REBUILD.md) covers
the ones whose closure has a guard attached.

**Standing rule for an item:** branch per item, verify each fix by *mutation*, browser-verify
anything client-side, and **delete the item from this file when it lands.**

### The release — in sequence

103. **Clear the name in software classes.** (Ruling, legal not code. Gates everything below it.)
     Trademark search in the relevant classes. What is already known and is *not* a blocker on its
     own: TALIESIN is a live registered mark of the Frank Lloyd Wright Foundation (Reg. 4150375),
     software is outside the recited goods so legal risk is low, and the real cost is permanent SEO
     invisibility (`github.com/taliesin` and `/taliesins` are both taken). **Renaming twice is worse
     than a bad search name** — if the answer is keep, always publish as "Taliesin — the `.tmd` dev
     server" so the disambiguator travels.

100. **The public flip: RULED 2026-07-28 — "archive plus fresh public", and it is specced.** The
     design spec was deleted with `docs/superpowers/` on 2026-08-09 (R6-1) and lives in git
     history; the findings it produced are in
     [2026-07-28-public-flip-audit.md](2026-07-28-public-flip-audit.md).
     The ruling threads the needle both earlier routes missed: **the history IS published** (the
     single-author commit record is the evidence a grant applicant wants), and `git rm` in a new
     commit leaves a file in every commit that ever held it. Mechanism: relocate the purged docs to
     `~/Documents/personal/taliesin-private/`, rewrite history, rename this remote to
     `taliesin-private-archive` (stays private, complete backup), create a **new public**
     `AJBogo9/taliesin` and push the rewritten history there. No force-push, no destructive remote
     op, and the private blobs never reach the public repo at all. Zero forks and never having been
     public is what makes it cheap.
     **Kept, not purged:** security audits, `.claude/`, `LESSONS.md` — for the stated goal those
     are the exhibit. (`AGENTS.md` and `docs/superpowers/` were on this list and have since been
     deleted from `HEAD` by the cut campaign — wave 2 and R6-1. That does not reopen the ruling:
     "kept" meant *not rewritten out of history*, and the history is what gets published.)
     **Purged:** money and strategy
     documents only (`notes/STARTUP-PLAN.md`, `notes/FUNDING-RESEARCH.md` — both git-**tracked**
     while their own headers say they must not be), plus ~11 commit subjects that name them.
     **Execution status: NOT STARTED, and not to be started without a separate instruction.** Phase 1
     is a read-only audit and is safe whenever wanted; **Phase 2 is irreversible** and is
     additionally gated on Phase 1's findings being signed off *and* on a green `./tools/gates.sh`.
     What still lands on this item:
     - The spec's own D-checks, including the provenance check on corpus documents.
     - **Whether tags travel to the new public repo** (five local MIT tags were deleted on
       2026-07-28; none had ever been pushed).
     - **Whether to prune `notes/`.** Half-answered: `docs/superpowers/` was deleted on 2026-08-09
       (R6-1, 97 files / 35,585 lines / 2.8 MB), which took the worst of it — the 1,129,527-byte
       `2026-07-03-quarto-design-decisions-catalog.md`, adversarial self-critique sitting under
       `docs/`, where a visitor reads it as "the manual". `notes/` is not under `docs/` and has no
       such misreading, so its 97 tracked files are a separate call. The remediation plan files it
       as tier 2, needing an explicit ruling.
     - **A procedure collision to fix in the same change:** `***REMOVED***
       (fresh repo), while this file and `2026-07-17-security-release-audit.md:217-218` sequence the
       `oss-*` items to "whenever the repo actually flips public". Fix the losing document or the
       next session follows it.
     - **Phase 1 RAN 2026-08-03** — findings in
       [2026-07-28-public-flip-audit.md](2026-07-28-public-flip-audit.md), 61 findings over the ten
       dimensions. **D2's verdict is `--replace-text` across all history, not a link-repair commit**:
       seven restatements of the purged docs' commercial conclusions exist ONLY in history, where a
       commit on top cannot reach them. Two of the seven sit at paths already absent from `HEAD`
       (`todo.md`, `2026-07-02-tmd-editor-grammar-plan.md`) and are cheaper to add to
       `--invert-paths` than to string-match. D4 (secrets) and D8 (tone) came back **empty**, with
       what was searched enumerated so an empty dimension is distinguishable from an unrun one.
       The reversible half was applied the same day; what is left is the irreversible half only.
     - **`git grep -Il "/home/bogo"` → 14 files** (re-measured 2026-08-09 after R6-1; it was 21 on
       2026-08-05 and 11 on 2026-07-28). All 14 are now in `notes/`, the `docs/superpowers/plans/`
       half having gone with the archive. Low impact (the username is public via git author
       metadata) but it is
       the failure mode `LESSONS.md` warns about.
     - **Phase 2's tooling prerequisite is discharged:** `git-filter-repo` 2.47.0 is installed at
       `~/.local/bin/git-filter-repo` (verified 2026-08-05). The audit's prerequisite table and the
       Phase 2 inputs README both still say "not installed", and the inputs README **contradicts
       itself** (its own rehearsal section records the full rewrite running under 2.47.0). Trust the
       binary, not either note. *(Item **25**, the pre-public flip procedure, is folded into this
       item.)*

148. **Cut a tag, then verify the release assets exist.** (Flip-gated, and the ordering is measured.)
     `.github/workflows/release.yml` builds Linux x86-64, macOS arm64 and macOS x86-64 on a `v*` tag
     and attaches a tarball + `.sha256` with `LICENSE` + `THIRD_PARTY.md` inside; the README states
     the matrix (Windows explicitly unsupported). **No tag has ever been cut**, and `README.md:59-66`
     marks three platforms "built and released" against **zero** releases (`git tag -l 'v*'` is empty
     locally and on the remote). Owner ruled 2026-08-03 that the audience is **strangers evaluating a
     product**, so this is a launch blocker rather than cosmetic, and the resolution is
     flip-then-tag, not softening the table.
     - **ORDERING CONSTRAINT, measured 2026-08-03: the flip must come BEFORE the tag.**
       `release.yml` triggers solely on `tags: ["v*"]` **and both its jobs are guarded on
       `github.event.repository.private != true`** (`:27`, `:41`), so tagging while private builds
       nothing and produces no release — silently. Sequence: flip public → push a `v*` tag → verify
       the release assets exist → only then is the README true.
     - **Never tag before the licence is settled**, and cut a release tag only from a tree whose
       `LICENSE` matches `Cargo.toml`.
     - **Package managers are a separate decision, after the tag.** crates.io `taliesin` /
       `taliesin-core` / `taliesin-server` are all 404 (all three names free); no Homebrew, Nix or
       install script. `cargo publish` will reject this workspace as-is: `Cargo.toml:14` declares
       `taliesin-core = { path = "crates/core" }` with **no `version`**, and `keywords`,
       `categories`, `readme`, `homepage`, `documentation` are blank in every manifest. The `.crate`
       size blocker is discharged — the vendored pyodide payload that caused it was deleted outright
       on 2026-08-04, so the crate is under the cap on its own bytes.
     - Cold build is **2m11s, 268 crates, 2.6 GB peak RSS at `-j4`** for one ~38 MB binary. The
       audience for a documentation tool is not the population that will install a Rust toolchain and
       wait it out, which is exactly why the release workflow exists.

149. **Launch presentation.** (All gated on the flip; each is small once it is.)
     - **`homepageUrl` is empty** although `taliesin.sh` is bought and already set as `url:` in
       `site/_site.yml`. Re-measured 2026-08-05 via `gh repo view`: the description is a real
       one-line description and there are **6 topics**, and the four screencasts **do** appear on
       pages a visitor sees — both halves of the filed "dead first impression" were rot. Still true:
       empty `homepageUrl`, zero releases (= 148), and the README's only image is the licence badge.
       The screencasts are MP4, so putting one in the README needs a GIF conversion or an uploaded
       asset URL, not a one-line embed.
     - **Anything quoting the speed ratio reads `RESULTS.md`'s "why the ratio is 9x and not 83x"
       section first.** The README led with a **wrong** pair until 2026-08-05 (3.2 KB against a
       270 KB page — 3.2 KB is the other 54 ops, **not** the patch, overstating the shrink by ~10x).
       The gated numbers are: payload **32,303 bytes vs a 291,691-byte page, 9x smaller**, 53
       `SetMeta` / 1 `Update`, cold **135,010.4 µs** vs warm **13,403.9 µs**. One fenced div carries
       90% of the payload on its own, and the honest headline is the op shape, not the ratio.
     - **Still absent: a code of conduct and GitHub issue templates**, both only worth doing once the
       repo is public. (`CONTRIBUTING.md` with the inbound relicensing grant and the platform matrix
       both shipped 2026-07-28.)

170. **Marketing site + a deploy mechanism.** (Last, and it is a build not a switch.) The
     `live-edit-hero-demo` clip (= `ROADMAP.md` Wave 2's unshipped deliverable), a demo-led hero
     rebuild, mobile embed refinement, and deploy. **"Swapping the `site/_site.yml` placeholders" is
     rot** (2026-08-05): `url:` is already `https://taliesin.sh`, and placeholder/TBD/lorem greps
     across `site/` return zero. **There is no deploy mechanism at all** to flip on:
     `.github/workflows/` holds only `ci.yml` and `release.yml`. `site/build.sh` builds all 8
     projects into one tree and **the ordering is load-bearing** — the parent build's `sweep_stale`
     deletes anything under the output dir it did not write, so a mount built first is silently swept
     away. Overlaps 149; do not build the same thing twice from both entries. Browser-verify at the
     three project viewports (390x844, 1440x900, **900x1440** — the forgotten portrait band is where
     layout defects show).

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Per the PMF audit (2026-07-18) the tool is
feature-complete for ~one real user, so the highest-leverage next move is **real users**, not more
features — which is the whole argument behind the 2026-08-07 prune above. When publishing, lead the
copy with the **speed moat** (warm server, block-level incremental, no per-edit rebuild), the single
most-repeated Quarto grievance and the most under-marketed asset.
