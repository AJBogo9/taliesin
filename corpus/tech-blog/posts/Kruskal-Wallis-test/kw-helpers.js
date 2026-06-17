// kw-helpers.js
// Shared helper functions for the Kruskal-Wallis interactive visualiser.
// Imported by the OJS cells in index.qmd via:
//   import { gaussianKDE, linspace, silvermanBw } from "./kw-helpers.js"

export function linspace(lo, hi, n) {
  return Array.from({ length: n }, (_, i) => lo + (i / (n - 1)) * (hi - lo));
}

export function silvermanBw(values) {
  const n = values.length;
  const sorted = [...values].sort((a, b) => a - b);
  const mean = values.reduce((s, v) => s + v, 0) / n;
  const variance = values.reduce((s, v) => s + (v - mean) ** 2, 0) / (n - 1);
  const std = Math.sqrt(variance);
  const q1 = sorted[Math.floor(0.25 * n)];
  const q3 = sorted[Math.floor(0.75 * n)];
  const iqr = (q3 - q1) / 1.34;
  const s = Math.min(std, iqr > 0 ? iqr : std);
  return 1.06 * s * Math.pow(n, -0.2);
}

export function gaussianKDE(values, xs, bw) {
  const n = values.length;
  const factor = 1 / (bw * Math.sqrt(2 * Math.PI) * n);
  return xs.map(x => {
    const density = values.reduce((sum, v) => {
      const z = (x - v) / bw;
      return sum + Math.exp(-0.5 * z * z);
    }, 0) * factor;
    return { x, density };
  });
}

export function makeRugData(groups, groupLabels) {
  return groups.flatMap((g, j) =>
    g.map(v => ({ value: v, group: groupLabels[j] }))
  );
}

export function makeKDECurves(groups, groupLabels, xs) {
  return groups.flatMap((g, j) => {
    const bw = silvermanBw(g);
    return gaussianKDE(g, xs, bw).map(({ x, density }) => ({
      x,
      density,
      group: groupLabels[j],
    }));
  });
}
