//! A Cmd-K hit inside a closed `<details>` (a collapsed callout, a folded appendix) is
//! revealed, not flashed invisibly.
//!
//! The palette flashed the first occurrence of the term even when a closed `<details>` hid
//! it, although a visible copy followed in the next paragraph, and a same-page Enter on a
//! heading inside one left the page where it was (audit 2026-09-24, search #8). The rule
//! that decides what a closed `<details>` hides is `closedAround` in `search.js`; this runs
//! the shipped function in node against stand-in nodes (only `tagName`, `open` and
//! `parentElement` are read).

use std::process::Command;

/// `closedAround` and `reveal`, sliced out of the shipped palette.
fn rule() -> String {
    let src = include_str!("../../../web-client/search.js");
    let start = src
        .find("  function closedAround(")
        .expect("search.js defines closedAround");
    let end = src[start..]
        .find("  // The first substring occurrence")
        .expect("reveal() is followed by firstTermRange's comment")
        + start;
    src[start..end].to_string()
}

#[test]
fn only_the_body_of_a_closed_details_is_hidden() {
    let require = std::env::var_os("TALIESIN_REQUIRE_NODE").is_some();
    let have_node =
        matches!(Command::new("node").arg("--version").output(), Ok(o) if o.status.success());
    if !have_node {
        assert!(
            !require,
            "TALIESIN_REQUIRE_NODE=1 but `node` is unavailable: the reveal rule cannot run"
        );
        eprintln!("skipping search_reveal: node unavailable");
        return;
    }
    let script = format!(
        "{}\n\
         function el(tag, parent, open) {{ return {{ tagName: tag, open: !!open, parentElement: parent || null }}; }}\n\
         function text(parent) {{ return {{ parentElement: parent }}; }}\n\
         var callout = el('DETAILS', el('DIV', null));\n\
         var title = text(el('SUMMARY', callout));\n\
         var body = text(el('P', el('DIV', callout)));\n\
         var shown = text(el('P', el('DETAILS', null, true)));\n\
         var outer = el('DETAILS', null);\n\
         var inner = el('DETAILS', el('DIV', outer));\n\
         var nested = text(el('P', inner));\n\
         var heading = el('H3', inner);\n\
         var out = {{\n\
           title: closedAround(title).length, body: closedAround(body).length,\n\
           shown: closedAround(shown).length, nested: closedAround(nested).length,\n\
           heading: closedAround(heading).length,\n\
         }};\n\
         reveal(heading);\n\
         out.revealed = [outer.open, inner.open, callout.open];\n\
         console.log(JSON.stringify(out));",
        rule()
    );
    let out = Command::new("node")
        .arg("-e")
        .arg(&script)
        .output()
        .expect("launch node");
    assert!(
        out.status.success(),
        "node failed running the extracted rule:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got = String::from_utf8(out.stdout).expect("utf-8");
    // A summary shows while its details is closed; its body does not; an open details hides
    // nothing; nested closed ones both hide; revealing opens exactly the ones around it.
    assert_eq!(
        got.trim(),
        r#"{"title":0,"body":1,"shown":0,"nested":2,"heading":2,"revealed":[true,true,false]}"#
    );
}
