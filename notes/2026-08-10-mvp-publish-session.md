# MVP publish path — what ran on 2026-08-10, and what is left

**Read this before `notes/mvp-waves/`.** That directory now holds only the waves that were
**not taken**; the five that were are deleted, because a spent plan whose every line number
is stale is the exact artifact this project keeps getting caught by. What they concluded is
in their commit messages, which carry the evidence, the deviations and a "surfaced, not
fixed" section each.

The tool is at **0.3.0**, `./tools/gates.sh` runs **eleven** gates, and nothing has been
tagged, pushed or published.

## What landed

Eight commits, one per wave, each fast-forward merged to `main` on a green gate.

| Commit | What |
|---|---|
| `5f25b356` | **W2** — the two workflows name only what exists, and CI runs the gates it promises |
| `9da62c2b` | **W4** — the manual stops teaching what the binary refuses |
| `37cb9e0b` | **P1** — `new post` refuses to write outside a project instead of orphaning it |
| `727859c9` | **P2** — an error goes to stderr and nothing goes to stdout |
| `781b0a1c` | **P4** — the build residue is deleted, and `__pycache__/` is ignored |
| `1ffd2aae` | **P3** — the pre-publish gate stops claiming more than it checked |
| `0178e403` | **P5** — `doctor`'s `env` row, a green tick that could never be anything else |
| `17bb5f93` | **W1** — every published number re-measured, and the one with an instrument gated |
| `315d67db` | **W11** — 0.3.0, the retired runner label, and the companion's install path |

W5 (`70de784f`, the licence surface) had landed in the previous session.

## The three things that outlive the plans

### 1. W7 now has a hard dependency it did not have when it was written

**`notes/mvp-waves/W7-dead-code-sweep.md` deletes three corpus documents. The census is now
a gate.** `tools/portability-census.py --verify` is gate 11 in `tools/gates.sh`, and it
asserts that `README.md` and `docs/guide/using/choosing.tmd` still publish the document
count, the line count, the beyond-CommonMark count, the percentage, its complement, and all
six per-family `| n | share |` pairs that the instrument measures.

**So taking W7 without re-running the census and rewriting both pages in the same commit
turns `gates.sh` red, and it will stay red until they agree again.** This was cross-wave
hazard 1 in the original plan; skipping W7 dissolved it, and landing W1 armed it. The step
is mechanical — `python3 tools/portability-census.py`, copy the figures into the two pages,
`--verify` — but it is not optional and it exists nowhere else now that the plan file is
gone. The same applies to **any** change that adds or removes a `corpus/**/*.tmd`.

### 2. W8 stays declined

`notes/mvp-waves/W8-author-and-shared-bib-DECLINED.md` is kept in full so the decision can
be revisited in one instruction. The structured `author:` form and the project-wide shared
bibliography are the only two document features the 2026-08-09 re-audit still recommended
cutting, and both cases are strong. They were declined because cutting them is a refactor
through five crates whose failure mode is a **silently missing byline**, it moves the census
a second time, and the author's next step is publishing. **Take it after real users exist,
or never.**

### 3. Surfaced during the work, not fixed

None of these is a defect a reader can see; all are code or test-name drift.

- `crates/core/src/frontmatter.rs`'s test is named
  `csl_stays_recognized_because_dropping_it_would_mis_suggest_css` — the dead `css`/`csl`
  edit-distance rationale surviving in a test name, though the body already states the live
  one and W4 fixed the manual's copy.
- `crates/server/src/serve_site/mod.rs` still lists `.panel-tabset`.
- `crates/core/src/cite/render.rs`'s `RETIRED_XREF_PREFIXES` keeps seven labels alive on
  purpose, documented and correct.
- `vocab.rs`'s `xrefPrefixes` offers 5 of the 12 `XREF_LABELS`, as `CLAUDE.md` records.
- Running `new post` from a *subdirectory* of a project writes
  `<project>/posts/posts/<slug>`. The page is inside the project so the walker finds it and
  it ships; the path is merely odd. Tightening the P1 guard to "the CWD must BE the project
  root" is a wider behaviour change than that item specified.

## What stands between this tree and a tag

1. **Push `main`.** Nine commits ahead of `origin/main`, none pushed.
2. **Make the repository public.** Every `ci.yml` job is guarded on
   `github.event.repository.private != true`, so CI certifies nothing until then. Do this
   **before** tagging: a red CI on the publish commit is a bad first impression, a broken
   release tarball is worse.
3. **`git tag v0.3.0 && git push --tags`, then watch the workflow actually run.** It never
   has — `git tag` holds only `interpreter-resolution-fix` and `pre-cut`, and
   `release.yml` triggers on `push: tags: ["v*"]`, so the entire release path is unexercised
   on any repo state. Its two blocking items have landed (W5 puts the licence notices in the
   tarball, W2 makes the workflows name only what exists) and the retired `macos-13` runner
   label is fixed, but nothing has ever executed it.
4. **Verify the produced `.tar.gz`** holds the binary, `LICENSE`, `THIRD_PARTY.md` and the
   four licence files W5 added.

## Notes on how the numbers were arrived at, since they will drift again

Measured 2026-08-10 against a fresh release build, and now published:

- Census: **82 documents, 7,157 lines, 498 beyond-CommonMark (7.0%)**. Largest family is the
  **attribute block (108)**, not the fenced div — the published table had the ranking wrong
  as well as every cell.
- Builds, best of three, warm `_freeze/`, page counts read from **the build's own summary
  line** (`find -name '*.html'` is not the page count: a project may ship its own `404.html`,
  which is a page, and `_`-prefixed files never are): `docs/internals` 7 pages / 0.13 s,
  `docs/guide` 19 / 0.25 s, `corpus/tech-blog` 17 / 0.58 s.
- Time-to-ready, from the `log::ready` line the tool already prints: single document
  **3–8 ms**, 19-page book **≈130 ms**.
- Cold release build: **257 crates, 1m 43s at `-j3`, ~32 MB binary**. Measured with
  `CARGO_TARGET_DIR=/tmp/cold-target` rather than `cargo clean`, so the existing
  `target/release/taliesin` survived.

**The standing rule that came out of this is in `CLAUDE.md` under Conventions:** never
publish a number about this tool that has no committed instrument; one without an instrument
carries its measured-on date and is re-measured before a release tag. The census is gated
because the page hands the reader the command, which makes a mismatch self-refuting rather
than merely stale. Wall clocks, binary size and crate count are deliberately **not** gated —
they measure the machine, so they carry a date instead.

## The gate count is 11, and it is not to be incremented by hand

`rg -c 'PASSED\+=' tools/gates.sh` returns 2 and `rg -c '^run_gate '` returns 5; six gates
are hand-rolled stanzas invisible to both, so neither command is a recount. **The
authoritative number is the one the script prints.** It is written in three places
(`tools/gates.sh:16`, the verdict comment near the end, and the `CLAUDE.md` paragraph), all
three of which now also say to take it from the verdict line.
