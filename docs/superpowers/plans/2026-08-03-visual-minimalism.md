# Visual Minimalism Pass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete the T4+T5 reader-facing chrome (2,571 measured lines across 13 files) and trim the construct vocabulary, so a Taliesin page's default state is text rather than text-plus-affordances.

**Architecture:** Pure subtraction. No new configuration keys, no new UI, no replacement mechanisms. Each deletion is (1) a test asserting the feature is gone, (2) removal of the fragment/module plus its registration site, (3) removal of its CSS, (4) removal or amendment of its tests, (5) the drift-gate tail (retired registers, canaries, docs, corpus). Client enhancers are ordered fragments concatenated into one script, so deleting one means deleting the file, its `include_str!` line in `render/mod.rs`, and its `reg.register(...)` line in `09-register.js`.

**Tech Stack:** Rust (edition 2024, workspace resolver 3), vanilla JS (no build step, `// @ts-check`), `.tmd` corpus documents, `tools/gates.sh` as the real verification gate.

## Global Constraints

- **`./tools/gates.sh` is the gate, not `cargo test --workspace`.** Two of the eight drift gates live outside `taliesin-core`, and the pre-push hook does not run browser suites. A green `cargo test` proves nothing about this change.
- **`RETIRED_KEYS` is scoped `(scope, key, note)`.** Never flatten it. The same word may be retired in one vocabulary and live in another.
- **A withdrawn div class REQUIRES a `RETIRED_DIV_CLASSES` entry.** Div classes are an open vocabulary: without one, a leftover class gets silence, not a did-you-mean, and the page quietly loses layout.
- **A retired front-matter key trips EIGHT gates.** Including `docs/guide/using/from-quarto.tmd`, `vocab.rs`'s `descriptions_present`, and `editor/vscode/schema/tali-site.schema.json` (a bundled copy gated only by the companion's own `node --test`).
- **No new config knobs.** "Perfect the default before adding a knob." Nothing here is replaced by an option.
- **Do NOT touch:** `MAX_WARM_PAGES` + the LRU order in `serve_site/exec_pool.rs` (the one standing freeze); the deck engine (`deck.rs`/`deck.js`/`deck.css` — ruled frozen, not cut); `scrolly`, `code-walkthrough`, `{glsl}`, `numerics`, `magic-move` (shipped roadmap items with corpus pins).
- **Block-model invariants hold:** every emitted block keeps `data-block-id` + `data-sourcepos`; included blocks keep `data-source-file`.
- **Editing `assets/css/*` or `assets/js/*` needs a `cargo build`** before the change shows up in a built site — they are `include_str!`-compiled into the binary.
- **Commit style:** one commit per task. Do not push (the author pushes; a push takes ~9 min because the pre-push hook runs the full suite).

## File Structure

**Deleted outright (13 files, 2,571 lines):**

| File | Lines | Task |
|---|---|---|
| `crates/core/assets/js/code-enhance/11-lightbox.js` | 292 | 1 |
| `crates/core/assets/js/code-enhance/15-reading-progress.js` | 141 | 2 |
| `web-client/toc-sheet.js` | 184 | 3 |
| `crates/core/assets/js/code-enhance/12-link-preview.js` | 244 | 4 |
| `crates/core/src/site/hover.rs` | 199 | 4 |
| `crates/core/assets/js/code-enhance/02-anchor-links.js` | 56 | 5 |
| `crates/core/assets/js/code-enhance/18-media.js` | 118 | 6 |
| `crates/core/assets/js/code-enhance/20-code-visibility.js` | 68 | 7 |
| `crates/core/src/site/backlinks.rs` | 477 | 8 |
| `crates/core/src/site/sentences.rs` | 252 | 8 |
| `crates/core/assets/js/code-enhance/10-category-filter.js` | 110 | 9 |
| `crates/core/src/site/categories.rs` | 181 | 9 |
| `crates/core/assets/js/code-enhance/19-book-outline.js` | 249 | 10 |

**Modified in most tasks (the shared registration surface):**
- `crates/core/src/render/mod.rs` — the `CODE_ENHANCE_JS` `concat!` list (~2140) and the `code_scripts_for` doc comments (~2003, ~2012)
- `crates/core/assets/js/code-enhance/09-register.js` — the `reg.register(...)` block
- `crates/core/assets/css/base.css`, `crates/core/assets/css/site.css`
- `crates/core/tests/retired_names.rs` — the home for "this token must not appear" assertions

**Test files deleted:** `crates/core/tests/hover.rs`, `crates/core/tests/backlinks_are_exercised.rs`, `crates/core/tests/xref_backlinks.rs`, `crates/server/tests/reader_chrome_browser.rs` (progressively trimmed, deleted in Task 3).

**Note on the fragment-list guard:** `code_enhance_bundle_matches_fragments_in_order` (`render/tests.rs:3955`) re-reads the `code-enhance/` directory and asserts it matches the `concat!`. It therefore stays green automatically **provided you delete the file and its `include_str!` line in the same commit**. If it fails, you did one and not the other.

---

## Wave 1 — the browser-tested chrome

These three share one test file (`reader_chrome_browser.rs`, 5 tests) and one gates.sh canary, so they are sequenced together. Test allocation:
- Lightbox → `clicking_the_enlarged_image_closes_the_lightbox`, `the_enlarged_image_advertises_zoom_out`, `stepping_the_gallery_does_not_close_the_lightbox`
- Mobile TOC pill → `the_mobile_contents_handle_reads_and_behaves_as_a_toggle`
- Reading progress → `the_reading_bar_is_gone_but_the_resume_position_is_not`

### Task 1: Delete the image/mermaid lightbox and retire its canary

**Files:**
- Delete: `crates/core/assets/js/code-enhance/11-lightbox.js`
- Modify: `crates/core/src/render/mod.rs` (remove the `11-lightbox.js` `include_str!`; update the `code_scripts_for` doc comments at ~2003 and ~2012 which name "lightbox")
- Modify: `crates/core/assets/js/code-enhance/09-register.js` (remove `reg.register(function () { taliInitLightbox(); });`)
- Modify: `tools/gates.sh` (remove the `CANARY_LIGHTBOX` block at lines 88–92 and its `TALIESIN_REQUIRE_*` wiring; decrement the browser-capability count in the surrounding comments, which currently say "four"/"fifth")
- Modify: `crates/core/tests/gate_script.rs` (it parses `CANARY_` prefixes out of `gates.sh`; update the expected canary set)
- Modify: `crates/server/tests/reader_chrome_browser.rs` (delete the 3 lightbox tests and the now-unused `open_lightbox`, `image_centre`, `mouse_click` helpers)
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:**
- Consumes: nothing (first task).
- Produces: `retired_names.rs` gains a reusable pattern — a test asserting a token is absent from the emitted client bundle. Later tasks extend the same file.

- [ ] **Step 1: Write the failing test**

Add to `crates/core/tests/retired_names.rs`:

```rust
/// The figure lightbox was deleted 2026-08-03 (visual minimalism pass): browsers
/// open images in a new tab and pinch-zoom natively, and the viewer cost a
/// permanently-armed capture-phase click handler on every figure.
#[test]
fn the_lightbox_is_gone_from_the_client_bundle() {
    let js = taliesin_core::render::code_scripts();
    for needle in ["taliInitLightbox", "tali-lightbox", "__taliLightbox"] {
        assert!(
            !js.contains(needle),
            "`{needle}` still ships in the client bundle; the lightbox was deleted"
        );
    }
}
```

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names the_lightbox_is_gone -- --nocapture`
Expected: FAIL — `taliInitLightbox` still ships in the client bundle.

- [ ] **Step 3: Delete the fragment and its two registration sites**

```bash
rm crates/core/assets/js/code-enhance/11-lightbox.js
```

In `crates/core/src/render/mod.rs`, delete the line:
```rust
    include_str!("../../assets/js/code-enhance/11-lightbox.js"),
```

In `crates/core/assets/js/code-enhance/09-register.js`, delete the line:
```js
  reg.register(function () { taliInitLightbox(); });
```

In the same `mod.rs`, the `code_scripts_for` doc comments name the lightbox in two places (~2003, ~2012: "copy buttons, lightbox, link-preview, category-filter" and "copy buttons / lightbox / link-preview + the whole reader menu"). Remove `lightbox` from both lists. Leave `link-preview` and `category-filter` for now — Tasks 4 and 9 remove them.

- [ ] **Step 4: Retire the canary**

In `tools/gates.sh`, delete the comment block and assignment at lines 88–92:
```sh
# A fifth browser-backed capability, independent of the other four: the figure lightbox.
# The whole viewer is built in JS, so nothing about it reaches the served HTML — every
# other test of a figure asserts what Rust EMITTED and would stay green with the viewer's
# open/close handlers inverted.
CANARY_LIGHTBOX="clicking_the_enlarged_image_closes_the_lightbox"
```
Then grep `tools/gates.sh` for `CANARY_LIGHTBOX` and `reader_chrome_browser` and remove every remaining reference (the test-target list around line 244 names `reader_chrome_browser`; leave that target listed for now — Tasks 2 and 3 still use it — but remove the lightbox canary assertion).

Update `crates/core/tests/gate_script.rs` so its expected canary set no longer contains `CANARY_LIGHTBOX`.

- [ ] **Step 5: Trim the browser suite**

In `crates/server/tests/reader_chrome_browser.rs`, delete these three tests and any helper that becomes unused (`open_lightbox` at ~516, `image_centre` at ~564, `mouse_click` at ~597 — verify with `cargo build` warnings):
- `clicking_the_enlarged_image_closes_the_lightbox` (~632)
- `the_enlarged_image_advertises_zoom_out` (~664)
- `stepping_the_gallery_does_not_close_the_lightbox` (~680)

- [ ] **Step 6: Run the tests and verify they pass**

```bash
cargo build -p taliesin-core
cargo test -p taliesin-core --test retired_names the_lightbox_is_gone
cargo test -p taliesin-core --test gate_script
cargo test -p taliesin-core --test render
```
Expected: all PASS. In particular `code_enhance_bundle_matches_fragments_in_order` must pass — if it fails you deleted the file but not the `include_str!` line (or vice versa).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(reader): delete the figure lightbox and its browser canary

Browsers open images in a new tab and pinch-zoom natively. The viewer cost
292 lines plus a permanently-armed capture-phase click handler on every
figure, and its own gates.sh canary.

Known loss: complex mermaid diagrams lose their comfortable mobile
inspection path. Accepted; revisit only on a real reader report."
```

### Task 2: Delete reading-position resume, the Continue-reading pill, and TOC read checkmarks

**Files:**
- Delete: `crates/core/assets/js/code-enhance/15-reading-progress.js`
- Modify: `crates/core/src/render/mod.rs` (remove the `include_str!`)
- Modify: `crates/core/assets/js/code-enhance/09-register.js` (remove `reg.register(function () { taliInitReadingProgress(); });`)
- Modify: `crates/core/assets/css/base.css` (6 occurrences of `tali-resume`)
- Modify: `crates/server/tests/reader_chrome_browser.rs` (delete `the_reading_bar_is_gone_but_the_resume_position_is_not` at ~755 and the `read_progress` helper at ~376)
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:**
- Consumes: the `retired_names.rs` pattern from Task 1.
- Produces: nothing new.

- [ ] **Step 1: Write the failing test**

```rust
/// Reading-position resume, the "Continue reading" pill, and the TOC read
/// checkmarks were deleted 2026-08-03. This finishes the deletion begun on
/// 2026-08-02, when the ambient top progress bar went for duplicating the
/// native scrollbar: browsers restore scroll on reload and back-navigation.
#[test]
fn the_reading_position_features_are_gone() {
    let js = taliesin_core::render::code_scripts();
    for needle in ["taliInitReadingProgress", "__taliProgress", "tali-resume"] {
        assert!(
            !js.contains(needle),
            "`{needle}` still ships; reading-position tracking was deleted"
        );
    }
    let css = taliesin_core::render::base_css();
    assert!(
        !css.contains("tali-resume"),
        "the resume pill's CSS survives in base.css"
    );
}
```

If `taliesin_core::render::base_css()` does not exist, read the const directly — grep `render/mod.rs` for `BASE_CSS` and expose a `pub fn base_css()` beside the existing `pub fn katex_css()` at ~2200, matching its shape exactly.

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names the_reading_position_features_are_gone`
Expected: FAIL — `taliInitReadingProgress` still ships.

- [ ] **Step 3: Delete the fragment, its registration, and its CSS**

```bash
rm crates/core/assets/js/code-enhance/15-reading-progress.js
```
Remove the `include_str!("../../assets/js/code-enhance/15-reading-progress.js"),` line from `render/mod.rs` and the `reg.register(function () { taliInitReadingProgress(); });` line from `09-register.js`.

In `crates/core/assets/css/base.css`, remove every rule mentioning `tali-resume` (6 occurrences — find them with `grep -n tali-resume crates/core/assets/css/base.css`). Remove whole rules, not just the selector, and do not reformat neighbouring lines.

- [ ] **Step 4: Trim the browser suite**

Delete `the_reading_bar_is_gone_but_the_resume_position_is_not` (~755) and the `read_progress` helper (~376) from `crates/server/tests/reader_chrome_browser.rs`.

- [ ] **Step 5: Run the tests and verify they pass**

```bash
cargo build -p taliesin-core
cargo test -p taliesin-core --test retired_names
cargo test -p taliesin-core --test render
```
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(reader): delete reading-position resume and TOC read marks

Finishes the deletion begun 2026-08-02, when the ambient progress bar went
for duplicating the native scrollbar. Resume-position is the same argument:
browsers restore scroll on reload and back-navigation natively. The TOC read
checkmarks animated the TOC in response to nothing the reader asked for."
```

### Task 3: Delete the mobile floating "Contents" pill

**Files:**
- Delete: `web-client/toc-sheet.js`
- Delete: `crates/server/tests/reader_chrome_browser.rs` (its last test goes here — the file becomes empty of tests)
- Modify: `crates/core/src/render/page.rs:481` (the `<button id="tali-toc-handle">` emission)
- Modify: `crates/core/assets/css/base.css` (9 occurrences of `tali-toc-handle`)
- Modify: `web-client/` bundling — grep for `toc-sheet` in `render/mod.rs` and `jsconfig.json`
- Modify: `tools/gates.sh` (remove `reader_chrome_browser` from the test-target list around line 244, now that the file is gone)
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:**
- Consumes: the `retired_names.rs` pattern.
- Produces: after this task no browser test references reader chrome; `gates.sh` is down to four browser canaries.

- [ ] **Step 1: Write the failing test**

```rust
/// The floating mobile "Contents" pill was deleted 2026-08-03: it duplicated
/// the topbar, which is already sticky and already carries Chapters.
#[test]
fn the_mobile_contents_pill_is_gone() {
    let page = taliesin_core::render::render_str_to_page("# Title\n\nBody.\n");
    assert!(
        !page.contains("tali-toc-handle"),
        "the mobile Contents pill still ships in the page shell"
    );
}
```

Use whatever single-document render helper `render/tests.rs` already uses — grep it for an existing `render_str`/`render_to_page` helper and match that call shape exactly rather than inventing one.

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names the_mobile_contents_pill_is_gone`
Expected: FAIL — `tali-toc-handle` is still emitted.

- [ ] **Step 3: Delete the client file and the server emission**

```bash
rm web-client/toc-sheet.js
```
In `crates/core/src/render/page.rs`, remove the `<button id="tali-toc-handle" type="button" aria-label="Contents">` emission at ~481 and any wrapper element that exists only to hold it. Remove every `tali-toc-handle` rule from `base.css` (9 occurrences).

Grep for the bundling site: `grep -rn "toc-sheet" crates web-client` and remove each reference.

- [ ] **Step 4: Delete the now-empty browser suite and its gates.sh target**

```bash
rm crates/server/tests/reader_chrome_browser.rs
```
Remove `reader_chrome_browser` from the `--features taliesin-server/headless-js` test-target list in `tools/gates.sh` (~line 244).

- [ ] **Step 5: Type-check the client and run the tests**

```bash
cd web-client && npx -y -p typescript tsc -p jsconfig.json && cd ..
cargo build -p taliesin-core
cargo test -p taliesin-core
cargo test -p taliesin-server
```
Expected: PASS, and `tsc` clean (a dangling `toc-sheet` reference in `jsconfig.json` shows up here).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(reader): delete the mobile floating Contents pill

It duplicated the topbar, which is already sticky and already carries
Chapters. With it goes the last reader-chrome browser test, so
reader_chrome_browser.rs and its gates.sh target are removed."
```

---

## Wave 2 — pure client deletions

### Task 4: Delete citation/xref hover previews and the hover index

The largest single removal, and the one with a server tail: the preview index is a route, a `Site` field, and a build artifact.

**Files:**
- Delete: `crates/core/assets/js/code-enhance/12-link-preview.js`, `crates/core/src/site/hover.rs`, `crates/core/tests/hover.rs`
- Modify: `crates/core/src/render/mod.rs` (`include_str!` + the two doc comments naming `link-preview`)
- Modify: `crates/core/assets/js/code-enhance/09-register.js` (remove `reg.register(function () { taliInitLinkPreview(); });`)
- Modify: `crates/core/src/site/mod.rs` — remove `mod hover;` (~222), the `pub hover_index_json: String` field (~179), its initialiser (~554), the `site.build_hover_index();` call (~567), the `build_hover_index` fn (~1474–1530), and the emission guard at ~796
- Modify: `crates/server/src/serve_site/mod.rs` — remove the `/hover-index.js` route (~662), the `hover_index_js` handler (~877–885), and the `lookup == "hover-index.js"` branch (~945)
- Modify: `crates/server/src/build.rs` — remove the two `hover_index_json` blocks (~2159, ~2382)
- Modify: `crates/core/src/site/links.rs` — four doc comments reference `hover::rebase_url` / `hover::leading_tag_has_id` (lines 87, 116, 127, 404). Rewrite them to describe the behaviour without naming the deleted module. **Do not delete the functions** — `rewrite_one_href` still uses them.
- Modify: `crates/core/src/site/CLAUDE.md` (module map)
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:**
- Consumes: the `retired_names.rs` pattern.
- Produces: `Site` no longer has a `hover_index_json` field. Any later task touching `Site` construction must not reintroduce it.

- [ ] **Step 1: Write the failing test**

```rust
/// Hover cross-reference cards were deleted 2026-08-03: they fired on passive
/// mouse movement, uninvited, over every citation and cross-reference.
#[test]
fn hover_preview_cards_are_gone() {
    let js = taliesin_core::render::code_scripts();
    for needle in ["taliInitLinkPreview", "__taliLinkPreview", "TALIESIN_HOVER_INDEX"] {
        assert!(!js.contains(needle), "`{needle}` still ships; hover cards were deleted");
    }
}
```

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names hover_preview_cards_are_gone`
Expected: FAIL — `taliInitLinkPreview` still ships.

- [ ] **Step 3: Delete the client fragment**

```bash
rm crates/core/assets/js/code-enhance/12-link-preview.js
```
Remove its `include_str!` line and its `reg.register(...)` line. Remove `link-preview` from the two `code_scripts_for` doc comments.

- [ ] **Step 4: Delete the server-side index**

```bash
rm crates/core/src/site/hover.rs crates/core/tests/hover.rs
```
Then remove, in this order (compiler-guided — after each removal run `cargo check -p taliesin-core`):
1. `mod hover;` from `site/mod.rs:222`
2. `site.build_hover_index();` from `site/mod.rs:567`
3. the `build_hover_index` fn body, `site/mod.rs:1474–1530`
4. the `pub hover_index_json: String` field (~179) and its `String::new()` initialiser (~554)
5. the guard at `site/mod.rs:796`

Then `cargo check -p taliesin-server` and remove the three `serve_site/mod.rs` sites and the two `build.rs` sites the errors point at.

- [ ] **Step 5: Fix the doc comments in links.rs**

The four references at `links.rs:87,116,127,404` name a module that no longer exists. Rewrite each to state the rule directly, e.g. line 116's "`hover::leading_tag_has_id` (any ` id=\"`)" becomes "a leading tag already carrying an ` id=\"` attribute". Do not change any code.

- [ ] **Step 6: Verify the artifact is gone end to end**

```bash
cargo build --release
./target/release/taliesin build docs/guide --out /tmp/tali-hovercheck
test ! -f /tmp/tali-hovercheck/hover-index.js && echo "OK: no hover-index.js"
grep -rc "TALIESIN_HOVER_INDEX" /tmp/tali-hovercheck/ | grep -v ':0' && echo "FAIL: index still emitted" || echo "OK: no index references"
```
Expected: both OK lines print.

- [ ] **Step 7: Run the tests**

```bash
cargo test -p taliesin-core
cargo test -p taliesin-server
```
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(reader): delete hover cross-reference cards and the hover index

They fired on passive mouse movement, uninvited, over every citation and
cross-reference — the single most intrusive thing on a reading page. With the
client fragment go the Site::hover_index_json field, the /hover-index.js
route, and the build artifact."
```

### Task 5: Delete heading and figure anchor links

**Files:**
- Delete: `crates/core/assets/js/code-enhance/02-anchor-links.js`
- Modify: `crates/core/src/render/mod.rs` (`include_str!`), `09-register.js` (remove `reg.register(taliInitAnchorLinks);`)
- Modify: `crates/core/assets/css/base.css` (11 × `tali-anchor`), `crates/core/assets/css/site.css` (1 ×)
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:** Consumes the `retired_names.rs` pattern. Produces nothing new.

- [ ] **Step 1: Write the failing test**

```rust
/// Heading/figure `#` anchor links were deleted 2026-08-03. The TOC already
/// emits deep links, and the fragment's own justification cited a "selection
/// toolbar's text-fragment Share" that does not exist anywhere in the tree.
#[test]
fn heading_anchor_links_are_gone() {
    let js = taliesin_core::render::code_scripts();
    for needle in ["taliInitAnchorLinks", "tali-anchor", "__taliAnchorLive"] {
        assert!(!js.contains(needle), "`{needle}` still ships; anchor links were deleted");
    }
}
```

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names heading_anchor_links_are_gone`
Expected: FAIL.

- [ ] **Step 3: Delete the fragment, registration, and CSS**

```bash
rm crates/core/assets/js/code-enhance/02-anchor-links.js
```
Remove the `include_str!` line and `reg.register(taliInitAnchorLinks);`. Remove all `tali-anchor` rules from `base.css` (11) and `site.css` (1).

- [ ] **Step 4: Run the tests**

```bash
cargo build -p taliesin-core && cargo test -p taliesin-core
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(reader): delete heading and figure anchor copy-links

The TOC already emits deep links. The fragment justified itself against a
selection toolbar that does not exist in the tree — a stale comment pointing
at a removed feature."
```

### Task 6: Delete video hover-play and fall back to native controls

**The one behavioural dependency in this plan.** Tasks 1 and 6 together remove *every* play path for `{{< video >}}` (click-to-lightbox was the touch path), so the native-controls fallback must land in this same commit.

**Files:**
- Delete: `crates/core/assets/js/code-enhance/18-media.js`
- Modify: `crates/core/src/render/mod.rs` (`include_str!`), `09-register.js` (remove its registration if present — grep for `taliInitMedia`)
- Modify: the `{{< video >}}` emitter in `crates/core/src/render/extension/` — make `controls` default to on
- Modify: `corpus/media/screencast.tmd` if it asserts hover behaviour
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:** Consumes the `retired_names.rs` pattern. Produces: `{{< video >}}` now emits `controls` unconditionally unless the author writes `controls=false`.

- [ ] **Step 1: Write the failing test**

```rust
/// Video hover-play was deleted 2026-08-03: playing on passive pointerenter is
/// motion the reader did not request (WCAG 2.2.2 territory). With the lightbox
/// also gone, native controls are the only play path, so they must be on.
#[test]
fn video_has_native_controls_and_no_hover_play() {
    let js = taliesin_core::render::code_scripts();
    assert!(!js.contains("taliInitMedia"), "the hover-play enhancer still ships");

    let html = taliesin_core::render::render_str("{{< video clip.mp4 >}}\n");
    assert!(
        html.contains("controls"),
        "a bare {{{{< video >}}}} must emit native controls — with hover-play and \
         the lightbox both deleted it is the only remaining play path"
    );
}
```

Match `render_str` to whatever single-document helper `render/tests.rs` already uses.

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names video_has_native_controls`
Expected: FAIL.

- [ ] **Step 3: Delete the enhancer**

```bash
rm crates/core/assets/js/code-enhance/18-media.js
```
Remove its `include_str!` line and any registration line.

- [ ] **Step 4: Make controls the default**

Find the video emitter: `grep -rn "controls" crates/core/src/render/extension/`. Change the attribute logic so `controls` is emitted unless the author explicitly passed `controls=false`. Keep `autoplay` off — the no-autoplay rule (WCAG 2.2.2) is unchanged and is not what this task relaxes.

- [ ] **Step 5: Run the tests and check the corpus pin**

```bash
cargo build -p taliesin-core && cargo test -p taliesin-core
./target/release/taliesin check corpus/media/screencast.tmd
```
Expected: tests PASS, `check` reports no diagnostics.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(reader): delete video hover-play, default to native controls

Playing on passive pointerenter is motion the reader did not request. Since
Task 1 removed the lightbox (the touch play path), native controls become the
only play path and must be on by default. Autoplay stays off."
```

### Task 7: Delete the reader show/hide code toggle

**Files:**
- Delete: `crates/core/assets/js/code-enhance/20-code-visibility.js`
- Modify: `crates/core/src/render/mod.rs` (`include_str!`), `09-register.js` (remove `reg.register(function () { taliInitCodeVisibility(); });`)
- Modify: `crates/core/src/render/theme.rs` — remove the pre-paint API `window.taliSetCodeHidden` (~226) and `window.taliGetCodeHidden` (~234) and the `codeHidden` state they wrap
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:** Consumes the `retired_names.rs` pattern. Produces: the pre-paint bootstrap in `theme.rs` now carries theme only. Task 13 also edits this bootstrap — do not conflict.

- [ ] **Step 1: Write the failing test**

```rust
/// The reader show/hide-code toggle was deleted 2026-08-03. The author already
/// decides per cell with `#| echo:`; a reader override of that presentation
/// decision cost a permanent row in the Settings menu.
#[test]
fn the_code_visibility_toggle_is_gone() {
    let js = taliesin_core::render::code_scripts();
    for needle in ["taliInitCodeVisibility", "taliSetCodeHidden", "taliGetCodeHidden"] {
        assert!(!js.contains(needle), "`{needle}` still ships; the toggle was deleted");
    }
}
```

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names the_code_visibility_toggle_is_gone`
Expected: FAIL.

- [ ] **Step 3: Delete the fragment, registration, and pre-paint API**

```bash
rm crates/core/assets/js/code-enhance/20-code-visibility.js
```
Remove the `include_str!` line and the `reg.register(...)` line. In `render/theme.rs`, remove `window.taliSetCodeHidden` (~226), `window.taliGetCodeHidden` (~234), and the `codeHidden` variable and any class they toggled. **Leave `taliSetTheme` / `taliGetThemeChoice` untouched** — the theme picker survives this pass.

- [ ] **Step 4: Run the tests**

```bash
cargo build -p taliesin-core && cargo test -p taliesin-core
```
Expected: PASS. `retired_names.rs`'s existing theme assertions must still pass — if a theme test broke you removed too much from `theme.rs`.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(reader): delete the reader show/hide-code toggle

The author already decides per cell with \`#| echo:\`. A reader override of an
author's presentation decision is a real but rare need, and it cost a
permanent row in the Settings menu plus a pre-paint API."
```

---

## Wave 3 — site-module deletions

### Task 8: Delete "Referenced by" backlinks

**Files:**
- Delete: `crates/core/src/site/backlinks.rs`, `crates/core/src/site/sentences.rs`, `crates/core/tests/backlinks_are_exercised.rs`, `crates/core/tests/xref_backlinks.rs`
- Modify: `crates/core/src/site/mod.rs` — remove `mod backlinks;` (215), `pub use backlinks::Backref;` (216), `mod sentences;` (227), and the index build at ~1446–1464
- Modify: `crates/core/src/site/CLAUDE.md` (module map names `sentences.rs`)
- Modify: any corpus doc asserting a "Referenced by" line — find with `grep -rln "Referenced by" corpus docs`
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:**
- Consumes: nothing from Wave 2.
- Produces: `Backref` is no longer a public export of `taliesin_core::site`. Verified before planning: `sentences.rs`'s only consumer is `backlinks.rs`, so the pair removes cleanly.

- [ ] **Step 1: Write the failing test**

```rust
/// The "Referenced by" backlink line was deleted 2026-08-03: it injected a
/// reverse-reference into a target block that a linear reader never asked for.
/// `sentences.rs` went with it — `backlinks.rs` was its only consumer.
#[test]
fn referenced_by_backlinks_are_gone() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/site");
    for gone in ["backlinks.rs", "sentences.rs"] {
        assert!(!dir.join(gone).exists(), "site/{gone} should have been deleted");
    }
}
```

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names referenced_by_backlinks_are_gone`
Expected: FAIL — `site/backlinks.rs` still exists.

- [ ] **Step 3: Delete the modules and their tests**

```bash
rm crates/core/src/site/backlinks.rs crates/core/src/site/sentences.rs
rm crates/core/tests/backlinks_are_exercised.rs crates/core/tests/xref_backlinks.rs
```

- [ ] **Step 4: Unwire, compiler-guided**

Remove from `site/mod.rs`: `mod backlinks;` (215), `pub use backlinks::Backref;` (216), `mod sentences;` (227). Then `cargo check -p taliesin-core` and remove the block it flags at ~1446–1464 (the `per_page` map, the `citing_sentence` call, and the `self.backlinks = build_backlink_index(...)` assignment), plus the `backlinks` field on `Site` and its initialiser. Repeat `cargo check` until clean.

- [ ] **Step 5: Update the module map**

In `crates/core/src/site/CLAUDE.md`, delete the `sentences.rs` bullet and any `backlinks.rs` mention from the module map.

- [ ] **Step 6: Run the tests and rebuild a site**

```bash
cargo build --release
cargo test -p taliesin-core
./target/release/taliesin build docs/guide --out /tmp/tali-backlinkcheck
grep -rl "Referenced by" /tmp/tali-backlinkcheck/ && echo "FAIL: still emitted" || echo "OK: gone"
```
Expected: tests PASS and "OK: gone".

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(site): delete Referenced-by backlinks and the sentence splitter

A reverse-reference line injected into a target block is wiki-brain; a linear
reader never asked for it. sentences.rs went with it — backlinks.rs was its
only consumer (verified)."
```

### Task 9: Delete the category filter chips and the category linter

Retires **one scoped key**: `listing.categories`. Page-level `categories:` front matter and the card badges survive.

**Files:**
- Delete: `crates/core/assets/js/code-enhance/10-category-filter.js`, `crates/core/src/site/categories.rs`
- Modify: `crates/core/src/render/mod.rs` (`include_str!` + remove `category-filter` from the doc comment), `09-register.js` (remove `reg.register(taliInitCategoryFilter);`)
- Modify: `crates/core/src/site/mod.rs` (remove `mod categories;` at 220 and its call sites)
- Modify: `crates/core/src/frontmatter.rs` — remove `"categories"` from `LISTING_KEYS` (303) and add a `RETIRED_KEYS` entry
- Modify: `crates/core/src/schema.rs:75` (remove `("categories", boolean())` from the listing schema)
- Modify: `crates/core/assets/css/site.css` (1 × `tali-cat-filter`, 3 × `tali-cat-chip`; **keep `tali-cat`** — the card badge survives)
- Modify: `editor/vscode/schema/tali-site.schema.json` (the bundled copy)
- Modify: `corpus/tech-blog/blog.tmd:7`, `corpus/tech-blog/projects.tmd:7` (drop `categories: true`)
- Modify: `docs/guide/using/formats.tmd:373,386`; `docs/guide/using/from-quarto.tmd`
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:**
- Consumes: nothing from Wave 2.
- Produces: `LISTING_KEYS` is `["contents", "id", "type", "max-items"]`.

- [ ] **Step 1: Write the failing test**

```rust
/// The listing category-filter chips were deleted 2026-08-03. They paid off only
/// on a blog with many posts AND disciplined category vocabulary — the linter
/// existed precisely because that discipline does not hold by default.
/// Page-level `categories:` and the card badges SURVIVE; only `listing.categories` is retired.
#[test]
fn the_listing_categories_subkey_is_retired_but_page_categories_live() {
    assert!(
        !taliesin_core::frontmatter::LISTING_KEYS.contains(&"categories"),
        "`listing.categories` should be retired"
    );
    assert!(
        taliesin_core::frontmatter::FRONT_MATTER_KEYS.contains(&"categories"),
        "page-level `categories:` must SURVIVE — only the listing sub-key is retired"
    );
    let js = taliesin_core::render::code_scripts();
    assert!(!js.contains("taliInitCategoryFilter"), "the filter enhancer still ships");
}
```

Match the two const names to what `frontmatter.rs` actually exports (grep it; the page-level list is around line 29). Make them `pub(crate)`-visible to the test the same way the existing `retired_names.rs` assertions reach their consts.

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names the_listing_categories_subkey_is_retired`
Expected: FAIL.

- [ ] **Step 3: Delete the client fragment and the linter**

```bash
rm crates/core/assets/js/code-enhance/10-category-filter.js crates/core/src/site/categories.rs
```
Remove the `include_str!` line, the `reg.register(taliInitCategoryFilter);` line, `category-filter` from the `code_scripts_for` doc comment, and `mod categories;` from `site/mod.rs:220` (then `cargo check` and remove the call sites it flags).

Remove `tali-cat-filter` (1) and `tali-cat-chip` (3) rules from `site.css`. **Leave `tali-cat` alone** — that is the surviving card badge.

- [ ] **Step 4: Retire the scoped key**

In `crates/core/src/frontmatter.rs`, change `LISTING_KEYS` (303) to:
```rust
pub(crate) const LISTING_KEYS: &[&str] = &["contents", "id", "type", "max-items"];
```
Add to `RETIRED_KEYS` (the `(scope, key, note)` tuple form at line 92):
```rust
    (
        "listing key",
        "categories",
        "it was removed on 2026-08-03: the filter-chip row paid off only on a large \
         archive with a disciplined category vocabulary. Page-level `categories:` still \
         works and still shows as a badge on each card",
    ),
```
Match the `scope` string to whatever scope the listing-key validator passes to `unknown_key_message` — grep for where `LISTING_KEYS` is consulted and copy that literal exactly, or the retired note will never fire.

Remove `("categories", boolean())` from `crates/core/src/schema.rs:75`, and mirror the change in `editor/vscode/schema/tali-site.schema.json`.

- [ ] **Step 5: Update the corpus and docs**

Remove `categories: true` from `corpus/tech-blog/blog.tmd:7` and `corpus/tech-blog/projects.tmd:7`. Update `docs/guide/using/formats.tmd:373` (the example) and `:386` (the prose bullet describing the chip row). Add a `from-quarto.tmd` line telling a migrating reader the sub-key is gone and page-level `categories:` remains.

- [ ] **Step 6: Run every gate this touches**

```bash
cargo build --release
cargo test -p taliesin-core
cargo test -p taliesin-server
cd editor/vscode && npm test && cd ../..
./target/release/taliesin check corpus/tech-blog
```
Expected: all PASS. The companion `npm test` is the ONLY gate on the bundled JSON schema copy — if you skipped Step 4's schema mirror, it fails here.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(site): delete listing category filter chips and the linter

Retires the scoped \`listing.categories\` key only; page-level \`categories:\`
and the card badges survive. The chips paid off only on a large archive with a
disciplined vocabulary — the linter existed because that does not hold."
```

### Task 10: Delete the chapter-outline disclosures in the book drawer

**Files:**
- Delete: `crates/core/assets/js/code-enhance/19-book-outline.js`
- Modify: `crates/core/src/render/mod.rs` (`include_str!`), `09-register.js` (remove `reg.register(function () { taliInitBookOutline(); });`)
- Modify: `crates/core/assets/css/site.css` (grep for the disclosure rules the fragment styled)
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:** Consumes the `retired_names.rs` pattern. Produces nothing new. Note the fragment reads the same `search-index.js` the Cmd-K palette uses — **that index survives**, since search survives. Do not delete it.

- [ ] **Step 1: Write the failing test**

```rust
/// The per-chapter outline disclosures in the book drawer were deleted
/// 2026-08-03: a second navigation layer inside a drawer that is already a
/// navigation layer. Justified against a 60-chapter book; the largest real book
/// in the tree is docs/guide at 25 chapters.
#[test]
fn the_book_drawer_outline_is_gone() {
    let js = taliesin_core::render::code_scripts();
    assert!(!js.contains("taliInitBookOutline"), "the drawer outline still ships");
}
```

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names the_book_drawer_outline_is_gone`
Expected: FAIL.

- [ ] **Step 3: Delete the fragment and its CSS**

```bash
rm crates/core/assets/js/code-enhance/19-book-outline.js
```
Remove the `include_str!` line and the `reg.register(...)` line. Remove the disclosure-specific rules from `site.css` (find them by grepping `site.css` for the class names the deleted fragment used). **Keep every `.tali-book-chapter` rule** — the flat chapter list survives.

- [ ] **Step 4: Verify the drawer still works**

```bash
cargo build --release
cargo test -p taliesin-core
./target/release/taliesin build docs/guide --out /tmp/tali-drawercheck
grep -c "tali-book-chapter" /tmp/tali-drawercheck/using/writing.html
ls /tmp/tali-drawercheck/_assets/ | grep -c search
```
Expected: tests PASS, the chapter count is non-zero (the flat chapter list
survives), and the search-index asset count is non-zero. **That last check is the
real assertion that the shared index survived** — the outline read the same
`search-index.js` Cmd-K uses, and deleting the outline must not take it with it.
A unit test cannot check this: it asserts on built output, not on the bundle.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(book): delete the per-chapter outline disclosures

A second navigation layer inside a drawer that is already one. Justified
against a 60-chapter book; the largest real book here is 25 chapters. The
flat chapter list and the shared search index both survive."
```

### Task 11: Delete the book download button

**Files:**
- Modify: `crates/core/src/site/chrome.rs` (the `tali-book-download` emission)
- Modify: `crates/core/assets/css/site.css` (4 × `tali-book-download`)
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:** Consumes nothing. Produces: the book topbar carries brand + Chapters + search + gear only.

- [ ] **Step 1: Write the failing test**

```rust
/// The book download button was deleted 2026-08-03: one more permanent control
/// in the topbar for an action a reader rarely wants.
#[test]
fn the_book_download_button_is_gone() {
    let css = taliesin_core::render::site_css();
    assert!(!css.contains("tali-book-download"), "its CSS survives in site.css");
}
```

Add a `pub fn site_css()` beside `pub fn katex_css()` (`render/mod.rs:2200`) if one does not exist, matching that function's shape.

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names the_book_download_button_is_gone`
Expected: FAIL.

- [ ] **Step 3: Remove the emission and the CSS**

In `crates/core/src/site/chrome.rs`, find the `tali-book-download` emission (`grep -n tali-book-download crates/core/src/site/chrome.rs`) and remove the element and any config plumbing that exists only to feed it. Remove all 4 `tali-book-download` rules from `site.css`.

- [ ] **Step 4: Run the tests**

```bash
cargo build -p taliesin-core && cargo test -p taliesin-core
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(book): delete the topbar download button

One more permanent control for an action a reader rarely wants. The topbar is
now brand + Chapters + search + settings."
```

---

## Wave 4 — the construct vocabulary

Gentler cut: these cost a reader nothing until an author writes them, so only redundancy and never-referenced surface goes.

### Task 12: Reduce callout kinds from 5 to 3

**Files:**
- Modify: `crates/core/src/render/validate.rs:35` (`CALLOUT_KINDS`)
- Modify: `crates/core/src/frontmatter.rs` (`RETIRED_KEYS`: two entries, scope `callout kind`)
- Modify: `crates/core/src/vocab.rs` (offered completions), `crates/core/src/schema.rs` if it enumerates them
- Modify: `crates/core/assets/css/base.css` (the `important` / `caution` callout rules)
- Modify: `corpus/callouts/kinds.tmd`; `docs/guide/using/recipes.tmd`, `using/theming.tmd`, `using/interactive.tmd`, `reference/shortcodes.tmd`, `reference/cell-options.tmd`, `docs/internals/client.tmd`, `docs/guide/using/from-quarto.tmd`
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:** Produces `CALLOUT_KINDS = ["note", "tip", "warning"]`.

- [ ] **Step 1: Write the failing test**

```rust
/// Callout kinds went 5 -> 3 on 2026-08-03: readers cannot decode *important*
/// vs *warning* vs *caution* visually, and all three rendered as a coloured box
/// with a different word in it.
#[test]
fn callout_kinds_are_three_and_the_two_cut_ones_are_registered() {
    let kinds = taliesin_core::render::callout_kinds();
    assert_eq!(kinds, &["note", "tip", "warning"], "callout vocabulary should be 3");
    for gone in ["important", "caution"] {
        assert!(
            taliesin_core::frontmatter::retired_note("callout kind", gone).is_some(),
            "`{gone}` must have a RETIRED_KEYS entry or an author gets silence"
        );
    }
}
```

Expose `callout_kinds()` and `retired_note(scope, key)` as thin `pub` accessors over the existing consts if they are not already reachable — match how `retired_names.rs` reaches other consts rather than changing any visibility broadly.

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names callout_kinds_are_three`
Expected: FAIL.

- [ ] **Step 3: Cut the vocabulary and register the retirements**

`validate.rs:35` becomes:
```rust
pub(crate) const CALLOUT_KINDS: &[&str] = &["note", "tip", "warning"];
```
Add two `RETIRED_KEYS` entries, scope `"callout kind"` (copy the exact scope literal `validate_callout_kind` passes to `unknown_key_message` at `validate.rs:135`):
```rust
    (
        "callout kind",
        "important",
        "it was removed on 2026-08-03: three kinds cover the distinctions a reader can \
         actually decode. Use `warning` for a consequence, `note` for an aside",
    ),
    (
        "callout kind",
        "caution",
        "it was removed on 2026-08-03: three kinds cover the distinctions a reader can \
         actually decode. Use `warning`",
    ),
```
Remove the `important` / `caution` rules from `base.css` and their `vocab.rs` entries.

- [ ] **Step 4: Migrate the corpus and docs**

Rewrite every `callout-important` / `callout-caution` in the 7 affected files to `callout-warning` (or `callout-note` where the content is an aside, author's judgement). Add a `from-quarto.tmd` line.

- [ ] **Step 5: Verify the did-you-mean actually fires**

```bash
cargo build --release
printf -- '---\ntitle: t\n---\n\n::: {.callout-caution}\nx\n:::\n' > /tmp/tali-callout.tmd
./target/release/taliesin check /tmp/tali-callout.tmd
```
Expected: a located diagnostic naming `caution` and pointing at `warning`. **A silent pass means the scope string in Step 3 does not match the validator's** — fix it before continuing.

- [ ] **Step 6: Run the tests**

```bash
cargo test -p taliesin-core && cargo test -p taliesin-server
cd editor/vscode && npm test && cd ../..
```
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(callouts): reduce callout kinds from five to three

note / tip / warning. Readers cannot decode important vs warning vs caution
visually — all three were a coloured box with a different word. Both cut
kinds are in RETIRED_KEYS so an author gets a did-you-mean, not silence."
```

### Task 13: Reduce margin-content spellings from 4 to 1

`.aside` and `.marginnote` have **zero** uses in the tree. `.sidenote` has exactly one (`samples/paper.tmd:62`).

**Files:**
- Modify: `crates/core/src/render/validate.rs:58` (`DIV_FEATURE_CLASSES` — remove `aside`, `sidenote`, `marginnote`), and `RETIRED_DIV_CLASSES` at ~191 (add three)
- Modify: `crates/core/src/render/divs.rs` (the alias match arm)
- Modify: `crates/core/src/vocab.rs` (`DIV_CLASS_NAMES`)
- Modify: `samples/paper.tmd:62` (`.sidenote` → `.column-margin`)
- Modify: `docs/guide/using/writing.tmd:248` (the sentence documenting the aliases)
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:** Produces: `column-margin` is the only accepted margin spelling.

- [ ] **Step 1: Write the failing test**

```rust
/// Margin-content spellings went 4 -> 1 on 2026-08-03. `.aside` and
/// `.marginnote` had zero uses in the tree; `.sidenote` had one. The aliases
/// were a Quarto/Tufte/Distill welcome mat for a tool that has otherwise shed
/// its Quarto vocabulary.
#[test]
fn margin_aliases_are_retired_with_notes() {
    for gone in ["aside", "sidenote", "marginnote"] {
        assert!(
            taliesin_core::render::retired_div_note(gone).is_some(),
            "`.{gone}` MUST have a RETIRED_DIV_CLASSES entry — div classes are an \
             open vocabulary, so without one a leftover class gets SILENCE and the \
             page quietly loses its layout"
        );
    }
}
```

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names margin_aliases_are_retired`
Expected: FAIL.

- [ ] **Step 3: Cut the aliases and register them**

Remove `"aside"`, `"sidenote"`, `"marginnote"` from `DIV_FEATURE_CLASSES` (`validate.rs:58`). Collapse the alias match arm in `divs.rs` so only `column-margin` enters the margin branch. Add three `RETIRED_DIV_CLASSES` entries (the `(&str, &str)` form at `validate.rs:191`):
```rust
    (
        "aside",
        "it was removed on 2026-08-03. `.column-margin` is the only margin spelling now",
    ),
    (
        "sidenote",
        "it was removed on 2026-08-03. `.column-margin` is the only margin spelling now",
    ),
    (
        "marginnote",
        "it was removed on 2026-08-03. `.column-margin` is the only margin spelling now",
    ),
```
Remove the three names from `vocab.rs`'s `DIV_CLASS_NAMES` if present.

- [ ] **Step 4: Migrate the one real use and the docs**

`samples/paper.tmd:62`: `::: {.sidenote}` → `::: {.column-margin}`. Rewrite `docs/guide/using/writing.tmd:248` (currently "`.column-margin` is the canonical name; `.sidenote`, `.marginnote`, and `.aside` are accepted aliases…") to state that `.column-margin` is the only spelling.

- [ ] **Step 5: Verify the retirement notice fires**

```bash
cargo build --release
printf -- '---\ntitle: t\n---\n\n::: {.sidenote}\nx\n:::\n' > /tmp/tali-margin.tmd
./target/release/taliesin check /tmp/tali-margin.tmd
```
Expected: a diagnostic naming `.sidenote` and pointing at `.column-margin`. **Silence here is the exact failure mode this task guards against.**

- [ ] **Step 6: Run the tests**

```bash
cargo test -p taliesin-core
./target/release/taliesin check samples/paper.tmd
```
Expected: PASS, no diagnostics.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(divs): collapse four margin spellings into column-margin

.aside and .marginnote had zero uses; .sidenote had one. All three are in
RETIRED_DIV_CLASSES — without an entry a leftover class gets silence, not a
did-you-mean, and the page quietly loses its layout."
```

### Task 14: Reduce theorem kinds from 8 to 5

`exm` / `prp` / `rem` are never cross-referenced by any document.

**Files:**
- Modify: `crates/core/src/render/validate.rs:42` (`THEOREM_KINDS` — remove `example`, `proposition`, `remark`)
- Modify: the xref-kind table (`crates/core/src/cite/` — remove the `exm-`, `prp-`, `rem-` prefixes)
- Modify: `crates/core/src/vocab.rs`
- Modify: `corpus/refs/theorems.tmd`, `docs/guide/using/theorems.tmd`, `docs/guide/using/from-quarto.tmd`
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:** Produces `THEOREM_KINDS = ["theorem", "lemma", "corollary", "definition", "proof"]`.

- [ ] **Step 1: Write the failing test**

```rust
/// Theorem kinds went 8 -> 5 on 2026-08-03. example / proposition / remark were
/// never cross-referenced by any document in the tree.
#[test]
fn theorem_kinds_are_five() {
    let kinds = taliesin_core::render::theorem_kinds();
    assert_eq!(
        kinds,
        &["theorem", "lemma", "corollary", "definition", "proof"],
        "theorem vocabulary should be 5"
    );
    for gone in ["example", "proposition", "remark"] {
        assert!(
            taliesin_core::render::retired_div_note(gone).is_some(),
            "`.{gone}` needs a RETIRED_DIV_CLASSES entry — a misspelled theorem kind \
             has no prefix to anchor a did-you-mean and falls through to a plain div"
        );
    }
}
```

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names theorem_kinds_are_five`
Expected: FAIL.

- [ ] **Step 3: Cut the kinds and register them**

`validate.rs:42` becomes:
```rust
pub(crate) const THEOREM_KINDS: &[&str] = &[
    "theorem",
    "lemma",
    "corollary",
    "definition",
    "proof",
];
```
Add three `RETIRED_DIV_CLASSES` entries for `example`, `proposition`, `remark` in the same form as Task 13, each noting removal on 2026-08-03 and pointing at `definition` or a plain callout. Remove the `exm-` / `prp-` / `rem-` xref prefixes from the cross-reference kind table in `crates/core/src/cite/` (grep for `"exm"` to find it) and from `vocab.rs`.

- [ ] **Step 4: Migrate the corpus and docs**

Rewrite the `example` / `proposition` / `remark` environments in `corpus/refs/theorems.tmd` and `docs/guide/using/theorems.tmd` to a surviving kind or a plain `::: {.callout-note}`. Add a `from-quarto.tmd` line.

- [ ] **Step 5: Verify and run the tests**

```bash
cargo build --release
cargo test -p taliesin-core
./target/release/taliesin check corpus/refs/theorems.tmd
./target/release/taliesin check docs/guide
```
Expected: PASS, no diagnostics.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(theorems): reduce theorem kinds from eight to five

example / proposition / remark were never cross-referenced by any document.
All three are in RETIRED_DIV_CLASSES, since a misspelled theorem kind has no
prefix to anchor a did-you-mean and otherwise falls through to a plain div."
```

---

## Wave 5 — the character-key shortcuts

### Task 15: Delete the `?` and `/` shortcuts and their WCAG off-switch

**This task implements option (b) from the spec and is the one decision the owner left open.** If the owner rules (a) instead, skip this task entirely and keep both the shortcuts and their off-switch — everything else in this plan is unaffected.

Rationale: `07-keyboard.js` was exempted as zero-pixel, but keeping character-key shortcuts **forces** a visible off-switch into the Settings menu (WCAG 2.1.4). `Esc` and `←`/`→` are not character keys and carry no such obligation, so they stay live with no control needed.

**Files:**
- Modify: `crates/core/assets/js/code-enhance/07-keyboard.js` — remove the `?` and `/` bindings and the cheatsheet section they mount; keep `Esc` and `←`/`→`
- Modify: `crates/core/assets/js/code-enhance/13-reader-menu.js` — remove the Shortcuts section and its off-switch row
- Modify: `docs/guide/using/reading.tmd:138–160` (the Keyboard and accessibility section)
- Test: `crates/core/tests/retired_names.rs`

**Interfaces:** Produces: the Settings menu contains Theme only (Code went in Task 7, Shortcuts goes here).

- [ ] **Step 1: Write the failing test**

```rust
/// The `?` and `/` character-key shortcuts were deleted 2026-08-03, and with
/// them the WCAG 2.1.4 off-switch they forced into the Settings menu. Esc and
/// the arrow keys are not character keys, so they stay live with no control.
#[test]
fn character_key_shortcuts_and_their_offswitch_are_gone() {
    let js = taliesin_core::render::code_scripts();
    for needle in ["taliShortcutsEnabled", "Keyboard shortcuts"] {
        assert!(
            !js.contains(needle),
            "`{needle}` still ships; the character-key shortcuts and their \
             WCAG 2.1.4 off-switch were deleted together"
        );
    }
    assert!(
        js.contains("ArrowLeft") || js.contains("ArrowRight"),
        "the arrow-key chapter nav must SURVIVE — it is not a character key"
    );
}
```

Match `taliShortcutsEnabled` to the real state name — grep `13-reader-menu.js` and `07-keyboard.js` for the off-switch's storage key before writing the test.

- [ ] **Step 2: Run it and verify it FAILS**

Run: `cargo test -p taliesin-core --test retired_names character_key_shortcuts`
Expected: FAIL.

- [ ] **Step 3: Remove the bindings and the menu section**

In `07-keyboard.js`, remove the `?` and `/` key handlers and the code that mounts the cheatsheet list into the Settings menu. **Keep** the `Esc` handler and the `←`/`→` chapter navigation, and keep the typing/modal guards that protect them.

In `13-reader-menu.js`, remove the Shortcuts section and its on/off control, including its `localStorage` read/write.

- [ ] **Step 4: Type-check and test**

```bash
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json && cd ../../../..
cargo build -p taliesin-core && cargo test -p taliesin-core
```
Expected: `tsc` clean and tests PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(a11y): delete the ? and / shortcuts and their WCAG off-switch

07-keyboard.js was exempted from this pass as zero-pixel, but character-key
shortcuts FORCE a visible off-switch (WCAG 2.1.4). Esc and the arrow keys are
not character keys, so they stay live and need no control. Net: one fewer
Settings row, still conformant."
```

---

## Wave 6 — docs, corpus, and real verification

### Task 16: Trim the manual and record what was removed

`docs/guide/using/reading.tmd` loses three whole sections plus the Code section — roughly 50 of 190 lines. **It does not become a stub:** five sections survive and carry most of the page's words.

**Files:**
- Modify: `docs/guide/using/reading.tmd` — delete lines 49–67 (Code), 92–105 (Reading progress), 107–113 (Hover cards), 115–120 (Anchor copy-links); fix the image-viewer clause at 171–172; rewrite the `description:` at line 3 (it names three deleted features)
- Modify: `docs/internals/client.tmd` (the enhancer list)
- Modify: `docs/guide/reference/shortcodes.tmd` (video controls)
- Delete: `corpus/reader/hovercards.tmd` (a dedicated pin for a deleted feature)
- Modify: `crates/core/tests/corpus.rs` if it names `hovercards.tmd`

**Interfaces:** Consumes every prior task. This is the documentation reconciliation.

- [ ] **Step 1: Delete the corpus pin for the deleted feature**

```bash
rm corpus/reader/hovercards.tmd
rmdir corpus/reader 2>/dev/null || true
grep -rn "hovercards" crates/core/tests/ corpus/README.md
```
Remove every reference the grep finds.

- [ ] **Step 2: Trim `reading.tmd` and extend its removal register**

Delete the four sections listed above and fix the a11y clause at 171–172 (it names "the search palette and the image viewer" as the two modal overlays; only the search palette remains).

**Then add the cut features to the page's existing removal register.** This page already records what was built and deliberately removed and why — the sepia theme (37–42), focus mode and fullscreen (122–136), the right-rail TOC (130–132), the progress bar (95–97), the text-size knob (44–47). Add a short section in the same voice covering: hover cross-reference cards, reading-position resume and TOC read marks, anchor copy-links, the image lightbox, the show/hide-code toggle, and the category filter chips — each with its one-line reason. This is in character for the page and it stops a future session re-proposing them.

Rewrite the `description:` front matter at line 3, which currently reads "theme, reading progress and resume, hover cross-reference cards, anchor links and keyboard access".

- [ ] **Step 3: Reconcile the internals book and the shortcode reference**

Update `docs/internals/client.tmd`'s enhancer list to the surviving set. Update `docs/guide/reference/shortcodes.tmd` for `{{< video >}}`'s new native-controls default.

- [ ] **Step 4: Verify the docs build clean**

```bash
cargo build --release
./target/release/taliesin check docs/guide
./target/release/taliesin check docs/internals
./target/release/taliesin build docs/guide --out /tmp/tali-docscheck
```
Expected: zero diagnostics, build succeeds.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs: reconcile the manual with the visual minimalism pass

reading.tmd loses four sections and gains a record of what was removed and
why, matching how the page already treats the sepia theme, focus mode,
fullscreen and the progress bar. Deletes corpus/reader/hovercards.tmd, the
pin for a deleted feature."
```

### Task 17: Full-gate and three-viewport visual verification

**No code changes.** This is the task that decides whether the pass is done.

**Files:** none modified unless a gate fails.

- [ ] **Step 1: Run the real gate**

```bash
./tools/gates.sh 2>&1 | tee /tmp/tali-gates.log
```
Expected: green, with every `TALIESIN_REQUIRE_*` armed and every remaining canary printing `... ok`. **A single ignored test is a failure.** Confirm the browser-canary count is now four, not five (Task 1 removed `CANARY_LIGHTBOX`).

If it fails on the companion schema, re-check Task 9 Step 4's `editor/vscode/schema/tali-site.schema.json` mirror — `cargo test --workspace` cannot catch that one.

- [ ] **Step 2: Confirm the feature catalogue matches reality**

```bash
./target/release/taliesin features . --format json > /tmp/tali-features.json
python3 -c "
import json; d=json.load(open('/tmp/tali-features.json'))
print(json.dumps(d, indent=2)[:2000])"
```
Expected: no group still advertises a deleted construct. Cross-check that `callout kinds` is 3, `theorem kinds` is 5, and the margin aliases are absent.

- [ ] **Step 3: Build the real sites and check for breakage**

```bash
./target/release/taliesin build docs/guide --out /tmp/tali-v/guide
./target/release/taliesin build docs/internals --out /tmp/tali-v/internals
./target/release/taliesin build site --out /tmp/tali-v/site
./target/release/taliesin build corpus/tech-blog --out /tmp/tali-v/blog
```
Expected: all four succeed. Record the built page sizes and compare against the pre-change baseline recorded in the spec (`using/writing.html` was 69,727 B; `app.js` was 89,148 B). **The point of the comparison is not that it shrank — it is that nothing grew.**

- [ ] **Step 4: Visual check at three viewports**

Serve `preview docs/guide` and drive the chrome-devtools MCP at mobile ~390×844, laptop landscape ~1440×900, and laptop portrait ~900×1440 (the forgotten band). The orphaned Chrome that held the MCP profile was killed on 2026-08-03; if `new_page` reports the profile is busy again, find the holder with `ps -eo pid,args | grep chrome-devtools-mcp/chrome-profile` and kill it by PID.

On each viewport confirm: prose renders; the topbar shows brand + Chapters + search + settings and nothing else; the Settings popover contains Theme only; code blocks still have copy buttons; Cmd-K still opens; prev/next still work; **no hover card appears when the pointer crosses a citation**; no `#` appears on heading hover; clicking a figure does nothing. Capture a screenshot per viewport and read the console for errors.

- [ ] **Step 5: Report honestly**

Write the outcome into `notes/` as a short execution-findings entry: what was cut, the measured line total, which gates ran, which did not, and anything that surprised you. Quote real output. If a check was skipped, say so.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "notes: record the visual minimalism pass execution findings"
```

---

## Self-Review

**Spec coverage.** Every spec item maps to a task: T5 items 1–5 → Tasks 4, 2, 8, 6, 3. T4 items 6–11 → Tasks 5, 1, 10, 11, 9, 7. The a11y exemption → honoured throughout (skip link, focus trap, scroll-a11y untouched), with its one failure mode handled in Task 15. Scale 2 → Tasks 12, 13, 14. The gate tail → Tasks 1 (canary), 9 (eight-gate key retirement), 13/14 (`RETIRED_DIV_CLASSES`). Docs/corpus → Task 16. Verification → Task 17. The video dependency → Task 6, explicitly coupled to Task 1.

**Placeholder scan.** No TBD/TODO. Three steps say "match the existing helper's shape" (Task 2 Step 1, Task 3 Step 1, Task 15 Step 1) rather than inventing a signature — that is deliberate: guessing a helper name that does not exist would be worse than instructing the implementer to read the neighbouring code. Each names the exact file and the exact symbol to grep for.

**Type consistency.** `code_scripts()` is used identically across Tasks 1, 2, 4, 5, 6, 7, 9, 10, 15. `retired_div_note(&str)` is introduced in Task 13 and reused in Task 14 with the same signature. `retired_note(scope, key)` appears in Task 12 only. `base_css()` (Task 2), `site_css()` (Task 11) are each introduced with an explicit instruction to match `katex_css()`'s existing shape at `render/mod.rs:2200`.

**Known ordering constraint:** Task 6 must come after Task 1, or `{{< video >}}` has no play path between them. Tasks 1→2→3 must stay in order — each trims `reader_chrome_browser.rs`, and Task 3 deletes it.
