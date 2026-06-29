# Theorem Environments — Phase 3: web-native affordances

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use `- [ ]` tracking.

**Goal:** The live-HTML payoff. (1) Hover-preview of a `@thm-` reference shows the theorem's statement in a card. (2) Collapsible proofs: `::: {.proof collapse="true"}` folds the proof behind a native `<details>`. (3) Deep-link anchors: a theorem's head gets a copy-link `#`, like headings.

**Architecture:** All three ride existing, page-wide enhancers/patterns. Hover-preview is ALREADY live (the `12-link-preview.js` enhancer fires on any `a[href^="#"]` and deep-clones the target), so it needs verifying + pinning, not code. Collapsible proofs mirror the callout `collapse=` `<details>` branch in `divs.rs` (no JS). Deep-link anchors extend the `02-anchor-links.js` enhancer to `.qmd-theorem[id]`. `collapse` is a fenced-div attribute (like callouts'), not a front-matter key, so NO validator/schema change.

## Global Constraints

HTML-only; read-only-additive; preview never writes source (anchors are clipboard-only, collapse is native `<details>`). Block ids/sourcepos untouched. `rustfmt`-clean; no em/en dashes. Enhancers ship on every non-Bare page (not reader-only), so the affordances work on a normal page.

---

### Task 1: Collapsible proofs (Rust + CSS)

**Files:** `crates/core/src/render/divs.rs` (proof arm ~549-557), `crates/core/assets/css/base.css` (proof CSS ~477), test in `crates/core/src/render/tests.rs`.

- [ ] **Step 1: failing test** (append to `render/tests.rs`):

```rust
#[test]
fn proof_collapse_folds_into_details() {
    let closed = render_document("::: {.proof collapse=\"true\"}\nBody.\n:::\n");
    let h = &closed.blocks[0].html;
    assert!(
        h.contains("<div class=\"qmd-proof qmd-proof-collapse\"")
            && h.contains("<details><summary class=\"qmd-proof-head\">Proof.</summary>"),
        "collapse=true folds the proof behind a closed <details>: {h}"
    );
    assert!(
        h.contains("<span class=\"qmd-qed\" aria-hidden=\"true\">\u{220e}</span></details>"),
        "QED sits inside <details> (shown only when expanded): {h}"
    );
    // collapse="false" is collapsible but starts open
    let open = render_document("::: {.proof collapse=\"false\"}\nBody.\n:::\n");
    assert!(
        open.blocks[0].html.contains("<details open>"),
        "collapse=false starts open: {}",
        open.blocks[0].html
    );
    // no collapse attr keeps the plain (non-details) proof
    let plain = render_document("::: {.proof}\nBody.\n:::\n");
    assert!(
        !plain.blocks[0].html.contains("<details"),
        "a plain proof is not a <details>: {}",
        plain.blocks[0].html
    );
}
```

- [ ] **Step 2: run, verify fail.** `cargo test -p qmd-fast-core --lib proof_collapse_folds_into_details`.

- [ ] **Step 3: implement.** In `divs.rs`, replace the `if kind == "proof"` body (lines 549-557) so it branches on `attrs.get("collapse")`, mirroring the callout-collapse branch (QED kept INSIDE `<details>` so a folded proof shows just the summary):

```rust
        if kind == "proof" {
            // Unnumbered, not cross-referenceable (matches Quarto/bookdown). Auto-QED.
            let head = attrs
                .get("title")
                .map(html_escape)
                .unwrap_or_else(|| "Proof".to_string());
            let qed = "<span class=\"qmd-qed\" aria-hidden=\"true\">\u{220e}</span>";
            // `collapse="true"` folds the proof behind a native <details> (starts closed);
            // `collapse="false"` is collapsible but starts open. QED rides inside <details>
            // so a collapsed proof shows only its "Proof." summary.
            match attrs.get("collapse") {
                Some(v) => {
                    let open = if v == "false" { " open" } else { "" };
                    format!(
                        "<div class=\"qmd-proof qmd-proof-collapse\"{data}><details{open}><summary class=\"qmd-proof-head\">{head}.</summary><div class=\"qmd-theorem-body\">{body}</div>{qed}</details></div>"
                    )
                }
                None => format!(
                    "<div class=\"qmd-proof\"{data}><p class=\"qmd-proof-head\">{head}.</p><div class=\"qmd-theorem-body\">{body}</div>{qed}</div>"
                ),
            }
        } else {
```

- [ ] **Step 4: CSS.** In `base.css`, after the `.qmd-qed` rule (~478), add:

```css
  .qmd-proof-collapse > details > summary.qmd-proof-head { cursor: pointer; margin: 0; }
  .qmd-proof-collapse > details[open] > summary.qmd-proof-head { margin-bottom: .2rem; }
```

- [ ] **Step 5: run.** `cargo test -p qmd-fast-core --lib proof_collapse_folds_into_details` (pass) + `cargo test -p qmd-fast-core --lib proof_` (regression: plain proof + QED unchanged).

- [ ] **Step 6: commit.** `feat(render): collapsible proofs (::: {.proof collapse="true"})`.

---

### Task 2: Deep-link anchors for theorems (JS + CSS)

**Files:** `crates/core/assets/js/code-enhance/02-anchor-links.js`, `crates/core/assets/css/base.css` (anchor reveal ~344). No Rust test (client-side; browser-verified in Task 3).

- [ ] **Step 1: JS.** In `02-anchor-links.js`, after the `figcaption, caption` block (line 44), add:

```js
  // A theorem carries its id on the wrapper; drop the `#` into its head paragraph.
  [].forEach.call(scope.querySelectorAll('.qmd-theorem[id]'), function (t) {
    var head = t.querySelector('.qmd-theorem-head');
    if (head) decorate(head, t.id);
  });
```

- [ ] **Step 2: CSS.** In `base.css`, add `.qmd-theorem-head` to the hover-reveal selector (line 344-345), so the anchor reveals on hover like a heading:

```css
  :is(h1, h2, h3, h4, h5, h6):hover > .qmd-anchor,
  figcaption:hover > .qmd-anchor, caption:hover > .qmd-anchor,
  .qmd-theorem-head:hover > .qmd-anchor,
  .qmd-anchor:focus-visible { opacity: 1; }
```

- [ ] **Step 3: type-check the client JS.** `cd web-client && npx -y -p typescript tsc -p jsconfig.json` is for web-client/; the code-enhance fragments are concatenated server-side. Just confirm `cargo build -p qmd-fast-core` succeeds (the fragment is `include_str!`-concatenated and guarded). Browser-verified in Task 3.

- [ ] **Step 4: commit.** `feat(assets): deep-link copy-anchor on theorems`.

---

### Task 3: Corpus pin + browser-verify all three affordances

**Files:** `corpus/refs/theorems-interactive.qmd`, `corpus/README.md`.

- [ ] **Step 1: pin doc.** Create `corpus/refs/theorems-interactive.qmd`:

```markdown
---
title: "Interactive theorems"
---

# Reading affordances

::: {.theorem #thm-fix}
A continuous map of a compact convex set to itself has a fixed point.
:::

Hover @thm-fix to preview its statement; the heading-style `#` on the box copies a deep link.

::: {.proof collapse="true"}
Apply Brouwer's theorem to the simplex and pass to the limit.
:::
```

- [ ] **Step 2: README row + run.** Add a `refs/theorems-interactive.qmd` row to `corpus/README.md`; `cargo test -p qmd-fast-core` (corpus walkers accept it; invariants hold).

- [ ] **Step 3: browser-verify** (`serve corpus/refs/theorems-interactive.qmd 4388`, chrome-devtools):
  - hover the `@thm-fix` link -> a card shows "Theorem 1 ... fixed point" (KaTeX-free here, but the statement text appears),
  - the proof shows a collapsed "Proof." summary; clicking it expands to the body + ∎,
  - hovering the theorem box reveals a `#` that copies a link (clipboard),
  - no console errors.

- [ ] **Step 4: commit.** `feat(corpus): pin interactive theorem affordances`.

## Self-review
- Hover-preview: 0 code (already live via 12-link-preview.js); verified + pinned. Collapsible proofs: divs.rs + CSS, native `<details>`, QED inside so a folded proof is clean. Anchors: 02-anchor-links.js + one CSS selector. `collapse` needs no validator/schema entry (div attr, not front-matter). All enhancers ship on every non-Bare page.
- Deferred: clickable-QED (separate micro-feature, low value); cross-page theorem refs (separate latent-bug increment); reference-name polish.
