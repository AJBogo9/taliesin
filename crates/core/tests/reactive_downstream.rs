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
    let mut start = src
        .find(&head)
        .unwrap_or_else(|| panic!("tali-js.js defines {name}"));
    if src[..start].ends_with("async ") {
        start -= "async ".len();
    }
    let end = src[start..]
        .find("\n  }\n")
        .unwrap_or_else(|| panic!("{name} closes at two-space indent"))
        + start
        + "\n  }\n".len();
    src[start..end].to_string()
}

/// Run `script` in node and return its stdout, or `None` when node is absent (which
/// `TALIESIN_REQUIRE_NODE` turns into a failure).
fn node(script: &str) -> Option<String> {
    let require = std::env::var_os("TALIESIN_REQUIRE_NODE").is_some();
    let have_node =
        matches!(Command::new("node").arg("--version").output(), Ok(o) if o.status.success());
    if !have_node {
        assert!(
            !require,
            "TALIESIN_REQUIRE_NODE=1 but `node` is unavailable: the reactive runtime rules \
             cannot run, and skipping them is how this coverage silently dies"
        );
        eprintln!("skipping reactive_downstream: node unavailable");
        return None;
    }
    let out = Command::new("node")
        .arg("-e")
        .arg(script)
        .output()
        .expect("launch node");
    assert!(
        out.status.success(),
        "node failed running the extracted rule:\n{}",
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
fn a_remounted_producer_re_runs_its_consumers_and_only_them() {
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

    let Some(got) = node(&script) else {
        return;
    };

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
    // `mountPlan` being right is worth nothing if `enhance` does not call it, and no
    // node harness can reach `enhance` (it is DOM-wide). Pin the lines that wire it: the
    // plan is read off the graph BEFORE anything runs, it is seeded with the controls this
    // mount bound and the names a teardown dropped, and the passes are CHAINED (after a
    // define's pass over the cells already mounted), because a stale consumer reads its
    // producer's value out of the shared scope and must not start before that producer's
    // `run()` has resolved.
    let js = include_str!("../assets/js/tali-js.js");
    for (needle, why) in [
        (
            "var plan = mountPlan(r, fresh, bound.concat(r.dropped.splice(0)));",
            "enhance no longer plans its passes from the graph and the changed names",
        ),
        (
            "Promise.resolve(defined)\n      .then(function () { return runSequentially(plan.fresh); })\n      .then(function () { return runSequentially(plan.stale); });",
            "the fresh and stale passes must be chained after the define pass, not raced",
        ),
        (
            "return changed ? runSequentially((r.graph || buildGraph(r)).order) : null;",
            "a define's re-run must go in dependency order and be handed to enhance",
        ),
    ] {
        assert!(js.contains(needle), "{why}: `{needle}` is gone");
    }
}

/// Audit 2026-09-24 D4 and liveops #6. What a mount runs, decided on stand-in cells.
#[test]
fn a_mount_runs_fresh_cells_in_dependency_order_and_seeds_changed_names() {
    let script = format!(
        "{}{}{}{}\n\
         function cell(id, defines, inputs) {{\n\
           return {{ id: id, defines: defines, inputs: inputs, container: null }};\n\
         }}\n\
         function ids(list) {{ return list.map(function (c) {{ return c.id; }}); }}\n\
         // corpus order a reader can write: the sink ABOVE the producer it reads.\n\
         var n = cell('n', 'n', []);\n\
         var sink = cell('sink', null, ['squared']);\n\
         var sq = cell('squared', 'squared', ['n']);\n\
         var r = {{ cells: [n, sink, sq] }};\n\
         var cold = mountPlan(r, r.cells, []);\n\
         var out = {{ coldFresh: ids(cold.fresh), coldStale: ids(cold.stale) }};\n\
         // An {{{{< input >}}}} control re-bound by an edit, and one a teardown dropped:\n\
         // their consumers were not re-mounted, so they are stale.\n\
         var kSink = cell('k-sink', null, ['k']);\n\
         var r2 = {{ cells: [kSink] }};\n\
         out.rebound = ids(mountPlan(r2, [], ['k']).stale);\n\
         out.coldBound = ids(mountPlan(r2, [kSink], ['k']).stale);\n\
         // No edge between them: authoring order, the earliest ready cell first.\n\
         var p1 = cell('p1', 'x', []), s1 = cell('s1', null, ['x']), p2 = cell('p2', 'y', []);\n\
         var r3 = {{ cells: [p1, s1, p2] }};\n\
         out.tieBreak = ids(buildGraph(r3).order);\n\
         console.log(JSON.stringify(out));",
        extract("buildGraph"),
        extract("downstreamInOrder"),
        extract("staleAfterMount"),
        extract("mountPlan"),
    );
    let Some(got) = node(&script) else {
        return;
    };
    assert!(
        got.contains(r#""coldFresh":["n","squared","sink"]"#),
        "a consumer above its producer must run after it on a cold load: {got}"
    );
    assert!(
        got.contains(r#""coldStale":[]"#) && got.contains(r#""coldBound":[]"#),
        "a cold mount schedules nothing on top of its own pass: {got}"
    );
    assert!(
        got.contains(r#""rebound":["k-sink"]"#),
        "a re-bound control's consumers re-run: {got}"
    );
    assert!(
        got.contains(r#""tieBreak":["p1","s1","p2"]"#),
        "authoring order holds where no edge decides: {got}"
    );
}

/// liveops #7. A define landing after a live edit re-ran `r.cells` in MOUNT order, which
/// an edit reshuffles (the re-mounted producer moves to the end), so the sink ran before
/// the producer it reads and kept the old product; the cyclic cells re-ran too.
#[test]
fn a_define_re_runs_the_mounted_cells_in_dependency_order() {
    let script = format!(
        "{}{}{}\n\
         var ran = [];\n\
         function cell(id, defines, inputs) {{\n\
           return {{ id: id, defines: defines, inputs: inputs, container: null,\n\
             run: function () {{ ran.push(id); return Promise.resolve(); }} }};\n\
         }}\n\
         var blob = {{ textContent: '{{\"z\": 7}}', setAttribute: function () {{}} }};\n\
         globalThis.document = {{ querySelectorAll: function () {{ return [blob]; }} }};\n\
         globalThis.window = globalThis;\n\
         // Mount order after the producer `prod` was edited: it re-registered last.\n\
         window.__talijs = {{ scope: {{}}, inputs: {{}}, defines: {{}}, listeners: {{}}, dropped: [],\n\
           cells: [cell('n', 'n', []), cell('sink', null, ['prod']), cell('a', 'a', ['b']),\n\
                   cell('b', 'b', ['a']), cell('prod', 'prod', ['n'])] }};\n\
         function rt() {{ return window.__talijs; }}\n\
         bindDefines().then(function () {{ console.log(JSON.stringify(ran)); }});",
        extract("bindDefines"),
        extract("buildGraph"),
        extract("runSequentially"),
    );
    let Some(got) = node(&script) else {
        return;
    };
    assert_eq!(
        got, r#"["n","prod","sink"]"#,
        "producer before sink, and the cyclic pair left to its diagnostic"
    );
}

/// liveops #6. A `{{< input >}}` control is not a cell, so teardown never unregistered
/// one: a deleted control stayed in `r.inputs`, detached, and its consumers kept reading
/// its last value.
#[test]
fn tearing_down_a_block_unregisters_the_controls_inside_it() {
    let script = format!(
        "{}\n\
         var ctl = {{ getAttribute: function () {{ return 'k'; }} }};\n\
         var other = {{ getAttribute: function () {{ return 'm'; }} }};\n\
         var block = {{ querySelectorAll: function () {{ return [ctl]; }}, contains: function () {{ return false; }} }};\n\
         globalThis.window = globalThis;\n\
         window.__talijs = {{ scope: {{}}, inputs: {{ k: ctl, m: other }}, defines: {{}}, listeners: {{}},\n\
           cells: [], dropped: [] }};\n\
         teardownIn(block);\n\
         console.log(JSON.stringify({{ inputs: Object.keys(window.__talijs.inputs), dropped: window.__talijs.dropped }}));",
        extract("teardownIn"),
    );
    let Some(got) = node(&script) else {
        return;
    };
    assert_eq!(got, r#"{"inputs":["m"],"dropped":["k"]}"#);
}

/// The runtime a real cell mounts into, reduced to what `setupCell` touches: one output
/// container that records what was painted into it, and a `{js}`-shaped language whose run
/// resolves after `ms` with the cell's source as its value.
fn cell_harness() -> String {
    let fns: String = [
        "rt",
        "readValue",
        "registerInput",
        "makeApi",
        "markLiveIfTextual",
        "showCellError",
        "setupCell",
        "runSequentially",
        "buildGraph",
        "downstreamInOrder",
        "scheduleFrom",
    ]
    .iter()
    .map(|f| extract(f))
    .collect();
    format!(
        "{fns}\n\
         globalThis.window = globalThis;\n\
         globalThis.Node = function () {{}};\n\
         var painted = [];\n\
         var box = {{ replaceChildren: function (n) {{ painted.push(n); }},\n\
           getAttribute: function () {{ return null; }}, querySelector: function () {{ return null; }},\n\
           compareDocumentPosition: function () {{ return 0; }} }};\n\
         globalThis.document = {{ getElementById: function () {{ return box; }},\n\
           createElement: function () {{ return {{}}; }} }};\n\
         var languages = {{ slow: function (src) {{ return {{ run: function () {{\n\
           var v = src === 'NODE' ? Object.assign(new Node(), {{ value: 7 }}) : src;\n\
           return new Promise(function (r) {{ setTimeout(function () {{ r(v); }}, 30); }}); }} }}; }} }};\n\
         function script(name, src) {{\n\
           var a = {{ type: 'slow', 'data-target': 't', 'data-name': name }};\n\
           return {{ textContent: src, getAttribute: function (k) {{ return a[k] || null; }},\n\
             setAttribute: function (k, v) {{ a[k] = v; }} }};\n\
         }}\n"
    )
}

/// Audit 2026-09-24 D3. A cell whose block is replaced while its async run is still
/// awaiting must not publish when that run resolves: a slow first save's value landed
/// after the fast second save's, won the shared scope for good (a producer with no inputs
/// never re-runs), and an async `viewof` registered a detached control so the visible
/// slider drove nothing.
#[test]
fn a_disposed_cell_publishes_nothing_when_its_run_resolves() {
    let script = format!(
        "{}\n\
         var cells = [setupCell(script('data', 'V1')), setupCell(script('el', 'NODE'))];\n\
         var pending = cells.map(function (c) {{ return c.run(); }});\n\
         cells.forEach(function (c) {{ c.dispose(); }});\n\
         Promise.all(pending).then(function () {{\n\
           var s = window.__talijs.scope;\n\
           console.log(JSON.stringify({{ scope: Object.keys(s).filter(function (k) {{ return s[k] !== undefined; }}), painted: painted.length }}));\n\
         }});",
        cell_harness()
    );
    let Some(got) = node(&script) else {
        return;
    };
    assert_eq!(
        got, r#"{"scope":[],"painted":0}"#,
        "a disposed cell's late value must reach neither the scope nor the page"
    );
}
