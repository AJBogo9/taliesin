# Shed Quarto Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Taliesin a fully separate tool from Quarto: `.tmd`-only input, `deck`/`define()` the only spellings, no migration on-ramps, and no user-facing mention of Quarto, while keeping all Markdown/Pandoc syntax.

**Architecture:** Three sequential phases on the `shed-quarto` branch, each a green, bisectable commit set. Phase 1 is a behavior-neutral mechanical migration of the corpus + docs + tests to `.tmd` *while `ext.rs` still accepts both extensions*. Phase 2 flips the acceptance and drops the aliases (the one behavioral break). Phase 3 removes the migration on-ramps and rewords user-facing prose.

**Tech Stack:** Rust (edition 2024, workspace `taliesin-core` + `taliesin-server`), the in-repo `.tmd` corpus + two dogfooded docs books, `taliesin check` as the link/validation gate.

**Spec:** `docs/superpowers/specs/2026-07-03-shed-quarto-design.md` (read it; this plan implements it).

## Global Constraints

Copied from the spec + `CLAUDE.md`; every task implicitly includes these.

- **HTML is the only output target.** No new formats. Keep all Markdown/Pandoc syntax (`:::`, `#|`/`//|`/`%%|`, `@fig-`/`@sec-`, `[@cite]`, `{{< >}}`, YAML) exactly as-is.
- **Single read-only editing surface.** No preview-to-source writes; nothing in this change touches that.
- **Green after every phase:** `cargo test -p taliesin-core` and `cargo test -p taliesin-server` pass; `cargo fmt` clean (a PostToolUse hook runs rustfmt on every edited `.rs`); `cargo clippy` clean; `cd web-client && npx -y -p typescript tsc -p jsconfig.json` clean.
- **`taliesin check` clean** on `corpus/` projects + `docs/guide` + `docs/internals` after Phase 1 and Phase 3 (the broken-link safety net).
- **DO NOT TOUCH (the spec's §5 keeps):** the `qmd-define` script-type wire (`kernel.rs`, `freeze.rs`, `qmd-js.js`); `freeze.rs` `FORMAT_VERSION`; the `data-block-id` / `data-sourcepos` / `data-source-file` attribute names; internal test-function names (e.g. `dropped_quarto_keys_now_warn`); `THIRD_PARTY.md` / licensing; the `tech_blog` test asserting no `quarto-ojs-runtime` / `window._ojs`; `notes/DROP-QUARTO.md`, `notes/BEYOND-QUARTO.md`, the planning specs, `CLAUDE.md`; structural dir names `_site.yml` / `_freeze/` / `_extensions/` / `_book/`; the internal enum name `DocFormat::Reveal` and fn `is_reveal_format` (internal, may keep their names).
- **Binary invocation in commands:** build once and use `cargo run -p taliesin-server --` (or the `taliesin` launcher on PATH) for `check`.

## A note on task altitude

This change is ~403 `.qmd` occurrences across 44 `.rs` files, 105 file renames, and prose across ~15 docs. The **mechanical** tasks (renames, link rewrites, fixture-path updates) are *refactors verified by the existing corpus + test net*, exactly the project's "corpus is the regression net" convention — they are scripted, then gated by `cargo test` + `taliesin check`, not TDD'd per occurrence. The **behavioral** tasks (dropping acceptance, the aliases, the did-you-mean, the on-ramp removal) introduce or change behavior and are written test-first. Each task says which it is.

---

## Phase 1 — Mechanical `.tmd` migration (ext.rs still dual-accepts; zero behavior change)

### Task 1: Rename corpus + docs to `.tmd` and rewrite intra-project references

**Type:** mechanical refactor (gate = corpus tests + `check`).

**Files:**
- Rename: every tracked `*.qmd` (105 files under `corpus/`, `docs/guide/`, `docs/internals/`) → `*.tmd`.
- Modify (link/target rewrite): the renamed `.tmd` files' internal markdown links `](…​.qmd)`, `{{< include … .qmd >}}` / `{{< embed … .qmd >}}` targets, and prose mentions of the `.qmd` extension.
- Modify: the 5 `_site.yml` files (`corpus/tech-blog/_site.yml`, `corpus/demo-book/_site.yml`, `corpus/bayesian-website/_site.yml`, `docs/guide/_site.yml`, `docs/internals/_site.yml`) — chapter/`href:`/`file:` entries `.qmd`→`.tmd`.
- Modify: Rust tests that reference **real** corpus/docs paths (found by running the suite): `crates/core/tests/corpus.rs`, `crates/core/tests/tech_blog.rs`, `crates/core/tests/config.rs`, `crates/core/tests/include_relative_base.rs`, `crates/core/tests/loose_deck.rs`, `crates/server/tests/parallel_build_determinism.rs`, and any others the failing suite points at.

**Interfaces:** No code interfaces change. `ext.rs` `ACCEPTED_SOURCE_EXTS` still `&["tmd", "qmd"]`, so both spellings resolve throughout Phase 1.

- [ ] **Step 1: Baseline green.** Confirm the starting tree is green so later failures are attributable.

```bash
cargo test -p taliesin-core -q && cargo test -p taliesin-server -q
```
Expected: PASS.

- [ ] **Step 2: Rename all tracked `.qmd` → `.tmd`.**

```bash
for f in $(git ls-files '*.qmd'); do git mv "$f" "${f%.qmd}.tmd"; done
git ls-files '*.qmd' | wc -l   # expect 0
git ls-files '*.tmd' | wc -l   # expect 105
```

- [ ] **Step 3: Rewrite `.qmd` → `.tmd` inside the migrated docs/corpus files.** Only the renamed `.tmd` files (not `.rs`, not `notes/`, not `docs/superpowers/`). This covers markdown links, include/embed targets, and prose extension-mentions in one pass; they should all read `.tmd` now.

```bash
git ls-files '*.tmd' | xargs sed -i 's/\.qmd\b/.tmd/g'
# _site.yml files are not *.tmd — rewrite them explicitly:
for y in corpus/tech-blog/_site.yml corpus/demo-book/_site.yml corpus/bayesian-website/_site.yml docs/guide/_site.yml docs/internals/_site.yml; do
  sed -i 's/\.qmd\b/.tmd/g' "$y"
done
```

- [ ] **Step 4: Verify no `.qmd` remains in migrated content.**

```bash
grep -rn '\.qmd\b' $(git ls-files '*.tmd') corpus/*/_site.yml docs/*/_site.yml || echo "clean"
```
Expected: `clean` (no matches). If any remain, they are intentional-historical only inside `notes/`/specs (not in this set) — investigate any hit here.

- [ ] **Step 5: Run the suite; it will fail on tests that reference the renamed real files.**

```bash
cargo test -p taliesin-core -q 2>&1 | grep -E 'FAILED|error|No such file' | head
```
Expected: failures naming `.qmd` paths in `corpus.rs` / `tech_blog.rs` / `config.rs` / etc. These are the real-path references to fix.

- [ ] **Step 6: Fix the real-path references in the failing test files.** In each failing test, change the hardcoded corpus/docs path literals `"…​.qmd"` → `"…​.tmd"` (these point at files renamed in Step 2). Do **not** touch fixture literals in tests that build their *own* temp trees (those are Task 2). Re-run until green.

```bash
cargo test -p taliesin-core -q && cargo test -p taliesin-server -q
```
Expected: PASS.

- [ ] **Step 7: `check` the corpus + docs (broken-link gate).**

```bash
cargo build -p taliesin-server -q
BIN=target/debug/taliesin
for p in corpus/tech-blog corpus/demo-book corpus/bayesian-website docs/guide docs/internals; do
  echo "== $p =="; $BIN check "$p" || true
done
```
Expected: no broken-link diagnostics (same clean result as before the rename). Fix any `.qmd` link the sed missed.

- [ ] **Step 8: Commit.**

```bash
git add -A
git commit -m "refactor(shed-quarto): migrate corpus + docs to .tmd (phase 1a)

Rename all 105 .qmd -> .tmd and rewrite intra-project links, include/embed
targets, _site.yml chapter lists, and the tests that reference real corpus/docs
paths. Behavior-neutral: ext.rs still accepts both spellings."
```

### Task 2: Migrate remaining self-built `.qmd` test fixtures to `.tmd`

**Type:** mechanical refactor (gate = `cargo test`). Needed so nothing depends on `.qmd` before Phase 2 flips acceptance. These pass either way today (dual-accept), so this is a clean, isolated commit.

**Files:** the `.rs` files that build their own temp-dir fixtures with `.qmd` names — the remaining hits after Task 1 in: `crates/core/src/site/mod.rs`, `crates/core/src/site/links.rs`, `crates/core/src/diagnostics/links.rs`, `crates/core/src/render/validate.rs`, `crates/server/src/check.rs`, `crates/server/src/build.rs`, `crates/server/src/exec.rs`, `crates/server/src/serve_site/mod.rs`, and any others.

**Interfaces:** none change.

- [ ] **Step 1: List the remaining `.qmd` occurrences to migrate (excluding intentional keeps).**

```bash
grep -rn '\.qmd' crates --include='*.rs' | grep -vi 'quarto' | wc -l   # the migration target set
grep -rln '\.qmd' crates --include='*.rs'
```
Note: leave `.qmd` **only** where it is (a) the `ext.rs` both-spellings test (rewritten in Task 3), or (b) a user-facing message string (migrated in Task 7). Everything else is a fixture path.

- [ ] **Step 2: Rewrite fixture path literals `.qmd` → `.tmd` in those files**, skipping the two exceptions above. Do it file-by-file (or a scoped sed) and keep the exceptions untouched:

```bash
# Example per file; DO NOT run on ext.rs. Review each diff.
sed -i 's/\.qmd"/.tmd"/g; s#\.qmd/#.tmd/#g' crates/core/src/site/mod.rs crates/core/src/site/links.rs # …etc
```

- [ ] **Step 3: Run the suite.**

```bash
cargo test -p taliesin-core -q && cargo test -p taliesin-server -q
```
Expected: PASS (fixtures now use `.tmd`, still accepted).

- [ ] **Step 4: Confirm only the intended `.qmd` keeps remain.**

```bash
grep -rn '\.qmd' crates --include='*.rs' | grep -v 'ext.rs' | grep -vE 'usage:|no \.qmd pages|<file\.qmd'
```
Expected: no output (all remaining `.qmd` are the ext.rs test + user-facing strings, handled in Tasks 3 and 7).

- [ ] **Step 5: Commit.**

```bash
git add -A
git commit -m "refactor(shed-quarto): migrate self-built test fixtures to .tmd (phase 1b)"
```

---

## Phase 2 — Drop `.qmd` acceptance + the aliases (the behavioral break)

### Task 3: `.tmd`-only — drop `.qmd` from `ACCEPTED_SOURCE_EXTS`

**Type:** behavioral (test-first).

**Files:**
- Modify: `crates/core/src/ext.rs` (const at line 13, module doc comment lines 1-7, the test `accepts_both_spellings_rejects_others` ~line 40).

**Interfaces:**
- Produces: `ACCEPTED_SOURCE_EXTS: &[&str] = &["tmd"]`; `is_source_ext("qmd") == false`.

- [ ] **Step 1: Rewrite the test to pin `.tmd`-only (make it fail first).** In `crates/core/src/ext.rs`, replace the `accepts_both_spellings_rejects_others` test:

```rust
    #[test]
    fn accepts_tmd_only_rejects_qmd_and_others() {
        assert!(is_source_ext("tmd"));
        assert!(!is_source_ext("qmd"), "qmd is no longer an accepted source extension");
        assert!(!is_source_ext("md") && !is_source_ext("html") && !is_source_ext(""));
        assert!(is_source_path(Path::new("a/b/index.tmd")));
        assert!(!is_source_path(Path::new("a/b/index.qmd")));
    }
```

- [ ] **Step 2: Run it; verify it fails.**

```bash
cargo test -p taliesin-core ext:: -q
```
Expected: FAIL (`is_source_ext("qmd")` is still true).

- [ ] **Step 3: Flip the const and reword the module doc.** In `crates/core/src/ext.rs`:

```rust
//! The source-file extension vocabulary, defined once.
//!
//! Taliesin's native and only source extension is `.tmd`. Every place that
//! recognizes a source file — the site page walker, the `check` walker, link
//! rewriting, book chapter naming, deck/embed href mapping — routes through here.

// …

/// Every accepted source extension (no leading dot). A file is a Taliesin source
/// document iff its extension is one of these.
pub const ACCEPTED_SOURCE_EXTS: &[&str] = &["tmd"];
```
Also update the `strip_source_ext` doc comment's `(`.tmd` / `.qmd`)` to `(`.tmd`)`.

- [ ] **Step 4: Run the full suite.**

```bash
cargo test -p taliesin-core -q && cargo test -p taliesin-server -q
```
Expected: PASS. (If anything fails on a `.qmd` path, a fixture was missed in Task 2 — fix it.)

- [ ] **Step 5: Commit.**

```bash
git add crates/core/src/ext.rs
git commit -m "feat(shed-quarto)!: .tmd is the only accepted source extension"
```

### Task 4: Drop the `format: revealjs` / `*-revealjs` alias

**Type:** behavioral (test-first).

**Files:**
- Modify: `crates/core/src/render/fm_extract.rs` (`is_reveal_format` line 92-95; the doc comments at lines 45-47, 60-63, 70-71, 89-91).
- Modify: `crates/core/src/render/tests.rs` (18 `format: revealjs` occurrences → `format: deck`; add one negative test).
- Modify: the 2 docs decks now on `.tmd`: `docs/guide/demo.tmd:4` and `docs/guide/tour.tmd:4` — `format: revealjs` → `format: deck`.
- Modify: `crates/core/src/render/model.rs:118` comment mentioning `*-revealjs`.

**Interfaces:**
- Produces: `is_reveal_format(n)` returns true only for `n == "deck" || n.ends_with("-deck")`.

- [ ] **Step 1: Add a negative test (fails first).** In `crates/core/src/render/tests.rs`:

```rust
    #[test]
    fn revealjs_format_is_no_longer_a_deck() {
        // `format: revealjs` was the deprecated Quarto spelling; after shedding it, a
        // doc with that format is a normal HTML page, not a deck.
        let doc = render_document("---\nformat: revealjs\n---\n\n## A Slide\n");
        assert_eq!(doc.format, DocFormat::Html);
    }
```
(Confirm `doc.format` is the field name; adjust to the real accessor if different.)

- [ ] **Step 2: Run it; verify it fails.**

```bash
cargo test -p taliesin-core revealjs_format_is_no_longer_a_deck -q
```
Expected: FAIL (currently detected as `Reveal`).

- [ ] **Step 3: Drop the `revealjs` spellings.** In `crates/core/src/render/fm_extract.rs`, `is_reveal_format`:

```rust
/// Whether a `format:` name selects a slide deck. The only spelling is `deck`
/// (or an extension variant `<ext>-deck`); the engine is taliesin's own.
fn is_reveal_format(name: &str) -> bool {
    let n = name.trim().trim_matches(['"', '\'']);
    n == "deck" || n.ends_with("-deck")
}
```
Update the surrounding doc comments (lines 45-47, 60-63, 70-71) to drop the `revealjs` / `*-revealjs` references, and `model.rs:118` likewise.

- [ ] **Step 4: Convert the deck tests + docs decks to `format: deck`.**

```bash
sed -i 's/format: revealjs/format: deck/g' crates/core/src/render/tests.rs docs/guide/demo.tmd docs/guide/tour.tmd
```

- [ ] **Step 5: Run the suite.**

```bash
cargo test -p taliesin-core -q && cargo test -p taliesin-server -q
```
Expected: PASS (the negative test passes; all deck tests still pass on `format: deck`).

- [ ] **Step 6: Commit.**

```bash
git add -A
git commit -m "feat(shed-quarto)!: format: deck is the only deck spelling (drop revealjs)"
```

### Task 5: Located "unknown format `revealjs`" did-you-mean

**Type:** behavioral, NEW code (test-first). Front-matter *keys* are validated today (`frontmatter::validate`), but `format` *values* are not. Add a targeted check so the exact spelling we just removed nudges to `deck` instead of silently rendering a plain page.

**Files:**
- Modify: `crates/core/src/frontmatter.rs` (the `validate` fn body ~lines 113-133; it already produces `located(...)` diagnostics and has `block_key_line`).

**Interfaces:**
- Consumes: `located(message, line)`, `block_key_line(block, key)` (existing, same file).
- Produces: a located diagnostic `unknown format `revealjs` (did you mean `deck`?)` when the top-level `format` value names a dropped `revealjs` / `*-revealjs` spelling.

- [ ] **Step 1: Write the failing test.** In `crates/core/src/frontmatter.rs` tests:

```rust
    #[test]
    fn revealjs_format_value_warns_with_did_you_mean() {
        let diags = validate("format: revealjs\ntitle: T\n");
        assert!(
            diags.iter().any(|d| d.message.contains("unknown format `revealjs`")
                && d.message.contains("did you mean `deck`")),
            "expected a revealjs->deck did-you-mean, got {diags:?}"
        );
    }

    #[test]
    fn deck_format_value_is_not_flagged() {
        let diags = validate("format: deck\n");
        assert!(!diags.iter().any(|d| d.message.contains("unknown format")));
    }
```
(Match the real `validate` signature + `Diagnostic.message` field; adjust names if they differ.)

- [ ] **Step 2: Run; verify it fails.**

```bash
cargo test -p taliesin-core revealjs_format_value_warns -q
```
Expected: FAIL (no such diagnostic today).

- [ ] **Step 3: Add the check** in `validate`, after the KNOWN_KEYS loop and before the nested validators. It reads the top-level `format` value (string form or block-form sub-keys) and flags a `revealjs` spelling:

```rust
    // `format: revealjs` / `*-revealjs` was the Quarto deck spelling; it is no longer
    // accepted. Nudge to `deck` (edit distance is too large for the generic did-you-mean,
    // so name the migration explicitly) rather than silently rendering a plain page.
    if let Some(fmt) = map.get("format") {
        let named_revealjs = |s: &str| {
            let s = s.trim().trim_matches(['"', '\'']);
            s == "revealjs" || s.ends_with("-revealjs")
        };
        let hit = match fmt {
            serde_yaml::Value::String(s) => named_revealjs(s).then(|| s.clone()),
            serde_yaml::Value::Mapping(m) => m
                .keys()
                .filter_map(|k| k.as_str())
                .find(|k| named_revealjs(k))
                .map(|k| k.to_string()),
            serde_yaml::Value::Sequence(seq) => seq
                .iter()
                .filter_map(|v| v.as_str())
                .find(|s| named_revealjs(s))
                .map(|s| s.to_string()),
            _ => None,
        };
        if let Some(spelling) = hit {
            let line = block_key_line(block, "format");
            out.push(located(
                format!("unknown format `{spelling}` (did you mean `deck`?)"),
                line,
            ));
        }
    }
```

- [ ] **Step 4: Run the tests.**

```bash
cargo test -p taliesin-core frontmatter -q
```
Expected: PASS (both new tests; existing frontmatter tests unaffected).

- [ ] **Step 5: Full suite + commit.**

```bash
cargo test -p taliesin-core -q && cargo test -p taliesin-server -q
git add crates/core/src/frontmatter.rs
git commit -m "feat(shed-quarto): warn 'unknown format revealjs (did you mean deck?)'"
```

### Task 6: Drop the `ojs_define()` alias + migrate corpus/docs callers

**Type:** behavioral. The Python global is removed; corpus/docs that call `ojs_define(...)` must call `define(...)` or they `NameError` when executed.

**Files:**
- Modify: `crates/server/src/kernel.rs:68` — delete `globals()["ojs_define"] = define`.
- Modify (source callers): the 8 corpus posts + docs teaching pages that call `ojs_define(` (now on `.tmd`): `corpus/posts/{pca-geometry,em-algorithm,fourier-transform}/index.tmd`, `corpus/tech-blog/posts/{pca-geometry,evidence-lower-bound,Kruskal-Wallis-test,em-algorithm,fourier-transform}/index.tmd`, and docs guide/internals code examples — `define(` instead.

**Interfaces:**
- Produces: `define` is the only Python-side reactive-publish global. The `qmd-define` script-type wire is UNTOUCHED.

- [ ] **Step 1: Remove the alias line** in `crates/server/src/kernel.rs` (keep line 67):

```python
globals()["define"] = define
```
(delete the following `globals()["ojs_define"] = define` line).

- [ ] **Step 2: Migrate the callers** in corpus + docs source:

```bash
grep -rl 'ojs_define' corpus docs --include='*.tmd' | xargs sed -i 's/\bojs_define(/define(/g'
grep -rn 'ojs_define' corpus docs --include='*.tmd'   # expect no matches
```

- [ ] **Step 3: Verify — no `ojs_define` in corpus/docs source, and `define` still bridges.** The static tests do not execute cells, so run a real kernel smoke on one migrated post to confirm the `define()` bridge still emits a `qmd-define` blob:

```bash
cargo test -p taliesin-core -q && cargo test -p taliesin-server -q
# Kernel smoke (needs QMD_FAST_PYTHON with ipykernel): render a post that uses define()
cargo run -p taliesin-server -- build corpus/posts/em-algorithm/index.tmd /tmp/emalg.html \
  && grep -c 'type="qmd-define"' /tmp/emalg.html   # expect >= 1
```
Expected: tests PASS; the built HTML still contains a `qmd-define` blob. (If no kernel is available, at minimum confirm the code removal + that `define(` is the only spelling in corpus.)

- [ ] **Step 4: Commit.**

```bash
git add -A
git commit -m "feat(shed-quarto)!: define() is the only reactive-publish global (drop ojs_define)"
```

### Task 7: Migrate user-facing `.qmd` strings + watcher extension

**Type:** behavioral/user-facing (gate = `cargo test` + grep). These are the CLI usage/error strings and the watcher's watched-extension list — the last places `.qmd` is user-visible in code.

**Files:**
- Modify: `crates/server/src/main.rs` (usage strings: `render <file.qmd>`, `blocks <file.qmd>`, the init `index.qmd` line), `crates/server/src/check.rs:231` (`check <file.qmd|dir>`), `crates/server/src/build.rs` (`build <file.qmd|dir> …`), the `no .qmd pages found under {}` error (locate via grep), and their test assertions.
- Modify: `crates/core/src/serve` watcher `EXTS` (`crates/server/src/serve/mod.rs:915`) — drop `"qmd"`.

**Interfaces:** none new; message text changes `.qmd` → `.tmd`.

- [ ] **Step 1: Find every user-facing `.qmd` string + its assertions.**

```bash
grep -rn 'file\.qmd\|\.qmd|dir\|no \.qmd pages\|index\.qmd\|render <file\|check <file\|build <file\|blocks <file' crates/server/src --include='*.rs'
grep -rn '\.qmd' crates --include='*.rs' | grep -iE 'usage|assert|expect|pages found'
```

- [ ] **Step 2: Rewrite each usage/error string `.qmd` → `.tmd`** (and the matching test assertions in the same commit). E.g. `usage: taliesin render <file.qmd>` → `<file.tmd>`; `no .qmd pages found under {}` → `no .tmd pages found under {}`; the init scaffold `index.qmd` → `index.tmd`.

- [ ] **Step 3: Drop `"qmd"` from the watcher `EXTS`** in `crates/server/src/serve/mod.rs` (line 915): the list starts `"tmd", "md", …` (it should already include `"tmd"`; confirm and remove only `"qmd"`).

- [ ] **Step 4: Confirm no `.qmd` remains anywhere in code except the ext.rs negative test.**

```bash
grep -rn '\.qmd' crates --include='*.rs' | grep -v 'ext.rs'
```
Expected: no output.

- [ ] **Step 5: Full suite + commit.**

```bash
cargo test -p taliesin-core -q && cargo test -p taliesin-server -q
git add -A
git commit -m "feat(shed-quarto): user-facing CLI/error strings + watcher use .tmd only"
```

---

## Phase 3 — Remove migration on-ramps + reword user-facing prose

### Task 8: Remove the `_quarto.yml` migration breadcrumb

**Type:** behavioral (test-first).

**Files:**
- Modify: `crates/server/src/check.rs` — delete `quarto_migration_hint` (lines 173-187), its call site in `cmd_check` (lines 241-256, the `if let Some(hint) = quarto_migration_hint(target)` block), and any call in `main::build_site`; delete its test.

**Interfaces:**
- Produces: a directory with only `_quarto.yml` (no `_site.yml`) falls through to the normal site-walker "no `_site.yml`" diagnostic.

- [ ] **Step 1: Write/adjust a test pinning the generic onboarding (fails first).** In `crates/server/src/check.rs` tests, replace any test asserting the Quarto breadcrumb with:

```rust
    #[test]
    fn quarto_only_dir_gets_generic_no_site_yml_not_a_quarto_breadcrumb() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("_quarto.yml"), "project:\n  type: website\n").unwrap();
        let out = run_check_human(dir.path());   // use the crate's existing check-invoking test helper
        assert!(out.contains("_site.yml"), "expected the generic no-_site.yml message");
        assert!(!out.to_lowercase().contains("quarto"), "no Quarto breadcrumb should remain");
    }
```
(Use the file's existing test harness for invoking `check`; match its helpers.)

- [ ] **Step 2: Run; verify it fails** (the breadcrumb still fires, so the output contains "quarto").

```bash
cargo test -p taliesin-server quarto_only_dir -q
```
Expected: FAIL.

- [ ] **Step 3: Delete `quarto_migration_hint` + its call sites + the old test.** Remove the function (check.rs:173-187), the `if let Some(hint) = quarto_migration_hint(target) { … }` block in `cmd_check`, the same breadcrumb in `main::build_site` if present (grep `quarto_migration_hint`), and the now-obsolete breadcrumb test.

```bash
grep -rn 'quarto_migration_hint' crates   # expect no matches after removal
```

- [ ] **Step 4: Run the suite.**

```bash
cargo test -p taliesin-server -q && cargo test -p taliesin-core -q
```
Expected: PASS (the new generic-onboarding test passes).

- [ ] **Step 5: Commit.**

```bash
git add -A
git commit -m "feat(shed-quarto): drop the _quarto.yml migration breadcrumb"
```

### Task 9: Remove `.quarto` from the watcher skip-list

**Type:** small behavioral change.

**Files:**
- Modify: `crates/server/src/serve/mod.rs` — `SKIP_DIRS` (line 924) drop `".quarto"`; reword the doc comment at line 905 to drop `.quarto`.

- [ ] **Step 1: Edit `SKIP_DIRS`** — remove the `".quarto",` entry (keep `_site`, `_book`, `_freeze`, `.git`, `node_modules`), and change the comment `(`.git`/`.quarto`)` → `(`.git`)`.

- [ ] **Step 2: Run the suite** (there may be a `relevant_path` test to keep green).

```bash
cargo test -p taliesin-server relevant_path -q ; cargo test -p taliesin-server -q
```
Expected: PASS (adjust any test that asserted `.quarto` is skipped — it should no longer be).

- [ ] **Step 3: Commit.**

```bash
git add crates/server/src/serve/mod.rs
git commit -m "feat(shed-quarto): stop skipping .quarto in the file watcher"
```

### Task 10: Delete the "migrating from Quarto" page

**Type:** content deletion.

**Files:**
- Delete: `docs/guide/using/migrating-from-quarto.tmd` (renamed from `.qmd` in Task 1).
- Modify: `docs/guide/_site.yml` — remove the `- using/migrating-from-quarto.tmd` chapter entry (line ~10).
- Modify: any doc that links to it (grep) — remove/redirect the link.

- [ ] **Step 1: Delete + de-list.**

```bash
git rm docs/guide/using/migrating-from-quarto.tmd
sed -i '\#using/migrating-from-quarto.tmd#d' docs/guide/_site.yml
grep -rn 'migrating-from-quarto' docs corpus --include='*.tmd'   # find stray links
```

- [ ] **Step 2: Remove any stray links** the grep found (e.g. a "Reference:" nav line), then `check`:

```bash
cargo run -p taliesin-server -- check docs/guide
```
Expected: no broken-link diagnostic pointing at the deleted page.

- [ ] **Step 3: Commit.**

```bash
git add -A
git commit -m "docs(shed-quarto): delete the migrating-from-quarto page + nav entry"
```

### Task 11: Rewrite the README off Quarto + `.qmd`

**Type:** prose.

**Files:**
- Modify: `README.md` — the qmd-fast/`.qmd` transition banner (lines 3-7), the `.qmd`/`.tmd` intro (line 9), the "focused replacement for Quarto / goals Quarto's architecture can't deliver" (lines 11-12), the click-to-source `.qmd` mention (line 14), the docs links `docs/guide/index.qmd`→`.tmd` (lines 21-22), and any other `.qmd`/Quarto line.

- [ ] **Step 1: Rewrite** to a stand-alone identity. Keep it honest and Quarto-free; the three goals stay but are stated as Taliesin's design, not as "what Quarto can't do." Example for lines 9-18:

```markdown
A single-purpose, performance-oriented tool for authoring HTML from `.tmd` files:
blog posts, slide decks, books, and **multi-page websites**. It is built around three
goals:

1. **Click-to-source.** Alt-click (Option-click on Mac) a rendered element, jump to its `.tmd` source.
2. **Block-level incremental updates.** Saving a change swaps only the affected
   block(s) in place, preserving scroll position and the runtime state of live
   components (Three.js, `{js}` cells).
3. **No per-edit startup cost.** A long-running Rust server with a warm Jupyter kernel.
```
Update the transition banner (drop the `.qmd`-accepted / qmd-fast framing, or reduce to a one-line "formerly qmd-fast" note without the `.qmd` compat claim) and the docs links to `.tmd`.

- [ ] **Step 2: Verify no `.qmd` / Quarto identity remains in README.**

```bash
grep -n '\.qmd\|[Qq]uarto' README.md   # expect nothing (or only an allowlisted historical note if you kept one)
```

- [ ] **Step 3: Commit.**

```bash
git add README.md
git commit -m "docs(shed-quarto): README stands on its own (no Quarto, .tmd only)"
```

### Task 12: Reframe the docs prose (guide fully, internals case-by-case) + `ojs_define`→`define` teaching

**Type:** prose (gate = the Task 13 grep-gate + `check`).

**Files (guide — fully reframe, drop Quarto):**
- `docs/guide/index.tmd`, `docs/guide/tour.tmd`, `docs/guide/using/formats.tmd`, `docs/guide/reference/cli.tmd`, `docs/guide/reference/frontmatter.tmd`, `docs/guide/reference/configuration.tmd`, `docs/guide/reference/cell-options.tmd`, `docs/guide/using/recipes.tmd`, `docs/guide/reference/troubleshooting.tmd`.
- The `format: revealjs` prose in these (formats/frontmatter/configuration/recipes/troubleshooting) → `format: deck`; drop the `/revealjs` and `<name>-revealjs` variants from the frontmatter/configuration tables.
- The guide pages teaching `ojs_define` (cli, cell-options, tour, code) → teach `define()`.

**Files (internals — case-by-case per spec §4: keep accurate architectural-contrast, drop identity/marketing):**
- `docs/internals/sites.tmd`, `docs/internals/validation.tmd`, `docs/internals/repository.tmd`, `docs/internals/rendering.tmd`, `docs/internals/server.tmd`, `docs/internals/execution.tmd`, `docs/internals/deck-engine.tmd`, `docs/internals/data-types.tmd`, `docs/internals/protocol.tmd` — reframe `format: revealjs`→`format: deck`; `ojs_define`→`define`; keep only mentions that explain a real architectural difference, reworded to not read as identity.

**Files (corpus sub-project tooling):**
- `corpus/tech-blog/.claude/skills/{new-post,new-project}/SKILL.md` (and `deploy`) — the descriptions say "Quarto frontmatter"; reword to "Taliesin frontmatter" (they scaffold `.tmd` now).

- [ ] **Step 1: Reframe the guide pages.** For each file, remove Quarto identity/marketing mentions and convert `format: revealjs`→`format: deck`, `/revealjs`/`*-revealjs` variants gone, `ojs_define`→`define`. Keep the how-to content intact.

- [ ] **Step 2: Reframe the internals pages** per the §4 rule (keep accurate contrast, drop identity), same `revealjs`/`ojs_define` conversions.

- [ ] **Step 3: Reword the corpus/tech-blog skills.**

```bash
grep -rn '[Qq]uarto' corpus/tech-blog/.claude
sed -i 's/Quarto frontmatter/Taliesin frontmatter/g' corpus/tech-blog/.claude/skills/*/SKILL.md
```

- [ ] **Step 4: `check` both books + confirm no `revealjs`/`ojs_define` remain in docs/corpus source.**

```bash
cargo run -p taliesin-server -- check docs/guide && cargo run -p taliesin-server -- check docs/internals
grep -rn 'revealjs\|ojs_define' docs corpus --include='*.tmd'   # expect nothing
```

- [ ] **Step 5: Commit.**

```bash
git add -A
git commit -m "docs(shed-quarto): reframe guide + internals prose off Quarto; define()/deck spellings"
```

### Task 13: Grep-gate — no user-facing "Quarto" can regress

**Type:** durable guard (a test).

**Files:**
- Create: `crates/server/tests/no_user_facing_quarto.rs` (or add to an existing integration test) — a test that greps the user-facing surfaces and fails if "quarto" appears outside the allowlist.

**Interfaces:** none; a CI-visible test.

- [ ] **Step 1: Write the gate test.** It scans CLI `--help`/usage output, `README.md`, and the built docs-guide source, allowlisting the §5 keeps.

```rust
// crates/server/tests/no_user_facing_quarto.rs
use std::process::Command;

/// User-facing surfaces must not name Quarto (the shed-Quarto invariant). Internal
/// comments, test names, THIRD_PARTY, and dated history are deliberately exempt.
#[test]
fn no_quarto_in_user_facing_surfaces() {
    // 1) CLI usage/help text.
    let bin = env!("CARGO_BIN_EXE_taliesin");
    let help = Command::new(bin).arg("--help").output().expect("run --help");
    let help_text = String::from_utf8_lossy(&help.stdout) + &String::from_utf8_lossy(&help.stderr);
    assert!(
        !help_text.to_lowercase().contains("quarto"),
        "CLI help names Quarto:\n{help_text}"
    );

    // 2) README + guide docs source (repo-relative to this crate).
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    for rel in ["README.md", "docs/guide"] {
        let path = std::path::Path::new(root).join(rel);
        for f in walk_tmd_and_md(&path) {
            let txt = std::fs::read_to_string(&f).unwrap_or_default();
            assert!(
                !txt.to_lowercase().contains("quarto"),
                "{} names Quarto",
                f.display()
            );
        }
    }
}

fn walk_tmd_and_md(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk_tmd_and_md(&p));
            } else if matches!(p.extension().and_then(|x| x.to_str()), Some("tmd") | Some("md")) {
                out.push(p);
            }
        }
    }
    out
}
```
(Adjust the crate-root relative path and the binary name to match this crate's layout; `CARGO_BIN_EXE_taliesin` assumes the server bin is named `taliesin` — confirm in `Cargo.toml`.)

- [ ] **Step 2: Run it.**

```bash
cargo test -p taliesin-server no_quarto_in_user_facing -q
```
Expected: PASS (Tasks 10-12 cleared the guide + README). If it fails, it names the offending file — fix the prose, not the test.

- [ ] **Step 3: Full suite + `check` + commit.**

```bash
cargo test -p taliesin-core -q && cargo test -p taliesin-server -q
cargo run -p taliesin-server -- check docs/guide && cargo run -p taliesin-server -- check docs/internals
git add -A
git commit -m "test(shed-quarto): gate that no user-facing surface names Quarto"
```

---

## Final verification (whole change)

- [ ] `cargo test -p taliesin-core && cargo test -p taliesin-server` green.
- [ ] `cargo clippy --workspace` clean; `cargo fmt --check` clean.
- [ ] `cd web-client && npx -y -p typescript tsc -p jsconfig.json` clean.
- [ ] `taliesin check` clean on `corpus/tech-blog`, `corpus/demo-book`, `corpus/bayesian-website`, `docs/guide`, `docs/internals`.
- [ ] Browser deck smoke (chrome-devtools MCP): serve a `format: deck` doc (`corpus/deck.tmd`), confirm the native engine mounts and console is error-free.
- [ ] `git ls-files '*.qmd'` returns nothing; `grep -rn '\.qmd' crates --include='*.rs' | grep -v ext.rs` returns nothing.
- [ ] Spot-check: `taliesin build corpus/posts/em-algorithm/index.tmd` still emits a `qmd-define` blob (wire unchanged).

## Self-review notes (spec coverage)

- Spec §3.A → Tasks 1, 2, 3, 7. §3.B → Task 4. §3.C → Task 6. §3.D → Tasks 8, 9. §3.E → Tasks 10, 11, 12.
- Spec §4 behaviors → Task 5 (revealjs did-you-mean), Task 6 (ojs_define NameError), Task 8 (generic onboarding), Task 4 (revealjs not a deck).
- Spec §6 migration mechanics → Tasks 1, 2 (with the discovered larger Rust-fixture surface made explicit).
- Spec §8 verification → per-task gates + the Final verification section + Task 13 grep-gate.
- Spec §5 keeps → enumerated in Global Constraints and respected (no touch to `qmd-define` wire, freeze version, internal names, THIRD_PARTY, notes/, CLAUDE.md).
