# Theorem Environments — Phase 2 (increment 1): config foundation + shared counters

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a validated `theorems:` front-matter block and implement `shared` counters (the differentiator: listed kinds draw one numbering sequence), so an author can write `theorems: { shared: [theorem, lemma, corollary, proposition] }` and get Theorem 1, Lemma 2, Theorem 3, ….

**Architecture:** A `TheoremConfig` parsed from front-matter (serde_yaml) in `render/fm_extract.rs`, threaded into the existing `number_theorems` post-pass, which keys its per-kind counter by a shared-group lookup (`TheoremConfig::counter_key`). Validation + JSON-schema follow the existing nested-block pattern (`execute:`/`listing:`). The visible label stays per-kind; only the number is shared.

**Tech Stack:** Rust (edition 2024), serde_yaml (already a runtime dep, used by `frontmatter::validate_front_matter`), serde_json (dev-dep, schema generator).

**Spec:** `docs/superpowers/specs/2026-06-29-theorem-environments-design.md`, Phase 2. This increment ships ONLY the `shared` key; `number-within` and `numbered` are later increments (the keys must be honored, not accepted-and-ignored, so they are NOT added to the allowlist until implemented).

## Global Constraints

- HTML-only output; preview read-only; block-model invariants preserved (this increment does not change emitted block ids/sourcepos at all, only the number text inside a theorem block).
- Read-only-additive: a new struct + parser, an additive validator entry, an additive schema entry, one new param on the private `number_theorems`. No scanner / numbering-scanner / cite-lowering / deck change.
- Clean-break vocabulary: every recognized key must be honored. Ship `shared` only; do NOT add `number-within`/`numbered` to `THEOREM_KEYS` yet (an author using them should still get an "unknown theorems key" warning until those features land).
- Schema is drift-locked: after changing `KNOWN_KEYS`/the generator, regenerate the golden file with `QMD_FAST_BLESS=1 cargo test -p qmd-fast-core --lib schema` or the schema test fails.
- `rustfmt` hook runs on edited `.rs`; keep `cargo fmt`-clean. No em/en dashes in authored text.
- Shared-group semantics: one group per doc (`shared:` is a flat list). All listed kinds collapse to the group's first member's counter; unlisted kinds keep their own. Multiple independent groups are out of scope (YAGNI).

---

### Task 1: `theorems:` config foundation (struct, parser, validation, schema)

**Files:**
- Modify: `crates/core/src/render/fm_extract.rs` (add `TheoremConfig` + `parse_theorem_config`)
- Modify: `crates/core/src/render/mod.rs:46` (import them)
- Modify: `crates/core/src/frontmatter.rs` (add `"theorems"` to `KNOWN_KEYS` line 19-59; add `THEOREM_KEYS` const ~line 76; add a `validate_nested` call ~line 118)
- Modify: `crates/core/src/schema.rs` (import `THEOREM_KEYS`; add a `theorems` override in `front_matter_schema`)
- Modify: `crates/core/assets/schema/qmd-frontmatter.schema.json` (regenerated via bless)
- Test: `crates/core/src/render/tests.rs` (parser/counter_key unit test), `crates/core/src/frontmatter.rs` tests mod (validator test)

**Interfaces:**
- Produces:
  - `pub(crate) struct TheoremConfig { shared: Vec<String> }` with `#[derive(Default)]` and `pub(crate) fn counter_key<'a>(&'a self, kind: &'a str) -> &'a str`.
  - `pub(crate) fn parse_theorem_config(front_matter: &str) -> TheoremConfig`.
  - `frontmatter::THEOREM_KEYS: &[&str]` (`["shared"]`).
- Consumes: `serde_yaml`, the `closed_object`/`properties`/`overrides` schema helpers, the `validate_nested` helper.

- [ ] **Step 1: Write the failing tests**

Append to `crates/core/src/render/tests.rs`:

```rust
#[test]
fn theorem_config_shared_group_shares_counter_key() {
    let cfg = parse_theorem_config("theorems:\n  shared: [theorem, lemma]\n");
    assert_eq!(
        cfg.counter_key("theorem"),
        cfg.counter_key("lemma"),
        "shared kinds collapse to one counter key"
    );
    assert_ne!(
        cfg.counter_key("theorem"),
        cfg.counter_key("definition"),
        "an unlisted kind keeps its own key"
    );
    let none = parse_theorem_config("title: x\n");
    assert_ne!(
        none.counter_key("theorem"),
        none.counter_key("lemma"),
        "no config means per-kind counters"
    );
}
```

Add to the `#[cfg(test)] mod tests` in `crates/core/src/frontmatter.rs` (which already has a `fn msgs(src: &str) -> Vec<String>` helper):

```rust
    #[test]
    fn theorems_block_is_validated() {
        assert!(
            msgs("---\ntheorems:\n  shared: [theorem, lemma]\n---\n").is_empty(),
            "a valid theorems block must not warn"
        );
        let m = msgs("---\ntheorems:\n  shard: [theorem]\n---\n");
        assert!(
            m.iter()
                .any(|w| w.contains("unknown theorems key `shard`") && w.contains("shared")),
            "a typo'd child key warns with did-you-mean: {m:?}"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p qmd-fast-core --lib theorem_config_shared_group_shares_counter_key`
Expected: FAIL to COMPILE (`parse_theorem_config`/`TheoremConfig` undefined).
Run: `cargo test -p qmd-fast-core --lib theorems_block_is_validated`
Expected: FAIL (`theorems` is an unknown top-level key, so the "valid block must not warn" assertion fails).

- [ ] **Step 3: Add `TheoremConfig` + `parse_theorem_config`**

Append to `crates/core/src/render/fm_extract.rs`:

```rust
/// Parsed `theorems:` front-matter config. This increment carries only `shared` (the
/// kinds that draw one shared numbering sequence). `number-within` and `numbered` are
/// future increments.
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

/// Parse the `theorems:` block out of a front-matter string into a `TheoremConfig`.
/// An absent block, a parse failure, or an unexpected shape yields the default
/// (per-kind numbering). `shared:` is a YAML list of kind names. Uses serde_yaml,
/// already a dependency (see `frontmatter::validate_front_matter`).
pub(crate) fn parse_theorem_config(front_matter: &str) -> TheoremConfig {
    let mut config = TheoremConfig::default();
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(front_matter) else {
        return config;
    };
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
    config
}
```

- [ ] **Step 4: Export them from `mod.rs`**

In `crates/core/src/render/mod.rs`, line 46 currently reads:

```rust
use fm_extract::{detect_format, detect_title_block_hidden, detect_toc, extract_field};
```

Replace with:

```rust
use fm_extract::{
    TheoremConfig, detect_format, detect_title_block_hidden, detect_toc, extract_field,
    parse_theorem_config,
};
```

- [ ] **Step 5: Register `theorems` in the validator**

In `crates/core/src/frontmatter.rs`, add `"theorems"` to `KNOWN_KEYS` (after `"prose-lint",` at line 58):

```rust
    "prose-lint",
    // Theorem environments (per-document numbering config; see render::TheoremConfig).
    "theorems",
];
```

Add the child-key allowlist after `PROSE_LINT_KEYS` (line 76):

```rust
/// `theorems:` sub-keys qmd-fast honors. This increment honors only `shared` (shared
/// counters); `number-within` + `numbered` are added when those features land, so an
/// author using them still gets an "unknown theorems key" warning until then.
pub(crate) const THEOREM_KEYS: &[&str] = &["shared"];
```

Add the validation call in `validate_front_matter`, after the `prose-lint` call (line 118):

```rust
    validate_nested(map, "theorems", "theorems key", THEOREM_KEYS, block, &mut out);
```

- [ ] **Step 6: Add the schema entry**

In `crates/core/src/schema.rs`, extend the `generate` module's import (line 20-22) to include `THEOREM_KEYS`:

```rust
    use crate::frontmatter::{
        ABOUT_KEYS, EXECUTE_KEYS, HERO_KEYS, KNOWN_KEYS, LISTING_KEYS, PROSE_LINT_KEYS,
        THEOREM_KEYS,
    };
```

In `front_matter_schema()`, build the theorems sub-schema (after the `prose_lint` block, before `let overrides`):

```rust
        // theorems: `shared` is a list of kind names sharing one counter.
        let theorems = closed_object(
            THEOREM_KEYS,
            &[("shared", json!({ "type": "array", "items": { "type": "string" } }))],
        );
```

Add it to the `overrides` array (after `("prose-lint", prose_lint),`):

```rust
            ("theorems", theorems),
```

- [ ] **Step 7: Regenerate the golden schema, then run the tests**

Bless the golden file (adding `"theorems"` to `KNOWN_KEYS` changed the generated schema):

Run: `QMD_FAST_BLESS=1 cargo test -p qmd-fast-core --lib schema`
Expected: prints `blessed assets/schema/qmd-frontmatter.schema.json`; tests pass.

Then verify the new tests and the schema drift-lock all pass:

Run: `cargo test -p qmd-fast-core --lib theorem_config_shared_group_shares_counter_key`
Run: `cargo test -p qmd-fast-core --lib theorems_block_is_validated`
Run: `cargo test -p qmd-fast-core --lib schema`
Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/core/src/render/fm_extract.rs crates/core/src/render/mod.rs crates/core/src/frontmatter.rs crates/core/src/schema.rs crates/core/assets/schema/qmd-frontmatter.schema.json crates/core/src/render/tests.rs
git commit -m "feat(render): theorems: front-matter config (shared key) + validation + schema"
```

---

### Task 2: Shared counters in `number_theorems`

**Files:**
- Modify: `crates/core/src/render/mod.rs` (`number_theorems` signature + counter key ~line 1227; the local + FrontMatter arm ~line 211/276; the call site ~line 587)
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `TheoremConfig::counter_key` (Task 1), `parse_theorem_config` (Task 1).
- Produces: `number_theorems(blocks, xrefs, warnings, config: &TheoremConfig)` numbering shared kinds as one sequence.

- [ ] **Step 1: Write the failing test**

Append to `crates/core/src/render/tests.rs`:

```rust
#[test]
fn shared_counter_numbers_across_kinds() {
    let doc = render_document(
        "---\ntheorems:\n  shared: [theorem, lemma]\n---\n\n::: {.theorem}\nA.\n:::\n\n::: {.lemma}\nB.\n:::\n\n::: {.theorem}\nC.\n:::\n\n::: {.definition}\nD.\n:::\n",
    );
    let body = doc.body_html();
    // theorem + lemma draw one sequence: Theorem 1, Lemma 2, Theorem 3
    assert!(
        body.contains(
            "<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\">&nbsp;1</span></span>"
        ),
        "got: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"qmd-theorem-label\">Lemma<span class=\"qmd-theorem-number\">&nbsp;2</span></span>"
        ),
        "lemma takes the shared sequence's 2: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"qmd-theorem-label\">Theorem<span class=\"qmd-theorem-number\">&nbsp;3</span></span>"
        ),
        "got: {body}"
    );
    // definition is NOT shared: its own counter starts at 1
    assert!(
        body.contains(
            "<span class=\"qmd-theorem-label\">Definition<span class=\"qmd-theorem-number\">&nbsp;1</span></span>"
        ),
        "unlisted kind keeps its own counter: {body}"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p qmd-fast-core --lib shared_counter_numbers_across_kinds`
Expected: FAIL to COMPILE first (the `number_theorems` call has no `config` arg yet) once Step 3-5 are partially applied — but as written before any Step 3 change, it FAILS on the assertion (lemma numbers as 1, not 2, because counters are still per-kind).

- [ ] **Step 3: Thread the config through `number_theorems`**

In `crates/core/src/render/mod.rs`, change the `number_theorems` signature and the counter key. The function currently is:

```rust
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
```

Change the signature line and the counter-key lines to:

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
```

(The rest of the function body, the number-slot `replacen` and the `register_xref`, is unchanged.)

- [ ] **Step 4: Declare the config local and parse it in the front-matter arm**

In `crates/core/src/render/mod.rs`, add the local after `let mut exec_cache = true;` (line 211):

```rust
    let mut theorem_config = TheoremConfig::default();
```

In the `NodeValue::FrontMatter(fm)` arm, after `(exec_echo, exec_include, exec_cache) = detect_execute_defaults(fm);` (line ~276):

```rust
                theorem_config = parse_theorem_config(fm);
```

- [ ] **Step 5: Pass the config at the call site**

In `crates/core/src/render/mod.rs`, the call (line ~587) currently reads:

```rust
    number_theorems(&mut blocks, &mut xref_registry, &mut warnings);
```

Change to:

```rust
    number_theorems(&mut blocks, &mut xref_registry, &mut warnings, &theorem_config);
```

- [ ] **Step 6: Run the test (and the Phase 1 numbering test for regression)**

Run: `cargo test -p qmd-fast-core --lib shared_counter_numbers_across_kinds`
Expected: PASS.
Run: `cargo test -p qmd-fast-core --lib theorems_number_continuously_per_kind`
Expected: PASS (no-config behavior unchanged: per-kind counters).

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/render/mod.rs crates/core/src/render/tests.rs
git commit -m "feat(render): shared theorem counters (theorems: shared: [...])"
```

---

### Task 3: Corpus pin + verification

**Files:**
- Create: `corpus/refs/theorems-shared.qmd`
- Modify: `corpus/README.md`
- Test: corpus walkers (auto-discover) + browser screenshot

**Interfaces:**
- Consumes: the `theorems: shared:` config (Tasks 1-2).

- [ ] **Step 1: Create the pin document**

Create `corpus/refs/theorems-shared.qmd` (front-matter has `title:` + the new `theorems:` block, both now in the validator vocabulary; no em/en dashes):

```markdown
---
title: "Shared theorem counters"
theorems:
  shared: [theorem, lemma, corollary, proposition]
---

# One sequence

These four kinds share a single counter, so the numbers run continuously across them
(amsthm's `\newtheorem{lem}[thm]{Lemma}`), while definitions number on their own.

::: {.theorem #thm-a}
First, a theorem.
:::

::: {.lemma #lem-b}
Then a lemma, which continues the same sequence.
:::

::: {.definition #def-x}
A definition counts separately.
:::

::: {.corollary #cor-c}
A corollary, still on the shared sequence.
:::

By @thm-a, @lem-b, and @cor-c, the shared numbering reads as one run; @def-x stands apart.
```

- [ ] **Step 2: Add the README row**

In `corpus/README.md`, after the `refs/theorems.qmd` row:

```markdown
| `refs/theorems-shared.qmd` | Shared theorem counters | `theorems: shared: [...]` makes theorem/lemma/corollary/proposition draw one sequence (Theorem 1, Lemma 2, Corollary 3) while `definition` counts separately; cross-refs resolve to the shared numbers | (purpose-built) |
```

- [ ] **Step 3: Run the full core suite**

The corpus walkers auto-discover the new doc and assert clean front-matter (`theorems` + `shared` are now recognized), no unknown-key warnings, and block invariants.

Run: `cargo test -p qmd-fast-core`
Expected: PASS (all). If `every_corpus_doc_emits_no_unknown_key_warnings` fails on `theorems`/`shared`, Task 1's validator changes are incomplete.

- [ ] **Step 4: Browser-verify**

Run: `cargo build -p qmd-fast-server` then `fuser -k 4388/tcp; ./target/debug/qmd-fast serve corpus/refs/theorems-shared.qmd 4388` (background); wait for HTTP 200.
In chrome-devtools: navigate to `http://127.0.0.1:4388/`, screenshot, and confirm:
- the shared sequence runs continuously across kinds in document order: Theorem 1, Lemma 2, then (definition is separate) Corollary 3,
- Definition 1 (its own counter, unaffected by the shared run),
- `@thm-a`/`@lem-b`/`@cor-c`/`@def-x` resolve to the matching numbers,
- no console errors.

Confirm no console errors before claiming success.

- [ ] **Step 5: Commit**

```bash
git add corpus/refs/theorems-shared.qmd corpus/README.md
git commit -m "feat(corpus): pin shared theorem counters (corpus/refs/theorems-shared.qmd)"
```

---

## Self-review

- **Spec coverage (this increment):** the `theorems:` config surface (validated + schema'd) → Task 1; shared counters (the differentiator) → Task 2; corpus pin → Task 3. Deferred to later Phase 2 increments (own plans): `number-within` book scoping ("Theorem 2.3" via `site/chapter.rs`), `numbered: false|unless-unique`, and singular/plural reference names.
- **Placeholder scan:** none; every step has complete code and an expected result.
- **Type consistency:** `TheoremConfig`/`parse_theorem_config` (Task 1, `fm_extract.rs`) are imported in `mod.rs` (Task 1 Step 4) and consumed by `number_theorems` (Task 2); `THEOREM_KEYS` (Task 1 validator) is reused by the schema generator (Task 1 Step 6); the counter-key contract (`counter_key`) is the single source the post-pass uses. The shared corpus doc (Task 3) exercises exactly the `shared:` shape Task 1 parses and Task 2 honors.
- **Numbering trace** for `theorems-shared.qmd` (document order): theorem #thm-a = shared 1, lemma #lem-b = shared 2, definition #def-x = own 1, corollary #cor-c = shared 3. So the expected render is Theorem 1, Lemma 2, Definition 1, Corollary 3, and `@thm-a`/`@lem-b`/`@cor-c`/`@def-x` resolve to 1/2/3/1 respectively.
