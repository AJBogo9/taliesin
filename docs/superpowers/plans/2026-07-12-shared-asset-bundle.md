# Shared Asset Bundle (#17) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** For `taliesin build <dir>` (multi-page sites), emit the framework CSS + KaTeX fonts + JS as content-hashed files under `<out>/_assets/`, linked once per page, instead of inlining a full copy into every page. Single-file `build file.tmd`, `--bare`, and live `preview` stay fully inlined and byte-identical.

**Architecture:** Core gains additive accessors returning the raw bytes of each shared blob, and a `PageParts.assets: AssetMode` (`Inline` = today's behavior, or `External` = emit `<link>`/`<script src>` for passed-in hrefs). The site build computes each blob once (minified + content-hashed via the existing `fnv1a`), writes `<out>/_assets/`, and renders each page in `External` mode with depth-adjusted hrefs. All external scripts are `defer` so the inline `STATIC_ENHANCE` (DOMContentLoaded-wrapped) and per-page search index still run in the correct order.

**Tech Stack:** Rust (edition 2024); `taliesin_core::hash::fnv1a` for content hashing; a new hand-rolled `crates/server/src/minify.rs`. No new dependencies.

## Global Constraints

- Rust edition 2024, workspace resolver 3. No new crate dependencies (`lightningcss` is decided-against; use `fnv1a` for hashing).
- **Inline paths must stay byte-identical.** `preview` (both servers), single-file `build file.tmd`, and `--bare` keep `AssetMode::Inline` and must produce exactly today's bytes. Only the multi-page `build <dir>` path changes.
- **Preserve the block-model invariants:** the body HTML (every block's `data-block-id` / `data-sourcepos` / `data-source-file`) is untouched; only the `<head>` and framework `<script>` wrapper differ between Inline and External.
- **Offline, no CDN:** every `_assets/` file is local + content-hashed. The mermaid loader's `{{MERMAID}}` CDN fallback URL must never be reachable on a page without the mermaid library, so the loader ships only in the (conditional) mermaid file, never in the always-on `app.js`.
- **All external framework `<script>` tags carry `defer`** (correctness: enhancer timing relies on it).
- `_assets/` is the reserved output dir name (underscore-prefixed: cannot clobber a user `assets/`, survives the general content sweep). The build owns its lifecycle (clear stale).
- No em dashes or en dashes in any string, comment, or doc prose (use a colon/comma/parens).
- A `PostToolUse` hook runs rustfmt; keep `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean.
- Spec: [docs/superpowers/specs/2026-07-12-shared-asset-bundle-design.md](../specs/2026-07-12-shared-asset-bundle-design.md).
- Branch: `shared-asset-bundle` (already created off `main`).

## File Structure

- `crates/core/src/render/mod.rs` — add pub accessors returning the raw bytes of each shared blob + `has_mermaid`. Purely additive; the inline emitters (`code_scripts_for`, `toc_scripts`, `js_cell_head`) are untouched.
- `crates/core/src/render/model.rs` — the `AssetMode` / `ExternalAssets` types.
- `crates/core/src/render/page.rs` — `PageParts.assets` field; the `External` branch in `assemble_html_page`; `html_page_inner` threading; `html_page_from_doc_in_site_external`.
- `crates/core/src/site/mod.rs` — `render_page_doc_external` (site render entry that supplies External assets).
- `crates/server/src/serve/mod.rs`, `crates/server/src/serve_site/mod.rs` — set `assets: AssetMode::Inline` on their `PageParts` (preview stays inline).
- `crates/server/src/minify.rs` — new: `minify_css` + `minify_js` (conservative, build-time).
- `crates/server/src/build.rs` — compute/minify/hash/write `_assets/`; render pages External with depth-adjusted hrefs; stale-sweep `_assets/`.
- `crates/server/src/main.rs` — `mod minify;`.
- `crates/server/tests/asset_bundle.rs` (or extend `tech_blog.rs`) — the corpus pin.

Note on lib re-exports: `AssetMode`, `ExternalAssets`, the accessors, and `html_page_from_doc_in_site_external` must be re-exported from `crates/core/src/lib.rs` (or `render/mod.rs`'s `pub use`) so `taliesin-server` can name them, matching how `PageParts`/`assemble_html_page` are already exported.

---

### Task 1: Core shared-blob accessors (additive, no behavior change)

**Files:**
- Modify: `crates/core/src/render/mod.rs` (add accessors near the existing asset consts, ~L1080-1136)
- Test: same file's `#[cfg(test)] mod tests`

**Interfaces:**
- Produces (all `pub`, re-exported from the crate root):
  - `fn shared_site_css() -> String` — `FONTS_CSS + BASE_CSS + DARK_CSS + SITE_CSS` (exactly what a non-bare site page inlines in the main `<style>`).
  - `fn katex_css_bytes() -> &'static str` — `KATEX_CSS`.
  - `fn core_enhance_js() -> String` — all of our own JS, concatenated: `CODE_ENHANCE_JS`, `TALIESIN_JS`, `WALKTHROUGH_JS`, `TABSET_JS`, `SCROLLY_JS`, `TOC_SPY_JS`, `TOC_SHEET_JS`, `SEARCH_JS`, each separated by `"\n;\n"` (a semicolon on its own line so concatenation is ASI-safe at every boundary).
  - `fn mermaid_bundle_js() -> String` — `MERMAID_MIN_JS`, then `"\n;\n"`, then `MERMAID_JS.replace("{{MERMAID}}", &mermaid_url())` (the lib followed by the loader; self-contained).
  - `fn js_cell_libs_js() -> String` — `D3_JS`, `"\n;\n"`, `PLOT_JS`.
  - `fn has_mermaid(body: &str) -> bool` — `body.contains("class=\"mermaid\"")` (mirrors the existing check at `code_scripts_for`).

- [ ] **Step 1: Write the failing tests**

First find a stable identifying substring in each source asset to assert on. Run:
`grep -o 'function tali[A-Za-z]*' crates/core/assets/js/code-enhance/01-registry.js | head -1` (a code-enhance marker), and pick a unique literal from `search.js`, `mermaid.min.js`, `d3.min.js`, `plot.umd.min.js`, `base.css`, `site.css`, `dark.css`. Use those literals below in place of the `MARKER_*` placeholders (each MUST be a substring you verified exists via grep).

Add to the tests module in `crates/core/src/render/mod.rs`:

```rust
    #[test]
    fn shared_site_css_bundles_the_framework_sheets() {
        let css = shared_site_css();
        // MARKER_BASE / MARKER_DARK / MARKER_SITE are literals grepped from base.css /
        // dark.css / site.css respectively.
        assert!(css.contains(MARKER_BASE), "base.css missing");
        assert!(css.contains(MARKER_DARK), "dark.css missing");
        assert!(css.contains(MARKER_SITE), "site.css missing");
    }

    #[test]
    fn core_enhance_js_has_our_scripts_not_the_big_libs() {
        let js = core_enhance_js();
        assert!(js.contains(MARKER_CODE_ENHANCE), "code-enhance missing");
        assert!(js.contains(MARKER_SEARCH), "search.js missing");
        // The big vendored libs must NOT be in the always-on core bundle.
        assert!(!js.contains(MARKER_MERMAID_LIB), "mermaid lib leaked into core");
        assert!(!js.contains(MARKER_D3), "d3 leaked into core");
    }

    #[test]
    fn mermaid_and_jslibs_bundles_carry_their_libs() {
        assert!(mermaid_bundle_js().contains(MARKER_MERMAID_LIB), "mermaid lib missing");
        // The loader's CDN placeholder must be resolved, never left raw.
        assert!(!mermaid_bundle_js().contains("{{MERMAID}}"), "loader placeholder unresolved");
        let libs = js_cell_libs_js();
        assert!(libs.contains(MARKER_D3) && libs.contains(MARKER_PLOT), "d3/plot missing");
    }

    #[test]
    fn has_mermaid_detects_the_diagram_marker() {
        assert!(has_mermaid("<pre class=\"mermaid\">graph TD</pre>"));
        assert!(!has_mermaid("<p>no diagrams here</p>"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p taliesin-core --lib shared_site_css_bundles core_enhance_js_has mermaid_and_jslibs has_mermaid_detects`
Expected: FAIL (the accessors do not exist).

- [ ] **Step 3: Add the accessors**

In `crates/core/src/render/mod.rs`, near the existing asset consts, add:

```rust
/// The raw framework CSS a non-bare site page inlines in its main `<style>` (fonts +
/// base + dark + site chrome). Exposed so the multi-page build can externalize it into
/// one content-hashed `_assets/app.<hash>.css` instead of inlining a copy per page.
pub fn shared_site_css() -> String {
    format!("{FONTS_CSS}{BASE_CSS}{DARK_CSS}{SITE_CSS}")
}

/// The KaTeX stylesheet (base64 fonts inlined), for the externalized `katex.<hash>.css`.
pub fn katex_css_bytes() -> &'static str {
    KATEX_CSS
}

/// All of Taliesin's OWN page JS, concatenated for the always-on `app.<hash>.js`. Each
/// piece is separated by a bare `;` on its own line so concatenation is ASI-safe. The
/// big vendored libs (mermaid, d3, Plot) are deliberately excluded (their own files).
pub fn core_enhance_js() -> String {
    [
        CODE_ENHANCE_JS,
        TALIESIN_JS,
        WALKTHROUGH_JS,
        TABSET_JS,
        SCROLLY_JS,
        TOC_SPY_JS,
        TOC_SHEET_JS,
        SEARCH_JS,
    ]
    .join("\n;\n")
}

/// The vendored mermaid library plus its loader (CDN placeholder already resolved), for
/// the conditional `mermaid.<hash>.js`. Ships only on pages that have a diagram, so the
/// loader's never-reached CDN fallback stays off prose pages.
pub fn mermaid_bundle_js() -> String {
    format!(
        "{MERMAID_MIN_JS}\n;\n{}",
        MERMAID_JS.replace("{{MERMAID}}", &mermaid_url())
    )
}

/// The vendored d3 + Observable Plot globals for the conditional `jslibs.<hash>.js`
/// (ships only on pages with `{js}` cells).
pub fn js_cell_libs_js() -> String {
    format!("{D3_JS}\n;\n{PLOT_JS}")
}

/// True if a rendered body contains a mermaid diagram (gates the mermaid file link).
pub fn has_mermaid(body: &str) -> bool {
    body.contains("class=\"mermaid\"")
}
```

If any referenced const (e.g. `WALKTHROUGH_JS`) is not visible here, it is defined in the same module below; no import needed. Add `pub use` re-exports for the six new items in `crates/core/src/lib.rs` (or wherever `assemble_html_page`/`PageParts` are re-exported) so `taliesin-server` can call them.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-core --lib shared_site_css_bundles core_enhance_js_has mermaid_and_jslibs has_mermaid_detects`
Expected: PASS. Then confirm nothing else broke: `cargo test -p taliesin-core` (full) → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/render/mod.rs crates/core/src/lib.rs
git commit -m "feat(render): expose shared CSS/JS blob accessors for externalization"
```

---

### Task 2: `AssetMode` type + `assemble_html_page` External branch

**Files:**
- Modify: `crates/core/src/render/model.rs` (add `AssetMode` / `ExternalAssets`)
- Modify: `crates/core/src/render/page.rs` (`PageParts.assets` field; External branch in `assemble_html_page`)
- Modify: `crates/core/src/render/page.rs:448`, `crates/server/src/serve/mod.rs:676`, `crates/server/src/serve_site/mod.rs:625` (set `assets: AssetMode::Inline`)
- Modify: `crates/core/src/lib.rs` (re-export the new types)
- Test: `crates/core/src/render/page.rs` tests

**Interfaces:**
- Produces:
  ```rust
  pub enum AssetMode<'a> {
      Inline,
      External(ExternalAssets<'a>),
  }
  pub struct ExternalAssets<'a> {
      pub app_css: &'a str,     // href, e.g. "_assets/app.<hash>.css" or "../_assets/..."
      pub katex_css: &'a str,   // href; linked only when ship_katex
      pub app_js: &'a str,      // href; linked always
      pub mermaid_js: &'a str,  // href; linked only when has_mermaid(body)
      pub jslibs_js: &'a str,   // href; linked only when has_js_cells(body)
  }
  ```
  `PageParts` gains `pub assets: AssetMode<'a>`.
- Consumes: `has_js_cells`, `has_mermaid` (Task 1), the existing `KATEX_CSS`/`BASE_CSS` consts (only in the Inline branch).

- [ ] **Step 1: Write the failing test**

Add to `crates/core/src/render/page.rs` tests (build a minimal `PageParts` directly). Use a body containing the math + js + mermaid markers so the conditional links fire:

```rust
    #[test]
    fn external_assets_link_instead_of_inlining() {
        let ext = ExternalAssets {
            app_css: "_assets/app.aaaa.css",
            katex_css: "_assets/katex.bbbb.css",
            app_js: "_assets/app.cccc.js",
            mermaid_js: "_assets/mermaid.dddd.js",
            jslibs_js: "_assets/jslibs.eeee.js",
        };
        let body = "<main id=\"tali-main\"><span class=\"katex\">x</span>\
                    <pre class=\"mermaid\">g</pre>\
                    <script type=\"application/qmd-js\">1</script></main>";
        let html = assemble_html_page(&PageParts {
            mode: OutputMode::Build,
            title: "T",
            lang: "en",
            favicon: "",
            theme_default: "dark",
            theme_css: "",
            with_site_css: true,
            ship_katex: true,
            extra_head: "",
            body_class: "",
            include_in_header: "",
            include_before_body: "",
            body,
            scripts_pre: "",
            scripts_post: "",
            include_after_body: "",
            assets: AssetMode::External(ext),
        });
        // Links, not inlined framework CSS.
        assert!(html.contains("<link rel=\"stylesheet\" href=\"_assets/app.aaaa.css\">"));
        assert!(html.contains("href=\"_assets/katex.bbbb.css\""));
        assert!(!html.contains(MARKER_BASE), "framework CSS must not be inlined in External mode");
        // Scripts as deferred external refs.
        assert!(html.contains("<script src=\"_assets/app.cccc.js\" defer></script>"));
        assert!(html.contains("src=\"_assets/mermaid.dddd.js\" defer"));
        assert!(html.contains("src=\"_assets/jslibs.eeee.js\" defer"));
    }

    #[test]
    fn external_omits_conditional_links_when_absent() {
        let ext = ExternalAssets {
            app_css: "a.css", katex_css: "k.css", app_js: "a.js",
            mermaid_js: "m.js", jslibs_js: "j.js",
        };
        let html = assemble_html_page(&PageParts {
            mode: OutputMode::Build, title: "T", lang: "en", favicon: "",
            theme_default: "dark", theme_css: "", with_site_css: true,
            ship_katex: false, extra_head: "", body_class: "",
            include_in_header: "", include_before_body: "",
            body: "<main id=\"tali-main\"><p>prose only</p></main>",
            scripts_pre: "", scripts_post: "", include_after_body: "",
            assets: AssetMode::External(ext),
        });
        assert!(html.contains("href=\"a.css\""), "app.css always linked");
        assert!(html.contains("src=\"a.js\" defer"), "app.js always linked");
        assert!(!html.contains("k.css"), "no katex link on a math-free page");
        assert!(!html.contains("m.js"), "no mermaid link on a diagram-free page");
        assert!(!html.contains("j.js"), "no jslibs link on a {js}-free page");
    }
```

(`MARKER_BASE` = the same base.css literal used in Task 1.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p taliesin-core --lib external_assets_link_instead external_omits_conditional`
Expected: FAIL to COMPILE (`assets` field + `AssetMode` do not exist).

- [ ] **Step 3: Add the types + field + External branch**

In `crates/core/src/render/model.rs`, add:

```rust
/// How a page's framework CSS/JS is delivered. `Inline` bakes every blob into the page
/// (the portable single-file build, `--bare`, and live preview). `External` links to
/// content-hashed shared files under `_assets/` (the multi-page `build <dir>` path).
pub enum AssetMode<'a> {
    Inline,
    External(ExternalAssets<'a>),
}

/// Depth-adjusted hrefs for the shared `_assets/` files, supplied per page by the build.
pub struct ExternalAssets<'a> {
    pub app_css: &'a str,
    pub katex_css: &'a str,
    pub app_js: &'a str,
    pub mermaid_js: &'a str,
    pub jslibs_js: &'a str,
}
```

Re-export both from `crates/core/src/lib.rs` alongside `PageParts`.

In `crates/core/src/render/page.rs`, add the field to `PageParts`:

```rust
    pub include_after_body: &'a str,
    /// How framework CSS/JS is delivered (inline blobs, or links to `_assets/`).
    pub assets: AssetMode<'a>,
```

In `assemble_html_page`, replace the head-CSS + script assembly with a branch on `p.assets`. Keep the existing Inline branch exactly as it is today; add the External branch. The cleanest shape: compute the five head/script fragments once, then interpolate them into the SAME template string.

```rust
    // The head CSS block + framework script tags differ by asset mode; the body frame,
    // skip link, theme bootstrap, and passed-in pre/post scripts are identical.
    let (style_block, katex_block, js_head_html, framework_scripts) = match &p.assets {
        AssetMode::Inline => {
            // ... exactly today's logic: `<style>{fonts}{base}{dark}{site}{bare_theme}</style>`,
            // the `{katex}` inline block, `js_head` from js_cell_head(), and
            // `code_scripts_for(p.body, p.mode)`. (Move the existing computations here
            // unchanged and return them as the 4-tuple.)
        }
        AssetMode::External(a) => {
            let style_block = format!("<link rel=\"stylesheet\" href=\"{}\">", a.app_css);
            let katex_block = if p.ship_katex {
                format!("\n<link rel=\"stylesheet\" href=\"{}\">", a.katex_css)
            } else {
                String::new()
            };
            let js_head_html = if !bare && has_js_cells(p.body) {
                format!("<script src=\"{}\" defer></script>", a.jslibs_js)
            } else {
                String::new()
            };
            let mermaid = if has_mermaid(p.body) {
                format!("\n<script src=\"{}\" defer></script>", a.mermaid_js)
            } else {
                String::new()
            };
            let framework_scripts =
                format!("<script src=\"{}\" defer></script>{mermaid}", a.app_js);
            (style_block, katex_block, js_head_html, framework_scripts)
        }
    };
```

Then update the `format!` template so it interpolates `{style_block}` where the inline `<style>…</style>{katex}` was, `{js_head}` = `js_head_html`, and `{code_scripts}` = `framework_scripts`. In the Inline branch, `style_block` is the full `<style>…</style>` and `katex_block` is the `{katex}` string, so the template reads `{style_block}{katex_block}`. Preserve every other token (`theme_init`, `theme_css`, `scripts_pre`, `scripts_post`, etc.) exactly.

Note: `bare` (`p.mode == OutputMode::Bare`) never combines with `External` (a site is never bare), but the `!bare` guard on `js_head_html` is kept for symmetry.

- [ ] **Step 4: Set `assets: AssetMode::Inline` on the three existing constructors**

- `crates/core/src/render/page.rs:448` (`html_page_inner`): add `assets: AssetMode::Inline,` to the `PageParts { … }` (Task 3 overrides this for the site-external entry).
- `crates/server/src/serve/mod.rs:676`: add `assets: taliesin_core::AssetMode::Inline,`.
- `crates/server/src/serve_site/mod.rs:625`: add `assets: taliesin_core::AssetMode::Inline,`.

- [ ] **Step 5: Run tests + confirm inline is byte-identical**

Run: `cargo test -p taliesin-core --lib external_assets_link_instead external_omits_conditional` → PASS.
Run the full core suite + the body-snapshot suite (the Inline path must be unchanged):
`cargo test -p taliesin-core` and `cargo test -p taliesin-core --test corpus` → PASS with no snapshot drift. If a snapshot changed, the Inline branch was altered: fix it to reproduce today's bytes exactly.
Run `cargo build -p taliesin-server` → the two preview constructors compile with the new field.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/model.rs crates/core/src/render/page.rs crates/core/src/lib.rs crates/server/src/serve/mod.rs crates/server/src/serve_site/mod.rs
git commit -m "feat(render): AssetMode::External emits _assets links in place of inline blobs"
```

---

### Task 3: External site-render entry (`html_page_inner` + `render_page_doc_external`)

**Files:**
- Modify: `crates/core/src/render/page.rs` (`html_page_inner` gains an `assets` param; drop `toc_scripts` from `scripts_post` in External; add `html_page_from_doc_in_site_external`)
- Modify: `crates/core/src/site/mod.rs` (add `render_page_doc_external`)
- Modify: `crates/core/src/lib.rs` (re-export `html_page_from_doc_in_site_external`)
- Test: `crates/core/src/site/mod.rs` tests (or `render/tests.rs`)

**Interfaces:**
- Produces:
  - `pub fn html_page_from_doc_in_site_external(doc: &RenderedDoc, fallback_title: &str, site: &SiteCtx, assets: ExternalAssets) -> String`
  - `Site::render_page_doc_external(&self, page: &Page, doc: RenderedDoc, assets: ExternalAssets) -> (String, Vec<Warning>)`
- Consumes: `AssetMode`/`ExternalAssets` (Task 2), the accessors (Task 1). Consumed by Task 5 (the build).

- [ ] **Step 1: Write the failing test**

The key behavior: in External mode the per-page search index stays inline, but the shared `toc_scripts()` (toc-spy/sheet/search code) is NOT re-inlined (it lives in `app.js`). Add to `crates/core/src/site/mod.rs` tests a small site page with a TOC, rendered External, asserting the search-index inline script is present but the toc-spy code is not:

```rust
    #[test]
    fn external_site_render_keeps_search_index_inline_drops_shared_toc_js() {
        // Build a tiny site + a TOC page (mirror the existing site-render test helpers in
        // this module; if a helper like `tiny_site()` exists, reuse it).
        // Render External:
        let ext = render::ExternalAssets {
            app_css: "_assets/app.a.css", katex_css: "_assets/katex.b.css",
            app_js: "_assets/app.c.js", mermaid_js: "_assets/mermaid.d.js",
            jslibs_js: "_assets/jslibs.e.js",
        };
        let (html, _w) = site.render_page_doc_external(&page, doc, ext);
        // app.js is linked (carries the toc/search code now).
        assert!(html.contains("src=\"_assets/app.c.js\" defer"));
        // The shared toc-spy code is NOT inlined again (MARKER_TOC_SPY is a literal from
        // toc-spy.js).
        assert!(!html.contains(MARKER_TOC_SPY), "toc-spy code must not be re-inlined");
        // The per-page search index (inline data) is still present when the site has one.
        // (Assert on a stable token the search index script emits, if the fixture has >1 page.)
    }
```

If the module lacks a ready site-render test helper, construct the smallest `Site` + `Page` the existing tests use; do not invent a new fixture style.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p taliesin-core --lib external_site_render_keeps_search_index`
Expected: FAIL to compile (`render_page_doc_external` does not exist).

- [ ] **Step 3: Thread `assets` through `html_page_inner`**

Change `html_page_inner` to take an `assets: AssetMode` param and pass it into `PageParts`. In the `toc_script` computation (page.rs ~337-347), when `assets` is `External`, drop the `toc_scripts()` tail (it is now in `app.js`), keeping only the optional per-page search index:

```rust
    let toc_script = if toc.is_empty() {
        String::new()
    } else {
        let index = site
            .map(|s| s.search_index.as_str())
            .filter(|s| !s.is_empty())
            .map(|idx| format!("<script>{idx}</script>\n"))
            .unwrap_or_default();
        match assets {
            // Inline: ship the per-page index (if any) followed by the shared toc/search JS.
            AssetMode::Inline => format!("{index}{}", toc_scripts()),
            // External: the shared toc/search JS is in app.js; keep only the per-page index.
            AssetMode::External(_) => index,
        }
    };
```

Keep `scripts_post: &format!("{STATIC_ENHANCE}\n{toc_script}")` as is (STATIC_ENHANCE stays inline; it is DOMContentLoaded-wrapped so it runs after the deferred `app.js` defines `taliEnhanceCode`).

Update the two existing wrappers:
- `html_page_from_doc_in_site` calls `html_page_inner(doc, title, Some(site), OutputMode::Build, AssetMode::Inline)`.
- `page_from_doc` (page.rs:7) and any other caller pass `AssetMode::Inline`.

Add the new entry:

```rust
/// Like [`html_page_from_doc_in_site`] but links the shared `_assets/` files instead of
/// inlining the framework CSS/JS. Used by the multi-page `build <dir>` path.
pub fn html_page_from_doc_in_site_external(
    doc: &RenderedDoc,
    fallback_title: &str,
    site: &SiteCtx,
    assets: ExternalAssets,
) -> String {
    html_page_inner(doc, fallback_title, Some(site), OutputMode::Build, AssetMode::External(assets))
}
```

- [ ] **Step 4: Add `render_page_doc_external` to `Site`**

In `crates/core/src/site/mod.rs`, mirror `render_page_doc_warned` (L518) but call the external entry:

```rust
    /// Render a page linking the shared `_assets/` bundle (the multi-page build path).
    /// Identical to [`Self::render_page_doc_warned`] except for the asset delivery.
    pub fn render_page_doc_external(
        &self,
        page: &Page,
        mut doc: render::RenderedDoc,
        assets: render::ExternalAssets,
    ) -> (String, Vec<Warning>) {
        doc.toc = self.page_toc(page, doc.toc_explicit, &doc.blocks);
        let mut warnings = std::mem::take(&mut doc.warnings);
        self.finish_blocks(page, &mut doc.blocks, &mut warnings);
        let ctx = self.page_chrome(page);
        let fallback = page.title.as_deref().unwrap_or("");
        let html = render::html_page_from_doc_in_site_external(&doc, fallback, &ctx, assets);
        (rewrite_qmd_links(&html), warnings)
    }
```

Re-export `html_page_from_doc_in_site_external` + `ExternalAssets` from the crate root.

- [ ] **Step 5: Run tests**

Run: `cargo test -p taliesin-core --lib external_site_render_keeps_search_index` → PASS.
Run: `cargo test -p taliesin-core` and `cargo test -p taliesin-core --test corpus` → PASS (Inline path unchanged; no snapshot drift).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/page.rs crates/core/src/site/mod.rs crates/core/src/lib.rs
git commit -m "feat(render): external site-render entry (drops re-inlined toc/search JS)"
```

---

### Task 4: Conservative build-time minifier (`crates/server/src/minify.rs`)

**Files:**
- Create: `crates/server/src/minify.rs`
- Modify: `crates/server/src/main.rs` (add `mod minify;`)
- Test: in `crates/server/src/minify.rs`

**Interfaces:**
- Produces: `pub fn minify_css(src: &str) -> String`, `pub fn minify_js(src: &str) -> String`. Consumed by Task 5.

- [ ] **Step 1: Write the failing tests**

Create `crates/server/src/minify.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_strips_comments_and_collapses_space_but_spares_strings() {
        let out = minify_css("/* c */ a  {  color : red ;  }\n.b{content:\"  keep  \"}");
        assert!(!out.contains("/*"), "comment stripped");
        assert!(!out.contains("  "), "runs of space collapsed");
        assert!(out.contains("\"  keep  \""), "string content preserved");
    }

    #[test]
    fn css_preserves_data_uri_in_url() {
        let src = ".x{background:url(data:image/png;base64,AA  BB)}";
        // Whitespace inside url(...) with a data: URI must not be mangled.
        assert!(minify_css(src).contains("data:image/png;base64,AA  BB"));
    }

    #[test]
    fn js_preserves_newlines_for_asi_and_strips_comments() {
        let src = "let a = 1 // trailing\n/* block */\nlet b = 2\n";
        let out = minify_js(src);
        assert!(!out.contains("//"), "line comment stripped");
        assert!(!out.contains("/*"), "block comment stripped");
        // Newlines between statements are preserved (ASI safety).
        assert!(out.matches('\n').count() >= 1, "statement newline kept");
    }

    #[test]
    fn js_does_not_strip_comment_markers_inside_strings_or_regex() {
        assert!(minify_js("let u = \"http://x\"\n").contains("http://x"));
        assert!(minify_js("let re = /a\\/\\/b/\n").contains("/a\\/\\/b/"));
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p taliesin-server --bin taliesin minify`
Expected: FAIL (module empty / functions missing). Add `mod minify;` to `main.rs` first if the module is not found.

- [ ] **Step 3: Implement the two minifiers**

Write conservative state machines. `minify_css`: track `"`/`'` strings and `/* */` comments; outside them, drop comments and collapse runs of ASCII whitespace to a single space; do NOT special-case `{}:;` beyond whitespace collapse (safe and simple). Critically, treat everything inside a string literal as opaque, and do not descend into `url(...)` specially (whitespace inside an unquoted `url(data:...)` is preserved because CSS whitespace-collapse there is only cosmetic and the test pins the base64 stays intact; if the collapse would touch it, guard `url(` ... `)` as opaque too).

`minify_js`: track `"`/`'`/`` ` `` strings, `/* */` and `//` comments, and regex literals (a `/` is a regex only when the previous non-space token is one that cannot end an expression: `(`, `,`, `=`, `:`, `[`, `!`, `&`, `|`, `?`, `{`, `}`, `;`, `return`, etc.; otherwise it is division). Outside strings/regex/comments: strip comments, strip fully-blank lines and leading indentation, but PRESERVE every remaining newline. Do not collapse intra-line spaces (keep it ultra-safe). Provide the full implementation:

```rust
//! Conservative, dependency-free minifiers for the build-time shared asset bundle. CSS
//! collapses whitespace + strips comments (string/`url()`-aware). JS strips comments and
//! blank-line indentation but PRESERVES newlines (ASI-safe) and never mangles tokens; it
//! runs only on Taliesin's own hand-written JS (vendored `*.min.js` bypass it entirely).

pub fn minify_css(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut last_was_space = false;
    while i < b.len() {
        // string literal: copy verbatim
        if b[i] == b'"' || b[i] == b'\'' {
            let q = b[i];
            out.push(q as char);
            i += 1;
            while i < b.len() {
                out.push(b[i] as char);
                if b[i] == b'\\' && i + 1 < b.len() {
                    out.push(b[i + 1] as char);
                    i += 2;
                    continue;
                }
                if b[i] == q {
                    i += 1;
                    break;
                }
                i += 1;
            }
            last_was_space = false;
            continue;
        }
        // url(...) : copy verbatim through the matching ')'
        if src[i..].starts_with("url(") {
            let end = src[i..].find(')').map(|e| i + e + 1).unwrap_or(b.len());
            out.push_str(&src[i..end]);
            i = end;
            last_was_space = false;
            continue;
        }
        // comment
        if src[i..].starts_with("/*") {
            let end = src[i + 2..].find("*/").map(|e| i + 2 + e + 2).unwrap_or(b.len());
            i = end;
            continue;
        }
        if b[i].is_ascii_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
            i += 1;
            continue;
        }
        out.push(b[i] as char);
        last_was_space = false;
        i += 1;
    }
    out.trim().to_string()
}

pub fn minify_js(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let stripped = strip_line_comment(line);
        let trimmed = stripped.trim_end();
        let lead_trimmed = trimmed.trim_start();
        if lead_trimmed.is_empty() {
            continue; // drop blank lines
        }
        out.push_str(lead_trimmed);
        out.push('\n'); // preserve the statement newline (ASI)
    }
    out
}

/// Remove a `//` line comment from `line`, honoring `"`/`'`/backtick strings and a
/// naive regex guard, and pass `/* ... */` starts through (block comments are rare in
/// our sources and multi-line ones are left intact rather than mishandled). Returns the
/// code portion of the line.
fn strip_line_comment(line: &str) -> &str {
    let b = line.as_bytes();
    let mut i = 0;
    let mut in_str: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        match in_str {
            Some(q) => {
                if c == b'\\' {
                    i += 2;
                    continue;
                }
                if c == q {
                    in_str = None;
                }
            }
            None => {
                if c == b'"' || c == b'\'' || c == b'`' {
                    in_str = Some(c);
                } else if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
                    return &line[..i];
                }
            }
        }
        i += 1;
    }
    line
}
```

Note the deliberate conservatism: `minify_js` does not attempt regex-literal parsing or multi-line block-comment removal (our sources put block comments on their own lines, which the blank-line drop handles once the `/* */` is single-line; a multi-line block comment is left intact, which is safe). This is correct-over-clever by design.

If the `js_does_not_strip_comment_markers_inside_strings_or_regex` regex test fails because `strip_line_comment` lacks a regex guard, extend `strip_line_comment` minimally so a `/` that is preceded (ignoring spaces) by `(`, `,`, `=`, `:`, `[`, `!`, `return`, `&`, `|`, `?`, `;`, `{`, `}` starts a regex copied verbatim to its closing unescaped `/`. Keep it minimal.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p taliesin-server --bin taliesin minify`
Expected: PASS (all four).

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/minify.rs crates/server/src/main.rs
git commit -m "feat(build): conservative dependency-free CSS/JS minifier"
```

---

### Task 5: Build `_assets/` emit + External page render + stale sweep

**Files:**
- Modify: `crates/server/src/build.rs` (the site build path: `build_site_async` / `build_one_page` around L760-863, and the writer)
- Test: integration in `crates/server/tests/asset_bundle.rs` (new; a lightweight `build <dir>` over a small fixture, so the corpus pin in Task 6 can stay focused)

**Interfaces:**
- Consumes: Task 1 accessors, Task 3 `render_page_doc_external`, Task 4 minifiers, `taliesin_core::hash::fnv1a`.
- Produces: `<out>/_assets/{app,katex}.<hash>.css` + `<out>/_assets/{app,mermaid,jslibs}.<hash>.js`; every page links them at depth-correct hrefs.

- [ ] **Step 1: Write the failing integration test**

Create `crates/server/tests/asset_bundle.rs`:

```rust
use std::process::Command;
fn bin() -> &'static str { env!("CARGO_BIN_EXE_taliesin") }

#[test]
fn site_build_externalizes_shared_assets() {
    let root = std::env::temp_dir().join(format!("tali-ab-src-{}", std::process::id()));
    let out = std::env::temp_dir().join(format!("tali-ab-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("_site.yml"), "title: AB\n").unwrap();
    std::fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
    std::fs::write(root.join("sub/page.tmd"), "---\ntitle: Sub\n---\n\nMath $x=1$.\n").unwrap();
    let ok = Command::new(bin()).args(["build"]).arg(&root).arg("--out").arg(&out)
        .output().expect("build");
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));

    // The shared files exist.
    let assets = std::fs::read_dir(out.join("_assets")).expect("_assets dir").flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>();
    assert!(assets.iter().any(|n| n.starts_with("app.") && n.ends_with(".css")), "{assets:?}");
    assert!(assets.iter().any(|n| n.starts_with("app.") && n.ends_with(".js")), "{assets:?}");

    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    let sub = std::fs::read_to_string(out.join("sub/page.html")).unwrap();
    // Dedup: both pages reference the SAME hashed app.css filename.
    let app_css = assets.iter().find(|n| n.starts_with("app.") && n.ends_with(".css")).unwrap();
    assert!(index.contains(&format!("_assets/{app_css}")), "root links app.css");
    assert!(sub.contains(&format!("../_assets/{app_css}")), "nested page uses ../ prefix");
    // No inlined framework CSS on the page (MARKER_BASE literal from base.css).
    assert!(!index.contains(MARKER_BASE), "framework CSS must not be inlined");
    // katex is conditional: the math sub-page links it, the prose home does not.
    assert!(sub.contains("_assets/katex."), "math page links katex");
    assert!(!index.contains("katex."), "prose page does not link katex");

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}
```

(`MARKER_BASE` = the base.css literal; define it as a `const` at the top of the test file.)

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p taliesin-server --test asset_bundle`
Expected: FAIL (no `_assets/` dir; pages still inline the CSS).

- [ ] **Step 3: Implement the `_assets/` emit + external render**

In `build.rs`'s site path, before rendering pages, compute the bundle once and write it. Add a helper:

```rust
/// The resolved shared-asset filenames (content-hashed), computed once per site build.
struct AssetBundle {
    app_css: String,   // "_assets/app.<hash>.css"
    katex_css: String,
    app_js: String,
    mermaid_js: String,
    jslibs_js: String,
}

/// Minify + content-hash each shared blob, write it once under `<out>/_assets/`, and
/// return the (root-relative) filenames. Clears any stale `_assets/` first so old hashes
/// do not accumulate.
fn write_asset_bundle(out: &Path) -> std::io::Result<AssetBundle> {
    use taliesin_core::hash::fnv1a;
    let dir = out.join("_assets");
    let _ = std::fs::remove_dir_all(&dir); // own the lifecycle; clear stale hashes
    std::fs::create_dir_all(&dir)?;
    let named = |stem: &str, ext: &str, bytes: &str| -> std::io::Result<String> {
        let name = format!("{stem}.{:x}.{ext}", fnv1a(bytes));
        std::fs::write(dir.join(&name), bytes)?;
        Ok(format!("_assets/{name}"))
    };
    let app_css = named("app", "css", &crate::minify::minify_css(&taliesin_core::shared_site_css()))?;
    let katex_css = named("katex", "css", &crate::minify::minify_css(taliesin_core::katex_css_bytes()))?;
    let app_js = named("app", "js", &crate::minify::minify_js(&taliesin_core::core_enhance_js()))?;
    // Vendored libs are already minified: hash + write as-is (do not re-minify).
    let mermaid_js = named("mermaid", "js", &taliesin_core::mermaid_bundle_js())?;
    let jslibs_js = named("jslibs", "js", &taliesin_core::js_cell_libs_js())?;
    Ok(AssetBundle { app_css, katex_css, app_js, mermaid_js, jslibs_js })
}

/// Rebase a root-relative `_assets/...` href for a page at `page_url` (e.g. `sub/p.html`
/// gets `../_assets/...`; a root page keeps `_assets/...`).
fn asset_href(page_url: &str, root_rel: &str) -> String {
    let depth = page_url.matches('/').count();
    format!("{}{root_rel}", "../".repeat(depth))
}
```

Thread `&AssetBundle` into `build_one_page` (add a param). Replace the render call:

```rust
    let (html, render_warnings) = site.render_page_doc_warned(page, doc);
```

with:

```rust
    let ext = taliesin_core::ExternalAssets {
        app_css: &asset_href(&page.url, &bundle.app_css),
        katex_css: &asset_href(&page.url, &bundle.katex_css),
        app_js: &asset_href(&page.url, &bundle.app_js),
        mermaid_js: &asset_href(&page.url, &bundle.mermaid_js),
        jslibs_js: &asset_href(&page.url, &bundle.jslibs_js),
    };
    let (html, render_warnings) = site.render_page_doc_external(page, doc, ext);
```

Call `write_asset_bundle(out)` once in `build_site_async` before the per-page fan-out, propagate its error as a build failure, and pass the resulting `&AssetBundle` into every `build_one_page`. (Bind the `asset_href` results to `let` locals before building `ExternalAssets` so the `&str`s live long enough.)

Also confirm the embed-deck build path at `build.rs:1106` (`render_doc_to_page(..., Build)`) is a single-doc render and stays Inline: an `{{< embed >}}` deck is a standalone page, so it keeps inlining. Leave it unchanged.

- [ ] **Step 4: Run the test + full build sanity**

Run: `cargo test -p taliesin-server --test asset_bundle` → PASS.
Run: `cargo build -p taliesin-server` then build a real corpus site and eyeball `_assets/`:
`cargo run -p taliesin-server -- build corpus/tech-blog --out /tmp/ab-tech && ls -la /tmp/ab-tech/_assets && grep -c "_assets/app" /tmp/ab-tech/index.html`
Expected: five `_assets/*` files; `index.html` references `_assets/app.*`.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/build.rs crates/server/tests/asset_bundle.rs
git commit -m "feat(build): emit content-hashed _assets bundle + render pages external"
```

---

### Task 6: Corpus pin (`corpus/tech-blog` dedup + conditional links)

**Files:**
- Test: `crates/server/tests/tech_blog.rs` (add a test) or extend `crates/server/tests/asset_bundle.rs`

**Interfaces:** consumes the Task 5 behavior; pins it against the real corpus doc per the scope policy.

- [ ] **Step 1: Write the pin**

Add a test that builds `corpus/tech-blog` (a real multi-page blog) to a temp dir and asserts: `_assets/app.<hash>.css` + `.js` exist; two different pages reference the same `app.<hash>.css`; no page inlines the framework `<style>` (MARKER_BASE absent from page HTML, present in the shared file); a math page links `katex.<hash>.css` while a prose page does not; a nested page's href carries the `../` prefix. Mirror the harness in `tech_blog.rs` (it already builds the corpus blog).

```rust
#[test]
fn tech_blog_shares_one_hashed_css_across_pages() {
    // build corpus/tech-blog to a temp out (reuse this file's existing build helper if present)
    // ... assert dedup + no-inline + conditional katex as described above ...
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p taliesin-server --test tech_blog tech_blog_shares_one_hashed_css`
Expected: PASS. If the corpus blog has no math page, drop the katex half of the assertion (or point it at a page you confirm has math) and note which page you used.

- [ ] **Step 3: Commit**

```bash
git add crates/server/tests/tech_blog.rs
git commit -m "test(build): pin tech-blog shared-asset dedup + conditional katex"
```

---

### Task 7: Browser verification (behavior parity across page types)

**Files:** none (verification task); record findings in the report.

This is the safety net for the one real behavior change: our own enhancers now load as one deferred `app.js` on every page (including pages that did not previously ship toc/search/{js}-enhancer). Confirm zero regressions.

- [ ] **Step 1: Build + serve the corpus blog**

Run: `cargo run -p taliesin-server -- build corpus/tech-blog --out /tmp/ab-verify` then serve it statically (e.g. `python3 -m http.server` from `/tmp/ab-verify`, or `taliesin preview corpus/tech-blog` for the live view; note preview is Inline so also spot-check a built page over the static server for the External path).

- [ ] **Step 2: Drive it with chrome-devtools MCP at three viewports**

For each of: a prose-only post, a post with math, a post with a mermaid diagram, and a page with `{js}` cells (the marketing "see it live" demos, or a corpus doc that has them), load the BUILT (External) page and confirm via the chrome-devtools MCP:
- The console has ZERO errors (especially: no "taliEnhanceCode is not defined", no failed `_assets/` requests, no mermaid CDN request on a prose page).
- Copy buttons, lightbox, the reader menu, KaTeX rendering, a mermaid diagram, and a `{js}` cell chart all still work.
- `_assets/app.*.css` and `app.*.js` load once (Network panel) and are shared across navigations (same URL, served from cache on the second page).
Check at ~390x844, ~1440x900, and ~900x1440 (per the project viewport matrix).

- [ ] **Step 3: Record evidence**

In the task report, list each page type x viewport, the console result (quote any error), and a note that the shared files were cache-hits on the second page. Any console error or broken feature is a blocker: diagnose (most likely a script that is not no-op-safe when loaded without its DOM, or a defer-ordering issue) and fix in `core_enhance_js` / the emission before completing.

- [ ] **Step 4: Full workspace verification + commit (if evidence files were added)**

Run: `cargo test -p taliesin-core -p taliesin-server` → PASS; `cargo clippy --workspace --all-targets -- -D warnings` → clean; `cargo fmt --check` → clean. No commit unless verification artifacts were saved.

---

## Self-Review Notes

- **Spec coverage:** file layout (Task 5 `write_asset_bundle`); AssetMode threading (Tasks 2-3); minifier (Task 4, CSS collapse + JS ASI-safe, vendored libs bypass); KaTeX base64 kept in `katex.<hash>.css` (Task 5 writes `katex_css_bytes()` through `minify_css`, fonts stay inline); `_assets/` reserved dir + stale clear (Task 5); depth prefix (Task 5 `asset_href`); dedup + conditional-link + no-inline + nested-depth pins (Tasks 5-6); Inline byte-identical guard (Tasks 2-3 corpus snapshots); mermaid loader kept off prose pages (Task 1 puts the loader only in `mermaid_bundle_js`, Task 2 links it only when `has_mermaid`); the `defer` correctness pin + the toc/search fold behavior change (Task 7 browser check). Mounts each get their own `_assets/` (Task 5 writes under each build `out`; cross-mount dedup out of scope, per spec).
- **Placeholder scan:** the only non-literal tokens are `MARKER_*` (deliberate: each is a substring the implementer greps from a bundled asset and pastes in Step 1) and the two "mirror the existing helper" test-fixture notes (Tasks 3, 6) where the module's own test style must be followed rather than a fresh fixture invented. All production code is complete.
- **Type consistency:** `ExternalAssets` fields (`app_css`/`katex_css`/`app_js`/`mermaid_js`/`jslibs_js`) are identical across Tasks 2, 3, 5. `AssetMode::{Inline,External}` used consistently. Accessor names (`shared_site_css`, `katex_css_bytes`, `core_enhance_js`, `mermaid_bundle_js`, `js_cell_libs_js`, `has_mermaid`) match between Task 1 (definition) and Tasks 4-5 (calls). `render_page_doc_external` signature matches between Task 3 (definition) and Task 5 (call).
