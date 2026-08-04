//! The `#| trace: true` execution harness.
//!
//! Runs the author's cell under `sys.settrace` and displays the recorded trace as an
//! `Output::Rich` HTML blob, so it flows back through the ordinary cell-output path.
//! That is the whole reason this needs no change to `freeze.rs`: a trace is just cell
//! output, so the existing cumulative-hash cache stores and replays it.
//!
//! Two things the harness does that `sys.settrace` cannot do on its own:
//!
//! 1. **`reads`.** Line-granularity tracing observes locals, never expression reads. The
//!    harness pre-parses each source line's `Subscript` nodes over plain names and
//!    resolves their indices from the live frame, so `a[j] > a[j+1]` reports `{j, j+1}`.
//!    An index that is not a whitelisted expression is skipped rather than guessed. Only
//!    a `Load`-context Subscript counts: an assignment target is a `Subscript` node too,
//!    and without that check a pure write (`a[j] = tmp`) reported the slot it was about
//!    to overwrite as "read" instead.
//! 2. **`writes`.** Diffed from the previous frame's locals snapshot. The snapshot is a
//!    bounded-depth recursive copy, not a one-level `list(v)`/`dict(v)`: a nested
//!    container (`dp[i][j] = x` on a list of lists) shares its ROW objects across a
//!    one-level copy, so mutating a row in place retroactively "corrupts" the previous
//!    snapshot too (they are the same row object), and a row-level write is silently
//!    never detected. Reproduced by hand before fixing: see the task report.
//!
//! What the harness deliberately does NOT change is how a cell behaves as a *cell*. It
//! runs the author's code against the kernel's own user namespace (`globals()`, since the
//! harness is defined at kernel top level) and re-raises whatever that code raised, so a
//! traced cell sees and mutates the same state every other cell in the document shares,
//! and its failures reach `build --strict` and the freeze cache's never-persist-an-error
//! rule exactly like an untraced cell's do. The first version got both wrong: a private
//! `ns` dict hid every upstream variable behind a silent `NameError`, and an `except` that
//! only *recorded* the exception turned a crashing cell into a cached, warning-free
//! success.

/// The tracer, as Python source. Stdlib only: it runs inside the author's kernel and must
/// not assume anything is installed.
const HARNESS: &str = r#"
def _tali_debug_run(_src):
    import sys, ast, json, io, itertools
    MAX_FRAMES, MAX_ITEMS, MAX_DEPTH, MAX_CHARS = 5000, 200, 4, 2000

    def enc(v, d=0):
        if d > MAX_DEPTH:
            return {"__repr__": type(v).__name__}
        if v is None or isinstance(v, bool) or isinstance(v, int) or isinstance(v, float):
            return v
        if isinstance(v, str):
            return v if len(v) <= MAX_CHARS else v[:MAX_CHARS] + "…"
        if isinstance(v, (list, tuple)):
            head = [enc(x, d + 1) for x in itertools.islice(v, MAX_ITEMS)]
            return head if len(v) <= MAX_ITEMS else {"__trunc__": len(v), "v": head}
        if isinstance(v, dict):
            out = {}
            for k in itertools.islice(v, MAX_ITEMS):
                out[str(k)] = enc(v[k], d + 1)
            return out if len(v) <= MAX_ITEMS else {"__trunc__": len(v), "v": out}
        if isinstance(v, (set, frozenset)):
            return {"__set__": [enc(x, d + 1) for x in itertools.islice(v, MAX_ITEMS)]}
        try:
            return {"__repr__": repr(v)[:MAX_CHARS]}
        except Exception:
            return {"__repr__": "<unrepresentable>"}

    # Per-line subscript reads, precomputed once. Only whitelisted index expressions are
    # ever evaluated: a Name, a literal, +/-/* over those, and unary minus. Anything else
    # is dropped, because a guessed read is worse than no read.
    #
    # `node.ctx` must be `ast.Load`, with one exception: an assignment TARGET is also a
    # `Subscript` node (`a[j] = tmp` parses to `Subscript(ctx=Store)`), and without this
    # check it counted as a "read" of the slot it is about to overwrite. Caught on a
    # temp-variable swap (`tmp = a[j]; a[j] = a[j+1]; a[j+1] = tmp`): the THIRD line has
    # no genuine array read at all (`tmp` is a scalar), yet the unfiltered scan reported
    # `a[1]` as read on the frame BEFORE that line runs, one step ahead of the write it
    # is actually about to become. A tuple-swap one-liner (`a[j], a[j+1] = a[j+1], a[j]`)
    # is unaffected by this filter either way: its RHS values are their own, separate
    # `Subscript(ctx=Load)` nodes at the SAME indices as the targets, so the reads they
    # contribute are real, not a target masquerading as one; that swap genuinely does
    # read both slots before overwriting them.
    #
    # The exception: `a[0] += 5` also parses its target as `Subscript(ctx=Store)`, but
    # unlike a plain assignment it genuinely reads the slot first (`counts[x] += 1` is
    # read-modify-write, not a bare write), so excluding it the same way the plain-Store
    # case is excluded would regress a read that IS real. `AugAssign.target` is collected
    # by node identity so the general ctx check below can special-case exactly that one
    # Subscript per augmented assignment, without loosening the plain-Store exclusion for
    # every other assignment target.
    OK = (ast.Name, ast.Constant, ast.BinOp, ast.UnaryOp, ast.Add, ast.Sub, ast.Mult, ast.USub, ast.Load)
    reads_by_line, src_names = {}, set()
    try:
        tree = ast.parse(_src)
        aug_targets = set(
            id(node.target)
            for node in ast.walk(tree)
            if isinstance(node, ast.AugAssign) and isinstance(node.target, ast.Subscript)
        )
        for node in ast.walk(tree):
            # Every name the cell's own source mentions, for `visible` below. Collected in
            # this walk because it is already paid for.
            if isinstance(node, ast.Name):
                src_names.add(node.id)
            elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                src_names.add(node.name)
            elif isinstance(node, ast.alias):
                src_names.add((node.asname or node.name).split(".")[0])
            if isinstance(node, ast.Subscript) and isinstance(node.value, ast.Name):
                if not (isinstance(node.ctx, ast.Load) or id(node) in aug_targets):
                    continue
                idx = node.slice
                if all(isinstance(n, OK) for n in ast.walk(idx)):
                    reads_by_line.setdefault(node.lineno, []).append((node.value.id, idx))
    except SyntaxError:
        reads_by_line, src_names = {}, set()
    # Sorted, so the variables panel's row order is stable from frame to frame: a set's
    # own iteration order is not.
    src_names = sorted(src_names)

    def reads_for(line, loc):
        out = {}
        for target, idx in reads_by_line.get(line, ()):
            if target not in loc:
                continue
            try:
                # Sandboxed on purpose: `idx` was already restricted to the OK whitelist
                # above (Name/Constant/BinOp/UnaryOp over +, -, * only, no Call and no
                # Attribute), and `__builtins__` is emptied out, so this can only ever
                # evaluate arithmetic over the frame's own bound locals.
                val = eval(compile(ast.Expression(idx), "<idx>", "eval"), {"__builtins__": {}}, dict(loc))
            except Exception:
                continue
            if isinstance(val, int):
                out.setdefault(target, []).append(val)
        return out

    def diff(prev, cur):
        out = {}
        for k, v in cur.items():
            p = prev.get(k, None) if prev else None
            if p is v or p == v:
                continue
            if isinstance(v, list) and isinstance(p, list) and len(v) == len(p):
                w = [i for i in range(len(v)) if v[i] != p[i]]
                if w:
                    out[k] = {"writes": w, "reads": []}
            else:
                out[k] = {"from": enc(p), "to": enc(v)}
        return out

    frames, truncated, prev = [], [False], [None]
    buf = io.StringIO()
    code = compile(_src, "<tali-debug>", "exec")

    # A `list`/`dict` value is snapshotted, not just referenced: `frame.f_locals`
    # hands back the SAME mutable object on every call, so an in-place mutation
    # (`a[i] = ...`) would otherwise retroactively change what last frame's snapshot
    # looks like too, and `diff` above (which relies on comparing this frame's value
    # against the PREVIOUS frame's) would never see a difference: `prev` and `cur`
    # would be `is`-identical, the same list, already mutated.
    #
    # A ONE-LEVEL copy is not enough: a nested container (`dp[i][j] = x` on a DP
    # table, a list of lists) shares its INNER rows across the copy, so mutating a
    # row in place retroactively mutates the "previous" snapshot's row too, since
    # both snapshots hold the same row object. `diff` then compares a row against
    # itself and reports no write, ever, for any nested structure -- reproduced by
    # hand: `p = {"dp": [[0,0],[0,0]]}` shallow-copied, mutate `dp[1][0] = 9`,
    # re-snapshot into `cur`; `p["dp"] == cur["dp"]` is True, because
    # `p["dp"][1] is cur["dp"][1]`. Recursing (bounded by the same MAX_DEPTH `enc`
    # already caps at, so a pathological self-referential structure cannot recurse
    # forever) copies each nested list/dict independently, so a row keeps the value
    # it had at snapshot time regardless of what a later line does to it.
    def snapshot(v, d=0):
        if d > MAX_DEPTH:
            return v
        if isinstance(v, list):
            return [snapshot(x, d + 1) for x in v]
        if isinstance(v, dict):
            return {k: snapshot(x, d + 1) for k, x in v.items()}
        return v

    # Whether a namespace entry is worth showing as algorithm state at MODULE level. A
    # helper function or an `import numpy as np` from an earlier cell is mentioned by the
    # traced source (so `src_names` holds it) but is not state the reader steps through.
    def is_state(v):
        return not callable(v) and not isinstance(v, type(sys))

    # The traced code runs against the kernel's own user namespace (see the `exec` call
    # below), so the MODULE frame's `f_locals` is that whole namespace: every earlier
    # cell's variables, every import, IPython's bookkeeping (`In`, `Out`, `_i1`,
    # `get_ipython`) and this harness itself. Snapshotting all of it per line would be
    # both unreadable and slow, so a module frame reports only the names the traced
    # source itself mentions -- which is exactly the set the reader can see being read
    # and written in the code panel beside it, and which still includes a variable an
    # EARLIER cell bound (`data.sort()` over an upstream `data = [3, 1, 2]`).
    #
    # A nested function frame is never filtered: its locals are real, small, and already
    # only what the author bound. The dunder filter there drops the exec-injected
    # `__builtins__` and friends, which are not code the author wrote.
    def visible(frame):
        raw = frame.f_locals
        if frame.f_code.co_name == "<module>":
            return {k: snapshot(raw[k]) for k in src_names if k in raw and is_state(raw[k])}
        return {
            k: snapshot(v)
            for k, v in raw.items()
            if not (k.startswith("__") and k.endswith("__"))
        }

    def tracer(frame, event, arg):
        if frame.f_code.co_filename != "<tali-debug>":
            return None
        if event not in ("line", "call", "return", "exception"):
            return tracer
        if len(frames) >= MAX_FRAMES:
            truncated[0] = True
            sys.settrace(None)
            return None
        d = 0
        f, stack = frame, []
        while f is not None and f.f_code.co_filename == "<tali-debug>":
            stack.append({"func": f.f_code.co_name, "line": f.f_lineno})
            f = f.f_back
            d += 1
        stack.reverse()
        if event == "call" and d == 1:
            # The outermost frame's own call event, fired before any traced line has
            # run: CPython reports it at line 0, which is not a line the reader can
            # step to. Skip it so the first real line event becomes frame 0. A call
            # into a function DEFINED in the traced code still records (its caller
            # frame also matches "<tali-debug>", so d >= 2 there), which is what lets
            # the stack show entry into it.
            return tracer
        loc = visible(frame)
        changed = diff(prev[0], loc)
        for target, idxs in reads_for(frame.f_lineno, loc).items():
            changed.setdefault(target, {"writes": [], "reads": []})
            changed[target].setdefault("reads", [])
            changed[target]["reads"] = idxs
        frames.append({
            "line": frame.f_lineno, "event": event, "depth": d,
            "func": frame.f_code.co_name,
            "locals": dict((k, enc(v)) for k, v in loc.items()),
            "changed": changed,
            "stack": stack,
            "stdout": buf.getvalue()[-MAX_CHARS:],
        })
        prev[0] = loc
        return tracer

    # `globals()` is the KERNEL's user namespace: this harness is defined and called at
    # the top level of an ordinary execute_request, so a function defined here shares the
    # globals every other cell in the document runs against. Handing that to `exec`
    # (rather than the fresh `{}` this used to build) is what makes a traced cell an
    # ordinary cell with respect to STATE: it sees what an earlier cell bound, and what it
    # binds is there for the next cell. With a private dict, `data.sort()` after an
    # upstream `data = [3, 1, 2]` raised NameError inside the harness and the reader got a
    # widget with three empty frames.
    ns, real_stdout = globals(), sys.stdout
    # Seed the write-diff baseline with the namespace as it stands BEFORE the cell runs,
    # so a variable an earlier cell already bound is not reported as a write on frame 0.
    prev[0] = {k: snapshot(ns[k]) for k in src_names if k in ns and is_state(ns[k])}
    sys.stdout = buf
    sys.settrace(tracer)
    err = None
    try:
        exec(code, ns)
    except BaseException as e:
        # Recorded here, RE-RAISED at the end. Swallowing it (what this used to do) made a
        # traced cell the one cell kind whose failures were invisible: no `tali-error`
        # output, so `build --strict` exited 0 and `_freeze/` cheerfully persisted the
        # broken run, against the documented invariant that an error is never cached. The
        # frame is appended first, and the blob is displayed before the re-raise, so the
        # reader still gets every step recorded up to the failure AND the error itself.
        err = e
        frames.append({"line": getattr(e, "lineno", 0), "event": "exception", "depth": 0,
                       "func": "", "locals": {}, "changed": {}, "stack": [],
                       "stdout": buf.getvalue()[-MAX_CHARS:] + "\n" + repr(e)})
    finally:
        sys.settrace(None)
        sys.stdout = real_stdout

    payload = json.dumps({"frames": frames, "truncated": truncated[0], "cap": MAX_FRAMES})
    # `</script>` inside a JSON string would close the tag the blob rides in. Escaping
    # every `<` is the standard fix and costs nothing: JSON parses < back to `<`.
    payload = payload.replace("<", "\\u003c")
    from IPython.display import display, HTML
    display(HTML('<script type="application/json" class="tali-debug-trace">'
                 + payload + '</script>'))
    if err is not None:
        raise err
"#;

/// Splice author code into the harness. The code is embedded as a JSON string literal:
/// JSON's escape set is a subset of Python's, so `serde_json` gives a safe Python literal
/// for free and there is no triple-quote or backslash hazard.
pub(crate) fn wrap_traced(code: &str) -> String {
    format!(
        "{HARNESS}\n_tali_debug_run({})\n",
        serde_json::to_string(code).expect("string always serializes")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_code_is_embedded_as_a_safe_python_literal() {
        let hostile = "s = '''triple''' + \"\\\\\" + '\\n'";
        let out = wrap_traced(hostile);
        // `hostile` carries one literal backslash before the `n`; a correct JSON
        // string literal escapes that backslash to two, so the needle below expects
        // `'\\n'` (two backslashes), not `'\n'` (one). Verified independently against
        // Python's own `json.dumps` on the identical source text.
        assert!(
            out.contains(r#"_tali_debug_run("s = '''triple''' + \"\\\\\" + '\\n'"#.trim_end()),
            "the literal must be JSON-escaped, not naively quoted:\n{out}"
        );
        assert!(
            !out.contains("_tali_debug_run('''"),
            "no triple-quote splicing"
        );
    }

    #[test]
    fn the_harness_escapes_angle_brackets_so_a_trace_cannot_close_its_own_script_tag() {
        assert!(
            HARNESS.contains(r#"payload.replace("<", "\\u003c")"#),
            "a JSON payload containing </script> would break out of the blob"
        );
    }
}
