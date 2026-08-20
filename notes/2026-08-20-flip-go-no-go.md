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
  on a branch, so the tag-derived naming path is unexercised and no GitHub Release has ever
  been published. The first real tag is the first test of that path.
- **DNS and Cloudflare Pages binding are unexercised.** taliesin.sh has no A record.
- **The rewritten history was verified in a clone, never pushed.**
- **`notes/backlog.md`'s redaction coverage is medium confidence**, not high. The literal
  strings were verified against today's wording and two named historical blobs, not against
  every historical version. See the mitigation below.
- **The redaction list was wrong twice and both times a sweep caught it, not the list.** It
  first missed a phrase by one word, then handled the co-author's surname while leaving the
  given name. Both are fixed and re-verified at 0, but the pattern is the point: after
  pushing, sweep the real remote rather than trusting the list.

---

## The exact Phase 2 sequence

Ordering is fixed. The bundle is written and verified FIRST because it becomes the only
complete backup in existence, and the original repository is deleted LAST, only after both
replacements verify.

```
0. PRE-FLIGHT
   git checkout main && git merge --ff-only item-100-publication-prep
   ./tools/gates.sh                     # must be PASSED, every gate ran
   git push origin :refs/heads/rehearse-2   # delete the throwaway rehearsal branch
   Decide: do stale local branches publish? (~40 exist; see "Open decisions" below)

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

4. THE PUBLIC REPO (Rewrite B)
   gh repo create AJBogo9/taliesin --public
   Run Rewrite B on a fresh mirror clone, push, verify.

5. VERIFY BOTH REMOTES BEFORE DELETING ANYTHING
   On both:   git log --all -- corpus/bayesian-website        -> nothing
   On public: the same for the money set and todo.md          -> nothing
   On public: git log -S "<the third-party surname>"          -> nothing

6. ONLY NOW
   gh repo delete AJBogo9/taliesin-old
```

The exact `--path` and `--replace-text` arguments are in the execution workspace at
`.superpowers/sdd/2026-08-20-publication-prep-plan/purge-enumeration.md`. They are
deliberately NOT in `notes/`: that file concentrates a third party's name, a university's
copyrighted text and the author's commercial figures into one place, and it purges itself in
both rewrites.

**Do not hand-type the arguments.** Paste them.

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

1. **Do stale branches publish?** The mirror carries roughly 40 local branches, mostly
   finished feature branches. A mirror push publishes all of them. Untidy rather than
   dangerous, but it is a decision, not a default. Pushing only `main` is the tidy option.
2. **`notes/backlog.md` redaction is medium confidence.** Mitigation, cheap and worth doing:
   after pushing Rewrite B, run `git log -S` on the public repo for each sensitive key and
   confirm zero. If something survives, the repo is minutes old with no audience and can be
   deleted and re-pushed.
3. **The bug-report template requires `taliesin doctor --format json`.** If doctor itself is
   what crashes, a reporter cannot satisfy the form. Consider making that field optional.

---

## Risks

| Risk | Mitigation |
|---|---|
| The rewrite drops something it should have kept | The un-rewritten bundle is written and verified first, and the original repo is deleted last |
| A sensitive string survives redaction | Verified at 0 in the dry run; re-verify on the real remote before step 6; the repo can be deleted and re-pushed while it has no audience |
| `release.yml` fails on its first real tag | Its build and package steps are proven; only the tag-naming path is new. A failed release is re-runnable and publishes nothing wrong |
| A macOS binary does not run | Unknown and unmitigated. The README already says building from source is the supported path |
