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
