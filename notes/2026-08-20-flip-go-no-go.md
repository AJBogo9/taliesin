# Flip go/no-go, 2026-08-20

## Verdict

**GREEN.** The tree is publishable, everything before the irreversible gate is done and
verified, and the gate has not been crossed. Phase 2 needs one instruction from the author.

---

## What was verified, with the command and its output

| Check | Command | Result |
|---|---|---|
| Full gate suite | `./tools/gates.sh` | `PASSED — every gate ran and passed (12 gates)`, exit 0 |
| Pre-publish gate | `tools/publish.sh --check` | `publish: ok - 4 project(s) check clean` (runs inside gates.sh) |
| Version | `taliesin --version` | `taliesin 1.0.0 (c7617ac8)` |
| README truth pin | `cargo test ... the_readme_does_not_advertise_withdrawn_constructs` | `1 passed; 0 failed` |
| Portability census | `python3 tools/portability-census.py --verify` | exit 0 |
| CI, final tree | run `32395506257` | **success**, 6 of 6 jobs |
| Release, final tree | run `32395509519` | **success**, 4 of 4 jobs, both macOS targets |
| Rewrite A (archive) | `git-filter-repo` on a mirror clone | 4 paths at 0 commits; co-author surname 0 hits across every blob of every commit |
| Rewrite B (public) | `git-filter-repo` on a mirror clone | 16 paths at 0 commits; all sensitive string classes at 0, given name included |
| Rewrite B, **re-run with `--replace-message`** | `git-filter-repo` on a fresh mirror clone, 2026-08-20 | 2,168 → 2,120 commits (48 pruned empty). All **16** paths at 0, which also rehearses **B-13** (`W9-notes-hygiene.md`) for the first time; the earlier run carried 15. All 19 literal keys at **0 objects and 0 messages**, against a control run on the un-rewritten repo scoring **679 objects and 5 message lines** |
| Rewritten history builds | `cargo test -p taliesin-core --no-fail-fast` in a clone | **50 suites, 827 passed, 0 failed, 0 ignored** |
| Rewritten history lints | `build docs/{guide,internals} --check-only` in a clone | `no static problems found`, exit 0 both |
| Browser, 4 projects | chrome-devtools MCP at 390x844 / 900x1440 | 0 console errors, 0 broken images |

Detail: `2026-08-20-workflow-rehearsal-log.md`, `2026-08-20-rewrite-dry-run.md`,
`2026-08-20-final-verification.md`.

---

## What is NOT verified

- **The macOS binaries were built, never executed.** The matrix proves they compile, link
  and package on `aarch64-apple-darwin` and `x86_64-apple-darwin`. Nobody has run them.
- **`release.yml` has never fired on a real `v*` tag.** Both runs were `workflow_dispatch`
  on a branch, so the tag-derived naming path is unexercised. The first real tag is the
  first test of that path. **Correction, 2026-08-20:** the claim that accompanied this,
  "no GitHub Release has ever been published", is FALSE and is corrected here and in the
  rehearsal log and final-verification record. `gh release list` returns **two** published
  Releases, `rehearse-2` (flagged Latest) and `rehearse-workflows`, each carrying all six
  expected assets: three targets, `.tar.gz` plus `.sha256`. So the `create` job, the upload
  and the packaging are proven end to end; only the `v*` trigger and the tag-derived asset
  naming are untested. Item 148 is smaller than it was written. Both Releases follow the
  rename into the private archive and reach no public repo.
- **DNS and Cloudflare Pages binding are unexercised.** taliesin.sh has no A record.
- **The rewritten history was verified in a clone, never pushed.**
- **`notes/backlog.md`'s redaction coverage is medium confidence**, not high. The literal
  strings were verified against today's wording and two named historical blobs, not against
  every historical version. See the mitigation below.
- **The redaction list was wrong THREE times and every time a sweep caught it, not the
  list.** It first missed a phrase by one word; then handled the co-author's surname while
  leaving the given name; then, found 2026-08-20, covered only blob contents while the same
  three name forms sat in three commit message bodies that `--replace-text` cannot reach.
  All three are fixed and re-verified at 0, but the pattern is the point, and the third
  instance sharpens it: the first two were wrong *entries*, the third was a wrong *surface*.
  Ask what the sweep does not cover, not just what the list does not contain. The surfaces
  are: blob contents, commit and tag messages, path names, ref names, and author identity.
  As of 2026-08-20 all five are swept, with path names at 0 of 1,543 distinct paths ever
  committed, both tags lightweight so they carry no message object, and one author.
  After pushing, sweep the real remote rather than trusting the list.

---

## The exact Phase 2 sequence

Ordering is fixed. The bundle is written and verified FIRST because it becomes the only
complete backup in existence, and the original repository is deleted LAST, only after both
replacements verify.

```
0. PRE-FLIGHT
   git checkout main                    # main already carries the prep; there is no
                                        #   item-100-publication-prep branch left to merge
   ~/.local/bin/git-filter-repo --version   # MUST run. The pipx symlink dangles after a
                                        #   VS Code snap revision bump, and this exact
                                        #   prerequisite was recorded as discharged while
                                        #   being completely non-functional. Verify by
                                        #   running it, never by reading the note.
   ./tools/gates.sh                     # must be PASSED, every gate ran
   # Stale local branches: DONE 2026-08-20. 29 were deleted (all were fully contained in
   #   main, so nothing was lost; name+SHA restore list is in taliesin-private/). Only
   #   `debug-mode` remains, pinned by the worktree at ../taliesin-debug. Nothing to decide.
   # The rehearsal residue on the remote is TWO TAGS, not a branch: refs/tags/rehearse-2
   #   and refs/tags/rehearse-workflows, plus two published Releases. `git push origin
   #   :refs/heads/rehearse-2` fails, there is no such branch. They are not in the local
   #   repo, and the mirror below clones the LOCAL repo, so they do not reach the public
   #   repo either way; they follow the rename into the private archive.

1. BUNDLE, AND VERIFY IT BEFORE DESTROYING ANYTHING
   git bundle create ~/Documents/personal/taliesin-private/taliesin-full-2026-08-20.bundle --all
   git clone ~/Documents/personal/taliesin-private/taliesin-full-2026-08-20.bundle /tmp/bundle-check
   diff -r --exclude=.git /tmp/bundle-check <working tree>    # must be identical
   THIS BUNDLE IS THE ONLY COMPLETE BACKUP THAT WILL EXIST.

2. FREE THE NAME
   gh repo rename taliesin-old

3. THE ARCHIVE (Rewrite A)
   gh repo create AJBogo9/taliesin-private-archive --private
   Run Rewrite A on a fresh mirror clone, push, verify.
   PASS replace-text-rewrite-a.txt TO BOTH --replace-text AND --replace-message.

4. THE PUBLIC REPO (Rewrite B)
   gh repo create AJBogo9/taliesin --public
   Run Rewrite B on a fresh mirror clone, push, verify.
   PASS replace-text-rewrite-b.txt TO BOTH --replace-text AND --replace-message.

5. VERIFY BOTH REMOTES BEFORE DELETING ANYTHING
   On both:   git log --all -- corpus/bayesian-website        -> nothing
   On public: the same for the money set and todo.md          -> nothing
   Then sweep BOTH SURFACES for every literal key, because they fail differently:
     objects:  git cat-file --batch-all-objects --batch --buffer \
                 | grep -a -i -c -F -f <keys>                 -> 0
     messages: git log --all --format='%H%n%s%n%b' \
                 | grep -a -i -c -F -f <keys>                 -> 0
   `grep -a` is not optional: without it the object stream trips grep's binary
   detection and the scan reports 0 on a repo that is full of hits.
   Run the same two scans against the UN-rewritten repo first as a control. A
   table of zeroes with no known-positive row is a broken probe, not a clean repo.

6. ONLY NOW
   gh repo delete AJBogo9/taliesin-old
```

The exact arguments live OUTSIDE this repository, in
`~/Documents/personal/taliesin-private/`:

| File | What it is |
|---|---|
| `purge-enumeration.md` | The full derivation: both `--path` lists, both `--replace-text` classes, per-row reasoning and confidence |
| `replace-text-rewrite-a.txt` | Rewrite A. Pass to **both** `--replace-text` and `--replace-message` |
| `replace-text-rewrite-b.txt` | Rewrite B. Pass to **both** `--replace-text` and `--replace-message` |
| `2026-08-20-sdd-ledger.md` | Every ruling taken during this work, with its reasoning and its cost if wrong |

They are deliberately NOT in `notes/` and NOT in git: the enumeration concentrates a third
party's name, a university's copyrighted text and the author's commercial figures into one
place, and it purges itself in both rewrites.

**Do not hand-type the arguments.** Paste them. **Do not sort the `--replace-text` files:**
`filter-repo` applies entries in file order, and the full-name forms must precede the bare
surname or they never match. (Re-verified mechanically 2026-08-20: neither file has an
entry that is a substring of a later entry, so nothing is shadowed.)

**One file, two flags, deliberately.** `--replace-text` rewrites blob contents;
`--replace-message` rewrites commit and tag messages. They are different surfaces and
`--replace-text` alone reaches neither commit subjects nor bodies. Pass the SAME file to
both rather than curating a second one, so the two lists cannot drift apart. Why this is a
correction and not a nicety: with `--replace-text` only, the co-author's full name, surname
and given name, plus the university/platform literal, survived in three commit message
bodies (`fa6a8e88`, `4ee1a1c8`, `c2cce9ad`) and would have published in the public repo's
`git log`. Two of those three are the fix-wave commits written to fix the redaction, which
quoted the name while explaining that it needed redacting.

---

## Immediately after the flip, in this order

1. **Dispatch `ci.yml` on the public repo and read it.** Free runners, real guard conditions,
   zero audience. Fix anything red before anyone is looking.
2. **Push `v1.0.0`.** This is the first time `release.yml` fires on a real tag.
3. **Verify the release assets exist**: three `.tar.gz` files with a `.sha256` beside each,
   for `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin`. Only then
   is the README's install section true. (Backlog item 148.)
4. **Then** the sites and DNS (item 170), and any announcement.

---

## Open decisions for the author

1. ~~**Do stale branches publish?**~~ **CLOSED 2026-08-20, by deletion.** The count was 31,
   not "roughly 40", and every one of them was fully contained in `main` with zero commits
   ahead, so they were names and nothing else. 29 deleted; the name+SHA restore list is in
   `taliesin-private/deleted-branches-2026-08-20.txt`. `debug-mode` remains, pinned by the
   clean worktree at `../taliesin-debug`. There is no longer a decision here.
2. **`notes/backlog.md` redaction is medium confidence.** Mitigation, cheap and worth doing:
   after pushing Rewrite B, sweep the public repo for each literal key and confirm zero. Use
   the two-surface scan in step 5, not `git log -S`: **`-S` searches diffs, never messages**,
   which is precisely how the commit-message gap survived the first dry run. If something
   survives, the repo is minutes old with no audience and can be deleted and re-pushed.
3. **Whether to re-sweep the public remote after pushing.** Recommended, and cheap. The
   redaction list was wrong three times during this work and a sweep caught it every time,
   not the list. Minutes after the push the repo has no audience, so a survivor is still
   recoverable by deleting and re-pushing.
4. **The two lightweight tags `interpreter-resolution-fix` and `pre-cut`** are on `main`'s
   ancestry and would travel on a mirror push. Recommendation: do not push tags, so `v1.0.0`
   is the first tag anyone sees. Untidy rather than dangerous either way.
5. **49 commits carry a message naming a purged document** and survive the path purge (only
   6 are pruned as empty). After the `--replace-message` fix the rights-touching subset is
   gone; what remains names a filename or a round name as provenance, which is the same
   "keep" class ruling D2-8 already made. One, `31765205`, puts a commercial framing and a
   benchmark correction in its subject line. Left as-is deliberately: it is engineering
   record, and the engineering record is the reason the history is being published at all.

---

## Risks

| Risk | Mitigation |
|---|---|
| The rewrite drops something it should have kept | The un-rewritten bundle is written and verified first, and the original repo is deleted last |
| A sensitive string survives redaction | Verified at 0 in the dry run; re-verify on the real remote before step 6; the repo can be deleted and re-pushed while it has no audience |
| `release.yml` fails on its first real tag | Its build and package steps are proven; only the tag-naming path is new. A failed release is re-runnable and publishes nothing wrong |
| A macOS binary does not run | Unknown and unmitigated. The README already says building from source is the supported path |
