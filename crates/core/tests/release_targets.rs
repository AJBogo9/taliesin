//! The README's platform matrix and the release workflow's build matrix are one claim
//! written twice, and the two directions of drift fail differently:
//!
//! - a target built but not documented is invisible — the binary is on the release page
//!   and the README still tells a Mac user to install a Rust toolchain;
//! - a target documented but not built is worse, because it is a promise. The reader
//!   downloads nothing and concludes the project is abandoned.
//!
//! So derive the list from the workflow (the thing that actually produces artifacts) and
//! require the README to name exactly it.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// The `target: <triple>` entries of the release workflow's build matrix.
fn released_targets() -> Vec<String> {
    read(".github/workflows/release.yml")
        .lines()
        .filter_map(|l| l.trim().strip_prefix("target: "))
        .map(|t| t.trim().to_string())
        // `targets: ${{ matrix.target }}` on the toolchain step is an expression, not a
        // triple; the matrix entries are literal.
        .filter(|t| !t.contains("${{"))
        .map(|t| t.to_string())
        .collect()
}

#[test]
fn the_readme_platform_matrix_matches_what_the_release_workflow_builds() {
    let targets = released_targets();
    assert!(
        targets.len() >= 2,
        "parsed {targets:?} from the release workflow's matrix — the parser broke, or the \
         release workflow stopped shipping binaries"
    );

    let readme = read("README.md");
    for t in &targets {
        assert!(
            readme.contains(&format!("`{t}`")),
            "the release workflow builds `{t}` but README.md's platform matrix never \
             names it, so nobody knows the binary exists (workflow builds {targets:?})"
        );
    }

    // The other direction: a triple advertised in the README that nothing builds.
    for line in readme.lines() {
        for word in line.split('`') {
            let looks_like_a_triple = word.matches('-').count() >= 2
                && (word.ends_with("-gnu")
                    || word.ends_with("-musl")
                    || word.ends_with("-darwin")
                    || word.ends_with("-msvc"));
            if looks_like_a_triple {
                assert!(
                    targets.contains(&word.to_string()),
                    "README.md advertises the target `{word}` but the release workflow \
                     builds only {targets:?} — that is a promise of a download that will \
                     not be there"
                );
            }
        }
    }
}

/// The release workflow ships the licence beside the binary. Taliesin is AGPL-3.0 and
/// vendors third-party assets into the binary, so a bare executable would be a
/// distribution stripped of the terms it is distributed under.
#[test]
fn the_release_tarball_carries_the_licence_and_third_party_notices() {
    let wf = read(".github/workflows/release.yml");
    assert!(
        wf.contains("cp LICENSE THIRD_PARTY.md"),
        "the release tarball no longer packages LICENSE + THIRD_PARTY.md beside the binary"
    );
    for f in ["LICENSE", "THIRD_PARTY.md"] {
        assert!(
            repo_root().join(f).is_file(),
            "the release workflow copies {f} into every tarball, but {f} does not exist"
        );
    }
}
