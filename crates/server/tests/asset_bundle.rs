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
        !post.contains("_assets/qmd-js") && !post.contains("_assets/qmdjs"),
        "the {{js}}-cell runtime must not be externalized into an _assets/ file"
    );

    let _ = std::fs::remove_dir_all(&out);
}

/// Regression pin for the #17 blocker: in an External (`build <dir>`) page, a documented
/// `include-after-body` extension that calls `window.taliEnhancers.register(...)` runs INLINE at
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
    std::fs::write(root.join("_site.yml"), "title: AB\n").unwrap();
    // The home page carries an `include-after-body` extension script (inline `text:` form, so no
    // separate file to ship) that registers an enhancer through the public hook taught in
    // docs/internals/extending.tmd. `TALI-PIN-HOOK` is a unique marker to locate its position.
    std::fs::write(
        root.join("index.tmd"),
        "---\ntitle: Home\ninclude-after-body:\n  text: |\n    \
         <script>window.taliEnhancers.register(function(root){/* TALI-PIN-HOOK */});</script>\n---\n\nHi.\n",
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

    // The extension hook was injected (proves the include-after-body wiring reached the page).
    let hook_pos = index
        .find("TALI-PIN-HOOK")
        .expect("include-after-body extension script must be present on the page");

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

    // The registry must be DEFINED before the `include-after-body` hook runs, so the hook's
    // `window.taliEnhancers.register(...)` resolves at parse instead of throwing.
    assert!(
        reg_pos < hook_pos,
        "the registry must be defined (inline) BEFORE the include-after-body extension script"
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
