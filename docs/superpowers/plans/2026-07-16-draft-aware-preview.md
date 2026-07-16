# Draft-aware preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `preview` shows draft pages inline (in nav/listings/prev-next, badged); `build`/`publish` exclude them and report "N drafts not published: …".

**Architecture:** The split lives at **discovery mode**. `Site::discover(root)` stays the published view (drops drafts, records their rels in `Site.excluded_drafts`) — build/publish/check/map/query keep the exact page set they have today. A new `Site::discover_with(root, DraftMode::Include)`, used only by the 4 preview call sites in `serve_site/mod.rs`, keeps drafts in the page set tagged `Page.draft: true` so they flow through numbering/listings/prev-next/nav. Badges and the dev-menu count are driven off `Page.draft`, which is always `false` in a build, so they're structurally inert there.

**Tech Stack:** Rust (edition 2024), `taliesin-core` + `taliesin-server`; vanilla JS client (`web-client/client.js`, `// @ts-check`); corpus regression tests (`cargo test -p taliesin-core`); chrome-devtools MCP for browser verification.

## Global Constraints

- **HTML-only output.** No new output format; no CDN; no preview write-back.
- **Block-model invariants:** every emitted block keeps `data-block-id` + `data-sourcepos`. This change alters page *membership* + adds one `bool`, never block emission.
- **Do-NOT-touch machinery:** `:::` scanner (`divs.rs`), `cite.rs`, `includes.rs`, numbering scanners, exec/freeze/kernel. Discovery mode only gates which pages enter the existing pipeline.
- **Minimal blast radius:** the published path (`Site::discover`) stays byte-identical to today. Drafts are strictly additive on the preview path.
- **Naming:** the enum is `DraftMode { Exclude, Include }`; the field is `Page.draft`; the site field is `Site.excluded_drafts: Vec<String>` (rel paths). Badge class `tali-draft-badge`; page banner class `tali-draft-banner`; injected global `window.TALIESIN_DRAFTS`.
- **rustfmt** runs on save (a `PostToolUse` hook); keep the tree `cargo fmt`-clean.
- **Never** commit to `main`; work on branch `draft-aware-preview` (already created). Fast-forward merge locally only when asked; the author pushes.

---

### Task 1: Corpus pins + data model (compiles, no behaviour change)

Add the two corpus drafts and the inert data-model fields first (pin-first, project ethos). With the fields defaulting to `false`/empty and no logic yet, drafts still vanish from every build exactly as today, so the whole suite stays green.

**Files:**
- Create: `corpus/tech-blog/posts/draft-example/index.tmd`
- Create: `corpus/demo-book/appendix.tmd`
- Modify: `corpus/demo-book/_site.yml` (append the appendix chapter)
- Modify: `crates/core/src/site/mod.rs` (`Page` struct + `Site` struct + `DraftMode` enum + `Page`/`Site` literal fields)
- Modify: `crates/core/src/site/discovery.rs` (`Page { … , draft: false }`)
- Modify: `crates/core/src/site/book.rs` (`Page { … , draft: false }`)
- Test: existing `crates/core/tests/*` must stay green.

**Interfaces:**
- Produces: `pub enum DraftMode { Exclude, Include }`; `Page.draft: bool`; `Site.excluded_drafts: Vec<String>`.

- [ ] **Step 1: Add the draft website post pin.**

Create `corpus/tech-blog/posts/draft-example/index.tmd`:

```markdown
---
title: "A Draft Post (unpublished)"
date: "2026-07-16"
description: "This post is a draft: it shows in preview with a DRAFT badge but never ships in a build."
categories:
  - Drafts
---

This post exists to pin **draft-aware preview**. In `preview` it appears in the blog
listing with a quiet `DRAFT` badge and in the dev-menu draft list; a `build` excludes it
entirely and reports it as "not published".

Nothing here needs to render richly; the point is the page's *membership*.
```

- [ ] **Step 2: Add the draft book-chapter pin.**

Create `corpus/demo-book/appendix.tmd`:

```markdown
---
draft: true
---

# Appendix: Draft Notes

A draft chapter. In `preview` it appears last in the chapter drawer (marked draft) and is
numbered in context; a `build` drops it, so the other chapters keep contiguous numbers.
```

- [ ] **Step 3: Wire the appendix into the book, last.**

In `corpus/demo-book/_site.yml`, append to the `chapters:` list (after the existing `- file: summary.tmd` entry) so an Exclude build stays byte-identical (no mid-list renumber):

```yaml
  - appendix.tmd            # draft: true — shows in preview only (draft-aware pin)
```

- [ ] **Step 4: Add `DraftMode` + the struct fields.**

In `crates/core/src/site/mod.rs`, add the enum near the top of the file's type
declarations (just above `pub struct Page {`):

```rust
/// Whether discovery keeps `draft: true` pages (`Include`, the preview view) or drops
/// them from the page set (`Exclude`, the published view: build/publish/check/map).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftMode {
    Exclude,
    Include,
}
```

Add the field to `pub struct Page` (after `has_bibliography: bool`):

```rust
    /// `draft: true` in front matter. `false` for every published page; `true` only for a
    /// draft surfaced in `DraftMode::Include` (preview). Drives the DRAFT badge/banner; a
    /// built page is always `false`, so those affordances are inert in a build.
    pub draft: bool,
```

Add the field to `pub struct Site` (after `decks: Vec<DeckRef>,`):

```rust
    /// Rel paths of `draft: true` pages dropped in `DraftMode::Exclude` (empty in
    /// `Include`). Drives the build's "N drafts not published" report.
    pub excluded_drafts: Vec<String>,
```

- [ ] **Step 5: Set the new fields at every literal site (default values).**

In `crates/core/src/site/discovery.rs`, in the `Some(Page { … })` of `website_pages`, add `draft: fm.draft,` (fm already parsed at line 18; this tags the field with no filtering yet — filtering lands in Task 2).

In `crates/core/src/site/book.rs`, in the `Page { … }` of `book_pages`, add `draft: false,` (book drafts land in Task 3).

In `crates/core/src/site/mod.rs`, in the `let mut site = Site { … }` inside `discover`, add `excluded_drafts: Vec::new(),`.

- [ ] **Step 6: Run the suite to confirm no behaviour change.**

Run: `cargo test -p taliesin-core 2>&1 | tail -20`
Expected: PASS. (`website_pages` still `return None`s on a draft at line 21, so the new `draft:` field is only ever set on published pages here; the corpus draft post + book appendix are dropped exactly as before — build snapshots/counts unchanged.)

- [ ] **Step 7: Commit.**

```bash
git add corpus/tech-blog/posts/draft-example crates/core/src/site/mod.rs \
        crates/core/src/site/discovery.rs crates/core/src/site/book.rs \
        corpus/demo-book/appendix.tmd corpus/demo-book/_site.yml
git commit -m "feat(site): draft-aware data model + corpus pins (inert)"
```

---

### Task 2: Website draft mode (`discover_with` + `website_pages`)

The core split for websites: `Site::discover` = Exclude (drops drafts, records rels), `discover_with(Include)` keeps them tagged, preview call sites opt into Include.

**Files:**
- Modify: `crates/core/src/site/mod.rs` (`discover` → `discover_with`; thread mode + `excluded_drafts`)
- Modify: `crates/core/src/site/discovery.rs` (`website_pages` takes `mode` + `excluded`)
- Modify: `crates/server/src/serve_site/mod.rs:119,145,1096,1119` (→ `discover_with(root, DraftMode::Include)`)
- Test: `crates/core/src/site/mod.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `DraftMode`, `Page.draft`, `Site.excluded_drafts` (Task 1).
- Produces: `pub fn Site::discover_with(root: &Path, drafts: DraftMode) -> Site`; `pub fn Site::discover(root: &Path) -> Site` (unchanged signature, now `= discover_with(root, DraftMode::Exclude)`); `fn website_pages(root: &Path, mode: DraftMode, warnings: &mut Vec<String>, excluded: &mut Vec<String>) -> Vec<Page>`.

- [ ] **Step 1: Write the failing test.**

In `crates/core/src/site/mod.rs` `#[cfg(test)]` module, add (mirror the existing `discover_*` tests' tmp-dir helper — they use a `tmp(name)` + write `.tmd` files; reuse that pattern):

```rust
#[test]
fn discover_excludes_drafts_but_records_them_include_keeps_them() {
    let dir = tmp("draft-mode");
    std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
    std::fs::write(dir.join("live.tmd"), "---\ntitle: Live\n---\nbody\n").unwrap();
    std::fs::write(dir.join("wip.tmd"), "---\ntitle: WIP\ndraft: true\n---\nbody\n").unwrap();

    let published = Site::discover(dir); // == discover_with(Exclude)
    assert!(published.pages.iter().any(|p| p.rel == "live.tmd"));
    assert!(!published.pages.iter().any(|p| p.rel == "wip.tmd"), "draft must be absent from the published set");
    assert_eq!(published.excluded_drafts, vec!["wip.tmd".to_string()]);

    let preview = Site::discover_with(dir, DraftMode::Include);
    let wip = preview.pages.iter().find(|p| p.rel == "wip.tmd").expect("draft present in preview");
    assert!(wip.draft, "the draft page is tagged");
    assert!(preview.excluded_drafts.is_empty(), "Include excludes nothing");
}
```

*(If `tmp()` isn't a shared helper in this module, use the same construction the neighbouring `discover_*` tests use to create a temp site dir.)*

- [ ] **Step 2: Run it to verify it fails.**

Run: `cargo test -p taliesin-core discover_excludes_drafts 2>&1 | tail -15`
Expected: FAIL to compile — `discover_with` / `DraftMode` not in scope at the call, or (once resolved) the assertion `excluded_drafts == ["wip.tmd"]` fails because `discover` doesn't populate it yet.

- [ ] **Step 3: Thread mode into `website_pages`.**

In `crates/core/src/site/discovery.rs`, change the signature and the draft branch:

```rust
pub(super) fn website_pages(
    root: &Path,
    mode: DraftMode,
    warnings: &mut Vec<String>,
    excluded: &mut Vec<String>,
) -> Vec<Page> {
```

Replace the current draft early-return (lines 19-23) with:

```rust
            // `draft: true`: dropped from the published set (Exclude) — recorded so the
            // build can report it — or kept and tagged for the preview view (Include).
            if fm.draft && mode == DraftMode::Exclude {
                excluded.push(rel);
                return None;
            }
```

(The `Some(Page { … , draft: fm.draft })` from Task 1 already tags the field for the Include case.)

- [ ] **Step 4: Split `discover` into `discover_with` + a thin wrapper; populate `excluded_drafts`.**

In `crates/core/src/site/mod.rs`, rename `pub fn discover(root: &Path) -> Site {` to `pub fn discover_with(root: &Path, drafts: DraftMode) -> Site {` and, at the top of its body, add `let mut excluded_drafts = Vec::new();`. Change the website arm to pass the mode + sink:

```rust
        } else {
            (
                website_pages(root, drafts, &mut warnings, &mut excluded_drafts),
                None,
            )
        };
```

In the `let mut site = Site { … }` literal, set `excluded_drafts,` (shorthand). Immediately below the renamed fn, add the wrapper:

```rust
    /// Discover a project's pages (published view: `draft: true` pages are excluded and
    /// recorded in [`Site::excluded_drafts`]). Used by build/publish/check/map/query.
    pub fn discover(root: &Path) -> Site {
        Self::discover_with(root, DraftMode::Exclude)
    }
```

- [ ] **Step 5: Point the preview call sites at Include.**

First re-export the enum: in `crates/core/src/lib.rs`, add `DraftMode` to the existing `pub use site::{…};` re-export line that already exports `Site` (so `taliesin_core::DraftMode` resolves). Then in `crates/server/src/serve_site/mod.rs`, change the 4 `Site::discover(&root)` / `Site::discover(&mroot)` / `Site::discover(&app.root)` calls at lines ~119, ~145, ~1096, ~1119 to `Site::discover_with(<path>, taliesin_core::DraftMode::Include)` (matching each call's path variable: `&root`, `&mroot`, `&app.root`, `&app.root`).

- [ ] **Step 6: Run the test + full suite.**

Run: `cargo test -p taliesin-core 2>&1 | tail -20`
Expected: PASS (new test green; all existing green — `discover` behaves exactly as before for build/publish/check/map since it now calls `discover_with(Exclude)`).

- [ ] **Step 7: Commit.**

```bash
git add -A
git commit -m "feat(site): discover_with(DraftMode) — preview keeps drafts, build excludes"
```

---

### Task 3: Book chapter drafts (`build_book` mode + drawer marker)

A draft chapter must be dropped inside `build_book` in Exclude (so numbering stays contiguous and the drawer never links an unbuilt page) and kept + marked in Include.

**Files:**
- Modify: `crates/core/src/site/book.rs` (`BookEntry.draft`; `build_book`/`push_chapter_entry`/`push_chapter` take `mode` + `excluded`; `book_pages` reads `c.draft`)
- Modify: `crates/core/src/site/mod.rs` (`discover_with` passes `drafts` + `&mut excluded_drafts` into `build_book`)
- Modify: `crates/core/src/site/chrome.rs` (drawer entry shows a draft marker)
- Test: `crates/core/src/site/mod.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `DraftMode`, `Page.draft`, `excluded_drafts` (Tasks 1-2).
- Produces: `BookEntry.draft: bool`; `fn build_book(root: &Path, config: &SiteConfig, mode: DraftMode, excluded: &mut Vec<String>) -> Book`.

- [ ] **Step 1: Write the failing test.**

```rust
#[test]
fn book_drafts_excluded_renumber_contiguously_include_numbers_in_context() {
    let dir = tmp("book-draft");
    std::fs::write(dir.join("_site.yml"),
        "title: B\nchapters:\n  - one.tmd\n  - wip.tmd\n  - two.tmd\n").unwrap();
    std::fs::write(dir.join("one.tmd"), "# One\n").unwrap();
    std::fs::write(dir.join("wip.tmd"), "---\ndraft: true\n---\n# WIP\n").unwrap();
    std::fs::write(dir.join("two.tmd"), "# Two\n").unwrap();

    let published = Site::discover(dir);
    assert!(!published.pages.iter().any(|p| p.rel == "wip.tmd"));
    assert_eq!(published.excluded_drafts, vec!["wip.tmd".to_string()]);
    let book = published.book.as_ref().unwrap();
    // Chapters renumber contiguously: One=1, Two=2 (no gap where WIP was).
    let nums: Vec<u32> = book.chapters().iter().filter_map(|c| c.number).collect();
    assert_eq!(nums, vec![1, 2]);
    assert!(!book.chapters().iter().any(|c| c.rel == "wip.tmd"));

    let preview = Site::discover_with(dir, DraftMode::Include);
    let pbook = preview.book.as_ref().unwrap();
    let wip = pbook.chapters().iter().find(|c| c.rel == "wip.tmd").expect("draft chapter present in preview");
    assert!(wip.draft);
    assert_eq!(wip.number, Some(2), "numbered in context (One=1, WIP=2, Two=3)");
    assert!(preview.pages.iter().any(|p| p.rel == "wip.tmd" && p.draft));
}
```

- [ ] **Step 2: Run it to verify it fails.**

Run: `cargo test -p taliesin-core book_drafts_excluded 2>&1 | tail -15`
Expected: FAIL — `build_book` signature mismatch / `BookEntry.draft` missing.

- [ ] **Step 3: Add `BookEntry.draft` + thread mode through `build_book`.**

In `crates/core/src/site/book.rs`:

Add the field to `pub struct BookEntry` (after `url: String,`):

```rust
    /// `draft: true` front matter on the chapter file. Only ever `true` in
    /// `DraftMode::Include` (a draft chapter is dropped entirely in `Exclude`).
    pub draft: bool,
```

Change `build_book` + the two push helpers to carry `mode` + `excluded`:

```rust
pub(super) fn build_book(
    root: &Path,
    config: &SiteConfig,
    mode: DraftMode,
    excluded: &mut Vec<String>,
) -> Book {
    let mut entries = Vec::new();
    let mut num = 0u32;
    for ch in &config.chapters {
        if push_chapter_entry(root, ch, &mut entries, &mut num, mode, excluded) {
            continue;
        }
        if let Some(map) = ch.as_mapping() {
            let part = map.get("part").and_then(|v| v.as_str()).unwrap_or("").to_string();
            entries.push(BookEntry { part: Some(part), ..Default::default() });
            if let Some(seq) = map.get("chapters").and_then(|v| v.as_sequence()) {
                for c in seq {
                    push_chapter_entry(root, c, &mut entries, &mut num, mode, excluded);
                }
            }
        }
    }
    Book { title: config.title.clone(), entries }
}
```

`push_chapter_entry` gains `mode: DraftMode, excluded: &mut Vec<String>` and forwards them to both `push_chapter(root, file, None, entries, num, mode, excluded)` calls.

Rewrite `push_chapter` to read front matter once and drop-or-tag the draft:

```rust
fn push_chapter(
    root: &Path,
    file: &str,
    label: Option<&str>,
    entries: &mut Vec<BookEntry>,
    num: &mut u32,
    mode: DraftMode,
    excluded: &mut Vec<String>,
) {
    let input = root.join(file);
    let rel = file.to_string();
    let (h1, unnumbered) = chapter_heading(&input);
    // Parse once: needed for the draft gate and the title fallback.
    let fm = parse_front_matter(&input, file, &mut Vec::new());
    if fm.draft && mode == DraftMode::Exclude {
        excluded.push(rel);
        return; // no entry, no number bump — the book renumbers as if it weren't listed
    }
    let title = label
        .map(str::to_string)
        .or(h1)
        .or(fm.title)
        .unwrap_or_else(|| crate::ext::strip_source_ext(&rel).unwrap_or(&rel).to_string());
    let number = if unnumbered || crate::ext::strip_source_ext(&rel) == Some("index") {
        None
    } else {
        *num += 1;
        Some(*num)
    };
    entries.push(BookEntry {
        part: None,
        number,
        title,
        url: qmd_to_html(&rel),
        rel,
        draft: fm.draft,
    });
}
```

- [ ] **Step 4: Carry `draft` from the entry into the book Page; update the discover call.**

In `book_pages`, set the `Page` field from the entry: change `draft: false,` (Task 1) to `draft: c.draft,`.

In `crates/core/src/site/mod.rs` `discover_with`, change the book arm:

```rust
        let (mut pages, book) = if config.is_book {
            let book = build_book(root, &config, drafts, &mut excluded_drafts);
            let pages = book_pages(root, &book, &mut warnings);
            (pages, Some(book))
        } else {
```

- [ ] **Step 5: Mark a draft chapter in the drawer.**

In `crates/core/src/site/chrome.rs`, in the sidebar/drawer builder (`sidebar_html`, the loop over `book.entries`), where a chapter row's label is emitted, append a marker when `entry.draft`. Locate the chapter `<a>`/label format and add:

```rust
        let draft_tag = if e.draft {
            " <span class=\"tali-draft-badge\">draft</span>"
        } else {
            ""
        };
```

and interpolate `{draft_tag}` after the chapter title text in that row's format string. (Keep it inside the existing `<a>` so it clicks through to the chapter.)

- [ ] **Step 6: Run the test + full suite.**

Run: `cargo test -p taliesin-core 2>&1 | tail -20`
Expected: PASS. (Existing demo-book snapshots unchanged: its draft appendix is *last*, so an Exclude build renumbers to the same numbers as before it was added.)

- [ ] **Step 7: Commit.**

```bash
git add -A
git commit -m "feat(site): draftable book chapters (drop+renumber in build, mark in preview)"
```

---

### Task 4: Build report — "N drafts not published"

**Files:**
- Modify: `crates/server/src/build.rs` (in `run_site_build`, after `Site::discover`, log `excluded_drafts`)
- Test: `crates/server/src/build.rs` `#[cfg(test)]` (a small helper-level assertion on the message) OR a corpus-level build assertion.

**Interfaces:**
- Consumes: `Site.excluded_drafts` (Task 1).

- [ ] **Step 1: Write the failing test.**

Add a pure formatter so the message is unit-testable without a full build. In `crates/server/src/build.rs`:

```rust
#[cfg(test)]
mod draft_report_tests {
    use super::draft_report_line;
    #[test]
    fn reports_count_and_names() {
        assert_eq!(draft_report_line(&[]), None);
        assert_eq!(
            draft_report_line(&["a.tmd".into(), "posts/b/index.tmd".into()]),
            Some("2 drafts not published: a.tmd, posts/b/index.tmd".to_string())
        );
        assert_eq!(
            draft_report_line(&["only.tmd".into()]),
            Some("1 draft not published: only.tmd".to_string())
        );
    }
}
```

- [ ] **Step 2: Run it to verify it fails.**

Run: `cargo test -p taliesin-server draft_report 2>&1 | tail -12`
Expected: FAIL — `draft_report_line` not found.

- [ ] **Step 3: Implement the formatter + call it.**

Add near the top of `crates/server/src/build.rs` (module-level fn):

```rust
/// The "N drafts not published" build report line, or `None` when nothing was held back.
/// Singular/plural aware; names the rel paths so the author sees exactly what was excluded.
pub(crate) fn draft_report_line(excluded: &[String]) -> Option<String> {
    if excluded.is_empty() {
        return None;
    }
    let n = excluded.len();
    let noun = if n == 1 { "draft" } else { "drafts" };
    Some(format!("{n} {noun} not published: {}", excluded.join(", ")))
}
```

In `run_site_build`, right after `let site = taliesin_core::Site::discover(root);` (build.rs:1182), log it:

```rust
    if let Some(line) = draft_report_line(&site.excluded_drafts) {
        crate::log::info(&line); // match the surrounding logging style at this call site
    }
```

(Use whatever info-logging helper the neighbouring lines in `run_site_build` use — mirror the existing "built N pages" style rather than a bare `println!`.)

- [ ] **Step 4: Run the test + build a real draft site.**

Run: `cargo test -p taliesin-server draft_report 2>&1 | tail -8` → PASS.
Run: `cargo run -p taliesin-server -- build corpus/demo-book --out /tmp/db-out 2>&1 | grep -i draft`
Expected: a line `1 draft not published: appendix.tmd`, and `/tmp/db-out` contains no `appendix.html`.

- [ ] **Step 5: Commit.**

```bash
git add crates/server/src/build.rs
git commit -m "feat(build): report drafts held back (N drafts not published: …)"
```

---

### Task 5: DRAFT badges — listing card + page banner

**Files:**
- Modify: `crates/core/src/site/mod.rs` (`card_html` badge when `p.draft`; `page_chrome` pushes a banner into `includes.before_body` when `page.draft`)
- Modify: `crates/core/assets/css/site.css` (badge + banner styling)
- Test: `crates/core/src/site/mod.rs` `#[cfg(test)]` (card HTML contains/omits the badge)

**Interfaces:**
- Consumes: `Page.draft` (Task 1).

- [ ] **Step 1: Write the failing test.**

```rust
#[test]
fn listing_card_shows_draft_badge_only_for_drafts() {
    let dir = tmp("card-badge");
    std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
    std::fs::write(dir.join("live.tmd"), "---\ntitle: Live\n---\nx\n").unwrap();
    std::fs::write(dir.join("wip.tmd"), "---\ntitle: WIP\ndraft: true\n---\nx\n").unwrap();
    let site = Site::discover_with(dir, DraftMode::Include);
    let live = site.pages.iter().find(|p| p.rel == "live.tmd").unwrap();
    let wip = site.pages.iter().find(|p| p.rel == "wip.tmd").unwrap();
    assert!(site.card_html(wip, "", false).contains("tali-draft-badge"));
    assert!(!site.card_html(live, "", false).contains("tali-draft-badge"));
}
```

(If `card_html` is private, this test lives in the same module so it can call it.)

- [ ] **Step 2: Run it to verify it fails.**

Run: `cargo test -p taliesin-core listing_card_shows_draft 2>&1 | tail -12`
Expected: FAIL — no `tali-draft-badge` in the output.

- [ ] **Step 3: Add the card badge.**

In `card_html` (`crates/core/src/site/mod.rs:1101`), before the final `format!`, build the badge and inject it at the start of `.tali-card-body`:

```rust
        let draft_badge = if p.draft {
            "<span class=\"tali-draft-badge\">Draft</span>"
        } else {
            ""
        };
```

Change the body of the returned card to lead with it:

```rust
            "<a class=\"tali-card\" href=\"{href}\" data-qmd-src=\"{src}\">{img}\
             <div class=\"tali-card-body\">{draft_badge}{date}<h3 class=\"tali-card-title\">{title}</h3>{desc}{cats}</div></a>",
```

- [ ] **Step 4: Add the page banner (preview-only, via `before_body`).**

In `page_chrome` (`crates/core/src/site/mod.rs:426`), after `let mut includes = self.includes.clone();`, add:

```rust
        // A draft page (only reachable in preview — a built page is never `draft`) gets a
        // quiet top-of-body banner so the author knows it won't publish. Read-only view
        // affordance; no source write-back.
        if page.draft {
            includes.before_body.insert_str(
                0,
                "<div class=\"tali-draft-banner\" role=\"status\">Draft — not published</div>",
            );
        }
```

- [ ] **Step 5: Style the badge + banner.**

In `crates/core/assets/css/site.css`, add (using `--tali-*` tokens only):

```css
.tali-draft-badge {
  display: inline-block;
  font-size: .7rem;
  font-weight: 700;
  letter-spacing: .04em;
  text-transform: uppercase;
  color: var(--tali-bg, #fff);
  background: var(--tali-muted, #888);
  border-radius: 4px;
  padding: .05rem .35rem;
  margin-right: .4rem;
  vertical-align: middle;
}
.tali-draft-banner {
  text-align: center;
  font-size: .8rem;
  font-weight: 600;
  letter-spacing: .03em;
  color: var(--tali-bg, #fff);
  background: var(--tali-muted, #888);
  padding: .3rem .6rem;
}
```

- [ ] **Step 6: Run the test + `cargo build` (CSS is `include_str!`-compiled).**

Run: `cargo test -p taliesin-core listing_card_shows_draft 2>&1 | tail -8` → PASS.
Run: `cargo build -p taliesin-server 2>&1 | tail -3` → builds clean (so the new CSS is embedded for the later browser check).

- [ ] **Step 7: Commit.**

```bash
git add crates/core/src/site/mod.rs crates/core/assets/css/site.css
git commit -m "feat(site): DRAFT badge on listing cards + a quiet draft page banner"
```

---

### Task 6: Dev-menu draft count/list (server inject + client render)

The dev menu is built client-side (`client.js`). The server (site preview only) injects `window.TALIESIN_DRAFTS`; the client adds a "Drafts" row that expands to click-to-open links.

**Files:**
- Modify: `crates/server/src/serve_site/mod.rs` (inject `window.TALIESIN_DRAFTS` into the preview page head, from `site.pages.filter(draft)`)
- Modify: `web-client/client.js` (render the dev-menu drafts row from the global)
- Modify: `crates/server/src/serve/mod.rs` (`STATUS_CSS`: a `.tali-dev-drafts` list style)
- Test: `web-client` type-check; browser verification in Task 7.

**Interfaces:**
- Consumes: `Page.draft`, `Page.url`, `Page.title` (Task 1); the existing `#tali-controls` client bootstrap (`client.js:362`).
- Produces: `window.TALIESIN_DRAFTS: Array<{url: string, title: string}>` on site-preview pages.

- [ ] **Step 1: Inject the drafts global (server, preview-only).**

In `crates/server/src/serve_site/mod.rs`, where a page's HTML is assembled for preview (the same place `#tali-controls` / preview head JS is added), compute and inject:

```rust
    // Preview-only: the draft list powers the dev-menu "Drafts" row. A built page never
    // carries this (build doesn't emit the dev menu and has no draft pages).
    let drafts_json: String = {
        let items: Vec<String> = site
            .pages
            .iter()
            .filter(|p| p.draft)
            .map(|p| format!(
                "{{\"url\":\"{}\",\"title\":\"{}\"}}",
                crate::serve::js_str(&p.url),
                crate::serve::js_str(p.title.as_deref().unwrap_or(&p.rel)),
            ))
            .collect();
        format!("<script>window.TALIESIN_DRAFTS=[{}];</script>", items.join(","))
    };
```

Inject `drafts_json` into the page head alongside the existing preview-only head injection. `crate::serve::js_str` is the existing `pub(crate)` escaper (it neutralizes `"`, `\`, `<`, newlines, U+2028/9 — sufficient for a URL/title inside a JSON string in a `<script>`); do not hand-roll a new one. If the injection point renders one page at a time from `site`, this is a per-page constant string.

- [ ] **Step 2: Write the client render (failing type-check first if it references undeclared globals).**

In `web-client/client.js`, in the dev-menu builder (near where rows like word-count are appended, ~`client.js:360-410`), add a drafts row driven by the global:

```js
    // Draft pages (preview only): a count that expands to click-to-open links. The server
    // sets window.TALIESIN_DRAFTS on site previews; absent/empty on single-doc + builds.
    var drafts = /** @type {Array<{url:string,title:string}>} */ (window.TALIESIN_DRAFTS || []);
    if (drafts.length) {
      var draftRow = document.createElement("div");
      draftRow.className = "tali-dev-row";
      var label = document.createElement("span");
      label.className = "tali-dev-label";
      label.textContent = drafts.length === 1 ? "1 draft" : drafts.length + " drafts";
      var list = document.createElement("div");
      list.className = "tali-dev-drafts";
      drafts.forEach(function (d) {
        var a = document.createElement("a");
        a.href = d.url;
        a.textContent = d.title;
        list.appendChild(a);
      });
      draftRow.appendChild(label);
      panel.appendChild(draftRow);
      panel.appendChild(list);
    }
```

(Place it after the word-count row is appended to `panel`; `panel` is the `tali-dev-panel` element created in the same function.)

- [ ] **Step 3: Add the drafts-list CSS.**

In `crates/server/src/serve/mod.rs` `STATUS_CSS`, add a rule (append inside the string):

```
    .tali-dev-drafts { display: flex; flex-direction: column; gap: .2rem; } \
    .tali-dev-drafts a { color: var(--tali-accent, #4c8dff); text-decoration: none; font-size: 12px; } \
    .tali-dev-drafts a:hover { text-decoration: underline; } \
```

- [ ] **Step 4: Type-check the client + build.**

Run: `cd web-client && npx -y -p typescript tsc -p jsconfig.json 2>&1 | tail -15`
Expected: no new errors (the `window.TALIESIN_DRAFTS` cast keeps it typed).
Run: `cd /home/bogo/Documents/personal/taliesin && cargo build -p taliesin-server 2>&1 | tail -3` → clean (embeds the new JS/CSS).

- [ ] **Step 5: Commit.**

```bash
git add crates/server/src/serve_site/mod.rs web-client/client.js crates/server/src/serve/mod.rs
git commit -m "feat(preview): dev-menu drafts count + click-to-open list"
```

---

### Task 7: Corpus assertions + browser verification

Lock the behaviour with corpus-level assertions and confirm the real browser + build behaviour.

**Files:**
- Modify: `crates/core/tests/` (a corpus assertion: draft present in Include discovery, absent from a build tree) — pick the existing suite that already discovers `corpus/tech-blog` (e.g. `tech_blog.rs`) and mirror its style.
- Verify: chrome-devtools MCP (preview) + a real `build`.

- [ ] **Step 1: Add the corpus assertion.**

In the tech-blog test suite (the one that calls `Site::discover` on `corpus/tech-blog`), add:

```rust
#[test]
fn tech_blog_draft_is_preview_only() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/tech-blog");
    let published = taliesin_core::Site::discover(&root);
    assert!(!published.pages.iter().any(|p| p.rel.contains("draft-example")),
        "the draft post must be absent from the published set");
    assert!(published.excluded_drafts.iter().any(|d| d.contains("draft-example")));

    let preview = taliesin_core::Site::discover_with(&root, taliesin_core::DraftMode::Include);
    let d = preview.pages.iter().find(|p| p.rel.contains("draft-example")).expect("draft present in preview");
    assert!(d.draft);
}
```

- [ ] **Step 2: Run the corpus test + full suite.**

Run: `cargo test -p taliesin-core 2>&1 | tail -20`
Expected: PASS (all groups). If a body-snapshot test drifted because of a `{js}`/listing change, regenerate per its blessed-file convention only if the diff is the expected draft-badge addition and nothing else.

- [ ] **Step 3: Browser-verify preview.**

Start: `cargo run -p taliesin-server -- preview corpus/tech-blog 4388 &` (note the pid from `ss -ltnp | grep 4388` to stop it later — do NOT `pkill -f 'taliesin preview'`).
Via chrome-devtools MCP: navigate to `http://localhost:4388/blog.html` (the listing) and assert the draft card shows a `Draft` badge; open the dev menu (bottom-left) and confirm it shows "1 draft" with a link; navigate to the draft post URL and confirm the top "Draft — not published" banner. Screenshot each. Check the console for zero errors.

- [ ] **Step 4: Browser-verify the build excludes it.**

Run: `cargo run -p taliesin-server -- build corpus/tech-blog --out /tmp/tb-out 2>&1 | grep -i draft` → `1 draft not published: posts/draft-example/index.tmd`.
Run: `test ! -e /tmp/tb-out/posts/draft-example/index.html && echo "ABSENT (correct)"`.
Confirm the built `blog.html` listing contains no `tali-draft-badge` (`grep -c tali-draft-badge /tmp/tb-out/blog.html` → 0).

- [ ] **Step 5: Stop the preview server + commit.**

Stop the preview by the pid captured in Step 3. Then:

```bash
git add crates/core/tests
git commit -m "test(site): pin draft-aware preview (preview shows, build excludes)"
```

---

## Verification (whole feature)

- `cargo test -p taliesin-core` + `cargo test -p taliesin-server` green.
- `cd web-client && npx -y -p typescript tsc -p jsconfig.json` clean.
- `cargo fmt --check` clean (the save hook keeps it so).
- Browser: preview shows the draft inline (listing badge + dev-menu + page banner, zero console errors); build excludes it and logs the report.
- Then: adversarial review (rust-reviewer + corpus-verifier), re-read the diff, and — only when asked — fast-forward merge `draft-aware-preview` into local `main`; delete the §A#7 backlog entry and the stale §B (publish hardening, already shipped).

## Post-implementation bookkeeping

- Remove backlog §A item 7 (`notes/backlog.md`) and the whole §B block (items 15-17 already shipped by the author — backlog rot).
- Update the `deck-redesign-direction` / project-status memory only if the git state materially changes (author push).
