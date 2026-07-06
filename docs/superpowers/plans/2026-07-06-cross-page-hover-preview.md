# F2a Cross-Page Hover-Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the hover-preview card fire for cross-page `.tali-xref` links, showing the rendered content of the referenced figure/theorem/table/equation/listing/section from another page.

**Architecture:** Mirror the Cmd-K search-index pattern. `Site::discover` harvests a per-anchor snippet index (anchor → rendered block HTML, asset URLs rebased root-relative) into a new `Site.hover_index_json`, served as `hover-index.js` (`window.TALIESIN_HOVER_INDEX`) in both build (`_site/hover-index.js`) and preview (`GET /hover-index.js`), lazy-loaded client-side via `<script src>` (file://-safe). `12-link-preview.js` gains a cross-page branch that looks the anchor up in that index and injects it into the same singleton card.

**Tech Stack:** Rust (edition 2024, `crates/core` + `crates/server`), vanilla JS (`code-enhance/` fragments, no build step), axum (preview server).

## Global Constraints

- HTML-only output; no new config key (auto-derived affordance).
- Preserve `data-block-id` / `data-sourcepos` block invariants — this feature only *reads* rendered block HTML, never re-emits blocks.
- Keep the client symbol `taliInitLinkPreview` and its `window.__qmdLinkPreview` idempotency guard (corpus test `assembled_page_ships_hover_cards` asserts the symbol ships).
- No `fetch()` for the index — load via `<script src>` so it works from `file://` (mirrors `search.js` `loadIndexThen`).
- Do NOT touch the exec/kernel zone or the single-editing-surface invariant (this is a read-only preview affordance).
- `rustfmt` runs on save (PostToolUse hook); keep the tree `cargo fmt`-clean.
- Same-page hover behavior (`href` starting with `#`) must be byte-for-byte unchanged.

---

### Task 1: `hover.rs` pure helpers (snippet extraction + URL rebasing)

**Files:**
- Create: `crates/core/src/site/hover.rs`
- Test: inline `#[cfg(test)] mod tests` in the same file.

**Interfaces:**
- Produces (used by Task 2):
  - `pub(super) fn extract_snippet(blocks: &[render::Block], anchor: &str) -> Option<String>` — the defining block's HTML (heading anchors also append up to 2 following non-heading, non-`id` blocks), capped at `SNIPPET_CAP` chars.
  - `pub(super) fn rewrite_snippet_urls(html: &str, page_url: &str) -> String` — rebases relative `src=`/`href=` values to site-root-relative (via `links::join_rel` + `.tmd`→`.html`), leaving `#frag`, `data:`, `//`, `scheme://`, `mailto:`, `tel:`, `/abs` untouched.

- [ ] **Step 1: Write failing tests for the two helpers**

Add to `crates/core/src/site/hover.rs` (create the file with just `use super::*;` + `const SNIPPET_CAP: usize = 8000;` + the two `fn` stubs returning `None`/`String::new()` so it compiles, then this test module):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Block;

    fn blk(html: &str) -> Block {
        Block { id: "b".into(), sourcepos: "1:1-1:1".into(), source_file: None, html: html.into(), cell: None }
    }

    #[test]
    fn extract_single_element_block() {
        let blocks = vec![
            blk("<p>before</p>"),
            blk("<figure id=\"fig-x\" class=\"tali-figure\"><img src=\"p.png\"><figcaption>Figure&nbsp;3</figcaption></figure>"),
            blk("<p>after</p>"),
        ];
        let s = extract_snippet(&blocks, "fig-x").unwrap();
        assert!(s.contains("id=\"fig-x\"") && s.contains("Figure&nbsp;3"));
        assert!(!s.contains("before") && !s.contains("after"), "a figure is a single-element snippet");
    }

    #[test]
    fn extract_heading_takes_following_blocks_until_next_heading_or_id() {
        let blocks = vec![
            blk("<h2 id=\"sec-m\">Methods</h2>"),
            blk("<p>intro one</p>"),
            blk("<p>intro two</p>"),
            blk("<h2 id=\"sec-n\">Next</h2>"),
        ];
        let s = extract_snippet(&blocks, "sec-m").unwrap();
        assert!(s.contains("Methods") && s.contains("intro one") && s.contains("intro two"));
        assert!(!s.contains("Next"), "stops at the next heading");
    }

    #[test]
    fn extract_heading_caps_at_two_following_blocks() {
        let blocks = vec![
            blk("<h2 id=\"sec-m\">Methods</h2>"),
            blk("<p>one</p>"), blk("<p>two</p>"), blk("<p>three</p>"),
        ];
        let s = extract_snippet(&blocks, "sec-m").unwrap();
        assert!(s.contains("one") && s.contains("two") && !s.contains("three"));
    }

    #[test]
    fn extract_returns_none_for_unknown_anchor() {
        assert!(extract_snippet(&[blk("<p>x</p>")], "fig-x").is_none());
    }

    #[test]
    fn rewrite_rebases_relative_asset_from_nested_page() {
        let html = "<img src=\"figs/p.png\"><a href=\"other.tmd#s\">o</a>";
        let out = rewrite_snippet_urls(html, "ch/methods.html");
        assert!(out.contains("src=\"ch/figs/p.png\""), "img rebased to root-relative: {out}");
        assert!(out.contains("href=\"ch/other.html#s\""), ".tmd→.html + rebased + frag kept: {out}");
    }

    #[test]
    fn rewrite_leaves_absolute_external_data_and_anchor_untouched() {
        let html = "<img src=\"https://x/y.png\"><img src=\"data:image/png;base64,AA\"><a href=\"#top\">t</a><img src=\"/root.png\">";
        let out = rewrite_snippet_urls(html, "ch/methods.html");
        assert!(out.contains("src=\"https://x/y.png\""));
        assert!(out.contains("src=\"data:image/png;base64,AA\""));
        assert!(out.contains("href=\"#top\""));
        assert!(out.contains("src=\"root.png\""), "site-absolute /x becomes root-relative x: {out}");
    }

    #[test]
    fn rewrite_root_page_leaves_relative_as_is() {
        let out = rewrite_snippet_urls("<img src=\"p.png\">", "methods.html");
        assert!(out.contains("src=\"p.png\""));
    }
}
```

- [ ] **Step 2: Run the tests, verify they fail**

Run: `cargo test -p taliesin-core hover:: 2>&1 | tail -20`
Expected: FAIL (assertions fail / stubs return empty).

- [ ] **Step 3: Implement the helpers**

Replace the stubs in `crates/core/src/site/hover.rs`:

```rust
//! Cross-page hover-preview snippet index: anchor → the rendered HTML of the block
//! that defines it (figure / theorem / table / equation / listing / section), with
//! relative asset URLs rebased site-root-relative. Built once at discovery, served as
//! `hover-index.js` and lazy-loaded by `12-link-preview.js` when a reader hovers a
//! cross-page `.tali-xref`. `use super::*` reaches Site/Page/links/render.

use super::*;

/// One snippet is capped so a giant figure/table can't blow up the index.
const SNIPPET_CAP: usize = 8000;

/// Whether a block's leading element tag is a heading (`<h1..6`).
fn is_heading(html: &str) -> bool {
    let t = html.trim_start().as_bytes();
    t.len() >= 3 && t[0] == b'<' && t[1] == b'h' && (b'1'..=b'6').contains(&t[2])
}

/// Whether a block's leading element tag carries any `id="…"` (mirrors the client's
/// `!n.id` stop condition when gathering a heading's following blocks).
fn leading_tag_has_id(html: &str) -> bool {
    match crate::render::tag_end(html) {
        Some(gt) => html[..gt].contains("id=\""),
        None => html.contains("id=\""),
    }
}

/// Truncate to at most `SNIPPET_CAP` chars on a char boundary.
fn cap(mut s: String) -> String {
    if let Some((i, _)) = s.char_indices().nth(SNIPPET_CAP) {
        s.truncate(i);
    }
    s
}

/// The rendered HTML for `anchor`'s defining block. A heading anchor also appends up
/// to two following blocks (stopping at the next heading or a block with its own id),
/// matching the same-page card's "heading + up to 2 siblings" behavior.
pub(super) fn extract_snippet(blocks: &[render::Block], anchor: &str) -> Option<String> {
    let bi = blocks
        .iter()
        .position(|b| links::block_tag_has_id(&b.html, anchor))
        .or_else(|| {
            let needle = format!("id=\"{anchor}\"");
            blocks.iter().position(|b| b.html.contains(&needle))
        })?;
    let mut out = blocks[bi].html.clone();
    if is_heading(&blocks[bi].html) {
        let mut added = 0;
        for b in &blocks[bi + 1..] {
            if added >= 2 || is_heading(&b.html) || leading_tag_has_id(&b.html) {
                break;
            }
            out.push_str(&b.html);
            added += 1;
        }
    }
    Some(cap(out))
}

/// Rebase relative `src=`/`href=` values in a snippet to site-root-relative, so the
/// snippet renders correctly in a card shown on a page at any depth. `page_url` is the
/// defining page's url (e.g. `ch/methods.html`). External/absolute/data/anchor URLs are
/// left untouched; `.tmd` path components map to `.html`; a `#fragment` is preserved.
pub(super) fn rewrite_snippet_urls(html: &str, page_url: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    // Rewrite both attributes in one pass, whichever comes first.
    loop {
        let src = rest.find("src=\"").map(|p| (p, 5usize));
        let href = rest.find("href=\"").map(|p| (p, 6usize));
        let next = match (src, href) {
            (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some((pos, kw)) = next else {
            out.push_str(rest);
            break;
        };
        let val_start = pos + kw;
        out.push_str(&rest[..val_start]);
        let after = &rest[val_start..];
        let Some(end) = after.find('"') else {
            out.push_str(after);
            break;
        };
        out.push_str(&rebase_url(&after[..end], page_url));
        out.push('"');
        rest = &after[end + 1..];
    }
    out
}

/// Root-relative rebase of one attribute value; skips external/absolute/data/anchor.
fn rebase_url(val: &str, page_url: &str) -> String {
    if val.is_empty()
        || val.starts_with('#')
        || val.starts_with("//")
        || val.contains("://")
        || val.starts_with("data:")
        || val.starts_with("mailto:")
        || val.starts_with("tel:")
        || val.starts_with("vscode:")
    {
        return val.to_string();
    }
    let (path, frag) = match val.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (val, None),
    };
    // Site-absolute (`/x`) → root-relative (`x`); relative → resolved against the
    // defining page's directory. `.tmd`→`.html` on the path either way.
    let mapped = links::qmd_to_html(path.trim_start_matches('/'));
    let rooted = if path.starts_with('/') {
        mapped
    } else {
        links::join_rel(page_url, &mapped)
    };
    match frag {
        Some(f) => format!("{rooted}#{f}"),
        None => rooted,
    }
}
```

Note: `extract_snippet` takes `&[render::Block]`; confirm `render::Block` and `render::tag_end` are reachable (`tag_end` is `pub(crate)` in `render/mod.rs`; `links::block_tag_has_id`/`join_rel`/`qmd_to_html` are `pub(super)` in `site/links.rs`, reachable via `use super::*`). If `render::Block` isn't already re-exported for `site`, use the path the sibling modules use (`render::Block` is used widely in `site/`).

- [ ] **Step 4: Run the tests, verify they pass**

Run: `cargo test -p taliesin-core hover:: 2>&1 | tail -20`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/site/hover.rs
git commit -m "feat(site): hover-preview snippet extraction + URL rebasing helpers"
```

---

### Task 2: `Site::build_hover_index` + `hover_index_json` field, built in `discover`

**Files:**
- Modify: `crates/core/src/site/mod.rs` (add `mod hover;`, the field, the constructor init + `build_hover_index()` call, the method)
- Test: `crates/core/tests/corpus.rs` (integration test against `demo-book`)

**Interfaces:**
- Consumes: `hover::extract_snippet`, `hover::rewrite_snippet_urls` (Task 1); `self.pages`, `self.xref_targets`, `self.chapter_for(page)`, `render::render_document_with_includes_scoped`.
- Produces (used by Tasks 3): `Site.hover_index_json: String` — a JSON object `{"<anchor>":"<snippet html>", …}`, or empty when there are no xref targets.

- [ ] **Step 1: Write the failing integration test**

Add to `crates/core/tests/corpus.rs`:

```rust
#[test]
fn demo_book_hover_index_has_cross_page_snippets() {
    use taliesin_core::Site;
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let idx = &site.hover_index_json;
    // The theorem defined on methods.tmd is in the index with its scoped label…
    assert!(
        idx.contains("\"thm-kl\":\""),
        "hover index missing thm-kl: {idx}"
    );
    assert!(
        idx.contains("Theorem"),
        "theorem snippet should carry its rendered label: {idx}"
    );
    // …as are the two section anchors referenced across chapters.
    assert!(idx.contains("\"sec-methods\":\""), "missing sec-methods: {idx}");
    assert!(idx.contains("\"sec-setup\":\""), "missing sec-setup: {idx}");
    // The index is valid JSON (parses) and `</script>` can't break the <script> wrapper.
    assert!(!idx.contains("</script"), "raw </script must be neutralized");
}
```

- [ ] **Step 2: Run it, verify it fails to compile**

Run: `cargo test -p taliesin-core --test corpus demo_book_hover_index 2>&1 | tail -20`
Expected: FAIL — no field `hover_index_json` on `Site`.

- [ ] **Step 3: Add the module, field, and method**

In `crates/core/src/site/mod.rs`:

1. Register the module next to `mod search;` (~line 156):
```rust
mod hover;
mod search;
```

2. Add the struct field after `search_index_json` (~line 144):
```rust
    /// Inlinable JSON of every cross-reference anchor → the rendered HTML of the block
    /// that defines it, so hovering a CROSS-PAGE `.tali-xref` previews its target
    /// (`window.TALIESIN_HOVER_INDEX`). Built once at discovery; served as
    /// `hover-index.js` and lazy-loaded by `12-link-preview.js`. Empty when the project
    /// has no cross-reference targets.
    pub hover_index_json: String,
```

3. In the `Site { … }` constructor literal (~line 279-289) add the field init and, right after the literal, populate it before returning. Change the tail of `discover` from:
```rust
        Site {
            root: root.to_path_buf(),
            config,
            pages,
            book,
            xref_targets,
            includes,
            warnings,
            search_index_json,
            decks,
        }
```
to:
```rust
        let mut site = Site {
            root: root.to_path_buf(),
            config,
            pages,
            book,
            xref_targets,
            includes,
            warnings,
            search_index_json,
            decks,
            hover_index_json: String::new(),
        };
        site.build_hover_index();
        site
```

4. Add the method (put it next to `harvest_xref_numbers`, ~line 696, so the two render-harvest passes read together):
```rust
    /// Discovery render-harvest: render each page that defines cross-reference targets
    /// once (scoped to its chapter, like `harvest_xref_numbers`) and capture, per anchor,
    /// the rendered HTML of its defining block — the snippet the cross-page hover-preview
    /// card shows. Relative asset URLs are rebased site-root-relative so the snippet
    /// renders correctly on any viewing page. Runs inside `discover`, so the index is
    /// always populated (build, preview, and after a preview structural rebuild) with no
    /// extra call site. `hover::` does the per-anchor extraction + URL rebasing.
    fn build_hover_index(&mut self) {
        if self.xref_targets.is_empty() {
            return;
        }
        // anchors grouped by their defining page's url, so each page renders at most once.
        let mut by_page: std::collections::HashMap<&str, Vec<&str>> = HashMap::new();
        for (anchor, t) in &self.xref_targets {
            by_page.entry(t.url.as_str()).or_default().push(anchor.as_str());
        }
        let mut entries: Vec<(String, String)> = Vec::new();
        for page in &self.pages {
            let Some(anchors) = by_page.get(page.url.as_str()) else {
                continue;
            };
            let Ok(src) = std::fs::read_to_string(&page.input) else {
                continue;
            };
            let base = page.input.parent().unwrap_or(&self.root);
            let doc =
                render::render_document_with_includes_scoped(&src, base, self.chapter_for(page));
            for anchor in anchors {
                if let Some(snippet) = hover::extract_snippet(&doc.blocks, anchor) {
                    let snippet = hover::rewrite_snippet_urls(&snippet, &page.url);
                    entries.push((anchor.to_string(), snippet));
                }
            }
        }
        if entries.is_empty() {
            return;
        }
        // Stable order so the index is deterministic across builds.
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut out = String::from("{");
        for (i, (anchor, snippet)) in entries.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "\"{}\":\"{}\"",
                search::json_str(anchor),
                search::json_str(snippet)
            ));
        }
        out.push('}');
        self.hover_index_json = out;
    }
```

Note: reuse `search::json_str` for escaping (already `pub(super)`; neutralizes `</script>` via `<`). `HashMap` is already imported in `mod.rs` (used by `xref_targets`).

- [ ] **Step 4: Run the integration test, verify it passes**

Run: `cargo test -p taliesin-core --test corpus demo_book_hover_index 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Run the full core test suite (nothing regressed)**

Run: `cargo test -p taliesin-core 2>&1 | tail -25`
Expected: PASS (all existing tests + the new one).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/site/mod.rs crates/core/tests/corpus.rs
git commit -m "feat(site): build the cross-page hover snippet index at discovery"
```

---

### Task 3: Serve the index (preview route + build write + mounts) and inject the pointer

**Files:**
- Modify: `crates/core/src/site/mod.rs` (`page_chrome`: inject `TALIESIN_SITE_ROOT` + `TALIESIN_HOVER_URL` into `includes.in_header` on every page)
- Modify: `crates/server/src/serve_site/mod.rs` (route `/hover-index.js` + handler + mounted fallback branch)
- Modify: `crates/server/src/build.rs` (write `_site/hover-index.js`)
- Test: `crates/core/tests/corpus.rs` (page_chrome pointer present; index not inlined)

**Interfaces:**
- Consumes: `Site.hover_index_json` (Task 2).
- Produces: `window.TALIESIN_HOVER_URL` + `window.TALIESIN_SITE_ROOT` globals on every site page (Task 4 reads them); a `hover-index.js` served in preview and written on build.

- [ ] **Step 1: Write the failing test for the page pointer**

Add to `crates/core/tests/corpus.rs`:

```rust
#[test]
fn demo_book_pages_point_at_hover_index_without_inlining_it() {
    use taliesin_core::Site;
    let site = Site::discover(&corpus_dir().join("demo-book"));
    // results.tmd has cross-page refs but no TOC — the hover pointer must still ship.
    let results = site.render_page("results.tmd").expect("results renders");
    assert!(
        results.contains("window.TALIESIN_HOVER_URL="),
        "every page needs the hover-index pointer: {results}"
    );
    assert!(
        results.contains("window.TALIESIN_SITE_ROOT="),
        "hover needs the site root to resolve rebased asset URLs"
    );
    // The (potentially large) index itself is lazy-loaded, never inlined into a page.
    assert!(
        !results.contains("window.TALIESIN_HOVER_INDEX="),
        "the index must not be inlined into the page body"
    );
}
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cargo test -p taliesin-core --test corpus demo_book_pages_point 2>&1 | tail -20`
Expected: FAIL — no `TALIESIN_HOVER_URL` in output.

- [ ] **Step 3: Inject the pointer in `page_chrome`**

In `crates/core/src/site/mod.rs` `page_chrome` (after the `search_index` block, ~line 362, before the `SiteCtx { … }` literal). `depth` is already computed at the top of the fn:
```rust
        // Cross-page hover-preview: point every page at the lazy hover-index.js and set the
        // site root so the client can resolve a snippet's rebased (root-relative) asset URLs.
        // Injected into the always-emitted head (unlike search, which rides only TOC pages)
        // because a cross-page ref can appear on any page. Idempotent with search's own
        // TALIESIN_SITE_ROOT on TOC pages (same value).
        let has_hover = !self.hover_index_json.is_empty();
        if has_hover {
            let up = "../".repeat(depth);
            includes.in_header.push_str(&format!(
                "<script>window.TALIESIN_SITE_ROOT=\"{up}\";\
                 window.TALIESIN_HOVER_URL=\"{up}hover-index.js\";</script>"
            ));
        }
```

- [ ] **Step 4: Run the pointer test, verify it passes**

Run: `cargo test -p taliesin-core --test corpus demo_book_pages_point 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Add the preview route + handler + mounted fallback**

In `crates/server/src/serve_site/mod.rs`:

1. Add the route next to `/search-index.js` (~line 166):
```rust
        .route("/hover-index.js", get(hover_index_js))
```

2. Add the handler next to `search_index_js` (~line 242):
```rust
/// The cross-page hover-preview snippet index as a `hover-index.js` script (assigns
/// `window.TALIESIN_HOVER_INDEX`), lazy-loaded by `12-link-preview.js` on the first
/// cross-page hover. Served as JS (not JSON) so a `<script>` load works under file://.
async fn hover_index_js(State(app): State<Arc<SiteApp>>) -> impl IntoResponse {
    let json = { app.site.lock().hover_index_json.clone() };
    let json = if json.is_empty() { "{}".to_string() } else { json };
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/javascript; charset=utf-8",
        )],
        format!("window.TALIESIN_HOVER_INDEX={json};"),
    )
        .into_response()
}
```

3. Add the mounted-sub-site branch next to the `search-index.js` branch (~line 298-304):
```rust
            if lookup == "hover-index.js" {
                let j = m.site.hover_index_json.clone();
                let j = if j.is_empty() { "{}".to_string() } else { j };
                let js_ct = "text/javascript; charset=utf-8";
                let body = format!("window.TALIESIN_HOVER_INDEX={j};");
                return ([(axum::http::header::CONTENT_TYPE, js_ct)], body).into_response();
            }
```

- [ ] **Step 6: Add the build write**

In `crates/server/src/build.rs`, after the `search-index.js` write block (~line 1002):
```rust
    // Cross-page hover-preview snippet index, lazy-loaded by 12-link-preview.js (pages
    // point at it via window.TALIESIN_HOVER_URL). Same file:// rationale as search-index.js.
    if !site.hover_index_json.is_empty() {
        let js = format!("window.TALIESIN_HOVER_INDEX={};", site.hover_index_json);
        if let Err(e) = std::fs::write(out.join("hover-index.js"), js) {
            log::warn(&format!("cannot write hover-index.js: {e}"));
        }
    }
```

- [ ] **Step 7: Build the server + run the core suite**

Run: `cargo build -p taliesin-server 2>&1 | tail -15 && cargo test -p taliesin-core --test corpus demo_book 2>&1 | tail -20`
Expected: server builds; demo_book tests PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/core/src/site/mod.rs crates/server/src/serve_site/mod.rs crates/server/src/build.rs crates/core/tests/corpus.rs
git commit -m "feat(server): serve hover-index.js in preview + build, wire the page pointer"
```

---

### Task 4: Extend `12-link-preview.js` with the cross-page branch

**Files:**
- Modify: `crates/core/assets/js/code-enhance/12-link-preview.js`
- Test: existing corpus guard `assembled_page_ships_hover_cards` (stays green); browser verification is Task 5.

**Interfaces:**
- Consumes: `window.TALIESIN_HOVER_URL`, `window.TALIESIN_HOVER_INDEX`, `window.TALIESIN_SITE_ROOT` (Task 3); shared `taliCloneStripped` (registry).
- Produces: cross-page hover behavior in the shared `#tali-link-preview` card.

- [ ] **Step 1: Rewrite the eligibility, delegation, show, and lazy-load**

Apply these edits to `crates/core/assets/js/code-enhance/12-link-preview.js` (keep `taliInitLinkPreview` + `window.__qmdLinkPreview` + the style/card setup unchanged):

Replace `eligible` + add a cross-page classifier and `lastHovered` state. Change:
```js
  var showTimer = null, hideTimer = null, pinned = false, currentLink = null;

  function eligible(a) {
    if (!a) return false;
    var href = a.getAttribute('href') || '';
    if (href.charAt(0) !== '#' || href.length < 2) return false;
    return !a.closest('#TOC') && !a.closest('#tali-link-preview');
  }
```
to:
```js
  var showTimer = null, hideTimer = null, pinned = false, currentLink = null, lastHovered = null;

  // Same-page target: an in-page fragment link (existing behavior).
  function eligibleSame(a) {
    if (!a) return false;
    var href = a.getAttribute('href') || '';
    if (href.charAt(0) !== '#' || href.length < 2) return false;
    return !a.closest('#TOC') && !a.closest('#tali-link-preview');
  }
  // Cross-page target: a resolved cross-reference to another page — a `.tali-xref`
  // whose href is `page.html#anchor` (not a bare `#frag`). Its target lives in a
  // different document, so it's previewed from the served hover index, not the DOM.
  function eligibleCross(a) {
    if (!a || !a.classList.contains('tali-xref')) return false;
    var href = a.getAttribute('href') || '';
    if (href.charAt(0) === '#' || href.indexOf('#') < 0) return false;
    return !a.closest('#TOC') && !a.closest('#tali-link-preview');
  }
  function eligible(a) { return eligibleSame(a) || eligibleCross(a); }
```

Replace the `show` function to branch:
```js
  function show(link) {
    if (eligibleCross(link)) { showCross(link); return; }
    var id = decodeURIComponent((link.getAttribute('href') || '').slice(1));
    var target = id && document.getElementById(id);
    if (!target) return;
    var body = buildPreview(target);
    if (!body || !body.textContent.trim()) return;
    card.innerHTML = '';
    card.appendChild(body);
    currentLink = link;
    card.classList.add('open');
    place(link);
  }
```

Add the cross-page renderer + lazy loader after `show` (before `scheduleShow`):
```js
  // Lazy-load the served hover index on the first cross-page hover (a <script> load, so
  // it works under file:// like search-index.js), then render if still hovering the link.
  var hoverFetched = false;
  function loadHoverThen(cb) {
    if (window.TALIESIN_HOVER_INDEX || !window.TALIESIN_HOVER_URL || hoverFetched) { cb(); return; }
    hoverFetched = true;
    var s = document.createElement('script');
    s.src = window.TALIESIN_HOVER_URL;
    s.onload = cb;
    s.onerror = cb;
    document.head.appendChild(s);
  }
  // Root-relative asset URLs in a snippet resolve against the site root; prefix them with
  // TALIESIN_SITE_ROOT (this page's up-path to root) so an image from another page loads.
  function resolveUrls(container) {
    var root = window.TALIESIN_SITE_ROOT || '';
    if (!root) return;
    function abs(v) {
      return !v || v.charAt(0) === '#' || v.charAt(0) === '/' ||
        v.indexOf('//') === 0 || v.indexOf('://') > -1 ||
        v.indexOf('data:') === 0 || v.indexOf('mailto:') === 0 || v.indexOf('tel:') === 0;
    }
    container.querySelectorAll('img[src]').forEach(function (n) {
      var v = n.getAttribute('src'); if (!abs(v)) n.setAttribute('src', root + v);
    });
    container.querySelectorAll('a[href]').forEach(function (n) {
      var v = n.getAttribute('href'); if (!abs(v)) n.setAttribute('href', root + v);
    });
  }
  function showCross(link) {
    loadHoverThen(function () {
      if (lastHovered !== link) return; // pointer moved away while the index loaded
      var href = link.getAttribute('href') || '';
      var anchor = decodeURIComponent(href.slice(href.indexOf('#') + 1));
      var snippet = (window.TALIESIN_HOVER_INDEX || {})[anchor];
      if (!snippet) return;
      // Parse inertly in a <template> (its images don't load until adopted), rebase URLs,
      // strip interactive chrome, then adopt into the card.
      var tpl = document.createElement('template');
      tpl.innerHTML = snippet;
      resolveUrls(tpl.content);
      tpl.content.querySelectorAll('.tali-anchor, .tali-copy').forEach(function (n) { n.remove(); });
      if (!tpl.content.textContent.trim()) return;
      card.innerHTML = '';
      card.appendChild(tpl.content);
      currentLink = link;
      card.classList.add('open');
      place(link);
    });
  }
```

Update the delegation listeners to track `lastHovered` and match any anchor (not only `a[href^='#']`):
```js
  document.addEventListener('mouseover', function (e) {
    var a = e.target.closest && e.target.closest('a[href]');
    if (a && eligible(a)) { lastHovered = a; scheduleShow(a); }
  });
  document.addEventListener('mouseout', function (e) {
    var a = e.target.closest && e.target.closest('a[href]');
    if (a && eligible(a)) {
      var to = e.relatedTarget;
      if (to && to.closest && to.closest('#tali-link-preview')) return; // moving into the card
      lastHovered = null;
      scheduleHide();
    }
  });
```

- [ ] **Step 2: Type-check the client (no build step; tsc catches syntax/type slips)**

Run: `cd web-client && npx -y -p typescript tsc -p jsconfig.json 2>&1 | tail -20`
Expected: no NEW errors from `12-link-preview.js` (note: code-enhance fragments may not all be in the tsc project; if this file isn't type-checked, verify it loads clean in Task 5's browser console instead).

- [ ] **Step 3: Confirm the corpus guard still ships the symbol**

Run: `cargo test -p taliesin-core assembled_page_ships_hover_cards 2>&1 | tail -10`
Expected: PASS (symbol `taliInitLinkPreview` still present).

- [ ] **Step 4: Commit**

```bash
git add crates/core/assets/js/code-enhance/12-link-preview.js
git commit -m "feat(client): cross-page hover-preview branch in 12-link-preview.js"
```

---

### Task 5: Browser verification + backlog/memory update

**Files:**
- Modify: `notes/backlog.md` (remove the shipped F2a item; note cross-page-theorem-number item still open)
- (No code changes; this task verifies the feature live.)

- [ ] **Step 1: Preview the demo-book corpus and drive the browser**

Serve: `cargo run -p taliesin-server -- preview crates/core/tests/corpus/demo-book <port>` (background), then via chrome-devtools MCP:
- Navigate to `results.html` (has the `@sec-methods` / `@sec-setup` / `@thm-kl` cross-page refs).
- Hover the "Chapter 2" (`methods.html#sec-methods`) link → assert the card opens with the Methods section content.
- Hover the "Theorem" (`methods.html#thm-kl`) link → assert the card shows the theorem statement with its label.
- Check the console: 0 errors; confirm `hover-index.js` loaded once (network) on the first cross-page hover.
- Repeat at three viewports (~390×844, ~1440×900, ~900×1440) per the UI testing matrix.

- [ ] **Step 2: Verify an image figure resolves cross-page (scratchpad fixture)**

Build a throwaway 2-page site in the scratchpad: page A with `![cap](plot.png){#fig-x}` + a tiny `plot.png`; page B with `@fig-x`. Preview it, hover the ref on page B, confirm the figure image renders in the card (not a broken image). This exercises the asset-URL rebasing end-to-end (demo-book has no image figure).

- [ ] **Step 3: Update the backlog**

In `notes/backlog.md`, remove the "F2a: hover preview for cross-page refs" bullet from Tier 1. Leave "Cross-page theorem refs drop the number" (a distinct item, though now the hover CARD shows correct numbers — the LINK TEXT fix is still separate). Optionally note the shipped feature under "Recently shipped".

- [ ] **Step 4: Final full verification**

Run: `cargo test -p taliesin-core 2>&1 | tail -20 && cargo build -p taliesin-server 2>&1 | tail -5`
Expected: all green.

- [ ] **Step 5: Commit the backlog update**

```bash
git add notes/backlog.md
git commit -m "docs(backlog): ship F2a cross-page hover-preview"
```

---

## Self-Review

**Spec coverage:** Snippet index (Task 2) ✓; block-level capture + heading siblings (Task 1) ✓; clean `.tali-anchor`/`.tali-copy` — moved to the client `showCross` (single source of cleaning truth) ✓; serve like search-index.js in preview+build+mounts (Task 3) ✓; `file://`-safe `<script>` load (Task 4) ✓; asset-URL rebasing server-side + client `SITE_ROOT` prefix (Tasks 1/4) ✓; no new config key ✓; a11y untouched ✓; corpus pin (Tasks 2/5) ✓; staleness = search parity (built in discover, rebuilt with it) ✓.

**Deviation from spec:** cleaning (`.tali-anchor`/`.tali-copy` strip) is done client-side in `showCross` rather than server-side at harvest — this reuses the existing client cleaning idiom and avoids fragile server-side HTML surgery. Snippet HTML is stored raw. Equivalent result, simpler server.

**Placeholder scan:** none — every code step has complete code.

**Type/name consistency:** `extract_snippet`/`rewrite_snippet_urls` (Task 1) match their calls in `build_hover_index` (Task 2); `hover_index_json` field name consistent across Tasks 2/3; `TALIESIN_HOVER_URL`/`TALIESIN_HOVER_INDEX`/`TALIESIN_SITE_ROOT` consistent across Tasks 3/4; `search::json_str` reused for escaping.

**Risk to verify during execution:** `render::Block` and `render::tag_end` reachability from `site/hover.rs` (both used by sibling `site/` modules already — confirm the exact path when it compiles).
