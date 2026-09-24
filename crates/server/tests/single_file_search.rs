//! `build <file.tmd>` searches with the index the preview serves.
//!
//! The single-file page used to ship no index, so its Cmd-K palette built one in the
//! browser from the DOM's `textContent`: raw TeX from KaTeX's MathML annotation, the body of
//! every `<script>`, and each section cut at 1500 characters, all of which the Rust index
//! (the one `preview <file.tmd>` serves) had been fixed not to do. The author checked search
//! in the preview and published different results.

use std::fs;
use std::process::Command;

#[test]
fn a_single_file_build_inlines_the_index_the_preview_serves() {
    let dir = std::env::temp_dir().join(format!("tali-single-search-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("paper.tmd");
    let long = "lorem ipsum dolor sit amet ".repeat(80);
    fs::write(
        &doc,
        format!(
            "---\ntitle: Paper\ntoc: true\n---\n\n## Math part\n\nProse alphaword.\n\n\
             $$\n\\frac{{1}}{{3}}\n$$\n\n## Script part\n\n```{{=html}}\n<div id=\"app\"></div>\n\
             <script>const secretVariableName = 42;</script>\n```\n\nVisible scriptprose.\n\n\
             ## Long part\n\n{long}needlepastcap.\n"
        ),
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", doc.to_str().unwrap(), "--stdout", "--no-exec"])
        .output()
        .expect("run taliesin build");
    let html = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);

    // The index is the page's own script body, so it is read as script text here.
    let start = html
        .find("window.TALIESIN_SEARCH_INDEX=[")
        .expect("the page inlines its search index");
    let index = &html[start..start + html[start..].find("];").expect("index closes") + 1];
    assert!(
        html.contains("window.TALIESIN_PAGE_URL=\"paper.html\""),
        "the page names itself, so a hit scrolls in place"
    );
    assert_eq!(
        index.matches("\"u\":").count(),
        index.matches("\"u\":\"paper.html\"").count(),
        "every entry points into this page: {index}"
    );
    for kept in ["alphaword", "scriptprose", "needlepastcap"] {
        assert!(index.contains(kept), "{kept} is on the page: {index}");
    }
    for gone in ["secretVariableName", "\\\\frac"] {
        assert!(!index.contains(gone), "{gone} is not on the page: {index}");
    }
}
