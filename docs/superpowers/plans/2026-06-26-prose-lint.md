# Prose-lint diagnostics — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An opt-in, markdown-aware prose linter (doubled words, weasel words, custom banned terms) that emits located click-to-source warnings into the existing diagnostics channel.

**Architecture:** A new pure `crates/core/src/prose.rs` module (`config()` parses the `prose-lint` front-matter key; `lint()` scans the markdown-aware source → `(line, message)` pairs). Wired into `render_internal_impl` only when opted in, mapping lines through `map_origin` into `Warning`s. `prose-lint` is added to the front-matter known-key sets. Fully Rust-testable; no client/JS/browser.

**Tech Stack:** Rust (edition 2024), `serde_yaml` (already a dep) for config, hand-rolled tokenization (no `regex` dep). Tests: Rust unit + integration + corpus.

## Global Constraints

- **Opt-in only:** off unless front-matter `prose-lint: true` / `prose-lint: { banned: [...] }`.
- **Diagnostic-only / read-only:** never mutates blocks, ids, sourcepos, or rendered HTML.
- **Markdown-aware:** never flag text inside fenced code, inline code, `$…$`/`$$` math, link/image targets `](…)`, autolinks/HTML `<…>`, or `:::` fence lines.
- **Three rules only** (doubled words, weasel words, banned terms); **no passive voice**.
- **Offline:** the weasel list is an inline constant; no download/dep beyond `serde_yaml`.
- **Located:** each finding maps through `map_origin(origins, line)` → a `Warning` with file+line, the same channel as broken xrefs / unknown shortcodes.
- **Rides the diagnostics channel + front-matter known-key machinery.** Do-NOT-touch machinery untouched.
- **Message formats (exact):** `repeated word \`X\`` · `weasel word \`X\` (consider cutting)` · `banned term \`X\``.
- **Weasel list (exact):** `very, really, quite, just, actually, basically, simply, clearly, obviously, essentially, fairly, somewhat, rather`.

---

### Task 1: `prose.rs` — `config()` + `lint()` + unit tests

**Files:**
- Create: `crates/core/src/prose.rs`
- Modify: `crates/core/src/lib.rs` (add `pub(crate) mod prose;`)

**Interfaces:**
- Produces: `pub(crate) struct ProseLint { pub banned: Vec<String> }`;
  `pub(crate) fn config(front_matter: &str) -> Option<ProseLint>`;
  `pub(crate) fn lint(src: &str, cfg: &ProseLint) -> Vec<(usize, String)>`.

- [ ] **Step 1: Declare the module**

In `crates/core/src/lib.rs`, after `pub mod math;` (line ~34), add:

```rust
pub(crate) mod prose;
```

- [ ] **Step 2: Write `prose.rs` with the implementation + failing-then-passing unit tests**

Create `crates/core/src/prose.rs`:

```rust
//! Opt-in, markdown-aware prose linter. Diagnostic-only: [`lint`] returns `(line, message)`
//! pairs that `render` maps into located, click-to-source warnings (the same channel as
//! broken xrefs / unknown shortcodes). Three high-precision rules — doubled words, weasel
//! words, a custom banned-terms list — skipping code, math, links, and HTML so only prose is
//! checked. Off unless the doc opts in via the `prose-lint` front-matter key ([`config`]).

use serde_yaml::Value;

/// Resolved `prose-lint` configuration (the linter is off when [`config`] returns `None`).
pub(crate) struct ProseLint {
    pub banned: Vec<String>,
}

/// Conservative, well-known hedges. Whole-word, case-insensitive.
const WEASEL_WORDS: &[&str] = &[
    "very", "really", "quite", "just", "actually", "basically", "simply", "clearly",
    "obviously", "essentially", "fairly", "somewhat", "rather",
];

/// Parse the `prose-lint` front-matter key. `None` = linter off. `true` enables the built-in
/// rules; a mapping additionally reads a `banned` string list.
pub(crate) fn config(front_matter: &str) -> Option<ProseLint> {
    let value: Value = serde_yaml::from_str(front_matter).ok()?;
    let pl = value.get("prose-lint")?;
    match pl {
        Value::Bool(true) => Some(ProseLint { banned: Vec::new() }),
        Value::Mapping(_) => {
            let banned = pl
                .get("banned")
                .and_then(Value::as_sequence)
                .map(|seq| {
                    seq.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            Some(ProseLint { banned })
        }
        _ => None, // false / null / scalar -> off
    }
}

/// Scan markdown `src` for prose-rule violations. Returns `(1-based line, message)`.
pub(crate) fn lint(src: &str, cfg: &ProseLint) -> Vec<(usize, String)> {
    let banned: Vec<String> = cfg.banned.iter().map(|b| b.to_lowercase()).collect();
    let mut out = Vec::new();
    let mut in_front = false;
    let mut fence: Option<char> = None; // inside a ``` or ~~~ code block
    for (i, raw) in src.lines().enumerate() {
        let line_no = i + 1;
        let t = raw.trim_start();
        // Front matter: a leading `---` (line 1 only) opens; the next `---`/`...` closes.
        if i == 0 && t == "---" {
            in_front = true;
            continue;
        }
        if in_front {
            if t == "---" || t == "..." {
                in_front = false;
            }
            continue;
        }
        // Fenced code blocks: skip the fence lines and everything between.
        if let Some(f) = fence {
            if (f == '`' && t.starts_with("```")) || (f == '~' && t.starts_with("~~~")) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") {
            fence = Some('`');
            continue;
        }
        if t.starts_with("~~~") {
            fence = Some('~');
            continue;
        }
        // `:::` div fence lines carry attributes, not prose.
        if t.starts_with(":::") {
            continue;
        }
        let text = strip_inline(raw);
        scan_line(&text, line_no, &banned, &mut out);
    }
    out
}

/// Blank out inline code, math, link/image targets, autolinks, and HTML tags (replaced with
/// spaces, so word boundaries survive) leaving only prose text. Line numbers are all we need,
/// so per-byte space padding is fine.
fn strip_inline(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let blank = |out: &mut String, n: usize| {
        for _ in 0..n {
            out.push(' ');
        }
    };
    while i < line.len() {
        if bytes[i] == b'`' {
            let run = line[i..].bytes().take_while(|&b| b == b'`').count();
            let ticks = &line[i..i + run];
            if let Some(rel) = line[i + run..].find(ticks) {
                let close = i + run + rel + run;
                blank(&mut out, close - i);
                i = close;
            } else {
                blank(&mut out, run);
                i += run;
            }
        } else if bytes[i] == b'$' {
            let marker = if line[i..].starts_with("$$") { "$$" } else { "$" };
            let start = i + marker.len();
            if let Some(rel) = line[start..].find(marker) {
                let close = start + rel + marker.len();
                blank(&mut out, close - i);
                i = close;
            } else {
                out.push('$');
                i += 1;
            }
        } else if line[i..].starts_with("](") {
            if let Some(rel) = line[i + 2..].find(')') {
                let close = i + 2 + rel + 1;
                blank(&mut out, close - i);
                i = close;
            } else {
                out.push_str("](");
                i += 2;
            }
        } else if bytes[i] == b'<' {
            if let Some(rel) = line[i..].find('>') {
                let close = i + rel + 1;
                blank(&mut out, close - i);
                i = close;
            } else {
                out.push('<');
                i += 1;
            }
        } else {
            let ch = line[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Maximal runs of alphanumeric + apostrophe, as the prose "words".
fn words(text: &str) -> Vec<String> {
    let mut ws = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '\'' {
            cur.push(ch);
        } else if !cur.is_empty() {
            ws.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        ws.push(cur);
    }
    ws
}

fn scan_line(text: &str, line_no: usize, banned: &[String], out: &mut Vec<(usize, String)>) {
    let ws = words(text);
    let mut prev: Option<String> = None;
    for w in &ws {
        let lw = w.to_lowercase();
        let is_alpha = lw.chars().next().is_some_and(|c| c.is_alphabetic());
        if is_alpha && prev.as_deref() == Some(lw.as_str()) {
            out.push((line_no, format!("repeated word `{lw}`")));
        }
        if WEASEL_WORDS.contains(&lw.as_str()) {
            out.push((line_no, format!("weasel word `{lw}` (consider cutting)")));
        }
        if banned.contains(&lw) {
            out.push((line_no, format!("banned term `{lw}`")));
        }
        prev = Some(lw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(banned: &[&str]) -> ProseLint {
        ProseLint {
            banned: banned.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn config_off_unless_opted_in() {
        assert!(config("title: T").is_none());
        assert!(config("prose-lint: false").is_none());
        assert!(config("prose-lint: true").is_some());
    }

    #[test]
    fn config_reads_banned_list() {
        let c = config("prose-lint:\n  banned: [utilize, leverage]").expect("on");
        assert_eq!(c.banned, vec!["utilize", "leverage"]);
    }

    #[test]
    fn flags_doubled_words() {
        let w = lint("We we should fix it.", &cfg(&[]));
        assert_eq!(w, vec![(1, "repeated word `we`".to_string())]);
    }

    #[test]
    fn flags_weasel_words() {
        let w = lint("This is very fast and really clever.", &cfg(&[]));
        assert!(w.contains(&(1, "weasel word `very` (consider cutting)".to_string())));
        assert!(w.contains(&(1, "weasel word `really` (consider cutting)".to_string())));
    }

    #[test]
    fn flags_banned_terms_case_insensitively() {
        let w = lint("Please Utilize the API.", &cfg(&["utilize"]));
        assert_eq!(w, vec![(1, "banned term `utilize`".to_string())]);
    }

    #[test]
    fn skips_code_math_links_and_fences() {
        let src = "`utilize` and $very$ and [very](http://very.x) stay clean.\n\n```\nutilize very very\n```\n";
        let w = lint(src, &cfg(&["utilize"]));
        assert!(w.is_empty(), "markdown spans + fences must be skipped, got: {w:?}");
    }

    #[test]
    fn reports_correct_line_numbers() {
        let src = "Clean line.\nAnother clean one.\nPlease utilize this.";
        let w = lint(src, &cfg(&["utilize"]));
        assert_eq!(w, vec![(3, "banned term `utilize`".to_string())]);
    }
}
```

- [ ] **Step 3: Run the unit tests**

Run: `cargo test -p qmd-fast-core --lib prose 2>&1 | tail -15`
Expected: PASS (all 7).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/prose.rs crates/core/src/lib.rs
git commit -m "feat(diagnostics): prose-lint scanner (doubled/weasel/banned, markdown-aware)"
```

---

### Task 2: Front-matter known-key recognition for `prose-lint`

**Files:**
- Modify: `crates/core/src/frontmatter.rs` (add to `KNOWN_KEYS`, add `PROSE_LINT_KEYS`, wire `validate_nested`, add a test)

**Interfaces:**
- Consumes: existing `KNOWN_KEYS`, `validate_nested`, `validate_front_matter`.

- [ ] **Step 1: Write the failing test**

In `crates/core/src/frontmatter.rs`, inside `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn prose_lint_key_is_recognized_and_nested_validated() {
        // top-level `prose-lint` is known
        assert!(
            validate_front_matter("---\ntitle: T\nprose-lint: true\n---\n").is_empty(),
            "prose-lint should be a known top-level key"
        );
        // a typo'd nested key is flagged with did-you-mean
        let w = validate_front_matter("---\ntitle: T\nprose-lint:\n  bnned: [x]\n---\n");
        assert!(
            w.iter().any(|x| x.message.contains("bnned") && x.message.contains("banned")),
            "nested prose-lint typo should be flagged, got: {w:?}"
        );
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p qmd-fast-core --lib frontmatter::tests::prose_lint 2>&1 | tail -15`
Expected: FAIL — `prose-lint` flagged as unknown (the first assert fails).

- [ ] **Step 3: Add the key + nested validation**

In `crates/core/src/frontmatter.rs`, add `"prose-lint",` to the `KNOWN_KEYS` array (near `"toc",`). After the `HERO_KEYS` const (~line 71), add:

```rust
pub(crate) const PROSE_LINT_KEYS: &[&str] = &["banned"];
```

In `validate_front_matter`, next to the other `validate_nested(...)` calls (for `execute`/`listing`/`about`/`hero`), add:

```rust
    validate_nested(&map, "prose-lint", "prose-lint option", PROSE_LINT_KEYS, block, &mut out);
```

(Match the exact parameter names/order used by the neighboring `validate_nested` calls — read them first.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p qmd-fast-core --lib frontmatter 2>&1 | tail -8`
Expected: PASS (the new test + existing front-matter tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/frontmatter.rs
git commit -m "feat(diagnostics): recognize + nested-validate the prose-lint front-matter key"
```

---

### Task 3: Wire prose-lint into the render pipeline

**Files:**
- Modify: `crates/core/src/render/mod.rs` (`render_internal_impl`)
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `crate::prose::{config, lint}` (Task 1); `crate::frontmatter::front_matter_block`; `map_origin(origins, line) -> (Option<String>, usize)`; `Warning::new(..).at(file, line_u32)`.

- [ ] **Step 1: Write the failing integration test**

In `crates/core/src/render/tests.rs`, append:

```rust
#[test]
fn prose_lint_emits_located_warnings_when_opted_in() {
    let doc = render_document(
        "---\ntitle: T\nprose-lint: true\n---\n\nThis is very very good.\n",
    );
    // "very very" -> a doubled word AND two weasel-word hits, all on line 6.
    let msgs: Vec<_> = doc.warnings.iter().map(|w| w.message.as_str()).collect();
    assert!(
        msgs.contains(&"repeated word `very`"),
        "expected doubled-word warning, got: {msgs:?}"
    );
    assert!(
        doc.warnings.iter().any(|w| w.message.contains("weasel word `very`") && w.line == Some(6)),
        "weasel warning should be located on line 6, got: {:?}",
        doc.warnings
    );
}

#[test]
fn prose_lint_is_silent_when_not_opted_in() {
    let doc = render_document("# T\n\nThis is very very good.\n");
    assert!(
        !doc.warnings.iter().any(|w| w.message.contains("weasel") || w.message.contains("repeated word")),
        "prose-lint must be off without opt-in, got: {:?}",
        doc.warnings
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p qmd-fast-core --lib render::tests::prose_lint 2>&1 | tail -15`
Expected: FAIL — no prose warnings emitted yet.

- [ ] **Step 3: Wire it in**

In `crates/core/src/render/mod.rs`, in `render_internal_impl`, immediately after the existing `warnings.extend(crate::frontmatter::validate_front_matter(src));` line, add:

```rust
    // Opt-in prose lint (front-matter `prose-lint:`): markdown-aware, diagnostic-only,
    // located via map_origin like every other warning.
    if let Some(cfg) = crate::prose::config(crate::frontmatter::front_matter_block(src).unwrap_or(""))
    {
        for (line, msg) in crate::prose::lint(src, &cfg) {
            let (file, mapped) = map_origin(origins, line);
            warnings.push(Warning::new(msg).at(file, mapped as u32));
        }
    }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p qmd-fast-core --lib render::tests::prose_lint 2>&1 | tail -10`
Expected: PASS (both).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/render/mod.rs crates/core/src/render/tests.rs
git commit -m "feat(diagnostics): run opt-in prose-lint in the render pipeline (located)"
```

---

### Task 4: Corpus pin doc + exact-warning integration test

**Files:**
- Create: `corpus/diagnostics/prose.qmd`
- Create: `crates/core/tests/prose_lint.rs`

**Interfaces:**
- Consumes: `qmd_fast_core::render_document_with_includes(&str, &Path).warnings`.

- [ ] **Step 1: Write the pin doc**

Create `corpus/diagnostics/prose.qmd`:

````markdown
---
title: "Prose lint"
prose-lint:
  banned: [utilize]
---

We we should fix this doubled word.

This is very fast and really clever.

Please utilize the new API.

`utilize` in code and $very$ in math and [very](https://very.example) stay clean,
and so does this fenced block:

```python
# utilize very very
x = 1
```
````

- [ ] **Step 2: Write the integration test**

Create `crates/core/tests/prose_lint.rs`:

```rust
//! The prose-lint pin doc trips each rule (doubled / weasel / banned) and proves
//! markdown-awareness: code, math, links, and fenced blocks are NOT flagged. Mirrors
//! `nested_validation.rs` (asserts the exact located warning set).

mod common;
use common::corpus_dir;
use std::fs;

fn warnings() -> Vec<qmd_fast_core::render::Warning> {
    let path = corpus_dir().join("diagnostics/prose.qmd");
    let src = fs::read_to_string(&path).unwrap();
    qmd_fast_core::render_document_with_includes(&src, path.parent().unwrap()).warnings
}

#[test]
fn prose_lint_pin_doc_trips_each_rule_and_skips_markdown() {
    let ws = warnings();
    let has = |needle: &str| ws.iter().any(|w| w.message == needle);
    assert!(has("repeated word `we`"), "doubled word: {ws:?}");
    assert!(has("weasel word `very` (consider cutting)"), "weasel: {ws:?}");
    assert!(has("weasel word `really` (consider cutting)"), "weasel: {ws:?}");
    assert!(has("banned term `utilize`"), "banned: {ws:?}");
    // markdown-awareness: the `utilize`/`very` inside code/math/link/fence must NOT warn.
    // The only `utilize` warning is the prose one on the "Please utilize" line.
    let utilize_hits = ws.iter().filter(|w| w.message == "banned term `utilize`").count();
    assert_eq!(utilize_hits, 1, "only the prose `utilize` should warn: {ws:?}");
    // every prose warning is located
    assert!(
        ws.iter()
            .filter(|w| w.message.contains("word") || w.message.contains("term"))
            .all(|w| w.line.is_some()),
        "prose warnings must carry a line: {ws:?}"
    );
}
```

(`Warning` must be reachable as `qmd_fast_core::render::Warning` — it is re-exported there; if the path differs, adjust to the actual public path, e.g. `qmd_fast_core::render::model::Warning`.)

- [ ] **Step 3: Run the test (red→green) + corpus invariants**

Run: `cargo test -p qmd-fast-core --test prose_lint 2>&1 | tail -15 && cargo test -p qmd-fast-core --test corpus 2>&1 | tail -4`
Expected: both PASS. (If `Warning`'s public path is wrong, fix the import per the compiler error, then re-run.)

- [ ] **Step 4: Commit**

```bash
git add corpus/diagnostics/prose.qmd crates/core/tests/prose_lint.rs
git commit -m "test(diagnostics): pin prose-lint corpus doc + exact located-warning test"
```

---

### Task 5: Docs + full verification

**Files:**
- Modify: `corpus/README.md` (note the new diagnostics doc)
- Modify: `notes/backlog.md` + `notes/FEATURE-IDEAS.md` (mark #29 shipped)
- Consider: `docs/guide/reference/configuration.qmd` (document `prose-lint:`) if a natural spot exists

- [ ] **Step 1: Corpus README**

In `corpus/README.md`, update the `diagnostics/` description (or add a sentence) to note `prose.qmd` exercises the opt-in prose linter (doubled/weasel/banned, markdown-aware), pinned by `crates/core/tests/prose_lint.rs`.

- [ ] **Step 2: Document the front-matter key**

If `docs/guide/reference/configuration.qmd` lists front-matter keys, add a short `prose-lint: true | { banned: [...] }` entry describing the three rules, opt-in, and click-to-source. If there's no natural list, skip and note it in the backlog instead. (No em dashes.)

- [ ] **Step 3: Notes**

In `notes/backlog.md`, add a one-line shipped note (Pillar I prose-lint: opt-in `prose-lint:` front-matter; doubled/weasel/banned; markdown-aware; located click-to-source via the diagnostics channel; pinned `corpus/diagnostics/prose.qmd`; passive-voice deferred). In `notes/FEATURE-IDEAS.md`, mark idea #29 ✅ SHIPPED 2026-06-26.

- [ ] **Step 4: Full verification**

Run:
```bash
cargo test -p qmd-fast-core
cargo fmt --check
```
Expected: all tests pass (0 failed across binaries), fmt clean.

- [ ] **Step 5: Commit**

```bash
git add corpus/README.md notes/backlog.md notes/FEATURE-IDEAS.md docs/guide
git commit -m "docs(diagnostics): record prose-lint; mark idea #29 shipped"
```

---

## Self-Review

**Spec coverage:**
- `config()` (bool / map / off) → Task 1. ✓
- `lint()` 3 rules + markdown-aware stripping + fence/`:::` skip + front-matter skip → Task 1. ✓
- Weasel list + exact messages → Task 1 (Global Constraints copy). ✓
- `prose-lint` known key + `PROSE_LINT_KEYS` nested validation → Task 2. ✓
- Wiring after `validate_front_matter`, `map_origin` located, opt-in gate → Task 3. ✓
- Corpus pin + exact located-warning test + markdown-awareness assertion → Task 4. ✓
- Corpus invariants (diagnostics/ exempt) → Task 4 Step 3. ✓
- Docs → Task 5. ✓
- Invariants (diagnostic-only, offline, rides channel) → Global Constraints + design. ✓

**Placeholder scan:** no TBD/TODO; all code complete. Two compiler-guided fallbacks are flagged explicitly (the `Warning` public path in Task 4; matching `validate_nested` arg order in Task 2) — both resolve against the real signatures at implementation time. ✓

**Type/name consistency:** `ProseLint { banned: Vec<String> }`, `config(&str) -> Option<ProseLint>`, `lint(&str, &ProseLint) -> Vec<(usize, String)>` defined in Task 1, consumed in Task 3. `PROSE_LINT_KEYS` defined + used in Task 2. Message strings identical across Task 1 (emits), Task 1 tests, Task 3 test, Task 4 test. `map_origin` returns `(Option<String>, usize)` → cast `as u32` for `.at`. ✓
```
