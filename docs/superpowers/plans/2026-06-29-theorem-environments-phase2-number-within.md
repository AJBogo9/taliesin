# Theorem Environments — Phase 2 (increment 2): `number-within: chapter` scoping

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `theorems: { number-within: chapter }` makes a theorem on a numbered BOOK chapter page render "Theorem 2.3" (chapter number + its running count in the chapter), with `@thm-` refs resolving to the same "2.3". Standalone docs (no chapter) fall back to continuous numbering and warn.

**Architecture:** The chapter number lives at the site layer (`BookEntry.number`), but in-page cross-refs resolve during render (`cite::process` runs inside `render_internal_impl`), so the chapter number must reach `number_theorems` before resolution. Thread an `Option<u32>` chapter through a new `pub(crate) render_document_with_includes_scoped` entry point down to `number_theorems`; the public `render_document`/`render_document_with_includes` pass `None` (API unchanged). The book path (`site/mod.rs::render_page`) computes the chapter number and calls the scoped entry.

**Tech Stack:** Rust (edition 2024), serde_yaml, the existing site/book layer.

**Spec:** `docs/superpowers/specs/2026-06-29-theorem-environments-design.md`, Phase 2. This increment ships `number-within: chapter` only. `number-within: section` (and the `numbered` key) are later; `section` values degrade to continuous (documented), `chapter` is honored.

## Global Constraints

- HTML-only; preview read-only; block-model invariants preserved (only the theorem's number TEXT changes, not block ids/sourcepos).
- Read-only-additive: a new `pub(crate)` render wrapper + an `Option<u32>` param threaded through 3 private fns + `number_theorems`; one new `TheoremConfig` field; one new `THEOREM_KEYS` entry; one site call-site change. No public API change. No scanner / numbering-scanner / cite-lowering / deck change. `number_chapter_headings`, `resolve_cross_refs`, `finish_blocks` untouched.
- Clean break: `number-within` becomes a honored key (so add it to `THEOREM_KEYS`); only the value `chapter` is implemented this increment (other values degrade to continuous).
- Cross-ref correctness: a theorem's number and its in-page `@thm-` reference MUST agree, so scoping happens during render (before `cite::process`), never in a later site pass.
- Schema drift-lock: re-bless after the `THEOREM_KEYS` change. `rustfmt`-clean. No em/en dashes.

---

### Task 1: `number-within` config (enum, parse, validation, schema)

**Files:**
- Modify: `crates/core/src/render/fm_extract.rs` (`NumberWithin` enum; `TheoremConfig.number_within` + `chapter_scoped()`; parse it)
- Modify: `crates/core/src/frontmatter.rs` (`THEOREM_KEYS` += `"number-within"`)
- Modify: `crates/core/src/schema.rs` (add `number-within` string override to the theorems sub-schema)
- Modify: `crates/core/assets/schema/qmd-frontmatter.schema.json` (re-bless)
- Test: `crates/core/src/render/tests.rs`, `crates/core/src/frontmatter.rs` tests mod

**Interfaces:**
- Produces: `TheoremConfig::chapter_scoped(&self) -> bool`; `number-within` parsed from front-matter.

- [ ] **Step 1: Write the failing tests**

Append to `crates/core/src/render/tests.rs`:

```rust
#[test]
fn theorem_config_parses_number_within_chapter() {
    let cfg = parse_theorem_config("theorems:\n  number-within: chapter\n");
    assert!(cfg.chapter_scoped(), "number-within: chapter sets chapter scoping");
    let none = parse_theorem_config("theorems:\n  shared: [theorem]\n");
    assert!(
        !none.chapter_scoped(),
        "absent number-within is not chapter-scoped"
    );
}
```

Add to the `#[cfg(test)] mod tests` in `crates/core/src/frontmatter.rs`:

```rust
    #[test]
    fn theorems_number_within_is_recognized() {
        assert!(
            msgs("---\ntheorems:\n  number-within: chapter\n---\n").is_empty(),
            "number-within is a recognized theorems key"
        );
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p qmd-fast-core --lib theorem_config_parses_number_within_chapter`
Expected: FAIL to compile (`chapter_scoped` undefined).
Run: `cargo test -p qmd-fast-core --lib theorems_number_within_is_recognized`
Expected: FAIL (`number-within` is an unknown theorems key → warns).

- [ ] **Step 3: Add the enum, field, accessor, and parse**

In `crates/core/src/render/fm_extract.rs`, replace the existing `TheoremConfig` struct + impl + parser. The current code is:

```rust
#[derive(Default)]
pub(crate) struct TheoremConfig {
    /// Kinds that share a single counter, in declaration order. Empty = the default
    /// (each kind counts independently).
    shared: Vec<String>,
}

impl TheoremConfig {
    /// The counter key for `kind`: every kind in the shared group collapses to one key
    /// (the group's first member) so they draw a single sequence; an unlisted kind keys
    /// by itself. This governs only the NUMBER; the visible label stays per-kind.
    pub(crate) fn counter_key<'a>(&'a self, kind: &'a str) -> &'a str {
        if self.shared.iter().any(|k| k == kind) {
            self.shared.first().map(String::as_str).unwrap_or(kind)
        } else {
            kind
        }
    }
}
```

Replace with (adds the enum, the field, and `chapter_scoped`):

```rust
/// `theorems: number-within:` scope. Only `Chapter` is honored this increment
/// (book chapter pages render "Theorem 2.3"); other values degrade to `None`.
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub(crate) enum NumberWithin {
    #[default]
    None,
    Chapter,
}

#[derive(Default)]
pub(crate) struct TheoremConfig {
    /// Kinds that share a single counter, in declaration order. Empty = the default
    /// (each kind counts independently).
    shared: Vec<String>,
    /// Numbering scope. `Chapter` prepends the book chapter number ("Theorem 2.3").
    number_within: NumberWithin,
}

impl TheoremConfig {
    /// The counter key for `kind`: every kind in the shared group collapses to one key
    /// (the group's first member) so they draw a single sequence; an unlisted kind keys
    /// by itself. This governs only the NUMBER; the visible label stays per-kind.
    pub(crate) fn counter_key<'a>(&'a self, kind: &'a str) -> &'a str {
        if self.shared.iter().any(|k| k == kind) {
            self.shared.first().map(String::as_str).unwrap_or(kind)
        } else {
            kind
        }
    }

    /// Whether theorem numbers are chapter-scoped (`number-within: chapter`).
    pub(crate) fn chapter_scoped(&self) -> bool {
        self.number_within == NumberWithin::Chapter
    }
}
```

Then in `parse_theorem_config`, after the `shared` block (before `config` is returned), read `number-within`:

```rust
    if value
        .get("theorems")
        .and_then(|t| t.get("number-within"))
        .and_then(|v| v.as_str())
        == Some("chapter")
    {
        config.number_within = NumberWithin::Chapter;
    }
    config
```

(Insert the block immediately before the trailing `config` return; the existing `if let Some(shared) = ...` block stays.)

- [ ] **Step 4: Recognize `number-within` in the validator**

In `crates/core/src/frontmatter.rs`, extend `THEOREM_KEYS`:

```rust
pub(crate) const THEOREM_KEYS: &[&str] = &["shared", "number-within"];
```

- [ ] **Step 5: Add it to the schema**

In `crates/core/src/schema.rs`, the theorems sub-schema currently is:

```rust
        let theorems = closed_object(
            THEOREM_KEYS,
            &[(
                "shared",
                json!({ "type": "array", "items": { "type": "string" } }),
            )],
        );
```

Add a `number-within` string override:

```rust
        let theorems = closed_object(
            THEOREM_KEYS,
            &[
                (
                    "shared",
                    json!({ "type": "array", "items": { "type": "string" } }),
                ),
                ("number-within", json!({ "type": "string" })),
            ],
        );
```

- [ ] **Step 6: Re-bless the schema, then run the tests**

Run: `QMD_FAST_BLESS=1 cargo test -p qmd-fast-core --lib schema`
Expected: re-writes the golden file; passes.
Run: `cargo test -p qmd-fast-core --lib theorem_config_parses_number_within_chapter`
Run: `cargo test -p qmd-fast-core --lib theorems_number_within_is_recognized`
Run: `cargo test -p qmd-fast-core --lib theorem_config_shared_group_shares_counter_key` (regression: shared still works)
Expected: all PASS.

(Do NOT commit yet — `chapter_scoped` is unused in non-test code until Task 2, which would be an intermediate unused-code state. Commit Task 1 + Task 2 together at the end of Task 2.)

---

### Task 2: thread the chapter number + chapter-scope in `number_theorems`

**Files:**
- Modify: `crates/core/src/render/mod.rs` (new `render_document_with_includes_scoped`; `chapter: Option<u32>` on `render_internal`, `render_internal_impl`, `number_theorems`; the scoping + fallback warning; the call site)
- Modify: `crates/core/src/site/mod.rs` (`render_page` computes the chapter number and calls the scoped entry)
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `TheoremConfig::chapter_scoped` (Task 1).
- Produces: `pub(crate) fn render_document_with_includes_scoped(src: &str, base_dir: &Path, chapter: Option<u32>) -> RenderedDoc`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/core/src/render/tests.rs` (it already has `use super::*;`; `Path` is reachable via `std::path::Path`):

```rust
#[test]
fn number_within_chapter_scopes_to_book_chapter() {
    let doc = render_document_with_includes_scoped(
        "---\ntheorems:\n  number-within: chapter\n---\n\n::: {.theorem #thm-a}\nA.\n:::\n\nSee @thm-a.\n\n::: {.theorem #thm-b}\nB.\n:::\n",
        std::path::Path::new("."),
        Some(2),
    );
    let body = doc.body_html();
    assert!(
        body.contains(
            "<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\">&nbsp;2.1</span></span>"
        ),
        "first theorem in chapter 2 is 2.1: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\">&nbsp;2.2</span></span>"
        ),
        "second is 2.2: {body}"
    );
    assert!(
        body.contains("<a href=\"#thm-a\" class=\"qmd-xref\">Theorem&nbsp;2.1</a>"),
        "the in-page ref agrees with the chapter-scoped number: {body}"
    );
}

#[test]
fn number_within_chapter_falls_back_and_warns_without_a_chapter() {
    let doc = render_document(
        "---\ntheorems:\n  number-within: chapter\n---\n\n::: {.theorem}\nA.\n:::\n",
    );
    assert!(
        doc.body_html().contains(
            "<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\">&nbsp;1</span></span>"
        ),
        "no chapter context falls back to continuous numbering: {}",
        doc.body_html()
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("number-within")),
        "a warning explains the no-op outside a book: {:?}",
        doc.warnings
    );
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p qmd-fast-core --lib number_within_chapter`
Expected: FAIL to compile (`render_document_with_includes_scoped` undefined).

- [ ] **Step 3: Add the scoped render entry point and thread `chapter`**

In `crates/core/src/render/mod.rs`, the public + private chain currently is (lines ~100-160):

```rust
pub fn render_document(src: &str) -> RenderedDoc {
    render_internal(src, None, None)
}

pub fn render_document_with_includes(src: &str, base_dir: &Path) -> RenderedDoc {
    let (expanded, origins, include_warnings) = crate::includes::resolve_warned(src, base_dir);
    let (expanded, shortcode_warnings) = extension::expand_shortcodes(&expanded, Some(base_dir));
    let mut doc = render_internal(&expanded, Some(&origins), Some(base_dir));
    // ... extends warnings ...
    doc
}
```

Change `render_document` to pass `None`, make `render_document_with_includes` delegate to a new scoped fn, and add the scoped fn. Replace the two public fns with:

```rust
pub fn render_document(src: &str) -> RenderedDoc {
    render_internal(src, None, None, None)
}

pub fn render_document_with_includes(src: &str, base_dir: &Path) -> RenderedDoc {
    render_document_with_includes_scoped(src, base_dir, None)
}

/// Like [`render_document_with_includes`] but with an optional book chapter number,
/// so `theorems: number-within: chapter` can render "Theorem 2.3". Only the site book
/// path passes `Some(n)`; everything else is `None` (continuous numbering).
pub(crate) fn render_document_with_includes_scoped(
    src: &str,
    base_dir: &Path,
    chapter: Option<u32>,
) -> RenderedDoc {
    let (expanded, origins, include_warnings) = crate::includes::resolve_warned(src, base_dir);
    let (expanded, shortcode_warnings) = extension::expand_shortcodes(&expanded, Some(base_dir));
    let mut doc = render_internal(&expanded, Some(&origins), Some(base_dir), chapter);
    doc.warnings.extend(include_warnings);
    doc.warnings.extend(shortcode_warnings);
    doc
}
```

(IMPORTANT: the original `render_document_with_includes` body extends `doc.warnings` with `include_warnings` + `shortcode_warnings` — preserve that exact warning-extension logic in the scoped fn. If the original differs from the two-line form above, copy its real body and only add the `chapter` argument to the `render_internal` call.)

- [ ] **Step 4: Add `chapter` to the private render chain**

In `crates/core/src/render/mod.rs`, add `chapter: Option<u32>` as the last param to `render_internal` and `render_internal_impl`, and pass it through:

`render_internal` signature becomes:

```rust
fn render_internal(
    src: &str,
    origins: Option<&[LineOrigin]>,
    base_dir: Option<&Path>,
    chapter: Option<u32>,
) -> RenderedDoc {
```

Inside `render_internal`, the call to `render_internal_impl(...)` (inside the spawned big-stack thread) gains `chapter`:

```rust
    render_internal_impl(src, origins, base_dir, chapter)
```

(`chapter: Option<u32>` is `Copy`, so moving it into the thread closure is fine; if the closure is `move`, it captures by copy.)

`render_internal_impl` signature becomes:

```rust
fn render_internal_impl(
    src: &str,
    origins: Option<&[LineOrigin]>,
    base_dir: Option<&Path>,
    chapter: Option<u32>,
) -> RenderedDoc {
```

- [ ] **Step 5: Chapter-scope in `number_theorems`**

In `crates/core/src/render/mod.rs`, add `chapter: Option<u32>` to `number_theorems` and compute the displayed number. The function currently is:

```rust
fn number_theorems(
    blocks: &mut [Block],
    xrefs: &mut HashMap<String, String>,
    warnings: &mut Vec<Warning>,
    config: &TheoremConfig,
) {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for b in blocks.iter_mut() {
        let tag_end_idx = tag_end(&b.html).map(|i| i + 1).unwrap_or(b.html.len());
        let open_tag = b.html[..tag_end_idx].to_string();
        let Some(kind) = extract_attr(&open_tag, "data-qmd-theorem-kind") else {
            continue;
        };
        // Shared-group kinds collapse to one counter key; the visible label stays
        // per-kind (only the number is shared).
        let key = config.counter_key(&kind).to_string();
        let n = {
            let c = counts.entry(key).or_insert(0);
            *c += 1;
            *c
        };
        b.html = b.html.replacen(
            "<span class=\"qmd-theorem-number\"></span>",
            &format!("<span class=\"qmd-theorem-number\">&nbsp;{n}</span>"),
            1,
        );
        if let Some(id) = extract_attr(&open_tag, "id") {
            register_xref(xrefs, warnings, &id, n.to_string());
        }
    }
}
```

Replace it with (adds `chapter` param, a `warned` flag, the `display` computation, and uses `display` for both the slot and the xref):

```rust
fn number_theorems(
    blocks: &mut [Block],
    xrefs: &mut HashMap<String, String>,
    warnings: &mut Vec<Warning>,
    config: &TheoremConfig,
    chapter: Option<u32>,
) {
    let mut counts: HashMap<String, u32> = HashMap::new();
    let mut warned_no_chapter = false;
    for b in blocks.iter_mut() {
        let tag_end_idx = tag_end(&b.html).map(|i| i + 1).unwrap_or(b.html.len());
        let open_tag = b.html[..tag_end_idx].to_string();
        let Some(kind) = extract_attr(&open_tag, "data-qmd-theorem-kind") else {
            continue;
        };
        // Shared-group kinds collapse to one counter key; the visible label stays
        // per-kind (only the number is shared).
        let key = config.counter_key(&kind).to_string();
        let n = {
            let c = counts.entry(key).or_insert(0);
            *c += 1;
            *c
        };
        // `number-within: chapter` prepends the book chapter number ("Theorem 2.3").
        // Outside a numbered chapter there is no chapter to scope to, so fall back to
        // continuous numbering and warn once.
        let display = if config.chapter_scoped() {
            match chapter {
                Some(c) => format!("{c}.{n}"),
                None => {
                    if !warned_no_chapter {
                        warnings.push(Warning::new(
                            "`theorems: number-within: chapter` has no effect outside a book chapter; using continuous theorem numbering".to_string(),
                        ));
                        warned_no_chapter = true;
                    }
                    n.to_string()
                }
            }
        } else {
            n.to_string()
        };
        b.html = b.html.replacen(
            "<span class=\"qmd-theorem-number\"></span>",
            &format!("<span class=\"qmd-theorem-number\">&nbsp;{display}</span>"),
            1,
        );
        if let Some(id) = extract_attr(&open_tag, "id") {
            register_xref(xrefs, warnings, &id, display);
        }
    }
}
```

Update the call site (the `number_theorems(...)` call after `apply_table_captions`):

```rust
    number_theorems(
        &mut blocks,
        &mut xref_registry,
        &mut warnings,
        &theorem_config,
        chapter,
    );
```

- [ ] **Step 6: Wire the chapter number in at the site book path**

In `crates/core/src/site/mod.rs`, `render_page` currently is:

```rust
    pub fn render_page(&self, rel_or_url: &str) -> Option<String> {
        let page = self.page(rel_or_url)?;
        let src = std::fs::read_to_string(&page.input).ok()?;
        let base = page.input.parent().unwrap_or(&self.root);
        let doc = render::render_document_with_includes(&src, base);
        Some(self.render_page_doc(page, doc))
    }
```

Change the render call to compute the chapter number (the same lookup `number_chapter` does) and pass it through the scoped entry:

```rust
    pub fn render_page(&self, rel_or_url: &str) -> Option<String> {
        let page = self.page(rel_or_url)?;
        let src = std::fs::read_to_string(&page.input).ok()?;
        let base = page.input.parent().unwrap_or(&self.root);
        // A numbered book chapter scopes its theorems to its chapter number
        // ("Theorem 2.3"); non-book / unnumbered pages pass None (continuous).
        let chapter = self
            .book
            .as_ref()
            .and_then(|b| b.entries.iter().find(|e| e.rel == page.rel).and_then(|e| e.number));
        let doc = render::render_document_with_includes_scoped(&src, base, chapter);
        Some(self.render_page_doc(page, doc))
    }
```

(If `render_page` also has a `_warned` sibling that does its own render call, leave it; only the `render_document_with_includes` call for the page body needs the scoped variant. Verify there is exactly one such call in `render_page`; `render_page_doc_warned` takes an already-rendered `doc`, so it does not render again.)

- [ ] **Step 7: Run the tests + regression**

Run: `cargo test -p qmd-fast-core --lib number_within_chapter`
Expected: PASS (both tests).
Run: `cargo test -p qmd-fast-core --lib theorem` and `cargo test -p qmd-fast-core --lib shared_counter`
Expected: PASS (Phase 1 + shared-counter behavior unchanged when no `number-within`).
Run: `cargo test -p qmd-fast-core` (full core, incl. site/book tests)
Expected: PASS.

- [ ] **Step 8: Commit (Task 1 + Task 2 together)**

```bash
git add crates/core/src/render/fm_extract.rs crates/core/src/frontmatter.rs crates/core/src/schema.rs crates/core/assets/schema/qmd-frontmatter.schema.json crates/core/src/render/mod.rs crates/core/src/site/mod.rs crates/core/src/render/tests.rs
git commit -m "feat(render): theorems: number-within: chapter (book chapter-scoped numbering)"
```

---

### Task 3: Corpus pin (book chapter) + verification

**Files:**
- Modify: `corpus/demo-book/methods.qmd` (chapter 2: add `number-within: chapter` + a theorem)
- Modify: `crates/core/tests/corpus.rs` (a test rendering the chapter as a book page asserting "Theorem 2.1")
- Browser verify

- [ ] **Step 1: Add a chapter-scoped theorem to demo-book chapter 2**

Replace `corpus/demo-book/methods.qmd` with (adds front-matter + a theorem; `methods.qmd` is chapter 2 in `_site.yml`):

```markdown
---
theorems:
  number-within: chapter
---

# Methods {#sec-methods}

The approach in one display equation:

$$ D_{\mathrm{KL}}(P\,\|\,Q) = \sum_k p_k \log\frac{p_k}{q_k} $$

::: {.theorem #thm-kl}
The KL divergence is non-negative, with equality iff $P = Q$.
:::

By @thm-kl, the objective is bounded below.

## Setup {#sec-setup}

The setup for the experiments.
```

- [ ] **Step 2: Add a book-render test asserting chapter scoping**

In `crates/core/tests/corpus.rs`, after `book_discovers_chapters_with_parts_numbering_and_chrome` (ends ~line 730; find its closing `}`), add:

```rust
#[test]
fn book_chapter_scopes_theorem_numbers() {
    use qmd_fast_core::Site;
    let site = Site::discover(&corpus_dir().join("demo-book"));
    // methods.qmd is chapter 2, with `theorems: number-within: chapter`.
    let methods = site.render_page("methods.qmd").expect("methods renders");
    assert!(
        methods.contains(
            "<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\">&nbsp;2.1</span></span>"
        ),
        "the chapter-2 theorem numbers as 2.1: {methods}"
    );
    assert!(
        methods.contains("<a href=\"#thm-kl\" class=\"qmd-xref\">Theorem&nbsp;2.1</a>"),
        "its in-page cross-ref agrees: {methods}"
    );
}
```

(If `Site::discover` is not the constructor the existing book test uses, mirror that test's exact constructor call, e.g. `Site::discover` / `Site::new`.)

- [ ] **Step 3: Run the suite**

Run: `cargo test -p qmd-fast-core`
Expected: PASS, including `book_chapter_scopes_theorem_numbers` and the existing `book_discovers_chapters_with_parts_numbering_and_chrome` (adding a theorem + front-matter to methods.qmd must not break its chrome/section-number assertions). The corpus walkers also render `methods.qmd` standalone (chapter `None`) — that emits the continuous-fallback warning, which is fine: `every_corpus_doc_emits_no_unknown_key_warnings` only filters `unknown ` warnings, and `every_corpus_doc_has_clean_front_matter` accepts `theorems`/`number-within`.

- [ ] **Step 4: Browser verify**

Run: `cargo build -p qmd-fast-server` then `fuser -k 4388/tcp; ./target/debug/qmd-fast preview corpus/demo-book 4388` (background; a book preview); wait for HTTP 200.
In chrome-devtools: navigate to the Methods chapter (`http://127.0.0.1:4388/methods.html` or via the sidebar), screenshot, confirm the theorem reads "Theorem 2.1", the `@thm-kl` link reads "Theorem 2.1", section headings still read "2", "2.1", and there are no console errors.

- [ ] **Step 5: Commit**

```bash
git add corpus/demo-book/methods.qmd crates/core/tests/corpus.rs
git commit -m "feat(corpus): pin chapter-scoped theorem numbering in demo-book"
```

---

## Self-review

- **Spec coverage:** `number-within: chapter` book scoping → Tasks 1-2; corpus pin → Task 3. Deferred: `number-within: section`, `numbered: false|unless-unique`, reference-name polish.
- **Placeholder scan:** none; the two "if the real body differs" notes (render_document_with_includes warning-extension; the Site constructor name) are verification guards, not placeholders — the concrete code is given.
- **Type consistency:** `chapter: Option<u32>` is threaded uniformly through `render_document_with_includes_scoped` → `render_internal` → `render_internal_impl` → `number_theorems`; `TheoremConfig::chapter_scoped()` (Task 1) is the single switch `number_theorems` reads; the site path computes `chapter` from `BookEntry.number` exactly as `number_chapter` already does. The corpus assertion ("2.1") matches `{chapter}.{n}` with chapter=2, n=1.
