// Hero clip for the marketing site: the live-edit loop. Scroll through a real
// document, return to the top, then edit one block — only that block re-renders,
// in place, with a change-flash. `node record.mjs demos/live-edit.mjs`.
export default {
  name: "live-edit",
  doc: "demos/live-edit.tmd",
  viewport: { width: 1100, height: 720 },
  theme: "dark",
  // MP4 keeps the whole demo; the GIF is just the live-edit beat (small + focused).
  gif: { fps: 15, width: 800, clip: [12.0, 16.5] },

  async steps(page, { sleep, smoothScroll, editDoc }) {
    await sleep(1300); // first paint (KaTeX + mermaid settle)
    await smoothScroll(0.5, 4500); // down through the math + the diagram
    await sleep(600);
    await smoothScroll(1.0, 3800); // down to the callouts
    await sleep(700);
    await smoothScroll(0.0, 3000); // back to the top for the edit
    await sleep(900);

    // The beat: edit the opening sentence. Only that paragraph re-renders, in
    // place, and flashes — the rest of the page (and the diagram) never moves.
    await editDoc((src) =>
      src.replace(
        "against the gradient — with a learning rate $\\eta$:",
        "against the gradient — one small step at a time, with a learning rate $\\eta$:",
      ),
    );
    await sleep(2600);
  },
};
