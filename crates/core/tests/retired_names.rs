//! The retired `q`-prefix brand is gone. This asserts it stays gone.
//!
//! Every other assertion in the suite is a string literal living in the same file as
//! its emitter, so a rename moves both sides together and nothing fails (measured
//! 2026-07-25: a blanket substitution over the tree built clean and changed the state
//! of 5 of 1387 tests, 3 of them only block-id hash drift). That blindness is what
//! let the half-finished rename sit in the tree for a month. This file is the backstop.
//!
//! `docs/superpowers/` and `notes/` are deliberately exempt: they are the dated
//! plan/spec archive and the pre-rename record, and rewriting them would make a
//! 2026-06 document claim it used names that did not exist yet.

use std::path::{Path, PathBuf};

mod common;
use common::corpus_dir;

/// The retired token, assembled at runtime so this file can hunt for it without
/// containing it as a literal (which would make the guard flag itself).
fn retired() -> String {
    format!("{}{}", "q", "md")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Directory NAMES never scanned, at any depth. These must match by basename, not by
/// path prefix: build output nests (`site/_site/`, `corpus/tech-blog/_site/`,
/// `docs/guide/_book/`), and a root-anchored check silently scans every nested copy —
/// which is stale HTML emitted by whatever binary last ran, so it reports the retired
/// name forever and can never be fixed by editing source.
const SKIP_DIR_NAMES: &[&str] = &[
    ".git",
    "target",
    "_site",
    "_book",
    "_freeze",
    "node_modules",
    ".vscode-test",
    // The ui-audit harness's gitignored scratch build (`tools/ui-audit/.work/`).
    ".work",
    // Gitignored Python kernel virtualenvs (`.venv-audit/` for local audits;
    // `.venv/` is what the interpreter resolver finds on its own). Skipped because
    // pip's base64 package metadata in `*.dist-info/RECORD` files produces false
    // positives matching the retired brand.
    ".venv-audit",
    ".venv",
];

/// Directories never scanned, matched as a path prefix from the repo root: the dated
/// plan/spec archive, the pre-rename record, and the gitignored local `.superpowers/`
/// task-report archive (all three describe the rename as it happened).
///
/// `.claude/worktrees` is the same category — gitignored local scratch — but it earns its
/// own line, because it is the one entry that exists to defend the *root-anchored* matching
/// above. A parallel session's worktree is a full second checkout, so it carries its own
/// `notes/`; `notes` is listed here and still would not cover
/// `.claude/worktrees/<branch>/notes/`. Without this, any session that opens a worktree turns
/// the guard red in **every other** session's tree, over files that are neither tracked nor
/// theirs to edit.
const SKIP_PATHS: &[&str] = &[
    "docs/superpowers",
    "notes",
    ".superpowers",
    ".claude/worktrees",
];

/// Is this one occurrence of the retired token a legitimately-retired NAME rather than
/// a reintroduction?
///
/// Exemptions are decided **per occurrence, with boundaries** — not by stripping
/// substrings, and not by allowlisting whole files. Two reasons:
///
/// - A whole-file allowlist would have blinded the guard to future additions in
///   `serve/mod.rs` and `site/links.rs`, two of the largest files in the tree.
/// - A naive `line.replace(".<tok>", "")` looks equivalent and is not: it also eats the
///   `.` in `window.<tok>Js`, so a reintroduced global goes unreported. That exact
///   false negative was caught by `the_guard_detects_a_reintroduction` while writing
///   this, which is why the boundary check below is spelled out rather than inlined.
fn occurrence_is_exempt(lower: &str, at: usize, q: &str) -> bool {
    let bytes = lower.as_bytes();
    let before = at.checked_sub(1).map(|i| bytes[i]);
    let after = bytes.get(at + q.len()).copied();
    let is_word =
        |c: Option<u8>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-');

    // The retired SOURCE EXTENSION (`.<tok>`). The renderer stopped accepting it in the
    // .tmd-only break and several tests assert exactly that ("a stray one is invisible
    // to the walker"), so they must be able to name it. Requires a real extension
    // boundary after it, so `window.<tok>Js` is NOT exempt.
    if before == Some(b'.') && !is_word(after) {
        return true;
    }
    // The same extension as a bare quoted `is_source_ext` argument (`crates/core/src/ext.rs`).
    if before == Some(b'"') && after == Some(b'"') {
        return true;
    }
    // The retired PRODUCT BRAND (`<tok>-fast` / `<tok>fast`).
    // `editor/vscode/src/test/manifest.test.ts` guards against it returning to the
    // manifest (it shipped once as the default binary path), and `corpus/README.md`
    // names `<brand>-testbed`, a real sibling repo on disk.
    let rest = &lower[at + q.len()..];
    if rest.starts_with("-fast") || rest.starts_with("fast") {
        return true;
    }
    false
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let rel = p
            .strip_prefix(root)
            .unwrap_or(&p)
            .to_string_lossy()
            .replace('\\', "/");
        if SKIP_PATHS
            .iter()
            .any(|s| rel == *s || rel.starts_with(&format!("{s}/")))
        {
            continue;
        }
        if p.is_dir() {
            let base = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if SKIP_DIR_NAMES.contains(&base.as_str()) {
                continue;
            }
            walk(&p, root, out);
        } else {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            // Vendored minified bundles are not ours to edit: `mermaid.min.js` carries
            // one incidental hit inside a base64 blob.
            if name.ends_with(".min.js") || name == "package-lock.json" {
                continue;
            }
            out.push(p);
        }
    }
}

/// Does this line carry at least one occurrence that is NOT a legitimately-retired name?
/// Case-insensitive: the brand appeared as `Qmd*` and `<tok>Fast` as well as lowercase.
fn line_offends(line: &str, q: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(off) = lower[from..].find(q) {
        let at = from + off;
        if !occurrence_is_exempt(&lower, at, q) {
            return true;
        }
        from = at + q.len();
    }
    false
}

#[test]
fn the_retired_brand_stays_retired() {
    let q = retired();
    let root = repo_root().canonicalize().expect("repo root");
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    files.sort();
    assert!(
        files.len() > 100,
        "the walker found only {} files; check SKIP_DIR_NAMES/SKIP_PATHS",
        files.len()
    );

    let this_file = Path::new(file!())
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut offenders = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(&root)
            .unwrap_or(f)
            .to_string_lossy()
            .replace('\\', "/");
        // This file documents what it bans, so it necessarily spells it out.
        if rel.ends_with(&this_file) {
            continue;
        }
        // Binaries (fonts, images) fail the utf8 read and are skipped, which also keeps
        // base64-looking payloads from producing incidental hits.
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            if line_offends(line, &q) {
                offenders.push(format!("{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the retired `{q}` brand is gone; use `tali`. {} occurrence(s):\n{}\n\n\
         If one of these legitimately names something retired (the old source \
         extension, the old product brand), add the exact spelling to \
         `exempt_spellings()` with a reason. Do not widen it to a bare `{q}`.",
        offenders.len(),
        offenders.join("\n")
    );
}

/// The guard must be able to *fail*. A scanner that silently matches nothing (a broken
/// walker, an over-broad exemption) would pass forever and pin the very half-done state
/// this file exists to prevent.
#[test]
fn the_guard_detects_a_reintroduction() {
    let q = retired();
    // Realistic reintroductions, one per family this purge touched.
    for bad in [
        format!("<div data-{q}-out>"),
        format!("window.{q}Js = window.taliJs;"),
        format!("localStorage.getItem('{q}-theme')"),
        format!("application/{q}-js"),
        format!("_{q}_render(fig)"),
        format!("id=\"{q}-title-block\""),
    ] {
        assert!(
            line_offends(&bad, &q),
            "the guard failed to flag a reintroduction: {bad}"
        );
    }

    // And the exemptions must still let the legitimately-retired names through.
    for ok in [
        format!("assert!(!is_source_path(Path::new(\"a/b/index.{q}\")));"),
        format!("if (/{q}-fast|{q}Fast/i.test(line))"),
        format!("lives in the separate `{q}-fast-testbed` repo, not here."),
    ] {
        assert!(
            !line_offends(&ok, &q),
            "an exempt spelling was wrongly flagged: {ok}"
        );
    }

    // The walker must actually reach source files.
    let root = repo_root().canonicalize().expect("repo root");
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    assert!(
        files.iter().any(|f| f.ends_with("crates/core/src/lib.rs")),
        "the walker never reached crates/core/src/lib.rs"
    );
    assert!(
        !files
            .iter()
            .any(|f| f.to_string_lossy().contains("/docs/superpowers/")),
        "the frozen plan archive must stay out of the scan"
    );
}

/// The rest of this file hunts the `qmd` token. That is not the whole brand: the
/// `{{< input >}}` controls emitted DOM ids prefixed `q` + `in` — `qin-k`, `qin-rate` —
/// for a year after the rename, on nine corpus pages including the deck and the
/// `descent` gallery exhibit. The token guard could never see it (it is not the literal
/// `qmd`), and every other assertion lives in the same file as its emitter, so nothing
/// did. Renamed to `tali-in-` on 2026-08-05; this keeps the whole `q`-prefix family out
/// of emitted markup rather than just the one spelling that was found.
#[test]
fn no_q_prefixed_identifier_ships_in_emitted_markup() {
    // A real corpus page, through the entry point that expands shortcodes: `{{< input >}}`
    // is expanded by the include-aware path, so a bare `render_document` leaves the
    // directive as literal text and every needle below passes vacuously.
    let dir = corpus_dir().join("reactive");
    let src = std::fs::read_to_string(dir.join("inputs.tmd")).unwrap();
    let h = taliesin_core::render_document_with_includes(&src, &dir).body_html();
    assert!(
        h.contains("tali-in-k"),
        "fixture is wrong: the control id should be name-derived, got {h}"
    );
    for attr in ["id=\"q", "for=\"q", "class=\"q", "data-q"] {
        assert!(
            !h.contains(attr),
            "`{attr}…` is a retired `q`-prefix brand leftover in emitted markup: {h}"
        );
    }
}

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
    // The TOC read checkmarks lived in toc-spy.js, not the code-enhance bundle: the
    // scrollspy that file also carries (`tali-toc-active`) SURVIVES this pass (item T2,
    // structural), so this checks the specific script rather than folding it into the
    // `js` bundle check above, which never contained toc-spy.js in the first place and
    // would pass vacuously either way.
    assert!(
        !taliesin_core::render::TOC_SPY_JS.contains("tali-toc-read"),
        "the TOC read-tracking class still ships in toc-spy.js"
    );
    let css = taliesin_core::render::base_css();
    for needle in ["tali-resume", "tali-toc-read"] {
        assert!(!css.contains(needle), "`{needle}` CSS survives in base.css");
    }
    // The Continue-reading pill's server-emitted slot lived in `site.css` (book chrome),
    // not `base.css` (reader chrome) — a different stylesheet the checks above never
    // reach, which is exactly how its `.tali-book-continue*` rules survived one fix round
    // as dead CSS after the emitter that used them was deleted.
    assert!(
        !taliesin_core::render::site_css().contains("tali-book-continue"),
        "the Continue-reading pill's CSS survives in site.css"
    );
}

/// The floating mobile "Contents" pill was deleted 2026-08-03: it duplicated
/// the topbar, which is already sticky and already carries Chapters. It went as a
/// whole feature, not just the button: the pull-up sheet it opened (backdrop, drag
/// gestures, `tali-toc-sheet`/`tali-toc-open` body-class wiring) had nothing else to
/// drive it, so `#TOC` reverts to its in-flow mobile layout unconditionally.
#[test]
fn the_mobile_contents_pill_is_gone() {
    let page = taliesin_core::render::render_html_page(
        "---\ntitle: T\ntoc: true\n---\n\n# A\n\ntext\n\n## B\n\nmore\n",
        "f",
    );
    for needle in [
        "tali-toc-handle",
        "tali-toc-backdrop",
        "tali-toc-sheet",
        "tali-toc-open",
        "tali-toc-cur",
    ] {
        assert!(
            !page.contains(needle),
            "the mobile Contents pill's `{needle}` still ships in the page shell"
        );
    }
    // `code_scripts()` never bundled toc-spy.js (it is inlined separately, only on TOC
    // pages), so a needle against it would pass vacuously whether or not the pill's
    // leftover chip-write/scroll-hook code was cut from the file. Pin the script itself.
    for needle in ["taliTocScrollHook", "tali-toc-cur"] {
        assert!(
            !taliesin_core::render::TOC_SPY_JS.contains(needle),
            "`{needle}` still ships in toc-spy.js; the mobile pill's leftover hook survives \
             in the scrollspy it was deleted out of"
        );
    }
    let css = taliesin_core::render::base_css();
    for needle in [
        "tali-toc-handle",
        "tali-toc-backdrop",
        "tali-toc-sheet",
        "tali-toc-open",
        "tali-toc-cur",
        "tali-show-label",
    ] {
        assert!(!css.contains(needle), "`{needle}` CSS survives in base.css");
    }
}

/// Inverse search moved to Ctrl/Cmd-click on 2026-07-28; the modifier it used before is
/// retired. This asserts the old spelling stays gone.
///
/// Same rationale as the brand guard above: every other assertion for this gesture is a
/// string literal sitting beside its emitter, so a half-finished rename fails nothing. The
/// stale spelling is worse than cosmetic here, because it teaches a gesture that no longer
/// works.
///
/// `altKey` is NOT hunted. It is a legitimate DOM property, and several "no modifier is
/// held" guards (`deck.js`, `code-enhance/07-keyboard.js`) must keep testing it. Only the
/// *names* of the retired gesture are hunted, which is also why this comment cannot spell
/// it.
#[test]
fn the_alt_click_gesture_stays_retired() {
    // Assembled at runtime for the same reason `retired()` is: a guard holding its own
    // needle as a literal reports itself and can never be satisfied.
    let alt = format!("{}{}", "a", "lt");
    let needles = [
        format!("tali-{alt}"),
        format!("{alt}-click"),
        format!("{alt}+click"),
        format!("{}{}", "option", "-click"),
    ];
    let root = repo_root();
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    let mut offenders = Vec::new();
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            if needles.iter().any(|needle| lower.contains(needle.as_str())) {
                offenders.push(format!(
                    "{}:{}: {}",
                    path.strip_prefix(&root).unwrap_or(path).display(),
                    n + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the retired gesture is still named in {} place(s):\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

/// Hover cross-reference cards were deleted 2026-08-03: they fired on passive
/// mouse movement, uninvited, over every citation and cross-reference.
#[test]
fn hover_preview_cards_are_gone() {
    let js = taliesin_core::render::code_scripts();
    for needle in [
        "taliInitLinkPreview",
        "__taliLinkPreview",
        "TALIESIN_HOVER_INDEX",
    ] {
        assert!(
            !js.contains(needle),
            "`{needle}` still ships; hover cards were deleted"
        );
    }
}

/// Heading/figure `#` anchor links were deleted 2026-08-03. The TOC already
/// emits deep links, and the fragment's own justification cited a "selection
/// toolbar's text-fragment Share" that does not exist anywhere in the tree.
#[test]
fn heading_anchor_links_are_gone() {
    let js = taliesin_core::render::code_scripts();
    for needle in ["taliInitAnchorLinks", "tali-anchor", "__taliAnchorLive"] {
        assert!(
            !js.contains(needle),
            "`{needle}` still ships; anchor links were deleted"
        );
    }
}

/// Video hover-play (pointerenter/focusin play, pointerleave/focusout pause, plus the
/// document-level single-active-player coordinator) was deleted 2026-08-03: playing on
/// passive pointerenter is motion the reader did not request (WCAG 2.2.2 territory).
/// The figure lightbox (the touch play path) was already deleted the same day, so
/// native `controls` is now the ONLY way a reader can start a `{{< video >}}` clip —
/// it must ship on by default.
#[test]
fn video_hover_play_is_gone_and_controls_default_on() {
    let js = taliesin_core::render::code_scripts();
    // Needles unique to 18-media.js (checked against every other bundled script):
    // function/attribute names, not the generic pointer-event names that
    // plot.umd.min.js also carries.
    for needle in ["reduceMotion", "visibleVideo", "data-media-wired"] {
        assert!(
            !js.contains(needle),
            "`{needle}` still ships; the video hover-play enhancer (18-media.js) survives"
        );
    }

    let html = taliesin_core::render::render_document_with_includes(
        "{{< video clip.mp4 >}}\n",
        std::path::Path::new("."),
    )
    .body_html();
    assert!(
        html.contains(
            "<video src=\"clip.mp4\" muted loop controls playsinline preload=\"metadata\" \
             tabindex=\"0\" aria-label=\"Screencast\"></video>"
        ),
        "a bare {{{{< video >}}}} must emit the native `controls` attribute on the tag \
         itself — with hover-play and the lightbox both deleted it is the only \
         remaining play path: {html}"
    );
}

/// The reader show/hide-code toggle was deleted 2026-08-03. The author already
/// decides per cell with `#| echo:`; a reader override of that presentation
/// decision cost a permanent row in the Settings menu.
#[test]
fn the_code_visibility_toggle_is_gone() {
    let js = taliesin_core::render::code_scripts();
    for needle in [
        "taliInitCodeVisibility",
        "taliSetCodeHidden",
        "taliGetCodeHidden",
    ] {
        assert!(
            !js.contains(needle),
            "`{needle}` still ships; the toggle was deleted"
        );
    }
}

/// The toggle's needles above only prove the UI fragment is gone: `taliSetCodeHidden`/
/// `taliGetCodeHidden` were STRINGS in that fragment (it merely referenced the pre-paint
/// API by name), not their definition site. The definitions lived in `theme.rs`'s
/// pre-paint bootstrap, which ships in every rendered page's `<head>`, not in
/// `code_scripts()` — so this checks the actual page output and guards the half the
/// fragment-only test above cannot see. The theme half of the same bootstrap
/// (`taliSetTheme`/`taliGetThemeChoice`) must still be present: it is a separate
/// feature and survives this pass untouched.
#[test]
fn the_code_visibility_pre_paint_api_is_gone_and_theme_survives() {
    let doc = taliesin_core::render::render_document("hello\n");
    let html = taliesin_core::render::render_doc_to_page(
        &doc,
        "t",
        taliesin_core::render::OutputMode::Build,
    );
    for needle in [
        "taliSetCodeHidden",
        "taliGetCodeHidden",
        "tali-code-hidden",
        "tali:codevisibility",
    ] {
        assert!(
            !html.contains(needle),
            "`{needle}` still ships in the page's pre-paint bootstrap; the toggle was deleted"
        );
    }
    for needle in ["taliSetTheme", "taliGetThemeChoice", "tali-theme"] {
        assert!(
            html.contains(needle),
            "`{needle}` must survive — the theme picker is untouched by this pass"
        );
    }
}

/// The "Referenced by" backlink line was deleted 2026-08-04: it injected a
/// reverse-reference into a target block that a linear reader never asked for.
/// `sentences.rs` went with it — `backlinks.rs` was its only consumer.
#[test]
fn referenced_by_backlinks_are_gone() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/site");
    for gone in ["backlinks.rs", "sentences.rs"] {
        assert!(
            !dir.join(gone).exists(),
            "site/{gone} should have been deleted"
        );
    }
    let css = taliesin_core::render::site_css();
    assert!(
        !css.contains("tali-backref"),
        "the backref line's CSS (`.tali-backrefs`/`.tali-backref`/`.tali-backref-cite`) \
         survives in site.css"
    );
}

/// The listing category-filter chips were deleted 2026-08-04 (visual minimalism pass):
/// they paid off only on a blog with many posts AND disciplined category vocabulary. Only
/// the `listing.categories` sub-key is retired (see `frontmatter.rs`'s own pin for the
/// retirement message) — page-level `categories:` front matter and the per-card badges
/// (`tali-cat` / `data-cat`, checked by `token_contract.rs`) survive untouched.
#[test]
fn the_category_filter_chips_are_gone_from_the_client_bundle() {
    let js = taliesin_core::render::code_scripts();
    for needle in ["taliInitCategoryFilter", "tali-cat-filter", "tali-cat-chip"] {
        assert!(
            !js.contains(needle),
            "`{needle}` still ships; the category-filter chips were deleted"
        );
    }
    let css = taliesin_core::render::site_css();
    for needle in [
        "tali-cat-filter",
        "tali-cat-chip",
        "tali-cat-count",
        "tali-cat-on",
    ] {
        assert!(!css.contains(needle), "`{needle}` CSS survives in site.css");
    }
}

/// The backlink line is the REVERSE of cross-references (a target -> its referrers);
/// `xref.rs` is the FORWARD direction (a `@thm-kl` -> its target) and must survive this
/// deletion untouched. Rendered on the real `demo-book` fixture, where `results.tmd`
/// cross-references `methods.tmd`'s `@thm-kl`.
#[test]
fn forward_xrefs_survive_the_backlink_deletion() {
    let site = taliesin_core::Site::discover(&corpus_dir().join("demo-book"));
    let methods = site.render_page("methods.tmd").expect("methods renders");
    let results = site.render_page("results.tmd").expect("results renders");
    for (name, page) in [("methods.html", &methods), ("results.html", &results)] {
        assert!(
            !page.contains("Referenced by"),
            "{name} still carries a backlink line: {page}"
        );
        assert!(
            !page.contains("tali-backref"),
            "{name} still carries a backref class"
        );
    }
    // The forward cross-reference from results.tmd to methods.tmd's theorem still
    // resolves, with its number — this is `xref.rs`, which this deletion does not touch.
    assert!(
        results
            .contains("<a href=\"methods.html#thm-kl\" class=\"tali-xref\">Theorem&nbsp;2.1</a>"),
        "the forward cross-reference to thm-kl must still resolve: {results}"
    );
}

/// The per-chapter outline disclosures in the book drawer were deleted 2026-08-04
/// (visual minimalism pass, task 10): a second navigation layer inside a drawer that is
/// already a navigation layer, justified against a 60-chapter book when the largest real
/// book in the tree (docs/guide) has 25 chapters. The drawer's OWN flat chapter list
/// (`.tali-book-chapter`, `.tali-book-chapters`, `.tali-chap-num`, `.tali-chap-words`) and
/// the shared `search-index.js` both survive untouched — Cmd-K search reads the same
/// index and is a keeper of this whole pass.
#[test]
fn the_book_drawer_outline_is_gone() {
    let js = taliesin_core::render::code_scripts();
    for needle in [
        "taliInitBookOutline",
        "taliBookOutline",
        "taliBookMarkSection",
    ] {
        assert!(
            !js.contains(needle),
            "`{needle}` still ships in the client bundle"
        );
    }
    let css = taliesin_core::render::site_css();
    for needle in [
        "tali-book-expand",
        "tali-book-sections",
        "tali-book-section",
        "tali-book-section-active",
        "tali-book-sd2",
        "tali-book-sd3",
        "tali-book-sd4",
        "tali-book-row",
    ] {
        assert!(!css.contains(needle), "`{needle}` CSS survives in site.css");
    }
}

/// The book topbar's offline-download button was deleted 2026-08-04 (visual minimalism
/// pass, task 11): one more permanent control in the topbar for an action a reader rarely
/// wants. The `<book>.zip` build step itself (`write_book_archive` in `build.rs`) still
/// runs for every book build — only the topbar link to it is gone.
#[test]
fn the_book_download_button_is_gone() {
    let css = taliesin_core::render::site_css();
    assert!(
        !css.contains("tali-book-download"),
        "its CSS survives in site.css"
    );
}

/// Callout kinds went 5 -> 3 on 2026-08-03 (visual minimalism pass, task 12): readers
/// cannot decode *important* vs *warning* vs *caution* visually, and all three rendered
/// as a coloured box with a different word in it. Both cut kinds must be registered in
/// `RETIRED_KEYS` (scope `"callout kind"`, matching the exact literal
/// `validate_callout_kind` passes to `unknown_key_message`) or an author who leaves one
/// in a document gets silence: a plain unstyled box, no diagnostic.
#[test]
fn callout_kinds_are_three_and_the_two_cut_ones_are_registered() {
    let kinds = taliesin_core::render::callout_kinds();
    assert_eq!(
        kinds,
        &["note", "tip", "warning"],
        "callout vocabulary should be 3"
    );
    for gone in ["important", "caution"] {
        assert!(
            taliesin_core::frontmatter::retired_note("callout kind", gone).is_some(),
            "`{gone}` must have a RETIRED_KEYS entry or an author gets silence"
        );
    }
    // The CSS half of the same subtraction: the retired kinds' selector rules must be
    // gone from base.css too (the underlying `--tali-callout-important`/`-caution` CSS
    // custom properties stay defined — `.tali-error`/`.tali-js-error` and the frozen
    // `deck.css` still read `--tali-callout-important`, and `deck.css` alone still reads
    // `--tali-callout-caution` — so only the class *selectors* are checked here, not the
    // tokens themselves).
    let css = taliesin_core::render::base_css();
    for needle in [".callout-important", ".callout-caution"] {
        assert!(
            !css.contains(needle),
            "`{needle}` selector rule survives in base.css"
        );
    }
}

/// Margin-content spellings went 4 -> 1 on 2026-08-03 (visual minimalism pass, task 13):
/// `.aside` and `.marginnote` had zero uses in the tree; `.sidenote` had exactly one
/// (`samples/paper.tmd`, migrated to `.column-margin` in the same change). The three
/// aliases were a Quarto/Tufte/Distill welcome mat for a tool that has otherwise shed its
/// Quarto vocabulary.
///
/// `validate.rs`'s own `#[cfg(test)]` block pins that `RETIRED_DIV_CLASSES` carries all
/// three entries directly, since the const is `pub(crate)` and unreachable from an
/// integration test. This pins the EMITTED surface instead: the actual diagnostic an
/// author sees through the full render pipeline, plus the CSS half of the same
/// subtraction. Div classes are an open vocabulary — the validator stays silent on a
/// class it does not recognize, since it cannot tell a typo from a legitimate custom
/// class — so without a `RETIRED_DIV_CLASSES` entry a leftover `.sidenote` would get
/// NOTHING: no error, no warning, no did-you-mean, and the page would quietly lose its
/// margin layout. That silence is the exact failure mode this test guards against.
#[test]
fn margin_aliases_are_retired_through_the_full_render_pipeline() {
    for gone in ["aside", "sidenote", "marginnote"] {
        let src = format!("::: {{.{gone}}}\nx\n:::\n");
        let doc = taliesin_core::render::render_document(&src);
        let w = doc
            .warnings
            .iter()
            .find(|w| w.message.contains("div class"))
            .unwrap_or_else(|| {
                panic!(
                    "`.{gone}` must warn (silence is the failure mode this test exists to \
                     catch); warnings: {:?}",
                    doc.warnings
                )
            });
        assert!(
            w.message
                .starts_with(&format!("unknown div class `{gone}`: it was removed")),
            "`.{gone}` must carry the removal note, got: {}",
            w.message
        );
        assert!(
            w.message
                .contains("`.column-margin` is the only margin spelling now"),
            "`.{gone}`'s removal note must point at `.column-margin`, got: {}",
            w.message
        );
        assert!(
            !w.message.contains("did you mean"),
            "a retired class is not a did-you-mean: {}",
            w.message
        );
        // Purely diagnostic: the div still renders with its given class (validate.rs's own
        // contract for every validator), so the warning above is the ONLY thing telling the
        // author their margin note stopped working.
        assert!(
            doc.body_html().contains(&format!("class=\"{gone}\"")),
            "`.{gone}` must still render with its given class: {}",
            doc.body_html()
        );
    }
    // The CSS half of the same subtraction: none of the three retired spellings are styled
    // any more (a leftover `.sidenote` therefore really does render as a plain unstyled
    // block, matching the warning above). `.tali-sidenote` (the unrelated auto-generated
    // footnote margin note, a different feature entirely) is deliberately NOT in this list.
    let css = taliesin_core::render::base_css();
    for needle in [".sidenote", ".marginnote", ".aside"] {
        assert!(
            !css.contains(needle),
            "`{needle}` selector rule survives in base.css"
        );
    }
    let site_css = taliesin_core::render::site_css();
    for needle in [".sidenote", ".marginnote", ".aside"] {
        assert!(
            !site_css.contains(needle),
            "`{needle}` selector rule survives in site.css"
        );
    }
    // `.column-margin` itself must survive, styled, untouched.
    assert!(
        css.contains(".column-margin {"),
        "`.column-margin` must still be styled in base.css"
    );
}

/// Theorem kinds went 8 -> 5 on 2026-08-03 (visual minimalism pass, task 14): `example`,
/// `proposition` and `remark` were never cross-referenced by any document in the tree.
///
/// Unlike a callout kind, a theorem kind carries NO namespace prefix (`.theorem`
/// dispatches directly; there is no `theorem-` prefix the way there is `callout-`), so
/// `THEOREM_KINDS` itself IS the dispatch vocabulary. A misspelled or retired kind has
/// nothing to anchor a "did you mean", so without a `RETIRED_DIV_CLASSES` entry it falls
/// through to a plain, unnumbered, unreferenceable div with NO diagnostic at all — the
/// exact silent failure mode this test guards against. `validate.rs`'s own `#[cfg(test)]`
/// block pins that `RETIRED_DIV_CLASSES` carries all three entries directly, since the
/// const is `pub(crate)` and unreachable from this integration test; this pins the
/// EMITTED surface, through the full render pipeline, plus the CSS half of the same
/// subtraction and the SECOND, independent vocabulary this retirement also touches: the
/// `@exm-`/`@prp-`/`@rem-` cross-reference prefixes — which resolve the OPPOSITE way:
/// those three stay in `cite::XREF_LABELS` on purpose (see its doc comment), so a
/// leftover reference is a loud "broken cross-reference" instead of silent text.
#[test]
fn theorem_kinds_are_five_and_the_three_cut_ones_are_registered() {
    let kinds = taliesin_core::render::theorem_kinds();
    assert_eq!(
        kinds,
        &["theorem", "lemma", "corollary", "definition", "proof"],
        "theorem vocabulary should be 5"
    );
    for gone in ["example", "proposition", "remark"] {
        assert!(
            taliesin_core::render::retired_div_note(gone).is_some(),
            "`.{gone}` needs a RETIRED_DIV_CLASSES entry — a misspelled theorem kind has \
             no prefix to anchor a did-you-mean and falls through to a plain div"
        );
        // The full render pipeline: silence is the failure mode this test exists to catch.
        let src = format!("::: {{.{gone}}}\nx\n:::\n");
        let doc = taliesin_core::render::render_document(&src);
        let w = doc
            .warnings
            .iter()
            .find(|w| w.message.contains("div class"))
            .unwrap_or_else(|| {
                panic!(
                    "`.{gone}` must warn (silence is the failure mode this test exists to \
                     catch); warnings: {:?}",
                    doc.warnings
                )
            });
        assert!(
            w.message
                .starts_with(&format!("unknown div class `{gone}`: it was removed")),
            "`.{gone}` must carry the removal note, got: {}",
            w.message
        );
        assert!(
            !w.message.contains("did you mean"),
            "a retired class is not a did-you-mean: {}",
            w.message
        );
        // Purely diagnostic: the div still renders with its given class, matching every
        // other validator in `validate.rs`.
        assert!(
            doc.body_html().contains(&format!("class=\"{gone}\"")),
            "`.{gone}` must still render with its given class: {}",
            doc.body_html()
        );
    }
    // The CSS half of the same subtraction. `.tali-thm-style-remark` was the only
    // kind-specific style selector among the three cut kinds — `example` and `proposition`
    // reused the surviving `definition`/`plain` styles, so there was never a per-kind
    // selector for either of them to begin with.
    let css = taliesin_core::render::base_css();
    assert!(
        !css.contains(".tali-thm-style-remark"),
        "`.tali-thm-style-remark` selector rule survives in base.css"
    );
    assert!(
        css.contains(".tali-thm-style-plain") && css.contains(".tali-thm-style-definition"),
        "the two surviving theorem styles must still be styled in base.css"
    );

    // The SECOND vocabulary this retirement touches: the cross-reference prefixes.
    // Fix round 1 (2026-08-04): the first cut of this task ALSO deleted `exm`/`prp`/
    // `rem` from `cite::XREF_LABELS`, which was wrong — with the prefix gone,
    // `@exm-x` never reaches `parse_xref`'s `Some` branch at all, so it degraded
    // silently to literal text (no link, no error, nothing). The three prefixes are
    // kept ON PURPOSE now, specifically because their div classes are gone: every
    // `@exm-`/`@prp-`/`@rem-` reference is therefore necessarily dangling (nothing can
    // ever define that anchor again), which the ordinary "broken cross-reference" path
    // already reports for free — no retirement register needed, unlike the div-class
    // case above.
    for prefix in ["exm", "prp", "rem"] {
        let anchor = format!("{prefix}-x");
        assert!(
            taliesin_core::cite::is_xref_anchor(&anchor),
            "`@{anchor}` must still be a recognized cross-reference SHAPE (its div class \
             is gone, but the prefix survives so a stray reference errors loudly instead \
             of degrading to silent text)"
        );
    }
    // Render a document that references `@exm-oldid` with nothing anywhere to define it
    // (impossible after this retirement, since no div class produces an `exm-` anchor any
    // more) and confirm it is reported as a broken cross-reference, not silence.
    let dangling = taliesin_core::render_document_with_includes(
        "---\ntitle: T\n---\n\nSee @exm-oldid, @prp-oldid, @rem-oldid.\n",
        std::path::Path::new("."),
    );
    let xref_warnings = taliesin_core::cite::validate_xrefs(&dangling.blocks);
    for prefix in ["exm", "prp", "rem"] {
        let anchor = format!("{prefix}-oldid");
        assert!(
            xref_warnings.iter().any(|w| w
                .message
                .contains(&format!("broken cross-reference: @{anchor}"))),
            "`@{anchor}` must be reported as a broken cross-reference, not silence: {:?}",
            xref_warnings
        );
    }
    // The five surviving kinds' prefixes must still resolve, end to end: a real theorem
    // env with an id in the wild renders as a linked, numbered cross-reference. `proof`
    // deliberately carries no prefix (unnumbered, unreferenceable by design) so it is not
    // in this list.
    let src = "::: {.theorem #thm-a}\nT.\n:::\n::: {.lemma #lem-a}\nL.\n:::\n\
               ::: {.corollary #cor-a}\nC.\n:::\n::: {.definition #def-a}\nD.\n:::\n\
               ::: {.proof}\nP.\n:::\n\nSee @thm-a, @lem-a, @cor-a, @def-a.\n";
    let doc = taliesin_core::render::render_document(src);
    let body = doc.body_html();
    for (anchor, label) in [
        ("thm-a", "Theorem"),
        ("lem-a", "Lemma"),
        ("cor-a", "Corollary"),
        ("def-a", "Definition"),
    ] {
        assert!(
            body.contains(&format!(
                "<a href=\"#{anchor}\" class=\"tali-xref\">{label}&nbsp;1</a>"
            )),
            "`@{anchor}` must resolve to a numbered, linked cross-reference: {body}"
        );
    }
    // `proof` deliberately carries no `data-tali-theorem-kind` (it is not numbered or
    // cross-referenceable by design; `proof_emits_qed_and_no_number_slot` in
    // `render/tests.rs` pins the number-slot half of the same contract in isolation).
    assert!(
        body.contains("class=\"tali-proof\"") && !body.contains("data-tali-theorem-kind=\"proof\""),
        "the fifth survivor, `proof`, must still render, unnumbered: {body}"
    );
}

/// The `?` and `/` character-key shortcuts were deleted 2026-08-04 (visual minimalism
/// pass, task 15), and with them the WCAG 2.1.4 off-switch they forced into the
/// Settings menu: `taliShortcutsOn`/`taliSetShortcuts` (code-enhance/01-registry.js),
/// storage key `tali-shortcuts`, and the "Keyboard shortcuts" section 07-keyboard.js
/// mounted (its `.tali-keys-list` cheatsheet). Esc and the arrow keys are not
/// character keys, so they stay live with no control needed.
#[test]
fn character_key_shortcuts_and_their_offswitch_are_gone() {
    let js = taliesin_core::render::code_scripts();
    for needle in [
        "taliShortcutsOn",
        "taliSetShortcuts",
        "tali-shortcuts",
        "Keyboard shortcuts",
        "tali-keys-list",
    ] {
        assert!(
            !js.contains(needle),
            "`{needle}` still ships; the character-key shortcuts and their WCAG 2.1.4 \
             off-switch were deleted together"
        );
    }
    assert!(
        js.contains("ArrowLeft") || js.contains("ArrowRight"),
        "the arrow-key chapter nav must SURVIVE, it is not a character key"
    );

    let css = taliesin_core::render::base_css();
    assert!(
        !css.contains(".tali-keys-list"),
        "the shortcuts cheatsheet's CSS survives in base.css"
    );

    // The generic `window.taliReaderMenu.addSection(...)` mounting API (13-reader-menu.js)
    // survives untouched: Theme (14-reader-prefs.js) is its one remaining live caller.
    assert!(
        js.contains("addSection"),
        "the Settings menu's generic section-mounting API must survive; Theme still uses it"
    );
}

/// `{pyodide}` was withdrawn on 2026-08-04 (MVP scope pass): a vendored 15.7 MiB
/// CPython/WASM runtime that could only ever ship the stdlib plus NumPy, since the tool
/// does no network fetch, which is exactly the workload `{js}` already covers at zero
/// marginal bytes. Adoption at withdrawal was author 0 / manual 1 / pin 1.
///
/// **Fence languages are an OPEN vocabulary**, so this needs the same kind of retirement
/// register `RETIRED_KEYS` gives front matter and `RETIRED_DIV_CLASSES` gives fenced divs.
/// Without one an author who leaves a `{pyodide}` cell in a document does not get silence
/// (the generic `TAL-CODE-LANG` arm still fires) but gets something arguably worse: advice
/// to "check the spelling", when the spelling was right and the capability is gone. This
/// pins the specific note instead, through the full render pipeline rather than by reading
/// the const, so it is the diagnostic an author actually sees.
#[test]
fn a_leftover_pyodide_cell_is_told_it_was_withdrawn_not_that_it_is_a_typo() {
    let lang = format!("{}{}", "pyo", "dide");
    assert!(
        taliesin_core::diagnostics::retired_cell_lang(&lang).is_some(),
        "`{lang}` must have a RETIRED_CELL_LANGS entry or an author is told to check the spelling"
    );

    let doc = taliesin_core::render::render_document_with_includes(
        &format!("intro\n\n```{{{lang}}}\nimport numpy as np\n```\n"),
        Path::new("."),
    );
    let ws = taliesin_core::diagnostics::validate_code_languages(&doc.blocks);
    let w = ws
        .iter()
        .find(|w| w.message.contains("was removed"))
        .unwrap_or_else(|| panic!("expected a retirement warning, got {:?}", ws));
    // **The severity, which is the half a unit test alone missed.** Classified, this message
    // is TAL-CELL-RETIRED/WARNING. Unclassified it falls through to `(GENERIC, ERROR)` and
    // fails `check`, `build --strict` and `publish` on a document that merely has not been
    // migrated — measured by running `taliesin check` on a leftover cell, which reported
    // `error[TAL-CHECK]` before the classifier row existed. Asserting the pair (not just that
    // a diagnostic fired) is what makes this fail for that regression.
    assert_eq!(
        taliesin_core::diagnostics::codes::classify(&w.message),
        (
            "TAL-CELL-RETIRED",
            taliesin_core::diagnostics::codes::WARNING
        ),
        "a retired cell language must not fall through to the generic error: {}",
        w.message
    );
    // The replacement must be NAMED. A retirement note that only says "gone" leaves the
    // author with a broken document and no next step, which is the whole reason the
    // register carries a note rather than a bare list of withdrawn spellings.
    assert!(
        w.message.contains("`{js}`") && w.message.contains("`{python}`"),
        "the note must name both replacements: {}",
        w.message
    );
    // Located, like every other member of this family: an unlocated warning cannot be
    // clicked back to the offending fence.
    assert_eq!(w.line, Some(3), "points at the fence, not the doc start");
    // The generic spelling advice must NOT also fire: two warnings for one cell, one of
    // them actively wrong, is the state this register exists to prevent.
    assert!(
        !w.message.contains("check the spelling"),
        "the retirement note must REPLACE the generic unknown-language advice: {}",
        w.message
    );
    assert_eq!(
        ws.len(),
        1,
        "exactly one warning per withdrawn cell: {ws:?}"
    );
}

/// The runtime's bytes, its enhancer and its corpus pin must all be gone from the tree,
/// not merely unreferenced. 15.7 MiB of vendored WASM that no code path can reach is the
/// failure shape this catches: `cargo test` stays green while the payload still ships.
#[test]
fn the_vendored_browser_python_runtime_is_gone_from_the_tree() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lang = format!("{}{}", "pyo", "dide");
    for gone in [
        core.join(format!("assets/{lang}")),
        core.join(format!("assets/js/{lang}.js")),
        core.join(format!("src/render/{lang}.rs")),
        repo_root().join(format!("corpus/reactive/{lang}.tmd")),
    ] {
        assert!(
            !gone.exists(),
            "`{}` survives the withdrawal",
            gone.display()
        );
    }
    // The language is out of the registry, so no emitter can produce a live wrapper for it.
    assert!(
        taliesin_core::render::client_lang(&lang).is_none(),
        "`{lang}` is still a registered client language"
    );
}
