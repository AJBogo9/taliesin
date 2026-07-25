//! AP7-2: a `//| input:` sink is a region of the document that rewrites itself when the
//! reader operates a control somewhere else on the page, and it used to do so silently.
//!
//! Driving `corpus/reactive/inputs.tmd`'s slider from the keyboard changed six output
//! regions with every live region on the page empty, and no `.tali-js-out` carried
//! `aria-live` or `role` (7 of 7). This pins the rule that decides which sinks announce,
//! by running the shipped function itself in node against stand-in containers — the whole
//! point being that the rule is *selective*, and a version that marks everything is as
//! wrong as the version that marked nothing.

use std::process::Command;

/// The `markLiveIfTextual` function, sliced out of the shipped bundle so this test can
/// never drift from what ships (the `deck_qr_golden` pattern).
fn extract_rule() -> String {
    let src = include_str!("../assets/js/tali-js.js");
    let start = src
        .find("function markLiveIfTextual(")
        .expect("tali-js.js defines markLiveIfTextual");
    let end = src[start..]
        .find("\n  }\n")
        .expect("markLiveIfTextual closes at two-space indent")
        + start
        + "\n  }\n".len();
    src[start..end].to_string()
}

#[test]
fn only_a_textual_sink_becomes_a_live_region() {
    let require = std::env::var_os("TALIESIN_REQUIRE_NODE").is_some();
    let have_node =
        matches!(Command::new("node").arg("--version").output(), Ok(o) if o.status.success());
    if !have_node {
        assert!(
            !require,
            "TALIESIN_REQUIRE_NODE=1 but `node` is unavailable: the reactive live-region \
             rule cannot run, and skipping it is how this coverage silently dies"
        );
        eprintln!("skipping reactive_live_region: node unavailable");
        return;
    }

    // A stand-in for the output container: only the four DOM calls the rule makes.
    let script = format!(
        "{}\n\
         function el(text, sel, live) {{\n\
           var attrs = live ? {{'aria-live': live}} : {{}};\n\
           return {{\n\
             textContent: text,\n\
             querySelector: function (q) {{ return sel ? {{}} : null; }},\n\
             getAttribute: function (k) {{ return attrs[k] || null; }},\n\
             setAttribute: function (k, v) {{ attrs[k] = v; }},\n\
             attrs: attrs,\n\
           }};\n\
         }}\n\
         var cases = {{\n\
           text: el('k doubled (transitively) = 6', false, null),\n\
           graphic: el('0 5 10 15 20', true, null),\n\
           empty: el('   ', false, null),\n\
           already: el('x', false, 'assertive'),\n\
         }};\n\
         var out = {{}};\n\
         Object.keys(cases).forEach(function (k) {{\n\
           markLiveIfTextual(cases[k]);\n\
           out[k] = cases[k].attrs;\n\
         }});\n\
         markLiveIfTextual(null);\n\
         console.log(JSON.stringify(out));",
        extract_rule()
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
    let got = String::from_utf8(out.stdout)
        .expect("utf-8")
        .trim()
        .to_string();

    // A text sink announces its new content as one unit: "k doubled (transitively) = 16"
    // reads as a sentence, not as a diff.
    assert!(
        got.contains(r#""text":{"aria-live":"polite","aria-atomic":"true"}"#),
        "a textual sink must become a polite atomic live region: {got}"
    );
    // A chart is the common sink and has nothing useful to speak. Measured on built
    // `corpus/descent`: one of its three sink regions has Plot's injected stylesheet as its
    // text content, so marking every sink live would announce raw CSS on every arrow key.
    assert!(
        got.contains(r#""graphic":{}"#),
        "a graphic sink must stay silent: {got}"
    );
    // Nothing to announce, and never clobber an author/enhancer-set politeness.
    assert!(
        got.contains(r#""empty":{}"#),
        "empty sink stays bare: {got}"
    );
    assert!(
        got.contains(r#""already":{"aria-live":"assertive"}"#),
        "an existing aria-live must not be overwritten: {got}"
    );
}

#[test]
fn a_reactive_sink_is_a_distinct_cell_kind_from_a_one_shot_cell() {
    // The rule is applied per `kind`, so pin that the kinds it keys off still exist and are
    // still derived the same way: `viewof` => input, non-empty `//| input:` => sink,
    // otherwise a `once` cell that never re-runs and therefore must never be live.
    let js = include_str!("../assets/js/tali-js.js");
    assert!(
        js.contains(r#"var kind = viewof ? "input" : (inputs.length ? "sink" : "once");"#),
        "the cell-kind derivation moved; re-check which cells markLiveIfTextual is applied to"
    );
    assert!(
        js.contains(r#"if (kind === "sink") { markLiveIfTextual(container); }"#),
        "only a sink may be marked live: a `once` cell painting at load would announce on \
         page load, and an `input` cell is the control the reader is already operating"
    );
}
