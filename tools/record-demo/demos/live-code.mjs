// "The part you can't screenshot": edit a {python} cell, the figure re-runs in
// place against the warm kernel. `node record.mjs demos/live-code.mjs`.
// Needs a Python kernel with numpy + matplotlib (TALIESIN_PYTHON).
export default {
  name: "live-code",
  doc: "demos/live-code.tmd",
  viewport: { width: 1100, height: 720 },
  theme: "dark",
  gif: { fps: 15, width: 800, clip: [4.5, 10.5] },

  async steps(page, { sleep, smoothScroll, editDoc }) {
    await sleep(2600); // first paint + the initial cell execution (figure renders)
    await smoothScroll(0.35, 2500); // bring the code + figure into view
    await sleep(1000);

    // Beat 1: 3 → 8 cycles. The cell re-runs; the figure updates in place.
    await editDoc((src) => src.replace("cycles = 3 ", "cycles = 8 "));
    await sleep(3000);

    // Beat 2: 8 → 5, to show it's a live loop, not a one-shot.
    await editDoc((src) => src.replace("cycles = 8 ", "cycles = 5 "));
    await sleep(3000);
  },
};
