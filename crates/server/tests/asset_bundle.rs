//! A multi-page `build <dir>` externalizes the shared framework CSS/JS into content-hashed
//! `_assets/` files instead of inlining a copy into every page: pages link them (depth-
//! adjusted), and math/mermaid/`{js}`-cell libs stay conditional per page.

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_taliesin")
}

// The base.css marker literal (confirmed present via grep; see render/tests.rs's own
// comment on this same literal): a framework rule that must never be inlined into a page
// under `AssetMode::External`.
const MARKER_BASE: &str = ".tali-reader-seg";

#[test]
fn site_build_externalizes_shared_assets() {
    let root = std::env::temp_dir().join(format!("tali-ab-src-{}", std::process::id()));
    let out = std::env::temp_dir().join(format!("tali-ab-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("_site.yml"), "title: AB\n").unwrap();
    std::fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
    std::fs::write(
        root.join("sub/page.tmd"),
        "---\ntitle: Sub\n---\n\nMath $x=1$.\n",
    )
    .unwrap();
    let ok = Command::new(bin())
        .args(["build"])
        .arg(&root)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build");
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    // The shared files exist.
    let assets = std::fs::read_dir(out.join("_assets"))
        .expect("_assets dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(
        assets
            .iter()
            .any(|n| n.starts_with("app.") && n.ends_with(".css")),
        "{assets:?}"
    );
    assert!(
        assets
            .iter()
            .any(|n| n.starts_with("app.") && n.ends_with(".js")),
        "{assets:?}"
    );

    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    let sub = std::fs::read_to_string(out.join("sub/page.html")).unwrap();
    // Dedup: both pages reference the SAME hashed app.css filename.
    let app_css = assets
        .iter()
        .find(|n| n.starts_with("app.") && n.ends_with(".css"))
        .unwrap();
    assert!(
        index.contains(&format!("_assets/{app_css}")),
        "root links app.css"
    );
    assert!(
        sub.contains(&format!("../_assets/{app_css}")),
        "nested page uses ../ prefix"
    );
    // No inlined framework CSS on the page (MARKER_BASE literal from base.css).
    assert!(
        !index.contains(MARKER_BASE),
        "framework CSS must not be inlined"
    );
    // katex is conditional: the math sub-page links it, the prose home does not.
    assert!(sub.contains("_assets/katex."), "math page links katex");
    assert!(!index.contains("katex."), "prose page does not link katex");

    // The built-in `404.html` links the same bundle — it was the one page in a build still
    // assembled by the INLINE renderer, so a site of ~26 KB pages shipped a 356 KB 404.
    let not_found = std::fs::read_to_string(out.join("404.html")).unwrap();
    assert!(
        !not_found.contains(MARKER_BASE),
        "the 404 page must not inline the framework CSS either"
    );
    // ROOT-ABSOLUTE, unlike every other page: a static host serves this one file for any
    // unknown path, so a depth-relative href resolves against the directory the reader
    // guessed at. This is the same rule the page's `/` home link already follows, and it is
    // the reason the 404 cannot simply reuse a page's depth-adjusted hrefs.
    assert!(
        not_found.contains(&format!("href=\"/_assets/{app_css}\"")),
        "404 links the shared css root-absolutely"
    );
    assert!(
        !not_found.contains("href=\"../") && !not_found.contains("src=\"../"),
        "no depth-relative reference may appear in a page served at arbitrary depth"
    );
    assert!(
        not_found.len() < index.len(),
        "the 404 is the smallest page in the build, not the largest: {} vs {}",
        not_found.len(),
        index.len()
    );

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

/// Same behavior, pinned against the REAL `corpus/tech-blog` instead of a synthetic
/// two-page fixture: dedup, no-inline, conditional katex, and a depth-relative href all
/// hold once real content (math-dense posts, code cells, a prose-only CV) is in the mix.
///
/// Mermaid is NOT exercised here: no corpus document contains a mermaid diagram (`grep -rl
/// 'class="mermaid"\|```mermaid' corpus/` finds nothing), so the mermaid-conditional link
/// stays pinned only by the unit test `external_assets_link_instead_of_inlining` in
/// `crates/core/src/render/page.rs`, not by a corpus build.
#[test]
fn tech_blog_shares_one_hashed_css_across_pages() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/tech-blog");
    let out = std::env::temp_dir().join(format!("tali-ab-techblog-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);

    let ok = Command::new(bin())
        .args(["build"])
        .arg(&src)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build tech-blog");
    let stderr = String::from_utf8_lossy(&ok.stderr);
    // tech-blog has `{python}` cells, so on a machine with no ipykernel the build now
    // fails by design ("executable cells but no kernel"). That failure is not this test's
    // subject: the site is still fully written first (same shape as `--strict`), so every
    // asset assertion below holds either way. Tolerated *narrowly* — any other failure is
    // still a failure, and with a kernel present (`tools/gates.sh`) this is a plain
    // success assertion. `--no-exec` is deliberately NOT used here: it would also stop the
    // `{js}` cells this corpus contains from running, changing which conditional `_assets`
    // blobs get linked — i.e. changing the very thing under test.
    assert!(
        ok.status.success() || stderr.contains("no python kernel available"),
        "{stderr}"
    );

    let assets = std::fs::read_dir(out.join("_assets"))
        .expect("_assets dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let app_css = assets
        .iter()
        .find(|n| n.starts_with("app.") && n.ends_with(".css"))
        .unwrap_or_else(|| panic!("no app.<hash>.css in {assets:?}"))
        .clone();
    let app_js = assets
        .iter()
        .find(|n| n.starts_with("app.") && n.ends_with(".js"))
        .unwrap_or_else(|| panic!("no app.<hash>.js in {assets:?}"));
    assert!(
        std::fs::metadata(out.join("_assets").join(&app_css))
            .unwrap()
            .len()
            > 0,
        "app.<hash>.css is empty"
    );
    assert!(
        std::fs::metadata(out.join("_assets").join(app_js))
            .unwrap()
            .len()
            > 0,
        "app.<hash>.js is empty"
    );

    // Two different built pages: the root home page and `posts/em-algorithm/index.html`
    // (two directories deep). Dedup and the depth-relative href both pin off the same
    // app.<hash>.css filename.
    let home = std::fs::read_to_string(out.join("index.html")).unwrap();
    let post = std::fs::read_to_string(out.join("posts/em-algorithm/index.html")).unwrap();
    assert!(
        home.contains(&format!("_assets/{app_css}")),
        "home does not link the shared app.css"
    );
    assert!(
        post.contains(&format!("../../_assets/{app_css}")),
        "a post two directories deep must carry a ../../ prefix to the same app.css"
    );

    // No page inlines the framework CSS (MARKER_BASE); the shared file carries it instead.
    assert!(
        !home.contains(MARKER_BASE) && !post.contains(MARKER_BASE),
        "framework CSS must not be inlined into a page"
    );
    let app_css_content = std::fs::read_to_string(out.join("_assets").join(&app_css)).unwrap();
    assert!(
        app_css_content.contains(MARKER_BASE),
        "the shared app.css must carry the framework CSS"
    );

    // Conditional katex: `em-algorithm` is math-dense (see
    // crates/core/tests/tech_blog.rs's `math_renders_inline_display_and_align`, which
    // counts >20 KaTeX spans), so it links katex.<hash>.css. The CV is prose-only (no
    // `$...$` anywhere), so it links nothing.
    assert!(
        post.contains("_assets/katex."),
        "a math-heavy post must link katex.<hash>.css"
    );
    let cv = std::fs::read_to_string(out.join("cv.html")).unwrap();
    assert!(
        !cv.contains("_assets/katex."),
        "a prose-only page must not link katex"
    );

    // The `{js}`-cell runtime must ship INLINE on `em-algorithm` (it has real
    // `await import("./em-helpers.js")` cells): the runtime's own `AsyncFunction` literal
    // appears verbatim in the page, proving it was NOT folded into the deferred, shared
    // `_assets/app.<hash>.js`. If it were, `new AsyncFunction(..., src)`'s dynamic `import()`
    // would resolve `./em-helpers.js` against `/_assets/` (a 404) instead of the page. This
    // pins the js-import fix so a future asset-bundle refactor cannot silently regress it.
    assert!(
        post.contains("new AsyncFunction("),
        "the {{js}}-cell runtime must be inlined verbatim (AsyncFunction token) on an import() page"
    );
    // The heavy d3/Plot libs ARE externalized (`_assets/jslibs.<hash>.js`), but the runtime
    // itself is never delivered via an `_assets/` link (no runtime asset file exists) —
    // folding it into a shared bundle is exactly the regression this guards against.
    assert!(
        post.contains("_assets/jslibs."),
        "a {{js}} page still links the shared jslibs bundle"
    );
    assert!(
        !post.contains("_assets/tali-js") && !post.contains("_assets/talijs"),
        "the {{js}}-cell runtime must not be externalized into an _assets/ file"
    );

    let _ = std::fs::remove_dir_all(&out);
}

// Needles that appear ONLY when the deck framework is inlined into the page, rather than
// linked out of `_assets/`. Each is a *definition* in exactly one bundled file, so the
// whole-page `contains` cannot be satisfied by some other blob riding along in the head
// (the inlined-asset needle trap): a CSS declaration body for deck.css, the facade
// assignment for deck.js, and the vendored library's esbuild namespace for mermaid.
const MARKER_DECK_CSS: &str = ".tali-blackout-overlay { position: fixed;";
const MARKER_DECK_JS: &str = "window.TaliesinDeck = facade;";
const MARKER_MERMAID_LIB: &str = "__esbuild_esm_mermaid_nm";

/// L2-1: a deck built *inside* a multi-page `build <dir>` shares that build's `_assets/`
/// instead of re-inlining the whole framework. Measured before the fix on a site whose only
/// deck drew one mermaid diagram: `talk.html` was **4,583,261 bytes** and linked `_assets/`
/// **zero** times, while the ordinary page beside it was 24,718 — so mermaid shipped twice
/// in one output tree, and a second deck would have shipped a third copy.
///
/// The *standalone* deck build is deliberately left alone (asserted at the bottom): that is
/// the artifact you hand someone, and it has no `_assets/` to link.
#[test]
fn a_site_deck_links_the_shared_assets_instead_of_inlining_them() {
    let root = std::env::temp_dir().join(format!("tali-ab-deck-src-{}", std::process::id()));
    let out = std::env::temp_dir().join(format!("tali-ab-deck-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("_site.yml"), "title: DeckWeight\n").unwrap();
    std::fs::write(
        root.join("index.tmd"),
        "---\ntitle: Home\n---\n\n{{< embed talk.tmd >}}\n",
    )
    .unwrap();
    // A mermaid diagram (the measured worst case) and math (so the conditional katex link
    // is exercised on a deck too, not only on a page).
    std::fs::write(
        root.join("talk.tmd"),
        "---\ntitle: Talk\nformat: deck\n---\n\n# One\n\n```mermaid\ngraph TD;\n  A-->B;\n```\n\n## Two\n\nMath $x=1$.\n",
    )
    .unwrap();
    let ok = Command::new(bin())
        .args(["build"])
        .arg(&root)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build");
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    let assets = std::fs::read_dir(out.join("_assets"))
        .expect("_assets dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let named = |stem: &str, ext: &str| -> String {
        assets
            .iter()
            .find(|n| n.starts_with(&format!("{stem}.")) && n.ends_with(ext))
            .unwrap_or_else(|| panic!("no {stem}.<hash>{ext} in {assets:?}"))
            .clone()
    };
    let deck_css = named("deck", ".css");
    let deck_js = named("deck", ".js");
    let mermaid_js = named("mermaid", ".js");
    let katex_css = named("katex", ".css");

    let talk = std::fs::read_to_string(out.join("talk.html")).unwrap();
    for href in [&deck_css, &deck_js, &mermaid_js, &katex_css] {
        assert!(
            talk.contains(&format!("_assets/{href}")),
            "the deck must link _assets/{href}"
        );
    }
    // Nothing that is now a shared file may also be inlined into the deck page.
    for (what, needle) in [
        ("deck.css", MARKER_DECK_CSS),
        ("deck.js", MARKER_DECK_JS),
        ("the mermaid library", MARKER_MERMAID_LIB),
    ] {
        assert!(
            !talk.contains(needle),
            "{what} must not be inlined into a deck inside a site build"
        );
    }
    // Mermaid ships ONCE in the tree, not once per deck.
    assert_eq!(
        assets.iter().filter(|n| n.starts_with("mermaid.")).count(),
        1,
        "{assets:?}"
    );
    // The whole point: the deck page is now page-sized, not framework-sized.
    let bytes = talk.len();
    assert!(
        bytes < 300_000,
        "a site deck is still shipping the framework: {bytes} bytes"
    );

    // The standalone artifact is untouched: `build <deck.tmd>` has no `_assets/` to link,
    // so it must still carry the engine (and its diagram library) inside the one file.
    let solo = std::env::temp_dir().join(format!("tali-ab-deck-solo-{}.html", std::process::id()));
    let _ = std::fs::remove_file(&solo);
    let ok = Command::new(bin())
        .args(["build"])
        .arg(root.join("talk.tmd"))
        .arg(&solo)
        .output()
        .expect("standalone deck build");
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let solo_html = std::fs::read_to_string(&solo).unwrap();
    for (what, needle) in [
        ("deck.css", MARKER_DECK_CSS),
        ("deck.js", MARKER_DECK_JS),
        ("the mermaid library", MARKER_MERMAID_LIB),
    ] {
        assert!(
            solo_html.contains(needle),
            "a standalone deck must stay self-contained: {what} is missing"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::remove_file(&solo);
}

/// Regression pin for the #17 blocker: in an External (`build <dir>`) page, a documented
/// A `_site.yml` `head:` extension that calls `window.taliEnhancers.register(...)` runs at
/// parse. #17 folded the registry into the DEFERRED `_assets/app.<hash>.js`, so that hook fired
/// before the registry was defined and threw `Cannot read properties of undefined`. The fix emits
/// the registry inline at parse (ahead of the deferred app.js), so the hook resolves. This asserts
/// the registry DEFINITION ships inline (a `<script>` WITHOUT `src=`) and BEFORE the
/// `include-after-body` script position.
#[test]
fn external_inlines_enhancer_registry_before_include_after_body() {
    let root = std::env::temp_dir().join(format!("tali-ab-reg-src-{}", std::process::id()));
    let out = std::env::temp_dir().join(format!("tali-ab-reg-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(root.join("sub")).unwrap();
    // The project carries an extension script in `_site.yml` `head:` (inline `text:` form, so
    // no separate file to ship) that registers an enhancer through the public hook taught in
    // docs/internals/extending.tmd. `TALI-PIN-HOOK` is a unique marker to locate its position.
    //
    // `head:` is the ONLY raw-injection route left (the per-document `include-*` family was
    // retired 2026-08-02), and it injects into `<head>` — EARLIER than the body slot this
    // test was originally written against, so the ordering it pins matters more, not less.
    std::fs::write(
        root.join("_site.yml"),
        "title: AB\nhead:\n  text: |\n    \
         <script defer>window.taliEnhancers.register(function(root){/* TALI-PIN-HOOK */});</script>\n",
    )
    .unwrap();
    std::fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
    std::fs::write(
        root.join("sub/page.tmd"),
        "---\ntitle: Sub\n---\n\nHello.\n",
    )
    .unwrap();
    let ok = Command::new(bin())
        .args(["build"])
        .arg(&root)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build");
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    let index = std::fs::read_to_string(out.join("index.html")).unwrap();

    // The extension hook was injected (proves the include-after-body wiring reached the page).
    let hook_pos = index
        .find("TALI-PIN-HOOK")
        .expect("the `head:` extension script must be present on the page");

    // The registry DEFINITION (`window.taliEnhancers = {`) is a literal unique to the registry
    // source (the hook only *calls* `.register`), and app.js is an external file whose body is
    // NOT in the page, so its presence in the HTML proves the registry shipped INLINE.
    let reg_pos = index
        .find("window.taliEnhancers = {")
        .expect("the enhancer registry must be emitted INLINE (not only in the deferred app.js)");
    // The idempotency guard verbatim from 01-registry.js: only ever inline (never in the page
    // via the external app.js link), a second confirmation the registry source is inlined.
    assert!(
        index.contains("if (window.taliEnhancers) return;"),
        "the inline registry must carry its idempotency guard verbatim"
    );

    // The registry must be DEFINED before the `head:` hook runs, so the hook's
    // `window.taliEnhancers.register(...)` resolves instead of throwing.
    assert!(
        reg_pos < hook_pos,
        "the registry must be defined (inline) BEFORE a `head:` extension script"
    );

    // The registry is inline, but app.js is STILL deferred + external (the fix keeps the bundle
    // deferred; only the tiny registry is duplicated inline). Its bundled registry copy no-ops.
    let assets = std::fs::read_dir(out.join("_assets"))
        .expect("_assets dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let app_js = assets
        .iter()
        .find(|n| n.starts_with("app.") && n.ends_with(".js"))
        .unwrap_or_else(|| panic!("no app.<hash>.js in {assets:?}"));
    assert!(
        index.contains(&format!("<script src=\"_assets/{app_js}\" defer></script>")),
        "app.js must stay deferred + external alongside the inline registry"
    );

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

/// Item 150: the body typeface was 160 KB of base64 sitting inside the render-blocking
/// stylesheet that **every** page of a site links (23 of 23 on a built `docs/guide`) — the
/// only weight a reader pays on all of them. A site build already emits separate hashed
/// assets, so there the two faces become their own `.woff2` files with a `preload`.
///
/// **Per-target, not global.** `build <file.tmd>` promises ONE self-contained file, so it
/// must keep inlining; the last assertion here is what stops a future change from "fixing"
/// this by breaking that promise. Still self-hosted, still offline, still no CDN either way.
#[test]
fn a_site_build_links_the_body_font_instead_of_inlining_160kb_of_base64() {
    let root = std::env::temp_dir().join(format!("tali-ab-font-src-{}", std::process::id()));
    let out = std::env::temp_dir().join(format!("tali-ab-font-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("_site.yml"), "title: Fonts\n").unwrap();
    std::fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
    std::fs::write(root.join("sub/deep.tmd"), "---\ntitle: Deep\n---\n\nHi.\n").unwrap();
    let ok = Command::new(bin())
        .args(["build"])
        .arg(&root)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build");
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    let assets = std::fs::read_dir(out.join("_assets"))
        .expect("_assets dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    // Both faces ship as real files, content-hashed like every other shared asset.
    let faces = assets
        .iter()
        .filter(|n| n.ends_with(".woff2"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        faces.len(),
        2,
        "the roman + italic faces must each be their own file: {assets:?}"
    );

    let app_css_name = assets
        .iter()
        .find(|n| n.starts_with("app.") && n.ends_with(".css"))
        .unwrap_or_else(|| panic!("no app.<hash>.css in {assets:?}"));
    let app_css = std::fs::read_to_string(out.join("_assets").join(app_css_name)).unwrap();

    // The point of the item: no base64 face inside the render-blocking sheet.
    assert!(
        !app_css.contains("data:font/woff2;base64,"),
        "the body face must not be base64 inside app.css ({} bytes)",
        app_css.len()
    );
    // ...and the @font-face rules still resolve, as SIBLING refs. A `url()` inside a
    // stylesheet resolves against the STYLESHEET's url, not the page's, so a bare filename
    // is correct from `_assets/` at every page depth — no `../` climb belongs here.
    for face in &faces {
        assert!(
            app_css.contains(&format!("url({face})")),
            "app.css must reference {face} as a sibling: no `_assets/` prefix, no `../`"
        );
    }
    // Sanity floor on the saving. The sheet was measured at 224 KB raw with the faces in it.
    assert!(
        app_css.len() < 120_000,
        "app.css is still font-sized: {} bytes",
        app_css.len()
    );

    // The roman face is preloaded from the page, so its fetch starts beside the stylesheet
    // instead of after it parses. Depth-adjusted, because THIS href is page-relative.
    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    let deep = std::fs::read_to_string(out.join("sub/deep.html")).unwrap();
    let roman = faces
        .iter()
        .find(|n| n.contains("normal"))
        .expect("a roman face");
    assert!(
        index.contains(&format!(
            "<link rel=\"preload\" as=\"font\" type=\"font/woff2\" href=\"_assets/{roman}\" crossorigin>"
        )),
        "a root page must preload the roman face"
    );
    assert!(
        deep.contains(&format!("href=\"../_assets/{roman}\" crossorigin>")),
        "a nested page's preload must climb: {deep:.0}"
    );
    // Preload only pays if it starts before the sheet it beats.
    let (p, s) = (
        deep.find("rel=\"preload\" as=\"font\"").expect("preload"),
        deep.find("rel=\"stylesheet\"").expect("stylesheet"),
    );
    assert!(p < s, "the font preload must precede the stylesheet link");

    // The single-file promise is untouched: `build <file.tmd>` is ONE file, so it inlines.
    let solo = std::env::temp_dir().join(format!("tali-ab-font-solo-{}.html", std::process::id()));
    let _ = std::fs::remove_file(&solo);
    let ok = Command::new(bin())
        .args(["build"])
        .arg(root.join("index.tmd"))
        .arg(&solo)
        .output()
        .expect("standalone build");
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let one = std::fs::read_to_string(&solo).unwrap();
    assert!(
        one.contains("url(data:font/woff2;base64,"),
        "a single-file build must still inline the face: it has no _assets/ to link"
    );
    assert!(
        !one.contains(".woff2\" crossorigin"),
        "a single-file build has nothing to preload"
    );

    let _ = std::fs::remove_file(&solo);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

/// Item 137: `mermaid.js` (3.57 MB) + `jslibs.js` (487 KB) + `katex.css` (369 KB) were
/// written into every site build, referenced by zero pages on a prose-only project — 85%
/// of `_assets/` bytes on a 113-page build, 92% on another. The author's own hand already
/// gates the deck pair this way four members away in `AssetBundle`, with the comment "a
/// site without a deck should not pay for a file nothing links".
///
/// Both halves matter and the second is the one that could regress silently: a build that
/// stopped writing an asset a page *does* link is a live 404, which is strictly worse than
/// the weight. So this asserts absence on the prose site AND presence on the feature site.
#[test]
fn the_conditional_bundles_are_written_only_when_a_page_links_them() {
    let names = |kind: &str, body: &str| -> Vec<String> {
        let root =
            std::env::temp_dir().join(format!("tali-ab-cond-src-{kind}-{}", std::process::id()));
        let out =
            std::env::temp_dir().join(format!("tali-ab-cond-out-{kind}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("_site.yml"), "title: Cond\n").unwrap();
        std::fs::write(
            root.join("index.tmd"),
            "---\ntitle: Home\n---\n\nPlain prose.\n",
        )
        .unwrap();
        std::fs::write(root.join("feature.tmd"), body).unwrap();
        let ok = Command::new(bin())
            .args(["build"])
            .arg(&root)
            .arg("--out")
            .arg(&out)
            .output()
            .expect("build");
        assert!(
            ok.status.success(),
            "{}",
            String::from_utf8_lossy(&ok.stderr)
        );
        let mut got = std::fs::read_dir(out.join("_assets"))
            .expect("_assets dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        got.sort();
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&out);
        got
    };
    let has = |assets: &[String], stem: &str| assets.iter().any(|n| n.starts_with(stem));

    // A second prose page, so the project shape matches the feature build exactly and the
    // only difference between the two runs is what the page CONTAINS.
    let prose = names("prose", "---\ntitle: More\n---\n\nStill just prose.\n");
    for stem in ["mermaid.", "jslibs.", "katex."] {
        assert!(
            !has(&prose, stem),
            "no page links {stem}<hash>, so it must not be written: {prose:?}"
        );
    }
    // Control: the two unconditional files are still there, or "wrote nothing" would pass
    // this test just as well as "wrote only what is linked".
    assert!(
        has(&prose, "app.") && prose.iter().filter(|n| n.starts_with("app.")).count() == 2,
        "app.css + app.js are unconditional: {prose:?}"
    );

    // The other direction, which is the one that would be a live 404 if it broke.
    let feature = names(
        "feature",
        "---\ntitle: More\n---\n\nMath $x=1$.\n\n```{mermaid}\ngraph TD;\n  A-->B;\n```\n\n```{js}\n1 + 1;\n```\n",
    );
    for stem in ["mermaid.", "jslibs.", "katex."] {
        assert!(
            has(&feature, stem),
            "a page links {stem}<hash>, so it must be written: {feature:?}"
        );
    }
}

/// A site build is the documented way to ship a working `{pyodide}` page, and it had NO test:
/// nothing in the tree referenced `_assets/pyodide`, `PYODIDE_DIR_NAME` or the payload copy
/// (item 158). The whole delivery path — copy the directory, stamp a page-relative index —
/// could have broken with every gate green, and it fails only in the reader's browser, as a
/// module-load error with no server-side symptom.
///
/// The nested page is the point of the second assertion: `_assets/` is reached through a
/// per-page relative prefix, so a chapter one directory down must resolve `../_assets/...`.
/// A root-only test would pass with the depth arithmetic hardcoded to `""`.
///
/// Gated per-test, not per-file: this file's other asset assertions are feature-independent.
/// There is no payload to copy when the runtime is compiled out. `tools/gates.sh` arms the
/// feature and asserts this printed `... ok` by name.
#[cfg(feature = "pyodide")]
#[test]
fn site_build_copies_the_pyodide_runtime_and_stamps_a_page_relative_index() {
    let root = std::env::temp_dir().join(format!("tali-ab-pyo-src-{}", std::process::id()));
    let out = std::env::temp_dir().join(format!("tali-ab-pyo-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("_site.yml"), "title: PY\n").unwrap();
    std::fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
    std::fs::write(
        root.join("sub/page.tmd"),
        "---\ntitle: Sub\n---\n\n```{pyodide}\nimport numpy as np\nnp.arange(3).tolist()\n```\n",
    )
    .unwrap();
    let ok = Command::new(bin())
        .args(["build"])
        .arg(&root)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build");
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    let dir = out.join("_assets").join(taliesin_core::PYODIDE_DIR_NAME);
    // Compared against the accessor, never a hardcoded file list: a literal on both sides of
    // this comparison would agree with itself and with nothing else.
    for (name, bytes) in taliesin_core::pyodide_payload() {
        let copied = std::fs::read(dir.join(name))
            .unwrap_or_else(|e| panic!("site build did not copy _assets/*/{name}: {e}"));
        assert_eq!(
            copied.len(),
            bytes.len(),
            "the copied `{name}` is not the vendored one"
        );
    }

    let page = std::fs::read_to_string(out.join("sub/page.html")).expect("built page");
    assert!(
        page.contains(&format!(
            "<meta name=\"tali-pyodide-index\" content=\"../_assets/{}/\">",
            taliesin_core::PYODIDE_DIR_NAME
        )),
        "a page one directory down must reach the runtime through `../_assets/`"
    );
    // The known-positive: a site build must NOT degrade — this is the mode where it runs.
    assert!(
        page.contains("<script type=\"application/tali-pyodide\""),
        "the live wrapper must survive a site build, or the page ships a dead listing"
    );

    // The control: the prose page shares the same `_assets/`, and must not claim a runtime.
    let home = std::fs::read_to_string(out.join("index.html")).expect("built home");
    assert!(
        !home.contains("<meta name=\"tali-pyodide-index\""),
        "a page with no `{{pyodide}}` cell must not stamp the index meta"
    );

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}
