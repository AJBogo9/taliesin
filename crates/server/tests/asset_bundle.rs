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

    let _ = std::fs::remove_dir_all(&out);
}
