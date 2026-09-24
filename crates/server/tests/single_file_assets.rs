//! What `build <file.tmd>` does with the local files a page references when the output is
//! written somewhere else: `build post.tmd elsewhere/p.html` and `build post.tmd --out <dir>`
//! copy each referenced asset to the same relative path beside the page, so the output
//! keeps working away from the source tree.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-sfa-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// `(success, stderr)` of one `taliesin build` run.
fn build(args: &[&std::ffi::OsStr]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .args(args)
        .arg("--no-exec")
        .output()
        .expect("run taliesin");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Two posts built into one folder, each with its own `figures/plot.png`: the second build
/// used to replace the first post's figure with no word, so `a.html` showed post B's plot.
/// And a file of the author's that happened to share a name was overwritten the same way.
#[test]
fn a_single_file_build_never_replaces_a_different_file_beside_its_output() {
    let dir = tmp_dir("two-posts");
    for (post, bytes) in [("a", "A-PLOT"), ("b", "B-PLOT")] {
        fs::create_dir_all(dir.join(post).join("figures")).unwrap();
        fs::write(dir.join(post).join("figures/plot.png"), bytes).unwrap();
        fs::write(
            dir.join(post).join("post.tmd"),
            format!("---\ntitle: {post}\n---\n\n![Post {post}'s plot.](figures/plot.png)\n"),
        )
        .unwrap();
    }
    let share = dir.join("share");
    fs::create_dir_all(&share).unwrap();

    let (ok_a, err_a) = build(&[
        dir.join("a/post.tmd").as_os_str(),
        share.join("a.html").as_os_str(),
    ]);
    assert!(ok_a, "{err_a}");
    assert!(
        err_a.contains("1 asset"),
        "the built line reports what was copied beside the page:\n{err_a}"
    );

    let (ok_b, err_b) = build(&[
        dir.join("b/post.tmd").as_os_str(),
        share.join("b.html").as_os_str(),
        "--strict".as_ref(),
    ]);
    assert_eq!(
        fs::read_to_string(share.join("figures/plot.png")).unwrap(),
        "A-PLOT",
        "post A's figure must survive post B's build"
    );
    assert!(
        err_b.contains("figures/plot.png") && err_b.contains("error"),
        "the refusal is an error that names the file:\n{err_b}"
    );
    assert!(
        err_b.contains("post.tmd:5:"),
        "and it is located at the line that references it:\n{err_b}"
    );
    assert!(!ok_b, "a refused asset is a --strict problem:\n{err_b}");
    assert!(
        share.join("b.html").is_file(),
        "the page itself is still written"
    );

    let _ = fs::remove_dir_all(&dir);
}
