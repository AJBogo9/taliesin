//! The emitted-HTML attribute contract, pinned as one census.
//!
//! Every other test in the suite asserts on string literals that live in the same
//! file as the emitter, so a rename moves both sides together and nothing fails.
//! (Measured 2026-07-25: a blanket rename of the retired prefix over the tree built
//! clean and changed the state of 5 of 1387 tests, 3 of them only block-id hash
//! drift.) This file is the one place a change to the `data-*` vocabulary must be
//! declared by hand, which makes an incomplete rename a visible diff instead of a
//! silent one.

mod common;

use common::corpus_dir;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Every `data-*` attribute name the corpus renders. Sorted. Update deliberately.
const EMITTED_DATA_ATTRS: &[&str] = &[
    "data-block-id",
    "data-inputs",
    "data-name",
    "data-section-end",
    "data-source-file",
    "data-sourcepos",
    "data-tali-cell",
    "data-tali-input",
    "data-tali-out",
    "data-tali-out-for",
    "data-tali-xref",
    "data-target",
    "data-viewof",
];

/// Every `data-*` attribute name the bundled browser code selects on. Sorted.
///
/// A block-level corpus render only reaches a fraction of the vocabulary: site
/// chrome, the page shell, and attributes JS stamps at runtime (`data-*-ran`,
/// `data-*-bound`) never appear in it. Measured 2026-07-25: the render census sees
/// 4 of the 13 `data-tali-*` names the browser code references. This second pin
/// covers the rest, so a rename cannot move one side and leave the other.
///
/// Entries ending in `-` are attribute name PREFIXES built by string concatenation
/// (e.g. `"data-background-" + kind`). They are kept deliberately: a split token is
/// exactly what a find-and-replace misses.
const BROWSER_SELECTED_DATA_ATTRS: &[&str] = &[
    "data-attribute",
    "data-attrs",
    "data-block-id",
    "data-drawer-wired",
    "data-inputs",
    "data-label",
    "data-mermaid-error",
    "data-name",
    "data-nav-wired",
    "data-processed",
    "data-scroll-a11y",
    "data-section-end",
    "data-source-file",
    "data-sourcepos",
    "data-src",
    "data-state",
    "data-tali-bound",
    "data-tali-cell",
    "data-tali-cell-source",
    "data-tali-cell-state",
    "data-tali-done",
    "data-tali-drawer-close",
    "data-tali-input",
    "data-tali-input-bound",
    "data-tali-op",
    "data-tali-out",
    "data-tali-ran",
    "data-tali-search",
    "data-tali-settings",
    "data-tali-src",
    "data-tali-theme-toggle",
    "data-tali-xref",
    "data-target",
    "data-theme",
    "data-viewof",
    "data-wired",
];

/// Emitted attributes with no browser-side consumer. Each needs a stated reason,
/// because "nothing selects on this" is otherwise indistinguishable from "the
/// rename moved the emitter and forgot the consumer".
const NO_RUNTIME_CONSUMER: &[(&str, &str)] = &[
    (
        "data-tali-xref",
        "build-time only: cite/validate.rs:15 scans for it as a Rust string needle to report unresolved cross-references",
    ),
    (
        "data-section-end",
        "informational substrate, no consumer YET and that is the decision, not an oversight: \
         `section-extents` option (b), ruled 2026-07-26. Blocks are flat siblings with no \
         per-section wrapper, so the DOM could not say where a section stops; render/mod.rs's \
         `mark_section_extents` now records it on each heading. Emitted default-on so a consumer \
         (per-section length, section-scoped read state or change marks, a JS-driven fold) needs \
         no render change to arrive. Option (a), a real <section> wrapper, was explicitly \
         deferred: it changes the parent/child shape the incremental diff mounts.",
    ),
    (
        "data-tali-out-for",
        "build-time only: it marks the empty output slot a `:::` container leaves for a code \
         cell it folded away (render/divs.rs's output_slot), and exec.rs's fill_output_slot \
         scans for it as a Rust string needle when the cell's output comes back. The browser \
         never asks for it: the slot ALSO carries the same `{cell}-out` data-block-id a \
         top-level output block would, which is the name client.js already looks up for \
         streaming output and per-cell state, so a nested cell needs no second lookup path.",
    ),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// How strictly `data-` must be delimited. The two inputs need different rules and
/// unifying them breaks one or the other (both failure modes measured 2026-07-25).
#[derive(Clone, Copy, PartialEq)]
enum Scan {
    /// Rendered HTML: `data-` must be preceded by whitespace AND the name must be
    /// followed by `=`, `>` or `/`, i.e. sit in real attribute position.
    ///
    /// Both halves are load-bearing. Without the first, `id="data-modeling"` matches
    /// (the slug of a `# Data Modeling` heading in `corpus/single-page-report/`).
    /// Without the second, ordinary prose matches: "the canonical data-figure loop"
    /// in `corpus/recipes/csv-figure.tmd` and "data-vs-model push-pull" in
    /// `corpus/posts/born-machines.tmd` both sit after a space.
    ///
    /// Known limit: a valueless attribute followed by *another* attribute rather
    /// than by `>` is missed here. That is fine, because `Scan::Source` censuses the
    /// vocabulary comprehensively; this census exists to prove what the emitters
    /// actually put on the page.
    Html,
    /// Source code: `data-` need only start a token, because attributes appear
    /// inside strings as `querySelectorAll("[data-x]")` or `'[data-x]'`. The
    /// whitespace rule finds 1 of the 13 `data-tali-*` names in the browser sources,
    /// and a census that cannot see a name cannot notice it being renamed.
    Source,
}

/// Collect every `data-<name>` attribute name in `text`.
///
/// A trailing `-` is preserved, marking a name built by concatenation
/// (`"data-background-" + kind`). Those are the tokens a find-and-replace misses.
fn scan_data_attrs(text: &str, mode: Scan, out: &mut BTreeSet<String>) {
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while let Some(off) = text[i..].find("data-") {
        let start = i + off;
        let delimited = start == 0 || {
            let p = bytes[start - 1];
            match mode {
                Scan::Html => p.is_ascii_whitespace(),
                Scan::Source => !(p.is_ascii_alphanumeric() || p == b'_' || p == b'-'),
            }
        };
        if !delimited {
            i = start + 5;
            continue;
        }
        let rest = &text[start + 5..];
        let end = rest
            .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
            .unwrap_or(rest.len());
        let in_attr_position = mode == Scan::Source
            || matches!(
                rest.as_bytes().get(end),
                Some(b'=') | Some(b'>') | Some(b'/')
            );
        if end > 0 && in_attr_position {
            out.insert(format!("data-{}", &rest[..end]));
        }
        i = start + 5 + end.max(1);
    }
}

fn tmd_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if name.starts_with('_') || name == "node_modules" {
                continue;
            }
            tmd_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "tmd") {
            out.push(p);
        }
    }
}

fn census() -> BTreeSet<String> {
    let mut files = Vec::new();
    tmd_files(&corpus_dir(), &mut files);
    files.sort();
    assert!(!files.is_empty(), "corpus/ has no .tmd files to census");

    let mut attrs = BTreeSet::new();
    for f in &files {
        let src =
            std::fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()));
        // `_with_includes` so blocks pulled in via `{{< include >}}` are censused too.
        let doc = taliesin_core::render_document_with_includes(&src, f.parent().unwrap());
        for block in &doc.blocks {
            scan_data_attrs(&block.html, Scan::Html, &mut attrs);
        }
    }
    attrs
}

/// Every file whose contents can select on an attribute at runtime: the bundled
/// browser assets, the preview client, and any Rust file carrying inline `<script>`
/// (the book drawer, the theme runtime, the deck runtime, the search UI, ...).
///
/// The Rust half is found by content, not by a hand-written list: `data-tali-drawer-close`
/// lives only inside a `<script>` string literal in `site/chrome.rs`, so a list that
/// happened to omit that file would silently drop the attribute from the census.
/// Browser-side files ONLY: the bundled assets and the preview client. Deliberately
/// excludes Rust, because Rust is where attributes are *emitted*; counting it as a
/// consumer makes the orphan check vacuous. (Measured 2026-07-25: renaming
/// `data-tali-out` in the Rust emitter alone left the orphan check passing, because
/// the new name appeared in the very file that had just been edited.)
fn browser_sources() -> String {
    let root = repo_root();
    let mut buf = String::new();
    let asset_dirs = [
        root.join("crates/core/assets/js"),
        root.join("crates/core/assets/js/code-enhance"),
        root.join("crates/core/assets/css"),
        root.join("web-client"),
    ];
    for d in asset_dirs {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if name.ends_with(".min.js") || !(name.ends_with(".js") || name.ends_with(".css")) {
                continue;
            }
            buf.push_str(&std::fs::read_to_string(&p).unwrap_or_default());
            buf.push('\n');
        }
    }
    buf
}

fn runtime_sources() -> String {
    let root = repo_root();
    let mut buf = String::new();

    let asset_dirs = [
        root.join("crates/core/assets/js"),
        root.join("crates/core/assets/js/code-enhance"),
        root.join("crates/core/assets/css"),
        root.join("web-client"),
    ];
    for d in asset_dirs {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if name.ends_with(".min.js") || !(name.ends_with(".js") || name.ends_with(".css")) {
                continue;
            }
            buf.push_str(&std::fs::read_to_string(&p).unwrap_or_default());
            buf.push('\n');
        }
    }

    let mut rs = Vec::new();
    for d in ["crates/core/src", "crates/server/src"] {
        rust_files(&root.join(d), &mut rs);
    }
    rs.sort();
    for p in rs {
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        if text.contains("<script") {
            buf.push_str(&text);
            buf.push('\n');
        }
    }
    buf
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn emitted_data_attribute_census_is_pinned() {
    let actual: Vec<String> = census().into_iter().collect();
    let expected: Vec<String> = EMITTED_DATA_ATTRS.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        actual, expected,
        "the emitted data-* vocabulary changed.\n\
         If this is a deliberate rename, paste the ACTUAL list into EMITTED_DATA_ATTRS \
         and confirm every consumer (assets/css, assets/js, web-client) moved with it.\n\
         ACTUAL:\n{actual:#?}"
    );
}

/// `data-foo-bar` as JS's camelCase dataset accessor, `dataset.fooBar`. Browser code
/// reaches an attribute either way, so an orphan check that only looks for the
/// hyphenated literal reports false positives.
fn dataset_accessor(attr: &str) -> String {
    let mut out = String::from("dataset.");
    let mut upper = false;
    for c in attr.trim_start_matches("data-").chars() {
        if c == '-' {
            upper = true;
        } else if upper {
            out.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

#[test]
fn browser_selected_data_attribute_census_is_pinned() {
    let mut attrs = BTreeSet::new();
    scan_data_attrs(&runtime_sources(), Scan::Source, &mut attrs);
    let actual: Vec<String> = attrs.into_iter().collect();
    let expected: Vec<String> = BROWSER_SELECTED_DATA_ATTRS
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        actual, expected,
        "the data-* vocabulary the browser code selects on changed.\n\
         If this is a deliberate rename, paste the ACTUAL list into \
         BROWSER_SELECTED_DATA_ATTRS and confirm the Rust emitters moved with it.\n\
         ACTUAL:\n{actual:#?}"
    );
}

#[test]
fn every_emitted_attribute_has_a_runtime_consumer() {
    let sources = browser_sources();
    let exempt: BTreeSet<&str> = NO_RUNTIME_CONSUMER.iter().map(|(a, _)| *a).collect();
    // The LIVE census, not EMITTED_DATA_ATTRS. Reading the pin makes this test lag a
    // rename by one step: it would keep checking the old name against the browser,
    // find it, and pass, until someone got around to updating the pin.
    let orphans: Vec<String> = census()
        .into_iter()
        .filter(|a| {
            !exempt.contains(a.as_str())
                && !sources.contains(a)
                && !sources.contains(&dataset_accessor(a))
        })
        .collect();
    assert!(
        orphans.is_empty(),
        "attributes emitted into HTML that no bundled CSS/JS references, by literal \
         or by dataset accessor:\n{orphans:#?}\n\
         Either a rename moved the Rust side without the browser side, or the attribute \
         is informational and belongs in NO_RUNTIME_CONSUMER with a reason."
    );
}

// ---------------------------------------------------------------------------
// The `--tali-*` custom-property vocabulary
// ---------------------------------------------------------------------------

/// Every `--tali-*` a stylesheet READS must be defined somewhere, or carry a fallback.
///
/// **Why this gate exists.** CSS custom properties fail *silently and destructively*: a
/// `var()` naming a property nothing defines makes the whole declaration invalid at
/// computed-value time, so the browser drops it — and for a shorthand it drops every
/// longhand with it. `border: 1px solid var(--tali-rule)` where no `--tali-rule` exists is
/// not a hairline in the wrong colour, it is **no border at all**, and nothing anywhere
/// reports it: the sheet parses, the page loads, the suite is green, and the element is
/// simply invisible.
///
/// This is not hypothetical. It shipped on 2026-07-29 in the same change that added this
/// test: three invented names (`--tali-rule`, `--tali-surface`, `--tali-radius`, against
/// the real `--tali-border`, `--tali-code-bg`, `--tali-radius-sm`) made the draggable
/// `point` pad render as a floating dot with no box, and it took a browser screenshot to
/// notice. The real vocabulary is 60-odd names; guessing one is easy.
///
/// A `var(--x, fallback)` reference is exempt: the fallback is what makes it well-defined.
#[test]
fn every_tali_custom_property_read_is_defined_somewhere() {
    let root = repo_root();

    // Definitions come from anywhere a value can be assigned: the bundled sheets (including
    // inside `@keyframes`), a JS `setProperty`, and the Rust that emits inline custom
    // themes / per-page style blocks.
    let mut defined: BTreeSet<String> = BTreeSet::new();
    let mut read: BTreeSet<(String, String)> = BTreeSet::new(); // (name, file)

    let mut files: Vec<PathBuf> = Vec::new();
    for d in [
        root.join("crates/core/assets/css"),
        root.join("crates/core/assets/js"),
        root.join("crates/core/assets/js/code-enhance"),
        root.join("web-client"),
    ] {
        if let Ok(entries) = std::fs::read_dir(&d) {
            for e in entries.flatten() {
                let p = e.path();
                let name = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if name.ends_with(".min.js") || !(name.ends_with(".js") || name.ends_with(".css")) {
                    continue;
                }
                files.push(p);
            }
        }
    }
    let mut rs = Vec::new();
    for d in ["crates/core/src", "crates/server/src"] {
        rust_files(&root.join(d), &mut rs);
    }
    files.extend(rs);
    files.sort();

    for p in &files {
        let text = std::fs::read_to_string(p).unwrap_or_default();
        let label = p
            .strip_prefix(&root)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string();
        for (i, _) in text.match_indices("--tali-") {
            let rest = &text[i..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(rest.len());
            let name = rest[..end].to_string();
            let after = rest[end..].trim_start();
            // `--x:` is a definition (in a rule, in `@keyframes`, or in a JS/Rust string).
            if after.starts_with(':') {
                defined.insert(name);
                continue;
            }
            // `setProperty('--x', …)` / `setProperty("--x", …)` is a runtime definition.
            let before = &text[..i];
            let head = before.trim_end().trim_end_matches(['"', '\'']);
            if head.ends_with("setProperty(") {
                defined.insert(name);
                continue;
            }
            // A read. `var(--x, fallback)` is well-defined by its fallback, so exempt.
            if before.trim_end().ends_with("var(") && !after.starts_with(',') {
                read.insert((name, label.clone()));
            }
        }
    }

    assert!(
        defined.len() > 30,
        "only {} `--tali-*` definitions found — the scan broke, and an empty gate passes \
         forever",
        defined.len()
    );
    assert!(
        read.len() > 30,
        "only {} `--tali-*` reads found — the scan broke",
        read.len()
    );

    let orphans: Vec<String> = read
        .iter()
        .filter(|(name, _)| !defined.contains(name))
        .map(|(name, file)| format!("{name} (read in {file})"))
        .collect();
    assert!(
        orphans.is_empty(),
        "these `--tali-*` properties are read but never defined, so every declaration \
         using one is silently DROPPED by the browser (a missing border, not a wrong \
         one). Use an existing token or define it in tokens.css:\n  {}",
        orphans.join("\n  ")
    );
}
