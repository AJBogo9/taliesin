//! A `_site` build mirrors the source tree, so a page or asset removed/renamed in the
//! source must not linger in the output across rebuilds. The build sweeps the stale
//! files it no longer produces — but leaves the dot/underscore deploy metadata it never
//! emits (`.nojekyll`, `_headers`, a `.git` worktree), which the author placed on purpose.
//!
//! Because that sweep deletes, it may only run somewhere the build owns. The other half
//! of this file pins the two refusals that bound it: an output directory holding files
//! this build did not produce, and one that *contains* the source it is building.

use std::fs;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-sweep-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// `(exited_zero, stderr)` for one `build <dir> --out <out>`.
fn try_build(dir: &std::path::Path, out: &std::path::Path) -> (bool, String) {
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(dir)
        .arg("--out")
        .arg(out)
        .output()
        .expect("run build");
    (
        res.status.success(),
        String::from_utf8_lossy(&res.stderr).into_owned(),
    )
}

fn build(dir: &std::path::Path, out: &std::path::Path) {
    let (ok, err) = try_build(dir, out);
    assert!(ok, "build failed: {err}");
}

/// A minimal one-page project at `root`.
fn project(root: &std::path::Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("_site.yml"), "title: Probe\n").unwrap();
    fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
}

#[test]
fn rebuild_sweeps_removed_page_and_asset_but_keeps_deploy_metadata() {
    let dir = tmp_dir("removed");
    let out = dir.join("out");
    fs::write(dir.join("_site.yml"), "title: Sweep probe\n").unwrap();
    fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
    fs::create_dir_all(dir.join("posts")).unwrap();
    fs::write(
        dir.join("posts").join("old.tmd"),
        "---\ntitle: Old\n---\n\nGoing away.\n",
    )
    .unwrap();
    fs::write(dir.join("logo.png"), b"not-a-real-png").unwrap();

    build(&dir, &out);
    assert!(
        out.join("posts").join("old.html").exists(),
        "first build writes the page"
    );
    assert!(
        out.join("logo.png").exists(),
        "first build mirrors the asset"
    );
    assert!(out.join("index.html").exists());

    // Deploy metadata the build never emits (dot/underscore) must survive a rebuild.
    fs::write(out.join(".nojekyll"), "").unwrap();
    fs::write(out.join("_headers"), "/*\n  X-Frame-Options: DENY\n").unwrap();

    // Remove a page + an asset from the source, then rebuild.
    fs::remove_file(dir.join("posts").join("old.tmd")).unwrap();
    fs::remove_file(dir.join("logo.png")).unwrap();
    build(&dir, &out);

    assert!(
        !out.join("posts").join("old.html").exists(),
        "a removed page must be swept from the output"
    );
    assert!(
        !out.join("logo.png").exists(),
        "a removed asset must be swept from the output"
    );
    assert!(
        !out.join("posts").exists(),
        "an emptied output directory is pruned"
    );
    assert!(
        out.join("index.html").exists(),
        "a surviving page is still built"
    );
    assert!(
        out.join(".nojekyll").exists(),
        ".nojekyll (deploy metadata) must survive the sweep"
    );
    assert!(
        out.join("_headers").exists(),
        "_headers (deploy metadata) must survive the sweep"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn build_refuses_an_output_directory_holding_files_it_did_not_produce() {
    // The ordinary GitHub Pages shape: `public/` already holds a CNAME, an unrelated
    // file and a media folder. The sweep would delete all three and exit 0.
    let dir = tmp_dir("foreign");
    let src = dir.join("blog");
    let out = dir.join("public");
    project(&src);
    fs::create_dir_all(out.join("photos")).unwrap();
    fs::write(out.join("CNAME"), "example.com\n").unwrap();
    fs::write(out.join("thesis.txt"), "thesis\n").unwrap();
    fs::write(out.join("photos").join("cat.jpg"), b"jpeg").unwrap();

    let (ok, err) = try_build(&src, &out);

    assert!(
        !ok,
        "must not build into a directory it does not own: {err}"
    );
    assert!(
        err.contains("CNAME"),
        "the refusal must name what it found: {err}"
    );
    for kept in ["CNAME", "thesis.txt", "photos/cat.jpg"] {
        assert!(
            out.join(kept).is_file(),
            "{kept} must survive a refused build: {err}"
        );
    }
    assert!(
        !out.join("index.html").exists(),
        "a refused build writes nothing: {err}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn build_refuses_an_output_directory_that_contains_the_source() {
    // `taliesin build myblog --out .` from the parent, the natural deploy-to-repo-root
    // spelling: the sweep walks down into `myblog/` and deletes the sources themselves.
    let dir = tmp_dir("ancestor");
    let src = dir.join("myblog");
    project(&src);
    fs::write(src.join("post.tmd"), "---\ntitle: P\n---\n\nPost.\n").unwrap();
    fs::write(src.join("README.md"), "# readme\n").unwrap();

    let (ok, err) = try_build(&src, &dir);

    assert!(!ok, "must not build into an ancestor of the source: {err}");
    assert!(
        err.contains("contains the source directory"),
        "the refusal must name the containment, not just the foreign files: {err}"
    );
    for kept in ["_site.yml", "index.tmd", "post.tmd", "README.md"] {
        assert!(
            src.join(kept).is_file(),
            "{kept} must survive a refused build: {err}"
        );
    }
    assert!(
        !dir.join("index.html").exists(),
        "a refused build writes nothing: {err}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_output_directory_from_an_earlier_binary_is_still_recognised() {
    // The refusal is bookkeeping this build only started keeping on 2026-08-13. Every
    // `_site/` and every live deploy folder written before that carries no marker, and
    // refusing them all once would be a worse bug than the one being fixed. The asset
    // bundle is the standing evidence: nothing but a Taliesin build writes
    // `_assets/app.<hash>.css`.
    let dir = tmp_dir("legacy");
    let src = dir.join("blog");
    let out = dir.join("public");
    project(&src);
    fs::write(src.join("gone.tmd"), "---\ntitle: Gone\n---\n\nGoing.\n").unwrap();

    build(&src, &out);
    fs::remove_file(out.join(".taliesin-build")).expect("the build claims its output");
    fs::remove_file(src.join("gone.tmd")).unwrap();

    let (ok, err) = try_build(&src, &out);

    assert!(ok, "an unmarked prior output must still build: {err}");
    assert!(
        !out.join("gone.html").exists(),
        "and must still sweep what it no longer produces: {err}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_foreign_app_stylesheet_does_not_claim_the_output_directory() {
    // The fallback above recognises `_assets/app.<hash>.css` because nothing but a
    // Taliesin build writes that name, but only the *hashed* shape is exclusive.
    // `app.min.css` is one of the most conventional stylesheet names there is, and a
    // webpack/parcel `app.<contenthash>.css` is another; matching those handed the sweep
    // a stranger's directory and deleted their files with an exit-0 build.
    let dir = tmp_dir("foreign-bundle");
    let src = dir.join("blog");
    let out = dir.join("dist");
    project(&src);
    fs::create_dir_all(out.join("_assets")).unwrap();
    fs::write(out.join("_assets").join("app.min.css"), "body{}").unwrap();
    fs::write(out.join("precious.txt"), "hi\n").unwrap();

    let (ok, err) = try_build(&src, &out);

    assert!(
        !ok,
        "a foreign `_assets/app.min.css` must not claim the directory: {err}"
    );
    assert!(
        out.join("precious.txt").is_file(),
        "a refused build must touch nothing: {err}"
    );
    assert!(
        out.join("_assets").join("app.min.css").is_file(),
        "including the file it mistook for its own: {err}"
    );
    assert!(
        !out.join("index.html").exists(),
        "and writes nothing: {err}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_deploy_metadata_only_output_directory_is_still_usable() {
    // The boundary the refusal turns on: `.nojekyll`, `_headers` and a `.git` worktree
    // are exactly what the sweep already promises never to touch, so a deploy folder
    // holding only those is not a stranger's directory and must still build.
    let dir = tmp_dir("metadata-only");
    let src = dir.join("blog");
    let out = dir.join("public");
    project(&src);
    fs::create_dir_all(out.join(".git")).unwrap();
    fs::write(out.join(".git").join("HEAD"), "ref: refs/heads/gh-pages\n").unwrap();
    fs::write(out.join(".nojekyll"), "").unwrap();
    fs::write(out.join("_headers"), "/*\n  X-Frame-Options: DENY\n").unwrap();

    let (ok, err) = try_build(&src, &out);

    assert!(ok, "a deploy-metadata-only directory must build: {err}");
    assert!(out.join("index.html").is_file(), "the page is written");
    assert!(out.join(".git").join("HEAD").is_file(), ".git survives");
    assert!(out.join(".nojekyll").is_file(), ".nojekyll survives");
    assert!(out.join("_headers").is_file(), "_headers survives");

    let _ = fs::remove_dir_all(&dir);
}
