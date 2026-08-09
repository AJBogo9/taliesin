//! Item 101. Every page Taliesin builds contains verbatim copies of Taliesin's own
//! AGPL-licensed CSS and JavaScript — that is what makes a built page work offline — so
//! without an explicit grant a user could reasonably read every page they publish as an
//! AGPL work. The remedy is the Taliesin Output Exception, an additional permission under
//! AGPL section 7 (the instrument GCC uses so that compiling with GCC does not make your
//! program GPL).
//!
//! A licensing position is worth exactly as much as its discoverability, and it is the kind
//! of prose that rots silently: nothing else in the tree fails when a README paragraph is
//! rewritten. So the position is pinned here in the three places a user actually looks —
//! the licence file, the README, and the guide — plus the premise it rests on.
//!
//! **The first published page fixes the answer in the wild**, which is why this gate exists
//! before publication rather than after.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Read a prose file with runs of whitespace collapsed to single spaces.
///
/// A needle asserted against hard-wrapped Markdown is a pin that fails on a reflow rather
/// than on a meaning change — the most annoying kind of false alarm, and the one that
/// teaches people to delete the test. Every prose assertion here goes through this.
fn read_unwrapped(rel: &str) -> String {
    read(rel).split_whitespace().collect::<Vec<_>>().join(" ")
}

const EXCEPTION: &str = "LICENSE-OUTPUT-EXCEPTION.md";

#[test]
fn the_output_exception_exists_and_grants_what_it_claims() {
    let text = read_unwrapped(EXCEPTION);
    // The legal instrument. Without this line it is a statement of intent, not a grant.
    assert!(
        text.contains("Additional permission under GNU AGPL version 3 section 7"),
        "the exception must invoke AGPL s7, which is what makes it operative"
    );
    // The four promises the guide and README make on its behalf. Each is load-bearing for a
    // different anxiety the finding named: copyleft reach, attribution burden, s13 exposure,
    // and whether the route you used to produce the page matters.
    for needle in [
        "under terms of your choice",
        "No notice is required in your output",
        "does not, by itself, engage AGPL section 13",
        // The verb list narrowed on 2026-08-10: `render` is retired and `publish` was never
        // a command, so the instrument was naming two routes a reader cannot take. This
        // needle is what held the falsehood in place, which is why the document and the
        // needle move together or not at all.
        "`build` or `preview`",
    ] {
        assert!(
            text.contains(needle),
            "the exception must still promise {needle:?} -- the guide says it does"
        );
    }
    // The limit. An exception that swallowed the AGPL would give away the thing the licence
    // was chosen to protect, so the carve-out has to stay carved.
    assert!(
        text.contains("It grants nothing in respect of Taliesin itself"),
        "the exception must NOT extend to conveying Taliesin itself"
    );
    assert!(
        text.contains("section 13 included"),
        "running a modified Taliesin as a service must still trigger s13"
    );
}

#[test]
fn the_readme_states_the_position_and_links_the_exception() {
    let readme = read_unwrapped("README.md");
    assert!(
        readme.contains(EXCEPTION),
        "README's Licence section must link {EXCEPTION}: it is where an evaluator looks first"
    );
    assert!(
        readme.contains("What you build with it is yours"),
        "the README must answer the question in its own words, not only by link"
    );
}

#[test]
fn the_guide_documents_it_for_users() {
    let guide = read_unwrapped("docs/guide/reference/licensing.tmd");
    assert!(
        guide.contains("under any terms you like, with nothing to attribute"),
        "the guide must give the plain-language answer"
    );
    // A chapter absent from the book's spine is a chapter nobody reads.
    assert!(
        read("docs/guide/_site.yml").contains("reference/licensing.tmd"),
        "the licensing chapter must be in the guide's chapter list"
    );
}

#[test]
fn the_vendored_notice_points_at_the_exception() {
    // `assets/js/LICENSES.md` is the file a careful reader lands on when they ask what the
    // scripts in a built page are licensed as. It used to answer only for the *vendored*
    // bundles, which is the gap that left item 101 open after that file was added.
    let notices = read_unwrapped("crates/core/assets/js/LICENSES.md");
    assert!(
        notices.contains("LICENSE-OUTPUT-EXCEPTION.md"),
        "the asset notice must point at the exception, not only at the root LICENSE"
    );
}

#[test]
fn the_exceptions_premise_still_holds() {
    // The exception exists because Taliesin's OWN runtime is copied into every built page.
    // If that ever stopped being true the document would be harmless but misleading, and if
    // it is still true the grant is doing real work. Assert the mechanism directly rather
    // than trusting the prose: `include_str!` of a first-party asset is the thing that makes
    // a user's page contain AGPL material.
    let render = read("crates/core/src/render/mod.rs");
    for asset in ["assets/css/base.css", "assets/js/tali-js.js"] {
        assert!(
            render.contains(&format!("include_str!(\"../../{asset}\")")),
            "{asset} is no longer inlined into built pages -- re-check whether the Output \
             Exception still describes what ships, then update it rather than this test"
        );
    }
}

#[test]
fn no_first_party_asset_smuggles_a_licence_header_into_every_page() {
    // The deliberate flip side of the exception: because these files are copied verbatim
    // into every page, a per-file licence header would add roughly a kilobyte to each page
    // to assert a licence the exception exists to disclaim. The notice lives in
    // `LICENSES.md` instead, where it costs a reader nothing. This is the same reader-cost
    // discipline that moved the body font off the critical path.
    let dir = repo_root().join("crates/core/assets");
    let mut checked = 0;
    for sub in ["css", "js", "js/code-enhance"] {
        let Ok(entries) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Vendored bundles keep their upstream headers; that is a requirement of THEIR
            // licences, not ours.
            if name.ends_with(".min.js") || name == "mermaid.js" || !is_asset(&p) {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            let head: String = text.chars().take(400).collect();
            assert!(
                !head.contains("SPDX-License-Identifier") && !head.contains("GNU Affero"),
                "{name} carries a licence header, which ships in every built page. \
                 Put the notice in assets/js/LICENSES.md instead."
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 10,
        "expected to scan the first-party asset payload, only saw {checked} files -- \
         this test passes vacuously if the walk stops finding them"
    );
}

fn is_asset(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("css") | Some("js")
    )
}
