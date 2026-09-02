//! Editing a `//| name:` producer used to leave its consumers painting the OLD code's value.
//!
//! A save re-mounts only the BLOCK that changed, so `enhance` saw exactly one fresh cell and
//! ran exactly that cell. Changing `corpus/reactive/graph.tmd`'s `squared` producer from
//! `** 2` to `** 3` therefore republished `squared` into the shared scope while the sink
//! paragraph below it went on displaying the square, until the reader happened to move the
//! slider and `scheduleFrom` swept the sink up as a side effect. The live preview, whose
//! whole promise is a correct block-level incremental update, was displaying a page that
//! contradicted its own source.
//!
//! `staleAfterMount` is the decision that closes that hole, and it is a decision the fix can
//! get wrong in BOTH directions: too little and the consumer stays stale, too much and every
//! cold page load re-runs its whole graph on top of the initial pass. So this pins the rule
//! by running the shipped function itself in node against stand-in cells (the
//! `reactive_live_region` pattern), never a copy that can drift from what ships.

use std::process::Command;

/// A named function, sliced out of the shipped bundle at its closing two-space-indent brace.
fn extract(name: &str) -> String {
    let src = include_str!("../assets/js/tali-js.js");
    let head = format!("function {name}(");
    let start = src
        .find(&head)
        .unwrap_or_else(|| panic!("tali-js.js defines {name}"));
    let end = src[start..]
        .find("\n  }\n")
        .unwrap_or_else(|| panic!("{name} closes at two-space indent"))
        + start
        + "\n  }\n".len();
    src[start..end].to_string()
}

#[test]
fn a_remounted_producer_re_runs_its_consumers_and_only_them() {
    let require = std::env::var_os("TALIESIN_REQUIRE_NODE").is_some();
    let have_node =
        matches!(Command::new("node").arg("--version").output(), Ok(o) if o.status.success());
    if !have_node {
        assert!(
            !require,
            "TALIESIN_REQUIRE_NODE=1 but `node` is unavailable: the downstream-staleness rule \
             cannot run, and skipping it is how this coverage silently dies"
        );
        eprintln!("skipping reactive_downstream: node unavailable");
        return;
    }

    // Stand-in cells: `buildGraph` reads `inputs`, `defines` and `container` (null, so the
    // cycle diagnostic never reaches `document`), and the two downstream passes only ever
    // compare cells by identity.
    let script = format!(
        "{}{}{}\n\
         function cell(id, defines, inputs) {{\n\
           return {{ id: id, defines: defines, inputs: inputs, container: null }};\n\
         }}\n\
         function ids(list) {{ return list.map(function (c) {{ return c.id; }}); }}\n\
         // corpus/reactive/graph.tmd, in document order: a slider, a derived value that\n\
         // consumes it, a sink that consumes the derived value, and an independent pair.\n\
         var n = cell('n', 'n', []);\n\
         var sq = cell('squared', 'squared', ['n']);\n\
         var sink = cell('sink', null, ['squared']);\n\
         var m = cell('m', 'm', []);\n\
         var mSink = cell('m-sink', null, ['m']);\n\
         var r = {{ cells: [n, sq, sink, m, mSink] }};\n\
         buildGraph(r);\n\
         var out = {{\n\
           cold: ids(staleAfterMount(r, r.cells)),\n\
           producerEdited: ids(staleAfterMount(r, [sq])),\n\
           inputEdited: ids(staleAfterMount(r, [n])),\n\
           consumerEdited: ids(staleAfterMount(r, [sink])),\n\
           bothEdited: ids(staleAfterMount(r, [sq, sink])),\n\
         }};\n\
         var a = cell('a', 'a', []);\n\
         var b = cell('b', 'b', []);\n\
         var both = cell('both', null, ['a', 'b']);\n\
         var r2 = {{ cells: [a, b, both] }};\n\
         buildGraph(r2);\n\
         out.diamond = ids(staleAfterMount(r2, [a, b]));\n\
         var x = cell('x', 'x', ['y']);\n\
         var y = cell('y', 'y', ['x']);\n\
         var r3 = {{ cells: [x, y] }};\n\
         buildGraph(r3);\n\
         out.cycleDetected = ids(r3.graph.cyclic);\n\
         out.cycleScheduled = ids(staleAfterMount(r3, [x]));\n\
         console.log(JSON.stringify(out));",
        extract("buildGraph"),
        extract("downstreamInOrder"),
        extract("staleAfterMount"),
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

    // THE BUG. Editing the `//| name: squared` producer's body re-mounts that block alone;
    // the sink consuming `squared` is not fresh, so nothing else re-ran it.
    assert!(
        got.contains(r#""producerEdited":["sink"]"#),
        "a re-mounted producer must re-run its consumers: {got}"
    );
    // Transitively, and in DEPENDENCY order: re-mounting the `//| viewof: n` slider must
    // re-run `squared` before the sink that reads it out of the shared scope, or the sink
    // paints against the value the previous slider published.
    assert!(
        got.contains(r#""inputEdited":["squared","sink"]"#),
        "the closure must be transitive and topologically ordered: {got}"
    );
    // The other direction, which is the half a naive fix gets wrong. On a cold load EVERY
    // cell is fresh, so the seeds' whole closure IS the fresh set: re-running it would
    // repaint every chart and rebuild every `import()`ed renderer on first paint.
    assert!(
        got.contains(r#""cold":[]"#),
        "a cold mount must schedule nothing on top of its own initial pass: {got}"
    );
    // A cell that publishes no name cannot have left anything stale, so editing a sink is
    // one run, not two.
    assert!(
        got.contains(r#""consumerEdited":[]"#),
        "a cell with no `defines` seeds nothing: {got}"
    );
    assert!(
        got.contains(r#""bothEdited":[]"#),
        "a consumer that was itself re-mounted has already run: {got}"
    );
    // Once, not once per producer feeding it: this is why it is one merged pass over all the
    // seeds rather than a `scheduleFrom` call each.
    assert!(
        got.contains(r#""diamond":["both"]"#),
        "a consumer fed by two edited producers runs exactly once: {got}"
    );
    // A cycle is diagnosed and then excluded from scheduling; scheduling it would be the
    // unguarded recursion the cycle check exists to prevent.
    assert!(
        got.contains(r#""cycleDetected":["x","y"]"#) && got.contains(r#""cycleScheduled":[]"#),
        "cyclic cells stay diagnosed and unscheduled: {got}"
    );
}

#[test]
fn the_mount_actually_runs_the_stale_pass_after_the_fresh_one() {
    // `staleAfterMount` being right is worth nothing if `enhance` does not call it, and no
    // node harness can reach `enhance` (it is DOM-wide). Pin the two lines that wire it: the
    // stale set is read off the graph BEFORE anything runs, and the two passes are CHAINED,
    // because a stale consumer reads its producer's value out of the shared scope and must
    // not start before that producer's `run()` has resolved.
    let js = include_str!("../assets/js/tali-js.js");
    assert!(
        js.contains("var stale = staleAfterMount(r, runnable);"),
        "enhance no longer computes the stale set; a producer edit re-runs nothing downstream"
    );
    assert!(
        js.contains(
            "runSequentially(runnable).then(function () { return runSequentially(stale); });"
        ),
        "the stale pass must be chained after the fresh one, not raced against it"
    );
}
