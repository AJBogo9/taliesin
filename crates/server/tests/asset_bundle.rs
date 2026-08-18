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
const MARKER_BASE: &str = ".tali-title-block";

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

/// Regression pin for the #17 blocker: in an External (`build <dir>`) page, an author script
/// that calls `window.taliEnhancers.register(...)` runs at parse. #17 folded the registry into
/// the DEFERRED `_assets/app.<hash>.js`, so that hook fired before the registry was defined and
/// threw `Cannot read properties of undefined`. The fix emits the registry inline at parse
/// (ahead of the deferred app.js), so the hook resolves. This asserts the registry DEFINITION
/// ships inline (a `<script>` WITHOUT `src=`) and BEFORE the author's own script.
///
/// The vehicle used to be `_site.yml`'s `head:`, cut 2026-08-18. Raw HTML in the document body
/// is the surviving route (`docs/internals/extending.tmd` teaches it), and the pin is still
/// live rather than tautological: the script below is a PLAIN inline `<script>`, so it executes
/// during parse, and re-folding the registry into the deferred bundle would throw exactly as it
/// did in #17 — the registry is emitted in `<head>`, the hook runs in the body, and the
/// ordering assert catches any change that moves the definition after the content.
#[test]
fn external_inlines_enhancer_registry_before_an_author_script() {
    let root = std::env::temp_dir().join(format!("tali-ab-reg-src-{}", std::process::id()));
    let out = std::env::temp_dir().join(format!("tali-ab-reg-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(root.join("sub")).unwrap();
    // The page carries an extension script as raw HTML in its own body — the surviving
    // raw-injection route now that the per-document `include-*` family (2026-08-02) and
    // `_site.yml`'s `head:` (2026-08-18) are both gone. It registers an enhancer through the
    // public hook taught in docs/internals/extending.tmd; `TALI-PIN-HOOK` locates its position.
    //
    // Deliberately NOT `defer`: a plain inline script executes during parse, which is the
    // moment #17 threw. A deferred one would run after the deferred bundle and could not fail.
    std::fs::write(root.join("_site.yml"), "title: AB\n").unwrap();
    std::fs::write(
        root.join("index.tmd"),
        "---\ntitle: Home\n---\n\nHi.\n\n\
         <script>window.taliEnhancers.register(function(root){/* TALI-PIN-HOOK */});</script>\n",
    )
    .unwrap();
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

    // The extension hook reached the page (raw HTML in the body passes through).
    let hook_pos = index
        .find("TALI-PIN-HOOK")
        .expect("the author's extension script must be present on the page");

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

    // The registry must be DEFINED before the author's hook runs, so the hook's
    // `window.taliEnhancers.register(...)` resolves instead of throwing.
    assert!(
        reg_pos < hook_pos,
        "the registry must be defined (inline) BEFORE an author's extension script"
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
/// assets, so there the three faces become their own `.woff2` files with a `preload`.
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

    // All three faces ship as real files, content-hashed like every other shared asset.
    let faces = assets
        .iter()
        .filter(|n| n.ends_with(".woff2"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        faces.len(),
        3,
        "the body roman, body italic and mono faces must each be their own file: {assets:?}"
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
    // Match the body face's own stem, not the bare substring "normal": the mono face
    // (`jetbrains-mono-latin-wght-normal.<hash>.woff2`) contains "normal" too, and
    // `faces` is built from `read_dir`, whose order is not guaranteed, so a substring
    // match here could silently select the mono face on some runs.
    let roman = faces
        .iter()
        .find(|n| n.starts_with("literata-latin-wght-normal"))
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
/// gated the (now-retired) deck pair this way in `AssetBundle`, with the comment "a site
/// without a deck should not pay for a file nothing links".
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
