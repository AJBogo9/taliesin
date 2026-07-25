# Purge `qmd` Naming Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove every live `qmd` token from Taliesin's code, assets, corpus, docs, editor extension, tooling and CI, replacing it with the established `tali` prefix, behind a regression net that makes a half-finished rename impossible to land silently.

**Architecture:** Build the guard net *first*, against the current `qmd` names, so it demonstrably passes before the rename and demonstrably diffs during it. Then rename family-by-family, hardest-to-see first (things no Rust test can observe: browser storage, DOM events, URL params, cross-process postMessage), then the mechanical HTML/CSS/JS contract, then prose. Close with a repo-wide zero-`qmd` assertion wired into CI.

**Tech Stack:** Rust (edition 2024), vanilla browser JS, `cargo test`, `tsc` for JS type-checking, chrome-devtools MCP for browser verification.

## Progress

**Tasks 1 and 2 are done and on `origin/main`. Resume at Task 3.**

| Task | State |
|---|---|
| 1. Regression net | ✅ `183069b` — 3 checks in `token_contract.rs` + a `FORMAT_VERSION` pin in `freeze.rs`, each verified to *fire* on the defect it targets |
| 2. Silent contracts | ✅ `e1cfa3b` — storage keys, theme event, `?tali=`, cookie, postMessage. Browser-verified |
| 3–9 | ⬜ not started |

**Corrections to this plan discovered while executing it** (the plan text below has been updated, but
these are the ones that cost time):

- The census in Task 1 as originally drafted saw only **4 of the 15** `data-qmd-*` names, because a
  block-level corpus render never produces site chrome or JS-stamped attributes. A second, source-side
  census was added. The two need *different* scanners; unifying them breaks one or the other.
- The orphan check must read the **live census**, not the pinned const, and must **exclude Rust** from
  the set of "consumers". Both were wrong in the first draft and both made the check vacuous.
- Task 2's file list missed `<style id="qmd-theme">` consumers in `client.js`, `protocol.rs` and
  `theme_css.rs`, and a **second** `{ qmd: 'deck' }` postMessage protocol inside `deck.js`.
- `qmdFast.path` / `qmdFast.open` are **not** live VS Code settings keys (the real one is
  `taliesin.path`); delete the stale references rather than renaming them.

**Environment notes for whoever resumes:**

- Port **4388 is the `/preview` port** and a parallel session may own it. `python3 -m http.server`
  fails to bind silently and you will then verify *someone else's* page and get a confident wrong
  answer. Check `ss -ltn | grep <port>` first.
- `pkill -f <pattern>` matches this session's own shell command line and **kills the shell**. Kill by
  PID.
- chrome-devtools MCP refuses to start when another session holds
  `~/.cache/chrome-devtools-mcp/chrome-profile`. Fall back to `puppeteer-core` from
  `tools/ui-audit/node_modules` with your own `userDataDir`; import it from
  `.../puppeteer-core/lib/puppeteer/puppeteer-core.js`.
- A fresh worktree has no `node_modules` or `.vscode-test`, so 4 of the extension's grammar tests fail
  for environmental reasons. That is not your rename.

## Global Constraints

- **Breaking changes are allowed.** The author is the only user. No back-compat aliases survive: `window.qmdJs`, `window.QmdDeck`, `window.qmdEnhancers`, `window.qmdEnhanceCode`, `window.qmdInit*` and the `qmd` cell-API parameter are all deleted outright, not renamed.
- **Frozen-name exemptions in CLAUDE.md are void.** CLAUDE.md currently states that the `qmd-theme` storage key and `qmd:themechange` event keep their frozen runtime names. This plan supersedes that; Task 9 updates CLAUDE.md.
- **`docs/superpowers/` and `notes/` are excluded from the purge.** They are the dated plan/spec archive and pre-rename historical record. They keep their `qmd` text. The guard test in Task 9 carries an explicit allowlist for exactly these two directories. (This plan file lives in `docs/superpowers/plans/`, so its own `qmd` text is exempt by that same rule.)
- **Also excluded from the purge and the guard:** `.git/`, `target/`, `_site/`, `_book/`, `_freeze/`, `node_modules/`, `editor/vscode/.vscode-test/`, `editor/vscode/package-lock.json`, and `*.min.js` (vendored: `mermaid.min.js` contains one incidental `QMD` inside a base64 blob and must not be edited).
- **Never run a blanket `sed` across the tree.** It compiles clean (verified 2026-07-25) and hides every real defect. Each task renames one family with an explicit, reviewed edit set.
- **Rebuild the binary before any browser check.** `assets/css/*` and `assets/js/*` are `include_str!`-compiled, so `taliesin build <dir>` alone re-emits the *old* bundled CSS/JS. Always `cargo build` first.
- **Commit after every task.** Do not batch.
- The `PostToolUse` hook runs `rustfmt` on every edited `.rs`; do not hand-format.

## Name Mapping (authoritative)

| Old | New | Family |
|---|---|---|
| `data-qmd-*` (15 attrs) | `data-tali-*` | Task 3 |
| `application/qmd-js` | `application/tali-js` | Task 4 |
| `qmd-define` (script type) | `tali-define` | Task 4 |
| `qmd_define` (Jupyter metadata key) | `tali_define` | Task 4 |
| `qmd-js.js` (filename) | `tali-js.js` | Task 4 |
| `qmd-theme` (localStorage key + `<style>` id) | `tali-theme` | Task 2 |
| `qmd-deck-theme` (localStorage key) | `tali-deck-theme` | Task 2 |
| `qmd-read:<path>` (localStorage key) | `tali-read:<path>` | Task 2 |
| `qmd:themechange` (CustomEvent) | `tali:themechange` | Task 2 |
| `?qmd=speaker\|feed\|present` | `?tali=speaker\|feed\|present` | Task 2 |
| `qmd_token` (cookie) | `tali_token` | Task 2 |
| `qmd-goto` / `qmd-cursor` (postMessage) | `tali-goto` / `tali-cursor` | Task 2 |
| `id="qmd"` (webview iframe) | `id="tali-preview"` | Task 2 |
| `window.qmdToHost` / `window.qmdGot` | `window.taliToHost` / `window.taliGot` | Task 2 |
| `qmd-references` / `qmd-footnotes` / `qmd-cite-this` / `qmd-title-block` (block ids) | `tali-references` / `tali-footnotes` / `tali-cite-this` / `tali-title-block` | Task 5 |
| `_qmd_*` / `_QMD_*` (Python, injected into kernel) | `_tali_*` / `_TALI_*` | Task 6 |
| `var(--qmd-*)` in `site/showcase.tmd` (6 props, never defined) | `var(--tali-*)` | Task 7 |
| `collect_qmd` (Rust test helper) | `collect_tmd` | Task 7 |
| `qmdFast.path` in `docs/guide/reference/troubleshooting.tmd:136` | delete the sentence | Task 8 |

**Note on `qmdFast.*`:** it is *not* a live VS Code settings key. The real key is `taliesin.path` (`editor/vscode/package.json:100`). The `qmdFast` occurrences are a stale-branding guard regex in `manifest.test.ts` and stale prose in `troubleshooting.tmd`. Do not "rename" it; delete the stale references.

---

### Task 1: Build the regression net (before any rename)  ✅ DONE (`183069b`)

The net must pass on today's `qmd` names, so it proves it measures the contract rather than the rename.

**Why this is needed:** the existing suite is blind to renames. Verified 2026-07-25 by running a blanket `sed` on a copy: build clean, and only 5 of 1387 tests changed state, 3 of which were block-id hash drift. The assertions are string literals in the same files a rename edits, so emitter and assertion move in lockstep.

**Files:**
- Create: `crates/core/tests/token_contract.rs`
- Modify: `crates/server/src/freeze.rs` (append one unit test to the existing `mod tests`)
- Test: both files above are the tests

**Interfaces:**
- Consumes: `crates/core/tests/common/mod.rs::corpus_dir()`
- Produces: `EMITTED_DATA_ATTRS` census in `token_contract.rs`, which Tasks 4 and 6 must update as their visible diff.

- [x] **Step 1: Write the census test with an empty pin so it fails loudly**

Create `crates/core/tests/token_contract.rs`:

```rust
//! The emitted-HTML attribute contract, pinned as one census.
//!
//! Every other test in the suite asserts on string literals that live in the same
//! file as the emitter, so a rename moves both sides together and nothing fails.
//! This file is the one place a change to the `data-*` vocabulary must be declared
//! by hand, which makes an incomplete rename a visible diff instead of a silent one.

mod common;

use common::corpus_dir;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Every `data-*` attribute name the corpus renders. Sorted. Update deliberately.
const EMITTED_DATA_ATTRS: &[&str] = &[];

/// Attributes with no CSS/JS consumer: purely informational markers read by
/// tooling or humans, never selected on at runtime. Each needs a one-line reason.
const NO_RUNTIME_CONSUMER: &[(&str, &str)] = &[];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Collect every `data-<name>` attribute name in `html`, in attribute position
/// (preceded by whitespace), so `data-` inside text or a URL is not counted.
fn scan_data_attrs(html: &str, out: &mut BTreeSet<String>) {
    let bytes = html.as_bytes();
    let mut i = 0usize;
    while let Some(off) = html[i..].find("data-") {
        let start = i + off;
        if start > 0 && !bytes[start - 1].is_ascii_whitespace() {
            i = start + 5;
            continue;
        }
        let rest = &html[start + 5..];
        let end = rest
            .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
            .unwrap_or(rest.len());
        if end > 0 && rest.as_bytes()[end - 1] != b'-' {
            out.insert(format!("data-{}", &rest[..end]));
        }
        i = start + 5 + end.max(1);
    }
}

fn tmd_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name.starts_with('_') || name == "node_modules" {
                continue;
            }
            tmd_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "tmd") {
            out.push(p);
        }
    }
}

fn census() -> BTreeSet<String> {
    let mut files = Vec::new();
    tmd_files(&corpus_dir(), &mut files);
    files.sort();
    assert!(!files.is_empty(), "corpus/ has no .tmd files to census");

    let mut attrs = BTreeSet::new();
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()));
        // `_with_includes` so blocks pulled in via `{{< include >}}` are censused too.
        let doc = taliesin_core::render_document_with_includes(&src, f.parent().unwrap());
        for block in &doc.blocks {
            scan_data_attrs(&block.html, &mut attrs);
        }
    }
    attrs
}

/// All bundled browser sources that could consume an attribute at runtime.
fn runtime_sources() -> String {
    let root = repo_root();
    let mut buf = String::new();
    let dirs = [
        root.join("crates/core/assets/js"),
        root.join("crates/core/assets/js/code-enhance"),
        root.join("crates/core/assets/css"),
        root.join("web-client"),
    ];
    for d in dirs {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name.ends_with(".min.js") || !(name.ends_with(".js") || name.ends_with(".css")) {
                continue;
            }
            buf.push_str(&std::fs::read_to_string(&p).unwrap_or_default());
            buf.push('\n');
        }
    }
    // The theme/deck runtimes are Rust-embedded JS string literals, not .js files.
    for rel in ["crates/core/src/render/theme.rs", "crates/core/src/render/deck.rs"] {
        buf.push_str(&std::fs::read_to_string(root.join(rel)).unwrap_or_default());
        buf.push('\n');
    }
    buf
}

#[test]
fn emitted_data_attribute_census_is_pinned() {
    let actual: Vec<String> = census().into_iter().collect();
    let expected: Vec<String> = EMITTED_DATA_ATTRS.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        actual, expected,
        "the emitted data-* vocabulary changed.\n\
         If this is a deliberate rename, paste the ACTUAL list into EMITTED_DATA_ATTRS \
         and confirm every consumer (assets/css, assets/js, web-client) moved with it.\n\
         ACTUAL:\n{actual:#?}"
    );
}

#[test]
fn every_emitted_attribute_has_a_runtime_consumer() {
    let sources = runtime_sources();
    let exempt: BTreeSet<&str> = NO_RUNTIME_CONSUMER.iter().map(|(a, _)| *a).collect();
    let orphans: Vec<&str> = EMITTED_DATA_ATTRS
        .iter()
        .copied()
        .filter(|a| !exempt.contains(a) && !sources.contains(a))
        .collect();
    assert!(
        orphans.is_empty(),
        "attributes emitted into HTML that no bundled CSS/JS references:\n{orphans:#?}\n\
         Either a rename moved the Rust side without the browser side, or the attribute \
         is informational and belongs in NO_RUNTIME_CONSUMER with a reason."
    );
}
```

- [x] **Step 2: Run it to get the real census printed**

```sh
cargo test -p taliesin-core --test token_contract 2>&1 | head -80
```

Expected: `emitted_data_attribute_census_is_pinned` FAILS, printing the actual sorted list.

(API confirmed 2026-07-25: `render_document_with_includes(src: &str, base_dir: &Path) -> RenderedDoc` at `crates/core/src/render/mod.rs:153`; `RenderedDoc.blocks: Vec<Block>` at `model.rs:244`; `Block.html: String` at `model.rs:109`. Note the bare `render_document` takes only `src`, so it will not compile with a second argument.)

- [x] **Step 3: Paste the actual list into `EMITTED_DATA_ATTRS`**

Copy the `ACTUAL:` list verbatim into the const, one `"data-…",` per line, keeping sort order.

- [x] **Step 4: Run again and resolve orphans**

```sh
cargo test -p taliesin-core --test token_contract
```

If `every_emitted_attribute_has_a_runtime_consumer` fails, for each orphan decide: is it selected on by CSS/JS somewhere the scan missed (widen `runtime_sources`), or is it informational? Informational ones go into `NO_RUNTIME_CONSUMER` as `("data-x", "reason")`, e.g. `("data-sourcepos", "click-to-source coordinate, read by the editor bridge not by a selector")`. Both tests must pass before moving on.

- [x] **Step 5: Add the freeze-format pin**

Append to the existing `mod tests` in `crates/server/src/freeze.rs`:

```rust
/// Tokens that appear inside a CACHED cell's rendered output.
///
/// `_freeze/` keys hash a cell's SOURCE, never its output, so changing the
/// vocabulary a cell's output carries busts nothing on its own: old entries
/// replay verbatim and the new browser runtime no longer recognises them.
/// `FORMAT_VERSION` is the only lever. This pin forces the two to move together.
/// (`d0b1ffa` is the bug this prevents: the `qmd-fig-*` -> `tali-fig-*` rename
/// shipped without a bump and needed a follow-up fix.)
const CACHED_OUTPUT_TOKENS: &[&str] = &[
    "application/qmd-js",
    "qmd-define",
    "tali-fig-light",
    "tali-fig-dark",
];

#[test]
fn cached_output_vocabulary_is_tied_to_format_version() {
    let joined = CACHED_OUTPUT_TOKENS.join("\u{1f}");
    let digest = format!("{:016x}", fnv1a(&joined));
    assert_eq!(
        (digest.as_str(), FORMAT_VERSION),
        ("0000000000000000", 3),
        "the cached-output token vocabulary changed. Bump FORMAT_VERSION, then \
         update BOTH values here. Skipping the bump makes every existing _freeze/ \
         entry replay markup the current runtime cannot read."
    );
}
```

- [x] **Step 6: Run it, paste in the real digest**

```sh
cargo test -p taliesin-server --lib freeze::tests::cached_output_vocabulary
```

Expected: FAILS showing the real digest. Replace `"0000000000000000"` with it. Re-run; expect PASS.

- [x] **Step 7: Confirm the whole suite is still green**

```sh
cargo test --workspace 2>&1 | grep -E '^test result'
```

Expected: no failures. (`cargo test` fail-fast can hide later binaries; if anything fails, re-run with `--no-fail-fast`.)

- [x] **Step 8: Commit**

```sh
git add crates/core/tests/token_contract.rs crates/server/src/freeze.rs
git commit -m "test: pin the emitted data-* census and tie cached-output tokens to FORMAT_VERSION

The suite asserts on string literals colocated with their emitters, so a rename
moves both sides at once and nothing fails. These two pins are the exception:
the census must be edited by hand, and the freeze digest forces a version bump
whenever a cached cell's output vocabulary changes."
```

---

### Task 2: Rename the contracts no Rust test can observe  ✅ DONE (`e1cfa3b`)

Browser storage, DOM events, URL params, an HTTP cookie, and the cross-process postMessage protocol. These break silently and are verified by hand in a browser, not by `cargo test`.

**Files:**
- Modify: `crates/core/src/render/theme.rs:95,147,148,239` (`qmd-theme` storage key + `<style>` id), `:127,185,207` (`qmd:themechange`)
- Modify: `crates/core/src/render/deck.rs:200,215` (`qmd-deck-theme`), `:213` (`qmd:themechange`)
- Modify: `crates/core/assets/js/code-enhance/14-reader-prefs.js:56`, `crates/core/assets/js/mermaid.js:16,134` (`qmd:themechange` listeners)
- Modify: `web-client/toc-spy.js:24` (`qmd-read:`)
- Modify: `crates/server/src/serve/mod.rs:2033,2055,2059` (`qmd-read:` assertions)
- Modify: `crates/core/assets/js/deck.js:1262,1956,2334,2520,2529,2538,2603,2604` and `crates/core/assets/css/deck.css:432` (`?qmd=`)
- Modify: `crates/server/src/serve/security.rs:91,107,115,119,123,323,345,360,365` (`qmd_token`)
- Modify: `web-client/client.js:1429,1451,1464,1594` (`qmd-goto`/`qmd-cursor`)
- Modify: `editor/vscode/src/extension.ts:54,57,70,79`, `editor/vscode/src/webview.ts:9,12,17,18` (wire names + `id="qmd"`)
- Modify: `editor/vscode/scripts/relay-harness.cjs:12,14,15,16,28,30,32` (`qmdToHost`/`qmdGot`)
- Modify: `crates/core/src/render/tests.rs:1941,1965,2357` (the `?qmd=` and `qmd:themechange` drift pins)

**Interfaces:**
- Consumes: nothing from Task 1 at compile time; Task 1's suite must be green first.
- Produces: `tali-goto` / `tali-cursor` postMessage names that Task 8's extension repackage depends on.

- [x] **Step 1: Rename the four browser-storage keys**

Apply the mapping table: `qmd-theme` → `tali-theme` (both the `localStorage` key and the `<style id="…">`), `qmd-deck-theme` → `tali-deck-theme`, `qmd-read:` → `tali-read:`. Update surrounding comments so they name the new key.

- [x] **Step 2: Rename the CustomEvent across all 7 emit/listen sites**

`qmd:themechange` → `tali:themechange`. All of: `theme.rs:127` (dispatch), `theme.rs:185,207` (listen), `deck.rs:213` (dispatch), `14-reader-prefs.js:56` (listen), `mermaid.js:134` (listen), `mermaid.js:16` (comment). Missing one listener leaves mermaid diagrams or the reader-prefs segmented control frozen on the old theme with no error.

- [x] **Step 3: Rename the deck URL param and the cookie**

`?qmd=` → `?tali=` in `deck.js` and the `deck.css` comment; the JS reads the param into a variable also named `qmd` (`deck.js:2520,2538`), rename that to `tali`. Then `qmd_token` → `tali_token` in `security.rs`, including its four unit-test assertions.

- [x] **Step 4: Rename the postMessage wire protocol on both sides at once**

`qmd-goto` → `tali-goto`, `qmd-cursor` → `tali-cursor` in `web-client/client.js`, `editor/vscode/src/extension.ts`, `editor/vscode/src/webview.ts`, `editor/vscode/scripts/relay-harness.cjs`. Also `id="qmd"` → `id="tali-preview"` and `getElementById("qmd")` → `getElementById("tali-preview")` in `webview.ts:9,12`, and `window.qmdToHost`/`window.qmdGot` → `window.taliToHost`/`window.taliGot` in `relay-harness.cjs`. This is one logical change across two processes; do not split it.

- [x] **Step 5: Update the three drift pins in `render/tests.rs`**

`tests.rs:1941,1965` assert on `?qmd=feed` / `?qmd=present`; `tests.rs:2357` on `qmd:themechange`. Update to the new names.

- [x] **Step 6: Build and type-check**

```sh
cargo build --workspace
cd web-client && npx -y -p typescript tsc -p jsconfig.json && cd ..
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json && cd ../../../..
cd editor/vscode && npm run compile && cd ../..
```

Expected: all clean.

- [x] **Step 7: Run the suite**

```sh
cargo test --workspace --no-fail-fast 2>&1 | grep -E '^test result'
```

Expected: no failures. Task 1's census must still pass (no `data-*` attribute changed in this task).

- [x] **Step 8: Verify in a real browser (the only check that covers this task)**

```sh
cargo build --release
./target/release/taliesin build corpus/deck.tmd --out /tmp/qmdcheck && \
  python3 -m http.server 4388 --directory /tmp/qmdcheck &
```

Using the chrome-devtools MCP, confirm all five, and capture a screenshot of each:
1. Toggle the theme, reload: the choice persists (`tali-theme` in localStorage, old `qmd-theme` ignored).
2. A page with a mermaid diagram re-renders on theme toggle (proves the `tali:themechange` listener fired).
3. `?tali=speaker` opens speaker view; `?tali=feed` forces the feed.
4. Scroll a book chapter, reload: read position restores (`tali-read:<path>`).
5. Console is clean, no `ReferenceError` / `undefined`.

If chrome-devtools MCP is unavailable (a parallel session can hold the Chrome profile), fall back to `tools/ui-audit`, which drives `puppeteer-core` directly.

- [x] **Step 9: Commit**

```sh
git add -A
git commit -m "refactor(naming): rename the silent contracts qmd -> tali

Browser storage keys, the themechange CustomEvent, the deck ?qmd= param, the
LAN-share cookie, and the editor postMessage protocol. None of these are
observable from cargo test; verified by hand in the browser. Saved reader
themes and read positions from before this commit are discarded by design."
```

---

### Task 3: Rename `data-qmd-*` to `data-tali-*`  ⬅ RESUME HERE

15 attributes, 162 hits across 39 files, spanning Rust emitters, bundled CSS, bundled JS and the preview client. Task 1's census turns this from an invisible change into a required diff.

**Files:**
- Modify: every file matching `rg -l 'data-qmd-' crates web-client corpus site docs/guide docs/internals tools` (39 files)
- Modify: `crates/core/tests/token_contract.rs` (`EMITTED_DATA_ATTRS`)

**Interfaces:**
- Consumes: `EMITTED_DATA_ATTRS` from Task 1.
- Produces: the `data-tali-*` vocabulary Task 4 and Task 6 assume.

The full attribute set:
`data-qmd-bound`, `data-qmd-cell-source`, `data-qmd-cell-state`, `data-qmd-done`, `data-qmd-drawer-close`, `data-qmd-input`, `data-qmd-input-bound`, `data-qmd-lb`, `data-qmd-out`, `data-qmd-ran`, `data-qmd-search`, `data-qmd-src`, `data-qmd-theme-toggle`, `data-qmd-theorem-kind`, `data-qmd-xref`.

- [ ] **Step 1: Rename all 15, all languages, in one pass**

```sh
rg -l 'data-qmd-' crates web-client corpus site docs/guide docs/internals tools \
  -g '!*.min.js' -g '!_book/**' -g '!_site/**' \
  | xargs sed -i 's/data-qmd-/data-tali-/g'
```

This narrow substitution is safe (unlike a blanket `qmd` → `tali`) because `data-qmd-` is unambiguous and has no `data-tali-` counterpart to collide with. Verify nothing else moved:

```sh
git diff --stat
git diff | grep -E '^[+-]' | grep -v 'data-\(qmd\|tali\)-' | grep -vE '^(\+\+\+|---)'
```

Expected: the second command prints nothing.

- [ ] **Step 2: Run the census and watch it fail**

```sh
cargo test -p taliesin-core --test token_contract 2>&1 | head -60
```

Expected: `emitted_data_attribute_census_is_pinned` FAILS listing the new `data-tali-*` names. **This failure is the point of Task 1.** If it passes, the census is not wired to the emitter; stop and fix Task 1 before continuing.

- [ ] **Step 3: Update the census**

Paste the new `ACTUAL:` list into `EMITTED_DATA_ATTRS`, and update any `NO_RUNTIME_CONSUMER` entries to the new names.

- [ ] **Step 4: Confirm the orphan detector passes**

```sh
cargo test -p taliesin-core --test token_contract
```

Expected: both tests PASS. A failure here means the Rust emitter moved but a CSS or JS consumer did not, which is exactly the defect this net exists to catch.

- [ ] **Step 5: Build, type-check, full suite**

```sh
cargo build --workspace
cd web-client && npx -y -p typescript tsc -p jsconfig.json && cd ..
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json && cd ../../../..
cargo test --workspace --no-fail-fast 2>&1 | grep -E '^test result'
```

Expected: clean. The 4 files in `crates/core/tests/snapshots/` were rewritten by the `sed` above; if a snapshot test now fails it is because a corpus `.tmd` also changed, which is handled in Task 7. Note any such failure and carry it forward rather than re-blessing here.

- [ ] **Step 6: Commit**

```sh
git add -A
git commit -m "refactor(naming): data-qmd-* -> data-tali-*

15 attributes across Rust emitters, bundled CSS/JS, and the preview client.
The census pin in token_contract.rs is the required visible diff."
```

---

### Task 4: Rename the `{js}` runtime script types and drop the `qmd` cell-API alias

**Files:**
- Rename: `crates/core/assets/js/qmd-js.js` → `crates/core/assets/js/tali-js.js`
- Modify: `crates/core/src/render/mod.rs:1635,1640,1651`, `crates/core/tests/third_party.rs:11`, `crates/core/assets/js/jsconfig.json:2,35`, `.github/workflows/ci.yml:117`
- Modify: `crates/server/src/kernel.rs:88,109`, `crates/server/src/freeze.rs:62`, `crates/server/src/protocol.rs:127`, `crates/server/src/serve/mod.rs:1399`, `crates/server/src/serve_site/mod.rs:1045`, `crates/server/src/headless_js.rs`
- Modify: `crates/core/tests/corpus.rs:889,891,932`, `crates/server/tests/asset_bundle.rs:203`
- Modify: `crates/server/src/freeze.rs` (`CACHED_OUTPUT_TOKENS` + `FORMAT_VERSION`)

**Interfaces:**
- Consumes: the freeze pin from Task 1.
- Produces: `application/tali-js` and `tali-define`, which Task 7's corpus edits reference in prose.

- [ ] **Step 1: Rename the file and every reference to it**

```sh
git mv crates/core/assets/js/qmd-js.js crates/core/assets/js/tali-js.js
```

Then update `render/mod.rs:1640` (`include_str!`), `third_party.rs:11` (`OWN_JS`), `jsconfig.json:2,35`, `ci.yml:117`, and the comment at `render/mod.rs:1635`. A missed `include_str!` is the one thing the compiler catches here.

- [ ] **Step 2: Rename the script types and the Jupyter metadata key**

`application/qmd-js` → `application/tali-js` and `qmd-define` → `tali-define` everywhere, plus `metadata=dict(qmd_define=True)` → `metadata=dict(tali_define=True)` at `kernel.rs:109`.

Note: `qmd_define` has exactly one occurrence and nothing on the Rust side reads it. Rename it rather than delete it, and open a follow-up to confirm whether it is dead; deleting it is out of scope here.

- [ ] **Step 3: Delete the `qmd` cell-API parameter**

In `tali-js.js` around line 222, the cell function is constructed as:

```js
var fn = new AsyncFunction(
  "tali", "qmd", "Plot", "d3", "container", "invalidation",
  src
);
```
and invoked as `await fn(api, api, window.Plot, window.d3, container, currentInv)`.

Delete the `"qmd"` parameter and its duplicate `api` argument, and delete the two-line comment above them:

```js
var fn = new AsyncFunction(
  "tali", "Plot", "d3", "container", "invalidation",
  src
);
```
```js
var node = await fn(api, window.Plot, window.d3, container, currentInv);
```

Do **not** rename `"qmd"` to `"tali"`: that yields a duplicate parameter name, which is legal in sloppy mode but a `SyntaxError` in strict mode (verified 2026-07-25), so any `{js}` cell whose author writes `"use strict"` would die at page load with no test-visible signal.

- [ ] **Step 4: Update the freeze pin and bump the version**

In `crates/server/src/freeze.rs`, update `CACHED_OUTPUT_TOKENS` to `"application/tali-js"` and `"tali-define"`, set `const FORMAT_VERSION: u32 = 4;`, and extend the doc comment above `FORMAT_VERSION`:

```rust
/// v4: the `{js}` runtime script types became `application/tali-js` / `tali-define`
/// (was `qmd-*`); entries cached before that rename carry the old types, which the
/// current runtime's exact-match selectors never ingest, so `{js}` cells would
/// silently receive no data.
```

- [ ] **Step 5: Run the freeze pin, watch it fail, then fix the digest**

```sh
cargo test -p taliesin-server --lib freeze::tests::cached_output_vocabulary
```

Expected: FAILS with the new digest and `FORMAT_VERSION` 4. Paste both values in. Re-run; expect PASS. **This failure is the point of the pin.** If it passes without editing, the pin is not wired up.

- [ ] **Step 6: Build, type-check, full suite**

```sh
cargo build --workspace
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json && cd ../../../..
cargo test --workspace --no-fail-fast 2>&1 | grep -E '^test result'
```

Expected: clean.

- [ ] **Step 7: Verify `{js}` cells actually still execute**

```sh
cargo build --release
rm -rf _freeze
./target/release/taliesin build corpus/reactive/inputs.tmd --out /tmp/jscheck
python3 -m http.server 4388 --directory /tmp/jscheck &
```

Via chrome-devtools MCP: the reactive inputs render and respond to the slider, and the console is clean. A blank stage with no error is the signature of a missed script-type rename.

- [ ] **Step 8: Commit**

```sh
git add -A
git commit -m "refactor(naming): tali-js.js, application/tali-js, tali-define; drop the qmd cell param

FORMAT_VERSION 3 -> 4: cached cell output carries these script types verbatim and
the key hashes source, not output, so old _freeze/ entries would replay markup the
runtime cannot read. The qmd cell-API parameter is deleted rather than renamed;
renaming it produces a duplicate parameter name that is a SyntaxError under
\"use strict\"."
```

---

### Task 5: Resolve the namespace collisions and delete the back-compat aliases

Six tokens exist in both `qmd-*` and `tali-*` form today, and seven `window.qmd*` globals are aliases pointing at `window.tali*`. Merging them naively either creates self-assignments or destroys a test's discriminator.

**Files:**
- Modify: `crates/core/assets/js/tali-js.js:342`, `crates/core/assets/js/deck.js:2620`, `crates/core/assets/js/code-enhance/01-registry.js:35,39`, and the remaining `window.qmdInit*` alias sites
- Modify: `crates/core/assets/js/globals.d.ts:17,19,35,43`, `web-client/globals.d.ts`
- Modify: `crates/core/src/cite/render.rs:108,151`, `crates/core/src/site/cite_this.rs`, and the `qmd-title-block` / `qmd-footnotes` emitters
- Modify: `crates/core/tests/cite_this.rs:60,81`, plus any test asserting on the affected ids

**Interfaces:**
- Consumes: `data-tali-*` from Task 3.
- Produces: block ids `tali-references`, `tali-footnotes`, `tali-cite-this`, `tali-title-block`.

- [ ] **Step 1: Delete the seven dead aliases outright**

```js
window.qmdJs = window.taliJs;                    // tali-js.js:342
window.QmdDeck = window.TaliesinDeck;            // deck.js:2620
window.qmdEnhancers = window.taliEnhancers;      // 01-registry.js:35
window.qmdEnhanceCode = window.taliEnhanceCode;  // 01-registry.js:39
```
plus `window.qmdInitAnchorLinks`, `window.qmdInitFocusMode`, `window.qmdInitReadingProgress`, `window.qmdInitSkipLink`.

Delete each assignment line and its explanatory comment, and delete the matching optional properties from `globals.d.ts` (`qmdEnhanceCode?`, `qmdEnhancers?`, `qmdJs?`, `QmdDeck?` and the `qmdInit*` entries). Renaming instead of deleting produces `window.taliJs = window.taliJs;` (verified 2026-07-25), which silently removes the compatibility the line existed for while looking correct.

- [ ] **Step 2: Rename the four block ids**

`qmd-references` → `tali-references`, `qmd-footnotes` → `tali-footnotes`, `qmd-cite-this` → `tali-cite-this`, `qmd-title-block` → `tali-title-block`. In `cite/render.rs:108,151` this makes the line read `class=\"tali-references\" data-block-id=\"tali-references\"`, which is correct: the id and the class name the same block.

- [ ] **Step 3: Make the ambiguous assertions precise**

The rename makes a bare substring assertion ambiguous, because `html.contains("tali-cite-this")` now matches both the class (which the always-inlined enhancer JS contains unconditionally) and the id. Rewrite each to assert on the full attribute.

`crates/core/tests/cite_this.rs:60` and `:81`, and its comment:

```rust
    // Key on the full block-id ATTRIBUTE: the `tali-cite-this` class string also
    // appears in the always-inlined enhancer JS, so only `data-block-id="…"`
    // proves the generated block is absent.
    assert!(
        !html.contains("data-block-id=\"tali-cite-this\""),
        "a page without a date must render no citation box"
    );
```

- [ ] **Step 4: Audit for other newly-ambiguous assertions**

```sh
rg -n 'contains\("tali-(cite-this|references|footnotes|title-block|input|search|xref)"' crates
```

For each hit, decide whether the bare class substring is still the intended assertion; if the test means the block, switch it to `data-block-id="…"` or `data-tali-…="…"`. Record the decision in a one-line comment on each edited assertion.

- [ ] **Step 5: Build, type-check, full suite**

```sh
cargo build --workspace
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json && cd ../../../..
cd web-client && npx -y -p typescript tsc -p jsconfig.json && cd ..
cargo test --workspace --no-fail-fast 2>&1 | grep -E '^test result'
```

Expected: clean. `cite_this.rs`'s two tests are the ones that caught this collision in the 2026-07-25 blanket-sed experiment; they must pass on their new assertions.

- [ ] **Step 6: Commit**

```sh
git add -A
git commit -m "refactor(naming): drop the qmd->tali back-compat aliases, merge the colliding ids

The seven window.qmd* globals are deleted, not renamed: renaming yields
self-assignments. The four block ids merge into the tali-* namespace, and the
assertions that relied on id/class being different namespaces now key on the
full data-block-id attribute."
```

---

### Task 6: Rename the Python identifiers injected into the kernel

~20 `_qmd_*` names plus `_QMD_LIGHT` / `_QMD_DARK`, defined and referenced inside Python source embedded in Rust string literals. The compiler sees none of it.

**Files:**
- Modify: `crates/server/src/kernel.rs:147-266` (the matplotlib theming + export prelude)
- Modify: `crates/server/src/exec.rs:959,1031,1048,1053`

- [ ] **Step 1: Rename every `_qmd_` / `_QMD_` identifier**

```sh
sed -i 's/_qmd_/_tali_/g; s/_QMD_/_TALI_/g' crates/server/src/kernel.rs crates/server/src/exec.rs
```

This substitution is safe: `_qmd_` is a unique prefix with no `_tali_` counterpart. Confirm the diff touches only identifiers:

```sh
git diff | grep -E '^[+-]' | grep -v '_\(qmd\|tali\|QMD\|TALI\)_' | grep -vE '^(\+\+\+|---)'
```

Expected: prints nothing.

- [ ] **Step 2: Check for split identifiers the `sed` could not see**

```sh
rg -n 'qmd' crates/server/src/kernel.rs crates/server/src/exec.rs
```

Expected: only the `qmd-define`-era comments already handled in Task 4, or nothing. Any Python identifier built by string concatenation (`"_qmd" + "_export"`) would survive the `sed`; there are none today, but confirm.

- [ ] **Step 3: Build and run the kernel-dependent tests against a fresh kernel**

A warm kernel from before this change still holds the old `_qmd_*` definitions, so a stale kernel can mask a broken rename. Kill any running preview first (by PID; `pkill -f 'taliesin preview'` kills the shell).

```sh
cargo build --workspace
rm -rf _freeze
TALIESIN_PYTHON=~/.local/share/qmd-venv/bin/python cargo test --workspace --no-fail-fast 2>&1 | grep -E '^test result'
```

Expected: clean.

- [ ] **Step 4: Execute a real figure-producing document end to end**

```sh
rm -rf _freeze
./target/release/taliesin build corpus/posts/pca-geometry/index.tmd --out /tmp/pycheck
```

Expected: exit 0, and `/tmp/pycheck/index.html` contains both `tali-fig-light` and `tali-fig-dark`. That proves `_tali_render` / `_tali_recolour` / `_tali_install` all still resolve inside the kernel. A `NameError` in a cell's output is the signature of a partial rename.

- [ ] **Step 5: Commit**

```sh
git add -A
git commit -m "refactor(naming): _qmd_* -> _tali_* in the injected Python prelude

These identifiers live in Python embedded in Rust string literals, so nothing
type-checks them. Verified by a cold-kernel build of a matplotlib document."
```

---

### Task 7: Purge the remaining prose, corpus cell code, tooling and CI

Everything left: corpus `.tmd` documents (including `{js}` cell code that calls the API), the two dogfooded books, the marketing site, `tools/`, the CI workflow, `web-client/README.md`, `corpus/README.md`, and `crates/core/src/site/CLAUDE.md`.

**Files:**
- Modify: all remaining files from `rg -l -i qmd` minus the exclusions in Global Constraints
- Modify: `crates/core/tests/snapshots/*.html` (re-bless)
- Modify: `crates/core/tests/corpus.rs` (`collect_qmd` → `collect_tmd`, 6 call sites)
- Modify: `docs/guide/reference/troubleshooting.tmd:136-138` (delete the stale `qmdFast.path` sentence)
- Modify: `editor/vscode/src/test/manifest.test.ts:116-131`

- [ ] **Step 1: Rename the corpus cell-API calls**

```sh
rg -l '\bqmd\.' corpus site docs/guide docs/internals | xargs sed -i 's/\bqmd\./tali./g'
```

`tali` is already the primary parameter name, so this is a straight swap onto the surviving binding. Then handle the remaining prose by hand: `rg -i qmd corpus site docs/guide docs/internals` and edit each hit in context, since prose needs rewording, not substitution.

- [ ] **Step 2: Fix the dead `--qmd-*` custom properties in `site/showcase.tmd` (a real bug, not a rename)**

`site/showcase.tmd:183-232` (8 lines, in two near-identical `{js}` cells) build inline `style.cssText` strings referencing `var(--qmd-font-head)`, `var(--qmd-accent)`, `var(--qmd-border)`, `var(--qmd-code-bg)`, `var(--qmd-fg)`, `var(--qmd-muted)`.

**None of those custom properties are defined anywhere** (verified 2026-07-25: the real tokens are `--tali-*`, defined in `assets/css/tokens.css:18,79` and `base.css:871`). So these inline styles silently fall back to no colour and no font today. Rename them to `--tali-*`, which fixes the bug as a side effect.

This is the only live `--qmd-*` usage in the tree; the other ~140 occurrences are all in the frozen `docs/superpowers/` archive.

- [ ] **Step 3: Fix the stale `qmdFast` references**

At `docs/guide/reference/troubleshooting.tmd:136-138`, delete the sentence describing the `qmdFast.path` setting and its `qmd-fast` default; the live setting is `taliesin.path`. In `editor/vscode/src/test/manifest.test.ts`, the stale-branding guard's own regex contains `qmd-fast|qmdFast|qmdfast` and its `allowed` list contains `qmd-goto|qmd-cursor|getElementById\("qmd"\)|id="qmd"`. Update `allowed` to the Task 2 names (`tali-goto|tali-cursor|getElementById\("tali-preview"\)|id="tali-preview"`) but **leave the offender regex matching `qmd-fast`**, since it guards against the pre-rename branding returning and must keep naming it. Add a comment saying so, and add the file to the Task 9 allowlist.

- [ ] **Step 4: Rename the test helper and the remaining Rust prose**

`collect_qmd` → `collect_tmd` in `crates/core/tests/corpus.rs` (declaration plus 6 call sites). Then sweep `rg -i qmd crates` and fix every remaining comment.

- [ ] **Step 5: Rename in `tools/` and CI**

`tools/record-demo/record.mjs`, `tools/ui-audit/lib/browser.mjs`, `tools/ui-audit/lib/probe.mjs`, `tools/ui-audit/probe-run.mjs`, `.github/workflows/ci.yml:117`. The ui-audit probes select on `data-qmd-done` / `data-qmd-ran`; those became `data-tali-*` in Task 3, so these are already-broken selectors being repaired.

- [ ] **Step 6: Re-bless the four snapshots**

Corpus `.tmd` source text changed, so every affected block's content-hash id changed. Regenerate rather than hand-edit:

```sh
UPDATE_SNAPSHOTS=1 cargo test -p taliesin-core --test body_html_snapshots
```

(Confirmed 2026-07-25 at `body_html_snapshots.rs:62`. It is `UPDATE_SNAPSHOTS`, not a `*_BLESS` variable.)

Then inspect the diff and confirm the only changes are block ids and the renamed tokens. An unreviewed snapshot update pins whatever the bug is:

```sh
git diff crates/core/tests/snapshots/
```

Only three of the four snapshots should move; `reactive_js_error.html` has just 2 `qmd` hits and may not change at all.

- [ ] **Step 7: Full suite, type-checks, and a corpus build**

```sh
cargo build --workspace
cd web-client && npx -y -p typescript tsc -p jsconfig.json && cd ..
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json && cd ../../../..
cargo test --workspace --no-fail-fast 2>&1 | grep -E '^test result'
cargo build --release && ./target/release/taliesin build docs/guide --out /tmp/guidecheck
```

Expected: all clean, guide builds.

- [ ] **Step 8: Verify the marketing showcase in a browser**

Step 2 changed live styling, so this needs eyes:

```sh
./target/release/taliesin build site --out /tmp/sitecheck
python3 -m http.server 4388 --directory /tmp/sitecheck &
```

Via chrome-devtools MCP, open `showcase.html` and confirm the two `{js}` exhibits now render their buttons with the accent background, border and heading font. They were unstyled before this task because `--qmd-*` resolved to nothing. Screenshot at all three viewport sizes: mobile (~390×844), laptop landscape (~1440×900), laptop portrait (~900×1440).

- [ ] **Step 9: Commit**

```sh
git add -A
git commit -m "refactor(naming): purge qmd from corpus, docs, site, tools and CI

Corpus {js} cells now call tali.* directly. Snapshots re-blessed for the
block-id churn that editing the source text causes. docs/superpowers/ and
notes/ are deliberately untouched as the pre-rename record.

Also fixes site/showcase.tmd, whose two {js} exhibits referenced var(--qmd-*)
custom properties that were never defined, so their inline styles had been
silently falling back to nothing."
```

---

### Task 8: Repackage and reinstall the VS Code extension

The extension is a separately-packaged `.vsix`. An installed build from before Task 2 speaks `qmd-goto` / `qmd-cursor` and will silently fail to talk to the new preview client. This has bitten before (`check produced unexpected output` from a stale companion after a source fix).

**Files:**
- Modify: none expected; this task verifies and repackages.

- [ ] **Step 1: Confirm no `qmd` remains in the extension source**

```sh
rg -n -i qmd editor/vscode/src editor/vscode/scripts editor/vscode/package.json editor/vscode/README.md
```

Expected: only the deliberate `qmd-fast` stale-branding guard in `manifest.test.ts` from Task 7 Step 2.

- [ ] **Step 2: Run the extension's own tests**

```sh
cd editor/vscode && npm run compile && npm test && cd ../..
```

Expected: green, including `manifest.test.ts`.

- [ ] **Step 3: Exercise the relay harness**

```sh
cd editor/vscode && node scripts/relay-harness.cjs && cd ../..
```

Expected: it asserts `tali-goto` from the iframe reaches the host and `tali-cursor` from the host reaches the iframe (see the script header for the harness contract). Both directions must pass; a one-directional pass means half the protocol was renamed.

- [ ] **Step 4: Repackage and reinstall**

```sh
cd editor/vscode && npx -y @vscode/vsce package && cd ../..
```

Uninstall the currently-installed `taliesin.taliesin-companion`, install the new `.vsix`, and reload the window. Skipping this leaves a stale extension speaking the old wire names.

- [ ] **Step 5: Verify click-to-source round-trips in the real editor**

Open a `.tmd`, run `taliesin.openPreview`, then confirm both directions by hand:
- Alt-click a block in the preview moves the editor cursor to its source line (`tali-goto`).
- Moving the editor cursor highlights the corresponding block in the preview (`tali-cursor`).

Click-to-source is one of the three load-bearing goals; do not mark this task done on the harness alone.

- [ ] **Step 6: Commit**

```sh
git add -A
git commit -m "chore(editor): repackage the companion for the tali-goto/tali-cursor wire names

The installed vsix is a separate artifact from the source; a stale one speaks the
old protocol and fails silently. Click-to-source verified by hand in both
directions."
```

---

### Task 9: Lock it shut

**Files:**
- Create: `crates/core/tests/no_qmd.rs`
- Modify: `CLAUDE.md`
- Modify: `.github/workflows/ci.yml` (only if the new test is not already covered by `cargo test --workspace`)

- [ ] **Step 1: Write the repo-wide guard**

Create `crates/core/tests/no_qmd.rs`:

```rust
//! `qmd` is a retired name. This asserts it stays retired.
//!
//! `docs/superpowers/` and `notes/` are deliberately exempt: they are the dated
//! plan/spec archive and the pre-rename record, and rewriting them would make a
//! 2026-06 document claim it used names that did not exist yet.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Directories never scanned: build output, vendored deps, and the frozen record.
const SKIP_DIRS: &[&str] = &[
    ".git", "target", "_site", "_book", "_freeze", "node_modules",
    ".vscode-test", "docs/superpowers", "notes",
];

/// Files that must keep the retired name, each for a stated reason.
const ALLOWED_FILES: &[(&str, &str)] = &[(
    "editor/vscode/src/test/manifest.test.ts",
    "guards against the pre-rename `qmd-fast` branding returning, so it must name it",
)];

fn walk(dir: &Path, root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
        if SKIP_DIRS.iter().any(|s| rel == *s || rel.starts_with(&format!("{s}/"))) {
            continue;
        }
        if p.is_dir() {
            walk(&p, root, out);
        } else {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name.ends_with(".min.js") || name == "package-lock.json" {
                continue;
            }
            out.push(p);
        }
    }
}

#[test]
fn the_retired_qmd_name_stays_retired() {
    let root = repo_root().canonicalize().expect("repo root");
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    assert!(files.len() > 100, "the walker found only {} files; check SKIP_DIRS", files.len());

    let allowed: Vec<&str> = ALLOWED_FILES.iter().map(|(f, _)| *f).collect();
    let mut offenders = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&root).unwrap_or(f).to_string_lossy().replace('\\', "/");
        if allowed.contains(&rel.as_str()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(f) else { continue }; // skip binaries
        for (i, line) in text.lines().enumerate() {
            if line.to_ascii_lowercase().contains("qmd") {
                offenders.push(format!("{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "`qmd` is retired; use `tali`. {} occurrence(s):\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}
```

- [ ] **Step 2: Run it**

```sh
cargo test -p taliesin-core --test no_qmd 2>&1 | head -60
```

Expected: PASS. If it lists offenders, they are genuine misses from Tasks 2 through 7; fix them in place rather than widening `ALLOWED_FILES`. Only add an allowlist entry when the file must *name* the retired token to do its job, and always with a reason string.

- [ ] **Step 3: Update CLAUDE.md**

Three edits, all of which currently describe the old names:
- In the `render/deck.rs` line, replace the sentence beginning "`window.QmdDeck` is a back-compat alias only" with a note that the alias was removed and `window.TaliesinDeck` is the only name.
- In the `render/theme.rs` line, replace "The `qmd-theme` storage key + `qmd:themechange` event keep their frozen runtime names" with "The storage key is `tali-theme` and the event is `tali:themechange`; `crates/core/tests/no_qmd.rs` keeps the retired `qmd` spelling out of the tree."
- In the `assets/js` line, change `qmd-js.js` to `tali-js.js`.

Note that `crates/core/tests/stale_docs.rs` already asserts CLAUDE.md does not list deleted machinery; consider adding a matching assertion there that CLAUDE.md no longer says `qmd-theme`.

- [ ] **Step 4: Confirm CI covers the new tests**

```sh
rg -n 'cargo test' .github/workflows/ci.yml
```

If CI runs `cargo test --workspace`, `no_qmd` and `token_contract` are already covered and no change is needed. If it names test binaries individually, add both.

- [ ] **Step 5: Final full verification**

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast 2>&1 | grep -E '^test result'
cd web-client && npx -y -p typescript tsc -p jsconfig.json && cd ..
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json && cd ../../../..
```

Expected: all clean. If the suite flakes at full parallelism (`kernel_executes_..._runaway_cell` is a known flake), re-run the failing binary with `--test-threads=1` before concluding anything is broken.

- [ ] **Step 6: Commit**

```sh
git add -A
git commit -m "test: assert the retired qmd name stays retired; update CLAUDE.md

Repo-wide guard with an explicit allowlist. docs/superpowers/ and notes/ are
exempt as the pre-rename record. CLAUDE.md no longer claims qmd-theme and
qmd:themechange are frozen names."
```

---

## Verification Summary

What each layer actually covers, so nothing is assumed:

| Layer | Covers | Blind to |
|---|---|---|
| `cargo build` | `include_str!` paths, Rust identifiers | every string-literal contract |
| `tsc` (both jsconfigs) | JS globals declared in `globals.d.ts` | selector strings, storage keys, event names |
| `token_contract.rs` | emitted `data-*` vocabulary + missing CSS/JS consumers | anything not a `data-*` attribute |
| `freeze.rs` pin | forgetting a `FORMAT_VERSION` bump | non-cached output |
| `no_qmd.rs` | reintroduction of the retired name | whether the *new* name is wired correctly |
| browser pass (Tasks 2, 4) | storage, events, URL params, `{js}` execution | nothing else covers these at all |
| relay harness + manual (Task 8) | the cross-process wire protocol | nothing else covers this at all |

The two browser tasks and the manual click-to-source check are not optional garnish; they are the only coverage that exists for those families.
