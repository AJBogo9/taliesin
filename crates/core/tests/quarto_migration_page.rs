//! The "Coming from Quarto" chapter is *derived from* the vocabulary constants, not
//! written from memory beside them.
//!
//! `check` already emits a located diagnostic for every Quarto-ism the tool knows about;
//! the migration assistant has shipped for a long time and nothing said so. The page that
//! says so is only worth having while it stays complete, and the way a page like that dies
//! is silent: someone adds `"markdown"` to `NON_HTML_FORMATS`, or retires another key, and
//! the chapter keeps listing the old set with no test anywhere disagreeing.
//!
//! So read the constants out of the sources and require the page to name each entry. The
//! constants are `pub(crate)`, and widening them to `pub` to satisfy a test would be the
//! tail wagging the dog — parsing the source text is what `stale_docs.rs` and
//! `gate_script.rs` already do for the same reason.

use std::path::{Path, PathBuf};

const PAGE: &str = "docs/guide/using/from-quarto.tmd";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// The body of `const <name>: … = &[ … ];`, i.e. everything up to the first `];`.
fn const_block<'a>(src: &'a str, name: &str) -> &'a str {
    let start = src
        .find(&format!("const {name}:"))
        .unwrap_or_else(|| panic!("no `const {name}:` in the source — was it renamed?"));
    let tail = &src[start..];
    let end = tail
        .find("];")
        .unwrap_or_else(|| panic!("`const {name}` has no closing `];`"));
    &tail[..end]
}

/// Every string literal in a slice of source, in order. Handles `\"` so a message
/// containing a quote cannot silently re-pair the delimiters and shift every later entry.
fn string_literals(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut lit = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    chars.next();
                }
                '"' => break,
                _ => lit.push(c),
            }
        }
        out.push(lit);
    }
    out
}

#[test]
fn the_migration_page_exists_and_is_in_the_guides_nav() {
    assert!(
        repo_root().join(PAGE).is_file(),
        "{PAGE} is missing: the migration assistant ships and nothing documents it"
    );
    assert!(
        read("docs/guide/_site.yml").contains("using/from-quarto.tmd"),
        "{PAGE} is not listed in docs/guide/_site.yml, so it is an orphan page: built, \
         reachable only by URL, and in no book's nav"
    );
}

/// Every output format `check` reports as unsupported is named on the page. A format added
/// to the diagnostic and not to the page is a migrant who is told "unsupported" by the tool
/// and finds no list saying which ones those are.
#[test]
fn the_page_lists_every_unsupported_output_format() {
    let src = read("crates/core/src/frontmatter.rs");
    let formats = string_literals(const_block(&src, "NON_HTML_FORMATS"));
    assert!(
        formats.len() >= 5,
        "parsed only {formats:?} out of NON_HTML_FORMATS — the parser, not the page, is \
         what broke"
    );

    let page = read(PAGE);
    for f in &formats {
        // The backticked spelling, not the bare word: `pdf` occurs in prose ("no PDF
        // target") on a page that lists none of these.
        assert!(
            page.contains(&format!("`{f}`")),
            "`{f}` is reported as an unsupported format but {PAGE} never names it \
             (page must list all of {formats:?})"
        );
    }
}

/// Every retired or unhonoured front-matter key is on the page, with its colon — a bare
/// `about` is an English word and would match by accident.
#[test]
fn the_page_names_every_retired_and_unhonoured_key() {
    let src = read("crates/core/src/frontmatter.rs");

    // RETIRED_KEYS is `(scope, key, what to do instead)`; the key is the second literal
    // of each triple.
    let retired: Vec<String> = string_literals(const_block(&src, "RETIRED_KEYS"))
        .chunks(3)
        .filter_map(|t| t.get(1).cloned())
        .collect();
    let unsupported = string_literals(const_block(&src, "UNSUPPORTED_KEYS"));

    assert!(
        !retired.is_empty() && !unsupported.is_empty(),
        "parsed retired={retired:?} unsupported={unsupported:?} — the parser broke"
    );

    let page = read(PAGE);
    for key in retired.iter().chain(unsupported.iter()) {
        assert!(
            page.contains(&format!("{key}:")),
            "`{key}:` warns on a migrated document but {PAGE} does not tell the reader \
             what to do about it (retired={retired:?}, unsupported={unsupported:?})"
        );
    }
}

/// The link-extension family the tool offers a `.tmd` spelling for. This is the single
/// biggest source of noise in a fresh migration (118 of 123 link errors on one real book),
/// so the page has to name the whole set it covers, not the two that were measured.
#[test]
fn the_page_names_every_migrated_link_extension() {
    let src = read("crates/core/src/ext.rs");
    let exts = string_literals(const_block(&src, "MIGRATED_DOC_EXTS"));
    assert!(
        exts.len() >= 4,
        "parsed only {exts:?} out of MIGRATED_DOC_EXTS — the parser broke"
    );

    let page = read(PAGE);
    for e in &exts {
        assert!(
            page.contains(&format!("`.{e}`")),
            "a link ending `.{e}` gets a `.tmd` suggestion, but {PAGE} does not list it \
             (page must list all of {exts:?})"
        );
    }
}
