# Publication prep: Taliesin 1.0 (design)

**Date:** 2026-08-20. **Branch:** `item-100-publication-prep`. **Baseline:** `main` at
`3ebcdecb`, clean.

This is the design for backlog item **100's prep half** plus the parts of **148**, **149**
and **170** that can land while the repository is still private. It does **not** cover
Phase 2 of item 100, which is the irreversible flip and stays behind a separate explicit
instruction from the author.

It supersedes the sequencing in `2026-07-28-public-flip-audit.md` where the two disagree,
because that audit's tables are reads of the 2026-07-28 tree and the 2026-08-03
remediation. The tree has moved materially since (see "Findings that change the audit's
tables" below).

## The decisions this rests on

All seven were taken by the author on 2026-08-20, in this session.

| # | Decision |
|---|---|
| D-1 | **Prep everything, stop at the irreversible gate.** Nothing public happens without a second explicit instruction. |
| D-2 | **Item 103 is closed: keep the name.** The SEO cost is accepted. Publish as "Taliesin, the `.tmd` dev server" so the disambiguator always travels. |
| D-3 | **The first public tag is `v1.0.0`, with an explicit maintenance stance.** Feature-complete for its one use case, bug reports accepted, feature requests closed by design. |
| D-4 | **Freeze scope on the current tree.** Phase 4 of the feature-audit backlog already landed at `58a22328`; C8 and X1 are dropped and recorded so they are not re-filed. |
| D-5 | **`notes/` is KEPT whole**, minus the purged seven, with its dead links repaired. Revised from an earlier answer in the same session; the earlier answer was "remove entirely" and is superseded. |
| D-6 | **The four sites are built and browser-verified but NOT deployed.** taliesin.sh stays dark until after the flip. |
| D-7 | **The workflow rehearsal runs on a guard-neutralised branch** against the private repo, accepting the Actions minutes. |

### Why D-5 was revised

Deleting `notes/` would have removed `DO-NOT-REBUILD.md`, `LESSONS.md`, `CUT-PROGRESS.md`
and `ROADMAP.md` from the repository that CLAUDE.md points at by name, and the mitigation
(folding them into CLAUDE.md) is lossy. Three tests already exempt `notes/` by name and
with a stated reason (`retired_names.rs:9`, `stale_docs.rs:22` which says it "must stay
excluded", `cross_site_links.rs:40`), `notes/README.md` already frames the directory as
notes-to-self, and `notes/` is in none of the four published projects. Deleting it would
have contradicted a call the codebase has already made three times. The standing "lean
towards cutting" directive is a bar for product features (B1 to B4 are all user-facing
utility) and does not govern the engineering log.

## The gate, stated precisely

Everything in this design happens on branches off `main` and lands on local `main`. The
only things that leave this machine are pushes to the **existing private** `origin` and
the Actions rehearsal on that private repo.

**The gate is item 100 Phase 2:** `git filter-repo` on a clone, `gh repo rename` to
`taliesin-private-archive`, `gh repo create AJBogo9/taliesin --public`, push. This design
stops before it and produces a go/no-go document instead.

Also past the gate, and therefore out of scope here: pushing the `v1.0.0` tag (item 148,
because `release.yml` is guarded on `repository.private != true` and tagging while private
produces nothing, silently), setting up DNS, deploying the four sites, and any
announcement.

## Stage 1: rehearse both workflows (FA4)

**First, before any content work.** This is the only item that can fail *structurally*, and
discovering that after the 1.0 tree is written wastes the write.

**The problem this solves.** `ci.yml` and `release.yml` have never executed once. Actions is
off at the repository-settings level (`gh api repos/AJBogo9/taliesin/actions/permissions`
returned `{"enabled": false}` when measured on 2026-08-17), so no run is created by any
trigger. That is roughly 300 lines of YAML across 10 jobs and a 3-OS matrix whose first
execution would otherwise be launch day.

**The complication the backlog does not record.** Every job in both files carries
`if: github.event.repository.private != true`. The `workflow_dispatch` payload *does*
include `repository`, so simply enabling Actions and clicking Run on the private repo
creates a run in which every job **skips**: a green checkmark that certifies nothing. The
rehearsal must neutralise the guard.

**Steps.**

1. Enable Actions on the private repo.
2. Push branch `rehearse-workflows` off `main` with the **eight** `if:` guard lines removed
   (6 in `ci.yml` at :59, :108, :141, :166, :191, :203; 2 in `release.yml` at :29, :43;
   `ci.yml:14` is a comment that quotes the guard, not a guard) and
   nothing else changed.
3. `gh workflow run ci.yml --ref rehearse-workflows` and the same for `release.yml`.
4. Read both runs' logs in full. `release.yml` dispatched without a `v*` tag derives its
   stage name from `github.ref_name`, so artifact names will read
   `taliesin-rehearse-workflows-<target>`. That is expected. Read for real failures
   (toolchain, missing files in the `Package` step's `cp` list, permissions, cache), not
   for cosmetics.
5. Record what each job did, delete the branch, leave `main`'s guards untouched.

**Done condition.** Both workflows have completed at least once with their logs read, and
every failure is either fixed on `main` or recorded in the go/no-go document as a known
issue. Post-flip the guard evaluates true anyway, so this runs the exact job bodies that
will run for real.

**Re-run at Stage 5** against the final tree, because an early green run expires with the
next merge.

## Stage 2: the 1.0 tree

One branch, several commits.

### 2.1 Version

`[workspace.package] version` from `0.3.0` to `1.0.0` in the root `Cargo.toml`, and rewrite
the explanatory comment above it (it currently justifies 0.3.0 over 0.2.1 and reasons about
being pre-1.0).

### 2.2 README truth pass

Two claims are **confirmed false** against the current source:

- Line ~161 advertises "themes (light/dark + **custom**)". The `theme:` key was cut on
  2026-08-17; no `"theme"` key is read anywhere in `crates/core/src`.
- Line ~154 advertises "attributed `.btn` links". Link attribute blocks were cut in
  `093d8b0c`; `parse_pandoc_attrs` and `link_attr` have no hits in `crates/core/src`.

Every remaining bullet in that list is verified against the **validator consts**, never
against `vocab.rs` (which is the offered-completions subset and under-reports). Re-run
`python3 tools/portability-census.py` and copy its output into both `README.md` and
`docs/guide/using/choosing.tmd`: the Phase 0 to 3 cuts changed lines under `corpus/`, and
the census runs only in `gates.sh`, not in `cargo test` and not in the pre-push hook.

### 2.3 A committed instrument for 2.2

`crates/core/tests/retired_names.rs` currently exempts `notes/` and never reaches
`README.md`. Extend its sweep to cover `README.md`, so a cut feature named there fails
`cargo test` instead of a reader's expectation.

This is the project's own rule applied to prose: never publish a claim about this tool that
has no committed instrument. It is also the exact failure that just bit, twice in one file.

**Verify by mutation:** re-add the word "custom" to the themes bullet, watch the named test
fail, remove it again.

### 2.4 The maintenance stance

- README's third "Before you adopt it" bullet currently reads "**One maintainer, pre-1.0.**
  No support contract, no release cadence, no bus factor above one." Rewrite the heading
  half for 1.0 while keeping the honest risk framing and the three bounding facts
  (Markdown source you already hold, built HTML with no dependency on this tool, an AGPL
  licence that makes a fork always available).
- Add a short **Project status** section: feature-complete for its one use case, bug
  reports welcome, feature requests closed by design, pointing at `CONTRIBUTING.md` for the
  scope rules.
- `CHANGELOG.md`: write the `1.0.0` entry covering what changed since `0.3.0` (the cut
  campaign through Phase 4, the `theme:` removal, the six-verb CLI), and revise the
  preamble's "loose semantic versioning while pre-1.0" paragraph into the post-1.0 stance.
- Check `CONTRIBUTING.md` for anything that now contradicts the stance. Its opening already
  says feature requests adding an output format are out of scope by design, which is
  consistent.

## Stage 3: launch presentation (item 149, the pre-gate parts)

- `CODE_OF_CONDUCT.md`: Contributor Covenant 2.1. Contact address is
  `andreas.bogossian9@gmail.com`, which `SECURITY.md:26` already publishes as the fallback
  vulnerability-report address, so this reuses a published decision rather than making a
  new disclosure.
- `.github/ISSUE_TEMPLATE/`: a bug-report form, and a `config.yml` that states feature
  requests are out of scope by design and links `CONTRIBUTING.md`. This is the mechanism
  that makes D-3's maintenance stance cheap to keep.
- `SECURITY.md`: read it end to end for coherence as a public document.
- `homepageUrl` set to `https://taliesin.sh` via `gh repo edit`. Invisible while private
  and reversible.

Explicitly **not** here: the README screencast (the four clips are MP4 and would need a GIF
conversion or an uploaded asset URL, which needs the public repo), and anything quoting the
speed ratio, which must read `RESULTS.md`'s "why the ratio is 9x and not 83x" section first.

## Stage 4: the purge set and the rewrite dry run

### 4.1 Re-run the enumeration first

**Do not trust the audit's D1 and D2 tables.** They are reads of the 2026-07-28 tree plus a
2026-08-03 remediation, and the tree has moved. Re-run the path enumeration and the leaked
content search against the current tree before building the filter-repo invocation.

### 4.2 The purge set

Purged from all history with `--invert-paths`:

| Path(s) | Why |
|---|---|
| `notes/STARTUP-PLAN.md` **and** `STARTUP-PLAN.md` | money and strategy; git-tracked while its own header says it must not be |
| `notes/FUNDING-RESEARCH.md` **and** `FUNDING-RESEARCH.md` | same |
| `notes/2026-07-18-pmf-audit.md` | commercial conclusions |
| `notes/2026-07-27-due-diligence-audit.md` | commercial conclusions |
| `notes/2026-07-28-demand-positioning-audit.md` | commercial conclusions |
| `notes/2026-07-28-launch-critique.md` | commercial conclusions |
| `notes/2026-07-27-adoption-friction-audit.md` | commercial conclusions |
| `todo.md` | renamed to `notes/backlog.md`, so its blobs stay reachable at the old path across 83 stored versions |
| `corpus/bayesian-website/` | **a third party's rights.** See 4.3. |

Both path spellings are required for the first two: two of the purge set carry a root-level
predecessor path from before the move to `notes/`, and naming only one spelling leaves the
pre-move blobs in the rewrite.

Plus `--replace-text` for whatever restatements of the purged documents' commercial
conclusions survive in files that are **not** purged, and for the roughly 11 commit subjects
that name the purged documents. Most of the seven known restatements live under `notes/` in
files that are themselves purged, so re-run 4.1 to find what actually remains rather than
carrying the audit's list forward.

### 4.3 The finding that changes the purge set

`corpus/bayesian-website/` is **a named co-author's joint academic work plus a university's
own assignment brief committed verbatim** (findings D7-1 and D7-2, which the audit called
"the only things that genuinely must be right", because they involve someone else's rights).

It was deleted from `HEAD` on 2026-08-03 and replaced by a purpose-built
`corpus/single-page-report/`. But that remediation was explicitly the reversible half only:
"repair commits only, no history rewrite". **It survives in 7 commits of history, and the
audit's D1 purge table does not name it.** Publishing the history as the audit's tables
describe would publish work the author cannot license alone.

Adding it to `--invert-paths` is this design's amendment to the audit, and it is the single
most consequential line in this document.

### 4.4 Repairs on `HEAD`, same change

The D2 Class-A link repairs were deliberately deferred on 2026-08-03 because they are
*coupled to the purge*: removing an index row that points at a file still in the tree would
make the documentation wrong. Now that the purge is happening, they land:

- `notes/AUDITS.md`: five live Markdown links to purge-set files.
- `notes/README.md:17`: the row indexing the two money documents. Also refresh the file's
  opening, which still says "START HERE while the scope reduction is running"; the cut is
  complete and `CUT-PROGRESS.md` is now its durable record.
- `notes/backlog.md:336`: a link to the adoption-friction audit.
- `notes/2026-07-28-deck-exemption-audit.md:85`: a link to the due-diligence round.

Re-locate each by search string, not by line number.

Also in this change: **move `docs/superpowers/`'s 6 tracked files into `notes/`.** R6-1
deleted that directory on 2026-08-09 because plans and specs under `docs/` read as "the
manual"; the reason was placement, not content, and the directory has since regrown to 6
files and 336 KB while CLAUDE.md and the backlog both still record it as gone. Moving
honours both rulings. Update CLAUDE.md's and the backlog's statements about it.

`.githooks/pre-push:45-46` carries a purge tripwire that refuses to push files matching the
purge set. Keep it and extend it to `corpus/bayesian-website`.

### 4.5 The dry run

Run `git filter-repo` **on a clone**, never on the working repository. Then verify the
clone:

- it builds;
- `./tools/gates.sh` is green, count taken from the script's own verdict line;
- `git log -S` finds none of the purged strings anywhere in the rewritten history;
- `git log --all -- <each purged path>` returns nothing;
- the commit count and the authorship record are otherwise intact.

## Stage 5: verify and hand over

- `./tools/gates.sh` on the final tree, with `TALIESIN_PYTHON="$PWD/.venv/bin/python"`
  exported. Take the count from its own verdict line.
- `tools/publish.sh --check`.
- A real `preview` of all four projects, browser-verified through the chrome-devtools MCP
  at 390x844, 1440x900 and 900x1440. The portrait band is where layout defects show.
- Re-run both workflows against the final tree per Stage 1.
- Write the go/no-go document: the exact Phase 2 command sequence, the dry-run transcript,
  what every check actually said, and an explicit list of what could not be verified.

## Out of scope

- **Phase 2 of item 100.** The flip itself. Separate explicit instruction.
- **Item 148's tag.** `release.yml` produces nothing while the repo is private.
- **Item 170's deploy** and the `live-edit-hero-demo` clip.
- **Package managers.** crates.io, Homebrew and Nix are a decision after the tag.
  `cargo publish` would reject this workspace as-is: `taliesin-core` is declared as a path
  dependency with no `version`, and `keywords`, `categories`, `readme`, `homepage` and
  `documentation` are blank in every manifest.
- **C8 and X1.** Dropped per D-4. C8's own spot-check step 3 asks whether a Taliesin-native
  paper with affiliations is planned before EUSIPCO; the author's active paper is on
  IEEEtran, outside Taliesin, which answers it toward the drop. X1 stays frozen pending an
  explicit written override.
- **Any further feature work.** D-4 froze the scope.

## Done conditions

1. `main` carries the 1.0 tree: version `1.0.0`, a README with no false claims and a test
   that keeps it that way, the maintenance stance, `CHANGELOG` 1.0.0, code of conduct,
   issue templates, repaired links.
2. A rewritten clone exists, verified per 4.5, pushed nowhere.
3. Both workflows have executed at least once with their logs read and reported.
4. All four site projects browser-verified at the three viewports.
5. `./tools/gates.sh` green on the final tree, count quoted from its verdict line.
6. A go/no-go document exists naming the exact Phase 2 sequence and everything unverified.

## What is still open afterwards

`notes/backlog.md` items 148, 149 and 170 remain open, because tag, presentation and deploy
all live past the gate. **"Done" here means the scope is frozen and the tree is
publishable, not that the backlog is empty.**
