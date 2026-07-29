// `num` — the curated numerics namespace `{js}` cells draw with, beside `Plot` and `d3`.
//
// **What this exists for.** The number-one friction in a scientific explorable is that
// there is nowhere to get a pdf from: every cell that wants a Gaussian curve hand-rolls
// `Math.exp(-0.5*z*z)/Math.sqrt(2*Math.PI)`, every cell that wants a reproducible sample
// re-implements a PRNG, and the errors are silent because the shapes still look plausible.
// This is one more drawing global that removes exactly that: no reactive-graph change, no
// scheduler change, nothing here knows a cell exists.
//
// **What this is not, and must not become.** It is not a numeric VM, an array library, or
// a competitor to numpy. The rule for adding anything: it must be something a scientific
// explorable needs *in the cell that draws*, and it must be small enough to read. Anything
// needing a matrix decomposition beyond 2x2 or Cholesky belongs in a `{python}` cell, where
// there is a real kernel. Everything is plain arrays — no wrapper type — so a value crosses
// straight into Plot/d3 with no conversion.
//
// Vendored: nothing. All first-party (that is why it can ship on the `{js}` gate).
(function () {
  "use strict";

  // --- special functions ----------------------------------------------------
  // Lanczos g=7, n=9: ~15 significant digits over the range a document will ever ask for,
  // in twelve lines. The reflection formula covers x < 0.5, where the series is unusable.
  var LANCZOS = [
    0.99999999999980993, 676.5203681218851, -1259.1392167224028,
    771.32342877765313, -176.61502916214059, 12.507343278686905,
    -0.13857109526572012, 9.9843695780195716e-6, 1.5056327351493116e-7,
  ];

  /** log Γ(x). @param {number} x @returns {number} */
  function lgamma(x) {
    if (x < 0.5) return Math.log(Math.PI / Math.sin(Math.PI * x)) - lgamma(1 - x);
    x -= 1;
    var a = LANCZOS[0];
    var t = x + 7.5;
    for (var i = 1; i < 9; i++) a += LANCZOS[i] / (x + i);
    return 0.5 * Math.log(2 * Math.PI) + (x + 0.5) * Math.log(t) - t + Math.log(a);
  }

  /** Γ(x). @param {number} x @returns {number} */
  function gammafn(x) {
    return x < 0.5
      ? Math.PI / (Math.sin(Math.PI * x) * gammafn(1 - x))
      : Math.exp(lgamma(x));
  }

  /** The error function, Abramowitz & Stegun 7.1.26 refined by a Chebyshev fit (|ε| < 1.2e-7). @param {number} x */
  function erf(x) {
    var s = x < 0 ? -1 : 1;
    var z = Math.abs(x);
    var t = 1 / (1 + 0.5 * z);
    var y = t * Math.exp(
      -z * z - 1.26551223 + t * (1.00002368 + t * (0.37409196 + t * (0.09678418 +
      t * (-0.18628806 + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587 +
      t * (-0.82215223 + t * 0.17087277)))))))));
    return s * (1 - y);
  }

  /** Regularized lower incomplete gamma P(a, x): series below the transition, continued
   * fraction above it, which is the standard split (Numerical Recipes §6.2).
   * @param {number} a @param {number} x */
  function lowerGamma(a, x) {
    if (x <= 0) return 0;
    if (x < a + 1) {
      var ap = a;
      var sum = 1 / a;
      var del = sum;
      for (var i = 0; i < 300; i++) {
        ap += 1;
        del *= x / ap;
        sum += del;
        if (Math.abs(del) < Math.abs(sum) * 1e-14) break;
      }
      return sum * Math.exp(-x + a * Math.log(x) - lgamma(a));
    }
    var b = x + 1 - a;
    var c = 1e300;
    var d = 1 / b;
    var h = d;
    for (var j = 1; j < 300; j++) {
      var an = -j * (j - a);
      b += 2;
      d = an * d + b;
      if (Math.abs(d) < 1e-300) d = 1e-300;
      c = b + an / c;
      if (Math.abs(c) < 1e-300) c = 1e-300;
      d = 1 / d;
      var delta = d * c;
      h *= delta;
      if (Math.abs(delta - 1) < 1e-14) break;
    }
    return 1 - Math.exp(-x + a * Math.log(x) - lgamma(a)) * h;
  }

  /** Regularized incomplete beta I_x(a, b) by Lentz's continued fraction.
   * @param {number} a @param {number} b @param {number} x @returns {number} */
  function incBeta(a, b, x) {
    if (x <= 0) return 0;
    if (x >= 1) return 1;
    // The fraction converges fast only on one side of the symmetry point; reflect otherwise.
    if (x > (a + 1) / (a + b + 2)) return 1 - incBeta(b, a, 1 - x);
    var lbeta = lgamma(a) + lgamma(b) - lgamma(a + b);
    var front = Math.exp(a * Math.log(x) + b * Math.log(1 - x) - lbeta) / a;
    var f = 1, c = 1, d = 0;
    for (var i = 0; i <= 250; i++) {
      var m = Math.floor(i / 2);
      var num;
      if (i === 0) num = 1;
      else if (i % 2 === 0) num = (m * (b - m) * x) / ((a + 2 * m - 1) * (a + 2 * m));
      else num = -((a + m) * (a + b + m) * x) / ((a + 2 * m) * (a + 2 * m + 1));
      d = 1 + num * d;
      if (Math.abs(d) < 1e-300) d = 1e-300;
      d = 1 / d;
      c = 1 + num / c;
      if (Math.abs(c) < 1e-300) c = 1e-300;
      var cd = c * d;
      f *= cd;
      if (Math.abs(1 - cd) < 1e-14) break;
    }
    return front * (f - 1);
  }

  // --- distributions --------------------------------------------------------
  // Each is `{pdf, cdf}` (`pmf` where the support is discrete), taking parameters in the
  // order a statistics text writes them. Out-of-support inputs return 0 / a clamped cdf
  // rather than NaN: a plot with one NaN in it renders as nothing at all, and "the curve
  // vanished" is a far worse diagnostic than a flat zero.
  var gaussian = {
    /** @param {number} x @param {number} [mu] @param {number} [sigma] */
    pdf: function (x, mu, sigma) {
      var m = mu === undefined ? 0 : mu;
      var s = sigma === undefined ? 1 : sigma;
      if (!(s > 0)) return NaN;
      var z = (x - m) / s;
      return Math.exp(-0.5 * z * z) / (s * Math.sqrt(2 * Math.PI));
    },
    /** @param {number} x @param {number} [mu] @param {number} [sigma] */
    cdf: function (x, mu, sigma) {
      var m = mu === undefined ? 0 : mu;
      var s = sigma === undefined ? 1 : sigma;
      return 0.5 * (1 + erf((x - m) / (s * Math.SQRT2)));
    },
  };

  var gamma = {
    /** shape–rate parameterization (the Bayesian convention). @param {number} x @param {number} shape @param {number} [rate] */
    pdf: function (x, shape, rate) {
      var b = rate === undefined ? 1 : rate;
      if (x < 0) return 0;
      if (x === 0) return shape < 1 ? Infinity : (shape === 1 ? b : 0);
      return Math.exp(shape * Math.log(b) + (shape - 1) * Math.log(x) - b * x - lgamma(shape));
    },
    /** @param {number} x @param {number} shape @param {number} [rate] */
    cdf: function (x, shape, rate) {
      var b = rate === undefined ? 1 : rate;
      return x <= 0 ? 0 : lowerGamma(shape, b * x);
    },
  };

  var beta = {
    /** @param {number} x @param {number} a @param {number} b */
    pdf: function (x, a, b) {
      if (x < 0 || x > 1) return 0;
      return Math.exp((a - 1) * Math.log(x) + (b - 1) * Math.log(1 - x)
        - (lgamma(a) + lgamma(b) - lgamma(a + b)));
    },
    /** @param {number} x @param {number} a @param {number} b */
    cdf: function (x, a, b) { return incBeta(a, b, x); },
  };

  var poisson = {
    /** @param {number} k @param {number} lambda */
    pmf: function (k, lambda) {
      if (k < 0 || k !== Math.round(k)) return 0;
      return Math.exp(k * Math.log(lambda) - lambda - lgamma(k + 1));
    },
    /** @param {number} k @param {number} lambda */
    cdf: function (k, lambda) {
      var n = Math.floor(k);
      return n < 0 ? 0 : 1 - lowerGamma(n + 1, lambda);
    },
  };

  var exponential = {
    /** @param {number} x @param {number} [rate] */
    pdf: function (x, rate) {
      var l = rate === undefined ? 1 : rate;
      return x < 0 ? 0 : l * Math.exp(-l * x);
    },
    /** @param {number} x @param {number} [rate] */
    cdf: function (x, rate) {
      var l = rate === undefined ? 1 : rate;
      return x < 0 ? 0 : 1 - Math.exp(-l * x);
    },
  };

  // --- summary statistics ---------------------------------------------------
  /** @param {number[]} xs */
  function sum(xs) {
    // Neumaier compensation: a 100k-point trace summed naively loses digits exactly where
    // a document is trying to show that two quantities agree.
    var s = 0, c = 0;
    for (var i = 0; i < xs.length; i++) {
      var t = s + xs[i];
      c += Math.abs(s) >= Math.abs(xs[i]) ? (s - t) + xs[i] : (xs[i] - t) + s;
      s = t;
    }
    return s + c;
  }
  /** @param {number[]} xs */
  function mean(xs) { return xs.length ? sum(xs) / xs.length : NaN; }
  /** Sample variance (n−1). @param {number[]} xs @param {boolean} [population] */
  function variance(xs, population) {
    var n = xs.length;
    if (n < 2) return population && n === 1 ? 0 : NaN;
    var m = mean(xs);
    var ss = sum(xs.map(function (x) { return (x - m) * (x - m); }));
    return ss / (population ? n : n - 1);
  }
  /** @param {number[]} xs @param {boolean} [population] */
  function sd(xs, population) { return Math.sqrt(variance(xs, population)); }
  /** Linear-interpolated quantile (the R type-7 / numpy default). @param {number[]} xs @param {number} p */
  function quantile(xs, p) {
    if (!xs.length) return NaN;
    var s = xs.slice().sort(function (a, b) { return a - b; });
    var h = (s.length - 1) * Math.min(1, Math.max(0, p));
    var lo = Math.floor(h);
    return s[lo] + (h - lo) * (s[Math.min(s.length - 1, lo + 1)] - s[lo]);
  }
  /** @param {number[]} xs */
  function median(xs) { return quantile(xs, 0.5); }

  // --- seeded PRNG ----------------------------------------------------------
  // **Seeded by default and by design.** A published explorable that resamples on every
  // re-render is not reproducible: the prose says "note the outlier at the right" and the
  // outlier is gone. `Math.random()` cannot be seeded, so every sampling demo needs this.
  // sfc32, a small fast counter generator: 128-bit state, passes PractRand, ~10 lines.
  /** @param {number} [seed] */
  function random(seed) {
    var s = (seed === undefined ? 1 : seed) >>> 0;
    // Expand a single integer seed into four words (splitmix32) so seed 1 and seed 2 give
    // uncorrelated streams rather than adjacent ones.
    var a = 0, b = 0, c = 0, d = 0;
    var mix = function () {
      s = (s + 0x9e3779b9) >>> 0;
      var z = s;
      z = Math.imul(z ^ (z >>> 16), 0x21f0aaad) >>> 0;
      z = Math.imul(z ^ (z >>> 15), 0x735a2d97) >>> 0;
      return (z ^ (z >>> 15)) >>> 0;
    };
    a = mix(); b = mix(); c = mix(); d = mix();
    function u32() {
      a >>>= 0; b >>>= 0; c >>>= 0; d >>>= 0;
      var t = (a + b) >>> 0;
      a = b ^ (b >>> 9);
      b = (c + (c << 3)) >>> 0;
      c = (c << 21) | (c >>> 11);
      d = (d + 1) >>> 0;
      t = (t + d) >>> 0;
      c = (c + t) >>> 0;
      return t >>> 0;
    }
    /** @type {number | null} */
    var spare = null;
    var api = {
      /** uniform on [0, 1). */
      uniform: function () { return u32() / 4294967296; },
      /** uniform integer on [0, n). @param {number} n */
      int: function (n) { return Math.floor(api.uniform() * n); },
      /** @param {number} [mu] @param {number} [sigma] */
      normal: function (mu, sigma) {
        var m = mu === undefined ? 0 : mu;
        var s = sigma === undefined ? 1 : sigma;
        // Marsaglia polar, keeping the second variate: half the calls cost no logs at all.
        if (spare !== null) { var v = spare; spare = null; return m + s * v; }
        var x, y, q;
        do {
          x = api.uniform() * 2 - 1;
          y = api.uniform() * 2 - 1;
          q = x * x + y * y;
        } while (q === 0 || q >= 1);
        var f = Math.sqrt(-2 * Math.log(q) / q);
        spare = y * f;
        return m + s * x * f;
      },
      /** n draws from `fn`. @param {number} n @param {() => number} [fn] */
      sample: function (n, fn) {
        var f = fn || api.uniform;
        var out = new Array(n);
        for (var i = 0; i < n; i++) out[i] = f();
        return out;
      },
    };
    return api;
  }

  // --- small dense linear algebra -------------------------------------------
  // Plain nested arrays, row-major. Sized for what a 2-D explorable needs (a covariance,
  // a precision, a rotation) — deliberately NOT a general solver.
  /** @param {number[][]} A @param {number[][]} B */
  function matmul(A, B) {
    var n = A.length, m = B[0].length, k = B.length;
    var C = [];
    for (var i = 0; i < n; i++) {
      var row = new Array(m).fill(0);
      for (var p = 0; p < k; p++) {
        var a = A[i][p];
        if (a === 0) continue;
        for (var j = 0; j < m; j++) row[j] += a * B[p][j];
      }
      C.push(row);
    }
    return C;
  }
  /** @param {number[][]} A @param {number[]} x */
  function matvec(A, x) {
    return A.map(function (row) {
      return row.reduce(function (s, a, j) { return s + a * x[j]; }, 0);
    });
  }
  /** @param {number[][]} A */
  function transpose(A) {
    return A[0].map(function (_, j) { return A.map(function (row) { return row[j]; }); });
  }
  /** @param {number} n */
  function identity(n) {
    return Array.from({ length: n }, function (_, i) {
      return Array.from({ length: n }, function (_, j) { return i === j ? 1 : 0; });
    });
  }
  /** Lower-triangular Cholesky factor L with A = L Lᵀ; throws if A is not positive
   * definite, which is the message a covariance demo actually wants.
   * @param {number[][]} A */
  function cholesky(A) {
    var n = A.length;
    var L = Array.from({ length: n }, function () { return new Array(n).fill(0); });
    for (var i = 0; i < n; i++) {
      for (var j = 0; j <= i; j++) {
        var s = 0;
        for (var k = 0; k < j; k++) s += L[i][k] * L[j][k];
        if (i === j) {
          var d = A[i][i] - s;
          if (!(d > 0)) throw new Error("num.cholesky: matrix is not positive definite");
          L[i][j] = Math.sqrt(d);
        } else {
          L[i][j] = (A[i][j] - s) / L[j][j];
        }
      }
    }
    return L;
  }
  /** Inverse of a 2x2. @param {number[][]} A */
  function inv2(A) {
    var det = A[0][0] * A[1][1] - A[0][1] * A[1][0];
    if (det === 0) throw new Error("num.inv2: matrix is singular");
    return [[A[1][1] / det, -A[0][1] / det], [-A[1][0] / det, A[0][0] / det]];
  }
  /** Eigenvalues + unit eigenvectors of a SYMMETRIC 2x2, closed form (no iteration).
   * Returns `{values: [l1, l2], vectors: [[..],[..]]}` with l1 >= l2 and vectors as
   * COLUMNS — the order an ellipse-drawing demo expects.
   * @param {number[][]} A */
  function eig2(A) {
    var a = A[0][0], b = A[0][1], d = A[1][1];
    var tr = a + d;
    var disc = Math.sqrt(Math.max(0, (a - d) * (a - d) + 4 * b * b));
    var l1 = (tr + disc) / 2;
    var l2 = (tr - disc) / 2;
    var v1 = b === 0 ? (a >= d ? [1, 0] : [0, 1]) : [l1 - d, b];
    var v2 = b === 0 ? (a >= d ? [0, 1] : [1, 0]) : [l2 - d, b];
    var unit = /** @param {number[]} v */ function (v) {
      var n = Math.hypot(v[0], v[1]) || 1;
      return [v[0] / n, v[1] / n];
    };
    return { values: [l1, l2], vectors: transpose([unit(v1), unit(v2)]) };
  }

  // --- grids ----------------------------------------------------------------
  /** `n` points from `a` to `b` inclusive. @param {number} a @param {number} b @param {number} [n] */
  function linspace(a, b, n) {
    var k = n === undefined ? 100 : Math.max(1, Math.floor(n));
    if (k === 1) return [a];
    return Array.from({ length: k }, function (_, i) { return a + ((b - a) * i) / (k - 1); });
  }
  /** Integers `[a, b)` (or `[0, a)` with one argument). @param {number} a @param {number} [b] */
  function range(a, b) {
    var lo = b === undefined ? 0 : a;
    var hi = b === undefined ? a : b;
    var out = [];
    for (var i = lo; i < hi; i++) out.push(i);
    return out;
  }

  window.taliNum = {
    gaussian: gaussian,
    normal: gaussian, // the name half of statistics uses for the same thing
    gamma: gamma,
    beta: beta,
    poisson: poisson,
    exponential: exponential,
    lgamma: lgamma,
    gammafn: gammafn,
    erf: erf,
    sum: sum,
    mean: mean,
    variance: variance,
    sd: sd,
    median: median,
    quantile: quantile,
    random: random,
    matmul: matmul,
    matvec: matvec,
    transpose: transpose,
    identity: identity,
    cholesky: cholesky,
    inv2: inv2,
    eig2: eig2,
    linspace: linspace,
    range: range,
  };
})();
