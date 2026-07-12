# Heading Demotion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a page emits a title-block `<h1 class="title">`, demote every body markdown heading one level so the page has exactly one `<h1>` (its title) and a correct nested outline.

**Architecture:** The block loop in `render/mod.rs` already knows, per heading, both its comrak level (`heading_level: Option<u8>`) and whether this render will insert a title block (`format == Html && !hide_title_block && title.is_some()`). We reuse that exact condition as a demotion gate and rewrite only the emitted `<hN>`/`</hN>` tag names (`N → min(N+1, 6)`), leaving id/sourcepos/block-id/text untouched. The on-page TOC filter changes from an absolute `level <= 3` to a filter relative to the shallowest heading present so a demoted page (sections start at `<h2>`) still shows three levels.

**Tech Stack:** Rust (edition 2024, let-chains), comrak block model, `cargo test -p taliesin-core`.

## Global Constraints

- No em dashes or en dashes in any code, comment, or copy. Use commas/colons/parentheses.
- Rust edition 2024, workspace resolver 3; let-chains (`if let … && …`) are allowed and idiomatic here.
- Every emitted block MUST keep `data-block-id` (source-keyed content hash) + `data-sourcepos`; included blocks keep `data-source-file`. Enforced by `crates/core/tests/corpus.rs`.
- `data-block-id` is `make_id(&block_src)` = a hash of the block's SOURCE text (`render/mod.rs:395`, `:1752`), NOT the emitted HTML. Demotion changes no block-id. (This corrects the spec's Mechanism-#3, which assumed an HTML hash.)
- Do NOT touch: `divs.rs`, `cite.rs`, `includes.rs`, the section/theorem numbering scanners, exec/kernel, or the deck engine core. Demotion is emission-only and Html-gated; decks (Reveal) and books (untitled numbered chapters) never enter the gate by construction.
- `rustfmt` runs on every edited `.rs` via a PostToolUse hook; keep the tree `cargo fmt`-clean.
- Work on branch `feat/heading-demotion` (already created; spec committed as `673c75f`). Commit each task; fast-forward merge to LOCAL `main` only at the end. NEVER push (the author pushes).

---

### Task 1: Relative TOC filter (behavior-preserving refactor)

Make the on-page TOC filter relative to the shallowest heading present, and DRY the two
call sites (`toc_entry_count` + `toc_html`) through one shared helper so their filters
cannot drift. For every document that exists today (shallowest heading is `<h1>`, so
`base == 1`) this is a no-op: `level - 1 <= 2` is identical to `level <= 3`. It only
changes behavior once Task 2 introduces `base == 2` (demoted) documents.

**Files:**
- Modify: `crates/core/src/render/mod.rs:1663-1688` (replace `toc_entry_count` and the head of `toc_html`)
- Test: `crates/core/src/render/tests.rs` (append one test)

**Interfaces:**
- Produces: `fn toc_items(blocks: &[Block]) -> Vec<(u8, String, String)>` (private to `render`), the shared filtered heading set `(level, id, text)`.
- `pub(crate) fn toc_entry_count(blocks: &[Block]) -> usize` keeps its signature.
- `fn toc_html(blocks: &[Block]) -> String` keeps its signature.

- [ ] **Step 1: Write the failing test**

Append to `crates/core/src/render/tests.rs`:

```rust
#[test]
fn toc_filter_is_relative_to_the_shallowest_heading() {
    // A titleless document whose sections start at <h2> (shallowest level = 2): the TOC
    // shows three levels (h2/h3/h4) and drops h5. This is the relative window, not the
    // old absolute `level <= 3` (which would have stopped at h3 and shown only two).
    let doc = render_document("## A\n\n### B\n\n#### C\n\n##### D\n");
    assert_eq!(toc_entry_count(&doc.blocks), 3, "h2/h3/h4 shown, h5 dropped");
    // A conventional document (shallowest level = 1) is unchanged: h1/h2/h3 shown, h4 dropped.
    let doc = render_document("# A\n\n## B\n\n### C\n\n#### D\n");
    assert_eq!(toc_entry_count(&doc.blocks), 3, "h1/h2/h3 shown, h4 dropped");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-core --lib toc_filter_is_relative_to_the_shallowest_heading`
Expected: FAIL on the first assertion (old absolute `level <= 3` counts only h2+h3 = 2, not 3).

- [ ] **Step 3: Replace the two filter sites with a shared relative helper**

In `crates/core/src/render/mod.rs`, replace the current `toc_entry_count` (lines ~1667-1675) and the HEAD of `toc_html` (lines ~1676-1688, down to and including the `let base = …` line) with:

```rust
/// The headings the on-page TOC shows: those carrying an anchor `id`, within two levels
/// of the shallowest heading present (`level - base <= 2`). `toc_entry_count` and
/// `toc_html` share this so their filters cannot drift, and so a title-demoted page
/// (whose sections start at `<h2>`) still surfaces three levels instead of two.
fn toc_items(blocks: &[Block]) -> Vec<(u8, String, String)> {
    let all: Vec<(u8, String, String)> = blocks
        .iter()
        .filter_map(|b| {
            Some((
                block_heading_level(&b.html)?,
                extract_attr(&b.html, "id")?,
                strip_tags(&b.html),
            ))
        })
        .collect();
    let Some(base) = all.iter().map(|(l, _, _)| *l).min() else {
        return Vec::new();
    };
    all.into_iter().filter(|(l, _, _)| *l - base <= 2).collect()
}

/// How many entries the table of contents would list (exactly the set [`toc_html`]
/// renders). The site auto-gates the "on this page" TOC on this count: a short page reads
/// as one column; only a long, chunkable page earns the sidebar TOC (NN/g).
pub(crate) fn toc_entry_count(blocks: &[Block]) -> usize {
    toc_items(blocks).len()
}

fn toc_html(blocks: &[Block]) -> String {
    let items = toc_items(blocks);
    if items.is_empty() {
        return String::new();
    }
    let base = items.iter().map(|(l, _, _)| *l).min().unwrap();
```

Leave everything in `toc_html` from the `let mut out = String::from(…)` line onward exactly as it is (the `<nav>`-building loop is unchanged; `base` is still in scope). Delete the old doc-comment above `toc_entry_count` (its `level <= 3` / "lockstep" wording is now stale and replaced by the `toc_items` doc-comment).

Note the `*l - base` subtraction never underflows: `base` is the minimum, so `*l >= base` for every item.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p taliesin-core --lib toc_filter_is_relative_to_the_shallowest_heading`
Expected: PASS.

- [ ] **Step 5: Confirm no existing behavior drifted**

Run: `cargo test -p taliesin-core`
Expected: PASS (all existing docs have `base == 1`, so the filter is identical to before; `website_renders_with_toc_anchored_headings_and_numbered_figures` and the snapshot tests stay green).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/mod.rs crates/core/src/render/tests.rs
git commit -m "refactor(render): make the TOC filter relative to the shallowest heading

Share one toc_items() helper between toc_entry_count and toc_html so their
filters cannot drift, and switch from an absolute level<=3 to level-base<=2.
No-op for today's base=1 documents; readies the TOC for title-demoted pages."
```

---

### Task 2: Heading demotion mechanism

Add the `demote_heading_html` helper and the demotion gate in the block loop. With Task 1
already in place, a demoted document's TOC is automatically correct.

**Files:**
- Modify: `crates/core/src/render/mod.rs` (add `demote_heading_html` near the block-id helpers ~line 1755; add the gate in the block loop after the emit if/else chain ~line 615)
- Test: `crates/core/src/render/tests.rs` (append five tests)

**Interfaces:**
- Consumes: `heading_level: Option<u8>`, `format: DocFormat`, `hide_title_block: bool`, `title: Option<String>`, and the built `html: String`, all live in the block loop.
- Produces: `fn demote_heading_html(html: &str, level: u8) -> String` (private to `render`).

- [ ] **Step 1: Write the failing tests**

Append to `crates/core/src/render/tests.rs`:

```rust
#[test]
fn title_block_demotes_body_headings_to_a_single_h1() {
    let doc = render_document("---\ntitle: \"Post\"\n---\n\n# Theory\n\n## Model\n\n### Detail\n");
    // The only <h1> is the title block; body sections each shift down one level.
    let body = doc.blocks.iter().map(|b| b.html.as_str()).collect::<String>();
    assert_eq!(body.matches("<h1").count(), 1, "exactly one h1 (the title):\n{body}");
    assert!(doc.blocks[0].html.contains("<h1 class=\"title\">Post</h1>"));
    assert!(doc.blocks[1].html.starts_with("<h2 "), "# Theory -> h2, got: {}", doc.blocks[1].html);
    assert!(doc.blocks[2].html.starts_with("<h3 "), "## Model -> h3, got: {}", doc.blocks[2].html);
    assert!(doc.blocks[3].html.starts_with("<h4 "), "### Detail -> h4, got: {}", doc.blocks[3].html);
}

#[test]
fn demotion_preserves_anchor_id_and_source_keyed_block_id() {
    let titled = render_document("---\ntitle: T\n---\n\n# Methods\n");
    let demoted = &titled.blocks[1]; // the body heading, demoted h1 -> h2
    assert!(demoted.html.starts_with("<h2 "), "got: {}", demoted.html);
    // The anchor slug is text-derived, so it survives demotion (#anchors + @sec- refs hold).
    assert!(demoted.html.contains("id=\"methods\""), "anchor id unchanged: {}", demoted.html);
    // block-id hashes the SOURCE line, not the emitted tag: same source `# Methods` -> same id.
    let undemoted = render_document("# Methods\n"); // no title block, so <h1> stays
    assert_eq!(demoted.id, undemoted.blocks[0].id, "block-id keys off source, not the tag");
}

#[test]
fn heading_demotion_clamps_at_h6() {
    let doc = render_document("---\ntitle: T\n---\n\n###### Deep\n");
    // A body <h6> has nowhere lower to go; it stays <h6> (never <h7>).
    assert!(doc.blocks[1].html.starts_with("<h6 "), "got: {}", doc.blocks[1].html);
}

#[test]
fn hidden_title_block_leaves_body_headings_alone() {
    // `title-block-style: none` emits no title block, so the trigger is absent: a body
    // `# Section` stays <h1> (the author's own heading hierarchy is untouched).
    let doc = render_document("---\ntitle: T\ntitle-block-style: none\n---\n\n# Section\n");
    assert!(doc.blocks[0].html.starts_with("<h1 "), "got: {}", doc.blocks[0].html);
}

#[test]
fn deck_headings_are_not_demoted() {
    // A deck (Reveal) builds its own title slide and uses h1/h2 as slide breaks; demotion
    // must never touch it. `## Slide` stays <h2> (the slide-open level), `### Point` <h3>.
    let doc = render_document("---\ntitle: T\nformat: deck\n---\n\n## Slide\n\n### Point\n");
    let joined = doc.blocks.iter().map(|b| b.html.as_str()).collect::<String>();
    assert!(joined.contains("<h2 "), "slide heading stays h2:\n{joined}");
    assert!(joined.contains("<h3 "), "sub-heading stays h3:\n{joined}");
}

#[test]
fn a_demoted_post_still_lists_all_its_sections_in_the_toc() {
    // After demotion the sections are h2/h3/h4; the relative TOC filter surfaces all three
    // (the title block starts with <header>, not <hN>, so it is not counted as a heading).
    let doc = render_document("---\ntitle: T\n---\n\n# A\n\n## B\n\n### C\n");
    assert_eq!(toc_entry_count(&doc.blocks), 3, "all three demoted sections listed");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p taliesin-core --lib demot`
Expected: FAIL (e.g. `title_block_demotes_body_headings_to_a_single_h1` sees `# Theory` still `<h1 `, so `<h1` count is 4, not 1). `hidden_title_block_leaves_body_headings_alone` and `deck_headings_are_not_demoted` may already pass (nothing demotes yet); that is fine, they are guard rails for Step 3.

- [ ] **Step 3: Add the `demote_heading_html` helper**

In `crates/core/src/render/mod.rs`, add near the block-id helpers (right after `make_id`, ~line 1755):

```rust
/// Demote a heading block's visible tag one level (`<hN>` -> `<h{N+1}>`, clamped at
/// `<h6>`), leaving its attributes, `id`, `data-block-id`, `data-sourcepos` and text
/// untouched. Used when a page renders a title-block `<h1 class="title">` so its body
/// sections nest beneath the single page title: one `<h1>` per page (a11y + SEO).
fn demote_heading_html(html: &str, level: u8) -> String {
    let to = (level + 1).min(6);
    if to == level {
        return html.to_string();
    }
    // `html` is `<hN...>...</hN>`: rewrite only the opening tag name (at index 0) and the
    // lone closing tag. Heading text has its `<`/`>` escaped to entities, so the literal
    // `</hN>` appears exactly once (the real closing tag).
    html.replacen(&format!("<h{level}"), &format!("<h{to}"), 1)
        .replacen(&format!("</h{level}>"), &format!("</h{to}>"), 1)
}
```

- [ ] **Step 4: Add the demotion gate in the block loop**

In `crates/core/src/render/mod.rs`, immediately AFTER the emit if/else chain that builds
`html` (after the closing `}` of the `} else { … }` block at ~line 615) and BEFORE the
deck heading-background block (`if heading_level.is_some() && format == DocFormat::Reveal`
at ~line 619), insert:

```rust
        // One <h1> per page: when this render emits a visible title block, demote every
        // body heading one level so sections nest beneath the title. The gate mirrors the
        // title-block insertion condition exactly (Html, not hidden, titled). Decks
        // (Reveal) and books (untitled, numbered chapters) never satisfy it, so their
        // slide-break and section-numbering machinery is never entered.
        if let Some(level) = heading_level
            && format == DocFormat::Html
            && !hide_title_block
            && title.is_some()
        {
            html = demote_heading_html(&html, level);
        }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p taliesin-core --lib demot toc_filter title_block deck reveal`
Expected: PASS for the five new demotion tests and the Task-1 TOC test.

Run: `cargo clippy -p taliesin-core --all-targets`
Expected: clean (no warnings).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/mod.rs crates/core/src/render/tests.rs
git commit -m "feat(render): demote body headings under a title-block h1

When a page emits its <h1 class=\"title\"> title block, demote every body
markdown heading one level (clamped at h6) so the page has one <h1> and a
correct nested outline (a11y/SEO). Emission-only: id, block-id, sourcepos and
text are untouched. Gated exactly on the title-block insertion condition, so
decks and books are excluded by construction. Closes backlog #11."
```

---

### Task 3: Corpus invariant + full regression sweep + browser verification

Pin the fix corpus-wide with an affirmative invariant, then run the whole net (including
the excluded-path guards that already exist) and verify a real post in the browser.

**Files:**
- Test: `crates/core/tests/corpus.rs` (append one test)

**Interfaces:**
- Consumes: `corpus_dir()` (from `mod common`), `collect_qmd(&Path, &mut Vec<PathBuf>)`, `taliesin_core::render_document_with_includes`, `RenderedDoc::body_html()`.

- [ ] **Step 1: Write the failing corpus invariant**

Append to `crates/core/tests/corpus.rs`:

```rust
#[test]
fn every_titled_post_emits_exactly_one_h1() {
    // Heading demotion (#11): a post renders its title as the sole <h1>; its body `#`
    // sections demote to <h2>+ so the page keeps a single-<h1> document outline (a11y/SEO).
    let posts_dir = corpus_dir().join("tech-blog/posts");
    let mut posts = Vec::new();
    collect_qmd(&posts_dir, &mut posts);
    assert!(!posts.is_empty(), "expected posts under {}", posts_dir.display());
    for f in &posts {
        let src = fs::read_to_string(f).unwrap();
        let doc = taliesin_core::render_document_with_includes(&src, f.parent().unwrap());
        let n = doc.body_html().matches("<h1").count();
        let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
        assert_eq!(n, 1, "{label} should emit exactly one <h1>, found {n}");
    }
}
```

- [ ] **Step 2: Run it to confirm it passes (the fix is already in from Task 2)**

Run: `cargo test -p taliesin-core --test corpus every_titled_post_emits_exactly_one_h1`
Expected: PASS. (If any post fails, read its rendered body: a legitimate exception means the invariant needs scoping, but every current post is `title` + `#` sections and should collapse to one `<h1>`.)

Sanity that it is a real guard: temporarily comment out the demotion gate from Task 2, re-run, and confirm this test FAILS (posts show 3+ `<h1>`); then restore the gate. This is a manual gate-the-gate check, not a committed step.

- [ ] **Step 3: Confirm the excluded paths and snapshots did not drift**

Run: `cargo test -p taliesin-core --test body_html_snapshots`
Expected: PASS with NO drift. (The four snapshotted `{js}` docs are title-blocked but have zero body markdown headings, so demotion changes nothing in them. If any drifts, STOP and investigate before regenerating.)

Run: `cargo test -p taliesin-core`
Expected: PASS. In particular `website_renders_with_toc_anchored_headings_and_numbered_figures` (asserts a book chapter's `<h1 id="introduction">` is NOT demoted) and `reveal_deck_detects_format_and_splits_into_slides` (asserts deck slides still split on h1/h2) stay green: these are the pre-existing regression guards for the book and deck exclusions.

- [ ] **Step 4: Browser verification of a real post**

Rebuild the binary so the browser sees the change (assets/render are compiled in):

```bash
cargo build -p taliesin-server
```

Then preview the tech-blog site and open a post:

```bash
cargo run -p taliesin-server -- preview corpus/tech-blog 4388
```

With the chrome-devtools MCP, navigate to the EM-algorithm post
(`http://localhost:4388/posts/em-algorithm/index.html`) and verify:
- exactly one `<h1>` on the page and it is the title block (`document.querySelectorAll('h1').length === 1`, and it carries `class="title"`);
- the former body `# Theory` / `# Code demo` render as `<h2>` and their `##` children as `<h3>` (spot-check the DOM outline);
- the "on this page" TOC still lists those sections (nesting intact);
- check at desktop (1440x900) and mobile (390x844) widths.

Take a screenshot at each width and confirm the console is clean. Stop the preview when done.

- [ ] **Step 5: Commit**

```bash
git add crates/core/tests/corpus.rs
git commit -m "test(corpus): pin one <h1> per titled post (heading demotion #11)"
```

- [ ] **Step 6: Remove the backlog item and fast-forward to local main**

Delete backlog entry #11 (`#multiple-h1-per-post`) from `notes/backlog.md` (completed items are removed, never left checked), commit that removal, then fast-forward merge `feat/heading-demotion` into local `main`. Do NOT push.

```bash
git add notes/backlog.md
git commit -m "docs(backlog): drop completed #11 heading demotion"
git checkout main && git merge --ff-only feat/heading-demotion
```

---

## Self-Review

**Spec coverage:**
- Trigger = title-block h1 (Html, not hidden, titled): Task 2 gate mirrors the insertion condition exactly. ✓
- Demote emitted tag only, `N -> min(N+1, 6)`, id/sourcepos/level/block-id preserved: Task 2 `demote_heading_html` + `demotion_preserves_anchor_id_and_source_keyed_block_id`. ✓ (Spec's Mechanism-#3 HTML-hash premise corrected in Global Constraints: block-id is source-keyed, so it does not change at all.)
- TOC relative filter `level - base <= 2`: Task 1. ✓
- Books/decks excluded by construction: Task 2 comment + `deck_headings_are_not_demoted`; existing book/deck corpus tests guard it (Task 3 Step 3). ✓
- Corpus pins (post one-h1; book/deck regression): Task 3 invariant + existing guards. ✓
- Non-goals (no new knob, no book renumber, no deck level change): honored; no config added. ✓

**Placeholder scan:** none. Every code step shows complete code and an exact command with expected output.

**Type consistency:** `toc_items(&[Block]) -> Vec<(u8, String, String)>` defined in Task 1 and consumed by `toc_entry_count`/`toc_html` in the same task; `demote_heading_html(&str, u8) -> String` defined and called in Task 2; `heading_level: Option<u8>` matches `block_heading_level`'s `u8` and comrak `h.level: u8`. Consistent.

**Ambiguity check:** the one spec-deferred choice (rewrite emitted tag vs. bump the comrak node) is resolved in Task 2 to the string rewrite, justified by the fact that block-id is source-keyed (so rewrite ordering relative to the hash is irrelevant) and that the rewrite stays entirely inside `mod.rs` without threading a flag through the recursive `emit`.
