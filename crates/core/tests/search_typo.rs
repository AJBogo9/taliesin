//! The Cmd-K typo tier forgives one edit in a word the reader can see, whatever punctuation
//! touches that word on the page.
//!
//! It split a section's text on whitespace alone, so `analysis.` and `gödel’s` were each one
//! "word", two or more edits from a one-edit typo of the word inside it: `anaylsis` found
//! nothing on a page reading "wrote the analysis.", while `analysis` did. Measured on the
//! Guide's index, an adjacent-letter swap found 76.6% of the sections containing the word
//! (audit 2026-09-24, search #7). This runs the shipped matcher itself, sliced out of
//! `search.js`, in node.

use std::process::Command;

/// `search.js` from the typo tier through `score()`, verbatim.
fn matcher() -> String {
    let src = include_str!("../../../web-client/search.js");
    let start = src
        .find("  function within1(")
        .expect("search.js defines within1");
    let end = src[start..]
        .find("  /** One rendered row.")
        .expect("score() is followed by the Row typedef")
        + start;
    src[start..end].to_string()
}

fn node(script: &str) -> Option<String> {
    let require = std::env::var_os("TALIESIN_REQUIRE_NODE").is_some();
    let have_node =
        matches!(Command::new("node").arg("--version").output(), Ok(o) if o.status.success());
    if !have_node {
        assert!(
            !require,
            "TALIESIN_REQUIRE_NODE=1 but `node` is unavailable: the search typo tier cannot run"
        );
        eprintln!("skipping search_typo: node unavailable");
        return None;
    }
    let out = Command::new("node")
        .arg("-e")
        .arg(script)
        .output()
        .expect("launch node");
    assert!(
        out.status.success(),
        "node failed running the extracted matcher:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(
        String::from_utf8(out.stdout)
            .expect("utf-8")
            .trim()
            .to_string(),
    )
}

#[test]
fn a_typo_finds_a_word_that_touches_punctuation() {
    // Each case: the typed term, and a section body that shows the word it misspells.
    let script = format!(
        "{}\n\
         function item(t, b) {{ return {{ tLow: t.toLowerCase(), bLow: b.toLowerCase() }}; }}\n\
         var cases = [\n\
           ['anaylsis', 'Then we wrote the analysis.'],\n\
           ['argmuents', 'A call with no arguments) at all.'],\n\
           ['alognside', 'It runs alongside, quietly.'],\n\
           ['gödle', 'By Gödel\u{2019}s theorem.'],\n\
           ['linpsace', 'bins = np.linspace(0, 1)'],\n\
           ['zebar', 'no such word here'],\n\
         ];\n\
         var out = {{}};\n\
         cases.forEach(function (c) {{ out[c[0]] = score(item('Title', c[1]), [c[0]], false).s > 0; }});\n\
         console.log(JSON.stringify(out));",
        matcher()
    );
    let Some(got) = node(&script) else { return };
    assert_eq!(
        got,
        r#"{"anaylsis":true,"argmuents":true,"alognside":true,"gödle":true,"linpsace":true,"zebar":false}"#,
        "one edit from a word on the page is a match, punctuation or not"
    );
}
