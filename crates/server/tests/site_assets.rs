//! What a site build publishes beside its pages. The mirror copies the project's files
//! wholesale, leaving out `_`/`.`-prefixed names and source-only extensions; a second pass
//! ships any file a page REFERENCES that the mirror left out. Both go through one
//! publication rule (`taliesin_core::includes::publishable`), the one the gate checks and
//! the preview serves by.

use std::fs;
use std::path::Path;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-siteassets-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// `(success, stderr)` for `build <root> --out <out> --no-exec [extra]`.
fn build(root: &Path, out: &Path, extra: &[&str]) -> (bool, String) {
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(root)
        .arg("--out")
        .arg(out)
        .arg("--no-exec")
        .args(extra)
        .output()
        .expect("run build");
    (
        res.status.success(),
        String::from_utf8_lossy(&res.stderr).into_owned(),
    )
}

/// `_images/` and the folder beside an `_includes/` partial are where people keep pictures,
/// and the preview served them and both gates passed, but the deploy had none: the mirror
/// skips `_` folders and the referenced-file pass shipped source types only. A referenced
/// file ships whatever folder it is in, from a nested page and from the navbar `logo:` alike;
/// an unreferenced one in the same folder still stays out.
#[test]
fn a_referenced_file_in_an_underscore_folder_is_deployed() {
    let dir = tmp_dir("underscore");
    let root = dir.join("site");
    fs::create_dir_all(root.join("_images")).unwrap();
    fs::create_dir_all(root.join("posts/p")).unwrap();
    fs::write(
        root.join("_site.yml"),
        "title: S\nurl: https://s.example.com\nlogo: _images/logo.png\n",
    )
    .unwrap();
    for f in [
        "hero.png",
        "deep.png",
        "logo.png",
        "social.png",
        "unused.png",
    ] {
        fs::write(root.join("_images").join(f), f).unwrap();
    }
    fs::write(
        root.join("index.tmd"),
        "---\ntitle: Home\n---\n\n![A hero.](_images/hero.png)\n",
    )
    .unwrap();
    fs::write(
        root.join("posts/p/index.tmd"),
        "---\ntitle: P\n---\n\n![A deep one.](../../_images/deep.png)\n",
    )
    .unwrap();
    // Referenced by its front-matter `image:` alone (the `og:image` a shared link unfurls
    // with), on a page nothing lists.
    fs::write(
        root.join("solo.tmd"),
        "---\ntitle: Solo\nimage: _images/social.png\nimage-alt: A card.\n---\n\nText.\n",
    )
    .unwrap();
    let out = dir.join("out");

    let (ok, err) = build(&root, &out, &["--strict"]);

    assert!(ok, "{err}");
    for f in ["hero.png", "deep.png", "logo.png", "social.png"] {
        assert_eq!(
            fs::read_to_string(out.join("_images").join(f)).unwrap_or_default(),
            f,
            "the referenced `_images/{f}` must be deployed:\n{err}"
        );
    }
    assert!(
        !out.join("_images/unused.png").exists(),
        "a file nothing references stays out of the deploy"
    );
    assert!(
        err.contains("4 assets"),
        "each referenced file counted once:\n{err}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// A draft's text stayed out of a build, but its folder went live: the figure and data
/// beside a `draft: true` post were mirrored wholesale. So were an editor's backups and a
/// merge's leftovers, `index.tmd~`, `#index.tmd#` and `index.tmd.orig`, each a full copy of
/// a page's source under a name the source-extension rule did not recognise. A folder whose
/// only pages are drafts is not mirrored, and residue never is; a file a published page
/// references still ships from either.
#[test]
fn a_site_build_leaves_out_draft_folders_and_editor_residue() {
    let dir = tmp_dir("residue");
    let root = dir.join("site");
    fs::create_dir_all(root.join("posts/draft")).unwrap();
    fs::create_dir_all(root.join("posts/live")).unwrap();
    fs::write(root.join("_site.yml"), "title: S\n").unwrap();
    let page = "---\ntitle: Home\n---\n\n![Shared.](posts/draft/shared.png)\n";
    for (f, body) in [
        ("index.tmd", page),
        ("index.tmd~", page),
        ("#index.tmd#", page),
        ("index.tmd.orig", page),
        ("patch.rej", "x"),
        (
            "posts/live/index.tmd",
            "---\ntitle: Live\n---\n\n![Fig.](fig.png)\n",
        ),
        ("posts/live/fig.png", "live"),
        (
            "posts/draft/index.tmd",
            "---\ntitle: WIP\ndraft: true\n---\n\nWIP.\n",
        ),
        ("posts/draft/wip-figure.png", "wip"),
        ("posts/draft/data.csv", "wip"),
        ("posts/draft/shared.png", "shared"),
    ] {
        fs::write(root.join(f), body).unwrap();
    }
    let out = dir.join("out");

    let (ok, err) = build(&root, &out, &[]);

    assert!(ok, "{err}");
    for residue in [
        "index.tmd~",
        "#index.tmd#",
        "index.tmd.orig",
        "patch.rej",
        "posts/draft/wip-figure.png",
        "posts/draft/data.csv",
    ] {
        assert!(
            !out.join(residue).exists(),
            "`{residue}` must not be published"
        );
    }
    assert!(
        out.join("posts/live/fig.png").is_file(),
        "a live post's folder ships"
    );
    assert!(
        out.join("posts/draft/shared.png").is_file(),
        "a file a published page references ships from a draft's folder too"
    );
    let _ = fs::remove_dir_all(&dir);
}
