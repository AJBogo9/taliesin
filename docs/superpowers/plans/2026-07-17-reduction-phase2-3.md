# Reduction Phase 2 + T1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the confirmed dead code (`about:` block, `TAL-MEDIA` audio row, `search_button` full variant), consolidate `llms.rs` text extraction onto `render::indexable_text` *if* proven output-equivalent, and de-churn `PageParts` so a new field is a one-site edit.

**Architecture:** Five independent changes, each its own commit, ordered smallest-risk-first (D2, D3, D1, R1, T1). Each is guarded by the 71-doc corpus suite plus targeted unit tests. Deletions assert the *new* behavior (e.g. a retired key now warns) before landing. R1 is gated on an equivalence test and may correctly end in "do not consolidate." T1 is a behavior-preserving refactor guarded by existing page-assembly snapshots.

**Tech Stack:** Rust (taliesin-core, taliesin-server), edition 2024. Tests: `cargo test -p taliesin-core` and `-p taliesin-server`. Bundled CSS is `include_str!`-compiled, so a CSS edit needs `cargo build` before a *build*-path test sees it (unit tests that assert on rendered HTML rebuild automatically via cargo).

## Global Constraints

- **Invariants untouched:** every emitted block keeps `data-block-id` + `data-sourcepos` (+ `data-source-file` on includes). None of these changes touch the block model, numbering, includes, cite, exec/freeze/kernel, or warm-page eviction.
- **Do not touch the deck** (`render/deck.rs`, deck.css/js): mid-redesign, out of scope.
- **`cargo fmt` clean** after every task (a PostToolUse hook runs rustfmt; CI enforces it).
- **Leave the stray `.claude/worktrees/agent-*`** worktrees alone (possibly live parallel sessions).
- **One commit per task**, message prefix per the repo convention (`refactor(...)`, `fix(...)`, `chore(...)`).
- **Branch:** `reduction-modularity-pass` (already checked out; the spec/plan/map commits are on it).

---

### Task D2: delete the dead `TAL-MEDIA` audio catalog row

**Files:**
- Modify: `crates/core/src/diagnostics/codes.rs:65`

**Interfaces:**
- Consumes: nothing. Produces: nothing (pure dead-row removal).

Context: `classify()` maps a diagnostic message substring to a code. The row `("local audio not found", "TAL-MEDIA", ERROR)` at line 65 has **no producer** anywhere in the repo (verified: the string exists only at this line; `diagnostics/media.rs` deliberately never validates `<audio>`, guarded by `audio_source_is_not_a_video_false_positive`). The sibling video row at line 64 stays.

- [ ] **Step 1: Confirm no producer, then delete the row.** Re-verify, then remove line 65.

Run first: `rg -n "local audio not found" crates/` — expected: only `codes.rs:65`.

Delete exactly this line from `crates/core/src/diagnostics/codes.rs` (keep line 64, the video row):

```rust
    ("local audio not found", "TAL-MEDIA", ERROR),
```

- [ ] **Step 2: Verify the guard test still passes**

Run: `cargo test -p taliesin-core diagnostics -- audio_source_is_not_a_video_false_positive`
Expected: PASS (audio is still, correctly, never validated).

- [ ] **Step 3: Full diagnostics suite green**

Run: `cargo test -p taliesin-core diagnostics`
Expected: PASS. (No test asserted the dead row existed, because nothing ever produced its message.)

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/diagnostics/codes.rs
git commit -m "chore(diagnostics): drop the orphaned TAL-MEDIA audio catalog row"
```

---

### Task D3: remove the dead `search_button(full=true)` variant

**Files:**
- Modify: `crates/core/src/site/chrome.rs:30-53` (fn + doc-comment), `:110`, `:262` (call sites), `:473-492` (tests)
- Modify: `crates/core/assets/css/site.css` (sweep `.tali-search-full` / `.tali-search-label` if present)

**Interfaces:**
- Consumes: nothing. Produces: `search_button()` (no argument) for the two chrome call sites.

Context: `search_button(full: bool)` is called with `false` at both production sites (navbar `:110`, sidebar `:262`); only a unit test passes `true`. The `full` branch emits `tali-search-full` + a `tali-search-label` span that no production page renders.

- [ ] **Step 1: Delete the full-variant test (it tests dead code)**

Remove this test from the `#[cfg(test)]` module in `crates/core/src/site/chrome.rs` (around :488-492):

```rust
    fn full_search_button_name_matches_its_visible_label() {
        let b = search_button(true);
        // …assertions on the "Search the book" label…
    }
```

- [ ] **Step 2: Update the surviving test to the new signature**

In `search_button_hides_the_shortcut_hint_from_its_name` (around :473), change `search_button(false)` to `search_button()`.

- [ ] **Step 3: Run the test to verify it FAILS to compile**

Run: `cargo test -p taliesin-core site::chrome`
Expected: FAIL (compile error: `search_button` still takes a `bool`).

- [ ] **Step 4: Collapse the function to the compact variant**

Replace `search_button` (`crates/core/src/site/chrome.rs:34-53`) with the no-arg form (drop the `full` param, the `if full` branch, and the stale doc sentence "`full` widens it with a label for the sidebar"):

```rust
fn search_button() -> String {
    // The kbd is a shortcut hint, not part of the label: aria-hidden keeps it out of the
    // accessible name (WCAG 2.5.3 Label-in-Name).
    format!(
        "<button class='tali-search-btn' type='button' data-qmd-search aria-label='Search' \
         aria-keyshortcuts='Control+K Meta+K'>{SEARCH_ICON}\
         <kbd class='tali-search-kbd' aria-hidden='true'>\u{2318}K</kbd></button>"
    )
}
```

- [ ] **Step 5: Update the two call sites**

`crates/core/src/site/chrome.rs:110` and `:262`: change `search_button(false)` to `search_button()`.

- [ ] **Step 6: Sweep the now-dead CSS**

Run: `rg -n "tali-search-full|tali-search-label" crates/core/assets/css/`
If present, delete those rules from `site.css` (they only styled the removed `full` variant). Re-grep to confirm zero matches remain in `crates/core/src` and `crates/core/assets`.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p taliesin-core site`
Expected: PASS. Also run the chrome/navbar/book-sidebar rendering tests to confirm the compact button still renders in both.

- [ ] **Step 8: Commit**

```bash
git add crates/core/src/site/chrome.rs crates/core/assets/css/site.css
git commit -m "refactor(chrome): drop the unused full-width search-button variant"
```

---

### Task D1: delete the `about:` front-matter block (superseded by `hero:`)

**Files (all sites, verified):**
- Modify: `crates/core/src/frontmatter.rs` — `KNOWN_KEYS` entry `"about"` (:55), the `validate_nested(map, "about", "about key", ABOUT_KEYS, …)` call (:148), the `ABOUT_KEYS` const (its only reference is :148), the doc comments naming `about:` (:7, :17, :85, :114), and the two tests using `about:` (:730-733 and the combined fixture at :745)
- Modify: `crates/core/src/diagnostics/codes.rs:31` — the `("unknown about key", "TAL-FM-KEY", WARNING)` row (dead once `ABOUT_KEYS` validation is gone)
- Modify: `crates/core/src/site/frontmatter.rs` — `FrontInfo.about` field (:19), the `about: parse_about(...)` initializer (:60), the `parse_about` fn (:97-120)
- Modify: `crates/core/src/site/mod.rs` — `Page.about` field (:68-69), the `AboutSpec` struct (:84-93), the `else if let Some(about)` branch in `expand_page` (:1162-1163), the `about_html` fn (:1447-1489), and doc comments naming `about:` (:633, :811, :1153, :1447, :1507)
- Modify: `crates/core/src/site/discovery.rs:61` — `about: fm.about,` (Page construction)
- Modify: `crates/core/src/site/links.rs:426` — `about: None,` (synthetic Page)
- Modify: `crates/core/assets/css/site.css:171-182` — the `.tali-about*` rule block

**Interfaces:**
- Consumes: nothing. Produces: `about:` becomes an unrecognized key that warns (the `image:`/`csl` retirement precedent — subtract the key so a stale author config gets a diagnostic, not a silent no-op). **Keep `NavItem`** (shared with nav) and **keep `hero:`** (the replacement).

- [ ] **Step 1: Write the failing test — `about:` now warns as unknown**

Add to the front-matter validation tests in `crates/core/src/frontmatter.rs` (mirror the existing `msgs(...)` style at :730):

```rust
    #[test]
    fn retired_about_key_warns_as_unknown() {
        let a = msgs("---\ntitle: X\nabout:\n  template: jolla\n---\n");
        assert!(
            a.iter().any(|m| m.contains("unknown front-matter key") && m.contains("about")),
            "a stale `about:` should warn now that the feature is gone, got {a:?}"
        );
    }
```

- [ ] **Step 2: Run it — expect FAIL**

Run: `cargo test -p taliesin-core frontmatter -- retired_about_key_warns_as_unknown`
Expected: FAIL (`about` is still in `KNOWN_KEYS`, so no warning).

- [ ] **Step 3: Remove `about` from the accepted keyspace + its nested validation**

In `crates/core/src/frontmatter.rs`: delete the `"about",` entry from `KNOWN_KEYS` (:55); delete the `validate_nested(map, "about", "about key", ABOUT_KEYS, block, &mut out);` line (:148); delete the `ABOUT_KEYS` const; update the doc comments at :7, :17, :114 to drop `about:`, and delete the `about:` sub-keys doc comment at :85. Delete the pre-existing `about:` tests (the `unknown about key imagee` test at :730-733) and remove the `about:` block from the combined fixture at :745 (keep the rest of that fixture).

- [ ] **Step 4: Remove the dead diagnostic row**

In `crates/core/src/diagnostics/codes.rs`, delete line 31: `("unknown about key", "TAL-FM-KEY", WARNING),`.

- [ ] **Step 5: Remove the parse + model + render sites**

- `crates/core/src/site/frontmatter.rs`: delete the `about: Option<AboutSpec>` field (:19), the `about: parse_about(val.get("about")),` initializer (:60), and the whole `parse_about` fn (:97-120).
- `crates/core/src/site/mod.rs`: delete the `Page.about` field + its doc (:68-69), the `AboutSpec` struct + its doc (:84-93), the `about_html` fn + its doc (:1447-1489), and the `else if let Some(about) = &page.about { set_title_block(blocks, self.about_html(page, about)); }` branch in `expand_page` (:1162-1163). Update the doc comments at :633, :811, :1153, :1507 to drop the `about:` mention (keep the `listing:`/`hero:` mentions).
- `crates/core/src/site/discovery.rs:61`: delete `about: fm.about,`.
- `crates/core/src/site/links.rs:426`: delete `about: None,`.

- [ ] **Step 6: Remove the dead CSS**

Delete the `.tali-about` / `.tali-about-img` / `.tali-about-name` / `.tali-about-links` / `.tali-about-link` rules and their `/* about: … */` comment from `crates/core/assets/css/site.css:171-182`.

- [ ] **Step 7: Confirm nothing references the removed symbols**

Run: `rg -n "AboutSpec|about_html|parse_about|tali-about" crates/`
Expected: **zero** matches.

- [ ] **Step 8: Run tests — expect PASS**

Run: `cargo test -p taliesin-core` (the new warn-test passes; the corpus suite stays green because no corpus doc uses `about:`).
Expected: PASS. If a `tech_blog.rs` test pinned the old `.tali-about*` markup absence, it stays green.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "refactor(site): remove the retired about: block (superseded by hero:)"
```

---

### Task R1: reuse `render::indexable_text` in `llms.rs` — GATED on equivalence

**Files:**
- Test: `crates/core/src/site/llms.rs` (`#[cfg(test)]` mod at :251)
- Modify (only if the gate passes): `crates/core/src/site/llms.rs:169-190` (`page_prose`), delete `text_content` (:221-249)
- Consumes: `render::indexable_text` (`crates/core/src/render/text.rs:121`, `pub(crate)`)

**Interfaces:**
- `render::indexable_text(html: &str) -> String` — strip tags (space-separated), decode entities, collapse whitespace. Keeps math text.
- `llms::text_content(html: &str) -> String` — same shape, but a **different** entity-decode set (it explicitly decodes `&#8217;`, `&nbsp;`, `&#39;`). `llms` also runs `strip_katex` first (v1 omits math), so after `strip_katex` there is no math left for either function to differ on.

Context: `search.rs` already reuses `render::indexable_text`; `llms.rs` hand-rolls `text_content`. Consolidating is only safe if `indexable_text(strip_katex(x))` equals `text_content(strip_katex(x))` on real inputs. The entity sets differ, so **this task may correctly conclude "do not consolidate."**

- [ ] **Step 1: Write the equivalence gate test**

Add to `crates/core/src/site/llms.rs` tests. Cover entities that `text_content` handles specially (`&#8217;` `&nbsp;` `&#39;` `&amp;`), inline math, and adjacent blocks:

```rust
    #[test]
    fn indexable_text_matches_text_content_after_strip_katex() {
        let cases = [
            r#"<h1>KL Divergence</h1><p>How &amp; why</p>"#,
            r#"<p>it&#8217;s fine&nbsp;here</p>"#,
            r#"<p>a</p><p>b</p>"#,
            r#"<p>x <span class="katex">…MathML…<annotation>\alpha</annotation></span> y</p>"#,
        ];
        for html in cases {
            let stripped = strip_katex(html);
            assert_eq!(
                crate::render::indexable_text(&stripped),
                text_content(&stripped),
                "divergence on: {html}"
            );
        }
    }
```

- [ ] **Step 2: Run the gate**

Run: `cargo test -p taliesin-core site::llms -- indexable_text_matches_text_content_after_strip_katex`

- [ ] **Step 3: Branch on the result**

**If PASS** (equivalent): proceed to Step 4.
**If FAIL** (they diverge): **stop here.** Do not consolidate. Keep the test as a `#[ignore]`d record of the divergence, and note in the commit + map that R1 is deferred because `indexable_text` and `text_content` decode a different entity set (deciding which is correct is a separate call, out of this pass's "present benefit, low risk" scope). Commit only the documenting test:

```bash
git add crates/core/src/site/llms.rs
git commit -m "test(llms): record that indexable_text and text_content diverge (R1 deferred)"
```
Then skip to Task T1.

- [ ] **Step 4 (only if gate passed): Point `page_prose` at `indexable_text`**

In `crates/core/src/site/llms.rs`, change the extraction line in `page_prose` (:184) from `let t = text_content(&html);` to `let t = render::indexable_text(&html);`, then delete the now-unused `text_content` fn (:221-249) and its own unit tests (`text_content_strips_tags_and_decodes_entities`, `text_content_separates_adjacent_blocks`). Keep `strip_katex`.

- [ ] **Step 5: Confirm `text_content` is gone and unreferenced**

Run: `rg -n "text_content" crates/core/src/site/llms.rs` — expected: zero matches.

- [ ] **Step 6: Run the llms + search suites**

Run: `cargo test -p taliesin-core site::llms site::search`
Expected: PASS (the `llms` generation test `seo_and_llm_artifacts_are_generated_for_the_blog` still produces the same prose).

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/site/llms.rs
git commit -m "refactor(llms): reuse render::indexable_text instead of a hand-rolled twin"
```

---

### Task T1: de-churn `PageParts` with a defaults base

**Files:**
- Modify: `crates/core/src/render/page.rs` — add `impl<'a> PageParts<'a>` with `defaults()`; convert the internal construction sites (:563, :667) to struct-update
- Modify: `crates/server/src/serve/mod.rs:692` and `crates/server/src/serve_site/mod.rs:668` — convert to struct-update
- (Test construction sites `page.rs:734,775` may stay as-is or adopt the base; behavior is snapshot-guarded either way)

**Interfaces:**
- Produces: `PageParts::defaults() -> PageParts<'static>` — every field at a safe default (`""` for the `&str` fields, `false` for the bools, `OutputMode::Build`, `AssetMode::Inline`). Every construction site sets the fields it cares about and ends with `..PageParts::defaults()`, so **a new `PageParts` field is a one-line edit in `defaults()` instead of an edit at every site.**

Context: `PageParts` is a 17-field struct (`render/page.rs:81`) hand-built at 4 production sites; a field addition must be mirrored at each, and drift here caused a real past title-consistency bug (`page.rs:380-390`). This is a behavior-preserving refactor guarded by the existing page-assembly snapshot tests.

- [ ] **Step 1: Establish the safety net (characterization)**

Run the existing page-assembly + body-snapshot suite and confirm green *before* touching anything:

Run: `cargo test -p taliesin-core -- page assemble body_html_snapshots`
Expected: PASS. These snapshots are the refactor's oracle — the assembled bytes must not change.

- [ ] **Step 2: Add the defaults constructor**

In `crates/core/src/render/page.rs`, after the `PageParts` struct (:120), add:

```rust
impl PageParts<'static> {
    /// Every field at a safe default, so a construction site sets only what it needs
    /// and ends with `..PageParts::defaults()`. Adding a new field updates this one
    /// place instead of every hand-rolled site.
    pub fn defaults() -> PageParts<'static> {
        PageParts {
            mode: OutputMode::Build,
            title: "",
            lang: "en",
            favicon: "",
            theme_default: "",
            theme_css: "",
            with_site_css: false,
            ship_katex: false,
            extra_head: "",
            body_class: "",
            include_in_header: "",
            include_before_body: "",
            body: "",
            scripts_pre: "",
            scripts_post: "",
            include_after_body: "",
            assets: AssetMode::Inline,
        }
    }
}
```

- [ ] **Step 3: Verify it compiles and the struct-update coerces**

Convert the internal site `page.rs:563` to end its literal with `..PageParts::defaults()` (dropping the fields whose value already equals the default, e.g. any `""`). Build:

Run: `cargo build -p taliesin-core`
Expected: compiles. **If the lifetime is rejected** (`PageParts<'static>` not coercing into `PageParts<'a>` in struct-update), change the signature to a generic free function form `pub fn defaults<'a>() -> PageParts<'a>` and retry — the body is identical (`&'static` literals satisfy any `'a`).

- [ ] **Step 4: Convert the remaining sites to struct-update**

Apply the same `..PageParts::defaults()` tail to `page.rs:667`, `crates/server/src/serve/mod.rs:692`, and `crates/server/src/serve_site/mod.rs:668`, removing only the fields that already match the default. Every site keeps every field it sets to a non-default value (e.g. the preview site keeps `mode: Preview`, `ship_katex: true`, `assets: Inline`, the favicon, etc.).

- [ ] **Step 5: Prove behavior is unchanged**

Run: `cargo test -p taliesin-core -- page assemble body_html_snapshots`
Run: `cargo test -p taliesin-server`
Expected: PASS with **no snapshot diffs**. A changed snapshot means a field's default did not match a site's old explicit value — fix the site (re-add the explicit field), do not accept the diff.

- [ ] **Step 6: Add the regression test that proves the churn is gone**

Add to `crates/core/src/render/tests.rs` (or the page tests module) a test asserting `defaults()` is usable and a minimal page assembles:

```rust
    #[test]
    fn page_parts_defaults_assemble_a_minimal_page() {
        let html = assemble_html_page(&PageParts { title: "T", body: "<p>x</p>", ..PageParts::defaults() });
        assert!(html.contains("<title>T</title>"));
        assert!(html.contains("<p>x</p>"));
    }
```

Run: `cargo test -p taliesin-core -- page_parts_defaults_assemble_a_minimal_page`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/render/page.rs crates/core/src/render/tests.rs crates/server/src/serve/mod.rs crates/server/src/serve_site/mod.rs
git commit -m "refactor(page): a PageParts defaults base so a new field is one edit, not four"
```

---

## Final verification (after all tasks)

- [ ] `cargo test -p taliesin-core && cargo test -p taliesin-server` — full suite green.
- [ ] `cargo fmt --check` — clean.
- [ ] `cargo clippy --workspace` — no new warnings.
- [ ] Re-grep the removed symbols: `rg -n "AboutSpec|about_html|parse_about|tali-about|local audio not found|search_button\(true\)|search_button\(false\)" crates/` — zero matches (except `search_button()` no-arg calls).
- [ ] Browser smoke via chrome-devtools MCP: `taliesin preview corpus/tech-blog` renders (homepage `hero:` intact, search button present, no console errors).
- [ ] Update `notes/2026-07-17-reduction-audit-map.md`: mark D1/D2/D3/T1 done and record R1's outcome (consolidated, or deferred with the divergence noted).

## Self-review notes

- **Spec coverage:** D1, D2, D3, R1, T1 map to the map's items of the same id; R2/T2, coverage gaps (C1-C7), doc drift, and worktrees are explicitly deferred per the owner's "Phase 2 + T1" scope.
- **R1 is honestly gated:** it may end in "do not consolidate" if the entity-decode sets diverge; the plan handles both branches rather than assuming the merge is safe.
- **No placeholder steps:** every code step shows the real edit; deletion sites are enumerated with exact paths + line numbers from a direct read.
