# Book-level `theorems:` config Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a book set its theorem-numbering policy once in `_site.yml`; chapters inherit it unless they declare their own `theorems:` block.

**Architecture:** Add `theorems: Option<TheoremConfig>` to `SiteConfig` (recognized `_site.yml` key). Thread a book-level `Option<&TheoremConfig>` through the private render chain as a fallback used only when a page has no `theorems:` block. Public render API unchanged; standalone docs render identically.

**Tech Stack:** Rust (taliesin-core), serde_yaml, comrak.

## Global Constraints

- No em/en dashes in any authored prose, comments, or fixtures. Commas/colons/parentheses instead.
- Preserve every block's `data-block-id` + `data-sourcepos` (no front-matter injection).
- Do not touch `number_theorems`, the BibTeX/CSL path, or `MAX_WARM_PAGES`/`exec_pool.rs`.
- Verify each test by mutation (restore the bug, watch the named test fail) before committing.
- Merge semantics: a page's own `theorems:` block (even `theorems: {}`) fully overrides the book's; no per-field merge.
- Run tests with the three gates when running the full suite: `TALIESIN_REQUIRE_NODE=1`, `TALIESIN_R=R TALIESIN_REQUIRE_R=1`, `TALIESIN_PYTHON=~/.local/share/qmd-venv/bin/python TALIESIN_REQUIRE_KERNEL=1`. Task-scoped `cargo test -p taliesin-core <filter>` needs no kernel.

---

### Task 1: `TheoremConfig` derives + shared value/fallback parsers

**Files:**
- Modify: `crates/core/src/render/fm_extract.rs:216-273`
- Test: `crates/core/src/render/fm_extract.rs` (inline `#[cfg(test)]` or the module's existing tests)

**Interfaces:**
- Produces: `pub(crate) fn parse_theorem_config_value(value: &serde_yaml::Value) -> TheoremConfig`; `pub(crate) fn theorem_config_with_fallback(fm: &str, book: Option<&TheoremConfig>) -> TheoremConfig`; `TheoremConfig: Clone + Debug + PartialEq`.
- Consumes: nothing new.

- [ ] **Step 1: Write the failing test**

Add to `fm_extract.rs` tests:

```rust
#[test]
fn theorem_fallback_prefers_page_then_book_then_default() {
    let book = TheoremConfig { shared: vec![], numbered: Numbered::No };
    // Page with no `theorems:` inherits the book config.
    let none_page = "---\ntitle: X\n---";
    assert_eq!(theorem_config_with_fallback(none_page, Some(&book)).numbered(), Numbered::No);
    // Page with its own `theorems:` overrides the book.
    let own_page = "---\ntheorems:\n  numbered: true\n---";
    assert_eq!(theorem_config_with_fallback(own_page, Some(&book)).numbered(), Numbered::Yes);
    // No book, no page config -> default (Yes).
    assert_eq!(theorem_config_with_fallback(none_page, None).numbered(), Numbered::Yes);
}
```

- [ ] **Step 2: Run it, expect a compile error** (functions/derives don't exist yet)

Run: `cargo test -p taliesin-core --lib fm_extract::tests::theorem_fallback_prefers_page_then_book_then_default 2>&1 | tail -5`
Expected: does not compile (`theorem_config_with_fallback` not found).

- [ ] **Step 3: Implement**

In `fm_extract.rs`:
- Change `#[derive(Default)]` on `TheoremConfig` (line 216) to `#[derive(Default, Clone, Debug, PartialEq)]`.
- Extract the value-level logic and add the fallback wrapper; redefine `parse_theorem_config`:

```rust
/// Parse a `theorems:` block out of an already-parsed YAML value.
pub(crate) fn parse_theorem_config_value(value: &serde_yaml::Value) -> TheoremConfig {
    let mut config = TheoremConfig::default();
    if let Some(shared) = value
        .get("theorems")
        .and_then(|t| t.get("shared"))
        .and_then(|s| s.as_sequence())
    {
        config.shared = shared
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
    }
    match value.get("theorems").and_then(|t| t.get("numbered")) {
        Some(serde_yaml::Value::Bool(false)) => config.numbered = Numbered::No,
        Some(v) if v.as_str() == Some("unless-unique") => config.numbered = Numbered::UnlessUnique,
        _ => {}
    }
    config
}

/// The effective theorem config for a page: its own `theorems:` block if the
/// front-matter declares one (even an empty `theorems: {}`), else `book` when
/// present, else the default.
pub(crate) fn theorem_config_with_fallback(fm: &str, book: Option<&TheoremConfig>) -> TheoremConfig {
    let body = fm.trim();
    let body = body.strip_prefix("---").unwrap_or(body);
    let body = body.strip_suffix("---").unwrap_or(body);
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(body) else {
        return book.cloned().unwrap_or_default();
    };
    if value.get("theorems").is_some() {
        parse_theorem_config_value(&value)
    } else {
        book.cloned().unwrap_or_default()
    }
}

/// Parse the `theorems:` block out of a front-matter string (no book fallback).
pub(crate) fn parse_theorem_config(front_matter: &str) -> TheoremConfig {
    theorem_config_with_fallback(front_matter, None)
}
```

Note: a YAML parse failure with a page that had no config now yields the book's config (or default) rather than always default; this only changes behavior when a book config exists, which is the intended fallback.

- [ ] **Step 4: Run the test, expect PASS**

Run: `cargo test -p taliesin-core --lib fm_extract 2>&1 | tail -8`
Expected: the new test + existing `fm_extract` tests pass.

- [ ] **Step 5: Mutation-check + commit**

Temporarily make `theorem_config_with_fallback` ignore `book` (always `parse_theorem_config_value` on the page or default) -> the "inherits the book config" assert fails. Restore.

```bash
git add crates/core/src/render/fm_extract.rs
git commit -m "refactor(render): theorem_config_with_fallback + value parser (item 16 F-01)"
```

---

### Task 2: Thread a book-level config through the render chain

**Files:**
- Modify: `crates/core/src/render/mod.rs:146-244` (entries + private chain), `:388` (merge site)
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `theorem_config_with_fallback`, `TheoremConfig` (Task 1).
- Produces: `pub(crate) fn render_document_scoped_with_theorems(src: &str, base_dir: &Path, chapter: Option<u32>, book_theorems: Option<&TheoremConfig>) -> RenderedDoc`.

- [ ] **Step 1: Write the failing test** (in `render/tests.rs`)

```rust
#[test]
fn book_theorem_config_is_a_fallback_for_a_pageless_of_its_own() {
    use super::fm_extract::{Numbered, TheoremConfig};
    let src = "::: {.theorem title=\"T\"}\nbody\n:::\n"; // no front-matter `theorems:`
    let book = TheoremConfig { shared: vec![], numbered: Numbered::No };
    let base = std::path::Path::new(".");
    // With the book fallback (numbered: false) the number span is empty.
    let doc = render_document_scoped_with_theorems(src, base, None, Some(&book));
    let html = doc.body_html();
    assert!(html.contains(r#"tali-theorem-number"></span>"#), "book policy applied:\n{html}");
    // Without it, the default (numbered) fills the span.
    let plain = render_document_scoped_with_theorems(src, base, None, None);
    assert!(plain.body_html().contains(r#"tali-theorem-number">&nbsp;1</span>"#), "default numbers");
}
```

(If `body_html()` is not the accessor, use the same one `render/tests.rs` already uses to read a doc's HTML; grep the file first.)

- [ ] **Step 2: Run it, expect a compile error**

Run: `cargo test -p taliesin-core --lib render::tests::book_theorem_config_is_a_fallback_for_a_pageless_of_its_own 2>&1 | tail -5`
Expected: `render_document_scoped_with_theorems` not found.

- [ ] **Step 3: Implement the threading**

In `mod.rs`:
- Add the entry (after `render_document_with_includes_scoped`, ~line 159):

```rust
/// Like [`render_document_with_includes_scoped`] but with a book-level `theorems:`
/// fallback (the site book path passes `Some`; a page without its own `theorems:`
/// block inherits it). Everything else passes `None`, unchanged.
pub(crate) fn render_document_scoped_with_theorems(
    src: &str,
    base_dir: &Path,
    chapter: Option<u32>,
    book_theorems: Option<&TheoremConfig>,
) -> RenderedDoc {
    render_doc_with_includes_impl(src, base_dir, chapter, None, book_theorems)
}
```

- Change `render_document_with_includes_scoped` (line 153-159) to delegate with `None`:
  `render_doc_with_includes_impl(src, base_dir, chapter, None, None)`.
- Change `render_document_with_includes_rooted` (line 168-174) call to pass `None` for the new arg.
- Add `book_theorems: Option<&TheoremConfig>` as the last parameter of `render_doc_with_includes_impl` (line 176-181), `render_internal` (line 213-219), and `render_internal_impl` (line 238-243). Thread it through unchanged at each call (lines 189, 224, 233). The `std::thread::scope` closure (line 223) captures `book_theorems` by reference; its lifetime outlives the scoped thread, so this compiles.
- At the merge site (line 388), replace:
  `theorem_config = parse_theorem_config(fm);`
  with:
  `theorem_config = theorem_config_with_fallback(fm, book_theorems);`
- Add `theorem_config_with_fallback, render::fm_extract::TheoremConfig` (already imported as `TheoremConfig` at line 48) and `render_document_scoped_with_theorems` visibility to any `use`/re-export as needed. `theorem_config_with_fallback` import: extend the `fm_extract` `use` (line 48-50) to include it.

- [ ] **Step 4: Run the test, expect PASS**

Run: `cargo test -p taliesin-core --lib render::tests::book_theorem_config_is_a_fallback_for_a_pageless_of_its_own 2>&1 | tail -8`
Then the whole render suite: `cargo test -p taliesin-core --lib render 2>&1 | tail -8` (no regressions).

- [ ] **Step 5: Mutation-check + commit**

Temporarily hard-code `book_theorems = None` inside `render_document_scoped_with_theorems` -> the book-policy assert fails. Restore.

```bash
git add crates/core/src/render/mod.rs
git commit -m "feat(render): book-level theorems fallback threaded through scoped render (item 16 F-01)"
```

---

### Task 3: Recognize + parse `theorems:` in `_site.yml`

**Files:**
- Modify: `crates/core/src/site/config/mod.rs:117` (NATIVE_KEYS), the `SiteConfig` struct (~line 60), `parse_native` (line 222-250)
- Modify: `crates/core/src/frontmatter.rs:325` (`validate_theorem_values` -> `pub(crate)`)
- Test: `crates/core/src/site/config/mod.rs` tests (near line 482)

**Interfaces:**
- Consumes: `parse_theorem_config_value` (Task 1), `crate::frontmatter::validate_theorem_values`.
- Produces: `SiteConfig.theorems: Option<TheoremConfig>`.

- [ ] **Step 1: Write the failing tests** (in `site/config/mod.rs` tests)

```rust
#[test]
fn parses_book_level_theorems_and_absence() {
    let (cfg, warns) = parse_native_for_test("title: X\ntheorems:\n  numbered: false\n");
    assert!(cfg.theorems.is_some(), "declared theorems: -> Some");
    assert!(warns.iter().all(|w| !w.contains("config key")), "theorems is a known key: {warns:?}");
    let (cfg2, _) = parse_native_for_test("title: X\n");
    assert!(cfg2.theorems.is_none(), "absent theorems: -> None");
}

#[test]
fn a_typod_theorems_subkey_warns() {
    let (_cfg, warns) = parse_native_for_test("title: X\ntheorems:\n  numbred: true\n");
    assert!(warns.iter().any(|w| w.contains("numbred") || w.to_lowercase().contains("theorem")),
        "typo inside theorems warns: {warns:?}");
}
```

(Use the module's existing test helper for calling `parse_native`; grep the tests near line 482 for how they invoke it and mirror that. If none exists, call `parse_native(&serde_yaml::from_str(src).unwrap(), &mut warns)` directly.)

- [ ] **Step 2: Run, expect FAIL** (`theorems` field missing / unknown-key warning present)

Run: `cargo test -p taliesin-core --lib site::config 2>&1 | tail -8`
Expected: compile error (no `theorems` field) or assertion failure.

- [ ] **Step 3: Implement**

- `crates/core/src/frontmatter.rs`: change `fn validate_theorem_values` (line 325) to `pub(crate) fn validate_theorem_values`.
- `site/config/mod.rs`:
  - `NATIVE_KEYS` (line 117): add `"theorems",` (next to `"python"`, `"r"`).
  - `SiteConfig` struct: add `pub theorems: Option<crate::render::fm_extract::TheoremConfig>,` (mirror the `python`/`r` doc-comment style).
  - In `parse_native` (before building `Self { ... }`), validate + parse:

```rust
// `theorems:` is a book-wide numbering policy (inherited by any chapter without
// its own `theorems:` block). Validate its sub-keys like a per-doc block, then
// parse it; absent -> None, so the render fallback can tell "book set a policy".
let theorems = if value.get("theorems").is_some() {
    if let serde_yaml::Value::Mapping(map) = value {
        let mut tw: Vec<crate::Warning> = Vec::new();
        crate::frontmatter::validate_theorem_values(map, "theorems", &mut tw);
        warnings.extend(tw.into_iter().map(|w| w.to_string()));
    }
    Some(crate::render::fm_extract::parse_theorem_config_value(value))
} else {
    None
};
```

  - Add `theorems,` to the `Self { ... }` literal (line 233-250 region).
- Confirm `Warning` has a `Display`/`to_string()` yielding the message; if not, use its message-accessor. Grep `impl.*Warning` / `fn message` to confirm the exact call, adjust the `.map(...)`.
- Re-export `fm_extract::parse_theorem_config_value` / `TheoremConfig` at `crate::render` if the path `crate::render::fm_extract::...` is not reachable (grep how `TheoremConfig` is referenced from outside `render`; `render/mod.rs:48` already `pub use`s it, so prefer `crate::render::TheoremConfig` + a `pub(crate) use` for `parse_theorem_config_value`).

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p taliesin-core --lib site::config 2>&1 | tail -10`

- [ ] **Step 5: Mutation-check + commit**

Remove `"theorems"` from `NATIVE_KEYS` -> `parses_book_level_theorems_and_absence` fails on the unknown-key warning. Restore.

```bash
git add crates/core/src/site/config/mod.rs crates/core/src/frontmatter.rs
git commit -m "feat(site): recognize + parse book-level theorems: in _site.yml (item 16 F-01)"
```

---

### Task 4: Wire the site render call sites to pass the book config

**Files:**
- Modify: `crates/core/src/site/mod.rs:649,1028,1149`; `crates/core/src/site/llms.rs:174`; `crates/core/src/site/search.rs:72` (+ `page_fragment` signature)

**Interfaces:**
- Consumes: `SiteConfig.theorems` (Task 3), `render_document_scoped_with_theorems` (Task 2).

- [ ] **Step 1: Change the four `self`-having call sites**

At `site/mod.rs:649`, `:1028`, `:1149`, and `site/llms.rs:174`, replace
`render::render_document_with_includes_scoped(&src, base, self.chapter_for(page))`
with
`render::render_document_scoped_with_theorems(&src, base, self.chapter_for(page), self.config.theorems.as_ref())`.

- [ ] **Step 2: Thread `page_fragment` (search.rs:72)**

`page_fragment` receives `chapter` from its `Site` caller. Add a `book_theorems: Option<&TheoremConfig>` parameter to `page_fragment`, pass it to `render_document_scoped_with_theorems`, and update the caller in `site/search.rs` (the `impl Site` search-index builder) to pass `self.config.theorems.as_ref()`. Grep `page_fragment(` for the exact caller and signature.

- [ ] **Step 3: Build + run the site + existing theorem suites**

Run: `cargo test -p taliesin-core --lib site 2>&1 | tail -10`
Then the theorem-touching integration tests: `cargo test -p taliesin-core --test course --test cite_this 2>&1 | tail -10`
Expected: all green (no book sets `theorems:` yet, so behavior is unchanged; this proves the `None` path is inert).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/site/mod.rs crates/core/src/site/llms.rs crates/core/src/site/search.rs
git commit -m "feat(site): pass book-level theorems config to every page render (item 16 F-01)"
```

---

### Task 5: Corpus pin + README row

**Files:**
- Create: `corpus/theorem-book/_site.yml`, `corpus/theorem-book/alpha.tmd`, `corpus/theorem-book/beta.tmd`
- Create: `crates/core/tests/book_theorems.rs`
- Modify: `corpus/README.md` (add a row for the new fixture)

**Interfaces:**
- Consumes: the public `Site::discover` + `render_page` API.

- [ ] **Step 1: Write the fixture**

`corpus/theorem-book/_site.yml`:

```yaml
title: "Theorem policy book"
chapters:
  - alpha.tmd
  - beta.tmd
theorems:
  numbered: false
```

`corpus/theorem-book/alpha.tmd` (inherits the book's `numbered: false`, no own block):

```markdown
# Alpha

::: {.theorem #thm-alpha title="Alpha"}
A statement that inherits the book-wide unnumbered policy.
:::
```

`corpus/theorem-book/beta.tmd` (overrides with its own numbered policy):

```markdown
---
theorems:
  numbered: true
---

# Beta

::: {.theorem #thm-beta title="Beta"}
A statement that overrides back to numbered.
:::
```

- [ ] **Step 2: Write the failing test** (`crates/core/tests/book_theorems.rs`)

```rust
//! Book-level `theorems:` (item 16 F-01): a `_site.yml` policy is inherited by a
//! chapter with no `theorems:` block and overridden by one that declares its own.

mod common;
use common::corpus_dir;
use taliesin_core::site::Site;

#[test]
fn book_level_theorems_policy_is_inherited_and_overridable() {
    let site = Site::discover(&corpus_dir().join("theorem-book"));

    // alpha inherits `numbered: false` from _site.yml -> empty number span.
    let alpha = site.render_page("alpha.tmd").expect("alpha renders");
    assert!(
        alpha.contains(r#"tali-theorem-number"></span>"#),
        "alpha inherits book numbered:false (empty number):\n{alpha}"
    );
    assert!(
        !alpha.contains(r#"tali-theorem-number">&nbsp;"#),
        "alpha theorem must carry no number"
    );

    // beta overrides with its own `numbered: true` -> chapter-scoped number 2.1.
    let beta = site.render_page("beta.tmd").expect("beta renders");
    assert!(
        beta.contains(r#"tali-theorem-number">&nbsp;2.1</span>"#),
        "beta overrides to numbered, chapter-scoped:\n{beta}"
    );
}
```

(Confirm the `Site` import path the other tests use, e.g. `taliesin_core::site::Site`, by grepping `use taliesin_core` in `crates/core/tests/course.rs`; mirror it. Confirm `common` module + `corpus_dir` are the same helpers `course.rs`/`text_projection.rs` use.)

- [ ] **Step 3: Run, expect FAIL then PASS**

Run: `cargo test -p taliesin-core --test book_theorems 2>&1 | tail -15`
If the exact number is not `2.1` (chapter scoping detail), render `alpha`/`beta` once to read the real number span and correct the literal, then re-run to green. Keep the `numbered:false` empty-span assert as the load-bearing one.

- [ ] **Step 4: Add the corpus README row**

Add a row to `corpus/README.md` describing `theorem-book/` (book-level `theorems:` inheritance + per-chapter override). Match the table's existing column shape.

- [ ] **Step 5: Mutation-check + commit**

With Task 2/3 in place, temporarily set `_site.yml`'s `theorems: numbered: true` -> alpha's empty-span assert fails (it would number). Restore to `false`.

```bash
git add corpus/theorem-book crates/core/tests/book_theorems.rs corpus/README.md
git commit -m "test(corpus): pin book-level theorems inheritance + override (item 16 F-01)"
```

---

## Final verification

- [ ] Full core suite under gates:
  `TALIESIN_REQUIRE_NODE=1 TALIESIN_R=R TALIESIN_REQUIRE_R=1 TALIESIN_PYTHON=~/.local/share/qmd-venv/bin/python TALIESIN_REQUIRE_KERNEL=1 cargo test -p taliesin-core 2>&1 | tail -20`
- [ ] `cargo fmt` clean (the PostToolUse hook keeps it so; confirm `cargo fmt --check`).
- [ ] Re-read the diff; confirm no snapshot churn outside the new fixture (existing books set no `theorems:`, so the `None` path is inert).
- [ ] Backlog: remove item 16 F-01, add a one-line "Already shipped" entry.
- [ ] Finish per superpowers:finishing-a-development-branch (this work is on `main` directly per the session's flow; present the merge/keep options or, since it is already committed on `main`, summarize and leave the push to the author).

## Self-review

- Spec coverage: recognize (T3) + parse (T1/T3) + merge (T1) + thread (T2) + wire (T4) + pin (T5). All spec sections mapped.
- Placeholder scan: two "grep to confirm the exact accessor" notes (`body_html`/`Warning::to_string`/`page_fragment` caller) are verification steps, not unresolved design; each names the concrete fallback.
- Type consistency: `TheoremConfig` (Clone/Debug/PartialEq), `Option<&TheoremConfig>` threaded uniformly; `render_document_scoped_with_theorems` signature identical in T2 (def) and T4 (calls).
