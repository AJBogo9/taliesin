//! Shared helpers for the integration test crates. Each `tests/*.rs` file is its
//! own crate, so common code lives here and is pulled in with `mod common;`. Not
//! every crate uses every item, hence the module-wide `dead_code` allow.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// The repository's `corpus/` directory (the spec documents), resolved relative
/// to the `taliesin-core` crate the test belongs to.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

/// Where [`RenderPage::render_page`] links the shared bundle: the site build's `_assets/`
/// (a test reads the markup, never these hrefs' depth).
pub const TEST_ASSETS: taliesin_core::ExternalAssets<'static> = taliesin_core::ExternalAssets {
    app_css: "_assets/app.css",
    katex_css: "_assets/katex.css",
    app_js: "_assets/app.js",
    mermaid_js: "_assets/mermaid.js",
    jslibs_js: "_assets/jslibs.js",
    font_preload: "",
};

/// A project's pages as the site build writes them.
pub trait RenderPage {
    /// One page (by rel-path or URL) rendered, finished and wrapped in its chrome linking
    /// `_assets/` (`Site::page_html_external`). `None` when it is no page of the site.
    fn render_page(&self, rel_or_url: &str) -> Option<String>;

    /// The build's last two steps for a rendered `doc`: `Site::finish_blocks`, then the page
    /// in its chrome. Returns the page with the render's and the finish's warnings.
    fn finish_page(
        &self,
        page: &taliesin_core::Page,
        doc: taliesin_core::RenderedDoc,
    ) -> (String, Vec<taliesin_core::render::Warning>);
}

impl RenderPage for taliesin_core::Site {
    fn render_page(&self, rel_or_url: &str) -> Option<String> {
        let page = self.page(rel_or_url)?;
        let src = taliesin_core::includes::read_source(&page.input).ok()?;
        let base = page.input.parent().unwrap_or(&self.root);
        let doc = taliesin_core::render_document_scoped_with_site(
            &src,
            base,
            self.chapter_for(page),
            Some(&self.render_defaults()),
        );
        Some(self.finish_page(page, doc).0)
    }

    fn finish_page(
        &self,
        page: &taliesin_core::Page,
        mut doc: taliesin_core::RenderedDoc,
    ) -> (String, Vec<taliesin_core::render::Warning>) {
        let mut warnings = std::mem::take(&mut doc.warnings);
        doc.toc = self.finish_blocks(page, &mut doc.blocks, &mut warnings, None, doc.toc_explicit);
        (self.page_html_external(page, &doc, TEST_ASSETS), warnings)
    }
}

/// A rendered page with the *contents* of every `<script>` and `<style>` blanked, for
/// assertions about the page's own markup.
///
/// A built page inlines the bundled CSS and JS, so a `contains("…")` against the whole
/// page can be satisfied by a comment or a selector inside those assets rather than by
/// anything the renderer emitted. That is not hypothetical: `06-skip-link.js` opens by
/// quoting `<main id="tali-main" tabindex="-1">` verbatim to say what the server already
/// does, so the a11y chrome test passed with the real `<main>` emission commented out, and
/// every `tali-site-nav` / `tali-site-footer` needle is also a selector in `site.css`.
///
/// The tags survive (only their bodies are emptied) so a needle *about* a script or style
/// element (`<script type="application/tali-js">`, `<script src=…>`) still works.
pub fn markup_only(page: &str) -> String {
    let mut out = String::with_capacity(page.len());
    let mut rest = page;
    while let Some((at, tag)) = ["<script", "<style"]
        .iter()
        .filter_map(|t| rest.find(t).map(|i| (i, *t)))
        .min()
    {
        // The open tag itself is markup and is kept whole; only what it wraps goes.
        let Some(open_end) = rest[at..].find('>').map(|i| at + i + 1) else {
            // An unterminated open tag: keep what precedes it, drop the rest unread.
            out.push_str(&rest[..at]);
            rest = "";
            break;
        };
        out.push_str(&rest[..open_end]);
        let close = format!("</{}", &tag[1..]);
        match rest[open_end..].find(&close) {
            Some(i) => rest = &rest[open_end + i..],
            // An unclosed script/style: drop what it swallowed rather than assert on it.
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A throwaway project directory built up in-test, so each case can express
/// exactly the manifest/config/files it needs without committed fixtures. The
/// directory is removed on drop.
pub struct TempProj(pub PathBuf);

impl TempProj {
    pub fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "tali-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        TempProj(p)
    }

    /// Write a file (creating parent dirs) relative to the project root.
    pub fn file(&self, rel: &str, content: &str) -> &Self {
        let f = self.0.join(rel);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, content).unwrap();
        self
    }

    /// Install an extension `name` whose `_extension.yml` is `manifest`.
    pub fn ext(&self, name: &str, manifest: &str) -> &Self {
        self.file(&format!("_extensions/{name}/_extension.yml"), manifest)
    }
}

impl Default for TempProj {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TempProj {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
