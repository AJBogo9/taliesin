# Theorem Environments — Phase 2 (increment 3): `numbered: false | unless-unique`

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or subagent-driven-development). Steps use `- [ ]` tracking.

**Goal:** `theorems: { numbered: false }` renders theorem kinds with their label but no number (and no cross-ref target); `numbered: unless-unique` numbers a kind only when it appears more than once in the doc (a lone Theorem shows just "Theorem"). Default (`true`/absent) is unchanged.

**Architecture:** A `Numbered` enum on `TheoremConfig`. `number_theorems` does a pre-count of occurrences per counter-key, then decides per theorem whether to show a number; an unnumbered theorem leaves its number slot empty and is not registered as a cross-ref target. Composes with `shared` (count per shared group) and `number-within` (the shown number is still chapter-scoped).

## Global Constraints

- HTML-only; read-only-additive; block ids/sourcepos untouched (only the number-slot content changes). Clean-break: `numbered` becomes a honored key (add to `THEOREM_KEYS`).
- Schema drift-lock: re-bless after `THEOREM_KEYS` changes. `rustfmt`-clean. No em/en dashes.
- `numbered: false` suppresses the cross-ref target too (an unnumbered theorem has no number to point at; `@thm-x` to it stays unresolved and warns, which is correct).

---

### Task 1: config + numbering logic (TDD)

**Files:** `crates/core/src/render/fm_extract.rs` (`Numbered` enum + field + accessor + parse), `crates/core/src/frontmatter.rs` (`THEOREM_KEYS`), `crates/core/src/schema.rs` (+ golden file), `crates/core/src/render/mod.rs` (`number_theorems`), tests in `crates/core/src/render/tests.rs`.

- [ ] **Step 1: failing tests** (append to `render/tests.rs`):

```rust
#[test]
fn numbered_false_suppresses_the_number() {
    let doc = render_document(
        "---\ntheorems:\n  numbered: false\n---\n\n::: {.theorem}\nA.\n:::\n\n::: {.theorem}\nB.\n:::\n",
    );
    let body = doc.body_html();
    assert!(
        body.contains("<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\"></span></span>"),
        "numbered: false leaves the number slot empty: {body}"
    );
    assert!(
        !body.contains("qmd-theorem-number\">&nbsp;"),
        "no number is emitted anywhere: {body}"
    );
}

#[test]
fn numbered_unless_unique_numbers_only_repeated_kinds() {
    let doc = render_document(
        "---\ntheorems:\n  numbered: unless-unique\n---\n\n::: {.definition}\nLone.\n:::\n\n::: {.theorem}\nT1.\n:::\n\n::: {.theorem}\nT2.\n:::\n",
    );
    let body = doc.body_html();
    // definition appears once -> unnumbered
    assert!(
        body.contains("<span class=\"qmd-theorem-label\">Definition<span class=\"qmd-theorem-number\"></span></span>"),
        "a lone kind is unnumbered: {body}"
    );
    // theorem appears twice -> numbered 1, 2
    assert!(
        body.contains("<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\">&nbsp;1</span></span>")
            && body.contains("<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\">&nbsp;2</span></span>"),
        "a repeated kind is numbered: {body}"
    );
}
```

Add to `frontmatter.rs` tests mod:

```rust
    #[test]
    fn theorems_numbered_is_recognized() {
        assert!(msgs("---\ntheorems:\n  numbered: false\n---\n").is_empty());
        assert!(msgs("---\ntheorems:\n  numbered: unless-unique\n---\n").is_empty());
    }
```

- [ ] **Step 2: run, verify fail.** `cargo test -p qmd-fast-core --lib numbered_` (compile error: `Numbered`/field absent) and `theorems_numbered_is_recognized` (unknown key warns).

- [ ] **Step 3: `Numbered` enum + field + accessor + parse** (`fm_extract.rs`).

Add enum above `TheoremConfig`:

```rust
/// `theorems: numbered:` mode. `UnlessUnique` numbers a kind only when it appears more
/// than once (a lone Theorem shows just "Theorem").
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Numbered {
    #[default]
    Yes,
    No,
    UnlessUnique,
}
```

Add field `numbered: Numbered,` to `TheoremConfig` and an accessor:

```rust
    /// The `numbered:` mode (whether/when to show a number).
    pub(crate) fn numbered(&self) -> Numbered {
        self.numbered
    }
```

In `parse_theorem_config`, after the `number-within` block:

```rust
    match value.get("theorems").and_then(|t| t.get("numbered")) {
        Some(serde_yaml::Value::Bool(false)) => config.numbered = Numbered::No,
        Some(v) if v.as_str() == Some("unless-unique") => config.numbered = Numbered::UnlessUnique,
        _ => {} // true / absent / unrecognized -> Yes (default)
    }
```

- [ ] **Step 4: `THEOREM_KEYS` += `"numbered"`** (`frontmatter.rs`): `&["shared", "number-within", "numbered"]`.

- [ ] **Step 5: schema** (`schema.rs`): add to the theorems override `("numbered", json!({ "oneOf": [ { "type": "boolean" }, { "type": "string" } ] }))`. Import `Numbered`? No — schema uses `THEOREM_KEYS` only.

- [ ] **Step 6: `number_theorems` logic** (`render/mod.rs`). Import `Numbered` (add to the `use fm_extract::{...}` line). At the top of `number_theorems`, add a pre-count pass (before the main loop), and gate the displayed number:

```rust
    // For `numbered: unless-unique`, a kind is numbered only if it occurs more than
    // once; pre-count occurrences per counter-key.
    let mut totals: HashMap<String, u32> = HashMap::new();
    if config.numbered() == Numbered::UnlessUnique {
        for b in blocks.iter() {
            let end = tag_end(&b.html).map(|i| i + 1).unwrap_or(b.html.len());
            if let Some(kind) = extract_attr(&b.html[..end], "data-qmd-theorem-kind") {
                *totals.entry(config.counter_key(&kind).to_string()).or_insert(0) += 1;
            }
        }
    }
```

Inside the loop, after computing `n` and before building `display`, decide `show_number`, and only build a number / register a ref when shown. Replace the `display` computation + slot + xref tail with:

```rust
        let show_number = match config.numbered() {
            Numbered::Yes => true,
            Numbered::No => false,
            Numbered::UnlessUnique => totals.get(&key).copied().unwrap_or(0) > 1,
        };
        let display = if !show_number {
            String::new()
        } else if config.chapter_scoped() {
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
        // An unnumbered theorem leaves the slot empty (no &nbsp;) and is not a ref target.
        let slot = if display.is_empty() {
            String::new()
        } else {
            format!("&nbsp;{display}")
        };
        b.html = b.html.replacen(
            "<span class=\"qmd-theorem-number\"></span>",
            &format!("<span class=\"qmd-theorem-number\">{slot}</span>"),
            1,
        );
        if !display.is_empty()
            && let Some(id) = extract_attr(&open_tag, "id")
        {
            register_xref(xrefs, warnings, &id, display);
        }
```

(The `let n = { counts.entry(key)... }` still runs for every theorem so a numbered sibling keeps a stable sequence; `key` must stay in scope for the `totals` lookup — it is, computed just above.)

- [ ] **Step 7: re-bless schema + run.** `QMD_FAST_BLESS=1 cargo test -p qmd-fast-core --lib schema`; then `cargo test -p qmd-fast-core --lib numbered_`, `theorems_numbered_is_recognized`, and the full `theorem`/`shared_counter`/`number_within` sets (regression). Then `cargo test -p qmd-fast-core`.

- [ ] **Step 8: commit.** `feat(render): theorems: numbered (false | unless-unique)`.

---

### Task 2: corpus pin + verify

- [ ] Add a small `corpus/refs/theorems-unnumbered.qmd` (front-matter `numbered: unless-unique`, a lone `definition` + two `theorem`s) + a README row.
- [ ] `cargo test -p qmd-fast-core` (corpus walkers accept `numbered`; invariants hold).
- [ ] Browser: `serve corpus/refs/theorems-unnumbered.qmd` — confirm the lone Definition shows no number, the two Theorems show 1 and 2, no console errors.
- [ ] Commit `feat(corpus): pin numbered: unless-unique`.

## Self-review
- Composes: `numbered` gates display on top of `shared` (totals per counter-key) and `number-within` (shown number still chapter-scoped). `numbered: false` overrides scoping. Slot stays empty (no stray `&nbsp;`) when unnumbered; unnumbered theorems are not ref targets.
- Deferred (Phase 2 remainder): reference-name polish. Phases 3-4 unchanged.
