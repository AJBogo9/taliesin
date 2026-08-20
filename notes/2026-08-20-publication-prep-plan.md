# Taliesin 1.0 Publication Prep Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the tree to a publishable Taliesin 1.0 and produce a verified go/no-go
dossier, stopping before the irreversible flip.

**Architecture:** Eleven tasks in three arcs. Tasks 1 to 5 make the tree say true things
about itself at version 1.0. Tasks 6 to 9 prepare the history rewrite and rehearse it on
throwaway clones. Tasks 10 and 11 verify and hand over. No task touches the render engine,
and no task performs a public act.

**Tech Stack:** Rust (edition 2024, resolver 3), `git-filter-repo` 2.47.0 at
`~/.local/bin/git-filter-repo`, GitHub CLI (`gh`), Cloudflare Wrangler (build check only),
`./tools/gates.sh`, chrome-devtools MCP for browser verification.

**Spec:** `notes/2026-08-20-publication-prep-design.md`

## Global Constraints

Every task's requirements implicitly include this section.

- **NEVER cross the gate.** No push to any public repository, no `gh repo create`, no
  `gh repo rename`, no `gh repo delete`, no `v*` tag, no `wrangler pages deploy`. Phase 2
  of backlog item 100 requires a separate explicit instruction from the author.
- **`git filter-repo` runs on a CLONE, never on the working repository.** Ever.
- **Work on branch `item-100-publication-prep`.** Do not commit on `main`.
- **Never run two cargo test suites concurrently.** They deadlock. Kill stale runs first.
- **Run `cargo fmt --all` LAST**, after every `.rs` edit in a task, before committing.
- **`export TALIESIN_PYTHON="$PWD/.venv/bin/python"`** before `./tools/gates.sh`, or it
  exits 2 at preflight.
- **Take the gate count from `gates.sh`'s own verdict line**, never from prose.
- **Verify every fix by mutation**: restore the defect, watch the *named* test fail, restore
  the fix. A green suite is not evidence.
- **No em dashes or en dashes in any prose this plan writes.** Use commas, colons,
  parentheses, or restructured sentences. This is a standing author preference.
- **`cargo build` is NOT a sufficient compile check.** It does not compile `#[cfg(test)]`
  fixtures. Use `cargo test --workspace --no-run`.
- **Version:** the workspace moves `0.3.0` to `1.0.0`. Nothing else changes version.
- **Name:** publish as "Taliesin, the `.tmd` dev server" wherever a disambiguator is needed.
- Line numbers in this plan are reads of the tree at `3ebcdecb` plus this branch's commits.
  **Re-locate every one by search string.**

---

### Task 1: Rehearse both GitHub workflows (FA4)

This is an operational task with no test cycle. It comes first because it is the only
item that can fail structurally, and discovering that after the 1.0 tree is written wastes
the write.

**Files:**
- Create (throwaway branch only): none permanent
- Modify (on the throwaway branch `rehearse-workflows` only): `.github/workflows/ci.yml`,
  `.github/workflows/release.yml`
- Record: `notes/2026-08-20-workflow-rehearsal-log.md`

**Interfaces:**
- Consumes: nothing
- Produces: `notes/2026-08-20-workflow-rehearsal-log.md`, read by Task 11's dossier.

**Background the implementer needs.** Both workflows have never executed once. Actions is
disabled at the repository-settings level, so GitHub creates no run for any trigger. Every
job carries `if: github.event.repository.private != true`, and the `workflow_dispatch`
payload *does* include `repository`, so simply enabling Actions and clicking Run on the
still-private repo produces a run in which every job **skips**. That is a green checkmark
that certifies nothing. The guards must come off for the rehearsal. Post-flip the guard
evaluates true anyway, so this runs the exact job bodies that will run for real.

- [ ] **Step 1: Enable Actions on the private repository**

```bash
gh api -X PUT repos/AJBogo9/taliesin/actions/permissions -f enabled=true
gh api repos/AJBogo9/taliesin/actions/permissions
```

Expected: `{"enabled":true,...}`.

- [ ] **Step 2: Create the throwaway branch with the guards removed**

There are exactly **eight** guard lines: `ci.yml` at 59, 108, 141, 166, 191, 203 and
`release.yml` at 29, 43. `ci.yml:14` is a comment that quotes the guard and must NOT be
touched.

```bash
git checkout -b rehearse-workflows
sed -i '/^    if: github\.event\.repository\.private != true$/d' \
  .github/workflows/ci.yml .github/workflows/release.yml
grep -c "if: github.event.repository.private != true" \
  .github/workflows/ci.yml .github/workflows/release.yml
```

Expected: `ci.yml:1` (the surviving comment on line 14) and `release.yml:0`.

- [ ] **Step 3: Verify nothing else changed, then push the branch**

```bash
git diff --stat
git diff | grep '^[+-]' | grep -v '^[+-][+-]' | grep -v "if: github.event.repository.private"
```

Expected: the second command prints nothing. Then:

```bash
git commit -am "test(ci): THROWAWAY rehearsal branch, guards removed. Do not merge."
git push -u origin rehearse-workflows
```

- [ ] **Step 4: Dispatch both workflows**

```bash
gh workflow run ci.yml --ref rehearse-workflows
gh workflow run release.yml --ref rehearse-workflows
sleep 20 && gh run list --branch rehearse-workflows
```

- [ ] **Step 5: Watch both to completion and read the logs in full**

```bash
gh run watch $(gh run list --branch rehearse-workflows --workflow ci.yml --limit 1 --json databaseId --jq '.[0].databaseId')
gh run view  $(gh run list --branch rehearse-workflows --workflow ci.yml --limit 1 --json databaseId --jq '.[0].databaseId') --log > /tmp/ci-rehearsal.log
gh run watch $(gh run list --branch rehearse-workflows --workflow release.yml --limit 1 --json databaseId --jq '.[0].databaseId')
gh run view  $(gh run list --branch rehearse-workflows --workflow release.yml --limit 1 --json databaseId --jq '.[0].databaseId') --log > /tmp/release-rehearsal.log
```

**Expected oddity, not a failure:** `release.yml` dispatched without a `v*` tag derives its
stage name from `github.ref_name`, so artifacts will be named
`taliesin-rehearse-workflows-<target>.tar.gz`. Read for *real* failures: toolchain
resolution, a missing file in the `Package` step's `cp` list, `contents: write`
permissions, cache behaviour, and whether all three matrix targets built.

- [ ] **Step 6: Record the results**

Write `notes/2026-08-20-workflow-rehearsal-log.md` containing, for each of the ten job
runs: the job name, conclusion, duration, and for any failure the quoted log excerpt and
whether it is fixed on `main` or carried into the dossier as a known issue. State the run
IDs so a reader can re-open them.

- [ ] **Step 7: Delete the throwaway branch, return to the working branch**

```bash
git checkout item-100-publication-prep
git push origin --delete rehearse-workflows
git branch -D rehearse-workflows
grep -c "if: github.event.repository.private != true" .github/workflows/ci.yml
```

Expected: `7` (six guards plus the line-14 comment), proving `main`'s guards are intact.

- [ ] **Step 8: Commit the log**

```bash
git add notes/2026-08-20-workflow-rehearsal-log.md
git commit -m "docs(notes): first-ever execution of ci.yml and release.yml, logged

FA4. Both workflows had never run: Actions was off at the repository-settings
level, so no trigger created a run. Rehearsed on a throwaway branch with the
eight private-repo guards removed, because workflow_dispatch carries the
repository payload and would otherwise skip all ten jobs and return a green
checkmark certifying nothing."
```

**If any job failed:** fix it on `item-100-publication-prep`, then re-run Task 1 steps 2
through 6 before moving on. A failing workflow is a launch blocker, because the README
points strangers at the releases page.

---

### Task 2: Version bump to 1.0.0

**Files:**
- Modify: `Cargo.toml:5-13` (the `[workspace.package]` block)
- Modify: `Cargo.lock` (regenerated)

**Interfaces:**
- Consumes: nothing
- Produces: workspace version `1.0.0`, read by Task 4's CHANGELOG entry and by
  `release.yml`'s tarball naming after the flip.

- [ ] **Step 1: Read the current block**

```bash
sed -n '5,14p' Cargo.toml
```

Current content:

```toml
[workspace.package]
# 0.3.0, not 0.2.1: the scope-reduction campaign removed eleven CLI verbs and about half
# the document feature set, which is breaking under any reading. Pre-1.0, so a minor
# carries breaking changes — CHANGELOG.md's own stated policy.
version = "0.3.0"
```

- [ ] **Step 2: Replace the comment and the version**

Replace those five lines with exactly this (note: no em dashes):

```toml
[workspace.package]
# 1.0.0: the scope is closed. The 2026-08 reduction campaign finished at Phase 4, the
# feature set is final for the tool's one use case, and the first public tag says so.
# Post-1.0 the promise is ordinary semver: a breaking change needs a 2.0.
version = "1.0.0"
```

- [ ] **Step 3: Regenerate the lockfile and verify the version propagated**

```bash
cargo check --workspace --quiet
grep -A2 'name = "taliesin-core"' Cargo.lock | head -3
cargo run -q -p taliesin-server -- --version
```

Expected: `Cargo.lock` shows `version = "1.0.0"` for both workspace crates, and the binary
prints a `1.0.0` version string.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: 1.0.0

The scope is closed. Phase 4 of the feature audit landed at 58a22328, C8 and
X1 are dropped, and the first public tag says so rather than leaving the
README's 'pre-1.0' hedge to carry it."
```

---

### Task 3: Pin the README against withdrawn constructs, then make it true

TDD applies: the pin is written first and must FAIL, because the README currently
advertises two features the tool deleted.

**Files:**
- Modify: `crates/core/src/render/tests.rs` (append at end of file; `repo_root()` already
  exists at `crates/core/src/render/tests.rs:3474`)
- Modify: `README.md` (the feature list, around lines 150 to 165)

**Interfaces:**
- Consumes: `repo_root() -> std::path::PathBuf`, already defined in the same file.
- Produces: `WITHDRAWN_README_PHRASES: &[(&str, &str)]` and
  `the_readme_does_not_advertise_withdrawn_constructs()`, both in
  `crates/core/src/render/tests.rs`. Task 4 and Task 5 also edit `README.md` and must keep
  this test green.

**Why a unit test and not an integration test.** `KNOWN_KEYS`, `XREF_LABELS` and
`DIV_FEATURE_CLASSES` are all `pub(crate)`, so a test under `crates/core/tests/` cannot see
them. The existing sibling gate `the_reference_page_documents_every_known_key` lives inside
`crates/core/src/frontmatter.rs` for exactly this reason. Follow that precedent.

- [ ] **Step 1: Write the failing test**

Append to the end of `crates/core/src/render/tests.rs`:

```rust
/// Constructs Taliesin no longer implements, which the README must not advertise.
///
/// **Why this is a pin and not a register.** Nothing user-facing reads this list; it
/// exists because the README's feature list is *prose*, and no structural gate reaches
/// prose. `the_reference_page_documents_every_known_key` walks `KNOWN_KEYS` into the
/// guide and would not have caught either entry below, because neither construct is a
/// front-matter key.
///
/// Both entries shipped in the README for weeks after their code was cut, which is the
/// whole argument for the file: a reader's first two minutes with the project are spent
/// here, and the project's own rule is that no claim about the tool ships without a
/// committed instrument.
const WITHDRAWN_README_PHRASES: &[(&str, &str)] = &[
    (
        "+ custom",
        "the `theme:` key was CUT 2026-08-17: both palettes always ship and the reader's \
         device selects one at paint, so there is no author theme control at all",
    ),
    (
        ".btn",
        "link attribute blocks `[text](url){.class}` were CUT in 093d8b0c, so there is no \
         way to attribute a link and no `.btn` class to attach",
    ),
];

#[test]
fn the_readme_does_not_advertise_withdrawn_constructs() {
    let path = repo_root().join("README.md");
    let src = std::fs::read_to_string(&path).unwrap();
    let found: Vec<String> = WITHDRAWN_README_PHRASES
        .iter()
        .filter(|(phrase, _)| src.contains(phrase))
        .map(|(phrase, why)| format!("{phrase:?}: {why}"))
        .collect();
    assert!(
        found.is_empty(),
        "README.md advertises constructs the tool no longer implements:\n  {}",
        found.join("\n  ")
    );
}
```

- [ ] **Step 2: Run it and verify it FAILS with both entries listed**

```bash
cargo test -p taliesin-core --lib the_readme_does_not_advertise_withdrawn_constructs -- --nocapture
```

Expected: **FAIL**, and the message names both `"+ custom"` and `".btn"`. If it names only
one, the README wording has drifted from this plan's read: re-locate by search string and
adjust the phrase before continuing. **Do not weaken the test to make it pass.**

- [ ] **Step 3: Fix the two false claims in README.md**

Locate by search string, not line number.

Find the bullet containing `attributed \`.btn\` links` and replace that bullet with:

```markdown
- Callouts, `layout-ncol` grids, raw `{=html}` passthrough.
```

Find the bullet containing `themes (light/dark + custom)` and replace that bullet with:

```markdown
- Live **`{js}`** cells (a tiny native enhancer with vendored d3 + Observable Plot,
  no Observable runtime), **mermaid** diagrams, and a responsive reading layout
  (print stylesheet). Light and dark palettes both ship and the reader's device
  selects one, with no per-site theme control to configure.
```

- [ ] **Step 4: Run the test and verify it PASSES**

```bash
cargo test -p taliesin-core --lib the_readme_does_not_advertise_withdrawn_constructs
```

Expected: **PASS**.

- [ ] **Step 5: Verify by mutation**

**Do NOT use `git checkout README.md` to undo the mutation.** Step 3's fixes are not
committed yet, so a checkout would revert them to the original false claims and silently
throw the work away. Use a backup copy:

```bash
cp README.md /tmp/README.fixed.md
sed -i 's/(print stylesheet)\. Light and dark/(print stylesheet), themes (light\/dark + custom). Light and dark/' README.md
cargo test -p taliesin-core --lib the_readme_does_not_advertise_withdrawn_constructs 2>&1 | tail -5
cp /tmp/README.fixed.md README.md
cargo test -p taliesin-core --lib the_readme_does_not_advertise_withdrawn_constructs
```

Expected: the first test run FAILS naming `"+ custom"`, and the second PASSES. If the `sed`
matches nothing (check with `git diff --stat README.md` between the two), the mutation did
not happen and the test proved nothing: edit the file by hand instead and repeat.

- [ ] **Step 6: Audit the rest of the feature list against the validator consts**

For every remaining bullet in that README section, confirm the construct still exists.
Read the **validator consts**, never `vocab.rs` (it is the offered-completions subset and
under-reports).

```bash
sed -n '/^## /,$p' README.md | sed -n '1,60p'   # re-read the section
git grep -n "XREF_LABELS: " -- crates/core/src/cite/render.rs
sed -n '30,45p' crates/core/src/cite/render.rs   # the 5 xref prefixes
git grep -n "DIV_FEATURE_CLASSES: " -- crates/core/src/render/validate.rs
git grep -n "KNOWN_KEYS: " -- crates/core/src/frontmatter.rs
```

Check specifically: the five `@fig-`/`@eq-`/`@lst-`/`@tbl-`/`@sec-` prefixes the README
names must all appear in `XREF_LABELS`; `listing:` and `hero:` must appear in
`KNOWN_KEYS`; the client-side accessibility audit bullet must match what
`crates/core/src/diagnostics/` actually implements (CLAUDE.md records a11y as a
**server-side static validator**, so "advisory client-side accessibility audit" may be
wrong about *where* it runs). Add a `WITHDRAWN_README_PHRASES` entry for anything else you
find false, re-run Step 2's failure check for it, then fix it.

- [ ] **Step 7: Re-run the portability census and update both consumers**

The Phase 0 to 3 cuts changed lines under `corpus/`, and the census runs only in
`gates.sh`, not in `cargo test` and not in the pre-push hook, so the published figures are
stale right now.

```bash
python3 tools/portability-census.py
```

Copy the printed figures into **both** `README.md` (the "Your source stays yours" bullet,
currently "81 documents / 7,095 lines ... 6.7%") and
`docs/guide/using/choosing.tmd`. Then:

```bash
python3 tools/portability-census.py --verify
```

Expected: exit 0.

- [ ] **Step 8: Format, run the affected suites, commit**

```bash
cargo fmt --all
cargo test -p taliesin-core --lib
```

Expected: all pass.

```bash
git add crates/core/src/render/tests.rs README.md docs/guide/using/choosing.tmd
git commit -m "fix(readme): stop advertising two constructs the tool deleted, and pin it

README advertised 'themes (light/dark + custom)' (the theme: key was cut
2026-08-17; there is no author theme control at all) and 'attributed .btn
links' (link attribute blocks were cut in 093d8b0c). Both had been false for
weeks.

The pin is the point. the_reference_page_documents_every_known_key walks
KNOWN_KEYS into the guide and could never have caught either, because neither
construct is a front-matter key. WITHDRAWN_README_PHRASES covers the prose
that no structural gate reaches. Mutation-checked in both directions.

Also re-runs tools/portability-census.py, whose figures the Phase 0-3 cuts
staled: the census runs only in gates.sh, so cargo test stayed green while
README and choosing.tmd were wrong."
```

---

### Task 4: The maintenance stance

**Files:**
- Modify: `README.md` (the "Before you adopt it" third bullet; a new "Project status"
  section)
- Modify: `CHANGELOG.md:1-9` (the preamble and `[Unreleased]`)
- Read only: `CONTRIBUTING.md`

**Interfaces:**
- Consumes: workspace version `1.0.0` from Task 2.
- Produces: the README "Project status" section, referenced by Task 5's issue-template
  `config.yml`.

- [ ] **Step 1: Rewrite the pre-1.0 bullet**

Find the bullet beginning `- **One maintainer, pre-1.0.**` and replace it with:

```markdown
- **One maintainer, and the scope is closed.** No support contract, no release cadence, no
  bus factor above one. 1.0 means the feature set is final for this tool's one use case,
  not that a team stands behind it. What that risk is bounded by: Markdown source you
  already hold, built HTML with no dependency on this tool, and an AGPL-3.0 licence that
  makes a fork always available.
```

- [ ] **Step 2: Add the Project status section**

Insert immediately after the "Before you adopt it" section, before "## Architecture (at a
glance)":

```markdown
## Project status

**Taliesin 1.0 is feature-complete for its one use case:** rendering `.tmd` to HTML for one
author's writing workflow. The scope is deliberately closed.

- **Bug reports are welcome.** Something rendering wrongly, a crash, a diagnostic that
  fires on valid source: please open an issue.
- **Feature requests are closed by design**, not by backlog order. The tool is built around
  subtraction, and a 2026-08 campaign cut roughly 40% of the tree to get here. Adding an
  output format (PDF, LaTeX, Word, ePub) is out of scope permanently; HTML is the only
  target.
- **Security reports go through `SECURITY.md`**, privately, not as a public issue.

`CONTRIBUTING.md` has the scope rules in full. If you want something the tool will not do,
the AGPL licence means forking is always available and is often the honest answer.
```

- [ ] **Step 3: Rewrite the CHANGELOG preamble**

Replace the paragraph beginning `All notable changes to Taliesin are recorded here.` with:

```markdown
All notable changes to Taliesin are recorded here. From 1.0 this project follows
[semantic versioning](https://semver.org/): a breaking change to the load-bearing
invariants (content-hash block model, click-to-source, single editing surface, HTML-only
output) or to the CLI's six verbs needs a major version. Before 1.0 the policy was looser
and minor versions carried breaking changes; the 0.x entries below were written under it.
```

- [ ] **Step 4: Write the 1.0.0 entry**

Replace the `## [Unreleased]` line with:

```markdown
## [Unreleased]

## [1.0.0] - 2026-08-20

The scope is closed. This is the first public release; the code is unchanged from 0.3.0
except for the cuts listed below, and 1.0 is a statement about the feature set being final
rather than a feature release.

### Changed

- **The project is public**, and the version says the scope is closed. Feature requests are
  closed by design from here; bug reports are welcome. See "Project status" in `README.md`.
- The CLI is **six subcommands**: `preview`, `build`, `init`, `doctor`, `lsp`, `help`.

### Removed

Continuing the 2026-08 reduction campaign, from the final feature audit:

- The `lang:` and `csl:` front-matter keys, `page-layout: full`, and link attribute blocks
  (`[text](url){.class}`).
- The `theme:` key and all author theme control. Both palettes always ship and the reader's
  device selects one at paint.
- The missing-local-video lint and the uncited-entry lint.
- Five VS Code companion features: the first-kernel-failure doctor hint, the build/check
  tasks and their Problems-panel matchers, the Diagnose Setup command, the Get Started
  walkthrough, and the bundled `_site.yml` schema copy. The terminal path replaces the task
  provider: every located diagnostic line in the integrated terminal is clickable.

### Fixed

- `README.md` no longer advertises constructs the tool deleted, and a test now keeps it
  that way.
```

- [ ] **Step 5: Check CONTRIBUTING for contradictions**

```bash
grep -n -i "pre-1.0\|version\|roadmap\|feature request" CONTRIBUTING.md
```

Its opening already says feature requests adding an output format are out of scope by
design, which is consistent. Fix anything that contradicts the stance; change nothing else.

- [ ] **Step 6: Verify the README pin still passes, then commit**

```bash
cargo test -p taliesin-core --lib the_readme_does_not_advertise_withdrawn_constructs
git add README.md CHANGELOG.md CONTRIBUTING.md
git commit -m "docs: the 1.0 maintenance stance

README gains a Project status section (feature-complete for one use case,
bug reports welcome, feature requests closed by design) and the 'One
maintainer, pre-1.0' bullet becomes 'One maintainer, and the scope is
closed', keeping the three bounding facts that made the original honest.

CHANGELOG's preamble moves from loose-semver-while-pre-1.0 to ordinary
semver, and gains the 1.0.0 entry."
```

---

### Task 5: Launch presentation

**Files:**
- Create: `CODE_OF_CONDUCT.md`
- Create: `.github/ISSUE_TEMPLATE/bug_report.yml`
- Create: `.github/ISSUE_TEMPLATE/config.yml`
- Read only: `SECURITY.md`

**Interfaces:**
- Consumes: the README "Project status" section from Task 4.
- Produces: nothing later tasks read.

- [ ] **Step 1: Write CODE_OF_CONDUCT.md**

Use Contributor Covenant 2.1 verbatim from https://www.contributor-covenant.org/version/2/1/code_of_conduct/,
with the enforcement contact filled in as `andreas.bogossian9@gmail.com`. That address is
already published at `SECURITY.md:26` as the fallback vulnerability-report address, so this
reuses a published decision rather than making a new disclosure.

- [ ] **Step 2: Write the bug report form**

Create `.github/ISSUE_TEMPLATE/bug_report.yml`:

```yaml
name: Bug report
description: Something renders wrongly, crashes, or reports a problem that is not one.
labels: ["bug"]
body:
  - type: markdown
    attributes:
      value: |
        Taliesin is a single-maintainer project with a closed scope. Bug reports are
        welcome; feature requests are not (see CONTRIBUTING.md).
  - type: textarea
    id: what-happened
    attributes:
      label: What happened, and what did you expect instead?
    validations:
      required: true
  - type: textarea
    id: source
    attributes:
      label: The smallest `.tmd` source that reproduces it
      render: markdown
    validations:
      required: true
  - type: input
    id: command
    attributes:
      label: The exact command you ran
      placeholder: taliesin build post.tmd
    validations:
      required: true
  - type: textarea
    id: doctor
    attributes:
      label: Output of `taliesin doctor --format json`
      render: json
    validations:
      required: true
  - type: input
    id: version
    attributes:
      label: Version (`taliesin --version`) and platform
      placeholder: 1.0.0, Linux x86-64
    validations:
      required: true
```

- [ ] **Step 3: Write the template chooser config**

Create `.github/ISSUE_TEMPLATE/config.yml`:

```yaml
blank_issues_enabled: false
contact_links:
  - name: Feature requests are closed by design
    url: https://github.com/AJBogo9/taliesin/blob/main/CONTRIBUTING.md
    about: >-
      Taliesin 1.0 is feature-complete for its one use case and the scope is closed.
      CONTRIBUTING.md explains what that rules out and why. Forking is always available
      under the AGPL.
  - name: Report a security vulnerability privately
    url: https://github.com/AJBogo9/taliesin/security/advisories/new
    about: Do not open a public issue for a suspected vulnerability. See SECURITY.md.
```

- [ ] **Step 4: Validate the YAML parses**

```bash
python3 -c "import yaml,sys; [yaml.safe_load(open(p)) for p in ['.github/ISSUE_TEMPLATE/bug_report.yml','.github/ISSUE_TEMPLATE/config.yml']]; print('both parse')"
```

Expected: `both parse`.

- [ ] **Step 5: Read SECURITY.md end to end for public coherence**

```bash
cat SECURITY.md
```

Confirm: the reporting instructions work for a public repo, the threat model section does
not reference anything cut, and the supported-versions statement is consistent with 1.0.
Fix only what is incoherent.

- [ ] **Step 6: Set homepageUrl**

```bash
gh repo edit AJBogo9/taliesin --homepage "https://taliesin.sh"
gh repo view AJBogo9/taliesin --json homepageUrl
```

Expected: `{"homepageUrl":"https://taliesin.sh"}`. This is a repository setting, invisible
while private and reversible. It is **not** a public act.

- [ ] **Step 7: Commit**

```bash
git add CODE_OF_CONDUCT.md .github/ISSUE_TEMPLATE/ SECURITY.md
git commit -m "docs: code of conduct and issue templates

Contributor Covenant 2.1, contact reusing the address SECURITY.md:26 already
publishes. The issue templates are the mechanism that makes the 1.0
maintenance stance cheap to keep: blank issues off, a bug form that demands
a reproduction and doctor output, and a chooser link that says feature
requests are closed by design before anyone types one."
```

---

### Task 6: Relocate docs/superpowers into notes/

**Files:**
- Move: `docs/superpowers/plans/*.md` and `docs/superpowers/specs/*.md` (6 tracked files)
  into `notes/superpowers/`
- Modify: `CLAUDE.md` (any statement that the directory is gone)
- Modify: `notes/backlog.md` (item 100's parenthetical about R6-1)

**Interfaces:**
- Consumes: nothing
- Produces: `notes/superpowers/`, which Task 8's enumeration must account for.

**Background.** R6-1 deleted `docs/superpowers/` on 2026-08-09 because plans and specs
under `docs/` read as "the manual" to a visitor. The reason was **placement**, not content.
The directory has since regrown to 6 tracked files and 336 KB, and both `CLAUDE.md` and
`notes/backlog.md` still record it as gone. Moving honours both rulings at once.

- [ ] **Step 1: Confirm the current contents**

```bash
git ls-files docs/superpowers
du -sh docs/superpowers
```

Expected: 6 files, roughly 336K.

- [ ] **Step 2: Move them**

```bash
mkdir -p notes/superpowers
git mv docs/superpowers/plans notes/superpowers/plans
git mv docs/superpowers/specs notes/superpowers/specs
rmdir docs/superpowers 2>/dev/null || true
ls docs/
```

Expected: `docs/` now holds only `guide` and `internals`.

- [ ] **Step 3: Find and fix every claim that the directory is gone**

```bash
git grep -n "docs/superpowers" -- . ':!notes/superpowers'
```

Update each hit so it reads as relocated rather than deleted. In `CLAUDE.md` and
`notes/backlog.md`, the phrasing to correct is any variant of "deleted with
`docs/superpowers/` on 2026-08-09". Note that the files ARE still deleted from `docs/`;
what changed is that they now live under `notes/`.

- [ ] **Step 4: Verify no test or gate referenced the old path**

```bash
cargo test --workspace --no-run 2>&1 | tail -5
git grep -n "docs/superpowers" -- crates/ editor/ web-client/ tools/ .githooks/
```

Expected: the build succeeds and the grep returns nothing.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs(notes): relocate docs/superpowers into notes/, rather than deleting it again

R6-1 deleted it on 2026-08-09 because plans and specs under docs/ read as
'the manual' to a visitor. The reason was placement, not content, and it has
since regrown to 6 files and 336K while CLAUDE.md and the backlog both still
record it as gone. Moving honours both rulings: docs/ holds only the two
books, and the working material sits with the rest of the working material."
```

---

### Task 7: The purge-coupled repairs

**Files:**
- Modify: `notes/AUDITS.md` (five Markdown links)
- Modify: `notes/README.md` (the index row at ~:17; the stale opening)
- Modify: `notes/backlog.md` (~:336)
- Modify: `notes/2026-07-28-deck-exemption-audit.md` (~:85)
- Modify: `.githooks/pre-push:45-46`

**Interfaces:**
- Consumes: nothing
- Produces: a tree in which no surviving file links to a file the Task 9 purge removes.

**Background.** These repairs were deliberately deferred on 2026-08-03 because they are
*coupled to the purge*: removing an index row that points at a file still in the tree would
have made the documentation wrong. The purge is now happening, so they land.

- [ ] **Step 1: Find every surviving reference to the purge set**

```bash
git grep -n "STARTUP-PLAN\|FUNDING-RESEARCH\|2026-07-18-pmf-audit\|2026-07-27-due-diligence-audit\|2026-07-28-demand-positioning-audit\|2026-07-28-launch-critique\|2026-07-27-adoption-friction-audit"
```

Work from this output, not from the line numbers above.

- [ ] **Step 2: Remove the dead links**

For each hit that is a Markdown link or an index-table row pointing at a purge-set file:
delete the row, or unlink the text while keeping the sentence readable. Do not invent
replacement destinations. `.githooks/pre-push:45-46` is a **tripwire regex, not a
reference**: leave those two lines for Step 4.

- [ ] **Step 3: Refresh the notes/README.md opening**

Its table currently says `CUT-PROGRESS.md` is "**START HERE while the scope reduction is
running.**" The reduction is complete. Change that cell to say it is the durable record of
the completed 2026-08 campaign. Also remove the index row naming the two money documents.

- [ ] **Step 4: Extend the pre-push purge tripwire**

Read `.githooks/pre-push:40-55` first. The regex currently reads:

```sh
purge_re='(^|/)(STARTUP-PLAN|FUNDING-RESEARCH)\.md$'
purge_re+='|(pmf-audit|due-diligence-audit|demand-positioning-audit|launch-critique|adoption-friction-audit)\.md$'
```

Add a third line so the tripwire also refuses the third-party-rights path:

```sh
purge_re+='|(^|/)corpus/bayesian-website/'
```

- [ ] **Step 5: Verify the tripwire matches what it should and nothing else**

```bash
printf '%s\n' "notes/STARTUP-PLAN.md" "corpus/bayesian-website/index.qmd" \
  "corpus/single-page-report/index.tmd" "notes/backlog.md" |
  while read -r f; do
    if echo "$f" | grep -Eq "$(sed -n "s/^purge_re='\(.*\)'$/\1/p;s/^purge_re+='\(.*\)'$/\1/p" .githooks/pre-push | paste -sd'|')"; then
      echo "MATCH   $f"; else echo "no      $f"; fi
  done
```

Expected: MATCH for the first two, `no` for the last two.

- [ ] **Step 6: Confirm no dead links survive, then commit**

```bash
git grep -n "STARTUP-PLAN\|FUNDING-RESEARCH\|2026-07-18-pmf-audit\|2026-07-27-due-diligence-audit\|2026-07-28-demand-positioning-audit\|2026-07-28-launch-critique\|2026-07-27-adoption-friction-audit" -- . ':!.githooks/pre-push'
```

Expected: only prose mentions that do not link, or nothing at all.

```bash
git add -A
git commit -m "docs(notes): repair the links the purge is about to break

The D2 Class-A repairs, deferred 2026-08-03 because they are coupled to the
purge: removing an index row that points at a file still in the tree makes
the documentation wrong, so they had to wait for the purge to be real.

Also extends .githooks/pre-push's purge tripwire to corpus/bayesian-website,
which is a third party's rights and not merely private."
```

---

### Task 8: Re-run the D1/D2 enumeration against the current tree

**Files:**
- Create: `notes/2026-08-20-purge-enumeration.md`

**Interfaces:**
- Consumes: the repaired tree from Task 7.
- Produces: `notes/2026-08-20-purge-enumeration.md`, containing the exact `--path` and
  `--replace-text` arguments Task 9 uses. Task 9 must not invent arguments of its own.

**Background.** The flip audit's D1 and D2 tables are reads of the 2026-07-28 tree plus a
2026-08-03 remediation. The tree has moved materially since: `docs/internals/repository.tmd`
no longer exists, `corpus/bayesian-website/` was replaced at HEAD, `docs/superpowers/` died
and regrew and has now moved, and the whole cut campaign landed. One gap in those tables is
already known (`corpus/bayesian-website/` was never in the purge list). **Assume there are
others.**

- [ ] **Step 1: Enumerate every historical path spelling for each purge-set document**

```bash
for n in STARTUP-PLAN FUNDING-RESEARCH 2026-07-18-pmf-audit 2026-07-27-due-diligence-audit \
         2026-07-28-demand-positioning-audit 2026-07-28-launch-critique \
         2026-07-27-adoption-friction-audit todo bayesian-website; do
  echo "### $n"
  git log --all --name-only --format="" | sort -u | grep -i "$n" || echo "  (none)"
done
```

Record every distinct spelling. **Both the `notes/` and the pre-move root spellings must be
named, or the pre-move blobs survive the rewrite.**

- [ ] **Step 2: Confirm the bayesian directory's full file list**

```bash
git log --all --diff-filter=A --name-only --format="" -- corpus/bayesian-website | sort -u
git log --oneline --all -- corpus/bayesian-website
```

Expected: 21 files across 7 commits, including `project_instructions.md` (the university's
assignment brief) and `index.qmd` with seven `subsections/_*.qmd`.

- [ ] **Step 3: Find content restatements that survive in files the purge does NOT remove**

```bash
git log --all -S "revenue" --oneline | head -20
git log --all -S "pricing" --oneline | head -20
git log --all -S "monetis" --oneline | head -20
git log --all -S "monetiz" --oneline | head -20
git log --all -S "seed round" --oneline | head -20
git log --all -S "ARR" --oneline | head -20
```

For each hit, determine whether the file is already in the `--path` purge set. If it is,
nothing more is needed. If it is not, it needs a `--replace-text` entry. Record the exact
strings.

- [ ] **Step 4: Find commit subjects naming the purged documents**

```bash
git log --all --format="%H %s" | grep -iE "startup-plan|funding-research|pmf-audit|due-diligence|demand-positioning|launch-critique|adoption-friction|bayesian"
```

Record each. These need a message callback or a `--replace-text` entry covering messages.

- [ ] **Step 5: Write the enumeration document**

`notes/2026-08-20-purge-enumeration.md` must contain, ready to paste:

1. The complete `--path` list for **Rewrite A** (archive): every spelling of
   `corpus/bayesian-website` only.
2. The complete `--path` list for **Rewrite B** (public): Rewrite A's list plus every
   spelling of the seven money documents plus `todo.md`.
3. The `--replace-text` file contents for Rewrite B, if Step 3 or Step 4 found anything.
4. The count of commits each path appears in, so Task 9 can verify the rewrite did work.
5. An explicit statement of what was searched, so an empty result is distinguishable from
   an unrun search. (D4 and D8 of the original audit did this and it is why their empty
   results are trustworthy.)

- [ ] **Step 6: Commit**

```bash
git add notes/2026-08-20-purge-enumeration.md
git commit -m "docs(notes): re-enumerate the purge set against the current tree

The 2026-07-28 audit's D1/D2 tables are reads of a tree that has since moved
a long way, and one gap in them is already known: corpus/bayesian-website
was never listed, although it is the one path in the whole set that is a
third party's rights rather than the author's own privacy. This is the
list Task 9 pastes from; it invents no arguments of its own."
```

---

### Task 9: Both filter-repo dry runs, on clones, verified

**Files:**
- Create (outside the repository): `/tmp/taliesin-rewrite-a.git/`, `/tmp/taliesin-rewrite-b.git/`
  (mirror clones) and `/tmp/taliesin-rewrite-b/` (a working clone made from the second)
- Create: `notes/2026-08-20-rewrite-dry-run.md`

**Interfaces:**
- Consumes: `notes/2026-08-20-purge-enumeration.md` from Task 8.
- Produces: `notes/2026-08-20-rewrite-dry-run.md`, quoted by Task 11's dossier.

**THE HARD RULE FOR THIS TASK: `git filter-repo` runs on a CLONE. Never on the working
repository. Never with `--force` on the working repository. If you find yourself typing
`filter-repo` while the working directory is the project, stop.**

- [ ] **Step 1: Verify the tool**

```bash
~/.local/bin/git-filter-repo --version
```

Expected: `2.47.0` or later. If this fails with "command not found", the pipx symlinks have
dangled after a snap update; reinstall before continuing.

- [ ] **Step 2: Make the two clones**

```bash
rm -rf /tmp/taliesin-rewrite-a /tmp/taliesin-rewrite-a.git \
       /tmp/taliesin-rewrite-b /tmp/taliesin-rewrite-b.git
git clone --no-local --mirror . /tmp/taliesin-rewrite-a.git
git clone --no-local --mirror . /tmp/taliesin-rewrite-b.git
```

`--no-local` forces a real object copy rather than hardlinks, so a rewrite in the clone can
never reach back into the working repository's object store. This is not optional.

**If `filter-repo` refuses with "does not look like a fresh clone",** add `--force`. That is
safe *here and only here*, because the target is a throwaway mirror under `/tmp`. It would
never be safe in the working repository.

- [ ] **Step 3: Run Rewrite A (archive: the bayesian directory only)**

Paste the `--path` arguments from `notes/2026-08-20-purge-enumeration.md` section 1. The
shape is:

```bash
cd /tmp/taliesin-rewrite-a.git
~/.local/bin/git-filter-repo --invert-paths \
  --path corpus/bayesian-website/ \
  --path-glob 'corpus/bayesian-website/*'
```

Add any additional spellings Task 8 found.

- [ ] **Step 4: Verify Rewrite A**

```bash
cd /tmp/taliesin-rewrite-a.git
git log --all --oneline -- corpus/bayesian-website | wc -l
git log --all --name-only --format="" | sort -u | grep -c bayesian
git rev-list --count --all
```

Expected: the first two print `0`. The third should be close to 2,138 (filter-repo prunes
commits that become empty, so a small drop is expected and should be recorded, not
alarmed at).

- [ ] **Step 5: Run Rewrite B (public: everything)**

Paste the `--path` arguments from `notes/2026-08-20-purge-enumeration.md` section 2, and
the `--replace-text` file from section 3 if there is one.

```bash
cd /tmp/taliesin-rewrite-b.git
~/.local/bin/git-filter-repo --invert-paths \
  --path corpus/bayesian-website/ \
  --path notes/STARTUP-PLAN.md --path STARTUP-PLAN.md \
  --path notes/FUNDING-RESEARCH.md --path FUNDING-RESEARCH.md \
  --path notes/2026-07-18-pmf-audit.md \
  --path notes/2026-07-27-due-diligence-audit.md \
  --path notes/2026-07-28-demand-positioning-audit.md \
  --path notes/2026-07-28-launch-critique.md \
  --path notes/2026-07-27-adoption-friction-audit.md \
  --path todo.md
```

**Use Task 8's list, not this sketch, if the two disagree.**

- [ ] **Step 6: Verify Rewrite B**

```bash
cd /tmp/taliesin-rewrite-b.git
for p in corpus/bayesian-website notes/STARTUP-PLAN.md STARTUP-PLAN.md \
         notes/FUNDING-RESEARCH.md FUNDING-RESEARCH.md todo.md \
         notes/2026-07-18-pmf-audit.md notes/2026-07-27-due-diligence-audit.md \
         notes/2026-07-28-demand-positioning-audit.md notes/2026-07-28-launch-critique.md \
         notes/2026-07-27-adoption-friction-audit.md; do
  printf "%-55s %s\n" "$p" "$(git log --all --oneline -- "$p" | wc -l)"
done
```

Expected: every count is `0`.

- [ ] **Step 7: Verify the rewritten public history still builds and passes the gates**

```bash
rm -rf /tmp/taliesin-rewrite-b && git clone /tmp/taliesin-rewrite-b.git /tmp/taliesin-rewrite-b
cd /tmp/taliesin-rewrite-b
git config core.hooksPath .githooks
python3 -m venv .venv && ./.venv/bin/pip -q install ipykernel
export TALIESIN_PYTHON="$PWD/.venv/bin/python"
./tools/gates.sh
```

Expected: the script's own verdict line says PASSED with every gate run. **Quote that line
verbatim in the dry-run document; do not paraphrase the count.**

- [ ] **Step 8: Confirm the authorship record survived**

```bash
cd /tmp/taliesin-rewrite-b
git log --format="%an" | sort -u
git rev-list --count HEAD
git log --oneline | tail -3
```

Expected: one author, a commit count close to 2,138, and the earliest commits intact. The
whole point of publishing the history is that this record travels.

- [ ] **Step 9: Write the dry-run document and commit**

`notes/2026-08-20-rewrite-dry-run.md` records: the exact commands run, the before and after
commit counts for both rewrites, the Step 6 table showing every purged path at zero, the
verbatim `gates.sh` verdict line from Step 7, and anything that surprised you.

```bash
cd /home/bogo/Documents/personal/taliesin
git add notes/2026-08-20-rewrite-dry-run.md
git commit -m "docs(notes): both history rewrites, rehearsed on throwaway clones

Rewrite A (archive, bayesian only) and Rewrite B (public, everything)
executed end to end on mirror clones under /tmp, with every purged path
verified to zero commits and the rewritten public history cloned, built and
gated. Nothing was pushed and the working repository was never touched by
filter-repo."
```

---

### Task 10: Final verification

**Files:**
- Create: `notes/2026-08-20-final-verification.md`
- Create: screenshots under the session scratchpad

**Interfaces:**
- Consumes: the finished tree from Tasks 1 to 9.
- Produces: `notes/2026-08-20-final-verification.md`, quoted by Task 11.

- [ ] **Step 1: Run the full gate script**

```bash
export TALIESIN_PYTHON="$PWD/.venv/bin/python"
./tools/gates.sh 2>&1 | tee /tmp/gates-final.log
tail -20 /tmp/gates-final.log
```

**Take the gate count from the script's own verdict line.** If it is not green, fix and
re-run; do not proceed with a red gate.

- [ ] **Step 2: Run the publish check**

```bash
./tools/publish.sh --check
```

Expected: all four projects build with no problems and nothing is deployed.

- [ ] **Step 3: Browser-verify all four projects at three viewports**

For each of `site`, `docs/guide`, `docs/internals`, `gallery`:

```bash
cargo run -q -p taliesin-server -- preview <dir> 4388
```

Then, through the chrome-devtools MCP: navigate to `http://127.0.0.1:4388`, resize to
390x844, 1440x900 and **900x1440**, screenshot each, and read the console for errors. The
900x1440 portrait band is where layout defects show and is the one usually forgotten.

Record any defect found. A layout defect is **not** automatically a blocker; note it and
let the author decide.

- [ ] **Step 4: Re-run both workflows against the final tree**

Repeat Task 1 steps 2 through 7 against the current `item-100-publication-prep` tip. Task
1's earlier run was against a tree that has since changed; the flip inherits whatever
exists now.

- [ ] **Step 5: Confirm the pin still holds and the census still verifies**

```bash
cargo test -p taliesin-core --lib the_readme_does_not_advertise_withdrawn_constructs
python3 tools/portability-census.py --verify
cargo run -q -p taliesin-server -- --version
```

Expected: PASS, exit 0, and `1.0.0`.

- [ ] **Step 6: Write the verification record and commit**

`notes/2026-08-20-final-verification.md` records each check, the command, and the **quoted**
output or exit code. Name explicitly what was NOT verified.

```bash
git add notes/2026-08-20-final-verification.md
git commit -m "docs(notes): final verification of the 1.0 tree

Gates, publish --check, all four projects browser-verified at 390x844,
1440x900 and 900x1440, both workflows re-run against the final tree, the
README pin and the portability census re-checked. Every number quoted from
the instrument that produced it."
```

---

### Task 11: The go/no-go dossier

**Files:**
- Create: `notes/2026-08-20-flip-go-no-go.md`

**Interfaces:**
- Consumes: the four record documents from Tasks 1, 8, 9 and 10.
- Produces: the document the author reads before giving or withholding the Phase 2
  instruction.

- [ ] **Step 1: Write the dossier**

It must contain, in this order:

1. **The verdict line.** Green or not green, in one sentence, at the top.
2. **What was verified**, each with its command and quoted output: gates, publish check,
   browser viewports, both workflow runs, both rewrite dry runs, the README pin, the census.
3. **What was NOT verified**, explicitly. At minimum: macOS binaries were built in CI but
   never executed on macOS hardware; the four sites were built but never deployed, so DNS
   and Cloudflare Pages custom-domain binding are unexercised; no `v*` tag has ever fired
   `release.yml` on a real tag.
4. **The exact Phase 2 sequence**, copy-pasteable, in the order fixed by the spec's section
   4.6:

```
1. git bundle create ~/Documents/personal/taliesin-private/taliesin-full-2026-08-20.bundle --all
   Verify: clone FROM the bundle into a temp dir, diff its tip tree against main.
   THIS BUNDLE IS THE ONLY COMPLETE BACKUP THAT WILL EXIST. Verify before destroying anything.
2. gh repo rename taliesin-old            (frees the name)
3. gh repo create AJBogo9/taliesin-private-archive --private   ; push Rewrite A ; verify
4. gh repo create AJBogo9/taliesin --public                    ; push Rewrite B ; verify
5. Verify on BOTH new remotes: git log --all -- corpus/bayesian-website returns nothing.
   On the public one, the same for the money set and todo.md.
6. gh repo delete AJBogo9/taliesin-old    (LAST, only after both replacements verify)
```

5. **What happens immediately after the flip, and in what order:** dispatch `ci.yml` on the
   public repo and read it; push `v1.0.0` and verify the three release assets exist; then
   items 149 and 170 (deploy the sites, DNS).
6. **The open risks**, each with its mitigation.

- [ ] **Step 2: Verify every claim in the dossier is sourced**

Re-read it against the four record documents. Every number must be quoted from an
instrument, with the command that produced it. Delete or mark any claim you cannot source.

- [ ] **Step 3: Commit**

```bash
git add notes/2026-08-20-flip-go-no-go.md
git commit -m "docs(notes): the flip go/no-go dossier

Everything verified with its command and quoted output, everything NOT
verified named explicitly, and the exact Phase 2 sequence in the order the
spec fixes: bundle and verify first, delete the original repository last."
```

- [ ] **Step 4: Report to the author and STOP**

Summarise: the verdict, what is green, what is not, and the single instruction needed to
proceed. **Do not execute Phase 2.** It requires a separate explicit instruction.

---

## What this plan deliberately does not do

- Phase 2 of backlog item 100 (the flip itself).
- Item 148's `v1.0.0` tag: `release.yml` produces nothing while the repo is private.
- Item 170's deploy, DNS, and the `live-edit-hero-demo` clip.
- Package managers: crates.io, Homebrew and Nix are a decision after the tag. `cargo
  publish` would reject this workspace as-is (`taliesin-core` is a path dependency with no
  `version`; `keywords`, `categories`, `readme`, `homepage` and `documentation` are blank
  in every manifest).
- C8 and X1 from the feature-audit backlog: dropped per the spec's D-4.
- Any further feature work. The scope is frozen.
