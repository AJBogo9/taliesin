//! A `{{< embed >}}` in a SINGLE-doc build ships a dead iframe: only a *site*
//! build also builds the embedded target beside the page. The single-doc build
//! must warn rather than fail silently.

use std::fs;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    // NB: keep "embed" out of the dir name — it lands in the printed output path and
    // would pollute the `stderr.contains("embed")` assertions below.
    let dir = std::env::temp_dir().join(format!("qmd-singledoc-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn single_doc_build_warns_on_unresolved_embed() {
    let dir = tmp_dir("warn");
    let doc = dir.join("post.qmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\n{{< embed talk.qmd >}}\n").unwrap();
    fs::write(
        dir.join("talk.qmd"),
        "---\ntitle: Talk\nformat: revealjs\n---\n\n## Slide one\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_qmd-fast"))
        .arg("build")
        .arg(&doc)
        .arg(dir.join("post.html"))
        .output()
        .expect("run build");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        stderr.contains("embed") && stderr.contains("talk.qmd"),
        "expected an unresolved-embed warning for a single-doc build, stderr was:\n{stderr}"
    );
}

#[test]
fn single_doc_build_without_embed_does_not_warn_about_embeds() {
    let dir = tmp_dir("clean");
    let doc = dir.join("post.qmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\nJust prose, no embed.\n").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_qmd-fast"))
        .arg("build")
        .arg(&doc)
        .arg(dir.join("post.html"))
        .output()
        .expect("run build");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !stderr.contains("embed"),
        "a doc with no embed must not warn about embeds, stderr was:\n{stderr}"
    );
}
