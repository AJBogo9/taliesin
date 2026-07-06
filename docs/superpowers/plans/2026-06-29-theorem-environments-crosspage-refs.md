# Theorem Environments — cross-page references fix

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use `- [ ]` tracking.

**Goal:** Fix the confirmed bug (2026-06-29 adversarial review): a cross-PAGE `@thm-x` in a book (e.g. chapter 3 referencing a theorem in chapter 2) renders as a dead bare-label link + a false "broken cross-reference" warning, because the site-wide xref scanner never registers theorem anchors.

**Root cause + fix (both in `crates/core/src/site/xref.rs`):** (1) `brace_id` only matches an id written id-first (`{#x}`), but a theorem is class-first (`::: {.theorem #thm-x}`) — generalize it to find a `#id` token anywhere in a `{…}` block. (2) `is_ref_anchor` omits `lem-`/`cor-`/`prp-`/`exm-`/`rem-` — add them. After both, a theorem anchor enters the cross-page registry and `rewrite_cross_refs` resolves the marker to a real link (which also removes the false warning).

**Design decision — bare label cross-page, like figures.** The investigation confirmed cross-page non-heading anchors (figure/equation/table) ALL carry no number (`scan_page_anchors` stores `number=""` for non-headings); only headings get a number cross-page. So a cross-page `@fig-x` already shows "Figure" with no number. Theorems behave identically: a cross-page `@thm-x` resolves to a working link with a bare "Theorem" label, no number. This is CONSISTENT with the other xref families; propagating computed numbers cross-page is a separate general limitation (would need a render-harvest pass) and is explicitly out of scope here.

> **SUPERSEDED (2026-07-06):** this "bare label cross-page / out of scope" decision was reversed. The render-harvest pass (`Site::harvest_xref_numbers`) now runs inside `Site::discover`, so cross-page `@thm-`/`@fig-`/`@eq-` refs render their number ("Theorem 2.1") in the live preview as well as the static build. See the `cross-page-theorem-refs` change.

## Global Constraints

HTML-only; read-only-additive (two small fn changes in the cross-page scanner; no render/numbering change, no RenderedDoc change); block invariants untouched. `rustfmt`-clean; no em/en dashes. Same-page theorem refs (already working, with numbers) must stay unchanged.

---

### Task 1: scanner fix + corpus pin (TDD)

**Files:** `crates/core/src/site/xref.rs` (`brace_id`, `is_ref_anchor`), `corpus/demo-book/results.qmd` (add a cross-page `@thm-kl`), `crates/core/tests/corpus.rs` (extend the cross-page test), `docs/superpowers/specs/2026-06-29-theorem-environments-design.md` (correct the cross-page note).

- [ ] **Step 1: pin the cross-page ref + failing assertion.** Append a cross-page theorem ref to `corpus/demo-book/results.qmd` (methods.qmd in chapter 2 defines `::: {.theorem #thm-kl}`):

  Add after the existing intro paragraph: `\n\nIt also leans on @thm-kl from the methods chapter.\n`

  Extend `book_cross_chapter_refs_resolve` (the test ending ~line 758 in `corpus.rs`, after the `data-qmd-xref="sec-methods"` assertion) with:

```rust
    // A cross-PAGE theorem ref resolves to the defining chapter with a bare label
    // (no number cross-page, consistent with figures/equations).
    assert!(
        results.contains("<a href=\"methods.html#thm-kl\" class=\"qmd-xref\">Theorem</a>"),
        "cross-chapter theorem ref not resolved: {results}"
    );
    assert!(
        !results.contains("data-qmd-xref=\"thm-kl\""),
        "resolved theorem cross-ref still carries its broken marker"
    );
```

- [ ] **Step 2: run, verify fail.** `cargo test -p qmd-fast-core --test corpus book_discovers_chapters_with_parts_numbering_and_chrome` (the test name; confirm via grep). Expected: FAIL — `@thm-kl` is left as a `data-qmd-xref` marker (dead bare link), not rewritten to `methods.html#thm-kl`.

- [ ] **Step 3: generalize `brace_id`** (`xref.rs:115-121`) to find a `#id` token anywhere in a `{…}` block:

```rust
/// The id from a `{…}` attribute block on a line, if any: a `#id` token that starts the
/// block (id-first `{#sec-x}`) or follows a space (class-first `::: {.theorem #thm-x}`),
/// read up to a space, `.`, or `}`.
fn brace_id(line: &str) -> Option<String> {
    let open = line.find('{')?;
    let block = &line[open + 1..];
    let block = &block[..block.find('}').unwrap_or(block.len())];
    let bytes = block.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'#' && (i == 0 || bytes[i - 1] == b' ') {
            let after = &block[i + 1..];
            let end = after.find([' ', '.', '}']).unwrap_or(after.len());
            let id = &after[..end];
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}
```

- [ ] **Step 4: extend `is_ref_anchor`** (`xref.rs:123-127`):

```rust
fn is_ref_anchor(id: &str) -> bool {
    [
        "sec-", "fig-", "tbl-", "eq-", "lst-", "thm-", "lem-", "cor-", "prp-", "def-", "exm-",
        "rem-",
    ]
    .iter()
    .any(|p| id.starts_with(p))
}
```

- [ ] **Step 5: run.** The extended corpus test passes; the existing `@sec-` cross-page assertions still pass (heading scan unchanged). `cargo test -p qmd-fast-core` (full; the new cross-page ref in results.qmd must not trip other corpus walkers — `@thm-kl` is now a recognized resolvable anchor).

- [ ] **Step 6: correct the spec.** In `docs/superpowers/specs/2026-06-29-theorem-environments-design.md`, replace the "CROSS-page resolution does NOT yet work" note with: cross-page theorem refs now resolve to a working link with a bare label (no number cross-page, consistent with `@fig-`/`@eq-`); per-number cross-page propagation remains a general non-heading-xref limitation, out of scope.

- [ ] **Step 7: commit.** `fix(site): resolve cross-page theorem references (brace_id + is_ref_anchor)`.

---

### Task 2: browser sanity-check + finish

- [ ] **Step 1:** `cargo build -p qmd-fast-server`; `serve corpus/demo-book`; navigate to `results.html`; confirm the `@thm-kl` link points to `methods.html#thm-kl` (bare "Theorem" label), no console errors, no dev-menu diagnostic.
- [ ] **Step 2:** fmt + clippy clean; full workspace test (note the known unrelated `warm_pool` forkserver flake).

## Self-review
- Two-line scanner fix; bare-label cross-page matches figures/equations (consistency, not a special render-harvest path). Same-page numbered theorem refs unchanged. Fixes both the dead link AND the false broken-ref warning (the marker is now rewritten before `validate_xrefs` runs).
- `brace_id` generalization also benefits any class-first `{.x #id}` anchor, and the `#`-after-`{`-or-space guard avoids false matches on a `#` inside an attribute value.
