// Algorithm debug mode: step a recorded execution trace.
//
// The server emits `::: {.debug}` as a `.tali-debug` container holding a line-wrapped
// `.dbg-code` panel, an optional hidden `.tali-debug-input[data-tali-input]` bridge, and a
// `.dbg-views` column. The trace itself arrives as a `<script type="application/json"
// class="tali-debug-trace">` blob, either emitted by the Python harness at build time or
// produced here by draining an author generator (see the JS adapter).
//
// That trace blob is NOT a descendant of `.tali-debug`: `divs.rs` folds the whole
// `::: {.debug}` fence into one composite block (and its own HTML string) before the
// traced cell has run, so the executor can only splice the cell's output back as a
// SIBLING block immediately after the container closes, never back inside the string it
// already serialized (confirmed by inspecting a real build: `</div><div
// class="tali-output" ...><script class="tali-debug-trace">`). `traceEl` below looks
// there first, and `refresh` re-reads it when a live-preview re-run replaces that sibling
// under a widget that was never itself swapped.
//
// Stepping publishes the frame index into the hidden input, which is the SAME bridge
// scrolly uses, so a `{js}` view cell re-runs through `//| input:` with no new reactive
// machinery. Read-only: nothing here ever writes source.
//
// This file owns the transport chrome, the line cursor, the variables panel, and
// `renderStage`: the automatic bars/boxes/grid/pointer-caret view of the current frame,
// a CLOSED set of four shapes (`viewFor` below). Anything else falls through to the
// variables panel instead of growing a fifth guess. `.dbg-stage:empty` in debug.css
// still collapses to nothing when a frame's locals earn no picture at all.
//
// Registered through the shared `taliEnhancers` API (a third sibling of walkthrough.js
// and scrolly.js), idempotent via `data-dbg-init`, and self-cleaning: the only thing
// that outlives a live-diff swap is the `setTimeout` play loop, which checks
// `root.isConnected` before scheduling its next tick. The click/keydown listeners live on
// `root` itself, so they are garbage-collected with it. The one exception is the Expand
// control's `fullscreenchange` sync (near the bottom of this file): a SINGLE
// `document`-level listener registered ONCE for the whole page, not once per block, so a
// live-diff swap that mounts and unmounts many `.tali-debug` blocks over a session never
// accumulates listeners to leak. It closes over nothing block-specific and re-queries the
// DOM on every fire, so there is nothing to tear down when a block disconnects.
(function () {
  var STEP_MS = 260;
  var noAutoplay = !!(
    window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches
  );

  /** @type {Record<string, {frames: any[], idx: number}>} */
  var registry = {}; // name -> { frames, idx }

  // The stand-in `tali.frame(n)` returns before a named `.debug` block has mounted (the
  // trace hasn't landed yet, or nothing is named `n` at all). Same shape as a real frame
  // (`drain` below), just empty, so a view cell reads `f.locals.a` / `f.changed.a` on the
  // FIRST render exactly like it does on every later one: the only idiom an author ever
  // needs is the same `(f.locals.a || [])` fallback they already need for a variable the
  // algorithm hasn't assigned yet, not a second, null-specific guard. Returning `null`
  // here (the first shipped version of this feature) forced every view cell to check `f`
  // itself before touching anything on it: ceremony this project's own "perfect the
  // default" convention says to design away rather than merely document. Frozen once, at
  // module load: it is a single shared singleton (never mutated, so `Object.freeze`
  // without `deepFreeze`'s recursion is enough, since every value inside is already
  // empty).
  var EMPTY_FRAME = Object.freeze({
    line: null,
    event: null,
    depth: 0,
    func: '',
    locals: Object.freeze({}),
    changed: Object.freeze({}),
    stack: Object.freeze([]),
    stdout: '',
  });

  // `tali.frame(name)` (tali-js.js) and `window.taliDebug.current`/`.frames` below hand
  // author `{js}` cell source a LIVE reference into this registry, on purpose (a copy on
  // every call would be the wrong cost: a trace can hold up to 5000 frames, and a view
  // cell re-runs on every step). A live reference an author can also WRITE through is a
  // real hole, not a hypothetical one: `tali.frame('sort').line = 99` or
  // `tali.frame('sort').locals.a.push(1)` would permanently corrupt that frame for every
  // later view and every other cell reading it, since there is only ever one object per
  // frame, shared by everyone who asks.
  //
  // `Object.freeze` is shallow (freezing a frame would still leave `frame.locals`
  // writable), so this walks every array/object the parsed JSON contains, recursively,
  // ONCE per block, right after `JSON.parse`, rather than on every read: freezing is the
  // cheap direction (paid once for the whole trace) where a defensive clone would be paid
  // on every reactive re-run of every view cell built on `tali.frame`.
  /** @param {any} v @returns {any} */
  function deepFreeze(v) {
    if (v === null || typeof v !== 'object' || Object.isFrozen(v)) return v;
    Object.freeze(v);
    if (Array.isArray(v)) {
      for (var i = 0; i < v.length; i++) deepFreeze(v[i]);
    } else {
      Object.keys(v).forEach(function (k) {
        deepFreeze(v[k]);
      });
    }
    return v;
  }

  /** The `<script class="tali-debug-trace">` element holding this block's recorded trace,
   * or `null` when none has landed. Inside `.tali-debug` first (keeps this future-proof
   * for a JS-side trace producer that mounts its blob in the container itself, see the
   * file header); the real Python-harness path lands it on the SIBLING `.tali-output`
   * block spliced in right after the container instead. Returned as an element rather than
   * as text because `refresh` below identifies a swapped-in trace by node identity.
   * @param {Element} root @returns {Element | null} */
  function traceEl(root) {
    return (
      root.querySelector('script.tali-debug-trace') ||
      (root.nextElementSibling &&
        root.nextElementSibling.querySelector('script.tali-debug-trace')) ||
      null
    );
  }

  /** @param {string | null} text @returns {{frames: any[], truncated: boolean, cap: number}} */
  function parseTrace(text) {
    if (text === null) return deepFreeze({ frames: [], truncated: false, cap: 0 });
    try {
      return deepFreeze(JSON.parse(text || '{}'));
    } catch (e) {
      console.error('tali-debug: unparseable trace', e);
      return deepFreeze({ frames: [], truncated: false, cap: 0 });
    }
  }

  // --- the JS generator capture adapter --------------------------------------------
  // A `//| trace: true` `{js}` cell has no server-side trace at all: `mod.rs` emits it
  // as plain highlighted SOURCE (see the comment at that branch) carrying the
  // build-time-stamped, runnable text in `data-tali-js-src` rather than as a live
  // `<script type="application/tali-js">`, so there is nothing for `traceEl` above to
  // find and nothing for `tali-js.js`'s own enhancer to run. This section is the other
  // half: find that source, run it exactly like a live `{js}` cell would (same `tali`/
  // `Plot`/`d3`/`num` scope, via `window.taliJs.runDebugSource`), and drain the
  // resulting generator into the SAME frame shape `parseTrace` hands `mount` below, so
  // everything downstream (line cursor, variables panel, the four data views) is
  // written once against that one shape regardless of which language produced it.

  /** The raw (possibly `__at`-stamped) source of a captured `{js}` cell inside `root`,
   * or `null` when this `.debug` block has no such cell (the ordinary Python path).
   * @param {Element} root @returns {string | null} */
  function jsDebugSource(root) {
    var pre = root.querySelector('.dbg-code pre[data-tali-js-src]');
    return pre ? pre.getAttribute('data-tali-js-src') : null;
  }

  // Matches `trace_py.rs`'s own `MAX_DEPTH`/`MAX_ITEMS`: deep enough for a DP table's
  // rows, shallow enough to bound a pathological/cyclic structure; wide enough for a
  // real algorithm's working array, capped before an oversized one costs a reader's tab
  // anything. Measured before this cap existed: a 4,000-element yielded array built
  // 12,229 DOM nodes and cost 114ms per step. `encValue` below is what stops that, the
  // JS adapter's mirror of the Python harness's own `enc()` cap.
  var MAX_DEPTH = 4;
  var MAX_ITEMS = 200;

  // A bounded-depth recursive copy of one yielded value. Not optional: the author's
  // generator keeps mutating the SAME array/object between yields (the design's own
  // worked example does exactly this: `yield {a, i, j}` then `[a[j], a[j+1]] =
  // [a[j+1], a[j]]` on the very next line), so storing the reference verbatim would
  // make every earlier frame's `locals.a` retroactively show the FINAL array once the
  // generator finishes draining, the identical aliasing bug Task 4 found and fixed on
  // the Python side (`trace_py.rs`'s `snapshot`). Beyond `MAX_DEPTH` the value is kept
  // as-is rather than recursed into further, same tradeoff `enc`/`snapshot` make there.
  /** @param {any} v @param {number} [d] @returns {any} */
  function snapshotValue(v, d) {
    var depth = d || 0;
    if (depth > MAX_DEPTH || v === null || typeof v !== 'object') return v;
    if (Array.isArray(v)) {
      return v.map(function (x) {
        return snapshotValue(x, depth + 1);
      });
    }
    /** @type {Record<string, any>} */
    var out = {};
    Object.keys(v).forEach(function (k) {
      out[k] = snapshotValue(v[k], depth + 1);
    });
    return out;
  }

  // Cap a value at `MAX_ITEMS` items for DISPLAY only, the JS twin of `trace_py.rs`'s
  // `enc()`. Deliberately a SEPARATE pass from `snapshotValue` above rather than folded
  // into it: `snapshotValue`'s own output also becomes `prev` for the NEXT frame's
  // `diffLocals` (see `drain` below), and `diffLocals`'s element-wise write detection
  // needs the FULL array to compare index-for-index, exactly like `trace_py.rs`'s own
  // `diff()` runs against the un-truncated snapshot before `enc()` ever caps anything for
  // JSON. Capping inside `snapshotValue` itself would silently downgrade every write on
  // an over-cap array from precise per-index highlighting to a single opaque "changed"
  // flag. An over-cap container becomes the same `{"__trunc__": N, "v": [...]}` escape
  // the Python adapter emits, so `fmt()`'s `__trunc__` branch and `renderStage`'s
  // `truncatedArray()` unwrap (both already written against the Python shape) handle a
  // JS-adapter trace identically, with no adapter-specific branching downstream.
  /** @param {any} v @returns {any} */
  function encValue(v) {
    if (v === null || typeof v !== 'object') return v;
    if (Array.isArray(v)) {
      var mapped = v.slice(0, MAX_ITEMS).map(encValue);
      return v.length <= MAX_ITEMS ? mapped : { __trunc__: v.length, v: mapped };
    }
    var keys = Object.keys(v);
    /** @type {Record<string, any>} */
    var out = {};
    keys.slice(0, MAX_ITEMS).forEach(function (k) {
      out[k] = encValue(v[k]);
    });
    return keys.length <= MAX_ITEMS ? out : { __trunc__: keys.length, v: out };
  }

  // Deep-enough equality for the "did this local change at all" gate below: reference
  // equality first (cheap, and correct for every primitive), then a structural
  // fallback for two independently-snapshotted arrays/objects that happen to hold the
  // same values. `JSON.stringify` is a pragmatic stand-in for real structural equality
  // here (this project does not carry a deep-equal dependency for one small gate), and
  // a false "changed" only costs an extra flash in the variables panel, never a wrong
  // picture (unlike the writes/from-to fields below, which are still computed exactly).
  /** @param {any} a @param {any} b @returns {boolean} */
  function sameValue(a, b) {
    if (a === b) return true;
    if (a === null || b === null || typeof a !== typeof b) return false;
    if (typeof a !== 'object') return false;
    try {
      return JSON.stringify(a) === JSON.stringify(b);
    } catch (e) {
      return false;
    }
  }

  // The JS twin of `trace_py.rs`'s `diff`: element-wise `writes` for two same-length
  // arrays, `from`/`to` otherwise. `reads` is always `[]` here and stays that way: the
  // Python harness derives it from a static AST scan of the traced source, which this
  // adapter deliberately does not attempt (a second, JS-flavoured static analyzer is
  // not a trade this project makes for a value the author can still show directly by
  // yielding it, e.g. `yield {a, i, j, comparing: [j, j + 1]}`).
  /** @param {Record<string, any> | null} prev @param {Record<string, any>} cur
   *  @returns {Record<string, any>} */
  function diffLocals(prev, cur) {
    /** @type {Record<string, any>} */
    var out = {};
    Object.keys(cur).forEach(function (k) {
      var v = cur[k];
      var p = prev ? prev[k] : undefined;
      if (sameValue(p, v)) return;
      if (Array.isArray(v) && Array.isArray(p) && v.length === p.length) {
        var writes = [];
        for (var i = 0; i < v.length; i++) {
          if (!sameValue(v[i], p[i])) writes.push(i);
        }
        if (writes.length) out[k] = { writes: writes, reads: [] };
      } else {
        out[k] = { from: p === undefined ? null : p, to: v };
      }
    });
    return out;
  }

  // Drain a captured generator into the shared frame shape, under the same 5,000-frame
  // cap the Python harness uses (`trace_py.rs`'s `MAX_FRAMES`). `__at` (tali-js.js)
  // stamps a yielded object's `$line`; a cell the yield scanner had to leave unmodified
  // (an unterminated literal, or simply a `yield` it never reached) yields a plain
  // object with no `$line` at all, so `line` is `null` and the cursor just does not
  // move for that frame; never a crash, and never a wrong line.
  /** @param {Generator} gen @returns {{frames: any[], truncated: boolean, cap: number}} */
  function drain(gen) {
    var MAX = 5000,
      frames = [],
      prev = null,
      truncated = false,
      n;
    for (n = 0; n < MAX; n++) {
      var step = gen.next();
      if (step.done) break;
      var v = step.value || {};
      var line = v.$line != null ? v.$line : null;
      // `raw` is the full, UNCAPPED snapshot: it is what `diffLocals` compares against
      // (both this frame's own diff below and the NEXT frame's, via `prev`), so a write
      // past `MAX_ITEMS` is still detected precisely. `locals` is the separate, capped
      // view that actually ships in the frame object (what `renderStage`/`renderVars`/a
      // `tali.frame()` caller ever see); see `encValue`'s own comment for why these two
      // must stay independent passes.
      /** @type {Record<string, any>} */
      var raw = {};
      Object.keys(v).forEach(function (k) {
        if (k !== '$line') raw[k] = snapshotValue(v[k]);
      });
      var changed = diffLocals(prev, raw);
      /** @type {Record<string, any>} */
      var locals = {};
      Object.keys(raw).forEach(function (k) {
        locals[k] = encValue(raw[k]);
      });
      frames.push({
        line: line,
        event: 'line',
        depth: 1,
        func: '',
        locals: locals,
        changed: changed,
        stack: [],
        stdout: '',
      });
      prev = raw;
    }
    if (n >= MAX) truncated = true;
    return { frames: frames, truncated: truncated, cap: MAX };
  }

  // Run the captured source and drain it. Failures (a syntax error the scanner's
  // conservative rewrite still let through, an author exception before the first
  // yield, an exception on a RE-capture triggered by a new input value) are left to
  // REJECT rather than degraded to an empty trace here: the caller (`initJsCapture`
  // below) is the one place that knows whether this is the first capture (nothing
  // built yet, so it degrades to a quiet empty state, matching a Python `.debug` with
  // no trace) or a re-capture (something IS built, and swallowing the error here
  // would leave the OLD trace on screen looking current against a NEW input value --
  // exactly the half-updated block the error affordance exists to avoid).
  /** @param {Element} root @param {string} src @returns {Promise<{frames: any[], truncated: boolean, cap: number}>} */
  async function runJsCapture(root, src) {
    if (!window.taliJs || !window.taliJs.runDebugSource) {
      throw new Error('tali-js.js not loaded');
    }
    var container = document.createElement('div');
    container.id =
      (root.getAttribute('data-debug-name') || 'debug') + '-' + Math.random().toString(36).slice(2);
    var gen = await window.taliJs.runDebugSource(src, container);
    return drain(gen);
  }

  // The debugger's own error box, reusing tali-js.js's `tali-js-error` class (already
  // styled) rather than inventing a second one. Placed inside `.dbg-vars` when that
  // panel already exists (a re-capture failure after a successful mount: clearing the
  // stage too, so a stale array picture never sits next to a NEW input value), or
  // appended to `.dbg-views` when nothing has been built yet (the very first capture
  // failed). Same preview-vs-built message split `showCellError` uses.
  //
  // The transport's STEPPING controls are disabled too, not just left showing a stale
  // count: `recapture` never touches them on a throw (it returns before doing
  // anything, since the error propagates straight out of `runJsCapture`), so without
  // this a reader could click Next and step through the PREVIOUS input's now-orphaned
  // frames while the code panel and the input control both say something else.
  // Deliberately excludes `.dbg-expand` (fullscreen has nothing to do with which
  // trace is loaded) and `.dbg-first`/`.dbg-back`/`.dbg-forward`/`.dbg-last` are left
  // to `bar.sync`'s own idx-based disabling rather than fought here. Re-enabled by
  // `apply()` itself on the next successful frame (an initial success or a later
  // recapture), which is the one place both `.dbg-play` and `.dbg-scrub` are
  // otherwise never touched at all.
  /** @param {Element} root @param {any} e */
  function showDebugError(root, e) {
    var vars = root.querySelector('.dbg-vars');
    var stage = root.querySelector('.dbg-stage');
    if (stage) stage.replaceChildren();
    root.querySelectorAll('.dbg-first, .dbg-back, .dbg-play, .dbg-forward, .dbg-last, .dbg-scrub').forEach(
      function (el) {
        /** @type {HTMLButtonElement | HTMLInputElement} */ (el).disabled = true;
      }
    );
    var msg = document.createElement('pre');
    msg.className = 'tali-js-error';
    msg.textContent =
      typeof window.taliOpenPageSource === 'function'
        ? String((e && e.stack) || e)
        : 'This algorithm view could not be captured.';
    if (vars) {
      vars.replaceChildren(msg);
    } else {
      (root.querySelector('.dbg-views') || root).appendChild(msg);
    }
  }

  // Focus one 1-based line in the panel, reusing the walkthrough/deck `.tali-hl-ln`
  // contract rather than a second highlight vocabulary.
  /** @param {Element | null} pre @param {number} line */
  function focusLine(pre, line) {
    if (!pre) return;
    var lines = pre.querySelectorAll('.tali-hl-ln');
    pre.classList.add('tali-hl-lines-active');
    lines.forEach(function (l, i) {
      l.classList.toggle('tali-hl-ln-hl', i + 1 === line);
    });
    var cur = lines[line - 1];
    if (cur && cur.scrollIntoView) cur.scrollIntoView({ block: 'nearest' });
  }

  // Render one encoded trace value as a compact, Python-flavoured string. Shared between
  // the variables panel here and (eventually) the stage views, so a value reads the same
  // wherever it shows up. Handles the harness's encoding escapes: `__repr__` (an
  // unrepresentable/opaque object), `__set__` (a Python set/frozenset) and `__trunc__` (a
  // container cut at MAX_ITEMS, `trace_py.rs`'s `enc`).
  /** @param {any} v @returns {string} */
  function fmt(v) {
    if (v === null || v === undefined) return 'None';
    if (v === true) return 'True';
    if (v === false) return 'False';
    if (typeof v === 'number') return String(v);
    if (typeof v === 'string') return JSON.stringify(v);
    if (Array.isArray(v)) return '[' + v.map(fmt).join(', ') + ']';
    if (typeof v === 'object') {
      if ('__repr__' in v) return String(v.__repr__);
      if ('__set__' in v) return '{' + (v.__set__ || []).map(fmt).join(', ') + '}';
      if ('__trunc__' in v) {
        var head = Array.isArray(v.v) ? v.v.map(fmt).join(', ') : '';
        var open = Array.isArray(v.v) ? '[' : '{';
        var close = Array.isArray(v.v) ? ']' : '}';
        return open + head + ', …' + close + ' (' + v.__trunc__ + ' total)';
      }
      return (
        '{' +
        Object.keys(v)
          .map(function (k) {
            return k + ': ' + fmt(v[k]);
          })
          .join(', ') +
        '}'
      );
    }
    return String(v);
  }

  // One row per local, marked `dbg-changed` when this frame's diff touched it (the diff
  // shape itself, writes/reads vs from/to, is a later task's concern: here it is only a
  // presence check). The call stack renders only when SOME frame in the trace ever went
  // deeper than the top level (a flat trace has nothing to show), and the stdout pane only
  // when this frame actually buffered output.
  /** @param {Element} el @param {any} frame @param {boolean} hasStack */
  function renderVars(el, frame, hasStack) {
    el.replaceChildren();
    var locals = frame.locals || {};
    var changed = frame.changed || {};
    var list = document.createElement('div');
    list.className = 'dbg-var-list';
    Object.keys(locals).forEach(function (n) {
      var row = document.createElement('div');
      row.className = 'dbg-var' + (Object.prototype.hasOwnProperty.call(changed, n) ? ' dbg-changed' : '');
      var name = document.createElement('span');
      name.className = 'dbg-var-name';
      name.textContent = n;
      var value = document.createElement('span');
      value.className = 'dbg-var-value';
      value.textContent = fmt(locals[n]);
      row.append(name, value);
      list.appendChild(row);
    });
    el.appendChild(list);

    if (hasStack) {
      var stack = document.createElement('div');
      stack.className = 'dbg-stack';
      /** @type {{func: string, line: number}[]} */
      (frame.stack || []).forEach(function (s, i, arr) {
        var f = document.createElement('span');
        f.className = 'dbg-stack-frame' + (i === arr.length - 1 ? ' dbg-stack-current' : '');
        f.textContent = (s.func || '(module)') + ':' + s.line;
        stack.appendChild(f);
      });
      el.appendChild(stack);
    }

    if (frame.stdout) {
      var out = document.createElement('pre');
      out.className = 'dbg-stdout';
      out.textContent = frame.stdout;
      el.appendChild(out);
    }
  }

  // The harness's over-cap escape for a CONTAINER (`trace_py.rs`'s `enc`, mirrored by
  // `encValue` below for the JS adapter): `{"__trunc__": N, "v": [...200 items]}` in
  // place of an array once it holds more than `MAX_ITEMS`. Only the ARRAY shape earns a
  // picture here: an over-cap DICT wraps a plain object in `v`, which none of the four
  // views knows how to draw either way, so that shape is left to the variables panel
  // exactly like an over-cap dict already was, unwrapped or not. Returns `null` for
  // anything else, so a caller can `||` straight through to the un-truncated case.
  /** @param {any} v @returns {any[] | null} */
  function truncatedArray(v) {
    if (v && typeof v === 'object' && !Array.isArray(v) && Array.isArray(v.v) && typeof v.__trunc__ === 'number') {
      return v.v;
    }
    return null;
  }

  // Which built-in view a value earns. A CLOSED set: bars, boxes, grid, or nothing.
  // `null` means "leave it to the variables panel", which is the honest answer for a
  // shape we have no good picture for.
  /** @param {any} v @returns {"bars"|"boxes"|"grid"|null} */
  function viewFor(v) {
    if (!Array.isArray(v) || !v.length) return null;
    if (
      v.every(function (r) {
        return Array.isArray(r);
      })
    )
      return 'grid';
    if (
      v.every(function (x) {
        return typeof x === 'number' && isFinite(x);
      })
    )
      return 'bars';
    if (
      v.every(function (x) {
        return x === null || typeof x !== 'object';
      })
    )
      return 'boxes';
    return null;
  }

  // An integer local that is a valid index into a rendered array becomes a labelled caret
  // under that slot. `i`, `j`, `lo`, `hi`, `left`, `right` need no declaration and no
  // naming convention: being in range IS the signal.
  /** @param {any} locals @param {string} arrayName @param {number} len */
  function pointersInto(locals, arrayName, len) {
    /** @type {{name: string, at: number}[]} */
    var out = [];
    Object.keys(locals).forEach(function (k) {
      var v = locals[k];
      if (k === arrayName) return;
      if (typeof v !== 'number' || !Number.isInteger(v)) return;
      if (v < 0 || v >= len) return;
      out.push({ name: k, at: v });
    });
    return out;
  }

  // Turn a frame's `changed[name].writes`/`.reads` index list into an O(1) membership
  // test. A plain object copy, never a mutation of the frozen array it reads from.
  /** @param {number[] | undefined} indices @returns {Record<number, boolean>} */
  function toSet(indices) {
    /** @type {Record<number, boolean>} */
    var s = {};
    (indices || []).forEach(function (i) {
      s[i] = true;
    });
    return s;
  }

  // One or more pointer badges for a single slot, stacked vertically (never overlapped)
  // so `lo` and `hi` meeting on the same index both stay legible.
  /** @param {{name: string, at: number}[]} ptrs @returns {HTMLElement} */
  function renderPointers(ptrs) {
    var wrap = document.createElement('div');
    wrap.className = 'dbg-ptrs';
    ptrs.forEach(function (p) {
      var tag = document.createElement('span');
      tag.className = 'dbg-ptr';
      tag.textContent = p.name;
      wrap.appendChild(tag);
    });
    return wrap;
  }

  // bars: one `.dbg-bar` per number, height scaled to the largest magnitude in the array
  // (not per-bar, so relative size stays comparable across the row). A value label under
  // each bar only when the array is 24 elements or fewer, so a big array stays legible
  // instead of drowning in text.
  /** @param {number[]} values @param {any} diff @param {any} locals @param {string} name
   *  @returns {HTMLElement} */
  function renderBars(values, diff, locals, name) {
    var row = document.createElement('div');
    row.className = 'dbg-bars';
    var writes = toSet(diff.writes);
    var reads = toSet(diff.reads);
    var max = 0;
    values.forEach(function (v) {
      max = Math.max(max, Math.abs(v));
    });
    if (!max) max = 1;
    var showLabels = values.length <= 24;
    var pointers = pointersInto(locals, name, values.length);
    values.forEach(function (v, i) {
      var slot = document.createElement('div');
      slot.className = 'dbg-slot';
      var track = document.createElement('div');
      track.className = 'dbg-bar-track';
      var bar = document.createElement('div');
      bar.className = 'dbg-bar' + (writes[i] ? ' dbg-write' : '') + (reads[i] ? ' dbg-read' : '');
      bar.setAttribute('data-i', String(i));
      bar.style.height = (Math.abs(v) / max) * 100 + '%';
      track.appendChild(bar);
      slot.appendChild(track);
      if (showLabels) {
        var label = document.createElement('span');
        label.className = 'dbg-bar-label';
        label.textContent = fmt(v);
        slot.appendChild(label);
      }
      var here = pointers.filter(function (p) {
        return p.at === i;
      });
      if (here.length) slot.appendChild(renderPointers(here));
      row.appendChild(slot);
    });
    return row;
  }

  // boxes: any other 1-D array, or a string (the caller splits it into one-character
  // strings first, formatted bare rather than through `fmt` so a box shows `a`, not
  // `"a"`). One `.dbg-box` per value.
  /** @param {any[]} values @param {any} diff @param {any} locals @param {string} name
   *  @param {(v: any) => string} format @returns {HTMLElement} */
  function renderBoxes(values, diff, locals, name, format) {
    var row = document.createElement('div');
    row.className = 'dbg-boxes';
    var writes = toSet(diff.writes);
    var reads = toSet(diff.reads);
    var pointers = pointersInto(locals, name, values.length);
    values.forEach(function (v, i) {
      var slot = document.createElement('div');
      slot.className = 'dbg-slot';
      var box = document.createElement('div');
      box.className = 'dbg-box' + (writes[i] ? ' dbg-write' : '') + (reads[i] ? ' dbg-read' : '');
      box.setAttribute('data-i', String(i));
      box.textContent = format(v);
      slot.appendChild(box);
      var here = pointers.filter(function (p) {
        return p.at === i;
      });
      if (here.length) slot.appendChild(renderPointers(here));
      row.appendChild(slot);
    });
    return row;
  }

  // grid: DP tables, matrices, boards. `changed[name].writes` only pins the ROW a nested
  // `dp[i][j]`-shaped assignment touched (the harness's per-line scan cannot see past the
  // first index of a chained Subscript, see trace_py.rs's `reads_by_line`); the exact
  // cell is resolved here instead, by diffing this row against the SAME row one frame
  // back. Reads stay row-granular for the same structural reason, and a row is shown as
  // read only when it is NOT also the row being written: writing `dp[i][j]` always
  // touches `dp[i]` too (Python must fetch the row before it can store into it), so
  // without this exclusion every written row would also paint itself as read, which is
  // the grid-shaped twin of the flat known defect this task evaluates separately. A row
  // that is purely a lookup (say `dp[i-1]`) keeps its read tint.
  /** @param {string} name @param {any[][]} rows @param {any} diff @param {any} frame @param {any[]} frames
   *  @returns {HTMLElement} */
  function renderGrid(name, rows, diff, frame, frames) {
    var scroll = document.createElement('div');
    scroll.className = 'dbg-grid-scroll';
    var grid = document.createElement('div');
    grid.className = 'dbg-grid';
    var cols = 0;
    rows.forEach(function (r) {
      cols = Math.max(cols, r.length);
    });
    grid.style.setProperty('--dbg-cols', String(cols || 1));
    var writeRows = toSet(diff.writes);
    var readRows = toSet(diff.reads);
    var at = frames.indexOf(frame);
    var prevRows = at > 0 ? frames[at - 1].locals[name] : null;
    rows.forEach(function (row, r) {
      var prevRow = Array.isArray(prevRows) && Array.isArray(prevRows[r]) ? prevRows[r] : null;
      row.forEach(function (v, c) {
        var cell = document.createElement('div');
        var cls = 'dbg-cell';
        var wroteHere = writeRows[r] && (!prevRow || prevRow.length !== row.length || prevRow[c] !== v);
        if (wroteHere) cls += ' dbg-write';
        else if (readRows[r] && !writeRows[r]) cls += ' dbg-read';
        cell.className = cls;
        cell.style.gridColumn = String(c + 1);
        cell.style.gridRow = String(r + 1);
        cell.setAttribute('data-r', String(r));
        cell.setAttribute('data-c', String(c));
        cell.textContent = fmt(v);
        grid.appendChild(cell);
      });
    });
    scroll.appendChild(grid);
    return scroll;
  }

  // The four automatic data views: bars, boxes, grid and in-range pointer carets under a
  // slot. One `.dbg-view` per local that earns a picture (`viewFor`, plus the string
  // special-case above it); everything else already has its honest answer in the
  // variables panel, so it is skipped here rather than guessed at.
  /** @param {Element} el @param {any} frame @param {any[]} frames */
  function renderStage(el, frame, frames) {
    el.replaceChildren();
    var locals = frame.locals || {};
    var changed = frame.changed || {};
    Object.keys(locals).forEach(function (name) {
      var value = locals[name];
      var diff = changed[name] || {};
      /** @type {HTMLElement | null} */
      var body = null;
      // An over-cap container (past MAX_ITEMS on either adapter) still earns a picture:
      // unwrap to its capped `v` array and draw exactly that, rather than falling
      // through to the variables panel just because the value is no longer a bare
      // array. `diff.writes`/`.reads` were computed against the FULL, un-truncated
      // value (`trace_py.rs`'s `diff` runs before `enc` truncates for display), so an
      // index past the visible slots is simply never reached by the render loops below:
      // never a crash, just a caret or highlight this view cannot show.
      var trunc = truncatedArray(value);
      var arr = trunc || value;
      if (typeof arr === 'string' && arr.length) {
        // A literal space renders as an invisible, width-collapsing box; a non-breaking
        // space keeps the slot visible, so a palindrome check over "a man a plan" still
        // shows one box per character.
        body = renderBoxes(arr.split(''), diff, locals, name, function (c) {
          return c === ' ' ? ' ' : c;
        });
      } else {
        var kind = viewFor(arr);
        if (kind === 'bars') body = renderBars(arr, diff, locals, name);
        else if (kind === 'boxes') body = renderBoxes(arr, diff, locals, name, fmt);
        else if (kind === 'grid') body = renderGrid(name, arr, diff, frame, frames);
      }
      if (!body) return;
      var view = document.createElement('div');
      view.className = 'dbg-view';
      var label = document.createElement('div');
      label.className = 'dbg-view-label';
      label.textContent = name;
      view.append(label, body);
      if (trunc) {
        // Honest about what is NOT shown: the harness already cut the container at
        // MAX_ITEMS (200) before this ever reached the client, so the picture itself is
        // real for every slot it draws, only incomplete past the cap.
        var note = document.createElement('div');
        note.className = 'dbg-view-truncated';
        note.textContent = 'showing ' + trunc.length + ' of ' + value.__trunc__ + ' (truncated)';
        view.appendChild(note);
      }
      el.appendChild(view);
    });
  }

  // The transport bar: first/back/play-pause/forward/last, a scrub range, a step count and
  // an expand button (fullscreen toggle, wired in `init` via `toggleExpand`/
  // `syncExpandButtons` below). Built with no outside references so `init` wires every
  // control's behaviour itself, over the plain element handles returned alongside
  // `{el, sync}`.
  /** @returns {{el: HTMLElement, sync: (i: number, total: number, frame: any) => void,
   *   first: HTMLButtonElement, back: HTMLButtonElement, play: HTMLButtonElement | null,
   *   forward: HTMLButtonElement, last: HTMLButtonElement, range: HTMLInputElement,
   *   expand: HTMLButtonElement}} */
  function buildTransport() {
    var el = document.createElement('div');
    el.className = 'dbg-transport';

    /** @param {string} label @param {string} cls @returns {HTMLButtonElement} */
    function mkBtn(label, cls) {
      var b = document.createElement('button');
      b.type = 'button';
      b.className = 'dbg-btn ' + cls;
      b.setAttribute('aria-label', label);
      b.textContent = label;
      el.appendChild(b);
      return b;
    }

    var first = mkBtn('First', 'dbg-first');
    var back = mkBtn('Back', 'dbg-back');
    // `prefers-reduced-motion: reduce` disables autoplay entirely: no play button at all,
    // rather than a button that does nothing when pressed.
    var play = noAutoplay ? null : mkBtn('Play', 'dbg-play');
    var forward = mkBtn('Next', 'dbg-forward');
    var last = mkBtn('Last', 'dbg-last');

    var range = /** @type {HTMLInputElement} */ (document.createElement('input'));
    range.type = 'range';
    range.className = 'dbg-scrub';
    range.min = '0';
    el.appendChild(range);

    var count = document.createElement('span');
    count.className = 'dbg-count';
    el.appendChild(count);

    // `aria-pressed` starts honest (nothing is fullscreen yet); `syncExpandButtons` below
    // keeps it honest afterward, including when the browser exits fullscreen on its own
    // (Escape) without ever calling our click handler.
    var expand = mkBtn('Expand', 'dbg-expand');
    expand.setAttribute('aria-pressed', 'false');

    /** @param {number} i @param {number} total */
    function sync(i, total) {
      range.max = String(Math.max(total - 1, 0));
      range.value = String(i);
      // `total === 0` (an empty capture) is the one case `i + 1` lies: there is no step
      // "1" to be on when there are zero steps. Show the honest "0 of 0" / "0 / 0"
      // rather than the 1-based count that only makes sense once at least one frame
      // exists.
      var shown = total > 0 ? i + 1 : 0;
      range.setAttribute('aria-valuetext', 'step ' + shown + ' of ' + total);
      count.textContent = shown + ' / ' + total;
      first.disabled = back.disabled = i <= 0 || total === 0;
      forward.disabled = last.disabled = i >= total - 1 || total === 0;
    }

    return {
      el: el,
      sync: sync,
      first: first,
      back: back,
      play: play,
      forward: forward,
      last: last,
      range: range,
      expand: expand,
    };
  }

  // Fullscreen toggle for one `.tali-debug` block: reused verbatim from deck.js's
  // `toggleFullscreen` (crates/core/assets/js/deck.js, search `toggleFullscreen`), the
  // project's one guarded pattern for this, rather than a second implementation here. The
  // `.dbg-overlay` class is the fallback for wherever the real Fullscreen API is missing
  // or throws (a sandboxed iframe, a permissions-policy block, a browser that never
  // implemented it): a fixed-position overlay that debug.css styles to the same layout.
  /** @param {Element} root */
  function toggleExpand(root) {
    try {
      if (document.fullscreenElement) document.exitFullscreen();
      else if (root.requestFullscreen) root.requestFullscreen();
      else root.classList.toggle('dbg-overlay');
    } catch (e) {
      root.classList.toggle('dbg-overlay');
    }
  }

  // Keep every mounted block's Expand button honest in one pass: `aria-pressed`, its
  // label, and its visible text. Driven by the single page-level `fullscreenchange`
  // listener registered once near the bottom of this file (see the file header for why
  // that is one listener total, not one per block), and also called directly right after
  // a click that used the `.dbg-overlay` fallback, since toggling a plain class never
  // fires `fullscreenchange` (that event belongs to the real Fullscreen API only). Reads
  // fresh DOM state each call rather than closing over any one block, so it stays correct
  // across live-diff swaps with no teardown needed.
  function syncExpandButtons() {
    document.querySelectorAll('.tali-debug').forEach(function (el) {
      var btn = el.querySelector('.dbg-expand');
      if (!btn) return;
      var active = document.fullscreenElement === el || el.classList.contains('dbg-overlay');
      btn.setAttribute('aria-pressed', active ? 'true' : 'false');
      btn.setAttribute('aria-label', active ? 'Collapse' : 'Expand');
      btn.textContent = active ? 'Collapse' : 'Expand';
    });
  }

  // Drive the JS capture adapter's whole lifecycle: the initial capture, AND
  // re-capturing whenever a `//| input:` name this cell depends on changes -- the
  // adapter's entire reason to exist over the Python one (spec: "the reader can
  // change the input and re-run"). `data-debug-inputs` (divs.rs) is the cell's own
  // `//| input:` names, surfaced into the DOM because the server already strips the
  // `//|` option lines from the displayed source.
  //
  // `recapture` starts `null` and is filled in by `mount`'s return value the first
  // time a capture actually produces frames to show; every capture after that
  // (whether the first one came back empty or not) either builds the widget for the
  // first time or replays through the SAME recapture closure `mount` returned, so
  // Python-style chrome (transport, cursor, vars, four views) is never duplicated.
  //
  // Re-entrancy: an `epoch` counter is bumped on every capture request (initial or
  // input-driven) and captured as `myEpoch` in that request's own closure. A capture
  // is async (`runDebugSource` always goes through a real `AsyncFunction`), so a
  // slider dragged quickly can have several in flight at once; when one resolves, it
  // applies its result only if `myEpoch` still matches the CURRENT `epoch` -- an
  // older capture that resolves after a newer one has already started is dropped, so
  // a stale array can never overwrite a fresher one no matter which promise settles
  // first.
  /** @param {Element} root @param {string} jsSrc */
  function initJsCapture(root, jsSrc) {
    var inputNames = (root.getAttribute('data-debug-inputs') || '')
      .split(',')
      .map(function (s) {
        return s.trim();
      })
      .filter(Boolean);
    var epoch = 0;
    /** @type {((trace: {frames: any[], truncated: boolean, cap: number}) => void) | null} */
    var recapture = null;

    function runOnce() {
      var myEpoch = ++epoch;
      runJsCapture(root, jsSrc)
        .then(function (trace) {
          if (myEpoch !== epoch) return; // superseded by a later capture
          var frozen = deepFreeze(trace);
          if (recapture) {
            recapture(frozen);
          } else {
            recapture = mount(root, frozen);
          }
        })
        .catch(function (e) {
          if (myEpoch !== epoch) return;
          console.error('tali-debug: JS capture failed', e);
          showDebugError(root, e);
        });
    }

    runOnce();
    if (inputNames.length && window.taliJs && window.taliJs.onInputChange) {
      window.taliJs.onInputChange(inputNames, runOnce);
    }
  }

  // Dispatch to the right capture adapter. The JS path is async and self-driving
  // (`initJsCapture` above owns its own re-capture loop); the Python path stays
  // fully synchronous, reading a trace the server already baked into the page, and
  // mounts once with nothing further to wire up. Either way `mount` below is the
  // single place that turns a `{frames, truncated, cap}` trace into the transport
  // bar, the line cursor, the variables panel and the four data views, so the two
  // adapters cannot drift in how they are rendered even though they arrive
  // completely differently.
  /** Per-block state for the PYTHON path only, so `refresh` below can tell an
   * already-mounted block whose trace changed underneath it from one that is untouched.
   * `el` is the trace `<script>` node the block was built from (identity, not text: a
   * live-diff swap replaces the node, an untouched block keeps it, so the common case
   * costs one comparison); `text` is that node's content, so a swap that happens to carry
   * a byte-identical trace does not reset the reader's position to frame 0; `recapture` is
   * what `mount` handed back, or `null` when there were no frames to build chrome from.
   * A `{js}` block has no entry at all: `initJsCapture` owns its own re-capture loop.
   * @type {WeakMap<Element, {el: Element | null, text: string | null,
   *   recapture: ((trace: {frames: any[], truncated: boolean, cap: number}) => void) | null}>} */
  var pyState = new WeakMap();

  /** @param {Element} root */
  function init(root) {
    var jsSrc = jsDebugSource(root);
    if (jsSrc !== null) {
      initJsCapture(root, jsSrc);
      return;
    }
    var el = traceEl(root);
    var text = el ? el.textContent || '' : null;
    pyState.set(root, { el: el, text: text, recapture: mount(root, parseTrace(text)) });
  }

  /** Re-point an already-mounted Python `.debug` block at a trace that changed underneath
   * it. debug.js is the first enhancer whose state lives OUTSIDE its own container: the
   * recorded trace rides in the SIBLING output block, so editing an upstream cell in live
   * preview re-runs the traced cell and replaces that sibling while leaving `.tali-debug`
   * itself (same block id, unchanged source) exactly where it was. `enhance`'s
   * `:not([data-dbg-init])` query can therefore never revisit the widget, and it went on
   * stepping the old trace until a manual reload. The mark-and-skip idempotence
   * scrolly.js/walkthrough.js use does not transfer for exactly that reason: their state
   * is all inside the element they marked.
   *
   * Re-uses the `recapture` closure `mount` returned rather than re-running `init`: it
   * swaps the frames into the chrome that is already built, so the click/keydown listeners
   * bound to `root` are never bound a second time. A block that had no frames at all built
   * no chrome (and bound nothing), so that one mounts for real now.
   * @param {Element} root */
  function refresh(root) {
    var state = pyState.get(root);
    if (!state) return;
    var el = traceEl(root);
    if (!el || el === state.el) return;
    state.el = el;
    var text = el.textContent || '';
    if (text === state.text) return;
    state.text = text;
    var trace = parseTrace(text);
    if (state.recapture) state.recapture(trace);
    else state.recapture = mount(root, trace);
  }

  // Builds the widget chrome once frames actually exist, and returns a `recapture`
  // function closed over the SAME `frames`/`idx`/`playing`/`timer`/`bar`/`vars`/
  // `stage` this call built, so a later JS capture can swap the trace in place
  // instead of building a second copy of the chrome beside the first. Returns `null`
  // when there is nothing to show yet (a Python `.debug` with no trace at all, or a
  // JS capture whose very first run produced zero frames) -- the caller is
  // responsible for deciding what "nothing yet" means for its own adapter.
  /** @param {Element} root @param {{frames: any[], truncated: boolean, cap: number}} trace
   *  @returns {((trace: {frames: any[], truncated: boolean, cap: number}) => void) | null} */
  function mount(root, trace) {
    var frames = trace.frames || [];
    var name = root.getAttribute('data-debug-name');
    var pre = root.querySelector('.dbg-code pre');
    var bridge = /** @type {HTMLInputElement | null} */ (root.querySelector('.tali-debug-input'));
    if (!frames.length) return null;

    var idx = 0;
    var playing = false;
    var timer = /** @type {number | null} */ (null);
    var speed = 1;
    // `apply` below only publishes to the bridge on a VALUE change, which is the right
    // rule step-to-step (a re-render every 260ms of Play should not spam a downstream
    // cell when the index didn't move), but it means the very FIRST publish, always
    // index 0 against a bridge whose server-rendered value is already the string "0",
    // compares equal and is silently skipped. Left alone, a `{js}` view cell reading
    // `tali.frame(name)` (which ran once already, during the page's initial reactive
    // pass, before THIS mount even started, and so saw the empty stand-in frame) would
    // never hear that the real frame 0 landed, unless the reader manually steps at
    // least once first: the same "before this block has mounted" gap `tali.frame`'s
    // empty-frame fallback exists for, just persisting past mount instead of ending at
    // it. `forcePublish` overrides the equality check exactly once per mount and once
    // per re-capture (set again at the top of `recapture` below, for the identical
    // reason: a freshly captured trace's own frame 0 is new data even when the bridge's
    // STRING value happens not to change, e.g. the reader was already sitting at index
    // 0 when the input that triggered the re-capture fired).
    var forcePublish = true;
    if (name) registry[name] = { frames: frames, idx: 0 };
    // The call stack is worth a row only if some frame in the WHOLE trace ever recursed
    // or called into a nested function; computed once, not per-frame.
    var hasStack = frames.some(function (f) {
      return f.depth > 1;
    });

    if (trace.truncated) {
      var warn = document.createElement('p');
      warn.className = 'dbg-truncated';
      warn.textContent = 'Trace truncated at ' + trace.cap + ' steps.';
      root.prepend(warn);
    }

    var bar = buildTransport();
    var vars = document.createElement('div');
    vars.className = 'dbg-vars';
    var stage = document.createElement('div');
    stage.className = 'dbg-stage';
    var codeEl = root.querySelector('.dbg-code');
    if (codeEl) codeEl.after(stage);
    root.append(vars, bar.el);

    /** @param {number} i */
    function apply(i) {
      // Inert after an empty re-capture: `frames.length - 1` is `-1` there, so the
      // clamp below would still settle on `idx = 0` and hand `frames[0]` (`undefined`)
      // to `focusLine`/`renderVars`/`renderStage`, which crash. `recapture` already
      // disables every control this reaches THROUGH (see there), but the keydown
      // handler calls `apply` directly and does not consult any control's `disabled`
      // state, so this guard is the one place that actually stops the crash regardless
      // of entry point (arrow keys, a dragged `.dbg-scrub`, or a `.dbg-play` tick).
      if (!frames.length) return;
      idx = Math.max(0, Math.min(frames.length - 1, i));
      var f = frames[idx];
      // Undo any disabling `showDebugError` did after a failed capture: reaching
      // here at all means a real frame exists to show. `bar.range` and `bar.play`
      // are the two controls `bar.sync` below never manages either way (it only
      // ever sets first/back/forward/last from the current position), so this is
      // the one place they can be relied on to come back.
      bar.range.disabled = false;
      if (bar.play) bar.play.disabled = false;
      if (name) registry[name].idx = idx;
      focusLine(pre, f.line);
      renderVars(vars, f, hasStack);
      renderStage(stage, f, frames);
      bar.sync(idx, frames.length, f);
      // Publish LAST, so a view cell that re-runs synchronously reads a settled registry.
      if (bridge && (forcePublish || bridge.value !== String(idx))) {
        bridge.value = String(idx);
        bridge.dispatchEvent(new Event('input', { bubbles: true }));
        forcePublish = false;
      }
    }

    /** Reflect play/pause on the button; a no-op when reduced motion dropped it. */
    function syncPlayButton() {
      if (!bar.play) return;
      bar.play.textContent = playing ? 'Pause' : 'Play';
      bar.play.setAttribute('aria-label', playing ? 'Pause' : 'Play');
      bar.play.classList.toggle('dbg-playing', playing);
    }

    function tick() {
      timer = null;
      if (!root.isConnected) {
        // The container was swapped out by a live diff mid-play: stop rescheduling
        // rather than stepping a detached frame that no reader can see.
        playing = false;
        return;
      }
      if (idx >= frames.length - 1) {
        playing = false;
        syncPlayButton();
        return;
      }
      apply(idx + 1);
      if (playing) timer = window.setTimeout(tick, STEP_MS / speed);
    }

    function togglePlay() {
      if (noAutoplay || !bar.play) return;
      playing = !playing;
      syncPlayButton();
      if (playing) {
        if (idx >= frames.length - 1) apply(0);
        timer = window.setTimeout(tick, STEP_MS / speed);
      } else if (timer !== null) {
        window.clearTimeout(timer);
        timer = null;
      }
    }

    bar.first.addEventListener('click', function () {
      apply(0);
    });
    bar.back.addEventListener('click', function () {
      apply(idx - 1);
    });
    bar.forward.addEventListener('click', function () {
      apply(idx + 1);
    });
    bar.last.addEventListener('click', function () {
      apply(frames.length - 1);
    });
    bar.range.addEventListener('input', function () {
      apply(+bar.range.value);
    });
    if (bar.play) {
      bar.play.addEventListener('click', function () {
        togglePlay();
      });
    }
    bar.expand.addEventListener('click', function () {
      toggleExpand(root);
      // The real Fullscreen API reports back through the page-level `fullscreenchange`
      // listener (including on Escape); the `.dbg-overlay` fallback never fires that
      // event at all, so sync here too. Idempotent either way, since both paths land on
      // the same DOM-driven check.
      syncExpandButtons();
    });

    // Keyboard on the CONTAINER, not on individual controls: `tabindex="-1"` keeps it out
    // of the normal tab order (the buttons and the range are still reachable that way),
    // and a click anywhere inside (including on a non-focusable area like the code panel)
    // focuses it, so arrow keys keep stepping without having to re-click a button each
    // time. `preventDefault()` on every handled key also suppresses the range input's own
    // native arrow-key behaviour and a focused button's native Space-triggers-click, so
    // one press never double-steps.
    var rootEl = /** @type {HTMLElement} */ (root);
    rootEl.tabIndex = -1;
    rootEl.addEventListener('click', function () {
      rootEl.focus();
    });
    /** @param {KeyboardEvent} e */
    function onKeydown(e) {
      switch (e.key) {
        case 'ArrowLeft':
          e.preventDefault();
          apply(idx - 1);
          break;
        case 'ArrowRight':
          e.preventDefault();
          apply(idx + 1);
          break;
        case 'Home':
          e.preventDefault();
          apply(0);
          break;
        case 'End':
          e.preventDefault();
          apply(frames.length - 1);
          break;
        case ' ':
        case 'Spacebar':
          e.preventDefault();
          togglePlay();
          break;
        case 'Escape':
          // The real Fullscreen API already handles Escape itself (a browser-level
          // shortcut our `preventDefault()` cannot and should not intercept); only the
          // `.dbg-overlay` fallback needs this, since it is nothing more than a class
          // toggle with no native Escape behaviour of its own.
          if (!rootEl.classList.contains('dbg-overlay')) return;
          e.preventDefault();
          rootEl.classList.remove('dbg-overlay');
          syncExpandButtons();
          break;
        default:
          return;
      }
    }
    rootEl.addEventListener('keydown', onKeydown);

    apply(0);

    // Swap a freshly captured trace into this SAME widget: reuses every closure
    // above (`frames`/`idx`/`playing`/`timer`/`hasStack`/`bar`/`vars`/`stage`/`pre`/
    // `bridge`/`name`) rather than tearing the DOM down and rebuilding it, which is
    // what keeps a re-capture from flashing the whole block or losing focus.
    /** @param {{frames: any[], truncated: boolean, cap: number}} newTrace */
    function recapture(newTrace) {
      // A freshly captured trace's own frame 0 is new data even when the bridge's
      // STRING value happens not to change (see `forcePublish`'s declaration above);
      // force the publish below's next `apply(0)` to fire regardless.
      forcePublish = true;
      // Stop any in-flight play loop FIRST: it closes over `frames` by reference, so
      // if it fired again after `frames` is reassigned below it would step through
      // the wrong array (or past its new, possibly shorter, end).
      if (playing) {
        togglePlay();
      } else if (timer !== null) {
        window.clearTimeout(timer);
        timer = null;
      }
      frames = newTrace.frames || [];
      hasStack = frames.some(function (f) {
        return f.depth > 1;
      });
      // A stale truncation warning (or the lack of one) must not survive a
      // re-capture with a different frame count.
      var oldWarn = root.querySelector('.dbg-truncated');
      if (oldWarn) oldWarn.remove();
      if (newTrace.truncated) {
        var warn = document.createElement('p');
        warn.className = 'dbg-truncated';
        warn.textContent = 'Trace truncated at ' + newTrace.cap + ' steps.';
        root.prepend(warn);
      }
      idx = 0;
      if (!frames.length) {
        // A legitimate empty re-capture (the new input value yields nothing): leave the
        // block honestly INERT rather than a stale transport over frames that no longer
        // exist.
        //
        // The registry entry is left UNTOUCHED here, not overwritten with an empty one:
        // this used to run BEFORE this check, so `registry[name] = {frames: [], idx: 0}`
        // landed even on an empty result, and a `tali.frame(name)` caller (a downstream
        // view cell) got reset to nothing just because the reader typed an input value
        // that happens to yield an empty trace, instead of still seeing the last good
        // frame. `window.taliDebug.current` already tolerates a registry entry that is
        // absent, stale, or shorter than its `idx` (falls back to `EMPTY_FRAME`), so
        // nothing downstream depends on this write happening.
        //
        // `bar.range`/`bar.play` are disabled explicitly: `bar.sync` only ever manages
        // first/back/forward/last (see its own comment), and the keydown handler calls
        // `apply` directly regardless of any control's `disabled` state: `apply`'s own
        // `!frames.length` guard is what actually stops that path from crashing, this is
        // the honest-UI half of the fix.
        bar.sync(0, 0, null);
        bar.range.disabled = true;
        if (bar.play) bar.play.disabled = true;
        vars.replaceChildren();
        stage.replaceChildren();
        return;
      }
      // Replace, not mutate: any prior array a `tali.frame(name)` caller still holds
      // stays exactly what it was (frozen, and now orphaned), never rewritten out
      // from under it, and `window.taliDebug.current`/`.frames` see the new trace on
      // their very next call.
      if (name) registry[name] = { frames: frames, idx: 0 };
      apply(0);
    }

    return recapture;
  }

  window.taliDebug = {
    /** Every recorded frame for a named `.debug` block. Read-only: the array and every
     * frame (and every value nested inside a frame) came through `deepFreeze` above, so
     * a write attempt through the returned reference is a no-op in sloppy-mode caller
     * code and throws in strict-mode caller code (both are `Object.freeze`'s ordinary
     * behaviour); it is never silently accepted as a real mutation of stepper state.
     * @param {string} n */
    frames: function (n) {
      return registry[n] ? registry[n].frames : [];
    },
    /** The frame the stepper is currently sitting on, or the shared `EMPTY_FRAME` when
     * no `.debug` block named `n` has mounted a real frame there (never `undefined`; see
     * `EMPTY_FRAME` above). Frozen the same way `frames` is.
     *
     * Guards on the FRAME, not just on the registry entry: `r.frames[r.idx]` can be
     * `undefined` even when `registry[n]` exists, in two reproduced routes. (a) An empty
     * re-capture used to write `registry[name] = {frames: [], idx: 0}` before checking
     * whether there was anything to show, so `frames[0]` was `undefined` (fixed
     * separately, in `recapture`, by not writing an empty entry at all, but a stale
     * `idx` can still outrun a SHORTER later trace, see (b)). (b) Two `.debug` blocks
     * sharing the same `name=` overwrite each other's registry entry; one block's `apply`
     * can then publish an `idx` past the OTHER (shorter) block's `frames.length`. Either
     * way, a view cell written exactly as the guide prescribes (`f.locals.a || []`)
     * crashed with `TypeError: Cannot read properties of undefined (reading 'locals')`
     * instead of getting the documented "always safe to read" frame.
     * @param {string} n */
    current: function (n) {
      var r = registry[n];
      var f = r ? r.frames[r.idx] : undefined;
      return f !== undefined ? f : EMPTY_FRAME;
    },
  };

  /** @param {ParentNode | null} [root] */
  function enhance(root) {
    var scope = root || document;
    scope.querySelectorAll('.tali-debug:not([data-dbg-init])').forEach(function (el) {
      el.setAttribute('data-dbg-init', '1');
      init(el);
    });
    // Already-mounted blocks whose trace blob was replaced underneath them (see `refresh`).
    // The live client calls every enhancer with `#tali-root`, not with the swapped block,
    // so a sibling-only change is still in scope here.
    scope.querySelectorAll('.tali-debug[data-dbg-init]').forEach(refresh);
  }

  // The ONE `document`-level listener this file ever adds, registered exactly once at
  // module load (this IIFE runs once per page), never per block and never removed: see
  // the file header and `syncExpandButtons` above for why one listener for the whole page
  // is the right shape here, rather than one per `.tali-debug` that would need teardown
  // on every live-diff swap.
  document.addEventListener('fullscreenchange', syncExpandButtons);

  if (window.taliEnhancers && window.taliEnhancers.register) {
    window.taliEnhancers.register(enhance);
  } else {
    document.addEventListener('DOMContentLoaded', function () {
      enhance(document);
    });
  }
})();
