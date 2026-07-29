// `{glsl}` cells — a fragment shader onto a live <canvas>, driven by the same reactive
// graph a `{js}` cell uses.
//
// This file is the whole of the `{glsl}` language, and its size is the point: it is a
// REGISTRATION against the seam `tali-js.js` exposes (`window.taliJs.registerLanguage`),
// not a second runtime. Mounting, `//| name:` publication, the dependency graph, the error
// box, teardown and click-to-source are the shared wrapper's job and appear nowhere below.
//
// Why `{glsl}` and not `{sql}`/`{ts}` first: WebGL is a browser API. There is nothing to
// vendor, no licence question and no payload — the entire language costs this file, which
// is why it could ship as the proof the seam works rather than as its own project.
//
// The cell's source IS the fragment shader body. Taliesin supplies the vertex shader (a
// full-screen triangle) and a small uniform preamble, so the author writes only the part
// that differs:
//
//   ```{glsl}
//   //| input: freq
//   void main() {
//     vec2 p = uv * 2.0 - 1.0;
//     float v = sin(p.x * u_freq) * cos(p.y * u_freq);
//     gl_FragColor = vec4(vec3(0.5 + 0.5 * v), 1.0);
//   }
//   ```
//
// Preamble contract (documented in docs/guide/using/interactive.tmd):
//   varying vec2 uv        pixel position, 0..1, y up
//   uniform vec2 u_res     canvas size in device pixels
//   uniform float u_time   seconds since the cell mounted (only ticks if referenced)
//   uniform float u_<name> one per `//| input:` name whose value is a number
//   uniform vec2  u_<name> one per `//| input:` name whose value is an {x, y}
(function () {
  "use strict";

  var VERT = [
    "attribute vec2 a_pos;",
    "varying vec2 uv;",
    "void main() {",
    "  uv = a_pos * 0.5 + 0.5;",
    "  gl_Position = vec4(a_pos, 0.0, 1.0);",
    "}",
  ].join("\n");

  /** @param {string[]} inputs @param {any} api */
  function preamble(inputs, api) {
    var lines = [
      "precision mediump float;",
      "varying vec2 uv;",
      "uniform vec2 u_res;",
      "uniform float u_time;",
    ];
    inputs.forEach(function (n) {
      if (!/^[A-Za-z_]\w*$/.test(n)) return; // not a GLSL identifier; skip rather than emit invalid source
      var v = api.value(n);
      var type = v && typeof v === "object" && typeof v.x === "number" ? "vec2" : "float";
      lines.push("uniform " + type + " u_" + n + ";");
    });
    return lines.join("\n") + "\n";
  }

  /** @param {WebGLRenderingContext} gl @param {number} type @param {string} src */
  function compile(gl, type, src) {
    var sh = gl.createShader(type);
    if (!sh) throw new Error("glsl: could not create shader");
    gl.shaderSource(sh, src);
    gl.compileShader(sh);
    if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
      // The author's line numbers are what matter, so report the driver's log as-is; the
      // preamble is prepended, so subtract its lines before quoting a number back.
      var log = gl.getShaderInfoLog(sh) || "unknown compile error";
      gl.deleteShader(sh);
      throw new Error("glsl: " + log.trim());
    }
    return sh;
  }

  /**
   * @param {string} src @param {any} api
   * @param {{name: string|null, viewof: string|null, inputs: string[], kind: string}} opts
   */
  function setupGlsl(src, api, opts) {
    var canvas = document.createElement("canvas");
    canvas.className = "tali-glsl-canvas";
    // A shader canvas is a figure, not a control: nothing to focus, nothing to read.
    // `role="img"` + the cell's own caption is what a screen reader should get, and an
    // author who wants more writes a `fig-cap:`.
    canvas.setAttribute("role", "img");
    // `const` so the null-guard survives into the draw/dispose closures below.
    const gl = /** @type {WebGLRenderingContext | null} */ (
      canvas.getContext("webgl", { antialias: true, alpha: true })
    );
    if (!gl) throw new Error("glsl: WebGL is unavailable in this browser");

    var names = opts.inputs.filter(function (n) { return /^[A-Za-z_]\w*$/.test(n); });
    var frag = preamble(opts.inputs, api) + src;
    var prog = gl.createProgram();
    if (!prog) throw new Error("glsl: could not create program");
    var vs = compile(gl, gl.VERTEX_SHADER, VERT);
    var fs = compile(gl, gl.FRAGMENT_SHADER, frag);
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      throw new Error("glsl: " + (gl.getProgramInfoLog(prog) || "link failed").trim());
    }
    gl.useProgram(prog);

    // One full-screen triangle (not two triangles): fewer vertices, no seam down the
    // diagonal, and `uv` still spans 0..1 across the visible area.
    var buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    var aPos = gl.getAttribLocation(prog, "a_pos");
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

    var uRes = gl.getUniformLocation(prog, "u_res");
    var uTime = gl.getUniformLocation(prog, "u_time");
    var uIn = names.map(function (n) { return [n, gl.getUniformLocation(prog, "u_" + n)]; });
    // Animate ONLY when the author's source actually reads `u_time`. A shader that does not
    // is drawn on mount and on each input change and then costs nothing — a page of static
    // shaders must not pin a core at 60 fps for pixels that never change.
    var animated = /\bu_time\b/.test(src);
    var t0 = 0;
    var raf = 0;
    var dead = false;

    function resize() {
      var dpr = Math.min(window.devicePixelRatio || 1, 2); // 2 is enough; 3 quadruples fill for nothing
      var w = Math.max(1, Math.round(canvas.clientWidth * dpr));
      var h = Math.max(1, Math.round(canvas.clientHeight * dpr));
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w;
        canvas.height = h;
      }
    }

    /** @param {number} nowMs */
    function draw(nowMs) {
      if (dead || !gl) return;
      resize();
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.useProgram(prog);
      if (uRes) gl.uniform2f(uRes, canvas.width, canvas.height);
      if (uTime) {
        if (!t0) t0 = nowMs;
        gl.uniform1f(uTime, (nowMs - t0) / 1000);
      }
      uIn.forEach(function (pair) {
        var loc = /** @type {WebGLUniformLocation | null} */ (pair[1]);
        if (!loc) return; // declared but unused: the linker drops it, which is not an error
        var v = api.value(/** @type {string} */ (pair[0]));
        if (v && typeof v === "object" && typeof v.x === "number") gl.uniform2f(loc, v.x, v.y);
        else gl.uniform1f(loc, typeof v === "number" ? v : (v ? 1 : 0));
      });
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }

    /** @param {number} now */
    function loop(now) {
      if (dead) return;
      draw(now);
      raf = requestAnimationFrame(loop);
    }

    // A STATIC shader is drawn once, and `resize()` assigns `canvas.width` — which clears
    // the drawing buffer. So any layout change (a window resize, the reader's text-size
    // control, a `.scrolly` stage settling) blanked an unanimated shader with nothing left
    // to redraw it. An animated one hid the bug: it redraws next frame either way.
    var ro = typeof ResizeObserver === "function"
      ? new ResizeObserver(function () {
          if (!dead && !animated) requestAnimationFrame(draw);
        })
      : null;
    if (ro) ro.observe(canvas);

    return {
      // A re-run (an input changed) redraws in place: the canvas keeps its context, its
      // program and its bitmap, so the wrapper is handed nothing to mount. Returning the
      // canvas instead would re-parent it on every frame of a driven demo.
      run: function () {
        if (canvas.parentNode !== api.container) api.container.replaceChildren(canvas);
        if (animated) {
          if (!raf) raf = requestAnimationFrame(loop);
        } else {
          // One frame, after layout, so `clientWidth` is real rather than 0.
          requestAnimationFrame(draw);
        }
        return undefined;
      },
      dispose: function () {
        dead = true;
        if (raf) { cancelAnimationFrame(raf); raf = 0; }
        if (ro) ro.disconnect();
        // Hand the GPU context back rather than waiting for GC: a book chapter edited
        // twenty times would otherwise walk into the browser's ~16-context ceiling and
        // start silently killing the OLDEST canvases on the page.
        var lose = gl && gl.getExtension("WEBGL_lose_context");
        if (lose) lose.loseContext();
      },
    };
  }

  if (window.taliJs && window.taliJs.registerLanguage) {
    window.taliJs.registerLanguage("application/tali-glsl", setupGlsl);
  }
})();
