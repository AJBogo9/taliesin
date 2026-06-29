# Theorem Environments — Phase 1 (MVP) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship LaTeX-style theorem environments (the core 8 kinds + proof) as styled, continuously-numbered, cross-referenceable `:::` blocks, rendered HTML-only and corpus-pinned.

**Architecture:** A new `build_container` arm emits a theorem/proof container (the same div-block pattern callouts use); a new `number_theorems` post-pass (modeled on `apply_table_captions`) assigns per-kind continuous numbers in document order and registers `#thm-`/`#lem-`/… anchors; the existing two-pass cross-ref machinery resolves them. Read-only-additive: no scanner, numbering-scanner, cite-lowering, includes, exec, or deck change.

**Tech Stack:** Rust (edition 2024), comrak block model, server-side KaTeX, the bundled `base.css`; tests via `cargo test -p qmd-fast-core`.

**Spec:** `docs/superpowers/specs/2026-06-29-theorem-environments-design.md` (this plan implements Phase 1 only; phases 2 to 4 get their own plans when relevant).

## Global Constraints

- HTML-only output; the preview is read-only and never writes back to source.
- Block-model invariants (corpus-enforced): every emitted block keeps `data-block-id` + valid `data-sourcepos`; included blocks keep `data-source-file`. The theorem container reuses the existing `build_container` id/sourcepos pattern; do not alter it.
- Do-NOT-touch (rewrite): the `:::` scanner, existing figure/table/section numbering, `cite.rs` `[@key]` lowering + BibTeX/CSL, `includes.rs`, exec/freeze/kernel, the deck engine. This plan only ADDS a `build_container` arm, a post-pass, `xref_label` entries, a const, and CSS.
- Prose/copy rule: never use em dashes or en dashes in any authored text (corpus doc, comments).
- A `PostToolUse` hook runs `rustfmt` on edited `.rs` files; keep the tree `cargo fmt`-clean.
- Numbering MVP semantics: continuous, per-kind, document/page-wide (Theorem 1, 2, 3 …; Lemma 1, 2, 3 … independently). Section/chapter scoping and shared counters are Phase 2, not here.
- Kind set is fixed (no author-defined kinds in the MVP). There is NO did-you-mean for a misspelled theorem kind: theorems have no namespace prefix (unlike `callout-`), so a misspelled kind renders as a plain div. Do not add a heuristic kind-typo validator in Phase 1.

---

### Task 1: Theorem + proof emission (`build_container` arm)

Emit the styled container with an empty number slot. No numbering or cross-ref yet.

**Files:**
- Modify: `crates/core/src/render/validate.rs` (add `THEOREM_KINDS` after `CALLOUT_KINDS`, line 36)
- Modify: `crates/core/src/render/mod.rs` (add `DivAttrs::theorem_kind` after `callout_kind`, ~line 1382)
- Modify: `crates/core/src/render/divs.rs` (add `theorem_meta` helper + a new arm in `build_container` before the final `else`, ~line 532)
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `DivAttrs::get`, `DivAttrs::id`, `id_attr`, `html_escape`, the `concat` closure, the `data`/`open_line`/`file` locals already in `build_container`.
- Produces:
  - `validate::THEOREM_KINDS: &[&str]` (the dispatch vocabulary, also the single source of truth Task 2 / later phases reuse).
  - `DivAttrs::theorem_kind(&self) -> Option<&str>` (first class that is a `THEOREM_KINDS` member).
  - `theorem_meta(kind: &str) -> (&'static str, &'static str)` in `divs.rs` (display name, style suffix `plain`|`definition`|`remark`).
  - Container HTML contract consumed by Task 2's post-pass: a numbered kind emits `<div class="qmd-theorem qmd-theorem-{kind} qmd-thm-style-{style}"[ id="…"] data-qmd-theorem-kind="{kind}"{data}>…</div>` with the number slot `<span class="qmd-theorem-number"></span>`; `proof` emits `<div class="qmd-proof"{data}>…<span class="qmd-qed" aria-hidden="true">∎</span></div>` and carries NO `data-qmd-theorem-kind`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/core/src/render/tests.rs`:

```rust
#[test]
fn theorem_div_emits_styled_block_with_number_slot() {
    let doc = render_document(
        "::: {.theorem #thm-pyth title=\"Pythagorean theorem\"}\n$a^2+b^2=c^2$.\n:::\n",
    );
    assert_eq!(doc.blocks.len(), 1, "the theorem is one container block");
    let h = &doc.blocks[0].html;
    assert!(
        h.contains("class=\"qmd-theorem qmd-theorem-theorem qmd-thm-style-plain\""),
        "got: {h}"
    );
    assert!(h.contains("data-qmd-theorem-kind=\"theorem\""), "got: {h}");
    assert!(h.contains(" id=\"thm-pyth\""), "author anchor on container: {h}");
    assert!(
        h.contains(
            "<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\"></span></span>"
        ),
        "head carries the kind name + an empty number slot: {h}"
    );
    assert!(
        h.contains("<span class=\"qmd-theorem-title\">(Pythagorean theorem)</span>"),
        "got: {h}"
    );
    // inner content keeps its own block id (click-to-source) and math is KaTeX-rendered
    assert!(h.contains("data-block-id"), "inner block lost id: {h}");
    assert!(h.contains("katex"), "math should render inside the theorem: {h}");
}

#[test]
fn theorem_styles_map_kinds() {
    let d = render_document("::: {.definition}\nA set.\n:::\n");
    assert!(
        d.blocks[0]
            .html
            .contains("qmd-theorem-definition qmd-thm-style-definition"),
        "got: {}",
        d.blocks[0].html
    );
    assert!(
        d.blocks[0]
            .html
            .contains("<span class=\"qmd-theorem-label\">Definition"),
        "got: {}",
        d.blocks[0].html
    );
    let r = render_document("::: {.remark}\nAside.\n:::\n");
    assert!(
        r.blocks[0].html.contains("qmd-thm-style-remark"),
        "got: {}",
        r.blocks[0].html
    );
}

#[test]
fn proof_emits_qed_and_no_number_slot() {
    let p = render_document("::: {.proof}\nBy the diagram.\n:::\n");
    let h = &p.blocks[0].html;
    assert!(h.contains("class=\"qmd-proof\""), "got: {h}");
    assert!(h.contains("<p class=\"qmd-proof-head\">Proof.</p>"), "got: {h}");
    assert!(
        h.contains("<span class=\"qmd-qed\" aria-hidden=\"true\">\u{220e}</span>"),
        "got: {h}"
    );
    assert!(!h.contains("qmd-theorem-number"), "proof is unnumbered: {h}");

    let renamed =
        render_document("::: {.proof title=\"Proof of the main theorem\"}\nx.\n:::\n");
    assert!(
        renamed
            .blocks[0]
            .html
            .contains("<p class=\"qmd-proof-head\">Proof of the main theorem.</p>"),
        "got: {}",
        renamed.blocks[0].html
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p qmd-fast-core theorem_div_emits_styled_block_with_number_slot theorem_styles_map_kinds proof_emits_qed_and_no_number_slot`
Expected: FAIL (a `.theorem` div currently falls through to the generic arm and emits `<div class="theorem" …>`, so none of the `qmd-theorem*`/`qmd-proof` assertions match).

- [ ] **Step 3: Add the dispatch vocabulary const**

In `crates/core/src/render/validate.rs`, directly after the `CALLOUT_KINDS` const (line 36):

```rust
/// Theorem-environment kinds qmd-fast recognizes (`::: {.theorem}`, `::: {.proof}`, …).
/// Unlike callouts there is no namespace prefix, so this set IS the dispatch
/// vocabulary: a div whose class is one of these enters the theorem arm. `proof` is
/// included but is unnumbered + unreferenceable. A misspelled kind has no prefix to
/// anchor a did-you-mean, so it falls through to a plain div (see the design doc).
pub(crate) const THEOREM_KINDS: &[&str] = &[
    "theorem",
    "lemma",
    "corollary",
    "proposition",
    "definition",
    "example",
    "remark",
    "proof",
];
```

- [ ] **Step 4: Add the `DivAttrs::theorem_kind` accessor**

In `crates/core/src/render/mod.rs`, inside `impl DivAttrs`, directly after `callout_kind` (ends line 1382):

```rust
    /// The first class that names a theorem-environment kind, or `None`.
    fn theorem_kind(&self) -> Option<&str> {
        self.classes
            .iter()
            .map(String::as_str)
            .find(|c| validate::THEOREM_KINDS.contains(c))
    }
```

- [ ] **Step 5: Add `theorem_meta` and the `build_container` arm**

In `crates/core/src/render/divs.rs`, add this helper directly above `fn build_container` (line 320):

```rust
/// (display name, amsthm style suffix) for a NUMBERED theorem kind. `proof` is handled
/// separately (unnumbered) and never reaches here; an unknown kind never enters the arm.
fn theorem_meta(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "theorem" => ("Theorem", "plain"),
        "lemma" => ("Lemma", "plain"),
        "corollary" => ("Corollary", "plain"),
        "proposition" => ("Proposition", "plain"),
        "definition" => ("Definition", "definition"),
        "example" => ("Example", "definition"),
        "remark" => ("Remark", "remark"),
        _ => ("", "plain"),
    }
}
```

In `build_container`, insert a new arm immediately before the final `} else {` (line 532, the generic-div arm):

```rust
    } else if let Some(kind) = attrs.theorem_kind() {
        let body = concat(&inner);
        if kind == "proof" {
            // Unnumbered, not cross-referenceable (matches Quarto/bookdown). Auto-QED.
            let head = attrs
                .get("title")
                .map(html_escape)
                .unwrap_or_else(|| "Proof".to_string());
            format!(
                "<div class=\"qmd-proof\"{data}><p class=\"qmd-proof-head\">{head}.</p><div class=\"qmd-theorem-body\">{body}</div><span class=\"qmd-qed\" aria-hidden=\"true\">\u{220e}</span></div>"
            )
        } else {
            // The number slot is filled by the `number_theorems` post-pass (after
            // group_divs, before cite::process), so numbering stays document-ordered.
            let (name, style) = theorem_meta(kind);
            let id_attr = id_attr(attrs.id.as_deref());
            let title = match attrs.get("title") {
                Some(t) => format!(
                    " <span class=\"qmd-theorem-title\">({})</span>",
                    html_escape(t)
                ),
                None => String::new(),
            };
            format!(
                "<div class=\"qmd-theorem qmd-theorem-{kind} qmd-thm-style-{style}\"{id_attr} data-qmd-theorem-kind=\"{kind}\"{data}><p class=\"qmd-theorem-head\"><span class=\"qmd-theorem-label\">{name}<span class=\"qmd-theorem-number\"></span></span>{title}</p><div class=\"qmd-theorem-body\">{body}</div></div>"
            )
        }
    } else {
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p qmd-fast-core theorem_div_emits_styled_block_with_number_slot theorem_styles_map_kinds proof_emits_qed_and_no_number_slot`
Expected: PASS (3 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/render/validate.rs crates/core/src/render/mod.rs crates/core/src/render/divs.rs crates/core/src/render/tests.rs
git commit -m "feat(render): theorem-environment emission (8 kinds + proof/QED)"
```

---

### Task 2: Numbering + cross-references

Fill each theorem's number slot in document order and resolve `@thm-`/`@lem-`/… refs.

**Files:**
- Modify: `crates/core/src/cite/render.rs` (add 5 prefixes to `xref_label`, lines 11-22)
- Modify: `crates/core/src/render/mod.rs` (add `fn number_theorems` near `apply_table_captions` ~line 1216; call it at line 585, between `apply_table_captions` and `cite::process`)
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: the Task 1 container contract (`data-qmd-theorem-kind`, the empty number slot, the optional container `id`); `register_xref(reg, warnings, anchor, number: String)`, `extract_attr(html, name) -> Option<String>`, `tag_end(html) -> Option<usize>` (all in `mod.rs`); `xref_label` (cite).
- Produces: `number_theorems(blocks: &mut [Block], xrefs: &mut HashMap<String, String>, warnings: &mut Vec<Warning>)`; the cross-ref resolution `<a href="#thm-x" class="qmd-xref">Theorem&nbsp;N</a>` (via the existing `xref_anchor_link`).

- [ ] **Step 1: Write the failing tests**

Append to `crates/core/src/render/tests.rs`:

```rust
#[test]
fn theorems_number_continuously_per_kind() {
    let doc = render_document(
        "::: {.theorem}\nA.\n:::\n\n::: {.lemma}\nB.\n:::\n\n::: {.theorem}\nC.\n:::\n",
    );
    let body = doc.body_html();
    assert!(
        body.contains(
            "<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\">&nbsp;1</span></span>"
        ),
        "first theorem is 1: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"qmd-theorem-label\">Lemma<span class=\"qmd-theorem-number\">&nbsp;1</span></span>"
        ),
        "lemma counts independently: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\">&nbsp;2</span></span>"
        ),
        "second theorem is 2: {body}"
    );
}

#[test]
fn theorem_crossref_resolves_with_label_and_number() {
    let doc = render_document(
        "See @thm-pyth and @lem-bound.\n\n::: {.theorem #thm-pyth}\nA.\n:::\n\n::: {.lemma #lem-bound}\nB.\n:::\n",
    );
    let body = doc.body_html();
    assert!(
        body.contains("<a href=\"#thm-pyth\" class=\"qmd-xref\">Theorem&nbsp;1</a>"),
        "got: {body}"
    );
    assert!(
        body.contains("<a href=\"#lem-bound\" class=\"qmd-xref\">Lemma&nbsp;1</a>"),
        "got: {body}"
    );
    assert!(!body.contains("@thm-pyth"), "ref left unresolved: {body}");
}

#[test]
fn proof_is_not_numbered() {
    let doc = render_document("::: {.proof}\nx.\n:::\n");
    assert!(
        !doc.body_html().contains("qmd-theorem-number"),
        "proof has no number slot: {}",
        doc.body_html()
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p qmd-fast-core theorems_number_continuously_per_kind theorem_crossref_resolves_with_label_and_number proof_is_not_numbered`
Expected: `theorems_number_continuously_per_kind` and `theorem_crossref_resolves_with_label_and_number` FAIL (number slot stays empty; `@thm-`/`@lem-` are unresolved because `lem` is unknown to `xref_label` and nothing is registered). `proof_is_not_numbered` PASSES already (no behavior added yet) — that is fine, it guards Task 2 from regressing proof.

- [ ] **Step 3: Teach `xref_label` the rest of the prefixes**

In `crates/core/src/cite/render.rs`, extend the `xref_label` match (lines 12-21) so it reads:

```rust
fn xref_label(prefix: &str) -> Option<&'static str> {
    match prefix {
        "fig" => Some("Figure"),
        "tbl" => Some("Table"),
        "sec" => Some("Section"),
        "eq" => Some("Equation"),
        "lst" => Some("Listing"),
        "thm" => Some("Theorem"),
        "lem" => Some("Lemma"),
        "cor" => Some("Corollary"),
        "prp" => Some("Proposition"),
        "def" => Some("Definition"),
        "exm" => Some("Example"),
        "rem" => Some("Remark"),
        _ => None,
    }
}
```

- [ ] **Step 4: Add the `number_theorems` post-pass**

In `crates/core/src/render/mod.rs`, add directly above `fn apply_table_captions` (line 1216):

```rust
/// Assign continuous, per-kind theorem numbers in document order (Theorem 1, 2, …;
/// Lemma 1, 2, … independently), fill each theorem's number slot, and register its
/// `#thm-`/`#lem-`/… anchor so `@thm-x` resolves. Runs after `apply_table_captions`
/// and before `cite::process`. `proof` carries no `data-qmd-theorem-kind`, so it is
/// skipped (unnumbered, unreferenceable). Top-level theorems only — a theorem nested
/// inside another container is embedded in the parent block's HTML (same limitation as
/// table captions). The container id is read from the OPENING tag only (via `tag_end`)
/// so a child block's `id=` is never mistaken for the theorem anchor.
fn number_theorems(
    blocks: &mut [Block],
    xrefs: &mut HashMap<String, String>,
    warnings: &mut Vec<Warning>,
) {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for b in blocks.iter_mut() {
        let tag_end_idx = tag_end(&b.html).map(|i| i + 1).unwrap_or(b.html.len());
        let open_tag = b.html[..tag_end_idx].to_string();
        let Some(kind) = extract_attr(&open_tag, "data-qmd-theorem-kind") else {
            continue;
        };
        let n = {
            let c = counts.entry(kind).or_insert(0);
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

- [ ] **Step 5: Wire the post-pass into the pipeline**

In `crates/core/src/render/mod.rs`, the render pipeline currently reads (lines 584-586):

```rust
    apply_table_captions(&mut blocks, &mut xref_registry, &mut warnings);
    let bib = load_bibliography(bib_field.as_deref(), base_dir, &mut warnings);
    warnings.extend(crate::cite::process(&mut blocks, &bib, &xref_registry));
```

Insert the theorem pass directly after `apply_table_captions` (so it registers anchors before `cite::process` resolves them):

```rust
    apply_table_captions(&mut blocks, &mut xref_registry, &mut warnings);
    // Theorem environments: number per-kind in document order + register #thm-/#lem-/…
    // anchors. Must run before cite::process resolves @thm-/@lem-/… references.
    number_theorems(&mut blocks, &mut xref_registry, &mut warnings);
    let bib = load_bibliography(bib_field.as_deref(), base_dir, &mut warnings);
    warnings.extend(crate::cite::process(&mut blocks, &bib, &xref_registry));
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p qmd-fast-core theorems_number_continuously_per_kind theorem_crossref_resolves_with_label_and_number proof_is_not_numbered`
Expected: PASS (3 tests). Then run the full Task 1 set again to confirm no regression: `cargo test -p qmd-fast-core theorem_div_emits_styled_block_with_number_slot`.

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/cite/render.rs crates/core/src/render/mod.rs crates/core/src/render/tests.rs
git commit -m "feat(render): number theorems (per-kind, continuous) + thm/lem/cor/prp/exm/rem cross-refs"
```

---

### Task 3: Styling, corpus pin, and verification

Add the CSS, the pinned corpus document, and verify the whole pipeline + the visual result.

**Files:**
- Modify: `crates/core/assets/css/base.css` (tokens near line 13; rules after the callout block ~line 458)
- Create: `corpus/refs/theorems.qmd`
- Modify: `corpus/README.md` (add a row in the documents table)
- Test: the corpus walking tests in `crates/core/tests/corpus.rs` (auto-discover the new doc; no new test code) + a browser screenshot

**Interfaces:**
- Consumes: the class contract emitted in Tasks 1-2 (`.qmd-theorem`, `.qmd-theorem-head`, `.qmd-theorem-title`, `.qmd-theorem-body`, `.qmd-thm-style-{plain,definition,remark}`, `.qmd-proof`, `.qmd-proof-head`, `.qmd-qed`).
- Produces: bundled CSS (inlined via `include_str!` in `mod.rs`); the regression pin.

- [ ] **Step 1: Add the theme tokens**

In `crates/core/assets/css/base.css`, the `:root` block defines callout tokens at lines 13-14. Add a theorem-token line directly after line 14:

```css
    --qmd-thm-plain: #6b5ea8; --qmd-thm-definition: #2a7a5e; --qmd-thm-remark: #7a7f87;
```

- [ ] **Step 2: Add the theorem rules**

In `crates/core/assets/css/base.css`, after the callout rules block (the last callout rule is at line 458, `.callout-collapse > details[open] > summary.callout-title { margin-bottom: 0; }`), insert:

```css
  /* theorem environments (definition/theorem/lemma/proof/…); light + dark from one
     token set via color-mix, matching the callout aesthetic but lighter. */
  .qmd-theorem { border: 1px solid var(--qmd-border); border-left-width: 4px; border-radius: 5px; margin: 1.1rem 0; }
  .qmd-theorem-head { font-family: ui-sans-serif, system-ui, sans-serif; font-weight: 600; margin: 0; padding: .3rem .9rem; }
  .qmd-theorem-title { font-weight: 600; }
  .qmd-theorem-body { padding: .3rem .9rem .6rem; }
  .qmd-theorem-body > :first-child { margin-top: .4rem; }
  .qmd-thm-style-plain { border-left-color: var(--qmd-thm-plain); }
  .qmd-thm-style-definition { border-left-color: var(--qmd-thm-definition); }
  .qmd-thm-style-remark { border-left-color: var(--qmd-thm-remark); }
  .qmd-thm-style-plain .qmd-theorem-head { background: color-mix(in srgb, var(--qmd-thm-plain) 12%, transparent); }
  .qmd-thm-style-definition .qmd-theorem-head { background: color-mix(in srgb, var(--qmd-thm-definition) 12%, transparent); }
  .qmd-thm-style-remark .qmd-theorem-head { background: color-mix(in srgb, var(--qmd-thm-remark) 10%, transparent); }
  /* amsthm convention: plain-style bodies are italic; definition/remark stay upright. */
  .qmd-thm-style-plain .qmd-theorem-body { font-style: italic; }
  /* proof: italic lead, right-floated QED. */
  .qmd-proof { margin: 1.1rem 0; }
  .qmd-proof-head { font-style: italic; font-weight: 600; margin: 0 0 .2rem; }
  .qmd-qed { display: block; text-align: right; }
```

(No unit test: CSS is verified by the corpus invariants in Step 4 and the browser screenshot in Step 5, per the project's screenshot-loop convention.)

- [ ] **Step 3: Create the pinned corpus document**

Create `corpus/refs/theorems.qmd` (front-matter is `title:` only, so the corpus front-matter validator stays silent; no em or en dashes):

```markdown
---
title: "Theorem environments"
---

# Groups

::: {.definition #def-group}
A **group** is a set $G$ with an associative binary operation, an identity element, and an inverse for every element.
:::

By @def-group, the integers under addition form a group.

::: {.theorem #thm-lagrange title="Lagrange"}
For a finite group $G$ and a subgroup $H \le G$, the order $|H|$ divides $|G|$.
:::

::: {.lemma #lem-coset}
Distinct left cosets of $H$ are disjoint and have equal size.
:::

::: {.proof}
Partition $G$ into left cosets of $H$ using @lem-coset, then count.
:::

::: {.corollary}
A group of prime order is cyclic.
:::

::: {.proposition}
The intersection of two subgroups of $G$ is again a subgroup.
:::

::: {.example}
The residues $\mathbb{Z}/n\mathbb{Z}$ form a group of order $n$ under addition.
:::

::: {.remark}
@thm-lagrange does not extend to infinite groups.
:::
```

- [ ] **Step 4: Add the corpus README row and run the test suite**

In `corpus/README.md`, add a row to the documents table (after the `reactive/js-error.qmd` row, keeping the table's column shape):

```markdown
| `refs/theorems.qmd` | Theorem environments | all 8 kinds across the 3 amsthm styles, `title=`, a proof with auto-QED, per-kind continuous numbering, and `@thm-`/`@def-`/`@lem-` cross-refs resolving | (purpose-built) |
```

Run the full core suite (the three `every_corpus_doc_*` walkers auto-discover `corpus/refs/theorems.qmd`):

Run: `cargo test -p qmd-fast-core`
Expected: PASS (all unit + corpus tests, including `every_corpus_doc_has_clean_front_matter`, `every_corpus_doc_emits_no_unknown_key_warnings`, `every_corpus_doc_renders_with_invariants`).

- [ ] **Step 5: Browser-verify the rendered result**

Build and serve the pin doc, then screenshot it light + dark via the chrome-devtools MCP (use the `/preview` skill, which serves on port 4388 and verifies in-browser):

Run: `cargo run -p qmd-fast-server -- preview corpus/refs/theorems.qmd 4388`
Then, in the browser (chrome-devtools MCP): navigate to `http://localhost:4388`, take a screenshot, and confirm:
- each kind shows its label + number (Definition 1, Theorem 1, Lemma 1, Corollary 1, Proposition 1, Example 1, Remark 1), the proof shows "Proof." + a right-floated ∎,
- the three styles read distinctly (plain bodies italic; definition/remark upright), with the left accent color per style,
- math inside theorems renders,
- `@thm-lagrange`/`@def-group`/`@lem-coset` are clickable "Theorem 1"/"Definition 1"/"Lemma 1" links,
- toggle the theme (dark) and confirm the accents/tints adapt (no unreadable contrast),
- console has no errors.

Verify there are no `<system-reminder>`-flagged console errors before claiming success.

- [ ] **Step 6: Commit**

```bash
git add crates/core/assets/css/base.css corpus/refs/theorems.qmd corpus/README.md
git commit -m "feat(assets): theorem CSS + pin corpus/refs/theorems.qmd"
```

---

## Later phases (separate plans, when relevant)

Per the spec, each gets its own plan + corpus pin when it becomes the next piece of work:

- **Phase 2 — numbering config + parity polish:** a validated `theorems:` block (`number-within: none|section|chapter`, `shared: […]`, `numbered`), book-page "Theorem 2.3" scoping wired into `site/chapter.rs`, shared counters (the differentiator), and singular/plural/capitalized reference names.
- **Phase 3 — web-native affordances:** hover-preview of `@thm-` refs (extend the reader/ hover cross-ref card), collapsible proofs (`proof collapse="true"` reusing the callout `<details>` pattern), deep-link anchors + clickable QED.
- **Phase 4 — rich deck support:** extract a shared `assets/css/theorem.css` into both `base.css` and `deck.css`, reveal proofs via the existing `.fragment` mechanism, per-slide-group numbering via a `QmdDeck.registerPlugin` client plugin.

When Phase 1 lands on `main`, move `crossref-family-and-labels` in `notes/BEYOND-QUARTO.md` from CUT to an active/closed Pillar IV item referencing the spec.

## Self-review

- **Spec coverage (Phase 1):** kinds + 3 styles → Task 1; proof + auto-QED + rename → Task 1; emission/block contract → Task 1; continuous per-kind numbering → Task 2; `@thm-`/… cross-refs → Task 2; CSS/theming → Task 3; corpus pin + invariants → Task 3; ARIA/ math → Task 1 (visible head text read in order; math transparent). The spec's "validator did-you-mean" and "aria-labelledby" lines were tightened during planning (no namespace prefix for did-you-mean; no forced ARIA role since a theorem is primary content, not a "note"); the spec is being updated to match.
- **Placeholder scan:** none; every code/test/CSS step shows complete content and every command an expected result.
- **Type consistency:** `THEOREM_KINDS` (validate.rs) is the single dispatch source used by `DivAttrs::theorem_kind`; the container HTML contract (`data-qmd-theorem-kind`, the `qmd-theorem-number` slot, the optional container `id`) emitted in Task 1 is exactly what `number_theorems` consumes in Task 2; the `qmd-theorem*`/`qmd-proof` classes emitted in Tasks 1-2 are exactly what Task 3's CSS targets.
