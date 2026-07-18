# DX5 — div/theorem-class did-you-mean + `.columns` alias: Plan

> REQUIRED SUB-SKILL: superpowers:executing-plans. Checkbox steps, TDD.

**Goal:** `.columns`/`.column` alias to the `layout-ncol` grid (silent — it just works); a
misspelled feature/theorem `:::` class draws a located "did you mean" (near-miss only; custom
classes stay silent).

**Files:** `crates/core/src/render/validate.rs` (const + validator), `crates/core/src/render/divs.rs`
(columns arm + wire the validator), `crates/core/src/render/mod.rs` (re-export if needed),
`crates/core/src/vocab.rs` (drift test), a corpus doc + `corpus/diagnostics/`.

## Global Constraints
- Near-miss only (`closest` ≤ 2); exact-known + far-custom stay silent (open vocabulary).
- Both parts are additive: Part A adds a grid arm (same HTML as `layout-ncol`); Part B only pushes a warning, never changes what renders.
- Every emitted div keeps `data-block-id`/`data-sourcepos`. `cargo fmt`-clean.

---

### Task 1 (Part B): `validate_div_class` + the const

- [ ] **Step 1 (failing test):** in `validate.rs` `#[cfg(test)]`, add:
```rust
#[test]
fn validate_div_class_suggests_near_miss_only() {
    let s = |c: &str| vec![c.to_string()];
    assert_eq!(
        validate_div_class(&s("fragmnet"), 3, None).unwrap().message,
        "unknown div class `fragmnet` (did you mean `fragment`?)"
    );
    assert!(validate_div_class(&s("theorm"), 3, None).is_some(), "theorem typo");
    assert!(validate_div_class(&s("aside"), 3, None).is_none(), "known class is silent");
    assert!(validate_div_class(&s("my-widget"), 3, None).is_none(), "far custom class is silent");
}
```
(Confirm the `Warning` field is `.message` — grep `struct Warning`; adjust if it's `.msg`.)
- [ ] **Step 2:** run → FAIL (fn missing). `cargo test -p taliesin-core validate_div_class`
- [ ] **Step 3:** implement in `validate.rs`:
```rust
/// Structural + deck feature classes a `:::` div can carry (the near-miss anchor for the
/// did-you-mean). Not a closed vocabulary — custom classes are legal — so this only drives
/// *suggestions*, never rejection. Keep in sync with the `.class` dispatch in `render/divs.rs`
/// (a `vocab.rs` test pins `div_classes()`'s names as a subset).
pub(crate) const DIV_FEATURE_CLASSES: &[&str] = &[
    "panel-tabset", "code-walkthrough", "scrolly", "magic-move", "step",
    "column-margin", "aside", "sidenote", "marginnote",
    "fragment", "incremental", "notes", "columns", "column",
];

/// A misspelled feature/theorem `:::` class (near-miss of a known name) → a located
/// "did you mean". Only near-misses warn; an exactly-known class or a genuine custom class
/// (far from every known name) stays silent, since div classes are an open vocabulary.
/// Purely diagnostic — the div still renders.
pub(crate) fn validate_div_class(
    classes: &[String],
    line: usize,
    file: Option<String>,
) -> Option<Warning> {
    let known: Vec<&'static str> = DIV_FEATURE_CLASSES
        .iter()
        .copied()
        .chain(THEOREM_KINDS.iter().copied())
        .collect();
    classes.iter().find_map(|c| {
        if known.contains(&c.as_str()) {
            return None;
        }
        crate::frontmatter::closest(c, &known).map(|s| {
            Warning::new(format!("unknown div class `{c}` (did you mean `{s}`?)"))
                .at(file.clone(), line as u32)
        })
    })
}
```
- [ ] **Step 4:** run → PASS.
- [ ] **Step 5 (drift test):** in `vocab.rs` `#[cfg(test)]`, add a test that every name in `div_classes()` is in `render::DIV_FEATURE_CLASSES` (re-export it from `render/mod.rs` first: add `DIV_FEATURE_CLASSES` to the `pub(crate) use validate::{…}` line). Run → PASS (adjust the const if a vocab name is missing).
- [ ] **Step 6:** commit `feat(render): validate_div_class did-you-mean for misspelled ::: classes (DX5)`.

---

### Task 2 (Part B wiring + diagnostics pin): fire it from `build_container`

- [ ] **Step 1:** in `divs.rs` generic `else` (the `class = attrs.classes.join(" ")` branch), before building the div, push the warning:
```rust
    } else {
        if let Some(w) = super::validate::validate_div_class(&attrs.classes, open_line, file.clone()) {
            warnings.push(w);
        }
        let mut class = attrs.classes.join(" ");
        // … unchanged …
```
- [ ] **Step 2:** add a misspelled class to `corpus/diagnostics/` (e.g. append a `::: {.fragmnet}` block to an existing diagnostics doc, or a small new one) and pin the warning in the diagnostics test that renders it (mirror how `validate_callout_kind` / typos.tmd warnings are asserted — grep `corpus/diagnostics` in `crates/core/tests/`). Run that test → PASS.
- [ ] **Step 3:** `cargo test -p taliesin-core` (whole corpus still clean; the diagnostics doc is exempt). Mutation-check: revert the `validate_div_class` call → the pin fails → restore.
- [ ] **Step 4:** commit `feat(render): warn on misspelled ::: class in the div fall-through (DX5)`.

---

### Task 3 (Part A): `.columns` → `layout-ncol` grid

- [ ] **Step 1 (failing test):** in `divs.rs` `#[cfg(test)]`, add a render test: build a `.columns` div with two `.column` children (use the crate's div-grouping entry, mirroring existing divs.rs render tests), assert the output contains `class="tali-layout"` and `repeat(2,minmax(0,1fr))`. (Find the existing render helper in divs.rs tests; if none, drive `render_internal`/`group_divs` as the other tests do.)
- [ ] **Step 2:** run → FAIL (currently a plain `<div class="columns">`).
- [ ] **Step 3:** add the arm in `build_container`, right after the `layout-ncol` arm:
```rust
    } else if attrs.classes.iter().any(|c| c == "columns")
        && attrs.get("layout-ncol").is_none()
    {
        // Reveal muscle-memory: `::: {.columns}` with `.column` children. Alias it to the
        // native layout grid so it lays out side-by-side instead of silently stacking (DX5).
        let cols = inner
            .iter()
            .filter(|b| b.html.trim_start().starts_with("<div class=\"column\""))
            .count();
        let ncol = if cols >= 1 { cols } else { 2 };
        let body = concat(&inner);
        format!(
            "<div class=\"tali-layout\" style=\"display:grid;grid-template-columns:repeat({ncol},minmax(0,1fr));gap:1rem\"{data}>{body}</div>"
        )
```
- [ ] **Step 4:** run → PASS.
- [ ] **Step 5 (capability corpus pin):** add a `::: {.columns}` example with two `.column` children to a general (non-deck) corpus doc that already uses divs (decide in planning — e.g. `corpus/reader/` or a layout doc), so `cargo test -p taliesin-core` renders + lints it. Confirm the whole corpus stays green.
- [ ] **Step 6:** commit `feat(render): alias ::: {.columns} to the layout-ncol grid (DX5)`.

---

### Task 4: full gate
- [ ] `cargo test -p taliesin-core -p taliesin-server`, `cargo fmt --check`, `cargo clippy -p taliesin-core --all-targets -- -D warnings` — all green.

## Self-Review
- Spec coverage: Part A → T3; Part B validator → T1, wiring+pin → T2; drift test → T1.S5.
- Types: `validate_div_class(classes:&[String], line:usize, file:Option<String>) -> Option<Warning>` consistent T1↔T2; `DIV_FEATURE_CLASSES` re-export for the drift test + validator. Verify `Warning`'s message field name before asserting on it.
