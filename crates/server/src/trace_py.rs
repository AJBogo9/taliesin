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
//!    An index that is not a whitelisted expression is skipped rather than guessed.
//! 2. **`writes`.** Diffed from the previous frame's locals snapshot.

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
    OK = (ast.Name, ast.Constant, ast.BinOp, ast.UnaryOp, ast.Add, ast.Sub, ast.Mult, ast.USub, ast.Load)
    reads_by_line = {}
    try:
        for node in ast.walk(ast.parse(_src)):
            if isinstance(node, ast.Subscript) and isinstance(node.value, ast.Name):
                idx = node.slice
                if all(isinstance(n, OK) for n in ast.walk(idx)):
                    reads_by_line.setdefault(node.lineno, []).append((node.value.id, idx))
    except SyntaxError:
        reads_by_line = {}

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

    def tracer(frame, event, arg):
        if frame.f_code.co_filename != "<tali-debug>":
            return None
        if event not in ("line", "call", "return", "exception"):
            return tracer
        if len(frames) >= MAX_FRAMES:
            truncated[0] = True
            sys.settrace(None)
            return None
        # `ns` is passed as BOTH globals and locals to `exec()` below, so the traced
        # module frame's own locals are the same dict `exec` auto-populates with
        # `__builtins__` (and friends). Those dunder keys are not code the author
        # wrote; drop them so a frame's `locals` shows only what the author bound.
        #
        # A `list`/`dict` value is snapshotted (a shallow copy), not just referenced:
        # `frame.f_locals` hands back the SAME mutable object on every call, so an
        # in-place mutation (`a[i] = ...`) would otherwise retroactively change what
        # last frame's snapshot looks like too, and `diff` below (which relies on
        # comparing this frame's value against the PREVIOUS frame's) would never see
        # a difference: `prev` and `cur` would be `is`-identical, the same list,
        # already mutated. A shallow copy is enough for the writes/reads this diffs
        # (index assignment into a flat container); a container nested inside
        # another can still alias the same way, same limitation `enc`'s depth cap
        # already accepts.
        def snapshot(v):
            if isinstance(v, list):
                return list(v)
            if isinstance(v, dict):
                return dict(v)
            return v

        loc = {
            k: snapshot(v)
            for k, v in frame.f_locals.items()
            if not (k.startswith("__") and k.endswith("__"))
        }
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
            # step to, and its locals are only the exec-injected dunders just
            # filtered above. Skip it so the first real line event becomes frame 0.
            # A call into a function DEFINED in the traced code still records (its
            # caller frame also matches "<tali-debug>", so d >= 2 there), which is
            # what lets the stack show entry into it.
            return tracer
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

    ns, real_stdout = {}, sys.stdout
    sys.stdout = buf
    sys.settrace(tracer)
    try:
        exec(code, ns, ns)
    except Exception as e:
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
