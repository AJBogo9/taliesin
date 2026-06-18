// A demo spec. `node record.mjs demos/sample.mjs` → out/sample.{mp4,gif}.
//
// Shape: { name, doc (relative to this dir), viewport, port?, theme?, gif?, steps }.
// `steps(page, { sleep, smoothScroll, editDoc })` drives the recorded session;
// the doc is restored afterward, so editing it in a step is safe.
export default {
  name: "sample",
  doc: "demos/sample.qmd",
  viewport: { width: 1000, height: 720 },
  theme: "dark",
  // MP4 keeps the whole demo; the GIF is just the live-edit beat (small + focused).
  gif: { fps: 14, width: 760, clip: [13.5, 17.5] },

  async steps(page, { sleep, smoothScroll, editDoc }) {
    await sleep(1200); // first paint (KaTeX + mermaid settle)
    await smoothScroll(0.55, 5000); // scroll through math + the diagram
    await sleep(700);
    await smoothScroll(1.0, 4000); // scroll to the callouts + prose
    await sleep(800);
    await smoothScroll(0.0, 2500); // back to the top for the live edit
    await sleep(600);

    // The live-edit beat: change a heading; only that block re-renders in place.
    await editDoc((src) =>
      src.replace(
        "# Write in Markdown, get a live document",
        "# Write in Markdown, get a live document ✨",
      ),
    );
    await sleep(2200);
  },
};
