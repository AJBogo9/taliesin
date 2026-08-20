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
"Standing constraints" section was corrected on 2026-08-13 (it had named `taliesin features`,
"four gates", "FIVE drift gates / EIGHT for a retired key", and a four-projection sweep, all of
them cut or superseded). **`CLAUDE.md` remains the authority on any count**; take a gate count
from `./tools/gates.sh`'s own verdict line and never from prose here.

**Taliesin 1.0.0 was published on 2026-08-20.** The repository is public, the history is
published, the tag is cut and the release assets are verified. What remains is item 100's
single un-runnable step (deleting `taliesin-old`, which needs an interactive `gh auth
refresh`), then 149's README image and 170's marketing site. Items 103 and 148 are closed.

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
- **There is no adoption-table instrument any more.** `taliesin features` was cut in wave 2, so
  answer "what does the tool support" from the **validator consts** directly, never from
  `vocab.rs` (which is the *offered-completions* subset and under-reports: it offers 5 of the 12
  `XREF_LABELS`).
- **Nothing is owed by the author except the Phase 2 go-ahead.** Item 103 was ruled and closed
  on 2026-08-20, and every other prerequisite is discharged and recorded in the go/no-go dossier.

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
- **Run `./tools/gates.sh`, or the suite silently under-tests itself.** It arms
  `TALIESIN_REQUIRE_KERNEL` and `TALIESIN_REQUIRE_NODE` (the `_R` and `_CHROME` runtimes went with
  the `{r}` cell language and the headless-Chrome driver in wave 6), asserts each canary printed
  `... ok`, and refuses to be green when one skipped. **Take the gate count from its own verdict
  line**, never from prose. It needs `TALIESIN_PYTHON="$PWD/.venv/bin/python"` or it declines to
  start (exit **2**; a failed gate is exit **1**). `cargo test` aborts the remaining binaries at the
  first failure, so use `--no-fail-fast` before trusting a total.
- **Derive, don't declare.** Every proposed front-matter key must first answer *what on the page
  already implies this?* A key is the highest fixed cost per feature anywhere in the tool, so the bar
  is that the value is genuinely underivable, not merely convenient to state. Proven precedents:
  `citation_arxiv_id` from the `links:` host, affiliation numbers from first appearance, a dataset's
  size and digest from the file itself, and `doi:` as the counter-example that earns a key.
  **Underivable is not the same as belonging in front matter**: `datasets:` passed the derive test
  and was still retired, because an annotation that describes one invocation belongs *on* that
  invocation.
- **A new front-matter key trips FOUR drift gates; a RETIREMENT costs ONE line.** `CLAUDE.md` names
  them and is the current count. **One of the four lives outside `taliesin-core`**
  (`editor/vscode/schema/tali-site.schema.json`, a bundled copy gated only by the companion's own
  `node --test`), so `cargo test --workspace` can be green while it is stale; only
  `./tools/gates.sh` catches it. Retiring is now the cheap direction: add the `RETIRED_KEYS` entry
  and stop — **do not write a tombstone test**, the register derives it. What the register cannot
  derive is the *parser* still reading the key, which is the other half of a retirement and wants a
  parser-side pin. That asymmetry is the standing argument for "derive, don't declare" above.
- **Any new generated block owes the search-index sweep** or its text leaks into Cmd-K results.
  This was a *four*-projection sweep until `taliesin read`, `skim.rs` and `llms-full.txt` were all
  cut; the search index is the one that is left. Two known leaks were found only by building a real
  site and grepping the artefacts, so grep the built output, not the source.
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

> **⚠ THE FLIP IS DONE, 2026-08-20.** `AJBogo9/taliesin` is public, `v1.0.0` is released.
> The dossier that drove it is [2026-08-20-flip-go-no-go.md](2026-08-20-flip-go-no-go.md),
> kept as the record of what was verified and what was not. Supporting records:
> [design](2026-08-20-publication-prep-design.md),
> [plan](2026-08-20-publication-prep-plan.md),
> [workflow rehearsal](2026-08-20-workflow-rehearsal-log.md),
> [rewrite dry run](2026-08-20-rewrite-dry-run.md),
> [final verification](2026-08-20-final-verification.md).
>
> **The operational files stay OUT of this repository**, at
> `~/Documents/personal/taliesin-private/`: both `--path` lists, both `replace-text` files
> (each passed to `--replace-text` AND `--replace-message`), the ruling ledger and the
> un-rewritten bundle. **Do not sort the replace-text files** and do not re-add them here;
> `.githooks/pre-push` refuses that class of file.
>
> **The lesson that nearly cost the most:** the redaction list was wrong three times. Twice
> it was a wrong ENTRY, caught by a content sweep. The third time it was a wrong SURFACE,
> `--replace-text` never touching commit messages, and only enumerating the surfaces found
> it. Blob contents, commit and tag messages, path names, ref names, author identity.
>
> **Item 103 is CLOSED**, ruled 2026-08-20: keep the name, accept the SEO cost, and always
> publish as "Taliesin, the `.tmd` dev server" so the disambiguator travels.

100. **The public flip: DONE 2026-08-20, except one step only the author can run.**
     `AJBogo9/taliesin` is public at 2,122 commits with the full single-author history.
     `AJBogo9/taliesin-private-archive` holds the complete record minus the third-party
     material (D-8). Both were verified by cloning them back from GitHub: all 16 purged
     paths at 0 commits, and every redaction key at 0 across BOTH surfaces, objects and
     commit messages. One author, `321b658d` intact.
     - **THE ONE THING LEFT, and it needs your hands.** `AJBogo9/taliesin-old` (the
       original, still private) has NOT been deleted: the `gh` token lacks the
       `delete_repo` scope and refreshing it is an interactive OAuth flow. It still holds
       the un-rewritten history, which means **the co-author's joint work and the
       university's assignment brief are still on GitHub's servers**, which is exactly
       what ruling D-8 forbids. Not a public exposure (the repo is private), but it is a
       rights question, so do it soon:
       ```sh
       gh auth refresh -h github.com -s delete_repo
       gh repo delete AJBogo9/taliesin-old --yes
       ```
       Safe to do: the 36 MB un-rewritten bundle at
       `~/Documents/personal/taliesin-private/taliesin-full-2026-08-20.bundle` was written
       and verified (cloned back, identical tree hash) before anything was renamed, and the
       archive repo carries 2,168 commits. Delete this item when that command returns.
     - **Working-copy hazard, already defused, worth knowing.** After the rename the old
       `origin` URL resolved to the NEW PUBLIC repo while the local tree still held the
       un-rewritten history, so a force-push would have published the purge set.
       `~/Documents/personal/taliesin`'s `origin` now points at the private archive
       instead. **That directory is the pre-publication artifact, not the public repo.**
       A clone of the public repo is at `~/Documents/personal/taliesin-public`; swap the
       two directories when convenient.

149. **Launch presentation.** (The flip discharged most of it.) `homepageUrl` is set to
     `https://taliesin.sh`, the description carries the disambiguator, `CODE_OF_CONDUCT.md`
     and the issue templates shipped, and **v1.0.0 is released with all six assets**
     (three targets, `.tar.gz` plus `.sha256`, tag-derived names correct). The Linux
     binary was downloaded, checksum-verified, executed and used to render a document, so
     the README's install section is now true and tested rather than asserted. **What is
     left is the README's only-image-is-a-badge problem**: the four screencasts are MP4 and
     need a GIF conversion or an uploaded asset URL.
     - ~~**`homepageUrl` is empty**~~ although `taliesin.sh` is bought and already set as `url:` in
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
