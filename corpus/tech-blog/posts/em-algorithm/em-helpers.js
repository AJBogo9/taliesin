// em-helpers.js
// Shared helper functions for the EM algorithm interactive visualiser.
// Imported by the OJS cells in index.qmd via:
//   import { gaussPDF, makeCurve, makeResp, makeLLData, linspace } from "./em-helpers.js"

/**
 * Evaluate the Gaussian (Normal) PDF at x given mean mu and std sigma.
 */
export function gaussPDF(x, mu, sigma) {
  return (
    Math.exp(-0.5 * ((x - mu) / sigma) ** 2) /
    (sigma * Math.sqrt(2 * Math.PI))
  );
}

/**
 * Build {x, y, label} curve points for one Gaussian, weighted by pi_k.
 */
export function makeCurve(xs, mu, sigma, pi, label) {
  return xs.map((x) => ({ x, y: pi * gaussPDF(x, mu, sigma), label }));
}

/**
 * Build rug-mark data: one entry per data point with r0 for colour mapping.
 */
export function makeResp(X_data, r_matrix, true_labels = null) {
  return X_data.map((x, i) => ({
    x,
    r0: r_matrix[i][0],
    trueLabel: true_labels ? true_labels[i] : null,
  }));
}

/**
 * Build the log-likelihood trace array for the bottom panel.
 */
export function makeLLData(history) {
  return history.map((h, i) => ({ iter: i + 1, ll: h.ll }));
}

/**
 * Generate n evenly-spaced values from lo to hi (inclusive).
 */
export function linspace(lo, hi, n) {
  return Array.from({ length: n }, (_, i) => lo + (i / (n - 1)) * (hi - lo));
}
