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
// class="tali-output" ...><script class="tali-debug-trace">`). `readTrace` below looks
// there first.
//
// Stepping publishes the frame index into the hidden input, which is the SAME bridge
// scrolly uses, so a `{js}` view cell re-runs through `//| input:` with no new reactive
// machinery. Read-only: nothing here ever writes source.
//
// This file owns the transport chrome, the line cursor and the variables panel.
// `renderStage` (the bars/boxes/grids/pointers view of the current frame) is a
// deliberate no-op stub: that rendering is a later task's job, and `.dbg-stage:empty`
// collapses to nothing in debug.css so the stub costs no layout space meanwhile.
//
// Registered through the shared `taliEnhancers` API (a third sibling of walkthrough.js
// and scrolly.js), idempotent via `data-dbg-init`, and self-cleaning: the only thing
// that outlives a live-diff swap is the `setTimeout` play loop, which checks
// `root.isConnected` before scheduling its next tick. The click/keydown listeners live on
// `root` itself, so they are garbage-collected with it: nothing is attached to
// `window`/`document` that would need explicit teardown.
(function () {
  var STEP_MS = 260;
  var noAutoplay = !!(
    window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches
  );

  /** @type {Record<string, {frames: any[], idx: number}>} */
  var registry = {}; // name -> { frames, idx }

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

  /** @param {Element} root @returns {{frames: any[], truncated: boolean, cap: number}} */
  function readTrace(root) {
    // Inside `.tali-debug` first (keeps this future-proof for a JS-side trace producer
    // that mounts its blob in the container itself, see the file header); the real
    // Python-harness path lands it on the sibling `.tali-output` block splice in right
    // after the container instead.
    var el =
      root.querySelector('script.tali-debug-trace') ||
      (root.nextElementSibling && root.nextElementSibling.querySelector('script.tali-debug-trace'));
    if (!el) return deepFreeze({ frames: [], truncated: false, cap: 0 });
    try {
      return deepFreeze(JSON.parse(el.textContent || '{}'));
    } catch (e) {
      console.error('tali-debug: unparseable trace', e);
      return deepFreeze({ frames: [], truncated: false, cap: 0 });
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

  // Data-structure views (bars/boxes/grids/pointers) are a later task's job. Intentionally
  // empty here: `dbg-stage` stays in the DOM as the slot that task fills in, and
  // `.dbg-stage:empty` in debug.css keeps an unfilled slot from taking any layout space.
  /** @param {Element} el @param {any} frame @param {any[]} frames */
  function renderStage(el, frame, frames) {}

  // The transport bar: first/back/play-pause/forward/last, a scrub range, a step count and
  // an expand button (the last one is inert here; a later task wires fullscreen to it, but
  // rendering it now means the bar's layout doesn't shift when that lands). Built with no
  // outside references so `init` wires every control's behaviour itself, over the plain
  // element handles returned alongside `{el, sync}`.
  /** @returns {{el: HTMLElement, sync: (i: number, total: number, frame: any) => void,
   *   first: HTMLButtonElement, back: HTMLButtonElement, play: HTMLButtonElement | null,
   *   forward: HTMLButtonElement, last: HTMLButtonElement, range: HTMLInputElement}} */
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

    // Wired in a later task (fullscreen); rendered now so the bar's width is stable.
    mkBtn('Expand', 'dbg-expand');

    /** @param {number} i @param {number} total */
    function sync(i, total) {
      range.max = String(Math.max(total - 1, 0));
      range.value = String(i);
      range.setAttribute('aria-valuetext', 'step ' + (i + 1) + ' of ' + total);
      count.textContent = i + 1 + ' / ' + total;
      first.disabled = back.disabled = i <= 0;
      forward.disabled = last.disabled = i >= total - 1;
    }

    return { el: el, sync: sync, first: first, back: back, play: play, forward: forward, last: last, range: range };
  }

  /** @param {Element} root */
  function init(root) {
    var trace = readTrace(root);
    var frames = trace.frames || [];
    var name = root.getAttribute('data-debug-name');
    var pre = root.querySelector('.dbg-code pre');
    var bridge = /** @type {HTMLInputElement | null} */ (root.querySelector('.tali-debug-input'));
    if (!frames.length) return;

    var idx = 0;
    var playing = false;
    var timer = /** @type {number | null} */ (null);
    var speed = 1;
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
      idx = Math.max(0, Math.min(frames.length - 1, i));
      var f = frames[idx];
      if (name) registry[name].idx = idx;
      focusLine(pre, f.line);
      renderVars(vars, f, hasStack);
      renderStage(stage, f, frames);
      bar.sync(idx, frames.length, f);
      // Publish LAST, so a view cell that re-runs synchronously reads a settled registry.
      if (bridge && bridge.value !== String(idx)) {
        bridge.value = String(idx);
        bridge.dispatchEvent(new Event('input', { bubbles: true }));
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
        default:
          return;
      }
    }
    rootEl.addEventListener('keydown', onKeydown);

    apply(0);
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
    /** The frame the stepper is currently sitting on, or `null` before any `.debug` block
     * named `n` has mounted. Frozen the same way `frames` is. @param {string} n */
    current: function (n) {
      var r = registry[n];
      return r ? r.frames[r.idx] : null;
    },
  };

  /** @param {ParentNode | null} [root] */
  function enhance(root) {
    (root || document).querySelectorAll('.tali-debug:not([data-dbg-init])').forEach(function (el) {
      el.setAttribute('data-dbg-init', '1');
      init(el);
    });
  }

  if (window.taliEnhancers && window.taliEnhancers.register) {
    window.taliEnhancers.register(enhance);
  } else {
    document.addEventListener('DOMContentLoaded', function () {
      enhance(document);
    });
  }
})();
