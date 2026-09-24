# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: `CLAUDE.md`** ("done" = the docs
under `corpus/` render correctly; a feature witness belongs in `crates/core/src/render/tests.rs`).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git, [AUDITS.md](AUDITS.md) and
> [ROADMAP.md](ROADMAP.md); **delete an item when it lands** — never a `[x]`, never a strikethrough.
> Method lessons and detection gaps go to [LESSONS.md](LESSONS.md), and everything that must
> not be rebuilt, re-filed or re-scoped goes to [DO-NOT-REBUILD.md](DO-NOT-REBUILD.md).
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

> **The defect queue is not in this file. It is
> [2026-09-24-execution-verified-audit.md](2026-09-24-execution-verified-audit.md)**, whose
> "Suggested sequence" orders it. The 2026-08-13 queue it replaces is superseded, not verified
> empty: its register decision was mooted when the registers were cut on 2026-08-17, the flip it
> deferred happened on 2026-08-20, and its "do not cut another feature" line was overridden by
> the cuts that followed. An item from it comes back only if it bites.
>
> **The 2026-09-01 defect queue landed the same day it was filed:
> [2026-09-01-product-audit-backlog.md](2026-09-01-product-audit-backlog.md)**, the
> whole-product audit's 13 confirmed critical/moderate items (3 critical), all
> implemented, adversarially reviewed (14 review findings folded in, including a HIGH
> sibling of the diff-ordering defect) and gate-verified on 2026-09-01. The file now
> holds the landing record, the residuals, the two recorded author decisions (D1/D2) and
> the refuted register. T1's fix reverted the README pin to v1.0.1; cutting a v1.1.0
> release remains the author's call, and a new pre-push + gates check pins the README
> VERSION to an existing tag either way.

**Everything below this line predates the 2026-08-08 scope reduction and is stale in places** — its
"Standing constraints" section was corrected on 2026-08-13 (it had named `taliesin features`,
"four gates", "FIVE drift gates / EIGHT for a retired key", and a four-projection sweep, all of
them cut or superseded). **`CLAUDE.md` remains the authority on any count**; take a gate count
from `./tools/gates.sh`'s own verdict line and never from prose here.

**Taliesin 1.0.0 was published on 2026-08-20.** The repository is public, the history is
published, and CI runs on every push to `main`. What remains below is item 100's last check and
149's README image. The marketing site deploys with `tools/publish.sh` (four Cloudflare Pages
projects), and its unshipped hero clip is `live-edit-hero-demo` in [ROADMAP.md](ROADMAP.md).

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
  `vocab.rs`, which is the *offered-completions* subset (today it offers all 5 `XREF_LABELS`,
  but nothing requires it to).

## Standing constraints (read before working)

- **Do-NOT-touch (one freeze):** `MAX_WARM_PAGES` + the deterministic LRU eviction in
  `serve_site/exec_pool.rs` (M6a, sign-off refused 2026-07-17) and the **single-editing-surface**
  invariant (the preview is read-only; it must never write back to source). The rest of the
  exec/kernel zone is not frozen.
- **Website / brand** (2026-07-11 audit, detail:
  [2026-07-11-website-design-audit.md](2026-07-11-website-design-audit.md)): that audit's
  "Marginalia" direction is superseded; the live design is `tokens.css` plus its gates in
  `render/tests.rs`. Every change stays invariant-safe: no CDN, no preview write-back, no new output
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
- **A new front-matter key trips THREE drift gates, all inside `taliesin-core`.** `CLAUDE.md` names
  them and is the current count (the companion's bundled schema copy, once a fourth, was cut on
  2026-08-20). Withdrawing a key has no register to update: the retirement registers were cut on
  2026-08-17. It means deleting the *read* as well as the vocabulary entry, and a parser-side pin is
  the only thing that says the read is gone. That cost is the standing argument for "derive, don't
  declare" above.
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

100. **Confirm `AJBogo9/taliesin-old` is gone, then delete this item.** It held the un-rewritten
     history that ruling D-8 forbids keeping on GitHub. On 2026-09-24 `gh repo view
     AJBogo9/taliesin-old` could not resolve it and `gh repo list AJBogo9 --visibility private`
     listed only `taliesin-private-archive`, so it looks deleted; the author confirms. There is one
     working copy, `~/Documents/personal/taliesin`, whose `origin` is the public `AJBogo9/taliesin`.

149. **The README's only image is the licence badge.** The four screencasts are MP4, so putting
     one in the README needs a GIF conversion or an uploaded asset URL, not a one-line embed.
     Anything quoting the speed ratio reads `tools/live-edit-bench/RESULTS.md`'s "why the ratio is
     9x and not 83x" section first.

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Per the PMF audit (2026-07-18) the tool is
feature-complete for ~one real user, so the highest-leverage next move is **real users**, not more
features — which is the whole argument behind the 2026-08-07 prune above. When publishing, lead the
copy with the **speed moat** (warm server, block-level incremental, no per-edit rebuild), the single
most-repeated Quarto grievance and the most under-marketed asset.
