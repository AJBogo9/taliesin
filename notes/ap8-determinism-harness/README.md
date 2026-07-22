# AP8 determinism harness (audit-only)

- `det_render.rs` — example: render one `.tmd` to a full page, print to stdout.
  Build: `cp into crates/core/examples/ && cargo build --example det_render -p taliesin-core`.

## Drivers used (run each input in SEPARATE processes → different HashMap seeds → catches map-order nondeterminism):

Single-doc:   for f in $(find corpus docs -name '*.tmd'); do compare sha256 of 3 separate `det_render $f` runs; done
Site build:   for each dir with _site.yml: `TALIESIN_NO_CACHE=1 taliesin build <dir> --out $O1/$O2` twice; `diff -rq $O1 $O2`
Exec path:    build a doc with a warning-emitting {python} cell twice under TALIESIN_NO_CACHE=1; diff the HTML
Leak check:   grep the built output tree for the absolute source path / $HOME / username

## Repro of AP8-1 (the one finding):
  echo '```{python}\nimport matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt; plt.plot([0,1]); plt.show()\n```' > w.tmd
  TALIESIN_NO_CACHE=1 taliesin build w.tmd a.html ; TALIESIN_NO_CACHE=1 taliesin build w.tmd b.html ; diff a.html b.html
  # -> the tali-stderr warning line differs: /tmp/ipykernel_<PID1>/…py vs …<PID2>…
