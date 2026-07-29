//! `site/build.sh` is the one command that produces a *complete* deploy of the marketing
//! site: the parent project plus every project it `mounts:`. A mount added to `_site.yml`
//! and not to the script silently reintroduces item 149 — a tree whose links 404 — and the
//! only warning would be one the deploy pipeline never reads.
//!
//! So the two are pinned against each other in both directions. This is the same shape as
//! `release_targets.rs` pinning the README's platform matrix against the release workflow:
//! a list duplicated for a good reason gets a test, not a comment asking people to be careful.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// The `at:` prefixes declared under `mounts:` in `site/_site.yml`.
fn config_mounts() -> Vec<String> {
    let src = std::fs::read_to_string(repo_root().join("site/_site.yml")).expect("read _site.yml");
    let mut out = Vec::new();
    let mut in_mounts = false;
    for line in src.lines() {
        if line.starts_with("mounts:") {
            in_mounts = true;
            continue;
        }
        if in_mounts {
            // The block ends at the next top-level key (a non-indented, non-comment line).
            if !line.starts_with(char::is_whitespace) && !line.trim().is_empty() {
                break;
            }
            if let Some((at, _path)) = line.trim().split_once(':')
                && !at.starts_with('#')
                && !at.is_empty()
            {
                out.push(at.trim().to_string());
            }
        }
    }
    out
}

/// The `at:` prefixes the build script iterates over.
fn script_mounts() -> Vec<String> {
    let src = std::fs::read_to_string(repo_root().join("site/build.sh")).expect("read build.sh");
    let body = src
        .split_once("mounts=(")
        .expect("build.sh declares a mounts array")
        .1
        .split_once(')')
        .expect("…that is closed")
        .0;
    body.lines()
        .filter_map(|l| {
            let l = l.trim().trim_matches('"');
            let at = l.split_whitespace().next()?;
            (!at.is_empty() && !at.starts_with('#')).then(|| at.to_string())
        })
        .collect()
}

#[test]
fn mounts_match_the_site_config() {
    let config = config_mounts();
    let script = script_mounts();
    assert!(
        !config.is_empty(),
        "the marketing site declares mounts; if that changed, this test needs rescoping"
    );
    // Sorted comparison: the script's order is load-bearing for the BUILD (parent first),
    // but not among the mounts themselves, so order is not what is pinned here.
    let (mut a, mut b) = (config.clone(), script.clone());
    a.sort();
    b.sort();
    assert_eq!(
        a, b,
        "site/build.sh and site/_site.yml disagree about mounts.\n\
         _site.yml: {config:?}\nbuild.sh:  {script:?}\n\
         A mount missing from the script builds a deploy whose links 404 (item 149)."
    );
}

#[test]
fn the_script_builds_the_parent_before_the_mounts() {
    // Not a style preference: the parent build's `sweep_stale` deletes everything under the
    // output directory it did not write, and `_site/docs/guide/` is not dot-, underscore- or
    // symlink-exempt. Mounts built first are silently swept away by the parent.
    let src = std::fs::read_to_string(repo_root().join("site/build.sh")).expect("read build.sh");
    let parent = src
        .find(r#""$tali" build "$here" --out "$out""#)
        .expect("builds the site itself");
    let loop_start = src.find("for entry in").expect("loops over the mounts");
    assert!(
        parent < loop_start,
        "the parent build must precede the mount loop, or the sweep deletes the mounts"
    );
}

#[test]
fn the_script_is_executable() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(repo_root().join("site/build.sh"))
            .expect("stat build.sh")
            .permissions()
            .mode();
        assert!(
            mode & 0o111 != 0,
            "site/build.sh is documented as `./site/build.sh`, so it must be executable (mode {mode:o})"
        );
    }
}
